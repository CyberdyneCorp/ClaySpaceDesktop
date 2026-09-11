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

/// The drag that is NOT previewed still costs one grab per image, not one per
/// segment.
///
/// Everything above is about the live transaction. This is the other path:
/// when no transaction is open — a mirror that could not be pointed, or a
/// caller that never opened one — `baked_stroke` falls to
/// `move_surface_stroke`, which writes the drag with
/// `clay_layer_move_surface`. That call coalesces successive grabs only while
/// the centre and the radius repeat **exactly**, so what the segments carry
/// decides whether the fallback costs one grab or one per pointer event.
///
/// It used to cost one per pointer event. `Stroke::pending` hands a
/// path-driven tool the last sample it already sent, so every segment after
/// the first began where the previous one stopped, the centre moved with the
/// pointer, and nothing coalesced. The chain's Lipschitz bound then multiplies
/// once per segment, which is exactly the decay the header of this file
/// describes for the pre-transaction live path — the fallback had kept it.
///
/// The ViewModel now replays a field Move from its anchor, pinned by
/// `every_segment_of_a_field_drag_also_starts_at_the_anchor`. That test pins
/// the decision; this one pins the consequence and the engine contract the
/// decision leans on. If a future engine stops coalescing a repeated centre,
/// the replay silently buys nothing and only this notices.
///
/// **One grab per mirror image, not one in total.** The layer mirror does not
/// reach this verb, so `baked_stroke` reflects the gesture by hand and calls
/// the engine once per image; each image coalesces within itself. Under the
/// x mirror that is two, and two is the floor rather than the defect.
#[test]
fn the_unpreviewed_drag_coalesces_to_one_grab_per_image() {
    const SEGMENTS: usize = 6;
    let anchor = [0.0f32, 0.0, 0.95];
    let path: Vec<GestureSample> = (0..=SEGMENTS)
        .map(|step| GestureSample {
            position: [anchor[0] + step as f32 * 0.03, anchor[1], anchor[2]],
            pressure: 1.0,
            time: step as f32,
        })
        .collect();

    let chain_of = |from_the_anchor: bool, symmetry: [bool; 3]| -> i32 {
        let mut document = sphere();
        // No `open_live_gesture`, which is what puts this on the fallback.
        SculptModel::begin_gesture(&mut document);
        let mut applied = 0usize;
        for end in 2..=path.len() {
            let from = if from_the_anchor {
                0
            } else {
                // What `pending` used to hand over: the newest samples plus
                // the last one already sent.
                applied.saturating_sub(1)
            };
            document
                .apply_stroke(ToolKind::Mover, brush(), &path[from..end], symmetry)
                .expect("a segment of the drag");
            applied = end;
        }
        let key = document.scene().active_layer().expect("a layer").key;
        let id = document.layer_id(key).expect("its id");
        document
            .document()
            .field_report(id, 0.0)
            .expect("a field report")
            .longest_deformer_chain
    };

    // Unmirrored: the sculptor made one drag, so the field carries one grab.
    assert_eq!(
        chain_of(true, [false; 3]),
        1,
        "sent from the anchor and unmirrored, {SEGMENTS} segments must coalesce \
         into the single grab the drag asked for"
    );
    // Mirrored: one per image, and the images do not stack each other.
    assert_eq!(
        chain_of(true, STARTING_SYMMETRY),
        2,
        "under one mirror the drag is written once per image, so two — a third \
         would mean the images are being appended rather than coalesced"
    );
    // And the shape the replay replaced, so this states the defect and not
    // only the fix. If these ever match the numbers above, the engine has
    // started coalescing a moving centre and the replay is no longer
    // load-bearing.
    assert_eq!(
        chain_of(false, [false; 3]) as usize,
        SEGMENTS,
        "re-anchoring each segment is what used to happen, and it is what costs \
         a grab per pointer event"
    );
    assert_eq!(
        chain_of(false, STARTING_SYMMETRY) as usize,
        SEGMENTS * 2,
        "and mirrored it cost one per image per segment"
    );
}

/// A press on an open drag starts its own drag, and does not extend the last.
///
/// The invariant used to be held by convention: `arm_live_move` refused while
/// a transaction was open, and everything downstream assumed the refusal meant
/// the press had been handled. It did not. `apply_stroke` routes to the live
/// path on `live_move.is_some()` without consulting the arming, so a second
/// press silently continued the FIRST transaction — measuring the new drag's
/// displacement from an anchor the sculptor had already released. And because
/// the caller had been told the gesture was not live, the release never closed
/// it: the orphan stayed open and collected every Move that followed.
///
/// Nothing in the application was found that could issue that press, which is
/// why this went unnoticed. It is pinned here because "no caller does this
/// today" is the kind of guarantee that a new caller silently revokes.
///
/// The second press abandons the first drag rather than banking it: a gesture
/// that never got its pointer-up has not earned a commit, which is the rule
/// the whole live path already runs on.
#[test]
fn a_second_press_abandons_the_open_drag_instead_of_extending_it() {
    const SEGMENTS: usize = 4;

    // What one clean drag leaves, for the second press to be measured against.
    let alone = {
        let mut document = sphere();
        assert!(document.open_live_gesture(ToolKind::Mover, STARTING_SYMMETRY));
        for step in 1..=SEGMENTS {
            document
                .apply_stroke(
                    ToolKind::Mover,
                    brush(),
                    &drag_to(step, SEGMENTS),
                    STARTING_SYMMETRY,
                )
                .expect("a segment");
        }
        document.close_live_gesture().expect("close");
        reach_along_x(&drawn_vertices(&document))
    };

    // The same drag, but preceded by an abandoned one that was never closed.
    let mut document = sphere();
    assert!(document.open_live_gesture(ToolKind::Mover, STARTING_SYMMETRY));
    for step in 1..=SEGMENTS {
        document
            .apply_stroke(
                ToolKind::Mover,
                brush(),
                &drag_to(step, SEGMENTS),
                STARTING_SYMMETRY,
            )
            .expect("a segment of the gesture that never ends");
    }

    // The press that never should have come, and no release before it.
    assert!(
        document.open_live_gesture(ToolKind::Mover, STARTING_SYMMETRY),
        "a press must open its own drag; refusing here is what used to leave \
         the first transaction open and collecting"
    );
    for step in 1..=SEGMENTS {
        document
            .apply_stroke(
                ToolKind::Mover,
                brush(),
                &drag_to(step, SEGMENTS),
                STARTING_SYMMETRY,
            )
            .expect("a segment of the second drag");
    }
    document.close_live_gesture().expect("close");

    let after = reach_along_x(&drawn_vertices(&document));
    assert!(
        (after - alone).abs() < 1e-3,
        "the second drag reached {after} where the same drag alone reaches \
         {alone}. The abandoned gesture is still in the surface, so the press \
         extended it rather than replacing it"
    );

    // And the field carries one drag's worth of grabs, not two.
    let key = document.scene().active_layer().expect("a layer").key;
    let id = document.layer_id(key).expect("its id");
    let chain = document
        .document()
        .field_report(id, 0.0)
        .expect("a field report")
        .longest_deformer_chain;
    assert_eq!(
        chain, 2,
        "one drag under one mirror is one grab per image; {chain} means the \
         abandoned drag was committed alongside it"
    );
}
