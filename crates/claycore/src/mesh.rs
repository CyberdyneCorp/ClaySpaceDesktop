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

impl Mesh {
    pub(crate) fn from_raw(raw: *mut sys::clay_mesh, operation: &'static str) -> Result<Self> {
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure(operation, ErrorKind::Backend))
    }

    /// Reads a mesh from a file. Format follows the extension.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let c_path = crate::cstring(path.as_ref().to_string_lossy().as_ref(), "clay_mesh_load")?;
        let budget = sys::clay_import_budget::sized();
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
        unsafe { slice_of(sys::clay_mesh_positions(self.raw.as_ptr()), self.vertex_count()) }
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

