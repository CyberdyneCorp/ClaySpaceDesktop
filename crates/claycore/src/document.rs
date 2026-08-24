//! Documents, layers and the items placed in them.
//!
//! A [`Document`] owns its engine handle and releases it on drop. It is `Send`
//! but not `Sync`: the engine's header states that calls on one handle are the
//! host's to serialize, while the batched evaluation entry point is
//! free-threaded against one const document. [`Document::eval_points`] takes
//! `&self` accordingly; everything that mutates takes `&mut self`.

use std::path::Path;
use std::ptr::NonNull;

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::error::{check, ErrorKind, Result};
use crate::mesh::{Mesh, MeshLayerDesc, MeshParams};
use crate::{cstring, raw_failure, Backend};

/// A layer within a document. Borrowed: the document owns the layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(pub(crate) sys::clay_layer_id);

/// One edit to a placed armature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArmatureEdit {
    /// A new sphere under `target`.
    AddChild {
        position: [f32; 3],
        radius: f32,
    },
    /// Moves `target` and its whole subtree by a delta.
    Move {
        delta: [f32; 3],
    },
    SetRadius {
        radius: f32,
    },
    /// Removes `target` and everything under it.
    Delete,
    /// Makes `target` cut into the rig, or add to it again.
    ///
    /// A new child is always positive; this is what flips it. The node keeps
    /// its children either way — they keep their own signs.
    SetSign {
        negative: bool,
    },
}

/// A node placed in a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) sys::clay_node_id);

impl NodeId {
    /// A layer's root, which is the group every top-level node hangs under.
    ///
    /// The one node id a host can name without having been handed it, and
    /// therefore where enumerating a reloaded layer starts.
    pub const ROOT: Self = Self(0);

    /// The raw id, for a host that has to log or compare one.
    pub fn get(self) -> u32 {
        self.0
    }
}

// `NodeId::from_raw` used to live here — a node id a host was *probing for*
// rather than holding, because nothing in the ABI enumerated a layer's nodes.
// `Document::layer_nodes` does since ClayCore 0.30.0 (#91), and probing was
// never sound: ids are not dense, so a scan lost everything past the longest
// gap it tolerated. It came out with its only caller rather than staying
// available to write the same bug again.

/// The primitive values `Document::node_prim` reports, for the few a host has
/// to recognise by name.
pub mod prim {
    use claycore_sys as sys;

    pub const ARMATURE: i32 = sys::clay_prim::CLAY_PRIM_ARMATURE as i32;
    pub const STROKE: i32 = sys::clay_prim::CLAY_PRIM_STROKE as i32;
}

/// An item under construction, owned by the caller until it is added to a
/// layer. Adding copies it, so one builder may be placed any number of times.
pub struct Item {
    raw: NonNull<sys::clay_item>,
}

impl Item {
    /// Builds a sphere of the given radius.
    pub fn sphere(radius: f32) -> Result<Self> {
        Self::new(sys::clay_prim::CLAY_PRIM_SPHERE as i32, &[radius])
    }

    /// Builds a swept-sphere chain — the primitive a snakehook resolves into.
    ///
    /// Its points are set separately, so it takes no parameters here.
    pub fn stroke() -> Result<Self> {
        Self::new(sys::clay_prim::CLAY_PRIM_STROKE as i32, &[])
    }

    /// A profile carried along a guide curve — the primitive a tube is.
    ///
    /// The guide is the item's own curve points, set with
    /// [`Item::set_curve_points`]; the profiles are added with
    /// [`Item::add_profile`], and the engine is explicit that neither is a new
    /// kind of thing: "a guide is not a new kind of curve and a swept profile
    /// is not a new kind of profile". `easing` is the one parameter, indexing
    /// the curve that interpolates between profiles along the guide.
    pub fn swept(easing: f32) -> Result<Self> {
        Self::new(sys::clay_prim::CLAY_PRIM_SWEPT as i32, &[easing])
    }

    /// One 2D profile of a lift primitive — a loft's or a sweep's.
    ///
    /// Two or more are interpolated evenly along the guide; one is carried
    /// unchanged. A lift with none set uses a unit circle.
    pub fn add_profile(&mut self, profile: Profile, params: &[f32]) -> Result<()> {
        // SAFETY: a valid item, an enum the entry point range-checks, and a
        // slice whose length is passed beside it. No polygon here — the
        // arbitrary-vertex form takes its own array, and null with a zero
        // count is what "not a polygon" spells.
        check(
            unsafe {
                sys::clay_item_add_loft_profile(
                    self.as_ptr(),
                    profile as i32,
                    params.as_ptr(),
                    params.len(),
                    std::ptr::null(),
                    0,
                )
            },
            "clay_item_add_loft_profile",
        )
    }

    /// A tree of spheres, skinned by one sphere-swept cone per node-parent
    /// pair — the engine's words, and ZBrush's ZSpheres.
    ///
    /// The nodes are the points [`Item::set_stroke_points`] takes; the tree is
    /// the parent array beside them. An armature whose parents form a line is
    /// a stroke and evaluates identically to one.
    pub fn armature() -> Result<Self> {
        Self::new(sys::clay_prim::CLAY_PRIM_ARMATURE as i32, &[])
    }

    /// One parent index per node; a node whose parent is itself is a root.
    ///
    /// An index outside the range, or a chain that closes a cycle, is refused
    /// by the engine rather than accepted — a cycle would make the field
    /// depend on traversal order rather than on the tree.
    pub fn set_armature_parents(&mut self, parents: &[u32]) -> Result<()> {
        // SAFETY: valid handle; `parents` holds exactly `len` indices.
        check(
            unsafe {
                sys::clay_item_set_armature_parents(self.as_ptr(), parents.as_ptr(), parents.len())
            },
            "clay_item_set_armature_parents",
        )
    }

    /// One sign per node: `false` adds to the rig, `true` cuts into it.
    ///
    /// ZBrush's negative ZSphere. The field is the armature of the positive
    /// nodes *minus* the armature of the negative ones, each half built as the
    /// unsigned armature is. A link exists only between two nodes of the same
    /// sign, so the skin along a negative node's links is never drawn — the
    /// membrane cut — and a carve never sweeps a positive parent's radius, so
    /// an eye socket does not swallow the head.
    ///
    /// An array shorter than the nodes reads as positive-padded, exactly as a
    /// short parent array reads as roots. A negative node may carry children.
    ///
    /// Added in ClayCore 0.30.0, closing #99.
    pub fn set_armature_signs(&mut self, negative: &[bool]) -> Result<()> {
        // The ABI takes +1 or -1 and refuses any other magnitude: a magnitude
        // here would be the negative-radius convention the feature explicitly
        // did not take.
        let signs: Vec<i8> = negative.iter().map(|n| if *n { -1 } else { 1 }).collect();
        // SAFETY: valid handle; `signs` holds exactly `len` values.
        check(
            unsafe {
                sys::clay_item_set_armature_signs(self.as_ptr(), signs.as_ptr(), signs.len())
            },
            "clay_item_set_armature_signs",
        )
    }

    /// Appends one node under `parent`, or under the last node when `None`.
    ///
    /// `None` is what dragging a new sphere out of the previous one does.
    pub fn add_child(
        &mut self,
        position: [f32; 3],
        radius: f32,
        parent: Option<u32>,
    ) -> Result<()> {
        // SAFETY: valid handle and a three-float position.
        check(
            unsafe {
                sys::clay_item_add_child(
                    self.as_ptr(),
                    position.as_ptr(),
                    radius,
                    parent.map_or(-1, |p| p as i32),
                )
            },
            "clay_item_add_child",
        )
    }

    /// The chain's control points, as `x y z r` quadruples.
    ///
    /// The radius travels with each point, which is what lets a tendril taper
    /// toward its tip.
    /// The same chain, with every point joined to the next by a curve rather
    /// than a straight segment.
    ///
    /// A stroke's points are hard corners by default, which is right for a
    /// chain authored before types existed and wrong for a tendril pulled
    /// along a curving drag: every sample becomes a kink. Catmull-Rom passes
    /// *through* the points, so the curve is the path the pointer took.
    ///
    /// Tessellated into the same segment chain at compile time, so it costs
    /// nothing at evaluation and no backend knows it exists.
    pub fn set_curve_points(&mut self, points_xyzr: &[f32], kind: PointType) -> Result<()> {
        if points_xyzr.len() % 4 != 0 {
            return Err(crate::raw_failure(
                "clay_item_set_curve_points",
                ErrorKind::InvalidArgument,
            ));
        }
        let count = points_xyzr.len() / 4;
        let types = vec![kind as i32; count];
        // SAFETY: `count` quadruples and `count` types, both guaranteed by the
        // length check and the vector above; the two handle arrays are
        // optional and null leaves them at their defaults.
        check(
            unsafe {
                sys::clay_item_set_curve_points(
                    self.as_ptr(),
                    points_xyzr.as_ptr(),
                    count,
                    types.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            },
            "clay_item_set_curve_points",
        )
    }

    pub fn set_stroke_points(&mut self, points_xyzr: &[f32]) -> Result<()> {
        if points_xyzr.len() % 4 != 0 {
            return Err(crate::raw_failure(
                "clay_item_set_stroke_points",
                ErrorKind::InvalidArgument,
            ));
        }
        // SAFETY: the engine reads `count` quadruples from the pointer, which
        // is what the length check above guarantees.
        check(
            unsafe {
                sys::clay_item_set_stroke_points(
                    self.as_ptr(),
                    points_xyzr.as_ptr(),
                    points_xyzr.len() / 4,
                )
            },
            "clay_item_set_stroke_points",
        )
    }

    /// How wide the chain's links blend into one another.
    pub fn set_stroke_blend_k(&mut self, k: f32) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_item_set_stroke_blend_k(self.as_ptr(), k) },
            "clay_item_set_stroke_blend_k",
        )
    }

    pub(crate) fn from_raw(raw: *mut sys::clay_item, operation: &'static str) -> Result<Self> {
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure(operation, ErrorKind::InvalidArgument))
    }

    pub(crate) fn as_ptr(&self) -> *mut sys::clay_item {
        self.raw.as_ptr()
    }

    /// Builds any primitive from its parameter list. The engine documents the
    /// expected count per primitive and rejects a wrong one.
    pub fn new(prim: i32, params: &[f32]) -> Result<Self> {
        // SAFETY: `params` is a valid slice for `params.len()` floats, and the
        // engine copies what it needs before returning.
        let raw = unsafe { sys::clay_item_create(prim, params.as_ptr(), params.len()) };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_item_create", ErrorKind::InvalidArgument))
    }

    /// Places the item at a world position.
    pub fn set_position(&mut self, position: [f32; 3]) -> Result<()> {
        // SAFETY: the handle is non-null and owned here; `position` is three
        // floats as the entry point requires.
        check(
            unsafe { sys::clay_item_set_position(self.raw.as_ptr(), position.as_ptr()) },
            "clay_item_set_position",
        )
    }
}

impl std::fmt::Debug for Item {
    /// Opaque: an item builder holds engine state with no cheap summary, and
    /// formatting one must not compile anything.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Item(..)")
    }
}

impl Drop for Item {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value, non-null, and released
        // exactly once because `Item` is not `Copy` or `Clone`.
        unsafe { sys::clay_item_destroy(self.raw.as_ptr()) };
    }
}

/// A sculpting document: layers, the items in them, and the field they define.
pub struct Document {
    raw: NonNull<sys::clay_document>,
}

impl std::fmt::Debug for Document {
    /// Deliberately opaque: the engine exposes no cheap summary of a document,
    /// and formatting one must not compile a tape.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Document(..)")
    }
}

// SAFETY: the engine's header states that a document may be moved between
// threads and read from several at once, but that calls on one handle are the
// host's to serialize. `Send` expresses the first; the absence of `Sync`
// expresses the second, since `&Document` methods that reach the engine are
// limited to the free-threaded batch entry points.
unsafe impl Send for Document {}

impl Document {
    /// Creates an empty document.
    pub fn new() -> Result<Self> {
        // SAFETY: takes no arguments and returns either a fresh owned handle
        // or null.
        let raw = unsafe { sys::clay_document_create() };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_document_create", ErrorKind::Backend))
    }

    /// Opens a `.clayspace` document.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let c_path = cstring(path.to_string_lossy().as_ref(), "clay_document_load")?;
        let mut raw: *mut sys::clay_document = std::ptr::null_mut();
        // SAFETY: `c_path` is NUL-terminated and outlives the call; `raw` is a
        // valid out-parameter which the engine fills only on success.
        check(
            unsafe { sys::clay_document_load(c_path.as_ptr(), &mut raw) },
            "clay_document_load",
        )?;
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_document_load", ErrorKind::Io))
    }

    /// Writes the document to a `.clayspace` file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let c_path = cstring(path.to_string_lossy().as_ref(), "clay_document_save")?;
        // SAFETY: the handle is valid; `c_path` is NUL-terminated and outlives
        // the call.
        check(
            unsafe { sys::clay_document_save(self.raw.as_ptr(), c_path.as_ptr()) },
            "clay_document_save",
        )
    }

    /// Adds an SDF layer and returns its id.
    pub fn add_sdf_layer(&mut self, name: &str) -> Result<LayerId> {
        let c_name = cstring(name, "clay_add_sdf_layer")?;
        let mut layer: sys::clay_layer_id = Default::default();
        // SAFETY: the handle is valid and uniquely borrowed; `c_name` is
        // NUL-terminated; `layer` is a valid out-parameter.
        check(
            unsafe { sys::clay_add_sdf_layer(self.raw.as_ptr(), c_name.as_ptr(), &mut layer) },
            "clay_add_sdf_layer",
        )?;
        Ok(LayerId(layer))
    }

    /// Places an item in a layer. The item is copied, so the builder remains
    /// usable afterwards.
    pub fn add_item(&mut self, layer: LayerId, item: &Item) -> Result<NodeId> {
        let mut node: sys::clay_node_id = Default::default();
        // SAFETY: all three handles are valid; the engine copies the item and
        // writes the new node id into `node` only on success.
        check(
            unsafe {
                sys::clay_layer_add_item(self.raw.as_ptr(), layer.0, item.raw.as_ptr(), &mut node)
            },
            "clay_layer_add_item",
        )?;
        Ok(NodeId(node))
    }

    /// Evaluates the field at a batch of points.
    ///
    /// Takes `&self`: the engine documents this entry point as free-threaded
    /// against one const document. `backend` selects where it runs; `None`
    /// means the CPU reference path. Backend choice changes speed, never
    /// results.
    pub fn eval_points(&self, backend: Option<&Backend>, points: &[[f32; 3]]) -> Result<Vec<f32>> {
        let mut distances = vec![0.0f32; points.len()];
        if points.is_empty() {
            return Ok(distances);
        }

        let name = backend
            .map(|b| cstring(b.as_str(), "clay_eval_points"))
            .transpose()?;
        let name_ptr = name.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());

        // SAFETY: `points` is `points.len() * 3` contiguous floats — `[f32; 3]`
        // has no padding — and `distances` is `points.len()` floats. Colours
        // are declined with a null pointer, which the entry point permits.
        check(
            unsafe {
                sys::clay_eval_points(
                    self.raw.as_ptr(),
                    name_ptr,
                    points.as_ptr() as *const f32,
                    points.len(),
                    distances.as_mut_ptr(),
                    std::ptr::null_mut(),
                )
            },
            "clay_eval_points",
        )?;
        Ok(distances)
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value and released exactly once,
        // because `Document` is neither `Copy` nor `Clone`.
        unsafe { sys::clay_document_destroy(self.raw.as_ptr()) };
    }
}

/// The 2D cross-section a lift primitive carries.
///
/// The parameters each one takes are its own, and are listed beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// `r`
    Circle = 0,
    /// `hx hy`
    Box = 1,
    /// `r`, the face radius
    Hexagon = 2,
    /// `r`
    Triangle = 3,
    /// `bottom top half_height`
    Trapezoid = 4,
    /// `r d`
    Vesica = 5,
}

/// How a curve point joins the one after it.
///
/// A stroke's points are hard corners by default — a straight segment to the
/// next — which is exactly what a chain authored before types existed means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointType {
    /// A straight segment to the next point.
    #[default]
    Hard = 0,
    /// Catmull-Rom, passing *through* the points.
    Spline = 1,
    /// A uniform cubic B-spline: approximating, so it rounds corners.
    BSpline = 2,
    /// A cubic shaped by the handles, which this wrapper does not carry.
    Bezier = 3,
}

/// A lattice cage placed in the world, over a whole SDF layer.
///
/// The counterpart to [`crate::MeshLattice`], and a different thing under the
/// hood: a mesh knows where its vertices are and can be deformed forward,
/// while a field is deformed by an *inverse* point map. The engine resolves
/// this into one lattice deformer per item, each carrying the transform that
/// takes that item's frame into the cage's — which is what makes it exact for
/// a rotated item, where no axis-aligned per-item box could reproduce a
/// world-placed cage.
///
/// Divisions are capped at **4** per axis, against the mesh lattice's 32,
/// because this is evaluated per sample rather than once per vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoCage {
    /// Where the cage sits in the world.
    pub position: [f32; 3],
    /// Rotation axis; the zero vector means no rotation.
    pub axis: [f32; 3],
    pub angle: f32,
    /// Uniform, and must be positive.
    pub scale: f32,
    /// The box the cage spans, in its own space.
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// Control points per axis, clamped to [2, 4] by the engine.
    pub divisions: [i32; 3],
}

impl Default for GizmoCage {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            axis: [0.0; 3],
            angle: 0.0,
            scale: 1.0,
            min: [-1.0; 3],
            max: [1.0; 3],
            divisions: [2; 3],
        }
    }
}

impl GizmoCage {
    /// How many control points the cage has, which is how many offsets it
    /// expects.
    pub fn point_count(self) -> usize {
        (self.divisions[0].max(0) * self.divisions[1].max(0) * self.divisions[2].max(0)) as usize
    }

    fn to_raw(self) -> sys::clay_gizmo_cage {
        sys::clay_gizmo_cage {
            struct_size: std::mem::size_of::<sys::clay_gizmo_cage>() as u32,
            position: self.position,
            axis: self.axis,
            angle: self.angle,
            scale: self.scale,
            box_min: self.min,
            box_max: self.max,
            nx: self.divisions[0],
            ny: self.divisions[1],
            nz: self.divisions[2],
        }
    }
}

/// The cage's offsets as the engine wants them: x fastest, or nothing at all.
///
/// `None` rather than a run of zeroes for an untouched cage, because the entry
/// point spells that as a null pointer and says it does nothing — handing it
/// zeroes would ask it to evaluate a deformer per item to move everything by
/// exactly zero.
fn flatten_offsets(cage: GizmoCage, offsets: &[[f32; 3]]) -> Result<Option<Vec<f32>>> {
    if offsets.is_empty() {
        return Ok(None);
    }
    let wanted = cage.point_count();
    if offsets.len() != wanted {
        return Err(crate::raw_failure(
            "clay_layer_lattice_gizmo",
            crate::ErrorKind::InvalidArgument,
        ));
    }
    Ok(Some(offsets.iter().flatten().copied().collect()))
}

impl Document {
    /// The raw handle, for sibling modules in this crate only.
    pub(crate) fn as_ptr(&self) -> *mut sys::clay_document {
        self.raw.as_ptr()
    }

    /// Meshes the whole document.
    ///
    /// This compiles a tape and marches the result, so it is the export path
    /// rather than the interactive one. For display, mesh the brick cache's
    /// dirty subset instead.
    pub fn mesh(&self, params: MeshParams) -> Result<Mesh> {
        let raw_params = params.to_raw();
        let mut mesh = std::ptr::null_mut();
        // SAFETY: the handle is valid, the descriptor carries its struct_size,
        // and `mesh` is written only on success.
        check(
            unsafe { sys::clay_document_mesh(self.as_ptr(), &raw_params, &mut mesh) },
            "clay_document_mesh",
        )?;
        Mesh::from_raw(mesh, "clay_document_mesh")
    }

    /// Meshes the field and appends every *visible* mesh layer under its own
    /// transform, indices rebased.
    ///
    /// The export path, as against [`Document::mesh`], which means "mesh the
    /// field" and keeps meaning exactly that. The engine's attribute rule
    /// applies: an attribute present on some inputs and absent on others is
    /// dropped from the result rather than padded, so the meshed field's
    /// normals are lost to a mesh layer that has none.
    pub fn mesh_combined(&self, params: MeshParams) -> Result<Mesh> {
        let raw_params = params.to_raw();
        let mut mesh = std::ptr::null_mut();
        // SAFETY: the handle is valid, the descriptor carries its struct_size,
        // and `mesh` is written only on success.
        check(
            unsafe { sys::clay_document_mesh_combined(self.as_ptr(), &raw_params, &mut mesh) },
            "clay_document_mesh_combined",
        )?;
        Mesh::from_raw(mesh, "clay_document_mesh_combined")
    }

    /// Attaches an already-loaded mesh as a layer, copying its geometry.
    ///
    /// A mesh layer is carried rather than evaluated: it is not compiled into
    /// a tape, takes no part in a blend and is not pickable. That is what a
    /// scan or a scale reference needs — geometry that leaves the pipeline as
    /// what it entered as.
    pub fn attach_mesh_layer(&mut self, mesh: &Mesh, desc: &MeshLayerDesc) -> Result<LayerId> {
        let name = crate::cstring(&desc.name, "clay_document_add_mesh_layer")?;
        let mut raw = sys::clay_mesh_layer_desc::sized();
        raw.name = name.as_ptr();
        raw.max_vertices = desc.max_vertices;
        raw.max_triangles = desc.max_triangles;
        raw.import_scale = desc.import_scale;

        let mut layer = 0;
        // The engine borrows the attached mesh back; we do not keep it, since
        // the document owns the copy from here on.
        let mut borrowed = std::ptr::null_mut();
        // SAFETY: every pointer is valid for the call, the name outlives it,
        // and both out-parameters are written only on success.
        check(
            unsafe {
                sys::clay_document_add_mesh_layer(
                    self.as_ptr(),
                    mesh.as_ptr(),
                    &raw,
                    &mut layer,
                    &mut borrowed,
                )
            },
            "clay_document_add_mesh_layer",
        )?;
        Ok(LayerId(layer))
    }
}

impl Document {
    /// Rasterizes this document's field into one of its own voxel layers.
    ///
    /// SDF to voxel, in one call, because the two halves cannot be held apart
    /// in Rust: `voxel_layer` hands back a grid carrying an exclusive borrow of
    /// the document, and rasterizing needs the document as well. At the C
    /// boundary there is no conflict — a grid and a document are distinct
    /// objects and `clay_voxel_rasterize` takes the document as `const` — so
    /// the two pointers meet here, in the crate where `unsafe` is allowed to
    /// live, rather than forcing a copy of the document on the caller.
    ///
    /// The layer must already exist and be a voxel layer.
    pub fn rasterize_into_voxel_layer(
        &mut self,
        layer_name: &str,
        region: ([f32; 3], [f32; 3]),
    ) -> Result<()> {
        let c_name = crate::cstring(layer_name, "clay_voxel_rasterize")?;
        let mut layer: sys::clay_layer_id = Default::default();
        let mut grid = std::ptr::null_mut();
        // SAFETY: a valid document handle and a NUL-terminated name; both
        // out-parameters are written only on success.
        check(
            unsafe {
                sys::clay_document_voxel_layer(
                    self.as_ptr(),
                    c_name.as_ptr(),
                    &mut layer,
                    &mut grid,
                )
            },
            "clay_document_voxel_layer",
        )?;
        let (min, max) = region;
        // SAFETY: `grid` was just written by a successful call and belongs to
        // this document; the document is passed as the `const` operand the
        // entry point declares, and the region is two arrays of three floats.
        check(
            unsafe {
                sys::clay_voxel_rasterize(
                    grid,
                    self.as_ptr() as *const _,
                    min.as_ptr(),
                    max.as_ptr(),
                )
            },
            "clay_voxel_rasterize",
        )
    }

    /// Rasterizes one of this document's mesh layers into one of its voxel
    /// layers — triangles straight into cells, in one sampling.
    ///
    /// Combined for the reason [`Document::rasterize_into_voxel_layer`] is:
    /// the mesh and the grid are both borrowed from this document, and Rust
    /// will not hold two such borrows at once even though the C boundary takes
    /// the mesh as `const`.
    pub fn rasterize_mesh_into_voxel_layer(
        &mut self,
        mesh_layer: &str,
        voxel_layer: &str,
        region: ([f32; 3], [f32; 3]),
    ) -> Result<()> {
        let c_mesh = crate::cstring(mesh_layer, "clay_document_mesh_layer")?;
        let c_voxel = crate::cstring(voxel_layer, "clay_document_voxel_layer")?;
        let mut layer: sys::clay_layer_id = Default::default();
        let mut mesh = std::ptr::null_mut();
        // SAFETY: a valid document and a NUL-terminated name; the outputs are
        // written only on success.
        check(
            unsafe {
                sys::clay_document_mesh_layer(self.as_ptr(), c_mesh.as_ptr(), &mut layer, &mut mesh)
            },
            "clay_document_mesh_layer",
        )?;
        let mut grid = std::ptr::null_mut();
        // SAFETY: as above, for the voxel side.
        check(
            unsafe {
                sys::clay_document_voxel_layer(
                    self.as_ptr(),
                    c_voxel.as_ptr(),
                    &mut layer,
                    &mut grid,
                )
            },
            "clay_document_voxel_layer",
        )?;
        let (min, max) = region;
        // SAFETY: both handles were written by successful calls and belong to
        // this document; the region is two arrays of three floats.
        check(
            unsafe { sys::clay_voxel_rasterize_mesh(grid, mesh, min.as_ptr(), max.as_ptr()) },
            "clay_voxel_rasterize_mesh",
        )
    }

    /// Converts one of this document's voxel layers into a new SDF layer.
    ///
    /// Combined for the same reason as the two above.
    pub fn voxel_layer_to_sdf_layer(
        &mut self,
        voxel_layer: &str,
        name: &str,
        blur: i32,
    ) -> Result<LayerId> {
        let c_voxel = crate::cstring(voxel_layer, "clay_document_voxel_layer")?;
        let c_name = crate::cstring(name, "clay_voxel_to_layer")?;
        let mut layer: sys::clay_layer_id = Default::default();
        let mut grid = std::ptr::null_mut();
        // SAFETY: a valid document and a NUL-terminated name.
        check(
            unsafe {
                sys::clay_document_voxel_layer(
                    self.as_ptr(),
                    c_voxel.as_ptr(),
                    &mut layer,
                    &mut grid,
                )
            },
            "clay_document_voxel_layer",
        )?;
        let mut made: sys::clay_layer_id = Default::default();
        // SAFETY: the grid belongs to this document and was just written; the
        // name is NUL-terminated and the output is written only on success.
        check(
            unsafe {
                sys::clay_voxel_to_layer(self.as_ptr(), grid, c_name.as_ptr(), blur, &mut made)
            },
            "clay_voxel_to_layer",
        )?;
        Ok(LayerId(made))
    }

    /// Copies a mesh layer's triangles out, for a viewport to draw.
    ///
    /// A mesh layer is in neither the tape nor the brick cache, so the surface
    /// the viewport builds from bricks cannot contain it — a sculpted mesh
    /// would move and show nothing. This is where its geometry comes from
    /// instead.
    ///
    /// Copied rather than borrowed, because the caller is going to upload it
    /// and because a layer's mesh must not be wrapped in an owning [`Mesh`]:
    /// that destroys what it holds on drop. The handle stays inside this call.
    #[allow(clippy::type_complexity)]
    pub fn read_mesh_layer(
        &mut self,
        layer_name: &str,
    ) -> Result<(Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>)> {
        let c_name = crate::cstring(layer_name, "clay_document_mesh_layer")?;
        let mut layer: sys::clay_layer_id = Default::default();
        let mut mesh = std::ptr::null_mut();
        // SAFETY: a valid document and a NUL-terminated name; both outputs are
        // written only on success.
        check(
            unsafe {
                sys::clay_document_mesh_layer(self.as_ptr(), c_name.as_ptr(), &mut layer, &mut mesh)
            },
            "clay_document_mesh_layer",
        )?;
        // SAFETY: the handle was just written by a successful call and is
        // borrowed for the length of this function. `ManuallyDrop` is what
        // keeps the borrow from being destroyed when the wrapper goes out of
        // scope — the layer owns it, not this.
        let borrowed =
            std::mem::ManuallyDrop::new(Mesh::from_raw(mesh, "clay_document_mesh_layer")?);
        let positions = borrowed.positions().to_vec();
        let count = positions.len();
        let normals = borrowed
            .normals()
            .map(<[[f32; 3]]>::to_vec)
            .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; count]);
        let colors = borrowed
            .colors()
            .map(<[[f32; 3]]>::to_vec)
            .unwrap_or_else(|| vec![[1.0; 3]; count]);
        let indices = borrowed.indices().to_vec();
        Ok((positions, normals, colors, indices))
    }

    /// Converts one of this document's mesh layers into a new SDF layer.
    ///
    /// Mesh to SDF: the triangles are resampled onto a lattice as a volume
    /// item. What comes back is an operand — boolean it, blend it, deform it —
    /// and what does not come back is the edge loops and the UVs, which is
    /// precisely what made the mesh worth importing. That is the whole reason
    /// this is a conversion offered rather than something done automatically.
    ///
    /// The mesh handle stays inside this call. A borrowed layer mesh must not
    /// be wrapped in [`Mesh`], which destroys what it holds on drop.
    pub fn mesh_layer_to_sdf_layer(
        &mut self,
        mesh_layer: &str,
        name: &str,
        params: crate::VolumeParams,
    ) -> Result<LayerId> {
        let c_mesh = crate::cstring(mesh_layer, "clay_document_mesh_layer")?;
        let mut layer: sys::clay_layer_id = Default::default();
        let mut mesh = std::ptr::null_mut();
        // SAFETY: a valid document and a NUL-terminated name; both outputs are
        // written only on success. The handle is borrowed and is never wrapped
        // in an owning `Mesh`, so nothing here will destroy the layer's mesh.
        check(
            unsafe {
                sys::clay_document_mesh_layer(self.as_ptr(), c_mesh.as_ptr(), &mut layer, &mut mesh)
            },
            "clay_document_mesh_layer",
        )?;
        // Built through the ordinary constructor rather than by calling the
        // entry point again here: `Item::volume_from_mesh` is where that call
        // and its safety argument live, and a second copy of both is a second
        // place to get it wrong. It needs an owning `Mesh`, which this handle
        // is not, so the raw call is made once through the shared descriptor.
        let raw = params.into_raw();
        let mut item = std::ptr::null_mut();
        // SAFETY: the mesh belongs to this document and was just written; the
        // descriptor carries its own size.
        check(
            unsafe { sys::clay_item_volume_from_mesh(mesh, &raw, &mut item) },
            "clay_item_volume_from_mesh",
        )?;
        let item = crate::Item::from_raw(item, "clay_item_volume_from_mesh")?;
        let made = self.add_sdf_layer(name)?;
        self.add_item(made, &item)?;
        Ok(made)
    }

    /// Converts a whole voxel grid into a new SDF layer, colour and all.
    ///
    /// One volume item per palette entry, which is what carries the colour: a
    /// distance field has none in it, so a single item could only come back
    /// grey. `blur` is as [`Item::volume_from_voxels`] describes it.
    ///
    /// A new layer rather than a replacement. The crossing discards the
    /// procedural history in one direction and is lossy in the other, so the
    /// source staying where it is *is* the way back — undo works until the
    /// session ends, and the layer works after it.
    /// Warps every item of a layer through a lattice cage, undoably.
    ///
    /// `offsets` is one displacement per control point in the cage's own
    /// space, x fastest — index `(i, j, k)` at `((k * ny + j) * nx + i)` — or
    /// empty for an untouched cage, which does nothing. Returns how many nodes
    /// were warped; the whole cage is one undo step.
    ///
    /// It reaches every item in the layer on purpose. A lattice's displacement
    /// outside its box is clamped rather than zero, so material out there
    /// travels rigidly with the nearest part of the cage — skipping distant
    /// items would tear the form.
    pub fn lattice_gizmo(
        &mut self,
        layer: LayerId,
        cage: GizmoCage,
        offsets: &[[f32; 3]],
    ) -> Result<usize> {
        let raw_cage = cage.to_raw();
        let flat = flatten_offsets(cage, offsets)?;
        let mut applied = 0;
        // SAFETY: valid handle, a descriptor carrying its struct_size, and
        // either null or a buffer of exactly nx*ny*nz*3 floats as the entry
        // point requires; `applied` is written on success.
        check(
            unsafe {
                sys::clay_layer_lattice_gizmo(
                    self.as_ptr(),
                    layer.0,
                    &raw_cage,
                    flat.as_ref().map_or(std::ptr::null(), |f| f.as_ptr()),
                    &mut applied,
                )
            },
            "clay_layer_lattice_gizmo",
        )?;
        Ok(applied)
    }

    /// How many nodes a cage *would* warp, without touching the document.
    ///
    /// Asked before applying, because a cage that reaches nothing and reports
    /// success is harder to notice than one that says so.
    pub fn lattice_gizmo_reach(
        &self,
        layer: LayerId,
        cage: GizmoCage,
        offsets: &[[f32; 3]],
    ) -> Result<usize> {
        let raw_cage = cage.to_raw();
        let flat = flatten_offsets(cage, offsets)?;
        let mut count = 0;
        // SAFETY: the size-query form — a null buffer with zero capacity asks
        // for the count only, which is what the entry point documents.
        check(
            unsafe {
                sys::clay_layer_lattice_gizmo_preview(
                    self.as_ptr(),
                    layer.0,
                    &raw_cage,
                    flat.as_ref().map_or(std::ptr::null(), |f| f.as_ptr()),
                    std::ptr::null_mut(),
                    0,
                    &mut count,
                )
            },
            "clay_layer_lattice_gizmo_preview",
        )?;
        Ok(count)
    }

    /// Replaces a placed stroke or curve's whole point list, undoably.
    ///
    /// A whole-list replace rather than granular edits: a curve is tens of
    /// points, so this costs less than the bookkeeping granular commands would
    /// need and its inverse is exact.
    ///
    /// This is what lets a gesture *grow* a curve rather than leave a trail of
    /// them. A tendril dragged across the viewport arrives in segments, and a
    /// segment that added its own item would bead the result — each one
    /// restarting the taper — where replacing the one item's points gives a
    /// single curve the length of the whole drag.
    pub fn set_layer_stroke_points(
        &mut self,
        layer: LayerId,
        node: NodeId,
        points_xyzr: &[f32],
        kind: PointType,
        tolerance: f32,
    ) -> Result<()> {
        if points_xyzr.len() % 4 != 0 {
            return Err(crate::raw_failure(
                "clay_layer_set_stroke_points",
                ErrorKind::InvalidArgument,
            ));
        }
        let count = points_xyzr.len() / 4;
        let types = vec![kind as i32; count];
        // SAFETY: a valid document, a layer and node the engine range-checks,
        // `count` quadruples and `count` types guaranteed by the length check,
        // and null for the two optional handle arrays.
        check(
            unsafe {
                sys::clay_layer_set_stroke_points(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    points_xyzr.as_ptr(),
                    count,
                    types.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    tolerance,
                )
            },
            "clay_layer_set_stroke_points",
        )
    }

    pub fn voxel_to_layer(
        &mut self,
        grid: &crate::VoxelGrid,
        name: &str,
        blur: i32,
    ) -> Result<LayerId> {
        let c_name = crate::cstring(name, "clay_voxel_to_layer")?;
        let mut layer: sys::clay_layer_id = Default::default();
        // SAFETY: valid handles, a NUL-terminated name, and an out-parameter
        // written only on success.
        check(
            unsafe {
                sys::clay_voxel_to_layer(
                    self.as_ptr(),
                    grid.as_ptr(),
                    c_name.as_ptr(),
                    blur,
                    &mut layer,
                )
            },
            "clay_voxel_to_layer",
        )?;
        Ok(LayerId(layer))
    }
}
