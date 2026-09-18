//! Undoing mask work the way the application produces it.
//!
//! There were already undo tests over the mask, and they passed, and a mask
//! edit still could not be taken back. They drove the *engine*, where an
//! operation on a mask records an entry like any other edit — while the
//! application drives ViewModels, and the history a sculptor presses is the
//! sculpting ViewModel's: a stack of how many engine entries each action
//! spent, where an undo pops one count and spends exactly that many.
//!
//! Nothing was pushed for a mask edit. So the next Cmd+Z popped the *previous*
//! command's count, spent it on entries belonging to the mask, and the one
//! after that reached further still. Measured through the agent door before
//! this: `layer/remove; mask/apply clear; lasso; dab` and two undos walked
//! back seven entries and took two subtools with them.
//!
//! These go through the same seam the composition root does — `dispatch`, then
//! bank what the edit cost — which is the seam nothing tested.

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    ExtrudeSettings, MaskGesture, MaskOp, OutlineFrame, Representation, SceneModel, SculptModel,
    StrokeModifiers, ToolKind,
};
use clayspace_vm::{Command, MaskViewModel, SceneViewModel, SculptViewModel};

struct Masking {
    document: SharedDocument,
    sculpt: SculptViewModel,
    scene: SceneViewModel,
    mask: MaskViewModel,
}

impl Masking {
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
            mask: MaskViewModel::new(Box::new(document.clone())),
            document,
        })
    }

    /// One command through the mask ViewModel, banked as the composition root
    /// banks it.
    fn apply(&mut self, command: Command) {
        self.mask.dispatch(&command);
        for entries in self.mask.take_unbanked_actions() {
            self.sculpt.record_external_action(entries);
        }
    }

    /// What the interface offers to take back: actions, not engine entries.
    fn depth(&self) -> usize {
        self.sculpt.history().get().depth
    }

    fn undo(&mut self) {
        self.sculpt.dispatch(Command::Undo).expect("undo");
        self.mask.refresh();
        self.scene.refresh();
    }

    fn redo(&mut self) {
        self.sculpt.dispatch(Command::Redo).expect("redo");
        self.mask.refresh();
        self.scene.refresh();
    }

    fn layers(&self) -> usize {
        self.document.with(|d| SceneModel::scene(d).layers.len())
    }

    fn frozen_cells(&self) -> usize {
        self.mask.state().get().painted_cells
    }

    /// Where the surface stands under a ray straight down -z, which is what a
    /// stroke on it moves and an undo meant for the mask must not.
    fn surface_height(&self) -> Option<f32> {
        self.document
            .with(|d| SculptModel::pick(d, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]))
            .map(|hit| hit[2])
    }

    /// One dab with the tool in hand, through the ViewModel that banks it.
    fn dab(&mut self, tool: ToolKind) {
        let at = self
            .document
            .with(|d| SculptModel::pick(d, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]))
            .expect("the starting form is under the ray");
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

    /// A patch of the near face frozen, as the mask brush freezes it.
    fn paint_the_mask(&mut self) {
        self.sculpt
            .dispatch(Command::SetBrushSize(0.3))
            .expect("a brush wide enough to freeze a patch");
        self.dab(ToolKind::Mascara);
        self.mask.refresh();
        assert!(
            self.frozen_cells() > 0,
            "the fixture froze nothing, so nothing below measures anything"
        );
    }
}

/// Looking down -z, so the frame's x and y are the world's.
fn looking_down_z() -> OutlineFrame {
    OutlineFrame {
        origin: [0.0, 0.0, 0.0],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        forward: [0.0, 0.0, -1.0],
        scale: [1.0, 1.0],
    }
}

/// Every operation over the mask is one thing the sculptor can take back.
#[test]
fn each_mask_operation_is_one_step_of_the_history_the_interface_reads() {
    let Some(mut app) = Masking::new() else {
        return;
    };
    app.paint_the_mask();
    let mut expected = app.depth();

    for op in [
        MaskOp::Invert,
        MaskOp::Expand(2),
        MaskOp::Contract(2),
        MaskOp::Smooth(2),
        MaskOp::InvertWithinBounds,
        MaskOp::Clear,
    ] {
        app.apply(Command::ApplyMaskOp(op));
        expected += 1;
        assert_eq!(
            app.depth(),
            expected,
            "{op:?} banked {} steps; it has to be exactly one, or the next \
             Cmd+Z spends the previous command's count on the mask's entries",
            app.depth() as i64 - (expected as i64 - 1)
        );
    }

    // And the two gestures beside the menu: an outline drawn round a region,
    // and the wall pulled off the frozen patch.
    app.paint_the_mask();
    expected = app.depth();
    app.apply(Command::SetMaskGesture(MaskGesture::Lasso));
    app.apply(Command::BeginMaskOutline([-0.5, -0.5], false));
    for at in [[-0.1, -0.5], [-0.1, -0.1], [-0.5, -0.1]] {
        app.apply(Command::ExtendMaskOutline(at));
    }
    app.apply(Command::EndMaskOutline(looking_down_z()));
    assert_eq!(app.depth(), expected + 1, "an outline was not one step");

    app.apply(Command::ExtrudeMask(ExtrudeSettings {
        thickness: 0.2,
        ..ExtrudeSettings::default()
    }));
    assert_eq!(app.depth(), expected + 2, "an extrusion was not one step");
}

/// One undo after a mask operation takes back the operation and nothing else.
///
/// The shape the audit measured, in the order it measured it: a subtool
/// removed, a mask cleared, a region drawn, a dab on the clay. Before this, two
/// undos reached past all four.
#[test]
fn one_undo_after_a_mask_operation_leaves_the_layers_and_the_clay_alone() {
    let Some(mut app) = Masking::new() else {
        return;
    };
    app.document
        .with(|d| d.add_layer("Segunda", Representation::Sdf))
        .expect("a second subtool");
    app.scene.refresh();
    let key = app
        .document
        .with(|d| SceneModel::scene(d).layers.last().map(|layer| layer.key))
        .expect("the subtool that was just added");
    app.scene
        .dispatch(&Command::RemoveLayer(key))
        .expect("take it away again");
    let layers = app.layers();

    app.paint_the_mask();
    let frozen = app.frozen_cells();
    app.dab(ToolKind::Padrao);
    let sculpted = app.surface_height().expect("the form is under the ray");

    app.apply(Command::ApplyMaskOp(MaskOp::Clear));
    assert_eq!(app.frozen_cells(), 0, "the clear froze nothing free");

    app.undo();
    assert_eq!(
        app.frozen_cells(),
        frozen,
        "the undo after a clear did not put the frozen region back"
    );
    assert_eq!(
        app.layers(),
        layers,
        "an undo meant for a mask operation took a subtool away — which is \
         what an undo spending a count that belongs to an earlier command does"
    );
    assert_eq!(
        app.surface_height(),
        Some(sculpted),
        "an undo meant for a mask operation reached the clay under it"
    );

    // And forward again, which is the other half of taking it back.
    app.redo();
    assert_eq!(
        app.frozen_cells(),
        0,
        "the redo did not put the clear back on"
    );
    assert_eq!(app.layers(), layers, "the redo added a subtool of its own");
}

/// Clearing a mask that freezes nothing is not something to take back.
///
/// The menu entry is always there and pressing it on an empty mask does the
/// obvious nothing: nothing is written, nothing is banked, and the caller is
/// told rather than left to look for an entry that was never made.
#[test]
fn clearing_an_empty_mask_banks_nothing_and_says_so() {
    let Some(mut app) = Masking::new() else {
        return;
    };
    app.dab(ToolKind::Padrao);
    let depth = app.depth();

    app.apply(Command::ApplyMaskOp(MaskOp::Clear));
    assert_eq!(
        app.depth(),
        depth,
        "a clear with nothing to clear was banked as something to take back"
    );
    assert_eq!(
        app.mask.remark().get().as_deref(),
        Some("não havia máscara para limpar"),
        "nothing was said about a command that did nothing"
    );
    assert!(
        app.mask.notice().get().is_none(),
        "clearing nothing was reported as a refusal"
    );
}
