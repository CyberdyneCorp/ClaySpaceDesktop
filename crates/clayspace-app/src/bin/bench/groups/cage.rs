//! What a frame of a cage drag costs on a mesh, against the cage's size
//! (#176).
//!
//! `op.mesh.lattice_drag` prices a cage laid down once as an operation. This
//! is the other half, the one a sculptor feels: a drag on one control point of
//! a raised cage, frame by frame, from the pointer move to the bent surface
//! arriving on the screen. It is measured at 3³, 8³ and 32³ — the smallest
//! useful cage, the default, and the ceiling — with one point in hand each
//! time, so the three figures differ only by the size of the cage.
//!
//! The frame budget is the specification's interface-thread bound, attached to
//! every size: a drag is on the interface thread and has no busy cursor in
//! front of it. `cage.scaling` states the other half of the claim — that a
//! frame follows the points in hand rather than the points the cage holds — as
//! a ratio, which survives a change of machine where an absolute timing does
//! not.
//!
//! Both are reported and not enforced until the engine pin carries
//! ClayCore#655, which sums a mesh cage over its dragged points alone. Before
//! it, one corner of a 32³ cage is 32,768 terms a vertex and the 32³ figure is
//! seconds; see `a_mesh_cage_evaluation_is_priced_by_every_point_the_cage_holds`
//! in `tests/claycore_repros.rs`.
//!
//! `cage.memory` is what the device holds after a long drag, released, against
//! what it held before it: the per-frame upload goes into the buffers the
//! surface already has, so a drag should leave nothing behind.
//!
//! One-shot sampling for every size, even though a frame is repeatable. Three
//! frames of a 32³ drag on the old engine is already the longest measurement
//! in the harness, and the same record for all three sizes keeps their figures
//! comparable by name.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::LatticeModel;
use clayspace_view::Gpu;

use crate::figures::{ms, Figure, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

/// What the specification allows an engine operation to hold the interface
/// thread for — stated there, not chosen here.
const FRAME_BUDGET_MS: f64 = 16.0;

/// Control points per axis: the smallest useful cage, the default, the
/// ceiling.
const SIZES: [i32; 3] = [3, 8, 32];

/// How many times the ceiling's frame may cost the smallest cage's.
///
/// A small multiple rather than one: the larger cage still has more handles to
/// draw and a larger rest grid to read, which is work the frame genuinely
/// does. ClayCore#655 measured ~11 ms at 32³ against ~9–10 ms at 3³.
const SCALING_BUDGET: f64 = 3.0;

/// How many frames the memory figure drags for — the acceptance criterion's
/// own count.
const LONG_DRAG: usize = 100;

/// How far a long drag may leave the device's holding above where it started.
const MEMORY_BUDGET: f64 = 1.2;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("cage", Skip::NoHeadlessGpu);
    };
    let mut frames = Vec::new();
    for divisions in SIZES {
        let prefix = format!("cage.drag_{divisions}");
        match drag_frames(&gpu, policy, divisions, Record::OneShot.samples()) {
            Ok(samples) => {
                record_frames(run, &prefix, &samples);
                frames.push(samples);
            }
            Err(why) => run.skip(prefix, why),
        }
    }
    if let [smallest, .., largest] = frames.as_slice() {
        let ratio = median(largest) / median(smallest).max(f64::MIN_POSITIVE);
        run.insert(
            "cage.scaling",
            Figure::ratio(ratio, Some(SCALING_BUDGET), 2.0),
        );
    }
    match long_drag_memory(&gpu, policy) {
        Ok(ratio) => run.insert(
            "cage.memory",
            Figure::ratio(ratio, Some(MEMORY_BUDGET), 1.1),
        ),
        Err(why) => run.skip("cage.memory", why),
    }
}

/// A one-shot's figures, with the frame budget attached.
fn record_frames(run: &mut Run, prefix: &str, samples: &[f64]) {
    for (name, figure) in Record::OneShot.figures(prefix, samples) {
        run.spread(&name, samples);
        run.insert(
            name,
            Figure {
                budget: Some(FRAME_BUDGET_MS),
                ..figure
            },
        );
    }
}

fn median(samples: &[f64]) -> f64 {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
    crate::figures::quantile(&sorted, 0.5)
}

/// The mesh reference with a cage raised and its first corner in hand, drawn
/// once so nothing below times the first upload.
fn caged(
    gpu: &Gpu,
    policy: &BackendPolicy,
    divisions: i32,
) -> Result<(ClayDocument, Screen, [f32; 3]), Skip> {
    let mut document = Scene::MeshReference
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;
    document
        .begin_lattice([divisions; 3])
        .map_err(|_| Skip::EditRefused)?;
    document.select_lattice_point(Some(0));
    let start = document.lattice().points[0];
    Ok((document, screen, start))
}

/// One pointer move of the drag: the corner a little further out.
fn step(start: [f32; 3], frame: usize) -> [f32; 3] {
    let by = 0.01 * frame as f32;
    [start[0] + by, start[1] - by, start[2]]
}

/// `frames` frames of a single-point drag, each from the pointer move to the
/// surface arriving.
fn drag_frames(
    gpu: &Gpu,
    policy: &BackendPolicy,
    divisions: i32,
    frames: usize,
) -> Result<Vec<f64>, Skip> {
    let (mut document, mut screen, start) = caged(gpu, policy, divisions)?;
    (1..=frames)
        .map(|frame| {
            let started = Instant::now();
            document
                .drag_lattice_point(step(start, frame))
                .map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, &mut document)?;
            Ok(ms(started.elapsed()))
        })
        .collect()
}

/// What the device holds after a long drag on the smallest cage, released,
/// over what it held before the first frame.
///
/// The device is waited on at both ends so staging — writes the device may
/// not have finished with — is not counted as held on either side.
fn long_drag_memory(gpu: &Gpu, policy: &BackendPolicy) -> Result<f64, Skip> {
    let (mut document, mut screen, start) = caged(gpu, policy, SIZES[0])?;
    let before = settled_memory(gpu);
    for frame in 1..=LONG_DRAG {
        document
            .drag_lattice_point(step(start, frame % 10 + 1))
            .map_err(|_| Skip::EditRefused)?;
        screen.refresh(gpu, &mut document)?;
    }
    document.cancel_lattice();
    screen.refresh(gpu, &mut document)?;
    let after = settled_memory(gpu);
    Ok(after as f64 / before.max(1) as f64)
}

fn settled_memory(gpu: &Gpu) -> u64 {
    gpu.device.poll(wgpu::Maintain::Wait);
    gpu.note_device_idle();
    gpu.memory().total()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every size's frame is judged against the interface-thread budget, not
    /// only the ceiling's: the smallest cage is what a sculptor starts with.
    #[test]
    fn every_frame_figure_carries_the_frame_budget() {
        let mut run = Run::new(None);
        record_frames(&mut run, "cage.drag_3", &[30.0, 10.0, 20.0]);
        let figure = &run.figures()["cage.drag_3.ms"];
        assert_eq!(figure.value, 20.0);
        assert_eq!(figure.budget, Some(FRAME_BUDGET_MS));
        assert!(run.spreads().contains_key("cage.drag_3.ms"));
    }

    /// A frame that put the corner back at rest would make the cage the
    /// identity, and the long drag would time frames that bend nothing.
    #[test]
    fn the_long_drag_never_steps_back_to_rest() {
        let start = [1.0, 1.0, 1.0];
        assert!((1..=LONG_DRAG).all(|frame| step(start, frame % 10 + 1) != start));
    }
}
