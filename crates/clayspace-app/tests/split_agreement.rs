//! Does an incremental sync leave the same per-key geometry as a full rebuild?
//!
//! A rendered difference says "something is wrong near the edit". This says
//! which key, and whether it lost triangles or gained them.

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

#[test]
fn an_incremental_sync_stores_what_a_rebuild_would() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };

    let mut incremental = SurfaceGeometry::new(&harness.gpu);
    incremental
        .rebuild(&harness.gpu, &mut document)
        .expect("first mesh");

    let dabs: usize = std::env::var("DABS").ok().and_then(|v| v.parse().ok()).unwrap_or(6);
    for i in 0..dabs {
        let t = if dabs > 1 { i as f32 / (dabs - 1) as f32 } else { 0.5 };
        let angle = (t - 0.5) * 1.0;
        let (s, c) = angle.sin_cos();
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &[GestureSample {
                    position: [s * 1.01, 0.1, c * 1.01],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("stroke");
        incremental
            .sync(&harness.gpu, &mut document)
            .expect("sync");
    }

    let mut reference = SurfaceGeometry::new(&harness.gpu);
    reference
        .rebuild(&harness.gpu, &mut document)
        .expect("reference");

    let mine = incremental.stored_triangles();
    let theirs = reference.stored_triangles();

    let mut keys_differing = 0usize;
    let mut missing = 0usize;
    let mut extra = 0usize;
    let mut examples = Vec::new();
    for (key, want) in &theirs {
        let got = mine.get(key).cloned().unwrap_or_default();
        if &got == want {
            continue;
        }
        keys_differing += 1;
        let lost = want.iter().filter(|t| !got.contains(t)).count();
        let gained = got.iter().filter(|t| !want.contains(t)).count();
        missing += lost;
        extra += gained;
        if examples.len() < 6 {
            examples.push(format!(
                "{key:?}: rebuild {} triangles, sync {} — {lost} missing, {gained} extra",
                want.len(),
                got.len()
            ));
        }
    }
    // Keys the sync holds that a rebuild does not know about at all.
    let orphans = mine.keys().filter(|k| !theirs.contains_key(*k)).count();

    println!("\nkeys: {} rebuilt, {} after sync", theirs.len(), mine.len());
    println!("  differing keys : {keys_differing}");
    println!("  triangles missing from the sync : {missing}");
    println!("  triangles the sync has spare    : {extra}");
    println!("  keys the sync holds and a rebuild does not: {orphans}");
    for line in &examples {
        println!("    {line}");
    }
    println!();

    // Not zero, and not expected to be. A subset mesh omits the triangles
    // straddling its boundary (ClayCore #66), so a sync leaves the surface
    // short of them until `settle` re-meshes everything. What this pins is the
    // *scale* of that: it should be a seam's worth of triangles around the
    // edit, not a growing wound.
    //
    // Measured for six dabs at the time of writing: 100 bricks, ~3500
    // triangles each way, out of 733 bricks and ~280k triangles.
    assert_eq!(orphans, 0, "the sync holds geometry a rebuild does not know about");
    assert!(
        keys_differing < theirs.len() / 4,
        "{keys_differing} of {} bricks disagree with a rebuild — that is past a \
         seam and into the surface",
        theirs.len()
    );

    // And settling must close it completely.
    let mut settled = SurfaceGeometry::new(&harness.gpu);
    settled
        .rebuild(&harness.gpu, &mut document)
        .expect("settle");
    assert_eq!(
        settled.stored_triangles(),
        theirs,
        "settling did not reproduce a full rebuild"
    );
}
