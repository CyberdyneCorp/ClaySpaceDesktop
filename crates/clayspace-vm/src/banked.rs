//! What an edit made outside the sculpting ViewModel cost the shared history.
//!
//! A sculptor has one Cmd+Z and does not care which part of the application
//! produced the thing they want back, so there is one history: the sculpting
//! ViewModel's stack of *how many engine entries each action spent*, where one
//! undo pops one count and spends exactly that many entries.
//!
//! Every other ViewModel that writes to the document therefore owes a count.
//! One that banked nothing did not merely lose its own undo — the next Cmd+Z
//! popped the **previous** command's count and spent it on entries belonging
//! to the edit that banked nothing, and the shortfall accumulated for as long
//! as the session mixed the two. Measured on a cage: add a subtool, insert a
//! shape into it, bend it through a lattice, undo once, and the subtool left
//! the document.
//!
//! The count is *measured* rather than assumed to be one. The unit a sculptor
//! thinks in and the number of entries underneath it are different questions —
//! inserting a shape as a subtool of its own is a layer and an item together,
//! a boolean is two bakes and a layer, and an operation the engine recorded
//! nothing for is nothing to take back at all.

/// The counts an edit left for the ViewModel that owns Cmd+Z.
///
/// A list rather than a running total: two edits banked as one would be one
/// undo where the sculptor made two.
#[derive(Debug, Default)]
pub struct Unbanked {
    counts: Vec<usize>,
}

impl Unbanked {
    /// Records what one edit cost, from the history depth either side of it.
    ///
    /// An edit that wrote nothing banks nothing, which is how a refusal, a
    /// cage dragged back to where it started and a command that met no target
    /// all cost the history nothing without any of them having to say so.
    pub fn record(&mut self, before: usize, after: usize) {
        let spent = after.saturating_sub(before);
        if spent > 0 {
            self.counts.push(spent);
        }
    }

    /// The counts, taken once.
    ///
    /// Taken rather than read: the ViewModel that owns Cmd+Z banks each count
    /// as one action, and a count banked twice is one undo too many.
    pub fn take(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_that_wrote_nothing_banks_nothing() {
        let mut unbanked = Unbanked::default();
        unbanked.record(3, 3);
        assert!(unbanked.take().is_empty());
    }

    #[test]
    fn two_edits_are_two_counts() {
        // Not one count of three. Banked together they would be one undo where
        // the sculptor made two things to take back.
        let mut unbanked = Unbanked::default();
        unbanked.record(0, 1);
        unbanked.record(1, 3);
        assert_eq!(unbanked.take(), vec![1, 2]);
    }

    #[test]
    fn the_counts_are_taken_once() {
        let mut unbanked = Unbanked::default();
        unbanked.record(0, 1);
        assert_eq!(unbanked.take(), vec![1]);
        assert!(unbanked.take().is_empty());
    }

    /// The engine evicts its oldest entries to stay inside a budget, so a
    /// depth can fall across an edit that did write. Nothing is owed then —
    /// what would be spent is no longer there to spend.
    #[test]
    fn a_history_that_shrank_banks_nothing() {
        let mut unbanked = Unbanked::default();
        unbanked.record(128, 127);
        assert!(unbanked.take().is_empty());
    }
}
