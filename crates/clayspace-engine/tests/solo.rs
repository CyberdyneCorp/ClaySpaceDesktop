//! Showing one subtool alone, and bringing the rest back.
//!
//! The constraint that shapes all of this is the engine's: there is no journal
//! pause, so once undo is enabled every command is recorded — `SetLayerVisibleCmd`
//! among them — and the merged SDF surface cannot drop a layer any way other
//! than engine visibility. So solo writes visibility like anything else and the
//! host steps over the entries it made, which is what these hold: the pattern
//! comes back exactly, the sculptor's ⌘Z reaches their sculpt rather than a way
//! of looking at it, an operation that fails inside the hidden window leaves
//! the scene as it was, and a file written while soloed records what the
//! sculptor set rather than what they were looking at.

use std::path::PathBuf;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, DocumentModel, GestureSample, LayerKey, ModelError,
    ObjectModel, Representation, SceneModel, SculptModel, Shape, ToolKind,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Four subtools, each holding a form of its own.
///
/// Four rather than two because the pattern the spec asks to round-trip is a
/// mixed one — "three layers are visible, one is hidden" — and a pair cannot
/// tell a restore that puts the recorded flags back from one that shows
/// everything.
fn four_subtools() -> (ClayDocument, Vec<LayerKey>) {
    let mut doc = document();
    let mut keys = vec![doc.scene().active.expect("a starting layer")];
    for (index, name) in ["Segunda", "Terceira", "Quarta"].iter().enumerate() {
        let key = doc
            .add_layer(name, Representation::Sdf)
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
        // Clear of the others, so each has a surface of its own to answer for.
        doc.set_layer_transform(key, [3.0 * (index as f32 + 1.0), 0.0, 0.0], 1.0)
            .expect("stand it clear");
        keys.push(key);
    }
    doc.set_active_layer(keys[0]).expect("back to the first");
    (doc, keys)
}

/// What every layer's eye says, in stack order.
fn visibility(doc: &ClayDocument) -> Vec<bool> {
    doc.scene()
        .layers
        .iter()
        .map(|layer| layer.visible)
        .collect()
}

/// How many items a layer holds — what says whether a dab is still there.
fn items(doc: &ClayDocument, key: LayerKey) -> usize {
    let id = doc.layer_id(key).expect("a layer");
    doc.document().layer_nodes(id).expect("its nodes").len()
}

/// One dab at a point, on whatever layer is active.
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

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("clayspace-solo");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

// -- isolating and restoring -------------------------------------------------

#[test]
fn solo_shows_one_subtool_and_hides_the_rest() {
    let (mut doc, keys) = four_subtools();
    doc.set_solo(Some(keys[2])).expect("solo the third");

    assert_eq!(
        visibility(&doc),
        vec![false, false, true, false],
        "solo left more than the soloed subtool showing"
    );
    assert_eq!(
        doc.scene().soloed,
        Some(keys[2]),
        "the scene has to say which subtool is alone; the eyes alone cannot \
         tell a solo from a sculptor who hid three layers by hand"
    );
}

/// The spec's scenario, exactly: three visible, one hidden, solo, release.
#[test]
fn solo_round_trips_a_mixed_pattern() {
    let (mut doc, keys) = four_subtools();
    doc.set_layer_visible(keys[1], false)
        .expect("hide one by hand");
    let before = visibility(&doc);
    assert_eq!(before, vec![true, false, true, true]);

    doc.set_solo(Some(keys[0])).expect("solo the first");
    assert_eq!(visibility(&doc), vec![true, false, false, false]);

    doc.set_solo(None).expect("release the solo");
    assert_eq!(
        visibility(&doc),
        before,
        "releasing the solo showed a layer the sculptor had hidden, or hid one \
         they had shown"
    );
    assert_eq!(doc.scene().soloed, None, "the solo is no longer engaged");
}

/// A second solo restores what stood before the *first* one.
#[test]
fn soloing_a_second_subtool_still_restores_the_original_pattern() {
    let (mut doc, keys) = four_subtools();
    doc.set_layer_visible(keys[3], false)
        .expect("hide the last");
    let before = visibility(&doc);

    doc.set_solo(Some(keys[0])).expect("solo one");
    doc.set_solo(Some(keys[2])).expect("then another");
    assert_eq!(visibility(&doc), vec![false, false, true, false]);

    doc.set_solo(None).expect("release");
    assert_eq!(
        visibility(&doc),
        before,
        "the second solo restored what the first one left rather than what the \
         sculptor set"
    );
}

#[test]
fn solo_does_not_change_which_subtool_is_active() {
    let (mut doc, keys) = four_subtools();
    let active = doc.scene().active;
    assert_eq!(active, Some(keys[0]));

    doc.set_solo(Some(keys[2])).expect("solo another");
    assert_eq!(
        doc.scene().active,
        active,
        "solo moved the sculpt target; it is a way of looking at the scene, \
         not a way of choosing what to work on"
    );

    // And the next dab still lands where the sculptor left it, once the solo
    // is the one they are working on. A solo elsewhere hides the active
    // subtool, and a hidden layer refuses edits with that reason — the
    // sculptor is told, rather than dabbing on something nobody can see.
    doc.set_solo(Some(keys[0])).expect("solo the active one");
    let before = items(&doc, keys[0]);
    dab(&mut doc, [0.0, 0.0, 1.0]);
    assert!(
        items(&doc, keys[0]) > before,
        "the dab went somewhere other than the active subtool"
    );
}

/// The consequence of leaving activation alone: a solo somewhere else hides
/// the subtool the brush is pointed at, and the refusal says so.
#[test]
fn soloing_elsewhere_leaves_the_active_subtool_hidden_and_says_so() {
    let (mut doc, keys) = four_subtools();
    doc.set_solo(Some(keys[2])).expect("solo another");

    let refused = doc
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect_err("a hidden subtool takes no dab");
    assert!(
        matches!(refused, ModelError::Unavailable(_)),
        "the refusal has to be the stated one: {refused}"
    );
}

// -- history -----------------------------------------------------------------

#[test]
fn solo_adds_nothing_to_the_history() {
    let (mut doc, keys) = four_subtools();
    let before = doc.history();

    doc.set_solo(Some(keys[1])).expect("solo");
    doc.set_solo(None).expect("release");

    assert_eq!(
        doc.history().depth,
        before.depth,
        "solo left entries the sculptor would have to undo"
    );
}

/// The spec's other scenario: ⌘Z after a released solo takes back the sculpt.
#[test]
fn one_undo_after_a_solo_reverts_the_sculpt_and_not_the_visibility() {
    let (mut doc, keys) = four_subtools();
    doc.set_layer_visible(keys[1], false)
        .expect("hide one by hand");
    let pattern = visibility(&doc);

    let before = items(&doc, keys[0]);
    dab(&mut doc, [0.0, 0.0, 1.0]);
    let sculpted = items(&doc, keys[0]);
    assert!(sculpted > before, "the dab landed");

    doc.set_solo(Some(keys[2])).expect("solo");
    doc.set_solo(None).expect("release");

    assert!(doc.undo().expect("undo"), "there was an edit to take back");
    assert_eq!(
        items(&doc, keys[0]),
        before,
        "the undo took back the solo instead of the sculpt"
    );
    assert_eq!(
        visibility(&doc),
        pattern,
        "the undo moved visibility, which is not something the sculptor did"
    );
}

/// And forward again: the hop does not eat the redo of the edit under it.
#[test]
fn redo_after_hopping_a_solo_puts_the_sculpt_back() {
    let (mut doc, keys) = four_subtools();
    let before = items(&doc, keys[0]);
    dab(&mut doc, [0.0, 0.0, 1.0]);
    let sculpted = items(&doc, keys[0]);
    let pattern = visibility(&doc);

    doc.set_solo(Some(keys[1])).expect("solo");
    doc.set_solo(None).expect("release");

    assert!(doc.undo().expect("undo"));
    assert_eq!(items(&doc, keys[0]), before);
    assert!(doc.redo().expect("redo"), "the sculpt was not redoable");
    assert_eq!(
        items(&doc, keys[0]),
        sculpted,
        "the redo did not put the dab back"
    );
    assert_eq!(visibility(&doc), pattern, "the redo moved visibility");
}

/// A sculpt made *while* soloed is the newest edit, so undo takes it back and
/// leaves the solo standing.
#[test]
fn an_edit_made_while_soloed_undoes_on_its_own() {
    let (mut doc, keys) = four_subtools();
    doc.set_solo(Some(keys[0])).expect("solo the active one");

    let before = items(&doc, keys[0]);
    dab(&mut doc, [0.0, 0.0, 1.0]);
    assert!(items(&doc, keys[0]) > before);

    assert!(doc.undo().expect("undo"));
    assert_eq!(items(&doc, keys[0]), before, "the dab was not taken back");
    assert_eq!(
        doc.scene().soloed,
        Some(keys[0]),
        "undoing an edit made inside a solo released the solo as well"
    );
    assert_eq!(visibility(&doc), vec![true, false, false, false]);
}

/// Undoing a visibility change the *sculptor* made is still an undo.
///
/// The hop is for the entries solo wrote, not for every visibility command:
/// hiding a layer by hand is something a person did and ⌘Z owes them the eye
/// back. It did not get it — the engine reverts the flag and cannot say that it
/// has, so the stack went on showing the layer as hidden.
#[test]
fn undoing_a_hand_made_hide_brings_the_eye_back() {
    let (mut doc, keys) = four_subtools();
    let before = visibility(&doc);
    doc.set_layer_visible(keys[2], false).expect("hide one");
    assert_eq!(visibility(&doc), vec![true, true, false, true]);

    assert!(doc.undo().expect("undo"), "hiding a layer is undoable");
    assert_eq!(
        visibility(&doc),
        before,
        "the engine put the layer back and the stack went on saying it was hidden"
    );
}

// -- the primitive the boolean bake shares -----------------------------------

#[test]
fn an_operation_runs_with_only_the_layers_it_asked_for() {
    let (mut doc, keys) = four_subtools();
    doc.set_layer_visible(keys[1], false).expect("hide one");
    let before = visibility(&doc);

    let seen = doc
        .with_only_visible(&[keys[2]], |doc| Ok(visibility(doc)))
        .expect("the operation");

    assert_eq!(
        seen,
        vec![false, false, true, false],
        "the operation did not run with its operand alone"
    );
    assert_eq!(
        visibility(&doc),
        before,
        "the window was not closed behind it"
    );
}

/// The one that matters: a bake that refuses must not leave the scene hidden.
#[test]
fn a_failure_inside_the_hidden_window_restores_the_visibility() {
    let (mut doc, keys) = four_subtools();
    doc.set_layer_visible(keys[3], false).expect("hide one");
    let before = visibility(&doc);
    let history = doc.history().depth;

    let hidden_while_it_ran = std::cell::Cell::new(Vec::new());
    let outcome: Result<(), ModelError> = doc.with_only_visible(&[keys[0]], |doc| {
        hidden_while_it_ran.set(visibility(doc));
        Err(ModelError::engine("a operação falhou de propósito"))
    });

    assert!(outcome.is_err(), "the forced failure did not fail");
    assert_eq!(
        hidden_while_it_ran.take(),
        vec![true, false, false, false],
        "the operation did not run with the others hidden"
    );
    assert_eq!(
        visibility(&doc),
        before,
        "an operation that failed inside the window left the sculptor's scene hidden"
    );
    assert_eq!(
        doc.history().depth,
        history,
        "the window's own commands were left for the sculptor to undo"
    );
}

#[test]
fn the_window_keeps_a_solo_that_was_engaged_around_it() {
    let (mut doc, keys) = four_subtools();
    doc.set_solo(Some(keys[1])).expect("solo");

    let outcome: Result<(), ModelError> = doc.with_only_visible(&[keys[3]], |_| {
        Err(ModelError::engine("a operação falhou de propósito"))
    });
    assert!(outcome.is_err());

    assert_eq!(
        doc.scene().soloed,
        Some(keys[1]),
        "borrowing the flags for an operation released the sculptor's solo"
    );
    assert_eq!(visibility(&doc), vec![false, true, false, false]);
}

// -- saving ------------------------------------------------------------------

/// A file records what the sculptor set, not what they were looking at.
#[test]
fn saving_while_soloed_writes_the_real_visibility() {
    let (mut doc, keys) = four_subtools();
    doc.set_layer_visible(keys[1], false)
        .expect("hide one by hand");
    let pattern = visibility(&doc);

    doc.set_solo(Some(keys[3])).expect("solo the last");
    let path = scratch("soloed.clay");
    doc.save(&path).expect("save while soloed");

    assert_eq!(
        visibility(&doc),
        vec![false, false, false, true],
        "the save left the solo released"
    );
    assert_eq!(
        doc.scene().soloed,
        Some(keys[3]),
        "the save released the solo it was asked to look past"
    );

    let mut reopened = document();
    reopened.open(&path).expect("reopen");
    assert_eq!(
        visibility(&reopened),
        pattern,
        "the file recorded the solo, so the document reopened with everything \
         but one subtool hidden"
    );
    assert_eq!(
        reopened.scene().soloed,
        None,
        "a reopened document is not soloed; nobody asked it to be"
    );
}

// -- what history does around a solo ----------------------------------------

/// Removing the soloed subtool gives the rest of the scene back.
///
/// `remove_layer` never looked at `self.solo`, so it left the field naming a
/// key the document no longer had — and the layers the solo hid stayed hidden.
/// Measured before the fix: soloing the second of two subtools and removing it
/// reported `soloed Some(LayerKey(2))` over a scene whose only remaining layer
/// was hidden. The viewport is blank, the solo control is drawn per stack row
/// and the soloed row is the one that left, so there is nothing left to click.
#[test]
fn removing_the_soloed_subtool_brings_the_scene_back() {
    let (mut doc, keys) = four_subtools();
    doc.set_solo(Some(keys[2])).expect("solo the third");
    assert_eq!(visibility(&doc), vec![false, false, true, false]);

    doc.remove_layer(keys[2])
        .expect("remove the soloed subtool");

    assert_eq!(
        doc.scene().soloed,
        None,
        "the solo names a subtool the document no longer has"
    );
    assert_eq!(
        visibility(&doc),
        vec![true, true, true],
        "the layers the solo hid were left hidden with no row to release them"
    );
}

/// And removing something else leaves the solo standing.
#[test]
fn removing_another_subtool_leaves_the_solo_engaged() {
    let (mut doc, keys) = four_subtools();
    doc.set_solo(Some(keys[2])).expect("solo the third");
    doc.remove_layer(keys[0]).expect("remove the first");

    assert_eq!(doc.scene().soloed, Some(keys[2]));
    assert_eq!(visibility(&doc), vec![false, true, false]);

    doc.set_solo(None).expect("release");
    assert_eq!(
        visibility(&doc),
        vec![true, true, true],
        "releasing restores the layers that are still there"
    );
}

/// A redo after an ordinary edit is the sculptor's, not a solo's.
///
/// `visibility_redo` was cleared only by `write_visibility`, so a gesture left
/// on the redo side outlived new work and went on matching `depths.first() ==
/// engine_undo_depth() + 1` whenever the depth happened to return to that
/// value. Measured before the fix: solo, undo, a fresh dab, undo, redo — the
/// redo was spent by the *hop*, which put the dab back without resyncing the
/// object table or the layer transforms, and left the interface showing a solo
/// engaged over a scene in which every layer was visible.
#[test]
fn a_redo_after_a_new_edit_does_not_re_engage_a_released_solo() {
    let (mut doc, keys) = four_subtools();
    doc.set_active_layer(keys[0]).expect("the first");
    dab(&mut doc, [0.0, 0.0, 1.0]);
    let after_first = items(&doc, keys[0]);

    doc.set_solo(Some(keys[1])).expect("solo the second");
    doc.undo().expect("one step back");

    // A fresh edit, which ends the redo line the hop left behind.
    doc.set_active_layer(keys[0]).expect("the first again");
    dab(&mut doc, [0.0, 0.0, 1.0]);
    doc.undo().expect("take the fresh dab back");
    assert!(doc.redo().expect("put it back"), "there was a dab to redo");

    assert_eq!(
        doc.scene().soloed,
        None,
        "the redo put a dab back; nobody asked for a solo"
    );
    assert_eq!(
        visibility(&doc),
        vec![true; 4],
        "and no layer was hidden by a gesture that no longer describes \
         anything"
    );
    assert_eq!(
        items(&doc, keys[0]),
        after_first,
        "the dab is what came back, through the path that resyncs the object \
         table rather than through the visibility hop"
    );
}

/// A mesh subtool, imported and shown alone.
///
/// A mesh gesture is recorded against the engine's depth *without raising it*,
/// which is the whole reason the two histories interleave — and the reason a
/// solo engaged on either side of a stroke can be mistaken for it.
fn soloable_mesh(who: &str) -> (ClayDocument, LayerKey) {
    use clayspace_model::{ExchangeModel, ImportAs, ImportSettings};

    let mut doc = document();
    let path = scratch(&format!("carried-{who}.obj"));
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
    (doc, key)
}

/// The drawn vertices of the whole scene, for comparing a stroke against what
/// undid it.
fn carried(doc: &mut ClayDocument) -> Vec<[f32; 3]> {
    let (positions, ..) = doc.visible_mesh_geometry();
    positions
}

/// A mesh stroke made under a solo is what one undo takes back.
///
/// `undo` hopped the visibility gestures *before* asking whether a mesh
/// gesture was the newest thing. A mesh gesture records the engine's depth and
/// does not raise it, so a solo engaged before the stroke ends at exactly that
/// depth and both answered "newest" — and the hop won. Measured before the
/// fix: one dab on a soloed mesh subtool, one undo, and the solo was released
/// *and* the engine undid the entry underneath it — the import that created
/// the layer, so the subtool vanished — while the stroke itself was never
/// taken back and its gesture was stranded at a depth the engine would never
/// return to.
#[test]
fn a_mesh_stroke_made_under_a_solo_is_what_one_undo_takes_back() {
    let (mut doc, key) = soloable_mesh("undone");
    let layers = doc.scene().layers.len();
    doc.set_solo(Some(key)).expect("show it alone");

    let before = carried(&mut doc);
    dab(&mut doc, [0.0, 0.0, 1.0]);
    let sculpted = carried(&mut doc);
    assert_ne!(before, sculpted, "the dab moved no vertex");

    assert!(doc.undo().expect("one step back"));

    assert_eq!(
        doc.scene().layers.len(),
        layers,
        "the undo took back the entry under the solo instead of the stroke, \
         so the subtool the stroke was on is gone"
    );
    assert_eq!(
        doc.scene().soloed,
        Some(key),
        "the solo is a way of looking at the scene, not the edit the sculptor \
         asked to take back"
    );
    assert_eq!(
        carried(&mut doc),
        before,
        "the stroke is still standing after the undo that was meant for it"
    );
}

/// And one undone under a solo comes back.
///
/// `redo` hopped forward first, and the hop's guard — the gesture's first
/// depth is one past the engine's — is satisfied by a visibility gesture even
/// when a *mesh* undo, which moves no engine depth, is what was taken back
/// last. Measured before the fix: the redo was spent re-applying the solo, the
/// engine depth moved past what the mesh gesture recorded, and the stroke
/// could never be put back — `redo` answered false over a stroke the document
/// still held.
#[test]
fn a_mesh_stroke_undone_under_a_solo_comes_back() {
    let (mut doc, key) = soloable_mesh("redone");
    let before = carried(&mut doc);
    dab(&mut doc, [0.0, 0.0, 1.0]);
    let sculpted = carried(&mut doc);
    assert_ne!(before, sculpted);

    doc.set_solo(Some(key)).expect("show it alone");
    assert!(doc.undo().expect("one step back"));
    assert_eq!(
        carried(&mut doc),
        before,
        "the undo did not reach the stroke"
    );

    assert!(
        doc.redo().expect("one step forward"),
        "the redo was spent on the solo, so there is nothing left to put the \
         stroke back with"
    );
    assert_eq!(carried(&mut doc), sculpted, "the stroke did not come back");
}

/// A solo that is refused halfway leaves the scene as it was.
///
/// `write_visibility` states that "a batch that failed halfway is a batch whose
/// caller is about to restore" — `with_visibility` honours that and `set_solo`
/// did not. Each flag goes through `set_layer_visible`, and the engine refuses
/// a visibility write naming a *locked* layer, so a solo across one is a batch
/// that stops partway through. Before the fix it left the layers written so far
/// hidden with `self.solo` still `None`: the interface showed no solo engaged,
/// and offered nothing that would put the rest of the scene back.
#[test]
fn a_solo_refused_halfway_puts_the_scene_back() {
    let (mut doc, keys) = four_subtools();
    let before = visibility(&doc);
    doc.set_layer_protection(
        keys[2],
        clayspace_model::Protection {
            ghost: false,
            locked: true,
        },
    )
    .expect("lock the third");

    doc.set_solo(Some(keys[3]))
        .expect_err("a locked layer takes no visibility write");

    assert_eq!(
        visibility(&doc),
        before,
        "the flags written before the refusal were left where the failed \
         batch put them"
    );
    assert_eq!(
        doc.scene().soloed,
        None,
        "nothing is soloed, which is what the refusal means"
    );
}
