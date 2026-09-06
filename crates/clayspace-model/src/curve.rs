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

    /// Every consecutive pair of *control points*.
    ///
    /// The control polygon. Kept for `Corners`, where it is the curve, and for
    /// anything that wants the points' own chain — but it is no longer what
    /// the viewport draws as the guide, because for the other two joins it is
    /// a different line from the one the tube follows. See [`Self::path`].
    pub fn edges(&self) -> Vec<(u32, u32)> {
        (1..self.points.len())
            .map(|at| (at as u32 - 1, at as u32))
            .collect()
    }

    /// The line the tube actually follows, tessellated.
    ///
    /// The control polygon was drawn here first, on the reasoning that the
    /// sweep already shows the curve so drawing it again would be drawing the
    /// surface twice. That is true of a curve you can see and false of one you
    /// cannot: the guide runs *inside* the tube, and with `Through` or
    /// `Rounded` the chords visibly leave it — cutting corners the tube rounds
    /// — so the one line a sculptor can see was the one the tube does not
    /// take.
    ///
    /// **This is the interface's own reading of the join, not a number the
    /// engine handed back.** There is no ABI call that returns a swept guide's
    /// tessellation, so agreement with the engine is a thing to *test* rather
    /// than to assume, and `the_guide_lies_inside_the_tube_it_describes` is
    /// what holds it: every sample here is evaluated against the swept field
    /// and has to be inside it.
    pub fn path(&self) -> Vec<[f32; 3]> {
        if self.points.len() < FEWEST_POINTS {
            return self.points.iter().map(|point| point.position).collect();
        }
        if self.join == CurveJoin::Corners {
            // Straight from each point to the next: the control polygon *is*
            // the curve, and sampling it would only add collinear vertices.
            return self.points.iter().map(|point| point.position).collect();
        }

        let at = |index: isize| -> [f32; 3] {
            let last = self.points.len() as isize - 1;
            self.points[index.clamp(0, last) as usize].position
        };
        let mut out = Vec::with_capacity((self.points.len() - 1) * SAMPLES_PER_SPAN + 1);
        for span in 0..self.points.len() as isize - 1 {
            let (p0, p1, p2, p3) = (at(span - 1), at(span), at(span + 1), at(span + 2));
            for step in 0..SAMPLES_PER_SPAN {
                let t = step as f32 / SAMPLES_PER_SPAN as f32;
                out.push(match self.join {
                    CurveJoin::Through => catmull_rom(p0, p1, p2, p3, t),
                    _ => b_spline(p0, p1, p2, p3, t),
                });
            }
        }
        // The end of the last span, which no span's interior reaches because
        // each stops short of `t = 1`.
        //
        // Evaluated through the same basis as everything before it rather than
        // written out as a case. The first draft pushed the last *control
        // point* instead, reasoning that a curve ends on it — true of
        // `Through`, which interpolates, and false of `Rounded`, which does
        // not reach its own endpoints. That put one vertex outside the tube,
        // and `the_guide_lies_inside_the_tube_it_describes` caught it at
        // exactly one sample of thirty-seven. Catmull-Rom at `t = 1` is the
        // last control point anyway, so there is nothing a special case buys.
        let last = self.points.len() as isize - 2;
        let (p0, p1, p2, p3) = (at(last - 1), at(last), at(last + 1), at(last + 2));
        out.push(match self.join {
            CurveJoin::Through => catmull_rom(p0, p1, p2, p3, 1.0),
            _ => b_spline(p0, p1, p2, p3, 1.0),
        });
        out
    }

    /// Where a point added at guide sample `sample` belongs in the control
    /// point list.
    ///
    /// The guide is tessellated, so a place on it is not a place in the list:
    /// a sample is somewhere inside a *span* between two control points, and
    /// what a sculptor means by "add a point here" is one that splits that
    /// span. The index returned is the one the new point takes, so everything
    /// from it onward shifts up by one.
    ///
    /// Clamped to a real span. A double-click landing on the very last sample
    /// must add a point before the end rather than after it, or the click that
    /// looks like splitting the curve extends it instead.
    pub fn insertion_for_sample(&self, sample: usize) -> usize {
        if self.points.len() < FEWEST_POINTS {
            return self.points.len();
        }
        let span = match self.join {
            // The path *is* the control points, so a sample sits on the point
            // of that index and the span after it is the one to split.
            CurveJoin::Corners => sample,
            _ => sample / SAMPLES_PER_SPAN,
        };
        (span + 1).min(self.points.len() - 1)
    }

    /// The consecutive pairs of [`Self::path`], for drawing it as a line list.
    pub fn path_edges(&self) -> Vec<(u32, u32)> {
        (1..self.path().len())
            .map(|at| (at as u32 - 1, at as u32))
            .collect()
    }
}

/// How finely each span between two control points is sampled.
///
/// Twelve. The guide is drawn as lines, so this is the only thing standing
/// between a curve and a visible chain of chords; it is not a cost anyone
/// notices, since a curve has tens of control points at most and this is the
/// overlay rather than the surface.
pub const SAMPLES_PER_SPAN: usize = 12;

/// Catmull-Rom, which passes through `p1` and `p2`.
fn catmull_rom(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3], t: f32) -> [f32; 3] {
    let (t2, t3) = (t * t, t * t * t);
    std::array::from_fn(|axis| {
        0.5 * ((2.0 * p1[axis])
            + (-p0[axis] + p2[axis]) * t
            + (2.0 * p0[axis] - 5.0 * p1[axis] + 4.0 * p2[axis] - p3[axis]) * t2
            + (-p0[axis] + 3.0 * p1[axis] - 3.0 * p2[axis] + p3[axis]) * t3)
    })
}

/// A uniform cubic B-spline, which approximates rather than interpolates — so
/// it rounds the corners off and sits inside its own control points.
fn b_spline(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3], t: f32) -> [f32; 3] {
    let (t2, t3) = (t * t, t * t * t);
    std::array::from_fn(|axis| {
        ((1.0 - t) * (1.0 - t) * (1.0 - t) * p0[axis]
            + (3.0 * t3 - 6.0 * t2 + 4.0) * p1[axis]
            + (-3.0 * t3 + 3.0 * t2 + 3.0 * t + 1.0) * p2[axis]
            + t3 * p3[axis])
            / 6.0
    })
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

    /// Puts a control point *into* the curve at `index`, splitting a span.
    ///
    /// Appending is what a click on empty space does; this is what a
    /// double-click on the guide does, and a curve that can only grow at its
    /// end cannot be refined in the middle — which is where a tube usually
    /// needs another point.
    fn insert_curve_point(
        &mut self,
        index: usize,
        at: [f32; 3],
        radius: f32,
    ) -> Result<(), ModelError>;

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

    /// The property the guide exists to have: it is the line the tube takes.
    ///
    /// `Through` is Catmull-Rom, which interpolates — so every control point
    /// has to appear on the path, or the guide is not passing through the
    /// points the sculptor placed.
    #[test]
    fn a_curve_through_its_points_visits_every_one_of_them() {
        let mut curve = curve(4);
        // Bent, so a straight line through them would fail this rather than
        // pass it by accident.
        curve.points[1].position = [1.0, 0.8, 0.0];
        curve.points[2].position = [2.0, -0.8, 0.0];
        curve.join = CurveJoin::Through;
        let path = curve.path();

        for point in &curve.points {
            let nearest = path
                .iter()
                .map(|at| {
                    (0..3)
                        .map(|axis| (at[axis] - point.position[axis]).powi(2))
                        .sum::<f32>()
                        .sqrt()
                })
                .fold(f32::INFINITY, f32::min);
            assert!(
                nearest < 1e-4,
                "the guide misses the control point at {:?} by {nearest}",
                point.position
            );
        }
    }

    /// And it is not the control polygon, which is what it used to be.
    #[test]
    fn a_curved_guide_leaves_the_chords_between_its_points() {
        let mut curve = curve(4);
        curve.points[1].position = [1.0, 1.0, 0.0];
        curve.points[2].position = [2.0, -1.0, 0.0];
        curve.join = CurveJoin::Through;

        // The chord from point 1 to point 2 runs straight; a Catmull-Rom
        // through four points does not, so some sample has to be off it.
        let (a, b) = (curve.points[1].position, curve.points[2].position);
        let farthest = curve
            .path()
            .iter()
            .map(|at| {
                // Distance from the infinite line through a and b.
                let d: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
                let w: [f32; 3] = std::array::from_fn(|i| at[i] - a[i]);
                let cross = [
                    w[1] * d[2] - w[2] * d[1],
                    w[2] * d[0] - w[0] * d[2],
                    w[0] * d[1] - w[1] * d[0],
                ];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt() / len
            })
            .fold(0.0f32, f32::max);
        assert!(
            farthest > 1e-2,
            "the guide is still the straight chain: it never leaves the chord              by more than {farthest}"
        );
    }

    /// Corners is the one join where the polygon *is* the curve, so sampling
    /// it would only add collinear vertices to draw.
    #[test]
    fn a_cornered_guide_is_exactly_its_control_points() {
        let mut curve = curve(4);
        curve.join = CurveJoin::Corners;
        let path = curve.path();
        assert_eq!(path.len(), 4);
        for (at, point) in path.iter().zip(&curve.points) {
            assert_eq!(*at, point.position);
        }
    }

    /// A rounded curve approximates: it must NOT pass through its interior
    /// points, or it is not the join it says it is.
    #[test]
    fn a_rounded_guide_sits_inside_its_own_points() {
        let mut curve = curve(4);
        curve.points[1].position = [1.0, 1.0, 0.0];
        curve.points[2].position = [2.0, -1.0, 0.0];
        curve.join = CurveJoin::Rounded;
        let path = curve.path();

        let interior = curve.points[1].position;
        let nearest = path
            .iter()
            .map(|at| {
                (0..3)
                    .map(|axis| (at[axis] - interior[axis]).powi(2))
                    .sum::<f32>()
                    .sqrt()
            })
            .fold(f32::INFINITY, f32::min);
        assert!(
            nearest > 1e-3,
            "a rounded curve passed through an interior control point, which              makes it indistinguishable from Through"
        );
    }

    /// Too few points to sweep is too few to tessellate, and must not panic.
    #[test]
    fn a_curve_with_nothing_in_it_has_a_path_with_nothing_in_it() {
        assert!(curve(0).path().is_empty());
        assert_eq!(curve(1).path().len(), 1);
        assert!(curve(0).path_edges().is_empty());
        // Two points still tessellate, so the guide and the sweep agree even
        // where the answer happens to be a straight line.
        assert!(curve(2).path().len() >= 2);
    }

    /// A double-click on the guide has to split the span it landed on, and
    /// the span is not the sample: the guide is tessellated, so sample 20 of a
    /// four-point curve is somewhere inside the second span rather than at
    /// control point 20.
    #[test]
    fn a_place_on_the_guide_names_the_span_it_splits() {
        let mut curve = curve(4);
        curve.join = CurveJoin::Through;
        // First span, so the new point goes between control points 0 and 1.
        assert_eq!(curve.insertion_for_sample(0), 1);
        assert_eq!(curve.insertion_for_sample(SAMPLES_PER_SPAN - 1), 1);
        // Second span.
        assert_eq!(curve.insertion_for_sample(SAMPLES_PER_SPAN), 2);
        // The very last sample must still split a span rather than extend the
        // curve: a click that looks like dividing the line would otherwise add
        // a point past the end.
        let last = curve.path().len() - 1;
        assert_eq!(curve.insertion_for_sample(last), curve.points.len() - 1);
    }

    /// On `Corners` the guide *is* the control points, so the mapping is not
    /// the same arithmetic and would be wrong if it were.
    #[test]
    fn a_cornered_guide_names_its_own_points() {
        let mut curve = curve(4);
        curve.join = CurveJoin::Corners;
        assert_eq!(curve.insertion_for_sample(0), 1);
        assert_eq!(curve.insertion_for_sample(1), 2);
        assert_eq!(curve.insertion_for_sample(3), 3, "clamped to a real span");
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
