//! The sculpting ViewModel.
//!
//! Holds what the interface draws and turns commands into Model calls. It
//! depends on no interface library and no renderer, so every behaviour here is
//! exercised in a test with no window and no GPU.

use clayspace_model::{
    BrushSettings, EditOutcome, GestureSample, HistoryState, ModelError, Representation,
    SceneStats, SculptModel, ToolKind, ViewPresetKind,
};

use crate::command::{Axis, Command};

/// What the status line says when a layer change forced a different tool.
///
/// A marker rather than the sentence: the ViewModel layer carries no locale,
/// and the View swaps it for the localised string. Every other status here is
/// an engine refusal, which is already English and already flows through.
pub const TOOL_SUBSTITUTED: &str = "tool-substituted";
use crate::observable::Observable;

/// A stroke being drawn.
#[derive(Debug, Default)]
struct ActiveStroke {
    samples: Vec<GestureSample>,
    /// Wall-clock is not available here, so time advances by sample index.
    /// The engine uses it only for ordering and taper.
    next_time: f32,
    /// How many samples have already been sent to the model.
    ///
    /// A gesture is applied as it is drawn rather than on release, so the
    /// sculptor watches the clay move under the pointer. This marks the
    /// boundary between what the document already has and what is still only
    /// a pointer path.
    applied: usize,
    /// Arc length travelled since the last segment was sent.
    travelled: f32,
}

/// What the last completed operation did, for the status area.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LastAction {
    /// The tool that made it, where one did.
    ///
    /// Carried beside the label rather than instead of it, because the View is
    /// the only layer with a language: it names the tool from its own table
    /// and falls back to the label for the actions no tool made.
    pub tool: Option<ToolKind>,
    pub label: String,
    /// False when the engine reported the edit changed nothing.
    pub changed: bool,
}

/// One thing a sculptor did, and what it cost the document's history.
///
/// The label is carried beside the count rather than derived from it because
/// the only other place a name for it existed was [`LastAction`], which names
/// the *previous* thing that happened and is overwritten by the undo itself.
/// Reading it as "what the next undo would take back" is how a session that
/// had just undone a clay stroke reported that its next undo would undo an
/// "undo" — see `SculptViewModel::next_undo`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BankedAction {
    /// In the words the interface uses for it, which is what a person reads
    /// off the history panel and what an agent is told it is about to revert.
    label: String,
    entries: usize,
}

/// Everything the sculpting interface reads.
pub struct SculptViewModel {
    model: Box<dyn SculptModel>,

    tool: Observable<ToolKind>,
    /// Brush settings, per tool *and* per representation.
    ///
    /// A size that suits a voxel grid's cells is not the size that suits a
    /// field, so returning to a tool on a layer returns the settings it had
    /// *there* rather than the ones it last had anywhere. A slot nothing has
    /// been set in holds its representation's own default,
    /// `BrushSettings::default_for`, and never a value carried from another.
    brushes: [[BrushSettings; ToolKind::ALL.len()]; Representation::ALL.len()],
    /// Set when the last layer change had to change the tool too, so the
    /// status line can say so rather than leaving it unexplained.
    substituted: bool,
    /// The tool the sculptor chose and the one standing in for it, while a
    /// stand-in is in hand.
    ///
    /// Kept past the moment of the swap, which `substituted` is not: a switch
    /// back to a layer that carries the chosen tool returns it, and `state`
    /// has to be able to tell an agent that the tool it reads was given rather
    /// than chosen. Cleared by choosing a tool, which is the sculptor
    /// answering the question the swap asked.
    substitution: Option<clayspace_model::Substitution>,
    brush: Observable<BrushSettings>,
    /// How the next SDF edit combines with what is under it.
    ///
    /// One setting for the document rather than one per tool: it is a property
    /// of the edit being made, and a sculptor who sets an operation and then
    /// changes brush expects to still be cutting.
    combine: Observable<clayspace_model::CombineSettings>,
    /// What the colour brushes paint with, and the colours before it.
    ///
    /// Beside the combine operation and for the same reason: one value for the
    /// session, not one per tool. `ToolKind::writes_colour` names who reads it,
    /// which is also where the swatch is shown.
    colour: Observable<clayspace_model::ColourState>,
    /// Which frequency a smooth on a hierarchy acts on.
    ///
    /// One value for the session, as the two above are, and read from the
    /// model at construction rather than assumed: the bar shows what this
    /// holds and the stroke uses what the document holds, and the two starting
    /// out disagreeing would be a control that lies before anybody has touched
    /// it.
    smooth_mode: Observable<clayspace_model::SmoothFrequency>,
    symmetry: Observable<[bool; 3]>,
    view_preset: Observable<ViewPresetKind>,
    grid: Observable<bool>,
    /// Whether a mesh layer is drawn with its edges over it.
    polyframe: Observable<bool>,
    /// What was held when the current gesture began. Cleared with it.
    modifiers: clayspace_model::StrokeModifiers,
    /// What was in hand when mask painting was turned on.
    ///
    /// Held so the key can turn it off again: freezing a region is a detour
    /// from whatever is being sculpted, and a sculptor who takes it should not
    /// have to find their brush on the shelf afterwards. Cleared by choosing a
    /// tool outright, which says the detour is over.
    tool_before_mask: Option<ToolKind>,

    history: Observable<HistoryState>,
    stats: Observable<SceneStats>,
    /// Why the active tool cannot be used, when it cannot.
    tool_status: Observable<Option<String>>,
    last_action: Observable<LastAction>,
    /// Set when an edit dirtied bricks the viewport has not re-meshed yet.
    pending_remesh: Observable<usize>,

    stroke: Option<ActiveStroke>,

    /// What each user-visible action was, and how many model-level entries it
    /// produced, newest last.
    ///
    /// A live stroke reaches the document as several calls so the clay moves
    /// under the pointer, and each is its own entry in the document's history.
    /// A sculptor did one thing, though, and expects one undo to remove it —
    /// forty presses to erase one stroke is not undo, it is punishment. The
    /// engine's own undo grouping does not collapse them (measured: three
    /// grouped strokes left seven entries, and undoing twice reverted none),
    /// so the count is kept here and `Undo` spends it all at once.
    undo_stack: Vec<BankedAction>,
    /// The same, for actions that have been undone.
    redo_stack: Vec<BankedAction>,
    /// Where the model's history stood when the gesture in progress opened.
    ///
    /// What a gesture cost is the distance from this line to where the history
    /// stands when the gesture ends — however it ends. Counting the segments
    /// instead was the defect: a segment is one entry on a field, and a mesh
    /// gesture is previewed while it is made and banked as a *single* record
    /// however many segments drew it. A three-segment mesh gesture therefore
    /// banked three, and one Cmd+Z after it walked back the gesture and then
    /// whatever was underneath — the gestures committed before it, and on a
    /// layer that had only just been made, the layer itself. Cancel spent the
    /// same wrong number and was the most destructive command in the mesh
    /// path.
    ///
    /// The depth the gesture started from is the line, and it is the same line
    /// whatever the representation chose to write above it — one record or
    /// twenty. `None` when no gesture is open, which is what makes a cancel
    /// with nothing to cancel a no-op rather than an undo of whatever came
    /// last.
    gesture_floor: Option<usize>,
    /// Whether the gesture in progress is being shown as it is made.
    ///
    /// A live gesture writes nothing to the document until it closes, so the
    /// segments are not held whole and the commit is where the record lands.
    live: bool,
}

impl SculptViewModel {
    pub fn new(model: Box<dyn SculptModel>) -> Self {
        let stats = model.stats();
        let smooth_mode = model.smooth_mode();
        // Empty, not the model's. The engine's history counts building the
        // starting form, which is not something the user did and must not be
        // something they can undo.
        let history = HistoryState::default();
        let mut vm = Self {
            model,
            tool: Observable::new(ToolKind::Padrao),
            brushes: Representation::ALL.map(|representation| {
                [BrushSettings::default_for(representation); ToolKind::ALL.len()]
            }),
            substituted: false,
            substitution: None,
            brush: Observable::new(BrushSettings::default()),
            // X on, matching the document the engine adapter builds. These two
            // are separate pieces of state and they must not start out
            // disagreeing: the ViewModel is what the options bar shows, and a
            // bar reading "X on" over a document with no mirror is a lie
            // before the user has touched anything.
            combine: Observable::new(clayspace_model::CombineSettings::for_strokes()),
            colour: Observable::new(clayspace_model::ColourState::default()),
            smooth_mode: Observable::new(smooth_mode),
            symmetry: Observable::new([true, false, false]),
            view_preset: Observable::new(ViewPresetKind::Perspective),
            grid: Observable::new(true),
            // Off by default. A polyframe over a dense mesh is a lot of ink,
            // and it is asked for when a question about density comes up
            // rather than kept on.
            polyframe: Observable::new(false),
            modifiers: clayspace_model::StrokeModifiers::default(),
            tool_before_mask: None,
            history: Observable::new(history),
            stats: Observable::new(stats),
            tool_status: Observable::new(None),
            last_action: Observable::new(LastAction::default()),
            pending_remesh: Observable::new(0),
            stroke: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            gesture_floor: None,
            live: false,
        };
        vm.refresh_tool_status();
        vm
    }

    // -- what the interface reads ----------------------------------------

    pub fn tool(&self) -> &Observable<ToolKind> {
        &self.tool
    }

    pub fn brush(&self) -> &Observable<BrushSettings> {
        &self.brush
    }

    pub fn combine(&self) -> &Observable<clayspace_model::CombineSettings> {
        &self.combine
    }

    pub fn colour(&self) -> &Observable<clayspace_model::ColourState> {
        &self.colour
    }

    pub fn smooth_mode(&self) -> &Observable<clayspace_model::SmoothFrequency> {
        &self.smooth_mode
    }

    /// Whether the smooth frequency is a choice worth showing right now.
    ///
    /// Both halves, and they fail differently. On the other three
    /// representations there is one smooth and a three-way control would
    /// decide nothing; with another tool in hand there is no smooth at all.
    /// The same question the options bar asks before drawing the control and
    /// the only one that may decide where it appears.
    pub fn offers_smooth_mode(&self) -> bool {
        clayspace_model::SmoothFrequency::is_offered_on(self.active_representation())
            && *self.tool.get() == ToolKind::Suavizar
    }

    pub fn symmetry(&self) -> &Observable<[bool; 3]> {
        &self.symmetry
    }

    pub fn view_preset(&self) -> &Observable<ViewPresetKind> {
        &self.view_preset
    }

    pub fn polyframe(&self) -> &Observable<bool> {
        &self.polyframe
    }

    pub fn grid(&self) -> &Observable<bool> {
        &self.grid
    }

    pub fn history(&self) -> &Observable<HistoryState> {
        &self.history
    }

    pub fn stats(&self) -> &Observable<SceneStats> {
        &self.stats
    }

    /// Why the active tool is unavailable, when it is. `None` means usable.
    pub fn tool_status(&self) -> &Observable<Option<String>> {
        &self.tool_status
    }

    pub fn last_action(&self) -> &Observable<LastAction> {
        &self.last_action
    }

    /// What the next undo would take back, in the words the interface uses.
    ///
    /// The question a caller about to press Cmd+Z is actually asking, and not
    /// the one [`Self::last_action`] answers: that names the last thing that
    /// *happened*, which after an undo is the undo. A status area can live
    /// with the difference because a person watched the undo happen; an agent
    /// reading state cannot, and read "undo" as the name of the edit it was
    /// about to revert.
    pub fn next_undo(&self) -> Option<&str> {
        self.undo_stack.last().map(|action| action.label.as_str())
    }

    /// What the next redo would put back. The mirror of [`Self::next_undo`].
    pub fn next_redo(&self) -> Option<&str> {
        self.redo_stack.last().map(|action| action.label.as_str())
    }

    pub fn pending_remesh(&self) -> &Observable<usize> {
        &self.pending_remesh
    }

    /// Whether a stroke is being drawn.
    pub fn is_stroking(&self) -> bool {
        self.stroke.is_some()
    }

    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        self.model.bounds()
    }

    /// Where a ray meets the surface — what the brush cursor follows.
    pub fn pick(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]> {
        self.model.pick(origin, direction)
    }

    /// Drops the undo history, for when the document underneath is replaced.
    ///
    /// Opening a document or starting a new one must not leave undo able to
    /// reach back into a document the user is no longer looking at — the
    /// entries would apply to a document that is gone, and the counts would
    /// spend undos the engine no longer has.
    pub fn forget_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.gesture_floor = None;
        self.stroke = None;
        self.publish_history();
        self.stats.set(self.model.stats());
        self.last_action.set(LastAction::default());
    }

    /// Clears the pending re-mesh count once the viewport has caught up.
    pub fn acknowledge_remesh(&mut self) {
        self.pending_remesh.set_if_changed(0);
    }

    // -- the one path that changes anything ------------------------------

    /// Applies a command. The only entry point that mutates.
    pub fn dispatch(&mut self, command: Command) -> Result<(), ModelError> {
        match command {
            Command::SelectTool(tool) => {
                // Chosen from the shelf, so the mask key has nothing to
                // return to: the sculptor has said which tool they want.
                self.tool_before_mask = None;
                self.select(tool);
            }
            Command::ToggleMaskPainting => {
                let tool = match self.tool_before_mask.take() {
                    // Painting already, so this is the way out — back to
                    // whatever was in hand when the key was first pressed.
                    Some(previous) => previous,
                    None => {
                        self.tool_before_mask = Some(*self.tool.get());
                        ToolKind::Mascara
                    }
                };
                self.select(tool);
            }
            Command::SetCombine(combine) => {
                let combine = combine.sanitized();
                if self.combine.set_if_changed(combine) {
                    self.model.set_combine(combine);
                }
            }
            Command::SetSmoothMode(mode) => {
                if self.smooth_mode.set_if_changed(mode) {
                    self.model.set_smooth_mode(mode);
                }
            }
            Command::SetBrushColour(colour) => {
                self.model.set_colour(colour);
                self.colour.set_if_changed(self.model.colour_state());
            }
            Command::PickRecentColour(index) => {
                if self.model.choose_recent_colour(index) {
                    self.colour.set_if_changed(self.model.colour_state());
                }
            }
            Command::SetBrushSize(size) => self.edit_brush(|b| b.size = size),
            Command::SetBrushIntensity(value) => self.edit_brush(|b| b.intensity = value),
            Command::SetBrushFlow(value) => self.edit_brush(|b| b.flow = value),
            Command::SetBrushNoise(value) => self.edit_brush(|b| b.shaping.noise = value),
            Command::SetBrushPressureSize(value) => {
                self.edit_brush(|b| b.dynamics.pressure_size = value)
            }
            Command::SetBrushPressureStrength(value) => {
                self.edit_brush(|b| b.dynamics.pressure_strength = value)
            }
            Command::SetBrushPressureCurve(value) => {
                self.edit_brush(|b| b.dynamics.pressure_curve = value)
            }
            Command::SetBrushTaperStart(value) => {
                self.edit_brush(|b| b.dynamics.taper_start = value)
            }
            Command::SetBrushTaperEnd(value) => self.edit_brush(|b| b.dynamics.taper_end = value),
            Command::SetBrushRake(value) => self.edit_brush(|b| b.dynamics.rake = value),
            Command::SetBrushDragFalloff(value) => self.edit_brush(|b| b.drag.falloff = value),
            Command::SetBrushFrontOnly(value) => self.edit_brush(|b| b.drag.front_only = value),
            Command::SetBrushAzimuth(value) => self.edit_brush(|b| b.shaping.azimuth = value),
            Command::SetBrushFalloff(falloff) => self.edit_brush(|b| b.shaping.falloff = falloff),
            Command::SetBrushAccumulate(on) => self.edit_brush(|b| b.shaping.accumulate = on),
            Command::SetBrushAlpha(on) => self.edit_brush(|b| b.alpha = on),
            Command::SetBrushSmoothing(value) => self.edit_brush(|b| b.shaping.smoothing = value),

            // Scene, layer, mask and armature commands belong to other
            // ViewModels; the sculpting one ignores them rather than
            // half-handling them.
            // Placed objects are the object ViewModel's, for the reason the
            // cage is the lattice one's: a brush asks what happens to the
            // surface under the pointer, and an object asks what happens to a
            // thing standing in the scene.
            Command::ToggleShapes
            | Command::SetShape(_)
            | Command::SetShapeParameters(_)
            | Command::InsertShape
            | Command::SetInsertAs(_)
            | Command::InsertMesh
            | Command::CopySubtool(_)
            // A boolean between two subtools is the boolean ViewModel's, and
            // it is a decision about two whole forms rather than an edit under
            // the pointer.
            | Command::ToggleBoolean
            | Command::SetBoolean(_)
            | Command::RunBoolean
            | Command::SetMeshOperand(_)
            | Command::SelectObject(_)
            | Command::SetObjectShape(..)
            | Command::SetObjectCombine(_)
            | Command::RemoveObject
            | Command::SetGizmoTarget(_)
            | Command::NewDocument
            | Command::OpenDocument
            | Command::OpenRecent(_)
            | Command::Save
            | Command::SaveAs
            | Command::Quit
            | Command::ToggleImport
            | Command::ToggleExport
            | Command::SetImportSettings(_)
            | Command::SetExportSettings(_)
            | Command::RunImport
            | Command::RunExport
            | Command::NewArmature
            | Command::ToggleArmatureEditing
            | Command::RemoveZsphere
            | Command::ToggleSkinPreview
            | Command::ToggleZsphereNegative
            | Command::SetSkinThickness(_)
            | Command::ApplyMaskOp(_)
            // The mask's own gesture, which belongs to the mask ViewModel.
            | Command::SetMaskGesture(_)
            | Command::BeginMaskOutline(..)
            | Command::ExtendMaskOutline(_)
            | Command::EndMaskOutline(_)
            | Command::CancelMaskOutline
            | Command::ToggleCurve
            // The cut's own gesture, which belongs to the cut ViewModel: it
            // is drawn on the view frame rather than across the surface, so
            // nothing here has an opinion about it.
            | Command::SetCutGesture(_)
            | Command::BeginCut(_)
            | Command::ExtendCut(_)
            | Command::EndCut(_)
            | Command::CancelCut
            | Command::AddCurvePoint(..)
            | Command::InsertCurvePoint(..)
            | Command::SelectCurvePoint(_)
            | Command::ToggleCurvePoint(_)
            | Command::DragCurve(_)
            | Command::SetCurveRadius(_)
            | Command::SetCurveJoin(_)
            | Command::SetCurveProfile(_)
            | Command::RemoveCurvePoints
            | Command::ApplyCurve
            | Command::ToggleLattice
            | Command::SetLatticeDivisions(_)
            | Command::SelectLatticePoint(_)
            | Command::SelectLatticePoints(_)
            | Command::ToggleLatticePoint(_)
            | Command::SetGizmoMode(_)
            | Command::BeginGizmoDrag(..)
            | Command::DragGizmo(..)
            | Command::EndGizmoDrag
            | Command::DragLatticePoint(_)
            | Command::ApplyLattice
            | Command::SetMaskSteps(_)
            | Command::SetExtrudeSettings(_)
            | Command::ExtrudeMask(_)
            | Command::ToggleConvert
            | Command::ToggleRepair
            | Command::SetConversion(_)
            | Command::RunConversion
            // Retopology belongs to its own ViewModel, which owns the job that
            // runs it off this thread. Nothing here reads or changes.
            | Command::SetRetopoSettings(_)
            | Command::RunRetopology
            | Command::CancelRetopology
            | Command::SetUvSettings(_)
            | Command::RunUvAtlas
            | Command::CancelUvAtlas
            | Command::SetConformSettings(_)
            | Command::RunConform
            | Command::CancelConform
            | Command::SetBakeSettings(_)
            | Command::ChooseBakeDestination
            | Command::RunBake
            | Command::CancelBake
            // How the next rebuild is made, which reaches nothing until it is
            // asked for.
            | Command::SetRemeshSettings(_)
            // The stamp is loaded through the composition root, which owns the
            // file dialog; the ViewModel only reads its name back.
            | Command::LoadAlpha
            | Command::ClearAlpha
            | Command::ToggleDeform
            | Command::SetDeform(_)
            | Command::RunDeform
            | Command::SculptLayer(_)
            // A level moves where a brush writes and what is drawn, and
            // neither is a brush setting: the shelf offers the same verbs
            // either way, so nothing this ViewModel holds follows it.
            | Command::MultiresLevel(_)
            // And a pass is a property of the hierarchy's stack, not of the
            // brush: the shelf offers the same verbs whichever pass a stroke
            // is going into.
            | Command::MultiresSculptLayer(_)
            | Command::SetLayerVisible(..)
            // Solo shows a subtool alone without making it the one a brush
            // lands on, so nothing this ViewModel holds follows it.
            | Command::SoloLayer(_)
            | Command::BeginRenameLayer(_)
            | Command::EditLayerName(_)
            | Command::CommitRenameLayer
            | Command::CancelRenameLayer => {}
            // The layer changed, so what the shelf offers and what the brush
            // is set to may both belong to a different representation now.
            // Pre-bake repairs move the surface, so the history and the
            // statistics both change — the composition root runs them and
            // refreshes, exactly as it does a conversion.
            // A rebuild joins them: it replaces a mesh layer's triangles and
            // leaves it a mesh layer, so the shelf offers what it offered and
            // only the statistics and the history move.
            Command::CloseHoles
            | Command::FillVoids
            | Command::OptimizeLayer(_)
            | Command::RemeshLayer(_) => {}
            // Everything that moves the sculpt target, and a stack click is
            // only the most obvious of them. A new layer arrives *active* —
            // `add_layer` activates it through the same call a click takes —
            // and removing one hands the target to whatever is left, which
            // may be a different representation. Both used to be ignored
            // here, so `layer add {kind:'grid'}` left the brush holding the
            // field layer's settings: a size of 100 mm on a field is a metre
            // on a grid, and the first dab came out that wide.
            //
            // All three read the document, so the scene ViewModel has to have
            // acted already. That is why the composition root dispatches to it
            // first; see `App::dispatch_to_models`.
            Command::SelectLayer(_) | Command::AddLayer(_) | Command::RemoveLayer(_) => {
                self.follow_the_active_layer()
            }
            Command::ToggleSymmetry(axis) => {
                let index = match axis {
                    Axis::X => 0,
                    Axis::Y => 1,
                    Axis::Z => 2,
                };
                let mut axes = *self.symmetry.get();
                axes[index] = !axes[index];
                // Written through to the model, because symmetry belongs to
                // the subtool rather than to the session: the mirror the
                // engine keeps is per layer, and this is what puts the
                // sculptor's answer where switching away and back will find
                // it again.
                self.model.set_symmetry(axes)?;
                self.symmetry.set(axes);
            }

            Command::BeginStroke {
                position,
                pressure,
                modifiers,
            } => {
                // A cage owns the form it stands around, and it owns it for
                // every caller and not only for the pointer. The viewport
                // already routes a press away from the brush while one is up;
                // this is the same rule where the command arrives, so a
                // caller that never touched a pointer meets it too.
                if self.model.active_layer_is_caged() {
                    return Err(ModelError::Unavailable(
                        clayspace_model::Unavailable::LayerCaged,
                    ));
                }
                // Refuse before collecting anything, so an unavailable tool
                // cannot accumulate a gesture it will never apply.
                self.ensure_tool_available()?;
                // Read before anything this gesture does reaches the model:
                // opening a live gesture points the layer's mirror, and that
                // is an edit the gesture caused and therefore owes back.
                self.gesture_floor = Some(self.model.history().depth);
                // Held for the gesture. The shelf still shows the tool that was
                // chosen — letting go of the key returns to it — so this is a
                // substitution rather than a selection.
                self.modifiers = modifiers;
                // Asked before the segmentation is decided, because the answer
                // is what decides it: a gesture the model can show while it is
                // made is sent in segments rather than held.
                let tool = self.stroking_tool();
                self.live = self.model.open_live_gesture(tool, *self.symmetry.get());
                let tool_is_region = self.holds_the_whole_gesture(tool);
                // The model is told a gesture is open, so a dragging verb on a
                // mesh can preview it — take back what the last segment did and
                // lay the whole gesture down again from its anchor — instead of
                // stacking segment on segment.
                self.model.begin_gesture();
                let mut stroke = ActiveStroke::default();
                stroke.push(position, pressure);
                self.stroke = Some(stroke);
                // The first dab lands on the press rather than on the first
                // move: a click is a stroke too. A region tool has nothing to
                // act on yet — it needs the gesture, not a point.
                if !tool_is_region {
                    return self.apply_segment();
                }
            }
            Command::ContinueStroke { position, pressure } => {
                let tool = self.stroking_tool();
                // Asked before the stroke is borrowed: it reads the model, and
                // the borrow below is exclusive.
                let whole = self.holds_the_whole_gesture(tool);
                let stamps = self.stamps_between_segments(tool);
                let Some(stroke) = self.stroke.as_mut() else {
                    return Ok(());
                };
                stroke.push(position, pressure);
                let brush = *self.brush.get();
                // A region tool is applied once, when the gesture is complete.
                // Segmenting it stacks a replacement per segment and the
                // result crumbles.
                if !whole && stroke.segment_is_worth_applying(&brush, stamps) {
                    return self.apply_segment();
                }
            }
            Command::EndStroke => return self.commit_stroke(),
            Command::CancelStroke => return self.cancel_stroke(),

            Command::Undo => return self.undo_action(),
            Command::Redo => return self.redo_action(),

            Command::SetViewPreset(preset) => {
                self.view_preset.set_if_changed(preset);
            }
            Command::ToggleGrid => {
                let current = *self.grid.get();
                self.grid.set(!current);
            }
            Command::TogglePolyframe => {
                let current = *self.polyframe.get();
                self.polyframe.set(!current);
            }
            // Framing, material and the reference images are somebody else's
            // business; the ViewModel records nothing for them because they
            // change no state it owns.
            Command::ToggleReferences
            | Command::LoadReference(_)
            | Command::ClearReference(_)
            | Command::SetReferenceSettings(..)
            | Command::SetSurfaceOpacity(_)
            | Command::FrameAll
            | Command::NextMaterial
            // Shading, the cavity term and the studio rig's shadow are the
            // renderer's own display state, like the material beside them: no
            // ViewModel state changes, so there is nothing here to record.
            | Command::ToggleShading
            | Command::ToggleCavity
            | Command::ToggleShadows
            | Command::NextDisplayUnit
            | Command::SetLocale(_)
            | Command::SetVoxelDisplay(..)
            | Command::ToggleAttribution
            | Command::ToggleDiagnostics
            | Command::CopyDiagnostics
            // The door is not the sculpture.
            | Command::SelectZsphere(_)
            | Command::AddZsphere { .. }
            | Command::InsertZsphere(_)
            | Command::MoveZsphere { .. }
            | Command::ResizeZsphere { .. }
            | Command::ReparentZsphere { .. }
            | Command::ToggleAgentDoor
            | Command::ShowAgentAccess(_)
            | Command::AnswerAgentAsk(_)
            // The file belongs to the platform, as the clipboard does. The
            // composition root writes it; no ViewModel state changes here.
            | Command::ExportProfile => {}
        }
        Ok(())
    }

    // -- internals --------------------------------------------------------

    fn index(tool: ToolKind) -> usize {
        ToolKind::ALL
            .iter()
            .position(|candidate| *candidate == tool)
            .unwrap_or(0)
    }

    fn row(representation: Representation) -> usize {
        Representation::ALL
            .iter()
            .position(|candidate| *candidate == representation)
            .unwrap_or(0)
    }

    /// Where this tool's settings live for the layer that is active now.
    fn slot(&self, tool: ToolKind) -> (usize, usize) {
        (
            Self::row(self.model.active_representation()),
            Self::index(tool),
        )
    }

    /// Keeps the active tool where the new layer has it, replaces it where it
    /// does not.
    ///
    /// Called after the layer changed, so `active_representation` already
    /// answers for the new one. Two things move: the tool, if the new
    /// representation has no verb for it, and the brush settings, which are
    /// held per representation and so belong to the new layer now even when
    /// the tool did not change.
    ///
    /// Silently resetting to the first tool was the other option, and it is
    /// the one that leaves a sculptor wondering what they pressed. The status
    /// line says what happened instead.
    ///
    /// The substitute is the capability table's answer,
    /// `ToolKind::substitute_on`, and it is asked on behalf of the tool the
    /// sculptor *chose* rather than the one in hand: moving from a grid to a
    /// field with Raspar gives Planar, and moving back gives Raspar again —
    /// not Planar, which nobody picked.
    fn follow_the_active_layer(&mut self) {
        // The incoming subtool's own mirror, not the one left behind on the
        // outgoing one. Turning symmetry off to work one ear said nothing
        // about the subtool beside it.
        self.symmetry.set_if_changed(self.model.symmetry());
        let representation = self.model.active_representation();
        let chosen = self
            .substitution
            .map_or(*self.tool.get(), |substitution| substitution.chosen);
        let standing_in = chosen.substitute_on(representation);
        self.substitution = (standing_in != chosen).then_some(clayspace_model::Substitution {
            chosen,
            standing_in,
            representation,
        });
        // Announced only when the tool in hand moved *to* a stand-in. One that
        // stays in hand across a second switch is not news, and a return to
        // the chosen tool is the swap being undone rather than a new one.
        if self.tool.set_if_changed(standing_in) && self.substitution.is_some() {
            self.substituted = true;
        }
        let (row, index) = self.slot(*self.tool.get());
        self.brush.set(self.brushes[row][index]);
        self.refresh_tool_status();
    }

    fn store_brush(&mut self) {
        let (row, index) = self.slot(*self.tool.get());
        self.brushes[row][index] = *self.brush.get();
    }

    fn edit_brush(&mut self, change: impl FnOnce(&mut BrushSettings)) {
        let mut settings = *self.brush.get();
        change(&mut settings);
        let settings = settings.sanitized();
        if self.brush.set_if_changed(settings) {
            let (row, index) = self.slot(*self.tool.get());
            self.brushes[row][index] = settings;
        }
    }

    /// Catches the ViewModel up with whichever layer is active *now*.
    ///
    /// For the callers that move the active layer without a `SelectLayer`
    /// passing through `dispatch`, and there are three of them:
    ///
    /// - a crossing, which adds the converted layer and makes it active, so
    ///   the shelf and the brush would otherwise still belong to the layer
    ///   the sculptor converted *from*;
    /// - opening a file, which changes every per-subtool setting at once —
    ///   and since a layer mirror cannot be read back out of a file, the
    ///   symmetry toggles claiming the closed document's is the one thing the
    ///   options bar must not do;
    /// - starting a rig, which gives the armature a layer of its own *with
    ///   symmetry off*, because `add_zsphere` places the reflected node
    ///   itself. Left showing the previous subtool's mirror, the first
    ///   ZSphere hung a second arm off the first.
    ///
    /// One name rather than one per caller: what they have in common is the
    /// only thing this does.
    pub fn refresh_for_active_layer(&mut self) {
        self.follow_the_active_layer();
    }

    /// The tool the sculptor chose and the one in hand instead, while a layer
    /// switch has left a stand-in in hand.
    ///
    /// What `state` reports and what the answer to the switching command
    /// says, so a caller can tell a tool it chose from one it was given.
    pub fn substitution(&self) -> Option<clayspace_model::Substitution> {
        self.substitution
    }

    /// What the active layer holds, for the shell to show and the shelf to
    /// filter on.
    pub fn active_representation(&self) -> Representation {
        self.model.active_representation()
    }

    fn refresh_tool_status(&mut self) {
        // A substitution outranks an availability reason: the tool that was
        // swapped in is available by construction, so there would be nothing
        // else to report, and the swap is the part that needs explaining.
        if self.substituted {
            self.substituted = false;
            // Announced: a swap is something that just happened, and two
            // swaps in a row are two of them even though the sentence is the
            // same. The availability reason below is state rather than an
            // event — it goes on being true between commands — so it stays a
            // plain set.
            self.tool_status
                .announce(Some(TOOL_SUBSTITUTED.to_string()));
            return;
        }
        let status = self
            .tool
            .get()
            .availability(self.model.active_layer_state())
            .err()
            .map(|why| why.to_string());
        self.tool_status.set_if_changed(status);
    }

    fn ensure_tool_available(&mut self) -> Result<(), ModelError> {
        self.refresh_tool_status();
        let tool = *self.tool.get();
        tool.availability(self.model.active_layer_state())
            .map_err(ModelError::Unavailable)?;

        if !tool.is_stroke_tool() {
            return Err(ModelError::Unavailable(
                clayspace_model::Unavailable::WrongGesture {
                    needs: "a shape on the frame",
                },
            ));
        }
        Ok(())
    }

    /// Whether this tool waits for the whole gesture before applying any of it.
    ///
    /// A region tool does: it acts on the area the gesture encloses, and
    /// segmenting one stacks a replacement per segment until the result
    /// crumbles.
    fn holds_the_whole_gesture(&self, tool: ToolKind) -> bool {
        // Not on a mesh. A region tool is one that *bakes*: on a field
        // Suavizar, Relaxar, Planar and Polir sample a region into a volume,
        // modify it and put it back with a replace, and segmenting that stacks
        // a replacement per segment until the result crumbles.
        //
        // On a mesh they are none of those things — they are ordinary stamps
        // over the vertices in reach, exactly like Padrão. Held whole, they
        // arrived only when the pointer came up, which is half of why Suavizar
        // read as doing nothing: the other half was that it was clamped and
        // barely smoothed at all.
        // And not while the model is showing the gesture as it is made: a
        // live gesture is exactly one that no longer has to be held.
        // On a mesh they are none of those things, so the question is asked
        // of the representation rather than of the tool alone —
        // `ToolKind::holds_the_whole_gesture` carries the one case that
        // depends on it, which is the drag on a grid.
        !self.live
            && self.model.active_representation() != Representation::Mesh
            && tool.holds_the_whole_gesture(self.model.active_representation())
    }

    /// How far a stroke travels before a segment is sent, in stamps.
    ///
    /// A segment on a *field* or a *grid* costs a re-mesh of every brick it
    /// touched, so it waits for three stamps' worth: sending one per pointer
    /// move would re-mesh the same neighbourhood over and over.
    ///
    /// On a mesh nothing is re-meshed — the layer's own triangles are what the
    /// viewport reads — so a segment costs the stamp and the buffer it fills,
    /// measured at about 7 ms for a 0.18 brush on 140,774 vertices. One stamp
    /// is the natural grain there: the engine resolves a stroke into stamps a
    /// spacing apart, so waiting longer only delays what it was going to do
    /// anyway, and delay is exactly what a sculptor sees.
    ///
    /// Zero for any dragging verb that replays from its anchor, on any
    /// representation.
    ///
    /// **The representation is the wrong question for a replayed gesture, and
    /// asking it first is what kept a field's Move from being seen while it
    /// was made.** The threshold exists because a *stamping* segment costs a
    /// re-mesh of everything it touched, and it grows with the gesture. A
    /// replayed one does not: the whole drag is laid down from its anchor
    /// every time, so the work is the same on the first segment and the
    /// fortieth, and waiting buys nothing while costing exactly what a
    /// sculptor sees.
    ///
    /// A field's Move and Puxar have replayed from their anchor since #99, and
    /// a field's Move additionally has a live transaction armed for it before
    /// the first segment — `open_live_gesture` routes `Mover` into
    /// `arm_live_move` — whose entire purpose is to draw the drag while the
    /// pointer is down. None of that machinery ran until the pointer came up,
    /// because this returned `STAMPS_PER_SEGMENT` for every field gesture
    /// before it ever asked whether the gesture replays.
    ///
    /// At the default flow and a brush of 0.858 that threshold is 1.03 world
    /// units — most of the way across a unit sphere — so an ordinary drag
    /// ended before one segment fired. Reported from a session: "on our app we
    /// only see the effect of the move brush (in sdf) after the stroke
    /// finishes". Mesh mode was correct, which is exactly the asymmetry this
    /// ordering produced.
    fn stamps_between_segments(&self, tool: ToolKind) -> f32 {
        if self.replays_from_the_anchor(tool) {
            return 0.0;
        }
        if self.model.active_representation() != Representation::Mesh {
            return STAMPS_PER_SEGMENT;
        }
        1.0
    }

    /// Selects a tool, bringing its remembered brush with it.
    fn select(&mut self, tool: ToolKind) {
        // A choice, so there is no longer a chosen tool waiting for a layer
        // that carries it.
        self.substitution = None;
        self.store_brush();
        if self.tool.set_if_changed(tool) {
            let (row, index) = self.slot(tool);
            self.brush.set(self.brushes[row][index]);
            self.refresh_tool_status();
        }
    }

    /// The tool this stroke is actually using, which a held modifier can
    /// substitute.
    fn stroking_tool(&self) -> ToolKind {
        self.modifiers.tool(*self.tool.get())
    }

    /// The brush this stroke is actually using.
    fn stroking_brush(&self) -> BrushSettings {
        BrushSettings {
            invert: self.modifiers.invert,
            ..*self.brush.get()
        }
    }

    /// Whether a segment carries the gesture from its anchor rather than only
    /// what is new since the last one.
    ///
    /// A dragging verb on a mesh does. Grab anchors on the first stamp and
    /// carries that region by the motion that follows, so a segment holding
    /// only the newest samples is a *second* grab anchoring where the first
    /// stopped — measured against Blender, two anchors sharing one drag reach
    /// nearly twice as far and move less.
    ///
    /// Replaying is also what lets the drag be seen while it happens. The
    /// model takes back what the last segment did and lays the gesture down
    /// again from its anchor, so the surface follows the pointer rather than
    /// appearing only when the pointer comes up. It stays one undo: every
    /// segment replaces the last, and only the release banks anything.
    fn replays_from_the_anchor(&self, tool: ToolKind) -> bool {
        if !tool.is_path_driven() {
            return false;
        }
        match self.model.active_representation() {
            Representation::Mesh => true,
            // A field's snakehook authors a *curve*, and a curve is the whole
            // path or it is a trail of short ones. Given only what is new, each
            // segment restarted the taper and the tendril came out a string of
            // beads — measured, the thickness along a curving pull wobbled by
            // 0.210 against 0.137 for the same pull delivered whole, and that
            // 0.137 is the taper itself. The model grows the one curve rather
            // than adding another, so replaying is free of the stacking that
            // makes it wrong elsewhere.
            //
            // A field's Move replays for a different reason, and needs none of
            // the take-back above. When a live transaction is open the anchor
            // lives in the transaction; when one is not — a mirror that could
            // not be pointed, or a caller that never opened one — the drag
            // falls to `clay_layer_move_surface`, which takes the segment's
            // FIRST sample as the centre of the grab. That call coalesces
            // successive grabs only while the centre and radius repeat
            // exactly, so a segment starting where the last one stopped is a
            // new grab every time: measured on the pinned engine, six segments
            // sent that way leave a deformer chain of 6 where six sent from a
            // fixed anchor leave 1, and the chain's Lipschitz bound
            // multiplies. Replaying costs nothing here — the verb reads only
            // the first and last sample — and the live path is unaffected,
            // since it takes its anchor from the first segment either way.
            Representation::Sdf => matches!(tool, ToolKind::Puxar | ToolKind::Mover),
            Representation::Voxel => false,
            // The mesh's answer, because the drag IS the mesh's drag: a stamp
            // on a hierarchy is the fixed sculptor's stamp over the bound
            // level's own mesh. And the take-back the replay needs is exact
            // there — a layered gesture's cancel restores recorded `before`
            // values rather than reconstructing them — so a segment can be
            // undone and the gesture laid down again from its anchor.
            Representation::Multires => true,
        }
    }

    /// Sends the part of the gesture the model has not seen yet.
    ///
    /// The stroke stays open: this is a piece of it, not the end of it.
    fn apply_segment(&mut self) -> Result<(), ModelError> {
        let tool = self.stroking_tool();
        // Asked before the stroke is borrowed: it reads the model.
        let replay = self.replays_from_the_anchor(tool);
        let Some(stroke) = self.stroke.as_ref() else {
            return Ok(());
        };
        let pending = if replay {
            stroke.whole()
        } else {
            stroke.pending(tool)
        };
        // One sample is a whole instruction for a stamping tool and none at
        // all for a dragging one, which needs a start and an end.
        let enough = if tool.is_path_driven() { 2 } else { 1 };
        if pending.len() < enough {
            return Ok(());
        }
        let outcome =
            self.model
                .apply_stroke(tool, self.stroking_brush(), pending, *self.symmetry.get());

        // Marked applied whether or not the engine accepted them. Re-sending a
        // segment the engine already refused would refuse again every frame,
        // and re-sending one it accepted would deposit it twice.
        if let Some(stroke) = self.stroke.as_mut() {
            stroke.mark_applied();
        }

        self.record(tool, outcome?);
        Ok(())
    }

    fn commit_stroke(&mut self) -> Result<(), ModelError> {
        if self.stroke.is_none() {
            return Ok(());
        }
        // The tail of the gesture, then the count is banked. Banking is owed
        // even if the tail failed, or the entries already deposited would be
        // undoable only one segment at a time.
        let applied = self.apply_segment();
        self.stroke = None;
        // Installed before the count is banked: the commit is the only thing
        // a live gesture writes, so its entries are the ones an undo spends.
        let closed = self.close_live_gesture();
        // The gesture is over: what was previewed becomes the edit, and one
        // undo takes the whole drag back however many segments drew it.
        self.model.end_gesture();
        // And the keys that were held with it stop meaning anything.
        self.modifiers = clayspace_model::StrokeModifiers::default();
        self.close_gesture();
        applied.and(closed)
    }

    /// Installs what a live gesture previewed.
    ///
    /// What it recorded is not counted here. The gesture is measured from the
    /// depth it opened at, which already includes whatever the commit wrote —
    /// see [`SculptViewModel::gesture_floor`].
    fn close_live_gesture(&mut self) -> Result<(), ModelError> {
        if !std::mem::take(&mut self.live) {
            return Ok(());
        }
        let _recorded = self.model.close_live_gesture()?;
        Ok(())
    }

    /// Takes back the gesture in progress, and says that it did.
    fn cancel_stroke(&mut self) -> Result<(), ModelError> {
        // Nothing open is nothing to cancel, and it stops here: the model is
        // not told a gesture ended, because a cancel an agent may safely
        // repeat must not settle the document a second time, and the revert
        // below would be spending history belonging to whatever came before.
        let Some(floor) = self.gesture_floor.take() else {
            self.report_cancel(false);
            return Ok(());
        };
        // A live stroke has already put clay down, so cancelling has to take
        // it back rather than merely stop. Abandoning it would leave the
        // sculptor with half a stroke they explicitly said they did not want.
        self.stroke = None;
        // A live gesture wrote nothing, so it is dropped rather than reverted.
        // What opening it did record — pointing the layer's mirror, which
        // happens before the transaction begins — sits above the floor, so the
        // revert below already reaches it and the count handed back here would
        // only be counting it twice.
        if std::mem::take(&mut self.live) {
            let _ = self.model.discard_live_gesture();
        }
        // Closed before the revert, so the preview it was holding is banked
        // and then taken back with everything else rather than being left on
        // the surface.
        self.model.end_gesture();
        self.modifiers = clayspace_model::StrokeModifiers::default();
        self.abandon_gesture(floor)
    }

    /// Reverts what the gesture in progress put into the document, down to the
    /// depth it started from and not one entry further.
    ///
    /// `floor` rather than the entries the segments were counted as producing:
    /// see [`SculptViewModel::gesture_floor`] for why those two numbers are
    /// not the same on a mesh, and what spending the wrong one destroyed.
    fn abandon_gesture(&mut self, floor: usize) -> Result<(), ModelError> {
        let owed = self.model.history().depth.saturating_sub(floor);
        let mut reverted = 0;
        for _ in 0..owed {
            if !self.model.undo()? {
                break;
            }
            reverted += 1;
        }
        if reverted > 0 {
            // Not pushed onto the redo stack: a cancelled gesture is not an
            // action the sculptor can ask for back.
            self.stats.set(self.model.stats());
            self.pending_remesh.update(|pending| *pending += 1);
        }
        self.report_cancel(reverted > 0);
        self.publish_history();
        Ok(())
    }

    /// Says a cancel happened, and whether it had anything to take back.
    ///
    /// No tool name: what the sculptor did was cancel, and naming the brush
    /// they were holding would read as the stroke having landed.
    fn report_cancel(&mut self, changed: bool) {
        self.last_action.set(LastAction {
            tool: None,
            label: "cancel stroke".to_string(),
            changed,
        });
    }

    /// Records an undoable action this ViewModel did not perform.
    ///
    /// Rigging goes through its own ViewModel but shares this history, because
    /// a sculptor has one Cmd+Z and does not care which part of the
    /// application produced the thing they want back. The armature edits group
    /// themselves into one engine entry each, so the count is one; passing it
    /// explicitly keeps the accounting in the one place that understands it.
    ///
    /// Clears the redo stack for the same reason any new action does: history
    /// branched, and the old forward path is no longer reachable.
    pub fn record_external_action(&mut self, label: impl Into<String>, entries: usize) {
        if entries == 0 {
            return;
        }
        self.undo_stack.push(BankedAction {
            label: label.into(),
            entries,
        });
        self.redo_stack.clear();
        self.publish_history();
    }

    /// Banks the gesture's entries as one undoable action.
    ///
    /// Measured from the depth the gesture opened at rather than counted from
    /// the segments, for the reason a cancel is — the two numbers are not the
    /// same, and see [`SculptViewModel::gesture_floor`] for what spending the
    /// wrong one destroyed. A gesture that wrote nothing banks nothing, which
    /// is how a stroke that landed on no clay stops being something to take
    /// back: what the stroke *wrote* decides it, not where it started, so a
    /// dab that deposited a blob off the surface is still one entry.
    fn close_gesture(&mut self) {
        // The gesture is over, so the depth it was measured from goes with it:
        // a cancel arriving after the release has nothing of its own left to
        // take back.
        let Some(floor) = self.gesture_floor.take() else {
            return;
        };
        let entries = self.model.history().depth.saturating_sub(floor);
        if entries > 0 {
            // The tool that made it, as the last segment named it. `record`
            // writes that name on every segment, so by the time a gesture
            // closes it is the tool the clay actually moved under — which is
            // not always the tool on the shelf, since a layer can substitute
            // one. The shelf's own name is the fallback for a gesture that
            // banked entries without any segment reaching `record`.
            let label = match self.last_action.get().label.as_str() {
                "" => self.tool.get().label().to_string(),
                named => named.to_string(),
            };
            self.undo_stack.push(BankedAction { label, entries });
            self.publish_history();
        }
    }

    /// What the interface shows: actions, not engine entries.
    fn publish_history(&mut self) {
        self.history.set(HistoryState {
            can_undo: !self.undo_stack.is_empty(),
            can_redo: !self.redo_stack.is_empty(),
            depth: self.undo_stack.len(),
            redo_depth: self.redo_stack.len(),
        });
    }

    fn record(&mut self, tool: ToolKind, outcome: EditOutcome) {
        self.last_action.set(LastAction {
            tool: Some(tool),
            // Kept as well: the log and the diagnostics read it, and neither
            // has a language to name a tool in.
            label: tool.label().to_string(),
            changed: outcome.changed,
        });

        // An edit that changed nothing adds no history and needs no re-mesh.
        if !outcome.changed {
            return;
        }
        self.pending_remesh
            .update(|pending| *pending += outcome.dirty_bricks);
        // Nothing is banked here. A segment is part of a gesture in progress,
        // and what the whole gesture cost is measured when it closes — see
        // `close_gesture`.
        //
        // Anything new makes the redone future unreachable, which is what
        // every editor does and what the engine does underneath.
        self.redo_stack.clear();
        self.publish_history();
        self.stats.set(self.model.stats());
    }

    /// Reverts the whole of the last action, however many entries it took.
    fn undo_action(&mut self) -> Result<(), ModelError> {
        let Some(action) = self.undo_stack.pop() else {
            self.last_action.set(LastAction {
                tool: None,
                label: "undo".to_string(),
                changed: false,
            });
            return Ok(());
        };
        let mut reverted = 0;
        for _ in 0..action.entries {
            if !self.model.undo()? {
                break;
            }
            reverted += 1;
        }
        if reverted > 0 {
            // The name travels with the count, so a redo can say what it would
            // put back rather than answering "redo".
            self.redo_stack.push(BankedAction {
                label: action.label,
                entries: reverted,
            });
        }
        self.after_history_change("undo", reverted > 0);
        Ok(())
    }

    /// Reapplies the whole of the last undone action.
    fn redo_action(&mut self) -> Result<(), ModelError> {
        let Some(action) = self.redo_stack.pop() else {
            self.last_action.set(LastAction {
                tool: None,
                label: "redo".to_string(),
                changed: false,
            });
            return Ok(());
        };
        let mut redone = 0;
        for _ in 0..action.entries {
            if !self.model.redo()? {
                break;
            }
            redone += 1;
        }
        if redone > 0 {
            self.undo_stack.push(BankedAction {
                label: action.label,
                entries: redone,
            });
        }
        self.after_history_change("redo", redone > 0);
        Ok(())
    }

    fn after_history_change(&mut self, label: &str, moved: bool) {
        self.last_action.set(LastAction {
            tool: None,
            label: label.to_string(),
            changed: moved,
        });
        if moved {
            self.publish_history();
            self.stats.set(self.model.stats());
            // Undo can change any part of the surface, so the viewport rebuilds.
            self.pending_remesh.update(|pending| *pending += 1);
        }
    }
}

impl ActiveStroke {
    fn push(&mut self, position: [f32; 3], pressure: f32) {
        if let Some(previous) = self.samples.last() {
            let step = (0..3)
                .map(|axis| {
                    let d = position[axis] - previous.position[axis];
                    d * d
                })
                .sum::<f32>()
                .sqrt();
            self.travelled += step;
        }
        self.samples.push(GestureSample {
            position,
            pressure: pressure.clamp(0.0, 1.0),
            time: self.next_time,
        });
        // A nominal step. Spacing follows arc length in the engine, so this
        // only orders the samples and drives taper.
        self.next_time += 0.008;
    }

    /// Whether enough of the path is unapplied to be worth sending.
    ///
    /// Paced by the brush's own stamp spacing rather than by sample count or
    /// by a timer. A segment shorter than one stamp gap gives the engine's
    /// stroke engine nothing to space out, and it would deposit at the
    /// segment's start regardless — so a fast machine would lay down more
    /// material than a slow one for the same gesture. Pacing by distance makes
    /// the result depend on the path, which is the only thing the sculptor
    /// controls.
    /// Whether enough of the gesture has happened to be worth sending.
    ///
    /// A stamping segment costs a re-mesh of everything it touched, so it
    /// waits for three stamps' worth of travel — sending one per pointer move
    /// would re-mesh the same neighbourhood over and over.
    ///
    /// A *replayed* one costs a revert and a single stamp, and does not grow
    /// with the gesture: the whole drag is laid down from its anchor every
    /// time, so the work is the same on the first segment and the fortieth.
    /// It waits for nothing, because waiting is exactly what a sculptor sees.
    /// At the default flow and a brush of 0.858 the stamping threshold is 1.03
    /// world units — most of the way across a unit sphere — so a drag reached
    /// its end before a single segment fired and the surface only moved when
    /// the pointer came up.
    fn segment_is_worth_applying(&self, brush: &BrushSettings, stamps: f32) -> bool {
        if self.applied >= self.samples.len() {
            return false;
        }
        stamps <= 0.0 || self.travelled >= stamp_gap(brush) * stamps
    }

    /// The samples not yet sent.
    ///
    /// A dragging tool also gets the last sample it was already sent, because
    /// a displacement needs somewhere to start from — see
    /// [`ToolKind::is_path_driven`]. Re-sending it costs nothing there: the
    /// tool moves the surface from that point, it does not deposit at it.
    fn pending(&self, tool: ToolKind) -> &[GestureSample] {
        let applied = self.applied.min(self.samples.len());
        let from = if tool.is_path_driven() {
            applied.saturating_sub(1)
        } else {
            applied
        };
        &self.samples[from..]
    }

    /// Every sample since the press, which is what a replayed gesture needs.
    fn whole(&self) -> &[GestureSample] {
        &self.samples
    }

    fn mark_applied(&mut self) {
        self.applied = self.samples.len();
        self.travelled = 0.0;
    }
}

/// How many stamps' worth of path each segment carries.
///
/// One was tried, on the reasoning that the smallest segment gives the most
/// responsive feedback. It gives the *worst* result: the engine receives a
/// single sample per call, so its stroke engine has no path to space stamps
/// along and simply deposits one at the start. The stroke came out as a row of
/// separate beads rather than a ridge, and every one of them cost a re-mesh.
///
/// Three is enough path for the engine to interpolate along and cuts the
/// re-meshes by the same factor. It is still far below what an eye reads as
/// lag.
const STAMPS_PER_SEGMENT: f32 = 3.0;

/// How far the brush travels between stamps, in world units.
///
/// Mirrors the engine adapter's mapping of flow onto stroke spacing — flow is
/// spacing, and spacing is a fraction of the footprint's diameter. Kept in the
/// ViewModel because pacing is a matter of when to talk to the model, which is
/// the ViewModel's business, but it has to agree with what the model does or
/// the segments will not line up with the stamps.
fn stamp_gap(brush: &BrushSettings) -> f32 {
    let brush = brush.sanitized();
    let spacing = (1.0 - brush.flow).clamp(0.05, 0.9);
    (spacing * brush.size * 2.0).max(1e-4)
}
