//! Everything that changes state, said in one vocabulary.
//!
//! A View may read ViewModel state and emit commands. It has no other way to
//! affect anything, which is what makes the interface a pure function of state
//! and what puts every mutation in one place a debugger can watch.
//!
//! Commands are values, not closures: a menu item, a keyboard shortcut and a
//! panel button that mean the same thing emit the *same* command, so they
//! cannot drift apart.

use clayspace_model::{ExtrudeSettings, Falloff, LayerKey, MaskOp, ToolKind, ViewPresetKind};

/// A change to the application or the document.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // -- tools ------------------------------------------------------------
    SelectTool(ToolKind),
    /// An operation on the mask itself, not through it.
    ApplyMaskOp(MaskOp),
    /// Pulls the masked patch off as its own layer.
    ExtrudeMask(ExtrudeSettings),
    SetBrushSize(f32),
    SetBrushIntensity(f32),
    SetBrushFlow(f32),
    SetBrushNoise(f32),
    SetBrushFalloff(Falloff),
    SetBrushAccumulate(bool),
    SetBrushSmoothing(f32),
    ToggleSymmetry(Axis),

    // -- scene and layers -------------------------------------------------
    SelectLayer(LayerKey),
    SetLayerVisible(LayerKey, bool),
    AddLayer,
    RemoveLayer(LayerKey),

    // -- armatures --------------------------------------------------------
    /// Starts a rig on the active layer, replacing whatever it had.
    NewArmature,
    /// Turns rigging on and off. The pointer means different things in each,
    /// so this is a mode — the one mode in the application, and the reason it
    /// is stated in the menu rather than inferred.
    ToggleArmatureEditing,
    /// Removes the selected sphere and everything hanging off it.
    RemoveZsphere,
    /// Whether a new sphere is mirrored as it is added.
    SetArmatureMirror(bool),
    /// The skin thickness, as a multiplier on the authored radii.
    SetSkinThickness(f32),

    // -- the sculpting gesture -------------------------------------------
    /// A stroke began at a point on the surface.
    BeginStroke {
        position: [f32; 3],
        pressure: f32,
    },
    /// The stroke continued. Samples accumulate until it ends.
    ContinueStroke {
        position: [f32; 3],
        pressure: f32,
    },
    /// The stroke ended and should be committed as one undoable edit.
    EndStroke,
    /// The stroke was abandoned before it committed.
    CancelStroke,

    // -- history ----------------------------------------------------------
    Undo,
    Redo,

    // -- view -------------------------------------------------------------
    /// View changes never enter the undo history.
    SetViewPreset(ViewPresetKind),
    FrameAll,
    NextMaterial,
    ToggleGrid,
    /// Shows or hides the diagnostics report.
    ToggleDiagnostics,
    /// Puts the report on the clipboard, which is the whole point of having
    /// one: a person pastes it into an issue rather than transcribing it.
    CopyDiagnostics,
}

impl Command {
    /// Whether this command can change the document.
    ///
    /// Camera movement, view presets, material and grid are display state; the
    /// specification says they create no history entry, and this is where that
    /// is decided rather than at each call site.
    pub fn touches_document(&self) -> bool {
        !matches!(
            self,
            Self::SetViewPreset(_)
                | Self::FrameAll
                | Self::NextMaterial
                | Self::ToggleGrid
                | Self::ToggleDiagnostics
                | Self::CopyDiagnostics
                | Self::SelectTool(_)
                | Self::SetBrushSize(_)
                | Self::SetBrushIntensity(_)
                | Self::SetBrushFlow(_)
                | Self::SetBrushNoise(_)
                | Self::SetBrushFalloff(_)
                | Self::SetBrushAccumulate(_)
                | Self::SetBrushSmoothing(_)
                | Self::ToggleSymmetry(_)
                // Choosing which layer to work on changes nothing in the
                // document; changing that layer does. Entering rigging is the
                // same: it changes what the pointer means, not the surface.
                | Self::SelectLayer(_)
                | Self::ToggleArmatureEditing
                | Self::SetArmatureMirror(_)
        )
    }

    /// A short name for the history panel and for diagnostics.
    pub fn label(&self) -> &'static str {
        match self {
            Self::SelectTool(_) => "select tool",
            Self::ApplyMaskOp(op) => op.label(),
            Self::ExtrudeMask(_) => "extrude mask",
            Self::SetBrushSize(_) => "brush size",
            Self::SetBrushIntensity(_) => "brush intensity",
            Self::SetBrushFlow(_) => "brush flow",
            Self::SetBrushNoise(_) => "brush noise",
            Self::SetBrushFalloff(_) => "brush edge",
            Self::SetBrushAccumulate(_) => "brush accumulation",
            Self::SetBrushSmoothing(_) => "brush smoothing",
            Self::SelectLayer(_) => "select layer",
            Self::SetLayerVisible(..) => "layer visibility",
            Self::AddLayer => "new layer",
            Self::RemoveLayer(_) => "remove layer",
            Self::ToggleSymmetry(_) => "symmetry",
            Self::NewArmature => "new armature",
            Self::ToggleArmatureEditing => "edit armature",
            Self::RemoveZsphere => "remove zsphere",
            Self::SetArmatureMirror(_) => "armature mirror",
            Self::SetSkinThickness(_) => "skin thickness",
            Self::BeginStroke { .. } => "begin stroke",
            Self::ContinueStroke { .. } => "continue stroke",
            Self::EndStroke => "stroke",
            Self::CancelStroke => "cancel stroke",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::SetViewPreset(_) => "view preset",
            Self::FrameAll => "frame all",
            Self::NextMaterial => "material",
            Self::ToggleGrid => "grid",
            Self::ToggleDiagnostics => "diagnostics",
            Self::CopyDiagnostics => "copy diagnostics",
        }
    }
}

/// A symmetry axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Self::X, Self::Y, Self::Z];

    pub fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }
}

/// Collects commands emitted while the interface is being built.
///
/// A View function takes one of these and pushes to it. It cannot reach the
/// Model, so this is the only channel out.
#[derive(Debug, Default)]
pub struct CommandQueue {
    commands: Vec<Command>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, command: Command) {
        self.commands.push(command);
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Takes everything queued, leaving the queue empty.
    pub fn drain(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_commands_do_not_touch_the_document() {
        for command in [
            Command::SetViewPreset(ViewPresetKind::Front),
            Command::FrameAll,
            Command::NextMaterial,
            Command::ToggleGrid,
        ] {
            assert!(
                !command.touches_document(),
                "{} must not be able to enter the undo history",
                command.label()
            );
        }
    }

    #[test]
    fn tool_settings_do_not_touch_the_document() {
        // Choosing a brush changes what the *next* edit will do; it is not
        // itself an edit.
        for command in [
            Command::SelectTool(ToolKind::Padrao),
            Command::SetBrushSize(20.0),
            Command::ToggleSymmetry(Axis::X),
        ] {
            assert!(
                !command.touches_document(),
                "{} is not an edit",
                command.label()
            );
        }
    }

    #[test]
    fn layer_changes_touch_the_document_but_selection_does_not() {
        assert!(
            !Command::SelectLayer(clayspace_model::LayerKey(1)).touches_document(),
            "choosing which layer to work on is not itself an edit"
        );
        for command in [
            Command::SetLayerVisible(clayspace_model::LayerKey(1), false),
            Command::AddLayer,
            Command::RemoveLayer(clayspace_model::LayerKey(1)),
        ] {
            assert!(command.touches_document(), "{} is an edit", command.label());
        }
    }

    #[test]
    fn strokes_and_history_touch_the_document() {
        for command in [
            Command::BeginStroke {
                position: [0.0; 3],
                pressure: 1.0,
            },
            Command::EndStroke,
            Command::Undo,
            Command::Redo,
        ] {
            assert!(command.touches_document(), "{} is an edit", command.label());
        }
    }

    #[test]
    fn the_queue_drains_once() {
        let mut queue = CommandQueue::new();
        queue.push(Command::Undo);
        queue.push(Command::Redo);
        assert_eq!(queue.len(), 2);

        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert!(queue.is_empty(), "draining must leave the queue empty");
        assert!(queue.drain().is_empty());
    }
}
