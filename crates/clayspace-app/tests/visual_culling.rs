//! Subtools the camera is not pointing at are not drawn.
//!
//! The renderer draws one call per carried subtool, and the comment saying a
//! handful of draw calls is noise has been right for as long as a scene held a
//! handful. It stops being right at fifty: every span is a draw, a bind, and a
//! full pass over geometry outside the frame.
//!
//! The test is about the *count*, not the picture — and the picture is the
//! other half of it. A cull that removed something visible would be a hole in
//! the frame, so each case here checks both that the right number of spans were
//! dropped and that the frame is the same one an uncalled renderer draws.

mod support;

use clayspace_view::{Camera, MeshSpan, Vertex};
use support::{save, Harness};

/// A cube of the given half-size at a point, as vertices and indices.
///
/// Solid rather than a quad: the surface pipeline culls back faces, and a
/// fixture that vanished when the camera swung round would be measuring the
/// winding rather than the frustum.
fn cube(center: [f32; 3], half: f32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        (
            [0.0, 0.0, 1.0],
            [
                [-1.0, -1.0, 1.0],
                [1.0, -1.0, 1.0],
                [1.0, 1.0, 1.0],
                [-1.0, 1.0, 1.0],
            ],
        ),
        (
            [0.0, 0.0, -1.0],
            [
                [1.0, -1.0, -1.0],
                [-1.0, -1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [1.0, 1.0, -1.0],
            ],
        ),
        (
            [1.0, 0.0, 0.0],
            [
                [1.0, -1.0, 1.0],
                [1.0, -1.0, -1.0],
                [1.0, 1.0, -1.0],
                [1.0, 1.0, 1.0],
            ],
        ),
        (
            [-1.0, 0.0, 0.0],
            [
                [-1.0, -1.0, -1.0],
                [-1.0, -1.0, 1.0],
                [-1.0, 1.0, 1.0],
                [-1.0, 1.0, -1.0],
            ],
        ),
        (
            [0.0, 1.0, 0.0],
            [
                [-1.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, -1.0],
                [-1.0, 1.0, -1.0],
            ],
        ),
        (
            [0.0, -1.0, 0.0],
            [
                [-1.0, -1.0, -1.0],
                [1.0, -1.0, -1.0],
                [1.0, -1.0, 1.0],
                [-1.0, -1.0, 1.0],
            ],
        ),
    ];
    for (normal, corners) in faces {
        let base = vertices.len() as u32;
        for corner in corners {
            vertices.push(Vertex {
                position: std::array::from_fn(|i| center[i] + corner[i] * half),
                normal,
                color: [1.0, 1.0, 1.0],
                mask: 0.0,
            });
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}

/// Two cubes far apart, as two spans of one buffer — which is how the
/// application hands carried subtools over.
fn two_subtools(apart: f32) -> (Vec<Vertex>, Vec<u32>, Vec<MeshSpan>) {
    let (near_vertices, near_indices) = cube([0.0, 0.0, 0.0], 1.0);
    let (far_vertices, far_indices) = cube([apart, 0.0, 0.0], 1.0);

    let offset = near_vertices.len() as u32;
    let split = near_indices.len() as u32;
    let mut vertices = near_vertices;
    vertices.extend(far_vertices);
    let mut indices = near_indices;
    indices.extend(far_indices.into_iter().map(|i| i + offset));

    let spans = vec![
        MeshSpan::new(clayspace_model::LayerKey(1), 0..split),
        MeshSpan::new(clayspace_model::LayerKey(2), split..indices.len() as u32),
    ];
    (vertices, indices, spans)
}

/// A camera framing the cube at the origin and nothing else.
fn on_the_near_cube() -> Camera {
    let mut camera = Camera::default();
    camera.frame_bounds([-1.0, -1.0, -1.0].into(), [1.0, 1.0, 1.0].into());
    camera
}

/// The subtool outside the view is not drawn, and the one inside it still is.
#[test]
fn a_subtool_outside_the_view_is_not_drawn() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let empty = clayspace_view::GpuMesh::new(&harness.gpu);
    let camera = on_the_near_cube();

    // Far enough that no part of it is within a hundred units of the frame.
    let (vertices, indices, spans) = two_subtools(400.0);
    let gpu = harness.gpu.clone();
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);

    let image = harness.capture(&empty, &camera, false, "97-culling-one-visible");
    let stats = harness.renderer.frame_stats();
    println!(
        "two subtools, one framed: {} draws, {} culled",
        stats.draw_calls, stats.culled
    );
    assert_eq!(
        stats.culled, 1,
        "the subtool four hundred units away was drawn"
    );
    save(&image, "97-culling-one-visible");

    // And the frame still shows the one that is there. A cull that emptied the
    // viewport would satisfy the count above and nothing else.
    let ground = harness.background();
    let covered = image.pixels_differing_from(ground, 6);
    assert!(
        covered > 2_000,
        "only {covered} pixels were drawn, so the visible subtool was culled too"
    );
}

/// Both are drawn when both are in view, and the picture is the same one an
/// unculled renderer draws.
#[test]
fn nothing_is_culled_when_everything_is_in_view() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let empty = clayspace_view::GpuMesh::new(&harness.gpu);

    let (vertices, indices, spans) = two_subtools(4.0);
    let gpu = harness.gpu.clone();
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);

    // Framed on both, so neither can be outside.
    let mut camera = Camera::default();
    camera.frame_bounds([-1.0, -1.0, -1.0].into(), [5.0, 1.0, 1.0].into());
    let both = harness.capture(&empty, &camera, false, "97-culling-both-visible");
    assert_eq!(
        harness.renderer.frame_stats().culled,
        0,
        "a subtool inside the view was culled, which is a hole in the frame"
    );

    // The same scene with the spans withheld draws the whole buffer in one
    // call and cannot cull anything. The two frames must agree.
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &[]);
    let unculled = harness.capture(&empty, &camera, false, "97-culling-unspanned");
    assert_eq!(
        harness.renderer.frame_stats().culled,
        0,
        "a buffer with no spans has nothing to cull"
    );
    let differing = support::differing_pixels(&both, &unculled);
    assert_eq!(
        differing, 0,
        "{differing} pixels differ between the culled and unculled frames"
    );
}

/// A span with no bounds is never culled.
///
/// The application fills the bounds in for every span it makes, and
/// `set_mesh_layers` works them out from the geometry — so the only spans that
/// end up without a box are the ones that name no triangles. The safe reading
/// of "I cannot say where this is" is "draw it", not "drop it": a wrong cull
/// is a hole in the frame, and a wrong draw is a draw call.
#[test]
fn a_span_that_states_no_bounds_is_always_drawn() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let empty = clayspace_view::GpuMesh::new(&harness.gpu);
    let gpu = harness.gpu.clone();

    let (vertices, indices) = cube([0.0, 0.0, 0.0], 1.0);
    let spans = vec![
        // An empty span, which bounds nothing.
        MeshSpan::new(clayspace_model::LayerKey(1), 0..0),
        MeshSpan::new(clayspace_model::LayerKey(2), 0..indices.len() as u32),
    ];
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);

    // Framed a long way off the cube, so a span that *could* be culled would
    // be. The empty one still must not be.
    let mut camera = Camera::default();
    camera.frame_bounds([-1.0, -1.0, -1.0].into(), [1.0, 1.0, 1.0].into());
    camera.target = [500.0, 0.0, 0.0].into();
    let _ = harness.capture(&empty, &camera, false, "97-culling-unbounded");

    let stats = harness.renderer.frame_stats();
    println!(
        "off-camera with an empty span: {} draws, {} culled",
        stats.draw_calls, stats.culled
    );
    assert_eq!(
        stats.culled, 1,
        "the cube out of view should have been culled and the empty span should not"
    );
}
