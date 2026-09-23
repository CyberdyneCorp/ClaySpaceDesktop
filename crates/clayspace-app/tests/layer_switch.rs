//! A layer switch is atomic: every ViewModel sees the new subtool at once.
//!
//! Reported from a session, and measured three times in the voxel area and
//! three more in the coverage pass: the first stroke after *any* switch was
//! made with the settings of the subtool the sculptor had just left. A new
//! grid layer inherited a field layer's brush size of 100, so the first dab on
//! it came out a metre across. A new armature layer inherited the previous
//! subtool's mirror, so the rig grew a second arm. `state.mask.present` stayed
//! true on a subtool that has no mask at all.
//!
//! One cause under all of it. `Command::SelectLayer` reached the followers
//! before it reached the scene: the sculpting ViewModel asks the document
//! which representation is active to decide what the shelf offers and which
//! brush settings to restore, and it asked *before* the scene ViewModel had
//! moved the active layer. Each follower therefore set itself up for the
//! outgoing subtool, and the next command was the first one to see a
//! consistent document — which is why the error was always exactly one switch
//! behind.
//!
//! Two instruments here, because the defect has two halves:
//!
//! - the **followers** have to describe the new subtool once the document has
//!   moved, which is behaviour and is measured against a real document below;
//! - the **order** has to put the scene first, which is a property of
//!   `App::dispatch_to_models` — and `App` lives in the binary rather than the
//!   library, so it is read as source, the same instrument
//!   `document_replaced_refreshes` points at `after_document_replaced`. A
//!   rename would confuse it, and it is still worth more than a test that
//!   cannot fail.

use std::cell::RefCell;
use std::rc::Rc;

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, EditOutcome, GestureSample, HistoryState, ModelError, Representation,
    SceneModel, SceneStats, SculptModel, StrokeModifiers, ToolKind,
};
use clayspace_vm::{Command, MaskViewModel, SceneViewModel, SculptViewModel};

// -- what a stroke was made with ----------------------------------------------

/// Every stroke the document was handed: the layer it landed on and the radius
/// it was made at.
type Stamps = Rc<RefCell<Vec<(Representation, f32)>>>;

/// The shared document, with a note taken of each stroke on its way through.
///
/// What the options bar *shows* is one number and what a stroke is *made
/// with* is another, and the defect was the two disagreeing — so the radius is
/// read at the last point before the engine rather than off the ViewModel.
/// Every other call passes straight through, so the document behaves exactly
/// as the composition root's does.
struct Recording {
    document: SharedDocument,
    stamps: Stamps,
}

impl SculptModel for Recording {
    fn active_representation(&self) -> Representation {
        self.document.active_representation()
    }
    fn active_layer_editable(&self) -> bool {
        self.document.active_layer_editable()
    }
    fn active_layer_carries_geometry(&self) -> bool {
        self.document.active_layer_carries_geometry()
    }
    fn active_layer_visible(&self) -> bool {
        self.document.active_layer_visible()
    }
    fn active_layer_is_caged(&self) -> bool {
        self.document.active_layer_is_caged()
    }
    fn active_layer_stroke_lands_in_a_pass(&self) -> bool {
        self.document.active_layer_stroke_lands_in_a_pass()
    }
    fn apply_operation(
        &mut self,
        operation: clayspace_model::LayerOperation,
    ) -> Result<EditOutcome, ModelError> {
        self.document.apply_operation(operation)
    }
    fn apply_stroke(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        self.stamps
            .borrow_mut()
            .push((self.document.active_representation(), brush.size));
        self.document.apply_stroke(tool, brush, samples, symmetry)
    }
    fn symmetry(&self) -> [bool; 3] {
        SculptModel::symmetry(&self.document)
    }
    fn set_symmetry(&mut self, symmetry: [bool; 3]) -> Result<(), ModelError> {
        self.document.set_symmetry(symmetry)
    }
    fn set_combine(&mut self, combine: clayspace_model::CombineSettings) {
        self.document.set_combine(combine);
    }
    fn combine(&self) -> clayspace_model::CombineSettings {
        self.document.combine()
    }
    fn smooth_mode(&self) -> clayspace_model::SmoothFrequency {
        self.document.smooth_mode()
    }
    fn set_smooth_mode(&mut self, mode: clayspace_model::SmoothFrequency) {
        self.document.set_smooth_mode(mode);
    }
    fn set_colour(&mut self, colour: clayspace_model::Colour) {
        self.document.set_colour(colour);
    }
    fn choose_recent_colour(&mut self, index: usize) -> bool {
        self.document.choose_recent_colour(index)
    }
    fn colour_state(&self) -> clayspace_model::ColourState {
        self.document.colour_state()
    }
    fn set_alpha(&mut self, alpha: Option<clayspace_model::Alpha>) {
        self.document.set_alpha(alpha);
    }
    fn alpha_name(&self) -> Option<String> {
        self.document.alpha_name()
    }
    fn pick(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]> {
        SculptModel::pick(&self.document, origin, direction)
    }
    fn undo(&mut self) -> Result<bool, ModelError> {
        SculptModel::undo(&mut self.document)
    }
    fn redo(&mut self) -> Result<bool, ModelError> {
        SculptModel::redo(&mut self.document)
    }
    fn history(&self) -> HistoryState {
        SculptModel::history(&self.document)
    }
    fn stats(&self) -> SceneStats {
        self.document.stats()
    }
    fn begin_gesture(&mut self) {
        self.document.begin_gesture();
    }
    fn end_gesture(&mut self) {
        self.document.end_gesture();
    }
    fn open_live_gesture(&mut self, tool: ToolKind, symmetry: [bool; 3]) -> bool {
        self.document.open_live_gesture(tool, symmetry)
    }
    fn close_live_gesture(&mut self) -> Result<usize, ModelError> {
        self.document.close_live_gesture()
    }
    fn discard_live_gesture(&mut self) -> usize {
        self.document.discard_live_gesture()
    }
    fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        SculptModel::bounds(&self.document)
    }
}

// -- the followers, against a real document ----------------------------------

/// The three ViewModels a switch has to move together, over one document.
///
/// The composition root's own arrangement without the window: the same
/// `SharedDocument` behind each of them, which is what makes the order below
/// observable at all.
struct Switching {
    document: SharedDocument,
    stamps: Stamps,
    sculpt: SculptViewModel,
    scene: SceneViewModel,
    mask: MaskViewModel,
}

impl Switching {
    fn new() -> Option<Self> {
        let policy = BackendPolicy::discover(None).ok()?;
        let document = SharedDocument::new(
            ClayDocument::new(policy)
                .and_then(ClayDocument::with_starting_form)
                .ok()?,
        );
        let stamps = Stamps::default();
        Some(Self {
            sculpt: SculptViewModel::new(Box::new(Recording {
                document: document.clone(),
                stamps: stamps.clone(),
            })),
            stamps,
            scene: SceneViewModel::new(Box::new(document.clone())),
            mask: MaskViewModel::new(Box::new(document.clone())),
            document,
        })
    }

    /// One command, dispatched the way `App::dispatch_to_models` dispatches
    /// it: the scene first, because it is the one that moves the active layer,
    /// then the followers that read it.
    fn apply(&mut self, command: Command) {
        self.scene.dispatch(&command).expect("the scene");
        self.sculpt.dispatch(command.clone()).expect("the sculptor");
        self.mask.dispatch(&command);
    }

    fn active_representation(&self) -> Representation {
        self.document
            .with(|d| SculptModel::active_representation(d))
    }

    fn keys(&self) -> Vec<clayspace_model::LayerKey> {
        self.document
            .with(|d| SceneModel::scene(d).layers.iter().map(|l| l.key).collect())
    }

    /// One dab with the tool in hand, on the starting form under the ray.
    fn dab(&mut self, tool: ToolKind) {
        let at = self
            .document
            .with(|d| SculptModel::pick(d, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]))
            .expect("the starting form is under the ray");
        self.dab_at(tool, at);
    }

    /// One dab at a point, for a layer with nothing on it yet to pick.
    fn dab_at(&mut self, tool: ToolKind, at: [f32; 3]) {
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
}

/// A field-sized brush is not a grid-sized one, and the switch is where the
/// two are told apart.
#[test]
fn the_first_stroke_after_a_switch_uses_the_new_layers_brush() {
    let Some(mut app) = Switching::new() else {
        eprintln!("no backend on this machine; skipping");
        return;
    };
    assert_eq!(app.active_representation(), Representation::Sdf);
    app.sculpt
        .dispatch(Command::SelectTool(ToolKind::Suavizar))
        .expect("smooth has a verb on both");
    app.sculpt
        .dispatch(Command::SetBrushSize(0.4))
        .expect("a field-sized brush");

    // A new grid layer arrives *active*, which is the switch the report was
    // made on: `layer add {kind:'grid'}` and then a stroke.
    app.apply(Command::AddLayer(Representation::Voxel));
    assert_eq!(
        app.active_representation(),
        Representation::Voxel,
        "the new grid layer is the one a stroke would land on"
    );

    assert_ne!(
        app.sculpt.brush().get().size,
        0.4,
        "the grid layer is holding the field layer's brush size, so the size \
         the options bar shows is not the size the next dab is made at"
    );
    let on_the_grid = app.sculpt.brush().get().size;

    // And back, which must return the field layer's own size rather than
    // carry the grid's along.
    let field = app.keys()[0];
    app.apply(Command::SelectLayer(field));
    assert_eq!(
        app.sculpt.brush().get().size,
        0.4,
        "the field layer's brush did not come back; it is showing {on_the_grid}"
    );
}

/// The radius a stroke is *made at* follows the layer it lands on.
///
/// The test above reads the options bar; this reads the call the engine was
/// handed, across a field and a grid set to different sizes, there and back
/// twice. A grid dab made at the field's size is the metre-wide dab of the
/// report, and it is the stroke — not the bar — that makes it.
#[test]
fn the_stamp_radius_used_is_the_target_layers_setting() {
    let Some(mut app) = Switching::new() else {
        eprintln!("no backend on this machine; skipping");
        return;
    };
    let field = app.keys()[0];
    app.sculpt
        .dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("Padrão is on both");
    app.sculpt
        .dispatch(Command::SetBrushSize(0.4))
        .expect("a field-sized brush");

    app.apply(Command::AddLayer(Representation::Voxel));
    let grid = *app.keys().last().expect("the grid layer");
    assert_eq!(
        app.sculpt.brush().get().size,
        BrushSettings::default_for(Representation::Voxel).size,
        "a grid nothing has been set on starts at the grid's own default"
    );
    app.sculpt
        .dispatch(Command::SetBrushSize(0.1))
        .expect("a grid-sized brush");

    let at = [0.0, 0.0, 1.0];
    for _ in 0..2 {
        app.dab_at(ToolKind::Padrao, at);
        app.apply(Command::SelectLayer(field));
        app.dab_at(ToolKind::Padrao, at);
        app.apply(Command::SelectLayer(grid));
    }

    let stamps = app.stamps.borrow();
    assert!(!stamps.is_empty(), "no stroke reached the document");
    for (on, radius) in stamps.iter() {
        let expected = match on {
            Representation::Voxel => 0.1,
            _ => 0.4,
        };
        assert_eq!(
            *radius,
            expected,
            "a stroke on a {} layer was made at {radius}, the other layer's size",
            on.label()
        );
    }
    let layers: Vec<_> = stamps.iter().map(|(on, _)| *on).collect();
    assert!(
        layers.contains(&Representation::Voxel) && layers.contains(&Representation::Sdf),
        "the strokes did not land on both layers, so this measured one: {layers:?}"
    );
}

/// A mask belongs to a subtool, so a subtool with none has none to report.
///
/// Choosing a layer is not an edit, so the refresh that follows every
/// document-touching command never ran for a switch: `state.mask.present`
/// stayed true on the new subtool, describing the mask of the one left behind.
#[test]
fn mask_state_follows_the_active_layer() {
    let Some(mut app) = Switching::new() else {
        eprintln!("no backend on this machine; skipping");
        return;
    };
    app.sculpt
        .dispatch(Command::SetBrushSize(0.3))
        .expect("a brush wide enough to freeze a patch");
    app.dab(ToolKind::Mascara);
    app.mask.refresh();
    assert!(
        app.mask.state().get().is_active(),
        "nothing was frozen, so nothing below measures anything"
    );

    app.apply(Command::AddLayer(Representation::Voxel));

    assert!(
        !app.mask.state().get().present,
        "the new subtool reports a mask that belongs to the subtool before it"
    );
}

// -- the order, read from the composition root -------------------------------

fn main_source() -> String {
    std::fs::read_to_string("src/main.rs").expect("the app's own source")
}

/// The body of an `App` method, as text.
///
/// To the next method at the same indentation, which is where the body ends.
/// Naive, and it fails loudly rather than silently: an empty body fails every
/// assertion made about it.
fn body_of(signature: &str) -> String {
    let source = main_source();
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("{signature} is where this rule lives"));
    let rest = &source[start..];
    let end = rest[1..]
        .find("\n    fn ")
        .or_else(|| rest[1..].find("\n    pub fn "))
        .map(|at| at + 1)
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

/// Where a call to a ViewModel sits in a body, or a failure naming it.
fn position_of(body: &str, call: &str) -> usize {
    body.find(call)
        .unwrap_or_else(|| panic!("{call} is not in the body this rule is about"))
}

/// The scene moves the active layer; everything else here reads it.
#[test]
fn every_view_model_sees_the_new_active_layer_after_a_switch() {
    let body = body_of("fn dispatch_to_models(&mut self, command: &Command)");
    let scene = position_of(&body, "self.scene.dispatch(command)");

    // Each of these reads the document's active layer while it handles the
    // command, so each of them is wrong if it acts before the scene has moved
    // it. The sculptor decides the shelf and the brush from it; the cage and
    // the manipulator are handed the active representation outright; the mask
    // is re-read from the active layer's own frozen cells.
    //
    // Named by the field rather than by the whole call, because the call is
    // wrapped by the formatter and the field is not.
    let followers = [
        ("the sculpting ViewModel", "self.sculpt.dispatch("),
        ("the mask ViewModel", "self.mask.dispatch("),
        ("the cage ViewModel", "self.lattice"),
        ("the manipulator ViewModel", "self.objects"),
    ];

    let mut early = Vec::new();
    for (who, field) in followers {
        if position_of(&body, field) < scene {
            early.push(format!("  {who}, as `{field}`"));
        }
    }

    assert!(
        early.is_empty(),
        "these ViewModels read the active layer before the scene ViewModel has \
         moved it, so a switch leaves each of them set up for the subtool that \
         was left — the first stroke after every switch is made with the \
         previous subtool's brush, tool and mirror:\n{}\n\nDispatch \
         `self.scene.dispatch(command)` first.",
        early.join("\n")
    );
}

/// A rig layer is a switch nothing announces.
///
/// `begin_armature` adds the armature's own layer and turns that layer's
/// mirror off, and no `SelectLayer` passes through `dispatch` to say so. Left
/// unrefreshed, the options bar and the next stroke keep the previous
/// subtool's mirror — which, since `add_zsphere` places the reflected node
/// itself, hung a second arm off the first.
#[test]
fn a_new_rig_layer_uses_its_own_symmetry() {
    let source = main_source();
    let start = source
        .find("Command::NewArmature => {")
        .expect("starting a rig is handled in the composition root");
    let arm = &source[start
        ..start
            + source[start..]
                .find("\n            }")
                .expect("the arm ends")];

    assert!(
        arm.contains("self.armature.begin(at);"),
        "this is no longer the arm that starts a rig"
    );
    assert!(
        arm.contains("self.sculpt.refresh_for_active_layer();"),
        "starting a rig makes a layer of its own active, with symmetry off, \
         and nothing tells the sculpting ViewModel — so the first ZSphere is \
         placed with the previous subtool's mirror"
    );
}
