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
use clayspace_vm::CommandQueue;
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

fn state<'a>(strings: &'a Strings, scene: &'a Scene, materials: &'a [&'a str]) -> ShellState<'a> {
    ShellState {
        // A mask with something in it, so the menu's enabled state is what the
        // capture shows rather than a row of grey.
        mask: clayspace_model::MaskState {
            present: true,
            painted_cells: 4096,
        },
        extrude: clayspace_model::ExtrudeSettings::default(),
        // A rig, mid-edit, so the capture shows the armature section and the
        // menu entries that depend on it rather than a row of grey.
        armature: clayspace_view::ArmatureState {
            exists: true,
            editing: true,
            selection: true,
            spheres: 12,
            mirror: true,
            skin: 1.0,
        },
        strings,
        document_name: "Cabeça_Estudo_v03",
        modified: true,
        tool: ToolKind::Padrao,
        brush: BrushSettings::default(),
        tool_status: None,
        symmetry: [true, false, false],
        scene,
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
        units: "mm",
        last_action: Some(("Padrão", true)),
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

/// Draws the whole shell into one egui frame and returns the captured image.
fn capture_shell(harness: &Harness, state: &ShellState<'_>, name: &str) -> clayspace_view::Image {
    let ctx = egui::Context::default();
    shell::apply_theme(&ctx);

    let mut queue = CommandQueue::new();
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(SHELL_WIDTH as f32, SHELL_HEIGHT as f32),
        )),
        ..Default::default()
    };

    let output = ctx.run(raw_input, |ctx| {
        egui::TopBottomPanel::top("menu")
            .exact_height(region::MENU_BAR)
            .show(ctx, |ui| shell::menu_bar(ui, state, &mut queue));
        egui::TopBottomPanel::top("options")
            .exact_height(region::OPTIONS_BAR)
            .show(ctx, |ui| shell::options_bar(ui, state, &mut queue));
        egui::TopBottomPanel::bottom("status")
            .exact_height(region::STATUS)
            .show(ctx, |ui| shell::status_bar(ui, state));
        egui::TopBottomPanel::bottom("shelf")
            .exact_height(region::SHELF)
            .show(ctx, |ui| {
                egui::ScrollArea::horizontal()
                    .show(ui, |ui| shell::brush_shelf(ui, state, &mut queue));
            });
        egui::SidePanel::left("left")
            .exact_width(region::LEFT)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .show(ui, |ui| shell::left_panel(ui, state, &mut queue));
            });
        egui::SidePanel::right("right")
            .exact_width(region::RIGHT)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .show(ui, |ui| shell::right_panel(ui, state, &mut queue));
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Tokens::ground()))
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    shell::viewport_bar(ui, state, &mut queue);
                });
            });
    });

    // The interface must not have mutated anything to draw itself; commands
    // are the only channel out, and nothing was clicked.
    assert!(
        queue.is_empty(),
        "drawing the interface emitted {} commands without any input",
        queue.len()
    );

    let target = OffscreenTarget::new(&harness.gpu, SHELL_WIDTH, SHELL_HEIGHT);
    let image = render_egui(harness, &ctx, output, &target);
    support::save(&image, name);
    image
}

/// Paints one egui frame into an offscreen target and reads it back.
fn render_egui(
    harness: &Harness,
    ctx: &egui::Context,
    output: egui::FullOutput,
    target: &OffscreenTarget,
) -> clayspace_view::Image {
    let mut renderer =
        egui_wgpu::Renderer::new(&harness.gpu.device, OffscreenTarget::FORMAT, None, 1, false);

    let pixels_per_point = ctx.pixels_per_point();
    let primitives = ctx.tessellate(output.shapes, pixels_per_point);
    for (id, delta) in &output.textures_delta.set {
        renderer.update_texture(&harness.gpu.device, &harness.gpu.queue, *id, delta);
    }

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
    let state = state(strings, &scene, &materials);

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
    let state = state(strings, &scene, &materials);
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
        let state = state(strings, &scene, &materials);
        let name = format!("62-shell-{:?}", locale).to_lowercase();
        captured.push((locale, capture_shell(&harness, &state, &name)));
    }

    // A locale whose labels are longer must not blank the interface or push a
    // region off screen; the frames differ but both draw.
    let (first, second) = (&captured[0].1, &captured[1].1);
    assert!(
        first.mean_difference(second) > 0.1,
        "the two locales rendered identically, so the strings are not reaching the interface"
    );
}

#[test]
fn the_active_tool_is_the_only_thing_wearing_the_accent() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let strings = Strings::for_locale(Locale::PtBr);
    let scene = scene();
    let materials = ["MatCap Cinza 01"];

    let mut first = state(strings, &scene, &materials);
    first.tool = ToolKind::Padrao;
    let a = capture_shell(&harness, &first, "63-accent-padrao");

    let mut second = state(strings, &scene, &materials);
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
