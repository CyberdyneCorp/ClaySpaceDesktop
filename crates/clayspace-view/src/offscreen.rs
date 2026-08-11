//! Rendering to an image instead of a window.
//!
//! This exists so that every visual feature — a material, a brush's effect on
//! the surface, an overlay, a view preset — can be rendered in CI, written to
//! a PNG and looked at. A renderer that can only draw into a window can only
//! be checked by someone sitting in front of one.

use crate::camera::Camera;
use crate::gpu::{Framebuffer, Gpu};
use crate::renderer::{GpuMesh, Renderer};

/// An offscreen colour target that can be read back as pixels.
pub struct OffscreenTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    framebuffer: Framebuffer,
    /// Row stride in the readback buffer. Copies require rows aligned to 256
    /// bytes, so this is usually wider than the image.
    padded_bytes_per_row: u32,
    readback: wgpu::Buffer,
}

impl OffscreenTarget {
    /// The format offscreen renders use. sRGB so that what a test writes to a
    /// PNG is what a display would show.
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1), height.max(1));

        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let unpadded = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded.div_ceil(align) * align;

        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            texture,
            view,
            framebuffer: Framebuffer::new(gpu, width, height),
            padded_bytes_per_row,
            readback,
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn framebuffer(&self) -> &Framebuffer {
        &self.framebuffer
    }

    pub fn width(&self) -> u32 {
        self.framebuffer.width
    }

    pub fn height(&self) -> u32 {
        self.framebuffer.height
    }

    /// Renders one frame and returns it as RGBA8 rows, unpadded.
    pub fn capture(
        &self,
        gpu: &Gpu,
        renderer: &Renderer,
        camera: &Camera,
        mesh: &GpuMesh,
        has_vertex_colors: bool,
    ) -> Image {
        renderer.render(
            gpu,
            &self.view,
            &self.framebuffer,
            camera,
            mesh,
            has_vertex_colors,
        );
        self.read_back(gpu)
    }

    /// Copies the colour target into host memory.
    ///
    /// Public so a caller that painted the target itself — the interface, which
    /// egui renders rather than this module — can read the result back.
    pub fn read_back_public(&self, gpu: &Gpu) -> Image {
        self.read_back(gpu)
    }

    /// Copies the colour target into host memory.
    fn read_back(&self, gpu: &Gpu) -> Image {
        let (width, height) = (self.width(), self.height());

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit(Some(encoder.finish()));

        let slice = self.readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        // The map completes on the device's own timeline, so the queue has to
        // be pumped until it does.
        gpu.device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .expect("the readback mapping was dropped")
            .expect("the readback buffer could not be mapped");

        let padded = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (row * self.padded_bytes_per_row) as usize;
            pixels.extend_from_slice(&padded[start..start + (width * 4) as usize]);
        }
        drop(padded);
        self.readback.unmap();

        Image {
            width,
            height,
            pixels,
        }
    }
}

/// A captured frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// RGBA8, row major, no padding.
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }

    /// How many pixels differ from the background by more than `threshold`.
    ///
    /// The measure visual tests use to answer "did anything get drawn?" and
    /// "did this edit change the silhouette?" without needing a golden file.
    pub fn pixels_differing_from(&self, background: [u8; 4], threshold: u8) -> usize {
        self.pixels
            .chunks_exact(4)
            .filter(|p| (0..3).any(|i| p[i].abs_diff(background[i]) > threshold))
            .count()
    }

    /// Mean absolute per-channel difference against another image.
    ///
    /// Used to assert that two renders differ (a brush changed the surface) or
    /// agree (a backend swap did not).
    pub fn mean_difference(&self, other: &Image) -> f64 {
        assert_eq!(
            (self.width, self.height),
            (other.width, other.height),
            "images of different sizes cannot be compared"
        );
        let total: u64 = self
            .pixels
            .iter()
            .zip(&other.pixels)
            .map(|(a, b)| a.abs_diff(*b) as u64)
            .sum();
        total as f64 / self.pixels.len() as f64
    }

    /// Mean absolute difference over the drawn subject only.
    ///
    /// Averaging across the whole frame dilutes a real difference by however
    /// much empty ground surrounds the subject, so two materials that look
    /// obviously different can score near zero. This measures what a viewer
    /// actually compares.
    pub fn mean_difference_over_subject(
        &self,
        other: &Image,
        background: [u8; 4],
        threshold: u8,
    ) -> f64 {
        assert_eq!(
            (self.width, self.height),
            (other.width, other.height),
            "images of different sizes cannot be compared"
        );
        let is_subject = |p: &[u8]| (0..3).any(|i| p[i].abs_diff(background[i]) > threshold);

        let (mut total, mut counted) = (0u64, 0usize);
        for (a, b) in self
            .pixels
            .chunks_exact(4)
            .zip(other.pixels.chunks_exact(4))
        {
            if is_subject(a) || is_subject(b) {
                total += (0..3).map(|i| a[i].abs_diff(b[i]) as u64).sum::<u64>();
                counted += 3;
            }
        }
        if counted == 0 {
            return 0.0;
        }
        total as f64 / counted as f64
    }

    /// The fraction of subject pixels that visibly changed.
    ///
    /// A mean is the wrong measure for a dent or a ridge: a real, obvious
    /// local change averages away against the untouched surface around it.
    /// This asks how much of the subject moved, which is what an eye judges.
    pub fn changed_fraction_over_subject(
        &self,
        other: &Image,
        background: [u8; 4],
        threshold: u8,
    ) -> f64 {
        assert_eq!(
            (self.width, self.height),
            (other.width, other.height),
            "images of different sizes cannot be compared"
        );
        let is_subject = |p: &[u8]| (0..3).any(|i| p[i].abs_diff(background[i]) > threshold);

        let (mut changed, mut subject) = (0usize, 0usize);
        for (a, b) in self
            .pixels
            .chunks_exact(4)
            .zip(other.pixels.chunks_exact(4))
        {
            if !is_subject(a) && !is_subject(b) {
                continue;
            }
            subject += 1;
            if (0..3).any(|i| a[i].abs_diff(b[i]) > threshold) {
                changed += 1;
            }
        }
        if subject == 0 {
            return 0.0;
        }
        changed as f64 / subject as f64
    }

    /// The average colour of the whole frame, for coarse comparisons.
    pub fn mean_color(&self) -> [f64; 3] {
        let mut sums = [0u64; 3];
        for p in self.pixels.chunks_exact(4) {
            for c in 0..3 {
                sums[c] += p[c] as u64;
            }
        }
        let count = (self.pixels.len() / 4) as f64;
        [
            sums[0] as f64 / count,
            sums[1] as f64 / count,
            sums[2] as f64 / count,
        ]
    }
}
