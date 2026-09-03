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
    ConversionSettings, CurveJoin, CurveProfile, ExportSettings, ExtrudeSettings, Falloff,
    GizmoHandle, GizmoMode, ImportSettings, LayerKey, Locale, MaskGesture, MaskOp, OutlineFrame,
    RefPlane, ReferenceSettings, Representation, SmoothBlur, StrokeModifiers, SurfaceOpacity,
    ToolKind, ViewPresetKind, VoxelDisplay,
};

/// A change to the application or the document.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // -- tools ------------------------------------------------------------
    SelectTool(ToolKind),
    /// Starts a curve, or takes the one that is up down.
    ToggleCurve,
    /// Appends a control point at the end of the curve.
    AddCurvePoint([f32; 3], f32),
    /// The control point under the pointer, or none. Replaces the selection.
    SelectCurvePoint(Option<usize>),
    /// Adds or removes one control point without disturbing the rest.
    ToggleCurvePoint(usize),
    /// Moves every selected control point by a displacement.
    DragCurve([f32; 3]),
    /// How thick the tube is at the selected points, or at all of them.
    SetCurveRadius(f32),
    SetCurveJoin(CurveJoin),
    SetCurveProfile(CurveProfile),
    /// Removes the selected control points.
    RemoveCurvePoints,
    /// Leaves the swept form and takes the curve down.
    ApplyCurve,
    // -- placed objects ---------------------------------------------------
    /// Shows or hides the panel that offers the shapes.
    ToggleShapes,
    /// Which shape the picker is set to, and what it is measured by.
    SetShape(clayspace_model::Shape),
    /// The numbers for the shape the picker is set to.
    SetShapeParameters(Vec<f32>),
    /// Puts the picked form into the scene, selected, combining the way the
    /// options bar is set.
    ///
    /// Where it lands is [`Command::SetInsertAs`]'s business and not this
    /// command's: one verb for "put the form I picked into the scene", so a
    /// button and a shortcut that mean it cannot come to disagree about which
    /// of the two destinations they meant.
    InsertShape,
    /// Whether the next insertion makes a subtool of its own or an object in
    /// the active layer.
    ///
    /// The sculptor's choice rather than a guess from context: a form put into
    /// the scene to be worked on its own is a subtool, a form put into the
    /// layer being worked is a part of that form, and guessing between them
    /// would be wrong half the time.
    SetInsertAs(clayspace_model::InsertAs),
    /// Asks for a mesh file and brings it in as a subtool of its own.
    ///
    /// Handled by the composition root, as every command that needs a file
    /// dialog is: a ViewModel that could open one would be a ViewModel that
    /// needs a window to test.
    InsertMesh,
    /// Copies a subtool already in the document into one of its own.
    ///
    /// A copy and not an instance. The engine composes layers by hard union
    /// and has no instancing (ClayCore #364), so what this makes is the source
    /// sampled into a volume of its own — which is why sculpting the copy
    /// cannot reach the original, and why the word in the interface is
    /// "copiar".
    CopySubtool(LayerKey),
    /// Opens or closes the panel that resolves a boolean between two subtools.
    ToggleBoolean,
    /// What that panel is set to: the two operands, the operation and the
    /// resolution.
    ///
    /// Choosing changes nothing in the document — it states what the operation
    /// would cost. `RunBoolean` is the consent.
    SetBoolean(clayspace_model::BooleanSettings),
    /// Resolves the boolean the panel is set to, as one undo step.
    RunBoolean,
    /// Which mesh layer the picker would place as an operand, or none for one
    /// of the offered shapes.
    ///
    /// Choosing one states what the crossing costs; it does not run it.
    /// Nothing reaches the document until `InsertShape`.
    SetMeshOperand(Option<clayspace_model::LayerKey>),
    /// Selects a placed object, or clears the selection. The manipulator
    /// follows it.
    SelectObject(Option<clayspace_model::ObjectId>),
    /// Exchanges the selected object's shape, keeping where it stands and how
    /// it combines.
    SetObjectShape(clayspace_model::Shape, Vec<f32>),
    /// Changes how the selected object meets what is under it.
    SetObjectCombine(clayspace_model::CombineSettings),
    /// Takes the selected object away.
    RemoveObject,
    /// What the manipulator is acting on: an object, a whole layer, or a
    /// curve's points.
    SetGizmoTarget(Option<clayspace_model::GizmoTarget>),

    /// Puts a lattice cage around the active layer, or takes one down.
    ToggleLattice,
    /// How many control points the cage has per axis.
    SetLatticeDivisions([i32; 3]),
    /// The control point under the pointer, or none. Replaces the selection.
    SelectLatticePoint(Option<usize>),
    /// Adds or removes one control point without disturbing the rest.
    ToggleLatticePoint(usize),
    /// Every control point a selection box caught. Replaces the selection.
    SelectLatticePoints(Vec<usize>),
    /// Which of the manipulator's three modes is in force.
    SetGizmoMode(GizmoMode),
    /// Grabs a manipulator handle at a point on the drag plane, carrying the
    /// direction the camera faces — which is what the outer ring turns about.
    BeginGizmoDrag(GizmoHandle, [f32; 3], [f32; 3]),
    /// Carries the selection to where the pointer is now. The flag is whether
    /// a rotation snaps to whole increments.
    DragGizmo([f32; 3], bool),
    /// Lets the manipulator go.
    EndGizmoDrag,
    /// Moves the selected control point to a world position.
    DragLatticePoint([f32; 3]),
    /// Bends the layer through the cage and takes the cage down.
    ApplyLattice,
    /// Starts painting a mask, or stops and returns to the tool in hand.
    ///
    /// A toggle rather than a plain selection because that is what the key is
    /// for: freeze a region, then carry on with the brush you were using
    /// without having to find it on the shelf again.
    ToggleMaskPainting,
    /// An operation on the mask itself, not through it.
    ApplyMaskOp(MaskOp),
    /// Which gesture the mask brush makes: a drag across the surface, or an
    /// outline drawn over the form.
    ///
    /// A setting rather than a second tool, because it is one question about
    /// one brush — ZBrush keeps them in the stroke palette for the same
    /// reason — and because a second tool would need a second answer to every
    /// availability question the first one already answers.
    SetMaskGesture(MaskGesture),
    /// Starts an outline at a point of the viewport, in normalised device
    /// coordinates. The flag is whether the modifier that releases rather than
    /// freezes was held when the gesture began.
    ///
    /// Latched at the press, as a stroke's modifiers are: a key caught
    /// mid-drag would change what the outline means under the sculptor's hand.
    BeginMaskOutline([f32; 2], bool),
    /// Carries the outline to where the pointer is now.
    ExtendMaskOutline([f32; 2]),
    /// Closes the outline and applies it, on the frame it was drawn over.
    ///
    /// The frame arrives with the command rather than being read from a camera
    /// the ViewModel cannot see: the outline is normalised device coordinates
    /// until this moment, and this is what carries it into the world.
    EndMaskOutline(OutlineFrame),
    /// Abandons the outline, leaving the mask as it was.
    CancelMaskOutline,
    /// How far Expandir, Contrair and Suavizar máscara reach.
    SetMaskSteps(i32),
    /// What an extrusion would use, as the panel has it set.
    SetExtrudeSettings(ExtrudeSettings),
    /// Pulls the masked patch off as its own layer.
    ExtrudeMask(ExtrudeSettings),
    SetBrushSize(f32),
    SetBrushIntensity(f32),
    SetBrushFlow(f32),
    SetBrushNoise(f32),
    /// How far each stamp is turned about its own facing, in radians.
    ///
    /// The grain. Radians rather than degrees because that is what the engine
    /// takes and what the domain carries; the dial is the View's own reading
    /// of it.
    SetBrushAzimuth(f32),
    SetBrushFalloff(Falloff),
    SetBrushAccumulate(bool),
    /// Whether the active tool's brush is modulated by the loaded stamp.
    SetBrushAlpha(bool),
    /// What the colour brushes paint with.
    ///
    /// One value for the session rather than one per tool: it is what the
    /// sculptor is painting with now, not a property of a brush. See
    /// `clayspace_model::colour`.
    SetBrushColour(clayspace_model::Colour),
    /// Picks the nth colour off the recent list back into the swatch.
    PickRecentColour(usize),
    /// Asks for a PNG and loads it as the alpha stamp.
    LoadAlpha,
    /// Drops the loaded stamp, so no brush is modulated.
    ClearAlpha,

    // -- reference images -------------------------------------------------
    /// Opens or closes the reference panel.
    ToggleReferences,
    /// Asks for a PNG and places it on one plane, behind the sculpt.
    LoadReference(RefPlane),
    /// Takes one plane's reference away.
    ClearReference(RefPlane),
    /// How one plane's reference sits: shown, how large, where, how far back.
    SetReferenceSettings(RefPlane, ReferenceSettings),
    /// How opaque the sculpted surface is drawn, so a reference behind it can
    /// be seen through the clay.
    SetSurfaceOpacity(SurfaceOpacity),
    SetBrushSmoothing(f32),
    ToggleSymmetry(Axis),

    // -- scene and layers -------------------------------------------------
    SelectLayer(LayerKey),
    SetLayerVisible(LayerKey, bool),
    /// Shows one subtool alone, or releases the solo with `None`.
    ///
    /// The state to be in rather than a toggle, so the stack row and any other
    /// route to it cannot disagree about whether a solo is engaged.
    SoloLayer(Option<LayerKey>),
    /// Adds an empty layer carrying the chosen representation.
    ///
    /// Stated at creation rather than reached by a conversion afterwards: a
    /// grid asked for after the fact costs a crossing, and the crossing is the
    /// thing the sculptor was trying to avoid. SDF where nothing is chosen,
    /// which is what a layer has always been.
    AddLayer(Representation),
    RemoveLayer(LayerKey),
    /// Collapses a field layer's edit list into one volume.
    ///
    /// Always the sculptor's decision and never taken quietly: it costs
    /// seconds on a worked layer and it changes what the layer holds. The
    /// engine says when it is worth doing — see [`clayspace_model::FieldHealth`]
    /// — and the interface offers it; nothing here acts on that advice by
    /// itself.
    OptimizeLayer(LayerKey),
    /// Rebuilds a mesh layer's topology through a voxel field — DynaMesh.
    ///
    /// The mesh counterpart to [`Command::OptimizeLayer`] and offered on the
    /// same terms: it is what a sculptor reaches for when a form has been
    /// pulled somewhere its triangles cannot follow, it destroys the topology
    /// it replaces, and it is never taken quietly. One undoable step.
    RemeshLayer(LayerKey),
    /// How the next rebuild is made. Not an edit — nothing reaches the
    /// document until the rebuild is asked for.
    SetRemeshSettings(clayspace_model::RemeshSettings),
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
    /// Moves the active hierarchy's levels, or changes how many it has.
    ///
    /// One command carrying the operation rather than four, as
    /// [`Command::SculptLayer`] is one: the four differ in what they cost and
    /// in whether they redraw, and `MultiresLevelOp` is where that is stated.
    /// Deliberately not `SculptLayer`'s enum — that one addresses a grid's
    /// passes by position, and a hierarchy's are a different stack with
    /// different addressing.
    MultiresLevel(clayspace_model::MultiresLevelOp),
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
    /// Switches between the MatCap the sculpt path is tuned for and a fixed
    /// studio light rig.
    ///
    /// A MatCap is indexed by the view-space normal, so its lighting is welded
    /// to the camera: orbiting the form orbits the light with it. That is what
    /// makes it good for reading form and useless for judging how a surface
    /// takes a real light, which is the one thing the other mode is for.
    ToggleShading,
    /// Whether small creases are sharpened by a screen-space curvature term.
    ///
    /// A MatCap knows only the local normal and occlusion knows only its own
    /// radius, so neither says anything about a crease finer than that — which
    /// is most of the detail in a finished sculpt.
    ToggleCavity,
    /// Whether the studio rig's key light casts. Nothing in MatCap mode, whose
    /// light moves with the camera and whose shadow would swing with it.
    ToggleShadows,
    /// Which language the interface is presented in.
    SetLocale(Locale),
    /// Which picture of a voxel layer the viewport draws, and how much its
    /// occupancy is filtered before the smooth one is taken.
    SetVoxelDisplay(VoxelDisplay, SmoothBlur),
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
                | Self::ToggleShading
                | Self::ToggleCavity
                | Self::ToggleShadows
                | Self::NextDisplayUnit
                | Self::SetLocale(_)
                // A picture of a grid, not a change to one. The engine keeps
                // it an argument rather than grid state for the same reason.
                | Self::SetVoxelDisplay(..)
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
                // A level is not undo either, and for a nearer reason: three
                // of the four move a number and the fourth allocates, and
                // none of them is an entry the engine records. It takes the
                // composition root's own path, where its refusal has
                // somewhere to land.
                | Self::MultiresLevel(_)
                // Choosing how the *next* edit combines changes nothing yet;
                // the stroke that follows is the entry.
                | Self::SetCombine(_)
                // The same for the mask panel: dialling how far Expandir
                // reaches, or how thick an extrusion would be, is not the
                // operation. Applying one is, and does mark the document.
                | Self::SetMaskSteps(_)
                | Self::SetExtrudeSettings(_)
                // Choosing the gesture, drawing the outline and abandoning it
                // change nothing in the document. Only closing it does, which
                // is why `EndMaskOutline` is not listed here.
                | Self::SetMaskGesture(_)
                | Self::BeginMaskOutline(..)
                | Self::ExtendMaskOutline(_)
                | Self::CancelMaskOutline
                // Putting a cage up, resizing it, choosing a point and
                // dragging one all change the *cage* and not the clay. Only
                // applying it is an edit — which is also what makes the whole
                // cage one undo.
                // Placing and shaping a curve is authoring, and every one of
                // these reaches the document — so none of them is listed here.
                // Opening the picker, setting it, choosing an object and
                // saying what the manipulator is on all change what the
                // *interface* is doing and not the clay. Placing one is an
                // edit, and so is moving one, so neither is listed here.
                | Self::ToggleShapes
                | Self::SelectObject(_)
                | Self::SetShape(_)
                | Self::SetShapeParameters(_)
                // Saying where the *next* form would land changes nothing yet;
                // inserting one is the entry.
                | Self::SetInsertAs(_)
                // Importing a mesh as a subtool does change the document, but
                // it goes through the composition root's own path — dialog,
                // then model — and marks the document itself, exactly as
                // `RunImport` does. Routing it through the ordinary edit path
                // as well would double the entry.
                | Self::InsertMesh
                // Opening the boolean panel and setting it change nothing:
                // the whole point of stating a cost beforehand is that
                // choosing is free. Running one is the edit, and it is not
                // listed here.
                | Self::ToggleBoolean
                | Self::SetBoolean(_)
                | Self::SetMeshOperand(_)
                | Self::SetGizmoTarget(_)
                | Self::ToggleLattice
                | Self::SetLatticeDivisions(_)
                | Self::SelectLatticePoint(_)
                | Self::SelectLatticePoints(_)
                | Self::ToggleLatticePoint(_)
                | Self::SetGizmoMode(_)
                | Self::BeginGizmoDrag(..)
                | Self::DragGizmo(..)
                | Self::EndGizmoDrag
                | Self::DragLatticePoint(_)
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
                | Self::SetBrushAzimuth(_)
                | Self::SetBrushFalloff(_)
                | Self::SetBrushAccumulate(_)
                | Self::SetBrushAlpha(_)
                | Self::SetBrushColour(_)
                | Self::PickRecentColour(_)
                // Loading a stamp changes no surface; the stroke that uses it
                // does. Loading takes the composition root's own path for the
                // reason import does — it opens a dialog.
                | Self::LoadAlpha
                | Self::ClearAlpha
                // A reference is what the sculptor works *from*. None of it is
                // in the document, so none of it can mark one as modified.
                | Self::ToggleReferences
                | Self::LoadReference(_)
                | Self::ClearReference(_)
                | Self::SetReferenceSettings(..)
                | Self::SetSurfaceOpacity(_)
                | Self::SetBrushSmoothing(_)
                | Self::ToggleSymmetry(_)
                // Choosing which layer to work on changes nothing in the
                // document; changing that layer does. Entering rigging is the
                // same: it changes what the pointer means, not the surface.
                | Self::SelectLayer(_)
                // Solo is a way of looking at the scene. It writes visibility
                // and the engine journals that, but the document is the
                // sculpture and this changed none of it — a title bar saying
                // "não salvo" because someone looked at one subtool alone
                // would be reporting their attention as work.
                | Self::SoloLayer(_)
                | Self::ToggleArmatureEditing
                | Self::ToggleSkinPreview
        )
    }

    /// A short name for the history panel and for diagnostics.
    pub fn label(&self) -> &'static str {
        match self {
            Self::SelectTool(_) => "select tool",
            Self::ToggleCurve => "curva",
            Self::AddCurvePoint(..) => "ponto da curva",
            Self::SelectCurvePoint(_) | Self::ToggleCurvePoint(_) => "escolher ponto",
            Self::DragCurve(_) => "arrastar curva",
            Self::SetCurveRadius(_) => "espessura do tubo",
            Self::SetCurveJoin(_) => "junção da curva",
            Self::SetCurveProfile(_) => "perfil do tubo",
            Self::RemoveCurvePoints => "remover pontos",
            Self::ApplyCurve => "aplicar curva",
            Self::ToggleShapes => "formas",
            Self::SetShape(_) => "forma",
            Self::SetShapeParameters(_) => "medidas da forma",
            Self::InsertShape => "inserir forma",
            Self::SetInsertAs(_) => "destino da inserção",
            Self::InsertMesh => "inserir malha",
            Self::CopySubtool(_) => "copiar subtool",
            Self::ToggleBoolean => "painel de booleanas",
            Self::SetBoolean(_) => "ajustes da booleana",
            Self::RunBoolean => "booleana entre subtools",
            Self::SetMeshOperand(_) => "operando de malha",
            Self::SelectObject(_) => "selecionar objeto",
            Self::SetObjectShape(..) => "trocar forma",
            Self::SetObjectCombine(_) => "combinação do objeto",
            Self::RemoveObject => "remover objeto",
            Self::SetGizmoTarget(_) => "alvo do manipulador",
            Self::ToggleLattice => "gaiola",
            Self::SetLatticeDivisions(_) => "divisões da gaiola",
            Self::SelectLatticePoint(_) => "escolher ponto",
            Self::SelectLatticePoints(_) => "escolher pontos",
            Self::ToggleLatticePoint(_) => "escolher ponto",
            Self::SetGizmoMode(_) => "modo do manipulador",
            Self::BeginGizmoDrag(..) | Self::DragGizmo(..) | Self::EndGizmoDrag => "manipular",
            Self::DragLatticePoint(_) => "arrastar ponto",
            Self::ApplyLattice => "deformar pela gaiola",
            Self::ToggleMaskPainting => "máscara",
            Self::ApplyMaskOp(op) => op.label(),
            Self::SetMaskGesture(_) => "gesto da máscara",
            Self::BeginMaskOutline(..) | Self::ExtendMaskOutline(_) => "desenhar o laço",
            Self::EndMaskOutline(_) => "máscara em laço",
            Self::CancelMaskOutline => "abandonar o laço",
            Self::SetMaskSteps(_) => "mask steps",
            Self::SetExtrudeSettings(_) => "extrude settings",
            Self::ExtrudeMask(_) => "extrude mask",
            Self::SetBrushSize(_) => "brush size",
            Self::SetBrushIntensity(_) => "brush intensity",
            Self::SetBrushFlow(_) => "brush flow",
            Self::SetBrushNoise(_) => "brush noise",
            Self::SetBrushAzimuth(_) => "brush grain",
            Self::SetBrushFalloff(_) => "brush edge",
            Self::SetBrushAccumulate(_) => "brush accumulation",
            Self::SetBrushAlpha(_) => "brush stamp",
            Self::SetBrushColour(_) => "brush colour",
            Self::PickRecentColour(_) => "recent colour",
            Self::LoadAlpha => "load stamp",
            Self::ClearAlpha => "clear stamp",
            Self::ToggleReferences => "reference panel",
            Self::LoadReference(_) => "load reference",
            Self::ClearReference(_) => "clear reference",
            Self::SetReferenceSettings(..) => "reference placement",
            Self::SetSurfaceOpacity(_) => "surface opacity",
            Self::SetBrushSmoothing(_) => "brush smoothing",
            Self::SelectLayer(_) => "select layer",
            Self::SetLayerVisible(..) => "layer visibility",
            Self::SoloLayer(_) => "solo layer",
            Self::AddLayer(_) => "new layer",
            Self::RemoveLayer(_) => "remove layer",
            Self::OptimizeLayer(_) => "optimize layer",
            Self::RemeshLayer(_) => "remesh layer",
            Self::SetRemeshSettings(_) => "remesh settings",
            Self::BeginRenameLayer(_) => "rename layer",
            Self::EditLayerName(_) => "layer name",
            Self::CommitRenameLayer => "rename layer",
            Self::CancelRenameLayer => "cancel rename",
            Self::ToggleSymmetry(_) => "symmetry",
            Self::NewArmature => "new armature",
            Self::ToggleArmatureEditing => "edit armature",
            Self::RemoveZsphere => "remove zsphere",
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
            Self::ToggleShading => "shading",
            Self::ToggleCavity => "cavity",
            Self::ToggleShadows => "shadows",
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
            Self::MultiresLevel(op) => op.label(),
            Self::ToggleDeform => "deform panel",
            Self::SetDeform(_) => "deform settings",
            Self::RunDeform => "deform",
            Self::SetConversion(_) => "conversion settings",
            Self::RunConversion => "convert",
            Self::RunImport => "import",
            Self::RunExport => "export",
            Self::NextDisplayUnit => "display unit",
            Self::SetLocale(_) => "idioma",
            Self::SetVoxelDisplay(..) => "exibição de voxels",
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
            Command::ToggleShading,
            Command::ToggleCavity,
            Command::ToggleShadows,
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
            Command::AddLayer(Representation::Sdf),
            Command::RemoveLayer(clayspace_model::LayerKey(1)),
        ] {
            assert!(command.touches_document(), "{} is an edit", command.label());
        }
    }

    /// Putting a form into the scene is an edit whichever destination it takes,
    /// and saying which destination is not. The two used to be one command and
    /// a hidden default; keeping them apart is what lets a control offer the
    /// choice without the choice itself entering the history.
    #[test]
    fn inserting_a_form_is_an_edit_and_choosing_where_is_not() {
        assert!(Command::InsertShape.touches_document());
        assert!(Command::CopySubtool(clayspace_model::LayerKey(1)).touches_document());
        assert!(
            !Command::SetInsertAs(clayspace_model::InsertAs::Object).touches_document(),
            "saying where the next form lands is not itself an edit"
        );
        assert!(
            !Command::InsertMesh.touches_document(),
            "the import marks the document on the composition root's own path; \
             counting it here would double the entry"
        );
    }

    /// Resolving a boolean is an edit; choosing what one would do is not.
    /// The cost has to be readable while the sculptor decides, and a panel
    /// that marked the document unsaved for being looked at would be
    /// reporting their attention as work.
    #[test]
    fn running_a_boolean_is_an_edit_and_setting_one_up_is_not() {
        assert!(Command::RunBoolean.touches_document());
        assert!(!Command::ToggleBoolean.touches_document());
        assert!(
            !Command::SetBoolean(clayspace_model::BooleanSettings::default()).touches_document(),
            "choosing the operands and the resolution must run nothing"
        );
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
