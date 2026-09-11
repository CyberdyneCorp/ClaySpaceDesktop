//! Which mechanism is costing the marcher, and the answer a host acts on.
//!
//! This file exists because of a field that was in the ABI and not in the
//! wrapper. `clay_field_report` carries `degradation`, the engine's own
//! statement of *which* thing has gone wrong and therefore which cure applies,
//! and the header's instruction about it is explicit: **"READ `degradation`
//! BEFORE ACTING."** The host read `advises_consolidation` and nothing else.
//!
//! That is worse than it sounds, because `advises_consolidation` is keyed on
//! the mechanism rather than on the number. It is deliberately **false** for a
//! layer whose degradation is all deformer chain — there the bake is a
//! straight loss, measured 6x worse — so a host reading only the advisory sees
//! the same `false` for a healthy layer and for one whose step scale has
//! collapsed by three orders of magnitude. One of those wants nothing done and
//! the other wants something done that consolidation is not.
//!
//! A session of Move dabs builds exactly that layer: one grab per dab per
//! mirror image, and the safe step scale is a product over the chain. The
//! engine's own note on the call says so — "each drag appends a grab to the
//! deformer chain, and those multiply, so the safe step scale decays by a
//! constant factor per drag — 79x the marching cost by nine".

use claycore::{Degradation, Document, Item, LayerId, MoveParams};

fn sphere() -> Option<(Document, LayerId)> {
    let mut doc = Document::new().ok()?;
    let layer = doc.add_sdf_layer("Base").ok()?;
    doc.add_item(layer, &Item::sphere(1.0).ok()?).ok()?;
    Some((doc, layer))
}

fn dab(doc: &mut Document, layer: LayerId, index: usize) {
    // Somewhere new each time: dabs repeating a centre and radius exactly are
    // folded into one grab by the engine, and would build no chain at all.
    let a = index as f32 * 0.7;
    let at = [0.95 * a.cos(), 0.95 * a.sin(), 0.0];
    doc.move_surface(
        layer,
        at,
        [0.05, 0.02, 0.0],
        MoveParams {
            radius: 0.35,
            ease: 0,
            front_only: true,
        },
    )
    .expect("a dab");
}

/// A chain of grabs reports `Deformers`, and declines to advise a bake.
///
/// Both halves matter. The first is the signal the host was throwing away;
/// the second is the reason throwing it away was not obviously wrong — the
/// advisory really does say `false` here, and it is right to.
#[test]
fn a_session_of_dabs_degrades_by_deformers_and_is_offered_no_cure() {
    let Some((mut doc, layer)) = sphere() else {
        println!("no engine backend; skipping");
        return;
    };
    // 0.5 is the tolerance the application passes.
    let fresh = doc.field_report(layer, 0.5).expect("a report");
    assert_eq!(
        fresh.degradation,
        Degradation::None,
        "a sphere with no edits is not degraded"
    );

    for index in 1..=8 {
        dab(&mut doc, layer, index);
    }
    let after = doc.field_report(layer, 0.5).expect("a report");

    assert!(
        after.longest_deformer_chain >= 8,
        "eight dabs in eight places should leave a grab apiece, not {}",
        after.longest_deformer_chain
    );
    assert_eq!(
        after.degradation,
        Degradation::Deformers,
        "a chain of grabs on a layer with no volumes to absorb is the \
         deformer case, and naming it is the whole point of the field"
    );
    assert!(
        !after.advises_consolidation,
        "the engine must NOT advise a bake here: it would swap a cheap \
         analytic item for a dense volume and measure 6x worse. If this ever \
         becomes true the engine has changed its mind and the host's reading \
         of it has to be revisited"
    );
    assert!(
        !after.degradation.consolidation_would_help(),
        "and the wrapper must agree with the advisory about why"
    );
    assert!(
        after.safe_step_scale < fresh.safe_step_scale,
        "the marcher is paying for the chain: {} against {} before the dabs",
        after.safe_step_scale,
        fresh.safe_step_scale
    );
}

/// The other mechanism still recommends the cure that fits it.
///
/// Without this the test above would pass against a wrapper that reported
/// `Deformers` unconditionally, and the distinction the field exists to make
/// would go unchecked.
#[test]
fn the_two_mechanisms_do_not_report_the_same_thing() {
    let Some((mut doc, layer)) = sphere() else {
        return;
    };
    for index in 1..=8 {
        dab(&mut doc, layer, index);
    }
    let deformers = doc.field_report(layer, 0.5).expect("a report").degradation;

    // A tolerance nothing can satisfy, which is how a caller asks "is this
    // layer perfect?" — the answer moves off `None` for the same layer, so the
    // field is reading the request and not a constant.
    let strict = doc.field_report(layer, 1.0).expect("a report");
    assert_ne!(
        strict.degradation,
        Degradation::None,
        "at a tolerance of 1.0 a dabbed layer cannot be undegraded"
    );
    assert_eq!(
        deformers,
        Degradation::Deformers,
        "and the mechanism does not change with the tolerance, only whether \
         it is reported at all"
    );
}

/// An enumerator this build has not been taught is carried, not flattened.
///
/// `Degradation::None` is the one answer that must never be invented: a host
/// that treats an unknown mechanism as "nothing is wrong" acts on a layer it
/// has been told is degraded. The same rule the error table already follows
/// for a result code from the future.
#[test]
fn a_mechanism_from_the_future_is_not_reported_as_health() {
    let unknown = Degradation::Unknown(99);
    assert_ne!(unknown, Degradation::None);
    assert!(
        !unknown.consolidation_would_help(),
        "and it must not claim a cure it knows nothing about"
    );
}
