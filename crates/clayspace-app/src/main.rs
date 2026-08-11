//! The composition root.
//!
//! The one place that constructs the engine bridge, the Model, the ViewModels,
//! the renderer and the window, injecting each downward. No other crate builds
//! a layer other than its own.

#![forbid(unsafe_code)]

fn main() {
    // The window arrives in milestone 2; for now this proves the layering
    // links end to end.
    println!("ClaySpaceDesktop {}", env!("CARGO_PKG_VERSION"));
}
