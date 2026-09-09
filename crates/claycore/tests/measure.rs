//! Every surface measure this crate offers, reached at least once.
//!
//! This file exists because of a defect it is easy to produce and hard to
//! notice: **six measures were declared and one was called.** The bake bridge
//! wanted `Occlusion`, so `Occlusion` was exercised; `Curvature`, `Cavity`,
//! `Convexity`, `NormalDirection` and `Thickness` crossed `unsafe` with a
//! SAFETY comment nobody had checked, and nothing failed, because a variant
//! nobody constructs cannot be wrong.
//!
//! That is the shape ClayCore's own reach tests are written against — "the
//! engine can do it, the host cannot reach it" is a defect every internal
//! test passes through — and it arrives one level down just as easily: a host
//! that binds six entry points and calls one has five unverified bindings and
//! a green suite.
//!
//! The rule this crate already states, in `tests/multires.rs`: a wrapper
//! nobody runs is a SAFETY comment nobody has checked. Stated there in prose
//! about one module. Here it is executed, for these six.

use claycore::{Document, Item, MeasureParams, Op, SurfaceMeasure};

/// A sculpt with somewhere to measure: two overlapping lobes, so the surface
/// has a crevice at the seam, ridges on the outside, and an interior.
///
/// A single sphere would answer every measure with the same number
/// everywhere, which is the fixture that makes a walk look like a check while
/// distinguishing nothing.
fn lobes() -> Option<Document> {
    let mut document = Document::new().ok()?;
    let layer = document.add_sdf_layer("corpo").ok()?;
    for x in [-0.35f32, 0.35] {
        let mut lobe = Item::sphere(0.5).ok()?;
        lobe.set_op(Op::Add).ok()?;
        lobe.set_position([x, 0.0, 0.0]).ok()?;
        document.add_item(layer, &lobe).ok()?;
    }
    Some(document)
}

/// Points **on** the surface, found by sphere-tracing rather than asserted.
///
/// The first version of this named five coordinates and described them in a
/// comment as the crevice, the outer flanks and the interior. Measured, the
/// two "crevice" points were *outside* the material and the two "flanks"
/// *inside* it — the comment was the only thing making them what they
/// claimed to be, and every measure was being asked about somewhere other
/// than where the prose said. A hand-placed probe set is a claim about
/// geometry, and this file exists because of unverified claims.
///
/// So they are traced in: one along +z into the seam, which lands in the
/// concave join, and one along +x onto a lobe's outer flank, which lands on
/// convex ground. Those two must disagree for any measure that distinguishes
/// concave from convex, which is what makes the walk below a check rather
/// than a survey.
fn on_the_surface(document: &Document, direction: [f32; 3]) -> [f32; 3] {
    let mut at: [f32; 3] = std::array::from_fn(|axis| direction[axis] * 3.0);
    for _ in 0..2000 {
        let distance = document
            .eval_points(None, &[at])
            .ok()
            .and_then(|d| d.first().copied())
            .unwrap_or(f32::MAX);
        if distance <= 0.001 {
            break;
        }
        let step = distance.max(0.002);
        at = std::array::from_fn(|axis| at[axis] - direction[axis] * step);
    }
    at
}

/// Every measure returns a value per point, and returns it in range.
///
/// Walked rather than spot-checked, and the loop is over `ALL` rather than a
/// hand-written list, so a measure added to the enum without a call fails on
/// the row that was added.
#[test]
fn every_declared_measure_is_reachable() {
    let Some(document) = lobes() else {
        println!("no engine backend; skipping");
        return;
    };
    // The seam and a flank: concave ground and convex ground, traced rather
    // than named.
    let seam = on_the_surface(&document, [0.0, 0.0, 1.0]);
    let flank = on_the_surface(&document, [1.0, 0.0, 0.0]);
    let crown = on_the_surface(&document, [0.0, 1.0, 0.0]);
    let points = vec![seam, flank, crown];
    println!("  seam {seam:?}  flank {flank:?}  crown {crown:?}");
    let mut answered: Vec<(SurfaceMeasure, Vec<f32>)> = Vec::new();

    for measure in SurfaceMeasure::ALL {
        let params = match measure {
            // The two that cast rays need a length to cast over; the engine's
            // own default is fine for the rest.
            SurfaceMeasure::Occlusion => MeasureParams::occlusion(1.0, 16),
            _ => MeasureParams::default(),
        };
        let values = document
            .measure_points(measure, &points, params)
            .unwrap_or_else(|e| panic!("{measure:?} is declared and refused: {e}"));

        assert_eq!(
            values.len(),
            points.len(),
            "{measure:?} answered {} values for {} points",
            values.len(),
            points.len()
        );
        println!("  {measure:?}: {values:?}");
        for (point, value) in points.iter().zip(&values) {
            assert!(
                value.is_finite(),
                "{measure:?} answered {value} at {point:?}, which is on the \
                 surface — see the degenerate case below for where NaN is \
                 the engine's documented answer"
            );
        }
        answered.push((measure, values));
    }

    // THE TWO-SIDED HALF. Above walks every measure and proves each answers;
    // it does not prove any of them measures anything, and a measure that
    // returned a constant would pass it. These pin the three whose whole
    // meaning is a distinction, against ground chosen to carry it.
    let (seam_i, flank_i, crown_i) = (0usize, 1usize, 2usize);
    let of = |want: SurfaceMeasure| -> &Vec<f32> {
        &answered
            .iter()
            .find(|(measure, _)| *measure == want)
            .expect("the walk above answered every measure")
            .1
    };
    let cavity = of(SurfaceMeasure::Cavity);
    assert!(
        cavity[seam_i] > cavity[flank_i],
        "Cavity read {} in the concave seam and {} on the convex flank, so it \
         is not distinguishing concave from convex",
        cavity[seam_i],
        cavity[flank_i]
    );
    let convexity = of(SurfaceMeasure::Convexity);
    assert!(
        convexity[flank_i] > convexity[seam_i],
        "Convexity read {} on the convex flank and {} in the seam",
        convexity[flank_i],
        convexity[seam_i]
    );
    let occlusion = of(SurfaceMeasure::Occlusion);
    assert!(
        occlusion[seam_i] > occlusion[flank_i],
        "Occlusion read {} in the seam and {} on the open flank; 1 is \
         enclosed and 0 is open sky, so the seam must be the greater",
        occlusion[seam_i],
        occlusion[flank_i]
    );
    // And the one the other two probes cannot express: agreement with +y is
    // what NormalDirection means, so it needs ground that faces up.
    let facing = of(SurfaceMeasure::NormalDirection);
    assert!(
        facing[crown_i] > facing[flank_i],
        "NormalDirection read {} on the upward crown and {} on the sideways \
         flank",
        facing[crown_i],
        facing[flank_i]
    );
}

/// An empty request is answered, not refused.
///
/// `measure_points` returns early for an empty slice without crossing the
/// ABI, so this pins the one path through that function that never reaches
/// the engine — and it is the path a host hits when a selection is empty.
#[test]
fn no_points_is_no_values_and_not_an_error() {
    let Some(document) = lobes() else {
        return;
    };
    let values = document
        .measure_points(SurfaceMeasure::Cavity, &[], MeasureParams::default())
        .expect("an empty measure is not a failure");
    assert!(values.is_empty());
}

/// Where the field's gradient vanishes, `NormalDirection` answers **NaN**.
///
/// Found by this file's first run, at the symmetry centre of two equal lobes:
/// both contribute alike, the gradient is zero, and a normalised zero has no
/// direction. NaN is a defensible answer — "facing up" is genuinely undefined
/// there — and it is pinned here rather than avoided, because a host that
/// measures over a *grid* rather than over surface points will meet it, and a
/// NaN reaching a baker becomes a pixel nobody can explain.
///
/// The other five answer finitely at the same point, so this is specific
/// rather than a general property of interior probes.
#[test]
fn normal_direction_has_no_answer_where_the_gradient_vanishes() {
    let Some(document) = lobes() else {
        return;
    };
    // The midpoint of two equal, opposite lobes.
    let centre = [[0.0f32, 0.0, 0.0]];

    let facing = document
        .measure_points(
            SurfaceMeasure::NormalDirection,
            &centre,
            MeasureParams::default(),
        )
        .expect("the call succeeds; it is the value that is undefined");
    assert!(
        facing[0].is_nan(),
        "NormalDirection answered {} at the symmetry centre. If the engine has \
         started returning a defined value there, that is an improvement — \
         update this and drop whatever guard the host carries for it",
        facing[0]
    );

    for measure in SurfaceMeasure::ALL {
        if measure == SurfaceMeasure::NormalDirection {
            continue;
        }
        let params = match measure {
            SurfaceMeasure::Occlusion => MeasureParams::occlusion(1.0, 16),
            _ => MeasureParams::default(),
        };
        let values = document
            .measure_points(measure, &centre, params)
            .unwrap_or_else(|e| panic!("{measure:?}: {e}"));
        assert!(
            values[0].is_finite(),
            "{measure:?} also answered {} at the symmetry centre, so the NaN \
             above is not specific to NormalDirection and this test names the \
             wrong property",
            values[0]
        );
    }
}
