//! Which picture of a grid the viewport draws.
//!
//! A grid is boxes. Whether it should *look* like boxes is a separate
//! question, and the engine answers it plainly: the boxy picture is "correct
//! for hard-surface voxel work and for export, and the wrong picture of an
//! organic sculpt". It ships a mesher for each and keeps the choice an
//! argument rather than grid state, "so two hosts sharing a document cannot
//! disagree about what it looks like and one host can show both pictures of
//! one sculpt without mutating it".
//!
//! Two facts shape how this is wired, both measured rather than assumed:
//!
//! - `clay_voxel_mesh_smooth` carries **no normals**. Colour blends across a
//!   smooth surface, which has no facet to hold one palette entry, but a
//!   normal is the host's to work out — and without them the surface renders
//!   as a flat silhouette, which is what the first attempt looked like.
//! - It cannot be meshed a chunk at a time. `clay_voxel_mesh_chunks` is the
//!   greedy mesher alone, because greedy quads clamp to a chunk boundary
//!   exactly while surface nets place a vertex from a cell's neighbourhood and
//!   would tear. So the smooth picture is a settle, not a live path.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, GestureSample, SceneModel, SculptModel, SmoothBlur, ToolKind, VoxelDisplay,
};

fn sculpted() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document
        .add_voxel_layer("Voxels", 0.04)
        .expect("add a grid");
    for step in 0..40 {
        let t = step as f32 / 39.0;
        let angle = t * 6.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.28,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: [angle.cos() * 0.45, (t - 0.5) * 0.9, angle.sin() * 0.45],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("deposit");
    }

    // Detail for the blur to take down, placed on purpose: four single cells
    // standing off the surface. Before #139 a fractional falloff dithered the
    // rim of every dab and left specks like these by accident, and the blur
    // test read those as its subject — so the fixture measured the defect. A
    // grid dab is solid now, and an isolated voxel is one this fixture put
    // there.
    for at in [
        [0.62f32, 0.0, 0.0],
        [-0.62, 0.1, 0.0],
        [0.0, 0.52, 0.12],
        [0.0, -0.52, -0.12],
    ] {
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.04,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: at,
                    pressure: 1.0,
                    time: 0.0,
                }],
                [false; 3],
            )
            .expect("a speck");
    }
    document
}

/// The geometry the viewport would upload.
fn drawn(document: &mut ClayDocument) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, usize) {
    let (positions, normals, _, indices, _) = document.visible_mesh_geometry();
    (positions, normals, indices.len())
}

#[test]
fn a_grid_draws_its_form_until_it_is_asked_for_its_cells() {
    // The default. A sculptor is shaping a form, not a lattice, and the cells
    // are a fact about the storage — but the blur stays at zero, because a
    // default that silently deletes detail is the wrong one however good it
    // looks.
    let mut document = sculpted();
    assert_eq!(document.voxel_display(), VoxelDisplay::Smooth);
    assert_eq!(document.voxel_blur().passes(), 0);
    let (_, _, smooth) = drawn(&mut document);
    assert!(smooth > 0, "the grid drew nothing at all");
}

#[test]
fn the_two_pictures_are_different_surfaces_over_the_same_cells() {
    let mut document = sculpted();
    let (_, _, smooth) = drawn(&mut document);
    let cells = document.stats().objects;

    document
        .set_voxel_display(VoxelDisplay::Boxes, SmoothBlur::default())
        .expect("the boxes were refused");
    let (_, _, boxes) = drawn(&mut document);

    assert_ne!(
        boxes, smooth,
        "asking for the cells drew the same {smooth} indices"
    );
    assert!(boxes > 0, "the boxy picture drew nothing");
    assert_eq!(
        document.stats().objects,
        cells,
        "changing the picture changed the document; it is a display setting \
         and must touch no cell"
    );

    // And back, exactly.
    document
        .set_voxel_display(VoxelDisplay::Smooth, SmoothBlur::default())
        .expect("the smooth mesh was refused");
    assert_eq!(
        drawn(&mut document).2,
        smooth,
        "the smooth surface did not come back"
    );
}

#[test]
fn the_smooth_surface_carries_normals() {
    // The engine's does not — colour blends across a smooth surface and a
    // normal is the host's to work out. Without them the surface renders as a
    // flat silhouette, which is what the first attempt at this looked like, so
    // this is the assertion that would have caught it.
    let mut document = sculpted();
    document
        .set_voxel_display(VoxelDisplay::Smooth, SmoothBlur::default())
        .expect("the smooth mesh was refused");
    let (positions, normals, _) = drawn(&mut document);

    assert_eq!(normals.len(), positions.len());
    let flat = normals
        .iter()
        .filter(|n| n.iter().map(|c| c * c).sum::<f32>() < 0.5)
        .count();
    assert_eq!(flat, 0, "{flat} vertices carry no normal");
    // And they point in more than one direction, which a fallback would not.
    let up = normals.iter().filter(|n| n[1] > 0.99).count();
    assert!(
        up < normals.len() / 2,
        "{up} of {} normals point straight up, which is the fallback rather \
         than a surface",
        normals.len()
    );
}

#[test]
fn blurring_smooths_further_and_says_it_may_cost_detail() {
    // The trade the engine states: at 0 nothing is filtered and nothing can be
    // lost, but the surface still terraces; at 1 it reads as clay, and an
    // isolated voxel sits under the isolevel and is gone.
    let mut document = sculpted();
    document
        .set_voxel_display(VoxelDisplay::Smooth, SmoothBlur::new(0))
        .expect("refused");
    let (crisp, ..) = drawn(&mut document);

    document
        .set_voxel_display(VoxelDisplay::Smooth, SmoothBlur::new(1))
        .expect("refused");
    let (blurred, ..) = drawn(&mut document);

    assert!(
        blurred.len() < crisp.len(),
        "one pass of blur left {} vertices against {} — the filter is meant \
         to take the surface down, and taking detail with it is the trade the \
         setting exists to offer",
        blurred.len(),
        crisp.len()
    );
    assert!(!SmoothBlur::new(0).can_lose_detail());
    assert!(SmoothBlur::new(1).can_lose_detail());
}

#[test]
fn a_stroke_while_smooth_is_still_drawn() {
    // The settle is what rebuilds the smooth mesh, and a stroke that left the
    // old one on screen would show a sculptor the shape they had before their
    // last dab.
    let mut document = sculpted();
    document
        .set_voxel_display(VoxelDisplay::Smooth, SmoothBlur::default())
        .expect("refused");
    let before = drawn(&mut document).0.len();

    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.3,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: [0.0, 0.8, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the stroke was refused");
    document.resmooth_voxels().expect("the settle was refused");

    assert_ne!(
        drawn(&mut document).0.len(),
        before,
        "a dab made while the smooth picture was up did not reach it"
    );
}

#[test]
fn the_picture_is_a_setting_and_not_an_edit() {
    // Nothing here belongs in the history: two hosts sharing a document must
    // not disagree about it, which is why the engine keeps it an argument.
    let mut document = sculpted();
    let before = document.history().depth;
    document
        .set_voxel_display(VoxelDisplay::Smooth, SmoothBlur::new(1))
        .expect("refused");
    assert_eq!(
        document.history().depth,
        before,
        "changing the picture recorded an edit"
    );
}

#[test]
fn the_smooth_surface_keeps_up_with_the_brush() {
    // It used to be rebuilt only when a gesture settled, which was right while
    // the boxes were the default and wrong once the smooth surface is what a
    // sculptor sees: a form that waited for the pointer to come up would lag a
    // whole gesture behind the brush.
    //
    // It is rebuilt where the geometry is assembled now, guarded on the grid's
    // change count — a whole-grid mesh is 17 to 21 ms and a comparison is
    // nothing, so the cost is paid when something moved and not otherwise.
    let mut document = sculpted();
    let before = drawn(&mut document).0.len();

    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.3,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: [0.0, 0.8, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the stroke was refused");

    // No settle, no explicit rebuild: just what the viewport asks for.
    assert_ne!(
        drawn(&mut document).0.len(),
        before,
        "a dab did not reach the smooth surface without a settle, so the form \
         lags the brush by a whole gesture"
    );
}

#[test]
fn an_unchanged_grid_is_not_meshed_again() {
    // The guard that lets the rebuild sit on the frame path. Without it every
    // frame would pay for a whole-grid mesh to redraw a form nobody touched.
    let mut document = sculpted();
    let first = drawn(&mut document);
    let again = drawn(&mut document);
    assert_eq!(
        first.0, again.0,
        "two reads of an untouched grid gave different surfaces"
    );
    // The revision is what the viewport watches, and it must sit still too.
    let settled = document.mesh_revision();
    assert_eq!(
        settled,
        document.mesh_revision(),
        "an untouched grid kept moving its revision, so the viewport would \
         re-upload the same surface every frame"
    );
}

#[test]
fn a_display_change_does_not_remesh_a_hidden_grid() {
    // A whole-grid smooth mesh is 17 to 21 ms and nothing else in the settle
    // is close to it, so a document carrying six hidden grids paid over a
    // tenth of a second on the UI thread to change a picture none of them is
    // in. The mesh is not built until the frame that draws the grid, which is
    // the frame that needs it.
    let mut document = sculpted();
    let grid = document.scene().active.expect("the grid is active");
    document
        .set_layer_visible(grid, false)
        .expect("hide the grid");

    document
        .set_voxel_display(VoxelDisplay::Smooth, SmoothBlur::default())
        .expect("refused");

    assert_eq!(
        document.smoothed_grids(),
        0,
        "the display change meshed a grid nobody is looking at"
    );
    assert!(
        drawn(&mut document).0.is_empty(),
        "a hidden grid was handed to the viewport"
    );

    // And it is built the moment it is asked for, so what the sculptor gets
    // back is the picture they chose rather than the boxes they left.
    document.set_layer_visible(grid, true).expect("show it");
    let shown = drawn(&mut document);
    assert_eq!(
        document.smoothed_grids(),
        1,
        "the grid came back without its smooth surface being built"
    );
    assert!(
        !shown.0.is_empty(),
        "the grid came back with nothing to draw"
    );

    // The same surface a grid that was never hidden would have: skipping the
    // rebuild is a deferral and not a different picture.
    let mut always_shown = sculpted();
    always_shown
        .set_voxel_display(VoxelDisplay::Smooth, SmoothBlur::default())
        .expect("refused");
    assert_eq!(
        shown.0,
        drawn(&mut always_shown).0,
        "the deferred rebuild produced a different surface"
    );
}

#[test]
fn a_crossing_leaves_the_source_alone() {
    // A crossing reads its source and adds a layer beside it. The grid it read
    // has not moved a cell, so drawing the document afterwards must not mesh
    // it again — neither its chunks nor its smooth surface.
    use clayspace_model::Direction;
    let mut document = sculpted();
    let source = drawn(&mut document);
    document
        .convert_layer(Direction::VoxelToSdf, 0.04, 1)
        .expect("grid to field");
    let after = drawn(&mut document);
    assert_eq!(
        (document.meshed_chunks(), document.smoothed_grids()),
        (0, 0),
        "the crossing re-meshed the grid it read"
    );
    assert_eq!(after.0, source.0, "the source grid is drawn differently");
}

#[test]
fn a_crossing_out_of_the_field_leaves_the_field_alone() {
    // The other direction's half of the same rule: a field-to-grid crossing
    // beside its source changes no brick, so it leaves nothing for the brick
    // surface to re-mesh — which is what lets the application skip the
    // whole-surface settle it used to pay after every crossing.
    use clayspace_model::Direction;
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a starting form");
    document.take_dirty_keys();
    document
        .convert_layer(Direction::SdfToVoxel, 0.05, 1)
        .expect("field to grid");
    assert!(!Direction::SdfToVoxel.changes_the_field(false));
    assert!(
        document.dirty_keys().is_empty(),
        "a crossing that leaves the field alone dirtied {} bricks",
        document.dirty_keys().len()
    );
}
