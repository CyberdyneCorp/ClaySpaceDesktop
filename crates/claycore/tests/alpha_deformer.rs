//! The alpha deformer, and the one place it does not reach.
//!
//! `clay_item_add_alpha` appends a scalar stamp to an item's deformer chain: a
//! greyscale image read as a distance offset, so the surface moves along its own
//! normal. Pores, fabric, scales, stitching.
//!
//! It works on a *placed* item, and on a stroke's template it is not resolved
//! into each stamp's frame — which is what these two tests hold between them.
//! That is not a wrapper defect and no amount of parameter tuning changes it,
//! so the application refuses an alpha on an SDF stroke by name rather than
//! passing one that would land somewhere else. Filed upstream as
//! CyberdyneCorp/ClayCore#392. If a later ClayCore resolves the chain,
//! `a_stroke_does_not_carry_the_chain_into_each_stamp` is what fails, and
//! `clayspace_model::AlphaSupport` is what changes back.
//!
//! **Re-checked at ClayCore v0.78.0 and unchanged.** #392 appears nowhere in
//! that release — not in its fixes, not in its known limits, and no entry
//! point answering it is among the 146 the ABI gained. What the release does
//! say about alphas is a *different* defect and the two must not collapse into
//! one sentence: "a stroke still duplicates its alpha's samples per stamp —
//! roughly 800 MB of blob for a 200-stamp stroke carrying a 1024x1024 alpha.
//! Filed, not fixed here." That is memory, and it is about a stroke that
//! carries an alpha correctly. This file is about one that does not carry it
//! at all. Both are open; only the second is what `AlphaSupport` refuses for.
//!
//! **The second test proved this with the wrong variable until #392 was
//! written**, and the correction is worth reading before trusting either.
//! It swept the *amplitude* with a zero alpha `direction` — and
//! `clay_item_add_alpha` documents `direction` as the normal of the stamp's
//! plane, with no all-zeroes fallback of the kind the mesh brush descriptor
//! has. Zeroes give a degenerate plane that does nothing whatever the stroke
//! engine does with the chain, so the test would have gone on passing after
//! the engine fixed the thing it was written to catch. The reading was right
//! and the evidence did not support it. What discriminates is the stamp's
//! *centre*, and the test now sweeps that.

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

/// A single off-centre bump, so a stamp applied per sample and one applied
/// once in a fixed frame do not look alike.
///
/// `rings` cannot tell them apart: it is radially symmetric, so smearing it
/// along a path and repeating it along a path both come out as a ridge.
fn one_bump(dim: usize) -> Vec<f32> {
    (0..dim * dim)
        .map(|i| {
            let x = (i % dim) as f32 / dim as f32 - 0.5;
            let y = (i / dim) as f32 / dim as f32 - 0.5;
            let d = ((x - 0.25).powi(2) + y * y).sqrt();
            (1.0 - d / 0.15).clamp(0.0, 1.0)
        })
        .collect()
}

/// The surface's height above the stroke's path, at five places along it.
fn profile_along(document: &Document) -> Vec<f32> {
    let mesh = document
        .mesh(MeshParams {
            voxel_size: Some(0.01),
            ..Default::default()
        })
        .expect("mesh the document");
    (0..5)
        .map(|i| {
            let at = (i as f32 / 4.0 - 0.5) * 0.8;
            mesh.positions()
                .iter()
                .filter(|p| (p[0] - at).abs() < 0.04 && p[1].abs() < 0.04 && p[2] > 0.0)
                .map(|p| p[2])
                .fold(f32::NEG_INFINITY, f32::max)
        })
        .collect()
}

/// And a stroke does not carry it into each stamp's frame.
///
/// `clay_layer_apply_stroke` documents its item as "the stamp template scaled
/// to each stamp's radius", and `clay_item_add_alpha` puts the stamp's centre,
/// extent and radius in the **item's own space**. So a caller places the alpha
/// on the template and expects it at every stamp. It is not resolved there: the
/// deformer acts where the template's untransformed coordinates put it.
///
/// **This test used to prove that with the wrong variable, and would have gone
/// on passing after the engine fixed it.** It passed a zero `direction`, which
/// `clay_item_add_alpha` documents as the normal of the stamp's plane and says
/// nothing about at zero — the mesh brush descriptor's `alpha_direction` has an
/// all-zeroes fallback and this entry point does not, so zeroes give a
/// degenerate plane that does nothing whatever the stroke engine does with the
/// chain. The reading was right and the evidence did not support it. Filed as
/// CyberdyneCorp/ClayCore#392, both halves.
///
/// What discriminates is the alpha's **centre**, swept through the template's
/// own frame and out past it. Measured on 0.60.0, as the rise of the surface at
/// five points along a stroke from x −0.4 to +0.4, against amplitude zero:
///
/// | centre | rise along the stroke |
/// |---|---|
/// | `[0, 0, 0]`    | +0.0000  +0.0000  +0.0000  +0.0000  +0.0000 |
/// | `[0, 0, 0.2]`  | +0.0000  +0.0000  +0.0000  +0.0000  +0.0000 |
/// | `[0, 0, 0.35]` | +0.0000  +0.0000  +0.0000  +0.0000  +0.0000 |
/// | `[0, 0, 0.7]`  | +0.0082  +0.0071  +0.0070  +0.0020  +0.0079 |
/// | `[0, 0, 1.0]`  | +0.0150  +0.0167  +0.0178  +0.0030  +0.0130 |
///
/// The template's surface is at 0.35 in its own frame, so the first three rows
/// are every sensible place to put a stamp, and they do nothing. The last two
/// are meaningless in that frame and correspond to where the *body's* surface
/// sits in the world — and they lift the whole path roughly evenly rather than
/// leaving the periodic mark a travelling bump would.
///
/// Held as a **ratio** rather than against a fixed number, and measured in the
/// same run: what a stamp placed correctly does, against what one placed at a
/// centre that means nothing in the template's frame does. Adding the deformer
/// changes the item's declared Lipschitz — the header says the bound is derived
/// from the samples — so the marcher shifts the surface a thousandth or two
/// even where the stamp contributes nothing, and an absolute threshold reads
/// that as the stamp. The ratio does not: 0.0016 against 0.0150 is a stamp that
/// is not landing, and if it started landing the two would be comparable.
///
/// So it fails the day the stroke resolves the chain into each stamp.
/// `clayspace_model::AlphaSupport` refuses an alpha on an SDF stroke because of
/// this, and both change together.
#[test]
fn a_stroke_does_not_carry_the_chain_into_each_stamp() {
    // Relief alone, and that is the point rather than a gap. The claim is
    // about the brush a sculptor reaches for to put detail on a field, which
    // is a relief stroke — and it is what `AlphaSupport` refuses. Under
    // `Op::Add` the template *adds a sphere*, which swallows the displacement:
    // measured, no centre produces a mark above the fixture's own noise
    // (0.0016), so the fixture cannot tell a stamp that lands from one that
    // does not and would assert nothing. Carrying an op the fixture cannot
    // measure is how the version of this test before it went wrong.
    {
        let op = Op::Relief;
        // What a stamp centred where the template's frame does *not* reach
        // produces. Not a thing a caller would ask for — it is the frame the
        // deformer is wrongly evaluated in — and it is the scale a correctly
        // placed stamp has to be measured against.
        let misplaced = rise_along_a_stroke(op, [0.0, 0.0, 1.0]);
        assert!(
            misplaced > 5e-3,
            "the fixture produced no mark at any centre ({misplaced:.4}), so it \
             cannot tell a stamp that lands from one that does not"
        );
        // Every centre on or inside the template's own surface, which is the
        // whole of where a caller would put a stamp meant to travel with it.
        for centre in [[0.0f32, 0.0, 0.0], [0.0, 0.0, 0.2], [0.0, 0.0, 0.35]] {
            let placed = rise_along_a_stroke(op, centre);
            assert!(
                placed < misplaced / 5.0,
                "a stroke under {op:?} with its alpha centred at {centre:?} now \
                 moves the surface {placed:.4} against {misplaced:.4} for a \
                 centre outside the template's frame, so the chain is being \
                 resolved into the stamps. That is good news: \
                 clayspace_model::AlphaSupport refuses an alpha on an SDF \
                 stroke because of this, and both should change together — see \
                 CyberdyneCorp/ClayCore#392."
            );
        }
    }
}

/// How far a stroked template's alpha moves the surface along the path.
///
/// The largest rise over five points, against the same stroke with no stamp on
/// it at all.
fn rise_along_a_stroke(op: Op, centre: [f32; 3]) -> f32 {
    let mut profiles = Vec::new();
    {
        {
            for amplitude in [0.0f32, 0.3] {
                let Some((mut document, layer)) = sphere_document() else {
                    return 0.0;
                };
                let mut item = Item::sphere(0.35).expect("a stamp item");
                item.set_op(op).expect("op");
                item.set_blend(Blend::Quadratic, 0.35).expect("blend");
                item.set_rounding(0.35).expect("rounding");
                if amplitude > 0.0 {
                    item.add_alpha(
                        &one_bump(64),
                        64,
                        64,
                        centre,
                        // A real plane normal, not zeroes. See above.
                        [0.0, 0.0, 1.0],
                        [1.0, 0.0, 0.0],
                        0.7,
                        0.35,
                        amplitude,
                        0,
                    )
                    .expect("the wrapper accepts it; the stroke is what drops it");
                }
                let samples: Vec<StrokeSample> = (0..9)
                    .map(|i| {
                        let t = i as f32 / 8.0;
                        StrokeSample {
                            position: [(t - 0.5) * 0.8, 0.0, 1.0],
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
                profiles.push(profile_along(&document));
            }
        }
    }
    profiles[1]
        .iter()
        .zip(&profiles[0])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max)
}
