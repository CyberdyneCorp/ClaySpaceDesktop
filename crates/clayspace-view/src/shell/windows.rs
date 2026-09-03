//! The panels that open over the work rather than beside it.
//!
//! Diagnostics, attribution, repair, conversion, import, export, the
//! deformations and the reference images. Each is drawn only while its toggle
//! holds, and each closes from the same command its rail button and its menu
//! entry push.

use super::*;

/// One plane's reference, as the panel needs it.
///
/// The file's name and not its pixels: the interface says which drawing is on
/// which plane and has no business holding megabytes to do it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReferenceSlot<'a> {
    pub name: Option<&'a str>,
    pub settings: ReferenceSettings,
}

/// The reference images: one drawing a plane, sat behind the sculpt.
///
/// A panel rather than an inspector section, because it is not about the
/// active layer — a reference outlives every layer in the document and is not
/// in the document at all.
pub fn reference_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_references {
        return;
    }
    let s = state.strings;
    let mut open = true;

    egui::Window::new(s.action_references)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(320.0);
            // The clay's own opacity, above the three planes rather than
            // inside one: it is not a property of any reference. It lives here
            // because this is the panel a sculptor opens when the answer they
            // want is "let me see the drawing through the model", and a
            // control filed by what it belongs to rather than by what it is
            // for is a control nobody finds.
            if let Some(opacity) = slider(
                ui,
                s.label_surface_opacity,
                state.surface_opacity.get(),
                SurfaceOpacity::FAINTEST..=1.0,
                2,
            ) {
                queue.push(Command::SetSurfaceOpacity(SurfaceOpacity::new(opacity)));
            }
            if !state.surface_opacity.is_solid() {
                ui.label(
                    egui::RichText::new(s.hint_surface_opacity)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
            }
            ui.separator();

            for plane in RefPlane::ALL {
                reference_plane(ui, s, plane, state.references[plane as usize], queue);
            }
        });
    if !open {
        queue.push(Command::ToggleReferences);
    }
}

/// What a reference panel's slider is recorded under.
///
/// The same four controls are drawn once a plane, so the plane is part of the
/// name — the label a sculptor reads stays short, under its own heading.
pub fn reference_slider_name(plane: RefPlane, label: &str) -> String {
    format!("{} {label}", plane.tag())
}

/// One plane's row: the file, and the four numbers that place it.
pub(super) fn reference_plane(
    ui: &mut egui::Ui,
    s: &Strings,
    plane: RefPlane,
    slot: ReferenceSlot<'_>,
    queue: &mut CommandQueue,
) {
    let settings = slot.settings;
    if !heading(ui, s.ref_plane_name(plane)) {
        return;
    }
    ui.horizontal(|ui| match slot.name {
        Some(name) => {
            // Shown, and only then, because there is nothing to hide.
            let mut visible = settings.visible;
            if ui.checkbox(&mut visible, name).changed() {
                queue.push(Command::SetReferenceSettings(
                    plane,
                    ReferenceSettings {
                        visible,
                        ..settings
                    },
                ));
            }
            if ui.button(s.action_clear_reference).clicked() {
                queue.push(Command::ClearReference(plane));
            }
        }
        None => {
            ui.label(
                egui::RichText::new(s.reference_none)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            if ui.button(s.action_load_reference).clicked() {
                queue.push(Command::LoadReference(plane));
            }
        }
    });

    // Nothing to place while the plane is empty, and a row of dead sliders
    // reads as a broken panel rather than an empty one.
    if slot.name.is_none() {
        return;
    }

    let mut place = |label: &str, value: f32, range, decimals| {
        slider_named(
            ui,
            &reference_slider_name(plane, label),
            label,
            value,
            range,
            decimals,
        )
    };

    if let Some(opacity) = place(s.label_reference_opacity, settings.opacity, 0.0..=1.0, 2) {
        queue.push(Command::SetReferenceSettings(
            plane,
            ReferenceSettings {
                opacity,
                ..settings
            },
        ));
    }
    // Reachable ranges rather than the domain's own clamps, which run to a
    // hundred: a slider spanning that moves a reference by a whole extent per
    // pixel and cannot be used to line one up.
    if let Some(height) = place(s.label_reference_size, settings.height, 0.05..=REACH, 2) {
        queue.push(Command::SetReferenceSettings(
            plane,
            ReferenceSettings { height, ..settings },
        ));
    }
    for (label, axis) in [
        (s.label_reference_across, 0usize),
        (s.label_reference_up, 1usize),
    ] {
        if let Some(value) = place(label, settings.offset[axis], -REACH..=REACH, 2) {
            let mut offset = settings.offset;
            offset[axis] = value;
            queue.push(Command::SetReferenceSettings(
                plane,
                ReferenceSettings { offset, ..settings },
            ));
        }
    }
    if let Some(depth) = place(s.label_reference_depth, settings.depth, -REACH..=REACH, 2) {
        queue.push(Command::SetReferenceSettings(
            plane,
            ReferenceSettings { depth, ..settings },
        ));
    }
}

/// How far a reference's sliders reach, in document units.
///
/// Enough to place one around a form a few units across, and no more. A value
/// outside it can still be held — the domain clamps at a hundred — but a
/// sculptor lining a drawing up wants a slider they can aim.
pub(super) const REACH: f32 = 10.0;

/// The whole-form deformers, and what each one is set to.
///
/// A panel rather than a gesture: a deformer states something about the *form*
/// — no centre, no radius, no falloff — so there is nothing for a drag to be
/// resolved from. The lattice is the third deformer and is absent here for the
/// opposite reason: it *is* a drag, of a cage's control points, and four
/// numbers cannot say what it does.
pub fn deform_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_deform {
        return;
    }
    let s = state.strings;
    let mut open = true;
    let mut settings = state.deform;

    egui::Window::new(s.action_deform)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(300.0);
            if state.representation != Representation::Mesh {
                ui.label(
                    egui::RichText::new(s.deform_mesh_only)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
                return;
            }

            // The two deformations as chips with their shape on them, the
            // way the manipulator's modes are: what a deformation does is a
            // picture before it is a word.
            ui.horizontal(|ui| {
                for verb in DeformVerb::ALL {
                    let chosen = settings.verb == verb;
                    let icon = match verb {
                        DeformVerb::Taper => Icon::Taper,
                        DeformVerb::Twist => Icon::Twist,
                    };
                    if icon_chip(ui, icon, s.deform_verb_name(verb), chosen, Tokens::panel())
                        .clicked()
                        && !chosen
                    {
                        settings.verb = verb;
                        queue.push(Command::SetDeform(settings));
                    }
                }
            });

            ui.add_space(space::SNUG);
            if let Some(axis) = axis_control(ui, s.label_axis, settings.axis) {
                queue.push(Command::SetDeform(DeformSettings { axis, ..settings }));
            }

            if let Some(span) = slider(
                ui,
                s.label_span,
                settings.span,
                DeformSettings::SPAN_RANGE,
                2,
            ) {
                queue.push(Command::SetDeform(DeformSettings { span, ..settings }));
            }

            // Only the controls the chosen verb reads. A scale beside a twist
            // is a number that does nothing, and there is no way for the panel
            // to say so that is better than not drawing it.
            if settings.verb.takes_a_scale() {
                if let Some(scale_start) = slider(
                    ui,
                    s.label_scale_start,
                    settings.scale_start,
                    DeformSettings::SCALE_RANGE,
                    2,
                ) {
                    queue.push(Command::SetDeform(DeformSettings {
                        scale_start,
                        ..settings
                    }));
                }
                if let Some(scale_end) = slider(
                    ui,
                    s.label_scale_end,
                    settings.scale_end,
                    DeformSettings::SCALE_RANGE,
                    2,
                ) {
                    queue.push(Command::SetDeform(DeformSettings {
                        scale_end,
                        ..settings
                    }));
                }
            }
            if settings.verb.takes_an_angle() {
                if let Some(degrees) = slider(
                    ui,
                    s.label_angle,
                    settings.degrees,
                    DeformSettings::DEGREES_RANGE,
                    0,
                ) {
                    queue.push(Command::SetDeform(DeformSettings {
                        degrees,
                        ..settings
                    }));
                }
            }

            ui.add_space(space::SECTION);
            if ui.button(s.deform_verb_name(settings.verb)).clicked() {
                queue.push(Command::RunDeform);
            }
        });
    if !open {
        queue.push(Command::ToggleDeform);
    }
}

/// Three sliders for a direction, returning the axis when one moves.
pub(super) fn axis_control(ui: &mut egui::Ui, label: &str, axis: [f32; 3]) -> Option<[f32; 3]> {
    ui.label(
        egui::RichText::new(label)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    let mut changed = axis;
    let mut moved = false;
    ui.horizontal(|ui| {
        for (index, name) in ["X", "Y", "Z"].into_iter().enumerate() {
            let mut value = axis[index];
            if ui
                .add(egui::DragValue::new(&mut value).speed(0.02).prefix(name))
                .changed()
            {
                changed[index] = value;
                moved = true;
            }
        }
    });
    moved.then_some(changed)
}

/// The diagnostics report, as a window rather than a panel.
///
/// A window because it is read rarely and copied whole: docking it would cost
/// a permanent strip of the interface for something a person opens twice a
/// year, and then only when something has already gone wrong.
///
/// Every value is a readout the reader can compare against an issue, and the
/// copy button takes the lot. A report that has to be retyped is one that
/// arrives with a digit wrong.
pub fn diagnostics_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_diagnostics {
        return;
    }
    let s = state.strings;
    let mut open = true;
    egui::Window::new(s.action_diagnostics)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(360.0);
            let d = state.diagnostics;

            if heading(ui, s.section_diagnostics) {
                diagnostics_build(ui, d);
            }
            if heading(ui, s.label_backend) {
                diagnostics_backend(ui, d);
            }
            if d.render.is_some() && heading(ui, s.section_rendering) {
                diagnostics_render(ui, d);
            }
            if d.mesh.is_some() && heading(ui, s.section_mesh_sculpting) {
                diagnostics_mesh(ui, d);
            }
            if d.memory.is_some() && heading(ui, s.section_memory) {
                diagnostics_memory(ui, d);
            }

            ui.add_space(space::SNUG);
            ui.horizontal(|ui| {
                if ui.button(s.action_copy).clicked() {
                    queue.push(Command::CopyDiagnostics);
                }
                if state.diagnostics_copied {
                    ui.label(
                        egui::RichText::new(s.state_copied)
                            .size(type_scale::LABEL)
                            .color(Tokens::accent()),
                    );
                }
            });
        });

    // The window's own close button and the menu entry mean the same thing, so
    // they emit the same command rather than each owning a copy of the state.
    if !open {
        queue.push(Command::ToggleDiagnostics);
    }
}

/// What was built: the application, the engine and where they came from.
pub(super) fn diagnostics_build(ui: &mut egui::Ui, d: &Diagnostics) {
    readout(ui, "Aplicação", d.app_version.clone());
    readout(ui, "Motor", d.engine_version.clone());
    readout(ui, "Revisão", d.engine_revision.clone());
    readout(ui, "Plataforma", d.platform.clone());
}

/// What is running: the backends, the one chosen, and what went wrong on it.
pub(super) fn diagnostics_backend(ui: &mut egui::Ui, d: &Diagnostics) {
    readout(ui, "Disponíveis", d.backends.join(", "));
    readout(
        ui,
        "Ativo",
        format!("{} — {}", d.active_backend, d.selection),
    );
    if let Some(renderer) = &d.renderer {
        readout(ui, "Vídeo", renderer.clone());
    }

    // The stalls, which are what "it stutters" turns into. Listed even
    // when there are none, for the same reason as the fallbacks below.
    if d.stalls.is_empty() {
        readout(ui, "Travamentos", "nenhum acima de um quadro");
    } else {
        for stall in &d.stalls {
            readout(ui, "Travamento", stall.clone());
        }
    }

    // Fallbacks are listed even when there are none. Silence here reads
    // as "the panel is broken" rather than as "nothing fell back", and
    // a reader cannot tell the two apart.
    if d.fallbacks.is_empty() {
        readout(ui, "Alternativas", "nenhuma nesta sessão");
    } else {
        for fallback in &d.fallbacks {
            readout(
                ui,
                "Alternativa",
                format!("{} recusou {}", fallback.declined_by, fallback.operation),
            );
        }
    }
}

/// What mesh sculpting has had to correct for itself.
///
/// Two numbers, and they are here because what they count is otherwise
/// invisible:
/// a brush handed a seed from a numbering that has been retired reaches
/// nothing, and a stroke that reached nothing looks exactly like a stroke over
/// a frozen mask. Reported at zero for the same reason the fallbacks are —
/// silence would read as a broken panel rather than as a quiet session.
pub(super) fn diagnostics_mesh(ui: &mut egui::Ui, d: &Diagnostics) {
    let Some(mesh) = &d.mesh else {
        return;
    };
    readout(ui, "Esculturas em malha", format!("{}", mesh.sculptors));
    readout(
        ui,
        "Sementes recusadas",
        format!("{}", mesh.stale_seeds_rejected),
    );
}

/// Where the document's memory is, in the terms that decide what may go.
///
/// Three figures rather than one, because a total answers the wrong question.
/// A sculptor under memory pressure is not asking how big the document is,
/// they are asking which part they can let go of: the first line is their own
/// work and is never released, the second reconstructs identically and costs
/// only a stall, and the third is undo depth, which is a policy rather than a
/// fact about the sculpture.
///
/// The last row is the honesty check. A mesh-sculpting session is held beside
/// the document rather than inside it, so the engine's own roll-up reports it
/// as nothing — correctly, since it cannot walk what it does not own. This
/// application asks each session what it costs and folds the answers in, and
/// the row says how many it asked, so a surface tier of zero reads as "there
/// are none" rather than as "nobody asked".
pub(super) fn diagnostics_memory(ui: &mut egui::Ui, d: &Diagnostics) {
    let Some(m) = &d.memory else {
        return;
    };
    readout(ui, "Trabalho", megabytes(m.essential));
    readout(ui, "Reconstruível", megabytes(m.rebuildable));
    readout(ui, "Desfazer", megabytes(m.undoable));
    readout(ui, "Total", megabytes(m.total));
    readout(
        ui,
        "Superfícies",
        format!("{} · {}", m.surfaces, megabytes(m.surface_bytes)),
    );
}

/// Bytes as a figure a sculptor can hold against what the machine has.
fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

/// What the viewport drew, and what the device charged for it.
///
/// Here rather than in a developer-only overlay because it answers the two
/// questions a rendering report is actually opened for — "why is this slow"
/// and "is occlusion even running" — and a person who can reach the
/// diagnostics window can reach the answer.
pub(super) fn diagnostics_render(ui: &mut egui::Ui, d: &Diagnostics) {
    let Some(r) = &d.render else {
        return;
    };
    readout(
        ui,
        "Área",
        format!("{}×{} · {}× MSAA", r.viewport[0], r.viewport[1], r.samples),
    );
    match &r.ao {
        Some(ao) => readout(
            ui,
            "Oclusão",
            format!(
                "{}×{} · {} amostras · temporal {}",
                ao.width,
                ao.height,
                ao.samples,
                if ao.temporal { "ligada" } else { "desligada" }
            ),
        ),
        None => readout(ui, "Oclusão", "desligada".to_string()),
    }
    readout(
        ui,
        "Desenhos",
        format!(
            "{} · {} descartados · {} triângulos · {} linhas",
            r.draw_calls, r.culled, r.triangles, r.lines
        ),
    );
    readout(ui, "Enviado", format!("{} bytes", r.uploaded_bytes));

    // Listed even when the adapter cannot answer, for the reason the
    // fallbacks are: silence reads as a broken panel rather than as an
    // unmeasurable device, and a reader cannot tell the two apart.
    if !r.gpu_timing {
        readout(ui, "GPU", "sem marcas de tempo neste adaptador".to_string());
    } else if r.gpu_passes.is_empty() {
        readout(ui, "GPU", "nenhum quadro medido ainda".to_string());
    } else {
        let total: f32 = r.gpu_passes.iter().map(|(_, ms)| ms).sum();
        readout(ui, "GPU", format!("{total:.2} ms"));
        for (pass, ms) in &r.gpu_passes {
            readout(ui, pass, format!("{ms:.2} ms"));
        }
    }
}

/// What the application is built from, and on what terms.
///
/// Shown rather than only shipped beside the binary: the licence policy in
/// `deny.toml` is written on the understanding that attribution travels with
/// the distribution, and a file nobody can reach from the application is one
/// that goes missing the first time it is repackaged.
pub fn attribution_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_attribution {
        return;
    }
    let mut open = true;
    egui::Window::new(state.strings.action_attribution)
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(520.0, 420.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(
                    egui::RichText::new(state.attribution)
                        .size(type_scale::LABEL)
                        .family(egui::FontFamily::Monospace)
                        .color(Tokens::text_dim()),
                );
            });
        });
    if !open {
        queue.push(Command::ToggleAttribution);
    }
}

/// Bringing geometry in.
///
/// A panel rather than a bare file dialog, because the one real decision —
/// whether the model becomes a reference or becomes clay — cannot be made
/// after the fact, and a native dialog has nowhere to ask it.
/// Pre-bake repair: what is wrong with a grid, and the two verbs that fix it.
///
/// The report comes first and is shown whether or not anything is repaired. A
/// sealed void is invisible until something needs the model to be solid, so a
/// sculptor cannot see the problem by looking — and a repair that ran before
/// saying what it would change would be asking consent for something unstated.
pub fn repair_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_repair {
        return;
    }
    let s = state.strings;
    let mut open = true;
    egui::Window::new(s.action_repair)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(300.0);
            let Some(report) = state.repair else {
                ui.label(
                    egui::RichText::new(s.repair_voxel_only)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
                return;
            };

            if report.airtight {
                ui.label(
                    egui::RichText::new(s.repair_airtight)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
            } else {
                readout(ui, s.repair_voids, thousands(report.enclosed_voids));
                readout(ui, s.repair_largest, thousands(report.largest_void));
            }

            ui.add_space(space::SECTION);
            if ui.button(s.repair_close_holes).clicked() {
                queue.push(Command::CloseHoles);
            }
            // Offered only where there is something to fill: a button that can
            // only report having done nothing is worse than one that is not
            // there.
            if !report.airtight && ui.button(s.repair_fill_voids).clicked() {
                queue.push(Command::FillVoids);
            }
        });
    if !open {
        queue.push(Command::ToggleRepair);
    }
}

/// The conversion panel: where a layer crosses to another representation.
///
/// Its whole job is to state the losses *before* the crossing runs. They are
/// recomputed from the chosen cell size every frame rather than written into
/// the strings, because a number written down is wrong the first time somebody
/// changes the default — and the resolution is the whole of the decision being
/// made here.
pub fn convert_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_convert {
        return;
    }
    let s = state.strings;
    let mut open = true;
    let mut settings = state.conversion;
    let available = Direction::from_representation(state.representation);

    egui::Window::new(s.action_convert)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            if available.is_empty() {
                ui.label(
                    egui::RichText::new(s.convert_none_here)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
                return;
            }

            ui.label(
                egui::RichText::new(s.label_convert_to)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            for direction in &available {
                let chosen = settings.direction == *direction;
                if ui.radio(chosen, direction.to().label()).clicked() && !chosen {
                    settings.direction = *direction;
                    queue.push(Command::SetConversion(settings));
                }
            }

            if settings.direction.chooses_resolution() {
                ui.add_space(space::SNUG);
                ui.label(
                    egui::RichText::new(s.label_cell_size)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
                let mut cell = settings.cell_size;
                if ui
                    .add(
                        egui::Slider::new(
                            &mut cell,
                            clayspace_model::ConversionSettings::CELL_RANGE,
                        )
                        .logarithmic(true)
                        .show_value(true),
                    )
                    .changed()
                {
                    settings.cell_size = cell;
                    queue.push(Command::SetConversion(settings));
                }
            }

            ui.add_space(space::SECTION);
            ui.label(
                egui::RichText::new(s.label_convert_costs)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            for line in cost_lines(state, settings) {
                ui.label(
                    egui::RichText::new(line)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
            }
            // What the crossing costs in history, where a sculptor decides
            // whether to make one. It used to say a crossing could not be
            // undone; it can, and saying otherwise beside a control that
            // removes a layer would be the worst place to be out of date.
            ui.label(
                egui::RichText::new(s.convert_undo_note)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );

            ui.add_space(space::SNUG);
            // Adding a layer is the default because it cannot lose work.
            // Replacing is what a sculptor means by converting *this* layer,
            // and it is one undo away either way.
            let mut in_place = settings.in_place;
            if ui
                .checkbox(&mut in_place, s.convert_in_place)
                .on_hover_text(s.convert_in_place_hint)
                .changed()
            {
                queue.push(Command::SetConversion(
                    clayspace_model::ConversionSettings {
                        in_place,
                        ..settings
                    },
                ));
            }
            ui.label(
                egui::RichText::new(s.convert_in_place_hint)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );

            ui.add_space(space::SECTION);
            if ui.button(s.convert_run).clicked() {
                queue.push(Command::RunConversion);
            }
        });
    if !open {
        queue.push(Command::ToggleConvert);
    }
}

/// What the crossing costs, in the units the document is measured in.
///
/// Built from [`clayspace_model::Cost`] rather than from prose, so the figures
/// and the settings above them cannot disagree.
pub(super) fn cost_lines(
    state: &ShellState<'_>,
    settings: clayspace_model::ConversionSettings,
) -> Vec<String> {
    let Some(cost) = state.conversion_cost else {
        return Vec::new();
    };
    crossing_cost_lines(state, settings.direction, cost)
}

/// What a crossing costs, as lines a panel can print.
///
/// Shared by the conversion panel and the shapes panel, because they state the
/// same thing about the same crossing: placing a mesh as a boolean operand
/// pays exactly the crossing the conversion panel would run, and two panels
/// with two opinions about it would be two prices for one thing.
pub(super) fn crossing_cost_lines(
    state: &ShellState<'_>,
    direction: clayspace_model::Direction,
    cost: clayspace_model::Cost,
) -> Vec<String> {
    let s = state.strings;
    let mut lines = Vec::new();
    if direction.chooses_resolution() {
        lines.push(format!(
            "· {} {}",
            s.convert_surface_moves,
            state.units.format(cost.surface_movement)
        ));
        lines.push(format!(
            "· {} ({})",
            s.convert_features_vanish,
            state.units.format(cost.vanishing_feature)
        ));
        // Grouped, like the geometry readout: "1000000 cells" is a number a
        // reader has to count the digits of before it means anything.
        lines.push(format!(
            "· {} {}",
            thousands(cost.cells as usize),
            s.convert_cells
        ));
    }
    if !cost.keeps_sharp_edges {
        lines.push(format!("· {}", s.convert_sharp_edges_lost));
    }
    if !cost.keeps_history {
        lines.push(format!("· {}", s.convert_history_lost));
    }
    // Said before the crossing rather than discovered after it. What comes out
    // sculpts, and it sculpts by moving the vertices it was given — there is
    // no retopology in it to spend, and nothing here adds one.
    if cost.fixed_topology {
        lines.push(format!("· {}", s.convert_fixed_topology));
    }
    lines
}

pub fn import_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_import {
        return;
    }
    let s = state.strings;
    let mut open = true;
    let mut settings = state.import;
    egui::Window::new(s.action_import)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(320.0);
            ui.label(
                egui::RichText::new(s.label_import_as)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            for becomes in ImportAs::ALL {
                if ui
                    .radio(settings.becomes == becomes, becomes.label())
                    .on_hover_text(becomes.detail())
                    .clicked()
                {
                    settings.becomes = becomes;
                }
            }
            if let Some(value) = slider(ui, s.label_scale, settings.scale, 0.01..=100.0, 2) {
                settings.scale = value;
            }
            if settings != state.import {
                queue.push(Command::SetImportSettings(settings));
            }
            ui.add_space(space::SNUG);
            if ui.button(s.action_choose_file).clicked() {
                queue.push(Command::RunImport);
            }
        });
    if !open {
        queue.push(Command::ToggleImport);
    }
}

/// Writing geometry out, and saying beforehand what will not survive.
pub fn export_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_export {
        return;
    }
    let s = state.strings;
    let mut open = true;
    let mut settings = state.export;
    egui::Window::new(s.action_export)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.label(
                egui::RichText::new(s.label_mesher)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            for mesher in ExportMesher::ALL {
                let response = ui.radio(settings.mesher == mesher, mesher.label());
                let response = match mesher.caveat() {
                    Some(caveat) => response.on_hover_text(caveat),
                    None => response,
                };
                if response.clicked() {
                    settings.mesher = mesher;
                }
            }
            if let Some(value) = slider(
                ui,
                s.label_export_resolution,
                settings.resolution,
                0.005..=0.2,
                3,
            ) {
                settings.resolution = value;
            }

            // Decimation is off by default and expressed as a ratio, so the
            // checkbox and the slider are one control: unticking it means
            // "keep every triangle" rather than "keep 100% of them", which is
            // the same file by a slower route.
            let mut decimating = settings.decimate_to.is_some();
            if ui.checkbox(&mut decimating, s.label_decimate).clicked() {
                settings.decimate_to = decimating.then_some(0.5);
            }
            if let Some(ratio) = settings.decimate_to {
                // "Manter", not "Reduzir" again: the value is the share of
                // triangles kept, and labelling both the checkbox and the
                // slider the same way reads as one control repeated.
                if let Some(value) = slider(ui, s.label_keep, ratio, 0.05..=0.95, 2) {
                    settings.decimate_to = Some(value);
                }
            }
            if settings != state.export {
                queue.push(Command::SetExportSettings(settings));
            }

            // Before the write, not after. Every one of these is knowable now
            // and otherwise found out by opening the file somewhere else.
            if !state.export_warnings.is_empty() && heading(ui, s.section_warnings) {
                for warning in state.export_warnings {
                    ui.label(
                        egui::RichText::new(&warning.message)
                            .size(type_scale::LABEL)
                            .color(Tokens::accent()),
                    );
                }
            }

            ui.add_space(space::SNUG);
            if ui.button(s.action_choose_file).clicked() {
                queue.push(Command::RunExport);
            }
        });
    if !open {
        queue.push(Command::ToggleExport);
    }
}
