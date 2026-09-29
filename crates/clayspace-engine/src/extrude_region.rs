//! The region a mask extrusion may fill (#178, audit defect F5).
//!
//! `clay_document_mask_extrude` keeps the part of the wall's shell that lies
//! inside the mask's *own volume*, read at the point itself. A mask painted on
//! a surface is a thin volume around that surface — a dab's ball, an outline's
//! sweep — so a wall could never stand taller than the paint reached off the
//! form: 0.3 and 0.6 both stopped at about 0.11 on the unit sphere, with a top
//! that followed the dabs. CyberdyneCorp/ClayCore#660 asks the engine to read
//! the region where a point *projects onto the surface* instead.
//!
//! Until it does, the host hands the engine that region already built: a mask
//! whose value at every cell is the painted mask's value at the cell's foot on
//! the source surface, for every cell whose distance from the surface lies in
//! the band the chosen side fills. It is the painted patch swept along the
//! surface normal — a prism as tall as the wall asked for — so the engine's
//! intersection keeps the whole shell, and the wall's top is the shell's own
//! offset surface, even wherever the patch is.
//!
//! Everything here is arithmetic on the lattice. The engine calls that feed it
//! (the layer's distances and normals, the mask's samples) are made by
//! [`ClayDocument`](crate::ClayDocument), which owns the document.

use clayspace_model::ExtrudeSide;

/// A mask cell, in the mask lattice's integer coordinates.
pub(crate) type Cell = [i32; 3];

/// The most cells the region may consider before the extrusion is refused.
///
/// The box the region is searched in grows by the thickness on every side, and
/// the engine's own measurement of the mask is a dense array over the same
/// box, so a thickness far past the patch would cost seconds and gigabytes on
/// either side of the call. About 200³, which is a 0.5 wall on a patch the
/// size of the starting form at the mask's 0.02 cell.
pub(crate) const MAX_CANDIDATES: usize = 8_000_000;

/// The signed distances from the source surface the chosen side fills, widened
/// by a cell each way so the engine's intersection, not this sweep, decides
/// where the wall ends.
pub(crate) fn distance_band(side: ExtrudeSide, thickness: f32, cell: f32) -> (f32, f32) {
    let (low, high) = match side {
        ExtrudeSide::Outward => (0.0, thickness),
        ExtrudeSide::Inward => (-thickness, 0.0),
        ExtrudeSide::Centred => (-0.5 * thickness, 0.5 * thickness),
    };
    (low - cell, high + cell)
}

/// Every cell of the mask's bounds grown by `reach` world units, or `None`
/// when that box holds more than [`MAX_CANDIDATES`] cells.
///
/// `bounds` are the mask's inclusive cell bounds as the engine reports them.
pub(crate) fn candidates(bounds: (Cell, Cell), cell: f32, reach: f32) -> Option<Vec<Cell>> {
    let grow = (reach / cell).ceil() as i32 + 1;
    let lo = bounds.0.map(|v| v - grow);
    let hi = bounds.1.map(|v| v + grow);
    let span: Vec<usize> = (0..3)
        .map(|a| (hi[a] - lo[a] + 1).max(0) as usize)
        .collect();
    let count = span.iter().try_fold(1usize, |n, s| n.checked_mul(*s))?;
    if count > MAX_CANDIDATES {
        return None;
    }
    let mut cells = Vec::with_capacity(count);
    for z in lo[2]..=hi[2] {
        for y in lo[1]..=hi[1] {
            cells.extend((lo[0]..=hi[0]).map(|x| [x, y, z]));
        }
    }
    Some(cells)
}

/// The world position of a cell's centre. Cell centres sit half a cell in,
/// which is how the engine lays out every mask lattice.
pub(crate) fn centre(cell: Cell, size: f32) -> [f32; 3] {
    cell.map(|v| (v as f32 + 0.5) * size)
}

/// Where a point lands on the surface it is `distance` from, along the unit
/// `normal` the field reports there.
pub(crate) fn foot(point: [f32; 3], distance: f32, normal: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|a| point[a] - distance * normal[a])
}

/// The box that sets exactly one cell: a mask fill sets the cells whose centre
/// lies inside the box, and this one is half a cell wide around that centre.
pub(crate) fn fill_box(cell: Cell, size: f32) -> ([f32; 3], [f32; 3]) {
    let c = centre(cell, size);
    (c.map(|v| v - 0.25 * size), c.map(|v| v + 0.25 * size))
}

/// Which of `distances` fall in `band`, as indices into it.
pub(crate) fn in_band(distances: &[f32], band: (f32, f32)) -> Vec<usize> {
    distances
        .iter()
        .enumerate()
        .filter(|(_, d)| (band.0..=band.1).contains(*d))
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_side_fills_its_own_band() {
        let cell = 0.02;
        let near =
            |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6;
        assert!(near(
            distance_band(ExtrudeSide::Outward, 0.6, cell),
            (-0.02, 0.62)
        ));
        assert!(near(
            distance_band(ExtrudeSide::Inward, 0.6, cell),
            (-0.62, 0.02)
        ));
        assert!(near(
            distance_band(ExtrudeSide::Centred, 0.6, cell),
            (-0.32, 0.32)
        ));
    }

    #[test]
    fn candidates_grow_the_bounds_by_the_reach() {
        let cells = candidates(([0, 0, 0], [1, 1, 1]), 0.1, 0.1).expect("small");
        // Grown by ceil(0.1 / 0.1) + 1 = 2 cells each way: 6 per axis.
        assert_eq!(cells.len(), 6 * 6 * 6);
        assert!(cells.contains(&[-2, -2, -2]));
        assert!(cells.contains(&[3, 3, 3]));
        assert!(!cells.contains(&[4, 0, 0]));
    }

    #[test]
    fn a_box_past_the_budget_is_refused_rather_than_built() {
        assert!(candidates(([0, 0, 0], [0, 0, 0]), 0.02, 100.0).is_none());
    }

    #[test]
    fn a_foot_is_the_point_moved_back_along_the_normal() {
        let f = foot([0.0, 0.0, 1.5], 0.5, [0.0, 0.0, 1.0]);
        assert!((f[2] - 1.0).abs() < 1e-6 && f[0] == 0.0 && f[1] == 0.0);
        let inside = foot([0.0, 0.8, 0.0], -0.2, [0.0, 1.0, 0.0]);
        assert!((inside[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_fill_box_holds_its_own_centre_and_no_neighbour() {
        let (lo, hi) = fill_box([3, -2, 0], 0.02);
        let own = centre([3, -2, 0], 0.02);
        let next = centre([4, -2, 0], 0.02);
        assert!((0..3).all(|a| lo[a] < own[a] && own[a] < hi[a]));
        assert!(!(0..3).all(|a| lo[a] < next[a] && next[a] < hi[a]));
    }

    #[test]
    fn only_distances_in_the_band_are_kept() {
        assert_eq!(in_band(&[-0.5, 0.0, 0.3, 0.7], (-0.02, 0.62)), vec![1, 2]);
    }
}
