//! Whether a painted mask reaches the screen.
//!
//! A mask that cannot be seen is worse than no mask. A sculptor who freezes a
//! region and then finds a brush doing nothing there has no way to tell a
//! protected surface from a broken tool — and `masking.rs` says the protection
//! is near-total, so that is exactly the experience an invisible mask gives.
//!
//! The engine had every piece of this and drew none of it: `clay_mask_sample`
//! has been there all along, the mask menu has been there all along, and the
//! viewport had no idea either existed. These run the path a sculptor runs —
//! paint a mask, assemble the vertices the viewport would upload, render a
//! frame — and measure the picture.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_mask
//! open target/visual
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, GestureSample, MaskModel, MaskOp, SculptModel, ToolKind,
};
use clayspace_view::{Camera, Image, Vertex};
use support::Harness;

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

/// Freezes the patch facing the camera.
fn freeze_the_near_face(document: &mut ClayDocument) {
    let brush = BrushSettings {
        size: 0.35,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    let samples: Vec<GestureSample> = (0..=6)
        .filter_map(|step| {
            let t = step as f32 / 6.0;
            SculptModel::pick(document, [-0.2 + t * 0.4, 0.0, 4.0], [0.0, 0.0, -1.0]).map(|hit| {
                GestureSample {
                    position: hit,
                    pressure: 1.0,
                    time: t,
                }
            })
        })
        .collect();
    document
        .apply_stroke(ToolKind::Mascara, brush, &samples, [false; 3])
        .expect("the mask stroke was refused");
}

/// The vertices the viewport would upload, mask weights and all.
///
/// The same two calls `App::sync_mesh_layers` makes, in the same order, so
/// this measures the assembly the application performs rather than a
/// test-only one that could drift from it.
fn viewport_geometry(document: &mut ClayDocument) -> (Vec<Vertex>, Vec<u32>) {
    let (positions, normals, colors, indices) = document.visible_mesh_geometry();
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

/// How many pixels differ between two frames, and by how much on average over
/// those pixels.
/// Mean absolute luminance difference across every pixel.
///
/// Every pixel, deliberately. `difference` averages over only the pixels that
/// crossed its threshold, so when a change is small its denominator is small
/// too and the mean is whatever a handful of pixels happened to do. That is
/// fine for "did this region get darker" and useless for comparing two
/// pictures, which is what the mask tests need.
fn distance(a: &Image, b: &Image) -> f32 {
    let luma = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;
    let total: i64 = a
        .pixels
        .chunks_exact(4)
        .zip(b.pixels.chunks_exact(4))
        .map(|(x, y)| (luma(y) - luma(x)).unsigned_abs() as i64)
        .sum();
    total as f32 / (a.pixels.len() / 4).max(1) as f32
}

/// Pixels that moved, and the mean signed luminance change across them.
///
/// The mean is over the changed pixels only, so it is meaningful only once
/// enough of them changed: assert on `changed` before believing `mean`, as
/// both callers here do. Skipping that guard is how this file once shipped a
/// test that passed on Linux because *nothing* changed — a mean of zero over
/// zero pixels satisfies a "did not get darker" assertion perfectly.
fn difference(before: &Image, after: &Image) -> (usize, f32) {
    let mut changed = 0usize;
    let mut total = 0i32;
    for (a, b) in before
        .pixels
        .chunks_exact(4)
        .zip(after.pixels.chunks_exact(4))
    {
        // Signed, and on luminance: "the frozen region got darker" is the
        // claim, and an unsigned count would be just as happy with lighter.
        let luma = |p: &[u8]| p[0] as i32 + p[1] as i32 + p[2] as i32;
        let delta = luma(b) - luma(a);
        if delta.abs() > 12 {
            changed += 1;
            total += delta;
        }
    }
    (changed, total as f32 / changed.max(1) as f32)
}

fn framed(document: &ClayDocument) -> Camera {
    let mut camera = Camera::default();
    match SculptModel::bounds(document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }
    camera
}

#[test]
fn a_painted_mask_darkens_the_surface_it_freezes() {
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
    let before = harness.capture(&mesh, &camera, false, "70-mask-none");

    freeze_the_near_face(&mut document);
    let (frozen, indices) = viewport_geometry(&mut document);
    assert!(
        frozen.iter().any(|v| v.mask > 0.5),
        "the mask stroke reached no vertex, so this would be measuring nothing"
    );
    mesh.upload(&harness.gpu, &frozen, &indices);
    let after = harness.capture(&mesh, &camera, false, "71-mask-painted");

    let (changed, mean) = difference(&before, &after);
    assert!(
        changed > 500,
        "painting a mask changed {changed} pixels — the frozen region is \
         invisible, and a brush that refuses to move it reads as a broken \
         tool. See target/visual/71-mask-painted.png"
    );
    assert!(
        mean < -20.0,
        "the frozen region changed by {mean} per pixel: a mask should read as \
         darker clay, and this got lighter"
    );
    assert!(
        changed < plain.len(),
        "{changed} pixels changed, which is most of the frame — the mask is \
         being drawn over clay it never froze"
    );
}

#[test]
fn clearing_the_mask_gives_the_surface_back() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed_sphere() else {
        return;
    };
    let camera = framed(&document);

    let (plain, indices) = viewport_geometry(&mut document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &plain, &indices);
    let before = harness.capture(&mesh, &camera, false, "72-mask-before");

    freeze_the_near_face(&mut document);
    document.apply_mask_op(MaskOp::Clear).expect("clear");

    let (cleared, indices) = viewport_geometry(&mut document);
    assert!(
        cleared.iter().all(|v| v.mask == 0.0),
        "a cleared mask still reads as painted on the vertices"
    );
    mesh.upload(&harness.gpu, &cleared, &indices);
    let after = harness.capture(&mesh, &camera, false, "73-mask-cleared");

    // Past the driver's noise, not bit-identical: `difference` counts a
    // luminance shift of twelve, which a macOS runner crosses on 69 pixels of
    // an unchanged frame. The frozen tint this is looking for reaches 2,587
    // pixels past `RENDER_NOISE`; unchanged frames reach none.
    let changed = support::differing_pixels(&before, &after);
    assert_eq!(
        changed, 0,
        "clearing the mask left {changed} pixels changed, so the frozen shading \
         outlives the mask that put it there"
    );
}

// -- the brick surface -------------------------------------------------------
//
// The other half, and the common one. A pure-SDF document is drawn from the
// brick cache through `SurfaceGeometry`, which uploads incrementally: only the
// bricks an edit dirtied are re-meshed and re-written. A mask stroke dirties
// none of them — it moves no clay — so the incremental path has nothing to
// carry the frozen region and would leave it undrawn however dark the shader
// paints it. `refresh_mask` is the path that exists for that, and this is what
// holds it.

#[test]
fn a_mask_on_a_field_reaches_the_drawn_surface() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(policy) = BackendPolicy::discover(None).ok() else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };
    let camera = framed(&document);

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("the first mesh");
    let before = harness.capture(geometry.mesh(), &camera, false, "74-field-mask-none");

    freeze_the_near_face(&mut document);
    // Exactly what the application does when the mask's revision moves — and
    // deliberately *without* a `sync`, because a sync is what a mask stroke
    // does not earn: it dirties no brick, and the whole point is that the
    // frozen region appears anyway.
    geometry.refresh_mask(&harness.gpu, &document);
    let after = harness.capture(geometry.mesh(), &camera, false, "75-field-mask-painted");

    let (changed, mean) = difference(&before, &after);
    assert!(
        changed > 500,
        "a mask painted on a field changed {changed} pixels of the drawn \
         surface. The brick path uploads only what an edit dirtied, and a mask \
         stroke dirties nothing — so without a pass of its own the sculptor \
         freezes a region and watches nothing happen. See \
         target/visual/75-field-mask-painted.png"
    );
    assert!(
        mean < -20.0,
        "the frozen region on a field changed by {mean} per pixel, which is \
         not darker clay"
    );
}

#[test]
fn a_dab_beside_the_mask_does_not_wipe_it_off() {
    // The incremental path replaces a brick's vertices outright, so a stroke
    // near a frozen region re-meshes bricks whose old vertices carried the
    // mask. Whatever comes back has to carry it too, or a sculptor's mask
    // erodes as they work around it — the failure that would show up as
    // "the mask keeps disappearing" and be blamed on the mask.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(policy) = BackendPolicy::discover(None).ok() else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };
    let camera = framed(&document);

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.sync(&harness.gpu, &mut document).expect("mesh");
    // The same surface with no mask on it, kept as the thing the picture must
    // *not* drift towards. Comparing against it is what makes this test say
    // something: an absolute "did the pixels move" threshold cannot tell a
    // mask being wiped off from the dab's own shading, and on a platform where
    // the dab is subtle it cannot tell either from nothing at all.
    let plain = harness.capture(geometry.mesh(), &camera, false, "75-field-mask-absent");

    freeze_the_near_face(&mut document);
    geometry.refresh_mask(&harness.gpu, &document);
    let masked = harness.capture(geometry.mesh(), &camera, false, "76-field-mask-kept-before");

    // If the mask is not plainly visible to begin with, nothing below can
    // distinguish it surviving from it never having been drawn.
    let mask_shows = distance(&plain, &masked);
    assert!(
        mask_shows > 1.0,
        "painting a mask changed the picture by only {mask_shows:.2} per pixel, \
         so this test cannot tell whether a mask survives a re-mesh. See \
         target/visual/76-field-mask-kept-before.png"
    );

    // A dab overlapping the frozen patch, which the mask itself resists.
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.25,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the dab was refused");
    geometry.sync(&harness.gpu, &mut document).expect("re-mesh");
    let after = harness.capture(geometry.mesh(), &camera, false, "77-field-mask-kept-after");

    // The question is which of the two pictures the dabbed one resembles: the
    // masked surface it should still be, or the bare surface it would become
    // if the re-mesh handed back vertices that had forgotten they were frozen.
    // A ratio rather than a threshold, because the dab's own contribution to
    // the picture differs by platform and backend and is not what is on trial.
    let from_masked = distance(&masked, &after);
    let from_plain = distance(&plain, &after);
    assert!(
        from_masked * 4.0 < from_plain,
        "after a dab beside it the surface is {from_masked:.2} per pixel from \
         the masked picture and {from_plain:.2} from the unmasked one, so the \
         re-mesh lost the mask rather than carrying it. See \
         target/visual/77-field-mask-kept-after.png"
    );
    // Painted twice rather than kept: the mask laid over itself is darker than
    // the mask, and would move the picture away from both references at once.
    assert!(
        from_masked < mask_shows,
        "a dab moved the surface {from_masked:.2} per pixel from the masked \
         picture, more than painting the mask moved it in the first place \
         ({mask_shows:.2}), which is the mask being applied twice"
    );
}
