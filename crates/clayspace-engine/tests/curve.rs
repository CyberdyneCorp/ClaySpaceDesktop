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

use claycore::BrickMeshParams;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    CurveJoin, CurveModel, CurveProfile, FieldRefusal, GizmoTarget, ModelError, ObjectModel,
    SculptModel,
};

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

/// The YZ plane is transverse to this straight X guide and clear of the
/// starting form, so the zero crossing is the tube's actual half-width.
fn section_half_width(document: &ClayDocument, x: f32, direction: [f32; 2], high: f32) -> f32 {
    let sample = |distance| {
        document
            .document()
            .eval_points(
                None,
                &[[x, 2.0 + direction[0] * distance, direction[1] * distance]],
            )
            .expect("tube field")[0]
    };
    assert!(sample(0.0) < 0.0, "the guide lies outside its tube");
    assert!(sample(high) > 0.0, "the half-width exceeds {high}");
    let (mut inside, mut outside) = (0.0, high);
    for _ in 0..14 {
        let middle = (inside + outside) * 0.5;
        if sample(middle) < 0.0 {
            inside = middle;
        } else {
            outside = middle;
        }
    }
    (inside + outside) * 0.5
}

fn tube_half_width(document: &ClayDocument, x: f32, high: f32) -> f32 {
    section_half_width(document, x, [0.0, 1.0], high)
}

#[test]
fn a_tube_matches_its_radius_independent_of_point_density() {
    for radius in [0.02, 0.1, 0.5] {
        let mut widths = Vec::new();
        for count in [2, 10, 50] {
            let mut document = document();
            document.begin_curve();
            for index in 0..count {
                let x = index as f32 / (count - 1) as f32;
                document
                    .add_curve_point([x, 2.0, 0.0], radius)
                    .expect("point");
            }
            document.set_curve_join(CurveJoin::Corners).expect("join");
            let middle = tube_half_width(&document, 0.5, radius * 5.0 + 0.1);
            widths.push(middle);
            println!("radius={radius} points={count} half-width={middle}");
            assert!(
                (middle - radius).abs() <= radius * 0.1,
                "asked {radius} with {count} points, measured {middle}"
            );
        }
        let spread = widths.iter().copied().fold(f32::NEG_INFINITY, f32::max)
            - widths.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(
            spread <= radius * 0.05,
            "point density changed width by {spread}"
        );
    }
}

#[test]
fn circle_thickness_is_uniform_along_a_straight_span() {
    for join in CurveJoin::ALL {
        for count in [10, 50] {
            let radius = 0.1;
            let mut document = document();
            document.begin_curve();
            for index in 0..count {
                let x = index as f32 / (count - 1) as f32;
                document
                    .add_curve_point([x, 2.0, 0.0], radius)
                    .expect("point");
            }
            document.set_curve_join(join).expect("join");
            for x in [0.25, 0.5, 0.75] {
                let width = tube_half_width(&document, x, 0.3);
                assert!(
                    (width - radius).abs() <= radius * 0.1,
                    "{join:?}, {count} points at {x}: {width} rather than {radius}"
                );
            }
        }
    }
}

#[test]
fn non_circle_sections_have_no_spikes_along_a_straight_span() {
    for profile in [
        CurveProfile::Square,
        CurveProfile::Hexagon,
        CurveProfile::Triangle,
    ] {
        let mut document = document();
        document.begin_curve();
        for index in 0..50 {
            let x = index as f32 / 49.0;
            document.add_curve_point([x, 2.0, 0.0], 0.1).expect("point");
        }
        document.set_curve_profile(profile).expect("profile");
        for angle in 0..8 {
            let radians = angle as f32 * std::f32::consts::TAU / 8.0;
            let direction = [radians.cos(), radians.sin()];
            let widths =
                [0.25, 0.5, 0.75].map(|x| section_half_width(&document, x, direction, 0.3));
            let spread = widths.iter().copied().fold(f32::NEG_INFINITY, f32::max)
                - widths.iter().copied().fold(f32::INFINITY, f32::min);
            assert!(
                spread < 0.01,
                "{profile:?} at angle {angle} has a {spread} spike: {widths:?}"
            );
        }
    }
}

fn surface_digest(document: &ClayDocument) -> Vec<[i32; 3]> {
    let (mesh, _) = document
        .cache()
        .mesh(
            None,
            BrickMeshParams {
                gradient_normals: false,
                ..BrickMeshParams::default()
            },
            &[],
        )
        .expect("mesh cached surface");
    let mut positions: Vec<_> = mesh
        .positions()
        .iter()
        .map(|point| point.map(|axis| (axis * 1000.0).round() as i32))
        .collect();
    positions.sort_unstable();
    positions
}

fn lay_wide_curve(document: &mut ClayDocument, radius: f32) {
    document.begin_curve();
    for step in 0..5 {
        let t = step as f32 / 4.0;
        document
            .add_curve_point(
                [
                    -1.2 + t * 2.4,
                    1.45 + (t * 5.0).sin() * 0.25,
                    (t * 3.0).cos() * 0.15,
                ],
                radius,
            )
            .expect("point");
    }
}

fn settle_curve(document: &mut ClayDocument) {
    let join = document.curve().join;
    let other = if join == CurveJoin::Corners {
        CurveJoin::Through
    } else {
        CurveJoin::Corners
    };
    document.set_curve_join(other).expect("settle away");
    document.set_curve_join(join).expect("settle back");
}

fn assert_matches_full_refill(document: &mut ClayDocument) {
    let incremental = surface_digest(document);
    settle_curve(document);
    let whole = surface_digest(document);
    assert_eq!(
        incremental.len(),
        whole.len(),
        "the full refill changed vertex count"
    );
    if incremental != whole {
        let stale: Vec<_> = incremental
            .iter()
            .filter(|point| whole.binary_search(point).is_err())
            .take(8)
            .copied()
            .collect();
        let missing: Vec<_> = whole
            .iter()
            .filter(|point| incremental.binary_search(point).is_err())
            .take(8)
            .copied()
            .collect();
        panic!("the full refill moved cached vertices: stale {stale:?}; missing {missing:?}");
    }
}

#[test]
fn a_dense_curve_drag_matches_a_full_refill() {
    let mut document = document();
    document.begin_curve();
    for step in 0..16 {
        let t = step as f32 / 16.0;
        document
            .add_curve_point(
                [
                    -1.3 + t * 2.6,
                    1.35 + (t * 7.0).sin() * 0.45,
                    (t * 5.0).cos() * 0.25,
                ],
                0.09,
            )
            .expect("point");
    }
    assert_matches_full_refill(&mut document);
    document.select_curve_point(Some(8));
    for _ in 0..6 {
        document.drag_curve([0.0, 0.09, 0.0]).expect("drag");
    }
    assert_matches_full_refill(&mut document);
}

#[test]
fn a_curve_drag_matches_a_full_refill() {
    for radius in [0.04, 0.08, 0.16] {
        for join in [CurveJoin::Corners, CurveJoin::Through, CurveJoin::Rounded] {
            let mut document = document();
            lay_wide_curve(&mut document, radius);
            document.set_curve_join(join).expect("join");
            settle_curve(&mut document);
            document.select_curve_point(Some(2));
            document.drag_curve([0.0, 0.4, 0.0]).expect("drag");
            assert_matches_full_refill(&mut document);
        }
    }
}

#[test]
fn a_far_drag_clears_the_old_curve_extent() {
    let mut document = document();
    lay_wide_curve(&mut document, 0.08);
    settle_curve(&mut document);
    document.select_curve_point(Some(2));
    document.drag_curve([0.0, 1.5, 0.0]).expect("drag");
    assert_matches_full_refill(&mut document);
}

#[test]
fn an_append_matches_a_full_refill() {
    let mut document = document();
    document.begin_curve();
    for step in 0..5 {
        let t = step as f32 / 4.0;
        document
            .add_curve_point(
                [
                    -1.2 + t * 2.4,
                    1.45 + (t * 5.0).sin() * 0.25,
                    (t * 3.0).cos() * 0.15,
                ],
                0.08,
            )
            .expect("point");
        if step == 1 {
            settle_curve(&mut document);
        } else if step >= 2 {
            assert_matches_full_refill(&mut document);
        }
    }
}

#[test]
fn dense_appends_match_a_full_refill() {
    let mut document = document();
    document.begin_curve();
    for step in 0..2 {
        let t = step as f32 / 30.0;
        document
            .add_curve_point(
                [
                    -1.3 + t * 2.6,
                    1.35 + (t * 7.0).sin() * 0.45,
                    (t * 5.0).cos() * 0.3,
                ],
                0.09,
            )
            .expect("point");
    }
    settle_curve(&mut document);
    document
        .set_curve_join(CurveJoin::Through)
        .expect("through join");
    for step in 2..30 {
        let t = step as f32 / 30.0;
        document
            .add_curve_point(
                [
                    -1.3 + t * 2.6,
                    1.35 + (t * 7.0).sin() * 0.45,
                    (t * 5.0).cos() * 0.3,
                ],
                0.09,
            )
            .expect("point");
    }
    assert_matches_full_refill(&mut document);
}

#[test]
fn a_zero_drag_changes_nothing() {
    let mut document = document();
    lay_wide_curve(&mut document, 0.08);
    settle_curve(&mut document);
    let before = surface_digest(&document);
    document.select_curve_point(Some(2));
    document.take_dirty_keys();
    document.drag_curve([0.0; 3]).expect("zero drag");
    assert!(
        document.dirty_keys().is_empty(),
        "a zero drag dirtied bricks"
    );
    assert_eq!(
        before,
        surface_digest(&document),
        "a zero drag moved cached vertices"
    );
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
fn a_profile_change_reapplies_the_join() {
    for profile in [
        CurveProfile::Square,
        CurveProfile::Hexagon,
        CurveProfile::Triangle,
    ] {
        let mut document = document();
        lay(&mut document);
        document.set_curve_join(CurveJoin::Rounded).expect("join");
        document.set_curve_profile(profile).expect("profile");
        let directions = [[0.0, 1.0, 0.0], [-0.4, 1.0, 0.0], [0.4, 1.0, 0.0]];
        let immediate = directions.map(|direction| reach(&document, direction));
        document.set_curve_join(CurveJoin::Through).expect("away");
        document.set_curve_join(CurveJoin::Rounded).expect("back");
        let refilled = directions.map(|direction| reach(&document, direction));
        let worst = immediate
            .iter()
            .zip(&refilled)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 1e-3, "{profile:?} lost the curve join by {worst}");
    }
}

#[test]
fn a_profile_change_is_one_undoable_edit() {
    let mut document = document();
    lay(&mut document);
    let before = document.curve().points;
    document
        .set_curve_profile(CurveProfile::Square)
        .expect("profile");
    assert_eq!(document.curve().profile, CurveProfile::Square);
    assert!(SculptModel::undo(&mut document).expect("undo profile"));
    assert_eq!(document.curve().profile, CurveProfile::Circle);
    assert_eq!(document.curve().points, before);
    assert!(SculptModel::redo(&mut document).expect("redo profile"));
    assert_eq!(document.curve().profile, CurveProfile::Square);
    assert_eq!(document.curve().points, before);
}

#[test]
fn multiple_profile_replacements_follow_the_full_history() {
    let mut document = document();
    lay(&mut document);
    document
        .set_curve_profile(CurveProfile::Square)
        .expect("square");
    document
        .set_curve_profile(CurveProfile::Triangle)
        .expect("triangle");
    assert!(SculptModel::undo(&mut document).expect("undo triangle"));
    assert_eq!(document.curve().profile, CurveProfile::Square);
    assert!(SculptModel::undo(&mut document).expect("undo square"));
    assert_eq!(document.curve().profile, CurveProfile::Circle);
    assert!(SculptModel::redo(&mut document).expect("redo square"));
    assert_eq!(document.curve().profile, CurveProfile::Square);
    assert!(SculptModel::redo(&mut document).expect("redo triangle"));
    assert_eq!(document.curve().profile, CurveProfile::Triangle);
}

#[test]
fn a_radius_change_reshapes_each_section_profile() {
    for profile in [
        CurveProfile::Square,
        CurveProfile::Hexagon,
        CurveProfile::Triangle,
    ] {
        let mut document = document();
        document.begin_curve();
        for at in [[-0.9, 1.5, 0.0], [0.0, 1.7, 0.0], [0.9, 1.5, 0.0]] {
            document.add_curve_point(at, 0.06).expect("point");
        }
        document.set_curve_profile(profile).expect("profile");
        document.select_curve_point(None);
        let thin = reach(&document, [0.0, 1.0, 0.0]);
        document.set_curve_radius(0.24).expect("radius");
        let thick = reach(&document, [0.0, 1.0, 0.0]);
        assert!(
            thick > thin + 0.1,
            "{profile:?} kept the old profile: {thin} -> {thick}"
        );
        assert!(SculptModel::undo(&mut document).expect("undo radius"));
        assert_eq!(document.curve().points[0].radius, 0.06);
        assert!(SculptModel::redo(&mut document).expect("redo radius"));
        assert_eq!(document.curve().points[0].radius, 0.24);
    }
}

#[test]
fn adding_a_point_without_a_curve_is_refused() {
    let mut document = document();
    assert!(document.add_curve_point([0.0; 3], 0.1).is_err());
}

#[test]
fn inactive_curve_verbs_are_refused() {
    let mut document = document();
    assert!(document.insert_curve_point(0, [0.0; 3], 0.1).is_err());
    assert!(document.drag_curve([0.1, 0.0, 0.0]).is_err());
    assert!(document.set_curve_radius(0.1).is_err());
    assert!(document.set_curve_join(CurveJoin::Corners).is_err());
    assert!(document.set_curve_profile(CurveProfile::Square).is_err());
    assert!(document.remove_curve_points().is_err());
    assert!(document.apply_curve().is_err());
    assert!(ObjectModel::target_transform(&mut document, GizmoTarget::Curve).is_none());
    assert!(ObjectModel::set_target_transform(
        &mut document,
        GizmoTarget::Curve,
        clayspace_model::Transform::default(),
    )
    .is_err());
}

#[test]
fn the_curve_target_moves_selected_points_from_the_gesture_start() {
    let mut document = document();
    document.begin_curve();
    document
        .add_curve_point([-0.5, 1.5, 0.0], 0.1)
        .expect("point");
    document
        .add_curve_point([0.5, 1.5, 0.0], 0.1)
        .expect("point");
    document.select_curve_point(Some(0));
    document.toggle_curve_point(1);
    let start = ObjectModel::target_transform(&mut document, GizmoTarget::Curve)
        .expect("selected curve target");
    assert!((start.position[0]).abs() < 1e-6);
    ObjectModel::begin_target_drag(&mut document, GizmoTarget::Curve);
    for amount in [0.3, 0.1] {
        ObjectModel::set_target_transform(
            &mut document,
            GizmoTarget::Curve,
            clayspace_model::Transform {
                position: [0.0, 1.5 + amount, 0.0],
                ..start
            },
        )
        .expect("move selected points");
    }
    ObjectModel::end_target_drag(&mut document);
    let points = document.curve().points;
    assert!((points[0].position[1] - 1.6).abs() < 1e-5);
    assert!((points[1].position[1] - 1.6).abs() < 1e-5);
    assert!(SculptModel::undo(&mut document).expect("undo drag"));
    let restored = document.curve().points;
    assert!((restored[0].position[1] - 1.5).abs() < 1e-5);
    assert!((restored[1].position[1] - 1.5).abs() < 1e-5);
}

#[test]
fn the_curve_target_rotates_and_scales_selected_points() {
    let mut document = document();
    document.begin_curve();
    document
        .add_curve_point([-0.5, 1.5, 0.0], 0.1)
        .expect("point");
    document
        .add_curve_point([0.5, 1.5, 0.0], 0.1)
        .expect("point");
    document.select_curve_point(Some(0));
    document.toggle_curve_point(1);
    let start = ObjectModel::target_transform(&mut document, GizmoTarget::Curve).unwrap();
    ObjectModel::begin_target_drag(&mut document, GizmoTarget::Curve);
    ObjectModel::set_target_transform(
        &mut document,
        GizmoTarget::Curve,
        clayspace_model::Transform {
            rotation_axis: [0.0, 0.0, 1.0],
            rotation_angle: std::f32::consts::FRAC_PI_2,
            scale: [2.0, 1.0, 1.0],
            ..start
        },
    )
    .expect("rotate and scale");
    ObjectModel::end_target_drag(&mut document);
    let points = document.curve().points;
    assert!((points[0].position[1] - 0.5).abs() < 1e-5);
    assert!((points[1].position[1] - 2.5).abs() < 1e-5);
}

#[test]
fn an_unaffordable_curve_transform_leaves_the_points_in_place() {
    let mut document = document();
    lay(&mut document);
    document.select_curve_point(Some(0));
    document.toggle_curve_point(2);
    let before = document.curve().points;
    let start = ObjectModel::target_transform(&mut document, GizmoTarget::Curve).unwrap();
    let result = ObjectModel::set_target_transform(
        &mut document,
        GizmoTarget::Curve,
        clayspace_model::Transform {
            scale: [1_000_000.0; 3],
            ..start
        },
    );
    assert!(
        result.is_err(),
        "a huge curve should exceed the field budget"
    );
    assert_eq!(document.curve().points, before);
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

/// A curve built point by point has the same surface as one built whole.
/// The pick path alone cannot detect a stale cache; the mesh digest tests
/// above compare the cached surface directly.
#[test]
fn a_curve_laid_point_by_point_is_not_left_stale() {
    let mut document = document();
    document.begin_curve();
    // A wandering path exposes stale geometry between control points.
    for step in 0..14 {
        let t = step as f32 / 14.0;
        let at = [
            -1.3 + t * 2.6,
            1.35 + (t * 7.0).sin() * 0.45,
            (t * 5.0).cos() * 0.3,
        ];
        document.add_curve_point(at, 0.09).expect("refused");
    }

    let probes: Vec<[f32; 3]> = (0..24)
        .map(|step| {
            let angle = step as f32 / 24.0 * std::f32::consts::TAU;
            [angle.cos(), 1.0 + angle.sin() * 0.6, angle.sin() * 0.4]
        })
        .collect();
    let incremental: Vec<f32> = probes.iter().map(|at| reach(&document, *at)).collect();

    // The whole tube, refilled from scratch.
    //
    // Through the join, because a join change is **not** an append: it can
    // move the entire curve, so it takes the node's own bound rather than a
    // tail region. Away and back leaves the geometry exactly as it was and the
    // cache rebuilt for all of it — which is the comparison this test needs
    // and there is no other public way to ask for.
    let join = document.curve().join;
    let other = if join == CurveJoin::Corners {
        CurveJoin::Through
    } else {
        CurveJoin::Corners
    };
    document.set_curve_join(other).expect("join away");
    document.set_curve_join(join).expect("join back");
    let whole: Vec<f32> = probes.iter().map(|at| reach(&document, *at)).collect();

    let worst = incremental
        .iter()
        .zip(&whole)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        worst < 1e-3,
        "refilling the whole layer moved the surface by {worst}, so laying the \
         curve point by point had left bricks stale that the appended end \
         changed"
    );
}

/// An append marks the whole curve until the field supports a local bound.
#[test]
fn appending_a_point_refills_the_curve_extent() {
    let mut document = document();
    document.begin_curve();
    for step in 0..20 {
        let t = step as f32 / 20.0;
        document
            .add_curve_point([-1.3 + t * 2.6, 1.35 + (t * 7.0).sin() * 0.45, 0.0], 0.09)
            .expect("point");
    }
    document.take_dirty_keys();
    document
        .add_curve_point([1.4, 1.5, 0.0], 0.09)
        .expect("append");
    let appended = document.dirty_keys().len();
    document.take_dirty_keys();
    settle_curve(&mut document);
    let whole = document.dirty_keys().len();
    assert!(appended > 0);
    assert_eq!(appended, whole, "append left part of the curve stale");
}

/// Dragging a control point leaves nothing stale — where it went, and where it
/// came from.
///
/// A drag can change the surface beyond adjacent spans. This test compares
/// the cached result with a full refill so a missed region cannot hide
/// it — checked, by shrinking the drag region's margin to a twentieth of the
/// radius and watching `a_curve_laid_point_by_point_is_not_left_stale` pass
/// anyway. Two code paths, two margins, one test between them.
///
/// The half a narrow region gets wrong first is the place the point *left*:
/// refilling only where it arrived leaves the old bulge standing on the
/// surface, and nothing reports it.
#[test]
fn dragging_a_point_leaves_nothing_stale_behind_it() {
    let mut document = document();
    document.begin_curve();
    for step in 0..16 {
        let t = step as f32 / 16.0;
        document
            .add_curve_point(
                [
                    -1.3 + t * 2.6,
                    1.35 + (t * 7.0).sin() * 0.45,
                    (t * 5.0).cos() * 0.25,
                ],
                0.09,
            )
            .expect("refused");
    }
    document.select_curve_point(Some(8));
    // Far enough that where it came from and where it went do not overlap.
    for _ in 0..6 {
        document.drag_curve([0.0, 0.09, 0.0]).expect("drag");
    }

    let probes: Vec<[f32; 3]> = (0..32)
        .map(|step| {
            let angle = step as f32 / 32.0 * std::f32::consts::TAU;
            [angle.cos(), 1.0 + angle.sin() * 0.7, angle.sin() * 0.4]
        })
        .collect();
    let dragged: Vec<f32> = probes.iter().map(|at| reach(&document, *at)).collect();

    // The whole tube from scratch, through a join change — which is neither an
    // append nor a drag, so it takes the node's own bound.
    let join = document.curve().join;
    let other = if join == CurveJoin::Corners {
        CurveJoin::Through
    } else {
        CurveJoin::Corners
    };
    document.set_curve_join(other).expect("join away");
    document.set_curve_join(join).expect("join back");
    let whole: Vec<f32> = probes.iter().map(|at| reach(&document, *at)).collect();

    let worst = dragged
        .iter()
        .zip(&whole)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        worst < 1e-3,
        "refilling the whole tube after the drag moved the surface by {worst}, \
         so dragging a control point had left bricks stale — most likely where \
         the point came from rather than where it went"
    );
}

/// A thickness the field cannot hold is refused, and the curve is left alone.
///
/// **This number had no upper bound at all.** It was clamped to a minimum and
/// nothing above it, so `curve/set_radius 5` on a three-point guide was
/// accepted: the application held for more than thirty seconds and took itself
/// to four and a half gigabytes sweeping a tube nobody could have wanted. It
/// is priced now, the way a crossing has always been priced, and the price is
/// paid before a single radius is written.
#[test]
fn an_oversized_curve_radius_is_refused() {
    let mut document = document();
    lay(&mut document);
    let before = document.curve().points.clone();

    let refused = document
        .set_curve_radius(5.0)
        .expect_err("a radius of 5 was accepted");
    assert!(
        matches!(
            refused,
            ModelError::Field(FieldRefusal::RegionOverBudget { .. })
        ),
        "refused for the wrong reason: {refused}"
    );

    assert_eq!(
        document.curve().points,
        before,
        "a refused thickness reached the guide anyway"
    );

    // A thickness the document can carry still goes through, so this is a
    // bound and not a wall. Laying a point leaves it selected, so it is that
    // point which takes the new thickness and the rest that keep theirs.
    document
        .set_curve_radius(0.3)
        .expect("an ordinary thickness was refused");
    let after = document.curve().points;
    assert!((after[2].radius - 0.3).abs() < 1e-6);
    assert_eq!(after[0].radius, before[0].radius);
}

/// A guide that has been refused a thickness is still a guide that can be
/// taken away.
///
/// The measured failure was the other way round: a region past the cache's
/// limit left a curve that could not be removed, so the document could not be
/// repaired by the person who broke it. Refusing the thickness is what stops
/// that region existing, and this holds the rest of the way — the refusal does
/// not strand the curve.
#[test]
fn a_curve_over_the_brick_limit_can_be_removed() {
    let mut document = document();
    lay(&mut document);
    assert!(document.set_curve_radius(5.0).is_err());

    document
        .remove_curve_points()
        .expect("the refused curve would not give up its points");

    document.cancel_curve();
    assert!(!document.curve().active, "the curve outlived its removal");
}
