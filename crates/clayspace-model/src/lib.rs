//! The domain: what the application is about, in its own words.
//!
//! Deliberately free of the engine. The interfaces here are what the layers
//! above are written against, and the ClayCore-backed implementations live in
//! `clayspace-engine` beside this crate rather than beneath it.
//!
//! That is what lets a View be unable to reach a ClayCore handle even in
//! principle, and what lets the ViewModel tests run without building a C++
//! engine first.

#![forbid(unsafe_code)]

pub mod scene;
pub mod sculpt;
pub mod tools;

pub use scene::{
    LayerCost, LayerKey, LayerSummary, Protection, Scene, SceneModel, SceneNode,
};
pub use sculpt::{
    Detail, EditOutcome, GestureSample, HistoryEntry, HistoryState, ModelError, SceneStats,
    SculptModel,
};
pub use tools::{
    BrushSettings, Falloff, Representation, Shaping, ToolKind, Unavailable, ViewPresetKind,
};
