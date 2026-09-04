//! What the moment between two strokes costs.
//!
//! Every mesh sculptor write path asks for its layer's ray-query tree to be
//! rebuilt, and the ask is serviced at the one moment where a stall belongs to
//! nobody: the pointer coming up. `clayspace-engine`'s `Maintenance` spends
//! eight milliseconds there — half the sixteen the specification allows an
//! engine operation to hold the interface thread, because that moment already
//! pays for a surface refresh and a mip chain.
//!
//! That is a budget, and a budget nobody measures is a number in a comment. So
//! two figures, and the pair is the point:
//!
//!   * `maintenance.drain` is the drain a mesh gesture's end actually performs,
//!     with a rebuild queued against the layer just stroked. It includes the
//!     *deciding* — the drain reads the tree's quality once and rebuilds only
//!     where it has drifted past half again as many triangle tests as it
//!     scored when it was built — which is a walk of the tree and is paid
//!     whether or not a rebuild follows.
//!   * `maintenance.idle` is the same call with nothing queued, which is what
//!     every gesture on a field or a grid pays for the mechanism existing.
//!     It should be indistinguishable from zero, and the figure is here so
//!     that it staying that way is a fact rather than an expectation.
//!
//! # What it measured, the first time it was run
//!
//! **Both figures came out at zero, and the queue was working.** Twelve Grab
//! segments across the mesh reference queued one request each and the drain
//! serviced one each time, in under five microseconds — and no rebuild was
//! ever performed, because a twelve-segment drag does not take a 296,216
//! triangle tree half again past what it scored when it was built. Two things
//! follow, and both are worth having in a baseline rather than in someone's
//! head.
//!
//! The deciding is free. `clay_mesh_sculptor_quality` answers without walking
//! anything a caller can feel, so reading it at every gesture's end — which is
//! what this host chose over reading it per segment — costs nothing at all,
//! and the paragraph in `clayspace-engine`'s `Maintenance` that justifies the
//! choice is understating it.
//!
//! And the decay bar is clear of ordinary sculpting, which is what it was set
//! for: the engine's own measurement is that a rebuild produced a better tree
//! in one deformation of five and a dramatically worse one in two, so a
//! mechanism that fires on a normal drag would be the bug. A figure that
//! climbed off zero would mean either that rebuilds had started happening on
//! ordinary strokes or that asking had stopped being free, and both are things
//! to know.
//!
//! # Why there is no figure for a rebuild on its own
//!
//! Because it cannot be asked for. Whether the drain rebuilds is the host's
//! decision, taken against a quality figure that is only meaningful against
//! the same tree's own history, and there is no way from outside to put a tree
//! into a state where the answer is certainly yes. A figure that appeared on
//! the runs where a stroke happened to decay a tree far enough and vanished on
//! the runs where it did not would fail `compare::missing` as a measurement
//! that stopped running, which is exactly the signal that machinery exists to
//! give and exactly the wrong thing to spend it on. What a rebuild cost, when
//! one happened, is already kept: the host times the first one and carries the
//! figure, because the engine has no machine model and says so.

use std::time::{Duration, Instant};

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{Representation, SceneModel, SculptModel, ToolKind};

use crate::figures::{ms, Record};
use crate::run::Run;
use crate::skip::Skip;

/// The budget the drain is given, which is `clayspace-engine`'s own
/// `Maintenance::BUDGET`.
///
/// Restated here rather than reached for: the constant is private to the crate
/// that spends it, and a benchmark that could change it would be measuring a
/// budget of its own choosing. If the two ever disagree, this figure stops
/// describing what a gesture's end does — which is why it is written down
/// beside the reason rather than passed as a round number.
const BUDGET: Duration = Duration::from_millis(8);

/// The drag the tree is decayed by.
///
/// Grab rather than Draw: a drag moves a region bodily and is what actually
/// costs a bounding-volume hierarchy its shape, which is the decay the rebuild
/// exists to undo. A stamp that swells the surface in place leaves the
/// partition roughly where it was.
const TOOL: ToolKind = ToolKind::Mover;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("maintenance") {
        return;
    }
    match rounds(policy) {
        Ok((drained, idle)) => {
            run.timings("maintenance.drain", Record::Repeatable, drained);
            run.timings("maintenance.idle", Record::Repeatable, idle);
        }
        Err(why) => run.skip("maintenance", why),
    }
}

/// A stroke, the drain that follows it, and the drain that follows *that*.
///
/// The two figures are taken in one loop on one document on purpose. The
/// second call is the first call's own aftermath — the queue it just emptied —
/// so the pair is a difference measured moments apart rather than two numbers
/// from two arrangements.
fn rounds(policy: &BackendPolicy) -> Result<(Vec<f64>, Vec<f64>), Skip> {
    let scene = Scene::MeshReference;
    let mut document = scene
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mesh = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .ok_or(Skip::NoReferenceScene)?;
    // Activation is what arms the sculptor, and a drain with no sculptor to
    // ask has nothing to decide about.
    document
        .set_active_layer(mesh)
        .map_err(|_| Skip::EditRefused)?;
    settle(&mut document);

    let count = Record::Repeatable.samples();
    let path = scene.stroke(count + 1);
    let mut drained = Vec::new();
    let mut idle = Vec::new();
    for pair in path.windows(2) {
        // Outside the clock: this is what queues the request, and
        // `brush.mesh.mover` is what prices it.
        document
            .apply_stroke(TOOL, scene.brush(), pair, [false; 3])
            .map_err(|_| Skip::EditRefused)?;

        let started = Instant::now();
        document.drain_maintenance(BUDGET);
        drained.push(ms(started.elapsed()));

        let started = Instant::now();
        document.drain_maintenance(BUDGET);
        idle.push(ms(started.elapsed()));
    }
    Ok((drained, idle))
}

/// Empties whatever the scene's own construction left queued, so the first
/// sample is a drain of one request rather than of the whole build.
fn settle(document: &mut ClayDocument) {
    // Generously, and not against the interaction budget: this is the harness
    // clearing the bench, not a moment anyone is waiting through.
    document.drain_maintenance(Duration::from_secs(10));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The budget restated here has to be the one the engine spends, and the
    /// only thing this side can check is that it is half the interface-thread
    /// bound the specification states — which is the reasoning the engine's
    /// own constant is written against.
    #[test]
    fn the_budget_is_half_the_interface_thread_bound() {
        assert_eq!(BUDGET, Duration::from_millis(8));
        assert_eq!(BUDGET * 2, Duration::from_millis(16));
    }

    /// The decay a rebuild answers is a drag's, so the figure is taken behind
    /// one.
    #[test]
    fn the_tree_is_decayed_by_a_drag() {
        assert!(TOOL.is_path_driven(), "a drag, not a stamp");
    }
}
