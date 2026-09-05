//! ClaySpaceDesktop, as a library so its parts can be tested.
//!
//! The binary is a thin shell over this: window, event loop, and the wiring
//! that only the composition root may do.

#![forbid(unsafe_code)]

pub mod geometry;
pub mod input;
pub mod json;
pub mod keys;
pub mod profile_file;
pub mod reference;
pub mod session;
pub mod shared;
pub mod slots;

pub use geometry::{Shading, SurfaceGeometry, SyncCost};
pub use input::{ray_at, ViewportInput};
pub use json::Json;
pub use keys::chord_for;
pub use profile_file::{DocumentShape, LayerShape};
pub use reference::{conditions, Conditions, Scene};
pub use session::SessionStore;
pub use shared::SharedDocument;
