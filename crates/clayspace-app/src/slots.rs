//! Where each brick's geometry sits in the GPU buffers.
//!
//! The layout lives in [`clayspace_engine::slots`], because an adaptive
//! surface's chunks are placed in the carried buffer by the same rule and the
//! engine crate is where that buffer is built. Re-exported here so the brick
//! path keeps its own name for it.

pub use clayspace_engine::slots::{Blank, Placed, Slot, SlotMap};
