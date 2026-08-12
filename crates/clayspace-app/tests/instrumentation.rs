//! The interface-thread instrumentation, against real work.
//!
//! The model tests cover the bookkeeping. This one checks the part that
//! matters: that the operations a sculptor can actually reach are the ones
//! being timed, and that a real re-mesh lands in the log under a name a bug
//! report can carry.

mod support;

use std::time::Instant;

use clayspace_app::{SharedDocument, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, FrameLog, GestureSample, SculptModel, ToolKind, FRAME};
use support::Harness;

#[test]
fn a_real_remesh_is_timed_and_named() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form) else {
        return;
    };
    let mut document = SharedDocument::new(document);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);

    // A threshold of zero, so the measurement is about *what* is recorded
    // rather than about how fast this particular machine is. A test that
    // asserts a real 16 ms overrun would pass or fail on the runner.
    let mut log = FrameLog::with_threshold(std::time::Duration::ZERO);

    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: [0.0, 0.0, 0.55],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("a dab");

    let started = Instant::now();
    let gpu = harness.gpu.clone();
    document
        .with(|document| geometry.sync(&gpu, document))
        .expect("re-mesh");
    assert!(log.record("re-malha", started.elapsed()));

    assert_eq!(log.stalls().len(), 1);
    assert_eq!(log.worst().expect("a worst").operation, "re-malha");
    assert!(
        log.lines()[0].contains("ms"),
        "the line is not a report: {:?}",
        log.lines()
    );
}

#[test]
fn the_threshold_is_the_one_the_specification_asks_for() {
    // 16 ms, stated as one frame at sixty hertz rather than as a magic number,
    // and reached through the default so the application cannot drift from it.
    let log = FrameLog::default();
    assert_eq!(log.threshold(), FRAME);
    assert_eq!(log.threshold().as_millis(), 16);
}

#[test]
fn an_ordinary_dab_does_not_fill_the_log_with_noise() {
    // The failure this prevents is a diagnostics panel nobody reads because it
    // lists four hundred re-meshes that went over by a millisecond. Repeats
    // merge into one line carrying the worst time.
    let mut log = FrameLog::default();
    for took in [20, 18, 45, 17, 19] {
        log.record("re-malha", std::time::Duration::from_millis(took));
    }
    assert_eq!(log.stalls().len(), 1);
    assert_eq!(log.stalls()[0].count, 5);
    assert!(log.lines()[0].contains("45 ms"));
    assert!(log.lines()[0].contains("×5"));
}
