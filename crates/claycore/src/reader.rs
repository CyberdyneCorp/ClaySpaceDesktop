//! Reading one document from several threads.
//!
//! The engine states the contract: a document is safe to read from more than
//! one thread at once, a reader receives a snapshot that stays valid for the
//! duration of its call, and calls on one mutable handle are the host's to
//! serialize. The batched evaluation entry point is free-threaded against one
//! const document.
//!
//! [`Reader`] is how that reaches Rust. It is `Send + Sync` and borrows the
//! document immutably, so the borrow checker refuses a concurrent mutation
//! rather than leaving it to a convention someone will forget.

use claycore_sys as sys;

use crate::error::Result;
use crate::{Backend, Document, Hit, Snapped};

/// A shared, concurrently usable view of a document.
///
/// Obtained from [`Document::reader`]. While one exists the document cannot be
/// mutated, which is exactly the guarantee the engine asks the host to provide.
#[derive(Clone, Copy)]
pub struct Reader<'doc> {
    doc: &'doc Document,
}

// SAFETY: every method here reaches only entry points the engine documents as
// safe against a const document from several threads at once — batched
// evaluation, picking and snapping. `Reader` cannot mutate, and holding one
// prevents mutation through the borrow checker, so no reader can observe a
// document being rebuilt underneath it.
unsafe impl Send for Reader<'_> {}
// SAFETY: as above; sharing a `&Reader` grants no capability a `Reader` does
// not already have.
unsafe impl Sync for Reader<'_> {}

impl<'doc> Reader<'doc> {
    pub(crate) fn new(doc: &'doc Document) -> Self {
        Self { doc }
    }

    /// Evaluates the field at a batch of points.
    pub fn eval_points(&self, backend: Option<&Backend>, points: &[[f32; 3]]) -> Result<Vec<f32>> {
        self.doc.eval_points(backend, points)
    }

    /// Casts a ray, without attribution.
    pub fn raycast(&self, origin: [f32; 3], direction: [f32; 3]) -> Result<Option<Hit>> {
        self.doc.raycast(origin, direction)
    }

    /// Casts a ray and reports which layer and node it met.
    pub fn raycast_attributed(&self, origin: [f32; 3], direction: [f32; 3]) -> Result<Option<Hit>> {
        self.doc.raycast_attributed(origin, direction)
    }

    /// Casts many rays at once.
    pub fn raycast_many(&self, rays: &[([f32; 3], [f32; 3])]) -> Result<Vec<Option<Hit>>> {
        self.doc.raycast_many(rays)
    }

    /// Moves points onto the nearest surface.
    pub fn snap_to_surface(&self, points: &[[f32; 3]]) -> Result<Vec<Option<Snapped>>> {
        self.doc.snap_to_surface(points)
    }

    /// The tape's Lipschitz safety factor: multiply a field distance by this
    /// before stepping along a ray.
    pub fn safe_step_scale(&self) -> Result<f32> {
        let mut scale = 0.0f32;
        // SAFETY: valid handle and a valid out-parameter; the call only reads.
        crate::error::check(
            unsafe { sys::clay_safe_step_scale(self.doc.as_ptr(), &mut scale) },
            "clay_safe_step_scale",
        )?;
        Ok(scale)
    }
}

impl std::fmt::Debug for Reader<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reader(..)")
    }
}

impl Document {
    /// Opens a shared view usable from several threads at once.
    ///
    /// The returned [`Reader`] borrows the document immutably, so the compiler
    /// rejects any mutation while one is alive — including from the thread
    /// that created it.
    pub fn reader(&self) -> Reader<'_> {
        Reader::new(self)
    }
}
