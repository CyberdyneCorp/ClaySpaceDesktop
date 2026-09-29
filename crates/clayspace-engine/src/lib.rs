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

pub mod adaptive;
pub mod alpha;
pub mod backend;
pub mod chunked;
pub mod compaction;
pub mod document;
mod extrude_region;
pub mod grid_to_field;
mod holes;
mod live;
mod maintenance;
pub mod multires;
pub mod objects;
mod reference;
mod retopo;
mod retopo_session;
mod rigs;
mod sculptors;
mod seed;
pub mod slots;

pub use alpha::read_alpha;
pub use backend::{BackendPolicy, Operation, SelectionReason, UnavailableBackend};
pub use compaction::{Collapse, CompactionTotals, Declined};
pub use document::{CarriedPatch, CarriedSpan, ClayDocument, RefillBudget};
pub use grid_to_field::{FieldFromGrid, GridToField};
pub use live::LiveSurface;
pub use reference::read_reference;
pub use retopo::{EngineBaker, EngineConformer, EngineRetopologiser, EngineUnwrapper};

pub use claycore;
