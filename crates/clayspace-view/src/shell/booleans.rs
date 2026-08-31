//! The boolean section: two operands, an operation, and what the pair costs.
//!
//! Docked rather than floated. It was a window over the viewport, and the
//! viewport is where the form being cut stands — the panel covered the thing
//! it was being used on.

use super::*;

/// The id one operation chip carries, so a test can press it by name.
///
/// By position rather than by label, for the reason [`insert_as_chip_id`]
/// states: an id built from interface text moves when a translation does.
pub fn boolean_op_chip_id(op: clayspace_model::BooleanOp) -> egui::Id {
    egui::Id::new(("boolean-op", op as u8))
}

/// Resolving a boolean between two subtools.
///
/// Honest about being *resolved* rather than live, which is the whole of what
/// this section has to say beyond its four controls: the engine composes
/// layers by hard union (ClayCore #321), so what comes out is baked, and the
/// operands are kept because that is what makes the operation recoverable.
///
/// A section of the right panel rather than a window, because a window floats
/// over the viewport, and the viewport is where the two forms being cut from
/// one another stand: it hid the very thing the operation was being set up
/// on. Docked, the operands stay in view while the sentence about them is
/// being composed. Closed from its own heading, as the window was from its
/// title bar, or from the rail.
pub(super) fn boolean_section(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    if closable_heading(ui, s.section_boolean) {
        queue.push(Command::ToggleBoolean);
    }
    boolean_operation(ui, state, queue);
    ui.add_space(space::SNUG);
    boolean_operands(ui, state, queue);
    ui.add_space(space::SNUG);
    boolean_resolution(ui, state, queue);
    ui.add_space(space::ROOMY);
    boolean_costs(ui, state);
    ui.add_space(space::ROOMY);
    boolean_confirm(ui, state, queue);
}

/// Which of the three operations.
pub(super) fn boolean_operation(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    ui.label(
        egui::RichText::new(s.label_boolean_op)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    ui.horizontal(|ui| {
        for op in clayspace_model::BooleanOp::ALL {
            let on = state.boolean.op == op;
            let response = ui.add(chip(s.boolean_op(op), on, Tokens::panel()));
            ui.ctx().memory_mut(|memory| {
                memory
                    .data
                    .insert_temp(boolean_op_chip_id(op), response.rect)
            });
            if response.clicked() && !on {
                queue.push(Command::SetBoolean(clayspace_model::BooleanSettings {
                    op,
                    ..state.boolean
                }));
            }
        }
    });
}

/// The two operands, each under the name of the role it plays.
///
/// Both are named because subtraction is not symmetric: "A minus B" is the
/// whole of what the sculptor is choosing, and a panel that offered two
/// unlabelled slots would leave them to find out which way round it went by
/// running it.
pub(super) fn boolean_operands(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    boolean_operand_picker(ui, state, queue, Role::Base);
    ui.add_space(space::TIGHT);
    boolean_operand_picker(ui, state, queue, Role::Tool);

    // The sentence the operation reads as, spelled out where the order
    // matters. Swapping the two above changes it, which is the point.
    if !state.boolean.op.is_symmetric() {
        if let (Some(base), Some(tool)) = (state.boolean.base, state.boolean.tool) {
            ui.add_space(space::TIGHT);
            ui.label(
                egui::RichText::new(format!(
                    "{} {} {}",
                    boolean_operand_name(state, Some(base)),
                    s.boolean_minus,
                    boolean_operand_name(state, Some(tool))
                ))
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
            );
        }
    }
}

/// Which of the two an operand picker is for.
///
/// Named rather than passed as a setter, because the two differ in three
/// things at once — the label, the widget's id and which field a choice lands
/// in — and a picker that took all three separately could be given a label for
/// one and a field for the other.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Base,
    Tool,
}

/// One operand's label and its picker.
pub(super) fn boolean_operand_picker(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
    role: Role,
) {
    let s = state.strings;
    let (id, label, chosen) = match role {
        Role::Base => ("boolean-base", s.label_boolean_base, state.boolean.base),
        Role::Tool => ("boolean-tool", s.label_boolean_tool, state.boolean.tool),
    };
    ui.label(
        egui::RichText::new(label)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    egui::ComboBox::from_id_salt(id)
        .selected_text(boolean_operand_name(state, chosen))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (key, name) in state.boolean_operands {
                if ui.selectable_label(chosen == Some(*key), name).clicked() {
                    let mut settings = state.boolean;
                    match role {
                        Role::Base => settings.base = Some(*key),
                        Role::Tool => settings.tool = Some(*key),
                    }
                    queue.push(Command::SetBoolean(settings));
                }
            }
        });
}

/// What a chosen operand is called, or the prompt where none is chosen.
pub(super) fn boolean_operand_name(state: &ShellState<'_>, key: Option<LayerKey>) -> String {
    key.and_then(|key| {
        state
            .boolean_operands
            .iter()
            .find(|(candidate, _)| *candidate == key)
    })
    .map(|(_, name)| name.clone())
    .unwrap_or_else(|| state.strings.boolean_pick_one.to_string())
}

/// The cell the result is sampled at. The sculptor's to change, starting from
/// the operands' own detail.
pub(super) fn boolean_resolution(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    if let Some(cell) = slider_named(
        ui,
        "boolean-cell",
        s.label_cell_size,
        state.boolean.cell_size,
        clayspace_model::ConversionSettings::CELL_RANGE,
        3,
    ) {
        queue.push(Command::SetBoolean(clayspace_model::BooleanSettings {
            cell_size: cell,
            ..state.boolean
        }));
    }
}

/// What the operation costs, before it runs.
///
/// The conversion panel's own lines, because the result is sampled onto a
/// lattice exactly as a crossing is — plus the two sentences that belong to
/// this operation alone: that it is resolved rather than live, and what
/// becomes of the operands.
pub(super) fn boolean_costs(ui: &mut egui::Ui, state: &ShellState<'_>) {
    let s = state.strings;
    // The heading only where there are figures under it: a pair has not been
    // chosen yet, and a heading over nothing reads as a panel that failed to
    // work something out.
    if let Some(cost) = state.boolean_cost {
        ui.label(
            egui::RichText::new(s.label_convert_costs)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        for line in crossing_cost_lines(state, clayspace_model::Direction::SdfToVoxel, cost) {
            ui.label(
                egui::RichText::new(line)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
        }
    }
    ui.label(
        egui::RichText::new(s.boolean_resolved)
            .size(type_scale::LABEL)
            .color(Tokens::accent()),
    );
    if !state.boolean.consume {
        ui.label(
            egui::RichText::new(s.boolean_keeps_operands)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
    }
}

/// What happens to the operands, and the consent itself.
pub(super) fn boolean_confirm(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    let mut consume = state.boolean.consume;
    if ui
        .checkbox(&mut consume, s.action_boolean_consume)
        .changed()
    {
        queue.push(Command::SetBoolean(clayspace_model::BooleanSettings {
            consume,
            ..state.boolean
        }));
    }
    if state.boolean.consume {
        // Said before it runs rather than discovered after: consuming is the
        // one choice here that cannot be reconsidered from what is left.
        ui.label(
            egui::RichText::new(s.hint_boolean_consume)
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }

    ui.add_space(space::SNUG);
    // Nothing runs unconfirmed, and nothing is offered to confirm until there
    // are two different subtools to run it between.
    let ready = state.boolean.pair().is_some();
    if ui
        .add_enabled(ready, egui::Button::new(s.action_boolean_run))
        .clicked()
    {
        queue.push(Command::RunBoolean);
    }
    if !ready {
        ui.label(
            egui::RichText::new(s.boolean_pick_two)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
    }
    if let Some(refusal) = state.boolean_notice {
        ui.label(
            egui::RichText::new(refusal)
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }
}
