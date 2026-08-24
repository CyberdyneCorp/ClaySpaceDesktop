//! Everything that changes state, said in one vocabulary.
//!
//! A View may read ViewModel state and emit commands. It has no other way to
//! affect anything, which is what makes the interface a pure function of state
//! and what puts every mutation in one place a debugger can watch.
//!
//! Commands are values, not closures: a menu item, a keyboard shortcut and a
//! panel button that mean the same thing emit the *same* command, so they
//! cannot drift apart.

use std::path::PathBuf;

use clayspace_model::{
    ConversionSettings, ExportSettings, ExtrudeSettings, Falloff, ImportSettings, LayerKey, MaskOp,
    StrokeModifiers, ToolKind, ViewPresetKind,
};

/// A change to the application or the document.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // -- tools ------------------------------------------------------------
    SelectTool(ToolKind),
    /// Starts painting a mask, or stops and returns to the tool in hand.
    ///
    /// A toggle rather than a plain selection because that is what the key is
    /// for: freeze a region, then carry on with the brush you were using
    /// without having to find it on the shelf again.
    ToggleMaskPainting,
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
    /// Whether the active tool's brush is modulated by the loaded stamp.
    SetBrushAlpha(bool),
    /// Asks for a PNG and loads it as the alpha stamp.
    LoadAlpha,
    /// Drops the loaded stamp, so no brush is modulated.
    ClearAlpha,
    SetBrushSmoothing(f32),
    ToggleSymmetry(Axis),

    // -- scene and layers -------------------------------------------------
    SelectLayer(LayerKey),
    SetLayerVisible(LayerKey, bool),
    AddLayer,
    RemoveLayer(LayerKey),
    /// Starts renaming a layer, with its current name in the field.
    ///
    /// A mode rather than a dialog: a layer stack is renamed in place, and a
    /// modal for one word would stop the sculptor to ask for it.
    BeginRenameLayer(LayerKey),
    /// What the rename field holds now. Not an edit — nothing reaches the
    /// document until the name is committed.
    EditLayerName(String),
    /// Commits the field to the layer it was opened on.
    CommitRenameLayer,
    /// Abandons it, leaving the name as it was.
    CancelRenameLayer,

    // -- documents --------------------------------------------------------
    // Handled by the composition root rather than a ViewModel: each one may
    // need a file dialog, and a ViewModel that could open one would be a
    // ViewModel that needs a window to test.
    NewDocument,
    OpenDocument,
    /// Opens a document straight from the recent list.
    OpenRecent(PathBuf),
    Save,
    SaveAs,
    Quit,
    /// Shows or hides the import and export panels.
    ToggleImport,
    ToggleExport,
    /// Opens or closes the panel that crosses a layer to another
    /// representation.
    ToggleConvert,
    /// Opens or closes the pre-bake repair panel.
    ToggleRepair,
    /// Seals perforations in the active grid.
    CloseHoles,
    /// Fills every empty cell the outside cannot reach.
    FillVoids,
    /// How the next SDF edit combines with what is under it.
    SetCombine(clayspace_model::CombineSettings),
    /// Acts on a recorded pass of the active voxel layer.
    SculptLayer(clayspace_model::SculptLayerOp),
    /// Whether the deform panel is open.
    ToggleDeform,
    /// What that panel is set to.
    SetDeform(clayspace_model::DeformSettings),
    /// Applies it to the active layer, as one undo step.
    RunDeform,
    /// What the conversion panel is set to.
    SetConversion(ConversionSettings),
    /// Crosses the active layer, adding a new one.
    RunConversion,
    SetImportSettings(ImportSettings),
    SetExportSettings(ExportSettings),
    /// Asks for a file and brings it in with the settings as they stand.
    RunImport,
    /// Asks for a file and writes it with the settings as they stand.
    RunExport,

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
    /// Whether the viewport draws the skin or only the ZSpheres.
    ///
    /// ZBrush's Adaptive Skin preview, on `A`: while building a rig you want
    /// to see the spheres, and every so often you want to see what they make.
    ToggleSkinPreview,
    /// Makes the selected sphere cut into the rig rather than add to it.
    ToggleZsphereNegative,
    /// The skin thickness, as a multiplier on the authored radii.
    SetSkinThickness(f32),

    // -- the sculpting gesture -------------------------------------------
    /// A stroke began at a point on the surface.
    BeginStroke {
        position: [f32; 3],
        pressure: f32,
        /// What was held down when the press landed.
        ///
        /// On the press and not on every sample: a modifier caught and
        /// released mid-drag would change the verb under the sculptor's hand,
        /// and neither reference does that.
        modifiers: StrokeModifiers,
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
    /// Whether a mesh layer is drawn with its own edges over it.
    ///
    /// ZBrush's polyframe. It answers the one question a shaded surface hides:
    /// how much geometry is actually there.
    TogglePolyframe,
    /// Cycles what lengths are shown in. Presentation only: no geometry
    /// moves, which is what makes it safe to put on a single click.
    NextDisplayUnit,
    /// Shows or hides the attribution manifest.
    ToggleAttribution,
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
                | Self::TogglePolyframe
                | Self::NextDisplayUnit
                | Self::ToggleAttribution
                | Self::ToggleDiagnostics
                | Self::CopyDiagnostics
                // Document lifecycle is not an edit. Opening replaces the
                // document wholesale and saving changes nothing in it, so
                // neither belongs in the undo history or the modified mark.
                | Self::NewDocument
                | Self::OpenDocument
                | Self::OpenRecent(_)
                | Self::Save
                | Self::SaveAs
                | Self::Quit
                | Self::ToggleImport
                | Self::ToggleExport
                | Self::ToggleConvert
                | Self::ToggleRepair
                | Self::SetImportSettings(_)
                | Self::SetExportSettings(_)
                // Choosing what a crossing would do changes nothing; running
                // one adds a layer, and takes the composition root's own path
                // for the same reason import does.
                | Self::SetConversion(_)
                | Self::RunConversion
                // Opening the panel and setting it change nothing; running it
                // takes the composition root's own path, as the other layer
                // operations do.
                | Self::ToggleDeform
                | Self::SetDeform(_)
                | Self::RunDeform
                // A pass is not undo — dialling one is a property of the stack
                // rather than an entry in a history — so it takes the
                // composition root's own path like the other layer work.
                | Self::SculptLayer(_)
                // Choosing how the *next* edit combines changes nothing yet;
                // the stroke that follows is the entry.
                | Self::SetCombine(_)
                // Opening, typing into and abandoning the rename field change
                // nothing in the document. Committing does, and it takes the
                // composition root's own path for the reason import does: the
                // draft lives there, the ViewModel refreshes the panel itself,
                // and a *refused* commit must not mark the document modified
                // for a name it did not accept.
                | Self::BeginRenameLayer(_)
                | Self::EditLayerName(_)
                | Self::CancelRenameLayer
                | Self::CommitRenameLayer
                // Import *does* change the document, but it goes through the
                // composition root's own path — dialog, then model — and
                // marks the document itself. Routing it through the ordinary
                // edit path as well would double the entry.
                | Self::RunImport
                | Self::RunExport
                | Self::SelectTool(_)
                | Self::SetBrushSize(_)
                | Self::SetBrushIntensity(_)
                | Self::SetBrushFlow(_)
                | Self::SetBrushNoise(_)
                | Self::SetBrushFalloff(_)
                | Self::SetBrushAccumulate(_)
                | Self::SetBrushAlpha(_)
                // Loading a stamp changes no surface; the stroke that uses it
                // does. Loading takes the composition root's own path for the
                // reason import does — it opens a dialog.
                | Self::LoadAlpha
                | Self::ClearAlpha
                | Self::SetBrushSmoothing(_)
                | Self::ToggleSymmetry(_)
                // Choosing which layer to work on changes nothing in the
                // document; changing that layer does. Entering rigging is the
                // same: it changes what the pointer means, not the surface.
                | Self::SelectLayer(_)
                | Self::ToggleArmatureEditing
                | Self::SetArmatureMirror(_)
                | Self::ToggleSkinPreview
        )
    }

    /// A short name for the history panel and for diagnostics.
    pub fn label(&self) -> &'static str {
        match self {
            Self::SelectTool(_) => "select tool",
            Self::ToggleMaskPainting => "máscara",
            Self::ApplyMaskOp(op) => op.label(),
            Self::ExtrudeMask(_) => "extrude mask",
            Self::SetBrushSize(_) => "brush size",
            Self::SetBrushIntensity(_) => "brush intensity",
            Self::SetBrushFlow(_) => "brush flow",
            Self::SetBrushNoise(_) => "brush noise",
            Self::SetBrushFalloff(_) => "brush edge",
            Self::SetBrushAccumulate(_) => "brush accumulation",
            Self::SetBrushAlpha(_) => "brush stamp",
            Self::LoadAlpha => "load stamp",
            Self::ClearAlpha => "clear stamp",
            Self::SetBrushSmoothing(_) => "brush smoothing",
            Self::SelectLayer(_) => "select layer",
            Self::SetLayerVisible(..) => "layer visibility",
            Self::AddLayer => "new layer",
            Self::RemoveLayer(_) => "remove layer",
            Self::BeginRenameLayer(_) => "rename layer",
            Self::EditLayerName(_) => "layer name",
            Self::CommitRenameLayer => "rename layer",
            Self::CancelRenameLayer => "cancel rename",
            Self::ToggleSymmetry(_) => "symmetry",
            Self::NewArmature => "new armature",
            Self::ToggleArmatureEditing => "edit armature",
            Self::RemoveZsphere => "remove zsphere",
            Self::SetArmatureMirror(_) => "armature mirror",
            Self::ToggleSkinPreview => "skin preview",
            Self::ToggleZsphereNegative => "negative zsphere",
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
            Self::TogglePolyframe => "polyframe",
            Self::NewDocument => "new document",
            Self::OpenDocument => "open document",
            Self::OpenRecent(_) => "open recent",
            Self::Save => "save",
            Self::SaveAs => "save as",
            Self::Quit => "quit",
            Self::ToggleImport => "import panel",
            Self::ToggleExport => "export panel",
            Self::SetImportSettings(_) => "import settings",
            Self::SetExportSettings(_) => "export settings",
            Self::ToggleConvert => "convert panel",
            Self::ToggleRepair => "repair panel",
            Self::CloseHoles => "close holes",
            Self::FillVoids => "fill voids",
            Self::SetCombine(_) => "combine operation",
            Self::SculptLayer(op) => op.label(),
            Self::ToggleDeform => "deform panel",
            Self::SetDeform(_) => "deform settings",
            Self::RunDeform => "deform",
            Self::SetConversion(_) => "conversion settings",
            Self::RunConversion => "convert",
            Self::RunImport => "import",
            Self::RunExport => "export",
            Self::NextDisplayUnit => "display unit",
            Self::ToggleAttribution => "attribution",
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

    /// What is queued, without taking it.
    ///
    /// For tests that drive the interface with real input and then ask what it
    /// asked for — the only way to check that a menu entry is wired to the
    /// command it names.
    pub fn commands(&self) -> &[Command] {
        &self.commands
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
                modifiers: Default::default(),
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
