//! The Model layer: the domain, and the only layer that reaches the engine.
//!
//! Everything above this sees domain types, never a ClayCore handle. That is
//! what lets the ViewModel layer be tested against a double with no engine,
//! no GPU and no window.

#![forbid(unsafe_code)]

pub use claycore;
