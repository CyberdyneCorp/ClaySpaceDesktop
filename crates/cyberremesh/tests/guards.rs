//! The two guards that must hold before any retopology work is built on this.
//!
//! Both are about the sculpting application this engine now lives inside, and
//! neither is about retopology: a second compute engine in the process is the
//! risk this integration carries, and these are the two decisions that contain
//! it.

/// The worker pool is capped, and the cap takes.
///
/// Uncapped, the engine sizes its parallel loops from hardware concurrency —
/// right for a batch run, wrong inside a sculpting application, where "an
/// uncapped bake takes every core the host is trying to share" in the engine
/// authors' own words.
///
/// The setter returns a status, so a refused cap is a real outcome and the
/// wrapper reports it rather than dropping it. A cap that silently did not
/// apply would leave the pool at hardware concurrency while every comment in
/// this workspace said otherwise.
#[test]
fn the_worker_pool_can_be_capped_and_reports_what_it_is() {
    cyberremesh::set_max_worker_threads(4).expect("the cap is accepted");
    assert_eq!(
        cyberremesh::max_worker_threads(),
        4,
        "the cap was accepted and did not take, so the pool is still sized \
         from hardware concurrency"
    );

    // And zero is the documented way back to uncapped, which is what the
    // engine defaults to — asserted so that "0 means uncapped" stays a fact
    // about the engine rather than a belief about it.
    cyberremesh::set_max_worker_threads(0).expect("uncapped is accepted");
    assert_eq!(cyberremesh::max_worker_threads(), 0);

    cyberremesh::set_max_worker_threads(4).expect("re-capped");
}

/// The engine we linked is the one we asked for.
///
/// There is no ABI number and no soname change between minor releases —
/// `libcyber_capi.so.0` names every 0.x — so this and the submodule pin are
/// the whole of the compatibility story. It is weaker than the sibling
/// engine's `EXPECTED_ABI` assertion and weaker *because of the engine*: its
/// authors know, and have put an ABI-level number to their user.
#[test]
fn the_linked_engine_is_v0_8() {
    let version = cyberremesh::version();
    assert_eq!((version.major, version.minor), (0, 8), "linked {version}");
}
