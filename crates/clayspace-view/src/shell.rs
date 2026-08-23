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

use clayspace_model::{
    AlphaSupport, BlendProfile, BrushSettings, Combine, CombineSettings, DeformSettings,
    DeformVerb, Diagnostics, Direction, ExportMesher, ExportSettings, ExportWarning,
    ExtrudeSettings, ExtrudeSide, Falloff, ImportAs, ImportSettings, LayerSummary, MaskOp,
    MaskState, RecentDocuments, Representation, Scene, SceneStats, SculptLayer, SculptLayerCost,
    SculptLayerOp, ToolKind, Units, ViewPresetKind,
};
use clayspace_vm::{Axis, Command, CommandQueue};

use crate::design::{size, space, type_scale, Tokens};
use crate::icons::{self, Icon};
use crate::shortcuts::{Action, Shortcuts};
use crate::strings::Strings;

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
    pub stats: SceneStats,

    pub view_preset: ViewPresetKind,
    pub material: &'a str,
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
    pub const SHELF: f32 = 84.0;
    pub const STATUS: f32 = 28.0;
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

/// A section heading: small, spaced, low contrast.
fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(space::ROOMY);
    ui.label(
        egui::RichText::new(text)
            .size(type_scale::HEADING)
            .color(Tokens::text_faint()),
    );
    ui.add_space(space::TIGHT);
}

/// A numeric readout, set monospaced so digits do not reflow as they change.
fn numeric(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(
        egui::RichText::new(text)
            .monospace()
            .size(type_scale::NUMERIC)
            .color(Tokens::text()),
    );
}

/// A label and its value on one row.
fn readout(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            numeric(ui, value);
        });
    });
}

/// A slider with its value shown monospaced beside it.
fn slider(
    ui: &mut egui::Ui,
    label: &str,
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
            numeric(ui, format!("{edited:.decimals$}"));
        });
    });
    if ui
        .add(egui::Slider::new(&mut edited, range).show_value(false))
        .changed()
    {
        changed = Some(edited);
    }
    changed
}

// -- the regions -------------------------------------------------------------

/// How a menu item spells the shortcut that does the same thing.
///
/// Empty where nothing is bound, which is what `Button::shortcut_text` wants
/// for "this item has no shortcut" — so an unbound action simply reads as a
/// plain item rather than as a gap.
fn chord_text(state: &ShellState<'_>, action: Action) -> String {
    state
        .shortcuts
        .chord(action)
        .map(|chord| chord.label())
        .unwrap_or_default()
}

/// A menu item, labelled with the chord bound to the same action.
fn item(ui: &mut egui::Ui, state: &ShellState<'_>, label: &str, action: Action) -> egui::Response {
    ui.add(egui::Button::new(label).shortcut_text(chord_text(state, action)))
}

/// The same, greyed out when the action cannot be taken.
fn item_enabled(
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
                // Beside import and export, because a crossing is the same
                // kind of act: it produces a new layer from something the
                // document already holds, and states what it costs first.
                if ui.button(s.action_convert).clicked() {
                    queue.push(Command::ToggleConvert);
                    ui.close_menu();
                }
                if ui.button(s.action_repair).clicked() {
                    queue.push(Command::ToggleRepair);
                    ui.close_menu();
                }
                if ui.button(s.action_deform).clicked() {
                    queue.push(Command::ToggleDeform);
                    ui.close_menu();
                }
                if ui.button(s.action_import).clicked() {
                    queue.push(Command::ToggleImport);
                    ui.close_menu();
                }
                if ui.button(s.action_export).clicked() {
                    queue.push(Command::ToggleExport);
                    ui.close_menu();
                }
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
                    if item(ui, state, preset.label(), action).clicked() {
                        queue.push(Command::SetViewPreset(preset));
                        ui.close_menu();
                    }
                }
                if item(ui, state, s.action_frame_all, Action::FrameAll).clicked() {
                    queue.push(Command::FrameAll);
                    ui.close_menu();
                }
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
                    if ui.button(tool.label()).clicked() {
                        queue.push(Command::SelectTool(tool));
                        ui.close_menu();
                    }
                }
            });
            ui.menu_button(s.menu_dynamics, |_| {});
            ui.menu_button(s.menu_masks, |ui| {
                // Disabled rather than hidden: a menu whose entries come and
                // go is harder to learn than one whose entries are sometimes
                // grey, and the grey says *why* the tool is unavailable.
                for op in [
                    MaskOp::Invert,
                    MaskOp::Expand(1),
                    MaskOp::Contract(1),
                    MaskOp::Smooth(1),
                    MaskOp::InvertWithinBounds,
                    MaskOp::Clear,
                ] {
                    let enabled = !op.needs_a_mask() || state.mask.is_active();
                    if ui
                        .add_enabled(enabled, egui::Button::new(op.label()))
                        .clicked()
                    {
                        queue.push(Command::ApplyMaskOp(op));
                        ui.close_menu();
                    }
                }
                ui.separator();
                for side in ExtrudeSide::ALL {
                    let label = format!("{} — {}", s.action_extrude, side.label());
                    if ui
                        .add_enabled(state.mask.is_active(), egui::Button::new(label))
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
            ui.menu_button(s.menu_window, |_| {});
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
            let title = if state.modified {
                format!("{} • {}", state.document_name, state.strings.state_unsaved)
            } else {
                state.document_name.to_string()
            };
            ui.label(
                egui::RichText::new(title)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
        });
    });
}

/// The tool options bar: the active brush's primary parameters, always visible.
pub fn options_bar(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.add_space(space::SNUG);
    ui.horizontal(|ui| {
        ui.add_space(space::PANEL);
        ui.vertical(|ui| {
            ui.set_width(180.0);
            if let Some(value) = slider(ui, s.label_intensity, state.brush.intensity, 0.0..=1.0, 2)
            {
                queue.push(Command::SetBrushIntensity(value));
            }
        });
        ui.add_space(space::SECTION);
        ui.vertical(|ui| {
            ui.set_width(180.0);
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
        ui.add_space(space::SECTION);
        ui.vertical(|ui| {
            ui.set_width(180.0);
            if let Some(value) = slider(ui, s.label_flow, state.brush.flow, 0.01..=1.0, 2) {
                queue.push(Command::SetBrushFlow(value));
            }
        });

        // The combine vocabulary is the SDF side's alone: cells are set or
        // cleared and vertices are moved, so neither has a join to make. The
        // controls are absent rather than greyed because there is no
        // representation-independent meaning for them to be disabled *from*.
        if state.representation == Representation::Sdf {
            ui.add_space(space::SECTION);
            combine_controls(ui, state, queue);
        }

        ui.add_space(space::SECTION);
        alpha_control(ui, state, queue);

        // Why the tool cannot be used, where the user is looking when they try.
        if let Some(reason) = state.tool_status {
            // The ViewModel carries no locale, so a status it raises itself
            // arrives as a marker and is localised here. An engine refusal is
            // already a sentence and passes through as one.
            let reason = if reason == clayspace_vm::TOOL_SUBSTITUTED {
                state.strings.tool_substituted
            } else {
                reason
            };
            ui.add_space(space::SECTION);
            ui.label(
                egui::RichText::new(reason)
                    .size(type_scale::LABEL)
                    .color(Tokens::accent()),
            );
        }
    });
}

/// The combine operation, its join profile, and how wide the join reaches.
///
/// Three controls rather than one list of their product: an operation with a
/// hard join and the same one rounded are the same operation and different
/// shapes, and a list of seventy entries is not a vocabulary anybody learns.
fn combine_controls(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    let settings = state.combine;

    ui.vertical(|ui| {
        ui.set_width(150.0);
        ui.label(
            egui::RichText::new(s.label_combine)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        egui::ComboBox::from_id_salt("combine-op")
            .selected_text(settings.op.label())
            .width(150.0)
            .show_ui(ui, |ui| {
                for op in Combine::offered_for_strokes() {
                    if ui.selectable_label(op == settings.op, op.label()).clicked()
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
        ui.set_width(140.0);
        ui.label(
            egui::RichText::new(s.label_blend)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        egui::ComboBox::from_id_salt("combine-blend")
            .selected_text(settings.blend.label())
            .width(140.0)
            .show_ui(ui, |ui| {
                for blend in BlendProfile::ALL {
                    if ui
                        .selectable_label(blend == settings.blend, blend.label())
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
        ui.set_width(150.0);
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

/// The alpha stamp: which one is loaded, and whether this brush uses it.
///
/// Where a stamp cannot be used the reason is shown in place of the toggle,
/// rather than the toggle being drawn and doing nothing. That distinction
/// matters more here than for most controls: two of the three representations
/// take a stamp and the third does not, so a sculptor meets the unavailable
/// case by moving between layers and needs to be told which of the two they
/// are in.
fn alpha_control(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    let support = AlphaSupport::of(state.representation, state.combine.op);

    ui.vertical(|ui| {
        ui.set_width(210.0);
        ui.label(
            egui::RichText::new(s.label_alpha)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );

        if !support.accepted() {
            ui.label(
                egui::RichText::new(support.to_string())
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
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

/// The whole-form deformers, and what each one is set to.
///
/// A panel rather than a gesture: a deformer states something about the *form*
/// — no centre, no radius, no falloff — so there is nothing for a drag to be
/// resolved from. The lattice is the third deformer and is absent here for the
/// opposite reason: it *is* a drag, of a cage's control points, and four
/// numbers cannot say what it does.
pub fn deform_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_deform {
        return;
    }
    let s = state.strings;
    let mut open = true;
    let mut settings = state.deform;

    egui::Window::new(s.action_deform)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(300.0);
            if state.representation != Representation::Mesh {
                ui.label(
                    egui::RichText::new(s.deform_mesh_only)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
                return;
            }

            for verb in DeformVerb::ALL {
                let chosen = settings.verb == verb;
                if ui.radio(chosen, verb.label()).clicked() && !chosen {
                    settings.verb = verb;
                    queue.push(Command::SetDeform(settings));
                }
            }

            ui.add_space(space::SNUG);
            if let Some(axis) = axis_control(ui, s.label_axis, settings.axis) {
                queue.push(Command::SetDeform(DeformSettings { axis, ..settings }));
            }

            if let Some(span) = slider(
                ui,
                s.label_span,
                settings.span,
                DeformSettings::SPAN_RANGE,
                2,
            ) {
                queue.push(Command::SetDeform(DeformSettings { span, ..settings }));
            }

            // Only the controls the chosen verb reads. A scale beside a twist
            // is a number that does nothing, and there is no way for the panel
            // to say so that is better than not drawing it.
            if settings.verb.takes_a_scale() {
                if let Some(scale_start) = slider(
                    ui,
                    s.label_scale_start,
                    settings.scale_start,
                    DeformSettings::SCALE_RANGE,
                    2,
                ) {
                    queue.push(Command::SetDeform(DeformSettings {
                        scale_start,
                        ..settings
                    }));
                }
                if let Some(scale_end) = slider(
                    ui,
                    s.label_scale_end,
                    settings.scale_end,
                    DeformSettings::SCALE_RANGE,
                    2,
                ) {
                    queue.push(Command::SetDeform(DeformSettings {
                        scale_end,
                        ..settings
                    }));
                }
            }
            if settings.verb.takes_an_angle() {
                if let Some(degrees) = slider(
                    ui,
                    s.label_angle,
                    settings.degrees,
                    DeformSettings::DEGREES_RANGE,
                    0,
                ) {
                    queue.push(Command::SetDeform(DeformSettings {
                        degrees,
                        ..settings
                    }));
                }
            }

            ui.add_space(space::SECTION);
            if ui.button(settings.verb.label()).clicked() {
                queue.push(Command::RunDeform);
            }
        });
    if !open {
        queue.push(Command::ToggleDeform);
    }
}

/// Three sliders for a direction, returning the axis when one moves.
fn axis_control(ui: &mut egui::Ui, label: &str, axis: [f32; 3]) -> Option<[f32; 3]> {
    ui.label(
        egui::RichText::new(label)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    let mut changed = axis;
    let mut moved = false;
    ui.horizontal(|ui| {
        for (index, name) in ["X", "Y", "Z"].into_iter().enumerate() {
            let mut value = axis[index];
            if ui
                .add(egui::DragValue::new(&mut value).speed(0.02).prefix(name))
                .changed()
            {
                changed[index] = value;
                moved = true;
            }
        }
    });
    moved.then_some(changed)
}

/// The scene tree and the layer stack.
pub fn left_panel(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;

    heading(ui, s.section_scene);
    for node in &state.scene.nodes {
        ui.horizontal(|ui| {
            ui.add_space(space::SNUG + node.depth as f32 * space::ROOMY);
            let selected = state.scene.selected == Some(node.key);
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

    heading(ui, s.section_layers);
    for layer in state.scene.layers.iter().rev() {
        layer_row(ui, state, layer, queue);
    }
    // Recording a pass, and what the stack costs. Under the layer list because
    // that is where the passes themselves are, and only for a grid — a field
    // and a mesh have no stack to record into, and a button that can only
    // refuse is worse than one that is not there.
    if state.representation == Representation::Voxel {
        ui.add_space(space::SNUG);
        sculpt_recording_control(ui, state, queue);
    }

    ui.add_space(space::SNUG);
    if ui.button(format!("+  {}", s.label_new_layer)).clicked() {
        // Layer creation is a document change like any other.
        queue.push(Command::AddLayer);
    }

    heading(ui, s.section_sculpt_settings);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(s.label_symmetry)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        for (index, axis) in Axis::ALL.iter().enumerate() {
            let on = state.symmetry[index];
            let button = egui::Button::new(
                egui::RichText::new(axis.label())
                    .size(type_scale::LABEL)
                    .color(if on {
                        Tokens::text()
                    } else {
                        Tokens::text_dim()
                    }),
            )
            .fill(if on {
                Tokens::raised()
            } else {
                Tokens::panel()
            });
            if ui.add(button).clicked() {
                queue.push(Command::ToggleSymmetry(*axis));
            }
        }
    });
}

fn layer_row(
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

    egui::Frame::new()
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

                let name = egui::RichText::new(&layer.name)
                    .size(type_scale::BODY)
                    // Selection is indicated by surface tone and weight, never
                    // by the accent — that marks the active brush alone.
                    .color(if active {
                        Tokens::text()
                    } else {
                        Tokens::text_dim()
                    });
                // The name gets what is left after the row's right-hand side
                // is reserved, and truncates into it. A label sized to its own
                // text takes the whole row and the tag lands on top of it:
                // "Detalhes_secundario$DF" is what that looks like.
                //
                // Reserved rather than laid out by egui because a
                // right-to-left group inside a horizontal claims the remaining
                // width, which is the whole of it when the name has not been
                // bounded first.
                let width = (ui.available_width() - size::LAYER_ROW_TAIL).max(size::SWATCH);
                if ui
                    .allocate_ui_with_layout(
                        egui::vec2(width, ui.available_height()),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.add(
                                egui::Label::new(name)
                                    .truncate()
                                    .sense(egui::Sense::click()),
                            )
                        },
                    )
                    .inner
                    .clicked()
                {
                    queue.push(Command::SelectLayer(layer.key));
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
fn sculpt_layer_row(
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

/// A byte count in the largest unit that keeps it readable.
fn bytes_label(bytes: usize) -> String {
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

/// Starting and stopping a recording, and what the stack occupies.
fn sculpt_recording_control(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
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

/// The diagnostics report, as a window rather than a panel.
///
/// A window because it is read rarely and copied whole: docking it would cost
/// a permanent strip of the interface for something a person opens twice a
/// year, and then only when something has already gone wrong.
///
/// Every value is a readout the reader can compare against an issue, and the
/// copy button takes the lot. A report that has to be retyped is one that
/// arrives with a digit wrong.
pub fn diagnostics_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_diagnostics {
        return;
    }
    let s = state.strings;
    let mut open = true;
    egui::Window::new(s.action_diagnostics)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(360.0);
            let d = state.diagnostics;

            heading(ui, s.section_diagnostics);
            readout(ui, "Aplicação", d.app_version.clone());
            readout(ui, "Motor", d.engine_version.clone());
            readout(ui, "Revisão", d.engine_revision.clone());
            readout(ui, "Plataforma", d.platform.clone());

            heading(ui, s.label_backend);
            readout(ui, "Disponíveis", d.backends.join(", "));
            readout(
                ui,
                "Ativo",
                format!("{} — {}", d.active_backend, d.selection),
            );
            if let Some(renderer) = &d.renderer {
                readout(ui, "Vídeo", renderer.clone());
            }

            // The stalls, which are what "it stutters" turns into. Listed even
            // when there are none, for the same reason as the fallbacks below.
            if d.stalls.is_empty() {
                readout(ui, "Travamentos", "nenhum acima de um quadro");
            } else {
                for stall in &d.stalls {
                    readout(ui, "Travamento", stall.clone());
                }
            }

            // Fallbacks are listed even when there are none. Silence here reads
            // as "the panel is broken" rather than as "nothing fell back", and
            // a reader cannot tell the two apart.
            if d.fallbacks.is_empty() {
                readout(ui, "Alternativas", "nenhuma nesta sessão");
            } else {
                for fallback in &d.fallbacks {
                    readout(
                        ui,
                        "Alternativa",
                        format!("{} recusou {}", fallback.declined_by, fallback.operation),
                    );
                }
            }

            ui.add_space(space::SNUG);
            ui.horizontal(|ui| {
                if ui.button(s.action_copy).clicked() {
                    queue.push(Command::CopyDiagnostics);
                }
                if state.diagnostics_copied {
                    ui.label(
                        egui::RichText::new(s.state_copied)
                            .size(type_scale::LABEL)
                            .color(Tokens::accent()),
                    );
                }
            });
        });

    // The window's own close button and the menu entry mean the same thing, so
    // they emit the same command rather than each owning a copy of the state.
    if !open {
        queue.push(Command::ToggleDiagnostics);
    }
}

/// What the application is built from, and on what terms.
///
/// Shown rather than only shipped beside the binary: the licence policy in
/// `deny.toml` is written on the understanding that attribution travels with
/// the distribution, and a file nobody can reach from the application is one
/// that goes missing the first time it is repackaged.
pub fn attribution_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_attribution {
        return;
    }
    let mut open = true;
    egui::Window::new(state.strings.action_attribution)
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(520.0, 420.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(
                    egui::RichText::new(state.attribution)
                        .size(type_scale::LABEL)
                        .family(egui::FontFamily::Monospace)
                        .color(Tokens::text_dim()),
                );
            });
        });
    if !open {
        queue.push(Command::ToggleAttribution);
    }
}

/// Bringing geometry in.
///
/// A panel rather than a bare file dialog, because the one real decision —
/// whether the model becomes a reference or becomes clay — cannot be made
/// after the fact, and a native dialog has nowhere to ask it.
/// Pre-bake repair: what is wrong with a grid, and the two verbs that fix it.
///
/// The report comes first and is shown whether or not anything is repaired. A
/// sealed void is invisible until something needs the model to be solid, so a
/// sculptor cannot see the problem by looking — and a repair that ran before
/// saying what it would change would be asking consent for something unstated.
pub fn repair_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_repair {
        return;
    }
    let s = state.strings;
    let mut open = true;
    egui::Window::new(s.action_repair)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(300.0);
            let Some(report) = state.repair else {
                ui.label(
                    egui::RichText::new(s.repair_voxel_only)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
                return;
            };

            if report.airtight {
                ui.label(
                    egui::RichText::new(s.repair_airtight)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
            } else {
                readout(ui, s.repair_voids, thousands(report.enclosed_voids));
                readout(ui, s.repair_largest, thousands(report.largest_void));
            }

            ui.add_space(space::SECTION);
            if ui.button(s.repair_close_holes).clicked() {
                queue.push(Command::CloseHoles);
            }
            // Offered only where there is something to fill: a button that can
            // only report having done nothing is worse than one that is not
            // there.
            if !report.airtight && ui.button(s.repair_fill_voids).clicked() {
                queue.push(Command::FillVoids);
            }
        });
    if !open {
        queue.push(Command::ToggleRepair);
    }
}

/// The conversion panel: where a layer crosses to another representation.
///
/// Its whole job is to state the losses *before* the crossing runs. They are
/// recomputed from the chosen cell size every frame rather than written into
/// the strings, because a number written down is wrong the first time somebody
/// changes the default — and the resolution is the whole of the decision being
/// made here.
pub fn convert_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_convert {
        return;
    }
    let s = state.strings;
    let mut open = true;
    let mut settings = state.conversion;
    let available = Direction::from_representation(state.representation);

    egui::Window::new(s.action_convert)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            if available.is_empty() {
                ui.label(
                    egui::RichText::new(s.convert_none_here)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
                return;
            }

            ui.label(
                egui::RichText::new(s.label_convert_to)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            for direction in &available {
                let chosen = settings.direction == *direction;
                if ui.radio(chosen, direction.to().label()).clicked() && !chosen {
                    settings.direction = *direction;
                    queue.push(Command::SetConversion(settings));
                }
            }

            if settings.direction.chooses_resolution() {
                ui.add_space(space::SNUG);
                ui.label(
                    egui::RichText::new(s.label_cell_size)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
                let mut cell = settings.cell_size;
                if ui
                    .add(
                        egui::Slider::new(
                            &mut cell,
                            clayspace_model::ConversionSettings::CELL_RANGE,
                        )
                        .logarithmic(true)
                        .show_value(true),
                    )
                    .changed()
                {
                    settings.cell_size = cell;
                    queue.push(Command::SetConversion(settings));
                }
            }

            ui.add_space(space::SECTION);
            ui.label(
                egui::RichText::new(s.label_convert_costs)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            for line in cost_lines(state, settings) {
                ui.label(
                    egui::RichText::new(line)
                        .size(type_scale::LABEL)
                        .color(Tokens::text_dim()),
                );
            }
            // The one the specification asks for by name: a sculptor must not
            // discover after the fact that the crossing cannot be undone.
            ui.label(
                egui::RichText::new(s.convert_not_undoable)
                    .size(type_scale::LABEL)
                    .color(Tokens::accent()),
            );

            ui.add_space(space::SECTION);
            if ui.button(s.convert_run).clicked() {
                queue.push(Command::RunConversion);
            }
        });
    if !open {
        queue.push(Command::ToggleConvert);
    }
}

/// What the crossing costs, in the units the document is measured in.
///
/// Built from [`clayspace_model::Cost`] rather than from prose, so the figures
/// and the settings above them cannot disagree.
fn cost_lines(
    state: &ShellState<'_>,
    settings: clayspace_model::ConversionSettings,
) -> Vec<String> {
    let s = state.strings;
    let Some(cost) = state.conversion_cost else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if settings.direction.chooses_resolution() {
        lines.push(format!(
            "· {} {}",
            s.convert_surface_moves,
            state.units.format(cost.surface_movement)
        ));
        lines.push(format!(
            "· {} ({})",
            s.convert_features_vanish,
            state.units.format(cost.vanishing_feature)
        ));
        // Grouped, like the geometry readout: "1000000 cells" is a number a
        // reader has to count the digits of before it means anything.
        lines.push(format!(
            "· {} {}",
            thousands(cost.cells as usize),
            s.convert_cells
        ));
    }
    if !cost.keeps_sharp_edges {
        lines.push(format!("· {}", s.convert_sharp_edges_lost));
    }
    if !cost.keeps_history {
        lines.push(format!("· {}", s.convert_history_lost));
    }
    lines
}

pub fn import_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_import {
        return;
    }
    let s = state.strings;
    let mut open = true;
    let mut settings = state.import;
    egui::Window::new(s.action_import)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(320.0);
            ui.label(
                egui::RichText::new(s.label_import_as)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            for becomes in ImportAs::ALL {
                if ui
                    .radio(settings.becomes == becomes, becomes.label())
                    .on_hover_text(becomes.detail())
                    .clicked()
                {
                    settings.becomes = becomes;
                }
            }
            if let Some(value) = slider(ui, s.label_scale, settings.scale, 0.01..=100.0, 2) {
                settings.scale = value;
            }
            if settings != state.import {
                queue.push(Command::SetImportSettings(settings));
            }
            ui.add_space(space::SNUG);
            if ui.button(s.action_choose_file).clicked() {
                queue.push(Command::RunImport);
            }
        });
    if !open {
        queue.push(Command::ToggleImport);
    }
}

/// Writing geometry out, and saying beforehand what will not survive.
pub fn export_window(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !state.show_export {
        return;
    }
    let s = state.strings;
    let mut open = true;
    let mut settings = state.export;
    egui::Window::new(s.action_export)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.label(
                egui::RichText::new(s.label_mesher)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            for mesher in ExportMesher::ALL {
                let response = ui.radio(settings.mesher == mesher, mesher.label());
                let response = match mesher.caveat() {
                    Some(caveat) => response.on_hover_text(caveat),
                    None => response,
                };
                if response.clicked() {
                    settings.mesher = mesher;
                }
            }
            if let Some(value) = slider(
                ui,
                s.label_export_resolution,
                settings.resolution,
                0.005..=0.2,
                3,
            ) {
                settings.resolution = value;
            }

            // Decimation is off by default and expressed as a ratio, so the
            // checkbox and the slider are one control: unticking it means
            // "keep every triangle" rather than "keep 100% of them", which is
            // the same file by a slower route.
            let mut decimating = settings.decimate_to.is_some();
            if ui.checkbox(&mut decimating, s.label_decimate).clicked() {
                settings.decimate_to = decimating.then_some(0.5);
            }
            if let Some(ratio) = settings.decimate_to {
                // "Manter", not "Reduzir" again: the value is the share of
                // triangles kept, and labelling both the checkbox and the
                // slider the same way reads as one control repeated.
                if let Some(value) = slider(ui, s.label_keep, ratio, 0.05..=0.95, 2) {
                    settings.decimate_to = Some(value);
                }
            }
            if settings != state.export {
                queue.push(Command::SetExportSettings(settings));
            }

            // Before the write, not after. Every one of these is knowable now
            // and otherwise found out by opening the file somewhere else.
            if !state.export_warnings.is_empty() {
                heading(ui, s.section_warnings);
                for warning in state.export_warnings {
                    ui.label(
                        egui::RichText::new(&warning.message)
                            .size(type_scale::LABEL)
                            .color(Tokens::accent()),
                    );
                }
            }

            ui.add_space(space::SNUG);
            if ui.button(s.action_choose_file).clicked() {
                queue.push(Command::RunExport);
            }
        });
    if !open {
        queue.push(Command::ToggleExport);
    }
}

/// Material, geometry, resolution and brush controls.
pub fn right_panel(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;

    heading(ui, s.section_material);
    ui.horizontal(|ui| {
        // The material preview: a shaded sphere, which is where the design
        // spends its skeuomorphic budget.
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(size::SWATCH, size::SWATCH), egui::Sense::click());
        paint_sphere(ui, rect, Tokens::text_dim(), false);
        if response.clicked() {
            queue.push(Command::NextMaterial);
        }
        ui.add_space(space::SNUG);
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(state.material)
                    .size(type_scale::BODY)
                    .color(Tokens::text()),
            );
            ui.label(
                egui::RichText::new(format!("{} materiais", state.materials.len()))
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
        });
    });

    heading(ui, s.section_geometry);
    // A count without its detail level reads as a smaller model, so where the
    // viewport is not showing full resolution the interface says so.
    if let Some(note) = state.stats.detail.note() {
        ui.label(
            egui::RichText::new(note)
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }
    readout(ui, s.label_polygons, thousands(state.stats.triangles));
    readout(ui, s.label_vertices, thousands(state.stats.vertices));
    readout(ui, s.label_triangles, thousands(state.stats.triangles));
    readout(ui, s.label_objects, format!("{}", state.stats.objects));

    if state.armature.exists {
        heading(ui, s.section_armature);
        readout(ui, s.label_spheres, format!("{}", state.armature.spheres));
        if let Some(value) = slider(ui, s.label_skin, state.armature.skin, 0.5..=3.0, 2) {
            queue.push(Command::SetSkinThickness(value));
        }
        let mut mirror = state.armature.mirror;
        if ui.checkbox(&mut mirror, s.label_mirror_new).clicked() {
            queue.push(Command::SetArmatureMirror(mirror));
        }
        if state.armature.editing {
            // The gestures, where a person is when they need them. ZBrush
            // teaches these by tutorial; one line costs nothing.
            ui.label(
                egui::RichText::new(s.hint_armature)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
            if ui
                .add_enabled(
                    state.armature.selection,
                    egui::Button::new(s.action_armature_remove),
                )
                .clicked()
            {
                queue.push(Command::RemoveZsphere);
            }
        }
    }

    heading(ui, s.section_brush_controls);
    if let Some(value) = slider(ui, s.label_noise, state.brush.shaping.noise, 0.0..=1.0, 2) {
        queue.push(Command::SetBrushNoise(value));
    }
    ui.label(
        egui::RichText::new(s.label_edge)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    ui.horizontal_wrapped(|ui| {
        for falloff in Falloff::ALL {
            let on = state.brush.shaping.falloff == falloff;
            let button = egui::Button::new(
                egui::RichText::new(falloff.label())
                    .size(type_scale::LABEL)
                    .color(if on {
                        Tokens::text()
                    } else {
                        Tokens::text_dim()
                    }),
            )
            .fill(if on {
                Tokens::raised()
            } else {
                Tokens::panel()
            });
            if ui.add(button).clicked() {
                queue.push(Command::SetBrushFalloff(falloff));
            }
        }
    });

    let mut accumulate = state.brush.shaping.accumulate;
    if ui.checkbox(&mut accumulate, s.label_accumulate).changed() {
        queue.push(Command::SetBrushAccumulate(accumulate));
    }
    if let Some(value) = slider(
        ui,
        s.label_smoothing,
        state.brush.shaping.smoothing,
        0.0..=0.95,
        2,
    ) {
        queue.push(Command::SetBrushSmoothing(value));
    }
}

/// The brush shelf: every tool, with the active one accented.
/// The brush shelf, holding the verbs the active representation has.
///
/// Not every tool with the ones that do not apply greyed out. Three
/// representations carry substantially different vocabularies, so a single
/// list would be mostly disabled entries whatever the active layer, every one
/// of them saying the same thing — absence carries that better than a greyed
/// row does. A tool that *has* a verb here and cannot be used right now is
/// still shown; that is a different sentence and worth the space.
pub fn brush_shelf(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let tools = ToolKind::for_representation(state.representation);
    if tools.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(space::PANEL);
            ui.label(
                egui::RichText::new(state.strings.shelf_no_tools)
                    .size(type_scale::LABEL)
                    .color(Tokens::text_dim()),
            );
        });
        return;
    }
    ui.horizontal(|ui| {
        ui.add_space(space::PANEL);
        for tool in tools {
            let active = state.tool == tool;
            ui.vertical(|ui| {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(size::SWATCH, size::SWATCH),
                    egui::Sense::click(),
                );
                paint_sphere(ui, rect, Tokens::text_dim(), active);
                ui.label(
                    egui::RichText::new(tool.label())
                        .size(type_scale::LABEL)
                        // The accent, on the active brush and nowhere else.
                        .color(if active {
                            Tokens::accent()
                        } else {
                            Tokens::text_dim()
                        }),
                );
                if response.clicked() {
                    queue.push(Command::SelectTool(tool));
                }
            });
            ui.add_space(space::SNUG);
        }
    });
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

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(space::PANEL);
            // A control, not a label: the design shows the unit here, and a
            // person looking for where to change it looks where it is shown.
            let shown = format!("{}: {}", s.label_units, state.units.display.label());
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

/// The view presets, under the viewport as the design places them.
pub fn viewport_bar(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    ui.horizontal(|ui| {
        ui.add_space(space::PANEL);
        for preset in ViewPresetKind::ALL {
            let on = state.view_preset == preset;
            let button = egui::Button::new(
                egui::RichText::new(preset.label())
                    .size(type_scale::LABEL)
                    .color(if on {
                        Tokens::text()
                    } else {
                        Tokens::text_dim()
                    }),
            )
            .fill(if on {
                Tokens::raised()
            } else {
                Tokens::ground()
            });
            if ui.add(button).clicked() {
                queue.push(Command::SetViewPreset(preset));
            }
        }
        // The active layer's representation, at the far end of the bar. The
        // shelf's contents follow it, so it has to be visible without opening
        // a panel or a shelf that changed under you is unexplained.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(space::PANEL);
            ui.label(
                egui::RichText::new(format!(
                    "{}: {}",
                    state.strings.representation_label,
                    state.representation.label()
                ))
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
            );
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

/// Paints a shaded sphere: the one place the design spends skeuomorphism.
fn paint_sphere(ui: &egui::Ui, rect: egui::Rect, tint: egui::Color32, active: bool) {
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
fn thousands(value: usize) -> String {
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
fn gigabytes(bytes: u64) -> String {
    format!("{:.2} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
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
        let height =
            800.0 - region::MENU_BAR - region::OPTIONS_BAR - region::SHELF - region::STATUS;
        assert!(width > 400.0, "the viewport would be {width} wide");
        assert!(height > 300.0, "the viewport would be {height} tall");
    }
}
