//! Keyboard shortcuts, and what happens when two want the same key.
//!
//! The specification asks for the sculpting loop to be reachable from the
//! keyboard and for the bindings to be remappable, with a conflict reported
//! rather than silently overriding. That last part is the whole reason this is
//! a table rather than a match arm in the event loop.

use std::collections::BTreeMap;

/// A key with its modifiers, as a value that can be compared and stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chord {
    pub key: Key,
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    pub fn plain(key: Key) -> Self {
        Self {
            key,
            command: false,
            shift: false,
            alt: false,
        }
    }

    /// With the platform's primary modifier — Command on macOS, Control
    /// elsewhere. One name for both, because a shortcut table that spells them
    /// separately drifts.
    pub fn primary(key: Key) -> Self {
        Self {
            key,
            command: true,
            shift: false,
            alt: false,
        }
    }

    pub fn primary_shift(key: Key) -> Self {
        Self {
            key,
            command: true,
            shift: true,
            alt: false,
        }
    }

    /// With Shift alone, for a bare-letter binding that has a variant.
    pub fn shift(key: Key) -> Self {
        Self {
            key,
            command: false,
            shift: true,
            alt: false,
        }
    }

    /// How the menu displays it.
    pub fn label(self) -> String {
        let mut text = String::new();
        if self.command {
            text.push_str(if cfg!(target_os = "macos") {
                "⌘"
            } else {
                "Ctrl+"
            });
        }
        if self.shift {
            text.push_str(if cfg!(target_os = "macos") {
                "⇧"
            } else {
                "Shift+"
            });
        }
        if self.alt {
            text.push_str(if cfg!(target_os = "macos") {
                "⌥"
            } else {
                "Alt+"
            });
        }
        text.push_str(self.key.label());
        text
    }
}

/// The keys the application binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    A,
    F,
    M,
    N,
    O,
    Z,
    X,
    Y,
    S,
    BracketLeft,
    BracketRight,
    Delete,
    Backspace,
    Escape,
}

impl Key {
    pub fn label(self) -> &'static str {
        match self {
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::A => "A",
            Self::F => "F",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::Z => "Z",
            Self::X => "X",
            Self::Y => "Y",
            Self::S => "S",
            Self::BracketLeft => "[",
            Self::BracketRight => "]",
            Self::Delete => "Del",
            Self::Backspace => "⌫",
            Self::Escape => "Esc",
        }
    }
}

/// What a shortcut does.
///
/// Named actions rather than commands, because a shortcut for "smaller brush"
/// has to read the current size to produce a command, and the table should not
/// need to know that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    NewDocument,
    OpenDocument,
    Save,
    SaveAs,
    Undo,
    Redo,
    FrameAll,
    NextMaterial,
    ViewPerspective,
    ViewFront,
    ViewSide,
    ViewTop,
    SymmetryX,
    SymmetryY,
    SymmetryZ,
    BrushSmaller,
    BrushLarger,
    ToggleSkinPreview,
    ToggleArmatureEditing,
    RemoveZsphere,
    Quit,
}

impl Action {
    pub const ALL: [Action; 21] = [
        Self::NewDocument,
        Self::OpenDocument,
        Self::Save,
        Self::SaveAs,
        Self::Undo,
        Self::Redo,
        Self::FrameAll,
        Self::NextMaterial,
        Self::ViewPerspective,
        Self::ViewFront,
        Self::ViewSide,
        Self::ViewTop,
        Self::SymmetryX,
        Self::SymmetryY,
        Self::SymmetryZ,
        Self::BrushSmaller,
        Self::BrushLarger,
        Self::ToggleSkinPreview,
        Self::ToggleArmatureEditing,
        Self::RemoveZsphere,
        Self::Quit,
    ];
}

/// A conflicting assignment, reported rather than applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conflict {
    pub chord: Chord,
    /// What already holds it.
    pub held_by: Action,
}

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} is already bound to {:?}",
            self.chord.label(),
            self.held_by
        )
    }
}

/// The bindings in force.
#[derive(Debug, Clone, PartialEq)]
pub struct Shortcuts {
    bindings: BTreeMap<Chord, Action>,
}

impl Default for Shortcuts {
    fn default() -> Self {
        let mut bindings = BTreeMap::new();
        let mut bind = |chord: Chord, action: Action| {
            bindings.insert(chord, action);
        };

        // The platform's primary modifier for the file and history actions,
        // which is what every application on every desktop uses for these —
        // `Chord::primary` is Command on macOS and Control elsewhere, so this
        // table is the same table on all three.
        bind(Chord::primary(Key::N), Action::NewDocument);
        bind(Chord::primary(Key::O), Action::OpenDocument);
        bind(Chord::primary(Key::S), Action::Save);
        bind(Chord::primary_shift(Key::S), Action::SaveAs);
        bind(Chord::primary(Key::Z), Action::Undo);
        bind(Chord::primary_shift(Key::Z), Action::Redo);

        // Bare letters for what a hand on the keyboard reaches for while the
        // other hand is sculpting.
        bind(Chord::plain(Key::F), Action::FrameAll);
        bind(Chord::plain(Key::M), Action::NextMaterial);
        bind(Chord::plain(Key::Digit1), Action::ViewPerspective);
        bind(Chord::plain(Key::Digit2), Action::ViewFront);
        bind(Chord::plain(Key::Digit3), Action::ViewSide);
        bind(Chord::plain(Key::Digit4), Action::ViewTop);
        bind(Chord::plain(Key::X), Action::SymmetryX);
        bind(Chord::plain(Key::Y), Action::SymmetryY);
        bind(Chord::plain(Key::S), Action::SymmetryZ);
        bind(Chord::plain(Key::BracketLeft), Action::BrushSmaller);
        bind(Chord::plain(Key::BracketRight), Action::BrushLarger);

        // `A` is Adaptive Skin preview in ZBrush, and anyone who has rigged
        // before will reach for it. Entering the mode takes the modifier
        // rather than the other way round: it is done once a session, and
        // previewing is done constantly.
        bind(Chord::plain(Key::A), Action::ToggleSkinPreview);
        bind(Chord::shift(Key::A), Action::ToggleArmatureEditing);
        // Two keys, one action: whichever the keyboard has where the thumb
        // expects it. `chord` reports the first for the menu.
        bind(Chord::plain(Key::Delete), Action::RemoveZsphere);
        bind(Chord::plain(Key::Backspace), Action::RemoveZsphere);

        bind(Chord::plain(Key::Escape), Action::Quit);

        Self { bindings }
    }
}

impl Shortcuts {
    /// What a chord does, if anything.
    pub fn action(&self, chord: Chord) -> Option<Action> {
        self.bindings.get(&chord).copied()
    }

    /// The chord bound to an action, for the menu to display.
    pub fn chord(&self, action: Action) -> Option<Chord> {
        self.bindings
            .iter()
            .find(|(_, bound)| **bound == action)
            .map(|(chord, _)| *chord)
    }

    /// Binds a chord, refusing one another action already holds.
    ///
    /// The refusal names the holder, because "that shortcut is taken" without
    /// saying by what leaves the user hunting.
    pub fn bind(&mut self, chord: Chord, action: Action) -> Result<(), Conflict> {
        if let Some(held_by) = self.bindings.get(&chord) {
            if *held_by != action {
                return Err(Conflict {
                    chord,
                    held_by: *held_by,
                });
            }
            return Ok(());
        }
        // An action holds one chord at a time, so its previous binding goes.
        self.bindings.retain(|_, bound| *bound != action);
        self.bindings.insert(chord, action);
        Ok(())
    }

    /// Binds a chord, displacing whatever held it.
    ///
    /// For when the user has seen the conflict and chosen anyway.
    pub fn rebind(&mut self, chord: Chord, action: Action) {
        self.bindings.remove(&chord);
        self.bindings.retain(|_, bound| *bound != action);
        self.bindings.insert(chord, action);
    }

    pub fn unbind(&mut self, action: Action) {
        self.bindings.retain(|_, bound| *bound != action);
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Every binding, for the preferences list.
    pub fn all(&self) -> impl Iterator<Item = (Chord, Action)> + '_ {
        self.bindings
            .iter()
            .map(|(chord, action)| (*chord, *action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sculpting_loop_is_reachable_from_the_keyboard() {
        // The specification names these as the operations used continuously
        // while sculpting; every one must have a binding out of the box.
        let shortcuts = Shortcuts::default();
        for action in Action::ALL {
            assert!(
                shortcuts.chord(action).is_some(),
                "{action:?} has no default binding"
            );
        }
    }

    #[test]
    fn no_two_actions_share_a_default_chord() {
        let shortcuts = Shortcuts::default();
        let mut seen = std::collections::BTreeSet::new();
        for (chord, _) in shortcuts.all() {
            assert!(seen.insert(chord), "{} is bound twice", chord.label());
        }
    }

    #[test]
    fn a_conflicting_assignment_is_reported_and_not_applied() {
        let mut shortcuts = Shortcuts::default();
        let undo = Chord::primary(Key::Z);

        let conflict = shortcuts
            .bind(undo, Action::FrameAll)
            .expect_err("that chord is taken");
        assert_eq!(conflict.held_by, Action::Undo);
        assert!(
            conflict.to_string().contains("Undo"),
            "the refusal must name what holds it: {conflict}"
        );
        assert_eq!(
            shortcuts.action(undo),
            Some(Action::Undo),
            "the refused assignment was applied anyway"
        );
    }

    #[test]
    fn rebinding_after_seeing_the_conflict_displaces_the_holder() {
        let mut shortcuts = Shortcuts::default();
        let undo = Chord::primary(Key::Z);
        shortcuts.rebind(undo, Action::FrameAll);

        assert_eq!(shortcuts.action(undo), Some(Action::FrameAll));
        assert_eq!(
            shortcuts.chord(Action::Undo),
            None,
            "the displaced action should be left unbound rather than pointing at a stolen chord"
        );
    }

    #[test]
    fn an_action_holds_one_chord_at_a_time() {
        let mut shortcuts = Shortcuts::default();
        let old = shortcuts.chord(Action::FrameAll).expect("bound");
        shortcuts
            .bind(Chord::plain(Key::Digit1), Action::ViewPerspective)
            .expect("already its own");

        shortcuts.rebind(Chord::primary(Key::S), Action::FrameAll);
        assert_eq!(
            shortcuts.action(old),
            None,
            "the previous binding survived, so the action now answers to two chords"
        );
    }

    #[test]
    fn rebinding_an_action_to_the_chord_it_already_holds_is_not_a_conflict() {
        let mut shortcuts = Shortcuts::default();
        let undo = Chord::primary(Key::Z);
        assert!(
            shortcuts.bind(undo, Action::Undo).is_ok(),
            "an action conflicting with itself is not a conflict"
        );
    }

    #[test]
    fn reset_restores_every_default() {
        let mut shortcuts = Shortcuts::default();
        shortcuts.rebind(Chord::plain(Key::Digit1), Action::Quit);
        shortcuts.unbind(Action::Undo);
        shortcuts.reset();
        assert_eq!(shortcuts, Shortcuts::default());
    }

    #[test]
    fn a_chord_reads_as_the_platform_spells_it() {
        let label = Chord::primary(Key::Z).label();
        if cfg!(target_os = "macos") {
            assert!(label.starts_with('⌘'), "{label}");
        } else {
            assert!(label.starts_with("Ctrl+"), "{label}");
        }
        assert!(label.ends_with('Z'));
    }
}
