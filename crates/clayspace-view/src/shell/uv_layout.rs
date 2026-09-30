//! How a UV layout is chosen to be seen.
//!
//! Two surfaces offer it: the mesh inspector for an active layer carrying UVs,
//! and the retopology panel for a held preview carrying them. Both draw the
//! same chips from the same state, so a display chosen in one is the display
//! the other shows.

use super::*;

use clayspace_model::UvDisplay;

/// Where one UV display chip was drawn, for a test to press it.
pub fn uv_display_chip_id(display: UvDisplay) -> egui::Id {
    egui::Id::new(("uv-display", display as u8))
}

/// How the layout is drawn, where there is one.
///
/// Nothing at all where there is no layout: three chips that change nothing
/// would say there is something to see.
pub(super) fn uv_display_controls(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    if !state.carries_uvs {
        return;
    }
    let s = state.strings;
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(s.label_uv_display)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    ui.horizontal_wrapped(|ui| {
        for display in UvDisplay::ALL {
            let on = state.uv_display == display;
            let response = ui.add(chip(s.uv_display_name(display), on, Tokens::panel()));
            remember_rect(ui, uv_display_chip_id(display), response.rect);
            if response.clicked() {
                queue.push(Command::SetUvDisplay(display));
            }
        }
    });
    if state.uv_display.is_on() {
        ui.label(
            egui::RichText::new(s.hint_uv_display)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
    }
}

fn remember_rect(ui: &egui::Ui, id: egui::Id, rect: egui::Rect) {
    ui.ctx()
        .memory_mut(|memory| memory.data.insert_temp(id, rect));
}
