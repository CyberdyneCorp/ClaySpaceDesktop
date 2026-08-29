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
use clayspace_model::{BrushSettings, GestureSample, SceneModel, SculptModel, ToolKind};

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

#[test]
fn a_second_field_subtool_falls_back_to_the_gesture_being_held() {
    let mut document = sphere();
    document
        .add_layer("Segundo", clayspace_model::Representation::Sdf)
        .expect("a second field subtool");

    assert!(
        !document.open_live_gesture(ToolKind::Suavizar, STARTING_SYMMETRY),
        "the brick cache holds the union of every visible field layer and the \
         preview is of one, so this case is refused rather than drawn wrong"
    );
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
