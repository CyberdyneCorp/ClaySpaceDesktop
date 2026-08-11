//! Headless verification of the engine bridge.
//!
//! Milestone 1's definition of done: author a document, evaluate it on every
//! registered backend, and round-trip it through disk. Requires no display and
//! no GPU — on a CPU-only machine the backend loop runs once and still passes.

use claycore::{backends, Backend, Document, Item};

/// The engine holds GPU backends to 1e-4 relative against the CPU scalar
/// reference. Nothing here should need more room than the engine's own gate.
const PARITY_REL_TOLERANCE: f32 = 1e-4;

/// A sphere of radius 1 at the origin, whose field we can check by hand.
fn unit_sphere_document() -> Document {
    let mut doc = Document::new().expect("create document");
    let layer = doc.add_sdf_layer("Base").expect("add layer");
    let item = Item::sphere(1.0).expect("build sphere");
    doc.add_item(layer, &item).expect("place sphere");
    doc
}

/// Points chosen to sit inside, on, and outside the surface.
fn probe_points() -> Vec<[f32; 3]> {
    vec![
        [0.0, 0.0, 0.0],
        [0.5, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, 1.5, 0.0],
        [0.0, 0.0, -3.0],
        [0.7, 0.7, 0.7],
    ]
}

#[test]
fn cpu_backend_evaluates_a_known_field() {
    let doc = unit_sphere_document();
    let points = probe_points();
    let distances = doc.eval_points(None, &points).expect("evaluate on cpu");

    assert_eq!(distances.len(), points.len());

    // For a unit sphere at the origin the field is |p| - 1 exactly, which is
    // what makes this a check of the bridge rather than of the engine.
    for (point, actual) in points.iter().zip(&distances) {
        let expected = (point[0].powi(2) + point[1].powi(2) + point[2].powi(2)).sqrt() - 1.0;
        assert!(
            (actual - expected).abs() < 1e-5,
            "at {point:?}: expected {expected}, got {actual}"
        );
    }
}

#[test]
fn every_registered_backend_agrees_with_cpu() {
    let registered = backends().expect("discover backends");
    assert!(
        registered.contains(&Backend::Cpu),
        "the CPU backend is compiled in unconditionally but was not registered: {registered:?}"
    );

    let doc = unit_sphere_document();
    let points = probe_points();
    let reference = doc.eval_points(Some(&Backend::Cpu), &points).expect("cpu reference");

    for backend in &registered {
        let actual = match doc.eval_points(Some(backend), &points) {
            Ok(values) => values,
            // A backend may decline an operation it does not implement. That
            // is a routing outcome, not a failure.
            Err(e) if e.is_unsupported() => continue,
            Err(e) => panic!("{backend} failed to evaluate: {e}"),
        };

        for (i, (got, want)) in actual.iter().zip(&reference).enumerate() {
            let tolerance = PARITY_REL_TOLERANCE * want.abs().max(1.0);
            assert!(
                (got - want).abs() <= tolerance,
                "{backend} disagrees with cpu at point {i} ({:?}): {got} vs {want}",
                points[i]
            );
        }
    }
}

#[test]
fn a_document_round_trips_through_disk() {
    let dir = std::env::temp_dir().join(format!("claycore-bridge-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("round-trip.clayspace");

    let points = probe_points();
    let before = {
        let doc = unit_sphere_document();
        doc.save(&path).expect("save document");
        doc.eval_points(None, &points).expect("evaluate before save")
    };

    let reopened = Document::open(&path).expect("reopen document");
    let after = reopened.eval_points(None, &points).expect("evaluate after load");

    assert_eq!(before, after, "the reopened document evaluates differently");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_absent_document_fails_without_panicking() {
    let err = Document::open("/nonexistent/path/does-not-exist.clayspace")
        .expect_err("opening a missing file must fail");

    // The detail message is the engine's own, captured at the failure site.
    assert!(
        !format!("{err}").is_empty(),
        "the error must describe itself: {err:?}"
    );
}

#[test]
fn an_empty_batch_is_not_an_error() {
    let doc = unit_sphere_document();
    let distances = doc.eval_points(None, &[]).expect("empty batch");
    assert!(distances.is_empty());
}
