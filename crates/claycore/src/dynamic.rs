//! Adaptive topology: a surface whose *connectivity* changes under the brush.
//!
//! The third representation beside the fixed mesh of [`mesh_sculpt`] and the
//! hierarchy of [`multires`], and never a mode either of them slips into.
//! Geometry is created where a stroke needs it and removed where it does not,
//! which is what lets a sculptor pull a horn out of a sphere without having
//! decided beforehand how many triangles a horn costs.
//!
//! What that buys is paid for in two places a host has to know about.
//!
//! # A dynamic surface is triangles, and the round trip says so
//!
//! Converting a quad mesh to one of these and back gives triangles with
//! `quads` empty. No quad pairing is re-derived, because none survived the
//! split. A quad workflow does not pass through this representation, and
//! [`DynamicSurface::to_mesh`] is where a caller finds that out — stated here
//! so it is not discovered by exporting one.
//!
//! # Nothing borrows into the surface
//!
//! A stamp can move or free anything the half-edge structure holds, so a
//! pointer a caller kept across one would be a use-after-free with no
//! generation to check it against. Every read here therefore *copies*:
//! [`DynamicSculptor::copy_chunk`] fills memory the caller owns, and the
//! surface hands out no slices at all. That is the engine's own rule rather
//! than this wrapper being cautious, and it is why the chunk transport exists
//! in the shape it does.
//!
//! # The chunk transport is the point, not a detail
//!
//! A host that re-uploads the whole model per dab cannot use this
//! representation at the sizes it exists for. So the surface is partitioned
//! into chunks, each carrying its own revision, and a stamp reports which ones
//! it touched: [`DynamicSculptor::dirty_chunks`] then
//! [`DynamicSculptor::copy_chunk_into`] is one frame's upload, and
//! [`DynamicSculptor::clear_dirty`] retires the set. The three
//! [`SurfaceRevision`] counters are the coarse form of the same question —
//! `topology` moving is an index buffer that has stopped describing the
//! surface, `geometry` moving is vertex data that has to be sent again.
//!
//! [`mesh_sculpt`]: crate::MeshSculptor
//! [`multires`]: crate::Multires

use std::ptr::NonNull;

use claycore_sys as sys;

use crate::buffer::{size_query_array, size_query_bytes, size_query_string};
use crate::descriptor::Descriptor;
use crate::error::{check, ClayError, ErrorKind, Result};
use crate::maintenance::MaintenanceQueue;
use crate::mask::MaskField;
use crate::memory::{
    preflight, MemoryLedger, MemoryPin, Pressure, SculptMemoryProfile, SurfacePreflight, TrimReport,
};
use crate::mesh::Mesh;
use crate::mesh_sculpt::MeshStamp;
use crate::multires::{ArenaStats, PeakTelemetry};
use crate::raw_failure;

// -- refusals ---------------------------------------------------------------

/// Why a conversion to an adaptive surface was refused.
///
/// Mirrors `clay_dynamic_build_error`. Distinct from [`ClayError`] for the
/// reason [`MultiresError`](crate::MultiresError) is: the result code says an
/// argument was rejected, and this says *which model problem* rejected it —
/// "three faces meet on one edge" is a sentence a user can act on and "invalid
/// argument" is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicError {
    /// No reason from the conversion itself: nothing was refused.
    None,
    /// The mesh has no faces.
    EmptyMesh,
    IndexOutOfRange,
    /// A triangle with repeated or collinear corners.
    DegenerateTriangle,
    /// Three or more faces on one edge. A half-edge surface cannot express it,
    /// and dropping the third face silently would be a conversion that changes
    /// the model without saying so.
    NonManifoldEdge,
    /// A value this build does not know, carried verbatim rather than mapped
    /// onto one it does.
    Unknown(i32),
}

impl DynamicError {
    fn from_raw(code: i32) -> Self {
        use sys::clay_dynamic_build_error as e;
        match code as sys::clay_dynamic_build_error::Type {
            e::CLAY_DYNAMIC_OK => Self::None,
            e::CLAY_DYNAMIC_EMPTY_MESH => Self::EmptyMesh,
            e::CLAY_DYNAMIC_INDEX_OUT_OF_RANGE => Self::IndexOutOfRange,
            e::CLAY_DYNAMIC_DEGENERATE_TRIANGLE => Self::DegenerateTriangle,
            e::CLAY_DYNAMIC_NON_MANIFOLD_EDGE => Self::NonManifoldEdge,
            _ => Self::Unknown(code),
        }
    }

    /// A sentence for this refusal.
    ///
    /// Written here rather than read from the engine, which is the one place
    /// this type differs from [`MultiresError`](crate::MultiresError): the ABI
    /// has a `clay_multires_error_text` and no `clay_dynamic_build_error_text`,
    /// so there is no static table to defer to. If one is ever added these
    /// strings should be retired in favour of it rather than kept beside it.
    pub fn text(self) -> &'static str {
        match self {
            Self::None => "ok",
            Self::EmptyMesh => "the mesh has no faces",
            Self::IndexOutOfRange => "a triangle names a vertex that does not exist",
            Self::DegenerateTriangle => "a triangle has repeated or collinear corners",
            Self::NonManifoldEdge => "three or more faces meet on one edge",
            Self::Unknown(_) => "unknown",
        }
    }
}

impl std::fmt::Display for DynamicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.text())
    }
}

/// A refused conversion: the result code, and which model problem caused it.
///
/// Both halves are carried because they answer different questions. A host
/// logging a failure wants the result code and the engine's detail message; a
/// host putting a sentence in front of a sculptor wants to know that the mesh
/// is not manifold rather than that an argument was invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicRefusal {
    pub error: ClayError,
    pub reason: DynamicError,
}

impl std::fmt::Display for DynamicRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.reason {
            DynamicError::None => self.error.fmt(f),
            reason => write!(f, "{}: {reason}", self.error.operation()),
        }
    }
}

impl std::error::Error for DynamicRefusal {}

// -- building one -----------------------------------------------------------

/// How a flat mesh is read into a half-edge surface.
///
/// Both tolerances take the engine's own default when left unset, which is
/// what `clay_dynamic_surface_defaults` answers and what a zero in the
/// descriptor means.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DynamicDesc {
    /// Vertices closer together than this are **one** geometric vertex — the
    /// same rule the fixed adjacency uses, and what lets a brush cross a UV
    /// seam without opening a crack.
    pub weld_epsilon: Option<f32>,
    /// Corners disagreeing by more than this across an edge mark it a UV seam,
    /// so the seam survives remeshing as an edge property rather than as a
    /// crack in the texture.
    pub uv_seam_epsilon: Option<f32>,
}

impl DynamicDesc {
    fn to_raw(self) -> sys::clay_dynamic_surface_desc {
        // The engine's defaults first, then what this descriptor means — the
        // arrangement every `*_defaults` entry point exists for. A failure to
        // read them is not fatal: the zeroed descriptor still carries a valid
        // struct_size, and both fields document zero as "take the library's
        // own".
        let mut raw = sys::clay_dynamic_surface_desc::sized();
        // SAFETY: a valid versioned descriptor out-parameter whose
        // struct_size is set above, which is what the boundary requires.
        let _ = unsafe { sys::clay_dynamic_surface_defaults(&mut raw) };
        if let Some(epsilon) = self.weld_epsilon {
            raw.weld_epsilon = epsilon;
        }
        if let Some(epsilon) = self.uv_seam_epsilon {
            raw.uv_seam_epsilon = epsilon;
        }
        raw
    }
}

// -- what the surface reports about itself ----------------------------------

/// The half-edge structure's own census.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DynamicStats {
    pub vertices: u64,
    pub edges: u64,
    pub halfedges: u64,
    pub faces: u64,
    pub boundary_edges: u64,
    /// Slots allocated and not live.
    ///
    /// A surface never compacts, so this is what a caller watches to decide
    /// whether a round trip through [`DynamicSurface::to_mesh`] is worth its
    /// cost — a long session of splits and collapses leaves holes that nothing
    /// else reclaims.
    pub dead_slots: u64,
    pub bytes: u64,
}

impl DynamicStats {
    fn from_raw(raw: sys::clay_dynamic_surface_stats) -> Self {
        Self {
            vertices: raw.vertices,
            edges: raw.edges,
            halfedges: raw.halfedges,
            faces: raw.faces,
            boundary_edges: raw.boundary_edges,
            dead_slots: raw.dead_slots,
            bytes: raw.bytes,
        }
    }
}

/// Three revisions, not one.
///
/// Topology, geometry and attributes advance independently, so a host
/// re-uploads an index buffer only when connectivity changed and vertex data
/// only when it moved. The same distinction serves cache invalidation for
/// anything derived from the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SurfaceRevision {
    /// Moves when connectivity changed — a split, a collapse or a flip.
    pub topology: u64,
    /// Moves when a vertex moved.
    pub geometry: u64,
    /// Moves when a corner attribute changed.
    pub attributes: u64,
}

impl SurfaceRevision {
    fn from_raw(raw: sys::clay_surface_revision) -> Self {
        Self {
            topology: raw.topology,
            geometry: raw.geometry,
            attributes: raw.attributes,
        }
    }
}

/// Every invariant of the half-edge structure, checked.
///
/// Cheap enough for a debug build after every edit and far too slow for a
/// release inner loop, which is why the ABI makes it a call rather than an
/// assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceValidation {
    pub ok: bool,
    /// The engine's own summary, sound or not. Kept on the sound path too:
    /// a message a caller only sees when something is wrong is a message
    /// nobody has read the day it matters.
    pub message: String,
}

// -- the topology policy ----------------------------------------------------

/// What `target_edge_length` is measured against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DetailMode {
    /// `target_edge_length` in world units, so detail is the same size
    /// everywhere whatever the brush is doing.
    #[default]
    World,
    /// `radius / detail_resolution`, so a small brush makes small triangles.
    BrushRelative,
    /// No adaptation at all: deformation only, and the connectivity that went
    /// in comes out.
    Constant,
}

impl DetailMode {
    pub const ALL: [DetailMode; 3] = [Self::World, Self::BrushRelative, Self::Constant];

    fn to_raw(self) -> i32 {
        (match self {
            Self::World => sys::clay_dynamic_detail_mode::CLAY_DETAIL_WORLD,
            Self::BrushRelative => sys::clay_dynamic_detail_mode::CLAY_DETAIL_BRUSH_RELATIVE,
            Self::Constant => sys::clay_dynamic_detail_mode::CLAY_DETAIL_CONSTANT,
        }) as i32
    }

    fn from_raw(code: i32) -> Self {
        use sys::clay_dynamic_detail_mode as m;
        match code as sys::clay_dynamic_detail_mode::Type {
            m::CLAY_DETAIL_BRUSH_RELATIVE => Self::BrushRelative,
            m::CLAY_DETAIL_CONSTANT => Self::Constant,
            // The engine refuses a mode outside its own list rather than
            // clamping, so anything that arrives here is World.
            _ => Self::World,
        }
    }
}

/// How much connectivity a stamp is allowed to change, and in which direction.
///
/// Every field is a concrete value rather than an `Option`, and
/// [`DynamicTopology::default`] reads the engine's own defaults through
/// `clay_dynamic_topology_defaults`. Sixteen `Option`s would say the same
/// thing at four times the length, and this way a host that wants to *show*
/// the policy has numbers to show rather than a column of "engine default".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicTopology {
    /// Off is a pure deformation: vertices move and no edge is touched.
    pub enabled: bool,
    pub detail_mode: DetailMode,
    pub target_edge_length: f32,
    pub detail_resolution: f32,
    /// Split above `target * split_factor`.
    ///
    /// The gap between this and [`collapse_factor`](Self::collapse_factor) is
    /// **hysteresis**: with one threshold an edge just above it splits into two
    /// just below it and they collapse back, for as long as the brush is held
    /// still.
    pub split_factor: f32,
    /// Collapse below `target * collapse_factor`.
    pub collapse_factor: f32,
    pub max_passes: i32,
    /// A bound, and a parameter rather than a constant, so a host can trade
    /// detail for latency on a slower device. When it stops a pass the report
    /// says so through [`DynamicStampReport::hit_budget`].
    pub max_ops_per_stamp: i32,
    pub allow_split: bool,
    pub allow_collapse: bool,
    pub allow_flip: bool,
    pub relax_after_remesh: bool,
    pub relax_strength: f32,
    pub preserve_boundaries: bool,
    pub preserve_uv_seams: bool,
    pub preserve_sharp_edges: bool,
}

impl Default for DynamicTopology {
    fn default() -> Self {
        let mut raw = sys::clay_dynamic_topology_desc::sized();
        // SAFETY: a valid versioned descriptor out-parameter whose
        // struct_size is set above. A failure leaves the zeroed descriptor,
        // which reads back as a policy that changes nothing — the safe
        // outcome of the two.
        let _ = unsafe { sys::clay_dynamic_topology_defaults(&mut raw) };
        Self {
            enabled: raw.enabled != 0,
            detail_mode: DetailMode::from_raw(raw.detail_mode),
            target_edge_length: raw.target_edge_length,
            detail_resolution: raw.detail_resolution,
            split_factor: raw.split_factor,
            collapse_factor: raw.collapse_factor,
            max_passes: raw.max_passes,
            max_ops_per_stamp: raw.max_ops_per_stamp,
            allow_split: raw.allow_split != 0,
            allow_collapse: raw.allow_collapse != 0,
            allow_flip: raw.allow_flip != 0,
            relax_after_remesh: raw.relax_after_remesh != 0,
            relax_strength: raw.relax_strength,
            preserve_boundaries: raw.preserve_boundaries != 0,
            preserve_uv_seams: raw.preserve_uv_seams != 0,
            preserve_sharp_edges: raw.preserve_sharp_edges != 0,
        }
    }
}

impl DynamicTopology {
    fn to_raw(self) -> sys::clay_dynamic_topology_desc {
        let mut raw = sys::clay_dynamic_topology_desc::sized();
        raw.enabled = i32::from(self.enabled);
        raw.detail_mode = self.detail_mode.to_raw();
        raw.target_edge_length = self.target_edge_length;
        raw.detail_resolution = self.detail_resolution;
        raw.split_factor = self.split_factor;
        raw.collapse_factor = self.collapse_factor;
        raw.max_passes = self.max_passes;
        raw.max_ops_per_stamp = self.max_ops_per_stamp;
        raw.allow_split = i32::from(self.allow_split);
        raw.allow_collapse = i32::from(self.allow_collapse);
        raw.allow_flip = i32::from(self.allow_flip);
        raw.relax_after_remesh = i32::from(self.relax_after_remesh);
        raw.relax_strength = self.relax_strength;
        raw.preserve_boundaries = i32::from(self.preserve_boundaries);
        raw.preserve_uv_seams = i32::from(self.preserve_uv_seams);
        raw.preserve_sharp_edges = i32::from(self.preserve_sharp_edges);
        raw
    }
}

/// What one stamp did.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DynamicStampReport {
    pub moved_vertices: u64,
    pub split_edges: u64,
    pub collapsed_edges: u64,
    pub flipped_edges: u64,
    pub relaxed_vertices: u64,
    /// Whether the operation budget stopped the pass, so a host can tell "the
    /// region converged" from "it ran out of budget" — two outcomes that look
    /// identical in the counters above.
    pub hit_budget: bool,
    /// The box the stamp changed, for a host invalidating by region.
    pub dirty_min: [f32; 3],
    pub dirty_max: [f32; 3],
    /// The three counters as they stood afterwards.
    pub revision: SurfaceRevision,
}

impl DynamicStampReport {
    fn from_raw(raw: sys::clay_dynamic_stamp_report) -> Self {
        Self {
            moved_vertices: raw.moved_vertices,
            split_edges: raw.split_edges,
            collapsed_edges: raw.collapsed_edges,
            flipped_edges: raw.flipped_edges,
            relaxed_vertices: raw.relaxed_vertices,
            hit_budget: raw.hit_budget != 0,
            dirty_min: raw.dirty_min,
            dirty_max: raw.dirty_max,
            revision: SurfaceRevision::from_raw(raw.revision),
        }
    }
}

// -- where a session is -----------------------------------------------------

/// The placement a sculpting session declares once.
///
/// A surface's vertices are **layer-local** and something has to place them
/// against the world-addressed lattices a brush consults — the painted mask
/// above all. Unset is the identity, so a host that has never heard of this
/// is not opted in and nothing it already does changes. Set, every position,
/// radius and direction crossing the handle is *world*.
///
/// There is deliberately no "use the layer's transform" here, and that is a
/// fact about the ABI rather than an omission: a dynamic surface is not a
/// document layer and there is no transform to read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldFrame {
    pub position: [f32; 3],
    /// Quaternion, `x y z w`. Normalized by the engine on the way in, so a
    /// read-back is the normalized form rather than what was written.
    pub rotation: [f32; 4],
    /// Uniform, and strictly positive — a per-axis scale is refused rather
    /// than approximated, because under one a round brush in world is an
    /// ellipsoid on the model and `radius` stops naming anything a spherical
    /// walk can honour.
    pub scale: f32,
}

impl Default for WorldFrame {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: 1.0,
        }
    }
}

impl WorldFrame {
    fn to_raw(self) -> sys::clay_mesh_frame {
        let mut raw = sys::clay_mesh_frame::sized();
        raw.position = self.position;
        raw.rotation = self.rotation;
        raw.scale = self.scale;
        raw
    }

    fn from_raw(raw: sys::clay_mesh_frame) -> Self {
        Self {
            position: raw.position,
            rotation: raw.rotation,
            scale: raw.scale,
        }
    }
}

// -- the chunk transport ----------------------------------------------------

/// One chunk's shape, before anything is copied.
///
/// Named apart from [`ChunkInfo`](crate::ChunkInfo), which belongs to the
/// read-only [`SurfaceView`](crate::SurfaceView) seam over any representation.
/// This one is the adaptive surface's own, reported by the sculptor that owns
/// the chunked index.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DynamicChunkInfo {
    pub index: u32,
    pub revision: u64,
    /// How many `[f32; 3]` the chunk needs.
    ///
    /// An adaptive surface's chunks copy as **unwelded triangles**: its
    /// topology changes under the stamp being uploaded, so there is no stable
    /// per-chunk vertex list to weld against. Read this rather than deriving
    /// it from the index count.
    pub vertex_count: u32,
    /// How many `u32` the chunk needs.
    pub index_count: u32,
    pub geometry_dirty: bool,
    pub topology_dirty: bool,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

impl DynamicChunkInfo {
    fn from_raw(raw: sys::clay_dynamic_chunk_info) -> Self {
        Self {
            index: raw.index,
            revision: raw.revision,
            vertex_count: raw.vertex_count,
            index_count: raw.index_count,
            geometry_dirty: raw.geometry_dirty != 0,
            topology_dirty: raw.topology_dirty != 0,
            bounds_min: raw.bounds_min,
            bounds_max: raw.bounds_max,
        }
    }
}

/// One chunk, copied out of the surface.
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicChunk {
    /// What was actually written, which is what a host uploads by rather than
    /// what it asked for.
    pub info: DynamicChunkInfo,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// **Local to the chunk**, so a host uploads it as a standalone draw.
    pub indices: Vec<u32>,
}

// -- what the index is worth ------------------------------------------------

/// The measurement behind a queued index rebuild, so a host declining the item
/// is declining something it can see rather than a name.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IndexQuality {
    pub leaf_count: u64,
    /// Mean leaf volume against the volume of their union. **Lower is
    /// better**: 1 is a perfect partition and larger means the leaves overlap,
    /// which is what local edits do to a tree over time. Meaningful only
    /// against this same tree's own history, never against another model's.
    ///
    /// A surface with no volume reports 0, and that follows from the measure
    /// rather than from a bug — a flat sheet's leaves and their union are both
    /// zero-volume boxes. Any model with thickness reports a real figure.
    pub quality: f32,
    /// The engine's opinion of its own partition, and **not an instruction**.
    /// It is one of the two conditions
    /// [`DynamicSculptor::request_index_rebuild`] checks; the other is the
    /// host's [`SculptMemoryProfile::allow_index_rebuild`].
    pub wants_rebuild: bool,
}

impl IndexQuality {
    fn from_raw(raw: sys::clay_index_quality) -> Self {
        Self {
            leaf_count: raw.leaf_count,
            quality: raw.quality,
            wants_rebuild: raw.wants_rebuild != 0,
        }
    }
}

// -- where a stamp's time goes ----------------------------------------------

/// One phase of a stamp.
///
/// Defined here rather than beside the fixed sculptor because the adaptive
/// surface is the representation whose per-dab cost is hardest to predict — it
/// splits, collapses and flips as it goes, so the same brush at the same
/// radius is not the same work twice running, and [`Topology`](Self::Topology)
/// is the stage only this one fills. The engine fills the same report for the
/// fixed mesh, so this type is where that wrapper should reach when it arrives
/// rather than a second transcription of the same table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SculptStage {
    /// Where the dab landed.
    SeedResolve,
    /// The region walk.
    SpatialQuery,
    Weight,
    Alpha,
    Automask,
    Snapshot,
    NeighborBuild,
    /// The verb itself.
    Kernel,
    Writeback,
    NormalRefresh,
    /// Adaptive surfaces only; zero elsewhere.
    Topology,
    ChunkMark,
    BvhUpdate,
    /// A stage a newer engine filled that this build has no name for, carried
    /// by its index rather than dropped.
    Unknown(u32),
}

impl SculptStage {
    fn from_index(index: u32) -> Self {
        use sys::clay_sculpt_stage as s;
        match index as sys::clay_sculpt_stage::Type {
            s::CLAY_SCULPT_STAGE_SEED_RESOLVE => Self::SeedResolve,
            s::CLAY_SCULPT_STAGE_SPATIAL_QUERY => Self::SpatialQuery,
            s::CLAY_SCULPT_STAGE_WEIGHT => Self::Weight,
            s::CLAY_SCULPT_STAGE_ALPHA => Self::Alpha,
            s::CLAY_SCULPT_STAGE_AUTOMASK => Self::Automask,
            s::CLAY_SCULPT_STAGE_SNAPSHOT => Self::Snapshot,
            s::CLAY_SCULPT_STAGE_NEIGHBOR_BUILD => Self::NeighborBuild,
            s::CLAY_SCULPT_STAGE_KERNEL => Self::Kernel,
            s::CLAY_SCULPT_STAGE_WRITEBACK => Self::Writeback,
            s::CLAY_SCULPT_STAGE_NORMAL_REFRESH => Self::NormalRefresh,
            s::CLAY_SCULPT_STAGE_TOPOLOGY => Self::Topology,
            s::CLAY_SCULPT_STAGE_CHUNK_MARK => Self::ChunkMark,
            s::CLAY_SCULPT_STAGE_BVH_UPDATE => Self::BvhUpdate,
            _ => Self::Unknown(index),
        }
    }
}

/// What one stage cost, and how often it ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StageTiming {
    pub nanos: u64,
    pub calls: u64,
}

/// Where a session's stamps spent their time, and what they did.
///
/// Off by default; [`DynamicSculptor::set_stage_report_enabled`] starts it.
/// Reading it while nothing was enabled answers zeroes, which is the honest
/// figure rather than a refusal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StageReport {
    /// One entry per stage **this engine filled**, which may be fewer than
    /// this build's header declares if the library is older. Trailing zeroes a
    /// caller cannot tell from measured ones are the failure this shape
    /// avoids.
    pub stages: Vec<(SculptStage, StageTiming)>,
    /// Everything the brush reached, including the rim of the falloff where
    /// the weight is zero and everything a mask held still.
    pub vertices_considered: u64,
    /// The ones it actually moved. The gap between the two is the first thing
    /// to look at when a dab costs more than it should.
    pub vertices_affected: u64,
    /// Class positions measured while resolving where the dab landed. The
    /// number that says "a dab costs what it touches" is still true; it is not
    /// a time and no machine changes it.
    pub positions_measured: u64,
    pub faces_touched: u64,
    pub chunks_touched: u64,
    pub neighbors_gathered: u64,
    pub kernel_passes: u64,
    pub splits: u64,
    pub collapses: u64,
    pub flips: u64,
    pub detail_blocks_touched: u64,
    pub history_bytes: u64,
    pub scratch_high_water: u64,
}

impl StageReport {
    fn from_raw(raw: sys::clay_sculpt_stage_report) -> Self {
        let filled = (raw.stage_count as usize).min(raw.nanos.len());
        Self {
            stages: (0..filled)
                .map(|i| {
                    (
                        SculptStage::from_index(i as u32),
                        StageTiming {
                            nanos: raw.nanos[i],
                            calls: raw.calls[i],
                        },
                    )
                })
                .collect(),
            vertices_considered: raw.vertices_considered,
            vertices_affected: raw.vertices_affected,
            positions_measured: raw.positions_measured,
            faces_touched: raw.faces_touched,
            chunks_touched: raw.chunks_touched,
            neighbors_gathered: raw.neighbors_gathered,
            kernel_passes: raw.kernel_passes,
            splits: raw.splits,
            collapses: raw.collapses,
            flips: raw.flips,
            detail_blocks_touched: raw.detail_blocks_touched,
            history_bytes: raw.history_bytes,
            scratch_high_water: raw.scratch_high_water,
        }
    }

    /// The total across every stage the engine filled.
    pub fn total_nanos(&self) -> u64 {
        self.stages.iter().map(|(_, timing)| timing.nanos).sum()
    }
}

// -- the surface ------------------------------------------------------------

/// A half-edge surface whose connectivity changes under the brush.
///
/// Owns its engine handle and destroys it on drop. It is held *beside* a
/// document rather than inside one — there is no document layer behind it, and
/// [`serialize`](Self::serialize) hands back the bytes to store with one.
pub struct DynamicSurface {
    raw: NonNull<sys::clay_dynamic_surface>,
}

// SAFETY: the handle is an owning pointer into the engine's own allocation and
// this type hands out no interior pointers, so moving one between threads
// moves the whole surface with it. The engine's contract is one handle, one
// thread at a time, which `&mut` for every mutation already enforces — hence
// `Send` and no `Sync`.
unsafe impl Send for DynamicSurface {}

impl DynamicSurface {
    /// Builds one from a flat mesh, which is **read and not retained**.
    ///
    /// This refuses rather than repairs: a conversion that quietly drops the
    /// third face on an edge changes the model without saying so, so the
    /// problems a half-edge structure cannot express come back as a
    /// [`DynamicRefusal`] naming which.
    ///
    /// [`Mesh::preflight_to_dynamic`] prices the call before it is made. The
    /// peak matters more than the result's size here: the source mesh, the
    /// half-edge structure and the weld map are all live at once.
    pub fn from_mesh(mesh: &Mesh, desc: DynamicDesc) -> std::result::Result<Self, DynamicRefusal> {
        let raw_desc = desc.to_raw();
        let mut surface = std::ptr::null_mut();
        let mut reason = 0i32;
        // SAFETY: a valid mesh handle read but not retained, a descriptor
        // carrying its own struct_size, and two out-parameters. `reason` is
        // written on every path, including success, where the engine sets it
        // to CLAY_DYNAMIC_OK.
        let code = unsafe {
            sys::clay_dynamic_surface_from_mesh(mesh.as_ptr(), &raw_desc, &mut surface, &mut reason)
        };
        let reason = DynamicError::from_raw(reason);
        check(code, "clay_dynamic_surface_from_mesh")
            .map_err(|error| DynamicRefusal { error, reason })?;
        NonNull::new(surface)
            .map(|raw| Self { raw })
            .ok_or_else(|| DynamicRefusal {
                error: raw_failure("clay_dynamic_surface_from_mesh", ErrorKind::Backend),
                reason,
            })
    }

    /// Reconstructs one from [`serialize`](Self::serialize)'s bytes.
    ///
    /// A versioned format of its own: a flat mesh's encoding cannot express a
    /// half-edge structure. Generations are preserved, so a chunk index taken
    /// before a save still resolves after a load.
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        let mut surface = std::ptr::null_mut();
        // SAFETY: `bytes` is valid for reads of `bytes.len()`, which is the
        // length passed; the buffer is read and not retained, and the
        // out-parameter is written only on success.
        check(
            unsafe {
                sys::clay_dynamic_surface_deserialize(bytes.as_ptr(), bytes.len(), &mut surface)
            },
            "clay_dynamic_surface_deserialize",
        )?;
        NonNull::new(surface)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_dynamic_surface_deserialize", ErrorKind::Backend))
    }

    /// A fresh flat mesh, which the caller owns.
    ///
    /// **Triangles**, with `quads` empty, whatever went in — see the module's
    /// own note. The export splits a geometric vertex into as many export
    /// vertices as it has distinct corner attributes, so the result is bounded
    /// by *corners* rather than by vertices, which is the term that makes this
    /// bigger than it looks on a seam-heavy model.
    /// [`preflight_to_mesh`](Self::preflight_to_mesh) is that arithmetic done
    /// before it is paid for.
    pub fn to_mesh(&self) -> Result<Mesh> {
        let mut mesh = std::ptr::null_mut();
        // SAFETY: a valid surface handle the call only reads, and an
        // out-parameter written only on success; the mesh it yields is a fresh
        // allocation this crate takes ownership of.
        check(
            unsafe { sys::clay_dynamic_surface_to_mesh(self.raw.as_ptr(), &mut mesh) },
            "clay_dynamic_surface_to_mesh",
        )?;
        Mesh::from_raw(mesh, "clay_dynamic_surface_to_mesh")
    }

    /// The half-edge structure's census.
    pub fn stats(&self) -> Result<DynamicStats> {
        let mut raw = sys::clay_dynamic_surface_stats::sized();
        // SAFETY: valid handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_dynamic_surface_stats_get(self.raw.as_ptr(), &mut raw) },
            "clay_dynamic_surface_stats_get",
        )?;
        Ok(DynamicStats::from_raw(raw))
    }

    /// The three counters a host compares to decide what to upload again.
    pub fn revision(&self) -> Result<SurfaceRevision> {
        let mut raw = sys::clay_surface_revision::sized();
        // SAFETY: valid handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_dynamic_surface_revision(self.raw.as_ptr(), &mut raw) },
            "clay_dynamic_surface_revision",
        )?;
        Ok(SurfaceRevision::from_raw(raw))
    }

    /// Every invariant of the structure, checked.
    pub fn validate(&self) -> Result<SurfaceValidation> {
        let handle = self.raw.as_ptr();
        let mut ok = 0i32;
        let message = size_query_string("clay_dynamic_surface_validate", |buf, len| {
            // SAFETY: the buffer protocol's two calls. `buf` is either null —
            // asking for the size — or valid for writes of `*len` bytes, which
            // is what the engine is told it has and checks before copying.
            // `ok` is a valid `i32` out-parameter for the whole of the borrow.
            unsafe { sys::clay_dynamic_surface_validate(handle, &mut ok, buf, len) }
        })?;
        Ok(SurfaceValidation {
            ok: ok != 0,
            message,
        })
    }

    /// The bytes to store beside the document.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let handle = self.raw.as_ptr();
        size_query_bytes("clay_dynamic_surface_serialize", |buf, size| {
            // SAFETY: the buffer protocol's two calls. `buf` is either null —
            // asking for the size — or valid for writes of `*size` bytes,
            // which is what the engine is told it has and what it checks
            // before it copies. `u8` and `c_char` have the same size and
            // alignment.
            unsafe { sys::clay_dynamic_surface_serialize(handle, buf as *mut u8, size) }
        })
    }

    /// What [`to_mesh`](Self::to_mesh) would cost, asked before it is paid.
    ///
    /// `budget` of zero means no budget, which is what a desktop host passes.
    /// The figure that matters on a device that kills an application rather
    /// than warning it is [`SurfacePreflight::peak_bytes`], and it errs
    /// **high** on purpose: a budget that errs low says yes to an operation
    /// that does not fit.
    pub fn preflight_to_mesh(&self, budget: u64) -> Result<SurfacePreflight> {
        preflight("clay_dynamic_surface_preflight_to_mesh", |raw| {
            // SAFETY: a valid surface handle the call only reads, and a
            // versioned out-descriptor whose struct_size `preflight` set.
            unsafe { sys::clay_dynamic_surface_preflight_to_mesh(self.raw.as_ptr(), budget, raw) }
        })
    }

    /// What [`serialize`](Self::serialize) would cost.
    ///
    /// The blob is a second copy of everything and it exists while the surface
    /// still does, which is why the peak is worth asking for before a save.
    pub fn preflight_encode(&self, budget: u64) -> Result<SurfacePreflight> {
        preflight("clay_dynamic_surface_preflight_encode", |raw| {
            // SAFETY: as above.
            unsafe { sys::clay_dynamic_surface_preflight_encode(self.raw.as_ptr(), budget, raw) }
        })
    }

    /// A sculptor bound to this surface.
    ///
    /// The surface is borrowed exclusively for as long as the sculptor lives,
    /// which is the ABI's own rule — "the surface must outlive the sculptor" —
    /// expressed in the type system. The sculptor lends the surface back for
    /// *reading* through [`DynamicSculptor::surface`] and never as a `&mut`,
    /// because a `&mut` to an owning wrapper is a destructor safe code can run
    /// while the sculptor still holds a bare pointer into it.
    pub fn sculptor(&mut self) -> Result<DynamicSculptor<'_>> {
        let mut sculptor = std::ptr::null_mut();
        // SAFETY: valid handle and an out-parameter written only on success.
        check(
            unsafe { sys::clay_dynamic_sculptor_create(self.raw.as_ptr(), &mut sculptor) },
            "clay_dynamic_sculptor_create",
        )?;
        let raw = NonNull::new(sculptor)
            .ok_or_else(|| raw_failure("clay_dynamic_sculptor_create", ErrorKind::Backend))?;
        Ok(DynamicSculptor { raw, surface: self })
    }
}

impl Drop for DynamicSurface {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once. Every borrow of it — a
        // sculptor above all — carries a lifetime that ends before this does,
        // and nothing reachable from a sculptor yields a `&mut DynamicSurface`
        // to run this destructor through early.
        unsafe { sys::clay_dynamic_surface_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for DynamicSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicSurface")
            .field("stats", &self.stats().ok())
            .field("revision", &self.revision().ok())
            .finish()
    }
}

// -- the sculptor -----------------------------------------------------------

/// A brush over an adaptive surface, and the chunked index it drains.
///
/// Stateful, like [`MeshSculptor`](crate::MeshSculptor): it builds a spatial
/// index over the surface once and keeps it, which is what makes a dab cost
/// what its falloff reached rather than what the surface holds. Creating one
/// per stroke would pay the build per stroke.
///
/// It also owns the **chunk table** the transport drains, which is why
/// [`chunk_count`](Self::chunk_count) and the rest are here rather than on the
/// surface: the partition is the sculptor's, and a surface nobody has sculpted
/// has none.
pub struct DynamicSculptor<'s> {
    raw: NonNull<sys::clay_dynamic_sculptor>,
    surface: &'s mut DynamicSurface,
}

impl<'s> DynamicSculptor<'s> {
    /// The surface this sculptor writes into, for reading.
    ///
    /// Shared and never exclusive — see [`DynamicSurface::sculptor`] for why.
    /// It is enough for everything a caller wants mid-session: the revision
    /// after a stamp, the census, the serialized bytes.
    pub fn surface(&self) -> &DynamicSurface {
        self.surface
    }

    /// One stamp.
    ///
    /// `topology` of `None` takes the engine's own policy, which is what a
    /// host that has not exposed the controls yet should pass rather than a
    /// zeroed descriptor.
    ///
    /// **Fifteen of the sixteen verbs.** The one an adaptive surface declines
    /// is [`MeshBrush::Layer`](crate::MeshBrush), and it is refused rather
    /// than silently becoming something else: layer deposits up to a ceiling
    /// measured from where the surface was when the *stroke* began, and half
    /// the vertices under the brush at the end of a stroke here did not exist
    /// at the start.
    pub fn stamp(
        &mut self,
        stamp: MeshStamp<'_>,
        topology: Option<&DynamicTopology>,
        mask: Option<&MaskField>,
    ) -> Result<DynamicStampReport> {
        let brush = stamp.as_raw();
        let topology = topology.map(|t| t.to_raw());
        let mut report = sys::clay_dynamic_stamp_report::sized();
        // SAFETY: valid handle and a brush descriptor carrying its own size
        // and borrowing its alpha from `stamp`, which outlives this call. The
        // topology descriptor is either a versioned one or null, and the mask
        // is either a valid handle or null — both of which the entry point
        // documents as allowed. `report` is a versioned out-descriptor.
        check(
            unsafe {
                sys::clay_dynamic_sculptor_stamp(
                    self.raw.as_ptr(),
                    &brush,
                    topology
                        .as_ref()
                        .map_or(std::ptr::null(), |t| t as *const _),
                    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _),
                    &mut report,
                )
            },
            "clay_dynamic_sculptor_stamp",
        )?;
        Ok(DynamicStampReport::from_raw(report))
    }

    /// Rebuilds the chunked spatial index.
    ///
    /// **Between strokes, never mid-drag**: a refit stays correct and does not
    /// stay fast, and a rebuild is not automatically an improvement — measured
    /// over five deformations upstream, it produced a better tree in exactly
    /// one and a dramatically worse one in two. [`index_quality`] is the
    /// measurement to decide on.
    ///
    /// [`index_quality`]: Self::index_quality
    pub fn rebuild_index(&mut self) -> Result<()> {
        // SAFETY: valid handle; the call rewrites an index the sculptor owns.
        check(
            unsafe { sys::clay_dynamic_sculptor_rebuild_index(self.raw.as_ptr()) },
            "clay_dynamic_sculptor_rebuild_index",
        )
    }

    /// Names the lattices the brush's automask factors consume, or clears them
    /// with `None`.
    ///
    /// **Setting sources does not enable anything.** The bits in
    /// [`Automask`](crate::Automask) still decide which factors run; these are
    /// the inputs those bits consume. A host that sets a source and no bit
    /// gets exactly the stamps it got before, which is what makes wiring this
    /// up safe to do once at session start.
    ///
    /// The `cavity` field borrows for at least as long as this sculptor
    /// lives, because the engine holds what it resolves to rather than copying
    /// it. That is the `'s` in the signature, and it is the ABI's "borrowed,
    /// and for the whole stroke" made a compile error instead of a
    /// use-after-free.
    ///
    /// The polygroup half of `clay_automask_sources` is **not** here: it takes
    /// a `clay_groups` handle this crate does not wrap yet, so the
    /// surface-group factor stays inert. Named rather than hidden, for the
    /// reason [`Automask`](crate::Automask) names its own inert pair.
    pub fn set_automask_sources(&mut self, cavity: Option<&'s MaskField>) -> Result<()> {
        let sources = cavity.map(|mask| {
            let mut raw = sys::clay_automask_sources::sized();
            raw.cavity = mask.as_ptr() as *const _;
            raw
        });
        // SAFETY: valid handle. The descriptor is either a versioned one whose
        // `cavity` is a live mask handle borrowed for `'s` — which outlives
        // this sculptor — or null, which the entry point documents as clearing
        // both factors.
        check(
            unsafe {
                sys::clay_dynamic_sculptor_set_automask_sources(
                    self.raw.as_ptr(),
                    sources.as_ref().map_or(std::ptr::null(), |s| s as *const _),
                )
            },
            "clay_dynamic_sculptor_set_automask_sources",
        )
    }

    /// Declares where this surface is, or clears the declaration with `None`.
    ///
    /// Unset is the identity. Set, every position, radius and direction
    /// crossing this handle is world — the stamp's centre and the mask gate
    /// together, which is the point: a placed mask gate beside a local stamp
    /// centre would be a new disagreement of exactly the kind a frame exists
    /// to remove.
    pub fn set_world_frame(&mut self, frame: Option<WorldFrame>) -> Result<()> {
        let raw = frame.map(WorldFrame::to_raw);
        // SAFETY: valid handle and a descriptor that is either versioned and
        // read-only for the call, or null — which the entry point documents as
        // returning the session to the identity.
        check(
            unsafe {
                sys::clay_dynamic_sculptor_set_world_frame(
                    self.raw.as_ptr(),
                    raw.as_ref().map_or(std::ptr::null(), |f| f as *const _),
                )
            },
            "clay_dynamic_sculptor_set_world_frame",
        )
    }

    /// What this handle currently declares, or `None` if it declares nothing.
    ///
    /// Provided because a declared frame a host cannot read back makes "did
    /// that take?" answerable only by stamping and inspecting the result.
    pub fn world_frame(&self) -> Result<Option<WorldFrame>> {
        let mut raw = sys::clay_mesh_frame::sized();
        let mut declared = 0i32;
        // SAFETY: valid handle, a versioned out-descriptor and a valid `i32`
        // out-parameter; both are written on success.
        check(
            unsafe {
                sys::clay_dynamic_sculptor_world_frame(self.raw.as_ptr(), &mut raw, &mut declared)
            },
            "clay_dynamic_sculptor_world_frame",
        )?;
        Ok((declared != 0).then(|| WorldFrame::from_raw(raw)))
    }

    // -- the chunk transport ------------------------------------------------

    /// How many chunks the surface is partitioned into.
    pub fn chunk_count(&self) -> usize {
        // SAFETY: valid handle; the call only reads, and answers zero for a
        // handle it cannot resolve rather than failing.
        unsafe { sys::clay_dynamic_surface_chunk_count(self.raw.as_ptr()) }
    }

    /// One chunk's shape, for sizing a buffer before copying into it.
    pub fn chunk_info(&self, index: usize) -> Result<DynamicChunkInfo> {
        let mut raw = sys::clay_dynamic_chunk_info::sized();
        // SAFETY: valid handle and a versioned out-descriptor; the index is
        // range-checked by the entry point, which refuses rather than reading
        // past the chunk table.
        check(
            unsafe { sys::clay_dynamic_surface_chunk_info(self.raw.as_ptr(), index, &mut raw) },
            "clay_dynamic_surface_chunk_info",
        )?;
        Ok(DynamicChunkInfo::from_raw(raw))
    }

    /// The chunks the stamps since the last [`clear_dirty`] touched.
    ///
    /// Sized from the count the engine writes rather than from a guess, so a
    /// caller never sees a truncation — a short read here would silently drop
    /// a chunk the host had to re-upload, and a dropped chunk is a hole in the
    /// drawn surface that nothing else reports.
    ///
    /// [`clear_dirty`]: Self::clear_dirty
    pub fn dirty_chunks(&self) -> Result<Vec<u32>> {
        let handle = self.raw.as_ptr();
        size_query_array("clay_dynamic_surface_dirty_chunks", |buf, count| {
            // SAFETY: the array protocol's two calls. `buf` is either null —
            // asking for the count — or valid for writes of `*count` u32,
            // which the engine checks before it writes anything.
            unsafe { sys::clay_dynamic_surface_dirty_chunks(handle, buf, count) }
        })
    }

    /// Drops the whole dirty set.
    ///
    /// **All or nothing.** A host that drains incrementally and clears here
    /// loses the chunks it had not reached yet, because there is no
    /// acknowledgement on this entry point — take a
    /// [`SurfaceView`](crate::SurfaceView) over this sculptor for that.
    pub fn clear_dirty(&mut self) -> Result<()> {
        // SAFETY: valid handle; the call clears a set the sculptor owns.
        check(
            unsafe { sys::clay_dynamic_surface_clear_dirty(self.raw.as_ptr()) },
            "clay_dynamic_surface_clear_dirty",
        )
    }

    /// Copies one chunk into freshly allocated buffers.
    ///
    /// The convenience form. A renderer draining a dirty set every frame wants
    /// [`copy_chunk_into`](Self::copy_chunk_into) and a pool instead: nothing
    /// in the transport allocates a heap object per chunk per frame, and this
    /// method is the one place that would.
    pub fn copy_chunk(&self, index: usize) -> Result<DynamicChunk> {
        let info = self.chunk_info(index)?;
        let mut positions = vec![[0.0f32; 3]; info.vertex_count as usize];
        let mut normals = vec![[0.0f32; 3]; info.vertex_count as usize];
        let mut indices = vec![0u32; info.index_count as usize];
        let written = self.copy_chunk_into(index, &mut positions, &mut normals, &mut indices)?;
        positions.truncate(written.vertex_count as usize);
        normals.truncate(written.vertex_count as usize);
        indices.truncate(written.index_count as usize);
        Ok(DynamicChunk {
            info: written,
            positions,
            normals,
            indices,
        })
    }

    /// Copies one chunk into buffers the caller owns, and reports what was
    /// written.
    ///
    /// The buffers may be **larger** than the chunk needs, which is the whole
    /// point: a host sizes a pool once to the largest chunk it has seen and
    /// copies into it every frame. Every capacity is checked by the engine and
    /// nothing is written past it, so the tail of an oversized buffer is left
    /// exactly as the caller left it.
    ///
    /// The indices are **local to the chunk**, so it uploads as a standalone
    /// draw, and the vertices are unwelded triangles — see
    /// [`DynamicChunkInfo::vertex_count`].
    pub fn copy_chunk_into(
        &self,
        index: usize,
        positions: &mut [[f32; 3]],
        normals: &mut [[f32; 3]],
        indices: &mut [u32],
    ) -> Result<DynamicChunkInfo> {
        let mut written = sys::clay_dynamic_chunk_info::sized();
        // SAFETY: the three buffers are valid for writes of exactly the
        // capacities passed — three floats per `[f32; 3]`, whose layout is
        // three consecutive floats, and `indices.len()` u32 — and the engine
        // checks every one of them against the chunk before it writes
        // anything. `written` is a versioned out-descriptor.
        check(
            unsafe {
                sys::clay_dynamic_surface_copy_chunk(
                    self.raw.as_ptr(),
                    index,
                    positions.as_mut_ptr() as *mut f32,
                    positions.len() * 3,
                    normals.as_mut_ptr() as *mut f32,
                    normals.len() * 3,
                    indices.as_mut_ptr(),
                    indices.len(),
                    &mut written,
                )
            },
            "clay_dynamic_surface_copy_chunk",
        )?;
        Ok(DynamicChunkInfo::from_raw(written))
    }

    // -- memory and telemetry -----------------------------------------------

    /// What this session costs, in the vocabulary every representation shares.
    ///
    /// **Filled, not merged**: a caller adding up several surfaces adds the
    /// fields itself, because only it knows which surfaces belong together.
    pub fn memory_ledger(&self) -> Result<MemoryLedger> {
        let mut raw = sys::clay_memory_ledger::sized();
        // SAFETY: valid handle and a versioned out-descriptor whose
        // struct_size is written from the compiled type, so the engine knows
        // how many category entries it may fill.
        check(
            unsafe { sys::clay_dynamic_sculptor_memory_ledger(self.raw.as_ptr(), &mut raw) },
            "clay_dynamic_sculptor_memory_ledger",
        )?;
        Ok(MemoryLedger::from_raw(raw))
    }

    /// Releases rebuildable caches at a stated pressure, in a fixed order.
    ///
    /// Never authoritative content and never history: not the topology, not
    /// the positions, not a mask. A held `pin` makes this a no-op that reports
    /// what it *would* have released, with [`TrimReport::pinned`] set.
    pub fn trim(&mut self, pressure: Pressure, pin: Option<&MemoryPin>) -> Result<TrimReport> {
        let mut raw = sys::clay_trim_report::sized();
        // SAFETY: valid handle, a pressure the entry point range-checks, a pin
        // that is either a valid handle borrowed for the duration of the call
        // or null, and a versioned out-descriptor.
        check(
            unsafe {
                sys::clay_dynamic_sculptor_trim(
                    self.raw.as_ptr(),
                    pressure.to_raw(),
                    pin.map_or(std::ptr::null(), MemoryPin::as_ptr),
                    &mut raw,
                )
            },
            "clay_dynamic_sculptor_trim",
        )?;
        Ok(TrimReport::from_raw(raw))
    }

    /// The per-stamp scratch arena's high-water mark.
    ///
    /// `growths` is the one to watch: a count that has stopped rising over
    /// stamps of similar footprint is the arena having converged; one that
    /// rises every stamp is scratch that is never released.
    pub fn arena_stats(&self) -> Result<ArenaStats> {
        let mut raw = sys::clay_brush_arena_stats::sized();
        // SAFETY: valid handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_dynamic_sculptor_arena_stats(self.raw.as_ptr(), &mut raw) },
            "clay_dynamic_sculptor_arena_stats",
        )?;
        Ok(ArenaStats {
            capacity_bytes: raw.capacity_bytes,
            high_water_bytes: raw.high_water_bytes,
            growths: raw.growths,
        })
    }

    /// The four high-water marks a host tunes a [`SculptMemoryProfile`]
    /// against.
    ///
    /// The adaptive surface is the only representation that fills all three
    /// live counters: `topology_ops` is a split-and-collapse count no other
    /// one has, and it is the number that decides how big the slot pools have
    /// to be.
    pub fn peak_telemetry(&self) -> Result<PeakTelemetry> {
        let mut raw = sys::clay_peak_telemetry::sized();
        // SAFETY: valid handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_dynamic_sculptor_peak_telemetry(self.raw.as_ptr(), &mut raw) },
            "clay_dynamic_sculptor_peak_telemetry",
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
            unsafe { sys::clay_dynamic_sculptor_reset_peak_telemetry(self.raw.as_ptr()) },
            "clay_dynamic_sculptor_reset_peak_telemetry",
        )
    }

    /// What the spatial index is worth right now.
    pub fn index_quality(&self) -> Result<IndexQuality> {
        let mut raw = sys::clay_index_quality::sized();
        // SAFETY: valid handle and a versioned out-descriptor.
        check(
            unsafe { sys::clay_dynamic_sculptor_index_quality(self.raw.as_ptr(), &mut raw) },
            "clay_dynamic_sculptor_index_quality",
        )?;
        Ok(IndexQuality::from_raw(raw))
    }

    /// Queues an index rebuild if the tree wants one **and** the profile
    /// allows it, and reports whether anything was queued.
    ///
    /// Deliberately still a request: it puts the job where a host can see it
    /// and rebuilds nothing. [`rebuild_index`](Self::rebuild_index) is what
    /// performs it, between strokes. `profile` of `None` is the default
    /// profile, which allows it.
    pub fn request_index_rebuild(
        &self,
        profile: Option<SculptMemoryProfile>,
        target: u32,
        queue: &mut MaintenanceQueue,
    ) -> Result<bool> {
        let raw_profile = profile.map(SculptMemoryProfile::to_raw);
        let mut queued = 0i32;
        // SAFETY: valid sculptor and queue handles, a profile descriptor that
        // is either versioned and read-only for the call or null, and a valid
        // `i32` out-parameter.
        check(
            unsafe {
                sys::clay_dynamic_sculptor_request_index_rebuild(
                    self.raw.as_ptr(),
                    raw_profile
                        .as_ref()
                        .map_or(std::ptr::null(), |p| p as *const _),
                    target,
                    queue.as_ptr(),
                    &mut queued,
                )
            },
            "clay_dynamic_sculptor_request_index_rebuild",
        )?;
        Ok(queued != 0)
    }

    /// Starts or stops timing and counting on this session. Off by default.
    ///
    /// Enabling does not clear what is already there; enabling on a fresh
    /// session starts from zero.
    pub fn set_stage_report_enabled(&mut self, enabled: bool) -> Result<()> {
        // SAFETY: valid handle and a plain flag.
        check(
            unsafe {
                sys::clay_dynamic_sculptor_set_stage_report_enabled(
                    self.raw.as_ptr(),
                    i32::from(enabled),
                )
            },
            "clay_dynamic_sculptor_set_stage_report_enabled",
        )
    }

    /// Where the stamps spent their time, and what they did.
    pub fn stage_report(&self) -> Result<StageReport> {
        let mut raw = sys::clay_sculpt_stage_report::sized();
        // SAFETY: valid handle and a versioned out-descriptor whose
        // struct_size is written from the compiled type, so the engine fills
        // only the stages this build has room for and says how many.
        check(
            unsafe { sys::clay_dynamic_sculptor_stage_report(self.raw.as_ptr(), &mut raw) },
            "clay_dynamic_sculptor_stage_report",
        )?;
        Ok(StageReport::from_raw(raw))
    }

    /// Zeroes it, so the next stamp is measured on its own.
    pub fn reset_stage_report(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_dynamic_sculptor_reset_stage_report(self.raw.as_ptr()) },
            "clay_dynamic_sculptor_reset_stage_report",
        )
    }

    /// The raw handle, for sibling modules in this crate only.
    pub(crate) fn as_ptr(&mut self) -> *mut sys::clay_dynamic_sculptor {
        self.raw.as_ptr()
    }
}

impl Drop for DynamicSculptor<'_> {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once, and before the borrow
        // of the surface it points at ends. Nothing reachable from this type
        // yields a `&mut DynamicSurface`, so the surface cannot have been
        // destroyed early and is still alive here.
        unsafe { sys::clay_dynamic_sculptor_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for DynamicSculptor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicSculptor")
            .field("surface", &self.surface)
            .field("chunks", &self.chunk_count())
            .finish()
    }
}
