//! Move Topológico on a field: a long drag lands whole, without a tear.
//!
//! The engine re-samples the baked volume through the inverse of the move,
//! and that inverse folds once the displacement outruns the falloff: two
//! points of the result read the same point of the source and the surface
//! between them tears. The drag is applied as a series of short steps, each
//! anchored where the last one carried the material, and with a crossfade
//! band wide enough to express the whole move.
//!
//! ```sh
//! cargo test -p clayspace-engine --test topological_move
//! ```

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, MaskModel, SculptModel, ToolKind};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// `n + 1` samples along `at(t)` for `t` in `[0, 1]`.
fn along(n: usize, at: impl Fn(f32) -> [f32; 3]) -> Vec<GestureSample> {
    (0..=n)
        .map(|step| {
            let t = step as f32 / n as f32;
            GestureSample {
                position: at(t),
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// A drag out of the top of the sphere, 0.64 long, twice the brush.
fn the_drag() -> Vec<GestureSample> {
    along(24, |t| [t * 0.5, 0.0, 1.0 + t * 0.4])
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.3,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// Delivered whole, as the interface holds a topological drag on a field.
fn drag(document: &mut ClayDocument) {
    document.begin_gesture();
    document
        .apply_stroke(ToolKind::MoverTopologico, brush(), &the_drag(), [false; 3])
        .expect("the drag was refused");
    document.end_gesture();
}

/// Where the surface stands, looking straight down at `(x, y)`.
fn top_at(document: &ClayDocument, x: f32, y: f32) -> f32 {
    document
        .pick([x, y, 4.0], [0.0, 0.0, -1.0])
        .map(|hit| hit[2])
        .unwrap_or(f32::NAN)
}

/// The surface meshed and checked for holes and pinches.
fn report(document: &ClayDocument) -> claycore::ValidationReport {
    document
        .document()
        .mesh(claycore::MeshParams {
            voxel_size: Some(ClayDocument::VOXEL_SIZE),
            resolution: 128,
            decimate_ratio: None,
            mesher: claycore::Mesher::MarchingTetrahedra,
        })
        .expect("mesh the document")
        .validation_report(0)
        .expect("validate the mesh")
}

#[test]
fn a_long_topological_drag_rises_steadily_to_its_end() {
    // Measured before the fix: one move of the whole drag folded, and the
    // pulled lump had a crater in its middle — heights along the drag of
    // 1.16, 1.00, 0.96, 0.98 and back up to 1.27.
    let mut document = document();
    drag(&mut document);
    let heights: Vec<f32> = (0..=18)
        .map(|i| top_at(&document, i as f32 * 0.025, 0.0))
        .collect();
    println!("heights {heights:?}");
    for (i, pair) in heights.windows(2).enumerate() {
        assert!(
            pair[1] >= pair[0] - 2e-3,
            "the surface dips from {:.4} to {:.4} at x = {:.3} along the drag, \
             which is a fold rather than a pull",
            pair[0],
            pair[1],
            (i + 1) as f32 * 0.025
        );
    }
    assert!(
        heights[18] > 1.3,
        "the drag carried the surface only to {:.4}, short of where it was \
         dragged",
        heights[18]
    );
}

#[test]
fn a_topological_drag_leaves_a_closed_surface() {
    let mut document = document();
    drag(&mut document);
    let report = report(&document);
    assert!(
        report.watertight && report.manifold,
        "the drag tore the surface: {report:?}"
    );
    assert_eq!(
        report.euler_characteristic, 2,
        "the drag left a spur or a hole rather than one closed lump: {report:?}"
    );
}

#[test]
fn a_topological_drag_across_a_mask_leaves_the_band_and_the_surface_whole() {
    // A band masked across the path the drag takes, beside its anchor.
    let mut document = document();
    let band = along(8, |t| {
        let y = (t - 0.5) * 1.2;
        [0.15, y, (1.0 - y * y - 0.0225).sqrt()]
    });
    let painted = BrushSettings {
        size: 0.15,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    document
        .apply_stroke(ToolKind::Mascara, painted, &band, [false; 3])
        .expect("the mask stroke was refused");
    assert!(document.mask_state().present, "the mask painted nothing");
    // Off the lump's reach across y, where only the band's own surface is.
    let before = top_at(&document, 0.15, 0.2);
    drag(&mut document);
    let after = top_at(&document, 0.15, 0.2);
    assert!(
        (after - before).abs() < 5e-3,
        "the masked band moved from {before:.4} to {after:.4}"
    );
    let report = report(&document);
    assert!(
        report.watertight && report.manifold,
        "the drag across a mask tore the surface: {report:?}"
    );
}
