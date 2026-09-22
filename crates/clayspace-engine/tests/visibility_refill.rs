//! What an eye is allowed to cost, and what it owes.
//!
//! Showing and hiding is the most ordinary thing in the layer stack and was
//! one of the most expensive: every flag written refilled a layer's bricks on
//! the UI thread, whether or not the field the cache holds has a term for that
//! layer, and a hop back through the history refilled whichever layer happened
//! to be selected rather than the ones whose eye had moved.
//!
//! Two promises, and they pull in opposite directions, which is why they are
//! held together here:
//!
//! - **A layer the field does not hold costs nothing.** The cache evaluates the
//!   fold of the visible SDF layers; a grid and a carried mesh arrive by the
//!   other route entirely, and `visible_mesh_geometry` selects which of them to
//!   hand over by the same flag. So their eye is honoured where the frame is
//!   assembled and there is nothing to re-evaluate.
//! - **A layer the field does hold is refilled, exactly.** Hiding one really
//!   does change the field, so the bricks it reached have to be rewritten or
//!   the viewport draws a surface the document no longer describes — and that
//!   is true of the layer whose flag moved, not of the active one.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, GestureSample, LayerKey, ObjectModel, Representation,
    SceneModel, SculptModel, Shape, ToolKind,
};

/// Where the subtools of [`four_subtools`] stand, so a ray can be aimed at one.
const APART: f32 = 3.0;

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Four field subtools, each holding a form of its own and standing clear of
/// the others.
///
/// Standing clear is the point: a ray aimed down one of them answers for that
/// subtool alone, so "which layers did the cache refill?" is a question the
/// surface itself can be asked.
fn four_subtools() -> (ClayDocument, Vec<LayerKey>) {
    let mut doc = document();
    let mut keys = vec![doc.scene().active.expect("a starting layer")];
    for index in 0..3 {
        let key = doc
            .add_layer(&format!("Subtool {}", index + 2), Representation::Sdf)
            .expect("another subtool");
        doc.place_object(
            Shape::Sphere,
            &[0.6],
            [0.0; 3],
            CombineSettings {
                op: Combine::Add,
                ..CombineSettings::default()
            },
        )
        .expect("a form in it");
        doc.set_layer_transform(key, [APART * (index as f32 + 1.0), 0.0, 0.0], 1.0)
            .expect("stand it clear");
        keys.push(key);
    }
    doc.set_active_layer(keys[0]).expect("back to the first");
    (doc, keys)
}

/// A grid beside the starting form, with material in it.
fn a_grid_beside_the_form() -> (ClayDocument, LayerKey) {
    let mut doc = document();
    doc.add_voxel_layer("Grade", 0.04).expect("a grid");
    let key = doc.scene().active.expect("the grid is active");
    doc.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size: 0.3,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: [0.0, 0.0, 1.0],
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    )
    .expect("deposit");
    (doc, key)
}

/// Where the **cache** says the surface is over one subtool, which is where
/// the viewport would draw it.
///
/// Asked of the cache and never of the document: a refill that did not happen
/// leaves the two disagreeing, and the document's answer is the one that is
/// right for the wrong reason.
fn surface_over(doc: &ClayDocument, x: f32) -> Option<f32> {
    doc.cache()
        .raycast([x, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("the cache was asked")
        .map(|hit| hit.position[2])
}

fn visibility(doc: &ClayDocument) -> Vec<bool> {
    doc.scene()
        .layers
        .iter()
        .map(|layer| layer.visible)
        .collect()
}

/// One dab on whatever layer is active, so a history step has something to
/// reach that is not a flag.
fn dab(doc: &mut ClayDocument, at: [f32; 3]) {
    doc.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size: 0.3,
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

// -- what a layer the field does not hold costs ------------------------------

#[test]
fn hiding_a_grid_does_not_dirty_the_field() {
    let (mut doc, grid) = a_grid_beside_the_form();
    // The viewport's pending set, emptied so what follows is this toggle's own
    // work and not the fixture's.
    doc.take_dirty_keys();

    doc.set_layer_visible(grid, false).expect("hide the grid");

    assert!(
        doc.dirty_keys().is_empty(),
        "hiding a grid refilled {} bricks of a field it is not a term of",
        doc.dirty_keys().len()
    );

    doc.set_layer_visible(grid, true).expect("show it again");
    assert!(
        doc.dirty_keys().is_empty(),
        "showing a grid refilled {} bricks",
        doc.dirty_keys().len()
    );
}

#[test]
fn a_grids_eye_still_reaches_what_is_drawn() {
    // The other half of the promise above: nothing is refilled *because* the
    // flag is read where the frame is assembled. A cheaper hide that stopped
    // hiding anything would pass the test before this one.
    let (mut doc, grid) = a_grid_beside_the_form();
    let shown = doc.visible_mesh_geometry().0.len();
    assert!(shown > 0, "the fixture deposited nothing");

    doc.set_layer_visible(grid, false).expect("hide the grid");

    assert_eq!(
        doc.visible_mesh_geometry().0.len(),
        0,
        "a hidden grid was still handed to the viewport"
    );

    doc.set_layer_visible(grid, true).expect("show it again");
    assert_eq!(
        doc.visible_mesh_geometry().0.len(),
        shown,
        "the grid did not come back whole"
    );
}

// -- what a layer the field does hold owes -----------------------------------

#[test]
fn hiding_a_field_subtool_refills_it() {
    let (mut doc, keys) = four_subtools();
    assert!(
        surface_over(&doc, APART).is_some(),
        "the second subtool was not on the surface to begin with"
    );

    doc.set_layer_visible(keys[1], false).expect("hide it");

    assert_eq!(
        surface_over(&doc, APART),
        None,
        "a hidden field subtool was still in the cache: the viewport would \
         draw a surface the document no longer describes"
    );
}

#[test]
fn showing_a_field_subtool_restores_the_same_surface() {
    let (mut doc, keys) = four_subtools();
    let before: Vec<Option<f32>> = (0..4)
        .map(|index| surface_over(&doc, APART * index as f32))
        .collect();

    doc.set_layer_visible(keys[1], false).expect("hide it");
    doc.set_layer_visible(keys[1], true).expect("show it again");

    assert_eq!(
        (0..4)
            .map(|index| surface_over(&doc, APART * index as f32))
            .collect::<Vec<_>>(),
        before,
        "the subtool came back somewhere other than where it was"
    );
}

#[test]
fn undoing_a_solo_refills_every_subtool_it_hid() {
    // The defect this is the regression for: the hop back refilled the ACTIVE
    // layer, which is not the layer whose eye moved. Soloing the second
    // subtool hides the other three; a ⌘Z gives all three back to the
    // document, and only the active one was given back to the cache — so the
    // stack showed four subtools and the viewport drew two.
    let (mut doc, keys) = four_subtools();
    dab(&mut doc, [0.0, 0.0, 1.0]);
    let before: Vec<Option<f32>> = (2..4)
        .map(|index| surface_over(&doc, APART * index as f32))
        .collect();
    assert!(
        before.iter().all(Option::is_some),
        "the far subtools were not on the surface to begin with"
    );

    doc.set_solo(Some(keys[1])).expect("solo the second");
    assert!(
        doc.undo().expect("undo"),
        "there was something to take back"
    );

    assert_eq!(
        visibility(&doc),
        vec![true; 4],
        "the hop did not put the flags back"
    );
    assert_eq!(
        (2..4)
            .map(|index| surface_over(&doc, APART * index as f32))
            .collect::<Vec<_>>(),
        before,
        "the subtools the solo hid came back in the stack and not in the cache"
    );
}
