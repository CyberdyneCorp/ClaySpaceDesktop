//! The cut entry points, and the two things the wrapper is responsible for:
//! holding the size query, and refusing rather than correcting.

use claycore::{cut, CutFrame, CutOutline, CutShape, PointType, TrimSide};

/// A frame looking down -Z, which is what a camera at the front gives.
fn frame() -> CutFrame {
    CutFrame {
        origin: [0.0, 0.0, 3.0],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        region: ([-1.5, -1.5, -1.5], [1.5, 1.5, 1.5]),
        rounding: 0.0,
    }
}

/// An open stroke across the frame, four points, x y z r.
fn across() -> Vec<f32> {
    vec![
        -2.0, 0.1, 0.0, 0.0, //
        -0.7, 0.25, 0.0, 0.0, //
        0.7, -0.15, 0.0, 0.0, //
        2.0, 0.0, 0.0, 0.0,
    ]
}

#[test]
fn an_open_stroke_closes_against_the_frame() {
    let outline = CutOutline::from_open_curve(
        &across(),
        Some(&[PointType::Spline as i32; 4]),
        TrimSide::Below,
        [8.0, 8.0],
        0.002,
    )
    .expect("the open curve was refused");
    assert!(
        outline.points() >= 4,
        "a stroke closed against the frame is at least the stroke plus its \
         closing edge, and this came back with {} points",
        outline.points()
    );
}

/// The two sides are different polygons, which is what makes the side worth
/// inferring at all rather than defaulting.
#[test]
fn the_two_sides_of_one_stroke_are_different_outlines() {
    let below = CutOutline::from_open_curve(&across(), None, TrimSide::Below, [8.0, 8.0], 0.002)
        .expect("below");
    let above = CutOutline::from_open_curve(&across(), None, TrimSide::Above, [8.0, 8.0], 0.002)
        .expect("above");
    assert_ne!(
        below, above,
        "the same stroke closed on either side produced the same polygon, so \
         the side is not reaching the engine"
    );
}

/// A closed lasso is a different shape from the same points, which is why the
/// two are different entry points rather than one with a flag.
#[test]
fn a_closed_lasso_is_not_the_open_stroke_closed() {
    let open = CutOutline::from_open_curve(&across(), None, TrimSide::Below, [8.0, 8.0], 0.002)
        .expect("open");
    let closed = CutOutline::from_closed_curve(&across(), None, 0.002).expect("closed");
    assert_ne!(
        open, closed,
        "joining the stroke's endpoints gave the same polygon as closing it \
         against the frame; the engine warns these are different shapes"
    );
}

#[test]
fn a_polygon_resolves_into_an_item() {
    let outline = CutOutline::from_closed_curve(&across(), None, 0.002).expect("outline");
    cut(&frame(), CutShape::Polygon, &outline).expect("the cut was refused");
}

/// The rectangle is the engine's own, not a four-point polygon: it can express
/// the shape exactly and going through the polygon path would tessellate it.
#[test]
fn a_rectangle_needs_no_outline_at_all() {
    cut(
        &frame(),
        CutShape::Rect {
            half_width: 0.5,
            half_height: 0.25,
        },
        &CutOutline::default(),
    )
    .expect("the rect cut was refused");
}

/// The engine refuses a frame that is not orthonormal deliberately — the shape
/// the sculptor saw was drawn in the frame they think they have — and this
/// wrapper must pass that refusal on rather than squaring the frame up.
#[test]
fn a_frame_that_is_not_orthonormal_is_refused() {
    let mut bad = frame();
    // `up` parallel to `right`: no plane, so no shape.
    bad.up = [1.0, 0.0, 0.0];
    let outline = CutOutline::from_closed_curve(&across(), None, 0.002).expect("outline");
    assert!(
        cut(&bad, CutShape::Polygon, &outline).is_err(),
        "a degenerate frame was accepted, which means it was corrected \
         somewhere rather than reported"
    );
}
