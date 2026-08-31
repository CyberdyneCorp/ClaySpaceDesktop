//! The tool options bar: the active brush, and the numbers that belong to it.
//!
//! Headed by the brush itself, which is ZBrush's arrangement: a glance at the
//! bar says *which* brush the intensity is for without looking down at the
//! shelf. What follows is the settings that change most often, and only those
//! the active representation and the active tool actually read — the combine
//! vocabulary is the field's alone, and the colour swatch appears for the two
//! tools that write one.

use super::*;

/// The tool options bar: the active brush's primary parameters, always visible.
pub fn options_bar(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.add_space(space::SNUG);
    // Scrolls sideways when the window is narrower than the bar, rather than
    // cutting the last control off: a clipped Alpha is one nobody knows is
    // there.
    egui::ScrollArea::horizontal()
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(space::PANEL);
                brush_badge(ui, state);
                ui.add_space(space::ROOMY);
                // A hairline between the brush and its settings, so the row reads
                // as "this brush: these numbers" rather than as a run of sliders.
                let (rule, _) =
                    ui.allocate_exact_size(egui::vec2(1.0, size::BADGE), egui::Sense::hover());
                ui.painter().rect_filled(rule, 0.0, Tokens::rule());
                ui.add_space(space::ROOMY);
                ui.vertical(|ui| {
                    ui.set_width(OPTION_SLIDER_WIDTH);
                    if let Some(value) =
                        slider(ui, s.label_intensity, state.brush.intensity, 0.0..=1.0, 2)
                    {
                        queue.push(Command::SetBrushIntensity(value));
                    }
                });
                ui.add_space(space::ROOMY);
                ui.vertical(|ui| {
                    ui.set_width(OPTION_SLIDER_WIDTH);
                    // The label carries the size on the model; the slider keeps
                    // editing engine units. A unit-aware slider whose range shifts
                    // under the pointer when the unit is switched is one nobody
                    // trusts, and the options bar has a fixed height that a second
                    // row would overflow.
                    let label = format!(
                        "{} · {}",
                        s.label_size,
                        state.units.format(state.brush.size)
                    );
                    if let Some(value) = slider(ui, &label, state.brush.size, 0.005..=1.0, 3) {
                        queue.push(Command::SetBrushSize(value));
                    }
                });
                ui.add_space(space::ROOMY);
                ui.vertical(|ui| {
                    ui.set_width(OPTION_SLIDER_WIDTH);
                    if let Some(value) = slider(ui, s.label_flow, state.brush.flow, 0.01..=1.0, 2) {
                        queue.push(Command::SetBrushFlow(value));
                    }
                });

                // Shown only with the mask brush in hand, because it is that
                // brush's own question: one of the twenty tools freezes a
                // region, and a Pincel/Laço pair beside a Standard brush would
                // be a control that decides nothing. Beside the brush's own
                // numbers rather than at the end of the bar, where it sat
                // first and where a narrow window pushed half of it off the
                // screen.
                if state.tool.is_mask_tool() {
                    ui.add_space(space::SECTION);
                    mask_gesture_control(ui, state, queue);
                }

                ui.add_space(space::ROOMY);
                ui.vertical(|ui| {
                    ui.set_width(OPTION_SLIDER_WIDTH);
                    // Lazy-mouse lag, moved here from the brush-controls
                    // section: it shapes the stroke being made, which is what
                    // this bar is for, and a sculptor reaching for it mid-line
                    // should not have to find a panel section.
                    if let Some(value) = slider(
                        ui,
                        s.label_smoothing,
                        state.brush.shaping.smoothing,
                        0.0..=0.95,
                        2,
                    ) {
                        queue.push(Command::SetBrushSmoothing(value));
                    }
                });

                ui.add_space(space::SECTION);
                symmetry_control(ui, state, queue);

                // The combine vocabulary is the SDF side's alone: cells are set or
                // cleared and vertices are moved, so neither has a join to make. The
                // controls are absent rather than greyed because there is no
                // representation-independent meaning for them to be disabled *from*.
                if state.representation == Representation::Sdf {
                    ui.add_space(space::SECTION);
                    combine_controls(ui, state, queue);
                }

                // Shown only where it is read. Two of the twenty tools write
                // colour, and a swatch beside a Standard brush would be a
                // control that does nothing — `ToolKind::writes_colour` is the
                // same question the engine adapter asks before resolving a
                // palette entry, so the swatch appears exactly where the value
                // is consumed.
                if state.tool.writes_colour() {
                    ui.add_space(space::SECTION);
                    colour_control(ui, state, queue);
                }

                ui.add_space(space::SECTION);
                alpha_control(ui, state, queue);
            });
        });
}

/// How wide the mask brush's three gestures need between them.
const GESTURE_WIDTH: f32 = 190.0;

/// Which gesture the mask brush makes: a drag across the surface, a shape
/// traced over the form, or a box dragged corner to corner.
fn mask_gesture_control(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.vertical(|ui| {
        ui.set_width(GESTURE_WIDTH);
        numeric(ui, s.label_mask_gesture);
        let response = ui.scope(|ui| {
            segmented(
                ui,
                &MaskGesture::ALL,
                |gesture| s.mask_gesture_name(gesture),
                state.mask_gesture,
            )
        });
        if let Some(gesture) = response.inner {
            queue.push(Command::SetMaskGesture(gesture));
        }
        // What the drawn gestures do and what the modifier does to them, on
        // the row itself: a gesture nobody can discover is one nobody uses.
        response.response.on_hover_text(s.hint_mask_outline);
    });
}

/// Which axes a stroke mirrors across.
///
/// Moved out of the left region's sculpt-settings section, which held nothing
/// else and is gone with it. Symmetry belongs to the stroke rather than to the
/// scene, and every other thing the stroke is made under is on this bar.
fn symmetry_control(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.vertical(|ui| {
        // A dim label, as Operação and Junção beside it are. `numeric` is a
        // monospaced face for digits, and the heading over three axis chips
        // came out heavier than every other heading on the bar.
        ui.label(
            egui::RichText::new(s.label_symmetry)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        ui.horizontal(|ui| {
            for (index, axis) in Axis::ALL.iter().enumerate() {
                let on = state.symmetry[index];
                let action = match axis {
                    Axis::X => Action::SymmetryX,
                    Axis::Y => Action::SymmetryY,
                    Axis::Z => Action::SymmetryZ,
                };
                // The engaged axis wears the soft accent rather than a raised
                // grey: symmetry is state a sculptor needs to see without
                // looking for it, and a mirrored stroke they did not expect is
                // the most expensive surprise on this bar.
                let response = ui.add(chip_tinted(
                    axis.label(),
                    on,
                    Tokens::ground(),
                    Tokens::selection_soft(),
                ));
                if with_chord(response, state, action).clicked() {
                    queue.push(Command::ToggleSymmetry(*axis));
                }
            }
        });
    });
}

/// The id the options bar's brush badge is recorded under, for tests.
pub fn brush_badge_id() -> egui::Id {
    egui::Id::new("options-brush-badge")
}

/// The active brush at the head of its own settings.
///
/// ZBrush puts the brush where its numbers are, and a sculptor glancing at
/// the bar sees *which* brush the intensity belongs to without looking down
/// at the shelf. The same ball and the same mark the shelf draws, its name
/// beside it, and what it does in one line under the name — the sentence the
/// shelf only gives on hover, here where there is room for it.
pub(super) fn brush_badge(ui: &mut egui::Ui, state: &ShellState<'_>) {
    let s = state.strings;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(size::BADGE, size::BADGE), egui::Sense::hover());
    paint_sphere(ui, rect, Tokens::text_dim(), false);
    glyphs::paint(ui.painter(), rect, state.tool, Tokens::ground());
    ui.ctx()
        .memory_mut(|memory| memory.data.insert_temp(brush_badge_id(), rect));
    ui.add_space(space::TIGHT);
    ui.vertical(|ui| {
        ui.set_width(BADGE_TEXT_WIDTH);
        ui.add_space(space::TIGHT);
        ui.label(
            egui::RichText::new(s.tool(state.tool))
                .size(type_scale::BODY)
                .color(Tokens::text()),
        );
        // The first line on the badge and the whole sentence on hover: the
        // badge is one row, and a caveat drawn into it would push the numbers
        // off the bar.
        let sentence = s.tool_sentence(state.tool, state.representation);
        ui.add(
            egui::Label::new(
                egui::RichText::new(s.tool_hint(state.tool))
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            )
            .truncate(),
        )
        .on_hover_text(sentence);
    });
}

/// How wide the badge's name and sentence may run before the sentence is cut.
pub(super) const BADGE_TEXT_WIDTH: f32 = 132.0;

/// One of the bar's three sliders. Sized so the bar fits the design's 1280
/// with the badge at its head; below that the bar scrolls.
pub(super) const OPTION_SLIDER_WIDTH: f32 = 128.0;

/// The combine operation, its join profile, and how wide the join reaches.
///
/// Three controls rather than one list of their product: an operation with a
/// hard join and the same one rounded are the same operation and different
/// shapes, and a list of seventy entries is not a vocabulary anybody learns.
pub(super) fn combine_controls(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    let settings = state.combine;

    ui.vertical(|ui| {
        ui.set_width(130.0);
        ui.label(
            egui::RichText::new(s.label_combine)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        egui::ComboBox::from_id_salt("combine-op")
            .selected_text(state.strings.combine_name(settings.op))
            .width(130.0)
            .show_ui(ui, |ui| {
                for op in Combine::offered_for_strokes() {
                    if ui
                        .selectable_label(op == settings.op, state.strings.combine_name(op))
                        .clicked()
                        && op != settings.op
                    {
                        queue.push(Command::SetCombine(CombineSettings { op, ..settings }));
                    }
                }
            });
    });

    // A profile and a width describe how a *join* is rounded. Replace discards
    // what was under it and Paint touches no surface, so neither makes one —
    // and offering the controls beside them would be offering two that do
    // nothing.
    if !settings.op.takes_a_blend() {
        return;
    }

    ui.add_space(space::SNUG);
    ui.vertical(|ui| {
        ui.set_width(120.0);
        ui.label(
            egui::RichText::new(s.label_blend)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        egui::ComboBox::from_id_salt("combine-blend")
            .selected_text(state.strings.blend_name(settings.blend))
            .width(120.0)
            .show_ui(ui, |ui| {
                for blend in BlendProfile::ALL {
                    if ui
                        .selectable_label(blend == settings.blend, state.strings.blend_name(blend))
                        .clicked()
                        && blend != settings.blend
                    {
                        queue.push(Command::SetCombine(CombineSettings { blend, ..settings }));
                    }
                }
            });
    });

    ui.add_space(space::SNUG);
    ui.vertical(|ui| {
        ui.set_width(130.0);
        // The same number means the amplitude a stroke displaces by for the
        // relief family and the width of the join for every other operation,
        // so the label follows the operation rather than being fixed.
        if let Some(radius) = slider(
            ui,
            settings.radius_label(),
            settings.radius,
            settings.radius_range(),
            3,
        ) {
            queue.push(Command::SetCombine(CombineSettings { radius, ..settings }));
        }
    });
}

/// The colour a colour brush paints with, and the ones just before it.
///
/// A swatch and a row of recents rather than a full palette editor: the
/// question a sculptor asks mid-pass is "back to the red I was using", and six
/// squares answer it. Anything larger is a palette feature and belongs
/// somewhere a stroke is not being made.
pub(super) fn colour_control(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    let current = state.colour.current();

    ui.vertical(|ui| {
        ui.set_width(COLOUR_CONTROL_WIDTH);
        ui.label(
            egui::RichText::new(s.label_colour)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        ui.horizontal(|ui| {
            // egui edits in sRGB bytes; the engine's palettes and vertex
            // colours are linear. Converting at the boundary rather than
            // storing what the widget happens to hand back is what keeps the
            // swatch and the painted cell the same colour.
            // the sculptor's own colour
            let mut edited = egui::Color32::from_rgb(
                to_srgb_byte(current.rgb[0]),
                to_srgb_byte(current.rgb[1]),
                to_srgb_byte(current.rgb[2]),
            );
            if ui.color_edit_button_srgba(&mut edited).changed() {
                queue.push(Command::SetBrushColour(clayspace_model::Colour::new([
                    from_srgb_byte(edited.r()),
                    from_srgb_byte(edited.g()),
                    from_srgb_byte(edited.b()),
                ])));
            }
            ui.label(
                egui::RichText::new(current.hex())
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
        });

        if state.colour.recent().is_empty() {
            return;
        }
        ui.horizontal(|ui| {
            for (index, colour) in state.colour.recent().iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(size::COLOUR_CHIP, size::COLOUR_CHIP),
                    egui::Sense::click(),
                );
                ui.painter().rect_filled(
                    rect,
                    2.0,
                    // the sculptor's own colour
                    egui::Color32::from_rgb(
                        to_srgb_byte(colour.rgb[0]),
                        to_srgb_byte(colour.rgb[1]),
                        to_srgb_byte(colour.rgb[2]),
                    ),
                );
                if response.on_hover_text(colour.hex()).clicked() {
                    queue.push(Command::PickRecentColour(index));
                }
            }
        })
        .response
        .on_hover_text(s.label_recent_colours);
    });
}

/// How wide the swatch, its hex and the recent row sit.
pub(super) const COLOUR_CONTROL_WIDTH: f32 = 150.0;

/// The alpha stamp: which one is loaded, and whether this brush uses it.
///
/// Where a stamp cannot be used the reason is shown in place of the toggle,
/// rather than the toggle being drawn and doing nothing. That distinction
/// matters more here than for most controls: two of the three representations
/// take a stamp and the third does not, so a sculptor meets the unavailable
/// case by moving between layers and needs to be told which of the two they
/// are in.
pub(super) fn alpha_control(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    let support = AlphaSupport::of(state.representation, state.combine.op);

    ui.vertical(|ui| {
        ui.set_width(180.0);
        ui.label(
            egui::RichText::new(s.label_alpha)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );

        if !support.accepted() {
            // Cut to the bar's width, with the whole sentence on hover. The
            // reason is a paragraph, and the bar is one row: drawn whole it
            // ran under the panel beside it and was read by nobody.
            let sentence = support.to_string();
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&sentence)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                )
                .truncate(),
            )
            .on_hover_text(sentence);
            return;
        }

        ui.horizontal(|ui| match state.alpha {
            Some(name) => {
                let mut on = state.brush.alpha;
                if ui.checkbox(&mut on, name).changed() {
                    queue.push(Command::SetBrushAlpha(on));
                }
                if ui
                    .small_button("×")
                    .on_hover_text(s.action_clear_alpha)
                    .clicked()
                {
                    queue.push(Command::ClearAlpha);
                }
            }
            None => {
                if ui.button(s.action_load_alpha).clicked() {
                    queue.push(Command::LoadAlpha);
                }
            }
        });
    });
}
