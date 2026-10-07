//! The box a mask extrusion is measured in, and the budget it must fit.
//!
//! `clay_document_mask_extrude` measures the mask as a dense distance array
//! over the mask's bounds padded on every side by the thickness, the rim
//! rounding and a few cells — two bytes and three floats per cell, before the
//! volume it then samples over the same box. The thickness is the caller's to
//! set and nothing in the engine bounds it, so a wall of 100 units on a patch
//! the size of the starting form would cost gigabytes and seconds before it was
//! refused, if it was refused at all. The document counts the cells first and
//! refuses past [`MAX_MEASURED_CELLS`] with a reason, leaving no layer behind.
//!
//! This used to be the module that built the region for the engine to read —
//! the painted patch swept along the surface normal — because the engine of
//! ClayCore v0.120.1 intersected the wall with the mask's own volume and so
//! capped every wall at how far the paint reached off the surface. Since
//! ClayCore v0.126.0 (#667, issue #660) the engine reads the mask at the
//! source surface under each sample itself, and the swept region bought
//! nothing: on the unit sphere, 0.05, 0.1 and 0.6 walls from the painted mask
//! match the swept ones to within 0.0002 and are as even along the boundary,
//! at 1.3 to 4.8 times less cost (`mask_extrude_thickness.rs` has the figures).
//! Only the budget stayed.

/// A mask cell, in the mask lattice's integer coordinates.
pub(crate) type Cell = [i32; 3];

/// The most cells the engine's measurement may cover before the extrusion is
/// refused.
///
/// About 200³ — on the starting form at the mask's 0.02 cell, a 0.6 wall on an
/// outline through the form measures 1.6 million cells, and the budget is
/// reached by a wall of about 1.6 units.
pub(crate) const MAX_MEASURED_CELLS: usize = 8_000_000;

/// How far past the mask's bounds the engine measures, in world units, for a
/// wall of `thickness` with a rim rounded by `round` on a lattice of `cell`.
///
/// What `brush::mask_extrude` pads by: the thickness, the rounding, its band
/// of three cells and two more.
pub(crate) fn measured_reach(thickness: f32, round: f32, cell: f32) -> f32 {
    thickness + round.max(0.0) + 5.0 * cell
}

/// How many cells the engine's measurement of a mask with these inclusive
/// cell `bounds` covers once grown by `reach` world units on every side, or
/// `None` when the count overflows.
pub(crate) fn measured_cells(bounds: (Cell, Cell), cell: f32, reach: f32) -> Option<usize> {
    let grow = (reach / cell).ceil() as i64 + 1;
    (0..3).try_fold(1usize, |count, axis| {
        let span = (bounds.1[axis] as i64 - bounds.0[axis] as i64 + 1 + 2 * grow).max(0);
        count.checked_mul(usize::try_from(span).ok()?)
    })
}

/// Whether a measurement of `cells` fits the budget.
pub(crate) fn within_budget(cells: Option<usize>) -> bool {
    cells.is_some_and(|count| count <= MAX_MEASURED_CELLS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reach_is_the_wall_plus_its_rim_plus_five_cells() {
        assert!((measured_reach(0.6, 0.0, 0.02) - 0.7).abs() < 1e-6);
        assert!((measured_reach(0.6, 0.05, 0.02) - 0.75).abs() < 1e-6);
        assert!((measured_reach(0.6, -1.0, 0.02) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn the_measurement_grows_the_bounds_by_the_reach() {
        // Grown by ceil(0.1 / 0.1) + 1 = 2 cells each way: 6 per axis.
        assert_eq!(
            measured_cells(([0, 0, 0], [1, 1, 1]), 0.1, 0.1),
            Some(6 * 6 * 6)
        );
    }

    #[test]
    fn a_wall_past_the_budget_is_outside_it_and_an_overflow_is_too() {
        assert!(within_budget(measured_cells(
            ([0, 0, 0], [24, 24, 99]),
            0.02,
            0.7
        )));
        assert!(!within_budget(measured_cells(
            ([0, 0, 0], [0, 0, 0]),
            0.02,
            100.0
        )));
        assert!(!within_budget(None));
    }
}
