//! The viewport's critical path, headlessly.
//!
//! Mark an edit dirty, refill only what it touched, mesh a subset, and read
//! bricks in the form a GPU upload wants. Everything the interactive display
//! depends on is exercised here without a window.

use claycore::{
    BrickCache, BrickConfig, BrickMeshParams, BrickState, Document, Item, MeshParams, Mesher,
};

/// Big enough that a sphere of radius 1 crosses many bricks, small enough that
/// the test stays quick.
fn cache_config() -> BrickConfig {
    BrickConfig {
        dim: 8,
        voxel_size: 0.1,
        band_voxels: 3,
        memory_budget: None,
        colors: false,
    }
}

/// The same cache, carrying colour. Opting in at creation is the engine's
/// rule: a colour lattice has to be evaluated to exist.
fn colored_config() -> BrickConfig {
    BrickConfig {
        colors: true,
        ..cache_config()
    }
}

fn sphere_document() -> (Document, claycore::LayerId) {
    let mut doc = Document::new().expect("create document");
    let layer = doc.add_sdf_layer("Base").expect("add layer");
    let item = Item::sphere(1.0).expect("build sphere");
    doc.add_item(layer, &item).expect("place sphere");
    (doc, layer)
}

/// A cache filled from a unit sphere, with the layer marked and drained.
fn filled_cache() -> (Document, BrickCache) {
    let (doc, layer) = sphere_document();
    let mut cache = BrickCache::new(cache_config()).expect("create cache");
    cache.mark_dirty_layer(&doc, layer).expect("mark layer");
    let accepted = cache.refill_all(&doc, None, 256).expect("refill");
    assert!(accepted > 0, "refilling a sphere accepted no bricks");
    (doc, cache)
}

#[test]
fn an_edit_fills_only_surface_bricks() {
    let (_doc, cache) = filled_cache();
    let stats = cache.stats().expect("stats");

    assert!(stats.surface_bricks > 0, "no surface bricks after refill");
    assert!(
        stats.surface_bricks < stats.tracked_bricks,
        "every tracked brick stores a lattice ({} of {}), so empty space is \
         not being represented implicitly",
        stats.surface_bricks,
        stats.tracked_bricks
    );
    assert!(stats.memory_usage > 0, "surface bricks report no payload");
}

#[test]
fn surface_bricks_are_enumerable_and_meshable() {
    let (doc, cache) = filled_cache();
    let keys = cache.surface_bricks().expect("surface bricks");
    assert!(!keys.is_empty(), "no surface bricks to mesh");

    let (mesh, ranges) = cache
        .mesh(Some(&doc), BrickMeshParams::default(), &keys)
        .expect("mesh every surface brick");

    assert!(!mesh.is_empty(), "meshing surface bricks produced nothing");
    assert_eq!(
        ranges.len(),
        keys.len(),
        "one range per key was not returned"
    );
    assert!(mesh.normals().is_some(), "gradient normals were requested");

    // Every triangle must index a vertex that exists. The weld spans brick
    // seams, so ranges are not independent — but the mesh as a whole is.
    let vertex_count = mesh.vertex_count() as u32;
    assert!(
        mesh.indices().iter().all(|&i| i < vertex_count),
        "an index points past the end of the vertex buffer"
    );
}

#[test]
fn meshing_a_subset_costs_less_than_meshing_everything() {
    let (doc, cache) = filled_cache();
    let all = cache.surface_bricks().expect("surface bricks");
    assert!(
        all.len() > 8,
        "need a scene with more bricks than a dab touches"
    );

    let (whole, _) = cache
        .mesh(Some(&doc), BrickMeshParams::default(), &all)
        .expect("mesh all");

    let subset = &all[..8];
    let (part, ranges) = cache
        .mesh(Some(&doc), BrickMeshParams::default(), subset)
        .expect("mesh subset");

    assert_eq!(ranges.len(), subset.len());
    assert!(
        part.index_count() < whole.index_count(),
        "a subset mesh ({} indices) was not smaller than the whole ({})",
        part.index_count(),
        whole.index_count()
    );

    // The ranges must describe the mesh that was actually returned.
    for range in &ranges {
        assert!(
            range.vertex_first + range.vertex_count <= part.vertex_count() as u32,
            "range for key {:?} runs past the vertex buffer",
            range.key
        );
        assert!(
            range.index_first + range.index_count <= part.index_count() as u32,
            "range for key {:?} runs past the index buffer",
            range.key
        );
    }
}

#[test]
fn bricks_read_back_at_a_fixed_stride_whatever_their_state() {
    let (_doc, cache) = filled_cache();
    let keys = cache.surface_bricks().expect("surface bricks");
    let sample = &keys[..keys.len().min(4)];

    let config = cache.config();
    let samples = cache.read_bricks(sample, 0, 0, false).expect("read bricks");

    assert_eq!(samples.padded_dim, config.dim, "no apron was requested");
    assert_eq!(
        samples.values.len(),
        sample.len() * config.samples_per_brick(0),
        "the stride is not fixed at dim^3 per key"
    );
    assert_eq!(samples.states.len(), sample.len());
    assert!(
        samples.states.iter().all(|s| *s == BrickState::Surface),
        "surface_bricks returned a key that does not store a lattice"
    );
}

#[test]
fn an_apron_widens_every_brick_uniformly() {
    let (_doc, cache) = filled_cache();
    let keys = cache.surface_bricks().expect("surface bricks");
    let sample = &keys[..keys.len().min(2)];
    let config = cache.config();

    for apron in [1, 2] {
        let samples = cache
            .read_bricks(sample, 0, apron, false)
            .expect("read with apron");
        assert_eq!(samples.padded_dim, config.dim + 2 * apron);
        assert_eq!(
            samples.values.len(),
            sample.len() * config.samples_per_brick(apron),
            "apron {apron} did not widen the stride uniformly"
        );
    }
}

#[test]
fn colour_is_opt_in_and_matches_the_distance_stride() {
    let (doc, layer) = sphere_document();
    let mut cache = BrickCache::new(colored_config()).expect("create colour cache");
    cache.mark_dirty_layer(&doc, layer).expect("mark layer");
    cache
        .refill_all(&doc, None, 256)
        .expect("refill with colour");

    let keys = cache.surface_bricks().expect("surface bricks");
    let sample = &keys[..keys.len().min(2)];
    let config = cache.config();

    let without = cache
        .read_bricks(sample, 0, 1, false)
        .expect("read without colour");
    assert!(without.colors.is_none(), "colour was not requested");

    let with = cache
        .read_bricks(sample, 0, 1, true)
        .expect("read with colour");
    let colors = with.colors.expect("colour was requested");
    assert_eq!(
        colors.len(),
        sample.len() * config.samples_per_brick(1) * 4,
        "the RGBA8 lattice does not match the distance lattice's stride"
    );
}

#[test]
fn a_distance_only_cache_refuses_colour_rather_than_inventing_it() {
    let (_doc, cache) = filled_cache();
    let keys = cache.surface_bricks().expect("surface bricks");

    let err = cache
        .read_bricks(&keys[..1], 0, 0, true)
        .expect_err("a cache created without colour cannot return any");
    assert!(
        err.detail().is_some_and(|d| d.contains("colour")),
        "the refusal must say why: {err}"
    );
}

#[test]
fn the_cache_raycasts_the_surface_it_holds() {
    let (_doc, cache) = filled_cache();

    let hit = cache
        .raycast([0.0, 0.0, -5.0], [0.0, 0.0, 1.0])
        .expect("raycast the cache")
        .expect("a ray down the axis must meet a unit sphere at the origin");

    // The sphere has radius 1, so the near face sits at z = -1.
    assert!(
        (hit.position[2] + 1.0).abs() < 0.1,
        "hit at {:?}, which is not the near face of the sphere",
        hit.position
    );
    assert!(hit.normal[2] < 0.0, "the normal must face the ray");

    let miss = cache
        .raycast([10.0, 10.0, -5.0], [0.0, 0.0, 1.0])
        .expect("raycast that misses");
    assert!(
        miss.is_none(),
        "a ray well outside the sphere reported a hit"
    );
}

#[test]
fn document_meshing_is_watertight_by_default() {
    let (doc, _layer) = sphere_document();
    let mesh = doc
        .mesh(MeshParams {
            resolution: 48,
            ..Default::default()
        })
        .expect("mesh the document");

    assert!(!mesh.is_empty());
    let validity = mesh.validate().expect("validate");
    assert!(
        validity.watertight && validity.manifold,
        "the default mesher must be watertight and 2-manifold, got {validity:?}"
    );
}

#[test]
fn the_preview_mesher_is_available_and_cheaper() {
    let (doc, _layer) = sphere_document();
    let params = MeshParams {
        resolution: 48,
        ..Default::default()
    };

    let watertight = doc.mesh(params).expect("marching tetrahedra");
    let preview = doc
        .mesh(MeshParams {
            mesher: Mesher::SurfaceNets,
            ..params
        })
        .expect("surface nets");

    assert!(!preview.is_empty());
    assert!(
        preview.index_count() < watertight.index_count(),
        "surface nets ({}) produced no fewer indices than marching tetrahedra ({})",
        preview.index_count(),
        watertight.index_count()
    );
}

#[test]
fn vertices_copy_into_a_caller_layout_in_one_pass() {
    let (doc, _layer) = sphere_document();
    let mesh = doc
        .mesh(MeshParams {
            resolution: 32,
            ..Default::default()
        })
        .expect("mesh");

    // position (12 bytes) + normal (12 bytes), interleaved, as a renderer
    // would want it.
    const STRIDE: usize = 24;
    let layout = claycore::VertexLayout {
        stride: Some(STRIDE as u32),
        position_offset: Some(0),
        normal_offset: Some(12),
        color_offset: None,
        uv_offset: None,
    };

    let mut buffer = vec![0u8; mesh.vertex_count() * STRIDE];
    mesh.copy_vertices(layout, &mut buffer)
        .expect("copy vertices");

    // The first vertex's position must match what the attribute array reports.
    let expected = mesh.positions()[0];
    let actual: [f32; 3] =
        std::array::from_fn(|i| f32::from_le_bytes(buffer[i * 4..i * 4 + 4].try_into().unwrap()));
    assert_eq!(
        actual, expected,
        "the interleaved copy disagrees with the arrays"
    );

    let mut indices = vec![0u32; mesh.index_count()];
    mesh.copy_indices(&mut indices).expect("copy indices");
    assert_eq!(indices, mesh.indices());
}

#[test]
fn a_layout_naming_an_absent_attribute_is_refused() {
    let (doc, _layer) = sphere_document();
    // No document colours are requested, so the mesh carries none.
    let mesh = doc
        .mesh(MeshParams {
            resolution: 16,
            ..Default::default()
        })
        .expect("mesh");

    if mesh.colors().is_some() {
        return; // Nothing to assert if this build does carry colours.
    }

    let layout = claycore::VertexLayout {
        stride: Some(36),
        position_offset: Some(0),
        normal_offset: Some(12),
        color_offset: Some(24),
        uv_offset: None,
    };
    let mut buffer = vec![0u8; mesh.vertex_count() * 36];

    assert!(
        mesh.copy_vertices(layout, &mut buffer).is_err(),
        "a layout naming colours the mesh does not carry must be refused, \
         not filled with whatever was in the buffer"
    );
}

#[test]
fn states_answers_without_reading_a_sample() {
    let (_doc, cache) = filled_cache();
    let surface = cache.surface_bricks().expect("surface bricks");
    assert!(!surface.is_empty(), "the fixture built no surface bricks");

    // A key the cache has never seen, so the answer covers more than the
    // states a surface brick can be in.
    let unseen = [10_000, 10_000, 10_000];
    let mut keys = surface[..surface.len().min(4)].to_vec();
    keys.push(unseen);

    let states = cache.states(&keys).expect("states");
    assert_eq!(states.len(), keys.len());
    // Zipped against the surface keys this call actually asked about, not
    // against every surface brick: the last entry is the unseen one.
    for (key, state) in keys[..keys.len() - 1].iter().zip(&states) {
        assert_eq!(
            *state,
            BrickState::Surface,
            "{key:?} came back from surface_bricks and does not hold a lattice"
        );
    }
    assert_eq!(
        states.last().copied(),
        Some(BrickState::Missing),
        "a key the cache has never seen should read as missing"
    );

    // The same answer the full read gives, which is what says the states-only
    // form is the same call and not a second implementation.
    let full = cache.read_bricks(&keys, 0, 0, false).expect("read bricks");
    assert_eq!(states, full.states);
}

#[test]
fn states_of_nothing_is_nothing() {
    let (_doc, cache) = filled_cache();
    // Not an error and not a whole-cache query: the C boundary refuses a call
    // that would write nothing, so this one never reaches it.
    assert!(cache.states(&[]).expect("states").is_empty());
}
