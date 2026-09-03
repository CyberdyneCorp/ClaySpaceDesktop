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

use crate::gizmo::{GizmoDrag, GizmoHandle, GizmoMode};
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
        // Nor has a hierarchy, and this is the one answer here that looks
        // wrong at a glance. A cage is exactly what a subdivision surface has,
        // and bending it at level 0 propagating up through every level is the
        // representation's whole point — so "no cage" reads like a mistake.
        //
        // It is not, because the two cages are different objects. This one is
        // a lattice the interface makes up around whatever it is given, and
        // the engine applies it through `clay_mesh_sculptor_lattice`, which
        // takes a fixed mesh. There is no `clay_multires_*_lattice`: a level
        // above the base is *derived*, so pushing its vertices through a point
        // map writes nothing the next evaluation would not overwrite, and
        // pushing the base's through one is a deformation of the cage that the
        // ABI offers no entry point for. The hierarchy's own cage is dragged
        // by sculpting level 0, which is a stroke and not a lattice.
        Representation::Multires => None,
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
    /// The points under the sculptor's hand, in ascending order.
    ///
    /// A set rather than one point, because that is what makes the manipulator
    /// worth having: dragging points one at a time needs no gizmo, and turning
    /// a face of the cage cannot be done without one.
    pub selection: Vec<usize>,
    /// Which of the manipulator's three modes is in force.
    pub mode: GizmoMode,
    /// The longest side of the box the cage was *built* with, in world units.
    ///
    /// The rest box and not the current one, because this is what sizes the
    /// control-point handles — and a handle sized from where the points are
    /// now grows every time one is dragged out, so pulling a single corner
    /// inflated every other handle on screen.
    pub rest_span: f32,
    /// Whether any point has been dragged.
    ///
    /// An untouched cage is exactly the identity, and applying one would pay
    /// for a pass over every vertex to move them all by zero.
    pub touched: bool,
}

impl LatticeState {
    pub fn is_selected(&self, index: usize) -> bool {
        self.selection.binary_search(&index).is_ok()
    }

    /// The point the manipulator sits on and transforms about: the middle of
    /// the selection.
    ///
    /// The middle rather than the last point picked, so adding a point to a
    /// selection moves the widget to where the *selection* is rather than
    /// leaving it on whichever corner happened to be clicked first.
    /// Whether the manipulator's turn and scale can do anything.
    ///
    /// They act about the middle of the selection, and one point's middle is
    /// itself: turning a point about itself moves it exactly nowhere, and so
    /// does scaling it. The arithmetic is right and the gesture is empty, which
    /// is the worst kind of broken — so the interface refuses the mode rather
    /// than drawing a live-looking widget that cannot move anything.
    ///
    /// Moving is not affected: it needs no pivot.
    pub fn can_transform(&self) -> bool {
        self.selection.len() >= 2
    }

    pub fn pivot(&self) -> Option<[f32; 3]> {
        if self.selection.is_empty() {
            return None;
        }
        let mut sum = [0.0f32; 3];
        for index in &self.selection {
            let point = self.points.get(*index)?;
            for axis in 0..3 {
                sum[axis] += point[axis];
            }
        }
        let count = self.selection.len() as f32;
        Some(sum.map(|axis| axis / count))
    }

    /// A drag on the manipulator, ready to be applied to each selected point.
    ///
    /// `None` where there is nothing selected to transform.
    /// The gesture a press on the manipulator starts.
    ///
    /// `view_axis` is where the camera is looking, which only the outer ring
    /// uses — but it is taken for every drag rather than only that one, so
    /// there is no handle whose gesture is built a different way.
    pub fn drag_from(
        &self,
        handle: GizmoHandle,
        anchor: [f32; 3],
        view_axis: [f32; 3],
    ) -> Option<GizmoDrag> {
        Some(GizmoDrag {
            mode: self.mode,
            handle,
            pivot: self.pivot()?,
            anchor,
            view_axis,
        })
    }

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

    /// Selects one control point, replacing what was selected. `None` clears
    /// the selection.
    fn select_lattice_point(&mut self, index: Option<usize>);

    /// Adds or removes one point without disturbing the rest.
    ///
    /// What a modifier-click does, and the only way to build the selection a
    /// manipulator exists to transform.
    fn toggle_lattice_point(&mut self, index: usize);

    /// Selects a whole set of points at once, replacing what was selected.
    ///
    /// What a selection box leaves behind. Not a loop over
    /// [`LatticeModel::select_lattice_point`], which would keep only the last
    /// of them, nor one over [`LatticeModel::toggle_lattice_point`], which
    /// would take back any point already held — a box says *these*, not *these
    /// as well as the opposite of what you had*.
    fn select_lattice_points(&mut self, indices: &[usize]);

    /// Which of the manipulator's three modes is in force.
    fn set_gizmo_mode(&mut self, mode: GizmoMode);

    /// Starts a manipulator drag on a handle, from a point on the drag plane.
    /// `view_axis` is where the camera is looking, which the outer ring turns
    /// about. Taken at the press and held, so a camera that moves mid-drag
    /// does not twist the selection under a hand that has not moved.
    fn begin_gizmo_drag(&mut self, handle: GizmoHandle, anchor: [f32; 3], view_axis: [f32; 3]);

    /// Carries the selection to where the pointer is now.
    ///
    /// Resolved from the anchor every time rather than accumulated, so a drag
    /// ends where the pointer ends however many frames it took.
    /// `snap` rounds a rotation to whole increments. Read per drag rather
    /// than per gesture, so the modifier can be taken up and let go part-way
    /// through one.
    fn drag_gizmo(&mut self, to: [f32; 3], snap: bool) -> Result<(), ModelError>;

    /// Ends a manipulator drag, banking where the selection ended up.
    fn end_gizmo_drag(&mut self);

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
            selection: Vec::new(),
            mode: GizmoMode::default(),
            rest_span: 1.0,
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
        // And a hierarchy, which HAS a cage and still cannot be given this
        // one: the engine's lattice takes a fixed mesh, and a level above the
        // base is derived from the level below rather than stored.
        assert_eq!(division_limit(Representation::Multires), None);
        assert!(!can_be_caged(Representation::Multires));

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

#[cfg(test)]
mod selection_tests {
    use super::*;

    fn corners() -> LatticeState {
        LatticeState {
            active: true,
            divisions: [2, 2, 2],
            points: (0..8)
                .map(|at| {
                    [
                        if at & 1 == 0 { -1.0 } else { 1.0 },
                        if at & 2 == 0 { -1.0 } else { 1.0 },
                        if at & 4 == 0 { -1.0 } else { 1.0 },
                    ]
                })
                .collect(),
            selection: Vec::new(),
            mode: GizmoMode::Move,
            rest_span: 2.0,
            touched: false,
        }
    }

    #[test]
    fn nothing_selected_has_no_pivot() {
        // And so no manipulator: a widget floating over an empty selection
        // would be a control with nothing to control.
        assert_eq!(corners().pivot(), None);
        assert_eq!(
            corners().drag_from(GizmoHandle::Centre, [0.0; 3], [0.0, 0.0, 1.0]),
            None
        );
    }

    #[test]
    fn the_pivot_is_the_middle_of_the_selection() {
        // The middle rather than the last point picked, so adding a point
        // moves the widget to where the selection is rather than leaving it on
        // whichever corner was clicked first.
        let mut cage = corners();
        cage.selection = vec![0];
        assert_eq!(cage.pivot(), Some([-1.0, -1.0, -1.0]));

        // The whole top face: four corners, all at y = 1.
        cage.selection = vec![2, 3, 6, 7];
        let pivot = cage.pivot().expect("a selection has a middle");
        assert!(
            (pivot[0]).abs() < 1e-6 && (pivot[1] - 1.0).abs() < 1e-6 && pivot[2].abs() < 1e-6,
            "the top face's middle is {pivot:?}"
        );
    }

    #[test]
    fn a_selection_reads_back_as_selected() {
        let mut cage = corners();
        cage.selection = vec![1, 4, 6];
        for at in 0..8 {
            assert_eq!(
                cage.is_selected(at),
                [1, 4, 6].contains(&at),
                "point {at} disagreed about being selected"
            );
        }
    }
    #[test]
    fn one_point_has_nothing_to_turn_or_scale_about() {
        // Reported: the rings draw and dragging them does nothing. They were
        // drawn on a selection of one, whose middle is the point itself — so
        // the turn was about the very thing it was turning, and the scale was
        // about the very thing it was scaling. Both are exactly no movement.
        // The arithmetic was right and the gesture was empty.
        let mut cage = corners();
        cage.selection = vec![0];
        assert!(!cage.can_transform(), "one point offered a turn");

        // And it really is a no-op, not merely disallowed: this is why the
        // interface refuses it rather than letting it run.
        let pivot = cage.pivot().expect("one point still has a middle");
        let point = cage.points[0];
        assert_eq!(pivot, point, "the pivot of one point is that point");
        for mode in [GizmoMode::Rotate, GizmoMode::Scale] {
            let drag = GizmoDrag {
                mode,
                handle: GizmoHandle::Axis(1),
                pivot,
                anchor: [pivot[0] + 1.0, pivot[1], pivot[2]],
                view_axis: [0.0, 0.0, 1.0],
            };
            let after = drag.apply(point, [pivot[0], pivot[1], pivot[2] + 1.0], false);
            let moved = (0..3)
                .map(|i| (after[i] - point[i]).powi(2))
                .sum::<f32>()
                .sqrt();
            assert!(
                moved < 1e-6,
                "{mode:?} on a selection of one moved it by {moved}, which \
                 would mean the pivot is not the point"
            );
        }
    }

    #[test]
    fn two_points_are_enough_to_turn() {
        let mut cage = corners();
        cage.selection = vec![0, 1];
        assert!(cage.can_transform());
        // And moving never needed a pivot, so it is not gated on this.
        cage.selection = vec![0];
        assert!(cage.pivot().is_some(), "a move still has somewhere to act");
    }
}

#[cfg(test)]
mod handle_tests {
    use super::*;

    #[test]
    fn the_handle_size_is_the_rest_box_and_not_the_dragged_one() {
        // Reported: selecting a point and moving it made every *other* handle
        // grow. The size came from the cage's current extent, so pulling one
        // corner out inflated the whole set — and the handles a sculptor was
        // aiming at moved under the pointer as they worked.
        let mut cage = LatticeState {
            active: true,
            divisions: [2, 2, 2],
            points: (0..8)
                .map(|at| {
                    [
                        if at & 1 == 0 { -1.0 } else { 1.0 },
                        if at & 2 == 0 { -1.0 } else { 1.0 },
                        if at & 4 == 0 { -1.0 } else { 1.0 },
                    ]
                })
                .collect(),
            selection: Vec::new(),
            mode: GizmoMode::Move,
            rest_span: 2.0,
            touched: false,
        };
        let before = cage.rest_span;
        // One corner hauled a long way out.
        cage.points[7] = [9.0, 9.0, 9.0];
        cage.touched = true;
        assert_eq!(
            cage.rest_span, before,
            "dragging a corner changed what the handles are sized from"
        );
    }
}
