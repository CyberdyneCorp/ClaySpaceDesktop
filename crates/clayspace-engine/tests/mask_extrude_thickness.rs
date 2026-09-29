//! Extrudar builds the wall it is asked for (#178, audit defect F5).
//!
//! The engine keeps the part of the wall's shell that lies inside the mask's
//! own volume, and a mask painted on a surface is a thin volume around it, so
//! every wall used to stop where the paint stopped: on the unit sphere with an
//! outline mask, 0.3 and 0.6 both came out about 0.11 tall, with a top that
//! followed the dabs. The document now hands the engine the painted patch
//! swept along the surface normal as far as the wall goes.
//!
//! Every wall here is measured the same way: the radius of the surface along a
//! direction inside the patch, after the extrusion minus before it. On a unit
//! sphere the surface normal is the radial direction, so that difference *is*
//! the wall's height at that spot.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ExtrudeSettings, ExtrudeSide, GestureSample, MaskModel, MaskOutline,
    OutlineFrame, OutlineMode, SceneModel, SculptModel, ToolKind,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// The near face frozen by a square outline drawn looking down -z — the
/// audit's repro, and a mask swept straight through the form.
fn outlined() -> ClayDocument {
    let mut document = document();
    let half = 0.25;
    document
        .apply_outline(&MaskOutline {
            outline: vec![[-half, -half], [half, -half], [half, half], [-half, half]],
            frame: OutlineFrame {
                origin: [0.0, 0.0, 0.0],
                right: [1.0, 0.0, 0.0],
                up: [0.0, 1.0, 0.0],
                forward: [0.0, 0.0, -1.0],
                scale: [1.0, 1.0],
            },
            mode: OutlineMode::Freeze,
        })
        .expect("the outline froze the near face");
    document
}

/// The near face frozen by one Máscara dab, the other way a mask is made.
fn dabbed() -> ClayDocument {
    let mut document = document();
    let at = SculptModel::pick(&document, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("the starting form is under the ray");
    let samples: Vec<GestureSample> = (0..4)
        .map(|i| GestureSample {
            position: at,
            pressure: 1.0,
            time: i as f32 * 0.1,
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Mascara,
            BrushSettings {
                size: 0.3,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("paint the mask");
    document
}

/// Directions inside the patch: its middle and four points around it, the
/// outer ones near the edge of the dab's frozen core.
const SPOTS: [[f32; 3]; 5] = [
    [0.0, 0.0, 1.0],
    [0.07, 0.0, 1.0],
    [-0.07, 0.0, 1.0],
    [0.0, 0.07, 1.0],
    [0.0, -0.07, 1.0],
];

fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> f32 {
    let n = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / n);
    let hit = SculptModel::pick(document, unit.map(|c| c * 4.0), unit.map(|c| -c))
        .expect("the surface is under the ray");
    hit.iter().map(|c| c * c).sum::<f32>().sqrt()
}

/// A document with the near face frozen.
type Fixture = fn() -> ClayDocument;

/// The wall's height at every spot, for one thickness.
fn walls(masked: Fixture, side: ExtrudeSide, thickness: f32) -> Vec<f32> {
    walls_at(&SPOTS, masked, side, thickness)
}

fn walls_at(spots: &[[f32; 3]], masked: Fixture, side: ExtrudeSide, thickness: f32) -> Vec<f32> {
    let mut document = masked();
    let before: Vec<f32> = spots.iter().map(|&d| radius_along(&document, d)).collect();
    document
        .extrude_mask(ExtrudeSettings {
            thickness,
            side,
            border_round: 0.0,
            border_smooth: 0,
        })
        .expect("the extrusion was refused");
    spots
        .iter()
        .zip(before)
        .map(|(&d, b)| radius_along(&document, d) - b)
        .collect()
}

fn assert_matches(walls: &[f32], expected: f32, what: &str) {
    for (spot, wall) in SPOTS.iter().zip(walls) {
        assert!(
            (wall - expected).abs() <= 0.1 * expected,
            "{what}: the wall at {spot:?} is {wall}, more than 10% from {expected} \
             (every spot: {walls:?})"
        );
    }
}

fn spread(walls: &[f32]) -> f32 {
    let max = walls.iter().copied().fold(f32::MIN, f32::max);
    let min = walls.iter().copied().fold(f32::MAX, f32::min);
    max - min
}

#[test]
fn extrude_uses_its_thickness_on_an_outline_mask() {
    // Before: 0.05 → 0.050, 0.1 → ~0.1, and 0.6 stopped at ~0.11.
    for thickness in [0.05, 0.1, 0.6] {
        let measured = walls(outlined, ExtrudeSide::Outward, thickness);
        assert_matches(&measured, thickness, "outline mask, Para fora");
    }
}

#[test]
fn extrude_uses_its_thickness_on_a_painted_mask() {
    // A dab's ball reaches about 0.16 off the surface, which is where every
    // wall used to stop.
    for thickness in [0.05, 0.1, 0.6] {
        let measured = walls(dabbed, ExtrudeSide::Outward, thickness);
        assert_matches(&measured, thickness, "painted mask, Para fora");
    }
}

#[test]
fn the_wall_is_even_across_the_patch() {
    // The top of the wall used to follow the mask's volume — one lobe per dab
    // on an outline. It is the shell's own offset surface now, so the height
    // is the same wherever it is measured inside the patch.
    //
    // The outline's spots run out to 0.02 inside its edge, where the old wall
    // stood 0.13 against 0.08-0.10 in the middle.
    let across: Vec<[f32; 3]> = [0.0, 0.05, 0.1, 0.15, 0.2, 0.23]
        .iter()
        .map(|&x| [x, 0.5 * x, 1.0])
        .collect();
    let checks: [(&[[f32; 3]], Fixture); 2] = [(&across, outlined), (&SPOTS, dabbed)];
    for (spots, masked) in checks {
        let measured = walls_at(spots, masked, ExtrudeSide::Outward, 0.6);
        assert!(
            spread(&measured) <= 0.02,
            "a 0.6 wall varies by {} across the patch: {measured:?}",
            spread(&measured)
        );
    }
}

#[test]
fn centred_and_inward_walls_keep_their_thickness_too() {
    // Centrado puts half the wall above the surface; Para dentro leaves the
    // outside where it was. Both used to be capped the same way.
    let centred = walls(outlined, ExtrudeSide::Centred, 0.6);
    assert_matches(&centred, 0.3, "outline mask, Centrado (outer half)");
    let inward = walls(outlined, ExtrudeSide::Inward, 0.6);
    for wall in &inward {
        assert!(
            wall.abs() < 0.01,
            "Para dentro moved the outside by {wall}: {inward:?}"
        );
    }
}

#[test]
fn a_thickness_past_what_the_region_can_hold_is_refused() {
    // The engine's own measurement of the mask is a dense array over a box
    // grown by the thickness, so a wall of 100 on a patch this size would cost
    // gigabytes. Refused with a reason, and no layer left behind.
    let mut document = outlined();
    let layers = document.scene().layers.len();
    let refusal = document
        .extrude_mask(ExtrudeSettings {
            thickness: 100.0,
            ..ExtrudeSettings::default()
        })
        .expect_err("a 100-unit wall");
    assert!(
        refusal.to_string().contains("espessura"),
        "the refusal does not say why: {refusal}"
    );
    assert_eq!(document.scene().layers.len(), layers);
}
