//! A shape drawn on the view, and the form it leaves behind.
//!
//! Measured through `clay_eval_points` rather than a pick: a pick is answered
//! from a path that stays correct whether or not the brick cache was refilled,
//! so it cannot see a cut that was placed but not evaluated. The field is what
//! a cut changes, so the field is what is asserted.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{CutGesture, CutModel, DrawnCut, OutlineFrame, SculptModel};

fn sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// A frame looking along -Z, which is what a camera at the front gives: the
/// frame's x is world x and its y is world y.
fn frame() -> OutlineFrame {
    OutlineFrame {
        origin: [0.0; 3],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        scale: [1.0, 1.0],
    }
}

/// A horizontal line across the middle of the form, drawn `rightwards` or not.
fn across(rightwards: bool) -> DrawnCut {
    let track = if rightwards {
        vec![[-2.0, 0.0], [2.0, 0.0]]
    } else {
        vec![[2.0, 0.0], [-2.0, 0.0]]
    };
    DrawnCut {
        track,
        frame: frame(),
        gesture: CutGesture::Line,
    }
}

/// Whether a world point is inside the form.
fn inside(document: &ClayDocument, at: [f32; 3]) -> bool {
    document
        .document()
        .eval_points(None, &[at])
        .ok()
        .and_then(|values| values.first().copied())
        .is_some_and(|value| value < 0.0)
}

const HIGH: [f32; 3] = [0.0, 0.55, 0.0];
const LOW: [f32; 3] = [0.0, -0.55, 0.0];

#[test]
fn a_line_drawn_left_to_right_takes_what_is_below_it() {
    let Some(mut document) = sphere() else {
        return;
    };
    assert!(inside(&document, HIGH) && inside(&document, LOW), "fixture");

    document
        .apply_cut(&across(true))
        .expect("the cut was refused");

    assert!(
        inside(&document, HIGH),
        "the half above the line was removed, and the line was drawn rightwards"
    );
    assert!(
        !inside(&document, LOW),
        "the half below the line survived a rightward cut"
    );
}

/// The same line the other way keeps the other half, with no modifier touched.
/// That is the whole reason the direction is read.
#[test]
fn the_same_line_drawn_back_takes_the_other_half() {
    let Some(mut document) = sphere() else {
        return;
    };
    document
        .apply_cut(&across(false))
        .expect("the cut was refused");

    assert!(
        !inside(&document, HIGH),
        "drawn leftwards, the half above should have gone"
    );
    assert!(
        inside(&document, LOW),
        "the half below should have survived"
    );
}

/// A loop wound one way removes what it encloses; wound the other it keeps
/// only that. The lasso's answer to the question a line answers with its
/// direction.
#[test]
fn a_lasso_removes_or_keeps_what_it_encloses_by_the_way_it_is_wound() {
    let ring = |clockwise: bool| {
        let mut track: Vec<[f32; 2]> = (0..16)
            .map(|step| {
                let angle = step as f32 / 16.0 * std::f32::consts::TAU;
                [angle.cos() * 0.45, angle.sin() * 0.45 + 0.5]
            })
            .collect();
        if clockwise {
            track.reverse();
        }
        DrawnCut {
            track,
            frame: frame(),
            gesture: CutGesture::Lasso,
        }
    };

    let Some(mut removed) = sphere() else {
        return;
    };
    removed.apply_cut(&ring(true)).expect("clockwise refused");
    assert!(
        !inside(&removed, [0.0, 0.5, 0.0]),
        "a clockwise lasso should have removed what it enclosed"
    );

    let Some(mut kept) = sphere() else {
        return;
    };
    kept.apply_cut(&ring(false)).expect("anticlockwise refused");
    assert!(
        inside(&kept, [0.0, 0.5, 0.0]),
        "an anticlockwise lasso should have kept what it enclosed"
    );
    assert!(
        !inside(&kept, [0.0, -0.8, 0.0]),
        "and should have removed everything outside it"
    );
}

/// A gesture too small to have a direction is refused rather than guessed at.
#[test]
fn a_gesture_that_went_nowhere_is_refused() {
    let Some(mut document) = sphere() else {
        return;
    };
    let stationary = DrawnCut {
        track: vec![[0.1, 0.1], [0.1, 0.1]],
        frame: frame(),
        gesture: CutGesture::Line,
    };
    assert!(document.apply_cut(&stationary).is_err());
}

/// Every gesture's own refusal, reached.
///
/// There were two gates here — `is_drawn` in front and the per-gesture
/// arithmetic inside — and only one of them could ever run. The refusal test
/// above passed whichever it was, which is what made the pair invisible: a
/// guard that is never exercised reads at review as protection and is not.
/// One gate now, and this is what says each of its three arms is reachable.
///
/// Written to fail against the collapse being done the other way round — keep
/// `is_drawn` and delete the arms — because then all three of these report the
/// same sentence and the `contains` checks go red.
#[test]
fn each_gesture_is_refused_in_its_own_terms() {
    let Some(mut document) = sphere() else {
        return;
    };
    // A press that never travelled, a loop that encloses nothing, and a box
    // dragged to no width: one degenerate gesture per arm.
    let cases = [
        (CutGesture::Line, vec![[0.1, 0.1], [0.1, 0.1]], "direção"),
        (CutGesture::Lasso, vec![[0.0, 0.0], [0.5, 0.0]], "laço"),
        (
            CutGesture::Rectangle,
            vec![[0.2, 0.2], [0.2, 0.9]],
            "retângulo",
        ),
    ];
    for (gesture, track, says) in cases {
        let cut = DrawnCut {
            track,
            frame: frame(),
            gesture,
        };
        let refused = document
            .apply_cut(&cut)
            .expect_err("a degenerate gesture was cut with");
        assert!(
            refused.to_string().contains(says),
            "{gesture:?} was refused as {refused}, which does not say which \
             gesture failed — the arm for it is not the one that ran"
        );
    }
}

/// A cut is not reflected by the layer's mirror.
///
/// The mirror is a property of the layer and reflects its items, so a cut
/// added to a symmetric layer came back as **two** cuts — the one drawn and
/// one nobody drew. Right for a shape, which is what symmetry is for; wrong
/// for a trim, which is drawn on the view in the sculptor's own sight.
/// Reported from a session, where every cut appeared mirrored.
#[test]
fn a_cut_is_not_reflected_by_the_layers_mirror() {
    let Some(mut document) = sphere() else {
        return;
    };
    // Symmetry on, as a sculptor leaves it between strokes.
    document
        .apply_stroke(
            clayspace_model::ToolKind::Padrao,
            clayspace_model::BrushSettings::default(),
            &[clayspace_model::GestureSample {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [true, false, false],
        )
        .expect("a dab that points the mirror");

    // A lasso well to one side of the plane, wound to remove what it encloses.
    let mut track: Vec<[f32; 2]> = (0..16)
        .map(|step| {
            let angle = step as f32 / 16.0 * std::f32::consts::TAU;
            [0.6 + angle.cos() * 0.3, angle.sin() * 0.3]
        })
        .collect();
    track.reverse();
    document
        .apply_cut(&DrawnCut {
            track,
            frame: frame(),
            gesture: CutGesture::Lasso,
        })
        .expect("the cut was refused");

    assert!(
        !inside(&document, [0.6, 0.0, 0.0]),
        "the cut did not remove what it enclosed"
    );
    assert!(
        inside(&document, [-0.6, 0.0, 0.0]),
        "the cut was reflected: material was removed on the far side of the \
         form, where nothing was drawn"
    );
}

/// A rectangle cuts where it was drawn.
///
/// It was placed **twice** — `clay_cut_desc` measures the shape from the frame
/// origin, and a node transform was set to the rectangle's centre on top of
/// that, so the box landed at double its offset and cut empty space beside the
/// form. Reported from a session as "the rectangle is not working".
#[test]
fn a_rectangle_cuts_where_it_was_drawn() {
    let Some(mut document) = sphere() else {
        return;
    };
    // A box over the form's upper right, well off centre so a doubled offset
    // misses the sphere entirely.
    document
        .apply_cut(&DrawnCut {
            track: vec![[0.15, 0.15], [0.95, 0.95]],
            frame: frame(),
            gesture: CutGesture::Rectangle,
        })
        .expect("the rectangle was refused");

    assert!(
        !inside(&document, [0.5, 0.5, 0.0]),
        "the rectangle removed nothing where it was drawn"
    );
    assert!(
        inside(&document, [-0.5, -0.5, 0.0]),
        "the rectangle removed material on the opposite corner, where nothing \
         was drawn"
    );
}

/// One gesture, one entry in the history — and what it leaves can be taken
/// back, which a bake could not be.
///
/// The cut is bracketed in an undo group because placing the item and pointing
/// it are two engine edits and a sculptor asked for one thing. Without the
/// bracket the depth moves by two and the first undo leaves an item standing
/// where nobody put one.
#[test]
fn a_cut_is_one_undo_entry_and_gives_the_material_back() {
    let Some(mut document) = sphere() else {
        return;
    };
    assert!(inside(&document, LOW), "fixture");
    let before = document.history().depth;

    document.apply_cut(&across(true)).expect("the cut lands");
    assert!(!inside(&document, LOW), "the cut removed nothing");
    assert_eq!(
        document.history().depth,
        before + 1,
        "a cut is one thing a sculptor did, so it is one entry to take back"
    );

    assert!(document.undo().expect("undo"), "there was nothing to undo");
    assert!(
        inside(&document, LOW),
        "one undo did not give the material back, so what the cut left is not \
         an item in the tape"
    );
    assert!(document.redo().expect("redo"));
    assert!(!inside(&document, LOW), "the redo did not put the cut back");
}

/// Cutting again, and again, and again.
///
/// Reported from a session: "after some subsequent Line cuts it stops working,
/// it doesn't cut anymore". Three defects underneath it, and the first two
/// compounded:
///
/// 1. the polygon closing an open curve reached `span * 4.0 + 1.0` either side
///    of the line, so cutting a form 2.0 across placed an item 18 across;
/// 2. the next cut read the layer's bounds — now holding that item — as the
///    region it had to clear, so the reach grew eightfold per cut: measured
///    2.0, 18, 146, 1170, 9362. The second cut ran and removed nothing and the
///    third was refused;
/// 3. the shape was positioned relative to the *region's centre* rather than
///    the frame's own origin, so once a cut had made the form lopsided the
///    next one landed somewhere the sculptor had not drawn.
///
/// Only Line reached the first two — a lasso closes on its own points and a
/// rectangle is sized by the drag, so neither inflates anything — but all
/// three gestures read the same region and the same origin, so all three are
/// walked here. Measured before the fix: line 1 of 5 cuts landed, lasso 5,
/// rectangle 5; after it, 5, 5 and 5.
#[test]
fn successive_cuts_keep_cutting() {
    for (gesture, cuts) in [
        (CutGesture::Line, line_cuts()),
        (CutGesture::Lasso, lasso_cuts()),
        (CutGesture::Rectangle, rectangle_cuts()),
    ] {
        let Some(mut document) = sphere() else {
            return;
        };
        let started = layer_span(&document);
        for (step, (track, probe)) in cuts.into_iter().enumerate() {
            assert!(
                inside(&document, probe),
                "{gesture:?} {step}: the fixture's probe is already outside the \
                 form, so this step could not tell a working cut from a broken \
                 one"
            );
            document
                .apply_cut(&DrawnCut {
                    track,
                    frame: frame(),
                    gesture,
                })
                .unwrap_or_else(|e| panic!("{gesture:?} {step} was refused: {e}"));
            assert!(
                !inside(&document, probe),
                "{gesture:?} {step} removed nothing where it was drawn"
            );
            // The runaway, caught by its own number rather than by the cut
            // that fails several steps later. One bad cut took this from 2.0
            // to 18.0, so anything short of a doubling-and-a-bit is the fault
            // rather than the margin.
            let span = layer_span(&document);
            assert!(
                span < started * 4.0,
                "{gesture:?} {step}: the layer spans {span:.3} against {started:.3} \
                 before any cut. A cut is an item, so an oversized one grows the \
                 layer's bounds — and the next cut frames itself against those \
                 bounds"
            );
        }
    }
}

/// The largest axis of the layer's bounds.
fn layer_span(document: &ClayDocument) -> f32 {
    match SculptModel::bounds(document) {
        Some((low, high)) => (0..3)
            .map(|axis| (high[axis] - low[axis]).abs())
            .fold(0.0f32, f32::max),
        None => 0.0,
    }
}

/// Five slabs off the bottom, each above the last.
fn line_cuts() -> Vec<(Vec<[f32; 2]>, [f32; 3])> {
    (0..5)
        .map(|step| {
            let y = -0.7 + step as f32 * 0.1;
            (vec![[-2.0, y], [2.0, y]], [0.0, y - 0.03, 0.0])
        })
        .collect()
}

/// Five loops across the form, each clear of the last.
fn lasso_cuts() -> Vec<(Vec<[f32; 2]>, [f32; 3])> {
    (0..5)
        .map(|step| {
            let x = -0.6 + step as f32 * 0.3;
            let ring = (0..16)
                .map(|at| {
                    // Clockwise, which is the winding that removes what it
                    // encloses.
                    let angle = -(at as f32) / 16.0 * std::f32::consts::TAU;
                    [x + angle.cos() * 0.14, angle.sin() * 0.14]
                })
                .collect();
            (ring, [x, 0.0, 0.0])
        })
        .collect()
}

/// Five boxes across the form.
fn rectangle_cuts() -> Vec<(Vec<[f32; 2]>, [f32; 3])> {
    (0..5)
        .map(|step| {
            let x = -0.6 + step as f32 * 0.3;
            (vec![[x - 0.1, -0.1], [x + 0.1, 0.1]], [x, 0.0, 0.0])
        })
        .collect()
}

/// A camera that is not square to the form, cut a dozen times.
///
/// The sibling test above walks a frame squared up with the world, and it
/// **cannot see this**: with the region taken from the layer's bounds it stays
/// green, because a box projected onto a frame parallel to it is exactly its
/// own width. Turn the frame and the projection becomes the box's diagonal,
/// so each cut's item is larger than the region it was framed against and the
/// next cut inherits the difference.
///
/// Measured at 45 degrees about y, taking the region from the layer's bounds:
/// 2.000 before, then 4.83, 7.66, 10.49 — a steady 2.83 per cut — and from the
/// tenth the placement is refused outright. Against the surface's own extent
/// it is 5.071 and stays there however many times the sculptor cuts, because a
/// subtract cannot add surface.
///
/// So this asserts **stability** rather than a threshold. A cut that grows the
/// region grows it every time, and the size it settles at is a property of the
/// margin and the fixture; that it settles at all is the property under test.
#[test]
fn a_turned_frame_does_not_grow_what_the_next_cut_must_clear() {
    let Some(mut document) = sphere() else {
        return;
    };
    // Turned about y, so the region's projection onto the frame is the box's
    // diagonal rather than its side.
    let k = std::f32::consts::FRAC_1_SQRT_2;
    let turned = OutlineFrame {
        origin: [0.0; 3],
        right: [k, 0.0, k],
        up: [0.0, 1.0, 0.0],
        forward: [-k, 0.0, k],
        scale: [1.0, 1.0],
    };

    let mut settled: Option<f32> = None;
    for step in 0..12 {
        let y = -0.85 + step as f32 * 0.05;
        document
            .apply_cut(&DrawnCut {
                track: vec![[-2.0, y], [2.0, y]],
                frame: turned,
                gesture: CutGesture::Line,
            })
            .unwrap_or_else(|e| panic!("cut {step} on a turned frame was refused: {e}"));

        let span = layer_span(&document);
        match settled {
            None => settled = Some(span),
            Some(first) => assert!(
                (span - first).abs() < 0.01,
                "after cut {step} the layer spans {span:.3} where the first cut \
                 left {first:.3}. Each cut is enlarging what the next one \
                 believes it has to clear, which ends in a cut that is refused"
            ),
        }
    }
}
