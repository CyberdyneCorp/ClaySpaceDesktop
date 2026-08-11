//! The ViewModel layer: observable state, and the commands that change it.
//!
//! Deliberately free of `egui`, `wgpu` and `winit`, so every ViewModel can be
//! constructed and driven in a test with no display and no GPU. That absence
//! is a Cargo dependency fact, which CI asserts rather than review.

#![forbid(unsafe_code)]

pub mod command;
pub mod jobs;
pub mod observable;
pub mod scene_vm;
pub mod sculpt_vm;

pub use command::{Axis, Command, CommandQueue};
pub use jobs::{Completion, Generation, JobRunner, Outcome, Progress, Reporter};
pub use observable::{Observable, Watcher};
pub use scene_vm::SceneViewModel;
pub use sculpt_vm::{LastAction, SculptViewModel};
