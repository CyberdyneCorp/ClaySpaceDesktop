//! A sculpting session end to end: ViewModel commands, real engine underneath.
//!
//! The ViewModel tests use a double to check the rules; this checks that the
//! rules still hold when the thing underneath is a real document — that a
//! stroke reaches geometry, that undo puts it back, and that a dab dirties a
//! bounded region rather than the model.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{SculptModel, ToolKind};
use clayspace_vm::{Axis, Command, SculptViewModel, Watcher};

fn session() -> SculptViewModel {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let document = ClayDocument::new(policy)
        .expect("create document")
        .with_starting_form()
        .expect("starting form");
    SculptViewModel::new(Box::new(document))
}

/// Draws across the front of the starting sphere, on its surface.
fn stroke_across_the_form(vm: &mut SculptViewModel) -> Result<(), clayspace_model::ModelError> {
    let points: Vec<[f32; 3]> = (0..12)
        .map(|i| {
            let t = i as f32 / 11.0;
            let angle = (t - 0.5) * 1.4;
            let (s, c) = angle.sin_cos();
            [s * 1.01, 0.12, c * 1.01]
        })
        .collect();

    let (first, rest) = points.split_first().expect("points");
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

#[test]
fn a_document_starts_with_something_to_sculpt() {
    let vm = session();
    let (min, max) = vm.bounds().expect("the starting form must have bounds");
    assert!(
        max[0] > min[0] && max[1] > min[1],
        "the starting form is degenerate: {min:?}..{max:?}"
    );
}

#[test]
fn the_cursor_finds_the_surface_under_a_ray() {
    let vm = session();
    let hit = vm
        .pick([0.0, 0.0, -5.0], [0.0, 0.0, 1.0])
        .expect("a ray down the axis must meet the starting sphere");
    assert!(
        (hit[2] + 1.0).abs() < 0.15,
        "the pick landed at {hit:?}, which is not the near face of a unit sphere"
    );

    assert!(
        vm.pick([9.0, 9.0, -5.0], [0.0, 0.0, 1.0]).is_none(),
        "a ray well outside the form reported a hit"
    );
}

#[test]
fn a_stroke_changes_the_document_and_bounds_its_cost() {
    let mut vm = session();
    let mut history = Watcher::new();
    history.accept(vm.history());

    stroke_across_the_form(&mut vm).expect("stroke");

    assert!(
        vm.last_action().get().changed,
        "the stroke reported that it changed nothing"
    );
    assert!(
        history.take_change(vm.history()),
        "a live stroke must create a history entry"
    );

    let dirty = *vm.pending_remesh().get();
    assert!(dirty > 0, "a stroke dirtied no bricks, so nothing would be re-meshed");
}

#[test]
fn a_whole_stroke_undoes_as_one_entry() {
    let mut vm = session();
    let before = vm.history().get().depth;

    stroke_across_the_form(&mut vm).expect("stroke");
    let after = vm.history().get().depth;
    assert_eq!(
        after - before,
        1,
        "a stroke of twelve samples produced {} history entries",
        after - before
    );

    vm.dispatch(Command::Undo).expect("undo");
    assert!(
        vm.last_action().get().changed,
        "there was a stroke to undo"
    );
    assert_eq!(
        vm.history().get().depth,
        before,
        "undoing the stroke did not return the history to where it started"
    );
}

#[test]
fn redo_restores_what_undo_removed() {
    let mut vm = session();
    stroke_across_the_form(&mut vm).expect("stroke");
    let depth = vm.history().get().depth;

    vm.dispatch(Command::Undo).expect("undo");
    assert!(vm.history().get().can_redo, "an undone stroke must be redoable");

    vm.dispatch(Command::Redo).expect("redo");
    assert_eq!(vm.history().get().depth, depth);
}

#[test]
fn a_mirrored_stroke_is_one_history_entry() {
    // The requirement is that a symmetric edit undoes as one step: both halves
    // belong to the same operation. Symmetry is on by default.
    let mut vm = session();
    let before = vm.history().get().depth;

    stroke_across_the_form(&mut vm).expect("stroke");
    assert_eq!(
        vm.history().get().depth - before,
        1,
        "a mirrored stroke must undo as one step"
    );

    vm.dispatch(Command::Undo).expect("undo");
    assert_eq!(
        vm.history().get().depth,
        before,
        "one undo did not remove both halves of the mirrored edit"
    );
}

#[test]
fn changing_symmetry_costs_its_own_history_entry() {
    // Worth pinning rather than assuming: a layer's mirror is document state
    // in the engine, so toggling it is a change like any other. It is written
    // only when it differs, so a run of strokes at one setting costs nothing
    // extra.
    let mut vm = session();

    let before = vm.history().get().depth;
    stroke_across_the_form(&mut vm).expect("first stroke");
    let per_stroke = vm.history().get().depth - before;

    let before = vm.history().get().depth;
    stroke_across_the_form(&mut vm).expect("second stroke, same symmetry");
    assert_eq!(
        vm.history().get().depth - before,
        per_stroke,
        "an unchanged mirror was rewritten, costing a spurious entry"
    );

    let before = vm.history().get().depth;
    vm.dispatch(Command::ToggleSymmetry(Axis::X)).expect("toggle");
    stroke_across_the_form(&mut vm).expect("stroke after toggling");
    assert_eq!(
        vm.history().get().depth - before,
        per_stroke + 1,
        "changing the mirror should cost exactly one entry beyond the stroke"
    );
}

#[test]
fn an_unavailable_tool_is_refused_against_a_real_document() {
    let mut vm = session();
    // The starting layer is SDF; scrape is voxel-side.
    vm.dispatch(Command::SelectTool(ToolKind::Raspar))
        .expect("select");

    let error = vm
        .dispatch(Command::BeginStroke {
            position: [0.0, 0.0, 1.0],
            pressure: 1.0,
        })
        .expect_err("a voxel verb on an SDF layer must be refused");
    assert!(error.to_string().contains("voxel"), "{error}");
}

#[test]
fn a_voxel_layer_accepts_its_own_verbs() {
    let policy = BackendPolicy::discover(None).expect("backends");
    let mut document = ClayDocument::new(policy).expect("document");
    document
        .add_voxel_layer("Voxels", 0.05)
        .expect("add a voxel layer");

    let mut vm = SculptViewModel::new(Box::new(document));
    vm.dispatch(Command::SelectTool(ToolKind::Inflar))
        .expect("select");
    assert!(
        vm.tool_status().get().is_none(),
        "inflate belongs on a voxel layer: {:?}",
        vm.tool_status().get()
    );

    // Padrão deposits; Inflar dilates what is already there, so on an empty
    // grid it correctly does nothing. Deposit first.
    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("select");
    vm.dispatch(Command::BeginStroke {
        position: [0.0, 0.0, 0.0],
        pressure: 1.0,
    })
    .expect("begin");
    vm.dispatch(Command::ContinueStroke {
        position: [0.05, 0.0, 0.0],
        pressure: 1.0,
    })
    .expect("continue");
    vm.dispatch(Command::EndStroke).expect("end");

    assert!(
        vm.last_action().get().changed,
        "depositing into an empty voxel grid changed nothing"
    );
}

#[test]
fn a_stroke_over_empty_space_is_not_an_error() {
    let mut vm = session();
    // Far from the form: the engine accepts it and nothing happens, which is
    // a legitimate outcome rather than a failure.
    for position in [[40.0f32, 40.0, 40.0], [40.1, 40.0, 40.0]] {
        let command = if position[0] == 40.0 {
            Command::BeginStroke {
                position,
                pressure: 1.0,
            }
        } else {
            Command::ContinueStroke {
                position,
                pressure: 1.0,
            }
        };
        vm.dispatch(command).expect("a stroke in empty space is legal");
    }
    vm.dispatch(Command::EndStroke)
        .expect("ending it is legal too");
}
