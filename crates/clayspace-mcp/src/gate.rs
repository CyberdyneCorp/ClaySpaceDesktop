//! What an agent may not do on the strength of the connection secret alone.
//!
//! The table is written down rather than derived. `Command::touches_document`
//! answers a different question — whether an edit becomes a history entry —
//! and a gate inferred from it would hold back every stroke while letting an
//! export through. A gate that drifts from its intent because it was inferred
//! is worse than one written out, because nobody can see that it drifted.
//!
//! The rule the table follows: **what the edit history can bring back is not
//! gated.** Sculpting, masking, transforming, selecting and undoing are what
//! the session is for and are all recoverable. What is gated is what leaves
//! the document — a file written over, an export, a document opened over an
//! unsaved one, the application closing.

use clayspace_vm::Command;

use crate::session::GateKind;

/// Which gate a command is held behind, where it is held behind one.
///
/// Exhaustive over the gated cases and permissive by default: a command that
/// is not named here is one the history covers.
pub fn gate_of(command: &Command) -> Option<GateKind> {
    match command {
        // Writes over whatever is at the document's own path.
        Command::Save => Some(GateKind::Overwrite),
        // Reads a document over the one that is open.
        Command::OpenRecent(_) => Some(GateKind::Open),
        // Reads a mesh in, over or beside what is open.
        Command::RunImport => Some(GateKind::Open),
        // Writes a mesh out of the document.
        Command::RunExport => Some(GateKind::Export),
        // Loses whatever is unsaved.
        Command::NewDocument => Some(GateKind::DiscardUnsaved),
        Command::Quit => Some(GateKind::Quit),

        // `GateKind::IrreversibleRemoval` has no command today, and that is a
        // statement rather than an omission: every removal this application
        // offers — a layer, a subtool, an object, a control point — is one
        // history entry and comes back with one undo. The kind stays so that a
        // removal which does *not* can be gated without inventing a mechanism
        // for it.
        _ => None,
    }
}

/// What would lift the gate, in words an agent can repeat to a person.
pub fn what_would_lift(gate: GateKind) -> &'static str {
    match gate {
        GateKind::Overwrite => {
            "the person at the window agreeing to it, or an opt-in for \
             \"sobrescrever\" recorded in the session store"
        }
        GateKind::Export => {
            "the person at the window agreeing to it, or an opt-in for \
             \"exportar\" recorded in the session store"
        }
        GateKind::Open => {
            "the person at the window agreeing to it, or an opt-in for \
             \"abrir\" recorded in the session store"
        }
        GateKind::DiscardUnsaved => {
            "the person at the window agreeing to it, or an opt-in for \
             \"descartar\" recorded in the session store"
        }
        GateKind::IrreversibleRemoval => {
            "the person at the window agreeing to it, or an opt-in for \
             \"remover\" recorded in the session store"
        }
        GateKind::Quit => {
            "the person at the window agreeing to it, or an opt-in for \
             \"sair\" recorded in the session store"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clayspace_model::{LayerKey, Representation, ToolKind};

    #[test]
    fn what_leaves_the_document_is_gated() {
        assert_eq!(gate_of(&Command::Save), Some(GateKind::Overwrite));
        assert_eq!(gate_of(&Command::RunExport), Some(GateKind::Export));
        assert_eq!(
            gate_of(&Command::OpenRecent("/tmp/a.clayspace".into())),
            Some(GateKind::Open)
        );
        assert_eq!(gate_of(&Command::RunImport), Some(GateKind::Open));
        assert_eq!(
            gate_of(&Command::NewDocument),
            Some(GateKind::DiscardUnsaved)
        );
        assert_eq!(gate_of(&Command::Quit), Some(GateKind::Quit));
    }

    #[test]
    fn what_the_history_covers_is_not_gated() {
        // Every one of these changes the document, and every one of them is
        // one undo away from not having happened.
        for command in [
            Command::EndStroke,
            Command::Undo,
            Command::Redo,
            Command::SelectTool(ToolKind::Argila),
            Command::RemoveLayer(LayerKey(2)),
            Command::RemoveObject,
            Command::AddLayer(Representation::Sdf),
            Command::RunBoolean,
            Command::RunConversion,
            Command::ApplyLattice,
        ] {
            assert_eq!(gate_of(&command), None, "{command:?}");
        }
    }

    #[test]
    fn every_gate_says_what_would_lift_it() {
        for gate in [
            GateKind::Overwrite,
            GateKind::Export,
            GateKind::Open,
            GateKind::DiscardUnsaved,
            GateKind::IrreversibleRemoval,
            GateKind::Quit,
        ] {
            let words = what_would_lift(gate);
            assert!(words.contains("person"), "{words}");
            assert!(words.contains(gate.tag()), "{words}");
        }
    }
}
