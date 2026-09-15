//! Keeping the displayed surface up to date with the document.
//!
//! The composition root is the only place that can do this: it needs the
//! engine (to mesh) and the renderer (to upload), and no layer between them is
//! allowed to see both.
//!
//! The cost that matters is meshing, which the engine bounds for us — its own
//! benchmark is 0.64 ms for the eight bricks a dab dirties against 22.6 ms for
//! a full re-mesh of 232. So the dirty subset is meshed and nothing else, and
//! each key's geometry is kept so the whole surface can be reassembled without
//! re-marching it.
//!
//! Keeping it per key is also what lets the GPU buffers be written per key:
//! see [`crate::slots`], which gives each key a span it keeps, so a dab writes
//! only what it changed. Both halves matter — meshing the dirty subset is
//! wasted if the result is then copied to the GPU in full.

use std::collections::HashMap;

use clayspace_engine::claycore::{
    BrickKey, BrickMeshParams, BrickState, ClayError, Document, Mesh, VertexLayout,
};
use clayspace_engine::ClayDocument;
use clayspace_model::Detail;
use clayspace_view::{Gpu, GpuMesh, Vertex};

use crate::slots::SlotMap;

/// How much of the drawn index range may be holes before it is worth the cost
/// of laying the whole surface out again.
///
/// Holes are degenerate triangles, so they are paid on every frame while a
/// rebuild is paid once. A fifth is the point where the per-frame vertex work
/// outweighs the rebuild on the reference scene.
const MAX_WASTE: f32 = 0.2;

/// One key's contribution to the surface.
#[derive(Debug, Clone, Default)]
struct KeyGeometry {
    vertices: Vec<Vertex>,
    /// Indices relative to this key's own vertices.
    indices: Vec<u32>,
}

/// One mesh's global indices, reused across its brick ranges.
/// Only touched entries are cleared between bricks; nothing survives the split.
struct VertexRemap {
    indices: Vec<Option<u32>>,
    touched: Vec<usize>,
}

impl VertexRemap {
    fn new(vertices: usize) -> Self {
        Self {
            indices: vec![None; vertices],
            touched: Vec::new(),
        }
    }

    fn local_index(&mut self, global: usize, source: &[Vertex], local: &mut Vec<Vertex>) -> u32 {
        if let Some(index) = self.indices[global] {
            return index;
        }
        let index = local.len() as u32;
        local.push(source[global]);
        self.indices[global] = Some(index);
        self.touched.push(global);
        index
    }

    fn clear(&mut self) {
        for global in self.touched.drain(..) {
            self.indices[global] = None;
        }
    }
}

fn write_key_geometry(
    into: &mut KeyGeometry,
    vertices: &[Vertex],
    triangles: &[[u32; 3]],
    remap: &mut VertexRemap,
) {
    // Preserve the former placeholder behavior for malformed engine indices.
    if triangles
        .iter()
        .flatten()
        .any(|&index| index as usize >= vertices.len())
    {
        write_key_geometry_sparse(into, vertices, triangles);
        return;
    }
    into.vertices.clear();
    into.indices.clear();
    for &global in triangles.iter().flatten() {
        let index = remap.local_index(global as usize, vertices, &mut into.vertices);
        into.indices.push(index);
    }
    remap.clear();
}

/// The original remapping algorithm, retained for out-of-range indices.
fn write_key_geometry_sparse(into: &mut KeyGeometry, vertices: &[Vertex], triangles: &[[u32; 3]]) {
    into.vertices.clear();
    into.indices.clear();
    let mut local = HashMap::new();
    for &global in triangles.iter().flatten() {
        let next = local.len() as u32;
        into.indices.push(*local.entry(global).or_insert(next));
    }
    into.vertices.resize(
        local.len(),
        Vertex {
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            color: [1.0; 3],
            mask: 0.0,
        },
    );
    for (global, index) in local {
        if let Some(vertex) = vertices.get(global as usize) {
            into.vertices[index as usize] = *vertex;
        }
    }
}

/// What a sync cost, for the latency budget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyncCost {
    /// Keys re-meshed.
    pub keys: usize,
    /// Everything between asking the cache for geometry and having it stored
    /// per key: the engine's mesh call, the copy into our vertex layout, and
    /// the per-key split. Broken out below.
    pub mesh_time: std::time::Duration,
    /// The engine's own `clay_brick_cache_mesh`.
    pub engine_mesh_time: std::time::Duration,
    /// Copying the engine's mesh into the renderer's vertex layout.
    pub read_time: std::time::Duration,
    /// Splitting the triangles into per-key geometry so a dab can replace one.
    pub split_time: std::time::Duration,
    /// Writing the changed spans to the GPU.
    pub upload_time: std::time::Duration,
    pub triangles: usize,
    pub vertices: usize,
}

/// Which of `settle`'s three routes ran, because they cost very different
/// things and the ledger records them under one label.
///
/// `re-malha final` averaged 59.4 ms over a session while ClayCore measures
/// the whole-field mesh at 3.1 ms on a clean sphere. Those cannot both
/// describe the same work, and until this existed there was no way to say
/// which route a given occurrence took, let alone how the time inside it
/// divided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettleRoute {
    /// Exact duplicate compaction without evaluating or meshing the field.
    Compact,
    /// A per-key rebuild at full resolution, used for explicit rebuilding and
    /// release geometry that cannot be compacted.
    ///
    /// This used to be a whole-field `clay_document_mesh`. It was there to hide
    /// the brick mesher's sliver triangles — 2,297 near-zero-area triangles in
    /// 83,464, whose face normals are cross products of near-parallel edges and
    /// shade black. ClayCore #549 fixed them at the source in v0.113.0, so the
    /// workaround went with the defect.
    Bricks,
    /// A per-key rebuild at the coarse level, taken when that is the one
    /// requested.
    Coarse,
    /// The field is empty, so there was nothing to mesh.
    Empty,
}

/// What one `settle` cost, split the way `SyncCost` splits a sync.
///
/// The point of the split is that it separates the ENGINE's call from ours.
/// A whole-document mesh that is slow because the engine is slow and one that
/// is slow because we spend the time copying, masking and uploading afterwards
/// are different defects with different owners, and the single `re-malha final`
/// number could not tell them apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SettleCost {
    pub route: SettleRoute,
    /// The engine's share of the mesh, and only that.
    pub engine_mesh_time: std::time::Duration,
    /// Copying the engine's mesh into the renderer's vertex layout, and
    /// sampling the mask over it.
    pub read_time: std::time::Duration,
    /// Separately timed GPU writes, when available. Rebuild and compaction
    /// currently include layout/upload work in `total_time` instead.
    pub upload_time: std::time::Duration,
    /// Everything `settle` spent, so `total - (engine + read + upload)` is
    /// what the bookkeeping around them cost.
    pub total_time: std::time::Duration,
    pub triangles: usize,
    pub vertices: usize,
}

/// The surface as the viewport holds it.
pub struct SurfaceGeometry {
    /// Per-key geometry, so a re-mesh replaces only what changed.
    keys: HashMap<BrickKey, KeyGeometry>,
    mesh: GpuMesh,
    /// Set when the keys have changed but the GPU buffer has not been rebuilt.
    dirty: bool,
    /// Set when the surface at the level being drawn would not fit the
    /// device's largest buffer, so the last layout was refused and what is on
    /// screen is stale. The composition root reads it and drops a level.
    over_budget: bool,
    /// Which keys changed since the last upload, so only those are written.
    touched: std::collections::HashSet<BrickKey>,
    /// Where each key's geometry sits in the GPU buffers.
    layout: SlotMap,
    /// Set when the layout cannot be patched and must be laid out afresh.
    relayout: bool,
    /// The union of every key's bounds, exact as of the last full rebuild.
    bounds: Option<([f32; 3], [f32; 3])>,
    last_cost: Option<SyncCost>,
    /// What the last `settle` cost, by route. `None` until one has run.
    last_settle: Option<SettleCost>,
    /// Stage timings from the last `remesh`, for `SyncCost`.
    last_engine_mesh: std::time::Duration,
    last_read: std::time::Duration,
    last_split: std::time::Duration,
    /// Where the warped keys' vertices were before a cage preview moved them.
    ///
    /// Positions only, and only while a cage is up. A preview is shown by
    /// moving the vertices the viewport already holds, and putting them back
    /// needs the originals — recomputing them by warping backwards would
    /// accumulate the error of two approximations instead of none.
    cage_rest: HashMap<BrickKey, Vec<[f32; 3]>>,
    /// Which surface the stored geometry belongs to.
    ///
    /// A live gesture draws from a cache of its own whose keys name different
    /// bricks, so the store cannot be patched across the swap and is laid out
    /// again when this stops matching the document's.
    surface_epoch: u64,
    /// Separate partial requests may assign the same boundary triangle to
    /// different keys. A full replacement or eligible release compaction clears it.
    needs_settle: bool,
    /// Every retained triangle uses request-independent document gradients.
    document_gradients: bool,
    /// The level the stored geometry was meshed at.
    ///
    /// Distinct from `requested` because a coarse surface is not always
    /// available: with no mip built yet, asking for `Reduced` draws `Full`
    /// rather than nothing, and this records what is actually on screen.
    detail: Detail,
    /// The level last asked for.
    ///
    /// Kept so a fallback settles instead of retrying: a request that could
    /// not be met is not re-attempted on the next frame, only when the
    /// request changes or [`SurfaceGeometry::reapply_detail`] says the mips
    /// have since been built.
    requested: Detail,
}

/// How a mesh is shaded.
///
/// Both produce identical vertex *positions* — normals are an attribute, not
/// a displacement — so switching between them cannot move the surface. What
/// changes is the gradient sampling, measured after 96 edits over 80 bricks:
///
/// | engine | face normals | gradient normals | premium |
/// |---|---|---|---|
/// | 0.28.0 | 7.7 ms | 83.2 ms | 11x |
/// | 0.29.1 | 8.0 ms | 11.5 ms | 1.4x |
/// | 0.30.0 | 12.6 ms | 13.2 ms | 1.04x |
///
/// Three upstream fixes narrowed it — #73 culling the tape per brick, #83
/// batching the attribute taps, and #93's release carrying the rest.
///
/// The table above is a *fixed 80-brick sample*, which is not what a segment
/// meshes. Over the 27 keys a dab actually dirties, three runs of 96 segments
/// each on the same worked model:
///
/// | shading | median | p95 | worst |
/// |---|---|---|---|
/// | face normals | 3.4 / 3.4 / 4.1 ms | 3.9 / 4.0 / 5.3 ms | 4.1 / 4.2 / 6.2 ms |
/// | gradient | 5.0 / 4.7 / 4.9 ms | 8.1 / 5.2 / 11.4 ms | 14.9 / 5.6 / 18.9 ms |
///
/// So the premium is 40% at the median and, in the tail, the difference
/// between a segment that always fits a frame and one that sometimes takes
/// 19 ms. The cursor ring is drawn in the frame that meshes the edit, so that
/// tail is what a sculptor feels as the ring trailing the pointer.
///
/// The historical drag path used `Fast` and bought the gradient back on idle
/// frames. That exposed degenerate face normals as persistent-looking pits and
/// specks, while the current engine's parallel brick mesher has made the old
/// saving too small to justify drawing damaged clay. Full-resolution document
/// edits therefore use `Full` immediately. `Fast` remains for live preview
/// caches, which have no document to sample a gradient from, and coarse LOD,
/// which refuses gradient attributes.
///
/// [`Shading::Fast`] is also what the coarse LOD surface uses, and there for
/// a different reason: level 1 *refuses* gradient normals rather than
/// downgrading them, which is the one place the choice is not about cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shading {
    /// Area-weighted face normals. Needs no field sampling, so it is flat
    /// against the document's size — which is exactly what #73 is not.
    Fast,
    /// The field gradient, so blends read smooth across a seam.
    Full,
}

impl Shading {
    fn gradient(self) -> bool {
        matches!(self, Self::Full)
    }
}

impl SurfaceGeometry {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            cage_rest: HashMap::new(),
            surface_epoch: 0,
            needs_settle: false,
            document_gradients: true,
            keys: HashMap::new(),
            mesh: GpuMesh::new(gpu),
            dirty: false,
            touched: std::collections::HashSet::new(),
            layout: SlotMap::default(),
            relayout: true,
            bounds: None,
            last_cost: None,
            last_settle: None,
            last_engine_mesh: std::time::Duration::ZERO,
            last_read: std::time::Duration::ZERO,
            last_split: std::time::Duration::ZERO,
            detail: Detail::Full,
            requested: Detail::Full,
            over_budget: false,
        }
    }

    /// Whether stored geometry combines separate partial meshing requests.
    pub fn needs_settle(&self) -> bool {
        self.needs_settle
    }

    /// Whether the surface, at the level being drawn, is more than the device
    /// can hold — the cue to draw it coarser.
    pub fn over_budget(&self) -> bool {
        self.over_budget
    }

    pub fn mesh(&self) -> &GpuMesh {
        &self.mesh
    }

    pub fn last_cost(&self) -> Option<SyncCost> {
        self.last_cost
    }

    /// Finish an already synchronized stroke without re-evaluating unchanged
    /// geometry. Document gradients depend on vertex position, so partial
    /// requests differ only in ownership and exact duplicate copies. Preview
    /// face normals depend on the requested neighborhood and cannot use this.
    /// Explicit `settle` and `rebuild` still perform a complete rebuild.
    pub fn settle_after_edit(
        &mut self,
        gpu: &Gpu,
        document: &mut ClayDocument,
    ) -> Result<(), ClayError> {
        if document.live_gesture_is_open() {
            return Ok(());
        }
        if !self.document_gradients
            || self.surface_epoch != document.surface_epoch()
            || self.detail != Detail::Full
            || !self.cage_rest.is_empty()
        {
            return self.settle(gpu, document);
        }
        let started = std::time::Instant::now();
        compact_release_geometry(&mut self.keys);
        self.relayout = true;
        self.dirty = true;
        self.lay_out_prepared(gpu);
        // A refused layout must retain its debt and dirty geometry.
        if !self.dirty {
            self.needs_settle = false;
        }
        self.last_settle = Some(SettleCost {
            route: SettleRoute::Compact,
            engine_mesh_time: std::time::Duration::ZERO,
            read_time: std::time::Duration::ZERO,
            upload_time: std::time::Duration::ZERO,
            total_time: started.elapsed(),
            triangles: self.triangle_count(),
            vertices: self.vertex_count(),
        });
        document.record_geometry(self.triangle_count(), self.vertex_count(), self.detail);
        Ok(())
    }

    /// What the last `settle` cost, and which route it took.
    pub fn last_settle(&self) -> Option<SettleCost> {
        self.last_settle
    }

    pub fn triangle_count(&self) -> usize {
        self.keys.values().map(|k| k.indices.len() / 3).sum()
    }

    pub fn vertex_count(&self) -> usize {
        self.keys.values().map(|k| k.vertices.len()).sum()
    }

    /// Re-meshes whatever the document reports as dirty and uploads the result.
    ///
    /// Returns `None` when nothing was dirty, so a frame with no edit costs
    /// nothing at all.
    pub fn sync(
        &mut self,
        gpu: &Gpu,
        document: &mut ClayDocument,
    ) -> Result<Option<SyncCost>, ClayError> {
        // An edit while the coarse surface is drawn returns to full resolution
        // first. The two levels do not share a key space — a coarse key names
        // a 2x2x2 block of fine ones — so the dirty keys the engine hands back
        // do not address the store a coarse rebuild left behind. Without this,
        // the sync fails outright rather than drawing something wrong, which
        // `lod_switching.rs` records. It is also what the sculptor wants:
        // dirtying any child drops its mip, so there is nothing coarse left to
        // draw where the edit landed anyway.
        //
        // The rebuild meshes the edit and drains the dirty set with it, so the
        // `take` below finds nothing and this reports no incremental cost. The
        // surface is correct; only the latency line skips a frame.
        if self.detail == Detail::Reduced {
            self.rebuild_at(gpu, document, Detail::Full)?;
        }
        // The surface swapped identity: a live gesture opened, installed what
        // it previewed, or was abandoned. A preview key and a document key of
        // the same coordinate name different bricks, so there is nothing to
        // patch and the whole surface is laid out again — once per gesture, at
        // each end of it.
        if self.surface_epoch != document.surface_epoch() {
            self.surface_epoch = document.surface_epoch();
            self.rebuild_at(gpu, document, Detail::Full)?;
        }
        let dirty = document.take_dirty_keys();
        if dirty.is_empty() {
            return Ok(None);
        }
        // The dirty bricks, and only those.
        //
        // There used to be a ring of dilation around them. It was there
        // because a subset mesh omitted triangles straddling its boundary, so
        // a stroke left seams — and the ring did not close them either, which
        // is what `settle` was for. ClayCore 0.28.0 fixed the omission (#66),
        // `settle` went, and the ring stayed: nobody asked whether it was
        // still buying anything.
        //
        // It was not. A subset now returns every triangle with at least one
        // corner in a requested brick, attributed to the lowest requested key,
        // so requesting the dirty core is enough — `settle_needed.rs` holds
        // the result to being triangle-for-triangle what a rebuild produces
        // either way. Measured on the same stroke, only the ring changing:
        //
        //   keys per dab      200 -> 48        engine mesh   5.8 -> 1.7 ms
        //   Padrao's segment 36.6 -> 17.4 ms   Puxar's       239 -> 177 ms
        //
        // Everything requested is also replaced, which is the other half of
        // exactness: a straddling triangle is attributed to whichever
        // requested key owns a corner, so replacing less than the request
        // would drop it.
        //
        // Meshed is a subset of that again: only the keys that actually hold a
        // surface. A dirty set is an edit's *influence bound*, which is a box,
        // and a box around a surface is mostly not surface — a third of a
        // dab's keys and two thirds of an undo's are uniformly inside or
        // outside, where marching `dim³` cells is guaranteed to produce
        // nothing. Measured on the reference form, 1043 surface bricks:
        //
        //   a dab    27 dirty keys,   18 hold a surface
        //   an undo  2940 dirty keys, 1045 hold a surface
        //
        // They are still *replaced*: a brick the surface has left has to lose
        // its stored triangles, and `remesh` clears a replaced key it was not
        // asked to mesh. Replacing more than was requested is safe where
        // replacing less is not, because a straddling triangle is attributed
        // to a *requested* key and an unrequested one can hold none.
        let replace: std::collections::HashSet<BrickKey> = dirty.iter().copied().collect();
        let meshed = Self::holding_a_surface(document, &dirty)?;

        let started = std::time::Instant::now();
        // A live transaction draws a cache of its own and has no document
        // behind it from which to sample gradients. Every ordinary edit does,
        // and uses them immediately: face normals make the cache's degenerate
        // preview triangles appear as pits across an otherwise smooth form.
        let shading = if document.live_gesture_is_open() {
            Shading::Fast
        } else {
            Shading::Full
        };
        self.remesh(document, &meshed, Some(&replace), shading, 0)?;
        let mesh_time = started.elapsed();

        let started = std::time::Instant::now();
        self.upload(gpu);
        let upload_time = started.elapsed();

        // The interface reports what is on screen, so the counts come from
        // what was actually built rather than from an estimate.
        document.record_geometry(self.triangle_count(), self.vertex_count(), self.detail);

        let cost = SyncCost {
            keys: meshed.len(),
            engine_mesh_time: self.last_engine_mesh,
            read_time: self.last_read,
            split_time: self.last_split,
            mesh_time,
            upload_time,
            triangles: self.triangle_count(),
            vertices: self.vertex_count(),
        };
        self.last_cost = Some(cost);
        Ok(Some(cost))
    }

    /// Meshes a set of keys and replaces their stored geometry.
    /// The keys of `dirty` that hold an fp16 lattice, in the same order.
    ///
    /// Asked per key rather than by intersecting with `surface_bricks`, which
    /// is a size query plus a copy of every stored key in the cache: a dab
    /// filtering 27 keys would pay for the whole surface to learn about nine.
    fn holding_a_surface(
        document: &ClayDocument,
        dirty: &[BrickKey],
    ) -> Result<Vec<BrickKey>, ClayError> {
        let states = document.drawn_cache().0.states(dirty)?;
        Ok(dirty
            .iter()
            .zip(states)
            .filter(|(_, state)| *state == BrickState::Surface)
            .map(|(key, _)| *key)
            .collect())
    }

    /// Meshes `keys` and replaces the stored geometry of `replace`.
    ///
    /// `replace` of `None` means every meshed key, which is what a full
    /// rebuild wants. A subset re-mesh passes a smaller set than it meshed —
    /// see [`SurfaceGeometry::sync`] for why.
    ///
    /// `lod` is 0 for the full-resolution bricks or 1 for their mips, where
    /// `keys` names coarse keys instead. A caller may not mix the two: the
    /// stored geometry is keyed by whatever level built it.
    fn remesh(
        &mut self,
        document: &ClayDocument,
        keys: &[BrickKey],
        replace: Option<&std::collections::HashSet<BrickKey>>,
        shading: Shading,
        lod: i32,
    ) -> Result<(), ClayError> {
        // Even a dirty-key request can replace the entire stored surface
        // (for example undo). Only retained old triangles mix ownership.
        let needs_settle = retains_unreplaced_triangles(&self.keys, replace);
        let document_gradients = shading.gradient()
            && !document.live_gesture_is_open()
            && (!needs_settle || self.document_gradients);
        let engine_started = std::time::Instant::now();
        // The document is what a gradient is sampled through, so it goes
        // wherever gradient normals are asked for — which, since ClayCore
        // #550, includes level 1. Before that the level refused them outright
        // and the coarse mesh took face normals from its triangles, so passing
        // a document there only cost a tape nothing would read.
        //
        // Still no document while a live gesture is drawing, and that is a
        // different rule with a different reason: the preview's lattice is not
        // the document's field, so attributing gradient normals through the
        // document would shade the previewed surface with the one it is
        // standing in for. `level_for` asks for face normals there to match.
        let (cache, offset) = document.drawn_cache();
        let doc =
            (shading.gradient() && !document.live_gesture_is_open()).then(|| document.document());
        // Nothing requested means nothing meshed — *not* what the same words
        // mean one layer down. An empty key list is how the C ABI spells "every
        // surface brick", which is right for an export and catastrophic here:
        // an edit whose dirty keys all turned out to be uniformly inside or
        // outside asks for nothing and would be handed the whole model. It cost
        // 1.31 s and 2.9 M triangles on a 9466-brick scene to establish that a
        // dab under the surface changed nothing.
        //
        // Reachable only since the dirty set started being filtered to the keys
        // that can hold a triangle, which is what made "nothing to mesh" a
        // state rather than an impossibility. `scaling_probe.rs` holds it.
        let (mesh, ranges) = if keys.is_empty() {
            (None, Vec::new())
        } else {
            let (mesh, ranges) = cache.mesh_lod(
                doc,
                BrickMeshParams {
                    gradient_normals: shading.gradient(),
                    colors: false,
                    gradient_eps: None,
                },
                lod,
                keys,
            )?;
            (Some(mesh), ranges)
        };

        self.last_engine_mesh = engine_started.elapsed();

        let read_started = std::time::Instant::now();
        let (vertices, indices) = read_drawn_mesh(mesh.as_ref(), offset, document)?;
        self.last_read = read_started.elapsed();
        let split_started = std::time::Instant::now();

        // Each triangle is filed under exactly one key, and a key stores the
        // vertices its own triangles reference — which for a boundary triangle
        // includes vertices welded to a neighbour's.
        //
        // Which key is the whole question, and it has been wrong twice.
        //
        // The first version stored only a key's own vertex range and dropped
        // any triangle reaching outside it. That opened a crack along every
        // brick boundary: the engine welds vertices across seams, so a great
        // many triangles reach outside. The capture showed a grid of holes
        // across the whole surface, which no count or timing would have named.
        //
        // The second version filed a triangle under whichever key's *vertex*
        // range held its first corner. That is not where the engine put it.
        // Welding spans seams — "a triangle in one key's index range may
        // reference a vertex in an EARLIER key's vertex range" — so a triangle
        // could be filed under a key holding none of its corners. Nothing is
        // wrong with the surface that frame; the damage comes later, when that
        // key is replaced by a request whose bricks the triangle does not
        // touch. Then it is cleared and nothing re-emits it, because the
        // engine only returns triangles with a corner in a requested brick.
        // That is the hole a sculptor sees, appearing minutes after the stroke
        // that caused it.
        //
        // A triangle is filed under the key whose *index* range the engine
        // listed it in. That is the engine's own attribution — "the
        // lexicographically lowest requested key whose closed box contains one
        // of its corners" — so the key always holds a corner, and any later
        // request naming that key re-emits the triangle before replacing it.
        // The ranges partition the mesh, so this files each triangle exactly
        // once and needs no search.
        // Only keys actually being replaced need anything built for them;
        // building the rest anyway was most of the cost of meshing the whole
        // surface.
        let wanted =
            |slot: usize| replace.is_none_or(|replace| replace.contains(&ranges[slot].key));
        let mut owned: Vec<Vec<[u32; 3]>> = vec![Vec::new(); ranges.len()];
        for (slot, range) in ranges.iter().enumerate() {
            if !wanted(slot) {
                continue;
            }
            let first = range.index_first as usize;
            let last = (first + range.index_count as usize).min(indices.len());
            for triangle in indices[first..last].chunks_exact(3) {
                owned[slot].push([triangle[0], triangle[1], triangle[2]]);
            }
        }

        // `None` means replace everything this call meshed, which is what a
        // full rebuild wants.
        let to_replace: Vec<BrickKey> = match replace {
            Some(replace) => replace.iter().copied().collect(),
            None => ranges.iter().map(|range| range.key).collect(),
        };
        // A key's stored geometry is replaced outright rather than merged.
        //
        // Keeping the triangles a partial mesh could not have regenerated was
        // tried — recording which brick owns each vertex, and holding on to
        // any triangle referencing a vertex from a brick this call did not
        // mesh. It never fired: before 0.28.0 those triangles were exactly the
        // ones the engine omitted from a subset (#66), so they were not in the
        // stored geometry to be kept either; since 0.28.0 they are returned.
        // The machinery came out rather than sitting there looking like it did
        // something.
        // Keyed rather than scanned. `position` per replaced key is
        // `replace * ranges`, which is three million comparisons where an undo
        // replaces 2940 keys against a 1045-key request — most of the split.
        let slot_of: HashMap<BrickKey, usize> = ranges
            .iter()
            .enumerate()
            .map(|(slot, range)| (range.key, slot))
            .collect();
        let mut remap = VertexRemap::new(vertices.len());
        for key in &to_replace {
            self.touched.insert(*key);
            let triangles = slot_of
                .get(key)
                .map(|&slot| owned[slot].as_slice())
                .unwrap_or_default();
            // An absent or empty range clears the previous contribution too.
            write_key_geometry(
                self.keys.entry(*key).or_default(),
                &vertices,
                triangles,
                &mut remap,
            );
        }
        self.last_split = split_started.elapsed();
        // The vertices a preview was holding the originals of have been
        // replaced, so those originals describe geometry that is gone. Dropped
        // rather than patched: the next preview stores them again from what is
        // there now.
        self.cage_rest.clear();
        self.dirty = true;
        self.needs_settle = needs_settle;
        self.document_gradients = document_gradients;
        Ok(())
    }

    /// Writes the keys that changed, and only those.
    ///
    /// Each key owns a span of both buffers and keeps it, so a dab writes the
    /// twenty-odd spans it touched instead of rewriting the surface. That is
    /// the difference between an upload that costs what the edit costs and one
    /// that costs what the model costs — measured on the reference stroke,
    /// 3.1 ms down to 0.2 ms with the model six times larger.
    ///
    /// Falls back to a full rebuild when the layout can no longer take a
    /// patch: the first upload, a buffer that has run out of room, or too much
    /// of the drawn range gone to holes.
    fn upload(&mut self, gpu: &Gpu) {
        if !self.dirty {
            return;
        }
        if self.relayout || self.layout.waste() > MAX_WASTE {
            self.lay_out(gpu);
            return;
        }
        for key in std::mem::take(&mut self.touched) {
            if !self.patch(gpu, key) {
                // Out of room. Everything written so far is still consistent,
                // and the rebuild below replaces all of it anyway.
                self.touched.clear();
                self.lay_out(gpu);
                return;
            }
        }
        self.mesh.set_index_count(self.layout.index_count());
        self.mesh.set_bounds(self.bounds);
        self.dirty = false;
    }

    /// Writes one key into its span, re-homing it if it has outgrown it.
    ///
    /// `false` means the buffers are full and the caller must rebuild.
    fn patch(&mut self, gpu: &Gpu, key: BrickKey) -> bool {
        let Some(geometry) = self.keys.get(&key) else {
            return true;
        };
        if geometry.indices.is_empty() {
            // Emptied by an edit rather than removed: the key keeps its span
            // so a later edit finds it, but must stop drawing.
            if let Some(slot) = self.layout.get(key) {
                let span = (slot.index_base, slot.index_base + slot.index_capacity);
                blank(&mut self.mesh, gpu, span);
            }
            return true;
        }
        let Some(placed) = self.layout.place(
            key,
            geometry.vertices.len() as u32,
            geometry.indices.len() as u32,
        ) else {
            return false;
        };
        if let Some(span) = placed.stranded {
            blank(&mut self.mesh, gpu, span);
        }
        let slot = placed.slot;

        // Indices are stored relative to the key's own vertices, so they are
        // rebased onto wherever the span landed. The tail of the span is
        // filled with degenerate triangles: the surface is one draw call over
        // one range, and a zero-area triangle is the cheapest way for a
        // partly-used span to draw only the part that is used.
        let mut indices = Vec::with_capacity(slot.index_capacity as usize);
        indices.extend(geometry.indices.iter().map(|i| i + slot.vertex_base));
        indices.resize(slot.index_capacity as usize, slot.vertex_base);

        self.bounds = union(self.bounds, Vertex::bounds(&geometry.vertices));
        self.mesh
            .patch_vertices(gpu, slot.vertex_base, &geometry.vertices);
        self.mesh.patch_indices(gpu, slot.index_base, &indices);
        true
    }

    /// Lays the whole surface out afresh and writes it in one go.
    ///
    /// Distinct from [`SurfaceGeometry::rebuild`], which re-meshes from the
    /// document; this only re-arranges geometry already in hand.
    ///
    /// The slow path, and the one that reclaims holes. Spans are allocated
    /// with the same headroom a patch would give them, so the strokes right
    /// after a rebuild stay incremental rather than immediately re-homing
    /// everything.
    fn lay_out(&mut self, gpu: &Gpu) {
        self.prune_duplicates();
        self.lay_out_prepared(gpu);
    }

    /// Allocate and upload geometry after its duplicate pass has completed.
    fn lay_out_prepared(&mut self, gpu: &Gpu) {
        let vertices_needed = self.vertex_count() + self.keys.len() * 64;
        let indices_needed = self.triangle_count() * 3 + self.keys.len() * 64;
        // Twice the need where the device allows it, so the strokes after a
        // rebuild stay incremental; the bare need where it does not. A surface
        // that fits neither is refused whole rather than attempted: a subtool
        // scaled up a few times is ten million vertices at the field's fixed
        // resolution, and `create_buffer` past the device's ceiling used to end
        // the session. What is on screen stays stale and `over_budget` says
        // so, and the composition root drops to the coarse level.
        let Some((vertex_slots, index_slots)) = [2usize, 1]
            .into_iter()
            .map(|headroom| {
                (
                    (vertices_needed * headroom).max(1024),
                    (indices_needed * headroom).max(1024),
                )
            })
            .find(|(vertices, indices)| GpuMesh::fits(gpu, *vertices, *indices))
        else {
            eprintln!(
                "a surface of {} vertices is more than the graphics device can hold at this \
                 level of detail",
                self.vertex_count()
            );
            self.over_budget = true;
            return;
        };
        self.over_budget = false;
        if !self.mesh.reserve(gpu, vertex_slots, index_slots) {
            self.over_budget = true;
            return;
        }
        self.layout = SlotMap::new(vertex_slots as u32, index_slots as u32);
        self.bounds = None;
        self.touched.clear();

        let keys: Vec<BrickKey> = self.keys.keys().copied().collect();
        for key in keys {
            // A fresh layout has room for everything it was sized from, so a
            // refusal here would be a sizing bug rather than a full buffer.
            let placed = self.patch(gpu, key);
            debug_assert!(placed, "a fresh layout ran out of room");
        }
        self.mesh.set_index_count(self.layout.index_count());
        self.mesh.set_bounds(self.bounds);
        self.relayout = false;
        self.dirty = false;
    }

    /// Drops triangles this store holds under more than one key.
    ///
    /// The engine attributes a triangle straddling two bricks to the *lowest
    /// requested* key owning a corner, and says as much: it "may move to
    /// another key's share when a later request names a different set — its
    /// content is identical wherever it lands, so keeping either copy is
    /// right". Keeping *both* is what a store filed per key does by default,
    /// and a long session accumulates them — measured at 55 of 597,521
    /// triangles.
    ///
    /// 55, not the 11,333 an earlier version of this counted. That version
    /// keyed on the three positions and so counted every pair of triangles
    /// sitting at the same three points, most of which are not one triangle
    /// twice: they are two meshings of one patch of surface with different
    /// brick neighbourhoods, carrying different normals. Dropping one of those
    /// is not tidying, it is choosing a shading, and it made a settled surface
    /// differ from a full re-mesh.
    ///
    /// A true duplicate costs upload and draw rather than correctness: the
    /// copies agree in every attribute, so whichever one the depth test keeps
    /// draws the same pixels. This runs during layout rather than ordinary
    /// patches, as required for a host retaining independently meshed bricks.
    /// Hashing complete corners for every triangle was itself expensive, so
    /// the lookup interns exact vertex values and compares triples of IDs.
    fn prune_duplicates(&mut self) {
        prune_exact_triangles(&mut self.keys);
    }

    /// Whether independently re-meshed bricks left the same triangle in more
    /// than one key.
    ///
    /// The copies can carry different normals because each subset was meshed
    /// with a different neighbourhood. Drawing both lets depth ordering choose
    /// the shading and appears as persistent pits or specks. A whole-surface
    /// rebuild assigns each triangle once, so this is the cue for the
    /// composition root to take that slower path after a gesture.
    pub fn has_coincident_triangles(&self) -> bool {
        contains_coincident_triangles(&self.keys)
    }

    /// Shows what a lattice cage would do to the drawn surface.
    ///
    /// The field route has no cheap way to preview itself — applying a cage
    /// writes a deformer into the document as an undoable edit and refills the
    /// layer's whole brick region, 68.8 ms measured — so the preview is done
    /// *here*, by moving the vertices the viewport already holds.
    ///
    /// The engine supplies the warp (`clay_mesh_lattice_displacement`, which
    /// exists for exactly this) so no lattice arithmetic is written twice. It
    /// is the *forward* map where the field's own deformer is the inverse one;
    /// measured against the engine's own result on a cage spanning ±1.1, the
    /// two agree to **0.6% of the drag** for drags up to a quarter of the
    /// box's half-width and diverge for very large ones. That is a preview's
    /// error budget rather than an edit's — what lands on Deformar is the
    /// engine's, computed the engine's way.
    pub fn preview_cage(&mut self, gpu: &Gpu, document: &ClayDocument) {
        // Every stored vertex in one call: the warp is an FFI hop per point,
        // and asking per key would pay the crossing a thousand times over.
        let mut keys: Vec<BrickKey> = Vec::new();
        let mut points: Vec<[f32; 3]> = Vec::new();
        for (key, geometry) in self.keys.iter() {
            if geometry.vertices.is_empty() {
                continue;
            }
            let rest = self
                .cage_rest
                .get(key)
                .cloned()
                .unwrap_or_else(|| geometry.vertices.iter().map(|v| v.position).collect());
            points.extend_from_slice(&rest);
            keys.push(*key);
            self.cage_rest.entry(*key).or_insert(rest);
        }
        let Some(warp) = document.cage_warp(&points) else {
            // No cage, an untouched one, or a mesh layer — which previews by
            // being deformed rather than by being displaced here.
            return self.clear_cage_preview(gpu);
        };

        let mut at = 0;
        for key in keys {
            let Some(geometry) = self.keys.get_mut(&key) else {
                continue;
            };
            let rest = &self.cage_rest[&key];
            for (vertex, was) in geometry.vertices.iter_mut().zip(rest) {
                let by = warp[at];
                vertex.position = std::array::from_fn(|axis| was[axis] + by[axis]);
                at += 1;
            }
            self.touched.insert(key);
        }
        if self.touched.is_empty() {
            return;
        }
        self.dirty = true;
        self.upload(gpu);
    }

    /// Puts the surface back where it was, if a preview moved it.
    pub fn clear_cage_preview(&mut self, gpu: &Gpu) {
        if self.cage_rest.is_empty() {
            return;
        }
        for (key, rest) in std::mem::take(&mut self.cage_rest) {
            let Some(geometry) = self.keys.get_mut(&key) else {
                continue;
            };
            for (vertex, was) in geometry.vertices.iter_mut().zip(&rest) {
                vertex.position = *was;
            }
            self.touched.insert(key);
        }
        self.dirty = true;
        self.upload(gpu);
    }

    /// Re-samples the mask across the whole stored surface, and uploads it.
    ///
    /// The mask's own path, separate from `sync`, because painting one moves
    /// no clay: it dirties no brick, so the incremental re-mesh has nothing to
    /// re-mesh and would leave the frozen region undrawn. The caller watches
    /// [`ClayDocument::mask_revision`] and calls this when it moves.
    ///
    /// The whole surface rather than a subset. A mask operation — invert,
    /// expand, the bounded complement — can change any cell of it, and the
    /// mask keeps no dirty set of its own to narrow it down.
    pub fn refresh_mask(&mut self, gpu: &Gpu, document: &ClayDocument) {
        for (key, geometry) in self.keys.iter_mut() {
            if geometry.vertices.is_empty() {
                continue;
            }
            sample_mask(document, &mut geometry.vertices);
            self.touched.insert(*key);
        }
        if self.touched.is_empty() {
            return;
        }
        self.dirty = true;
        self.upload(gpu);
    }

    /// Lays the settled surface out again from the document's bricks.
    ///
    /// **This used to mesh the whole field through `clay_document_mesh`.** The
    /// brick mesher could leave isolated artifacts even after a full rebuild —
    /// sliver triangles whose face normals are cross products of near-parallel
    /// edges, which shade black — and a whole-field mesh has none of them
    /// because it is a different mesher. ClayCore #549 fixed the slivers at
    /// their source in v0.113.0: 2,297 in 83,464 triangles, now none. So the
    /// workaround went with the defect it was hiding.
    ///
    /// What that costs is negative twice over. The whole-field path has no
    /// region to cull against, so it evaluates every grab on the layer for
    /// every sample, and its cost tracks the *document* rather than the edit:
    /// ClayCore measure 2.8 ms at one dab against 26.2 ms at 48, where the
    /// per-brick path goes 4.2 ms to 6.3 ms over the same span. And because it
    /// replaced every key at once it forced a full GPU relayout, 13.96 ms,
    /// where a per-key rebuild patches the slots it touched.
    ///
    /// The second saving is the one that was not in the issue. A whole-document
    /// mesh cannot be patched incrementally — the store held one giant key
    /// under `[i32::MIN; 3]` and the brick keys the engine reports dirty do not
    /// address it — so `clean_override` made the *next* `sync` throw it away
    /// and rebuild every brick anyway. The field was meshed twice per stroke:
    /// once whole on release, once per brick on the next edit. Rebuilding from
    /// bricks here means the store is already what a sync can patch, and that
    /// second rebuild is simply gone, along with the flag that scheduled it.
    pub fn settle(&mut self, gpu: &Gpu, document: &mut ClayDocument) -> Result<(), ClayError> {
        let settle_started = std::time::Instant::now();
        let outcome = self.rebuild(gpu, document);

        // Taken here rather than left to `sync`. The store now holds exactly
        // what the document's bricks say, so the epoch it was built from is
        // this one — and a stale epoch would send the next sync through a
        // whole rebuild to reach the state it is already in.
        self.surface_epoch = document.surface_epoch();

        // An empty field is not a failure, it is an empty surface, and
        // `rebuild_at` reaches it correctly: it clears the store first and
        // meshes per key, so no keys means no triangles. The whole-document
        // path had to be told, because `clay_document_mesh` REFUSES an empty
        // document rather than returning an empty mesh, and it cleared `keys`
        // only after succeeding — so a field that had just gone left the
        // refusal propagating and the old surface standing in the GPU buffers.
        // That case is now unreachable rather than handled.
        let route = if self.keys.is_empty() {
            SettleRoute::Empty
        } else if self.detail == Detail::Reduced {
            SettleRoute::Coarse
        } else {
            SettleRoute::Bricks
        };
        self.last_settle = Some(SettleCost {
            route,
            // The per-key path times itself into `last_cost`'s fields, so the
            // engine's share is already known and is not re-timed.
            engine_mesh_time: self.last_engine_mesh,
            read_time: self.last_read,
            upload_time: std::time::Duration::ZERO,
            total_time: settle_started.elapsed(),
            triangles: self.triangle_count(),
            vertices: self.vertex_count(),
        });
        outcome
    }

    /// Rebuilds every key from scratch, at the level last asked for.
    ///
    /// The compaction the specification calls for: per-key slots accumulate
    /// empty entries as the surface moves, and this is where they go. Off the
    /// interaction path — it costs a full re-mesh.
    pub fn rebuild(&mut self, gpu: &Gpu, document: &mut ClayDocument) -> Result<(), ClayError> {
        self.rebuild_at(gpu, document, self.requested)
    }

    /// The level currently on screen, which is not always the one asked for.
    pub fn detail(&self) -> Detail {
        self.detail
    }

    /// Draws the surface at `detail`, rebuilding when that is not the request.
    ///
    /// Returns whether anything was rebuilt. Switching level is a full
    /// re-mesh, which is affordable only because it is rare: the policy's
    /// hysteresis band is what stops a resting camera paying it every frame.
    /// Incremental syncing happens at full resolution only.
    pub fn set_detail(
        &mut self,
        gpu: &Gpu,
        document: &mut ClayDocument,
        detail: Detail,
    ) -> Result<bool, ClayError> {
        if detail == self.requested {
            return Ok(false);
        }
        self.requested = detail;
        self.rebuild_at(gpu, document, detail)?;
        Ok(true)
    }

    /// Tries the requested level again, for when the mips it wanted have since
    /// been built.
    ///
    /// A request for the coarse surface made before any mip existed draws full
    /// resolution instead. That is the right answer at the time and the wrong
    /// one once a gesture ends and the mips go up, so the end of a gesture
    /// asks again rather than leaving the fallback in place until the camera
    /// happens to move.
    pub fn reapply_detail(
        &mut self,
        gpu: &Gpu,
        document: &mut ClayDocument,
    ) -> Result<bool, ClayError> {
        if self.detail == self.requested {
            return Ok(false);
        }
        self.rebuild_at(gpu, document, self.requested)?;
        Ok(self.detail == self.requested)
    }

    /// Which keys, level and shading draw `detail`.
    ///
    /// Falls back to full resolution when the coarse surface is not there to
    /// draw: no mip has been built yet, or every coarse brick still has a
    /// child the last stroke left dirty. Drawing the model at the wrong size
    /// or not at all would both be worse than drawing it slowly.
    fn level_for(
        &self,
        document: &ClayDocument,
        detail: Detail,
    ) -> Result<(Vec<BrickKey>, i32, Shading), ClayError> {
        // Face-shaded while a live gesture is drawing the surface: the
        // preview's cache has no document behind it to evaluate a gradient
        // through, and the gesture's end lays the surface out again anyway.
        let live = document.live_gesture_is_open();
        // And never coarse. A mip belongs to the cache that built it, and the
        // preview's has none — asking the document for coarse keys and then
        // meshing them out of the preview would name a level that was never
        // built there.
        if detail == Detail::Reduced && !live {
            let coarse = document.drawable_coarse_keys()?;
            if !coarse.is_empty() {
                // Gradient-shaded, which level 1 could not do until ClayCore
                // #550. It used to REFUSE gradient normals rather than
                // downgrade them, so the coarse surface was face-shaded by
                // construction — and face normals on a coarse lattice measure
                // up to 84.78 degrees off the field, which is what made
                // drawing the coarse level a visible downgrade rather than a
                // cheaper route to the same picture.
                return Ok((coarse, 1, Shading::Full));
            }
        }
        let shading = if live { Shading::Fast } else { Shading::Full };
        Ok((document.drawn_cache().0.surface_bricks()?, 0, shading))
    }

    /// Rebuilds every key from scratch at `detail`.
    fn rebuild_at(
        &mut self,
        gpu: &Gpu,
        document: &mut ClayDocument,
        detail: Detail,
    ) -> Result<(), ClayError> {
        let (keys, lod, shading) = self.level_for(document, detail)?;
        self.keys.clear();
        self.touched.clear();
        // The spans described geometry that has just been discarded, so the
        // layout cannot be patched onto what replaces it.
        self.relayout = true;
        // What is on screen, which is the fallback rather than the request
        // when there was no coarse surface to draw.
        self.detail = if lod == 1 {
            Detail::Reduced
        } else {
            Detail::Full
        };
        if keys.is_empty() {
            self.mesh.upload(gpu, &[], &[]);
            self.layout = SlotMap::default();
            self.needs_settle = false;
            document.take_dirty_keys();
            return Ok(());
        }
        self.remesh(document, &keys, None, shading, lod)?;
        self.upload(gpu);
        document.record_geometry(self.triangle_count(), self.vertex_count(), self.detail);
        // Drained, because everything it could name has just been meshed.
        //
        // Left undrained, the pending set from building the starting form —
        // the whole layer — survived into the first `sync` of the session,
        // which then dilated it and re-meshed 5832 keys for one dab. That was
        // the 240 ms every tool reported as its worst segment, and it was the
        // same 240 ms for the mask tool, which re-meshes nothing at all.
        document.take_dirty_keys();
        Ok(())
    }

    /// Every triangle this store holds, as whole vertices, duplicates kept.
    ///
    /// Diagnostic, and deliberately not the quantised positions below:
    /// rounding is what a comparison between two independently meshed stores
    /// needs and what a check on pruning must not use. Asking whether pruning
    /// dropped anything in a form coarser than the one it matched on can only
    /// answer no -- first with a tolerance it did not have, and then with the
    /// positions alone, which is how a prune that changed the shading of a
    /// seam went on reporting that it had lost nothing.
    pub fn stored_triangles_exact(&self) -> Vec<[[u32; 10]; 3]> {
        self.keys
            .values()
            .flat_map(|geometry| {
                geometry.indices.chunks_exact(3).filter_map(|t| {
                    let mut corners = [
                        geometry.vertices.get(t[0] as usize)?,
                        geometry.vertices.get(t[1] as usize)?,
                        geometry.vertices.get(t[2] as usize)?,
                    ]
                    .map(vertex_key);
                    corners.sort_unstable();
                    Some(corners)
                })
            })
            .collect()
    }

    /// The triangles stored against each key, quantised to world positions.
    ///
    /// Diagnostic. The per-key split is where an incremental re-mesh can
    /// silently disagree with a full one, and comparing two of these says
    /// exactly which key lost or gained what — which a rendered difference
    /// cannot.
    pub fn stored_triangles(&self) -> std::collections::BTreeMap<BrickKey, Vec<[[i32; 3]; 3]>> {
        self.keys
            .iter()
            // A key with no triangles draws nothing, and whether it holds an
            // empty slot or no slot at all is bookkeeping rather than
            // geometry. Comparing those would report differences a viewer
            // could never see.
            .filter(|(_, geometry)| !geometry.indices.is_empty())
            .map(|(key, geometry)| {
                let mut triangles: Vec<[[i32; 3]; 3]> = geometry
                    .indices
                    .chunks_exact(3)
                    .filter_map(|t| {
                        let mut corners = [
                            geometry.vertices.get(t[0] as usize)?.position,
                            geometry.vertices.get(t[1] as usize)?.position,
                            geometry.vertices.get(t[2] as usize)?.position,
                        ]
                        .map(|p| p.map(|c| (c * 4096.0).round() as i32));
                        corners.sort_unstable();
                        Some(corners)
                    })
                    .collect();
                triangles.sort_unstable();
                (*key, triangles)
            })
            .collect()
    }

    /// Lays the buffer out again, which is where duplicates are pruned.
    ///
    /// Exposed for the test that measures them; the application reaches this
    /// through `upload` when the drawn range has gone too far to holes.
    pub fn settle_layout(&mut self, gpu: &Gpu) {
        self.lay_out(gpu);
    }
}

/// Writes each vertex's mask weight, when there is a mask to read.
///
/// Free-standing so both the incremental path and the whole-surface refresh
/// spell it the same way, and so the "no mask" case costs one `Option` check
/// rather than a pass over the vertices.
fn sample_mask(document: &ClayDocument, vertices: &mut [Vertex]) {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
    match document.mask_at(&positions) {
        Some(weights) => {
            for (vertex, weight) in vertices.iter_mut().zip(weights) {
                vertex.mask = weight;
            }
        }
        // Nothing frozen. Cleared rather than left, because a mask that was
        // cleared has to stop being drawn.
        None => {
            for vertex in vertices.iter_mut() {
                vertex.mask = 0.0;
            }
        }
    }
}

/// A replacement covering all stored triangles is one consistent request,
/// including when empty bookkeeping entries remain under other keys.
fn retains_unreplaced_triangles(
    keys: &HashMap<BrickKey, KeyGeometry>,
    replace: Option<&std::collections::HashSet<BrickKey>>,
) -> bool {
    replace.is_some_and(|replace| {
        keys.iter()
            .any(|(key, geometry)| !geometry.indices.is_empty() && !replace.contains(key))
    })
}

/// Reclaim storage a complete remesh would have discarded, preserving each
/// surviving vertex bit and triangle order. Scratch space is reused per key.
fn compact_release_geometry(geometries: &mut HashMap<BrickKey, KeyGeometry>) {
    prune_exact_triangles(geometries);
    geometries.retain(|_, geometry| !geometry.indices.is_empty());
    let mut remap = Vec::new();
    for geometry in geometries.values_mut() {
        compact_referenced_vertices(geometry, &mut remap);
        geometry.vertices.shrink_to_fit();
        geometry.indices.shrink_to_fit();
    }
}

fn compact_referenced_vertices(geometry: &mut KeyGeometry, remap: &mut Vec<u32>) {
    remap.clear();
    remap.resize(geometry.vertices.len(), u32::MAX);
    for &index in &geometry.indices {
        remap[index as usize] = 0;
    }
    if remap.iter().all(|&index| index != u32::MAX) {
        return;
    }
    let mut old = 0;
    let mut next = 0usize;
    geometry.vertices.retain(|_| {
        let index = old;
        old += 1;
        if remap[index] == u32::MAX {
            return false;
        }
        // A retained vertex's new index cannot exceed its old u32 index.
        remap[index] = next as u32;
        next += 1;
        true
    });
    for index in &mut geometry.indices {
        *index = remap[*index as usize];
    }
}

/// Match complete vertex bits once, then use compact exact triangle keys.
fn prune_exact_triangles(geometries: &mut HashMap<BrickKey, KeyGeometry>) {
    prune_exact_triangles_with_hasher(geometries, ahash::RandomState::new());
}

fn triangle_ids_fit_packed(vertex_count: usize) -> bool {
    vertex_count <= u32::MAX as usize
}

fn packed_triangle_ids(ids: [usize; 3]) -> u128 {
    debug_assert!(ids.iter().all(|&id| id <= u32::MAX as usize));
    ids[0] as u128 | ((ids[1] as u128) << 32) | ((ids[2] as u128) << 64)
}

fn prune_exact_triangles_with_hasher<S: std::hash::BuildHasher + Clone>(
    geometries: &mut HashMap<BrickKey, KeyGeometry>,
    state: S,
) {
    let vertex_count = geometries.values().map(|g| g.vertices.len()).sum();
    // Every assigned ID is smaller than the input vertex count. Keep the
    // original representation when that bound cannot prove lossless packing.
    if triangle_ids_fit_packed(vertex_count) {
        prune_triangles_with_keys(geometries, state, vertex_count, packed_triangle_ids);
    } else {
        prune_triangles_with_keys(geometries, state, vertex_count, std::convert::identity);
    }
}

// Borrow immutable vertex storage only for the duration of pruning. Equality
// uses complete float bits, so separate allocations, NaNs and signed zero
// behave exactly like the owned vertex_key representation.
#[derive(Clone, Copy)]
struct VertexBits<'a>(&'a Vertex);

impl std::hash::Hash for VertexBits<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&vertex_key(self.0), state);
    }
}

impl PartialEq for VertexBits<'_> {
    fn eq(&self, other: &Self) -> bool {
        vertex_key(self.0) == vertex_key(other.0)
    }
}

impl Eq for VertexBits<'_> {}

fn prune_triangles_with_keys<S, K>(
    geometries: &mut HashMap<BrickKey, KeyGeometry>,
    state: S,
    vertex_count: usize,
    triangle_key: impl Fn([usize; 3]) -> K,
) where
    S: std::hash::BuildHasher + Clone,
    K: Eq + std::hash::Hash,
{
    let triangle_count: usize = geometries.values().map(|g| g.indices.len() / 3).sum();
    if triangle_count == 0 {
        return;
    }
    let mut vertex_ids: HashMap<VertexBits<'_>, usize, S> =
        HashMap::with_capacity_and_hasher(vertex_count, state.clone());
    let mut seen = std::collections::HashSet::with_capacity_and_hasher(triangle_count, state);
    let mut entries: Vec<_> = geometries.iter_mut().collect();
    entries.sort_unstable_by_key(|(key, _)| **key);
    for (_, geometry) in entries {
        let KeyGeometry { vertices, indices } = geometry;
        if indices.is_empty() {
            continue;
        }
        let ids: Vec<_> = vertices
            .iter()
            .map(|vertex| {
                let next = vertex_ids.len();
                *vertex_ids.entry(VertexBits(vertex)).or_insert(next)
            })
            .collect();
        let mut kept = 0;
        for read in (0..indices.len() / 3 * 3).step_by(3) {
            let mut corners = [
                ids[indices[read] as usize],
                ids[indices[read + 1] as usize],
                ids[indices[read + 2] as usize],
            ];
            corners.sort_unstable();
            if seen.insert(triangle_key(corners)) {
                if read != kept {
                    indices.copy_within(read..read + 3, kept);
                }
                kept += 3;
            }
        }
        indices.truncate(kept);
    }
}

fn position_key(vertex: &Vertex) -> [u32; 3] {
    vertex.position.map(f32::to_bits)
}

fn contains_coincident_triangles(keys: &HashMap<BrickKey, KeyGeometry>) -> bool {
    let mut seen = std::collections::HashSet::new();
    for geometry in keys.values() {
        for triangle in geometry.indices.chunks_exact(3) {
            let mut positions = [
                position_key(&geometry.vertices[triangle[0] as usize]),
                position_key(&geometry.vertices[triangle[1] as usize]),
                position_key(&geometry.vertices[triangle[2] as usize]),
            ];
            positions.sort_unstable();
            if !seen.insert(positions) {
                return true;
            }
        }
    }
    false
}

/// Read the drawn cache's mesh in world coordinates and sample its mask.
fn read_drawn_mesh(
    mesh: Option<&Mesh>,
    offset: [f32; 3],
    document: &ClayDocument,
) -> Result<(Vec<Vertex>, Vec<u32>), ClayError> {
    let (mut vertices, indices) = match mesh {
        Some(mesh) => read_mesh(mesh)?,
        None => (Vec::new(), Vec::new()),
    };
    // The preview lattice is the cache's lattice in a world translated by
    // its own origin, so the translation is undone here — on the vertices
    // and nowhere else, which is what keeps every reader of this geometry
    // (bounds, picking, the mask below) working in one space.
    if offset != [0.0; 3] {
        for vertex in &mut vertices {
            for (axis, by) in offset.iter().enumerate() {
                vertex.position[axis] += by;
            }
        }
    }
    // The frozen region, on the vertices this re-mesh just produced.
    //
    // Only these, which is the dirty subset: a dab that re-meshes twenty
    // bricks samples twenty bricks' worth rather than the whole surface.
    // A mask that *changes* is the other direction and is
    // `refresh_mask`'s job.
    sample_mask(document, &mut vertices);
    Ok((vertices, indices))
}

/// Reads an engine mesh into the renderer's vertex layout in one pass.
fn read_mesh(mesh: &Mesh) -> Result<(Vec<Vertex>, Vec<u32>), ClayError> {
    let count = mesh.vertex_count();
    // Empty meshes have no attributes, so do not request a vertex layout.
    if count == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let has_colors = mesh.colors().is_some();
    let mut vertices = vec![
        Vertex {
            position: [0.0; 3],
            normal: [0.0; 3],
            color: [1.0; 3],
            mask: 0.0,
        };
        count
    ];
    // Vertex is Pod with the renderer's declared layout. The engine writes
    // only requested attributes; default color and mask remain initialized.
    mesh.copy_vertices(
        VertexLayout {
            stride: Some(Vertex::STRIDE as u32),
            position_offset: Some(Vertex::POSITION_OFFSET as i32),
            normal_offset: Some(Vertex::NORMAL_OFFSET as i32),
            color_offset: has_colors.then_some(Vertex::COLOR_OFFSET as i32),
            uv_offset: None,
        },
        bytemuck::cast_slice_mut(&mut vertices),
    )?;
    // Preserve the original reader's little-endian decoding on other hosts.
    #[cfg(target_endian = "big")]
    for vertex in &mut vertices {
        let decode = |value: f32| f32::from_bits(value.to_bits().swap_bytes());
        vertex.position = vertex.position.map(decode);
        vertex.normal = vertex.normal.map(decode);
        if has_colors {
            vertex.color = vertex.color.map(decode);
        }
    }
    let mut indices = vec![0u32; mesh.index_count()];
    mesh.copy_indices(&mut indices)?;
    Ok((vertices, indices))
}

/// Kept so the document type is visible to readers of the imports.
const _: fn(&Document) -> bool = |_| true;

/// Both boxes, or whichever one exists.
fn union(
    a: Option<([f32; 3], [f32; 3])>,
    b: Option<([f32; 3], [f32; 3])>,
) -> Option<([f32; 3], [f32; 3])> {
    match (a, b) {
        (Some(a), Some(b)) => Some((
            [a.0[0].min(b.0[0]), a.0[1].min(b.0[1]), a.0[2].min(b.0[2])],
            [a.1[0].max(b.1[0]), a.1[1].max(b.1[1]), a.1[2].max(b.1[2])],
        )),
        (some, None) | (None, some) => some,
    }
}

/// Makes a span of indices draw nothing.
///
/// Degenerate triangles rather than a shorter draw: the surface is one range,
/// and a hole in the middle of it has to be covered by something.
fn blank(mesh: &mut GpuMesh, gpu: &Gpu, (first, last): (u32, u32)) {
    mesh.patch_indices(gpu, first, &vec![0; (last - first) as usize]);
}

/// A vertex as an exact value, for deciding whether two triangles are the
/// same one.
///
/// Every attribute the shader reads, in bits: two vertices that differ
/// anywhere here draw differently, and a dedupe that treats them as one
/// changes the picture.
fn vertex_key(vertex: &Vertex) -> [u32; 10] {
    let [px, py, pz] = vertex.position;
    let [nx, ny, nz] = vertex.normal;
    let [r, g, b] = vertex.color;
    [px, py, pz, nx, ny, nz, r, g, b, vertex.mask].map(f32::to_bits)
}

#[cfg(test)]
mod tests {
    use super::{
        contains_coincident_triangles, prune_exact_triangles, vertex_key, ClayError, HashMap,
        KeyGeometry, Mesh, Vertex, VertexLayout,
    };

    fn read_mesh_reference(mesh: &Mesh) -> Result<(Vec<Vertex>, Vec<u32>), ClayError> {
        let count = mesh.vertex_count();
        // A brick with no surface in it comes back as a mesh with no attributes at
        // all, and the engine refuses a layout naming positions on it — "the
        // layout names positions, which this mesh does not carry" — which is not
        // a failure to mesh, it is nothing to mesh.
        if count == 0 {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut bytes = vec![0u8; count * Vertex::STRIDE];

        let has_colors = mesh.colors().is_some();
        if !has_colors {
            // The engine refuses a layout naming an attribute the mesh lacks, so
            // white is written here and the copy writes around it.
            for vertex in bytes.chunks_exact_mut(Vertex::STRIDE) {
                for channel in 0..3 {
                    let at = Vertex::COLOR_OFFSET + channel * 4;
                    vertex[at..at + 4].copy_from_slice(&1.0f32.to_le_bytes());
                }
            }
        }

        mesh.copy_vertices(
            VertexLayout {
                stride: Some(Vertex::STRIDE as u32),
                position_offset: Some(Vertex::POSITION_OFFSET as i32),
                normal_offset: Some(Vertex::NORMAL_OFFSET as i32),
                color_offset: has_colors.then_some(Vertex::COLOR_OFFSET as i32),
                uv_offset: None,
            },
            &mut bytes,
        )?;

        let read = |v: &[u8], offset: usize| -> [f32; 3] {
            std::array::from_fn(|i| {
                let at = offset + i * 4;
                f32::from_le_bytes(v[at..at + 4].try_into().unwrap())
            })
        };
        let vertices = bytes
            .chunks_exact(Vertex::STRIDE)
            .map(|v| Vertex {
                position: read(v, Vertex::POSITION_OFFSET),
                normal: read(v, Vertex::NORMAL_OFFSET),
                color: read(v, Vertex::COLOR_OFFSET),
                mask: 0.0,
            })
            .collect();

        let mut indices = vec![0u32; mesh.index_count()];
        mesh.copy_indices(&mut indices)?;
        Ok((vertices, indices))
    }

    #[test]
    fn direct_readback_preserves_colored_uncolored_and_empty_meshes() {
        use clayspace_engine::claycore::{
            BrickCache, BrickConfig, BrickMeshParams, Document, Item,
        };
        let mut document = Document::new().unwrap();
        let layer = document.add_sdf_layer("readback").unwrap();
        document
            .add_item(layer, &Item::sphere(1.0).unwrap())
            .unwrap();
        for colors in [false, true] {
            let mut cache = BrickCache::new(BrickConfig {
                dim: 8,
                voxel_size: 0.1,
                band_voxels: 3,
                memory_budget: None,
                colors,
            })
            .unwrap();
            let params = BrickMeshParams {
                colors,
                ..Default::default()
            };
            let (empty, _) = cache.mesh(Some(&document), params, &[]).unwrap();
            let (vertices, indices) = super::read_mesh(&empty).unwrap();
            assert!(vertices.is_empty() && indices.is_empty());
            cache.mark_dirty_layer(&document, layer).unwrap();
            assert!(cache.refill_all(&document, None, 256).unwrap() > 0);
            let (mesh, _) = cache.mesh(Some(&document), params, &[]).unwrap();
            assert!(!mesh.is_empty());
            assert_eq!(mesh.colors().is_some(), colors);
            let expected = read_mesh_reference(&mesh).unwrap();
            let actual = super::read_mesh(&mesh).unwrap();
            assert_eq!(actual.1, expected.1);
            assert_eq!(
                actual.0.iter().map(vertex_key).collect::<Vec<_>>(),
                expected.0.iter().map(vertex_key).collect::<Vec<_>>()
            );
            assert!(actual.0.iter().all(|v| v.mask.to_bits() == 0));
        }
    }

    #[test]
    #[ignore = "informational release timing; no portable timing threshold"]
    fn profile_mesh_readback() {
        use clayspace_engine::claycore::{
            BrickCache, BrickConfig, BrickMeshParams, Document, Item,
        };
        let mut document = Document::new().unwrap();
        let layer = document.add_sdf_layer("readback timing").unwrap();
        document
            .add_item(layer, &Item::sphere(1.0).unwrap())
            .unwrap();
        eprintln!("readback,voxel_size,colors,repeat,variant,ms");
        for voxel_size in [0.2, 0.1, 0.025] {
            for colors in [false, true] {
                let mut cache = BrickCache::new(BrickConfig {
                    dim: 8,
                    voxel_size,
                    band_voxels: 3,
                    memory_budget: None,
                    colors,
                })
                .unwrap();
                cache.mark_dirty_layer(&document, layer).unwrap();
                cache.refill_all(&document, None, 256).unwrap();
                let (mesh, _) = cache
                    .mesh(
                        Some(&document),
                        BrickMeshParams {
                            colors,
                            ..Default::default()
                        },
                        &[],
                    )
                    .unwrap();
                compare_readback_timing(&mesh, voxel_size, colors);
            }
        }
    }

    fn compare_readback_timing(mesh: &Mesh, voxel_size: f32, colors: bool) {
        let expected = read_mesh_reference(mesh).unwrap();
        let expected_bits: Vec<_> = expected.0.iter().map(vertex_key).collect();
        for repeat in 0..7 {
            for slot in 0..2 {
                let variant = (repeat + slot) % 2;
                let start = std::time::Instant::now();
                let actual = if variant == 0 {
                    read_mesh_reference(mesh)
                } else {
                    super::read_mesh(mesh)
                }
                .unwrap();
                let ms = start.elapsed().as_secs_f64() * 1000.0;
                assert_eq!(actual.1, expected.1);
                assert_eq!(
                    actual.0.iter().map(vertex_key).collect::<Vec<_>>(),
                    expected_bits
                );
                eprintln!("readback,{voxel_size},{colors},{repeat},{variant},{ms:.6}");
            }
        }
    }

    #[test]
    fn direct_readback_preserves_missing_normal_errors() {
        let mesh = super::Mesh::from_triangles(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[0, 1, 2],
        )
        .unwrap();
        assert!(mesh.normals().is_none());
        assert!(read_mesh_reference(&mesh).is_err());
        assert!(super::read_mesh(&mesh).is_err());
    }

    fn remap_fixture() -> Vec<Vertex> {
        let words = [0, 0x8000_0000, 0x7fc0_0001, 0x3f80_0000, 0x7fa0_0002];
        (0..32)
            .map(|i| {
                let values: [f32; 10] =
                    std::array::from_fn(|j| f32::from_bits(words[(i + j) % words.len()]));
                Vertex {
                    position: [values[0], values[1], values[2]],
                    normal: [values[3], values[4], values[5]],
                    color: [values[6], values[7], values[8]],
                    mask: values[9],
                }
            })
            .collect()
    }

    fn geometry_bits(geometry: &KeyGeometry) -> (Vec<[u32; 10]>, Vec<u32>) {
        (
            geometry.vertices.iter().map(vertex_key).collect(),
            geometry.indices.clone(),
        )
    }

    #[test]
    fn dense_vertex_remapping_preserves_bits_order_and_reused_bricks() {
        let vertices = remap_fixture();
        let groups = [
            vec![[9, 1, 5], [5, 2, 9]],
            vec![[1, 6, 1]],
            vec![],
            vec![[31, 0, 31]],
        ];
        let mut remap = super::VertexRemap::new(vertices.len());
        let mut expected = KeyGeometry::default();
        let mut actual = KeyGeometry::default();
        for order in [[0, 1, 2, 3], [3, 2, 1, 0]] {
            for at in order {
                super::write_key_geometry_sparse(&mut expected, &vertices, &groups[at]);
                super::write_key_geometry(&mut actual, &vertices, &groups[at], &mut remap);
                assert_eq!(geometry_bits(&actual), geometry_bits(&expected));
                assert!(remap.indices.iter().all(Option::is_none));
                assert!(remap.touched.is_empty());
            }
        }
    }

    #[test]
    fn dense_vertex_remapping_preserves_invalid_placeholders_and_empty_replacements() {
        let vertices = remap_fixture();
        let mut remap = super::VertexRemap::new(vertices.len());
        let mut actual = KeyGeometry::default();
        let triangles = [
            [u32::MAX, 0, u32::MAX],
            [vertices.len() as u32, 0, u32::MAX],
        ];
        super::write_key_geometry(&mut actual, &vertices, &triangles, &mut remap);
        let placeholder = vertex([0.0; 3], [0.0, 1.0, 0.0]);
        let expected = KeyGeometry {
            vertices: vec![placeholder, vertices[0], placeholder],
            indices: vec![0, 1, 0, 2, 1, 0],
        };
        assert_eq!(geometry_bits(&actual), geometry_bits(&expected));
        super::write_key_geometry(&mut actual, &vertices, &[], &mut remap);
        assert!(actual.vertices.is_empty());
        assert!(actual.indices.is_empty());

        let mut empty = super::VertexRemap::new(0);
        super::write_key_geometry(&mut actual, &[], &[[7, 7, 7]], &mut empty);
        assert_eq!(
            geometry_bits(&actual),
            (vec![vertex_key(&placeholder)], vec![0, 0, 0])
        );
    }

    fn prune_reference(geometries: &mut HashMap<super::BrickKey, KeyGeometry>) {
        let mut keys: Vec<_> = geometries.keys().copied().collect();
        keys.sort_unstable();
        let mut seen: std::collections::HashSet<[[u32; 10]; 3]> = std::collections::HashSet::new();
        for key in keys {
            let Some(geometry) = geometries.get_mut(&key) else {
                continue;
            };
            if geometry.indices.is_empty() {
                continue;
            }
            let mut kept: Vec<u32> = Vec::with_capacity(geometry.indices.len());
            for triangle in geometry.indices.chunks_exact(3) {
                let mut corners = [
                    &geometry.vertices[triangle[0] as usize],
                    &geometry.vertices[triangle[1] as usize],
                    &geometry.vertices[triangle[2] as usize],
                ]
                .map(vertex_key);
                corners.sort_unstable();
                if seen.insert(corners) {
                    kept.extend_from_slice(triangle);
                }
            }
            if kept.len() != geometry.indices.len() {
                geometry.indices = kept;
            }
        }
    }

    fn vertex(position: [f32; 3], normal: [f32; 3]) -> Vertex {
        Vertex {
            position,
            normal,
            color: [1.0; 3],
            mask: 0.0,
        }
    }

    fn triangle(normal: [f32; 3]) -> KeyGeometry {
        KeyGeometry {
            vertices: vec![
                vertex([0.0, 0.0, 0.0], normal),
                vertex([1.0, 0.0, 0.0], normal),
                vertex([0.0, 1.0, 0.0], normal),
            ],
            indices: vec![0, 1, 2],
        }
    }

    #[test]
    fn release_compaction_reclaims_empty_bricks_and_unused_vertices() {
        let reference = triangle([0.0, 0.0, 1.0]);
        let expected: Vec<_> = reference.vertices.iter().map(vertex_key).collect();
        let mut used = triangle([0.0, 0.0, 1.0]);
        used.vertices.insert(1, vertex([99.0; 3], [0.0; 3]));
        used.indices = vec![0, 2, 3];
        used.vertices.reserve(4096);
        used.indices.reserve(4096);
        let mut empty = triangle([0.0, 0.0, 1.0]);
        empty.vertices.reserve(4096);
        empty.vertices.clear();
        empty.indices.clear();
        let mut keys = HashMap::from([
            ([0, 0, 0], used),
            ([1, 0, 0], reference),
            ([2, 0, 0], empty),
        ]);
        super::compact_release_geometry(&mut keys);
        assert_eq!(
            keys.len(),
            1,
            "empty and duplicate-only entries retain allocations"
        );
        let actual = keys.get(&[0, 0, 0]).expect("first surviving owner");
        assert!(
            actual.vertices.capacity() < 64,
            "release old vertex backing storage"
        );
        assert!(
            actual.indices.capacity() < 64,
            "release old index backing storage"
        );
        assert_eq!(
            actual.vertices.iter().map(vertex_key).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            actual.indices,
            [0, 1, 2],
            "preserve winding while remapping references"
        );
    }

    #[test]
    fn the_same_triangle_in_two_bricks_is_contamination_even_if_its_normals_differ() {
        let keys = HashMap::from([
            ([0, 0, 0], triangle([0.0, 0.0, 1.0])),
            ([1, 0, 0], triangle([0.0, 0.1, 0.995])),
        ]);
        assert!(contains_coincident_triangles(&keys));
    }

    #[test]
    fn different_triangles_are_not_contamination() {
        let mut second = triangle([0.0, 0.0, 1.0]);
        second.vertices[0].position[2] = 0.1;
        let keys = HashMap::from([([0, 0, 0], triangle([0.0, 0.0, 1.0])), ([1, 0, 0], second)]);
        assert!(!contains_coincident_triangles(&keys));
    }
    fn snapshot(
        keys: &HashMap<super::BrickKey, KeyGeometry>,
    ) -> std::collections::BTreeMap<super::BrickKey, (Vec<[u32; 10]>, Vec<u32>)> {
        keys.iter()
            .map(|(key, g)| {
                (
                    *key,
                    (
                        g.vertices.iter().map(vertex_key).collect(),
                        g.indices.clone(),
                    ),
                )
            })
            .collect()
    }

    fn copy_geometry(
        keys: &HashMap<super::BrickKey, KeyGeometry>,
    ) -> HashMap<super::BrickKey, KeyGeometry> {
        keys.iter()
            .map(|(key, g)| {
                (
                    *key,
                    KeyGeometry {
                        vertices: g.vertices.clone(),
                        indices: g.indices.clone(),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn compact_duplicate_keys_preserve_the_original_exact_result() {
        let mut keys = HashMap::new();
        prune_exact_triangles(&mut keys);
        assert!(keys.is_empty());
        for component in 0..10 {
            for bits in [0, 0x8000_0000, 0x3f80_0001, 0x7fc0_0001, 0x7fc0_0002] {
                let original = triangle([0.0, 0.0, 1.0]);
                let mut changed = triangle([0.0, 0.0, 1.0]);
                let v = &mut changed.vertices[0];
                let channels = [
                    &mut v.position[..],
                    &mut v.normal[..],
                    &mut v.color[..],
                    std::slice::from_mut(&mut v.mask),
                ];
                *channels.into_iter().flatten().nth(component).unwrap() = f32::from_bits(bits);
                let mut duplicate = triangle([0.0, 0.0, 1.0]);
                duplicate.indices = vec![2, 1, 0, 0, 1, 2];
                let mut keys = HashMap::from([
                    ([-1, 0, 0], original),
                    ([0, 0, 0], changed),
                    ([1, 0, 0], duplicate),
                    (
                        [2, 0, 0],
                        KeyGeometry {
                            vertices: vec![],
                            indices: vec![],
                        },
                    ),
                ]);
                let mut expected = copy_geometry(&keys);
                prune_reference(&mut expected);
                prune_exact_triangles(&mut keys);
                assert_eq!(
                    snapshot(&keys),
                    snapshot(&expected),
                    "component {component}, bits {bits:x}"
                );
            }
        }
    }
    #[test]
    fn packed_triangle_identifiers_preserve_all_lanes_and_use_a_bounded_domain() {
        let values = [0, 1, u32::MAX as usize - 1, u32::MAX as usize];
        let mut seen = std::collections::HashSet::new();
        for selector in 0..64 {
            let ids = std::array::from_fn(|lane| values[(selector >> (lane * 2)) & 3]);
            let packed = super::packed_triangle_ids(ids);
            let restored: [usize; 3] =
                std::array::from_fn(|lane| ((packed >> (lane * 32)) & u32::MAX as u128) as usize);
            assert_eq!(restored, ids);
            assert!(seen.insert(packed));
        }
        assert!(super::triangle_ids_fit_packed(0));
        assert!(super::triangle_ids_fit_packed(u32::MAX as usize));
        if let Some(wide) = (u32::MAX as usize).checked_add(1) {
            assert!(!super::triangle_ids_fit_packed(wide));
            assert!(!super::triangle_ids_fit_packed(usize::MAX));
        }
    }

    #[test]
    fn packed_and_wide_pruning_preserve_order_under_collisions_and_trailing_indices() {
        let geometry = KeyGeometry {
            vertices: (0..6)
                .map(|i| vertex([i as f32, 0.0, 0.0], [0.0, 1.0, 0.0]))
                .collect(),
            indices: vec![0, 1, 2, 2, 1, 0, 3, 4, 5, 4, 3, 5, 5, 4],
        };
        let mut input = HashMap::from([([-1, 0, 0], geometry)]);
        let mut duplicate = copy_geometry(&input).remove(&[-1, 0, 0]).unwrap();
        duplicate.indices = vec![5, 4, 3, 2, 0, 1];
        input.insert([1, 0, 0], duplicate);
        let mut expected = copy_geometry(&input);
        prune_reference(&mut expected);
        assert_eq!(expected[&[-1, 0, 0]].indices, vec![0, 1, 2, 3, 4, 5]);
        assert!(expected[&[1, 0, 0]].indices.is_empty());
        let mut packed = copy_geometry(&input);
        let mut wide = copy_geometry(&input);
        let state = std::hash::BuildHasherDefault::<CollidingHasher>::default();
        super::prune_exact_triangles_with_hasher(&mut packed, state.clone());
        super::prune_triangles_with_keys(&mut wide, state, 12, std::convert::identity);
        assert_eq!(snapshot(&packed), snapshot(&expected));
        assert_eq!(snapshot(&wide), snapshot(&expected));
    }

    #[test]
    fn repeated_release_pruning_preserves_bits_after_vertex_storage_changes() {
        let mut first = triangle([0.0, 0.0, 1.0]);
        first.vertices[0].mask = f32::from_bits(0x7fc0_0001);
        first.vertices.insert(0, vertex([99.0; 3], [-0.0; 3]));
        first.indices = vec![1, 2, 3];
        let duplicate = KeyGeometry {
            vertices: first.vertices.clone(),
            indices: vec![3, 2, 1],
        };
        let mut keys = HashMap::from([([-1, 0, 0], first), ([1, 0, 0], duplicate)]);
        super::compact_release_geometry(&mut keys);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[&[-1, 0, 0]].vertices.len(), 3);
        // Force new backing storage and a new brick after the first pass.
        keys.insert([2, 0, 0], copy_geometry(&keys).remove(&[-1, 0, 0]).unwrap());
        let mut expected = copy_geometry(&keys);
        prune_reference(&mut expected);
        expected.retain(|_, geometry| !geometry.indices.is_empty());
        super::compact_release_geometry(&mut keys);
        assert_eq!(snapshot(&keys), snapshot(&expected));
        assert_eq!(keys[&[-1, 0, 0]].vertices[0].mask.to_bits(), 0x7fc0_0001);
    }

    #[test]
    fn pruning_without_complete_triangles_preserves_the_existing_no_op() {
        let mut geometry = triangle([0.0, 1.0, 0.0]);
        geometry.indices = vec![0, 1];
        let mut keys = HashMap::from([([0, 0, 0], geometry)]);
        let before = snapshot(&keys);
        prune_exact_triangles(&mut keys);
        assert_eq!(snapshot(&keys), before);
    }

    #[derive(Default)]
    struct CollidingHasher;

    impl std::hash::Hasher for CollidingHasher {
        fn finish(&self) -> u64 {
            0
        }

        fn write(&mut self, _: &[u8]) {}
    }

    fn assert_pruning_hash_independent<S: std::hash::BuildHasher + Clone>(state: S) {
        let mut keys = HashMap::new();
        for component in 0..10 {
            for (variant, bits) in [0, 0x8000_0000, 0x3f80_0001, 0x7fc0_0001, 0x7fc0_0002]
                .into_iter()
                .enumerate()
            {
                let mut geometry = triangle([0.0, 0.0, 1.0]);
                let vertex = &mut geometry.vertices[0];
                let channels = [
                    &mut vertex.position[..],
                    &mut vertex.normal[..],
                    &mut vertex.color[..],
                    std::slice::from_mut(&mut vertex.mask),
                ];
                *channels.into_iter().flatten().nth(component).unwrap() = f32::from_bits(bits);
                // A reversed copy must lose to the first owner even when every key collides.
                let duplicate = KeyGeometry {
                    vertices: geometry.vertices.clone(),
                    indices: vec![2, 1, 0],
                };
                keys.insert([component as i32, variant as i32, 0], geometry);
                keys.insert([component as i32, variant as i32, 1], duplicate);
            }
        }
        let mut expected = copy_geometry(&keys);
        prune_reference(&mut expected);
        super::prune_exact_triangles_with_hasher(&mut keys, state);
        assert_eq!(snapshot(&keys), snapshot(&expected));
    }

    #[test]
    fn pruning_preserves_exact_geometry_despite_hash_collisions_and_seeds() {
        assert_pruning_hash_independent(std::hash::BuildHasherDefault::<CollidingHasher>::default());
        for seed in 0..4 {
            assert_pruning_hash_independent(ahash::RandomState::with_seeds(seed, 17, 29, 41));
        }
    }

    fn pruning_workload(shared: bool) -> HashMap<super::BrickKey, KeyGeometry> {
        let mut keys = HashMap::new();
        for key in 0..8 {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            let mut vertex = |x: f32, y: f32| {
                vertices.push(Vertex {
                    position: [x, y, key as f32],
                    normal: [0., 0., 1.],
                    color: [1.; 3],
                    mask: 0.,
                });
            };
            if shared {
                for y in 0..65 {
                    for x in 0..65 {
                        vertex(x as f32, y as f32);
                    }
                }
                for y in 0..64 {
                    for x in 0..64 {
                        let a = y * 65 + x;
                        indices.extend([a, a + 1, a + 65, a + 1, a + 66, a + 65]);
                    }
                }
            } else {
                for t in 0..8192 {
                    let x = t as f32 * 2.;
                    vertex(x, 0.);
                    vertex(x + 1., 0.);
                    vertex(x, 1.);
                    indices.extend([t * 3, t * 3 + 1, t * 3 + 2]);
                }
            }
            keys.insert([key, 0, 0], KeyGeometry { vertices, indices });
        }
        keys
    }

    #[test]
    #[ignore = "informational release timing; no portable timing threshold"]
    fn profile_exact_triangle_pruning() {
        for shared in [true, false] {
            let keys = pruning_workload(shared);
            let mut expected = copy_geometry(&keys);
            let mut actual = copy_geometry(&keys);
            prune_reference(&mut expected);
            prune_exact_triangles(&mut actual);
            assert_eq!(snapshot(&actual), snapshot(&expected));
            let mut times = [Vec::new(), Vec::new()];
            for run in 0..7 {
                for which in [run % 2, 1 - run % 2] {
                    let mut copy = copy_geometry(&keys);
                    let start = std::time::Instant::now();
                    [prune_reference, prune_exact_triangles][which](&mut copy);
                    times[which].push(start.elapsed().as_secs_f64() * 1000.0);
                    std::hint::black_box(copy);
                }
            }
            for samples in &mut times {
                samples.sort_by(f64::total_cmp);
            }
            println!(
                "shared={shared} reference_ms={:.3} compact_ms={:.3}",
                times[0][3], times[1][3]
            );
        }
    }
    #[test]
    fn empty_bookkeeping_keys_do_not_keep_old_triangle_ownership_alive() {
        let mut keys = HashMap::from([
            ([0, 0, 0], triangle([0.0, 0.0, 1.0])),
            ([1, 0, 0], triangle([0.0, 0.0, 1.0])),
        ]);
        let replaced = std::collections::HashSet::from([[0, 0, 0]]);
        assert!(super::retains_unreplaced_triangles(&keys, Some(&replaced)));
        keys.get_mut(&[1, 0, 0]).unwrap().indices.clear();
        assert!(!super::retains_unreplaced_triangles(&keys, Some(&replaced)));
    }
}
