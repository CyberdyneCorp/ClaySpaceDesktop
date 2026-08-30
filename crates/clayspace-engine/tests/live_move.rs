//! A Move drag that costs the field one grab instead of one per segment.
//!
//! Move degrades an SDF layer by a mechanism `live_smooth.rs` says nothing
//! about. A drag appends a `grab` to the deformer chain of every item it
//! reaches, and the engine's Lipschitz bound for a chain is the *product* of
//! its links — so writing a grab per segment made the safe step scale decay
//! geometrically in the number of segments, and the marcher's cost rise with
//! it. Measured on this starting form before the transaction was adopted,
//! twelve drags took the step scale from 0.264 to below what a float reports
//! and a segment from 5.2 ms to 26 ms.
//!
//! `claycore`'s `a_session_of_drags_steepens_by_the_drag_and_no_longer_by_the
//! _segment` holds the engine-level claim. This file holds the application's:
//! that the drag is *shown* while it is made, that the document carries none of
//! it until the pointer comes up, and that abandoning one leaves nothing
//! behind.
//!
//! ## Why the preview is not `live_smooth.rs`'s preview
//!
//! A Smooth transaction hands over sampled bricks and the application meshes a
//! lattice of its own from them. A Move transaction hands over no samples at
//! all — ClayCore's C++ class exposes a `preview_layer()` for this and the C
//! ABI does not carry it (see `docs/roadmap.md`, under *Known costs and escape
//! routes*). So the drag is drawn by writing the transaction's resolved grabs
//! onto the layer, sampling them into the document's own brick cache, and
//! undoing them within the same segment.
//! What stays on screen is the cache, which keeps what it was last given.

use clayspace_engine::claycore;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SceneModel, SculptModel, ToolKind};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// The starting form's own symmetry, so opening a gesture does not point the
/// layer's mirror — which is an edit, and one this file is not about.
const STARTING_SYMMETRY: [bool; 3] = [true, false, false];

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.35,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// How far the drag travels, whatever it is cut into.
const TRAVEL: f32 = 0.30;

/// The drag so far, as the ViewModel sends it: a path-driven tool is told
/// where it started as well as where it is now, so a segment carries the
/// gesture from its anchor.
///
/// One gesture over [`TRAVEL`], cut into `segments` of them; `step` is how
/// many have been made. Cutting the *same* drag more finely is the comparison
/// this file turns on, so the travel cannot depend on the count.
fn drag_to(step: usize, segments: usize) -> Vec<GestureSample> {
    (0..=step)
        .map(|i| {
            let t = i as f32 / segments as f32;
            GestureSample {
                position: [1.0 + t * TRAVEL, 0.0, 0.0],
                pressure: 1.0,
                time: i as f32 * 0.016,
            }
        })
        .collect()
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

/// How far the drawn surface reaches along +x, which is what this drag pulls.
fn reach_along_x(vertices: &[[f32; 3]]) -> f32 {
    vertices
        .iter()
        .filter(|v| v[1].abs() < 0.1 && v[2].abs() < 0.1)
        .fold(f32::NEG_INFINITY, |far, v| far.max(v[0]))
}

fn step_scale(document: &ClayDocument) -> f32 {
    let key = document.scene().active_layer().expect("a layer").key;
    document
        .layer_cost(key)
        .expect("layer cost")
        .safe_step_scale
}

#[test]
fn a_drag_shows_itself_before_the_document_changes() {
    let mut document = sphere();
    let before_depth = document.history().depth;
    let resting = reach_along_x(&drawn_vertices(&document));

    assert!(
        document.open_live_gesture(ToolKind::Mover, STARTING_SYMMETRY),
        "an editable field subtool is exactly the case a live drag is for"
    );

    const SEGMENTS: usize = 6;
    let mut moved = false;
    for step in 1..=SEGMENTS {
        document
            .apply_stroke(
                ToolKind::Mover,
                brush(),
                &drag_to(step, SEGMENTS),
                STARTING_SYMMETRY,
            )
            .expect("a live segment");
        moved |= reach_along_x(&drawn_vertices(&document)) > resting + 1e-3;
    }

    assert!(
        moved,
        "the drawn surface never followed the pointer; a Move that only \
         appears on release is the regression this file exists to catch"
    );
    assert_eq!(
        document.history().depth,
        before_depth,
        "a live drag writes nothing to the document until it commits — the \
         preview is drawn and taken back inside each segment"
    );
    assert_eq!(
        step_scale(&document),
        1.0,
        "the layer's field is supposed to be untouched mid-drag: a step scale \
         below one says a preview grab was left on it"
    );
}

#[test]
fn a_whole_drag_costs_the_field_one_grab() {
    // The measurement that motivated the change, at the application's level:
    // the same gesture delivered in more segments must not cost the field more.
    let scale_after = |segments: usize| {
        let mut document = sphere();
        assert!(document.open_live_gesture(ToolKind::Mover, STARTING_SYMMETRY));
        for step in 1..=segments {
            document
                .apply_stroke(
                    ToolKind::Mover,
                    brush(),
                    &drag_to(step, segments),
                    STARTING_SYMMETRY,
                )
                .expect("a live segment");
        }
        document.close_live_gesture().expect("commit");
        step_scale(&document)
    };

    let few = scale_after(3);
    let many = scale_after(24);
    assert!(
        (few - many).abs() < 1e-3,
        "the same drag cost the field {few} in three segments and {many} in \
         twenty-four: the gesture is being written per segment again"
    );
}

#[test]
fn a_drag_lands_where_the_preview_showed_it() {
    let mut document = sphere();
    const SEGMENTS: usize = 8;
    assert!(document.open_live_gesture(ToolKind::Mover, STARTING_SYMMETRY));
    for step in 1..=SEGMENTS {
        document
            .apply_stroke(
                ToolKind::Mover,
                brush(),
                &drag_to(step, SEGMENTS),
                STARTING_SYMMETRY,
            )
            .expect("a live segment");
    }
    let previewed = reach_along_x(&drawn_vertices(&document));
    let entries = document.close_live_gesture().expect("commit");
    let installed = reach_along_x(&drawn_vertices(&document));

    assert!(
        (previewed - installed).abs() < 1e-2,
        "the drag previewed at {previewed} and landed at {installed}"
    );
    assert_eq!(
        entries, 1,
        "a whole drag is one history entry, however many segments drew it"
    );
}

#[test]
fn one_undo_takes_a_whole_drag_back() {
    const SEGMENTS: usize = 10;
    let mut document = sphere();
    let resting = reach_along_x(&drawn_vertices(&document));

    assert!(document.open_live_gesture(ToolKind::Mover, STARTING_SYMMETRY));
    for step in 1..=SEGMENTS {
        document
            .apply_stroke(
                ToolKind::Mover,
                brush(),
                &drag_to(step, SEGMENTS),
                STARTING_SYMMETRY,
            )
            .expect("a live segment");
    }
    let entries = document.close_live_gesture().expect("commit");
    let dragged = reach_along_x(&drawn_vertices(&document));
    assert!(dragged > resting + 1e-3, "the drag did not land");

    for _ in 0..entries {
        assert!(
            document.undo().expect("undo"),
            "an entry the commit reported"
        );
    }
    let back = reach_along_x(&drawn_vertices(&document));
    assert!(
        (back - resting).abs() < 1e-2,
        "spending the {entries} entries the drag reported left the surface at \
         {back} where it rested at {resting}"
    );
}

#[test]
fn an_abandoned_drag_leaves_neither_a_mark_nor_a_preview() {
    let mut document = sphere();
    let before_depth = document.history().depth;
    let resting = reach_along_x(&drawn_vertices(&document));

    const SEGMENTS: usize = 6;
    assert!(document.open_live_gesture(ToolKind::Mover, STARTING_SYMMETRY));
    for step in 1..=SEGMENTS {
        document
            .apply_stroke(
                ToolKind::Mover,
                brush(),
                &drag_to(step, SEGMENTS),
                STARTING_SYMMETRY,
            )
            .expect("a live segment");
    }
    assert_eq!(
        document.discard_live_gesture(),
        0,
        "nothing was written, so nothing is owed back"
    );

    assert_eq!(document.history().depth, before_depth);
    let after = reach_along_x(&drawn_vertices(&document));
    assert!(
        (after - resting).abs() < 1e-2,
        "an abandoned drag left the surface at {after} where it rested at \
         {resting}: the preview was never taken off the cache"
    );
}

#[test]
fn a_press_that_never_drags_leaves_nothing_open() {
    let mut document = sphere();
    let before_depth = document.history().depth;
    assert!(document.open_live_gesture(ToolKind::Mover, STARTING_SYMMETRY));
    // A click: the pointer went down and came up without travelling.
    assert_eq!(
        document.close_live_gesture().expect("close"),
        0,
        "a press that never became a drag records nothing"
    );
    assert_eq!(document.history().depth, before_depth);
    assert!(
        !document.live_gesture_is_open(),
        "the gesture is over and must not still be holding the layer"
    );
}
