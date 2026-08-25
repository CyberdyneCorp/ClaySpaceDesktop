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

/// The surface as the viewport holds it.
pub struct SurfaceGeometry {
    /// Per-key geometry, so a re-mesh replaces only what changed.
    keys: HashMap<BrickKey, KeyGeometry>,
    mesh: GpuMesh,
    /// Set when the keys have changed but the GPU buffer has not been rebuilt.
    dirty: bool,
    /// Which keys changed since the last upload, so only those are written.
    touched: std::collections::HashSet<BrickKey>,
    /// Where each key's geometry sits in the GPU buffers.
    layout: SlotMap,
    /// Set when the layout cannot be patched and must be laid out afresh.
    relayout: bool,
    /// The union of every key's bounds, exact as of the last full rebuild.
    bounds: Option<([f32; 3], [f32; 3])>,
    last_cost: Option<SyncCost>,
    /// Stage timings from the last `remesh`, for `SyncCost`.
    last_engine_mesh: std::time::Duration,
    last_read: std::time::Duration,
    last_split: std::time::Duration,
    /// Key sets meshed with face normals, oldest first.
    ///
    /// One entry per [`SurfaceGeometry::sync`] that shaded fast, holding
    /// exactly the keys that sync requested. The set is the unit because the
    /// engine attributes a straddling triangle to the lowest *requested* key
    /// (ClayCore #66): re-meshing the same request re-runs that sync with the
    /// gradient and replaces exactly what it wrote, where re-meshing some
    /// other slice of it would move triangles between keys and leave the ones
    /// it moved away from behind.
    ///
    /// Ordered because a later sync overwrote an earlier one, and the shading
    /// pass has to land in the same order to end up with the same surface.
    pending_shading: std::collections::VecDeque<Vec<BrickKey>>,
    /// How many queued sets name each key.
    ///
    /// So that "is this set meshed again later?" is a lookup per key rather
    /// than a scan of the queue. Scanning is fine for the two dozen sets a
    /// gesture leaves and quadratic in the thousands a drag that never pauses
    /// can reach — and a drag that never pauses is exactly the one that never
    /// gets to drain.
    pending_keys: HashMap<BrickKey, usize>,
    /// Where the warped keys' vertices were before a cage preview moved them.
    ///
    /// Positions only, and only while a cage is up. A preview is shown by
    /// moving the vertices the viewport already holds, and putting them back
    /// needs the originals — recomputing them by warping backwards would
    /// accumulate the error of two approximations instead of none.
    cage_rest: HashMap<BrickKey, Vec<[f32; 3]>>,
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

/// How a mesh is shaded, which is the one knob worth changing mid-gesture.
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
/// Sculpting therefore shades fast and owes the gradient, which
/// [`SurfaceGeometry::refine_within`] pays off on frames that are not
/// sculpting. Not at pointer-up in one pass — that was 15.7 ms in one frame,
/// the hitch `gesture_end.rs` exists to hold — and not beside the sample
/// either, which would cost the frame both shadings at once.
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
            keys: HashMap::new(),
            mesh: GpuMesh::new(gpu),
            dirty: false,
            touched: std::collections::HashSet::new(),
            layout: SlotMap::default(),
            relayout: true,
            bounds: None,
            last_cost: None,
            last_engine_mesh: std::time::Duration::ZERO,
            last_read: std::time::Duration::ZERO,
            last_split: std::time::Duration::ZERO,
            pending_shading: std::collections::VecDeque::new(),
            pending_keys: HashMap::new(),
            detail: Detail::Full,
            requested: Detail::Full,
        }
    }

    pub fn mesh(&self) -> &GpuMesh {
        &self.mesh
    }

    pub fn last_cost(&self) -> Option<SyncCost> {
        self.last_cost
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
        // Face normals while the pointer is down, and the gradient owed.
        //
        // The gradient is no longer eleven times the rest of a segment — it is
        // 40% of it, and a tail that reaches 19 ms where face normals never
        // leave 6 — so it is deferred rather than skipped, and paid off a
        // segment at a time by `refine_within` on frames that are not
        // sculpting. See [`Shading`] for the measurements.
        self.remesh(document, &meshed, Some(&replace), Shading::Fast, 0)?;
        for key in &meshed {
            *self.pending_keys.entry(*key).or_insert(0) += 1;
        }
        self.pending_shading.push_back(meshed.clone());
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
        let states = document.cache().states(dirty)?;
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
        let engine_started = std::time::Instant::now();
        // No document at level 1, which skips compiling a tape the coarse
        // mesh cannot use anyway: the level refuses gradient normals and
        // colours, and face normals come from the triangles.
        let doc = (lod == 0).then(|| document.document());
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
            let (mesh, ranges) = document.cache().mesh_lod(
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
        let (mut vertices, indices) = match &mesh {
            Some(mesh) => read_mesh(mesh)?,
            None => (Vec::new(), Vec::new()),
        };
        // The frozen region, on the vertices this re-mesh just produced.
        //
        // Only these, which is the dirty subset: a dab that re-meshes twenty
        // bricks samples twenty bricks' worth rather than the whole surface.
        // A mask that *changes* is the other direction and is
        // `refresh_mask`'s job.
        sample_mask(document, &mut vertices);
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
        for key in &to_replace {
            self.touched.insert(*key);
            let slot = slot_of.get(key).copied();
            let entry = self.keys.entry(*key).or_default();
            entry.vertices.clear();
            entry.indices.clear();

            let Some(triangles) = slot.map(|slot| &owned[slot]) else {
                // Asked for and not returned: the surface has left this brick.
                // Cleared above, and kept as an empty slot so a later edit
                // finds it.
                continue;
            };
            if triangles.is_empty() {
                continue;
            }

            // A local vertex table holding exactly what these triangles use.
            let mut local = std::collections::HashMap::new();
            for triangle in triangles {
                for global in triangle {
                    let next = local.len() as u32;
                    let index = *local.entry(*global).or_insert(next);
                    entry.indices.push(index);
                }
            }
            entry.vertices.resize(
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
                    entry.vertices[index as usize] = *vertex;
                }
            }
        }
        self.last_split = split_started.elapsed();
        // The vertices a preview was holding the originals of have been
        // replaced, so those originals describe geometry that is gone. Dropped
        // rather than patched: the next preview stores them again from what is
        // there now.
        self.cage_rest.clear();
        self.dirty = true;
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

        self.bounds = union(self.bounds, bounds_of(&geometry.vertices));
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
        let vertices_needed = self.vertex_count() + self.keys.len() * 64;
        let indices_needed = self.triangle_count() * 3 + self.keys.len() * 64;
        self.mesh.reserve(
            gpu,
            (vertices_needed * 2).max(1024),
            (indices_needed * 2).max(1024),
        );
        self.layout = SlotMap::new(
            (vertices_needed * 2).max(1024) as u32,
            (indices_needed * 2).max(1024) as u32,
        );
        self.bounds = None;
        self.touched.clear();
        self.prune_duplicates();

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
    /// and a long session accumulates them — measured at 13,684 of 597,597
    /// triangles, against 351 in a rebuild of the same document.
    ///
    /// They cost upload and draw rather than correctness: the copies are
    /// coincident, so nothing shimmers and no hole appears. Which is why this
    /// runs *here* rather than on the interaction path. Preventing a duplicate
    /// at the moment it is filed means knowing which other bricks hold a
    /// corner of the triangle and reaching into them, on every triangle of
    /// every dab, to save two per cent of a buffer — the cost lands on the
    /// gesture and the saving does not. A relayout already walks and rewrites
    /// everything, so one pass over it is proportionate, and the duplicates go
    /// no further than the next one.
    fn prune_duplicates(&mut self) {
        // In key order, so the copy that survives is the one under the
        // lexicographically lowest key. Two things follow, and both matter.
        // The result does not depend on how a `HashMap` happens to iterate, so
        // the drawn buffer is the same from one run to the next. And it is the
        // same copy the engine would have chosen for a whole-surface request —
        // it attributes to the lowest requested key owning a corner — so a
        // store pruned this way still agrees with a rebuild key for key, which
        // is what `visual_incremental` and `lod_switching` check.
        let mut keys: Vec<BrickKey> = self.keys.keys().copied().collect();
        keys.sort_unstable();
        let mut seen: std::collections::HashSet<[[i32; 3]; 3]> = std::collections::HashSet::new();
        for key in keys {
            let Some(geometry) = self.keys.get_mut(&key) else {
                continue;
            };
            if geometry.indices.is_empty() {
                continue;
            }
            let mut kept: Vec<u32> = Vec::with_capacity(geometry.indices.len());
            for triangle in geometry.indices.chunks_exact(3) {
                // Quantised and sorted, so the same triangle reached from two
                // keys is the same value however each key numbered its own
                // vertices.
                let mut corners = [
                    geometry.vertices[triangle[0] as usize].position,
                    geometry.vertices[triangle[1] as usize].position,
                    geometry.vertices[triangle[2] as usize].position,
                ]
                .map(|p| p.map(|c| (c * 4096.0).round() as i32));
                corners.sort_unstable();
                if seen.insert(corners) {
                    kept.extend_from_slice(triangle);
                }
            }
            if kept.len() != geometry.indices.len() {
                geometry.indices = kept;
                // The vertices a dropped triangle used may still be referenced
                // by the ones kept, so the table is left alone: it is bounded
                // by what this key's triangles ever used, and the next re-mesh
                // of the key rebuilds it exactly.
            }
        }
    }

    /// Re-shades what sculpting shaded fast, for as long as `budget` allows.
    ///
    /// Returns whether anything is still owed, which is the caller's cue to
    /// ask for another frame.
    ///
    /// The positions are already right — `sync` is exact, and normals do not
    /// move a vertex — so nothing a silhouette test could see changes here.
    /// What it buys back is the gradient the drag deferred.
    ///
    /// The first set of a call runs whatever the budget says, so a caller with
    /// nothing to spare still makes progress rather than spinning; every set
    /// after it has to fit in what is left. One set is a dab's worth of keys,
    /// which is the granularity that keeps that guarantee cheap: 27 keys and
    /// about 2.2 ms on the reference form, against the 15.7 ms the single
    /// pointer-up pass cost over all 111 keys a stroke touches.
    pub fn refine_within(
        &mut self,
        gpu: &Gpu,
        document: &mut ClayDocument,
        budget: std::time::Duration,
    ) -> Result<bool, ClayError> {
        // Level 1 is face-shaded by construction and has no gradient to buy
        // back, and the queued keys are from the other level's space anyway —
        // so they are dropped rather than carried across the switch.
        if self.detail == Detail::Reduced {
            self.forget_pending();
            return Ok(false);
        }
        let started = std::time::Instant::now();
        let mut last: Option<std::time::Duration> = None;
        while !self.pending_shading.is_empty() {
            // What the previous set cost is the estimate for the next one, so
            // the budget is a ceiling rather than something to overshoot by
            // whatever the set after it happens to cost. Stopping only once
            // the budget was already spent ran 17.7 ms against 12.7 ms.
            if last.is_some_and(|cost| started.elapsed() + cost > budget) {
                break;
            }
            let keys = self.pending_shading.pop_front().expect("not empty");
            // Dropped from the tally first, so what is left against a key is
            // what the sets *behind* this one name.
            self.untally(&keys);
            // Every key of it is meshed again further down the queue, so
            // shading it now would only be overwritten by a set that has to
            // run regardless. Common in a slow drag, where consecutive
            // samples dirty the same bricks.
            if keys.iter().all(|key| self.pending_keys.contains_key(key)) {
                continue;
            }
            let set_started = std::time::Instant::now();
            let replace: std::collections::HashSet<BrickKey> = keys.iter().copied().collect();
            self.remesh(document, &keys, Some(&replace), Shading::Full, 0)?;
            self.upload(gpu);
            last = Some(set_started.elapsed());
        }
        Ok(!self.pending_shading.is_empty())
    }

    /// Takes one set back out of [`SurfaceGeometry::pending_keys`].
    ///
    /// A key whose count reaches zero is removed rather than left at zero, so
    /// "is this key still queued" stays a `contains_key`.
    fn untally(&mut self, keys: &[BrickKey]) {
        for key in keys {
            let Some(count) = self.pending_keys.get_mut(key) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.pending_keys.remove(key);
            }
        }
    }

    /// Drops the whole shading debt, for when nothing it names is addressable.
    fn forget_pending(&mut self) {
        self.pending_shading.clear();
        self.pending_keys.clear();
    }

    /// How many segments are waiting for their gradient.
    pub fn awaiting_refinement(&self) -> usize {
        self.pending_shading.len()
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

    /// Re-meshes the whole surface and compacts the per-key slots.
    ///
    /// No longer needed to close seams. Until ClayCore 0.28.0 a subset mesh
    /// omitted straddling triangles (#66), so every gesture ended with a full
    /// re-mesh to pay off what the fast path had approximated; `sync` is exact
    /// now, held to that by `settle_needed.rs`.
    ///
    /// What is left is compaction: slots for bricks the surface has moved out
    /// of are kept empty rather than removed, so a long session accumulates
    /// them. That is bookkeeping rather than something a viewer can see, so it
    /// belongs somewhere deliberate — a document being replaced, an armature
    /// rewritten — and not on the end of every stroke.
    pub fn settle(&mut self, gpu: &Gpu, document: &mut ClayDocument) -> Result<(), ClayError> {
        self.rebuild(gpu, document)
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
        if detail == Detail::Reduced {
            let coarse = document.drawable_coarse_keys()?;
            if !coarse.is_empty() {
                // Level 1 refuses gradient normals rather than downgrading
                // them, so the coarse surface is face-shaded by construction.
                return Ok((coarse, 1, Shading::Fast));
            }
        }
        Ok((document.cache().surface_bricks()?, 0, Shading::Full))
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
        // Everything it could name is about to be meshed at the level's own
        // shading, so the debt is settled rather than carried.
        self.forget_pending();
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

    /// How much of the stored geometry is empty slots.
    ///
    /// The signal for when compaction is worth its cost.
    pub fn fragmentation(&self) -> f64 {
        if self.keys.is_empty() {
            return 0.0;
        }
        let empty = self.keys.values().filter(|k| k.indices.is_empty()).count();
        empty as f64 / self.keys.len() as f64
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

/// Reads an engine mesh into the renderer's vertex layout in one pass.
fn read_mesh(mesh: &Mesh) -> Result<(Vec<Vertex>, Vec<u32>), ClayError> {
    let count = mesh.vertex_count();
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

/// Kept so the document type is visible to readers of the imports.
const _: fn(&Document) -> bool = |_| true;

/// The corners of a box containing every vertex.
fn bounds_of(vertices: &[Vertex]) -> Option<([f32; 3], [f32; 3])> {
    let first = vertices.first()?.position;
    Some(vertices.iter().fold((first, first), |(min, max), v| {
        let at = v.position;
        (
            [min[0].min(at[0]), min[1].min(at[1]), min[2].min(at[2])],
            [max[0].max(at[0]), max[1].max(at[1]), max[2].max(at[2])],
        )
    }))
}

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
