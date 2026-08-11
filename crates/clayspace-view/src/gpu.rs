//! The WebGPU device, and the surfaces things are drawn into.
//!
//! Headless first: the renderer draws into a texture view, and a window is one
//! way to obtain one. That is what lets every visual test render and compare a
//! real image in CI, with no display attached.
//!
//! This device is chosen independently of the engine's evaluation backend. A
//! software rendering adapter does not stop the engine using a GPU, and a
//! missing GPU backend does not stop rendering.

use std::sync::Arc;

/// A WebGPU device and its queue.
#[derive(Clone)]
pub struct Gpu {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    adapter: Arc<wgpu::Adapter>,
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

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("clayspace"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::Performance,
            }, None)
            .await
            .map_err(|e| GpuError::NoDevice(e.to_string()))?;

        Ok(Self {
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
            instance,
        })
    }

    /// The instance this device came from.
    ///
    /// Surfaces must be created from it, and must not outlive it.
    pub fn instance(&self) -> &Arc<wgpu::Instance> {
        &self.instance
    }

    /// The adapter this device came from, for surface configuration.
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// Which adapter is rendering, for the diagnostics view.
    ///
    /// Distinct from the engine's evaluation backend: this is what draws, that
    /// is what evaluates, and they are chosen separately.
    pub fn adapter_description(&self) -> String {
        let info = self.adapter.get_info();
        format!("{} ({:?}, {:?})", info.name, info.device_type, info.backend)
    }
}

impl std::fmt::Debug for Gpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gpu")
            .field("adapter", &self.adapter_description())
            .finish()
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
    depth: wgpu::TextureView,
}

impl Framebuffer {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
        let depth = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: Self::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            width: width.max(1),
            height: height.max(1),
            depth,
        }
    }

    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth
    }

    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}
