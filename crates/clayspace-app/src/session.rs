//! Where session state lives on this machine.
//!
//! The rules are in `clayspace_model::session`; this is the disk. Kept apart
//! because the rules are worth testing and the directory layout is worth
//! writing down once — and because a test of "does a reopened document move up
//! the recent list" should not need a home directory.

use std::path::{Path, PathBuf};

use clayspace_model::ToolKind;
use clayspace_model::{Locale, RecentDocuments, Recovery, RememberedReference};
use clayspace_view::{Layout, ViewportProfile};

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

    fn reference_path(&self) -> PathBuf {
        self.root.join("referências.txt")
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
    /// The language the interface was last set to.
    ///
    /// `None` where nothing has been chosen — which is a first run, and is
    /// what lets the system's own language be honoured that once instead of
    /// being overruled by a preference nobody set.
    pub fn load_locale(&self) -> Option<Locale> {
        let tag = std::fs::read_to_string(self.root.join("locale")).ok()?;
        let tag = tag.trim();
        // Only a tag we actually recognise. `from_tag` answers with the
        // default for anything else, and taking that would turn a corrupted
        // file into a silent preference the user never set.
        Locale::ALL.into_iter().find(|locale| locale.tag() == tag)
    }

    pub fn save_locale(&self, locale: Locale) {
        if self.ensure_root().is_err() {
            return;
        }
        let _ = std::fs::write(self.root.join("locale"), locale.tag());
    }

    /// The reference images the last session had placed.
    ///
    /// Pruned on the way in, like the recent list and for the same reason: a
    /// plane that says it holds a drawing and shows nothing is worse than an
    /// empty plane.
    pub fn load_references(&self) -> Vec<RememberedReference> {
        let text = std::fs::read_to_string(self.reference_path()).unwrap_or_default();
        let mut entries = clayspace_model::read_references(&text);
        entries.retain(|entry| entry.path.is_file());
        entries
    }

    pub fn save_references(&self, entries: &[RememberedReference]) {
        if self.ensure_root().is_err() {
            return;
        }
        let _ = std::fs::write(
            self.reference_path(),
            clayspace_model::write_references(entries),
        );
    }

    /// How the regions were arranged when the application last closed.
    ///
    /// State rather than configuration, in the directory the recent list and
    /// the locale already live in. `Layout::deserialize` falls back to the
    /// design's own sizes on anything malformed and clamps a size written by a
    /// version with different bounds, so a corrupt line costs a sculptor their
    /// arrangement and never their start-up.
    ///
    /// `layout.rs` has carried the sizes, the minimums, the collapse state and
    /// this pair of serialisers since it was written, and nothing had ever
    /// called them: the regions were drawn at fixed widths and the module was
    /// exported from `clayspace-view` to no consumer at all.
    pub fn load_layout(&self) -> Layout {
        std::fs::read_to_string(self.layout_path())
            .map(|text| Layout::deserialize(&text))
            .unwrap_or_default()
    }

    pub fn save_layout(&self, layout: &Layout) {
        if self.ensure_root().is_err() {
            return;
        }
        let _ = std::fs::write(self.layout_path(), layout.serialize());
    }

    fn layout_path(&self) -> PathBuf {
        self.root.join("layout")
    }

    /// The brushes this sculptor starred.
    ///
    /// One key a line, and a key this build does not recognise is dropped
    /// rather than failing the file: a shortlist written by a version with a
    /// brush this one has not got should still bring back the rest.
    pub fn load_favourites(&self) -> Vec<ToolKind> {
        let text = std::fs::read_to_string(self.favourites_path()).unwrap_or_default();
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter_map(|line| ToolKind::ALL.into_iter().find(|tool| tool.key() == line))
            .collect()
    }

    pub fn save_favourites(&self, favourites: &[ToolKind]) {
        if self.ensure_root().is_err() {
            return;
        }
        let mut text = String::new();
        for tool in favourites {
            text.push_str(tool.key());
            text.push('\n');
        }
        let _ = std::fs::write(self.favourites_path(), text);
    }

    fn favourites_path(&self) -> PathBuf {
        self.root.join("favourites")
    }

    /// How much an idle frame is worth spending on.
    ///
    /// Only a tier this build recognises, as the locale is: taking a default
    /// for anything else would turn a corrupted file into a silent preference
    /// nobody set.
    pub fn load_viewport_profile(&self) -> Option<ViewportProfile> {
        let text = std::fs::read_to_string(self.root.join("viewport-profile")).ok()?;
        let name = text.trim();
        ViewportProfile::ALL
            .into_iter()
            .find(|profile| profile.key() == name)
    }

    pub fn save_viewport_profile(&self, profile: ViewportProfile) {
        if self.ensure_root().is_err() {
            return;
        }
        let _ = std::fs::write(self.root.join("viewport-profile"), profile.key());
    }

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

    /// The arrangement of the regions survives a restart.
    ///
    /// `layout.rs` carried `serialize` and `deserialize` from the day it was
    /// written, with a note that they exist for "a line the composition root
    /// can store". Nothing had ever stored one.
    #[test]
    fn a_layout_survives_a_restart() {
        let store = Scratch::new("layout-round-trip");
        let mut layout = Layout::default();
        layout.resize(clayspace_view::Panel::Left, 300.0);
        layout.set_collapsed(clayspace_view::Panel::Shelf, true);
        store.0.save_layout(&layout);

        assert_eq!(
            store.0.load_layout(),
            layout,
            "the arrangement did not come back"
        );
    }

    /// A machine with nothing stored opens at the design's own sizes.
    #[test]
    fn nothing_stored_is_the_designs_arrangement() {
        let store = Scratch::new("layout-absent");
        assert_eq!(store.0.load_layout(), Layout::default());
    }

    /// And a corrupt line costs the arrangement, never the start-up.
    ///
    /// The whole reason `deserialize` falls back rather than failing: a
    /// sculptor whose layout file was truncated by a full disk should meet the
    /// design's arrangement, not an application that will not open.
    #[test]
    fn a_corrupt_layout_costs_the_arrangement_and_not_the_start_up() {
        let store = Scratch::new("layout-corrupt");
        std::fs::create_dir_all(store.0.root()).expect("a place to write");
        std::fs::write(store.0.root().join("layout"), "not a layout at all").expect("write");
        assert_eq!(store.0.load_layout(), Layout::default());
    }

    /// A shortlist of brushes survives a restart.
    #[test]
    fn favourites_survive_a_restart() {
        let store = Scratch::new("favourites");
        let starred = [ToolKind::Argila, ToolKind::Polir];
        store.0.save_favourites(&starred);
        assert_eq!(store.0.load_favourites(), starred.to_vec());
    }

    /// A brush this build does not know is dropped, and the rest come back.
    ///
    /// A shortlist written by a version carrying a brush this one has not got
    /// should cost that entry and not the file.
    #[test]
    fn an_unknown_brush_costs_its_own_line() {
        let store = Scratch::new("favourites-unknown");
        std::fs::create_dir_all(store.0.root()).expect("a place to write");
        std::fs::write(
            store.0.root().join("favourites"),
            "clay\nsomething-this-build-has-never-heard-of\npolish\n",
        )
        .expect("write");
        assert_eq!(
            store.0.load_favourites(),
            vec![ToolKind::Argila, ToolKind::Polir]
        );
    }

    /// The viewport profile survives a restart, and only a real tier loads.
    #[test]
    fn a_viewport_profile_survives_a_restart() {
        let store = Scratch::new("profile");
        store.0.save_viewport_profile(ViewportProfile::Presentation);
        assert_eq!(
            store.0.load_viewport_profile(),
            Some(ViewportProfile::Presentation)
        );

        std::fs::write(store.0.root().join("viewport-profile"), "luxurious").expect("write");
        assert_eq!(
            store.0.load_viewport_profile(),
            None,
            "a tier this build does not know should not become a silent preference"
        );
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
    fn the_placed_references_survive_a_restart() {
        use clayspace_model::{RefPlane, ReferenceSettings};

        let scratch = Scratch::new("references");
        let store = &scratch.0;
        let drawing = store.root().join("frente.png");
        touch(&drawing);
        let gone = store.root().join("perdido.png");
        touch(&gone);

        let entries = vec![
            RememberedReference {
                plane: RefPlane::Front,
                path: drawing.clone(),
                settings: ReferenceSettings {
                    opacity: 0.25,
                    height: 3.0,
                    ..ReferenceSettings::default()
                },
            },
            RememberedReference {
                plane: RefPlane::Side,
                path: gone.clone(),
                settings: ReferenceSettings::default(),
            },
        ];
        store.save_references(&entries);
        std::fs::remove_file(&gone).expect("remove");

        let read = store.load_references();
        assert_eq!(read.len(), 1, "a file that is gone was offered anyway");
        assert_eq!(read[0].path, drawing);
        assert!((read[0].settings.opacity - 0.25).abs() < 1e-6);
        assert!((read[0].settings.height - 3.0).abs() < 1e-6);
    }

    #[test]
    fn a_store_with_no_directory_yet_reads_as_empty_rather_than_failing() {
        let scratch = Scratch::new("absent");
        assert!(scratch.0.load_recent().is_empty());
        assert!(scratch.0.load_references().is_empty());
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
