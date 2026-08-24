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
    /// Wheel or trackpad scroll.
    pub scroll: f32,
    /// Whether the modifier that forces orbiting is held.
    ///
    /// While rigging this same key means "move this sphere rather than grow a
    /// new one" — over a sphere it moves, over empty space it orbits, so one
    /// key covers both without either meaning being taken away.
    pub orbit_modifier: bool,
    /// Whether the platform's command modifier is held.
    pub command_modifier: bool,
}

impl ViewportInput {
    /// Reads the frame's input for an allocated viewport region.
    pub fn read(ui: &egui::Ui, response: &egui::Response) -> Self {
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
            scroll: i.smooth_scroll_delta.y,
            // Option on a Mac, Alt elsewhere: the trackpad has no second
            // button worth reaching for.
            orbit_modifier: i.modifiers.alt,
            command_modifier: i.modifiers.command,
        })
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
