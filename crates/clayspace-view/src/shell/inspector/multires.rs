//! A multiresolution surface: a cage, levels over it, and detail per level.

use super::*;

/// Where the offer to add a level was drawn. Absent for every representation
/// but a hierarchy.
pub fn subdivide_button_id() -> egui::Id {
    egui::Id::new("multires-subdivide")
}

/// The two levels, what one more would cost, and the contract that explains
/// the brushes.
///
/// The contract first, because a sculptor who does not know that the level
/// being written and the level being drawn are two numbers reads a dab that
/// lands softer than the pointer suggests as a bug in the brush.
///
/// Then the numbers themselves, which are the one item from the concept art's
/// list of per-layer controls that has stopped being unexpressible — see this
/// module's parent. They are drawn as two rows rather than one, because
/// collapsing them into a single "current level" would offer only "sculpt
/// coarse and look coarse" or "sculpt fine and look fine", which are the two
/// things a plain mesh already does.
///
/// The pass stack is not here. It is nested under the layer it stands on in
/// the left stack, for the same reason a grid's is: a pass has no meaning
/// apart from the surface it was recorded on.
pub(super) fn show(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    use clayspace_model::MultiresLevelOp;

    let s = state.strings;
    if !heading(ui, s.section_multires) {
        return;
    }
    ui.label(
        egui::RichText::new(s.multires_two_levels)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );

    let Some(hierarchy) = state
        .scene
        .active_layer()
        .and_then(|layer| layer.multires.as_ref())
    else {
        return;
    };
    let levels = hierarchy.levels;
    readout(ui, s.label_multires_levels, levels.count.to_string());

    // Both rows are drawn even where there is only the cage, and the sliders
    // then have no travel — which is the honest picture of a hierarchy nobody
    // has subdivided, and is better than two controls that appear once a
    // button has been pressed.
    let highest = levels.highest() as f32;
    if let Some(level) = slider(
        ui,
        s.label_multires_sculpt_level,
        levels.sculpt as f32,
        0.0..=highest,
        0,
    ) {
        queue.push(Command::MultiresLevel(MultiresLevelOp::SetSculptLevel(
            level.round().max(0.0) as u32,
        )));
    }
    if let Some(level) = slider(
        ui,
        s.label_multires_display_level,
        levels.display as f32,
        0.0..=highest,
        0,
    ) {
        queue.push(Command::MultiresLevel(MultiresLevelOp::SetDisplayLevel(
            level.round().max(0.0) as u32,
        )));
    }
    // The state that reads as a brush not working, said where it is true.
    if !levels.draws_what_it_sculpts() {
        ui.label(
            egui::RichText::new(s.hint_multires_levels_apart)
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }

    ui.horizontal(|ui| {
        // Enabled, and refused by the model where it has to be — the same
        // arrangement the rebuild button takes, and for a sharper reason here:
        // the engine prices the level against its own budget and refuses over
        // it *without attempting it*, so the refusal is exact where a guess
        // from the numbers this side holds would not be. It arrives beside the
        // viewport, through the scene's refusal.
        let button = ui.button(s.multires_subdivide);
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(subdivide_button_id(), button.rect));
        if button.clicked() {
            queue.push(Command::MultiresLevel(MultiresLevelOp::AddLevel));
        }
        // What it would cost, beside the button that would spend it. The
        // **peak** rather than what remains: a level that fits once it is
        // built and does not fit while it is being built is a level that
        // cannot be added, and on a constrained machine the high-water mark is
        // what ends the session.
        if let Some(cost) = state.subdivision_cost {
            ui.label(
                egui::RichText::new(format!(
                    "{} · {} {}",
                    thousands(cost.faces as usize),
                    megabytes(cost.peak_bytes),
                    s.multires_peak
                ))
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
            );
        }
    });

    release_caches(ui, s, hierarchy, queue);
    bake_report(ui, s, hierarchy);
}

/// The one lever a sculptor has over what a hierarchy holds without losing
/// anything, and what it gave back the last time it was pulled.
///
/// The figure is the hierarchy's own total either side of the release rather
/// than an estimate beforehand: what is rebuildable depends on which levels
/// were evaluated since, and a prediction that was wrong would be worse than
/// the measurement after.
fn release_caches(
    ui: &mut egui::Ui,
    s: &crate::strings::Strings,
    hierarchy: &clayspace_model::MultiresState,
    queue: &mut CommandQueue,
) {
    use clayspace_model::MultiresLevelOp;

    ui.horizontal(|ui| {
        if ui.button(s.multires_release_caches).clicked() {
            queue.push(Command::MultiresLevel(MultiresLevelOp::ReleaseCaches));
        }
        if let Some(release) = hierarchy.last_release {
            ui.label(
                egui::RichText::new(format!(
                    "{} {}",
                    megabytes(release.freed_bytes()),
                    s.multires_freed
                ))
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
            );
        }
    });
    if hierarchy
        .last_release
        .is_some_and(|release| !release.detail_kept)
    {
        ui.label(
            egui::RichText::new(s.multires_release_touched_detail)
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }
}

/// What a bake into a mesh would take and what it would drop, said before the
/// crossing is chosen rather than discovered after it.
fn bake_report(
    ui: &mut egui::Ui,
    s: &crate::strings::Strings,
    hierarchy: &clayspace_model::MultiresState,
) {
    let bake = hierarchy.bake();
    ui.label(
        egui::RichText::new(format!(
            "{} {} · {} {} · {} {}",
            s.multires_bake_takes,
            bake.level,
            bake.finer_levels_dropped,
            s.multires_bake_finer_lost,
            bake.passes_dropped,
            s.multires_bake_passes_lost
        ))
        .size(type_scale::LABEL)
        .color(if bake.loses_detail() {
            Tokens::accent()
        } else {
            Tokens::text_dim()
        }),
    );
}

/// A byte count as whole megabytes, which is the size a subdivision is
/// discussed in.
fn megabytes(bytes: u64) -> String {
    format!("{} MB", bytes / (1024 * 1024))
}
