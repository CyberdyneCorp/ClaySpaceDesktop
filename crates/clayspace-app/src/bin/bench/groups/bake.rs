//! The unbounded work: collapsing a tape, writing a file, checking a grid.
//!
//! None of these is an edit. They are the operations the specification puts
//! behind a busy cursor and requires never to run unasked — and the reason
//! they are measured is that the figure is what should decide when to offer
//! one. `tape.growth` says a document gets slower the more it is used;
//! `bake.consolidate` is what that costs to undo.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{ExchangeModel, ExportSettings, SceneModel};
use clayspace_view::Gpu;

use crate::figures::{ms, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("bake", Skip::NoHeadlessGpu);
    };
    one(
        &gpu,
        policy,
        run,
        "bake.consolidate",
        Scene::Reference,
        consolidate,
    );
    one(&gpu, policy, run, "bake.export", Scene::Reference, export);
    one(
        &gpu,
        policy,
        run,
        "bake.repair_report",
        Scene::VoxelPocked,
        repair_report,
    );
}

fn one(
    gpu: &Gpu,
    policy: &BackendPolicy,
    run: &mut Run,
    prefix: &str,
    scene: Scene,
    what: fn(&mut ClayDocument) -> Result<(), Skip>,
) {
    if !run.wants_group(prefix) {
        return;
    }
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| time(gpu, policy, scene, what))
        .collect();
    match samples {
        Ok(samples) => run.timings(prefix, Record::OneShot, samples),
        Err(why) => run.skip(prefix, why),
    }
}

fn time(
    gpu: &Gpu,
    policy: &BackendPolicy,
    scene: Scene,
    what: fn(&mut ClayDocument) -> Result<(), Skip>,
) -> Result<f64, Skip> {
    let mut document = scene
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let started = Instant::now();
    what(&mut document)?;
    let took = started.elapsed();
    // After the clock, because the surface a consolidation leaves is the same
    // surface: this is here so a failure to re-mesh is caught, not timed.
    screen.refresh(gpu, &mut document)?;
    Ok(ms(took))
}

/// Collapsing the active layer's tape into a baked volume.
fn consolidate(document: &mut ClayDocument) -> Result<(), Skip> {
    let key = document.scene().active.ok_or(Skip::SceneWouldNotBuild)?;
    document
        .consolidate_layer(key)
        .map_err(|_| Skip::EditRefused)
}

/// Writing the reference scene out, and taking the file away again.
fn export(document: &mut ClayDocument) -> Result<(), Skip> {
    let path = std::env::temp_dir().join("clayspace-bench-export.obj");
    let _ = std::fs::remove_file(&path);
    let written = document
        .export_mesh(&path, ExportSettings::default())
        .map_err(|_| Skip::EditRefused);
    let _ = std::fs::remove_file(&path);
    written.map(|_| ())
}

/// Asking a grid what is wrong with it before a bake.
fn repair_report(document: &mut ClayDocument) -> Result<(), Skip> {
    document
        .repair_report()
        .map(|_| ())
        .ok_or(Skip::NotOnThisRepresentation)
}
