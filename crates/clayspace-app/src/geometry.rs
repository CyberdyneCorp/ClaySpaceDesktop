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

use std::collections::HashMap;

use clayspace_engine::claycore::{
    BrickKey, BrickMeshParams, ClayError, Document, Mesh, VertexLayout,
};
use clayspace_engine::ClayDocument;
use clayspace_view::{Gpu, GpuMesh, Vertex};

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
    /// Concatenating the keys and handing the buffer to the GPU.
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
    last_cost: Option<SyncCost>,
    /// Stage timings from the last `remesh`, for `SyncCost`.
    last_engine_mesh: std::time::Duration,
    last_read: std::time::Duration,
    last_split: std::time::Duration,
}

impl SurfaceGeometry {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            keys: HashMap::new(),
            mesh: GpuMesh::new(gpu),
            dirty: false,
            last_cost: None,
            last_engine_mesh: std::time::Duration::ZERO,
            last_read: std::time::Duration::ZERO,
            last_split: std::time::Duration::ZERO,
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
        let dirty = document.take_dirty_keys();
        if dirty.is_empty() {
            return Ok(None);
        }
        // The edited region and one ring around it.
        //
        // Meshing a subset leaves faint seams along the edge of the edit, and
        // `settle` clears them when the stroke ends. The cause is ours, not
        // the engine's: `clay_brick_cache_mesh` given a key list was measured
        // against the same call with none, and inside those bricks it returns
        // identical vertex positions *and* identical triangles. What a subset
        // cannot emit is a triangle reaching outside itself — and this drops
        // exactly those, because it clears a key's stored geometry and rebuilds
        // it from a mesh that could not contain them.
        //
        // The fix is to keep, rather than clear, the stored triangles whose
        // vertices lie outside the meshed set; that needs per-vertex ownership
        // recorded alongside the geometry. Until then the ring is a cheap
        // reduction in how often it shows and `settle` is the guarantee.
        //
        // An earlier version of this comment blamed the engine. It was wrong,
        // and it nearly became a filed bug.
        // The edited region and one ring around it.
        //
        // Everything requested is also replaced, which is what makes the
        // incremental surface exact.
        //
        // Until ClayCore 0.28.0 a subset mesh emitted only triangles wholly
        // inside the keys it was given, so a stroke left seams that no amount
        // of dilation closed and `settle` had to re-mesh the world when the
        // pointer came up (#66). A subset now returns every triangle with at
        // least one corner in a requested brick — but attributed to the
        // lowest *requested* key owning a corner, which is request-relative.
        //
        // That is why the replaced set has to be the whole request rather than
        // the dirty core: a straddling triangle can be attributed to a key in
        // the ring, and replacing only the core would drop it. Measured over
        // six dabs, the difference is 153 triangles missing against a rebuild
        // versus none at all; `settle_needed.rs` is that measurement.
        let meshed = dilate(&dirty, 1);
        let replace: std::collections::HashSet<BrickKey> = meshed.iter().copied().collect();

        let started = std::time::Instant::now();
        self.remesh(document, &meshed, Some(&replace))?;
        let mesh_time = started.elapsed();

        let started = std::time::Instant::now();
        self.upload(gpu);
        let upload_time = started.elapsed();

        // The interface reports what is on screen, so the counts come from
        // what was actually built rather than from an estimate.
        document.record_geometry(
            self.triangle_count(),
            self.vertex_count(),
            clayspace_model::Detail::Full,
        );

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
    /// Meshes `keys` and replaces the stored geometry of `replace`.
    ///
    /// `replace` of `None` means every meshed key, which is what a full
    /// rebuild wants. A subset re-mesh passes a smaller set than it meshed —
    /// see [`SurfaceGeometry::sync`] for why.
    fn remesh(
        &mut self,
        document: &ClayDocument,
        keys: &[BrickKey],
        replace: Option<&std::collections::HashSet<BrickKey>>,
    ) -> Result<(), ClayError> {
        let engine_started = std::time::Instant::now();
        let (mesh, ranges) = document.cache().mesh(
            Some(document.document()),
            BrickMeshParams {
                gradient_normals: true,
                colors: false,
                gradient_eps: None,
            },
            keys,
        )?;

        self.last_engine_mesh = engine_started.elapsed();

        let read_started = std::time::Instant::now();
        let (vertices, indices) = read_mesh(&mesh)?;
        self.last_read = read_started.elapsed();
        let split_started = std::time::Instant::now();

        // Each triangle is owned by exactly one key — the one whose vertex
        // range contains its first index — so the union of keys carries every
        // triangle once. A key then stores the vertices its own triangles
        // reference, which for a boundary triangle includes vertices from a
        // neighbour.
        //
        // The first version stored only a key's own vertex range and dropped
        // any triangle reaching outside it. That opened a crack along every
        // brick boundary: the engine welds vertices across seams, so a great
        // many triangles reach outside. The capture showed a grid of holes
        // across the whole surface, which no count or timing would have named.
        // Binary search, not a scan. Finding the owner is done once per
        // triangle and the scan was linear in the number of keys, so the cost
        // of a re-mesh grew with the *square* of the region: 24 segments of a
        // stroke took 600 ms each with a few hundred keys in play.
        let mut by_first: Vec<(u32, u32, usize)> = ranges
            .iter()
            .enumerate()
            .map(|(slot, range)| (range.vertex_first, range.vertex_count, slot))
            .collect();
        by_first.sort_unstable();
        let owner_of = |index: u32| -> Option<usize> {
            // The last range starting at or before `index`, then a bounds
            // check: ranges do not overlap, so at most one can contain it.
            let at = by_first.partition_point(|(first, _, _)| *first <= index);
            let (first, count, slot) = *by_first.get(at.checked_sub(1)?)?;
            (index < first + count).then_some(slot)
        };

        // Only triangles belonging to a key that is actually being replaced
        // are collected. The scan still has to visit every triangle — a
        // triangle listed under one key's index range can be owned by another
        // — but a key that is being kept needs nothing built for it, and
        // building it anyway was most of the cost of meshing the whole
        // surface.
        let wanted =
            |slot: usize| replace.is_none_or(|replace| replace.contains(&ranges[slot].key));
        let mut owned: Vec<Vec<[u32; 3]>> = vec![Vec::new(); ranges.len()];
        for range in &ranges {
            let first = range.index_first as usize;
            let last = (first + range.index_count as usize).min(indices.len());
            for triangle in indices[first..last].chunks_exact(3) {
                let triangle = [triangle[0], triangle[1], triangle[2]];
                // Ownership follows the first vertex, so a triangle listed
                // under two keys' index ranges is still stored once.
                if let Some(owner) = owner_of(triangle[0]) {
                    if wanted(owner) {
                        owned[owner].push(triangle);
                    }
                }
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
        for key in &to_replace {
            let slot = ranges.iter().position(|range| range.key == *key);
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
                },
            );
            for (global, index) in local {
                if let Some(vertex) = vertices.get(global as usize) {
                    entry.vertices[index as usize] = *vertex;
                }
            }
        }
        self.last_split = split_started.elapsed();
        self.dirty = true;
        Ok(())
    }

    /// Rebuilds the GPU buffer from the stored keys.
    ///
    /// Concatenation rather than sub-range patching: the engine welds vertices
    /// across brick seams, so a key's range cannot be relocated independently.
    /// Meshing is the cost the engine bounds and this does not repeat it; the
    /// upload is a memcpy whose size the latency test keeps honest.
    fn upload(&mut self, gpu: &Gpu) {
        if !self.dirty {
            return;
        }
        let mut vertices = Vec::with_capacity(self.vertex_count());
        let mut indices = Vec::with_capacity(self.triangle_count() * 3);

        for geometry in self.keys.values() {
            let base = vertices.len() as u32;
            vertices.extend_from_slice(&geometry.vertices);
            indices.extend(geometry.indices.iter().map(|i| i + base));
        }

        self.mesh.upload(gpu, &vertices, &indices);
        self.dirty = false;
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

    /// Rebuilds every key from scratch.
    ///
    /// The compaction the specification calls for: per-key slots accumulate
    /// empty entries as the surface moves, and this is where they go. Off the
    /// interaction path — it costs a full re-mesh.
    pub fn rebuild(&mut self, gpu: &Gpu, document: &mut ClayDocument) -> Result<(), ClayError> {
        let keys = document.cache().surface_bricks()?;
        self.keys.clear();
        if keys.is_empty() {
            self.mesh.upload(gpu, &[], &[]);
            document.take_dirty_keys();
            return Ok(());
        }
        self.remesh(document, &keys, None)?;
        self.upload(gpu);
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

/// Grows a key set by every neighbour that shares any boundary with it.
///
/// All twenty-six, not the six faces. Face-only was tried, reasoning that a
/// seam is shared across a face and that corners cost six times the keys for
/// a boundary no triangle spans. The second half of that is wrong: the engine
/// welds vertices across seams, so a triangle at a brick corner can be owned
/// by a diagonal neighbour, and a diagonal neighbour left out of the re-mesh
/// keeps its stale copy of a vertex that has since moved. On screen that is a
/// dark sliver at the corner of every brick the stroke crossed — visible in
/// `visual_incremental`, and invisible to any count or timing.
fn dilate(keys: &[BrickKey], rings: i32) -> Vec<BrickKey> {
    let mut grown: std::collections::HashSet<BrickKey> = keys.iter().copied().collect();
    for key in keys {
        for dx in -rings..=rings {
            for dy in -rings..=rings {
                for dz in -rings..=rings {
                    grown.insert([key[0] + dx, key[1] + dy, key[2] + dz]);
                }
            }
        }
    }
    let mut grown: Vec<BrickKey> = grown.into_iter().collect();
    grown.sort();
    grown
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
        })
        .collect();

    let mut indices = vec![0u32; mesh.index_count()];
    mesh.copy_indices(&mut indices)?;
    Ok((vertices, indices))
}

/// Kept so the document type is visible to readers of the imports.
const _: fn(&Document) -> bool = |_| true;
