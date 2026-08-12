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
}

/// A node placed in a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) sys::clay_node_id);

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
