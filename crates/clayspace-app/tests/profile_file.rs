//! That the exported profile says everything, and says nothing it should not.
//!
//! The file is written to be attached to a public issue on somebody else's
//! repository. Two properties matter more than any field in it: that a figure
//! which was never measured is never written as a zero, and that nothing the
//! sculptor named comes out.
//!
//! ```sh
//! cargo test -p clayspace-app --test profile_file
//! ```

use std::time::Duration;

use clayspace_app::profile_file::{self, Ask, DocumentShape, LayerShape};
use clayspace_model::{
    AoDiagnostics, Diagnostics, Fallback, MemoryDiagnostics, Phase, RefillDiagnostics,
    RenderDiagnostics, StrokeProfile, Work,
};

fn diagnostics() -> Diagnostics {
    Diagnostics {
        app_version: "ClaySpaceDesktop 0.1.0".into(),
        engine_version: "claycore 0.78.0".into(),
        engine_revision: "v0.78.0-0-g512c8c5d".into(),
        document_format: "1.16".into(),
        platform: "linux x86_64".into(),
        backends: vec!["cpu".into(), "cuda".into()],
        active_backend: "cuda".into(),
        selection: "automática".into(),
        fallbacks: vec![Fallback {
            operation: "raycast".into(),
            declined_by: "opencl".into(),
        }],
        renderer: Some("NVIDIA GeForce RTX 5060 (Vulkan)".into()),
        stalls: vec!["consolidar 6400 ms".into()],
        render: Some(RenderDiagnostics {
            viewport: [1280, 800],
            samples: 4,
            ao: Some(AoDiagnostics {
                width: 640,
                height: 400,
                samples: 8,
                temporal: true,
            }),
            gpu_passes: vec![("scene".into(), 2.4), ("ao".into(), 0.8)],
            gpu_timing: true,
            draw_calls: 12,
            culled: 3,
            triangles: 283_612,
            lines: 480,
            uploaded_bytes: 1_048_576,
        }),
        mesh: None,
        hierarchies: None,
        memory: Some(MemoryDiagnostics {
            essential: 8 * 1024 * 1024,
            rebuildable: 2 * 1024 * 1024,
            undoable: 1024 * 1024,
            total: 11 * 1024 * 1024,
            surfaces: 2,
            surface_bytes: 3 * 1024 * 1024,
        }),
        agent: None,
        stroke: None,
        refill: Some(RefillDiagnostics {
            accelerated: "cuda".into(),
            cpu: Some(118.0),
            accelerated_cost: Some(413.0),
        }),
    }
}

fn worked() -> StrokeProfile {
    let mut profile = StrokeProfile::default();
    for step in 0..24 {
        profile.record(
            "Padrão",
            Phase::EngineEdit,
            Duration::from_micros(500 + step * 10),
            Work::bricks(27),
        );
        profile.record(
            "Padrão",
            Phase::EngineMesh,
            Duration::from_micros(6_000 + step * 40),
            Work::meshed(27, 9_000),
        );
    }
    profile
}

/// A layer the sculptor has named. The name must not survive the export, and
/// this shape is the reason it cannot: there is nowhere in it to put one.
fn document() -> DocumentShape {
    DocumentShape {
        layers: vec![LayerShape {
            index: 0,
            representation: "SDF".into(),
            visible: true,
            sculpt_layers: 0,
            multires_levels: 0,
            items: Some(96),
            consolidated: Some(false),
            cell_size: None,
            occupied: None,
        }],
        triangles: 283_612,
        vertices: 141_806,
        objects: 1,
        detail: "full".into(),
    }
}

fn rendered() -> String {
    profile_file::render(&diagnostics(), &worked(), &document())
}

#[test]
fn the_file_is_well_formed() {
    let text = rendered();
    assert_eq!(
        text.matches('{').count(),
        text.matches('}').count(),
        "unbalanced objects:\n{text}"
    );
    assert_eq!(
        text.matches('[').count(),
        text.matches(']').count(),
        "unbalanced arrays:\n{text}"
    );
    assert!(text.trim_start().starts_with('{'));
    assert!(text.trim_end().ends_with('}'));
}

/// Everything this project has ever had to reconstruct by hand in an upstream
/// issue. A follow-up question is a round trip, and a round trip is where a
/// performance report dies.
#[test]
fn the_file_answers_what_would_otherwise_be_asked() {
    let text = rendered();
    for expected in [
        "\"engine\": \"claycore 0.78.0\"",
        "\"engine_revision\": \"v0.78.0-0-g512c8c5d\"",
        "\"platform\": \"linux x86_64\"",
        "\"active_backend\": \"cuda\"",
        "\"renderer\": \"NVIDIA GeForce RTX 5060 (Vulkan)\"",
        "\"declined_by\": \"opencl\"",
        "\"consolidar 6400 ms\"",
        "\"cpu_ns_per_brick\": 118.000",
        "\"accelerated_ns_per_brick\": 413.000",
        "\"surfaces_asked\": 2",
    ] {
        assert!(text.contains(expected), "the file lost {expected}:\n{text}");
    }
}

/// The point of the whole file: a reader can tell whose milliseconds these
/// were without asking.
#[test]
fn every_phase_says_which_side_of_the_boundary_it_is_on() {
    let text = rendered();
    assert!(text.contains("\"phase\": \"engine edit\""), "{text}");
    assert!(
        text.contains("\"entry_point\": \"clay_brick_cache_mesh\""),
        "{text}"
    );
    assert_eq!(
        text.matches("\"side\": \"engine\"").count(),
        // Two engine phases, in the aggregate and once for the one tool.
        4,
        "{text}"
    );
    assert!(text.contains("\"side\": \"ours\""), "{text}");
}

/// A mean is the statistic that hides the tail a sculptor is complaining
/// about, so the file carries a distribution and says which population it
/// describes.
#[test]
fn a_distribution_is_exported_and_not_an_average() {
    let text = rendered();
    for key in [
        "\"seen\":",
        "\"retained\":",
        "\"median_ms\":",
        "\"p95_ms\":",
        "\"worst_ms\":",
    ] {
        assert!(text.contains(key), "the file lost {key}:\n{text}");
    }
    assert!(!text.contains("mean"), "{text}");
    assert!(text.contains("\"seen\": 24"), "{text}");
}

/// A phase that never ran, a backend never timed and an adapter with no
/// timestamps are all *unmeasured*. A zero would read as free, which is the
/// reading that sends somebody looking in the wrong place.
#[test]
fn nothing_unmeasured_is_written_as_a_zero() {
    let text = rendered();
    // Upload never ran in this profile.
    assert!(text.contains("\"median_ms\": null"), "{text}");
    assert!(
        !text.contains("\"median_ms\": 0.000"),
        "an unmeasured phase was written as a free one:\n{text}"
    );

    let mut without = diagnostics();
    without.refill = Some(RefillDiagnostics {
        accelerated: "cuda".into(),
        cpu: Some(118.0),
        accelerated_cost: None,
    });
    if let Some(render) = without.render.as_mut() {
        render.gpu_timing = false;
        render.gpu_passes.clear();
    }
    let text = profile_file::render(&without, &worked(), &document());
    assert!(
        text.contains("\"accelerated_ns_per_brick\": null"),
        "{text}"
    );
    assert!(text.contains("\"gpu_passes\": null"), "{text}");
    assert!(
        !text.contains("\"accelerated_ns_per_brick\": 0.000"),
        "{text}"
    );
}

/// The file is written to be attached to a public issue. It carries no
/// document path and nothing the sculptor named — enforced by what is
/// collected, so a field added later cannot quietly weaken it.
#[test]
fn a_named_subtool_does_not_reach_the_file() {
    let text = rendered();
    for private in ["Cabeça do dragão", "/home/", "clayspace/", ".clayspace"] {
        assert!(
            !text.contains(private),
            "the file carries {private}, which the sculptor never agreed to publish:\n{text}"
        );
    }
    // And a layer is still identifiable, by the two things it may be.
    assert!(text.contains("\"representation\": \"SDF\""), "{text}");
    assert!(text.contains("\"index\": 0"), "{text}");
}

/// An unoptimised build runs this work about two and a half times slower, so a
/// duration from one is a fact about the build profile and not about the
/// engine. The file says so before any number.
#[test]
fn the_file_declares_whether_its_timings_mean_anything() {
    let text = rendered();
    let debug = cfg!(debug_assertions);
    assert_eq!(
        profile_file::build_profile(),
        if debug { "debug" } else { "release" }
    );
    assert_eq!(profile_file::timings_comparable(), !debug);
    assert!(
        text.contains(&format!("\"build\": \"{}\"", profile_file::build_profile())),
        "{text}"
    );
    assert!(
        text.contains(&format!("\"timings_comparable\": {}", !debug)),
        "{text}"
    );
}

/// A session in which nothing was sculpted is a fact about the session, not a
/// reason to refuse the export.
#[test]
fn an_unworked_session_still_exports_every_phase() {
    let text = profile_file::render(&diagnostics(), &StrokeProfile::default(), &document());
    assert!(text.contains("\"phase\": \"engine edit\""), "{text}");
    assert!(text.contains("\"seen\": 0"), "{text}");
    assert!(
        !text.contains("\"median_ms\": 0.000"),
        "an unworked session read as one where everything was instant:\n{text}"
    );
}

/// A document with nothing open still produces a file: the identifying half of
/// a profile is what diagnoses a session that cannot sculpt at all.
#[test]
fn a_report_with_no_document_still_carries_the_conditions() {
    let mut bare = diagnostics();
    bare.memory = None;
    bare.render = None;
    let text = profile_file::render(&bare, &StrokeProfile::default(), &DocumentShape::default());
    assert!(text.contains("\"memory\": null"), "{text}");
    assert!(text.contains("\"rendering\": null"), "{text}");
    assert!(text.contains("\"engine_revision\""), "{text}");
}

/// A path that cannot be written must leave nothing behind. A truncated JSON
/// document fails in the reader rather than where it was made, by which time
/// whoever could have explained it has moved on.
#[test]
fn a_failed_export_leaves_nothing_behind() {
    let nowhere = std::path::Path::new("target/no-such-directory-here/perfil.json");
    assert!(profile_file::write(nowhere, &rendered()).is_err());
    assert!(!nowhere.exists());
}

#[test]
fn a_written_profile_is_the_document_that_was_rendered() {
    let path = std::env::temp_dir().join("clayspace-profile-test.json");
    let text = rendered();
    profile_file::write(&path, &text).expect("a writable path");
    assert_eq!(std::fs::read_to_string(&path).expect("the file"), text);
    std::fs::remove_file(&path).ok();
}

// -- the two dialogs ---------------------------------------------------------
//
// A native dialog cannot be driven headlessly, so what is held here is what a
// dialog is *for*: the decision it carries and the parameters it is opened
// with. The dialog itself is then a call with no judgement left in it.

/// The failure this guards is drift. The warning and the file's own marker are
/// two statements of one fact, and a build that stamped
/// `"timings_comparable": false` while asking nobody would be a build that
/// published a debug figure with a clean conscience.
#[test]
fn the_question_asked_and_the_claim_written_cannot_disagree() {
    match profile_file::ask_before_writing() {
        Ask::Nothing => assert!(
            profile_file::timings_comparable(),
            "the export wrote without asking from a build whose timings it marks incomparable"
        ),
        Ask::WarnTimingsAreNotComparable => assert!(
            !profile_file::timings_comparable(),
            "the export warned about timings the file itself says are comparable"
        ),
    }
}

/// The one branch a person actually meets, tied to the build it belongs to.
#[test]
fn a_debug_build_asks_first_and_a_release_build_does_not() {
    let expected = if cfg!(debug_assertions) {
        Ask::WarnTimingsAreNotComparable
    } else {
        Ask::Nothing
    };
    assert_eq!(profile_file::ask_before_writing(), expected);
    // And the build the file names is the same one the decision was taken on.
    assert_eq!(
        profile_file::build_profile(),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
}

/// A save dialog whose default name does not match the filter it offers is a
/// dialog that argues with itself: the person accepts the name, the filter
/// rejects it, and the extension is typed by hand.
#[test]
fn the_save_dialog_offers_what_its_default_name_already_is() {
    let extension = profile_file::FILE_NAME
        .rsplit_once('.')
        .expect("the default name carries an extension")
        .1;
    assert!(
        profile_file::EXTENSIONS.contains(&extension),
        "the dialog defaults to {} and offers {:?}",
        profile_file::FILE_NAME,
        profile_file::EXTENSIONS
    );
}

/// What the writer produces is what the dialog said it would.
#[test]
fn the_offered_extension_is_the_format_that_is_written() {
    assert_eq!(profile_file::EXTENSIONS, ["json"]);
    let text = rendered();
    assert!(text.trim_start().starts_with('{'), "{text}");
}
