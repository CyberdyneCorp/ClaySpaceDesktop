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
pub mod boolean;
pub mod colour;
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
pub mod outline;
pub mod reference;
pub mod scene;
pub mod sculpt;
pub mod session;
pub mod shape;
pub mod surface;
pub mod tools;
pub mod units;
pub mod voxel_display;

pub use alpha::{Alpha, AlphaRefusal, AlphaSupport};
pub use armature::{Armature, ArmatureModel, NodeIndex, SkinSettings, Zsphere};
pub use boolean::{BooleanOp, BooleanRefusal, BooleanSettings};
pub use colour::{Colour, ColourState};
pub use combine::{BlendProfile, Combine, CombineSettings, StrokeModifiers};
pub use conversion::{
    ConversionSettings, Cost, DeformSettings, DeformVerb, Direction, Refusal, RepairReport,
};
pub use curve::{CurveJoin, CurveModel, CurvePoint, CurveProfile, CurveState, FEWEST_POINTS};
pub use detail::DetailPolicy;
pub use diagnostics::{AoDiagnostics, Diagnostics, DiagnosticsModel, Fallback, RenderDiagnostics};
pub use document::{DocumentModel, OpenError};
pub use exchange::{
    ExchangeModel, ExportMesher, ExportSettings, ExportWarning, Format, ImportAs, ImportSettings,
};
pub use gizmo::{
    drag_plane, perpendicular_frame, ray_hits_segment, ray_hits_sphere, ring_samples, snapped,
    GizmoDrag, GizmoHandle, GizmoMode, Transform, SNAP_DEGREES,
};
pub use instrument::{FrameLog, Stall, FRAME};
pub use lattice::{
    can_be_caged, clamp_divisions, division_limit, LatticeModel, LatticeState, MIN_DIVISIONS,
};
pub use locale::Locale;
pub use mask::{can_extrude, ExtrudeSettings, ExtrudeSide, MaskModel, MaskOp, MaskState};
pub use outline::{
    cells_to_write, coverage_path, lattice_pitch, MaskGesture, MaskOutline, OutlineDraft,
    OutlineFrame, OutlineMode, CELL_CEILING, COVERING, OUTLINE_SPACING,
};
pub use reference::{
    read_references, write_references, RefFormat, RefPlane, ReferenceImage, ReferenceRefusal,
    ReferenceSettings, RememberedReference,
};
pub use scene::{
    FieldHealth, LayerCost, LayerKey, LayerSummary, Protection, RemeshOutcome, RemeshSettings,
    Scene, SceneModel, SceneNode, SculptLayer, SculptLayerCost, SculptLayerOp, VoxelStats,
};
pub use sculpt::{
    Detail, EditOutcome, GestureSample, HistoryEntry, HistoryState, ModelError, SceneStats,
    SculptModel,
};
pub use session::{AutosavePolicy, RecentDocuments, Recovery};
pub use shape::{
    GizmoTarget, InsertAs, Inserted, ItemKind, ObjectId, ObjectModel, ObjectSource, SceneObject,
    Shape, ShapeParameter, OBJECT_VERBS, PARAMETER_KEYS,
};
pub use surface::SurfaceOpacity;
pub use tools::{
    BrushSettings, Falloff, LayerOperation, LayerState, Representation, Shaping, ToolKind,
    ToolNote, Unavailable, Verbs, ViewPresetKind,
};
pub use units::{Unit, Units, UnitsModel};
pub use voxel_display::{SmoothBlur, VoxelDisplay};
