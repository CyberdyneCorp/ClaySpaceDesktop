//! A dab costs what the edit costs, not what the model costs.
//!
//! The specification's locality requirement, as a test rather than as a
//! reported figure. `src/bin/bench/groups/locality.rs` reports it, and
//! reported `locality.key_ratio` 0.00 against a budget of 2 while the
//! ten-times scene took 1367 ms for a dab the reference scene did in 4 — a
//! gate passing *because* its probe had stopped touching the surface.
//!
//! What it was hiding: an empty key list means "every surface brick" at the C
//! boundary, which is right for an export and wrong for a re-mesh. Filtering
//! the dirty set to the keys that can hold a triangle made "nothing to mesh" a
//! reachable state for the first time — an edit landing wholly inside the
//! material dirties bricks and none of them is a surface brick — and asking
//! for nothing was answered with everything: 2.9 M triangles and 1.31 s to
//! establish that a dab under the surface changed nothing.

mod support;

use std::time::Instant;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_model::{GestureSample, SculptModel, ToolKind};
use support::Harness;

/// How much dearer a dab may be on a model ten times the area.
///
/// The specification asks for the work to follow the edit;
/// `src/bin/bench/groups/locality.rs` budgets the *key* ratio at 2, and this is
/// the same claim in milliseconds with room for the larger scene's cache being
/// colder. The failure this exists for was 714x.
const RATIO: f64 = 10.0;

#[test]
fn a_dab_costs_the_same_on_a_model_ten_times_the_area() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };

    let mut measured = Vec::new();
    for scene in [Scene::Reference, Scene::TenTimesLarger] {
        let Ok(mut document) = scene.build(policy.clone()) else {
            return;
        };
        let mut geometry = SurfaceGeometry::new(&harness.gpu);
        geometry
            .rebuild(&harness.gpu, &mut document)
            .expect("rebuild");
        let bricks = document
            .cache()
            .surface_bricks()
            .map(|keys| keys.len())
            .unwrap_or(0);

        let sample = scene.stroke(3)[1];
        let Some(position) = Scene::probe_point(&document, sample.position) else {
            panic!("{scene:?}: no surface brick among {bricks} to dab onto");
        };

        // The same absolute brush on both, which is what makes this about the
        // model rather than about the tool — see `Scene::probe_brush`.
        let started = Instant::now();
        document
            .apply_stroke(
                ToolKind::Padrao,
                Scene::probe_brush(),
                &[GestureSample { position, ..sample }],
                [false; 3],
            )
            .expect("a dab");
        let cost = geometry.sync(&harness.gpu, &mut document).expect("sync");
        let took = started.elapsed().as_secs_f64() * 1000.0;

        let keys = cost.map(|c| c.keys).unwrap_or(0);
        println!("{scene:?}: {bricks} surface bricks, dab {took:.2} ms over {keys} keys");
        assert!(
            keys > 0,
            "{scene:?}: a dab picked onto the surface re-meshed nothing, so \
             this measures a miss rather than an edit"
        );
        measured.push((took, keys));
    }

    let [(small_ms, small_keys), (large_ms, large_keys)] = measured[..] else {
        return;
    };
    println!(
        "ten times the area: {:.1}x the milliseconds, {:.1}x the keys",
        large_ms / small_ms.max(0.001),
        large_keys as f64 / small_keys.max(1) as f64
    );

    assert!(
        large_ms < RATIO * small_ms,
        "a dab took {large_ms:.0} ms on the larger scene against {small_ms:.0} ms \
         on the reference one. The work is following the model rather than the \
         edit — which is what asking the engine to mesh an empty key list did, \
         since that is how it spells \"everything\"."
    );
}

#[test]
fn a_sync_with_nothing_to_mesh_meshes_nothing() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = Scene::Reference.build(policy) else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("rebuild");
    let before = geometry.triangle_count();

    // Deep inside the form, where a dab moves the field and reaches no
    // surface: bricks are dirtied and not one of them can hold a triangle.
    document
        .apply_stroke(
            ToolKind::Padrao,
            Scene::probe_brush(),
            &[GestureSample {
                position: [0.0, 0.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("a dab");

    let started = Instant::now();
    let cost = geometry.sync(&harness.gpu, &mut document).expect("sync");
    let took = started.elapsed().as_secs_f64() * 1000.0;
    let keys = cost.map(|c| c.keys).unwrap_or(0);
    println!("a dab at the centre: {keys} keys meshed in {took:.2} ms");

    // The surface cannot have moved, and re-deriving it must not have been
    // attempted: a full re-mesh here returns the same triangles, so a count is
    // not enough to tell the two apart — the time is.
    assert_eq!(geometry.triangle_count(), before);
    assert!(
        took < 50.0,
        "a sync with no surface key to mesh took {took:.0} ms. An empty key \
         list has been handed to the engine, which reads it as every surface \
         brick."
    );
}
