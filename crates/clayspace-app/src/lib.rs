//! ClaySpaceDesktop, as a library so its parts can be tested.
//!
//! The binary is a thin shell over this: window, event loop, and the wiring
//! that only the composition root may do.

#![forbid(unsafe_code)]

pub mod geometry;
pub mod input;
pub mod reference;
pub mod session;
pub mod shared;

pub use geometry::{Shading, SurfaceGeometry, SyncCost};
pub use input::{ray_at, ViewportInput};
pub use reference::{conditions, Conditions, Scene};
pub use session::SessionStore;
pub use shared::SharedDocument;
