//! Whether a mirrored Move drag pulls each side once or twice.
//!
//! The application reflects a gesture itself and calls the verb once per image
//! — `baked_stroke`'s mirror loop — because through v0.52.2 the engine's own
//! `clay_layer_move_surface` said nothing about a layer mirror. In v0.60.0 it
//! does: "UNDER A LAYER MIRROR OR RADIAL SYMMETRY the drag is reflected and
//! rotated into every image the layer emits of it". Two reflections of one
//! gesture is two pulls, and a doubled pull is symmetric — so it cannot be
//! caught by comparing the two sides against each other. It is caught by
//! comparing a mirrored drag against an unmirrored one.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far the surface stands from the origin along `direction`.
fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> Option<f32> {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    let origin = unit.map(|c| c * 4.0);
    let hit = document.pick(origin, unit.map(|c| -c))?;
    Some(hit.iter().map(|c| c * c).sum::<f32>().sqrt())
}

/// The path of the drag, from its anchor to `step` of six.
fn path(step: usize) -> Vec<GestureSample> {
    (0..=step)
        .map(|i| {
            let t = i as f32 / 6.0;
            GestureSample {
                position: [1.0 + t * 0.25, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// The same drag, made live: opened, sent in segments, committed.
///
/// The live path does NOT run `baked_stroke`'s mirror loop — `clay_sdf_move_*`
/// reflects the drag into every image the layer emits and resolves one grab per
/// image, so reflecting the gesture again would be the doubling this file
/// exists to catch, reached by the other door.
fn live_drag(document: &mut ClayDocument, symmetry: [bool; 3]) {
    assert!(
        document.open_live_gesture(ToolKind::Mover, symmetry),
        "an editable field subtool takes a live drag"
    );
    for step in 1..=6 {
        document
            .apply_stroke(
                ToolKind::Mover,
                BrushSettings {
                    size: 0.35,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &path(step),
                symmetry,
            )
            .expect("the drag was refused");
    }
    document.close_live_gesture().expect("commit");
}

/// A drag outward at the +x limb.
fn drag(document: &mut ClayDocument, symmetry: [bool; 3]) {
    let samples: Vec<GestureSample> = (0..=6)
        .map(|step| {
            let t = step as f32 / 6.0;
            GestureSample {
                position: [1.0 + t * 0.25, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                size: 0.35,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            symmetry,
        )
        .expect("the drag was refused");
}

#[test]
fn a_mirrored_drag_pulls_each_side_once() {
    let mut plain = sphere();
    let rest = radius_along(&plain, [1.0, 0.0, 0.0]).expect("the rest surface");
    drag(&mut plain, [false; 3]);
    let unmirrored = radius_along(&plain, [1.0, 0.0, 0.0]).expect("the dragged surface") - rest;

    let mut mirrored = sphere();
    drag(&mut mirrored, [true, false, false]);
    let near = radius_along(&mirrored, [1.0, 0.0, 0.0]).expect("the near side") - rest;
    let far = radius_along(&mirrored, [-1.0, 0.0, 0.0]).expect("the far side") - rest;

    eprintln!(
        "unmirrored +{unmirrored:.4}; mirrored near +{near:.4}, far +{far:.4} \
         (near/unmirrored {:.2}x)",
        near / unmirrored.max(1e-6)
    );

    // Both sides move: that is what symmetry is for.
    assert!(
        far > unmirrored * 0.5,
        "the mirror did not reach the far side: +{far:.4} against +{unmirrored:.4}"
    );
    // And each side is pulled as far as one drag pulls, not as far as two.
    // The application reflects the gesture and the engine reflects it again,
    // so a doubled pull is what this exists to catch — and it is symmetric,
    // which is why the far side alone cannot catch it.
    assert!(
        near < unmirrored * 1.5,
        "a mirrored drag pulled the near side +{near:.4} where an unmirrored \
         one pulls +{unmirrored:.4} — {:.2}x, which is the gesture applied \
         twice: once by the application's own mirror loop and once by the \
         engine's",
        near / unmirrored.max(1e-6)
    );
}

#[test]
fn a_mirrored_live_drag_pulls_each_side_once() {
    // The same claim for the transactional path, which mirrors by a different
    // mechanism: the engine reflects the drag itself and the application does
    // not reflect the gesture, where the held path does the opposite.
    let mut plain = sphere();
    let rest = radius_along(&plain, [1.0, 0.0, 0.0]).expect("the rest surface");
    live_drag(&mut plain, [false; 3]);
    let unmirrored = radius_along(&plain, [1.0, 0.0, 0.0]).expect("the dragged surface") - rest;

    let mut mirrored = sphere();
    live_drag(&mut mirrored, [true, false, false]);
    let near = radius_along(&mirrored, [1.0, 0.0, 0.0]).expect("the near side") - rest;
    let far = radius_along(&mirrored, [-1.0, 0.0, 0.0]).expect("the far side") - rest;

    eprintln!("live: unmirrored +{unmirrored:.4}; mirrored near +{near:.4}, far +{far:.4}");
    assert!(
        far > unmirrored * 0.5,
        "the mirror did not reach the far side: +{far:.4} against +{unmirrored:.4}"
    );
    assert!(
        near < unmirrored * 1.5,
        "a mirrored live drag pulled the near side +{near:.4} where an \
         unmirrored one pulls +{unmirrored:.4} — the drag is being reflected \
         twice, once by the application and once by the engine"
    );
}
