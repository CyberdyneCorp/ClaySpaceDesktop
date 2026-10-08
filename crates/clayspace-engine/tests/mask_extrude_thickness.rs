//! Extrudar builds the wall it is asked for, on a field and on a grid (#178,
//! audit defect F5).
//!
//! The engine of ClayCore v0.120.1 kept the part of the wall's shell that lay
//! inside the mask's own volume, and a mask painted on a surface is a thin
//! volume around it, so every wall stopped where the paint stopped: on the unit
//! sphere with an outline mask, 0.3 and 0.6 both came out about 0.11 tall,
//! with a top that followed the dabs. The document worked around it on a field
//! layer by handing the engine the painted patch swept along the surface
//! normal. ClayCore v0.126.0 (#667, issue #660) reads the mask at the source
//! surface under each sample itself, so the document hands it the painted
//! mask again. These tests are what holds the engine to that.
//!
//! Measured on this machine (Apple M3 Pro, Metal) before the sweep was
//! removed, the swept region against the painted mask, Para fora, one call:
//!
//! | Mask | Thickness | Wall, swept | Wall, painted | Cost, swept | Cost, painted |
//! |---|---|---|---|---|---|
//! | Outline, 0.5 square | 0.05 | 0.050 | 0.050 | 67 ms | 43 ms |
//! | Outline, 0.5 square | 0.1 | 0.100 | 0.100 | 115 ms | 57 ms |
//! | Outline, 0.5 square | 0.6 | 0.600 | 0.600 | 2313 ms | 478 ms |
//! | One Máscara dab, size 0.3 | 0.05 | 0.050 | 0.050 | 23 ms | 18 ms |
//! | One Máscara dab, size 0.3 | 0.1 | 0.100 | 0.100 | 42 ms | 19 ms |
//! | One Máscara dab, size 0.3 | 0.6 | 0.600 | 0.600 | 434 ms | 186 ms |
//!
//! Every wall agrees to within 0.0002 and varies by no more than 0.0009
//! around the boundary either way, so the sweep bought nothing but cost.
//!
//! Every wall here is measured the same way: the radius of the surface along a
//! direction inside the patch, after the extrusion minus before it. On a unit
//! sphere the surface normal is the radial direction, so that difference *is*
//! the wall's height at that spot. A grid's wall is read with its row active,
//! because a pick on a grid answers from the grid's cells alone.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, ExtrudeSettings, ExtrudeSide, GestureSample, MaskModel, MaskOutline,
    OutlineFrame, OutlineMode, SceneModel, SculptModel, ToolKind,
};

/// The cell the grid fixture is quantised to: the panel's default.
const GRID_CELL: f32 = 0.02;

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

/// Freezes the near face with one Máscara dab, the other way a mask is made.
fn dab(document: &mut ClayDocument) {
    let at = SculptModel::pick(document, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
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
}

fn dabbed() -> ClayDocument {
    let mut document = document();
    dab(&mut document);
    document
}

/// The starting form crossed to a grid at the panel's default cell, with the
/// same dab frozen on it — the fixture `undo_ordering.rs` extrudes grids on.
fn gridded() -> ClayDocument {
    let mut document = document();
    document
        .convert_layer(Direction::SdfToVoxel, GRID_CELL, 1)
        .expect("to a grid");
    dab(&mut document);
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

/// Eight directions around a circle of radius `r` on the near face.
fn ring(r: f32) -> Vec<[f32; 3]> {
    (0..8)
        .map(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_4;
            [r * a.cos(), r * a.sin(), 1.0]
        })
        .collect()
}

/// Eight directions along the inside of the outline's square, `h` from its
/// middle: the four corners and the four sides.
fn square_ring(h: f32) -> Vec<[f32; 3]> {
    vec![
        [-h, -h, 1.0],
        [0.0, -h, 1.0],
        [h, -h, 1.0],
        [h, 0.0, 1.0],
        [h, h, 1.0],
        [0.0, h, 1.0],
        [-h, h, 1.0],
        [-h, 0.0, 1.0],
    ]
}

fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> f32 {
    let n = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / n);
    let hit = SculptModel::pick(document, unit.map(|c| c * 4.0), unit.map(|c| -c))
        .expect("the surface is under the ray");
    hit.iter().map(|c| c * c).sum::<f32>().sqrt()
}

/// A document with a patch of the near face frozen.
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
    if document.active_representation() == clayspace_model::Representation::Voxel {
        let wall = document.scene().layers.last().expect("the wall row").key;
        document.set_active_layer(wall).expect("activate the wall");
    }
    spots
        .iter()
        .zip(before)
        .map(|(&d, b)| radius_along(&document, d) - b)
        .collect()
}

fn assert_within(spots: &[[f32; 3]], walls: &[f32], expected: f32, tolerance: f32, what: &str) {
    for (spot, wall) in spots.iter().zip(walls) {
        assert!(
            (wall - expected).abs() <= tolerance,
            "{what}: the wall at {spot:?} is {wall}, more than {tolerance} from {expected} \
             (every spot: {walls:?})"
        );
    }
}

fn assert_matches(walls: &[f32], expected: f32, what: &str) {
    assert_within(&SPOTS, walls, expected, 0.1 * expected, what);
}

fn spread(walls: &[f32]) -> f32 {
    let max = walls.iter().copied().fold(f32::MIN, f32::max);
    let min = walls.iter().copied().fold(f32::MAX, f32::min);
    max - min
}

// -- on a field layer ---------------------------------------------------------

#[test]
fn extrude_uses_its_thickness_on_an_outline_mask() {
    // v0.120.1: 0.05 → 0.050, 0.1 → ~0.1, and 0.6 stopped at ~0.11.
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
    // stood 0.13 against 0.08-0.10 in the middle. Measured: within 0.0001.
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
fn the_wall_is_even_along_the_mask_boundary() {
    // The issue's second criterion. Eight spots just inside the boundary: 0.02
    // inside the outline's square, corners and sides, and on the rim of the
    // dab's frozen core. Measured spread at 0.1 and 0.6: 0.0000 to 0.0009, so
    // the tolerance is a quarter of the mask's cell.
    let tolerance = 0.005;
    let checks: [(Vec<[f32; 3]>, Fixture, &str); 2] = [
        (square_ring(0.23), outlined, "outline"),
        (ring(0.10), dabbed, "dab"),
    ];
    for (spots, masked, what) in &checks {
        for thickness in [0.1, 0.6] {
            let measured = walls_at(spots, *masked, ExtrudeSide::Outward, thickness);
            assert!(
                spread(&measured) <= tolerance,
                "a {thickness} wall varies by {} along the {what}'s boundary: {measured:?}",
                spread(&measured)
            );
            assert_within(spots, &measured, thickness, 0.1 * thickness, what);
        }
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
fn a_thickness_past_what_the_engine_can_measure_is_refused() {
    // The engine measures the mask as a dense array over its bounds grown by
    // the thickness, so a wall of 100 on a patch this size would cost
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

// -- on a grid ----------------------------------------------------------------
//
// `clay_voxel_mask_extrude` grows the wall from each masked surface cell along
// an estimated normal, one cell per layer, for the number of layers the
// thickness rounds to. On the 0.02 grid every wall is read to within one cell,
// which is coarser than 10% of 0.05 and of 0.1 and finer than 10% of 0.6.

#[test]
fn extrude_uses_its_thickness_on_a_grid() {
    // v0.120.1 stopped a grid's wall at the paint depth too. Measured at the
    // crown now: 0.05 → 0.060 (three layers, the nearest whole number of
    // cells), 0.1 → 0.100, 0.6 → 0.600.
    for thickness in [0.05, 0.1, 0.6] {
        let measured = walls(gridded, ExtrudeSide::Outward, thickness);
        assert_within(
            &SPOTS[..1],
            &measured[..1],
            thickness,
            GRID_CELL,
            "grid, Para fora, at the crown",
        );
    }
    // Across the patch too while the wall is a few cells tall: the five spots
    // read 0.060 and 0.100 to within 0.0006.
    for thickness in [0.05, 0.1] {
        let measured = walls(gridded, ExtrudeSide::Outward, thickness);
        assert_within(&SPOTS, &measured, thickness, GRID_CELL, "grid, Para fora");
    }
}

#[test]
fn a_thin_grid_wall_is_even_along_the_mask_boundary() {
    // Around the dab's core at 0.1 and 0.2, measured spread 0.0006 and
    // 0.0043: half a cell is the tolerance.
    for thickness in [0.1, 0.2] {
        let measured = walls_at(&ring(0.07), gridded, ExtrudeSide::Outward, thickness);
        assert!(
            spread(&measured) <= 0.5 * GRID_CELL,
            "a {thickness} grid wall varies by {} around the dab: {measured:?}",
            spread(&measured)
        );
    }
}

/// A tripwire on an engine limitation, not a requirement.
///
/// Each seed's column is one cell wide and the columns follow normals that
/// diverge on a curved surface and are quantised to the grid, so past a few
/// layers they no longer fill the wall between them: at 0.6 the crown reads
/// 0.600 but the ring around the dab reads 0.35 to 0.41, and a ray through the
/// outline's patch finds holes down to 0.08. When a later engine fills the
/// wall this fails, and the limitation leaves `docs/features.md` with it.
#[test]
fn a_thick_grid_wall_is_still_porous() {
    let measured = walls_at(&ring(0.07), gridded, ExtrudeSide::Outward, 0.6);
    assert!(
        measured.iter().any(|wall| (wall - 0.6).abs() > 0.06),
        "the engine's 0.6 grid wall is even around the dab now ({measured:?}): \
         drop this tripwire and the limitation in docs/features.md"
    );
}
