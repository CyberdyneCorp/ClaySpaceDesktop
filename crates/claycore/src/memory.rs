//! What a document and the surfaces beside it cost, and what a host may take
//! back.
//!
//! Three questions, and only the first of them had an answer before this
//! release. *What does this cost* is [`MemoryReport`]. *What am I allowed to
//! spend* is [`SculptMemoryProfile`], filled by the host, with no device
//! detection anywhere. *What can I release right now* is a trim, priced by
//! [`MemoryLedger`] and reported by [`TrimReport`].
//!
//! # The breakdown is the feature and a total is not
//!
//! A memory warning arrives with no argument. Under pressure a host does not
//! need to know how big the document is, it needs to know *which part*,
//! because that is what decides what it is allowed to release:
//!
//! | | what letting it go costs |
//! |---|---|
//! | [`essential`](MemoryReport::essential) | the user's work. Never. |
//! | [`rebuildable`](MemoryReport::rebuildable) | a stall, and nothing else — it reconstructs bit-identically |
//! | [`undoable`](MemoryReport::undoable) | undo depth, which is the host's own policy |
//!
//! Those three are **derived by the engine** from the category lines rather
//! than counted beside them, so a line added upstream without being classified
//! cannot make them disagree with the total. They are read here and never
//! recomputed: a second derivation in Rust would be a second thing that can be
//! right about a build it was written against and wrong about the next one.
//!
//! # The ledger is the host's, and the API says so
//!
//! A [`crate::Multires`] and a [`crate::MeshSculptor`] are opaque and *owning*:
//! a host holds one beside its document, never inside it, so
//! [`Document::memory`] reports the surface tier as zero and that is ownership
//! rather than an omission. To get a whole answer the host asks each surface
//! for its ledger, [merges](MemoryLedger::merge) the ones that belong to this
//! document — only the host knows which do — and hands the result to
//! [`Document::memory_with_surfaces`]. Every step of that is visible in the
//! signatures, deliberately: hiding it behind a call that walked "the
//! document's surfaces" would be inventing an ownership the engine refuses to
//! claim.
//!
//! # A floor, not an equality
//!
//! These are container walks. Allocator block headers, size-class rounding and
//! arena fragmentation are invisible from here, as are the library's own code
//! and static data. Expect the operating system to charge the process *more*
//! than this, and do not read the gap as a leak.
//!
//! # What is deliberately not here
//!
//! `clay_dynamic_sculptor_memory_ledger`, `clay_dynamic_sculptor_trim` and the
//! two `clay_dynamic_surface_preflight_*` entry points take a handle this
//! crate does not wrap: there is no adaptive surface in this workspace yet, so
//! a wrapper for them would be an unconstructible type, or a raw pointer
//! crossing a safe boundary. They arrive with the adaptive surface, which is
//! the change that can also run them.

use std::ptr::NonNull;

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::document::{Document, LayerId};
use crate::error::{check, ErrorKind, Result};
use crate::mesh::Mesh;
use crate::mesh_sculpt::MeshSculptor;
use crate::{engine_text, raw_failure};

// -- refusing an operation that will not fit --------------------------------

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
/// What an operation would cost, asked before any of it is paid.
///
/// One estimator underneath every preflight that fills this, and the reason is
/// arithmetic rather than tidiness: a bespoke estimate per operation is a
/// place for `vertices * bytes` to wrap 64 bits and report a *small* number,
/// and the failure mode of that bug is that the operation is **allowed**.
/// Every multiply is checked and an overflow is [`BudgetError::Overflow`] —
/// a refusal rather than a wrapped estimate.
///
/// [`peak_bytes`](Self::peak_bytes) is reported apart from
/// [`persistent_bytes`](Self::persistent_bytes) because on a device that kills
/// an application rather than warning it, the transient high-water mark is
/// what does the killing, and an engine that discovers this by being
/// terminated cannot say what happened.
///
/// These are *ceilings*, deliberately. The structural figures are exact byte
/// costs, but the arrays that carry them are grown rather than reserved, so a
/// figure measured afterwards includes capacity slack the prediction does not.
/// A budget that errs low is the one that gets an application killed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfacePreflight {
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

impl SurfacePreflight {
    pub(crate) fn from_raw(raw: sys::clay_surface_preflight) -> Self {
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

    pub(crate) fn to_raw(self) -> i32 {
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
    pub(crate) fn from_raw(raw: sys::clay_memory_ledger) -> Self {
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
    pub(crate) fn from_raw(raw: sys::clay_trim_report) -> Self {
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

    pub(crate) fn from_raw(raw: sys::clay_sculpt_memory_profile) -> Self {
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

    pub(crate) fn to_raw(self) -> sys::clay_sculpt_memory_profile {
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

    pub(crate) fn as_ptr(&self) -> *const sys::clay_memory_pin {
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
/// Takes a [`MemoryPin`] for as long as this value lives.
///
/// The C header says a caller "brackets the region itself", which is the same
/// discipline every other paired call in that header asks for and the same
/// discipline that survives until the first early return. Here the bracket is
/// the borrow: a trim arriving while this exists releases nothing and reports
/// what it *would* have released, and the pin comes back when the guard is
/// dropped — including when a `?` or a panic unwinds past it, which is the
/// case a hand-written release does not cover.
pub struct PinHold<'a> {
    pin: &'a mut MemoryPin,
}

impl Drop for PinHold<'_> {
    fn drop(&mut self) {
        // A release that fails has nothing left to do about it: the pin is a
        // counter the engine documents as harmless to over-release, and a
        // panic in a drop while unwinding aborts the process.
        let _ = self.pin.release();
    }
}

impl std::ops::Deref for PinHold<'_> {
    type Target = MemoryPin;

    /// The pin itself, so a trim taken inside the region can be handed the pin
    /// the region is holding. Without it the guard's mutable borrow would make
    /// the held pin unreachable for the whole scope it exists to protect.
    fn deref(&self) -> &MemoryPin {
        self.pin
    }
}

impl std::fmt::Debug for PinHold<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinHold").finish()
    }
}

impl MemoryPin {
    /// Holds the pin until the returned guard is dropped.
    ///
    /// The form to reach for. [`acquire`](Self::acquire) and
    /// [`release`](Self::release) remain for a host whose region does not fit
    /// a scope — a save driven by a state machine across several frames — and
    /// they are still a counter, so a guard taken inside such a region does
    /// not un-pin it on the way out.
    pub fn hold(&mut self) -> Result<PinHold<'_>> {
        self.acquire()?;
        Ok(PinHold { pin: self })
    }
}

impl MemoryLedger {
    /// Adds another surface's ledger into this one.
    ///
    /// The engine fills a ledger and never merges two, and the boundary is
    /// meant: only a host knows which surfaces belong to which document. This
    /// is that addition, offered here because the category array is private —
    /// not an engine call, and it says nothing about ownership that the caller
    /// did not already decide.
    ///
    /// Where the two were filled by builds that know a different number of
    /// categories, the merged count is the shorter of them: a prefix both
    /// agree on is the honest answer, and reporting the longer would claim a
    /// figure for a category one side never filled.
    pub fn merge(&mut self, other: &MemoryLedger) {
        self.essential = self.essential.saturating_add(other.essential);
        self.rebuildable = self.rebuildable.saturating_add(other.rebuildable);
        self.undoable = self.undoable.saturating_add(other.undoable);
        self.total = self.total.saturating_add(other.total);
        for (into, from) in self.bytes.iter_mut().zip(other.bytes.iter()) {
            *into = into.saturating_add(*from);
        }
        self.category_count = self.category_count.min(other.category_count);
    }

    fn to_raw(self) -> sys::clay_memory_ledger {
        let mut raw = sys::clay_memory_ledger::sized();
        raw.category_count = self.category_count;
        raw.essential = self.essential;
        raw.rebuildable = self.rebuildable;
        raw.undoable = self.undoable;
        raw.total = self.total;
        raw.bytes = self.bytes;
        raw
    }
}

// -- what a document costs --------------------------------------------------

/// Where a document's memory is, in the terms that decide what may be released.
///
/// A floor rather than an equality, and larger than the same document's file:
/// a `.clayspace` is RLE- and palette-compressed and a live voxel chunk is a
/// flat array whether one cell is set or all of them.
///
/// [`voxel_content`](Self::voxel_content) follows *chunks*, not cells: a chunk
/// is 32³ cells allocated whole, so one voxel costs 32 KiB and 32 768 filling
/// that same chunk cost the same 32 KiB. What grows it is the region an artist
/// has worked in, not how solidly they filled it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryReport {
    // -- the model. None of this may be released; it *is* the user's work.
    /// Nodes, strokes, deformer chains, sampled volumes.
    pub edit_list: u64,
    /// Chunk storage across every level.
    pub voxel_content: u64,
    /// Imported geometry, unrecoverable.
    pub mesh_layers: u64,
    pub masks: u64,

    // -- droppable, in the order to reach for it.
    /// Undo for voxel layers. Held *inside* the grids, beside
    /// [`voxel_content`](Self::voxel_content), and separated from it for
    /// exactly that reason: this is the only voxel figure a host may act on.
    pub voxel_sculpt_layers: u64,
    /// The undo history and its journal — the only part of a document the
    /// engine evicts itself. The lever for it, `clay_document_set_history_budget`,
    /// is not wrapped in this crate yet.
    pub history: u64,
    /// A thumbnail and camera bookmarks, carried without being interpreted.
    pub passthrough: u64,

    /// Memory held only while an operation is in flight.
    ///
    /// **Through this ABI it is always zero**, and that is a statement about
    /// the ABI rather than about the mechanism: every mask entry point opens
    /// its step and closes it before returning, so there is no moment at which
    /// a host could hold a handle, have a step open, and call this. It is
    /// reported so that [`total`](Self::total) stays the sum of the fields
    /// above if an entry point that spans a step is ever added.
    pub transient: u64,
    /// The sum of every field above.
    pub total: u64,

    // -- what is here, for presenting the figure.
    pub voxel_layers: u64,
    pub mesh_layer_count: u64,
    pub mask_count: u64,

    // -- the surface tier.
    /// Adaptive surfaces: geometry and connectivity.
    pub surface_content: u64,
    /// The coefficients: the wrinkles themselves.
    pub multires_detail: u64,
    /// A mesh layer stack's content.
    pub sculpt_layers: u64,
    /// Chunk indices, evaluated levels, runtime caches.
    pub surface_caches: u64,
    /// Per-stamp working sets and preview staging.
    pub surface_scratch: u64,
    /// Vertex deltas and detail undo.
    pub surface_undo: u64,

    /// The user's work; never released.
    pub essential: u64,
    /// Reconstructs to an identical surface.
    pub rebuildable: u64,
    /// Undo depth, and the host's own policy.
    pub undoable: u64,
}

impl MemoryReport {
    fn from_raw(raw: sys::clay_memory_report) -> Self {
        Self {
            edit_list: raw.edit_list,
            voxel_content: raw.voxel_content,
            mesh_layers: raw.mesh_layers,
            masks: raw.masks,
            voxel_sculpt_layers: raw.voxel_sculpt_layers,
            history: raw.history,
            passthrough: raw.passthrough,
            transient: raw.transient,
            total: raw.total,
            voxel_layers: raw.voxel_layers,
            mesh_layer_count: raw.mesh_layer_count,
            mask_count: raw.mask_count,
            surface_content: raw.surface_content,
            multires_detail: raw.multires_detail,
            sculpt_layers: raw.sculpt_layers,
            surface_caches: raw.surface_caches,
            surface_scratch: raw.surface_scratch,
            surface_undo: raw.surface_undo,
            essential: raw.essential,
            rebuildable: raw.rebuildable,
            undoable: raw.undoable,
        }
    }
}

impl Document {
    /// What this document costs, with the surface tier at zero.
    ///
    /// Zero because a hierarchy and an adaptive surface are held *beside* a
    /// document rather than inside it, so the document cannot walk them and a
    /// guess would be worse than nothing. Use
    /// [`memory_with_surfaces`](Self::memory_with_surfaces) once the host has
    /// asked its own surfaces what they cost.
    pub fn memory(&self) -> Result<MemoryReport> {
        let mut raw = sys::clay_memory_report::sized();
        // SAFETY: a valid document handle and a versioned out-descriptor whose
        // struct_size is set above.
        check(
            unsafe { sys::clay_document_memory(self.as_ptr(), &mut raw) },
            "clay_document_memory",
        )?;
        Ok(MemoryReport::from_raw(raw))
    }

    /// The same report, with a ledger the host filled folded into the
    /// surface-tier lines and into the three roll-ups.
    ///
    /// `surfaces` is what the host built from [`crate::Multires::memory_ledger`]
    /// and [`MeshSculptor::memory_ledger`], [merged](MemoryLedger::merge) by
    /// the host because only the host knows which surfaces belong to this
    /// document.
    pub fn memory_with_surfaces(&self, surfaces: &MemoryLedger) -> Result<MemoryReport> {
        let ledger = surfaces.to_raw();
        let mut raw = sys::clay_memory_report::sized();
        // SAFETY: a valid document handle, a versioned ledger the engine only
        // reads, and a versioned out-descriptor. Both struct_sizes are set
        // from the types compiled here.
        check(
            unsafe { sys::clay_document_memory_with_surfaces(self.as_ptr(), &ledger, &mut raw) },
            "clay_document_memory_with_surfaces",
        )?;
        Ok(MemoryReport::from_raw(raw))
    }

    /// The same breakdown for one layer, so a large document can be attributed
    /// to the layer responsible rather than merely reported as large.
    ///
    /// [`history`](MemoryReport::history) and
    /// [`passthrough`](MemoryReport::passthrough) are document-wide and are
    /// therefore always zero here.
    ///
    /// The content lines sum exactly across layers and the edit list does not:
    /// every voxel chunk, mask cell and triangle belongs to exactly one layer,
    /// but instance layers share one edit list, which the document counts once
    /// and each instance reports in full. A layer's
    /// [`edit_list`](MemoryReport::edit_list) is a *ceiling* on its
    /// contribution, not a partition of it.
    pub fn layer_memory(&self, layer: LayerId) -> Result<MemoryReport> {
        let mut raw = sys::clay_memory_report::sized();
        // SAFETY: a valid document handle, a layer id the engine validates
        // itself (an unknown one is NOT_FOUND rather than a zeroed report),
        // and a versioned out-descriptor.
        check(
            unsafe { sys::clay_layer_memory(self.as_ptr(), layer.0, &mut raw) },
            "clay_layer_memory",
        )?;
        Ok(MemoryReport::from_raw(raw))
    }
}

impl MeshSculptor {
    /// What this session costs, in the shared vocabulary.
    ///
    /// Not const upstream: the figure includes caches the sculptor fills as it
    /// is asked about them.
    pub fn memory_ledger(&mut self) -> Result<MemoryLedger> {
        let mut raw = sys::clay_memory_ledger::sized();
        // SAFETY: an owned sculptor handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_mesh_sculptor_memory_ledger(self.as_ptr(), &mut raw) },
            "clay_mesh_sculptor_memory_ledger",
        )?;
        Ok(MemoryLedger::from_raw(raw))
    }
}

// -- pricing an operation before it is paid for -----------------------------

/// Runs one preflight entry point into a [`SurfacePreflight`].
///
/// The five share an estimator upstream and they share a descriptor here for
/// the same reason: a per-call transcription is a place for one of them to
/// forget the `struct_size` or the refusal.
fn preflight(
    operation: &'static str,
    call: impl FnOnce(&mut sys::clay_surface_preflight) -> crate::error::RawResult,
) -> Result<SurfacePreflight> {
    let mut raw = sys::clay_surface_preflight::sized();
    check(call(&mut raw), operation)?;
    Ok(SurfacePreflight::from_raw(raw))
}

impl Mesh {
    /// What turning this mesh into an adaptive surface would cost.
    ///
    /// The peak holds the source mesh, the half-edge structure and the weld
    /// map at once, which is why it is the figure to check rather than the
    /// result's size.
    ///
    /// A `budget` of zero means no budget, which is what a desktop host passes
    /// and what every caller got before there was a budget to pass.
    pub fn preflight_to_dynamic(&self, budget: u64) -> Result<SurfacePreflight> {
        preflight("clay_mesh_preflight_to_dynamic", |raw| {
            // SAFETY: a valid mesh handle the call only reads, and a versioned
            // out-descriptor whose struct_size is set by `preflight`.
            unsafe { sys::clay_mesh_preflight_to_dynamic(self.as_ptr(), budget, raw) }
        })
    }

    /// What a global remesh to `target_triangles` would cost.
    ///
    /// Source and target are live at the same time, which is the whole reason
    /// this one is asked. Note that it prices an *adaptive* global remesh to a
    /// triangle count and not the voxel rebuild — for that,
    /// [`Mesh::remesh_estimate`] is the preflight, and using this one there
    /// would price something other than what runs.
    pub fn preflight_global_remesh(
        &self,
        target_triangles: u64,
        budget: u64,
    ) -> Result<SurfacePreflight> {
        preflight("clay_mesh_preflight_global_remesh", |raw| {
            // SAFETY: as above.
            unsafe {
                sys::clay_mesh_preflight_global_remesh(self.as_ptr(), target_triangles, budget, raw)
            }
        })
    }
}
