//! The history, as a list a user can read and move within.
//!
//! The engine owns undo; this owns what the entries are *called*. An engine
//! reports that something can be undone, not that it was a Padrão stroke, and
//! a panel listing "edit, edit, edit" is not a history anyone can navigate.
//!
//! The list is therefore kept alongside the engine's stack and reconciled with
//! its depth after every operation. Where the two disagree the engine wins:
//! it is the one that actually holds the document.

use clayspace_model::{HistoryEntry, HistoryState};

use crate::observable::Observable;

/// What the history panel reads.
pub struct HistoryViewModel {
    entries: Observable<Vec<HistoryEntry>>,
    state: Observable<HistoryState>,
    /// How many entries are kept before the oldest are discarded.
    depth_limit: usize,
}

impl HistoryViewModel {
    /// The default bound. Deep enough that a session rarely reaches it,
    /// shallow enough that the memory is bounded, and shown to the user.
    pub const DEFAULT_DEPTH: usize = 128;

    pub fn new(state: HistoryState) -> Self {
        Self {
            entries: Observable::new(Vec::new()),
            state: Observable::new(state),
            depth_limit: Self::DEFAULT_DEPTH,
        }
    }

    pub fn entries(&self) -> &Observable<Vec<HistoryEntry>> {
        &self.entries
    }

    pub fn state(&self) -> &Observable<HistoryState> {
        &self.state
    }

    pub fn depth_limit(&self) -> usize {
        self.depth_limit
    }

    pub fn set_depth_limit(&mut self, limit: usize) {
        self.depth_limit = limit.max(1);
        self.trim();
    }

    /// How far back the current position is, counted from the newest entry.
    pub fn position(&self) -> usize {
        self.entries.get().iter().filter(|e| !e.undone).count()
    }

    /// Records an operation that changed the document.
    ///
    /// A new edit made while positioned before the end discards the redo
    /// entries beyond it — the engine does the same to its stack, and a panel
    /// still listing them would be lying.
    pub fn record(&mut self, label: impl Into<String>, state: HistoryState) {
        let discarded = self.discard_redo_branch();
        self.entries.update(|entries| {
            entries.push(HistoryEntry {
                label: label.into(),
                undone: false,
            });
        });
        self.trim();
        self.state.set_if_changed(state);
        let _ = discarded;
    }

    /// Marks the newest live entry as undone.
    pub fn undone(&mut self, state: HistoryState) {
        self.entries.update(|entries| {
            if let Some(entry) = entries.iter_mut().rev().find(|e| !e.undone) {
                entry.undone = true;
            }
        });
        self.state.set_if_changed(state);
    }

    /// Marks the oldest undone entry as live again.
    pub fn redone(&mut self, state: HistoryState) {
        self.entries.update(|entries| {
            if let Some(entry) = entries.iter_mut().find(|e| e.undone) {
                entry.undone = false;
            }
        });
        self.state.set_if_changed(state);
    }

    /// How many undo or redo steps reach the given entry.
    ///
    /// Positive means undo that many times; negative means redo. The
    /// interface turns this into a run of ordinary commands rather than a
    /// separate jump operation the engine does not have.
    pub fn steps_to(&self, index: usize) -> i32 {
        let entries = self.entries.get();
        if index >= entries.len() {
            return 0;
        }
        let live = entries.iter().filter(|e| !e.undone).count() as i32;
        // Selecting entry `index` means everything up to and including it is
        // live, and everything after it is not.
        let wanted = index as i32 + 1;
        live - wanted
    }

    /// Whether the entries and the engine's own depth still agree.
    ///
    /// They can drift: the engine coalesces, and an operation may produce no
    /// entry at all. The panel prefers the engine's count and says so rather
    /// than showing a list that has quietly diverged.
    pub fn agrees_with_engine(&self) -> bool {
        self.position() == self.state.get().depth
    }

    /// Drops entries beyond the current position.
    fn discard_redo_branch(&mut self) -> usize {
        let undone = self.entries.get().iter().filter(|e| e.undone).count();
        if undone > 0 {
            self.entries.update(|entries| entries.retain(|e| !e.undone));
        }
        undone
    }

    /// Discards the oldest entries once the bound is reached.
    fn trim(&mut self) {
        let limit = self.depth_limit;
        if self.entries.get().len() > limit {
            self.entries.update(|entries| {
                let excess = entries.len() - limit;
                entries.drain(0..excess);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(depth: usize, redo: usize) -> HistoryState {
        HistoryState {
            can_undo: depth > 0,
            can_redo: redo > 0,
            depth,
            redo_depth: redo,
        }
    }

    fn with_entries(labels: &[&str]) -> HistoryViewModel {
        let mut history = HistoryViewModel::new(state(0, 0));
        for (index, label) in labels.iter().enumerate() {
            history.record(*label, state(index + 1, 0));
        }
        history
    }

    #[test]
    fn entries_are_named_for_what_they_did() {
        let history = with_entries(&["Padrão", "Suavizar", "Mover"]);
        let names: Vec<_> = history
            .entries()
            .get()
            .iter()
            .map(|e| e.label.as_str())
            .collect();
        assert_eq!(names, ["Padrão", "Suavizar", "Mover"]);
    }

    #[test]
    fn undo_moves_the_position_without_dropping_the_entry() {
        let mut history = with_entries(&["Padrão", "Suavizar"]);
        assert_eq!(history.position(), 2);

        history.undone(state(1, 1));
        assert_eq!(history.position(), 1);
        assert_eq!(
            history.entries().get().len(),
            2,
            "an undone entry stays in the list so it can be redone"
        );

        history.redone(state(2, 0));
        assert_eq!(history.position(), 2);
    }

    #[test]
    fn a_new_edit_discards_the_redo_branch() {
        let mut history = with_entries(&["Padrão", "Suavizar", "Mover"]);
        history.undone(state(2, 1));
        history.undone(state(1, 2));
        assert_eq!(history.position(), 1);

        history.record("Inflar", state(2, 0));

        let entries = history.entries().get();
        assert_eq!(entries.len(), 2, "the undone entries should be gone");
        assert_eq!(entries[1].label, "Inflar");
        assert!(
            entries.iter().all(|e| !e.undone),
            "a discarded branch left an undone entry behind"
        );
    }

    #[test]
    fn jumping_back_is_a_run_of_undos() {
        let history = with_entries(&["a", "b", "c", "d"]);
        // Selecting the second entry means two live, so two undos.
        assert_eq!(history.steps_to(1), 2);
        assert_eq!(history.steps_to(3), 0, "the newest entry is where we are");
        assert_eq!(history.steps_to(0), 3);
    }

    #[test]
    fn jumping_forward_is_a_run_of_redos() {
        let mut history = with_entries(&["a", "b", "c"]);
        history.undone(state(2, 1));
        history.undone(state(1, 2));
        // One live; selecting the third entry needs two redos.
        assert_eq!(history.steps_to(2), -2);
    }

    #[test]
    fn the_oldest_entries_are_discarded_at_the_bound() {
        let mut history = HistoryViewModel::new(state(0, 0));
        history.set_depth_limit(3);
        for i in 0..6 {
            history.record(format!("edit {i}"), state(i + 1, 0));
        }

        let entries = history.entries().get();
        assert_eq!(entries.len(), 3, "the bound was not applied");
        assert_eq!(
            entries[0].label, "edit 3",
            "the oldest entries should be the ones discarded"
        );
    }

    #[test]
    fn a_depth_limit_of_zero_is_refused() {
        let mut history = HistoryViewModel::new(state(0, 0));
        history.set_depth_limit(0);
        assert!(
            history.depth_limit() >= 1,
            "a history that keeps nothing cannot undo anything"
        );
    }

    #[test]
    fn the_panel_notices_when_it_disagrees_with_the_engine() {
        let mut history = with_entries(&["Padrão"]);
        assert!(history.agrees_with_engine());

        // The engine coalesced, or the operation produced no entry.
        history.state.set(state(5, 0));
        assert!(
            !history.agrees_with_engine(),
            "the panel should notice a divergence rather than show a stale list"
        );
    }

    #[test]
    fn recording_the_same_state_twice_still_adds_an_entry() {
        // Two identical strokes are two entries, even though the state value
        // may compare equal.
        let mut history = HistoryViewModel::new(state(0, 0));
        history.record("Padrão", state(1, 0));
        history.record("Padrão", state(2, 0));
        assert_eq!(history.entries().get().len(), 2);
    }
}
