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

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::BackendPolicy;
use clayspace_model::{ConversionSettings, Direction, ModelError, Refusal, SceneModel};
use clayspace_view::Gpu;

use crate::figures::{ms, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
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
    fn every_direction_is_measured_once() {
        let names: BTreeSet<String> = Direction::ALL.into_iter().map(name).collect();
        assert_eq!(names.len(), Direction::ALL.len());
    }
}
