//! Does the incremental surface still match a rebuild after a real session?
//!
//! `settle_needed.rs` asks the same question of six gentle dabs on a fresh
//! sphere and answers yes. Reported artifacts — small holes, and torn-looking
//! seams — came from a long session with a snake hook and a polish at four
//! hundred thousand triangles, which is a different question: strokes that
//! *move* the surface across brick boundaries, and dabs that leave a brick
//! uniform where its neighbour still holds surface.
//!
//! It found one. A triangle was filed under whichever key's *vertex* range held
//! its first corner, and welding spans brick seams — so a triangle could be
//! filed under a key holding none of its corners. It survived until that key
//! was replaced by a request whose bricks the triangle did not touch, and then
//! nothing re-emitted it. These tests are what caught it and what holds it
//! fixed.

mod support;

use std::collections::HashSet;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use support::Harness;

fn triangles(geometry: &SurfaceGeometry) -> HashSet<[[i32; 3]; 3]> {
    geometry
        .stored_triangles()
        .into_values()
        .flatten()
        .collect()
}

/// One gesture of `samples` along a path, synced as a stroke would be.
fn stroke(
    incremental: &mut SurfaceGeometry,
    harness: &Harness,
    document: &mut ClayDocument,
    tool: ToolKind,
    settings: BrushSettings,
    path: &[[f32; 3]],
) {
    for (step, position) in path.iter().enumerate() {
        document
            .apply_stroke(
                tool,
                settings,
                &[GestureSample {
                    position: *position,
                    pressure: 1.0,
                    time: step as f32 * 0.01,
                }],
                [false; 3],
            )
            .expect("a dab");
        incremental
            .sync(&harness.gpu, document)
            .expect("an incremental sync");
    }
}

#[test]
fn a_long_session_still_draws_what_a_rebuild_would() {
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

    // Build up: strong additive dabs around the form, which is what pushes
    // bricks in and out of holding a surface.
    let strong = BrushSettings {
        intensity: 0.9,
        ..BrushSettings::default()
    };
    let ring: Vec<[f32; 3]> = (0..24)
        .map(|step| {
            let angle = step as f32 / 24.0 * std::f32::consts::TAU;
            [angle.cos() * 0.9, angle.sin() * 0.9, 0.7]
        })
        .collect();
    stroke(
        &mut incremental,
        &harness,
        &mut document,
        ToolKind::Padrao,
        strong,
        &ring,
    );

    // Pull tendrils out, which carries the surface across brick boundaries.
    let pull: Vec<[f32; 3]> = (0..12)
        .map(|step| {
            let out = 1.0 + step as f32 * 0.06;
            [0.0, 0.3, out]
        })
        .collect();
    stroke(
        &mut incremental,
        &harness,
        &mut document,
        ToolKind::Puxar,
        strong,
        &pull,
    );

    // And take material away, which is what leaves a brick uniform beside a
    // neighbour that still holds surface.
    let carve = BrushSettings {
        intensity: 0.9,
        invert: true,
        ..BrushSettings::default()
    };
    let cut: Vec<[f32; 3]> = (0..16)
        .map(|step| {
            let angle = step as f32 / 16.0 * std::f32::consts::TAU;
            [angle.cos() * 0.5, angle.sin() * 0.5, 0.95]
        })
        .collect();
    stroke(
        &mut incremental,
        &harness,
        &mut document,
        ToolKind::Padrao,
        carve,
        &cut,
    );

    let mut rebuilt = SurfaceGeometry::new(&harness.gpu);
    rebuilt
        .rebuild(&harness.gpu, &mut document)
        .expect("rebuild");

    let (mine, theirs) = (triangles(&incremental), triangles(&rebuilt));
    let missing = theirs.difference(&mine).count();
    let extra = mine.difference(&theirs).count();
    println!(
        "after 52 dabs across three strokes: sync {} triangles, rebuild {} — \
         {missing} missing, {extra} spare",
        mine.len(),
        theirs.len()
    );

    // Holes are what a sculptor sees.
    assert_eq!(
        missing, 0,
        "the incremental surface is missing {missing} triangles a rebuild has — \
         these are the holes and torn seams a long session shows"
    );
    assert_eq!(
        extra, 0,
        "the incremental surface holds {extra} triangles a rebuild does not"
    );
}

/// One stroke at a time, to say which kind of edit loses triangles.
fn only(kind: &str) -> usize {
    let Some(harness) = Harness::new() else {
        return 0;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return 0;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return 0;
    };
    let mut incremental = SurfaceGeometry::new(&harness.gpu);
    incremental
        .rebuild(&harness.gpu, &mut document)
        .expect("first mesh");

    let strong = BrushSettings {
        intensity: 0.9,
        ..BrushSettings::default()
    };
    match kind {
        "ring" => {
            let path: Vec<[f32; 3]> = (0..24)
                .map(|s| {
                    let a = s as f32 / 24.0 * std::f32::consts::TAU;
                    [a.cos() * 0.9, a.sin() * 0.9, 0.7]
                })
                .collect();
            stroke(
                &mut incremental,
                &harness,
                &mut document,
                ToolKind::Padrao,
                strong,
                &path,
            );
        }
        "pull" => {
            let path: Vec<[f32; 3]> = (0..12).map(|s| [0.0, 0.3, 1.0 + s as f32 * 0.06]).collect();
            stroke(
                &mut incremental,
                &harness,
                &mut document,
                ToolKind::Puxar,
                strong,
                &path,
            );
        }
        _ => {
            let carve = BrushSettings {
                intensity: 0.9,
                invert: true,
                ..BrushSettings::default()
            };
            let path: Vec<[f32; 3]> = (0..16)
                .map(|s| {
                    let a = s as f32 / 16.0 * std::f32::consts::TAU;
                    [a.cos() * 0.5, a.sin() * 0.5, 0.95]
                })
                .collect();
            stroke(
                &mut incremental,
                &harness,
                &mut document,
                ToolKind::Padrao,
                carve,
                &path,
            );
        }
    }

    let mut rebuilt = SurfaceGeometry::new(&harness.gpu);
    rebuilt
        .rebuild(&harness.gpu, &mut document)
        .expect("rebuild");
    let (mine, theirs) = (triangles(&incremental), triangles(&rebuilt));
    let missing = theirs.difference(&mine).count();
    println!(
        "{kind}: {missing} missing, {} spare",
        mine.difference(&theirs).count()
    );
    missing
}

#[test]
fn which_stroke_loses_triangles() {
    let ring = only("ring");
    let pull = only("pull");
    let carve = only("carve");
    println!("ring {ring}, pull {pull}, carve {carve}");
    assert_eq!((ring, pull, carve), (0, 0, 0), "a stroke lost triangles");
}

#[test]
fn where_do_the_missing_triangles_sit() {
    // A diagnostic rather than a guarantee: it prints where the losses are so
    // the mechanism can be identified from data instead of from reasoning.
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
        .expect("first");

    let carve = BrushSettings {
        intensity: 0.9,
        invert: true,
        ..BrushSettings::default()
    };
    let path: Vec<[f32; 3]> = (0..16)
        .map(|s| {
            let a = s as f32 / 16.0 * std::f32::consts::TAU;
            [a.cos() * 0.5, a.sin() * 0.5, 0.95]
        })
        .collect();
    stroke(
        &mut incremental,
        &harness,
        &mut document,
        ToolKind::Padrao,
        carve,
        &path,
    );

    let mut rebuilt = SurfaceGeometry::new(&harness.gpu);
    rebuilt
        .rebuild(&harness.gpu, &mut document)
        .expect("rebuild");

    let mine = triangles(&incremental);
    // Which key the rebuild filed each missing triangle under, and whether we
    // hold that key at all.
    let ours = incremental.stored_triangles();
    let mut by_key: std::collections::BTreeMap<[i32; 3], usize> = Default::default();
    for (key, tris) in rebuilt.stored_triangles() {
        for t in tris {
            if !mine.contains(&t) {
                *by_key.entry(key).or_default() += 1;
            }
        }
    }
    println!("missing triangles by the key a rebuild files them under:");
    for (key, count) in &by_key {
        let held = ours.get(key).map(|t| t.len()).unwrap_or(0);
        println!("  key {key:?}: {count} missing; we hold {held} triangles for it");
    }
}

/// The carve, truncated to `dabs`, and how many triangles that loses.
fn carve_of(dabs: usize) -> Option<usize> {
    let harness = Harness::new()?;
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let mut incremental = SurfaceGeometry::new(&harness.gpu);
    incremental.rebuild(&harness.gpu, &mut document).ok()?;

    let carve = BrushSettings {
        intensity: 0.9,
        invert: true,
        ..BrushSettings::default()
    };
    let path: Vec<[f32; 3]> = (0..dabs)
        .map(|s| {
            let a = s as f32 / 16.0 * std::f32::consts::TAU;
            [a.cos() * 0.5, a.sin() * 0.5, 0.95]
        })
        .collect();
    stroke(
        &mut incremental,
        &harness,
        &mut document,
        ToolKind::Padrao,
        carve,
        &path,
    );

    let mut rebuilt = SurfaceGeometry::new(&harness.gpu);
    rebuilt.rebuild(&harness.gpu, &mut document).ok()?;
    Some(
        triangles(&rebuilt)
            .difference(&triangles(&incremental))
            .count(),
    )
}

#[test]
fn how_few_dabs_lose_a_triangle() {
    // A 52-dab reproduction is not something anybody can reason about. This
    // finds the smallest one that still fails, which is what makes the defect
    // tractable.
    for dabs in 1..=16 {
        let Some(missing) = carve_of(dabs) else {
            return;
        };
        println!("{dabs} dab(s): {missing} missing");
        if missing > 0 {
            println!("--> the smallest losing case is {dabs} dab(s)");
            return;
        }
    }
    println!("no dab count up to 16 lost anything");
}

#[test]
fn the_four_dab_case() {
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
        .expect("first");

    let carve = BrushSettings {
        intensity: 0.9,
        invert: true,
        ..BrushSettings::default()
    };
    let path: Vec<[f32; 3]> = (0..6)
        .map(|s| {
            let a = s as f32 / 16.0 * std::f32::consts::TAU;
            [a.cos() * 0.5, a.sin() * 0.5, 0.95]
        })
        .collect();
    stroke(
        &mut incremental,
        &harness,
        &mut document,
        ToolKind::Padrao,
        carve,
        &path,
    );

    let mut rebuilt = SurfaceGeometry::new(&harness.gpu);
    rebuilt
        .rebuild(&harness.gpu, &mut document)
        .expect("rebuild");

    let mine = triangles(&incremental);
    let ours = incremental.stored_triangles();
    for (key, tris) in rebuilt.stored_triangles() {
        for t in tris {
            if !mine.contains(&t) {
                println!("MISSING triangle filed by the rebuild under key {key:?}");
                println!("  corners (1/4096 units): {t:?}");
                println!(
                    "  we hold {} triangles for that key",
                    ours.get(&key).map(|v| v.len()).unwrap_or(0)
                );
                println!("  do we hold that key at all? {}", ours.contains_key(&key));
                let config = document.cache().config();
                let span = config.voxel_size * config.dim as f32;
                println!(
                    "  brick span {span} world units (dim {}, voxel {})",
                    config.dim, config.voxel_size
                );
                for corner in t {
                    let world = corner.map(|c| c as f32 / 4096.0);
                    let key: [i32; 3] = std::array::from_fn(|i| (world[i] / span).floor() as i32);
                    println!("    corner {world:?} -> brick {key:?}");
                }
            }
        }
    }
}

#[test]
fn do_the_two_surface_queries_agree() {
    // Filtering the same dilated request through `surface_bricks()` rather
    // than through `states()` took the losses from 17 and 12 to 2 and 1, so
    // the two disagree somewhere. This asks them the same question directly.
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
        .expect("first");

    let carve = BrushSettings {
        intensity: 0.9,
        invert: true,
        ..BrushSettings::default()
    };
    for step in 0..6 {
        let a = step as f32 / 16.0 * std::f32::consts::TAU;
        document
            .apply_stroke(
                ToolKind::Padrao,
                carve,
                &[GestureSample {
                    position: [a.cos() * 0.5, a.sin() * 0.5, 0.95],
                    pressure: 1.0,
                    time: step as f32 * 0.01,
                }],
                [false; 3],
            )
            .expect("a dab");

        // Ask both, about the same keys, at the same moment — before `sync`
        // drains the dirty set.
        let surface: std::collections::HashSet<[i32; 3]> = document
            .cache()
            .surface_bricks()
            .expect("surface bricks")
            .into_iter()
            .collect();
        let dirty = document.dirty_keys().to_vec();
        let states = document.cache().states(&dirty).expect("states");
        let mut disagree = 0;
        for (key, state) in dirty.iter().zip(&states) {
            let says_surface = *state == clayspace_engine::claycore::BrickState::Surface;
            let listed = surface.contains(key);
            if says_surface != listed {
                if disagree < 6 {
                    println!(
                        "  dab {step}: key {key:?} — states() says {state:?}, \
                         surface_bricks() {} it",
                        if listed { "lists" } else { "does not list" }
                    );
                }
                disagree += 1;
            }
        }
        println!(
            "dab {step}: {} dirty keys, {disagree} disagreements",
            dirty.len()
        );

        incremental.sync(&harness.gpu, &mut document).expect("sync");
    }
}
