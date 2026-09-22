//! Undoing the structural and deformation commands the way the application
//! produces them.
//!
//! The engine records an entry for every one of these, and taking one back
//! through the engine worked. The application does not drive the engine: it
//! drives ViewModels, and the history a sculptor presses is the sculpting
//! ViewModel's — a stack of how many engine entries each *action* spent, where
//! one Cmd+Z pops one count and spends exactly that many.
//!
//! None of these pushed a count. So the next Cmd+Z popped the **previous**
//! command's count and spent it on entries belonging to the command that
//! banked nothing. Measured in the audit: add a subtool, insert a shape into
//! it, put a cage up, bend it, apply — then one undo, which took the *subtool*
//! away. Another sequence removed four layers with a single undo.
//!
//! These go through the same seam the composition root does — dispatch, then
//! bank what each ViewModel says the change cost — which is the seam nothing
//! tested.

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    GizmoHandle, GizmoTarget, InsertAs, LayerKey, Representation, SceneModel, Shape,
};
use clayspace_vm::{
    BooleanViewModel, Command, LatticeViewModel, ObjectViewModel, SceneViewModel, SculptViewModel,
};

struct Structural {
    document: SharedDocument,
    sculpt: SculptViewModel,
    scene: SceneViewModel,
    lattice: LatticeViewModel,
    objects: ObjectViewModel,
    boolean: BooleanViewModel,
}

impl Structural {
    fn new() -> Option<Self> {
        let policy = BackendPolicy::discover(None).ok()?;
        let document = SharedDocument::new(
            ClayDocument::new(policy)
                .and_then(ClayDocument::with_starting_form)
                .ok()?,
        );
        Some(Self {
            sculpt: SculptViewModel::new(Box::new(document.clone())),
            scene: SceneViewModel::new(Box::new(document.clone())),
            lattice: LatticeViewModel::new(Box::new(document.clone())),
            objects: ObjectViewModel::new(Box::new(document.clone())),
            boolean: BooleanViewModel::new(Box::new(document.clone())),
            document,
        })
    }

    /// One command through the ViewModels, banked as the composition root
    /// banks it — the same order, and every count taken once.
    fn apply(&mut self, command: Command) {
        let representation = self.sculpt.active_representation();
        if let Err(e) = self.scene.dispatch(&command) {
            eprintln!("{e}");
        }
        self.lattice.dispatch(&command, representation);
        self.objects.dispatch(&command, representation);
        self.boolean.dispatch(&command);
        let counts = self
            .scene
            .take_unbanked_actions()
            .into_iter()
            .chain(self.lattice.take_unbanked_actions())
            .chain(self.objects.take_unbanked_actions())
            .chain(self.boolean.take_unbanked_actions())
            .collect::<Vec<_>>();
        for entries in counts {
            self.sculpt.record_external_action(command.label(), entries);
        }
        self.settle();
    }

    fn settle(&mut self) {
        self.scene.refresh();
        self.objects.refresh();
        self.lattice.refresh();
    }

    /// What the interface offers to take back: actions, not engine entries.
    fn depth(&self) -> usize {
        self.sculpt.history().get().depth
    }

    fn undo(&mut self) {
        self.sculpt.dispatch(Command::Undo).expect("undo");
        self.settle();
    }

    fn layers(&self) -> Vec<LayerKey> {
        self.document
            .with(|d| SceneModel::scene(d).layers.iter().map(|l| l.key).collect())
    }

    /// Where the surface stands under a ray straight down -z, which is what a
    /// deformation moves.
    fn surface_height(&self) -> Option<f32> {
        self.document
            .with(|d| clayspace_model::SculptModel::pick(d, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]))
            .map(|hit| hit[2])
    }

    /// Puts a cage up, pulls one control point and lays the bend down.
    ///
    /// The sequence from the audit's repro, and the one the manipulator drives
    /// in the interface.
    fn bend_through_a_cage(&mut self) {
        self.apply(Command::ToggleLattice);
        assert!(
            self.lattice.state().get().active,
            "the cage did not go up, so nothing below measures a bend"
        );
        self.apply(Command::SelectLatticePoint(Some(0)));
        let at = self.lattice.state().get().points[0];
        self.apply(Command::DragLatticePoint([at[0] - 0.3, at[1], at[2]]));
        assert!(
            self.lattice.state().get().touched,
            "the drag moved no control point"
        );
        self.apply(Command::ApplyLattice);
    }
}

/// Each of these is exactly one thing the sculptor can take back.
///
/// The depth the interface reports is what the audit watched not move: "the
/// reported history depth never moves for these commands", four times over.
#[test]
fn each_structural_command_is_one_step_of_the_history_the_interface_reads() {
    let Some(mut app) = Structural::new() else {
        return;
    };
    let mut expected = app.depth();

    app.apply(Command::AddLayer(Representation::Sdf));
    expected += 1;
    assert_eq!(app.depth(), expected, "adding a subtool was not one step");

    app.apply(Command::SetInsertAs(InsertAs::Object));
    app.apply(Command::SetShape(Shape::Sphere));
    app.apply(Command::InsertShape);
    expected += 1;
    assert_eq!(app.depth(), expected, "placing a shape was not one step");

    app.apply(Command::SetGizmoTarget(Some(GizmoTarget::Layer(
        *app.layers().last().expect("the new subtool"),
    ))));
    app.apply(Command::BeginGizmoDrag(
        GizmoHandle::Centre,
        [0.0; 3],
        [0.0, 0.0, 1.0],
    ));
    for step in 1..=5 {
        app.apply(Command::DragGizmo([step as f32 * 0.05, 0.0, 0.0], false));
        assert_eq!(
            app.depth(),
            expected,
            "a frame of a drag was banked on its own, which is one undo per \
             pointer move"
        );
    }
    app.apply(Command::EndGizmoDrag);
    expected += 1;
    assert_eq!(
        app.depth(),
        expected,
        "the whole drag was not one thing to take back"
    );

    app.bend_through_a_cage();
    expected += 1;
    assert_eq!(app.depth(), expected, "applying a cage was not one step");

    app.apply(Command::RemoveLayer(
        *app.layers().last().expect("a subtool to remove"),
    ));
    expected += 1;
    assert_eq!(app.depth(), expected, "removing a subtool was not one step");
}

/// One undo after a bend takes the bend back and leaves every subtool alone.
///
/// The audit's headline: "layer 71: layer/add, solo, insert shape, cage up,
/// drag, lattice/apply, then one undo — the undo deleted layer 71".
#[test]
fn one_undo_after_a_bend_keeps_every_subtool() {
    let Some(mut app) = Structural::new() else {
        return;
    };
    app.apply(Command::AddLayer(Representation::Sdf));
    app.apply(Command::SetInsertAs(InsertAs::Object));
    app.apply(Command::SetShape(Shape::Sphere));
    app.apply(Command::InsertShape);
    let layers = app.layers();
    let before_the_bend = app.surface_height();

    app.bend_through_a_cage();
    let depth = app.depth();

    app.undo();
    assert_eq!(
        app.layers(),
        layers,
        "an undo meant for the bend took a subtool away — which is what an \
         undo spending a count that belongs to an earlier command does"
    );
    assert_eq!(
        app.depth(),
        depth - 1,
        "the undo did not spend exactly the bend's own step"
    );
    assert_eq!(
        app.surface_height(),
        before_the_bend,
        "the undo did not put the form back where the bend found it"
    );
}

/// An undo after adding a subtool removes that subtool and stops.
#[test]
fn one_undo_after_adding_a_subtool_removes_only_that_subtool() {
    let Some(mut app) = Structural::new() else {
        return;
    };
    let before = app.layers();

    app.apply(Command::AddLayer(Representation::Sdf));
    assert_eq!(app.layers().len(), before.len() + 1);

    app.undo();
    assert_eq!(
        app.layers(),
        before,
        "the undo after a layer was added did not leave the document as it \
         found it"
    );
}

/// A scripted session undone to the start, one command at a time.
///
/// The property the audit could not get: "no undo removes a layer that the
/// undone command did not create". Each step back is checked against the layer
/// set the document held before the command it is taking back, so an undo that
/// reaches one command too far is caught at the step it happens on rather than
/// at the end.
#[test]
fn a_mixed_session_undoes_one_command_at_a_time() {
    let Some(mut app) = Structural::new() else {
        return;
    };
    // The layer set before each command, so an undo can be held against the
    // document the command found rather than against a total at the end.
    let mut history: Vec<Vec<LayerKey>> = Vec::new();
    let start = app.layers();

    for round in 0..4 {
        for command in [
            Command::AddLayer(Representation::Sdf),
            Command::SetInsertAs(InsertAs::Object),
            Command::SetShape(if round % 2 == 0 {
                Shape::Sphere
            } else {
                Shape::Box
            }),
            Command::InsertShape,
        ] {
            let was = app.layers();
            let depth = app.depth();
            app.apply(command);
            if app.depth() > depth {
                history.push(was);
            }
        }
        let was = app.layers();
        let depth = app.depth();
        app.bend_through_a_cage();
        assert_eq!(app.depth(), depth + 1, "the bend was not one step");
        history.push(was);
    }

    assert_eq!(
        app.depth(),
        history.len(),
        "the reported depth and the commands that changed the document \
         disagree"
    );

    while let Some(was) = history.pop() {
        app.undo();
        assert_eq!(
            app.layers(),
            was,
            "an undo left the document with a different set of subtools than \
             the command it took back had found"
        );
    }
    assert_eq!(app.layers(), start, "the session did not undo to its start");
    assert_eq!(app.depth(), 0, "the history did not empty");
}
