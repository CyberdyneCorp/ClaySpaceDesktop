//! The composition root.
//!
//! The one place that constructs the engine bridge, the Model, the ViewModels,
//! the renderer and the window, and injects each downward. No other crate
//! builds a layer other than its own.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use clayspace_app::{ray_at, SessionStore, SharedDocument, SurfaceGeometry, ViewportInput};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    AutosavePolicy, Diagnostics, RecentDocuments, Recovery, SkinSettings, ViewPresetKind,
};
use clayspace_view::shell::{self, region, ArmatureState, ShellState};
use clayspace_view::{
    mirrored_cursors, ArmatureView, BrushCursor, Camera, Gpu, Locale, MatCap, Overlays, Renderer,
    Strings, SurfaceLoss, ViewPreset, WindowSurface,
};
use clayspace_vm::{
    ArmatureViewModel, Axis, Command, CommandQueue, DocumentViewModel, Grab, Guard, MaskViewModel,
    SceneViewModel, SculptViewModel,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

fn main() {
    let policy = match BackendPolicy::discover(None) {
        Ok(policy) => policy,
        Err(e) => {
            eprintln!("the engine's backends could not be discovered: {e}");
            return;
        }
    };
    report(&policy);

    let document =
        match ClayDocument::new(policy.clone()).and_then(ClayDocument::with_starting_form) {
            Ok(document) => document,
            Err(e) => {
                eprintln!("the starting document could not be built: {e}");
                return;
            }
        };

    let event_loop = EventLoop::new().expect("create the event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(SharedDocument::new(document), policy);
    event_loop.run_app(&mut app).expect("run the application");
}

/// The same report the diagnostics window shows, on the way up.
///
/// Printed from the one value rather than assembled again here: a startup
/// banner that drifts from the panel is worse than no banner, because the two
/// disagree in a bug report.
fn report(policy: &BackendPolicy) {
    print!("{}", policy.diagnostics().to_report());
}

/// What the pointer is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drag {
    None,
    Sculpt,
    Orbit,
    Pan,
    /// A ZSphere gesture. The plane it runs on is held alongside.
    Rig,
}

struct App {
    document: SharedDocument,
    sculpt: SculptViewModel,
    scene: SceneViewModel,
    document_vm: DocumentViewModel,
    mask: MaskViewModel,
    armature: ArmatureViewModel,
    policy: BackendPolicy,

    camera: Camera,
    window: Option<Arc<Window>>,
    graphics: Option<Graphics>,

    drag: Drag,
    /// Where the pointer last met the surface, and the direction it arrived
    /// from.
    ///
    /// Kept rather than the finished cursor so the ring can be rebuilt from
    /// the *current* brush size and symmetry every frame. Storing the ring
    /// instead left it stale until the pointer moved again, which is a cursor
    /// that lies about what the next click will do.
    hover: Option<([f32; 3], [f32; 3])>,
    /// The viewport's rectangle in egui points, which is what a pointer
    /// position from egui arrives in.
    viewport: Option<egui::Rect>,
    /// The symmetry the overlays were last built for.
    overlay_symmetry: [bool; 3],
    /// Whether the pointer is rigging rather than sculpting.
    rigging: bool,
    /// Whether the diagnostics window is open, and whether the last thing that
    /// happened in it was a copy.
    show_diagnostics: bool,
    diagnostics_copied: bool,

    /// Where session state lives, when this machine has somewhere to put it.
    store: Option<SessionStore>,
    recent: RecentDocuments,
    autosave: AutosavePolicy,
    /// When the last autosave was written, or when the session began.
    saved_at: Instant,
    /// What the previous session left, until it has been offered.
    pending_recovery: Recovery,
    /// Set by the menu's Sair, acted on where the event loop can be exited.
    quit_requested: bool,
    /// The plane a rig gesture runs on: a point on it, and its normal.
    ///
    /// Fixed at the press rather than recomputed per sample. A plane that
    /// re-derives itself from the moving pointer drifts, and the sphere slides
    /// away from the cursor over a long drag.
    rig_plane: Option<([f32; 3], [f32; 3])>,
    strings: &'static Strings,
}

struct Graphics {
    gpu: Gpu,
    surface: WindowSurface,
    renderer: Renderer,
    geometry: SurfaceGeometry,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl App {
    fn new(document: SharedDocument, policy: BackendPolicy) -> Self {
        let sculpt = SculptViewModel::new(Box::new(document.clone()));
        let scene = SceneViewModel::new(Box::new(document.clone()));
        let document_vm = DocumentViewModel::new(Box::new(document.clone()), "Sem título");
        let mask = MaskViewModel::new(Box::new(document.clone()));
        let armature = ArmatureViewModel::new(Box::new(document.clone()));

        // Read before the marker for this session is written, or every run
        // would find its own marker and offer to recover from itself.
        let store = SessionStore::discover();
        let (recent, pending_recovery) = match &store {
            Some(store) => {
                let leftovers = store.recovery();
                store.begin_session();
                (store.load_recent(), leftovers)
            }
            None => (RecentDocuments::default(), Recovery::Nothing),
        };

        Self {
            document,
            sculpt,
            scene,
            document_vm,
            mask,
            armature,
            policy,
            camera: Camera::default(),
            window: None,
            graphics: None,
            drag: Drag::None,
            hover: None,
            viewport: None,
            overlay_symmetry: [false; 3],
            rigging: false,
            show_diagnostics: false,
            diagnostics_copied: false,
            store,
            recent,
            autosave: AutosavePolicy::default(),
            saved_at: Instant::now(),
            pending_recovery,
            quit_requested: false,
            rig_plane: None,
            strings: Strings::for_locale(Locale::default()),
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Builds every GPU-side resource. Called at startup and after device loss.
    fn create_graphics(&mut self) -> bool {
        let Some(window) = self.window.clone() else {
            return false;
        };
        let (gpu, surface) = match pollster::block_on(WindowSurface::new(window.clone())) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("could not start rendering: {e}");
                return false;
            }
        };

        let mut renderer = Renderer::new(&gpu, surface.format());
        renderer.set_overlays(&gpu, Overlays::default(), 3.0);
        renderer.show_gizmo = true;

        let context = egui::Context::default();
        shell::apply_theme(&context);
        let egui_state = egui_winit::State::new(
            context,
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(&gpu.device, surface.format(), None, 1, false);

        let geometry = SurfaceGeometry::new(&gpu);
        self.graphics = Some(Graphics {
            gpu,
            surface,
            renderer,
            geometry,
            egui_state,
            egui_renderer,
        });

        self.sync_geometry();
        self.frame_all();
        true
    }

    /// Re-meshes whatever the document reports as dirty.
    fn sync_geometry(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let gpu = graphics.gpu.clone();
        let result = self
            .document
            .with(|document| graphics.geometry.sync(&gpu, document));
        match result {
            Ok(_) => self.sculpt.acknowledge_remesh(),
            Err(e) => eprintln!("the surface could not be re-meshed: {e}"),
        }
    }

    /// Saves, asking for a path when there is not one yet.
    fn save(&mut self, ask_for_path: bool) {
        let known = self.document_vm.path().get().clone();
        let path = match (known, ask_for_path) {
            (Some(path), false) => Some(path),
            _ => rfd::FileDialog::new()
                .set_title("Salvar escultura")
                .add_filter("ClaySpace", &["clayspace"])
                .set_file_name(format!("{}.clayspace", self.document_vm.name().get()))
                .save_file(),
        };
        let Some(path) = path else {
            return; // Cancelled. Not a failure, and nothing to report.
        };
        match self.document_vm.save_as(&path) {
            Ok(()) => {
                self.remember(&path);
                // The autosave clock restarts from a real save: the point is
                // how long work has been at risk, not how long the timer has
                // been running.
                self.saved_at = Instant::now();
            }
            Err(e) => eprintln!("{e}"),
        }
        self.request_redraw();
    }

    /// Opens a document, after asking about unsaved work.
    fn open(&mut self) {
        if self.document_vm.guard() == Guard::WouldLoseWork && !self.confirm_discarding_work() {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title("Abrir escultura")
            .add_filter("ClaySpace", &["clayspace"])
            .pick_file()
        else {
            return;
        };
        self.open_path(&path);
    }

    /// Opens a known path, after the caller has cleared unsaved work.
    fn open_path(&mut self, path: &std::path::Path) {
        match self.document_vm.open(path) {
            Ok(()) => {
                self.remember(path);
                self.saved_at = Instant::now();
                self.after_document_replaced();
            }
            Err(e) => {
                eprintln!("{e}");
                // A document that could not be opened should not stay in the
                // list offering to fail again. The store prunes missing files
                // on the way in; this covers the rest — permissions, a
                // corrupt file, a version this build cannot read.
                self.recent.prune(|candidate| candidate != path);
                if let Some(store) = &self.store {
                    store.save_recent(&self.recent);
                }
            }
        }
        self.request_redraw();
    }

    /// Starts a new document, after asking about unsaved work.
    fn new_document(&mut self) {
        if self.document_vm.guard() == Guard::WouldLoseWork && !self.confirm_discarding_work() {
            return;
        }
        match self.document_vm.new_document() {
            Ok(()) => self.after_document_replaced(),
            Err(e) => eprintln!("{e}"),
        }
        self.request_redraw();
    }

    /// Offers back what a session that did not close left behind.
    ///
    /// Once, at the start, and only where there is something to offer. A
    /// declined offer takes the file with it: asking again next time about
    /// work the sculptor has already said they do not want is nagging.
    fn offer_recovery(&mut self) {
        let Some(path) = self.pending_recovery.path().map(PathBuf::from) else {
            return;
        };
        self.pending_recovery = Recovery::Nothing;

        let wanted = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Trabalho recuperado")
            .set_description(
                "A sessão anterior terminou inesperadamente. Recuperar o que \
                 estava aberto?",
            )
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes;

        if !wanted {
            if let Some(store) = &self.store {
                store.discard_recovery();
            }
            return;
        }
        match self.document_vm.recover(&path, "Recuperado") {
            Ok(()) => self.after_document_replaced(),
            Err(e) => eprintln!("{e}"),
        }
        self.request_redraw();
    }

    /// Writes the recovery file, when one is due.
    ///
    /// Failure is reported to the log and nowhere else. An autosave that could
    /// not be written is worth knowing about; interrupting a sculptor to say
    /// so, mid-stroke, is not.
    fn maybe_autosave(&mut self) {
        let Some(store) = self.store.clone() else {
            return;
        };
        if !self
            .autosave
            .is_due(self.saved_at.elapsed(), *self.document_vm.modified().get())
        {
            return;
        }
        // Stamped before the attempt, not after: a save that keeps failing
        // must not retry on every wake-up.
        self.saved_at = Instant::now();
        if let Err(e) = self.document_vm.autosave_to(&store.autosave_path()) {
            eprintln!("a recuperação automática falhou: {e}");
        }
    }

    /// How long the event loop may sleep before an autosave could be due.
    fn autosave_deadline(&self) -> Option<Instant> {
        self.autosave
            .next_in(self.saved_at.elapsed(), *self.document_vm.modified().get())
            .map(|wait| Instant::now() + wait)
    }

    /// Records a document in the recent list and writes it out.
    fn remember(&mut self, path: &std::path::Path) {
        self.recent.remember(path);
        if let Some(store) = &self.store {
            store.save_recent(&self.recent);
        }
    }

    /// Everything that has to happen before the process ends.
    fn end_session(&self) {
        if let Some(store) = &self.store {
            store.end_session();
        }
    }

    /// Asks whether unsaved work may be thrown away.
    ///
    /// A native dialog rather than something drawn in the shell: this is the
    /// one question in the application whose answer cannot be undone, and the
    /// platform's own dialog is the one a user already knows how to read.
    fn confirm_discarding_work(&self) -> bool {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Alterações não salvas")
            .set_description("A escultura tem alterações que não foram salvas. Descartar?")
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes
    }

    /// Everything that has to catch up when the document underneath changes.
    fn after_document_replaced(&mut self) {
        self.scene.refresh();
        self.mask.refresh();
        self.armature.refresh();
        // Rigging is a mode over a rig that no longer exists. Leaving it on
        // would hand the next click to a tree that was thrown away.
        self.rigging = self.rigging && self.armature.is_rigging();
        self.sculpt.forget_history();
        if let Some(graphics) = self.graphics.as_mut() {
            let gpu = graphics.gpu.clone();
            // A rebuild rather than a sync: nothing about the old document's
            // dirty set describes the new one.
            if let Err(e) = self
                .document
                .with(|document| graphics.geometry.rebuild(&gpu, document))
            {
                eprintln!("the surface could not be meshed: {e}");
            }
        }
        self.frame_all();
    }

    /// Everything that has to catch up after the rig changed outside a drag.
    fn after_armature_edit(&mut self) {
        self.settle_geometry();
        self.scene.refresh();
        self.document_vm.touched();
        if let Some(notice) = self.armature.notice().get() {
            eprintln!("{notice}");
        }
        self.request_redraw();
    }

    /// Re-meshes the whole surface after a gesture, clearing the seams the
    /// per-segment path leaves.
    fn settle_geometry(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let gpu = graphics.gpu.clone();
        let result = self
            .document
            .with(|document| graphics.geometry.settle(&gpu, document));
        match result {
            Ok(()) => self.sculpt.acknowledge_remesh(),
            Err(e) => eprintln!("the surface could not be re-meshed: {e}"),
        }
    }

    fn frame_all(&mut self) {
        match self.sculpt.bounds() {
            Some((min, max)) => self.camera.frame_bounds(min.into(), max.into()),
            None => self.camera.frame_default(),
        }
    }

    /// Turns a pointer position into a ray, when it is over the viewport.
    fn ray_at(&self, point: egui::Pos2) -> Option<([f32; 3], [f32; 3])> {
        ray_at(&self.camera, self.viewport?, point)
    }

    /// Where the surface is under a pointer position, if it is there at all.
    fn pick_at(&self, point: egui::Pos2) -> Option<([f32; 3], [f32; 3])> {
        let (origin, direction) = self.ray_at(point)?;
        let hit = self.sculpt.pick(origin, direction)?;
        // Toward the camera: the pick reports where the surface is, not which
        // way it faces, and a ring in the view plane reads correctly from any
        // angle.
        Some((hit, [-direction[0], -direction[1], -direction[2]]))
    }

    /// Where a pointer ray meets the plane a rig gesture runs on.
    ///
    /// A rig lives in three dimensions and the pointer has two, so a gesture
    /// needs a plane. This one faces the camera and passes through whatever was
    /// grabbed, which is the plane a person is already picturing when they drag
    /// a shoulder sideways.
    fn on_rig_plane(&self, point: egui::Pos2) -> Option<[f32; 3]> {
        let (anchor, normal) = self.rig_plane?;
        let (origin, direction) = self.ray_at(point)?;
        let slope: f32 = (0..3).map(|i| direction[i] * normal[i]).sum();
        if slope.abs() < 1e-6 {
            return None; // Looking along the plane; nothing to meet.
        }
        let reach: f32 = (0..3)
            .map(|i| (anchor[i] - origin[i]) * normal[i])
            .sum::<f32>()
            / slope;
        if reach <= 0.0 {
            return None; // Behind the eye.
        }
        Some([
            origin[0] + direction[0] * reach,
            origin[1] + direction[1] * reach,
            origin[2] + direction[2] * reach,
        ])
    }

    /// Begins a rig gesture, if the press landed on a sphere.
    ///
    /// Returns whether it did: a press on empty space is left to fall through
    /// to orbiting, so a rig can be turned to look at without leaving the mode.
    fn begin_rig(&mut self, point: egui::Pos2, input: &ViewportInput) -> bool {
        let Some((origin, direction)) = self.ray_at(point) else {
            return false;
        };
        // The default is to grow, which is what makes rigging feel like
        // drawing; holding the modifier takes hold instead.
        let grab = self.armature.grab_at(
            origin,
            direction,
            !input.orbit_modifier,
            input.command_modifier,
        );
        let index = match grab {
            Grab::Empty => return false,
            Grab::Move(i) | Grab::Grow(i) | Grab::Resize(i) => i,
        };
        let Some(centre) = self
            .armature
            .tree()
            .get()
            .as_ref()
            .and_then(|tree| tree.get(index).map(|sphere| sphere.position))
        else {
            return false;
        };
        // Facing the eye, through the sphere that was grabbed.
        self.rig_plane = Some((centre, [-direction[0], -direction[1], -direction[2]]));
        let at = self.on_rig_plane(point).unwrap_or(centre);
        self.armature.press(grab, at);
        true
    }

    /// This build and this machine, as the window shows it.
    ///
    /// Rebuilt each frame rather than cached: it is a handful of string
    /// allocations against a frame that meshes a surface, and a cached report
    /// is one that goes stale precisely when a fallback happens — which is the
    /// moment it exists for.
    fn diagnostics(&self) -> Diagnostics {
        let mut report = self.policy.diagnostics();
        report.renderer = self
            .graphics
            .as_ref()
            .map(|graphics| graphics.gpu.adapter_description());
        report
    }

    /// The state the shell needs about the rig.
    fn armature_state(&self) -> ArmatureState {
        let tree = self.armature.tree().get();
        ArmatureState {
            exists: tree.is_some(),
            editing: self.rigging,
            selection: self.armature.selected().get().is_some(),
            spheres: tree.as_ref().map(|t| t.nodes.len()).unwrap_or(0),
            mirror: *self.armature.symmetric().get(),
            skin: self.armature.skin().get().thickness,
        }
    }

    /// Hands the viewport the rig to draw, or nothing when there is none.
    fn sync_armature_view(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let gpu = graphics.gpu.clone();
        let tree = self.armature.tree().get().clone();
        // Drawn only while rigging: a rig is scaffolding, and leaving it over
        // a finished sculpt would hide the surface it produced.
        let Some(tree) = tree.filter(|_| self.rigging) else {
            graphics.renderer.set_armature(
                &gpu,
                ArmatureView {
                    spheres: &[],
                    links: &[],
                    selected: None,
                    root: None,
                },
            );
            return;
        };

        let thickness = self.armature.skin().get().thickness;
        let spheres: Vec<([f32; 3], f32)> = tree
            .nodes
            .iter()
            .map(|node| (node.position, node.radius * thickness))
            .collect();
        let links: Vec<(u32, u32)> = tree.links();
        // Whichever is under the pointer wins the highlight; the selection is
        // what a menu command would act on when nothing is hovered.
        let selected = self
            .armature
            .hovered()
            .get()
            .or(*self.armature.selected().get());
        graphics.renderer.set_armature(
            &gpu,
            ArmatureView {
                spheres: &spheres,
                links: &links,
                selected,
                root: (!spheres.is_empty()).then_some(0),
            },
        );
    }

    /// The rings to draw: the pointer's, plus one for every place symmetry
    /// will also deposit the dab.
    fn cursors(&self) -> Vec<BrushCursor> {
        let Some((position, normal)) = self.hover else {
            return Vec::new();
        };
        let cursor = BrushCursor {
            position,
            normal,
            radius: self.sculpt.brush().get().size,
            mirrored: false,
        };
        mirrored_cursors(cursor, *self.sculpt.symmetry().get())
    }

    /// Rebuilds the mirror-plane overlays when the symmetry changes.
    ///
    /// The planes cost a rebuild, so this is not done every frame — but it is
    /// checked every frame, because symmetry can be toggled from the options
    /// bar as well as the keyboard and neither should need a nudge to show.
    fn sync_symmetry_overlay(&mut self) {
        let symmetry = *self.sculpt.symmetry().get();
        if symmetry == self.overlay_symmetry {
            return;
        }
        self.overlay_symmetry = symmetry;
        if let Some(graphics) = self.graphics.as_mut() {
            let gpu = graphics.gpu.clone();
            graphics.renderer.set_overlays(
                &gpu,
                Overlays {
                    grid: true,
                    symmetry_planes: symmetry,
                },
                3.0,
            );
        }
    }

    /// Sends a stroke sample at the pointer, if it met the surface.
    fn stroke_at(&mut self, point: egui::Pos2, begin: bool) {
        let Some((position, _)) = self.pick_at(point) else {
            return;
        };
        let command = if begin {
            Command::BeginStroke {
                position,
                pressure: 1.0,
            }
        } else {
            Command::ContinueStroke {
                position,
                pressure: 1.0,
            }
        };
        self.apply(command);
    }

    /// Acts on one frame's worth of viewport input.
    ///
    /// Routing lives here rather than in the winit handler because the panels
    /// decide where the viewport is, and only egui knows whether a press
    /// landed on a widget. Asking winit produced a version that dropped every
    /// press in the window.
    fn drive(&mut self, input: &ViewportInput) {
        // Hover first, and unconditionally: the ring should follow the pointer
        // during a stroke as well as before one.
        if let Some(point) = input.pointer {
            if self.rigging {
                // The brush ring has no meaning while rigging; the highlighted
                // sphere is the cursor.
                self.hover = None;
                if let Some((origin, direction)) = self.ray_at(point) {
                    self.armature.hover(origin, direction);
                }
            } else {
                self.hover = self.pick_at(point);
            }
        }

        if input.released && self.drag != Drag::None {
            match self.drag {
                Drag::Sculpt => self.apply(Command::EndStroke),
                Drag::Rig => {
                    self.armature.release();
                    self.rig_plane = None;
                    // The same debt the stroke path pays off at the end of a
                    // gesture: the per-edit re-mesh leaves seams where the
                    // refilled region met the rest.
                    self.settle_geometry();
                }
                _ => {}
            }
            self.drag = Drag::None;
        }

        if let (Some(point), Some(button), true) =
            (input.pointer, input.pressed, input.over_viewport)
        {
            // Rigging takes the primary button first, and only where it lands
            // on a sphere: everywhere else the camera keeps working, so a rig
            // can be turned to look at without leaving the mode.
            let rigged = self.rigging
                && button == egui::PointerButton::Primary
                && self.begin_rig(point, input);
            let on_surface = !rigged && self.pick_at(point).is_some();
            let started = match button {
                _ if rigged => Drag::Rig,
                egui::PointerButton::Middle => Drag::Pan,
                egui::PointerButton::Secondary => Drag::Orbit,
                // On the surface it sculpts; off it, it orbits. That is
                // ZBrush's rule, and it is the only one that leaves a Mac
                // trackpad — which has no comfortable right-drag — able to
                // turn the model.
                _ if input.orbit_modifier || !on_surface => Drag::Orbit,
                _ => Drag::Sculpt,
            };
            self.drag = started;
            if started == Drag::Sculpt {
                self.stroke_at(point, true);
            }
        }

        if self.drag != Drag::None && input.delta != egui::Vec2::ZERO {
            match self.drag {
                Drag::Sculpt => {
                    if let Some(point) = input.pointer {
                        self.stroke_at(point, false);
                    }
                }
                Drag::Rig => {
                    if let Some(at) = input.pointer.and_then(|point| self.on_rig_plane(point)) {
                        self.armature.drag(at);
                        // Live, like a stroke: the surface follows the sphere
                        // rather than appearing when the pointer comes up.
                        self.sync_geometry();
                        self.document_vm.touched();
                    }
                }
                Drag::Orbit => self
                    .camera
                    .orbit(input.delta.x * 0.008, input.delta.y * 0.008),
                Drag::Pan => self.camera.pan(input.delta.x, input.delta.y),
                Drag::None => {}
            }
        }

        if input.over_viewport && input.scroll != 0.0 {
            self.camera.zoom(input.scroll);
        }
    }

    /// Dispatches a command to whichever ViewModel owns it.
    fn apply(&mut self, command: Command) {
        if let Err(e) = self.sculpt.dispatch(command.clone()) {
            // A refusal is not swallowed; the tool status carries the reason
            // to the options bar, and this records it for the log.
            eprintln!("{e}");
        }
        if let Err(e) = self.scene.dispatch(&command) {
            eprintln!("{e}");
        }
        self.mask.dispatch(&command);
        // A rig belongs to a layer, so choosing another layer changes which
        // one — or whether there is one at all.
        if matches!(command, Command::SelectLayer(_)) {
            self.armature.refresh();
            self.rigging = self.rigging && self.armature.is_rigging();
        }
        if command.touches_document() {
            self.scene.refresh();
            // Painting a mask arrives as a stroke, which this ViewModel never
            // sees, so it is told to look again rather than left stale.
            self.mask.refresh();
            self.sync_geometry();
            // The title bar's "não salvo" is driven from here rather than
            // inferred inside the document ViewModel, which never sees a
            // sculpting command and would have to guess.
            self.document_vm.touched();
        }
        // The end of a gesture is where the fast path's approximation is paid
        // off: a full re-mesh, once, rather than the slivers it leaves along
        // the seams of the edit. Affordable here because the pointer is up and
        // nobody is waiting on the next sample.
        if matches!(command, Command::EndStroke | Command::CancelStroke) {
            self.settle_geometry();
        }
        self.request_redraw();
    }

    fn redraw(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if self.graphics.is_none() {
            return;
        }

        // The interface is built first, because it decides where the viewport
        // is and therefore what a pointer position means.
        let raw_input = self
            .graphics
            .as_mut()
            .expect("graphics")
            .egui_state
            .take_egui_input(&window);
        let context = self
            .graphics
            .as_ref()
            .expect("graphics")
            .egui_state
            .egui_ctx()
            .clone();

        let scene = self.scene.scene().get().clone();
        let materials: Vec<&str> = MatCap::ALL.iter().map(|m| m.label()).collect();
        let material = self
            .graphics
            .as_ref()
            .map(|g| g.renderer.matcap().label())
            .unwrap_or("");
        let backend = self.policy.active().to_string();
        let memory = self
            .document
            .with(|document| document.cache().stats().ok())
            .map(|stats| (stats.memory_usage, stats.memory_budget.unwrap_or(0)))
            .unwrap_or((0, 0));
        let document_name = self.document_vm.name().get().clone();
        let last = self.sculpt.last_action().get().clone();
        let history = *self.sculpt.history().get();

        let diagnostics = self.diagnostics();
        let mut queue = CommandQueue::new();
        let mut viewport = None;
        let mut input = ViewportInput::default();
        let state = ShellState {
            strings: self.strings,
            mask: *self.mask.state().get(),
            armature: self.armature_state(),
            recent: self.recent.paths(),
            diagnostics: &diagnostics,
            show_diagnostics: self.show_diagnostics,
            diagnostics_copied: self.diagnostics_copied,
            extrude: *self.mask.extrude_settings().get(),
            document_name: document_name.as_str(),
            modified: *self.document_vm.modified().get(),
            tool: *self.sculpt.tool().get(),
            brush: *self.sculpt.brush().get(),
            tool_status: self.sculpt.tool_status().get().as_deref(),
            symmetry: *self.sculpt.symmetry().get(),
            scene: &scene,
            stats: *self.sculpt.stats().get(),
            view_preset: *self.sculpt.view_preset().get(),
            material,
            materials: &materials,
            can_undo: history.can_undo,
            can_redo: history.can_redo,
            memory,
            backend: &backend,
            units: "mm",
            last_action: (!last.label.is_empty()).then_some((last.label.as_str(), last.changed)),
        };

        let output = context.run(raw_input, |ctx| {
            egui::TopBottomPanel::top("menu")
                .exact_height(region::MENU_BAR)
                .show(ctx, |ui| shell::menu_bar(ui, &state, &mut queue));
            egui::TopBottomPanel::top("options")
                .exact_height(region::OPTIONS_BAR)
                .show(ctx, |ui| shell::options_bar(ui, &state, &mut queue));
            egui::TopBottomPanel::bottom("status")
                .exact_height(region::STATUS)
                .show(ctx, |ui| shell::status_bar(ui, &state));
            egui::TopBottomPanel::bottom("shelf")
                .exact_height(region::SHELF)
                .show(ctx, |ui| {
                    egui::ScrollArea::horizontal()
                        .show(ui, |ui| shell::brush_shelf(ui, &state, &mut queue));
                });
            egui::SidePanel::left("left")
                .exact_width(region::LEFT)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| shell::left_panel(ui, &state, &mut queue));
                });
            egui::SidePanel::right("right")
                .exact_width(region::RIGHT)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| shell::right_panel(ui, &state, &mut queue));
                });
            shell::diagnostics_window(ctx, &state, &mut queue);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        shell::viewport_bar(ui, &state, &mut queue);
                        // Whatever the bar left is the viewport, allocated as
                        // a real region so egui does the hit test. Its rect is
                        // also what the scene is drawn into, so the ray and
                        // the pixels cannot drift apart.
                        let (rect, response) = ui.allocate_exact_size(
                            ui.available_size(),
                            egui::Sense::click_and_drag(),
                        );
                        viewport = Some(rect);
                        input = ViewportInput::read(ui, &response);
                    });
                });
        });

        let scale = context.pixels_per_point();
        self.viewport = viewport;

        // `state` borrowed the scene and the strings for the frame. It is not
        // `Drop`, so calling `drop` on it did nothing but say so; letting the
        // binding end is what actually returns the borrows.
        let _ = &state;
        self.drive(&input);
        for command in queue.drain() {
            if matches!(command, Command::CopyDiagnostics) {
                // The clipboard belongs to the platform, so it is reached here
                // rather than from a View that is meant to emit commands and
                // nothing else.
                context.copy_text(diagnostics.to_report());
                self.diagnostics_copied = true;
                continue;
            }
            self.handle(command);
        }

        // The rings are rebuilt every frame from the live brush and symmetry,
        // so `[` and `]` and the mirror toggles show up without the pointer
        // having to move to prompt them.
        let cursors = self.cursors();
        let scene_viewport = self.viewport.map(|rect| {
            [
                rect.min.x * scale,
                rect.min.y * scale,
                rect.width() * scale,
                rect.height() * scale,
            ]
        });
        self.sync_symmetry_overlay();
        self.sync_armature_view();

        let graphics = self.graphics.as_mut().expect("graphics");
        let frame = match graphics.surface.acquire(&graphics.gpu) {
            Ok(frame) => frame,
            Err(SurfaceLoss::Skip | SurfaceLoss::Reconfigure) => return,
            Err(SurfaceLoss::DeviceLost) => {
                eprintln!("the graphics device was lost; rebuilding rendering");
                self.graphics = None;
                if self.create_graphics() {
                    self.request_redraw();
                }
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        graphics.renderer.set_cursors(&graphics.gpu, &cursors);
        graphics.renderer.set_scene_viewport(scene_viewport);
        graphics.renderer.render(
            &graphics.gpu,
            &view,
            graphics.surface.framebuffer(),
            &self.camera,
            graphics.geometry.mesh(),
            false,
        );
        paint_interface(graphics, &context, output, &view, scale);
        frame.present();
    }

    /// Applies a command, plus the view-side effects no ViewModel owns.
    fn handle(&mut self, command: Command) {
        match command {
            Command::FrameAll => {
                self.frame_all();
                self.request_redraw();
            }
            Command::NextMaterial => {
                if let Some(graphics) = self.graphics.as_mut() {
                    let gpu = graphics.gpu.clone();
                    let next = next_matcap(graphics.renderer.matcap());
                    graphics.renderer.set_matcap(&gpu, next);
                }
                self.request_redraw();
            }
            Command::NewDocument => self.new_document(),
            Command::OpenDocument => self.open(),
            Command::OpenRecent(path) => {
                if self.document_vm.guard() == Guard::WouldLoseWork
                    && !self.confirm_discarding_work()
                {
                    return;
                }
                self.open_path(&path);
            }
            Command::Save => self.save(false),
            Command::SaveAs => self.save(true),
            Command::Quit => self.quit_requested = true,
            Command::ToggleDiagnostics => {
                self.show_diagnostics = !self.show_diagnostics;
                // The confirmation belongs to one visit. Leaving it set means
                // the window reopens claiming a copy that never happened.
                self.diagnostics_copied = false;
                self.request_redraw();
            }
            Command::CopyDiagnostics => {}
            Command::NewArmature => {
                // At the middle of what is already there, so the first sphere
                // lands inside the model rather than at a world origin that
                // may be nowhere near it.
                let at = self
                    .sculpt
                    .bounds()
                    .map(|(min, max)| {
                        [
                            (min[0] + max[0]) * 0.5,
                            (min[1] + max[1]) * 0.5,
                            (min[2] + max[2]) * 0.5,
                        ]
                    })
                    .unwrap_or([0.0; 3]);
                self.armature.begin(at);
                self.rigging = self.armature.is_rigging();
                self.after_armature_edit();
            }
            Command::ToggleArmatureEditing => {
                self.rigging = !self.rigging && self.armature.is_rigging();
                self.request_redraw();
            }
            Command::RemoveZsphere => {
                self.armature.remove_selected();
                self.after_armature_edit();
            }
            Command::SetArmatureMirror(on) => {
                self.armature.set_symmetric(on);
                self.request_redraw();
            }
            Command::SetSkinThickness(thickness) => {
                self.armature.set_skin(SkinSettings { thickness });
                self.after_armature_edit();
            }
            Command::SetViewPreset(preset) => {
                self.camera.apply_preset(match preset {
                    ViewPresetKind::Perspective => ViewPreset::Perspective,
                    ViewPresetKind::Front => ViewPreset::Front,
                    ViewPresetKind::Side => ViewPreset::Side,
                    ViewPresetKind::Top => ViewPreset::Top,
                });
                self.apply(command);
            }
            other => self.apply(other),
        }
    }
}

/// Paints one egui frame over the viewport.
fn paint_interface(
    graphics: &mut Graphics,
    context: &egui::Context,
    output: egui::FullOutput,
    target: &wgpu::TextureView,
    pixels_per_point: f32,
) {
    let primitives = context.tessellate(output.shapes, pixels_per_point);
    for (id, delta) in &output.textures_delta.set {
        graphics.egui_renderer.update_texture(
            &graphics.gpu.device,
            &graphics.gpu.queue,
            *id,
            delta,
        );
    }

    let descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [
            graphics.surface.framebuffer().width,
            graphics.surface.framebuffer().height,
        ],
        pixels_per_point,
    };
    let mut encoder = graphics
        .gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("interface"),
        });
    graphics.egui_renderer.update_buffers(
        &graphics.gpu.device,
        &graphics.gpu.queue,
        &mut encoder,
        &primitives,
        &descriptor,
    );
    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("interface"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load, not clear: the sculpt is already there.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        graphics
            .egui_renderer
            .render(&mut pass.forget_lifetime(), &primitives, &descriptor);
    }
    graphics.gpu.queue.submit(Some(encoder.finish()));

    for id in &output.textures_delta.free {
        graphics.egui_renderer.free_texture(id);
    }
}

impl ApplicationHandler for App {
    /// Sleeps until there is something to do, or until an autosave is due.
    ///
    /// `Wait` alone would be right for an application that only acts on input,
    /// and wrong for this one: the case autosave exists for is a sculptor who
    /// edits and then walks away, which produces no further events at all.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.quit_requested {
            self.quit_requested = false;
            if self.document_vm.guard() == Guard::Clear || self.confirm_discarding_work() {
                self.end_session();
                event_loop.exit();
                return;
            }
        }
        self.maybe_autosave();
        event_loop.set_control_flow(match self.autosave_deadline() {
            Some(deadline) => ControlFlow::WaitUntil(deadline),
            None => ControlFlow::Wait,
        });
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Sculptor 3D")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        self.window = Some(Arc::new(
            event_loop
                .create_window(attributes)
                .expect("create the window"),
        ));
        if !self.create_graphics() {
            event_loop.exit();
            return;
        }
        // After the window, so the dialog has something to sit in front of.
        self.offer_recovery();
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // The interface sees input first and says whether it wanted it.
        let consumed = {
            let Some(window) = self.window.clone() else {
                return;
            };
            match self.graphics.as_mut() {
                Some(graphics) => {
                    let response = graphics.egui_state.on_window_event(&window, &event);
                    if response.repaint {
                        window.request_redraw();
                    }
                    response.consumed
                }
                None => false,
            }
        };

        match event {
            WindowEvent::CloseRequested => {
                // The last chance to keep the work. Closing over unsaved edits
                // without asking is the one mistake this application can make
                // that a user cannot undo.
                if self.document_vm.guard() == Guard::Clear || self.confirm_discarding_work() {
                    // Clearing the marker is what makes the *next* run silent.
                    // Left behind, an ordinary quit looks like a crash.
                    self.end_session();
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(graphics) = self.graphics.as_mut() {
                    let gpu = graphics.gpu.clone();
                    graphics.surface.resize(&gpu, size.width, size.height);
                }
                self.request_redraw();
            }

            WindowEvent::RedrawRequested => self.redraw(),

            // Pointer input is not handled here. egui owns the hit test —
            // it knows where the panels are — so the viewport reads its own
            // input inside the frame; see `ViewportInput`. All this handler
            // does is make sure a frame happens.
            WindowEvent::MouseInput { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseWheel { .. } => self.request_redraw(),

            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() && !consumed => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };

                // The file shortcuts, which take the platform's command
                // modifier and so are handled before the bare-letter ones.
                let modifiers = self
                    .graphics
                    .as_ref()
                    .map(|g| g.egui_state.egui_ctx().input(|i| i.modifiers))
                    .unwrap_or_default();
                if modifiers.command {
                    match code {
                        KeyCode::KeyS if modifiers.shift => self.save(true),
                        KeyCode::KeyS => self.save(false),
                        KeyCode::KeyO => self.open(),
                        KeyCode::KeyN => self.new_document(),
                        _ => {}
                    }
                    return;
                }

                let command = match code {
                    KeyCode::Digit1 => Some(Command::SetViewPreset(ViewPresetKind::Perspective)),
                    KeyCode::Digit2 => Some(Command::SetViewPreset(ViewPresetKind::Front)),
                    KeyCode::Digit3 => Some(Command::SetViewPreset(ViewPresetKind::Side)),
                    KeyCode::Digit4 => Some(Command::SetViewPreset(ViewPresetKind::Top)),
                    KeyCode::KeyA => Some(Command::ToggleArmatureEditing),
                    KeyCode::Delete | KeyCode::Backspace if self.rigging => {
                        Some(Command::RemoveZsphere)
                    }
                    KeyCode::KeyF => Some(Command::FrameAll),
                    KeyCode::KeyM => Some(Command::NextMaterial),
                    KeyCode::KeyX => Some(Command::ToggleSymmetry(Axis::X)),
                    KeyCode::KeyY => Some(Command::ToggleSymmetry(Axis::Y)),
                    KeyCode::KeyZ => Some(Command::Undo),
                    KeyCode::KeyR => Some(Command::Redo),
                    KeyCode::BracketLeft => {
                        Some(Command::SetBrushSize(self.sculpt.brush().get().size * 0.8))
                    }
                    KeyCode::BracketRight => {
                        Some(Command::SetBrushSize(self.sculpt.brush().get().size * 1.25))
                    }
                    KeyCode::Escape => {
                        // The same question the close button asks, for the
                        // same reason.
                        if self.document_vm.guard() == Guard::Clear
                            || self.confirm_discarding_work()
                        {
                            self.end_session();
                            event_loop.exit();
                        }
                        None
                    }
                    _ => None,
                };
                if let Some(command) = command {
                    self.handle(command);
                }
            }

            _ => {}
        }
    }
}

fn next_matcap(current: MatCap) -> MatCap {
    let all = MatCap::ALL;
    let index = all.iter().position(|m| *m == current).unwrap_or(0);
    all[(index + 1) % all.len()]
}
