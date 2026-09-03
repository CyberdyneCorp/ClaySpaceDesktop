//! The multiresolution surface: a cage, a hierarchy over it, and detail that
//! survives an edit to the form beneath it.
//!
//! A third representation beside the fixed mesh of [`crate::MeshSculptor`] and
//! the field, and the one sentence the whole tier exists to make true:
//!
//! ```text
//! P(0) = the cage    S(n) = Subdivide(P(n-1))    P(n) = S(n) + Frame(n) * D(n)
//! ```
//!
//! An artist cuts wrinkles at level 4, then moves the skull at level 1, and
//! the wrinkles are still there and still lying the way they were laid — not
//! because they were re-projected, but because what is *stored* at level 4 is
//! a displacement in a frame carried up from the level below, so moving the
//! level below moves the frame and the wrinkle rides on it.
//!
//! # What this module models
//!
//! **Opaque and owning.** A [`Multires`] is not a layer of a document and has
//! no `clay_layer_id`: the engine hands back a handle, the host holds it
//! beside its document, and [`Multires::serialize`] hands back the bytes to
//! store with it. Everything that borrows from a hierarchy — a
//! [`MultiresSculptor`] above all — carries a lifetime that cannot outlive it,
//! because the C header is explicit that the surface must outlive the sculptor
//! and the sculptor keeps a bare pointer to it.
//!
//! **Two levels, not one.** Where the brush writes and what the host draws are
//! independent numbers ([`Multires::set_sculpt_level`] against
//! [`Multires::set_display_level`]), because editing a coarse level while
//! displaying a fine one is the workflow the feature exists for.
//!
//! **Three revisions, not one.** [`Multires::revision`] answers with all three,
//! because a host re-uploads an index buffer only when the hierarchy's *shape*
//! changed, re-reads detail only when the *detail* changed, and redraws only
//! when the *evaluated* surface moved. One counter cannot say which happened.
//!
//! # What is deliberately not here yet
//!
//! The sculpt-layer stack (`clay_multires_sculpt_layer_*` and its stroke
//! transaction) and the projection pass (`clay_multires_project`) are left for
//! the changes that adopt them. Both are large surfaces with vocabulary of
//! their own — a sculpt layer is addressed by *id* and never by index, which
//! the application's existing voxel pass stack is not — and a wrapper nothing
//! runs is a SAFETY comment nobody has checked.
//!
//! Per-vertex detail coefficients do not cross the C ABI at all: what a host
//! can ask about the detail field is its [checksum](Multires::detail_checksum)
//! and what it [costs](MultiresMemory::detail). The checksum is the load-
//! bearing one — it is how a host proves to itself that releasing caches or
//! trimming changed nothing that matters.
//!
//! Neither `add_level` nor anything else here takes a cancel token: no wrapper
//! for `clay_cancel_token` exists in this crate yet, so NULL is passed, which
//! the entry point documents as "cannot be cancelled" rather than as an error.

use std::ffi::{c_char, CStr};
use std::ptr::NonNull;

use claycore_sys as sys;

use crate::buffer::size_query_bytes;
use crate::descriptor::Descriptor;
use crate::error::{check, ClayError, ErrorKind, Result};
use crate::mask::MaskField;
use crate::mesh::Mesh;
use crate::mesh_sculpt::MeshStamp;
use crate::raw_failure;

/// A string the engine promises is never null, for any value.
///
/// The `*_text` entry points here are documented as total: they answer
/// "unknown" for a value this build does not know rather than returning NULL.
fn engine_text(ptr: *const c_char) -> &'static str {
    if ptr.is_null() {
        return "unknown";
    }
    // SAFETY: a non-null pointer to a NUL-terminated string literal in the
    // library's own static storage — these entry points return a `const char*`
    // chosen from a fixed table, so it is valid for the life of the process
    // and `'static` is the honest lifetime.
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("unknown")
}

// -- refusals ---------------------------------------------------------------

/// Why the hierarchy refused an operation.
///
/// Mirrors `clay_multires_error`. Distinct from [`ClayError`] on purpose: the
/// result code says an argument was rejected, and this says *which model
/// problem* rejected it — "your cage is not manifold" is a sentence a user can
/// act on and "invalid argument" is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MultiresError {
    /// No reason from the hierarchy itself: the call was refused before it was
    /// asked, or nothing was refused at all.
    None,
    /// The cage has no faces.
    EmptyBase,
    IndexOutOfRange,
    /// A face with repeated or collinear corners. Refused rather than repaired:
    /// a conversion that quietly welds a face changes the retopology somebody
    /// paid for without saying so.
    DegenerateFace,
    /// An edge shared by more than two faces, or a fan that does not close.
    NonManifold,
    LevelOutOfRange,
    /// `remove_highest_level` on a hierarchy that is only its cage.
    NoLevelToRemove,
    /// The level would cost more than the descriptor's `memory_budget`.
    OverBudget,
    Cancelled,
    /// The operation would discard detail that is somebody's work.
    DetailPresent,
    /// Past this build's ceiling on hierarchy depth, refused before anything
    /// is allocated.
    DepthLimit,
    /// A truncated, hostile or newer serialized hierarchy.
    Decode,
    NoSuchSculptLayer,
    /// A sculpt write, a merge or a bake aimed at a locked layer. A lock
    /// refuses a write to a layer's coefficients and still permits every
    /// property change.
    SculptLayerLocked,
    /// A composition change asked for while a stroke is open.
    SculptLayerStrokeOpen,
    /// A count that does not fit the width it is stored in. New in ABI 0.78.0,
    /// and the one refusal here that is about arithmetic rather than about the
    /// model: the failure mode of an unchecked multiply is that the operation
    /// is *allowed*.
    CapacityOverflow,
    /// A value this build does not know, carried verbatim.
    Unknown(i32),
}

impl MultiresError {
    fn from_raw(code: i32) -> Self {
        use sys::clay_multires_error as e;
        match code as sys::clay_multires_error::Type {
            e::CLAY_MULTIRES_OK => Self::None,
            e::CLAY_MULTIRES_EMPTY_BASE => Self::EmptyBase,
            e::CLAY_MULTIRES_INDEX_OUT_OF_RANGE => Self::IndexOutOfRange,
            e::CLAY_MULTIRES_DEGENERATE_FACE => Self::DegenerateFace,
            e::CLAY_MULTIRES_NON_MANIFOLD => Self::NonManifold,
            e::CLAY_MULTIRES_LEVEL_OUT_OF_RANGE => Self::LevelOutOfRange,
            e::CLAY_MULTIRES_NO_LEVEL_TO_REMOVE => Self::NoLevelToRemove,
            e::CLAY_MULTIRES_OVER_BUDGET => Self::OverBudget,
            e::CLAY_MULTIRES_CANCELLED => Self::Cancelled,
            e::CLAY_MULTIRES_DETAIL_PRESENT => Self::DetailPresent,
            e::CLAY_MULTIRES_DEPTH_LIMIT => Self::DepthLimit,
            e::CLAY_MULTIRES_DECODE => Self::Decode,
            e::CLAY_MULTIRES_NO_SUCH_SCULPT_LAYER => Self::NoSuchSculptLayer,
            e::CLAY_MULTIRES_SCULPT_LAYER_LOCKED => Self::SculptLayerLocked,
            e::CLAY_MULTIRES_SCULPT_LAYER_STROKE_OPEN => Self::SculptLayerStrokeOpen,
            e::CLAY_MULTIRES_CAPACITY_OVERFLOW => Self::CapacityOverflow,
            _ => Self::Unknown(code),
        }
    }

    fn to_raw(self) -> i32 {
        use sys::clay_multires_error as e;
        (match self {
            Self::None => e::CLAY_MULTIRES_OK,
            Self::EmptyBase => e::CLAY_MULTIRES_EMPTY_BASE,
            Self::IndexOutOfRange => e::CLAY_MULTIRES_INDEX_OUT_OF_RANGE,
            Self::DegenerateFace => e::CLAY_MULTIRES_DEGENERATE_FACE,
            Self::NonManifold => e::CLAY_MULTIRES_NON_MANIFOLD,
            Self::LevelOutOfRange => e::CLAY_MULTIRES_LEVEL_OUT_OF_RANGE,
            Self::NoLevelToRemove => e::CLAY_MULTIRES_NO_LEVEL_TO_REMOVE,
            Self::OverBudget => e::CLAY_MULTIRES_OVER_BUDGET,
            Self::Cancelled => e::CLAY_MULTIRES_CANCELLED,
            Self::DetailPresent => e::CLAY_MULTIRES_DETAIL_PRESENT,
            Self::DepthLimit => e::CLAY_MULTIRES_DEPTH_LIMIT,
            Self::Decode => e::CLAY_MULTIRES_DECODE,
            Self::NoSuchSculptLayer => e::CLAY_MULTIRES_NO_SUCH_SCULPT_LAYER,
            Self::SculptLayerLocked => e::CLAY_MULTIRES_SCULPT_LAYER_LOCKED,
            Self::SculptLayerStrokeOpen => e::CLAY_MULTIRES_SCULPT_LAYER_STROKE_OPEN,
            Self::CapacityOverflow => e::CLAY_MULTIRES_CAPACITY_OVERFLOW,
            Self::Unknown(other) => return other,
        }) as i32
    }

    /// The engine's own sentence for this refusal.
    pub fn text(self) -> &'static str {
        // SAFETY: the entry point takes a plain integer, is documented as
        // never returning NULL for any value including one it does not know,
        // and answers from a static table.
        engine_text(unsafe { sys::clay_multires_error_text(self.to_raw()) })
    }
}

impl std::fmt::Display for MultiresError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.text())
    }
}

/// A refused hierarchy operation: the result code, and which model problem
/// caused it.
///
/// Both halves are carried because they answer different questions. A host
/// logging a failure wants the result code and the engine's detail message; a
/// host putting a sentence in front of a sculptor wants to know that the cage
/// is not manifold rather than that an argument was invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiresRefusal {
    pub error: ClayError,
    pub reason: MultiresError,
}

impl std::fmt::Display for MultiresRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.reason {
            MultiresError::None => self.error.fmt(f),
            reason => write!(f, "{}: {reason}", self.error.operation()),
        }
    }
}

impl std::error::Error for MultiresRefusal {}

/// Why a priced operation was refused before it was paid for.
///
/// Mirrors `clay_budget_error`. [`Self::Overflow`] is a refusal at *any*
/// budget, including no budget: an estimate nobody can compute is not one
/// anybody may rely on, and a wrapped multiply reports a small number, which
/// would let the operation through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BudgetError {
    /// Nothing was refused.
    None,
    OverBudget,
    Overflow,
    Unknown(i32),
}

impl BudgetError {
    fn from_raw(code: i32) -> Self {
        use sys::clay_budget_error as e;
        match code as sys::clay_budget_error::Type {
            e::CLAY_BUDGET_OK => Self::None,
            e::CLAY_BUDGET_OVER_BUDGET => Self::OverBudget,
            e::CLAY_BUDGET_OVERFLOW => Self::Overflow,
            _ => Self::Unknown(code),
        }
    }

    fn to_raw(self) -> i32 {
        use sys::clay_budget_error as e;
        (match self {
            Self::None => e::CLAY_BUDGET_OK,
            Self::OverBudget => e::CLAY_BUDGET_OVER_BUDGET,
            Self::Overflow => e::CLAY_BUDGET_OVERFLOW,
            Self::Unknown(other) => return other,
        }) as i32
    }

    /// The engine's own sentence for this refusal.
    pub fn text(self) -> &'static str {
        // SAFETY: as `MultiresError::text` — a total function over a plain
        // integer, answering from a static table.
        engine_text(unsafe { sys::clay_budget_error_text(self.to_raw()) })
    }
}

impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.text())
    }
}

// -- building a hierarchy ---------------------------------------------------

/// The subdivision rule.
///
/// Recorded in the serialized stream rather than assumed, because a hierarchy
/// reconstructed with a different rule than it was authored with is a
/// different surface and nothing else in the stream reveals the substitution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubdivisionRule {
    #[default]
    CatmullClark,
}

impl SubdivisionRule {
    fn to_raw(self) -> i32 {
        (match self {
            Self::CatmullClark => sys::clay_subdivision_rule::CLAY_SUBDIVISION_CATMULL_CLARK,
        }) as i32
    }
}

/// How a cage becomes a hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MultiresDesc {
    pub rule: SubdivisionRule,
    /// Vertices closer than this are one geometric point of the cage. `None`
    /// takes the same default the adjacency builder and the fixed sculptor
    /// use, so a mesh welds the same way on every path through the library.
    pub weld_epsilon: Option<f32>,
    /// What a level may cost, in bytes. Zero means no budget — what a desktop
    /// host wants, and what a memory-constrained one must not use without
    /// asking [`Multires::preflight_add_level`] first.
    pub memory_budget: u64,
}

impl MultiresDesc {
    fn to_raw(self) -> sys::clay_multires_desc {
        // The engine's defaults first, then what this descriptor means — the
        // arrangement every `*_defaults` entry point exists for. A failure to
        // read them is not fatal: the zeroed descriptor still carries a valid
        // struct_size, and every field this type does not override has a
        // documented "take the library's own" value of zero.
        let mut raw = sys::clay_multires_desc::sized();
        // SAFETY: a valid versioned descriptor out-parameter whose
        // struct_size is set above, which is what the boundary requires.
        let _ = unsafe { sys::clay_multires_defaults(&mut raw) };
        raw.rule = self.rule.to_raw();
        raw.weld_epsilon = self.weld_epsilon.unwrap_or(0.0);
        raw.memory_budget = self.memory_budget;
        raw
    }
}

// -- reports ----------------------------------------------------------------

/// The three counters a host compares to decide what to do again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Revisions {
    /// The hierarchy's shape: the cage and the level count. Moves when an
    /// index buffer a host uploaded has stopped describing the surface.
    pub base: u64,
    /// The detail coefficients.
    pub detail: u64,
    /// The evaluated positions. Moves whenever the drawn surface moved, for
    /// any reason, which is the one a redraw watches.
    pub evaluated: u64,
}

/// What adding a level would cost, asked before any of it is paid.
///
/// Catmull-Clark multiplies faces by four, so a 20k-quad cage is 5.1M faces at
/// level 4 and 20.5M at level 5. On a memory-constrained device it is
/// [`peak_bytes`](Self::peak_bytes) that kills an application rather than the
/// steady state, which is why the two are reported apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddLevelPreflight {
    /// The level that would come into existence.
    pub level: u32,
    pub vertices: u64,
    pub faces: u64,
    /// Kept for the life of the level.
    pub topology_bytes: u64,
    /// If every vertex of it were detailed.
    pub detail_bytes: u64,
    /// Held only while the level is resident.
    pub evaluated_bytes: u64,
    pub runtime_bytes: u64,
    /// What remains after the call.
    pub persistent_bytes: u64,
    /// The high-water mark during it.
    pub peak_bytes: u64,
    pub allowed: bool,
    /// Why not, when `allowed` is false.
    pub error: MultiresError,
}

impl AddLevelPreflight {
    fn from_raw(raw: sys::clay_multires_preflight) -> Self {
        Self {
            level: raw.level,
            vertices: raw.vertices,
            faces: raw.faces,
            topology_bytes: raw.topology_bytes,
            detail_bytes: raw.detail_bytes,
            evaluated_bytes: raw.evaluated_bytes,
            runtime_bytes: raw.runtime_bytes,
            persistent_bytes: raw.persistent_bytes,
            peak_bytes: raw.peak_bytes,
            allowed: raw.allowed != 0,
            error: MultiresError::from_raw(raw.error),
        }
    }
}

/// What serializing this hierarchy would cost.
///
/// These are *ceilings*, deliberately. The structural figures are exact byte
/// costs, but the arrays that carry them are grown rather than reserved, so a
/// figure measured afterwards includes capacity slack the prediction does not.
/// A budget that errs low is the one that gets an application killed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncodePreflight {
    pub allowed: bool,
    /// Kept, and the user's work.
    pub authoritative_bytes: u64,
    /// Kept, and rebuildable.
    pub runtime_bytes: u64,
    /// The two above: what remains after the call.
    pub persistent_bytes: u64,
    /// The high-water mark during it.
    pub peak_bytes: u64,
    pub error: BudgetError,
}

impl EncodePreflight {
    fn from_raw(raw: sys::clay_surface_preflight) -> Self {
        Self {
            allowed: raw.allowed != 0,
            authoritative_bytes: raw.authoritative_bytes,
            runtime_bytes: raw.runtime_bytes,
            persistent_bytes: raw.persistent_bytes,
            peak_bytes: raw.peak_bytes,
            error: BudgetError::from_raw(raw.error),
        }
    }
}

/// What a hierarchy costs, split by what a host under pressure may act on.
///
/// Authoritative detail is never reported as rebuildable: a host acting on
/// that distinction would delete the user's work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MultiresMemory {
    pub resident_levels: u32,
    /// The cage, with its attributes.
    pub base: u64,
    /// Every level's face list.
    pub topology: u64,
    /// Every level's coefficients — the wrinkles themselves.
    pub detail: u64,
    /// The three above; none of it droppable.
    pub authoritative: u64,
    /// Subdivided positions, frames and normals.
    pub evaluated: u64,
    /// Connectivity, level meshes, adjacency.
    pub runtime_index: u64,
    /// The per-level chunk tables and their face maps. Reported apart because
    /// it follows the face count rather than the vertex count.
    pub chunk_index: u64,
    pub rebuildable: u64,
    pub total: u64,
    /// Every sculpt layer's coefficients and masks. Counted in
    /// `authoritative`, and reported apart from `detail` because the two are
    /// the same quantity under different owners — a host deciding what to
    /// merge, bake or delete needs to see which of them is costing it.
    pub sculpt_layers: u64,
    /// The materialized composition per level. Derived and droppable, so it is
    /// counted in `rebuildable`.
    pub composed: u64,
}

impl MultiresMemory {
    fn from_raw(raw: sys::clay_multires_memory) -> Self {
        Self {
            resident_levels: raw.resident_levels,
            base: raw.base,
            topology: raw.topology,
            detail: raw.detail,
            authoritative: raw.authoritative,
            evaluated: raw.evaluated,
            runtime_index: raw.runtime_index,
            chunk_index: raw.chunk_index,
            rebuildable: raw.rebuildable,
            total: raw.total,
            sculpt_layers: raw.sculpt_layers,
            composed: raw.composed,
        }
    }
}

// -- the shared memory vocabulary -------------------------------------------

/// What it costs to let a category of memory go.
///
/// The split that matters is not "big against small" but what releasing it
/// destroys: the first group is the user's work, the second reconstructs
/// bit-identically, the third is undo depth and belongs to the host's policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryCategory {
    BaseGeometry,
    Topology,
    MultiresDetail,
    SculptLayers,
    Masks,
    ChunkIndex,
    EvaluatedCache,
    LevelRuntimeCache,
    LayerEvalCache,
    DerivedPositions,
    Scratch,
    PreviewStaging,
    UndoHistory,
}

impl MemoryCategory {
    pub const ALL: [MemoryCategory; 13] = [
        Self::BaseGeometry,
        Self::Topology,
        Self::MultiresDetail,
        Self::SculptLayers,
        Self::Masks,
        Self::ChunkIndex,
        Self::EvaluatedCache,
        Self::LevelRuntimeCache,
        Self::LayerEvalCache,
        Self::DerivedPositions,
        Self::Scratch,
        Self::PreviewStaging,
        Self::UndoHistory,
    ];

    /// Its index into a ledger's or a trim report's array.
    pub fn index(self) -> usize {
        use sys::clay_memory_category as c;
        (match self {
            Self::BaseGeometry => c::CLAY_MEMORY_BASE_GEOMETRY,
            Self::Topology => c::CLAY_MEMORY_TOPOLOGY,
            Self::MultiresDetail => c::CLAY_MEMORY_MULTIRES_DETAIL,
            Self::SculptLayers => c::CLAY_MEMORY_SCULPT_LAYERS,
            Self::Masks => c::CLAY_MEMORY_MASKS,
            Self::ChunkIndex => c::CLAY_MEMORY_CHUNK_INDEX,
            Self::EvaluatedCache => c::CLAY_MEMORY_EVALUATED_CACHE,
            Self::LevelRuntimeCache => c::CLAY_MEMORY_LEVEL_RUNTIME_CACHE,
            Self::LayerEvalCache => c::CLAY_MEMORY_LAYER_EVAL_CACHE,
            Self::DerivedPositions => c::CLAY_MEMORY_DERIVED_POSITIONS,
            Self::Scratch => c::CLAY_MEMORY_SCRATCH,
            Self::PreviewStaging => c::CLAY_MEMORY_PREVIEW_STAGING,
            Self::UndoHistory => c::CLAY_MEMORY_UNDO_HISTORY,
        }) as usize
    }

    /// The engine's own name for it, so a host's memory readout and the
    /// engine's diagnostics use one vocabulary.
    pub fn text(self) -> &'static str {
        // SAFETY: a total function over a plain integer, documented as never
        // returning NULL even for a value this build does not know.
        engine_text(unsafe { sys::clay_memory_category_text(self.index() as i32) })
    }
}

/// What a host is willing to spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoryClass {
    /// No budget. Every byte field is advisory and the runtime keeps what it
    /// builds — what a desktop host gets.
    #[default]
    Full,
    /// Budgets are real, inactive levels hold compact detail only, and
    /// maintenance runs between interactions.
    Constrained,
    /// What a host sets when the operating system has already warned it once.
    Minimal,
}

impl MemoryClass {
    fn to_raw(self) -> i32 {
        (match self {
            Self::Full => sys::clay_memory_class::CLAY_MEMORY_CLASS_FULL,
            Self::Constrained => sys::clay_memory_class::CLAY_MEMORY_CLASS_CONSTRAINED,
            Self::Minimal => sys::clay_memory_class::CLAY_MEMORY_CLASS_MINIMAL,
        }) as i32
    }

    fn from_raw(code: i32) -> Self {
        use sys::clay_memory_class as c;
        match code as sys::clay_memory_class::Type {
            c::CLAY_MEMORY_CLASS_CONSTRAINED => Self::Constrained,
            c::CLAY_MEMORY_CLASS_MINIMAL => Self::Minimal,
            // Full is zero, and it is also the honest reading of a value this
            // build does not know: no budget is what every caller that never
            // set one already gets.
            _ => Self::Full,
        }
    }

    pub fn text(self) -> &'static str {
        // SAFETY: a total function over a plain integer, never NULL.
        engine_text(unsafe { sys::clay_memory_class_text(self.to_raw()) })
    }
}

/// How hard a host is asking for memory back. Never inferred by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pressure {
    /// Give back what is free anyway.
    #[default]
    None,
    Warning,
    Urgent,
    /// The last stop before the operating system kills the process:
    /// everything rebuildable goes, and the next edit pays to rebuild it.
    Critical,
}

impl Pressure {
    pub const ALL: [Pressure; 4] = [Self::None, Self::Warning, Self::Urgent, Self::Critical];

    fn to_raw(self) -> i32 {
        (match self {
            Self::None => sys::clay_pressure::CLAY_PRESSURE_NONE,
            Self::Warning => sys::clay_pressure::CLAY_PRESSURE_WARNING,
            Self::Urgent => sys::clay_pressure::CLAY_PRESSURE_URGENT,
            Self::Critical => sys::clay_pressure::CLAY_PRESSURE_CRITICAL,
        }) as i32
    }

    fn from_raw(code: i32) -> Self {
        use sys::clay_pressure as p;
        match code as sys::clay_pressure::Type {
            p::CLAY_PRESSURE_WARNING => Self::Warning,
            p::CLAY_PRESSURE_URGENT => Self::Urgent,
            p::CLAY_PRESSURE_CRITICAL => Self::Critical,
            _ => Self::None,
        }
    }

    pub fn text(self) -> &'static str {
        // SAFETY: a total function over a plain integer, never NULL.
        engine_text(unsafe { sys::clay_pressure_text(self.to_raw()) })
    }
}

/// Bytes by category, plus the three roll-ups.
///
/// The shared vocabulary: a host holding a hierarchy, an adaptive surface and
/// a document gets one of these from each rather than three reports it has to
/// reconcile. It is *filled*, not merged — a caller accumulating several
/// surfaces adds the fields itself, because only it knows which surfaces
/// belong together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryLedger {
    /// Releasing any of this destroys work.
    pub essential: u64,
    /// Reconstructs to a bit-identical surface.
    pub rebuildable: u64,
    /// Undo depth: the host's policy, never the engine's.
    pub undoable: u64,
    pub total: u64,
    /// How many entries of the array below this library filled. Read this
    /// rather than assuming this build's own category count was reached.
    pub category_count: u32,
    bytes: [u64; sys::CLAY_MEMORY_CATEGORY_COUNT as usize],
}

impl MemoryLedger {
    fn from_raw(raw: sys::clay_memory_ledger) -> Self {
        Self {
            essential: raw.essential,
            rebuildable: raw.rebuildable,
            undoable: raw.undoable,
            total: raw.total,
            category_count: raw.category_count,
            bytes: raw.bytes,
        }
    }

    /// What one category costs, or `None` where the library filled fewer
    /// entries than this build knows about.
    pub fn bytes(&self, category: MemoryCategory) -> Option<u64> {
        let index = category.index();
        (index < self.category_count as usize).then(|| self.bytes[index])
    }
}

/// What a trim actually did, per category.
///
/// Reported per category rather than as one number, because a host that asked
/// for 40 MB and got it out of preview staging made a different decision from
/// one that got it out of the evaluated caches it is about to need again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrimReport {
    /// The pressure that was asked for, echoed.
    pub pressure: Pressure,
    pub total_released: u64,
    /// True when a pin was held: *nothing* was released and the figures are
    /// what the call would have released. A host that receives a memory
    /// warning while a save is running gets an honest answer instead of a
    /// document mutating under the writer.
    pub pinned: bool,
    pub category_count: u32,
    released: [u64; sys::CLAY_MEMORY_CATEGORY_COUNT as usize],
}

impl TrimReport {
    fn from_raw(raw: sys::clay_trim_report) -> Self {
        Self {
            pressure: Pressure::from_raw(raw.pressure),
            total_released: raw.total_released,
            pinned: raw.pinned != 0,
            category_count: raw.category_count,
            released: raw.released,
        }
    }

    /// What was released from one category, or `None` where the library filled
    /// fewer entries than this build knows about.
    pub fn released(&self, category: MemoryCategory) -> Option<u64> {
        let index = category.index();
        (index < self.category_count as usize).then(|| self.released[index])
    }
}

/// The budgets and deferrals a host declares, all of them hints.
///
/// Every field names something that can be recomputed *exactly* from what was
/// committed: normals during a drag, index quality, cache residency, the rate
/// a preview drains. There is deliberately no field for anything that *is* the
/// committed result, so "a memory-saving mode changed my sculpt" is
/// unrepresentable here rather than merely forbidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SculptMemoryProfile {
    pub class: MemoryClass,
    /// Chunk indices, runtime caches, derived positions. Zero is no budget.
    pub cache_budget: u64,
    /// The engine never trims this on its own.
    pub undo_budget: u64,
    /// The per-stamp working set, and a hard bound: a footprint larger than it
    /// is processed in blocks rather than allocated.
    pub scratch_budget: u64,
    pub preview_budget: u64,
    /// How many levels keep their rebuildable caches; zero means no limit.
    pub max_resident_levels: u32,
    /// Recompute exact normals at stroke end rather than per stamp. The final
    /// state is exact either way; this only decides when the work happens.
    pub defer_normals_in_stroke: bool,
    /// Whether a spatial index may be *rebuilt* — never whether it may be
    /// refitted, which is correctness.
    pub allow_index_rebuild: bool,
    /// How many dirty chunks a host expects to drain per frame; zero means as
    /// many as there are.
    pub preview_chunks_per_frame: u32,
}

impl SculptMemoryProfile {
    /// The library's own defaults, read from the engine rather than
    /// transcribed here — a transcribed default drifts.
    pub fn defaults() -> Result<Self> {
        let mut raw = sys::clay_sculpt_memory_profile::sized();
        // SAFETY: a valid versioned descriptor out-parameter whose
        // struct_size is set above.
        check(
            unsafe { sys::clay_sculpt_memory_profile_defaults(&mut raw) },
            "clay_sculpt_memory_profile_defaults",
        )?;
        Ok(Self::from_raw(raw))
    }

    fn from_raw(raw: sys::clay_sculpt_memory_profile) -> Self {
        Self {
            class: MemoryClass::from_raw(raw.memory_class),
            cache_budget: raw.cache_budget,
            undo_budget: raw.undo_budget,
            scratch_budget: raw.scratch_budget,
            preview_budget: raw.preview_budget,
            max_resident_levels: raw.max_resident_levels,
            defer_normals_in_stroke: raw.defer_normals_in_stroke != 0,
            allow_index_rebuild: raw.allow_index_rebuild != 0,
            preview_chunks_per_frame: raw.preview_chunks_per_frame,
        }
    }

    fn to_raw(self) -> sys::clay_sculpt_memory_profile {
        let mut raw = sys::clay_sculpt_memory_profile::sized();
        raw.memory_class = self.class.to_raw();
        raw.cache_budget = self.cache_budget;
        raw.undo_budget = self.undo_budget;
        raw.scratch_budget = self.scratch_budget;
        raw.preview_budget = self.preview_budget;
        raw.max_resident_levels = self.max_resident_levels;
        raw.defer_normals_in_stroke = i32::from(self.defer_normals_in_stroke);
        raw.allow_index_rebuild = i32::from(self.allow_index_rebuild);
        raw.preview_chunks_per_frame = self.preview_chunks_per_frame;
        raw
    }
}

/// What a serializer or a readback holds so a trim arriving mid-save is honest
/// rather than destructive.
///
/// Reentrant, because a readback inside a save must not un-pin the save when
/// it returns: [`acquire`](Self::acquire) and [`release`](Self::release) are a
/// counter, and the pin is held while the count is non-zero.
pub struct MemoryPin {
    raw: NonNull<sys::clay_memory_pin>,
}

impl MemoryPin {
    pub fn new() -> Result<Self> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: a valid out-parameter, written only on success.
        check(
            unsafe { sys::clay_memory_pin_create(&mut raw) },
            "clay_memory_pin_create",
        )?;
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_memory_pin_create", ErrorKind::Backend))
    }

    /// Takes the pin. Balanced by exactly one [`release`](Self::release).
    pub fn acquire(&mut self) -> Result<()> {
        // SAFETY: owned handle, valid for the life of `self`.
        check(
            unsafe { sys::clay_memory_pin_acquire(self.raw.as_ptr()) },
            "clay_memory_pin_acquire",
        )
    }

    /// Gives it back. Releasing a pin nobody acquired does nothing: an
    /// unbalanced release is a caller's bug, and leaving the count at zero is
    /// the harmless reading of it — underflowing to "pinned forever" is not.
    pub fn release(&mut self) -> Result<()> {
        // SAFETY: owned handle, valid for the life of `self`.
        check(
            unsafe { sys::clay_memory_pin_release(self.raw.as_ptr()) },
            "clay_memory_pin_release",
        )
    }

    pub fn is_held(&self) -> bool {
        // SAFETY: owned handle; the call only reads the counter.
        unsafe { sys::clay_memory_pin_held(self.raw.as_ptr()) != 0 }
    }

    fn as_ptr(&self) -> *const sys::clay_memory_pin {
        self.raw.as_ptr()
    }
}

impl Drop for MemoryPin {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once.
        unsafe { sys::clay_memory_pin_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for MemoryPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryPin")
            .field("held", &self.is_held())
            .finish()
    }
}

// -- the changed-block transport --------------------------------------------

/// What one block holds, at one level.
///
/// A block is addressed by *base patch*: a level-0 face owns a subtree that
/// never moves between faces, so a block's identity is stable for the life of
/// the hierarchy and no re-partition can invalidate what a host has already
/// uploaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockInfo {
    pub patch: u32,
    pub level: u32,
    pub vertex_count: u32,
    /// Triangle indices, local to the block.
    pub index_count: u32,
}

impl BlockInfo {
    fn from_raw(raw: sys::clay_multires_block_info) -> Self {
        Self {
            patch: raw.patch,
            level: raw.level,
            vertex_count: raw.vertex_count,
            index_count: raw.index_count,
        }
    }
}

/// One block, copied out of the hierarchy.
///
/// Copied rather than borrowed, deliberately: a mutation can move or free
/// anything the surface holds, and a pointer held across one would be a
/// use-after-free with no generation to check.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub info: BlockInfo,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Local to the block, so a host uploads it as a standalone draw.
    pub indices: Vec<u32>,
}

// -- the hierarchy ----------------------------------------------------------

/// A base cage and a deterministic Catmull-Clark hierarchy over it.
///
/// Owns its engine handle and destroys it on drop.
pub struct Multires {
    raw: NonNull<sys::clay_multires>,
}

impl Multires {
    /// Builds a hierarchy of one level — the cage itself.
    ///
    /// Adding levels is a separate and priced operation. This refuses rather
    /// than repairs: a conversion that quietly welds a face changes the
    /// retopology somebody paid for without saying so, so a non-manifold or
    /// degenerate cage comes back as a [`MultiresRefusal`] naming which.
    pub fn from_mesh(
        mesh: &Mesh,
        desc: MultiresDesc,
    ) -> std::result::Result<Self, MultiresRefusal> {
        let raw_desc = desc.to_raw();
        let mut surface = std::ptr::null_mut();
        let mut reason = 0i32;
        // SAFETY: a valid mesh handle read but not retained — the hierarchy
        // copies the cage — a descriptor carrying its own struct_size, and two
        // out-parameters. `reason` is written on every path, including
        // success, where the engine sets it to CLAY_MULTIRES_OK.
        let code = unsafe {
            sys::clay_multires_from_mesh(mesh.as_ptr(), &raw_desc, &mut surface, &mut reason)
        };
        let reason = MultiresError::from_raw(reason);
        check(code, "clay_multires_from_mesh")
            .map_err(|error| MultiresRefusal { error, reason })?;
        NonNull::new(surface)
            .map(|raw| Self { raw })
            .ok_or_else(|| MultiresRefusal {
                error: raw_failure("clay_multires_from_mesh", ErrorKind::Backend),
                reason,
            })
    }

    /// Reconstructs a hierarchy from [`serialize`](Self::serialize)'s bytes.
    ///
    /// Refuses a truncated, hostile or newer buffer — including one declaring
    /// a depth whose reconstruction over its own cage would exceed this
    /// build's ceiling, which is refused before anything is allocated.
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        let mut surface = std::ptr::null_mut();
        // SAFETY: `bytes` is valid for reads of `bytes.len()`, which is the
        // length passed; the buffer is read and not retained, and the
        // out-parameter is written only on success.
        check(
            unsafe { sys::clay_multires_deserialize(bytes.as_ptr(), bytes.len(), &mut surface) },
            "clay_multires_deserialize",
        )?;
        NonNull::new(surface)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_multires_deserialize", ErrorKind::Backend))
    }

    /// How many levels the hierarchy holds. One is a cage and nothing else.
    pub fn level_count(&self) -> u32 {
        // SAFETY: valid handle; the call only reads, and answers zero for a
        // handle it cannot resolve rather than failing.
        unsafe { sys::clay_multires_level_count(self.raw.as_ptr()) }
    }

    /// Where the brush writes.
    pub fn sculpt_level(&self) -> Result<u32> {
        let mut level = 0;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_multires_sculpt_level(self.raw.as_ptr(), &mut level) },
            "clay_multires_sculpt_level",
        )?;
        Ok(level)
    }

    /// What the host draws.
    ///
    /// Independent of the sculpt level, and that independence is the workflow
    /// the tier exists for: move the broad form at level 1 and watch the pores
    /// at level 4 move with it.
    pub fn display_level(&self) -> Result<u32> {
        let mut level = 0;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_multires_display_level(self.raw.as_ptr(), &mut level) },
            "clay_multires_display_level",
        )?;
        Ok(level)
    }

    pub fn set_sculpt_level(&mut self, level: u32) -> Result<()> {
        // SAFETY: valid handle; the level is range-checked by the entry point,
        // which refuses rather than clamping.
        check(
            unsafe { sys::clay_multires_set_sculpt_level(self.raw.as_ptr(), level) },
            "clay_multires_set_sculpt_level",
        )
    }

    pub fn set_display_level(&mut self, level: u32) -> Result<()> {
        // SAFETY: as above.
        check(
            unsafe { sys::clay_multires_set_display_level(self.raw.as_ptr(), level) },
            "clay_multires_set_display_level",
        )
    }

    /// How many vertices and faces one level holds.
    pub fn level_counts(&self, level: u32) -> Result<(u64, u64)> {
        let (mut vertices, mut faces) = (0, 0);
        // SAFETY: valid handle, a range-checked level, and two out-parameters
        // written on success.
        check(
            unsafe {
                sys::clay_multires_level_counts(self.raw.as_ptr(), level, &mut vertices, &mut faces)
            },
            "clay_multires_level_counts",
        )?;
        Ok((vertices, faces))
    }

    /// What adding a level would cost. Allocates nothing and has no side
    /// effects: it is arithmetic on the level below.
    pub fn preflight_add_level(&self) -> Result<AddLevelPreflight> {
        let mut raw = sys::clay_multires_preflight::sized();
        // SAFETY: valid handle and a versioned out-descriptor whose
        // struct_size is written from the compiled type.
        check(
            unsafe { sys::clay_multires_preflight_add_level(self.raw.as_ptr(), &mut raw) },
            "clay_multires_preflight_add_level",
        )?;
        Ok(AddLevelPreflight::from_raw(raw))
    }

    /// Adds one level.
    ///
    /// Build-then-publish: a refusal leaves the surface exactly as it was
    /// rather than one level into a state nothing knows how to read. Sets the
    /// sculpt and display levels to the new one, which is what an artist means
    /// by "subdivide".
    pub fn add_level(&mut self) -> std::result::Result<u32, MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle, a null cancel token (which the entry point
        // documents as "cannot be cancelled" rather than as an error), and an
        // out-parameter written on every path.
        let code = unsafe {
            sys::clay_multires_add_level(self.raw.as_ptr(), std::ptr::null_mut(), &mut reason)
        };
        let reason = MultiresError::from_raw(reason);
        check(code, "clay_multires_add_level")
            .map_err(|error| MultiresRefusal { error, reason })?;
        Ok(self.level_count() - 1)
    }

    /// Drops the highest level and the detail on it.
    ///
    /// Destructive; a host that wants it reversible records its own copy
    /// first, because nothing in the ABI takes this back.
    pub fn remove_highest_level(&mut self) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle and an out-parameter written on every path.
        let code =
            unsafe { sys::clay_multires_remove_highest_level(self.raw.as_ptr(), &mut reason) };
        let reason = MultiresError::from_raw(reason);
        check(code, "clay_multires_remove_highest_level")
            .map_err(|error| MultiresRefusal { error, reason })
    }

    /// One level as an ordinary mesh — the evaluated positions, out.
    ///
    /// The cage's attributes are subdivided over their own connectivity, so a
    /// UV seam is interpolated along itself and never across itself.
    pub fn copy_level_mesh(&mut self, level: u32) -> Result<Mesh> {
        let mut mesh = std::ptr::null_mut();
        // SAFETY: valid handle, a range-checked level, and an out-parameter
        // the engine fills with a mesh whose ownership passes to `Mesh` below,
        // which destroys it exactly once on drop.
        check(
            unsafe { sys::clay_multires_copy_level_mesh(self.raw.as_ptr(), level, &mut mesh) },
            "clay_multires_copy_level_mesh",
        )?;
        Mesh::from_raw(mesh, "clay_multires_copy_level_mesh")
    }

    /// The three counters. Compare, do not add.
    pub fn revision(&self) -> Result<Revisions> {
        let mut out = Revisions::default();
        // SAFETY: valid handle and three out-parameters, each of which the
        // entry point allows to be null and none of which is.
        check(
            unsafe {
                sys::clay_multires_revision(
                    self.raw.as_ptr(),
                    &mut out.base,
                    &mut out.detail,
                    &mut out.evaluated,
                )
            },
            "clay_multires_revision",
        )?;
        Ok(out)
    }

    /// A hash of every level's authoritative detail.
    ///
    /// What a host proves to itself with rather than taking on trust:
    /// [`drop_inactive_caches`](Self::drop_inactive_caches) and
    /// [`trim`](Self::trim) both leave this unchanged, and a value that moved
    /// across either means work was released rather than caches.
    pub fn detail_checksum(&self) -> Result<u64> {
        let mut checksum = 0;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_multires_detail_checksum(self.raw.as_ptr(), &mut checksum) },
            "clay_multires_detail_checksum",
        )?;
        Ok(checksum)
    }

    /// What the hierarchy costs, split by what may be released.
    pub fn memory(&self) -> Result<MultiresMemory> {
        let mut raw = sys::clay_multires_memory::sized();
        // SAFETY: valid handle and a versioned out-descriptor whose
        // struct_size is written from the compiled type — which is what tells
        // the engine it may fill the fields appended in 0.76.0 and 0.78.0.
        check(
            unsafe { sys::clay_multires_memory_get(self.raw.as_ptr(), &mut raw) },
            "clay_multires_memory_get",
        )?;
        Ok(MultiresMemory::from_raw(raw))
    }

    /// The same cost in the shared vocabulary, for a host adding up several
    /// surfaces.
    pub fn memory_ledger(&self) -> Result<MemoryLedger> {
        let mut raw = sys::clay_memory_ledger::sized();
        // SAFETY: valid handle and a versioned out-descriptor whose
        // struct_size is written from the compiled type, so the engine knows
        // how many category entries it may fill.
        check(
            unsafe { sys::clay_multires_memory_ledger(self.raw.as_ptr(), &mut raw) },
            "clay_multires_memory_ledger",
        )?;
        Ok(MemoryLedger::from_raw(raw))
    }

    /// What this surface is currently allowed to spend.
    pub fn memory_profile(&self) -> Result<SculptMemoryProfile> {
        let mut raw = sys::clay_sculpt_memory_profile::sized();
        // SAFETY: valid handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_multires_memory_profile(self.raw.as_ptr(), &mut raw) },
            "clay_multires_memory_profile",
        )?;
        Ok(SculptMemoryProfile::from_raw(raw))
    }

    /// Declares what it may spend.
    ///
    /// Set on the hierarchy alone, and that is a statement rather than an
    /// omission: this is the representation that *holds levels*, so residency
    /// is the only place a budget currently changes what the engine keeps.
    pub fn set_memory_profile(&mut self, profile: SculptMemoryProfile) -> Result<()> {
        let raw = profile.to_raw();
        // SAFETY: valid handle and a descriptor carrying its own struct_size,
        // read and not retained.
        check(
            unsafe { sys::clay_multires_set_memory_profile(self.raw.as_ptr(), &raw) },
            "clay_multires_set_memory_profile",
        )
    }

    /// Releases the rebuildable caches of the levels nothing is using.
    ///
    /// Rebuilding them reproduces the surface bit-identically, which
    /// [`detail_checksum`](Self::detail_checksum) is how a host checks.
    pub fn drop_inactive_caches(&mut self) -> Result<()> {
        // SAFETY: valid handle; releases caches the surface owns.
        check(
            unsafe { sys::clay_multires_drop_inactive_caches(self.raw.as_ptr()) },
            "clay_multires_drop_inactive_caches",
        )
    }

    /// Releases rebuildable caches at a stated pressure, in a fixed order.
    ///
    /// Never authoritative content and never history: not the cage, not a
    /// level's topology, not the detail, not a sculpt layer, not a mask. A
    /// held `pin` makes this a no-op that reports what it *would* have
    /// released, with [`TrimReport::pinned`] set.
    pub fn trim(&mut self, pressure: Pressure, pin: Option<&MemoryPin>) -> Result<TrimReport> {
        let mut raw = sys::clay_trim_report::sized();
        // SAFETY: valid handle, a pressure the entry point range-checks, a pin
        // that is either a valid handle borrowed for the duration of the call
        // or null (both of which it allows), and a versioned out-descriptor.
        check(
            unsafe {
                sys::clay_multires_trim(
                    self.raw.as_ptr(),
                    pressure.to_raw(),
                    pin.map_or(std::ptr::null(), MemoryPin::as_ptr),
                    &mut raw,
                )
            },
            "clay_multires_trim",
        )?;
        Ok(TrimReport::from_raw(raw))
    }

    /// Reclaims the storage a sculpt-layer pass that undid itself left behind.
    ///
    /// The cheapest of the four levers a host under memory pressure has, the
    /// other three being merge, bake and delete. Never inside a pointer event:
    /// it walks the stored blocks of every layer, which is proportional to the
    /// stack rather than to the dab.
    pub fn compact_sculpt_layers(&mut self) -> Result<()> {
        // SAFETY: valid handle; the call rewrites storage the surface owns.
        check(
            unsafe { sys::clay_multires_compact_sculpt_layers(self.raw.as_ptr()) },
            "clay_multires_compact_sculpt_layers",
        )
    }

    /// The bytes to store beside the document.
    ///
    /// The cage, the rule, the level count, the active levels and each level's
    /// detail. The face lists and every evaluated position follow from those
    /// and are not written — writing them would create a second answer that a
    /// corrupt file could make disagree with the first.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let handle = self.raw.as_ptr();
        size_query_bytes("clay_multires_serialize", |buf, size| {
            // SAFETY: the buffer protocol's two calls. `buf` is either null —
            // asking for the size — or valid for writes of `*size` bytes, which
            // is what the engine is told it has and what it checks before it
            // copies. `u8` and `c_char` have the same size and alignment.
            unsafe { sys::clay_multires_serialize(handle, buf as *mut u8, size) }
        })
    }

    /// What [`serialize`](Self::serialize) would cost.
    ///
    /// The blob is a second copy of everything and it exists while the surface
    /// still does, which is why the peak is worth asking for before a save on
    /// a device that kills an application rather than warning it. `budget` of
    /// zero means no budget.
    pub fn preflight_encode(&self, budget: u64) -> Result<EncodePreflight> {
        let mut raw = sys::clay_surface_preflight::sized();
        // SAFETY: valid handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_multires_preflight_encode(self.raw.as_ptr(), budget, &mut raw) },
            "clay_multires_preflight_encode",
        )?;
        Ok(EncodePreflight::from_raw(raw))
    }

    /// How many base patches the stamps since the last clear touched.
    pub fn dirty_block_count(&self) -> usize {
        // SAFETY: valid handle; the call only reads, and answers zero for a
        // handle it cannot resolve.
        unsafe { sys::clay_multires_dirty_block_count(self.raw.as_ptr()) }
    }

    /// Which ones.
    ///
    /// The entry point truncates to the capacity it is given rather than
    /// refusing, so the count is asked for first and the buffer is sized to
    /// the answer — a short read here would silently drop a block a host had
    /// to re-upload.
    pub fn dirty_blocks(&self) -> Result<Vec<u32>> {
        let mut count = 0usize;
        // SAFETY: valid handle; a null buffer is the documented count query
        // and `count` is written with how many there are.
        check(
            unsafe {
                sys::clay_multires_dirty_blocks(self.raw.as_ptr(), std::ptr::null_mut(), &mut count)
            },
            "clay_multires_dirty_blocks",
        )?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut patches = vec![0u32; count];
        let mut capacity = count;
        // SAFETY: `patches` is valid for writes of `capacity` u32, which is
        // exactly what the engine is told it has; it writes at most that many
        // and reports how many in `capacity`.
        check(
            unsafe {
                sys::clay_multires_dirty_blocks(
                    self.raw.as_ptr(),
                    patches.as_mut_ptr(),
                    &mut capacity,
                )
            },
            "clay_multires_dirty_blocks",
        )?;
        patches.truncate(capacity.min(count));
        Ok(patches)
    }

    /// Forgets which blocks changed. A host calls this once it has uploaded
    /// what it was told about.
    pub fn clear_dirty(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_multires_clear_dirty(self.raw.as_ptr()) },
            "clay_multires_clear_dirty",
        )
    }

    /// What one block would cost to copy, at a level.
    ///
    /// Takes `&mut self` because the engine builds the block into the
    /// handle's own scratch: the query and the copy share it so the two cannot
    /// disagree about what a block contains.
    pub fn block_info(&mut self, patch: u32, level: u32) -> Result<BlockInfo> {
        let mut raw = sys::clay_multires_block_info::sized();
        // SAFETY: valid handle, a patch and level both range-checked by the
        // entry point, and a versioned out-descriptor.
        check(
            unsafe { sys::clay_multires_block_info_get(self.raw.as_ptr(), patch, level, &mut raw) },
            "clay_multires_block_info_get",
        )?;
        Ok(BlockInfo::from_raw(raw))
    }

    /// Copies one block's geometry out.
    ///
    /// Sized from [`block_info`](Self::block_info) first, because every
    /// capacity is checked before anything is written: a partial fill would
    /// leave a host drawing a block that is half this frame's and half the
    /// last one's, so the engine refuses instead.
    pub fn copy_block(&mut self, patch: u32, level: u32) -> Result<Block> {
        let info = self.block_info(patch, level)?;
        let vertices = info.vertex_count as usize;
        let mut positions = vec![[0.0f32; 3]; vertices];
        let mut normals = vec![[0.0f32; 3]; vertices];
        let mut indices = vec![0u32; info.index_count as usize];
        let mut written = sys::clay_multires_block_info::sized();
        // SAFETY: the three buffers are valid for writes of exactly the
        // capacities passed — `3 * vertices` floats each for the two `[f32; 3]`
        // vectors, whose layout is three consecutive floats, and `index_count`
        // u32 — and the engine checks every one of them against the block
        // before it writes anything. `written` is a versioned out-descriptor.
        check(
            unsafe {
                sys::clay_multires_copy_block(
                    self.raw.as_ptr(),
                    patch,
                    level,
                    positions.as_mut_ptr() as *mut f32,
                    vertices * 3,
                    normals.as_mut_ptr() as *mut f32,
                    vertices * 3,
                    indices.as_mut_ptr(),
                    indices.len(),
                    &mut written,
                )
            },
            "clay_multires_copy_block",
        )?;
        Ok(Block {
            info: BlockInfo::from_raw(written),
            positions,
            normals,
            indices,
        })
    }

    /// A sculptor bound to this hierarchy.
    ///
    /// The hierarchy is borrowed for as long as the sculptor lives, which is
    /// the ABI's own rule expressed in the type system: the sculptor keeps a
    /// bare pointer to the surface and using it after the surface is gone
    /// would be a use-after-free.
    pub fn sculptor(&mut self) -> Result<MultiresSculptor<'_>> {
        let mut sculptor = std::ptr::null_mut();
        // SAFETY: valid handle and an out-parameter written only on success.
        check(
            unsafe { sys::clay_multires_sculptor_create(self.raw.as_ptr(), &mut sculptor) },
            "clay_multires_sculptor_create",
        )?;
        let raw = NonNull::new(sculptor)
            .ok_or_else(|| raw_failure("clay_multires_sculptor_create", ErrorKind::Backend))?;
        Ok(MultiresSculptor { raw, surface: self })
    }
}

impl Drop for Multires {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once. Every borrow of it —
        // a sculptor above all — carries a lifetime that ends before this
        // does, so nothing else holds a pointer into it here.
        unsafe { sys::clay_multires_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for Multires {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Multires")
            .field("levels", &self.level_count())
            .field("sculpt_level", &self.sculpt_level().ok())
            .field("display_level", &self.display_level().ok())
            .finish()
    }
}

// -- the level sculptor -----------------------------------------------------

/// What a stamp or a stroke did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StampReport {
    /// The level it was made on.
    pub level: u32,
    pub moved_vertices: u64,
    /// The three counters as they stood afterwards.
    pub revisions: Revisions,
}

impl StampReport {
    fn from_raw(raw: sys::clay_multires_stamp_report) -> Self {
        Self {
            level: raw.level,
            moved_vertices: raw.moved_vertices,
            revisions: Revisions {
                base: raw.base_revision,
                detail: raw.detail_revision,
                evaluated: raw.evaluated_revision,
            },
        }
    }
}

/// The four high-water marks a host tunes a [`SculptMemoryProfile`] against.
///
/// High-water marks and not averages, and that is the point rather than a
/// detail: a buffer sized to the vertex count, allocated once during warm-up
/// and reused forever, allocates nothing on a warm stamp and still costs
/// O(model) storage. Only a peak catches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeakTelemetry {
    pub scratch_bytes: u64,
    pub workset_vertices: u64,
    pub dirty_chunks: u64,
    pub topology_ops: u64,
}

/// What the per-stamp scratch arena owns.
///
/// A high-water mark that has stopped rising over stamps of similar footprint
/// is the arena having converged; one that rises every stamp is scratch that
/// is never released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ArenaStats {
    /// What the arena currently owns.
    pub capacity_bytes: u64,
    /// The largest a single stamp has ever used.
    pub high_water_bytes: u64,
    /// How many times it has had to take more.
    pub growths: u64,
}

/// A brush over the hierarchy's currently bound sculpt level.
///
/// The same verbs, falloffs, masks and alphas as a mesh layer, because it is
/// the same code: a stamp runs the fixed sculptor over the active level's own
/// mesh, and what this owns is the step that turns the moved positions back
/// into what the hierarchy stores.
///
/// Borrows its hierarchy exclusively and lends it back through
/// [`surface`](Self::surface) and [`surface_mut`](Self::surface_mut), so
/// changing the sculpt level mid-session — which is what a rebind is — is
/// reachable without dropping the sculptor and losing its stroke record.
pub struct MultiresSculptor<'s> {
    raw: NonNull<sys::clay_multires_sculptor>,
    surface: &'s mut Multires,
}

impl<'s> MultiresSculptor<'s> {
    /// The hierarchy this sculptor writes into.
    pub fn surface(&self) -> &Multires {
        self.surface
    }

    /// The same, to change the level or read a block out between stamps.
    ///
    /// A level change renumbers the classes the seed is picked in, so a
    /// [`seed_revision`](Self::seed_revision) taken before one is stale
    /// afterwards — and a stale seed makes the walk find nothing, which looks
    /// exactly like a fully masked stroke.
    pub fn surface_mut(&mut self) -> &mut Multires {
        self.surface
    }

    /// Starts a gesture.
    ///
    /// Clears the record the Layer verb measures its ceiling against, so a
    /// second stroke over the same place deposits from the surface as *that*
    /// stroke found it.
    pub fn begin_stroke(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_multires_sculptor_begin_stroke(self.raw.as_ptr()) },
            "clay_multires_sculptor_begin_stroke",
        )
    }

    /// The token the currently bound level's classes are numbered in.
    ///
    /// Takes `&mut self` because the call *binds*: the answer is a property of
    /// the bound level, and a host asking before its first stamp would
    /// otherwise be handed the token of whatever was bound last. Store it
    /// beside any seed class picked off that level's mesh — a host that keeps
    /// the class and forgets the token has kept exactly the half that cannot
    /// be checked. Zero for a surface that cannot bind, which is the value
    /// that claims nothing.
    pub fn seed_revision(&mut self) -> Result<u64> {
        let mut revision = 0;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_multires_sculptor_seed_revision(self.raw.as_ptr(), &mut revision) },
            "clay_multires_sculptor_seed_revision",
        )?;
        Ok(revision)
    }

    /// One stamp at the surface's current sculpt level.
    pub fn stamp(&mut self, stamp: MeshStamp<'_>, mask: Option<&MaskField>) -> Result<StampReport> {
        let desc = stamp.as_raw();
        let mut report = sys::clay_multires_stamp_report::sized();
        // SAFETY: valid handle; `desc` carries its own struct_size and borrows
        // any alpha samples from `stamp`, which outlives this call; the mask is
        // either a valid handle or null, both of which the entry point allows;
        // the report is a versioned out-descriptor.
        check(
            unsafe {
                sys::clay_multires_sculptor_stamp(
                    self.raw.as_ptr(),
                    &desc,
                    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _),
                    &mut report,
                )
            },
            "clay_multires_sculptor_stamp",
        )?;
        Ok(StampReport::from_raw(report))
    }

    /// A whole stroke at the active sculpt level, resolved into spaced stamps
    /// by the same engine that drives a mesh layer.
    ///
    /// `samples` is position, pressure and tilt per sample, in the surface's
    /// own space. `defer_normals` recomputes normals once at the end instead
    /// of per stamp — faster, identical result, and it flushes at the end of
    /// this call because here the library does know where the stroke ended.
    ///
    /// The stroke record does not cross this ABI: a host wanting one gesture
    /// as one undo step cannot get it from here, which is stated rather than
    /// left to be discovered.
    pub fn apply_stroke(
        &mut self,
        samples: &[[f32; 5]],
        preset: &crate::StrokePreset,
        stamp: MeshStamp<'_>,
        mask: Option<&MaskField>,
        defer_normals: bool,
    ) -> Result<(usize, StampReport)> {
        if samples.is_empty() {
            return Ok((0, StampReport::default()));
        }
        let desc = stamp.as_raw();
        let raw_preset = preset.to_raw();
        let mut applied = 0;
        let mut report = sys::clay_multires_stamp_report::sized();
        // SAFETY: `samples` is `samples.len() * 5` contiguous floats, which is
        // the layout the entry point reads and the count it is given; both
        // descriptors carry their own struct_size; the mask is nullable and the
        // world transform is null, meaning the surface's own space.
        check(
            unsafe {
                sys::clay_multires_sculptor_apply_stroke(
                    self.raw.as_ptr(),
                    samples.as_ptr() as *const f32,
                    samples.len(),
                    &raw_preset,
                    &desc,
                    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _),
                    std::ptr::null(),
                    i32::from(defer_normals),
                    &mut applied,
                    &mut report,
                )
            },
            "clay_multires_sculptor_apply_stroke",
        )?;
        Ok((applied, StampReport::from_raw(report)))
    }

    /// Stops recomputing normals per stamp.
    ///
    /// Forwarded to whichever level is bound, including one bound later: a
    /// rebind builds a new level sculptor, and a deferral that stopped
    /// applying the moment the host changed level would leave a drag half
    /// deferred and half not. **A host that defers must flush** — nothing
    /// flushes on its own, because the sculptor does not know where a stroke
    /// ends and guessing would flush mid-drag, which is the cost this exists
    /// to avoid.
    pub fn set_defer_normals(&mut self, defer: bool) -> Result<()> {
        // SAFETY: valid handle and a plain integer.
        check(
            unsafe {
                sys::clay_multires_sculptor_set_defer_normals(self.raw.as_ptr(), i32::from(defer))
            },
            "clay_multires_sculptor_set_defer_normals",
        )
    }

    pub fn defer_normals(&self) -> Result<bool> {
        let mut defer = 0;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_multires_sculptor_defer_normals(self.raw.as_ptr(), &mut defer) },
            "clay_multires_sculptor_defer_normals",
        )?;
        Ok(defer != 0)
    }

    /// Recomputes what was deferred. Binds nothing: there is nothing to flush
    /// on a level nobody has stamped.
    pub fn flush_normals(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_multires_sculptor_flush_normals(self.raw.as_ptr()) },
            "clay_multires_sculptor_flush_normals",
        )
    }

    /// The session's peaks. The session's, not the bound level's: a rebind
    /// builds a new level sculptor and this one keeps filling.
    pub fn peak_telemetry(&self) -> Result<PeakTelemetry> {
        let mut raw = sys::clay_peak_telemetry::sized();
        // SAFETY: valid handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_multires_sculptor_peak_telemetry(self.raw.as_ptr(), &mut raw) },
            "clay_multires_sculptor_peak_telemetry",
        )?;
        Ok(PeakTelemetry {
            scratch_bytes: raw.scratch_bytes,
            workset_vertices: raw.workset_vertices,
            dirty_chunks: raw.dirty_chunks,
            topology_ops: raw.topology_ops,
        })
    }

    pub fn reset_peak_telemetry(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_multires_sculptor_reset_peak_telemetry(self.raw.as_ptr()) },
            "clay_multires_sculptor_reset_peak_telemetry",
        )
    }

    /// The bound level sculptor's scratch arena.
    ///
    /// Before the first stamp there is no bound level and every field reads
    /// zero, which is the truth rather than a placeholder.
    pub fn arena_stats(&self) -> Result<ArenaStats> {
        let mut raw = sys::clay_brush_arena_stats::sized();
        // SAFETY: valid handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_multires_sculptor_arena_stats(self.raw.as_ptr(), &mut raw) },
            "clay_multires_sculptor_arena_stats",
        )?;
        Ok(ArenaStats {
            capacity_bytes: raw.capacity_bytes,
            high_water_bytes: raw.high_water_bytes,
            growths: raw.growths,
        })
    }
}

impl Drop for MultiresSculptor<'_> {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once, and before the borrow
        // of the surface it points at ends — so the surface it holds a bare
        // pointer to is still alive here.
        unsafe { sys::clay_multires_sculptor_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for MultiresSculptor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiresSculptor")
            .field("surface", &self.surface)
            .field("defer_normals", &self.defer_normals().ok())
            .finish()
    }
}
