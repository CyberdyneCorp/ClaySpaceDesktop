//! That a stroke's phases are kept, and kept honestly.
//!
//! `dab_profile.rs` beside this file measures the same five terms and prints
//! them. This one asserts that the application *keeps* them — the figures it
//! has always computed used to be dropped on the floor by the composition
//! root, and a report that says `re-malha 42 ms` cannot tell the engine's
//! authors whether the 42 ms was theirs.
//!
//! ```sh
//! cargo test -p clayspace-app --test stroke_profile --release
//! ```

mod support;

use clayspace_app::{SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, GestureSample, Phase, Representation, SculptModel, StrokeDiagnostics, ToolKind,
};
use support::Harness;

/// The starting form, behind the handle every stroke passes through.
fn document() -> Option<SharedDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    Some(SharedDocument::new(document))
}

fn dab(at: f32) -> [GestureSample; 1] {
    [GestureSample {
        position: [(at - 0.5) * 0.8, 0.1, 1.0],
        pressure: 1.0,
        time: at,
    }]
}

#[test]
fn every_stroke_leaves_what_the_engine_spent_on_it() {
    let Some(mut document) = document() else {
        return;
    };
    for step in 0..3 {
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &dab(step as f32 / 3.0),
                [false; 3],
            )
            .expect("the starting form takes a standard dab");
    }

    let profile = document.profile();
    let edit = profile.across_tools().phase(Phase::EngineEdit);
    assert_eq!(edit.seen(), 3, "the engine's own half was not measured");
    assert!(edit.median().is_some());
    assert!(
        edit.work().bricks > 0,
        "a duration with no workload beside it is not comparable with any other"
    );
}

/// An error is not a measurement of the engine doing the work, and folding one
/// in would drag the phase's median towards whatever refusing costs.
#[test]
fn a_refused_stroke_leaves_the_profile_untouched() {
    let Some(mut document) = document() else {
        return;
    };
    // Whichever tool the table says a field has no verb for — asked of the
    // table rather than named here, so this keeps testing the refusal and not
    // a tool that has since gained a field verb.
    let Some(elsewhere) = ToolKind::ALL
        .into_iter()
        .find(|tool| !tool.exists_on(Representation::Sdf))
    else {
        return;
    };

    assert!(document
        .apply_stroke(elsewhere, BrushSettings::default(), &dab(0.5), [false; 3])
        .is_err());
    assert!(
        document.profile().is_empty(),
        "a refusal was recorded as though the engine had done the work"
    );
}

#[test]
fn a_dab_populates_every_phase_of_a_stroke() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = document() else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    if document
        .with(|d| geometry.rebuild(&harness.gpu, d))
        .is_err()
    {
        return;
    }

    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &dab(0.5),
            [false; 3],
        )
        .expect("the starting form takes a standard dab");
    let cost = document
        .with(|d| geometry.sync(&harness.gpu, d))
        .expect("the dab re-meshes")
        .expect("a dab dirties bricks, so there is a cost");
    document.record_remesh(ToolKind::Padrao.label(), cost);

    let whole = document.profile().across_tools();
    for phase in Phase::ALL {
        assert!(
            !whole.phase(phase).is_empty(),
            "{} was not kept",
            phase.label()
        );
    }
    assert!(
        whole.phase(Phase::EngineMesh).work().keys > 0,
        "the meshing figures carry no workload"
    );

    // And a second sync with nothing dirty is not a measurement of anything.
    // A re-mesh that re-meshed nothing would otherwise pull every phase's
    // median towards zero, once per frame, for as long as the pointer rests.
    let before = whole.phase(Phase::EngineMesh).seen();
    let idle = document.with(|d| geometry.sync(&harness.gpu, d));
    assert!(
        matches!(idle, Ok(None)),
        "a sync with nothing dirty reported a cost"
    );
    assert_eq!(
        document
            .profile()
            .across_tools()
            .phase(Phase::EngineMesh)
            .seen(),
        before
    );
}

/// The two engine phases are the point of the split: they are the figures the
/// engine's authors can act on, and nothing of ours runs inside either.
#[test]
fn the_engine_phases_are_named_as_the_engines() {
    let Some(mut document) = document() else {
        return;
    };
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &dab(0.5),
            [false; 3],
        )
        .expect("the starting form takes a standard dab");

    let report = StrokeDiagnostics::of(&document.profile());
    let engine: Vec<&str> = report
        .phases
        .iter()
        .filter(|phase| phase.engine)
        .map(|phase| phase.phase.as_str())
        .collect();
    assert_eq!(engine, ["engine edit", "engine mesh"]);
}

/// A phase that did not run and a phase that was free are different facts, and
/// a session that sculpted nothing must not read as one where everything was
/// instant.
#[test]
fn an_unworked_session_reports_no_samples_rather_than_zeroes() {
    let Some(document) = document() else {
        return;
    };
    let report = StrokeDiagnostics::of(&document.profile());
    assert!(report.is_empty());
    assert!(report
        .phases
        .iter()
        .all(|phase| phase.median.is_none() && phase.worst.is_none()));
    assert_eq!(
        report.phases.len(),
        Phase::ALL.len(),
        "a phase with nothing in it is still reported, or a reader cannot tell \
         it from one this build does not measure"
    );
}

/// Each tool keeps its own figures: "the smooth brush is the slow one" is a
/// sentence an engine team can act on and an aggregate cannot produce it.
#[test]
fn two_tools_are_measured_apart() {
    let Some(mut document) = document() else {
        return;
    };
    let brush = BrushSettings::default();
    for tool in [ToolKind::Padrao, ToolKind::Suavizar] {
        if document
            .apply_stroke(tool, brush, &dab(0.5), [false; 3])
            .is_err()
        {
            return;
        }
    }

    let profile = document.profile();
    assert_eq!(profile.tools().count(), 2);
    assert_eq!(
        profile.across_tools().phase(Phase::EngineEdit).seen(),
        2,
        "the aggregate lost one of the tools"
    );
}
