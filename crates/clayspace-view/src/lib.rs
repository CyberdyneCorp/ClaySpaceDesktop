//! The View layer: widgets and the renderer.
//!
//! Depends on no engine crate, directly or transitively. A View function reads
//! ViewModel state and emits commands; it cannot reach a ClayCore handle even
//! if someone wants it to, because the dependency is not there to use.

#![forbid(unsafe_code)]
