//! The View layer: widgets and the renderer.
//!
//! Depends on no engine crate, directly or transitively. A View function reads
//! ViewModel state and emits commands; it cannot reach a ClayCore handle even
//! if someone wants it to, because the dependency is not there to use.
//!
//! The renderer takes plain vertex and index data for the same reason. It can
//! draw a document, a voxel grid or a test fixture without knowing which, and
//! it can draw into a window or into an image — which is what lets every
//! visual feature be checked in CI rather than by eye on someone's desk.

#![forbid(unsafe_code)]

pub mod camera;
pub mod gpu;
pub mod matcap;
pub mod offscreen;
pub mod palette;
pub mod renderer;

pub use camera::{Camera, ViewPreset};
pub use gpu::{Framebuffer, Gpu, GpuError};
pub use matcap::MatCap;
pub use offscreen::{Image, OffscreenTarget};
pub use renderer::{GpuMesh, Overlays, Renderer, SymmetryAxis, Vertex};
