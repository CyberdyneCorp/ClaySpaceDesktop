//! What one run of the benchmark collected.
//!
//! Figures, the skips that explain the figures that are not here, and what
//! each group cost to measure. A group is handed this and fills it in; the
//! report, the baseline writer and the comparison all read it.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::figures::{Figure, Record, Spread};
use crate::skip::Skip;

pub struct Run {
    figures: BTreeMap<String, Figure>,
    /// What the samples behind a figure looked like, where the measurement
    /// took more than one.
    ///
    /// Keyed by the figure's own name rather than by the measurement's prefix,
    /// even though `.mean` and `.p95` are reduced from one sample set and so
    /// share a spread. The comparison asks the question one figure at a time,
    /// and a lookup that had to strip a suffix to find the answer would be one
    /// renamed figure away from silently finding nothing.
    spreads: BTreeMap<String, Spread>,
    /// Keyed by the name — or the name prefix — the missing figures share.
    skips: BTreeMap<String, Skip>,
    durations: Vec<(&'static str, Duration)>,
    /// `--only`: measure and report figures whose name starts with this.
    filter: Option<String>,
    /// The group being measured, which every figure it emits is named under.
    group: Option<&'static str>,
}

impl Run {
    pub fn new(filter: Option<String>) -> Self {
        Self {
            figures: BTreeMap::new(),
            spreads: BTreeMap::new(),
            skips: BTreeMap::new(),
            durations: Vec::new(),
            filter,
            group: None,
        }
    }

    pub fn figures(&self) -> &BTreeMap<String, Figure> {
        &self.figures
    }

    pub fn spreads(&self) -> &BTreeMap<String, Spread> {
        &self.spreads
    }

    pub fn skips(&self) -> &BTreeMap<String, Skip> {
        &self.skips
    }

    pub fn durations(&self) -> &[(&'static str, Duration)] {
        &self.durations
    }

    pub fn is_filtered(&self) -> bool {
        self.filter.is_some()
    }

    /// Measures one group, timing it.
    ///
    /// `title` is the prefix every figure the group emits is named under, so
    /// that a filter can decide whether to run the group at all rather than
    /// building its scenes and discarding the result.
    pub fn group(&mut self, title: &'static str, measure: impl FnOnce(&mut Self)) {
        if !self.wants_group(title) {
            return;
        }
        let started = Instant::now();
        self.group = Some(title);
        measure(self);
        self.group = None;
        self.durations.push((title, started.elapsed()));
    }

    pub fn insert(&mut self, name: impl Into<String>, figure: Figure) {
        let name = name.into();
        debug_assert!(
            self.group.is_none_or(|title| name.starts_with(title)),
            "figure {name} is not named under its group {:?}",
            self.group
        );
        if !self.wants(&name) {
            return;
        }
        self.figures.insert(name, figure);
    }

    /// Inserts a figure only if that name is not already taken.
    pub fn insert_once(&mut self, name: impl Into<String>, figure: impl FnOnce() -> Figure) {
        let name = name.into();
        if self.figures.contains_key(&name) {
            return;
        }
        self.insert(name, figure());
    }

    /// Keeps what the samples behind a figure looked like.
    ///
    /// Called by [`Run::timings`] for everything routed through it, and by
    /// hand from the few groups that take their own quantiles. A measurement
    /// that genuinely has one observation records none, which is the honest
    /// answer and is visible as a blank in the report rather than as a
    /// confident range over a sample of one.
    pub fn spread(&mut self, name: &str, samples: &[f64]) {
        if !self.wants(name) {
            return;
        }
        if let Some(spread) = Spread::of(samples) {
            self.spreads.insert(name.to_string(), spread);
        }
    }

    /// Records the timings of one measurement, under the names its record
    /// kind gives them.
    pub fn timings(&mut self, prefix: &str, record: Record, samples: Vec<f64>) {
        for (name, figure) in record.figures(prefix, &samples) {
            self.spread(&name, &samples);
            self.insert(name, figure);
        }
    }

    /// Says that everything named under `prefix` is not here, and why.
    ///
    /// Returns nothing on purpose: a measurement's last act is to say why it
    /// stopped, and there is nothing sensible to do with the result.
    pub fn skip(&mut self, prefix: impl Into<String>, why: Skip) {
        let prefix = prefix.into();
        // Recorded when the filter would have wanted anything under it, which
        // is the same question a group asks before running at all.
        if !self.wants_group(&prefix) {
            return;
        }
        self.skips.insert(prefix, why);
    }

    /// Whether a figure of this name is wanted by the filter.
    pub fn wants(&self, name: &str) -> bool {
        self.filter
            .as_deref()
            .is_none_or(|filter| name.starts_with(filter))
    }

    /// Whether a group whose figures share this prefix has anything to do.
    ///
    /// A filter narrower than the group still wants the group — `brush.voxel`
    /// is measured by the `brush` group, which then discards what does not
    /// match.
    pub fn wants_group(&self, title: &str) -> bool {
        self.filter
            .as_deref()
            .is_none_or(|filter| filter.starts_with(title) || title.starts_with(filter))
    }
}
