//! Mapping the engine's result codes onto Rust errors.
//!
//! The engine records a thread-local detail message for the last failing call.
//! That message is overwritten by the next failure on the same thread, so it
//! is read here at the moment of failure and carried in the error value —
//! never fetched later, when it may describe something else entirely.

use std::ffi::CStr;
use std::fmt;

use claycore_sys as sys;

/// The engine's result code as bindgen represents it: a plain integer, since
/// `clay_result` is a C enum whose constants live in a module of the same name.
pub(crate) type RawResult = sys::clay_result::Type;

/// Why an engine call failed.
///
/// Mirrors `clay_result` minus its success value, which is represented by
/// `Ok`. `Unsupported` is deliberately ordinary: several backends report it
/// for operations they do not implement, and the caller is expected to fall
/// back rather than treat it as a fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidArgument,
    NotFound,
    BufferTooSmall,
    Io,
    /// The file was written by a newer engine than this build understands.
    ForwardVersion,
    BudgetExceeded,
    /// The backend does not implement this operation. Fall back; do not fail.
    Unsupported,
    Backend,
    /// The user stopped it. An ordinary outcome of an interactive session
    /// rather than a fault: a cancelled operation leaves everything it was
    /// given exactly as it found it, so a caller unwinds rather than repairs.
    Cancelled,
    /// A result code this build does not know, carried verbatim.
    Unknown(i32),
}

impl ErrorKind {
    fn from_raw(code: RawResult) -> Option<Self> {
        use sys::clay_result as r;
        Some(match code {
            r::CLAY_OK => return None,
            r::CLAY_ERROR_INVALID_ARGUMENT => Self::InvalidArgument,
            r::CLAY_ERROR_NOT_FOUND => Self::NotFound,
            r::CLAY_ERROR_BUFFER_TOO_SMALL => Self::BufferTooSmall,
            r::CLAY_ERROR_IO => Self::Io,
            r::CLAY_ERROR_FORWARD_VERSION => Self::ForwardVersion,
            r::CLAY_ERROR_BUDGET_EXCEEDED => Self::BudgetExceeded,
            r::CLAY_ERROR_UNSUPPORTED => Self::Unsupported,
            r::CLAY_ERROR_BACKEND => Self::Backend,
            r::CLAY_ERROR_CANCELLED => Self::Cancelled,
            other => Self::Unknown(other as i32),
        })
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidArgument => "invalid argument",
            Self::NotFound => "not found",
            Self::BufferTooSmall => "buffer too small",
            Self::Io => "I/O error",
            Self::ForwardVersion => "written by a newer engine version",
            Self::BudgetExceeded => "memory budget exceeded",
            Self::Unsupported => "unsupported by this backend",
            Self::Backend => "backend error",
            Self::Cancelled => "cancelled",
            Self::Unknown(_) => "unknown engine result",
        }
    }
}

/// An engine call that failed, with the detail message captured at the point
/// of failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClayError {
    kind: ErrorKind,
    /// What the engine was asked to do, for a message a user can act on.
    operation: &'static str,
    detail: Option<String>,
}

impl ClayError {
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn operation(&self) -> &str {
        self.operation
    }

    /// The engine's own description, when it recorded one.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    /// True when the failure means "this backend does not do that", which is a
    /// routing decision rather than a fault.
    pub fn is_unsupported(&self) -> bool {
        self.kind == ErrorKind::Unsupported
    }
}

impl fmt::Display for ClayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.operation, self.kind.as_str())?;
        if let Some(detail) = &self.detail {
            write!(f, " ({detail})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ClayError {}

pub type Result<T> = std::result::Result<T, ClayError>;

#[cfg(feature = "test-support")]
impl ClayError {
    /// Builds an error of a given kind, for tests.
    ///
    /// Some engine errors cannot be produced on demand — `Unsupported` needs a
    /// backend that declines the operation, which not every machine has. A
    /// test that fabricates one by guessing at an input tests the guess
    /// instead of the handling, which is how the first version of the
    /// acceleration policy's tests came to assert against `NotFound`.
    pub fn for_testing(kind: ErrorKind, operation: &'static str) -> Self {
        Self {
            kind,
            operation,
            detail: Some("synthesised for a test".to_string()),
        }
    }
}

/// Reads the engine's thread-local message for the failure that just happened.
///
/// Called only from [`check`], immediately after the failing call, because the
/// next failure on this thread replaces it.
fn take_last_error() -> Option<String> {
    // SAFETY: `clay_last_error` returns either NULL or a pointer to a
    // NUL-terminated string owned by the engine's thread-local storage, valid
    // until the next failing call on this thread. It is copied before return.
    let ptr = unsafe { sys::clay_last_error() };
    if ptr.is_null() {
        return None;
    }
    let text = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Turns a raw `clay_result` into a `Result`, capturing the detail message.
///
/// `operation` names the call for the error message; it is a static string so
/// that constructing an error allocates only the detail it actually has.
pub(crate) fn check(code: RawResult, operation: &'static str) -> Result<()> {
    match ErrorKind::from_raw(code) {
        None => Ok(()),
        Some(kind) => Err(ClayError {
            kind,
            operation,
            detail: take_last_error(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every result code `clay_result` defines, and the kind it becomes.
    ///
    /// Written out rather than derived, because the whole point of the list is
    /// to be a second opinion: a `match` that fell through to `Unknown` would
    /// still compile, still return an error, and still print something
    /// plausible, and the only thing that can catch it is a table someone read
    /// the header to write.
    const VOCABULARY: &[(RawResult, ErrorKind)] = &[
        (
            sys::clay_result::CLAY_ERROR_INVALID_ARGUMENT,
            ErrorKind::InvalidArgument,
        ),
        (sys::clay_result::CLAY_ERROR_NOT_FOUND, ErrorKind::NotFound),
        (
            sys::clay_result::CLAY_ERROR_BUFFER_TOO_SMALL,
            ErrorKind::BufferTooSmall,
        ),
        (sys::clay_result::CLAY_ERROR_IO, ErrorKind::Io),
        (
            sys::clay_result::CLAY_ERROR_FORWARD_VERSION,
            ErrorKind::ForwardVersion,
        ),
        (
            sys::clay_result::CLAY_ERROR_BUDGET_EXCEEDED,
            ErrorKind::BudgetExceeded,
        ),
        (
            sys::clay_result::CLAY_ERROR_UNSUPPORTED,
            ErrorKind::Unsupported,
        ),
        (sys::clay_result::CLAY_ERROR_BACKEND, ErrorKind::Backend),
        (sys::clay_result::CLAY_ERROR_CANCELLED, ErrorKind::Cancelled),
    ];

    #[test]
    fn success_is_not_an_error() {
        assert_eq!(ErrorKind::from_raw(sys::clay_result::CLAY_OK), None);
        assert!(check(sys::clay_result::CLAY_OK, "nothing").is_ok());
    }

    /// Every code the header defines reaches a named kind, through the same
    /// path a real call takes.
    #[test]
    fn every_result_code_arrives_as_the_kind_it_names() {
        for (code, expected) in VOCABULARY {
            assert_eq!(
                ErrorKind::from_raw(*code),
                Some(*expected),
                "result code {code} did not map to {expected:?}"
            );
            let error = check(*code, "a call that failed")
                .expect_err("a non-zero result code came back as success");
            assert_eq!(
                error.kind(),
                *expected,
                "the code survived `from_raw` and was lost on the way into the error"
            );
            assert_eq!(error.operation(), "a call that failed");
        }
    }

    /// A kind whose sentence is another kind's sentence is a kind a user
    /// cannot tell from it.
    #[test]
    fn no_two_kinds_say_the_same_thing() {
        for (i, (_, left)) in VOCABULARY.iter().enumerate() {
            for (_, right) in &VOCABULARY[i + 1..] {
                assert_ne!(
                    left.as_str(),
                    right.as_str(),
                    "{left:?} and {right:?} print identically"
                );
            }
        }
    }

    /// A code this build has never heard of is carried verbatim rather than
    /// folded into a neighbouring one, so an upgrade that adds a result code
    /// is legible in the message before anyone has read the header.
    #[test]
    fn a_code_from_the_future_is_carried_rather_than_guessed_at() {
        let beyond = VOCABULARY.iter().map(|(c, _)| *c).max().expect("codes") + 1;
        assert_eq!(
            ErrorKind::from_raw(beyond),
            Some(ErrorKind::Unknown(beyond as i32)),
            "an unrecognised code was mapped onto a kind that means something"
        );
        let error = check(beyond, "a call from a newer engine")
            .expect_err("an unrecognised code came back as success");
        assert!(
            format!("{error}").contains("unknown engine result"),
            "an unrecognised code printed as something knowable: {error}"
        );
    }

    /// The ratchet: the table above against the header itself.
    ///
    /// `VOCABULARY` is a copy of something upstream owns, and a copy is only
    /// as good as the last time someone compared it. This reads the pinned
    /// header and fails when `clay_result` has grown a code nothing here
    /// names — which is the one way an added code can be caught before it
    /// reaches a user as "unknown engine result".
    ///
    /// Skipped rather than failed where the header is not on disk: a package
    /// build has the generated bindings and not the vendored source, and a
    /// test that cannot run is not a test that failed.
    #[test]
    fn the_table_is_every_code_the_pinned_header_declares() {
        let header = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/ClayCore/bindings/c/clay.h");
        let Ok(text) = std::fs::read_to_string(&header) else {
            return;
        };

        // The enum's members, by name, taken from the block that declares it.
        let body = text
            .split_once("typedef enum clay_result")
            .expect("the header declares no clay_result")
            .1;
        let body = &body[..body.find('}').expect("the enum is not closed")];
        let declared: Vec<&str> = body
            .lines()
            .filter_map(|line| line.trim().strip_prefix("CLAY_ERROR_"))
            .map(|rest| {
                rest.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or_default()
            })
            .filter(|name| !name.is_empty())
            .collect();

        assert_eq!(
            declared.len(),
            VOCABULARY.len(),
            "the header declares {} error codes and this table names {}: {declared:?}",
            declared.len(),
            VOCABULARY.len()
        );
    }
}
