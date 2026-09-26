//! Crossing a layer from one representation to another.
//!
//! Six directions, each measured from the suite member that can start it. A
//! crossing is unbounded work behind a busy cursor rather than something a
//! sculptor waits mid-gesture for, so these are the figures that say whether
//! it is a pause or an interruption.
//!
//! One-shots by construction: a crossing adds a new layer and leaves the
//! source where it was, so the second crossing of a document is a crossing of
//! a document with an extra layer in it.

use std::time::{Duration, Instant};

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument, RefillBudget};
use clayspace_model::{ConversionSettings, Direction, ModelError, Refusal, SceneModel};
use clayspace_view::Gpu;

use crate::figures::{mean, ms, Figure, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    measure_grid_to_field(policy, run);
    let Some(gpu) = headless_gpu() else {
        return run.skip("convert", Skip::NoHeadlessGpu);
    };
    for direction in Direction::ALL {
        let prefix = name(direction);
        if !run.wants_group(&prefix) {
            continue;
        }
        let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
            .map(|_| time(&gpu, policy, direction))
            .collect();
        match samples {
            Ok(samples) => run.timings(&prefix, Record::OneShot, samples),
            Err(why) => run.skip(prefix, why),
        }
    }
}

/// `convert.sdf_to_voxel`.
fn name(direction: Direction) -> String {
    format!("convert.{:?}_to_{:?}", direction.from(), direction.to()).to_lowercase()
}

fn time(gpu: &Gpu, policy: &BackendPolicy, direction: Direction) -> Result<f64, Skip> {
    let scene = Scene::for_representation(direction.from()).ok_or(Skip::NoReferenceScene)?;
    let mut document = scene
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    // The panel's own defaults, so the figure describes the crossing a
    // sculptor gets rather than one tuned for the benchmark.
    let settings = ConversionSettings::default();
    let started = Instant::now();
    let made = document
        .convert_layer(direction, settings.cell_size, settings.blur)
        .map_err(refusal)?;
    document
        .set_active_layer(made)
        .map_err(|_| Skip::EditRefused)?;
    screen.refresh(gpu, &mut document)?;
    Ok(ms(started.elapsed()))
}

/// The grid sizes issue #185 asked a grid-to-field crossing to be measured at,
/// and the cell that crosses the starting sphere into about that many cells.
const GRID_SIZES: [(&str, f32); 3] = [("5k", 0.095), ("23k", 0.057), ("100k", 0.035)];

/// What a grid-to-field crossing may hold the interface thread for: reading
/// the grid out, and placing the field with the application's bounded first
/// refill. Three frames, so a crossing is a stutter at worst.
const INTERFACE_BUDGET_MS: f64 = 50.0;

/// What the conversion itself may take on its worker, up to 100k cells. Not
/// the interface's time; this is how long the new layer takes to arrive.
const WORKER_BUDGET_MS: f64 = 1000.0;

/// The refill budget the application sets, so the placement is timed as the
/// application pays for it. See `REFILL_BUDGET` in `main.rs`.
const APP_REFILL_BUDGET: Duration = Duration::from_millis(8);

/// A grid-to-field crossing split where the application splits it: the
/// share the interface thread pays, and the conversion that runs on a worker.
///
/// No GPU: neither half draws, and the figures should exist on every machine.
/// The refill the placement leaves is not timed here — it is spread over the
/// frames that follow at the application's budget, and its size follows the
/// converted volume's box rather than anything this crossing controls.
fn measure_grid_to_field(policy: &BackendPolicy, run: &mut Run) {
    for (size, cell) in GRID_SIZES {
        let prefix = format!("convert.grid_to_field.{size}");
        if !run.wants_group(&prefix) {
            continue;
        }
        let samples: Result<Vec<(f64, f64)>, Skip> = (0..Record::OneShot.samples())
            .map(|_| time_grid_to_field(policy, cell))
            .collect();
        let samples = match samples {
            Ok(samples) => samples,
            Err(why) => {
                run.skip(prefix, why);
                continue;
            }
        };
        let (interface, worker): (Vec<f64>, Vec<f64>) = samples.into_iter().unzip();
        run.spread(&format!("{prefix}.interface"), &interface);
        run.spread(&format!("{prefix}.worker"), &worker);
        run.insert(
            format!("{prefix}.interface"),
            Figure::ms(mean(&interface), Some(INTERFACE_BUDGET_MS)),
        );
        run.insert(
            format!("{prefix}.worker"),
            Figure::ms(mean(&worker), Some(WORKER_BUDGET_MS)),
        );
    }
}

/// One crossing of a freshly rasterized grid: (interface ms, worker ms).
fn time_grid_to_field(policy: &BackendPolicy, cell: f32) -> Result<(f64, f64), Skip> {
    let mut document = ClayDocument::new(policy.clone())
        .and_then(ClayDocument::with_starting_form)
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    document
        .convert_layer(Direction::SdfToVoxel, cell, 1)
        .map_err(refusal)?;
    document.set_refill_budget(RefillBudget::Within(APP_REFILL_BUDGET));
    let settings = ConversionSettings::default();

    let started = Instant::now();
    let crossing = document
        .begin_grid_to_field(settings.cell_size, settings.blur, false)
        .map_err(refusal)?;
    let read = started.elapsed();

    let made = std::thread::spawn(move || {
        let started = Instant::now();
        crossing.convert().map(|made| (made, started.elapsed()))
    })
    .join()
    .map_err(|_| Skip::EditRefused)?
    .map_err(refusal)?;

    let started = Instant::now();
    document.finish_grid_to_field(made.0).map_err(refusal)?;
    let placed = started.elapsed();
    Ok((ms(read + placed), ms(made.1)))
}

/// What a refused crossing is, as a stated reason.
fn refusal(error: ModelError) -> Skip {
    match error {
        ModelError::Conversion(Refusal::UnboundedRegion) => Skip::NoRegionToConvertInto,
        _ => Skip::EditRefused,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn a_figure_name_says_which_way_it_goes() {
        assert_eq!(name(Direction::SdfToVoxel), "convert.sdf_to_voxel");
        assert_eq!(name(Direction::VoxelToMesh), "convert.voxel_to_mesh");
    }

    #[test]
    fn the_grid_sizes_are_named_apart() {
        let names: BTreeSet<&str> = GRID_SIZES.iter().map(|(size, _)| *size).collect();
        assert_eq!(names.len(), GRID_SIZES.len());
    }

    #[test]
    fn every_direction_is_measured_once() {
        let names: BTreeSet<String> = Direction::ALL.into_iter().map(name).collect();
        assert_eq!(names.len(), Direction::ALL.len());
    }
}
