//! The window and its swapchain.
//!
//! The surface is the *other* place a frame can be drawn into; the offscreen
//! target is the first. Both hand the renderer a texture view and a
//! framebuffer, so nothing below here knows which it is drawing to.

use std::sync::Arc;

use winit::window::Window;

use crate::gpu::{Framebuffer, Gpu};

/// A window's drawable surface, resized and recovered as needed.
pub struct WindowSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    framebuffer: Framebuffer,
    /// The same instance the device was made from, held so the surface cannot
    /// outlive it.
    _instance: Arc<wgpu::Instance>,
    /// Kept alive because the surface borrows from it.
    _window: Arc<Window>,
}

impl WindowSurface {
    /// Creates a surface and a device able to present to it.
    pub async fn new(window: Arc<Window>) -> Result<(Gpu, Self), crate::gpu::GpuError> {
        // ONE instance, used for both the surface and the device.
        //
        // The first version made two: this function created one for the
        // surface, and the device constructor quietly created another for the
        // adapter. Everything reported success and the first presented frame
        // aborted with `Surface does not exist`, because a surface lives in
        // the registry of the instance that made it and the device belonged to
        // a different one.
        let instance = Arc::new(Gpu::new_instance());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| crate::gpu::GpuError::NoDevice(e.to_string()))?;

        let gpu = Gpu::create(instance.clone(), Some(&surface)).await?;

        let size = window.inner_size();
        let capabilities = surface.get_capabilities(gpu.adapter());
        // An sRGB target so the palette's linear values are encoded by the
        // hardware, exactly as the offscreen target does.
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&gpu.device, &config);

        let framebuffer = Framebuffer::new(&gpu, config.width, config.height);
        Ok((
            gpu.clone(),
            Self {
                surface,
                config,
                framebuffer,
                _instance: instance,
                _window: window,
            },
        ))
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    /// Reconfigures for a new size. A zero dimension is ignored rather than
    /// producing an invalid swapchain, which a minimise reports.
    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&gpu.device, &self.config);
        self.framebuffer = Framebuffer::new(gpu, width, height);
    }

    /// Acquires the next image to draw into.
    ///
    /// A lost or outdated swapchain is reconfigured and retried once, which is
    /// the ordinary response to a resize or a display change. A device that is
    /// genuinely gone is reported so the caller can rebuild everything without
    /// losing the document.
    pub fn acquire(&mut self, gpu: &Gpu) -> Result<wgpu::SurfaceTexture, SurfaceLoss> {
        match self.surface.get_current_texture() {
            Ok(frame) => Ok(frame),
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&gpu.device, &self.config);
                self.surface
                    .get_current_texture()
                    .map_err(|_| SurfaceLoss::Reconfigure)
            }
            Err(wgpu::SurfaceError::Timeout) => Err(SurfaceLoss::Skip),
            Err(wgpu::SurfaceError::OutOfMemory | wgpu::SurfaceError::Other) => {
                Err(SurfaceLoss::DeviceLost)
            }
        }
    }
}

/// Why a frame could not be acquired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceLoss {
    /// Transient; skip this frame and try the next.
    Skip,
    /// The swapchain needed rebuilding and still failed; try again next frame.
    Reconfigure,
    /// The device is gone. Everything GPU-side must be recreated — and the
    /// document must survive it, because none of it lives on the GPU.
    DeviceLost,
}

impl std::fmt::Display for SurfaceLoss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Skip => f.write_str("the frame timed out"),
            Self::Reconfigure => f.write_str("the swapchain had to be rebuilt"),
            Self::DeviceLost => f.write_str("the graphics device was lost"),
        }
    }
}
