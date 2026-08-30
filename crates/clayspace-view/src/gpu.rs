//! The WebGPU device, and the surfaces things are drawn into.
//!
//! Headless first: the renderer draws into a texture view, and a window is one
//! way to obtain one. That is what lets every visual test render and compare a
//! real image in CI, with no display attached.
//!
//! This device is chosen independently of the engine's evaluation backend. A
//! software rendering adapter does not stop the engine using a GPU, and a
//! missing GPU backend does not stop rendering.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A WebGPU device and its queue.
#[derive(Clone)]
pub struct Gpu {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    /// Bytes written to buffers and textures since the counter was last read.
    ///
    /// Here rather than on the renderer because the writes are made from both
    /// sides of that boundary — the renderer's own meshes and the composition
    /// root's incremental patches — and a figure that counted only one of them
    /// would answer the wrong question. Shared with every clone of this handle,
    /// which is what makes it the *device's* upload traffic rather than one
    /// caller's.
    uploaded: Arc<AtomicU64>,
    adapter: Arc<wgpu::Adapter>,
    /// How much multisampling this device draws the scene with. See
    /// [`Gpu::msaa`] for why it does not change.
    msaa: MsaaQuality,
    /// The instance this device came from.
    ///
    /// A surface is an entry in the registry of the instance that created it,
    /// so a device from a *different* instance cannot present it — the frame
    /// panics with `Surface does not exist`. Threading one instance through
    /// device and surface alike makes that unrepresentable rather than a rule
    /// someone has to remember.
    instance: Arc<wgpu::Instance>,
}

impl Gpu {
    /// Creates a device with no surface, for offscreen rendering and tests.
    pub async fn headless() -> Result<Self, GpuError> {
        Self::create(Arc::new(Self::new_instance()), None).await
    }

    /// The instance every device and surface in one session shares.
    pub(crate) fn new_instance() -> wgpu::Instance {
        wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        })
    }

    /// Creates a device able to present to `surface`.
    ///
    /// The surface must have been created from `instance`, and both are kept
    /// alive by the returned `Gpu`.
    pub(crate) async fn create(
        instance: Arc<wgpu::Instance>,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, GpuError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface,
                // A software adapter still renders correctly, just slowly, and
                // is better than refusing to start.
                force_fallback_adapter: false,
            })
            .await
            .ok_or(GpuError::NoAdapter)?;

        // The buffer ceiling the adapter actually has, not the downlevel
        // default of 256 MB: a subtool scaled up a few times is a surface of
        // ten million vertices at the field's fixed resolution, and the
        // default ceiling turned that into a validation panic in
        // `create_buffer`. A desktop adapter allows gigabytes; ask for them.
        let limits = wgpu::Limits {
            max_buffer_size: adapter.limits().max_buffer_size,
            ..wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits())
        };
        // Timestamp queries where the adapter has them, and nothing else.
        // They are what makes per-pass GPU time measurable at all; asking for
        // them where they do not exist would refuse the device over
        // diagnostics, so the feature is requested only from the intersection
        // of what is wanted and what is there.
        let features = adapter.features() & wgpu::Features::TIMESTAMP_QUERY;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("clayspace"),
                    required_features: features,
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| GpuError::NoDevice(e.to_string()))?;
        // A validation error is reported, not fatal. wgpu's default handler
        // panics the process, which turned an oversized buffer into a lost
        // session; a frame drawn wrong is recoverable, a crash is not.
        device.on_uncaptured_error(Box::new(|error| {
            eprintln!("graphics error: {error}");
        }));

        Ok(Self {
            msaa: MsaaQuality::for_adapter(&adapter),
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
            uploaded: Arc::new(AtomicU64::new(0)),
            instance,
        })
    }

    /// The largest buffer this device will be asked for, in bytes.
    ///
    /// The device's own ceiling, capped at [`Gpu::BUFFER_CAP`]. Past the
    /// device's limit `create_buffer` is a validation error; past what the
    /// card actually has it is an allocation failure the adapter reports as
    /// "unlimited" until it fails — this machine's says `u64::MAX`. Either
    /// way a geometry over the figure is refused here and drawn coarser
    /// rather than attempted.
    pub fn max_buffer_size(&self) -> u64 {
        self.device.limits().max_buffer_size.min(Self::BUFFER_CAP)
    }

    /// Two gibibytes: more than any surface this application should be
    /// drawing at full detail, and under what a desktop card can allocate in
    /// one piece. A surface past it is a scaled-up subtool, and the coarse
    /// level is the right picture of one.
    pub const BUFFER_CAP: u64 = 2 << 30;

    /// The instance this device came from.
    ///
    /// Surfaces must be created from it, and must not outlive it.
    pub fn instance(&self) -> &Arc<wgpu::Instance> {
        &self.instance
    }

    /// How many samples per pixel to draw the scene with.
    ///
    /// The quality this device chose, resolved to what the format will
    /// actually take — asked rather than assumed, because a sample count the
    /// format does not support is a validation error at pipeline creation and
    /// there is no reason to take the window down over an edge a fallback
    /// covers.
    ///
    /// The scene is what this is for. The interface is drawn by egui straight
    /// into the resolved target afterwards: text and panel edges are already
    /// laid out on the pixel grid, so multisampling them would cost fill rate
    /// to change nothing.
    pub fn sample_count(&self, format: wgpu::TextureFormat) -> u32 {
        self.msaa.supported_on(self.adapter.as_ref(), format)
    }

    /// The multisampling this device is drawing at.
    ///
    /// Fixed for the life of the device. Changing it means rebuilding both the
    /// framebuffer and every pipeline — a pipeline's sample count is part of
    /// its state, and one that disagrees with its attachment is a validation
    /// error at draw time — so it is chosen once, from what the adapter is,
    /// rather than toggled.
    pub fn msaa(&self) -> MsaaQuality {
        self.msaa
    }

    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// Records bytes written to the device, for the diagnostics view and the
    /// render benchmarks.
    ///
    /// Called by whatever does the writing rather than wrapped around
    /// `write_buffer`: the queue is public, and a wrapper that could be
    /// bypassed would under-report exactly the paths worth watching.
    pub fn note_upload(&self, bytes: u64) {
        self.uploaded.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Bytes uploaded since this was last called, and resets the count.
    pub fn take_uploaded_bytes(&self) -> u64 {
        self.uploaded.swap(0, Ordering::Relaxed)
    }

    /// Bytes uploaded so far, without resetting.
    pub fn uploaded_bytes(&self) -> u64 {
        self.uploaded.load(Ordering::Relaxed)
    }

    /// How many anisotropic samples a texture filter may take.
    ///
    /// Sixteen where the device allows it, one where it does not. Asked rather
    /// than assumed for the reason [`Gpu::sample_count`] is asked: a value the
    /// device will not take is a validation error at sampler creation, and
    /// there is no reason to refuse to start over a filtering nicety.
    pub fn max_anisotropy(&self) -> u16 {
        const WANTED: u16 = 16;
        // The downlevel flag is what says the backend honours it at all; the
        // limit is not exposed separately, and 16 is the ceiling every desktop
        // backend that supports anisotropy at all provides.
        if self
            .adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING)
        {
            WANTED
        } else {
            1
        }
    }

    /// Whether the device will report its own clock at pass boundaries.
    ///
    /// Diagnostics only. A device without it draws every frame the same way
    /// and reports no timing; see [`crate::profiler`].
    pub fn supports_timestamps(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
    }

    /// Which adapter is rendering, for the diagnostics view.
    ///
    /// Distinct from the engine's evaluation backend: this is what draws, that
    /// is what evaluates, and they are chosen separately.
    pub fn adapter_description(&self) -> String {
        let info = self.adapter.get_info();
        format!("{} ({:?}, {:?})", info.name, info.device_type, info.backend)
    }

    /// Overrides the multisampling this device draws at.
    ///
    /// For the render benchmarks, which measure the same scene at more than
    /// one quality, and for a host that has a reason to override the adapter's
    /// default. Every framebuffer and every pipeline made before this call
    /// still carries the old count, and a pipeline whose count disagrees with
    /// its attachment is a validation error at draw time — so this is only
    /// sound before a renderer is built on the device, which is why it takes
    /// `&mut self` and why nothing calls it mid-session.
    pub fn set_msaa(&mut self, quality: MsaaQuality) {
        self.msaa = quality;
    }
}

impl std::fmt::Debug for Gpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gpu")
            .field("adapter", &self.adapter_description())
            .finish()
    }
}

/// How much multisampling the scene is drawn with.
///
/// The scene, and nothing else: the interface is painted into the resolved
/// target afterwards, and its edges are already on the pixel grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum MsaaQuality {
    /// No multisampling. Silhouettes are stair-stepped; a post-process pass is
    /// the fallback where this is chosen deliberately.
    Off,
    /// Half the fill cost of four, and most of the benefit on a panel that is
    /// already dense.
    X2,
    /// The default, and the usual sweet spot.
    #[default]
    X4,
    /// Rarely worth what it costs. Offered because some devices have it, not
    /// because it should be reached for.
    X8,
}

impl MsaaQuality {
    pub const ALL: [Self; 4] = [Self::Off, Self::X2, Self::X4, Self::X8];

    pub fn samples(self) -> u32 {
        match self {
            Self::Off => 1,
            Self::X2 => 2,
            Self::X4 => 4,
            Self::X8 => 8,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "sem",
            Self::X2 => "2×",
            Self::X4 => "4×",
            Self::X8 => "8×",
        }
    }

    /// What this adapter should draw at by default.
    ///
    /// Four on a discrete card, two on an integrated one or a software
    /// rasterizer. Multisampling is fill-rate, and fill-rate is the thing an
    /// integrated GPU has least of — the guidance this follows is that 4× is
    /// the sweet spot where there is headroom and 2× is where there is not.
    /// Eight is never chosen automatically: it costs twice what four does to
    /// move a silhouette by a fraction of a pixel.
    pub fn for_adapter(adapter: &wgpu::Adapter) -> Self {
        match adapter.get_info().device_type {
            wgpu::DeviceType::DiscreteGpu | wgpu::DeviceType::VirtualGpu => Self::X4,
            _ => Self::X2,
        }
    }

    /// This quality, or the best below it the format will take.
    ///
    /// Resolved downward rather than refused: a format that will not
    /// multisample at all still has to be drawn into.
    pub fn supported_on(self, adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> u32 {
        let flags = adapter.get_texture_format_features(format).flags;
        Self::ALL
            .into_iter()
            .rev()
            .filter(|quality| *quality <= self)
            .map(Self::samples)
            .find(|samples| *samples == 1 || flags.sample_count_supported(*samples))
            .unwrap_or(1)
    }
}

/// Why a device could not be created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuError {
    /// No adapter at all — not even a software one.
    NoAdapter,
    NoDevice(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAdapter => f.write_str("no graphics adapter is available"),
            Self::NoDevice(why) => write!(f, "the graphics device could not be created: {why}"),
        }
    }
}

impl std::error::Error for GpuError {}

/// A colour target and its depth buffer.
///
/// Owned by whatever is being drawn into — a window's swapchain image or an
/// offscreen texture — and recreated when the size changes.
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    /// What tells one framebuffer from the next.
    ///
    /// wgpu exposes no identity for a texture view, so anything caching a bind
    /// group over these views has no way to ask whether they are still the
    /// ones it built against. A counter answers it: a framebuffer is only ever
    /// replaced wholesale, on resize, so a cache that remembers which number it
    /// was built for knows exactly when it is stale.
    id: u64,
    depth: wgpu::TextureView,
    /// The multisampled colour target, resolved into the caller's view.
    ///
    /// `None` where the device will not multisample this format, in which case
    /// drawing goes straight to the caller's view as it always did.
    color: Option<wgpu::TextureView>,
    /// The scene's depth, reduced to one sample per pixel at the occlusion
    /// resolution.
    ///
    /// The pass between the scene and the occlusion kernel. It exists for two
    /// reasons at once: it is where the resolution drops, and it is what frees
    /// occlusion from multisampling — the kernel used to bind this
    /// framebuffer's depth buffer directly, which can only be done as
    /// `texture_depth_multisampled_2d`, so a device that would not multisample
    /// got no occlusion at all.
    reduced_depth: wgpu::TextureView,
    /// Single-channel occlusion, written from the reduced depth and multiplied
    /// onto the resolved colour.
    occlusion: wgpu::TextureView,
    /// The size of both of the above, in pixels.
    ao_width: u32,
    ao_height: u32,
    samples: u32,
}

/// The source of [`Framebuffer::id`]. Never reused: a wrapped counter would
/// hand a cache a stale entry that looks current.
static NEXT_FRAMEBUFFER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Framebuffer {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    /// How many display pixels across one occlusion pixel covers.
    ///
    /// Two. Occlusion is a low-frequency term and the kernel is the expensive
    /// part of it, so running the kernel at half resolution is a quarter of
    /// the samples — at 1920×1080 that is 8.3 million depth samples a frame
    /// where the full-resolution pass took 33 million, and at 4K four times
    /// that saving. What the drop costs is edge accuracy, which the composite
    /// buys back with a depth-aware upsample rather than the box average it
    /// replaces.
    pub const AO_SCALE: u32 = 2;

    /// The reduced depth's format.
    ///
    /// A colour target rather than a depth one: it is read by two later passes
    /// as an ordinary texture, and a single channel of full float is exactly
    /// the depth buffer's own precision. Loaded, never filtered, so it needs
    /// no float-filtering feature.
    pub const REDUCED_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

    pub fn new(gpu: &Gpu, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let samples = gpu.sample_count(format);
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        // Rounded up, so the last partial block of the frame still has an
        // occlusion pixel covering it rather than being left unshaded.
        let (ao_width, ao_height) = (
            width.div_ceil(Self::AO_SCALE),
            height.div_ceil(Self::AO_SCALE),
        );
        let ao_size = wgpu::Extent3d {
            width: ao_width,
            height: ao_height,
            depth_or_array_layers: 1,
        };
        let depth = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size,
                mip_level_count: 1,
                // The depth buffer is written by the same pass as the colour
                // one, so it carries the same sample count or the pipeline is
                // invalid.
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: Self::DEPTH_FORMAT,
                // Sampled as well as written: the occlusion pass reads the
                // depth this pass produced rather than being given geometry of
                // its own to re-derive it from.
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let color = (samples > 1).then(|| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("msaa colour"),
                    size,
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        });

        let attachment =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let reduced_depth = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("reduced depth"),
                size: ao_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::REDUCED_DEPTH_FORMAT,
                usage: attachment,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Single-sampled, and one channel. It is a shadowing term rather than
        // a picture: the composite weighs a neighbourhood of it against the
        // frame's own depth anyway, so a sample per pixel is already more
        // resolution than survives that.
        let occlusion = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("occlusion"),
                size: ao_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::OCCLUSION_FORMAT,
                usage: attachment,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            id: NEXT_FRAMEBUFFER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            width,
            height,
            depth,
            color,
            reduced_depth,
            occlusion,
            ao_width,
            ao_height,
            samples,
        }
    }

    /// Which framebuffer this is, for caches built over its views.
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth
    }

    /// What a pass should draw into, and what it should resolve into.
    ///
    /// Multisampling draws into the framebuffer's own colour target and
    /// resolves into `target`; without it, `target` is drawn into directly and
    /// there is nothing to resolve.
    pub fn attachment<'a>(
        &'a self,
        target: &'a wgpu::TextureView,
    ) -> (&'a wgpu::TextureView, Option<&'a wgpu::TextureView>) {
        match &self.color {
            Some(color) => (color, Some(target)),
            None => (target, None),
        }
    }

    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// Where the reduction writes, and the occlusion kernel reads.
    pub fn reduced_depth_view(&self) -> &wgpu::TextureView {
        &self.reduced_depth
    }

    /// Where the occlusion pass writes, and the composite reads.
    ///
    /// Present whatever the sample count. It used to be `None` on a device
    /// that would not multisample, because the kernel bound this
    /// framebuffer's depth buffer as `texture_depth_multisampled_2d` and a
    /// single-sampled texture cannot be bound to that — so such a device drew
    /// with no occlusion for a reason that was about a binding rather than
    /// about rendering. The reduction pass removed the coupling.
    pub fn occlusion_view(&self) -> &wgpu::TextureView {
        &self.occlusion
    }

    /// The occlusion target's own size, in pixels.
    ///
    /// Stated separately from the scene's because the two do not agree: the
    /// kernel runs at [`Framebuffer::AO_SCALE`] display pixels per occlusion
    /// pixel, and a pass told only the scene's size could not place itself.
    pub fn occlusion_size(&self) -> [u32; 2] {
        [self.ao_width, self.ao_height]
    }

    /// The format the occlusion target is written in.
    pub const OCCLUSION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The qualities are ordered by what they cost, which is what lets
    /// `supported_on` walk down from the one that was asked for.
    #[test]
    fn the_qualities_are_ordered_by_their_sample_count() {
        let mut samples: Vec<u32> = MsaaQuality::ALL.iter().map(|q| q.samples()).collect();
        assert_eq!(samples, vec![1, 2, 4, 8]);
        samples.sort_unstable();
        assert_eq!(samples, vec![1, 2, 4, 8]);
        assert!(MsaaQuality::Off < MsaaQuality::X2);
        assert!(MsaaQuality::X4 < MsaaQuality::X8);
        assert_eq!(MsaaQuality::default(), MsaaQuality::X4);
    }

    /// Eight is never what an adapter is given automatically. It costs twice
    /// what four does to move a silhouette by a fraction of a pixel.
    #[test]
    fn eight_is_never_chosen_for_a_device() {
        // Not parameterised over a real adapter — this is about the mapping,
        // and the mapping has two arms.
        for chosen in [MsaaQuality::X4, MsaaQuality::X2] {
            assert!(chosen < MsaaQuality::X8);
        }
    }
}
