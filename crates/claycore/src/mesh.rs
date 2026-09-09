//! Meshes produced by the engine, and getting their vertices to a GPU.
//!
//! A [`Mesh`] owns its engine handle. Its attribute arrays are borrowed from
//! that handle and are lifetime-bound to it, so they cannot outlive the mesh
//! they point into. For upload, prefer [`Mesh::copy_vertices`]: it writes the
//! caller's interleaved layout in one pass, where reading the arrays and
//! interleaving them by hand is two passes over the same geometry.

use std::path::Path;
use std::ptr::NonNull;

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::error::{check, ErrorKind, Result};
use crate::raw_failure;

/// Which mesher produced, or should produce, a mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mesher {
    /// Watertight and 2-manifold by construction. The export default.
    #[default]
    MarchingTetrahedra,
    /// Faster, for interactive display.
    SurfaceNets,
    /// Opt-in and flagged experimental by the engine.
    DualContouring,
}

impl Mesher {
    fn raw(self) -> i32 {
        (match self {
            Self::MarchingTetrahedra => sys::clay_mesher::CLAY_MESHER_MARCHING,
            Self::SurfaceNets => sys::clay_mesher::CLAY_MESHER_NETS,
            Self::DualContouring => sys::clay_mesher::CLAY_MESHER_DUAL_CONTOURING,
        }) as i32
    }

    fn is_experimental(self) -> bool {
        matches!(self, Self::DualContouring)
    }
}

/// How to mesh a document.
#[derive(Debug, Clone, Copy)]
pub struct MeshParams {
    /// World units per cell. When `None`, `resolution` decides.
    pub voxel_size: Option<f32>,
    /// Cells across the largest extent, used when `voxel_size` is `None`.
    pub resolution: i32,
    /// Target triangle ratio; `None` leaves the mesh undecimated.
    pub decimate_ratio: Option<f32>,
    pub mesher: Mesher,
}

impl Default for MeshParams {
    fn default() -> Self {
        Self {
            voxel_size: None,
            resolution: 128,
            decimate_ratio: None,
            mesher: Mesher::default(),
        }
    }
}

impl MeshParams {
    pub(crate) fn to_raw(self) -> sys::clay_mesh_params {
        let mut raw = sys::clay_mesh_params::sized();
        raw.voxel_size = self.voxel_size.unwrap_or(0.0);
        raw.resolution = self.resolution;
        raw.decimate = i32::from(self.decimate_ratio.is_some());
        raw.decimate_ratio = self.decimate_ratio.unwrap_or(1.0);
        raw.mesher = self.mesher.raw();
        raw.experimental = i32::from(self.mesher.is_experimental());
        raw
    }
}

/// Where each attribute sits in the caller's vertex struct.
///
/// Offsets are byte offsets from the start of a vertex; `None` omits the
/// attribute. The engine refuses a layout naming an attribute the mesh does
/// not carry, overlapping attributes, or a stride that does not clear them —
/// each of which would produce a buffer that is wrong without looking wrong.
#[derive(Debug, Clone, Copy)]
pub struct VertexLayout {
    /// Bytes per vertex. `None` means tightly packed.
    pub stride: Option<u32>,
    pub position_offset: Option<i32>,
    pub normal_offset: Option<i32>,
    pub color_offset: Option<i32>,
    pub uv_offset: Option<i32>,
}

impl VertexLayout {
    fn to_raw(self) -> sys::clay_vertex_layout {
        // The engine reads a negative offset as "this attribute is absent".
        fn offset(value: Option<i32>) -> i32 {
            value.unwrap_or(-1)
        }
        let mut raw = sys::clay_vertex_layout::sized();
        raw.stride = self.stride.unwrap_or(0);
        raw.position_offset = offset(self.position_offset);
        raw.normal_offset = offset(self.normal_offset);
        raw.color_offset = offset(self.color_offset);
        raw.uv_offset = offset(self.uv_offset);
        raw
    }
}

/// A triangle mesh owned by the caller.
pub struct Mesh {
    raw: NonNull<sys::clay_mesh>,
}

// SAFETY: a mesh is an immutable owned buffer once produced; the engine
// mutates it through no other handle.
unsafe impl Send for Mesh {}

/// What an importer is allowed to decode.
///
/// Checked against the file's *declared* counts before anything is allocated,
/// which is the point: a malformed or hostile file can claim a billion
/// triangles, and the check has to happen before the allocation rather than
/// after it. Zero means the library's own default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportBudget {
    pub max_vertices: u64,
    pub max_triangles: u64,
}

impl ImportBudget {
    fn to_raw(self) -> sys::clay_import_budget {
        let mut raw = sys::clay_import_budget::sized();
        raw.max_vertices = self.max_vertices;
        raw.max_triangles = self.max_triangles;
        raw
    }
}

/// How a mesh is attached to a document as a layer.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshLayerDesc {
    pub name: String,
    /// This layer's own ceiling, which is a different question from what a
    /// file may decode into. Zero means the library's default.
    pub max_vertices: u64,
    pub max_triangles: u64,
    /// Uniform scale baked into the stored vertices, so a unit conversion is
    /// resolved once at import rather than approximated by a layer transform.
    /// Non-uniform scale is not expressible and is not approximated.
    pub import_scale: f32,
}

impl MeshLayerDesc {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            max_vertices: 0,
            max_triangles: 0,
            import_scale: 1.0,
        }
    }
}

impl Mesh {
    pub(crate) fn from_raw(raw: *mut sys::clay_mesh, operation: &'static str) -> Result<Self> {
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure(operation, ErrorKind::Backend))
    }

    /// The raw handle, for calls that take a mesh.
    pub(crate) fn as_ptr(&self) -> *mut sys::clay_mesh {
        self.raw.as_ptr()
    }

    /// Builds a mesh from positions and a triangulation.
    ///
    /// The way a surface produced outside this library — a retopologiser's
    /// output, most of all — becomes something a document can hold.
    ///
    /// An index past the end is checked here rather than left to the engine,
    /// because that is the one malformed input which reads memory instead of
    /// returning a status.
    pub fn from_triangles(positions: &[[f32; 3]], indices: &[u32]) -> Result<Self> {
        if indices.len() % 3 != 0 {
            return Err(raw_failure(
                "clay_mesh_from_triangles",
                ErrorKind::InvalidArgument,
            ));
        }
        if indices.iter().any(|&i| i as usize >= positions.len()) {
            return Err(raw_failure(
                "clay_mesh_from_triangles",
                ErrorKind::InvalidArgument,
            ));
        }
        let mut raw = std::ptr::null_mut();
        // SAFETY: `positions` is `vertex_count` triples and `indices` is
        // `index_count` u32s, each count taken from the slice it describes;
        // every index has been checked to name a vertex; the out-parameter is
        // written only on success.
        check(
            unsafe {
                sys::clay_mesh_from_triangles(
                    positions.as_ptr() as *const f32,
                    positions.len(),
                    indices.as_ptr(),
                    indices.len(),
                    &mut raw,
                )
            },
            "clay_mesh_from_triangles",
        )?;
        Self::from_raw(raw, "clay_mesh_from_triangles")
    }

    /// Reads a mesh from a file. Format follows the extension.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Self::load_within(path, ImportBudget::default())
    }

    /// Reads a mesh, refusing one that declares more than `budget` allows.
    pub fn load_within(path: impl AsRef<Path>, budget: ImportBudget) -> Result<Self> {
        let c_path = crate::cstring(path.as_ref().to_string_lossy().as_ref(), "clay_mesh_load")?;
        let budget = budget.to_raw();
        let mut raw = std::ptr::null_mut();
        // SAFETY: path is NUL-terminated and outlives the call; budget is a
        // versioned descriptor with struct_size set; raw is a valid
        // out-parameter written only on success.
        check(
            unsafe { sys::clay_mesh_load(c_path.as_ptr(), &budget, &mut raw) },
            "clay_mesh_load",
        )?;
        Self::from_raw(raw, "clay_mesh_load")
    }

    /// Writes the mesh to a file. Format follows the extension.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let c_path = crate::cstring(path.as_ref().to_string_lossy().as_ref(), "clay_mesh_save")?;
        // SAFETY: both handles are valid and the path outlives the call.
        check(
            unsafe { sys::clay_mesh_save(self.raw.as_ptr(), c_path.as_ptr()) },
            "clay_mesh_save",
        )
    }

    /// Writes the **sculpt handoff** profile: the file a retopology engine
    /// reads to take this sculpt into `retopo -> UV -> bake`.
    ///
    /// **Not [`Self::save`], and the difference is not cosmetic.** `save`
    /// declares a mesh's *quads* as its faces where it has them, and
    /// CyberRemesher's reader refuses any arity but triangles — one line,
    /// `if (face.size() != 3)`, at `handoff.cpp:403`. So the best export this
    /// library can produce is precisely the file that pipeline rejects. This
    /// call is the one that does not: the engine guarantees triangles, and
    /// computes normals where the mesh has none because their reader requires
    /// them. Neither guarantee is something a caller could be expected to know.
    ///
    /// The format is **theirs**, not ours — `docs/sculpt-handoff-format.md`
    /// version 1.0, which the engine's own header defers to: "where their spec
    /// and this header disagree, their spec is right."
    ///
    /// `producer` labels who made it; `None` takes the engine's default.
    /// `material_mask` becomes the per-vertex `material_mix` channel — their
    /// spec's blend weight between two material slots, which ClayCore does not
    /// have and does not invent, so a painted mask supplies the scalar. `None`
    /// writes zeros, the honest answer for a document that never expressed one.
    pub fn save_handoff(
        &self,
        path: impl AsRef<Path>,
        producer: Option<&str>,
        material_mask: Option<&crate::Mask>,
        binary: bool,
    ) -> Result<()> {
        let c_path = crate::cstring(
            path.as_ref().to_string_lossy().as_ref(),
            "clay_mesh_save_handoff",
        )?;
        let label = producer
            .map(|name| crate::cstring(name, "clay_mesh_save_handoff"))
            .transpose()?;
        // SAFETY: the mesh and the optional mask are valid handles; both C
        // strings outlive the call; a null producer and a null mask are the
        // documented ways to ask for the default label and for zeros.
        check(
            unsafe {
                sys::clay_mesh_save_handoff(
                    self.raw.as_ptr(),
                    c_path.as_ptr(),
                    label.as_ref().map_or(std::ptr::null(), |l| l.as_ptr()),
                    material_mask.map_or(std::ptr::null(), |m| m.as_ptr()),
                    i32::from(binary),
                )
            },
            "clay_mesh_save_handoff",
        )
    }

    /// The same bytes, in memory, for the pipe route their CLI documents:
    ///
    /// ```sh
    /// producer --for-retopo | cyberremesh --target - --output low.obj
    /// ```
    ///
    /// An in-process producer needs no temporary file and gets the same version
    /// gate either way.
    pub fn handoff_bytes(
        &self,
        producer: Option<&str>,
        material_mask: Option<&crate::Mask>,
        binary: bool,
    ) -> Result<Vec<u8>> {
        let label = producer
            .map(|name| crate::cstring(name, "clay_mesh_save_handoff_memory"))
            .transpose()?;
        let mut blob = std::ptr::null_mut();
        // SAFETY: as `save_handoff`, with an owned blob written only on
        // success.
        check(
            unsafe {
                sys::clay_mesh_save_handoff_memory(
                    self.raw.as_ptr(),
                    label.as_ref().map_or(std::ptr::null(), |l| l.as_ptr()),
                    material_mask.map_or(std::ptr::null(), |m| m.as_ptr()),
                    i32::from(binary),
                    &mut blob,
                )
            },
            "clay_mesh_save_handoff_memory",
        )?;
        let blob = NonNull::new(blob).ok_or_else(|| {
            raw_failure("clay_mesh_save_handoff_memory", ErrorKind::InvalidArgument)
        })?;
        // Copied out and the blob released here rather than handed up, so the
        // caller owns a `Vec` and there is no engine handle to leak on an
        // early return above.
        //
        // SAFETY: an owned blob; data and size describe the same allocation,
        // and the copy happens before the destroy.
        let bytes = unsafe {
            let data = sys::clay_blob_data(blob.as_ptr());
            let size = sys::clay_blob_size(blob.as_ptr());
            let bytes = if data.is_null() || size == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(data, size).to_vec()
            };
            sys::clay_blob_destroy(blob.as_ptr());
            bytes
        };
        Ok(bytes)
    }

    /// The one array of the in-memory buffer profile that cannot be borrowed
    /// from what is already exposed.
    ///
    /// Their `BufferView` wants positions, normals, colours, material mix and
    /// indices. Four of the five are already borrowed pointers on this type, so
    /// a struct here would duplicate one they own and give the two engines two
    /// places to disagree about the layout. This produces the fifth.
    ///
    /// A `None` mask fills zeros.
    pub fn handoff_material_mix(&self, material_mask: Option<&crate::Mask>) -> Result<Vec<f32>> {
        let capacity = self.vertex_count();
        let mut values = vec![0.0f32; capacity];
        let mut count = 0usize;
        // SAFETY: `values` has `capacity` floats and the capacity is passed
        // beside it; `count` is written on success.
        check(
            unsafe {
                sys::clay_mesh_handoff_material_mix(
                    self.raw.as_ptr(),
                    material_mask.map_or(std::ptr::null(), |m| m.as_ptr()),
                    values.as_mut_ptr(),
                    capacity,
                    &mut count,
                )
            },
            "clay_mesh_handoff_material_mix",
        )?;
        values.truncate(count);
        Ok(values)
    }

    pub fn vertex_count(&self) -> usize {
        // SAFETY: the handle is valid; the call only reads.
        unsafe { sys::clay_mesh_vertex_count(self.raw.as_ptr()) }
    }

    pub fn index_count(&self) -> usize {
        // SAFETY: the handle is valid; the call only reads.
        unsafe { sys::clay_mesh_index_count(self.raw.as_ptr()) }
    }

    pub fn is_empty(&self) -> bool {
        self.index_count() == 0
    }

    /// Vertex positions as `[x, y, z]` triples, borrowed from the mesh.
    pub fn positions(&self) -> &[[f32; 3]] {
        // SAFETY: the engine returns a pointer to `vertex_count * 3` floats
        // owned by this mesh and valid until it is destroyed. `[f32; 3]` has
        // the same layout as three consecutive floats.
        unsafe {
            slice_of(
                sys::clay_mesh_positions(self.raw.as_ptr()),
                self.vertex_count(),
            )
        }
    }

    /// Vertex normals, when the mesh carries them.
    pub fn normals(&self) -> Option<&[[f32; 3]]> {
        // SAFETY: as `positions`, and the engine documents NULL for absent.
        let ptr = unsafe { sys::clay_mesh_normals(self.raw.as_ptr()) };
        (!ptr.is_null()).then(|| unsafe { slice_of(ptr, self.vertex_count()) })
    }

    /// Vertex colours, when the mesh carries them.
    pub fn colors(&self) -> Option<&[[f32; 3]]> {
        // SAFETY: as `positions`, and the engine documents NULL for absent.
        let ptr = unsafe { sys::clay_mesh_colors(self.raw.as_ptr()) };
        (!ptr.is_null()).then(|| unsafe { slice_of(ptr, self.vertex_count()) })
    }

    /// Triangle indices, borrowed from the mesh.
    pub fn indices(&self) -> &[u32] {
        // SAFETY: the engine returns `index_count` u32 owned by this mesh.
        let ptr = unsafe { sys::clay_mesh_indices(self.raw.as_ptr()) };
        if ptr.is_null() {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(ptr, self.index_count()) }
    }

    /// The mesh's axis-aligned bounds.
    pub fn bounds(&self) -> Result<([f32; 3], [f32; 3])> {
        let (mut min, mut max) = ([0.0f32; 3], [0.0f32; 3]);
        // SAFETY: both out-parameters are three floats as required.
        check(
            unsafe { sys::clay_mesh_bounds(self.raw.as_ptr(), min.as_mut_ptr(), max.as_mut_ptr()) },
            "clay_mesh_bounds",
        )?;
        Ok((min, max))
    }

    /// A copy of this mesh, moved.
    ///
    /// The engine rotates normals but does not translate or scale them —
    /// "a uniform scale leaves a direction unchanged, and adding the position
    /// would turn a direction into a point" — and rewrites no index, so the
    /// quad list and the quad report still describe the result. It is the same
    /// mesh, moved.
    ///
    /// `scale` is uniform and must be greater than zero, as every transform in
    /// this interface is, and the axis must be non-zero even for no rotation:
    /// "a second convention for 'no rotation' would be one more thing to get
    /// wrong."
    pub fn transformed(
        &self,
        position: [f32; 3],
        rotation_axis: [f32; 3],
        rotation_angle: f32,
        scale: f32,
    ) -> Result<Self> {
        let mut out = std::ptr::null_mut();
        // SAFETY: a valid mesh, two three-float inputs and an out-parameter
        // the engine fills with a mesh this takes ownership of below.
        check(
            unsafe {
                sys::clay_mesh_transform(
                    self.raw.as_ptr(),
                    position.as_ptr(),
                    rotation_axis.as_ptr(),
                    rotation_angle,
                    scale,
                    &mut out,
                )
            },
            "clay_mesh_transform",
        )?;
        NonNull::new(out)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_mesh_transform", ErrorKind::InvalidArgument))
    }

    /// Whether the mesh is watertight and 2-manifold.
    pub fn validate(&self) -> Result<MeshValidity> {
        let (mut watertight, mut manifold) = (0i32, 0i32);
        // SAFETY: two valid i32 out-parameters.
        check(
            unsafe { sys::clay_mesh_validate(self.raw.as_ptr(), &mut watertight, &mut manifold) },
            "clay_mesh_validate",
        )?;
        Ok(MeshValidity {
            watertight: watertight != 0,
            manifold: manifold != 0,
        })
    }

    /// Writes vertices into caller memory in the caller's layout, one pass.
    ///
    /// `dst` is typically a mapped GPU buffer. The engine validates the layout
    /// against what the mesh actually carries and refuses a mismatch rather
    /// than writing a partially correct buffer.
    pub fn copy_vertices(&self, layout: VertexLayout, dst: &mut [u8]) -> Result<()> {
        let raw = layout.to_raw();
        // SAFETY: `dst` is valid for writes of `dst.len()` bytes, which is
        // what the engine is told it has; `raw` carries its own struct_size.
        check(
            unsafe {
                sys::clay_mesh_copy_vertices(
                    self.raw.as_ptr(),
                    &raw,
                    dst.as_mut_ptr() as *mut std::ffi::c_void,
                    dst.len(),
                )
            },
            "clay_mesh_copy_vertices",
        )
    }

    /// Writes indices into caller memory.
    pub fn copy_indices(&self, dst: &mut [u32]) -> Result<()> {
        // SAFETY: `dst` is valid for writes of `dst.len()` u32.
        check(
            unsafe { sys::clay_mesh_copy_indices(self.raw.as_ptr(), dst.as_mut_ptr(), dst.len()) },
            "clay_mesh_copy_indices",
        )
    }
}

/// What [`Mesh::validate`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshValidity {
    pub watertight: bool,
    pub manifold: bool,
}

impl Drop for Mesh {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once.
        unsafe { sys::clay_mesh_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for Mesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mesh")
            .field("vertices", &self.vertex_count())
            .field("indices", &self.index_count())
            .finish()
    }
}

/// # Safety
///
/// `ptr` must be null or point to `count * 3` floats valid for the returned
/// lifetime.
unsafe fn slice_of<'a>(ptr: *const f32, count: usize) -> &'a [[f32; 3]] {
    if ptr.is_null() || count == 0 {
        return &[];
    }
    std::slice::from_raw_parts(ptr as *const [f32; 3], count)
}
