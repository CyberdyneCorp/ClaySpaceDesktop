//! What one run of the benchmark collected.
//!
//! Figures, the skips that explain the figures that are not here, and what
//! each group cost to measure. A group is handed this and fills it in; the
//! report, the baseline writer and the comparison all read it.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::figures::{Figure, Record};
use crate::skip::Skip;

pub struct Run {
    figures: BTreeMap<String, Figure>,
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
            skips: BTreeMap::new(),
            durations: Vec::new(),
            filter,
            group: None,
        }
    }

    pub fn figures(&self) -> &BTreeMap<String, Figure> {
        &self.figures
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

    /// Records the timings of one measurement, under the names its record
    /// kind gives them.
    pub fn timings(&mut self, prefix: &str, record: Record, samples: Vec<f64>) {
        for (name, figure) in record.figures(prefix, samples) {
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
