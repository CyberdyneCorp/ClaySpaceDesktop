//! Sculpting state that belongs to a subtool rather than to the document.
//!
//! Symmetry, a mask and a rig were one apiece for the whole scene, and the
//! consequences were all of a kind: turning symmetry off to work one ear
//! turned it off on every other form, one frozen region was consulted whatever
//! was being sculpted, and a rig was found only when the layer holding it
//! happened to be active. The engine has been per-layer for all three from the
//! start — `clay_set_layer_mirror` takes a layer, a mask is world-addressed
//! and painted against what it covers, and armature nodes live in a layer — so
//! what these hold is that the host now agrees.
//!
//! The cage is the odd one out and deliberately so: it is a transient
//! authoring gesture, sized to what one form contains, and a box fitted to one
//! subtool means nothing around another. It is resolved on a switch rather
//! than carried, which is the last test here.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Armature, ArmatureModel, BrushSettings, DocumentModel, GestureSample, LatticeModel, LayerKey,
    MaskModel, Representation, SceneModel, SculptModel, ToolKind,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Two SDF subtools, the starting form and an empty one beside it, with the
/// first active — which is where a sculptor is before they reach for the
/// second.
fn two_subtools() -> (ClayDocument, LayerKey, LayerKey) {
    let mut doc = document();
    let first = doc.scene().active.expect("a starting layer");
    let second = doc
        .add_layer("Segunda", Representation::Sdf)
        .expect("a second layer");
    // Stamps rather than the strokes' own Relief, which *displaces an existing
    // surface* along its normal — the second subtool starts empty, so a Relief
    // dab on it moves nothing and would be indistinguishable from a dab a mask
    // froze. Add deposits, so the two are told apart by what the surface does.
    doc.set_combine(clayspace_model::CombineSettings {
        op: clayspace_model::Combine::Add,
        ..clayspace_model::CombineSettings::for_strokes()
    });
    doc.set_active_layer(first).expect("back to the first");
    (doc, first, second)
}

fn samples(at: [f32; 3], count: usize) -> Vec<GestureSample> {
    (0..count)
        .map(|i| GestureSample {
            position: at,
            pressure: 1.0,
            time: i as f32 * 0.01,
        })
        .collect()
}

/// One dab, mirrored by whatever the *subtool* is set to.
///
/// `[bool; 3]` is still handed in per stroke — it is what this gesture asks
/// for — so the axes come from the model rather than from the test, which is
/// the whole point: a stroke that supplied its own would prove nothing about
/// where the setting is kept.
fn dab(doc: &mut ClayDocument, at: [f32; 3]) -> Result<(), clayspace_model::ModelError> {
    let axes = SculptModel::symmetry(doc);
    doc.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size: 0.3,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        // Six, as `masking.rs` sends: one sample is one stamp and the stroke
        // engine spaces it by arc length, so a single point deposits something
        // too small to move the surface it lands on.
        &samples(at, 6),
        axes,
    )
    .map(|_| ())
}

/// Paints the mask over a spot, generously enough to cover a brush there.
fn freeze(doc: &mut ClayDocument, at: [f32; 3]) {
    doc.apply_stroke(
        ToolKind::Mascara,
        BrushSettings {
            size: 0.4,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &samples(at, 4),
        [false; 3],
    )
    .expect("paint the mask");
}

/// Whether the surface encloses a point.
fn inside(doc: &ClayDocument, at: [f32; 3]) -> bool {
    doc.document()
        .eval_points(None, &[at])
        .is_ok_and(|values| values[0] < 0.0)
}

/// How far the composed surface reaches along +z.
///
/// The whole document rather than one subtool, because that is what a raycast
/// answers and what a sculptor sees. It is enough to tell the two apart here:
/// only one of them is dabbed at a time.
fn reach(doc: &ClayDocument) -> f32 {
    SculptModel::pick(doc, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .map(|hit| hit[2])
        .expect("the near face")
}

// -- symmetry ----------------------------------------------------------------

#[test]
fn a_new_subtool_starts_with_x_symmetry_on() {
    let (mut doc, _, second) = two_subtools();
    doc.set_active_layer(second).expect("activate the second");
    assert_eq!(
        SculptModel::symmetry(&doc),
        [true, false, false],
        "a subtool arrives with the mirror the design asks for"
    );
}

#[test]
fn symmetry_off_on_one_subtool_survives_a_visit_to_another() {
    let (mut doc, first, second) = two_subtools();
    doc.set_symmetry([false; 3]).expect("turn symmetry off");

    doc.set_active_layer(second).expect("activate the second");
    assert_eq!(
        SculptModel::symmetry(&doc),
        [true, false, false],
        "the second subtool was handed the first one's setting"
    );

    doc.set_active_layer(first).expect("back to the first");
    assert_eq!(
        SculptModel::symmetry(&doc),
        [false; 3],
        "the visit to the second subtool carried its symmetry back"
    );
}

#[test]
fn a_mirror_turned_off_stays_off_through_a_visit_elsewhere() {
    // The setting is one thing and the engine's mirror is another: the ABI
    // sets a layer mirror and never reads one back, so the host's record is
    // only worth having if a stroke lands where the record says it will.
    // Before this was per layer, one number for the document meant the write
    // was skipped whenever the incoming subtool happened to be asked for the
    // same axes the outgoing one had — and the dab came out mirrored against
    // a plane nobody had asked for.
    let (mut doc, first, second) = two_subtools();
    doc.set_symmetry([false; 3]).expect("turn symmetry off");
    doc.set_active_layer(second).expect("activate the second");
    doc.set_active_layer(first).expect("back to the first");

    let off_centre = [0.7f32, 0.0, 0.7];
    dab(&mut doc, off_centre).expect("a dab on the near face");

    assert!(
        !inside(&doc, [-off_centre[0], off_centre[1], off_centre[2] + 0.35]),
        "the dab was mirrored on a subtool whose symmetry is off"
    );
}

// -- masks -------------------------------------------------------------------

#[test]
fn two_subtools_keep_independent_masks() {
    let (mut doc, first, second) = two_subtools();

    freeze(&mut doc, [0.0, 0.0, 1.0]);
    let painted = doc.mask_state().painted_cells;
    assert!(painted > 0, "nothing was frozen on the first subtool");

    doc.set_active_layer(second).expect("activate the second");
    assert_eq!(
        doc.mask_state(),
        clayspace_model::MaskState::default(),
        "the second subtool was handed the first one's mask"
    );

    freeze(&mut doc, [0.0, 0.0, -1.0]);
    let elsewhere = doc.mask_state().painted_cells;
    assert!(elsewhere > 0, "nothing was frozen on the second subtool");

    doc.set_active_layer(first).expect("back to the first");
    assert_eq!(
        doc.mask_state().painted_cells,
        painted,
        "the first subtool's mask is not the one it was painted with"
    );
}

#[test]
fn a_mask_gates_only_its_own_subtools_edits() {
    let (mut doc, _, second) = two_subtools();
    // Measured on the surface rather than on the item count: an SDF stroke
    // consumes the mask as it *becomes* items, so a fully frozen stroke can
    // still bank a stamp that reaches nothing — which is the engine's own
    // account of it, and what `masking.rs` measures too.
    let at = [0.0f32, 0.0, 1.0];
    let before = reach(&doc);

    freeze(&mut doc, at);
    dab(&mut doc, at).expect("a dab into the frozen region");
    let frozen = reach(&doc);
    assert!(
        (frozen - before).abs() < 0.002,
        "the frozen region moved from {before} to {frozen} on the subtool the \
         mask was painted on"
    );

    doc.set_active_layer(second).expect("activate the second");
    dab(&mut doc, at).expect("the same dab on the other subtool");
    let elsewhere = reach(&doc);
    assert!(
        elsewhere > frozen + 0.1,
        "the first subtool's mask froze the same place on the second: {frozen} \
         to {elsewhere}"
    );
}

#[test]
fn clearing_one_subtools_mask_leaves_the_others_alone() {
    let (mut doc, first, second) = two_subtools();
    freeze(&mut doc, [0.0, 0.0, 1.0]);
    let painted = doc.mask_state().painted_cells;

    doc.set_active_layer(second).expect("activate the second");
    freeze(&mut doc, [0.0, 0.0, -1.0]);
    doc.apply_mask_op(clayspace_model::MaskOp::Clear)
        .expect("clear the second subtool's mask");
    // Emptied rather than removed: a mask belongs to the layer inside the
    // engine's document — which is what makes it survive a save — and the
    // document has no verb for detaching one. `is_active` is the question the
    // interface asks, and it is the one that has to answer no.
    assert!(
        !doc.mask_state().is_active(),
        "the cleared mask still freezes {} cells",
        doc.mask_state().painted_cells
    );

    doc.set_active_layer(first).expect("back to the first");
    assert_eq!(
        doc.mask_state().painted_cells,
        painted,
        "clearing one subtool's mask took another's with it"
    );
}

// -- rigs --------------------------------------------------------------------

/// Puts a rig on a subtool of its own and names the layer, so it can be found
/// again in a reopened document.
fn rig(doc: &mut ClayDocument, name: &str, at: [f32; 3]) -> LayerKey {
    doc.begin_armature(at, 0.3).expect("start a rig");
    let key = doc.scene().active.expect("the rig's own layer");
    doc.rename_layer(key, name).expect("name the rig's layer");
    doc.add_zsphere(0, [at[0], at[1] + 0.6, at[2]], 0.25, false)
        .expect("a second sphere");
    key
}

fn layer_named(doc: &ClayDocument, name: &str) -> LayerKey {
    doc.scene()
        .layers
        .iter()
        .find(|layer| layer.name == name)
        .map(|layer| layer.key)
        .unwrap_or_else(|| panic!("no layer called {name}"))
}

fn tree_of(doc: &mut ClayDocument, key: LayerKey) -> Armature {
    doc.set_active_layer(key).expect("activate the rig's layer");
    doc.armature().expect("that subtool carries a rig")
}

#[test]
fn two_rigs_pose_independently_and_both_survive_a_reopen() {
    let mut doc = document();
    let left = rig(&mut doc, "Esquerda", [-1.5, 0.0, 0.0]);
    let right = rig(&mut doc, "Direita", [1.5, 0.0, 0.0]);

    // Posed on the right-hand rig only. The tip goes up; the other rig is not
    // addressed at all, and its own tip must be where it was left.
    doc.set_active_layer(right).expect("activate the right rig");
    doc.move_zsphere(1, [0.0, 0.9, 0.0]).expect("lift the tip");

    let posed = tree_of(&mut doc, right).nodes[1].position;
    let untouched = tree_of(&mut doc, left).nodes[1].position;
    assert!(
        (posed[1] - 1.5).abs() < 1e-3,
        "the pose did not reach the rig it was applied to: {posed:?}"
    );
    assert!(
        (untouched[1] - 0.6).abs() < 1e-3,
        "posing one rig moved the other: {untouched:?}"
    );

    let dir = std::env::temp_dir().join("clayspace-subtool-rigs");
    std::fs::create_dir_all(&dir).expect("a place to save");
    let path = dir.join("two-rigs.clay");
    doc.save(&path).expect("save");

    let mut reopened = document();
    reopened.open(&path).expect("reopen");

    let left = layer_named(&reopened, "Esquerda");
    let right = layer_named(&reopened, "Direita");
    let back_left = tree_of(&mut reopened, left).nodes[1].position;
    let back_right = tree_of(&mut reopened, right).nodes[1].position;
    assert!(
        (back_left[1] - 0.6).abs() < 1e-3,
        "the left rig came back somewhere else: {back_left:?}"
    );
    assert!(
        (back_right[1] - 1.5).abs() < 1e-3,
        "the right rig lost its pose: {back_right:?}"
    );
}

#[test]
fn a_subtool_with_no_rig_answers_with_none() {
    let mut doc = document();
    let plain = doc.scene().active.expect("the starting layer");
    rig(&mut doc, "Armação", [0.0, 0.0, 0.0]);
    doc.set_active_layer(plain).expect("back to the sculpt");
    assert!(
        doc.armature().is_none(),
        "a subtool carrying no rig was handed another subtool's"
    );
}

// -- the cage, which is not per-subtool state --------------------------------

#[test]
fn a_standing_cage_does_not_follow_a_subtool_switch() {
    let (mut doc, _, second) = two_subtools();
    doc.begin_lattice([2, 2, 2]).expect("a cage on the sculpt");
    let point = doc.lattice().points[0];
    doc.select_lattice_point(Some(0));
    doc.drag_lattice_point([point[0] - 0.4, point[1], point[2]])
        .expect("drag a corner");
    assert!(
        doc.lattice().touched,
        "the drag did not reach the cage, so the switch has nothing to resolve"
    );

    doc.set_active_layer(second).expect("activate the second");

    assert!(
        !doc.lattice().active,
        "the cage followed the switch onto a form it was never sized to"
    );
}
