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
    pub orbit_modifier: bool,
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
