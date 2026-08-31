//! A mesh layer: triangles, held verbatim.

use super::*;

/// The one thing that is true of every mesh layer and explains its brushes.
///
/// Fixed topology is not a setting — it is the contract the engine's mesh
/// sculptor works under, and the reason Inflar and Suavizar behave differently
/// here than on a field. Stated rather than implied, because a sculptor who
/// does not know it reads the difference as a bug.
///
/// The counts are deliberately not repeated: the geometry section above states
/// them for what is drawn, and they are the scene's rather than this layer's.
/// Two numbers under two headings that disagree about what they count is worse
/// than one.
pub(super) fn show(ui: &mut egui::Ui, state: &ShellState<'_>) {
    let s = state.strings;
    if !heading(ui, s.section_mesh) {
        return;
    }
    ui.label(
        egui::RichText::new(s.mesh_topology_fixed)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
}
