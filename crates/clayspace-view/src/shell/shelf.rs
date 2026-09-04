//! The brush shelf and the tool rail — the two ways to a tool.
//!
//! The shelf holds the brushes the active representation actually has, which
//! is `ToolKind::for_representation` and not a filter over a fixed list. The
//! rail holds the panels and the actions: mask painting, the references, the
//! shapes and boolean sections, the cage, the curve, the deformations, undo
//! and redo — every one of which the menus also reach, because a panel three
//! menus deep is a panel a new sculptor never opens.

use super::*;

/// The brush shelf: every tool, with the active one accented.
/// The brush shelf, holding the verbs the active representation has.
///
/// Not every tool with the ones that do not apply greyed out. Three
/// representations carry substantially different vocabularies, so a single
/// list would be mostly disabled entries whatever the active layer, every one
/// of them saying the same thing — absence carries that better than a greyed
/// row does. A tool that *has* a verb here and cannot be used right now is
/// still shown; that is a different sentence and worth the space.
/// Where a brush's swatch was drawn on the shelf, for tests.
///
/// The swatch's *size* is the thing worth pinning, and it cannot be read off
/// the source usefully — a token name in an `allocate_exact_size` call proves
/// nothing about what reached the screen. So the rect goes into memory and a
/// test asks the shelf how big it drew its brushes.
pub fn brush_swatch_id(tool: ToolKind) -> egui::Id {
    egui::Id::new(("brush-swatch", tool))
}

/// Which set of brushes the shelf is showing.
///
/// `None` is the sculpt workflow: the brushes the active layer can actually
/// use, which is what the shelf showed before it could show anything else and
/// what it goes back to. `Some(r)` is browsing — the vocabulary another
/// representation has, so a sculptor can find out what crossing to it would
/// give them without crossing first.
///
/// Interface state, not the document's: it lives in egui's own memory beside
/// the section folds, enters no history, emits no command, and is forgotten
/// when the application closes. A shelf that reopened filtered to Malha on an
/// SDF document would be a document that had changed while nobody was looking.
pub(super) fn shelf_filter(ctx: &egui::Context) -> ShelfFilter {
    ctx.data(|data| data.get_temp(shelf_filter_id()))
        .unwrap_or_default()
}

/// Which set of brushes the shelf lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ShelfFilter {
    /// The brushes the active layer can actually use. The default, and the
    /// sculpt workflow.
    #[default]
    Available,
    /// Another representation's vocabulary, for finding out what crossing to it
    /// would give.
    Elsewhere(Representation),
    /// The ones a sculptor starred, across every representation.
    Favourites,
}

impl ShelfFilter {
    /// Every filter the column offers, in the order it draws them.
    pub fn all() -> Vec<Self> {
        let mut all = vec![Self::Available];
        all.extend(Representation::ALL.map(Self::Elsewhere));
        all.push(Self::Favourites);
        all
    }
}

/// Where a request to star a brush, or take its star off, is left.
///
/// The route the section folds and the shelf's filter take: a favourite is the
/// sculptor's own shortlist, it reaches no document, and `ToolKind` is a domain
/// type but *starring* is not a domain verb — there is no engine call and no
/// edit to record.
pub fn favourite_toggle_id() -> egui::Id {
    egui::Id::new("favourite-toggle")
}

/// Where the shelf's filter is kept, and where a test can set it.
pub fn shelf_filter_id() -> egui::Id {
    egui::Id::new("shelf-filter")
}

/// The id one filter entry is recorded under.
pub fn shelf_filter_chip_id(filter: ShelfFilter) -> egui::Id {
    egui::Id::new(("shelf-filter-chip", filter))
}

/// How wide the filter column runs.
const FILTER_WIDTH: f32 = 74.0;

/// The tallest a filter row is allowed to be.
///
/// The height that reads well beside a swatch. It is a ceiling rather than the
/// height itself — see [`filter_row`].
const FILTER_ROW_MAX: f32 = 15.0;

/// How tall one filter row is: the shelf's own height, shared out.
///
/// Derived rather than fixed, and this is a repair. The column holds one row
/// per representation plus "available" and "favourites", so its length is
/// `Representation::ALL.len() + 2` — it was five and a constant chosen for five
/// was written down beside a comment saying so. The fourth representation makes
/// it six, and six rows of fifteen pixels is ninety inside a region that is
/// eighty-four: the column would have overrun the shelf with nothing erroring
/// and no test failing, because every assertion here is about *which* rows are
/// drawn rather than whether they fit.
///
/// Capped at [`FILTER_ROW_MAX`] so that a shorter list keeps the proportions it
/// had rather than growing into the slack.
fn filter_row() -> f32 {
    (region::SHELF / ShelfFilter::all().len() as f32).min(FILTER_ROW_MAX)
}

/// The filter column at the shelf's leading edge.
///
/// A column rather than a row because the shelf is one swatch tall and a row
/// of filters above them would take a swatch's worth of height from a region
/// that has none to give.
fn shelf_filters(ui: &mut egui::Ui, state: &ShellState<'_>) -> ShelfFilter {
    let s = state.strings;
    let current = shelf_filter(ui.ctx());
    let mut chosen = current;
    let row = filter_row();
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for filter in ShelfFilter::all() {
            let label = match filter {
                ShelfFilter::Available => s.shelf_filter_all,
                ShelfFilter::Elsewhere(representation) => s.representation_name(representation),
                ShelfFilter::Favourites => s.shelf_filter_favourites,
            };
            let on = filter == current;
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(FILTER_WIDTH, row), egui::Sense::click());
            let lit = on || response.hovered();
            if lit {
                ui.painter()
                    .rect_filled(rect, size::RADIUS, Tokens::raised());
            }
            // The chosen row is marked as the active layer is: a rail at the
            // leading edge over a raised surface, so the two read as one
            // grammar and neither rests on hue alone.
            if on {
                selection_rail(ui, rect);
            }
            ui.painter().text(
                egui::pos2(rect.left() + space::SNUG, rect.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(type_scale::LABEL),
                if lit {
                    Tokens::text()
                } else {
                    Tokens::text_dim()
                },
            );
            ui.ctx()
                .memory_mut(|memory| memory.data.insert_temp(shelf_filter_chip_id(filter), rect));
            if response.clicked() {
                chosen = filter;
            }
        }
    });
    if chosen != current {
        ui.ctx()
            .data_mut(|data| data.insert_temp(shelf_filter_id(), chosen));
    }
    chosen
}

/// The brush shelf: the tools the active representation has, and a way to see
/// what the other two have without crossing to them.
///
/// The default is unchanged and deliberately so — the shelf shows what the
/// active layer can be sculpted with, derived from the one declared table
/// rather than a rule written per tool. The filter is a browsing aid on top of
/// that: choosing another representation lists its vocabulary, and every brush
/// in it that the active layer has no verb for is drawn dim and refuses to be
/// picked, with the reason on it. A shelf that let a sculptor select a brush
/// their layer cannot run would be offering a click that does nothing.
pub fn brush_shelf(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    ui.horizontal(|ui| {
        ui.add_space(space::PANEL);
        let filter = shelf_filters(ui, state);
        ui.add_space(space::ROOMY);

        let tools = match filter {
            ShelfFilter::Available => ToolKind::for_representation(state.representation),
            ShelfFilter::Elsewhere(representation) => ToolKind::for_representation(representation),
            // Across every representation, because a favourite is a sculptor's
            // own shortlist rather than a property of the active layer — and
            // the ones their layer cannot run are drawn dim like any other
            // brush met while browsing.
            ShelfFilter::Favourites => ToolKind::ALL
                .into_iter()
                .filter(|tool| state.favourites.contains(tool))
                .collect(),
        };
        if tools.is_empty() {
            // Two different silences: a representation with no verbs yet, and a
            // shortlist nobody has added to. The second one says how to.
            let empty = if filter == ShelfFilter::Favourites {
                state.strings.shelf_no_favourites
            } else {
                state.strings.shelf_no_tools
            };
            ui.label(
                egui::RichText::new(empty)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            return;
        }
        for tool in tools {
            brush_swatch(ui, state, tool, queue);
            ui.add_space(space::SNUG);
        }
    });
}

/// One brush on the shelf.
fn brush_swatch(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    tool: ToolKind,
    queue: &mut CommandQueue,
) {
    let active = state.tool == tool;
    // Whether this brush can be picked at all, which is a different question
    // from whether it is being shown: the filter lists another
    // representation's vocabulary, and most of it has no verb here.
    let usable = tool.exists_on(state.representation);
    // A backdrop under the active swatch and the one under the pointer, set
    // before the swatch is drawn and filled after, once its extent is known.
    // The active brush is then carried by tone as well as by the accent, which
    // is what a colour-blind sculptor reads; and a swatch lifting under the
    // pointer is the "quiet until addressed" rule in the one place it was
    // missing.
    let backdrop = ui.painter().add(egui::Shape::Noop);
    let group = ui.vertical(|ui| {
        // `size::SWATCH`, which is the size named for this. It was changed to
        // the size of one entry in the recent-colour row, added in the same
        // commit, and the shelf spent a release drawing its brushes as
        // sixteen-pixel discs with their marks illegible inside them.
        //
        // A browsed brush senses hover and not clicks, which is what actually
        // refuses the selection — the `usable` guard on the click below is a
        // second lock on the same door, and removing either one alone leaves
        // it shut.
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(size::SWATCH, size::SWATCH),
            if usable {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            },
        );
        paint_sphere(
            ui,
            rect,
            if usable {
                Tokens::text_dim()
            } else {
                Tokens::text_faint()
            },
            active,
        );
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(brush_swatch_id(tool), rect));
        // The brush's mark, in the ground's ink: dark on the lit clay, the way
        // a mark pressed into a ball reads. Not the accent, which stays on the
        // active brush alone.
        glyphs::paint(ui.painter(), rect, tool, Tokens::ground());
        ui.label(
            egui::RichText::new(state.strings.tool(tool))
                .size(type_scale::LABEL)
                // The accent, on the active brush and nowhere else.
                .color(if active {
                    Tokens::accent()
                } else if usable {
                    Tokens::text_dim()
                } else {
                    Tokens::text_faint()
                }),
        );
        // The name and what the brush does, for a hand that hovers. ZBrush
        // teaches its brushes by tooltip; one sentence costs nothing and saves
        // a stroke and an undo.
        // Starring, from the brush's own menu — the gesture a layer row already
        // uses for the things that are not its primary click. A brush met while
        // browsing can be starred too: a shortlist is for finding a brush
        // again, and the layer it applies to is a separate question.
        let starred = state.favourites.contains(&tool);
        response.context_menu(|ui| {
            let label = if starred {
                state.strings.action_favourite_remove
            } else {
                state.strings.action_favourite_add
            };
            if ui.button(label).clicked() {
                ui.ctx()
                    .data_mut(|data| data.insert_temp(favourite_toggle_id(), tool));
                ui.close_menu();
            }
        });

        let response = response.on_hover_text(if usable {
            format!(
                "{}\n{}",
                state.strings.tool(tool),
                // The sentence *for this representation*: a tool whose engine
                // verb differs on the active layer says so here, rather than
                // describing one representation's behaviour to a sculptor
                // holding another.
                state.strings.tool_sentence(tool, state.representation)
            )
        } else {
            // Browsed rather than offered, so the tooltip says why it cannot
            // be picked instead of describing a stroke that will not happen.
            format!(
                "{}\n{}",
                state.strings.tool(tool),
                state.strings.shelf_tool_elsewhere
            )
        });
        if usable && response.clicked() {
            queue.push(Command::SelectTool(tool));
        }
        response.hovered()
    });
    // No rail here, though the active layer wears one. The swatch already
    // carries the same gesture at the same weight — a thin accent stroke
    // tracing the thing itself, which for a ball is a ring — plus a raised
    // card and a label in the accent. A rail would be a fourth mark on one
    // sixty-pixel card, and the restraint the style budget asks for is the
    // reason the layer row needed a mark in the first place: it had none.
    if usable && (active || group.inner) {
        ui.painter().set(
            backdrop,
            egui::Shape::rect_filled(
                group.response.rect.expand(space::TIGHT),
                size::RADIUS,
                Tokens::raised(),
            ),
        );
    }
}

/// One button on the rail: an icon, lit when its state is on.
///
/// The tooltip is the whole of its label — the rail is icons alone, so the
/// word lives on hover — with the key that does the same thing where one is
/// bound. Recorded under `chip_id(label)` so a test can find it by the word
/// rather than by the pixel.
pub(super) fn rail_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    tooltip: String,
    on: bool,
    enabled: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(size::RAIL_BUTTON, size::RAIL_BUTTON),
        egui::Sense::click(),
    );
    let tint = if !enabled {
        Tokens::text_faint()
    } else if on || response.hovered() {
        Tokens::text()
    } else {
        Tokens::text_dim()
    };
    if enabled && (on || response.hovered()) {
        ui.painter()
            .rect_filled(rect, size::RADIUS, Tokens::raised());
    }
    let icon_rect =
        egui::Rect::from_center_size(rect.center(), egui::vec2(size::CHIP_ICON, size::CHIP_ICON));
    icons::paint(ui.painter(), icon_rect, icon, tint);
    ui.ctx()
        .memory_mut(|memory| memory.data.insert_temp(chip_id(label), rect));
    response.on_hover_text(tooltip)
}

/// One entry on the rail: what it shows, what it says, and what it does.
pub(super) struct RailEntry {
    icon: Icon,
    label: &'static str,
    tooltip: String,
    on: bool,
    enabled: bool,
    command: Command,
}

/// The tool rail, on the leading edge as the design places it.
///
/// Every button dispatches the command its menu entry does, under the same
/// conditions, so the two cannot disagree; the rail exists because the menus
/// were the *only* way to the shapes panel, the cage, the deformations, the
/// references and the curve, and a panel three menus deep is a panel a new
/// sculptor never opens. ZBrush keeps these on its shelves for the same
/// reason. Grouped by what they are: what the pointer does, what the view
/// shows, which panels and modes are up, and history.
pub fn tool_rail(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    let cageable = clayspace_model::can_be_caged(state.representation);
    let plain = |label: &'static str| label.to_owned();
    let groups: [Vec<RailEntry>; 4] = [
        // What the pointer does: sculpt, or paint the mask. On while the
        // mask brush is in hand, which is what the key toggles.
        vec![RailEntry {
            icon: Icon::MaskPaint,
            label: s.action_paint_mask,
            tooltip: labelled_chord(state, s.action_paint_mask, Action::ToggleMaskPainting),
            on: state.tool == ToolKind::Mascara,
            enabled: true,
            command: Command::ToggleMaskPainting,
        }],
        // What the view shows.
        vec![
            RailEntry {
                icon: Icon::Frame,
                label: s.action_frame_all,
                tooltip: labelled_chord(state, s.action_frame_all, Action::FrameAll),
                on: false,
                enabled: true,
                command: Command::FrameAll,
            },
            RailEntry {
                icon: Icon::Polyframe,
                label: s.action_polyframe,
                tooltip: labelled_chord(state, s.action_polyframe, Action::TogglePolyframe),
                on: state.polyframe,
                enabled: true,
                command: Command::TogglePolyframe,
            },
            RailEntry {
                icon: Icon::Reference,
                label: s.action_references,
                tooltip: plain(s.action_references),
                on: state.show_references,
                enabled: true,
                command: Command::ToggleReferences,
            },
        ],
        // The panels and the modes: what is placed rather than brushed.
        vec![
            RailEntry {
                icon: Icon::Shapes,
                label: s.action_shapes,
                tooltip: plain(s.action_shapes),
                on: state.show_shapes,
                enabled: true,
                command: Command::ToggleShapes,
            },
            RailEntry {
                icon: Icon::Union,
                label: s.action_boolean,
                tooltip: plain(s.action_boolean),
                on: state.show_boolean,
                enabled: true,
                command: Command::ToggleBoolean,
            },
            RailEntry {
                icon: Icon::Cage,
                label: s.action_cage,
                // Grey with the reason on it, as the menu has it.
                tooltip: if cageable {
                    plain(s.action_cage)
                } else {
                    format!("{}\n{}", s.action_cage, s.status_cage_needs_a_field)
                },
                on: state.lattice.active,
                enabled: cageable,
                command: Command::ToggleLattice,
            },
            RailEntry {
                icon: Icon::Curve,
                label: s.action_curve,
                tooltip: plain(s.action_curve),
                on: state.curve.active,
                enabled: true,
                command: Command::ToggleCurve,
            },
            RailEntry {
                icon: Icon::Taper,
                label: s.action_deform,
                tooltip: plain(s.action_deform),
                on: state.show_deform,
                enabled: true,
                command: Command::ToggleDeform,
            },
        ],
        // History, greyed exactly as the Edit menu greys it.
        vec![
            RailEntry {
                icon: Icon::Undo,
                label: s.action_undo,
                tooltip: labelled_chord(state, s.action_undo, Action::Undo),
                on: false,
                enabled: state.can_undo,
                command: Command::Undo,
            },
            RailEntry {
                icon: Icon::Redo,
                label: s.action_redo,
                tooltip: labelled_chord(state, s.action_redo, Action::Redo),
                on: false,
                enabled: state.can_redo,
                command: Command::Redo,
            },
        ],
    ];

    ui.vertical_centered(|ui| {
        ui.add_space(space::SNUG);
        for (index, group) in groups.into_iter().enumerate() {
            if index > 0 {
                rail_rule(ui);
            }
            for entry in group {
                let response = rail_button(
                    ui,
                    entry.icon,
                    entry.label,
                    entry.tooltip,
                    entry.on,
                    entry.enabled,
                );
                if response.clicked() && entry.enabled {
                    queue.push(entry.command);
                }
            }
        }
    });
}

/// A short hairline between the rail's groups.
pub(super) fn rail_rule(ui: &mut egui::Ui) {
    ui.add_space(space::SNUG);
    let (rule, _) = ui.allocate_exact_size(egui::vec2(size::CHIP_ICON, 1.0), egui::Sense::hover());
    ui.painter().rect_filled(rule, 0.0, Tokens::rule());
    ui.add_space(space::SNUG);
}
