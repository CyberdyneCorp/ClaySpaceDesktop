//! Task 3.9: drawing the brick cache's mips.
//!
//! For three releases the LOD half a host could reach was the half it could
//! not use — `build_mip` built a level, `read_bricks` read one and
//! `current_lod` reported one, and nothing meshed one. That was ClayCore #93,
//! and these tests held the two host halves either side of the gap.
//!
//! ClayCore 0.30.0 closed it with `clay_brick_cache_mesh_lod`, so they now
//! assert the joined path: the policy decides, the maintenance keeps the mips
//! built, and the coarse surface meshes. What is left of the old shape is the
//! set of engine contracts the host path depends on — the refusals at level 1
//! are load-bearing, not incidental, so they are pinned here.

use clayspace_engine::claycore::BrickMeshParams;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Detail, DetailPolicy, GestureSample, SculptModel, ToolKind};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// A dab at the given offset, so a test can settle a surface worth mipping.
fn dab(document: &mut ClayDocument, at: [f32; 3], time: f32) {
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: at,
                pressure: 1.0,
                time,
            }],
            [false; 3],
        )
        .expect("a dab");
}

/// A settled document with mips built over it.
fn settled() -> ClayDocument {
    let mut document = document();
    for step in 0..6 {
        let t = step as f32 / 5.0;
        dab(&mut document, [(t - 0.5) * 0.5, 0.0, 1.0], t);
    }
    document.build_mips().expect("build the mips");
    document
}

/// Face normals and no colours — the only shading level 1 accepts.
fn coarse_params() -> BrickMeshParams {
    BrickMeshParams {
        gradient_normals: false,
        colors: false,
        gradient_eps: None,
    }
}

#[test]
fn the_cache_has_surface_bricks_to_build_mips_from() {
    let mut document = document();
    dab(&mut document, [0.0, 0.0, 0.55], 0.0);

    let bricks = document.cache().surface_bricks().expect("surface bricks");
    assert!(!bricks.is_empty(), "the starting form produced no bricks");
}

#[test]
fn a_mip_can_be_built_and_read() {
    let mut document = document();
    dab(&mut document, [0.0, 0.0, 0.55], 0.0);

    let bricks = document.cache().surface_bricks().expect("surface bricks");
    let first = bricks.first().copied().expect("a surface brick");
    // The coarse key covering it: each coarse brick spans 2×2×2 fine ones.
    let coarse = [
        first[0].div_euclid(2),
        first[1].div_euclid(2),
        first[2].div_euclid(2),
    ];

    // Buildable only when all eight children are evaluated and clean, so a
    // "not yet" is an ordinary answer rather than a failure.
    let built = document.cache_mut().build_mip(coarse);
    assert!(
        built.is_ok(),
        "building a mip is not even reachable: {built:?}"
    );
}

#[test]
fn the_surfaces_mips_can_be_built_and_are_ready_to_draw() {
    let mut document = settled();
    let built = document.build_mips().expect("build the mips");
    assert!(
        built > 0,
        "no coarse brick was buildable over a settled surface"
    );

    let bricks = document.cache().surface_bricks().expect("surface bricks");
    let first = bricks.first().copied().expect("a surface brick");
    let coarse = [
        first[0].div_euclid(2),
        first[1].div_euclid(2),
        first[2].div_euclid(2),
    ];
    let lod = document.coarse_lod(coarse).expect("current lod");
    assert!(
        lod == 0 || lod == 1,
        "a coarse key reported an unexpected level: {lod}"
    );
}

#[test]
fn the_coarse_surface_meshes() {
    // What #93 blocked, now the whole point: triangles out of a mip.
    let document = settled();
    let coarse = document
        .drawable_coarse_keys()
        .expect("the coarse keys with mips");
    if coarse.is_empty() {
        return;
    }

    let (mesh, ranges) = document
        .cache()
        .mesh_lod(None, coarse_params(), 1, &coarse)
        .expect("mesh the coarse level");

    assert!(
        mesh.index_count() > 0,
        "the coarse level meshed to nothing at all"
    );
    assert_eq!(
        ranges.len(),
        coarse.len(),
        "a range per requested key, in the order they were given"
    );
    assert!(
        ranges.iter().any(|range| range.index_count > 0),
        "every coarse key contributed nothing"
    );
}

#[test]
fn the_coarse_surface_is_coarser_than_the_full_one() {
    // The whole reason to draw it. Twice the spacing over the same surface, so
    // the triangle count should fall substantially rather than marginally.
    let document = settled();
    let coarse = document
        .drawable_coarse_keys()
        .expect("the coarse keys with mips");
    if coarse.is_empty() {
        return;
    }

    let (fine_mesh, _) = document
        .cache()
        .mesh_lod(None, coarse_params(), 0, &[])
        .expect("mesh the full level");
    let (coarse_mesh, _) = document
        .cache()
        .mesh_lod(None, coarse_params(), 1, &coarse)
        .expect("mesh the coarse level");

    assert!(
        coarse_mesh.index_count() < fine_mesh.index_count(),
        "the coarse level was not coarser: {} indices against {}",
        coarse_mesh.index_count(),
        fine_mesh.index_count()
    );
}

#[test]
fn level_one_refuses_gradient_normals() {
    // Load-bearing rather than incidental: the host draws the coarse surface
    // face-shaded *because* of this, so a release that started downgrading
    // instead of refusing should show up here rather than as a shading
    // difference nobody traced back.
    let document = settled();
    let coarse = document
        .drawable_coarse_keys()
        .expect("the coarse keys with mips");
    if coarse.is_empty() {
        return;
    }

    let gradient = BrickMeshParams {
        gradient_normals: true,
        colors: false,
        gradient_eps: None,
    };
    let refused = document
        .cache()
        .mesh_lod(Some(document.document()), gradient, 1, &coarse);
    assert!(
        refused.is_err(),
        "level 1 accepted gradient normals; the coarse path can stop \
         forcing face normals"
    );
}

#[test]
fn a_coarse_key_with_no_mip_is_refused() {
    // Why `drawable_coarse_keys` filters rather than handing the whole coarse
    // set over: one unbuilt mip fails the entire call, so an unfiltered list
    // would lose the coarse surface whenever a stroke left a child dirty.
    let mut document = document();
    dab(&mut document, [0.0, 0.0, 0.55], 0.0);

    let bricks = document.cache().surface_bricks().expect("surface bricks");
    let first = bricks.first().copied().expect("a surface brick");
    // Deliberately far from anything the surface occupies, so no mip exists
    // for it however settled the document is.
    let absent = [first[0] + 4096, first[1] + 4096, first[2] + 4096];

    let refused = document
        .cache()
        .mesh_lod(None, coarse_params(), 1, &[absent]);
    assert!(
        refused.is_err(),
        "a coarse key with no mip meshed anyway; the filter in \
         drawable_coarse_keys is no longer what makes the coarse path safe"
    );
}

#[test]
fn every_drawable_coarse_key_actually_meshes() {
    // The filter's contract, stated as the property the host relies on: what
    // it returns can be meshed in one call without a single refusal.
    let document = settled();
    let coarse = document
        .drawable_coarse_keys()
        .expect("the coarse keys with mips");
    if coarse.is_empty() {
        return;
    }

    for key in &coarse {
        assert_eq!(
            document.coarse_lod(*key).expect("current lod"),
            1,
            "a key that reported no mip came back as drawable: {key:?}"
        );
        assert!(
            document
                .cache()
                .mesh_lod(None, coarse_params(), 1, &[*key])
                .is_ok(),
            "a drawable coarse key was refused on its own: {key:?}"
        );
    }
}

#[test]
fn the_policy_and_the_maintenance_now_meet() {
    // The gap, closed. The policy asks for `Reduced`, the mips exist for it,
    // and the call that takes the two together is `mesh_lod` — which is the
    // whole of #93.
    let document = settled();
    let policy = DetailPolicy::default();
    assert_eq!(
        policy.decide(Detail::Full, 10.0, 10_000),
        Detail::Reduced,
        "the policy no longer asks for a coarse surface"
    );

    let coarse = document
        .drawable_coarse_keys()
        .expect("the coarse keys with mips");
    if coarse.is_empty() {
        return;
    }
    let (mesh, _) = document
        .cache()
        .mesh_lod(None, coarse_params(), 1, &coarse)
        .expect("what the policy asked for is drawable");
    assert!(mesh.index_count() > 0, "the coarse surface drew nothing");
}
