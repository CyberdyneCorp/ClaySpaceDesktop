//! What survives between sessions: unsaved work, and where you have been.
//!
//! The rules live here, without a filesystem, because they are the part worth
//! testing. Whether a recovery file is offered, whether an autosave is due,
//! and what the recent list looks like after a document is opened twice are
//! all decidable from values; only the reading and writing needs a disk, and
//! that is the composition root's business.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// When to write an autosave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutosavePolicy {
    /// How long between autosaves of a document that keeps changing.
    pub every: Duration,
}

impl Default for AutosavePolicy {
    fn default() -> Self {
        // Two minutes. Long enough that a large document is not written
        // constantly, short enough that a crash costs a recognisable amount of
        // work rather than an afternoon.
        Self {
            every: Duration::from_secs(120),
        }
    }
}

impl AutosavePolicy {
    /// Whether an autosave should happen now.
    ///
    /// An unmodified document is never written. Autosave exists to preserve
    /// work that is not on disk; rewriting a file that already matches costs
    /// I/O and, worse, keeps the recovery file looking fresh when there is
    /// nothing to recover.
    pub fn is_due(&self, since_last: Duration, modified: bool) -> bool {
        modified && since_last >= self.every
    }

    /// How long until the next autosave could be due.
    ///
    /// `None` when there is nothing to save, which is what lets the event loop
    /// go back to waiting indefinitely instead of ticking at an idle
    /// application.
    pub fn next_in(&self, since_last: Duration, modified: bool) -> Option<Duration> {
        modified.then(|| self.every.saturating_sub(since_last))
    }
}

/// What a previous session left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovery {
    /// The last session closed cleanly, or left nothing worth recovering.
    Nothing,
    /// A session ended without closing, and this file holds its last autosave.
    Available(PathBuf),
}

impl Recovery {
    /// What a session's leftovers mean.
    ///
    /// Both conditions are required. A marker with no autosave is a crash that
    /// happened before anything was worth saving, and offering to recover an
    /// empty document is worse than saying nothing. An autosave with no marker
    /// is the ordinary case — the file is left in place after a clean exit and
    /// simply overwritten next time.
    pub fn assess(marker_present: bool, autosave: Option<PathBuf>) -> Self {
        match (marker_present, autosave) {
            (true, Some(path)) => Self::Available(path),
            _ => Self::Nothing,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Available(path) => Some(path),
            Self::Nothing => None,
        }
    }
}

/// The documents opened lately, most recent first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentDocuments {
    paths: Vec<PathBuf>,
    limit: usize,
}

impl Default for RecentDocuments {
    fn default() -> Self {
        Self::new(10)
    }
}

impl RecentDocuments {
    pub fn new(limit: usize) -> Self {
        Self {
            // A limit of zero would make `remember` a no-op that looks like a
            // bug; one entry is the smallest list that is still a list.
            limit: limit.max(1),
            paths: Vec::new(),
        }
    }

    /// Builds from stored lines, keeping the order they were written in.
    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>, limit: usize) -> Self {
        let mut list = Self::new(limit);
        // Through `remember` in reverse so duplicates and the limit are
        // applied by the same rules that apply at runtime — a stored file is
        // not more trustworthy than a live one.
        for path in paths.into_iter().collect::<Vec<_>>().into_iter().rev() {
            list.remember(path);
        }
        list
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Puts a document at the front, moving rather than duplicating it.
    pub fn remember(&mut self, path: impl Into<PathBuf>) {
        let path = path.into();
        self.paths.retain(|known| known != &path);
        self.paths.insert(0, path);
        self.paths.truncate(self.limit);
    }

    /// Drops entries the predicate says are gone.
    ///
    /// Taken as a predicate rather than reaching for the filesystem so the
    /// rule can be tested without one — and so a slow or missing network
    /// volume is the caller's problem to decide about, not this type's.
    pub fn prune(&mut self, exists: impl Fn(&Path) -> bool) {
        self.paths.retain(|path| exists(path));
    }

    /// The name to show for an entry, falling back to the whole path.
    pub fn label(path: &Path) -> String {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> AutosavePolicy {
        AutosavePolicy {
            every: Duration::from_secs(60),
        }
    }

    #[test]
    fn an_unmodified_document_is_never_autosaved() {
        // However long it has been. Rewriting a file that already matches
        // keeps the recovery file looking fresh when there is nothing to
        // recover.
        assert!(!policy().is_due(Duration::from_secs(600), false));
        assert_eq!(policy().next_in(Duration::from_secs(600), false), None);
    }

    #[test]
    fn a_modified_document_is_autosaved_once_the_interval_has_passed() {
        assert!(!policy().is_due(Duration::from_secs(59), true));
        assert!(policy().is_due(Duration::from_secs(60), true));
        assert!(policy().is_due(Duration::from_secs(600), true));
    }

    #[test]
    fn the_wait_shrinks_and_never_goes_negative() {
        assert_eq!(
            policy().next_in(Duration::from_secs(20), true),
            Some(Duration::from_secs(40))
        );
        // Overdue is zero, not a panic on subtraction.
        assert_eq!(
            policy().next_in(Duration::from_secs(90), true),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn recovery_needs_both_a_marker_and_a_file() {
        let file = PathBuf::from("/tmp/auto.clayspace");
        assert_eq!(
            Recovery::assess(true, Some(file.clone())),
            Recovery::Available(file.clone())
        );
        // A crash before anything was worth saving: offering to recover an
        // empty document is worse than saying nothing.
        assert_eq!(Recovery::assess(true, None), Recovery::Nothing);
        // The ordinary case: the file is left behind by a clean exit.
        assert_eq!(Recovery::assess(false, Some(file)), Recovery::Nothing);
        assert_eq!(Recovery::assess(false, None), Recovery::Nothing);
    }

    #[test]
    fn a_document_opened_again_moves_up_rather_than_appearing_twice() {
        let mut recent = RecentDocuments::new(5);
        recent.remember("/a.clayspace");
        recent.remember("/b.clayspace");
        recent.remember("/a.clayspace");
        assert_eq!(
            recent.paths(),
            [PathBuf::from("/a.clayspace"), PathBuf::from("/b.clayspace")]
        );
    }

    #[test]
    fn the_list_stops_at_its_limit() {
        let mut recent = RecentDocuments::new(2);
        for name in ["/a", "/b", "/c"] {
            recent.remember(name);
        }
        assert_eq!(recent.paths(), [PathBuf::from("/c"), PathBuf::from("/b")]);
    }

    #[test]
    fn a_stored_list_round_trips_in_order() {
        let mut recent = RecentDocuments::new(4);
        recent.remember("/a");
        recent.remember("/b");
        recent.remember("/c");
        let restored = RecentDocuments::from_paths(recent.paths().to_vec(), 4);
        assert_eq!(restored, recent);
    }

    #[test]
    fn a_stored_list_is_held_to_the_same_rules_as_a_live_one() {
        // A file edited by hand, or written by an older build with a larger
        // limit, must not produce a list the application would never build.
        let stored = ["/a", "/b", "/a", "/c"].map(PathBuf::from);
        let restored = RecentDocuments::from_paths(stored, 2);
        assert_eq!(restored.paths(), [PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn pruning_drops_what_is_gone_and_keeps_the_rest_in_order() {
        let mut recent = RecentDocuments::new(5);
        for name in ["/gone", "/here", "/also-gone", "/still-here"] {
            recent.remember(name);
        }
        recent.prune(|path| !path.to_string_lossy().contains("gone"));
        assert_eq!(
            recent.paths(),
            [PathBuf::from("/still-here"), PathBuf::from("/here")]
        );
    }

    #[test]
    fn an_entry_is_labelled_by_its_file_name() {
        assert_eq!(
            RecentDocuments::label(Path::new("/estudos/Cabeça_v03.clayspace")),
            "Cabeça_v03.clayspace"
        );
        // And something with no file name still says something.
        assert!(!RecentDocuments::label(Path::new("/")).is_empty());
    }

    #[test]
    fn a_zero_limit_still_holds_one() {
        let mut recent = RecentDocuments::new(0);
        recent.remember("/a");
        assert_eq!(recent.paths().len(), 1);
    }
}
