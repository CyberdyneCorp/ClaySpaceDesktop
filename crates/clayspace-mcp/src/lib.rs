//! The agent-facing door.
//!
//! A Model Context Protocol server that runs inside the application for as
//! long as it is open, so that an agent works the session a sculptor is
//! already in rather than starting one of its own.
//!
//! This crate holds the protocol, the tool catalogue, the mapping from a tool
//! call to a [`clayspace_vm::Command`], the gate table and the refusal
//! vocabulary. It holds no window, no GPU, no engine and no document: what it
//! needs of the running application it asks for through [`session::Session`],
//! which the composition root implements and calls on the interface thread.
//!
//! That seam is the reason the whole surface is exercisable in a test with no
//! display, no GPU and no C++ engine built — the same reason `clayspace-vm`
//! has no `egui` and `clayspace-view` has no ClayCore. CI asserts it rather
//! than review.

#![forbid(unsafe_code)]

pub mod access;
pub mod base64;
pub mod catalogue;
pub mod gate;
pub mod http;
pub mod jsonrpc;
pub mod protocol;
pub mod queue;
pub mod report;
pub mod server;
pub mod session;
pub mod testing;

pub use access::Access;
pub use catalogue::{Bounds, Catalogue};
pub use protocol::{CallResult, Content, Protocol, ToolDescriptor, ToolSurface, PROTOCOL_VERSION};
pub use queue::{Answer, JobQueue};
pub use server::{BindError, Server, ServerHandle};
pub use session::{
    Applied, CaptureRequest, CaptureWhat, Consent, ConsentOutcome, Frame, GateKind, Measured,
    Outstanding, PhaseCostState, Refusal, RefusalCode, Session, Settled, StateQuery, StateReport,
    StrokeCostState,
};
