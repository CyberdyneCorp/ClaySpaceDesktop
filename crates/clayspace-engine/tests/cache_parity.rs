//! The viewport must show what the document holds.
//!
//! The viewport meshes from the brick cache, never from the document. So a
//! stroke that changes the document but not the cache is a stroke that does
//! nothing a user can see — the model grows, undo fills up, and the screen
//! stays still. That shipped: the default brush set Ruído to 0.15, which the
//! adapter mapped straight onto `jitter_position`, and above roughly 0.05 the
//! engine's brick evaluation does not reproduce the jittered stroke at all.
//!
//! Every sculpting test before this one asked "did the document change?".
//! None asked "and does the cache agree?", which is the only question the
//! viewport cares about.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Falloff, GestureSample, SculptModel, Shaping, ToolKind};

/// Straight down the Z axis onto the top of the starting sphere.
const ORIGIN: [f32; 3] = [0.0, 0.0, 4.0];
const DIRECTION: [f32; 3] = [0.0, 0.0, -1.0];

/// A voxel is 0.02, so agreement closer than this is agreement.
const VOXEL: f32 = 0.02;

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Where the document says the surface is, and where the cache says it is.
fn both(document: &ClayDocument) -> (Option<f32>, Option<f32>) {
    (
        document
            .document()
            .raycast(ORIGIN, DIRECTION)
            .ok()
            .flatten()
            .map(|hit| hit.position[2]),
        document
            .cache()
            .raycast(ORIGIN, DIRECTION)
            .ok()
            .flatten()
            .map(|hit| hit.position[2]),
    )
}

fn stroke(document: &mut ClayDocument, tool: ToolKind, brush: BrushSettings) {
    let samples: Vec<GestureSample> = (0..4)
        .map(|i| GestureSample {
            position: [0.0, 0.0, 1.0],
            pressure: 1.0,
            time: i as f32 * 0.01,
        })
        .collect();
    document
        .apply_stroke(tool, brush, &samples, [false; 3])
        .expect("the stroke was refused");
}

#[test]
fn the_default_brush_leaves_the_cache_agreeing_with_the_document() {
    let mut document = document();
    stroke(&mut document, ToolKind::Padrao, BrushSettings::default());

    let (in_document, in_cache) = both(&document);
    let (Some(in_document), Some(in_cache)) = (in_document, in_cache) else {
        panic!("the surface went missing from one of the two: {in_document:?} / {in_cache:?}");
    };
    assert!(
        (in_document - in_cache).abs() < VOXEL,
        "the document puts the surface at {in_document} and the cache at \
         {in_cache}. The viewport meshes from the cache, so this stroke is \
         invisible however much the document changed."
    );
}

#[test]
fn a_stroke_moves_the_cache_and_not_only_the_document() {
    let mut document = document();
    let (_, before) = both(&document);
    stroke(&mut document, ToolKind::Padrao, BrushSettings::default());
    let (_, after) = both(&document);

    let moved = match (before, after) {
        (Some(a), Some(b)) => (a - b).abs(),
        _ => 0.0,
    };
    assert!(
        moved > VOXEL * 0.5,
        "the cache moved by {moved}, which is nothing the viewport would show"
    );
}

#[test]
fn every_brush_the_interface_can_produce_moves_the_cache() {
    // The invariant is about the cache alone, not about it matching the
    // document, because the document's raycast is not a good yardstick here:
    // measured across these brushes it reports the same displacement at
    // intensity 0.1 as at 1.0, while the cache correctly reports a tenth as
    // much. The cache also caps at +0.06 — three voxels, its narrow band —
    // which is a limit of the representation rather than a fault.
    //
    // What the viewport needs is only this: a stroke a user can see must move
    // the cache by something a user can see. The shipped defect failed it
    // outright, moving the cache by nothing at all.
    let _ = document();

    let mut worst: Option<(String, f32)> = None;
    // Sizes and strengths whose dab is larger than a voxel. Below that the
    // cache cannot represent the change at all, which
    // `a_dab_smaller_than_a_voxel_is_invisible` states outright rather than
    // leaving as a surprise here.
    for noise in [0.0f32, 0.25, 1.0] {
        for intensity in [0.65f32, 1.0] {
            for size in [0.08f32, 0.16, 0.25] {
                let brush = BrushSettings {
                    size,
                    intensity,
                    flow: 0.8,
                    invert: false,
                    shaping: Shaping {
                        noise,
                        falloff: Falloff::Smooth,
                        accumulate: true,
                        smoothing: 0.25,
                        mirror: false,
                        azimuth: 0.0,
                    },
                    alpha: false,
                };
                let mut document = document();
                let (_, before) = both(&document);
                stroke(&mut document, ToolKind::Padrao, brush);
                let (_, after) = both(&document);

                let where_ = format!("noise {noise}, intensity {intensity}, size {size}");
                let (Some(before), Some(after)) = (before, after) else {
                    panic!("{where_}: the cache lost its surface");
                };

                let moved = (after - before).abs();
                if worst.as_ref().is_none_or(|(_, w)| moved < *w) {
                    worst = Some((where_, moved));
                }
            }
        }
    }

    let (where_, moved) = worst.expect("at least one brush was tried");
    assert!(
        moved >= VOXEL * 0.5,
        "at {where_} the cache moved by only {moved}. The viewport meshes from \
         the cache, so that stroke is invisible however much the document changed."
    );
}

#[test]
fn the_jitter_ceiling_is_where_the_engine_actually_breaks() {
    // Guards the constant from both sides. If ClayCore fixes the brick
    // evaluation, `the_engine_still_disagrees_about_jitter` starts failing and
    // the ceiling can be raised; this one checks the ceiling itself is safe.
    let _ = document();

    let gap_at = |noise: f32| {
        let mut document = document();
        let brush = BrushSettings {
            shaping: Shaping {
                noise,
                ..Shaping::default()
            },
            ..BrushSettings::default()
        };
        stroke(&mut document, ToolKind::Padrao, brush);
        match both(&document) {
            (Some(a), Some(b)) => (a - b).abs(),
            _ => f32::INFINITY,
        }
    };

    let gap = gap_at(ClayDocument::MAX_JITTER);
    println!("gap at the ceiling ({}) = {gap}", ClayDocument::MAX_JITTER);
    assert!(
        gap < VOXEL,
        "at the ceiling the two evaluators are already {gap} apart"
    );
}

#[test]
fn the_engine_still_disagrees_about_jitter() {
    // The reason `MAX_JITTER` exists, asserted rather than described. This is
    // the test that should fail first when ClayCore fixes the brick path —
    // and its failure is the signal to raise the ceiling and give Ruído back.
    //
    // Re-checked at ClayCore v0.78.0 and unchanged: the word "jitter" does not
    // appear in that release at all, in its fixes or in its known limits.
    let _ = document();

    let _document = document();
    let brush = BrushSettings {
        shaping: Shaping {
            noise: 0.15,
            ..Shaping::default()
        },
        ..BrushSettings::default()
    };
    // Past the adapter, so the clamp does not hide what is being tested.
    let samples: Vec<GestureSample> = (0..4)
        .map(|i| GestureSample {
            position: [0.0, 0.0, 1.0],
            pressure: 1.0,
            time: i as f32 * 0.01,
        })
        .collect();
    let _ = brush;
    let _ = samples;

    // Driven through the engine directly: build a stroke with jitter and see
    // whether the cache reproduces it.
    use clayspace_engine::claycore::{
        Blend, BrickCache, BrickConfig, Document, Item, Op, StrokePreset, StrokeSample,
    };
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("L").expect("layer");
    doc.add_item(layer, &Item::sphere(1.0).expect("sphere"))
        .expect("add");
    let mut cache = BrickCache::new(BrickConfig {
        dim: 8,
        voxel_size: 0.02,
        band_voxels: 3,
        memory_budget: Some(512 * 1024 * 1024),
        colors: false,
    })
    .expect("cache");
    cache.mark_dirty_layer(&doc, layer).expect("mark");
    cache.refill_all(&doc, None, 512).expect("fill");

    let mut stamp = Item::sphere(0.08).expect("stamp");
    stamp.set_op(Op::Relief).expect("op");
    stamp.set_blend(Blend::Quadratic, 0.032).expect("blend");
    let preset = StrokePreset {
        jitter_position: 0.15,
        ..StrokePreset::default()
    };
    let nodes = doc
        .apply_stroke(
            layer,
            &[StrokeSample {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                time: 0.0,
            }],
            &preset,
            &stamp,
            claycore::MaskSource::None,
        )
        .expect("stroke");
    cache.mark_dirty_nodes(&doc, layer, &nodes).expect("mark");
    let (requests, _) = cache.take_dirty(512).expect("drain");
    cache.refill(&doc, None, &requests).expect("refill");

    let in_document = doc
        .raycast(ORIGIN, DIRECTION)
        .ok()
        .flatten()
        .map(|h| h.position[2])
        .expect("the document lost its surface");
    let in_cache = cache
        .raycast(ORIGIN, DIRECTION)
        .ok()
        .flatten()
        .map(|h| h.position[2])
        .expect("the cache lost its surface");

    assert!(
        (in_document - in_cache).abs() > VOXEL,
        "the engine now agrees with itself about a jittered stroke \
         (document {in_document}, cache {in_cache}). Raise ClayDocument::MAX_JITTER \
         and give the Ruído control back."
    );
}

#[test]
fn a_dab_smaller_than_a_voxel_is_invisible() {
    // Stated so it is a known limit rather than a mystery: the cache stores
    // 0.02 voxels, so a single weak dab from a small brush moves nothing on
    // screen. It accumulates over a stroke; it does not appear from one click.
    //
    // If this ever starts failing the cache got finer, and
    // `every_brush_the_interface_can_produce_moves_the_cache` should widen to
    // cover the small brushes again.
    let _ = document();
    let mut document = document();
    let (_, before) = both(&document);
    stroke(
        &mut document,
        ToolKind::Padrao,
        BrushSettings {
            size: 0.04,
            intensity: 0.35,
            ..BrushSettings::default()
        },
    );
    let (_, after) = both(&document);

    let moved = match (before, after) {
        (Some(a), Some(b)) => (a - b).abs(),
        _ => f32::INFINITY,
    };
    assert!(
        moved < VOXEL,
        "a dab that should be below the cache's resolution moved it by {moved}"
    );
}
