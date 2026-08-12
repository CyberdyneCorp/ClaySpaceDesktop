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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
            platform: "macos aarch64".into(),
            backends: vec!["cpu".into(), "metal".into()],
            active_backend: "metal".into(),
            selection: "automática".into(),
            fallbacks: Vec::new(),
            renderer: Some("Apple M3 Max (Metal)".into()),
            stalls: Vec::new(),
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

    #[test]
    fn every_line_is_a_key_and_a_value() {
        for line in sample().to_report().lines() {
            assert!(line.contains(": "), "unparseable line: {line}");
        }
    }
}
