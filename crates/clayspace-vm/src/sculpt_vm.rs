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
}

impl SculptViewModel {
    pub fn new(model: Box<dyn SculptModel>) -> Self {
        let history = model.history();
        let stats = model.stats();
        let mut vm = Self {
            model,
            tool: Observable::new(ToolKind::Padrao),
            brushes: [BrushSettings::default(); ToolKind::ALL.len()],
            brush: Observable::new(BrushSettings::default()),
            symmetry: Observable::new([true, false, false]),
            view_preset: Observable::new(ViewPresetKind::Perspective),
            grid: Observable::new(true),
            history: Observable::new(history),
            stats: Observable::new(stats),
            tool_status: Observable::new(None),
            last_action: Observable::new(LastAction::default()),
            pending_remesh: Observable::new(0),
            stroke: None,
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
                let mut stroke = ActiveStroke::default();
                stroke.push(position, pressure);
                self.stroke = Some(stroke);
            }
            Command::ContinueStroke { position, pressure } => {
                if let Some(stroke) = self.stroke.as_mut() {
                    stroke.push(position, pressure);
                }
            }
            Command::EndStroke => return self.commit_stroke(),
            Command::CancelStroke => {
                self.stroke = None;
            }

            Command::Undo => {
                let moved = self.model.undo()?;
                self.after_history_change("undo", moved);
            }
            Command::Redo => {
                let moved = self.model.redo()?;
                self.after_history_change("redo", moved);
            }

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
        self.tool
            .get()
            .availability(
                self.model.active_representation(),
                self.model.active_layer_editable(),
            )
            .map_err(ModelError::Unavailable)
    }

    fn commit_stroke(&mut self) -> Result<(), ModelError> {
        let Some(stroke) = self.stroke.take() else {
            return Ok(());
        };
        if stroke.samples.is_empty() {
            return Ok(());
        }

        let tool = *self.tool.get();
        let outcome = self.model.apply_stroke(
            tool,
            *self.brush.get(),
            &stroke.samples,
            *self.symmetry.get(),
        )?;

        self.record(tool.label(), outcome);
        Ok(())
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
        self.history.set(self.model.history());
        self.stats.set(self.model.stats());
    }

    fn after_history_change(&mut self, label: &str, moved: bool) {
        self.last_action.set(LastAction {
            label: label.to_string(),
            changed: moved,
        });
        if moved {
            self.history.set(self.model.history());
            self.stats.set(self.model.stats());
            // Undo can change any part of the surface, so the viewport rebuilds.
            self.pending_remesh.update(|pending| *pending += 1);
        }
    }
}

impl ActiveStroke {
    fn push(&mut self, position: [f32; 3], pressure: f32) {
        self.samples.push(GestureSample {
            position,
            pressure: pressure.clamp(0.0, 1.0),
            time: self.next_time,
        });
        // A nominal step. Spacing follows arc length in the engine, so this
        // only orders the samples and drives taper.
        self.next_time += 0.008;
    }
}
