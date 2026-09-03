//! The drawing vocabulary the regions are built from.
//!
//! Headings, chips, segmented bars, sliders, readouts, rails and the two
//! painted balls. Nothing here knows which region it is in — each takes state
//! and gives back a response or a value — which is what lets the same heading
//! and the same slider appear in the left panel, the right panel and a docked
//! window without three of them drifting apart.
//!
//! This is where a treatment changes. `sculpt_slider` is drawn once and
//! reached by thirty-odd call sites; styling them individually is how an
//! interface ends up with three kinds of slider.

use super::*;

/// The id a section's heading is recorded under, so a test can fold the
/// section by its word rather than by a pixel.
pub fn heading_id(section: &str) -> egui::Id {
    egui::Id::new(("heading", section))
}

/// Where a section's fold is kept between frames.
pub(super) fn fold_id(section: &str) -> egui::Id {
    egui::Id::new(("fold", section))
}

/// A section heading: small, spaced, low contrast. Hands back whether the
/// section under it is open, and a caller draws the body only then.
///
/// A hairline rule above it where something already stands above it, so the
/// sections of a long panel read as sections rather than as one column of
/// rows — by tone, as the design asks, and never by a box around them.
///
/// The whole row folds the section, with a chevron at its trailing end
/// saying which way it stands. The right panel carries ten sections and the
/// left has grown too; a sculptor working the brush controls should not have
/// to scroll past a material and a rig every time. The fold is interface
/// state and not document state: it lives in egui's own memory keyed by the
/// heading's word, so it enters no undo history, emits no command, and is
/// forgotten when the application closes — every section opens shown.
#[must_use]
pub(super) fn heading(ui: &mut egui::Ui, text: &str) -> bool {
    heading_rule(ui);
    let fold = fold_id(text);
    let was_open = ui.ctx().data(|data| data.get_temp(fold)).unwrap_or(true);
    let response = heading_row(ui, text, was_open);
    let open = if response.clicked() {
        !was_open
    } else {
        was_open
    };
    ui.ctx().data_mut(|data| {
        data.insert_temp(fold, open);
        data.insert_temp(heading_id(text), response.rect);
    });
    ui.add_space(space::TIGHT);
    open
}

/// The row a folding heading is: its word at the leading edge, the chevron at
/// the trailing one, and the whole width between them a place to click.
///
/// The chevron is faint at rest and lifts under the pointer, as every control
/// here does; it is the one mark on the row that is not text, and drawn in
/// the icon set so it reads as the tree's own collapse mark rather than a
/// second kind.
pub(super) fn heading_row(ui: &mut egui::Ui, text: &str, open: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::FontId::proportional(type_scale::HEADING),
        Tokens::text_faint(),
    );
    let height = galley.size().y.max(size::ICON);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    let painter = ui.painter();
    let text_at = egui::pos2(rect.left(), rect.center().y - galley.size().y * 0.5);
    painter.galley(text_at, galley, Tokens::text_faint());
    let tint = if response.hovered() {
        Tokens::text()
    } else {
        Tokens::text_faint()
    };
    let chevron = egui::Rect::from_center_size(
        egui::pos2(rect.right() - size::ICON * 0.5, rect.center().y),
        egui::Vec2::splat(size::ICON),
    );
    let icon = if open {
        Icon::Expanded
    } else {
        Icon::Collapsed
    };
    icons::paint(painter, chevron, icon, tint);
    response
}

/// The rule a heading stands under, where something already stands above it.
///
/// The break between two sections is one `space::SECTION`, spent either side of
/// the rule rather than piled on one: a rule hard against the section below it
/// reads as part of that section rather than as the boundary between two. The
/// larger half goes below, to the section the heading names.
///
/// Written as the section step less the group step rather than as two
/// constants that happen to add up to it. The pixels are the ones that were
/// already there; what changes is that the rhythm is now tied to the scale, so
/// moving `SECTION` moves the panels instead of leaving them on a number
/// nobody would think to look for here.
pub(super) fn heading_rule(ui: &mut egui::Ui) {
    if ui.min_rect().height() > 0.0 {
        ui.add_space(space::SECTION - space::ROOMY);
        let (rule, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().rect_filled(rule, 0.0, Tokens::rule());
    }
    ui.add_space(space::ROOMY);
}

/// A heading's word, set as the design sets every heading.
pub(super) fn heading_text(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .size(type_scale::HEADING)
        .color(Tokens::text_faint())
}

/// The id a section's close control is recorded under, so a test can put the
/// section away by its word rather than by a pixel.
pub fn close_id(section: &str) -> egui::Id {
    egui::Id::new(("close", section))
}

/// A heading with the means to put its section away. Hands back whether the
/// sculptor just did.
///
/// For the two sections that stand where a window used to: a window is closed
/// from its own title bar, and a section that could only be closed from the
/// rail would have taken that with the window. The mark is quiet in the way
/// every control here is — dim ink at rest, lifting under the pointer — and
/// at the trailing end, so the word still reads as the other headings do.
pub(super) fn closable_heading(ui: &mut egui::Ui, text: &str) -> bool {
    heading_rule(ui);
    let closed = ui
        .horizontal(|ui| {
            ui.label(heading_text(text));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(size::CONTROL, size::CONTROL),
                    egui::Sense::click(),
                );
                let tint = if response.hovered() {
                    ui.painter()
                        .rect_filled(rect, size::RADIUS, Tokens::raised());
                    Tokens::text()
                } else {
                    Tokens::text_dim()
                };
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "×",
                    egui::FontId::proportional(type_scale::BODY),
                    tint,
                );
                ui.ctx()
                    .memory_mut(|memory| memory.data.insert_temp(close_id(text), rect));
                response.clicked()
            })
            .inner
        })
        .inner;
    ui.add_space(space::TIGHT);
    closed
}

/// How wide the mark on an active row is.
///
/// Two pixels. The design's rule is that the accent marks at the scale of a
/// rail rather than filling anything, and this is what "a rail" is: enough to
/// find the active row from across a desk, not enough to be a coloured row.
pub(super) const SELECTION_RAIL: f32 = 2.0;

/// The mark an active row wears: a rail down its leading edge.
///
/// Drawn over a surface that is already raised and beside text that is already
/// primary, never instead of them. Cover the hue and the row is still the
/// lighter one with the brighter name — which is the accessibility rule the
/// design states, and the reason the rail is an addition rather than a
/// replacement for the tone step it stands on.
pub(super) fn selection_rail(ui: &egui::Ui, rect: egui::Rect) {
    let rail = egui::Rect::from_min_max(
        rect.min,
        egui::pos2(rect.min.x + SELECTION_RAIL, rect.max.y),
    );
    ui.painter().rect_filled(rail, 0.0, Tokens::selection());
}

/// A per-axis scale, as a person would say it.
///
/// One number where the three agree, which is what a uniformly scaled object
/// has and what "1.00×" has always meant. Three only where there is something
/// to tell apart — a squashed capsule reads "2.00 × 1.00 × 1.00" and an
/// unsquashed one does not have to be read three times to find that out.
pub(super) fn scale_text(scale: [f32; 3]) -> String {
    let [x, y, z] = scale;
    if (x - y).abs() < 1e-4 && (y - z).abs() < 1e-4 {
        format!("{x:.2}×")
    } else {
        format!("{x:.2} × {y:.2} × {z:.2}")
    }
}

/// The symbol lengths are shown in: `mm`, `cm`, `m`.
///
/// The one place the unit's own `label()` is called. It is a domain `label()`
/// like any other as far as the shell's ratchet can tell, and the ratchet is
/// right to count it — but an SI symbol is not an interface word and has no
/// translation to move into `Strings`. So there is exactly one call, and the
/// status bar and the transform readout both come here for it rather than
/// each reaching for the domain themselves.
pub(super) fn unit_symbol(units: Units) -> &'static str {
    units.display.label()
}

/// A numeric readout, set monospaced so digits do not reflow as they change.
pub(super) fn numeric(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(
        egui::RichText::new(text)
            .monospace()
            .size(type_scale::NUMERIC)
            .color(Tokens::text()),
    );
}

/// A toggle chip: a small button that reads as selected or not.
///
/// Handed back as a `Button` rather than added here, because one of the three
/// places that draw a row of these adds it disabled with the reason on it.
/// `unselected` is what an off chip fills with, which is the surface behind it
/// — the ground under the viewport bar, a panel everywhere else.
pub(super) fn chip(label: &str, on: bool, unselected: egui::Color32) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(label)
            .size(type_scale::LABEL)
            .color(if on {
                Tokens::text()
            } else {
                Tokens::text_dim()
            }),
    )
    .fill(if on { Tokens::raised() } else { unselected })
}

/// The same, filling with a colour of its own when engaged.
///
/// For state a sculptor has to see without looking for it. `chip` lifts an
/// engaged control to the raised surface, which is a 3.5% step and reads as
/// "hovered" as much as "on" — fine for a view preset and not for symmetry,
/// where a mirrored stroke nobody expected is the most expensive surprise on
/// the options bar.
pub(super) fn chip_tinted(
    label: &str,
    on: bool,
    unselected: egui::Color32,
    engaged: egui::Color32,
) -> egui::Button<'static> {
    egui::Button::new(
        egui::RichText::new(label)
            .size(type_scale::LABEL)
            .color(if on {
                Tokens::text()
            } else {
                Tokens::text_dim()
            }),
    )
    .fill(if on { engaged } else { unselected })
}

/// The id a chip carrying an icon is recorded under, so a test can find it.
///
/// For the same reason `slider_id` exists: a test that reaches a control by
/// pixel coordinate reaches a different control the next time a section lands
/// above it.
pub fn chip_id(name: &str) -> egui::Id {
    egui::Id::new(("chip", name))
}

/// A toggle chip with an icon before its word.
///
/// The word alone is what the mode chips were, and Mover, Girar and Escalar
/// are three words a sculptor has to read; an arrow, a ring and a box are
/// three shapes they already know from the manipulator itself. The icon is
/// drawn in the same set at the same weight as every other, and the chip
/// fills, dims and lifts exactly as `chip` does.
pub(super) fn icon_chip(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    on: bool,
    unselected: egui::Color32,
) -> egui::Response {
    icon_chip_recorded(ui, icon, label, on, unselected, true, true)
}

/// The same, choosing whether the chip claims `chip_id(label)` and whether
/// it is enabled.
///
/// Two rows can carry the same word — the object list's Girar and the layer
/// transform's — and one memory slot cannot hold both. A caller with ids of
/// its own passes `record: false` and leaves the slot to the row a test looks
/// for.
///
/// `enabled` is a parameter rather than `add_enabled_ui` around the call
/// because that scope is a child ui, and a wrapped row places a child at its
/// cursor without the wrap: the third mode chip in the object list ran off
/// the panel while the same three chips wrapped everywhere they were drawn
/// bare.
#[allow(clippy::too_many_arguments)]
pub(super) fn icon_chip_recorded(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    on: bool,
    unselected: egui::Color32,
    record: bool,
    enabled: bool,
) -> egui::Response {
    let enabled = enabled && ui.is_enabled();
    let padding = ui.spacing().button_padding;
    let font = egui::FontId::proportional(type_scale::LABEL);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, Tokens::text());
    let width = padding.x * 2.0 + size::CHIP_ICON + space::TIGHT + galley.size().x;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, size::CONTROL), egui::Sense::click());
    let lit = enabled && (on || response.hovered());
    let tint = if !enabled {
        Tokens::text_faint()
    } else if lit {
        Tokens::text()
    } else {
        Tokens::text_dim()
    };
    let fill = if enabled && (on || response.hovered()) {
        Tokens::raised()
    } else {
        unselected
    };
    let painter = ui.painter();
    painter.rect_filled(rect, size::RADIUS, fill);
    let icon_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(padding.x, (rect.height() - size::CHIP_ICON) * 0.5),
        egui::vec2(size::CHIP_ICON, size::CHIP_ICON),
    );
    icons::paint(painter, icon_rect, icon, tint);
    let text_at = egui::pos2(
        icon_rect.max.x + space::TIGHT,
        rect.center().y - galley.size().y * 0.5,
    );
    // The galley was laid out in one tone and is painted in another: a plain
    // `galley` keeps the colour it was laid out with and treats the tint as
    // a fallback, so only the override actually dims a quiet chip.
    painter.galley_with_override_text_color(text_at, galley, tint);
    if record {
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(chip_id(label), rect));
    }
    response
}

/// One bar as wide as the row, divided among a set of choices.
///
/// Four edge profiles as four chips wrapped in English and Spanish, and a
/// word standing alone on a second line reads as a second setting. A bar that
/// is *given* the row's width cannot wrap: each word takes what it measures
/// plus a tight pad, and whatever the row has left over is dealt out evenly,
/// so the four sit flush with the controls above and below them in every
/// locale. The chosen cell is lifted from a track the tone of the ground, as a
/// slider's knob is from its own; the rest are quiet until hovered. Each cell
/// is recorded under `chip_id` of its word, so a test can find it. Hands back
/// the choice that was clicked, when it was not already the current one.
pub(super) fn segmented<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    choices: &[T],
    name: impl Fn(T) -> &'static str,
    current: T,
) -> Option<T> {
    let font = egui::FontId::proportional(type_scale::LABEL);
    let galleys: Vec<_> = choices
        .iter()
        .map(|&choice| {
            ui.painter()
                .layout_no_wrap(name(choice).to_owned(), font.clone(), Tokens::text())
        })
        .collect();
    let fitted: f32 = galleys
        .iter()
        .map(|galley| galley.size().x + 2.0 * space::TIGHT)
        .sum();
    // A panel narrowed below what the words need gets a row that overruns it
    // rather than words squeezed into cells they do not fit, because a
    // clipped word is at least still a word.
    let width = ui.available_width().max(fitted);
    let slack = (width - fitted) / choices.len() as f32;
    let (track, _) = ui.allocate_exact_size(egui::vec2(width, size::CONTROL), egui::Sense::hover());
    ui.painter()
        .rect_filled(track, size::RADIUS, Tokens::ground());

    let mut left = track.min.x;
    let mut clicked = None;
    for (&choice, galley) in choices.iter().zip(galleys) {
        let cell = egui::Rect::from_min_size(
            egui::pos2(left, track.min.y),
            egui::vec2(galley.size().x + 2.0 * space::TIGHT + slack, track.height()),
        );
        left = cell.max.x;
        let word = name(choice);
        let response = ui.interact(cell, ui.id().with(word), egui::Sense::click());
        let on = choice == current;
        let lit = on || response.hovered();
        if lit {
            ui.painter()
                .rect_filled(cell.shrink(space::HAIR), size::RADIUS, Tokens::raised());
        }
        let tint = if lit {
            Tokens::text()
        } else {
            Tokens::text_dim()
        };
        ui.painter().galley_with_override_text_color(
            cell.center() - galley.size() * 0.5,
            galley,
            tint,
        );
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(chip_id(word), cell));
        if response.clicked() && !on {
            clicked = Some(choice);
        }
    }
    clicked
}

/// The icon a manipulator mode wears, the same shape the viewport draws for it.
pub(super) fn gizmo_mode_icon(mode: GizmoMode) -> Icon {
    match mode {
        GizmoMode::Move => Icon::Move,
        GizmoMode::Rotate => Icon::Turn,
        GizmoMode::Scale => Icon::Scale,
    }
}

/// A label and its chord, for a rail tooltip.
pub(super) fn labelled_chord(state: &ShellState<'_>, label: &str, action: Action) -> String {
    let chord = chord_text(state, action);
    if chord.is_empty() {
        label.to_owned()
    } else {
        format!("{label}  ·  {chord}")
    }
}

/// The keyboard action that puts the manipulator into a mode.
///
/// W, E and R — Maya's keys and Unity's. Asked of the table rather than
/// spelled into a tooltip, so a remapped binding is the one the interface
/// names.
pub(super) fn gizmo_mode_action(mode: GizmoMode) -> Action {
    match mode {
        GizmoMode::Move => Action::TransformMove,
        GizmoMode::Rotate => Action::TransformTurn,
        GizmoMode::Scale => Action::TransformScale,
    }
}

/// The manipulator's three modes as one row of chips.
///
/// One row wherever the manipulator can be worked — the cage section, the
/// object list, the shapes panel — so the sculptor meets the same three chips
/// in the same order everywhere. `can_transform` is the cage's rule: turning
/// and scaling act about the middle of the selection, and one point's middle
/// is itself, so on a selection of one they are disabled with the reason on
/// them rather than drawn live and inert.
pub(super) fn gizmo_mode_row(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    current: GizmoMode,
    can_transform: bool,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    ui.horizontal_wrapped(|ui| {
        for mode in GizmoMode::ALL {
            let on = current == mode;
            let usable = can_transform || mode == GizmoMode::Move;
            let response = icon_chip_recorded(
                ui,
                gizmo_mode_icon(mode),
                s.gizmo_mode_name(mode),
                on,
                Tokens::panel(),
                true,
                usable,
            );
            if !usable {
                response.on_hover_text(s.hint_gizmo_needs_two);
                continue;
            }
            if response.clicked() && !on {
                queue.push(Command::SetGizmoMode(mode));
            }
        }
    });
}

/// The id a readout's row is recorded under, so a test can ask whether a
/// value was drawn at all — which is what a folded section has to answer.
pub fn readout_id(label: &str) -> egui::Id {
    egui::Id::new(("readout", label))
}

/// A label and its value on one row.
pub(super) fn readout(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    let row = ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            numeric(ui, value);
        });
    });
    ui.ctx().memory_mut(|memory| {
        memory
            .data
            .insert_temp(readout_id(label), row.response.rect)
    });
}

/// How tall a slider's track is drawn.
///
/// Thicker than the hairline it replaced. The track is the only part of the
/// control visible from across a desk, and a sculptor adjusting Intensidade
/// mid-stroke is not reading the digits beside it.
pub(super) const SLIDER_TRACK: f32 = 6.0;

/// The most arrow-key presses it should take to cross a slider's whole range.
///
/// The floor on how *coarse* a press is. A press also never moves less than
/// one displayed unit — a step finer than the readout is a press that changes
/// the number by nothing anyone can see, which is what a fiftieth of the
/// one-to-sixteen mask range was: 5.3, rendered as 5.
pub(super) const SLIDER_KEY_STEPS: f32 = 100.0;

/// The knob's radius at rest, and under the pointer.
///
/// The rest of the interface says "quiet until addressed" by dimming; a knob
/// is too small for a tone change alone to register, so it grows as well.
pub(super) const SLIDER_KNOB: f32 = 5.0;

pub(super) const SLIDER_KNOB_HOT: f32 = 7.0;

/// The one slider the shell draws: a track, the range travelled, and a knob.
///
/// The fill is the control's *state* rather than ornament — it says how far
/// into its range the value sits, which is the one thing the digits above it
/// cannot say without being read — so it spans the start of the track to the
/// knob and stops, and a value at the bottom of its range draws none at all.
///
/// Drawn here rather than configured on `egui::Slider` because the parts that
/// matter are the ones egui does not expose: which side of the knob is filled,
/// what it is filled with, and how the knob answers the pointer. Written once
/// so the treatment lives in one place instead of in the thirty-odd call sites
/// that reach `slider_named`.
pub(super) fn sculpt_slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    decimals: usize,
) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), size::CONTROL),
        egui::Sense::click_and_drag(),
    );

    // Inset by the *hot* radius at both ends, always. Insetting by the current
    // radius would slide the whole track sideways the moment the pointer
    // arrived and the knob grew, which reads as the value having changed.
    let left = rect.left() + SLIDER_KNOB_HOT;
    let right = rect.right() - SLIDER_KNOB_HOT;
    let span = (right - left).max(1.0);
    let (low, high) = (*range.start(), *range.end());
    let extent = high - low;

    if response.dragged() || response.clicked() {
        if let Some(at) = response.interact_pointer_pos() {
            let t = ((at.x - left) / span).clamp(0.0, 1.0);
            let next = low + t * extent;
            if next != *value {
                *value = next;
                response.mark_changed();
            }
        }
    }

    // The arrow keys, which `egui::Slider` handled and a hand-drawn track
    // would otherwise have quietly taken away. `Sense::click_and_drag` is
    // focusable, so the control still takes focus from the keyboard — it just
    // did nothing once it had it, which is worse than not being reachable.
    //
    // How far a press moves is decided below, from the slider's own precision.
    if response.has_focus() {
        // Without this the arrows move focus to the next control instead of
        // moving the value, which is egui's default for a focused widget and
        // is why `egui::Slider` sets the same filter. Horizontal only: the
        // slider runs sideways, so up and down should still leave it.
        ui.ctx().memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    ..Default::default()
                },
            );
        });
        let presses = ui.input(|input| {
            input.num_presses(egui::Key::ArrowRight) as i32
                - input.num_presses(egui::Key::ArrowLeft) as i32
        });
        if presses != 0 {
            // One displayed unit, or a hundredth of the range where that is
            // coarser. The first keeps a press visible in the readout; the
            // second keeps a slider set to three decimals from needing a
            // thousand presses to cross.
            let unit = 0.1_f32.powi(decimals as i32);
            let step = unit.max(extent / SLIDER_KEY_STEPS);
            let next = (*value + presses as f32 * step).clamp(low, high);
            if next != *value {
                *value = next;
                response.mark_changed();
            }
        }
    }

    let t = if extent.abs() > f32::EPSILON {
        ((*value - low) / extent).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let travelled = left + t * span;
    let middle = rect.center().y;
    let track = egui::Rect::from_min_max(
        egui::pos2(left, middle - SLIDER_TRACK * 0.5),
        egui::pos2(right, middle + SLIDER_TRACK * 0.5),
    );
    let radius = egui::epaint::CornerRadius::same((SLIDER_TRACK * 0.5) as u8);
    let painter = ui.painter();
    painter.rect_filled(track, radius, Tokens::control_track());
    // Guarded rather than left to a zero-width rectangle: a rounded rect of no
    // width still paints its corners, which is a dot of accent on a control
    // whose value is nothing.
    if travelled > left + 0.5 {
        painter.rect_filled(
            egui::Rect::from_min_max(track.min, egui::pos2(travelled, track.max.y)),
            radius,
            Tokens::control_fill(),
        );
    }

    let hot = response.hovered() || response.dragged();
    let knob = if hot { SLIDER_KNOB_HOT } else { SLIDER_KNOB };
    let ink = if hot {
        Tokens::text()
    } else {
        Tokens::text_dim()
    };
    painter.circle_filled(egui::pos2(travelled, middle), knob, ink);

    response
}

/// The id egui gave a named slider's widget, so a test can put the keyboard
/// on it.
///
/// Not `slider_id`, which is the key its *rectangle* is filed under. Focus is
/// addressed by the widget's own id, and egui derives that from the layout —
/// so it has to be handed out rather than guessed. The arrows only act on a
/// focused slider, as they do on `egui::Slider`, and focus arrives by Tab: a
/// click does not take it. A test that clicked and then pressed an arrow was
/// measuring the click.
pub fn slider_widget_id(name: &str) -> egui::Id {
    egui::Id::new(("slider-widget", name))
}

/// The id a named slider carries, so a test can find where it went.
///
/// Panels grow, and a test that reaches a control by pixel coordinate reaches
/// a different control the next time a section is added above it — which is
/// exactly what happened when the cage section landed above the mask's.
pub fn slider_id(name: &str) -> egui::Id {
    egui::Id::new(("slider", name))
}

/// A slider with its value shown monospaced beside it.
pub(super) fn slider(
    ui: &mut egui::Ui,
    label: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    decimals: usize,
) -> Option<f32> {
    slider_named(ui, label, label, value, range, decimals)
}

/// The same, recorded under a name that is not the label it shows.
///
/// The reference panel draws the same four controls three times, once a plane.
/// They cannot share an id — egui would have them fight — and they should not
/// share a *name* either, or a test asking where the opacity slider is would be
/// asking which of three.
pub(super) fn slider_named(
    ui: &mut egui::Ui,
    name: &str,
    label: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    decimals: usize,
) -> Option<f32> {
    slider_reading(
        ui,
        name,
        label,
        &format!("{value:.decimals$}"),
        value,
        range,
        decimals,
    )
}

/// The same, showing a reading of its own rather than the raw number.
///
/// For a value the sculptor thinks of in another unit: brush size is edited in
/// engine units and read in millimetres, and spelling both — a label carrying
/// the millimetres and a readout carrying the fraction — was the same fact
/// twice, in the widest control on a bar that had run off the window.
pub(super) fn slider_reading(
    ui: &mut egui::Ui,
    name: &str,
    label: &str,
    reading: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    decimals: usize,
) -> Option<f32> {
    let mut edited = value;
    let mut changed = None;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            numeric(ui, reading.to_owned());
        });
    });
    // Identified by its name, which for most sliders is the label — what a
    // person would point at. Two sliders sharing a name in one panel would
    // share an id, and egui would say so rather than let them fight silently.
    let response = ui
        .push_id(slider_id(name), |ui| {
            sculpt_slider(ui, &mut edited, range, decimals)
        })
        .inner;
    if response.changed() {
        changed = Some(edited);
    }
    // Recorded under a name of our own, because egui derives the widget's id
    // from the layout and a test cannot guess it. `push_id` scopes it so two
    // sliders sharing a label in different sections stay apart.
    ui.ctx().memory_mut(|memory| {
        memory.data.insert_temp(slider_id(name), response.rect);
        memory.data.insert_temp(slider_widget_id(name), response.id);
    });
    changed
}

// -- the regions -------------------------------------------------------------

/// How a menu item spells the shortcut that does the same thing.
///
/// Empty where nothing is bound, which is what `Button::shortcut_text` wants
/// for "this item has no shortcut" — so an unbound action simply reads as a
/// plain item rather than as a gap.
pub(super) fn chord_text(state: &ShellState<'_>, action: Action) -> String {
    state
        .shortcuts
        .chord(action)
        .map(|chord| chord.label())
        .unwrap_or_default()
}

/// Says on hover which key does what this control does, where one is bound.
///
/// The menus already spell their chords; the chips a sculptor actually clicks
/// did not, so the keys for the views and for symmetry were learnt from the
/// README or not at all. Nothing is shown for an unbound action rather than
/// an empty tooltip.
pub(super) fn with_chord(
    response: egui::Response,
    state: &ShellState<'_>,
    action: Action,
) -> egui::Response {
    let chord = chord_text(state, action);
    if chord.is_empty() {
        response
    } else {
        response.on_hover_text(chord)
    }
}

/// A menu item, labelled with the chord bound to the same action.
pub(super) fn item(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    label: &str,
    action: Action,
) -> egui::Response {
    ui.add(egui::Button::new(label).shortcut_text(chord_text(state, action)))
}

/// The menu entries that open a panel, in the order given.
///
/// One helper rather than a copy of the same four lines per panel: none of them
/// carries a chord to label, none of them is ever disabled, and what the File
/// menu has to say about them is the order they stand in — which is a decision,
/// and is the only thing left at the call site.
pub(super) fn panel_items(
    ui: &mut egui::Ui,
    queue: &mut CommandQueue,
    entries: &[(&str, Command)],
) {
    for (label, command) in entries {
        if ui.button(*label).clicked() {
            queue.push(command.clone());
            ui.close_menu();
        }
    }
}

/// The same, greyed out when the action cannot be taken.
pub(super) fn item_enabled(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    enabled: bool,
    label: &str,
    action: Action,
) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(label).shortcut_text(chord_text(state, action)),
    )
}

/// Linear to an sRGB byte, and back. The engine stores linear and egui edits
/// sRGB, so the two are converted at the one place they meet.
pub(super) fn to_srgb_byte(linear: f32) -> u8 {
    let linear = linear.clamp(0.0, 1.0);
    let encoded = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

pub(super) fn from_srgb_byte(byte: u8) -> f32 {
    let encoded = f32::from(byte) / 255.0;
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

/// The rename field, in the row where the name was.
///
/// In place rather than in a dialog: renaming a layer is one word, and a modal
/// for one word stops the sculptor to ask for it.
///
/// Enter commits and Escape abandons — and so does clicking away, which is the
/// same path: egui surrenders focus for both, and the two are told apart by
/// whether Enter was the key that did it. A field left open after the pointer
/// leaves it would swallow the next shortcut typed.
pub(super) fn rename_field(ui: &mut egui::Ui, draft: &str, queue: &mut CommandQueue) {
    let mut text = draft.to_string();
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Body),
    );
    // Focused on the frame it appears, and not re-grabbed on the frame it is
    // surrendered — that frame is the click-away, and taking focus back would
    // make the field impossible to leave.
    if !response.has_focus() && !response.lost_focus() {
        response.request_focus();
    }
    if text != draft {
        queue.push(Command::EditLayerName(text));
    }
    if response.lost_focus() {
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            queue.push(Command::CommitRenameLayer);
        } else {
            queue.push(Command::CancelRenameLayer);
        }
    }
}

/// A byte count in the largest unit that keeps it readable.
pub(super) fn bytes_label(bytes: usize) -> String {
    const MB: usize = 1024 * 1024;
    const KB: usize = 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f32 / MB as f32)
    } else if bytes >= KB {
        format!("{:.1} kB", bytes as f32 / KB as f32)
    } else {
        format!("{bytes} B")
    }
}

/// Paints the material itself on a ball.
///
/// The MatCap *is* a picture of a lit sphere, so the preview is that picture:
/// the same recipe the viewport shades with, cut out of its square and cached
/// as a texture the first time each material is shown. Terracotta reads warm,
/// Polido reads shiny, and switching materials is seen before it is read.
pub(super) fn paint_matcap(ui: &egui::Ui, rect: egui::Rect, matcap: MatCap) {
    const SIDE: u32 = 96;
    let ctx = ui.ctx();
    let key = egui::Id::new(("matcap-swatch", format!("{matcap:?}")));
    let texture = ctx
        .data(|data| data.get_temp::<egui::TextureHandle>(key))
        .unwrap_or_else(|| {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [SIDE as usize, SIDE as usize],
                &matcap.swatch(SIDE),
            );
            let handle = ctx.load_texture(
                format!("matcap swatch {matcap:?}"),
                image,
                egui::TextureOptions::LINEAR,
            );
            ctx.data_mut(|data| data.insert_temp(key, handle.clone()));
            handle
        });
    // The same diameter `paint_sphere` gives the brush swatches, so the two
    // kinds of ball on screen are one size.
    let side = rect.width().min(rect.height()) * 0.84;
    let square = egui::Rect::from_center_size(rect.center(), egui::vec2(side, side));
    ui.painter().image(
        texture.id(),
        square,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Tokens::untinted(),
    );
}

/// Paints a shaded sphere: the one place the design spends skeuomorphism.
pub(super) fn paint_sphere(ui: &egui::Ui, rect: egui::Rect, tint: egui::Color32, active: bool) {
    let painter = ui.painter();
    let centre = rect.center();
    let radius = rect.width().min(rect.height()) * 0.42;

    // A few concentric passes lighten toward the upper left, which is where
    // the material previews are lit from.
    const STEPS: usize = 7;
    for step in (0..STEPS).rev() {
        let t = step as f32 / STEPS as f32;
        let offset = egui::vec2(-radius * 0.22 * t, -radius * 0.22 * t);
        let shade = 0.55 + 0.45 * (1.0 - t);
        // Darkening toward the rim, not choosing a colour: the tint arrives
        // derived from a token.
        let color = egui::Color32::from_rgb(
            (tint.r() as f32 * shade) as u8,
            (tint.g() as f32 * shade) as u8,
            (tint.b() as f32 * shade) as u8,
        );
        painter.circle_filled(centre + offset, radius * (1.0 - 0.12 * t), color);
    }

    if active {
        painter.circle_stroke(
            centre,
            radius + 3.0,
            egui::Stroke::new(1.5_f32, Tokens::accent()),
        );
    }
}

/// Groups digits so a large count is readable at a glance.
pub(super) fn thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut grouped = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(ch);
    }
    grouped
}

/// Bytes as gigabytes, to two places.
pub(super) fn gigabytes(bytes: u64) -> String {
    format!("{:.2} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}
