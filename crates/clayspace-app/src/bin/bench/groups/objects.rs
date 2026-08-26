//! What a placed object costs: putting one there, moving it, and finding it.
//!
//! The figure that matters here is `object.drag_frame`. A placed object is a
//! *live* operand — the boolean is re-evaluated from where the object now
//! stands — so a drag pays for a re-evaluation on every frame of it, and
//! whether that is affordable is the whole question of whether the feature is
//! usable rather than merely present.
//!
//! It is measured twice, because one of the fourteen operations is
//! categorically more expensive than the other thirteen. The engine drops a
//! node's finite influence bound for "a non-local op (intersect, the spatial
//! morphs) anywhere in the subtree", so an ordinary cube placed with
//! `Intersect` dirties the whole layer every frame while the same cube
//! subtracting dirties its own box. That is not a fault in this application
//! and it is not visible from the interface, which is exactly why it is worth
//! a figure of its own.
//!
//! `object.pick` is here for a different reason. The attributing raycast "is
//! not the cheap path — it compiles the document, then one tape per layer and
//! one per candidate item", and the application only calls it on a press. This
//! is what stops that decision quietly becoming wrong.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Combine, CombineSettings, GizmoDrag, GizmoHandle, GizmoMode, GizmoTarget, ObjectId,
    ObjectModel, Shape,
};
use clayspace_view::Gpu;

use crate::figures::{ms, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

/// A shape big enough to reach the surface it is cutting, and small enough
/// that its box is not the whole form.
const CUT: [f32; 2] = [0.25, 1.6];

fn subtracting() -> CombineSettings {
    CombineSettings {
        op: Combine::Subtract,
        ..CombineSettings::default()
    }
}

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("object", Skip::NoHeadlessGpu);
    };

    one(
        &gpu,
        policy,
        run,
        "object.place",
        |document, screen, gpu| {
            document
                .place_object(Shape::Cylinder, &CUT, [0.0, 0.9, 0.0], subtracting())
                .map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, document)
        },
    );

    one(
        &gpu,
        policy,
        run,
        "object.remove",
        |document, screen, gpu| {
            let id = placed(document)?;
            document.remove_object(id).map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, document)
        },
    );

    one(
        &gpu,
        policy,
        run,
        "object.re_op",
        |document, screen, gpu| {
            let id = placed(document)?;
            document
                .set_object_combine(id, CombineSettings::default())
                .map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, document)
        },
    );

    one(
        &gpu,
        policy,
        run,
        "object.re_shape",
        |document, screen, gpu| {
            let id = placed(document)?;
            document
                .set_object_shape(id, Shape::Box, &Shape::Box.defaults())
                .map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, document)
        },
    );

    pick(&gpu, policy, run);

    drag_frames(&gpu, policy, run, "object.drag_frame", Combine::Subtract);
    // The same drag on the operation with no finite influence bound.
    drag_frames(
        &gpu,
        policy,
        run,
        "object.drag_frame_intersect",
        Combine::Intersect,
    );
}

/// What finding an object under the pointer costs.
///
/// Its own fixture, because the object has to be the thing the ray attributes
/// to. Two earlier versions of this measured nothing: one cast down the bore's
/// own axis, which passes through the hole and meets no surface at all, and
/// one cast at the worked form, where the nearest field is a *stroke stamp* —
/// correctly not an object, so the pick answered None and the figure was
/// skipped.
///
/// So the fixture places a lump standing proud of the surface and aims at
/// that. The figure is the cost of the attributing raycast, which "compiles
/// the document, then one tape per layer and one per candidate item" — the
/// reason the application only makes this call on a press.
fn pick(gpu: &Gpu, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("object.pick") {
        return;
    }
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| {
            let mut document = Scene::Reference
                .build(policy.clone())
                .map_err(|_| Skip::SceneWouldNotBuild)?;
            let mut screen = Screen::new(gpu);
            screen.prime(gpu, &mut document)?;
            // Adding, and clear of the form's own surface, so the ray meets it
            // before anything else.
            document
                .place_object(
                    Shape::Sphere,
                    &[0.45],
                    [0.0, 1.15, 0.0],
                    CombineSettings::default(),
                )
                .map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, &mut document)?;

            let started = Instant::now();
            let found = document.pick_object([0.0, 4.0, 0.0], [0.0, -1.0, 0.0]);
            let took = ms(started.elapsed());
            found.map(|_| took).ok_or(Skip::NoSurfaceUnderProbe)
        })
        .collect();
    match samples {
        Ok(samples) => run.timings("object.pick", Record::OneShot, samples),
        Err(why) => run.skip("object.pick", why),
    }
}

/// The object the fixture placed.
fn placed(document: &mut ClayDocument) -> Result<ObjectId, Skip> {
    document
        .objects()
        .last()
        .map(|object| object.id)
        .ok_or(Skip::EditRefused)
}

/// A one-shot on a document rebuilt for it, with an object already placed.
fn one(
    gpu: &Gpu,
    policy: &BackendPolicy,
    run: &mut Run,
    prefix: &str,
    what: fn(&mut ClayDocument, &mut Screen, &Gpu) -> Result<(), Skip>,
) {
    if !run.wants_group(prefix) {
        return;
    }
    // `object.place` measures placing into a scene with nothing placed; the
    // rest need something to act on.
    let seed = prefix != "object.place";
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| {
            let (mut document, mut screen) = arrange(gpu, policy, seed, Combine::Subtract)?;
            let started = Instant::now();
            what(&mut document, &mut screen, gpu)?;
            Ok(ms(started.elapsed()))
        })
        .collect();
    match samples {
        Ok(samples) => run.timings(prefix, Record::OneShot, samples),
        Err(why) => run.skip(prefix, why),
    }
}

/// One frame of a live boolean drag, repeatedly.
///
/// Repeatable rather than one-shot: the frames of a drag *are* a sequence of
/// different costs, exactly as a stroke's segments are, so the mean is what a
/// sculptor pays for the gesture.
fn drag_frames(gpu: &Gpu, policy: &BackendPolicy, run: &mut Run, prefix: &str, op: Combine) {
    if !run.wants_group(prefix) {
        return;
    }
    let (mut document, mut screen) = match arrange(gpu, policy, true, op) {
        Ok(ready) => ready,
        Err(why) => return run.skip(prefix, why),
    };
    let Ok(id) = placed(&mut document) else {
        return run.skip(prefix, Skip::EditRefused);
    };
    let target = GizmoTarget::Object(id);
    let Some(start) = document.target_transform(target) else {
        return run.skip(prefix, Skip::EditRefused);
    };
    let gesture = GizmoDrag {
        mode: GizmoMode::Move,
        handle: GizmoHandle::Axis(0),
        pivot: start.position,
        anchor: start.position,
        view_axis: [0.0, 0.0, 1.0],
    };

    document.begin_target_drag(target);
    let samples: Result<Vec<f64>, Skip> = (0..Record::Repeatable.samples())
        .map(|step| {
            // Across the form and back, which is what a hand aiming a hole
            // does — and which keeps the object over surface rather than
            // walking it off into empty space where a frame costs nothing.
            let t = step as f32 / Record::Repeatable.samples() as f32;
            let to = [(t * std::f32::consts::TAU).sin() * 0.7, 0.9, 0.0];
            let moved = gesture.resolve(start, to, false);
            let started = Instant::now();
            document
                .set_target_transform(target, moved)
                .map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, &mut document)?;
            Ok(ms(started.elapsed()))
        })
        .collect();
    document.end_target_drag();

    match samples {
        Ok(samples) => run.timings(prefix, Record::Repeatable, samples),
        Err(why) => run.skip(prefix, why),
    }
}

/// The reference scene, meshed, optionally with an object already in it.
fn arrange(
    gpu: &Gpu,
    policy: &BackendPolicy,
    seed: bool,
    op: Combine,
) -> Result<(ClayDocument, Screen), Skip> {
    let mut document = Scene::Reference
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;
    if seed {
        document
            .place_object(
                Shape::Cylinder,
                &CUT,
                [0.0, 0.9, 0.0],
                CombineSettings {
                    op,
                    ..CombineSettings::default()
                },
            )
            .map_err(|_| Skip::EditRefused)?;
        screen.refresh(gpu, &mut document)?;
    }
    Ok((document, screen))
}
