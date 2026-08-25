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

#[cfg(test)]
mod press_tests {
    use super::{notches, press_sculpts};

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
