//! The retopology, UV and baking engine, safely.
//!
//! One of two crates in this workspace permitted `unsafe`; `claycore` is the
//! other. Everything above this line reaches the engine through owned types
//! that free what the C ABI hands back, so a refused call cannot leak.
//!
//! ## What this engine is for, and what it is not
//!
//! It owns the second half of `sculpt -> retopo -> UV -> bake`. It has no build
//! or link dependency on any sculpting or volumetric engine and states that as
//! a rule about itself; ClayCore states the mirror image. This application is
//! the only place both are present, which is why the correspondence between
//! them lives here rather than in either.
//!
//! ## Two rules that are not negotiable
//!
//! **A mesh handle is valid for one operation.** The engine's element-id
//! stability contract is that most retopology operations reassign ids and
//! subdivision reassigns *all* of them, with nothing to announce it. So
//! anything keyed on a vertex or face id — a selection, a pin, a mapping back
//! into ClayCore — is stale the moment such a call returns. Build, use, drop.
//!
//! **The worker pool is capped before any work runs.** Uncapped, the engine
//! sizes its parallel loops from hardware concurrency, which is right for a
//! batch run and wrong inside a sculpting application. The cap cannot change a
//! result — the engine pins that with a test comparing a capped run against an
//! uncapped one — so it costs a result nothing and costs a stroke everything.
#![forbid(clippy::undocumented_unsafe_blocks)]

use cyberremesh_sys as sys;

mod bake;
mod conform;
mod error;
mod mesh;
mod remesh;
mod uv;
mod version;

pub use bake::{bake_field, BakeParams, Field, FieldMap, Image};
pub use conform::{conform, Conformed};
pub use error::{Error, Result};
pub use mesh::Mesh;
pub use remesh::{remesh, was_cancelled, QuadMethod, RemeshParams, Unwatched, Watcher};
pub use uv::{atlas, Atlas, AtlasParams};
pub use version::{version, Solver, Version};

/// Caps the engine's worker pool.
///
/// Zero means uncapped, which is the engine's default and is not what an
/// interactive host wants: an uncapped run takes every core the host is trying
/// to share. Callable at any time from any thread; loops already running keep
/// the fan-out they started with.
///
/// The cap **cannot change a result**. The engine's loops split a range into
/// contiguous chunks and each chunk writes only its own indices, so a capped
/// run is byte-identical to an uncapped one — which is what makes it safe to
/// turn down mid-session.
pub fn set_max_worker_threads(threads: u32) -> Result<()> {
    // SAFETY: a plain integer setter with no handle and no out-parameter. The
    // engine documents it as callable from any thread at any time.
    //
    // It returns a status rather than nothing, which is worth checking rather
    // than discarding: a cap that was refused leaves the pool uncapped, and an
    // uncapped pool is the thing this call exists to prevent.
    error::check(
        unsafe { sys::cyber_set_max_worker_threads(threads as _) },
        "cyber_set_max_worker_threads",
    )
}

/// What the pool is capped to, or zero for uncapped.
pub fn max_worker_threads() -> u32 {
    // SAFETY: reads a process-global with no handle.
    unsafe { sys::cyber_max_worker_threads() as u32 }
}
