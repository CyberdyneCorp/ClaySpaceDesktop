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
use clayspace_view::{Camera, Image, Vertex};
use support::Harness;

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// The carried-layer buffer, as the viewport assembles it.
fn carried(document: &mut ClayDocument) -> (Vec<Vertex>, Vec<u32>) {
    let (positions, normals, colors, indices) = document.visible_mesh_geometry();
    let vertices = positions
        .into_iter()
        .zip(normals)
        .zip(colors)
        .map(|((position, normal), color)| Vertex {
            position,
            normal,
            color,
        })
        .collect();
    (vertices, indices)
}

fn framed(document: &ClayDocument) -> Camera {
    let mut camera = Camera::default();
    match SculptModel::bounds(document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }
    camera
}

fn capture(harness: &Harness, document: &mut ClayDocument, camera: &Camera, name: &str) -> Image {
    let (vertices, indices) = carried(document);
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
