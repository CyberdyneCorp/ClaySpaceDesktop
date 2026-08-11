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
    BrushSettings, Falloff, LayerSummary, Scene, SceneStats, ToolKind, ViewPresetKind,
};
use clayspace_vm::{Axis, Command, CommandQueue};

use crate::design::{size, space, type_scale, Tokens};
use crate::icons::{self, Icon};
use crate::strings::Strings;

/// Everything a frame of interface needs to read.
///
/// Assembled by the composition root from the ViewModels. Passing one struct
/// rather than a dozen arguments keeps a View function's signature honest
/// about being a function of state.
pub struct ShellState<'a> {
    pub strings: &'a Strings,
    pub document_name: &'a str,
    pub modified: bool,

    pub tool: ToolKind,
    pub brush: BrushSettings,
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
    pub units: &'a str,
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

/// The menu bar. Every item dispatches the same command its shortcut does.
pub fn menu_bar(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    ui.horizontal(|ui| {
        ui.add_space(space::SNUG);
        egui::menu::bar(ui, |ui| {
            ui.menu_button(s.menu_file, |_| {});
            ui.menu_button(s.menu_edit, |ui| {
                if ui
                    .add_enabled(state.can_undo, egui::Button::new(s.action_undo))
                    .clicked()
                {
                    queue.push(Command::Undo);
                    ui.close_menu();
                }
                if ui
                    .add_enabled(state.can_redo, egui::Button::new(s.action_redo))
                    .clicked()
                {
                    queue.push(Command::Redo);
                    ui.close_menu();
                }
            });
            ui.menu_button(s.menu_view, |ui| {
                for preset in ViewPresetKind::ALL {
                    if ui.button(preset.label()).clicked() {
                        queue.push(Command::SetViewPreset(preset));
                        ui.close_menu();
                    }
                }
                if ui.button(s.action_frame_all).clicked() {
                    queue.push(Command::FrameAll);
                    ui.close_menu();
                }
            });
            ui.menu_button(s.menu_sculpt, |_| {});
            ui.menu_button(s.menu_brushes, |ui| {
                for tool in ToolKind::ALL {
                    if ui.button(tool.label()).clicked() {
                        queue.push(Command::SelectTool(tool));
                        ui.close_menu();
                    }
                }
            });
            ui.menu_button(s.menu_dynamics, |_| {});
            ui.menu_button(s.menu_masks, |_| {});
            ui.menu_button(s.menu_window, |_| {});
            ui.menu_button(s.menu_help, |_| {});
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
            if let Some(value) = slider(ui, s.label_size, state.brush.size, 0.005..=1.0, 3) {
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

        // Why the tool cannot be used, where the user is looking when they try.
        if let Some(reason) = state.tool_status {
            ui.add_space(space::SECTION);
            ui.label(
                egui::RichText::new(reason)
                    .size(type_scale::LABEL)
                    .color(Tokens::accent()),
            );
        }
    });
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
                if ui
                    .add(egui::Label::new(name).sense(egui::Sense::click()))
                    .clicked()
                {
                    queue.push(Command::SelectLayer(layer.key));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    numeric(ui, format!("{:>3}", layer.intensity));
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
    ui.add_space(space::HAIR);
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
pub fn brush_shelf(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    ui.horizontal(|ui| {
        ui.add_space(space::PANEL);
        for tool in ToolKind::ALL {
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
pub fn status_bar(ui: &mut egui::Ui, state: &ShellState<'_>) {
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
            numeric(ui, format!("{}: {}", s.label_units, state.units));
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
    });
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
