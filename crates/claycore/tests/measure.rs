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

/// What each measure is asked for, in one place.
///
/// Three tests ask, and a measure whose ray length differs between them is
/// three different questions wearing one name — the seam reads 0.717 thick at
/// a ray length of 1.0 and saturates at 0.5, so the number a test compares
/// against is a property of this function, not of the engine.
fn params_for(measure: SurfaceMeasure) -> MeasureParams {
    match measure {
        // The two that cast rays need a length to cast over; the engine's own
        // default is fine for the rest. Both lengths are stated rather than
        // inherited, because what the half below pins about `Thickness` is a
        // distinction *within* a ray length: at 1.0 this fixture's waist
        // resolves and its long axis saturates, and an engine that changed its
        // own default would move that line without touching this file.
        SurfaceMeasure::Occlusion => MeasureParams::occlusion(1.0, 16),
        SurfaceMeasure::Thickness => MeasureParams {
            ray_length: Some(1.0),
            ..MeasureParams::default()
        },
        _ => MeasureParams::default(),
    }
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
        let values = document
            .measure_points(measure, &points, params_for(measure))
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
    // returned a constant would pass it. These pin each of the six against
    // ground chosen to carry it: five against the seam and the flank, and the
    // sixth against ground that faces up.
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
    // Curvature is the unsigned one — the magnitude of the bend, whichever
    // way the surface bends — so it takes two claims rather than one. It must
    // read the seam above the flank, like the three above; and it must read
    // *something* on the flank, where `Cavity` reads nothing at all. A
    // `Curvature` that answered only on concave ground would be `Cavity` under
    // another name, and the first claim alone would not catch it.
    let curvature = of(SurfaceMeasure::Curvature);
    assert!(
        curvature[seam_i] > curvature[flank_i],
        "Curvature read {} in the seam and {} on the flank, so it is not \
         distinguishing bent ground from smooth",
        curvature[seam_i],
        curvature[flank_i]
    );
    assert!(
        curvature[flank_i] > cavity[flank_i],
        "Curvature read {} on the convex flank, where Cavity read {}. \
         Curvature is the bend either way; if it answers only where Cavity \
         answers, it is measuring the concave half and is misnamed",
        curvature[flank_i],
        cavity[flank_i]
    );

    // Thickness asks what is behind the surface, so the ground that carries it
    // is not concave against convex but thin against deep. Inward from the
    // flank is the long axis of the whole body, longer than the ray length set
    // above and so saturated; inward from the seam is the narrow waist where
    // the lobes cross, which the same ray length resolves.
    let thickness = of(SurfaceMeasure::Thickness);
    assert!(
        thickness[flank_i] > thickness[seam_i],
        "Thickness read {} inward from the flank, down the long axis of the \
         body, and {} through the narrow waist at the seam. The flank is the \
         deeper of the two; if these agree, thickness is not reading depth",
        thickness[flank_i],
        thickness[seam_i]
    );

    // And the one the other two probes cannot express: agreement with +y is
    // what NormalDirection means, so it needs ground that faces up. It is the
    // only measure here that is not a property of the shape at a point but a
    // comparison against a direction, which is the trap in extending this
    // pattern — it is the one that will *not* separate two shapes, because
    // two shapes present much the same normals at the same place.
    let facing = of(SurfaceMeasure::NormalDirection);
    assert!(
        facing[crown_i] > facing[flank_i],
        "NormalDirection read {} on the upward crown and {} on the sideways \
         flank",
        facing[crown_i],
        facing[flank_i]
    );
}

/// No two measures are the same measure.
///
/// The two-sided half above pins each measure against ground chosen for it,
/// which asks every measure about itself and never asks whether it is
/// *distinct* from the other five. That gap is not hypothetical: a `Curvature`
/// wired to the engine's `CAVITY` still reads the seam above the flank and
/// still passes that claim. It is caught above only by the second `Curvature`
/// claim, and only because someone thought to write that claim.
///
/// This catches the swap nobody thought of. Six bindings cross the ABI as six
/// enumerators, and any two of them agreeing at every point is one binding
/// pointing at the other's value.
///
/// Agreeing at *a* point is legitimate and expected — `Cavity` and `Curvature`
/// both read 1 in the seam, because a crevice is bent and concave at once, and
/// asserting otherwise would be asserting a coincidence. Agreeing at *every*
/// point is the defect.
///
/// ClayCore's own reach test had this same hole on the C surface and closed it
/// the same way (`3ed6ff11`). Neither check subsumes the other: this one
/// catches a swap no claim was written for, and the claims above catch a
/// measure that is wrong while still being distinct from all five others.
///
/// What it does **not** catch is a *mutual* swap. Two bindings exchanged with
/// each other leave all six values distinct, so nothing here fires. Measured,
/// not supposed: exchanging `Cavity` and `Occlusion` in `to_raw` passes this
/// test and every claim above, because both read the seam above the flank and
/// the claims say only that. `only_occlusion_is_sampled` below closes that
/// particular pair on a property no other measure can imitate.
#[test]
fn no_two_measures_are_the_same_measure() {
    let Some(document) = lobes() else {
        return;
    };
    let points = [
        on_the_surface(&document, [0.0, 0.0, 1.0]),
        on_the_surface(&document, [1.0, 0.0, 0.0]),
        on_the_surface(&document, [0.0, 1.0, 0.0]),
    ];
    let answered: Vec<(SurfaceMeasure, Vec<f32>)> = SurfaceMeasure::ALL
        .into_iter()
        .map(|measure| {
            let values = document
                .measure_points(measure, &points, params_for(measure))
                .unwrap_or_else(|e| panic!("{measure:?} is declared and refused: {e}"));
            (measure, values)
        })
        .collect();

    for (index, (left, left_values)) in answered.iter().enumerate() {
        for (right, right_values) in &answered[index + 1..] {
            let apart = left_values
                .iter()
                .zip(right_values)
                .map(|(l, r)| (l - r).abs())
                .fold(0.0f32, f32::max);
            println!("  {left:?} vs {right:?}: {apart}");
            // Identical, not merely close: the threshold says "these are the
            // same number", it does not assert a minimum separation the
            // measures never promised. The tightest real pair on this fixture
            // is Curvature against Cavity at 0.2 — which is the swap that
            // matters — so there is room to spare.
            assert!(
                apart > 1e-6,
                "{left:?} and {right:?} answered the same value at all {} \
                 points, the widest disagreement between them being {apart}. \
                 Two measures cannot be one measure; the likeliest cause is a \
                 binding in `SurfaceMeasure::to_raw` pointing at the other's \
                 enumerator",
                points.len()
            );
        }
    }
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
        let values = document
            .measure_points(measure, &centre, params_for(measure))
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

/// Exactly one of the six is a sampled measure, and it is `Occlusion`.
///
/// This exists for a swap the two checks above both miss. Exchanging `Cavity`
/// and `Occlusion` with each other in `to_raw` leaves six distinct values, and
/// satisfies every claim in the two-sided half, because the only thing those
/// claims say about either is "the seam reads above the flank" — which is true
/// of both. Two measures that agree about the shape of the ground are
/// interchangeable to any test that only asks about the ground.
///
/// So this asks about the *measure* instead. `Occlusion` is the blocked
/// fraction of a hemisphere, estimated from `ray_count` samples: change the
/// sample count and the estimate changes. The other five are analytic — they
/// read the field and its derivatives, and cannot depend on how many rays
/// nobody cast. That is a property of what each measure *is*, so no rearranged
/// binding satisfies it.
///
/// Nothing here asserts which way the estimate moves. It is a sampling
/// estimator converging, not a monotone function of the count.
#[test]
fn only_occlusion_is_sampled() {
    let Some(document) = lobes() else {
        return;
    };
    let points = [
        on_the_surface(&document, [0.0, 0.0, 1.0]),
        on_the_surface(&document, [1.0, 0.0, 0.0]),
        on_the_surface(&document, [0.0, 1.0, 0.0]),
    ];
    // One axis moves and everything else is held, including the seed — the
    // hemisphere pattern is rotated by a hash of the point and the seed, so a
    // fixed seed makes this the same bits on every backend and every run.
    let with_rays = |count: i32, measure: SurfaceMeasure| -> Vec<f32> {
        let params = MeasureParams {
            ray_length: Some(1.0),
            ray_count: Some(count),
            ..MeasureParams::default()
        };
        document
            .measure_points(measure, &points, params)
            .unwrap_or_else(|e| panic!("{measure:?} is declared and refused: {e}"))
    };

    for measure in SurfaceMeasure::ALL {
        let few = with_rays(16, measure);
        let many = with_rays(64, measure);
        let moved = few.iter().zip(&many).any(|(a, b)| a != b);
        println!("  {measure:?}: 16 rays {few:?}  64 rays {many:?}  moved {moved}");

        if measure == SurfaceMeasure::Occlusion {
            assert!(
                moved,
                "Occlusion read the same values from 16 rays and 64: {few:?}. \
                 It is the hemisphere estimate, so more samples must change it \
                 somewhere. If the engine has made it exact, that is an \
                 improvement — this test then names the wrong property and \
                 the Cavity/Occlusion swap it guards needs another guard"
            );
        } else {
            assert!(
                !moved,
                "{measure:?} answered {few:?} from 16 rays and {many:?} from \
                 64. Only Occlusion samples a hemisphere; a measure that reads \
                 the field itself cannot depend on a ray count, so this one is \
                 bound to the wrong enumerator"
            );
        }
    }
}
