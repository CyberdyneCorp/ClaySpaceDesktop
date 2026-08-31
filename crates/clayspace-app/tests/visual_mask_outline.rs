//! Whether an outline drawn over the form freezes what it was drawn around.
//!
//! `mask_lasso.rs` establishes that the enclosed side resists a brush and the
//! rest does not. That is the contract; this is the picture, and the picture is
//! what a sculptor judges the gesture by. A lasso whose region lands a
//! centimetre from the line is one nobody can aim, and no assertion about
//! surface radii would notice.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_mask_outline
//! open target/visual
//! ```

mod support;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Direction, MaskGesture, MaskModel, MaskOutline, OutlineDraft, OutlineFrame, OutlineMode,
};
use clayspace_view::{Camera, Image, Vertex};
use support::{framed, Harness};

/// A sphere crossed into a mesh layer, so the viewport has vertices to carry
/// the mask on.
fn meshed_sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    document.convert_layer(Direction::SdfToMesh, 0.03, 0).ok()?;
    Some(document)
}

/// The vertices the viewport would upload, mask weights and all.
fn viewport_geometry(document: &mut ClayDocument) -> (Vec<Vertex>, Vec<u32>) {
    let (positions, normals, colors, indices, _) = document.visible_mesh_geometry();
    let frozen = document.mask_at(&positions);
    let vertices = positions
        .into_iter()
        .zip(normals)
        .zip(colors)
        .enumerate()
        .map(|(at, ((position, normal), color))| Vertex {
            position,
            normal,
            color,
            mask: frozen.as_ref().map_or(0.0, |weights| weights[at]),
        })
        .collect();
    (vertices, indices)
}

/// The frame this camera would carry an outline onto.
///
/// The same construction `App::outline_frame` makes — the plane through what is
/// being looked at, perpendicular to the view, with `right` and `up` taken from
/// the camera's own rays. Built here from the same public calls rather than
/// reached for through the binary, so what this measures is the gesture rather
/// than a test-only arrangement of it.
fn frame_of(camera: &Camera, at: [f32; 3]) -> OutlineFrame {
    let aspect = Harness::WIDTH as f32 / Harness::HEIGHT as f32;
    let through = |ndc: [f32; 2]| camera.ray_through(ndc, aspect);
    let (_, forward) = through([0.0, 0.0]);
    let on_plane = |ndc: [f32; 2]| {
        let (origin, direction) = through(ndc);
        let denominator: f32 = (0..3).map(|i| direction[i] * forward[i]).sum();
        let reach: f32 = (0..3)
            .map(|i| (at[i] - origin[i]) * forward[i])
            .sum::<f32>()
            / denominator;
        std::array::from_fn::<f32, 3, _>(|i| origin[i] + direction[i] * reach)
    };
    let origin = on_plane([0.0, 0.0]);
    let across = on_plane([1.0, 0.0]);
    let above = on_plane([0.0, 1.0]);
    let length = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let away = |b: [f32; 3]| std::array::from_fn::<f32, 3, _>(|i| b[i] - origin[i]);
    let (width, height) = (length(away(across)), length(away(above)));
    OutlineFrame {
        origin,
        right: away(across).map(|c| c / width),
        up: away(above).map(|c| c / height),
        forward,
        scale: [width, height],
    }
}

/// An outline over one half of the viewport, in normalised device coordinates.
fn half_of_the_screen(frame: OutlineFrame, from: f32, to: f32) -> MaskOutline {
    let outline = [[from, -0.9], [to, -0.9], [to, 0.9], [from, 0.9]];
    MaskOutline {
        outline: outline.iter().map(|ndc| frame.from_ndc(*ndc)).collect(),
        frame,
        mode: OutlineMode::Freeze,
    }
}

/// The same region, dragged as a rectangle the way the pointer drags one.
///
/// Through `OutlineDraft` rather than by writing four corners, so what this
/// measures includes the step that turns a press and a drag into a box.
fn dragged_box(frame: OutlineFrame, from: [f32; 2], to: [f32; 2]) -> MaskOutline {
    let mut draft = OutlineDraft::new(from, OutlineMode::Freeze, MaskGesture::Rectangle);
    // Several moves, as a hand makes: only the last one decides the corner.
    for at in [[to[0] * 0.3, to[1] * 0.8], [to[0] * 0.7, to[1] * 0.2], to] {
        draft.extend(at);
    }
    draft.onto(frame)
}

/// Pixels that moved, and the mean signed luminance change across them.
///
/// The mean is over the changed pixels only, so it means nothing until enough
/// of them changed: assert on the count first, as every caller here does.
fn difference(before: &Image, after: &Image) -> (usize, f32) {
    let luma = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;
    let mut changed = 0usize;
    let mut total = 0i32;
    for (a, b) in before
        .pixels
        .chunks_exact(4)
        .zip(after.pixels.chunks_exact(4))
    {
        let delta = luma(b) - luma(a);
        if delta.abs() > 12 {
            changed += 1;
            total += delta;
        }
    }
    (changed, total as f32 / changed.max(1) as f32)
}

/// The same, over one vertical band of the frame.
fn difference_in_band(before: &Image, after: &Image, from: f32, to: f32) -> usize {
    let luma = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;
    let width = before.width as usize;
    let (first, last) = (
        (from * width as f32) as usize,
        ((to * width as f32) as usize).min(width),
    );
    let mut changed = 0usize;
    for (at, (a, b)) in before
        .pixels
        .chunks_exact(4)
        .zip(after.pixels.chunks_exact(4))
        .enumerate()
    {
        let column = at % width;
        if column < first || column >= last {
            continue;
        }
        if (luma(b) - luma(a)).abs() > 12 {
            changed += 1;
        }
    }
    changed
}

#[test]
fn an_outline_freezes_the_half_of_the_form_it_was_drawn_around() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed_sphere() else {
        return;
    };
    let camera = framed(&document);

    let (plain, indices) = viewport_geometry(&mut document);
    assert!(
        !indices.is_empty(),
        "the fixture offered the viewport no triangles"
    );
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &plain, &indices);
    let before = harness.capture(&mesh, &camera, false, "78-lasso-none");

    // The left half of the viewport, drawn from well outside the form on both
    // sides — which is how a lasso is actually drawn.
    let frame = frame_of(&camera, camera.target.into());
    document
        .apply_outline(&half_of_the_screen(frame, -0.95, 0.0))
        .expect("the lasso");
    let (frozen, indices) = viewport_geometry(&mut document);
    assert!(
        frozen.iter().any(|v| v.mask > 0.5),
        "the lasso reached no vertex, so this would be measuring nothing"
    );
    mesh.upload(&harness.gpu, &frozen, &indices);
    let after = harness.capture(&mesh, &camera, false, "79-lasso-half-frozen");

    let (changed, mean) = difference(&before, &after);
    assert!(
        changed > 500,
        "the lasso changed {changed} pixels — the frozen region is invisible. \
         See target/visual/79-lasso-half-frozen.png"
    );
    assert!(
        mean < -20.0,
        "the frozen region changed by {mean} per pixel: a mask reads as darker \
         clay, and this got lighter"
    );

    // And it stopped where it was drawn. The outline ran to the middle of the
    // viewport, so the form's right-hand side must be untouched — a region
    // that bled past the line would be one nobody could aim. The bands are
    // over the form rather than over the frame: the framed sphere covers
    // roughly the middle two fifths, and a band of background would satisfy
    // "nothing changed" for the wrong reason.
    let bled = difference_in_band(&before, &after, 0.55, 0.68);
    assert_eq!(
        bled, 0,
        "{bled} pixels changed on the side the outline never covered. See \
         target/visual/79-lasso-half-frozen.png"
    );
    let covered = difference_in_band(&before, &after, 0.33, 0.45);
    assert!(
        covered > 100,
        "only {covered} pixels changed on the side the outline covered"
    );
}

#[test]
fn a_dragged_box_freezes_the_same_region_a_traced_one_does() {
    // The two gestures are two ways of saying the same thing to the same mask,
    // and this is the picture that holds them to it: a box dragged corner to
    // corner over the left half of the form has to land where an outline
    // traced round the same half lands.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut traced) = meshed_sphere() else {
        return;
    };
    let Some(mut dragged) = meshed_sphere() else {
        return;
    };
    let camera = framed(&traced);
    let frame = frame_of(&camera, camera.target.into());

    traced
        .apply_outline(&half_of_the_screen(frame, -0.95, 0.0))
        .expect("the traced outline");
    dragged
        .apply_outline(&dragged_box(frame, [-0.95, -0.9], [0.0, 0.9]))
        .expect("the dragged box");

    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    let (vertices, indices) = viewport_geometry(&mut traced);
    mesh.upload(&harness.gpu, &vertices, &indices);
    let a = harness.capture(&mesh, &camera, false, "82-outline-traced");

    let (vertices, indices) = viewport_geometry(&mut dragged);
    assert!(
        vertices.iter().any(|v| v.mask > 0.5),
        "the dragged box reached no vertex"
    );
    mesh.upload(&harness.gpu, &vertices, &indices);
    let b = harness.capture(&mesh, &camera, false, "83-outline-dragged-box");

    let (changed, _) = difference(&a, &b);
    assert!(
        changed < 200,
        "{changed} pixels differ between a traced outline and a dragged box \
         over the same region. See target/visual/83-outline-dragged-box.png"
    );
}

#[test]
fn drawing_over_the_same_form_twice_releases_what_it_froze() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed_sphere() else {
        return;
    };
    let camera = framed(&document);
    let frame = frame_of(&camera, camera.target.into());

    let (plain, indices) = viewport_geometry(&mut document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &plain, &indices);
    let before = harness.capture(&mesh, &camera, false, "80-lasso-clean");

    document
        .apply_outline(&half_of_the_screen(frame, -0.95, 0.0))
        .expect("the lasso");
    document
        .apply_outline(&MaskOutline {
            mode: OutlineMode::Thaw,
            ..half_of_the_screen(frame, -0.95, 0.0)
        })
        .expect("the lasso");

    let (released, indices) = viewport_geometry(&mut document);
    mesh.upload(&harness.gpu, &released, &indices);
    let after = harness.capture(&mesh, &camera, false, "81-lasso-released");

    let (changed, _) = difference(&before, &after);
    assert!(
        changed < 100,
        "{changed} pixels are still dark after the same outline released what \
         it froze. See target/visual/81-lasso-released.png"
    );
}
