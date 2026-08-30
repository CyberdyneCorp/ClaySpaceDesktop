//! What each render pass actually costs the GPU.
//!
//! CPU time around `begin_render_pass` measures *submission*: the encoder
//! records commands and returns long before the device has drawn anything. The
//! only honest figure comes from the device's own clock, which WebGPU exposes
//! as timestamp queries written at the boundaries of a pass.
//!
//! Timestamps are an optional feature, and this module is diagnostics. A
//! device without them renders exactly as it otherwise would and reports that
//! the timing is unavailable — never a requirement, never a reason to refuse a
//! frame.
//!
//! The read is one frame behind on purpose. Resolving a query set and mapping
//! the result in the same frame means waiting for the device to finish, which
//! would make measuring the frame the thing that slows it down.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use crate::gpu::Gpu;

/// A pass worth attributing time to.
///
/// The overlays are not on this list because they are not a pass: the grid,
/// the cursor, the rig and the manipulator are drawn inside the scene pass, so
/// the device has no boundary to timestamp between them and the sculpt.
/// Splitting the pass to measure it would change what is being measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuPass {
    /// The sculpt, the mesh layers and every overlay drawn with them.
    Scene,
    /// Full-resolution multisampled depth down to the occlusion resolution.
    DepthReduce,
    /// The occlusion kernel.
    Ao,
    /// The upsample and multiply onto the resolved colour.
    AoComposite,
}

impl GpuPass {
    pub const ALL: [GpuPass; 4] = [Self::Scene, Self::DepthReduce, Self::Ao, Self::AoComposite];

    pub fn label(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::DepthReduce => "depth reduce",
            Self::Ao => "ao",
            Self::AoComposite => "ao composite",
        }
    }

    /// Where this pass's pair of timestamps sits in the query set.
    fn slot(self) -> u32 {
        match self {
            Self::Scene => 0,
            Self::DepthReduce => 1,
            Self::Ao => 2,
            Self::AoComposite => 3,
        }
    }
}

/// How long each pass took, in milliseconds.
///
/// A pass that did not run in the measured frame is absent rather than zero:
/// occlusion switched off and occlusion costing nothing are different claims.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GpuFrameTiming {
    passes: [Option<f32>; GpuPass::ALL.len()],
}

impl GpuFrameTiming {
    pub fn get(&self, pass: GpuPass) -> Option<f32> {
        self.passes[pass.slot() as usize]
    }

    /// The passes that ran, summed. Not the whole frame — the interface is
    /// drawn afterwards, by egui, into a pass this renderer does not own.
    pub fn total(&self) -> f32 {
        self.passes.iter().flatten().sum()
    }

    /// Every pass that ran, in the order they run.
    pub fn measured(&self) -> impl Iterator<Item = (GpuPass, f32)> + '_ {
        GpuPass::ALL
            .into_iter()
            .filter_map(|pass| self.get(pass).map(|ms| (pass, ms)))
    }
}

/// Two timestamps per pass: one at the start, one at the end.
const STAMPS_PER_PASS: u32 = 2;

/// Where one pass's pair of timestamps lands in the resolve buffer.
///
/// A whole 256 bytes for sixteen bytes of result, because that is the
/// alignment `resolve_query_set` requires of its destination. The buffer is a
/// kilobyte; the alternative is arithmetic that is wrong on one backend.
const SLOT_STRIDE: u64 = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
const RESULT_BYTES: u64 = GpuPass::ALL.len() as u64 * SLOT_STRIDE;

/// Records per-pass GPU time where the device will report it.
pub enum GpuProfiler {
    /// The adapter has no timestamp queries. Every frame draws as it would
    /// otherwise; nothing is measured.
    Unsupported,
    Measuring(Measurements),
}

pub struct Measurements {
    /// One query set per pass rather than one set with a pair of slots each.
    ///
    /// This is not tidiness. Resolving a query that was never written is a
    /// wait on a result that will never become available, and a device asked
    /// for one stops answering: the frame never completes, and sixty seconds
    /// later the driver gives up. A pass does not always run — occlusion can
    /// be switched off, and every capture that compares the two switches it —
    /// so with one shared set every such frame would resolve six unwritten
    /// queries and hang.
    ///
    /// A set can only be resolved whole, and `resolve_query_set` writes at a
    /// 256-byte alignment, so "resolve just the pairs that ran" means a set
    /// per pair. Query sets are cheap; a hung device is not.
    queries: Vec<wgpu::QuerySet>,
    /// Where `resolve_query_set` writes. Not mappable, which is why there are
    /// two buffers.
    resolved: wgpu::Buffer,
    readback: wgpu::Buffer,
    /// Nanoseconds per tick of the device's clock.
    period: f32,
    /// Which passes wrote timestamps in the frame being encoded.
    encoding: Vec<GpuPass>,
    /// Which passes wrote timestamps in the frame whose result is in flight.
    in_flight: Option<Vec<GpuPass>>,
    /// Whether a map has been asked for and not yet answered.
    ///
    /// Separate from `in_flight`, and the separation is the whole point: a
    /// result stays in flight across as many frames as the device takes to
    /// finish it, and asking to map a buffer that is already mapped is a panic
    /// in wgpu rather than a no-op. One map is requested per resolve, and the
    /// frames in between ask for nothing.
    awaiting_map: bool,
    /// What the map callback reported. See [`MapState`].
    map_state: Arc<AtomicU8>,
    latest: Option<GpuFrameTiming>,
}

/// The state of the outstanding readback map, shared with wgpu's callback.
///
/// An integer rather than a `bool` because a failed map and a pending one are
/// different things: a pending one is waited for, and a failed one has to be
/// abandoned or the profiler stops measuring for the rest of the session.
mod map_state {
    pub const PENDING: u8 = 0;
    pub const READY: u8 = 1;
    pub const FAILED: u8 = 2;
}

impl GpuProfiler {
    pub fn new(gpu: &Gpu) -> Self {
        if !gpu.supports_timestamps() {
            return Self::Unsupported;
        }
        Self::Measuring(Measurements {
            queries: GpuPass::ALL
                .iter()
                .map(|pass| {
                    gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
                        label: Some(pass.label()),
                        ty: wgpu::QueryType::Timestamp,
                        count: STAMPS_PER_PASS,
                    })
                })
                .collect(),
            resolved: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu passes resolved"),
                size: RESULT_BYTES,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu passes readback"),
                size: RESULT_BYTES,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            period: gpu.queue.get_timestamp_period(),
            encoding: Vec::new(),
            in_flight: None,
            awaiting_map: false,
            map_state: Arc::new(AtomicU8::new(map_state::PENDING)),
            latest: None,
        })
    }

    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Measuring(_))
    }

    /// Starts a frame, collecting whatever the previous one reported.
    ///
    /// Called before anything is encoded: the result being collected is the
    /// one for a frame the device has already finished, so nothing waits.
    pub fn begin_frame(&mut self, gpu: &Gpu) {
        let Self::Measuring(state) = self else {
            return;
        };
        state.collect(gpu);
        state.encoding.clear();
    }

    /// The timestamp writes to hand a render pass, when it is being measured.
    ///
    /// Returns `None` on a device without timestamps, which is exactly what
    /// `RenderPassDescriptor::timestamp_writes` wants for "do not measure".
    pub fn writes(&mut self, pass: GpuPass) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let Self::Measuring(state) = self else {
            return None;
        };
        // A pass encoded twice in one frame would overwrite its own start, so
        // the second one is left unmeasured rather than reported wrong.
        if state.encoding.contains(&pass) {
            return None;
        }
        state.encoding.push(pass);
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &state.queries[pass.slot() as usize],
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        })
    }

    /// Resolves this frame's queries into the readback buffer.
    ///
    /// Encoded after the last measured pass and before the encoder is
    /// finished; the map is requested once the frame has been submitted. Only
    /// the passes that actually ran are resolved — see `queries`.
    pub fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let Self::Measuring(state) = self else {
            return;
        };
        if state.encoding.is_empty() || state.in_flight.is_some() {
            return;
        }
        for pass in &state.encoding {
            let slot = pass.slot() as usize;
            encoder.resolve_query_set(
                &state.queries[slot],
                0..STAMPS_PER_PASS,
                &state.resolved,
                slot as u64 * SLOT_STRIDE,
            );
        }
        encoder.copy_buffer_to_buffer(&state.resolved, 0, &state.readback, 0, RESULT_BYTES);
        state.in_flight = Some(state.encoding.clone());
        state.awaiting_map = true;
        state.map_state.store(map_state::PENDING, Ordering::Release);
    }

    /// Asks for the resolved timestamps, after the frame has been submitted.
    pub fn after_submit(&mut self) {
        let Self::Measuring(state) = self else {
            return;
        };
        // Once per resolve, not once per frame. A result the device has not
        // finished stays in flight over several frames, and mapping a buffer
        // that is already mapped is a panic in wgpu — which is how this was
        // found, in the one test that presents real frames rather than reading
        // an offscreen target back and waiting for the device each time.
        if !state.awaiting_map {
            return;
        }
        state.awaiting_map = false;
        let outcome = Arc::clone(&state.map_state);
        state
            .readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let value = if result.is_ok() {
                    map_state::READY
                } else {
                    map_state::FAILED
                };
                outcome.store(value, Ordering::Release);
            });
    }

    /// The most recent frame the device reported, if any has been.
    pub fn latest(&self) -> Option<GpuFrameTiming> {
        match self {
            Self::Unsupported => None,
            Self::Measuring(state) => state.latest,
        }
    }
}

impl Measurements {
    fn collect(&mut self, gpu: &Gpu) {
        let Some(measured) = self.in_flight.clone() else {
            return;
        };
        // Callbacks fire on a poll, and nothing else in a frame polls without
        // also waiting. This one does not wait: if the device has not finished,
        // the result is collected on a later frame instead.
        let _ = gpu.device.poll(wgpu::Maintain::Poll);
        match self.map_state.swap(map_state::PENDING, Ordering::AcqRel) {
            map_state::READY => {}
            // A map that failed leaves the buffer unmapped and nothing to
            // read. Abandoning the result frees the profiler to measure the
            // next frame; keeping it would stop it measuring anything again.
            map_state::FAILED => {
                self.in_flight = None;
                return;
            }
            _ => return,
        }

        let bytes: Vec<u8> = self.readback.slice(..).get_mapped_range().to_vec();
        self.readback.unmap();
        self.in_flight = None;

        let mut timing = GpuFrameTiming::default();
        for pass in measured {
            let base = pass.slot() as usize * SLOT_STRIDE as usize;
            let tick =
                |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"));
            let (start, end) = (tick(base), tick(base + 8));
            // A device may report equal or decreasing timestamps across a
            // boundary it did not actually order; a negative duration is not a
            // measurement, so it is dropped rather than reported as zero.
            if end <= start {
                continue;
            }
            let nanoseconds = (end - start) as f64 * self.period as f64;
            timing.passes[pass.slot() as usize] = Some((nanoseconds / 1.0e6) as f32);
        }
        self.latest = Some(timing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A timing reports what ran, and says nothing about what did not.
    #[test]
    fn an_absent_pass_is_absent_rather_than_zero() {
        let mut timing = GpuFrameTiming::default();
        timing.passes[GpuPass::Scene.slot() as usize] = Some(2.5);
        timing.passes[GpuPass::Ao.slot() as usize] = Some(1.5);

        assert_eq!(timing.get(GpuPass::Scene), Some(2.5));
        assert_eq!(
            timing.get(GpuPass::AoComposite),
            None,
            "a pass that did not run has no time, which is not the same as no time taken"
        );
        assert_eq!(timing.total(), 4.0);
        assert_eq!(
            timing.measured().collect::<Vec<_>>(),
            vec![(GpuPass::Scene, 2.5), (GpuPass::Ao, 1.5)]
        );
    }

    /// Every pass owns its own query set and its own slice of the resolve
    /// buffer, or two passes would overwrite each other's timestamps and both
    /// would be reported wrong.
    #[test]
    fn every_pass_has_its_own_place_to_be_resolved_into() {
        let mut slots: Vec<u32> = GpuPass::ALL.iter().map(|pass| pass.slot()).collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), GpuPass::ALL.len());
        assert!(slots
            .iter()
            .all(|slot| (*slot as u64 + 1) * SLOT_STRIDE <= RESULT_BYTES));
    }

    /// The destination of a resolve has to sit on the alignment the backend
    /// demands, or the call is a validation error on the device it is least
    /// convenient to discover on.
    #[test]
    fn each_slot_is_aligned_for_a_resolve() {
        assert_eq!(SLOT_STRIDE % wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT, 0);
        assert!(SLOT_STRIDE >= STAMPS_PER_PASS as u64 * 8);
    }
}
