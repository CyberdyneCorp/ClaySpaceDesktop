//! Safe Rust over the ClayCore C ABI.
//!
//! This is the only crate that calls [`claycore_sys`], and together with it
//! the only crate in the workspace allowed to contain `unsafe`. Everything
//! above this layer sees ordinary Rust: `Result` instead of result codes,
//! ownership in the type system instead of in the header's prose, and the
//! engine's thread-safety contract expressed as `Send`/`Sync` bounds.
//!
//! # What this layer promises
//!
//! - Every fallible entry point returns [`Result`], carrying the engine's own
//!   detail message captured at the moment of failure.
//! - A handle the caller owns releases on drop; a handle borrowed from a
//!   document cannot outlive it and has no destroy operation.
//! - No panic and no unwind crosses the C boundary.

mod backend;
mod buffer;
mod document;
mod error;

pub use backend::{backends, compiled_backends, Backend};
pub use document::{Document, Item, LayerId, NodeId};
pub use error::{ClayError, ErrorKind, Result};

use claycore_sys as sys;

/// The engine version this build is linked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Reads the linked engine's version.
pub fn version() -> Version {
    let (mut major, mut minor, mut patch) = (0, 0, 0);
    // SAFETY: three out-parameters, all valid for writes of i32. The call
    // cannot fail and has no other effect.
    unsafe { sys::clay_version(&mut major, &mut minor, &mut patch) };
    Version { major, minor, patch }
}

/// The engine ABI this crate was written against.
///
/// The engine's own header warns that while the major version is 0, a minor
/// bump may break the ABI. Since the engine is vendored and built from source
/// here, a mismatch is a compile error rather than a load-time surprise — this
/// constant exists so that a mismatch can also be reported in diagnostics.
pub const EXPECTED_ABI: Version = Version {
    major: 0,
    minor: 26,
    patch: 0,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_the_pinned_engine() {
        let v = version();
        assert_eq!(
            (v.major, v.minor),
            (EXPECTED_ABI.major, EXPECTED_ABI.minor),
            "linked engine {v} is not the ABI this wrapper was written against \
             ({EXPECTED_ABI}); the submodule pin and EXPECTED_ABI disagree"
        );
    }
}
