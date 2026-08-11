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

mod authoring;
mod backend;
mod brick;
mod brush;
mod buffer;
mod consolidate;
mod descriptor;
mod document;
mod error;
mod mask;
mod mesh;
mod pick;
mod reader;
mod sculpt;
mod voxel;

pub use authoring::{Blend, Op, Protection, UndoState};
pub use backend::{backends, compiled_backends, Backend};
pub use brick::{
    BrickCache, BrickConfig, BrickKey, BrickMeshParams, BrickMeshRange, BrickRequest, BrickSamples,
    BrickState, BrickStats,
};
pub use brush::{Accumulation, BrushParams, BrushShape, Falloff, StrokePreset, StrokeSample};
pub use consolidate::{ConsolidationCost, ConsolidationParams, FieldReport};
pub use document::{Document, Item, LayerId, NodeId};
pub use error::{ClayError, ErrorKind, Result};
pub use mask::{ExtrudeSide, Mask, MaskExtrudeParams, MaskField, MaskRef};
pub use mesh::{Mesh, MeshParams, MeshValidity, Mesher, VertexLayout};
pub use pick::{Hit, Snapped};
pub use reader::Reader;
pub use sculpt::{
    resolve_stroke, FlattenMode, FlattenParams, MoveParams, RelaxParams, VolumeParams,
};
pub use voxel::{Cell, RepairReport, VoxelField, VoxelGrid, VoxelGridRef};

use claycore_sys as sys;

/// Builds an error for a call that reported failure by returning null rather
/// than a result code.
pub(crate) fn raw_failure(operation: &'static str, kind: ErrorKind) -> ClayError {
    let raw = match kind {
        ErrorKind::InvalidArgument => sys::clay_result::CLAY_ERROR_INVALID_ARGUMENT,
        ErrorKind::Io => sys::clay_result::CLAY_ERROR_IO,
        ErrorKind::NotFound => sys::clay_result::CLAY_ERROR_NOT_FOUND,
        _ => sys::clay_result::CLAY_ERROR_BACKEND,
    };
    match error::check(raw, operation) {
        Err(e) => e,
        Ok(()) => unreachable!("a failure code is not a success code"),
    }
}

/// An interior NUL cannot reach the engine, so it is rejected here rather than
/// silently truncating the caller's string.
pub(crate) fn cstring(value: &str, operation: &'static str) -> Result<std::ffi::CString> {
    std::ffi::CString::new(value).map_err(|_| raw_failure(operation, ErrorKind::InvalidArgument))
}

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
    Version {
        major,
        minor,
        patch,
    }
}

/// The engine ABI this crate was written against.
///
/// The engine's own header warns that while the major version is 0, a minor
/// bump may break the ABI. Since the engine is vendored and built from source
/// here, a mismatch is a compile error rather than a load-time surprise — this
/// constant exists so that a mismatch can also be reported in diagnostics.
pub const EXPECTED_ABI: Version = Version {
    major: 0,
    minor: 27,
    patch: 3,
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
