//! ClaySpaceDesktop, as a library so its parts can be tested.
//!
//! The binary is a thin shell over this: window, event loop, and the wiring
//! that only the composition root may do.

#![forbid(unsafe_code)]

pub mod geometry;

pub use geometry::{SurfaceGeometry, SyncCost};
