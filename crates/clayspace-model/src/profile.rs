//! What a stroke cost, split into the phases it is made of.
//!
//! [`FrameLog`](crate::instrument::FrameLog) beside this module answers *which
//! operation stalled*. This one answers the question after it: **whose
//! milliseconds were they**. A re-mesh reported as one total spanning an engine
//! call and this application's work around it cannot be acted on by either
//! party, because neither can tell from it whether the cost was theirs — and a
//! performance report that cannot be acted on is a conversation, which is what
//! [`crate::diagnostics`] exists to prevent.
//!
//! No clock here, as in `instrument`: durations are passed in. That is what
//! lets the retention rule, the quantiles and the aggregate be tested without a
//! GPU, an engine, or a machine that happens to have the right hardware.

use std::collections::BTreeMap;
use std::time::Duration;

/// How many durations one phase keeps for its quantiles.
///
/// An hour of sculpting is tens of thousands of dabs, and an unbounded `Vec`
/// in the interactive path is a leak with extra steps. The count and the worst
/// time are kept for the whole session; the quantiles describe this window,
/// and [`Samples::seen`] beside [`Samples::retained`] is what says so rather
/// than leaving a reader to assume the two are the same number.
pub const RETAINED: usize = 4096;

/// One term of a stroke's cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    /// The engine applies the stroke and refills the bricks it dirtied.
    /// Nothing of this application's runs inside it.
    EngineEdit,
    /// The engine turns those bricks into triangles.
    EngineMesh,
    /// Copying the engine's mesh into the renderer's vertex layout.
    Read,
    /// Splitting the triangles per key, so one dab can replace one of them.
    Split,
    /// Writing the changed spans to the device.
    Upload,
}

impl Phase {
    pub const ALL: [Phase; 5] = [
        Self::EngineEdit,
        Self::EngineMesh,
        Self::Read,
        Self::Split,
        Self::Upload,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::EngineEdit => "engine edit",
            Self::EngineMesh => "engine mesh",
            Self::Read => "read",
            Self::Split => "split",
            Self::Upload => "upload",
        }
    }

    /// Whether the time was spent inside the engine.
    ///
    /// The whole point of the split: a figure a reader cannot attribute to one
    /// side of the boundary is a figure neither side will act on.
    pub fn is_engine(self) -> bool {
        matches!(self, Self::EngineEdit | Self::EngineMesh)
    }

    /// Where inside the engine, in words rather than as a C symbol.
    ///
    /// A stroke reaches a different entry point on each representation, so
    /// naming one would be wrong on three of the four. Meshing has exactly one
    /// call underneath and is named.
    pub fn entry_point(self) -> Option<&'static str> {
        match self {
            Self::EngineEdit => Some("stroke and brick refill"),
            Self::EngineMesh => Some("clay_brick_cache_mesh"),
            _ => None,
        }
    }
}

/// How much work one sample covered.
///
/// A duration without this is not comparable with any other duration: eleven
/// milliseconds over four keys and eleven over ninety are the same number and
/// opposite facts. Summed rather than averaged here — the sample count is
/// reported beside it, so a reader can divide and see that the division was
/// theirs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Work {
    /// Bricks the edit dirtied. Zero for the phases that mesh what it dirtied.
    pub bricks: usize,
    /// Keys re-meshed. Zero for the edit, which does not mesh.
    pub keys: usize,
    /// Triangles the re-mesh produced.
    pub triangles: usize,
}

impl Work {
    pub const NONE: Self = Self {
        bricks: 0,
        keys: 0,
        triangles: 0,
    };

    pub fn bricks(bricks: usize) -> Self {
        Self {
            bricks,
            ..Self::NONE
        }
    }

    pub fn meshed(keys: usize, triangles: usize) -> Self {
        Self {
            bricks: 0,
            keys,
            triangles,
        }
    }

    fn add(&mut self, other: Self) {
        self.bricks = self.bricks.saturating_add(other.bricks);
        self.keys = self.keys.saturating_add(other.keys);
        self.triangles = self.triangles.saturating_add(other.triangles);
    }
}

/// What one phase has cost, as a distribution rather than as a figure.
///
/// A mean is deliberately not offered. The tail is what a sculptor is
/// complaining about, and a mean is the statistic that hides it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Samples {
    seen: u64,
    worst: Duration,
    window: Vec<Duration>,
    /// Where the next sample overwrites, once the window is full.
    next: usize,
    work: Work,
}

impl Samples {
    pub fn record(&mut self, took: Duration, work: Work) {
        self.seen = self.seen.saturating_add(1);
        self.worst = self.worst.max(took);
        self.work.add(work);

        if self.window.len() < RETAINED {
            self.window.push(took);
            return;
        }
        self.window[self.next] = took;
        self.next = (self.next + 1) % RETAINED;
    }

    /// Every sample this session, including those the window has dropped.
    pub fn seen(&self) -> u64 {
        self.seen
    }

    /// How many of them the quantiles below were computed over.
    pub fn retained(&self) -> usize {
        self.window.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen == 0
    }

    /// The worst time this session, whether or not it is still in the window.
    ///
    /// `None` rather than zero where nothing ran: a phase that never happened
    /// and a phase that was free are different facts, and reporting the first
    /// as the second is how a report starts lying.
    pub fn worst(&self) -> Option<Duration> {
        (!self.is_empty()).then_some(self.worst)
    }

    pub fn median(&self) -> Option<Duration> {
        self.quantile(0.5)
    }

    pub fn p95(&self) -> Option<Duration> {
        self.quantile(0.95)
    }

    /// Both quantiles and the worst, from a single sort.
    ///
    /// Asking for them one at a time sorts the retained window once per
    /// question, and this is read while a report is being assembled — which
    /// the composition root does every frame. Measured at 4096 retained
    /// samples across five phases, halving the sorts halves the cost of the
    /// whole summary.
    pub fn summary(&self) -> Option<Summary> {
        if self.window.is_empty() {
            return None;
        }
        let mut sorted = self.window.clone();
        sorted.sort_unstable();
        let last = sorted.len() - 1;
        let at = |fraction: f64| sorted[((fraction * last as f64).round() as usize).min(last)];
        Some(Summary {
            median: at(0.5),
            p95: at(0.95),
            worst: self.worst,
        })
    }

    /// The quantile over the retained window.
    ///
    /// The same definition the benchmark harness uses — the nearest rank over
    /// `len - 1` — so a figure read here and a figure read there mean the same
    /// thing and can be quoted beside each other.
    pub fn quantile(&self, fraction: f64) -> Option<Duration> {
        if self.window.is_empty() {
            return None;
        }
        let mut sorted = self.window.clone();
        sorted.sort_unstable();
        let last = sorted.len() - 1;
        let at = (fraction * last as f64).round() as usize;
        Some(sorted[at.min(last)])
    }

    /// What every sample behind these figures covered, summed.
    pub fn work(&self) -> Work {
        self.work
    }

    /// Folds another phase's samples into this one, for a cross-tool view.
    ///
    /// The windows are concatenated rather than interleaved: they are separate
    /// populations and there is no shared order between them to preserve.
    fn merge(&mut self, other: &Self) {
        self.seen = self.seen.saturating_add(other.seen);
        self.worst = self.worst.max(other.worst);
        self.work.add(other.work);
        self.window.extend_from_slice(&other.window);
    }
}

/// A phase's figures, taken together.
///
/// The worst is the session's rather than the window's, as
/// [`Samples::worst`] is: a sample that has left the window still happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub median: Duration,
    pub p95: Duration,
    pub worst: Duration,
}

/// What one tool has cost, phase by phase.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolProfile {
    phases: BTreeMap<Phase, Samples>,
}

impl ToolProfile {
    pub fn record(&mut self, phase: Phase, took: Duration, work: Work) {
        self.phases.entry(phase).or_default().record(took, work);
    }

    /// One phase's samples. Always answers — a phase that never ran answers
    /// with an empty distribution rather than with nothing, so a caller
    /// rendering all five needs no second code path for the absent ones.
    pub fn phase(&self, phase: Phase) -> Samples {
        self.phases.get(&phase).cloned().unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.phases.values().all(Samples::is_empty)
    }

    fn merge(&mut self, other: &Self) {
        for (phase, samples) in &other.phases {
            self.phases.entry(*phase).or_default().merge(samples);
        }
    }
}

/// Every phase of every stroke this session, by the tool that ran it.
///
/// Keyed by tool because "the smooth brush is the slow one" is a sentence an
/// engine team can act on, and an aggregate over twenty-one tools cannot
/// produce it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StrokeProfile {
    tools: BTreeMap<String, ToolProfile>,
}

impl StrokeProfile {
    pub fn record(&mut self, tool: &str, phase: Phase, took: Duration, work: Work) {
        self.tools
            .entry(tool.to_string())
            .or_default()
            .record(phase, took, work);
    }

    /// Each tool that ran, in a stable order.
    pub fn tools(&self) -> impl Iterator<Item = (&str, &ToolProfile)> {
        self.tools
            .iter()
            .map(|(tool, profile)| (tool.as_str(), profile))
    }

    pub fn is_empty(&self) -> bool {
        self.tools.values().all(ToolProfile::is_empty)
    }

    /// The whole session, tools folded together.
    ///
    /// Computed on read rather than accumulated beside the per-tool figures,
    /// so the two cannot come to disagree.
    pub fn across_tools(&self) -> ToolProfile {
        let mut whole = ToolProfile::default();
        for profile in self.tools.values() {
            whole.merge(profile);
        }
        whole
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn a_phase_that_never_ran_reports_no_samples_not_a_zero() {
        // A phase that did not happen and a phase that was free are different
        // facts, and a report that conflates them is worse than one that omits
        // the phase.
        let samples = Samples::default();
        assert!(samples.is_empty());
        assert_eq!(samples.seen(), 0);
        assert_eq!(samples.worst(), None);
        assert_eq!(samples.median(), None);
        assert_eq!(samples.p95(), None);
    }

    #[test]
    fn the_window_is_bounded_and_the_worst_outlives_it() {
        let mut samples = Samples::default();
        // The worst time first, so it has left the window by the end.
        samples.record(ms(900), Work::bricks(1));
        for _ in 0..5000 {
            samples.record(ms(2), Work::bricks(1));
        }

        assert_eq!(samples.seen(), 5001, "every sample is counted");
        assert_eq!(samples.retained(), RETAINED, "the window is bounded");
        assert_eq!(
            samples.worst(),
            Some(ms(900)),
            "the worst time was dropped with its sample"
        );
        assert_eq!(samples.median(), Some(ms(2)));
    }

    #[test]
    fn the_quantiles_describe_the_window_and_say_so() {
        let mut samples = Samples::default();
        for value in 1..=100 {
            samples.record(ms(value), Work::NONE);
        }
        assert_eq!(samples.seen(), 100);
        assert_eq!(samples.retained(), 100);
        // The benchmark harness's definition of a quantile, so a figure from
        // here can be quoted beside one from there.
        assert_eq!(samples.median(), Some(ms(51)));
        assert_eq!(samples.p95(), Some(ms(95)));
        assert_eq!(samples.worst(), Some(ms(100)));
    }

    #[test]
    fn one_sort_answers_the_same_as_three_questions() {
        let mut samples = Samples::default();
        for value in 1..=100 {
            samples.record(ms(value), Work::NONE);
        }
        let summary = samples.summary().expect("samples");
        assert_eq!(Some(summary.median), samples.median());
        assert_eq!(Some(summary.p95), samples.p95());
        assert_eq!(Some(summary.worst), samples.worst());
    }

    #[test]
    fn a_phase_that_never_ran_has_no_summary() {
        assert_eq!(Samples::default().summary(), None);
    }

    #[test]
    fn one_sample_is_its_own_median_and_worst() {
        let mut samples = Samples::default();
        samples.record(ms(7), Work::NONE);
        assert_eq!(samples.median(), Some(ms(7)));
        assert_eq!(samples.p95(), Some(ms(7)));
        assert_eq!(samples.worst(), Some(ms(7)));
    }

    #[test]
    fn the_workload_behind_a_figure_is_kept() {
        let mut samples = Samples::default();
        samples.record(ms(4), Work::meshed(27, 9_000));
        samples.record(ms(6), Work::meshed(31, 11_000));
        assert_eq!(samples.work().keys, 58);
        assert_eq!(samples.work().triangles, 20_000);
        assert_eq!(samples.work().bricks, 0);
    }

    #[test]
    fn the_aggregate_is_computed_and_not_kept() {
        let mut profile = StrokeProfile::default();
        profile.record("Padrão", Phase::EngineEdit, ms(3), Work::bricks(27));
        profile.record("Suavizar", Phase::EngineEdit, ms(9), Work::bricks(31));

        let whole = profile.across_tools().phase(Phase::EngineEdit);
        assert_eq!(whole.seen(), 2);
        assert_eq!(whole.worst(), Some(ms(9)));
        assert_eq!(whole.work().bricks, 58);

        // And the per-tool figures are untouched by having been aggregated.
        assert_eq!(profile.tools().count(), 2);
        let smooth = profile
            .tools()
            .find(|(tool, _)| *tool == "Suavizar")
            .expect("the tool that ran")
            .1;
        assert_eq!(smooth.phase(Phase::EngineEdit).seen(), 1);
    }

    #[test]
    fn a_tool_is_asked_for_a_phase_it_never_ran() {
        let mut profile = StrokeProfile::default();
        profile.record("Padrão", Phase::EngineEdit, ms(3), Work::bricks(1));

        let tool = profile.tools().next().expect("the tool that ran").1;
        assert!(tool.phase(Phase::Upload).is_empty());
        assert_eq!(tool.phase(Phase::Upload).worst(), None);
    }

    #[test]
    fn every_phase_says_which_side_of_the_boundary_it_is_on() {
        let engine: Vec<&str> = Phase::ALL
            .iter()
            .filter(|phase| phase.is_engine())
            .map(|phase| phase.label())
            .collect();
        assert_eq!(engine, ["engine edit", "engine mesh"]);
        assert!(Phase::ALL.iter().all(|phase| !phase.label().is_empty()));
        assert!(Phase::EngineMesh.entry_point().is_some());
        assert!(Phase::Upload.entry_point().is_none());
    }

    #[test]
    fn an_empty_profile_says_so() {
        assert!(StrokeProfile::default().is_empty());
        assert!(StrokeProfile::default().across_tools().is_empty());
    }
}
