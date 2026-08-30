//! An undo re-meshes what it reached, and not the active layer's bound.
//!
//! Until `clay_document_undo_bound`, the narrowest region nameable after a
//! step through the history was the active layer's own — the engine reverted
//! whatever it reverted and would not say where. That was wrong in two
//! directions at once. It re-meshed the whole layer to take back one dab
//! (measured at 1045 keys and 273 ms against the dab's 18 keys and 7.5 ms),
//! and where the step landed on some *other* subtool it re-meshed the active
//! one and left the layer that actually changed stale on screen.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, GestureSample, Representation, SceneModel, SculptModel, ToolKind,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far the drawn surface stands from the origin along `direction`.
fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> Option<f32> {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    let hit = document.pick(unit.map(|c| c * 4.0), unit.map(|c| -c))?;
    Some(hit.iter().map(|c| c * c).sum::<f32>().sqrt())
}

fn dab(document: &mut ClayDocument, at: [f32; 3]) {
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.2,
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
        .expect("a dab");
}

#[test]
fn undoing_an_edit_on_another_subtool_re_meshes_that_subtool() {
    let mut document = document();
    let first = document.scene().layers[0].key;

    // A second field subtool, made *before* the dab so that the newest entry
    // in the history is the dab and not the layer.
    let second = document
        .add_layer("Segundo", Representation::Sdf)
        .expect("a second subtool");

    // The dab lands on the starting form, which is the first subtool: a relief
    // stroke moves a surface that is already there, so an empty layer has
    // nothing for it to move.
    document
        .set_active_layer(first)
        .expect("activate the first");
    dab(&mut document, [1.0, 0.0, 0.0]);
    let raised = radius_along(&document, [1.0, 0.0, 0.0]).expect("the ray met the form");
    assert!(
        raised > 1.0 + 1e-3,
        "the dab did not raise the surface ({raised}), so undoing it proves nothing"
    );

    // Now the sculptor is looking at the *other* subtool, and takes the dab
    // back from there. The active layer is empty, so a re-mesh bounded by it
    // reaches nothing at all and the dab stays on screen.
    document
        .set_active_layer(second)
        .expect("activate the second");
    assert!(document.undo().expect("undo"), "there was nothing to undo");

    let after = radius_along(&document, [1.0, 0.0, 0.0]).expect("the ray met the form");
    assert!(
        after < raised - 1e-3,
        "the dab was undone but the surface still stands where it left it \
         ({after} against {raised}): the re-mesh followed the active layer \
         rather than the region the undo reached"
    );
}
