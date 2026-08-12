//! Task 3.9: what a host can actually do with the brick cache's mips.
//!
//! The roadmap listed LOD as unblocked because `read_bricks(lod)`,
//! `build_mip` and `current_lod` are all present. They are — and they are not
//! enough: nothing meshes a mip. Filed as ClayCore #93.
//!
//! So the host halves are built and tested — the policy that decides when a
//! coarser level is worth drawing, and the maintenance that keeps the mips
//! ready — and the one call that would join them is the thing being waited
//! for. These tests hold both halves and the gap between them.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

#[test]
fn the_cache_has_surface_bricks_to_build_mips_from() {
    let Some(mut document) = document() else {
        return;
    };
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: [0.0, 0.0, 0.55],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("a dab");

    let bricks = document.cache().surface_bricks().expect("surface bricks");
    assert!(!bricks.is_empty(), "the starting form produced no bricks");
}

#[test]
fn nothing_meshes_a_mip() {
    // The stopping point. `clay_brick_cache_mesh` takes a document, meshing
    // parameters and a key list — and no level. Its own header says the
    // lattice is the cache's and already decided, which is true at lod 0 and
    // is exactly what a coarse level would have to change.
    //
    // So mips are *readable* — `clay_brick_cache_read_bricks` takes a lod —
    // and not *meshable*. A host that wants a coarse viewport has to march
    // the samples itself, which means reimplementing the mesher this
    // application deliberately does not own.
    //
    // Written as a grep over the header so it fails when a level appears in
    // the meshing call, which is the signal to build LOD selection on top.
    let header = include_str!("../../../vendor/ClayCore/bindings/c/clay.h");
    let signature: String = header
        .split("clay_result clay_brick_cache_mesh(")
        .nth(1)
        .expect("the meshing entry point")
        .split(");")
        .next()
        .expect("its argument list")
        .to_string();

    assert!(
        !signature.contains("lod"),
        "the meshing call grew a level: {signature}\n\
         build the LOD path on it instead of leaving this note"
    );
}

#[test]
fn a_mip_can_be_built_and_read_even_though_it_cannot_be_meshed() {
    // The half that does work, recorded so the gap is precisely one call wide
    // rather than a vague "LOD is not supported".
    let Some(mut document) = document() else {
        return;
    };
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: [0.0, 0.0, 0.55],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("a dab");

    let bricks = document.cache().surface_bricks().expect("surface bricks");
    let Some(first) = bricks.first().copied() else {
        return;
    };
    // The coarse key covering it: each coarse brick spans 2×2×2 fine ones.
    let coarse = [
        first[0].div_euclid(2),
        first[1].div_euclid(2),
        first[2].div_euclid(2),
    ];

    // Buildable only when all eight children are evaluated and clean, so a
    // "not yet" is an ordinary answer rather than a failure — and either
    // answer proves the call is reachable, which is what this records.
    let built = document.cache_mut().build_mip(coarse);
    assert!(
        built.is_ok(),
        "building a mip is not even reachable: {built:?}"
    );
}

#[test]
fn the_surfaces_mips_can_be_built_and_are_ready_to_draw() {
    // The maintenance half. A coarse brick is buildable only when all eight
    // children are evaluated and clean, so this runs against a settled
    // document rather than mid-stroke.
    let Some(mut document) = document() else {
        return;
    };
    for step in 0..6 {
        let t = step as f32 / 5.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &[GestureSample {
                    position: [(t - 0.5) * 0.5, 0.0, 1.0],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("a dab");
    }

    let built = document.build_mips().expect("build the mips");
    assert!(
        built > 0,
        "no coarse brick was buildable over a settled surface"
    );

    // And they report as available, which is what a meshing call would ask
    // before requesting one.
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
fn the_policy_and_the_maintenance_meet_nothing() {
    // The gap, stated where both halves are in scope. The policy can ask for
    // `Detail::Reduced`, the mips exist for it, and there is no call that
    // takes the two together — which is the whole of #93.
    let policy = clayspace_model::DetailPolicy::default();
    let far = policy.decide(clayspace_model::Detail::Full, 10.0, 10_000);
    assert_eq!(far, clayspace_model::Detail::Reduced);

    let header = include_str!("../../../vendor/ClayCore/bindings/c/clay.h");
    let signature: String = header
        .split("clay_result clay_brick_cache_mesh(")
        .nth(1)
        .expect("the meshing entry point")
        .split(");")
        .next()
        .expect("its argument list")
        .to_string();
    assert!(
        !signature.contains("lod"),
        "the meshing call grew a level: {signature}\n\
         join the policy to it and delete this test"
    );
}
