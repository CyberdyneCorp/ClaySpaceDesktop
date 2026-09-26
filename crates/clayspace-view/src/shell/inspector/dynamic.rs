//! An adaptive surface: triangles whose connectivity follows the brush.

use super::*;

/// The one thing that is true of every adaptive layer and tells it apart from
/// a mesh.
///
/// A mesh's section says its topology is fixed; this one says the opposite,
/// and that quads do not survive, so a sculptor about to spend a retopology
/// reads it before rather than after.
pub(super) fn show(ui: &mut egui::Ui, state: &ShellState<'_>) {
    let s = state.strings;
    if !heading(ui, s.section_dynamic) {
        return;
    }
    ui.label(
        egui::RichText::new(s.dynamic_topology_adapts)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
}
