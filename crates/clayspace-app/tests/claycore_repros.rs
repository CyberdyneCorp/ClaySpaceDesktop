//! What ClayCore actually does, measured.
//!
//! Written to be pasted into issues: each names the entry points, prints the
//! numbers, and asserts the behaviour *as it is today*, so a ClayCore release
//! that changes it fails here rather than surprising us in a render.
//!
//! Two of these exist because a claim did not survive being written down.
//! Subset meshing and jitter were both about to be filed as engine bugs on the
//! strength of a visual artifact; measured properly, subset meshing agrees
//! with whole meshing exactly, and the jitter disagreement was the narrow band
//! being too thin for the brush. Only the mirror (#60) and `CLAY_OP_ADD`
//! ignoring `strength` (#61) are engine bugs.
//!
//! ```sh
//! cargo test -p clayspace-app --test claycore_repros --release -- --nocapture
//! ```

use clayspace_engine::claycore::{
    Blend, BrickCache, BrickConfig, Document, Item, LayerId, Op, StrokePreset, StrokeSample,
};

const CONFIG: BrickConfig = BrickConfig {
    dim: 8,
    voxel_size: 0.01,
    band_voxels: 6,
    memory_budget: Some(512 * 1024 * 1024),
    colors: false,
};

/// A unit sphere in one SDF layer.
fn sphere() -> (Document, LayerId) {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("L").expect("layer");
    doc.add_item(layer, &Item::sphere(1.0).expect("sphere"))
        .expect("add");
    (doc, layer)
}

/// A cache filled from the whole document.
fn filled(doc: &Document) -> BrickCache {
    let mut cache = BrickCache::new(CONFIG).expect("cache");
    cache.mark_dirty([-2.0; 3], [2.0; 3]).expect("mark");
    cache.refill_all(doc, None, 512).expect("fill");
    cache
}

/// A relief stamp of the given radius.
fn stamp(radius: f32) -> Item {
    let mut stamp = Item::sphere(radius).expect("stamp");
    stamp.set_op(Op::Relief).expect("op");
    stamp.set_blend(Blend::Quadratic, radius).expect("blend");
    stamp.set_rounding(radius).expect("rounding");
    stamp
}

/// Distance from the origin to the surface along `direction`.
fn radius_along(cache: &BrickCache, direction: [f32; 3]) -> Option<f32> {
    let n = (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
        .sqrt();
    let u = direction.map(|c| c / n);
    cache
        .raycast(u.map(|c| c * 4.0), u.map(|c| -c))
        .ok()
        .flatten()
        .map(|h| (h.position[0].powi(2) + h.position[1].powi(2) + h.position[2].powi(2)).sqrt())
}

fn doc_radius_along(doc: &Document, direction: [f32; 3]) -> Option<f32> {
    let n = (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
        .sqrt();
    let u = direction.map(|c| c / n);
    doc.raycast(u.map(|c| c * 4.0), u.map(|c| -c))
        .ok()
        .flatten()
        .map(|h| (h.position[0].powi(2) + h.position[1].powi(2) + h.position[2].powi(2)).sqrt())
}

/// A fingerprint of a mesh's vertex positions.
fn fingerprint(positions: &[[f32; 3]]) -> u64 {
    let mut hash = 1469598103934665603u64;
    let mut sorted: Vec<[i64; 3]> = positions
        .iter()
        .map(|p| p.map(|c| (c * 8192.0).round() as i64))
        .collect();
    sorted.sort_unstable();
    for p in sorted {
        for c in p {
            for byte in c.to_le_bytes() {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(1099511628211);
            }
        }
    }
    hash
}

#[test]
fn subset_meshing_reproduces_whole_surface_meshing() {
    // clay_brick_cache_mesh with a key list, against the same call with none.
    let (mut doc, layer) = sphere();
    let mut cache = filled(&doc);

    let nodes = doc
        .apply_stroke(
            layer,
            &[StrokeSample {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                time: 0.0,
            }],
            &StrokePreset {
                radius: 0.18,
                ..Default::default()
            },
            &stamp(0.18),
            None,
        )
        .expect("stroke");
    cache.mark_dirty_nodes(&doc, layer, &nodes).expect("mark");
    let (requests, _) = cache.take_dirty(512).expect("drain");
    let dirty: Vec<_> = requests.iter().map(|r| r.key()).collect();
    cache.refill(&doc, None, &requests).expect("refill");

    let params = Default::default();
    let (whole, _) = cache.mesh(Some(&doc), params, &[]).expect("whole");
    let (subset, ranges) = cache.mesh(Some(&doc), params, &dirty).expect("subset");

    // Every vertex the subset produced, against the vertices the whole mesh
    // produced inside the same bricks. Compared as sets of positions, so
    // ordering and indexing differences do not count.
    let brick = CONFIG.voxel_size * CONFIG.dim as f32;
    let inside = |p: &[f32; 3]| {
        dirty.iter().any(|k| {
            (0..3).all(|a| {
                let lo = k[a] as f32 * brick;
                p[a] >= lo - 1e-4 && p[a] <= lo + brick + 1e-4
            })
        })
    };
    let whole_inside: Vec<[f32; 3]> = whole
        .positions()
        .iter()
        .copied()
        .filter(inside)
        .collect();
    let subset_inside: Vec<[f32; 3]> = subset
        .positions()
        .iter()
        .copied()
        .filter(inside)
        .collect();

    println!("\n--- subset vs whole meshing ---");
    println!("  dirty keys              : {}", dirty.len());
    println!("  ranges returned         : {}", ranges.len());
    println!("  whole mesh vertices     : {}", whole.vertex_count());
    println!("  subset mesh vertices    : {}", subset.vertex_count());
    println!("  ...of which inside dirty: whole {} / subset {}", whole_inside.len(), subset_inside.len());
    println!(
        "  positions agree         : {}",
        fingerprint(&whole_inside) == fingerprint(&subset_inside)
    );

    // And the triangles, which is what actually gets drawn. A vertex set can
    // match while the faces joining them do not.
    let faces = |mesh: &clayspace_engine::claycore::Mesh| {
        let positions = mesh.positions();
        let mut set: Vec<[[i64; 3]; 3]> = mesh
            .indices()
            .chunks_exact(3)
            .filter_map(|t| {
                let corners = [
                    positions.get(t[0] as usize)?,
                    positions.get(t[1] as usize)?,
                    positions.get(t[2] as usize)?,
                ];
                if !corners.iter().all(|p| inside(p)) {
                    return None;
                }
                let mut c = corners.map(|p| p.map(|v| (v * 8192.0).round() as i64));
                c.sort_unstable();
                Some(c)
            })
            .collect();
        set.sort_unstable();
        set
    };
    // The same again, but for triangles with *at least* one corner inside the
    // requested bricks — the ones that span a boundary. Those are where the
    // two calls part company.
    let touching = |mesh: &clayspace_engine::claycore::Mesh| {
        let positions = mesh.positions();
        let mut set: Vec<[[i64; 3]; 3]> = mesh
            .indices()
            .chunks_exact(3)
            .filter_map(|t| {
                let corners = [
                    positions.get(t[0] as usize)?,
                    positions.get(t[1] as usize)?,
                    positions.get(t[2] as usize)?,
                ];
                if !corners.iter().any(|p| inside(p)) {
                    return None;
                }
                let mut c = corners.map(|p| p.map(|v| (v * 8192.0).round() as i64));
                c.sort_unstable();
                Some(c)
            })
            .collect();
        set.sort_unstable();
        set
    };
    let whole_touching = touching(&whole);
    let subset_touching = touching(&subset);
    let only_whole = whole_touching.iter().filter(|t| !subset_touching.contains(t)).count();
    let only_subset = subset_touching.iter().filter(|t| !whole_touching.contains(t)).count();
    println!(
        "  triangles touching     : whole {} / subset {}  ({only_whole} only in whole, {only_subset} only in subset)",
        whole_touching.len(),
        subset_touching.len()
    );

    let whole_faces = faces(&whole);
    let subset_faces = faces(&subset);
    println!(
        "  triangles fully inside  : whole {} / subset {}",
        whole_faces.len(),
        subset_faces.len()
    );
    println!("  triangles agree         : {}", whole_faces == subset_faces);

    // They agree. This was very nearly filed as a ClayCore bug on the strength
    // of a visual artifact whose cause is ours — see `SurfaceGeometry::sync`.
    // Kept as a guard so that if subset meshing ever does diverge, we find out
    // here rather than by looking at a render and guessing.
    assert_eq!(
        fingerprint(&whole_inside),
        fingerprint(&subset_inside),
        "subset meshing no longer reproduces whole-surface vertex positions"
    );
    assert_eq!(
        whole_faces, subset_faces,
        "subset meshing no longer reproduces whole-surface triangles"
    );
}

#[test]
fn the_layer_mirror_has_no_observable_effect() {
    // clay_layer_set_mirror against clay_brick_cache_refill.
    let (mut doc, layer) = sphere();
    doc.set_layer_mirror(layer, [true, false, false], 0.0)
        .expect("mirror");
    let cache = filled(&doc);

    let at = [0.5f32, 0.3, 0.8124];
    let mirrored = [-at[0], at[1], at[2]];
    doc.apply_stroke(
        layer,
        &[StrokeSample {
            position: at,
            pressure: 1.0,
            time: 0.0,
        }],
        &StrokePreset {
            radius: 0.18,
            ..Default::default()
        },
        &stamp(0.18),
        None,
    )
    .expect("stroke");

    // A cache built from nothing afterwards, so dirty marking cannot be
    // blamed for what it does not contain.
    let fresh = filled(&doc);

    println!("\n--- layer mirror in the brick cache ---");
    println!(
        "  document : near {:?} far {:?}",
        doc_radius_along(&doc, at),
        doc_radius_along(&doc, mirrored)
    );
    println!(
        "  cache (pre-stroke) : near {:?} far {:?}",
        radius_along(&cache, at),
        radius_along(&cache, mirrored)
    );
    println!(
        "  cache (rebuilt)    : near {:?} far {:?}",
        radius_along(&fresh, at),
        radius_along(&fresh, mirrored)
    );

    let far_in_document = doc_radius_along(&doc, mirrored).expect("document");
    let far_in_cache = radius_along(&fresh, mirrored).expect("cache");
    // Filed as ClayCore #60. The mirror is accepted and changes nothing — not
    // in the document, not in the cache, and not for a plain placed item
    // either. When this starts failing the engine has been fixed, and
    // ClaySpace's starting symmetry can go back on.
    let near = doc_radius_along(&doc, at).expect("near");
    assert!(
        (far_in_document - 1.0).abs() < 0.02 && (near - 1.0).abs() > 0.02,
        "the layer mirror now reaches the far side (near {near}, far \
         {far_in_document}, cache {far_in_cache}) — see ClayCore #60"
    );
}

#[test]
fn a_jittered_stroke_reaches_a_cache_with_a_wide_enough_band() {
    // clay_stroke_preset.jitter_position against clay_brick_cache_refill.
    let (mut doc, layer) = sphere();
    let mut cache = filled(&doc);

    let nodes = doc
        .apply_stroke(
            layer,
            &[StrokeSample {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                time: 0.0,
            }],
            &StrokePreset {
                radius: 0.18,
                jitter_position: 0.15,
                ..Default::default()
            },
            &stamp(0.18),
            None,
        )
        .expect("stroke");
    cache.mark_dirty_nodes(&doc, layer, &nodes).expect("mark");
    let (requests, _) = cache.take_dirty(512).expect("drain");
    cache.refill(&doc, None, &requests).expect("refill");
    let fresh = filled(&doc);

    let up = [0.0f32, 0.0, 1.0];
    println!("\n--- jitter_position in the brick cache ---");
    println!("  document          : {:?}", doc_radius_along(&doc, up));
    println!("  cache (incremental): {:?}", radius_along(&cache, up));
    println!("  cache (rebuilt)    : {:?}", radius_along(&fresh, up));

    // They agree here. The disagreement that put `ClayDocument::MAX_JITTER` at
    // zero was measured with 0.02 voxels and a 3-voxel band, where the stamp
    // displaces further than the band can carry. Jitter was the wrong culprit.
    let in_document = doc_radius_along(&doc, up).expect("document");
    let in_cache = radius_along(&fresh, up).expect("cache");
    assert!(
        (in_document - in_cache).abs() < 0.02,
        "with a band wide enough for the brush, a jittered stroke should reach \
         the cache: document {in_document}, cache {in_cache}"
    );
}

#[test]
fn op_add_ignores_the_stroke_presets_strength() {
    // clay_stroke_preset.strength against CLAY_OP_ADD, and against
    // CLAY_OP_RELIEF for contrast.
    let displacement = |op: Op, strength: f32| {
        let (mut doc, layer) = sphere();
        let before = radius_along(&filled(&doc), [0.0, 0.0, 1.0]).expect("before");
        let mut item = Item::sphere(0.18).expect("stamp");
        item.set_op(op).expect("op");
        item.set_blend(Blend::Quadratic, 0.18).expect("blend");
        item.set_rounding(0.18).expect("rounding");
        doc.apply_stroke(
            layer,
            &[StrokeSample {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                time: 0.0,
            }],
            &StrokePreset {
                radius: 0.18,
                strength,
                ..Default::default()
            },
            &item,
            None,
        )
        .expect("stroke");
        radius_along(&filled(&doc), [0.0, 0.0, 1.0]).expect("after") - before
    };

    println!("\n--- stroke_preset.strength ---");
    println!("  {:>10} {:>12} {:>12}", "strength", "CLAY_OP_ADD", "CLAY_OP_RELIEF");
    let mut add = Vec::new();
    for strength in [0.0f32, 0.1, 0.5, 1.0] {
        let a = displacement(Op::Add, strength);
        let r = displacement(Op::Relief, strength);
        println!("  {strength:>10} {a:>12.4} {r:>12.4}");
        add.push(a);
    }

    let spread = add
        .iter()
        .fold(f32::NEG_INFINITY, |m, v| m.max(*v))
        - add.iter().fold(f32::INFINITY, |m, v| m.min(*v));
    assert!(
        spread < 1e-4,
        "CLAY_OP_ADD now responds to strength (spread {spread}) — the tools \
         that use it can stop ignoring Intensidade"
    );
}
