//! Gating an item by a mask, and what the engine does with one.
//!
//! `clay_item_set_gate` is the entry point that makes masking protect a
//! surface from *any* operation rather than only from a brush. The engine's
//! own note is explicit about the gap it fills: a mask gates authoring
//! elsewhere — a voxel edit consumes one per cell as it writes, an SDF stroke
//! consumes one when it becomes items — but "neither touches an item already
//! in the edit list, so a mask over an ear has never done anything about the
//! next boolean. This does."
//!
//! **It did not, for as long as this repository had measured it.** Through
//! ClayCore 0.66.0 the call was accepted and the subtraction ate the protected
//! region anyway, at every width and threshold tried, with a mask sampling 1.0
//! at the cut's own centre — against the entry point's own contract twice
//! over, since it neither protected nor refused and the contract promises one
//! or the other. So these tests were written as tripwires, to fail the day the
//! engine started honouring the gate.
//!
//! They fired on the move to 0.73.0. The cause was never the threshold or the
//! width: the gate was placed by the transform of *the item it protects*,
//! while the mask it measures is stored in world units, so a cut with a
//! placement carried its own protection away from where the mask was painted.
//! At the identity nothing moves, which is why no fixture upstream caught it.
//! Fixed in ABI 0.67.0 as CyberdyneCorp/ClayCore#394, and the header now says
//! outright that "the gate is in world space, and does not travel with the
//! item".
//!
//! So they are held the other way round now: the gate protects, and it
//! protects at every width and threshold the earlier version swept. That is
//! what lets `clayspace-engine`'s `stroke_sdf` set a gate on the stroke
//! template and have every stamp respect it, which is where the call went
//! back.

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

/// The measurement: a gate protects the region the mask covers.
///
/// Held as an inequality on the *rise*, not an equality against the ungated
/// cut. The gate is a distance measured from the mask and faded across
/// `width`, so a fully protected centre still sits a little below an
/// untouched sphere where the fade reaches it — an equality would be asking
/// the gate to be a step, which is precisely what it is documented not to be.
///
/// Measured on 0.73.0: the ungated subtraction takes the top of the sphere
/// from 1.0 to 0.729, and the gated one leaves it at 1.0.
#[test]
fn a_gate_protects_the_masked_region() {
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
    assert!(
        protected > open + 0.1,
        "the gate did not protect the masked region: {protected} against \
         {open} ungated, on a sphere that started at 1.0. This is what \
         clayspace-engine's stroke_sdf sets a gate for, and a mask that stops \
         protecting would leave the call there doing nothing"
    );
}

/// And it protects at every width and threshold, which is the sweep the
/// earlier version of this file used to establish the opposite.
///
/// Worth keeping rather than folding into the test above. What made the old
/// defect hard to read was that it looked like a tuning problem: the obvious
/// response to an inert gate is to try a different threshold, and this is the
/// evidence that no threshold was the answer. Held now, it says the same thing
/// in the other direction — the protection is not balanced on one lucky pair.
#[test]
fn every_width_and_threshold_protects() {
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
        (0.5, 0.15),
        (0.1, 0.15),
    ] {
        let Some(gated) = cut_sphere(Some((&mask, threshold, width))) else {
            return;
        };
        let protected = top_of_the_cut(&gated);
        assert!(
            protected > open + 0.1,
            "a gate at threshold {threshold} and width {width} left the region \
             unprotected: {protected} against {open} ungated"
        );
    }
}
