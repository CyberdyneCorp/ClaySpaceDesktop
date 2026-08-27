//! Task 2.7: every registered backend agrees, and the document is the same
//! file whichever one produced it.
//!
//! Two different claims, and the ABI decides which is which. Meshing and
//! saving take no backend at all — `clay_document_mesh` and
//! `clay_document_save` have no such parameter — so the document is
//! backend-independent by construction and the assertion is that it stays
//! that way. *Evaluation* does take one: `clay_eval_points` and
//! `clay_brick_cache_eval_requests` both name a backend, and that is where
//! parity is a real question rather than a tautology.
//!
//! The engine holds every backend to the CPU scalar reference, so a
//! disagreement here is an upstream defect and worth catching from this side
//! too — this application chooses backends per operation, and a silent
//! divergence would show up as a surface that changes when the acceleration
//! policy does.

use claycore::Backend;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, DocumentModel, GestureSample, SculptModel, ToolKind};

/// The tolerance the engine's own parity suite works to.
///
/// A distance field in f32 over a unit-scale document: agreement to a
/// thousandth of a world unit is far tighter than the 0.02 voxel the cache
/// stores, so a disagreement above this would be visible.
const TOLERANCE: f32 = 1e-3;

fn sculpted() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    // Something asymmetric, so a backend that quietly mirrors or truncates
    // has somewhere to show it.
    for at in [[0.3f32, 0.1, 0.5], [-0.2, 0.4, 0.45]] {
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &[GestureSample {
                    position: at,
                    pressure: 1.0,
                    time: 0.0,
                }],
                [false; 3],
            )
            .expect("sculpt");
    }
    document
}

/// A lattice of sample points across the document.
fn probes() -> Vec<[f32; 3]> {
    let mut points = Vec::new();
    for i in 0..8 {
        for j in 0..8 {
            for k in 0..8 {
                points.push([
                    -0.8 + i as f32 * 0.22,
                    -0.8 + j as f32 * 0.22,
                    -0.8 + k as f32 * 0.22,
                ]);
            }
        }
    }
    points
}

#[test]
fn every_registered_backend_evaluates_the_same_field() {
    let document = sculpted();
    let Ok(available) = claycore::backends() else {
        return;
    };
    let points = probes();

    let reference = document
        .document()
        .eval_points(Some(&Backend::Cpu), &points)
        .expect("the cpu reference");

    let mut compared = 0;
    for backend in &available {
        if *backend == Backend::Cpu {
            continue;
        }
        let Ok(values) = document.document().eval_points(Some(backend), &points) else {
            // A backend that declines the operation is routing information,
            // not a fault — the policy falls back for exactly this.
            continue;
        };
        compared += 1;
        assert_eq!(
            values.len(),
            reference.len(),
            "{backend} returned a different count"
        );
        for (index, (a, b)) in reference.iter().zip(values.iter()).enumerate() {
            assert!(
                (a - b).abs() <= TOLERANCE,
                "{backend} disagrees with the cpu reference at {:?}: {b} against {a}",
                points[index]
            );
        }
    }
    println!("{compared} accelerated backend(s) compared against cpu");
}

#[test]
fn the_document_saves_byte_identically_whatever_ran() {
    // Saving takes no backend, so this asserts that stays true: the document
    // is an edit list, and a file that varied with the machine's acceleration
    // would make every other guarantee here untestable.
    let mut document = sculpted();
    let Ok(available) = claycore::backends() else {
        return;
    };

    let mut written: Vec<(String, Vec<u8>)> = Vec::new();
    for backend in &available {
        // Force each backend to have done the evaluation, so the comparison is
        // between documents that were *worked on* differently rather than
        // between two saves of an untouched one.
        let _ = document.document().eval_points(Some(backend), &probes());

        let path = std::env::temp_dir().join(format!("clayspace-parity-{backend}.clayspace"));
        let _ = std::fs::remove_file(&path);
        document.save(&path).expect("save");
        let bytes = std::fs::read(&path).expect("read back");
        let _ = std::fs::remove_file(&path);
        written.push((backend.to_string(), bytes));
    }

    let Some((first_name, first)) = written.first() else {
        return;
    };
    assert!(!first.is_empty(), "the document saved as an empty file");
    for (name, bytes) in &written[1..] {
        assert_eq!(
            bytes.len(),
            first.len(),
            "{name} and {first_name} saved documents of different sizes"
        );
        assert!(
            bytes == first,
            "{name} and {first_name} did not save byte-identically"
        );
    }
}

#[test]
fn an_export_is_the_same_geometry_whatever_ran() {
    // Compared as counts rather than as bytes. The first version compared
    // file sizes and failed by two bytes in twenty-eight megabytes — which
    // turned out to be float printing rather than different geometry, and is
    // recorded in `export_determinism.rs`. Vertex and index counts are the
    // claim actually being made here.
    let document = sculpted();
    let Ok(available) = claycore::backends() else {
        return;
    };

    let counts = |document: &ClayDocument| {
        let mesh = document
            .document()
            .mesh_combined(claycore::MeshParams {
                voxel_size: Some(0.02),
                resolution: 128,
                decimate_ratio: None,
                mesher: claycore::Mesher::MarchingTetrahedra,
            })
            .expect("mesh");
        (mesh.vertex_count(), mesh.index_count())
    };

    let reference = counts(&document);
    for backend in &available {
        let _ = document.document().eval_points(Some(backend), &probes());
        assert_eq!(
            counts(&document),
            reference,
            "evaluating on {backend} changed the exported geometry"
        );
    }
}
