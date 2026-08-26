//! The ViewModel layer: observable state, and the commands that change it.
//!
//! Deliberately free of `egui`, `wgpu` and `winit`, so every ViewModel can be
//! constructed and driven in a test with no display and no GPU. That absence
//! is a Cargo dependency fact, which CI asserts rather than review.

#![forbid(unsafe_code)]

pub mod armature_vm;
pub mod command;
mod curve_vm;
pub mod document_vm;
pub mod history_vm;
pub mod jobs;
mod lattice_vm;
mod mask_vm;
pub mod notice;
mod object_vm;
pub mod observable;
mod reference_vm;
pub mod scene_vm;
pub mod sculpt_vm;

pub use command::{Axis, Command, CommandQueue};
pub use history_vm::HistoryViewModel;
pub use jobs::{Completion, Generation, JobRunner, Outcome, Progress, Reporter};
pub use notice::{MemoryState, Notice, NoticeBoard, Severity, Where};
pub use object_vm::ObjectViewModel;
pub use observable::{Observable, Watcher};
pub use scene_vm::SceneViewModel;
pub use sculpt_vm::{LastAction, SculptViewModel, TOOL_SUBSTITUTED};

pub use armature_vm::{ArmatureViewModel, Grab};
pub use curve_vm::CurveViewModel;
pub use document_vm::{DocumentViewModel, Guard};
pub use lattice_vm::LatticeViewModel;
pub use mask_vm::MaskViewModel;
pub use reference_vm::ReferenceViewModel;
