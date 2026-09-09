//! A mesh the retopology engine owns, and the bridge that fills one.
//!
//! **The bridge is the handoff buffer profile, not a file and not a setter.**
//! The engine has `cyber_mesh_set_positions` and no index setter at all, so an
//! empty mesh cannot be populated field by field. `cyber_handoff_open_buffers`
//! is the in-process path both engines were designed around: positions,
//! normals, colours, a material-mix channel and indices, version-gated the same
//! way the file profile is, with no temporary file between two libraries that
//! are already in one address space.

use std::ptr::NonNull;

use cyberremesh_sys as sys;

use crate::error::{check, Error, Result};

/// A mesh owned by the retopology engine.
///
/// **Valid for the operation it was made for.** The engine's element-id
/// stability contract is that most retopology operations reassign vertex and
/// face ids, and subdivision reassigns all of them, with nothing to announce
/// it. So this is deliberately not a long-lived handle: build one, use it,
/// drop it. Anything a caller wants to keep is read out into owned Rust data
/// first.
pub struct Mesh {
    raw: NonNull<sys::CyberMesh>,
}

// SAFETY: the engine reaches this allocation only through this handle, and the
// handle is moved rather than shared.
unsafe impl Send for Mesh {}

impl Mesh {
    /// Takes a sculpt across through the handoff buffer profile.
    ///
    /// `normals` may be empty — the buffer profile accepts a mesh without
    /// them, unlike the PLY profile which requires them — but a bake is better
    /// with them, so pass them where they exist.
    ///
    /// The version travels from ClayCore's own handoff constants rather than
    /// being restated: a version written twice is one that can disagree with
    /// itself, and the engine refuses one it does not support naming both
    /// rather than reading it with the unknown parts dropped.
    pub fn from_handoff(
        positions: &[f32],
        normals: &[f32],
        indices: &[u32],
        version: (u32, u32),
        producer: &str,
    ) -> Result<Self> {
        if positions.len() % 3 != 0 {
            return Err(Error::misuse(
                "cyber_handoff_open_buffers",
                "positions are not a whole number of points",
            ));
        }
        if indices.len() % 3 != 0 {
            return Err(Error::misuse(
                "cyber_handoff_open_buffers",
                "indices are not a whole number of triangles",
            ));
        }
        let vertex_count = positions.len() / 3;
        if !normals.is_empty() && normals.len() != positions.len() {
            return Err(Error::misuse(
                "cyber_handoff_open_buffers",
                "there are normals but not one per vertex",
            ));
        }
        // Checked here rather than left to the engine, because an index past
        // the end is the one malformed input that reads memory rather than
        // returning a status.
        if let Some(&past) = indices.iter().find(|&&i| i as usize >= vertex_count) {
            return Err(Error::misuse(
                "cyber_handoff_open_buffers",
                &format!("index {past} names no vertex of {vertex_count}"),
            ));
        }

        let label = std::ffi::CString::new(producer).map_err(|_| {
            Error::misuse(
                "cyber_handoff_open_buffers",
                "the producer label contains a NUL",
            )
        })?;
        let buffers = sys::CyberHandoffBuffers {
            positions: positions.as_ptr(),
            normals: if normals.is_empty() {
                std::ptr::null()
            } else {
                normals.as_ptr()
            },
            colors: std::ptr::null(),
            material_mix: std::ptr::null(),
            vertex_count,
            indices: indices.as_ptr(),
            index_count: indices.len(),
            version_major: version.0 as _,
            version_minor: version.1 as _,
            producer: label.as_ptr(),
        };
        let mut out = std::ptr::null_mut();
        let mut info = sys::CyberHandoffInfo::default();
        check(
            // SAFETY: every pointer in `buffers` is either null or borrowed from a
            // slice that outlives this call, and each count is that slice's own
            // length. The out-parameters are valid and written only on success.
            unsafe { sys::cyber_handoff_open_buffers(&buffers, &mut out, &mut info) },
            "cyber_handoff_open_buffers",
        )?;
        Self::from_raw(out, "cyber_handoff_open_buffers")
    }

    pub(crate) fn from_raw(raw: *mut sys::CyberMesh, operation: &'static str) -> Result<Self> {
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| Error::misuse(operation, "the engine returned no mesh"))
    }

    pub(crate) fn as_ptr(&self) -> *mut sys::CyberMesh {
        self.raw.as_ptr()
    }

    pub fn vertex_count(&self) -> usize {
        // SAFETY: a valid handle; the call only reads.
        unsafe { sys::cyber_mesh_vertex_count(self.raw.as_ptr()) }
    }

    pub fn triangle_count(&self) -> usize {
        // SAFETY: a valid handle; the call only reads.
        unsafe { sys::cyber_mesh_triangle_count(self.raw.as_ptr()) }
    }

    /// Faces as the engine counts them: quads where it made quads.
    ///
    /// Different from [`Self::triangle_count`] on retopologised output and the
    /// same on a triangle mesh, which is the whole point of asking.
    pub fn face_count(&self) -> usize {
        // SAFETY: a valid handle; the call only reads.
        unsafe { sys::cyber_mesh_face_count(self.raw.as_ptr()) }
    }

    /// The positions, copied out.
    pub fn positions(&self) -> Vec<[f32; 3]> {
        let mut floats = vec![0.0f32; self.vertex_count() * 3];
        // SAFETY: the destination holds exactly the float count passed, and
        // the engine writes no more than it is told it may.
        let written = unsafe {
            sys::cyber_mesh_copy_positions(self.raw.as_ptr(), floats.as_mut_ptr(), floats.len())
        };
        floats.truncate(written);
        floats.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect()
    }

    /// The triangulation, copied out.
    ///
    /// Quads are carried *beside* the triangles rather than instead of them, so
    /// this is the same buffer a renderer has always drawn — a host that
    /// ignores the quads draws the mesh it always drew.
    pub fn triangle_indices(&self) -> Vec<u32> {
        let mut indices = vec![0u32; self.triangle_count() * 3];
        // SAFETY: as `positions`.
        let written = unsafe {
            sys::cyber_mesh_copy_triangle_indices(
                self.raw.as_ptr(),
                indices.as_mut_ptr(),
                indices.len(),
            )
        };
        indices.truncate(written);
        indices
    }
}

impl Drop for Mesh {
    fn drop(&mut self) {
        // SAFETY: an owned handle, released exactly once. Nothing borrows it:
        // every reader above copies out rather than holding a pointer in.
        unsafe { sys::cyber_mesh_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for Mesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mesh")
            .field("vertices", &self.vertex_count())
            .field("faces", &self.face_count())
            .field("triangles", &self.triangle_count())
            .finish()
    }
}
