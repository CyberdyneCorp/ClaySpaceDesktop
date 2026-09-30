//! A mesh layer: triangles, held verbatim.

use super::*;

/// The one thing that is true of every mesh layer and explains its brushes,
/// and the way from a fixed mesh to a hierarchy.
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
pub(super) fn show(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    if !heading(ui, s.section_mesh) {
        return;
    }
    ui.label(
        egui::RichText::new(s.mesh_topology_fixed)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    uv_display(ui, state, queue);
    create_multires(ui, state, queue);
}

/// How the layer's UV layout is drawn, where it carries one.
///
/// Not here while a retopology preview is held: the display is then about the
/// preview, and the retopology panel beside it offers it.
fn uv_display(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if state.retopo_pending {
        return;
    }
    super::super::uv_layout::uv_display_controls(ui, state, queue);
}

/// Where the Create Multires button was drawn, for a test to press it.
pub fn create_multires_button_id() -> egui::Id {
    egui::Id::new("create-multires")
}

/// What the Create Multires controls are set to, between frames.
///
/// The interface's own memory rather than the application's: nothing reads
/// the depth until the button carries it, and an agent passes its own.
fn settings_id() -> egui::Id {
    egui::Id::new("create-multires-settings")
}

/// Create Multires: how deep, whether the mesh is replaced, what it would
/// cost against what the document holds — said before the button is pressed —
/// and the button.
///
/// The button stays enabled over budget, as Subdivide does: the refusal comes
/// back from the model in the same words the line above it already shows.
fn create_multires(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.add_space(6.0);
    ui.label(egui::RichText::new(s.create_multires).size(type_scale::LABEL));
    ui.label(
        egui::RichText::new(s.create_multires_hint)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    let mut settings = ui
        .ctx()
        .data(|data| data.get_temp::<clayspace_model::HierarchySettings>(settings_id()))
        .unwrap_or_default();
    let range = clayspace_model::HierarchySettings::LEVELS;
    if let Some(levels) = slider(
        ui,
        s.label_create_multires_levels,
        settings.levels as f32,
        *range.start() as f32..=*range.end() as f32,
        0,
    ) {
        settings.levels = levels.round().max(0.0) as u32;
    }
    ui.checkbox(&mut settings.in_place, s.create_multires_in_place);
    let settings = settings.sanitized();
    ui.ctx()
        .data_mut(|data| data.insert_temp(settings_id(), settings));

    match &state.hierarchy_plan {
        Some(Ok(plan)) => price(ui, s, plan, settings.levels),
        Some(Err(why)) => warn(ui, why),
        None => {}
    }
    let button = ui.button(s.create_multires);
    ui.ctx().memory_mut(|memory| {
        memory
            .data
            .insert_temp(create_multires_button_id(), button.rect)
    });
    if button.clicked() {
        queue.push(Command::CreateHierarchy(settings));
    }
}

/// The three figures a Create Multires is priced in, the top level's faces,
/// and the refusal where the hierarchy would not fit.
fn price(
    ui: &mut egui::Ui,
    s: &crate::strings::Strings,
    plan: &clayspace_model::HierarchyPlan,
    levels: u32,
) {
    let projected = if clayspace_model::HierarchyPlan::is_quoted(levels) {
        String::new()
    } else {
        format!(" ({})", s.create_multires_projected)
    };
    let dim = |text: String| {
        egui::RichText::new(text)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim())
    };
    ui.label(dim(format!(
        "{} {}{projected}",
        thousands(plan.faces(levels) as usize),
        s.create_multires_faces,
    )));
    ui.label(dim(format!(
        "{} {} · {} {} · {} {}",
        s.create_multires_held,
        super::multires::megabytes(plan.held_bytes),
        s.create_multires_adds,
        super::multires::megabytes(plan.hierarchy_bytes(levels)),
        s.create_multires_limit,
        super::multires::megabytes(plan.budget_bytes),
    )));
    if let Err(refusal) = plan.within(levels) {
        warn(ui, &refusal.to_string());
    }
}

fn warn(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(type_scale::LABEL)
            .color(Tokens::accent()),
    );
}
