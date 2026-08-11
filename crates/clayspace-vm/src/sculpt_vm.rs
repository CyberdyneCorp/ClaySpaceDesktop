//! The sculpting ViewModel.
//!
//! Holds what the interface draws and turns commands into Model calls. It
//! depends on no interface library and no renderer, so every behaviour here is
//! exercised in a test with no window and no GPU.

use clayspace_model::{
    BrushSettings, EditOutcome, GestureSample, HistoryState, ModelError, SceneStats,
    SculptModel, ToolKind, ViewPresetKind,
};

use crate::command::{Axis, Command};
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
    pub label: String,
    /// False when the engine reported the edit changed nothing.
    pub changed: bool,
}

/// Everything the sculpting interface reads.
pub struct SculptViewModel {
    model: Box<dyn SculptModel>,

    tool: Observable<ToolKind>,
    /// Settings are held per tool: switching away and back returns what the
    /// user left, not a default.
    brushes: [BrushSettings; ToolKind::ALL.len()],
    brush: Observable<BrushSettings>,
    symmetry: Observable<[bool; 3]>,
    view_preset: Observable<ViewPresetKind>,
    grid: Observable<bool>,

    history: Observable<HistoryState>,
    stats: Observable<SceneStats>,
    /// Why the active tool cannot be used, when it cannot.
    tool_status: Observable<Option<String>>,
    last_action: Observable<LastAction>,
    /// Set when an edit dirtied bricks the viewport has not re-meshed yet.
    pending_remesh: Observable<usize>,

    stroke: Option<ActiveStroke>,

    /// How many model-level entries each user-visible action produced, newest
    /// last.
    ///
    /// A live stroke reaches the document as several calls so the clay moves
    /// under the pointer, and each is its own entry in the document's history.
    /// A sculptor did one thing, though, and expects one undo to remove it —
    /// forty presses to erase one stroke is not undo, it is punishment. The
    /// engine's own undo grouping does not collapse them (measured: three
    /// grouped strokes left seven entries, and undoing twice reverted none),
    /// so the count is kept here and `Undo` spends it all at once.
    undo_stack: Vec<usize>,
    /// The same counts for actions that have been undone.
    redo_stack: Vec<usize>,
    /// Entries the gesture in progress has produced so far.
    gesture_entries: usize,
    /// Entries the call being recorded produced, as counted from the model.
    pending_entries: usize,
}

impl SculptViewModel {
    pub fn new(model: Box<dyn SculptModel>) -> Self {
        let stats = model.stats();
        // Empty, not the model's. The engine's history counts building the
        // starting form, which is not something the user did and must not be
        // something they can undo.
        let history = HistoryState::default();
        let mut vm = Self {
            model,
            tool: Observable::new(ToolKind::Padrao),
            brushes: [BrushSettings::default(); ToolKind::ALL.len()],
            brush: Observable::new(BrushSettings::default()),
            // Off, matching the document the engine adapter builds. These two
            // are separate pieces of state and they must not start out
            // disagreeing: the ViewModel is what the options bar shows, and a
            // bar reading "X on" over a document with no mirror is a lie
            // before the user has touched anything.
            symmetry: Observable::new([false, false, false]),
            view_preset: Observable::new(ViewPresetKind::Perspective),
            grid: Observable::new(true),
            history: Observable::new(history),
            stats: Observable::new(stats),
            tool_status: Observable::new(None),
            last_action: Observable::new(LastAction::default()),
            pending_remesh: Observable::new(0),
            stroke: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            gesture_entries: 0,
            pending_entries: 1,
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

    pub fn symmetry(&self) -> &Observable<[bool; 3]> {
        &self.symmetry
    }

    pub fn view_preset(&self) -> &Observable<ViewPresetKind> {
        &self.view_preset
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
        self.gesture_entries = 0;
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
                self.store_brush();
                if self.tool.set_if_changed(tool) {
                    let stored = self.brushes[Self::index(tool)];
                    self.brush.set(stored);
                    self.refresh_tool_status();
                }
            }
            Command::SetBrushSize(size) => self.edit_brush(|b| b.size = size),
            Command::SetBrushIntensity(value) => self.edit_brush(|b| b.intensity = value),
            Command::SetBrushFlow(value) => self.edit_brush(|b| b.flow = value),
            Command::SetBrushNoise(value) => self.edit_brush(|b| b.shaping.noise = value),
            Command::SetBrushFalloff(falloff) => {
                self.edit_brush(|b| b.shaping.falloff = falloff)
            }
            Command::SetBrushAccumulate(on) => self.edit_brush(|b| b.shaping.accumulate = on),
            Command::SetBrushSmoothing(value) => {
                self.edit_brush(|b| b.shaping.smoothing = value)
            }

            // Scene and layer commands are the SceneViewModel's; the sculpting
            // ViewModel ignores them rather than half-handling them.
            Command::SelectLayer(_)
            | Command::SetLayerVisible(..)
            | Command::AddLayer
            | Command::RemoveLayer(_) => {}
            Command::ToggleSymmetry(axis) => {
                let index = match axis {
                    Axis::X => 0,
                    Axis::Y => 1,
                    Axis::Z => 2,
                };
                self.symmetry.update(|axes| axes[index] = !axes[index]);
            }

            Command::BeginStroke { position, pressure } => {
                // Refuse before collecting anything, so an unavailable tool
                // cannot accumulate a gesture it will never apply.
                self.ensure_tool_available()?;
                let tool_is_region = self.tool.get().is_region_based();
                self.gesture_entries = 0;
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
                let tool = *self.tool.get();
                let Some(stroke) = self.stroke.as_mut() else {
                    return Ok(());
                };
                stroke.push(position, pressure);
                let brush = *self.brush.get();
                // A region tool is applied once, when the gesture is complete.
                // Segmenting it stacks a replacement per segment and the
                // result crumbles.
                if !tool.is_region_based() && stroke.segment_is_worth_applying(&brush) {
                    return self.apply_segment();
                }
            }
            Command::EndStroke => return self.commit_stroke(),
            Command::CancelStroke => {
                // A live stroke has already put clay down, so cancelling has
                // to take it back rather than merely stop. Abandoning it would
                // leave the sculptor with half a stroke they explicitly said
                // they did not want.
                self.stroke = None;
                return self.abandon_gesture();
            }

            Command::Undo => return self.undo_action(),
            Command::Redo => return self.redo_action(),

            Command::SetViewPreset(preset) => {
                self.view_preset.set_if_changed(preset);
            }
            Command::ToggleGrid => {
                let current = *self.grid.get();
                self.grid.set(!current);
            }
            // Framing and material are the renderer's business; the ViewModel
            // records nothing for them because they change no state it owns.
            Command::FrameAll | Command::NextMaterial => {}
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

    fn store_brush(&mut self) {
        let index = Self::index(*self.tool.get());
        self.brushes[index] = *self.brush.get();
    }

    fn edit_brush(&mut self, change: impl FnOnce(&mut BrushSettings)) {
        let mut settings = *self.brush.get();
        change(&mut settings);
        let settings = settings.sanitized();
        if self.brush.set_if_changed(settings) {
            self.brushes[Self::index(*self.tool.get())] = settings;
        }
    }

    fn refresh_tool_status(&mut self) {
        let status = self
            .tool
            .get()
            .availability(
                self.model.active_representation(),
                self.model.active_layer_editable(),
            )
            .err()
            .map(|why| why.to_string());
        self.tool_status.set_if_changed(status);
    }

    fn ensure_tool_available(&mut self) -> Result<(), ModelError> {
        self.refresh_tool_status();
        let tool = *self.tool.get();
        tool.availability(
            self.model.active_representation(),
            self.model.active_layer_editable(),
        )
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

    /// Sends the part of the gesture the model has not seen yet.
    ///
    /// The stroke stays open: this is a piece of it, not the end of it.
    fn apply_segment(&mut self) -> Result<(), ModelError> {
        let Some(stroke) = self.stroke.as_ref() else {
            return Ok(());
        };
        let tool = *self.tool.get();
        let pending = stroke.pending(tool);
        // One sample is a whole instruction for a stamping tool and none at
        // all for a dragging one, which needs a start and an end.
        let enough = if tool.is_path_driven() { 2 } else { 1 };
        if pending.len() < enough {
            return Ok(());
        }
        // Counted from the model rather than assumed to be one. A call can
        // record more than one entry — setting the layer mirror is its own,
        // and it happens inside the first segment that uses a new symmetry.
        // Undoing a gesture has to spend every entry it made, or the parts it
        // misses stay behind.
        let before = self.model.history().depth;
        let outcome = self.model.apply_stroke(
            tool,
            *self.brush.get(),
            pending,
            *self.symmetry.get(),
        );
        let recorded = self.model.history().depth.saturating_sub(before);

        // Marked applied whether or not the engine accepted them. Re-sending a
        // segment the engine already refused would refuse again every frame,
        // and re-sending one it accepted would deposit it twice.
        if let Some(stroke) = self.stroke.as_mut() {
            stroke.mark_applied();
        }
        self.pending_entries = recorded;

        self.record(tool.label(), outcome?);
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
        self.close_gesture();
        applied
    }

    /// Reverts everything the gesture in progress has applied.
    fn abandon_gesture(&mut self) -> Result<(), ModelError> {
        let entries = std::mem::take(&mut self.gesture_entries);
        let mut reverted = 0;
        for _ in 0..entries {
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
        self.publish_history();
        Ok(())
    }

    /// Banks the gesture's entries as one undoable action.
    fn close_gesture(&mut self) {
        let entries = std::mem::take(&mut self.gesture_entries);
        if entries > 0 {
            self.undo_stack.push(entries);
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

    fn record(&mut self, label: &str, outcome: EditOutcome) {
        self.last_action.set(LastAction {
            label: label.to_string(),
            changed: outcome.changed,
        });

        // An edit that changed nothing adds no history and needs no re-mesh.
        if !outcome.changed {
            return;
        }
        self.pending_remesh
            .update(|pending| *pending += outcome.dirty_bricks);
        let entries = std::mem::replace(&mut self.pending_entries, 1).max(1);
        if self.stroke.is_some() {
            // Part of a gesture in progress; it is banked when the gesture
            // closes so the whole thing undoes together.
            self.gesture_entries += entries;
        } else {
            self.undo_stack.push(entries);
        }
        // Anything new makes the redone future unreachable, which is what
        // every editor does and what the engine does underneath.
        self.redo_stack.clear();
        self.publish_history();
        self.stats.set(self.model.stats());
    }

    /// Reverts the whole of the last action, however many entries it took.
    fn undo_action(&mut self) -> Result<(), ModelError> {
        let Some(entries) = self.undo_stack.pop() else {
            self.last_action.set(LastAction {
                label: "undo".to_string(),
                changed: false,
            });
            return Ok(());
        };
        let mut reverted = 0;
        for _ in 0..entries {
            if !self.model.undo()? {
                break;
            }
            reverted += 1;
        }
        if reverted > 0 {
            self.redo_stack.push(reverted);
        }
        self.after_history_change("undo", reverted > 0);
        Ok(())
    }

    /// Reapplies the whole of the last undone action.
    fn redo_action(&mut self) -> Result<(), ModelError> {
        let Some(entries) = self.redo_stack.pop() else {
            self.last_action.set(LastAction {
                label: "redo".to_string(),
                changed: false,
            });
            return Ok(());
        };
        let mut redone = 0;
        for _ in 0..entries {
            if !self.model.redo()? {
                break;
            }
            redone += 1;
        }
        if redone > 0 {
            self.undo_stack.push(redone);
        }
        self.after_history_change("redo", redone > 0);
        Ok(())
    }

    fn after_history_change(&mut self, label: &str, moved: bool) {
        self.last_action.set(LastAction {
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
    fn segment_is_worth_applying(&self, brush: &BrushSettings) -> bool {
        self.applied < self.samples.len()
            && self.travelled >= stamp_gap(brush) * STAMPS_PER_SEGMENT
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
