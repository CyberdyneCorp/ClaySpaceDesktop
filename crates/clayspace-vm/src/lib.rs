//! The ViewModel layer: observable state, and the commands that change it.
//!
//! Deliberately free of `egui`, `wgpu` and `winit`, so every ViewModel can be
//! constructed and driven in a test with no display and no GPU. That absence
//! is a Cargo dependency fact, which CI asserts rather than review.

#![forbid(unsafe_code)]

pub mod armature_vm;
pub mod command;
pub mod document_vm;
pub mod history_vm;
pub mod jobs;
pub mod mask_vm;
pub mod notice;
pub mod observable;
pub mod scene_vm;
pub mod sculpt_vm;

pub use command::{Axis, Command, CommandQueue};
pub use history_vm::HistoryViewModel;
pub use jobs::{Completion, Generation, JobRunner, Outcome, Progress, Reporter};
pub use notice::{MemoryState, Notice, NoticeBoard, Severity, Where};
pub use observable::{Observable, Watcher};
pub use scene_vm::SceneViewModel;
pub use sculpt_vm::{LastAction, SculptViewModel};

pub use armature_vm::{ArmatureViewModel, Grab};
pub use document_vm::{DocumentViewModel, Guard};
pub use mask_vm::MaskViewModel;
