//! The left region: the scene, the layer stack, and what a stroke is made
//! under.
//!
//! The stack is the part a sculptor looks at most, so the rows carry more than
//! their names: what each layer holds, how strongly it acts, whether it is
//! visible, whether it is protected, and — on the active one — an accent rail,
//! because the tone step that used to carry that alone is three and a half per
//! cent of relative luminance.

use super::*;

/// Where the offer to collapse a costly layer was drawn, for a test that has
/// to establish it was drawn at all. Absent when the engine is not advising,
/// which is most of the time.
pub fn optimize_button_id() -> egui::Id {
    egui::Id::new("subtool-optimize")
}

/// Where the offer to rebuild a mesh layer's topology was drawn. Absent for
/// every representation but a mesh, and for a mesh row whose triangles have
/// not arrived.
pub fn remesh_button_id() -> egui::Id {
    egui::Id::new("subtool-remesh")
}

/// The scene tree and the layer stack.
pub fn left_panel(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    scene_section(ui, state);
    layers_section(ui, state, queue);
}

/// The scene tree: every node, indented by its depth, with whether it shows.
pub(super) fn scene_section(ui: &mut egui::Ui, state: &ShellState<'_>) {
    if !heading(ui, state.strings.section_scene) {
        return;
    }
    for node in &state.scene.nodes {
        ui.horizontal(|ui| {
            ui.add_space(space::SNUG + node.depth as f32 * space::ROOMY);
            // The active layer, because there is only one: the tree and the
            // stack read the same fact, so a click in the viewport lights the
            // same row a click in the stack does.
            let selected = state.scene.active == Some(node.key);
            let text = egui::RichText::new(&node.name)
                .size(type_scale::BODY)
                .color(if selected {
                    Tokens::text()
                } else {
                    Tokens::text_dim()
                });
            ui.label(text);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                icons::button(
                    ui,
                    if node.visible {
                        Icon::Visible
                    } else {
                        Icon::Hidden
                    },
                    node.visible,
                );
            });
        });
    }
}

/// The layer stack, and what stands under it: the placed objects on a field,
/// the recording control on a grid, and the control that adds a layer.
pub(super) fn layers_section(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !heading(ui, state.strings.section_layers) {
        return;
    }
    for layer in state.scene.layers.iter().rev() {
        layer_row(ui, state, layer, queue);
    }

    // The shapes standing in the active layer, under the layer they stand in.
    // Only on a field, because that is the only place an object can live, and
    // a heading that could only ever say "none" is worse than no heading.
    if state.representation == Representation::Sdf {
        ui.add_space(space::SNUG);
        object_rows(ui, state, queue);
    }
    // Recording a pass, and what the stack costs. Under the layer list because
    // that is where the passes themselves are, and only for a grid — a field
    // and a mesh have no stack to record into, and a button that can only
    // refuse is worse than one that is not there.
    if state.representation == Representation::Voxel {
        ui.add_space(space::SNUG);
        sculpt_recording_control(ui, state, queue);
    }
    // And what the field costs, where there is a field. Only when the engine
    // advises collapsing it: a row that is always there is a row nobody reads,
    // and the number it would carry means nothing to a sculptor until it means
    // something.
    if state.representation == Representation::Sdf {
        field_health_control(ui, state, queue);
    }
    // And the mesh counterpart, which is a rebuild rather than a collapse.
    // Always offered rather than only when something is wrong: a field
    // steepens measurably and the engine can say when, and there is no
    // equivalent number for "this topology has stopped taking detail" — the
    // sculptor is the one who can see that, so the control waits for them
    // instead of waiting for advice that does not exist.
    if state.representation == Representation::Mesh {
        remesh_control(ui, state, queue);
    }

    ui.add_space(space::SNUG);
    add_layer_control(ui, state, queue);
}

/// Adding a layer, and saying what it should hold.
///
/// The button alone makes the field layer it always made — "the default stays
/// what it was" — and the list beside it is how a sculptor asks for a grid
/// without crossing one afterwards, which is the cost the choice exists to
/// avoid.
///
/// `Representation::CREATABLE` and not `ALL`: a mesh layer comes from carrying
/// a mesh, and the entry that offered one here made a row labelled "Malha"
/// that could never hold a triangle. See the constant for the specification's
/// own qualification.
pub(super) fn add_layer_control(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    ui.horizontal(|ui| {
        let button = ui.button(format!("+  {}", s.label_new_layer));
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(new_layer_button_id(), button.rect));
        if button.clicked() {
            // Layer creation is a document change like any other.
            queue.push(Command::AddLayer(Representation::Sdf));
        }
        let list = egui::ComboBox::from_id_salt("new-layer-kind")
            .selected_text(s.label_new_layer_kind)
            .width(96.0)
            .show_ui(ui, |ui| {
                for representation in Representation::CREATABLE {
                    let response =
                        ui.selectable_label(false, s.representation_name(representation));
                    // Recorded where a test can find it, for the reason
                    // `slider_id` states: this whole control was unreachable
                    // from a test, which is how the entry that made a dead mesh
                    // row went unnoticed.
                    ui.ctx().memory_mut(|memory| {
                        memory
                            .data
                            .insert_temp(new_layer_kind_id(representation), response.rect)
                    });
                    if response.clicked() {
                        queue.push(Command::AddLayer(representation));
                    }
                }
            });
        ui.ctx().memory_mut(|memory| {
            memory
                .data
                .insert_temp(new_layer_kind_menu_id(), list.response.rect)
        });
    });
}

/// The id the "new layer" button carries, so a test can press it.
///
/// The same arrangement [`insert_as_chip_id`] has and for the same reason:
/// reaching a control by coordinate reaches whatever landed above it.
pub fn new_layer_button_id() -> egui::Id {
    egui::Id::new("new-layer-button")
}

/// The id the list beside it carries, which is what has to be pressed before
/// the entries inside it exist at all.
pub fn new_layer_kind_menu_id() -> egui::Id {
    egui::Id::new("new-layer-kind-menu")
}

/// The id one entry of that list carries.
pub fn new_layer_kind_id(representation: Representation) -> egui::Id {
    // By position rather than by name: `representation_name` is interface text
    // and an id built from one moves when a translation does.
    egui::Id::new(("new-layer-kind", representation as u8))
}

pub(super) fn layer_row(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    layer: &LayerSummary,
    queue: &mut CommandQueue,
) {
    let active = state.scene.active == Some(layer.key);
    let fill = if active {
        Tokens::raised()
    } else {
        Tokens::panel()
    };

    let row = egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::symmetric(
            space::SNUG as i8,
            space::TIGHT as i8,
        ))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let eye = if layer.visible {
                    Icon::Visible
                } else {
                    Icon::Hidden
                };
                if icons::button(ui, eye, layer.visible).clicked() {
                    queue.push(Command::SetLayerVisible(layer.key, !layer.visible));
                }

                // The name gets what is left after the row's right-hand side
                // is reserved, and truncates into it. A label sized to its own
                // text takes the whole row and the tag lands on top of it:
                // "Detalhes_secundario$DF" is what that looks like.
                //
                // Reserved rather than laid out by egui because a
                // right-to-left group inside a horizontal claims the remaining
                // width, which is the whole of it when the name has not been
                // bounded first.
                // The height of one widget, not of the rest of the panel:
                // this strip is *allocated* rather than bounded, so a height
                // of `available_height` would make the row as tall as the
                // layer stack has space for.
                let strip = egui::vec2(
                    (ui.available_width() - size::LAYER_ROW_TAIL).max(size::LAYER_NAME_MIN),
                    ui.spacing().interact_size.y,
                );
                match state.renaming {
                    Some((key, draft)) if key == layer.key => {
                        ui.allocate_ui_with_layout(
                            strip,
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| rename_field(ui, draft, queue),
                        );
                    }
                    _ => layer_name(ui, strip, state, layer, active, queue),
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    numeric(ui, format!("{:>3}", layer.intensity));
                    // What the layer holds. Told apart by the word rather than
                    // by a colour, so it survives the contrast theme and a
                    // reader who cannot separate the hues.
                    ui.label(
                        egui::RichText::new(representation_tag(layer.representation))
                            .size(type_scale::LABEL)
                            .color(Tokens::text_dim()),
                    )
                    .on_hover_text(layer.representation.label());
                    // The name grows into whatever is left, so without this
                    // the tag sits flush against it and the two read as one
                    // word: "Detalhes_secundariosSDF".
                    ui.add_space(space::SNUG);
                    if layer.protection.locked || layer.protection.ghost {
                        let icon = if layer.protection.ghost {
                            Icon::Ghost
                        } else {
                            Icon::Locked
                        };
                        let response = icons::button(ui, icon, false);
                        if let Some(refusal) = layer.protection.refusal() {
                            response.on_hover_text(refusal);
                        }
                    }
                });
            });
        });

    // The tone step from `panel` to `raised` is 3.5% of relative luminance,
    // and it was the only thing saying which of four layers a dab would land
    // on. The rail is what makes that answerable at a glance.
    if active {
        selection_rail(ui, row.response.rect);
    }

    // The passes recorded on this layer, nested under it.
    //
    // Under the layer rather than in a panel of its own: a pass has no meaning
    // apart from the grid it was recorded on, and a second stack elsewhere
    // would have to repeat which layer each entry belongs to. Shown only for
    // the active layer — a document with several grids would otherwise unroll
    // every stack at once, and only one of them can be recorded into.
    if active {
        let count = layer.sculpt_layers.len();
        // Top of the stack first, as the layer list itself is ordered: the
        // last pass recorded wins where two overlap, and the thing that wins
        // belongs at the top.
        for pass in layer.sculpt_layers.iter().rev() {
            sculpt_layer_row(ui, state, pass, count, queue);
        }
    }
    ui.add_space(space::HAIR);
}

/// One recorded pass, indented under the layer it belongs to.
pub(super) fn sculpt_layer_row(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    pass: &SculptLayer,
    // How many passes the stack holds, so the top one offers no move up.
    count: usize,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    ui.horizontal(|ui| {
        ui.add_space(space::ROOMY);

        let eye = if pass.visible {
            Icon::Visible
        } else {
            Icon::Hidden
        };
        if icons::button(ui, eye, pass.visible).clicked() {
            queue.push(Command::SculptLayer(SculptLayerOp::SetVisible {
                index: pass.index,
                visible: !pass.visible,
            }));
        }

        ui.label(
            egui::RichText::new(pass.display_name())
                .size(type_scale::LABEL)
                // A pass that recorded nothing is shown rather than hidden — a
                // sculptor may have started recording and thought better of it,
                // and a row that vanishes is harder to explain than a dim one —
                // but dialling it does nothing, and it reads that way.
                .color(if pass.is_empty() {
                    Tokens::text_dim()
                } else {
                    Tokens::text()
                }),
        )
        .on_hover_text(format!(
            "{} · {}",
            format_args!("{} {}", thousands(pass.cells), s.sculpt_cells),
            bytes_label(pass.bytes)
        ));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("×")
                .on_hover_text(s.sculpt_remove)
                .clicked()
            {
                queue.push(Command::SculptLayer(SculptLayerOp::Remove {
                    index: pass.index,
                }));
            }
            // Merging needs something below to merge into, so the button is
            // absent on the bottom pass rather than present and refusing.
            if pass.index > 0
                && ui
                    .small_button("⤓")
                    .on_hover_text(s.sculpt_merge_down)
                    .clicked()
            {
                queue.push(Command::SculptLayer(SculptLayerOp::MergeDown {
                    index: pass.index,
                }));
            }
            // Order decides which pass wins where two touched the same cell,
            // so it is worth moving. Each arrow is absent at the end it cannot
            // move toward.
            if pass.index + 1 < count
                && ui
                    .small_button("▲")
                    .on_hover_text(s.sculpt_move_up)
                    .clicked()
            {
                queue.push(Command::SculptLayer(SculptLayerOp::Move {
                    from: pass.index,
                    to: pass.index + 1,
                }));
            }
            if pass.index > 0
                && ui
                    .small_button("▼")
                    .on_hover_text(s.sculpt_move_down)
                    .clicked()
            {
                queue.push(Command::SculptLayer(SculptLayerOp::Move {
                    from: pass.index,
                    to: pass.index - 1,
                }));
            }
            let mut strength = pass.strength;
            if ui
                .add(
                    egui::DragValue::new(&mut strength)
                        .speed(0.01)
                        .range(0.0..=1.0)
                        .fixed_decimals(2),
                )
                .changed()
            {
                queue.push(Command::SculptLayer(SculptLayerOp::SetStrength {
                    index: pass.index,
                    strength,
                }));
            }
        });
    });
}

/// The id the clickable part of a layer's row is registered under.
///
/// Public so a test can ask the interface where a row is rather than measuring
/// it off a capture — a coordinate read from a screenshot is a coordinate that
/// goes stale the next time anything above it changes height.
pub fn layer_row_id(key: LayerKey) -> egui::Id {
    egui::Id::new(("layer-row", key.0))
}

/// A layer's name, and the two gestures that act on it.
///
/// A single click makes the layer active, which is what the panel is mostly
/// for. A double click renames it in place, which is where every layer stack
/// puts that gesture — and the row's own menu says so as well, because a
/// gesture nothing announces is one only its authors know about.
pub(super) fn layer_name(
    ui: &mut egui::Ui,
    strip: egui::Vec2,
    state: &ShellState<'_>,
    layer: &LayerSummary,
    active: bool,
    queue: &mut CommandQueue,
) {
    let name = egui::RichText::new(&layer.name)
        .size(type_scale::BODY)
        // Primary text on the active row, secondary on the rest. One of the
        // three marks the active layer carries — the others are the raised
        // surface under it and the rail at its leading edge — so that none of
        // them is load-bearing on its own.
        .color(if active {
            Tokens::text()
        } else {
            Tokens::text_dim()
        });
    // The whole strip senses the click, not the glyphs. A label senses only
    // the width its own text takes, so a row named "Base" answered across
    // sixty pixels and was dead across the rest of itself: clicking the empty
    // part of the row selected nothing, and right-clicking it opened no menu.
    //
    // Interacted under the layer's own id rather than under the positional one
    // egui would generate. A context menu is keyed by the id of the widget it
    // hangs off, so a positional id moves the menu's identity whenever a row
    // moves in the stack — and it gives a test a name to ask the rectangle for
    // instead of a coordinate measured off a screenshot.
    let (rect, _) = ui.allocate_exact_size(strip, egui::Sense::hover());
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| ui.add(egui::Label::new(name).truncate()),
    );
    // After the label, not before. A truncated `Label` senses hover so it can
    // show the full text, and the widget registered last is the one on top —
    // so an interaction claimed first was shadowed for exactly the rows whose
    // names were long enough to truncate. Short names worked, long ones were
    // dead: the two halves of the stack behaved differently for a reason
    // nothing in the row said.
    let response = ui.interact(rect, layer_row_id(layer.key), egui::Sense::click());
    if response.double_clicked() {
        queue.push(Command::BeginRenameLayer(layer.key));
    } else if response.clicked() {
        queue.push(Command::SelectLayer(layer.key));
    }
    response.context_menu(|ui| layer_menu(ui, state, layer, queue));
}

/// What a layer row's own menu offers.
/// Where a layer row's crossing entry is recorded, so a test can find it by
/// the layer and the representation rather than by a pixel.
pub fn layer_convert_id(key: LayerKey, target: Representation) -> egui::Id {
    egui::Id::new(("layer-convert", key.0, target))
}

pub(super) fn layer_menu(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    layer: &LayerSummary,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    if ui.button(s.action_rename_layer).clicked() {
        queue.push(Command::BeginRenameLayer(layer.key));
        ui.close_menu();
    }
    // The state to push rather than a toggle read off the row: the scene says
    // which subtool is alone, so the entry cannot offer to solo one that
    // already is.
    let soloed = state.scene.is_soloed(layer.key);
    let solo = if soloed {
        s.action_release_solo
    } else {
        s.action_solo_layer
    };
    if ui.button(solo).clicked() {
        queue.push(Command::SoloLayer((!soloed).then_some(layer.key)));
        ui.close_menu();
    }
    // Crossing this layer into another representation, in place.
    //
    // `ConversionSettings::in_place` is what a sculptor means by converting
    // *this* layer — the source leaves as the result arrives and the result
    // stands where it stood, rather than a second layer appearing beside the
    // original. The setting has always been there; there was no way to ask for
    // it from the layer itself.
    //
    // Offered from the row rather than only from the representation bar because
    // the bar speaks for the *active* layer, and a sculptor looking at a stack
    // means the row they opened the menu on.
    let crossings = Direction::from_representation(layer.representation);
    if !crossings.is_empty() {
        ui.separator();
        for direction in crossings {
            let target = direction.to();
            // An ellipsis, because this opens the panel rather than converting:
            // a crossing costs something, a crossing into cells needs a size
            // chosen, and one that would not fit the budget is refused. The
            // panel is where all three are said.
            let label = format!("{} {}…", s.label_convert_to, s.representation_name(target));
            let entry = ui.button(label);
            ui.ctx().memory_mut(|memory| {
                memory
                    .data
                    .insert_temp(layer_convert_id(layer.key, target), entry.rect)
            });
            if entry.clicked() {
                // Made active first: the conversion acts on the active layer,
                // so a crossing asked of a row that is not the active one would
                // otherwise convert something else entirely.
                queue.push(Command::SelectLayer(layer.key));
                queue.push(Command::SetConversion(
                    clayspace_model::ConversionSettings {
                        direction,
                        in_place: true,
                        ..state.conversion
                    },
                ));
                if !state.show_convert {
                    queue.push(Command::ToggleConvert);
                }
                ui.close_menu();
            }
        }
        ui.separator();
    }

    // Disabled with the reason on it rather than offered and refused. The
    // document keeps one layer to sculpt on, and the model says so — but a
    // menu entry whose only outcome is an error message in the status area is
    // one the sculptor has to try before learning anything.
    let last = state.scene.layers.len() <= 1;
    let remove = ui.add_enabled(!last, egui::Button::new(s.action_remove_layer));
    if remove.clicked() {
        queue.push(Command::RemoveLayer(layer.key));
        ui.close_menu();
    }
    if last {
        remove.on_hover_text(s.layer_last_one);
    }
}

/// What the engine says about the active field layer, and the one thing to do
/// about it.
///
/// A chain of edits steepens the field it produces until a ray march takes
/// many small steps and every dab pays for it; the engine measures that and
/// says when collapsing the layer is worth it. Measured here, a layer the
/// engine advised on took a dab from 56 ms to 13 ms once collapsed — and
/// collapsing it took about six seconds.
///
/// Which is why this offers and never acts. Consolidation costs seconds and
/// changes what the layer holds, so it is the sculptor's decision; the
/// interface's job is to say that the moment has arrived, which until now it
/// never did — the engine's advice was computed and read by nothing.
pub(super) fn field_health_control(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    let Some(layer) = state.scene.active_layer() else {
        return;
    };
    let Some(health) = layer.health else {
        return;
    };
    if !health.advises_consolidation || health.consolidated {
        return;
    }

    ui.add_space(space::SNUG);
    ui.label(
        egui::RichText::new(s.optimize_advice)
            .size(type_scale::LABEL)
            .color(Tokens::accent()),
    );
    ui.horizontal(|ui| {
        let button = ui.button(s.optimize_action);
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(optimize_button_id(), button.rect));
        if button.clicked() {
            queue.push(Command::OptimizeLayer(layer.key));
        }
        // The count rather than the step scale: a sculptor can see how many
        // strokes they have made and cannot see a Lipschitz bound.
        ui.label(
            egui::RichText::new(health.items.to_string())
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
    });
}

/// Rebuilding a mesh layer's topology through a voxel field — DynaMesh.
///
/// The mesh counterpart to [`field_health_control`], and offered on the same
/// terms: it costs seconds, it destroys what it replaces, and it is never
/// taken quietly. What differs is when it is shown. A field's steepening is
/// something the engine measures and advises on, so that row appears only when
/// the advice arrives; a mesh's topology going wrong under a pull is something
/// only the sculptor can see, so this one is always there for them to reach.
///
/// The price is on the heading's hover rather than in a paragraph under the
/// button. It is the same sentence every time and a sculptor reaching for this
/// the tenth time is not reading it — but the first time, and the time they
/// wonder where their UVs went, it has to be somewhere.
pub(super) fn remesh_control(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    let Some(layer) = state.scene.active_layer() else {
        return;
    };

    ui.add_space(space::SNUG);
    ui.label(
        egui::RichText::new(s.remesh_heading)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    )
    .on_hover_text(s.remesh_hint);

    let mut settings = state.remesh;
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(s.remesh_resolution)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        // Logarithmic, because the numbers a sculptor moves between are 64,
        // 128, 256 rather than 128, 129, 130: on a linear track the whole
        // useful lower half of the range is the first centimetre.
        let slider = ui.add(
            egui::Slider::new(
                &mut settings.resolution,
                clayspace_model::RemeshSettings::RESOLUTION,
            )
            .logarithmic(true)
            .show_value(true),
        );
        if slider.changed() {
            changed = true;
        }
        slider.on_hover_text(s.remesh_resolution_hint);
    });

    for (label, hint, flag) in [
        (
            s.remesh_remove_loose,
            s.remesh_remove_loose_hint,
            &mut settings.remove_loose_pieces,
        ),
        (
            s.remesh_follow,
            s.remesh_follow_hint,
            &mut settings.follow_the_source,
        ),
        (s.remesh_sharp, s.remesh_sharp_hint, &mut settings.sharp),
    ] {
        let toggle = ui.checkbox(
            flag,
            egui::RichText::new(label)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        if toggle.changed() {
            changed = true;
        }
        toggle.on_hover_text(hint);
    }

    if changed {
        queue.push(Command::SetRemeshSettings(settings));
    }

    ui.horizontal(|ui| {
        // Enabled, and refused by the model where it has to be. The usual
        // arrangement in this file is to disable a control with its reason on
        // it, and the one case that would need it here — a mesh row whose
        // triangles have not arrived, since an import fills one later — is not
        // a fact the scene carries. Guessing at it from the layer's
        // representation would disable nothing and only look as though it
        // did, so the refusal is left where it is measurable: the model
        // states it and the shell shows it beside the layer stack.
        let button = ui.button(s.remesh_action);
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(remesh_button_id(), button.rect));
        if button.clicked() {
            // Made active first, as a conversion is: the rebuild acts on the
            // active layer, so asking it of a row that is not the active one
            // would rebuild something else entirely.
            queue.push(Command::SelectLayer(layer.key));
            queue.push(Command::RemeshLayer(layer.key));
        }

        // What the last one came to, beside the button that made it. The
        // triangle counts are the answer to "was that the resolution I meant";
        // the piece count is the answer to "why did those two not join".
        if let Some(outcome) = state.remesh_outcome {
            ui.label(
                egui::RichText::new(format!(
                    "{} → {} {}",
                    outcome.triangles_before, outcome.triangles_after, s.remesh_result
                ))
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
            );
        }
    });

    let Some(outcome) = state.remesh_outcome else {
        return;
    };
    if outcome.pieces > 1 {
        ui.label(
            egui::RichText::new(format!("{} {}", outcome.pieces, s.remesh_pieces))
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }
    // Both of these are things the sculptor cannot see by looking at the
    // result, which is the whole reason they are said. Dropped UVs are not a
    // failure — the engine will not pretend to reproject a layout across a
    // seam — and a rebuild that did not come out closed is the sharp mode
    // being what it is documented to be.
    for notice in [
        outcome.uvs_dropped.then_some(s.remesh_uvs_dropped),
        (!outcome.watertight).then_some(s.remesh_not_watertight),
    ]
    .into_iter()
    .flatten()
    {
        ui.label(
            egui::RichText::new(notice)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
    }
}

/// Starting and stopping a recording, and what the stack occupies.
pub(super) fn sculpt_recording_control(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    let cost = state.sculpt_cost;

    ui.horizontal(|ui| {
        if cost.recording {
            if ui.button(s.sculpt_end).clicked() {
                queue.push(Command::SculptLayer(SculptLayerOp::EndRecording));
            }
            ui.label(
                egui::RichText::new(s.sculpt_recording)
                    .size(type_scale::LABEL)
                    .color(Tokens::accent()),
            );
        } else if ui.button(s.sculpt_begin).clicked() {
            // Unnamed: the engine takes a name and the panel numbers what has
            // none, so a sculptor is not stopped by a dialog for something
            // they will rename or never look at.
            queue.push(Command::SculptLayer(SculptLayerOp::BeginRecording {
                name: String::new(),
            }));
        }
    });

    if cost.layers > 0 {
        ui.label(
            egui::RichText::new(bytes_label(cost.bytes))
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        // Said rather than enforced. A cap that silently stopped recording
        // would leave a pass on the grid and un-dialable, which is a
        // correctness bug wearing a memory limit's clothes — so the number is
        // shown and the sculptor decides.
        if cost.worth_merging() {
            ui.label(
                egui::RichText::new(s.sculpt_worth_merging)
                    .size(type_scale::LABEL)
                    .color(Tokens::accent()),
            );
        }
    }
}
