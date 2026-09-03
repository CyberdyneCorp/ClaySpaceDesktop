//! The tool options bar: what the whole form is worked with, and the numbers
//! the stroke is made under.
//!
//! Headed by the two controls that act on the form rather than on a point of
//! it — the manipulator on the whole layer, and the deformations — because
//! they are modes a sculptor enters and leaves, and a mode belongs at the top
//! of the window where its state can be seen without looking for it. The brush
//! stood here once and does not now: the shelf along the bottom already draws
//! which brush is in hand, lit and named, and the same fact twice on one screen
//! is a row of pixels that says nothing new.
//!
//! What follows is the settings that change most often, and only those the
//! active representation and the active tool actually read — the combine
//! vocabulary is the field's alone, and the colour swatch appears for the two
//! tools that write one.
//!
//! Every group is as wide as its longest word and no wider, and a hairline
//! rather than twenty pixels of air stands between two of them: the bar was a
//! hundred and sixty pixels wider than the window it is drawn in, which put
//! Alpha off the right edge in every language. A window narrower than the bar
//! still scrolls it sideways rather than cutting the last control off.

use super::*;

/// The tool options bar: what works the whole form, and the stroke's own
/// numbers. Always visible, and sized to end inside the design's 1280.
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
                ui.add_space(space::SNUG);
                form_controls(ui, state, queue);
                bar_rule(ui);
                ui.vertical(|ui| {
                    ui.set_width(OPTION_SLIDER_WIDTH);
                    if let Some(value) =
                        slider(ui, s.label_intensity, state.brush.intensity, 0.0..=1.0, 2)
                    {
                        queue.push(Command::SetBrushIntensity(value));
                    }
                });
                ui.add_space(space::SNUG);
                ui.vertical(|ui| {
                    ui.set_width(OPTION_SLIDER_WIDTH);
                    // The readout carries the size in the sculptor's unit; the
                    // slider keeps editing engine units. A unit-aware slider
                    // whose range shifts under the pointer when the unit is
                    // switched is one nobody trusts. The label said the
                    // millimetres and the readout beside it said the same size
                    // as a fraction of the field — one fact twice, in the
                    // widest control on a bar that had run off the window.
                    if let Some(value) = slider_reading(
                        ui,
                        s.label_size,
                        s.label_size,
                        &state.units.format(state.brush.size),
                        state.brush.size,
                        0.005..=1.0,
                        3,
                    ) {
                        queue.push(Command::SetBrushSize(value));
                    }
                });
                ui.add_space(space::SNUG);
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
                    bar_rule(ui);
                    mask_gesture_control(ui, state, queue);
                }

                ui.add_space(space::SNUG);
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

                bar_rule(ui);
                symmetry_control(ui, state, queue);

                // The combine vocabulary is the SDF side's alone: cells are set or
                // cleared and vertices are moved, so neither has a join to make. The
                // controls are absent rather than greyed because there is no
                // representation-independent meaning for them to be disabled *from*.
                if state.representation == Representation::Sdf {
                    bar_rule(ui);
                    combine_controls(ui, state, queue);
                }

                // Shown only where it is read. Two of the twenty tools write
                // colour, and a swatch beside a Standard brush would be a
                // control that does nothing — `ToolKind::writes_colour` is the
                // same question the engine adapter asks before resolving a
                // palette entry, so the swatch appears exactly where the value
                // is consumed.
                if state.tool.writes_colour() {
                    bar_rule(ui);
                    colour_control(ui, state, queue);
                }

                bar_rule(ui);
                alpha_control(ui, state, queue);
                ui.add_space(space::SNUG);
            });
        });
}

/// The dim word over a group on the bar.
///
/// One helper rather than six copies of the same three lines: the mask
/// gesture's heading was set in the monospaced face the readouts use and stood
/// out from Simetria and Operação beside it, which is the kind of drift a
/// shared helper is for.
fn group_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
}

/// A hairline between two groups on the bar.
///
/// Groups were told apart by a wide gap, which cost twenty pixels a boundary
/// and still read as one run of controls at a glance. A rule says the same
/// thing in one pixel and says it better: what belongs to the stroke, what
/// belongs to the mirror, what belongs to the join.
fn bar_rule(ui: &mut egui::Ui) {
    ui.add_space(space::TIGHT);
    let (rule, _) = ui.allocate_exact_size(egui::vec2(1.0, BAR_RULE_HEIGHT), egui::Sense::hover());
    ui.painter().rect_filled(rule, 0.0, Tokens::rule());
    ui.add_space(space::TIGHT);
}

/// How tall a group separator stands: the height of a label over a control,
/// so it ends where the controls it divides end rather than running past them.
const BAR_RULE_HEIGHT: f32 = 36.0;

/// How wide the mask brush's three gestures sit, where their words fit in it.
/// `segmented` grows past this rather than cutting an entry, so it is a floor
/// and not a promise — Retângulo needs more of it than Rectangle does.
const GESTURE_WIDTH: f32 = 140.0;

/// Which gesture the mask brush makes: a drag across the surface, a shape
/// traced over the form, or a box dragged corner to corner.
fn mask_gesture_control(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.vertical(|ui| {
        ui.set_width(GESTURE_WIDTH);
        group_label(ui, s.label_mask_gesture);
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
        group_label(ui, s.label_symmetry);
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
                let response = ui.add_enabled(
                    !state.armature.editing || matches!(axis, Axis::X),
                    chip_tinted(axis.label(), on, Tokens::ground(), Tokens::selection_soft()),
                );
                if with_chord(response, state, action).clicked() {
                    queue.push(Command::ToggleSymmetry(*axis));
                }
            }
        });
    });
}

/// The id the whole-subtool manipulator chip carries, so a test can find it.
///
/// The same arrangement [`slider_id`] has and for the same reason: a control
/// this bar draws is wiring that has to be exercised, and reaching it by
/// coordinate reaches whatever landed beside it instead.
pub fn layer_transform_chip_id() -> egui::Id {
    egui::Id::new("layer-transform")
}

/// The id the chip that opens the deformations carries.
pub fn deform_chip_id() -> egui::Id {
    egui::Id::new("options-deform")
}

/// What acts on the whole form: the manipulator on the active layer, and the
/// deformations.
///
/// At the head of the bar, where the brush badge was. Both are modes rather
/// than amounts — one puts a widget up over the clay, the other opens the
/// deformation panel — and a mode a sculptor is in and cannot see is the most
/// expensive thing this window can hide.
fn form_controls(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.vertical(|ui| {
        // A dim label, as Simetria and Operação beside it are.
        group_label(ui, s.label_transform);
        ui.horizontal(|ui| {
            layer_transform_chip(ui, state, queue);
            deform_chip(ui, state, queue);
        });
    });
}

/// The manipulator that moves, turns and scales the whole active layer.
///
/// One chip rather than three. It puts the widget up and takes it away, and
/// W, E and R say which of the three modes it is in — Maya's keys and Unity's,
/// so a hand coming from either already knows them. Three chips said the same
/// thing three times and spent the head of the bar saying it; the chip wears
/// the mode's own shape, so what the widget will do is still readable without
/// pressing anything.
///
/// Live only where nothing smaller owns the widget. A cage that is up, a curve
/// being authored and a selected object each already have the manipulator, and
/// two of them over one selection is a press nobody can aim — which is the same
/// rule `begin_gizmo_drag` applies on the other side. Greyed with the reason on
/// it rather than taken away, because it stands at the head of the bar and a
/// chip that came and went would shift every slider beside it.
fn layer_transform_chip(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    let taken = state.selected_object.is_some() || state.lattice.active || state.curve.active;
    let key = state.scene.active.filter(|_| !taken);
    let on = key.is_some() && state.gizmo_target == key.map(clayspace_model::GizmoTarget::Layer);

    // The mode's own shape and the mode's own word — the arrow, the ring, the
    // box that every other mode row draws — so the widget on a whole subtool
    // reads as the one widget it is, and the chip says which of the three the
    // next press on the clay is in without being hovered.
    let response = icon_chip_recorded(
        ui,
        gizmo_mode_icon(state.gizmo_mode),
        s.gizmo_mode_name(state.gizmo_mode),
        on,
        Tokens::panel(),
        false,
        key.is_some(),
    );
    // Recorded where a test can find it, for the reason `slider_id` states: a
    // control reached by pixel coordinate is a different control the next time
    // something lands beside it.
    ui.ctx().memory_mut(|memory| {
        memory
            .data
            .insert_temp(layer_transform_chip_id(), response.rect)
    });

    let Some(key) = key else {
        response.on_hover_text(if taken {
            s.hint_transform_taken
        } else {
            s.hint_transform_needs_a_layer
        });
        return;
    };
    // The three modes with the key that reaches each, and under them what the
    // widget does — the sentence the three chips used to carry between them.
    // The keys come from the shortcut table rather than from this string, so a
    // rebound one is the one the tooltip names.
    let modes: Vec<String> = GizmoMode::ALL
        .iter()
        .map(|&mode| labelled_chord(state, s.gizmo_mode_name(mode), gizmo_mode_action(mode)))
        .collect();
    let hint = format!("{}\n{}", modes.join("\n"), s.hint_layer_transform);
    if response.on_hover_text(hint).clicked() {
        // Pressing the chip that is already lit puts the manipulator away, so
        // a form can be looked at without one standing over the middle of it.
        // The same bargain the object rows make, where clicking the selected
        // row clears it.
        queue.push(Command::SetGizmoTarget(
            (!on).then_some(clayspace_model::GizmoTarget::Layer(key)),
        ));
    }
}

/// The whole-form deformations, opened from the head of the bar.
///
/// Lit while the panel is up, and the same chip closes it — the toggle the
/// tool rail and the Dinâmica menu both push, so the three cannot disagree.
fn deform_chip(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let response = icon_chip_recorded(
        ui,
        Icon::Taper,
        state.strings.action_deform,
        state.show_deform,
        Tokens::panel(),
        false,
        true,
    );
    ui.ctx()
        .memory_mut(|memory| memory.data.insert_temp(deform_chip_id(), response.rect));
    if response.clicked() {
        queue.push(Command::ToggleDeform);
    }
}

/// How wide the combine vocabulary's two lists sit. Their longest words —
/// Substituir and Quadrática — set them; narrower and the list cuts its own
/// entries.
const COMBINE_OP_WIDTH: f32 = 106.0;
const COMBINE_BLEND_WIDTH: f32 = 94.0;

/// How wide the alpha stamp's name and its buttons sit.
pub(super) const ALPHA_CONTROL_WIDTH: f32 = 126.0;

/// One of the bar's three sliders. Sized so the bar fits the design's 1280
/// with the form controls at its head; below that the bar scrolls.
pub(super) const OPTION_SLIDER_WIDTH: f32 = 96.0;

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
        ui.set_width(COMBINE_OP_WIDTH);
        group_label(ui, s.label_combine);
        egui::ComboBox::from_id_salt("combine-op")
            .selected_text(state.strings.combine_name(settings.op))
            .width(COMBINE_OP_WIDTH)
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
        ui.set_width(COMBINE_BLEND_WIDTH);
        group_label(ui, s.label_blend);
        egui::ComboBox::from_id_salt("combine-blend")
            .selected_text(state.strings.blend_name(settings.blend))
            .width(COMBINE_BLEND_WIDTH)
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
        ui.set_width(OPTION_SLIDER_WIDTH);
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
        group_label(ui, s.label_colour);
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
pub(super) const COLOUR_CONTROL_WIDTH: f32 = 132.0;

/// The id the alpha group is recorded under, so a test can ask whether the
/// bar still ends inside the window.
pub fn alpha_control_id() -> egui::Id {
    egui::Id::new("options-alpha")
}

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

    let group = ui.vertical(|ui| {
        ui.set_width(ALPHA_CONTROL_WIDTH);
        group_label(ui, s.label_alpha);

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
    // The last group on the bar, and so the one a bar too wide for the window
    // cuts first. Recorded where a test can measure it: the bar ran off the
    // right edge once and the Alpha nobody could see was how it was noticed.
    ui.ctx().memory_mut(|memory| {
        memory
            .data
            .insert_temp(alpha_control_id(), group.response.rect)
    });
}
