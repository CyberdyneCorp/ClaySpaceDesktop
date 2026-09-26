//! What a step through the history owes the rig and the curve in hand.
//!
//! Both are host state over a document the engine moves: the tree of ZSpheres
//! and the list of control points live on this side, while the radii and the
//! guide they produce live in the document, where undo can reach them and
//! nothing can tell this side that it did. The answer in both cases is to read
//! them back after every step — and the ways that went wrong are what these
//! cover.
//!
//! Driven through the model rather than through a ViewModel: the seam being
//! tested is between `ClayDocument` and the engine, and the application's own
//! banking of a gesture is covered in `clayspace-app`'s `armature_undo`.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{ArmatureModel, CurveModel, SceneModel, SculptModel, SkinSettings};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy).expect("a document")
}

/// A root with one sphere hanging off it, on a rig layer of its own.
fn rig(document: &mut ClayDocument) {
    document.begin_armature([0.0, 0.0, 0.0], 0.3).expect("root");
    document
        .add_zsphere(0, [0.5, 0.0, 0.0], 0.2, false)
        .expect("a child");
}

fn radii(document: &ClayDocument) -> Vec<f32> {
    document
        .armature()
        .expect("a tree")
        .nodes
        .iter()
        .map(|node| node.radius)
        .collect()
}

/// Steps back until the rig's own layer has left the scene, and says how far
/// that was.
fn undo_past_the_rig(document: &mut ClayDocument) -> usize {
    let mut steps = 0;
    while SceneModel::scene(document).layers.len() > 1 {
        assert!(
            SculptModel::undo(document).expect("undo"),
            "the history ran out with the rig's layer still in the scene"
        );
        steps += 1;
    }
    steps
}

#[test]
fn a_rig_survives_undo_and_redo() {
    // The rig's creation is several engine entries — a layer, the subtools it
    // hides, the placed armature — so this goes back over all of them and
    // forward again, which is what the interface's Undo and Redo do.
    let mut document = document();
    rig(&mut document);
    let before = document.armature().expect("a tree");

    let steps = undo_past_the_rig(&mut document);
    assert!(
        document.armature().is_none(),
        "the rig outlived the layer it was on"
    );
    for _ in 0..steps {
        assert!(SculptModel::redo(&mut document).expect("redo"), "no redo");
    }

    let after = document
        .armature()
        .expect("the redone rig never came back to the editor");
    assert_eq!(after.nodes.len(), before.nodes.len());
    assert_eq!(after.nodes[1].position, before.nodes[1].position);

    // And it is a rig that can be worked on, not a picture of one. Each of
    // these edits the tree this side holds and rewrites the document from it,
    // so a rig recovered in name only refuses all three.
    let tip = document
        .add_zsphere(1, [1.0, 0.0, 0.0], 0.15, false)
        .expect("a redone rig refused a new sphere");
    document
        .resize_zsphere(tip, 0.25)
        .expect("a redone rig refused a resize");
    document
        .reparent_zsphere(tip, 0)
        .expect("a redone rig refused a reparent");
    assert_eq!(
        document.armature().expect("a tree").nodes[tip as usize].parent,
        0
    );
}

#[test]
fn radii_round_trip_at_a_non_default_thickness() {
    // The tree keeps the radii a sculptor authored and the document keeps them
    // with the thickness applied, so reading one back is a division — and it
    // used to divide by `SkinSettings::default()` whatever the slider said.
    // At thickness 0.5 that returned radii twice the size on every step, and
    // the next edit wrote them back out: a rig that halved, then halved again,
    // with no way back.
    let mut document = document();
    rig(&mut document);
    document
        .set_skin(SkinSettings { thickness: 0.5 })
        .expect("a thinner skin");
    let authored = radii(&document);

    for cycle in 0..5 {
        assert!(SculptModel::undo(&mut document).expect("undo"));
        assert!(SculptModel::redo(&mut document).expect("redo"));
        assert_eq!(
            radii(&document),
            authored,
            "the radii moved on cycle {cycle}"
        );
    }
}

#[test]
fn a_thickness_change_is_undoable() {
    // The multiplier is not in the document — the engine is handed radii with
    // it already applied — so undo reverts the rewrite and cannot revert the
    // slider. Left at that, the old radii were read back through the new
    // thickness and the tree came out wrong.
    let mut document = document();
    rig(&mut document);
    let authored = radii(&document);

    document
        .set_skin(SkinSettings { thickness: 0.5 })
        .expect("a thinner skin");
    assert!(SculptModel::undo(&mut document).expect("undo"));
    assert_eq!(
        document.skin().thickness,
        1.0,
        "the thickness stayed where the undone change left it"
    );
    assert_eq!(radii(&document), authored, "the tree followed the slider");

    assert!(SculptModel::redo(&mut document).expect("redo"));
    assert_eq!(document.skin().thickness, 0.5);
    assert_eq!(radii(&document), authored);
}

#[test]
fn undoing_across_a_thickness_does_not_compound_it() {
    // The sequence from the report: thicken, undo, resize another sphere.
    // Each cycle used to write the untouched spheres back scaled once more,
    // and setting the thickness back to 1 left them there.
    let mut document = document();
    rig(&mut document);
    let root = radii(&document)[0];

    for cycle in 0..4 {
        document
            .set_skin(SkinSettings { thickness: 2.0 })
            .expect("a thicker skin");
        assert!(SculptModel::undo(&mut document).expect("undo"));
        document.resize_zsphere(1, 0.15).expect("resize");
        assert_eq!(
            radii(&document)[0],
            root,
            "the untouched root moved on cycle {cycle}"
        );
    }
    document
        .set_skin(SkinSettings { thickness: 1.0 })
        .expect("back to the default");
    assert_eq!(radii(&document), vec![root, 0.15]);
}

fn active(document: &ClayDocument) -> clayspace_model::LayerKey {
    SceneModel::scene(document).active.expect("an active layer")
}

#[test]
fn thickness_is_per_rig() {
    // It was one document-wide value, so a thickness chosen for one rig was
    // the thickness every other rig was read back through on the next step —
    // and a rig written at 1 and read at 2 came back with its radii halved.
    let mut document = document();
    rig(&mut document);
    let first = active(&document);
    let first_radii = radii(&document);

    rig(&mut document);
    let second = active(&document);
    assert_ne!(first, second, "each rig has a subtool of its own");
    document
        .set_skin(SkinSettings { thickness: 2.0 })
        .expect("a thicker skin on the second rig");
    let second_radii = radii(&document);

    for cycle in 0..3 {
        assert!(SculptModel::undo(&mut document).expect("undo"));
        assert!(SculptModel::redo(&mut document).expect("redo"));
        assert_eq!(radii(&document), second_radii, "cycle {cycle}");
    }

    document.set_active_layer(first).expect("switch");
    assert_eq!(
        document.skin().thickness,
        1.0,
        "the second rig's thickness showed up on the first"
    );
    assert_eq!(
        radii(&document),
        first_radii,
        "the first rig was read back through the second one's thickness"
    );

    document.set_active_layer(second).expect("switch back");
    assert_eq!(document.skin().thickness, 2.0);
    assert_eq!(radii(&document), second_radii);
}

#[test]
fn a_thickness_is_refused_without_a_rig_and_unchanged_is_not_a_step() {
    let mut document = document();
    let depth = SculptModel::history(&document).depth;
    assert!(
        document.set_skin(SkinSettings { thickness: 2.0 }).is_err(),
        "a subtool without a rig took a thickness"
    );
    assert_eq!(SculptModel::history(&document).depth, depth);

    rig(&mut document);
    let depth = SculptModel::history(&document).depth;
    document
        .set_skin(SkinSettings { thickness: 1.0 })
        .expect("the thickness it already has");
    assert_eq!(
        SculptModel::history(&document).depth,
        depth,
        "a thickness that changed nothing was recorded as a step"
    );
}

/// A curve laid across the front of the starting form.
fn lay(document: &mut ClayDocument) {
    document.begin_curve();
    for (at, radius) in [
        ([-0.9f32, 1.4, 0.0], 0.12f32),
        ([0.0, 1.7, 0.0], 0.16),
        ([0.9, 1.4, 0.0], 0.10),
    ] {
        document
            .add_curve_point(at, radius)
            .expect("the point was refused");
    }
}

fn shaped() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

#[test]
fn a_curve_resyncs_after_a_history_step() {
    // The guide is the document's and the point list is the hand's, and there
    // was nothing keeping them in step: the hand kept every point it had ever
    // placed while the document gave them back one at a time.
    let mut document = shaped();
    lay(&mut document);
    assert_eq!(document.curve().points.len(), 3);

    assert!(SculptModel::undo(&mut document).expect("undo"));
    assert_eq!(
        document.curve().points.len(),
        2,
        "the hand kept a point the document had given back"
    );

    assert!(SculptModel::redo(&mut document).expect("redo"));
    let points = document.curve().points;
    assert_eq!(points.len(), 3, "redo did not bring the point back");
    assert_eq!(points[2].position, [0.9, 1.4, 0.0]);
}

#[test]
fn an_undone_curve_point_does_not_come_back_on_the_next_edit() {
    // The compounding half of the same defect. A hand still holding the
    // undone point appended past it, so the very next point wrote it out
    // again — the undo looked as if it had never happened.
    let mut document = shaped();
    lay(&mut document);
    assert!(SculptModel::undo(&mut document).expect("undo"));

    document
        .add_curve_point([0.4, 1.9, 0.0], 0.1)
        .expect("a point after an undo was refused");

    let points = document.curve().points;
    assert_eq!(
        points.len(),
        3,
        "the undone point came back beside the new one"
    );
    assert_eq!(points[2].position, [0.4, 1.9, 0.0]);
}

#[test]
fn a_curve_undone_past_its_sweep_comes_back_whole() {
    // Far enough back that the swept node itself is gone: the hand empties,
    // because the points *are* the guide. Forward again it is the same curve
    // rather than a second one placed beside the first, which is what the
    // node's own id being remembered is for.
    let mut document = shaped();
    lay(&mut document);

    let mut steps = 0;
    while !document.curve().points.is_empty() {
        assert!(SculptModel::undo(&mut document).expect("undo"));
        steps += 1;
    }
    assert!(
        document.curve().active,
        "the curve left the sculptor's hand rather than emptying"
    );

    for _ in 0..steps {
        assert!(SculptModel::redo(&mut document).expect("redo"));
    }
    assert_eq!(
        document.curve().points.len(),
        3,
        "the curve came back short"
    );

    document
        .add_curve_point([1.4, 1.0, 0.0], 0.1)
        .expect("the recovered curve refused a point");
    assert_eq!(document.curve().points.len(), 4);
}
