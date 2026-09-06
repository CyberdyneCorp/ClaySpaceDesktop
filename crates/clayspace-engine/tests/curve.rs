//! A curve placed with control points, and the tube swept along it.
//!
//! Nomad calls it a Tube, 3DCoat a spline. What makes it different from a
//! brush is not the shape it leaves but that it can be **gone back to**: a
//! stroke is over when the pointer comes up, and a curve is a set of points
//! that stay where they were put.
//!
//! Every piece of it was already in the engine — `CLAY_PRIM_SWEPT` carries a
//! profile along a guide, `clay_item_add_loft_profile` supplies the profiles,
//! `clay_item_set_curve_points` types each point, and
//! `clay_layer_set_stroke_points` edits a placed guide undoably. What was
//! missing was a tool that placed one.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{CurveJoin, CurveModel, CurveProfile, SculptModel};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// A curve laid across the front of the form, clear of it.
fn lay(document: &mut ClayDocument) {
    document.begin_curve();
    for (at, radius) in [
        ([-0.9f32, 1.4, 0.0], 0.12f32),
        ([0.0, 1.7, 0.0], 0.16),
        ([0.9, 1.4, 0.0], 0.10),
    ] {
        document
            .add_curve_point(at, radius)
            .expect("the point was refused");
    }
}

/// How far the form reaches along a direction.
fn reach(document: &ClayDocument, direction: [f32; 3]) -> f32 {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    SculptModel::pick(document, unit.map(|c| c * 6.0), unit.map(|c| -c))
        .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
        .unwrap_or(0.0)
}

#[test]
fn a_curve_sweeps_a_tube_once_it_has_two_points() {
    // One point is a point, and the engine refuses to sweep along it: cutting
    // a guide below two "would leave the sweep with nothing to follow".
    let mut document = document();
    let bare = reach(&document, [0.0, 1.0, 0.0]);

    document.begin_curve();
    document
        .add_curve_point([-0.9, 1.4, 0.0], 0.12)
        .expect("refused");
    assert!(!document.curve().can_be_swept());
    assert!(
        (reach(&document, [0.0, 1.0, 0.0]) - bare).abs() < 1e-3,
        "one point swept something"
    );

    document
        .add_curve_point([0.9, 1.4, 0.0], 0.12)
        .expect("refused");
    assert!(document.curve().can_be_swept());
    assert!(
        reach(&document, [0.0, 1.0, 0.0]) > bare + 0.2,
        "two points swept nothing"
    );
}

#[test]
fn dragging_a_control_point_moves_the_tube() {
    // The whole of what a curve is for: it can be gone back to.
    let mut document = document();
    lay(&mut document);
    let before = reach(&document, [0.0, 1.0, 0.0]);

    // The middle point, lifted.
    document.select_curve_point(Some(1));
    document.drag_curve([0.0, 0.5, 0.0]).expect("refused");

    let after = reach(&document, [0.0, 1.0, 0.0]);
    assert!(
        after > before + 0.3,
        "the tube reached {after} from {before} after its middle point was \
         lifted by 0.5"
    );
}

#[test]
fn editing_replaces_the_sweep_rather_than_adding_another() {
    // A curve dragged across the viewport would otherwise leave a sweep behind
    // on every move — the same fault the snakehook had, and the same fix.
    let mut document = document();
    lay(&mut document);
    let placed = SculptModel::stats(&document).objects;

    document.select_curve_point(Some(1));
    for _ in 0..8 {
        document.drag_curve([0.0, 0.05, 0.0]).expect("refused");
    }
    assert_eq!(
        SculptModel::stats(&document).objects,
        placed,
        "eight drags left eight sweeps behind"
    );
}

#[test]
fn the_radius_is_per_point_so_a_tube_can_taper() {
    let mut document = document();
    lay(&mut document);
    let curve = document.curve();
    assert_eq!(curve.points.len(), 3);
    let radii: Vec<f32> = curve.points.iter().map(|p| p.radius).collect();
    assert!(
        radii[0] != radii[1] || radii[1] != radii[2],
        "the guide carries one radius for the whole tube: {radii:?}"
    );

    // Setting it with nothing picked reaches the whole tube; with a point
    // picked it reaches that point.
    document.select_curve_point(None);
    document.set_curve_radius(0.2).expect("refused");
    assert!(document
        .curve()
        .points
        .iter()
        .all(|p| (p.radius - 0.2).abs() < 1e-6));

    document.select_curve_point(Some(2));
    document.set_curve_radius(0.05).expect("refused");
    let radii: Vec<f32> = document.curve().points.iter().map(|p| p.radius).collect();
    assert!(
        (radii[0] - 0.2).abs() < 1e-6 && (radii[2] - 0.05).abs() < 1e-6,
        "{radii:?}"
    );
}

#[test]
fn the_join_and_the_profile_change_the_form() {
    let mut document = document();
    lay(&mut document);
    let through = reach(&document, [0.0, 1.0, 0.0]);

    // A B-spline approximates rather than interpolates, so the curve sits
    // inside its own points and the tube comes down.
    document
        .set_curve_join(CurveJoin::Rounded)
        .expect("refused");
    let rounded = reach(&document, [0.0, 1.0, 0.0]);
    assert!(
        rounded < through - 0.01,
        "rounding the join left the tube at {rounded} from {through}; a \
         B-spline does not pass through its points"
    );

    // And a square section is wider across its diagonal than a circle of the
    // same radius, so the form changes measurably.
    document
        .set_curve_join(CurveJoin::Through)
        .expect("refused");
    let circle = reach(&document, [0.0, 1.0, 0.0]);
    document
        .set_curve_profile(CurveProfile::Square)
        .expect("refused");
    let square = reach(&document, [0.0, 1.0, 0.0]);
    assert!(
        (square - circle).abs() > 1e-3,
        "the profile made no difference: {circle} against {square}"
    );
}

#[test]
fn abandoning_a_curve_takes_its_tube_with_it() {
    let mut document = document();
    let bare = reach(&document, [0.0, 1.0, 0.0]);
    lay(&mut document);
    assert!(reach(&document, [0.0, 1.0, 0.0]) > bare + 0.2);

    document.cancel_curve();
    assert!(!document.curve().active);
    assert!(
        (reach(&document, [0.0, 1.0, 0.0]) - bare).abs() < 1e-3,
        "abandoning the curve left its tube behind"
    );
}

#[test]
fn applying_a_curve_leaves_the_tube_and_takes_the_points_down() {
    let mut document = document();
    lay(&mut document);
    let swept = reach(&document, [0.0, 1.0, 0.0]);

    document.apply_curve().expect("refused");
    assert!(!document.curve().active, "the curve stayed up");
    assert!(
        (reach(&document, [0.0, 1.0, 0.0]) - swept).abs() < 1e-3,
        "applying the curve changed the form it had already swept"
    );
}

#[test]
fn removing_the_last_points_takes_the_tube_down() {
    // A guide below two points has nothing to sweep along, and the engine
    // refuses to cut one there rather than ignoring it. Taking the sweep down
    // is the honest answer while the curve is still being placed.
    let mut document = document();
    let bare = reach(&document, [0.0, 1.0, 0.0]);
    lay(&mut document);

    document.select_curve_point(Some(0));
    document.toggle_curve_point(1);
    document.remove_curve_points().expect("refused");

    assert_eq!(document.curve().points.len(), 1);
    assert!(document.curve().active, "the curve itself went too");
    assert!(
        (reach(&document, [0.0, 1.0, 0.0]) - bare).abs() < 1e-3,
        "one point still swept a tube"
    );
}

#[test]
fn a_round_tube_takes_its_thickness_from_every_point() {
    // A round tube is a swept-sphere chain, which carries a radius *per
    // point*. The swept primitive does not: measured, a tube swept along the
    // same guide with radii of 0.05, 0.15 and 0.4 reached 2.901 every time —
    // the unit circle's size — because its thickness comes from the profile
    // parameters instead. That is why the two sections use two primitives.
    let mut thin = document();
    let mut thick = document();
    for (document, radius) in [(&mut thin, 0.06f32), (&mut thick, 0.24)] {
        document.begin_curve();
        for at in [[-0.9f32, 1.4, 0.0], [0.0, 1.7, 0.0], [0.9, 1.4, 0.0]] {
            document.add_curve_point(at, radius).expect("refused");
        }
    }
    let (slim, stout) = (
        reach(&thin, [0.0, 1.0, 0.0]),
        reach(&thick, [0.0, 1.0, 0.0]),
    );
    assert!(
        stout > slim + 0.1,
        "a 0.24 tube reached {stout} and a 0.06 one {slim}; the thickness \
         does not reach the form"
    );

    // And it tapers: thick at one end, thin at the other.
    // Wholly on one side of x. The starting form carries an X layer mirror,
    // and a curve is an *item* — so a tube laid across the plane is reflected
    // onto itself and comes out symmetric whatever its radii do. Measured that
    // way, both ends read 0.37354326 to the last digit, which is the mirror
    // rather than the taper.
    let mut tapered = document();
    tapered.begin_curve();
    for (at, radius) in [
        ([0.35f32, 1.5, 0.0], 0.24f32),
        ([0.95, 1.7, 0.0], 0.16),
        ([1.55, 1.5, 0.0], 0.05),
    ] {
        tapered.add_curve_point(at, radius).expect("refused");
    }
    // How far the tube's surface stands off its own guide at each end. Not
    // `reach`, which measures from the origin: the two ends are the same
    // distance from it whatever the tube is doing, so it cannot see a taper.
    let across = |document: &ClayDocument, at: [f32; 3]| {
        SculptModel::pick(document, [at[0], at[1], 4.0], [0.0, 0.0, -1.0])
            .map(|hit| hit[2])
            .unwrap_or(0.0)
    };
    // Inside the span rather than at the very ends, where the chain's
    // spherical cap sits and reads the same whatever the taper does.
    let root = across(&tapered, [0.55, 1.6, 0.0]);
    let tip = across(&tapered, [1.35, 1.6, 0.0]);
    assert!(
        root > tip + 0.05,
        "the tube stands {root} off its guide at the thick end and {tip} at \
         the thin one, which is not a taper"
    );
}

#[test]
fn a_sectioned_tube_takes_its_thickness_from_its_ends() {
    // The swept primitive ignores the guide's radius, so the thickness is the
    // first point's at one end and the last point's at the other, interpolated
    // between. Stated rather than left as a surprise: it is a taper and not a
    // radius per point, and the interface offers the same control for both.
    let mut thin = document();
    let mut thick = document();
    for (document, radius) in [(&mut thin, 0.06f32), (&mut thick, 0.24)] {
        document.begin_curve();
        for at in [[-0.9f32, 1.4, 0.0], [0.0, 1.7, 0.0], [0.9, 1.4, 0.0]] {
            document.add_curve_point(at, radius).expect("refused");
        }
        document
            .set_curve_profile(CurveProfile::Square)
            .expect("refused");
    }
    let (slim, stout) = (
        reach(&thin, [0.0, 1.0, 0.0]),
        reach(&thick, [0.0, 1.0, 0.0]),
    );
    assert!(
        stout > slim + 0.1,
        "a 0.24 square tube reached {stout} and a 0.06 one {slim}; the \
         thickness does not reach the profile"
    );
}

#[test]
fn a_curve_on_a_mirrored_layer_comes_out_mirrored() {
    // Worth writing down rather than meeting by surprise: a curve is an
    // *item*, and the layer mirror reflects a layer's items. The starting form
    // carries an X mirror, so a tube placed on one side appears on both.
    //
    // That is the mirror doing what a mirror does, and it is what a sculptor
    // asking for symmetry wants — but it means a tube laid *across* the plane
    // is folded onto itself, which is how a tapered one reads as symmetric.
    let mut document = document();
    let bare = reach(&document, [-1.0, 1.3, 0.0]);
    document.begin_curve();
    for at in [[0.35f32, 1.5, 0.0], [0.95, 1.7, 0.0], [1.55, 1.5, 0.0]] {
        document.add_curve_point(at, 0.16).expect("refused");
    }

    let here = reach(&document, [1.0, 1.3, 0.0]);
    let there = reach(&document, [-1.0, 1.3, 0.0]);
    assert!(here > 1.0, "the tube was not placed at all");
    assert!(
        there > bare + 0.1,
        "the tube reached {here} where it was placed and left the far side at \
         {there}; the layer's mirror should have carried it across"
    );
}

/// The guide the viewport draws has to be the line the tube actually follows.
///
/// The overlay used to draw the control polygon — straight chords between the
/// points — which is a *different line* from the swept guide under `Through`
/// and `Rounded`. A sculptor looking at it saw a chain that cut the corners
/// the tube rounds, so the one line they could see was the one the tube does
/// not take.
///
/// `CurveState::path` is the interface's own tessellation, and there is no ABI
/// call that hands back a swept guide's, so agreement is something to measure
/// rather than assume. This measures the property itself rather than a proxy:
/// every sample of the drawn guide is evaluated against the swept field and
/// has to be **inside** the tube. A tessellation that disagreed with the
/// engine's would put samples outside it.
#[test]
fn the_guide_lies_inside_the_tube_it_describes() {
    /// Thin against the bend below, so a line that cuts the corner leaves it.
    const RADIUS: f32 = 0.1;

    for join in CurveJoin::ALL {
        let mut document = document();
        document.begin_curve();
        // Bent hard in two planes, so a wrong tessellation has somewhere to go
        // wrong: a gentle curve is close enough to its chords to pass this by
        // accident.
        for at in [
            [-0.9f32, 1.5, 0.0],
            [-0.3, 2.1, 0.5],
            [0.3, 1.2, -0.5],
            [0.9, 1.9, 0.0],
        ] {
            document.add_curve_point(at, RADIUS).expect("refused");
        }
        document.set_curve_join(join).expect("join");

        let path = document.curve().path();
        assert!(path.len() >= 4, "{join:?} tessellated to nothing");

        let values = document
            .document()
            .eval_points(None, &path)
            .expect("the swept field");
        let worst = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        // Deep inside, not merely inside.
        //
        // "Every sample is within the tube" was the first version of this and
        // it does not discriminate: checked by feeding it the control polygon,
        // which is the line this change exists to stop drawing, and it passed.
        // A chord across a gentle bend stays inside a tube of this radius, so
        // the assertion was true of the wrong line as well as the right one.
        //
        // On the guide the tube is swept along, the field is about minus the
        // radius — that is what the centre of a tube of that thickness reads.
        // A line that cuts a corner rides closer to the wall and reads nearer
        // zero, which is what this measures.
        assert!(
            worst < -0.6 * RADIUS,
            "{join:?}: the worst of {} guide samples reads {worst} where the \
             centre line of a {RADIUS} tube reads about {}. The drawn guide is \
             not the line the sweep follows",
            path.len(),
            -RADIUS
        );
    }
}

/// A point put into the middle of a curve stays in the middle of it.
///
/// Appending is what a click on empty space does; a curve that can only grow
/// at its end cannot be refined where a tube usually needs it. The order
/// matters as much as the count: a point inserted at index 2 that landed at
/// the end would leave the guide doubling back on itself, and the tube with
/// it.
#[test]
fn a_point_inserted_into_a_curve_splits_the_span_it_names() {
    let mut document = document();
    lay(&mut document);
    let before = document.curve().points.len();
    let ends = (
        document.curve().points[0].position,
        document.curve().points[before - 1].position,
    );

    document
        .insert_curve_point(1, [-0.45, 1.9, 0.0], 0.13)
        .expect("the insertion was refused");

    let curve = document.curve();
    assert_eq!(curve.points.len(), before + 1, "nothing was inserted");
    assert_eq!(
        curve.points[1].position,
        [-0.45, 1.9, 0.0],
        "the point did not land where it was put"
    );
    assert_eq!(curve.points[0].position, ends.0, "the start moved");
    assert_eq!(
        curve.points[curve.points.len() - 1].position,
        ends.1,
        "the end moved"
    );
    assert_eq!(
        curve.selection,
        vec![1],
        "the point just placed is the one that should be in hand"
    );

    // And the tube still follows it: an insertion that produced a guide the
    // sweep disagreed with would show here as a sample outside the surface.
    let path = curve.path();
    let values = document
        .document()
        .eval_points(None, &path)
        .expect("the swept field");
    let worst = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        worst < 0.0,
        "after inserting a point the guide leaves its own tube by {worst}"
    );
}
