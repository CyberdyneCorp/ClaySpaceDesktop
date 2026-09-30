//! A placed object's own surface, for drawing its drag before the field moves.
//!
//! The first frame of a placed object's drag used to write the move and
//! re-mesh what it reached (#196, D14). The drag now draws the object alone,
//! from a mesh of its primitive taken once at the press; these check that the
//! mesh is the object's, that it is placed and reflected the way the engine
//! places the item, and that taking it writes nothing to the document.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    CombineSettings, DocumentModel, GizmoTarget, ObjectModel, SceneModel, SculptModel, Shape,
    Transform,
};

fn starting_form() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

fn length(v: [f32; 3]) -> f32 {
    v.iter().map(|c| c * c).sum::<f32>().sqrt()
}

fn centre(positions: &[[f32; 3]]) -> [f32; 3] {
    let n = positions.len() as f32;
    std::array::from_fn(|i| positions.iter().map(|p| p[i]).sum::<f32>() / n)
}

/// A sphere's preview is a sphere of its radius about its own origin, and
/// taking it touches neither the history nor the brick cache.
#[test]
fn a_placed_sphere_previews_as_its_own_surface_and_writes_nothing() {
    let mut document = starting_form();
    let id = document
        .place_object(
            Shape::Sphere,
            &[0.3],
            [0.9, 0.2, 0.0],
            CombineSettings::default(),
        )
        .expect("place a sphere");
    document.take_dirty_keys();
    let depth = document.history_depth();

    let preview = document.object_preview(id).expect("a sphere has a surface");
    assert!(
        preview.indices.len() >= 3 * 1000,
        "too coarse to read as a sphere"
    );
    assert_eq!(preview.positions.len(), preview.normals.len());
    // Within a cell of the radius: 48 cells across the diameter.
    let cell = 0.6 / 48.0;
    for &p in &preview.positions {
        assert!((length(p) - 0.3).abs() < cell, "{p:?} is not on the sphere");
    }
    assert_eq!(document.history_depth(), depth, "a preview wrote history");
    assert!(document.dirty_keys().is_empty(), "a preview dirtied bricks");
}

/// Under the starting symmetry the object is drawn with its X twin, which is
/// the image the engine emits of it; placed with symmetry off, it is drawn
/// alone, as the engine evaluates it.
#[test]
fn the_preview_reflects_the_object_only_where_the_engine_does() {
    let mut document = starting_form();
    let mirrored = document
        .place_object(
            Shape::Sphere,
            &[0.2],
            [0.9, 0.0, 0.0],
            CombineSettings::default(),
        )
        .expect("place a mirrored sphere");
    document.set_symmetry([false; 3]).expect("symmetry off");
    let alone = document
        .place_object(
            Shape::Sphere,
            &[0.2],
            [0.0, 0.9, 0.0],
            CombineSettings::default(),
        )
        .expect("place a one-sided sphere");

    let to = Transform::at([0.6, 0.5, 0.0]);
    let twin = document.object_preview(mirrored).unwrap();
    assert_eq!(twin.mirror, [true, false, false]);
    let posed = twin.posed(to);
    let half = posed.positions.len() / 2;
    assert!(length(sub(centre(&posed.positions[..half]), [0.6, 0.5, 0.0])) < 0.01);
    assert!(length(sub(centre(&posed.positions[half..]), [-0.6, 0.5, 0.0])) < 0.01);

    let single = document.object_preview(alone).unwrap();
    assert_eq!(single.mirror, [false; 3]);
    assert_eq!(single.posed(to).positions.len(), single.positions.len());
}

/// After a save and a reopen the preview still reflects exactly what the
/// engine reflects.
///
/// A reopened document builds each layer's mirror record as a fresh layer's,
/// "no mirror", whatever the file carries. The preview read that record first,
/// so a mirrored object reopened under X symmetry was drawn without its twin
/// while the twin stayed in the field, and jumped on release. The engine is
/// asked now; the object that opted out stays alone.
#[test]
fn a_reopened_document_previews_the_mirror_the_engine_evaluates() {
    let mut document = starting_form();
    let mirrored = document
        .place_object(
            Shape::Sphere,
            &[0.2],
            [0.9, 0.0, 0.0],
            CombineSettings::default(),
        )
        .expect("place a mirrored sphere");
    document.set_symmetry([false; 3]).expect("symmetry off");
    let alone = document
        .place_object(
            Shape::Sphere,
            &[0.2],
            [0.0, 0.9, 0.0],
            CombineSettings::default(),
        )
        .expect("place a one-sided sphere");
    let path = std::env::temp_dir().join(format!(
        "clayspace-object-preview-mirror-{}.clayspace",
        std::process::id()
    ));
    document.save(&path).expect("save");

    let mut reopened = starting_form();
    reopened.open(&path).expect("open");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(clayspace_engine::objects::sidecar_for(&path));

    let key = reopened.scene().active_layer().unwrap().key;
    let layer = reopened.layer_id(key).unwrap();
    let (carried, _) = reopened.document().layer_mirror(layer).unwrap();
    assert_eq!(carried, [true, false, false], "the file lost the mirror");
    assert_eq!(
        reopened.object_preview(mirrored).unwrap().mirror,
        carried,
        "a reopened mirrored object lost the twin the engine still draws"
    );
    assert_eq!(
        reopened.object_preview(alone).unwrap().mirror,
        [false; 3],
        "a one-sided object came back reflected"
    );
}

/// A subtool that has been moved places the preview with it: the object is
/// drawn where the manipulator says it stands, not where the node's own
/// values would put it.
#[test]
fn a_moved_subtool_places_its_objects_preview() {
    let mut document = starting_form();
    let id = document
        .place_object(
            Shape::Sphere,
            &[0.2],
            [0.9, 0.0, 0.0],
            CombineSettings::default(),
        )
        .expect("place a sphere");
    let layer = document.scene().active_layer().unwrap().key;
    let placement = Transform {
        position: [0.0, 1.0, 0.0],
        rotation_axis: [0.0, 0.0, 1.0],
        rotation_angle: std::f32::consts::FRAC_PI_2,
        scale: [1.0; 3],
    };
    document
        .set_target_transform(GizmoTarget::Layer(layer), placement)
        .expect("move the subtool");
    let world = document.target_transform(GizmoTarget::Object(id)).unwrap();

    let posed = document.object_preview(id).unwrap().posed(world);
    let half = posed.positions.len() / 2;
    assert!(
        length(sub(centre(&posed.positions[..half]), world.position)) < 0.01,
        "drawn at {:?}, standing at {:?}",
        centre(&posed.positions[..half]),
        world.position
    );
}

/// An object that is gone has nothing to draw, and its drag — should one
/// still be in flight — is evaluated live as before.
#[test]
fn a_removed_object_has_no_preview() {
    let mut document = starting_form();
    let id = document
        .place_object(
            Shape::Sphere,
            &[0.2],
            [0.9, 0.0, 0.0],
            CombineSettings::default(),
        )
        .expect("place a sphere");
    document.remove_object(id).expect("remove it");
    assert!(document.object_preview(id).is_none());
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
