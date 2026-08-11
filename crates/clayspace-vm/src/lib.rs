//! The ViewModel layer: observable state, and the commands that change it.
//!
//! Deliberately free of `egui`, `wgpu` and `winit`, so every ViewModel can be
//! constructed and driven in a test with no display and no GPU. That absence
//! is a Cargo dependency fact, which CI asserts rather than review.

#![forbid(unsafe_code)]
