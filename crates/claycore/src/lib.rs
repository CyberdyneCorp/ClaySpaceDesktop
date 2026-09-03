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
//! - Every wrapper is executed by a test, including the ones nothing above
//!   this layer calls yet. The surface is deliberately kept whole rather than
//!   trimmed to today's callers — the engine's entry points are what this
//!   crate exists to expose — but a wrapper nobody runs is a SAFETY comment
//!   nobody has checked. `tests/abi_surface.rs` runs the scattered ones the
//!   application has not reached for; a tier large enough to have vocabulary
//!   of its own gets a file named after it (`tests/multires.rs`,
//!   `tests/surface_view.rs`, `tests/maintenance.rs`, `tests/memory.rs`),
//!   because the assertions there are about what the tier *claims* and not
//!   only about having been called.

mod authoring;
mod backend;
mod brick;
mod brush;
mod buffer;
mod consolidate;
mod descriptor;
mod document;
mod error;
mod live;
mod maintenance;
mod mask;
mod memory;
mod mesh;
mod mesh_sculpt;
mod multires;
mod pick;
mod reader;
mod remesh;
mod sculpt;
mod surface_view;
mod voxel;

pub use authoring::{
    Blend, Influence, LayerInfo, LayerRepresentation, LayerTransform, Op, Protection, UndoState,
    Undone,
};
pub use backend::{backends, compiled_backends, Backend};
pub use brick::{
    BrickCache, BrickConfig, BrickKey, BrickMeshParams, BrickMeshRange, BrickRequest, BrickSamples,
    BrickState, BrickStats, BrickSubmit, BrickValues,
};
pub use brush::{Accumulation, BrushParams, BrushShape, Falloff, StrokePreset, StrokeSample};
pub use consolidate::{ConsolidationCost, ConsolidationParams, FieldReport};
pub use document::{
    prim, ArmatureEdit, Document, FormatVersion, GizmoCage, Item, LayerId, NodeId, PointType,
    Primitive, Profile,
};
pub use error::{ClayError, ErrorKind, Result};
pub use live::{
    MoveTransaction, PreviewBrick, PreviewDelta, PreviewGrab, PreviewPending, SculptBudget,
    SculptDirty, SculptPolicy, SmoothTransaction,
};
pub use maintenance::{
    MaintenanceItem, MaintenanceKind, MaintenanceQueue, StrokeGuard, StrokeScope,
};
pub use mask::{ExtrudeSide, Mask, MaskExtrudeParams, MaskField, MaskLease, MaskRef, MaskSource};
pub use memory::{
    BudgetError, MemoryCategory, MemoryClass, MemoryLedger, MemoryPin, MemoryReport, PinHold,
    Pressure, SculptMemoryProfile, SurfacePreflight, TrimReport,
};
pub use mesh::{ImportBudget, Mesh, MeshLayerDesc, MeshParams, MeshValidity, Mesher, VertexLayout};
pub use mesh_sculpt::{
    AlphaStamp, MeshBrush, MeshDeform, MeshDeformer, MeshDeltas, MeshFalloff, MeshHit, MeshLattice,
    MeshSculptor, MeshSeed, MeshStamp,
};
pub use multires::{
    AddLevelPreflight, ArenaStats, Block, BlockInfo, DetailStamp, DetailStampMode,
    DetailStampReport, Multires, MultiresDesc, MultiresError, MultiresMemory, MultiresRefusal,
    MultiresSculptor, PeakTelemetry, Revisions, SculptLayerId, SculptLayerInfo, SculptLayerKind,
    SculptLayerRevisions, SculptLayerStats, SculptLayerStroke, SmoothMode, StampReport,
    SubdivisionRule, WriteDomain,
};
pub use pick::{Hit, Snapped};
pub use reader::Reader;
pub use remesh::{
    OpenSurface, Projection, RemeshEstimate, RemeshParams, RemeshRefusal, RemeshReport, Resolution,
    SmallComponents, Surface,
};
pub use sculpt::{
    resolve_stroke, FlattenMode, FlattenParams, MoveParams, RelaxParams, TopologicalMoveParams,
    VolumeParams,
};
pub use surface_view::{
    ChunkAck, ChunkCopy, ChunkInfo, ChunkOptions, ChunkReadback, ChunkRevisions, SurfaceKind,
    SurfaceView,
};
pub use voxel::{
    Cell, ChunkRange, MaskedGrid, RepairReport, VoxelField, VoxelGrid, VoxelGridRef, VoxelHit,
    VoxelReader,
};

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

/// A string the engine promises is never null, for any value.
///
/// The `*_text` entry points are documented as total: they answer "unknown"
/// for a value this build does not know rather than returning NULL. Shared
/// because three modules now name enumerations the engine can spell and this
/// crate cannot — a second transcription of a static table is a second thing
/// that can drift out of step with the header.
pub(crate) fn engine_text(ptr: *const std::ffi::c_char) -> &'static str {
    if ptr.is_null() {
        return "unknown";
    }
    // SAFETY: a non-null pointer to a NUL-terminated string literal in the
    // library's own static storage — these entry points return a `const char*`
    // chosen from a fixed table, so it is valid for the life of the process
    // and `'static` is the honest lifetime.
    unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_str()
        .unwrap_or("unknown")
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

/// The vendored engine's git revision, as this build was compiled against it.
pub fn revision() -> &'static str {
    sys::CLAYCORE_REVISION
}

/// The engine ABI this crate was written against.
///
/// The engine's own header warns that while the major version is 0, a minor
/// bump may break the ABI. Since the engine is vendored and built from source
/// here, a mismatch is a compile error rather than a load-time surprise — this
/// constant exists so that a mismatch can also be reported in diagnostics.
pub const EXPECTED_ABI: Version = Version {
    major: 0,
    minor: 78,
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
