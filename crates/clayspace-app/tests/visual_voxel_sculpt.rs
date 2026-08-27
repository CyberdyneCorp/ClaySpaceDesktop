//! Whether sculpting a voxel layer reaches the screen.
//!
//! Every test the voxel tools had asked the *grid* whether it had changed —
//! `voxel_tools.rs` at the engine, `visual_sculpting.rs` against a standalone
//! `VoxelGrid`. All of them passed, and none of them was the question a
//! sculptor asks, which is whether the stroke appears.
//!
//! It did not. The viewport draws two things: the surface built from the brick
//! cache, and the mesh layers. The cache holds the document's SDF field and a
//! voxel layer carries no SDF content — the engine says so at
//! `clay_document_add_voxel_layer` — so a document holding nothing but a
//! sculpted grid meshed to zero triangles and rendered as bare ground. The
//! grid changed, the stroke committed, the counters moved, and the screen was
//! empty.
//!
//! So these tests run the whole path: a stroke through `apply_stroke`, the
//! geometry the viewport would upload, and a rendered frame.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_voxel_sculpt
//! open target/visual
//! ```

mod support;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use clayspace_view::{Camera, Image};
use support::Harness;

/// A document whose only layer is a voxel grid with material in it.
///
/// Deposited through `apply_stroke`, which is the path the application uses:
/// filling the grid directly would prove the renderer works and say nothing
/// about whether a stroke reaches it.
fn sculpted() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy).ok()?;
    document.add_voxel_layer("Voxels", 0.05).ok()?;
    deposit(&mut document, 0.0);
    Some(document)
}

/// A ridge of material dragged across the grid, at a height.
fn deposit(document: &mut ClayDocument, height: f32) {
    let brush = BrushSettings {
        size: 0.3,
        ..BrushSettings::default()
    };
    for step in 0..10 {
        let t = step as f32 / 9.0;
        let samples = [GestureSample {
            position: [-0.35 + t * 0.7, height, 0.0],
            pressure: 1.0,
            time: t,
        }];
        document
            .apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])
            .expect("the stroke was refused");
    }
}

/// How many pixels the surface covers.
fn lit(image: &Image) -> usize {
    let ground = image.pixel(4, 4);
    image
        .pixels
        .chunks_exact(4)
        .filter(|p| (0..3).any(|c| p[c].abs_diff(ground[c]) > 12))
        .count()
}

#[test]
fn a_voxel_stroke_reaches_the_viewport() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = sculpted() else {
        return;
    };

    let (vertices, indices) = support::viewport_geometry(&mut document);
    assert!(
        !indices.is_empty(),
        "a sculpted voxel layer offered the viewport no triangles, so nothing \
         a sculptor does to a grid can appear"
    );

    let mut camera = Camera::default();
    match SculptModel::bounds(&document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }

    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);
    let image = harness.capture(&mesh, &camera, false, "69-voxel-stroke");
    let covered = lit(&image);
    assert!(
        covered > 2000,
        "the sculpted grid drew {covered} pixels — see \
         target/visual/69-voxel-stroke.png"
    );
}

#[test]
fn a_second_voxel_stroke_changes_what_is_drawn() {
    // The first test would pass on a viewport that drew the grid once and
    // never looked again. This is the other half: an edit after the first
    // upload has to change the picture.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = sculpted() else {
        return;
    };

    let mut camera = Camera::default();
    match SculptModel::bounds(&document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }

    let first_revision = document.mesh_revision();
    let (vertices, indices) = support::viewport_geometry(&mut document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);
    let before = harness.capture(&mesh, &camera, false, "69-voxel-before");

    deposit(&mut document, 0.35);

    let second_revision = document.mesh_revision();
    assert_ne!(
        first_revision, second_revision,
        "the grid changed and the revision did not, so the viewport would keep \
         its stale copy and never ask for the new triangles"
    );

    let (vertices, indices) = support::viewport_geometry(&mut document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);
    let after = harness.capture(&mesh, &camera, false, "69-voxel-after");

    let moved = (0..before.height)
        .flat_map(|y| (0..before.width).map(move |x| (x, y)))
        .filter(|(x, y)| before.pixel(*x, *y) != after.pixel(*x, *y))
        .count();
    assert!(
        moved > 1000,
        "a second stroke moved {moved} pixels; the grid is drawn once and then \
         frozen — see target/visual/69-voxel-before.png and -after.png"
    );
}

#[test]
fn hiding_a_voxel_layer_stops_drawing_it() {
    // The mesh-layer path skips hidden layers, and a voxel layer now travels
    // it. Left out, a hidden grid would be the one representation the eye
    // icon did nothing to.
    use clayspace_model::SceneModel;

    let Some(mut document) = sculpted() else {
        return;
    };
    let key = document.scene().active.expect("an active layer");

    let (_, indices) = support::viewport_geometry(&mut document);
    assert!(!indices.is_empty(), "nothing was drawn to begin with");

    document.set_layer_visible(key, false).expect("hide it");
    let (_, hidden) = support::viewport_geometry(&mut document);
    assert!(
        hidden.is_empty(),
        "a hidden voxel layer still offered {} indices to the viewport",
        hidden.len()
    );
}

/// Frame All finds a sculpted grid.
///
/// `layer_bounds` answers with a layer's SDF extent and a voxel layer carries
/// none, so this was `None` however much material the grid held: the camera
/// fell back to a default box and framed somewhere the sculpt was not.
#[test]
fn a_sculpted_grid_has_bounds_to_frame() {
    let Some(mut document) = sculpted() else {
        return;
    };
    let (min, max) = SculptModel::bounds(&document).expect(
        "a grid with material in it reported no extent, so Frame All has \
         nothing to frame",
    );
    // The ridge was dragged along x from -0.35 to 0.35 with a brush 0.3 wide,
    // so the extent must cover it and must not be the whole world.
    assert!(
        min[0] < -0.2 && max[0] > 0.2,
        "the extent {min:?}..{max:?} does not cover the stroke"
    );
    assert!(
        max[1] - min[1] < 1.0,
        "the extent {min:?}..{max:?} is taller than anything was deposited"
    );

    // And it follows the sculpt rather than being measured once.
    deposit(&mut document, 0.6);
    let (_, taller) = SculptModel::bounds(&document).expect("bounds");
    assert!(
        taller[1] > max[1],
        "material was added above {max:?} and the extent stayed at {taller:?}"
    );
}

/// A pointer ray finds the sculpt, so a press on a grid sculpts it.
///
/// The last link in the chain. The viewport turns a pointer position into a
/// ray and asks the document where the surface is; a `None` there is what
/// makes a press orbit the camera instead of laying material down. The field
/// a raycast marches holds no voxel content, so a grid answered `None` from
/// every direction however much was in it.
#[test]
fn a_pointer_ray_finds_a_sculpted_grid() {
    let Some(mut document) = sculpted() else {
        return;
    };

    // Down the axis the ridge was dragged along, from outside it.
    let hit = SculptModel::pick(&document, [0.0, 2.0, 0.0], [0.0, -1.0, 0.0]).expect(
        "a ray straight down onto a ridge of material found nothing, so a press \
         on a voxel layer orbits instead of sculpting",
    );
    assert!(
        hit[1] > 0.0 && hit[1] < 0.5,
        "the ray met the ridge at {hit:?}, which is not where the material is"
    );

    // A miss is still a miss: a pick that answered everywhere would put the
    // brush on empty space.
    assert!(
        SculptModel::pick(&document, [3.0, 3.0, 3.0], [0.0, 0.0, -1.0]).is_none(),
        "a ray nowhere near the grid reported a hit"
    );

    // And the point it reports is one a stroke can act on, which is the whole
    // reason the interface asks.
    let brush = BrushSettings {
        size: 0.3,
        ..BrushSettings::default()
    };
    let outcome = document
        .apply_stroke(
            ToolKind::Padrao,
            brush,
            &[GestureSample {
                position: hit,
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the stroke was refused");
    assert!(
        outcome.changed,
        "a stroke at the point the pick reported changed nothing"
    );
}

/// An edit re-meshes what it touched, not the sculpt.
///
/// The first version of this drawing meshed the whole grid on every change,
/// which is what `clay_voxel_mesh_smooth` offers and all it offers. Measured
/// on a 0.01 grid: a 3.2 ms dab cost **309 ms** to re-mesh, against a 50 ms
/// budget, and rising with the sculpt — the representation was drawn and
/// unusable. Draining the engine's own dirty-chunk set and meshing only those
/// keys costs 3.3 ms and does not rise.
///
/// Asserted as a count rather than as a duration. A millisecond budget on a
/// shared machine measures the machine; the count is what the claim actually
/// is — the work follows the edit and not the model.
#[test]
fn an_edit_remeshes_only_the_chunks_it_touched() {
    // A grid fine enough that the sculpt spans many chunks, so "all of it"
    // and "what one dab touched" are different numbers at all. At the 0.05
    // used elsewhere here the whole ridge fits in eight.
    let Some(policy) = BackendPolicy::discover(None).ok() else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy) else {
        return;
    };
    document
        .add_voxel_layer("Voxels", 0.01)
        .expect("a voxel layer");
    let wide = BrushSettings {
        size: 0.2,
        ..BrushSettings::default()
    };
    for step in 0..48 {
        let t = step as f32 / 47.0;
        let angle = t * std::f32::consts::TAU;
        document
            .apply_stroke(
                ToolKind::Padrao,
                wide,
                &[GestureSample {
                    position: [angle.cos() * 0.5, -0.2 + t * 0.4, angle.sin() * 0.5],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("the stroke was refused");
    }
    let _ = document.visible_mesh_geometry();
    let whole = document.meshed_chunks();
    assert!(
        whole > 16,
        "the sculpt occupies only {whole} chunks, so this proves nothing about \
         an edit costing less than the model"
    );

    // Nothing changed, so nothing is owed.
    let _ = document.visible_mesh_geometry();
    assert_eq!(
        document.meshed_chunks(),
        0,
        "a frame in which no grid changed still re-meshed chunks"
    );

    // One dab, and only its neighbourhood.
    let brush = BrushSettings {
        size: 0.1,
        ..BrushSettings::default()
    };
    document
        .apply_stroke(
            ToolKind::Padrao,
            brush,
            &[GestureSample {
                position: [0.0, 0.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the dab was refused");
    let _ = document.visible_mesh_geometry();
    let after = document.meshed_chunks();
    assert!(
        after > 0,
        "a dab that changed the grid re-meshed nothing, so the viewport is \
         showing the sculpt as it was before it"
    );
    assert!(
        after < whole,
        "a dab re-meshed {after} of the sculpt's {whole} chunks — the work \
         follows the model rather than the edit"
    );
    println!("a {whole}-chunk sculpt, and one dab re-meshed {after}");
}

/// The polygon counters count what is on screen.
///
/// They were fed by the brick cache alone, which holds the document's SDF
/// field. A document whose only layer is a sculpted grid drew triangles and
/// reported none of them: "Triângulos 0" over a visible sculpt, and the
/// detail line saying nothing had been meshed yet.
#[test]
fn the_counters_include_a_voxel_layer() {
    use clayspace_model::Detail;

    let Some(mut document) = sculpted() else {
        return;
    };
    let (_, indices) = support::viewport_geometry(&mut document);
    let drawn = indices.len() / 3;
    assert!(drawn > 0, "nothing was drawn to count");

    let stats = SculptModel::stats(&document);
    assert_eq!(
        stats.triangles, drawn,
        "the panel reports {} triangles over a sculpt drawing {drawn}",
        stats.triangles
    );
    assert_ne!(
        stats.detail,
        Detail::Pending,
        "the panel says nothing has been meshed yet over a sculpt that is on \
         screen"
    );
}

/// Two different documents can report the same geometry revision.
///
/// The number says "something changed", not "this is that document". The
/// viewport compares it to decide whether to re-upload the carried layers, so
/// a document swap has to *forget* it rather than compare against it — two
/// documents holding no mesh gestures and no grids both report the same thing,
/// and the viewport would go on drawing the one that was closed.
///
/// A tripwire: if the revision ever becomes an identity, this fails and the
/// forgetting in `after_document_replaced` can go.
#[test]
fn the_geometry_revision_is_not_a_document_identity() {
    let Some(policy) = BackendPolicy::discover(None).ok() else {
        return;
    };
    let Ok(mut empty) = ClayDocument::new(policy.clone()) else {
        return;
    };
    let Ok(mut sphere) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };
    assert_eq!(
        empty.mesh_revision(),
        sphere.mesh_revision(),
        "the revision now tells two documents apart; the viewport can compare \
         it after a swap instead of forgetting it"
    );
}
