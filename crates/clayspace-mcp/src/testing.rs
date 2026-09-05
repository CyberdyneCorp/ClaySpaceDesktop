//! A session that is not an application.
//!
//! This is what makes the whole tool surface — every mapping, the catalogue,
//! the gates, the protocol — exercisable with no window, no GPU and no C++
//! engine built. It is the same bargain `clayspace-vm` makes by having no
//! `egui`: a surface that can only be tested by someone sitting in front of a
//! window is a surface nobody tests.

use std::collections::HashMap;
use std::time::Duration;

use clayspace_vm::Command;

use crate::session::{
    Applied, BackendState, CameraState, CaptureRequest, CaptureWhat, Consent, ConsentOutcome,
    DocumentState, Frame, GateKind, HistoryState, JobState, LayerState, MaskState, Measured,
    MemoryPart, MemoryState, Outstanding, Refusal, RefusalCode, SceneState, Session, Settled,
    StallState, StateQuery, StateReport, TimingState, ToolState,
};

/// A `Session` that records what it was asked and answers from a table.
pub struct FakeSession {
    /// Every command applied, in order.
    pub applied: Vec<Command>,
    /// Commands this session refuses, and what it says about each.
    pub refusals: HashMap<&'static str, Refusal>,
    /// What the next consent ask is answered with.
    pub consent: ConsentOutcome,
    /// Every consent asked for, in order.
    pub asked: Vec<Consent>,
    pub gesture: bool,
    pub history_depth: usize,
    pub outstanding: Vec<Outstanding>,
    /// How many times state was read, so a test can assert reading changed
    /// nothing.
    pub reads: std::cell::Cell<usize>,
    pub captures: usize,
    /// The colour a captured frame is filled with, so two captures can be made
    /// to differ on purpose.
    pub fill: [u8; 4],
    pub document_name: String,
    pub modified: bool,
}

impl Default for FakeSession {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeSession {
    pub fn new() -> Self {
        Self {
            applied: Vec::new(),
            refusals: HashMap::new(),
            consent: ConsentOutcome::Granted,
            asked: Vec::new(),
            gesture: false,
            history_depth: 0,
            outstanding: Vec::new(),
            reads: std::cell::Cell::new(0),
            captures: 0,
            fill: [40, 44, 52, 255],
            document_name: "sem título".to_string(),
            modified: false,
        }
    }

    /// Makes this session refuse a command, the way the Model would.
    pub fn refusing(mut self, label: &'static str, message: &str) -> Self {
        self.refusals.insert(
            label,
            Refusal::new(RefusalCode::ModelRefused, message.to_string()),
        );
        self
    }

    pub fn holding_a_gesture(mut self) -> Self {
        self.gesture = true;
        self
    }

    pub fn answering_consent(mut self, outcome: ConsentOutcome) -> Self {
        self.consent = outcome;
        self
    }

    pub fn still_running(mut self, what: &str) -> Self {
        self.outstanding.push(Outstanding {
            what: what.to_string(),
            fraction: None,
        });
        self
    }

    /// The last command applied, for a test that only cares about one.
    pub fn last(&self) -> Option<&Command> {
        self.applied.last()
    }
}

impl Session for FakeSession {
    fn apply(&mut self, command: Command) -> Result<Applied, Refusal> {
        if let Some(refusal) = self.refusals.get(command.label()) {
            return Err(refusal.clone());
        }
        let touched = command.touches_document();
        // A gesture is one history entry however many samples it took, so the
        // opening and the samples move nothing and the close moves one. The
        // fake models that because it is the property the tool surface is
        // asserted against.
        let mid_gesture = matches!(
            command,
            Command::BeginStroke { .. } | Command::ContinueStroke { .. } | Command::CancelStroke
        );
        if touched && !mid_gesture {
            self.history_depth += 1;
            self.modified = true;
        }
        let label = command.label().to_string();
        self.applied.push(command);
        Ok(Applied {
            label: label.clone(),
            touched_document: touched,
            history_depth: self.history_depth,
            undoes: (self.history_depth > 0).then_some(label),
            notices: Vec::new(),
        })
    }

    fn read(&mut self, query: StateQuery) -> StateReport {
        self.reads.set(self.reads.get() + 1);
        let mut report = StateReport::default();
        if query.document {
            report.document = Some(DocumentState {
                name: self.document_name.clone(),
                modified: self.modified,
                path: None,
                unit: "mm".into(),
                format: "1.16".into(),
            });
        }
        if query.scene {
            report.scene = Some(SceneState {
                layers: vec![LayerState {
                    key: 1,
                    name: "corpo".into(),
                    representation: "campo".into(),
                    visible: true,
                    locked: false,
                    translation: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0; 3],
                    objects: 0,
                }],
                active_layer: Some(1),
                selected_object: None,
            });
        }
        if query.tool {
            report.tool = Some(ToolState {
                tool: "argila".into(),
                radius: 0.1,
                strength: 0.5,
                falloff: "suave".into(),
                symmetry: vec!["x".into()],
                representation: "campo".into(),
            });
        }
        if query.camera {
            report.camera = Some(CameraState {
                eye: [0.0, 0.0, 3.0],
                target: [0.0; 3],
                up: [0.0, 1.0, 0.0],
                fov_degrees: 45.0,
                viewport: [1280, 720],
            });
        }
        if query.history {
            report.history = Some(HistoryState {
                depth: self.history_depth,
                undoes: (self.history_depth > 0).then(|| "argila".to_string()),
                redoes: None,
                from_agent: self.applied.len(),
            });
        }
        if query.mask {
            report.mask = Some(MaskState {
                present: false,
                coverage: None,
                inverted: false,
            });
        }
        if query.jobs {
            report.jobs = Some(
                self.outstanding
                    .iter()
                    .map(|item| JobState {
                        label: item.what.clone(),
                        fraction: item.fraction,
                    })
                    .collect(),
            );
        }
        if query.memory {
            report.memory = Some(MemoryState {
                in_use_bytes: 128 * 1024 * 1024,
                budget_bytes: 2 * 1024 * 1024 * 1024,
                parts: vec![MemoryPart {
                    part: "cache de blocos".into(),
                    bytes: 128 * 1024 * 1024,
                }],
            });
        }
        if query.timing {
            report.timing = Some(TimingState {
                frame_millis: 8.3,
                stalls: vec![StallState {
                    operation: "exportar".into(),
                    millis: 82.0,
                    count: 1,
                }],
            });
        }
        if query.backends {
            report.backends = Some(BackendState {
                active: "cpu".into(),
                registered: vec!["cpu".into()],
                engine_version: "0.78.0".into(),
                engine_revision: "não vendorizado".into(),
                platform: "test".into(),
                fallbacks: Vec::new(),
            });
        }
        report
    }

    fn capture(&mut self, request: CaptureRequest) -> Result<Frame, Refusal> {
        self.captures += 1;
        let width = request.width.unwrap_or(64);
        let height = request.height.unwrap_or(48);
        // The window variant is drawn larger here only so a test can tell the
        // two apart without a renderer.
        let (width, height) = match request.what {
            CaptureWhat::Viewport => (width, height),
            CaptureWhat::Window => (width + 8, height + 8),
        };
        let mut rows = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..width * height {
            rows.extend_from_slice(&self.fill);
        }
        Ok(Frame {
            width,
            height,
            rows,
            outstanding: self.outstanding.clone(),
        })
    }

    fn settle(&mut self, _budget: Duration) -> Settled {
        Settled {
            quiet: self.outstanding.is_empty(),
            waited_millis: 0,
            outstanding: self.outstanding.clone(),
        }
    }

    fn measure(&mut self, command: Command) -> Result<Measured, Refusal> {
        let label = command.label().to_string();
        self.apply(command)?;
        Ok(Measured {
            label,
            millis: 2.1,
            stalled: false,
            backend: "cpu".into(),
            platform: "test".into(),
            live_session: true,
        })
    }

    fn consent(&mut self, ask: &Consent) -> ConsentOutcome {
        self.asked.push(ask.clone());
        self.consent
    }

    fn gesture_in_progress(&self) -> bool {
        self.gesture
    }
}

/// The gate a test expects an operation to be held behind, for readability at
/// the assertion.
pub fn gate_of(refusal: &Refusal) -> Option<GateKind> {
    refusal.gate
}
