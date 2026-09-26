//! Borda on a field: each falloff lays down a different profile.
//!
//! A mesh and a grid take the falloff curve by name. A field stroke stamps an
//! item, which has no curve, and the setting used to be read and dropped: every
//! falloff produced the same affected area to the pixel (#178). It now reaches
//! the stamp as the width of its rim, which is what shapes the profile.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Falloff, GestureSample, SculptModel, ToolKind};

/// Distance from the centre of the starting sphere to its surface, looking in
/// from `angle` radians off +z in the xz plane.
fn radius(document: &ClayDocument, angle: f32) -> f32 {
    let direction = [angle.sin(), 0.0, angle.cos()];
    let hit = SculptModel::pick(document, direction.map(|c| c * 4.0), direction.map(|c| -c))
        .expect("the sphere is under the ray");
    hit.iter().map(|c| c * c).sum::<f32>().sqrt()
}

/// How far one Padrão dab at the pole raised the surface, from the centre of
/// the dab outward in steps of 0.025 rad.
fn profile(falloff: Falloff) -> Vec<f32> {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let rest = radius(&document, 0.0);
    let mut brush = BrushSettings {
        size: 0.18,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    brush.shaping.falloff = falloff;
    let dab = GestureSample {
        position: [0.0, 0.0, rest],
        pressure: 1.0,
        time: 0.0,
    };
    document
        .apply_stroke(ToolKind::Padrao, brush, &[dab], [false; 3])
        .expect("the dab");
    (0..10)
        .map(|step| radius(&document, 0.025 * step as f32) - rest)
        .collect()
}

/// How far out the dab moved the surface by more than a hair.
fn reach(profile: &[f32]) -> usize {
    profile.iter().take_while(|lift| **lift > 1e-3).count()
}

#[test]
fn falloff_changes_the_profile() {
    let profiles: Vec<Vec<f32>> = Falloff::ALL.into_iter().map(profile).collect();
    for (i, a) in profiles.iter().enumerate() {
        for b in &profiles[i + 1..] {
            let apart = a
                .iter()
                .zip(b)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0f32, f32::max);
            assert!(
                apart > 5e-3,
                "two falloffs laid down the same dab: {a:?} against {b:?}"
            );
        }
    }

    // And in the order the names promise: a hard edge stops soonest, a
    // Gaussian skirt reaches furthest.
    let [hard, linear, smooth, gaussian] = [0, 1, 2, 3].map(|i| reach(&profiles[i]));
    assert!(
        hard < smooth && linear <= smooth && smooth < gaussian,
        "reaches out of order: Dura {hard}, Linear {linear}, Suave {smooth}, \
         Gaussiana {gaussian}"
    );
}
