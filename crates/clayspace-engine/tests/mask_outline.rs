//! Freezing a region by drawing round it — ZBrush's mask lasso and mask rect.
//!
//! The gesture is made on the screen and not on the surface, so what it has to
//! be judged against is what a sculptor sees: the side of the form the outline
//! covered is frozen, the side it did not is free, and the far surface behind
//! the outline is frozen with the near one — because the outline was drawn
//! *through* the form.
//!
//! Which of the two gestures drew the outline is not visible here, and that is
//! the design: a lasso and a rectangle differ in how the pointer builds the
//! shape, and `outline.rs` is where that difference lives. By the time an
//! outline reaches the document it is a list of points either way.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, GestureSample, MaskModel, MaskOutline, OutlineFrame, OutlineMode, SculptModel,
    ToolKind,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Looking down -z, which puts the frame's x on world x and its y on world y.
///
/// A camera basis, spelled out rather than taken from one: the domain takes a
/// frame precisely so that a test does not need a viewport to build one.
fn looking_down_z() -> OutlineFrame {
    OutlineFrame {
        origin: [0.0, 0.0, 0.0],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        scale: [1.0, 1.0],
    }
}

/// A square outline of `half` world units about `centre` on the frame.
fn square(centre: [f32; 2], half: f32, mode: OutlineMode) -> MaskOutline {
    MaskOutline {
        outline: vec![
            [centre[0] - half, centre[1] - half],
            [centre[0] + half, centre[1] - half],
            [centre[0] + half, centre[1] + half],
            [centre[0] - half, centre[1] + half],
        ],
        frame: looking_down_z(),
        mode,
    }
}

/// Radius of the surface along a direction, the fingerprint the mask tests use.
fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> Option<f32> {
    let n =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
            .sqrt();
    let unit = direction.map(|c| c / n);
    document
        .pick(unit.map(|c| c * 4.0), unit.map(|c| -c))
        .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
}

/// A dab of the default tool at a spot on the surface.
fn dab(document: &mut ClayDocument, at: [f32; 3]) {
    let brush = BrushSettings {
        size: 0.3,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    let samples: Vec<GestureSample> = (0..4)
        .map(|i| GestureSample {
            position: at,
            pressure: 1.0,
            time: i as f32 * 0.01,
        })
        .collect();
    document
        .apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])
        .expect("a stroke");
}

#[test]
fn an_outline_freezes_what_it_encloses() {
    let mut document = document();
    assert!(
        !document.mask_state().is_active(),
        "the starting form arrived masked"
    );

    document
        .apply_outline(&square([0.5, 0.0], 0.4, OutlineMode::Freeze))
        .expect("the outline");

    let state = document.mask_state();
    assert!(state.is_active(), "the outline froze nothing");
    assert!(
        state.painted_cells > 100,
        "only {} cells were frozen",
        state.painted_cells
    );
}

#[test]
fn the_side_the_outline_covered_is_the_side_that_resists() {
    // The whole of what a drawn mask promises: what you drew round survives
    // the brush, and what you did not does not.
    let mut document = document();
    let inside = [0.5f32, 0.0, 1.0];
    let outside = [-0.5f32, 0.0, 1.0];

    document
        .apply_outline(&square([0.5, 0.0], 0.45, OutlineMode::Freeze))
        .expect("the outline");

    let before_inside = radius_along(&document, inside).expect("a hit inside");
    let before_outside = radius_along(&document, outside).expect("a hit outside");

    dab(&mut document, inside);
    dab(&mut document, outside);

    let after_inside = radius_along(&document, inside).expect("a hit inside");
    let after_outside = radius_along(&document, outside).expect("a hit outside");

    assert!(
        (after_inside - before_inside).abs() < 1e-3,
        "the frozen side moved by {}",
        after_inside - before_inside
    );
    assert!(
        after_outside - before_outside > 1e-2,
        "the free side did not move: {} to {after_outside}",
        before_outside
    );
}

#[test]
fn the_far_surface_freezes_with_the_near_one() {
    // The outline is drawn on the screen, so it goes through the form. ZBrush
    // masks both sides and so does this; an outline that froze only what faced
    // the camera would be a surface brush wearing the name.
    let mut document = document();
    let near = [0.0f32, 0.0, 1.0];
    let far = [0.0f32, 0.0, -1.0];

    document
        .apply_outline(&square([0.0, 0.0], 0.5, OutlineMode::Freeze))
        .expect("the outline");

    let before = radius_along(&document, far).expect("a hit behind");
    dab(&mut document, far);
    let after = radius_along(&document, far).expect("a hit behind");
    assert!(
        (after - before).abs() < 1e-3,
        "the far side moved by {}",
        after - before
    );

    let before = radius_along(&document, near).expect("a hit in front");
    dab(&mut document, near);
    let after = radius_along(&document, near).expect("a hit in front");
    assert!((after - before).abs() < 1e-3, "the near side moved");
}

#[test]
fn an_outline_with_the_modifier_held_releases_what_it_encloses() {
    let mut document = document();
    document
        .apply_outline(&square([0.0, 0.0], 0.8, OutlineMode::Freeze))
        .expect("the outline");
    let frozen = document.mask_state().painted_cells;
    assert!(frozen > 0);

    document
        .apply_outline(&square([0.0, 0.0], 0.4, OutlineMode::Thaw))
        .expect("the outline");
    let left = document.mask_state().painted_cells;
    assert!(
        left < frozen,
        "releasing froze more instead: {frozen} to {left}"
    );
    assert!(left > 0, "releasing the middle took the whole mask away");

    // And the released middle is a region a brush can reach again.
    let at = [0.0f32, 0.0, 1.0];
    let before = radius_along(&document, at).expect("a hit");
    dab(&mut document, at);
    let after = radius_along(&document, at).expect("a hit");
    assert!(
        after - before > 1e-2,
        "the released region still resisted: {before} to {after}"
    );
}

#[test]
fn a_whole_outline_is_one_thing_to_undo() {
    // The reason the region is delivered as one stroke rather than as cells:
    // a mask edit records on the engine's history, and an outline that recorded
    // per cell would be tens of thousands of undo entries.
    let mut document = document();
    document
        .apply_outline(&square([0.0, 0.0], 0.5, OutlineMode::Freeze))
        .expect("the outline");
    assert!(document.mask_state().is_active());

    assert!(document.undo().expect("undo"), "there was nothing to undo");
    assert!(
        !document.mask_state().is_active(),
        "one undo left {} cells frozen",
        document.mask_state().painted_cells
    );

    assert!(document.redo().expect("redo"));
    assert!(document.mask_state().is_active(), "redo lost the outline");
}

#[test]
fn the_viewport_is_told_the_mask_moved() {
    // An outline moves no clay, so no brick is dirty and nothing else would make
    // the viewport re-sample the frozen region it draws.
    let mut document = document();
    let before = document.mask_revision();
    document
        .apply_outline(&square([0.0, 0.0], 0.5, OutlineMode::Freeze))
        .expect("the outline");
    assert_ne!(document.mask_revision(), before, "the revision stood still");
}

#[test]
fn an_outline_that_encloses_nothing_is_refused_in_words() {
    let mut document = document();
    let e = document
        .apply_outline(&MaskOutline {
            outline: vec![[0.0, 0.0], [0.5, 0.0]],
            frame: looking_down_z(),
            mode: OutlineMode::Freeze,
        })
        .expect_err("a line is not a region");
    assert!(!e.to_string().is_empty());
    assert!(!document.mask_state().is_active());
}

#[test]
fn an_outline_drawn_beside_the_form_freezes_nothing_and_says_nothing() {
    // Missing is not an error: the mask is what it was, and an interface that
    // put up a refusal for a gesture the sculptor is about to repeat would be
    // noise.
    let mut document = document();
    document
        .apply_outline(&square([12.0, 12.0], 0.5, OutlineMode::Freeze))
        .expect("an outline that missed is not a refusal");
    assert!(!document.mask_state().is_active());
}

#[test]
fn an_outline_is_the_region_it_was_drawn_around_and_not_its_bounding_box() {
    // The traversal's whole reason for existing. A path that ran back and
    // forth across the rows would cross the opening of a concave outline, and
    // everything it crossed would freeze — so a C would come out a square.
    let mut document = document();
    let c = MaskOutline {
        outline: vec![
            [-0.6, -0.6],
            [0.6, -0.6],
            [0.6, -0.35],
            [-0.35, -0.35],
            [-0.35, 0.35],
            [0.6, 0.35],
            [0.6, 0.6],
            [-0.6, 0.6],
        ],
        frame: looking_down_z(),
        mode: OutlineMode::Freeze,
    };
    document.apply_outline(&c).expect("the outline");

    // The opening of the C, which is inside its bounding box and outside it.
    let at = [0.35f32, 0.0, 1.0];
    let before = radius_along(&document, at).expect("a hit");
    dab(&mut document, at);
    let after = radius_along(&document, at).expect("a hit");
    assert!(
        after - before > 1e-2,
        "the opening of the C was frozen too: {before} to {after}"
    );
}
