//! Where session state lives on this machine.
//!
//! The rules are in `clayspace_model::session`; this is the disk. Kept apart
//! because the rules are worth testing and the directory layout is worth
//! writing down once — and because a test of "does a reopened document move up
//! the recent list" should not need a home directory.

use std::path::{Path, PathBuf};

use clayspace_model::{RecentDocuments, Recovery};

/// The application's own directory, and the files in it.
#[derive(Debug, Clone)]
pub struct SessionStore {
    root: PathBuf,
}

impl SessionStore {
    /// The per-user directory this platform expects.
    ///
    /// macOS puts application state in Application Support. Linux follows the
    /// XDG base directory specification, honouring `XDG_STATE_HOME` where it
    /// is set — this is state, not configuration and not a cache: losing it
    /// costs unsaved work, so it does not belong under `~/.cache`.
    pub fn discover() -> Option<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let root = if cfg!(target_os = "macos") {
            home.join("Library/Application Support/ClaySpaceDesktop")
        } else {
            std::env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local/state"))
                .join("clayspace")
        };
        Some(Self::at(root))
    }

    /// A store rooted anywhere, for tests and for a portable install.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the autosave is written.
    pub fn autosave_path(&self) -> PathBuf {
        self.root.join("recuperação.clayspace")
    }

    /// The file whose presence means a session did not close.
    fn marker_path(&self) -> PathBuf {
        self.root.join("sessão.aberta")
    }

    fn recent_path(&self) -> PathBuf {
        self.root.join("recentes.txt")
    }

    fn ensure_root(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)
    }

    /// What the previous session left behind.
    ///
    /// Read before the marker for this session is written, or every run would
    /// find its own marker and offer to recover from itself.
    pub fn recovery(&self) -> Recovery {
        let autosave = self.autosave_path();
        Recovery::assess(
            self.marker_path().is_file(),
            autosave.is_file().then_some(autosave),
        )
    }

    /// Marks this session as open.
    ///
    /// Failure is not fatal and not reported to the user: a read-only home
    /// directory costs the recovery offer, not the ability to sculpt.
    pub fn begin_session(&self) {
        if self.ensure_root().is_ok() {
            let _ = std::fs::write(self.marker_path(), b"aberta\n");
        }
    }

    /// Marks this session as closed cleanly, and clears the autosave.
    ///
    /// Both, in that order. A marker removed while the autosave stays behind
    /// is the ordinary case and harmless; an autosave removed while the marker
    /// stays would turn a clean exit into a recovery offer with nothing behind
    /// it on the next run.
    pub fn end_session(&self) {
        let _ = std::fs::remove_file(self.marker_path());
        let _ = std::fs::remove_file(self.autosave_path());
    }

    /// Discards a recovery file the user declined, so it is offered once.
    pub fn discard_recovery(&self) {
        let _ = std::fs::remove_file(self.autosave_path());
    }

    /// Reads the recent list, dropping entries whose file is gone.
    pub fn load_recent(&self) -> RecentDocuments {
        let text = std::fs::read_to_string(self.recent_path()).unwrap_or_default();
        let mut recent = RecentDocuments::from_paths(
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(PathBuf::from),
            RECENT_LIMIT,
        );
        // Pruned on the way in rather than on the way out: a menu that lists a
        // document and then fails to open it is worse than a shorter menu.
        recent.prune(|path| path.is_file());
        recent
    }

    pub fn save_recent(&self, recent: &RecentDocuments) {
        if self.ensure_root().is_err() {
            return;
        }
        let mut text = String::new();
        for path in recent.paths() {
            text.push_str(&path.to_string_lossy());
            text.push('\n');
        }
        let _ = std::fs::write(self.recent_path(), text);
    }
}

/// How many documents the recent menu holds.
const RECENT_LIMIT: usize = 10;

#[cfg(test)]
mod tests {
    use super::*;

    /// A store in a directory of its own, removed when the test ends.
    struct Scratch(SessionStore);

    impl Scratch {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("clayspace-session-{name}"));
            let _ = std::fs::remove_dir_all(&root);
            Self(SessionStore::at(root))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.root());
        }
    }

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(path, b"x").expect("write the file");
    }

    #[test]
    fn a_clean_exit_leaves_nothing_to_recover() {
        let scratch = Scratch::new("clean");
        let store = &scratch.0;
        store.begin_session();
        touch(&store.autosave_path());

        store.end_session();
        assert_eq!(store.recovery(), Recovery::Nothing);
        assert!(!store.autosave_path().exists(), "the autosave outlived it");
    }

    #[test]
    fn a_session_that_never_closed_offers_its_autosave() {
        let scratch = Scratch::new("crash");
        let store = &scratch.0;
        store.begin_session();
        touch(&store.autosave_path());

        // No end_session: this is what a crash looks like from the outside.
        assert_eq!(
            store.recovery(),
            Recovery::Available(store.autosave_path()),
            "unsaved work was not offered back"
        );
    }

    #[test]
    fn a_crash_before_anything_was_saved_offers_nothing() {
        let scratch = Scratch::new("early-crash");
        let store = &scratch.0;
        store.begin_session();
        assert_eq!(store.recovery(), Recovery::Nothing);
    }

    #[test]
    fn a_declined_recovery_is_not_offered_twice() {
        let scratch = Scratch::new("declined");
        let store = &scratch.0;
        store.begin_session();
        touch(&store.autosave_path());
        assert!(matches!(store.recovery(), Recovery::Available(_)));

        store.discard_recovery();
        assert_eq!(store.recovery(), Recovery::Nothing);
    }

    #[test]
    fn the_recent_list_round_trips_through_the_disk() {
        let scratch = Scratch::new("recent");
        let store = &scratch.0;
        let a = store.root().join("a.clayspace");
        let b = store.root().join("b.clayspace");
        touch(&a);
        touch(&b);

        let mut recent = RecentDocuments::default();
        recent.remember(&a);
        recent.remember(&b);
        store.save_recent(&recent);

        assert_eq!(store.load_recent().paths(), [b, a]);
    }

    #[test]
    fn a_recent_entry_whose_file_is_gone_is_dropped_on_the_way_in() {
        // A menu that lists a document and then fails to open it is worse than
        // a shorter menu.
        let scratch = Scratch::new("pruned");
        let store = &scratch.0;
        let kept = store.root().join("kept.clayspace");
        let removed = store.root().join("removed.clayspace");
        touch(&kept);
        touch(&removed);

        let mut recent = RecentDocuments::default();
        recent.remember(&kept);
        recent.remember(&removed);
        store.save_recent(&recent);
        std::fs::remove_file(&removed).expect("remove");

        assert_eq!(store.load_recent().paths(), [kept]);
    }

    #[test]
    fn a_store_with_no_directory_yet_reads_as_empty_rather_than_failing() {
        let scratch = Scratch::new("absent");
        assert!(scratch.0.load_recent().is_empty());
        assert_eq!(scratch.0.recovery(), Recovery::Nothing);
    }

    #[test]
    fn the_discovered_root_is_under_the_home_directory() {
        let Some(store) = SessionStore::discover() else {
            return; // No HOME, which is a valid environment for a test runner.
        };
        let home = std::env::var("HOME").expect("HOME, since discover found it");
        assert!(
            store.root().starts_with(&home),
            "session state escaped the home directory: {}",
            store.root().display()
        );
    }
}
