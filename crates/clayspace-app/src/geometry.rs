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
    pub mesh_time: std::time::Duration,
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
}

impl SurfaceGeometry {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            keys: HashMap::new(),
            mesh: GpuMesh::new(gpu),
            dirty: false,
            last_cost: None,
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
        // Re-mesh the dirty keys together with their face neighbours. A key
        // meshed alone regenerates the triangles along its boundary, while the
        // neighbour still holds its previous version of the same seam — which
        // shows as a thin crack tracing the edit. Including the neighbours
        // makes both sides of every seam come from the same call.
        let dirty = dilate(&dirty);

        let started = std::time::Instant::now();
        self.remesh(document, &dirty)?;
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
            keys: dirty.len(),
            mesh_time,
            upload_time,
            triangles: self.triangle_count(),
            vertices: self.vertex_count(),
        };
        self.last_cost = Some(cost);
        Ok(Some(cost))
    }

    /// Meshes a set of keys and replaces their stored geometry.
    fn remesh(&mut self, document: &ClayDocument, keys: &[BrickKey]) -> Result<(), ClayError> {
        let (mesh, ranges) = document.cache().mesh(
            Some(document.document()),
            BrickMeshParams {
                gradient_normals: true,
                colors: false,
                gradient_eps: None,
            },
            keys,
        )?;

        let (vertices, indices) = read_mesh(&mesh)?;

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
        let owner_of = |index: u32| -> Option<usize> {
            ranges.iter().position(|range| {
                index >= range.vertex_first && index < range.vertex_first + range.vertex_count
            })
        };

        let mut owned: Vec<Vec<[u32; 3]>> = vec![Vec::new(); ranges.len()];
        for range in &ranges {
            let first = range.index_first as usize;
            let last = (first + range.index_count as usize).min(indices.len());
            for triangle in indices[first..last].chunks_exact(3) {
                let triangle = [triangle[0], triangle[1], triangle[2]];
                // Ownership follows the first vertex, so a triangle listed
                // under two keys' index ranges is still stored once.
                if let Some(owner) = owner_of(triangle[0]) {
                    owned[owner].push(triangle);
                }
            }
        }

        for (slot, range) in ranges.iter().enumerate() {
            let entry = self.keys.entry(range.key).or_default();
            entry.vertices.clear();
            entry.indices.clear();

            let triangles = &owned[slot];
            if triangles.is_empty() {
                // A key that no longer crosses the surface keeps an empty slot
                // rather than vanishing, so a later edit finds it.
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
            entry.vertices.resize(local.len(), Vertex {
                position: [0.0; 3],
                normal: [0.0, 1.0, 0.0],
                color: [1.0; 3],
            });
            for (global, index) in local {
                if let Some(vertex) = vertices.get(global as usize) {
                    entry.vertices[index as usize] = *vertex;
                }
            }
        }

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

    /// Rebuilds every key from scratch.
    ///
    /// The compaction the specification calls for: per-key slots accumulate
    /// empty entries as the surface moves, and this is where they go. Off the
    /// interaction path — it costs a full re-mesh.
    pub fn rebuild(
        &mut self,
        gpu: &Gpu,
        document: &ClayDocument,
    ) -> Result<(), ClayError> {
        let keys = document.cache().surface_bricks()?;
        self.keys.clear();
        if keys.is_empty() {
            self.mesh.upload(gpu, &[], &[]);
            return Ok(());
        }
        self.remesh(document, &keys)?;
        self.upload(gpu);
        Ok(())
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

/// Grows a key set by its face neighbours.
///
/// Face rather than the full 26-neighbourhood: a seam is shared across a face,
/// and the corners cost six times as many keys for a boundary no triangle
/// spans.
fn dilate(keys: &[BrickKey]) -> Vec<BrickKey> {
    let mut grown: std::collections::HashSet<BrickKey> = keys.iter().copied().collect();
    for key in keys {
        for axis in 0..3 {
            for step in [-1, 1] {
                let mut neighbour = *key;
                neighbour[axis] += step;
                grown.insert(neighbour);
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
