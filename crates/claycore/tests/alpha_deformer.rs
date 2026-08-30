//! The alpha deformer, and the one place it does not reach.
//!
//! `clay_item_add_alpha` appends a scalar stamp to an item's deformer chain: a
//! greyscale image read as a distance offset, so the surface moves along its own
//! normal. Pores, fabric, scales, stitching.
//!
//! It works on a *placed* item and not on a stroke's template, which is what
//! these two tests hold between them. That is not a wrapper defect and no
//! amount of parameter tuning changes it — the amplitude was swept and the
//! surface does not move — so the application refuses an alpha on an SDF stroke
//! by name rather than passing one that would be discarded. If a later ClayCore
//! carries the chain through a stroke, `a_stroke_does_not_carry_the_chain` is
//! what fails, and `clayspace_model::AlphaSupport` is what changes back.

use claycore::{Blend, Document, Item, MeshParams, Op, StrokePreset, StrokeSample};

/// Concentric rings, so a surface the stamp modulates is textured rather than
/// merely offset — a flat stamp and no stamp at all would be hard to tell
/// apart.
fn rings(side: usize, frequency: f32) -> Vec<f32> {
    (0..side)
        .flat_map(|y| {
            (0..side).map(move |x| {
                let (dx, dy) = (
                    x as f32 / (side - 1) as f32 - 0.5,
                    y as f32 / (side - 1) as f32 - 0.5,
                );
                (((dx * dx + dy * dy).sqrt() * frequency).sin() * 0.5 + 0.5).clamp(0.0, 1.0)
            })
        })
        .collect()
}

/// How far the surface ranges in z over the patch the stamp covers.
///
/// A stamp textures the surface, so what it changes is the *spread* rather than
/// any one height — a measurement at a single point reads the same with and
/// without one, because a relief displacement saturates.
fn spread(document: &Document) -> f32 {
    let mesh = document
        .mesh(MeshParams {
            voxel_size: Some(0.01),
            ..Default::default()
        })
        .expect("mesh the document");
    let near: Vec<f32> = mesh
        .positions()
        .iter()
        .filter(|p| p[0].abs() < 0.3 && p[1].abs() < 0.3 && p[2] > 0.0)
        .map(|p| p[2])
        .collect();
    if near.is_empty() {
        return 0.0;
    }
    let lo = near.iter().cloned().fold(f32::INFINITY, f32::min);
    let hi = near.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    hi - lo
}

fn sphere_document() -> Option<(Document, claycore::LayerId)> {
    let mut document = Document::new().ok()?;
    let layer = document.add_sdf_layer("corpo").ok()?;
    let mut body = Item::sphere(1.0).ok()?;
    body.set_op(Op::Add).ok()?;
    document.add_item(layer, &body).ok()?;
    Some((document, layer))
}

/// An item with an alpha, placed. The stamp reaches the surface and the
/// amplitude grades it.
#[test]
fn a_placed_item_carries_its_alpha() {
    let mut spreads = Vec::new();
    for amplitude in [0.0f32, 0.05, 0.2] {
        let Some((mut document, layer)) = sphere_document() else {
            return;
        };
        let mut item = Item::sphere(0.4).expect("a stamp item");
        item.set_op(Op::Add).expect("op");
        item.set_blend(Blend::Quadratic, 0.4).expect("blend");
        item.set_position([0.0, 0.0, 1.0]).expect("place it");
        if amplitude > 0.0 {
            let samples = rings(64, 12.0);
            item.add_alpha(
                &samples,
                64,
                64,
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                0.8,
                0.4,
                amplitude,
                0,
            )
            .expect("an alpha on a placed item");
        }
        document.add_item(layer, &item).expect("place the item");
        spreads.push(spread(&document));
    }

    assert!(
        spreads[1] > spreads[0],
        "a stamp left the surface as smooth as none at all: {spreads:?}"
    );
    assert!(
        spreads[2] != spreads[1],
        "the amplitude changed nothing between {} and {}, so the stamp is \
         being applied at a fixed strength: {spreads:?}",
        0.05,
        0.2
    );
}

/// And a stroke does not.
///
/// `clay_layer_apply_stroke` documents its item as "the stamp template scaled
/// to each stamp's radius". Measured, the chain hung off that template is not
/// carried: the same stroke at three amplitudes leaves one surface, under an
/// operation that builds a shape and under one that displaces alike.
///
/// Recorded rather than worked around. The application refuses an alpha on an
/// SDF stroke because of this, and a test that only checked the placed case
/// would let that refusal look like caution rather than a measurement.
#[test]
fn a_stroke_does_not_carry_the_chain() {
    for op in [Op::Add, Op::Relief] {
        let mut spreads = Vec::new();
        for amplitude in [0.0f32, 0.05, 0.25] {
            let Some((mut document, layer)) = sphere_document() else {
                return;
            };
            let mut item = Item::sphere(0.35).expect("a stamp item");
            item.set_op(op).expect("op");
            item.set_blend(Blend::Quadratic, 0.35).expect("blend");
            item.set_rounding(0.35).expect("rounding");
            if amplitude > 0.0 {
                let samples = rings(64, 14.0);
                item.add_alpha(
                    &samples,
                    64,
                    64,
                    [0.0; 3],
                    [0.0, 0.0, 1.0],
                    [1.0, 0.0, 0.0],
                    0.7,
                    0.35,
                    amplitude,
                    0,
                )
                .expect("the wrapper accepts it; the stroke is what drops it");
            }
            let samples: Vec<StrokeSample> = (0..6)
                .map(|i| {
                    let t = i as f32 / 5.0;
                    StrokeSample {
                        position: [(t - 0.5) * 0.4, 0.0, 1.0],
                        pressure: 1.0,
                        time: t,
                    }
                })
                .collect();
            document
                .apply_stroke(
                    layer,
                    &samples,
                    &StrokePreset::default(),
                    &item,
                    claycore::MaskSource::None,
                )
                .expect("a stroke");
            spreads.push(spread(&document));
        }

        // Held as "the same" rather than "different", which is the unusual
        // direction for a test and the deliberate one: this records an engine
        // limitation, and it should fail the day the limitation lifts.
        assert!(
            (spreads[1] - spreads[0]).abs() < 1e-3 && (spreads[2] - spreads[0]).abs() < 1e-3,
            "a stroke under {op:?} now carries its template's deformer chain \
             ({spreads:?}). That is good news: clayspace_model::AlphaSupport \
             refuses an alpha on an SDF stroke because of this, and both should \
             change together."
        );
    }
}
