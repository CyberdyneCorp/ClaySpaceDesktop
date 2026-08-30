//! Drawing the sculpt.
//!
//! The renderer takes plain vertex and index data. It knows nothing about
//! ClayCore — that is the layering rule, and it is also what lets the same
//! code draw a document, a voxel grid or a test fixture without caring which.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::camera::Camera;
use crate::frustum::Frustum;
use crate::gpu::{Framebuffer, Gpu};
use crate::matcap::MatCap;
use crate::palette;
use crate::profiler::{GpuFrameTiming, GpuPass, GpuProfiler};
use crate::quality::{ShadingMode, StudioMaterial, ViewportQuality};
use clayspace_model::{GizmoHandle, GizmoMode, LayerKey, SurfaceOpacity};

mod ao;
mod overlays;
mod pipelines;
mod textures;

use ao::*;
use overlays::*;
pub use overlays::{frame_about, BRACKET_REACH, RING_REACH, SCALE_BOX_REACH, VIEW_RING_REACH};
use pipelines::*;
pub use textures::Reference;
use textures::*;

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
                source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/matcap.wgsl").into()),
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
            shader_source(include_str!("../shaders/ao.wgsl"))
        } else {
            shader_source(include_str!("../shaders/ao.wgsl"))
                .replace("texture_depth_multisampled_2d", "texture_depth_2d")
        };
        let ao_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("ao"),
                source: wgpu::ShaderSource::Wgsl(ao_source.into()),
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
                source: wgpu::ShaderSource::Wgsl(
                    shader_source(include_str!("../shaders/fxaa.wgsl")).into(),
                ),
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

/// A shader source, with the shared definitions in front of it.
///
/// WGSL has no include. Two of the three shaders here draw a fullscreen
/// triangle and had an identical copy of the twelve lines that do it, and the
/// failure mode of two identical copies is that one of them is edited — so the
/// copy that survives is prepended here instead. See `shaders/common.wgsl`.
fn shader_source(body: &str) -> String {
    format!("{}\n{body}", include_str!("../shaders/common.wgsl"))
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
        let source = include_str!("../shaders/ao.wgsl");
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
        // does not re-implement the field. If one of these appears in any of
        // them, the drift this project is built to avoid has started.
        //
        // Every shader, read off the directory rather than listed here. A list
        // exempts whatever is not on it, silently, and the shader most likely
        // to be tempted into a field march is the next one somebody writes:
        // the occlusion pass reads the depth the mesh wrote, and reaching for
        // a distance function there would look like a shortcut rather than
        // like a layering violation.
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shaders");
        let shaders: Vec<std::path::PathBuf> = std::fs::read_dir(&directory)
            .expect("the shader directory")
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                (path.extension()? == "wgsl").then_some(path)
            })
            .collect();
        assert!(
            shaders.len() >= 4,
            "found {} shaders in {}, which is fewer than there are",
            shaders.len(),
            directory.display()
        );

        for path in shaders {
            let source = std::fs::read_to_string(&path)
                .expect("a shader")
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
                    !source.contains(forbidden),
                    "{} names `{forbidden}`, which is field math in a shader",
                    path.display()
                );
            }
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
