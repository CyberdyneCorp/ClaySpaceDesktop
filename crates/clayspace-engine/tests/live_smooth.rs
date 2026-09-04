//! Smoothing that shows itself while the pointer is down.
//!
//! Suavizar bakes on a field, so it used to be held whole and arrive only when
//! the pointer came up: the sculptor smoothed blind. ClayCore 0.60.0's
//! transaction holds the sampled volume open across pointer events, and this
//! file is about the two claims that makes:
//!
//! 1. **The surface moves while the gesture is being made**, and the document
//!    does not — no nodes, no history, nothing to undo until it commits.
//! 2. **The result lands where the preview showed it.** Not by the same
//!    arithmetic: the preview relaxes the transaction's retained volume
//!    cumulatively per dab, and the stroke is laid down by the bake that was
//!    always used — see `ClayDocument::close_live_gesture` for why the
//!    transaction's own commit is not taken. So the claim held here is
//!    agreement within a tolerance, and the tolerance is stated.
//!
//! What *is* exact is the preview itself: it is drawn from a lattice of the
//! transaction's own, relabelled into a cache rather than resampled onto the
//! document's, so the samples the mesher sees are the samples the engine
//! computed.

use clayspace_engine::claycore;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, CombineSettings, GestureSample, LayerKey, ObjectModel, Representation,
    SceneModel, SculptModel, Shape, ToolKind,
};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// The starting form's own symmetry.
///
/// Passed rather than `[false; 3]` so that opening the gesture does not point
/// the layer's mirror, which is an edit of its own — see
/// `a_gesture_that_changes_the_mirror_takes_it_back`, which is about exactly
/// that entry.
const STARTING_SYMMETRY: [bool; 3] = [true, false, false];

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.18,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// A short drag across the +x limb, as the ViewModel would send it: one
/// segment per step, with the gesture open throughout.
fn samples(step: usize) -> Vec<GestureSample> {
    let t = step as f32 * 0.06;
    vec![GestureSample {
        position: [0.55, t - 0.1, 0.0],
        pressure: 1.0,
        time: step as f32 * 0.016,
    }]
}

/// Every vertex the drawn surface currently carries, in world space.
fn drawn_vertices(document: &ClayDocument) -> Vec<[f32; 3]> {
    let (cache, offset) = document.drawn_cache();
    let keys = cache.surface_bricks().expect("surface bricks");
    let live = document.live_gesture_is_open();
    let (mesh, _) = cache
        .mesh(
            (!live).then(|| document.document()),
            claycore::BrickMeshParams {
                gradient_normals: false,
                colors: false,
                gradient_eps: None,
            },
            &keys,
        )
        .expect("mesh the drawn surface");
    mesh.positions()
        .iter()
        .map(|p| [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]])
        .collect()
}

/// How far the drawn surface reaches along +x, which is what a smoothing pass
/// over a limb changes.
fn reach_along_x(vertices: &[[f32; 3]]) -> f32 {
    vertices
        .iter()
        .filter(|v| v[1].abs() < 0.1 && v[2].abs() < 0.1)
        .fold(f32::NEG_INFINITY, |far, v| far.max(v[0]))
}

#[test]
fn a_smoothing_gesture_shows_itself_before_the_document_changes() {
    let mut document = sphere();
    let before_depth = document.history().depth;
    let before = drawn_vertices(&document);

    assert!(
        document.open_live_gesture(ToolKind::Suavizar, STARTING_SYMMETRY),
        "a lone field subtool is exactly the case a live gesture is for"
    );

    let mut moved = false;
    for step in 0..6 {
        document
            .apply_stroke(
                ToolKind::Suavizar,
                brush(),
                &samples(step),
                STARTING_SYMMETRY,
            )
            .expect("a live dab");
        let during = drawn_vertices(&document);
        moved |= !during.is_empty() && during != before;
    }

    assert!(
        moved,
        "the drawn surface never changed while the gesture was being made, \
         which is the whole point of a live one"
    );
    assert_eq!(
        document.history().depth,
        before_depth,
        "a live gesture writes nothing to the document until it commits"
    );
}

#[test]
fn the_stroke_lands_where_the_preview_showed_it() {
    let mut document = sphere();
    document.open_live_gesture(ToolKind::Suavizar, STARTING_SYMMETRY);
    for step in 0..6 {
        document
            .apply_stroke(
                ToolKind::Suavizar,
                brush(),
                &samples(step),
                STARTING_SYMMETRY,
            )
            .expect("a live dab");
    }
    let previewed = drawn_vertices(&document);
    assert!(!previewed.is_empty(), "the preview drew nothing");

    document.close_live_gesture().expect("commit");
    let installed = drawn_vertices(&document);

    // Compared as reach rather than vertex for vertex: the two are meshed from
    // lattices offset from each other, so a vertex of one has no partner in
    // the other. What has to agree is where the surface is.
    //
    // A hundredth of a unit on a form of radius one — the two are different
    // computations of the same smoothing, so this is the distance a sculptor
    // would have to be able to see for the preview to be lying. Measured on
    // the roughened reference surface the two land 0.09 apart in roughness,
    // 5.74 against 5.83, which is the same statement in the other units.
    let (shown, kept) = (reach_along_x(&previewed), reach_along_x(&installed));
    assert!(
        (shown - kept).abs() < 0.01,
        "the surface moved when the gesture was laid down: previewed {shown}, \
         committed {kept} — far enough that the preview was showing something \
         else"
    );
}

#[test]
fn abandoning_a_live_gesture_leaves_the_document_as_it_was() {
    let mut document = sphere();
    let before_depth = document.history().depth;
    let before = drawn_vertices(&document);

    document.open_live_gesture(ToolKind::Suavizar, STARTING_SYMMETRY);
    for step in 0..4 {
        document
            .apply_stroke(
                ToolKind::Suavizar,
                brush(),
                &samples(step),
                STARTING_SYMMETRY,
            )
            .expect("a live dab");
    }
    document.discard_live_gesture();

    assert!(!document.live_gesture_is_open());
    assert_eq!(
        document.history().depth,
        before_depth,
        "an abandoned live gesture left something behind to undo"
    );
    assert_eq!(
        drawn_vertices(&document),
        before,
        "an abandoned live gesture left the surface changed"
    );
}

// -- a second field subtool, which used to refuse the preview ---------------
//
// The transaction previews one layer, and the viewport meshes the preview
// instead of the document's cache — so with a second visible field subtool the
// preview was the layer under the brush and nothing else, and the rest of the
// scene would have vanished for the length of the drag. The gesture refused to
// be live at all rather than draw that. It was filed upstream as ClayCore#378
// and ClayCore 0.78.0 answers it: the document can be evaluated over every
// visible SDF layer *except* one, which is the other half of what the preview
// holds, and `clayspace_engine::live` composes the two by a minimum.
//
// What made the obvious route impossible is worth restating, because it is
// what these tests are really about: hiding the other subtools, sampling, and
// showing them again is three edits to the document, and the commit refuses a
// layer that moved since the transaction began. The excluding evaluation edits
// nothing and records no undo entry, so it is legal in the middle of one.

/// A second field subtool standing two units along +x, well clear of the
/// starting form so the two can be told apart by where their vertices are.
const BESIDE: [f32; 3] = [2.0, 0.0, 0.0];

/// The starting form, and a second subtool beside it. Returns both keys with
/// the *first* one active, which is the layer the gesture is made on.
fn two_subtools() -> (ClayDocument, LayerKey, LayerKey) {
    let mut document = sphere();
    let first = document
        .scene()
        .active
        .expect("the starting form is active");
    let second = document
        .add_layer("Segundo", Representation::Sdf)
        .expect("a second field subtool");
    document
        .place_object(Shape::Sphere, &[0.5], BESIDE, CombineSettings::default())
        .expect("something in it to be seen");
    document
        .set_active_layer(first)
        .expect("smooth the starting form, not the new subtool");
    (document, first, second)
}

/// How far the drawn surface reaches along +x, over everything drawn.
fn far_along_x(vertices: &[[f32; 3]]) -> f32 {
    vertices
        .iter()
        .fold(f32::NEG_INFINITY, |far, v| far.max(v[0]))
}

/// The inversion of the test this file used to carry.
///
/// It asserted that a second field subtool refused the live gesture. It now
/// asserts the gesture opens *and* that the second subtool is still on screen
/// while the first is being smoothed — which is the thing the refusal existed
/// to avoid getting wrong.
#[test]
fn a_second_field_subtool_is_still_drawn_while_the_first_is_smoothed() {
    let (mut document, _first, _second) = two_subtools();
    let settled = far_along_x(&drawn_vertices(&document));
    assert!(
        settled > 2.0,
        "the fixture never drew the second subtool at all: reach {settled}"
    );

    assert!(
        document.open_live_gesture(ToolKind::Suavizar, STARTING_SYMMETRY),
        "a second field subtool no longer refuses the preview"
    );
    document
        .apply_stroke(ToolKind::Suavizar, brush(), &samples(0), STARTING_SYMMETRY)
        .expect("a live dab");

    let drawn = drawn_vertices(&document);
    assert!(
        !drawn.is_empty(),
        "the preview drew nothing at all with two subtools visible"
    );
    let during = far_along_x(&drawn);
    assert!(
        during > 2.0,
        "the second subtool vanished while the first was smoothed: the preview \
         reached only {during} along +x, against {settled} before the gesture \
         opened. That is the defect the old refusal existed to prevent."
    );
    // And the layer being smoothed is still there too, which says the
    // composition is a union rather than a replacement.
    assert!(
        drawn.iter().any(|v| v[0].abs() < 1.2),
        "the layer under the brush is missing from its own preview"
    );
}

/// The regression test the release notes ask for by name.
///
/// Every evaluation of the rest of the document is taken between
/// `clay_sdf_smooth_begin` and its commit. The route this replaces — hide the
/// other subtools, sample, show them again — is three edits, and the commit
/// correctly refuses a layer that moved since it began. So the claim is not
/// that the composition looks right but that the gesture it was taken inside
/// of still commits, and that it wrote nothing while it was open.
#[test]
fn composing_the_rest_of_the_document_does_not_spoil_the_gesture_it_is_inside() {
    let (mut document, _first, _second) = two_subtools();
    let before = document.history().depth;

    assert!(document.open_live_gesture(ToolKind::Suavizar, STARTING_SYMMETRY));
    for step in 0..6 {
        document
            .apply_stroke(
                ToolKind::Suavizar,
                brush(),
                &samples(step),
                STARTING_SYMMETRY,
            )
            .expect("a live dab");
        assert_eq!(
            document.history().depth,
            before,
            "something in the live path wrote to the document: an excluding \
             evaluation records no undo entry, and an edit here is what makes \
             the commit refuse"
        );
    }

    let recorded = document
        .close_live_gesture()
        .expect("the gesture the exclusions were taken inside of must commit");
    assert!(
        recorded > 0,
        "the commit recorded nothing, so it did not install the smooth"
    );
    assert!(document.history().depth > before);
}

/// A hidden subtool is not composed in, and does not make the gesture pay for
/// a composition it has nothing to compose.
///
/// The engine's own rule is the asymmetry this leans on: excluding a hidden
/// layer succeeds rather than refusing, because a hidden layer contributes
/// nothing to the union already — see `claycore`'s
/// `the_brick_half_refuses_an_unknown_layer_and_accepts_a_hidden_one` for the
/// other side of it, an unknown layer, which is `NotFound`. Here the same fact
/// is reached from above: hiding the second subtool takes it out of what is
/// drawn, and the preview agrees.
#[test]
fn a_hidden_second_subtool_is_left_out_of_the_preview() {
    let (mut document, _first, second) = two_subtools();
    document
        .set_layer_visible(second, false)
        .expect("hide the second subtool");

    assert!(document.open_live_gesture(ToolKind::Suavizar, STARTING_SYMMETRY));
    document
        .apply_stroke(ToolKind::Suavizar, brush(), &samples(0), STARTING_SYMMETRY)
        .expect("a live dab");

    let during = far_along_x(&drawn_vertices(&document));
    assert!(
        during < 2.0,
        "a hidden subtool was drawn into the preview: it reached {during} \
         along +x, and the hidden form stands at {}",
        BESIDE[0]
    );
    document.discard_live_gesture();
}

/// A second subtool with nothing in it is not a refusal either.
///
/// It has no bounds to widen the preview lattice over and contributes nothing
/// to the union, so the gesture opens and the preview is what it would have
/// been with one subtool. Stated as a test because "an empty layer succeeds"
/// is half of the engine's own asymmetry, and the half a host is most likely
/// to get wrong by refusing it.
#[test]
fn an_empty_second_subtool_neither_refuses_the_gesture_nor_changes_it() {
    let mut document = sphere();
    let first = document.scene().active.expect("active");
    document
        .add_layer("Vazia", Representation::Sdf)
        .expect("an empty second field subtool");
    document.set_active_layer(first).expect("back to the form");

    assert!(
        document.open_live_gesture(ToolKind::Suavizar, STARTING_SYMMETRY),
        "an empty second subtool refused a gesture it has nothing to do with"
    );
    document
        .apply_stroke(ToolKind::Suavizar, brush(), &samples(0), STARTING_SYMMETRY)
        .expect("a live dab");
    assert!(
        !drawn_vertices(&document).is_empty(),
        "the preview drew nothing beside an empty subtool"
    );
    document.close_live_gesture().expect("commit");
}

#[test]
fn the_surface_the_viewport_draws_swaps_with_the_gesture() {
    let mut document = sphere();
    let settled = document.surface_epoch();

    document.open_live_gesture(ToolKind::Suavizar, STARTING_SYMMETRY);
    let opened = document.surface_epoch();
    assert_ne!(
        opened, settled,
        "the viewport was not told the surface it holds is no longer the one \
         being drawn, so it would patch preview keys into document geometry"
    );

    document
        .apply_stroke(ToolKind::Suavizar, brush(), &samples(0), STARTING_SYMMETRY)
        .expect("a live dab");
    document.close_live_gesture().expect("commit");
    assert_ne!(document.surface_epoch(), opened);
}

#[test]
fn a_gesture_that_changes_the_mirror_takes_it_back_when_abandoned() {
    let mut document = sphere();
    let before = document.history().depth;

    // Symmetry the layer does not already have, so opening the gesture points
    // the mirror — an edit, and one made *before* the transaction begins
    // because an edit after it begins is one the commit refuses.
    assert!(document.open_live_gesture(ToolKind::Suavizar, [false; 3]));
    assert_eq!(
        document.history().depth,
        before + 1,
        "pointing the mirror is an entry, and this test is about that entry"
    );

    let owed = document.discard_live_gesture();
    assert_eq!(
        owed, 1,
        "an abandoned gesture has to report what its opening wrote, or the \
         symmetry change outlives the stroke that asked for it"
    );
}
