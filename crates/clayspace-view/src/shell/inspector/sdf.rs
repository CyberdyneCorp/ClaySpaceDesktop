//! A field layer: an ordered edit list, evaluated as a distance field.

use super::*;

/// How long the list is, and whether it has been collapsed.
///
/// Two readouts and no controls, because a field's controls are not the
/// layer's. The combine operation, its join profile and its radius belong to
/// the *stroke* and stand in the options bar where the stroke's other numbers
/// are; the offer to collapse a costly list appears under the layer list, and
/// only while the engine is advising it — a row that is always there is a row
/// nobody reads.
///
/// What is left is worth stating plainly: the list's length is the one number
/// that explains why a field has become slow to march, and whether it has been
/// collapsed is the one piece of state a sculptor cannot otherwise see.
pub(super) fn show(ui: &mut egui::Ui, state: &ShellState<'_>) {
    let s = state.strings;
    // Asked *before* the heading. The engine reports this cheaply and only for
    // a layer that has a field; where it has not reported yet there is nothing
    // to put under the word, and a heading standing over nothing is both a
    // question left unanswered and a section's worth of height taken from the
    // panel below it — which is how adding this section pushed the mask
    // controls off the bottom of the right region.
    //
    // Nothing rather than a zero, too: a count of nought and a count nobody
    // has taken are different things.
    let Some(health) = state.scene.active_layer().and_then(|layer| layer.health) else {
        return;
    };
    if !heading(ui, s.section_field) {
        return;
    }
    readout(ui, s.label_field_items, format!("{}", health.items));
    readout(
        ui,
        s.label_field_collapsed,
        if health.consolidated {
            s.state_yes
        } else {
            s.state_no
        },
    );
}
