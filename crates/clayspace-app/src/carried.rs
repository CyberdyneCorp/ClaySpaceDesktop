//! Patching the carried buffer with an adaptive surface's dirty chunks.
//!
//! The carried layers — meshes, grids, hierarchies, adaptive surfaces — are
//! drawn from one buffer that is built whole whenever anything in it moves.
//! For an adaptive surface under the brush that is the model's size on every
//! dab. [`ClayDocument::carried_patch`] offers the chunks a stroke touched as
//! runs to write in place instead, and this is the half that turns them into
//! the renderer's vertices and hands them over. When no patch is offered, or
//! the renderer cannot take it, the caller builds the whole buffer as before.

use clayspace_engine::ClayDocument;
use clayspace_model::{AdaptiveUpload, LayerKey};
use clayspace_view::{Aabb, Gpu, Renderer, Vertex};

/// A patch in the renderer's vocabulary, ready to write.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PatchUpload {
    pub vertices: Vec<(u32, Vec<Vertex>)>,
    pub indices: Vec<(u32, Vec<u32>)>,
    pub grown: Vec<(LayerKey, Aabb)>,
    /// What the document counts this patch as, for the diagnostics.
    pub upload: AdaptiveUpload,
}

impl PatchUpload {
    /// What writing this sends to the device.
    pub fn bytes(&self) -> u64 {
        let vertices: usize = self.vertices.iter().map(|(_, run)| run.len()).sum();
        let indices: usize = self.indices.iter().map(|(_, run)| run.len()).sum();
        (vertices * Vertex::STRIDE + indices * 4) as u64
    }

    /// Whether there is nothing to write.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() && self.indices.is_empty()
    }

    /// Writes the runs into the renderer's carried buffer; `false`, having
    /// written nothing, where it cannot take them and the whole buffer has to
    /// be uploaded instead.
    pub fn apply(&self, gpu: &Gpu, renderer: &mut Renderer) -> bool {
        let mut vertices: Vec<(u32, &[Vertex])> = self
            .vertices
            .iter()
            .map(|(first, run)| (*first, run.as_slice()))
            .collect();
        let mut indices: Vec<(u32, &[u32])> = self
            .indices
            .iter()
            .map(|(first, run)| (*first, run.as_slice()))
            .collect();
        renderer.patch_mesh_layers(gpu, &mut vertices, &mut indices, &self.grown)
    }
}

/// The adaptive surfaces' dirty chunks as renderer runs, or `None` where the
/// carried buffer has to be built again — see
/// [`ClayDocument::carried_patch`].
///
/// `built` is the [`ClayDocument::carried_build`] of the buffer the renderer
/// holds. The frozen region is sampled for the patched vertices the way the
/// full build samples it for every vertex, from the same call.
pub fn patch_upload(document: &mut ClayDocument, built: u64) -> Option<PatchUpload> {
    let patch = document.carried_patch(built)?;
    let upload = patch.upload();
    let positions: Vec<[f32; 3]> = patch
        .vertices
        .iter()
        .flat_map(|run| run.positions.iter().copied())
        .collect();
    let frozen = document.mask_at(&positions);
    let mut at = 0;
    let vertices = patch
        .vertices
        .into_iter()
        .map(|run| {
            let vertices = run
                .positions
                .into_iter()
                .zip(run.normals)
                .map(|(position, normal)| {
                    let mask = frozen.as_ref().map_or(0.0, |weights| weights[at]);
                    at += 1;
                    Vertex {
                        position,
                        normal,
                        // White, as the full build draws an uncoloured
                        // surface; a coloured one is never patched.
                        color: [1.0; 3],
                        mask,
                    }
                })
                .collect();
            (run.first, vertices)
        })
        .collect();
    let indices = patch
        .indices
        .into_iter()
        .map(|run| (run.first, run.indices))
        .collect();
    Some(PatchUpload {
        vertices,
        indices,
        grown: patch.bounds,
        upload,
    })
}

/// What a full build of the carried buffer sent for the adaptive surfaces,
/// in bytes, from the document's count of it.
pub fn rebuilt_bytes(upload: AdaptiveUpload) -> u64 {
    (upload.vertices * Vertex::STRIDE + upload.indices * 4) as u64
}
