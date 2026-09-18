//! The mapped upload API must draw the same bytes as ordinary queue writes.
mod support;

use clayspace_view::{Camera, GpuMesh, Vertex};
use support::Harness;

#[test]
fn mapped_uploads_preserve_rendering_and_empty_uploads() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let vertices = [[-0.7, -0.6, 0.0], [0.7, -0.6, 0.0], [0.0, 0.7, 0.0]].map(|position| Vertex {
        position,
        normal: [0.0, 0.0, 1.0],
        color: [0.3, 0.7, 0.4],
        mask: 0.25,
    });
    let indices = [0u32, 1, 2];
    let bytes = (vertices.len() * Vertex::STRIDE + indices.len() * 4) as u64;
    let mut reference = GpuMesh::new(&harness.gpu);
    harness.gpu.take_uploaded_bytes();
    reference.upload(&harness.gpu, &vertices, &indices);
    assert_eq!(harness.gpu.take_uploaded_bytes(), bytes);

    let mut mapped = GpuMesh::new(&harness.gpu);
    assert!(mapped.reserve(&harness.gpu, vertices.len() + 64, indices.len() + 192));
    mapped.patch_vertices_with(&harness.gpu, vertices.len(), |into| {
        into.copy_from_slice(bytemuck::cast_slice(&vertices));
    });
    mapped.patch_indices_with(&harness.gpu, indices.len(), |into| {
        into.copy_from_slice(bytemuck::cast_slice(&indices));
    });
    assert_eq!(harness.gpu.take_uploaded_bytes(), bytes);
    mapped.set_index_count(indices.len() as u32);
    mapped.set_bounds(Vertex::bounds(&vertices));

    mapped.patch_vertices_with(&harness.gpu, 0, |_| panic!("empty vertex callback"));
    mapped.patch_indices_with(&harness.gpu, 0, |_| panic!("empty index callback"));
    assert_eq!(harness.gpu.take_uploaded_bytes(), 0);
    let mut camera = Camera::default();
    camera.frame_default();
    let before = harness.capture(&reference, &camera, true, "mapped-upload-reference");
    let after = harness.capture(&mapped, &camera, true, "mapped-upload-direct");
    let background = harness.background();
    assert!(
        before
            .pixels
            .chunks_exact(4)
            .filter(|pixel| *pixel != background)
            .count()
            > 100,
        "the reference triangle must actually be visible"
    );
    assert_eq!(after.pixels, before.pixels);
}
