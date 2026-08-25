//! The manipulator on a selection: move it, turn it, scale it.
//!
//! One widget with three modes rather than three widgets, which is what ZBrush
//! and Maya both settled on — the sculptor's hand stays in the same place and
//! the mode is what changes.
//!
//! It acts on a *selection*, which is what makes selecting more than one
//! control point worth having. Dragging points one at a time needs no
//! manipulator; turning a face of the cage does.

/// What a drag on the manipulator does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GizmoMode {
    #[default]
    Move,
    Rotate,
    Scale,
}

impl GizmoMode {
    pub const ALL: [GizmoMode; 3] = [Self::Move, Self::Rotate, Self::Scale];

    pub fn label(self) -> &'static str {
        match self {
            Self::Move => "Mover",
            Self::Rotate => "Girar",
            Self::Scale => "Escalar",
        }
    }
}

/// Which part of the manipulator was grabbed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoHandle {
    /// The outer ring, which turns about the axis facing the eye.
    ///
    /// ZBrush's outermost ring, and the one a sculptor reaches for most: the
    /// three axis rings turn the selection in the *world's* frame, and this
    /// one turns it in the frame they are looking at it from. It is the only
    /// handle whose axis is not a world axis, which is why a drag carries the
    /// direction the camera was facing when it began.
    View,
    /// One of the three axes: 0 is x, 1 is y, 2 is z.
    ///
    /// What it means depends on the mode — the shaft that slides along that
    /// axis, the ring that turns about it, or the box that scales along it.
    Axis(usize),
    /// The centre.
    ///
    /// Free in the view plane for a move, and uniform for a scale. A rotation
    /// has no centre handle: turning about the axis facing the eye is what the
    /// outer ring is for, and a second widget meaning the same thing is one
    /// more thing to hit by accident.
    Centre,
}

impl GizmoHandle {
    /// Every handle a mode offers, which is what the viewport draws and what
    /// the pointer is tested against.
    pub fn all_for(mode: GizmoMode) -> Vec<GizmoHandle> {
        let axes = (0..3).map(GizmoHandle::Axis);
        match mode {
            // The outer ring instead of a centre handle. Turning about the
            // axis facing the eye is what it is for, and a filled centre
            // meaning the same thing is one more thing to hit by accident.
            GizmoMode::Rotate => axes.chain(std::iter::once(Self::View)).collect(),
            _ => axes.chain(std::iter::once(Self::Centre)).collect(),
        }
    }

    /// The unit vector this handle works along, where the *world* gives it
    /// one.
    ///
    /// `View` has none here on purpose: its axis is where the camera was
    /// looking when the drag began, which this type cannot know. Ask
    /// [`GizmoDrag::axis`], which does.
    pub fn axis(self) -> Option<[f32; 3]> {
        match self {
            Self::Axis(index) => {
                let mut axis = [0.0; 3];
                *axis.get_mut(index)? = 1.0;
                Some(axis)
            }
            Self::Centre | Self::View => None,
        }
    }

    /// Whether this is a ring rather than a shaft or a box.
    pub fn is_ring(self, mode: GizmoMode) -> bool {
        mode == GizmoMode::Rotate && matches!(self, Self::Axis(_) | Self::View)
    }
}

/// A transform in progress, resolved from where a drag started and where it is
/// now.
///
/// Held as a value rather than accumulated, so a drag ends where the pointer
/// ends however many frames it took — the same reason a control point carries
/// an offset from rest rather than a running sum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoDrag {
    pub mode: GizmoMode,
    pub handle: GizmoHandle,
    /// The point the transform is about: the middle of the selection.
    pub pivot: [f32; 3],
    /// Where on the drag plane the gesture started.
    pub anchor: [f32; 3],
    /// Where the camera was looking when the drag began.
    ///
    /// Captured once rather than read each frame: the outer ring turns about
    /// the axis facing the eye, and if that axis followed a camera that moved
    /// mid-drag the selection would twist under a hand that had not moved.
    pub view_axis: [f32; 3],
}

impl GizmoDrag {
    /// Where a point ends up, given where the pointer is now.
    ///
    /// Pure arithmetic on purpose: this is the whole of what the manipulator
    /// means, and it is the part worth checking without a viewport.
    ///
    /// `snap` is read *now* rather than captured when the drag began, so the
    /// modifier can be taken up and let go part-way through one gesture —
    /// which is how Blender's works and what a hand reaching for a round
    /// number actually does.
    pub fn apply(self, point: [f32; 3], to: [f32; 3], snap: bool) -> [f32; 3] {
        match self.mode {
            GizmoMode::Move => self.moved(point, to),
            GizmoMode::Rotate => self.turned(point, to, snap),
            GizmoMode::Scale => self.scaled(point, to),
        }
    }

    /// The axis this drag works about, world or view.
    pub fn axis(self) -> Option<[f32; 3]> {
        match self.handle {
            GizmoHandle::View => Some(normalize(self.view_axis)?),
            other => other.axis(),
        }
    }

    fn moved(self, point: [f32; 3], to: [f32; 3]) -> [f32; 3] {
        let travel: [f32; 3] = std::array::from_fn(|i| to[i] - self.anchor[i]);
        // Constrained to the shaft that was grabbed, which is the whole
        // difference between an axis handle and the centre: a person pulling
        // the green arrow means "up", not "up and a little sideways because my
        // hand drifted".
        let along = match self.axis() {
            Some(axis) => {
                let amount: f32 = (0..3).map(|i| travel[i] * axis[i]).sum();
                std::array::from_fn(|i| axis[i] * amount)
            }
            None => travel,
        };
        std::array::from_fn(|i| point[i] + along[i])
    }

    fn turned(self, point: [f32; 3], to: [f32; 3], snap: bool) -> [f32; 3] {
        let Some(axis) = self.axis() else {
            return point;
        };
        let angle = self.angle(axis, to);
        let angle = if snap { snapped(angle) } else { angle };
        rotate_about(point, self.pivot, axis, angle)
    }

    /// The signed angle from where the drag started to where it is now, about
    /// the axis, measured in the plane the ring lies in.
    fn angle(self, axis: [f32; 3], to: [f32; 3]) -> f32 {
        let from = flatten(sub(self.anchor, self.pivot), axis);
        let now = flatten(sub(to, self.pivot), axis);
        let (a, b) = (length(from), length(now));
        if a < 1e-6 || b < 1e-6 {
            // The drag started on the axis itself, where "which way round" has
            // no answer. Zero rather than a guess: a manipulator that spun
            // when grabbed at its centre would be unusable.
            return 0.0;
        }
        let cross = [
            from[1] * now[2] - from[2] * now[1],
            from[2] * now[0] - from[0] * now[2],
            from[0] * now[1] - from[1] * now[0],
        ];
        let sine: f32 = (0..3).map(|i| cross[i] * axis[i]).sum();
        let cosine: f32 = (0..3).map(|i| from[i] * now[i]).sum();
        sine.atan2(cosine)
    }

    fn scaled(self, point: [f32; 3], to: [f32; 3]) -> [f32; 3] {
        let from = sub(self.anchor, self.pivot);
        let now = sub(to, self.pivot);
        let factor = match self.axis() {
            Some(axis) => {
                // Measured along the axis alone, so pulling the red box out
                // stretches in x and leaves y and z where they were.
                let was: f32 = (0..3).map(|i| from[i] * axis[i]).sum();
                let is: f32 = (0..3).map(|i| now[i] * axis[i]).sum();
                if was.abs() < 1e-6 {
                    1.0
                } else {
                    is / was
                }
            }
            None => {
                let was = length(from);
                if was < 1e-6 {
                    1.0
                } else {
                    length(now) / was
                }
            }
        };
        // Never through zero. A scale that could pass through it turns the
        // form inside out on a drag that overshot, and there is no way back
        // but undo.
        let factor = factor.clamp(MIN_SCALE, MAX_SCALE);
        let offset = sub(point, self.pivot);
        match self.axis() {
            Some(axis) => std::array::from_fn(|i| {
                // Only the component along the axis is scaled.
                let along: f32 = (0..3).map(|k| offset[k] * axis[k]).sum();
                point[i] + axis[i] * along * (factor - 1.0)
            }),
            None => std::array::from_fn(|i| self.pivot[i] + offset[i] * factor),
        }
    }
}

/// The increment a snapped rotation lands on.
///
/// Fifteen degrees: twenty-four to the turn, and it divides the angles a
/// sculptor actually reaches for — 30, 45, 60, 90 — which a rounder-looking
/// 10 does not.
pub const SNAP_DEGREES: f32 = 15.0;

/// Rounds an angle to the nearest [`SNAP_DEGREES`].
///
/// To the *nearest* rather than downward, so the handle stays under the
/// pointer as it crosses a boundary instead of lagging half an increment
/// behind it.
pub fn snapped(angle: f32) -> f32 {
    let step = SNAP_DEGREES.to_radians();
    (angle / step).round() * step
}

/// How far a scale may be dragged, either way.
///
/// A ceiling as well as a floor: a drag that started very near the pivot has a
/// tiny denominator, and without this an ordinary pull produces a factor in
/// the thousands.
pub const MIN_SCALE: f32 = 0.01;
pub const MAX_SCALE: f32 = 100.0;

/// A unit vector, or `None` where there is no direction to have.
fn normalize(v: [f32; 3]) -> Option<[f32; 3]> {
    let length = length(v);
    (length > 1e-6).then(|| std::array::from_fn(|i| v[i] / length))
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

fn length(v: [f32; 3]) -> f32 {
    v.iter().map(|c| c * c).sum::<f32>().sqrt()
}

/// The part of a vector lying in the plane perpendicular to an axis.
fn flatten(v: [f32; 3], axis: [f32; 3]) -> [f32; 3] {
    let along: f32 = (0..3).map(|i| v[i] * axis[i]).sum();
    std::array::from_fn(|i| v[i] - axis[i] * along)
}

/// Rodrigues' rotation, about an axis through a pivot.
fn rotate_about(point: [f32; 3], pivot: [f32; 3], axis: [f32; 3], angle: f32) -> [f32; 3] {
    let v = sub(point, pivot);
    let (sin, cos) = angle.sin_cos();
    let dot: f32 = (0..3).map(|i| v[i] * axis[i]).sum();
    let cross = [
        axis[1] * v[2] - axis[2] * v[1],
        axis[2] * v[0] - axis[0] * v[2],
        axis[0] * v[1] - axis[1] * v[0],
    ];
    std::array::from_fn(|i| pivot[i] + v[i] * cos + cross[i] * sin + axis[i] * dot * (1.0 - cos))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
    }

    fn drag(mode: GizmoMode, handle: GizmoHandle, anchor: [f32; 3]) -> GizmoDrag {
        GizmoDrag {
            mode,
            handle,
            pivot: [0.0; 3],
            anchor,
            // Looking down −z, which is where the default camera is. Only the
            // outer ring reads it.
            view_axis: [0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn an_axis_move_stays_on_its_axis() {
        // The whole difference between an arrow and the centre handle: a
        // person pulling the green arrow means "up", not "up and a little
        // sideways because my hand drifted".
        let drag = drag(GizmoMode::Move, GizmoHandle::Axis(1), [0.0, 0.0, 0.0]);
        let moved = drag.apply([1.0, 2.0, 3.0], [0.7, 0.5, -0.2], false);
        assert!(
            close(moved, [1.0, 2.5, 3.0]),
            "an axis move drifted off its axis: {moved:?}"
        );
    }

    #[test]
    fn the_centre_moves_freely() {
        let drag = drag(GizmoMode::Move, GizmoHandle::Centre, [0.0; 3]);
        let moved = drag.apply([1.0, 2.0, 3.0], [0.7, 0.5, -0.2], false);
        assert!(close(moved, [1.7, 2.5, 2.8]), "{moved:?}");
    }

    #[test]
    fn a_quarter_turn_is_a_quarter_turn() {
        // Dragging from +x to +y about z, with the pivot at the origin.
        let drag = drag(GizmoMode::Rotate, GizmoHandle::Axis(2), [1.0, 0.0, 0.0]);
        let turned = drag.apply([2.0, 0.0, 0.0], [0.0, 1.0, 0.0], false);
        assert!(
            close(turned, [0.0, 2.0, 0.0]),
            "a quarter turn about z put (2, 0, 0) at {turned:?}"
        );
        // And the axis itself does not move, which is what "about" means.
        assert!(close(
            drag.apply([0.0, 0.0, 5.0], [0.0, 1.0, 0.0], false),
            [0.0, 0.0, 5.0]
        ));
    }

    #[test]
    fn a_rotation_turns_about_the_pivot_and_not_the_origin() {
        let drag = GizmoDrag {
            mode: GizmoMode::Rotate,
            handle: GizmoHandle::Axis(2),
            pivot: [10.0, 0.0, 0.0],
            anchor: [11.0, 0.0, 0.0],
            view_axis: [0.0, 0.0, 1.0],
        };
        let turned = drag.apply([12.0, 0.0, 0.0], [10.0, 1.0, 0.0], false);
        assert!(
            close(turned, [10.0, 2.0, 0.0]),
            "the turn was about the origin rather than the pivot: {turned:?}"
        );
    }

    #[test]
    fn grabbing_a_ring_on_its_own_axis_does_not_spin_it() {
        // Where the drag started on the axis itself, "which way round" has no
        // answer, and a manipulator that spun when grabbed at its centre would
        // be unusable.
        let drag = drag(GizmoMode::Rotate, GizmoHandle::Axis(2), [0.0, 0.0, 3.0]);
        let point = [1.0, 0.0, 0.0];
        assert!(close(drag.apply(point, [0.0, 1.0, 0.0], false), point));
    }

    #[test]
    fn an_axis_scale_stretches_one_axis_only() {
        // Anchor one unit out along x, dragged to two: a factor of two on x
        // and nothing at all on y or z.
        let drag = drag(GizmoMode::Scale, GizmoHandle::Axis(0), [1.0, 0.0, 0.0]);
        let scaled = drag.apply([3.0, 5.0, 7.0], [2.0, 0.0, 0.0], false);
        assert!(
            close(scaled, [6.0, 5.0, 7.0]),
            "an axis scale reached the other two axes: {scaled:?}"
        );
    }

    #[test]
    fn the_centre_scales_uniformly_about_the_pivot() {
        let drag = GizmoDrag {
            mode: GizmoMode::Scale,
            handle: GizmoHandle::Centre,
            pivot: [1.0, 1.0, 1.0],
            anchor: [2.0, 1.0, 1.0],
            view_axis: [0.0, 0.0, 1.0],
        };
        let scaled = drag.apply([3.0, 1.0, 1.0], [3.0, 1.0, 1.0], false);
        assert!(
            close(scaled, [5.0, 1.0, 1.0]),
            "a uniform scale of two about (1,1,1) put (3,1,1) at {scaled:?}"
        );
    }

    #[test]
    fn a_scale_never_passes_through_zero() {
        // A drag that overshot the pivot would turn the form inside out, and
        // there is no way back but undo.
        let overshoot = drag(GizmoMode::Scale, GizmoHandle::Axis(0), [1.0, 0.0, 0.0]);
        let scaled = overshoot.apply([2.0, 0.0, 0.0], [-5.0, 0.0, 0.0], false);
        assert!(
            scaled[0] > 0.0,
            "an overshot scale flipped the point to {scaled:?}"
        );

        // And a drag that started almost on the pivot has a tiny denominator,
        // which without a ceiling turns an ordinary pull into a factor in the
        // thousands.
        let hair = drag(GizmoMode::Scale, GizmoHandle::Axis(0), [1e-7, 0.0, 0.0]);
        let scaled = hair.apply([1.0, 0.0, 0.0], [1.0, 0.0, 0.0], false);
        assert!(scaled[0].abs() <= 1.0 + MAX_SCALE, "{scaled:?}");
    }

    #[test]
    fn a_drag_that_has_not_moved_changes_nothing() {
        // Every mode: a press is not an edit, and a manipulator that nudged
        // the selection on being touched could not be used to inspect one.
        let point = [1.0, 2.0, 3.0];
        let anchor = [4.0, 0.0, 0.0];
        for mode in GizmoMode::ALL {
            for handle in GizmoHandle::all_for(mode) {
                let untouched = drag(mode, handle, anchor);
                let after = untouched.apply(point, anchor, false);
                assert!(
                    close(after, point),
                    "{mode:?} on {handle:?} moved {point:?} to {after:?} with \
                     no drag at all"
                );
            }
        }
    }

    #[test]
    fn a_rotation_offers_an_outer_ring_and_no_centre_handle() {
        // Turning about the axis facing the eye is what the outer ring is for,
        // and a filled centre meaning the same thing is one more thing to hit
        // by accident.
        let rotate = GizmoHandle::all_for(GizmoMode::Rotate);
        assert_eq!(rotate.len(), 4, "three axis rings and the outer one");
        assert!(
            rotate.contains(&GizmoHandle::View),
            "no outer ring is offered"
        );
        assert!(!rotate.contains(&GizmoHandle::Centre));
        for mode in [GizmoMode::Move, GizmoMode::Scale] {
            let handles = GizmoHandle::all_for(mode);
            assert!(handles.contains(&GizmoHandle::Centre));
            assert!(
                !handles.contains(&GizmoHandle::View),
                "{mode:?} offered a ring that only a rotation has"
            );
        }
    }

    #[test]
    fn the_outer_ring_turns_about_the_axis_the_camera_faces() {
        // The three axis rings turn the selection in the world's frame; this
        // one turns it in the frame the sculptor is looking at it from.
        let drag = GizmoDrag {
            mode: GizmoMode::Rotate,
            handle: GizmoHandle::View,
            pivot: [0.0; 3],
            anchor: [1.0, 0.0, 0.0],
            // Looking down y, which is no world axis the rings offer.
            view_axis: [0.0, 1.0, 0.0],
        };
        // A quarter turn in the plane the ring lies in: +x to −z about +y.
        let turned = drag.apply([2.0, 0.0, 0.0], [0.0, 0.0, -1.0], false);
        assert!(
            close(turned, [0.0, 0.0, -2.0]),
            "the outer ring put (2,0,0) at {turned:?}"
        );
        // And the camera's own axis is what stays put.
        assert!(close(
            drag.apply([0.0, 5.0, 0.0], [0.0, 0.0, -1.0], false),
            [0.0, 5.0, 0.0]
        ));
    }

    #[test]
    fn the_outer_ring_needs_a_direction_to_turn_about() {
        // A camera axis of nothing is not a rotation of nothing about
        // something: it is no gesture at all, and the point stays put.
        let drag = GizmoDrag {
            mode: GizmoMode::Rotate,
            handle: GizmoHandle::View,
            pivot: [0.0; 3],
            anchor: [1.0, 0.0, 0.0],
            view_axis: [0.0; 3],
        };
        let point = [2.0, 0.0, 0.0];
        assert!(close(drag.apply(point, [0.0, 1.0, 0.0], false), point));
    }

    #[test]
    fn a_snapped_turn_lands_on_a_multiple_of_fifteen_degrees() {
        let drag = drag(GizmoMode::Rotate, GizmoHandle::Axis(2), [1.0, 0.0, 0.0]);
        // 50 degrees, which is nobody's round number.
        let (s, c) = 50.0f32.to_radians().sin_cos();
        let free = drag.apply([1.0, 0.0, 0.0], [c, s, 0.0], false);
        let snap = drag.apply([1.0, 0.0, 0.0], [c, s, 0.0], true);

        let degrees = |p: [f32; 3]| p[1].atan2(p[0]).to_degrees();
        assert!(
            (degrees(free) - 50.0).abs() < 0.1,
            "an unsnapped drag did not turn by what was asked: {}",
            degrees(free)
        );
        assert!(
            (degrees(snap) - 45.0).abs() < 0.1,
            "50 degrees snapped to {} rather than 45",
            degrees(snap)
        );
    }

    #[test]
    fn snapping_rounds_to_the_nearest_rather_than_downward() {
        // Downward would leave the handle lagging half an increment behind the
        // pointer as it crosses a boundary.
        for (asked, wanted) in [(7.0f32, 0.0f32), (8.0, 15.0), (22.0, 15.0), (23.0, 30.0)] {
            let got = snapped(asked.to_radians()).to_degrees();
            assert!(
                (got - wanted).abs() < 0.01,
                "{asked} degrees snapped to {got} rather than {wanted}"
            );
        }
        // Both ways round: a turn anticlockwise snaps the same as clockwise.
        assert!((snapped((-23.0f32).to_radians()).to_degrees() + 30.0).abs() < 0.01);
    }

    #[test]
    fn every_snapped_angle_is_a_whole_number_of_increments() {
        // The property, rather than a handful of cases: whatever comes in, what
        // comes out divides by the increment.
        let step = SNAP_DEGREES.to_radians();
        for hundredths in -1000..1000 {
            let angle = hundredths as f32 / 100.0;
            let remainder = (snapped(angle) / step).fract().abs();
            assert!(
                remainder < 1e-3 || (1.0 - remainder) < 1e-3,
                "{angle} snapped to something off the grid"
            );
        }
    }

    #[test]
    fn snapping_leaves_the_other_two_modes_alone() {
        // It is *angle* snapping. A move that snapped to a grid nobody asked
        // for would be a surprise, and a snapped scale is not a thing here.
        let move_drag = drag(GizmoMode::Move, GizmoHandle::Axis(1), [0.0; 3]);
        assert_eq!(
            move_drag.apply([1.0, 2.0, 3.0], [0.7, 0.5, -0.2], true),
            move_drag.apply([1.0, 2.0, 3.0], [0.7, 0.5, -0.2], false)
        );
        let scale_drag = drag(GizmoMode::Scale, GizmoHandle::Axis(0), [1.0, 0.0, 0.0]);
        assert_eq!(
            scale_drag.apply([3.0, 5.0, 7.0], [2.3, 0.0, 0.0], true),
            scale_drag.apply([3.0, 5.0, 7.0], [2.3, 0.0, 0.0], false)
        );
    }
}
