//! The right region: what can be controlled about what is being sculpted.
//!
//! The material, the geometry it has produced, the rig, and the brush's
//! secondary settings — plus the sections that appear only while something is
//! being placed or cut, which live in `shapes` and `booleans` and are drawn
//! from here so the region has one order.

use super::*;

/// The curve being placed: how thick, how its points join, what it sweeps.
///
/// Only while one is up. Placing one is a menu entry, and a section that stood
/// there whether or not a curve existed would push the rest of the panel down
/// for nothing — the same bargain the cage's section makes.
pub(super) fn curve_section(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    use clayspace_model::{CurveJoin, CurveProfile};
    let s = state.strings;
    if !heading(ui, s.section_curve) {
        return;
    }

    if let Some(value) = slider(ui, s.label_curve_radius, state.curve_radius, 0.01..=0.6, 3) {
        queue.push(Command::SetCurveRadius(value));
    }

    ui.label(
        egui::RichText::new(s.label_curve_join)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    ui.horizontal_wrapped(|ui| {
        for join in CurveJoin::ALL {
            if ui
                .selectable_label(state.curve.join == join, s.curve_join_name(join))
                .clicked()
            {
                queue.push(Command::SetCurveJoin(join));
            }
        }
    });

    ui.label(
        egui::RichText::new(s.label_curve_profile)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    ui.horizontal_wrapped(|ui| {
        for profile in CurveProfile::ALL {
            if ui
                .selectable_label(
                    state.curve.profile == profile,
                    s.curve_profile_name(profile),
                )
                .clicked()
            {
                queue.push(Command::SetCurveProfile(profile));
            }
        }
    });

    // Enabled only once there is something to sweep along: one point is a
    // point, and the engine refuses a guide below two.
    if ui
        .add_enabled(
            state.curve.can_be_swept(),
            egui::Button::new(s.action_curve_apply),
        )
        .clicked()
    {
        queue.push(Command::ApplyCurve);
    }
    ui.label(
        egui::RichText::new(s.hint_curve)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
}

/// How a voxel layer is drawn.
///
/// A display setting and nothing more: the engine keeps the choice an argument
/// rather than grid state so two hosts sharing a document cannot disagree
/// about what it looks like, and nothing here touches a cell.
pub(super) fn voxel_section(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    use clayspace_model::{SmoothBlur, VoxelDisplay};
    let s = state.strings;
    if !heading(ui, s.section_geometry) {
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

/// The cage a sculptor is working in: how fine it is, and applying it.
///
/// Only while one is up. A cage is raised from the Dinâmica menu, and a
/// section that stood there whether or not one existed pushed everything below
/// it past the bottom of the panel.
pub(super) fn lattice_section(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    if !heading(ui, s.section_lattice) {
        return;
    }

    // Uniform, because a cage a sculptor can reason about is a grid rather
    // than a lattice of three different resolutions — and because the one
    // number is what both engine routes clamp.
    let limit = clayspace_model::division_limit(state.representation).unwrap_or(2);
    if let Some(value) = slider(
        ui,
        s.label_cage_divisions,
        state.lattice_divisions[0] as f32,
        clayspace_model::MIN_DIVISIONS as f32..=limit as f32,
        0,
    ) {
        queue.push(Command::SetLatticeDivisions([value.round() as i32; 3]));
    }

    // The manipulator's three modes, where the cage is worked. One widget
    // with three modes rather than three widgets is what ZBrush and Maya both
    // settled on: the sculptor's hand stays in the same place and the mode is
    // what changes.
    // Turning and scaling act about the middle of the selection, and one
    // point's middle is itself — so on a selection of one they are exactly no
    // movement. Disabled with the reason on them rather than drawn live and
    // inert, which is how they were: the rings appeared, the drag ran, and
    // nothing moved.
    let can_transform = state.lattice.can_transform();
    gizmo_mode_row(ui, state, state.lattice.mode, can_transform, queue);
    if !can_transform {
        ui.label(
            egui::RichText::new(s.hint_gizmo_needs_two)
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }

    if ui.button(s.action_bend).clicked() {
        queue.push(Command::ApplyLattice);
    }
    ui.label(
        egui::RichText::new(s.hint_cage)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    // Only while turning. The outer ring and the snap modifier mean nothing in
    // the other two modes, and a hint that is always there is a hint nobody
    // reads by the third time they see it.
    if state.lattice.mode == GizmoMode::Rotate && can_transform {
        ui.label(
            egui::RichText::new(s.hint_gizmo_rotate)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
    }
}

/// What the mask operations act with.
///
/// Here rather than in the menu because these are amounts, not actions: the
/// menu had `Expandir` fixed at one cell and an extrusion fixed at every
/// default it was born with, so three of the six operations took a number
/// nobody could set and the extrusion took four.
///
/// Shown only once a mask exists, which is also when every operation but
/// Limpar becomes usable.
pub(super) fn mask_section(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    if !heading(ui, s.section_mask) {
        return;
    }
    readout(ui, s.label_mask_cells, thousands(state.mask.painted_cells));
    if let Some(value) = slider(
        ui,
        s.label_mask_steps,
        state.mask_steps as f32,
        1.0..=16.0,
        0,
    ) {
        queue.push(Command::SetMaskSteps(value.round() as i32));
    }

    // The extrusion's own four, of which only the side was ever reachable.
    if let Some(value) = slider(
        ui,
        s.label_extrude_thickness,
        state.extrude.thickness,
        0.005..=0.5,
        3,
    ) {
        queue.push(Command::SetExtrudeSettings(ExtrudeSettings {
            thickness: value,
            ..state.extrude
        }));
    }
    if let Some(value) = slider(
        ui,
        s.label_extrude_round,
        state.extrude.border_round,
        0.0..=0.2,
        3,
    ) {
        queue.push(Command::SetExtrudeSettings(ExtrudeSettings {
            border_round: value,
            ..state.extrude
        }));
    }
    if let Some(value) = slider(
        ui,
        s.label_extrude_smooth,
        state.extrude.border_smooth as f32,
        0.0..=16.0,
        0,
    ) {
        queue.push(Command::SetExtrudeSettings(ExtrudeSettings {
            border_smooth: value.round() as i32,
            ..state.extrude
        }));
    }
}

/// Material, geometry, resolution and brush controls.
pub fn right_panel(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    material_section(ui, state, queue);

    // The two placing sections, where they stand beside the sculpt they act
    // on rather than over it.
    if state.show_shapes {
        shapes_section(ui, state, queue);
    }
    if state.show_boolean {
        boolean_section(ui, state, queue);
    }

    geometry_section(ui, state);
    if state.armature.exists {
        armature_section(ui, state, queue);
    }

    // A grid is boxes; whether it should *look* like boxes is a separate
    // question, and the answer belongs beside the layer it is asked about.
    if state.representation == Representation::Voxel {
        voxel_section(ui, state, queue);
    }
    if state.curve.active {
        curve_section(ui, state, queue);
    }
    if state.lattice.active {
        lattice_section(ui, state, queue);
    }
    // What is frozen rather than whether a mask handle exists: a mask belongs
    // to the document's layer and stays attached once painted, so Limpar
    // leaves an empty one behind and a panel keyed on its existence would sit
    // there offering to invert, expand and extrude nothing.
    if state.mask.is_active() {
        mask_section(ui, state, queue);
    }

    brush_controls_section(ui, state, queue);
}

/// The material: its preview, its name, and how many there are to step through.
pub(super) fn material_section(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    if !heading(ui, s.section_material) {
        return;
    }
    ui.horizontal(|ui| {
        // The material preview: a shaded sphere, which is where the design
        // spends its skeuomorphic budget.
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(size::SWATCH, size::SWATCH), egui::Sense::click());
        paint_matcap(ui, rect, state.matcap);
        if response.on_hover_text(s.hint_material).clicked() {
            queue.push(Command::NextMaterial);
        }
        ui.add_space(space::SNUG);
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(state.material)
                    .size(type_scale::BODY)
                    .color(Tokens::text()),
            );
            ui.label(
                egui::RichText::new(format!("{} materiais", state.materials.len()))
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
        });
    });
}

/// What the scene is made of, in numbers.
pub(super) fn geometry_section(ui: &mut egui::Ui, state: &ShellState<'_>) {
    let s = state.strings;
    if !heading(ui, s.section_geometry) {
        return;
    }
    // A count without its detail level reads as a smaller model, so where the
    // viewport is not showing full resolution the interface says so.
    if let Some(note) = s.detail_note(state.stats.detail) {
        ui.label(
            egui::RichText::new(note)
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }
    readout(ui, s.label_polygons, thousands(state.stats.triangles));
    readout(ui, s.label_vertices, thousands(state.stats.vertices));
    readout(ui, s.label_triangles, thousands(state.stats.triangles));
    readout(ui, s.label_objects, format!("{}", state.stats.objects));
}

/// The rig: how many spheres, how thick a skin, and the gestures while it is
/// being edited.
pub(super) fn armature_section(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    if !heading(ui, s.section_armature) {
        return;
    }
    readout(ui, s.label_spheres, format!("{}", state.armature.spheres));
    if let Some(value) = slider(ui, s.label_skin, state.armature.skin, 0.5..=3.0, 2) {
        queue.push(Command::SetSkinThickness(value));
    }
    let mut mirror = state.armature.mirror;
    if ui.checkbox(&mut mirror, s.label_mirror_new).clicked() {
        queue.push(Command::SetArmatureMirror(mirror));
    }
    if state.armature.editing {
        // The gestures, where a person is when they need them. ZBrush
        // teaches these by tutorial; one line costs nothing.
        ui.label(
            egui::RichText::new(s.hint_armature)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        if ui
            .add_enabled(
                state.armature.selection,
                egui::Button::new(s.action_armature_remove),
            )
            .clicked()
        {
            queue.push(Command::RemoveZsphere);
        }
    }
}

/// How a stroke is shaped, beyond its size and strength: noise, edge,
/// accumulation and smoothing.
pub(super) fn brush_controls_section(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    if !heading(ui, s.section_brush_controls) {
        return;
    }
    if let Some(value) = slider(ui, s.label_noise, state.brush.shaping.noise, 0.0..=1.0, 2) {
        queue.push(Command::SetBrushNoise(value));
    }
    ui.label(
        egui::RichText::new(s.label_edge)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    if let Some(falloff) = segmented(
        ui,
        &Falloff::ALL,
        |falloff| s.falloff_name(falloff),
        state.brush.shaping.falloff,
    ) {
        queue.push(Command::SetBrushFalloff(falloff));
    }

    let mut accumulate = state.brush.shaping.accumulate;
    if ui.checkbox(&mut accumulate, s.label_accumulate).changed() {
        queue.push(Command::SetBrushAccumulate(accumulate));
    }
    if let Some(value) = slider(
        ui,
        s.label_smoothing,
        state.brush.shaping.smoothing,
        0.0..=0.95,
        2,
    ) {
        queue.push(Command::SetBrushSmoothing(value));
    }
}
