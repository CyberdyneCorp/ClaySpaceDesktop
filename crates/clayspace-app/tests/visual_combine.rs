//! The combine operations and the blend profiles, drawn.
//!
//! The options bar's two new controls are the kind that read as working from
//! the code and do nothing in the picture: the setting is stored, a command is
//! dispatched, a stroke is applied, and whether the field ever saw the choice
//! is invisible one layer down. `clayspace-engine`'s `combine` test measures
//! the surface height under the stroke, which catches a dropped setting; this
//! runs the whole application path — command, live stroke segments,
//! incremental re-mesh, render — and leaves frames to look at.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_combine -- --nocapture
//! open target/visual
//! ```

mod support;

use clayspace_app::{ray_at, SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BlendProfile, Combine, CombineSettings, SculptModel, ToolKind};
use clayspace_view::{Camera, Image};
use clayspace_vm::{Command, SculptViewModel};
use support::Harness;

/// The viewport the shell leaves in a 1280×800 window.
fn viewport() -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(278.0, 92.0), egui::pos2(1032.0, 688.0))
}

/// Share of pixels that differ, of those either frame draws the subject in.
fn subject_change(before: &Image, after: &Image, background: [u8; 4]) -> f64 {
    let mut subject = 0usize;
    let mut moved = 0usize;
    for y in 0..before.height {
        for x in 0..before.width {
            let (a, b) = (before.pixel(x, y), after.pixel(x, y));
            let is_subject = (0..3).any(|c| a[c].abs_diff(background[c]) > 12)
                || (0..3).any(|c| b[c].abs_diff(background[c]) > 12);
            if !is_subject {
                continue;
            }
            subject += 1;
            if (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0) > 8 {
                moved += 1;
            }
        }
    }
    if subject == 0 {
        return 0.0;
    }
    moved as f64 / subject as f64
}

/// How many pixels two frames differ in outright, for comparing two *results*
/// rather than a result against its own start.
fn differing(a: &Image, b: &Image) -> usize {
    a.pixels
        .chunks_exact(4)
        .zip(b.pixels.chunks_exact(4))
        .filter(|(x, y)| (0..3).map(|c| x[c].abs_diff(y[c])).max().unwrap_or(0) > 8)
        .count()
}

/// One arc across the front of the starting form with the given settings, and
/// the frame it leaves.
fn draw_with(harness: &mut Harness, settings: CombineSettings, name: &str) -> Option<(Image, f64)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let document = SharedDocument::new(document);
    let mut vm = SculptViewModel::new(Box::new(document.clone()));

    let mut camera = Camera::default();
    match SculptModel::bounds(&document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    document
        .with(|document| geometry.rebuild(&harness.gpu, document))
        .expect("the first mesh");
    let background = harness.background();
    let before = harness.capture(
        geometry.mesh(),
        &camera,
        false,
        &format!("combine-{name}-before"),
    );

    vm.dispatch(Command::SelectTool(ToolKind::Padrao)).ok()?;
    // Through the command queue rather than by reaching into the model: the
    // point is that the options bar's own path carries the choice all the way
    // down, and a test that set it on the document directly would pass with
    // the command unhandled.
    vm.dispatch(Command::SetCombine(settings)).ok()?;
    // A brush wide enough that the operation is legible at this framing.
    vm.dispatch(Command::SetBrushSize(0.28)).ok()?;
    vm.dispatch(Command::SetBrushIntensity(1.0)).ok()?;

    let centre = viewport().center();
    let path: Vec<egui::Pos2> = (0..24)
        .map(|i| {
            let t = i as f32 / 23.0;
            centre + egui::vec2(-110.0 + t * 220.0, (t * std::f32::consts::PI).sin() * -40.0)
        })
        .collect();

    let mut began = false;
    for point in &path {
        let Some((origin, direction)) = ray_at(&camera, viewport(), *point) else {
            continue;
        };
        let Some(position) = vm.pick(origin, direction) else {
            continue;
        };
        let command = if began {
            Command::ContinueStroke {
                position,
                pressure: 1.0,
            }
        } else {
            began = true;
            Command::BeginStroke {
                position,
                pressure: 1.0,
            }
        };
        let _ = vm.dispatch(command);
        document
            .with(|document| geometry.sync(&harness.gpu, document))
            .ok();
    }
    let _ = vm.dispatch(Command::EndStroke);
    document
        .with(|document| geometry.rebuild(&harness.gpu, document))
        .ok();

    let after = harness.capture(
        geometry.mesh(),
        &camera,
        false,
        &format!("combine-{name}-after"),
    );
    let changed = subject_change(&before, &after, background);
    Some((after, changed))
}

/// Every operation, drawn, with a table of what each one moved.
///
/// The assertions hold what can be stated without eyes; the frames are what
/// the change is actually reviewed from.
#[test]
fn each_combine_operation_leaves_its_own_mark() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut frames: Vec<(Combine, Image, f64)> = Vec::new();
    for op in Combine::offered_for_strokes() {
        // The operations whose effect *is* the distance get one; the model
        // knows which those are and clamps a zero up, so this is the same
        // value the options bar would arrive at.
        let settings = CombineSettings {
            op,
            blend: BlendProfile::Quadratic,
            radius: if op.needs_a_distance() { 0.12 } else { 0.0 },
        }
        .sanitized();
        let name = format!("{op:?}").to_lowercase();
        let Some((frame, changed)) = draw_with(&mut harness, settings, &name) else {
            return;
        };
        println!(
            "{:>12}  changed {:>6.2}% of the subject",
            op.label(),
            changed * 100.0
        );
        frames.push((op, frame, changed));
    }

    // Every operation the options bar offers has to do something. Paint is
    // the one that would not, and it is not offered — see
    // `Combine::offered_for_strokes`.
    for (op, _, changed) in &frames {
        assert!(
            *changed > 0.001,
            "{} moved {:.4}% of the subject, which is nothing — either the \
             operation is not reaching the field, or it should not be on the \
             shelf",
            op.label(),
            changed * 100.0
        );
    }

    // And no two operations may draw the same picture: an adapter that mapped
    // several of them onto one op would pass every per-operation check and
    // still give a sculptor one shape under fourteen names.
    for (i, (a, frame_a, _)) in frames.iter().enumerate() {
        for (b, frame_b, _) in frames.iter().skip(i + 1) {
            assert!(
                differing(frame_a, frame_b) > 200,
                "{} and {} drew the same surface, so at least one of them is \
                 mapped onto the other",
                a.label(),
                b.label()
            );
        }
    }
}

/// The profiles, on the operation whose join is easiest to read.
#[test]
fn each_blend_profile_rounds_the_join_differently() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut frames: Vec<(BlendProfile, Image)> = Vec::new();
    for blend in BlendProfile::ALL {
        let settings = CombineSettings {
            op: Combine::Subtract,
            blend,
            // A join wide enough to have a shape. With no radius every
            // profile is the same hard minimum and the comparison would say
            // nothing.
            radius: 0.25,
        };
        let name = format!("blend-{blend:?}").to_lowercase();
        let Some((frame, changed)) = draw_with(&mut harness, settings, &name) else {
            return;
        };
        println!(
            "{:>12}  changed {:>6.2}% of the subject",
            blend.label(),
            changed * 100.0
        );
        frames.push((blend, frame));
    }

    for (i, (a, frame_a)) in frames.iter().enumerate() {
        for (b, frame_b) in frames.iter().skip(i + 1) {
            assert!(
                differing(frame_a, frame_b) > 100,
                "the {} join and the {} join drew the same surface, so the \
                 profile is being dropped between the options bar and the field",
                a.label(),
                b.label()
            );
        }
    }
}
