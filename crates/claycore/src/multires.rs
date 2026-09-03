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
//! and the sculptor keeps a bare pointer to it. A lifetime alone is not the
//! whole of that rule, though: see [`SurfaceMut`] for the half a `&mut` does
//! not cover.
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
//! **A stack of passes on top of all of it.** The hierarchy carries a sculpt
//! layer stack — named, reorderable, dialable channels of detail summed over
//! the level's own:
//!
//! ```text
//! E(n) = B(n) + SUM over i of  s_i * m_i(v) * L_i(n, v)
//! ```
//!
//! See [`SculptLayerId`] for the one thing about it a host must not get
//! wrong.
//!
//! # Two stacks share a noun, and must not share a type
//!
//! [`crate::VoxelGrid`] has had a "sculpt layer" stack since long before this
//! tier, and upstream spends the same word here **on purpose**: the artist's
//! statement is identical — a named pass you keep, as against undo, which is a
//! stack you pop — so the two read alike in a host's interface and neither
//! reads like `MeshBrush::Layer`, which is a brush algorithm. That is a shared
//! name and not a collision, and papering over it by inventing a third word
//! would cost the host the one widget that can draw both stacks.
//!
//! What the two do **not** share is how a pass is addressed, and reusing one
//! addressing for the other is a defect the C header documents itself against:
//!
//! | | the grid's stack | the hierarchy's stack |
//! |---|---|---|
//! | addressed by | `usize` position | [`SculptLayerId`], minted once |
//! | a reorder | renumbers every position at or below it | renumbers nothing |
//! | order | replays cell writes, so it *is* the result | additive, so it changes organisation and not geometry |
//! | opened by | `begin_sculpt_layer` / `end_sculpt_layer` recording | an active layer plus a per-stroke [`WriteDomain`] |
//!
//! So the grid's stack keeps its bare `usize` and this one is addressed by a
//! newtype that cannot be built from a stack position by accident. An index
//! crosses this boundary in exactly one place —
//! [`Multires::sculpt_layer_id_at`], which exists to walk the stack in draw
//! order — and what it hands back is an id.
//!
//! # What is deliberately not here yet
//!
//! The projection pass (`clay_multires_project`) is left for the change that
//! adopts it, and a wrapper nothing runs is a SAFETY comment nobody has
//! checked.
//!
//! Neither the level sculptor's stroke record nor the sculpt layer stroke's
//! crosses the C ABI at all, which the header states twice rather than leaving
//! to be discovered: [`SculptLayerStroke::commit`] reports how many entries
//! the record held and nothing else, so one layered gesture cannot yet become
//! one entry in a host's undo stack.
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

use std::ptr::NonNull;

use claycore_sys as sys;

use crate::buffer::{size_query_bytes, size_query_string};
use crate::descriptor::Descriptor;
use crate::error::{check, ClayError, ErrorKind, RawResult, Result};
use crate::mask::MaskField;
use crate::memory::{
    MemoryLedger, MemoryPin, Pressure, SculptMemoryProfile, SurfacePreflight, TrimReport,
};
use crate::mesh::Mesh;
use crate::mesh_sculpt::MeshStamp;
use crate::{cstring, engine_text, raw_failure, Document, LayerId};

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
    /// The raw handle, for sibling modules in this crate only.
    pub(crate) fn as_ptr(&mut self) -> *mut sys::clay_multires {
        self.raw.as_ptr()
    }

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
    pub fn preflight_encode(&self, budget: u64) -> Result<SurfacePreflight> {
        let mut raw = sys::clay_surface_preflight::sized();
        // SAFETY: valid handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_multires_preflight_encode(self.raw.as_ptr(), budget, &mut raw) },
            "clay_multires_preflight_encode",
        )?;
        Ok(SurfacePreflight::from_raw(raw))
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
    /// half of the ABI's own rule expressed in the type system: the sculptor
    /// keeps a bare pointer to the surface and using it after the surface is
    /// gone would be a use-after-free. The other half is [`SurfaceMut`] — an
    /// exclusive borrow stops the surface being moved out from under the
    /// pointer and does nothing at all about it being *replaced where it
    /// stands*, which runs the same destructor from entirely safe code.
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
        // does, and nothing that borrows it can reach a `&mut Multires` to run
        // this destructor early through (see `SurfaceMut`), so nothing else
        // holds a pointer into it here.
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

// -- lending a hierarchy that something else points into ---------------------

/// A hierarchy lent back by something that holds a bare pointer into it.
///
/// A [`MultiresSculptor`] and a [`SculptLayerStroke`] both keep an engine
/// handle built from a reference *into* the surface, so the surface has to
/// outlive them — the rule the C header states in one line and the reason both
/// borrow their hierarchy exclusively. But an exclusive borrow only forbids
/// the surface being *moved*; it does not forbid it being **replaced where it
/// stands**. Handing out a `&mut Multires` therefore hands out
/// `clay_multires_destroy`: `*sculptor.surface_mut() = other` — plain
/// assignment, no `unsafe` anywhere near the caller — drops the hierarchy the
/// live sculptor still points at, and the next stamp reads freed storage.
/// Measured on the pinned engine, that is a SIGSEGV; for a stroke it is worse,
/// because `Drop` cancels *into* the surface and so the crash needs no further
/// call at all.
///
/// So the surface is lent through this instead. It reads like the hierarchy —
/// every `&self` method arrives through [`Deref`] — and it forwards the
/// `&mut self` ones a session legitimately makes between stamps. What it never
/// yields is a `&mut Multires`, which is the one thing an owning wrapper with
/// a destructor must not be reachable through while a raw pointer into it is
/// live.
///
/// Four of the hierarchy's own methods are deliberately absent rather than
/// forwarded. [`Multires::add_level`] and [`Multires::remove_highest_level`]
/// rebuild the level set under a pointer that was bound to it; and
/// [`Multires::sculptor`] and [`Multires::sculpt_layer_stroke`] would open a
/// second live handle over a surface that already has one. Take all four
/// before the session starts.
pub struct SurfaceMut<'a> {
    surface: &'a mut Multires,
}

impl std::ops::Deref for SurfaceMut<'_> {
    type Target = Multires;

    fn deref(&self) -> &Multires {
        self.surface
    }
}

impl std::fmt::Debug for SurfaceMut<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.surface.fmt(f)
    }
}

/// Forwards one of the hierarchy's own `&mut self` methods.
///
/// A macro rather than twenty-four hand-written bodies, so the lent surface
/// cannot drift from the thing it lends by a copy-and-paste.
macro_rules! lends {
    ($($name:ident($($arg:ident : $ty:ty),*) -> $ret:ty;)*) => {
        impl SurfaceMut<'_> {
            $(
                #[doc = concat!("See [`Multires::", stringify!($name), "`].")]
                pub fn $name(&mut self, $($arg: $ty),*) -> $ret {
                    self.surface.$name($($arg),*)
                }
            )*
        }
    };
}

lends! {
    set_sculpt_level(level: u32) -> Result<()>;
    set_display_level(level: u32) -> Result<()>;
    copy_level_mesh(level: u32) -> Result<Mesh>;
    set_memory_profile(profile: SculptMemoryProfile) -> Result<()>;
    drop_inactive_caches() -> Result<()>;
    trim(pressure: Pressure, pin: Option<&MemoryPin>) -> Result<TrimReport>;
    compact_sculpt_layers() -> Result<()>;
    clear_dirty() -> Result<()>;
    block_info(patch: u32, level: u32) -> Result<BlockInfo>;
    copy_block(patch: u32, level: u32) -> Result<Block>;
    add_sculpt_layer(name: Option<&str>) -> std::result::Result<SculptLayerId, MultiresRefusal>;
    remove_sculpt_layer(id: SculptLayerId) -> std::result::Result<(), MultiresRefusal>;
    move_sculpt_layer(id: SculptLayerId, index: usize) -> std::result::Result<(), MultiresRefusal>;
    merge_sculpt_layer_down(id: SculptLayerId) -> std::result::Result<(), MultiresRefusal>;
    bake_sculpt_layer_to_base(id: SculptLayerId) -> std::result::Result<(), MultiresRefusal>;
    rename_sculpt_layer(id: SculptLayerId, name: &str) -> std::result::Result<(), MultiresRefusal>;
    set_sculpt_layer_strength(id: SculptLayerId, strength: f32) -> std::result::Result<(), MultiresRefusal>;
    set_sculpt_layer_visible(id: SculptLayerId, visible: bool) -> std::result::Result<(), MultiresRefusal>;
    set_sculpt_layer_locked(id: SculptLayerId, locked: bool) -> std::result::Result<(), MultiresRefusal>;
    set_active_sculpt_layer(id: SculptLayerId) -> std::result::Result<(), MultiresRefusal>;
    set_sculpt_layer_detail(id: SculptLayerId, level: u32, vertex: u32, tbn: [f32; 3]) -> std::result::Result<(), MultiresRefusal>;
    set_sculpt_layer_mask(id: SculptLayerId, level: u32, vertex: u32, weight: f32) -> std::result::Result<(), MultiresRefusal>;
    reset_sculpt_layer_stats() -> Result<()>;
    hold_sculpt_layer_composition(held: bool) -> Result<()>;
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
    ///
    /// Lent through a [`SurfaceMut`] rather than as a `&mut Multires`: this
    /// sculptor holds a bare pointer into the surface, and a `&mut` to an
    /// owning wrapper is a destructor safe code can run — see [`SurfaceMut`].
    pub fn surface_mut(&mut self) -> SurfaceMut<'_> {
        SurfaceMut {
            surface: self.surface,
        }
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
        // of the surface it points at ends. Nothing reachable from that borrow
        // yields a `&mut Multires` (see `SurfaceMut`), so the surface this
        // holds a bare pointer to cannot have been destroyed early and is
        // still alive here.
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

// -- the sculpt layer stack -------------------------------------------------

/// Which pass of a hierarchy's stack. **An id, never an index.**
///
/// This is a newtype rather than a `u64` for one reason, and it is the same
/// reason the C header sets in capitals: a *position* in the stack is renamed
/// by [`Multires::move_sculpt_layer`], which changes every position at or
/// below the layer it moves, so a position written into a file, handed to a
/// host or held across a drag names a different pass afterwards. An id is
/// minted once from a counter that is itself serialized — a save, a load and a
/// reorder leave every id exactly where the host left it.
///
/// [`crate::VoxelGrid`]'s stack is addressed by position and this one is not,
/// which is why the two cannot share a type even though they share a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SculptLayerId(u64);

impl SculptLayerId {
    /// Not a layer: the level's own detail, under every pass.
    ///
    /// What an empty stack's active layer reads as, and what a host sets to
    /// route the next stroke into the form beneath the passes — which is what
    /// every stroke did before this stack existed.
    pub const BASE: Self = Self(sys::CLAY_NO_SCULPT_LAYER as u64);

    /// An id read back from a file or a side-car.
    ///
    /// The honest constructor, and the only one: there is deliberately no
    /// `From<usize>`, because the value that would tempt a caller to write one
    /// is a stack position, and the two are not interchangeable.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// The value to store beside the hierarchy's own bytes.
    pub fn get(self) -> u64 {
        self.0
    }

    pub fn is_base(self) -> bool {
        self == Self::BASE
    }
}

/// What kind of thing a pass stores.
///
/// Versioned from the first release though only one kind ships, because a
/// reader that met a kind it did not know and *skipped* the layer would
/// present a surface missing an artist's work while claiming to be complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SculptLayerKind {
    /// Coefficients per vertex; the only kind this build writes or reads.
    Sampled,
    /// Reserved, and refused by this build's decoder.
    Procedural,
    /// A kind this build does not know, carried verbatim rather than guessed
    /// at.
    Unknown(i32),
}

impl SculptLayerKind {
    fn from_raw(code: i32) -> Self {
        use sys::clay_sculpt_layer_kind as k;
        match code as sys::clay_sculpt_layer_kind::Type {
            k::CLAY_SCULPT_LAYER_SAMPLED => Self::Sampled,
            k::CLAY_SCULPT_LAYER_PROCEDURAL => Self::Procedural,
            _ => Self::Unknown(code),
        }
    }
}

/// One pass, as a host lists it.
///
/// The name is not here. It crosses through
/// [`Multires::sculpt_layer_name`] into a buffer the caller owns, because a
/// pointer into an engine-owned string has no lifetime a host could reason
/// about — the next rename, remove or reorder frees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SculptLayerInfo {
    pub id: SculptLayerId,
    /// Bottom-first, and **valid only until the next structural change**. For
    /// drawing a list in order, never for holding on to.
    pub index: u32,
    pub kind: SculptLayerKind,
    /// Composition, not a scale on the pen: a stroke into a layer at 0.5
    /// records its *full* contribution and the surface moves half as far, so
    /// raising this to 1 afterwards doubles what is on screen and replays no
    /// stroke.
    pub strength: f32,
    pub visible: bool,
    /// Refuses a coefficient write and permits every property change — the
    /// rule stated rather than discovered, because a lock that also froze the
    /// name and the slider would make "lock" mean "hide from the interface".
    pub locked: bool,
    /// What [`Multires::sculpt_layer_name`] will ask for, terminator
    /// included, so a host sizes one buffer from this rather than calling
    /// twice.
    pub name_bytes: u32,
    /// Coefficients and mask, allocated.
    pub bytes: u64,
    /// A layer costs its *coverage* and not the model, which is what makes a
    /// hundred passes over one cheek affordable.
    pub coverage_vertices: u64,
}

impl SculptLayerInfo {
    fn from_raw(raw: sys::clay_sculpt_layer_info) -> Self {
        Self {
            id: SculptLayerId(raw.id),
            index: raw.index,
            kind: SculptLayerKind::from_raw(raw.kind),
            strength: raw.strength,
            visible: raw.visible != 0,
            locked: raw.locked != 0,
            name_bytes: raw.name_bytes,
            bytes: raw.bytes,
            coverage_vertices: raw.coverage_vertices,
        }
    }
}

/// What composition actually did.
///
/// Measurements rather than assertions, and there is no other way to see
/// either from outside: a correct implementation and a quadratic one produce
/// the same surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SculptLayerStats {
    /// What a strength change cost — the layer's allocated blocks, never the
    /// level's.
    pub blocks_recomposed: u64,
    /// The (block, layer) pairs actually summed, so a stamp on top of a deep
    /// stack can be *shown* not to sum every layer beneath it over unrelated
    /// geometry.
    pub layer_blocks_visited: u64,
    /// Calls that recomposed at least one block.
    pub compositions: u64,
}

impl SculptLayerStats {
    fn from_raw(raw: sys::clay_sculpt_layer_stats) -> Self {
        Self {
            blocks_recomposed: raw.blocks_recomposed,
            layer_blocks_visited: raw.layer_blocks_visited,
            compositions: raw.compositions,
        }
    }
}

/// The stack's three counters, for the same reason the hierarchy has three:
/// one counter cannot say which of three things happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SculptLayerRevisions {
    /// A rename, a change of active layer. Invalidates *nothing*, so a host
    /// keyed on the two below does not re-evaluate a model because a layer was
    /// renamed.
    pub metadata: u64,
    /// Strength, visibility, mask, order, add, remove.
    pub composition: u64,
    /// Coefficients written.
    pub content: u64,
}

/// The refusal protocol the stack's entry points share.
///
/// Written once because it is the same four lines at twenty call sites and
/// exactly one of them would read `reason` only on failure. The engine writes
/// it on **every** path, including success, where it is `CLAY_MULTIRES_OK` —
/// so it is read unconditionally here and the result code decides what to do
/// with it.
fn refusable(
    operation: &'static str,
    code: RawResult,
    reason: i32,
) -> std::result::Result<(), MultiresRefusal> {
    let reason = MultiresError::from_raw(reason);
    check(code, operation).map_err(|error| MultiresRefusal { error, reason })
}

impl Multires {
    /// How many passes the stack holds. Zero is a surface with base detail and
    /// nothing over it, which is what every hierarchy starts as.
    pub fn sculpt_layer_count(&self) -> Result<usize> {
        let mut count = 0usize;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_multires_sculpt_layer_count(self.raw.as_ptr(), &mut count) },
            "clay_multires_sculpt_layer_count",
        )?;
        Ok(count)
    }

    /// The id at a stack position, bottom-first.
    ///
    /// The one place a position crosses this boundary, and it exists to walk
    /// the stack in draw order. What it answers is an *id*, which is what a
    /// host keeps; past the end is a `NotFound`.
    pub fn sculpt_layer_id_at(&self, index: usize) -> Result<SculptLayerId> {
        let mut id = 0u64;
        // SAFETY: valid handle; the index is range-checked by the entry point,
        // which answers NOT_FOUND past the end rather than clamping.
        check(
            unsafe { sys::clay_multires_sculpt_layer_id_at(self.raw.as_ptr(), index, &mut id) },
            "clay_multires_sculpt_layer_id_at",
        )?;
        Ok(SculptLayerId(id))
    }

    /// The whole stack, bottom-first — the loop a host draws its list with.
    pub fn sculpt_layer_ids(&self) -> Result<Vec<SculptLayerId>> {
        (0..self.sculpt_layer_count()?)
            .map(|index| self.sculpt_layer_id_at(index))
            .collect()
    }

    /// One pass's properties.
    ///
    /// An unknown id is a `NotFound` rather than a zeroed descriptor, and that
    /// distinction is the entry point's own: a zeroed descriptor is
    /// indistinguishable from a real layer at strength 0.
    pub fn sculpt_layer_info(&self, id: SculptLayerId) -> Result<SculptLayerInfo> {
        let mut raw = sys::clay_sculpt_layer_info::sized();
        // SAFETY: valid handle and a versioned out-descriptor whose
        // struct_size is written from the compiled type.
        check(
            unsafe { sys::clay_multires_sculpt_layer_info(self.raw.as_ptr(), id.get(), &mut raw) },
            "clay_multires_sculpt_layer_info",
        )?;
        Ok(SculptLayerInfo::from_raw(raw))
    }

    /// A pass's name, copied out. An unnamed pass is an empty string.
    pub fn sculpt_layer_name(&self, id: SculptLayerId) -> Result<String> {
        let handle = self.raw.as_ptr();
        size_query_string("clay_multires_sculpt_layer_name", |buffer, size| {
            // SAFETY: the buffer protocol's two calls. `buffer` is either null
            // — asking for the byte count including the terminator — or valid
            // for writes of `*size` bytes, which is what the engine is told it
            // has and what it checks before it copies.
            unsafe { sys::clay_multires_sculpt_layer_name(handle, id.get(), buffer, size) }
        })
    }

    /// Which pass the next sculpt write lands in.
    ///
    /// [`SculptLayerId::BASE`] is the level's own detail — the form under the
    /// passes.
    pub fn active_sculpt_layer(&self) -> Result<SculptLayerId> {
        let mut id = 0u64;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_multires_active_sculpt_layer(self.raw.as_ptr(), &mut id) },
            "clay_multires_active_sculpt_layer",
        )?;
        Ok(SculptLayerId(id))
    }

    /// A new empty pass on top: full strength, visible, made active.
    ///
    /// Refused while a stroke holds the composition. `name` may be `None` for
    /// an unnamed pass.
    pub fn add_sculpt_layer(
        &mut self,
        name: Option<&str>,
    ) -> std::result::Result<SculptLayerId, MultiresRefusal> {
        let owned = name
            .map(|name| cstring(name, "clay_multires_add_sculpt_layer"))
            .transpose()
            .map_err(|error| MultiresRefusal {
                error,
                reason: MultiresError::None,
            })?;
        let mut id = 0u64;
        let mut reason = 0i32;
        // SAFETY: valid handle; `name` is either null — which the entry point
        // documents as an unnamed pass — or a NUL-terminated string borrowed
        // for the duration of the call and copied by the engine; both
        // out-parameters are written before they are read below.
        let code = unsafe {
            sys::clay_multires_add_sculpt_layer(
                self.raw.as_ptr(),
                owned.as_ref().map_or(std::ptr::null(), |n| n.as_ptr()),
                &mut id,
                &mut reason,
            )
        };
        refusable("clay_multires_add_sculpt_layer", code, reason)?;
        Ok(SculptLayerId(id))
    }

    /// Discards a pass.
    ///
    /// Re-evaluates the removed layer's *coverage* and nothing else: no stroke
    /// is replayed, and no other layer's coefficients, strength or relative
    /// order change.
    pub fn remove_sculpt_layer(
        &mut self,
        id: SculptLayerId,
    ) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle, an id the entry point resolves or refuses, and
        // an out-parameter written on every path.
        let code = unsafe {
            sys::clay_multires_remove_sculpt_layer(self.raw.as_ptr(), id.get(), &mut reason)
        };
        refusable("clay_multires_remove_sculpt_layer", code, reason)
    }

    /// Slides a pass to a stack position.
    ///
    /// **Organisation only, and this is a guarantee rather than a hope.** An
    /// additive stack commutes, so a reorder cannot move a vertex: the engine
    /// sums a block's contributors in id order precisely so that the surface
    /// is invariant under exactly this call. A host that invents an ordering
    /// rule here is solving a problem this representation does not have.
    pub fn move_sculpt_layer(
        &mut self,
        id: SculptLayerId,
        index: usize,
    ) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle, an id and an index both resolved or refused by
        // the entry point, and an out-parameter written on every path.
        let code = unsafe {
            sys::clay_multires_move_sculpt_layer(self.raw.as_ptr(), id.get(), index, &mut reason)
        };
        refusable("clay_multires_move_sculpt_layer", code, reason)
    }

    /// Folds a pass into the one below it and discards it.
    ///
    /// **Defined by visual parity**: the evaluated surface before equals the
    /// evaluated surface after, at any strength *including zero*. That is why
    /// it is an entry point rather than host arithmetic — the naive form
    /// `L' = L_l + (s_u·m_u)/(s_l·m_l)·L_u` divides by the lower layer's
    /// strength, and zero is a state one slider reaches. The engine stores the
    /// sum directly and sets the target's composition to the identity it
    /// needs, so nothing divides by a strength.
    ///
    /// What is lost is real and is named rather than smoothed over: the merged
    /// layer's slider no longer scales what the upper layer contributed
    /// independently, which is what merging *means*.
    pub fn merge_sculpt_layer_down(
        &mut self,
        id: SculptLayerId,
    ) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle, an id the entry point resolves or refuses, and
        // an out-parameter written on every path.
        let code = unsafe {
            sys::clay_multires_merge_sculpt_layer_down(self.raw.as_ptr(), id.get(), &mut reason)
        };
        refusable("clay_multires_merge_sculpt_layer_down", code, reason)
    }

    /// The same statement with the *base* as the target: the level's own
    /// detail, and the cage itself for a level-0 deformation.
    ///
    /// The pass stops being dialable and becomes the form. Visual parity
    /// again, and at any strength including zero.
    pub fn bake_sculpt_layer_to_base(
        &mut self,
        id: SculptLayerId,
    ) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle, an id the entry point resolves or refuses, and
        // an out-parameter written on every path.
        let code = unsafe {
            sys::clay_multires_bake_sculpt_layer_to_base(self.raw.as_ptr(), id.get(), &mut reason)
        };
        refusable("clay_multires_bake_sculpt_layer_to_base", code, reason)
    }

    /// Renames a pass. Permitted on a locked layer and during a stroke: it
    /// moves no vertex.
    pub fn rename_sculpt_layer(
        &mut self,
        id: SculptLayerId,
        name: &str,
    ) -> std::result::Result<(), MultiresRefusal> {
        let owned = cstring(name, "clay_multires_rename_sculpt_layer").map_err(|error| {
            MultiresRefusal {
                error,
                reason: MultiresError::None,
            }
        })?;
        let mut reason = 0i32;
        // SAFETY: valid handle, a NUL-terminated string borrowed for the
        // duration of the call and copied by the engine, and an out-parameter
        // written on every path.
        let code = unsafe {
            sys::clay_multires_rename_sculpt_layer(
                self.raw.as_ptr(),
                id.get(),
                owned.as_ptr(),
                &mut reason,
            )
        };
        refusable("clay_multires_rename_sculpt_layer", code, reason)
    }

    /// Dials a pass. 1 contributes fully, 0 contributes nothing, and neither
    /// replays a stroke.
    ///
    /// A composition change, so it is refused while a stroke is open: a stamp
    /// reads the evaluated surface, and a slider moved between two stamps
    /// would author one gesture against two different surfaces.
    pub fn set_sculpt_layer_strength(
        &mut self,
        id: SculptLayerId,
        strength: f32,
    ) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle, a plain float, and an out-parameter written on
        // every path.
        let code = unsafe {
            sys::clay_multires_set_sculpt_layer_strength(
                self.raw.as_ptr(),
                id.get(),
                strength,
                &mut reason,
            )
        };
        refusable("clay_multires_set_sculpt_layer_strength", code, reason)
    }

    /// Hides or shows a pass.
    ///
    /// Invisible is *exactly* zero rather than nearly zero, so a host can
    /// compare the two surfaces bit for bit.
    pub fn set_sculpt_layer_visible(
        &mut self,
        id: SculptLayerId,
        visible: bool,
    ) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle, a plain integer, and an out-parameter written
        // on every path.
        let code = unsafe {
            sys::clay_multires_set_sculpt_layer_visible(
                self.raw.as_ptr(),
                id.get(),
                i32::from(visible),
                &mut reason,
            )
        };
        refusable("clay_multires_set_sculpt_layer_visible", code, reason)
    }

    /// Locks or unlocks a pass.
    ///
    /// A lock refuses a *coefficient* write and permits every property
    /// change — the point of it is that an artist can keep working over a
    /// finished pass.
    pub fn set_sculpt_layer_locked(
        &mut self,
        id: SculptLayerId,
        locked: bool,
    ) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle, a plain integer, and an out-parameter written
        // on every path.
        let code = unsafe {
            sys::clay_multires_set_sculpt_layer_locked(
                self.raw.as_ptr(),
                id.get(),
                i32::from(locked),
                &mut reason,
            )
        };
        refusable("clay_multires_set_sculpt_layer_locked", code, reason)
    }

    /// Routes the next sculpt write. [`SculptLayerId::BASE`] sends it into the
    /// form under the passes.
    pub fn set_active_sculpt_layer(
        &mut self,
        id: SculptLayerId,
    ) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle, an id the entry point resolves or refuses —
        // CLAY_NO_SCULPT_LAYER always resolving — and an out-parameter written
        // on every path.
        let code = unsafe {
            sys::clay_multires_set_active_sculpt_layer(self.raw.as_ptr(), id.get(), &mut reason)
        };
        refusable("clay_multires_set_active_sculpt_layer", code, reason)
    }

    /// Writes one vertex's coefficients on a pass: tangent, bitangent and
    /// normal in the vertex's own transported frame.
    ///
    /// The same three the base detail stores, because they are the same
    /// quantity under a different owner. Writing marks the block, so the next
    /// evaluation recomposes it and the levels above follow.
    pub fn set_sculpt_layer_detail(
        &mut self,
        id: SculptLayerId,
        level: u32,
        vertex: u32,
        tbn: [f32; 3],
    ) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle; `tbn` is three contiguous floats, which is
        // what the entry point reads and all it reads; the level and vertex
        // are range-checked by it; the out-parameter is written on every path.
        let code = unsafe {
            sys::clay_multires_set_sculpt_layer_detail(
                self.raw.as_ptr(),
                id.get(),
                level,
                vertex,
                tbn.as_ptr(),
                &mut reason,
            )
        };
        refusable("clay_multires_set_sculpt_layer_detail", code, reason)
    }

    /// Reads them back. A vertex the pass never reached is three zeroes,
    /// which is a coefficient of nothing rather than an absence.
    pub fn sculpt_layer_detail(
        &self,
        id: SculptLayerId,
        level: u32,
        vertex: u32,
    ) -> Result<[f32; 3]> {
        let mut tbn = [0.0f32; 3];
        // SAFETY: valid handle and a buffer valid for writes of exactly the
        // three floats the entry point writes; level and vertex are
        // range-checked by it.
        check(
            unsafe {
                sys::clay_multires_sculpt_layer_detail(
                    self.raw.as_ptr(),
                    id.get(),
                    level,
                    vertex,
                    tbn.as_mut_ptr(),
                )
            },
            "clay_multires_sculpt_layer_detail",
        )?;
        Ok(tbn)
    }

    /// Where a *stored* pass contributes, and how much.
    ///
    /// A different question from the brush gate, which says where a brush
    /// writes and is gone when the pointer comes up; this is serialized with
    /// the layer. **Its identity is 1, not 0** — a mask the artist never
    /// touched must not erase the pass it belongs to, so an absent block means
    /// full weight. Writing exactly 1 releases the storage again.
    pub fn set_sculpt_layer_mask(
        &mut self,
        id: SculptLayerId,
        level: u32,
        vertex: u32,
        weight: f32,
    ) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle, a plain float, a range-checked level and
        // vertex, and an out-parameter written on every path.
        let code = unsafe {
            sys::clay_multires_set_sculpt_layer_mask(
                self.raw.as_ptr(),
                id.get(),
                level,
                vertex,
                weight,
                &mut reason,
            )
        };
        refusable("clay_multires_set_sculpt_layer_mask", code, reason)
    }

    /// Reads one back. An untouched vertex answers 1 — see above.
    pub fn sculpt_layer_mask(&self, id: SculptLayerId, level: u32, vertex: u32) -> Result<f32> {
        let mut weight = 0.0f32;
        // SAFETY: valid handle and an out-parameter written on success; level
        // and vertex are range-checked by the entry point.
        check(
            unsafe {
                sys::clay_multires_sculpt_layer_mask(
                    self.raw.as_ptr(),
                    id.get(),
                    level,
                    vertex,
                    &mut weight,
                )
            },
            "clay_multires_sculpt_layer_mask",
        )?;
        Ok(weight)
    }

    /// A hash of every pass's coefficients and masks, and nothing derived from
    /// them.
    ///
    /// Deliberately apart from [`detail_checksum`](Self::detail_checksum),
    /// which still hashes the base detail only — so a host asks "did the form
    /// change" and "did a pass change" separately, and a merge or a bake shows
    /// up as both moving.
    pub fn sculpt_layer_checksum(&self) -> Result<u64> {
        let mut checksum = 0u64;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_multires_sculpt_layer_checksum(self.raw.as_ptr(), &mut checksum) },
            "clay_multires_sculpt_layer_checksum",
        )?;
        Ok(checksum)
    }

    /// The stack's three counters. Compare, do not add.
    pub fn sculpt_layer_revision(&self) -> Result<SculptLayerRevisions> {
        let mut out = SculptLayerRevisions::default();
        // SAFETY: valid handle and three out-parameters, each of which the
        // entry point allows to be null and none of which is.
        check(
            unsafe {
                sys::clay_multires_sculpt_layer_revision(
                    self.raw.as_ptr(),
                    &mut out.metadata,
                    &mut out.composition,
                    &mut out.content,
                )
            },
            "clay_multires_sculpt_layer_revision",
        )?;
        Ok(out)
    }

    /// What composition has done since the last [reset](Self::reset_sculpt_layer_stats).
    pub fn sculpt_layer_stats(&self) -> Result<SculptLayerStats> {
        let mut raw = sys::clay_sculpt_layer_stats::sized();
        // SAFETY: valid handle and a versioned out-descriptor whose
        // struct_size is written from the compiled type.
        check(
            unsafe { sys::clay_multires_sculpt_layer_stats(self.raw.as_ptr(), &mut raw) },
            "clay_multires_sculpt_layer_stats",
        )?;
        Ok(SculptLayerStats::from_raw(raw))
    }

    pub fn reset_sculpt_layer_stats(&mut self) -> Result<()> {
        // SAFETY: valid handle; the call only zeroes counters.
        check(
            unsafe { sys::clay_multires_reset_sculpt_layer_stats(self.raw.as_ptr()) },
            "clay_multires_reset_sculpt_layer_stats",
        )
    }

    /// Holds the composition for the length of a gesture a host drives itself,
    /// stamp by stamp.
    ///
    /// [`SculptLayerStroke`] takes and releases this on its own, so a host
    /// using the transaction never calls it. A host stamping through the plain
    /// [`MultiresSculptor`] takes it for the same reason the transaction does:
    /// a stamp reads the evaluated surface, and recomposing between stamps
    /// costs the stack on every dab.
    ///
    /// **Balanced by the caller.** It is `held` rather than a guard because
    /// the borrow a guard would take is the same one the sculptor already
    /// holds, so a guard here would make the two mutually exclusive — which is
    /// exactly the pairing it exists for.
    pub fn hold_sculpt_layer_composition(&mut self, held: bool) -> Result<()> {
        // SAFETY: valid handle and a plain integer.
        check(
            unsafe {
                sys::clay_multires_hold_sculpt_layer_composition(self.raw.as_ptr(), i32::from(held))
            },
            "clay_multires_hold_sculpt_layer_composition",
        )
    }

    /// A layered gesture over this hierarchy.
    ///
    /// Borrows it exclusively for the same reason [`sculptor`](Self::sculptor)
    /// does: the transaction keeps a bare pointer to the surface, and using it
    /// after the surface is gone would be a use-after-free.
    pub fn sculpt_layer_stroke(&mut self) -> Result<SculptLayerStroke<'_>> {
        let mut stroke = std::ptr::null_mut();
        // SAFETY: valid handle and an out-parameter written only on success.
        check(
            unsafe {
                sys::clay_multires_sculpt_layer_stroke_create(self.raw.as_ptr(), &mut stroke)
            },
            "clay_multires_sculpt_layer_stroke_create",
        )?;
        let raw = NonNull::new(stroke).ok_or_else(|| {
            raw_failure(
                "clay_multires_sculpt_layer_stroke_create",
                ErrorKind::Backend,
            )
        })?;
        Ok(SculptLayerStroke {
            raw,
            surface: self,
            open: false,
        })
    }
}

// -- the layered stroke transaction -----------------------------------------

/// Where a stroke lands, chosen by the caller rather than inferred.
///
/// Inferred would be wrong either way: "sculpt the pass I am working on" and
/// "fix the form *under* the passes without disturbing them" are both
/// ordinary, and neither is a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WriteDomain {
    /// The active layer if there is one, the base if not.
    #[default]
    Automatic,
    /// The base: the cage at level 0, the level's own detail above it.
    Geometry,
    /// The active layer. Refuses to [begin](SculptLayerStroke::begin) when
    /// there is none, rather than silently writing the form the caller asked
    /// not to touch.
    Detail,
}

impl WriteDomain {
    fn to_raw(self) -> i32 {
        use sys::clay_multires_write_domain as d;
        (match self {
            Self::Automatic => d::CLAY_MULTIRES_WRITE_AUTOMATIC,
            Self::Geometry => d::CLAY_MULTIRES_WRITE_GEOMETRY,
            Self::Detail => d::CLAY_MULTIRES_WRITE_DETAIL,
        }) as i32
    }
}

/// Which frequency a smooth acts on.
///
/// Three operations rather than one filter with a cutoff, and the split is
/// *representational*: the hierarchy already stores the form and the detail
/// apart, so these are three different arrays rather than three settings of
/// one pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SmoothMode {
    /// Positions; exactly [`MeshBrush::Smooth`](crate::MeshBrush::Smooth). A
    /// plain Laplacian over pores removes the pores, which is rarely what was
    /// asked.
    #[default]
    Geometry,
    /// Coefficients in the target channel only.
    DetailOnly,
    /// The form, with the detail re-applied unchanged — the mode an artist
    /// correcting anatomy under pores is asking for, and the one that is
    /// impossible on a flat mesh.
    PreserveDetail,
}

impl SmoothMode {
    fn to_raw(self) -> i32 {
        use sys::clay_multires_smooth_mode as m;
        (match self {
            Self::Geometry => m::CLAY_MULTIRES_SMOOTH_GEOMETRY,
            Self::DetailOnly => m::CLAY_MULTIRES_SMOOTH_DETAIL_ONLY,
            Self::PreserveDetail => m::CLAY_MULTIRES_SMOOTH_PRESERVE_DETAIL,
        }) as i32
    }
}

/// What a high-detail stamp's samples *mean*.
///
/// The C enumeration has a third value, `CLAY_DETAIL_STAMP_WEIGHT`, and it is
/// deliberately absent here: it is the scalar alpha that already reaches every
/// verb through [`MeshStamp::alpha`], the detail entry point *refuses* it, and
/// a Rust enum that could express a value the only call taking it rejects
/// would be an error this type is in a position to make unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailStampMode {
    /// One channel: a signed displacement along the vertex normal.
    #[default]
    Height,
    /// Three channels, read in the vertex's own transported frame.
    ///
    /// **Never world space**, and that is not a preference: a world-space
    /// stamp is orientation-dependent, so the same map applied to the same
    /// feature on the left and right of a face produces two different shapes,
    /// and across a curved surface it shears.
    Vector,
}

impl DetailStampMode {
    fn to_raw(self) -> i32 {
        use sys::clay_detail_stamp_mode as m;
        (match self {
            Self::Height => m::CLAY_DETAIL_STAMP_HEIGHT,
            Self::Vector => m::CLAY_DETAIL_STAMP_VECTOR,
        }) as i32
    }

    /// How many planes the image must hold.
    fn channels(self) -> usize {
        match self {
            Self::Height => 1,
            Self::Vector => 3,
        }
    }
}

/// A height map or a tangent-space vector displacement, borrowed for one call.
///
/// **The image is planar and borrowed.** Three channels means three
/// consecutive `width * height` planes, not interleaved triples, because a
/// plane is exactly the buffer the existing alpha sampler reads. The engine
/// decodes no images and copies nothing: the samples must outlive the call and
/// nothing holds them afterwards, which is what the lifetime here says.
#[derive(Debug, Clone, Copy)]
pub struct DetailStamp<'a> {
    pub mode: DetailStampMode,
    /// `channels * width * height` samples, plane after plane.
    pub image: &'a [f32],
    pub width: i32,
    pub height: i32,
    /// World units per unit of sampled value. Signed, so one map deposits or
    /// digs without a second image.
    pub amplitude: f32,
    /// What a height map's zero is. A map cut out of a photograph sits around
    /// 0.5 and one authored as a displacement sits around 0, and guessing
    /// wrong inflates or deflates the whole stamp.
    pub bias: f32,
    /// The square: the plane through `center` whose normal is `direction`,
    /// oriented by `tangent` — any rough "up" works, it is re-orthogonalised —
    /// of side `extent`. A zero `direction` or `tangent` takes the brush's
    /// own, and a zero `extent` its diameter.
    pub center: [f32; 3],
    pub direction: [f32; 3],
    pub tangent: [f32; 3],
    pub extent: f32,
}

impl DetailStamp<'_> {
    /// Whether the samples fill the planes claimed.
    ///
    /// Checked before the pointer is handed over, for the same reason
    /// [`crate::AlphaStamp`]'s is: the engine reads
    /// `channels * width * height` floats out of it, so a shorter slice is a
    /// read past the end whatever the engine's own validation says about the
    /// dimensions.
    fn is_well_formed(&self) -> bool {
        self.width >= 2
            && self.height >= 2
            && (self.width as i64) * (self.height as i64) * self.mode.channels() as i64
                <= self.image.len() as i64
    }

    fn as_raw(&self) -> sys::clay_detail_stamp_desc {
        let mut raw = sys::clay_detail_stamp_desc::sized();
        raw.mode = self.mode.to_raw();
        raw.image = self.image.as_ptr();
        raw.width = self.width;
        raw.height = self.height;
        raw.amplitude = self.amplitude;
        raw.bias = self.bias;
        raw.center = self.center;
        raw.direction = self.direction;
        raw.tangent = self.tangent;
        raw.extent = self.extent;
        raw
    }
}

/// Whether the level can carry what the stamp holds.
///
/// Reported rather than smoothed over: a 2048-sample map across a 5 mm square
/// carries features finer than a level whose mean edge is 1 mm can represent,
/// and applying it anyway produces a surface that looks like the map through a
/// blur — which reads as a bug in the map, or in the brush, or in the artist's
/// file, and is none of those.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DetailStampReport {
    /// The world size of one image sample.
    pub sample_size: f32,
    /// The level's mean edge length.
    pub vertex_spacing: f32,
    /// Samples per vertex spacing. Above 1 is too fine.
    pub oversampling: f32,
    pub under_resolved: bool,
}

impl DetailStampReport {
    fn from_raw(raw: sys::clay_detail_stamp_report) -> Self {
        Self {
            sample_size: raw.sample_size,
            vertex_spacing: raw.vertex_spacing,
            oversampling: raw.oversampling,
            under_resolved: raw.under_resolved != 0,
        }
    }
}

/// A gesture into one channel: begin, stamp, commit, cancel.
///
/// Why a transaction and not a loop of stamps — three reasons, none of which
/// exists until a stack does:
///
/// - A stroke has to enter **one** layer, fixed at pointer-down rather than
///   read again per dab, or a host that changes the active layer mid-stroke
///   splits one gesture across two channels.
/// - A stamp **reads** the evaluated surface, so the composition is held for
///   the length of the stroke.
/// - Cancel has to be **exact**. A layered write is `L += dE`, so the only
///   exact restore is the recorded `before` values, which means the record has
///   to exist from the first stamp rather than be reconstructed at the end.
///
/// Under symmetry every mirrored stamp is another stamp in the same
/// transaction, so a mirrored stroke is one layer and one record whose
/// coverage is the union of the two sides.
///
/// The record itself does not cross this ABI: [`commit`](Self::commit) reports
/// how many entries it held so a host can see that the gesture coalesced, and
/// that is all. One layered gesture cannot yet become one entry in a host's
/// undo stack.
pub struct SculptLayerStroke<'s> {
    raw: NonNull<sys::clay_multires_sculpt_layer_stroke>,
    surface: &'s mut Multires,
    /// Whether a gesture is open, so [`Drop`] knows whether it has a
    /// composition hold to give back. Tracked here rather than asked of the
    /// engine because there is no entry point that answers it.
    open: bool,
}

impl SculptLayerStroke<'_> {
    /// The hierarchy this gesture writes into.
    pub fn surface(&self) -> &Multires {
        self.surface
    }

    /// The same, to read a block out between stamps.
    ///
    /// Every composition change reachable through it — strength, visibility,
    /// mask, order, add, remove — is refused by the engine while the gesture
    /// is open, and that refusal is deliberate rather than a limitation: a
    /// slider moved between two stamps would author one gesture against two
    /// different surfaces, and one that appeared to move and then silently
    /// applied at commit would be the worse surprise.
    ///
    /// Lent through a [`SurfaceMut`], and here that matters more than it does
    /// on the sculptor: this transaction's [`Drop`] cancels *into* the
    /// surface, so a hierarchy replaced through a `&mut Multires` would be
    /// read again on the way out of the scope that replaced it, with no
    /// further call by the caller at all.
    pub fn surface_mut(&mut self) -> SurfaceMut<'_> {
        SurfaceMut {
            surface: self.surface,
        }
    }

    /// Where the next [`begin`](Self::begin) will land the stroke.
    ///
    /// Set *before* begin. Changing it while a gesture is open does nothing:
    /// the domain is resolved once, which is the whole of the first reason a
    /// transaction exists.
    pub fn set_write_domain(&mut self, domain: WriteDomain) -> Result<()> {
        // SAFETY: valid handle and a plain integer the entry point
        // range-checks.
        check(
            unsafe {
                sys::clay_multires_sculpt_layer_stroke_set_write_domain(
                    self.raw.as_ptr(),
                    domain.to_raw(),
                )
            },
            "clay_multires_sculpt_layer_stroke_set_write_domain",
        )
    }

    /// Opens the gesture: fixes the target channel, holds the composition and
    /// clears the record.
    ///
    /// Refuses — changing nothing — on a gesture that is already open, on a
    /// locked target layer, and on [`WriteDomain::Detail`] with no active
    /// layer. The refusal names which, because those are three different
    /// sentences a host has to be able to say.
    pub fn begin(&mut self) -> std::result::Result<(), MultiresRefusal> {
        let mut reason = 0i32;
        // SAFETY: valid handle and an out-parameter written on every path.
        let code =
            unsafe { sys::clay_multires_sculpt_layer_stroke_begin(self.raw.as_ptr(), &mut reason) };
        refusable("clay_multires_sculpt_layer_stroke_begin", code, reason)?;
        self.open = true;
        Ok(())
    }

    /// The channel this gesture is writing. [`SculptLayerId::BASE`] is the
    /// base detail.
    pub fn target_layer(&self) -> Result<SculptLayerId> {
        let mut id = 0u64;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe {
                sys::clay_multires_sculpt_layer_stroke_target_layer(self.raw.as_ptr(), &mut id)
            },
            "clay_multires_sculpt_layer_stroke_target_layer",
        )?;
        Ok(SculptLayerId(id))
    }

    /// One stamp at the surface's sculpt level, into the target channel.
    ///
    /// The same sixteen verbs, the same falloffs, the same mask and the same
    /// automasking, because it is the same code. `mask` is the freeze.
    pub fn stamp(&mut self, stamp: MeshStamp<'_>, mask: Option<&MaskField>) -> Result<StampReport> {
        let desc = stamp.as_raw();
        self.report_of("clay_multires_sculpt_layer_stroke_stamp", |raw, report| {
            // SAFETY: valid handle; `desc` carries its own struct_size and
            // borrows any alpha samples from `stamp`, which outlives this
            // call; the mask is either a valid handle or null, both of which
            // the entry point allows; the report is a versioned
            // out-descriptor.
            unsafe {
                sys::clay_multires_sculpt_layer_stroke_stamp(raw, &desc, mask_ptr(mask), report)
            }
        })
    }

    /// A height or vector-displacement stamp, through the brush's own weight —
    /// so the falloff, the mask gate, the automasking and the alpha compose
    /// with it exactly as they do with a verb.
    ///
    /// Answers the oversampling reading beside the stamp's own report.
    ///
    /// A malformed image is **refused** rather than dropped, which is the
    /// opposite of what [`MeshStamp::alpha`] does with one — and deliberately:
    /// an alpha is a modulation, so losing it leaves the verb doing what it
    /// would have done anyway, while the image *is* this operation, and
    /// dropping it would report a successful stamp that moved nothing for a
    /// reason nobody was told. The check happens before the pointer is handed
    /// over, because the engine reads `channels * width * height` floats out
    /// of it whatever its own validation says about the dimensions.
    pub fn stamp_detail(
        &mut self,
        detail: DetailStamp<'_>,
        brush: MeshStamp<'_>,
        mask: Option<&MaskField>,
    ) -> Result<(DetailStampReport, StampReport)> {
        if !detail.is_well_formed() {
            return Err(raw_failure(
                "clay_multires_sculpt_layer_stroke_stamp_detail",
                ErrorKind::InvalidArgument,
            ));
        }
        let stamp_desc = detail.as_raw();
        let brush_desc = brush.as_raw();
        let mut resolution = sys::clay_detail_stamp_report::sized();
        let report = self.report_of(
            "clay_multires_sculpt_layer_stroke_stamp_detail",
            |raw, report| {
                // SAFETY: valid handle; both descriptors carry their own
                // struct_size and borrow from `detail` and `brush`, which
                // outlive this call; `detail.image` is `channels * width *
                // height` floats, checked above, which is what the entry point
                // reads; the mask is nullable; both reports are versioned
                // out-descriptors.
                unsafe {
                    sys::clay_multires_sculpt_layer_stroke_stamp_detail(
                        raw,
                        &stamp_desc,
                        &brush_desc,
                        mask_ptr(mask),
                        &mut resolution,
                        report,
                    )
                }
            },
        )?;
        Ok((DetailStampReport::from_raw(resolution), report))
    }

    /// Smooths at a stated frequency. See [`SmoothMode`] for why there are
    /// three of them rather than one filter with a cutoff.
    pub fn smooth(
        &mut self,
        mode: SmoothMode,
        brush: MeshStamp<'_>,
        mask: Option<&MaskField>,
    ) -> Result<StampReport> {
        let desc = brush.as_raw();
        self.report_of("clay_multires_sculpt_layer_stroke_smooth", |raw, report| {
            // SAFETY: as `stamp`, plus a mode the entry point range-checks.
            unsafe {
                sys::clay_multires_sculpt_layer_stroke_smooth(
                    raw,
                    mode.to_raw(),
                    &desc,
                    mask_ptr(mask),
                    report,
                )
            }
        })
    }

    /// The target channel toward zero.
    ///
    /// Touches neither the base nor any other layer, which is what makes it an
    /// eraser for *this* pass rather than a flattening brush.
    pub fn erase(&mut self, brush: MeshStamp<'_>, mask: Option<&MaskField>) -> Result<StampReport> {
        let desc = brush.as_raw();
        self.report_of("clay_multires_sculpt_layer_stroke_erase", |raw, report| {
            // SAFETY: as `stamp`.
            unsafe {
                sys::clay_multires_sculpt_layer_stroke_erase(raw, &desc, mask_ptr(mask), report)
            }
        })
    }

    /// The level's own detail toward zero: the form back toward the pure
    /// subdivision, with every layer left alone. Refused at level 0, where the
    /// cage has no pure subdivision to return to.
    ///
    /// Neither this nor [`erase`](Self::erase) is undo, and the difference is
    /// worth stating because a host will be tempted to wire one to the other:
    /// undo walks a step list backwards and restores what a gesture changed,
    /// wherever it was; these move the surface toward a named target under the
    /// cursor, and are themselves gestures.
    pub fn restore(
        &mut self,
        brush: MeshStamp<'_>,
        mask: Option<&MaskField>,
    ) -> Result<StampReport> {
        let desc = brush.as_raw();
        self.report_of(
            "clay_multires_sculpt_layer_stroke_restore",
            |raw, report| {
                // SAFETY: as `stamp`.
                unsafe {
                    sys::clay_multires_sculpt_layer_stroke_restore(
                        raw,
                        &desc,
                        mask_ptr(mask),
                        report,
                    )
                }
            },
        )
    }

    /// How many stamps this gesture has taken.
    ///
    /// Compare it against [`record_size`](Self::record_size): a hundred stamps
    /// over one vertex is *one* entry, because the record's size follows the
    /// vertices the stroke reached and not the stamps it took, and comparing
    /// the two is the only way to see that from outside.
    pub fn stamps(&self) -> Result<usize> {
        self.count_of("clay_multires_sculpt_layer_stroke_stamps", |raw, out| {
            // SAFETY: valid handle, out-parameter written on success.
            unsafe { sys::clay_multires_sculpt_layer_stroke_stamps(raw, out) }
        })
    }

    /// How many entries the record holds — the vertices this gesture reached.
    pub fn record_size(&self) -> Result<usize> {
        self.count_of(
            "clay_multires_sculpt_layer_stroke_record_size",
            |raw, out| {
                // SAFETY: valid handle, out-parameter written on success.
                unsafe { sys::clay_multires_sculpt_layer_stroke_record_size(raw, out) }
            },
        )
    }

    /// Closes the gesture: releases the composition hold and restores the
    /// stack's active layer.
    ///
    /// A gesture that changed nothing produces an empty record rather than a
    /// step, which is what the returned entry count says.
    pub fn commit(&mut self) -> Result<usize> {
        let mut entries = 0usize;
        // SAFETY: valid handle, out-parameter written on success.
        let code = unsafe {
            sys::clay_multires_sculpt_layer_stroke_commit(self.raw.as_ptr(), &mut entries)
        };
        // Cleared before the result is examined: a commit that refused still
        // reports whether the gesture is open, and leaving the flag set would
        // have `Drop` cancel a gesture the engine has already closed.
        self.open = false;
        check(code, "clay_multires_sculpt_layer_stroke_commit")?;
        Ok(entries)
    }

    /// Discards it.
    ///
    /// Restores the target channel **exactly** — the recorded `before` values,
    /// not a recomputation — and leaves the composition and the active layer
    /// as they were found.
    pub fn cancel(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        let code = unsafe { sys::clay_multires_sculpt_layer_stroke_cancel(self.raw.as_ptr()) };
        self.open = false;
        check(code, "clay_multires_sculpt_layer_stroke_cancel")
    }

    /// The three verbs' shared shape: a versioned report out, a stamp in.
    fn report_of(
        &mut self,
        operation: &'static str,
        call: impl FnOnce(
            *mut sys::clay_multires_sculpt_layer_stroke,
            *mut sys::clay_multires_stamp_report,
        ) -> RawResult,
    ) -> Result<StampReport> {
        let mut report = sys::clay_multires_stamp_report::sized();
        check(call(self.raw.as_ptr(), &mut report), operation)?;
        Ok(StampReport::from_raw(report))
    }

    /// The two counters' shared shape.
    fn count_of(
        &self,
        operation: &'static str,
        call: impl FnOnce(*const sys::clay_multires_sculpt_layer_stroke, *mut usize) -> RawResult,
    ) -> Result<usize> {
        let mut count = 0usize;
        check(call(self.raw.as_ptr(), &mut count), operation)?;
        Ok(count)
    }
}

/// A freeze, or nothing. Both are what the entry points accept.
fn mask_ptr(mask: Option<&MaskField>) -> *const sys::clay_mask {
    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _)
}

impl Drop for SculptLayerStroke<'_> {
    fn drop(&mut self) {
        // A gesture still open here is a caller's bug — a `?` that returned
        // between `begin` and `commit`, or a panic — and cancel is the only
        // outcome that leaves anything readable behind. Committing would bank
        // half a gesture, and destroying without either would leave the
        // composition held on a surface with no transaction left to release
        // it, which no later call can recover from. The result is discarded
        // because a `Drop` has nobody to report to.
        if self.open {
            // SAFETY: owned handle, still valid; cancel restores the recorded
            // `before` values and releases the hold.
            let _ = unsafe { sys::clay_multires_sculpt_layer_stroke_cancel(self.raw.as_ptr()) };
        }
        // SAFETY: owned handle, released exactly once, and before the borrow
        // of the surface it points at ends. Nothing reachable from that borrow
        // yields a `&mut Multires` (see `SurfaceMut`), so neither the cancel
        // above nor this destroy can be writing into a surface that was
        // replaced while the gesture was open.
        unsafe { sys::clay_multires_sculpt_layer_stroke_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for SculptLayerStroke<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SculptLayerStroke")
            .field("open", &self.open)
            .field("target", &self.target_layer().ok())
            .field("stamps", &self.stamps().ok())
            .finish()
    }
}

// -- a hierarchy over a layer the document already holds ---------------------

impl Document {
    /// Builds a hierarchy over one of this document's own mesh layers.
    ///
    /// Fused for the reason [`Document::rasterize_into_voxel_layer`] is fused:
    /// the cage is a mesh the *document* owns and lends out, and Rust will not
    /// let a borrow of it and a borrow of the document stand at once even
    /// though the C boundary takes the mesh as `const` and copies it. So the
    /// two pointers meet here, in the crate where `unsafe` lives, rather than
    /// forcing the caller to copy a mesh it already has.
    ///
    /// The hierarchy that comes back **owns a copy** of the cage and has no
    /// further connection to the layer: editing the layer's triangles
    /// afterwards does not reach it, and the hierarchy is not written by
    /// `clay_document_save`. That ownership boundary is the whole integration
    /// cost of this tier and it is deliberate upstream — see the module doc.
    ///
    /// Refuses rather than repairs, exactly as [`Multires::from_mesh`] does.
    pub fn multires_from_mesh_layer(
        &mut self,
        layer: LayerId,
        desc: MultiresDesc,
    ) -> std::result::Result<Multires, MultiresRefusal> {
        let mut mesh = std::ptr::null_mut();
        // SAFETY: a valid document and one out-parameter written only on
        // success. The handle that comes back is the layer's own and is
        // borrowed for the length of this call; it must not be wrapped in
        // `Mesh`, which destroys what it holds on drop.
        let found = check(
            unsafe { sys::clay_document_mesh_layer_by_id(self.as_ptr(), layer.0, &mut mesh) },
            "clay_document_mesh_layer_by_id",
        );
        if let Err(error) = found {
            return Err(MultiresRefusal {
                error,
                reason: MultiresError::EmptyBase,
            });
        }
        if mesh.is_null() {
            return Err(MultiresRefusal {
                error: raw_failure("clay_document_mesh_layer_by_id", ErrorKind::NotFound),
                reason: MultiresError::EmptyBase,
            });
        }

        let raw_desc = desc.to_raw();
        let mut surface = std::ptr::null_mut();
        let mut reason = 0i32;
        // SAFETY: the mesh handle was just written by a successful call and
        // belongs to this document, the descriptor carries its own
        // struct_size, and both out-parameters are written on every path —
        // `reason` including success, where it is CLAY_MULTIRES_OK.
        let code =
            unsafe { sys::clay_multires_from_mesh(mesh, &raw_desc, &mut surface, &mut reason) };
        let reason = MultiresError::from_raw(reason);
        check(code, "clay_multires_from_mesh")
            .map_err(|error| MultiresRefusal { error, reason })?;
        NonNull::new(surface)
            .map(|raw| Multires { raw })
            .ok_or_else(|| MultiresRefusal {
                error: raw_failure("clay_multires_from_mesh", ErrorKind::Backend),
                reason,
            })
    }
}

#[cfg(test)]
mod tests {
    /// The ratchet under [`SurfaceMut`].
    ///
    /// A hierarchy is an owning wrapper with a destructor, and `&mut` to one
    /// is not a permission to *use* it — it is a permission to overwrite it,
    /// which runs `clay_multires_destroy`. So a `&mut Multires` handed to safe
    /// code by anything holding a bare pointer into that surface is a
    /// use-after-free with no `unsafe` at the call site: measured on the
    /// pinned engine, `*sculptor.surface_mut() = another;` then one stamp is a
    /// SIGSEGV, and for a stroke the cancel in `Drop` reaches the freed
    /// surface with no further call at all.
    ///
    /// The repair is a type — [`super::SurfaceMut`] — and the repair is only
    /// as durable as the next person who writes an accessor. This reads the
    /// module's own source and fails if any signature in it returns a mutable
    /// reference to a hierarchy again, which is the shape the defect takes
    /// whatever it is called.
    ///
    /// Skipped rather than failed where the source is not on disk, for the
    /// reason the error table's ratchet is: a build from a package has the
    /// compiled crate and not the file it came from.
    #[test]
    fn nothing_lends_a_hierarchy_that_could_be_replaced_through_the_borrow() {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/multires.rs");
        let Ok(text) = std::fs::read_to_string(&source) else {
            return;
        };
        let lent: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("pub fn ") || line.starts_with("fn "))
            .filter(|line| line.contains("-> &mut Multires") || line.contains("mut Multires>"))
            .collect();
        assert!(
            lent.is_empty(),
            "a hierarchy is lent as `&mut Multires`, which is `clay_multires_destroy` \
             reachable from safe code while a sculptor or a stroke still points into it — \
             lend it through `SurfaceMut` instead: {lent:?}"
        );
    }
}
