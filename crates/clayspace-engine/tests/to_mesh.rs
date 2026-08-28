//! Crossing into a mesh, and sculpting what comes out.
//!
//! The application could bring a mesh *in* and sculpt it, and could cross a
//! mesh out to either of the others. It could not produce one: `Direction` had
//! four entries and none of them ended in `Mesh`, so the sixteen mesh brushes
//! were reachable only by importing a file. Blocking out a form and then
//! sculpting it as a mesh — the workflow the three representations exist for —
//! had no route through the application at all.
//!
//! These tests take that route: convert, check what came out is a real mesh
//! layer, and sculpt it.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, Direction, GestureSample, LayerKey, Representation,
    SceneModel, SculptModel, ToolKind,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// The layer the scene says is active, and what it holds.
fn active(document: &ClayDocument) -> (LayerKey, Representation, bool) {
    let scene = document.scene();
    let key = scene.active.expect("something is always active");
    let layer = scene.layer(key).expect("the active layer is in the scene");
    (key, layer.representation, layer.visible)
}

#[test]
fn a_field_crosses_into_a_mesh_layer() {
    let mut document = document();
    let (source, _, _) = active(&document);

    let made = document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .expect("the crossing was refused");

    let scene = document.scene();
    assert_eq!(scene.layers.len(), 2, "a crossing adds a layer");
    let mesh = scene.layer(made).expect("the new layer");
    assert_eq!(mesh.representation, Representation::Mesh);
    assert_eq!(
        scene.active,
        Some(made),
        "the layer a crossing made is the one to work on"
    );

    // The source is untouched. That is what makes the crossing safe to offer:
    // it is not a replacement, so the way back is the layer that is still
    // there.
    let source_layer = scene.layer(source).expect("the source is still there");
    assert_eq!(source_layer.representation, Representation::Sdf);
    assert!(source_layer.visible, "the source was left hidden");

    // And it carries triangles, which is what makes it sculptable rather than
    // an empty row waiting for an import.
    let (positions, _, _, indices, _) = document.visible_mesh_geometry();
    assert!(
        indices.len() >= 3 && !positions.is_empty(),
        "the mesh layer offered the viewport {} indices",
        indices.len()
    );
}

#[test]
fn the_mesh_is_the_source_layer_and_not_the_whole_document() {
    // The engine meshes a document, not a layer. The crossing hides the other
    // SDF layers across the call, and this is what holds it to that: with a
    // second layer carrying a blob well outside the sphere, a whole-document
    // mesh reaches past x = 1.2 and the source's own does not.
    let mut document = document();
    let (sphere, _, _) = active(&document);

    let second = document
        .add_layer("Segunda", Representation::Sdf)
        .expect("a second layer");
    document.set_active_layer(second).expect("activate it");
    // The stroke default displaces a surface along its normal, and in empty
    // space there is none to displace. An additive stamp is what puts a blob
    // where there was nothing.
    document.set_combine(CombineSettings {
        op: Combine::Add,
        ..CombineSettings::for_strokes()
    });
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.4,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: [1.1, 0.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the stroke was refused");

    document
        .set_active_layer(sphere)
        .expect("back to the sphere");
    document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .expect("the crossing was refused");

    let (positions, _, _, _, _) = document.visible_mesh_geometry();
    let furthest = positions
        .iter()
        .map(|p| p[0])
        .fold(f32::MIN, |worst, x| worst.max(x));
    assert!(
        furthest < 1.1,
        "the mesh reaches x = {furthest}, so it meshed the whole document \
         rather than the layer that was crossed"
    );

    // And the second layer is visible again. A crossing that left the rest of
    // the sculpt hidden would read as having deleted it.
    let scene = document.scene();
    assert!(
        scene.layer(second).expect("the second layer").visible,
        "the layers hidden for the mesher were not put back"
    );
}

#[test]
fn a_grid_crosses_into_a_mesh_layer() {
    let mut document = document();
    document
        .convert_layer(Direction::SdfToVoxel, 0.05, 0)
        .expect("into voxels");
    let made = document
        .convert_layer(Direction::VoxelToMesh, 0.05, 0)
        .expect("out to a mesh");

    let scene = document.scene();
    let layer = scene.layer(made).expect("the new layer");
    assert_eq!(layer.representation, Representation::Mesh);

    let (positions, normals, _, indices, _) = document.visible_mesh_geometry();
    assert!(!indices.is_empty(), "the grid meshed to nothing");
    // Normals, because a mesh layer is lit by them. The rounded voxel mesher
    // carries none and would draw as a flat silhouette.
    assert_eq!(normals.len(), positions.len());
    assert!(
        normals.iter().any(|n| n != &[0.0, 1.0, 0.0]),
        "every normal is the fallback, so the mesh carried none of its own"
    );
}

#[test]
fn what_a_crossing_makes_can_be_sculpted() {
    // The whole point. A mesh layer is sculpted by moving the vertices it has,
    // and until this crossing existed the only way to get one was a file.
    let mut document = document();
    document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .expect("the crossing was refused");

    let before = document.visible_mesh_geometry().0;
    assert!(!before.is_empty(), "nothing to sculpt");

    // On the surface the crossing produced, so the brush has something under
    // it — which is also what `pick` answers for a mesh layer.
    let hit = SculptModel::pick(&document, [0.0, 0.0, 3.0], [0.0, 0.0, -1.0])
        .expect("a ray onto the meshed sphere found nothing");

    let outcome = document
        .apply_stroke(
            ToolKind::Inflar,
            BrushSettings {
                size: 0.4,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: hit,
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the mesh verb was refused");
    assert!(outcome.changed, "the stroke moved no vertices");

    let after = document.visible_mesh_geometry().0;
    assert_eq!(
        after.len(),
        before.len(),
        "a mesh verb changed the vertex count; nothing here retessellates"
    );
    let moved = before.iter().zip(&after).filter(|(a, b)| a != b).count();
    assert!(moved > 0, "no vertex moved");
}

#[test]
fn a_crossing_into_a_mesh_is_one_undo_step() {
    let mut document = document();
    document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .expect("the crossing was refused");
    assert_eq!(document.scene().layers.len(), 2);

    document.undo().expect("undo");
    assert_eq!(
        document.scene().layers.len(),
        1,
        "undo left the layer the crossing added, or took back more than it"
    );
}

/// The pointer can find a mesh layer the moment it becomes active.
///
/// The interface places a stroke where the pick reported and sends nothing
/// when it reports nothing. A pick against a mesh layer is answered by the
/// mesh sculptor's own raycast, and that used to refuse until the sculptor was
/// built — which only the first stroke did. So the first stroke could never
/// arrive: a mesh layer was unsculptable through the pointer, imported or
/// converted, and a press orbited the camera instead.
///
/// The sculptor is armed when the layer becomes the active one, so this is the
/// order the interface actually goes in: select, point, then stroke.
#[test]
fn a_mesh_layer_answers_the_pointer_before_its_first_stroke() {
    let mut document = document();
    let (sphere, _, _) = active(&document);
    let mesh = document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .expect("the crossing was refused");

    let ray = ([0.0, 0.0, 3.0], [0.0, 0.0, -1.0]);
    assert!(
        SculptModel::pick(&document, ray.0, ray.1).is_some(),
        "the layer a crossing just made does not answer the pointer, so the \
         first stroke can never be placed on it"
    );

    // And after going away and coming back, which is the path an imported mesh
    // takes: it is selected rather than created active.
    document.set_active_layer(sphere).expect("to the source");
    document.set_active_layer(mesh).expect("back to the mesh");
    assert!(
        SculptModel::pick(&document, ray.0, ray.1).is_some(),
        "selecting a mesh layer does not make it pointable"
    );

    // A ray that meets nothing still meets nothing. A pick that answered
    // everywhere would put the brush on empty space.
    assert!(
        SculptModel::pick(&document, [3.0, 3.0, 3.0], [0.0, 0.0, -1.0]).is_none(),
        "a ray nowhere near the mesh reported a hit"
    );
}

/// A layer the viewport must draw changes the number it watches.
///
/// The viewport uploads the carried layers only when `mesh_revision` changes,
/// and adding a mesh layer moves no vertex and touches no grid — so the number
/// did not change and the mesh was never uploaded. A crossing appeared to work
/// only because the *source* layer was still contributing to the field: the
/// sphere on screen was the field, not the mesh. Removing the source then left
/// an empty viewport with 62,576 vertices sitting unuploaded, and the first
/// stroke brought them back, which is exactly how it was reported.
#[test]
fn a_new_mesh_layer_changes_what_the_viewport_watches() {
    let mut document = document();
    let empty = document.mesh_revision();

    let made = document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .expect("the crossing was refused");
    let crossed = document.mesh_revision();
    assert_ne!(
        crossed, empty,
        "crossing into a mesh left the viewport's number where it was, so the \
         layer it made is never uploaded and never drawn"
    );
    let (positions, _, _, indices, _) = document.visible_mesh_geometry();
    assert!(
        !indices.is_empty() && !positions.is_empty(),
        "there was nothing to upload in the first place"
    );

    // Hiding it is the same question from the other side: the viewport has to
    // stop drawing it, and it only looks again when the number moves.
    document.set_layer_visible(made, false).expect("hide");
    let hidden = document.mesh_revision();
    assert_ne!(
        hidden, crossed,
        "hiding a mesh layer left the number where it was, so the viewport \
         goes on drawing it"
    );
    assert!(
        document.visible_mesh_geometry().3.is_empty(),
        "a hidden mesh layer is still offered to the viewport"
    );

    document.set_layer_visible(made, true).expect("show");
    assert_eq!(
        document.mesh_revision(),
        crossed,
        "showing it again should read as the state it was in before hiding"
    );
}
