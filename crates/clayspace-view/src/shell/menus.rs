//! The menu bar, and what each menu offers.
//!
//! Every entry pushes the same command its rail button or its shortcut does,
//! under the same conditions — an entry that is grey here is grey there — so
//! the three routes to an action cannot disagree about whether it is legal.

use super::*;

/// The menu bar. Every item dispatches the same command its shortcut does.
pub fn menu_bar(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.horizontal(|ui| {
        ui.add_space(space::SNUG);
        egui::menu::bar(ui, |ui| {
            ui.menu_button(s.menu_file, |ui| {
                if item(ui, state, s.action_new, Action::NewDocument).clicked() {
                    queue.push(Command::NewDocument);
                    ui.close_menu();
                }
                if item(ui, state, s.action_open, Action::OpenDocument).clicked() {
                    queue.push(Command::OpenDocument);
                    ui.close_menu();
                }
                ui.menu_button(s.action_open_recent, |ui| {
                    if state.recent.is_empty() {
                        // Disabled rather than absent: an empty submenu that
                        // vanishes reads as a broken menu, and this says why.
                        ui.add_enabled(false, egui::Button::new(s.state_no_recent));
                        return;
                    }
                    for path in state.recent {
                        let label = RecentDocuments::label(path);
                        if ui
                            .button(label)
                            .on_hover_text(path.to_string_lossy())
                            .clicked()
                        {
                            queue.push(Command::OpenRecent(path.clone()));
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                if item(ui, state, s.action_save, Action::Save).clicked() {
                    queue.push(Command::Save);
                    ui.close_menu();
                }
                if item(ui, state, s.action_save_as, Action::SaveAs).clicked() {
                    queue.push(Command::SaveAs);
                    ui.close_menu();
                }
                ui.separator();
                // Convert beside import and export, because a crossing is the
                // same kind of act: it produces a new layer from something the
                // document already holds, and states what it costs first. And
                // the boolean beside the shapes, because it is the other half
                // of putting forms in a scene: one puts a second form in, this
                // one says what the two of them make.
                panel_items(
                    ui,
                    queue,
                    &[
                        (s.action_convert, Command::ToggleConvert),
                        (s.action_repair, Command::ToggleRepair),
                        (s.action_deform, Command::ToggleDeform),
                        (s.action_shapes, Command::ToggleShapes),
                        (s.action_boolean, Command::ToggleBoolean),
                        (s.action_import, Command::ToggleImport),
                        (s.action_export, Command::ToggleExport),
                    ],
                );
                ui.separator();
                if item(ui, state, s.action_quit, Action::Quit).clicked() {
                    queue.push(Command::Quit);
                    ui.close_menu();
                }
            });
            ui.menu_button(s.menu_edit, |ui| {
                if item_enabled(ui, state, state.can_undo, s.action_undo, Action::Undo).clicked() {
                    queue.push(Command::Undo);
                    ui.close_menu();
                }
                if item_enabled(ui, state, state.can_redo, s.action_redo, Action::Redo).clicked() {
                    queue.push(Command::Redo);
                    ui.close_menu();
                }
            });
            ui.menu_button(s.menu_view, |ui| {
                for preset in ViewPresetKind::ALL {
                    let action = match preset {
                        ViewPresetKind::Perspective => Action::ViewPerspective,
                        ViewPresetKind::Front => Action::ViewFront,
                        ViewPresetKind::Side => Action::ViewSide,
                        ViewPresetKind::Top => Action::ViewTop,
                    };
                    if item(ui, state, s.view_preset_name(preset), action).clicked() {
                        queue.push(Command::SetViewPreset(preset));
                        ui.close_menu();
                    }
                }
                if item(ui, state, s.action_frame_all, Action::FrameAll).clicked() {
                    queue.push(Command::FrameAll);
                    ui.close_menu();
                }
                // Checked, because it is a state and not an action: a menu
                // entry that toggles something has to say which way it is.
                if ui
                    .selectable_label(state.polyframe, s.action_polyframe)
                    .clicked()
                {
                    queue.push(Command::TogglePolyframe);
                    ui.close_menu();
                }
                // The three display terms, together and after the polyframe:
                // each is a way of shading what is already drawn rather than a
                // change to what is drawn, and grouping them says so.
                if ui
                    .selectable_label(state.studio_shading, s.action_shading)
                    .clicked()
                {
                    queue.push(Command::ToggleShading);
                    ui.close_menu();
                }
                if ui.selectable_label(state.cavity, s.action_cavity).clicked() {
                    queue.push(Command::ToggleCavity);
                    ui.close_menu();
                }
                // Only the studio rig casts, so the entry is only offered with
                // it on: a shadow toggle in MatCap mode is a control that does
                // nothing, which is worse than one that is not there.
                if state.studio_shading
                    && ui
                        .selectable_label(state.shadows, s.action_shadows)
                        .clicked()
                {
                    queue.push(Command::ToggleShadows);
                    ui.close_menu();
                }
                // How much an idle frame is worth spending on. Beside the
                // three shading terms because it is the same kind of setting —
                // it changes what the frame is drawn *with* and never what is
                // drawn — and a sculptor who finds one here should find the
                // rest.
                //
                // Written straight into the interface's own memory rather than
                // pushed as a command: nothing about it reaches the document,
                // and a `Command` carrying it could not be built anyway, since
                // the profile is a view type and commands live under the view.
                ui.separator();
                ui.label(
                    egui::RichText::new(s.label_viewport_profile)
                        .size(type_scale::HEADING)
                        .color(Tokens::text_faint()),
                );
                for profile in crate::quality::ViewportProfile::ALL {
                    let chosen = state.viewport_profile == profile;
                    if ui
                        .selectable_label(chosen, s.viewport_profile_name(profile))
                        .clicked()
                    {
                        ui.ctx()
                            .data_mut(|data| data.insert_temp(viewport_profile_id(), profile));
                        ui.close_menu();
                    }
                }
                ui.separator();

                // Beside the three view presets, because a reference is
                // placed on the plane one of them looks down.
                if ui
                    .selectable_label(state.show_references, s.action_references)
                    .clicked()
                {
                    queue.push(Command::ToggleReferences);
                    ui.close_menu();
                }
                ui.separator();
                // Three complete translations shipped from the beginning and
                // there was no way to choose between them: the locale was
                // taken from `Locale::default()` at startup and never asked
                // about again, so `Locale::from_tag` — written for exactly
                // this — was called by nothing.
                //
                // Each language is named in *itself*, which is the one rule a
                // language menu has: a reader who cannot read the current
                // interface can still find their own.
                ui.menu_button(s.menu_language, |ui| {
                    for locale in Locale::ALL {
                        if ui
                            .selectable_label(state.strings.locale == locale, locale.label())
                            .clicked()
                        {
                            queue.push(Command::SetLocale(locale));
                            ui.close_menu();
                        }
                    }
                });
            });
            ui.menu_button(s.menu_sculpt, |ui| {
                if ui.button(s.action_armature_new).clicked() {
                    queue.push(Command::NewArmature);
                    ui.close_menu();
                }
                // A checkbox rather than a button: this is the one mode in the
                // application, and a mode you cannot see the state of is the
                // kind that gets left on.
                let mut editing = state.armature.editing;
                if ui
                    .add_enabled(
                        state.armature.exists,
                        egui::Checkbox::new(&mut editing, s.action_armature_edit),
                    )
                    .clicked()
                {
                    queue.push(Command::ToggleArmatureEditing);
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        state.armature.editing && state.armature.selection,
                        egui::Button::new(s.action_armature_remove),
                    )
                    .clicked()
                {
                    queue.push(Command::RemoveZsphere);
                    ui.close_menu();
                }
                let mut negative = state.armature.selection_is_negative;
                if ui
                    .add_enabled(
                        state.armature.editing && state.armature.selection,
                        egui::Checkbox::new(&mut negative, s.action_zsphere_negative),
                    )
                    .clicked()
                {
                    queue.push(Command::ToggleZsphereNegative);
                    ui.close_menu();
                }
                ui.separator();
                let mut preview = state.armature.skin_preview;
                if ui
                    .add_enabled(
                        state.armature.exists,
                        egui::Checkbox::new(&mut preview, s.action_skin_preview),
                    )
                    .clicked()
                {
                    queue.push(Command::ToggleSkinPreview);
                    ui.close_menu();
                }
            });
            ui.menu_button(s.menu_brushes, |ui| {
                for tool in ToolKind::ALL {
                    if ui.button(s.tool(tool)).clicked() {
                        queue.push(Command::SelectTool(tool));
                        ui.close_menu();
                    }
                }
            });
            ui.menu_button(s.menu_dynamics, |ui| {
                // The menu the deformers belong in, and it was empty. A cage
                // is raised from here rather than from the inspector because
                // the inspector is where a cage is *worked*, and a section
                // that was there whether or not one was up pushed the rest of
                // the panel past the fold.
                let cageable = clayspace_model::can_be_caged(state.representation);
                let up = state.lattice.active;
                if ui
                    .add_enabled(cageable, egui::Button::new(s.action_cage))
                    .on_disabled_hover_text(s.status_cage_needs_a_field)
                    .clicked()
                {
                    queue.push(Command::ToggleLattice);
                    ui.close_menu();
                }
                if ui
                    .add_enabled(up, egui::Button::new(s.action_bend))
                    .clicked()
                {
                    queue.push(Command::ApplyLattice);
                    ui.close_menu();
                }
                ui.separator();
                // A curve is the other thing in this menu that is placed
                // rather than brushed: a set of points that stay where they
                // were put, which is what makes it something to go back to.
                if ui
                    .selectable_label(state.curve.active, s.action_curve)
                    .clicked()
                {
                    queue.push(Command::ToggleCurve);
                    ui.close_menu();
                }
            });
            ui.menu_button(s.menu_masks, |ui| {
                // First, because it is what the rest of the menu operates on:
                // there is nothing to invert or extrude until a region has
                // been frozen, and the key that does it is spelled out here so
                // it can be learned from the menu rather than from the manual.
                if item(ui, state, s.action_paint_mask, Action::ToggleMaskPainting).clicked() {
                    queue.push(Command::ToggleMaskPainting);
                    ui.close_menu();
                }
                // And which gesture it makes. Here as well as on the options
                // bar because the bar only carries it with the mask brush
                // already in hand, and this is where a sculptor comes looking
                // for what masking can do.
                for gesture in MaskGesture::ALL {
                    let chosen = state.mask_gesture == gesture;
                    if ui
                        .selectable_label(chosen, s.mask_gesture_name(gesture))
                        .on_hover_text(s.hint_mask_outline)
                        .clicked()
                    {
                        queue.push(Command::SetMaskGesture(gesture));
                        ui.close_menu();
                    }
                }
                ui.separator();
                // Disabled rather than hidden: a menu whose entries come and
                // go is harder to learn than one whose entries are sometimes
                // grey, and the grey says *why* the tool is unavailable.
                let steps = state.mask_steps;
                for op in [
                    MaskOp::Invert,
                    MaskOp::Expand(steps),
                    MaskOp::Contract(steps),
                    MaskOp::Smooth(steps),
                    MaskOp::InvertWithinBounds,
                    MaskOp::Clear,
                ] {
                    let enabled = !op.needs_a_mask() || state.mask.is_active();
                    // The amount beside the name, because the same menu entry
                    // now does a different amount of work depending on the
                    // panel — and the two units it stands for, cells and
                    // passes, are not the same quantity.
                    let label = match op.amount() {
                        Some(amount) => format!("{} · {amount}", s.mask_op_name(op)),
                        None => s.mask_op_name(op).to_string(),
                    };
                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                        queue.push(Command::ApplyMaskOp(op));
                        ui.close_menu();
                    }
                }
                ui.separator();
                // A mesh has no field to extrude from and the engine refuses
                // it. Grey with the reason on it rather than a click that does
                // nothing, which is what it was — and the reason names the way
                // round, which is a crossing this application already offers.
                let extrudable = clayspace_model::can_extrude(state.representation);
                for side in ExtrudeSide::ALL {
                    let label = format!("{} — {}", s.action_extrude, s.extrude_side_name(side));
                    if ui
                        .add_enabled(
                            state.mask.is_active() && extrudable,
                            egui::Button::new(label),
                        )
                        .on_disabled_hover_text(if extrudable {
                            ""
                        } else {
                            s.status_extrude_needs_a_field
                        })
                        .clicked()
                    {
                        queue.push(Command::ExtrudeMask(ExtrudeSettings {
                            side,
                            ..state.extrude
                        }));
                        ui.close_menu();
                    }
                }
            });
            // The regions: which are shown, and how to have them all back.
            // The menu was declared and left empty, beside a `layout` module
            // that carried the sizes and the collapse state and was called by
            // nothing — so a sculptor could neither put a panel away nor drag
            // one wider, and the design's own arrangement was the only one.
            ui.menu_button(s.menu_window, |ui| {
                for (index, panel) in crate::layout::Panel::ALL.into_iter().enumerate() {
                    // Shown rather than collapsed, so a tick means "this is on
                    // screen" — which is what a person reads a tick as.
                    let shown = !state.collapsed[index];
                    if ui.selectable_label(shown, s.panel_name(panel)).clicked() {
                        ui.ctx()
                            .data_mut(|data| data.insert_temp(panel_toggle_id(), panel));
                        ui.close_menu();
                    }
                }
                ui.separator();
                // The whole chrome at once, with the key beside it — a mode a
                // sculptor cannot find their way out of is worse than no mode,
                // and an empty window says nothing about how it happened.
                if with_chord(
                    ui.selectable_label(state.focus, s.action_focus),
                    state,
                    Action::ToggleFocus,
                )
                .clicked()
                {
                    ui.ctx()
                        .data_mut(|data| data.insert_temp(focus_toggle_id(), true));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(s.action_reset_layout).clicked() {
                    ui.ctx()
                        .data_mut(|data| data.insert_temp(layout_reset_id(), true));
                    ui.close_menu();
                }
            });
            ui.menu_button(s.menu_help, |ui| {
                if ui.button(s.action_diagnostics).clicked() {
                    queue.push(Command::ToggleDiagnostics);
                    ui.close_menu();
                }
                if ui.button(s.action_attribution).clicked() {
                    queue.push(Command::ToggleAttribution);
                    ui.close_menu();
                }
            });
        });

        // The document, on the trailing edge as the design places it.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(space::SNUG);
            let name = document_display_name(state.strings, state.document_name);
            let title = if state.modified {
                format!("{} • {}", name, state.strings.state_unsaved)
            } else {
                name.to_string()
            };
            ui.label(
                egui::RichText::new(title)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
        });
    });
}
