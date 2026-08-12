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

pub mod armature;
pub mod diagnostics;
pub mod document;
pub mod mask;
pub mod scene;
pub mod sculpt;
pub mod session;
pub mod tools;
pub mod units;

pub use armature::{Armature, ArmatureModel, NodeIndex, SkinSettings, Zsphere};
pub use diagnostics::{Diagnostics, DiagnosticsModel, Fallback};
pub use document::{DocumentModel, OpenError};
pub use mask::{ExtrudeSettings, ExtrudeSide, MaskModel, MaskOp, MaskState};
pub use scene::{LayerCost, LayerKey, LayerSummary, Protection, Scene, SceneModel, SceneNode};
pub use sculpt::{
    Detail, EditOutcome, GestureSample, HistoryEntry, HistoryState, ModelError, SceneStats,
    SculptModel,
};
pub use session::{AutosavePolicy, RecentDocuments, Recovery};
pub use tools::{
    BrushSettings, Falloff, Representation, Shaping, ToolKind, Unavailable, ViewPresetKind,
};
pub use units::{Unit, Units, UnitsModel};
