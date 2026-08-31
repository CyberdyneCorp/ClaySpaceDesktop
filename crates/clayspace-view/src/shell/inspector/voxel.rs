//! A voxel layer: a palette-indexed grid of cells.

use super::*;

/// How a voxel layer is drawn.
///
/// A display setting and nothing more: the engine keeps the choice an argument
/// rather than grid state so two hosts sharing a document cannot disagree
/// about what it looks like, and nothing here touches a cell.
///
/// It stood under the geometry heading, beside the polygon counts that stand
/// under the same word — two sections with one title between them, sharing the
/// fold that title is keyed by, so putting one away put the other away too.
///
/// The recorded passes are not repeated here. They are nested under the layer
/// they were recorded on, in the left stack, because a pass has no meaning
/// apart from that grid — and the control that starts and ends a recording
/// belongs beside the passes it makes, not in a second place that would have
/// to say which layer it meant.
pub(super) fn show(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    use clayspace_model::{SmoothBlur, VoxelDisplay};
    let s = state.strings;
    if !heading(ui, s.section_voxels) {
        return;
    }

    ui.label(
        egui::RichText::new(s.label_voxel_display)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    ui.horizontal_wrapped(|ui| {
        for display in VoxelDisplay::ALL {
            let on = state.voxel_display == display;
            if ui
                .add(chip(s.voxel_display_name(display), on, Tokens::panel()))
                .clicked()
            {
                queue.push(Command::SetVoxelDisplay(display, state.voxel_blur));
            }
        }
    });

    if state.voxel_display != VoxelDisplay::Smooth {
        return;
    }
    if let Some(value) = slider(
        ui,
        s.label_voxel_blur,
        state.voxel_blur.passes() as f32,
        0.0..=SmoothBlur::MOST as f32,
        0,
    ) {
        queue.push(Command::SetVoxelDisplay(
            state.voxel_display,
            SmoothBlur::new(value.round() as i32),
        ));
    }
    // Said where it is true rather than left for a sculptor to find out from a
    // missing finger.
    if state.voxel_blur.can_lose_detail() {
        ui.label(
            egui::RichText::new(s.hint_voxel_blur)
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }
}
