//! What this build is, and what it decided to run on.
//!
//! Written for one purpose: to turn a report from a user into something
//! actionable. Nearly every defect this project has filed upstream needed the
//! engine revision, the active backend and whether anything fell back — and
//! all three were reconstructed by hand from a conversation. This is that
//! conversation, prepared in advance.
//!
//! Plain strings rather than the engine's own enumerations. This layer has no
//! dependencies at all, which is what keeps it testable without a machine that
//! happens to have the right hardware; the engine words its own values on the
//! way in.

use std::time::Duration;

use crate::profile::{Phase, StrokeProfile};

/// Whether a second party could have been driving this session.
///
/// In the report for the reason the engine revision and the container minor
/// are: a defect report that does not say an agent was driving is a report
/// whose steps cannot be trusted to be the whole of what happened. "It moved
/// on its own" and "an agent applied forty strokes" are the same symptom with
/// different causes, and only one of them is a defect in this application.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentDiagnostics {
    pub listening: bool,
    /// Where a client would connect. **Never the secret**: a report is pasted
    /// into issues and chat windows, and a secret that reaches one of those is
    /// a session anyone reading it can drive.
    pub address: String,
    pub connected: usize,
    /// How many of this session's commands arrived from an agent.
    pub commands: u64,
}

/// One operation that ran somewhere other than the active backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fallback {
    /// The operation, in the words the engine uses for it.
    pub operation: String,
    /// The backend that declined it.
    pub declined_by: String,
}

/// Everything a bug report should carry.
///
/// Not `Eq`: the rendering section carries GPU milliseconds, and two frame
/// timings are never equal in the sense `Eq` promises.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Diagnostics {
    pub app_version: String,
    /// The engine's own version string.
    pub engine_version: String,
    /// The revision of the vendored engine this build was compiled against.
    ///
    /// Not the same question as the version: two builds can both say 0.27.3
    /// and differ by a commit, which is exactly the case that cost this
    /// project a round of issues filed against a stale engine.
    pub engine_revision: String,
    /// The `.clayspace` container version this build writes, as `major.minor`.
    ///
    /// In the report because it is the answer to the one question a document
    /// this build wrote can raise elsewhere: a build older than the one that
    /// introduced a minor *refuses* the file rather than misreading it, so
    /// "it will not open on the other machine" has a number behind it that a
    /// person can quote.
    pub document_format: String,
    pub platform: String,

    /// Every backend the engine registered on this machine.
    pub backends: Vec<String>,
    pub active_backend: String,
    /// Why that one, worded rather than encoded.
    pub selection: String,
    /// Operations that fell back this session, each recorded once.
    pub fallbacks: Vec<Fallback>,

    /// The graphics adapter the viewport is drawing on, once one exists.
    ///
    /// Optional because diagnostics are readable before the window is, and a
    /// report that cannot be produced until the GPU is up is no use for
    /// diagnosing a GPU that did not come up.
    pub renderer: Option<String>,

    /// Operations that held the interface thread longer than a frame.
    ///
    /// In the report because "it stutters" is the most common thing a user
    /// says and the least actionable, and this turns it into a name and a
    /// number.
    pub stalls: Vec<String>,

    /// What the viewport is costing, once a frame has been drawn.
    ///
    /// Optional for the reason `renderer` is: the report is readable before
    /// the window is, and one that could not be produced until the GPU was up
    /// would be no use for diagnosing a GPU that did not come up.
    pub render: Option<RenderDiagnostics>,

    /// What mesh sculpting has had to correct for itself.
    ///
    /// Optional because it is the document's answer rather than this build's,
    /// and the report is assembled by the layer that knows the build. A
    /// report taken with no document open carries `None`.
    pub mesh: Option<MeshDiagnostics>,

    /// What the subdivision hierarchies cost, and which of them were lost.
    ///
    /// Optional for the reason [`Self::mesh`] is: it is the document's answer
    /// rather than this build's, and a report taken with no document open
    /// carries `None`.
    pub hierarchies: Option<MultiresDiagnostics>,

    /// Where the document's memory is, and what it would cost to release it.
    ///
    /// Optional for the reason [`Self::mesh`] is: it is the document's answer
    /// rather than this build's, and a report taken with no document open
    /// carries `None`.
    pub memory: Option<MemoryDiagnostics>,
    /// The agent-facing door, where this build has one.
    pub agent: Option<AgentDiagnostics>,

    /// Where a stroke's milliseconds went, phase by phase.
    ///
    /// In the report because the line above it — an operation and a total —
    /// is the one thing neither party can act on. A re-mesh reported as 42 ms
    /// spans an engine call and this application's work around it, and nobody
    /// reading that number can tell whose it was.
    ///
    /// Optional for the reason [`Self::render`] is: the report is readable
    /// before anything has been sculpted, and a section that could not be
    /// produced until a stroke had run would be no use for diagnosing a
    /// session that cannot run one.
    pub stroke: Option<StrokeDiagnostics>,

    /// What a brick refill was measured to cost on each backend.
    ///
    /// The evidence the routing decision is actually made on, which until now
    /// was visible to nothing but a test. It is also the figure behind this
    /// project's sharpest engine finding — that an accelerated backend can be
    /// several times *slower* than the CPU on a given machine — and that is a
    /// fact about the engine which only a host that measures both is in a
    /// position to report.
    pub refill: Option<RefillDiagnostics>,
}

/// What a refill costs per brick on each backend the routing considered.
#[derive(Debug, Clone, PartialEq)]
pub struct RefillDiagnostics {
    /// The accelerated backend, in the engine's own word for it.
    pub accelerated: String,
    /// Nanoseconds per brick on the CPU, once one refill has been timed there.
    pub cpu: Option<f64>,
    /// The same on the accelerated backend.
    pub accelerated_cost: Option<f64>,
}

/// What one phase of a stroke has cost this session.
///
/// Plain fields rather than the profile's own types, as the rest of this
/// module is: the layer that renders a report should not have to know how the
/// window behind a quantile is kept.
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseCost {
    /// The phase, in the words the report uses for it.
    pub phase: String,
    /// Whether the time was the engine's.
    pub engine: bool,
    /// Which engine call, where it is the engine's and there is one to name.
    pub entry_point: Option<String>,
    /// Every sample this session, including any the window has dropped.
    pub samples: u64,
    /// `None` where the phase never ran — which is a different fact from
    /// costing nothing, and is reported as one.
    pub median: Option<Duration>,
    /// The tail a sculptor actually feels, which a median hides.
    pub p95: Option<Duration>,
    pub worst: Option<Duration>,
    /// Keys re-meshed across those samples, summed.
    pub keys: usize,
    /// Triangles produced across those samples, summed.
    pub triangles: usize,
    /// Bricks dirtied across those samples, summed.
    pub bricks: usize,
}

/// Where a stroke's milliseconds went.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StrokeDiagnostics {
    /// Every phase, in the order they run, whether or not each has samples.
    pub phases: Vec<PhaseCost>,
    /// The tools that ran, for the report's summary line.
    pub tools: usize,
}

impl StrokeDiagnostics {
    /// Reads a session's profile into the shape the report renders.
    ///
    /// Every phase is carried, including the ones with nothing in them. A
    /// section that showed only the phases that ran would leave a reader
    /// unable to tell a phase that cost nothing from a phase this build does
    /// not measure.
    pub fn of(profile: &StrokeProfile) -> Self {
        let whole = profile.across_tools();
        let phases = Phase::ALL
            .iter()
            .map(|phase| {
                let samples = whole.phase(*phase);
                let work = samples.work();
                // One sort rather than two: this is assembled while a report
                // is being built, and the composition root builds one whenever
                // something is going to read it.
                let summary = samples.summary();
                PhaseCost {
                    phase: phase.label().to_string(),
                    engine: phase.is_engine(),
                    entry_point: phase.entry_point().map(str::to_string),
                    samples: samples.seen(),
                    median: summary.map(|s| s.median),
                    p95: summary.map(|s| s.p95),
                    worst: summary.map(|s| s.worst),
                    keys: work.keys,
                    triangles: work.triangles,
                    bricks: work.bricks,
                }
            })
            .collect();
        Self {
            phases,
            tools: profile.tools().filter(|(_, tool)| !tool.is_empty()).count(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.phases.iter().all(|phase| phase.samples == 0)
    }
}

/// Where a document's memory is, in the terms that decide what may be released.
///
/// A single total is the wrong answer to the question a memory warning
/// actually asks. Under pressure a host does not need to know how big the
/// document is, it needs to know **which part**, because that is what decides
/// what it is allowed to let go of:
///
/// | | what letting it go costs |
/// |---|---|
/// | [`essential`](Self::essential) | the user's work. Never. |
/// | [`rebuildable`](Self::rebuildable) | a stall, and nothing else — it reconstructs identically |
/// | [`undoable`](Self::undoable) | undo depth, which is this application's own policy |
///
/// The three are the engine's own arithmetic over its category lines rather
/// than a sum taken here, so a line added upstream and not classified cannot
/// make them disagree with the total.
///
/// [`surfaces`](Self::surfaces) and [`surface_bytes`](Self::surface_bytes) are
/// in the report because of what they would otherwise hide. A hierarchy and a
/// mesh-sculpting session are held *beside* a document rather than inside it,
/// so the engine's plain roll-up reports them as zero — correctly, since it
/// cannot walk what it does not own. A host that stops there and shows the
/// plain figure at twenty million vertices publishes a number that omits the
/// largest thing the artist is holding. These two say that this application
/// asked its own surfaces and folded the answer in, and how much that was, so
/// a zero can be read as "there are none" rather than as "we did not ask".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemoryDiagnostics {
    /// The user's work. Never released.
    pub essential: u64,
    /// Reconstructs identically; releasing it costs a stall.
    pub rebuildable: u64,
    /// Undo depth, and this application's own policy.
    pub undoable: u64,
    /// What the engine reports for the whole, roll-ups and everything else.
    pub total: u64,
    /// How many surfaces this application asked for a ledger.
    pub surfaces: usize,
    /// What those surfaces contributed to the figures above.
    pub surface_bytes: u64,
}

/// What mesh sculpting has had to correct for itself.
///
/// One figure so far, and it is here rather than in a log because the thing it
/// reports is otherwise **silent by construction**. A mesh brush is told which
/// weld class to start its surface walk from, and a class is an index into a
/// numbering that this application retires whenever it rebuilds a sculptor —
/// an eviction, a removed subtool, an undo, a re-mesh. An index from a retired
/// numbering is still in bounds, so nothing refuses it; the walk simply starts
/// somewhere else and comes back empty, and the sculptor sees a brush that did
/// nothing. That is indistinguishable from a fully masked stroke from every
/// side but this one.
///
/// The engine catches it because the class travels with a token naming the
/// numbering it came from, and falls back to a scan. This is the count of
/// those catches: zero is the ordinary reading, and a figure that climbs says
/// the mechanism is earning its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeshDiagnostics {
    /// Mesh sculptors the document is holding.
    ///
    /// Beside the count below because zero rejections over no sculptors and
    /// zero rejections over four are the same number and different facts.
    pub sculptors: usize,
    /// Stamps whose seed named a numbering that had been retired.
    pub stale_seeds_rejected: usize,
}

/// What the subdivision hierarchies in this document cost, and which of them
/// did not survive being reopened.
///
/// Here for the reason [`MeshDiagnostics`] is here: the thing it reports is
/// otherwise silent by construction. A `.clayspace` carries a hierarchy's cage
/// and nothing standing on it — the engine's own ownership boundary, stated in
/// the C header — so the sculpt travels in a file beside the document. Open
/// the document without that file and every row comes back as the mesh layer
/// the document says it is: the cage, flat, with nothing refusing and nothing
/// warning. The representation changing is what a sculptor sees; this is what
/// they can paste.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MultiresDiagnostics {
    /// Hierarchies this document is holding.
    ///
    /// Beside the list below for the reason the sculptor count is beside the
    /// rejection count: no losses over no hierarchies and no losses over four
    /// are the same emptiness and different facts.
    pub held: usize,
    /// What this session could not put back, by name.
    ///
    /// A row where a record was found and could not be honoured; and,
    /// where the file itself could not be parsed into records at all, the
    /// file — a damaged side-car cannot say which rows it was holding, since
    /// that is what being damaged means, so it is named as one loss rather
    /// than as none.
    ///
    /// A side-car that is missing **altogether** is not here and cannot be:
    /// nothing in a `.clayspace` distinguishes a document that never held a
    /// hierarchy from one whose side-car went missing, which is the whole of
    /// why the side-car is load-bearing rather than decorative. A side-car
    /// that is *present and damaged* is a different fact and is named.
    pub lost: Vec<String>,
}

/// What the viewport drew, and what it cost.
///
/// The project measures its sculpting path carefully and measured its
/// rendering path not at all, which made every claim about rendering cost an
/// argument rather than a number. This is the number.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderDiagnostics {
    /// The scene rectangle, in physical pixels — not the window's.
    pub viewport: [u32; 2],
    /// Samples per pixel the scene is drawn with.
    pub samples: u32,
    /// The occlusion pass's own resolution and settings, when it runs.
    pub ao: Option<AoDiagnostics>,
    /// GPU milliseconds per pass, in the order the passes run.
    ///
    /// Empty where the adapter has no timestamp queries, which is a different
    /// thing from every pass costing nothing — [`Self::gpu_timing`] tells the
    /// two apart.
    pub gpu_passes: Vec<(String, f32)>,
    /// Whether the adapter reports GPU time at all.
    pub gpu_timing: bool,
    pub draw_calls: u32,
    /// Subtools the frustum test removed before they were drawn.
    pub culled: u32,
    pub triangles: u64,
    /// Indices drawn as lines: the polyframe and the scaffolding.
    pub lines: u64,
    /// Bytes written to the device since the last report.
    pub uploaded_bytes: u64,
}

/// How the occlusion pass is configured for the frame being reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AoDiagnostics {
    pub width: u32,
    pub height: u32,
    pub samples: u32,
    pub temporal: bool,
}

impl Diagnostics {
    /// The report as text, for the clipboard.
    ///
    /// The whole point of the panel: a person pastes this into an issue rather
    /// than transcribing it, and nothing important is lost to retyping.
    pub fn to_report(&self) -> String {
        let mut out = String::new();
        let mut line = |key: &str, value: &str| {
            out.push_str(key);
            out.push_str(": ");
            out.push_str(value);
            out.push('\n');
        };

        line("application", &self.app_version);
        line("engine", &self.engine_version);
        line("engine revision", &self.engine_revision);
        line("document format", &self.document_format);
        line("platform", &self.platform);
        line("backends", &self.backends.join(", "));
        line(
            "active",
            &format!("{} ({})", self.active_backend, self.selection),
        );
        if let Some(renderer) = &self.renderer {
            line("renderer", renderer);
        }
        if self.stalls.is_empty() {
            line("stalls", "none over one frame");
        } else {
            for stall in &self.stalls {
                line("stall", stall);
            }
        }
        match &self.agent {
            Some(agent) if agent.listening => {
                line(
                    "agent",
                    &format!(
                        "listening on {}, {} connected, {} commands this session",
                        agent.address, agent.connected, agent.commands
                    ),
                );
            }
            // Said explicitly rather than omitted. A missing section reads as
            // "this build has no door", and the two are different answers.
            Some(_) => line("agent", "not listening"),
            None => line("agent", "not built with a door"),
        }
        if self.fallbacks.is_empty() {
            line("fallbacks", "none this session");
        } else {
            for fallback in &self.fallbacks {
                line(
                    "fallback",
                    &format!("{} declined {}", fallback.declined_by, fallback.operation),
                );
            }
        }
        if let Some(render) = &self.render {
            line(
                "viewport",
                &format!(
                    "{}x{} @ {}x MSAA",
                    render.viewport[0], render.viewport[1], render.samples
                ),
            );
            match &render.ao {
                Some(ao) => line(
                    "ao",
                    &format!(
                        "{}x{}, {} samples, temporal {}",
                        ao.width,
                        ao.height,
                        ao.samples,
                        if ao.temporal { "on" } else { "off" }
                    ),
                ),
                None => line("ao", "off"),
            }
            line(
                "geometry",
                &format!(
                    "{} draws ({} culled), {} triangles, {} lines, {} bytes uploaded",
                    render.draw_calls,
                    render.culled,
                    render.triangles,
                    render.lines,
                    render.uploaded_bytes
                ),
            );
            if render.gpu_timing {
                for (pass, ms) in &render.gpu_passes {
                    line(&format!("gpu {pass}"), &format!("{ms:.2} ms"));
                }
                if render.gpu_passes.is_empty() {
                    line("gpu", "no frame measured yet");
                }
            } else {
                line("gpu", "timestamps unavailable on this adapter");
            }
        }
        if let Some(mesh) = &self.mesh {
            line(
                "mesh sculptors",
                &format!(
                    "{} held, {} stale seeds rejected",
                    mesh.sculptors, mesh.stale_seeds_rejected
                ),
            );
        }
        if let Some(hierarchies) = &self.hierarchies {
            if hierarchies.held > 0 || !hierarchies.lost.is_empty() {
                line(
                    "hierarchies",
                    &format!(
                        "{} held, {} lost{}",
                        hierarchies.held,
                        hierarchies.lost.len(),
                        if hierarchies.lost.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", hierarchies.lost.join(", "))
                        }
                    ),
                );
            }
        }
        if let Some(memory) = &self.memory {
            // The breakdown before the total, deliberately: the total is the
            // part a reader already has an intuition for and the split is the
            // part that decides anything.
            line(
                "memory",
                &format!(
                    "{} essential, {} rebuildable, {} undoable of {} total",
                    megabytes(memory.essential),
                    megabytes(memory.rebuildable),
                    megabytes(memory.undoable),
                    megabytes(memory.total)
                ),
            );
            // Reported at zero surfaces as well, because that is the line that
            // says the surfaces were *asked*. Without it a zero surface tier
            // cannot be told from a host that never filled the ledger.
            line(
                "memory surfaces",
                &format!(
                    "{} held, {} folded in",
                    memory.surfaces,
                    megabytes(memory.surface_bytes)
                ),
            );
        }
        if let Some(refill) = &self.refill {
            line(
                "refill",
                &format!(
                    "cpu {}, {} {}",
                    per_brick(refill.cpu),
                    refill.accelerated,
                    per_brick(refill.accelerated_cost)
                ),
            );
        }
        if let Some(stroke) = &self.stroke {
            self.report_stroke(stroke, &mut line);
        }
        out
    }

    /// The stroke section: one line per phase, in the order they run.
    ///
    /// Whether the time was the engine's is said on every line rather than
    /// implied by a heading, because these lines are pasted into an issue one
    /// at a time as often as the whole report is pasted at once.
    fn report_stroke(&self, stroke: &StrokeDiagnostics, line: &mut impl FnMut(&str, &str)) {
        if stroke.is_empty() {
            return line("stroke", "no samples this session");
        }
        line("stroke", &format!("{} tools measured", stroke.tools));
        for phase in &stroke.phases {
            let key = format!("stroke {}", phase.phase);
            line(&key, &phase.describe());
        }
    }
}

impl PhaseCost {
    /// The phase as a report line, saying whose time it was.
    pub fn describe(&self) -> String {
        let side = match (&self.entry_point, self.engine) {
            (Some(entry), _) => format!("engine, {entry}"),
            (None, true) => "engine".to_string(),
            (None, false) => "ours".to_string(),
        };
        let (Some(median), Some(worst)) = (self.median, self.worst) else {
            return format!("no samples ({side})");
        };
        format!(
            "{} samples, {} median, {} worst, over {} ({side})",
            self.samples,
            milliseconds(median),
            milliseconds(worst),
            self.describe_work()
        )
    }

    /// What the samples behind the figures covered.
    ///
    /// A duration without it is not comparable with any other duration: eleven
    /// milliseconds over four keys and eleven over ninety are the same number
    /// and opposite facts.
    fn describe_work(&self) -> String {
        if self.keys == 0 && self.triangles == 0 {
            return format!("{} bricks", self.bricks);
        }
        format!("{} keys, {} triangles", self.keys, self.triangles)
    }
}

/// A duration as the report states one.
fn milliseconds(took: Duration) -> String {
    format!("{:.2} ms", took.as_secs_f64() * 1000.0)
}

/// A measured per-brick cost, or the fact that there is not one yet.
///
/// "not measured" rather than zero, for the reason every other absent figure
/// in this module says so: a backend that has not been timed and a backend
/// that costs nothing are different claims, and only one of them is ever true.
fn per_brick(cost: Option<f64>) -> String {
    match cost {
        Some(ns) => format!("{ns:.0} ns/brick"),
        None => "not measured".to_string(),
    }
}

/// Bytes as a figure a person can compare against what their machine has.
///
/// Mebibytes, since that is what the engine's budgets are stated in, and one
/// decimal: a memory report read to the byte invites a reader to treat a
/// container walk as an equality, which the engine is explicit it is not.
fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

/// A source of diagnostics.
pub trait DiagnosticsModel {
    fn diagnostics(&self) -> Diagnostics;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Diagnostics {
        Diagnostics {
            app_version: "0.1.0".into(),
            engine_version: "0.27.3".into(),
            engine_revision: "804fc9d".into(),
            document_format: "1.16".into(),
            platform: "macos aarch64".into(),
            backends: vec!["cpu".into(), "metal".into()],
            active_backend: "metal".into(),
            selection: "automática".into(),
            fallbacks: Vec::new(),
            renderer: Some("Apple M3 Max (Metal)".into()),
            stalls: Vec::new(),
            render: None,
            mesh: None,
            hierarchies: None,
            memory: None,
            agent: None,
            stroke: None,
            refill: None,
        }
    }

    #[test]
    fn the_report_carries_what_an_issue_needs() {
        let text = sample().to_report();
        for expected in [
            "0.1.0",
            "0.27.3",
            "804fc9d",
            "macos",
            "cpu, metal",
            "metal",
            "Apple M3 Max",
        ] {
            assert!(
                text.contains(expected),
                "the report lost {expected}:\n{text}"
            );
        }
    }

    #[test]
    fn a_session_with_no_fallbacks_says_so_rather_than_staying_silent() {
        // Silence reads as "the panel is broken", and a reader cannot tell it
        // apart from "nothing was recorded".
        assert!(sample().to_report().contains("none this session"));
    }

    #[test]
    fn a_fallback_names_who_declined_and_what() {
        let mut diagnostics = sample();
        diagnostics.fallbacks.push(Fallback {
            operation: "raycast".into(),
            declined_by: "opencl".into(),
        });
        let text = diagnostics.to_report();
        assert!(text.contains("opencl declined raycast"), "{text}");
        assert!(!text.contains("none this session"));
    }

    #[test]
    fn a_report_taken_before_the_window_exists_omits_the_renderer() {
        let mut diagnostics = sample();
        diagnostics.renderer = None;
        let text = diagnostics.to_report();
        assert!(!text.contains("renderer"), "{text}");
        // And still carries the part that diagnoses why there is no window.
        assert!(text.contains("backends"), "{text}");
    }

    #[test]
    fn a_stall_reaches_the_report_because_it_stutters_is_not_actionable() {
        let mut diagnostics = sample();
        diagnostics.stalls.push("consolidar 6400 ms".into());
        let text = diagnostics.to_report();
        assert!(text.contains("stall: consolidar 6400 ms"), "{text}");
        assert!(!text.contains("none over one frame"));
    }

    #[test]
    fn a_smooth_session_says_so_rather_than_staying_silent() {
        assert!(sample().to_report().contains("none over one frame"));
    }

    /// The figure is silent-by-construction, so it is reported even at zero:
    /// a reader has to be able to tell "nothing was rejected" from "this build
    /// does not report it", and a line that only appears when something is
    /// wrong cannot.
    #[test]
    fn the_stale_seed_count_is_reported_at_zero_as_well() {
        let mut diagnostics = sample();
        diagnostics.mesh = Some(MeshDiagnostics {
            sculptors: 2,
            stale_seeds_rejected: 0,
        });
        let text = diagnostics.to_report();
        assert!(text.contains("2 held, 0 stale seeds rejected"), "{text}");
    }

    #[test]
    fn a_report_taken_with_no_document_omits_the_mesh_line() {
        assert!(!sample().to_report().contains("mesh sculptors"));
    }

    fn memory() -> MemoryDiagnostics {
        MemoryDiagnostics {
            essential: 8 * 1024 * 1024,
            rebuildable: 2 * 1024 * 1024,
            undoable: 1024 * 1024,
            total: 11 * 1024 * 1024,
            surfaces: 2,
            surface_bytes: 3 * 1024 * 1024,
        }
    }

    /// The whole reason the three are carried instead of one figure: a reader
    /// deciding what to release needs to know which part is which, and a total
    /// says nothing about that.
    #[test]
    fn the_report_names_which_part_the_memory_is_in_and_not_only_how_much() {
        let mut diagnostics = sample();
        diagnostics.memory = Some(memory());
        let text = diagnostics.to_report();
        assert!(
            text.contains("8.0 MB essential, 2.0 MB rebuildable, 1.0 MB undoable of 11.0 MB total"),
            "{text}"
        );
    }

    /// The line that says the surfaces were asked. A surface tier of zero is
    /// the right answer on a document holding none, and it is also what a host
    /// that never filled the ledger would print — so the count is what tells
    /// the two apart.
    #[test]
    fn the_surfaces_folded_in_are_reported_even_when_there_are_none() {
        let mut diagnostics = sample();
        diagnostics.memory = Some(MemoryDiagnostics {
            surfaces: 0,
            surface_bytes: 0,
            ..memory()
        });
        let text = diagnostics.to_report();
        assert!(
            text.contains("memory surfaces: 0 held, 0.0 MB folded in"),
            "{text}"
        );
    }

    #[test]
    fn a_report_taken_with_no_document_omits_the_memory_lines() {
        let text = sample().to_report();
        assert!(!text.contains("memory"), "{text}");
    }

    fn worked() -> StrokeProfile {
        use crate::profile::Work;

        let mut profile = StrokeProfile::default();
        for _ in 0..12 {
            profile.record(
                "Padrão",
                Phase::EngineEdit,
                Duration::from_micros(520),
                Work::bricks(27),
            );
            profile.record(
                "Padrão",
                Phase::EngineMesh,
                Duration::from_micros(6_260),
                Work::meshed(27, 9_000),
            );
        }
        profile
    }

    /// The whole reason the section exists: a re-mesh reported as one total
    /// spans an engine call and this application's work around it, and neither
    /// party can act on a number they cannot attribute.
    #[test]
    fn the_stroke_section_says_which_side_of_the_boundary_the_time_went_to() {
        let mut diagnostics = sample();
        diagnostics.stroke = Some(StrokeDiagnostics::of(&worked()));
        let text = diagnostics.to_report();

        assert!(
            text.contains("stroke engine edit: 12 samples"),
            "the engine's edit is not in the report:\n{text}"
        );
        assert!(
            text.contains("(engine, stroke and brick refill)"),
            "the edit does not say whose time it was:\n{text}"
        );
        assert!(
            text.contains("(engine, clay_brick_cache_mesh)"),
            "the meshing call is not named:\n{text}"
        );
    }

    /// A phase that did not run and a phase that was free are different facts.
    #[test]
    fn a_phase_that_never_ran_is_reported_as_having_no_samples() {
        let mut diagnostics = sample();
        diagnostics.stroke = Some(StrokeDiagnostics::of(&worked()));
        let text = diagnostics.to_report();
        assert!(text.contains("stroke upload: no samples (ours)"), "{text}");
        assert!(
            !text.contains("stroke upload: 0 samples, 0.00 ms"),
            "an unmeasured phase was reported as a free one:\n{text}"
        );
    }

    #[test]
    fn a_session_that_sculpted_nothing_says_so_rather_than_listing_five_zeroes() {
        let mut diagnostics = sample();
        diagnostics.stroke = Some(StrokeDiagnostics::of(&StrokeProfile::default()));
        let text = diagnostics.to_report();
        assert!(text.contains("stroke: no samples this session"), "{text}");
    }

    #[test]
    fn a_figure_carries_the_work_it_was_measured_over() {
        let mut diagnostics = sample();
        diagnostics.stroke = Some(StrokeDiagnostics::of(&worked()));
        let text = diagnostics.to_report();
        assert!(text.contains("over 324 keys, 108000 triangles"), "{text}");
        assert!(text.contains("over 324 bricks"), "{text}");
    }

    #[test]
    fn the_refill_routing_evidence_reaches_the_report() {
        let mut diagnostics = sample();
        diagnostics.refill = Some(RefillDiagnostics {
            accelerated: "cuda".into(),
            cpu: Some(118.0),
            accelerated_cost: Some(413.0),
        });
        let text = diagnostics.to_report();
        assert!(
            text.contains("refill: cpu 118 ns/brick, cuda 413 ns/brick"),
            "{text}"
        );
    }

    /// The routing runs on a constant until both sides have been timed, and a
    /// backend reported as costing nothing is the one reading that would send
    /// somebody looking in the wrong place.
    #[test]
    fn an_unmeasured_backend_is_not_reported_as_free() {
        let mut diagnostics = sample();
        diagnostics.refill = Some(RefillDiagnostics {
            accelerated: "cuda".into(),
            cpu: Some(118.0),
            accelerated_cost: None,
        });
        let text = diagnostics.to_report();
        assert!(text.contains("cuda not measured"), "{text}");
        assert!(!text.contains("cuda 0 ns/brick"), "{text}");
    }

    #[test]
    fn a_report_taken_before_anything_was_sculpted_omits_both_sections() {
        let text = sample().to_report();
        assert!(!text.contains("stroke"), "{text}");
        assert!(!text.contains("refill"), "{text}");
    }

    #[test]
    fn every_line_is_a_key_and_a_value() {
        let mut diagnostics = sample();
        diagnostics.stroke = Some(StrokeDiagnostics::of(&worked()));
        diagnostics.refill = Some(RefillDiagnostics {
            accelerated: "cuda".into(),
            cpu: Some(118.0),
            accelerated_cost: None,
        });
        for line in diagnostics.to_report().lines() {
            assert!(line.contains(": "), "unparseable line: {line}");
        }
    }
}

#[cfg(test)]
mod agent_tests {
    use super::*;

    fn listening() -> Diagnostics {
        Diagnostics {
            agent: Some(AgentDiagnostics {
                listening: true,
                address: "http://127.0.0.1:7457/mcp".into(),
                connected: 1,
                commands: 40,
            }),
            ..Diagnostics::default()
        }
    }

    #[test]
    fn a_report_from_a_driven_session_says_so() {
        let report = listening().to_report();
        assert!(
            report.contains("agent: listening on http://127.0.0.1:7457/mcp"),
            "{report}"
        );
        assert!(report.contains("40 commands"), "{report}");
    }

    #[test]
    fn a_report_from_an_untouched_session_says_that_too() {
        let mut diagnostics = listening();
        diagnostics.agent = Some(AgentDiagnostics::default());
        assert!(diagnostics.to_report().contains("agent: not listening"));

        diagnostics.agent = None;
        assert!(diagnostics
            .to_report()
            .contains("agent: not built with a door"));
    }

    /// A report is pasted into issues and chat windows. Nothing that reaches
    /// one may be enough to drive the session it came from.
    #[test]
    fn the_secret_is_not_in_the_report() {
        let secret = "f2c1b0a9e8d7c6b5a4938271605f4e3d2c1b0a9e8d7c6b5a4938271605f4e3d2";
        let mut diagnostics = listening();
        diagnostics.agent.as_mut().unwrap().address = "http://127.0.0.1:7457/mcp".into();
        let report = diagnostics.to_report();
        assert!(!report.contains(secret), "{report}");
        // Nor any field shaped like one: the structure carries no place to put
        // it, which is the property that makes this hold for every session and
        // not only this one.
        assert!(!report.to_lowercase().contains("chave"), "{report}");
        assert!(!report.to_lowercase().contains("secret"), "{report}");
        assert!(!report.to_lowercase().contains("bearer"), "{report}");
    }
}
