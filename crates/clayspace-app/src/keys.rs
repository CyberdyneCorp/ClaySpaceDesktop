//! What a key event is, in the shortcut table's terms.
//!
//! This lives in the library rather than the binary for the reason
//! [`crate::input`] does: the bug it exists to prevent was in the binary,
//! where nothing could reach it. That bug was ⌘Z and Ctrl+Z doing *nothing*.
//! The event loop matched `KeyCode` arms of its own and, with the command
//! modifier held, handled Save, Open and New and then returned — so every
//! other key with the modifier down was swallowed. Undo was on a bare `Z` and
//! redo on a bare `R`, which is not the binding on any platform, while
//! `Shortcuts` sat beside it holding the right ones and being read by nobody.
//!
//! So the mapping is one function with a test rather than a match arm inside
//! an event loop, and the table is the only place a binding is written down.

use clayspace_view::{Chord, Key};
use winit::keyboard::KeyCode;

/// The chord a pressed key makes with the modifiers held.
///
/// `None` for a key the table has no name for, which is most of the keyboard.
///
/// The platform difference is entirely inside `modifiers.command`: egui sets
/// it for ⌘ on macOS and for Ctrl on Windows and Linux, so nothing here — and
/// nothing in the table — needs to know which machine it is running on.
pub fn chord_for(code: KeyCode, modifiers: egui::Modifiers) -> Option<Chord> {
    Some(Chord {
        key: key_for(code)?,
        command: modifiers.command,
        shift: modifiers.shift,
        alt: modifiers.alt,
    })
}

/// The table's name for a physical key.
fn key_for(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Digit1 => Key::Digit1,
        KeyCode::Digit2 => Key::Digit2,
        KeyCode::Digit3 => Key::Digit3,
        KeyCode::Digit4 => Key::Digit4,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        KeyCode::BracketLeft => Key::BracketLeft,
        KeyCode::BracketRight => Key::BracketRight,
        KeyCode::Delete => Key::Delete,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Escape => Key::Escape,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clayspace_view::{Action, Shortcuts};

    fn modifiers(command: bool, shift: bool) -> egui::Modifiers {
        egui::Modifiers {
            command,
            shift,
            ..Default::default()
        }
    }

    /// The regression. On macOS this is ⌘Z and everywhere else Ctrl+Z, because
    /// that is what egui's `command` flag means — one assertion covers all
    /// three platforms, which is the point of routing through it.
    #[test]
    fn the_primary_modifier_and_z_is_undo() {
        let shortcuts = Shortcuts::default();
        let chord = chord_for(KeyCode::KeyZ, modifiers(true, false)).expect("a chord");
        assert_eq!(shortcuts.action(chord), Some(Action::Undo));
    }

    #[test]
    fn the_primary_modifier_with_shift_and_z_is_redo() {
        let shortcuts = Shortcuts::default();
        let chord = chord_for(KeyCode::KeyZ, modifiers(true, true)).expect("a chord");
        assert_eq!(shortcuts.action(chord), Some(Action::Redo));
    }

    /// A bare `Z` used to undo, which meant Shift+Z did too and neither was
    /// what anyone would press. It is unbound now rather than an alias: two
    /// ways to undo is two things to explain and one of them is wrong.
    #[test]
    fn a_bare_z_does_nothing() {
        let shortcuts = Shortcuts::default();
        let chord = chord_for(KeyCode::KeyZ, modifiers(false, false)).expect("a chord");
        assert_eq!(shortcuts.action(chord), None);
    }

    /// The file shortcuts kept the modifier they always had, so this is a
    /// check that routing them through the table did not move them.
    #[test]
    fn the_file_shortcuts_keep_their_modifier() {
        let shortcuts = Shortcuts::default();
        for (code, shift, action) in [
            (KeyCode::KeyN, false, Action::NewDocument),
            (KeyCode::KeyO, false, Action::OpenDocument),
            (KeyCode::KeyS, false, Action::Save),
            (KeyCode::KeyS, true, Action::SaveAs),
        ] {
            let chord = chord_for(code, modifiers(true, shift)).expect("a chord");
            assert_eq!(
                shortcuts.action(chord),
                Some(action),
                "{code:?} shift={shift}"
            );
        }
    }

    /// `S` is Save with the modifier and Z-symmetry without it. The two differ
    /// only by that flag, so this holds them apart.
    #[test]
    fn a_bare_s_is_symmetry_rather_than_save() {
        let shortcuts = Shortcuts::default();
        let chord = chord_for(KeyCode::KeyS, modifiers(false, false)).expect("a chord");
        assert_eq!(shortcuts.action(chord), Some(Action::SymmetryZ));
    }

    /// Every chord the table binds has to be reachable from a real key, or the
    /// binding is a line of documentation rather than a shortcut. This is what
    /// fails when an action is added to the table and not to `key_for`.
    #[test]
    fn every_bound_action_is_reachable_from_a_key() {
        let shortcuts = Shortcuts::default();
        let reachable: Vec<Action> = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::KeyA,
            KeyCode::KeyF,
            KeyCode::KeyM,
            KeyCode::KeyN,
            KeyCode::KeyO,
            KeyCode::KeyS,
            KeyCode::KeyX,
            KeyCode::KeyY,
            KeyCode::KeyZ,
            KeyCode::BracketLeft,
            KeyCode::BracketRight,
            KeyCode::Delete,
            KeyCode::Backspace,
            KeyCode::Escape,
        ]
        .into_iter()
        .flat_map(|code| {
            [(false, false), (true, false), (false, true), (true, true)]
                .into_iter()
                .map(move |(command, shift)| (code, command, shift))
        })
        .filter_map(|(code, command, shift)| {
            chord_for(code, modifiers(command, shift)).and_then(|chord| shortcuts.action(chord))
        })
        .collect();

        for action in Action::ALL {
            assert!(
                reachable.contains(&action),
                "{action:?} is in the table but no key produces its chord"
            );
        }
    }
}
