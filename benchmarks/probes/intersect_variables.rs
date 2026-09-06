//! Separating three variables in ClayCore #471's per-brick slope.
//!
//! `intersect_scaling` measured a subtract control whose brick count is pinned
//! (6,424 in both scenes) and whose per-brick cost still grew 3.4x. Two things
//! could produce that and the original fixture confounds them:
//!
//! - **Dab size.** `Scene::TenTimesLarger` scales the brush with the model —
//!   `0.18 * radius`, so 0.180 to 0.569 — while the brick lattice is fixed at
//!   8³ × 0.02. So a brick sits inside more overlapping items in the larger
//!   scene, and a culled tape keeps more of them.
//! - **Where the cutter sits.** `CUT` is fixed at `[0.0, 0.9, 0.0]`, so it
//!   straddles the surface of a radius-1 form and is buried deep inside a
//!   radius-√10 one. The same bricks in world space are near-surface bricks in
//!   one scene and interior bricks in the other.
//!
//! This builds the scene by hand so both can be varied independently.
//!
//! ```sh
//! cargo test -p clayspace-app --release --test intersect_variables -- --nocapture
//! ```

mod support;

use std::time::{Duration, Instant};

use clayspace_app::{SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, GestureSample, GizmoDrag, GizmoHandle, GizmoMode,
    GizmoTarget, ObjectModel, SculptModel, Shape,
};
use support::Harness;

const CUT: [f32; 2] = [0.25, 1.6];
const FRAMES: usize = 12;
const STROKES: usize = 8;
const SAMPLES_PER_STROKE: usize = 12;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
    v[v.len() / 2]
}

/// The reference construction, with the radius and the dab size given apart.
///
/// `Scene::build_sdf` ties them together — this is the same eight strokes of
/// twelve samples over a sphere, with the brush as a parameter.
fn worked_sphere(policy: BackendPolicy, radius: f32, dab: f32) -> Option<ClayDocument> {
    let mut document = ClayDocument::new(policy).ok()?;
    document.add_starting_sphere(radius).ok()?;
    let brush = BrushSettings {
        size: dab,
        ..BrushSettings::default()
    };
    for stroke in 0..STROKES {
        let band = (stroke as f32 / STROKES as f32 - 0.5) * 1.2;
        let samples: Vec<GestureSample> = (0..SAMPLES_PER_STROKE)
            .map(|i| {
                let t = i as f32 / (SAMPLES_PER_STROKE - 1) as f32;
                let angle = (t - 0.5) * 1.4;
                let (s, c) = angle.sin_cos();
                let (sb, cb) = band.sin_cos();
                GestureSample {
                    position: [
                        s * cb * radius * 1.01,
                        sb * radius * 1.01,
                        c * cb * radius * 1.01,
                    ],
                    pressure: 1.0,
                    time: t,
                }
            })
            .collect();
        document
            .apply_stroke(clayspace_model::ToolKind::Padrao, brush, &samples, [false; 3])
            .ok()?;
    }
    Some(document)
}

struct Run {
    refill: f64,
    refilled: u64,
    surface_bricks: u64,
}

/// One drag, at a cutter height given in units of the form's radius.
fn drag(
    harness: &Harness,
    radius: f32,
    dab: f32,
    at_surface: bool,
    op: Combine,
) -> Option<Run> {
    let policy = BackendPolicy::discover(None).ok()?;
    let document = SharedDocument::new(worked_sphere(policy, radius, dab)?);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    document.with(|d| geometry.rebuild(&harness.gpu, d)).ok()?;

    // 0.9 of the way out, so the cutter straddles the surface at any radius;
    // or the original fixed height, which buries it as the form grows.
    let height = if at_surface { 0.9 * radius } else { 0.9 };
    document
        .with(|d| {
            d.place_object(
                Shape::Cylinder,
                &CUT,
                [0.0, height, 0.0],
                CombineSettings {
                    op,
                    ..CombineSettings::default()
                },
            )
        })
        .ok()?;
    document.with(|d| geometry.sync(&harness.gpu, d)).ok()?;

    let id = document.with(|d| d.objects().last().map(|o| o.id))?;
    let target = GizmoTarget::Object(id);
    let start = document.with(|d| d.target_transform(target))?;
    let gesture = GizmoDrag {
        mode: GizmoMode::Move,
        handle: GizmoHandle::Axis(0),
        pivot: start.position,
        anchor: start.position,
        view_axis: [0.0, 0.0, 1.0],
    };

    let before = document.with(|d| d.document().resume_stats().ok())?;
    let surface_bricks = document
        .with(|d| d.cache().stats().ok())
        .map(|s| s.surface_bricks)
        .unwrap_or(0);

    document.with(|d| d.begin_target_drag(target));
    let mut refills = Vec::new();
    for step in 0..FRAMES {
        let t = step as f32 / FRAMES as f32;
        // The same sweep, in units of the form, so the drag covers the same
        // proportion of the surface at any radius.
        let to = [
            (t * std::f32::consts::TAU).sin() * 0.7 * radius,
            height,
            0.0,
        ];
        let moved = gesture.resolve(start, to, false);
        let started = Instant::now();
        let written = document.with(|d| d.set_target_transform(target, moved));
        refills.push(ms(started.elapsed()));
        if written.is_err() {
            break;
        }
        let _ = document.with(|d| geometry.sync(&harness.gpu, d));
    }
    document.with(|d| d.end_target_drag());

    let after = document.with(|d| d.document().resume_stats().ok())?;
    Some(Run {
        refill: median(refills),
        refilled: after.refilled_bricks.saturating_sub(before.refilled_bricks),
        surface_bricks,
    })
}

#[test]
fn what_the_per_brick_slope_is_actually_made_of() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let big = 10.0f32.sqrt();

    println!(
        "\n{:<44} {:>10} {:>12} {:>11} {:>10}",
        "radius / dab / cutter / op", "refill ms", "bricks/frame", "µs/brick", "surface"
    );
    let cases = [
        // The original pair, for continuity with what is already reported.
        (1.0f32, 0.18f32, false, "r=1    dab=0.18   cutter fixed"),
        (big, 0.18 * big, false, "r=√10  dab=0.569  cutter fixed"),
        // Dab held at the reference size: tests item overlap.
        (big, 0.18, false, "r=√10  dab=0.18   cutter fixed"),
        // Cutter kept on the surface: removes the buried-cutter confound.
        (1.0, 0.18, true, "r=1    dab=0.18   cutter surface"),
        (big, 0.18 * big, true, "r=√10  dab=0.569  cutter surface"),
        (big, 0.18, true, "r=√10  dab=0.18   cutter surface"),
    ];
    for (radius, dab, at_surface, label) in cases {
        for op in [Combine::Subtract, Combine::Intersect] {
            let Some(run) = drag(&harness, radius, dab, at_surface, op) else {
                println!("{:<44} skipped", format!("{label} {op:?}"));
                continue;
            };
            let per_frame = run.refilled as f64 / FRAMES as f64;
            println!(
                "{:<44} {:>9.2} {:>12.0} {:>10.2} {:>10}",
                format!("{label} {op:?}"),
                run.refill,
                per_frame,
                run.refill * 1000.0 / per_frame.max(1.0),
                run.surface_bricks
            );
        }
    }
}
