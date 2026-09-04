//! Putting a form into the scene as a subtool of its own.
//!
//! `place_object` has always put a shape into the *active* layer, which is how
//! the parts of one form are built. What was missing is the other half: a form
//! a sculptor wants to work on its own, standing beside what is already there
//! rather than inside it. Both are wanted, the specification makes the choice
//! the sculptor's, and these hold that each does what it says — including that
//! an insertion is one thing in the history however many engine edits it took,
//! and that a copy is a copy rather than a second name for the same field.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, GestureSample, LayerKey, ModelError, ObjectModel,
    Representation, SceneModel, SculptModel, Shape, ToolKind, Unavailable,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Clear of the starting form, which is a unit sphere at the origin, so a ray
/// down one of them cannot reach the other.
const APART: f32 = 3.0;

fn adding() -> CombineSettings {
    CombineSettings {
        op: Combine::Add,
        ..CombineSettings::default()
    }
}

/// How many items a layer holds.
///
/// A field stroke deposits stamps as items, so this is what says *which* layer
/// a dab landed on — the surface alone cannot, since layers compose into one.
fn items(doc: &ClayDocument, key: LayerKey) -> usize {
    let id = doc.layer_id(key).expect("a layer");
    doc.document().layer_nodes(id).expect("its nodes").len()
}

/// One dab where a ray meets the surface.
fn dab(doc: &mut ClayDocument, at: [f32; 3]) -> Result<(), ModelError> {
    let samples = [GestureSample {
        position: at,
        pressure: 1.0,
        time: 0.0,
    }];
    doc.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size: 0.3,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &samples,
        [false; 3],
    )
    .map(|_| ())
}

/// Whether the surface encloses a point.
fn inside(doc: &ClayDocument, at: [f32; 3]) -> bool {
    doc.document()
        .eval_points(None, &[at])
        .is_ok_and(|values| values[0] < 0.0)
}

// -- a primitive as a subtool -------------------------------------------------

/// The scenario the specification states in as many words: "a new subtool holds
/// the sphere, it is the active subtool, and sculpting lands on it rather than
/// on the form that was active before".
#[test]
fn a_sphere_inserted_as_a_subtool_takes_the_next_dab() {
    let mut doc = document();
    let first = doc.scene().active.expect("a starting layer");
    let layers_before = doc.scene().layers.len();
    let first_items = items(&doc, first);

    let inserted = doc
        .insert_shape_subtool(Shape::Sphere, &[0.6], [APART, 0.0, 0.0], adding())
        .expect("a sphere as its own subtool");

    assert_eq!(
        doc.scene().layers.len(),
        layers_before + 1,
        "the insertion did not add a subtool"
    );
    assert_eq!(
        doc.scene().active,
        Some(inserted.layer),
        "an inserted subtool arrives selected, or the next dab lands elsewhere"
    );
    assert_ne!(inserted.layer, first);

    let at = doc
        .pick([APART, 0.0, -8.0], [0.0, 0.0, 1.0])
        .expect("the new subtool's surface");
    let new_before = items(&doc, inserted.layer);
    dab(&mut doc, at).expect("a dab on the subtool that just arrived");

    assert!(
        items(&doc, inserted.layer) > new_before,
        "the dab did not land on the inserted subtool"
    );
    assert_eq!(
        items(&doc, first),
        first_items,
        "the subtool that was active before the insertion was edited anyway"
    );
}

/// The layer moves and the form sits at its middle, which is what leaves the
/// whole-subtool manipulator on the form rather than in empty space.
#[test]
fn an_inserted_subtool_stands_where_it_was_asked_for() {
    let mut doc = document();
    let inserted = doc
        .insert_shape_subtool(Shape::Sphere, &[0.6], [APART, 0.0, 0.0], adding())
        .expect("a sphere as its own subtool");

    assert!(
        inside(&doc, [APART, 0.0, 0.0]),
        "the inserted form is not where it was asked for"
    );
    assert_eq!(
        doc.target_transform(clayspace_model::GizmoTarget::Layer(inserted.layer))
            .map(|at| at.position),
        Some([APART, 0.0, 0.0]),
        "the subtool's own middle is elsewhere, so a manipulator on it would \
         stand away from the form it addresses"
    );
}

/// Two engine edits — the layer, then the form in it — and one thing the
/// sculptor asked for. Without the group, one step back takes the form away
/// and leaves an empty subtool standing.
#[test]
fn inserting_a_subtool_is_one_undo_step() {
    let mut doc = document();
    let before = doc.scene().layers.len();
    let active_before = doc.scene().active;

    doc.insert_shape_subtool(
        Shape::Box,
        &Shape::Box.defaults(),
        [APART, 0.0, 0.0],
        adding(),
    )
    .expect("a box as its own subtool");
    assert_eq!(doc.scene().layers.len(), before + 1);

    assert!(doc.undo().expect("undo"), "there was nothing to undo");
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "one step back left the subtool the insertion made"
    );
    assert_eq!(
        doc.scene().active,
        active_before,
        "the layer that was active before the insertion did not come back"
    );
    assert!(
        !inside(&doc, [APART, 0.0, 0.0]),
        "the form the insertion placed is still standing"
    );
}

/// The names are the handle a voxel layer's grid is fetched by (ClayCore
/// #365), so two subtools sharing one shadow each other and a stroke lands on
/// the wrong grid. An insertion never asks the sculptor for a name, so it is
/// the insertion that has to avoid the collision.
#[test]
fn inserted_subtools_do_not_share_a_name() {
    let mut doc = document();
    for _ in 0..3 {
        doc.insert_shape_subtool(Shape::Sphere, &[0.4], [0.0; 3], adding())
            .expect("another sphere");
    }
    let names: Vec<String> = doc
        .scene()
        .layers
        .iter()
        .map(|layer| layer.name.clone())
        .collect();
    let unique: std::collections::BTreeSet<&String> = names.iter().collect();
    assert_eq!(
        unique.len(),
        names.len(),
        "two subtools share a name: {names:?}"
    );
    assert!(
        names.iter().any(|name| name == Shape::Sphere.label()),
        "the first one should carry the shape's own name: {names:?}"
    );
}

/// The specification's refusal scenario, both halves: placing *into* a layer
/// that has no ordered list is refused with a reason naming what an object
/// needs, "while inserting the same primitive as its own subtool remains
/// available".
#[test]
fn a_grid_refuses_a_placed_object_and_takes_a_subtool_anyway() {
    let mut doc = document();
    let grid = doc
        .add_layer("Grade", Representation::Voxel)
        .expect("a voxel layer");
    doc.set_active_layer(grid).expect("work on the grid");

    let refused = doc
        .place_object(Shape::Sphere, &[0.4], [0.0; 3], adding())
        .expect_err("a grid has no ordered list to put an item in");
    match refused {
        ModelError::Unavailable(Unavailable::NoVerbHere { active, verbs, .. }) => {
            assert_eq!(active, Representation::Voxel);
            assert!(
                verbs.on(Representation::Sdf).is_some(),
                "the refusal must name what an object needs"
            );
        }
        other => panic!("the refusal did not name the cause: {other}"),
    }

    doc.set_active_layer(grid).expect("still on the grid");
    let inserted = doc
        .insert_shape_subtool(Shape::Sphere, &[0.4], [APART, 0.0, 0.0], adding())
        .expect("inserting as a subtool stays available over a grid");
    assert_ne!(inserted.layer, grid);
    assert_eq!(doc.scene().active, Some(inserted.layer));
}

/// The same over a mesh, which has no ordered list either — and reaches the
/// refusal by a different route, since a mesh layer carries triangles rather
/// than a field.
#[test]
fn a_mesh_refuses_a_placed_object_and_takes_a_subtool_anyway() {
    let mut doc = document();
    let carried = doc.add_mesh_layer("Modelo").expect("a mesh layer");
    doc.set_active_layer(carried).expect("work on the mesh");

    assert!(
        doc.place_object(Shape::Sphere, &[0.4], [0.0; 3], adding())
            .is_err(),
        "a mesh took an item into an ordered list it does not have"
    );
    assert!(doc
        .insert_shape_subtool(Shape::Sphere, &[0.4], [APART, 0.0, 0.0], adding())
        .is_ok());
}

// -- an imported mesh ---------------------------------------------------------

/// Somewhere for one test's file, named after the caller: tests run in
/// parallel and a shared name has them deleting each other's.
fn scratch(who: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("clayspace-insertion-{who}.obj"));
    let _ = std::fs::remove_file(&path);
    path
}

/// The specification: an imported mesh "stands in the scene as its own subtool,
/// carries its geometry, and can be moved with the manipulator".
///
/// This holds the first two, and *selected on arrival*, which is what makes it
/// the subtool the sculptor goes on to work.
/// `a_mesh_subtool_moves_with_the_manipulator` holds the third — it used to be
/// asserted here as "the layer is a target the manipulator can address, which
/// it is once it exists", and that was true of a subtool the manipulator could
/// not actually move.
#[test]
fn an_imported_mesh_arrives_as_the_active_subtool() {
    use clayspace_model::{ExchangeModel, ImportAs, ImportSettings};

    let mut doc = document();
    let path = scratch("carried");
    doc.export_mesh(&path, clayspace_model::ExportSettings::default())
        .expect("something to import");
    let before = doc.scene().layers.len();

    doc.import_mesh(
        &path,
        ImportSettings {
            becomes: ImportAs::Reference,
            ..Default::default()
        },
    )
    .expect("import the mesh");

    let scene = doc.scene();
    assert_eq!(scene.layers.len(), before + 1);
    let arrived = scene.active_layer().expect("an active layer");
    assert_eq!(
        arrived.representation,
        Representation::Mesh,
        "the imported subtool is not the one that became active"
    );
    assert!(
        doc.target_transform(clayspace_model::GizmoTarget::Layer(arrived.key))
            .is_some(),
        "the imported subtool carries no transform, so no manipulator can sit \
         on it"
    );
    // "Carries its geometry" is triangles, not a row that says mesh: the
    // representation alone was true of a layer nothing had ever attached a
    // mesh to.
    let (positions, _, _, indices, spans) = doc.visible_mesh_geometry();
    assert!(
        !indices.is_empty() && !positions.is_empty(),
        "the imported subtool draws no triangles"
    );
    assert!(
        spans.iter().any(|span| span.layer == arrived.key),
        "no run of the drawn buffer belongs to the imported subtool"
    );
    assert!(
        doc.layer_bounds(arrived.key).is_some(),
        "a carried mesh has no SDF extent, so its own triangles are the only \
         account of where it is — without one the whole-subtool manipulator \
         sizes itself to a default"
    );
    let _ = std::fs::remove_file(&path);
}

/// And the third of the three: it "can be moved with the manipulator".
///
/// `GizmoTarget::Layer` resolves to `place_layer`, which writes the engine's
/// layer transform — and a mesh layer is *carried* rather than evaluated, so
/// the tape has nothing to move and the transform reached nothing at all.
/// Measured before the fix: a mesh subtool moved five units along X drew its
/// first vertex at exactly where it drew it before, and `layer_bounds` still
/// answered `None`. Dragging the whole-subtool manipulator on an imported mesh
/// moved nothing on screen.
#[test]
fn a_mesh_subtool_moves_with_the_manipulator() {
    use clayspace_model::{ExchangeModel, ImportAs, ImportSettings};

    let mut doc = document();
    let path = scratch("moved");
    doc.export_mesh(&path, clayspace_model::ExportSettings::default())
        .expect("something to import");
    doc.import_mesh(
        &path,
        ImportSettings {
            becomes: ImportAs::Reference,
            ..Default::default()
        },
    )
    .expect("import the mesh");
    let _ = std::fs::remove_file(&path);
    let key = doc.scene().active.expect("the imported subtool");

    let (before, ..) = doc.visible_mesh_geometry();
    let first = *before.first().expect("triangles");
    let (was_min, _) = doc.layer_bounds(key).expect("an extent to move");

    doc.set_layer_transform(key, [APART, 0.0, 0.0], 1.0)
        .expect("move the whole subtool");

    let (after, ..) = doc.visible_mesh_geometry();
    let moved = *after.first().expect("triangles");
    assert!(
        (moved[0] - first[0] - APART).abs() < 1e-3,
        "the drawn geometry did not follow the layer transform: {first:?} -> \
         {moved:?}"
    );
    let (now_min, _) = doc.layer_bounds(key).expect("an extent");
    assert!(
        (now_min[0] - was_min[0] - APART).abs() < 1e-3,
        "the subtool's own box did not follow it, so the manipulator sizes \
         itself to the wrong place"
    );
    // And what is picked follows what is drawn, or the sculptor would sculpt
    // where the form is not.
    let hit = doc
        .pick([APART, 0.0, 5.0], [0.0, 0.0, -1.0])
        .expect("a ray down the moved subtool meets it");
    assert!(
        (hit[0] - APART).abs() < 0.2,
        "the pick answered {hit:?}, which is not on the form as drawn"
    );
    assert!(
        doc.pick([0.0, 0.0, 5.0], [0.0, 0.0, -1.0]).is_none(),
        "a ray down the origin still finds the mesh, so picking stayed where \
         the vertices are rather than where the subtool stands"
    );
}

/// Importing the same file twice is a thing sculptors do, and two layers
/// sharing a name shadow one another's grid the moment either is crossed to
/// voxels (ClayCore #365).
#[test]
fn importing_the_same_mesh_twice_gives_two_names() {
    use clayspace_model::{ExchangeModel, ImportAs, ImportSettings};

    let mut doc = document();
    let path = scratch("twice");
    doc.export_mesh(&path, clayspace_model::ExportSettings::default())
        .expect("something to import");
    let settings = ImportSettings {
        becomes: ImportAs::Reference,
        ..Default::default()
    };
    doc.import_mesh(&path, settings).expect("first import");
    doc.import_mesh(&path, settings).expect("second import");

    let names: Vec<String> = doc
        .scene()
        .layers
        .iter()
        .map(|layer| layer.name.clone())
        .collect();
    let unique: std::collections::BTreeSet<&String> = names.iter().collect();
    assert_eq!(
        unique.len(),
        names.len(),
        "two imported subtools share a name: {names:?}"
    );
    let _ = std::fs::remove_file(&path);
}

// -- a copy -------------------------------------------------------------------

/// The specification: "a copy SHALL be independent: sculpting the copy SHALL
/// NOT change the original". An instance would share the field and fail this,
/// which is why the interface says "copy" (ClayCore #364).
#[test]
fn a_copied_subtool_is_independent_of_its_original() {
    let mut doc = document();
    let original = doc.scene().active.expect("a starting layer");
    let original_items = items(&doc, original);
    let original_bounds = doc.layer_bounds(original).expect("the original's extent");

    let copy = doc
        .copy_subtool(original, 0.02)
        .expect("a copy of the starting form");
    assert_ne!(copy.layer, original);
    assert_eq!(
        doc.scene().active,
        Some(copy.layer),
        "a copy arrives selected, as every insertion does"
    );

    // Stood clear of the original, so the two are not the same points in space
    // and a dab can be aimed at one of them.
    doc.set_layer_transform(copy.layer, [APART, 0.0, 0.0], 1.0)
        .expect("stand the copy clear");
    doc.set_active_layer(copy.layer).expect("work on the copy");
    let at = doc
        .pick([APART, 0.0, -8.0], [0.0, 0.0, 1.0])
        .expect("the copy's surface");
    dab(&mut doc, at).expect("a dab on the copy");

    assert_eq!(
        items(&doc, original),
        original_items,
        "sculpting the copy reached the original, so it is not a copy"
    );
    assert_eq!(
        doc.layer_bounds(original),
        Some(original_bounds),
        "the original's extent moved while the copy was sculpted"
    );
    assert!(
        doc.scene().layers.iter().any(|l| l.key == original)
            && doc.scene().layers.iter().any(|l| l.key == copy.layer),
        "both must be present in the scene"
    );
}

/// A copy carries the source's form, not an empty subtool with its name.
#[test]
fn a_copy_carries_what_it_copied() {
    let mut doc = document();
    let original = doc.scene().active.expect("a starting layer");
    let copy = doc.copy_subtool(original, 0.02).expect("a copy");

    doc.set_layer_visible(original, false)
        .expect("hide the original");
    assert!(
        inside(&doc, [0.0; 3]),
        "with the original hidden the copy is all that is left, and it is empty"
    );
    assert!(
        doc.layer_bounds(copy.layer).is_some(),
        "the copy reports no extent, so it holds nothing"
    );
}

/// The bake hides the rest of the scene to sample one subtool alone, and every
/// exit path puts back what the sculptor had. A copy is the shortest path to
/// checking that on the real operation rather than on a stand-in.
#[test]
fn a_copy_leaves_the_visibility_the_sculptor_set() {
    let mut doc = document();
    let first = doc.scene().active.expect("a starting layer");
    let second = doc
        .insert_shape_subtool(Shape::Sphere, &[0.5], [APART, 0.0, 0.0], adding())
        .expect("a second subtool")
        .layer;
    doc.set_layer_visible(second, false).expect("hide it");

    let was: Vec<(LayerKey, bool)> = doc
        .scene()
        .layers
        .iter()
        .map(|layer| (layer.key, layer.visible))
        .collect();
    doc.copy_subtool(first, 0.02).expect("a copy of the first");

    for (key, visible) in was {
        assert_eq!(
            doc.scene().layer(key).map(|layer| layer.visible),
            Some(visible),
            "the bake left a layer's visibility where it borrowed it"
        );
    }
}

/// One thing the sculptor asked for, however many engine edits the bake and
/// the layer took.
#[test]
fn copying_a_subtool_is_one_undo_step() {
    let mut doc = document();
    let original = doc.scene().active.expect("a starting layer");
    let before = doc.scene().layers.len();

    doc.copy_subtool(original, 0.02).expect("a copy");
    assert_eq!(doc.scene().layers.len(), before + 1);

    assert!(doc.undo().expect("undo"), "there was nothing to undo");
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "one step back left the copy standing"
    );
}

/// An empty subtool has no field to sample, so the copy would be an empty
/// subtool with a name. Refused by name rather than made.
#[test]
fn copying_an_empty_subtool_is_refused_with_a_reason() {
    let mut doc = document();
    let empty = doc
        .add_layer("Vazia", Representation::Sdf)
        .expect("an empty layer");
    let before = doc.scene().layers.len();

    let refused = doc
        .copy_subtool(empty, 0.02)
        .expect_err("an empty subtool has nothing to copy");
    assert!(
        !refused.to_string().is_empty(),
        "a refusal has to be sayable"
    );
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "the refused copy left a layer behind"
    );
}

/// What the copy control is allowed to offer. A control that can only refuse is
/// worse than one that is not there.
#[test]
fn only_subtools_with_something_in_them_are_offered_for_copying() {
    let mut doc = document();
    let first = doc.scene().active.expect("a starting layer");
    let empty = doc
        .add_layer("Vazia", Representation::Sdf)
        .expect("an empty layer");
    let carried = doc.add_mesh_layer("Modelo").expect("a mesh layer");

    let offered: Vec<LayerKey> = doc
        .copyable_subtools()
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    assert!(
        offered.contains(&first),
        "the form with clay in it is not offered"
    );
    assert!(
        !offered.contains(&empty),
        "an empty subtool would copy to an empty subtool"
    );
    assert!(
        !offered.contains(&carried),
        "a mesh layer is carried rather than evaluated, so there is no field \
         to sample"
    );
}

/// The third source, held to the same promise as the other two: "a subtool is
/// inserted from any of the three sources and the sculptor undoes once — the
/// subtool is gone and nothing else has changed".
#[test]
fn undoing_an_imported_mesh_subtool_takes_it_away() {
    use clayspace_model::{ExchangeModel, ImportAs, ImportSettings};

    let mut doc = document();
    let path = scratch("undone");
    doc.export_mesh(&path, clayspace_model::ExportSettings::default())
        .expect("something to import");
    let before = doc.scene().layers.len();

    doc.import_mesh(
        &path,
        ImportSettings {
            becomes: ImportAs::Reference,
            ..Default::default()
        },
    )
    .expect("import the mesh");
    assert_eq!(doc.scene().layers.len(), before + 1);

    doc.undo().expect("undo");
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "one step back left the imported subtool standing"
    );
    let _ = std::fs::remove_file(&path);
}

/// The bake reads the *evaluated* field, which already has the source layer's
/// own transform in it. So a copy of a subtool that has been moved stands where
/// the source stands rather than back at the origin — and the copy's own layer
/// starts at identity, which is what makes the manipulator's next drag measure
/// from where the form actually is.
#[test]
fn a_copy_of_a_moved_subtool_lands_on_it() {
    let mut doc = document();
    let original = doc.scene().active.expect("a starting layer");
    doc.set_layer_transform(original, [APART, 0.0, 0.0], 1.0)
        .expect("move the original");
    assert!(inside(&doc, [APART, 0.0, 0.0]));

    let copy = doc.copy_subtool(original, 0.02).expect("a copy");
    doc.set_layer_visible(original, false)
        .expect("hide the original");

    assert!(
        inside(&doc, [APART, 0.0, 0.0]),
        "the copy did not land on the form it copied"
    );
    assert!(
        !inside(&doc, [0.0; 3]),
        "the copy landed at the origin, so the source's own transform was \
         sampled twice or not at all"
    );
    assert!(doc.layer_bounds(copy.layer).is_some());
}
