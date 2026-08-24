//! ViewModel behaviour, exercised against a double.
//!
//! No engine, no GPU, no window. That is the point of the Model being a trait:
//! the rules the interface must obey — an unavailable tool refuses before it
//! collects a gesture, a no-op adds no history, reading never schedules a
//! redraw — are checked here in microseconds rather than through a viewport.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use clayspace_model::{
    BrushSettings, EditOutcome, GestureSample, HistoryState, ModelError, Representation,
    SceneStats, SculptModel, StrokeModifiers, ToolKind, ViewPresetKind,
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
    /// Shared, because the layer the ViewModel is looking at can change under
    /// it and a test has to be able to say so after construction — the model
    /// is boxed into the ViewModel and unreachable from outside otherwise.
    representation: Rc<Cell<Representation>>,
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
            representation: Rc::new(Cell::new(Representation::Sdf)),
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
                detail: clayspace_model::Detail::Full,
            },
        }
    }
}

impl SculptModel for FakeModel {
    fn active_representation(&self) -> Representation {
        self.representation.get()
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
        if self.outcome.changed {
            // An edit that changed something is an entry, which is what the
            // ViewModel counts to know how far an undo has to reach.
            self.history.depth += 1;
            self.history.can_undo = true;
            self.history.redo_depth = 0;
            self.history.can_redo = false;
        }
        Ok(self.outcome)
    }

    fn pick(&self, _origin: [f32; 3], _direction: [f32; 3]) -> Option<[f32; 3]> {
        Some([0.0, 0.0, 1.0])
    }

    // A real history, not a fixed answer. The ViewModel now spends as many
    // model-level undos as the action cost, so a double that always says the
    // same thing cannot tell a correct implementation from one that stops
    // after the first step.
    fn undo(&mut self) -> Result<bool, ModelError> {
        self.recorded.borrow_mut().undos += 1;
        if !self.history.can_undo {
            return Ok(false);
        }
        self.history.depth -= 1;
        self.history.redo_depth += 1;
        self.history.can_undo = self.history.depth > 0;
        self.history.can_redo = true;
        Ok(true)
    }

    fn redo(&mut self) -> Result<bool, ModelError> {
        self.recorded.borrow_mut().redos += 1;
        if !self.history.can_redo {
            return Ok(false);
        }
        self.history.redo_depth -= 1;
        self.history.depth += 1;
        self.history.can_redo = self.history.redo_depth > 0;
        self.history.can_undo = true;
        Ok(true)
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
fn fixture_with(
    configure: impl FnOnce(&mut FakeModel),
) -> (SculptViewModel, Rc<RefCell<Recorded>>) {
    let recorded = Rc::new(RefCell::new(Recorded::default()));
    let mut model = FakeModel::new(recorded.clone());
    configure(&mut model);
    (SculptViewModel::new(Box::new(model)), recorded)
}

/// A fixture whose active representation a test can change afterwards, which
/// is what a layer change looks like from the ViewModel's side.
fn fixture_with_layer_changes() -> (SculptViewModel, Rc<Cell<Representation>>) {
    let recorded = Rc::new(RefCell::new(Recorded::default()));
    let model = FakeModel::new(recorded);
    let representation = model.representation.clone();
    (SculptViewModel::new(Box::new(model)), representation)
}

fn draw(vm: &mut SculptViewModel, points: &[[f32; 3]]) -> Result<(), ModelError> {
    let (first, rest) = points.split_first().expect("a stroke needs a point");
    vm.dispatch(Command::BeginStroke {
        position: *first,
        pressure: 1.0,
        modifiers: Default::default(),
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
fn a_stroke_reaches_the_model_as_it_is_drawn() {
    // The sculptor watches the clay move under the pointer, so the gesture is
    // sent in pieces rather than held until the release. What must not change
    // is that every sample gets there, exactly once and in order.
    let (mut vm, recorded) = fixture();
    let path = [[0.0; 3], [0.1, 0.0, 0.0], [0.2, 0.0, 0.0]];
    draw(&mut vm, &path).expect("stroke");

    let recorded = recorded.borrow();
    assert!(
        recorded.strokes.len() > 1,
        "the whole gesture arrived as one edit, so nothing would have been \
         visible until the pointer came up"
    );

    let carried: Vec<[f32; 3]> = recorded
        .strokes
        .iter()
        .flat_map(|stroke| stroke.1.iter().map(|sample| sample.position))
        .collect();
    assert_eq!(
        carried, path,
        "the segments must carry every sample once, in order"
    );
}

#[test]
fn a_stroke_undoes_as_one_action_however_many_segments_it_took() {
    let (mut vm, recorded) = fixture();
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0], [0.2, 0.0, 0.0]]).expect("stroke");
    assert!(recorded.borrow().strokes.len() > 1, "it was not segmented");

    assert_eq!(
        vm.history().get().depth,
        1,
        "a stroke the sculptor drew once must be one thing to undo"
    );
}

#[test]
fn samples_carry_increasing_time_and_clamped_pressure() {
    let (mut vm, recorded) = fixture();
    vm.dispatch(Command::BeginStroke {
        position: [0.0; 3],
        pressure: 4.0,
        modifiers: Default::default(),
    })
    .expect("begin");
    vm.dispatch(Command::ContinueStroke {
        position: [0.1, 0.0, 0.0],
        pressure: -1.0,
    })
    .expect("continue");
    vm.dispatch(Command::EndStroke).expect("end");

    // Flattened across segments: a live stroke arrives in pieces, and the
    // ordering has to hold across the whole gesture rather than within one.
    let recorded = recorded.borrow();
    let samples: Vec<_> = recorded
        .strokes
        .iter()
        .flat_map(|stroke| stroke.1.iter())
        .collect();
    assert!(samples.len() >= 2, "both samples must reach the model");
    assert!(
        samples.iter().all(|s| (0.0..=1.0).contains(&s.pressure)),
        "pressure must be clamped to what the engine expects"
    );
    assert!(
        samples[1].time > samples[0].time,
        "samples must be ordered in time"
    );
}

#[test]
fn a_cancelled_stroke_is_taken_back_off_the_model() {
    let (mut vm, recorded) = fixture();
    vm.dispatch(Command::BeginStroke {
        position: [0.0; 3],
        pressure: 1.0,
        modifiers: Default::default(),
    })
    .expect("begin");
    vm.dispatch(Command::ContinueStroke {
        position: [0.1, 0.0, 0.0],
        pressure: 1.0,
    })
    .expect("continue");
    vm.dispatch(Command::CancelStroke).expect("cancel");
    vm.dispatch(Command::EndStroke).expect("end after cancel");

    // A live stroke has already deposited by the time it is cancelled, so
    // "never applied" is no longer available. What is owed instead is that it
    // is taken back: as many undos as the gesture made entries.
    let recorded = recorded.borrow();
    assert!(
        !recorded.strokes.is_empty(),
        "the gesture never reached the model, so it was not live"
    );
    assert_eq!(
        recorded.undos,
        recorded.strokes.len(),
        "a cancelled gesture must be undone as far as it got"
    );
    assert!(!vm.is_stroking());
    assert!(
        !vm.history().get().can_undo,
        "a cancelled gesture must leave nothing to undo"
    );
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
    // X is on to start with, as the design asks and as the document the
    // engine adapter builds has it. Toggling X turns it *off*, and Z on.
    assert_eq!(*vm.symmetry().get(), [true, false, false]);
    vm.dispatch(Command::ToggleSymmetry(Axis::X))
        .expect("symmetry");
    vm.dispatch(Command::ToggleSymmetry(Axis::Z))
        .expect("symmetry");
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("stroke");

    assert_eq!(recorded.borrow().strokes[0].2, [false, false, true]);
}

// -- tool availability -------------------------------------------------------

#[test]
fn an_unavailable_tool_refuses_before_collecting_a_gesture() {
    // Raspar is voxel-side; the active layer is SDF.
    let (mut vm, recorded) = fixture();
    vm.dispatch(Command::SelectTool(ToolKind::Raspar))
        .expect("select");

    let error = vm
        .dispatch(Command::BeginStroke {
            position: [0.0; 3],
            pressure: 1.0,
            modifiers: Default::default(),
        })
        .expect_err("a voxel verb on an SDF layer must be refused");

    assert!(
        error.to_string().contains("voxel"),
        "the refusal must say what the tool needs: {error}"
    );
    assert!(
        !vm.is_stroking(),
        "a refused stroke must not start collecting"
    );
    assert!(recorded.borrow().strokes.is_empty());
}

#[test]
fn the_status_explains_why_a_tool_is_unusable() {
    let (mut vm, _) = fixture();
    assert!(
        vm.tool_status().get().is_none(),
        "Padrão works on an SDF layer"
    );

    vm.dispatch(Command::SelectTool(ToolKind::Preencher))
        .expect("select");
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
            modifiers: Default::default(),
        })
        .expect_err("a locked layer accepts no edit");
    assert!(error.to_string().contains("locked"), "{error}");
}

#[test]
fn a_voxel_layer_accepts_voxel_tools() {
    let (mut vm, recorded) = fixture_with(|model| model.representation.set(Representation::Voxel));
    vm.dispatch(Command::SelectTool(ToolKind::Raspar))
        .expect("select");
    assert!(
        vm.tool_status().get().is_none(),
        "scrape belongs on a voxel layer"
    );
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("stroke");
    assert!(
        !recorded.borrow().strokes.is_empty(),
        "the stroke never reached the model"
    );
    assert!(
        recorded
            .borrow()
            .strokes
            .iter()
            .all(|s| s.0 == ToolKind::Raspar),
        "every segment must carry the tool that was selected"
    );
}

// -- brush settings ----------------------------------------------------------

#[test]
fn brush_settings_are_remembered_per_tool() {
    let (mut vm, _) = fixture();
    vm.dispatch(Command::SetBrushSize(0.5)).expect("size");

    vm.dispatch(Command::SelectTool(ToolKind::Suavizar))
        .expect("select");
    vm.dispatch(Command::SetBrushSize(0.1)).expect("size");
    assert_eq!(vm.brush().get().size, 0.1);

    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("select");
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
    // Per segment, because a live stroke reaches the model more than once and
    // each piece dirties its own bricks. The bound that matters is that it
    // follows what the edits reported rather than standing for the whole
    // model.
    let segments = 2;
    assert_eq!(
        *vm.pending_remesh().get(),
        8 * segments,
        "the re-mesh must be bounded by the bricks the edits dirtied"
    );

    vm.acknowledge_remesh();
    assert_eq!(*vm.pending_remesh().get(), 0);
}

#[test]
fn undo_and_redo_reach_the_model() {
    // Undo spends whatever the last action cost. With nothing drawn there is
    // no action to spend, so the model is not touched — which is the point:
    // undo is bounded by what this session did, not by what the document
    // happens to have in its history from being built.
    let (mut vm, recorded) = fixture();
    vm.dispatch(Command::Undo).expect("undo");
    assert_eq!(recorded.borrow().undos, 0, "there was nothing to undo");

    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("stroke");
    let segments = recorded.borrow().strokes.len();

    vm.dispatch(Command::Undo).expect("undo");
    assert_eq!(
        recorded.borrow().undos,
        segments,
        "undoing a stroke must spend every entry the stroke made"
    );

    vm.dispatch(Command::Redo).expect("redo");
    assert_eq!(
        recorded.borrow().redos,
        segments,
        "redo must restore the whole action, not one segment of it"
    );
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

    assert!(
        !history.take_change(vm.history()),
        "a view change entered the history"
    );
    assert!(
        !stats.take_change(vm.stats()),
        "a view change altered the statistics"
    );
}

#[test]
fn the_view_preset_and_grid_are_observable() {
    let (mut vm, _) = fixture();
    let mut watcher = Watcher::new();
    watcher.accept(vm.view_preset());

    vm.dispatch(Command::SetViewPreset(ViewPresetKind::Top))
        .expect("preset");
    assert!(watcher.take_change(vm.view_preset()));
    assert_eq!(*vm.view_preset().get(), ViewPresetKind::Top);

    let before = *vm.grid().get();
    vm.dispatch(Command::ToggleGrid).expect("grid");
    assert_ne!(*vm.grid().get(), before);
}

/// The polyframe is a state the interface reads, not an action it fires.
///
/// It has to be observable for the same reason the grid is: the menu shows it
/// checked and the renderer is told each frame, and both read the same value.
/// Off to begin with — a polyframe over a dense mesh is a lot of ink, and it
/// is asked for when a question about density comes up rather than kept on.
#[test]
fn the_polyframe_is_an_observable_state() {
    let (mut vm, _) = fixture();
    assert!(
        !*vm.polyframe().get(),
        "the polyframe starts on, so every mesh layer opens covered in ink"
    );

    let mut watcher = Watcher::new();
    watcher.accept(vm.polyframe());
    vm.dispatch(Command::TogglePolyframe).expect("polyframe");
    assert!(
        watcher.take_change(vm.polyframe()),
        "the change was not seen"
    );
    assert!(*vm.polyframe().get());

    // And back, because a toggle that only goes one way is not a toggle.
    vm.dispatch(Command::TogglePolyframe).expect("polyframe");
    assert!(!*vm.polyframe().get());
}

#[test]
fn setting_the_preset_already_active_schedules_no_redraw() {
    let (mut vm, _) = fixture();
    vm.dispatch(Command::SetViewPreset(ViewPresetKind::Top))
        .expect("preset");
    let mut watcher = Watcher::new();
    watcher.accept(vm.view_preset());

    vm.dispatch(Command::SetViewPreset(ViewPresetKind::Top))
        .expect("preset again");
    assert!(
        !watcher.take_change(vm.view_preset()),
        "an immediate-mode interface sets controls to their current value constantly; \
         that must not schedule a redraw"
    );
}

#[test]
fn an_idle_viewmodel_reports_no_changes() {
    let (vm, _) = fixture();
    let mut watchers = (
        Watcher::new(),
        Watcher::new(),
        Watcher::new(),
        Watcher::new(),
    );
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

/// The shelf's contents follow the active layer, so the tool and the brush
/// settings have to as well.
///
/// Both of these are about a layer change, which is the moment the vocabulary
/// underfoot can change. Before the capability table there was one vocabulary
/// and neither question arose.
mod following_the_active_layer {
    use super::*;

    /// A voxel-only tool cannot survive a move to an SDF layer, and the
    /// alternative to substituting is a tool that silently refuses every
    /// stroke.
    #[test]
    fn a_tool_with_no_verb_on_the_new_layer_is_replaced() {
        let (mut vm, representation) = fixture_with_layer_changes();
        vm.dispatch(Command::SelectTool(ToolKind::Raspar))
            .expect("scrape is a voxel tool");
        assert_eq!(*vm.tool().get(), ToolKind::Raspar);

        representation.set(Representation::Sdf);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
            .expect("select");

        assert_ne!(
            *vm.tool().get(),
            ToolKind::Raspar,
            "scrape has no SDF verb, so it cannot still be the active tool"
        );
        assert!(
            vm.tool().get().exists_on(Representation::Sdf),
            "the replacement must be a tool this layer actually has"
        );
        assert_eq!(
            vm.tool_status().get().as_deref(),
            Some(clayspace_vm::TOOL_SUBSTITUTED),
            "a tool that changed under the user has to say so"
        );
    }

    /// The other half: a tool both representations carry is not disturbed.
    #[test]
    fn a_tool_the_new_layer_has_is_left_alone() {
        let (mut vm, representation) = fixture_with_layer_changes();
        vm.dispatch(Command::SelectTool(ToolKind::Suavizar))
            .expect("smooth is on both");

        representation.set(Representation::Voxel);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
            .expect("select");

        assert_eq!(
            *vm.tool().get(),
            ToolKind::Suavizar,
            "smooth has a voxel verb, so it must survive the move"
        );
    }

    /// A size that suits a field is not a size that suits a grid's cells.
    #[test]
    fn brush_settings_are_remembered_per_representation() {
        let (mut vm, representation) = fixture_with_layer_changes();
        vm.dispatch(Command::SelectTool(ToolKind::Suavizar))
            .expect("smooth");
        vm.dispatch(Command::SetBrushSize(0.4)).expect("size");

        representation.set(Representation::Voxel);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
            .expect("select");
        vm.dispatch(Command::SetBrushSize(0.05)).expect("size");
        assert_eq!(vm.brush().get().size, 0.05);

        representation.set(Representation::Sdf);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
            .expect("select");
        assert_eq!(
            vm.brush().get().size,
            0.4,
            "the SDF layer's size came back as the voxel layer's"
        );
    }
}

/// Every segment of a mesh drag carries the gesture from its anchor.
///
/// Grab anchors on the first stamp and carries that region by the motion that
/// follows, so a segment holding only the newest samples is a *second* grab
/// anchoring where the first stopped. Measured against Blender's Grab over
/// MCP — matched sphere, same brush radius in world units, same drag: one call
/// reaches 9.8% of the mesh and moves it 0.707, Blender reaches 11.4% and
/// moves 0.779, and the same gesture split into two independent segments
/// reaches 19.0% and moves 0.569 — two anchors sharing one drag.
///
/// So the segments stay, because they are what makes the drag *visible* while
/// it happens, and each one replays the whole gesture instead. The model takes
/// back what the last segment did before laying it down again, which is what
/// keeps one drag to one undo.
#[test]
fn every_segment_of_a_mesh_drag_replays_it_from_the_anchor() {
    let drag = |vm: &mut SculptViewModel| {
        vm.dispatch(Command::BeginStroke {
            position: [0.0, 0.0, 1.0],
            pressure: 1.0,
            modifiers: Default::default(),
        })
        .expect("begin");
        for step in 1..=24 {
            let t = step as f32 / 24.0;
            vm.dispatch(Command::ContinueStroke {
                position: [t * 2.0, t * 0.5, 1.0],
                pressure: 1.0,
            })
            .expect("continue");
        }
        vm.dispatch(Command::EndStroke).expect("end");
    };

    let (mut mesh, calls) = fixture_with(|model| {
        model.representation.set(Representation::Mesh);
    });
    mesh.dispatch(Command::SelectTool(ToolKind::Mover))
        .expect("tool");
    drag(&mut mesh);

    let strokes = calls.borrow();
    let drags: Vec<&Vec<GestureSample>> = strokes.strokes.iter().map(|s| &s.1).collect();
    assert!(
        drags.len() > 1,
        "a mesh drag reached the model as {} call(s), so nothing is drawn until \
         the pointer comes up",
        drags.len()
    );

    // Every one of them starts where the gesture did. A segment starting
    // anywhere else is a second grab.
    let anchor = drags[0][0].position;
    for (i, samples) in drags.iter().enumerate() {
        assert_eq!(
            samples[0].position, anchor,
            "segment {i} starts at {:?} rather than the gesture's anchor {anchor:?}",
            samples[0].position
        );
    }
    // And each carries more of it than the last.
    for pair in drags.windows(2) {
        assert!(
            pair[1].len() >= pair[0].len(),
            "a segment carried fewer samples than the one before it"
        );
    }
}

/// A mesh stroke is seen while it is made, whichever verb it is.
///
/// Two things kept Suavizar from being seen at all. It is *region-based* —
/// on a field it samples a region into a volume, modifies it and puts it back
/// with a replace, which cannot be segmented — so it was held until the
/// pointer came up. And a mesh segment waited for three stamps' worth of
/// travel, a threshold that exists because a field segment costs a re-mesh of
/// every brick it touched.
///
/// On a mesh neither applies: these verbs are ordinary stamps over the
/// vertices in reach, and nothing is re-meshed. The field keeps both
/// behaviours, and this holds the difference.
#[test]
fn a_mesh_stroke_is_applied_while_it_is_made() {
    let drag = |vm: &mut SculptViewModel| {
        vm.dispatch(Command::SetBrushSize(0.18)).expect("size");
        vm.dispatch(Command::BeginStroke {
            position: [0.0, 0.0, 1.0],
            pressure: 1.0,
            modifiers: Default::default(),
        })
        .expect("begin");
        for step in 1..=40 {
            let t = step as f32 / 40.0;
            vm.dispatch(Command::ContinueStroke {
                position: [t * 0.8, 0.0, 1.0],
                pressure: 1.0,
            })
            .expect("continue");
        }
    };

    for tool in [ToolKind::Suavizar, ToolKind::Padrao] {
        let (mut mesh, calls) = fixture_with(|model| {
            model.representation.set(Representation::Mesh);
        });
        mesh.dispatch(Command::SelectTool(tool)).expect("tool");
        drag(&mut mesh);
        let during = calls.borrow().strokes.len();
        assert!(
            during > 4,
            "{:?} on a mesh reached the model {during} time(s) over forty \
             pointer moves, so the sculptor rubs at a surface that does not \
             answer until they let go",
            tool
        );
    }

    // The field is unchanged: a bake is still applied once, at the end.
    let (mut field, calls) = fixture_with(|model| {
        model.representation.set(Representation::Sdf);
    });
    field
        .dispatch(Command::SelectTool(ToolKind::Suavizar))
        .expect("tool");
    drag(&mut field);
    assert_eq!(
        calls.borrow().strokes.len(),
        0,
        "a field bake was segmented; it stacks a replacement per segment and \
         the result crumbles"
    );
    field.dispatch(Command::EndStroke).expect("end");
    assert_eq!(
        calls.borrow().strokes.len(),
        1,
        "the field bake did not arrive when the gesture closed"
    );
}

// -- held keys ---------------------------------------------------------------

/// Draws a short stroke with the given keys held for the whole of it.
fn draw_holding(vm: &mut SculptViewModel, modifiers: StrokeModifiers) {
    vm.dispatch(Command::BeginStroke {
        position: [0.0, 0.0, 1.0],
        pressure: 1.0,
        modifiers,
    })
    .expect("begin");
    for step in 1..=8 {
        vm.dispatch(Command::ContinueStroke {
            position: [step as f32 * 0.05, 0.0, 1.0],
            pressure: 1.0,
        })
        .expect("continue");
    }
    vm.dispatch(Command::EndStroke).expect("end");
}

#[test]
fn holding_smooth_substitutes_the_verb_for_the_gesture() {
    let (mut vm, calls) = fixture_with(|_| {});
    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("tool");

    draw_holding(
        &mut vm,
        StrokeModifiers {
            smooth: true,
            invert: false,
        },
    );

    let held = calls.borrow();
    assert!(
        !held.strokes.is_empty(),
        "the stroke never reached the model"
    );
    for (tool, ..) in held.strokes.iter() {
        assert_eq!(
            *tool,
            ToolKind::Suavizar,
            "a segment of a Shift-held stroke arrived as {tool:?}; half the \
             drag would build up and the other half smooth it"
        );
    }
    drop(held);

    // The shelf never moved. Letting go returns to the chosen tool without the
    // sculptor having to re-pick it.
    assert_eq!(*vm.tool().get(), ToolKind::Padrao);
    let before = calls.borrow().strokes.len();
    draw_holding(&mut vm, StrokeModifiers::default());
    let after: Vec<ToolKind> = calls.borrow().strokes[before..]
        .iter()
        .map(|s| s.0)
        .collect();
    assert!(
        after.iter().all(|t| *t == ToolKind::Padrao),
        "letting Shift go left the brush smoothing: {after:?}"
    );
}

#[test]
fn holding_invert_turns_the_brush_over_without_changing_the_verb() {
    let (mut vm, calls) = fixture_with(|_| {});
    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("tool");

    draw_holding(
        &mut vm,
        StrokeModifiers {
            smooth: false,
            invert: true,
        },
    );

    let held = calls.borrow();
    assert!(
        !held.strokes.is_empty(),
        "the stroke never reached the model"
    );
    for (tool, _, _, brush) in held.strokes.iter() {
        assert_eq!(*tool, ToolKind::Padrao, "inverting picked a different verb");
        assert!(
            brush.invert,
            "a segment of a Ctrl-held stroke arrived upright, so the sculptor \
             adds clay where they asked to take it away"
        );
    }
    drop(held);

    // And the shelf's brush is untouched: the next stroke builds up again.
    assert!(!vm.brush().get().invert);
    let before = calls.borrow().strokes.len();
    draw_holding(&mut vm, StrokeModifiers::default());
    assert!(
        calls.borrow().strokes[before..].iter().all(|s| !s.3.invert),
        "letting Ctrl go left the brush inverted"
    );
}

#[test]
fn a_cancelled_stroke_lets_the_held_keys_go() {
    let (mut vm, calls) = fixture_with(|_| {});
    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("tool");
    vm.dispatch(Command::BeginStroke {
        position: [0.0, 0.0, 1.0],
        pressure: 1.0,
        modifiers: StrokeModifiers {
            smooth: true,
            invert: true,
        },
    })
    .expect("begin");
    vm.dispatch(Command::CancelStroke).expect("cancel");

    let before = calls.borrow().strokes.len();
    draw_holding(&mut vm, StrokeModifiers::default());
    let after: Vec<(ToolKind, bool)> = calls.borrow().strokes[before..]
        .iter()
        .map(|s| (s.0, s.3.invert))
        .collect();
    assert!(
        !after.is_empty() && after.iter().all(|(t, i)| *t == ToolKind::Padrao && !i),
        "an abandoned gesture kept its keys held into the next one: {after:?}"
    );
}

// -- the mask key ------------------------------------------------------------

#[test]
fn the_mask_key_goes_in_and_comes_back_out() {
    let (mut vm, _) = fixture_with(|_| {});
    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("tool");

    vm.dispatch(Command::ToggleMaskPainting).expect("in");
    assert_eq!(
        *vm.tool().get(),
        ToolKind::Mascara,
        "the key did not reach mask painting"
    );

    vm.dispatch(Command::ToggleMaskPainting).expect("out");
    assert_eq!(
        *vm.tool().get(),
        ToolKind::Padrao,
        "the key left the sculptor in mask painting; freezing a region is a \
         detour from what is being sculpted, and the way back should be the \
         same key rather than a hunt across the shelf"
    );
}

#[test]
fn choosing_a_tool_while_masking_is_not_undone_by_the_key() {
    let (mut vm, _) = fixture_with(|_| {});
    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("tool");
    vm.dispatch(Command::ToggleMaskPainting).expect("in");

    // Said outright, from the shelf: the detour is over.
    vm.dispatch(Command::SelectTool(ToolKind::Inflar))
        .expect("tool");
    assert_eq!(*vm.tool().get(), ToolKind::Inflar);

    // So the key starts a fresh one rather than rewinding to before the choice.
    vm.dispatch(Command::ToggleMaskPainting).expect("in again");
    assert_eq!(*vm.tool().get(), ToolKind::Mascara);
    vm.dispatch(Command::ToggleMaskPainting).expect("out");
    assert_eq!(
        *vm.tool().get(),
        ToolKind::Inflar,
        "the key returned to a tool the sculptor had already left"
    );
}

#[test]
fn the_mask_key_keeps_each_tools_own_brush() {
    // Máscara has its own remembered brush like every other tool, and the
    // toggle goes through the same selection — a route that bypassed it would
    // paint a mask with the sculpting brush's size and hand it back changed.
    let (mut vm, _) = fixture_with(|_| {});
    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("tool");
    vm.dispatch(Command::SetBrushSize(0.42)).expect("size");

    vm.dispatch(Command::ToggleMaskPainting).expect("in");
    vm.dispatch(Command::SetBrushSize(0.11)).expect("size");

    vm.dispatch(Command::ToggleMaskPainting).expect("out");
    assert!(
        (vm.brush().get().size - 0.42).abs() < 1e-6,
        "coming back from the mask left the brush at {}",
        vm.brush().get().size
    );
}
