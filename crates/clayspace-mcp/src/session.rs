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
    pub camera: bool,
    pub history: bool,
    pub mask: bool,
    pub jobs: bool,
    pub memory: bool,
    pub timing: bool,
    pub backends: bool,
    /// Where the last strokes spent their milliseconds, phase by phase.
    pub strokes: bool,
}

impl StateQuery {
    pub fn everything() -> Self {
        Self {
            document: true,
            scene: true,
            tool: true,
            camera: true,
            history: true,
            mask: true,
            jobs: true,
            memory: true,
            timing: true,
            backends: true,
            strokes: true,
        }
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
            match section.as_str() {
                "document" => query.document = true,
                "scene" => query.scene = true,
                "tool" => query.tool = true,
                "camera" => query.camera = true,
                "history" => query.history = true,
                "mask" => query.mask = true,
                "jobs" => query.jobs = true,
                "memory" => query.memory = true,
                "timing" => query.timing = true,
                "backends" => query.backends = true,
                "strokes" => query.strokes = true,
                other => {
                    return Err(Refusal::new(
                        RefusalCode::BadArgument,
                        format!(
                            "there is no section named {other}; the sections are \
                             document, scene, tool, camera, history, mask, jobs, \
                             memory, timing, backends and strokes"
                        ),
                    ))
                }
            }
        }
        Ok(query)
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
    pub camera: Option<CameraState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<HistoryState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<MaskState>,
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
    pub objects: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolState {
    pub tool: String,
    pub radius: f32,
    pub strength: f32,
    pub falloff: String,
    pub symmetry: Vec<String>,
    pub representation: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undoes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redoes: Option<String>,
    /// How many of this session's entries arrived from an agent.
    pub from_agent: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaskState {
    pub present: bool,
    /// The share of the active layer the mask protects, where the engine can
    /// say, and none where it cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f32>,
    pub inverted: bool,
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
    /// Which part of the document holds it — the engine's own accounting, not
    /// an estimate kept here.
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
}
