//! A finished retopology, drawn while it waits to be accepted or discarded.
//!
//! The result is not in the document yet, so it is not in the carried buffer
//! either: it arrives here as the triangles acceptance would place, standing
//! where acceptance would draw them, and is drawn from buffers of its own.
//!
//! **In place of its source**, where the source is a carried subtool: the
//! source's span and its polyframe lines are left out while the preview is
//! held, so the sculptor judges the quads rather than the quads fighting the
//! sculpt for the same pixels. A field source cannot be cut out of the one
//! surface every field layer shares, so there the whole field surface is left
//! out while the preview is held: drawn under it, the field hides it, since
//! the quads chord the isosurface and lie just inside it.
//!
//! **With the layer's own material**, through the surface's own pipeline, so
//! it is lit, shaded and drawn through exactly as the accepted layer will be.
//! Where it carries a layout and a UV display is chosen, the layout is drawn
//! instead, the way [`super::uv_preview`] draws an accepted layer's.

use clayspace_model::{LayerKey, UvDisplay, UvPreview};

use super::polyframe;
use super::uv_preview::{uv_geometry, UvPreviewMesh};
use super::{GpuMesh, MeshSpan, Vertex};

/// The preview on the device.
pub(super) struct HeldPreviewMesh {
    /// The subtool it stands in for.
    pub(super) source: LayerKey,
    /// Its triangles, in the carried buffer's vertex type.
    pub(super) surface: GpuMesh,
    /// Its authored edges over the same vertices, for the polyframe.
    pub(super) edges: GpuMesh,
    /// Its layout as a checker, while a display is chosen and it has one.
    pub(super) layout: Option<UvPreviewMesh>,
}

impl HeldPreviewMesh {
    pub(super) fn upload(
        gpu: &crate::gpu::Gpu,
        preview: &UvPreview,
        edges: &[u32],
        display: UvDisplay,
    ) -> Self {
        let (vertices, lines) = held_geometry(preview, edges);
        let mut surface = GpuMesh::new(gpu);
        surface.upload(gpu, &vertices, &preview.indices);
        let mut drawn_edges = GpuMesh::new(gpu);
        drawn_edges.upload(gpu, &vertices, &lines);
        let layout = (display.is_on() && preview.uvs.len() == preview.positions.len())
            .then(|| UvPreviewMesh::upload(gpu, preview.layer, &uv_geometry(preview, display)));
        Self {
            source: preview.layer,
            surface,
            edges: drawn_edges,
            layout,
        }
    }
}

/// The preview's vertices in the carried buffer's type, and its edges as a
/// line list over them.
///
/// The authored edges where the result has them — a quad drawn as its four
/// edges rather than its fan triangulation — and the triangulation's unique
/// edges where it has none: through [`polyframe::lines`], the derivation the
/// accepted layer's polyframe goes through, so the two cannot disagree.
pub fn held_geometry(preview: &UvPreview, edges: &[u32]) -> (Vec<Vertex>, Vec<u32>) {
    let vertices = preview
        .positions
        .iter()
        .enumerate()
        .map(|(at, &position)| Vertex {
            position,
            normal: preview.normals.get(at).copied().unwrap_or([0.0, 0.0, 1.0]),
            color: [1.0; 3],
            mask: 0.0,
        })
        .collect();
    let authored = (!edges.is_empty()).then(|| edges.to_vec());
    let span = MeshSpan::with_edges(preview.layer, 0..preview.indices.len() as u32, authored);
    let count = preview.positions.len() as u32;
    let lines = polyframe::lines(&preview.indices, &[span])
        .indices
        .chunks_exact(2)
        .filter(|line| line.iter().all(|&v| v < count))
        .flatten()
        .copied()
        .collect();
    (vertices, lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One quad, fanned, standing somewhere with its own normals.
    fn quad() -> UvPreview {
        UvPreview {
            layer: LayerKey(4),
            positions: vec![
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            uvs: Vec::new(),
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }

    #[test]
    fn the_preview_carries_its_triangles_unchanged() {
        let preview = quad();
        let (vertices, _) = held_geometry(&preview, &[]);
        let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
        assert_eq!(positions, preview.positions);
        assert!(vertices.iter().all(|v| v.normal == [0.0, 0.0, 1.0]));
        assert!(vertices.iter().all(|v| v.mask == 0.0));
    }

    #[test]
    fn authored_edges_are_drawn_rather_than_the_diagonal() {
        let quad_edges = [0, 1, 1, 2, 2, 3, 3, 0];
        let (_, lines) = held_geometry(&quad(), &quad_edges);
        // Each edge as the polyframe orders it, lower vertex first.
        assert_eq!(
            lines,
            vec![0, 1, 1, 2, 2, 3, 0, 3],
            "four edges and no diagonal"
        );
        // Without them, the triangulation's five unique edges.
        let (_, derived) = held_geometry(&quad(), &[]);
        assert_eq!(derived.len(), 10);
    }

    #[test]
    fn an_edge_past_the_vertices_is_dropped() {
        let (_, lines) = held_geometry(&quad(), &[0, 1, 2, 9]);
        assert_eq!(lines, vec![0, 1]);
    }
}
