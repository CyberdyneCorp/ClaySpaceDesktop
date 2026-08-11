//! Drawing the sculpt.
//!
//! The renderer takes plain vertex and index data. It knows nothing about
//! ClayCore — that is the layering rule, and it is also what lets the same
//! code draw a document, a voxel grid or a test fixture without caring which.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::gpu::{Framebuffer, Gpu};
use crate::matcap::MatCap;
use crate::palette;

/// One vertex, in the layout the shader and the engine's copy both use.
///
/// `position` at 0, `normal` at 12, `color` at 24, stride 36. The engine writes
/// the first two directly into a mapped buffer at these offsets, which is why
/// the layout is stated once and shared rather than described in two places.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

impl Vertex {
    pub const STRIDE: usize = std::mem::size_of::<Self>();
    pub const POSITION_OFFSET: usize = 0;
    pub const NORMAL_OFFSET: usize = 12;
    pub const COLOR_OFFSET: usize = 24;

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: Self::STRIDE as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: Self::POSITION_OFFSET as u64,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: Self::NORMAL_OFFSET as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: Self::COLOR_OFFSET as u64,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_projection: [[f32; 4]; 4],
    view_rotation: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialUniform {
    tint: [f32; 4],
}

/// Geometry living on the GPU.
///
/// Buffers grow when the geometry outgrows them and are kept when it shrinks,
/// so a sculpting session does not reallocate on every dab.
pub struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
    index_count: u32,
    bounds: Option<(Vec3, Vec3)>,
}

impl GpuMesh {
    /// An empty mesh with no allocation yet.
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            vertices: empty_buffer(gpu, "vertices", wgpu::BufferUsages::VERTEX),
            indices: empty_buffer(gpu, "indices", wgpu::BufferUsages::INDEX),
            vertex_capacity: 0,
            index_capacity: 0,
            index_count: 0,
            bounds: None,
        }
    }

    /// Replaces the whole mesh.
    pub fn upload(&mut self, gpu: &Gpu, vertices: &[Vertex], indices: &[u32]) {
        if vertices.len() > self.vertex_capacity {
            self.vertices = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vertices"),
                size: (vertices.len() * Vertex::STRIDE) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_capacity = vertices.len();
        }
        if indices.len() > self.index_capacity {
            self.indices = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("indices"),
                size: (indices.len() * 4) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.index_capacity = indices.len();
        }

        if !vertices.is_empty() {
            gpu.queue
                .write_buffer(&self.vertices, 0, bytemuck::cast_slice(vertices));
        }
        if !indices.is_empty() {
            gpu.queue
                .write_buffer(&self.indices, 0, bytemuck::cast_slice(indices));
        }
        self.index_count = indices.len() as u32;
        self.bounds = bounds_of(vertices);
    }

    /// Overwrites one contiguous run of vertices, leaving the rest alone.
    ///
    /// This is the incremental path: a dab re-meshes its dirty keys and each
    /// key's range is patched in place. Ranges may be overwritten but never
    /// freed in isolation, because the engine welds vertices across brick
    /// seams — compaction is a whole-mesh operation, not a per-dab one.
    pub fn patch_vertices(&mut self, gpu: &Gpu, first: u32, vertices: &[Vertex]) {
        if vertices.is_empty() {
            return;
        }
        debug_assert!(
            first as usize + vertices.len() <= self.vertex_capacity,
            "a patch must lie inside the allocated buffer"
        );
        gpu.queue.write_buffer(
            &self.vertices,
            (first as usize * Vertex::STRIDE) as u64,
            bytemuck::cast_slice(vertices),
        );
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    pub fn is_empty(&self) -> bool {
        self.index_count == 0
    }

    /// The mesh's world bounds, for framing.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        self.bounds
    }
}

fn empty_buffer(gpu: &Gpu, label: &str, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: 4,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn bounds_of(vertices: &[Vertex]) -> Option<(Vec3, Vec3)> {
    let mut iter = vertices.iter();
    let first = Vec3::from(iter.next()?.position);
    let (min, max) = iter.fold((first, first), |(min, max), v| {
        let p = Vec3::from(v.position);
        (min.min(p), max.max(p))
    });
    Some((min, max))
}

/// What the viewport draws behind the sculpt.
#[derive(Debug, Clone, Copy)]
pub struct Overlays {
    pub grid: bool,
    pub symmetry_plane: Option<SymmetryAxis>,
}

impl Default for Overlays {
    fn default() -> Self {
        Self {
            grid: true,
            symmetry_plane: None,
        }
    }
}

/// Which plane the symmetry indicator sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymmetryAxis {
    X,
    Y,
    Z,
}

/// Draws meshes with a MatCap material.
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    material_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    matcap: MatCap,
    format: wgpu::TextureFormat,
    /// The neutral, desaturated ground the design calls for. Never tinted:
    /// a coloured ground shifts the apparent value of the material.
    pub background: wgpu::Color,
    overlay_mesh: GpuMesh,
}

impl Renderer {
    /// The viewport's ground, `#23262B`, converted to linear because the
    /// render target is sRGB-encoded.
    pub const BACKGROUND: wgpu::Color = wgpu::Color {
        r: palette::GROUND[0] as f64,
        g: palette::GROUND[1] as f64,
        b: palette::GROUND[2] as f64,
        a: 1.0,
    };

    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Self {
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("matcap"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/matcap.wgsl").into()),
            });

        let bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("viewport"),
                    entries: &[
                        uniform_entry(0),
                        uniform_entry(1),
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let material_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("material"),
            size: std::mem::size_of::<MaterialUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("matcap"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let matcap = MatCap::default();
        let texture_view = upload_matcap(gpu, matcap);
        let bind_group = make_bind_group(
            gpu,
            &bind_group_layout,
            &camera_buffer,
            &material_buffer,
            &texture_view,
            &sampler,
        );

        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viewport"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let pipeline = make_pipeline(
            gpu,
            &layout,
            &shader,
            format,
            "vs_main",
            "fs_main",
            wgpu::PrimitiveTopology::TriangleList,
            true,
        );
        let overlay_pipeline = make_pipeline(
            gpu,
            &layout,
            &shader,
            format,
            "overlay_vs",
            "overlay_fs",
            wgpu::PrimitiveTopology::LineList,
            false,
        );

        Self {
            pipeline,
            overlay_pipeline,
            camera_buffer,
            material_buffer,
            bind_group,
            bind_group_layout,
            sampler,
            matcap,
            format,
            background: Self::BACKGROUND,
            overlay_mesh: GpuMesh::new(gpu),
        }
    }

    pub fn matcap(&self) -> MatCap {
        self.matcap
    }

    /// Changes the display material.
    ///
    /// Display only: this touches no geometry and belongs in no undo history.
    pub fn set_matcap(&mut self, gpu: &Gpu, matcap: MatCap) {
        if self.matcap == matcap {
            return;
        }
        self.matcap = matcap;
        let texture_view = upload_matcap(gpu, matcap);
        self.bind_group = make_bind_group(
            gpu,
            &self.bind_group_layout,
            &self.camera_buffer,
            &self.material_buffer,
            &texture_view,
            &self.sampler,
        );
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Rebuilds the overlay geometry for the current settings.
    pub fn set_overlays(&mut self, gpu: &Gpu, overlays: Overlays, extent: f32) {
        let (vertices, indices) = overlay_geometry(overlays, extent);
        self.overlay_mesh.upload(gpu, &vertices, &indices);
    }

    /// Draws one frame into `target`.
    pub fn render(
        &self,
        gpu: &Gpu,
        target: &wgpu::TextureView,
        framebuffer: &Framebuffer,
        camera: &Camera,
        mesh: &GpuMesh,
        has_vertex_colors: bool,
    ) {
        let aspect = framebuffer.aspect();
        let uniform = CameraUniform {
            view_projection: camera.view_projection(aspect).to_cols_array_2d(),
            view_rotation: camera.view_rotation().to_cols_array_2d(),
        };
        gpu.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        gpu.queue.write_buffer(
            &self.material_buffer,
            0,
            bytemuck::bytes_of(&MaterialUniform {
                tint: [1.0, 1.0, 1.0, if has_vertex_colors { 1.0 } else { 0.0 }],
            }),
        );

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("viewport"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.background),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: framebuffer.depth_view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &self.bind_group, &[]);

            if !self.overlay_mesh.is_empty() {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_vertex_buffer(0, self.overlay_mesh.vertices.slice(..));
                pass.set_index_buffer(
                    self.overlay_mesh.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.overlay_mesh.index_count, 0, 0..1);
            }

            if !mesh.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
        gpu.queue.submit(Some(encoder.finish()));
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn make_bind_group(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    camera: &wgpu::Buffer,
    material: &wgpu::Buffer,
    texture: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viewport"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: material.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(texture),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

#[allow(clippy::too_many_arguments)]
fn make_pipeline(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    vs: &str,
    fs: &str,
    topology: wgpu::PrimitiveTopology,
    cull: bool,
) -> wgpu::RenderPipeline {
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(vs),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some(vs),
                buffers: &[Vertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fs),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology,
                cull_mode: cull.then_some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Framebuffer::DEPTH_FORMAT,
                depth_write_enabled: cull,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
}

fn upload_matcap(gpu: &Gpu, matcap: MatCap) -> wgpu::TextureView {
    const SIZE: u32 = 256;
    let pixels = matcap.generate(SIZE);
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("matcap"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &pixels,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Builds the grid and symmetry-plane line geometry.
///
/// Overlays are drawn low-contrast and behind the sculpt in visual weight, and
/// are excluded from every export — they exist only in this function.
fn overlay_geometry(overlays: Overlays, extent: f32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let mut line = |a: Vec3, b: Vec3, color: [f32; 3]| {
        let base = vertices.len() as u32;
        vertices.push(Vertex {
            position: a.into(),
            normal: [0.0, 1.0, 0.0],
            color,
        });
        vertices.push(Vertex {
            position: b.into(),
            normal: [0.0, 1.0, 0.0],
            color,
        });
        indices.extend_from_slice(&[base, base + 1]);
    };

    if overlays.grid {
        let steps = 20;
        let step = extent * 2.0 / steps as f32;
        // One and two steps up from the ground. Written in linear, because
        // the target encodes: passing the design's hex values straight through
        // renders them several times too bright.
        let minor = palette::GRID_MINOR;
        let axis = palette::GRID_AXIS;
        for i in 0..=steps {
            let t = -extent + i as f32 * step;
            let color = if i == steps / 2 { axis } else { minor };
            line(Vec3::new(t, 0.0, -extent), Vec3::new(t, 0.0, extent), color);
            line(Vec3::new(-extent, 0.0, t), Vec3::new(extent, 0.0, t), color);
        }
    }

    if let Some(axis) = overlays.symmetry_plane {
        // The accent, because the symmetry plane is tool state rather than
        // scene furniture — but dimmed, since a reference overlay must not be
        // the brightest thing on screen.
        let color = palette::dimmed(palette::ACCENT, 0.25);
        let steps = 8;
        let step = extent * 2.0 / steps as f32;
        for i in 0..=steps {
            let t = -extent + i as f32 * step;
            let (a, b, c, d) = match axis {
                SymmetryAxis::X => (
                    Vec3::new(0.0, t, -extent),
                    Vec3::new(0.0, t, extent),
                    Vec3::new(0.0, -extent, t),
                    Vec3::new(0.0, extent, t),
                ),
                SymmetryAxis::Y => (
                    Vec3::new(t, 0.0, -extent),
                    Vec3::new(t, 0.0, extent),
                    Vec3::new(-extent, 0.0, t),
                    Vec3::new(extent, 0.0, t),
                ),
                SymmetryAxis::Z => (
                    Vec3::new(t, -extent, 0.0),
                    Vec3::new(t, extent, 0.0),
                    Vec3::new(-extent, t, 0.0),
                    Vec3::new(extent, t, 0.0),
                ),
            };
            line(a, b, color);
            line(c, d, color);
        }
    }

    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vertex_layout_matches_what_the_engine_is_told() {
        // The engine writes into this layout by byte offset, so a change here
        // that is not mirrored there produces a silently wrong buffer.
        assert_eq!(Vertex::STRIDE, 36);
        assert_eq!(Vertex::POSITION_OFFSET, 0);
        assert_eq!(Vertex::NORMAL_OFFSET, 12);
        assert_eq!(Vertex::COLOR_OFFSET, 24);
    }

    #[test]
    fn no_field_math_in_shaders() {
        // The whole point of meshing on the engine side is that the shader
        // does not re-implement the field. If one of these appears here, the
        // drift this project is built to avoid has started.
        let shader = include_str!("shaders/matcap.wgsl").to_lowercase();
        for forbidden in [
            "sd_sphere", "sdsphere", "smin", "smooth_min", "sdbox", "sd_box",
            "signed_distance", "raymarch", "sphere_trace", "ctape_eval",
        ] {
            assert!(
                !shader.contains(forbidden),
                "the viewport shader contains `{forbidden}`, which means it is \
                 evaluating the field instead of drawing the mesh the engine produced"
            );
        }
    }

    #[test]
    fn overlays_produce_line_geometry_only_when_asked() {
        let (none, _) = overlay_geometry(
            Overlays {
                grid: false,
                symmetry_plane: None,
            },
            1.0,
        );
        assert!(none.is_empty(), "overlays were built when none were requested");

        let (grid, grid_indices) = overlay_geometry(
            Overlays {
                grid: true,
                symmetry_plane: None,
            },
            1.0,
        );
        assert!(!grid.is_empty());
        assert_eq!(grid_indices.len() % 2, 0, "line geometry needs index pairs");

        let (both, _) = overlay_geometry(
            Overlays {
                grid: true,
                symmetry_plane: Some(SymmetryAxis::X),
            },
            1.0,
        );
        assert!(
            both.len() > grid.len(),
            "adding the symmetry plane produced no extra geometry"
        );
    }
}
