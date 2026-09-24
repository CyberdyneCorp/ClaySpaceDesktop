//! Bounding a field layer's deformer chain by baking the patch that was worked.
//!
//! Every Move dab appends one grab per mirror image to the layer's chain, and
//! nothing ever took one away: the audit measured an undo on a flat layer going
//! from 20 ms to 4.5 s over twenty edits while the geometry did not grow. The
//! end of a field gesture can now ask whether the layer has degraded past a
//! floor and, where it has, bake the region that gesture worked.
//!
//! It can, and by default does not: the floor is zero, because a baked patch
//! measured dearer to refill and to undo than the chain it replaced. These
//! tests pin both halves — that the mechanism works when it is turned on, and
//! the measurement that keeps it off, as a tripwire that fails the day an
//! engine pin reverses it. `clayspace_engine::compaction` has the series.
//!
//! These drive `ClayDocument` the way the sculpt view model does —
//! `begin_gesture`, the segments, `end_gesture` — because the collapse is
//! something the end of a gesture does, and a fixture that only called
//! `apply_stroke` would never meet it.

use clayspace_engine::compaction::CHAIN_FLOOR;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Drag, GestureSample, SceneModel, SculptModel, ToolKind};

const MIRRORED: [bool; 3] = [true, false, false];
const UNMIRRORED: [bool; 3] = [false; 3];

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// The Move brush with the front-only gate on: the setting that degrades a
/// layer fastest, and therefore the worst case the floor is calibrated on.
fn move_brush() -> BrushSettings {
    BrushSettings {
        size: 0.45,
        intensity: 1.0,
        drag: Drag {
            front_only: true,
            ..Drag::default()
        },
        ..BrushSettings::default()
    }
}

/// One Move gesture on the same patch, a little way from the last.
///
/// The patch is the +x side of the sphere; its mirror image is the −x side.
fn work_the_patch(document: &mut ClayDocument, index: usize, symmetry: [bool; 3]) {
    let wobble = (index as f32 * 0.9).sin() * 0.05;
    let at = [0.95, 0.1 + wobble, 0.2];
    let samples: Vec<GestureSample> = (0..4)
        .map(|i| GestureSample {
            position: [at[0] + i as f32 * 0.015, at[1], at[2] + i as f32 * 0.01],
            pressure: 1.0,
            time: i as f32,
        })
        .collect();
    gesture(document, ToolKind::Mover, move_brush(), &samples, symmetry);
}

/// A stamp gesture on the same patch: an item per stamp rather than a link in
/// a chain.
fn stamp_the_patch(document: &mut ClayDocument, tool: ToolKind, index: usize) {
    let wobble = (index as f32 * 0.9).sin() * 0.05;
    let samples: Vec<GestureSample> = (0..3)
        .map(|i| GestureSample {
            position: [0.95, 0.1 + wobble + i as f32 * 0.03, 0.2],
            pressure: 1.0,
            time: i as f32,
        })
        .collect();
    let brush = BrushSettings {
        size: 0.2,
        intensity: 0.6,
        ..BrushSettings::default()
    };
    gesture(document, tool, brush, &samples, MIRRORED);
}

fn gesture(
    document: &mut ClayDocument,
    tool: ToolKind,
    brush: BrushSettings,
    samples: &[GestureSample],
    symmetry: [bool; 3],
) {
    document.begin_gesture();
    document
        .apply_stroke(tool, brush, samples, symmetry)
        .expect("a stroke");
    document.end_gesture();
}

/// The chain length and step scale the engine reports for the starting layer.
fn chain_and_step(document: &ClayDocument) -> (i32, f32) {
    let key = document.scene().layers[0].key;
    let id = document.layer_id(key).expect("the layer");
    let report = document
        .document()
        .field_report(id, 0.5)
        .expect("a field report");
    (report.longest_deformer_chain, report.safe_step_scale)
}

/// How many of 512 rays aimed at the worked patch find the surface.
///
/// The engine's own march rather than `pick`, which answers from the brick
/// cache: `safe_step_scale` is read by the marcher, and past a point it is so
/// small that a ray runs out of steps before it arrives and the form stops
/// rendering where it was worked.
fn rays_that_arrive(document: &ClayDocument) -> usize {
    let mut found = 0;
    for row in 0..16 {
        for column in 0..32 {
            let y = -0.4 + 0.8 * row as f32 / 15.0 + 0.1;
            let z = -0.4 + 0.8 * column as f32 / 31.0 + 0.2;
            let hit = document.document().raycast([4.0, y, z], [-1.0, 0.0, 0.0]);
            if matches!(hit, Ok(Some(_))) {
                found += 1;
            }
        }
    }
    found
}

/// Tasks 1.1–1.4: where the surface starts to fail, and where the floor sits
/// against it.
///
/// Collapsing is switched off so the chain is left to grow, and every gesture
/// records the chain, the step scale and how many rays still arrive — mirrored
/// and unmirrored for a drag, and for two stamping tools. The assertion is the
/// calibration's claim: wherever a ray has been lost, the step scale is already
/// under the floor, so the floor fires before a sculptor could see anything.
#[test]
fn calibrate_the_floor_on_a_worked_patch() {
    let fresh = rays_that_arrive(&sphere());
    let mut worst_arriving_step = 0.0f32;
    for (label, symmetry) in [
        ("move, mirrored", MIRRORED),
        ("move, unmirrored", UNMIRRORED),
    ] {
        let mut document = sphere();
        document.set_compaction_floor(0.0);
        println!("{label}");
        for index in 1..=16 {
            work_the_patch(&mut document, index, symmetry);
            let (chain, step) = chain_and_step(&document);
            let arriving = rays_that_arrive(&document);
            println!("  gesture {index:>2}: chain {chain:>2}, step {step:.4}, rays {arriving}/512");
            if arriving < fresh {
                worst_arriving_step = worst_arriving_step.max(step);
            }
        }
    }
    for tool in [ToolKind::Padrao, ToolKind::Inflar, ToolKind::Camada] {
        let mut document = sphere();
        document.set_compaction_floor(0.0);
        for index in 1..=16 {
            stamp_the_patch(&mut document, tool, index);
        }
        let (chain, step) = chain_and_step(&document);
        println!(
            "{}: sixteen gestures, chain {chain}, step {step:.4}, rays {}/512",
            tool.label(),
            rays_that_arrive(&document)
        );
    }
    println!("highest step scale at which a ray was lost: {worst_arriving_step:.4}");
    assert!(
        worst_arriving_step < CHAIN_FLOOR,
        "a ray was lost at a step scale of {worst_arriving_step}, above the \
         floor of {CHAIN_FLOOR}: the surface fails before the collapse fires"
    );
}

/// A document whose end of gesture collapses at the calibrated floor.
fn compacting_sphere() -> ClayDocument {
    let mut document = sphere();
    document.set_compaction_floor(CHAIN_FLOOR);
    document
}

/// Nothing is collapsed unless a host sets a floor, and nothing in the
/// application does. The chain grows exactly as it did before this existed.
#[test]
fn compaction_is_off_by_default() {
    let mut document = sphere();
    assert_eq!(document.compaction_floor(), 0.0);
    for index in 1..=12 {
        work_the_patch(&mut document, index, MIRRORED);
    }
    assert_eq!(document.compaction_totals().collapses, 0);
    assert!(
        document.last_compaction().is_none(),
        "no plan was even asked for"
    );
    let (chain, _) = chain_and_step(&document);
    assert_eq!(chain, 24, "twelve mirrored gestures, two grabs each");
}

/// Task 6.1 and 6.3: with the floor set, a worked patch's chain returns to
/// zero and stays bounded — and the local path is what keeps it there.
///
/// The second half is the one that matters. The first collapse on a fresh
/// form takes the starting form and reports `whole_layer`; every collapse after
/// it must be local, because the engine's local path retains the volume the
/// first one installed. If the starting form's volume ever came to participate
/// in the layer mirror, the engine would refuse that path and every collapse
/// would silently become whole-layer maintenance — which the policy declines,
/// so the chain would grow again and this would fail on the bound.
#[test]
fn a_worked_patch_stays_bounded_on_the_local_path() {
    let mut document = compacting_sphere();
    let mut longest = 0;
    let mut whole = Vec::new();
    for index in 1..=24 {
        let before = document.compaction_totals().collapses;
        work_the_patch(&mut document, index, MIRRORED);
        let (chain, _) = chain_and_step(&document);
        longest = longest.max(chain);
        if document.compaction_totals().collapses > before {
            assert_eq!(
                chain, 0,
                "gesture {index} collapsed and left a chain of {chain}"
            );
            match document.last_compaction() {
                Some(Ok(collapse)) => whole.push(collapse.whole_layer),
                other => panic!("gesture {index} collapsed and recorded {other:?}"),
            }
        }
    }
    println!("longest chain {longest}, collapses {whole:?}");
    assert!(
        whole.len() >= 2,
        "twenty-four mirrored gestures collapsed {} times; the chain was not bounded",
        whole.len()
    );
    assert!(
        longest <= 20,
        "the chain reached {longest}: past the floor, where the surface starts \
         to be lost, before anything collapsed it"
    );
    assert!(whole[0], "the first collapse takes the starting form");
    assert!(
        whole[1..].iter().all(|whole| !whole),
        "a collapse after the first took the whole layer: {whole:?}. The \
         engine's local path is no longer reached on a mirrored document"
    );
    assert!(document.compaction_totals().stopped.is_empty());
}

/// The bake lands inside the gesture it followed.
///
/// The sculpt view model banks a gesture as every entry between the depth it
/// opened at and the depth after `end_gesture`, and undoes that many as one
/// action. So the collapse has to run *inside* `end_gesture` for one undo to
/// take back the stroke and the bake together — anywhere later and the first
/// undo would take back a bake that changes nothing a sculptor can see.
#[test]
fn the_collapse_is_taken_back_with_its_gesture() {
    let mut document = compacting_sphere();
    for index in 1..=9 {
        work_the_patch(&mut document, index, MIRRORED);
    }
    let (chain_before, _) = chain_and_step(&document);
    let depth = document.history().depth;
    work_the_patch(&mut document, 10, MIRRORED);
    assert_eq!(
        document.compaction_totals().collapses,
        1,
        "the tenth gesture is the one that crosses the floor"
    );
    let entries = document.history().depth - depth;
    assert_eq!(entries, 3, "two grabs and the bake, all inside the gesture");
    for _ in 0..entries {
        assert!(document.undo().expect("undo"));
    }
    assert_eq!(
        chain_and_step(&document).0,
        chain_before,
        "undoing the gesture's entries put back the chain it started from"
    );
}

/// Task 6.2: a session that never builds a chain is never collapsed, however
/// far its step scale falls. Stamps are items, not links; their cure is the
/// whole-layer bake, and this must not reach for them.
#[test]
fn a_stamp_session_is_never_collapsed() {
    let mut document = compacting_sphere();
    for index in 1..=16 {
        stamp_the_patch(&mut document, ToolKind::Padrao, index);
    }
    let (chain, step) = chain_and_step(&document);
    assert_eq!(chain, 0);
    assert!(
        step < CHAIN_FLOOR,
        "the fixture must fall past the floor to prove anything: {step}"
    );
    assert_eq!(
        document.compaction_totals().collapses,
        0,
        "a bake was performed"
    );
    assert!(
        document.last_compaction().is_none(),
        "a layer degraded by stamps was planned for: {:?}",
        document.last_compaction()
    );
}

/// Task 6.5: the collapse runs after the gesture and keeps the surface it was
/// given, to within what a sampled volume can hold.
///
/// Not bit-identical, and the task that asked for that asked for something a
/// bake cannot give: a sampled volume is the surface at a cell's resolution.
/// What it must not do is move what a sculptor drew, so two documents make the
/// same gestures, one collapsing and one not, and the rays marched at the
/// worked patch have to agree to within a sample.
#[test]
fn a_collapse_keeps_the_surface_it_was_given() {
    let mut plain = sphere();
    let mut compacted = compacting_sphere();
    for index in 1..=10 {
        work_the_patch(&mut plain, index, MIRRORED);
        work_the_patch(&mut compacted, index, MIRRORED);
    }
    assert!(
        compacted.compaction_totals().collapses > 0,
        "the fixture never collapsed"
    );
    let mut worst = 0.0f32;
    for row in 0..8 {
        for column in 0..8 {
            let origin = [4.0, -0.2 + 0.08 * row as f32, -0.1 + 0.08 * column as f32];
            let direction = [-1.0, 0.0, 0.0];
            let (Ok(Some(a)), Ok(Some(b))) = (
                plain.document().raycast(origin, direction),
                compacted.document().raycast(origin, direction),
            ) else {
                panic!("a ray at {origin:?} missed one of the two surfaces");
            };
            worst = worst.max((a.t - b.t).abs());
        }
    }
    println!("worst disagreement {worst}");
    assert!(
        worst < 0.04,
        "the collapsed surface stands {worst} from the one the gestures drew, \
         more than one sample of the bake"
    );
}

/// TRIPWIRE: fails on purpose the day the engine refills a baked patch at
/// close to what the chain it replaced cost.
///
/// This is the measurement that keeps the floor at zero. A baked patch is a
/// sampled volume, dearer per brick than the analytic chain it replaces.
/// ClayCore v0.120.1 narrows an undo to a grab's support, but undo over the
/// baked patch remains slower, so collapsing still makes this path dearer.
///
/// The margin is the build's, not the engine's. The sixty was measured on a
/// debug host; an optimised build runs the chain's arithmetic far faster than
/// it runs the volume's lookups. With v0.120.1, Linux debug measured 1.2x on
/// a shared runner while local debug and release measured about 4.9x. The
/// threshold is 1.05x: a direction check with room for the host-dependent
/// gap between the two costs.
///
/// When this fails, re-measure on an idle runner before deciding whether the
/// collapse floor can be enabled.
#[test]
fn a_baked_patch_still_refills_dearer_than_its_chain() {
    // The fastest of three, each taken back and put again: a shared machine
    // only ever adds time, so the minimum is the figure closest to the work.
    let undo_last = |document: &mut ClayDocument, index: usize| {
        let depth = document.history().depth;
        work_the_patch(document, index, MIRRORED);
        let entries = document.history().depth - depth;
        (0..3)
            .map(|_| {
                let started = std::time::Instant::now();
                for _ in 0..entries {
                    document.undo().expect("undo");
                }
                let took = started.elapsed();
                for _ in 0..entries {
                    document.redo().expect("redo");
                }
                took
            })
            .min()
            .expect("three samples")
    };
    let mut plain = sphere();
    let mut compacted = compacting_sphere();
    for index in 1..=10 {
        work_the_patch(&mut plain, index, MIRRORED);
        work_the_patch(&mut compacted, index, MIRRORED);
    }
    assert!(
        compacted.compaction_totals().collapses > 0,
        "the fixture never collapsed"
    );
    let chain = undo_last(&mut plain, 11);
    let baked = undo_last(&mut compacted, 11);
    let ratio = baked.as_secs_f64() / chain.as_secs_f64();
    println!("undo over the chain {chain:?}, over the baked patch {baked:?}: {ratio:.1}x");
    assert!(
        ratio > 1.05,
        "an undo over a baked patch now costs {ratio:.1}x one over the chain it \
         replaced. The collapse has stopped making undo dearer: see \
         `clayspace_engine::compaction` and turn the floor on"
    );
}
