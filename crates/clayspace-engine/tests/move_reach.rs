//! How far a Move drag reaches on a FIELD, for comparison with a grid.
//!
//! The voxel grab is an inverse map with finite support and its bulge saturates
//! at the brush radius: at four cells of radius, asking for sixteen cells of
//! lift rose four (#139). The question this answers is whether that is a
//! property of the application's Move or only of the grid's verb — if a field
//! drag reaches as far as it is asked, the two representations disagree about
//! what a drag means.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

const RADIUS: f32 = 0.18;

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far the surface stands from the origin along +x.
fn radius_along(document: &ClayDocument) -> f32 {
    let hit = document
        .pick([4.0, 0.0, 0.0], [-1.0, 0.0, 0.0])
        .expect("the ray reaches the surface");
    hit.iter().map(|c| c * c).sum::<f32>().sqrt()
}

#[test]
fn a_field_drag_reaches_as_far_as_it_is_asked() {
    for pull in [0.05f32, 0.1, 0.2, 0.4, 0.8] {
        let mut document = sphere();
        let rest = radius_along(&document);
        let samples: Vec<GestureSample> = (0..=8)
            .map(|step| {
                let t = step as f32 / 8.0;
                GestureSample {
                    position: [rest + t * pull, 0.0, 0.0],
                    pressure: 1.0,
                    time: t,
                }
            })
            .collect();
        SculptModel::begin_gesture(&mut document);
        document
            .apply_stroke(
                ToolKind::Mover,
                BrushSettings {
                    size: RADIUS,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &samples,
                [false; 3],
            )
            .expect("the drag was refused");
        SculptModel::end_gesture(&mut document);
        let moved = radius_along(&document) - rest;
        eprintln!(
            "field: asked {pull:.2}, surface moved {moved:.4}  ({:.0}% of the ask, \
             radius {RADIUS})",
            moved / pull * 100.0
        );
    }
}
