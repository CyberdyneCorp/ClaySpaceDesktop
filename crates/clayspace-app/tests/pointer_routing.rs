//! Where a press goes.
//!
//! These exist because of a shipped bug: the viewport read its input from
//! winit and gated it on `egui_winit::EventResponse::consumed`. That flag is
//! `Context::wants_pointer_input()`, which is true wherever the pointer is
//! over an egui *area* — and a `CentralPanel` is an area covering the entire
//! viewport. Every press, drag and scroll in the window was discarded before
//! it reached the sculpting or camera code. The panels still worked, the hover
//! ring still tracked, and the window still opened, so nothing in the suite
//! noticed. Nothing in the suite could have: no test went from a pointer
//! position to a stroke, and `main.rs` had no tests at all.
//!
//! The layout here mirrors the shell's — same panel sizes from
//! [`region`] — because the defect was a consequence of that arrangement.

use clayspace_app::ViewportInput;
use clayspace_view::shell::region;

const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// Runs two frames of the shell's panel arrangement and reports what the
/// viewport made of the pointer.
///
/// Two frames because egui lays panels out on one and hit-tests against that
/// layout on the next; a single frame would report on a viewport that did not
/// exist yet.
fn route(pointer: egui::Pos2, buttons: &[egui::PointerButton]) -> (ViewportInput, egui::Rect, bool) {
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, WINDOW);

    let mut captured = None;
    let mut build = |ctx: &egui::Context| {
        egui::TopBottomPanel::top("menu")
            .exact_height(region::MENU_BAR)
            .show(ctx, |ui| ui.label("menu"));
        egui::TopBottomPanel::top("options")
            .exact_height(region::OPTIONS_BAR)
            .show(ctx, |ui| ui.label("options"));
        egui::TopBottomPanel::bottom("status")
            .exact_height(region::STATUS)
            .show(ctx, |ui| ui.label("status"));
        egui::TopBottomPanel::bottom("shelf")
            .exact_height(region::SHELF)
            .show(ctx, |ui| ui.label("shelf"));
        egui::SidePanel::left("left")
            .exact_width(region::LEFT)
            .show(ctx, |ui| ui.label("left"));
        egui::SidePanel::right("right")
            .exact_width(region::RIGHT)
            .show(ctx, |ui| ui.label("right"));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label("viewport bar");
                    let (rect, response) =
                        ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
                    captured = Some((ViewportInput::read(ui, &response), rect));
                });
            });
    };

    let quiet = egui::RawInput {
        screen_rect: Some(screen),
        ..Default::default()
    };
    let _ = ctx.run(quiet.clone(), &mut build);

    let mut input = quiet;
    input.events.push(egui::Event::PointerMoved(pointer));
    for button in buttons {
        input.events.push(egui::Event::PointerButton {
            pos: pointer,
            button: *button,
            pressed: true,
            modifiers: Default::default(),
        });
    }
    let _ = ctx.run(input, &mut build);

    let (viewport, rect) = captured.expect("the viewport was never allocated");
    (viewport, rect, ctx.wants_pointer_input())
}

/// The middle of whatever the panels left.
fn viewport_centre() -> egui::Pos2 {
    let (_, rect, _) = route(egui::pos2(-100.0, -100.0), &[]);
    rect.center()
}

#[test]
fn a_press_in_the_viewport_reaches_the_viewport() {
    let (input, _, _) = route(viewport_centre(), &[egui::PointerButton::Primary]);

    assert!(
        input.over_viewport,
        "a press in the middle of the viewport was not recognised as being there"
    );
    assert_eq!(
        input.pressed,
        Some(egui::PointerButton::Primary),
        "the press never arrived, so nothing would be sculpted"
    );
}

#[test]
fn the_consumed_flag_would_have_swallowed_that_press() {
    // The regression, stated as the fact that caused it. If egui ever stops
    // reporting the central panel as an area this fails, and the comment above
    // `ViewportInput` should be revisited — but the routing must still not go
    // back to asking this question.
    let (_, _, wants_pointer_input) = route(viewport_centre(), &[]);

    assert!(
        wants_pointer_input,
        "wants_pointer_input() is false over the viewport, which is not what \
         made the original bug — check whether the routing rationale still holds"
    );
}

#[test]
fn a_press_on_a_panel_stays_on_the_panel() {
    for (name, pointer) in [
        ("left panel", egui::pos2(region::LEFT * 0.5, 400.0)),
        ("right panel", egui::pos2(WINDOW.x - region::RIGHT * 0.5, 400.0)),
        ("brush shelf", egui::pos2(640.0, WINDOW.y - region::STATUS - 20.0)),
        ("options bar", egui::pos2(640.0, region::MENU_BAR + 10.0)),
    ] {
        let (input, _, _) = route(pointer, &[egui::PointerButton::Primary]);
        assert!(
            !input.over_viewport,
            "a press on the {name} was routed to the viewport, so clicking a \
             control would also sculpt"
        );
    }
}

#[test]
fn the_viewport_bar_is_not_the_viewport() {
    // The bar sits inside the central panel. If the viewport claimed the whole
    // panel, every click on the bar would also start a stroke.
    let (_, rect, _) = route(egui::pos2(-100.0, -100.0), &[]);
    let below_the_viewport = egui::pos2(rect.center().x, rect.max.y + 8.0);

    let (input, _, _) = route(below_the_viewport, &[egui::PointerButton::Primary]);
    assert!(
        !input.over_viewport,
        "the viewport swallowed the bar beneath it"
    );
}

#[test]
fn the_viewport_is_the_hole_the_panels_left() {
    let (_, rect, _) = route(egui::pos2(-100.0, -100.0), &[]);

    assert!(
        rect.min.x >= region::LEFT && rect.max.x <= WINDOW.x - region::RIGHT,
        "the viewport {rect:?} overlaps a side panel"
    );
    assert!(
        rect.min.y >= region::MENU_BAR && rect.max.y <= WINDOW.y - region::STATUS,
        "the viewport {rect:?} overlaps a bar"
    );
    assert!(
        rect.width() > 100.0 && rect.height() > 100.0,
        "the viewport collapsed to {rect:?}, which no pointer could hit"
    );
}

#[test]
fn a_release_is_reported_wherever_it_lands() {
    // A stroke that ends over a panel must still end. The gate is on the
    // press, never on the release.
    let ctx = egui::Context::default();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, WINDOW);
    let mut captured = None;
    let mut build = |ctx: &egui::Context| {
        egui::SidePanel::left("left")
            .exact_width(region::LEFT)
            .show(ctx, |ui| ui.label("left"));
        egui::CentralPanel::default().show(ctx, |ui| {
            let (_, response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
            captured = Some(ViewportInput::read(ui, &response));
        });
    };

    let base = egui::RawInput {
        screen_rect: Some(screen),
        ..Default::default()
    };
    let _ = ctx.run(base.clone(), &mut build);

    let over_panel = egui::pos2(region::LEFT * 0.5, 400.0);
    let mut input = base;
    input.events.push(egui::Event::PointerMoved(over_panel));
    input.events.push(egui::Event::PointerButton {
        pos: over_panel,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });
    let _ = ctx.run(input, &mut build);

    let captured = captured.expect("the viewport was never allocated");
    assert!(
        !captured.over_viewport,
        "the pointer was over the panel, not the viewport"
    );
    assert!(
        captured.released,
        "a release over a panel was not reported, so the stroke would never end"
    );
}
