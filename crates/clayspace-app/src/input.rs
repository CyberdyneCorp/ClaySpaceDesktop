//! What the pointer did over the viewport, for one frame.
//!
//! This lives in the library rather than the binary for one reason: the bug it
//! exists to prevent was in the binary, where nothing could reach it.

/// One frame's pointer input over the viewport, as egui saw it.
///
/// Read from the viewport's own [`egui::Response`] rather than from winit.
/// egui owns the hit test — it knows the panels are in front and where the
/// viewport bar ends — and the first version of this asked winit instead,
/// gating on `EventResponse::consumed`. That flag is `wants_pointer_input()`,
/// which is true wherever the pointer is over an egui area, and a
/// `CentralPanel` *is* an area covering the whole viewport. Every press,
/// drag and wheel event in the window was therefore dropped: no sculpting,
/// no orbit, no zoom, while the panels and the hover ring carried on working.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ViewportInput {
    /// Where the pointer is, in egui points.
    pub pointer: Option<egui::Pos2>,
    /// Whether it is over the viewport rather than a panel or the bar.
    pub over_viewport: bool,
    /// A button that went down this frame.
    pub pressed: Option<egui::PointerButton>,
    /// Whether any button came up, wherever it came up.
    ///
    /// Ungated on purpose: a release that happened to land over a panel must
    /// still finish the stroke, or the next move keeps sculpting.
    pub released: bool,
    /// How far the pointer moved this frame.
    pub delta: egui::Vec2,
    /// Wheel or trackpad scroll, in **notches**.
    ///
    /// egui reports scrolling in points, and one wheel notch is forty of them
    /// — a number chosen for scrolling a document, not for driving a camera.
    /// Handed on raw it made a single notch ask the camera to move forty times
    /// what one notch was meant to move, which inward is a negative distance
    /// caught by a clamp and outward is five times further away. Divided by
    /// what egui itself says a line is worth, so a wheel gives about one and a
    /// trackpad gives the fraction it actually moved.
    pub scroll: f32,
    /// Whether the modifier that forces orbiting is held.
    ///
    /// While rigging this same key means "move this sphere rather than grow a
    /// new one" — over a sphere it moves, over empty space it orbits, so one
    /// key covers both without either meaning being taken away.
    pub orbit_modifier: bool,
    /// Whether the platform's command modifier is held.
    pub command_modifier: bool,
    /// Whether Shift is held: smooth instead, whatever tool is selected.
    pub smooth_modifier: bool,
    /// Whether Control is held: take material away rather than put it there.
    ///
    /// Control and not Alt, which both references would spell differently and
    /// this application cannot. Alt already forces the drag to orbit — ZBrush's
    /// own rule, and the one that leaves a trackpad able to turn the model —
    /// so ZBrush's Alt-to-invert has nowhere to go. Blender spells invert
    /// Control, and Control is free here while a stroke is being made.
    pub invert_modifier: bool,
}

impl ViewportInput {
    /// Reads the frame's input for an allocated viewport region.
    pub fn read(ui: &egui::Ui, response: &egui::Response) -> Self {
        // Read *before* the input closure. `Context::input` takes the write
        // lock and `Context::options` takes the read lock, both on the same
        // `RwLock` — so asking egui for its options from inside `input()`
        // deadlocks outright, and the application freezes on the first frame
        // that reads the pointer. Hoisting it is not a tidy-up; it is the fix.
        let points_per_notch = ui.ctx().options(|options| options.line_scroll_speed);
        ui.input(|i| Self {
            pointer: i.pointer.latest_pos(),
            over_viewport: response.contains_pointer(),
            pressed: [
                egui::PointerButton::Primary,
                egui::PointerButton::Secondary,
                egui::PointerButton::Middle,
            ]
            .into_iter()
            .find(|button| i.pointer.button_pressed(*button)),
            released: [
                egui::PointerButton::Primary,
                egui::PointerButton::Secondary,
                egui::PointerButton::Middle,
            ]
            .into_iter()
            .any(|button| i.pointer.button_released(button)),
            delta: i.pointer.delta(),
            scroll: notches(points_per_notch, i.smooth_scroll_delta.y),
            // Option on a Mac, Alt elsewhere: the trackpad has no second
            // button worth reaching for.
            orbit_modifier: i.modifiers.alt,
            command_modifier: i.modifiers.command,
            smooth_modifier: i.modifiers.shift,
            // `ctrl` rather than `command`: on macOS the platform modifier is
            // Command, which belongs to the menus, and Control is the free one
            // — the same key Blender uses.
            invert_modifier: i.modifiers.ctrl,
        })
    }
}

/// Scroll in points, as the number of wheel notches it stands for.
///
/// The divisor is egui's own `line_scroll_speed` rather than the forty it
/// happens to be, so the wheel keeps meaning one notch if egui ever revises
/// the number or the user changes it. A trackpad reports points directly and
/// comes through as the fraction of a notch it actually moved, which is what
/// makes a two-finger drag continuous rather than stepped.
///
/// Takes the figure rather than the context on purpose: a version of this that
/// reached for `ui.ctx().options(..)` itself could be called from inside
/// `ui.input(..)`, which deadlocks — and it was, and it did. Passing the number
/// in makes that impossible to write.
fn notches(points_per_notch: f32, points: f32) -> f32 {
    if points_per_notch <= f32::EPSILON {
        return 0.0;
    }
    points / points_per_notch
}

/// How wide a manipulator handle's grab radius is, against its reach.
///
/// Larger than what is drawn on purpose: a handle a person can see and cannot
/// hit is worse than one drawn a little small.
pub const GIZMO_GRAB: f32 = 0.16;

/// The unit vector from a point to the camera.
///
/// Which is what "the axis facing the eye" means, and what both the outer
/// ring's drawing and its rotation are built on — one vector rather than two
/// that could disagree by a fraction of a degree.
pub fn toward_eye(camera: &clayspace_view::Camera, from: [f32; 3]) -> [f32; 3] {
    let eye: [f32; 3] = camera.eye().into();
    let away: [f32; 3] = std::array::from_fn(|i| eye[i] - from[i]);
    let length = (away.iter().map(|c| c * c).sum::<f32>()).sqrt();
    if length < 1e-6 {
        // The camera is on top of the selection, where there is no direction
        // to have. Any unit vector will do and none is right; the ring is
        // invisible at this distance anyway.
        return [0.0, 0.0, 1.0];
    }
    std::array::from_fn(|i| away[i] / length)
}

/// Which handle of the manipulator a ray passes through, if any.
///
/// The nearest along the ray wins, so a handle in front takes a press over one
/// behind it — which is what a person aiming at what they can see expects.
///
/// In the library rather than the binary because it is the rule that decides
/// whether the widget can be used at all, and because the shape of every
/// handle's target is a claim worth holding: **an arrow is grabbed anywhere
/// along its shaft**, not at its point. Tested at the tip alone — which is how
/// it shipped — the target was a sphere a sixth of the arm's length across at
/// the far end of an arm the sculptor could plainly see, so a press on the
/// visible shaft fell through to the cage, the clay or the camera and the
/// widget read as broken. A ring already had this fixed; the arrows did not.
pub fn handle_under(
    mode: clayspace_model::GizmoMode,
    per_axis_scale: bool,
    pivot: [f32; 3],
    reach: f32,
    ray: ([f32; 3], [f32; 3]),
    camera: &clayspace_view::Camera,
) -> Option<(clayspace_model::GizmoMode, clayspace_model::GizmoHandle)> {
    use clayspace_model::{ray_hits_segment, ray_hits_sphere, GizmoHandle, GizmoMode};
    let slack = reach * GIZMO_GRAB;
    // A cell rather than a plain local, because the outer ring is tested only
    // if nothing nearer was hit, and that question is asked while the closure
    // that answers it is still in scope.
    let best: std::cell::Cell<Option<((GizmoMode, GizmoHandle), f32)>> = std::cell::Cell::new(None);
    let keep = |what: (GizmoMode, GizmoHandle), hit: Option<f32>| {
        if let Some(along) = hit {
            if best.get().is_none_or(|(_, closest)| along < closest) {
                best.set(Some((what, along)));
            }
        }
    };
    let mut consider = |what: (GizmoMode, GizmoHandle), at: [f32; 3], radius: f32| {
        keep(what, ray_hits_sphere(ray, at, radius));
    };
    let ring = |radius: f32,
                across: [f32; 3],
                other: [f32; 3],
                what: (GizmoMode, GizmoHandle),
                consider: &mut dyn FnMut((GizmoMode, GizmoHandle), [f32; 3], f32)| {
        // A ring is grabbed anywhere along it, so several points around it
        // are tested rather than one — a ring tested only at its four
        // cardinal points is a ring with four handles.
        let steps = clayspace_model::ring_samples(radius, slack);
        for step in 0..steps {
            let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
            let at = std::array::from_fn(|i| {
                pivot[i] + (across[i] * angle.cos() + other[i] * angle.sin()) * radius
            });
            consider(what, at, slack);
        }
    };

    // What is drawn and what can be grabbed have to be the same set, and
    // `GizmoHandle::combined` is that set: an arrow along its shaft, a ring
    // anywhere along it, a box on the shaft where the target scales per axis.
    // The radii are the renderer's own constants, so the picture and the hit
    // test cannot come apart.
    for (operation, handle) in GizmoHandle::combined(per_axis_scale) {
        let Some(index) = handle.axis_index() else {
            continue;
        };
        let Some(axis) = handle.axis() else {
            continue;
        };
        match operation {
            // The arrows come after everything else; see below.
            GizmoMode::Move => {}
            GizmoMode::Scale => {
                let at = std::array::from_fn(|i| {
                    pivot[i] + axis[i] * reach * clayspace_view::SCALE_BOX_REACH
                });
                consider((operation, handle), at, slack);
            }
            GizmoMode::Rotate => {
                let (u, v) = ((index + 1) % 3, (index + 2) % 3);
                let mut across = [0.0f32; 3];
                across[u] = 1.0;
                let mut other = [0.0f32; 3];
                other[v] = 1.0;
                ring(
                    reach * clayspace_view::RING_REACH,
                    across,
                    other,
                    (operation, handle),
                    &mut consider,
                );
            }
        }
    }
    // The centre does what the mode says: slides, or scales uniformly.
    consider(
        (GizmoHandle::centre_operation(mode), GizmoHandle::Centre),
        pivot,
        slack,
    );
    // The shafts. An arrow is drawn from the pivot to its cone and every part
    // of it reads as a handle, so a press anywhere along one slides along that
    // axis — the complaint this answers is that the manipulator "only works if
    // you land exactly on the axis arrow".
    //
    // Tested *after* the rings, the boxes and the centre, and with the same
    // nearest-along rule, which settles the two ways a shaft meets them. A
    // ring encircles the pivot, so a ray aimed anywhere down the inner shaft
    // passes near the ring's *far* side — behind the press — and the nearer
    // shaft takes it, which is why aiming at the arrow used to turn the
    // selection instead. Where a handle genuinely sits *on* the shaft — the
    // centre block at its foot, the scale box partway out, the two rings that
    // cross it — the two are the same distance away, and going last means the
    // smaller, more particular target keeps the press.
    for (operation, handle) in GizmoHandle::combined(per_axis_scale) {
        if operation != GizmoMode::Move {
            continue;
        }
        let Some(axis) = handle.axis() else {
            continue;
        };
        let tip = std::array::from_fn(|i| pivot[i] + axis[i] * reach);
        keep(
            (operation, handle),
            ray_hits_segment(ray, pivot, tip, slack),
        );
    }
    // The outer ring, tested the way the axis rings are and at the radius it
    // is drawn at. Only where nothing else was hit, so a press where it
    // crosses an arrow or a ring goes to that: the outer one is the easy
    // target everywhere else, and it should not steal the hard ones.
    if best.get().is_none() {
        let axis = toward_eye(camera, pivot);
        let (across, other) = clayspace_view::frame_about(axis.into());
        ring(
            reach * clayspace_view::VIEW_RING_REACH,
            across.into(),
            other.into(),
            (GizmoMode::Rotate, GizmoHandle::View),
            &mut consider,
        );
    }
    best.into_inner().map(|(what, _)| what)
}

/// Whether the brush ring is drawn under the pointer.
///
/// A ring says "the next press leaves a stroke here", so it may only be drawn
/// where that is true. Two modes take the press away from the brush and both
/// have to take the ring with it: the whole-subtool manipulator, where a press
/// on the clay moves it, and a deformation cage, where a press that misses a
/// control point orbits. The cage half was missed — the routing refused the
/// stroke and the ring promised one anyway, which is the worst of both: a
/// sculptor aiming at a control point sees a brush over the form they are
/// bending and cannot tell whether a slip will sculpt it.
pub fn shows_the_brush_ring(layer_manipulator_up: bool, caged: bool) -> bool {
    !layer_manipulator_up && !caged
}

/// Whether a press should start a stroke, or turn the camera instead.
///
/// Here rather than inline in the event loop because it is a rule rather than
/// a step, and because a rule with three clauses and no test is how a mode
/// stops being a mode.
///
/// - A press that misses the surface orbits. That is ZBrush's rule, and the
///   only one that leaves a trackpad with no comfortable right-drag able to
///   turn the model.
/// - The orbit modifier forces it, on the surface or off it.
/// - **A cage takes the whole viewport.** While one is up the layer is being
///   deformed, and a press that misses a control point used to fall through to
///   the brush — so a slip while aiming sculpted the very form the cage was
///   there to bend, and the strokes it left made the next point harder to hit.
pub fn press_sculpts(on_surface: bool, orbit_modifier: bool, caged: bool) -> bool {
    on_surface && !orbit_modifier && !caged
}

/// Whether a press on the clay transforms the whole subtool rather than
/// sculpting it.
///
/// Choosing Mover, Girar or Escalar under the layer stack is a mode, as it is
/// in ZBrush: while the manipulator on a whole subtool is up, a press on the
/// form that misses a handle moves, turns or scales the form — the arrows are
/// for a constrained gesture, the clay itself is the free one. A press that
/// sculpted instead was the worst of both: the sculptor dragged the arrow, saw
/// nothing move, and tried the clay, which left a stroke on a form that was
/// about to move out from under it. Off the form the press still orbits, so
/// the model can be turned to look at without leaving the mode.
pub fn press_transforms(on_surface: bool, layer_manipulator_up: bool) -> bool {
    on_surface && layer_manipulator_up
}

/// What a press in the viewport resolved to.
///
/// Three answers, in the order the subtools design resolves them, because a
/// press has to say two different things at once: which subtool the sculptor
/// is now working on, and whether the press itself belongs to a selection or
/// falls through to the brush.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    /// A placed object. It is selected and offers its manipulator, and the
    /// layer it stands in becomes the sculpt target.
    Object(clayspace_model::ObjectId),
    /// Geometry that is not a placed object — a stroke, a rig's skin, a grid,
    /// a carried mesh. Its layer becomes the sculpt target and the press falls
    /// through to whatever would have had it.
    Subtool(clayspace_model::LayerKey),
    /// The ray met nothing.
    Empty,
}

impl Activation {
    /// The subtool this press makes the sculpt target, where it makes one.
    ///
    /// `None` only for a press on nothing: a document always has a layer being
    /// sculpted, so there is no activation to take away.
    pub fn layer(self) -> Option<clayspace_model::LayerKey> {
        match self {
            Self::Object(id) => Some(id.layer),
            Self::Subtool(key) => Some(key),
            Self::Empty => None,
        }
    }
}

/// Which subtool a press works on from now on, and what the press itself is.
///
/// A rule rather than a step, and here rather than in the event loop for the
/// reason [`press_sculpts`] is: this is the order the whole feature turns on.
///
/// - A placed object keeps what a press on one has always done, and *also*
///   activates the layer it stands in — an object is part of a subtool, and
///   reaching for one means working on that subtool.
/// - Anything else the ray met activates the layer it belongs to. Ghosts never
///   appear here: the engine excludes them from the attributed raycast, so a
///   ray through a ghost answers with what stands behind it.
/// - A ray that met nothing activates nothing. Activation is not cleared,
///   because a document always has a layer being sculpted; what is put down is
///   the object selection, which is the selection that can be empty.
///
/// `picked` is asked only where the interface is picking objects at all, so a
/// press on the clay mid-sculpt is not answered with "that cannot be
/// transformed"; `hit` is the layer the same ray met.
pub fn activation(
    picked: clayspace_vm::Picked,
    hit: Option<clayspace_model::LayerKey>,
) -> Activation {
    match (picked, hit) {
        // The object's own layer rather than `hit`: the two come from the same
        // attributed raycast, and the object is the more specific answer.
        (clayspace_vm::Picked::Object(id), _) => Activation::Object(id),
        (_, Some(key)) => Activation::Subtool(key),
        (_, None) => Activation::Empty,
    }
}

/// What a press does to the object selection, once [`activation`] has said
/// what it met.
///
/// `Some(selection)` is a `Command::SelectObject` to dispatch and `None` is a
/// press that leaves the selection alone.
///
/// Here rather than in the event loop because it carries a requirement of its
/// own — scene-and-layers, "clicking empty space clears the selection": "the
/// selection is cleared rather than left on the previous target". That arm sat
/// inside the composition root where nothing could reach it, and the test that
/// had held the scenario was rewritten into one that asserts only what the
/// raycast answered. A press on nothing that left a form selected leaves its
/// manipulator drawn over clay the sculptor has stopped pointing at.
///
/// `selected` is whether anything is selected now: clearing what is already
/// clear is an entry in the history for nothing.
pub fn selection_after(
    activation: Activation,
    selected: bool,
) -> Option<Option<clayspace_model::ObjectId>> {
    match activation {
        Activation::Object(id) => Some(Some(id)),
        // A press on a subtool that is not an object falls through to the
        // brush, and the selection is whatever it was: the sculptor reached
        // for clay, not for a control.
        Activation::Subtool(_) => None,
        Activation::Empty => selected.then_some(None),
    }
}

/// The world-space ray through a point of the viewport.
///
/// Free-standing, and used by the binary rather than reimplemented there, so a
/// test can build the same ray the application does. Both the normalised
/// coordinates and the aspect come from `viewport` — the rectangle the scene
/// is also drawn into. When those two rectangles were allowed to differ the
/// pick landed beside the pointer.
pub fn ray_at(
    camera: &clayspace_view::Camera,
    viewport: egui::Rect,
    point: egui::Pos2,
) -> Option<([f32; 3], [f32; 3])> {
    if !viewport.contains(point) || viewport.width() < 1.0 || viewport.height() < 1.0 {
        return None;
    }
    let ndc = [
        ((point.x - viewport.min.x) / viewport.width()) * 2.0 - 1.0,
        1.0 - ((point.y - viewport.min.y) / viewport.height()) * 2.0,
    ];
    Some(camera.ray_through(ndc, viewport.aspect_ratio()))
}

/// Where a world point sits in the viewport, in egui points.
///
/// The inverse of [`ray_at`], through the camera's own inverse, so the two
/// cannot drift apart: a pick that lands beside the pointer and a selection
/// box that catches the wrong points are the same bug seen twice.
///
/// `None` where the point is behind the camera, which has no position on
/// screen to be at.
pub fn screen_at(
    camera: &clayspace_view::Camera,
    viewport: egui::Rect,
    world: [f32; 3],
) -> Option<egui::Pos2> {
    if viewport.width() < 1.0 || viewport.height() < 1.0 {
        return None;
    }
    let ndc = camera.screen_through(world, viewport.aspect_ratio())?;
    Some(egui::pos2(
        viewport.min.x + (ndc[0] + 1.0) * 0.5 * viewport.width(),
        viewport.min.y + (1.0 - ndc[1]) * 0.5 * viewport.height(),
    ))
}

/// How far a press has to travel before it is a selection box rather than a
/// click on nothing, in egui points.
///
/// A press and release at the same place is a click, and a click on nothing
/// clears the selection. Without a threshold the hand's own tremor between
/// button-down and button-up would make every such click a box of two points
/// across, which selects nothing and so *looks* the same — until the one time
/// it catches a control point the sculptor was not aiming at.
pub const MARQUEE_SLOP: f32 = 3.0;

/// Whether a press that took hold of nothing drew a selection box.
pub fn is_a_marquee(from: egui::Pos2, to: egui::Pos2) -> bool {
    (to - from).length() > MARQUEE_SLOP
}

/// Which of `points` a selection box drawn from `from` to `to` catches.
///
/// Every point inside the box, in ascending order — not the nearest one, and
/// not only the ones facing the camera. Half a cage's control points stand
/// behind the form and the viewport draws them through it for exactly that
/// reason: a box drawn around a face has to take the far corners with the near
/// ones, or turning a whole face — which is what the manipulator exists for —
/// would need eight separate Shift-clicks and a camera move in the middle.
///
/// Points behind the camera are not caught: they have no position on screen,
/// and a box cannot be drawn around something that is not in the picture.
pub fn points_within(
    camera: &clayspace_view::Camera,
    viewport: egui::Rect,
    points: &[[f32; 3]],
    from: egui::Pos2,
    to: egui::Pos2,
) -> Vec<usize> {
    let box_drawn = egui::Rect::from_two_pos(from, to);
    points
        .iter()
        .enumerate()
        .filter(|(_, at)| {
            screen_at(camera, viewport, **at).is_some_and(|on_screen| {
                box_drawn.contains(on_screen) && viewport.contains(on_screen)
            })
        })
        .map(|(index, _)| index)
        .collect()
}

/// The selection a marquee leaves behind.
///
/// Held apart from the box itself because it is the rule and not the geometry:
/// a plain drag *replaces* the selection, and one made with the add modifier
/// held adds to it — the same bargain a Shift-click on a single control point
/// already makes, so the two gestures can be mixed without one undoing the
/// other's work.
pub fn selection_from_marquee(held: &[usize], caught: &[usize], add: bool) -> Vec<usize> {
    let mut selection: Vec<usize> = if add {
        held.iter().copied().chain(caught.iter().copied()).collect()
    } else {
        caught.to_vec()
    };
    // Ascending and without repeats, which is what the model keeps and what
    // makes the pivot the same however the points were gathered.
    selection.sort_unstable();
    selection.dedup();
    selection
}

/// Where a drag has carried the point it took hold of.
///
/// A *dragging* verb — Mover, Puxar, Nudge — takes hold of the surface once
/// and then follows the pointer. Where it follows it to is a question about
/// the camera and not about the model: the anchor is carried along the plane
/// it was picked on, which is what makes a drag *away* from the form pull
/// material out of it.
///
/// The alternative, and what this replaces, is to re-pick the surface under
/// the pointer at every sample. That is right for a verb that stamps — the
/// stamp belongs where the pointer is — and wrong for one that drags, because
/// every sample then lands *on* the surface and the motion between two of them
/// is a walk along it. The form is never pulled anywhere; its skin slides
/// across it and folds. Dragging off the silhouette is worse than wrong: the
/// pick finds nothing, no sample is sent, and the stroke quietly stops.
///
/// `anchor` is the point the press took hold of, `press` is where the pointer
/// was then, and `to` is where it is now. The anchor's distance along its own
/// ray is what fixes the depth, so a perspective camera carries it by the
/// right amount rather than by a screen distance.
///
/// `None` when either point is outside the viewport, which is the one case
/// with no answer — the ray through it does not exist.
pub fn dragged_to(
    camera: &clayspace_view::Camera,
    viewport: egui::Rect,
    anchor: [f32; 3],
    press: egui::Pos2,
    to: egui::Pos2,
) -> Option<[f32; 3]> {
    let (origin, direction) = ray_at(camera, viewport, press)?;
    // How far along its ray the anchor sits. Projected rather than measured
    // straight, so a point picked off the ray's centre still reports the depth
    // the camera sees it at.
    let depth = (0..3)
        .map(|i| (anchor[i] - origin[i]) * direction[i])
        .sum::<f32>();
    let (moved_origin, moved_direction) = ray_at(camera, viewport, to)?;
    Some(std::array::from_fn(|i| {
        moved_origin[i] + moved_direction[i] * depth
    }))
}

#[cfg(test)]
mod drag_tests {
    use super::*;
    use clayspace_view::Camera;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))
    }

    /// A pointer that has not moved leaves the anchor where it was.
    #[test]
    fn a_drag_that_went_nowhere_carries_nothing() {
        let camera = Camera::default();
        let at = egui::pos2(400.0, 300.0);
        let (origin, direction) = ray_at(&camera, viewport(), at).expect("a ray");
        let anchor: [f32; 3] = std::array::from_fn(|i| origin[i] + direction[i] * 4.0);

        let held = dragged_to(&camera, viewport(), anchor, at, at).expect("a point");
        for axis in 0..3 {
            assert!(
                (held[axis] - anchor[axis]).abs() < 1e-3,
                "the anchor moved to {held:?} without the pointer moving from {anchor:?}"
            );
        }
    }

    /// And a pointer that moved carries it, by more than nothing and in the
    /// direction it went.
    #[test]
    fn a_drag_carries_the_anchor_with_the_pointer() {
        let camera = Camera::default();
        let press = egui::pos2(400.0, 300.0);
        let (origin, direction) = ray_at(&camera, viewport(), press).expect("a ray");
        let anchor: [f32; 3] = std::array::from_fn(|i| origin[i] + direction[i] * 4.0);

        let right = dragged_to(&camera, viewport(), anchor, press, egui::pos2(500.0, 300.0))
            .expect("a point");
        let moved: f32 = (0..3)
            .map(|i| (right[i] - anchor[i]).powi(2))
            .sum::<f32>()
            .sqrt();
        assert!(
            moved > 0.1,
            "a hundred pixels of drag carried the anchor {moved}, which is nowhere"
        );

        // Further pointer, further anchor, and monotonically.
        let further = dragged_to(&camera, viewport(), anchor, press, egui::pos2(600.0, 300.0))
            .expect("a point");
        let further_moved: f32 = (0..3)
            .map(|i| (further[i] - anchor[i]).powi(2))
            .sum::<f32>()
            .sqrt();
        assert!(
            further_moved > moved,
            "twice the drag carried the anchor {further_moved} against {moved}"
        );
    }

    /// The anchor stays at the depth it was taken from.
    ///
    /// This is what makes the carried point a *translation* of the surface
    /// rather than another point on it: the drag is free to leave the form,
    /// which is the whole difference between pulling a lobe out and sliding
    /// the skin around.
    #[test]
    fn a_drag_keeps_the_depth_it_took_hold_at() {
        let camera = Camera::default();
        let press = egui::pos2(400.0, 300.0);
        let (origin, direction) = ray_at(&camera, viewport(), press).expect("a ray");
        let anchor: [f32; 3] = std::array::from_fn(|i| origin[i] + direction[i] * 4.0);

        let moved = dragged_to(&camera, viewport(), anchor, press, egui::pos2(560.0, 220.0))
            .expect("a point");
        let (moved_origin, moved_direction) =
            ray_at(&camera, viewport(), egui::pos2(560.0, 220.0)).expect("a ray");
        let depth: f32 = (0..3)
            .map(|i| (moved[i] - moved_origin[i]) * moved_direction[i])
            .sum();
        assert!(
            (depth - 4.0).abs() < 1e-2,
            "the carried point sits {depth} along its ray, not the 4.0 the \
             anchor was taken from"
        );
    }

    /// Off the viewport there is no ray and so no answer.
    #[test]
    fn a_pointer_outside_the_viewport_has_no_answer() {
        let camera = Camera::default();
        let press = egui::pos2(400.0, 300.0);
        assert!(dragged_to(
            &camera,
            viewport(),
            [0.0, 0.0, 0.0],
            press,
            egui::pos2(-40.0, 300.0)
        )
        .is_none());
    }
}

#[cfg(test)]
mod manipulator_tests {
    use super::*;
    use clayspace_model::{GizmoHandle, GizmoMode};
    use clayspace_view::Camera;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))
    }

    /// The widget at the origin, small enough to sit well inside the frame.
    const REACH: f32 = 0.5;

    /// A camera off every axis.
    ///
    /// Square on, a ring lying in a plane containing the eye is drawn — and
    /// picked — as a line straight along an axis, so the two share presses
    /// that have nothing to do with either's grab radius. That is a real
    /// ambiguity in an edge-on ring rather than a fact about the shaft, and
    /// this test is about the shaft.
    fn angled() -> Camera {
        Camera {
            yaw: 0.7,
            pitch: 0.35,
            ..Camera::default()
        }
    }

    /// Which handle a press at a world point finds, through the ray the
    /// application would actually build for it.
    fn grabbed(camera: &Camera, at: [f32; 3], mode: GizmoMode) -> Option<(GizmoMode, GizmoHandle)> {
        let on_screen = screen_at(camera, viewport(), at).expect("a point in front of the camera");
        let ray = ray_at(camera, viewport(), on_screen).expect("a ray through it");
        handle_under(mode, true, [0.0; 3], REACH, ray, camera)
    }

    #[test]
    fn an_arrow_is_grabbed_anywhere_along_its_shaft() {
        // Reported from using it: "the gizmo for movement only works if we
        // perfectly land the mouse on the axis arrow". The shaft is drawn from
        // the pivot to the cone and every part of it reads as a handle, but
        // only a sphere at the tip was tested — so a press on most of what a
        // person can see fell through to the cage, the clay or the camera.
        let camera = angled();
        for step in 2..=10 {
            let along = step as f32 / 10.0;
            let at = [0.0, REACH * along, 0.0];
            assert!(
                grabbed(&camera, at, GizmoMode::Move).is_some(),
                "nothing at all was grabbable {along} of the way along the arrow"
            );
        }
        // And where nothing else is drawn on the shaft it is the arrow. The
        // widget's own geometry decides where that is: the centre block sits
        // at the foot, the scale box at `SCALE_BOX_REACH`, and two of the
        // three rings cross every axis at `RING_REACH` — each within one grab
        // radius either side.
        let free = [
            GIZMO_GRAB * 1.3,
            clayspace_view::SCALE_BOX_REACH - GIZMO_GRAB * 1.2,
            clayspace_view::RING_REACH + GIZMO_GRAB * 1.05,
            1.0,
        ];
        for along in free {
            let at = [0.0, REACH * along, 0.0];
            assert_eq!(
                grabbed(&camera, at, GizmoMode::Move),
                Some((GizmoMode::Move, GizmoHandle::Axis(1))),
                "a press {along} of the way along the vertical arrow did not move it"
            );
        }
    }

    #[test]
    fn the_particular_handles_keep_their_own_presses() {
        // The shaft is tested only where nothing more particular was hit, so
        // making the whole arm grabbable must not take the box's press or the
        // rings'. Both sit *on* the shaft — the box partway out, the rings
        // where they cross it — and both are the smaller target.
        let camera = Camera::default();
        let box_at = [0.0, REACH * clayspace_view::SCALE_BOX_REACH, 0.0];
        assert_eq!(
            grabbed(&camera, box_at, GizmoMode::Move),
            Some((GizmoMode::Scale, GizmoHandle::Axis(1))),
            "the scale box lost its press to the shaft it sits on"
        );
        // The centre block, at the foot of all three shafts.
        assert_eq!(
            grabbed(&camera, [0.0; 3], GizmoMode::Move),
            Some((GizmoMode::Move, GizmoHandle::Centre)),
            "the centre lost its press to a shaft"
        );
    }

    #[test]
    fn a_press_beyond_the_arrowhead_grabs_nothing() {
        // The shaft is a finite thing. Tested as a line rather than a segment,
        // every press along the axis out to the horizon would slide the
        // selection.
        let camera = Camera::default();
        let past = [0.0, REACH * 2.0, 0.0];
        assert_eq!(
            grabbed(&camera, past, GizmoMode::Move),
            None,
            "a press well past the arrowhead grabbed the arrow"
        );
    }

    #[test]
    fn the_brush_ring_is_off_wherever_a_press_cannot_sculpt() {
        // The other half of `press_sculpts`: the routing already refused the
        // stroke, and the ring went on promising one. Reported as brushes
        // showing over the form while a deformation cage was up.
        assert!(shows_the_brush_ring(false, false));
        assert!(!shows_the_brush_ring(false, true), "a cage kept the ring");
        assert!(!shows_the_brush_ring(true, false));
    }
}

#[cfg(test)]
mod marquee_tests {
    use super::*;
    use clayspace_view::Camera;

    fn viewport() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))
    }

    /// Eight points, a unit box about the origin — a cage's corners.
    fn corners() -> Vec<[f32; 3]> {
        let mut points = Vec::new();
        for z in [-0.5f32, 0.5] {
            for y in [-0.5f32, 0.5] {
                for x in [-0.5f32, 0.5] {
                    points.push([x, y, z]);
                }
            }
        }
        points
    }

    #[test]
    fn a_box_catches_the_points_inside_it_and_no_others() {
        let camera = Camera::default();
        let points = corners();
        // A band drawn around the top half of the box on screen.
        let top: Vec<egui::Pos2> = points
            .iter()
            .filter(|at| at[1] > 0.0)
            .map(|at| screen_at(&camera, viewport(), *at).expect("on screen"))
            .collect();
        let mut band = egui::Rect::NOTHING;
        for at in &top {
            band = band.union(egui::Rect::from_center_size(*at, egui::vec2(8.0, 8.0)));
        }
        let caught = points_within(&camera, viewport(), &points, band.min, band.max);
        assert_eq!(
            caught.len(),
            4,
            "a box round the top four corners caught {caught:?}"
        );
        for index in caught {
            assert!(
                points[index][1] > 0.0,
                "the box caught a point below it: {:?}",
                points[index]
            );
        }
    }

    #[test]
    fn a_box_catches_the_points_behind_the_form_too() {
        // Half a cage's control points stand behind the clay, and the viewport
        // draws them through it for exactly that reason. A box that caught
        // only the near four would make turning a whole face — which is what
        // the manipulator exists for — eight clicks and a camera move.
        let camera = Camera::default();
        let points = corners();
        let caught = points_within(&camera, viewport(), &points, viewport().min, viewport().max);
        assert_eq!(
            caught.len(),
            points.len(),
            "a box over the whole viewport missed some"
        );
    }

    #[test]
    fn a_click_is_not_a_box() {
        // A press and release in one place is a click on nothing, which clears
        // the selection. The hand's own tremor must not turn that into a box.
        let at = egui::pos2(400.0, 300.0);
        assert!(!is_a_marquee(at, at));
        assert!(!is_a_marquee(at, at + egui::vec2(2.0, 1.0)));
        assert!(is_a_marquee(at, at + egui::vec2(40.0, 30.0)));
    }

    #[test]
    fn a_plain_box_replaces_and_an_adding_one_adds() {
        let held = [1usize, 4];
        let caught = [4usize, 6];
        assert_eq!(selection_from_marquee(&held, &caught, false), vec![4, 6]);
        // Ascending and without repeats, which is what the model keeps: the
        // pivot has to be the same however the points were gathered.
        assert_eq!(selection_from_marquee(&held, &caught, true), vec![1, 4, 6]);
    }
}

#[cfg(test)]
mod press_tests {
    use super::{notches, press_sculpts, press_transforms};

    #[test]
    fn a_press_on_the_clay_transforms_while_the_layer_manipulator_is_up() {
        assert!(press_transforms(true, true));
        // Off the form the camera keeps working, so the mode can be looked
        // around in.
        assert!(!press_transforms(false, true));
        // Without the manipulator the clay is the brush's.
        assert!(!press_transforms(true, false));
    }

    #[test]
    fn a_press_on_the_surface_sculpts_and_one_off_it_orbits() {
        assert!(press_sculpts(true, false, false));
        assert!(!press_sculpts(false, false, false));
    }

    #[test]
    fn the_orbit_modifier_forces_orbiting_wherever_it_lands() {
        assert!(!press_sculpts(true, true, false));
        assert!(!press_sculpts(false, true, false));
    }

    #[test]
    fn a_cage_takes_the_whole_viewport() {
        // Reported from using it: with a cage up, a press that missed a
        // control point sculpted the form the cage was there to bend, and the
        // blobs it left made the next point harder to hit.
        assert!(
            !press_sculpts(true, false, true),
            "a press on the surface still sculpted while a cage was up"
        );
        // Orbiting rather than nothing, so a cage can be turned to look at
        // from behind without being taken down.
        assert!(!press_sculpts(false, false, true));
    }

    #[test]
    fn a_wheel_notch_is_one_notch_whatever_egui_measures_it_in() {
        // The bug this exists for: egui reports scrolling in points and one
        // wheel notch is forty of them. Handed to the camera raw, a single
        // notch asked it to move forty times what a notch is meant to move —
        // inward that is a negative distance saved only by a clamp, outward it
        // is five times further away in one click. "The zoom jumps are too
        // big" was the report; this is the conversion that was missing.
        let ctx = egui::Context::default();
        let per_notch = ctx.options(|options| options.line_scroll_speed);
        assert!(
            per_notch > 1.0,
            "egui measures a line in points, not notches"
        );

        assert!(
            (notches(per_notch, per_notch) - 1.0).abs() < 1e-4,
            "a notch of scroll did not come through as one notch"
        );
        assert!((notches(per_notch, -per_notch) + 1.0).abs() < 1e-4);
        // A trackpad's fraction stays a fraction rather than rounding to a step.
        assert!((notches(per_notch, per_notch / 4.0) - 0.25).abs() < 1e-4);
        assert_eq!(notches(per_notch, 0.0), 0.0);
        // And a nonsense figure is no scroll rather than an infinity.
        assert_eq!(notches(0.0, 100.0), 0.0);
    }

    #[test]
    fn reading_the_frame_does_not_deadlock() {
        // It did. `Context::input` takes the write lock and `Context::options`
        // takes the read lock on the same `RwLock`, and the first version of
        // the conversion asked egui for its options from inside the input
        // closure — so the application froze on the first frame that read the
        // pointer. The test above passed anyway, because it called the
        // conversion on its own rather than through `read`.
        //
        // This drives the whole of `read` inside a real frame. It hangs rather
        // than fails if the lock is taken twice, which is the failure being
        // guarded against.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, response) =
                    ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::click_and_drag());
                let input = super::ViewportInput::read(ui, &response);
                assert_eq!(input.scroll, 0.0, "no scroll was delivered this frame");
            });
        });
    }
}

#[cfg(test)]
mod activation_tests {
    use super::{activation, selection_after, Activation};
    use clayspace_model::{LayerKey, ObjectId};
    use clayspace_vm::Picked;

    fn object_in(layer: u64) -> ObjectId {
        ObjectId {
            layer: LayerKey(layer),
            node: 7,
        }
    }

    /// The order the whole feature turns on: an object hit keeps what a press
    /// on one has always done, and carries its subtool with it.
    #[test]
    fn an_object_hit_selects_the_object() {
        let id = object_in(2);
        assert_eq!(
            activation(Picked::Object(id), Some(LayerKey(9))),
            Activation::Object(id),
            "the object's own layer is the specific answer; the ray's is not"
        );
        assert_eq!(id.layer, LayerKey(2), "and it is the layer to activate");
    }

    /// A press on a stroke, a rig's skin or a grid is not a selection — but it
    /// is still a press on a subtool, and that subtool becomes the one a brush
    /// lands on.
    #[test]
    fn geometry_that_is_not_an_object_still_activates_its_subtool() {
        assert_eq!(
            activation(Picked::NotTransformable(LayerKey(4)), Some(LayerKey(4))),
            Activation::Subtool(LayerKey(4))
        );
        assert_eq!(
            activation(Picked::Nothing, Some(LayerKey(4))),
            Activation::Subtool(LayerKey(4)),
            "the object picker answers Nothing for a grid too; the layer the \
             ray met is what says a form was there"
        );
    }

    /// A ghosted subtool is excluded from the engine's raycast, so what
    /// reaches this rule is the layer *behind* it — there is no ghost case to
    /// write here, and this holds that none is invented.
    #[test]
    fn the_layer_the_ray_answered_is_the_one_activated() {
        assert_eq!(
            activation(Picked::Nothing, Some(LayerKey(1))),
            Activation::Subtool(LayerKey(1))
        );
    }

    /// The specification: "the selection is cleared rather than left on the
    /// previous target".
    ///
    /// The scenario had no test at any level. `a_ray_that_met_nothing_activates_nothing`
    /// stops at the enum value and the ViewModel's own test stops at what the
    /// pick answered; nothing said what the press then *does*, and the arm that
    /// does it sat in the event loop. Failure it let through: a press on empty
    /// space leaving the previously selected form selected, with its
    /// manipulator still drawn over clay nobody is pointing at.
    #[test]
    fn a_press_on_nothing_clears_the_selection() {
        assert_eq!(
            selection_after(Activation::Empty, true),
            Some(None),
            "a press on nothing has to put the selection down"
        );
        assert_eq!(
            selection_after(Activation::Empty, false),
            None,
            "and clearing what is already clear is an entry in the history \
             for nothing"
        );
    }

    /// The other two arms, so the one above cannot be satisfied by clearing
    /// everything.
    #[test]
    fn a_press_on_a_form_leaves_the_selection_where_it_belongs() {
        let id = object_in(3);
        assert_eq!(
            selection_after(Activation::Object(id), false),
            Some(Some(id)),
            "a press on a placed object selects it"
        );
        assert_eq!(
            selection_after(Activation::Subtool(LayerKey(5)), true),
            None,
            "a press on clay falls through to the brush and takes nothing away"
        );
    }

    /// Every activation but the empty one names the subtool to sculpt on.
    #[test]
    fn an_activation_names_the_subtool_it_activates() {
        assert_eq!(Activation::Object(object_in(2)).layer(), Some(LayerKey(2)));
        assert_eq!(Activation::Subtool(LayerKey(6)).layer(), Some(LayerKey(6)));
        assert_eq!(
            Activation::Empty.layer(),
            None,
            "a document always has a layer being sculpted, so a press on \
             nothing takes no activation away"
        );
    }

    #[test]
    fn a_ray_that_met_nothing_activates_nothing() {
        assert_eq!(activation(Picked::Nothing, None), Activation::Empty);
        assert_eq!(
            activation(Picked::NotTransformable(LayerKey(4)), None),
            Activation::Empty,
            "the layer comes from the press, not from the pick's own record: \
             a caller that answered `None` met nothing"
        );
    }
}
