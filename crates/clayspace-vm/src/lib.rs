//! The ViewModel layer: observable state, and the commands that change it.
//!
//! Deliberately free of `egui`, `wgpu` and `winit`, so every ViewModel can be
//! constructed and driven in a test with no display and no GPU. That absence
//! is a Cargo dependency fact, which CI asserts rather than review.

#![forbid(unsafe_code)]

pub mod agent_vm;
pub mod armature_vm;
mod bake_vm;
pub mod boolean_vm;
pub mod command;
mod conform_vm;
mod curve_vm;
mod cut_vm;
pub mod document_vm;
pub mod history_vm;
pub mod jobs;
mod lattice_vm;
mod mask_vm;
pub mod notice;
mod object_vm;
pub mod observable;
mod reference_vm;
mod retopo_vm;
pub mod scene_vm;
pub mod sculpt_vm;
mod uv_vm;

pub use agent_vm::{AgentAnswer, AgentAsk, AgentGate, AgentViewModel, Door};
pub use bake_vm::{BakeViewModel, Snapshotter};
pub use command::{Axis, Command, CommandQueue};
pub use conform_vm::ConformViewModel;
pub use history_vm::HistoryViewModel;
pub use jobs::{Completion, Generation, JobRunner, Outcome, Progress, Reporter};
pub use notice::{MemoryState, Notice, NoticeBoard, Severity, Where};
pub use object_vm::{ObjectViewModel, Picked, ITEM_NOT_TRANSFORMABLE};
pub use observable::{Observable, Watcher};
pub use retopo_vm::RetopoViewModel;
pub use scene_vm::SceneViewModel;
pub use sculpt_vm::{LastAction, SculptViewModel, TOOL_SUBSTITUTED};
pub use uv_vm::UvViewModel;

pub use armature_vm::{ArmatureViewModel, Grab};
pub use boolean_vm::BooleanViewModel;
pub use curve_vm::CurveViewModel;
pub use cut_vm::{CutDraft, CutViewModel};
pub use document_vm::{DocumentViewModel, Guard, UNTITLED};
pub use lattice_vm::LatticeViewModel;
pub use mask_vm::MaskViewModel;
pub use reference_vm::ReferenceViewModel;
