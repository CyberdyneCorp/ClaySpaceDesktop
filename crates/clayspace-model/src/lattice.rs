//! A cage around the form, and the points a sculptor drags to bend it.
//!
//! ZBrush spells it the Gizmo Lattice, Blender the Lattice modifier, Maya an
//! FFD. All three show the same thing: a box of control points around the
//! model, dragged directly in the viewport, with the surface following.
//!
//! It is not a brush and not a stroke. A brush acts where the pointer is; a
//! cage acts on the whole form at once, and the sculptor's attention is on a
//! handle rather than on the clay. That is why this is a vocabulary of its own
//! rather than another entry in [`crate::ToolKind`] — the same reason the mask
//! is.

use crate::sculpt::ModelError;
use crate::Representation;

/// How many control points a cage may have per axis, on this representation.
///
/// Two different engine routes with two different ceilings, and the difference
/// is not arbitrary. A mesh is deformed *forward*: each vertex is evaluated
/// once, so a fine cage costs what a fine cage costs. A field is deformed by
/// an inverse point map resolved into one deformer per item and evaluated at
/// every sample, so the same cage is paid for over and over.
pub fn division_limit(representation: Representation) -> Option<i32> {
    match representation {
        Representation::Mesh => Some(32),
        Representation::Sdf => Some(4),
        // A grid has no lattice route at all: no forward vertex pass and no
        // deformer stack to resolve one into.
        Representation::Voxel => None,
    }
}

/// Whether a cage can be put around a layer of this representation.
pub fn can_be_caged(representation: Representation) -> bool {
    division_limit(representation).is_some()
}

/// The smallest cage that is a cage: eight corners, exactly trilinear.
pub const MIN_DIVISIONS: i32 = 2;

/// Divisions clamped to what this representation accepts.
///
/// Clamped rather than refused, because the panel offers one control for both
/// and a sculptor who moves it past a ceiling meant "as fine as this can go"
/// rather than "fail".
pub fn clamp_divisions(divisions: [i32; 3], representation: Representation) -> [i32; 3] {
    let limit = division_limit(representation).unwrap_or(MIN_DIVISIONS);
    divisions.map(|n| n.clamp(MIN_DIVISIONS, limit))
}

/// The cage as the interface holds it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LatticeState {
    /// Whether a cage is up. Nothing below means anything when this is false.
    pub active: bool,
    pub divisions: [i32; 3],
    /// Where every control point is *now* — rest plus whatever it was dragged
    /// by — in world space, x fastest.
    ///
    /// Positions rather than offsets, because this is what the viewport draws
    /// and what the pointer is tested against. The offsets are the engine's
    /// business.
    pub points: Vec<[f32; 3]>,
    /// The point under the sculptor's hand, if any.
    pub selected: Option<usize>,
    /// Whether any point has been dragged.
    ///
    /// An untouched cage is exactly the identity, and applying one would pay
    /// for a pass over every vertex to move them all by zero.
    pub touched: bool,
}

impl LatticeState {
    /// The index of a control point, x fastest — the engine's own order.
    pub fn index(&self, at: [i32; 3]) -> Option<usize> {
        let [nx, ny, nz] = self.divisions;
        let inside = (0..3).all(|axis| {
            let n = [nx, ny, nz][axis];
            (0..n).contains(&at[axis])
        });
        inside.then(|| ((at[2] * ny + at[1]) * nx + at[0]) as usize)
    }

    /// The grid coordinate of a control point.
    pub fn coordinate(&self, index: usize) -> Option<[i32; 3]> {
        let [nx, ny, _] = self.divisions;
        if nx <= 0 || ny <= 0 || index >= self.points.len() {
            return None;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        Some([
            (index % nx) as i32,
            ((index / nx) % ny) as i32,
            (index / (nx * ny)) as i32,
        ])
    }

    /// Every pair of adjacent control points, which is what the cage is drawn
    /// as.
    ///
    /// Adjacency along the three axes only. The diagonals of a cell are not
    /// edges of the cage, and drawing them would turn a readable box into a
    /// thicket.
    pub fn edges(&self) -> Vec<(u32, u32)> {
        let [nx, ny, nz] = self.divisions;
        let mut edges = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let Some(from) = self.index([i, j, k]) else {
                        continue;
                    };
                    for step in [[1, 0, 0], [0, 1, 0], [0, 0, 1]] {
                        let to = [i + step[0], j + step[1], k + step[2]];
                        if let Some(to) = self.index(to) {
                            edges.push((from as u32, to as u32));
                        }
                    }
                }
            }
        }
        edges
    }
}

/// The cage, as something the interface can put up, drag and apply.
pub trait LatticeModel {
    fn lattice(&self) -> LatticeState;

    /// Puts a cage around the active layer, sized to what it contains.
    ///
    /// Replaces one already up: a sculptor who changes the divisions is asking
    /// for a different cage, and carrying the old drags across a change of
    /// resolution would move points they never touched.
    fn begin_lattice(&mut self, divisions: [i32; 3]) -> Result<(), ModelError>;

    /// Picks the control point nearest a ray, within a screen-independent
    /// tolerance in world units. `None` clears the selection.
    fn select_lattice_point(&mut self, index: Option<usize>);

    /// Moves the selected control point to a world position.
    ///
    /// The whole drag is one movement of one point rather than a series, so a
    /// gesture ends where the pointer ends however many frames it took.
    fn drag_lattice_point(&mut self, to: [f32; 3]) -> Result<(), ModelError>;

    /// Bends the layer through the cage and takes the cage down.
    ///
    /// One undo step for the whole thing, which is the unit a sculptor thinks
    /// in: they bent the form once.
    fn apply_lattice(&mut self) -> Result<(), ModelError>;

    /// Takes the cage down, changing nothing.
    fn cancel_lattice(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cage(divisions: [i32; 3]) -> LatticeState {
        let count = (divisions[0] * divisions[1] * divisions[2]) as usize;
        LatticeState {
            active: true,
            divisions,
            points: vec![[0.0; 3]; count],
            selected: None,
            touched: false,
        }
    }

    #[test]
    fn an_index_and_a_coordinate_are_the_same_point() {
        let cage = cage([3, 4, 5]);
        for k in 0..5 {
            for j in 0..4 {
                for i in 0..3 {
                    let index = cage.index([i, j, k]).expect("inside");
                    assert_eq!(
                        cage.coordinate(index),
                        Some([i, j, k]),
                        "index {index} does not read back as ({i}, {j}, {k})"
                    );
                }
            }
        }
    }

    #[test]
    fn a_coordinate_outside_the_cage_has_no_index() {
        let cage = cage([2, 2, 2]);
        assert_eq!(cage.index([2, 0, 0]), None);
        assert_eq!(cage.index([0, -1, 0]), None);
        assert!(cage.index([1, 1, 1]).is_some());
    }

    #[test]
    fn the_cage_is_drawn_as_a_box_and_not_a_thicket() {
        // Three axes, and only along them: a 2x2x2 cage is a cube, so it has
        // twelve edges. Adding the cell diagonals would give it twenty-four
        // and make it unreadable.
        let cube = cage([2, 2, 2]);
        assert_eq!(cube.edges().len(), 12);

        // In general, one edge per adjacent pair on each axis.
        let cage = cage([3, 2, 2]);
        // Two along x for each of the four (j, k) columns, plus one along y
        // for each of the six (i, k), plus one along z for each of the six
        // (i, j).
        assert_eq!(cage.edges().len(), 2 * 2 * 2 + 3 * 2 + 3 * 2);
    }

    #[test]
    fn the_division_ceiling_is_the_representations_own() {
        // Not arbitrary: a mesh is deformed forward, once per vertex; a field
        // is deformed by an inverse map evaluated at every sample.
        assert_eq!(division_limit(Representation::Mesh), Some(32));
        assert_eq!(division_limit(Representation::Sdf), Some(4));
        assert_eq!(division_limit(Representation::Voxel), None);
        assert!(!can_be_caged(Representation::Voxel));

        assert_eq!(
            clamp_divisions([40, 1, 8], Representation::Mesh),
            [32, 2, 8],
            "a mesh cage was not clamped to its own ceiling and floor"
        );
        assert_eq!(
            clamp_divisions([40, 1, 8], Representation::Sdf),
            [4, 2, 4],
            "a field cage went past the four points per axis the engine \
             accepts, which it would refuse outright"
        );
    }
}
