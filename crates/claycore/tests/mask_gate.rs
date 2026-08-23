//! Gating an item by a mask, and what the engine does with one.
//!
//! `clay_item_set_gate` is the entry point that would make masking protect a
//! surface from *any* operation rather than only from a brush. The engine's own
//! note is explicit about the gap it fills: a mask gates authoring elsewhere —
//! a voxel edit consumes one per cell as it writes, an SDF stroke consumes one
//! when it becomes items — but "neither touches an item already in the edit
//! list, so a mask over an ear has never done anything about the next boolean.
//! This does."
//!
//! Measured, in ClayCore 0.39.0, it does not. The call is accepted and the
//! subtraction eats the protected region anyway, at every width and threshold
//! tried, with a mask that samples 1.0 at the cut's own centre. That is against
//! the entry point's own contract twice over: it neither protects nor refuses,
//! and the contract promises one or the other — "Refused, leaving the item
//! ungated, when the mask is empty or nothing reaches the threshold — a gate
//! that protects nothing and reports success is harder to notice than a
//! failure."
//!
//! So this is written to **fail when the engine starts honouring it**. The
//! application does not call the gate, because a call per stroke that does
//! nothing is a cost with no benefit and a promise in the interface that would
//! not be kept. When this test fails, `stroke_sdf` is where the gate goes back.

use claycore::{
    BrushShape, Document, Falloff, Item, Mask, MeshParams, Op, StrokePreset, StrokeSample,
};

/// The highest surviving surface directly above the cut.
///
/// A subtraction eats down from the top, so this rising is what protection
/// would look like.
fn top_of_the_cut(document: &Document) -> f32 {
    let mesh = document
        .mesh(MeshParams {
            voxel_size: Some(0.02),
            ..Default::default()
        })
        .expect("mesh the document");
    mesh.positions()
        .iter()
        .filter(|p| p[0].abs() < 0.1 && p[1].abs() < 0.1 && p[2] > 0.0)
        .map(|p| p[2])
        .fold(f32::NEG_INFINITY, f32::max)
}

/// A mask painted solidly over the region the cut will cross.
fn mask_over_the_cut() -> Option<Mask> {
    let mut mask = Mask::new(0.02).ok()?;
    let samples: Vec<StrokeSample> = (0..5)
        .map(|i| {
            let t = i as f32 / 4.0;
            StrokeSample {
                position: [(t - 0.5) * 0.2, 0.0, 1.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    mask.apply_stroke(
        &samples,
        &StrokePreset {
            radius: 0.5,
            ..Default::default()
        },
        1.0,
        BrushShape::default(),
        Falloff::default(),
    )
    .ok()?;
    Some(mask)
}

/// A unit sphere with a smaller sphere subtracted from its top, gated or not.
fn cut_sphere(gate: Option<(&Mask, f32, f32)>) -> Option<Document> {
    let mut document = Document::new().ok()?;
    let layer = document.add_sdf_layer("corpo").ok()?;
    let mut body = Item::sphere(1.0).ok()?;
    body.set_op(Op::Add).ok()?;
    document.add_item(layer, &body).ok()?;

    let mut cut = Item::sphere(0.3).ok()?;
    cut.set_op(Op::Subtract).ok()?;
    cut.set_position([0.0, 0.0, 1.0]).ok()?;
    if let Some((mask, threshold, width)) = gate {
        // Accepted, which is half the problem.
        cut.set_gate(mask, threshold, width).ok()?;
    }
    document.add_item(layer, &cut).ok()?;
    Some(document)
}

/// The mask is real where the gate is asked to protect, so a gate that does
/// nothing cannot be blamed on an empty mask.
#[test]
fn the_mask_covers_the_region_the_gate_is_given() {
    let Some(mask) = mask_over_the_cut() else {
        return;
    };
    assert!(
        !mask.is_empty().expect("emptiness"),
        "the mask painted nothing"
    );
    assert!(
        mask.painted_count().expect("count") > 1000,
        "the mask is too sparse for the comparison to say anything"
    );
    assert!(
        (mask.sample([0.0, 0.0, 1.0]).expect("sample") - 1.0).abs() < 1e-3,
        "the mask does not protect the point the cut is centred on"
    );
}

/// The measurement, held in the direction that makes it a tripwire.
///
/// When this fails the engine has started honouring the gate. That is the good
/// outcome, and what changes with it is `clayspace-engine`'s `stroke_sdf`,
/// where the gate call was removed for exactly this reason.
#[test]
fn a_gate_is_accepted_and_does_not_yet_protect() {
    let Some(mask) = mask_over_the_cut() else {
        return;
    };
    let (Some(ungated), Some(gated)) = (cut_sphere(None), cut_sphere(Some((&mask, 0.0, 0.15))))
    else {
        return;
    };

    let open = top_of_the_cut(&ungated);
    let protected = top_of_the_cut(&gated);
    assert!(
        open < 0.99,
        "the ungated subtraction did not cut into the sphere ({open}), so the \
         comparison says nothing about the gate"
    );
    assert_eq!(
        open, protected,
        "the gate has started protecting the masked region. That is good news: \
         clayspace-engine's stroke_sdf removed its set_gate call because this \
         did nothing, and the call should go back."
    );
}

/// And it is inert at every width and threshold, so the application is not
/// simply holding it wrong.
#[test]
fn no_width_or_threshold_makes_the_gate_bite() {
    let Some(mask) = mask_over_the_cut() else {
        return;
    };
    let Some(ungated) = cut_sphere(None) else {
        return;
    };
    let open = top_of_the_cut(&ungated);

    for (threshold, width) in [
        (0.0, 0.05),
        (0.0, 0.15),
        (0.0, 0.4),
        (0.5, 0.15),
        (0.1, 0.15),
    ] {
        let Some(gated) = cut_sphere(Some((&mask, threshold, width))) else {
            return;
        };
        assert_eq!(
            top_of_the_cut(&gated),
            open,
            "a gate at threshold {threshold} and width {width} protected the \
             region. The engine honours the gate now, and the application \
             should call it again."
        );
    }
}
