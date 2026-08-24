//! How a voxel layer is drawn.
//!
//! A grid is boxes. Whether it should *look* like boxes is a separate
//! question, and the engine answers it plainly: the boxy picture is "correct
//! for hard-surface voxel work and for export, and the wrong picture of an
//! organic sculpt". It offers a mesher for each and says the choice is an
//! argument rather than grid state, "so two hosts sharing a document cannot
//! disagree about what it looks like and one host can show both pictures of
//! one sculpt without mutating it".
//!
//! So this is a *display* setting. Nothing here changes a cell.

/// Which picture of a grid the viewport draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoxelDisplay {
    /// One quad per exposed voxel face — the boxes the model actually is.
    ///
    /// The correct picture for hard-surface work, and what exports. Not the
    /// default: a sculptor works on a form, and the cells it is stored in are
    /// a fact about the storage. Kept because seeing the cells is sometimes
    /// exactly what is wanted, and because it is the only picture the engine
    /// can mesh a chunk at a time — see [`VoxelDisplay::is_incremental`].
    Boxes,
    /// Surface nets over the occupancy: one vertex per surface cell, at the
    /// centroid of that cell's edge crossings.
    ///
    /// The centroid is what smooths, so a corner rounds without anything being
    /// filtered first — and nothing vanishes, because a lone voxel has a sign
    /// change on each of its six edges and still gets a surface.
    ///
    /// Not dual contouring, and the difference is worth knowing: dual
    /// contouring fits a vertex by least squares to the hermite data and so
    /// keeps a sharp corner sharp. This takes the centroid, so corners round.
    /// Preserving them would be a change to the engine rather than to us.
    ///
    /// The default. A sculptor is shaping a form, not a lattice, and every
    /// tool this one sits beside shows the form — so showing the cells by
    /// default would make a grid the odd one out for a reason that belongs to
    /// how it is stored.
    #[default]
    Smooth,
}

impl VoxelDisplay {
    pub const ALL: [VoxelDisplay; 2] = [Self::Boxes, Self::Smooth];

    pub fn label(self) -> &'static str {
        match self {
            Self::Boxes => "Voxels",
            Self::Smooth => "Suave",
        }
    }

    /// Whether the engine can mesh this picture a chunk at a time.
    ///
    /// Only the boxes. `clay_voxel_mesh_chunks` is the greedy mesher alone,
    /// and the engine says why: greedy quads are axis-aligned and exact, so
    /// clamping the merge to a chunk boundary emits more, smaller quads over
    /// the identical surface and never a crack. Surface nets place a vertex
    /// from a cell's *neighbourhood*, so a chunk boundary would tear.
    ///
    /// That is the whole reason the smooth picture is drawn when a gesture
    /// settles rather than while it is made: whole-grid meshing measured at
    /// 309 ms a dab against 3.3 ms for the incremental path.
    pub fn is_incremental(self) -> bool {
        self == Self::Boxes
    }
}

/// How much the occupancy is filtered before the smooth surface is taken.
///
/// In passes of a 3×3×3 box, and the trade is real in both directions. At 0
/// nothing is filtered and nothing can be lost, but the surface still
/// *terraces*: every crossing over binary occupancy interpolates to the same
/// midpoint, so corners round and steps remain. At 1 it reads as clay, and an
/// isolated voxel sits near 0.3 occupancy — under the isolevel, and gone. Thin
/// features go the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmoothBlur(i32);

impl Default for SmoothBlur {
    /// Zero, and the engine's own reasoning for it: "a default that silently
    /// deletes a sculptor's detail is the wrong default however good it
    /// looks."
    fn default() -> Self {
        Self(0)
    }
}

impl SmoothBlur {
    /// The most the engine's own note describes. Past one pass the box filter
    /// is eating features rather than rounding them, and there is no reading
    /// of "smoother" that a sculptor wants which costs a limb.
    pub const MOST: i32 = 3;

    pub fn new(passes: i32) -> Self {
        Self(passes.clamp(0, Self::MOST))
    }

    pub fn passes(self) -> i32 {
        self.0
    }

    /// Whether this setting can delete detail.
    ///
    /// Worth asking, because the interface should say so where it is true
    /// rather than leaving a sculptor to find out from a missing finger.
    pub fn can_lose_detail(self) -> bool {
        self.0 > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_shows_its_form_by_default_and_loses_nothing_doing_it() {
        // The smooth surface, and no filtering. A sculptor is shaping a form,
        // not a lattice, and the cells are a fact about the storage — but the
        // *blur* stays at zero, because a default that silently deletes a
        // sculptor's detail is the wrong default however good it looks.
        assert_eq!(VoxelDisplay::default(), VoxelDisplay::Smooth);
        assert_eq!(SmoothBlur::default().passes(), 0);
        assert!(!SmoothBlur::default().can_lose_detail());
    }

    #[test]
    fn only_the_boxes_mesh_a_chunk_at_a_time() {
        // `clay_voxel_mesh_chunks` is the greedy mesher alone. Surface nets
        // place a vertex from a cell's neighbourhood, so a chunk boundary
        // would tear — which is why the smooth picture waits for a gesture to
        // settle.
        assert!(VoxelDisplay::Boxes.is_incremental());
        assert!(!VoxelDisplay::Smooth.is_incremental());
    }

    #[test]
    fn the_blur_is_clamped_to_what_the_engine_describes() {
        assert_eq!(SmoothBlur::new(-4).passes(), 0);
        assert_eq!(SmoothBlur::new(99).passes(), SmoothBlur::MOST);
        assert_eq!(SmoothBlur::new(1).passes(), 1);
        assert!(SmoothBlur::new(1).can_lose_detail());
    }
}
