//! What the final re-mesh spends, and on which of its three routes.
//!
//! `re-malha final` is one line in the stall ledger and three different pieces
//! of work behind it: a whole-field `clay_document_mesh`, a per-key rebuild
//! when the coarse level is the one requested, and an early return when the
//! field is empty. Averaged together they are unreadable — the ledger recorded
//! 59.4 ms over a session while ClayCore measures a whole-field mesh at 3.1 ms
//! on a clean sphere, and there was no way to ask which of those a given
//! occurrence was, or how the time inside it divided.
//!
//! So `settle` now records the route and the split. These pin that it does,
//! because a cost nobody asserts drifts — this repository has corrected three
//! numbers this week that nothing was watching.

mod support;

use clayspace_app::{SettleRoute, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

fn dab(document: &mut ClayDocument) {
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.25,
                intensity: 0.9,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("a dab");
}

/// A settle on an ordinary form goes through the document mesher, and says so.
#[test]
fn a_settle_reports_the_route_it_took_and_splits_its_time() {
    let Some(mut harness) = Harness::new() else {
        println!("no GPU harness; skipping");
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("a first surface");

    assert!(
        geometry.last_settle().is_none(),
        "nothing has settled yet, so there is nothing to report"
    );

    dab(&mut document);
    geometry.sync(&harness.gpu, &mut document).expect("sync");
    geometry
        .settle(&harness.gpu, &mut document)
        .expect("settle");

    let cost = geometry
        .last_settle()
        .expect("a settle that ran must say what it cost");

    println!(
        "  route {:?}: total {:.2} ms = engine {:.2} + read {:.2} + upload {:.2}, {} tris",
        cost.route,
        cost.total_time.as_secs_f64() * 1000.0,
        cost.engine_mesh_time.as_secs_f64() * 1000.0,
        cost.read_time.as_secs_f64() * 1000.0,
        cost.upload_time.as_secs_f64() * 1000.0,
        cost.triangles,
    );

    assert_eq!(
        cost.route,
        SettleRoute::Document,
        "a form with a surface, at full detail, settles through the document \
         mesher — the other two routes are the coarse rebuild and the empty \
         field"
    );
    assert!(
        cost.triangles > 0,
        "the settle produced {} triangles from a form that has a surface",
        cost.triangles
    );
    // The point of the split: the parts must be inside the whole. If they are
    // not, they are timing different things and the remainder that names OUR
    // share would be meaningless.
    let parts = cost.engine_mesh_time + cost.read_time + cost.upload_time;
    assert!(
        parts <= cost.total_time,
        "the parts sum to {:?} inside a total of {:?}, so they are not spans \
         of the same work",
        parts,
        cost.total_time
    );
    assert!(
        cost.engine_mesh_time > std::time::Duration::ZERO,
        "the engine's own mesh call took no measurable time, which means it \
         is not being timed"
    );
}

/// An empty field settles by the third route, and does not pretend to mesh.
///
/// Without this the route field could be hardcoded to `Document` and the test
/// above would still pass.
#[test]
fn an_empty_field_settles_by_its_own_route() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    // No starting form: nothing to mesh.
    let Ok(mut document) = ClayDocument::new(policy) else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .settle(&harness.gpu, &mut document)
        .expect("settle");

    let cost = geometry.last_settle().expect("a settle reports");
    assert_eq!(
        cost.route,
        SettleRoute::Empty,
        "a document with no field surface took the meshing route"
    );
    assert_eq!(cost.triangles, 0);
    assert_eq!(
        cost.engine_mesh_time,
        std::time::Duration::ZERO,
        "nothing was meshed, so no engine time can have been spent"
    );
}
