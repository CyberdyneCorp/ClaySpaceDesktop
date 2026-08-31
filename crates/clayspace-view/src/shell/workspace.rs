//! The representation bar: what the active layer is, and what it could become.
//!
//! Above the viewport rather than inside an inspector, because it answers the
//! question a sculptor asks most often about a ClaySpace document and could
//! only be answered by a tiny `SDF` tag on a layer row and a line of text at
//! the far end of the viewport bar.
//!
//! # The cards do not convert
//!
//! Three cards, one a representation, with the active one lit. They are a
//! statement and not a control: crossing between representations costs
//! something — a field rasterised into cells is a different object with
//! different verbs, and the crossing is not free and not always reversible —
//! so it stays behind the conversion panel, where the cost is shown and
//! confirmed. A card that silently rewrote the layer under the pointer would
//! be the worst control in the application.
//!
//! The action lives in the row beside them, and offers exactly the crossings
//! the domain says exist: `Direction::from_representation` gives two per
//! representation, so the buttons are derived rather than listed. Clicking one
//! aims the conversion panel and opens it; it does not convert.

use super::*;

/// How tall a card is, and how big its icon is drawn.
pub(super) const CARD_HEIGHT: f32 = 40.0;
pub(super) const CARD_ICON: f32 = 22.0;

/// The id a representation card is recorded under, so a test can ask where it
/// went rather than measuring it off a capture.
pub fn representation_card_id(representation: Representation) -> egui::Id {
    egui::Id::new(("representation-card", representation))
}

/// The id a crossing's button is recorded under, keyed by what it crosses to.
pub fn convert_to_id(target: Representation) -> egui::Id {
    egui::Id::new(("convert-to", target))
}

/// The icon a representation wears. Shape, never hue: the three are equals,
/// and a discriminator that is only a colour is one a colour-blind sculptor
/// cannot read.
pub(super) fn representation_icon(representation: Representation) -> Icon {
    match representation {
        Representation::Sdf => Icon::FieldRepresentation,
        Representation::Voxel => Icon::VoxelRepresentation,
        Representation::Mesh => Icon::MeshRepresentation,
    }
}

/// How wide a card has to be to hold what it shows.
///
/// Measured from the text rather than fixed, because the three names and the
/// three phrases differ in every locale — a width that fits "Signed Distance
/// Field" leaves "Malla" floating in a card twice the size of its word, and
/// one chosen for Portuguese clips the Spanish.
fn card_width(
    ui: &egui::Ui,
    state: &ShellState<'_>,
    representation: Representation,
    compact: bool,
) -> f32 {
    let s = state.strings;
    let measure = |text: &str, size: f32| {
        ui.painter()
            .layout_no_wrap(
                text.to_owned(),
                egui::FontId::proportional(size),
                Tokens::text(),
            )
            .size()
            .x
    };
    let name = measure(s.representation_name(representation), type_scale::LABEL);
    let text = if compact {
        name
    } else {
        name.max(measure(
            s.representation_sentence(representation),
            type_scale::HEADING,
        ))
    };
    space::SNUG + CARD_ICON + space::SNUG + text + space::SNUG
}

/// How much of itself the bar can afford to show.
///
/// A ladder rather than a switch, because the parts are not equally worth
/// keeping. The crossings are what a sculptor cannot do without — a Converter
/// button that has run off the end of the bar is a feature that is gone — so
/// they are never what gives way. The phrases go first: they explain a
/// vocabulary once and then say the same thing forever, and they survive in
/// the tooltip. The heading goes next: three cards under it already read as
/// one group, and the word is the least load-bearing thing in the row.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Density {
    /// Heading, and cards carrying their phrase.
    Full,
    /// Heading, and cards carrying their name alone.
    Compact,
    /// Cards alone.
    Tight,
}

impl Density {
    pub(super) fn compact(self) -> bool {
        self != Self::Full
    }

    pub(super) fn shows_heading(self) -> bool {
        self != Self::Tight
    }
}

/// The most the bar can show in the width it has been given.
fn density(ui: &egui::Ui, state: &ShellState<'_>) -> Density {
    // egui puts its own `item_spacing` between every pair of widgets, on top
    // of the gaps this bar adds itself. Leaving it out of the arithmetic is
    // how the first version of this overran the inspector's edge by eight
    // pixels while believing it had fitted.
    let gap = ui.spacing().item_spacing.x;
    let cards = |compact: bool| -> f32 {
        Representation::ALL
            .iter()
            .map(|representation| {
                card_width(ui, state, *representation, compact) + space::SNUG + gap
            })
            .sum()
    };
    let heading = ui
        .painter()
        .layout_no_wrap(
            state.strings.section_representation.to_owned(),
            egui::FontId::proportional(type_scale::HEADING),
            Tokens::text_faint(),
        )
        .size()
        .x
        + space::ROOMY
        + gap;
    // What is left once the crossings and the bar's own padding are reserved.
    let room = ui.available_width() - conversion_width(ui, state) - space::ROOMY - space::PANEL;
    if cards(false) + heading <= room {
        Density::Full
    } else if cards(true) + heading <= room {
        Density::Compact
    } else {
        Density::Tight
    }
}

/// What the conversion row needs, so the cards can be asked to give way to it.
///
/// The crossings are the part that must not be lost: a phrase moved to a
/// tooltip is a small thing beside a Converter button that ran off the end of
/// the bar, which is what the first version of this did.
fn conversion_width(ui: &egui::Ui, state: &ShellState<'_>) -> f32 {
    let s = state.strings;
    let measure = |text: &str, size: f32| {
        ui.painter()
            .layout_no_wrap(
                text.to_owned(),
                egui::FontId::proportional(size),
                Tokens::text(),
            )
            .size()
            .x
    };
    let padding = ui.spacing().button_padding.x * 2.0;
    let gap = ui.spacing().item_spacing.x;
    let label = measure(s.label_convert_to, type_scale::LABEL) + space::SNUG + gap;
    Direction::from_representation(state.representation)
        .into_iter()
        .map(|direction| {
            padding
                + size::CHIP_ICON
                + space::TIGHT
                + measure(s.representation_name(direction.to()), type_scale::LABEL)
                + space::TIGHT
                + gap
        })
        .sum::<f32>()
        + label
}

/// The bar, drawn above the viewport.
pub fn representation_bar(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.add_space(space::SNUG);
    // Scrolls sideways rather than cutting the last crossing off, as the
    // options bar does and for the same reason.
    egui::ScrollArea::horizontal()
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .id_salt("representation-bar")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(space::PANEL);
                let density = density(ui, state);
                if density.shows_heading() {
                    ui.label(
                        egui::RichText::new(s.section_representation)
                            .size(type_scale::HEADING)
                            .color(Tokens::text_faint()),
                    );
                    ui.add_space(space::ROOMY);
                }
                for representation in Representation::ALL {
                    representation_card(ui, state, representation, density.compact());
                    ui.add_space(space::SNUG);
                }
                ui.add_space(space::ROOMY);
                conversion_row(ui, state, queue);
            });
        });
    ui.add_space(space::SNUG);
}

/// One card: an icon, the representation's name, and what it is in a phrase.
///
/// The active one is raised and railed, which is the same grammar the active
/// layer row wears — and it carries its name in primary text, so the state
/// survives the hue being removed.
fn representation_card(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    representation: Representation,
    compact: bool,
) {
    let s = state.strings;
    let active = state.representation == representation;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(card_width(ui, state, representation, compact), CARD_HEIGHT),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        size::RADIUS,
        if active {
            Tokens::raised()
        } else {
            Tokens::panel()
        },
    );
    if active {
        selection_rail(ui, rect);
    }

    let ink = if active {
        Tokens::text()
    } else {
        Tokens::text_dim()
    };
    let icon_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(space::SNUG, (rect.height() - CARD_ICON) * 0.5),
        egui::Vec2::splat(CARD_ICON),
    );
    icons::paint(
        ui.painter(),
        icon_rect,
        representation_icon(representation),
        ink,
    );

    let left = icon_rect.max.x + space::SNUG;
    let painter = ui.painter();
    if compact {
        painter.text(
            egui::pos2(left, rect.center().y),
            egui::Align2::LEFT_CENTER,
            s.representation_name(representation),
            egui::FontId::proportional(type_scale::LABEL),
            ink,
        );
    } else {
        painter.text(
            egui::pos2(left, rect.center().y - space::HAIR),
            egui::Align2::LEFT_BOTTOM,
            s.representation_name(representation),
            egui::FontId::proportional(type_scale::LABEL),
            ink,
        );
        painter.text(
            egui::pos2(left, rect.center().y + space::HAIR),
            egui::Align2::LEFT_TOP,
            s.representation_sentence(representation),
            egui::FontId::proportional(type_scale::HEADING),
            Tokens::text_faint(),
        );
    }

    ui.ctx().memory_mut(|memory| {
        memory
            .data
            .insert_temp(representation_card_id(representation), rect)
    });
    // The phrase follows the card into the tooltip when it will not fit
    // beside the name, so a narrow window loses the layout and not the word.
    let state_hint = if active {
        s.hint_representation_active
    } else {
        s.hint_representation_other
    };
    response.on_hover_text(if compact {
        format!(
            "{}\n{state_hint}",
            s.representation_sentence(representation)
        )
    } else {
        state_hint.to_owned()
    });
}

/// The crossings the active representation actually has, as buttons.
///
/// Derived from `Direction::from_representation` rather than listed, so a
/// crossing the domain gains appears here and one it loses stops being
/// offered. Each aims the conversion panel and opens it — the panel is where
/// the cost is stated and the crossing is confirmed, which is the semantic
/// this bar is careful not to route around.
fn conversion_row(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    let crossings = Direction::from_representation(state.representation);
    if crossings.is_empty() {
        ui.label(
            egui::RichText::new(s.convert_none_here)
                .size(type_scale::LABEL)
                .color(Tokens::text_faint()),
        );
        return;
    }

    ui.label(
        egui::RichText::new(s.label_convert_to)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    for direction in crossings {
        let target = direction.to();
        let response = icon_chip_recorded(
            ui,
            representation_icon(target),
            s.representation_name(target),
            false,
            Tokens::panel(),
            false,
            true,
        );
        ui.ctx().memory_mut(|memory| {
            memory
                .data
                .insert_temp(convert_to_id(target), response.rect)
        });
        if response.on_hover_text(s.action_convert).clicked() {
            let mut settings = state.conversion;
            settings.direction = direction;
            queue.push(Command::SetConversion(settings));
            // Aimed, then shown. `ToggleConvert` is a toggle, so opening an
            // already-open panel would close it — and a sculptor who clicked
            // Malha would watch the panel they were aiming disappear.
            if !state.show_convert {
                queue.push(Command::ToggleConvert);
            }
        }
        ui.add_space(space::TIGHT);
    }
}
