//! What an operation costs — the second verb, beside the brushes.
//!
//! A brush is a gesture; these are the things a gesture cannot express. Taper
//! and twist come through the deform panel, the lattice drag through the cage,
//! close-holes, fill-voids and refine-region through the pre-bake repair menu.
//! They are one family here because they are one call in the model —
//! `apply_operation` — and because measuring the panel's verbs separately from
//! the menu's would report the same engine work twice under two names.
//!
//! Every one of them is a one-shot: a region refined twice is not refined
//! again, a hole closed stays closed, and a taper applied to an already
//! tapered form measures the second taper. So the document is rebuilt between
//! samples and the tolerance is the wider one.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::BackendPolicy;
use clayspace_model::{LayerOperation, Representation, SculptModel};
use clayspace_view::Gpu;

use crate::figures::{ms, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("op", Skip::NoHeadlessGpu);
    };
    for operation in LayerOperation::all() {
        for representation in Representation::ALL {
            if !operation.applies_to(representation) {
                continue;
            }
            let prefix = name(representation, operation);
            if !run.wants_group(&prefix) {
                continue;
            }
            let scene = scene_for(representation, operation);
            let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
                .map(|_| time(&gpu, policy, scene, operation))
                .collect();
            match samples {
                Ok(samples) => run.timings(&prefix, Record::OneShot, samples),
                Err(why) => run.skip(prefix, why),
            }
        }
    }
}

/// `op.mesh.taper`.
///
/// From the history's own label, which is the name a sculptor sees an
/// operation under when they undo it.
fn name(representation: Representation, operation: LayerOperation) -> String {
    format!(
        "op.{representation:?}.{}",
        operation.label().replace(' ', "_")
    )
    .to_lowercase()
}

/// Which member to measure this operation on.
///
/// The representation's own, except for the two pre-bake repairs: a solid grid
/// has no holes to close and no voids to fill, so on the plain voxel member
/// they would time the check and report it as the repair.
fn scene_for(representation: Representation, operation: LayerOperation) -> Scene {
    match operation {
        LayerOperation::CloseHoles { .. } | LayerOperation::FillVoids => Scene::VoxelPocked,
        _ => Scene::for_representation(representation),
    }
}

/// One application, from the operation to the surface arriving.
fn time(
    gpu: &Gpu,
    policy: &BackendPolicy,
    scene: Scene,
    operation: LayerOperation,
) -> Result<f64, Skip> {
    let mut document = scene
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let started = Instant::now();
    document
        .apply_operation(operation)
        .map_err(|_| Skip::EditRefused)?;
    screen.refresh(gpu, &mut document)?;
    Ok(ms(started.elapsed()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repair_is_measured_on_the_member_that_needs_repairing() {
        assert_eq!(
            scene_for(Representation::Voxel, LayerOperation::FillVoids),
            Scene::VoxelPocked
        );
        assert_eq!(
            scene_for(
                Representation::Voxel,
                LayerOperation::RefineRegion {
                    min: [0.0; 3],
                    max: [1.0; 3]
                }
            ),
            Scene::VoxelReference
        );
    }

    #[test]
    fn a_figure_name_is_lowercase_and_dotted() {
        assert_eq!(
            name(Representation::Voxel, LayerOperation::FillVoids),
            "op.voxel.fill_voids"
        );
    }

    /// Every operation the model offers is measured somewhere, and no two
    /// share a name.
    #[test]
    fn every_operation_is_named_once() {
        let mut names = std::collections::BTreeSet::new();
        let mut measured = 0;
        for operation in LayerOperation::all() {
            for representation in Representation::ALL {
                if operation.applies_to(representation) {
                    measured += 1;
                    names.insert(name(representation, operation));
                }
            }
        }
        assert_eq!(names.len(), measured, "two operations share a figure name");
        assert_eq!(
            names.len(),
            LayerOperation::all().len(),
            "an operation applies to more than one representation, or to none"
        );
    }
}
