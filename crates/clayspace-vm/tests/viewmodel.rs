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
    /// The shallowest the model's own history ever got.
    ///
    /// A cancel that reverts too far shows up here and nowhere else: the
    /// ViewModel's depth counts actions, and the entries a runaway cancel
    /// spends are the model's — the gestures committed before it, and the
    /// layer they were made on.
    shallowest: Option<usize>,
}

/// A Model that records its calls and answers however a test needs.
struct FakeModel {
    recorded: Rc<RefCell<Recorded>>,
    /// Shared, because the layer the ViewModel is looking at can change under
    /// it and a test has to be able to say so after construction — the model
    /// is boxed into the ViewModel and unreachable from outside otherwise.
    representation: Rc<Cell<Representation>>,
    /// Shared for the same reason, so a test can read back what the ViewModel
    /// told the document about which smooth to make.
    smooth_mode: Rc<Cell<clayspace_model::SmoothFrequency>>,
    /// Which row of a hierarchy takes the next stroke, shared so a test can
    /// move the selection the way a sculptor clicking the pass stack does.
    in_a_pass: Rc<Cell<bool>>,
    /// Whether a deformation cage stands around the active layer, shared so a
    /// test can raise and lower one the way `lattice/toggle` does.
    caged: Rc<Cell<bool>>,
    /// The active subtool's mirror, which the engine keeps per layer.
    ///
    /// Shared so a test can move it the way the document does when the layer
    /// underneath changes — a rig layer arrives with it off, and a subtool
    /// switched to carries its own.
    symmetry: Rc<Cell<[bool; 3]>>,
    editable: bool,
    /// What the next stroke reports.
    outcome: EditOutcome,
    history: HistoryState,
    stats: SceneStats,
    /// Whether the double banks a gesture the way a mesh layer does.
    ///
    /// A mesh gesture is previewed while it is made and banked as *one* record
    /// when it ends, however many segments drew it; the field path records an
    /// entry per segment. That difference is the whole of what a cancel has to
    /// get right, so the double has to be able to be either.
    banks_the_gesture_whole: bool,
    /// Whether a gesture is open, for the double that banks one whole.
    gesture_open: bool,
    /// How many segments the open gesture previewed, so that ending one that
    /// previewed nothing banks nothing.
    gesture_stamps: usize,
}

impl FakeModel {
    fn new(recorded: Rc<RefCell<Recorded>>) -> Self {
        Self {
            recorded,
            representation: Rc::new(Cell::new(Representation::Sdf)),
            smooth_mode: Rc::new(Cell::new(clayspace_model::SmoothFrequency::default())),
            in_a_pass: Rc::new(Cell::new(false)),
            caged: Rc::new(Cell::new(false)),
            // X on, as the document the engine adapter builds has it and as
            // the ViewModel starts out showing.
            symmetry: Rc::new(Cell::new([true, false, false])),
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
                faces: None,
                vertices: 60,
                objects: 1,
                detail: clayspace_model::Detail::Full,
            },
            banks_the_gesture_whole: false,
            gesture_open: false,
            gesture_stamps: 0,
        }
    }

    /// One more thing to take back.
    fn bank_an_entry(&mut self) {
        self.history.depth += 1;
        self.history.can_undo = true;
        self.history.redo_depth = 0;
        self.history.can_redo = false;
    }
}

impl SculptModel for FakeModel {
    fn active_representation(&self) -> Representation {
        self.representation.get()
    }

    fn active_layer_editable(&self) -> bool {
        self.editable
    }

    fn active_layer_stroke_lands_in_a_pass(&self) -> bool {
        self.in_a_pass.get()
    }

    fn active_layer_is_caged(&self) -> bool {
        self.caged.get()
    }

    fn smooth_mode(&self) -> clayspace_model::SmoothFrequency {
        self.smooth_mode.get()
    }

    fn symmetry(&self) -> [bool; 3] {
        self.symmetry.get()
    }

    fn set_symmetry(&mut self, symmetry: [bool; 3]) -> Result<(), ModelError> {
        self.symmetry.set(symmetry);
        Ok(())
    }

    fn set_smooth_mode(&mut self, mode: clayspace_model::SmoothFrequency) {
        self.smooth_mode.set(mode);
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
        if !self.outcome.changed {
            return Ok(self.outcome);
        }
        if self.banks_the_gesture_whole && self.gesture_open {
            // Previewed, not banked. The record is pushed once, when the
            // gesture ends.
            self.gesture_stamps += 1;
        } else {
            // An edit that changed something is an entry, which is what the
            // ViewModel counts to know how far an undo has to reach.
            self.bank_an_entry();
        }
        Ok(self.outcome)
    }

    fn begin_gesture(&mut self) {
        self.gesture_open = true;
        self.gesture_stamps = 0;
    }

    fn end_gesture(&mut self) {
        let previewed = std::mem::take(&mut self.gesture_stamps);
        let banking = self.banks_the_gesture_whole && self.gesture_open && previewed > 0;
        self.gesture_open = false;
        if banking {
            self.bank_an_entry();
        }
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
        let depth = self.history.depth;
        let mut recorded = self.recorded.borrow_mut();
        recorded.shallowest = Some(recorded.shallowest.map_or(depth, |seen| seen.min(depth)));
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

/// A fixture whose whole active subtool a test can move: what it holds, the
/// mirror it carries, and the record of what a stroke on it was made with.
///
/// The representation and the symmetry move *before* the command reaches the
/// ViewModel, because that is the order the composition root dispatches in —
/// the scene ViewModel is what shifts the document's active layer, and every
/// follower reads the document afterwards.
#[allow(clippy::type_complexity)]
fn fixture_with_a_moving_layer() -> (
    SculptViewModel,
    Rc<Cell<Representation>>,
    Rc<Cell<[bool; 3]>>,
    Rc<RefCell<Recorded>>,
) {
    let recorded = Rc::new(RefCell::new(Recorded::default()));
    let model = FakeModel::new(recorded.clone());
    let representation = model.representation.clone();
    let symmetry = model.symmetry.clone();
    (
        SculptViewModel::new(Box::new(model)),
        representation,
        symmetry,
        recorded,
    )
}

/// A fixture whose cage a test can raise and lower.
fn fixture_with_a_cage() -> (SculptViewModel, Rc<Cell<bool>>) {
    let recorded = Rc::new(RefCell::new(Recorded::default()));
    let model = FakeModel::new(recorded);
    let caged = model.caged.clone();
    (SculptViewModel::new(Box::new(model)), caged)
}

/// A fixture whose hierarchy row a test can move between the form and a pass.
fn fixture_with_a_pass_selection() -> (SculptViewModel, Rc<Cell<Representation>>, Rc<Cell<bool>>) {
    let recorded = Rc::new(RefCell::new(Recorded::default()));
    let model = FakeModel::new(recorded);
    let representation = model.representation.clone();
    let in_a_pass = model.in_a_pass.clone();
    (
        SculptViewModel::new(Box::new(model)),
        representation,
        in_a_pass,
    )
}

/// A fixture that hands back what the document was told to smooth.
fn fixture_with_a_smooth_mode() -> (
    SculptViewModel,
    Rc<Cell<Representation>>,
    Rc<Cell<clayspace_model::SmoothFrequency>>,
) {
    let recorded = Rc::new(RefCell::new(Recorded::default()));
    let model = FakeModel::new(recorded);
    let representation = model.representation.clone();
    let mode = model.smooth_mode.clone();
    (SculptViewModel::new(Box::new(model)), representation, mode)
}

/// What the double's history already holds before a test does anything: the
/// layer the sculptor is about to work on. No cancel may ever reach it.
const THE_LAYER: usize = 1;

/// A ViewModel over a mesh layer that banks a gesture the way the engine does:
/// one record when the gesture ends, however many segments drew it.
fn mesh_fixture() -> (SculptViewModel, Rc<RefCell<Recorded>>) {
    fixture_with(|model| {
        model.representation.set(Representation::Mesh);
        model.banks_the_gesture_whole = true;
    })
}

/// Opens a gesture and feeds it, without closing it.
fn open_a_gesture(vm: &mut SculptViewModel, points: &[[f32; 3]]) {
    let (first, rest) = points.split_first().expect("a gesture needs a point");
    vm.dispatch(Command::BeginStroke {
        position: *first,
        pressure: 1.0,
        modifiers: Default::default(),
    })
    .expect("begin");
    for point in rest {
        vm.dispatch(Command::ContinueStroke {
            position: *point,
            pressure: 1.0,
        })
        .expect("continue");
    }
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

/// The history names the *next step in each direction*, not the last thing
/// that happened.
///
/// They differ exactly where it matters. The status area used to be the only
/// reader and could live with the difference, because a person watched the
/// undo happen; an agent reading state could not, and was told that its next
/// undo would undo an "undo".
#[test]
fn history_labels_name_the_next_step() {
    let (mut vm, _) = fixture();
    vm.dispatch(Command::SelectTool(ToolKind::Argila))
        .expect("a tool");
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("a stroke");

    assert_eq!(
        vm.next_undo(),
        Some(ToolKind::Argila.label()),
        "the next undo takes the clay stroke back, and says so"
    );
    assert_eq!(vm.next_redo(), None, "nothing has been undone yet");

    vm.dispatch(Command::Undo).expect("undo");
    assert_eq!(
        vm.last_action().get().label,
        "undo",
        "the last thing that happened is the undo — which is what the report \
         used to send as what the next undo would take back"
    );
    assert_eq!(
        vm.next_undo(),
        None,
        "there is nothing left to take back, rather than an \"undo\" to undo"
    );
    assert_eq!(
        vm.next_redo(),
        Some(ToolKind::Argila.label()),
        "and the redo names the stroke it would put back"
    );

    vm.dispatch(Command::Redo).expect("redo");
    assert_eq!(vm.next_undo(), Some(ToolKind::Argila.label()));
    assert_eq!(vm.next_redo(), None);
}

/// A cancelled stroke banked nothing, so what the next undo would take back is
/// whatever came before it — and the report named the cancelled tool instead.
#[test]
fn a_cancelled_stroke_does_not_name_the_next_undo() {
    let (mut vm, _) = fixture();
    vm.dispatch(Command::SelectTool(ToolKind::Suavizar))
        .expect("a tool");
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("a stroke");
    vm.dispatch(Command::SelectTool(ToolKind::Argila))
        .expect("a second tool");

    open_a_gesture(&mut vm, &[[0.5, 0.0, 0.0], [0.6, 0.0, 0.0]]);
    vm.dispatch(Command::CancelStroke).expect("cancel");

    assert_eq!(
        vm.next_undo(),
        Some(ToolKind::Suavizar.label()),
        "the cancelled clay stroke is gone, so the next undo is the smooth \
         under it — not the tool the cancel was holding"
    );
}

/// An edit banked by another ViewModel carries its own name onto the shared
/// history, rather than borrowing whatever happened last.
#[test]
fn an_edit_banked_from_outside_names_itself() {
    let (mut vm, _) = fixture();
    vm.record_external_action("insert shape", 2);
    assert_eq!(vm.next_undo(), Some("insert shape"));

    vm.dispatch(Command::Undo).expect("undo");
    assert_eq!(vm.next_redo(), Some("insert shape"));
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

/// Cancelling a mesh gesture takes back that gesture and stops there.
///
/// It used to spend one undo per applied segment. That is the count the field
/// path records, and a mesh gesture is banked as ONE record however many
/// segments drew it — so the first undo took the gesture back and every one
/// after it took back whatever was underneath: the gestures already committed,
/// and on a fresh layer the layer itself. Cancel, the safe way out of a
/// gesture, was the most destructive command in the mesh path.
#[test]
fn cancel_on_a_mesh_reverts_only_the_open_gesture() {
    let (mut vm, recorded) = mesh_fixture();
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0], [0.2, 0.0, 0.0]]).expect("first gesture");
    draw(
        &mut vm,
        &[[0.0, 0.1, 0.0], [0.1, 0.1, 0.0], [0.2, 0.1, 0.0]],
    )
    .expect("second gesture");
    let committed = vm.history().get().depth;
    assert_eq!(
        committed, 2,
        "the two gestures drawn are two things to undo"
    );
    let before = recorded.borrow().strokes.len();

    open_a_gesture(
        &mut vm,
        &[[0.0, 0.2, 0.0], [0.1, 0.2, 0.0], [0.2, 0.2, 0.0]],
    );
    vm.dispatch(Command::CancelStroke).expect("cancel");

    let seen = recorded.borrow();
    assert!(
        seen.strokes.len() > before + 1,
        "the cancelled gesture reached the model whole, so a per-segment count \
         could not have been wrong and this proves nothing"
    );
    assert_eq!(
        seen.undos, 1,
        "a mesh gesture is one record, so cancelling it is one undo"
    );
    assert_eq!(
        seen.shallowest,
        Some(THE_LAYER + committed),
        "cancel reached past the gesture it was cancelling"
    );
    assert_eq!(
        vm.history().get().depth,
        committed,
        "the committed gestures must still be there to undo"
    );
    assert!(
        !vm.history().get().can_redo,
        "a cancelled gesture is not an action the sculptor can ask back"
    );
}

/// The layer survives its own first gesture being cancelled.
///
/// The worst of the runaway: on a layer made a moment ago there is nothing
/// under the gesture *but* the layer, so the extra undos took it, and the
/// sculptor was left looking at a document with one fewer subtool than before
/// they touched it.
#[test]
fn cancel_on_a_fresh_mesh_layer_keeps_the_layer() {
    let (mut vm, recorded) = mesh_fixture();
    open_a_gesture(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0], [0.2, 0.0, 0.0]]);
    vm.dispatch(Command::CancelStroke).expect("cancel");

    let seen = recorded.borrow();
    assert!(
        seen.strokes.len() > 1,
        "the gesture was not segmented, so this proves nothing"
    );
    assert_eq!(seen.undos, 1, "the one banked record is the one undo owed");
    assert_eq!(
        seen.shallowest,
        Some(THE_LAYER),
        "cancel reached past its own gesture and took the layer with it"
    );
    assert!(
        !vm.history().get().can_undo,
        "a cancelled gesture must leave nothing to undo"
    );
}

/// A *committed* mesh gesture is one undo, not one per segment.
///
/// The sibling of the cancel defect, on the other side of the same arithmetic
/// and found while fixing it: the count banked at the release was one per
/// applied segment, which is the number a field gesture records and not the
/// number a mesh one does. So one Cmd+Z after a three-segment mesh gesture
/// walked the model's history back by three — the gesture, and then whatever
/// was underneath it. Measured against this double before the fix: the gesture
/// banked 3 and one undo took the history to 0, which on a fresh layer is the
/// layer.
#[test]
fn a_committed_mesh_gesture_undoes_as_one_record() {
    let (mut vm, recorded) = mesh_fixture();
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0], [0.2, 0.0, 0.0]]).expect("gesture");
    assert!(
        recorded.borrow().strokes.len() > 1,
        "the gesture was not segmented, so a per-segment count could not have \
         been wrong and this proves nothing"
    );
    assert_eq!(
        vm.history().get().depth,
        1,
        "one gesture is one thing to take back"
    );

    vm.dispatch(Command::Undo).expect("undo");
    let seen = recorded.borrow();
    assert_eq!(
        seen.undos, 1,
        "a mesh gesture is one record, so one Cmd+Z is one undo"
    );
    assert_eq!(
        seen.shallowest,
        Some(THE_LAYER),
        "the undo reached past the gesture and took the layer with it"
    );
}

/// The field path is unchanged by the same measurement: a gesture that records
/// an entry per segment still costs one Cmd+Z, and that Cmd+Z spends every
/// entry the gesture made.
#[test]
fn a_committed_field_gesture_still_spends_every_entry_it_made() {
    let (mut vm, recorded) = fixture();
    draw(&mut vm, &[[0.0; 3], [0.5, 0.0, 0.0], [1.0, 0.0, 0.0]]).expect("gesture");
    let segments = recorded.borrow().strokes.len();
    assert!(segments > 1, "the gesture was not segmented");
    assert_eq!(vm.history().get().depth, 1);

    vm.dispatch(Command::Undo).expect("undo");
    let seen = recorded.borrow();
    assert_eq!(
        seen.undos, segments,
        "a field gesture is an entry per segment, and one Cmd+Z owes every one"
    );
    assert_eq!(seen.shallowest, Some(THE_LAYER));
}

/// A stroke the document recorded nothing for is nothing to take back.
///
/// Decided from what the stroke *wrote* rather than from where it started: a
/// dab that landed off the surface can still deposit a blob, and that blob is
/// an edit. What it must not do is bank an entry when the document came out
/// byte-identical, which left the history offering a "Padrão" to undo that
/// undid nothing the sculptor could see.
#[test]
fn a_gesture_that_wrote_nothing_is_not_something_to_undo() {
    let (mut vm, _) = fixture_with(|model| {
        model.outcome = EditOutcome {
            changed: false,
            dirty_bricks: 0,
        };
    });
    draw(&mut vm, &[[0.0; 3], [0.5, 0.0, 0.0], [1.0, 0.0, 0.0]]).expect("gesture");
    assert_eq!(
        vm.history().get().depth,
        0,
        "a stroke that changed nothing was banked as something to take back"
    );
    assert!(!vm.history().get().can_undo);
}

/// A cancel with nothing open changes nothing, and says so.
///
/// An agent may repeat one safely — the interface sends a release whether or
/// not a press opened anything — so it is not a refusal. What it must not be
/// is silent success over a document it quietly reverted.
#[test]
fn cancel_with_no_gesture_is_a_reported_no_op() {
    let (mut vm, recorded) = mesh_fixture();
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0], [0.2, 0.0, 0.0]]).expect("gesture");
    let (strokes, undos) = {
        let seen = recorded.borrow();
        (seen.strokes.len(), seen.undos)
    };

    vm.dispatch(Command::CancelStroke).expect("nothing open");
    vm.dispatch(Command::CancelStroke)
        .expect("still nothing open");

    let seen = recorded.borrow();
    assert_eq!(
        (seen.strokes.len(), seen.undos),
        (strokes, undos),
        "a cancel with nothing open spent history belonging to what came before"
    );
    assert_eq!(
        vm.history().get().depth,
        1,
        "the committed gesture must still be there to undo"
    );
    let last = vm.last_action().get().clone();
    assert_eq!(last.label, "cancel stroke");
    assert!(
        !last.changed,
        "a cancel with nothing to cancel reported that it had changed something"
    );
    assert!(
        last.tool.is_none(),
        "naming a tool reads as a stroke landing"
    );
}

/// The field path is unchanged: an entry per segment, and cancel spends each.
#[test]
fn cancel_on_a_field_layer_reverts_the_open_gesture() {
    let (mut vm, recorded) = fixture();
    // Far enough apart to be worth sending as they are made: a field gesture
    // that never leaves the brush's footprint arrives as one piece, and one
    // piece cannot tell a per-segment count from any other.
    draw(&mut vm, &[[0.0; 3], [0.5, 0.0, 0.0], [1.0, 0.0, 0.0]]).expect("committed gesture");
    let committed = recorded.borrow().strokes.len();

    open_a_gesture(
        &mut vm,
        &[[0.0, 1.0, 0.0], [0.5, 1.0, 0.0], [1.0, 1.0, 0.0]],
    );
    vm.dispatch(Command::CancelStroke).expect("cancel");

    let seen = recorded.borrow();
    let cancelled = seen.strokes.len() - committed;
    assert!(cancelled > 1, "the cancelled gesture was not segmented");
    assert_eq!(
        seen.undos, cancelled,
        "a field gesture is an entry per segment, and cancel owes every one"
    );
    assert_eq!(
        seen.shallowest,
        Some(THE_LAYER + committed),
        "cancel reached past the gesture it was cancelling"
    );
    assert_eq!(
        vm.history().get().depth,
        1,
        "the committed gesture must still be there to undo"
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

/// A cage owns the form it stands around, and it owns it for every caller.
///
/// The rule lived in `input::press_sculpts`, which only the pointer handler
/// consults: a press that missed a control point orbited rather than
/// sculpting. A caller that reached the ViewModel another way — the agent
/// door does — was unaffected by the cage and sculpted the very form the cage
/// was there to bend. The stroke it left survived the cage being applied,
/// which is what made it impossible to attribute afterwards.
///
/// Here rather than in the pointer handler's tests because that is where the
/// guard now is; the pointer's own check stays as a second line.
#[test]
fn a_stroke_is_refused_while_a_cage_is_up() {
    let (mut vm, caged) = fixture_with_a_cage();
    caged.set(true);

    let error = vm
        .dispatch(Command::BeginStroke {
            position: [0.0; 3],
            pressure: 1.0,
            modifiers: Default::default(),
        })
        .expect_err("a cage takes the form; no stroke may reach past it");

    assert!(
        error.to_string().contains("cage"),
        "the refusal must name the cage, so a caller knows what to do about \
         it: {error}"
    );
    assert!(
        !vm.is_stroking(),
        "a refused stroke must not start collecting"
    );

    // And the same ViewModel strokes once the cage is down, so what is
    // refused is the cage rather than the layer.
    caged.set(false);
    vm.dispatch(Command::BeginStroke {
        position: [0.0; 3],
        pressure: 1.0,
        modifiers: Default::default(),
    })
    .expect("with the cage down this is an ordinary stroke");
    assert!(vm.is_stroking());
}

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

/// The grain reaches the brush and is remembered per tool like everything
/// else on it.
///
/// A quarter turn rather than the default, because a default is what a
/// dropped field looks like: it arrives correct whether anything carried it
/// or not. Upstream lost this very field to a round trip made entirely of
/// zeroes.
#[test]
fn the_grain_is_a_brush_setting_like_any_other() {
    let (mut vm, _) = fixture();
    let quarter = std::f32::consts::FRAC_PI_2;
    vm.dispatch(Command::SetBrushAzimuth(quarter))
        .expect("grain");
    assert_eq!(vm.brush().get().shaping.azimuth, quarter);

    vm.dispatch(Command::SelectTool(ToolKind::Suavizar))
        .expect("select");
    assert_eq!(
        vm.brush().get().shaping.azimuth,
        0.0,
        "a tool that was never turned should not inherit another's grain"
    );

    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("select");
    assert_eq!(
        vm.brush().get().shaping.azimuth,
        quarter,
        "switching away and back must return the grain the sculptor left"
    );
}

/// A whole turn is no turn, and it is brought back rather than clamped: an
/// angle has no ends to run out of travel at.
#[test]
fn a_grain_dialled_all_the_way_round_comes_back_to_none() {
    let (mut vm, _) = fixture();
    vm.dispatch(Command::SetBrushAzimuth(std::f32::consts::TAU))
        .expect("grain");
    assert_eq!(vm.brush().get().shaping.azimuth, 0.0);
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

    /// Every substitution is counted, not only the ones whose sentence is new.
    ///
    /// This channel is read either side of a command to decide what to tell an
    /// agent, and a swap says the same thing every time it happens. A swap
    /// that went unreported would be a tool changing under the sculptor in
    /// silence.
    #[test]
    fn each_substitution_is_reported_in_its_own_right() {
        let (mut vm, representation) = fixture_with_layer_changes();
        let mut swaps = 0;

        for _ in 0..2 {
            representation.set(Representation::Voxel);
            vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
                .expect("select");
            vm.dispatch(Command::SelectTool(ToolKind::Raspar))
                .expect("scrape is a voxel tool");
            let before = vm.tool_status().occurrences();

            representation.set(Representation::Sdf);
            vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
                .expect("select");

            assert_eq!(
                vm.tool_status().get().as_deref(),
                Some(clayspace_vm::TOOL_SUBSTITUTED)
            );
            assert_ne!(
                vm.tool_status().occurrences(),
                before,
                "a swap that happened was not counted as having happened"
            );
            swaps += 1;
        }
        assert_eq!(swaps, 2);
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

    /// Two tools, two representations: four slots, and each gives back what
    /// was set in it.
    ///
    /// The grain the settings are kept at is the pair, so a size set for one
    /// tool on a grid is neither the size that tool has on a field nor the
    /// size another tool has on the same grid.
    #[test]
    fn settings_are_kept_per_tool_and_representation() {
        let (mut vm, representation) = fixture_with_layer_changes();
        let set = |vm: &mut SculptViewModel, on, tool, size| {
            representation.set(on);
            vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
                .expect("select");
            vm.dispatch(Command::SelectTool(tool)).expect("tool");
            vm.dispatch(Command::SetBrushSize(size)).expect("size");
        };
        let slots = [
            (Representation::Sdf, ToolKind::Suavizar, 0.40),
            (Representation::Sdf, ToolKind::Padrao, 0.30),
            (Representation::Voxel, ToolKind::Suavizar, 0.06),
            (Representation::Voxel, ToolKind::Padrao, 0.08),
        ];
        for (on, tool, size) in slots {
            set(&mut vm, on, tool, size);
        }

        // Read back in a different order from the one written, so a slot that
        // only echoes the last write fails.
        for (on, tool, size) in slots.into_iter().rev() {
            representation.set(on);
            vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
                .expect("select");
            vm.dispatch(Command::SelectTool(tool)).expect("tool");
            assert_eq!(
                vm.brush().get().size,
                size,
                "{} on {} came back at a size set somewhere else",
                tool.label(),
                on.label()
            );
        }
    }

    /// A slot nothing has been set in starts at its representation's own
    /// default, whatever was set on the layer before.
    ///
    /// The session this came from set a field's brush to 100 and added a
    /// grid; the first dab on the grid was a metre across. Measured on the
    /// stroke the model was handed, which is what reached the grid.
    #[test]
    fn no_representation_inherits_a_brush_size_from_another() {
        let (mut vm, representation, _symmetry, recorded) = fixture_with_a_moving_layer();
        vm.dispatch(Command::SelectTool(ToolKind::Suavizar))
            .expect("smooth is on both");
        vm.dispatch(Command::SetBrushSize(100.0))
            .expect("a field-sized brush");

        representation.set(Representation::Voxel);
        vm.dispatch(Command::AddLayer(Representation::Voxel))
            .expect("add");
        draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("stroke");

        let expected = clayspace_model::BrushSettings::default_for(Representation::Voxel);
        assert_eq!(vm.brush().get().size, expected.size);
        assert_eq!(
            recorded.borrow().strokes[0].3.size,
            expected.size,
            "the first dab on the grid was made at the field's size"
        );
    }

    /// The substitute is the capability table's, and the same switch always
    /// gives the same one.
    ///
    /// Raspar has no verb on a field, and the field's flatten is Planar — the
    /// same act — rather than whatever the shelf happens to list first.
    #[test]
    fn an_unsupported_tool_falls_back_deterministically() {
        for _ in 0..3 {
            let (mut vm, representation) = fixture_with_layer_changes();
            representation.set(Representation::Voxel);
            vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
                .expect("select");
            vm.dispatch(Command::SelectTool(ToolKind::Raspar))
                .expect("scrape is a voxel tool");

            representation.set(Representation::Sdf);
            vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(2)))
                .expect("select");

            assert_eq!(*vm.tool().get(), ToolKind::Planar);
            assert_eq!(
                vm.tool().get().intent(),
                ToolKind::Raspar.intent(),
                "the stand-in does a different act from the tool it replaced"
            );
        }
    }

    /// The swap is said where the sculptor looks, and kept where an agent
    /// reads: which tool was chosen, which is in hand, and on what.
    #[test]
    fn a_substitution_is_reported() {
        let (mut vm, representation) = fixture_with_layer_changes();
        representation.set(Representation::Voxel);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
            .expect("select");
        vm.dispatch(Command::SelectTool(ToolKind::Raspar))
            .expect("scrape is a voxel tool");
        assert_eq!(vm.substitution(), None, "a chosen tool is not a stand-in");

        representation.set(Representation::Sdf);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(2)))
            .expect("select");

        assert_eq!(
            vm.tool_status().get().as_deref(),
            Some(clayspace_vm::TOOL_SUBSTITUTED)
        );
        let substitution = vm
            .substitution()
            .expect("the swap is held, not only announced");
        assert_eq!(substitution.chosen, ToolKind::Raspar);
        assert_eq!(substitution.standing_in, ToolKind::Planar);
        assert_eq!(substitution.representation, Representation::Sdf);
        assert!(
            substitution.describe().contains("'scrape'"),
            "an agent is told the tool by the key it chose it with: {}",
            substitution.describe()
        );

        // Choosing a tool answers the swap.
        vm.dispatch(Command::SelectTool(ToolKind::Planar))
            .expect("planar is on a field");
        assert_eq!(vm.substitution(), None);
    }

    /// Switching away and back returns the tool that was chosen, not the one
    /// the switch away handed over.
    #[test]
    fn the_chosen_tool_returns_with_a_layer_that_carries_it() {
        let (mut vm, representation) = fixture_with_layer_changes();
        representation.set(Representation::Voxel);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
            .expect("select");
        vm.dispatch(Command::SelectTool(ToolKind::Raspar))
            .expect("scrape is a voxel tool");
        vm.dispatch(Command::SetBrushSize(0.07)).expect("size");

        representation.set(Representation::Sdf);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(2)))
            .expect("select");
        assert_eq!(*vm.tool().get(), ToolKind::Planar);

        representation.set(Representation::Voxel);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
            .expect("select");
        assert_eq!(*vm.tool().get(), ToolKind::Raspar);
        assert_eq!(
            vm.brush().get().size,
            0.07,
            "and with the size it had there"
        );
        assert_eq!(vm.substitution(), None);
        assert_ne!(
            vm.tool_status().get().as_deref(),
            Some(clayspace_vm::TOOL_SUBSTITUTED),
            "a return to the chosen tool is not a second swap"
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

    /// A new layer is a switch, because it arrives active.
    ///
    /// `add_layer` activates what it made, through the same call a stack click
    /// takes. This ViewModel used to ignore `AddLayer` outright, so the brush
    /// went on holding the settings of the subtool the sculptor was on: from a
    /// session, `layer add {kind:'grid'}` off a field layer at size 100 made
    /// the first dab on the grid a metre across.
    #[test]
    fn a_new_layer_arrives_with_its_own_brush_rather_than_the_previous_ones() {
        let (mut vm, representation) = fixture_with_layer_changes();
        vm.dispatch(Command::SelectTool(ToolKind::Suavizar))
            .expect("smooth is on both");
        vm.dispatch(Command::SetBrushSize(0.4))
            .expect("a field-sized brush");

        representation.set(Representation::Voxel);
        vm.dispatch(Command::AddLayer(Representation::Voxel))
            .expect("add");

        assert_ne!(
            vm.brush().get().size,
            0.4,
            "the new grid layer inherited the field layer's brush size, so the \
             first dab on it was made at the previous subtool's scale"
        );
    }

    /// The whole point of the switch being atomic: the *next stroke*.
    ///
    /// Measured on the stroke the model was handed rather than on what the
    /// options bar shows, because the two disagreeing is the defect — a size
    /// shown that is not the size used is exactly what a sculptor reports as
    /// "it drew far too big".
    #[test]
    fn the_first_stroke_after_a_switch_uses_the_new_layers_brush() {
        let (mut vm, representation, symmetry, recorded) = fixture_with_a_moving_layer();
        vm.dispatch(Command::SelectTool(ToolKind::Suavizar))
            .expect("smooth is on both");
        vm.dispatch(Command::SetBrushSize(0.4))
            .expect("a field-sized brush");

        // Away to a grid, which keeps its own size, and back.
        representation.set(Representation::Voxel);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(2)))
            .expect("select");
        vm.dispatch(Command::SetBrushSize(0.05))
            .expect("a cell-sized brush");

        representation.set(Representation::Sdf);
        symmetry.set([false, true, false]);
        vm.dispatch(Command::SelectLayer(clayspace_model::LayerKey(1)))
            .expect("select");

        draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("stroke");

        let recorded = recorded.borrow();
        let (_, _, mirror, brush) = &recorded.strokes[0];
        assert_eq!(
            brush.size, 0.4,
            "the first stroke after the switch was made with the grid's brush"
        );
        assert_eq!(
            *mirror,
            [false, true, false],
            "the first stroke after the switch was made with the grid's mirror"
        );
    }

    /// A rig layer arrives with its mirror off, and the options bar has to
    /// know before the first ZSphere is placed.
    ///
    /// `begin_armature` gives the armature a layer of its own and turns that
    /// layer's symmetry off — `add_zsphere` places the reflected node itself,
    /// so a layer mirror would reflect the placed item as well. No
    /// `SelectLayer` announces any of it, which is why the composition root
    /// asks for the refresh by name.
    #[test]
    fn a_new_rig_layer_uses_its_own_symmetry() {
        let (mut vm, _representation, symmetry, recorded) = fixture_with_a_moving_layer();
        assert_eq!(
            *vm.symmetry().get(),
            [true, false, false],
            "the fixture starts mirrored, or this measures nothing"
        );

        symmetry.set([false; 3]);
        vm.refresh_for_active_layer();

        assert_eq!(*vm.symmetry().get(), [false; 3]);
        draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("stroke");
        assert_eq!(
            recorded.borrow().strokes[0].2,
            [false; 3],
            "the first stroke on the rig layer came out mirrored"
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

/// A field drag that is not previewed still reaches the model from its anchor.
///
/// The regression this exists for. When a live Move transaction is open the
/// anchor lives in the transaction and the samples only have to carry the
/// pointer; when one is *not* open — a mirror that could not be pointed, or
/// any caller that never opened one — the drag falls to
/// `move_surface_stroke`, which takes `samples[0]` as the centre of the grab.
///
/// `pending()` hands a path-driven tool the last sample it already sent, so
/// `samples[0]` on every segment after the first was **where the previous
/// segment stopped**. The centre moved with the pointer, which is the one
/// thing `clay_layer_move_surface` cannot coalesce: measured on the pinned
/// engine, six segments sent that way leave a deformer chain of 6, where six
/// sent from a fixed anchor leave 1. Six grabs where the sculptor made one
/// drag, each multiplying the layer's Lipschitz bound for the life of the
/// edit list.
///
/// So a field Move replays from the anchor like a mesh drag does. It does not
/// need the model to take the last segment back the way a mesh does — the
/// engine coalesces a grab that repeats its centre and radius — but it does
/// need every segment to start in the same place, which is what this pins.
#[test]
fn every_segment_of_a_field_drag_also_starts_at_the_anchor() {
    let (mut vm, calls) = fixture_with(|model| {
        model.representation.set(Representation::Sdf);
    });
    vm.dispatch(Command::SelectTool(ToolKind::Mover))
        .expect("tool");
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

    let strokes = calls.borrow();
    let drags: Vec<&Vec<GestureSample>> = strokes.strokes.iter().map(|s| &s.1).collect();
    assert!(
        drags.len() > 1,
        "a field drag reached the model as {} call(s); this test is about what          the segments carry and needs more than one of them",
        drags.len()
    );

    let anchor = drags[0][0].position;
    for (index, samples) in drags.iter().enumerate() {
        assert_eq!(
            samples[0].position, anchor,
            "segment {index} starts at {:?} rather than the gesture's anchor \
             {anchor:?}, so the grab it writes is centred there and the engine \
             records it as a new one",
            samples[0].position
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
    println!("PADRAO during={}", calls.borrow().strokes.len());
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

/// A field drag is seen while it is made, not when the pointer comes up.
///
/// Reported from a session: *"on our app we only see the effect of the move
/// brush (in sdf) after the stroke finishes (after I leave the mouse button,
/// not while I'm dabbing)"*, with mesh mode behaving correctly.
///
/// `stamps_between_segments` asked the representation before it asked whether
/// the gesture replays, so every field gesture got `STAMPS_PER_SEGMENT` — three
/// stamps' worth of travel. That threshold exists because a *stamping* segment
/// costs a re-mesh of everything it touched and grows with the gesture. A
/// replayed one does not: the whole drag goes down from its anchor every time,
/// so the work is the same on the first segment and the fortieth.
///
/// A field's Move has replayed from its anchor since #99 *and* has a live
/// transaction armed for it before the first segment — `open_live_gesture`
/// routes `Mover` into `arm_live_move`, whose whole purpose is to draw the drag
/// while the pointer is down. None of it ran until the release.
///
/// The drag here is **0.08 world units**, far under the threshold at any
/// ordinary brush, and well under the 1.03 the default flow and a 0.858 brush
/// produce. Before the fix this reached the model zero times before `EndStroke`.
#[test]
fn a_short_field_drag_is_seen_before_the_pointer_comes_up() {
    let (mut vm, calls) = fixture_with(|model| {
        model.representation.set(Representation::Sdf);
    });
    vm.dispatch(Command::SelectTool(ToolKind::Mover))
        .expect("tool");
    vm.dispatch(Command::BeginStroke {
        position: [0.0, 0.0, 1.0],
        pressure: 1.0,
        modifiers: Default::default(),
    })
    .expect("begin");
    for step in 1..=8 {
        vm.dispatch(Command::ContinueStroke {
            position: [step as f32 * 0.01, 0.0, 1.0],
            pressure: 1.0,
        })
        .expect("continue");
    }

    // Read BEFORE the release, which is the whole point: after `EndStroke`
    // the drag arrives either way and the bug is invisible.
    let during = calls.borrow().strokes.len();
    assert_eq!(
        during, 8,
        "a 0.08-unit field drag reached the model {during} times before the \
         pointer came up, against one per pointer move. At 0 the sculptor \
         watches a still surface while dragging and the live Move transaction \
         armed for this gesture is never fed a segment to preview, which is \
         the reported fault"
    );
}

/// And a stamping field stroke still waits, which is what the threshold is for.
///
/// The companion that keeps the fix honest. If `stamps_between_segments`
/// returned zero for everything, a Padrão stroke would send a segment per
/// pointer move and re-mesh the same neighbourhood over and over — the cost
/// `STAMPS_PER_SEGMENT` exists to avoid, and the reason the original ordering
/// looked right.
///
/// **One, not zero.** The press applies a dab of its own before any movement —
/// "the first dab lands on the press rather than on the first move: a click is
/// a stroke too" — and that is not a segment. Measured across the change: this
/// verb sends 1 either way, where Move goes from 0 to 8.
#[test]
fn a_short_field_stamping_stroke_still_waits() {
    let (mut vm, calls) = fixture_with(|model| {
        model.representation.set(Representation::Sdf);
    });
    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("tool");
    vm.dispatch(Command::BeginStroke {
        position: [0.0, 0.0, 1.0],
        pressure: 1.0,
        modifiers: Default::default(),
    })
    .expect("begin");
    for step in 1..=8 {
        vm.dispatch(Command::ContinueStroke {
            position: [step as f32 * 0.01, 0.0, 1.0],
            pressure: 1.0,
        })
        .expect("continue");
    }

    assert_eq!(
        calls.borrow().strokes.len(),
        1,
        "a 0.08-unit stamping stroke sent more than the press's own dab before \
         travelling one stamp gap; the replay fast path has reached a verb \
         that does not replay, and every pointer move now costs a re-mesh"
    );
}

// -- which frequency a smooth acts on ----------------------------------------

/// Three smooths are a hierarchy's, and the control follows the layer.
///
/// Both halves of the question, because they fail differently: the other three
/// representations store one surface and therefore have one smooth, so a
/// three-way control over them would decide nothing; and with another tool in
/// hand there is no smooth to describe at all.
#[test]
fn the_smooth_mode_is_offered_only_on_a_hierarchy() {
    let (mut vm, representation) = fixture_with_layer_changes();
    vm.dispatch(Command::SelectTool(ToolKind::Suavizar))
        .expect("the smooth brush");
    assert!(
        !vm.offers_smooth_mode(),
        "a field stores one surface, so its smooth has one frequency"
    );

    representation.set(Representation::Multires);
    assert!(
        vm.offers_smooth_mode(),
        "a hierarchy stores the form and the detail apart, which is what makes \
         three smooths three operations rather than three strengths of one"
    );

    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("the standard brush");
    assert!(
        !vm.offers_smooth_mode(),
        "and a brush that does not smooth has no frequency to pick"
    );
}

/// What a sculptor who has not chosen gets, and where the choice lands.
///
/// The default is the claim the shelf's own note makes — the form corrected
/// under the detail, with the detail put back unchanged — and it is asserted
/// here rather than left to the document, because the bar shows what this
/// holds and the stroke uses what the document holds.
#[test]
fn the_smooth_mode_starts_at_the_one_the_note_promises() {
    let (mut vm, representation, mode) = fixture_with_a_smooth_mode();
    representation.set(Representation::Multires);
    assert_eq!(
        *vm.smooth_mode().get(),
        clayspace_model::SmoothFrequency::FormWithDetail
    );

    vm.dispatch(Command::SetSmoothMode(
        clayspace_model::SmoothFrequency::Form,
    ))
    .expect("the plain Laplacian is still reachable: sometimes the pores go");
    assert_eq!(
        *vm.smooth_mode().get(),
        clayspace_model::SmoothFrequency::Form
    );
    assert_eq!(
        mode.get(),
        clayspace_model::SmoothFrequency::Form,
        "and the document was told, or the bar is the only place the choice \
         exists"
    );
}

// -- the per-pass eraser -----------------------------------------------------

/// Erase is on a hierarchy's shelf, and it is the selected row that decides
/// whether it can be used.
///
/// Three states rather than two, because the middle one is the regression this
/// guards: the tool has to be *on the shelf* for a hierarchy at all — it was
/// bound to the grid alone and a sculptor with a pass stack was never offered
/// the one verb that takes a pass back — and it has to refuse, with words,
/// where the form is selected instead. A tool that were simply hidden on the
/// form would leave nobody to say why it had gone.
#[test]
fn erase_is_offered_on_a_hierarchy_with_a_pass() {
    let (mut vm, representation, in_a_pass) = fixture_with_a_pass_selection();
    representation.set(Representation::Multires);
    in_a_pass.set(true);

    vm.dispatch(Command::SelectTool(ToolKind::Apagar))
        .expect("a hierarchy's shelf carries the eraser");
    assert!(
        vm.tool_status().get().is_none(),
        "with a pass selected there is nothing to explain: {:?}",
        vm.tool_status().get()
    );
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("the erase reaches the model");

    // The form under the passes. The tool stays selected — it is the row that
    // moved, not the layer — and the status line is what carries the reason.
    in_a_pass.set(false);
    let refused = draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]])
        .expect_err("the form is not a pass, and erasing it is a different verb");
    assert!(
        refused.to_string().to_lowercase().contains("pass"),
        "the refusal has to name what a sculptor must select: {refused}"
    );
    assert!(
        vm.tool_status()
            .get()
            .as_deref()
            .is_some_and(|said| said.to_lowercase().contains("pass")),
        "and the status line has to say it without a stroke being attempted"
    );
}

/// The eraser on the other three representations is untouched.
///
/// The pass rule is a hierarchy's, and a grid has no passes at all — an eraser
/// that started asking a grid which row was selected would have vanished from
/// the one shelf it has always been on.
#[test]
fn a_grid_eraser_asks_about_no_pass() {
    let (mut vm, representation, in_a_pass) = fixture_with_a_pass_selection();
    representation.set(Representation::Voxel);
    in_a_pass.set(false);

    vm.dispatch(Command::SelectTool(ToolKind::Apagar))
        .expect("the grid's eraser");
    assert!(
        vm.tool_status().get().is_none(),
        "a grid stores cells and not passes: {:?}",
        vm.tool_status().get()
    );
    draw(&mut vm, &[[0.0; 3], [0.1, 0.0, 0.0]]).expect("the erase reaches the model");
}
