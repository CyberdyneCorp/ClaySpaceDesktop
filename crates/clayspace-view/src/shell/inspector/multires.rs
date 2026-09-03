//! A multiresolution surface: a cage, levels over it, and detail per level.

use super::*;

/// The one thing that is true of every hierarchy and explains its brushes.
///
/// The same shape as [`super::mesh`]'s section and for the same reason: what is
/// stated here is the *contract*, not a control. A sculptor who does not know
/// that the level being written and the level being drawn are two numbers reads
/// a dab that lands softer than the pointer suggests as a bug in the brush.
///
/// The two numbers themselves are deliberately not here yet. They are per-layer
/// state the shell does not carry, and a control drawn for a value nothing reads
/// is worse than an empty slot — which is this module's own doc's rule, applied
/// to itself.
pub(super) fn show(ui: &mut egui::Ui, state: &ShellState<'_>) {
    let s = state.strings;
    if !heading(ui, s.section_multires) {
        return;
    }
    ui.label(
        egui::RichText::new(s.multires_two_levels)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
}
