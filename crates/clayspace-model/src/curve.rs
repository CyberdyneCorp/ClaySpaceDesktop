//! A curve placed with control points, and the tube swept along it.
//!
//! Nomad calls it a Tube, 3DCoat a spline. The thing that makes it different
//! from a brush is not the shape it leaves but that **it can be gone back to**:
//! a stroke is over when the pointer comes up, and a curve is a set of points
//! that stay where they were put.
//!
//! The engine has every piece — a swept primitive carrying a profile along a
//! guide, seven profile kinds, four ways for a point to join the next, and an
//! undoable replace for a placed curve's whole point list. What is here is the
//! vocabulary above them.

use crate::sculpt::ModelError;

/// The 2D cross-section carried along the guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurveProfile {
    #[default]
    Circle,
    Square,
    Hexagon,
    Triangle,
}

impl CurveProfile {
    pub const ALL: [CurveProfile; 4] = [Self::Circle, Self::Square, Self::Hexagon, Self::Triangle];

    pub fn label(self) -> &'static str {
        match self {
            Self::Circle => "Círculo",
            Self::Square => "Quadrado",
            Self::Hexagon => "Hexágono",
            Self::Triangle => "Triângulo",
        }
    }
}

/// How each control point joins the one after it.
///
/// The engine offers a fourth — a cubic shaped by handles — which is not
/// offered here because handles need two more draggable things per point and
/// a way to break their symmetry. That is a tool of its own, not a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurveJoin {
    /// Straight from each point to the next.
    Corners,
    /// Catmull-Rom: passes *through* the points.
    #[default]
    Through,
    /// A uniform cubic B-spline. Approximating rather than interpolating, so
    /// it rounds the corners off and the curve sits inside its own points.
    Rounded,
}

impl CurveJoin {
    pub const ALL: [CurveJoin; 3] = [Self::Corners, Self::Through, Self::Rounded];

    pub fn label(self) -> &'static str {
        match self {
            Self::Corners => "Cantos",
            Self::Through => "Pelos pontos",
            Self::Rounded => "Arredondado",
        }
    }
}

/// One control point: where it is, and how thick the tube is there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurvePoint {
    pub position: [f32; 3],
    pub radius: f32,
}

/// The curve as the interface holds it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CurveState {
    /// Whether a curve is being placed or edited.
    pub active: bool,
    pub points: Vec<CurvePoint>,
    /// The points under the sculptor's hand, ascending.
    pub selection: Vec<usize>,
    pub join: CurveJoin,
    pub profile: CurveProfile,
}

/// The fewest points a curve can be swept along.
///
/// Two. One is a point and the engine refuses to sweep along it — "cutting it
/// below two points would leave the sweep with nothing to follow".
pub const FEWEST_POINTS: usize = 2;

impl CurveState {
    pub fn is_selected(&self, index: usize) -> bool {
        self.selection.binary_search(&index).is_ok()
    }

    /// Whether there is enough of a curve to sweep anything along.
    pub fn can_be_swept(&self) -> bool {
        self.points.len() >= FEWEST_POINTS
    }

    /// The middle of the selection — where a manipulator would sit.
    pub fn pivot(&self) -> Option<[f32; 3]> {
        if self.selection.is_empty() {
            return None;
        }
        let mut sum = [0.0f32; 3];
        for index in &self.selection {
            let point = self.points.get(*index)?;
            for (axis, at) in sum.iter_mut().zip(point.position) {
                *axis += at;
            }
        }
        let count = self.selection.len() as f32;
        Some(sum.map(|axis| axis / count))
    }

    /// Every consecutive pair, which is how the curve is drawn.
    ///
    /// The control polygon rather than the curve itself: a Catmull-Rom through
    /// the points and the straight lines between them are different lines, and
    /// what a sculptor drags are the points. Drawing the tessellated curve as
    /// well would be drawing the surface twice, since the sweep is already
    /// there.
    pub fn edges(&self) -> Vec<(u32, u32)> {
        (1..self.points.len())
            .map(|at| (at as u32 - 1, at as u32))
            .collect()
    }
}

/// The curve, as something the interface can place, edit and sweep.
pub trait CurveModel {
    fn curve(&self) -> CurveState;

    /// Starts a curve, taking down any that was up.
    fn begin_curve(&mut self);

    /// Appends a control point at the end of the curve.
    fn add_curve_point(&mut self, at: [f32; 3], radius: f32) -> Result<(), ModelError>;

    /// Selects one control point, replacing the selection. `None` clears it.
    fn select_curve_point(&mut self, index: Option<usize>);

    /// Adds or removes one without disturbing the rest.
    fn toggle_curve_point(&mut self, index: usize);

    /// Moves every selected control point by a displacement.
    fn drag_curve(&mut self, by: [f32; 3]) -> Result<(), ModelError>;

    /// Puts the manipulator on the selected control points.
    ///
    /// A curve is the one target the manipulator has that carries no engine
    /// transform: its points belong to the application while it is being
    /// authored, so a drag maps each of them to a new place rather than
    /// producing a position, an axis-angle and a scale for a node. This is the
    /// same path the deformation cage takes, and it is why the two turn and
    /// scale identically.
    ///
    /// Provided, so a double that models no curves ignores it rather than
    /// spelling out a refusal it never reaches.
    fn drag_curve_points(
        &mut self,
        drag: crate::GizmoDrag,
        to: [f32; 3],
        snap: bool,
    ) -> Result<(), ModelError> {
        let _ = (drag, to, snap);
        Ok(())
    }

    /// The middle of the selected control points, which is where a manipulator
    /// on them sits and what a turn is about.
    ///
    /// `None` when nothing is selected — a manipulator with nothing to act on
    /// is not drawn.
    fn curve_pivot(&self) -> Option<[f32; 3]> {
        let curve = self.curve();
        if !curve.active || curve.selection.is_empty() {
            return None;
        }
        let mut middle = [0.0f32; 3];
        for index in &curve.selection {
            let point = curve.points.get(*index)?;
            for (at, value) in middle.iter_mut().zip(point.position) {
                *at += value;
            }
        }
        let count = curve.selection.len() as f32;
        Some(middle.map(|value| value / count))
    }

    /// Sets the radius of every selected point, or of all of them where
    /// nothing is selected.
    fn set_curve_radius(&mut self, radius: f32) -> Result<(), ModelError>;

    fn set_curve_join(&mut self, join: CurveJoin) -> Result<(), ModelError>;
    fn set_curve_profile(&mut self, profile: CurveProfile) -> Result<(), ModelError>;

    /// Removes the selected control points.
    fn remove_curve_points(&mut self) -> Result<(), ModelError>;

    /// Leaves the swept form in the layer and takes the curve down.
    fn apply_curve(&mut self) -> Result<(), ModelError>;

    /// Takes the curve down, and the form with it.
    fn cancel_curve(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(count: usize) -> CurveState {
        CurveState {
            active: true,
            points: (0..count)
                .map(|at| CurvePoint {
                    position: [at as f32, 0.0, 0.0],
                    radius: 0.1,
                })
                .collect(),
            selection: Vec::new(),
            join: CurveJoin::default(),
            profile: CurveProfile::default(),
        }
    }

    #[test]
    fn a_curve_needs_two_points_to_sweep_along() {
        // One is a point, and the engine refuses to sweep along it — cutting a
        // guide below two "would leave the sweep with nothing to follow".
        assert!(!curve(0).can_be_swept());
        assert!(!curve(1).can_be_swept());
        assert!(curve(2).can_be_swept());
    }

    #[test]
    fn the_curve_is_drawn_as_a_chain() {
        // The control polygon, one edge per consecutive pair. Not a loop: a
        // swept guide cannot be closed, and the engine refuses one rather than
        // ignoring it — transporting a frame around a loop does not generally
        // return it to its starting orientation.
        assert_eq!(curve(4).edges(), vec![(0, 1), (1, 2), (2, 3)]);
        assert!(curve(1).edges().is_empty());
        assert!(curve(0).edges().is_empty());
    }

    #[test]
    fn the_pivot_is_the_middle_of_the_selection() {
        let mut curve = curve(5);
        assert_eq!(curve.pivot(), None, "nothing selected has no middle");
        curve.selection = vec![0, 4];
        assert_eq!(curve.pivot(), Some([2.0, 0.0, 0.0]));
        curve.selection = vec![2];
        assert_eq!(curve.pivot(), Some([2.0, 0.0, 0.0]));
    }

    #[test]
    fn joins_and_profiles_all_have_a_name() {
        for join in CurveJoin::ALL {
            assert!(!join.label().is_empty(), "{join:?} has no name");
        }
        for profile in CurveProfile::ALL {
            assert!(!profile.label().is_empty(), "{profile:?} has no name");
        }
        // Through the points by default: a curve laid down by clicking is a
        // path a person meant, and one that missed its own points would need
        // explaining.
        assert_eq!(CurveJoin::default(), CurveJoin::Through);
        assert_eq!(CurveProfile::default(), CurveProfile::Circle);
    }
}
