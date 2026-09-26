//! The tool surface an agent sees.
//!
//! One tool per domain the application already has a panel or a ViewModel for,
//! each taking an action and that action's arguments, and a handful of tools
//! that are not commands at all: reading state, seeing the viewport, waiting
//! for quiet, measuring, and asking what the vocabulary is.
//!
//! Not one tool per `Command` variant. There are a hundred and thirty of them,
//! and a tool list that long is a list an agent chooses badly from — while a
//! group whose actions it can ask for is one it can learn at the moment it
//! needs to.

pub mod actions;
pub mod args;
pub mod table;
pub mod tags;

#[cfg(test)]
mod contract_tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use clayspace_model::Representation;
use clayspace_vm::Command;

use crate::access;
use crate::gate;
use crate::protocol::{CallResult, ToolDescriptor, ToolSurface};
use crate::queue::{Answer, JobQueue};
use crate::session::{
    CaptureCamera, CaptureRequest, CaptureWhat, Consent, ConsentOutcome, Frame, Refusal,
    RefusalCode, StateQuery,
};

use self::args::Args;
use self::table::{Kind, GROUPS, TABLE};

/// How long each kind of work may take before a client is told rather than
/// left waiting.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    /// One command, applied between frames.
    pub call: Duration,
    /// One command plus the re-mesh and the frame that follow it.
    pub capture: Duration,
    /// Waiting for the session to go quiet, where the caller names none.
    pub settle: Duration,
    /// How long an ask may stand at the window unanswered.
    pub consent: Duration,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            call: Duration::from_secs(10),
            capture: Duration::from_secs(30),
            settle: Duration::from_secs(30),
            // Long enough for somebody looking at the screen to read the ask
            // and click, short enough that a client is told rather than left
            // hanging. The ask stays up when this expires, so an answer given
            // late is picked up by the agent's next try rather than lost.
            consent: Duration::from_secs(20),
        }
    }
}

/// How often the connection thread comes back to see whether the ask standing
/// at the window has been answered.
const POLL_THE_ASK: Duration = Duration::from_millis(300);

/// How many remembered frames a client may keep.
const REMEMBERED_FRAMES: usize = 8;

/// The tool surface, over a queue onto the interface thread.
pub struct Catalogue {
    queue: JobQueue,
    /// The session directory, where recorded consents live.
    store: PathBuf,
    bounds: Bounds,
    /// Frames a client asked to keep, for a later comparison.
    remembered: Mutex<HashMap<(String, String), Frame>>,
}

impl Catalogue {
    pub fn new(queue: JobQueue, store: impl Into<PathBuf>) -> Self {
        Self {
            queue,
            store: store.into(),
            bounds: Bounds::default(),
            remembered: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_bounds(mut self, bounds: Bounds) -> Self {
        self.bounds = bounds;
        self
    }

    // -- the group tools ----------------------------------------------------

    fn call_group(
        &self,
        group: &'static str,
        arguments: &Value,
        client: Option<&str>,
    ) -> Result<CallResult, Refusal> {
        let action = arguments
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                Refusal::new(
                    RefusalCode::BadArgument,
                    format!(
                        "{group} takes an action; it offers {}",
                        actions::actions_of(group).join(", ")
                    ),
                )
            })?;

        let args = Args::new(group, action, arguments);
        let command = actions::build(group, action, &args)?;
        let clamped = args.clamped();
        let capture = capture_of(arguments)?;

        if let Some(gate) = gate::gate_of(&command) {
            self.obtain(gate, &command, client)?;
        }

        // `changes_the_document` rather than `touches_document`: the narrower
        // question is about the undo history, and a crossing, an import and a
        // pass of the active layer's stack answer `false` to it while plainly
        // changing the document. Those are among the operations that most need
        // keeping out of an open gesture.
        //
        // Asked here rather than in the job, because the command is moved into
        // it. Both are properties of the command alone.
        let changes = command.changes_the_document();
        let continues_a_gesture = command.continues_a_gesture();
        let bound = if capture.is_some() {
            self.bounds.capture
        } else {
            self.bounds.call
        };
        let settle = self.bounds.settle.min(bound);

        let answer = self.queue.submit(bound, move |session| {
            // A person's stroke is not interrupted. Reading is served during
            // one; changing the document is not.
            if changes && session.gesture_in_progress() {
                return Err(Refusal::new(
                    RefusalCode::GestureInProgress,
                    "a gesture is in progress at the window; this would land in the \
                     middle of somebody's stroke. Try again once it is finished.",
                ));
            }

            // Nor is the agent's own. It opened the gesture, so it may send
            // the verbs that carry it on or close it — and nothing else.
            //
            // The exemption used to be the whole gesture guard: while the
            // agent held one, *every* command went through, because the rule
            // was written as "this is not a person's gesture" rather than as
            // "this belongs to the gesture that is open". `layer/add` and
            // `history/undo` were measured landing in the middle of the
            // agent's own stroke, and `layer/remesh` rebuilt a layer that had
            // half a stroke in it — which left the engine reporting errors and
            // a band of clay that survived the undo.
            if changes && !continues_a_gesture && session.agent_gesture_in_progress() {
                return Err(Refusal::new(
                    RefusalCode::GestureInProgress,
                    "a stroke this agent opened is still in progress; this would land in \
                     the middle of it. Close it with stroke.end or stroke.cancel first.",
                ));
            }

            let applied = session.apply(command)?;
            let mut value = serde_json::to_value(&applied).unwrap_or(json!({}));
            // What the application brought into range, beside the answer
            // rather than instead of it: the command was applied, at these
            // values, and an agent that asked for others is owed the ones it
            // got.
            if !clamped.is_empty() {
                value["clamped"] = json!(clamped);
            }

            let frame = match capture {
                None => None,
                Some(request) => {
                    // After the change has reached the surface, not before: a
                    // frame taken between the edit and its re-mesh shows
                    // neither the old surface nor the new one.
                    let settled = session.settle(settle);
                    value["settled"] = serde_json::to_value(&settled).unwrap_or(json!({}));
                    Some(session.capture(request)?)
                }
            };

            Ok(Answer { value, frame })
        })?;

        self.answer(answer)
    }

    /// Turns a job's answer into a tool result, encoding any frame here — on
    /// the connection thread, where a megabyte-and-a-half of PNG costs nobody
    /// a dropped frame.
    fn answer(&self, answer: Answer) -> Result<CallResult, Refusal> {
        match answer.frame {
            None => Ok(CallResult::data(answer.value)),
            Some(frame) => {
                let mut value = answer.value;
                value["image"] = json!({
                    "width": frame.width,
                    "height": frame.height,
                    "outstanding": frame.outstanding,
                });
                let png = encode_png(&frame)?;
                Ok(CallResult::data(value).with_image(png))
            }
        }
    }

    // -- consent ------------------------------------------------------------

    fn obtain(
        &self,
        gate: crate::session::GateKind,
        command: &Command,
        client: Option<&str>,
    ) -> Result<(), Refusal> {
        if access::read_consents(&self.store)
            .iter()
            .any(|tag| tag == gate.tag())
        {
            return Ok(());
        }

        let ask = Consent {
            // Stable per gate, not a counter. A retry after the wait expired
            // must re-raise the *same* question — otherwise the answer a
            // person gave to the first one belongs to an ask nobody is
            // waiting on any more.
            id: ask_id(gate),
            gate,
            operation: command.label().to_string(),
            // The application localizes this generic marker for legacy
            // clients. A supplied client name is carried unchanged.
            client: client.unwrap_or("").to_string(),
            path: path_of(command),
            bound: self.bounds.consent,
        };

        let deadline = Instant::now() + self.bounds.consent;
        loop {
            let asked = ask.clone();
            let answer = self.queue.submit(self.bounds.call, move |session| {
                Ok(Answer::value(json!(word_of(session.consent(&asked)))))
            })?;

            match answer.value.as_str().unwrap_or("pending") {
                "granted" | "recorded" => return Ok(()),
                "refused" => {
                    return Err(Refusal {
                        code: RefusalCode::ConsentRefused,
                        message: format!(
                            "the person at the window refused this: {}",
                            command.label()
                        ),
                        gate: Some(gate),
                    })
                }
                "timed_out" => return Err(timed_out(gate, command)),
                _ => {
                    if Instant::now() >= deadline {
                        return Err(timed_out(gate, command));
                    }
                    // Every poll wakes the interface thread, and an ask may
                    // stand for two minutes. Three a second is well inside
                    // what a person notices between clicking and the
                    // operation running, and it is a twelfth of the wake-ups
                    // a tighter loop would cost.
                    std::thread::sleep(POLL_THE_ASK);
                }
            }
        }
    }

    // -- the tools that are not commands ------------------------------------

    fn call_state(&self, arguments: &Value) -> Result<CallResult, Refusal> {
        let args = Args::new("state", "read", arguments);
        let query = StateQuery::from_sections(&args.text_list_or_empty("sections")?)?;
        let answer = self.queue.submit(self.bounds.call, move |session| {
            let report = session.read(query);
            Ok(Answer::value(
                serde_json::to_value(&report).unwrap_or(json!({})),
            ))
        })?;
        self.answer(answer)
    }

    fn call_wait(&self, arguments: &Value) -> Result<CallResult, Refusal> {
        let args = Args::new("wait", "settle", arguments);
        let asked = Duration::from_millis(args.integer_or("bound_ms", 5_000)?.max(0) as u64);
        let budget = asked.min(self.bounds.settle);
        let answer = self
            .queue
            .submit(budget + Duration::from_secs(5), move |session| {
                let settled = session.settle(budget);
                Ok(Answer::value(
                    serde_json::to_value(&settled).unwrap_or(json!({})),
                ))
            })?;
        self.answer(answer)
    }

    fn call_measure(&self, arguments: &Value, client: Option<&str>) -> Result<CallResult, Refusal> {
        let group = Args::new("measure", "run", arguments).text("group")?;
        let action = Args::new("measure", "run", arguments).text("action")?;
        let inner = arguments.get("arguments").cloned().unwrap_or(json!({}));
        let args = Args::new("measure", "run", &inner);
        // Bound by the table exactly as a group call is: `build` refuses a
        // group or an action `describe` does not offer, so measuring is never
        // a side door into what the catalogue withholds.
        let command = actions::build(&group, &action, &args)?;

        if let Some(gate) = gate::gate_of(&command) {
            self.obtain(gate, &command, client)?;
        }

        let answer = self.queue.submit(self.bounds.capture, move |session| {
            // Either gesture, and for one reason: a figure taken across an
            // open gesture measures the gesture as well. Whose it is changes
            // nothing about the arithmetic.
            if session.gesture_in_progress() || session.agent_gesture_in_progress() {
                return Err(Refusal::new(
                    RefusalCode::GestureInProgress,
                    "a gesture is in progress; a figure taken across an open stroke \
                     measures the stroke as well",
                ));
            }
            let measured = session.measure(command)?;
            Ok(Answer::value(
                serde_json::to_value(&measured).unwrap_or(json!({})),
            ))
        })?;

        let mut value = answer.value;
        let mut note = String::from(
            "a live-session figure: taken with a window open and a person's session in \
             memory. Evidence, not a baseline — the baselines in benchmarks/ are \
             recorded by the harness under stated conditions and nothing here writes one.",
        );
        if value["outstanding"]
            .as_array()
            .is_some_and(|outstanding| !outstanding.is_empty())
        {
            note.push_str(
                " Work in outstanding was still running when the clock stopped, so the \
                 figure is the time to start it; wait reports it until it is done.",
            );
        }
        value["note"] = json!(note);
        Ok(CallResult::data(value))
    }

    fn call_viewport(&self, caller: &str, arguments: &Value) -> Result<CallResult, Refusal> {
        let action = arguments
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("capture");
        match action {
            "capture" => self.capture(caller, arguments),
            "compare" => self.compare(caller, arguments),
            "forget" => {
                self.remembered
                    .lock()
                    .expect("the frame table is not poisoned")
                    .retain(|(owner, _), _| owner != caller);
                Ok(CallResult::data(json!({ "remembered": 0 })))
            }
            other => Err(Refusal::new(
                RefusalCode::UnknownAction,
                format!("viewport has no action {other}; it offers capture, compare, forget"),
            )),
        }
    }

    fn capture(&self, caller: &str, arguments: &Value) -> Result<CallResult, Refusal> {
        let args = Args::new("viewport", "capture", arguments);
        let request = capture_request(&args)?;
        let settle_first = args.boolean_or("settle", true)?;
        let budget = self.bounds.settle.min(self.bounds.capture);

        let answer = self.queue.submit(self.bounds.capture, move |session| {
            let mut value = json!({});
            if settle_first {
                let settled = session.settle(budget);
                value["settled"] = serde_json::to_value(&settled).unwrap_or(json!({}));
            }
            let frame = session.capture(request)?;
            Ok(Answer {
                value,
                frame: Some(frame),
            })
        })?;

        if let Some(name) = args.optional_text("remember")? {
            if let Some(frame) = answer.frame.clone() {
                let mut remembered = self
                    .remembered
                    .lock()
                    .expect("the frame table is not poisoned");
                let key = (caller.to_owned(), name);
                if remembered
                    .keys()
                    .filter(|(owner, _)| owner == caller)
                    .count()
                    >= REMEMBERED_FRAMES
                    && !remembered.contains_key(&key)
                {
                    return Err(Refusal::new(
                        RefusalCode::BadArgument,
                        format!(
                            "{REMEMBERED_FRAMES} frames are already remembered; \
                             viewport.forget clears them"
                        ),
                    ));
                }
                remembered.insert(key, frame);
            }
        }

        self.answer(answer)
    }

    /// A difference between two frames, against what two renders of the same
    /// subject already differ by on this machine.
    ///
    /// The floor is not the same everywhere — it is zero on Linux and it is
    /// not on macOS, where a runner was measured leaving 1,294 pixels
    /// byte-differing on a frame that was meant to be unchanged. A comparison
    /// that does not carry it is a comparison an agent reads the rasteriser
    /// through.
    fn compare(&self, caller: &str, arguments: &Value) -> Result<CallResult, Refusal> {
        let args = Args::new("viewport", "compare", arguments);
        let before_name = args.text("before")?;
        let after_name = args.text("after")?;

        let (before, after) = {
            let remembered = self
                .remembered
                .lock()
                .expect("the frame table is not poisoned");
            let before = remembered
                .get(&(caller.to_owned(), before_name.clone()))
                .cloned()
                .ok_or_else(|| {
                    Refusal::new(
                        RefusalCode::BadArgument,
                        format!("no frame is remembered as {before_name}"),
                    )
                })?;
            let after = remembered
                .get(&(caller.to_owned(), after_name.clone()))
                .cloned()
                .ok_or_else(|| {
                    Refusal::new(
                        RefusalCode::BadArgument,
                        format!("no frame is remembered as {after_name}"),
                    )
                })?;
            (before, after)
        };

        if (before.width, before.height) != (after.width, after.height) {
            return Err(Refusal::new(
                RefusalCode::BadArgument,
                format!(
                    "{before_name} is {}×{} and {after_name} is {}×{}; two frames of \
                     different sizes have no comparison",
                    before.width, before.height, after.width, after.height
                ),
            ));
        }

        // The floor is measured through the same path these went through:
        // whatever the renderer and this machine leave differing between two
        // draws of a subject that did not change.
        let request = CaptureRequest {
            what: CaptureWhat::Viewport,
            width: Some(before.width),
            height: Some(before.height),
            camera: None,
        };
        let through_a_remesh = args.boolean_or("through_a_remesh", false)?;
        let budget = self.bounds.settle.min(self.bounds.capture);
        let floor_answer = self.queue.submit(self.bounds.capture, move |session| {
            let one = session.capture(request)?;
            if through_a_remesh {
                // A re-mesh can return the same surface with its vertices in a
                // different order and move a rasterised edge, so a floor for a
                // comparison that spans one has to span one too.
                let _ = session.settle(budget);
            }
            let two = session.capture(request)?;
            Ok(Answer::value(json!(differing(&one, &two))))
        })?;

        let floor = floor_answer.value.as_u64().unwrap_or(0);
        let difference = differing(&before, &after);

        Ok(CallResult::data(json!({
            "differing_pixels": difference,
            "floor": floor,
            "floor_measured": if through_a_remesh { "through a re-mesh" } else { "through two draws" },
            "past_the_floor": difference.saturating_sub(floor),
            "width": before.width,
            "height": before.height,
            "note": "the floor is what two renders of an unchanged subject already \
                     differ by on this machine. It is zero on Linux and it is not on \
                     macOS. Read past_the_floor, not differing_pixels.",
        })))
    }

    fn call_describe(&self, arguments: &Value) -> Result<CallResult, Refusal> {
        let args = Args::new("describe", "groups", arguments);
        match args.optional_text("group")? {
            None => {
                let groups: Vec<Value> = GROUPS
                    .iter()
                    .map(|(name, title, summary)| {
                        json!({
                            "group": name,
                            "title": title,
                            "summary": summary,
                            "actions": actions::actions_of(name),
                        })
                    })
                    .collect();
                Ok(CallResult::data(json!({
                    "groups": groups,
                    "not_offered": not_offered(),
                })))
            }
            Some(group) => {
                if !TABLE.iter().any(|spec| spec.group == group) {
                    return Err(Refusal::new(
                        RefusalCode::UnknownAction,
                        format!(
                            "there is no group named {group}; the groups are {}",
                            GROUPS
                                .iter()
                                .map(|(name, _, _)| *name)
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    ));
                }
                // What the layer in hand is, for the one group whose choices
                // depend on it. Asked only then: the other twenty describe the
                // same whatever is open, and a read is a trip to the interface
                // thread.
                let layer = match narrows_by_layer(&group) {
                    true => Some(self.active_representation()?),
                    false => None,
                };
                let found: Vec<Value> = TABLE
                    .iter()
                    .filter(|spec| spec.group == group)
                    .map(|spec| spec.to_json(layer))
                    .collect();
                let mut answer = json!({
                    "group": group,
                    "actions": found,
                });
                if let Some(layer) = layer {
                    answer["layer"] = json!(tags::tag_of(tags::REPRESENTATIONS, layer));
                }
                Ok(CallResult::data(answer))
            }
        }
    }

    /// What the active layer holds, read through the same path an agent reads
    /// state through.
    ///
    /// A read that does not come back is a refusal here, and deliberately: the
    /// full unnarrowed list is exactly the answer this call exists to stop
    /// being given, so it is not what a timeout falls back to.
    fn active_representation(&self) -> Result<Representation, Refusal> {
        let answer = self.queue.submit(self.bounds.call, move |session| {
            let report = session.read(StateQuery {
                tool: true,
                ..StateQuery::nothing()
            });
            Ok(Answer::value(json!(report
                .tool
                .map(|state| state.representation))))
        })?;
        let tag = answer.value.as_str().ok_or_else(|| {
            Refusal::new(
                RefusalCode::Failed,
                "the session did not say what the active layer holds, so the tools \
                 it offers cannot be narrowed to it",
            )
        })?;
        tags::REPRESENTATIONS
            .iter()
            .find(|(known, _)| *known == tag)
            .map(|(_, representation)| *representation)
            .ok_or_else(|| {
                Refusal::new(
                    RefusalCode::Failed,
                    format!("the session named a layer this surface has no word for: {tag}"),
                )
            })
    }
}

impl ToolSurface for Catalogue {
    fn tools(&self) -> Vec<ToolDescriptor> {
        let mut tools: Vec<ToolDescriptor> = GROUPS
            .iter()
            .map(|(name, title, summary)| ToolDescriptor {
                name: name.to_string(),
                title: title.to_string(),
                description: format!(
                    "{summary} Every action here dispatches the same command the \
                     interface dispatches, so the change is one history entry and one \
                     undo away. Ask describe for the actions and their arguments."
                ),
                input_schema: group_schema(name),
            })
            .collect();

        tools.push(ToolDescriptor {
            name: "describe".into(),
            title: "Vocabulary".into(),
            description: "What the groups offer, what each action takes, and which \
                          commands are deliberately not offered and why."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "group": {
                        "type": "string",
                        "description": "one group's actions in full; omit for every group's names",
                        "enum": GROUPS.iter().map(|(name, _, _)| *name).collect::<Vec<_>>(),
                    }
                },
            }),
        });

        tools.push(ToolDescriptor {
            name: "state".into(),
            title: "Estado".into(),
            description: "Reads the session without changing it: the document, the \
                          scene tree with each layer's placed forms, grid passes and \
                          hierarchy levels, the active tool and the whole brush panel, \
                          how a stroke and a placed form combine, the camera, the \
                          history with what its next undo and redo would take, the \
                          mask, the cage, the deform settings, the placed forms by \
                          node id, what the last rebuild, retopology and crossing came \
                          to, how the viewport is presented, the reference images, the \
                          import and export settings, running jobs, memory, timings, \
                          backends, and where the strokes of this session spent their \
                          milliseconds — split into the engine's own calls and this \
                          application's work around them, per tool, as a distribution \
                          rather than an average. Figures from a live session are \
                          evidence and never a benchmark baseline."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "sections": {
                        "type": "array",
                        // From the one list the reader itself is driven by, so
                        // a section added there is advertised here without a
                        // second edit. The two spellings drifting is how this
                        // surface came to advertise eleven sections against a
                        // report that could answer more.
                        "items": { "type": "string", "enum": StateQuery::NAMES },
                        "description": "which sections; omit for all of them",
                    }
                },
            }),
        });

        tools.push(ToolDescriptor {
            name: "viewport".into(),
            title: "Viewport".into(),
            description: "Sees the session: the viewport or the whole window as a PNG, \
                          and the difference between two frames read against this \
                          machine's own render floor."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["capture", "compare", "forget"] },
                    "what": {
                        "type": "string", "enum": ["viewport", "window"],
                        "description": "the surface and its overlays, or the panels and bars too",
                    },
                    "width": { "type": "integer" },
                    "height": { "type": "integer" },
                    "camera": {
                        "type": "string", "enum": ["perspective", "front", "side", "top"],
                        "description": "camera preset for this frame only; the live view is preserved",
                    },
                    "settle": {
                        "type": "boolean",
                        "description": "wait for pending re-meshing first; true where none is given",
                    },
                    "remember": {
                        "type": "string",
                        "description": "keep this frame under a name, for a later compare",
                    },
                    "before": { "type": "string", "description": "for compare" },
                    "after": { "type": "string", "description": "for compare" },
                    "through_a_remesh": {
                        "type": "boolean",
                        "description": "for compare: measure the floor through a re-mesh, \
                                        because one can move a rasterised edge on its own",
                    },
                },
                "required": ["action"],
            }),
        });

        tools.push(ToolDescriptor {
            name: "wait".into(),
            title: "Aguardar".into(),
            description: "Waits for the session to go quiet — no pending re-mesh, no \
                          running job — and names what is still running where the \
                          bound is reached."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "bound_ms": { "type": "integer", "description": "how long to wait at most" }
                },
            }),
        });

        tools.push(ToolDescriptor {
            name: "measure".into(),
            title: "Medir".into(),
            description: "Runs one action a group offers with the clock around it and \
                          reports the wall time, whether a frame stalled, the conditions, \
                          and any work it left running. A live figure is evidence, never \
                          a benchmark baseline."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "group": { "type": "string", "enum": GROUPS.iter().map(|(name, _, _)| *name).collect::<Vec<_>>() },
                    "action": { "type": "string" },
                    "arguments": { "type": "object" },
                },
                "required": ["group", "action"],
            }),
        });

        tools
    }

    fn call(&self, name: &str, arguments: &Value) -> Result<CallResult, Refusal> {
        self.call_scoped("", None, name, arguments)
    }

    fn call_scoped(
        &self,
        caller: &str,
        client: Option<&str>,
        name: &str,
        arguments: &Value,
    ) -> Result<CallResult, Refusal> {
        match name {
            "describe" => self.call_describe(arguments),
            "state" => self.call_state(arguments),
            "viewport" => self.call_viewport(caller, arguments),
            "wait" => self.call_wait(arguments),
            "measure" => self.call_measure(arguments, client),
            other => match GROUPS.iter().find(|(group, _, _)| *group == other) {
                Some((group, _, _)) => self.call_group(group, arguments, client),
                None => Err(Refusal::new(
                    RefusalCode::UnknownAction,
                    format!(
                        "there is no tool named {other}; the tools are {}, describe, \
                         state, viewport, wait and measure",
                        GROUPS
                            .iter()
                            .map(|(group, _, _)| *group)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )),
            },
        }
    }

    fn instructions(&self) -> String {
        "You are driving a running ClaySpaceDesktop session that a person may be \
         sitting in front of. Every action you take is the same command their menu \
         emits: one history entry, undone by one undo, refused where their interface \
         would refuse it.\n\n\
         Start with `describe` to learn a group's actions rather than guessing them. \
         Pass `capture: \"viewport\"` to any group call to get the frame after the \
         change in the same answer — you cannot tell whether a dab landed where you \
         meant from text. Use `wait` before judging a surface: meshing outlasts a \
         frame, and a half-meshed surface is not a defect.\n\n\
         Saving over a file, exporting, opening a document, starting a new one and \
         quitting are gated: the person is asked at the window, and you will be told \
         what would lift the gate rather than being refused silently. A stroke of \
         theirs in progress refuses your changes and serves your reads.\n\n\
         Timings you take here are live-session figures. They are evidence about this \
         machine at this moment, not benchmark baselines."
            .to_string()
    }
}

// -- helpers ----------------------------------------------------------------

fn timed_out(gate: crate::session::GateKind, command: &Command) -> Refusal {
    Refusal {
        code: RefusalCode::ConsentTimedOut,
        message: format!(
            "nobody answered the request to {} yet. It is still standing at the \
             window, so ask again once somebody has looked at it. What would lift \
             this is {}.",
            command.label(),
            gate::what_would_lift(gate)
        ),
        gate: Some(gate),
    }
}

/// A number that names one gate's question, the same every time it is asked.
fn ask_id(gate: crate::session::GateKind) -> u64 {
    crate::session::GateKind::ALL
        .iter()
        .position(|kind| *kind == gate)
        .unwrap_or(0) as u64
        + 1
}

fn word_of(outcome: ConsentOutcome) -> &'static str {
    match outcome {
        ConsentOutcome::Pending => "pending",
        ConsentOutcome::Granted => "granted",
        ConsentOutcome::AlreadyRecorded => "recorded",
        ConsentOutcome::Refused => "refused",
        ConsentOutcome::TimedOut => "timed_out",
    }
}

fn path_of(command: &Command) -> Option<PathBuf> {
    match command {
        Command::OpenRecent(path) => Some(path.clone()),
        _ => None,
    }
}

/// The `capture` argument a group call may carry.
fn capture_of(arguments: &Value) -> Result<Option<CaptureRequest>, Refusal> {
    let what = match arguments.get("capture").and_then(Value::as_str) {
        None | Some("none") => return Ok(None),
        Some("viewport") => CaptureWhat::Viewport,
        Some("window") => CaptureWhat::Window,
        Some(other) => {
            return Err(Refusal::new(
                RefusalCode::BadArgument,
                format!("capture is none, viewport or window, and {other} is not"),
            ))
        }
    };
    let args = Args::new("capture", "with", arguments);
    Ok(Some(CaptureRequest {
        what,
        width: size(&args, "width")?,
        height: size(&args, "height")?,
        camera: capture_camera(&args)?,
    }))
}

fn capture_request(args: &Args<'_>) -> Result<CaptureRequest, Refusal> {
    const WHAT: &[(&str, CaptureWhat)] = &[
        ("viewport", CaptureWhat::Viewport),
        ("window", CaptureWhat::Window),
    ];
    Ok(CaptureRequest {
        what: args.choice_or("what", WHAT, CaptureWhat::Viewport)?,
        width: size(args, "width")?,
        height: size(args, "height")?,
        camera: capture_camera(args)?,
    })
}

fn capture_camera(args: &Args<'_>) -> Result<Option<CaptureCamera>, Refusal> {
    const CAMERAS: &[(&str, CaptureCamera)] = &[
        ("perspective", CaptureCamera::Perspective),
        ("front", CaptureCamera::Front),
        ("side", CaptureCamera::Side),
        ("top", CaptureCamera::Top),
    ];
    if args.optional_text("camera")?.is_some() {
        Ok(Some(args.choice("camera", CAMERAS)?))
    } else {
        Ok(None)
    }
}

fn size(args: &Args<'_>, name: &str) -> Result<Option<u32>, Refusal> {
    match args.integer_or(name, 0)? {
        0 => Ok(None),
        value if !(0..=8192).contains(&value) => Err(Refusal::new(
            RefusalCode::BadArgument,
            format!("{name} is between 1 and 8192, and {value} is not"),
        )),
        value => Ok(Some(value as u32)),
    }
}

/// How many pixels differ at all between two frames of the same size.
fn differing(one: &Frame, two: &Frame) -> u64 {
    one.rows
        .chunks_exact(4)
        .zip(two.rows.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count() as u64
}

/// Whether describing this group has to know what the active layer holds.
///
/// One does: the sculpting tools. `ToolKind::for_representation` is what the
/// shelf draws its buttons from, and an agent is owed the same list rather
/// than one that includes four tools the ViewModel would refuse a stroke with.
fn narrows_by_layer(group: &str) -> bool {
    TABLE
        .iter()
        .filter(|spec| spec.group == group)
        .flat_map(|spec| spec.arguments)
        .any(|argument| matches!(argument.kind, Kind::Tool))
}

/// The commands that are real and deliberately not offered, with the reason.
fn not_offered() -> Vec<Value> {
    not_offered_commands()
        .iter()
        .filter_map(|command| match actions::home_of(command) {
            actions::Home::NotOffered(why) => Some(json!({
                "command": command.label(),
                "why": why,
            })),
            actions::Home::In(..) => None,
        })
        .collect()
}

/// One of each command `home_of` places as not offered.
///
/// Named rather than enumerated, because the enum cannot be iterated.
/// `home_of` guarantees every variant is placed somewhere; the test
/// `not_offered_is_complete` holds this list to every variant it places as
/// not offered, so a command withheld from agents cannot also be missing from
/// what `describe` says is withheld — which left a caller unable to tell "not
/// available" from "does not exist".
fn not_offered_commands() -> [Command; 11] {
    [
        Command::OpenDocument,
        Command::SaveAs,
        Command::InsertMesh,
        Command::LoadAlpha,
        Command::LoadReference(clayspace_model::RefPlane::Front),
        Command::ChooseBakeDestination,
        Command::RunBake,
        Command::ExportProfile,
        Command::ToggleAgentDoor,
        Command::ShowAgentAccess(false),
        Command::AnswerAgentAsk(clayspace_vm::AgentAnswer::Yes),
    ]
}

/// The schema for one group's tool: the action, every argument any of its
/// actions takes, and the capture that can ride along with the answer.
fn group_schema(group: &str) -> Value {
    let mut properties = Map::new();
    let actions: Vec<&str> = actions::actions_of(group);
    properties.insert(
        "action".into(),
        json!({ "type": "string", "enum": actions }),
    );

    for spec in TABLE.iter().filter(|spec| spec.group == group) {
        for arg in spec.arguments {
            let entry = properties.entry(arg.name.to_string()).or_insert_with(|| {
                let mut schema = arg.kind.schema();
                schema["description"] = json!(format!("{} — {}", spec.name, arg.about));
                schema
            });
            // An argument several actions share names them all, so an agent
            // reading the schema alone can tell where it belongs.
            if let Some(description) = entry.get("description").and_then(Value::as_str) {
                if !description.starts_with(spec.name) && !description.contains(spec.name) {
                    let joined = format!("{description}; {} — {}", spec.name, arg.about);
                    entry["description"] = json!(joined);
                }
            }
        }
    }

    properties.insert(
        "capture".into(),
        json!({
            "type": "string",
            "enum": ["none", "viewport", "window"],
            "description": "return the frame after this change, in the same answer",
        }),
    );
    properties.insert(
        "camera".into(),
        json!({
            "type": "string", "enum": ["perspective", "front", "side", "top"],
            "description": "camera preset for the captured frame only; the live view is preserved",
        }),
    );

    json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": ["action"],
    })
}

fn encode_png(frame: &Frame) -> Result<Vec<u8>, Refusal> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, frame.width, frame.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| Refusal::new(RefusalCode::Failed, format!("the frame: {e}")))?;
        writer
            .write_image_data(&frame.rows)
            .map_err(|e| Refusal::new(RefusalCode::Failed, format!("the frame: {e}")))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::ConsentOutcome;
    use crate::testing::FakeSession;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// A catalogue with an interface thread behind it.
    ///
    /// The drainer stands in for the event loop: it holds the only `&mut` to
    /// the session and does the work between "frames", which is the whole
    /// shape of the real arrangement.
    struct Bench {
        catalogue: Catalogue,
        session: Arc<std::sync::Mutex<FakeSession>>,
        running: Arc<AtomicBool>,
        drainer: Option<std::thread::JoinHandle<()>>,
        store: PathBuf,
    }

    impl Bench {
        fn with(session: FakeSession) -> Self {
            let store = std::env::temp_dir().join(format!(
                "clayspace-mcp-bench-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&store);
            std::fs::create_dir_all(&store).unwrap();

            let queue = JobQueue::new();
            let session = Arc::new(std::sync::Mutex::new(session));
            let running = Arc::new(AtomicBool::new(true));

            let drainer = {
                let queue = queue.clone();
                let session = Arc::clone(&session);
                let running = Arc::clone(&running);
                std::thread::spawn(move || {
                    while running.load(Ordering::SeqCst) {
                        {
                            let mut held = session.lock().expect("the session is not poisoned");
                            queue.drain(&mut *held, 16);
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                })
            };

            Self {
                catalogue: Catalogue::new(queue, &store).with_bounds(Bounds {
                    call: Duration::from_secs(5),
                    capture: Duration::from_secs(5),
                    settle: Duration::from_secs(1),
                    consent: Duration::from_secs(2),
                }),
                session,
                running,
                drainer: Some(drainer),
                store,
            }
        }

        fn new() -> Self {
            Self::with(FakeSession::new())
        }

        fn call(&self, tool: &str, arguments: Value) -> Result<CallResult, Refusal> {
            self.catalogue.call(tool, &arguments)
        }

        fn applied(&self) -> Vec<Command> {
            self.session.lock().unwrap().applied.clone()
        }

        fn record_consent(&self, tag: &str) {
            access::write_consents(&self.store, &[tag.to_string()]).unwrap();
        }
    }

    impl Drop for Bench {
        fn drop(&mut self) {
            self.running.store(false, Ordering::SeqCst);
            if let Some(drainer) = self.drainer.take() {
                let _ = drainer.join();
            }
            let _ = std::fs::remove_dir_all(&self.store);
        }
    }

    fn structured(result: &CallResult) -> Value {
        result.structured.clone().unwrap_or(json!({}))
    }

    // -- the round trip that keeps the table honest -------------------------

    /// Every row of the table builds the command it claims to, and that
    /// command lands back on the row that built it.
    ///
    /// With `home_of` exhaustive over `Command`, this closes the loop: a
    /// variant with no home does not compile, and a home with no working row
    /// fails here.
    #[test]
    fn every_action_builds_the_command_it_claims() {
        for spec in TABLE {
            let example: Value = serde_json::from_str(spec.example).unwrap_or_else(|e| {
                panic!("{}.{}'s example is not JSON: {e}", spec.group, spec.name)
            });
            let args = Args::new(spec.group, spec.name, &example);
            let command = actions::build(spec.group, spec.name, &args).unwrap_or_else(|e| {
                panic!(
                    "{}.{} did not build from its own example: {e}",
                    spec.group, spec.name
                )
            });
            assert_eq!(
                actions::home_of(&command),
                actions::Home::In(spec.group, spec.name),
                "{}.{} built a command that lives somewhere else",
                spec.group,
                spec.name
            );
        }
    }

    /// Every required argument is required: dropping one refuses rather than
    /// quietly defaulting.
    #[test]
    fn a_required_argument_is_required() {
        for spec in TABLE {
            for arg in spec.arguments.iter().filter(|arg| arg.required) {
                let mut example: Value = serde_json::from_str(spec.example).unwrap();
                example.as_object_mut().unwrap().remove(arg.name);
                let args = Args::new(spec.group, spec.name, &example);
                let refused = actions::build(spec.group, spec.name, &args);
                assert!(
                    refused.is_err(),
                    "{}.{} built without its required {}",
                    spec.group,
                    spec.name,
                    arg.name
                );
            }
        }
    }

    #[test]
    fn every_group_holds_at_least_one_action_and_has_a_schema() {
        for (group, _, _) in GROUPS {
            let actions = actions::actions_of(group);
            assert!(!actions.is_empty(), "{group} offers nothing");
            let schema = group_schema(group);
            assert_eq!(schema["type"], "object");
            assert_eq!(schema["properties"]["action"]["enum"], json!(actions));
        }
    }

    #[test]
    fn every_table_row_belongs_to_a_declared_group() {
        for spec in TABLE {
            assert!(
                GROUPS.iter().any(|(group, _, _)| *group == spec.group),
                "{}.{} is in no declared group",
                spec.group,
                spec.name
            );
        }
    }

    #[test]
    fn the_commands_not_offered_say_why() {
        let listed = not_offered();
        assert_eq!(listed.len(), not_offered_commands().len(), "{listed:?}");
        for entry in listed {
            assert!(entry["why"].as_str().unwrap().len() > 20, "{entry}");
        }
    }

    /// The variant names `home_of` places as not offered, read from its arms.
    ///
    /// Read from the source because the enum cannot be iterated, the same way
    /// the contract test reads the routing arms.
    fn variants_placed_as_not_offered() -> std::collections::BTreeSet<String> {
        let source = include_str!("actions.rs");
        let start = source.find("pub fn home_of").expect("home_of");
        let end = start + source[start..].find("\nfn unknown(").expect("its end");
        let mut placed = std::collections::BTreeSet::new();
        let mut arm: Option<(String, String)> = None;
        let mut close = |arm: Option<(String, String)>| {
            if let Some((pattern, body)) = arm {
                if body.contains("Home::NotOffered") {
                    for variant in pattern.split('|') {
                        let name: String = variant
                            .trim()
                            .chars()
                            .take_while(char::is_ascii_alphanumeric)
                            .collect();
                        placed.insert(name);
                    }
                }
            }
        };
        for line in source[start..end].lines().map(str::trim) {
            if line.starts_with("//") {
                continue;
            }
            match line.split_once(" => ") {
                Some((pattern, body)) if !pattern.starts_with("match") => {
                    close(arm.take());
                    arm = Some((pattern.to_string(), body.to_string()));
                }
                _ => {
                    if let Some((_, body)) = arm.as_mut() {
                        body.push_str(line);
                    }
                }
            }
        }
        close(arm);
        placed
    }

    /// Every command withheld from agents is listed as withheld.
    ///
    /// The list once named eight of the eleven: a bake's run and destination
    /// and the profile export were withheld without a word, so a caller could
    /// not tell "not offered here" from "does not exist".
    #[test]
    fn not_offered_is_complete() {
        let placed = variants_placed_as_not_offered();
        assert!(placed.contains("RunBake"), "the parse found {placed:?}");
        let listed: std::collections::BTreeSet<String> = not_offered_commands()
            .iter()
            .map(|command| {
                format!("{command:?}")
                    .chars()
                    .take_while(char::is_ascii_alphanumeric)
                    .collect()
            })
            .collect();
        assert_eq!(listed, placed);

        let described = structured(&Bench::new().call("describe", json!({})).unwrap());
        let labels: Vec<&str> = described["not_offered"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["command"].as_str().unwrap())
            .collect();
        for command in not_offered_commands() {
            assert!(labels.contains(&command.label()), "{labels:?}");
        }
    }

    #[test]
    fn measure_refuses_an_undeclared_group() {
        let bench = Bench::new();
        let refusal = bench
            .call(
                "measure",
                json!({ "group": "trim", "action": "run", "arguments": {} }),
            )
            .unwrap_err();
        assert_eq!(refusal.code, RefusalCode::UnknownAction);
        assert!(
            refusal.message.contains("no group named trim"),
            "{}",
            refusal.message
        );
        assert!(bench.applied().is_empty());
    }

    /// The builder behind every group call and `measure` holds to the table:
    /// a route with no row is refused rather than dispatched.
    #[test]
    fn a_route_without_a_row_is_refused_whoever_builds_it() {
        let arguments = json!({});
        let args = Args::new("bake", "run", &arguments);
        let refusal = actions::build("bake", "run", &args).unwrap_err();
        assert_eq!(refusal.code, RefusalCode::UnknownAction);
        let args = Args::new("nowhere", "run", &arguments);
        let refusal = actions::build("nowhere", "run", &args).unwrap_err();
        assert!(refusal.message.contains("no group named nowhere"));
    }

    #[test]
    fn dynamics_actions_are_offered() {
        let bench = Bench::new();
        let brush = structured(&bench.call("describe", json!({ "group": "brush" })).unwrap());
        let curve = structured(&bench.call("describe", json!({ "group": "curve" })).unwrap());
        let offered = |described: &Value, name: &str| -> Value {
            described["actions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|action| action["action"] == name)
                .cloned()
                .unwrap_or_else(|| panic!("{name} is not offered"))
        };
        for name in [
            "set_pressure_size",
            "set_pressure_strength",
            "set_pressure_curve",
            "set_taper_start",
            "set_taper_end",
            "set_rake",
        ] {
            let action = offered(&brush, name);
            assert!(
                !action["arguments"].as_array().unwrap().is_empty(),
                "{action}"
            );
            assert!(action["example"].is_object(), "{action}");
        }
        let insert = offered(&curve, "insert_point");
        assert!(
            !insert["arguments"].as_array().unwrap().is_empty(),
            "{insert}"
        );
    }

    #[test]
    fn work_started_by_measure_is_outstanding() {
        let bench = Bench::new();
        let measured = structured(
            &bench
                .call("measure", json!({ "group": "retopo", "action": "run" }))
                .unwrap(),
        );
        assert_eq!(bench.applied(), vec![Command::RunRetopology]);
        assert_eq!(measured["outstanding"][0]["what"], "retopology");
        assert!(
            measured["note"].as_str().unwrap().contains("still running"),
            "{measured}"
        );

        let waited = structured(&bench.call("wait", json!({ "bound_ms": 10 })).unwrap());
        assert_eq!(waited["quiet"], false);
        assert_eq!(waited["outstanding"][0]["what"], "retopology");
    }

    fn summary_of(group: &str, name: &str) -> &'static str {
        TABLE
            .iter()
            .find(|spec| spec.group == group && spec.name == name)
            .map(|spec| spec.summary)
            .unwrap_or_else(|| panic!("{group}.{name} has no row"))
    }

    /// Each summary that once described something else, held to what the
    /// action actually does.
    #[test]
    fn each_corrected_summary_matches_the_action() {
        // The flag the manipulator carries is a rotation snap. It was offered
        // as `invert`, which no part of the drag reads.
        let arguments = json!({ "at": [0.1, 0.0, 0.0], "snap": true });
        let args = Args::new("transform", "drag", &arguments);
        assert_eq!(
            actions::build("transform", "drag", &args).unwrap(),
            Command::DragGizmo([0.1, 0.0, 0.0], true)
        );
        let arguments = json!({ "at": [0.1, 0.0, 0.0], "invert": true });
        let args = Args::new("transform", "drag", &arguments);
        assert!(actions::build("transform", "drag", &args).is_err());

        // The document moves exactly the one grabbed point; none or several
        // is a silent no-op, which the summary has to say.
        assert!(summary_of("lattice", "drag").contains("one selected control point"));
        assert!(summary_of("lattice", "drag").contains("moves nothing"));

        // `Command::SetCombine` is how the *next* SDF edit combines.
        assert!(summary_of("layer", "set_combine").contains("next SDF edit"));

        // Flow is dab spacing and smoothing is the lazy-pointer path steady —
        // `StrokePreset::spacing` and `StrokePreset::steady`.
        assert!(summary_of("brush", "set_flow").contains("spaced"));
        assert!(summary_of("brush", "set_flow").contains("does not change how hard"));
        assert!(summary_of("brush", "set_smoothing").contains("path"));
        assert!(summary_of("brush", "set_smoothing").contains("does not relax"));

        // Both repairs are bound on a grid and nowhere else.
        for (name, operation) in [
            (
                "close_holes",
                clayspace_model::LayerOperation::CloseHoles { passes: 1 },
            ),
            ("fill_voids", clayspace_model::LayerOperation::FillVoids),
        ] {
            let verbs = operation.verbs();
            assert!(verbs.voxel.is_some());
            assert!(verbs.sdf.is_none() && verbs.mesh.is_none() && verbs.multires.is_none());
            assert!(
                summary_of("repair", name).contains("Grid layers only"),
                "{name}"
            );
        }
    }

    // -- driving ------------------------------------------------------------

    #[test]
    fn a_tool_call_dispatches_the_command_a_click_would() {
        let bench = Bench::new();
        bench
            .call("tool", json!({ "action": "select", "tool": "clay" }))
            .unwrap();
        assert_eq!(
            bench.applied(),
            vec![Command::SelectTool(clayspace_model::ToolKind::Argila)]
        );
    }

    #[test]
    fn a_stroke_is_one_gesture_and_one_history_entry() {
        let bench = Bench::new();
        bench
            .call("stroke", json!({"action":"begin","at":[0,0,0]}))
            .unwrap();
        bench
            .call("stroke", json!({"action":"continue","at":[0.1,0,0]}))
            .unwrap();
        let ended = bench.call("stroke", json!({"action":"end"})).unwrap();

        // The whole gesture arrives at once and undoes as one step: the
        // history moved by the end alone.
        assert_eq!(structured(&ended)["history_depth"], 1);
        let undone = bench.call("history", json!({"action":"undo"})).unwrap();
        assert_eq!(structured(&undone)["label"], Command::Undo.label());
    }

    #[test]
    fn an_unknown_action_names_the_ones_the_group_has() {
        let bench = Bench::new();
        let refusal = bench.call("brush", json!({ "action": "fly" })).unwrap_err();
        assert_eq!(refusal.code, RefusalCode::UnknownAction);
        assert!(refusal.message.contains("set_size"), "{}", refusal.message);
    }

    #[test]
    fn a_malformed_call_changes_nothing() {
        let bench = Bench::new();
        let refusal = bench
            .call("brush", json!({ "action": "set_size", "size": "large" }))
            .unwrap_err();
        assert_eq!(refusal.code, RefusalCode::BadArgument);
        assert!(refusal.message.contains("large"), "{}", refusal.message);
        assert!(bench.applied().is_empty());
    }

    /// The clamp travels with the answer, and the command still applies: a
    /// value brought into range is not a refusal.
    #[test]
    fn a_clamped_value_is_reported_in_the_answer() {
        let bench = Bench::new();
        let result = bench
            .call(
                "view",
                json!({ "action": "set_surface_opacity", "opacity": 5.0 }),
            )
            .unwrap();
        let answer = structured(&result);
        assert_eq!(
            answer["clamped"],
            json!([{ "argument": "opacity", "asked": 5.0, "used": 1.0 }]),
            "{answer}"
        );
        assert_eq!(bench.applied().len(), 1);
    }

    #[test]
    fn an_unknown_key_changes_nothing() {
        let bench = Bench::new();
        let refusal = bench
            .call("brush", json!({ "action": "set_size", "sizee": 0.5 }))
            .unwrap_err();
        assert_eq!(refusal.code, RefusalCode::BadArgument);
        assert!(refusal.message.contains("sizee"), "{}", refusal.message);
        assert!(bench.applied().is_empty());
    }

    #[test]
    fn a_call_with_no_action_names_the_group_s_actions() {
        let bench = Bench::new();
        let refusal = bench.call("layer", json!({})).unwrap_err();
        assert!(refusal.message.contains("select"), "{}", refusal.message);
    }

    #[test]
    fn an_unknown_tool_names_the_tools_that_exist() {
        let bench = Bench::new();
        let refusal = bench.call("sculpting", json!({})).unwrap_err();
        assert_eq!(refusal.code, RefusalCode::UnknownAction);
        assert!(refusal.message.contains("describe"), "{}", refusal.message);
    }

    /// Where the Model refuses, the tool is refused for the same reason and in
    /// the same words. It does not reach past the refusal.
    #[test]
    fn a_tool_cannot_reach_past_a_refusal_the_interface_would_give() {
        let bench =
            Bench::with(FakeSession::new().refusing("gaiola", "esta subferramenta está esticada"));
        let refusal = bench
            .call("lattice", json!({ "action": "toggle" }))
            .unwrap_err();
        assert_eq!(refusal.code, RefusalCode::ModelRefused);
        assert_eq!(refusal.message, "esta subferramenta está esticada");
    }

    // -- a person's gesture -------------------------------------------------

    #[test]
    fn a_gesture_in_progress_refuses_a_change_and_serves_a_read() {
        let bench = Bench::with(FakeSession::new().holding_a_gesture());

        let refusal = bench
            .call("stroke", json!({"action":"begin","at":[0,0,0]}))
            .unwrap_err();
        assert_eq!(refusal.code, RefusalCode::GestureInProgress);
        assert!(refusal.message.contains("gesture"), "{}", refusal.message);
        assert!(bench.applied().is_empty());

        // Reading is served: an agent watching a person work is the point.
        let state = bench
            .call("state", json!({ "sections": ["document"] }))
            .unwrap();
        assert!(structured(&state)["document"]["name"].is_string());

        // So is a view change, which touches no document.
        bench
            .call("view", json!({ "action": "toggle_grid" }))
            .unwrap();
        assert_eq!(bench.applied(), vec![Command::ToggleGrid]);
    }

    // -- the agent's own gesture --------------------------------------------

    /// An agent holding a gesture may finish it, and may do nothing else that
    /// changes the document.
    ///
    /// The exemption that lets an agent close its own stroke used to be the
    /// whole gesture guard: while the agent held one, *every* command went
    /// through, because the rule was written as "this is not a person's
    /// gesture" rather than as "this belongs to the gesture that is open".
    /// Measured over the door: `layer/add` and `history/undo` applied in the
    /// middle of the agent's own stroke, and `layer/remesh` rebuilt a layer
    /// with half a stroke in it — which left the engine reporting
    /// `clay_mesh_sculptor_flush_normals` and `clay_mesh_deltas_revert`
    /// errors and a band of clay that survived the undo.
    ///
    /// A table rather than three tests, because what has to hold is a
    /// property of the *whole* verb list and a rule stated once per verb is a
    /// rule with holes in it.
    #[test]
    fn an_agent_may_only_continue_its_own_gesture() {
        let allowed = [
            ("stroke", json!({"action":"continue","at":[0.1,0,0]})),
            ("stroke", json!({"action":"end"})),
            ("stroke", json!({"action":"cancel"})),
            // Reading is served during a gesture, as it is during a person's.
            ("view", json!({"action":"toggle_grid"})),
        ];
        for (group, arguments) in allowed {
            let bench = Bench::with(FakeSession::new().agent_holding_a_gesture());
            bench
                .call(group, arguments.clone())
                .unwrap_or_else(|refused| {
                    panic!("{group} {arguments} belongs to the open gesture: {refused}")
                });
            assert_eq!(bench.applied().len(), 1);
        }

        // Everything structural. Each of these lands inside the half-finished
        // edit the open gesture is holding.
        let refused = [
            ("layer", json!({"action":"add","representation":"field"})),
            ("layer", json!({"action":"remove","layer":1})),
            ("layer", json!({"action":"remesh","layer":1})),
            ("history", json!({"action":"undo"})),
            ("history", json!({"action":"redo"})),
            ("repair", json!({"action":"close_holes"})),
            ("convert", json!({"action":"run"})),
            // A second gesture on top of the first is the one stroke verb
            // that is not a continuation: the engine holds one gesture at a
            // time, and opening another resolves by losing the first.
            ("stroke", json!({"action":"begin","at":[0,0,0]})),
        ];
        for (group, arguments) in refused {
            let bench = Bench::with(FakeSession::new().agent_holding_a_gesture());
            let refusal = match bench.call(group, arguments.clone()) {
                Err(refusal) => refusal,
                Ok(_) => panic!("{group} {arguments} must not land mid-gesture"),
            };
            assert_eq!(refusal.code, RefusalCode::GestureInProgress);
            assert!(
                refusal.message.contains("stroke this agent opened"),
                "the refusal must name the gesture that is in the way: {}",
                refusal.message
            );
            assert!(
                bench.applied().is_empty(),
                "{group} {arguments} reached the document anyway"
            );
        }
    }

    /// And a measurement is refused for either gesture: a figure taken across
    /// an open stroke measures the stroke as well, whoever is holding it.
    #[test]
    fn a_measurement_is_refused_during_the_agents_own_gesture() {
        let bench = Bench::with(FakeSession::new().agent_holding_a_gesture());
        let refusal = bench
            .call(
                "measure",
                json!({ "group": "history", "action": "undo", "arguments": {} }),
            )
            .unwrap_err();
        assert_eq!(refusal.code, RefusalCode::GestureInProgress);
        assert!(bench.applied().is_empty());
    }

    // -- gates --------------------------------------------------------------

    #[test]
    fn a_gated_operation_is_refused_where_the_person_refuses() {
        let bench = Bench::with(FakeSession::new().answering_consent(ConsentOutcome::Refused));
        let refusal = bench
            .call("document", json!({ "action": "save" }))
            .unwrap_err();
        assert_eq!(refusal.code, RefusalCode::ConsentRefused);
        assert_eq!(refusal.gate, Some(crate::session::GateKind::Overwrite));
        assert!(bench.applied().is_empty());
        assert_eq!(bench.session.lock().unwrap().asked.len(), 1);
    }

    #[test]
    fn a_gated_operation_proceeds_where_the_person_agrees() {
        let bench = Bench::new();
        bench.call("document", json!({ "action": "save" })).unwrap();
        assert_eq!(bench.applied(), vec![Command::Save]);

        let asked = bench.session.lock().unwrap().asked.clone();
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].gate, crate::session::GateKind::Overwrite);
        assert_eq!(asked[0].operation, Command::Save.label());
    }

    #[test]
    fn a_recorded_opt_in_lifts_the_gate_without_asking() {
        let bench = Bench::new();
        bench.record_consent("exportar");
        bench
            .call("exchange", json!({ "action": "run_export" }))
            .unwrap();
        assert_eq!(bench.applied(), vec![Command::RunExport]);
        assert!(bench.session.lock().unwrap().asked.is_empty());
    }

    #[test]
    fn consent_does_not_generalise_past_the_operation_asked_for() {
        let bench = Bench::new();
        bench.call("document", json!({ "action": "save" })).unwrap();
        bench.call("document", json!({ "action": "save" })).unwrap();
        // Asked both times: agreeing once is not an opt-in.
        assert_eq!(bench.session.lock().unwrap().asked.len(), 2);
    }

    #[test]
    fn consent_names_the_client_that_requested_it() {
        let bench = Bench::new();
        bench
            .catalogue
            .call_scoped(
                "agent",
                Some("Studio Agent"),
                "document",
                &json!({ "action": "save" }),
            )
            .expect("consent and save");
        assert_eq!(
            bench.session.lock().unwrap().asked[0].client,
            "Studio Agent"
        );
    }

    #[test]
    fn an_unanswered_ask_is_refused_after_its_bound_rather_than_held() {
        let bench = Bench::with(FakeSession::new().answering_consent(ConsentOutcome::Pending));
        let started = Instant::now();
        let refusal = bench
            .call("document", json!({ "action": "quit" }))
            .unwrap_err();
        assert_eq!(refusal.code, RefusalCode::ConsentTimedOut);
        assert!(
            refusal.message.contains("would lift"),
            "{}",
            refusal.message
        );
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(bench.applied().is_empty());
    }

    #[test]
    fn a_refusal_names_the_gate_and_what_would_lift_it() {
        let bench = Bench::with(FakeSession::new().answering_consent(ConsentOutcome::Pending));
        let refusal = bench
            .call(
                "document",
                json!({ "action": "open", "path": "/tmp/a.clayspace" }),
            )
            .unwrap_err();
        assert_eq!(refusal.gate, Some(crate::session::GateKind::Open));
        assert!(refusal.message.contains("abrir"), "{}", refusal.message);
    }

    #[test]
    fn the_ask_names_the_path_where_there_is_one() {
        let bench = Bench::new();
        bench
            .call(
                "document",
                json!({ "action": "open", "path": "/tmp/cabeça.clayspace" }),
            )
            .unwrap();
        let asked = bench.session.lock().unwrap().asked.clone();
        assert_eq!(
            asked[0].path.as_deref(),
            Some(std::path::Path::new("/tmp/cabeça.clayspace"))
        );
        assert!(
            asked[0].client.is_empty(),
            "legacy clients use the locale's generic name"
        );
    }

    #[test]
    fn sculpting_is_not_gated() {
        let bench = Bench::new();
        for call in [
            json!({"action":"begin","at":[0,0,0]}),
            json!({"action":"end"}),
        ] {
            bench.call("stroke", call).unwrap();
        }
        bench
            .call("mask", json!({"action":"apply","op":"invert"}))
            .unwrap();
        bench.call("history", json!({"action":"undo"})).unwrap();
        assert!(bench.session.lock().unwrap().asked.is_empty());
    }

    // -- seeing -------------------------------------------------------------

    #[test]
    fn a_capture_rides_along_with_the_change_it_shows() {
        let bench = Bench::new();
        let result = bench
            .call(
                "stroke",
                json!({ "action": "end", "capture": "viewport", "width": 32, "height": 24 }),
            )
            .unwrap();

        // One exchange carries both the outcome and the frame after it.
        assert_eq!(structured(&result)["image"]["width"], 32);
        assert!(matches!(
            result.content.last(),
            Some(crate::protocol::Content::Image(_))
        ));
        // The frame was taken after the change reached the surface.
        assert_eq!(structured(&result)["settled"]["quiet"], true);
    }

    #[test]
    fn a_capture_names_what_is_still_running() {
        let bench = Bench::with(FakeSession::new().still_running("malha"));
        let result = bench
            .call("viewport", json!({ "action": "capture" }))
            .unwrap();
        let outstanding = &structured(&result)["image"]["outstanding"];
        assert_eq!(outstanding[0]["what"], "malha");
    }

    #[test]
    fn the_whole_window_is_a_different_picture_from_the_viewport() {
        let bench = Bench::new();
        let viewport = bench
            .call(
                "viewport",
                json!({ "action": "capture", "what": "viewport", "width": 32, "height": 24 }),
            )
            .unwrap();
        let window = bench
            .call(
                "viewport",
                json!({ "action": "capture", "what": "window", "width": 32, "height": 24 }),
            )
            .unwrap();
        assert_ne!(
            structured(&viewport)["image"]["width"],
            structured(&window)["image"]["width"]
        );
    }

    #[test]
    fn a_capture_is_a_png_a_client_can_show() {
        let bench = Bench::new();
        let result = bench
            .call(
                "viewport",
                json!({ "action": "capture", "width": 8, "height": 8 }),
            )
            .unwrap();
        match result.content.last() {
            Some(crate::protocol::Content::Image(png)) => {
                assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
            }
            other => panic!("not an image: {other:?}"),
        }
    }

    #[test]
    fn a_difference_is_reported_against_the_measured_floor() {
        let bench = Bench::new();
        bench
            .call(
                "viewport",
                json!({ "action": "capture", "width": 8, "height": 8, "remember": "antes" }),
            )
            .unwrap();
        bench.session.lock().unwrap().fill = [255, 0, 0, 255];
        bench
            .call(
                "viewport",
                json!({ "action": "capture", "width": 8, "height": 8, "remember": "depois" }),
            )
            .unwrap();

        let compared = bench
            .call(
                "viewport",
                json!({ "action": "compare", "before": "antes", "after": "depois" }),
            )
            .unwrap();
        let value = structured(&compared);
        assert_eq!(value["differing_pixels"], 64);
        // Two draws of an unchanged subject: zero here, and not zero on every
        // machine, which is the whole reason it is measured rather than
        // assumed.
        assert_eq!(value["floor"], 0);
        assert_eq!(value["past_the_floor"], 64);
    }

    #[test]
    fn remembered_frames_belong_to_the_caller() {
        let bench = Bench::new();
        let capture = json!({"action":"capture", "width":8, "height":8, "remember":"same"});
        bench
            .catalogue
            .call_scoped("first", None, "viewport", &capture)
            .unwrap();
        bench.session.lock().unwrap().fill = [255, 0, 0, 255];
        bench
            .catalogue
            .call_scoped("second", None, "viewport", &capture)
            .unwrap();

        let compare = json!({"action":"compare", "before":"same", "after":"same"});
        assert_eq!(
            structured(
                &bench
                    .catalogue
                    .call_scoped("first", None, "viewport", &compare)
                    .unwrap()
            )["differing_pixels"],
            0
        );
        bench
            .catalogue
            .call_scoped("first", None, "viewport", &json!({"action":"forget"}))
            .unwrap();
        assert!(bench
            .catalogue
            .call_scoped("first", None, "viewport", &compare)
            .is_err());
        assert!(bench
            .catalogue
            .call_scoped("second", None, "viewport", &compare)
            .is_ok());
    }

    #[test]
    fn capture_camera_is_a_one_frame_override() {
        let bench = Bench::new();
        bench
            .call("viewport", json!({"action":"capture", "camera":"front"}))
            .unwrap();
        assert_eq!(
            bench.session.lock().unwrap().last_capture.unwrap().camera,
            Some(CaptureCamera::Front)
        );
        bench.call("viewport", json!({"action":"capture"})).unwrap();
        assert_eq!(
            bench.session.lock().unwrap().last_capture.unwrap().camera,
            None
        );
    }

    #[test]
    fn two_frames_of_different_sizes_have_no_comparison() {
        let bench = Bench::new();
        bench
            .call(
                "viewport",
                json!({"action":"capture","width":8,"height":8,"remember":"a"}),
            )
            .unwrap();
        bench
            .call(
                "viewport",
                json!({"action":"capture","width":16,"height":8,"remember":"b"}),
            )
            .unwrap();
        let refusal = bench
            .call(
                "viewport",
                json!({"action":"compare","before":"a","after":"b"}),
            )
            .unwrap_err();
        assert!(
            refusal.message.contains("different sizes"),
            "{}",
            refusal.message
        );
    }

    #[test]
    fn a_frame_nobody_remembered_is_named_rather_than_guessed() {
        let bench = Bench::new();
        let refusal = bench
            .call(
                "viewport",
                json!({"action":"compare","before":"a","after":"b"}),
            )
            .unwrap_err();
        assert!(
            refusal.message.contains("no frame is remembered as a"),
            "{}",
            refusal.message
        );
    }

    #[test]
    fn a_size_outside_the_bounds_is_refused() {
        let bench = Bench::new();
        let refusal = bench
            .call("viewport", json!({"action":"capture","width":100000}))
            .unwrap_err();
        assert!(refusal.message.contains("8192"), "{}", refusal.message);
    }

    // -- reading and waiting ------------------------------------------------

    #[test]
    fn reading_answers_the_sections_asked_for_and_no_others() {
        let bench = Bench::new();
        let result = bench
            .call("state", json!({ "sections": ["scene"] }))
            .unwrap();
        let value = structured(&result);
        assert!(value["scene"]["layers"].is_array());
        assert!(value["document"].is_null());
    }

    #[test]
    fn reading_everything_is_the_default() {
        let bench = Bench::new();
        let value = structured(&bench.call("state", json!({})).unwrap());
        for section in ["document", "scene", "tool", "camera", "history", "memory"] {
            assert!(!value[section].is_null(), "{section} is missing");
        }
    }

    #[test]
    fn reading_changes_nothing() {
        let bench = Bench::new();
        for _ in 0..5 {
            bench.call("state", json!({})).unwrap();
        }
        assert!(bench.applied().is_empty());
        assert_eq!(bench.session.lock().unwrap().reads.get(), 5);
    }

    #[test]
    fn waiting_names_what_is_still_running_when_the_bound_is_reached() {
        let bench = Bench::with(FakeSession::new().still_running("exportação"));
        let value = structured(&bench.call("wait", json!({ "bound_ms": 10 })).unwrap());
        assert_eq!(value["quiet"], false);
        assert_eq!(value["outstanding"][0]["what"], "exportação");
    }

    // -- measuring ----------------------------------------------------------

    #[test]
    fn a_measured_figure_says_it_came_from_a_live_session() {
        let bench = Bench::new();
        let value = structured(
            &bench
                .call("measure", json!({ "group": "history", "action": "undo" }))
                .unwrap(),
        );
        assert_eq!(value["live_session"], true);
        assert!(value["millis"].is_number());
        assert!(value["note"].as_str().unwrap().contains("not a baseline"));
        assert_eq!(bench.applied(), vec![Command::Undo]);
    }

    // -- describing ---------------------------------------------------------

    #[test]
    fn describe_lists_every_group_and_what_is_not_offered() {
        let bench = Bench::new();
        let value = structured(&bench.call("describe", json!({})).unwrap());
        assert_eq!(value["groups"].as_array().unwrap().len(), GROUPS.len());
        assert!(!value["not_offered"].as_array().unwrap().is_empty());
    }

    #[test]
    fn describe_titles_use_the_protocols_english_vocabulary() {
        for (tag, title, _) in GROUPS {
            assert!(title.is_ascii(), "{tag} has an untranslated title: {title}");
            assert_ne!(*title, "Multires");
        }
        assert_eq!(
            GROUPS
                .iter()
                .find(|(tag, _, _)| *tag == "hierarchy")
                .unwrap()
                .1,
            "Hierarchy"
        );
    }

    #[test]
    fn describe_gives_a_group_its_arguments_and_an_example() {
        let bench = Bench::new();
        let value = structured(&bench.call("describe", json!({ "group": "brush" })).unwrap());
        let actions = value["actions"].as_array().unwrap();
        let size = actions
            .iter()
            .find(|action| action["action"] == "set_size")
            .expect("set_size");
        assert_eq!(size["arguments"][0]["name"], "size");
        assert_eq!(size["arguments"][0]["required"], true);
        assert_eq!(size["example"]["size"], 0.12);
    }

    #[test]
    fn describe_spells_out_a_choice_rather_than_naming_a_type() {
        let bench = Bench::new();
        let value = structured(&bench.call("describe", json!({ "group": "tool" })).unwrap());
        let choices = value["actions"][0]["arguments"][0]["choices"]
            .as_array()
            .unwrap();
        assert!(choices.iter().any(|choice| choice == "clay"), "{choices:?}");
    }

    /// The choices an agent is offered are the buttons the shelf draws.
    ///
    /// Issue #127: four tools — erase, scrape, fill and nudge — have no verb on
    /// a field, which `ToolKind::verbs` says outright and the shelf acts on by
    /// not drawing them. This surface offered all twenty-one on every layer, so
    /// an agent chose one of the four, stroked, and was told it had worked.
    ///
    /// Pinch was the fourth when #127 was written and is not any more: #201
    /// bound it to `clay_layer_magnify_surface` at a negative strength, so a
    /// field offers it and this test would be asserting the opposite of the
    /// table. Erase stands in its place — `clay_voxel_erase_brush` is its only
    /// verb, on the grid, so a field has none to offer.
    #[test]
    fn describe_offers_only_the_tools_the_layer_has_a_verb_for() {
        let bench = Bench::with(FakeSession::new().on_a("field"));
        let value = structured(&bench.call("describe", json!({ "group": "tool" })).unwrap());
        let argument = &value["actions"][0]["arguments"][0];
        let choices = argument["choices"].as_array().unwrap();
        let missing = argument["no_verb_here"].as_array().unwrap();

        assert_eq!(value["layer"], "field");
        assert_eq!(argument["choices_on"], "field");
        for absent in ["erase", "scrape", "fill", "nudge"] {
            assert!(
                !choices.iter().any(|choice| choice == absent),
                "{absent} is offered on a field: {choices:?}"
            );
            assert!(
                missing.iter().any(|tool| tool == absent),
                "{absent} is not named as having no verb here: {missing:?}"
            );
        }
        assert!(choices.iter().any(|choice| choice == "clay"), "{choices:?}");
    }

    /// And narrowing is not deletion: scrape, one of the four a field leaves
    /// out, is on a mesh shelf, so it is offered there.
    #[test]
    fn describe_offers_a_tool_on_a_layer_that_has_a_verb_for_it() {
        let bench = Bench::with(FakeSession::new().on_a("mesh"));
        let value = structured(&bench.call("describe", json!({ "group": "tool" })).unwrap());
        let argument = &value["actions"][0]["arguments"][0];
        let choices = argument["choices"].as_array().unwrap();
        assert_eq!(value["layer"], "mesh");
        assert!(
            choices.iter().any(|choice| choice == "scrape"),
            "{choices:?}"
        );
        assert!(
            !argument["no_verb_here"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "scrape"),
            "{argument}"
        );
    }

    /// Walked against every representation, which is the test the issue asked
    /// for: what a surface offers is what the shelf shows, layer by layer.
    #[test]
    fn what_the_surface_offers_is_what_the_shelf_shows_on_every_layer() {
        for (tag, representation) in tags::REPRESENTATIONS {
            let bench = Bench::with(FakeSession::new().on_a(tag));
            let value = structured(&bench.call("describe", json!({ "group": "tool" })).unwrap());
            let offered: Vec<&str> = value["actions"][0]["arguments"][0]["choices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|choice| choice.as_str().unwrap())
                .collect();
            let shelf: Vec<&str> = clayspace_model::ToolKind::for_representation(*representation)
                .into_iter()
                .map(clayspace_model::ToolKind::key)
                .collect();
            assert_eq!(offered, shelf, "on a {tag} layer");
        }
    }

    /// The wire's word for a layer, and only the wire's.
    ///
    /// "SDF" and "voxel" are the interface's names for two of these and
    /// "field" and "grid" are the wire's. A surface that says both has two
    /// vocabularies on it, and an agent that learned either is right half the
    /// time.
    #[test]
    fn describe_names_the_layer_in_the_wire_s_own_words() {
        let bench = Bench::with(FakeSession::new().on_a("field"));
        let value = structured(&bench.call("describe", json!({ "group": "tool" })).unwrap());
        let said = value.to_string().to_lowercase();
        for interface_word in ["sdf", "voxel", "multires", "campo", "grade"] {
            assert!(!said.contains(interface_word), "{interface_word} in {said}");
        }
    }

    #[test]
    fn describe_refuses_a_group_that_does_not_exist_and_names_the_ones_that_do() {
        let bench = Bench::new();
        let refusal = bench
            .call("describe", json!({ "group": "sculpting" }))
            .unwrap_err();
        assert!(refusal.message.contains("brush"), "{}", refusal.message);
    }

    #[test]
    fn the_tool_list_carries_every_group_and_the_five_that_are_not_commands() {
        let bench = Bench::new();
        let tools = bench.catalogue.tools();
        assert_eq!(tools.len(), GROUPS.len() + 5);
        for extra in ["describe", "state", "viewport", "wait", "measure"] {
            assert!(tools.iter().any(|tool| tool.name == extra), "{extra}");
        }
        for tool in &tools {
            assert_eq!(tool.input_schema["type"], "object");
            assert!(!tool.description.is_empty());
        }
    }

    #[test]
    fn the_instructions_say_what_an_agent_needs_before_it_starts() {
        let bench = Bench::new();
        let instructions = bench.catalogue.instructions();
        assert!(instructions.contains("describe"));
        assert!(instructions.contains("capture"));
        assert!(instructions.contains("gated"));
    }
}
