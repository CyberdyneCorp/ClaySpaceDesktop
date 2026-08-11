//! ViewModel behaviour, exercised against a double.
//!
//! No engine, no GPU, no window. That is the point of the Model being a trait:
//! the rules the interface must obey — an unavailable tool refuses before it
//! collects a gesture, a no-op adds no history, reading never schedules a
//! redraw — are checked here in microseconds rather than through a viewport.

use std::cell::RefCell;
use std::rc::Rc;

use clayspace_model::{
    BrushSettings, EditOutcome, GestureSample, HistoryState, ModelError, Representation,
    SceneStats, SculptModel, ToolKind, ViewPresetKind,
};
use clayspace_vm::{Axis, Command, SculptViewModel, Watcher};

/// What the double was asked to do.
#[derive(Debug, Default)]
struct Recorded {
    strokes: Vec<(ToolKind, Vec<GestureSample>, [bool; 3], BrushSettings)>,
    undos: usize,
    redos: usize,
}

/// A Model that records its calls and answers however a test needs.
struct FakeModel {
    recorded: Rc<RefCell<Recorded>>,
    representation: Representation,
    editable: bool,
    /// What the next stroke reports.
    outcome: EditOutcome,
    history: HistoryState,
    stats: SceneStats,
}

impl FakeModel {
    fn new(recorded: Rc<RefCell<Recorded>>) -> Self {
        Self {
            recorded,
            representation: Representation::Sdf,
            editable: true,
            outcome: EditOutcome {
                changed: true,
                dirty_bricks: 8,
            },
            history: HistoryState {
                can_undo: true,
                can_redo: false,
                depth: 1,
                redo_depth: 0,
            },
            stats: SceneStats {
                triangles: 100,
                vertices: 60,
                objects: 1,
            },
        }
    }
}

impl SculptModel for FakeModel {
    fn active_representation(&self) -> Representation {
        self.representation
    }

    fn active_layer_editable(&self) -> bool {
        self.editable
    }

    fn apply_stroke(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        self.recorded
            .borrow_mut()
            .strokes
            .push((tool, samples.to_vec(), symmetry, brush));
        Ok(self.outcome)
    }

    fn pick(&self, _origin: [f32; 3], _direction: [f32; 3]) -> Option<[f32; 3]> {
        Some([0.0, 0.0, 1.0])
    }

    fn undo(&mut self) -> Result<bool, ModelError> {
        self.recorded.borrow_mut().undos += 1;
        Ok(self.history.can_undo)
    }

    fn redo(&mut self) -> Result<bool, ModelError> {
        self.recorded.borrow_mut().redos += 1;
        Ok(self.history.can_redo)
    }

    fn history(&self) -> HistoryState {
        self.history
    }

    fn stats(&self) -> SceneStats {
        self.stats
    }

    fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        Some(([-1.0; 3], [1.0; 3]))
    }
}

/// A ViewModel over a double, plus the record of what the double saw.
fn fixture() -> (SculptViewModel, Rc<RefCell<Recorded>>) {
    let recorded = Rc::new(RefCell::new(Recorded::default()));
    let model = FakeModel::new(recorded.clone());
    (SculptViewModel::new(Box::new(model)), recorded)
}

/// A ViewModel whose model is configured before it is handed over.
fn fixture_with(configure: impl FnOnce(&mut FakeModel)) -> (SculptViewModel, Rc<RefCell<Recorded>>) {
    let recorded = Rc::new(RefCell::new(Recorded::default()));
    let mut model = FakeModel::new(recorded.clone());
    configure(&mut model);
    (SculptViewModel::new(Box::new(model)), recorded)
}

fn draw(vm: &mut SculptViewModel, points: &[[f32; 3]]) -> Result<(), ModelError> {
    let (first, rest) = points.split_first().expect("a stroke needs a point");
    vm.dispatch(Command::BeginStroke {
        position: *first,
        pressure: 1.0,
    })?;
    for point in rest {
        vm.dispatch(Command::ContinueStroke {
            position: *point,
            pressure: 1.0,
        })?;
    }
    vm.dispatch(Command::EndStroke)
}

// -- strokes -----------------------------------------------------------------

#[test]
fn a_whole_stroke_reaches_the_model_as_one_edit() {
    let (mut vm, recorded) = fixture();
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0], [0.2, 0.0, 0.0]]).expect("stroke");

    let recorded = recorded.borrow();
    assert_eq!(
        recorded.strokes.len(),
        1,
        "a stroke must arrive as one edit, not one per sample"
    );
    assert_eq!(recorded.strokes[0].1.len(), 3, "every sample must be carried");
}

#[test]
fn samples_carry_increasing_time_and_clamped_pressure() {
    let (mut vm, recorded) = fixture();
    vm.dispatch(Command::BeginStroke {
        position: [0.0; 3],
        pressure: 4.0,
    })
    .expect("begin");
    vm.dispatch(Command::ContinueStroke {
        position: [0.1, 0.0, 0.0],
        pressure: -1.0,
    })
    .expect("continue");
    vm.dispatch(Command::EndStroke).expect("end");

    let recorded = recorded.borrow();
    let samples = &recorded.strokes[0].1;
    assert!(
        samples[0].pressure <= 1.0 && samples[1].pressure >= 0.0,
        "pressure must be clamped to what the engine expects"
    );
    assert!(
        samples[1].time > samples[0].time,
        "samples must be ordered in time"
    );
}

#[test]
fn a_cancelled_stroke_never_reaches_the_model() {
    let (mut vm, recorded) = fixture();
    vm.dispatch(Command::BeginStroke {
        position: [0.0; 3],
        pressure: 1.0,
    })
    .expect("begin");
    vm.dispatch(Command::ContinueStroke {
        position: [0.1, 0.0, 0.0],
        pressure: 1.0,
    })
    .expect("continue");
    vm.dispatch(Command::CancelStroke).expect("cancel");
    vm.dispatch(Command::EndStroke).expect("end after cancel");

    assert!(
        recorded.borrow().strokes.is_empty(),
        "a cancelled gesture was still applied"
    );
    assert!(!vm.is_stroking());
}

#[test]
fn ending_a_stroke_that_never_began_does_nothing() {
    let (mut vm, recorded) = fixture();
    vm.dispatch(Command::EndStroke).expect("end without begin");
    assert!(recorded.borrow().strokes.is_empty());
}

#[test]
fn symmetry_reaches_the_model_as_set() {
    let (mut vm, recorded) = fixture();
    // X is on by default; turn on Z as well.
    vm.dispatch(Command::ToggleSymmetry(Axis::Z)).expect("symmetry");
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("stroke");

    assert_eq!(recorded.borrow().strokes[0].2, [true, false, true]);
}

// -- tool availability -------------------------------------------------------

#[test]
fn an_unavailable_tool_refuses_before_collecting_a_gesture() {
    // Raspar is voxel-side; the active layer is SDF.
    let (mut vm, recorded) = fixture();
    vm.dispatch(Command::SelectTool(ToolKind::Raspar)).expect("select");

    let error = vm
        .dispatch(Command::BeginStroke {
            position: [0.0; 3],
            pressure: 1.0,
        })
        .expect_err("a voxel verb on an SDF layer must be refused");

    assert!(
        error.to_string().contains("voxel"),
        "the refusal must say what the tool needs: {error}"
    );
    assert!(!vm.is_stroking(), "a refused stroke must not start collecting");
    assert!(recorded.borrow().strokes.is_empty());
}

#[test]
fn the_status_explains_why_a_tool_is_unusable() {
    let (mut vm, _) = fixture();
    assert!(vm.tool_status().get().is_none(), "Padrão works on an SDF layer");

    vm.dispatch(Command::SelectTool(ToolKind::Preencher)).expect("select");
    let status = vm.tool_status().get().clone();
    assert!(
        status.is_some_and(|s| s.contains("voxel")),
        "an unavailable tool must state its reason for the interface to show"
    );
}

#[test]
fn a_protected_layer_refuses_every_tool() {
    let (mut vm, _) = fixture_with(|model| model.editable = false);
    let error = vm
        .dispatch(Command::BeginStroke {
            position: [0.0; 3],
            pressure: 1.0,
        })
        .expect_err("a locked layer accepts no edit");
    assert!(error.to_string().contains("locked"), "{error}");
}

#[test]
fn a_voxel_layer_accepts_voxel_tools() {
    let (mut vm, recorded) = fixture_with(|model| model.representation = Representation::Voxel);
    vm.dispatch(Command::SelectTool(ToolKind::Raspar)).expect("select");
    assert!(vm.tool_status().get().is_none(), "scrape belongs on a voxel layer");
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("stroke");
    assert_eq!(recorded.borrow().strokes.len(), 1);
}

// -- brush settings ----------------------------------------------------------

#[test]
fn brush_settings_are_remembered_per_tool() {
    let (mut vm, _) = fixture();
    vm.dispatch(Command::SetBrushSize(0.5)).expect("size");

    vm.dispatch(Command::SelectTool(ToolKind::Suavizar)).expect("select");
    vm.dispatch(Command::SetBrushSize(0.1)).expect("size");
    assert_eq!(vm.brush().get().size, 0.1);

    vm.dispatch(Command::SelectTool(ToolKind::Padrao)).expect("select");
    assert_eq!(
        vm.brush().get().size,
        0.5,
        "switching away and back must return the settings the user left"
    );
}

#[test]
fn brush_settings_are_clamped_rather_than_refused() {
    let (mut vm, _) = fixture();
    vm.dispatch(Command::SetBrushSize(-3.0)).expect("size");
    assert!(
        vm.brush().get().size > 0.0,
        "a non-positive radius would be rejected by the engine, so it is clamped here"
    );
}

// -- history -----------------------------------------------------------------

#[test]
fn an_edit_that_changed_nothing_adds_no_history() {
    let (mut vm, _) = fixture_with(|model| model.outcome = EditOutcome::NOTHING);
    let mut watcher = Watcher::new();
    watcher.accept(vm.history());

    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("stroke");

    assert!(
        !watcher.take_change(vm.history()),
        "a verb that changed nothing must not create a history entry"
    );
    assert!(
        !vm.last_action().get().changed,
        "the interface should be able to tell that nothing happened"
    );
    assert_eq!(
        *vm.pending_remesh().get(),
        0,
        "nothing changed, so there is nothing to re-mesh"
    );
}

#[test]
fn a_live_edit_schedules_a_remesh_bounded_by_what_it_dirtied() {
    let (mut vm, _) = fixture();
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("stroke");
    assert_eq!(
        *vm.pending_remesh().get(),
        8,
        "the re-mesh must be bounded by the bricks the edit dirtied"
    );

    vm.acknowledge_remesh();
    assert_eq!(*vm.pending_remesh().get(), 0);
}

#[test]
fn undo_and_redo_reach_the_model() {
    let (mut vm, recorded) = fixture();
    vm.dispatch(Command::Undo).expect("undo");
    vm.dispatch(Command::Redo).expect("redo");
    let recorded = recorded.borrow();
    assert_eq!((recorded.undos, recorded.redos), (1, 1));
}

#[test]
fn an_undo_with_nothing_to_undo_changes_nothing() {
    let (mut vm, _) = fixture_with(|model| {
        model.history = HistoryState {
            can_undo: false,
            can_redo: false,
            depth: 0,
            redo_depth: 0,
        }
    });
    let mut watcher = Watcher::new();
    watcher.accept(vm.history());

    vm.dispatch(Command::Undo).expect("undo");
    assert!(
        !watcher.take_change(vm.history()),
        "an undo that did nothing must not report a change"
    );
}

// -- view state --------------------------------------------------------------

#[test]
fn view_commands_never_touch_history_or_stats() {
    let (mut vm, _) = fixture();
    let (mut history, mut stats) = (Watcher::new(), Watcher::new());
    history.accept(vm.history());
    stats.accept(vm.stats());

    for command in [
        Command::SetViewPreset(ViewPresetKind::Front),
        Command::FrameAll,
        Command::NextMaterial,
        Command::ToggleGrid,
        Command::SelectTool(ToolKind::Suavizar),
        Command::SetBrushSize(0.2),
    ] {
        vm.dispatch(command).expect("view command");
    }

    assert!(!history.take_change(vm.history()), "a view change entered the history");
    assert!(!stats.take_change(vm.stats()), "a view change altered the statistics");
}

#[test]
fn the_view_preset_and_grid_are_observable() {
    let (mut vm, _) = fixture();
    let mut watcher = Watcher::new();
    watcher.accept(vm.view_preset());

    vm.dispatch(Command::SetViewPreset(ViewPresetKind::Top)).expect("preset");
    assert!(watcher.take_change(vm.view_preset()));
    assert_eq!(*vm.view_preset().get(), ViewPresetKind::Top);

    let before = *vm.grid().get();
    vm.dispatch(Command::ToggleGrid).expect("grid");
    assert_ne!(*vm.grid().get(), before);
}

#[test]
fn setting_the_preset_already_active_schedules_no_redraw() {
    let (mut vm, _) = fixture();
    vm.dispatch(Command::SetViewPreset(ViewPresetKind::Top)).expect("preset");
    let mut watcher = Watcher::new();
    watcher.accept(vm.view_preset());

    vm.dispatch(Command::SetViewPreset(ViewPresetKind::Top)).expect("preset again");
    assert!(
        !watcher.take_change(vm.view_preset()),
        "an immediate-mode interface sets controls to their current value constantly; \
         that must not schedule a redraw"
    );
}

#[test]
fn an_idle_viewmodel_reports_no_changes() {
    let (vm, _) = fixture();
    let mut watchers = (Watcher::new(), Watcher::new(), Watcher::new(), Watcher::new());
    watchers.0.accept(vm.tool());
    watchers.1.accept(vm.brush());
    watchers.2.accept(vm.history());
    watchers.3.accept(vm.stats());

    // Reading is what an interface does every frame.
    for _ in 0..100 {
        let _ = vm.tool().get();
        let _ = vm.brush().get();
        let _ = vm.history().get();
        let _ = vm.stats().get();
        let _ = vm.bounds();
        let _ = vm.pick([0.0, 0.0, -1.0], [0.0, 0.0, 1.0]);
    }

    assert!(!watchers.0.take_change(vm.tool()));
    assert!(!watchers.1.take_change(vm.brush()));
    assert!(!watchers.2.take_change(vm.history()));
    assert!(
        !watchers.3.take_change(vm.stats()),
        "reading state marked it dirty, which would redraw an idle application forever"
    );
}
