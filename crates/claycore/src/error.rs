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
    let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
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
