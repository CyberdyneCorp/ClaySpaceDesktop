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

    /// Allocates buffers of a fixed size without writing anything.
    ///
    /// The incremental path needs the addresses to exist before it knows what
    /// goes in them, which `upload` cannot offer — it sizes the buffers to the
    /// data it is given.
    pub fn reserve(&mut self, gpu: &Gpu, vertices: usize, indices: usize) {
        self.vertices = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vertices"),
            size: (vertices * Vertex::STRIDE).max(4) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.indices = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("indices"),
            size: (indices * 4).max(4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.vertex_capacity = vertices;
        self.index_capacity = indices;
        self.index_count = 0;
    }

    /// Overwrites one contiguous run of vertices, leaving the rest alone.
    ///
    /// This is the incremental path: a dab re-meshes its dirty keys and each
    /// key's span is patched where it already sits, so the cost of an edit is
    /// the size of the edit rather than the size of the model.
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

    /// Overwrites one contiguous run of indices, leaving the rest alone.
    pub fn patch_indices(&mut self, gpu: &Gpu, first: u32, indices: &[u32]) {
        if indices.is_empty() {
            return;
        }
        debug_assert!(
            first as usize + indices.len() <= self.index_capacity,
            "a patch must lie inside the allocated buffer"
        );
        gpu.queue.write_buffer(
            &self.indices,
            (first as usize * 4) as u64,
            bytemuck::cast_slice(indices),
        );
    }

    /// How many indices the draw call covers.
    ///
    /// The incremental path draws one range over spans that are not
    /// necessarily full, so the count is the layout's business, not the
    /// buffer's.
    pub fn set_index_count(&mut self, count: u32) {
        debug_assert!(count as usize <= self.index_capacity);
        self.index_count = count;
    }

    /// Replaces the bounds without touching the geometry.
    ///
    /// The incremental path never holds the whole surface in one slice, so it
    /// tracks bounds itself and states them here.
    pub fn set_bounds(&mut self, bounds: Option<([f32; 3], [f32; 3])>) {
        self.bounds = bounds.map(|(min, max)| (Vec3::from(min), Vec3::from(max)));
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
    /// Which mirror planes to draw, indexed X, Y, Z.
    ///
    /// A set rather than one axis: the document takes a mirror per axis and
    /// more than one can be on at once, and an overlay that could only show
    /// the first would quietly under-report the symmetry in force.
    pub symmetry_planes: [bool; 3],
}

impl Default for Overlays {
    fn default() -> Self {
        Self {
            grid: true,
            symmetry_planes: [false; 3],
        }
    }
}

/// Where the brush is, and how big it reads on the surface.
///
/// The cursor is drawn in the scene rather than as a screen circle, so it
/// shows the footprint the brush will actually cover — a screen circle would
/// lie about a surface angled away from the camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushCursor {
    /// The point on the surface under the pointer.
    pub position: [f32; 3],
    /// The surface normal there, which the ring is drawn perpendicular to.
    pub normal: [f32; 3],
    /// The brush radius in document units.
    pub radius: f32,
    /// Whether this ring is a mirror of the pointer rather than the pointer.
    ///
    /// Symmetry means a stroke lands in more than one place, and a cursor that
    /// only shows one of them is not telling the truth about what the next
    /// click does. The mirrors are drawn dimmer so the ring under the hand is
    /// still the one that reads first.
    pub mirrored: bool,
}

impl BrushCursor {
    /// This cursor reflected through the world plane normal to `axis`.
    ///
    /// The mirror is the document's: [`crate::SymmetryAxis`] planes pass
    /// through the origin, which is what the layer mirror is set to.
    pub fn mirror(self, axis: usize) -> Self {
        let mut cursor = self;
        cursor.position[axis] = -cursor.position[axis];
        cursor.normal[axis] = -cursor.normal[axis];
        cursor.mirrored = true;
        cursor
    }
}

/// Every place a stroke at `cursor` would land, given `symmetry`.
///
/// One ring per enabled combination — two mirrors give four, three give eight
/// — because that is how many dabs the engine deposits.
pub fn mirrored_cursors(cursor: BrushCursor, symmetry: [bool; 3]) -> Vec<BrushCursor> {
    let mut cursors = vec![cursor];
    for (axis, enabled) in symmetry.iter().enumerate() {
        if !enabled {
            continue;
        }
        cursors.extend(
            cursors
                .clone()
                .into_iter()
                .map(|existing| existing.mirror(axis)),
        );
    }
    cursors
}

/// The rig as the viewport should draw it.
#[derive(Debug, Clone, Copy)]
pub struct ArmatureView<'a> {
    /// Every sphere: position and radius.
    pub spheres: &'a [([f32; 3], f32)],
    /// Index pairs joined by a link.
    pub links: &'a [(u32, u32)],
    /// The one under the pointer or being dragged, drawn brighter.
    pub selected: Option<u32>,
    /// The root, which gets its own colour so a rig has a readable origin.
    pub root: Option<u32>,
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
    /// Whether the orientation gizmo is drawn. Off for captures that compare
    /// geometry, since it would register as a difference of its own.
    pub show_gizmo: bool,
    overlay_mesh: GpuMesh,
    cursor_mesh: GpuMesh,
    /// The ZSphere rig, drawn over the surface it skins.
    armature_mesh: GpuMesh,
    /// The translucent skin between the spheres, drawn while rigging.
    membrane_mesh: GpuMesh,
    membrane_pipeline: wgpu::RenderPipeline,
    /// The rectangle of the frame the scene is drawn into, in physical pixels.
    scene_viewport: Option<[f32; 4]>,
    gizmo_mesh: GpuMesh,
    gizmo_camera_buffer: wgpu::Buffer,
    gizmo_bind_group: wgpu::BindGroup,
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

        let gizmo_camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gizmo camera"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
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
        // The gizmo draws with its own view matrix but the same material and
        // texture, so it needs its own bind group over a second camera buffer.
        let gizmo_bind_group = make_bind_group(
            gpu,
            &bind_group_layout,
            &gizmo_camera_buffer,
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

        // Triangles, blended, and no depth write — so the spheres and links
        // behind the membrane still read through it.
        let membrane_pipeline = make_pipeline(
            gpu,
            &layout,
            &shader,
            format,
            "overlay_vs",
            "membrane_fs",
            wgpu::PrimitiveTopology::TriangleList,
            false,
        );

        Self {
            pipeline,
            overlay_pipeline,
            membrane_pipeline,
            camera_buffer,
            material_buffer,
            bind_group,
            bind_group_layout,
            sampler,
            matcap,
            format,
            background: Self::BACKGROUND,
            show_gizmo: false,
            overlay_mesh: GpuMesh::new(gpu),
            cursor_mesh: GpuMesh::new(gpu),
            armature_mesh: GpuMesh::new(gpu),
            membrane_mesh: GpuMesh::new(gpu),
            scene_viewport: None,
            gizmo_mesh: {
                let mut mesh = GpuMesh::new(gpu);
                let (vertices, indices) = gizmo_geometry();
                mesh.upload(gpu, &vertices, &indices);
                mesh
            },
            gizmo_camera_buffer,
            gizmo_bind_group,
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

    /// Places the brush cursor, or clears it when the pointer is off the
    /// surface.
    ///
    /// Clearing rather than leaving the last position is the point: a ring
    /// hanging in space at an arbitrary depth tells the user the brush would
    /// land somewhere it would not.
    pub fn set_cursors(&mut self, gpu: &Gpu, cursors: &[BrushCursor]) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for cursor in cursors {
            let (ring, ring_indices) = cursor_geometry(*cursor);
            let base = vertices.len() as u32;
            vertices.extend(ring);
            indices.extend(ring_indices.into_iter().map(|i| i + base));
        }
        self.cursor_mesh.upload(gpu, &vertices, &indices);
    }

    /// Draws the ZSphere rig: a ring per sphere and a line down each link.
    ///
    /// Rings rather than shaded balls, and drawn after the surface so they
    /// read through it. ZBrush shows its ZSpheres over the preview for the
    /// same reason: a rig you cannot see inside the skin is a rig you cannot
    /// edit.
    pub fn set_armature(&mut self, gpu: &Gpu, view: ArmatureView<'_>) {
        let (membrane_vertices, membrane_indices) = membrane_geometry(&view);
        self.membrane_mesh
            .upload(gpu, &membrane_vertices, &membrane_indices);
        let (vertices, indices) = armature_geometry(view);
        self.armature_mesh.upload(gpu, &vertices, &indices);
    }

    /// Confines the scene to a rectangle of the frame, in physical pixels.
    ///
    /// The panels cover part of the window, and a scene drawn across the whole
    /// framebuffer is centred on the window rather than on the hole the panels
    /// left. `None` restores the full frame, which is what an offscreen
    /// capture wants.
    pub fn set_scene_viewport(&mut self, viewport: Option<[f32; 4]>) {
        self.scene_viewport = viewport;
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
        // The scene's own rectangle decides the aspect, not the window's. A
        // ray is built from the same rectangle, so any disagreement here is a
        // pick that lands somewhere other than where the pixel was.
        let scene = self.scene_viewport.unwrap_or([
            0.0,
            0.0,
            framebuffer.width as f32,
            framebuffer.height as f32,
        ]);
        let aspect = scene[2] / scene[3].max(1.0);
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

        // The gizmo sits at a fixed distance looking at the origin, so it
        // shows only which way the camera is pointed.
        let mut gizmo_camera = *camera;
        gizmo_camera.target = glam::Vec3::ZERO;
        gizmo_camera.distance = 3.0;
        gizmo_camera.preset = crate::camera::ViewPreset::Perspective;
        let gizmo_uniform = CameraUniform {
            view_projection: gizmo_camera.view_projection(1.0).to_cols_array_2d(),
            view_rotation: gizmo_camera.view_rotation().to_cols_array_2d(),
        };
        gpu.queue.write_buffer(
            &self.gizmo_camera_buffer,
            0,
            bytemuck::bytes_of(&gizmo_uniform),
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

            pass.set_viewport(scene[0], scene[1], scene[2], scene[3], 0.0, 1.0);
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

            // The brush cursor, over the surface it will act on.
            if !self.cursor_mesh.is_empty() {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_vertex_buffer(0, self.cursor_mesh.vertices.slice(..));
                pass.set_index_buffer(
                    self.cursor_mesh.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.cursor_mesh.index_count, 0, 0..1);
            }

            // The rig, over the surface it skins.
            // The membrane first: it is translucent and writes no depth, so
            // the spheres and links drawn after it read through rather than
            // being hidden by it.
            if !self.membrane_mesh.is_empty() {
                pass.set_pipeline(&self.membrane_pipeline);
                pass.set_vertex_buffer(0, self.membrane_mesh.vertices.slice(..));
                pass.set_index_buffer(
                    self.membrane_mesh.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.membrane_mesh.index_count, 0, 0..1);
            }
            if !self.armature_mesh.is_empty() {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_vertex_buffer(0, self.armature_mesh.vertices.slice(..));
                pass.set_index_buffer(
                    self.armature_mesh.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.armature_mesh.index_count, 0, 0..1);
            }

            // The navigation gizmo, in its own corner viewport so it keeps a
            // fixed size whatever the window does. It shares the camera's
            // rotation and nothing else — it reports orientation, not position.
            if self.show_gizmo && !self.gizmo_mesh.is_empty() {
                // Anchored to the scene's rectangle, not the window's. Against
                // the window it sat in the corner the right panel covers, so
                // the gizmo was drawn every frame and never once visible.
                let size = (scene[3] * GIZMO_FRACTION).min(120.0);
                let margin = size * 0.25;
                pass.set_viewport(
                    scene[0] + scene[2] - size - margin,
                    scene[1] + margin,
                    size,
                    size,
                    0.0,
                    1.0,
                );
                pass.set_bind_group(0, &self.gizmo_bind_group, &[]);
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_vertex_buffer(0, self.gizmo_mesh.vertices.slice(..));
                pass.set_index_buffer(self.gizmo_mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.gizmo_mesh.index_count, 0, 0..1);
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

    for axis in [SymmetryAxis::X, SymmetryAxis::Y, SymmetryAxis::Z] {
        if !overlays.symmetry_planes[axis as usize] {
            continue;
        }
        // The accent, because the symmetry plane is tool state rather than
        // scene furniture — but dimmed, since a reference overlay must not be
        // the brightest thing on screen. At 0.25 over an eight-by-eight grid
        // it was: the capture showed a bright orange wall with the sculpt
        // behind it. Sparser and darker reads as "the mirror is here" without
        // competing with the thing being sculpted.
        let color = palette::dimmed(palette::ACCENT, 0.12);
        let steps = 4;
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

/// A ring on the surface, plus a mark at its centre.
///
/// The accent colour, because this is the active brush — the one thing the
/// design reserves it for.
fn cursor_geometry(cursor: BrushCursor) -> (Vec<Vertex>, Vec<u32>) {
    const SEGMENTS: usize = 48;

    let centre = Vec3::from(cursor.position);
    let normal = {
        let n = Vec3::from(cursor.normal);
        if n.length_squared() > 1e-6 {
            n.normalize()
        } else {
            Vec3::Y
        }
    };
    // Any pair perpendicular to the normal will do; picking the axis least
    // aligned with it avoids a degenerate cross product.
    let reference = if normal.x.abs() < 0.9 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let u = normal.cross(reference).normalize() * cursor.radius;
    let v = normal.cross(u).normalize() * cursor.radius;

    // A mirror is where the stroke also lands, not where the hand is. Dimming
    // it keeps the two readable as different things at a glance.
    let color = if cursor.mirrored {
        palette::dimmed(palette::ACCENT, 0.45)
    } else {
        palette::ACCENT
    };
    let mut vertices = Vec::with_capacity(SEGMENTS + 4);
    let mut indices = Vec::with_capacity(SEGMENTS * 2 + 4);

    for i in 0..SEGMENTS {
        let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let (s, c) = angle.sin_cos();
        // Lifted a hair along the normal so the ring is not swallowed by the
        // surface it sits on.
        let point = centre + u * c + v * s + normal * (cursor.radius * 0.02);
        vertices.push(Vertex {
            position: point.into(),
            normal: normal.into(),
            color,
        });
        indices.push(i as u32);
        indices.push(((i + 1) % SEGMENTS) as u32);
    }

    // A small cross at the centre, so the exact point is readable when the
    // ring is large.
    let tick = cursor.radius * 0.12;
    let base = vertices.len() as u32;
    for (a, b) in [
        (u.normalize() * tick, -u.normalize() * tick),
        (v.normalize() * tick, -v.normalize() * tick),
    ] {
        let offset = normal * (cursor.radius * 0.02);
        for point in [centre + a + offset, centre + b + offset] {
            vertices.push(Vertex {
                position: point.into(),
                normal: normal.into(),
                color,
            });
        }
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base + 3]);

    (vertices, indices)
}

/// A tapered sleeve along each link, which is the membrane a rig would skin
/// into.
///
/// ZBrush shows this while a rig is being built and shows it translucent, so
/// the chain reads through its own surface. Eight sides is enough at the size
/// a link is drawn — this is a hint about where the skin will go, not the skin.
fn membrane_geometry(view: &ArmatureView<'_>) -> (Vec<Vertex>, Vec<u32>) {
    const SIDES: usize = 8;
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for (child, parent) in view.links {
        let (Some((a, ra)), Some((b, rb))) = (
            view.spheres.get(*child as usize),
            view.spheres.get(*parent as usize),
        ) else {
            continue;
        };
        let (from, to) = (Vec3::from(*a), Vec3::from(*b));
        let axis = to - from;
        let length = axis.length();
        if length < 1e-5 {
            continue;
        }
        let forward = axis / length;
        // Any vector not along the axis gives a frame to sweep the ring in.
        let aside = if forward.x.abs() < 0.9 {
            Vec3::X
        } else {
            Vec3::Y
        };
        let u = forward.cross(aside).normalize();
        let v = forward.cross(u);

        let colour = palette::dimmed(palette::ACCENT, 0.5);
        let base = vertices.len() as u32;
        for side in 0..SIDES {
            let angle = side as f32 / SIDES as f32 * std::f32::consts::TAU;
            let (s, c) = angle.sin_cos();
            let offset = u * c + v * s;
            // Slightly inside each sphere, so the sleeve meets them rather
            // than poking out of their silhouettes.
            for (centre, radius) in [(from, *ra), (to, *rb)] {
                vertices.push(Vertex {
                    position: (centre + offset * radius * 0.72).into(),
                    normal: offset.into(),
                    color: colour,
                });
            }
        }
        for side in 0..SIDES {
            let next = (side + 1) % SIDES;
            let (a0, a1) = (base + side as u32 * 2, base + side as u32 * 2 + 1);
            let (b0, b1) = (base + next as u32 * 2, base + next as u32 * 2 + 1);
            // Both windings, because the sleeve is seen from inside as often
            // as outside and this pipeline does not cull.
            indices.extend_from_slice(&[a0, a1, b1, a0, b1, b0]);
        }
    }

    (vertices, indices)
}

/// Three rings and a cross per sphere, and a line per link.
///
/// Three rings rather than one: a single ring lies in the view plane and a rig
/// then reads as flat, which is exactly the information a rig has to convey.
fn armature_geometry(view: ArmatureView<'_>) -> (Vec<Vertex>, Vec<u32>) {
    const SEGMENTS: usize = 24;
    /// How far outside the skin the hoops sit.
    ///
    /// At a joint the skin *is* the sphere, so a hoop at the same radius is
    /// coincident with the surface it exists to annotate and vanishes into it.
    /// The first version drew rings flush and the rig was invisible over its
    /// own skin — 0.097 of the frame covered with the scaffolding on, and
    /// 0.097 with it off.
    const PROUD: f32 = 1.05;
    /// And a floor, so a small sphere is still ringed rather than swallowed by
    /// the surface's own thickness.
    const MARGIN: f32 = 0.01;
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut ring = |centre: Vec3, radius: f32, axis: usize, color: [f32; 3]| {
        let base = vertices.len() as u32;
        for i in 0..SEGMENTS {
            let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let (s, c) = angle.sin_cos();
            let offset = match axis {
                0 => Vec3::new(0.0, c, s),
                1 => Vec3::new(c, 0.0, s),
                _ => Vec3::new(c, s, 0.0),
            };
            vertices.push(Vertex {
                position: (centre + offset * radius).into(),
                normal: [0.0, 1.0, 0.0],
                color,
            });
            indices.push(base + i as u32);
            indices.push(base + ((i + 1) % SEGMENTS) as u32);
        }
    };

    for (index, (position, radius)) in view.spheres.iter().enumerate() {
        let index = index as u32;
        // The selected sphere is the accent at full strength; the root is
        // distinguished so a rig has a readable origin; the rest are quiet.
        let color = if view.selected == Some(index) {
            palette::ACCENT
        } else if view.root == Some(index) {
            palette::dimmed(palette::ACCENT, 0.7)
        } else {
            palette::dimmed(palette::FOREGROUND, 0.55)
        };
        let centre = Vec3::from(*position);
        let hoop = radius * PROUD + MARGIN;
        for axis in 0..3 {
            ring(centre, hoop, axis, color);
        }
    }

    // A line down each link, so the tree's shape is visible where the spheres
    // are far apart.
    for (child, parent) in view.links {
        let (Some((a, _)), Some((b, _))) = (
            view.spheres.get(*child as usize),
            view.spheres.get(*parent as usize),
        ) else {
            continue;
        };
        let color = palette::dimmed(palette::ACCENT, 0.45);
        let base = vertices.len() as u32;
        for point in [Vec3::from(*a), Vec3::from(*b)] {
            vertices.push(Vertex {
                position: point.into(),
                normal: [0.0, 1.0, 0.0],
                color,
            });
        }
        indices.push(base);
        indices.push(base + 1);
    }

    (vertices, indices)
}

/// How much of the frame's height the gizmo occupies.
const GIZMO_FRACTION: f32 = 0.18;

/// The three labelled axes, drawn as lines from the origin.
///
/// Each axis takes a distinct hue so the orientation is readable at a glance,
/// and the negative half is drawn dimmer so front and back are separable.
fn gizmo_geometry() -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let axes = [
        (Vec3::X, [0.85f32, 0.22, 0.24]),
        (Vec3::Y, [0.36, 0.72, 0.32]),
        (Vec3::Z, [0.28, 0.48, 0.88]),
    ];

    for (direction, color) in axes {
        for (end, shade) in [(direction, 1.0f32), (-direction, 0.25)] {
            let base = vertices.len() as u32;
            let tint = [color[0] * shade, color[1] * shade, color[2] * shade];
            vertices.push(Vertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                color: tint,
            });
            vertices.push(Vertex {
                position: (end * 0.9).into(),
                normal: [0.0, 1.0, 0.0],
                color: tint,
            });
            indices.extend_from_slice(&[base, base + 1]);
        }
    }

    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cursor at a point that no mirror plane passes through, so a mirror is
    /// always distinguishable from the original.
    fn off_axis() -> BrushCursor {
        BrushCursor {
            position: [0.3, 0.5, 0.7],
            normal: [0.0, 0.0, 1.0],
            radius: 0.1,
            mirrored: false,
        }
    }

    #[test]
    fn no_symmetry_leaves_one_ring() {
        let cursors = mirrored_cursors(off_axis(), [false; 3]);
        assert_eq!(cursors.len(), 1);
        assert!(
            !cursors[0].mirrored,
            "the pointer's own ring is not a mirror"
        );
    }

    #[test]
    fn each_axis_doubles_the_rings() {
        // A dab under symmetry lands in 2^n places, and the cursor has to show
        // all of them or it is under-reporting what the click will do.
        for (symmetry, expected) in [
            ([true, false, false], 2),
            ([true, true, false], 4),
            ([true, true, true], 8),
        ] {
            let cursors = mirrored_cursors(off_axis(), symmetry);
            assert_eq!(
                cursors.len(),
                expected,
                "{symmetry:?} should place {expected} rings"
            );
        }
    }

    #[test]
    fn a_mirror_is_the_reflection_through_the_origin_plane() {
        // The document sets its layer mirror at offset 0.0, so the planes pass
        // through the origin. A cursor mirrored anywhere else would show the
        // stroke landing where it does not.
        let cursors = mirrored_cursors(off_axis(), [true, false, false]);
        let mirror = cursors
            .iter()
            .find(|c| c.mirrored)
            .expect("a mirror was requested");

        assert_eq!(mirror.position, [-0.3, 0.5, 0.7]);
        assert_eq!(mirror.normal, [-0.0, 0.0, 1.0]);
        assert_eq!(
            mirror.radius,
            off_axis().radius,
            "a mirror is the same size"
        );
    }

    #[test]
    fn exactly_one_ring_is_the_pointer() {
        let cursors = mirrored_cursors(off_axis(), [true, true, true]);
        let originals = cursors.iter().filter(|c| !c.mirrored).count();
        assert_eq!(
            originals, 1,
            "the ring under the hand must be tellable from its mirrors"
        );
    }

    #[test]
    fn mirrors_are_drawn_dimmer_than_the_pointer() {
        let (pointer, _) = cursor_geometry(off_axis());
        let (mirror, _) = cursor_geometry(off_axis().mirror(0));

        let brightness = |v: &[Vertex]| v[0].color.iter().sum::<f32>();
        assert!(
            brightness(&mirror) < brightness(&pointer),
            "a mirror must not compete with the ring under the hand"
        );
    }

    #[test]
    fn every_ring_is_drawn() {
        // The rings share one mesh, so the indices of the later ones have to be
        // rebased. Getting that wrong draws the first ring several times.
        let one = mirrored_cursors(off_axis(), [false; 3]);
        let four = mirrored_cursors(off_axis(), [true, true, false]);

        let count = |cursors: &[BrushCursor]| {
            cursors
                .iter()
                .map(|c| cursor_geometry(*c).0.len())
                .sum::<usize>()
        };
        assert_eq!(count(&four), 4 * count(&one));

        let positions: Vec<[f32; 3]> = four.iter().map(|c| c.position).collect();
        for (i, a) in positions.iter().enumerate() {
            for b in positions.iter().skip(i + 1) {
                assert_ne!(a, b, "two rings landed in the same place");
            }
        }
    }

    #[test]
    fn every_symmetry_plane_is_drawn() {
        let one = overlay_geometry(
            Overlays {
                grid: false,
                symmetry_planes: [true, false, false],
            },
            1.0,
        );
        let all = overlay_geometry(
            Overlays {
                grid: false,
                symmetry_planes: [true; 3],
            },
            1.0,
        );
        assert!(!one.0.is_empty(), "a requested plane was not drawn");
        assert_eq!(
            all.0.len(),
            3 * one.0.len(),
            "planes beyond the first were dropped"
        );
    }

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
            "sd_sphere",
            "sdsphere",
            "smin",
            "smooth_min",
            "sdbox",
            "sd_box",
            "signed_distance",
            "raymarch",
            "sphere_trace",
            "ctape_eval",
        ] {
            assert!(
                !shader.contains(forbidden),
                "the viewport shader contains `{forbidden}`, which means it is \
                 evaluating the field instead of drawing the mesh the engine produced"
            );
        }
    }

    #[test]
    fn the_cursor_ring_lies_in_the_surface_plane() {
        let cursor = BrushCursor {
            position: [0.0, 1.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            radius: 0.5,
            mirrored: false,
        };
        let (vertices, indices) = cursor_geometry(cursor);
        assert!(!vertices.is_empty());
        assert_eq!(indices.len() % 2, 0, "line geometry needs index pairs");

        // Every ring point must sit at the radius from the centre, in the
        // plane the normal defines.
        let centre = Vec3::from(cursor.position);
        for vertex in vertices.iter().take(48) {
            let offset = Vec3::from(vertex.position) - centre;
            let in_plane = offset - Vec3::Y * offset.dot(Vec3::Y);
            assert!(
                (in_plane.length() - cursor.radius).abs() < 1e-3,
                "a ring point sits at {} rather than the radius {}",
                in_plane.length(),
                cursor.radius
            );
        }
    }

    #[test]
    fn a_degenerate_normal_still_produces_a_ring() {
        // A pick can report a zero normal; the cursor must not vanish or
        // produce NaN geometry because of it.
        let (vertices, _) = cursor_geometry(BrushCursor {
            position: [0.0; 3],
            normal: [0.0; 3],
            radius: 0.2,
            mirrored: false,
        });
        assert!(vertices
            .iter()
            .all(|v| v.position.iter().all(|c| c.is_finite())));
    }

    #[test]
    fn the_gizmo_draws_three_axes_in_both_directions() {
        let (vertices, indices) = gizmo_geometry();
        // Three axes, positive and negative, two vertices each.
        assert_eq!(vertices.len(), 12);
        assert_eq!(indices.len(), 12);

        // Each axis must be distinguishable by hue, or the gizmo reports
        // nothing a glance can read.
        let hues: Vec<[f32; 3]> = vertices
            .chunks_exact(2)
            .step_by(2)
            .map(|pair| pair[0].color)
            .collect();
        for (i, a) in hues.iter().enumerate() {
            for b in hues.iter().skip(i + 1) {
                assert_ne!(a, b, "two gizmo axes share a colour");
            }
        }
    }

    #[test]
    fn overlays_produce_line_geometry_only_when_asked() {
        let (none, _) = overlay_geometry(
            Overlays {
                grid: false,
                symmetry_planes: [false; 3],
            },
            1.0,
        );
        assert!(
            none.is_empty(),
            "overlays were built when none were requested"
        );

        let (grid, grid_indices) = overlay_geometry(
            Overlays {
                grid: true,
                symmetry_planes: [false; 3],
            },
            1.0,
        );
        assert!(!grid.is_empty());
        assert_eq!(grid_indices.len() % 2, 0, "line geometry needs index pairs");

        let (both, _) = overlay_geometry(
            Overlays {
                grid: true,
                symmetry_planes: [true, false, false],
            },
            1.0,
        );
        assert!(
            both.len() > grid.len(),
            "adding the symmetry plane produced no extra geometry"
        );
    }
}
