//! Drawing the sculpt.
//!
//! The renderer takes plain vertex and index data. It knows nothing about
//! ClayCore — that is the layering rule, and it is also what lets the same
//! code draw a document, a voxel grid or a test fixture without caring which.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::frustum::Frustum;
use crate::gpu::{Framebuffer, Gpu};
use crate::matcap::MatCap;
use crate::palette;
use crate::profiler::{GpuFrameTiming, GpuPass, GpuProfiler};
use crate::quality::{ShadingMode, StudioMaterial, ViewportQuality};
use clayspace_model::{GizmoHandle, GizmoMode, LayerKey, SurfaceOpacity};

/// Which run of the carried buffer belongs to which subtool.
///
/// The voxel and mesh layers arrive as one concatenated buffer, so this is the
/// only thing that says where one subtool's triangles end and the next one's
/// begin — and therefore the only thing that lets the active one be drawn
/// differently from the rest. One draw call per span rather than an instancing
/// scheme: a scene holds a handful of subtools, and a handful of draws is
/// noise beside the buffer they share.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshSpan {
    pub layer: LayerKey,
    /// Positions into the index buffer, which is what a draw call takes.
    pub indices: std::ops::Range<u32>,
    /// The box this subtool's triangles occupy, for culling it against the
    /// camera.
    ///
    /// Optional because a caller that has not worked them out still has to be
    /// able to draw: a span with no bounds is never culled, which is what
    /// every caller did before there was a frustum to cull against.
    pub bounds: Option<([f32; 3], [f32; 3])>,
}

impl MeshSpan {
    /// A span with its bounds not yet worked out.
    ///
    /// [`Renderer::set_mesh_layers`] fills them in, because it is the one
    /// place that holds both the spans and the vertices they index. A caller
    /// that computed them itself would be walking the same buffer twice, and a
    /// caller that forgot would silently lose the culling.
    pub fn new(layer: LayerKey, indices: std::ops::Range<u32>) -> Self {
        Self {
            layer,
            indices,
            bounds: None,
        }
    }
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
    /// How opaque the surface is drawn in x, and how far the silhouette is
    /// darkened in y. The rest pads the struct out to the sixteen bytes a
    /// uniform is aligned to, which is why neither is a bare f32.
    ghost: [f32; 4],
    /// Studio mode's roughness, metallic and exposure. Read by nothing in
    /// MatCap mode, and written every frame regardless: a uniform whose
    /// contents depend on which pipeline is bound is a uniform that is stale
    /// exactly when the mode is switched.
    studio: [f32; 4],
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
            let wanted = grown(self.vertex_capacity, vertices.len());
            // Only if the device will hold the grown figure. Growing past the
            // ceiling to leave room for a mesh that has not arrived would
            // refuse an upload that fits.
            let capacity = if Self::fits(gpu, wanted, 0) {
                wanted
            } else {
                vertices.len()
            };
            self.vertices = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vertices"),
                size: (capacity * Vertex::STRIDE) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_capacity = capacity;
        }
        if indices.len() > self.index_capacity {
            let wanted = grown(self.index_capacity, indices.len());
            let capacity = if Self::fits(gpu, 0, wanted) {
                wanted
            } else {
                indices.len()
            };
            self.indices = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("indices"),
                size: (capacity * 4) as u64,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.index_capacity = capacity;
        }

        if !vertices.is_empty() {
            gpu.queue
                .write_buffer(&self.vertices, 0, bytemuck::cast_slice(vertices));
            gpu.note_upload((vertices.len() * Vertex::STRIDE) as u64);
        }
        if !indices.is_empty() {
            gpu.queue
                .write_buffer(&self.indices, 0, bytemuck::cast_slice(indices));
            gpu.note_upload((indices.len() * 4) as u64);
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
        gpu.note_upload((vertices.len() * Vertex::STRIDE) as u64);
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
        gpu.note_upload((indices.len() * 4) as u64);
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

/// What to allocate for a buffer that has outgrown `current` and needs
/// `required`.
///
/// Half again as much, or the requirement if that is larger. Geometry grows a
/// little at a time — a dab adds a few thousand vertices to a surface of
/// millions — and allocating exactly what was asked for meant a fresh buffer
/// and a fresh copy of the whole surface on almost every edit that grew it. A
/// geometric policy makes the number of reallocations logarithmic in the final
/// size instead of linear in the number of edits.
///
/// It never shrinks. A buffer that shrank on the frame after it grew would
/// reallocate twice for one stroke, and the memory is reclaimed when the mesh
/// is replaced wholesale anyway.
fn grown(current: usize, required: usize) -> usize {
    required.max(current.saturating_mul(3) / 2)
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

/// What one frame asked the device to draw.
///
/// Counted rather than estimated: "a handful of subtools is a handful of draw
/// calls" is the reasoning several parts of this renderer rest on, and a
/// reasoning nothing measures is a reasoning that quietly stops being true.
///
/// Lines are counted apart from triangles on purpose. The polyframe and the
/// scaffolding are line lists, and folding their indices into a triangle count
/// would report a wireframed mesh as carrying half again as much geometry as
/// it does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub draw_calls: u32,
    /// Draws the frustum test removed before they were made.
    pub culled: u32,
    pub triangles: u64,
    pub lines: u64,
    /// Bytes written to the device since the previous frame began.
    ///
    /// Taken at the top of the frame rather than read on demand, so the figure
    /// is a frame's traffic and not "however much has happened since somebody
    /// last looked".
    pub uploaded_bytes: u64,
}

/// What a draw call is made of, for the count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Primitive {
    Triangles,
    Lines,
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
    /// The triangles the edges were last built from, when they have not been
    /// built for the triangles currently uploaded.
    ///
    /// The polyframe is off by default and stays off for most of most
    /// sessions, and deriving the edges means a hash set over three entries a
    /// triangle — on a two-million-triangle mesh, six million insertions on
    /// every upload, for a wireframe nobody asked to see. So the indices are
    /// kept here instead and the set is built when the polyframe is switched
    /// on, or when the mesh changes while it is already on.
    pending_edges: Option<Vec<u32>>,
    /// Multisampled full-resolution depth in, single-sampled half-resolution
    /// depth out. The pass that makes the two below cheap, and the one that
    /// makes them independent of multisampling.
    reduce_pipeline: wgpu::RenderPipeline,
    /// Reduced depth in, occlusion out.
    ao_pipeline: wgpu::RenderPipeline,
    /// Occlusion in, multiplied onto the resolved colour.
    composite_pipeline: wgpu::RenderPipeline,
    /// Both bind textures the framebuffer owns, so their groups are built
    /// against a framebuffer rather than against the renderer — and rebuilt
    /// when it is, which is on resize and not per frame.
    /// Anti-aliasing for a device that will not multisample. Run only there;
    /// see `shaders/fxaa.wgsl`.
    fxaa_pipeline: wgpu::RenderPipeline,
    fxaa_layout: wgpu::BindGroupLayout,
    fxaa_sampler: wgpu::Sampler,
    reduce_layout: wgpu::BindGroupLayout,
    ao_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    /// The groups themselves, and which framebuffer they were built for.
    ///
    /// Interior mutability because drawing a frame takes `&self`: the cache is
    /// an implementation of the same drawing, not a change to what is drawn.
    ao_resources: std::cell::RefCell<Option<AoResources>>,
    /// What the device says each pass cost, one frame behind.
    ///
    /// Interior mutability for the reason the bind-group cache has it: drawing
    /// a frame takes `&self`, and measuring the drawing is not a change to
    /// what is drawn.
    profiler: std::cell::RefCell<GpuProfiler>,
    /// What the last frame drew. A `Cell` for the reason the profiler is a
    /// `RefCell`: counting the draws is not a change to them.
    stats: std::cell::Cell<FrameStats>,
    ao_buffer: wgpu::Buffer,
    /// Whether the occlusion passes run.
    ///
    /// A switch rather than a constant because it is the only way to see what
    /// it is doing: the passes read the frame's own depth, so there is nothing
    /// to compare a capture against except the same capture without them.
    occlusion: bool,
    /// How much this frame is worth spending on.
    ///
    /// Told, never discovered. What the pointer is doing is the application's
    /// knowledge, and a renderer that read it would be a second place where
    /// "is the user sculpting" is defined — see [`crate::quality`].
    quality: ViewportQuality,
    /// Whether the post-process anti-aliasing runs, where there is one to run.
    ///
    /// A switch for the same reason occlusion has one: it is the only way to
    /// see what it is doing, since the pass reads the frame's own colour and
    /// there is nothing else to compare it against. It is also a real choice —
    /// the filter works on the picture rather than on the geometry, so it can
    /// mistake a fine sculpted crease for a stair-step and soften it, and a
    /// sculptor who would rather have the stair-step should be able to say so.
    antialias: bool,
    camera_buffer: wgpu::Buffer,
    material_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The reference planes' sampler: mipmapped and anisotropic, which the
    /// material's is not and does not need to be.
    reference_sampler: wgpu::Sampler,
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
    /// The same two under the studio rig, which is a different fragment stage
    /// over the same vertices and the same state.
    studio_pipeline: wgpu::RenderPipeline,
    studio_ghost_pipeline: wgpu::RenderPipeline,
    shading: ShadingMode,
    studio_material: StudioMaterial,
    ghosted: bool,
    /// How opaque the surface is drawn, as the sculptor set it.
    ///
    /// Held apart from `ghosted`, which is the cage imposing its own ceiling:
    /// putting a cage up must not forget the dial, and taking it down must not
    /// silently make a deliberately faint surface solid again.
    surface_opacity: SurfaceOpacity,
    /// How far the silhouette is darkened, 0 to 1.
    ///
    /// Zero by default, which is the material exactly as it was. A parameter
    /// rather than a fixed effect because it is a matter of taste and of
    /// material: the built-in MatCaps already carry a rim value baked into the
    /// texels outside their sphere, and doubling it would draw an outline
    /// rather than help a contour read.
    contour: f32,
    /// How strongly small creases are sharpened, 0 to 1.
    ///
    /// A screen-space curvature term applied in the occlusion composite. It
    /// reads a neighbourhood of reconstructed positions, which occlusion is
    /// already paying to have, so it is cheap where it runs — but it runs at
    /// display resolution and only [`ViewportQuality::High`] draws it, because
    /// the detail it sharpens is what a sculptor judges when they stop rather
    /// than while they push.
    cavity: f32,
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
            // The material now carries a mip chain, and a sampler that does not
            // interpolate between levels would swap abruptly from one to the
            // next as a subtool recedes — which reads as the material changing.
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // References get their own, because they are the one thing this
        // renderer draws that is genuinely viewed edge-on: a plane a sculptor
        // has swung the camera round to trace against is compressed far more
        // along one screen axis than the other, and that is exactly what
        // anisotropy is for. The MatCap is looked up in normal space and is
        // never oblique, so it would pay for nothing.
        //
        // Sixteen where the device allows it. Anisotropy requires linear
        // filtering on all three axes, which is why it cannot simply be added
        // to the sampler above without the mip filter that now precedes it.
        let reference_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("reference"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: gpu.max_anisotropy(),
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
            PipelineState::opaque(wgpu::PrimitiveTopology::TriangleList),
        );
        let overlay_pipeline = make_pipeline(
            gpu,
            &layout,
            &shader,
            format,
            "overlay_vs",
            "overlay_fs",
            PipelineState::transparent(wgpu::PrimitiveTopology::LineList),
        );
        let scaffold_pipeline = make_pipeline(
            gpu,
            &layout,
            &shader,
            format,
            "overlay_vs",
            "overlay_fs",
            PipelineState::scaffold(wgpu::PrimitiveTopology::LineList),
        );
        let scaffold_solid_pipeline = make_pipeline(
            gpu,
            &layout,
            &shader,
            format,
            "overlay_vs",
            "overlay_fs",
            PipelineState::scaffold(wgpu::PrimitiveTopology::TriangleList),
        );

        // The polyframe. The overlay's vertex stage — it is the same vertex
        // buffer, read the same way — with a fragment that draws ink rather
        // than the vertex colour, and a depth bias so the lines sit in front
        // of the very triangles they outline instead of fighting them.
        let wire_pipeline = make_pipeline(
            gpu,
            &layout,
            &shader,
            format,
            "overlay_vs",
            "wire_fs",
            PipelineState::wire(),
        );

        // The occlusion passes. Their own module: they bind depth textures and
        // a uniform of their own, so they share no layout with the scene.
        //
        // The source is rewritten where the device draws with one sample per
        // pixel. WGSL has no preprocessor and a texture's sample count is part
        // of its type, so the alternative to this one substitution is a second
        // copy of a three-hundred-line shader kept in step by hand.
        let samples = gpu.sample_count(format);
        let ao_source = if samples > 1 {
            std::borrow::Cow::Borrowed(include_str!("shaders/ao.wgsl"))
        } else {
            std::borrow::Cow::Owned(
                include_str!("shaders/ao.wgsl")
                    .replace("texture_depth_multisampled_2d", "texture_depth_2d"),
            )
        };
        let ao_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("ao"),
                source: wgpu::ShaderSource::Wgsl(ao_source),
            });

        // Binding 1 is the scene's depth, 2 the reduction's output, 3 the
        // occlusion. Each pass declares the layout for the subset it reads;
        // the numbers are shared so the shader declares each texture once.
        let scene_depth_entry = wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: samples > 1,
            },
            count: None,
        };
        let reduce_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ao depth reduction"),
                entries: &[uniform_entry(0), scene_depth_entry],
            });
        let ao_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ao"),
                entries: &[uniform_entry(0), read_entry(2)],
            });
        let composite_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("ao composite"),
                    entries: &[
                        uniform_entry(0),
                        scene_depth_entry,
                        read_entry(2),
                        read_entry(3),
                    ],
                });
        let reduce_pipeline = make_fullscreen_pipeline(
            gpu,
            &gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("ao depth reduction"),
                    bind_group_layouts: &[&reduce_layout],
                    push_constant_ranges: &[],
                }),
            &ao_shader,
            "reduce_fs",
            Framebuffer::REDUCED_DEPTH_FORMAT,
            None,
        );
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
        // Anti-aliasing for a device that will not multisample. Built
        // unconditionally and run only where the framebuffer says the scene
        // was drawn with one sample: a pipeline is cheap to hold and the
        // alternative is deciding at draw time whether one exists.
        let fxaa_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("fxaa"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/fxaa.wgsl").into()),
            });
        let fxaa_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("fxaa"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let fxaa_pipeline = make_fullscreen_pipeline(
            gpu,
            &gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("fxaa"),
                    bind_group_layouts: &[&fxaa_layout],
                    push_constant_ranges: &[],
                }),
            &fxaa_shader,
            "fxaa_fs",
            format,
            None,
        );
        // Filtered, and clamped: the kernel reaches a pixel either side of the
        // frame's edge, and a wrapped read there would fold the far edge in.
        let fxaa_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fxaa"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

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
            PipelineState::transparent(wgpu::PrimitiveTopology::TriangleList),
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
                PipelineState::transparent(wgpu::PrimitiveTopology::TriangleList),
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
                PipelineState::transparent(wgpu::PrimitiveTopology::TriangleList),
            ),
            studio_pipeline: make_pipeline(
                gpu,
                &layout,
                &shader,
                format,
                "vs_main",
                "fs_studio",
                PipelineState::opaque(wgpu::PrimitiveTopology::TriangleList),
            ),
            studio_ghost_pipeline: make_pipeline(
                gpu,
                &layout,
                &shader,
                format,
                "vs_main",
                "fs_studio_ghost",
                PipelineState::transparent(wgpu::PrimitiveTopology::TriangleList),
            ),
            shading: ShadingMode::MatCap,
            studio_material: StudioMaterial::default(),
            references: std::collections::BTreeMap::new(),
            ghosted: false,
            surface_opacity: SurfaceOpacity::SOLID,
            contour: 0.0,
            cavity: Self::CAVITY,
            pipeline,
            overlay_pipeline,
            scaffold_pipeline,
            scaffold_solid_pipeline,
            wire_pipeline,
            wire_indices: empty_buffer(gpu, "polyframe", wgpu::BufferUsages::INDEX),
            wire_index_count: 0,
            wire_capacity: 0,
            polyframe: false,
            pending_edges: None,
            membrane_pipeline,
            reduce_pipeline,
            ao_pipeline,
            composite_pipeline,
            fxaa_pipeline,
            fxaa_layout,
            fxaa_sampler,
            reduce_layout,
            ao_layout,
            composite_layout,
            ao_resources: std::cell::RefCell::new(None),
            profiler: std::cell::RefCell::new(GpuProfiler::new(gpu)),
            stats: std::cell::Cell::new(FrameStats::default()),
            ao_buffer,
            occlusion: true,
            // The best of them, because a renderer nobody has told anything to
            // is a renderer drawing a still frame — a capture, a test, an
            // export preview. Nothing is being sculpted in any of those.
            quality: ViewportQuality::High,
            antialias: true,
            camera_buffer,
            material_buffer,
            bind_group,
            bind_group_layout,
            sampler,
            reference_sampler,
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
            &self.reference_sampler,
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

    /// How much the next frames are worth spending on.
    ///
    /// Set from a [`crate::quality::QualityGovernor`], which is what watches
    /// the pointer and applies the hysteresis. Nothing here decides it.
    pub fn set_quality(&mut self, quality: ViewportQuality) {
        self.quality = quality;
    }

    pub fn quality(&self) -> ViewportQuality {
        self.quality
    }

    /// Whether silhouettes are smoothed after the fact on a device that will
    /// not multisample.
    ///
    /// Does nothing where the device *does* multisample: there is no
    /// post-process target there, and running both would be paying twice to
    /// lose detail once.
    pub fn set_antialias(&mut self, on: bool) {
        self.antialias = on;
    }

    pub fn antialias(&self) -> bool {
        self.antialias
    }

    /// What the last measured frame cost the GPU, per pass.
    ///
    /// `None` on a device without timestamp queries, and until the first
    /// measured frame has come back — the read is deliberately a frame behind,
    /// because waiting for it would make measuring the frame the thing that
    /// slowed it down.
    pub fn gpu_timing(&self) -> Option<GpuFrameTiming> {
        self.profiler.borrow().latest()
    }

    /// Whether the device will report per-pass GPU time at all.
    pub fn gpu_timing_available(&self) -> bool {
        self.profiler.borrow().is_supported()
    }

    /// What the last frame drawn asked the device for.
    pub fn frame_stats(&self) -> FrameStats {
        self.stats.get()
    }

    /// The rendering section of the diagnostics report.
    ///
    /// Assembled here because this is what knows it: the scene's rectangle,
    /// whether occlusion ran and at what size, what the device charged per
    /// pass, and how much geometry went across. The composition root has the
    /// framebuffer and nothing else of this.
    pub fn diagnostics(&self, framebuffer: &Framebuffer) -> clayspace_model::RenderDiagnostics {
        let scene = self.scene_viewport.unwrap_or([
            0.0,
            0.0,
            framebuffer.width as f32,
            framebuffer.height as f32,
        ]);
        let stats = self.stats.get();
        let profiler = self.profiler.borrow();
        clayspace_model::RenderDiagnostics {
            viewport: [scene[2].max(0.0) as u32, scene[3].max(0.0) as u32],
            samples: framebuffer.samples(),
            ao: self.occlusion.then(|| {
                let [width, height] = framebuffer.occlusion_size();
                clayspace_model::AoDiagnostics {
                    width,
                    height,
                    samples: self.quality.ao_samples(),
                    temporal: self.quality.temporal(),
                }
            }),
            gpu_passes: profiler
                .latest()
                .map(|timing| {
                    timing
                        .measured()
                        .map(|(pass, ms)| (pass.label().to_string(), ms))
                        .collect()
                })
                .unwrap_or_default(),
            gpu_timing: profiler.is_supported(),
            draw_calls: stats.draw_calls,
            culled: stats.culled,
            triangles: stats.triangles,
            lines: stats.lines,
            uploaded_bytes: stats.uploaded_bytes,
        }
    }

    /// The reference planes, furthest from the eye first.
    ///
    /// A plane's distance is taken from the middle of its quad, which is exact
    /// for the case that matters — planes that do not intersect each other —
    /// and is the usual approximation for the case that cannot be resolved by
    /// ordering at all.
    fn references_back_to_front(&self, eye: Vec3) -> Vec<&(GpuMesh, wgpu::BindGroup)> {
        let mut planes: Vec<&(GpuMesh, wgpu::BindGroup)> = self.references.values().collect();
        planes.sort_by(|a, b| {
            let distance = |mesh: &GpuMesh| {
                mesh.bounds()
                    .map(|(min, max)| ((min + max) * 0.5 - eye).length_squared())
                    // A plane with no bounds sorts to the back, where a
                    // reference belongs when nothing better is known.
                    .unwrap_or(f32::INFINITY)
            };
            distance(&b.0)
                .partial_cmp(&distance(&a.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        planes
    }

    /// Makes a draw call and counts it.
    ///
    /// Every indexed draw in this renderer goes through here, so the count is
    /// the frame's rather than an estimate of it.
    fn draw_indexed(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        indices: std::ops::Range<u32>,
        primitive: Primitive,
    ) {
        let count = indices.end.saturating_sub(indices.start) as u64;
        let mut stats = self.stats.get();
        stats.draw_calls += 1;
        match primitive {
            Primitive::Triangles => stats.triangles += count / 3,
            Primitive::Lines => stats.lines += count / 2,
        }
        self.stats.set(stats);
        pass.draw_indexed(indices, 0, 0..1);
    }

    /// A pass with no geometry — the fullscreen triangle the occlusion passes
    /// draw. Counted as a draw call, and as no geometry, because that is what
    /// it is.
    fn draw_fullscreen(&self, pass: &mut wgpu::RenderPass<'_>) {
        let mut stats = self.stats.get();
        stats.draw_calls += 1;
        self.stats.set(stats);
        pass.draw(0..3, 0..1);
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
        // The bounds are worked out here rather than by the caller: this is
        // the one place holding both the spans and the vertices they index,
        // and it is already walking them.
        self.mesh_spans = spans
            .iter()
            .map(|span| MeshSpan {
                bounds: span_bounds(vertices, indices, &span.indices),
                ..span.clone()
            })
            .collect();
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

    /// How far the silhouette is darkened, 0 to 1.
    ///
    /// Display only, like the material itself. Clamped rather than trusted: a
    /// value past one would take the contour below black and wrap the
    /// multiply into nonsense.
    pub fn set_contour(&mut self, strength: f32) {
        self.contour = strength.clamp(0.0, 1.0);
    }

    pub fn contour(&self) -> f32 {
        self.contour
    }

    /// How strongly small creases are sharpened, 0 to 1.
    ///
    /// Subtle by default. The term exists because a MatCap knows only the
    /// local normal and occlusion knows only the neighbourhood at its own
    /// radius, so neither says anything about a crease finer than that — which
    /// is most of the detail in a finished sculpt.
    pub fn set_cavity(&mut self, strength: f32) {
        self.cavity = strength.clamp(0.0, 1.0);
    }

    pub fn cavity(&self) -> f32 {
        self.cavity
    }

    /// How the sculpt is shaded.
    ///
    /// MatCap is the default and is what the sculpt path is tuned for; Studio
    /// answers one question a MatCap cannot — how the form takes a light that
    /// stays where it is while the camera moves — and is chosen, never
    /// arrived at.
    pub fn set_shading(&mut self, mode: ShadingMode) {
        self.shading = mode;
    }

    pub fn shading(&self) -> ShadingMode {
        self.shading
    }

    /// What the studio rig treats the surface as. Ignored in MatCap mode.
    pub fn set_studio_material(&mut self, material: StudioMaterial) {
        self.studio_material = material;
    }

    pub fn studio_material(&self) -> StudioMaterial {
        self.studio_material
    }

    /// The default cavity strength.
    ///
    /// Deliberately small. Turned up it stops being shading and starts being
    /// an ink line along every crease, which is a drawing of the mesh rather
    /// than a picture of the form — the same failure the normal-derivative
    /// cavity had, arrived at from the other direction.
    pub const CAVITY: f32 = 0.35;

    /// Whether the mesh layers are drawn with their own edges over them.
    ///
    /// ZBrush calls it the polyframe, and it answers the one question a
    /// wireframe is for: how much geometry is actually there. A sculptor
    /// deciding whether a mesh wants retopology is reading its density, and a
    /// shaded surface hides exactly that.
    ///
    /// The edges are derived here rather than on upload when the polyframe was
    /// off at the time, because deriving them is the expensive half and most
    /// sessions never ask for them.
    pub fn set_polyframe(&mut self, gpu: &Gpu, on: bool) {
        self.polyframe = on;
        if !on {
            return;
        }
        if let Some(indices) = self.pending_edges.take() {
            self.build_edges(gpu, &indices);
        }
    }

    /// The unique edges of a triangle list, as a line list.
    ///
    /// Deduplicated, and not only to halve the buffer: the lines are drawn
    /// translucent, so an edge shared by two triangles and emitted twice is
    /// blended twice and comes out darker than a boundary edge. A wireframe
    /// where the interior reads heavier than the silhouette is backwards.
    fn upload_edges(&mut self, gpu: &Gpu, indices: &[u32]) {
        if !self.polyframe {
            // Kept rather than derived. Switching the polyframe on is what
            // pays for it, and switching it on is a click.
            self.pending_edges = Some(indices.to_vec());
            self.wire_index_count = 0;
            return;
        }
        self.pending_edges = None;
        self.build_edges(gpu, indices);
    }

    /// The same, once it is known the edges are actually wanted.
    fn build_edges(&mut self, gpu: &Gpu, indices: &[u32]) {
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
        gpu.note_upload((edges.len() * 4) as u64);
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
    fn draw_carried(&self, pass: &mut wgpu::RenderPass<'_>, frustum: &Frustum) {
        if self.mesh_spans.is_empty() {
            self.draw_indexed(pass, 0..self.mesh_layers.index_count, Primitive::Triangles);
            return;
        }
        for span in &self.mesh_spans {
            // A span with no bounds is never culled: a caller that has not
            // said where its triangles are has not said they are elsewhere.
            if let Some((min, max)) = span.bounds {
                if !frustum.intersects(min.into(), max.into()) {
                    let mut stats = self.stats.get();
                    stats.culled += 1;
                    self.stats.set(stats);
                    continue;
                }
            }
            let material = if Some(span.layer) == self.active_subtool {
                &self.active_bind_group
            } else {
                &self.bind_group
            };
            pass.set_bind_group(0, material, &[]);
            self.draw_indexed(pass, span.indices.clone(), Primitive::Triangles);
        }
    }

    /// Draws one frame into `target`.
    ///
    /// The pass order, which is an invariant rather than an accident:
    ///
    /// 1. the opaque scene, multisampled, writing depth — the surface, the
    ///    mesh layers, and the helpers that are *behind* or *on* them: grid,
    ///    references, polyframe, cursor, membrane, rig;
    /// 2. the multisample resolve, which the pass above does on the way out;
    /// 3. the depth reduction, to the occlusion resolution;
    /// 4. the occlusion kernel, there;
    /// 5. the depth-aware upsample, multiplied onto the resolved colour;
    /// 6. the scaffolding — cage, object outline, manipulator, orientation
    ///    gizmo — drawn onto that finished frame;
    /// 7. egui, afterwards, by the composition root.
    ///
    /// The line between 1 and 6 is the one worth stating. Occlusion at step 5
    /// is a multiply over *everything already drawn*, so anything in step 1 is
    /// darkened by whatever the surface wrote at that pixel. For a helper that
    /// lies on the surface — the cursor ring, the polyframe, the rig — that is
    /// right: they are on the clay and should read as being on it. For a
    /// manipulator, which stands over the form rather than on it, it is not:
    /// the handle a person is aiming at came out dimmed exactly where the form
    /// is deepest. So the scaffolding is drawn after, into the resolved
    /// target, with no depth buffer — which it never used, since it compares
    /// `Always`.
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
        let view_projection = camera.view_projection(aspect);
        // The same matrix the vertex stage will use, so what is culled and
        // what is drawn cannot disagree about where the camera is pointing.
        let frustum = Frustum::from_view_projection(view_projection);
        let uniform = CameraUniform {
            view_projection: view_projection.to_cols_array_2d(),
            view_rotation: camera.view_rotation().to_cols_array_2d(),
        };
        gpu.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        let colored = if has_vertex_colors { 1.0 } else { 0.0 };
        // The effective opacity and not the dial: the cage imposes its own
        // ceiling, and writing the dial here would select the ghost pipeline
        // and then draw it solid.
        let ghost = [self.drawn_opacity().get(), self.contour, 0.0, 0.0];
        let studio = [
            self.studio_material.roughness,
            self.studio_material.metallic,
            self.studio_material.exposure,
            0.0,
        ];
        gpu.queue.write_buffer(
            &self.material_buffer,
            0,
            bytemuck::bytes_of(&MaterialUniform {
                tint: [1.0, 1.0, 1.0, colored],
                ghost,
                studio,
            }),
        );
        gpu.queue.write_buffer(
            &self.active_material_buffer,
            0,
            bytemuck::bytes_of(&MaterialUniform {
                tint: [ACTIVE_TINT[0], ACTIVE_TINT[1], ACTIVE_TINT[2], colored],
                ghost,
                studio,
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
        let (attachment, resolve_target) = framebuffer.attachment(target, self.antialias);

        // Collects what the previous frame reported before this one records
        // anything. Held across the whole frame so each pass can be given its
        // own pair of timestamps.
        let mut profiler = self.profiler.borrow_mut();
        profiler.begin_frame(gpu);
        self.stats.set(FrameStats {
            uploaded_bytes: gpu.take_uploaded_bytes(),
            ..FrameStats::default()
        });

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
                        load: wgpu::LoadOp::Clear(DEPTH_CLEAR),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: profiler.writes(GpuPass::Scene),
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
                self.draw_indexed(
                    &mut pass,
                    0..self.overlay_mesh.index_count,
                    Primitive::Lines,
                );
            }

            // The references first, and writing no depth, so everything else
            // is drawn over them whichever side of them the camera is on.
            //
            // Back to front. They are blended and write no depth, so the order
            // they are drawn in *is* the order they composite in — two planes
            // crossing behind the form came out with whichever was drawn last
            // in front, whatever the camera was actually looking at. Sorting
            // is three planes at most, so it is a sort rather than the
            // order-independent machinery a scene full of glass would want.
            for (mesh, bind_group) in self.references_back_to_front(camera.eye()) {
                if mesh.is_empty() {
                    continue;
                }
                pass.set_pipeline(&self.reference_pipeline);
                pass.set_bind_group(0, bind_group, &[]);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                self.draw_indexed(&mut pass, 0..mesh.index_count, Primitive::Triangles);
            }
            // Back to the scene's own bindings, which the loop above replaced.
            pass.set_bind_group(0, &self.bind_group, &[]);

            // Through, while a cage is up or the sculptor has dialled the
            // surface back. One choice for both the surface and the mesh
            // layers: a document with one of each half solid and half ghosted
            // would read as two objects.
            let surface = match (self.shading, self.drawn_opacity().is_solid()) {
                (ShadingMode::MatCap, true) => &self.pipeline,
                (ShadingMode::MatCap, false) => &self.ghost_pipeline,
                (ShadingMode::Studio, true) => &self.studio_pipeline,
                (ShadingMode::Studio, false) => &self.studio_ghost_pipeline,
            };

            if !mesh.is_empty() {
                pass.set_pipeline(surface);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                self.draw_indexed(&mut pass, 0..mesh.index_count, Primitive::Triangles);
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
                self.draw_carried(&mut pass, &frustum);
                // Back to the plain material, which a tinted span may have
                // replaced: everything after this belongs to no subtool.
                pass.set_bind_group(0, &self.bind_group, &[]);

                // And its edges over it, when the polyframe is on. The same
                // vertex buffer, read as a line list through its own indices.
                if self.polyframe && self.wire_index_count > 0 {
                    pass.set_pipeline(&self.wire_pipeline);
                    pass.set_index_buffer(self.wire_indices.slice(..), wgpu::IndexFormat::Uint32);
                    self.draw_indexed(&mut pass, 0..self.wire_index_count, Primitive::Lines);
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
                self.draw_indexed(&mut pass, 0..self.cursor_mesh.index_count, Primitive::Lines);
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
                self.draw_indexed(
                    &mut pass,
                    0..self.membrane_mesh.index_count,
                    Primitive::Triangles,
                );
            }
            if !self.armature_mesh.is_empty() {
                pass.set_pipeline(&self.overlay_pipeline);
                pass.set_vertex_buffer(0, self.armature_mesh.vertices.slice(..));
                pass.set_index_buffer(
                    self.armature_mesh.indices.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                self.draw_indexed(
                    &mut pass,
                    0..self.armature_mesh.index_count,
                    Primitive::Lines,
                );
            }
        }

        // The radius of everything drawn with depth: the surface and the mesh
        // layers together, since either may be the only one present.
        let radius = form_radius(union_bounds(mesh.bounds(), self.mesh_layers.bounds()));
        // Occlusion composites onto whatever the scene was drawn into, which
        // is the caller's target unless a post-process pass has to read the
        // scene back — a texture cannot be sampled and written by one pass.
        let scene_view = framebuffer.scene_view(target, self.antialias);
        self.ensure_resources(gpu, framebuffer);
        self.occlude(
            gpu,
            &mut encoder,
            scene_view,
            framebuffer,
            camera,
            aspect,
            scene,
            radius,
            &mut profiler,
        );
        // And the anti-aliasing, where the device would not multisample. It
        // runs before the scaffolding rather than after, so a manipulator's
        // lines — which are drawn at the resolution they are meant to be read
        // at — are not softened by a filter that exists to hide stair-steps in
        // the geometry.
        self.smooth_silhouettes(&mut encoder, target, scene);
        profiler.resolve(&mut encoder);

        // The scaffolding, the manipulator and the orientation gizmo, after
        // the occlusion composite and into the resolved target.
        //
        // After, and that is the whole reason they are a pass of their own.
        // Occlusion is a multiply over everything already drawn, so anything
        // inside the scene pass is darkened by whatever the *surface* wrote at
        // that pixel — a manipulator standing over a deep fold came out dimmed
        // by that fold, which is the handle a person is aiming at going dark
        // exactly where the form is most worth aiming at. These compare
        // `Always` and write no depth, so nothing is lost by separating them
        // from the depth buffer, and the orientation gizmo stops being
        // occludable by a sculpt that reaches into its corner.
        self.draw_over(&mut encoder, target, scene);

        gpu.queue.submit(Some(encoder.finish()));
        profiler.after_submit();
    }

    /// Builds the passes' bind groups for this framebuffer, if they are not
    /// the ones already held.
    ///
    /// Once a frame, before anything that reads them, rather than inside
    /// whichever pass happens to run first. Occlusion can be switched off and
    /// the anti-aliasing cannot, so "whichever is first" is not a fixed
    /// answer — the version that built them inside the occlusion pass left a
    /// device that draws single-sampled with no anti-aliasing at all whenever
    /// occlusion was off.
    fn ensure_resources(&self, gpu: &Gpu, framebuffer: &Framebuffer) {
        let mut cached = self.ao_resources.borrow_mut();
        if cached
            .as_ref()
            .is_some_and(|held| held.framebuffer == framebuffer.id())
        {
            return;
        }
        *cached = Some(AoResources::new(
            gpu,
            framebuffer,
            (&self.reduce_layout, &self.ao_layout, &self.composite_layout),
            &self.ao_buffer,
            (&self.fxaa_layout, &self.fxaa_sampler),
        ));
    }

    /// Anti-aliases the scene into `target`, where the device would not
    /// multisample it.
    ///
    /// Nothing at all where it would: four samples and a blur over the top is
    /// paying twice to lose detail once, and the detail lost would be sculpted
    /// crease mistaken for stair-step. See `shaders/fxaa.wgsl`.
    fn smooth_silhouettes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        scene: [f32; 4],
    ) {
        if !self.antialias {
            return;
        }
        let cached = self.ao_resources.borrow();
        let Some(bind_group) = cached.as_ref().and_then(|held| held.antialias.as_ref()) else {
            return;
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("antialias"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Cleared rather than loaded: every pixel of the scene's
                    // rectangle is written, and outside it the target has not
                    // been drawn to at all.
                    load: wgpu::LoadOp::Clear(self.background),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_viewport(scene[0], scene[1], scene[2], scene[3], 0.0, 1.0);
        pass.set_pipeline(&self.fxaa_pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        self.draw_fullscreen(&mut pass);
    }

    /// The scaffolding, drawn on the finished frame rather than in it.
    fn draw_over(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        scene: [f32; 4],
    ) {
        let gizmo = self.show_gizmo && !self.gizmo_mesh.is_empty();
        if self.lattice_mesh.is_empty() && self.lattice_solid_mesh.is_empty() && !gizmo {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scaffolding"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_viewport(scene[0], scene[1], scene[2], scene[3], 0.0, 1.0);
        pass.set_bind_group(0, &self.bind_group, &[]);

        if !self.lattice_mesh.is_empty() {
            pass.set_pipeline(&self.scaffold_pipeline);
            pass.set_vertex_buffer(0, self.lattice_mesh.vertices.slice(..));
            pass.set_index_buffer(
                self.lattice_mesh.indices.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            self.draw_indexed(
                &mut pass,
                0..self.lattice_mesh.index_count,
                Primitive::Lines,
            );
        }
        // The solid handles last, over the shafts they cap.
        if !self.lattice_solid_mesh.is_empty() {
            pass.set_pipeline(&self.scaffold_solid_pipeline);
            pass.set_vertex_buffer(0, self.lattice_solid_mesh.vertices.slice(..));
            pass.set_index_buffer(
                self.lattice_solid_mesh.indices.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            self.draw_indexed(
                &mut pass,
                0..self.lattice_solid_mesh.index_count,
                Primitive::Triangles,
            );
        }

        // The navigation gizmo, in its own corner viewport so it keeps a fixed
        // size whatever the window does. It shares the camera's rotation and
        // nothing else — it reports orientation, not position.
        if gizmo {
            // Anchored to the scene's rectangle, not the window's. Against the
            // window it sat in the corner the right panel covers, so the gizmo
            // was drawn every frame and never once visible.
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
            pass.set_pipeline(&self.scaffold_pipeline);
            pass.set_vertex_buffer(0, self.gizmo_mesh.vertices.slice(..));
            pass.set_index_buffer(self.gizmo_mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            self.draw_indexed(&mut pass, 0..self.gizmo_mesh.index_count, Primitive::Lines);
        }
    }

    /// Darkens what the surface closes in on, from the depth it just wrote.
    ///
    /// Three passes after the scene, described in `shaders/ao.wgsl`: the
    /// depth is reduced to the occlusion resolution, the kernel runs there,
    /// and the result is brought back up weighted by depth and multiplied onto
    /// the resolved colour. Nothing here reads that colour — the multiply is
    /// the blend state — so there is no copy of the frame and no third target.
    ///
    /// Run whatever the sample count. The kernel used to bind the scene's
    /// depth buffer directly and so could only run where the device would
    /// multisample; the reduction pass is what removed that.
    #[allow(clippy::too_many_arguments)]
    fn occlude(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene_view: &wgpu::TextureView,
        framebuffer: &Framebuffer,
        camera: &Camera,
        aspect: f32,
        scene: [f32; 4],
        form_radius: f32,
        profiler: &mut GpuProfiler,
    ) {
        if !self.occlusion {
            return;
        }
        let projection = camera.projection(aspect);
        let [ao_width, ao_height] = framebuffer.occlusion_size();
        let radius = form_radius * AO_RADIUS_FRACTION;
        gpu.queue.write_buffer(
            &self.ao_buffer,
            0,
            bytemuck::bytes_of(&AoUniform {
                projection: projection.to_cols_array_2d(),
                inverse_projection: projection.inverse().to_cols_array_2d(),
                viewport: scene,
                ao_size: [
                    ao_width as f32,
                    ao_height as f32,
                    1.0 / ao_width as f32,
                    1.0 / ao_height as f32,
                ],
                params: [
                    radius,
                    AO_INTENSITY,
                    radius * AO_BIAS_FRACTION,
                    self.quality.ao_samples() as f32,
                ],
                reduce: [
                    framebuffer.samples() as f32,
                    Framebuffer::AO_SCALE as f32,
                    // Per view unit, so a neighbour a fifth of the occlusion
                    // radius away in depth is already most of the way to being
                    // rejected. Tied to the radius rather than absolute for
                    // the reason the radius itself is: an imported model at a
                    // hundred times the scale would otherwise have every
                    // neighbour count as the same surface.
                    AO_DEPTH_SHARPNESS / radius.max(1e-6),
                    DEPTH_BACKGROUND,
                ],
                cavity: [
                    if self.quality.cavity() {
                        self.cavity
                    } else {
                        0.0
                    },
                    // The same reach the occlusion kernel uses, so a crease
                    // deep enough to occlude is a crease deep enough to sharpen
                    // and the two terms do not disagree about what a crease is.
                    radius,
                    0.0,
                    0.0,
                ],
                kernel: ao_kernel(self.quality.ao_samples() as usize),
            }),
        );

        let cached = self.ao_resources.borrow();
        let Some(resources) = cached.as_ref() else {
            return;
        };

        // The reduction and the kernel cover their whole target rather than
        // the scene's rectangle halved. Halving an odd offset would put the
        // occlusion image half a pixel off the frame it shades, and the work
        // saved is a strip of a quarter-resolution buffer whose depth is the
        // cleared value — which the kernel leaves after one load.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("depth reduction"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: framebuffer.reduced_depth_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Every pixel is written, so there is nothing to load
                        // and nothing to clear to.
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: profiler.writes(GpuPass::DepthReduce),
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.reduce_pipeline);
            pass.set_bind_group(0, &resources.reduce, &[]);
            self.draw_fullscreen(&mut pass);
        }
        {
            // Cleared to white rather than loaded: outside the scene's
            // rectangle nothing is occluded, and white is what the composite
            // reads as "leave this alone" when its neighbourhood reaches over
            // the edge.
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("occlusion"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: framebuffer.occlusion_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: profiler.writes(GpuPass::Ao),
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.ao_pipeline);
            pass.set_bind_group(0, &resources.ao, &[]);
            self.draw_fullscreen(&mut pass);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("occlusion composite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Loaded, because this darkens the frame that is
                        // already there rather than drawing one.
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: profiler.writes(GpuPass::AoComposite),
                occlusion_query_set: None,
            });
            pass.set_viewport(scene[0], scene[1], scene[2], scene[3], 0.0, 1.0);
            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, &resources.composite, &[]);
            self.draw_fullscreen(&mut pass);
        }
    }
}

/// The box the triangles in one index range occupy.
///
/// `None` for an empty range, and for a range that names no vertex this buffer
/// holds — a span that has outlived the geometry it described must not be
/// culled against a box made up from whatever the indices happened to reach.
fn span_bounds(
    vertices: &[Vertex],
    indices: &[u32],
    range: &std::ops::Range<u32>,
) -> Option<([f32; 3], [f32; 3])> {
    let slice = indices.get(range.start as usize..range.end as usize)?;
    let mut bounds: Option<([f32; 3], [f32; 3])> = None;
    for index in slice {
        let at = vertices.get(*index as usize)?.position;
        bounds = Some(match bounds {
            None => (at, at),
            Some((min, max)) => (
                [min[0].min(at[0]), min[1].min(at[1]), min[2].min(at[2])],
                [max[0].max(at[0]), max[1].max(at[1]), max[2].max(at[2])],
            ),
        });
    }
    bounds
}

/// The box containing both, or whichever of them there is.
fn union_bounds(a: Option<(Vec3, Vec3)>, b: Option<(Vec3, Vec3)>) -> Option<(Vec3, Vec3)> {
    match (a, b) {
        (Some((amin, amax)), Some((bmin, bmax))) => Some((amin.min(bmin), amax.max(bmax))),
        (only, None) | (None, only) => only,
    }
}

/// A texture the fragment stage only ever `textureLoad`s.
///
/// Non-filtering on purpose: nothing here samples with a filter, and asking
/// for a filtering sampler binding would refuse the reduced depth's
/// `R32Float` on any device without the float-filtering feature.
fn read_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
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

/// The occlusion passes' bind groups, and the framebuffer they read.
///
/// Held rather than built per frame. All three groups name texture views the
/// framebuffer owns, and a framebuffer is replaced only when the viewport is
/// resized — so rebuilding them every frame was three descriptor writes per
/// frame to say the same thing. [`Framebuffer::id`] is what makes the
/// staleness question answerable at all: wgpu gives a texture view no identity
/// to compare.
struct AoResources {
    framebuffer: u64,
    reduce: wgpu::BindGroup,
    ao: wgpu::BindGroup,
    composite: wgpu::BindGroup,
    /// The post-process pass's view of the scene. `None` where the scene was
    /// drawn straight into the caller's target, which is where there is no
    /// post-process pass to run.
    antialias: Option<wgpu::BindGroup>,
}

impl AoResources {
    fn new(
        gpu: &Gpu,
        framebuffer: &Framebuffer,
        layouts: (
            &wgpu::BindGroupLayout,
            &wgpu::BindGroupLayout,
            &wgpu::BindGroupLayout,
        ),
        ao_buffer: &wgpu::Buffer,
        fxaa: (&wgpu::BindGroupLayout, &wgpu::Sampler),
    ) -> Self {
        let (reduce_layout, ao_layout, composite_layout) = layouts;
        let (fxaa_layout, fxaa_sampler) = fxaa;
        let uniform = wgpu::BindGroupEntry {
            binding: 0,
            resource: ao_buffer.as_entire_binding(),
        };
        let scene_depth = wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureView(framebuffer.depth_view()),
        };
        let reduced = wgpu::BindGroupEntry {
            binding: 2,
            resource: wgpu::BindingResource::TextureView(framebuffer.reduced_depth_view()),
        };
        let occlusion = wgpu::BindGroupEntry {
            binding: 3,
            resource: wgpu::BindingResource::TextureView(framebuffer.occlusion_view()),
        };
        Self {
            framebuffer: framebuffer.id(),
            reduce: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ao depth reduction"),
                layout: reduce_layout,
                entries: &[uniform.clone(), scene_depth.clone()],
            }),
            ao: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ao"),
                layout: ao_layout,
                entries: &[uniform.clone(), reduced.clone()],
            }),
            composite: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ao composite"),
                layout: composite_layout,
                entries: &[uniform, scene_depth, reduced, occlusion],
            }),
            antialias: framebuffer.antialias_view().map(|view| {
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("fxaa"),
                    layout: fxaa_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(fxaa_sampler),
                        },
                    ],
                })
            }),
        }
    }
}

/// What the occlusion pass needs to turn a depth buffer into a shadowing term.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct AoUniform {
    projection: [[f32; 4]; 4],
    inverse_projection: [[f32; 4]; 4],
    /// Where the scene sits in the target, in full-resolution pixels.
    viewport: [f32; 4],
    /// The occlusion target's size, then its reciprocal. Distinct from the
    /// viewport because the kernel runs below display resolution.
    ao_size: [f32; 4],
    /// radius, intensity, bias, sample count.
    params: [f32; 4],
    /// Samples per scene pixel, display pixels per occlusion pixel, the
    /// upsample's depth sharpness, and the depth nothing was drawn at.
    reduce: [f32; 4],
    /// Cavity strength, and the reach of its neighbourhood in view units.
    /// Zero strength is the term switched off, which costs the composite a
    /// branch and nothing else.
    cavity: [f32; 4],
    /// The sample kernel, in the tangent frame. See [`ao_kernel`].
    kernel: [[f32; 4]; AO_KERNEL],
}

/// How many samples the kernel holds room for.
///
/// The highest quality tier's count. Stated here and in `ao.wgsl`, which the
/// uniform-size test holds together.
const AO_KERNEL: usize = 16;

/// The occlusion sample directions, in the tangent frame around the normal.
///
/// Computed on the host because none of it depends on the pixel. The loop used
/// to take a square root, a sine, a cosine and an interpolation *per sample per
/// pixel* to arrive at a direction that is a function of the sample index
/// alone; at half resolution and twelve samples that is a quarter of a million
/// transcendentals a frame at 1080p, computing the same sixteen numbers over
/// and over.
///
/// What is left in the shader is a rotation by the pixel's own turn, which is
/// two multiplies and an add against a precomputed pair.
///
/// The distribution is unchanged, and it is worth saying what it is. The
/// samples advance by the golden angle, so successive ones land as far from
/// each other as they can and a short loop still covers the hemisphere evenly.
/// The planar radius is a square root, so they spread over the disc's *area*
/// rather than crowding its centre. And the distance along the ray grows with
/// the index, so the near field is sampled as densely as the far one rather
/// than every sample sitting on the rim.
fn ao_kernel(count: usize) -> [[f32; 4]; AO_KERNEL] {
    /// The golden angle, in radians.
    const GOLDEN: f32 = 2.399_963_2;
    /// The shortest a sample's reach gets, as a fraction of the radius.
    const NEAREST: f32 = 0.15;

    let count = count.clamp(1, AO_KERNEL);
    std::array::from_fn(|i| {
        // Entries past the count are never read — the shader loops to `count`
        // — and are left as the first one rather than as zero, which would be
        // a direction of no length if one ever were.
        let i = i.min(count - 1);
        let t = (i as f32 + 0.5) / count as f32;
        let planar = t.sqrt();
        let up = (1.0 - planar * planar).max(0.0).sqrt();
        let angle = i as f32 * GOLDEN;
        [
            angle.cos() * planar,
            angle.sin() * planar,
            up,
            NEAREST + (1.0 - NEAREST) * t * t,
        ]
    })
}

/// How far an occluder can be and still count, as a fraction of the radius of
/// what is being drawn.
///
/// A world-space reach rather than a screen-space one, so a fold darkens by
/// how deep it is rather than by how much of the window it happens to cover —
/// and a *fraction* rather than an absolute figure, so the same form at any
/// scale shades the same way. It was 0.08 view units, tuned against the
/// reference form whose starting sphere has radius 1; expressed against that
/// form's own radius the number is unchanged and every other model's is
/// finally right. An imported mesh a hundredth of that size got no visible
/// occlusion at all, and one a hundred times it got total occlusion, neither
/// of which is a property of the shape.
const AO_RADIUS_FRACTION: f32 = 0.08;
/// How much of the surface's own colour full occlusion takes away.
const AO_INTENSITY: f32 = 0.85;
/// The depth difference below which an occluder is the surface itself, as a
/// fraction of the radius.
///
/// Without it a flat surface occludes itself everywhere, from the difference
/// between a sample's own depth and the depth of the pixel it projects to. A
/// fraction for the reason the radius is one: the bias that stops a surface
/// self-occluding at one scale lets it self-occlude at another. 0.05 of the
/// radius is the 0.004 the reference form was tuned to.
const AO_BIAS_FRACTION: f32 = 0.05;
/// How sharply the upsample rejects a neighbour whose depth differs, per
/// occlusion radius of difference.
///
/// The number that decides whether occlusion crosses a silhouette. Too low and
/// the average runs over the edge, which is the halo the box blur produced;
/// too high and the term degenerates to a nearest-neighbour lookup, which
/// brings the kernel's noise back at display resolution.
const AO_DEPTH_SHARPNESS: f32 = 4.0;

/// The radius of what is being drawn, for the occlusion figures above.
///
/// Half the longest side of the box the geometry occupies: for the reference
/// form, whose starting sphere spans −1 to 1, exactly 1 — which is what makes
/// the fractions above the numbers the pass was already tuned to.
///
/// One is the fallback rather than zero. A frame with nothing in it has no
/// scale to speak of, and a radius of zero would divide the depth sharpness by
/// nothing.
fn form_radius(bounds: Option<(Vec3, Vec3)>) -> f32 {
    let Some((min, max)) = bounds else {
        return 1.0;
    };
    let extent = (max - min).max_element() * 0.5;
    if extent.is_finite() && extent > 1e-6 {
        extent
    } else {
        1.0
    }
}

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

/// Which way depth runs, stated once.
///
/// Reversed: [`Camera::projection`] puts the *near* plane at 1 and the far
/// plane at 0, so nearer is greater and the buffer clears to zero. Floating
/// point crowds its precision near zero, and a conventional mapping spends
/// that on the far plane where nothing needs it — reversing the range puts the
/// precision where a sculptor is working, and makes it very nearly uniform
/// across the whole range besides.
///
/// Named rather than written at each pipeline because the convention is one
/// decision that eight pipelines, a clear value, a depth bias and three
/// occlusion passes all have to agree with. They agreed by coincidence before
/// this constant existed.
const DEPTH_COMPARE: wgpu::CompareFunction = wgpu::CompareFunction::GreaterEqual;

/// What the depth buffer is cleared to: the far plane under [`DEPTH_COMPARE`].
const DEPTH_CLEAR: f32 = 0.0;

/// The depth value nothing was drawn at, as the occlusion passes read it.
///
/// The same number as the clear, named separately because the two are
/// different claims: one is what the pass writes before drawing, the other is
/// what a later pass may conclude from finding it.
const DEPTH_BACKGROUND: f32 = DEPTH_CLEAR;

const NO_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: 0,
    slope_scale: 0.0,
    clamp: 0.0,
};

/// The polyframe's bias, toward the camera under [`DEPTH_COMPARE`].
///
/// A wireframe shares its vertices with the triangles it outlines, so without
/// one every line lands on exactly the same depth as the surface and the two
/// flicker against each other pixel by pixel. The slope term is what keeps a
/// steeply-angled triangle's edge from sinking into it.
///
/// Positive, because depth is reversed: toward the camera is *up* the range
/// now, and the bias that used to be negative would now push the lines behind
/// the surface they outline — which is the same flicker with an extra step.
const WIRE_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: 2,
    slope_scale: 1.0,
    clamp: 0.0,
};

/// Everything about a pipeline except which shader it runs.
///
/// One struct rather than the pair of booleans it replaces, because the pair
/// lied: the old helper took a `cull` flag and spent it on back-face culling
/// *and* on depth writing, so a surface that wanted one silently got the
/// other. They are unrelated decisions — a ghost surface is culled and writes
/// no depth, a scaffold line is unculled and writes none — and each has to be
/// said out loud before reversed-Z and the transparent helpers start depending
/// on it.
#[derive(Debug, Clone, Copy)]
struct PipelineState {
    topology: wgpu::PrimitiveTopology,
    cull_mode: Option<wgpu::Face>,
    /// Whether this pipeline runs in a pass that has a depth buffer at all.
    ///
    /// The scaffolding does not. It is drawn after the occlusion composite,
    /// into the resolved single-sampled target, precisely so that occlusion
    /// does not darken it — and a pass over that target has neither the scene's
    /// depth buffer, which is multisampled and cannot be attached to a
    /// single-sampled pipeline, nor any use for one, since the scaffolding
    /// compares `Always` regardless.
    depth: bool,
    /// Whether it is multisampled, which is to say whether it draws into the
    /// scene's own target or into the resolved one.
    multisampled: bool,
    /// Whether what this pipeline draws becomes the depth everything after it
    /// is tested against.
    depth_write: bool,
    depth_compare: wgpu::CompareFunction,
    depth_bias: wgpu::DepthBiasState,
    /// `None` is opaque. The solid surface returns alpha 1 and so looked the
    /// same blended, but blending it told the driver the frame was one it
    /// could not reject fragments in, for nothing.
    blend: Option<wgpu::BlendState>,
}

impl PipelineState {
    /// The solid sculpt: culled, writing depth, not blended.
    fn opaque(topology: wgpu::PrimitiveTopology) -> Self {
        Self {
            topology,
            cull_mode: Some(wgpu::Face::Back),
            depth: true,
            multisampled: true,
            depth_write: true,
            depth_compare: DEPTH_COMPARE,
            depth_bias: NO_BIAS,
            blend: None,
        }
    }

    /// A helper drawn through: unculled, blended, and leaving the depth buffer
    /// alone so what is behind it still reads.
    fn transparent(topology: wgpu::PrimitiveTopology) -> Self {
        Self {
            topology,
            cull_mode: None,
            depth: true,
            multisampled: true,
            depth_write: false,
            depth_compare: DEPTH_COMPARE,
            depth_bias: NO_BIAS,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        }
    }

    /// Scaffolding — the cage, the manipulator, an object's outline — drawn
    /// wherever it is, whatever stands in front of it.
    ///
    /// After the occlusion composite and into the resolved target, so it is
    /// drawn *on* the frame rather than *in* it: a manipulator standing over an
    /// occluded fold used to be darkened by that fold's occlusion, which made
    /// the handle a person is aiming at dimmer exactly where the form is
    /// deepest. It compares `Always` and writes no depth, so it has no use for
    /// the depth buffer it is now separated from.
    fn scaffold(topology: wgpu::PrimitiveTopology) -> Self {
        Self {
            depth: false,
            multisampled: false,
            depth_compare: wgpu::CompareFunction::Always,
            ..Self::transparent(topology)
        }
    }

    /// Ink over the surface it outlines.
    fn wire() -> Self {
        Self {
            depth_bias: WIRE_BIAS,
            ..Self::transparent(wgpu::PrimitiveTopology::LineList)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn make_pipeline(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    vs: &str,
    fs: &str,
    state: PipelineState,
) -> wgpu::RenderPipeline {
    // Read from the same place the framebuffer reads it, so the two cannot
    // disagree — a pipeline whose sample count differs from its attachment's
    // is a validation error at draw time rather than at creation.
    let samples = if state.multisampled {
        gpu.sample_count(format)
    } else {
        1
    };
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
                    blend: state.blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: state.topology,
                cull_mode: state.cull_mode,
                ..Default::default()
            },
            depth_stencil: state.depth.then(|| wgpu::DepthStencilState {
                format: Framebuffer::DEPTH_FORMAT,
                depth_write_enabled: state.depth_write,
                depth_compare: state.depth_compare,
                stencil: Default::default(),
                bias: state.depth_bias,
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
    // Every level rendered from the material's own recipe at that level's
    // size, rather than the coarser levels being filtered down from the finest.
    //
    // Downsampling would be wrong twice over. The image is stored sRGB-encoded,
    // so averaging its bytes averages in the wrong space and darkens every
    // level; and a MatCap is a *function of the normal* sampled on a grid, so
    // the honest coarse version is that function sampled coarsely — which the
    // recipe can produce exactly. It costs a few hundred microseconds once per
    // material change, which is a click.
    //
    // Why they are needed at all: a subtool far enough away that its normals
    // vary by more than a texel between neighbouring pixels samples the
    // material at random, and the shading sparkles as the camera moves. That
    // is the case mipmaps exist for, and the texture had none.
    let levels = SIZE.ilog2() + 1;
    let mut pixels = Vec::new();
    for level in 0..levels {
        pixels.extend(matcap.generate((SIZE >> level).max(1)));
    }
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("matcap"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::MipMajor,
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

/// Puts a reference image on a texture, with a mip chain.
///
/// Unlike a MatCap there is no recipe to re-render a coarse level from: a
/// reference is somebody's photograph. So the levels are filtered here, in
/// *linear* colour — decoded, averaged, re-encoded. Averaging the sRGB bytes
/// directly is the usual mistake and it darkens every level, which on a
/// reference reads as the opacity dial being wrong at a distance.
fn upload_reference(gpu: &Gpu, pixels: &[u8], width: u32, height: u32) -> wgpu::TextureView {
    let chain = mip_chain(pixels, width, height);
    let levels = chain.len() as u32;
    let data: Vec<u8> = chain.concat();
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("reference"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // sRGB, like the matcap beside it: a photograph stored as sRGB and
            // sampled as linear comes out washed out, which on a reference
            // reads as the opacity being wrong.
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::MipMajor,
        &data,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// An RGBA8 sRGB image and every mip level below it, each halved and rounded
/// up, down to one texel.
///
/// Alpha is averaged directly and colour is averaged premultiplied by it, so a
/// cut-out reference does not bleed the colour of its transparent texels into
/// its edge as the levels get coarser.
fn mip_chain(pixels: &[u8], width: u32, height: u32) -> Vec<Vec<u8>> {
    let mut levels = vec![pixels.to_vec()];
    let (mut w, mut h) = (width, height);
    while w > 1 || h > 1 {
        let source = levels.last().expect("the chain starts with level zero");
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = Vec::with_capacity((nw * nh * 4) as usize);
        for y in 0..nh {
            for x in 0..nw {
                let mut colour = [0.0f32; 3];
                let mut alpha = 0.0f32;
                let mut taken = 0.0f32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                        let at = ((sy * w + sx) * 4) as usize;
                        let a = source[at + 3] as f32 / 255.0;
                        for c in 0..3 {
                            colour[c] += from_srgb8(source[at + c]) * a;
                        }
                        alpha += a;
                        taken += 1.0;
                    }
                }
                // Back out of the premultiply. Where the whole block was
                // transparent there is no colour to recover and none to show.
                let weight = if alpha > 0.0 { 1.0 / alpha } else { 0.0 };
                next.extend_from_slice(&[
                    to_srgb8(colour[0] * weight),
                    to_srgb8(colour[1] * weight),
                    to_srgb8(colour[2] * weight),
                    (alpha / taken * 255.0 + 0.5) as u8,
                ]);
            }
        }
        levels.push(next);
        (w, h) = (nw, nh);
    }
    levels
}

/// 8-bit sRGB to linear, for filtering that has to happen in linear colour.
fn from_srgb8(value: u8) -> f32 {
    let c = value as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear back to 8-bit sRGB.
fn to_srgb8(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0 + 0.5) as u8
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

    /// The uniform structs are a contract with WGSL that nothing checks.
    ///
    /// A field added on the Rust side and forgotten in the shader is not a
    /// compile error on either side: the buffer is written one way and read
    /// another, and what comes out is a frame drawn wrong. These sizes are the
    /// shader's own — `mat4x4<f32>` is 64 bytes, `vec4<f32>` is 16, and a
    /// uniform struct's stride is a multiple of 16 — so a struct that grows
    /// fails here, next to the definition, rather than on a device.
    #[test]
    fn the_uniforms_are_the_size_their_shader_declarations_are() {
        // camera: two mat4x4.
        assert_eq!(std::mem::size_of::<CameraUniform>(), 128);
        // material: three vec4.
        assert_eq!(std::mem::size_of::<MaterialUniform>(), 48);
        // ao: two mat4x4, five vec4 and a sixteen-entry vec4 kernel.
        assert_eq!(std::mem::size_of::<AoUniform>(), 208 + AO_KERNEL * 16);
        for size in [
            std::mem::size_of::<CameraUniform>(),
            std::mem::size_of::<MaterialUniform>(),
            std::mem::size_of::<AoUniform>(),
        ] {
            assert_eq!(size % 16, 0, "a uniform struct is aligned to sixteen bytes");
        }
    }

    /// The vertex layout is stated in three places — the struct, the offset
    /// constants the engine's copy writes at, and the attribute descriptors —
    /// and only the first is checked by the compiler.
    #[test]
    fn the_vertex_attributes_are_where_the_offsets_say() {
        assert_eq!(Vertex::STRIDE, 40);
        let layout = Vertex::layout();
        assert_eq!(layout.array_stride, Vertex::STRIDE as u64);
        let offsets: Vec<u64> = layout.attributes.iter().map(|a| a.offset).collect();
        assert_eq!(
            offsets,
            vec![
                Vertex::POSITION_OFFSET as u64,
                Vertex::NORMAL_OFFSET as u64,
                Vertex::COLOR_OFFSET as u64,
                Vertex::MASK_OFFSET as u64,
            ]
        );
    }

    /// The opaque surface is the one pipeline that must not blend, and the one
    /// that must write depth. Every other pipeline is a helper drawn over it.
    ///
    /// These were one boolean until this change: `cull` decided back-face
    /// culling and depth writing together, so the two could not be told apart
    /// and neither could be tested.
    #[test]
    fn only_the_opaque_state_writes_depth_and_does_not_blend() {
        let opaque = PipelineState::opaque(wgpu::PrimitiveTopology::TriangleList);
        assert!(opaque.blend.is_none(), "the solid surface must not blend");
        assert!(opaque.depth_write);
        assert_eq!(opaque.cull_mode, Some(wgpu::Face::Back));

        for helper in [
            PipelineState::transparent(wgpu::PrimitiveTopology::TriangleList),
            PipelineState::scaffold(wgpu::PrimitiveTopology::LineList),
            PipelineState::wire(),
        ] {
            assert!(helper.blend.is_some(), "a helper is drawn through");
            assert!(!helper.depth_write, "a helper does not occlude the sculpt");
            assert_eq!(helper.cull_mode, None);
        }
    }

    /// Scaffolding is drawn wherever it is and the polyframe is not: a cage's
    /// control points are reached through the form, a wireframe sits on it.
    #[test]
    fn scaffolding_ignores_depth_and_the_polyframe_does_not() {
        assert_eq!(
            PipelineState::scaffold(wgpu::PrimitiveTopology::LineList).depth_compare,
            wgpu::CompareFunction::Always
        );
        let wire = PipelineState::wire();
        assert_eq!(wire.depth_compare, DEPTH_COMPARE);
        assert_ne!(
            wire.depth_bias.constant, 0,
            "without a bias the polyframe fights the triangles it outlines"
        );
    }

    /// The mip chain halves to one texel and stops.
    #[test]
    fn a_reference_mip_chain_reaches_one_texel() {
        // Not a power of two on either axis, and not square: a real
        // photograph is neither, and the rounding is where a chain goes wrong.
        let (width, height) = (100u32, 37u32);
        let pixels = vec![128u8; (width * height * 4) as usize];
        let chain = mip_chain(&pixels, width, height);

        let mut expected = Vec::new();
        let (mut w, mut h) = (width, height);
        loop {
            expected.push((w, h));
            if w == 1 && h == 1 {
                break;
            }
            (w, h) = ((w / 2).max(1), (h / 2).max(1));
        }
        assert_eq!(chain.len(), expected.len());
        for (level, (w, h)) in chain.iter().zip(expected) {
            assert_eq!(
                level.len(),
                (w * h * 4) as usize,
                "a {w}x{h} level is the wrong size"
            );
        }
    }

    /// The averaging happens in linear colour, not over the stored bytes.
    ///
    /// This is the whole reason the chain is built here rather than by a
    /// generic downsampler. Half black and half white is *linear* 0.5, which
    /// sRGB encodes as about 188 — averaging the bytes gives 128, which is
    /// linear 0.21, and a chain built that way darkens every level. On a
    /// reference plane that reads as the opacity dial being wrong whenever the
    /// camera pulls back.
    #[test]
    fn reference_mips_are_filtered_in_linear_colour() {
        // A 2x1 image: one black texel, one white.
        let pixels = vec![0, 0, 0, 255, 255, 255, 255, 255];
        let chain = mip_chain(&pixels, 2, 1);
        assert_eq!(chain.len(), 2);
        let averaged = chain[1][0];
        assert!(
            (180..=196).contains(&averaged),
            "half black and half white averaged to {averaged}; 128 would be \
             the sRGB bytes averaged directly, which is the mistake this \
             filter exists to avoid"
        );
    }

    /// A transparent texel contributes no colour, so a cut-out reference does
    /// not bleed whatever was stored behind its alpha into its own edge.
    #[test]
    fn a_transparent_texel_does_not_tint_the_level_above_it() {
        // One opaque white texel, one fully transparent one that happens to
        // carry black — which is exactly what an exporter leaves behind.
        let pixels = vec![255, 255, 255, 255, 0, 0, 0, 0];
        let chain = mip_chain(&pixels, 2, 1);
        assert_eq!(chain[1][0], 255, "the transparent texel darkened the mip");
        assert_eq!(chain[1][3], 128, "the coverage should have halved");
    }

    /// The sRGB conversions are each other's inverse, which everything above
    /// rests on.
    #[test]
    fn the_srgb_conversions_round_trip() {
        for value in 0u8..=255 {
            assert_eq!(
                to_srgb8(from_srgb8(value)),
                value,
                "{value} did not survive a round trip through linear"
            );
        }
    }

    /// The occlusion figures are fractions of what is being drawn, so the
    /// radius has to be the radius of that and not of its bounding box's
    /// diagonal or of nothing at all.
    #[test]
    fn the_form_radius_is_half_the_longest_side() {
        // The reference form's starting sphere: radius 1, which is what the
        // occlusion fractions were tuned against.
        assert_eq!(
            form_radius(Some((Vec3::splat(-1.0), Vec3::splat(1.0)))),
            1.0
        );
        // A long thin form is measured by its longest side, so occlusion does
        // not vanish on something wide and flat.
        assert_eq!(
            form_radius(Some((
                Vec3::new(-5.0, -0.1, -0.1),
                Vec3::new(5.0, 0.1, 0.1)
            ))),
            5.0
        );
        // Nothing drawn, and a degenerate box: one rather than zero, or the
        // depth sharpness divides by nothing.
        assert_eq!(form_radius(None), 1.0);
        assert_eq!(form_radius(Some((Vec3::ZERO, Vec3::ZERO))), 1.0);
    }

    /// The occlusion radius is measured against everything drawn with depth,
    /// because either the surface or the mesh layers may be the only one
    /// present — and a scene shaded to the size of half of itself is shaded
    /// wrong.
    #[test]
    fn the_bounds_are_the_union_of_what_is_drawn() {
        let a = (Vec3::new(-1.0, -1.0, -1.0), Vec3::new(0.0, 0.0, 0.0));
        let b = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.0, 1.0, 1.0));
        assert_eq!(
            union_bounds(Some(a), Some(b)),
            Some((Vec3::splat(-1.0), Vec3::new(3.0, 1.0, 1.0)))
        );
        assert_eq!(union_bounds(Some(a), None), Some(a));
        assert_eq!(union_bounds(None, Some(b)), Some(b));
        assert_eq!(union_bounds(None, None), None);
    }

    /// Growth is geometric and never shrinks.
    ///
    /// The figure that matters is the *number of reallocations*: geometry
    /// grows a few thousand vertices at a time and the buffer holds millions,
    /// so allocating exactly what was asked for meant a fresh buffer and a
    /// fresh copy of the whole surface on nearly every edit that grew it.
    #[test]
    fn buffers_grow_geometrically() {
        // A requirement larger than half again as much is honoured exactly:
        // a jump this big is a new model, not growth.
        assert_eq!(grown(100, 10_000), 10_000);
        // And a small one takes the geometric step instead.
        assert_eq!(grown(1_000, 1_001), 1_500);
        assert_eq!(grown(0, 1), 1);

        // Growing one vertex at a time from a thousand to a million: a linear
        // policy reallocates a million times.
        let mut capacity = 1_000usize;
        let mut reallocations = 0;
        for required in 1_001..=1_000_000 {
            if required > capacity {
                capacity = grown(capacity, required);
                reallocations += 1;
            }
        }
        assert!(
            reallocations < 20,
            "{reallocations} reallocations to reach a million vertices"
        );
    }

    /// A span's box covers the vertices its indices name and nothing else, so
    /// culling one subtool cannot be decided by another's geometry.
    #[test]
    fn a_span_is_bounded_by_the_vertices_it_names() {
        let vertices: Vec<Vertex> = [[0.0, 0.0, 0.0], [1.0, 2.0, 3.0], [-9.0, -9.0, -9.0]]
            .into_iter()
            .map(at)
            .collect();
        // The third vertex is in the buffer and outside the span.
        assert_eq!(
            span_bounds(&vertices, &[0, 1, 0, 2], &(0..2)),
            Some(([0.0, 0.0, 0.0], [1.0, 2.0, 3.0]))
        );
        // An empty range bounds nothing, and so is never culled.
        assert_eq!(span_bounds(&vertices, &[0, 1], &(0..0)), None);
        // A range past the end of the buffer is a span that outlived the
        // geometry it described. It must not be culled against a box made up
        // from whatever the indices happened to reach.
        assert_eq!(span_bounds(&vertices, &[0, 1], &(0..9)), None);
        assert_eq!(span_bounds(&vertices, &[7], &(0..1)), None);
    }

    /// The occlusion scale is stated twice — once as the constant that sizes
    /// the target, and once in the shader that reads it.
    ///
    /// It is stated twice so that the shader's loop bounds are visible in the
    /// shader. Two copies of one number is exactly the arrangement that goes
    /// wrong silently — a target sized for one span, read by a shader assuming
    /// another, draws occlusion offset from the frame it shades — so this is
    /// what keeps them together.
    #[test]
    fn the_shader_and_the_framebuffer_agree_on_the_occlusion_scale() {
        let source = include_str!("shaders/ao.wgsl");
        let declared = source
            .lines()
            .find_map(|line| line.trim().strip_prefix("const AO_SPAN: i32 = "))
            .and_then(|value| value.trim_end_matches(';').parse::<u32>().ok())
            .expect("ao.wgsl declares AO_SPAN");
        assert_eq!(
            declared,
            Framebuffer::AO_SCALE,
            "the shader reduces {declared} display pixels to an occlusion \
             pixel and the framebuffer sizes the target for \
             {}",
            Framebuffer::AO_SCALE
        );
    }

    /// The precomputed kernel is the distribution the shader used to compute.
    ///
    /// Moving it to the host is a pure optimisation, so the thing to hold is
    /// that it changed nothing: unit directions in the upper hemisphere, an
    /// even spread over the disc rather than a crowd at its centre, and reaches
    /// that grow with the index so the near field is sampled as densely as the
    /// far one.
    #[test]
    fn the_occlusion_kernel_covers_the_hemisphere_evenly() {
        for count in [6usize, 8, 12, 16] {
            let kernel = ao_kernel(count);
            let mut previous_reach = 0.0f32;
            let mut mean = [0.0f32; 3];
            for entry in kernel.iter().take(count) {
                let [x, y, z, reach] = *entry;
                let length = (x * x + y * y + z * z).sqrt();
                assert!(
                    (length - 1.0).abs() < 1e-4,
                    "a sample direction of length {length}"
                );
                assert!(z >= 0.0, "a sample pointing into the surface: z = {z}");
                assert!(
                    (0.1..=1.0).contains(&reach),
                    "a reach of {reach} of the radius"
                );
                assert!(
                    reach >= previous_reach,
                    "reaches must grow with the index, and {reach} follows {previous_reach}"
                );
                previous_reach = reach;
                for (axis, value) in mean.iter_mut().zip([x, y, z]) {
                    *axis += value / count as f32;
                }
            }
            // Spread rather than clustered: the mean of an even hemisphere
            // leans along the normal and barely at all across it.
            assert!(
                mean[0].abs() < 0.25 && mean[1].abs() < 0.25,
                "the kernel leans sideways: mean {mean:?} over {count} samples"
            );
            assert!(mean[2] > 0.3, "the kernel does not face the surface");
        }
    }

    /// Entries past the count are never read, and must not be a direction of
    /// no length in case one ever is.
    #[test]
    fn the_unused_kernel_entries_are_still_directions() {
        let kernel = ao_kernel(6);
        for entry in kernel.iter().skip(6) {
            let length = (entry[0] * entry[0] + entry[1] * entry[1] + entry[2] * entry[2]).sqrt();
            assert!(
                (length - 1.0).abs() < 1e-4,
                "an unused entry of length {length}"
            );
        }
        // And a count of zero is a count of one rather than a division by it.
        let _ = ao_kernel(0);
        let _ = ao_kernel(usize::MAX);
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
