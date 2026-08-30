//! Blocking out a form and then sculpting it as a mesh.
//!
//! The workflow the three representations exist for, and the one the
//! application had no route through: `Direction` had four entries and none of
//! them ended in a mesh, so the sixteen mesh brushes were reachable only by
//! importing a file. And a mesh layer could not be sculpted with the pointer
//! even then — a pick against one is answered by the mesh sculptor's raycast,
//! which refused until the sculptor was built, which only a stroke did, and
//! the interface sends no stroke where the pick found nothing.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_to_mesh
//! open target/visual
//! ```

mod support;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Direction, GestureSample, SculptModel, ToolKind};
use clayspace_view::{Camera, Image};
use support::{framed, Harness};

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

fn capture(harness: &Harness, document: &mut ClayDocument, camera: &Camera, name: &str) -> Image {
    let (vertices, indices) = support::viewport_geometry(document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);
    harness.capture(&mesh, camera, false, name)
}

/// How many pixels the surface covers.
fn drawn(image: &Image) -> usize {
    let ground = image.pixel(4, 4);
    image
        .pixels
        .chunks_exact(4)
        .filter(|p| (0..3).any(|c| p[c].abs_diff(ground[c]) > 12))
        .count()
}

#[test]
fn a_field_becomes_a_mesh_and_the_mesh_can_be_sculpted() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };

    document
        .convert_layer(Direction::SdfToMesh, 0.04, 0)
        .expect("the crossing was refused");

    let camera = framed(&document);
    let before = capture(&harness, &mut document, &camera, "71-mesh-from-field");
    let covered = drawn(&before);
    assert!(
        covered > 4000,
        "the meshed field drew {covered} pixels — see \
         target/visual/71-mesh-from-field.png"
    );

    // A ridge dragged across the front of it, with a mesh verb. Every position
    // comes from the pick, which is what the interface does: it places a
    // stroke where the pointer met the surface and sends nothing where it met
    // none. Driving it any other way would prove the verb works and say
    // nothing about whether a sculptor can reach it.
    let brush = BrushSettings {
        size: 0.35,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    let mut placed = 0;
    for step in 0..8 {
        let t = step as f32 / 7.0;
        let origin = [-0.5 + t, -0.25 + t * 0.5, 3.0];
        let Some(hit) = SculptModel::pick(&document, origin, [0.0, 0.0, -1.0]) else {
            continue;
        };
        placed += 1;
        document
            .apply_stroke(
                ToolKind::Inflar,
                brush,
                &[GestureSample {
                    position: hit,
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("the mesh verb was refused");
    }
    assert!(
        placed >= 6,
        "the pointer found the mesh at only {placed} of 8 points along the \
         stroke, so a drag across it would keep dropping out"
    );

    let after = capture(&harness, &mut document, &camera, "71-mesh-sculpted");
    let moved = (0..before.height)
        .flat_map(|y| (0..before.width).map(move |x| (x, y)))
        .filter(|(x, y)| before.pixel(*x, *y) != after.pixel(*x, *y))
        .count();
    assert!(
        moved > 1500,
        "sculpting the mesh moved {moved} pixels — see \
         target/visual/71-mesh-from-field.png and -sculpted.png"
    );
}

#[test]
fn a_grid_becomes_a_mesh_that_draws() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };
    document
        .convert_layer(Direction::SdfToVoxel, 0.05, 0)
        .expect("into voxels");
    document
        .convert_layer(Direction::VoxelToMesh, 0.05, 0)
        .expect("out to a mesh");

    let camera = framed(&document);
    let image = capture(&harness, &mut document, &camera, "71-mesh-from-grid");
    let covered = drawn(&image);
    assert!(
        covered > 4000,
        "the meshed grid drew {covered} pixels — see \
         target/visual/71-mesh-from-grid.png"
    );
}

/// The polyframe draws the mesh's own edges over it.
///
/// ZBrush's PolyF, and it answers the one question a shaded surface hides: how
/// much geometry is actually there. A sculptor deciding whether a mesh wants
/// retopology — which is exactly what a crossing into a mesh hands them — is
/// reading its density, and a smooth grey ball says nothing about that.
///
/// Drawn through the renderer's mesh-layer path rather than the surface one,
/// because that is where a mesh layer lives and where the edges are built.
#[test]
fn the_polyframe_draws_a_mesh_layers_edges() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };
    document
        .convert_layer(Direction::SdfToMesh, 0.06, 0)
        .expect("the crossing was refused");

    let camera = framed(&document);
    let (vertices, indices, spans) = support::viewport_layers(&mut document);
    assert!(!indices.is_empty(), "nothing to draw edges on");
    harness
        .renderer
        .set_mesh_layers(&harness.gpu, &vertices, &indices, &spans);

    // Nothing in the surface slot: the mesh layer is the whole subject, and
    // the polyframe is drawn over the mesh layers rather than over it.
    let surface = clayspace_view::GpuMesh::new(&harness.gpu);

    let gpu = harness.gpu.clone();
    harness.renderer.set_polyframe(&gpu, false);
    let plain = harness.capture(&surface, &camera, false, "72-polyframe-off");
    harness.renderer.set_polyframe(&gpu, true);
    let wired = harness.capture(&surface, &camera, false, "72-polyframe-on");

    let differing = (0..plain.height)
        .flat_map(|y| (0..plain.width).map(move |x| (x, y)))
        .filter(|(x, y)| plain.pixel(*x, *y) != wired.pixel(*x, *y))
        .count();
    assert!(
        differing > 3000,
        "turning the polyframe on changed {differing} pixels, so it did not \
         draw — see target/visual/72-polyframe-on.png"
    );

    // Ink, not light: the lines are dark and translucent, so the surface they
    // cover has to come out darker on average rather than merely different.
    let mean = |image: &Image| -> f64 {
        let lit: Vec<u8> = image
            .pixels
            .chunks_exact(4)
            .filter(|p| (0..3).any(|c| p[c].abs_diff(image.pixel(4, 4)[c]) > 12))
            .map(|p| p[0])
            .collect();
        lit.iter().map(|&v| v as f64).sum::<f64>() / lit.len().max(1) as f64
    };
    let (before, after) = (mean(&plain), mean(&wired));
    assert!(
        after < before,
        "the surface reads {after:.1} with the polyframe on and {before:.1} \
         with it off; the edges are drawn in ink and must darken what they \
         cover"
    );

    // And off again is off: a toggle that only goes one way is not a toggle.
    harness.renderer.set_polyframe(&gpu, false);
    let again = harness.capture(&surface, &camera, false, "72-polyframe-off-again");
    // Past the noise floor. Measured on a macOS runner: turning the polyframe
    // *on* moves 10,249 pixels past `RENDER_NOISE` and 33,399 by any amount at
    // all; turning it off again leaves 1,294 differing by up to 17 levels and
    // not one past the threshold. The wireframe went away; the rasteriser did
    // not settle on the same handful of edge pixels twice.
    let lingering = support::differing_pixels(&plain, &again);
    assert_eq!(
        lingering, 0,
        "the polyframe stayed on after being turned off"
    );
}
