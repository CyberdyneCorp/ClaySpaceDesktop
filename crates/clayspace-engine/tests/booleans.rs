//! Booleans between subtools: what comes out, what it costs, and what is
//! refused.
//!
//! The engine composes the layers of a document by hard union, so there is no
//! live boolean between two of them (ClayCore #321). What these hold is the
//! resolved one this application can build today: each operand sampled alone,
//! the two volumes combined in a subtool of their own, and the operands kept
//! so the whole thing can be reconsidered.
//!
//! One test per scenario of `specs/subtool-booleans/spec.md`, plus the two the
//! risks section asks for — a boolean over a *sculpted* operand rather than
//! over two pristine primitives, and every representation as an operand with
//! no crossing demanded of the sculptor beforehand.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BooleanOp, BooleanRefusal, BooleanSettings, BrushSettings, Combine, CombineSettings,
    GestureSample, LayerKey, ModelError, ObjectModel, Protection, Representation, SceneModel,
    SculptModel, Shape, ToolKind,
};

/// A document with one empty field layer and nothing in it.
///
/// No starting form on purpose: every one of these asks what the *result* of a
/// boolean encloses, and a sphere at the origin nobody put there would answer
/// half of those questions by itself.
fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy).expect("a document")
}

fn adding() -> CombineSettings {
    CombineSettings {
        op: Combine::Add,
        ..CombineSettings::default()
    }
}

/// The resolution the tests bake at. Fine enough that a 0.25 bore survives it,
/// coarse enough that a suite of these runs in seconds.
const CELL: f32 = 0.02;

/// A sphere at the origin, wide enough to be bored through.
fn a_sphere(doc: &mut ClayDocument) -> LayerKey {
    doc.insert_shape_subtool(Shape::Sphere, &[0.6], [0.0; 3], adding())
        .expect("a sphere subtool")
        .layer
}

/// A cylinder standing on the sphere's axis, long enough to pass through it.
fn a_cylinder(doc: &mut ClayDocument, at: [f32; 3]) -> LayerKey {
    doc.insert_shape_subtool(Shape::Cylinder, &[0.25, 1.0], at, adding())
        .expect("a cylinder subtool")
        .layer
}

fn settings(base: LayerKey, tool: LayerKey, op: BooleanOp) -> BooleanSettings {
    BooleanSettings {
        base: Some(base),
        tool: Some(tool),
        op,
        cell_size: CELL,
        consume: false,
    }
}

/// Whether the surface the document evaluates encloses a point.
///
/// The whole document's field, which is what makes it the right question: a
/// hidden operand contributes nothing to it, so this reads the result alone
/// once a boolean has hidden what made it.
fn inside(doc: &ClayDocument, at: [f32; 3]) -> bool {
    doc.document()
        .eval_points(None, &[at])
        .is_ok_and(|values| values[0] < 0.0)
}

/// How many items a layer holds, which is what says a dab landed on it.
fn items(doc: &ClayDocument, key: LayerKey) -> usize {
    let id = doc.layer_id(key).expect("a layer");
    doc.document().layer_nodes(id).expect("its nodes").len()
}

/// One dab where a ray meets the surface.
fn dab(doc: &mut ClayDocument, at: [f32; 3]) {
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
    .expect("a dab");
}

fn is_visible(doc: &ClayDocument, key: LayerKey) -> Option<bool> {
    doc.scene().layer(key).map(|layer| layer.visible)
}

// -- what comes out -----------------------------------------------------------

/// The specification's opening scenario, in as many words: "a new subtool holds
/// the sphere with a cylindrical bore through it, and it is the active
/// subtool".
#[test]
fn a_cylinder_bores_a_sphere() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    let layers_before = doc.scene().layers.len();

    let result = doc
        .run_boolean(settings(sphere, cylinder, BooleanOp::Subtract))
        .expect("the cylinder cuts the sphere");

    assert_eq!(
        doc.scene().layers.len(),
        layers_before + 1,
        "the boolean did not leave a subtool of its own"
    );
    assert_eq!(
        doc.scene().active,
        Some(result.layer),
        "the result must be the active subtool, or the next dab lands elsewhere"
    );
    assert!(
        !inside(&doc, [0.0; 3]),
        "the middle of the sphere is still solid, so nothing was bored"
    );
    assert!(
        inside(&doc, [0.45, 0.0, 0.0]),
        "the sphere's wall is gone too, so the subtraction took everything"
    );
}

/// Union and intersection over the same pair, so that each operation is shown
/// to be the operation it says it is rather than all three being subtraction.
#[test]
fn each_operation_leaves_a_different_form() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    // Offset along X so the two overlap in part rather than one containing the
    // other, which is what makes an intersection distinguishable from either.
    let cylinder = a_cylinder(&mut doc, [0.5, 0.0, 0.0]);

    let united = doc
        .run_boolean(settings(sphere, cylinder, BooleanOp::Union))
        .expect("a union");
    assert!(
        inside(&doc, [0.5, 0.8, 0.0]),
        "the cylinder's far end is not in the union"
    );
    assert!(
        inside(&doc, [-0.4, 0.0, 0.0]),
        "the sphere's far side is not in the union"
    );

    doc.set_layer_visible(united.layer, false).expect("hide it");
    doc.set_layer_visible(sphere, true)
        .expect("show the sphere");
    doc.set_layer_visible(cylinder, true)
        .expect("show the cylinder");
    doc.run_boolean(settings(sphere, cylinder, BooleanOp::Intersect))
        .expect("an intersection");

    assert!(
        !inside(&doc, [-0.4, 0.0, 0.0]),
        "the sphere's far side survived an intersection with a cylinder that \
         does not reach it"
    );
    assert!(
        inside(&doc, [0.5, 0.0, 0.0]),
        "where both forms stand is not in the intersection"
    );
}

/// Subtraction is not symmetric, and the interface names which is which
/// because of it. This is the fact that makes the naming worth doing.
#[test]
fn swapping_the_operands_changes_a_subtraction() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.5, 0.0, 0.0]);

    let cut = doc
        .run_boolean(settings(sphere, cylinder, BooleanOp::Subtract))
        .expect("sphere minus cylinder");
    // The sphere's far side is untouched by a cylinder standing off to one
    // side, and the cylinder's own body is not there at all.
    assert!(inside(&doc, [-0.4, 0.0, 0.0]));
    assert!(!inside(&doc, [0.5, 0.8, 0.0]));

    doc.set_layer_visible(cut.layer, false).expect("hide it");
    doc.set_layer_visible(sphere, true)
        .expect("show the sphere");
    doc.set_layer_visible(cylinder, true)
        .expect("show the cylinder");
    doc.run_boolean(settings(cylinder, sphere, BooleanOp::Subtract))
        .expect("cylinder minus sphere");

    assert!(
        !inside(&doc, [-0.4, 0.0, 0.0]),
        "the sphere is in the result of cylinder-minus-sphere"
    );
    assert!(
        inside(&doc, [0.5, 0.8, 0.0]),
        "the cylinder's far end is missing from cylinder-minus-sphere"
    );
}

/// "The result of a boolean is sculpted, moved and used as the operand of a
/// second boolean, and each of those works exactly as it does on a subtool
/// that was never a result."
#[test]
fn the_result_is_an_ordinary_subtool() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    let result = doc
        .run_boolean(settings(sphere, cylinder, BooleanOp::Subtract))
        .expect("a bored sphere")
        .layer;

    let before = items(&doc, result);
    let at = doc
        .pick([0.0, 0.0, -8.0], [0.0, 0.0, 1.0])
        .expect("the result's surface");
    dab(&mut doc, at);
    assert!(
        items(&doc, result) > before,
        "a dab did not land on the result, so it is not sculptable"
    );

    doc.set_layer_transform(result, [2.0, 0.0, 0.0], 1.0)
        .expect("move the result");
    let (min, max) = doc.layer_bounds(result).expect("the result has an extent");
    assert!(
        min[0] > 1.0 && max[0] > 1.0,
        "the result did not move with its layer transform: {min:?}..{max:?}"
    );

    let box_subtool = doc
        .insert_shape_subtool(Shape::Box, &[0.3, 0.3, 0.3], [2.0, 0.0, 0.0], adding())
        .expect("a box beside the result")
        .layer;
    let second = doc
        .run_boolean(settings(result, box_subtool, BooleanOp::Union))
        .expect("the result serves as an operand again");
    assert_eq!(doc.scene().active, Some(second.layer));
}

// -- what happens to the operands --------------------------------------------

/// "Both operands are still in the scene, hidden, and the result is what the
/// viewport shows."
#[test]
fn the_operands_are_kept_and_hidden() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);

    doc.run_boolean(settings(sphere, cylinder, BooleanOp::Subtract))
        .expect("a boolean");

    assert_eq!(
        is_visible(&doc, sphere),
        Some(false),
        "the base operand is still shown, so the scene holds it twice"
    );
    assert_eq!(is_visible(&doc, cylinder), Some(false));
    assert!(
        doc.scene().layer(sphere).is_some() && doc.scene().layer(cylinder).is_some(),
        "the operands were removed although nobody asked for that"
    );
}

/// "The result subtool is gone and both operands are visible again, exactly as
/// they were." One step back, however many engine edits the two bakes and the
/// layer took.
#[test]
fn one_undo_takes_back_the_whole_boolean() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    let before = doc.scene().layers.len();

    let result = doc
        .run_boolean(settings(sphere, cylinder, BooleanOp::Subtract))
        .expect("a boolean")
        .layer;

    assert!(doc.undo().expect("undo"), "there was nothing to undo");
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "one step back left the result standing"
    );
    assert!(
        doc.scene().layer(result).is_none(),
        "the result subtool survived the undo"
    );
    assert_eq!(
        is_visible(&doc, sphere),
        Some(true),
        "the base operand is still hidden after the boolean was taken back"
    );
    assert_eq!(is_visible(&doc, cylinder), Some(true));
    assert!(
        inside(&doc, [0.0; 3]),
        "the operands are back in the list and not back in the field"
    );
}

/// "The operands are removed and the interface has stated that this is what
/// will happen before it runs." The stating is the panel's; this is the half
/// the document owes — that it happens only when it was asked for.
#[test]
fn the_operands_are_consumed_only_on_request() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);

    let result = doc
        .run_boolean(BooleanSettings {
            consume: true,
            ..settings(sphere, cylinder, BooleanOp::Subtract)
        })
        .expect("a boolean that consumes its operands");

    let scene = doc.scene();
    assert!(
        scene.layer(sphere).is_none() && scene.layer(cylinder).is_none(),
        "the operands were kept although the sculptor asked to consume them"
    );
    assert!(
        scene.layer(result.layer).is_some(),
        "the result went with them"
    );
    assert_eq!(
        scene.active,
        Some(result.layer),
        "removing the operands moved the active subtool off the result"
    );
    assert!(
        !inside(&doc, [0.0; 3]),
        "the bore is gone, so what was kept is not the result"
    );
}

// -- what it costs ------------------------------------------------------------

/// "The estimated cost is shown and nothing is changed until it is confirmed."
#[test]
fn the_cost_is_stated_and_nothing_runs_until_it_is_confirmed() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    let before = doc.scene().layers.len();

    let cost = doc
        .boolean_cost(settings(sphere, cylinder, BooleanOp::Subtract))
        .expect("a pair has a price");

    assert!(cost.cells > 0, "a sampled result costs cells");
    assert!(
        (cost.surface_movement - CELL * 0.5).abs() < 1e-6,
        "the surface moves by half a cell, as every other crossing states"
    );
    assert!(
        !cost.keeps_history,
        "a sampled result cannot carry the operands' edit lists"
    );
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "asking what a boolean would cost ran one"
    );
    assert_eq!(is_visible(&doc, sphere), Some(true));
}

/// "The stated cost updates and the result is sampled at the chosen
/// resolution."
#[test]
fn a_finer_resolution_costs_more_and_states_it() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);

    let coarse = doc
        .boolean_cost(BooleanSettings {
            cell_size: 0.05,
            ..settings(sphere, cylinder, BooleanOp::Subtract)
        })
        .expect("a price");
    let fine = doc
        .boolean_cost(BooleanSettings {
            cell_size: 0.01,
            ..settings(sphere, cylinder, BooleanOp::Subtract)
        })
        .expect("a price");

    assert!(fine.cells > coarse.cells, "a finer cell costs more storage");
    assert!(fine.surface_movement < coarse.surface_movement);
    assert!(fine.vanishing_feature < coarse.vanishing_feature);
}

/// The default resolution follows the operands' own detail rather than a fixed
/// constant: a grid says what it is worked at, and the finer of the two is
/// what decides.
#[test]
fn the_default_resolution_follows_the_operands_detail() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    let fields = doc
        .boolean_cell(sphere, cylinder)
        .expect("two field subtools have a working cell");

    doc.add_voxel_layer("Grade", 0.004).expect("a fine grid");
    let grid = doc
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Voxel)
        .map(|layer| layer.key)
        .expect("the grid is in the stack");

    let mixed = doc
        .boolean_cell(sphere, grid)
        .expect("a grid says what it is worked at");
    assert!(
        mixed < fields,
        "a 0.004 grid did not pull the default finer than the working cell: \
         {mixed} against {fields}"
    );
}

/// A pair whose region cannot be sampled inside the document's budget is
/// refused with the budget named, and leaves the scene as it was.
#[test]
fn a_pair_over_the_budget_is_refused() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    // Far enough apart that the region covering both is enormous, which is
    // what a fine cell over a wide union actually costs.
    let cylinder = a_cylinder(&mut doc, [9.0, 0.0, 0.0]);
    let before = doc.scene().layers.len();

    let refused = doc
        .run_boolean(BooleanSettings {
            cell_size: 0.002,
            ..settings(sphere, cylinder, BooleanOp::Union)
        })
        .expect_err("a billion cells does not fit the budget");

    assert!(
        matches!(
            refused,
            ModelError::Boolean(BooleanRefusal::OverBudget { .. })
        ),
        "the refusal did not name the budget: {refused}"
    );
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "the refused boolean left a subtool behind"
    );
    assert_eq!(is_visible(&doc, sphere), Some(true));
}

// -- what is refused ----------------------------------------------------------

/// "The operation is refused with that as the stated reason, and no empty
/// subtool is created."
#[test]
fn intersecting_two_forms_that_do_not_touch_is_refused_by_name() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [4.0, 0.0, 0.0]);
    let before = doc.scene().layers.len();

    let refused = doc
        .run_boolean(settings(sphere, cylinder, BooleanOp::Intersect))
        .expect_err("two forms standing apart intersect in nothing");

    match &refused {
        ModelError::Boolean(BooleanRefusal::NoOverlap { base, tool }) => {
            assert!(
                !base.is_empty() && !tool.is_empty(),
                "the refusal named neither operand"
            );
        }
        other => panic!("an intersection of two forms apart was refused as {other}"),
    }
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "an empty subtool was created for an intersection of nothing"
    );

    // The same pair unites perfectly well, which is what makes the refusal
    // about the operation rather than about the pair.
    assert!(doc
        .run_boolean(settings(sphere, cylinder, BooleanOp::Union))
        .is_ok());
}

/// "The refusal names that subtool and why it cannot take part."
#[test]
fn a_ghosted_operand_is_refused_by_name() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    let name = doc
        .scene()
        .layer(cylinder)
        .map(|layer| layer.name.clone())
        .expect("the cylinder has a name");
    doc.set_layer_protection(
        cylinder,
        Protection {
            ghost: true,
            locked: false,
        },
    )
    .expect("ghost the cylinder");
    let before = doc.scene().layers.len();

    let refused = doc
        .run_boolean(settings(sphere, cylinder, BooleanOp::Subtract))
        .expect_err("a ghosted subtool cannot take part");

    assert!(
        refused.to_string().contains(&name),
        "the refusal does not name the ghosted subtool: {refused}"
    );
    assert!(
        matches!(
            refused,
            ModelError::Boolean(BooleanRefusal::Protected { ghost: true, .. })
        ),
        "the refusal does not say what is wrong with it: {refused}"
    );
    assert_eq!(doc.scene().layers.len(), before);
}

#[test]
fn a_locked_operand_is_refused_by_name() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    doc.set_layer_protection(
        sphere,
        Protection {
            ghost: false,
            locked: true,
        },
    )
    .expect("lock the sphere");

    let refused = doc
        .run_boolean(settings(sphere, cylinder, BooleanOp::Subtract))
        .expect_err("a locked subtool cannot take part");
    assert!(matches!(
        refused,
        ModelError::Boolean(BooleanRefusal::Protected { ghost: false, .. })
    ));
}

/// An empty operand has no field to sample, so the result would be a subtool
/// with a name and nothing in it.
#[test]
fn an_empty_operand_is_refused_by_name() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let empty = doc
        .add_layer("Vazia", Representation::Sdf)
        .expect("an empty subtool");
    let before = doc.scene().layers.len();

    let refused = doc
        .run_boolean(settings(sphere, empty, BooleanOp::Union))
        .expect_err("an empty subtool has nothing to combine");

    match &refused {
        ModelError::Boolean(BooleanRefusal::Empty { operand }) => {
            assert_eq!(operand, "Vazia", "the refusal named the wrong subtool");
        }
        other => panic!("an empty operand was refused as {other}"),
    }
    assert_eq!(doc.scene().layers.len(), before);
}

/// A boolean needs two different subtools, and half a panel is not a pair.
#[test]
fn one_subtool_is_not_a_pair() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);

    assert!(matches!(
        doc.run_boolean(settings(sphere, sphere, BooleanOp::Union))
            .expect_err("a subtool cannot be booleaned with itself"),
        ModelError::Boolean(BooleanRefusal::NotAPair)
    ));
    assert!(matches!(
        doc.run_boolean(BooleanSettings {
            base: Some(sphere),
            tool: None,
            ..BooleanSettings::default()
        })
        .expect_err("one operand is not a pair"),
        ModelError::Boolean(BooleanRefusal::NotAPair)
    ));
}

/// What the panel is allowed to offer. An empty subtool would only ever be
/// refused; a protected one is offered so that choosing it produces the named
/// refusal the specification asks for rather than the sculptor wondering where
/// their cylinder went.
#[test]
fn the_offered_operands_are_the_ones_with_something_in_them() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    let empty = doc
        .add_layer("Vazia", Representation::Sdf)
        .expect("an empty subtool");
    doc.set_layer_protection(
        cylinder,
        Protection {
            ghost: true,
            locked: false,
        },
    )
    .expect("ghost the cylinder");

    let offered: Vec<LayerKey> = doc
        .boolean_operands()
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    assert!(offered.contains(&sphere));
    assert!(
        offered.contains(&cylinder),
        "a ghosted subtool is not offered, so its refusal can never be read"
    );
    assert!(
        !offered.contains(&empty),
        "an empty subtool is offered although it can only be refused"
    );
}

// -- every representation as an operand --------------------------------------

/// "A voxel subtool and an SDF subtool are unioned: the result holds both
/// forms." No crossing asked of the sculptor first.
#[test]
fn a_voxel_subtool_unions_with_a_field_subtool() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    // A second form standing clear of the first, rasterized where it stands —
    // so the union is visibly two forms rather than one counted twice.
    doc.insert_shape_subtool(Shape::Sphere, &[0.5], [1.4, 0.0, 0.0], adding())
        .expect("a second sphere");
    let grid = doc
        .convert_layer(clayspace_model::Direction::SdfToVoxel, 0.03, 1)
        .expect("rasterize the second sphere into a grid");

    assert!(
        doc.boolean_cost(settings(sphere, grid, BooleanOp::Union))
            .is_some(),
        "a pair with a grid in it has no stated cost"
    );
    let result = doc
        .run_boolean(settings(sphere, grid, BooleanOp::Union))
        .expect("a grid unions with a field");

    assert_eq!(doc.scene().active, Some(result.layer));
    assert!(
        inside(&doc, [0.0; 3]),
        "the field operand is missing from the union"
    );
    assert!(
        inside(&doc, [1.4, 0.0, 0.0]),
        "the grid operand is missing from the union"
    );
}

/// "An imported mesh subtool is subtracted from with a box subtool: the result
/// is a subtool holding the cut mesh's form, and the sculptor was not asked to
/// convert anything first."
#[test]
fn a_mesh_subtool_is_cut_by_a_primitive() {
    use clayspace_model::{ExchangeModel, ExportSettings, ImportAs, ImportSettings};

    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let path = std::env::temp_dir().join("clayspace-boolean-mesh.obj");
    let _ = std::fs::remove_file(&path);
    doc.export_mesh(&path, ExportSettings::default())
        .expect("something to import");
    doc.import_mesh(
        &path,
        ImportSettings {
            becomes: ImportAs::Reference,
            ..Default::default()
        },
    )
    .expect("import it back as a subtool");
    let mesh = doc
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the imported mesh is a subtool");
    // The sphere it was exported from goes away, so what the result encloses
    // can only have come from the mesh.
    doc.set_layer_visible(sphere, false)
        .expect("hide the model");

    let cutter = doc
        .insert_shape_subtool(Shape::Box, &[0.3, 0.3, 0.6], [0.5, 0.0, 0.0], adding())
        .expect("a box to cut with")
        .layer;

    let result = doc
        .run_boolean(settings(mesh, cutter, BooleanOp::Subtract))
        .expect("a mesh subtool is an operand without a conversion first");

    assert_eq!(doc.scene().active, Some(result.layer));
    assert!(
        inside(&doc, [-0.4, 0.0, 0.0]),
        "the mesh's far side is missing, so nothing of it was sampled"
    );
    assert!(
        !inside(&doc, [0.5, 0.0, 0.0]),
        "the box did not cut the mesh"
    );
    let _ = std::fs::remove_file(&path);
}

// -- a sculpted operand -------------------------------------------------------

/// The risk the design names: "bake resolution loses detail on a heavily
/// sculpted operand", answered by running the boolean over a form that was
/// sculpted rather than over two pristine primitives.
#[test]
fn a_sculpted_operand_keeps_what_it_was_sculpted_into() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    // A dab on the side, well away from where the cutter will stand.
    let at = doc
        .pick([-8.0, 0.0, 0.0], [1.0, 0.0, 0.0])
        .expect("the sphere's surface");
    dab(&mut doc, at);
    let sculpted = doc
        .layer_bounds(sphere)
        .expect("the sculpted sphere has an extent");
    assert!(
        sculpted.0[0] < -0.6,
        "the dab did not move the surface, so there is nothing to preserve"
    );

    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    let result = doc
        .run_boolean(settings(sphere, cylinder, BooleanOp::Subtract))
        .expect("a boolean over a sculpted operand");

    let baked = doc.layer_bounds(result.layer).expect("the result's extent");
    assert!(
        baked.0[0] < -0.6,
        "the sculpted side did not survive the bake: {baked:?}"
    );
    assert!(
        !inside(&doc, [0.0; 3]),
        "the bore is missing from the result"
    );
}

// -- what the bake borrows ----------------------------------------------------

/// The bake hides the whole scene to sample one subtool alone. Every exit path
/// puts back what the sculptor set — and the operands' own hiding, which the
/// operation *does* mean, is what is left.
#[test]
fn the_boolean_leaves_the_rest_of_the_scene_as_it_was() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    let bystander = doc
        .insert_shape_subtool(Shape::Box, &[0.2, 0.2, 0.2], [3.0, 0.0, 0.0], adding())
        .expect("a third subtool")
        .layer;
    let hidden = doc
        .insert_shape_subtool(Shape::Box, &[0.2, 0.2, 0.2], [-3.0, 0.0, 0.0], adding())
        .expect("a fourth subtool")
        .layer;
    doc.set_layer_visible(hidden, false)
        .expect("hide the fourth");

    doc.run_boolean(settings(sphere, cylinder, BooleanOp::Subtract))
        .expect("a boolean");

    assert_eq!(
        is_visible(&doc, bystander),
        Some(true),
        "a subtool the boolean never named came back hidden"
    );
    assert_eq!(
        is_visible(&doc, hidden),
        Some(false),
        "a subtool the sculptor had hidden came back shown"
    );
}

/// A refused boolean leaves the visibility exactly where it found it, which is
/// the promise `with_only_visible` exists to keep.
#[test]
fn a_refused_boolean_changes_nothing() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [4.0, 0.0, 0.0]);
    let was: Vec<(LayerKey, bool)> = doc
        .scene()
        .layers
        .iter()
        .map(|layer| (layer.key, layer.visible))
        .collect();

    doc.run_boolean(settings(sphere, cylinder, BooleanOp::Intersect))
        .expect_err("two forms apart do not intersect");

    for (key, visible) in was {
        assert_eq!(
            is_visible(&doc, key),
            Some(visible),
            "a refused boolean left a subtool's visibility where it borrowed it"
        );
    }
}

/// "Whichever is chosen, the whole operation SHALL be a single undo step" —
/// including the choice that removes the operands, which is three engine edits
/// on top of the two the result took.
#[test]
fn one_undo_takes_back_a_consuming_boolean_too() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.0; 3]);
    let before = doc.scene().layers.len();

    doc.run_boolean(BooleanSettings {
        consume: true,
        ..settings(sphere, cylinder, BooleanOp::Subtract)
    })
    .expect("a boolean that consumes its operands");
    assert_eq!(
        doc.scene().layers.len(),
        before - 1,
        "two operands went and one result arrived"
    );

    assert!(doc.undo().expect("undo"), "there was nothing to undo");
    assert_eq!(
        doc.scene().layers.len(),
        before,
        "one step back did not put the consumed operands back beside the \
         result it took away"
    );
    assert!(
        inside(&doc, [0.0; 3]),
        "the operands are in the list again and not back in the field"
    );
}

/// An intersection is only meaningful if the cutting operand says "outside"
/// everywhere it is not — including beyond the lattice it was sampled on. A
/// field operand is baked over the pair's whole region for exactly that
/// reason; a grid is read from its own cells and cannot be, so this is where
/// the two are held to the same promise.
#[test]
fn an_intersection_with_a_grid_keeps_only_what_both_hold() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    doc.insert_shape_subtool(Shape::Sphere, &[0.5], [0.5, 0.0, 0.0], adding())
        .expect("a second sphere");
    let grid = doc
        .convert_layer(clayspace_model::Direction::SdfToVoxel, 0.02, 1)
        .expect("rasterize the second sphere");

    doc.run_boolean(settings(sphere, grid, BooleanOp::Intersect))
        .expect("an intersection with a grid");

    assert!(
        inside(&doc, [0.4, 0.0, 0.0]),
        "where both forms stand is missing from the intersection"
    );
    assert!(
        !inside(&doc, [-0.4, 0.0, 0.0]),
        "the base survived where the grid operand does not reach, so the \
         intersection was read as 'intersect here, leave alone there'"
    );
}

/// Undoing a consuming boolean gives the operands back *as they were*.
///
/// `retire_operands` removes each operand inside the boolean's undo group, and
/// the engine restores them on the way back — but `reconcile_layers` could not
/// match a restored layer to anything, so it rebuilt each one through
/// `..Layer::new(..)` with a freshly minted `LayerKey` and a default of
/// everything the host keeps. Measured before the fix: the two operands came
/// back as new keys, the whole-subtool manipulator read `position: [0, 0, 0]`
/// while the engine still held the real transform — so the next drag wrote an
/// absolute transform derived from identity and the form teleported — the
/// painted mask and the symmetry axes were gone, and `objects()` on the
/// restored subtool answered with no rows at all, because the `PlacedObject`
/// table is resynced by depth and was still filed under the key that had gone.
///
/// The branch's own `one_undo_takes_back_a_consuming_boolean_too` asserted
/// only that the count was back, which is why none of that was caught.
#[test]
fn one_undo_gives_the_consumed_operands_back_as_they_were() {
    let mut doc = document();
    let sphere = a_sphere(&mut doc);
    let cylinder = a_cylinder(&mut doc, [0.9, 0.0, 0.0]);

    doc.set_active_layer(sphere).expect("work the sphere");
    doc.set_symmetry([false, true, false]).expect("mirror it");
    let stood = doc
        .target_transform(clayspace_model::GizmoTarget::Layer(cylinder))
        .expect("where the cylinder stands");
    let rows = {
        doc.set_active_layer(cylinder).expect("the cylinder");
        doc.objects().len()
    };
    assert!(rows > 0, "an inserted primitive leaves a row to lose");
    doc.set_active_layer(sphere).expect("back to the sphere");

    doc.run_boolean(BooleanSettings {
        consume: true,
        ..settings(sphere, cylinder, BooleanOp::Union)
    })
    .expect("a boolean that consumes its operands");
    assert!(doc.undo().expect("undo"), "there was nothing to undo");

    let keys: Vec<LayerKey> = doc.scene().layers.iter().map(|layer| layer.key).collect();
    assert!(
        keys.contains(&sphere) && keys.contains(&cylinder),
        "the operands came back under different keys, so every panel holding \
         one — and the object table, and the layer states — is pointing at \
         nothing: {keys:?}"
    );
    assert_eq!(
        doc.target_transform(clayspace_model::GizmoTarget::Layer(cylinder)),
        Some(stood),
        "the host forgot where the restored subtool stands while the engine \
         still holds it, so the manipulator draws at the origin"
    );
    doc.set_active_layer(sphere).expect("the sphere again");
    assert_eq!(
        doc.symmetry(),
        [false, true, false],
        "the restored subtool came back with a fresh layer's mirror"
    );
    doc.set_active_layer(cylinder).expect("the cylinder again");
    assert_eq!(
        doc.objects().len(),
        rows,
        "the restored subtool has no object rows, so the shape controls have \
         nothing to measure"
    );
}

// -- what the cache is left holding ------------------------------------------

/// How many bricks the cache still has waiting to be filled.
///
/// Zero after any operation that has settled. A number above it means a caller
/// marked the cache dirty and returned without draining, which does not show as
/// an error anywhere — the viewport simply draws the surface as it was until
/// something unrelated happens to drain.
fn waiting(doc: &ClayDocument) -> u64 {
    doc.cache()
        .stats()
        .expect("the cache reports its own state")
        .dirty_bricks
}

/// A boolean settles the cache before it returns.
///
/// It used to settle it three times: `retire_operands` hid each operand through
/// `set_layer_visible`, which ends in a whole-layer refill, and then
/// `settle_subtool` refilled the result over a region that is the *union* of
/// the two operand boxes — so bricks the hides had just filled were dirtied and
/// filled again. Measured on the reference scene, the two hides were 5.5 s of a
/// 12 s operation.
///
/// The hides now mark and leave the draining to the settle. That is only sound
/// while the settle actually happens, which is what this holds: if the deferred
/// mark ever stops being drained, the cache disagrees with the document and
/// nothing says so.
#[test]
fn a_boolean_leaves_no_brick_waiting() {
    let mut doc = document();
    let base = a_sphere(&mut doc);
    let tool = a_cylinder(&mut doc, [0.0; 3]);

    doc.run_boolean(settings(base, tool, BooleanOp::Subtract))
        .expect("a bore through the sphere");

    assert_eq!(
        waiting(&doc),
        0,
        "the boolean returned with bricks still marked; the deferred hides were \
         never drained"
    );
}

/// And so does one that consumes its operands, which takes the other branch.
#[test]
fn a_consuming_boolean_leaves_no_brick_waiting() {
    let mut doc = document();
    let base = a_sphere(&mut doc);
    let tool = a_cylinder(&mut doc, [0.0; 3]);
    let mut consuming = settings(base, tool, BooleanOp::Subtract);
    consuming.consume = true;

    doc.run_boolean(consuming)
        .expect("a bore through the sphere");

    assert_eq!(waiting(&doc), 0, "the consuming branch left bricks marked");
}

/// A borrowed visibility pattern settles the cache on both of its exits.
///
/// `with_only_visible` writes every layer's flag, runs a body, and puts the
/// flags back — two batches, and the batch is where the drain now lives. A body
/// that fails takes the early exit, and the flags that did land have marked
/// their layers either way.
#[test]
fn a_failed_body_leaves_no_brick_waiting() {
    let mut doc = document();
    let shown = a_sphere(&mut doc);
    a_cylinder(&mut doc, [0.0; 3]);

    let refused: Result<(), ModelError> = doc.with_only_visible(&[shown], |_| {
        Err(ModelError::Boolean(BooleanRefusal::NotAPair))
    });
    assert!(refused.is_err(), "the body was supposed to refuse");

    assert_eq!(
        waiting(&doc),
        0,
        "the restore ran but left the cache marked, so the viewport keeps the \
         borrowed visibility until something else drains"
    );
}
