//! The seam between the protocol and the running application.
//!
//! Everything above this file is protocol and can be tested against a fake.
//! Everything below it is the composition root's business, and every method
//! here is called **on the interface thread**, between frames — because
//! `Observable` holds a `Cell` and the engine's safe wrapper is `!Sync`, so a
//! server thread cannot hold either. That is a borrow-check fact rather than a
//! convention, which is why the seam is shaped this way and not as a shared
//! reference behind a mutex.
//!
//! The values that cross are plain. No JSON reaches the composition root and
//! no ViewModel type reaches the wire: a domain type given a `Serialize`
//! derive is a domain type whose internal shape has become a contract with
//! every client, and this workspace refactors those freely.

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;

use clayspace_vm::Command;

/// What the application can be asked, on the interface thread.
///
/// A `&mut self` method may change the document; a `&self` method may not, and
/// in particular may not mark any `Observable` as changed — an agent polling
/// the session must not be the reason an idle application never sleeps.
pub trait Session {
    /// Applies one command, exactly as a menu item's click would.
    fn apply(&mut self, command: Command) -> Result<Applied, Refusal>;

    /// Reads state without changing anything.
    ///
    /// `&mut` because asking the engine where a layer stands borrows the
    /// document mutably — the ABI has no shared reader for it. It is still a
    /// read: nothing here may mark an `Observable` as changed, or an agent
    /// polling the session becomes the reason an idle application never
    /// sleeps.
    fn read(&mut self, query: StateQuery) -> StateReport;

    /// Renders one frame and hands back its pixels.
    fn capture(&mut self, request: CaptureRequest) -> Result<Frame, Refusal>;

    /// Waits for pending re-meshing, jobs and maintenance, up to a bound.
    fn settle(&mut self, budget: Duration) -> Settled;

    /// Applies one command with the clock around it.
    fn measure(&mut self, command: Command) -> Result<Measured, Refusal>;

    /// Asks the person at the window, or reads a recorded opt-in.
    fn consent(&mut self, ask: &Consent) -> ConsentOutcome;

    /// Whether a person is holding a stroke, a drag or an outline right now.
    fn gesture_in_progress(&self) -> bool;

    /// Whether the gesture that is open is the agent's own.
    ///
    /// Apart from [`Session::gesture_in_progress`] because the two carry
    /// different rules. A person's gesture refuses everything that would
    /// change the document: an agent must not land an edit in the middle of
    /// somebody's stroke. The agent's own gesture refuses everything *but* the
    /// verbs that finish it — it opened the gesture, so it has to be able to
    /// close it, and nothing else it might send belongs inside one.
    ///
    /// Provided, so a double that models no agent gesture behaves as it always
    /// did.
    fn agent_gesture_in_progress(&self) -> bool {
        false
    }
}

/// What applying a command did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Applied {
    /// The command's own label, in the words the interface uses for it.
    pub label: String,
    /// Whether this reached the document, as opposed to the view or a panel.
    pub touched_document: bool,
    /// The edit history's depth afterwards, so an agent can tell that one
    /// gesture became one entry.
    pub history_depth: usize,
    /// What the next undo would undo, in words, or none where there is
    /// nothing to undo.
    pub undoes: Option<String>,
    /// Anything the application would have shown the person: a substituted
    /// tool, a refused cage, a memory warning.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
}

/// Why something was refused.
///
/// Two fields and not one. The code is stable and is what an agent branches
/// on; the message is the interface's own words in the interface's own
/// language, and is what an agent repeats to a person.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Refusal {
    pub code: RefusalCode,
    pub message: String,
    /// Where a gate is what refused, which one, so that "ask the person" is
    /// actionable rather than a guess.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate: Option<GateKind>,
}

impl Refusal {
    pub fn new(code: RefusalCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            gate: None,
        }
    }

    pub fn gated(gate: GateKind, message: impl Into<String>) -> Self {
        Self {
            code: RefusalCode::ConsentRequired,
            message: message.into(),
            gate: Some(gate),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// The stable half of a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    /// The group has no such action.
    UnknownAction,
    /// An argument is missing, of the wrong type, or out of range.
    BadArgument,
    /// The action exists and the current state does not allow it — the wrong
    /// representation, nothing selected, an empty history.
    Unavailable,
    /// A person is holding a gesture.
    GestureInProgress,
    /// The operation can destroy work and nobody has consented to it.
    ConsentRequired,
    /// The person was asked and refused.
    ConsentRefused,
    /// The person was asked and did not answer within the bound.
    ConsentTimedOut,
    /// The Model refused it, and the message is the Model's own.
    ModelRefused,
    /// The action is real and deliberately not offered here.
    NotOffered,
    /// Something failed that was expected to work.
    Failed,
}

/// A kind of operation that can destroy work.
///
/// Consent is recorded per kind, which is what stops one agreed export
/// standing for every later one while still letting a person say "exports are
/// fine" once and mean it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    /// Writing over a file that exists.
    Overwrite,
    /// Writing a mesh or an image out of the document.
    Export,
    /// Reading a document or a mesh in over what is open.
    Open,
    /// Losing an unsaved document — a new document, or a close.
    DiscardUnsaved,
    /// Removing something the edit history cannot bring back.
    IrreversibleRemoval,
    /// Closing the application.
    Quit,
}

impl GateKind {
    /// Every kind, so that a caller mapping these onto its own enumeration can
    /// check the two agree rather than assume it.
    pub const ALL: [GateKind; 6] = [
        Self::Overwrite,
        Self::Export,
        Self::Open,
        Self::DiscardUnsaved,
        Self::IrreversibleRemoval,
        Self::Quit,
    ];

    /// The word recorded in the session store, one kind per line.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Overwrite => "sobrescrever",
            Self::Export => "exportar",
            Self::Open => "abrir",
            Self::DiscardUnsaved => "descartar",
            Self::IrreversibleRemoval => "remover",
            Self::Quit => "sair",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.tag() == tag)
    }
}

/// What is being asked of the person at the window.
#[derive(Debug, Clone, PartialEq)]
pub struct Consent {
    /// Names this ask across the several calls it takes to answer one.
    ///
    /// The ask cannot be answered inside one call: [`Session::consent`] runs
    /// on the interface thread, and an interface thread waiting thirty seconds
    /// for somebody to click is an interface that has stopped drawing. So the
    /// first call raises the ask and answers [`ConsentOutcome::Pending`], the
    /// connection thread comes back, and this is what says it is the same
    /// question.
    pub id: u64,
    pub gate: GateKind,
    /// The operation in the words the interface uses for it.
    pub operation: String,
    /// Which client is asking, so the person is not consenting into the dark.
    pub client: String,
    /// The path involved, where one is.
    pub path: Option<PathBuf>,
    /// How long the ask may stand before it is refused. An unanswered prompt
    /// on an unattended machine must not hold a connection open.
    pub bound: Duration,
}

/// How an ask was answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentOutcome {
    /// The ask is up at the window and nobody has answered it yet. Come back.
    Pending,
    /// The person agreed, at the window, to this operation.
    Granted,
    /// An opt-in for this kind was already recorded.
    AlreadyRecorded,
    /// The person refused.
    Refused,
    /// Nobody answered within the bound.
    TimedOut,
}

/// Which parts of the session to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StateQuery {
    pub document: bool,
    pub scene: bool,
    pub tool: bool,
    /// Everything the brush panel holds that the tool section does not: the
    /// flow, the six dynamics, the shaping controls and what a drag reads.
    pub brush: bool,
    /// How a stroke or a placed form meets what is already there.
    pub combine: bool,
    pub camera: bool,
    pub history: bool,
    pub mask: bool,
    /// The cage, where one is up.
    pub cage: bool,
    /// What the deform panel would do if it were run.
    pub deform: bool,
    /// The forms a sculptor has placed, by the node id that names them.
    pub objects: bool,
    /// What the last rebuild, retopology and crossing came to.
    pub outcomes: bool,
    /// How the viewport is presented, as opposed to what it holds.
    pub presentation: bool,
    /// The reference images, plane by plane.
    pub references: bool,
    /// What an import or an export would be given.
    pub exchange: bool,
    pub jobs: bool,
    pub memory: bool,
    pub timing: bool,
    pub backends: bool,
    /// Where the last strokes spent their milliseconds, phase by phase.
    pub strokes: bool,
}

impl StateQuery {
    /// Every section's name, in the order a report carries them.
    ///
    /// One list rather than three: it is what an unknown section is answered
    /// with, what the tool descriptor advertises and what the test that walks
    /// the sections iterates, and three copies of it would drift the first
    /// time a section was added — which is exactly how this report came to be
    /// missing most of what a caller needs.
    pub const NAMES: [&'static str; 20] = [
        "document",
        "scene",
        "tool",
        "brush",
        "combine",
        "camera",
        "history",
        "mask",
        "cage",
        "deform",
        "objects",
        "outcomes",
        "presentation",
        "references",
        "exchange",
        "jobs",
        "memory",
        "timing",
        "backends",
        "strokes",
    ];

    pub fn everything() -> Self {
        let mut query = Self::nothing();
        for name in Self::NAMES {
            query.turn_on(name);
        }
        query
    }

    pub fn nothing() -> Self {
        Self::default()
    }

    /// The sections named, or everything where none are.
    pub fn from_sections(sections: &[String]) -> Result<Self, Refusal> {
        if sections.is_empty() {
            return Ok(Self::everything());
        }
        let mut query = Self::nothing();
        for section in sections {
            if !query.turn_on(section) {
                return Err(Refusal::new(
                    RefusalCode::BadArgument,
                    format!(
                        "there is no section named {section}; the sections are {}",
                        Self::NAMES.join(", ")
                    ),
                ));
            }
        }
        Ok(query)
    }

    /// Asks for one section by name. False where there is no such section.
    fn turn_on(&mut self, section: &str) -> bool {
        let field = match section {
            "document" => &mut self.document,
            "scene" => &mut self.scene,
            "tool" => &mut self.tool,
            "brush" => &mut self.brush,
            "combine" => &mut self.combine,
            "camera" => &mut self.camera,
            "history" => &mut self.history,
            "mask" => &mut self.mask,
            "cage" => &mut self.cage,
            "deform" => &mut self.deform,
            "objects" => &mut self.objects,
            "outcomes" => &mut self.outcomes,
            "presentation" => &mut self.presentation,
            "references" => &mut self.references,
            "exchange" => &mut self.exchange,
            "jobs" => &mut self.jobs,
            "memory" => &mut self.memory,
            "timing" => &mut self.timing,
            "backends" => &mut self.backends,
            "strokes" => &mut self.strokes,
            _ => return false,
        };
        *field = true;
        true
    }
}

/// What was read. Every section is absent unless it was asked for.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct StateReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<DocumentState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<SceneState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<ToolState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brush: Option<BrushState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combine: Option<CombineState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<HistoryState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<MaskState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cage: Option<CageState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deform: Option<DeformState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objects: Option<Vec<ObjectState>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcomes: Option<OutcomeState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PresentationState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<ReferenceState>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange: Option<ExchangeState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Vec<JobState>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<TimingState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backends: Option<BackendState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strokes: Option<StrokeCostState>,
}

/// Where a stroke's milliseconds went, for an agent that drove the strokes.
///
/// The section an agent asks for when it wants to know *what it just cost*
/// rather than *whether it worked*. Every phase says which side of the engine
/// boundary it is on, because a total spanning an engine call and this
/// application's work around it is a figure neither party can act on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StrokeCostState {
    /// How many tools have figures. Zero is a session nobody has sculpted in.
    pub tools_measured: usize,
    pub phases: Vec<PhaseCostState>,
    /// Always true, and always sent, for the reason [`Measured::live_session`]
    /// carries it: a figure taken with a window open and a person's session in
    /// memory is evidence, not a baseline, and nothing here may write one.
    pub live_session: bool,
}

/// One phase of a stroke, as an agent reads it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhaseCostState {
    /// The phase, in the words the report uses for it.
    pub phase: String,
    /// `engine` or `ours`. The whole point of the section.
    pub side: String,
    /// Which engine call, where it is the engine's and there is one to name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_point: Option<String>,
    /// Every sample this session, including any the window has dropped.
    pub samples: u64,
    /// Absent where the phase never ran — which is a different fact from
    /// costing nothing, and is sent as one. A zero here would read as *free*.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub median_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worst_ms: Option<f64>,
    /// What the samples behind those figures covered. A duration without it is
    /// not comparable with any other duration.
    pub keys: usize,
    pub triangles: usize,
    pub bricks: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DocumentState {
    pub name: String,
    pub modified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The working unit, in the words the status area uses.
    pub unit: String,
    /// The container minor this build writes, so a refusal elsewhere has a
    /// number behind it a person can quote.
    pub format: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SceneState {
    pub layers: Vec<LayerState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_layer: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_object: Option<u64>,
    /// Which layer is being shown alone, while one is.
    ///
    /// A different question from `active` and from the visibility flags, and
    /// the only one that distinguishes a soloed scene from a sculptor who hid
    /// three layers by hand. Nothing reported it, so an agent that soloed a
    /// layer and then read the scene back could not tell that it had.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soloed: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayerState {
    pub key: u64,
    pub name: String,
    /// Field, grid, mesh or hierarchy, in the domain's own word.
    pub representation: String,
    pub visible: bool,
    pub locked: bool,
    /// Where the layer stands, read from the engine rather than from a
    /// host-side snapshot.
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// How many forms a sculptor has placed in this layer.
    ///
    /// One meaning, on every representation. It used to be the length of
    /// `sculpt_layers`, which is a *grid's recorded passes* — so it read zero
    /// for every field layer holding a dozen placed shapes and counted passes
    /// on the one representation that has them. The passes have their own
    /// name below.
    pub objects: usize,
    /// The recorded passes on this layer, bottom-up. Empty on anything but a
    /// grid, and on a grid nobody has recorded a pass on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub passes: Vec<PassState>,
    /// What the layer's grid is made of, where it is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid: Option<GridState>,
    /// The levels and the passes, where the layer is a hierarchy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hierarchy: Option<HierarchyState>,
}

/// One recorded pass on a grid, as the layer stack shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PassState {
    pub index: usize,
    pub name: String,
    /// How far it is dialled in, −1..=1.
    pub strength: f32,
    pub visible: bool,
    /// Cells it has recorded, which is what it costs to keep.
    pub cells: usize,
}

/// What a grid layer is made of.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GridState {
    /// In world units. A feature finer than a cell cannot be sculpted.
    pub cell_size: f32,
    pub occupied_cells: usize,
}

/// A hierarchy's levels and the passes above them.
///
/// Apart from [`LayerState::passes`] because they are a different mechanism
/// addressed a different way: a grid's pass is found by its position in the
/// stack and a hierarchy's by an id that survives a reorder, and the two must
/// not be reachable through one field.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HierarchyState {
    /// How many levels the hierarchy holds.
    pub levels: u32,
    /// Which level a stroke lands on, and which one is drawn.
    pub sculpt_level: u32,
    pub display_level: u32,
    /// Which channel a stroke would enter: `automatic`, `geometry` or
    /// `detail`.
    pub write_domain: String,
    /// The pass a stroke writes into, or none for the form under them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_pass: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub passes: Vec<HierarchyPassState>,
}

/// One pass on a hierarchy, named by the id that outlives a reorder.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HierarchyPassState {
    pub id: u64,
    pub index: usize,
    pub name: String,
    pub strength: f32,
    pub visible: bool,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolState {
    pub tool: String,
    pub radius: f32,
    pub strength: f32,
    pub falloff: String,
    pub symmetry: Vec<String>,
    pub representation: String,
    /// Which frequency a smooth acts on, where that is a choice: `form`,
    /// `detail_only` or `form_with_detail`. Absent on the three
    /// representations that store one surface and therefore have one smooth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth_mode: Option<String>,
    /// Whether the rig mirrors what it grows, while a rig is being edited.
    ///
    /// Beside `symmetry` rather than folded into it, because they are two
    /// mirrors with two switches: `symmetry` decides what a *brush* stamps and
    /// this decides whether a new ZSphere gets a partner. Reporting the brush's
    /// alone told an agent that symmetry was off on a rig that was mirroring
    /// every sphere it added. Absent when nothing is being rigged, for the
    /// reason `smooth_mode` is absent off a hierarchy: a switch reported where
    /// it decides nothing is a switch an agent will act on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rig_mirror: Option<bool>,
    /// The key of the tool that was chosen, when `tool` is standing in for it.
    ///
    /// A layer switch keeps the chosen tool where the new layer carries it and
    /// hands over a substitute from the capability table where it does not.
    /// The switch's own answer says so once; this says so for as long as it
    /// is true, so an agent that reads `state` after a switch can tell a tool
    /// it chose from one it was given — and knows that switching back to a
    /// layer that carries the chosen one returns it. Absent when the tool in
    /// hand is the one chosen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stands_in_for: Option<String>,
}

/// Everything the brush panel holds, beside the size and strength the tool
/// section already carries.
///
/// A section of its own rather than more fields on the tool, because these are
/// what a *stroke* comes out looking like — and an agent comparing two strokes
/// that differ needs all of them at once, not the two the options bar happens
/// to put at the front.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BrushState {
    /// How much of the stroke each stamp contributes, 0..=1.
    pub flow: f32,
    /// Positional jitter as a fraction of the radius.
    pub noise: f32,
    /// Whether overlapping stamps deposit twice.
    pub accumulate: bool,
    /// Lazy-mouse lag: 0 follows the pointer exactly.
    pub smoothing: f32,
    /// Whether each stamp is mirrored about the stroke.
    pub stroke_mirror: bool,
    /// How far each stamp is turned about its own facing, in DEGREES — the
    /// grain. Radians inside the document; an agent reasons in degrees, as it
    /// does for the camera's field of view.
    pub grain_degrees: f32,
    /// Whether this brush is modulated by the loaded alpha stamp.
    pub alpha: bool,
    /// Whether the stroke in hand takes material away rather than adding it.
    pub invert: bool,
    /// How the stroke varies along its own length.
    pub dynamics: DynamicsState,
    /// What a drag does, which no other verb reads.
    pub drag: DragState,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DynamicsState {
    pub pressure_size: f32,
    pub pressure_strength: f32,
    pub pressure_curve: f32,
    pub taper_start: f32,
    pub taper_end: f32,
    /// Whether each stamp is turned to follow the stroke's direction.
    pub rake: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DragState {
    /// How the pull falls off across the ball: the easing's own word.
    pub falloff: String,
    /// Whether only the side facing the eye is taken hold of.
    pub front_only: bool,
}

/// How a stroke or a placed form meets what is already there.
///
/// Two of them, because they are two settings: a stroke's is the sculpting
/// ViewModel's and a placed form's is the object ViewModel's, and an agent
/// that set one and read the other back could not tell why nothing changed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CombineState {
    pub stroke: CombineSetting,
    pub placement: CombineSetting,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CombineSetting {
    /// The operation, in the word the wire uses for it.
    pub op: String,
    /// How the two surfaces are blended where they meet.
    pub blend: String,
    /// How wide that blend is, in world units.
    pub radius: f32,
}

/// The cage, where one is up.
///
/// `active: false` and nothing else, where none is: the divisions below are
/// what the *next* cage would be built with, and reporting them as though a
/// cage were standing is how an agent comes to believe it has one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CageState {
    pub active: bool,
    /// The lattice's divisions on each axis.
    pub divisions: [i32; 3],
    /// How many control points the cage holds.
    pub points: usize,
    /// How many of them are in hand.
    pub selected_points: usize,
    /// Which of the manipulator's three modes is in force.
    pub mode: String,
}

/// What the deform panel would do if it were run.
///
/// Settings, not an outcome: nothing reaches the document until the deform is
/// asked for, so this is the only place they can be read from.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeformState {
    /// `taper` or `twist`.
    pub verb: String,
    /// The axis it runs along.
    pub axis: [f32; 3],
    /// How much of the layer it spans, 0..=1.
    pub span: f32,
    /// The taper's two ends. Inert for a twist, and sent anyway: a caller that
    /// sets one and reads the other back has to be able to see both.
    pub scale_start: f32,
    pub scale_end: f32,
    /// The twist's angle, in degrees.
    pub degrees: f32,
}

/// One form a sculptor has placed, named by the id the document knows it as.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObjectState {
    /// The layer and the node packed as one number, exactly as
    /// [`SceneState::selected_object`] packs them, so the two can be compared
    /// without unpacking either.
    pub id: u64,
    pub layer: u64,
    pub node: u32,
    /// What it is: a primitive's name, or the word for a mesh placed as one.
    pub source: String,
    /// What the shape is measured by. Empty for a mesh, which is measured by
    /// itself.
    pub parameters: Vec<f32>,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// Whether this is the one the manipulator is on.
    pub selected: bool,
}

/// What the last rebuild, retopology and crossing came to.
///
/// Each is absent until one has run, because "it has not been done" and "it
/// was done and changed nothing" are different answers and an agent branches
/// on the difference.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct OutcomeState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remesh: Option<RemeshOutcomeState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retopology: Option<RetopoOutcomeState>,
    /// What the last crossing between two representations produced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossing: Option<CrossingOutcomeState>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RemeshOutcomeState {
    pub triangles_before: u64,
    pub triangles_after: u64,
    /// What the resolution came to in world units.
    pub voxel_size: f32,
    /// How many separate pieces the form is in now. More than one after a
    /// rebuild meant to fuse is the answer to "why did that not join".
    pub pieces: u32,
    pub pieces_removed: u32,
    pub watertight: bool,
    pub uvs_dropped: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RetopoOutcomeState {
    pub triangles_before: usize,
    pub triangles: usize,
    pub vertices: usize,
    pub faces: usize,
    /// Whether the result came back as quads rather than triangles.
    pub quads: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CrossingOutcomeState {
    /// Which way it went, as `field -> mesh`.
    pub direction: String,
    /// The layer it landed on.
    pub layer: u64,
}

/// How the viewport is presented, as opposed to what it holds.
///
/// None of it enters the history or the document, which is why it is its own
/// section: an agent comparing two captures needs to know the chrome was away
/// and the surface was faded before it reads a difference as a defect.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PresentationState {
    /// Whether the chrome is cleared away and only the sculpt is left.
    pub focus: bool,
    pub grid: bool,
    /// Whether a mesh layer is drawn with its edges over it.
    pub polyframe: bool,
    /// The projection the viewport is drawn with.
    pub view_preset: String,
    /// How opaque the sculpted surface is drawn, 0..=1.
    pub surface_opacity: f32,
    /// Whether the pointer is rigging rather than sculpting.
    pub rigging: bool,
    /// Whether the rig's skin is previewed, or only its ZSpheres stand.
    pub skin_preview: bool,
}

/// One reference image plane, as the reference panel holds it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReferenceState {
    /// `front`, `side` or `top`.
    pub plane: String,
    /// Whether an image is placed on this plane at all.
    pub placed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub visible: bool,
    pub opacity: f32,
    pub height: f32,
    pub offset: [f32; 2],
    pub depth: f32,
}

/// What an import or an export would be given.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExchangeState {
    pub import: ImportState,
    pub export: ExportState,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportState {
    /// What an imported mesh becomes.
    pub becomes: String,
    pub scale: f32,
    pub max_vertices: u64,
    pub max_triangles: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportState {
    /// Which mesher the export runs through.
    pub mesher: String,
    pub resolution: f32,
    /// The fraction of triangles to keep, where the export decimates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimate_to: Option<f32>,
    /// What the last export turned out to be, as opposed to what it promised.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CameraState {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_degrees: f32,
    pub viewport: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryState {
    pub depth: usize,
    /// How many actions can be redone.
    ///
    /// The other half of `depth`, and it was missing: an agent that undid four
    /// things had no way to tell how many redoes would put them back, so
    /// "restore what I just took away" was a guess it had to verify against a
    /// picture.
    pub redo_depth: usize,
    /// What the next undo would take back, in the interface's own words.
    ///
    /// **The next step, not the last one.** It reported the last *action* —
    /// so after an undo it read "undo", and after a cancelled clay stroke it
    /// read "Argila", neither of which names anything an undo would revert.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undoes: Option<String>,
    /// What the next redo would put back. Absent where there is nothing to
    /// redo, which is a different answer from a redo that would restore
    /// something unnamed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redoes: Option<String>,
    /// How many of this session's entries arrived from an agent.
    pub from_agent: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaskState {
    /// Whether anything is frozen on the active subtool.
    ///
    /// The question an agent is asking — "will my next stroke be resisted" —
    /// rather than whether the layer carries a mask field at all. An emptied
    /// mask stays attached inside the document, so the two answers differ
    /// exactly after a clear.
    pub present: bool,
    /// The share of the active layer the mask protects, where the engine can
    /// say, and none where it cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f32>,
    pub inverted: bool,
    /// How many cells are frozen. The engine's own count, and the one number
    /// behind `coverage` that does not need a whole to be measured against.
    pub painted_cells: usize,
    /// How many steps an expand, a contract or a smooth would take.
    ///
    /// The panel's own setting, and it decides what those three operations do
    /// — so an agent that asked for a contract and got more than it expected
    /// had nowhere to look.
    pub steps: i32,
    /// How the mask is painted: `brush`, `lasso` or `rectangle`.
    pub gesture: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JobState {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fraction: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryState {
    pub in_use_bytes: u64,
    pub budget_bytes: u64,
    /// The brick cache on its own: payload and bookkeeping.
    ///
    /// Included in `in_use_bytes`, and named beside it because it is the part
    /// the budget above bounds — the budget limits the cache, not the whole,
    /// and comparing it against the whole reads as an overrun that is not one.
    pub cache_bytes: u64,
    /// What the operating system charges the process, where it can be read.
    ///
    /// The figure `in_use_bytes` is checked against. It is always larger —
    /// it also counts the code, the graphics driver and the interface — and a
    /// footprint that runs far past the ledger is logged with the ledger's
    /// breakdown, because that gap is what memory nobody counts looks like.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footprint_bytes: Option<u64>,
    /// Which part holds it. The engine's own accounting for the document and
    /// its surfaces, then the parts this application holds beside it: `cache`
    /// and `desenho` (the drawing), with the drawing's own four parts after
    /// it.
    pub parts: Vec<MemoryPart>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryPart {
    pub part: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TimingState {
    pub frame_millis: f32,
    /// Operations that held the interface thread longer than a frame, worst
    /// first, with how often each did.
    pub stalls: Vec<StallState>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StallState {
    pub operation: String,
    pub millis: f64,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BackendState {
    pub active: String,
    pub registered: Vec<String>,
    pub engine_version: String,
    pub engine_revision: String,
    pub platform: String,
    /// Every operation that ran somewhere other than the active backend.
    pub fallbacks: Vec<FallbackState>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FallbackState {
    pub operation: String,
    pub declined_by: String,
}

/// What to draw, and how large.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRequest {
    pub what: CaptureWhat,
    /// The window's own size where none is given.
    pub width: Option<u32>,
    pub height: Option<u32>,
}

impl Default for CaptureRequest {
    fn default() -> Self {
        Self {
            what: CaptureWhat::Viewport,
            width: None,
            height: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureWhat {
    /// The surface, its overlays, and nothing else. The cheaper of the two and
    /// the one most answers need.
    Viewport,
    /// The panels and bars as drawn, too. A defect in what a panel says is a
    /// defect no picture of the surface holds.
    Window,
}

/// One rendered frame, unencoded.
///
/// RGBA8 rows with no padding. PNG and base64 happen on the connection thread:
/// a megabyte-and-a-half encode inside a frame is a dropped frame for a result
/// nobody is watching in real time.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rows: Vec<u8>,
    /// What was still running when this was taken. Empty means settled — and
    /// an agent that reads a half-meshed surface as a defect is an agent that
    /// files one, so this is never left implicit.
    pub outstanding: Vec<Outstanding>,
}

/// A piece of work that has not finished.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outstanding {
    pub what: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fraction: Option<f32>,
}

/// What waiting for quiet found.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Settled {
    pub quiet: bool,
    pub waited_millis: u64,
    /// Bytes recorded by the GPU upload counter while completing this operation.
    pub uploaded_bytes: u64,
    /// Named rather than merely counted: "time ran out" is not something an
    /// agent can act on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outstanding: Vec<Outstanding>,
}

/// One operation, timed in the live session.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measured {
    pub label: String,
    pub millis: f64,
    /// Bytes recorded by the GPU upload counter during dispatch and geometry work.
    pub uploaded_bytes: u64,
    /// Whether this held the interface thread longer than a frame.
    pub stalled: bool,
    pub backend: String,
    pub platform: String,
    /// Always true, and always sent. A figure taken with a window open, panels
    /// drawn and a person's session in memory is evidence, not a baseline, and
    /// nothing here may write one.
    pub live_session: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gate_kind_round_trips_through_its_tag() {
        for gate in [
            GateKind::Overwrite,
            GateKind::Export,
            GateKind::Open,
            GateKind::DiscardUnsaved,
            GateKind::IrreversibleRemoval,
            GateKind::Quit,
        ] {
            assert_eq!(GateKind::from_tag(gate.tag()), Some(gate));
        }
        assert_eq!(GateKind::from_tag("sculpt"), None);
    }

    #[test]
    fn no_sections_means_every_section() {
        assert_eq!(
            StateQuery::from_sections(&[]).unwrap(),
            StateQuery::everything()
        );
    }

    #[test]
    fn an_unknown_section_names_the_ones_that_exist() {
        let refusal = StateQuery::from_sections(&["surface".to_string()]).unwrap_err();
        assert_eq!(refusal.code, RefusalCode::BadArgument);
        assert!(refusal.message.contains("surface"), "{}", refusal.message);
        assert!(refusal.message.contains("camera"), "{}", refusal.message);
    }

    #[test]
    fn a_named_section_is_the_only_one_read() {
        let query = StateQuery::from_sections(&["scene".to_string()]).unwrap();
        assert!(query.scene);
        assert!(!query.document);
        assert!(!query.timing);
    }

    /// Asking for every section by name is asking for everything.
    ///
    /// The two are written once and derived from each other, and this is what
    /// keeps them that way: a section added to the query with no name would be
    /// unreachable from the wire, and a name with no section would be refused
    /// by the reader after the schema had advertised it.
    #[test]
    fn every_section_has_a_name_and_every_name_a_section() {
        let all: Vec<String> = StateQuery::NAMES.iter().map(|n| n.to_string()).collect();
        assert_eq!(
            StateQuery::from_sections(&all).unwrap(),
            StateQuery::everything()
        );
        assert_eq!(
            StateQuery::NAMES.len(),
            StateQuery::NAMES
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            "a repeated name would make one section unreachable, silently"
        );
    }

    /// Every section an agent can ask for is a section the report can carry,
    /// under the same name.
    ///
    /// The point of the whole issue behind this one: verification fell back to
    /// comparing screenshots because a command could change the session in a
    /// way `state` had no field for. A name the report answers under a
    /// different spelling is the same hole wearing a typo, and this is where
    /// both are caught — the section list, the query and the wire's keys are
    /// asserted to be one set rather than three that happen to agree today.
    #[test]
    fn every_named_section_is_a_key_the_report_carries() {
        let filled = serde_json::to_value(a_report_of_everything()).expect("a report");
        let keys: std::collections::BTreeSet<&str> = filled
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        let named: std::collections::BTreeSet<&str> = StateQuery::NAMES.into_iter().collect();
        assert_eq!(
            keys, named,
            "a section an agent can ask for and a key the report writes have \
             to be the same word"
        );
    }

    /// A report with every section in it, for the test above.
    ///
    /// Written out rather than derived: it is the one place the whole shape of
    /// the answer is stated, and a section added without a line here is a
    /// section the test above will name.
    fn a_report_of_everything() -> StateReport {
        StateReport {
            document: Some(DocumentState {
                name: "forma".into(),
                modified: false,
                path: None,
                unit: "mm".into(),
                format: "1.16".into(),
            }),
            scene: Some(SceneState {
                layers: Vec::new(),
                active_layer: None,
                selected_object: None,
                soloed: None,
            }),
            tool: Some(ToolState {
                tool: "clay".into(),
                radius: 0.1,
                strength: 0.5,
                falloff: "smooth".into(),
                symmetry: Vec::new(),
                representation: "field".into(),
                smooth_mode: None,
                rig_mirror: None,
                stands_in_for: None,
            }),
            brush: Some(BrushState {
                flow: 1.0,
                noise: 0.0,
                accumulate: true,
                smoothing: 0.0,
                stroke_mirror: false,
                grain_degrees: 0.0,
                alpha: false,
                invert: false,
                dynamics: DynamicsState {
                    pressure_size: 0.0,
                    pressure_strength: 0.0,
                    pressure_curve: 1.0,
                    taper_start: 0.0,
                    taper_end: 0.0,
                    rake: false,
                },
                drag: DragState {
                    falloff: "smooth".into(),
                    front_only: false,
                },
            }),
            combine: Some(CombineState {
                stroke: a_combine(),
                placement: a_combine(),
            }),
            camera: Some(CameraState {
                eye: [0.0, 0.0, 3.0],
                target: [0.0; 3],
                up: [0.0, 1.0, 0.0],
                fov_degrees: 45.0,
                viewport: [1, 1],
            }),
            history: Some(HistoryState {
                depth: 0,
                redo_depth: 0,
                undoes: None,
                redoes: None,
                from_agent: 0,
            }),
            mask: Some(MaskState {
                present: false,
                coverage: None,
                inverted: false,
                painted_cells: 0,
                steps: 1,
                gesture: "brush".into(),
            }),
            cage: Some(CageState {
                active: false,
                divisions: [2; 3],
                points: 0,
                selected_points: 0,
                mode: "move".into(),
            }),
            deform: Some(DeformState {
                verb: "taper".into(),
                axis: [0.0, 1.0, 0.0],
                span: 1.0,
                scale_start: 1.0,
                scale_end: 1.0,
                degrees: 0.0,
            }),
            objects: Some(Vec::new()),
            outcomes: Some(OutcomeState::default()),
            presentation: Some(PresentationState {
                focus: false,
                grid: true,
                polyframe: false,
                view_preset: "perspective".into(),
                surface_opacity: 1.0,
                rigging: false,
                skin_preview: true,
            }),
            references: Some(Vec::new()),
            exchange: Some(ExchangeState {
                import: ImportState {
                    becomes: "clay".into(),
                    scale: 1.0,
                    max_vertices: 0,
                    max_triangles: 0,
                },
                export: ExportState {
                    mesher: "watertight".into(),
                    resolution: 0.01,
                    decimate_to: None,
                    findings: Vec::new(),
                },
            }),
            jobs: Some(Vec::new()),
            memory: Some(MemoryState {
                in_use_bytes: 0,
                budget_bytes: 0,
                cache_bytes: 0,
                footprint_bytes: None,
                parts: Vec::new(),
            }),
            timing: Some(TimingState {
                frame_millis: 0.0,
                stalls: Vec::new(),
            }),
            backends: Some(BackendState {
                active: "cpu".into(),
                registered: Vec::new(),
                engine_version: String::new(),
                engine_revision: String::new(),
                platform: String::new(),
                fallbacks: Vec::new(),
            }),
            strokes: Some(StrokeCostState {
                tools_measured: 0,
                phases: Vec::new(),
                live_session: true,
            }),
        }
    }

    fn a_combine() -> CombineSetting {
        CombineSetting {
            op: "add".into(),
            blend: "quadratic".into(),
            radius: 0.0,
        }
    }
}
