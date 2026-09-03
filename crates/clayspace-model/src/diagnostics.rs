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
        out
    }
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

    #[test]
    fn every_line_is_a_key_and_a_value() {
        for line in sample().to_report().lines() {
            assert!(line.contains(": "), "unparseable line: {line}");
        }
    }
}
