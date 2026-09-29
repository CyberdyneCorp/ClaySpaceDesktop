//! Moving a whole carried subtool leaves the field's bricks alone.
//!
//! A mesh, a grid, a hierarchy and an adaptive surface hold no field content:
//! the engine carries their triangles and the layer transform places them on
//! the way out. Placing one used to refill its box in the brick cache anyway,
//! so every frame of a whole-subtool drag re-meshed whatever field shared
//! that region — the source a crossing leaves beside its result, or a form
//! the subtool was moved across. Measured on the mesh reference scene
//! (#196, D14): 45–54 ms of re-meshing per drag frame, for bricks that had not
//! changed, from the first frame of the gesture on.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Direction, ExchangeModel, GizmoTarget, ImportSettings, LayerKey, ObjectModel, Representation,
    SceneModel, SculptModel, Transform,
};

fn starting_form() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// The starting form crossed beside itself, so the field it came from is
/// still drawn under the result.
fn crossed(direction: Direction) -> (ClayDocument, LayerKey) {
    let mut document = starting_form();
    let key = document
        .convert_layer(direction, 0.05, 0)
        .expect("cross beside the source");
    document.set_active_layer(key).expect("activate the result");
    (document, key)
}

/// A flat quad sheet through the starting form, carried as `direction` makes
/// it. Quads, welded, so it is a cage a hierarchy accepts and a surface the
/// adaptive crossing accepts.
fn sheet(direction: Direction) -> (ClayDocument, LayerKey) {
    use std::fmt::Write;
    let (divisions, step) = (24usize, 0.1f32);
    let half = step * divisions as f32 / 2.0;
    let mut text = String::new();
    for z in 0..=divisions {
        for x in 0..=divisions {
            let _ = writeln!(
                text,
                "v {} 0 {}",
                -half + step * x as f32,
                -half + step * z as f32
            );
        }
    }
    let stride = divisions + 1;
    for z in 0..divisions {
        for x in 0..divisions {
            let a = z * stride + x + 1;
            let _ = writeln!(text, "f {} {} {} {}", a, a + stride, a + stride + 1, a + 1);
        }
    }
    let path = std::env::temp_dir().join(format!(
        "clayspace-carried-drag-{direction:?}-{}.obj",
        std::process::id()
    ));
    std::fs::write(&path, text).expect("write the sheet");
    let mut document = starting_form();
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import the sheet");
    let _ = std::fs::remove_file(&path);
    let mesh = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the sheet is a mesh layer");
    document.set_active_layer(mesh).expect("activate the sheet");
    let key = document
        .convert_layer_in_place(direction, 0.05, 0)
        .expect("carry the sheet");
    document.set_active_layer(key).expect("activate the result");
    (document, key)
}

fn nudged(document: &mut ClayDocument, target: GizmoTarget, dx: f32) -> Transform {
    let at = document
        .target_transform(target)
        .expect("a layer transform");
    Transform {
        position: [at.position[0] + dx, at.position[1], at.position[2]],
        ..at
    }
}

/// One whole-subtool drag frame on `key`, and what it cost the brick cache
/// and the carried buffer.
fn drag_frame(document: &mut ClayDocument, key: LayerKey) {
    let representation = document
        .scene()
        .layer(key)
        .map(|layer| layer.representation)
        .expect("the layer");
    assert_ne!(representation, Representation::Sdf, "a carried layer");
    let (first, ..) = document.visible_mesh_geometry();
    let revision = document.mesh_revision();
    document.take_dirty_keys();

    let target = GizmoTarget::Layer(key);
    document.begin_target_drag(target);
    for step in 1..=3 {
        let moved = nudged(document, target, 0.05);
        document
            .set_target_transform(target, moved)
            .expect("place the carried subtool");
        assert!(
            document.take_dirty_keys().is_empty(),
            "{representation:?}: drag frame {step} dirtied field bricks the move cannot change"
        );
    }
    document.end_target_drag();
    assert!(document.take_dirty_keys().is_empty());

    assert_ne!(
        document.mesh_revision(),
        revision,
        "{representation:?}: the carried buffer is not told the subtool moved"
    );
    let (after, ..) = document.visible_mesh_geometry();
    let moved = first
        .iter()
        .zip(&after)
        .map(|(a, b)| b[0] - a[0])
        .fold(0.0f32, f32::max);
    assert!(
        (moved - 0.15).abs() < 1e-3,
        "{representation:?}: the drawn triangles did not follow the move ({moved})"
    );
}

#[test]
fn moving_a_mesh_beside_its_source_field_dirties_no_brick() {
    let (mut document, key) = crossed(Direction::SdfToMesh);
    drag_frame(&mut document, key);
}

#[test]
fn moving_a_grid_beside_its_source_field_dirties_no_brick() {
    let (mut document, key) = crossed(Direction::SdfToVoxel);
    drag_frame(&mut document, key);
}

#[test]
fn moving_a_hierarchy_across_a_field_dirties_no_brick() {
    let (mut document, key) = sheet(Direction::MeshToMultires);
    drag_frame(&mut document, key);
}

#[test]
fn moving_an_adaptive_surface_across_a_field_dirties_no_brick() {
    let (mut document, key) = sheet(Direction::MeshToDynamic);
    drag_frame(&mut document, key);
}

/// The control: a field subtool is what the bricks are made of, so moving it
/// still refills both where it was and where it went.
#[test]
fn moving_a_field_subtool_still_refills_its_bricks() {
    let mut document = starting_form();
    let key = document.scene().active.expect("the starting layer");
    document.take_dirty_keys();
    let target = GizmoTarget::Layer(key);
    let moved = nudged(&mut document, target, 0.3);
    document
        .set_target_transform(target, moved)
        .expect("place the field subtool");
    assert!(!document.take_dirty_keys().is_empty());
}

/// A carried subtool's move is still one undo step that puts it back.
#[test]
fn a_carried_subtool_move_is_taken_back() {
    let (mut document, key) = crossed(Direction::SdfToMesh);
    let target = GizmoTarget::Layer(key);
    let before = document.target_transform(target).expect("where it stands");
    document.begin_target_drag(target);
    let moved = nudged(&mut document, target, 0.4);
    document
        .set_target_transform(target, moved)
        .expect("place the mesh subtool");
    document.end_target_drag();
    document.undo().expect("undo the move");
    assert_eq!(document.target_transform(target), Some(before));
}
