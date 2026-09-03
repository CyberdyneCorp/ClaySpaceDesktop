//! Placing a form in the scene, and the objects already placed.
//!
//! One workflow rather than two panels: pick a shape, place it, aim it, and
//! change how it meets what is under it. Splitting the picker from the
//! properties would mean two places open at once for a single operation.

use super::*;

/// The shapes a sculptor can put in the scene, and what the selected one is.
///
/// One section for both because they are one workflow: pick a shape, place
/// it, aim it, and change how it meets what is under it. Splitting the picker
/// from the properties would mean two places open at once for a single
/// operation.
///
/// A section of the right panel rather than a window, because a window floats
/// over the viewport, and the viewport is where the form a shape is being
/// placed into stands: it hid the very thing the shape was being aimed at.
/// Docked, the picker and the sculpt are side by side while a shape is placed
/// and turned. Closed from its own heading, as the window was from its title
/// bar, or from the rail.
pub(super) fn shapes_section(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    if closable_heading(ui, s.section_shapes) {
        queue.push(Command::ToggleShapes);
    }
    insert_destination(ui, state, queue);

    ui.add_space(space::SNUG);
    shape_picker(ui, state, queue);
    // A shape's measurements only where a shape is what would be placed: a
    // model is measured by itself, and offering a radius for one would be
    // offering a control that does nothing.
    if state.mesh_operand.is_none() {
        ui.add_space(space::SNUG);
        shape_measurements(ui, state, queue);
    } else {
        mesh_operand_cost(ui, state);
    }

    ui.add_space(space::SNUG);
    if ui.button(s.action_insert).clicked() {
        queue.push(Command::InsertShape);
    }

    ui.separator();
    other_insert_sources(ui, state, queue);

    if let Some(object) = state
        .selected_object
        .and_then(|id| state.objects.iter().find(|object| object.id == id))
    {
        ui.separator();
        selected_object_controls(ui, state, object, queue);
    }

    ui.add_space(space::SNUG);
    ui.label(
        egui::RichText::new(s.hint_shapes)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
}

/// The id one destination chip carries, so a test can press it by name.
///
/// The same arrangement [`layer_transform_chip_id`] has and for the same
/// reason: reaching a control by coordinate reaches whatever landed above it.
pub fn insert_as_chip_id(destination: clayspace_model::InsertAs) -> egui::Id {
    // By position rather than by name, because a name is interface text and an
    // id built from one moves when a translation does.
    egui::Id::new(("insert-as", destination as u8))
}

/// Where the next inserted form lands.
///
/// Offered rather than inferred: the specification says a form worked on its
/// own is a subtool and a form put into the layer being worked is a part of
/// that form, that both are wanted, and that guessing between them from context
/// would be wrong half the time.
pub(super) fn insert_destination(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    ui.label(
        egui::RichText::new(s.label_insert_as)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    ui.horizontal(|ui| {
        for destination in clayspace_model::InsertAs::ALL {
            let on = state.insert_as == destination;
            let response = ui.add(chip(s.insert_as_name(destination), on, Tokens::panel()));
            // Recorded where a test can find it, for the reason `slider_id`
            // states: a control reached by pixel coordinate is a different
            // control the next time a section lands above it.
            ui.ctx().memory_mut(|memory| {
                memory
                    .data
                    .insert_temp(insert_as_chip_id(destination), response.rect)
            });
            if response.clicked() {
                queue.push(Command::SetInsertAs(destination));
            }
        }
    });
    // An object is an item in an SDF layer's ordered list, and a grid and a
    // mesh have no such list. Said while the choice is being made rather than
    // left to a refusal after the click — and the subtool destination stays
    // available, which is the whole point of stating it here.
    if state.representation != Representation::Sdf {
        ui.label(
            egui::RichText::new(s.label_shapes_sdf_only)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
    }
}

/// The two insertion sources that are not one of the offered shapes.
///
/// Under the shapes and separated from them, because neither is a thing the
/// picker above is set to: one reads a file and the other resamples a subtool
/// already in the scene, and both always arrive as a subtool of their own.
pub(super) fn other_insert_sources(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    if ui.button(s.action_insert_mesh).clicked() {
        queue.push(Command::InsertMesh);
    }

    ui.add_space(space::TIGHT);
    egui::ComboBox::from_id_salt("copy-subtool")
        .selected_text(s.action_copy_subtool)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (key, name) in state.copyable_subtools {
                if ui.selectable_label(false, name).clicked() {
                    queue.push(Command::CopySubtool(*key));
                }
            }
        });
    ui.label(
        egui::RichText::new(s.hint_copy_subtool)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
}

/// Which shape a placement would use.
pub(super) fn shape_picker(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.label(
        egui::RichText::new(s.label_shape)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    // The chosen mesh's name where one is chosen, since that is what would be
    // placed.
    let chosen = match state.mesh_operand.and_then(|key| {
        state
            .mesh_operands
            .iter()
            .find(|(candidate, _)| *candidate == key)
    }) {
        Some((_, name)) => name.as_str(),
        None => s.shape(state.shape),
    };
    egui::ComboBox::from_id_salt("shape-picker")
        .selected_text(chosen)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for shape in clayspace_model::Shape::ALL {
                let picked = state.mesh_operand.is_none() && shape == state.shape;
                if ui.selectable_label(picked, s.shape(shape)).clicked() {
                    // Choosing a shape clears any mesh: one thing is placed,
                    // and a picker showing a cylinder that would place a model
                    // is a picker lying about what the button does.
                    queue.push(Command::SetMeshOperand(None));
                    queue.push(Command::SetShape(shape));
                }
            }
            // The imported models, under the shapes and separated from them,
            // because placing one is a *crossing* and costs something the
            // shapes above do not.
            if !state.mesh_operands.is_empty() {
                ui.separator();
                for (key, name) in state.mesh_operands {
                    let picked = state.mesh_operand == Some(*key);
                    if ui.selectable_label(picked, name).clicked() && !picked {
                        queue.push(Command::SetMeshOperand(Some(*key)));
                    }
                }
            }
        });
}

/// What crossing a mesh into an operand would cost, before it is run.
///
/// Stated rather than discovered: the crossing quantises the vertices and
/// drops the edge loops that made the model worth keeping as a mesh, and
/// asking for consent to something unstated is not asking. The figures are the
/// conversion panel's own, for the same crossing at the same resolution.
pub(super) fn mesh_operand_cost(ui: &mut egui::Ui, state: &ShellState<'_>) {
    let Some(cost) = state.mesh_operand_cost else {
        return;
    };
    ui.add_space(space::SNUG);
    for line in crossing_cost_lines(state, clayspace_model::Direction::MeshToSdf, cost) {
        ui.label(
            egui::RichText::new(line)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
    }
}

/// A slider per number the shape is measured by.
///
/// Built from the shape's own description rather than from a case per shape:
/// a panel that knew a torus takes a major radius and then a minor one would
/// be a panel with fourteen special cases in it, and a fifteenth shape would
/// need a fifteenth.
pub(super) fn shape_measurements(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    let parameters = state.shape.parameters();
    let mut values = state.shape.sanitised(state.shape_parameters);
    for (at, parameter) in parameters.iter().enumerate() {
        if let Some(value) = slider(
            ui,
            s.shape_parameter(parameter.key),
            values[at],
            parameter.min..=parameter.max,
            2,
        ) {
            values[at] = value;
            queue.push(Command::SetShapeParameters(values.clone()));
            return;
        }
    }
}

/// What the selected object is, and how it meets what is under it.
pub(super) fn selected_object_controls(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    object: &clayspace_model::SceneObject,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    ui.label(
        egui::RichText::new(object_name(state, object))
            .size(type_scale::LABEL)
            .color(Tokens::text()),
    );

    // The same three controls a stroke's combine has, addressing the object
    // rather than the next gesture. An operation is a property of the object
    // and stays editable for as long as it does.
    let settings = object.combine;
    // The three booleans first, as chips with the two discs on them: these
    // are what a placed shape is for, and a sculptor should not have to open
    // a list of thirteen to find "cut". The list below keeps the rest. Wrapped,
    // because the row now stands in the panel rather than in a window sized to
    // it, and Interseção does not fit beside the other two there.
    ui.horizontal_wrapped(|ui| {
        for (op, icon) in [
            (Combine::Add, Icon::Union),
            (Combine::Subtract, Icon::Subtract),
            (Combine::Intersect, Icon::Intersect),
        ] {
            let on = settings.op == op;
            if icon_chip(ui, icon, s.combine_name(op), on, Tokens::panel()).clicked() && !on {
                queue.push(Command::SetObjectCombine(CombineSettings {
                    op,
                    ..settings
                }));
            }
        }
    });
    egui::ComboBox::from_id_salt("object-combine-op")
        .selected_text(s.combine_name(settings.op))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for op in Combine::offered_for_strokes() {
                if ui
                    .selectable_label(op == settings.op, s.combine_name(op))
                    .clicked()
                    && op != settings.op
                {
                    queue.push(Command::SetObjectCombine(CombineSettings {
                        op,
                        ..settings
                    }));
                }
            }
        });

    if settings.op.takes_a_blend() {
        egui::ComboBox::from_id_salt("object-combine-blend")
            .selected_text(s.blend_name(settings.blend))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for blend in BlendProfile::ALL {
                    if ui
                        .selectable_label(blend == settings.blend, s.blend_name(blend))
                        .clicked()
                        && blend != settings.blend
                    {
                        queue.push(Command::SetObjectCombine(CombineSettings {
                            blend,
                            ..settings
                        }));
                    }
                }
            });

        // `radius_range` is what keeps the seven operations that do nothing
        // at zero away from it: that is not a hard join, it is no operation,
        // and a sculptor who lands there sees a tool that appears broken with
        // nothing to say why. The same call the stroke's own slider makes, so
        // the two cannot come to disagree about what is reachable.
        if let Some(radius) = slider(
            ui,
            settings.radius_label(),
            settings.radius,
            settings.radius_range(),
            3,
        ) {
            queue.push(Command::SetObjectCombine(CombineSettings {
                radius,
                ..settings
            }));
        }
    }

    // How the manipulator on it behaves. Here and under the object list, so
    // the mode can be changed from whichever of the two is open.
    ui.add_space(space::SNUG);
    ui.label(
        egui::RichText::new(s.label_manipulator)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    gizmo_mode_row(ui, state, state.gizmo_mode, true, queue);

    ui.add_space(space::SNUG);
    ui.label(
        // Three factors where they differ and one where they do not, so a
        // uniformly scaled object still reads as one number rather than as
        // the same number three times.
        egui::RichText::new(format!(
            "{}: {}",
            s.label_object_scale,
            scale_text(object.scale)
        ))
        .size(type_scale::LABEL)
        .color(Tokens::text_dim()),
    );
    ui.label(
        egui::RichText::new(s.hint_axis_scale)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );

    ui.add_space(space::SNUG);
    if ui.button(s.action_remove_object).clicked() {
        queue.push(Command::RemoveObject);
    }
}

/// What a placed object is called.
///
/// A shape's own name in this language, or — for a model somebody imported —
/// the name of the layer it came from, which is the only name it has and is
/// not ours to translate.
pub(super) fn object_name(state: &ShellState<'_>, object: &clayspace_model::SceneObject) -> String {
    match object.source.shape() {
        Some(shape) => state.strings.shape(shape).to_string(),
        None => object.label(),
    }
}

/// The placed objects, as rows that can be picked.
///
/// Only the objects. A worked layer holds hundreds of stroke items and showing
/// them as rows would be a worse scene panel than none — which is why
/// objecthood is recorded rather than inferred from what a layer contains.
pub fn object_rows(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    // A section like Scene and Layers above it, not a label: it is a list of
    // things in the scene, and it read as a stray caption between two
    // headings.
    if !heading(ui, s.section_objects) {
        return;
    }

    if state.objects.is_empty() {
        ui.label(
            egui::RichText::new(s.label_no_placed_objects)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        return;
    }

    for object in state.objects {
        let chosen = state.selected_object == Some(object.id);
        // Named by its shape and its operation, which is what tells two
        // cylinders apart when one adds and the other cuts.
        let label = format!(
            "{} · {}",
            object_name(state, object),
            s.combine_name(object.combine.op)
        );
        if ui.selectable_label(chosen, label).clicked() {
            // Clicking the selected row clears it, so the manipulator can be
            // put away without reaching for the viewport.
            let next = (!chosen).then_some(object.id);
            queue.push(Command::SelectObject(next));
        }
    }

    // The manipulator's modes, while something is selected for it to act on.
    // Until this row existed the modes could only be changed with a cage up,
    // so an object's manipulator moved and did nothing else.
    if state.selected_object.is_some() {
        ui.add_space(space::TIGHT);
        gizmo_mode_row(ui, state, state.gizmo_mode, true, queue);
    }
}
