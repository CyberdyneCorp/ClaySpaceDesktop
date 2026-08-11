//! Every brush in the shelf, drawn the way a sculptor draws it.
//!
//! `visual_sculpting` covers the same vocabulary by calling the engine
//! directly, which checks that the verbs exist and do something. This runs the
//! whole application path instead — pointer positions, ray, pick, live stroke
//! segments, incremental re-mesh, render — because that is where the last
//! several defects lived, and none of them were visible one layer down.
//!
//! Each tool writes a before and an after to `target/visual/brush-*`, and the
//! run prints a table of what each did and what it cost. Looking at that table
//! and those frames is the point; the assertions only catch the failures that
//! can be stated without eyes.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_brushes -- --nocapture
//! open target/visual
//! ```

mod support;

use std::time::{Duration, Instant};

use clayspace_app::{ray_at, SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{SculptModel, ToolKind};
use clayspace_view::{Camera, Image};
use clayspace_vm::{Command, SculptViewModel};
use support::Harness;

/// The viewport the shell leaves in a 1280×800 window.
fn viewport() -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(278.0, 92.0), egui::pos2(1032.0, 688.0))
}

/// What one tool did.
struct Outcome {
    tool: ToolKind,
    /// None when the tool refused, with its reason.
    refused: Option<String>,
    segments: usize,
    /// Share of subject pixels that changed.
    changed: f64,
    /// Longest single segment, which is what a sculptor feels.
    worst_segment: Duration,
    triangles: usize,
    /// Whether the model said the edit did anything.
    reported: bool,
    /// The worst single re-mesh: keys touched, meshing, uploading.
    worst_sync: Option<clayspace_app::SyncCost>,
}

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

/// Draws one arc across the front of the form, through the whole application
/// path, and reports what happened.
fn exercise(harness: &mut Harness, tool: ToolKind) -> Option<Outcome> {
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

    let name = format!("{tool:?}").to_lowercase();
    let background = harness.background();
    let before = harness.capture(
        geometry.mesh(),
        &camera,
        false,
        &format!("brush-{name}-before"),
    );

    if let Err(refusal) = vm.dispatch(Command::SelectTool(tool)) {
        return Some(Outcome {
            tool,
            refused: Some(format!("{refusal}")),
            segments: 0,
            changed: 0.0,
            worst_segment: Duration::ZERO,
            triangles: geometry.triangle_count(),
            reported: false,
            worst_sync: None,
        });
    }

    // A short arc across the middle of the viewport, as a drag would deliver
    // it: many small steps rather than a few large ones.
    let centre = viewport().center();
    let path: Vec<egui::Pos2> = (0..24)
        .map(|i| {
            let t = i as f32 / 23.0;
            centre + egui::vec2(-110.0 + t * 220.0, (t * std::f32::consts::PI).sin() * -40.0)
        })
        .collect();

    let mut refused = None;
    let mut segments = 0usize;
    let mut worst = Duration::ZERO;
    let mut began = false;
    let mut worst_sync: Option<clayspace_app::SyncCost> = None;

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
            Command::BeginStroke {
                position,
                pressure: 1.0,
            }
        };

        let started = Instant::now();
        match vm.dispatch(command) {
            Ok(()) => began = true,
            Err(e) => {
                refused = Some(format!("{e}"));
                break;
            }
        }
        // The re-mesh is part of what the sculptor waits for, so it is timed
        // with the edit rather than after it.
        let cost = document
            .with(|document| geometry.sync(&harness.gpu, document))
            .expect("re-mesh");
        if let Some(cost) = cost {
            if worst_sync
                .is_none_or(|w| cost.mesh_time + cost.upload_time > w.mesh_time + w.upload_time)
            {
                worst_sync = Some(cost);
            }
        }
        let elapsed = started.elapsed();
        if elapsed > worst {
            worst = elapsed;
        }
        segments += 1;
    }

    if began {
        vm.dispatch(Command::EndStroke).expect("end");
        document
            .with(|document| geometry.sync(&harness.gpu, document))
            .expect("final re-mesh");
    }

    let after = harness.capture(
        geometry.mesh(),
        &camera,
        false,
        &format!("brush-{name}-after"),
    );

    Some(Outcome {
        tool,
        refused,
        segments,
        changed: subject_change(&before, &after, background),
        worst_segment: worst,
        triangles: geometry.triangle_count(),
        reported: vm.last_action().get().changed,
        worst_sync,
    })
}

#[test]
fn every_brush_in_the_shelf_draws_something_worth_looking_at() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut outcomes = Vec::new();
    for tool in ToolKind::ALL {
        if let Some(outcome) = exercise(&mut harness, tool) {
            outcomes.push(outcome);
        }
    }
    if outcomes.is_empty() {
        return;
    }

    println!(
        "\n{:<12} {:>5} {:>9} {:>9} {:>6} {:>8} {:>8}  refusal",
        "tool", "segs", "changed", "worst ms", "keys", "mesh ms", "up ms"
    );
    for o in &outcomes {
        let (keys, mesh, up) = o.worst_sync.map_or((0, 0.0, 0.0), |c| {
            (
                c.keys,
                c.mesh_time.as_secs_f64() * 1000.0,
                c.upload_time.as_secs_f64() * 1000.0,
            )
        });
        println!(
            "{:<12} {:>5} {:>8.2}% {:>9.1} {:>6} {:>8.1} {:>8.1} {:>9}  {}",
            format!("{:?}", o.tool),
            o.segments,
            o.changed * 100.0,
            o.worst_segment.as_secs_f64() * 1000.0,
            keys,
            mesh,
            up,
            o.triangles,
            o.refused.as_deref().unwrap_or("")
        );
    }
    println!();

    // Máscara paints a freeze the *next* stroke reads, so it is defined not to
    // move anything. It has its own check below; everything else that draws
    // must draw.
    let moves_the_surface = |tool: ToolKind| tool != ToolKind::Mascara;

    // A tool the shelf offers on this layer either changes the surface or says
    // why it cannot. Doing neither is the failure that matters: a brush that
    // silently does nothing is indistinguishable from a broken one.
    let silent: Vec<&Outcome> = outcomes
        .iter()
        .filter(|o| o.refused.is_none() && moves_the_surface(o.tool) && o.changed < 0.001)
        .collect();
    assert!(
        silent.is_empty(),
        "these tools were accepted and changed nothing: {:?}. A brush must \\
         either work or refuse — see target/visual/brush-*.png",
        silent.iter().map(|o| o.tool).collect::<Vec<_>>()
    );

    // A brush a sculptor can feel, not one that technically deposits. The
    // relief tools were mapping `blend_k` as if it were a smoothing distance
    // when the engine reads it as the amplitude, and never set the rounding
    // that is the falloff width — measured, a full stroke moved a tenth of one
    // percent of the subject and looked untouched.
    for o in &outcomes {
        if o.refused.is_some() || !moves_the_surface(o.tool) {
            continue;
        }
        assert!(
            o.changed > 0.01,
            "{:?} moved only {:.2}% of the subject over a whole stroke, which \
             reads as a brush that barely works",
            o.tool,
            o.changed * 100.0
        );
    }

    // The mask is the exception that has to be stated: it must report that it
    // did something while moving nothing at all.
    if let Some(mask) = outcomes.iter().find(|o| o.tool == ToolKind::Mascara) {
        assert!(
            mask.refused.is_none(),
            "the mask tool refused: {:?}",
            mask.refused
        );
        assert!(
            mask.reported,
            "the mask painted nothing — it was mapped onto a deformation verb \
             once, and reporting no change is how that would look now"
        );
        assert!(
            mask.changed < 0.001,
            "the mask moved {:.2}% of the surface; freezing a region must not \
             sculpt it",
            mask.changed * 100.0
        );
    }

    // Every refusal has to be readable. An empty reason in the options bar
    // tells a sculptor only that the tool is broken.
    for o in &outcomes {
        if let Some(reason) = &o.refused {
            assert!(
                !reason.trim().is_empty(),
                "{:?} refused without saying why",
                o.tool
            );
        }
    }
}

/// Tools that stamp along the path. These are the ones a sculptor uses
/// continuously, and they have to keep up with a drag.
const STAMPING: [ToolKind; 3] = [ToolKind::Padrao, ToolKind::Inflar, ToolKind::Camada];

#[test]
fn no_brush_stalls_the_stroke() {
    // Per-segment cost, which is what a sculptor feels as lag while dragging.
    // Reported in debug and asserted only in release, for the same reason as
    // `sculpt_latency`: an unoptimised build measures the profile.
    //
    // Two groups, because they are genuinely different work. A stamping tool
    // adds a small item and dirties the bricks under it. Suavizar, Planar,
    // Polir, Relaxar, Mover and Puxar bake a region of the document into a
    // volume and replace it, which dirties everything that region covers — so
    // they cost several times as much and are held to a looser bound while
    // that stands. Their number is printed either way so it cannot drift
    // quietly.
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut measured = Vec::new();
    for tool in ToolKind::ALL {
        let Some(outcome) = exercise(&mut harness, tool) else {
            continue;
        };
        if outcome.refused.is_none() {
            measured.push((outcome.tool, outcome.worst_segment));
        }
    }
    if measured.is_empty() {
        return;
    }

    for (tool, worst) in &measured {
        println!(
            "{:<12} worst segment {:>7.1} ms{}",
            format!("{tool:?}"),
            worst.as_secs_f64() * 1000.0,
            if STAMPING.contains(tool) {
                ""
            } else {
                "   (bake-and-replace)"
            }
        );
    }

    if cfg!(debug_assertions) {
        return;
    }

    let ceiling = |tool: &ToolKind| {
        if STAMPING.contains(tool) {
            Duration::from_millis(100)
        } else {
            // Not a target, a fence. It is roughly where these sit today, so
            // it catches a regression without pretending the current cost is
            // acceptable — bringing them down is open work.
            Duration::from_millis(900)
        }
    };

    for (tool, worst) in &measured {
        assert!(
            *worst <= ceiling(tool),
            "{tool:?} took {:.1} ms for one segment of a stroke, which reads \
             as a stall while dragging",
            worst.as_secs_f64() * 1000.0
        );
    }
}
