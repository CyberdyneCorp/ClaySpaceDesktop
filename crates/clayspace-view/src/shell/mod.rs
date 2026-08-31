//! The window's regions, and what goes in each.
//!
//! Every function here is a pure function of ViewModel state that emits
//! commands. None of them mutates anything, calls the Model, or performs I/O —
//! the crate cannot reach the engine to do so even if one tried.
//!
//! The layout is the design's: a menu bar, a tool rail on the leading edge, a
//! tool options bar under the menus, a left region carrying the scene and
//! layers, a central viewport, a right region of inspectors, a brush shelf
//! along the trailing edge, and a status area.
//!
//! One module a region, plus `widgets` for the vocabulary they share. This was
//! a single five-thousand-line file until a new region had to go into it; the
//! split is a move and nothing else, so what could call what is unchanged and
//! the tests that read this source now walk it rather than listing one
//! directory.

use clayspace_model::{
    AlphaSupport, BlendProfile, BrushSettings, Combine, CombineSettings, DeformSettings,
    DeformVerb, Diagnostics, Direction, ExportMesher, ExportSettings, ExportWarning,
    ExtrudeSettings, ExtrudeSide, Falloff, GizmoMode, ImportAs, ImportSettings, LayerKey,
    LayerSummary, MaskGesture, MaskOp, MaskState, OutlineDraft, OutlineMode, RecentDocuments,
    RefPlane, ReferenceSettings, Representation, Scene, SceneStats, SculptLayer, SculptLayerCost,
    SculptLayerOp, SurfaceOpacity, ToolKind, Units, ViewPresetKind,
};
use clayspace_vm::{Axis, Command, CommandQueue};

use crate::design::{size, space, type_scale, Tokens};
use crate::glyphs;
use crate::icons::{self, Icon};
use crate::matcap::MatCap;
use crate::shortcuts::{Action, Shortcuts};
use crate::strings::Locale;
use crate::strings::Strings;

mod booleans;
mod inspector;
mod left;
mod menus;
mod options;
mod right;
mod shapes;
mod shelf;
mod widgets;
mod windows;
mod workspace;

// The shared vocabulary, and the three modules whose sections another module
// embeds. Re-established at the `shell` level because these items reached each
// other freely while they shared one file, and this is a move rather than a
// redesign of who may call what.
//
// The five regions — menus, options, left, right, shelf — are absent on
// purpose: nothing calls into them, which is what makes them the leaves of
// this tree and the safe places for a new region to land.
use booleans::*;
use shapes::*;
use widgets::*;
use windows::*;

pub use booleans::boolean_op_chip_id;
pub use left::{
    layer_row_id, layer_transform_chip_id, left_panel, new_layer_button_id, new_layer_kind_id,
    new_layer_kind_menu_id, optimize_button_id,
};
pub use menus::menu_bar;
pub use options::{brush_badge_id, options_bar};
pub use right::right_panel;
pub use shapes::{insert_as_chip_id, object_rows};
pub use shelf::{
    brush_shelf, brush_swatch_id, favourite_toggle_id, shelf_filter_chip_id, shelf_filter_id,
    tool_rail, ShelfFilter,
};
pub use widgets::{chip_id, close_id, heading_id, readout_id, slider_id, slider_widget_id};
pub use windows::{
    attribution_window, convert_window, deform_window, diagnostics_window, export_window,
    import_window, reference_slider_name, reference_window, repair_window, ReferenceSlot,
};
pub use workspace::{convert_to_id, representation_bar, representation_card_id};

/// Everything a frame of interface needs to read.
///
/// Assembled by the composition root from the ViewModels. Passing one struct
/// rather than a dozen arguments keeps a View function's signature honest
/// about being a function of state.
/// What the interface needs to know about the rig.
///
/// A summary rather than the tree itself: the shell draws no spheres — the
/// viewport does — and handing it the tree would invite it to.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ArmatureState {
    /// Whether the active layer has a rig at all.
    pub exists: bool,
    /// Whether the pointer is currently rigging rather than sculpting.
    pub editing: bool,
    /// Whether a sphere is selected, which is what makes removal meaningful.
    pub selection: bool,
    /// Whether the viewport is drawing the skin or only the ZSpheres.
    pub skin_preview: bool,
    /// Whether the selected sphere cuts rather than adds.
    pub selection_is_negative: bool,
    /// How many spheres, for the readout.
    pub spheres: usize,
    pub mirror: bool,
    pub skin: f32,
}

pub struct ShellState<'a> {
    /// What is frozen, for the mask menu.
    pub mask: MaskState,
    /// Which gesture the mask brush makes.
    pub mask_gesture: MaskGesture,
    /// The outline being drawn over the viewport, while one is.
    pub outline: Option<&'a OutlineDraft>,
    /// The rig, as the menu and the armature panel need it.
    pub armature: ArmatureState,
    /// Documents opened lately, most recent first.
    pub recent: &'a [std::path::PathBuf],
    /// The exchange panels: whether they are open, and what they would do.
    pub show_repair: bool,
    /// What is wrong with the active grid. `None` where it is not one.
    pub repair: Option<clayspace_model::RepairReport>,
    pub show_convert: bool,
    /// What the conversion panel is set to, and what that would cost.
    ///
    /// The cost is computed by the layer that can see the document's bounds
    /// and handed in, rather than recomputed here: a View that worked out its
    /// own answer could disagree with the one the conversion actually uses.
    pub conversion: clayspace_model::ConversionSettings,
    pub conversion_cost: Option<clayspace_model::Cost>,
    /// The boolean panel: whether it is open, what it is set to, what could
    /// take part, what the pair would cost, and why the last attempt was
    /// refused.
    ///
    /// The cost is computed by the layer that can see both operands' extents
    /// and handed in, as the conversion's is and for the same reason: a View
    /// that worked out its own answer could disagree with the one the
    /// operation actually uses.
    pub show_boolean: bool,
    pub boolean: clayspace_model::BooleanSettings,
    pub boolean_operands: &'a [(LayerKey, String)],
    pub boolean_cost: Option<clayspace_model::Cost>,
    pub boolean_notice: Option<&'a str>,
    pub show_import: bool,
    pub show_export: bool,
    pub import: ImportSettings,
    pub export: ExportSettings,
    /// What the export as configured would give up.
    pub export_warnings: &'a [ExportWarning],
    /// This build and this machine.
    pub diagnostics: &'a Diagnostics,
    /// Whether the diagnostics window is open.
    pub show_diagnostics: bool,
    /// Whether the report was just put on the clipboard, for the confirmation.
    pub diagnostics_copied: bool,
    /// The attribution manifest, and whether it is open.
    pub attribution: &'a str,
    pub show_attribution: bool,
    /// What an extrusion would use.
    pub extrude: ExtrudeSettings,
    /// How far Expandir, Contrair and Suavizar máscara reach.
    pub mask_steps: i32,
    /// Which picture of a voxel layer the viewport draws, and how much its
    /// occupancy is filtered before the smooth one is taken.
    pub voxel_display: clayspace_model::VoxelDisplay,
    pub voxel_blur: clayspace_model::SmoothBlur,
    /// The curve being placed, while one is up, and the thickness a new point
    /// would be given.
    pub curve: clayspace_model::CurveState,
    pub curve_radius: f32,
    /// The shapes panel: whether it is open, what it is set to, and what has
    /// been placed.
    pub show_shapes: bool,
    /// Where the next insertion lands: a subtool of its own, or an object in
    /// the active layer.
    pub insert_as: clayspace_model::InsertAs,
    /// The subtools a copy could be made from, and what they are called.
    pub copyable_subtools: &'a [(LayerKey, String)],
    /// The mesh layers that could be placed as a boolean operand, and which
    /// one the picker is set to.
    pub mesh_operands: &'a [(LayerKey, String)],
    pub mesh_operand: Option<LayerKey>,
    /// What placing that mesh would cost — the conversion's own figures.
    pub mesh_operand_cost: Option<clayspace_model::Cost>,
    pub shape: clayspace_model::Shape,
    pub shape_parameters: &'a [f32],
    /// How a placed shape combines. Its own value rather than the stroke's:
    /// placing a shape means Add, and a stroke starts at Relief.
    pub object_combine: CombineSettings,
    pub objects: &'a [clayspace_model::SceneObject],
    pub selected_object: Option<clayspace_model::ObjectId>,
    /// Which of the manipulator's three modes is in force.
    pub gizmo_mode: clayspace_model::GizmoMode,
    /// What the manipulator is acting on, so a control that puts it on the
    /// whole layer can read as on rather than guessing from the mode.
    pub gizmo_target: Option<clayspace_model::GizmoTarget>,
    /// The cage around the form, while one is up.
    pub lattice: clayspace_model::LatticeState,
    /// What a fresh cage would be built with.
    pub lattice_divisions: [i32; 3],
    pub strings: &'a Strings,
    /// The bindings in force, so a menu item can show the chord that does the
    /// same thing. Borrowed rather than copied because remapping replaces the
    /// table and a menu built from a stale copy would advertise the binding
    /// the user just changed.
    pub shortcuts: &'a Shortcuts,
    pub document_name: &'a str,
    pub modified: bool,

    pub tool: ToolKind,
    /// What the active layer holds.
    ///
    /// The shelf offers this representation's verbs and nothing else, so this
    /// is what decides the shelf's contents rather than a fixed list.
    pub representation: Representation,
    pub brush: BrushSettings,
    /// How the next SDF edit combines with what is under it.
    pub combine: CombineSettings,
    /// The loaded alpha stamp's name, if one is loaded.
    ///
    /// The name and not the samples: the interface says which stamp is in use
    /// and has no business holding megabytes to do it.
    pub alpha: Option<&'a str>,
    /// What the colour brushes paint with, and the colours before it.
    ///
    /// Borrowed rather than copied: the recent list is a `Vec`, and the shell
    /// state is built afresh every frame.
    pub colour: &'a clayspace_model::ColourState,
    /// The reference panel: whether it is open, and what is on each plane.
    pub show_references: bool,
    pub references: [ReferenceSlot<'a>; RefPlane::ALL.len()],
    /// How opaque the sculpted surface is drawn.
    pub surface_opacity: SurfaceOpacity,
    /// The deform panel: whether it is open and what it would do.
    pub show_deform: bool,
    pub deform: DeformSettings,
    /// What the active layer's recorded passes cost, and whether one is
    /// being recorded right now.
    pub sculpt_cost: SculptLayerCost,
    /// Why the active tool cannot be used, when it cannot.
    pub tool_status: Option<&'a str>,
    pub symmetry: [bool; 3],

    pub scene: &'a Scene,
    /// Which layer is being renamed and what its field holds.
    ///
    /// The draft lives outside the View because the View is a pure function of
    /// state: a buffer owned by a widget would be the one piece of the
    /// interface a test could not set up or read back.
    pub renaming: Option<(LayerKey, &'a str)>,
    pub stats: SceneStats,

    pub view_preset: ViewPresetKind,
    /// Whether a mesh layer is drawn with its own edges over it.
    pub polyframe: bool,
    /// Whether the sculpt is shaded with the studio light rig rather than a
    /// MatCap. Display state the renderer owns, like the material.
    pub studio_shading: bool,
    /// How much an idle frame is worth spending on, as the governor holds it.
    pub viewport_profile: crate::quality::ViewportProfile,
    /// Which regions are put away, in `Panel::ALL` order.
    pub collapsed: [bool; crate::layout::Panel::ALL.len()],
    /// Whether the chrome is cleared away and only the sculpt is left.
    pub focus: bool,
    /// How long until the next autosave, where one is pending.
    ///
    /// `None` when the document matches what is on disk, which is not the same
    /// as "no autosave configured": an unmodified document is never written, so
    /// there is nothing to count down to. The status area says which.
    pub autosave_in: Option<std::time::Duration>,
    /// The brushes this sculptor starred, in no particular order.
    ///
    /// A preference rather than document state: it belongs to the person and
    /// not to the sculpture, so it is stored beside the recent list and the
    /// arrangement of the regions and reaches no `.clay` file.
    pub favourites: &'a [ToolKind],
    /// Whether small creases are sharpened by the screen-space curvature term.
    pub cavity: bool,
    /// Whether the studio rig's key light casts.
    pub shadows: bool,
    pub material: &'a str,
    /// The material itself, for the preview to be painted from — the same
    /// sphere image the viewport shades with, so the swatch is the material
    /// and not a grey ball standing in for it.
    pub matcap: MatCap,
    pub materials: &'a [&'a str],

    pub can_undo: bool,
    pub can_redo: bool,

    /// Bytes in use and the budget, for the memory meter.
    pub memory: (u64, u64),
    pub backend: &'a str,
    /// The document's scale and what lengths are shown in.
    pub units: Units,
    /// What the last action did, for the status area.
    pub last_action: Option<(&'a str, bool)>,
}

/// Widths the design's regions take, in logical pixels.
pub mod region {
    pub const RAIL: f32 = 46.0;
    pub const LEFT: f32 = 232.0;
    pub const RIGHT: f32 = 248.0;
    pub const MENU_BAR: f32 = 30.0;
    pub const OPTIONS_BAR: f32 = 62.0;
    /// The representation bar, above the viewport. Inside the central region
    /// rather than a panel of its own: it belongs to the viewport it labels,
    /// and a full-width strip would run behind the inspectors on both sides.
    pub const REPRESENTATION_BAR: f32 = 56.0;
    pub const SHELF: f32 = 84.0;
    pub const STATUS: f32 = 28.0;
}

/// Where the brush readout was drawn, so a test can ask whether it was.
pub fn brush_hud_id() -> egui::Id {
    egui::Id::new("brush-hud")
}

/// How wide the brush readout runs.
const BRUSH_HUD_WIDTH: f32 = 156.0;

/// The active brush and its numbers, over the viewport.
///
/// For focus mode, where the options bar that carries them has gone. Without
/// it, clearing the chrome away also clears away the size and the intensity a
/// sculptor is working at — which is not focus, it is working blind.
///
/// The brush's own ball at the head of it, as the options bar has, so the two
/// read as the same thing in two places rather than as two controls.
pub fn brush_hud(ui: &egui::Ui, rect: egui::Rect, state: &ShellState<'_>) {
    let s = state.strings;
    let rows = [
        (s.label_size, state.units.format(state.brush.size)),
        (s.label_intensity, format!("{:.2}", state.brush.intensity)),
        (s.label_flow, format!("{:.2}", state.brush.flow)),
    ];

    let line = type_scale::NUMERIC + space::TIGHT;
    let head = size::BADGE.max(type_scale::BODY * 2.0);
    let height = space::SNUG * 2.0 + head + rows.len() as f32 * (line + type_scale::HEADING);
    // The upper-leading corner: the opposite one from the transform readout,
    // so the two never stack, and the corner a sculptor's hand is least often
    // over.
    let card = egui::Rect::from_min_size(
        egui::pos2(rect.left() + HUD_MARGIN, rect.top() + HUD_MARGIN),
        egui::vec2(BRUSH_HUD_WIDTH, height),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(card, size::RADIUS, Tokens::panel().gamma_multiply(0.88));

    // The ball and the brush's mark, at the head.
    let ball = egui::Rect::from_min_size(
        card.min + egui::vec2(space::SNUG, space::SNUG),
        egui::Vec2::splat(size::BADGE),
    );
    paint_sphere(ui, ball, Tokens::text_dim(), false);
    glyphs::paint(&painter, ball, state.tool, Tokens::ground());
    painter.text(
        egui::pos2(ball.max.x + space::SNUG, ball.center().y - space::TIGHT),
        egui::Align2::LEFT_BOTTOM,
        s.tool(state.tool),
        egui::FontId::proportional(type_scale::BODY),
        Tokens::text(),
    );
    // Which representation the stroke will land on, which the layer stack and
    // the representation bar both say and neither is on screen here.
    painter.text(
        egui::pos2(ball.max.x + space::SNUG, ball.center().y + space::TIGHT),
        egui::Align2::LEFT_TOP,
        s.representation_name(state.representation),
        egui::FontId::proportional(type_scale::HEADING),
        Tokens::text_faint(),
    );

    let mut y = ball.max.y + space::SNUG;
    for (label, value) in rows {
        painter.text(
            egui::pos2(card.left() + space::SNUG, y),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(type_scale::HEADING),
            Tokens::text_faint(),
        );
        painter.text(
            egui::pos2(card.right() - space::SNUG, y),
            egui::Align2::RIGHT_TOP,
            value,
            egui::FontId::monospace(type_scale::NUMERIC),
            Tokens::text(),
        );
        y += line + type_scale::HEADING;
    }

    ui.ctx()
        .memory_mut(|memory| memory.data.insert_temp(brush_hud_id(), card));
}

/// Where a request to put a region away, or bring it back, is left.
///
/// The same route the viewport profile takes and for the same reasons: the
/// arrangement of the regions touches no document, and `Panel` is a view type
/// that a command in the layer below could not carry.
pub fn panel_toggle_id() -> egui::Id {
    egui::Id::new("panel-toggle")
}

/// Where a request to clear the chrome away, or bring it back, is left.
pub fn focus_toggle_id() -> egui::Id {
    egui::Id::new("focus-toggle")
}

/// Where a request to return every region to the design's size is left.
pub fn layout_reset_id() -> egui::Id {
    egui::Id::new("layout-reset")
}

/// Where a chosen viewport profile is left for the composition root to read.
///
/// The profile is the sculptor's choice about what an idle frame looks like,
/// and it touches no document — so it goes the way the section folds and the
/// shelf's filter go, through the interface's own memory. It could not go as a
/// command in any case: `ViewportProfile` is a view type, and commands live in
/// the layer underneath this one.
///
/// Not remembered across launches, for the same reason the shelf has no
/// favourites: there is nowhere yet to remember it.
pub fn viewport_profile_id() -> egui::Id {
    egui::Id::new("viewport-profile")
}

/// Applies the design system to an egui context.
///
/// Called once. Everything after it inherits these, so a widget that looks
/// wrong is a widget reaching past the tokens rather than a theme that was
/// never applied.
pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = Tokens::panel();
    visuals.window_fill = Tokens::panel();
    visuals.extreme_bg_color = Tokens::ground();
    visuals.faint_bg_color = Tokens::raised();
    visuals.override_text_color = Some(Tokens::text());

    // Flat: no shadows, no gloss. The skeuomorphic share of the style budget
    // is spent on the brush swatches and the material previews, and a shadow
    // under every panel would spend it here instead.
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.popup_shadow = egui::epaint::Shadow::NONE;

    let radius = egui::epaint::CornerRadius::same(size::RADIUS as u8);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = radius;
        widget.bg_stroke = egui::Stroke::NONE;
    }

    // Quiet at rest, gaining contrast on hover and while being adjusted.
    visuals.widgets.noninteractive.bg_fill = Tokens::panel();
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, Tokens::text_dim());
    visuals.widgets.inactive.bg_fill = Tokens::raised();
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, Tokens::text_dim());
    visuals.widgets.hovered.bg_fill = Tokens::raised();
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, Tokens::text());
    visuals.widgets.active.bg_fill = Tokens::raised();
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, Tokens::text());
    visuals.selection.bg_fill = Tokens::raised();
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, Tokens::text());

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(space::SNUG, space::TIGHT);
    style.spacing.button_padding = egui::vec2(space::SNUG, space::TIGHT);
    style.spacing.slider_width = 96.0;
    style.spacing.interact_size.y = size::CONTROL;
    ctx.set_style(style);
}

/// The document's name as the sculptor should read it.
///
/// A fresh document carries the ViewModel's untitled marker, which is fixed
/// because the ViewModel knows no locale; this is where that marker becomes
/// the word in the interface's language. A name that came from a file is the
/// file's and passes through untouched.
pub fn document_display_name<'a>(strings: &'a Strings, name: &'a str) -> &'a str {
    if name == clayspace_vm::UNTITLED {
        strings.document_untitled
    } else {
        name
    }
}

/// The id the autosave line is recorded under, for tests.
pub fn autosave_id() -> egui::Id {
    egui::Id::new("autosave-readout")
}

/// How long until the next autosave, or that there is nothing waiting.
///
/// Two states rather than a countdown that stops: an unmodified document is
/// never written — autosave exists to preserve work that is not on disk — so a
/// timer frozen at zero would read as a stalled save rather than as a saved
/// document.
///
/// A dot before it, in the accent while something is pending and quiet
/// otherwise, so the state is carried by more than the words: this is the one
/// line in the status area a sculptor reads to know whether their work is safe.
fn autosave_readout(ui: &mut egui::Ui, state: &ShellState<'_>) {
    let s = state.strings;
    let row = ui.horizontal(|ui| {
        let (dot, _) =
            ui.allocate_exact_size(egui::vec2(space::SNUG, space::SNUG), egui::Sense::hover());
        ui.painter().circle_filled(
            dot.center(),
            space::HAIR,
            if state.autosave_in.is_some() {
                Tokens::accent()
            } else {
                Tokens::text_faint()
            },
        );
        match state.autosave_in {
            Some(left) => {
                ui.label(
                    egui::RichText::new(s.label_autosave_in)
                        .size(type_scale::HEADING)
                        .color(Tokens::text_faint()),
                );
                numeric(ui, clock(left));
            }
            None => {
                ui.label(
                    egui::RichText::new(s.state_autosaved)
                        .size(type_scale::HEADING)
                        .color(Tokens::text_faint()),
                );
            }
        }
    });
    ui.ctx()
        .memory_mut(|memory| memory.data.insert_temp(autosave_id(), row.response.rect));
}

/// A duration as minutes and seconds, which is how a countdown is read.
fn clock(left: std::time::Duration) -> String {
    let seconds = left.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// The status area: document, memory, backend and units.
pub fn status_bar(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.horizontal(|ui| {
        ui.add_space(space::PANEL);

        let (used, budget) = state.memory;
        ui.label(
            egui::RichText::new(s.label_memory)
                .size(type_scale::HEADING)
                .color(Tokens::text_faint()),
        );
        numeric(ui, format!("{} / {}", gigabytes(used), gigabytes(budget)));

        // Approaching the budget changes state before it is exhausted, rather
        // than only at failure.
        let fraction = if budget == 0 {
            0.0
        } else {
            used as f32 / budget as f32
        };
        let (bar, _) = ui.allocate_exact_size(egui::vec2(120.0, 4.0), egui::Sense::hover());
        ui.painter().rect_filled(bar, 0.0, Tokens::raised());
        let filled = egui::Rect::from_min_size(
            bar.min,
            egui::vec2(bar.width() * fraction.clamp(0.0, 1.0), bar.height()),
        );
        ui.painter().rect_filled(
            filled,
            0.0,
            if fraction > 0.85 {
                Tokens::accent()
            } else {
                Tokens::text_dim()
            },
        );

        // Whether the work is on disk, and when it will be. The policy and the
        // clock have both been here since autosave shipped and neither had
        // ever been shown: a sculptor could not tell whether an hour's work was
        // written or waiting.
        ui.add_space(space::SECTION);
        autosave_readout(ui, state);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(space::PANEL);
            // A control, not a label: the design shows the unit here, and a
            // person looking for where to change it looks where it is shown.
            let shown = format!("{}: {}", s.label_units, unit_symbol(state.units));
            if ui
                .add(egui::Button::new(
                    egui::RichText::new(shown)
                        .size(type_scale::LABEL)
                        .family(egui::FontFamily::Monospace)
                        .color(Tokens::text_dim()),
                ))
                .on_hover_text(s.hint_units)
                .clicked()
            {
                queue.push(Command::NextDisplayUnit);
            }
            ui.add_space(space::SECTION);
            numeric(ui, format!("{}: {}", s.label_backend, state.backend));
            if let Some((label, changed)) = state.last_action {
                ui.add_space(space::SECTION);
                let text = if changed {
                    label.to_string()
                } else {
                    format!("{label} — {}", s.state_nothing_changed)
                };
                ui.label(
                    egui::RichText::new(text)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
            }
        });
    });
}

/// The outline being drawn, traced over the viewport.
///
/// Drawn by the View from ViewModel state rather than by the event loop that
/// collects the points: the composition root can see the pointer and the View
/// can see the design, and the line a sculptor is drawing is a picture rather
/// than an input.
///
/// `rect` is the viewport's own rectangle — the one the scene is drawn into
/// and the one the pointer was measured against, so the line lands under the
/// cursor rather than beside it.
pub fn outline_overlay(ui: &egui::Ui, rect: egui::Rect, state: &ShellState<'_>) {
    let Some(draft) = state.outline else {
        return;
    };
    let corners = draft.corners();
    if corners.len() < 2 {
        return;
    }
    let points: Vec<egui::Pos2> = corners
        .iter()
        .map(|ndc| outline_point(rect, *ndc))
        .collect();

    // Freezing and releasing are the same gesture and must not look alike:
    // the accent draws what will be frozen, and a dim neutral what will be
    // let go.
    let tint = match draft.mode {
        OutlineMode::Freeze => Tokens::accent(),
        OutlineMode::Thaw => Tokens::text_dim(),
    };
    let closing = points.first().copied().zip(points.last().copied());
    let painter = ui.painter_at(rect);
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(OUTLINE_WIDTH, tint),
    ));
    if let Some((first, last)) = closing {
        // A lasso closes itself across a gap the sculptor can see, so the edge
        // that will close it is drawn faint: showing where it lands is what
        // makes it predictable rather than surprising. A rectangle has no such
        // gap — four corners are four edges — and drawing one of them faint
        // would suggest it were less certain than the other three.
        let closing_tint = if draft.gesture.closes_a_gap() {
            tint.gamma_multiply(OUTLINE_CLOSING)
        } else {
            tint
        };
        painter.add(egui::Shape::line_segment(
            [last, first],
            egui::Stroke::new(OUTLINE_WIDTH, closing_tint),
        ));
    }
}

/// How wide the drawn outline is, in points.
const OUTLINE_WIDTH: f32 = 1.5;

/// How much of the outline's tint the closing edge keeps.
const OUTLINE_CLOSING: f32 = 0.45;

/// Where a normalised device coordinate lands in the viewport.
///
/// The inverse of what the ray through the pointer is built from, and it has
/// to stay the inverse: an overlay drawn from a different mapping than the
/// gesture is resolved with is a line that does not enclose what it appears
/// to.
///
/// The same arithmetic as the tail of `input::screen_at`, and it is copied
/// rather than shared because the layering says so: a View may not reach the
/// composition root. Two lines, and both carry a note to the other.
fn outline_point(rect: egui::Rect, ndc: [f32; 2]) -> egui::Pos2 {
    egui::pos2(
        rect.min.x + (ndc[0] + 1.0) * 0.5 * rect.width(),
        rect.min.y + (1.0 - ndc[1]) * 0.5 * rect.height(),
    )
}

/// Where the transform readout was drawn, so a test can ask whether it was.
pub fn transform_hud_id() -> egui::Id {
    egui::Id::new("transform-hud")
}

/// How wide the readout runs, and how far it sits from the viewport's corner.
const HUD_WIDTH: f32 = 148.0;
const HUD_MARGIN: f32 = 12.0;

/// The selected object's transform, beside the manipulator that changes it.
///
/// The one thing a manipulator cannot say is *what the numbers are*. A drag
/// moves the form and the sculptor watches it move; whether it has moved twelve
/// millimetres or twelve and a half is not something a widget can show, and it
/// is the question asked as soon as two objects have to line up.
///
/// Shown only while a manipulator is up and pointed at a placed object. A
/// cage's target is a set of control points and a layer's is everything it
/// holds — neither has a single position, rotation and scale to report, and a
/// readout that showed the pivot instead would be answering a question nobody
/// asked with a number that looks like the answer to one they did.
///
/// # What it does not show
///
/// Three rotations. The engine stores a quaternion and its ABI speaks axis and
/// angle, which it is careful to call *a* representative rather than the
/// representation — decomposing one into three Euler angles picks one of
/// several answers, and a readout that changed its numbers when nothing had
/// moved would be worse than one that shows what the document holds.
///
/// The scale is three, because the engine's node transform has taken three
/// since ABI 0.54.0. It showed one for as long as nothing had bound
/// `clay_layer_set_transform_nonuniform`.
pub fn transform_hud(ui: &egui::Ui, rect: egui::Rect, state: &ShellState<'_>) {
    let Some(clayspace_model::GizmoTarget::Object(id)) = state.gizmo_target else {
        return;
    };
    let Some(object) = state.objects.iter().find(|object| object.id == id) else {
        return;
    };
    let s = state.strings;

    // The unit on the heading rather than after each of three numbers, which
    // is what the reference does and what makes the row fit: "0.1 mm 0.0 mm
    // -0.0 mm" ran off the card, and three copies of the same word is not
    // three pieces of information.
    let unit = unit_symbol(state.units);
    let triple = |values: &[f32; 3], decimals: usize| {
        values
            .iter()
            .map(|v| {
                // A value that rounds to nothing is nothing. Left alone, a
                // position of -0.0032 mm reads as "-0.0", which is a minus
                // sign in front of zero and the sort of thing a person reads
                // twice.
                let rounded = format!("{v:.decimals$}");
                match rounded.strip_prefix('-') {
                    Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_owned(),
                    _ => rounded,
                }
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    let rows: Vec<(String, String)> = vec![
        (
            format!("{} ({unit})", s.hud_position),
            triple(
                &object.position.map(|v| state.units.to_display(v)),
                state.units.display.decimals(),
            ),
        ),
        (
            format!("{} (°)", s.hud_rotation),
            format!("{:.1}", object.rotation_angle.to_degrees()),
        ),
        (s.hud_axis.to_owned(), triple(&object.rotation_axis, 2)),
        (s.hud_scale.to_owned(), triple(&object.scale, 3)),
    ];

    let line = type_scale::NUMERIC + space::TIGHT;
    let height = space::SNUG * 2.0 + rows.len() as f32 * (line + type_scale::HEADING);
    // The lower-leading corner, which is the one the viewport bar already
    // occupies the other end of, and the one a right-handed sculptor's hand is
    // least often over.
    let card = egui::Rect::from_min_size(
        egui::pos2(
            rect.left() + HUD_MARGIN,
            rect.bottom() - HUD_MARGIN - height,
        ),
        egui::vec2(HUD_WIDTH, height),
    );
    let painter = ui.painter_at(rect);
    // Translucent, so the form behind it is not hidden by a readout about
    // that form. Derived from a token rather than mixed here.
    painter.rect_filled(card, size::RADIUS, Tokens::panel().gamma_multiply(0.88));

    let mut y = card.top() + space::SNUG;
    for (label, value) in rows {
        painter.text(
            egui::pos2(card.left() + space::SNUG, y),
            egui::Align2::LEFT_TOP,
            &label,
            egui::FontId::proportional(type_scale::HEADING),
            Tokens::text_faint(),
        );
        y += type_scale::HEADING;
        painter.text(
            egui::pos2(card.left() + space::SNUG, y),
            egui::Align2::LEFT_TOP,
            value,
            egui::FontId::monospace(type_scale::NUMERIC),
            Tokens::text(),
        );
        y += line;
    }

    ui.ctx()
        .memory_mut(|memory| memory.data.insert_temp(transform_hud_id(), card));
}

/// The rubber band a press draws across the viewport while it gathers control
/// points.
///
/// Drawn from the interface's own tokens rather than by the composition root
/// with a colour of its own, which is what keeps the closed set closed. A fill
/// as well as an outline: an outline alone over a shaded form reads as part of
/// the model at a glance, and what a sculptor needs to see is which side of the
/// line they are on.
pub fn selection_box(painter: &egui::Painter, drawn: egui::Rect) {
    painter.rect_filled(
        drawn,
        egui::epaint::CornerRadius::ZERO,
        Tokens::raised().linear_multiply(0.35),
    );
    painter.rect_stroke(
        drawn,
        egui::epaint::CornerRadius::ZERO,
        egui::Stroke::new(1.0_f32, Tokens::text_dim()),
        egui::StrokeKind::Inside,
    );
}

/// The view presets, under the viewport as the design places them.
pub fn viewport_bar(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    ui.horizontal(|ui| {
        ui.add_space(space::PANEL);
        for preset in ViewPresetKind::ALL {
            let on = state.view_preset == preset;
            let action = match preset {
                ViewPresetKind::Perspective => Action::ViewPerspective,
                ViewPresetKind::Front => Action::ViewFront,
                ViewPresetKind::Side => Action::ViewSide,
                ViewPresetKind::Top => Action::ViewTop,
            };
            if with_chord(
                ui.add(chip(
                    state.strings.view_preset_name(preset),
                    on,
                    // The bar stands over the viewport, so an unselected chip
                    // fills with the viewport's ground and disappears into it.
                    // It filled with the shell's while the two were one
                    // colour, and would now read as a lighter rectangle
                    // floating on the sculpt's ground.
                    Tokens::viewport(),
                )),
                state,
                action,
            )
            .clicked()
            {
                queue.push(Command::SetViewPreset(preset));
            }
        }
        // The active layer's representation, at the far end of the bar. The
        // shelf's contents follow it, so it has to be visible without opening
        // a panel or a shelf that changed under you is unexplained.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(space::PANEL);
            // Why the tool cannot be used, or what the last gesture was refused
            // for, beside the viewport the sculptor is looking at. It stood at
            // the tail of the options bar, past the right edge at the design's
            // 1280, and was read by nobody.
            if let Some(reason) = state.tool_status {
                // The ViewModel carries no locale, so a status it raises itself
                // arrives as a marker and is localised here. An engine refusal
                // is already a sentence and passes through as one.
                let reason = match reason {
                    clayspace_vm::TOOL_SUBSTITUTED => state.strings.tool_substituted,
                    clayspace_vm::ITEM_NOT_TRANSFORMABLE => state.strings.item_not_transformable,
                    sentence => sentence,
                };
                ui.add_space(space::SECTION);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(reason)
                            .size(type_scale::LABEL)
                            .color(Tokens::accent()),
                    )
                    .truncate(),
                )
                .on_hover_text(reason);
            }
        });
    });
}

/// A short tag for a representation, for where a row has no room for a word.
///
/// Distinguished by the letters rather than by colour: the design system's
/// contrast theme changes the hues, and a tag that only a hue told apart would
/// stop saying anything under it.
fn representation_tag(representation: Representation) -> &'static str {
    match representation {
        Representation::Sdf => "SDF",
        Representation::Voxel => "VOX",
        Representation::Mesh => "MSH",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_grouped_as_the_design_shows_them() {
        assert_eq!(thousands(2_356_789), "2.356.789");
        assert_eq!(thousands(789), "789");
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(1_000), "1.000");
    }

    #[test]
    fn memory_reads_in_gigabytes() {
        assert_eq!(gigabytes(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(gigabytes(0), "0.00 GB");
    }

    #[test]
    fn the_regions_leave_room_for_a_viewport() {
        // The panels are fixed and the viewport absorbs the rest, so at the
        // smallest window the design targets there must still be a viewport.
        let width = 1280.0 - region::RAIL - region::LEFT - region::RIGHT;
        let height = 800.0
            - region::MENU_BAR
            - region::OPTIONS_BAR
            - region::REPRESENTATION_BAR
            - region::SHELF
            - region::STATUS;
        assert!(width > 400.0, "the viewport would be {width} wide");
        assert!(height > 300.0, "the viewport would be {height} tall");
    }
}
