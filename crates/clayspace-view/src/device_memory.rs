//! What the graphics device is holding on this application's behalf.
//!
//! The engine can account for its own containers and for the surfaces a host
//! hands it, and nothing past them. Everything the viewport allocates to *draw*
//! the document — vertex and index buffers, the staging wgpu takes to carry a
//! write, the targets a frame or a capture is rendered into — is invisible from
//! there, and it was the largest part of the process: an audited session
//! reported 13 MB in use against a 26 GB footprint, almost all of it this.
//!
//! So the allocations count themselves. A [`Resident`] is taken beside every
//! buffer or texture this crate creates for geometry or a render target and
//! gives its bytes back when it is dropped, which it is exactly when the thing
//! it stands for is: a buffer replaced by a larger one, a framebuffer rebuilt
//! on a resize, a capture target thrown away after one frame. A figure kept
//! that way cannot drift from the allocations, where one recomputed from
//! capacities somewhere else could miss the next allocation site added.
//!
//! These are the sizes *asked for*. A driver rounds them up and keeps its own
//! bookkeeping beside them, so the device charges the process somewhat more
//! than this — a floor, the same kind of figure the engine's ledger is.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// What the device holds for this application, by the kind of allocation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeviceMemory {
    /// Vertex and index buffers, at their capacity rather than at what is
    /// drawn from them: a buffer kept after the surface shrank is still held.
    pub buffers: u64,
    /// Staging wgpu took to carry writes the device may not have finished
    /// with — this frame's so far, and the last submitted frame's.
    pub staging: u64,
    /// Render targets: the window's framebuffer, the studio shadow map once
    /// it has been asked for, and every capture target while it is alive.
    pub targets: u64,
}

impl DeviceMemory {
    pub fn total(&self) -> u64 {
        self.buffers
            .saturating_add(self.staging)
            .saturating_add(self.targets)
    }
}

/// The shared counters behind [`DeviceMemory`], one per device.
#[derive(Debug, Default)]
pub(crate) struct DeviceLedger {
    buffers: AtomicU64,
    targets: AtomicU64,
    /// Written since the last frame's poll, and so certainly still held.
    staging_open: AtomicU64,
    /// Written in the frame the last poll followed.
    ///
    /// Still counted after that poll, because a poll collects what has
    /// *finished* and the submission that frame made has usually not: the
    /// device runs a frame behind. It is the poll after next that sees it done.
    staging_submitted: AtomicU64,
}

impl DeviceLedger {
    pub(crate) fn read(&self) -> DeviceMemory {
        DeviceMemory {
            buffers: self.buffers.load(Ordering::Relaxed),
            staging: self
                .staging_open
                .load(Ordering::Relaxed)
                .saturating_add(self.staging_submitted.load(Ordering::Relaxed)),
            targets: self.targets.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn note_staging(&self, bytes: u64) {
        self.staging_open.fetch_add(bytes, Ordering::Relaxed);
    }

    /// A frame was submitted and the device polled without waiting.
    pub(crate) fn frame_polled(&self) {
        let open = self.staging_open.swap(0, Ordering::Relaxed);
        self.staging_submitted.store(open, Ordering::Relaxed);
    }

    /// The device was waited on until idle, so nothing it was given is held.
    pub(crate) fn device_idle(&self) {
        self.staging_open.store(0, Ordering::Relaxed);
        self.staging_submitted.store(0, Ordering::Relaxed);
    }

    fn counter(&self, kind: Residency) -> &AtomicU64 {
        match kind {
            Residency::Buffer => &self.buffers,
            Residency::Target => &self.targets,
        }
    }
}

/// Which figure an allocation is counted under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Residency {
    Buffer,
    Target,
}

/// An allocation's bytes, counted for as long as this is alive.
///
/// Held beside the buffer or texture it stands for, so the two are dropped
/// together and the figure cannot outlive the allocation or miss it.
#[derive(Debug)]
pub(crate) struct Resident {
    ledger: Arc<DeviceLedger>,
    kind: Residency,
    bytes: u64,
}

impl Resident {
    pub(crate) fn new(ledger: &Arc<DeviceLedger>, kind: Residency, bytes: u64) -> Self {
        ledger.counter(kind).fetch_add(bytes, Ordering::Relaxed);
        Self {
            ledger: Arc::clone(ledger),
            kind,
            bytes,
        }
    }
}

impl Drop for Resident {
    fn drop(&mut self) {
        self.ledger
            .counter(self.kind)
            .fetch_sub(self.bytes, Ordering::Relaxed);
    }
}

/// What one texture of this size and format occupies, as asked for.
///
/// Four bytes a texel where the format does not say — every format this crate
/// renders into is four bytes or fewer, so that errs toward counting.
pub(crate) fn texture_bytes(
    width: u32,
    height: u32,
    samples: u32,
    format: wgpu::TextureFormat,
) -> u64 {
    let texel = format.block_copy_size(None).unwrap_or(4) as u64;
    u64::from(width) * u64::from(height) * u64::from(samples.max(1)) * texel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resident_gives_its_bytes_back_when_dropped() {
        let ledger = Arc::new(DeviceLedger::default());
        let buffer = Resident::new(&ledger, Residency::Buffer, 1000);
        let target = Resident::new(&ledger, Residency::Target, 24);
        assert_eq!(
            ledger.read(),
            DeviceMemory {
                buffers: 1000,
                staging: 0,
                targets: 24
            }
        );
        drop(buffer);
        assert_eq!(ledger.read().buffers, 0);
        drop(target);
        assert_eq!(ledger.read().total(), 0);
    }

    /// A frame's writes are held until the poll *after* the one that follows
    /// their submission, because the device runs a frame behind.
    #[test]
    fn staging_is_counted_until_the_device_can_have_finished_with_it() {
        let ledger = DeviceLedger::default();
        ledger.note_staging(64);
        assert_eq!(ledger.read().staging, 64);
        ledger.frame_polled();
        assert_eq!(
            ledger.read().staging,
            64,
            "the submission is still in flight"
        );
        ledger.note_staging(8);
        assert_eq!(ledger.read().staging, 72);
        ledger.frame_polled();
        assert_eq!(
            ledger.read().staging,
            8,
            "the older frame has been collected"
        );
        ledger.device_idle();
        assert_eq!(ledger.read().staging, 0);
    }

    #[test]
    fn a_texture_counts_every_sample() {
        let colour = texture_bytes(10, 10, 4, wgpu::TextureFormat::Rgba8UnormSrgb);
        assert_eq!(colour, 10 * 10 * 4 * 4);
        assert_eq!(texture_bytes(10, 10, 1, wgpu::TextureFormat::R8Unorm), 100);
    }
}
