//! State the interface can watch without polling.
//!
//! An immediate-mode interface redraws when something changed. Deciding
//! *whether* anything changed by comparing state is expensive and fragile, so
//! state that matters carries a revision instead: readers remember the last
//! one they drew and redraw when it moves.
//!
//! Reading never marks anything dirty. That is the property that stops an
//! idle application redrawing forever.

use std::cell::Cell;

/// A value whose changes can be observed.
#[derive(Debug, Default)]
pub struct Observable<T> {
    value: T,
    revision: Cell<u64>,
    occurrences: Cell<u64>,
}

impl<T> Observable<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            revision: Cell::new(1),
            occurrences: Cell::new(0),
        }
    }

    /// Reads without marking anything changed.
    pub fn get(&self) -> &T {
        &self.value
    }

    /// The current revision. Compare against a remembered one to decide
    /// whether a redraw is needed.
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// How many times a value has been written here, whether or not it moved.
    ///
    /// The revision answers "must this be redrawn?"; this answers "did it
    /// happen again?". They are the same question for most state and a
    /// different one for a channel carrying events — a refusal repeated word
    /// for word is still a second refusal, and a reader that can only see the
    /// revision reads the second one as nothing having happened.
    pub fn occurrences(&self) -> u64 {
        self.occurrences.get()
    }

    /// Replaces the value and bumps the revision.
    ///
    /// Unconditional: use [`Self::set_if_changed`] where the caller can cheaply
    /// tell that nothing moved.
    pub fn set(&mut self, value: T) {
        self.value = value;
        self.bump();
    }

    /// Mutates in place and bumps the revision.
    pub fn update(&mut self, edit: impl FnOnce(&mut T)) {
        edit(&mut self.value);
        self.bump();
    }

    fn bump(&self) {
        self.revision
            .set(self.revision.get().wrapping_add(1).max(1));
        self.record_occurrence();
    }

    fn record_occurrence(&self) {
        self.occurrences.set(self.occurrences.get().wrapping_add(1));
    }
}

impl<T: PartialEq> Observable<T> {
    /// Replaces the value only when it differs.
    ///
    /// Setting a control to the value it already holds — which an
    /// immediate-mode interface does constantly — must not schedule a redraw.
    pub fn set_if_changed(&mut self, value: T) -> bool {
        if self.value == value {
            return false;
        }
        self.set(value);
        true
    }

    /// Records a value that may be the one already held, and counts it.
    ///
    /// For channels whose writes are *events* rather than state: a refusal, a
    /// substituted tool. Saying the same thing again is a second event and has
    /// to be visible as one, or a reader watching the channel either side of a
    /// command is told the command was never refused. It still does not
    /// redraw: the words on screen did not move, so neither does the revision,
    /// and only [`Self::occurrences`] counts the repeat.
    pub fn announce(&mut self, value: T) {
        if !self.set_if_changed(value) {
            self.record_occurrence();
        }
    }
}

/// Remembers a revision so a reader can tell when it has moved.
#[derive(Debug, Default, Clone, Copy)]
pub struct Watcher {
    seen: u64,
}

impl Watcher {
    pub fn new() -> Self {
        Self { seen: 0 }
    }

    /// Whether the observable has changed since this watcher last accepted it.
    pub fn is_stale<T>(&self, observable: &Observable<T>) -> bool {
        self.seen != observable.revision()
    }

    /// Marks the current revision as seen.
    pub fn accept<T>(&mut self, observable: &Observable<T>) {
        self.seen = observable.revision();
    }

    /// Combines the two: true exactly once per change.
    pub fn take_change<T>(&mut self, observable: &Observable<T>) -> bool {
        if self.is_stale(observable) {
            self.accept(observable);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_does_not_report_a_change() {
        let value = Observable::new(3);
        let mut watcher = Watcher::new();
        assert!(
            watcher.take_change(&value),
            "the first read is always a change"
        );

        for _ in 0..100 {
            let _ = value.get();
        }
        assert!(
            !watcher.take_change(&value),
            "reading marked the value dirty, which would redraw an idle application forever"
        );
    }

    #[test]
    fn a_change_is_reported_exactly_once() {
        let mut value = Observable::new(3);
        let mut watcher = Watcher::new();
        watcher.accept(&value);

        value.set(4);
        assert!(watcher.take_change(&value));
        assert!(
            !watcher.take_change(&value),
            "the same change was reported twice"
        );
    }

    #[test]
    fn setting_the_same_value_is_not_a_change() {
        let mut value = Observable::new(3);
        let mut watcher = Watcher::new();
        watcher.accept(&value);

        assert!(!value.set_if_changed(3));
        assert!(
            !watcher.take_change(&value),
            "writing the value it already held scheduled a redraw"
        );

        assert!(value.set_if_changed(4));
        assert!(watcher.take_change(&value));
    }

    #[test]
    fn an_announcement_that_repeats_is_still_a_second_occurrence() {
        let mut value = Observable::new(None);
        value.announce(Some("essa camada é uma grade"));
        let once = value.occurrences();

        value.announce(Some("essa camada é uma grade"));
        assert_ne!(
            value.occurrences(),
            once,
            "the same refusal twice counted once, so the second one is reported as success"
        );
    }

    #[test]
    fn an_announcement_that_repeats_does_not_schedule_a_redraw() {
        let mut value = Observable::new(None);
        value.announce(Some("essa camada é uma grade"));
        let mut watcher = Watcher::new();
        watcher.accept(&value);

        value.announce(Some("essa camada é uma grade"));
        assert!(
            !watcher.take_change(&value),
            "repeating the sentence already on screen scheduled a redraw of it"
        );
    }

    #[test]
    fn writing_the_value_already_held_is_not_an_occurrence() {
        // The counter belongs to channels that announce. A control being set
        // to what it already shows — which an immediate-mode interface does
        // constantly — is not an event, and counting it would make every
        // frame look like a refusal to a reader comparing the count.
        let mut value = Observable::new(3);
        let steady = value.occurrences();

        assert!(!value.set_if_changed(3));
        assert_eq!(value.occurrences(), steady);

        assert!(value.set_if_changed(4));
        assert_ne!(value.occurrences(), steady);
    }

    #[test]
    fn several_watchers_track_independently() {
        let mut value = Observable::new(0);
        let (mut a, mut b) = (Watcher::new(), Watcher::new());
        a.accept(&value);
        b.accept(&value);

        value.set(1);
        assert!(a.take_change(&value));
        assert!(
            b.take_change(&value),
            "one watcher consumed another's change"
        );
    }

    #[test]
    fn the_revision_never_returns_to_zero() {
        // Zero is the never-seen sentinel, so wrapping past it would make a
        // fresh watcher believe it was up to date.
        let mut value = Observable::new(0u8);
        value.revision.set(u64::MAX);
        value.set(1);
        assert_ne!(value.revision(), 0);
    }
}
