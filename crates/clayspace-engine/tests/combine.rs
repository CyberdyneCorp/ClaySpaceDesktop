//! What an SDF edit does when it meets the surface already there.
//!
//! An SDF layer is the one representation where the *operation* is a separate
//! choice from the tool: the same drag can raise the surface, cut into it,
//! subtract a shape, or cut a channel along the join, and until this landed the
//! adapter hardcoded one of them. These tests hold the choice actually
//! reaching the field — measured by where the surface ends up, because a call
//! that is made and ignored looks exactly like a call that is not made.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BlendProfile, BrushSettings, Combine, CombineSettings, GestureSample, SculptModel,
};

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// Where the surface is along +Z, measured by raycast from outside.
///
/// The starting form is a unit sphere at the origin, so this is about 1.0
/// before anything is done to it, larger where a stroke has raised it and
/// smaller where one has cut in.
fn surface_height(doc: &ClayDocument) -> Option<f32> {
    doc.pick([0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .map(|hit| hit[2])
}

/// A short drag across the top of the sphere.
fn stroke(doc: &mut ClayDocument) {
    let brush = BrushSettings {
        size: 0.35,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    let samples: Vec<GestureSample> = (0..5)
        .map(|i| {
            let t = i as f32 / 4.0;
            GestureSample {
                position: [(t - 0.5) * 0.3, 0.0, 1.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    let _ = doc.apply_stroke(
        clayspace_model::ToolKind::Padrao,
        brush,
        &samples,
        [false; 3],
    );
}

/// The whole point of the control: the same tool and the same drag, and the
/// surface goes the other way.
#[test]
fn subtracting_takes_material_where_relief_adds_it() {
    let Some(mut raised) = document() else {
        return;
    };
    let start = surface_height(&raised).expect("the starting form is in the way of the ray");

    raised.set_combine(CombineSettings::for_strokes());
    stroke(&mut raised);
    let after_relief = surface_height(&raised).expect("still a surface after raising it");

    let Some(mut cut) = document() else {
        return;
    };
    cut.set_combine(CombineSettings {
        op: Combine::Subtract,
        blend: BlendProfile::Quadratic,
        radius: 0.0,
    });
    stroke(&mut cut);
    let after_subtract = surface_height(&cut).expect("still a surface after cutting into it");

    assert!(
        after_relief > start,
        "relief left the surface at {after_relief}, no higher than the {start} it started at"
    );
    assert!(
        after_subtract < start,
        "subtract left the surface at {after_subtract}, no lower than the {start} it started at"
    );
}

/// A setting the adapter accepts and drops reads as working until somebody
/// measures it, which is what this does: the two profiles must not produce the
/// same surface.
#[test]
fn the_blend_profile_reaches_the_field() {
    let mut heights = Vec::new();
    for blend in [BlendProfile::Hard, BlendProfile::Circular] {
        let Some(mut doc) = document() else {
            return;
        };
        doc.set_combine(CombineSettings {
            op: Combine::Subtract,
            blend,
            // A join to round: with no radius at all every profile is the
            // same shape and the comparison would be vacuous.
            radius: 0.3,
        });
        stroke(&mut doc);
        heights.push(surface_height(&doc).expect("a surface"));
    }
    assert_ne!(
        heights[0], heights[1],
        "a hard join and a circular one produced the same surface, so the \
         profile is being dropped somewhere between the options bar and the field"
    );
}

/// Paint is the operation that colours without reshaping, and an interface
/// that offered it as a way to move the surface would be lying about it.
#[test]
fn painting_leaves_the_surface_where_it_was() {
    let Some(mut doc) = document() else {
        return;
    };
    let start = surface_height(&doc).expect("a surface");
    doc.set_combine(CombineSettings {
        op: Combine::Paint,
        ..Default::default()
    });
    stroke(&mut doc);
    let after = surface_height(&doc).expect("a surface");
    assert!(
        (after - start).abs() < 1e-3,
        "paint moved the surface from {start} to {after}"
    );
}

/// The setting is state, so it has to be the state that is read back — a
/// setter that stores a sanitized value and a getter that returns the raw one
/// would let the options bar disagree with what the next stroke does.
#[test]
fn the_setting_is_read_back_sanitized() {
    let Some(mut doc) = document() else {
        return;
    };
    doc.set_combine(CombineSettings {
        op: Combine::Replace,
        blend: BlendProfile::Circular,
        radius: 0.4,
    });
    assert_eq!(
        doc.combine().radius,
        0.0,
        "replace makes no join, so the radius it was given should have been dropped"
    );
    assert_eq!(doc.combine().op, Combine::Replace);
}
