//! The interface shell, rendered and captured.
//!
//! egui is drawn into an offscreen target exactly as it would be into a
//! window, so the panels, the brush shelf and the status area can be looked at
//! rather than only asserted about.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_shell
//! open target/visual
//! ```

mod support;

use clayspace_model::{
    BrushSettings, LayerKey, LayerSummary, Protection, Representation, Scene, SceneNode,
    SceneStats, ToolKind, ViewPresetKind,
};
use clayspace_view::shell::{self, region, ShellState};
use clayspace_view::{Locale, OffscreenTarget, Strings, Tokens};
use clayspace_vm::{Command, CommandQueue};
use support::Harness;

/// A scene with enough in it to fill the panels.
fn scene() -> Scene {
    let layer =
        |id: u64, name: &str, intensity: u8, visible: bool, protection: Protection| LayerSummary {
            key: LayerKey(id),
            name: name.to_string(),
            representation: Representation::Sdf,
            visible,
            protection,
            intensity,
            sculpt_layers: Vec::new(),
        };
    Scene {
        nodes: vec![
            SceneNode {
                key: LayerKey(0),
                name: "Cabeça_Estudo".into(),
                depth: 0,
                visible: true,
                expandable: true,
            },
            SceneNode {
                key: LayerKey(1),
                name: "Cabeça".into(),
                depth: 1,
                visible: true,
                expandable: true,
            },
            SceneNode {
                key: LayerKey(2),
                name: "Pescoço".into(),
                depth: 2,
                visible: true,
                expandable: false,
            },
            SceneNode {
                key: LayerKey(3),
                name: "Torso".into(),
                depth: 2,
                visible: true,
                expandable: false,
            },
            SceneNode {
                key: LayerKey(4),
                name: "Base".into(),
                depth: 2,
                visible: false,
                expandable: false,
            },
        ],
        layers: vec![
            layer(10, "Base", 100, true, Protection::default()),
            layer(11, "Forma_principal", 100, true, Protection::default()),
            layer(
                12,
                "Poros",
                70,
                true,
                Protection {
                    ghost: false,
                    locked: true,
                },
            ),
            layer(
                13,
                "Detalhes_secundarios",
                100,
                false,
                Protection::default(),
            ),
        ],
        active: Some(LayerKey(11)),
        selected: Some(LayerKey(1)),
    }
}

/// Documents the file menu offers to reopen.
static RECENT: &[std::path::PathBuf] = &[];

/// What the export panel would warn about.
static WARNINGS: &[clayspace_model::ExportWarning] = &[];

/// The report the diagnostics window shows, with a fallback in it so the
/// interesting branch is the one captured.
fn diagnostics() -> clayspace_model::Diagnostics {
    clayspace_model::Diagnostics {
        app_version: "ClaySpaceDesktop 0.1.0".into(),
        engine_version: "claycore 0.27.3".into(),
        engine_revision: "v0.27.3-0-g804fc9d".into(),
        platform: "macos aarch64".into(),
        backends: vec!["cpu".into(), "metal".into()],
        active_backend: "metal".into(),
        selection: "escolha automática".into(),
        fallbacks: vec![clayspace_model::Fallback {
            operation: "raycast".into(),
            declined_by: "metal".into(),
        }],
        renderer: Some("Apple M3 Max — Metal".into()),
        stalls: vec!["consolidar 6400 ms".into(), "re-malha 45 ms (×12)".into()],
    }
}

/// The default bindings, so the menus render the chords they advertise.
///
/// A single shared table rather than one per call: `ShellState` borrows it,
/// and a temporary would not outlive the state that holds it.
fn shortcuts() -> &'static clayspace_view::Shortcuts {
    static SHORTCUTS: std::sync::OnceLock<clayspace_view::Shortcuts> = std::sync::OnceLock::new();
    SHORTCUTS.get_or_init(clayspace_view::Shortcuts::default)
}

fn state<'a>(
    strings: &'a Strings,
    scene: &'a Scene,
    materials: &'a [&'a str],
    diagnostics: &'a clayspace_model::Diagnostics,
) -> ShellState<'a> {
    ShellState {
        shortcuts: shortcuts(),
        representation: clayspace_model::Representation::Sdf,
        show_repair: false,
        repair: None,
        show_convert: false,
        conversion: clayspace_model::ConversionSettings::default(),
        conversion_cost: None,
        // A mask with something in it, so the menu's enabled state is what the
        // capture shows rather than a row of grey.
        mask: clayspace_model::MaskState {
            present: true,
            painted_cells: 4096,
        },
        extrude: clayspace_model::ExtrudeSettings::default(),
        mask_steps: 1,
        curve: clayspace_model::CurveState::default(),
        curve_radius: 0.12,
        voxel_display: clayspace_model::VoxelDisplay::default(),
        voxel_blur: clayspace_model::SmoothBlur::default(),
        lattice: clayspace_model::LatticeState::default(),
        lattice_divisions: [3; 3],
        // A rig, mid-edit, so the capture shows the armature section and the
        // menu entries that depend on it rather than a row of grey.
        armature: clayspace_view::ArmatureState {
            exists: true,
            editing: true,
            selection: true,
            skin_preview: true,
            selection_is_negative: false,
            spheres: 12,
            mirror: true,
            skin: 1.0,
        },
        strings,
        document_name: "Cabeça_Estudo_v03",
        modified: true,
        tool: ToolKind::Padrao,
        brush: BrushSettings::default(),
        combine: clayspace_model::CombineSettings::for_strokes(),
        alpha: None,
        sculpt_cost: clayspace_model::SculptLayerCost::default(),
        show_deform: false,
        deform: clayspace_model::DeformSettings::default(),
        tool_status: None,
        symmetry: [true, false, false],
        scene,
        renaming: None,
        polyframe: false,
        stats: SceneStats {
            triangles: 2_356_789,
            vertices: 1_178_394,
            objects: 5,
            detail: clayspace_model::Detail::Full,
        },
        view_preset: ViewPresetKind::Perspective,
        material: "MatCap Cinza 01",
        materials,
        can_undo: true,
        can_redo: false,
        memory: (1_331_439_861, 4 * 1024 * 1024 * 1024),
        backend: "metal",
        units: clayspace_model::Units::default(),
        last_action: Some(("Padrão", true)),
        recent: RECENT,
        show_import: false,
        show_export: false,
        import: clayspace_model::ImportSettings::default(),
        export: clayspace_model::ExportSettings::default(),
        export_warnings: WARNINGS,
        diagnostics,
        show_diagnostics: false,
        diagnostics_copied: false,
        attribution: "# Attribution\n\n| ab_glyph | 0.2.32 | Apache-2.0 |\n",
        show_attribution: false,
    }
}

/// The window size the design specifies.
///
/// The mesh captures use a small target because they are looking at a
/// silhouette; an interface has to be captured at the size it was designed
/// for, or the panels consume the whole width and there is no viewport left
/// to tell them apart from.
const SHELL_WIDTH: u32 = 1280;
const SHELL_HEIGHT: u32 = 800;

/// Lays the whole shell out in one egui frame.
///
/// One definition, so a capture and a probe of where something landed are
/// looking at the same interface rather than at two arrangements that happen
/// to agree today.
fn build_shell(ctx: &egui::Context, state: &ShellState<'_>, queue: &mut CommandQueue) {
    egui::TopBottomPanel::top("menu")
        .exact_height(region::MENU_BAR)
        .show(ctx, |ui| shell::menu_bar(ui, state, queue));
    egui::TopBottomPanel::top("options")
        .exact_height(region::OPTIONS_BAR)
        .show(ctx, |ui| shell::options_bar(ui, state, queue));
    egui::TopBottomPanel::bottom("status")
        .exact_height(region::STATUS)
        .show(ctx, |ui| shell::status_bar(ui, state, queue));
    egui::TopBottomPanel::bottom("shelf")
        .exact_height(region::SHELF)
        .show(ctx, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| shell::brush_shelf(ui, state, queue));
        });
    egui::SidePanel::left("left")
        .exact_width(region::LEFT)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| shell::left_panel(ui, state, queue));
        });
    egui::SidePanel::right("right")
        .exact_width(region::RIGHT)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| shell::right_panel(ui, state, queue));
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(Tokens::ground()))
        .show(ctx, |ui| {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                shell::viewport_bar(ui, state, queue);
            });
        });
    shell::diagnostics_window(ctx, state, queue);
    shell::attribution_window(ctx, state, queue);
    shell::convert_window(ctx, state, queue);
    shell::repair_window(ctx, state, queue);
    shell::import_window(ctx, state, queue);
    shell::export_window(ctx, state, queue);
}

/// Runs the shell without capturing it, so a test can ask where a widget went.
///
/// Two passes for the same reason the capture takes two: a scroll area and an
/// area both measure before they place anything.
fn probe_shell(state: &ShellState<'_>) -> egui::Context {
    let ctx = egui::Context::default();
    shell::apply_theme(&ctx);
    let mut queue = CommandQueue::new();
    for _ in 0..2 {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(SHELL_WIDTH as f32, SHELL_HEIGHT as f32),
                )),
                ..Default::default()
            },
            |ctx| build_shell(ctx, state, &mut queue),
        );
    }
    ctx
}

/// Draws the whole shell into one egui frame and returns the captured image.
fn capture_shell(harness: &Harness, state: &ShellState<'_>, name: &str) -> clayspace_view::Image {
    capture_shell_after(harness, state, name, &[], |queue| {
        // Nothing was clicked, so the interface must not have emitted
        // anything: commands are the only channel out of a View.
        assert!(
            queue.is_empty(),
            "drawing the interface emitted {} commands without any input",
            queue.len()
        );
    })
}

/// The same, with pointer events delivered before the captured frame.
///
/// One entry per frame of input, because a menu takes two gestures: the
/// right-click that opens it and the click that chooses an entry, and an entry
/// cannot be clicked on the frame the menu is still measuring. `inspect` sees
/// the commands the input produced, which is the other half of what such a
/// test is for — a menu that draws and is wired to nothing looks identical.
fn capture_shell_after(
    harness: &Harness,
    state: &ShellState<'_>,
    name: &str,
    frames: &[Vec<egui::Event>],
    inspect: impl FnOnce(&CommandQueue),
) -> clayspace_view::Image {
    let ctx = egui::Context::default();
    shell::apply_theme(&ctx);

    let mut queue = CommandQueue::new();
    let raw_input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(SHELL_WIDTH as f32, SHELL_HEIGHT as f32),
        )),
        ..Default::default()
    };

    let mut build = |ctx: &egui::Context| build_shell(ctx, state, &mut queue);

    // Two passes, not one. An auto-sized `egui::Area` — which is what a window
    // is — spends its first frame measuring and paints nothing, so a
    // single-pass capture of the diagnostics window came back byte-identical
    // to one with the window closed. The panels do not need this; the window
    // does, and one capture path is better than two.
    let first = ctx.run(raw_input(), &mut build);
    let mut output = ctx.run(raw_input(), &mut build);

    for events in frames {
        // The frame the input lands on, and then the frame that draws what it
        // opened — a menu is an `Area` and measures before it paints, exactly
        // as a window does.
        let _ = ctx.run(
            egui::RawInput {
                events: events.clone(),
                ..raw_input()
            },
            &mut build,
        );
        let _ = ctx.run(raw_input(), &mut build);
        output = ctx.run(raw_input(), &mut build);
    }

    let target = OffscreenTarget::new(&harness.gpu, SHELL_WIDTH, SHELL_HEIGHT);
    // The font atlas arrives in the first pass's deltas, so both are applied
    // and only the second is tessellated.
    let image = render_egui(harness, &ctx, [first, output], &target);
    support::save(&image, name);
    // After the capture is written, so a failing expectation still leaves the
    // picture that explains it.
    inspect(&queue);
    image
}

/// Paints one egui frame into an offscreen target and reads it back.
fn render_egui(
    harness: &Harness,
    ctx: &egui::Context,
    passes: [egui::FullOutput; 2],
    target: &OffscreenTarget,
) -> clayspace_view::Image {
    let mut renderer =
        egui_wgpu::Renderer::new(&harness.gpu.device, OffscreenTarget::FORMAT, None, 1, false);

    let pixels_per_point = ctx.pixels_per_point();
    let [first, output] = passes;
    // Every pass's textures, only the last pass's shapes.
    for pass in [&first, &output] {
        for (id, delta) in &pass.textures_delta.set {
            renderer.update_texture(&harness.gpu.device, &harness.gpu.queue, *id, delta);
        }
    }
    let primitives = ctx.tessellate(output.shapes, pixels_per_point);

    let descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [target.width(), target.height()],
        pixels_per_point,
    };
    let mut encoder = harness
        .gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("egui"),
        });
    renderer.update_buffers(
        &harness.gpu.device,
        &harness.gpu.queue,
        &mut encoder,
        &primitives,
        &descriptor,
    );

    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("egui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0168,
                        g: 0.0194,
                        b: 0.0242,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        renderer.render(&mut pass.forget_lifetime(), &primitives, &descriptor);
    }
    harness.gpu.queue.submit(Some(encoder.finish()));

    target.read_back_public(&harness.gpu)
}

#[test]
fn the_shell_draws_every_region() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01", "MatCap Cinza 02", "Gesso"];
    let report = diagnostics();
    let state = state(strings, &scene, &materials, &report);

    let image = capture_shell(&harness, &state, "60-shell-pt-br");

    // Each region must have drawn something: a panel that renders as bare
    // ground is a panel that failed to lay out.
    let ground = image.pixel(image.width / 2, image.height / 2);
    let sample = |x: u32, y: u32| image.pixel(x, y);

    let left = sample(20, image.height / 2);
    let right = sample(image.width - 20, image.height / 2);
    let top = sample(image.width / 2, 8);
    let bottom = sample(image.width / 2, image.height - 8);

    for (name, pixel) in [
        ("left panel", left),
        ("right panel", right),
        ("menu bar", top),
        ("status area", bottom),
    ] {
        assert!(
            pixel != ground,
            "the {name} rendered as bare viewport ground, so it did not lay out"
        );
    }
}

#[test]
fn the_interface_is_readable_where_it_is_quiet() {
    // The tokens are checked in unit tests; this checks the rendered result,
    // where a theme that failed to apply would show as text on the wrong
    // ground.
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();
    let state = state(strings, &scene, &materials, &report);
    let image = capture_shell(&harness, &state, "61-shell-contrast");

    // Panels must be distinguishable from the viewport ground.
    let panel = image.pixel(20, image.height / 2);
    let viewport = image.pixel(image.width / 2, image.height / 2);
    let difference: i32 = (0..3)
        .map(|i| (panel[i] as i32 - viewport[i] as i32).abs())
        .sum();
    assert!(
        difference > 4,
        "panels and the viewport ground are indistinguishable ({panel:?} vs {viewport:?})"
    );
}

#[test]
fn the_shell_renders_in_every_locale() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let scene = scene();
    let materials = ["MatCap Cinza 01"];

    let mut captured = Vec::new();
    for locale in Locale::ALL {
        let strings = Strings::for_locale(locale);
        let report = diagnostics();
        let state = state(strings, &scene, &materials, &report);
        let name = format!("62-shell-{:?}", locale).to_lowercase();
        captured.push((locale, capture_shell(&harness, &state, &name)));
    }

    // A locale whose labels are longer must not blank the interface or push a
    // region off screen; the frames differ but all of them draw.
    for (index, (first_locale, first)) in captured.iter().enumerate() {
        for (second_locale, second) in &captured[index + 1..] {
            assert!(
                first.mean_difference(second) > 0.1,
                "{} and {} rendered identically, so the strings are not reaching the interface",
                first_locale.label(),
                second_locale.label()
            );
        }
    }
}

#[test]
fn the_active_tool_is_the_only_thing_wearing_the_accent() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];

    let report = diagnostics();
    let mut first = state(strings, &scene, &materials, &report);
    first.tool = ToolKind::Padrao;
    let a = capture_shell(&harness, &first, "63-accent-padrao");

    let report = diagnostics();
    let mut second = state(strings, &scene, &materials, &report);
    second.tool = ToolKind::Suavizar;
    let b = capture_shell(&harness, &second, "63-accent-suavizar");

    // Changing which tool is active must move the accent, and the amount of
    // accent on screen must stay about the same — it marks one thing.
    let accent = Tokens::accent();
    let count = |image: &clayspace_view::Image| {
        let mut n = 0usize;
        for y in 0..image.height {
            for x in 0..image.width {
                let p = image.pixel(x, y);
                if p[0].abs_diff(accent.r()) < 24
                    && p[1].abs_diff(accent.g()) < 24
                    && p[2].abs_diff(accent.b()) < 24
                {
                    n += 1;
                }
            }
        }
        n
    };

    let (first_count, second_count) = (count(&a), count(&b));
    assert!(first_count > 0, "the active tool was not accented at all");
    let drift = first_count.abs_diff(second_count) as f64 / first_count as f64;
    assert!(
        drift < 0.5,
        "the accent covers {first_count} pixels for one tool and {second_count} for another; \
         it should mark exactly one thing either way"
    );

    // The accent ring is a few hundred pixels in a million, so a mean over the
    // whole frame says nothing. Count the pixels that changed in the shelf,
    // which is where the accent lives.
    let shelf_top = a.height - 130;
    let mut moved = 0usize;
    for y in shelf_top..a.height {
        for x in 0..a.width {
            if a.pixel(x, y) != b.pixel(x, y) {
                moved += 1;
            }
        }
    }
    assert!(
        moved > 200,
        "changing the active tool moved only {moved} pixels in the brush shelf"
    );
}

#[test]
fn the_diagnostics_window_carries_what_an_issue_needs() {
    // Captured because this is the one panel whose job is to be *read* by
    // someone who is already having a bad day. If the revision wraps or the
    // fallback line runs off the edge, an assertion will not notice.
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();

    let mut open = state(strings, &scene, &materials, &report);
    open.show_diagnostics = true;
    let shown = capture_shell(&harness, &open, "64-diagnostics");

    let closed = state(strings, &scene, &materials, &report);
    let hidden = capture_shell(&harness, &closed, "64-diagnostics-closed");

    let changed = (0..shown.height)
        .flat_map(|y| (0..shown.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let (a, b) = (hidden.pixel(*x, *y), shown.pixel(*x, *y));
            (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0) > 8
        })
        .count();
    assert!(
        changed > 5_000,
        "the window drew almost nothing: {changed} pixels"
    );

    // And the report behind it is the thing that gets pasted.
    let text = report.to_report();
    assert!(
        text.contains("g804fc9d"),
        "the revision is missing:\n{text}"
    );
    assert!(
        text.contains("metal declined raycast"),
        "the fallback is missing:\n{text}"
    );
}

#[test]
fn the_export_panel_says_what_will_not_survive_the_write() {
    // Captured because the warnings are the point of the panel: if they run
    // off the edge or read as decoration, no assertion will notice.
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();

    let settings = clayspace_model::ExportSettings {
        mesher: clayspace_model::ExportMesher::Fast,
        decimate_to: Some(0.4),
        ..Default::default()
    };
    let warnings =
        clayspace_model::ExportWarning::for_export(clayspace_model::Format::Ply, settings, true);
    assert_eq!(warnings.len(), 3, "{warnings:?}");

    let mut open = state(strings, &scene, &materials, &report);
    open.show_export = true;
    open.export = settings;
    open.export_warnings = &warnings;
    let shown = capture_shell(&harness, &open, "65-export");

    let closed = state(strings, &scene, &materials, &report);
    let hidden = capture_shell(&harness, &closed, "65-export-closed");
    let changed = (0..shown.height)
        .flat_map(|y| (0..shown.width).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let (a, b) = (hidden.pixel(*x, *y), shown.pixel(*x, *y));
            (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0) > 8
        })
        .count();
    assert!(changed > 5_000, "the panel drew almost nothing: {changed}");
}

#[test]
fn the_import_panel_names_the_choice_that_cannot_be_undone() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();

    let mut open = state(strings, &scene, &materials, &report);
    open.show_import = true;
    capture_shell(&harness, &open, "65-import");

    // Reference or clay is the decision the panel exists for, and both have
    // to say what they mean.
    for becomes in clayspace_model::ImportAs::ALL {
        assert!(!becomes.label().is_empty());
        assert!(!becomes.detail().is_empty());
    }
}

/// The shelf holds the active representation's verbs and nothing else.
///
/// Before the capability table there was one shelf of fifteen whatever the
/// layer, four of which refused on an SDF layer and eleven on a voxel one, each
/// saying so only once clicked. The two captures below are the same shell with
/// only the active layer's representation changed.
#[test]
fn the_shelf_holds_what_the_representation_has() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::EnUs);
    let scene = scene();
    let materials = ["MatCap Cinza 01", "MatCap Cinza 02", "Gesso"];
    let report = diagnostics();

    let mut sdf = state(strings, &scene, &materials, &report);
    sdf.representation = clayspace_model::Representation::Sdf;
    let sdf_image = capture_shell(&harness, &sdf, "65-shelf-sdf");

    let mut voxel = state(strings, &scene, &materials, &report);
    voxel.representation = clayspace_model::Representation::Voxel;
    // A voxel tool, so the active brush is one this shelf actually holds.
    voxel.tool = clayspace_model::ToolKind::Raspar;
    let voxel_image = capture_shell(&harness, &voxel, "65-shelf-voxel");

    let sdf_tools =
        clayspace_model::ToolKind::for_representation(clayspace_model::Representation::Sdf);
    let voxel_tools =
        clayspace_model::ToolKind::for_representation(clayspace_model::Representation::Voxel);
    assert_ne!(
        sdf_tools, voxel_tools,
        "the two representations offer the same tools, so this proves nothing"
    );

    // The shelf occupies the bottom band, so a difference there is the shelf's.
    let band = sdf_image.height.saturating_sub(140)..sdf_image.height;
    let differing = band
        .flat_map(|y| (0..sdf_image.width).map(move |x| (x, y)))
        .filter(|(x, y)| sdf_image.pixel(*x, *y) != voxel_image.pixel(*x, *y))
        .count();
    assert!(
        differing > 500,
        "the shelf looks the same for both representations ({differing} pixels \
         differ), so it is not following the active layer"
    );
}

/// The conversion panel, in each direction a layer can cross.
///
/// Its whole job is to state the losses before the crossing runs, and the
/// figures are recomputed from the cell size rather than written into the
/// strings — so the capture is also the check that they render at all.
#[test]
fn the_conversion_panel_states_what_a_crossing_costs() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::EnUs);
    let scene = scene();
    let materials = ["MatCap Cinza 01", "MatCap Cinza 02", "Gesso"];
    let report = diagnostics();

    // Two resolutions, so the capture shows the figures following the choice
    // rather than describing a default.
    for (cell, name) in [(0.02, "66-convert-fine"), (0.1, "66-convert-coarse")] {
        let mut open = state(strings, &scene, &materials, &report);
        open.show_convert = true;
        open.conversion = clayspace_model::ConversionSettings {
            direction: clayspace_model::Direction::SdfToVoxel,
            cell_size: cell,
            blur: 1,
        };
        open.conversion_cost = Some(clayspace_model::Cost::of(
            clayspace_model::Direction::SdfToVoxel,
            cell,
            [2.0, 2.0, 2.0],
        ));
        let shown = capture_shell(&harness, &open, name);

        let mut closed = state(strings, &scene, &materials, &report);
        closed.show_convert = false;
        let hidden = capture_shell(&harness, &closed, "66-convert-closed");
        let differing = shown
            .pixels
            .chunks_exact(4)
            .zip(hidden.pixels.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        // Counted rather than compared whole: a failed `assert_ne!` on two
        // 1280x800 buffers prints megabytes of pixels and says nothing.
        assert!(
            differing > 2000,
            "the conversion panel drew nothing at cell {cell} ({differing} \
             pixels differ from the same shell with it closed)"
        );
    }

    // Every representation reaches the other two, so the panel offers two
    // crossings whatever the active layer is — its contents follow the active
    // representation like the shelf does. An SDF layer used to offer one,
    // because nothing crossed into a mesh.
    for representation in clayspace_model::Representation::ALL {
        assert_eq!(
            clayspace_model::Direction::from_representation(representation).len(),
            2,
            "a {representation:?} layer does not reach both of the others"
        );
    }
}

/// The repair panel reports before it offers to change anything.
///
/// A sealed void is invisible until something needs the model to be solid, so
/// the report is the only way a sculptor learns there is one — and a repair
/// that ran before saying what it would change would be asking consent for
/// something unstated.
#[test]
fn the_repair_panel_reports_before_it_repairs() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::EnUs);
    let scene = scene();
    let materials = ["MatCap Cinza 01", "MatCap Cinza 02", "Gesso"];
    let report = diagnostics();

    let mut damaged = state(strings, &scene, &materials, &report);
    damaged.show_repair = true;
    damaged.repair = Some(clayspace_model::RepairReport {
        enclosed_voids: 3,
        void_cells: 812,
        largest_void: 500,
        airtight: false,
    });
    let shown = capture_shell(&harness, &damaged, "67-repair-voids");

    // Airtight: the fill button has nothing to do, so it is not offered. A
    // button that can only report having done nothing is worse than absence.
    let mut airtight = state(strings, &scene, &materials, &report);
    airtight.show_repair = true;
    airtight.repair = Some(clayspace_model::RepairReport {
        airtight: true,
        ..Default::default()
    });
    let clean = capture_shell(&harness, &airtight, "67-repair-airtight");

    let differing = shown
        .pixels
        .chunks_exact(4)
        .zip(clean.pixels.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        differing > 500,
        "a grid with voids and an airtight one drew the same panel \
         ({differing} pixels differ), so the report is not reaching it"
    );

    // And on a layer that cannot have a report at all.
    let mut field = state(strings, &scene, &materials, &report);
    field.show_repair = true;
    field.repair = None;
    let _ = capture_shell(&harness, &field, "67-repair-not-a-grid");
}

/// The rename field replaces the name it is editing, in place.
///
/// The panel's own state is what this proves: the field draws where the label
/// was, on the row it was opened over and on no other. That is the part a
/// model test cannot reach — `layer_rename.rs` already holds the engine to
/// renaming the right layer, and it would still pass if the field appeared on
/// every row at once.
#[test]
fn renaming_a_layer_draws_a_field_in_its_row() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];

    let report = diagnostics();
    let plain = state(strings, &scene, &materials, &report);
    let before = capture_shell(&harness, &plain, "67-rename-before");

    let report = diagnostics();
    let mut editing = state(strings, &scene, &materials, &report);
    editing.renaming = Some((LayerKey(12), "Poros_finos"));
    let after = capture_shell(&harness, &editing, "67-rename-field");

    // The rows of the layer stack, from the top of the left panel down. The
    // field is a filled text box where a label was, so the row it is on
    // changes a great deal and the rows around it must not change at all —
    // which is the claim: one row is being renamed, not the stack.
    let mut changed_rows: Vec<(u32, usize)> = Vec::new();
    let mut run = 0usize;
    let mut start = 0u32;
    for y in 0..before.height {
        let mut differing = 0usize;
        for x in 0..region_width(&before) {
            if before.pixel(x, y) != after.pixel(x, y) {
                differing += 1;
            }
        }
        if differing > 0 {
            if run == 0 {
                start = y;
            }
            run += 1;
        } else if run > 0 {
            changed_rows.push((start, run));
            run = 0;
        }
    }
    if run > 0 {
        changed_rows.push((start, run));
    }

    let total: usize = changed_rows.iter().map(|(_, height)| height).sum();
    assert!(
        total > 0,
        "opening the rename field changed nothing in the left panel, so it did \
         not draw — see target/visual/67-rename-field.png"
    );
    // One band, not several: a field drawn on every row, or a layout shifted
    // by the field's height, would spread the difference down the stack.
    assert_eq!(
        changed_rows.len(),
        1,
        "the rename field changed {} separate bands of the left panel ({changed_rows:?}); \
         it belongs to one row",
        changed_rows.len()
    );
    let (_, height) = changed_rows[0];
    assert!(
        (8..=40).contains(&height),
        "the changed band is {height} pixels tall, which is not one layer row"
    );
}

/// The width of the left panel, for comparisons that must ignore the viewport.
///
/// The capture is one pixel per logical unit, so the region's own widths are
/// the answer without scaling.
fn region_width(image: &clayspace_view::Image) -> u32 {
    let width = (region::RAIL + region::LEFT) as u32;
    width.min(image.width)
}

/// A right-click on a layer row, as egui receives one.
fn right_click(at: egui::Pos2) -> Vec<egui::Event> {
    click(at, egui::PointerButton::Secondary)
}

/// A left-click on a menu entry.
fn left_click(at: egui::Pos2) -> Vec<egui::Event> {
    click(at, egui::PointerButton::Primary)
}

fn click(at: egui::Pos2, button: egui::PointerButton) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton {
            pos: at,
            button,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        },
        egui::Event::PointerButton {
            pos: at,
            button,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        },
    ]
}

/// The two entries of a layer's menu, relative to where it was opened.
///
/// A menu is laid out below and to the right of the pointer, so its entries
/// are found from the click rather than from the panel. The offsets are the
/// frame's own padding and one entry's height — small enough that landing on
/// the wrong one is caught by the assertion, not silently tolerated.
const RENAME_ENTRY: egui::Vec2 = egui::Vec2::new(37.0, 17.0);
const DELETE_ENTRY: egui::Vec2 = egui::Vec2::new(37.0, 42.0);

/// Renaming and deleting are reachable from a layer row.
///
/// The layer stack had no way to do either: the model has carried
/// `rename_layer` and `remove_layer` since the beginning and the panel offered
/// neither. Driven with real input rather than inspected as state, because
/// every part of this can fail on its own — a menu can fail to open, and an
/// entry that opens can be wired to nothing.
#[test]
fn a_layer_row_offers_renaming_and_deleting() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];

    // The top of the stack, which is the last layer in evaluation order.
    let top = scene.layers.last().expect("a layer").key;

    let report = diagnostics();
    let plain = state(strings, &scene, &materials, &report);
    let closed = capture_shell(&harness, &plain, "68-layer-menu-closed");
    let row = row_centre(&plain, top);

    // Opened and left open, which is the picture worth keeping.
    let report = diagnostics();
    let opened_state = state(strings, &scene, &materials, &report);
    let opened = capture_shell_after(
        &harness,
        &opened_state,
        "68-layer-menu",
        &[right_click(row)],
        |queue| {
            // Opening a menu is not an edit. Anything here would mean the
            // right-click also selected or renamed something.
            assert!(
                queue.is_empty(),
                "opening the layer menu emitted {:?}",
                queue.commands()
            );
        },
    );
    let differing = differing_pixels(&closed, &opened);
    assert!(
        differing > 400,
        "right-clicking a layer row changed {differing} pixels, so no menu \
         opened — see target/visual/68-layer-menu.png"
    );

    let report = diagnostics();
    let rename_state = state(strings, &scene, &materials, &report);
    capture_shell_after(
        &harness,
        &rename_state,
        "68-layer-menu-rename",
        &[right_click(row), left_click(row + RENAME_ENTRY)],
        |queue| {
            assert_eq!(
                queue.commands(),
                [Command::BeginRenameLayer(top)],
                "Renomear did not open the rename field on the row it was on"
            );
        },
    );

    let report = diagnostics();
    let delete_state = state(strings, &scene, &materials, &report);
    capture_shell_after(
        &harness,
        &delete_state,
        "68-layer-menu-delete",
        &[right_click(row), left_click(row + DELETE_ENTRY)],
        |queue| {
            assert_eq!(
                queue.commands(),
                [Command::RemoveLayer(top)],
                "Excluir did not remove the row it was on"
            );
        },
    );
}

/// The last layer's Delete is offered as refused rather than as working.
///
/// `removing_the_only_layer_is_refused` holds the model to the rule. This
/// holds the interface to *saying* it, which is the difference between a
/// sculptor learning the rule and a sculptor finding an error in the status
/// area after the fact.
#[test]
fn the_last_layer_cannot_be_deleted_from_its_menu() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let mut lone = scene();
    lone.layers.truncate(1);
    let only = lone.layers[0].key;
    lone.active = Some(only);
    let materials = ["MatCap Cinza 01"];

    let report = diagnostics();
    let probe = state(strings, &lone, &materials, &report);
    let row = row_centre(&probe, only);

    let report = diagnostics();
    let lone_state = state(strings, &lone, &materials, &report);
    capture_shell_after(
        &harness,
        &lone_state,
        "68-layer-menu-one",
        &[right_click(row), left_click(row + DELETE_ENTRY)],
        |queue| {
            assert!(
                queue.is_empty(),
                "the only layer's Excluir was live and emitted {:?}",
                queue.commands()
            );
        },
    );

    // And the menu did open — otherwise the assertion above passes for the
    // wrong reason, which is how the first version of this test passed.
    let report = diagnostics();
    let rename_state = state(strings, &lone, &materials, &report);
    capture_shell_after(
        &harness,
        &rename_state,
        "68-layer-menu-one-rename",
        &[right_click(row), left_click(row + RENAME_ENTRY)],
        |queue| {
            assert_eq!(
                queue.commands(),
                [Command::BeginRenameLayer(only)],
                "the only layer has no menu at all, so Excluir being quiet \
                 says nothing about it being disabled"
            );
        },
    );
}

/// Where a layer's row is, asked of the interface that drew it.
fn row_centre(state: &ShellState<'_>, key: LayerKey) -> egui::Pos2 {
    probe_shell(state)
        .read_response(shell::layer_row_id(key))
        .map(|response| response.rect.center())
        .unwrap_or_else(|| panic!("the layer stack drew no row for {key:?}"))
}

/// How many pixels differ between two captures.
fn differing_pixels(a: &clayspace_view::Image, b: &clayspace_view::Image) -> usize {
    let mut n = 0;
    for y in 0..a.height.min(b.height) {
        for x in 0..a.width.min(b.width) {
            if a.pixel(x, y) != b.pixel(x, y) {
                n += 1;
            }
        }
    }
    n
}

/// The panel offers the crossings into a mesh, and says what one costs.
///
/// `Direction` had four entries and none of them ended in a mesh, so the
/// panel could not offer what does not exist. This is the picture of the two
/// that now do, and of the loss that is theirs alone: what comes out sculpts
/// by moving the vertices it was given, and the topology is the sampling
/// lattice's rather than one built for deformation.
#[test]
fn the_conversion_panel_offers_a_crossing_into_a_mesh() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::EnUs);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();

    let mut open = state(strings, &scene, &materials, &report);
    open.show_convert = true;
    open.conversion = clayspace_model::ConversionSettings {
        direction: clayspace_model::Direction::SdfToMesh,
        cell_size: 0.05,
        blur: 0,
    };
    open.conversion_cost = Some(clayspace_model::Cost::of(
        clayspace_model::Direction::SdfToMesh,
        0.05,
        [2.0, 2.0, 2.0],
    ));
    let to_mesh = capture_shell(&harness, &open, "70-convert-to-mesh");

    // Against the same panel set to the crossing that does not end in a mesh:
    // the two differ, and they differ by the line about topology.
    let mut voxels = state(strings, &scene, &materials, &report);
    voxels.show_convert = true;
    voxels.conversion = clayspace_model::ConversionSettings {
        direction: clayspace_model::Direction::SdfToVoxel,
        cell_size: 0.05,
        blur: 0,
    };
    voxels.conversion_cost = Some(clayspace_model::Cost::of(
        clayspace_model::Direction::SdfToVoxel,
        0.05,
        [2.0, 2.0, 2.0],
    ));
    let to_voxels = capture_shell(&harness, &voxels, "70-convert-to-voxels");

    let differing = differing_pixels(&to_mesh, &to_voxels);
    assert!(
        differing > 200,
        "the panel draws the same thing for a crossing into a mesh and one \
         into a grid, {differing} pixels apart — see \
         target/visual/70-convert-to-mesh.png"
    );
}

// -- the mask panel and menu --------------------------------------------------
//
// Three of the six mask operations took an amount and the interface had no way
// to set one: the menu dispatched `Expandir` with a hard-coded 1, and an
// extrusion with every default it was born with, so its thickness, rounding
// and edge smoothing were unreachable. The amounts live in a MÁSCARA section
// of the inspector now, and the menu spells out what it would apply.

/// Where the Máscaras menu sits in the bar, and where its entries fall once it
/// is open.
///
/// The capture is one pixel per logical unit at the size the design specifies,
/// so these are the design's own coordinates rather than a scaling of them.
const MASKS_MENU: egui::Pos2 = egui::Pos2::new(359.0, 13.0);
const EXPAND_ENTRY: egui::Vec2 = egui::Vec2::new(0.0, 92.0);

#[test]
fn the_mask_section_appears_with_a_mask_and_not_without() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();

    let with = state(strings, &scene, &materials, &report);
    let masked = capture_shell(&harness, &with, "78-mask-panel");

    let mut without = state(strings, &scene, &materials, &report);
    without.mask = clayspace_model::MaskState::default();
    let bare = capture_shell(&harness, &without, "79-mask-panel-none");

    let changed = differing_pixels(&masked, &bare);
    assert!(
        changed > 400,
        "the inspector drew the same {changed} pixels with a mask and without \
         one, so the MÁSCARA section is not there. See \
         target/visual/78-mask-panel.png"
    );

    // And it is confined to the inspector: a section that pushed the shelf or
    // the viewport around would be a layout change rather than a section.
    let right_edge = SHELL_WIDTH - region::RIGHT as u32;
    let outside = (0..masked.height)
        .flat_map(|y| (0..right_edge).map(move |x| (x, y)))
        .filter(|(x, y)| masked.pixel(*x, *y) != bare.pixel(*x, *y))
        .count();
    assert_eq!(
        outside, 0,
        "{outside} pixels outside the inspector changed when the mask section \
         appeared"
    );
}

#[test]
fn the_mask_menu_applies_the_amount_the_panel_is_set_to() {
    // A menu entry that draws and is wired to nothing looks identical, and
    // this one now carries a number that has to come from somewhere. Driven
    // with real clicks for that reason.
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();

    let mut set = state(strings, &scene, &materials, &report);
    set.mask_steps = 5;
    capture_shell_after(
        &harness,
        &set,
        "80-mask-menu",
        &[
            left_click(MASKS_MENU),
            left_click(MASKS_MENU + EXPAND_ENTRY),
        ],
        |queue| {
            assert_eq!(
                queue.commands(),
                [Command::ApplyMaskOp(clayspace_model::MaskOp::Expand(5))],
                "Expandir carried something other than the panel's five \
                 steps. See target/visual/80-mask-menu.png"
            );
        },
    );
}

/// A drag across a slider, as egui receives one.
fn drag(from: egui::Pos2, to: egui::Pos2) -> Vec<Vec<egui::Event>> {
    vec![
        vec![
            egui::Event::PointerMoved(from),
            egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
        ],
        vec![egui::Event::PointerMoved(to)],
        vec![egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        }],
    ]
}

/// Where a named slider's handle is, asked of the interface that drew it.
///
/// Not a pixel coordinate: panels grow, and a coordinate that found the Passos
/// slider found the cage's Pontos por eixo the day a section landed above it.
fn slider_centre(state: &ShellState<'_>, label: &str) -> egui::Pos2 {
    probe_shell(state)
        .memory(|memory| memory.data.get_temp::<egui::Rect>(shell::slider_id(label)))
        .unwrap_or_else(|| panic!("the inspector drew no slider labelled {label:?}"))
        .center()
}

#[test]
fn the_steps_slider_sets_the_amount() {
    // The other half of the pair: the menu applies whatever the panel says,
    // and this is the panel saying it. A slider that draws and is wired to
    // nothing looks exactly like one that works.
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();
    let mut set = state(strings, &scene, &materials, &report);
    set.mask_steps = 5;
    let steps = slider_centre(&set, strings.label_mask_steps);

    capture_shell_after(
        &harness,
        &set,
        "81-mask-steps",
        &drag(steps, steps + egui::vec2(50.0, 0.0)),
        |queue| {
            let steps: Vec<i32> = queue
                .commands()
                .iter()
                .filter_map(|command| match command {
                    Command::SetMaskSteps(steps) => Some(*steps),
                    _ => None,
                })
                .collect();
            let last = steps.last().copied().unwrap_or_else(|| {
                panic!(
                    "dragging the Passos slider emitted {:?} and no amount. See \
                     target/visual/81-mask-steps.png",
                    queue.commands()
                )
            });
            assert!(
                last > 5,
                "dragging the slider to the right set {last} steps, down from 5"
            );
        },
    );
}

/// Where the first Extrudar entry falls once the Máscaras menu is open.
const EXTRUDE_ENTRY: egui::Vec2 = egui::Vec2::new(0.0, 232.0);

#[test]
fn extrudar_is_offered_on_a_field_and_greyed_on_a_mesh() {
    // It was offered everywhere and worked on one of the three. On a mesh the
    // engine refuses — there is no field to sample and no mesh-sculptor
    // equivalent — and the refusal went into a notice nobody read, so the
    // entry was a click that did nothing at all.
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();

    let field = state(strings, &scene, &materials, &report);
    capture_shell_after(
        &harness,
        &field,
        "82-extrude-field",
        &[
            left_click(MASKS_MENU),
            left_click(MASKS_MENU + EXTRUDE_ENTRY),
        ],
        |queue| {
            let extrusions: Vec<&Command> = queue
                .commands()
                .iter()
                .filter(|command| matches!(command, Command::ExtrudeMask(_)))
                .collect();
            assert_eq!(
                extrusions.len(),
                1,
                "Extrudar on a field emitted {:?}. See \
                 target/visual/82-extrude-field.png",
                queue.commands()
            );
        },
    );

    let mut mesh = state(strings, &scene, &materials, &report);
    mesh.representation = clayspace_model::Representation::Mesh;
    capture_shell_after(
        &harness,
        &mesh,
        "83-extrude-mesh",
        &[
            left_click(MASKS_MENU),
            left_click(MASKS_MENU + EXTRUDE_ENTRY),
        ],
        |queue| {
            assert!(
                queue.commands().is_empty(),
                "Extrudar was live on a mesh layer and emitted {:?}, which the \
                 engine refuses. See target/visual/83-extrude-mesh.png",
                queue.commands()
            );
        },
    );
}

/// Where the Dinâmica menu sits in the bar, and where its first entry falls.
const DYNAMICS_MENU: egui::Pos2 = egui::Pos2::new(296.0, 13.0);
const CAGE_ENTRY: egui::Vec2 = egui::Vec2::new(0.0, 30.0);

#[test]
fn the_cage_is_raised_from_the_menu_and_worked_in_the_panel() {
    // Two halves that fail independently: the menu can draw an entry wired to
    // nothing, and the panel can show a section for a cage that is not up.
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();

    let down = state(strings, &scene, &materials, &report);
    capture_shell_after(
        &harness,
        &down,
        "90-cage-menu",
        &[
            left_click(DYNAMICS_MENU),
            left_click(DYNAMICS_MENU + CAGE_ENTRY),
        ],
        |queue| {
            assert_eq!(
                queue.commands(),
                [Command::ToggleLattice],
                "the Dinâmica menu's cage entry emitted {:?}. See \
                 target/visual/90-cage-menu.png",
                queue.commands()
            );
        },
    );

    // The section is not in the panel until a cage is up: it is where a cage
    // is *worked*, and one standing there whether or not a cage existed pushed
    // the sections below it past the bottom of the panel.
    let bare = capture_shell(&harness, &down, "91-cage-panel-none");
    let mut up = state(strings, &scene, &materials, &report);
    up.lattice = clayspace_model::LatticeState {
        active: true,
        divisions: [3; 3],
        points: vec![[0.0; 3]; 27],
        selection: vec![0],
        mode: clayspace_model::GizmoMode::Move,
        rest_span: 2.0,
        touched: true,
    };
    let worked = capture_shell(&harness, &up, "92-cage-panel");
    assert!(
        differing_pixels(&bare, &worked) > 400,
        "the inspector drew the same panel with a cage up and without one"
    );

    // And its own control is reachable, which the mask's stopped being the day
    // a section landed above it.
    let divisions = slider_centre(&up, strings.label_cage_divisions);
    capture_shell_after(
        &harness,
        &up,
        "93-cage-divisions",
        &drag(divisions, divisions + egui::vec2(40.0, 0.0)),
        |queue| {
            let asked: Vec<[i32; 3]> = queue
                .commands()
                .iter()
                .filter_map(|command| match command {
                    Command::SetLatticeDivisions(divisions) => Some(*divisions),
                    _ => None,
                })
                .collect();
            let last = asked.last().copied().unwrap_or_else(|| {
                panic!(
                    "dragging Pontos por eixo emitted {:?} and no divisions",
                    queue.commands()
                )
            });
            assert!(
                last[0] > 3 && last == [last[0]; 3],
                "the cage was asked for {last:?}, which is not a uniform grid \
                 finer than the three it was at"
            );
        },
    );
}

// -- the language menu -------------------------------------------------------

/// Where the Vista menu sits, and where its Idioma entry falls once open.
/// The Portuguese layout's, because that is the one whose coordinates the
/// other menu tests here already use — the entry is the same entry in every
/// language.
const VIEW_MENU: egui::Pos2 = egui::Pos2::new(131.0, 13.0);
const LANGUAGE_ENTRY: egui::Vec2 = egui::Vec2::new(5.0, 196.0);

#[test]
fn the_language_can_be_chosen_from_the_menu() {
    // Three complete translations shipped from the beginning with no way to
    // choose between them: the locale came from `Locale::default()` at startup
    // and was never asked about again, so `Locale::from_tag` — written for
    // exactly this — was called by nothing.
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();
    let state = state(strings, &scene, &materials, &report);

    capture_shell_after(
        &harness,
        &state,
        "107-language-menu",
        &[left_click(VIEW_MENU)],
        |queue| {
            assert!(
                queue.is_empty(),
                "opening the View menu emitted {:?} without a choice",
                queue.commands()
            );
        },
    );

    // The entry is there, and its submenu carries the three languages, each
    // named in itself — the one rule a language menu has, because a reader who
    // cannot read the current interface still has to find their own.
    let opened = capture_shell_after(
        &harness,
        &state,
        "108-language-open",
        &[
            left_click(VIEW_MENU),
            left_click(VIEW_MENU + LANGUAGE_ENTRY),
            // A frame with no input, so the submenu is measured as well as
            // placed.
            Vec::new(),
        ],
        |_| {},
    );
    let closed = capture_shell(&harness, &state, "109-language-closed");
    assert!(
        differing_pixels(&opened, &closed) > 2000,
        "the Idioma entry opened nothing. See target/visual/108-language-open.png"
    );

    // A note for whoever reads that capture: the accented letters in
    // `Português` and `Español` are blank in it. They are not missing from the
    // font — the Spanish shell's own `Tamaño` renders its `ñ` in
    // `the_shell_renders_in_every_locale` — but they appear nowhere else in
    // the interface, so they are glyphs the atlas first meets on the frame
    // this submenu opens, and this helper applies the atlas deltas of the
    // first passes only. A window renders continuously and catches up.
}

#[test]
fn the_interface_opens_in_english() {
    // Not the design's own language, and deliberately: the interface has to
    // open in something a first-time user can read.
    assert_eq!(Locale::default(), Locale::EnUs);
    assert_eq!(
        Strings::for_locale(Locale::default()).menu_file,
        Strings::for_locale(Locale::EnUs).menu_file
    );
}

#[test]
fn every_table_knows_which_language_it_is() {
    // Carried with the words, so the menu's tick and the words on screen
    // cannot disagree about what the interface is in.
    for locale in Locale::ALL {
        assert_eq!(
            Strings::for_locale(locale).locale,
            locale,
            "the {} table reports itself as {:?}",
            locale.tag(),
            Strings::for_locale(locale).locale
        );
    }
}

#[test]
fn the_brush_shelf_is_in_the_interfaces_language() {
    // The shelf showed `ToolKind::label()` — the domain's own Portuguese — on
    // all three representations whatever the language was, so choosing English
    // translated the chrome and left Padrão, Inflar and Máscara on the shelf.
    let Some(harness) = Harness::new() else {
        return;
    };
    let scene = scene();
    let materials = ["MatCap Cinza 01"];
    let report = diagnostics();

    let mut shots = Vec::new();
    for locale in [Locale::PtBr, Locale::EnUs, Locale::Es419] {
        let strings = Strings::for_locale(locale);
        let state = state(strings, &scene, &materials, &report);
        shots.push(capture_shell(
            &harness,
            &state,
            &format!("110-shelf-{}", locale.tag()),
        ));
    }

    // The shelf is the bottom band, below the status bar's height. Comparing
    // only there keeps this about the brushes rather than about the panels,
    // which were already translated.
    let band = |image: &clayspace_view::Image| {
        let top = SHELL_HEIGHT - region::SHELF as u32 - region::STATUS as u32;
        let mut differing = 0usize;
        for y in top..(SHELL_HEIGHT - region::STATUS as u32) {
            for x in 0..image.width {
                differing += usize::from(image.pixel(x, y) != shots[0].pixel(x, y));
            }
        }
        differing
    };
    assert!(
        band(&shots[1]) > 500,
        "the English shelf draws the same {} pixels as the Portuguese one, so \
         the brush names are not translated. See target/visual/110-shelf-en-US.png",
        band(&shots[1])
    );
    assert!(
        band(&shots[2]) > 300,
        "the Spanish shelf draws the same pixels as the Portuguese one"
    );
}
