//! The engine adapter.
//!
//! Everything that touches ClayCore and turns it into the domain's vocabulary.
//! It sits *beside* the domain rather than beneath it, so the layers above —
//! ViewModels and the interface — depend on the domain alone and never reach
//! the engine, transitively or otherwise. `tools/check_layering.py` enforces
//! exactly that.
//!
//! A practical benefit falls out of the rule: the ViewModel tests build and
//! run without compiling the C++ engine at all.

#![forbid(unsafe_code)]

pub mod alpha;
pub mod backend;
pub mod document;

pub use alpha::read_alpha;
pub use backend::{BackendPolicy, Operation, SelectionReason, UnavailableBackend};
pub use document::ClayDocument;

pub use claycore;
