//! Turning a screen gesture into something in the document.
//!
//! Picking is a read: every entry point here takes the document by shared
//! reference, and the engine's ghost/lock semantics apply — a ghosted layer is
//! shown but never picked.

use claycore_sys as sys;

use crate::error::{check, Result};
use crate::{Document, LayerId, NodeId};

/// Where a ray met the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// Distance along the ray.
    pub t: f32,
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// Which layer was hit, when the call attributes it.
    pub layer: Option<LayerId>,
    /// Which node was hit, when the call attributes it.
    pub node: Option<NodeId>,
}

/// A point snapped onto the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapped {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl Document {
    /// Casts a ray, without asking what was hit.
    pub fn raycast(&self, origin: [f32; 3], direction: [f32; 3]) -> Result<Option<Hit>> {
        let mut hit = 0i32;
        let mut t = 0.0f32;
        let (mut position, mut normal) = ([0.0f32; 3], [0.0f32; 3]);
        // SAFETY: three-float inputs; every out-parameter is valid for one
        // write of its type.
        check(
            unsafe {
                sys::clay_raycast(
                    self.as_ptr(),
                    origin.as_ptr(),
                    direction.as_ptr(),
                    &mut hit,
                    &mut t,
                    position.as_mut_ptr(),
                    normal.as_mut_ptr(),
                )
            },
            "clay_raycast",
        )?;
        Ok((hit != 0).then_some(Hit {
            t,
            position,
            normal,
            layer: None,
            node: None,
        }))
    }

    /// Casts a ray and reports which layer and node it met.
    ///
    /// This is what a click uses: selection needs to name what it selected.
    pub fn raycast_attributed(&self, origin: [f32; 3], direction: [f32; 3]) -> Result<Option<Hit>> {
        let mut hit = 0i32;
        let mut t = 0.0f32;
        let (mut position, mut normal) = ([0.0f32; 3], [0.0f32; 3]);
        let mut layer: sys::clay_layer_id = Default::default();
        let mut node: sys::clay_node_id = Default::default();
        // SAFETY: as `raycast`, with two further out-parameters the engine
        // writes only when it reports a hit.
        check(
            unsafe {
                sys::clay_raycast_attributed(
                    self.as_ptr(),
                    origin.as_ptr(),
                    direction.as_ptr(),
                    &mut hit,
                    &mut t,
                    position.as_mut_ptr(),
                    normal.as_mut_ptr(),
                    &mut layer,
                    &mut node,
                )
            },
            "clay_raycast_attributed",
        )?;
        Ok((hit != 0).then_some(Hit {
            t,
            position,
            normal,
            layer: Some(LayerId(layer)),
            node: Some(NodeId(node)),
        }))
    }

    /// Casts many rays at once.
    ///
    /// Free-threaded against one unchanged document, so several batches may
    /// run concurrently.
    pub fn raycast_many(&self, rays: &[([f32; 3], [f32; 3])]) -> Result<Vec<Option<Hit>>> {
        if rays.is_empty() {
            return Ok(Vec::new());
        }
        // The engine wants origin and direction interleaved, six floats a ray.
        let mut packed = Vec::with_capacity(rays.len() * 6);
        for (origin, direction) in rays {
            packed.extend_from_slice(origin);
            packed.extend_from_slice(direction);
        }

        let mut hits = vec![0i32; rays.len()];
        let mut t = vec![0.0f32; rays.len()];
        let mut positions = vec![[0.0f32; 3]; rays.len()];
        let mut normals = vec![[0.0f32; 3]; rays.len()];

        // SAFETY: `packed` is `rays.len() * 6` floats; each output is sized to
        // the ray count, with position and normal three floats each.
        check(
            unsafe {
                sys::clay_raycast_many(
                    self.as_ptr(),
                    packed.as_ptr(),
                    rays.len(),
                    hits.as_mut_ptr(),
                    t.as_mut_ptr(),
                    positions.as_mut_ptr() as *mut f32,
                    normals.as_mut_ptr() as *mut f32,
                )
            },
            "clay_raycast_many",
        )?;

        Ok((0..rays.len())
            .map(|i| {
                (hits[i] != 0).then_some(Hit {
                    t: t[i],
                    position: positions[i],
                    normal: normals[i],
                    layer: None,
                    node: None,
                })
            })
            .collect())
    }

    /// Moves points onto the nearest surface.
    ///
    /// A point the engine could not snap comes back as `None` rather than as
    /// an arbitrary position.
    pub fn snap_to_surface(&self, points: &[[f32; 3]]) -> Result<Vec<Option<Snapped>>> {
        if points.is_empty() {
            return Ok(Vec::new());
        }
        let mut positions = vec![[0.0f32; 3]; points.len()];
        let mut normals = vec![[0.0f32; 3]; points.len()];
        let mut ok = vec![0i32; points.len()];

        // SAFETY: input and both outputs are `points.len() * 3` floats, and
        // `ok` is one flag per point.
        check(
            unsafe {
                sys::clay_snap_to_surface(
                    self.as_ptr(),
                    points.as_ptr() as *const f32,
                    points.len(),
                    positions.as_mut_ptr() as *mut f32,
                    normals.as_mut_ptr() as *mut f32,
                    ok.as_mut_ptr(),
                )
            },
            "clay_snap_to_surface",
        )?;

        Ok((0..points.len())
            .map(|i| {
                (ok[i] != 0).then_some(Snapped {
                    position: positions[i],
                    normal: normals[i],
                })
            })
            .collect())
    }
}
