//! What went wrong, said where it happened.
//!
//! The specification asks for failures reported near the action that caused
//! them, in the user's terms, with the engine's own result codes kept for the
//! diagnostics view rather than shown as the message. This is where a failure
//! becomes something a person can act on.

use crate::observable::Observable;

/// Where a notice belongs on screen.
///
/// A failure shown in a corner while the user is looking at a brush panel is
/// a failure they will not see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Where {
    /// The tool options bar — a refused stroke, an unavailable tool.
    Tool,
    /// The layer panel — a refused layer operation.
    Layers,
    /// The status area — memory, backend, background work.
    Status,
    /// A modal concern: nothing else can proceed until it is answered.
    Blocking,
}

/// How much attention a notice deserves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Something worth saying that is not a problem.
    Info,
    /// The operation did not happen, and the user should know why.
    Refusal,
    /// Something is wrong that will keep being wrong.
    Problem,
}

/// One thing to tell the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub severity: Severity,
    pub place: Where,
    /// What happened, in the user's terms. No result codes, no identifiers.
    pub message: String,
    /// The engine's own words, for the diagnostics view.
    pub detail: Option<String>,
    /// What the user can do about it, where there is something.
    pub remedy: Option<String>,
}

impl Notice {
    pub fn refusal(place: Where, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Refusal,
            place,
            message: message.into(),
            detail: None,
            remedy: None,
        }
    }

    pub fn problem(place: Where, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Problem,
            place,
            message: message.into(),
            detail: None,
            remedy: None,
        }
    }

    /// Attaches the engine's own description, which the diagnostics view shows
    /// and the notice does not.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_remedy(mut self, remedy: impl Into<String>) -> Self {
        self.remedy = Some(remedy.into());
        self
    }
}

/// What memory the document is using against what it may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryState {
    pub used: u64,
    pub budget: u64,
}

impl MemoryState {
    /// Above this, the meter changes state — before the budget is exhausted
    /// rather than at failure.
    pub const WARNING_FRACTION: f64 = 0.85;

    pub fn fraction(self) -> f64 {
        if self.budget == 0 {
            return 0.0;
        }
        self.used as f64 / self.budget as f64
    }

    pub fn is_near_budget(self) -> bool {
        self.fraction() >= Self::WARNING_FRACTION
    }
}

/// The notices currently worth showing, and the memory meter.
#[derive(Default)]
pub struct NoticeBoard {
    notices: Observable<Vec<Notice>>,
    memory: Observable<MemoryState>,
}

impl NoticeBoard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notices(&self) -> &Observable<Vec<Notice>> {
        &self.notices
    }

    pub fn memory(&self) -> &Observable<MemoryState> {
        &self.memory
    }

    /// The notice for a place, if any. One per place: a stack of refusals in
    /// the same spot is noise, and the newest is the one that matters.
    pub fn at(&self, place: Where) -> Option<&Notice> {
        self.notices.get().iter().find(|n| n.place == place)
    }

    /// Posts a notice, replacing any other in the same place.
    pub fn post(&mut self, notice: Notice) {
        self.notices.update(|notices| {
            notices.retain(|existing| existing.place != notice.place);
            notices.push(notice);
        });
    }

    /// Clears the notice at a place, once whatever it described has passed.
    pub fn clear(&mut self, place: Where) {
        let present = self.notices.get().iter().any(|n| n.place == place);
        if present {
            self.notices.update(|notices| notices.retain(|n| n.place != place));
        }
    }

    pub fn clear_all(&mut self) {
        if !self.notices.get().is_empty() {
            self.notices.set(Vec::new());
        }
    }

    /// Records memory usage and raises a notice as the budget approaches.
    pub fn set_memory(&mut self, memory: MemoryState) {
        let was_near = self.memory.get().is_near_budget();
        self.memory.set_if_changed(memory);

        if memory.is_near_budget() && !was_near {
            self.post(
                Notice::problem(Where::Status, "a memória está quase no limite")
                    .with_remedy("aumente o limite ou reduza a resolução"),
            );
        } else if !memory.is_near_budget() && was_near {
            self.clear(Where::Status);
        }
    }

    /// Reports that an operation was refused for want of memory.
    ///
    /// The document is untouched — the engine refuses rather than half-applies
    /// — so this states the shortfall and what can be done, not a failure.
    pub fn budget_exceeded(&mut self, needed: u64, budget: u64) {
        let shortfall = needed.saturating_sub(budget);
        self.post(
            Notice::problem(
                Where::Blocking,
                format!(
                    "a operação precisa de {} a mais do que o limite permite",
                    megabytes(shortfall)
                ),
            )
            .with_remedy("aumente o limite de memória ou reduza a resolução")
            .with_detail(format!(
                "needed {} against a budget of {}",
                megabytes(needed),
                megabytes(budget)
            )),
        );
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1024.0 / 1024.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notice_appears_where_the_action_was() {
        let mut board = NoticeBoard::new();
        board.post(Notice::refusal(Where::Tool, "esta camada está bloqueada"));

        assert!(board.at(Where::Tool).is_some());
        assert!(
            board.at(Where::Layers).is_none(),
            "a notice appeared somewhere the user is not looking"
        );
    }

    #[test]
    fn one_notice_per_place() {
        let mut board = NoticeBoard::new();
        board.post(Notice::refusal(Where::Tool, "first"));
        board.post(Notice::refusal(Where::Tool, "second"));

        assert_eq!(board.notices().get().len(), 1, "refusals stacked up");
        assert_eq!(board.at(Where::Tool).map(|n| n.message.as_str()), Some("second"));
    }

    #[test]
    fn engine_detail_is_carried_but_is_not_the_message() {
        let notice = Notice::problem(Where::Blocking, "o arquivo não pôde ser escrito")
            .with_detail("clay_document_save: I/O error (permission denied)");

        assert!(
            !notice.message.contains("clay_"),
            "an engine identifier reached the user's message"
        );
        assert!(
            notice.detail.is_some_and(|d| d.contains("clay_")),
            "the engine's own words must still be available for diagnostics"
        );
    }

    #[test]
    fn the_meter_warns_before_the_budget_is_exhausted() {
        let mut board = NoticeBoard::new();
        board.set_memory(MemoryState {
            used: 500,
            budget: 1000,
        });
        assert!(board.at(Where::Status).is_none(), "half full is not a warning");

        board.set_memory(MemoryState {
            used: 900,
            budget: 1000,
        });
        assert!(
            board.at(Where::Status).is_some(),
            "the meter must change state before the budget is exhausted, not at failure"
        );
    }

    #[test]
    fn the_warning_clears_when_memory_is_released() {
        let mut board = NoticeBoard::new();
        board.set_memory(MemoryState {
            used: 950,
            budget: 1000,
        });
        assert!(board.at(Where::Status).is_some());

        board.set_memory(MemoryState {
            used: 200,
            budget: 1000,
        });
        assert!(
            board.at(Where::Status).is_none(),
            "the warning outlived the condition it described"
        );
    }

    #[test]
    fn a_budget_refusal_states_the_shortfall_and_a_remedy() {
        let mut board = NoticeBoard::new();
        board.budget_exceeded(3 * 1024 * 1024 * 1024, 2 * 1024 * 1024 * 1024);

        let notice = board.at(Where::Blocking).expect("a refusal to show");
        assert!(notice.message.contains("MB"), "{}", notice.message);
        assert!(
            notice.remedy.is_some(),
            "a refusal the user can do something about should say what"
        );
    }

    #[test]
    fn an_unlimited_budget_never_warns() {
        let memory = MemoryState {
            used: u64::MAX,
            budget: 0,
        };
        assert!(
            !memory.is_near_budget(),
            "a budget of zero means unlimited, not exhausted"
        );
    }

    #[test]
    fn clearing_a_place_that_has_no_notice_is_not_a_change() {
        let mut board = NoticeBoard::new();
        let mut watcher = crate::Watcher::new();
        watcher.accept(board.notices());

        board.clear(Where::Tool);
        assert!(
            !watcher.take_change(board.notices()),
            "clearing nothing scheduled a redraw"
        );
    }
}
