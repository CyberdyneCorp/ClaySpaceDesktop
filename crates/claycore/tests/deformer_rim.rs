//! A grab's field *inside* its own ball, and at the rim in particular.
//!
//! Every deformer claim this repository makes is measured at the surface. The
//! strongest of them, in `tests/live_transactions.rs`, finds the surface by
//! **bisection** along an axis — so by construction it samples the zero
//! crossing and nowhere else. `alpha_deformer.rs` never touches the field at
//! all; it filters mesh vertices. A kernel wrong inside the falloff ball but
//! right at the isosurface passes all of it.
//!
//! The rim is the specific blind spot. A grab's observations here run along
//! the axis through the middle of the ball, where the effect is largest and
//! easiest to assert — and a kernel that shaved the *edge* of the falloff
//! while preserving finite support would leave that axis untouched. ClayCore
//! broke `cregion_weight` in exactly that way as an experiment: 132,011
//! assertions in its own deformer suite passed a kernel wrong by 2.4% inside
//! the rim, because those goldens only probed outside the ball.
//!
//! So this samples the field across the whole radius — centre, mid, rim,
//! outside — and pins the shape rather than one point of it.

use claycore::{Document, Item, LayerId, MoveParams};

/// A sphere and a grab of known centre and radius.
const CENTRE: [f32; 3] = [1.0, 0.0, 0.0];
const RADIUS: f32 = 0.5;
const PULL: [f32; 3] = [0.12, 0.0, 0.0];

fn sphere() -> Option<(Document, LayerId)> {
    let mut doc = Document::new().ok()?;
    let layer = doc.add_sdf_layer("Base").ok()?;
    doc.add_item(layer, &Item::sphere(1.0).ok()?).ok()?;
    Some((doc, layer))
}

/// The field at a set of points, before and after the grab.
///
/// The difference is what the deformer did — measuring the field alone would
/// pin the sphere, not the kernel.
fn shifted_by_the_grab(offsets: &[f32]) -> Option<Vec<f32>> {
    let (mut doc, layer) = sphere()?;
    // Probes along +y from the grab's centre, so they cross the falloff at a
    // known fraction of its radius and stay off the axis the pull runs along.
    let points: Vec<[f32; 3]> = offsets
        .iter()
        .map(|f| [CENTRE[0], CENTRE[1] + f * RADIUS, CENTRE[2]])
        .collect();
    let before = doc.eval_points(None, &points).ok()?;
    doc.move_surface(
        layer,
        CENTRE,
        PULL,
        MoveParams {
            radius: RADIUS,
            ease: 0,
            front_only: false,
        },
    )
    .ok()?;
    let after = doc.eval_points(None, &points).ok()?;
    Some(before.iter().zip(&after).map(|(b, a)| a - b).collect())
}

/// The grab's reach falls off across its ball and stops at the rim.
///
/// Four claims, and the last two are the ones no existing test makes:
/// something happens at the middle of the radius, something *smaller* happens
/// near the rim, and nothing at all happens outside it. A kernel that shaved
/// the rim keeps the first and loses the second.
#[test]
fn a_grab_falls_off_across_its_ball_and_stops_at_the_rim() {
    // 0.0 is the centre, 1.0 the rim, 1.2 outside it.
    let offsets = [0.0f32, 0.25, 0.5, 0.75, 0.9, 1.0, 1.2];
    let Some(shift) = shifted_by_the_grab(&offsets) else {
        println!("no engine backend; skipping");
        return;
    };
    for (f, d) in offsets.iter().zip(&shift) {
        println!("  {f:>5.2} of the radius: field moved {d:+.6}");
    }

    let at = |f: f32| shift[offsets.iter().position(|o| *o == f).expect("an offset")];

    assert!(
        at(0.0).abs() > 1e-4,
        "the centre of the grab did not move the field at all: {}",
        at(0.0)
    );
    // INSIDE THE BALL, not at the surface and not at the centre. This is the
    // sample no other test in this repository takes.
    assert!(
        at(0.5).abs() > 1e-5,
        "half way out along the radius the field did not move: {}. A kernel \
         that only acted near the centre would still pass every surface test \
         here",
        at(0.5)
    );
    // THE RIM. Falling off means the far end of the radius moves less than the
    // middle of it — the property a rim-shaving kernel destroys while leaving
    // finite support intact.
    assert!(
        at(0.9).abs() < at(0.5).abs(),
        "the field moved by {} at 0.9 of the radius and {} at 0.5. A falloff \
         that does not fall off toward its edge is not a falloff",
        at(0.9),
        at(0.5)
    );
    // And support really is finite.
    assert!(
        at(1.2).abs() < 1e-6,
        "the field moved by {} at 1.2 of the radius, outside the ball the \
         grab was given",
        at(1.2)
    );
}

/// The falloff is monotone from the centre outward.
///
/// Stronger than "the rim is smaller than the middle", and it is what a
/// shaved, dented or truncated kernel breaks. Asserted with a tolerance,
/// because the engine is free to choose the curve and not free to make it
/// wander.
#[test]
fn the_falloff_does_not_wander_on_its_way_out() {
    let offsets = [0.0f32, 0.2, 0.4, 0.6, 0.8, 1.0];
    let Some(shift) = shifted_by_the_grab(&offsets) else {
        return;
    };
    let magnitude: Vec<f32> = shift.iter().map(|d| d.abs()).collect();
    for pair in magnitude.windows(2) {
        assert!(
            pair[1] <= pair[0] + 1e-6,
            "the grab's effect grew on the way out: {:?} across {offsets:?}",
            magnitude
        );
    }
    assert!(
        magnitude[0] > magnitude[magnitude.len() - 1],
        "the centre and the rim moved the field equally, so there is no \
         falloff to speak of: {magnitude:?}"
    );
}

/// The shape of the falloff, pinned — which is the assertion that actually
/// catches a shaved rim.
///
/// The two tests above pin *ordering*, and ordering is not enough. A kernel
/// that shaves the edge of the ball makes the rim move **less**, which leaves
/// "the rim is smaller than the middle" true and "the falloff is monotone"
/// true. ClayCore's own experiment was exactly that: 2.4% wrong inside the
/// rim, finite support preserved, 132,011 assertions green. Only a test that
/// knows what the curve is supposed to *be* can see it.
///
/// Normalised against the centre rather than pinned in world units, so the
/// comparison is the falloff's shape and not the pull's magnitude — a backend
/// that disagreed in the last bits about absolute displacement would still
/// have to agree about the shape.
///
/// **The error is RELATIVE to each expected value, and that is the whole
/// design.** An absolute tolerance on the ratio is blind where it matters: at
/// 0.9 of the radius the shape is 0.09, so shaving that value by 2.4% moves
/// the ratio by 0.002 and any absolute threshold loose enough to survive a
/// backend would sail past it. Relative error sees the same shave as 2.4%
/// wherever on the curve it happens, which is the point — the rim is small,
/// and small is where a shave hides.
///
/// 1.5%, tighter than the 2.4% the experiment introduced. If this ever proves
/// flaky on an accelerated row the fix is MORE probes, not a looser bound:
/// past about 2% this stops being able to see the defect it exists for.
#[test]
fn the_falloff_has_the_shape_it_is_supposed_to_have() {
    let offsets = [0.0f32, 0.25, 0.5, 0.75, 0.9];
    // Measured on the pinned engine, CPU backend, normalised to the centre.
    let expected = [1.0f32, 0.7436, 0.4842, 0.2336, 0.0911];
    let Some(shift) = shifted_by_the_grab(&offsets) else {
        return;
    };
    let centre = shift[0].abs();
    assert!(centre > 1e-4, "nothing to normalise against: {centre}");

    for ((f, measured), want) in offsets.iter().zip(&shift).zip(&expected) {
        let ratio = measured.abs() / centre;
        // Relative to what is expected there, so a shave reads the same size
        // at the rim as at the centre.
        let drift = if *want > 0.0 {
            (ratio - want).abs() / want
        } else {
            (ratio - want).abs()
        };
        println!(
            "  {f:>5.2}: shape {ratio:.4}, expected {want:.4}, off by {:.2}%",
            drift * 100.0
        );
        assert!(
            drift < 0.015,
            "at {f} of the radius the falloff is {ratio:.4} of its centre \
             value where it should be {want:.4} — off by {:.2}%. The curve has \
             changed shape, which is what a shaved rim looks like and what no \
             ordering assertion can see",
            drift * 100.0
        );
    }
}
