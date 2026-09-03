//! The queue a host drains between interactions.
//!
//! A sculpt runtime accumulates jobs that make the *next* interaction cheaper
//! and *this* one slower: rebuilding a spatial index whose partition has
//! decayed, compacting a chunk arena that splits have left holes in, promoting
//! a detail field that is now dense, recomputing normals a drag deferred.
//! Every one of them is a stall if it happens while a finger is on the glass,
//! and none of them is the engine's decision, because the engine does not own
//! the moment between two interactions. The host does.
//!
//! So they are queued and not done. Nothing here performs any work: an item is
//! a *request* naming a [`MaintenanceKind`] and a target, and what services it
//! is an ordinary entry point the host already has —
//! [`crate::MeshSculptor::refresh`] for an index,
//! `clay_mesh_sculptor_flush_normals` for deferred normals. That is
//! deliberate: an item a host declines, defers forever or performs differently
//! must leave the surface correct either way, and it does, because none of it
//! was correctness in the first place.
//!
//! # The stroke gate is a mechanism rather than a convention
//!
//! "We only call this between strokes" is a rule that survives until the
//! second caller. The queue *refuses* to hand anything out while a stroke is
//! open, so a host that wired its drain to the wrong callback finds out by
//! nothing happening rather than by a stutter it will blame on the brush.
//!
//! In Rust the gate is [`MaintenanceQueue::stroke`], which returns a guard
//! whose `Drop` closes the stroke — the one form that cannot be left shut when
//! the stroke loop unwinds past a `?` or a panic. The guard derefs to the
//! queue, because requesting work during a stroke is exactly what a stamp
//! does: a stroke asks for the same rebuild on every dab, and folding those
//! into one entry is what the queue is for.
//!
//! ```no_run
//! # use claycore::{MaintenanceKind, MaintenanceQueue};
//! # fn main() -> claycore::Result<()> {
//! let mut queue = MaintenanceQueue::new()?;
//! {
//!     let mut stroke = queue.stroke()?;
//!     // ... every dab of the drag, folded into one entry ...
//!     stroke.request(MaintenanceKind::IndexRebuild, 0, 0)?;
//!     assert!(stroke.take_next()?.is_none(), "not while a finger is down");
//! }
//! while let Some(item) = queue.take_next()? {
//!     // ... do the work the item names, if the budget allows ...
//!     queue.complete(item.kind, item.target)?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Take and complete, rather than a callback
//!
//! The C++ form of this is `service(budget, run)` and a function pointer would
//! have crossed the ABI unchanged. It does not, for a reason worth keeping on
//! this side too: `run` is host code, host code that queued another item while
//! the queue was mid-drain would mutate the vector being walked, and a
//! boundary that cannot prevent that should not offer it.
//!
//! [`take_next`](MaintenanceQueue::take_next) **peeks**: the item stays queued
//! until [`complete`](MaintenanceQueue::complete) says it was done, so a host
//! that took one and then decided it could not afford it has *declined* rather
//! than dropped it. It answers `None` both while a stroke is open and while
//! the queue is empty, which is the same answer to "is there work I may do
//! now" — the question a drain is actually asking.

use std::ptr::NonNull;

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::error::{check, ErrorKind, Result};
use crate::{engine_text, raw_failure};

/// Work that is not required for correctness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaintenanceKind {
    /// The spatial index's partition has decayed under local edits.
    ///
    /// Advisory, and the most likely one for a host to decline: measured over
    /// five deformations, a rebuild produced a better tree in exactly one and
    /// a dramatically worse one in two.
    IndexRebuild,
    /// The chunk table's face arena has slack a split left behind.
    ChunkCompaction,
    /// A sparse detail field whose coverage has passed the promotion
    /// threshold. Speed rather than memory: what argues for it is the
    /// indirection on every read.
    DetailPromotion,
    /// Dead slots an adaptive surface's edits left in its pools.
    SlotPoolCompaction,
    /// Normals a drag deferred.
    ///
    /// **The one item that is not optional.** The committed state has to be
    /// exact, so a host that never services the queue must still flush these
    /// at stroke end. It is here so a host can spend its budget on them
    /// *first*, not so it can decide whether to do them at all.
    NormalFlush,
    /// A kind this build does not know, carried verbatim.
    ///
    /// Never mapped onto one this build does know: the engine refuses a kind
    /// outside its own list for the same reason, because a clamp would queue
    /// an index rebuild for a caller that asked for something else and the
    /// host would service it without ever learning it had been misheard.
    Unknown(i32),
}

impl MaintenanceKind {
    pub const ALL: [MaintenanceKind; 5] = [
        Self::IndexRebuild,
        Self::ChunkCompaction,
        Self::DetailPromotion,
        Self::SlotPoolCompaction,
        Self::NormalFlush,
    ];

    fn to_raw(self) -> i32 {
        use sys::clay_maintenance_kind as k;
        (match self {
            Self::IndexRebuild => k::CLAY_MAINTENANCE_INDEX_REBUILD,
            Self::ChunkCompaction => k::CLAY_MAINTENANCE_CHUNK_COMPACTION,
            Self::DetailPromotion => k::CLAY_MAINTENANCE_DETAIL_PROMOTION,
            Self::SlotPoolCompaction => k::CLAY_MAINTENANCE_SLOT_POOL_COMPACTION,
            Self::NormalFlush => k::CLAY_MAINTENANCE_NORMAL_FLUSH,
            Self::Unknown(other) => return other,
        }) as i32
    }

    fn from_raw(code: i32) -> Self {
        use sys::clay_maintenance_kind as k;
        match code as sys::clay_maintenance_kind::Type {
            k::CLAY_MAINTENANCE_INDEX_REBUILD => Self::IndexRebuild,
            k::CLAY_MAINTENANCE_CHUNK_COMPACTION => Self::ChunkCompaction,
            k::CLAY_MAINTENANCE_DETAIL_PROMOTION => Self::DetailPromotion,
            k::CLAY_MAINTENANCE_SLOT_POOL_COMPACTION => Self::SlotPoolCompaction,
            k::CLAY_MAINTENANCE_NORMAL_FLUSH => Self::NormalFlush,
            _ => Self::Unknown(code),
        }
    }

    /// The engine's own name for it, so a host's diagnostics and the engine's
    /// use one vocabulary. Never empty, including for a kind this build does
    /// not know.
    pub fn text(self) -> &'static str {
        // SAFETY: a total function over a plain integer, documented as never
        // returning NULL even for a value the library does not know.
        engine_text(unsafe { sys::clay_maintenance_kind_text(self.to_raw()) })
    }
}

impl std::fmt::Display for MaintenanceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.text())
    }
}

/// One queued request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaintenanceItem {
    pub kind: MaintenanceKind,
    /// What the item is about: a hierarchy level, a chunk, a surface id.
    ///
    /// The queue never interprets it — it is what makes two requests the
    /// *same* request rather than two entries for the same job.
    pub target: u32,
    /// How many times this item has been re-requested since it was last
    /// serviced. An entry whose count keeps climbing is one the host is
    /// starving, and that is worth being able to see.
    pub requests: u32,
    /// The requester's own estimate, or zero for "unknown" — which is what
    /// most callers honestly have. A host that has measured its own device
    /// replaces it by requesting the same item again with a figure of its own.
    pub estimated_micros: u64,
}

impl MaintenanceItem {
    fn from_raw(raw: sys::clay_maintenance_item) -> Self {
        Self {
            kind: MaintenanceKind::from_raw(raw.kind),
            target: raw.target,
            requests: raw.requests,
            estimated_micros: raw.estimated_micros,
        }
    }
}

/// A host's queue of work that is not required for correctness.
///
/// Owns nothing but its own entries and refers to no surface, so one queue may
/// collect the items of every surface a document holds — which is what a host
/// with a hierarchy, an adaptive surface and a fixed mesh open at once
/// actually has.
pub struct MaintenanceQueue {
    raw: NonNull<sys::clay_maintenance_queue>,
}

impl MaintenanceQueue {
    pub fn new() -> Result<Self> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: a valid out-parameter, written only on success.
        check(
            unsafe { sys::clay_maintenance_queue_create(&mut raw) },
            "clay_maintenance_queue_create",
        )?;
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_maintenance_queue_create", ErrorKind::Backend))
    }

    /// Queues an item, or folds it into the identical one already queued —
    /// same kind, same target — bumping that entry's
    /// [`requests`](MaintenanceItem::requests) and taking the latest non-zero
    /// estimate.
    ///
    /// Never allocates once the queue has reached its working size, which is
    /// what makes it safe to call from a stamp: a stroke requests the same
    /// rebuild on every dab.
    ///
    /// `estimated_micros` of zero means "unknown", which is what most callers
    /// honestly have.
    pub fn request(
        &mut self,
        kind: MaintenanceKind,
        target: u32,
        estimated_micros: u64,
    ) -> Result<()> {
        // SAFETY: an owned queue handle. A kind outside the engine's own list
        // is refused by it rather than clamped, which is why
        // `MaintenanceKind::Unknown` passes its number through unchanged.
        check(
            unsafe {
                sys::clay_maintenance_queue_request(
                    self.raw.as_ptr(),
                    kind.to_raw(),
                    target,
                    estimated_micros,
                )
            },
            "clay_maintenance_queue_request",
        )
    }

    /// How many entries are queued.
    pub fn len(&self) -> Result<usize> {
        let mut count = 0;
        // SAFETY: an owned queue handle and a valid out-parameter.
        check(
            unsafe { sys::clay_maintenance_queue_count(self.raw.as_ptr(), &mut count) },
            "clay_maintenance_queue_count",
        )?;
        Ok(count)
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// The item at `index`, in queue order, without removing it.
    ///
    /// Indexed rather than filled in bulk because the queue holds a handful of
    /// entries by construction — five kinds and a target apiece.
    pub fn item(&self, index: usize) -> Result<MaintenanceItem> {
        let mut raw = sys::clay_maintenance_item::sized();
        // SAFETY: an owned queue handle and a versioned out-descriptor whose
        // struct_size is set above. An index past the end is refused by the
        // engine rather than read.
        check(
            unsafe { sys::clay_maintenance_queue_item(self.raw.as_ptr(), index, &mut raw) },
            "clay_maintenance_queue_item",
        )?;
        Ok(MaintenanceItem::from_raw(raw))
    }

    /// Every queued item, in queue order.
    pub fn items(&self) -> Result<Vec<MaintenanceItem>> {
        (0..self.len()?).map(|i| self.item(i)).collect()
    }

    /// Whether this exact request — same kind, same target — is queued.
    pub fn has(&self, kind: MaintenanceKind, target: u32) -> Result<bool> {
        let mut has = 0;
        // SAFETY: an owned queue handle and a valid out-parameter.
        check(
            unsafe {
                sys::clay_maintenance_queue_has(self.raw.as_ptr(), kind.to_raw(), target, &mut has)
            },
            "clay_maintenance_queue_has",
        )?;
        Ok(has != 0)
    }

    /// Opens a stroke, and shuts it when the returned guard is dropped.
    ///
    /// The gate: a pointer event is not a maintenance window. While the guard
    /// exists, [`take_next`](Self::take_next) answers `None` and
    /// [`complete`](Self::complete) refuses, because an item completed
    /// mid-stroke would have been *performed* mid-stroke.
    ///
    /// The guard derefs to the queue, so a stamp inside the stroke can still
    /// [`request`](Self::request) — which is the call the gate is built to keep
    /// cheap.
    pub fn stroke(&mut self) -> Result<StrokeGuard<'_>> {
        // SAFETY: an owned queue handle.
        check(
            unsafe { sys::clay_maintenance_queue_begin_stroke(self.raw.as_ptr()) },
            "clay_maintenance_queue_begin_stroke",
        )?;
        Ok(StrokeGuard { queue: self })
    }

    /// Whether a stroke is open.
    pub fn in_stroke(&self) -> Result<bool> {
        let mut open = 0;
        // SAFETY: an owned queue handle and a valid out-parameter.
        check(
            unsafe { sys::clay_maintenance_queue_in_stroke(self.raw.as_ptr(), &mut open) },
            "clay_maintenance_queue_in_stroke",
        )?;
        Ok(open != 0)
    }

    /// The next item a host may do now, left queued until
    /// [`complete`](Self::complete) says it was done.
    ///
    /// `None` while a stroke is open and `None` while the queue is empty — the
    /// same answer to "is there work I may do now", which is the question a
    /// drain is actually asking. A host that took one and then found it could
    /// not afford it has declined rather than dropped it: the entry is still
    /// there next time.
    pub fn take_next(&mut self) -> Result<Option<MaintenanceItem>> {
        let mut raw = sys::clay_maintenance_item::sized();
        let mut have = 0;
        // SAFETY: an owned queue handle, a versioned out-descriptor whose
        // struct_size is set above, and a valid flag out-parameter. The
        // descriptor is only read where the flag says it was filled.
        check(
            unsafe {
                sys::clay_maintenance_queue_take_next(self.raw.as_ptr(), &mut raw, &mut have)
            },
            "clay_maintenance_queue_take_next",
        )?;
        Ok((have != 0).then(|| MaintenanceItem::from_raw(raw)))
    }

    /// Removes the item naming that kind and target, and reports whether one
    /// was there.
    ///
    /// Gated like [`take_next`](Self::take_next): an item completed mid-stroke
    /// would have been performed mid-stroke.
    pub fn complete(&mut self, kind: MaintenanceKind, target: u32) -> Result<bool> {
        let mut completed = 0;
        // SAFETY: an owned queue handle and a valid out-parameter.
        check(
            unsafe {
                sys::clay_maintenance_queue_complete(
                    self.raw.as_ptr(),
                    kind.to_raw(),
                    target,
                    &mut completed,
                )
            },
            "clay_maintenance_queue_complete",
        )?;
        Ok(completed != 0)
    }

    /// Drops every entry.
    ///
    /// Correct at any moment, because none of what is queued was correctness:
    /// a host closing a document throws the work away rather than servicing it.
    pub fn clear(&mut self) -> Result<()> {
        // SAFETY: an owned queue handle.
        check(
            unsafe { sys::clay_maintenance_queue_clear(self.raw.as_ptr()) },
            "clay_maintenance_queue_clear",
        )
    }

    /// What the queue itself costs, for the memory report.
    pub fn bytes(&self) -> Result<usize> {
        let mut bytes = 0;
        // SAFETY: an owned queue handle and a valid out-parameter.
        check(
            unsafe { sys::clay_maintenance_queue_bytes(self.raw.as_ptr(), &mut bytes) },
            "clay_maintenance_queue_bytes",
        )?;
        Ok(bytes)
    }
}

impl Drop for MaintenanceQueue {
    fn drop(&mut self) {
        // SAFETY: an owned handle, released exactly once. It refers to no
        // surface, so this frees entries and nothing else.
        unsafe { sys::clay_maintenance_queue_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for MaintenanceQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaintenanceQueue")
            .field("queued", &self.len().unwrap_or(0))
            .field("in_stroke", &self.in_stroke().unwrap_or(false))
            .finish()
    }
}

/// An open stroke, closed when this is dropped.
///
/// The gate expressed as a scope. A host that opened a stroke and returned
/// early — a refused tool, a lost pointer release, a panic unwinding through
/// the stamp loop — would otherwise leave the queue shut forever, and the
/// symptom is maintenance that silently never runs again, which is exactly the
/// failure the gate exists to make loud.
pub struct StrokeGuard<'a> {
    queue: &'a mut MaintenanceQueue,
}

impl std::ops::Deref for StrokeGuard<'_> {
    type Target = MaintenanceQueue;

    fn deref(&self) -> &MaintenanceQueue {
        self.queue
    }
}

impl std::ops::DerefMut for StrokeGuard<'_> {
    /// The queue itself, so a stamp inside the stroke can queue the rebuild it
    /// just made necessary. That call is the reason the fold exists.
    fn deref_mut(&mut self) -> &mut MaintenanceQueue {
        self.queue
    }
}

impl Drop for StrokeGuard<'_> {
    fn drop(&mut self) {
        // A failure here has nothing left to do about it, and a panic in a
        // drop while unwinding aborts the process. The only way this refuses
        // is a null handle, which the type system already rules out.
        // SAFETY: an owned queue handle, valid for the guard's lifetime.
        let _ = unsafe { sys::clay_maintenance_queue_end_stroke(self.queue.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for StrokeGuard<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrokeGuard")
            .field("queued", &self.queue.len().unwrap_or(0))
            .finish()
    }
}
