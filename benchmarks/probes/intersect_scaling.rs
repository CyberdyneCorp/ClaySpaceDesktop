//! Scratch probe for ClayCore #451's residual: does an intersect drag scale
//! with the layer while a subtract drag does not?
//!
//! `bounds.cpp:1270` returns `Nonlocality::BoundedByLayer` for an intersect
//! item, so its influence bound is the whole layer's extent. This measures that
//! from outside the engine: the same drag, the same frame path, on the
//! reference scene and on the one with ten times the surface area.
//!
//! Not for keeping.
//!
//! ```sh
//! cargo test -p clayspace-app --release --test intersect_scaling -- --nocapture
//! ```

mod support;

use std::time::{Duration, Instant};

use clayspace_app::{Scene, SharedDocument, SurfaceGeometry, SyncCost};
use clayspace_engine::BackendPolicy;
use clayspace_model::{
    Combine, CombineSettings, GizmoDrag, GizmoHandle, GizmoMode, GizmoTarget, ObjectModel, Shape,
};
use support::Harness;

/// The cutter, exactly as `bench/groups/objects.rs` places it.
const CUT: [f32; 2] = [0.25, 1.6];
const FRAMES: usize = 12;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
    values[values.len() / 2]
}

/// What one drag frame cost, split at the boundary the bound governs.
/// What the resumable refill did over one drag, as a difference.
#[derive(Debug, Clone, Copy)]
struct Resume {
    resumed: u64,
    refilled: u64,
    entries: u64,
    bytes: u64,
    budget: Option<u64>,
}

struct Frame {
    /// `set_object_transform` — the write and the refill it dirties. This is
    /// where `Nonlocality::BoundedByLayer` decides how much is invalidated.
    refill: Duration,
    /// The re-mesh of whatever that dirtied.
    cost: SyncCost,
}

/// One drag, returning what each frame cost on both sides, and what the
/// resumable refill reused across the whole gesture.
fn drag(harness: &Harness, scene: Scene, op: Combine) -> Option<(Vec<Frame>, Resume, u64)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let document = SharedDocument::new(scene.build(policy).ok()?);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    document
        .with(|d| geometry.rebuild(&harness.gpu, d))
        .ok()?;

    document
        .with(|d| {
            d.place_object(
                Shape::Cylinder,
                &CUT,
                [0.0, 0.9, 0.0],
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

    // Cumulative and never reset, so one gesture is the difference across it.
    let before = document.with(|d| d.document().resume_stats().ok())?;
    let surface_bricks = document
        .with(|d| d.cache().stats().ok())
        .map(|s| s.surface_bricks)
        .unwrap_or(0);

    document.with(|d| d.begin_target_drag(target));
    let mut frames = Vec::new();
    for step in 0..FRAMES {
        let t = step as f32 / FRAMES as f32;
        let to = [(t * std::f32::consts::TAU).sin() * 0.7, 0.9, 0.0];
        let moved = gesture.resolve(start, to, false);

        let started = Instant::now();
        let written = document.with(|d| d.set_target_transform(target, moved));
        let refill = started.elapsed();
        if written.is_err() {
            break;
        }
        if let Ok(Some(cost)) = document.with(|d| geometry.sync(&harness.gpu, d)) {
            frames.push(Frame { refill, cost });
        }
    }
    document.with(|d| d.end_target_drag());

    let after = document.with(|d| d.document().resume_stats().ok())?;
    let resume = Resume {
        resumed: after.resumed_bricks.saturating_sub(before.resumed_bricks),
        refilled: after.refilled_bricks.saturating_sub(before.refilled_bricks),
        entries: after.entries,
        bytes: after.bytes,
        budget: after.budget,
    };
    (!frames.is_empty()).then_some((frames, resume, surface_bricks))
}

#[test]
fn an_intersect_drag_scales_with_the_layer_and_a_subtract_does_not() {
    let Some(harness) = Harness::new() else {
        return;
    };

    println!(
        "\n{:<26} {:>12} {:>12} {:>9} {:>8} {:>10}",
        "scene / operation", "refill", "engine mesh", "ours", "keys", "triangles"
    );
    let mut summary = Vec::new();
    for scene in [Scene::Reference, Scene::TenTimesLarger] {
        for op in [Combine::Subtract, Combine::Intersect] {
            let Some((frames, resume, surface_bricks)) = drag(&harness, scene, op) else {
                println!("{:<26} skipped", format!("{} {op:?}", scene.member()));
                continue;
            };
            let mut refills: Vec<f64> = frames.iter().map(|f| ms(f.refill)).collect();
            refills.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
            let refill = median(refills.clone());
            let (lo, hi) = (refills[0], refills[refills.len() - 1]);
            let keys = median(frames.iter().map(|f| f.cost.keys as f64).collect());
            let engine = median(frames.iter().map(|f| ms(f.cost.engine_mesh_time)).collect());
            let ours = median(
                frames
                    .iter()
                    .map(|f| ms(f.cost.read_time + f.cost.split_time + f.cost.upload_time))
                    .collect(),
            );
            let tris = median(frames.iter().map(|f| f.cost.triangles as f64).collect());
            println!(
                "{:<26} {refill:>9.2} ms {engine:>9.2} ms {ours:>6.2} ms {keys:>8.0} {tris:>10.0}   refill {}x {lo:.2}-{hi:.2}",
                format!("{} {op:?}", scene.member()),
                frames.len()
            );
            let ratio = resume.resumed as f64
                / (resume.resumed + resume.refilled).max(1) as f64
                * 100.0;
            println!(
                "    resumed {:>9} / refilled {:>9}  = {ratio:>5.1}% resumed   \
                 seeds {} entries, {:.1} MiB of {}   surface bricks {surface_bricks}",
                resume.resumed,
                resume.refilled,
                resume.entries,
                resume.bytes as f64 / (1024.0 * 1024.0),
                resume
                    .budget
                    .map(|b| format!("{:.1} MiB", b as f64 / (1024.0 * 1024.0)))
                    .unwrap_or_else(|| "unlimited".into()),
            );
            summary.push((scene.member(), op, refill, engine));
        }
    }

    // The claim, as a ratio rather than an absolute: an intersect's dirty
    // region should follow the document and a subtract's should follow the cut.
    let find = |member: &str, op: Combine| {
        summary
            .iter()
            .find(|(m, o, ..)| *m == member && *o == op)
            .map(|(_, _, keys, engine)| (*keys, *engine))
    };
    if let (Some(sr), Some(s10), Some(ir), Some(i10)) = (
        find("reference", Combine::Subtract),
        find("reference-10x", Combine::Subtract),
        find("reference", Combine::Intersect),
        find("reference-10x", Combine::Intersect),
    ) {
        println!("\n  REFILL (where the bound applies), 10x ÷ reference:");
        println!("    subtract  {:.2}x   ({:.2} -> {:.2} ms)", s10.0 / sr.0.max(1e-6), sr.0, s10.0);
        println!("    intersect {:.2}x   ({:.2} -> {:.2} ms)", i10.0 / ir.0.max(1e-6), ir.0, i10.0);
        println!("  engine mesh, 10x ÷ reference:");
        println!("    subtract  {:.2}x", s10.1 / sr.1.max(1e-6));
        println!("    intersect {:.2}x", i10.1 / ir.1.max(1e-6));
        println!("\n  intersect ÷ subtract, within one scene, on the refill:");
        println!("    reference     {:.2}x", ir.0 / sr.0.max(1e-6));
        println!("    reference-10x {:.2}x", i10.0 / s10.0.max(1e-6));
    }
}
