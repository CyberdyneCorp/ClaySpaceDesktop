//! Undoing a grid's passes and a rebuild the way the application produces
//! them.
//!
//! There were already undo tests over both, and they passed, and neither could
//! be taken back from the interface. They drove the *document*, where a rebuild
//! records an engine entry like any other edit — while the application drives
//! ViewModels, and the history a sculptor presses is the sculpting ViewModel's:
//! a stack of how many entries each action spent, where one Cmd+Z pops one
//! count and spends exactly that many.
//!
//! Nothing was pushed for either. So the next Cmd+Z popped the *previous*
//! command's count and spent it on entries that belonged to the rebuild, and
//! the one after that reached further still. Measured through the agent door
//! before this: a rebuild left the history depth where it was, and the undo
//! after it removed two subtools the rebuild had never touched.
//!
//! Dialling a pass is the other half and is not the same defect. The engine
//! records nothing at all for one — a recompose is a replay and a replay is not
//! an edit — so the way back is the document's own, and there was none.
//!
//! These go through the same seam the composition root does: run the operation
//! through the scene ViewModel, then bank what it cost.

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Direction, LayerKey, RemeshSettings, Representation, SceneModel, SculptLayerOp, SculptModel,
    StrokeModifiers, ToolKind,
};
use clayspace_vm::{Command, SceneViewModel, SculptViewModel};

struct Sculpting {
    document: SharedDocument,
    sculpt: SculptViewModel,
    scene: SceneViewModel,
}

impl Sculpting {
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
            document,
        })
    }

    /// What the scene ViewModel's operations cost, banked as the composition
    /// root banks them. See `App::bank_scene_actions`.
    fn bank(&mut self) {
        for entries in self.scene.take_unbanked_actions() {
            self.sculpt.record_external_action("grid pass", entries);
        }
    }

    /// What the interface offers to take back: actions, not engine entries.
    fn depth(&self) -> usize {
        self.sculpt.history().get().depth
    }

    fn undo(&mut self) {
        self.sculpt.dispatch(Command::Undo).expect("undo");
        self.scene.refresh();
    }

    fn redo(&mut self) {
        self.sculpt.dispatch(Command::Redo).expect("redo");
        self.scene.refresh();
    }

    fn layers(&self) -> Vec<LayerKey> {
        self.document
            .with(|d| SceneModel::scene(d).layers.iter().map(|l| l.key).collect())
    }

    /// Where the surface stands under a ray straight down -z, which is what
    /// dialling a pass moves and an undo meant for something else must not.
    fn surface_height(&self) -> Option<f32> {
        self.document
            .with(|d| SculptModel::pick(d, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]))
            .map(|hit| hit[2])
    }

    /// How far the grid's first pass is dialled in.
    fn pass_strength(&self) -> Option<f32> {
        self.document.with(|d| {
            SceneModel::scene(d)
                .layers
                .iter()
                .find_map(|layer| layer.sculpt_layers.first().map(|pass| pass.strength))
        })
    }

    /// One dab with the tool in hand, through the ViewModel that banks it.
    fn dab(&mut self, tool: ToolKind) {
        let at = self
            .document
            .with(|d| SculptModel::pick(d, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]))
            .expect("the form is under the ray");
        self.sculpt
            .dispatch(Command::SelectTool(tool))
            .expect("the tool");
        self.sculpt
            .dispatch(Command::BeginStroke {
                position: at,
                pressure: 1.0,
                modifiers: StrokeModifiers::default(),
            })
            .expect("a press");
        self.sculpt
            .dispatch(Command::ContinueStroke {
                position: at,
                pressure: 1.0,
            })
            .expect("a sample");
        self.sculpt.dispatch(Command::EndStroke).expect("a release");
    }

    /// A crossing, banked as `App::run_conversion` banks it.
    fn cross_into(&mut self, direction: Direction) {
        let before = self.document.with(|d| SceneModel::history_depth(d));
        self.document
            .with(|d| d.convert_layer(direction, 0.05, 0))
            .expect("the crossing");
        let spent = self.document.with(|d| SceneModel::history_depth(d)) - before;
        self.sculpt.record_external_action("convert", spent);
        self.scene.refresh();
    }

    /// A grid with one recorded pass that visibly moved the surface.
    fn grid_with_a_pass(&mut self) {
        self.cross_into(Direction::SdfToVoxel);
        self.pass_op(SculptLayerOp::BeginRecording {
            name: "primeiro".into(),
        });
        self.dab(ToolKind::Padrao);
        self.pass_op(SculptLayerOp::EndRecording);
    }

    fn pass_op(&mut self, op: SculptLayerOp) {
        self.scene.apply_grid_pass_op(op).expect("the operation");
        self.bank();
        self.scene.refresh();
    }
}

/// Dialling a pass is one step of the history the interface reads, and one
/// Cmd+Z puts it back.
///
/// Measured in the audit before this: `set_strength` was not undoable at all,
/// the undo after it took back the stroke that came before, and strokes already
/// taken back came up with it.
#[test]
fn dialling_a_pass_is_one_step_and_one_undo_puts_it_back() {
    let Some(mut app) = Sculpting::new() else {
        return;
    };
    app.grid_with_a_pass();
    let layers = app.layers();
    let dialled_in = app.pass_strength().expect("the grid records a pass");
    let sculpted = app.surface_height().expect("the form is under the ray");
    let depth = app.depth();

    app.pass_op(SculptLayerOp::SetStrength {
        index: 0,
        strength: 0.0,
    });
    assert_eq!(
        app.depth(),
        depth + 1,
        "dialling a pass banked {} steps; it has to be exactly one, or the \
         next Cmd+Z spends the previous command's count on it",
        app.depth() as i64 - depth as i64
    );
    assert_eq!(app.pass_strength(), Some(0.0), "the pass was not dialled");

    app.undo();
    assert_eq!(
        app.pass_strength(),
        Some(dialled_in),
        "the undo after dialling a pass did not put the strength back"
    );
    assert_eq!(
        app.surface_height(),
        Some(sculpted),
        "the undo after dialling a pass did not put the surface back"
    );
    assert_eq!(
        app.layers(),
        layers,
        "an undo meant for a pass took a subtool away — which is what an undo \
         spending a count that belongs to an earlier command does"
    );
    assert_eq!(app.depth(), depth, "the undo spent more than one step");

    // And forward again, which is the other half of taking it back.
    app.redo();
    assert_eq!(
        app.pass_strength(),
        Some(0.0),
        "the redo did not put the dialling back on"
    );
    assert_eq!(app.layers(), layers, "the redo added a subtool of its own");
}

/// Rebuilding a mesh layer's topology is one step, and one Cmd+Z puts it back.
///
/// The engine records the rebuild as a single entry; nothing above banked it.
/// Measured in the audit: the history depth was unchanged after a rebuild, and
/// the next undo removed two subtools.
#[test]
fn a_rebuild_is_one_step_and_one_undo_leaves_the_subtools_standing() {
    let Some(mut app) = Sculpting::new() else {
        return;
    };
    app.dab(ToolKind::Padrao);
    let sculpted = app.layers()[0];
    app.document
        .with(|d| d.add_layer("Segunda", Representation::Sdf))
        .expect("a second subtool");
    app.sculpt.record_external_action("add subtool", 1);
    app.scene.refresh();
    // Back onto the form, because adding a subtool selects the empty one it
    // made and a crossing reads whichever layer is active.
    app.scene
        .dispatch(&Command::SelectLayer(sculpted))
        .expect("the form again");
    app.cross_into(Direction::SdfToMesh);

    let layers = app.layers();
    let key = app
        .document
        .with(|d| SceneModel::scene(d).active)
        .expect("the mesh the crossing made");
    let depth = app.depth();

    app.scene
        .remesh(
            key,
            RemeshSettings {
                // Coarser than the default, so the rebuild visibly replaces
                // the topology rather than landing back on it.
                resolution: 48,
                ..RemeshSettings::default()
            },
        )
        .expect("the rebuild");
    app.bank();
    app.scene.refresh();
    assert_eq!(
        app.depth(),
        depth + 1,
        "a rebuild banked {} steps; it has to be exactly one",
        app.depth() as i64 - depth as i64
    );

    app.undo();
    assert_eq!(
        app.layers(),
        layers,
        "an undo meant for a rebuild took subtools away"
    );
    assert_eq!(app.depth(), depth, "the undo spent more than one step");
}
