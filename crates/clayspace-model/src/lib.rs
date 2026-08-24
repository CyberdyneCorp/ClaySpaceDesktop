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

pub mod alpha;
pub mod armature;
pub mod combine;
pub mod conversion;
pub mod curve;
pub mod detail;
pub mod diagnostics;
pub mod document;
pub mod exchange;
pub mod gizmo;
pub mod instrument;
pub mod lattice;
pub mod locale;
pub mod mask;
pub mod scene;
pub mod sculpt;
pub mod session;
pub mod tools;
pub mod units;
pub mod voxel_display;

pub use alpha::{Alpha, AlphaRefusal, AlphaSupport};
pub use armature::{Armature, ArmatureModel, NodeIndex, SkinSettings, Zsphere};
pub use combine::{BlendProfile, Combine, CombineSettings, StrokeModifiers};
pub use conversion::{
    ConversionSettings, Cost, DeformSettings, DeformVerb, Direction, Refusal, RepairReport,
};
pub use curve::{CurveJoin, CurveModel, CurvePoint, CurveProfile, CurveState, FEWEST_POINTS};
pub use detail::DetailPolicy;
pub use diagnostics::{Diagnostics, DiagnosticsModel, Fallback};
pub use document::{DocumentModel, OpenError};
pub use exchange::{
    ExchangeModel, ExportMesher, ExportSettings, ExportWarning, Format, ImportAs, ImportSettings,
};
pub use gizmo::{GizmoDrag, GizmoHandle, GizmoMode};
pub use instrument::{FrameLog, Stall, FRAME};
pub use lattice::{
    can_be_caged, clamp_divisions, division_limit, LatticeModel, LatticeState, MIN_DIVISIONS,
};
pub use locale::Locale;
pub use mask::{can_extrude, ExtrudeSettings, ExtrudeSide, MaskModel, MaskOp, MaskState};
pub use scene::{
    LayerCost, LayerKey, LayerSummary, Protection, Scene, SceneModel, SceneNode, SculptLayer,
    SculptLayerCost, SculptLayerOp,
};
pub use sculpt::{
    Detail, EditOutcome, GestureSample, HistoryEntry, HistoryState, ModelError, SceneStats,
    SculptModel,
};
pub use session::{AutosavePolicy, RecentDocuments, Recovery};
pub use tools::{
    BrushSettings, Falloff, LayerOperation, LayerState, Representation, Shaping, ToolKind,
    Unavailable, Verbs, ViewPresetKind,
};
pub use units::{Unit, Units, UnitsModel};
pub use voxel_display::{SmoothBlur, VoxelDisplay};
