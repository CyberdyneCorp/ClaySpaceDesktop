//! What a settle is allowed to allocate, and how many writes it may take.
//!
//! An audited session reached a 26 GB process footprint while the application
//! reported 13 MB in use, most of it dirty graphics memory in hundreds of
//! thousands of small regions. Two things in the upload path were making it:
//! a layout allocated a fresh pair of buffers whether or not the old pair
//! already fitted, and every key was patched with a write of its own — each
//! one taking a staging buffer wgpu holds until the submission consuming it is
//! seen to have completed.
//!
//! These are the gates on both. The third part of the fix, polling the device
//! once a frame so those submissions are seen, is exercised by every test that
//! renders; there is nothing to assert about it that a counter can see.

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_view::{Camera, GpuMesh, Vertex};
use support::Harness;

/// Three triangles side by side, one per notional key.
fn triangles() -> ([Vertex; 9], [u32; 9]) {
    let mut vertices = [Vertex {
        position: [0.0; 3],
        normal: [0.0, 0.0, 1.0],
        color: [0.3, 0.7, 0.4],
        mask: 0.25,
    }; 9];
    for (key, corner) in (0..3).flat_map(|key| (0..3).map(move |corner| (key, corner))) {
        let left = -0.9 + key as f32 * 0.6;
        vertices[key * 3 + corner].position = match corner {
            0 => [left, -0.5, 0.0],
            1 => [left + 0.5, -0.5, 0.0],
            _ => [left + 0.25, 0.5, 0.0],
        };
    }
    (vertices, [0, 1, 2, 3, 4, 5, 6, 7, 8])
}

#[test]
fn reserve_reuses_a_buffer_that_already_fits() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let mut mesh = GpuMesh::new(&harness.gpu);

    harness.gpu.take_allocations();
    assert!(mesh.reserve(&harness.gpu, 4096, 4096));
    assert_eq!(
        harness.gpu.take_allocations(),
        2,
        "the first reservation has nothing to reuse and must allocate both buffers"
    );

    // The shape of a settle that did not change the surface's size: the same
    // figures asked for again. This used to drop both buffers and take two
    // more, which is the allocation a long session's footprint was following.
    assert!(mesh.reserve(&harness.gpu, 4096, 4096));
    assert_eq!(
        harness.gpu.take_allocations(),
        0,
        "a reservation the buffers already fit must reuse them"
    );

    // And one for less, which a surface that shrank asks for. Keeping the
    // larger buffer is the point: a buffer that shrank on one settle and grew
    // again on the next would allocate twice for one stroke.
    assert!(mesh.reserve(&harness.gpu, 1024, 1024));
    assert_eq!(harness.gpu.take_allocations(), 0);

    // Growing is still growing.
    assert!(mesh.reserve(&harness.gpu, 8192, 4096));
    assert_eq!(
        harness.gpu.take_allocations(),
        1,
        "only the buffer that ran out of room should have been replaced"
    );
}

#[test]
fn a_rebuilt_layout_of_the_same_size_allocates_nothing() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("first mesh");
    assert!(
        harness.gpu.take_allocations() > 0,
        "the first layout has to allocate something"
    );

    // The same surface laid out again asks for the same figures, so nothing
    // needs replacing. This is the acceptance criterion in one line: a settle
    // that does not grow the mesh allocates no new GPU buffers.
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("second mesh");
    assert_eq!(
        harness.gpu.take_allocations(),
        0,
        "a layout of an unchanged surface reallocated its buffers"
    );
}

#[test]
fn a_settle_merges_key_ranges_into_one_write() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let (vertices, indices) = triangles();
    let bytes = (vertices.len() * Vertex::STRIDE + indices.len() * 4) as u64;

    let mut reference = GpuMesh::new(&harness.gpu);
    reference.upload(&harness.gpu, &vertices, &indices);

    // Three keys whose spans meet exactly, which is what the slot layout hands
    // a settle for the keys it has just placed.
    let mut merged = GpuMesh::new(&harness.gpu);
    assert!(merged.reserve(&harness.gpu, vertices.len(), indices.len()));
    harness.gpu.take_uploaded_bytes();
    harness.gpu.take_writes();
    merged.patch_vertex_runs(
        &harness.gpu,
        &mut [
            (6, &vertices[6..9]),
            (0, &vertices[0..3]),
            (3, &vertices[3..6]),
        ],
    );
    merged.patch_index_runs(
        &harness.gpu,
        &mut [
            (3, &indices[3..6]),
            (0, &indices[0..3]),
            (6, &indices[6..9]),
        ],
    );
    assert_eq!(
        harness.gpu.take_writes(),
        2,
        "three abutting spans in each buffer should have cost one write each"
    );
    assert_eq!(
        harness.gpu.take_uploaded_bytes(),
        bytes,
        "merging must not change what is transferred"
    );
    merged.set_index_count(indices.len() as u32);
    merged.set_bounds(Vertex::bounds(&vertices));

    // And the merged bytes must be the bytes. A copy that mis-placed a span
    // would draw a different picture, not a smaller one.
    let mut camera = Camera::default();
    camera.frame_default();
    let expected = harness.capture(&reference, &camera, true, "merged-runs-reference");
    let actual = harness.capture(&merged, &camera, true, "merged-runs");
    let background = harness.background();
    assert!(
        expected
            .pixels
            .chunks_exact(4)
            .filter(|pixel| *pixel != background)
            .count()
            > 100,
        "the reference triangles must actually be visible"
    );
    assert_eq!(actual.pixels, expected.pixels);
}

#[test]
fn spans_with_a_gap_between_them_are_written_separately() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let (vertices, indices) = triangles();
    let mut mesh = GpuMesh::new(&harness.gpu);
    assert!(mesh.reserve(&harness.gpu, vertices.len(), indices.len()));

    // The middle key untouched: its neighbours must not be merged across it,
    // or a settle would overwrite geometry nobody asked it to touch.
    harness.gpu.take_writes();
    mesh.patch_vertex_runs(
        &harness.gpu,
        &mut [(0, &vertices[0..3]), (6, &vertices[6..9])],
    );
    assert_eq!(harness.gpu.take_writes(), 2);
}
