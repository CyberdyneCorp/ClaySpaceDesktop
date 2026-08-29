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
use clayspace_model::{GizmoHandle, GizmoMode, LayerKey, SurfaceOpacity};

/// Which run of the carried buffer belongs to which subtool.
///
/// The voxel and mesh layers arrive as one concatenated buffer, so this is the
/// only thing that says where one subtool's triangles end and the next one's
/// begin — and therefore the only thing that lets the active one be drawn
/// differently from the rest. One draw call per span rather than an instancing
/// scheme: a scene holds a handful of subtools, and a handful of draws is
/// noise beside the buffer they share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshSpan {
    pub layer: LayerKey,
    /// Positions into the index buffer, which is what a draw call takes.
    pub indices: std::ops::Range<u32>,
}

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

    /// The corners of a box containing every vertex.
    ///
    /// Here rather than beside either caller: the renderer takes it to frame
    /// what it just uploaded and the composition root takes it to accumulate a
    /// scene's bounds, and two folds over the same attribute agreeing by hand
    /// is one edit away from not agreeing. Arrays rather than `Vec3` because
    /// that is the vocabulary the vertex itself is in, and the callers that
    /// want glam already convert at their own boundary.
    pub fn bounds(vertices: &[Self]) -> Option<([f32; 3], [f32; 3])> {
        let first = vertices.first()?.position;
        Some(vertices.iter().fold((first, first), |(min, max), v| {
            let at = v.position;
            (
                [min[0].min(at[0]), min[1].min(at[1]), min[2].min(at[2])],
                [max[0].max(at[0]), max[1].max(at[1]), max[2].max(at[2])],
            )
        }))
    }

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
    /// How opaque the surface is drawn, in x. The other three pad the struct
    /// out to the sixteen bytes a uniform is aligned to, which is why the
    /// opacity is not a bare f32.
    ghost: [f32; 4],
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

    /// Whether buffers for this many vertices and indices are within what the
    /// device will create.
    pub fn fits(gpu: &Gpu, vertices: usize, indices: usize) -> bool {
        let ceiling = gpu.max_buffer_size();
        // Saturating, because the question is asked with counts a device
        // reporting a ceiling in the exabytes makes wrap in release builds.
        (vertices as u64).saturating_mul(Vertex::STRIDE as u64) <= ceiling
            && (indices as u64).saturating_mul(4) <= ceiling
    }

    /// Replaces the whole mesh.
    ///
    /// Refused, and left as it was, where the device could not hold it: an
    /// oversized `create_buffer` is a validation error, and a mesh drawn stale
    /// is better than a session lost.
    pub fn upload(&mut self, gpu: &Gpu, vertices: &[Vertex], indices: &[u32]) {
        if !Self::fits(gpu, vertices.len(), indices.len()) {
            eprintln!(
                "a mesh of {} vertices is more than the graphics device can hold; it was not drawn",
                vertices.len()
            );
            return;
        }
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
        self.set_bounds(Vertex::bounds(vertices));
    }

    /// Allocates buffers of a fixed size without writing anything.
    ///
    /// The incremental path needs the addresses to exist before it knows what
    /// goes in them, which `upload` cannot offer — it sizes the buffers to the
    /// data it is given.
    ///
    /// Returns whether it did. A reservation the device cannot hold is refused
    /// with the buffers left as they were, so the caller can draw coarser
    /// instead of the process ending in a validation panic.
    #[must_use]
    pub fn reserve(&mut self, gpu: &Gpu, vertices: usize, indices: usize) -> bool {
        if !Self::fits(gpu, vertices, indices) {
            return false;
        }
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
        true
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
    /// The box a selected placed object occupies, when one is selected.
    ///
    /// Drawn because a subtracting object is *inside* the form: what a
    /// sculptor sees of a cylinder bored through a head is the hole, and the
    /// cylinder itself is behind the surface where nothing shows it. Without
    /// an outline, aiming one means dragging a manipulator and inferring the
    /// shape from the cavity that appears.
    ///
    /// A box rather than the shape's own silhouette, which would mean marching
    /// a primitive on the interface thread every frame — this is a frame of
    /// reference, not a preview.
    pub outline: Option<([f32; 3], [f32; 3])>,
    /// The box the active SDF subtool occupies, when the cue applies to one.
    ///
    /// The merged surface is the hard union of every visible SDF layer and the
    /// engine attributes no triangle to the layer it came from, so an active
    /// SDF subtool cannot be tinted the way a carried one is. Its box is the
    /// cue instead: the same drawing a selected object gets, in its own colour,
    /// saying which of the forms in the union is the one a dab would land on.
    pub subtool_outline: Option<([f32; 3], [f32; 3])>,
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
    /// The handle under the pointer or being dragged, drawn brighter — with
    /// the operation it is performing, since one widget carries all three and
    /// an axis has an arrow, a ring and perhaps a box.
    pub hovered: Option<(GizmoMode, GizmoHandle)>,
    /// The direction from the pivot to the eye.
    ///
    /// What the outer ring lies perpendicular to, and what it turns about.
    /// Handed in rather than derived from the camera here, so the ring drawn
    /// and the ring dragged cannot disagree.
    pub view_axis: [f32; 3],
    /// Whether scale mode offers a box per axis.
    ///
    /// A cage does: it scales its own control points, and pulling the red box
    /// stretches in x alone. A placed object, a layer and a mesh do not —
    /// every transform in the engine's interface takes one scale factor and
    /// not three, so three boxes would be three handles for one number.
    ///
    /// Handed in rather than decided here for the same reason `view_axis` is:
    /// what is drawn and what can be dragged have to be the same set, and the
    /// caller is what knows which kind of thing is selected.
    pub per_axis_scale: bool,
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
    /// The overlay's lines, drawn whatever is in front of them.
    ///
    /// The cage, the curve's control polygon, an object's outline and the
    /// manipulator are scaffolding around the clay, and half of any of them
    /// is behind the surface: a subtracting object stands *inside* the form,
    /// and a manipulator sits on the middle of what it moves. Depth-tested,
    /// the manipulator on a placed sphere was three arrow tips poking out of
    /// it and nothing to grab — so this pipeline reads no depth at all, and
    /// the scaffolding is always where the hand expects it.
    scaffold_pipeline: wgpu::RenderPipeline,
    /// The manipulator's solid parts — arrowheads, boxes — drawn the same
    /// way: unlit vertex colour, no depth, but as triangles. An arrow drawn as
    /// four lines back from its tip reads as a direction; a cone reads as a
    /// handle, which is what ZBrush's and Blender's are.
    scaffold_solid_pipeline: wgpu::RenderPipeline,
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
    /// Which run of `mesh_layers` each subtool owns, in the order they were
    /// concatenated. Empty means "draw the buffer whole", which is what a
    /// caller that has nothing to cue wants.
    mesh_spans: Vec<MeshSpan>,
    /// The subtool a dab would land on, when it is one of the carried ones.
    active_subtool: Option<LayerKey>,
    /// The material the active subtool is drawn with.
    ///
    /// A second buffer and a second bind group rather than one buffer written
    /// twice: `Queue::write_buffer` is ordered against the *submission*, not
    /// against the draws inside it, so writing the tint between two draw calls
    /// would give both of them whichever value was written last.
    active_material_buffer: wgpu::Buffer,
    active_bind_group: wgpu::BindGroup,
    cursor_mesh: GpuMesh,
    /// The ZSphere rig, drawn over the surface it skins.
    armature_mesh: GpuMesh,
    /// The lattice cage's edges and control-point handles.
    lattice_mesh: GpuMesh,
    /// The manipulator's solid handles, as triangles.
    lattice_solid_mesh: GpuMesh,
    /// The translucent skin between the spheres, drawn while rigging.
    membrane_mesh: GpuMesh,
    membrane_pipeline: wgpu::RenderPipeline,
    /// The reference images, one quad a plane, and the pipeline that draws
    /// them.
    reference_pipeline: wgpu::RenderPipeline,
    /// Per plane: the quad, and the bind group carrying its texture.
    references: std::collections::BTreeMap<usize, (GpuMesh, wgpu::BindGroup)>,
    /// The surface drawn through, while a deformation cage is up.
    ghost_pipeline: wgpu::RenderPipeline,
    ghosted: bool,
    /// How opaque the surface is drawn, as the sculptor set it.
    ///
    /// Held apart from `ghosted`, which is the cage imposing its own ceiling:
    /// putting a cage up must not forget the dial, and taking it down must not
    /// silently make a deliberately faint surface solid again.
    surface_opacity: SurfaceOpacity,
    /// The rectangle of the frame the scene is drawn into, in physical pixels.
    scene_viewport: Option<[f32; 4]>,
    gizmo_mesh: GpuMesh,
    gizmo_camera_buffer: wgpu::Buffer,
    gizmo_bind_group: wgpu::BindGroup,
}

/// How far the active subtool's clay is carried toward the accent's hue.
///
/// Short of the whole way on purpose. The cue has to survive being looked past
/// — a sculptor reads the silhouette, not the colour — so it is a warmth the
/// eye picks up beside a neutral neighbour rather than a coat of paint.
const ACTIVE_TINT_STRENGTH: f32 = 0.45;

/// The multiplier the active subtool's material is drawn with.
///
/// The accent is the design's reserved colour for active tool state, and which
/// subtool a dab lands on is exactly that. A multiplier over the MatCap rather
/// than a replacement of it: the material carries the form's shading, and
/// overwriting it would say the active subtool is made of something else.
const ACTIVE_TINT: [f32; 3] = active_tint();

/// The accent's hue at full value, mixed back toward white.
///
/// Divided by its own strongest channel — red, which the accent's hex says it
/// is — so what survives is the ratio between the channels and not the accent's
/// darkness: multiplying the clay by the accent as stored took two thirds of
/// the value out of it, which reads as a shadow rather than as a cue.
const fn active_tint() -> [f32; 3] {
    let peak = palette::ACCENT[0];
    let mut tint = [0.0f32; 3];
    let mut channel = 0;
    while channel < 3 {
        let hue = palette::ACCENT[channel] / peak;
        tint[channel] = 1.0 - ACTIVE_TINT_STRENGTH * (1.0 - hue);
        channel += 1;
    }
    tint
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
        let active_material_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("active subtool material"),
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
        // And the active subtool the other way round: the scene's camera, its
        // own material.
        let active_bind_group = make_bind_group(
            gpu,
            &bind_group_layout,
            &camera_buffer,
            &active_material_buffer,
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
        let scaffold_pipeline = make_pipeline_with_depth(
            gpu,
            &layout,
            &shader,
            format,
            "overlay_vs",
            "overlay_fs",
            wgpu::PrimitiveTopology::LineList,
            false,
            wgpu::CompareFunction::Always,
        );
        let scaffold_solid_pipeline = make_pipeline_with_depth(
            gpu,
            &layout,
            &shader,
            format,
            "overlay_vs",
            "overlay_fs",
            wgpu::PrimitiveTopology::TriangleList,
            false,
            wgpu::CompareFunction::Always,
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
            // Uncalled: no back-face culling and so no depth write, which is
            // what lets the far half of the cage read through the form.
            ghost_pipeline: make_pipeline(
                gpu,
                &layout,
                &shader,
                format,
                "vs_main",
                "fs_ghost",
                wgpu::PrimitiveTopology::TriangleList,
                false,
            ),
            // Blended, unculled, and writing no depth. Drawn first and
            // leaving the depth buffer alone, a reference is *always* behind
            // the clay — including when the camera has swung round to the far
            // side of its plane, which is the whole point: a guide that
            // occludes the form it is guiding has stopped being a guide.
            // Unculled because the top plane's quad is seen from below as
            // often as from above.
            reference_pipeline: make_pipeline(
                gpu,
                &layout,
                &shader,
                format,
                "reference_vs",
                "reference_fs",
                wgpu::PrimitiveTopology::TriangleList,
                false,
            ),
            references: std::collections::BTreeMap::new(),
            ghosted: false,
            surface_opacity: SurfaceOpacity::SOLID,
            pipeline,
            overlay_pipeline,
            scaffold_pipeline,
            scaffold_solid_pipeline,
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
            mesh_spans: Vec::new(),
            active_subtool: None,
            active_material_buffer,
            active_bind_group,
            cursor_mesh: GpuMesh::new(gpu),
            armature_mesh: GpuMesh::new(gpu),
            lattice_mesh: GpuMesh::new(gpu),
            lattice_solid_mesh: GpuMesh::new(gpu),
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
        // The active subtool's group reads the same texture, so a material the
        // sculptor changed has to reach it too — the first version rebuilt only
        // the plain one and the tinted subtool kept the old MatCap.
        self.active_bind_group = make_bind_group(
            gpu,
            &self.bind_group_layout,
            &self.camera_buffer,
            &self.active_material_buffer,
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

    /// Places one reference image, or takes it away.
    ///
    /// `None` clears the plane. The corners come from the domain, which knows
    /// where a plane's axes are and keeps the picture's proportions — this
    /// places nothing itself.
    pub fn set_reference(&mut self, gpu: &Gpu, plane: usize, placed: Option<Reference<'_>>) {
        let Some(placed) = placed else {
            self.references.remove(&plane);
            return;
        };
        let Reference {
            pixels,
            width,
            height,
            corners,
            opacity,
        } = placed;
        let view = upload_reference(gpu, pixels, width, height);
        let bind_group = make_bind_group(
            gpu,
            &self.bind_group_layout,
            &self.camera_buffer,
            &self.material_buffer,
            &view,
            &self.sampler,
        );
        // The uv rides in the vertex colour's first two channels and the
        // opacity in the third, which is what `reference_vs` reads. The
        // alternative is an attribute on every vertex of every mesh in the
        // scene, paid by the surface to serve a quad.
        let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        let vertices: Vec<Vertex> = corners
            .iter()
            .zip(uv)
            .map(|(position, uv)| Vertex {
                position: *position,
                normal: [0.0, 1.0, 0.0],
                color: [uv[0], uv[1], opacity],
                mask: 0.0,
            })
            .collect();
        let mut mesh = GpuMesh::new(gpu);
        mesh.upload(gpu, &vertices, &[0, 1, 2, 0, 2, 3]);
        self.references.insert(plane, (mesh, bind_group));
    }

    /// The lattice cage, drawn over the form it wraps.
    ///
    /// Lines for the cage and a small box at every control point, in the same
    /// overlay pass the rig uses: both are scaffolding rather than clay, and
    /// scaffolding that is occluded by the thing it annotates is not
    /// scaffolding.
    pub fn set_lattice(&mut self, gpu: &Gpu, view: LatticeView<'_>) {
        let geometry = lattice_geometry(view);
        self.lattice_mesh
            .upload(gpu, &geometry.lines.0, &geometry.lines.1);
        self.lattice_solid_mesh
            .upload(gpu, &geometry.solids.0, &geometry.solids.1);
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
    /// The spans arrive with the buffer they describe rather than through a
    /// setter of their own, so a range can never outlive the indices it points
    /// into.
    pub fn set_mesh_layers(
        &mut self,
        gpu: &Gpu,
        vertices: &[Vertex],
        indices: &[u32],
        spans: &[MeshSpan],
    ) {
        self.mesh_layers.upload(gpu, vertices, indices);
        self.mesh_spans = spans.to_vec();
        self.upload_edges(gpu, indices);
    }

    /// Which subtool a dab would land on, for the cue to mark.
    ///
    /// Separate from the buffer because activation is a click and re-walking
    /// every visible grid to say the same triangles again would make choosing a
    /// subtool cost what sculpting one does. A key naming no span simply tints
    /// nothing, which is the honest answer when the active subtool is an SDF
    /// one — that cue is the outline instead.
    pub fn set_active_subtool(&mut self, layer: Option<LayerKey>) {
        self.active_subtool = layer;
    }

    /// Whether the surface is drawn through.
    ///
    /// On while a deformation cage is up: the sculptor is aiming at control
    /// points, and half of them are behind the form.
    /// How opaque the surface is drawn.
    ///
    /// A dial and not a switch, because the useful amount depends on what is
    /// behind the form: tracing a silhouette against a photograph wants a
    /// different number from reaching a cage's control points.
    pub fn set_surface_opacity(&mut self, opacity: SurfaceOpacity) {
        self.surface_opacity = opacity;
    }

    /// What the surface is actually drawn at, dial and cage together.
    fn drawn_opacity(&self) -> SurfaceOpacity {
        if self.ghosted {
            self.surface_opacity.and(SurfaceOpacity::CAGED)
        } else {
            self.surface_opacity
        }
    }

    pub fn set_ghosted(&mut self, on: bool) {
        self.ghosted = on;
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

    /// The carried layers, one draw per subtool.
    ///
    /// A caller that uploaded triangles without saying whose they are still has
    /// to see them, so an empty span list is the whole buffer in one draw —
    /// which is also what every frame did before there were subtools to tell
    /// apart.
    fn draw_carried(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.mesh_spans.is_empty() {
            pass.draw_indexed(0..self.mesh_layers.index_count, 0, 0..1);
            return;
        }
        for span in &self.mesh_spans {
            let material = if Some(span.layer) == self.active_subtool {
                &self.active_bind_group
            } else {
                &self.bind_group
            };
            pass.set_bind_group(0, material, &[]);
            pass.draw_indexed(span.indices.clone(), 0, 0..1);
        }
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
        let colored = if has_vertex_colors { 1.0 } else { 0.0 };
        // The effective opacity and not the dial: the cage imposes its own
        // ceiling, and writing the dial here would select the ghost pipeline
        // and then draw it solid.
        let ghost = [self.drawn_opacity().get(), 0.0, 0.0, 0.0];
        gpu.queue.write_buffer(
            &self.material_buffer,
            0,
            bytemuck::bytes_of(&MaterialUniform {
                tint: [1.0, 1.0, 1.0, colored],
                ghost,
            }),
        );
        gpu.queue.write_buffer(
            &self.active_material_buffer,
            0,
            bytemuck::bytes_of(&MaterialUniform {
                tint: [ACTIVE_TINT[0], ACTIVE_TINT[1], ACTIVE_TINT[2], colored],
                ghost,
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

            // The references first, and writing no depth, so everything else
            // is drawn over them whichever side of them the camera is on.
            for (mesh, bind_group) in self.references.values() {
                if mesh.is_empty() {
                    continue;
                }
                pass.set_pipeline(&self.reference_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
            // Back to the scene's own bindings, which the loop above replaced.
            pass.set_bind_group(0, &self.bind_group, &[]);

            // Through, while a cage is up or the sculptor has dialled the
            // surface back. One choice for both the surface and the mesh
            // layers: a document with one of each half solid and half ghosted
            // would read as two objects.
            let surface = if !self.drawn_opacity().is_solid() {
                &self.ghost_pipeline
            } else {
                &self.pipeline
            };

            if !mesh.is_empty() {
                pass.set_pipeline(surface);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }

            // The mesh layers, in the same pass and with the same pipeline, so
            // they take the same material, the same depth and the same
            // occlusion as everything else. Drawn after the surface only
            // because the depth test settles which is in front.
            if !self.mesh_layers.is_empty() {
                pass.set_pipeline(surface);
                pass.set_vertex_buffer(0, self.mesh_layers.vertices.slice(..));
                pass.set_index_buffer(
                    self.mesh_layers.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                self.draw_carried(&mut pass);
                // Back to the plain material, which a tinted span may have
                // replaced: everything after this belongs to no subtool.
                pass.set_bind_group(0, &self.bind_group, &[]);

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

            // The scaffolding — cage, curve, outline, manipulator — over
            // everything, whichever side of the surface it is on.
            if !self.lattice_mesh.is_empty() {
                pass.set_pipeline(&self.scaffold_pipeline);
                pass.set_vertex_buffer(0, self.lattice_mesh.vertices.slice(..));
                pass.set_index_buffer(
                    self.lattice_mesh.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.lattice_mesh.index_count, 0, 0..1);
            }
            // The solid handles last, over the shafts they cap.
            if !self.lattice_solid_mesh.is_empty() {
                pass.set_pipeline(&self.scaffold_solid_pipeline);
                pass.set_vertex_buffer(0, self.lattice_solid_mesh.vertices.slice(..));
                pass.set_index_buffer(
                    self.lattice_solid_mesh.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.lattice_solid_mesh.index_count, 0, 0..1);
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
    make_pipeline_with_depth(
        gpu,
        layout,
        shader,
        format,
        vs,
        fs,
        topology,
        cull,
        wgpu::CompareFunction::LessEqual,
    )
}

/// The same, choosing what the depth test compares with.
///
/// `LessEqual` is the ordinary case: a thing behind the surface is hidden by
/// it. `Always` is for scaffolding that has to be seen wherever it is.
#[allow(clippy::too_many_arguments)]
fn make_pipeline_with_depth(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    vs: &str,
    fs: &str,
    topology: wgpu::PrimitiveTopology,
    cull: bool,
    depth_compare: wgpu::CompareFunction,
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
                depth_compare,
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

/// One reference image, as the viewport is given it.
#[derive(Debug, Clone, Copy)]
pub struct Reference<'a> {
    /// RGBA, `width * height * 4` bytes.
    pub pixels: &'a [u8],
    pub width: u32,
    pub height: u32,
    /// Where the quad sits, bottom-left first and anticlockwise.
    pub corners: [[f32; 3]; 4],
    pub opacity: f32,
}

/// Puts a reference image on a texture.
fn upload_reference(gpu: &Gpu, pixels: &[u8], width: u32, height: u32) -> wgpu::TextureView {
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("reference"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // sRGB, like the matcap beside it: a photograph stored as sRGB and
            // sampled as linear comes out washed out, which on a reference
            // reads as the opacity being wrong.
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        pixels,
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
        // behind it. Four steps was still a lattice of orange across the
        // form on a running build, with the camera inside the plane's extent.
        // Two steps is the plane's outline and its two centre lines — the
        // mirror's axis where it meets the floor, and its edge — which says
        // "the mirror is here" and puts nothing across the clay. Six lines
        // can afford a little more light than forty: still a fifth of the
        // accent, nowhere near the active brush's ring.
        let color = palette::dimmed(palette::ACCENT, 0.22);
        let steps = 2;
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
fn gizmo_geometry_for(
    view: GizmoView,
    emit: &mut impl FnMut(Vec3, Vec3, [f32; 3]),
    triangle: &mut impl FnMut(Vec3, Vec3, Vec3, [f32; 3]),
) {
    const RING_SEGMENTS: usize = 40;
    let pivot = Vec3::from(view.pivot);

    // Drawn heavier than the cage it stands on. A line is one pixel wide
    // whatever the device, and a one-pixel manipulator over a shaded form is
    // a thing to squint for; ZBrush's is a handle. Each stroke is laid down
    // `HANDLE_WEIGHT` times, stepped *across itself in the screen plane* —
    // perpendicular both to the stroke and to the eye — so it widens the same
    // way from every angle, and a box's edges thicken rather than hatch.
    let eye = normalized(Vec3::from(view.view_axis)).unwrap_or(Vec3::Z);
    let step = view.reach * HANDLE_STEP;
    let mut segment = |from: Vec3, to: Vec3, colour: [f32; 3]| {
        // A stroke pointing straight at the eye has no across; any direction
        // in the screen plane widens it as well as another.
        let across = normalized(eye.cross(to - from)).unwrap_or_else(|| frame_about(eye).0);
        for i in 0..HANDLE_WEIGHT {
            let t = i as f32 - (HANDLE_WEIGHT - 1) as f32 * 0.5;
            let offset = across * (t * step);
            emit(from + offset, to + offset, colour);
        }
    };
    let lit = |operation: GizmoMode, handle: GizmoHandle, base: [f32; 3]| {
        if view.hovered == Some((operation, handle)) {
            [1.0, 0.85, 0.4]
        } else {
            base
        }
    };
    let ring = |centre: Vec3,
                across: Vec3,
                other: Vec3,
                radius: f32,
                colour: [f32; 3],
                segment: &mut dyn FnMut(Vec3, Vec3, [f32; 3])| {
        for step in 0..RING_SEGMENTS {
            let angle = |at: usize| at as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
            let at = |a: f32| centre + (across * a.cos() + other * a.sin()) * radius;
            segment(at(angle(step)), at(angle(step + 1)), colour);
        }
    };

    // One widget, every operation: ZBrush's Gizmo 3D. Along each axis an
    // arrow that slides, a ring that turns and — where a stretch can be
    // applied per axis — a box that scales, so the operation is chosen by the
    // handle grabbed rather than by a mode set first. Three modes drew three
    // different widgets once, and the chips became a step a sculptor had to
    // take before every move.
    for (operation, handle) in GizmoHandle::combined(view.per_axis_scale) {
        let Some(index) = handle.axis_index() else {
            continue;
        };
        let colour = lit(operation, handle, AXIS_COLOURS[index]);
        let mut unit = Vec3::ZERO;
        unit[index] = 1.0;
        let (u, v) = ((index + 1) % 3, (index + 2) % 3);
        let mut across = Vec3::ZERO;
        across[u] = 1.0;
        let mut other = Vec3::ZERO;
        other[v] = 1.0;
        match operation {
            GizmoMode::Move => {
                // A cone at the tip: a handle, not a hint of one. The shaft
                // stops where the cone starts so it does not show through the
                // base.
                let tip = pivot + unit * view.reach;
                let head = view.reach * 0.2;
                segment(pivot, tip - unit * head, colour);
                cone(tip, unit, head, head * 0.4, colour, triangle);
            }
            GizmoMode::Rotate => {
                // A ring in the plane perpendicular to the axis, inside the
                // arrows' reach so the two are told apart by radius as well as
                // by shape.
                ring(
                    pivot,
                    across,
                    other,
                    view.reach * RING_REACH,
                    colour,
                    &mut segment,
                );
            }
            GizmoMode::Scale => {
                // A box on the shaft, short of the ring.
                let at = pivot + unit * (view.reach * SCALE_BOX_REACH);
                solid_cube(at, view.reach * 0.07, colour, triangle);
            }
        }
    }

    // The centre: a solid block at the pivot, which reads as a centre from
    // any angle. What it does is the mode's — a slide, or a uniform scale.
    let centre_operation = GizmoHandle::centre_operation(view.mode);
    let colour = lit(centre_operation, GizmoHandle::Centre, CENTRE_COLOUR);
    solid_cube(pivot, view.reach * 0.12, colour, triangle);

    // The outer ring: ZBrush's, and the one a sculptor reaches for most.
    // Outside the arrows at `VIEW_RING_REACH` — among the axis rings it would
    // be a fourth thing to tell apart at the same radius, and the whole point
    // of this one is that it is the easy target. And the four corner brackets
    // that frame it in the screen plane: they say "this is the widget's
    // extent" and are grabbed by nothing.
    let (across, other) = frame_about(eye);
    let colour = lit(GizmoMode::Rotate, GizmoHandle::View, VIEW_RING_COLOUR);
    ring(
        pivot,
        across,
        other,
        view.reach * VIEW_RING_REACH,
        colour,
        &mut segment,
    );
    let half = view.reach * BRACKET_REACH;
    let arm = view.reach * 0.22;
    for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let corner = pivot + (across * sx + other * sy) * half;
        segment(corner, corner - across * (sx * arm), BRACKET_COLOUR);
        segment(corner, corner - other * (sy * arm), BRACKET_COLOUR);
    }
}

/// How far out the axis rings sit, against the arrows' reach.
pub const RING_REACH: f32 = 0.8;
/// Where a per-axis scale box sits along its arrow, against the reach.
pub const SCALE_BOX_REACH: f32 = 0.55;
/// Half the side of the corner-bracket square, against the reach.
pub const BRACKET_REACH: f32 = 1.42;
/// The centre block's colour: not an axis colour, and not the outer ring's.
const CENTRE_COLOUR: [f32; 3] = [0.82, 0.78, 0.42];
/// The brackets, quiet: they frame the widget and are not a handle.
const BRACKET_COLOUR: [f32; 3] = [0.55, 0.55, 0.58];

/// How far out the outer ring sits, against an axis ring's reach.
pub const VIEW_RING_REACH: f32 = 1.28;

/// How many passes each manipulator stroke is drawn in.
const HANDLE_WEIGHT: usize = 3;
/// How far apart those passes sit, against the manipulator's reach.
const HANDLE_STEP: f32 = 0.006;

/// Not one of the three axis colours: the outer ring belongs to no axis, and
/// borrowing red, green or blue would say it did.
const VIEW_RING_COLOUR: [f32; 3] = [0.82, 0.78, 0.42];

/// A unit vector, or `None` where there is no direction to have.
fn normalized(v: Vec3) -> Option<Vec3> {
    (v.length() > 1e-6).then(|| v / v.length())
}

/// Two unit vectors spanning the plane perpendicular to an axis.
///
/// The domain's, in this crate's vector type. One implementation rather than
/// two: the ring is *drawn* from this frame and *dragged* on a plane built
/// from the same one, and two copies could disagree.
pub fn frame_about(axis: Vec3) -> (Vec3, Vec3) {
    let (across, other) = clayspace_model::perpendicular_frame(axis.into());
    (across.into(), other.into())
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

/// The twelve edges of an axis-aligned box.
///
/// Two callers now — a selected object and the active subtool — and the corner
/// arithmetic is the part that is easy to get subtly wrong, so it is written
/// once.
fn outline_box(
    (min, max): ([f32; 3], [f32; 3]),
    colour: [f32; 3],
    segment: &mut impl FnMut(Vec3, Vec3, [f32; 3]),
) {
    let corner = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { min[0] } else { max[0] },
            if i & 2 == 0 { min[1] } else { max[1] },
            if i & 4 == 0 { min[2] } else { max[2] },
        )
    };
    // Every pair of corners differing in one bit, which is every pair one axis
    // apart.
    for a in 0..8usize {
        for bit in [1usize, 2, 4] {
            let b = a | bit;
            if b != a {
                segment(corner(a), corner(b), colour);
            }
        }
    }
}

/// Where the solid handles are lit from, in world space.
///
/// The overlay shader draws vertex colour as it is, so what makes a cone read
/// as a cone is baked here: each face is the handle's colour, darkened by how
/// far it turns from this light. Upper left and toward the eye, as the
/// material previews are lit — but fixed in the world, because the handles
/// are world-aligned and a light that turned with the camera would flatten
/// whichever face happened to face it.
const HANDLE_LIGHT: Vec3 = Vec3::new(-0.4, 0.7, 0.6);

/// One face's colour under `HANDLE_LIGHT`, never darker than a little over
/// half, so the shadowed side of a red cone is still red.
fn shaded(colour: [f32; 3], a: Vec3, b: Vec3, c: Vec3) -> [f32; 3] {
    let normal = (b - a).cross(c - a);
    let facing = normalized(normal)
        .map(|n| n.dot(HANDLE_LIGHT.normalize()).abs())
        .unwrap_or(0.0);
    let light = 0.55 + 0.45 * facing;
    [colour[0] * light, colour[1] * light, colour[2] * light]
}

/// A cone with its tip at `tip`, pointing along `axis`, `length` long and
/// `radius` wide at the base, closed with a disc.
fn cone(
    tip: Vec3,
    axis: Vec3,
    length: f32,
    radius: f32,
    colour: [f32; 3],
    triangle: &mut impl FnMut(Vec3, Vec3, Vec3, [f32; 3]),
) {
    const SEGMENTS: usize = 12;
    let (across, other) = frame_about(axis);
    let base = tip - axis * length;
    let rim = |i: usize| {
        let a = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        base + (across * a.cos() + other * a.sin()) * radius
    };
    for i in 0..SEGMENTS {
        let (p, q) = (rim(i), rim(i + 1));
        triangle(tip, p, q, colour);
        triangle(base, q, p, colour);
    }
}

/// The six faces of a cube, two triangles each.
fn solid_cube(
    centre: Vec3,
    size: f32,
    colour: [f32; 3],
    triangle: &mut impl FnMut(Vec3, Vec3, Vec3, [f32; 3]),
) {
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        for side in [-1.0f32, 1.0] {
            let corner = |du: f32, dv: f32| {
                let mut p = [0.0f32; 3];
                p[axis] = side * size;
                p[u] = du * size;
                p[v] = dv * size;
                centre + Vec3::from(p)
            };
            let (a, b, c, d) = (
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            );
            triangle(a, b, c, colour);
            triangle(a, c, d, colour);
        }
    }
}

/// What the cage overlay uploads: the lines, and the solid handles.
struct LatticeGeometry {
    lines: (Vec<Vertex>, Vec<u32>),
    solids: (Vec<Vertex>, Vec<u32>),
}

fn lattice_geometry(view: LatticeView<'_>) -> LatticeGeometry {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut solid_vertices: Vec<Vertex> = Vec::new();
    let mut solid_indices: Vec<u32> = Vec::new();

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
    let mut triangle = |a: Vec3, b: Vec3, c: Vec3, colour: [f32; 3]| {
        let base = solid_vertices.len() as u32;
        let color = shaded(colour, a, b, c);
        for position in [a, b, c] {
            solid_vertices.push(Vertex {
                position: position.into(),
                normal: [0.0, 1.0, 0.0],
                color,
                mask: 0.0,
            });
        }
        solid_indices.extend_from_slice(&[base, base + 1, base + 2]);
    };

    // A selected object's box, quieter still than the cage: it says where a
    // shape is, and a bright one would read as the shape itself.
    const OUTLINE: [f32; 3] = [0.52, 0.62, 0.72];
    /// The active SDF subtool's box, in the same hue its carried siblings are
    /// tinted with — one cue, two mechanisms, and a second colour would read as
    /// a second fact. Dimmed to sit a little below the object outline: which
    /// subtool is active is standing state, and the box a sculptor just put an
    /// object into is the more urgent of the two.
    const SUBTOOL_OUTLINE: [f32; 3] = palette::dimmed(ACTIVE_TINT, 0.68);
    if let Some(box_) = view.outline {
        outline_box(box_, OUTLINE, &mut segment);
    }
    if let Some(box_) = view.subtool_outline {
        outline_box(box_, SUBTOOL_OUTLINE, &mut segment);
    }

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
        gizmo_geometry_for(gizmo, &mut segment, &mut triangle);
    }

    LatticeGeometry {
        lines: (vertices, indices),
        solids: (solid_vertices, solid_indices),
    }
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
/// How many lines each half-axis of the navigation gizmo is drawn as.
const GIZMO_BUNDLE: usize = 5;
/// How far the copies sit from the axis, in the gizmo's own units.
const GIZMO_ROD: f32 = 0.018;

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

    // Each half-axis is a bundle of `GIZMO_BUNDLE` lines — the axis and four
    // copies stepped a little along the other two axes — so it reads as a rod
    // from every angle rather than as a hairline. A line is one pixel wide
    // whatever the device; the manipulator thickens itself the same way.
    let offsets = |direction: Vec3| -> [Vec3; GIZMO_BUNDLE] {
        let (across, other) = frame_about(direction);
        [
            Vec3::ZERO,
            across * GIZMO_ROD,
            -across * GIZMO_ROD,
            other * GIZMO_ROD,
            -other * GIZMO_ROD,
        ]
    };
    for (direction, color) in axes {
        for (end, shade) in [(direction, 1.0f32), (-direction, 0.25)] {
            let tint = [color[0] * shade, color[1] * shade, color[2] * shade];
            for offset in offsets(direction) {
                let base = vertices.len() as u32;
                vertices.push(Vertex {
                    position: offset.into(),
                    normal: [0.0, 1.0, 0.0],
                    color: tint,
                    mask: 0.0,
                });
                vertices.push(Vertex {
                    position: (end * 0.9 + offset).into(),
                    normal: [0.0, 1.0, 0.0],
                    color: tint,
                    mask: 0.0,
                });
                indices.extend_from_slice(&[base, base + 1]);
            }
        }
    }

    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(position: [f32; 3]) -> Vertex {
        Vertex {
            position,
            normal: [0.0, 1.0, 0.0],
            color: [1.0; 3],
            mask: 0.0,
        }
    }

    /// The box has to be the extremes on every axis, not the first vertex or
    /// the last.
    ///
    /// This is the framing box the viewport uses and the box the composition
    /// root accumulates a scene from — the same fold, which used to be written
    /// out once per crate with nothing keeping the two agreeing.
    #[test]
    fn the_bounds_are_the_extremes_on_every_axis() {
        // The first vertex holds none of the six extremes, so a fold that
        // seeded itself and then read only some of the axes would be caught.
        let vertices = [
            at([0.0, 0.0, 0.0]),
            at([1.0, -2.0, 9.0]),
            at([-3.0, 4.0, -7.0]),
        ];
        let (min, max) = Vertex::bounds(&vertices).expect("three vertices have a box");
        assert_eq!(min, [-3.0, -2.0, -7.0]);
        assert_eq!(max, [1.0, 4.0, 9.0]);
    }

    /// One vertex is a box of no size at that vertex, and no vertices is no
    /// box at all — a frame on an empty scene has to fall back rather than
    /// frame the origin.
    #[test]
    fn a_single_vertex_bounds_itself_and_none_bounds_nothing() {
        let one = [at([2.0, 3.0, 4.0])];
        assert_eq!(
            Vertex::bounds(&one),
            Some(([2.0, 3.0, 4.0], [2.0, 3.0, 4.0]))
        );
        assert_eq!(Vertex::bounds(&[]), None);
    }

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

    fn manipulator(mode: GizmoMode, per_axis_scale: bool) -> GizmoView {
        GizmoView {
            pivot: [0.0; 3],
            mode,
            reach: 1.0,
            hovered: None,
            view_axis: [0.0, 0.0, 1.0],
            per_axis_scale,
        }
    }

    fn handles(mode: GizmoMode, per_axis_scale: bool) -> (usize, usize) {
        let (mut lines, mut triangles) = (0usize, 0usize);
        gizmo_geometry_for(
            manipulator(mode, per_axis_scale),
            &mut |_, _, _| lines += 1,
            &mut |_, _, _, _| triangles += 1,
        );
        (lines, triangles)
    }

    /// Every segment and triangle the manipulator emits, in order.
    fn drawn(mode: GizmoMode, per_axis_scale: bool) -> (Vec<String>, Vec<String>) {
        let (mut lines, mut triangles) = (Vec::new(), Vec::new());
        gizmo_geometry_for(
            manipulator(mode, per_axis_scale),
            &mut |from, to, colour| lines.push(format!("{from:?}{to:?}{colour:?}")),
            &mut |a, b, c, colour| triangles.push(format!("{a:?}{b:?}{c:?}{colour:?}")),
        );
        (lines, triangles)
    }

    #[test]
    fn the_manipulator_is_the_same_geometry_in_every_mode() {
        // The exact form of "one widget, every operation": not a count, and
        // not a picture — the same segments and the same triangles, in the
        // same order, whichever mode is in force. The mode chooses what the
        // centre and a press on the clay do, and nothing that is drawn.
        //
        // Asserted here rather than by comparing captures because a capture
        // also asserts that the renderer is bit-deterministic from frame to
        // frame, which is not true on every device: macOS returned a mean
        // difference of 0.019 between two frames of identical geometry where
        // Linux returned zero, and a test that cannot tell that from a real
        // change is not testing what it says.
        for per_axis_scale in [false, true] {
            let reference = drawn(GizmoMode::Move, per_axis_scale);
            for mode in GizmoMode::ALL {
                assert_eq!(
                    drawn(mode, per_axis_scale),
                    reference,
                    "{mode:?} draws a different widget (per-axis scale: {per_axis_scale})"
                );
            }
        }
    }

    #[test]
    fn the_manipulator_carries_every_operation_whatever_the_mode() {
        // One widget: arrows with solid heads, rings of lines and the centre
        // block, in every mode. The mode chooses what the centre does, not
        // what is drawn.
        let mut pictures = Vec::new();
        for mode in GizmoMode::ALL {
            let (lines, solids) = handles(mode, false);
            assert!(solids > 0, "{mode:?} drew no solid handle");
            assert!(lines > 0, "{mode:?} drew no ring");
            pictures.push((lines, solids));
        }
        assert!(
            pictures.windows(2).all(|pair| pair[0] == pair[1]),
            "the modes draw different widgets: {pictures:?}"
        );
    }

    #[test]
    fn scale_boxes_are_drawn_only_where_a_stretch_can_be_applied() {
        let (_, uniform) = handles(GizmoMode::Scale, false);
        let (_, per_axis) = handles(GizmoMode::Scale, true);
        assert!(
            per_axis > uniform,
            "a per-axis target drew no more solids ({per_axis}) than a uniform one ({uniform})"
        );
    }

    #[test]
    fn a_solid_face_keeps_its_hue_in_shadow() {
        // The shadowed side of a red cone is a darker red, not grey and not
        // black: the light is baked as a factor with a floor.
        let red = [0.85, 0.24, 0.24];
        let dark = shaded(red, Vec3::ZERO, Vec3::X, Vec3::Y);
        let lit = shaded(red, Vec3::ZERO, Vec3::Y, Vec3::X);
        for face in [dark, lit] {
            assert!(
                face[0] > face[1] && face[0] > face[2],
                "the face lost its hue"
            );
            assert!(face[0] >= red[0] * 0.55 - 1e-6, "darker than the floor");
        }
    }

    #[test]
    fn the_gizmo_draws_three_axes_in_both_directions() {
        let (vertices, indices) = gizmo_geometry();
        // Six half-axes, each a bundle of lines, two vertices a line.
        assert_eq!(vertices.len(), 6 * GIZMO_BUNDLE * 2);
        assert_eq!(indices.len(), 6 * GIZMO_BUNDLE * 2);

        // Each axis must be distinguishable by hue, or the gizmo reports
        // nothing a glance can read: three full-shade hues and three dimmed,
        // six distinct colours over the whole bundle.
        let mut hues: Vec<[u8; 3]> = vertices
            .iter()
            .map(|v| v.color.map(|c| (c * 255.0) as u8))
            .collect();
        hues.sort();
        hues.dedup();
        assert_eq!(hues.len(), 6, "the gizmo's axes do not read as six colours");
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

    #[test]
    fn a_frame_about_any_axis_is_orthonormal() {
        // The outer ring is drawn in the plane these two span, and the axis is
        // wherever the camera happens to be — including straight down a world
        // axis, where a seed vector chosen carelessly collapses the cross
        // product to nothing and the ring degenerates to a point.
        let axes = [
            Vec3::X,
            Vec3::Y,
            Vec3::Z,
            -Vec3::X,
            -Vec3::Y,
            -Vec3::Z,
            Vec3::new(1.0, 1.0, 1.0).normalize(),
            Vec3::new(-0.3, 0.9, 0.31).normalize(),
            Vec3::new(1.0, 1e-7, -1e-7).normalize(),
        ];
        for axis in axes {
            let (across, other) = frame_about(axis);
            assert!(
                (across.length() - 1.0).abs() < 1e-4 && (other.length() - 1.0).abs() < 1e-4,
                "frame about {axis:?} was not unit: {across:?}, {other:?}"
            );
            assert!(
                across.dot(axis).abs() < 1e-4 && other.dot(axis).abs() < 1e-4,
                "frame about {axis:?} did not lie in its plane"
            );
            assert!(
                across.dot(other).abs() < 1e-4,
                "frame about {axis:?} was not square: {across:?}, {other:?}"
            );
        }
    }

    #[test]
    fn the_outer_ring_sits_outside_the_axis_rings() {
        // The whole point of it is that it is the easy target. Drawn among the
        // three axis rings it would be a fourth thing to tell apart at the same
        // radius.
        const { assert!(VIEW_RING_REACH > 1.0) };
        // And the axis rings sit inside the arrows, the boxes inside the
        // rings, the brackets outside everything.
        const { assert!(RING_REACH < 1.0 && SCALE_BOX_REACH < RING_REACH) };
        const { assert!(BRACKET_REACH > VIEW_RING_REACH) };
    }
}
