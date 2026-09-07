//! Which half a drawn cut takes, read from the direction it was drawn.
//!
//! ZBrush infers it from the stroke rather than asking, and a dialog after
//! every trim is not a sculpting tool. The rule has to answer for a line drawn
//! at any angle and for a lasso, which has no sides at all — so there are two
//! rules, and they are the same idea applied to two shapes: **the gesture's
//! direction says which half the sculptor meant to lose.**

/// Which half of the frame an **open** stroke's outline covers.
///
/// It does not decide that half's fate: the operation the resolved item is
/// placed with does. Two controls that both mean "the other half" would be the
/// second way to say one thing, which the engine's own note rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrimSide {
    #[default]
    Below,
    Above,
    Left,
    Right,
}

/// The half a stroke drawn from `first` to `last` covers, in frame
/// coordinates.
///
/// **What lies to the right of the travel is the half that goes.** Drawn
/// left-to-right that is everything below the line, which is ZBrush's
/// behaviour and the one a hand expects; drawn right-to-left it is everything
/// above, so re-drawing the same line the other way keeps the other half
/// without touching a modifier.
///
/// One rule rather than four cases: the removed side is the travel turned a
/// quarter-turn clockwise, and the axis-aligned side nearest that direction is
/// the one the engine takes. A diagonal stroke therefore resolves to whichever
/// of the four it leans towards rather than being refused.
///
/// `None` where the stroke has no direction — a press that never travelled, or
/// one that returned exactly to where it began. A cut needs a side, so the
/// caller refuses rather than guessing.
pub fn side_of(first: [f32; 2], last: [f32; 2]) -> Option<TrimSide> {
    let travel = [last[0] - first[0], last[1] - first[1]];
    // A quarter-turn clockwise: the direction the removed half lies in.
    let removed = [travel[1], -travel[0]];
    if removed[0].abs() < f32::EPSILON && removed[1].abs() < f32::EPSILON {
        return None;
    }
    Some(if removed[0].abs() > removed[1].abs() {
        if removed[0] > 0.0 {
            TrimSide::Right
        } else {
            TrimSide::Left
        }
    } else if removed[1] > 0.0 {
        TrimSide::Above
    } else {
        TrimSide::Below
    })
}

/// Twice the signed area a closed outline encloses, in frame coordinates.
///
/// Positive is counter-clockwise. The shoelace sum, not halved, because only
/// the sign is read.
fn winding(points: &[[f32; 2]]) -> f32 {
    if points.len() < 3 {
        return 0.0;
    }
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum()
}

/// Whether a closed lasso keeps what it encloses rather than removing it.
///
/// The lasso's answer to the same question a line answers with its direction.
/// A line has two sides and takes the one to the right of the travel; a loop
/// has an inside and an outside, and **the way it was drawn says which of
/// those the sculptor meant to lose**: clockwise removes what is inside,
/// counter-clockwise keeps only what is inside.
///
/// So the gesture carries the intent in both shapes and neither needs a
/// second control. `None` for a loop with no area, which encloses nothing to
/// keep or remove.
pub fn lasso_keeps_inside(points: &[[f32; 2]]) -> Option<bool> {
    let signed = winding(points);
    (signed.abs() > f32::EPSILON).then_some(signed > 0.0)
}

/// Which shape a drawn cut is, and therefore which of the engine's two
/// outline routes it takes.
///
/// Not one gesture with a "close it" toggle. The two produce **different
/// shapes from the same points** — joining an open stroke's endpoints cuts a
/// sliver between them instead of dividing the frame — so a toggle would
/// silently change what a drawn line means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CutGesture {
    /// A line drawn across the form, closed against the frame on the side the
    /// travel points away from.
    #[default]
    Line,
    /// A loop traced over the form, closed as it was drawn.
    Lasso,
    /// A box dragged corner to corner, square to the view.
    Rectangle,
}

impl CutGesture {
    pub const ALL: [CutGesture; 3] = [Self::Line, Self::Lasso, Self::Rectangle];

    /// Whether the drawn shape closes across a gap the sculptor can see.
    ///
    /// A lasso does and a line does not: a line is closed against the frame's
    /// own bounds, far outside the view, so drawing that edge would be drawing
    /// something the sculptor never traced.
    pub fn closes_a_gap(self) -> bool {
        self == Self::Lasso
    }
}

/// A cut as the interface finished drawing it.
///
/// `track` is in the frame's own world units and in the order it was drawn —
/// the order is not decoration, it is what says which half the sculptor meant
/// to lose.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnCut {
    pub track: Vec<[f32; 2]>,
    pub frame: crate::OutlineFrame,
    pub gesture: CutGesture,
}

impl DrawnCut {
    /// Whether there is enough of a gesture to cut with.
    ///
    /// A line needs two points to have a direction; a loop needs three to
    /// enclose anything. A gesture below that is a click, and cutting nothing
    /// while reporting a cut is worse than refusing.
    pub fn is_drawn(&self) -> bool {
        match self.gesture {
            CutGesture::Line => self.track.len() >= 2 && self.side().is_some(),
            CutGesture::Lasso => self.track.len() >= 3 && self.keeps_inside().is_some(),
            CutGesture::Rectangle => {
                self.track.len() >= 2 && self.corners().is_some_and(|(w, h)| w > 0.0 && h > 0.0)
            }
        }
    }

    /// The half a line covers, from the direction it was drawn.
    pub fn side(&self) -> Option<TrimSide> {
        match (self.track.first(), self.track.last()) {
            (Some(first), Some(last)) if self.gesture == CutGesture::Line => side_of(*first, *last),
            _ => None,
        }
    }

    /// Whether a lasso keeps what it encloses, from the way it was wound.
    pub fn keeps_inside(&self) -> Option<bool> {
        (self.gesture == CutGesture::Lasso).then(|| lasso_keeps_inside(&self.track))?
    }

    /// A rectangle's half-extents and centre, from the two corners dragged.
    pub fn corners(&self) -> Option<(f32, f32)> {
        let (first, last) = (self.track.first()?, self.track.last()?);
        Some((
            (last[0] - first[0]).abs() * 0.5,
            (last[1] - first[1]).abs() * 0.5,
        ))
    }

    /// The middle of a rectangle, on the frame.
    pub fn centre(&self) -> Option<[f32; 2]> {
        let (first, last) = (self.track.first()?, self.track.last()?);
        Some([(first[0] + last[0]) * 0.5, (first[1] + last[1]) * 0.5])
    }
}

/// Placing a cut drawn over the view.
pub trait CutModel {
    /// Resolves a drawn shape into an item and places it, as one undo entry.
    ///
    /// A cut is an item rather than a bake: it can be adjusted afterwards by
    /// the same route every other placed item is, and resolving it is a choice
    /// a sculptor makes rather than one a tool makes for them.
    fn apply_cut(&mut self, cut: &DrawnCut) -> Result<(), crate::ModelError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LayerState, Representation, ToolKind, Unavailable};

    /// The shelf shows Trim on a field and nowhere else, and it is the table
    /// that says so.
    ///
    /// Asked of `for_representation`, which is what the shelf itself calls,
    /// rather than of a list written here: a row added to `verbs` for a
    /// representation with no binding behind it fails on that row instead of
    /// shipping a tool that greets a sculptor by refusing.
    ///
    /// Proven rather than argued. "It reads from the table, so it must follow
    /// the table" is a claim about how the code is written; "it fails when the
    /// table changes" is a claim about what it does, and only the second is
    /// worth anything. Giving `Trim` a mesh verb fails this and
    /// `asking_for_a_cut_on_anything_else_names_the_representation` together —
    /// *the shelf offers Trim on Mesh* and *a cut on Mesh was not refused by
    /// the table*.
    #[test]
    fn trim_is_offered_on_a_field_and_greyed_out_everywhere_else() {
        for representation in Representation::ALL {
            let offered = ToolKind::for_representation(representation).contains(&ToolKind::Trim);
            assert_eq!(
                offered,
                representation == Representation::Sdf,
                "the shelf offers Trim on {representation:?}, and the cut \
                 resolves to a field item"
            );
        }
    }

    /// And a caller that asks anyway is told which representation is in the
    /// way, rather than meeting a silent no-op.
    ///
    /// Trimming a mesh would mean crossing into a field and back — a
    /// representation change with its own cost and its own undo entry — and a
    /// tool that says it removes material must not perform one silently.
    #[test]
    fn asking_for_a_cut_on_anything_else_names_the_representation() {
        for representation in Representation::ALL
            .into_iter()
            .filter(|it| *it != Representation::Sdf)
        {
            let refused = ToolKind::Trim.availability(LayerState::editable(representation));
            assert!(
                matches!(
                    refused,
                    Err(Unavailable::NoVerbHere { active, .. }) if active == representation
                ),
                "a cut on {representation:?} was not refused by the table"
            );
        }
    }

    /// The behaviour a hand expects, and the one ZBrush has.
    #[test]
    fn a_line_drawn_left_to_right_takes_what_is_below_it() {
        assert_eq!(side_of([-1.0, 0.0], [1.0, 0.0]), Some(TrimSide::Below));
    }

    /// And re-drawing it the other way keeps the other half, with no modifier
    /// touched — which is the whole reason the direction is read at all.
    #[test]
    fn the_same_line_drawn_back_takes_the_other_half() {
        assert_eq!(side_of([1.0, 0.0], [-1.0, 0.0]), Some(TrimSide::Above));
    }

    #[test]
    fn a_vertical_stroke_takes_a_vertical_half() {
        assert_eq!(side_of([0.0, -1.0], [0.0, 1.0]), Some(TrimSide::Right));
        assert_eq!(side_of([0.0, 1.0], [0.0, -1.0]), Some(TrimSide::Left));
    }

    /// A diagonal leans rather than being refused: a sculptor who draws at
    /// forty degrees meant one of the four, not an error.
    #[test]
    fn a_diagonal_leans_to_the_side_it_is_nearest() {
        // Mostly rightward, tilted up: still below.
        assert_eq!(side_of([-1.0, 0.0], [1.0, 0.4]), Some(TrimSide::Below));
        // Mostly upward, tilted right: to the right.
        assert_eq!(side_of([0.0, -1.0], [0.4, 1.0]), Some(TrimSide::Right));
    }

    /// A press that never travelled has no direction, and a cut needs a side.
    #[test]
    fn a_stroke_that_went_nowhere_names_no_side() {
        assert_eq!(side_of([0.5, 0.5], [0.5, 0.5]), None);
    }

    /// The lasso's version of the same question.
    #[test]
    fn a_lasso_drawn_clockwise_removes_what_it_encloses() {
        // Clockwise in a y-up frame.
        let loop_ = [[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]];
        assert_eq!(lasso_keeps_inside(&loop_), Some(false));
    }

    #[test]
    fn a_lasso_drawn_the_other_way_keeps_only_what_it_encloses() {
        let loop_ = [[0.0, 1.0], [-1.0, 0.0], [0.0, -1.0], [1.0, 0.0]];
        assert_eq!(lasso_keeps_inside(&loop_), Some(true));
    }

    #[test]
    fn a_loop_with_no_area_encloses_nothing() {
        assert_eq!(lasso_keeps_inside(&[[0.0, 0.0], [1.0, 0.0]]), None);
        assert_eq!(
            lasso_keeps_inside(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]),
            None
        );
    }
}
