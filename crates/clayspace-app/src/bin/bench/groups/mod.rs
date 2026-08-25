//! The measurement groups.
//!
//! One module per group, each arranging its own document and returning the
//! figures it took. Kept apart because a measurement is a build-arrange-time
//! sequence and one file of all of them is how a scene gets built for the
//! wrong group.

pub mod authoring;
pub mod bake;
pub mod brushes;
pub mod convert;
pub mod dab;
pub mod history;
pub mod locality;
pub mod mask;
pub mod memory;
pub mod operations;
pub mod render;
pub mod startup;
pub mod tape;
pub mod visible;
pub mod warmup;

use clayspace_view::Gpu;

/// Where the numbers come from — an offscreen target of this size.
pub const VIEWPORT: (u32, u32) = (1280, 800);

pub fn headless_gpu() -> Option<Gpu> {
    match pollster::block_on(Gpu::headless()) {
        Ok(gpu) => Some(gpu),
        Err(e) => {
            eprintln!("no headless GPU, skipping the measurements that need one: {e}");
            None
        }
    }
}
