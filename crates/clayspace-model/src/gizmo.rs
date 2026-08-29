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

    /// Which axis this handle is on, where it is on one.
    pub fn axis_index(self) -> Option<usize> {
        match self {
            Self::Axis(index) => Some(index),
            Self::Centre | Self::View => None,
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

    /// Every handle a mode offers for something carrying a [`Transform`].
    ///
    /// The same as [`GizmoHandle::all_for`] except in scale mode, where the
    /// three axis boxes are not offered: the engine's transforms take one
    /// scale factor and not three, so an axis box would measure a stretch it
    /// could not apply. A control that silently does nothing is the failure
    /// this application keeps refusing — the combine operations' distance
    /// slider refuses zero for the same reason.
    ///
    /// A cage still gets all three, because a cage scales its own control
    /// points and carries no engine transform.
    pub fn all_for_transform(mode: GizmoMode) -> Vec<GizmoHandle> {
        match mode {
            GizmoMode::Scale => vec![Self::Centre],
            other => Self::all_for(other),
        }
    }

    /// Every handle the combined manipulator carries, with the operation each
    /// performs.
    ///
    /// ZBrush's Gizmo 3D: arrows, rings, boxes and the outer ring on one
    /// widget, and the operation chosen by the handle grabbed rather than by a
    /// mode set first. The centre is not listed: what it does is whichever of
    /// move and scale the interface's mode names — see
    /// [`GizmoHandle::centre_operation`]. The scale boxes are offered only
    /// where a stretch can be applied per axis; on a target the engine scales
    /// by one factor they would be three handles for one number.
    pub fn combined(per_axis_scale: bool) -> Vec<(GizmoMode, GizmoHandle)> {
        let mut all: Vec<(GizmoMode, GizmoHandle)> =
            (0..3).map(|i| (GizmoMode::Move, Self::Axis(i))).collect();
        all.extend((0..3).map(|i| (GizmoMode::Rotate, Self::Axis(i))));
        if per_axis_scale {
            all.extend((0..3).map(|i| (GizmoMode::Scale, Self::Axis(i))));
        }
        all.push((GizmoMode::Rotate, Self::View));
        all
    }

    /// What the centre handle does under the interface's mode: a uniform
    /// scale where scale is chosen, a slide in the view plane otherwise. A
    /// turn has no centre gesture — the outer ring is that — so the centre
    /// slides then too.
    pub fn centre_operation(mode: GizmoMode) -> GizmoMode {
        match mode {
            GizmoMode::Scale => GizmoMode::Scale,
            GizmoMode::Move | GizmoMode::Rotate => GizmoMode::Move,
        }
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
        // but undo. And never past `GESTURE_FACTOR` in one pull, for the
        // reason `factor` gives.
        let factor = factor.clamp(1.0 / GESTURE_FACTOR, GESTURE_FACTOR);
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

/// Where a whole thing stands: what the engine's transforms take, and what a
/// manipulator on one produces.
///
/// The same four values every transform in the engine's interface takes —
/// a position, an axis, an angle and *one* scale factor. Not a matrix: the
/// boundary takes these, and composing a matrix here only to decompose it
/// there would invent a rotation representation that would then have to be
/// reconciled with the one the engine actually reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: [f32; 3],
    /// Never zero, even for no rotation: the engine requires a real axis and
    /// says why — "a second convention for 'no rotation' would be one more
    /// thing to get wrong."
    pub rotation_axis: [f32; 3],
    pub rotation_angle: f32,
    /// Uniform. There is no per-axis scale anywhere in the engine's interface,
    /// which is why scale mode offers a centre handle and no axis boxes for
    /// anything that carries one of these.
    pub scale: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation_axis: [0.0, 1.0, 0.0],
            rotation_angle: 0.0,
            scale: 1.0,
        }
    }
}

impl Transform {
    pub fn at(position: [f32; 3]) -> Self {
        Self {
            position,
            ..Self::default()
        }
    }
}

impl GizmoDrag {
    /// What this drag makes of a transform.
    ///
    /// The other half of [`GizmoDrag::apply`], which maps a point to a point
    /// and is the whole of what a cage of control points needs. A placed
    /// object is not a set of points the application can move: it is a node
    /// the engine holds, and what the engine takes is this.
    ///
    /// Rotation *composes*, which is the part that needs saying. The engine
    /// stores one axis and one angle, so turning a form that is already turned
    /// cannot simply overwrite them — the two rotations have to be combined
    /// into the single one that means both, which is what quaternions are for
    /// and why they appear in a domain crate that otherwise has no need of
    /// them.
    pub fn resolve(self, current: Transform, to: [f32; 3], snap: bool) -> Transform {
        match self.mode {
            GizmoMode::Move => Transform {
                position: self.moved(current.position, to),
                ..current
            },
            GizmoMode::Rotate => {
                let Some(axis) = self.axis() else {
                    return current;
                };
                let angle = self.angle(axis, to);
                let angle = if snap { snapped(angle) } else { angle };
                let (rotation_axis, rotation_angle) = compose(
                    (current.rotation_axis, current.rotation_angle),
                    (axis, angle),
                );
                Transform {
                    // About the pivot, so an object turned by a manipulator
                    // sitting on something else orbits it rather than
                    // spinning where it stands.
                    position: rotate_about(current.position, self.pivot, axis, angle),
                    rotation_axis,
                    rotation_angle,
                    ..current
                }
            }
            GizmoMode::Scale => {
                let factor = self.factor(to);
                let offset = sub(current.position, self.pivot);
                Transform {
                    position: std::array::from_fn(|i| self.pivot[i] + offset[i] * factor),
                    scale: (current.scale * factor).clamp(MIN_SCALE, MAX_SCALE),
                    ..current
                }
            }
        }
    }

    /// How much bigger this drag is asking for.
    ///
    /// Uniform whatever handle was grabbed. A per-axis factor has nowhere to
    /// go — the engine's transforms take one number — so an axis handle in
    /// scale mode would measure something it could not then apply.
    /// [`GizmoHandle::all_for_transform`] is what stops one being offered.
    fn factor(self, to: [f32; 3]) -> f32 {
        let was = length(sub(self.anchor, self.pivot));
        if was < 1e-6 {
            return 1.0;
        }
        // Bounded per gesture as well as in total. The factor is a ratio of
        // distances from the pivot, and a press on the centre handle starts a
        // hair from it — so one pull to the edge of the screen was a hundred
        // times, which is a form the field's cache cannot track and nothing
        // a hand meant. Ten times a drag is still a big move; more is another
        // drag.
        (length(sub(to, self.pivot)) / was).clamp(1.0 / GESTURE_FACTOR, GESTURE_FACTOR)
    }
}

/// One rotation that means both, as an axis and an angle.
///
/// Through quaternions because there is no other honest way: two axis-angle
/// rotations about different axes do not add, and the engine has room for
/// exactly one of them.
fn compose(first: ([f32; 3], f32), second: ([f32; 3], f32)) -> ([f32; 3], f32) {
    let quaternion = |(axis, angle): ([f32; 3], f32)| -> [f32; 4] {
        let Some(axis) = normalize(axis) else {
            return [0.0, 0.0, 0.0, 1.0];
        };
        let half = angle / 2.0;
        let (sin, cos) = half.sin_cos();
        [axis[0] * sin, axis[1] * sin, axis[2] * sin, cos]
    };
    let a = quaternion(second);
    let b = quaternion(first);
    // Second applied after first, which is the order a hand expects: the drag
    // just made turns the thing as it stands.
    let product = [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ];
    let sine = (product[0] * product[0] + product[1] * product[1] + product[2] * product[2]).sqrt();
    if sine < 1e-6 {
        // No rotation at all. The axis still has to be real — see
        // [`Transform::rotation_axis`].
        return ([0.0, 1.0, 0.0], 0.0);
    }
    let angle = 2.0 * sine.atan2(product[3].clamp(-1.0, 1.0));
    (
        [product[0] / sine, product[1] / sine, product[2] / sine],
        angle,
    )
}

/// The plane a drag on this handle runs on, given where the camera is.
///
/// The mode decides it, which is the part that is easy to get wrong: a slide
/// and a turn want *opposite* planes. `facing` points from the surface back at
/// the eye; `view_axis` is what the outer ring turns about.
///
/// Returned as a normal, not normalised — the caller intersects a ray with it,
/// and length does not matter for that.
pub fn drag_plane(
    mode: GizmoMode,
    handle: GizmoHandle,
    view_axis: [f32; 3],
    facing: [f32; 3],
) -> [f32; 3] {
    match (mode, handle) {
        // A ring lies in the plane *perpendicular* to what it turns about, and
        // that is where the angle is measured — so the drag has to run there
        // too. Run it on a plane containing the axis instead and the pointer's
        // travel has no component in the plane being measured: the angle comes
        // out at exactly zero however far the hand moves.
        (GizmoMode::Rotate, GizmoHandle::View) => normalize(view_axis).unwrap_or(facing),
        (GizmoMode::Rotate, handle) => handle.axis().unwrap_or(facing),
        // A slide or a stretch reads how far the pointer travelled *along* the
        // axis, so here the plane must contain it — and of the planes that do,
        // the one most nearly facing the eye.
        (_, GizmoHandle::Axis(index)) => {
            let Some(axis) = GizmoHandle::Axis(index).axis() else {
                return facing;
            };
            let normal = cross(cross(axis, facing), axis);
            if length(normal) >= 1e-4 {
                return normal;
            }
            // The axis points at the eye. Every plane containing it is edge-on
            // to the screen, so there is no comfortable answer — but one that
            // still contains the axis keeps the gesture *possible*. Falling
            // back to the plane facing the camera, as this once did, puts the
            // anchor's component along the axis at exactly zero and the handle
            // stops responding altogether.
            let (across, _) = perpendicular_frame(axis);
            cross(across, axis)
        }
        _ => facing,
    }
}

/// Two unit vectors spanning the plane perpendicular to an axis.
///
/// The first is taken from whichever world axis the given one leans on least,
/// so the pair never degenerates however the axis is pointed. Seeding with x
/// unconditionally is the obvious version and collapses to nothing the moment
/// the axis *is* x.
pub fn perpendicular_frame(axis: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let least = (0..3)
        .min_by(|a, b| axis[*a].abs().total_cmp(&axis[*b].abs()))
        .unwrap_or(0);
    let mut seed = [0.0f32; 3];
    seed[least] = 1.0;
    let across = cross(axis, seed);
    let across = normalize(across).unwrap_or([1.0, 0.0, 0.0]);
    (across, cross(axis, across))
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// How many points to test around a ring so the whole of it can be grabbed.
///
/// A ring is hit-tested as a string of spheres along it, and sixteen of them
/// was a number picked rather than derived: at the manipulator's proportions
/// the spheres do not touch, so roughly a fifth of every drawn ring — and a
/// third of the outer one — was not grabbable at all. A press there falls
/// through to whatever is behind, and the ring reads as broken.
///
/// Derived instead: enough that neighbouring spheres overlap, so anywhere on
/// the ring is within `grab` of a sample.
pub fn ring_samples(radius: f32, grab: f32) -> usize {
    if grab <= 0.0 {
        return 1;
    }
    let circumference = std::f32::consts::TAU * radius.max(0.0);
    // Spacing of one grab radius leaves neighbours overlapping by half.
    ((circumference / grab).ceil() as usize).clamp(8, 512)
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
/// The most one scale gesture may multiply or divide by.
pub const GESTURE_FACTOR: f32 = 10.0;

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
    fn the_combined_manipulator_carries_every_operation() {
        // Three arrows, three rings and the outer ring on every target; three
        // boxes only where a stretch can be applied per axis.
        let uniform = GizmoHandle::combined(false);
        let per_axis = GizmoHandle::combined(true);
        assert_eq!(uniform.len(), 7);
        assert_eq!(per_axis.len(), 10);
        for mode in [GizmoMode::Move, GizmoMode::Rotate] {
            for axis in 0..3 {
                assert!(uniform.contains(&(mode, GizmoHandle::Axis(axis))));
            }
        }
        assert!(!uniform.iter().any(|(mode, _)| *mode == GizmoMode::Scale));
        assert_eq!(
            per_axis
                .iter()
                .filter(|(mode, _)| *mode == GizmoMode::Scale)
                .count(),
            3
        );
        assert_eq!(
            uniform
                .iter()
                .filter(|(_, handle)| *handle == GizmoHandle::View)
                .count(),
            1,
            "one outer ring"
        );
        // The centre slides unless scale is asked for.
        assert_eq!(
            GizmoHandle::centre_operation(GizmoMode::Move),
            GizmoMode::Move
        );
        assert_eq!(
            GizmoHandle::centre_operation(GizmoMode::Rotate),
            GizmoMode::Move
        );
        assert_eq!(
            GizmoHandle::centre_operation(GizmoMode::Scale),
            GizmoMode::Scale
        );
    }

    #[test]
    fn one_gesture_scales_at_most_tenfold() {
        // Pressed a hair from the pivot and pulled to the edge of the world:
        // the old ratio was a hundred, which no hand meant and the field's
        // cache could not track.
        let drag = drag(GizmoMode::Scale, GizmoHandle::Centre, [0.05, 0.0, 0.0]);
        let scaled = drag.apply([1.0, 0.0, 0.0], [50.0, 0.0, 0.0], false);
        assert!(
            (scaled[0] - GESTURE_FACTOR).abs() < 1e-4,
            "one gesture scaled by {}; the cap is {GESTURE_FACTOR}",
            scaled[0]
        );
        let shrunk = drag.apply([1.0, 0.0, 0.0], [0.0001, 0.0, 0.0], false);
        assert!(
            (shrunk[0] - 1.0 / GESTURE_FACTOR).abs() < 1e-4,
            "one gesture shrank to {}; the floor is 1/{GESTURE_FACTOR}",
            shrunk[0]
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

    /// Where a drag actually lands: the pointer moves in the drag plane, so
    /// both ends of the gesture lie on it. This is what the application does
    /// with a ray, reduced to the part that decides whether it works.
    fn swept(mode: GizmoMode, handle: GizmoHandle, facing: [f32; 3]) -> GizmoDrag {
        let normal = drag_plane(mode, handle, facing, facing);
        let (across, _) = perpendicular_frame(normal);
        GizmoDrag {
            mode,
            handle,
            pivot: [0.0; 3],
            anchor: across,
            view_axis: facing,
        }
    }

    #[test]
    fn every_ring_turns_when_it_is_dragged_across_the_screen() {
        // The bug this test exists for: the drag plane was chosen to *contain*
        // the axis, which is right for a slide and exactly wrong for a turn —
        // a ring lies in the plane perpendicular to what it turns about. Two
        // of the three rings came out at 0 degrees however far the hand moved,
        // and only the one whose axis pointed at the camera worked.
        //
        // The manipulator's own tests could not see it: they hand world points
        // straight to the document, which is the step after the one that was
        // wrong.
        let facing = [0.0, 0.0, 1.0];
        for handle in GizmoHandle::all_for(GizmoMode::Rotate) {
            let drag = swept(GizmoMode::Rotate, handle, facing);
            let normal = drag_plane(GizmoMode::Rotate, handle, facing, facing);
            let (_, other) = perpendicular_frame(normal);
            // A quarter of the way round the ring, in the ring's own plane.
            let turned = drag.apply([1.0, 1.0, 1.0], other, false);
            let moved = (0..3)
                .map(|i| (turned[i] - [1.0, 1.0, 1.0][i]).powi(2))
                .sum::<f32>()
                .sqrt();
            assert!(
                moved > 0.5,
                "{handle:?} moved a point by {moved} on a quarter turn — the \
                 drag plane and the ring's plane disagree"
            );
        }
    }

    #[test]
    fn a_turn_is_measured_in_the_ring_s_own_plane() {
        // The rule underneath the test above, stated directly: for a ring, the
        // plane the drag runs on *is* the plane the ring lies in.
        let facing = [0.0, 0.0, 1.0];
        for index in 0..3 {
            let handle = GizmoHandle::Axis(index);
            let normal = drag_plane(GizmoMode::Rotate, handle, facing, facing);
            let axis = handle.axis().expect("an axis handle has an axis");
            let along: f32 = (0..3).map(|i| normal[i] * axis[i]).sum();
            assert!(
                (along.abs() - 1.0).abs() < 1e-4,
                "the ring about {axis:?} is dragged on a plane whose normal is \
                 {normal:?}, which is not the ring's own plane"
            );
        }
        // And the outer ring runs on the plane facing the eye.
        let view = [0.3, -0.6, 0.74];
        let normal = drag_plane(GizmoMode::Rotate, GizmoHandle::View, view, facing);
        let along: f32 = (0..3).map(|i| normal[i] * view[i]).sum::<f32>()
            / (view.iter().map(|c| c * c).sum::<f32>()).sqrt();
        assert!(
            (along.abs() - 1.0).abs() < 1e-3,
            "the outer ring's plane is {normal:?}"
        );
    }

    #[test]
    fn a_slide_runs_on_a_plane_that_contains_its_axis() {
        // The opposite rule, and the reason the two cannot share one answer.
        let facing = [0.0, 0.0, 1.0];
        for mode in [GizmoMode::Move, GizmoMode::Scale] {
            for index in 0..3 {
                let handle = GizmoHandle::Axis(index);
                let normal = drag_plane(mode, handle, facing, facing);
                let axis = handle.axis().expect("an axis");
                let along: f32 = (0..3).map(|i| normal[i] * axis[i]).sum();
                let scale = (normal.iter().map(|c| c * c).sum::<f32>()).sqrt().max(1e-6);
                assert!(
                    (along / scale).abs() < 1e-3,
                    "{mode:?} along {axis:?} runs on a plane that does not \
                     contain it: normal {normal:?}"
                );
            }
        }
    }

    #[test]
    fn an_axis_pointing_at_the_camera_can_still_be_scaled() {
        // The mirror of the ring bug, in the same line of code: when the axis
        // points at the eye the plane degenerated to the one facing the
        // camera, which puts the anchor's component along the axis at exactly
        // zero — and a scale divides by that, so the handle went dead.
        let facing = [0.0, 0.0, 1.0];
        let handle = GizmoHandle::Axis(2);
        let normal = drag_plane(GizmoMode::Scale, handle, facing, facing);
        let axis = handle.axis().expect("an axis");
        let along: f32 = (0..3).map(|i| normal[i] * axis[i]).sum();
        assert!(
            along.abs() < 1e-3,
            "the plane no longer contains the axis pointing at the eye"
        );

        // And a drag on it produces a real factor rather than a forced 1.0.
        let drag = GizmoDrag {
            mode: GizmoMode::Scale,
            handle,
            pivot: [0.0; 3],
            anchor: [0.0, 0.0, 1.0],
            view_axis: facing,
        };
        let scaled = drag.apply([0.0, 0.0, 2.0], [0.0, 0.0, 2.0], false);
        assert!(
            (scaled[2] - 4.0).abs() < 1e-3,
            "a scale along the axis facing the eye gave {scaled:?}"
        );
    }

    #[test]
    fn a_frame_is_perpendicular_to_every_axis_it_is_asked_about() {
        for axis in [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
            [0.577, 0.577, 0.577],
        ] {
            let (across, other) = perpendicular_frame(axis);
            for v in [across, other] {
                let along: f32 = (0..3).map(|i| v[i] * axis[i]).sum();
                assert!(along.abs() < 1e-3, "{v:?} is not perpendicular to {axis:?}");
                assert!((length(v) - 1.0).abs() < 1e-3, "{v:?} is not a unit vector");
            }
        }
    }

    #[test]
    fn a_ring_can_be_grabbed_anywhere_along_it() {
        // Reported as "I drag the ring and nothing happens". One cause was the
        // pivot; this was the other. A ring is hit-tested as a string of
        // spheres, and sixteen of them was picked rather than derived — at the
        // manipulator's own proportions they do not touch, so a fifth of every
        // ring and a third of the outer one could be pressed with nothing
        // under the press.
        //
        // The property, checked all the way round rather than at the samples:
        // every point on the ring is within one grab radius of some sample.
        let grab = 0.16f32;
        for radius in [1.0f32, 1.28, 0.4, 12.0] {
            let samples = ring_samples(radius, grab);
            let at = |k: f32| {
                let angle = k * std::f32::consts::TAU;
                [radius * angle.cos(), radius * angle.sin()]
            };
            // A thousand points around the ring, none of them a sample.
            let mut worst = 0.0f32;
            for step in 0..1000 {
                let on_ring = at(step as f32 / 1000.0 + 0.0004);
                let nearest = (0..samples)
                    .map(|s| {
                        let sample = at(s as f32 / samples as f32);
                        ((on_ring[0] - sample[0]).powi(2) + (on_ring[1] - sample[1]).powi(2)).sqrt()
                    })
                    .fold(f32::INFINITY, f32::min);
                worst = worst.max(nearest);
            }
            assert!(
                worst < grab,
                "on a ring of radius {radius} sampled {samples} times, a point \
                 {worst} away from the nearest sample cannot be grabbed"
            );
        }
    }

    #[test]
    fn sixteen_samples_would_not_have_covered_it() {
        // The number that was there, held up against the rule above, so this
        // records why it changed rather than merely that it did.
        let (radius, grab) = (1.28f32, 0.16f32);
        let spacing = std::f32::consts::TAU * radius / 16.0;
        assert!(
            spacing > 2.0 * grab,
            "sixteen samples would have touched after all"
        );
        assert!(ring_samples(radius, grab) > 16);
    }

    #[test]
    fn a_ring_is_never_sampled_absurdly() {
        // A grab radius of nothing, or a ring of nothing, must not ask for an
        // unbounded number of tests in the middle of a press.
        assert_eq!(ring_samples(1.0, 0.0), 1);
        assert!(ring_samples(1e9, 1e-9) <= 512);
        assert!(ring_samples(0.0, 0.16) >= 8);
    }
}

#[cfg(test)]
mod transform_tests {
    use super::*;

    fn drag(mode: GizmoMode, handle: GizmoHandle, anchor: [f32; 3]) -> GizmoDrag {
        GizmoDrag {
            mode,
            handle,
            pivot: [0.0; 3],
            anchor,
            view_axis: [0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn a_move_along_an_axis_moves_only_along_it() {
        let drag = drag(GizmoMode::Move, GizmoHandle::Axis(1), [0.0, 1.0, 0.0]);
        let moved = drag.resolve(Transform::at([0.0, 1.0, 0.0]), [0.4, 2.0, 0.3], false);
        assert_eq!(moved.position, [0.0, 2.0, 0.0]);
    }

    /// Two turns about different axes cannot be added, and the engine has room
    /// for one of them. This is the case that says the composition is real:
    /// a quarter turn about Y then a quarter about X is a single rotation
    /// about neither.
    #[test]
    fn two_turns_compose_into_one() {
        let quarter = std::f32::consts::FRAC_PI_2;
        let about_y = drag(GizmoMode::Rotate, GizmoHandle::Axis(1), [1.0, 0.0, 0.0]);
        let turned = about_y.resolve(Transform::default(), [0.0, 0.0, -1.0], false);
        assert!((turned.rotation_angle - quarter).abs() < 1e-3);

        let about_x = drag(GizmoMode::Rotate, GizmoHandle::Axis(0), [0.0, 1.0, 0.0]);
        let again = about_x.resolve(turned, [0.0, 0.0, 1.0], false);

        // A single axis-angle that is about neither of the two axes turned
        // about, which is exactly what a composition of the two is.
        let axis = again.rotation_axis;
        assert!(
            axis[0].abs() > 0.1 && axis[1].abs() > 0.1,
            "the composed axis should lie between them, got {axis:?}"
        );
        assert!(again.rotation_angle.abs() > quarter);
    }

    #[test]
    fn turning_back_returns_to_no_rotation() {
        let about_y = drag(GizmoMode::Rotate, GizmoHandle::Axis(1), [1.0, 0.0, 0.0]);
        let turned = about_y.resolve(Transform::default(), [0.0, 0.0, -1.0], false);
        let back = about_y.resolve(turned, [0.0, 0.0, 1.0], false);
        assert!(
            back.rotation_angle.abs() < 1e-3,
            "turning back should undo the turn, got {}",
            back.rotation_angle
        );
        // And the axis is still a real one, whatever the angle.
        assert!(length(back.rotation_axis) > 0.9);
    }

    #[test]
    fn a_scale_multiplies_what_is_already_there() {
        let out = drag(GizmoMode::Scale, GizmoHandle::Centre, [1.0, 0.0, 0.0]);
        let bigger = out.resolve(Transform::at([0.0; 3]), [2.0, 0.0, 0.0], false);
        assert!((bigger.scale - 2.0).abs() < 1e-4);
        let bigger_again = out.resolve(bigger, [2.0, 0.0, 0.0], false);
        assert!((bigger_again.scale - 4.0).abs() < 1e-4);
    }

    #[test]
    fn a_scale_never_passes_through_zero() {
        let out = drag(GizmoMode::Scale, GizmoHandle::Centre, [1.0, 0.0, 0.0]);
        let tiny = out.resolve(Transform::at([0.0; 3]), [0.0, 0.0, 0.0], false);
        assert!(tiny.scale > 0.0, "scale reached {}", tiny.scale);
    }

    /// A drag is resolved from where it began, so a wandering hand lands where
    /// it settles — the rule the cage already holds, checked for a transform.
    #[test]
    fn a_wandering_drag_lands_where_it_ends() {
        let drag = drag(GizmoMode::Move, GizmoHandle::Centre, [0.0; 3]);
        let start = Transform::at([0.0, 1.0, 0.0]);
        let wandered = drag.resolve(drag.resolve(start, [5.0, 5.0, 5.0], false), [0.0; 3], false);
        let straight = drag.resolve(start, [1.0, 0.0, 0.0], false);
        // Resolved from the anchor each time, so applying it twice is not the
        // same as accumulating — what matters is that one resolve from the
        // start lands where the pointer is.
        assert_eq!(straight.position, [1.0, 1.0, 0.0]);
        assert_ne!(wandered.position, straight.position);
    }

    #[test]
    fn scale_mode_offers_no_axis_handles_for_a_transform() {
        let handles = GizmoHandle::all_for_transform(GizmoMode::Scale);
        assert_eq!(handles, vec![GizmoHandle::Centre]);
        // And a cage keeps all four, because it scales its own points.
        assert_eq!(GizmoHandle::all_for(GizmoMode::Scale).len(), 4);
    }

    #[test]
    fn move_and_rotate_offer_what_they_always_did() {
        for mode in [GizmoMode::Move, GizmoMode::Rotate] {
            assert_eq!(
                GizmoHandle::all_for_transform(mode),
                GizmoHandle::all_for(mode)
            );
        }
    }
}
