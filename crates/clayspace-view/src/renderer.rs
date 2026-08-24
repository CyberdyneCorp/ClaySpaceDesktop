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
use clayspace_model::{GizmoHandle, GizmoMode};

/// One vertex, in the layout the shader and the engine's copy both use.
///
/// `position` at 0, `normal` at 12, `color` at 24, `mask` at 36, stride 40. The
/// engine writes the first three directly into a mapped buffer at these
/// offsets, which is why the layout is stated once and shared rather than
/// described in two places — and why `mask` is last: the engine's copy names
/// the offsets it writes and leaves the rest of the stride alone.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    /// How frozen this vertex is, 0 to 1.
    ///
    /// An attribute of its own rather than a darkened `color`, because the two
    /// are different things: colour modulates the material and is gated on the
    /// mesh actually carrying vertex colours, while a mask has to read on a
    /// surface that carries none — which is every SDF surface.
    pub mask: f32,
}

impl Vertex {
    pub const STRIDE: usize = std::mem::size_of::<Self>();
    pub const POSITION_OFFSET: usize = 0;
    pub const NORMAL_OFFSET: usize = 12;
    pub const COLOR_OFFSET: usize = 24;
    pub const MASK_OFFSET: usize = 36;

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
                wgpu::VertexAttribute {
                    offset: Self::MASK_OFFSET as u64,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
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

/// The lattice cage, as the viewport draws it.
///
/// Positions rather than a box and divisions, because the whole point of a
/// cage is that its points have been *moved* — a box would draw the cage as it
/// was before the sculptor touched it.
pub struct LatticeView<'a> {
    /// Every control point, x fastest.
    pub points: &'a [[f32; 3]],
    /// Index pairs joined by a cage edge.
    pub edges: &'a [(u32, u32)],
    /// Which control points are in hand.
    pub selected: &'a [usize],
    /// The manipulator on that selection, when there is one.
    pub gizmo: Option<GizmoView>,
    /// How big a control point handle is, in world units.
    ///
    /// Handed in rather than fixed, because a cage around a thumbnail and one
    /// around a bust want the same handle *on screen* and the model's units do
    /// not know which it is.
    pub handle: f32,
}

/// The manipulator on the selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoView {
    /// Where it sits: the middle of the selection.
    pub pivot: [f32; 3],
    pub mode: GizmoMode,
    /// How long an axis is, in world units.
    ///
    /// Handed in for the reason the control-point handle is: a manipulator on
    /// a thumbnail and one on a bust want the same size to the hand, and the
    /// model's units do not know which it is.
    pub reach: f32,
    /// The handle under the pointer or being dragged, drawn brighter.
    pub hovered: Option<GizmoHandle>,
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
    /// The mesh layers' own edges, drawn over them.
    wire_pipeline: wgpu::RenderPipeline,
    /// Those edges, as a line list over `mesh_layers`' own vertices.
    ///
    /// An index buffer rather than a mesh: the positions are the ones already
    /// uploaded, and duplicating them to draw lines over them would cost a
    /// second copy of every vertex for no new information.
    wire_indices: wgpu::Buffer,
    wire_index_count: u32,
    wire_capacity: usize,
    /// Whether to draw them.
    polyframe: bool,
    /// Depth in, occlusion out.
    ao_pipeline: wgpu::RenderPipeline,
    /// Occlusion in, multiplied onto the resolved colour.
    composite_pipeline: wgpu::RenderPipeline,
    /// Both bind textures the framebuffer owns and the framebuffer is rebuilt
    /// on every resize, so their groups are made per frame from these rather
    /// than held. A bind group is a descriptor write; the alternative is a
    /// cache keyed by a texture identity wgpu does not expose.
    ao_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    ao_buffer: wgpu::Buffer,
    /// Whether the occlusion passes run.
    ///
    /// A switch rather than a constant because it is the only way to see what
    /// it is doing: the passes read the frame's own depth, so there is nothing
    /// to compare a capture against except the same capture without them.
    occlusion: bool,
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
    /// The mesh layers, drawn beside the surface.
    ///
    /// A second buffer rather than part of the surface: a mesh layer has no
    /// bricks, so it never enters the per-key storage the surface is
    /// reassembled from, and the two are rebuilt by different things.
    mesh_layers: GpuMesh,
    cursor_mesh: GpuMesh,
    /// The ZSphere rig, drawn over the surface it skins.
    armature_mesh: GpuMesh,
    /// The lattice cage's edges and control-point handles.
    lattice_mesh: GpuMesh,
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

        // The polyframe. The overlay's vertex stage — it is the same vertex
        // buffer, read the same way — with a fragment that draws ink rather
        // than the vertex colour, and a depth bias so the lines sit in front
        // of the very triangles they outline instead of fighting them.
        let wire_pipeline = make_line_pipeline(
            gpu,
            &layout,
            &shader,
            format,
            "overlay_vs",
            "wire_fs",
            wgpu::DepthBiasState {
                // Toward the camera. Depth is reversed nowhere here, so a
                // negative constant is nearer; the slope term is what keeps a
                // steeply-angled triangle's edge from sinking into it.
                constant: -2,
                slope_scale: -1.0,
                clamp: 0.0,
            },
        );

        // The occlusion pass and the composite that multiplies it on. Their
        // own module: they bind a depth texture and a uniform of their own, so
        // they share no layout with the scene.
        let ao_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("ao"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ao.wgsl").into()),
            });
        let ao_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ao"),
                entries: &[
                    uniform_entry(0),
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: true,
                        },
                        count: None,
                    },
                ],
            });
        let composite_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("ao composite"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    }],
                });
        let ao_pipeline = make_fullscreen_pipeline(
            gpu,
            &gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("ao"),
                    bind_group_layouts: &[&ao_layout],
                    push_constant_ranges: &[],
                }),
            &ao_shader,
            "ao_fs",
            Framebuffer::OCCLUSION_FORMAT,
            None,
        );
        // `src * dst`, which is what "multiply this onto what is there"
        // is — and it is why the pass needs no copy of the colour it darkens.
        let composite_pipeline = make_fullscreen_pipeline(
            gpu,
            &gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("ao composite"),
                    bind_group_layouts: &[&composite_layout],
                    push_constant_ranges: &[],
                }),
            &ao_shader,
            "composite_fs",
            format,
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Dst,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::REPLACE,
            }),
        );
        let ao_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ao"),
            size: std::mem::size_of::<AoUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
            wire_pipeline,
            wire_indices: empty_buffer(gpu, "polyframe", wgpu::BufferUsages::INDEX),
            wire_index_count: 0,
            wire_capacity: 0,
            polyframe: false,
            membrane_pipeline,
            ao_pipeline,
            composite_pipeline,
            ao_layout,
            composite_layout,
            ao_buffer,
            occlusion: true,
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
            mesh_layers: GpuMesh::new(gpu),
            cursor_mesh: GpuMesh::new(gpu),
            armature_mesh: GpuMesh::new(gpu),
            lattice_mesh: GpuMesh::new(gpu),
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

    /// The lattice cage, drawn over the form it wraps.
    ///
    /// Lines for the cage and a small box at every control point, in the same
    /// overlay pass the rig uses: both are scaffolding rather than clay, and
    /// scaffolding that is occluded by the thing it annotates is not
    /// scaffolding.
    pub fn set_lattice(&mut self, gpu: &Gpu, view: LatticeView<'_>) {
        let (vertices, indices) = lattice_geometry(view);
        self.lattice_mesh.upload(gpu, &vertices, &indices);
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

    /// Whether the surface is darkened where it closes in on itself.
    pub fn set_occlusion(&mut self, on: bool) {
        self.occlusion = on;
    }

    pub fn occlusion(&self) -> bool {
        self.occlusion
    }

    /// The triangles of the document's mesh layers.
    ///
    /// Drawn with the surface pipeline and the same material, because a mesh
    /// layer *is* surface as far as a sculptor is concerned — it is only the
    /// route it took to get here that differs.
    pub fn set_mesh_layers(&mut self, gpu: &Gpu, vertices: &[Vertex], indices: &[u32]) {
        self.mesh_layers.upload(gpu, vertices, indices);
        self.upload_edges(gpu, indices);
    }

    /// Whether the mesh layers are drawn with their own edges over them.
    ///
    /// ZBrush calls it the polyframe, and it answers the one question a
    /// wireframe is for: how much geometry is actually there. A sculptor
    /// deciding whether a mesh wants retopology is reading its density, and a
    /// shaded surface hides exactly that.
    pub fn set_polyframe(&mut self, on: bool) {
        self.polyframe = on;
    }

    /// The unique edges of a triangle list, as a line list.
    ///
    /// Deduplicated, and not only to halve the buffer: the lines are drawn
    /// translucent, so an edge shared by two triangles and emitted twice is
    /// blended twice and comes out darker than a boundary edge. A wireframe
    /// where the interior reads heavier than the silhouette is backwards.
    fn upload_edges(&mut self, gpu: &Gpu, indices: &[u32]) {
        let mut seen = std::collections::HashSet::with_capacity(indices.len());
        let mut edges: Vec<u32> = Vec::with_capacity(indices.len());
        for triangle in indices.chunks_exact(3) {
            for (a, b) in [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ] {
                // Ordered, so the same edge reached from either of its two
                // triangles is the same key.
                let key = if a < b { (a, b) } else { (b, a) };
                if seen.insert(key) {
                    edges.push(key.0);
                    edges.push(key.1);
                }
            }
        }

        self.wire_index_count = edges.len() as u32;
        if edges.is_empty() {
            return;
        }
        if edges.len() > self.wire_capacity {
            self.wire_indices = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("polyframe"),
                size: (edges.len() * 4) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.wire_capacity = edges.len();
        }
        gpu.queue
            .write_buffer(&self.wire_indices, 0, bytemuck::cast_slice(&edges));
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

        // Multisampled where the device allows it: the scene is drawn into the
        // framebuffer's own target and resolved into `target`, which is what
        // egui then paints the interface onto.
        let (attachment, resolve_target) = framebuffer.attachment(target);

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("viewport"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: attachment,
                    resolve_target,
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

            // The mesh layers, in the same pass and with the same pipeline, so
            // they take the same material, the same depth and the same
            // occlusion as everything else. Drawn after the surface only
            // because the depth test settles which is in front.
            if !self.mesh_layers.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, self.mesh_layers.vertices.slice(..));
                pass.set_index_buffer(
                    self.mesh_layers.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.mesh_layers.index_count, 0, 0..1);

                // And its edges over it, when the polyframe is on. The same
                // vertex buffer, read as a line list through its own indices.
                if self.polyframe && self.wire_index_count > 0 {
                    pass.set_pipeline(&self.wire_pipeline);
                    pass.set_index_buffer(self.wire_indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..self.wire_index_count, 0, 0..1);
                }
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

            if !self.lattice_mesh.is_empty() {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_vertex_buffer(0, self.lattice_mesh.vertices.slice(..));
                pass.set_index_buffer(
                    self.lattice_mesh.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.lattice_mesh.index_count, 0, 0..1);
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

        self.occlude(
            gpu,
            &mut encoder,
            target,
            framebuffer,
            camera,
            aspect,
            scene,
        );
        gpu.queue.submit(Some(encoder.finish()));
    }

    /// Darkens what the surface closes in on, from the depth it just wrote.
    ///
    /// Two passes after the scene: occlusion into the framebuffer's own
    /// single-channel target, then a blurred multiply onto the resolved
    /// colour. Nothing here reads that colour — the multiply is the blend
    /// state — so there is no copy of the frame and no third target.
    ///
    /// Skipped where the device would not multisample. The occlusion pass
    /// binds the depth buffer as `texture_depth_multisampled_2d`, and a
    /// single-sampled texture cannot be bound to that; see
    /// [`Framebuffer::occlusion_view`].
    #[allow(clippy::too_many_arguments)]
    fn occlude(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        framebuffer: &Framebuffer,
        camera: &Camera,
        aspect: f32,
        scene: [f32; 4],
    ) {
        if !self.occlusion {
            return;
        }
        let Some(occlusion) = framebuffer.occlusion_view() else {
            return;
        };
        let projection = camera.projection(aspect);
        gpu.queue.write_buffer(
            &self.ao_buffer,
            0,
            bytemuck::bytes_of(&AoUniform {
                projection: projection.to_cols_array_2d(),
                inverse_projection: projection.inverse().to_cols_array_2d(),
                viewport: scene,
                params: [AO_RADIUS, AO_INTENSITY, AO_BIAS, AO_SAMPLES],
            }),
        );

        let ao_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ao"),
            layout: &self.ao_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.ao_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(framebuffer.depth_view()),
                },
            ],
        });
        let composite_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ao composite"),
            layout: &self.composite_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(occlusion),
            }],
        });

        {
            // Cleared to white rather than loaded: outside the scene's
            // rectangle nothing is occluded, and white is what the composite
            // reads as "leave this alone" when its blur reaches over the edge.
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("occlusion"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: occlusion,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_viewport(scene[0], scene[1], scene[2], scene[3], 0.0, 1.0);
            pass.set_pipeline(&self.ao_pipeline);
            pass.set_bind_group(0, &ao_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("occlusion composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Loaded, because this darkens the frame that is
                        // already there rather than drawing one.
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_viewport(scene[0], scene[1], scene[2], scene[3], 0.0, 1.0);
            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, &composite_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
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

/// What the occlusion pass needs to turn a depth buffer into a shadowing term.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct AoUniform {
    projection: [[f32; 4]; 4],
    inverse_projection: [[f32; 4]; 4],
    viewport: [f32; 4],
    params: [f32; 4],
}

/// How far an occluder can be and still count, in view units.
///
/// A world-space radius rather than a screen-space one, so a fold darkens by
/// how deep it is rather than by how much of the window it happens to cover.
/// Tuned against the reference form, whose starting sphere has radius 1.
const AO_RADIUS: f32 = 0.08;
/// How much of the surface's own colour full occlusion takes away.
const AO_INTENSITY: f32 = 0.85;
/// The depth difference below which an occluder is the surface itself.
///
/// Without it a flat surface occludes itself everywhere, from the difference
/// between a sample's own depth and the depth of the pixel it projects to.
const AO_BIAS: f32 = 0.004;
/// Samples per pixel. Sixteen is where the noise stops changing what the
/// composite's blur produces.
const AO_SAMPLES: f32 = 16.0;

/// A pipeline for a pass with no geometry and no depth.
///
/// Single-sampled whatever the scene is: both of these run over the *resolved*
/// target, after the scene has been resolved into it.
fn make_fullscreen_pipeline(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    fs: &str,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("fullscreen_vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fs),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
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
    // Read from the same place the framebuffer reads it, so the two cannot
    // disagree — a pipeline whose sample count differs from its attachment's
    // is a validation error at draw time rather than at creation.
    let samples = gpu.sample_count(format);
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
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            multiview: None,
            cache: None,
        })
}

/// A line pipeline with a depth bias, for geometry drawn *over* a surface.
///
/// Apart from `make_pipeline` only for the bias: a wireframe shares its
/// vertices with the triangles it outlines, so without one every line lands on
/// exactly the same depth as the surface and the two flicker against each
/// other pixel by pixel.
fn make_line_pipeline(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    vs: &str,
    fs: &str,
    bias: wgpu::DepthBiasState,
) -> wgpu::RenderPipeline {
    let samples = gpu.sample_count(format);
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs),
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
                topology: wgpu::PrimitiveTopology::LineList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Framebuffer::DEPTH_FORMAT,
                // Read but not written: the lines are ink over the surface,
                // and writing their depth would let them occlude each other.
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias,
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
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
            mask: 0.0,
        });
        vertices.push(Vertex {
            position: b.into(),
            normal: [0.0, 1.0, 0.0],
            color,
            mask: 0.0,
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
            mask: 0.0,
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
                mask: 0.0,
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
                    mask: 0.0,
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
/// The cage: a line along every edge, and a box at every control point.
///
/// Line topology, drawn by the overlay pipeline. The handles are boxes rather
/// than spheres because a box reads as a *handle* — something to grab — where a
/// sphere at this size reads as a bead on a wire, and because twelve lines cost
/// what one sphere's ring costs.
/// The three axis colours, which every application that has a manipulator
/// spells the same way: x red, y green, z blue.
const AXIS_COLOURS: [[f32; 3]; 3] = [[0.85, 0.24, 0.24], [0.36, 0.76, 0.30], [0.28, 0.45, 0.88]];

/// The manipulator: three axes and, where the mode has one, a centre.
///
/// Line topology like the cage, and shapes rather than colours alone carry the
/// meaning — an arrow slides, a ring turns, a box scales — because a person
/// reaching for a handle is not reading a legend, and because the three
/// colours are the one part of this a colour-blind sculptor cannot use.
fn gizmo_geometry_for(view: GizmoView, segment: &mut impl FnMut(Vec3, Vec3, [f32; 3])) {
    const RING_SEGMENTS: usize = 40;
    let pivot = Vec3::from(view.pivot);
    let lit = |handle: GizmoHandle, base: [f32; 3]| {
        if view.hovered == Some(handle) {
            [1.0, 0.85, 0.4]
        } else {
            base
        }
    };

    for index in 0..3 {
        let handle = GizmoHandle::Axis(index);
        let colour = lit(handle, AXIS_COLOURS[index]);
        let mut unit = Vec3::ZERO;
        unit[index] = 1.0;
        let (u, v) = ((index + 1) % 3, (index + 2) % 3);
        let mut across = Vec3::ZERO;
        across[u] = 1.0;
        let mut other = Vec3::ZERO;
        other[v] = 1.0;

        match view.mode {
            GizmoMode::Rotate => {
                // A ring in the plane perpendicular to the axis: what turns
                // about it.
                for step in 0..RING_SEGMENTS {
                    let angle =
                        |at: usize| at as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
                    let at = |a: f32| pivot + (across * a.cos() + other * a.sin()) * view.reach;
                    segment(at(angle(step)), at(angle(step + 1)), colour);
                }
            }
            mode => {
                let tip = pivot + unit * view.reach;
                segment(pivot, tip, colour);
                if mode == GizmoMode::Move {
                    // An arrowhead: four lines back from the tip, which reads
                    // as a direction from any angle.
                    let head = view.reach * 0.18;
                    for corner in 0..4 {
                        let (s, c) = (corner as f32 / 4.0 * std::f32::consts::TAU).sin_cos();
                        segment(
                            tip,
                            tip - unit * head + (across * c + other * s) * head * 0.5,
                            colour,
                        );
                    }
                } else {
                    // A box: what scales.
                    let box_size = view.reach * 0.08;
                    cube(tip, box_size, colour, segment);
                }
            }
        }
    }

    if let Some(handle) = GizmoHandle::all_for(view.mode)
        .into_iter()
        .find(|handle| *handle == GizmoHandle::Centre)
    {
        // A square facing no particular way, drawn on all three planes so it
        // reads as a centre from any angle rather than vanishing edge-on.
        let colour = lit(handle, [0.82, 0.78, 0.42]);
        let size = view.reach * 0.14;
        cube(pivot, size, colour, segment);
    }
}

/// The twelve edges of a cube, spelled as the four along each axis.
fn cube(centre: Vec3, size: f32, colour: [f32; 3], segment: &mut impl FnMut(Vec3, Vec3, [f32; 3])) {
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        for corner in 0..4 {
            let mut from = [0.0f32; 3];
            from[u] = if corner & 1 == 0 { -size } else { size };
            from[v] = if corner & 2 == 0 { -size } else { size };
            from[axis] = -size;
            let mut to = from;
            to[axis] = size;
            segment(centre + Vec3::from(from), centre + Vec3::from(to), colour);
        }
    }
}

fn lattice_geometry(view: LatticeView<'_>) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut segment = |from: Vec3, to: Vec3, color: [f32; 3]| {
        let base = vertices.len() as u32;
        for position in [from, to] {
            vertices.push(Vertex {
                position: position.into(),
                normal: [0.0, 1.0, 0.0],
                color,
                mask: 0.0,
            });
        }
        indices.push(base);
        indices.push(base + 1);
    };

    // The cage itself, quiet: it is a frame of reference, and a bright one
    // would compete with the form it is wrapped around.
    const CAGE: [f32; 3] = [0.62, 0.45, 0.28];
    const POINT: [f32; 3] = [0.78, 0.60, 0.38];
    const SELECTED: [f32; 3] = [1.0, 0.72, 0.30];

    for (from, to) in view.edges {
        let (Some(a), Some(b)) = (
            view.points.get(*from as usize),
            view.points.get(*to as usize),
        ) else {
            continue;
        };
        segment(Vec3::from(*a), Vec3::from(*b), CAGE);
    }

    for (index, point) in view.points.iter().enumerate() {
        let selected = view.selected.binary_search(&index).is_ok();
        let color = if selected { SELECTED } else { POINT };
        // Bigger when it is the one in hand, so which point is being dragged
        // is legible without reading the colour — which a sculptor looking at
        // the form is not doing.
        let size = view.handle * if selected { 1.6 } else { 1.0 };
        let centre = Vec3::from(*point);
        cube(centre, size, color, &mut segment);
    }

    // The manipulator last, so it draws over the cage it acts on.
    if let Some(gizmo) = view.gizmo {
        gizmo_geometry_for(gizmo, &mut segment);
    }

    (vertices, indices)
}

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
                mask: 0.0,
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
                mask: 0.0,
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
                mask: 0.0,
            });
            vertices.push(Vertex {
                position: (end * 0.9).into(),
                normal: [0.0, 1.0, 0.0],
                color: tint,
                mask: 0.0,
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
        assert_eq!(Vertex::STRIDE, 40);
        assert_eq!(Vertex::POSITION_OFFSET, 0);
        assert_eq!(Vertex::NORMAL_OFFSET, 12);
        assert_eq!(Vertex::COLOR_OFFSET, 24);
        // Last on purpose: the engine's copy names the three offsets it
        // writes and leaves the rest of the stride alone, so the mask can be
        // written by us either side of that call without being overwritten.
        assert_eq!(Vertex::MASK_OFFSET, 36);
    }

    #[test]
    fn no_field_math_in_shaders() {
        // The whole point of meshing on the engine side is that the shader
        // does not re-implement the field. If one of these appears here, the
        // drift this project is built to avoid has started.
        // Both of them. The occlusion pass reads the depth the mesh wrote and
        // is exactly the kind of pass that would be tempting to write a field
        // march into instead.
        let shader = format!(
            "{}{}",
            include_str!("shaders/matcap.wgsl"),
            include_str!("shaders/ao.wgsl")
        )
        .to_lowercase();
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
                "a viewport shader contains `{forbidden}`, which means it is \
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
