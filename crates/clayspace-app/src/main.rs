//! The composition root.
//!
//! The one place that constructs the engine bridge, the Model, the ViewModels,
//! the renderer and the window, and injects each downward. No other crate
//! builds a layer other than its own.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clayspace_app::input::Activation;
use clayspace_app::{
    chord_for, profile_file, ray_at, DocumentShape, SessionStore, SharedDocument, SurfaceGeometry,
    ViewportInput,
};
use clayspace_engine::{BackendPolicy, ClayDocument, RefillBudget};
use clayspace_mcp::{
    report, Applied, CaptureRequest, CaptureWhat, Catalogue, Consent, ConsentOutcome, Frame,
    JobQueue, Measured, Outstanding, Refusal, RefusalCode, Server, ServerHandle, Session, Settled,
    StateQuery, StateReport,
};
use clayspace_model::{
    AutosavePolicy, Detail, DetailPolicy, Diagnostics, ExchangeModel, ExportSettings,
    ExportWarning, Format, FrameLog, GizmoMode, ImportSettings, LayerKey, LayerOperation,
    ModelError, RecentDocuments, Recovery, RefFormat, RefPlane, Representation, SceneModel,
    SculptModel, SkinSettings, StrokeDiagnostics, StrokeModifiers, Units, ViewPresetKind,
};
use clayspace_view::shell::{self, region, ArmatureState, ShellState};
use clayspace_view::{
    mirrored_cursors, Action, ArmatureView, BrushCursor, Camera, Gpu, GpuMesh, InteractionState,
    Locale, MatCap, Overlays, Panel, QualityGovernor, Renderer, ShadingMode, Shortcuts, Strings,
    SurfaceLoss, Vertex, ViewPreset, ViewportProfile, WindowSurface,
};
use clayspace_vm::{
    AgentAnswer, AgentAsk, AgentViewModel, ArmatureViewModel, Axis, BooleanViewModel, Command,
    CommandQueue, CurveViewModel, DocumentViewModel, Door, Grab, Guard, LatticeViewModel,
    MaskViewModel, ObjectViewModel, Observable, Progress, ReferenceViewModel, SceneViewModel,
    SculptViewModel, UNTITLED,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{CursorIcon, Window, WindowId};

fn main() {
    let policy = match BackendPolicy::discover(None) {
        Ok(policy) => policy,
        Err(e) => {
            eprintln!("the engine's backends could not be discovered: {e}");
            return;
        }
    };
    report(&policy);

    let mut document =
        match ClayDocument::new(policy.clone()).and_then(ClayDocument::with_starting_form) {
            Ok(document) => document,
            Err(e) => {
                eprintln!("the starting document could not be built: {e}");
                return;
            }
        };
    // The budget is set here and nowhere else, because this is where a frame
    // starts existing. A document built headless — by a test, a benchmark, the
    // reference builder — has nothing waiting on it and keeps
    // `RefillBudget::Whole`, which is what makes an exact answer available to
    // whoever asks for one.
    document.set_refill_budget(RefillBudget::Within(REFILL_BUDGET));

    // With a user event, because the loop waits: `ControlFlow::Wait` is
    // deliberate — an idle application that redraws forever is the failure
    // `Observable` exists to prevent — and without a proxy to wake it, an
    // agent's command would sit in its queue until somebody moved the mouse.
    let event_loop = EventLoop::<AgentWake>::with_user_event()
        .build()
        .expect("create the event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(SharedDocument::new(document), policy);
    app.open_the_door(event_loop.create_proxy());
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

/// Half a frame, which is what a refill may spend on the interface thread
/// before it hands the frame back.
///
/// Half rather than all of one: a frame that spends every millisecond
/// refilling has none left to build the interface with, which is the stall
/// this exists to end wearing a different name. The budget sizes the batches
/// as well as stopping between them, so the overrun is one small batch and
/// not one large one.
///
/// The work is not dropped. `pump_refill` spends another budget on the next
/// frame and the frame after, and the viewport patches in each pump's bricks
/// as they land — so a cancel of a radius-5 tube comes back in a frame and
/// finishes redrawing over the next few, instead of holding the window for the
/// thirty-plus minutes the audit measured.
const REFILL_BUDGET: Duration = Duration::from_millis(8);

/// A request is waiting on the interface thread.
///
/// Carries nothing: what arrived is in the queue, and the event's only job is
/// to bring the loop out of `Wait` so the queue is looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AgentWake;

/// What this application is built from, generated by `tools/attribution.py`.
///
/// Embedded rather than read at runtime: a manifest that lives beside the
/// binary is one that goes missing the first time the application is
/// repackaged, and the licence policy in `deny.toml` is written on the
/// understanding that attribution travels with the distribution.
const ATTRIBUTION: &str = include_str!("../../../ATTRIBUTION.md");

/// Whether a report should carry the stroke section.
///
/// The report is rebuilt every frame, deliberately — a cached one goes stale
/// precisely when a fallback happens, which is the moment it exists for. Every
/// other section of it is a handful of string allocations; the stroke section
/// sorts every retained window, and measured **0.9 ms** once a session has
/// been worked. That is five per cent of a frame spent assembling a section
/// nobody had open, so it is assembled where somebody is going to read it —
/// the window, the export, an agent that asked for it — and nowhere else.
///
/// What this does **not** gate is the recording. `profile_overhead.rs`
/// measures one record at **18 ns**, five per dab, against a dab of two
/// milliseconds: the instrument is not part of what it measures, and switching
/// it off would buy nothing and cost the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StrokeSection {
    /// Read the profile and summarise it.
    Summarised,
    /// Leave it out. The section reads as absent, which is distinct from a
    /// session in which nothing was measured — that one reports every phase
    /// with no samples in it.
    Skipped,
}

/// The language the machine is set to, as a tag.
///
/// Read from the environment rather than through a crate: these are the
/// variables every desktop Unix sets and the ones a container inherits, and a
/// dependency to read three environment variables would be a dependency to
/// audit, license and keep. An empty answer is fine — `Locale::from_tag` gives
/// the default for anything it does not recognise.
fn system_language() -> String {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(name) {
            if !value.is_empty() && value != "C" && value != "POSIX" {
                return value;
            }
        }
    }
    String::new()
}

/// A manipulator drag: which handle, the plane it runs on, and where the hand
/// is taken to be.
#[derive(Debug, Clone, Copy, PartialEq)]
struct GizmoGesture {
    /// What the gesture does: the operation of the handle that was grabbed.
    mode: clayspace_model::GizmoMode,
    handle: clayspace_model::GizmoHandle,
    /// The plane the drag runs on, as an anchor and a normal.
    plane: ([f32; 3], [f32; 3]),
    /// Added to every pointer position on the plane before it is resolved.
    ///
    /// Zero for every handle but the centre in scale mode. A scale is a ratio
    /// of distances from the pivot, and a press on the centre handle starts a
    /// hair from it, so the ratio ran away in the first frame of the drag —
    /// one pull to the edge of the screen was ten times, and refused by the
    /// cache before it. The gesture is measured as if it had started one arm's
    /// length from the pivot instead: pulling outward by an arm doubles the
    /// form, pushing inward by an arm halves it, which is how ZBrush's scale
    /// reads and what a hand can meter.
    shift: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drag {
    None,
    Sculpt,
    Orbit,
    Pan,
    /// A ZSphere gesture. The plane it runs on is held alongside.
    Rig,
    /// Dragging a lattice control point, on the plane held alongside.
    Cage,
    /// Dragging the manipulator on the selection.
    Gizmo,
    /// Dragging a curve's control point.
    Curve,
    /// Drawing a box across the viewport to gather control points.
    Marquee,
    /// Drawing a mask outline over the form.
    Outline,
    /// Drawing a cut over the form: a line, a lasso or a box on the view
    /// frame, resolved into a prism when the pointer comes up.
    Cut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GizmoGeometryUpdate {
    None,
    Incremental,
    Settle,
}

/// How the SDF viewport catches up with a manipulator command.
///
/// A whole SDF layer is rebuilt from the document during its move so that the
/// brick mesher's artifacts never reach the screen.
fn gizmo_geometry_update(
    command: &Command,
    manipulating_clay: bool,
    representation: Representation,
) -> GizmoGeometryUpdate {
    if !manipulating_clay {
        return GizmoGeometryUpdate::None;
    }
    match command {
        Command::DragGizmo(..) | Command::EndGizmoDrag if representation == Representation::Sdf => {
            GizmoGeometryUpdate::Settle
        }
        Command::DragGizmo(..) | Command::EndGizmoDrag => GizmoGeometryUpdate::Incremental,
        _ => GizmoGeometryUpdate::None,
    }
}

/// A vector's direction and its length, or `None` where it has neither.
fn unit(v: [f32; 3]) -> Option<([f32; 3], f32)> {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    (length > 1e-6).then(|| (std::array::from_fn(|i| v[i] / length), length))
}

/// One reference lifted out of the ViewModel, owned.
///
/// Owned rather than borrowed because the pictures are read from `self` and
/// then handed to the viewport, which is also `self`: the copy is what ends
/// the first borrow. It happens once a change and not once a frame.
struct PlacedReference {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    corners: [[f32; 3]; 4],
    opacity: f32,
}

/// What a press on a curve resolves to. See [`App::curve_press_action`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurvePress {
    /// Split the span at this guide sample.
    Insert(usize),
    /// Add or remove this control point from the selection.
    Toggle(usize),
    /// Take hold of this control point.
    Grab(usize),
    /// On the line: the press is spent and nothing changes.
    Consume,
    /// Off everything: put a point down and open a freehand stroke.
    Append,
}

/// What the status area shows for memory, and when it was read.
///
/// `BrickCache::stats` is not the counter read its name suggests. The engine's
/// C binding fills `surface_bricks` by building a vector of every stored key
/// and taking its length, so one call is one allocation and one walk of the
/// whole cache — a cost proportional to the sculpture, paid whether or not
/// anything changed. The status bar asked for it on every redraw, which on a
/// worked document put 83% of main-thread samples inside that walk and left an
/// application with nobody touching it burning about 200% CPU (#167).
///
/// So it is read on a clock instead. The figure is a meter beside a progress
/// bar: a second behind reads the same to a person as exact, nothing else in
/// the application derives anything from it, and one walk a second is a cost
/// no sculpture can make matter.
///
/// What it reads is the whole ledger now — the engine's report with the
/// surfaces, the brick cache and the drawing, see [`clayspace_app::memory`] —
/// and the agent's `state.memory` and the diagnostics window read the same
/// reading, so no two of them can show different figures for one moment.
/// Generic over the reading so the clock can be tested without a document.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct MemoryMeter<T = (u64, u64)> {
    /// The figures as last read.
    figures: T,
    /// When they were read. `None` before the first reading, and again
    /// whenever the document they describe has been replaced.
    taken: Option<Instant>,
}

impl<T: Copy> MemoryMeter<T> {
    /// How stale the figure may be before it is read again.
    const INTERVAL: Duration = Duration::from_secs(1);

    /// The figures to show, taking a fresh reading only when the last has aged
    /// out.
    ///
    /// `now` is passed in rather than read here so the frame's own clock is
    /// what decides — the redraw already has one — and so this can be driven
    /// through an hour of frames in a test without sleeping through them.
    ///
    /// A reading that fails keeps the previous figures rather than showing
    /// zeroes: a cache that cannot answer has not thereby freed its memory.
    /// The stamp still moves, so a cache that answers no longer is asked once
    /// a second rather than once a frame.
    fn figures(&mut self, now: Instant, read: impl FnOnce() -> Option<T>) -> T {
        if self
            .taken
            .is_some_and(|taken| now.duration_since(taken) < Self::INTERVAL)
        {
            return self.figures;
        }
        self.taken = Some(now);
        if let Some(figures) = read() {
            self.figures = figures;
        }
        self.figures
    }

    /// Forgets the reading, so the next frame takes a fresh one.
    ///
    /// For the paths that replace the document outright — opening a file,
    /// resetting — where what is on screen is another document's memory and
    /// waiting out the interval would show it against the new one's name.
    fn forget(&mut self) {
        self.taken = None;
    }

    /// The figures as last read, without reading.
    fn last(&self) -> T {
        self.figures
    }
}

/// One reading of the memory meter.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct MemoryReading {
    /// The ledger, or `None` before the engine has answered once.
    ledger: Option<clayspace_model::MemoryDiagnostics>,
    /// What the operating system charged the process at the time, where the
    /// probe had a figure.
    footprint: Option<u64>,
}

/// Job progress is outstanding until a frame consumes the result. The same
/// list feeds `wait` and the `jobs` state section.
fn running_jobs(progress: &[&Option<Progress>]) -> Vec<Outstanding> {
    progress
        .iter()
        .filter_map(|item| {
            item.as_ref().map(|item| Outstanding {
                what: item.label.clone(),
                fraction: item.fraction,
            })
        })
        .collect()
}

struct App {
    document: SharedDocument,
    sculpt: SculptViewModel,
    scene: SceneViewModel,
    document_vm: DocumentViewModel,
    mask: MaskViewModel,
    lattice: LatticeViewModel,
    /// The shapes a sculptor has placed, and the manipulator on one.
    objects: ObjectViewModel,
    /// The boolean between two subtools, while one is being set up.
    boolean: BooleanViewModel,
    curve: CurveViewModel,
    cut: clayspace_vm::CutViewModel,
    /// Retopology to quads, which runs off this thread.
    retopo: clayspace_vm::RetopoViewModel,
    /// The UV layout, which runs off this thread as well.
    uv: clayspace_vm::UvViewModel,
    /// Conforming a retopologised mesh onto a field that has moved.
    conform: clayspace_vm::ConformViewModel,
    /// Baking maps from the field — the half of the pipeline only this
    /// application can supply, since it is the only place a field and a baker
    /// are both present.
    bake: clayspace_vm::BakeViewModel,
    /// Where the next bake writes its maps, chosen through the file panel.
    bake_into: Option<PathBuf>,
    /// The plane a curve drag runs on, and where it started.
    curve_drag: Option<([f32; 3], [f32; 3], [f32; 3])>,
    /// A freehand curve stroke in progress: the last point it laid, and the
    /// plane it is being drawn on.
    ///
    /// `Some` from the press that started the stroke until the release, which
    /// is what tells a drag from a click: a click never travels far enough to
    /// lay a second point.
    ///
    /// The plane is fixed at the press and the stroke stays on it. Re-picking
    /// the surface per sample would be worse than it sounds — by the second
    /// point there is a tube under the pointer, the one this stroke is
    /// drawing, so the ray would land on it and the curve would climb its own
    /// output.
    curve_draw: Option<([f32; 3], [f32; 3])>,
    /// When and where the last curve press landed, for spotting a double.
    ///
    /// Held here rather than read from egui because what a double-click *does*
    /// is a rule of this application — split the span under the pointer — and
    /// a rule worth testing should not need a window open to test it. See
    /// [`App::is_double_press`].
    last_curve_press: Option<(std::time::Instant, egui::Pos2)>,
    /// The plane a lattice drag runs on: an anchor and a normal facing the eye.
    cage_plane: Option<([f32; 3], [f32; 3])>,
    /// The selection box being drawn: where it began, where it has reached,
    /// and whether it adds to the selection rather than replacing it.
    ///
    /// Held for the length of one gesture. Read by the frame *after* the one
    /// that moved it, because the viewport's rectangle is what egui allocates
    /// and the pointer is not routed until that is known — a frame's lag on a
    /// band the hand is still dragging is not a thing an eye can catch.
    marquee: Option<(egui::Pos2, egui::Pos2, bool)>,
    /// The manipulator handle in hand, and the plane its drag runs on.
    gizmo_drag: Option<GizmoGesture>,
    /// The manipulator handle under the pointer, for the highlight.
    ///
    /// Kept rather than asked for while the frame is drawn, because the answer
    /// needs the camera, the pointer and the viewport's rectangle, and only
    /// two of the three are known where the widget is described.
    gizmo_hover: Option<(clayspace_model::GizmoMode, clayspace_model::GizmoHandle)>,
    /// How big the manipulator's target was when it was taken up: the target
    /// and half its widest extent. Measured once per selection rather than
    /// every frame, because the engine's bounds of a turned layer are a
    /// conservative box around the turn, and a widget sized from them swelled
    /// as the hand turned the form — and a widget that changes size under the
    /// hand mid-gesture is the wrong widget however right its size.
    gizmo_fit: Option<(clayspace_model::GizmoTarget, f32)>,
    armature: ArmatureViewModel,
    policy: BackendPolicy,

    camera: Camera,
    /// When to draw the coarse surface, and when to bring the full one back.
    detail_policy: DetailPolicy,
    /// The bindings in force, which the key handler is the only reader of.
    shortcuts: Shortcuts,
    /// The mesh revision the viewport's copy of the mesh layers was built at.
    ///
    /// A mesh layer is not in the brick cache, so `SurfaceGeometry` cannot
    /// hold it and the incremental machinery there does not apply: the
    /// triangles are copied whole or not at all. Comparing a revision is what
    /// keeps "not at all" the usual answer.
    mesh_revision: Option<u64>,
    /// The cage revision the drawn surface was warped at.
    cage_revision: Option<u64>,
    /// The mask revision the drawn surface was sampled at.
    ///
    /// Separate from `mesh_revision` because the brick surface is uploaded
    /// incrementally and the carried layers are not: a mask change re-samples
    /// the stored vertices in place rather than re-meshing anything.
    mask_revision: Option<u64>,
    /// The camera the level of detail was last decided for.
    ///
    /// The decision needs the model's bounds and its brick count, so it is
    /// made when the camera has actually moved rather than every frame.
    detail_camera: Option<([f32; 3], f32)>,
    window: Option<Arc<Window>>,
    graphics: Option<Graphics>,

    drag: Drag,
    /// What each frame is worth spending on, and when that may rise.
    ///
    /// Here rather than in the renderer because what the pointer is doing is
    /// this layer's knowledge: `drag` is right beside it, and a renderer that
    /// worked the answer out for itself would be a second definition of "is
    /// the user sculpting" for the two to disagree over.
    quality: QualityGovernor,
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
    /// Where the mirror planes were last drawn, so a subtool that moves
    /// rebuilds them and one that has not does not.
    overlay_frame: Option<clayspace_model::Transform>,
    /// Whether the pointer is rigging rather than sculpting.
    rigging: bool,
    /// Whether the viewport draws the rig's skin or only its ZSpheres.
    ///
    /// ZBrush's Adaptive Skin preview. On by default because the skin is what
    /// the rig is *for*; turned off, the scaffolding stands alone and a joint
    /// is easier to grab.
    skin_preview: bool,
    /// Whether the diagnostics window is open, and whether the last thing that
    /// happened in it was a copy.
    show_diagnostics: bool,
    diagnostics_copied: bool,
    /// The status area's memory figure, read on a clock rather than per frame.
    ///
    /// Here rather than at the call site because the figure has to outlive the
    /// frame that read it — that is the whole point of it — and because the
    /// document-swap path is what tells it to forget.
    memory_meter: MemoryMeter<MemoryReading>,
    /// What the operating system charges the process, read off this thread.
    footprint: clayspace_app::memory::FootprintProbe,
    /// Whether a footprint the ledger does not explain has been logged.
    footprint_watch: clayspace_app::memory::FootprintWatch,

    /// Where session state lives, when this machine has somewhere to put it.
    store: Option<SessionStore>,
    /// Whether the chrome is cleared away and only the sculpt is left.
    ///
    /// A presentation override rather than a layout: it hides the regions
    /// *without* touching the sizes and collapse states a sculptor chose, so
    /// leaving focus mode puts everything back exactly as it was. That is why
    /// it is not stored — an application that opened with its panels gone
    /// would look broken — and why it is a bool here rather than three more
    /// collapse flags in `Layout`.
    focus: bool,
    /// The brushes this sculptor starred, read at start-up and written when
    /// one is added or taken away.
    favourites: Vec<clayspace_model::ToolKind>,
    /// How wide the regions are and which are put away.
    ///
    /// Read at start-up and written when it changes. Not document state: it
    /// enters no history and no `.clay` file, which is why it lives here
    /// beside the recent list rather than in the model.
    layout: clayspace_view::Layout,
    recent: RecentDocuments,
    autosave: AutosavePolicy,
    /// When the last autosave was written, or when the session began.
    saved_at: Instant,
    /// What the previous session left, until it has been offered.
    pending_recovery: Recovery,
    /// Set by the menu's Sair, acted on where the event loop can be exited.
    quit_requested: bool,
    /// The document's scale, and what lengths are shown in.
    units: Units,
    /// What has held the interface thread longer than a frame.
    stalls: FrameLog,
    /// A stroke has ended and its clean whole-surface re-mesh has not run yet.
    ///
    /// See `flush_pending_settle`. Kept as a flag rather than done on the spot
    /// because settling costs about 29 ms — measured as 14.6 ms of
    /// `clay_document_mesh` plus 14.0 ms of full GPU upload — and paying it at
    /// the instant the pointer lifts is what a sculptor feels as the stroke
    /// sticking.
    settle_owed: bool,
    show_attribution: bool,
    /// The exchange panels and what they would do.
    show_import: bool,
    show_export: bool,
    /// What the last export turned out to be, as opposed to what it promised.
    ///
    /// Held on the application rather than recomputed, because it is the one
    /// thing in the export panel that cannot be derived from the settings: it
    /// is a property of the bytes that were written. Cleared by the next
    /// export, so it always describes the most recent file and never an older
    /// one the sculptor has stopped thinking about.
    export_findings: Vec<ExportWarning>,
    /// The conversion panel, and what it is set to.
    show_repair: bool,
    show_deform: bool,
    /// The reference images, and whether their panel is open.
    references: ReferenceViewModel,
    show_references: bool,
    /// How opaque the sculpted surface is drawn, as the sculptor set it.
    surface_opacity: clayspace_model::SurfaceOpacity,
    /// What the viewport was last given, so the pictures are re-uploaded when
    /// they change and not once a frame.
    references_drawn: Option<u64>,
    deform: clayspace_model::DeformSettings,
    show_convert: bool,
    conversion: clayspace_model::ConversionSettings,
    /// How the next mesh rebuild is made, and what the last one came to.
    ///
    /// Held here rather than in a ViewModel for the same reason the conversion
    /// settings are: nothing reaches the document until the rebuild is asked
    /// for, so there is no model state for them to be a view of.
    remesh: clayspace_model::RemeshSettings,
    remesh_outcome: Option<clayspace_model::RemeshOutcome>,
    /// Which way the last crossing went, and what it produced.
    ///
    /// Held for the same reason `remesh_outcome` is: the crossing leaves a
    /// layer and no record of having done so, and "which of these two layers
    /// did the conversion make" is a question the scene tree cannot answer on
    /// its own. Replaced by the next crossing, so it always describes the most
    /// recent one.
    crossing_outcome: Option<(clayspace_model::Direction, clayspace_model::LayerKey)>,
    import: ImportSettings,
    export: ExportSettings,
    /// What a dragging verb took hold of, and where the pointer was then.
    ///
    /// Mover, Puxar and Nudge follow the pointer from the point they took
    /// hold of; every other verb stamps where the pointer is. Held for the
    /// length of one gesture and cleared with it.
    drag_anchor: Option<([f32; 3], egui::Pos2)>,
    /// The layer being renamed and what its field holds.
    ///
    /// Held here rather than inside the widget so the View stays a pure
    /// function of state, and so the draft survives the frame it is typed on
    /// without egui owning a piece of the document's vocabulary.
    renaming: Option<(clayspace_model::LayerKey, String)>,
    /// The engine's undo depth when a rig gesture began.
    ///
    /// A rig drag edits once per sample, so a gesture is many engine entries
    /// and has to be banked as one action — the same accounting a stroke gets.
    /// Measured rather than counted: reading the depth either side says what
    /// actually happened, however many edits the gesture turned out to make.
    rig_depth_at_press: usize,
    /// The plane a rig gesture runs on: a point on it, and its normal.
    ///
    /// Fixed at the press rather than recomputed per sample. A plane that
    /// re-derives itself from the moving pointer drifts, and the sphere slides
    /// away from the cursor over a long drag.
    rig_plane: Option<([f32; 3], [f32; 3])>,
    strings: &'static Strings,

    /// The agent-facing door, as the interface sees it.
    agent: AgentViewModel,
    /// Where a request waits for the interface thread.
    agent_queue: JobQueue,
    /// The listener, while one is running.
    agent_door: Option<ServerHandle>,
    /// How the loop is woken, kept so a door shut from the menu can be opened
    /// again without restarting the application.
    agent_proxy: Option<winit::event_loop::EventLoopProxy<AgentWake>>,
    /// Whether the gesture in progress is the agent's own.
    ///
    /// Without this the mid-gesture guard refuses an agent its *own* stroke:
    /// `stroke.begin` opens a gesture, and the next call sees one open and
    /// treats it as a person's. What the guard means is "somebody is holding
    /// the pointer", and that is not the same question as "a gesture is
    /// open".
    agent_gesture: AgentGesture,
    /// What a ViewModel refused this command with, for the agent door to
    /// answer with.
    ///
    /// The interface shows a refusal in the options bar; the door has nowhere
    /// to show anything, so the error itself has to survive as far as
    /// [`Session::apply`] — which is where a refused stroke used to be
    /// reported as a success.
    sculpt_refusal: Option<ModelError>,
    /// Why the last operation the composition root ran *itself* was refused.
    ///
    /// A handful of operations belong to no ViewModel — a repair, a crossing,
    /// a pass of the active layer's stack, a rebuild — because each has an
    /// answer to carry back rather than a `Result<(), _>` to dispatch. Their
    /// refusals went to `eprintln!` and nowhere else, and stderr is not a
    /// surface: no sculptor is looking at it, and the door, which decides
    /// whether a command was refused by reading the channels the interface
    /// would have written, saw nothing written and answered success with
    /// `touched_document: true`. A repair asked for on an SDF layer, a
    /// crossing priced past its budget and a second `begin_recording` were all
    /// reported to an agent as work that had happened.
    ///
    /// An `Observable` rather than a plain `Option`, unlike `sculpt_refusal`
    /// above, because this one has two readers: the options bar draws it, and
    /// the door counts it. Nothing here is drawn twice for a repeat — see
    /// `Observable::announce`.
    operation_refusal: clayspace_vm::Observable<Option<String>>,
    /// When the clock on "an agent acted" was last advanced.
    ///
    /// Here rather than in the ViewModel because that layer has no clock,
    /// which is what lets its rules be tested without sleeping.
    agent_ticked: Instant,
    /// The interface's last frame, tessellated.
    ///
    /// Kept so a whole-window capture can run the same primitives over an
    /// offscreen target rather than building a second interface nobody drew.
    /// They are in window pixels, which is why that capture is at the window's
    /// own size whatever size was asked for — and why the answer says which.
    last_ui: Vec<egui::ClippedPrimitive>,
    last_ppp: f32,
}

struct Graphics {
    /// Nothing, for the frames that draw no surface.
    ///
    /// Held rather than made each time: the skin preview toggles often enough
    /// that allocating an empty buffer per frame would be a silly cost for a
    /// thing that never changes.
    nothing: GpuMesh,
    gpu: Gpu,
    surface: WindowSurface,
    renderer: Renderer,
    geometry: SurfaceGeometry,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

impl App {
    /// A control-point handle, as a fraction of the cage's longest side.
    ///
    /// Small enough not to hide the form it wraps, big enough to be a handle.
    const CAGE_HANDLE: f32 = 0.022;
    /// How far a manipulator axis reaches, as a multiple of a cage handle.
    ///
    /// Long enough to grab without covering the selection it sits on, which is
    /// what a person is looking at while they aim it.
    const GIZMO_REACH: f32 = 12.0;
    /// How much wider than the handle the grab radius is.
    ///
    /// Larger than what is drawn on purpose: a handle a person can see and
    /// cannot hit is worse than one drawn a little small, and this is the same
    /// slack a pointer target gets everywhere else.
    const CAGE_GRAB: f32 = 2.2;

    fn new(document: SharedDocument, policy: BackendPolicy) -> Self {
        let sculpt = SculptViewModel::new(Box::new(document.clone()));
        let scene = SceneViewModel::new(Box::new(document.clone()));
        let document_vm = DocumentViewModel::new(Box::new(document.clone()), UNTITLED);
        let mask = MaskViewModel::new(Box::new(document.clone()));
        let lattice = LatticeViewModel::new(Box::new(document.clone()));
        let objects = ObjectViewModel::new(Box::new(document.clone()));
        let boolean = BooleanViewModel::new(Box::new(document.clone()));
        let curve = CurveViewModel::new(Box::new(document.clone()));
        let cut = clayspace_vm::CutViewModel::new(Box::new(document.clone()));
        // The retopologiser is handed in rather than reached for: the
        // ViewModel depends on the domain's trait, and this is the only layer
        // that may know which engine implements it.
        let retopo = clayspace_vm::RetopoViewModel::new(
            Box::new(document.clone()),
            std::sync::Arc::new(clayspace_engine::EngineRetopologiser),
        );
        let uv = clayspace_vm::UvViewModel::new(
            Box::new(document.clone()),
            std::sync::Arc::new(clayspace_engine::EngineUnwrapper),
        );
        let conform = clayspace_vm::ConformViewModel::new(
            Box::new(document.clone()),
            std::sync::Arc::new(clayspace_engine::EngineConformer),
        );
        // The snapshotter is a closure over the document rather than a stored
        // baker: the snapshot has to be taken at the moment the sculptor asks,
        // and one taken at startup would bake the shape the session opened
        // with.
        let bake = {
            let document = document.clone();
            clayspace_vm::BakeViewModel::new(
                Box::new(document.clone()),
                Box::new(document.clone()),
                Box::new(move || {
                    document
                        .with(|d| clayspace_engine::EngineBaker::snapshot(d))
                        .map(|baker| {
                            std::sync::Arc::new(baker) as std::sync::Arc<dyn clayspace_model::Baker>
                        })
                        .map_err(|e| e.to_string())
                }),
            )
        };
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

        // The references the last session had placed, read back with their
        // pictures. A file that has since gone is dropped by the store; one
        // that will not read now is dropped here, quietly — a sculptor who
        // moved a drawing does not need to be told about it at startup.
        let mut references = ReferenceViewModel::new();
        for entry in store
            .as_ref()
            .map(SessionStore::load_references)
            .unwrap_or_default()
        {
            if let Ok(image) = clayspace_engine::read_reference(&entry.path) {
                references.place(entry.plane, Some((image, entry.path)));
                references.restore(entry.plane, entry.settings);
            }
        }

        // What the interface opens in. A choice already made wins; failing
        // that the system's own language, which `Locale::from_tag` was written
        // for and which nothing had ever called; failing that English.
        let locale = store
            .as_ref()
            .and_then(SessionStore::load_locale)
            .unwrap_or_else(|| Locale::from_tag(&system_language()));

        // How the regions were left. The design's own sizes where there is
        // nowhere to store one, or where what was stored is unreadable.
        let layout = store
            .as_ref()
            .map(SessionStore::load_layout)
            .unwrap_or_default();
        let favourites = store
            .as_ref()
            .map(SessionStore::load_favourites)
            .unwrap_or_default();
        // The profile the last session was left in, or the default.
        let profile = store
            .as_ref()
            .and_then(SessionStore::load_viewport_profile)
            .unwrap_or_default();

        Self {
            focus: false,
            favourites,
            layout,
            document,
            sculpt,
            scene,
            document_vm,
            mask,
            lattice,
            objects,
            boolean,
            curve,
            cut,
            retopo,
            uv,
            bake,
            conform,
            bake_into: None,
            curve_drag: None,
            curve_draw: None,
            last_curve_press: None,
            cage_plane: None,
            marquee: None,
            gizmo_hover: None,
            gizmo_drag: None,
            gizmo_fit: None,
            armature,
            policy,
            camera: Camera::default(),
            detail_policy: DetailPolicy::default(),
            shortcuts: Shortcuts::default(),
            mesh_revision: None,
            cage_revision: None,
            mask_revision: None,
            detail_camera: None,
            window: None,
            graphics: None,
            drag: Drag::None,
            quality: QualityGovernor::new(profile),
            hover: None,
            viewport: None,
            overlay_symmetry: [false; 3],
            overlay_frame: None,
            rigging: false,
            skin_preview: true,
            show_diagnostics: false,
            diagnostics_copied: false,
            memory_meter: MemoryMeter::default(),
            footprint: clayspace_app::memory::FootprintProbe::spawn(),
            footprint_watch: clayspace_app::memory::FootprintWatch::default(),
            store,
            recent,
            autosave: AutosavePolicy::default(),
            saved_at: Instant::now(),
            pending_recovery,
            quit_requested: false,
            units: Units::default(),
            stalls: FrameLog::default(),
            settle_owed: false,
            show_attribution: false,
            show_import: false,
            show_export: false,
            export_findings: Vec::new(),
            show_repair: false,
            show_deform: false,
            references,
            show_references: false,
            surface_opacity: clayspace_model::SurfaceOpacity::default(),
            references_drawn: None,
            deform: clayspace_model::DeformSettings::default(),
            show_convert: false,
            conversion: clayspace_model::ConversionSettings::default(),
            remesh: clayspace_model::RemeshSettings::default(),
            remesh_outcome: None,
            crossing_outcome: None,
            import: ImportSettings::default(),
            export: ExportSettings::default(),
            renaming: None,
            drag_anchor: None,
            rig_plane: None,
            rig_depth_at_press: 0,
            strings: Strings::for_locale(locale),

            agent: AgentViewModel::new(),
            agent_queue: JobQueue::new(),
            agent_door: None,
            agent_proxy: None,
            agent_gesture: AgentGesture::default(),
            sculpt_refusal: None,
            operation_refusal: clayspace_vm::Observable::new(None),
            agent_ticked: Instant::now(),
            last_ui: Vec::new(),
            last_ppp: 1.0,
        }
    }

    /// Opens the door, unless the person shut it by hand last time.
    ///
    /// Failure costs the door and not the application: a port that cannot be
    /// bound or a session directory that cannot be written is reported where
    /// errors are reported and the sculptor carries on.
    fn open_the_door(&mut self, proxy: winit::event_loop::EventLoopProxy<AgentWake>) {
        self.agent_proxy = Some(proxy.clone());
        let Some(root) = self.store.as_ref().map(|store| store.root().to_path_buf()) else {
            eprintln!("sem diretório de sessão: a porta do agente fica fechada");
            return;
        };
        if clayspace_mcp::access::door_was_shut(&root) {
            return;
        }

        let queue = self.agent_queue.clone();
        queue.set_waker(move || {
            let _ = proxy.send_event(AgentWake);
        });

        let catalogue = Arc::new(Catalogue::new(queue, &root));
        match Server::bind(catalogue) {
            Ok(server) => {
                let access = server.access().clone();
                if let Err(e) = access.publish(&root) {
                    eprintln!("o endereço do agente não pôde ser publicado: {e}");
                }
                let handle = server.serve();
                println!("agent: {}", handle.url());
                self.agent.listening(Door {
                    listening: true,
                    url: handle.url(),
                    secret: access.secret.clone(),
                    connected: 0,
                });
                self.agent_door = Some(handle);
            }
            Err(e) => eprintln!("a porta do agente não pôde ser aberta: {e}"),
        }
    }

    /// Shuts the door and takes the published address away.
    fn shut_the_door(&mut self, remember: bool) {
        if let Some(door) = self.agent_door.take() {
            door.stop();
        }
        self.agent_queue.close();
        if let Some(root) = self.store.as_ref().map(|store| store.root().to_path_buf()) {
            clayspace_mcp::Access::withdraw(&root);
            if remember {
                let _ = clayspace_mcp::access::remember_door(&root, false);
            }
        }
        self.agent.listening(Door::default());
    }

    /// Does the agent work waiting for this thread, and no more of it.
    ///
    /// Timed under its own name, so a request that holds the thread longer
    /// than a frame appears in the stall log beside every other stall rather
    /// than as an unexplained one.
    fn serve_agent(&mut self) {
        if self.agent_queue.pending() == 0 {
            self.refresh_door();
            return;
        }
        let queue = self.agent_queue.clone();
        self.timed("agente", |app| {
            queue.drain(app, report::JOBS_PER_FRAME);
        });
        self.refresh_door();
        self.request_redraw();
    }

    /// Keeps what the status area reads in step with the listener, and lets
    /// the clock on "an agent acted" run.
    ///
    /// Called on every wake, so it must cost nothing when nothing moved: the
    /// two cheap fields are compared before a `Door` is built, because
    /// building one clones the address and the secret and this runs beside
    /// every frame.
    fn refresh_door(&mut self) {
        let elapsed = self.agent_ticked.elapsed().as_secs();
        if elapsed >= 1 {
            self.agent_ticked = Instant::now();
            self.agent.tick(elapsed);
        }

        let Some(door) = self.agent_door.as_ref() else {
            return;
        };
        let listening = door.is_listening();
        let connected = door.connections();
        let shown = self.agent.door();
        if shown.listening == listening && shown.connected == connected {
            return;
        }
        let door = Door {
            listening,
            url: door.url(),
            secret: door.access().secret.clone(),
            connected,
        };
        self.agent.listening(door);
        self.request_redraw();
    }

    /// Whether a gesture a *command* can open is open in the ViewModels.
    ///
    /// The three an agent can reach: a stroke, a manipulator drag and a mask
    /// outline. Asked of the ViewModels rather than of the pointer's own
    /// record, because the pointer is not the only thing that sends commands
    /// and the record below it is only of what the pointer itself started.
    fn a_commanded_gesture_is_open(&self) -> bool {
        self.sculpt.is_stroking() || self.objects.is_dragging() || self.mask.draft().get().is_some()
    }

    /// Whether anything at all has hold of the document right now — a stroke,
    /// a manipulator drag, an outline — whoever opened it.
    ///
    /// The five `Option`s are the pointer's own record of what it started:
    /// a curve, a cage plane, a selection band and a rig plane have no
    /// ViewModel counterpart to ask.
    fn a_gesture_is_open(&self) -> bool {
        self.a_commanded_gesture_is_open()
            || self.gizmo_drag.is_some()
            || self.curve_drag.is_some()
            || self.cage_plane.is_some()
            || self.marquee.is_some()
            || self.rig_plane.is_some()
    }

    /// Whether a *person* has hold of something right now.
    ///
    /// A gesture the agent opened itself does not count, or an agent could not
    /// finish a stroke it started. A person cannot open a second gesture while
    /// the engine holds one, so the two cannot be confused in practice.
    ///
    /// This is half of the answer and not the whole of it. The agent's own
    /// gesture is still a gesture: the door asks
    /// [`App::agent_gesture_in_progress`] beside this and refuses everything
    /// but the verbs that finish it, because an exemption written as "the
    /// agent holds this one, so let it through" let `layer/add` and
    /// `history/undo` land in the middle of the agent's own stroke.
    fn holding_a_gesture(&self) -> bool {
        !self.agent_gesture.held() && self.a_gesture_is_open()
    }

    /// Whether the gesture that is open is the agent's own.
    ///
    /// Both halves, and the second is what keeps this from wedging. The flag
    /// records an *intent* — it is raised before the command is applied, so
    /// the agent's next call is not refused the stroke it is in the middle of
    /// — and a begin the ViewModel refused opened nothing. Read alone, one
    /// refused begin would stand for the rest of the session and every
    /// changing command the agent sent afterwards would be refused as landing
    /// in the middle of a gesture that does not exist.
    fn agent_gesture_in_progress(&self) -> bool {
        self.agent_gesture.held() && self.a_commanded_gesture_is_open()
    }

    /// What has not finished, in words an agent can act on.
    fn outstanding_work(&self) -> Vec<Outstanding> {
        let pending = *self.sculpt.pending_remesh().get();
        let mut outstanding = Vec::new();
        if pending > 0 {
            outstanding.push(Outstanding {
                what: format!("re-mesh of {pending} bricks"),
                fraction: None,
            });
        }
        if self.settle_owed {
            outstanding.push(Outstanding {
                what: "deferred surface settle".to_string(),
                fraction: None,
            });
        }
        // A bounded drain that stopped. An agent that asks whether the
        // document has settled has to be told about this one for the same
        // reason it is told about the pending re-mesh: the surface it would
        // measure is still catching up with the document it would measure it
        // against.
        if self.document.with(|document| document.refill_is_pending()) {
            outstanding.push(Outstanding {
                what: "refill of the surface cache".to_string(),
                fraction: None,
            });
        }
        if self.mask_revision != Some(self.document.with(|document| document.mask_revision())) {
            outstanding.push(Outstanding {
                what: "mask attribute refresh".to_string(),
                fraction: None,
            });
        }
        outstanding.extend(running_jobs(&[
            self.retopo.jobs().progress().get(),
            self.uv.jobs().progress().get(),
            self.conform.jobs().progress().get(),
            self.bake.jobs().progress().get(),
        ]));
        outstanding
    }

    /// Collect completed background work on either a frame or an agent wait.
    fn poll_jobs(&mut self) {
        self.retopo.poll();
        self.uv.poll();
        self.bake.poll();
        self.conform.poll();
    }

    /// Every channel a refusal arrives on, in the order the answer belongs to
    /// the command.
    ///
    /// The list, and not two lists. The door reads each channel's *count*
    /// before a command and its *words* after, and those were two hand-written
    /// arrays that had to agree position by position — a channel added to one
    /// and forgotten in the other would have read one ViewModel's count
    /// against another's sentence. Written once here, they agree by
    /// construction, and a channel added without widening
    /// [`NOTICE_REFUSAL_CHANNELS`] is a compile error rather than a silence.
    ///
    /// The first channel written wins, so the order is how direct an answer
    /// the channel gives: an operation the composition root ran itself is the
    /// most direct there is, and a panel's own notice the least.
    ///
    /// Everything from the cage down was missing altogether, and each was a
    /// panel writing a refusal nobody read: a boolean over a hierarchy came
    /// back as a success that produced no layer, a scale the brick cache
    /// refused came back as a scale that happened, and a refused cage apply
    /// discarded the drags without a word on any surface. Their order among
    /// themselves is not something a caller can observe — a command reaches
    /// one panel, so no two of them answer the same command.
    fn refusal_channels(&self) -> [&Observable<Option<String>>; NOTICE_REFUSAL_CHANNELS] {
        [
            &self.operation_refusal,
            self.scene.refusal(),
            self.objects.notice(),
            self.mask.notice(),
            self.document_vm.notice(),
            self.lattice.notice(),
            self.curve.notice(),
            self.boolean.notice(),
            self.armature.notice(),
            self.cut.notice(),
            self.references.notice(),
            self.retopo.notice(),
            self.uv.notice(),
            self.conform.notice(),
            self.bake.notice(),
        ]
    }

    /// Every channel a remark arrives on — something that *did* happen, said
    /// beside the answer rather than in place of it.
    fn remark_channels(&self) -> [&Observable<Option<String>>; NOTICE_REMARK_CHANNELS] {
        [
            self.sculpt.tool_status(),
            self.mask.remark(),
            // A shape parameter the field brought back inside its bounds. A
            // remark and not a refusal: the number was taken, at a value the
            // sculptor did not type, and a door that answered this as an error
            // would report a placement that happened as one that did not.
            self.objects.remark(),
        ]
    }

    /// How many times each channel a refusal or a notice arrives on has been
    /// written to.
    ///
    /// Compared either side of a command so that a notice already on screen is
    /// not reported as this command's refusal.
    ///
    /// Counts rather than revisions, because a revision deliberately does not
    /// move when a channel is written the words it already holds — that is
    /// what stops the options bar redrawing a sentence that did not change.
    /// Asking the same impossible thing twice produces the same sentence
    /// twice, and reading revisions here made the second attempt look like
    /// one nothing was said about, which this reported to the agent as
    /// success.
    fn notice_occurrences(&self) -> [u64; NOTICE_CHANNELS] {
        let mut occurrences = [0; NOTICE_CHANNELS];
        let channels = self
            .refusal_channels()
            .into_iter()
            .chain(self.remark_channels());
        for (slot, channel) in occurrences.iter_mut().zip(channels) {
            *slot = channel.occurrences();
        }
        occurrences
    }

    /// What the interface would have shown, of the channels that carry a
    /// refusal and the ones that carry a remark.
    fn notices_since(&self, before: [u64; NOTICE_CHANNELS]) -> (Option<String>, Vec<String>) {
        let now = self.notice_occurrences();
        let written = |channel: usize| now[channel] != before[channel];
        let refusals = self.refusal_channels();
        let remarks = self.remark_channels();
        let substitution = self.sculpt.substitution().map(|s| s.describe());
        notices_written(
            std::array::from_fn(|channel| (written(channel), refusals[channel].get().as_deref())),
            std::array::from_fn(|remark| {
                (
                    written(NOTICE_REFUSAL_CHANNELS + remark),
                    remark_for_an_agent(remarks[remark].get().as_deref(), substitution.as_deref()),
                )
            }),
        )
    }

    /// Records what an operation the composition root ran itself answered, and
    /// hands back what it produced.
    ///
    /// `None` is a refusal that has been *stated*, not one that has been lost:
    /// the reason goes onto the channel the options bar draws from and the
    /// door compares either side of a command, which is what makes a refused
    /// repair an error at the door rather than a success that changed nothing.
    /// It is the only thing a caller here needs to do about a refusal, and the
    /// reason none of them return one.
    ///
    /// Announced rather than set, for the reason every other refusal channel
    /// announces: asking the same impossible thing twice says the same
    /// sentence twice, and a reader that can only see the revision reads the
    /// second one as a command nothing was said about.
    fn stated<T>(&mut self, outcome: Result<T, ModelError>) -> Option<T> {
        match outcome {
            Ok(value) => {
                // Cleared by the next thing that works, so the line beside the
                // viewport belongs to the last thing that was asked rather
                // than to the last thing that failed.
                self.operation_refusal.set_if_changed(None);
                Some(value)
            }
            Err(refusal) => {
                self.operation_refusal.announce(Some(refusal.to_string()));
                None
            }
        }
    }

    /// Tells the quality governor what the pointer is doing, and the renderer
    /// what the governor decided.
    ///
    /// Asks for another frame when the answer moved. Without that this
    /// application, which draws on demand, would settle to its best quality
    /// and never draw a frame at it — the rise happens between frames, so
    /// nothing would ever show it.
    fn settle_quality(&mut self, now: Instant) {
        let state = InteractionState {
            sculpting: matches!(
                self.drag,
                Drag::Sculpt | Drag::Rig | Drag::Cage | Drag::Gizmo | Drag::Curve
            ),
            camera_moving: matches!(self.drag, Drag::Orbit | Drag::Pan),
        };
        if self.quality.observe(state, now) {
            self.request_redraw();
        }
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.renderer.set_quality(self.quality.quality());
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
        // A fresh renderer opens solid. This is also the path device-loss
        // recovery takes, so a surface the sculptor had dialled back would
        // otherwise come back solid after a driver reset.
        renderer.set_surface_opacity(self.surface_opacity);

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
        let nothing = GpuMesh::new(&gpu);
        self.graphics = Some(Graphics {
            nothing,
            gpu,
            surface,
            renderer,
            geometry,
            egui_state,
            egui_renderer,
        });

        // Timed under its own name. This meshes the whole starting form, so
        // it is always the largest re-mesh of a session — and reported as
        // "re-malha" it became the worst one in the diagnostics list every
        // time, masking whatever a drag actually cost. A hitch before the
        // first frame is a different thing from a hitch under the pointer.
        self.timed("malha inicial", Self::sync_geometry_now);
        // Finish initial attributes before serving an otherwise idle command.
        self.sync_mask();
        self.frame_all();
        true
    }

    /// Re-meshes whatever the document reports as dirty.
    fn sync_geometry(&mut self) {
        self.timed("re-malha", Self::sync_geometry_now);
    }

    fn sync_geometry_now(&mut self) {
        // Read before the sync, because acknowledging the re-mesh below is
        // what clears the pending count this reads.
        let cause = self.remesh_cause();
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let gpu = graphics.gpu.clone();
        let result = self
            .document
            .with(|document| graphics.geometry.sync(&gpu, document));
        match result {
            Ok(cost) => {
                if let Some(cost) = cost {
                    self.document.record_remesh(&cause, cost);
                }
                self.sculpt.acknowledge_remesh();
            }
            Err(e) => eprintln!("the surface could not be re-meshed: {e}"),
        }
        self.coarsen_if_over_budget();
    }

    /// What the work a re-mesh is about to do was caused by.
    ///
    /// The tool where a pending edit made the bricks dirty, because "the
    /// smooth brush is the slow one" is a sentence an engine team can act on
    /// and "the re-mesh is slow" is not. A re-mesh with nothing pending was
    /// caused by something else — an opened document, a conversion, a level
    /// of detail changing — and is named as that rather than blamed on
    /// whichever brush happened to run last.
    fn remesh_cause(&self) -> String {
        let pending = *self.sculpt.pending_remesh().get();
        match self.sculpt.last_action().get().tool.filter(|_| pending > 0) {
            Some(tool) => tool.label().to_string(),
            None => "re-malha".to_string(),
        }
    }

    /// Drops to the coarse level when the full surface is more than the
    /// device can hold, so a subtool scaled up a few times is drawn coarser
    /// rather than not at all — or, as it was, rather than ending the
    /// session in a buffer-size validation panic.
    fn coarsen_if_over_budget(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        if !graphics.geometry.over_budget() || graphics.geometry.detail() == Detail::Reduced {
            return;
        }
        let gpu = graphics.gpu.clone();
        if let Err(e) = self.document.with(|document| {
            graphics
                .geometry
                .set_detail(&gpu, document, Detail::Reduced)
        }) {
            eprintln!("o nível de detalhe não pôde ser trocado: {e}");
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
                .set_file_name(format!("{}.clayspace", self.document_display_name()))
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
    /// Where the next bake writes its maps.
    ///
    /// A **stem** rather than a file: a bake writes one image per map, named
    /// `<stem>_normal.png`, `<stem>_ao.png` and so on, so asking for a single
    /// filename would be asking for a name three of the four files will not
    /// have. The dialog is here because it is the platform's, and a ViewModel
    /// that opened one could not be exercised without a desktop.
    fn choose_bake_destination(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Onde gravar os mapas")
            .set_file_name("cozido")
            .save_file()
        else {
            return;
        };
        self.bake.bake_into(path.clone());
        self.bake_into = Some(path);
    }

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
    ///
    /// Whether one is due — and whether the clock starts again — is
    /// [`autosave_when_due`]'s, so that the ordering that made this stall can
    /// be held by a test. All this owes it is the two facts only the
    /// application knows: whether the document has unsaved work, and whether a
    /// hand is on it.
    fn maybe_autosave(&mut self) {
        let Some(store) = self.store.clone() else {
            return;
        };
        let path = store.autosave_path();
        let modified = *self.document_vm.modified().get();
        let gesture_open = self.a_gesture_is_open();
        let written = autosave_when_due(
            self.autosave,
            &mut self.saved_at,
            modified,
            gesture_open,
            || self.document_vm.autosave_to(&path),
        );
        if let Some(Err(e)) = written {
            eprintln!("a recuperação automática falhou: {e}");
        }
    }

    /// How long the event loop may sleep before an autosave could be due.
    fn autosave_deadline(&self) -> Option<Instant> {
        // Nothing is scheduled while a gesture is open. `maybe_autosave` would
        // skip the tick, and a deadline already in the past would then spin the
        // loop rather than let it sleep — for as long as a sculptor holds the
        // pointer still. The end of the gesture is an event of its own, and the
        // wait is worked out again then.
        if self.a_gesture_is_open() {
            return None;
        }
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

    /// Asks for a mesh file and brings it in as a subtool of its own.
    ///
    /// The import panel's own settings, with one overridden: an insertion is
    /// asked for as a *subtool*, and `ImportAs::Clay` samples the triangles
    /// into a field and gives up the geometry that made the model worth
    /// importing. Carried is what "brings a mesh in" means here, and it is what
    /// the spec asks for — "it stands in the scene as its own subtool, carries
    /// its geometry, and can be moved with the manipulator".
    fn insert_mesh_subtool(&mut self) {
        let settings = ImportSettings {
            becomes: clayspace_model::ImportAs::Reference,
            ..self.import
        };
        self.import_mesh_with(settings);
    }

    /// Asks for a file and brings it in.
    fn import_mesh(&mut self) {
        let settings = self.import;
        self.import_mesh_with(settings);
    }

    /// The dialog and the import behind both routes into the document.
    fn import_mesh_with(&mut self, settings: ImportSettings) {
        // Only the formats the engine actually reads. GLB is written and not
        // read, and offering it here would be a dialog that leads to a
        // refusal.
        let readable: Vec<&str> = Format::ALL
            .into_iter()
            .filter(|format| format.can_import())
            .map(|format| format.extension())
            .collect();
        let Some(path) = rfd::FileDialog::new()
            .set_title("Importar malha")
            .add_filter("Malhas", &readable)
            .pick_file()
        else {
            return;
        };
        match self.timed("importar", |app| app.document.import_mesh(&path, settings)) {
            Ok(()) => {
                self.show_import = false;
                self.scene.refresh();
                self.document_vm.touched();
                self.after_document_replaced();
            }
            Err(e) => eprintln!("não foi possível importar: {e}"),
        }
        self.request_redraw();
    }

    /// Acts on a recorded pass of the active voxel layer.
    ///
    /// Through the scene ViewModel rather than at the document directly, as
    /// the hierarchy's own stack is and for the same two reasons. The refusal
    /// is half the operation — a second `begin_recording` over an open one, a
    /// merge with nothing beneath it — and it used to be printed and nothing
    /// else, so the agent that asked was told a pass had been opened. And the
    /// ViewModel banks what the operation cost.
    ///
    /// **A pass dialled is one thing to take back.** This file used to say the
    /// opposite — that a pass is a slider rather than an undo entry — and the
    /// sculptor's experience was the argument against it: dialling a pass
    /// visibly moves the surface, and an operation that moves the surface and
    /// banks nothing leaves the next Cmd+Z spending the previous command's
    /// count. Measured in the audit: `set_strength` was not undoable at all
    /// and brought back strokes that had already been taken back.
    fn run_sculpt_layer_op(&mut self, op: clayspace_model::SculptLayerOp) {
        let changes_the_surface = op.changes_the_surface();
        let label = op.label();
        let outcome = self.scene.apply_grid_pass_op(op);
        // Banked here rather than in `dispatch_to_models`, because a pass
        // operation does not pass through it: the composition root has to know
        // whether the picture moved, so it runs this one directly. Outside the
        // branch below so that no count can be stranded by an outcome this
        // file decided not to act on; a refused operation wrote nothing and
        // therefore banks nothing on its own.
        self.bank_edits(label);
        if self.stated(outcome).is_some() {
            self.scene.refresh();
            if changes_the_surface {
                self.document_vm.touched();
                self.sync_geometry();
            }
        }
        self.request_redraw();
    }

    /// Moves the active hierarchy's levels, or changes how many it has.
    ///
    /// Through the scene ViewModel rather than the document directly, because
    /// the refusal is half the operation: adding a level is priced against a
    /// budget and refused over it rather than attempted, and a button that
    /// does nothing and says nothing is what this route exists to avoid.
    ///
    /// Only the operations that change what is drawn cost a redraw of the
    /// surface. Moving the *sculpt* level moves where the next stamp lands and
    /// nothing else — that is the whole reason a hierarchy carries two numbers
    /// — so re-meshing for it would be paying for a picture that has not
    /// changed.
    fn run_multires_level_op(&mut self, op: clayspace_model::MultiresLevelOp) {
        let redraws = op.changes_what_is_drawn();
        let outcome = self.scene.apply_level_op(op);
        // Through `stated` rather than dropped by an `.is_ok()`. The scene
        // ViewModel announces the refusal on its own channel, which both
        // readers already have, so this is the same sentence twice — and that
        // is the point: which of the composition root's operations happens to
        // be owned by a ViewModel is not something the next reader of this
        // file should have to work out before trusting that a refusal was
        // stated at all.
        if self.stated(outcome).is_some() {
            // Marked whatever it moved. Both levels are stored *inside* the
            // hierarchy's serialized bytes, so moving the sculpt level alone
            // changes what a save would write even though it changes nothing
            // that is drawn — and a document that did not know it had been
            // touched would offer to close without it.
            self.document_vm.touched();
            if redraws {
                self.sync_geometry();
            }
        }
        self.request_redraw();
    }

    /// Acts on the active hierarchy's stack of passes.
    ///
    /// Through the scene ViewModel, exactly as the levels are, and for the
    /// same reason: the refusal is half the operation. A locked pass, a slider
    /// moved before the pointer came up, a merge with nothing beneath it — all
    /// three are sentences a sculptor acts on, and all three would be a
    /// control that did nothing and said nothing if the outcome were dropped.
    ///
    /// Only three of the eleven redraw. An additive stack commutes, so sliding
    /// a pass through it moves no vertex; a rename, a lock and a change of
    /// which pass takes the next stroke move nothing either. Re-meshing for
    /// those would be paying for a picture that has not changed — on a
    /// representation where the picture is millions of vertices wide.
    fn run_multires_pass_op(&mut self, op: clayspace_model::MultiresSculptLayerOp) {
        let redraws = op.changes_the_surface();
        let outcome = self.scene.apply_sculpt_layer_op(op);
        // Stated rather than dropped, for the reason `run_multires_level_op`
        // states its own.
        if self.stated(outcome).is_some() {
            // Every one of the eleven changes what a save would write — the
            // names, the order, the strengths and the coefficients are all
            // inside the hierarchy's serialized bytes — so the document is
            // touched whether or not the picture moved. Only the three that
            // move it re-mesh.
            self.document_vm.touched();
            if redraws {
                self.sync_geometry();
            }
        }
        self.request_redraw();
    }

    /// Asks for a PNG and loads it as the alpha stamp.
    ///
    /// PNG alone in the filter, because PNG alone is read — a dialog offering
    /// what leads to a refusal is the same mistake the mesh import's filter
    /// avoids.
    fn load_alpha(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Carregar alfa")
            .add_filter("Alfas", &["png"])
            .pick_file()
        else {
            return;
        };
        match clayspace_engine::read_alpha(&path) {
            Ok(alpha) => {
                self.document
                    .with(|document| document.set_alpha(Some(alpha)));
            }
            // The refusal is a sentence naming what is wrong with *this* file,
            // built in the domain so that the same reason reaches a test.
            Err(refusal) => eprintln!("não foi possível carregar o alfa: {refusal}"),
        }
        self.request_redraw();
    }

    /// Asks for a picture and places it on one plane, behind the sculpt.
    ///
    /// The filter comes from the domain's own list of what opens, for the
    /// reason the alpha dialog gives: a dialog offering what leads to a
    /// refusal is a dialog that lies — and one that hides what would have
    /// worked is worse.
    fn load_reference(&mut self, plane: RefPlane) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.strings.action_load_reference)
            .add_filter("PNG, JPEG", &RefFormat::EXTENSIONS)
            .pick_file()
        else {
            return;
        };
        match clayspace_engine::read_reference(&path) {
            Ok(image) => self.references.place(plane, Some((image, path))),
            Err(refusal) => self.references.refuse(refusal.to_string()),
        }
        self.save_references();
        self.request_redraw();
    }

    /// Asks for a file and writes the document into it.
    fn export_mesh(&mut self) {
        let writable: Vec<&str> = Format::ALL
            .into_iter()
            .filter(|format| format.can_export())
            .map(|format| format.extension())
            .collect();
        let Some(path) = rfd::FileDialog::new()
            .set_title("Exportar malha")
            .add_filter("Malhas", &writable)
            .set_file_name(format!("{}.obj", self.document_display_name()))
            .save_file()
        else {
            return;
        };
        let settings = self.export;
        match self.timed("exportar", |app| app.document.export_mesh(&path, settings)) {
            // The panel stays OPEN when the written mesh is unsound, because
            // closing it is how the application says "that went fine" and the
            // finding has nowhere else to appear. A clean export closes it as
            // it always did.
            Ok(findings) => {
                self.show_export = !findings.is_empty();
                self.export_findings = findings;
            }
            Err(e) => {
                self.export_findings.clear();
                eprintln!("não foi possível exportar: {e}");
            }
        }
        self.request_redraw();
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

    /// Settles a cage left standing before the active subtool changes.
    ///
    /// Returns whether the switch may go ahead. A cage is a transient
    /// authoring gesture rather than per-subtool state — it is sized to what
    /// one form contains, and that box means nothing around another — so it
    /// cannot follow the sculptor across. An untouched cage is exactly the
    /// identity and is taken down without asking, which is the same bargain
    /// `apply_lattice` already makes with one. A dragged cage is work, so the
    /// sculptor says what becomes of it, and staying put is one of the
    /// answers.
    fn resolve_a_standing_cage(&mut self, incoming: clayspace_model::LayerKey) -> bool {
        let standing = {
            let cage = self.lattice.state().get();
            cage.active && cage.touched
        };
        if !standing || self.scene.scene().get().active == Some(incoming) {
            return true;
        }
        let s = self.strings;
        let answer = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title(s.cage_switch_title)
            .set_description(s.cage_switch_question)
            .set_buttons(rfd::MessageButtons::YesNoCancelCustom(
                s.cage_switch_apply.to_string(),
                s.cage_switch_drop.to_string(),
                s.cage_switch_stay.to_string(),
            ))
            .show();
        match answer {
            rfd::MessageDialogResult::Custom(chosen) if chosen == s.cage_switch_apply => {
                self.apply(Command::ApplyLattice);
                true
            }
            rfd::MessageDialogResult::Custom(chosen) if chosen == s.cage_switch_drop => {
                // The one command that takes a cage down; the model discards
                // its preview with it.
                self.apply(Command::ToggleLattice);
                true
            }
            _ => false,
        }
    }

    /// Everything that has to catch up when the document underneath changes.
    fn after_document_replaced(&mut self) {
        self.scene.refresh();
        self.mask.refresh();
        self.armature.refresh();
        // The placed objects and the meshes that could become them both belong
        // to the document that was just replaced.
        self.objects.refresh();
        self.objects.refresh_operands();
        // Rigging is a mode over a rig that no longer exists. Leaving it on
        // would hand the next click to a tree that was thrown away.
        self.rigging = self.rigging && self.armature.is_rigging();
        self.sculpt.forget_history();
        // The shelf, the brush and the symmetry toggles all belong to the
        // active subtool, and the document underneath just became a different
        // one.
        self.sculpt.refresh_for_active_layer();
        // The cage, the curve and the boolean's operands all describe the
        // document that has just been replaced.
        //
        // Reported from a session: after using the deformation cage or a tube
        // along a curve, choosing New left both still up, with their settings
        // and their control points, over a document that had never had either.
        // The boolean panel was the same omission and nobody had met it yet —
        // found by asking which view models *can* refresh rather than fixing
        // the two that were reported.
        self.lattice.refresh();
        self.curve.refresh();
        self.boolean.refresh();
        self.retopo.refresh();
        self.uv.refresh();
        self.bake.refresh();
        self.conform.refresh();
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
        // And the carried layers, which the surface rebuild does not touch.
        //
        // Forgotten rather than compared. `mesh_revision` is derived from what
        // the document holds, so two documents can report the same number —
        // one with a mesh layer and no gestures against it and one with
        // nothing at all both report zero — and the viewport would then keep
        // drawing the document that was just closed.
        self.mesh_revision = None;
        self.mask_revision = None;
        self.cage_revision = None;
        // For the same reason, and with the same word for it: the figure in
        // the status area is the closed document's memory, and its clock would
        // hold it there for up to a second beside the new document's name.
        self.memory_meter.forget();
        self.sync_mesh_layers();
        self.frame_all();
    }

    /// Hands every ViewModel's unbanked counts to the one that owns Cmd+Z.
    ///
    /// A sculptor has one Cmd+Z and does not care which part of the
    /// application produced the thing they want back, so there is one history:
    /// the sculpting ViewModel's stack of how many engine entries each action
    /// spent. A ViewModel that changed the document measures what the change
    /// cost where the change is made — see `Unbanked` — and this is where the
    /// counts are collected.
    ///
    /// One count per action, taken once: a count banked twice is one undo too
    /// many, and a ViewModel that banked nothing left the next Cmd+Z popping
    /// the *previous* command's count and spending it on entries that were not
    /// its own. That is how one undo after a cage took back the subtool the
    /// sculptor had just made.
    ///
    /// Called after the command has reached the ViewModels, and again beside
    /// the few operations the composition root runs directly — a rename, a
    /// rebuild, a reorder, a change of protection — which never pass through
    /// `dispatch_to_models`.
    ///
    /// `label` names what the sculptor did, in the interface's own words, and
    /// travels with the count onto the history. Without it the history could
    /// count the actions and not name any of them, so everything banked this
    /// way reported the *last thing that happened* as what the next undo would
    /// take back — which after an undo is "undo".
    fn bank_edits(&mut self, label: &str) {
        let counts = self
            .scene
            .take_unbanked_actions()
            .into_iter()
            .chain(self.mask.take_unbanked_actions())
            .chain(self.lattice.take_unbanked_actions())
            .chain(self.objects.take_unbanked_actions())
            .chain(self.boolean.take_unbanked_actions())
            .collect::<Vec<_>>();
        for entries in counts {
            self.sculpt.record_external_action(label, entries);
        }
    }

    /// The engine's undo depth, which is what a rig edit moves.
    fn engine_undo_depth(&self) -> usize {
        self.document
            .with(|document| clayspace_model::SculptModel::history(document).depth)
    }

    /// Everything that has to catch up after the rig changed outside a drag.
    ///
    /// `before` is the undo depth from before the edit, so this banks exactly
    /// what the edit did rather than assuming it was one entry.
    fn after_armature_edit(&mut self, label: &str, before: usize) {
        // The sculpting ViewModel owns the history Cmd+Z reads — a sculptor
        // has one undo and does not care which part of the application
        // produced the thing they want back.
        let entries = self.engine_undo_depth().saturating_sub(before);
        self.sculpt.record_external_action(label, entries);
        self.settle_geometry();
        self.scene.refresh();
        self.armature.refresh();
        self.document_vm.touched();
        // The rig's own refusal used to be printed here and nowhere else,
        // which is the same as nowhere: the options bar draws it now, and the
        // door compares the channel either side of the command. Printing it as
        // well would only put a sentence in a terminal nobody has open beside
        // the one the sculptor is reading.
        self.request_redraw();
    }

    /// Re-meshes the whole surface after a gesture, clearing the seams the
    /// per-segment path leaves.
    fn settle_geometry(&mut self) {
        self.timed("re-malha final", Self::settle_geometry_now);
        self.report_settle();
    }

    /// Spends one budget on a refill a bounded drain left standing, and asks
    /// for the next frame while there is more.
    ///
    /// The other half of [`REFILL_BUDGET`]: the drain stops, this is what
    /// starts it again. It runs before anything else in the frame so the
    /// bricks it fills are in the dirty set the geometry sync reads further
    /// down, which is what makes a long refill arrive as a surface filling in
    /// rather than as a window that has stopped.
    ///
    /// The redraw request is not optional. The loop waits on events — see
    /// `about_to_wait` — so a pump that did not ask for the next frame would
    /// leave the rest of the refill until somebody moved the mouse.
    fn pump_refill(&mut self) {
        if !self.document.with(|document| document.refill_is_pending()) {
            return;
        }
        if let Err(e) = self.document.with(ClayDocument::pump_refill) {
            eprintln!("a superfície não pôde ser recomposta: {e}");
            // And no frame is asked for. A refusal that repeats would spin the
            // loop at frame rate printing the same sentence — an idle
            // application that redraws forever is the failure `Observable`
            // exists to prevent, and a failing pump must not become one. The
            // bricks stay marked and the next thing that wakes the loop tries
            // again.
            return;
        }
        if self.document.with(|document| document.refill_is_pending()) {
            self.request_redraw();
        }
    }

    /// Pays a settle a finished stroke owed, at the top of a frame.
    ///
    /// A stroke asks for settlement after release. Independently meshed
    /// regions retain boundary copies. Document-gradient copies can be
    /// compacted exactly; preview shading still requires a complete request.
    ///
    /// Never while a live gesture is open: that would replace its preview.
    /// If synchronization already replaced every stored triangle, consuming
    /// the debt requires no additional rebuild.
    fn flush_pending_settle(&mut self) {
        if !self.settle_owed {
            return;
        }
        if !settle_is_due(
            self.settle_owed,
            self.document.with(|d| d.live_gesture_is_open()),
        ) {
            return;
        }
        self.settle_owed = false;
        // Epoch synchronization may already have rebuilt every key. Mask
        // painting also leaves a single-request surface intact.
        if self
            .graphics
            .as_ref()
            .is_some_and(|g| g.geometry.needs_settle())
        {
            self.timed("re-malha final", |app| app.settle_geometry_using(true));
            self.report_settle();
        }
    }

    /// Says what the settle just spent, and which route it took.
    ///
    /// Printed beside the stall line rather than folded into it, because the
    /// stall ledger records ONE duration per label and this is the split that
    /// duration was hiding: `re-malha final` averaged 59.4 ms over a session
    /// while ClayCore measures a whole-field mesh at 3.1 ms on a clean sphere.
    /// Those cannot both be the same work, and without the split there was no
    /// way to say whether a slow settle was the engine's call or ours around
    /// it.
    ///
    /// Only over the same threshold the ledger uses, so an ordinary settle
    /// stays silent and this cannot become the noise that teaches people to
    /// ignore the console.
    fn report_settle(&mut self) {
        let Some(graphics) = self.graphics.as_ref() else {
            return;
        };
        let Some(cost) = graphics.geometry.last_settle() else {
            return;
        };
        if cost.total_time < self.stalls.threshold() {
            return;
        }
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        // The remainder is ours and is the number worth looking at: it is
        // everything the settle spent that was neither the engine's mesh, nor
        // reading it, nor the upload.
        let ours = cost
            .total_time
            .saturating_sub(cost.engine_mesh_time)
            .saturating_sub(cost.read_time)
            .saturating_sub(cost.upload_time);
        eprintln!(
            "  re-malha final [{:?}] {:.0} ms = motor {:.0} + leitura {:.0} + envio {:.0} + resto {:.0}; {} triângulos",
            cost.route,
            ms(cost.total_time),
            ms(cost.engine_mesh_time),
            ms(cost.read_time),
            ms(cost.upload_time),
            ms(ours),
            cost.triangles,
        );
    }

    /// Brings the coarse levels up to date, once the surface has settled.
    ///
    /// Here rather than in `sync` because a coarse brick is buildable only
    /// when all eight of its children are evaluated *and* clean: dirtying any
    /// child drops its mip, so building them mid-stroke is work thrown away on
    /// the next sample.
    ///
    /// What draws them is [`App::update_detail`], since ClayCore 0.30.0 gave
    /// the meshing call a level (#93).
    fn build_mips(&mut self) {
        self.timed("níveis de detalhe", |app| {
            if let Err(e) = app.document.with(|document| document.build_mips()) {
                eprintln!("os níveis de detalhe não puderam ser construídos: {e}");
            }
        });
        // A request for the coarse surface made while the mips were still
        // down drew full resolution instead. They are up now, so the request
        // gets its second chance here rather than waiting for the camera to
        // move again.
        self.reapply_detail();
    }

    /// Draws the level the camera asks for, when the camera has moved.
    ///
    /// Guarded on movement because deciding needs the model's bounds and its
    /// brick count, and a resting camera would pay for both every frame to be
    /// told nothing changed. The policy's own hysteresis handles the rest:
    /// between the two bounds the answer is whatever it already was, so
    /// creeping across the band cannot swap the surface twice.
    fn update_detail(&mut self) {
        let here = (self.camera.target.into(), self.camera.distance);
        if self.detail_camera == Some(here) {
            return;
        }
        self.detail_camera = Some(here);

        // Extents rather than world units, so the policy reads the same on a
        // model of any size. Without bounds there is no surface to coarsen.
        let Some((min, max)) = self.sculpt.bounds() else {
            return;
        };
        let extent = (0..3).fold(0.0f32, |widest, axis| widest.max(max[axis] - min[axis]));
        if extent <= 0.0 {
            return;
        }
        let centre: [f32; 3] = std::array::from_fn(|axis| (min[axis] + max[axis]) * 0.5);
        let eye: [f32; 3] = self.camera.eye().into();
        let distance = (0..3)
            .map(|axis| (eye[axis] - centre[axis]).powi(2))
            .sum::<f32>()
            .sqrt();

        let bricks = self
            .document
            .with(|document| document.surface_brick_count());
        let current = self
            .graphics
            .as_ref()
            .map_or(Detail::Full, |graphics| graphics.geometry.detail());
        let wanted = self
            .detail_policy
            .decide(current, distance / extent, bricks);
        // Whatever the distance says, a surface the device cannot hold at full
        // resolution stays coarse.
        let wanted = if self
            .graphics
            .as_ref()
            .is_some_and(|graphics| graphics.geometry.over_budget())
        {
            Detail::Reduced
        } else {
            wanted
        };

        // Switching level is a full re-mesh, so it says so — but only when it
        // is actually switching. `set_detail` answers most calls by returning
        // false, and a cursor that flickered on every frame of an orbit would
        // be worse than no cursor at all.
        let switching = wanted != current;
        let swap = |app: &mut Self| {
            app.timed("nível de detalhe", |app| {
                let Some(graphics) = app.graphics.as_mut() else {
                    return;
                };
                let gpu = graphics.gpu.clone();
                if let Err(e) = app
                    .document
                    .with(|document| graphics.geometry.set_detail(&gpu, document, wanted))
                {
                    eprintln!("o nível de detalhe não pôde ser trocado: {e}");
                }
            });
        };
        if switching {
            self.busy(swap);
        } else {
            swap(self);
        }
    }

    /// Asks for the requested level again, once mips exist for it.
    fn reapply_detail(&mut self) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let gpu = graphics.gpu.clone();
        if let Err(e) = self
            .document
            .with(|document| graphics.geometry.reapply_detail(&gpu, document))
        {
            eprintln!("o nível de detalhe não pôde ser trocado: {e}");
        }
    }

    /// Brings the viewport's copy of the carried layers up to date.
    ///
    /// Mesh layers and voxel grids both, because neither is in the brick cache
    /// and both are drawn from this one buffer.
    ///
    /// Only when it is stale. The triangles are copied out whole rather than
    /// patched per key — and a grid is re-meshed from scratch, which is the
    /// whole grid every time (ClayCore #86 is the incremental path) — so doing
    /// it every frame would rebuild and upload the lot at sixty hertz to show
    /// a surface that has not moved.
    fn sync_mesh_layers(&mut self) {
        // Before the revision is read, so a grid that moved has its smooth
        // surface rebuilt and the revision reflects it. Cheap when nothing
        // moved — the engine compares the grid's change count first — which is
        // what lets this sit here rather than only on a settle: a surface that
        // waited for the pointer to come up would lag a whole gesture behind
        // the brush.
        if let Err(e) = self.document.with(ClayDocument::resmooth_voxels) {
            eprintln!("a malha suave não pôde ser reconstruída: {e}");
        }
        let revision = self.document.with(|document| document.mesh_revision());
        if self.mesh_revision == Some(revision) {
            return;
        }
        self.mesh_revision = Some(revision);
        self.timed("camadas de malha", |app| {
            let (vertices, indices, spans) = app.document.with(|document| {
                let (positions, normals, colors, indices, spans) = document.visible_mesh_geometry();
                // The frozen region reaches a carried layer the same way it
                // reaches the brick surface, and from the same sample call —
                // a mask is world-addressed, so it does not care which of the
                // three representations a vertex came out of.
                let frozen = document.mask_at(&positions);
                let vertices: Vec<Vertex> = positions
                    .into_iter()
                    .zip(normals)
                    .zip(colors)
                    .enumerate()
                    .map(|(at, ((position, normal), color))| Vertex {
                        position,
                        normal,
                        color,
                        mask: frozen.as_ref().map_or(0.0, |weights| weights[at]),
                    })
                    .collect();
                // The spans travel with the buffer they describe, so a range
                // can never name indices that a later rebuild moved.
                let spans: Vec<clayspace_view::MeshSpan> = spans
                    .into_iter()
                    .map(|span| {
                        // The authored edges travel with the span for the same
                        // reason the range does: they are numbered against the
                        // buffer being uploaded, so a later rebuild that moved
                        // it cannot leave them pointing at the old one.
                        clayspace_view::MeshSpan::with_edges(span.layer, span.indices, span.edges)
                    })
                    .collect();
                (vertices, indices, spans)
            });
            let Some(graphics) = app.graphics.as_mut() else {
                return;
            };
            let gpu = graphics.gpu.clone();
            graphics
                .renderer
                .set_mesh_layers(&gpu, &vertices, &indices, &spans);
        });
    }

    /// Tells the viewport which subtool a dab would land on.
    ///
    /// Its own pass, and not part of `sync_mesh_layers`: activating a subtool
    /// changes no triangle, and folding it into the buffer's staleness check
    /// would make every click re-walk and re-upload every visible grid.
    ///
    /// Nothing is cued while one layer is visible on its own. The requirement
    /// is that the active subtool be distinguishable *from the other visible
    /// ones*, and with none to be distinguished from a tint says only that the
    /// clay has changed colour.
    fn sync_active_subtool(&mut self) {
        let cued = self.cued_subtool();
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        graphics.renderer.set_active_subtool(cued);
    }

    /// The active layer, when the viewport has something to contrast it with.
    fn cued_subtool(&self) -> Option<LayerKey> {
        let scene = self.scene.scene().get();
        if scene.layers.iter().filter(|layer| layer.visible).count() < 2 {
            return None;
        }
        let active = scene.active_layer()?;
        active.visible.then_some(active.key)
    }

    /// Brings the drawn surface's idea of the frozen region up to date.
    ///
    /// Its own pass because a mask stroke moves no clay: it dirties no brick,
    /// so `sync_geometry` has nothing to re-mesh and would leave what was just
    /// painted undrawn. Only when the mask has actually changed, which is the
    /// same bargain `sync_mesh_layers` makes — this re-samples every stored
    /// vertex, and doing that at sixty hertz to show a mask nobody touched
    /// would be the whole frame budget.
    fn sync_mask(&mut self) {
        let revision = self.document.with(|document| document.mask_revision());
        if self.mask_revision == Some(revision) {
            return;
        }
        self.mask_revision = Some(revision);
        self.timed("máscara", |app| {
            let Some(graphics) = app.graphics.as_mut() else {
                return;
            };
            let gpu = graphics.gpu.clone();
            app.document
                .with(|document| graphics.geometry.refresh_mask(&gpu, document));
        });
    }

    /// Brings the drawn surface's idea of the cage up to date.
    ///
    /// Its own pass because a cage moves no clay until it is applied: the
    /// document is unchanged, so `sync_geometry` has nothing to re-mesh and
    /// the field surface would sit still while the sculptor pulled a corner
    /// across the viewport. On a mesh layer this does nothing — that route
    /// previews by being deformed, which is exact.
    fn sync_cage(&mut self) {
        let revision = self.document.with(|document| document.cage_revision());
        if self.cage_revision == Some(revision) {
            return;
        }
        self.cage_revision = Some(revision);
        self.timed("gaiola", |app| {
            let Some(graphics) = app.graphics.as_mut() else {
                return;
            };
            let gpu = graphics.gpu.clone();
            app.document
                .with(|document| graphics.geometry.preview_cage(&gpu, document));
        });
    }

    fn settle_geometry_now(&mut self) {
        self.settle_geometry_using(false);
    }

    fn settle_geometry_using(&mut self, after_edit: bool) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let gpu = graphics.gpu.clone();
        let result = self.document.with(|document| {
            if after_edit {
                graphics.geometry.settle_after_edit(&gpu, document)
            } else {
                graphics.geometry.settle(&gpu, document)
            }
        });
        match result {
            Ok(()) => self.sculpt.acknowledge_remesh(),
            Err(e) => eprintln!("the surface could not be re-meshed: {e}"),
        }
    }

    fn uploaded_bytes(&self) -> u64 {
        self.graphics
            .as_ref()
            .map_or(0, |graphics| graphics.gpu.uploaded_bytes())
    }

    /// Finish what a frame actually owes, without creating a full rebuild for
    /// a meter or an idle wait. A live gesture retains its deferred settle.
    fn finish_pending_geometry(&mut self) {
        if *self.sculpt.pending_remesh().get() > 0 {
            self.sync_geometry_now();
        }
        self.flush_pending_settle();
        // Mask painting dirties attributes, not field bricks. Include the
        // same revision-guarded refresh the next frame would otherwise do.
        self.sync_mask();
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
        Self::on_plane(self.ray_at(point)?, anchor, normal)
    }

    /// Where a ray meets a plane given by a point on it and its normal.
    ///
    /// Shared by the rig and the cage because it is the same question twice:
    /// a handle lives in three dimensions, the pointer has two, and the plane
    /// facing the camera through the handle is the one a person is picturing.
    fn on_plane(ray: ([f32; 3], [f32; 3]), anchor: [f32; 3], normal: [f32; 3]) -> Option<[f32; 3]> {
        let (origin, direction) = ray;
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
        Some(std::array::from_fn(|i| origin[i] + direction[i] * reach))
    }

    /// What a press on a curve means, decided before any of it is acted on.
    ///
    /// Pulled out of the ray arithmetic because the *order* of these questions
    /// is the whole of a bug that shipped: a press on the guide fell through
    /// to the append, so the first press of a double-click added a stray point
    /// at the end of the curve and selected it, and what a sculptor saw was a
    /// vertex jumping to the pointer instead of a point being inserted. An
    /// ordering worth getting right is worth being able to test without a
    /// window, a GPU or a camera.
    fn curve_press_action(
        double: bool,
        on_point: Option<usize>,
        on_guide: Option<usize>,
        add: bool,
    ) -> CurvePress {
        match (double, on_point, on_guide) {
            // A control point under the pointer answers first, double or not:
            // a control point sits *on* the guide, so asking the line first
            // would insert a second point coincident with every one that is
            // double-clicked. A double on a point is a double on a point.
            (_, Some(index), _) if add => CurvePress::Toggle(index),
            (_, Some(index), _) => CurvePress::Grab(index),
            (true, None, Some(sample)) => CurvePress::Insert(sample),
            // On the line, single: consumed. Appending here is what put the
            // new point at the far end of the curve.
            (false, None, Some(_)) => CurvePress::Consume,
            (_, None, None) => CurvePress::Append,
        }
    }

    /// A curve press: grab a control point, or place a new one.
    ///
    /// Before the cage and before the surface, for the reason each of those is
    /// before the next: a curve's points are drawn over everything and sit
    /// away from the form, so a press meant for one would otherwise find the
    /// clay behind it.
    fn begin_curve_press(&mut self, point: egui::Pos2, add: bool) -> bool {
        let curve = self.curve.state().get().clone();
        if !curve.active {
            return false;
        }
        let Some(ray) = self.ray_at(point) else {
            return false;
        };
        let now = std::time::Instant::now();
        let double = Self::is_double_press(self.last_curve_press, now, point);
        self.last_curve_press = Some((now, point));
        let positions: Vec<[f32; 3]> = curve.points.iter().map(|p| p.position).collect();
        let handle = Self::curve_handle(&curve);
        let reach = handle * Self::CAGE_GRAB;
        let on_point = Self::nearest_along(&positions, ray, reach);
        let guide = curve.path();
        let on_guide = Self::nearest_along(&guide, ray, reach);

        match Self::curve_press_action(double, on_point, on_guide, add) {
            CurvePress::Insert(sample) => {
                let index = curve.insertion_for_sample(sample);
                let radius = *self.curve.radius().get();
                self.handle(Command::InsertCurvePoint(index, guide[sample], radius));
                return true;
            }
            // On the line but not on a point, and not a double. Consumed, and
            // emphatically **not** appended: a press on the guide used to fall
            // through to the append below, which put a point at the *end* of
            // the curve — nowhere near the pointer — and selected it, so the
            // line appeared to jump. That is also what made a double-click
            // look like dragging a vertex, since its first press appended a
            // stray point before the second could be read as a double.
            CurvePress::Consume => return true,
            CurvePress::Append => {}
            CurvePress::Toggle(index) => {
                self.handle(Command::ToggleCurvePoint(index));
                return true;
            }
            CurvePress::Grab(index) => {
                let (_, direction) = ray;
                let normal = [-direction[0], -direction[1], -direction[2]];
                let at = positions[index];
                self.curve_drag = Self::on_plane(ray, at, normal).map(|from| (at, normal, from));
                self.handle(Command::SelectCurvePoint(Some(index)));
                return true;
            }
        }

        // Nothing under the pointer: place a point, and open a freehand
        // stroke. On the surface where the ray meets it, and otherwise on the
        // plane through the curve's last point — a curve is drawn *off* the
        // form as often as on it, and a press that missed the clay used to do
        // nothing at all.
        let radius = *self.curve.radius().get();
        let anchor = positions.last().copied().unwrap_or([0.0; 3]);
        let Some(at) = self.curve_point_at(point, anchor) else {
            return false;
        };
        self.handle(Command::AddCurvePoint(at, radius));
        // A click puts one point down; a drag lays a line of them. Nothing
        // else has to distinguish the two — a press that never moves simply
        // never reaches the spacing below.
        let (_, direction) = ray;
        self.curve_draw = Some((at, [-direction[0], -direction[1], -direction[2]]));
        true
    }

    /// Where a curve point goes for a pointer at `point`.
    ///
    /// The surface under the ray, or the camera-facing plane through `anchor`
    /// where the ray misses the clay. Shared by the press and by the freehand
    /// drag, so a stroke stays on the plane its first point chose instead of
    /// stepping off it the moment the pointer leaves the form.
    fn curve_point_at(&self, point: egui::Pos2, anchor: [f32; 3]) -> Option<[f32; 3]> {
        if let Some((hit, _)) = self.pick_at(point) {
            return Some(hit);
        }
        let ray = self.ray_at(point)?;
        let (_, direction) = ray;
        Self::on_plane(ray, anchor, [-direction[0], -direction[1], -direction[2]])
    }

    /// Lays down points while a freehand curve stroke is being dragged.
    ///
    /// Spaced rather than one per frame: a pointer sampled at sixty hertz over
    /// a slow drag would leave hundreds of control points on top of each
    /// other, which is a curve nobody can edit afterwards — and editing it
    /// afterwards is the whole difference between this tool and a brush.
    fn carry_curve_draw(&mut self, input: &ViewportInput) {
        let Some((last, normal)) = self.curve_draw else {
            return;
        };
        let Some(at) = input
            .pointer
            .and_then(|pointer| self.ray_at(pointer))
            .and_then(|ray| Self::on_plane(ray, last, normal))
        else {
            return;
        };
        let radius = *self.curve.radius().get();
        if !Self::curve_draw_reaches(last, at, radius) {
            return;
        }
        self.handle(Command::AddCurvePoint(at, radius));
        self.curve_draw = Some((at, normal));
    }

    /// Whether a freehand stroke has travelled far enough to lay another
    /// point.
    ///
    /// Spaced rather than one per frame: a pointer sampled at sixty hertz over
    /// a slow drag would leave hundreds of control points on top of each
    /// other, and a curve nobody can edit afterwards is a brush with extra
    /// steps — being able to go back to it is the whole difference.
    ///
    /// Measured in tube-widths, so a thick tube gets the coarser chain it
    /// wants and a thin one keeps its detail.
    fn curve_draw_reaches(last: [f32; 3], at: [f32; 3], radius: f32) -> bool {
        let spacing = (radius * 1.5).max(1e-2);
        let travelled: f32 = (0..3)
            .map(|axis| (at[axis] - last[axis]).powi(2))
            .sum::<f32>()
            .sqrt();
        travelled >= spacing
    }

    /// Where a curve drag has reached, as a displacement from where it began.
    fn curve_drag_to(&self, point: egui::Pos2) -> Option<[f32; 3]> {
        let (_, normal, from) = self.curve_drag?;
        let anchor = self.curve_drag?.0;
        let now = Self::on_plane(self.ray_at(point)?, anchor, normal)?;
        Some(std::array::from_fn(|axis| now[axis] - from[axis]))
    }

    /// Whether this press is the second half of a double-click.
    ///
    /// Ours rather than the window system's, because the *rule* is this
    /// application's — a double on a curve's guide splits the span under it —
    /// and a rule worth holding should be testable without a window. Both
    /// halves matter: two presses far apart in time are two presses, and two
    /// in the same instant at opposite corners of the viewport are a coincidence
    /// rather than a gesture.
    fn is_double_press(
        last: Option<(std::time::Instant, egui::Pos2)>,
        now: std::time::Instant,
        at: egui::Pos2,
    ) -> bool {
        /// The window every desktop uses, near enough.
        const WITHIN: std::time::Duration = std::time::Duration::from_millis(400);
        /// How far the pointer may drift between the two, in logical pixels.
        /// A hand resting on a tablet is not perfectly still, and a threshold
        /// of zero would make this fire only for a mouse.
        const DRIFT: f32 = 6.0;
        let Some((then, was)) = last else {
            return false;
        };
        now.duration_since(then) <= WITHIN && was.distance(at) <= DRIFT
    }

    /// How big a curve's control-point handle is, in world units.
    fn curve_handle(curve: &clayspace_model::CurveState) -> f32 {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for point in &curve.points {
            for axis in 0..3 {
                min[axis] = min[axis].min(point.position[axis]);
                max[axis] = max[axis].max(point.position[axis]);
            }
        }
        let span = (0..3)
            .map(|axis| max[axis] - min[axis])
            .fold(0.0f32, f32::max);
        // A floor, because a curve of one point has no span at all and its
        // handle still has to be grabbable.
        (span * Self::CAGE_HANDLE).max(0.02)
    }

    /// The point a ray passes nearest to, within a radius. Nearest *along* the
    /// ray, so one in front takes a press over one behind it.
    fn nearest_along(points: &[[f32; 3]], ray: ([f32; 3], [f32; 3]), reach: f32) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (index, at) in points.iter().enumerate() {
            if let Some(along) = Self::ray_hits(ray, *at, reach) {
                if best.is_none_or(|(_, closest)| along < closest) {
                    best = Some((index, along));
                }
            }
        }
        best.map(|(index, _)| index)
    }

    /// Begins a cage gesture, if the press landed on a control point.
    ///
    /// Returns whether it did. A press that misses every handle falls through
    /// to sculpting or orbiting, so a cage can be looked around without being
    /// taken down — the same bargain the rig makes.
    fn begin_cage_drag(&mut self, point: egui::Pos2, add: bool) -> bool {
        let cage = self.lattice.state().get().clone();
        if !cage.active {
            return false;
        }
        let Some((origin, direction)) = self.ray_at(point) else {
            return false;
        };
        // The nearest handle to the ray, and only if the ray actually passes
        // through it. Measured in world units against the handle's own size —
        // picking in screen space would make a distant cage unusable and a
        // close one grab points it is nowhere near.
        let reach = Self::cage_handle(&cage) * Self::CAGE_GRAB;
        let mut best: Option<(usize, f32)> = None;
        for (index, position) in cage.points.iter().enumerate() {
            let to: [f32; 3] = std::array::from_fn(|axis| position[axis] - origin[axis]);
            let along: f32 = (0..3).map(|axis| to[axis] * direction[axis]).sum();
            if along <= 0.0 {
                continue; // Behind the eye.
            }
            let miss: f32 = (0..3)
                .map(|axis| (to[axis] - direction[axis] * along).powi(2))
                .sum::<f32>()
                .sqrt();
            if miss > reach {
                continue;
            }
            // Nearest along the ray rather than nearest to it, so a near
            // handle wins over a far one the ray happens to graze more
            // centrally.
            if best.is_none_or(|(_, closest)| along < closest) {
                best = Some((index, along));
            }
        }
        let Some((index, _)) = best else {
            return false;
        };
        // The plane the drag runs on: facing the camera, through the handle
        // that was grabbed. A cage point lives in three dimensions and the
        // pointer has two, and this is the plane a person is already picturing
        // when they pull a corner sideways.
        let normal = [-direction[0], -direction[1], -direction[2]];
        self.cage_plane = Some((cage.points[index], normal));
        // Held, the press adds to the selection rather than replacing it —
        // which is the only way to build the several-point selection the
        // manipulator exists to transform. No drag follows an adding press:
        // the point that was just added is not necessarily the one in hand.
        if add {
            self.handle(Command::ToggleLatticePoint(index));
            self.cage_plane = None;
            return true;
        }
        self.handle(Command::SelectLatticePoint(Some(index)));
        true
    }

    /// Begins a manipulator drag, if the press landed on one of its handles.
    ///
    /// Before the control points, because the manipulator is drawn over them
    /// and sits on the selection: a press on the green arrow would otherwise
    /// find whichever control point happens to be behind it.
    fn begin_gizmo_drag(&mut self, point: egui::Pos2) -> bool {
        let Some((pivot, mode, reach, per_axis_scale)) = self.gizmo_target() else {
            return false;
        };
        let Some(ray) = self.ray_at(point) else {
            return false;
        };
        let Some((operation, handle)) = clayspace_app::input::handle_under(
            mode,
            per_axis_scale,
            pivot,
            reach,
            ray,
            &self.camera,
        ) else {
            return false;
        };
        // The handle chose the operation. The interface's mode follows it, so
        // the chips say what the last gesture did and what the next press on
        // the clay will do — one widget, and no step to take before a move.
        if operation != mode {
            self.handle(Command::SetGizmoMode(operation));
        }
        self.start_gizmo_drag(ray, pivot, operation, handle, reach)
    }

    /// What the manipulator is standing on this frame: where it sits, which
    /// mode it is in, how far its arms reach, and whether it offers a box per
    /// axis.
    ///
    /// A cage that is up owns the widget; a selected object owns it otherwise.
    /// They cannot both, because putting a cage up is what takes an object
    /// selection's manipulator away. Asked by the press and by the hover
    /// highlight alike, so what lights up under the pointer and what a press
    /// grabs cannot describe different widgets.
    fn gizmo_target(&mut self) -> Option<([f32; 3], clayspace_model::GizmoMode, f32, bool)> {
        let cage = self.lattice.state().get().clone();
        if cage.active {
            let pivot = cage.pivot()?;
            let reach = Self::cage_gizmo_reach(&cage, &self.camera);
            return Some((pivot, cage.mode, reach, true));
        }
        let pivot = self.objects.pivot()?;
        // Which handles the target carries is the ViewModel's answer, not a
        // rule restated here: a placed object stretches per axis, a whole
        // subtool does not, and the composition root should not be a second
        // place that knows why.
        let per_axis = self.objects.per_axis_scale();
        Some((
            pivot,
            *self.objects.mode().get(),
            self.gizmo_reach(),
            per_axis,
        ))
    }

    /// Which handle the pointer is over, for the highlight.
    ///
    /// The same question a press asks, asked every frame the pointer moves. A
    /// widget that lights the handle under the pointer says which of half a
    /// dozen overlapping targets a press will take *before* the press is made,
    /// which is most of what makes a small handle usable at all — and the
    /// drawing already had a `hovered` field that only a drag ever filled.
    fn hovered_handle(
        &mut self,
        point: egui::Pos2,
    ) -> Option<(clayspace_model::GizmoMode, clayspace_model::GizmoHandle)> {
        let (pivot, mode, reach, per_axis_scale) = self.gizmo_target()?;
        let ray = self.ray_at(point)?;
        clayspace_app::input::handle_under(mode, per_axis_scale, pivot, reach, ray, &self.camera)
    }

    /// Whether the manipulator is on something made of clay — a placed object
    /// or a whole subtool — rather than on a cage's points or a curve's.
    fn manipulating_the_clay(&self) -> bool {
        !self.lattice.state().get().active
            && matches!(
                *self.objects.target().get(),
                Some(clayspace_model::GizmoTarget::Object(_))
                    | Some(clayspace_model::GizmoTarget::Layer(_))
            )
    }

    /// Whether a whole subtool's manipulator is up — the transform *mode*.
    fn layer_manipulator_up(&self) -> bool {
        matches!(
            *self.objects.target().get(),
            Some(clayspace_model::GizmoTarget::Layer(_))
        )
    }

    /// A press on the clay while a whole subtool's manipulator is up.
    ///
    /// The handle a press on the form stands for is the mode's free one: the
    /// centre, which slides in the view plane or scales uniformly, or the
    /// outer ring, which turns in the screen plane. What ZBrush does with a
    /// drag on the canvas in gizmo mode, and what `input::press_transforms`
    /// decides the press means.
    fn begin_free_transform(&mut self, point: egui::Pos2) -> bool {
        if self.lattice.state().get().active {
            return false;
        }
        let Some(pivot) = self.objects.pivot() else {
            return false;
        };
        // The mode's free gesture: the outer ring's turn, or the centre's
        // slide or uniform scale.
        let mode = *self.objects.mode().get();
        let handle = match mode {
            clayspace_model::GizmoMode::Rotate => clayspace_model::GizmoHandle::View,
            clayspace_model::GizmoMode::Move | clayspace_model::GizmoMode::Scale => {
                clayspace_model::GizmoHandle::Centre
            }
        };
        let Some(ray) = self.ray_at(point) else {
            return false;
        };
        let reach = self.gizmo_reach();
        self.start_gizmo_drag(ray, pivot, mode, handle, reach)
    }

    /// Opens a manipulator gesture on `handle`, from wherever `ray` meets the
    /// plane that handle drags on.
    fn start_gizmo_drag(
        &mut self,
        ray: ([f32; 3], [f32; 3]),
        pivot: [f32; 3],
        mode: clayspace_model::GizmoMode,
        handle: clayspace_model::GizmoHandle,
        reach: f32,
    ) -> bool {
        // The plane the drag runs on, which the *mode* decides: a slide needs
        // a plane containing its axis and a turn needs the ring's own plane,
        // which is perpendicular to it. One answer for both is how two of the
        // three rings came to return zero degrees however far the hand moved.
        let (_, direction) = ray;
        let facing = [-direction[0], -direction[1], -direction[2]];
        // What the outer ring turns about: the direction from the selection to
        // the eye, which is the same vector the ring is drawn perpendicular to.
        // Taken here and held for the gesture, so a camera that moves mid-drag
        // does not twist the selection under a hand that has not moved.
        let view_axis = clayspace_app::input::toward_eye(&self.camera, pivot);
        let normal = clayspace_model::drag_plane(mode, handle, view_axis, facing);
        let Some(pressed) = Self::on_plane(ray, pivot, normal) else {
            return false;
        };
        let (anchor, shift) = Self::metered_anchor(mode, handle, pivot, pressed, reach, view_axis);
        self.gizmo_drag = Some(GizmoGesture {
            mode,
            handle,
            plane: (pivot, normal),
            shift,
        });
        self.handle(Command::BeginGizmoDrag(handle, anchor, view_axis));
        true
    }

    /// Where a scale gesture is taken to have started, and by how much every
    /// later pointer position is moved to match — see [`GizmoGesture::shift`].
    ///
    /// Other handles start where they were pressed and shift nothing.
    fn metered_anchor(
        mode: clayspace_model::GizmoMode,
        handle: clayspace_model::GizmoHandle,
        pivot: [f32; 3],
        pressed: [f32; 3],
        reach: f32,
        view_axis: [f32; 3],
    ) -> ([f32; 3], [f32; 3]) {
        use clayspace_model::{GizmoHandle, GizmoMode};
        if mode != GizmoMode::Scale || handle != GizmoHandle::Centre {
            return (pressed, [0.0; 3]);
        }
        let away: [f32; 3] = std::array::from_fn(|i| pressed[i] - pivot[i]);
        let length = away.iter().map(|c| c * c).sum::<f32>().sqrt();
        // A press dead on the pivot has no direction; take one across the
        // view, which lies in the drag plane facing the eye.
        let direction: [f32; 3] = if length > 1e-6 {
            std::array::from_fn(|i| away[i] / length)
        } else {
            let (across, _) = clayspace_model::perpendicular_frame(view_axis);
            across
        };
        let anchor: [f32; 3] = std::array::from_fn(|i| pivot[i] + direction[i] * reach);
        let shift: [f32; 3] = std::array::from_fn(|i| anchor[i] - pressed[i]);
        (anchor, shift)
    }

    /// How far along a ray a sphere is hit, if it is.
    ///
    /// The model's, so a control point, a curve point and a manipulator handle
    /// are all picked by one piece of arithmetic.
    fn ray_hits(ray: ([f32; 3], [f32; 3]), centre: [f32; 3], radius: f32) -> Option<f32> {
        clayspace_model::ray_hits_sphere(ray, centre, radius)
    }

    /// Where a pointer ray meets the plane the manipulator drag runs on.
    fn on_gizmo_plane(&self, point: egui::Pos2) -> Option<[f32; 3]> {
        let gesture = self.gizmo_drag?;
        let (anchor, normal) = gesture.plane;
        let at = Self::on_plane(self.ray_at(point)?, anchor, normal)?;
        Some(std::array::from_fn(|i| at[i] + gesture.shift[i]))
    }

    /// Where a pointer ray meets the plane a cage drag runs on.
    fn on_cage_plane(&self, point: egui::Pos2) -> Option<[f32; 3]> {
        let (anchor, normal) = self.cage_plane?;
        Self::on_plane(self.ray_at(point)?, anchor, normal)
    }

    /// How big a control-point handle is, in world units.
    ///
    /// From the cage's own extent rather than fixed, so a cage around a
    /// thumbnail and one around a bust both get a handle a person can hit.
    fn cage_handle(cage: &clayspace_model::LatticeState) -> f32 {
        // From the box the cage was *built* with, not from where its points
        // are now. Sized from the current extent, every handle grew whenever
        // any one of them was dragged out — so the targets a sculptor was
        // aiming at swelled under the pointer as they worked.
        (cage.rest_span * Self::CAGE_HANDLE).max(1e-4)
    }

    /// Brings the viewport's copy of the cage and the curve up to date.
    ///
    /// One overlay for both. A cage and a curve are the same picture — points
    /// with lines between them, one of them in hand — so they share the
    /// renderer's, and only one can be up at a time in any case.
    /// Puts the placed references on the viewport's planes.
    ///
    /// Guarded on the ViewModel's revision rather than run every frame: the
    /// pictures are megabytes and re-uploading one a frame would cost more
    /// than the sculpting.
    fn sync_references(&mut self) {
        let revision = self.references.revision();
        if self.references_drawn == Some(revision) {
            return;
        }
        self.references_drawn = Some(revision);
        let placed: Vec<(usize, Option<PlacedReference>)> = RefPlane::ALL
            .iter()
            .map(|&plane| {
                let settings = self.references.settings_for(plane);
                let shown = settings
                    .visible
                    .then(|| self.references.corners(plane))
                    .flatten();
                let placed = shown.and_then(|corners| {
                    let image = self.references.image(plane)?;
                    Some(PlacedReference {
                        pixels: image.pixels.clone(),
                        width: image.width,
                        height: image.height,
                        corners,
                        opacity: settings.opacity,
                    })
                });
                (plane as usize, placed)
            })
            .collect();
        let Some(graphics) = self.graphics.as_mut() else {
            // No viewport yet; the revision is left unrecorded so this runs
            // again once there is one.
            self.references_drawn = None;
            return;
        };
        let gpu = graphics.gpu.clone();
        for (plane, placed) in &placed {
            graphics.renderer.set_reference(
                &gpu,
                *plane,
                placed.as_ref().map(|placed| clayspace_view::Reference {
                    pixels: &placed.pixels,
                    width: placed.width,
                    height: placed.height,
                    corners: placed.corners,
                    opacity: placed.opacity,
                }),
            );
        }
    }

    /// Writes the placed references down, so they are there on the next run.
    fn save_references(&self) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let entries: Vec<clayspace_model::RememberedReference> = RefPlane::ALL
            .iter()
            .filter_map(|&plane| {
                Some(clayspace_model::RememberedReference {
                    plane,
                    path: self.references.path(plane)?.to_path_buf(),
                    settings: self.references.settings_for(plane),
                })
            })
            .collect();
        store.save_references(&entries);
    }

    fn sync_lattice_view(&mut self) {
        let cage = self.lattice.state().get().clone();
        let curve = self.curve.state().get().clone();
        // Copied before the viewport is reached for, which borrows `self`.
        let camera = self.camera;
        let object_pivot = self.objects.pivot();
        // Read here rather than at the draw, which borrows the graphics: the
        // reach asks the scene how big the subtool is.
        let object_reach = self.gizmo_reach();
        let outline = self.selected_outline();
        let subtool_outline = self.active_subtool_outline();
        let object_mode = *self.objects.mode().get();
        // The same question `gizmo_target` asks, so the picture and the hit
        // test offer the same handles.
        let object_per_axis = self.objects.per_axis_scale();
        // The handle in hand while a drag is under way, and the one under the
        // pointer otherwise: a gesture keeps its handle lit wherever the
        // pointer has since travelled.
        let gizmo_hovered = self
            .gizmo_drag
            .map(|gesture| (gesture.mode, gesture.handle))
            .or(self.gizmo_hover);
        let object_gizmo = object_pivot.map(|pivot| clayspace_view::GizmoView {
            pivot,
            mode: object_mode,
            reach: object_reach,
            hovered: gizmo_hovered,
            view_axis: clayspace_app::input::toward_eye(&camera, pivot),
            per_axis_scale: object_per_axis,
        });
        let curve_gizmo = if matches!(
            *self.objects.target().get(),
            Some(clayspace_model::GizmoTarget::Curve)
        ) {
            object_gizmo
        } else {
            None
        };
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };
        let gpu = graphics.gpu.clone();
        // Drawn through while a cage *or a curve* is up: half of a cage's
        // control points are behind the form, and a curve's guide runs
        // straight down the middle of the tube it describes — which is the one
        // place an opaque surface hides it completely. The scaffold shader
        // dims whatever the sculpt stands in front of, so a curve without this
        // was drawn at the dimmed alpha along its whole length.
        graphics.renderer.set_ghosted(cage.active || curve.active);
        if !cage.active {
            // The curve, which shares the overlay. Its points are drawn like a
            // cage's and joined in a chain — the control polygon rather than
            // the tessellated curve, because what a sculptor drags are the
            // points and the sweep already shows the curve itself.
            if curve.active {
                let points: Vec<[f32; 3]> =
                    curve.points.iter().map(|point| point.position).collect();
                // No chords: the guide below is the line, and drawing both put
                // two lines through the same points that agree only where the
                // curve happens to be straight.
                let edges: Vec<(u32, u32)> = Vec::new();
                let guide = curve.path();
                graphics.renderer.set_lattice(
                    &gpu,
                    clayspace_view::LatticeView {
                        points: &points,
                        edges: &edges,
                        guide: &guide,
                        selected: &curve.selection,
                        gizmo: curve_gizmo,
                        outline: None,
                        subtool_outline: None,
                        handle: Self::curve_handle(&curve),
                    },
                );
                return;
            }
            // No cage and no curve: the manipulator belongs to whatever
            // object is selected, if any. Its pivot comes from the model
            // rather than from the list, so it is where the engine has the
            // object rather than where the interface last drew it.
            graphics.renderer.set_lattice(
                &gpu,
                clayspace_view::LatticeView {
                    points: &[],
                    edges: &[],
                    guide: &[],
                    selected: &[],
                    gizmo: object_gizmo,
                    outline,
                    subtool_outline,
                    handle: 0.0,
                },
            );
            return;
        }
        let edges = cage.edges();
        let handle = Self::cage_handle(&cage);
        graphics.renderer.set_lattice(
            &gpu,
            clayspace_view::LatticeView {
                points: &cage.points,
                edges: &edges,
                guide: &[],
                selected: &cage.selection,
                gizmo: cage.pivot().map(|pivot| clayspace_view::GizmoView {
                    pivot,
                    mode: cage.mode,
                    reach: Self::cage_gizmo_reach(&cage, &camera),
                    hovered: gizmo_hovered,
                    view_axis: clayspace_app::input::toward_eye(&camera, pivot),
                    // A cage scales its own control points.
                    per_axis_scale: true,
                }),
                outline: None,
                // A cage is already the sculptor's answer to "which form am I
                // working": a box around the same form would be a second frame
                // saying the same thing.
                subtool_outline: None,
                handle,
            },
        );
    }

    /// The box the active subtool occupies, when its cue is an outline.
    ///
    /// Only for an SDF subtool. A voxel or mesh one is tinted in the carried
    /// buffer, which is the better cue — it marks the form itself rather than
    /// the air around it — and the merged SDF surface is the one picture that
    /// cannot be split per layer: the engine attributes no triangle to the
    /// layer it came from, so the box is what is left.
    fn active_subtool_outline(&self) -> Option<([f32; 3], [f32; 3])> {
        let key = self.cued_subtool()?;
        let scene = self.scene.scene().get();
        let layer = scene.layer(key)?;
        if layer.representation != Representation::Sdf {
            return None;
        }
        self.scene.layer_bounds(key)
    }

    /// The box a selected object occupies, for the viewport to outline.
    ///
    /// Computed from the shape's own measurements and its scale rather than
    /// asked of the engine: the engine's influence bound is dilated by
    /// rounding and blend support and, under the layer mirror, covers the
    /// reflection too — a box twice the width of the object and centred
    /// between the two copies. This one says where *this* shape is.
    fn selected_outline(&self) -> Option<([f32; 3], [f32; 3])> {
        let id = (*self.objects.selected().get())?;
        let objects = self.objects.objects().get();
        let object = objects.iter().find(|object| object.id == id)?;
        // The largest measurement the shape carries, which bounds every one of
        // them: a box's half-extent, a cylinder's radius or half-height, a
        // torus's major radius. Approximate on purpose — this frames the
        // object, it does not describe it.
        let reach = object
            .parameters
            .iter()
            .copied()
            .fold(0.0f32, f32::max)
            .max(1e-3)
            // The largest of the three, applied to every axis. A per-axis
            // reach would be tighter and would stop being a bound the moment
            // the object was turned — this frames the object from any angle,
            // which is what the sphere-like bound was already doing when the
            // scale was one number.
            * object.scale.iter().copied().fold(0.0f32, f32::max);
        Some((
            std::array::from_fn(|i| object.position[i] - reach),
            std::array::from_fn(|i| object.position[i] + reach),
        ))
    }

    /// How long the manipulator's arms are on a placed object, in world units.
    ///
    /// A share of the camera's distance rather than a length, so the widget is
    /// the same size *to the hand* whether the sculptor is looking at the whole
    /// form or has zoomed into a pore — which is how ZBrush's, Maya's and
    /// Blender's all behave, and what a fixed length was not: zoomed in, its
    /// arms left the screen; zoomed out, it was a speck. Not taken from the
    /// object's own size either, unlike the cage's, which scales with the box
    /// it was built around: an object may be a hundredth of the form or twice
    /// it, and a manipulator that shrank with a small one would be unusable
    /// exactly when precision matters most.
    ///
    /// The drawing and the hit test both read this, so the handle drawn and
    /// the handle grabbed cannot come apart.
    fn object_gizmo_reach(camera: &Camera) -> f32 {
        (camera.distance * Self::OBJECT_GIZMO_FRACTION).max(1e-3)
    }

    /// How long the cage manipulator's arms are.
    ///
    /// A share of the cage, floored by the same screen-constant share of the
    /// camera's distance that an object's manipulator is sized from. It was
    /// the cage's share alone, which meant the one widget in the application
    /// that still shrank with the camera: zoom out from a cage and its
    /// manipulator went with it, while the manipulator on a placed object
    /// beside it stayed the same size to the hand. Two widgets that look the
    /// same and behave differently under the same gesture.
    ///
    /// Still allowed to grow past the floor on a large cage, because the arms
    /// should reach past what they turn rather than sit as a mark in the
    /// middle of it — which is what the cage's own share is for.
    fn cage_gizmo_reach(cage: &clayspace_model::LatticeState, camera: &Camera) -> f32 {
        (Self::cage_handle(cage) * Self::GIZMO_REACH).max(Self::object_gizmo_reach(camera))
    }

    /// The manipulator's arm as a share of the distance to the camera's
    /// target. At the default distance this is the 0.45 the widget always had.
    const OBJECT_GIZMO_FRACTION: f32 = 0.11;

    /// How long the manipulator's arms are on whatever is selected.
    ///
    /// One rule for a placed object and a whole subtool alike: a share of the
    /// camera's distance, so the widget is the same size to the hand whether
    /// the sculptor is looking at the whole scene or has zoomed into a pore —
    /// see `object_gizmo_reach`. It was sized to the subtool's own box once,
    /// which left the widget on a small subtool a speck and the one on a
    /// large subtool off the screen at any zoom that showed its detail.
    fn gizmo_reach(&mut self) -> f32 {
        let floor = Self::object_gizmo_reach(&self.camera);
        // Over the whole form, as ZBrush's gizmo stands over the tool: the
        // arrows reach past the target's own box, so the widget encloses what
        // it moves rather than sitting as a mark in its middle. Never smaller
        // than the screen-constant floor — a small target keeps a usable
        // widget — and never so large it leaves the screen on a form the
        // camera is inside of.
        let half = self.target_half_extent();
        (half * Self::ENCLOSING_REACH)
            .max(floor)
            .min(self.camera.distance * Self::REACH_CEILING)
    }

    /// Half the widest extent of the manipulator's target, measured when the
    /// target was taken up and held until another is — see `gizmo_fit`.
    fn target_half_extent(&mut self) -> f32 {
        let Some(target) = *self.objects.target().get() else {
            self.gizmo_fit = None;
            return 0.0;
        };
        if let Some((fitted, half)) = self.gizmo_fit {
            if fitted == target {
                return half;
            }
        }
        let half = self
            .target_bounds()
            .map(|(min, max)| {
                (0..3)
                    .map(|i| (max[i] - min[i]) * 0.5)
                    .fold(0.0f32, f32::max)
            })
            .unwrap_or(0.0);
        self.gizmo_fit = Some((target, half));
        half
    }

    /// The box around whatever the manipulator is on, for sizing it.
    fn target_bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        match *self.objects.target().get() {
            Some(clayspace_model::GizmoTarget::Layer(key)) => self.scene.layer_bounds(key),
            Some(clayspace_model::GizmoTarget::Object(_)) => self.selected_outline(),
            _ => None,
        }
    }

    /// How far past the target's half-extent the arrows reach.
    const ENCLOSING_REACH: f32 = 1.1;
    /// The most of the camera distance the arrows may span, so the widget on
    /// a form the camera is close to stays on screen.
    const REACH_CEILING: f32 = 0.45;

    /// Whether a press on the clay should look for an object rather than
    /// starting a stroke.
    ///
    /// Only while the shapes panel is open or something is already selected.
    /// A sculptor mid-stroke must not have one turn into a selection because a
    /// placed cylinder happens to be under the brush — the same reason rigging
    /// is a mode and the cage takes the press before the clay.
    fn picking_objects(&self) -> bool {
        *self.objects.picking().get() || self.objects.selected().get().is_some()
    }

    /// Selects what the pointer is on, and makes its subtool the sculpt target.
    ///
    /// Returns whether the press was taken. A press that meets a stroke or
    /// empty space is left to fall through to sculpting or orbiting, so a form
    /// can be worked on and turned to look at without losing what is selected.
    ///
    /// A stroke is not taken either. The specification asks for a stroke to be
    /// *said* about — "an attempt to select one for transformation SHALL say
    /// so rather than doing nothing" — and not for the press: a press on the
    /// clay is a stroke, and taking that away from the brush to explain
    /// something would be the worse error. The ViewModel has raised the
    /// sentence by the time this returns.
    ///
    /// Activation rides the same press rather than getting a picker of its own.
    /// Two pickers over one click is how the picked layer and the sculpted one
    /// came to disagree, and the answer both need is the one attributed raycast
    /// this already pays for.
    fn pick_object_at(&mut self, point: egui::Pos2) -> bool {
        let Some((origin, direction)) = self.ray_at(point) else {
            return false;
        };
        // Asked only where the interface is picking objects at all: `pick_at`
        // is what raises "that cannot be transformed", and a sculptor pressing
        // on the clay must not be told that about their own strokes.
        let picked = if self.picking_objects() {
            self.objects.pick_at(origin, direction)
        } else {
            clayspace_vm::Picked::Nothing
        };
        // The layer the object pick already attributed, and a raycast of this
        // own only where it did not answer — a press the interface did not
        // pick objects for at all, or one that met no item. `pick_item`
        // attributes the layer alongside the kind, so asking the scene again
        // was paying a second attributed raycast for a question that had
        // already been answered, on every press including the start of every
        // stroke.
        let hit = picked
            .layer()
            .or_else(|| self.scene.layer_at(origin, direction));
        let activation = clayspace_app::input::activation(picked, hit);
        // A document always has a layer being sculpted, so there is no
        // activation to take away. What a press on nothing puts down is the
        // object selection, which is the one that can be empty — the rule and
        // its reasons are in `input`, where they can be exercised.
        let selection = clayspace_app::input::selection_after(
            activation,
            self.objects.selected().get().is_some(),
        );
        if let Some(id) = activation.layer() {
            self.activate(id);
        }
        if let Some(selection) = selection {
            self.apply_now(Command::SelectObject(selection));
        }
        matches!(activation, Activation::Object(_))
    }

    /// Makes a layer the sculpt target, if it is not already.
    ///
    /// Through `SelectLayer` and no other way, so the viewport and the layer
    /// stack cannot come to disagree about which subtool is being worked on.
    /// Silent when nothing changes: activation arms the mesh sculptor and
    /// re-meshes, and a press that lands on the layer already being sculpted
    /// should cost neither.
    fn activate(&mut self, key: clayspace_model::LayerKey) {
        if self.scene.scene().get().active == Some(key) {
            return;
        }
        self.handle(Command::SelectLayer(key));
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
            Grab::Move(i) | Grab::Grow(i) | Grab::Resize(i) | Grab::Insert(i) => i,
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

    /// The document's name in the interface's language, for the file dialogs.
    ///
    /// The same translation the menu bar applies, so a fresh document is
    /// offered as "Untitled.clayspace" rather than the ViewModel's marker.
    fn document_display_name(&self) -> &str {
        shell::document_display_name(self.strings, self.document_vm.name().get())
    }

    /// This build and this machine, as the window shows it.
    ///
    /// Rebuilt each frame rather than cached: it is a handful of string
    /// allocations against a frame that meshes a surface, and a cached report
    /// is one that goes stale precisely when a fallback happens — which is the
    /// moment it exists for.
    fn diagnostics(&self, stroke: StrokeSection) -> Diagnostics {
        let mut report = self.policy.diagnostics();
        report.renderer = self
            .graphics
            .as_ref()
            .map(|graphics| graphics.gpu.adapter_description());
        report.render = self.graphics.as_ref().map(|graphics| {
            graphics
                .renderer
                .diagnostics(graphics.surface.framebuffer())
        });
        report.stalls = self.stalls.lines();
        report.mesh = Some(
            self.document
                .with(|document| clayspace_model::MeshDiagnostics {
                    sculptors: document.mesh_sculptors_held(),
                    stale_seeds_rejected: document.stale_seeds_rejected(),
                }),
        );
        report.hierarchies = Some(
            self.document
                .with(|document| document.multires_diagnostics()),
        );
        // The meter's reading rather than a fresh one: this report is built
        // every frame, and the ledger is a walk of the brick cache and of
        // every surface. It is also what keeps this window, the status area
        // and an agent's `state.memory` on one figure.
        report.memory = self.memory_meter.last().ledger;
        // What is in hand, against the layer it would land on. Read out of the
        // capability table by the report itself rather than described here, so
        // that the line and the shelf cannot say different things about the
        // same tool.
        report.tool = Some(clayspace_model::ToolDiagnostics {
            tool: *self.sculpt.tool().get(),
            representation: self.sculpt.active_representation(),
        });
        // Whether a second party could have been driving this session. The
        // address and never the secret: a report is pasted into issues.
        report.agent = Some(clayspace_model::AgentDiagnostics {
            listening: self.agent.is_listening(),
            address: self.agent.door().url.clone(),
            connected: self.agent.door().connected,
            commands: self.agent.from_agent(),
        });
        // Both halves of a stroke meet in the handle every stroke passes
        // through, so the report has one thing to read rather than two.
        //
        // Summarised only where something is going to read it. Recording a
        // phase costs 18 ns and is free beside the dab it measures;
        // *summarising* the session sorts every retained window and measured
        // 0.9 ms once they fill — and this report is rebuilt every frame, so
        // folding it in unconditionally spent five per cent of every frame on
        // a section nobody had open. `profile_overhead.rs` holds both figures.
        report.stroke = match stroke {
            StrokeSection::Summarised => Some(self.document.with_profile(StrokeDiagnostics::of)),
            StrokeSection::Skipped => None,
        };
        report
    }

    /// Writes the session's profile to a file, for the engine's authors.
    ///
    /// The report on the clipboard is what a person reads; this is what a
    /// machine reads. It carries the distribution behind every phase, the
    /// conditions the figures were taken under and the shape of what was being
    /// sculpted — everything this project has otherwise had to reconstruct by
    /// hand in an upstream issue, because a follow-up question is a round trip
    /// and a round trip is where a performance report dies.
    ///
    /// Offered on a debug build as well, and told about. The identifying half
    /// of a profile is just as true there, and that is often the build
    /// somebody is running when they hit the thing worth reporting — but an
    /// unoptimised build runs this work about two and a half times slower, so
    /// a reader must be told before the file leaves the machine as well as
    /// inside it.
    fn export_profile(&mut self) {
        // The decision is the library's and is held by a test; this is only
        // the dialog that carries it.
        if profile_file::ask_before_writing() == profile_file::Ask::WarnTimingsAreNotComparable
            && !self.confirm_debug_profile()
        {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title(self.strings.action_export_profile)
            .set_file_name(profile_file::FILE_NAME)
            .add_filter("JSON", &profile_file::EXTENSIONS)
            .save_file()
        else {
            return;
        };
        let text = profile_file::render(
            &self.diagnostics(StrokeSection::Summarised),
            &self.document.profile(),
            &self.document_shape(),
        );
        // Written whole or not at all: a half-file is a file somebody attaches
        // to an issue and nobody can read.
        match profile_file::write(&path, &text) {
            Ok(()) => println!("perfil escrito em {}", path.display()),
            Err(e) => eprintln!("o perfil não pôde ser escrito: {e}"),
        }
        self.request_redraw();
    }

    /// Says what a debug build's durations are worth, before one is written.
    ///
    /// The file says the same thing twice inside itself. This is the third
    /// place, and it is the one that reaches somebody who is about to attach
    /// the file to somebody else's issue tracker.
    fn confirm_debug_profile(&self) -> bool {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title(self.strings.action_export_profile)
            .set_description(self.strings.ask_profile_from_debug)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes
    }

    /// What was being sculpted when the figures were taken.
    ///
    /// Read through the shape rather than handed the scene, so that nothing
    /// the sculptor named can reach the file: the shape has nowhere to put a
    /// name.
    fn document_shape(&self) -> DocumentShape {
        DocumentShape::of(self.scene.scene().get(), *self.sculpt.stats().get())
    }

    /// Whether a command blocks the interface long enough to say so.
    ///
    /// Measured rather than guessed: undo is 66 ms in the engine and 141 ms in
    /// the re-mesh on a 1043-brick model, and it grows with the model because
    /// `clay_document_undo` cannot say what it changed (ClayCore #210).
    /// Opening, saving and the exchange formats are unbounded — they are a file
    /// of somebody else's size.
    ///
    /// A stroke is deliberately absent. It is the one operation where a frame
    /// *is* the feedback, and swapping the cursor under a moving pointer would
    /// be worse than the wait.
    fn blocks_visibly(command: &Command) -> bool {
        matches!(
            command,
            Command::Undo
                | Command::Redo
                | Command::NewDocument
                | Command::OpenDocument
                | Command::OpenRecent(_)
                | Command::Save
                | Command::SaveAs
                | Command::RunImport
                | Command::InsertMesh
                | Command::RunExport
                // The bake samples a whole subtool's field, so it is a file of
                // somebody else's size in the same way an import is.
                | Command::CopySubtool(_)
                // Two of those bakes and a re-mesh of what they make.
                | Command::RunBoolean
        )
    }

    /// Runs something slow with the pointer saying so.
    ///
    /// The interface thread is blocked for the whole of it, so there is no
    /// frame to put a spinner in and nothing else the application can do. A
    /// cursor is what a blocked thread can still change: the request reaches
    /// the window system inside this call, and the window system draws it
    /// whether or not this thread ever produces another frame.
    ///
    /// Restored rather than left: egui sets its own cursor from the frame's
    /// output, so leaving this one in place would be corrected on the next
    /// frame and not on this one — which is exactly the frame that took a
    /// quarter of a second to arrive.
    fn busy<T>(&mut self, work: impl FnOnce(&mut Self) -> T) -> T {
        let window = self.window.clone();
        if let Some(window) = window.as_ref() {
            window.set_cursor(winit::window::Cursor::Icon(CursorIcon::Progress));
        }
        let outcome = work(self);
        if let Some(window) = window.as_ref() {
            window.set_cursor(winit::window::Cursor::Icon(CursorIcon::Default));
        }
        outcome
    }

    /// Runs something on the interface thread and records it if it stalls.
    ///
    /// Every heavy operation the pointer can reach goes through this. The
    /// name matters more than the number: "it stutters" is the most common
    /// thing a user reports and the least actionable, and a named operation
    /// with a millisecond count is a bug report.
    fn timed<T>(&mut self, operation: &str, work: impl FnOnce(&mut Self) -> T) -> T {
        let started = Instant::now();
        let outcome = work(self);
        let took = started.elapsed();
        if self.stalls.record(operation, took) {
            eprintln!(
                "a interface travou: {operation} {:.0} ms",
                took.as_secs_f64() * 1000.0
            );
        }
        outcome
    }

    /// The state the shell needs about the rig.
    fn armature_state(&self) -> ArmatureState {
        let tree = self.armature.tree().get();
        ArmatureState {
            exists: tree.is_some(),
            editing: self.rigging,
            selection: self.armature.selected().get().is_some(),
            skin_preview: self.skin_preview,
            selection_is_negative: self.armature.selected_is_negative(),
            spheres: tree.as_ref().map(|t| t.nodes.len()).unwrap_or(0),
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
    ///
    /// `&mut` for the frame a mirrored ring is reflected through, which the
    /// document answers.
    fn cursors(&mut self) -> Vec<BrushCursor> {
        // No brush ring where a press cannot leave a stroke: the transform
        // mode, where a press on the clay moves it; a cage, where a press that
        // misses a control point orbits; and a drawn mask gesture, where a
        // press begins an outline. The rule and its reasons are
        // `input::shows_the_brush_ring`.
        if !clayspace_app::input::shows_the_brush_ring(
            self.layer_manipulator_up(),
            self.lattice.state().get().active,
            self.draws_an_outline() || self.draws_a_cut(),
            self.curve.state().get().active,
        ) {
            return Vec::new();
        }
        let Some((position, normal)) = self.hover else {
            return Vec::new();
        };
        let cursor = BrushCursor {
            position,
            normal,
            radius: self.sculpt.brush().get().size,
            mirrored: false,
        };
        // Through the active subtool's own plane, which is the plane the
        // engine mirrors its items through: "the layer transform moves the
        // plane with the layer". Reflected through the world's instead, the
        // ring stood the subtool's own displacement away from where the dab
        // landed — on a form moved clear of the origin, off the form.
        let frame = self.mirror_frame();
        mirrored_cursors(cursor, *self.sculpt.symmetry().get(), frame.as_ref())
    }

    /// The frame a mirrored stroke is reflected through: the active subtool's
    /// placement, or nothing where it stands at the origin unturned.
    ///
    /// `None` rather than an identity so the drawing keeps the plain path it
    /// has always had, which is every subtool until one is dragged.
    fn mirror_frame(&mut self) -> Option<clayspace_model::Transform> {
        let key = self.scene.scene().get().active?;
        let placement = self.objects.layer_placement(key)?;
        (placement != clayspace_model::Transform::default()).then_some(placement)
    }

    /// Rebuilds the mirror-plane overlays when the symmetry changes.
    ///
    /// The planes cost a rebuild, so this is not done every frame — but it is
    /// checked every frame, because symmetry can be toggled from the options
    /// bar as well as the keyboard and neither should need a nudge to show.
    fn sync_symmetry_overlay(&mut self) {
        let symmetry = self.active_symmetry();
        // And where the mirror stands, which moves when the subtool does: the
        // planes are the layer's, so dragging a form has to rebuild them even
        // though the axes did not change.
        let frame = self.mirror_frame();
        if symmetry == self.overlay_symmetry && frame == self.overlay_frame {
            return;
        }
        self.overlay_symmetry = symmetry;
        self.overlay_frame = frame;
        if let Some(graphics) = self.graphics.as_mut() {
            let gpu = graphics.gpu.clone();
            graphics.renderer.set_overlays(
                &gpu,
                Overlays {
                    grid: true,
                    symmetry_planes: symmetry,
                    symmetry_frame: frame,
                },
                3.0,
            );
        }
    }

    /// The top-bar symmetry state. A ZSphere armature has one explicit mirror
    /// plane (X); its reflected nodes are authored by the rig rather than by
    /// the field's multi-axis stroke symmetry.
    fn active_symmetry(&self) -> [bool; 3] {
        if self.rigging {
            [*self.armature.symmetric().get(), false, false]
        } else {
            *self.sculpt.symmetry().get()
        }
    }

    /// Sends a stroke sample at the pointer.
    ///
    /// A stamping verb wants the surface under the pointer, so it picks. A
    /// *dragging* one — Mover, Puxar, Nudge — takes hold once and then follows
    /// the pointer, carrying what it took hold of along the plane it was
    /// picked on.
    ///
    /// Picking every sample is what made a drag slide rather than pull. Every
    /// position then lands *on* the surface, so the motion between two of them
    /// is a walk along it and the form is never carried anywhere — its skin
    /// stretches across it and folds over. It also ended a gesture the moment
    /// the pointer crossed the silhouette, because a pick that finds nothing
    /// sends nothing.
    fn stroke_at(&mut self, point: egui::Pos2, begin: bool, modifiers: StrokeModifiers) {
        // The tool the gesture will actually use, which is not the one on the
        // shelf when a modifier is held. Asked here because it decides where
        // the *following* samples come from: a dragging verb carries its
        // anchor across a plane, and everything else picks the surface afresh.
        // Reading the shelf instead would carry a Shift-held smooth across a
        // plane it never touches.
        let dragging = modifiers.tool(*self.sculpt.tool().get()).is_path_driven();
        let position = if begin {
            let Some((hit, _)) = self.pick_at(point) else {
                return;
            };
            self.drag_anchor = dragging.then_some((hit, point));
            hit
        } else if let Some((anchor, press)) = self.drag_anchor {
            let Some(viewport) = self.viewport else {
                return;
            };
            let Some(carried) =
                clayspace_app::input::dragged_to(&self.camera, viewport, anchor, press, point)
            else {
                return;
            };
            carried
        } else {
            let Some((hit, _)) = self.pick_at(point) else {
                return;
            };
            hit
        };

        let command = if begin {
            Command::BeginStroke {
                position,
                pressure: 1.0,
                modifiers,
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
        // Five phases, in the order a gesture happens: where the pointer
        // is, a release ending what was under way, a press choosing what
        // begins, movement carried to it, and the wheel. Each was inline
        // here, which put five levels of nesting and the whole
        // press-arbitration chain in one function. The phases share no
        // state beyond `self`, so naming them costs nothing and leaves the
        // order they run in as the only thing this function says.
        self.follow_the_pointer(input);
        self.finish_any_drag(input);
        self.begin_a_drag(input);
        self.carry_the_drag(input);
        self.zoom_the_view(input);
    }

    /// Where the cursor is, before anything acts on it.
    fn follow_the_pointer(&mut self, input: &ViewportInput) {
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
            // Which manipulator handle a press would take. Only with the
            // pointer up: during a drag the handle in hand is the one lit,
            // whatever the pointer has since travelled over.
            self.gizmo_hover = (self.drag == Drag::None)
                .then(|| self.hovered_handle(point))
                .flatten();
        }
    }

    /// A release ends whatever was under way.
    fn finish_any_drag(&mut self, input: &ViewportInput) {
        if input.released && self.drag != Drag::None {
            match self.drag {
                Drag::Sculpt => {
                    self.apply(Command::EndStroke);
                    // The hold is over. Left standing, the next gesture would
                    // follow the pointer from where this one took hold.
                    self.drag_anchor = None;
                }
                Drag::Rig => {
                    self.armature.release();
                    self.rig_plane = None;
                    // The whole gesture as one action. A drag edits once per
                    // sample, so this is usually a dozen engine entries and
                    // always exactly one Cmd+Z.
                    let entries = self
                        .engine_undo_depth()
                        .saturating_sub(self.rig_depth_at_press);
                    self.sculpt.record_external_action("rig", entries);
                    // Rigging rewrites the armature node outright and refills
                    // the box it vacated, so unlike a stroke it can leave slots
                    // for bricks the surface has moved out of. Compaction, not
                    // seams — and once per gesture, with the pointer up.
                    self.settle_geometry();
                }
                Drag::Curve => {
                    self.curve_drag = None;
                    self.curve_draw = None;
                }
                Drag::Outline => self.close_the_drawn_outline(),
                Drag::Cut => self.close_the_drawn_cut(),
                Drag::Gizmo => {
                    self.gizmo_drag = None;
                    self.handle(Command::EndGizmoDrag);
                }
                Drag::Cage => {
                    // The cage keeps its selection with the pointer up, so the
                    // point just dragged stays the one in hand — a sculptor
                    // adjusting a corner does it in several pulls, not one.
                    self.cage_plane = None;
                }
                Drag::Marquee => self.finish_marquee(),
                _ => {}
            }
            self.drag = Drag::None;
        }
    }

    /// A press, and which subsystem it belongs to.
    fn begin_a_drag(&mut self, input: &ViewportInput) {
        if let (Some(point), Some(button), true) =
            (input.pointer, input.pressed, input.over_viewport)
        {
            // Cleared here, at the top of a press, rather than when one turns
            // into a stroke. A press on a stroke *is* a press on the clay, so
            // clearing it on `Drag::Sculpt` cleared it on the very press that
            // raised it and the sentence never reached the screen — which is
            // the case `object-transform` names ("WHEN the user picks a
            // sculpting stroke in the viewport"). Clearing here keeps what a
            // press said until the next press, so it is read once and does not
            // stand over the session that follows.
            self.objects.clear_notice();
            // Rigging takes the primary button first, and only where it lands
            // on a sphere: everywhere else the camera keeps working, so a rig
            // can be turned to look at without leaving the mode.
            let rigged = self.rigging
                && button == egui::PointerButton::Primary
                && self.begin_rig(point, input);
            // A curve's points are drawn over everything and sit away from the
            // form, so they take the press before the cage and before the clay.
            let on_curve = !rigged
                && button == egui::PointerButton::Primary
                && self.begin_curve_press(point, input.smooth_modifier);
            // The manipulator before the cage's control points, because it is
            // drawn over them and sits on the selection: a press on the green
            // arrow would otherwise find whichever point is behind it.
            let manipulated = !rigged
                && !on_curve
                && button == egui::PointerButton::Primary
                && self.begin_gizmo_drag(point);
            // A press on the clay while a whole subtool's manipulator is up is
            // the mode's free gesture, not a stroke: the rule and its reasons
            // are `input::press_transforms`.
            let transformed = !rigged
                && !on_curve
                && !manipulated
                && button == egui::PointerButton::Primary
                && clayspace_app::input::press_transforms(
                    self.layer_manipulator_up() && self.pick_at(point).is_some(),
                    self.layer_manipulator_up(),
                )
                && self.begin_free_transform(point);
            // And both before the surface is asked about, because a control
            // point sits *outside* the form: a press on a corner handle would
            // otherwise find the clay behind it and start a stroke on the
            // layer the cage is there to bend.
            let caged = !rigged
                && !on_curve
                && !manipulated
                && !transformed
                && button == egui::PointerButton::Primary
                && self.begin_cage_drag(point, input.smooth_modifier);
            // A cage takes the whole viewport, so a press that took hold of
            // no handle draws a selection box over it rather than falling
            // through to an object or the camera. Turning the model is still
            // there — the secondary button and the orbit modifier both orbit
            // — so a cage can be looked at from behind without being taken
            // down, which is the bargain the miss-orbits rule was making.
            let boxing = !rigged
                && !on_curve
                && !manipulated
                && !transformed
                && !caged
                && button == egui::PointerButton::Primary
                && !input.orbit_modifier
                && self.lattice.state().get().active;
            if boxing {
                self.marquee = Some((point, point, input.smooth_modifier));
            }
            // Before the surface is asked about, and whether or not the press
            // landed on it: an outline is drawn *around* a region, so it usually
            // starts on empty space beside the form. Asking `pick_at` first
            // would send every such press to the camera instead.
            //
            // After the cage, though, and that is the cage's rule rather than a
            // preference: while one is up it owns the viewport, and
            // `press_sculpts` already refuses a stroke there — so a mask
            // gesture that took the press out from under a selection box would
            // be the one thing still reaching past a raised cage.
            let drawing_an_outline = !rigged
                && !on_curve
                && !manipulated
                && !transformed
                && !caged
                && !boxing
                && button == egui::PointerButton::Primary
                && !input.orbit_modifier
                && self.begin_outline(point, input);
            let cutting = !rigged
                && !on_curve
                && !manipulated
                && !transformed
                && !caged
                && !boxing
                && !drawing_an_outline
                && button == egui::PointerButton::Primary
                && !input.orbit_modifier
                && self.begin_cut(point);
            // Last of the four. It always runs now, because a press on
            // geometry is what makes that geometry's subtool the sculpt
            // target; whether it also *takes* the press is decided inside, and
            // stays what it was — only while a shape is being placed or one is
            // already selected, since a press on the clay is a stroke and a
            // sculptor who is sculpting must not have one turn into a selection
            // because a cylinder happens to be under the brush.
            //
            // Not while an outline is being drawn, though: it freezes a region
            // of the subtool being worked on, and a press that changed which
            // subtool that is half way into the gesture would apply the outline
            // to a form the sculptor was not aiming at.
            let picked_object = !rigged
                && !on_curve
                && !manipulated
                && !transformed
                && !caged
                && !boxing
                && !drawing_an_outline
                && button == egui::PointerButton::Primary
                && self.pick_object_at(point);
            let on_surface = !self.rigging
                && !rigged
                && !on_curve
                && !manipulated
                && !transformed
                && !caged
                && !boxing
                && !drawing_an_outline
                && !picked_object
                && self.pick_at(point).is_some();
            // In the order the flags above were decided in. They are mutually
            // exclusive by construction — each is gated on the ones before it —
            // so the order changes nothing; it is written this way so the match
            // reads as the arbitration does rather than as a second opinion
            // about it.
            let started = match button {
                _ if rigged => Drag::Rig,
                // An armature is a direct-manipulation mode. A miss belongs
                // to camera navigation, never to the active sculpt brush:
                // otherwise a click beside a ZSphere modified its skin.
                _ if self.rigging => Drag::Orbit,
                _ if on_curve => Drag::Curve,
                _ if manipulated || transformed => Drag::Gizmo,
                _ if caged => Drag::Cage,
                _ if boxing => Drag::Marquee,
                _ if cutting => Drag::Cut,
                _ if drawing_an_outline => Drag::Outline,
                egui::PointerButton::Middle => Drag::Pan,
                egui::PointerButton::Secondary => Drag::Orbit,
                // The rule lives in `input`, with its reasons and its tests.
                _ if clayspace_app::input::press_sculpts(
                    on_surface,
                    input.orbit_modifier,
                    self.lattice.state().get().active,
                ) =>
                {
                    Drag::Sculpt
                }
                _ => Drag::Orbit,
            };
            self.drag = started;
            if started == Drag::Rig {
                self.rig_depth_at_press = self.engine_undo_depth();
            }
            if started == Drag::Sculpt {
                // Read at the press and carried for the gesture: a key caught
                // or released mid-drag would change the verb under the
                // sculptor's hand, and neither reference does that.
                let modifiers = StrokeModifiers {
                    smooth: input.smooth_modifier,
                    invert: input.invert_modifier,
                };
                self.stroke_at(point, true, modifiers);
            }
        }
    }

    /// Movement, delivered to whatever the press began.
    fn carry_the_drag(&mut self, input: &ViewportInput) {
        if self.drag != Drag::None && input.delta != egui::Vec2::ZERO {
            match self.drag {
                Drag::Sculpt => self.carry_sculpt(input),
                Drag::Curve => self.carry_curve(input),
                Drag::Outline => self.carry_outline(input),
                Drag::Cut => self.carry_cut(input),
                Drag::Gizmo => self.carry_gizmo(input),
                Drag::Cage => self.carry_cage(input),
                Drag::Marquee => self.carry_marquee(input),
                Drag::Rig => self.carry_rig(input),
                Drag::Orbit => self
                    .camera
                    .orbit(input.delta.x * 0.008, input.delta.y * 0.008),
                Drag::Pan => self.camera.pan(input.delta.x, input.delta.y),
                Drag::None => {}
            }
        }
    }

    /// Movement, while a sculpt drag is under way.
    fn carry_sculpt(&mut self, input: &ViewportInput) {
        if let Some(point) = input.pointer {
            self.stroke_at(point, false, StrokeModifiers::default());
        }
    }

    /// A press with a drawn gesture in hand, which begins an outline.
    ///
    /// `false` where the mask brush is not in hand or its gesture is the
    /// brush, which is what leaves the press to the tools below.
    fn begin_outline(&mut self, point: egui::Pos2, input: &ViewportInput) -> bool {
        if !self.draws_an_outline() {
            return false;
        }
        let Some(at) = self.ndc_at(point) else {
            return false;
        };
        // Latched at the press, as a stroke's modifiers are: a key taken up
        // half way round the outline would change what it means under the hand
        // that is drawing it.
        self.handle(Command::BeginMaskOutline(at, input.invert_modifier));
        true
    }

    /// A press with the cut tool in hand, which begins a drawn cut.
    ///
    /// Before the surface and before the manipulator, as the outline is: a cut
    /// is drawn *on the view* and a press meant for one would otherwise find
    /// the clay behind it.
    fn begin_cut(&mut self, point: egui::Pos2) -> bool {
        if !self.draws_a_cut() {
            return false;
        }
        let Some(at) = self.ndc_at(point) else {
            return false;
        };
        self.handle(Command::BeginCut(at));
        true
    }

    /// Whether the next press over the viewport draws a cut.
    ///
    /// Asked by the press *and* by the brush ring, from here rather than
    /// separately — the same reason `draws_an_outline` is one function: a ring
    /// promising a stroke the press will not leave is the mistake the cage
    /// made once.
    fn draws_a_cut(&self) -> bool {
        *self.sculpt.tool().get() == clayspace_model::ToolKind::Trim
    }

    /// Movement, while a cut is being drawn.
    fn carry_cut(&mut self, input: &ViewportInput) {
        if let Some(at) = input.pointer.and_then(|point| self.ndc_at(point)) {
            self.handle(Command::ExtendCut(at));
        }
    }

    /// The pointer came up: resolve the cut on the frame it was drawn over.
    ///
    /// Nothing to cut through where there is no frame, and the draft is taken
    /// down either way, because the gesture has ended whatever came of it.
    fn close_the_drawn_cut(&mut self) {
        match self.outline_frame() {
            Some(frame) => self.busy(|app| {
                app.timed("corte desenhado", |app| {
                    app.handle(Command::EndCut(frame));
                });
            }),
            None => self.handle(Command::CancelCut),
        }
    }

    /// Whether the next press over the viewport draws an outline rather than
    /// painting.
    ///
    /// Asked by the press *and* by the brush ring, from here rather than
    /// separately: a ring that promised a stroke the press would not leave is
    /// the mistake the cage made once, and two copies of this question is how
    /// it would be made again.
    fn draws_an_outline(&self) -> bool {
        let painting = *self.sculpt.tool().get() == clayspace_model::ToolKind::Mascara;
        self.mask.draws_an_outline(painting)
    }

    /// Movement, while an outline is being drawn.
    fn carry_outline(&mut self, input: &ViewportInput) {
        if let Some(at) = input.pointer.and_then(|point| self.ndc_at(point)) {
            self.handle(Command::ExtendMaskOutline(at));
        }
    }

    /// The pointer came up: close the outline on the frame it was drawn over.
    ///
    /// Nothing happens where there is no frame — a subtool with no extent has
    /// nothing for the sweep to run through — and the outline is taken down
    /// either way, because the gesture has ended whatever came of it.
    fn close_the_drawn_outline(&mut self) {
        match self.outline_frame() {
            Some(frame) => self.busy(|app| {
                app.timed("máscara desenhada", |app| {
                    app.handle(Command::EndMaskOutline(frame));
                });
            }),
            None => self.handle(Command::CancelMaskOutline),
        }
    }

    /// The plane the outline was drawn on, in world terms.
    ///
    /// Through the active subtool, perpendicular to the view, with its `right`
    /// and `up` taken from the camera's own rays rather than rebuilt from its
    /// matrices: the outline is collected in the same normalised coordinates
    /// those rays are cast through, so deriving the frame from them is what
    /// keeps the region under the line the sculptor drew.
    fn outline_frame(&self) -> Option<clayspace_model::OutlineFrame> {
        let viewport = self.viewport?;
        let aspect = viewport.aspect_ratio();
        let through = |ndc: [f32; 2]| self.camera.ray_through(ndc, aspect);
        let (_, forward) = through([0.0, 0.0]);

        // Somewhere on the form, so the plane the outline is measured on cuts
        // through what it is being drawn around. Without a subtool to aim at,
        // what the camera is looking at.
        let anchor = self.sculpt.bounds().map_or_else(
            || self.camera.target.into(),
            |(min, max)| std::array::from_fn(|axis| (min[axis] + max[axis]) * 0.5),
        );
        let origin = Self::on_plane(through([0.0, 0.0]), anchor, forward)?;
        let across = Self::on_plane(through([1.0, 0.0]), anchor, forward)?;
        let above = Self::on_plane(through([0.0, 1.0]), anchor, forward)?;

        let (right, width) = unit(std::array::from_fn(|i| across[i] - origin[i]))?;
        let (up, height) = unit(std::array::from_fn(|i| above[i] - origin[i]))?;
        Some(clayspace_model::OutlineFrame {
            origin,
            right,
            up,
            forward,
            scale: [width, height],
        })
    }

    /// Where a viewport point sits in normalised device coordinates.
    fn ndc_at(&self, point: egui::Pos2) -> Option<[f32; 2]> {
        clayspace_app::input::ndc_at(self.viewport?, point)
    }

    /// Movement, while a curve drag is under way.
    fn carry_curve(&mut self, input: &ViewportInput) {
        // A freehand stroke and a control-point drag are the same button on
        // the same tool, told apart by what the press landed on: a drag has a
        // grabbed point, a stroke has a last-laid one, and never both.
        if self.curve_drag.is_none() {
            self.carry_curve_draw(input);
            return;
        }
        if let Some(by) = input.pointer.and_then(|point| self.curve_drag_to(point)) {
            self.handle(Command::DragCurve(by));
            // Re-anchored, because the model moves the points by a
            // displacement: sending the whole travel every frame
            // would move them by it again each time.
            if let Some((anchor, normal, _)) = self.curve_drag {
                if let Some(now) = input
                    .pointer
                    .and_then(|point| self.ray_at(point))
                    .and_then(|ray| Self::on_plane(ray, anchor, normal))
                {
                    self.curve_drag = Some((anchor, normal, now));
                }
            }
        }
    }

    /// Movement, while a gizmo drag is under way.
    fn carry_gizmo(&mut self, input: &ViewportInput) {
        if let Some(at) = input.pointer.and_then(|point| self.on_gizmo_plane(point)) {
            // Held rather than latched at the press, so the
            // modifier can be taken up part-way through a turn to
            // land it on a round number.
            self.handle(Command::DragGizmo(at, input.invert_modifier));
        }
    }

    /// Movement, while a cage drag is under way.
    fn carry_cage(&mut self, input: &ViewportInput) {
        if let Some(at) = input.pointer.and_then(|point| self.on_cage_plane(point)) {
            self.handle(Command::DragLatticePoint(at));
        }
    }

    /// Movement, while a selection box is being drawn.
    fn carry_marquee(&mut self, input: &ViewportInput) {
        if let (Some(point), Some((from, _, add))) = (input.pointer, self.marquee) {
            self.marquee = Some((from, point, add));
        }
    }

    /// A selection box, resolved where the pointer comes up.
    ///
    /// Resolved at the release rather than as it is drawn: a selection that
    /// changed under a moving band would move the manipulator to the middle of
    /// whatever was momentarily inside it, and the widget would wander across
    /// the screen while the sculptor was still drawing the box.
    fn finish_marquee(&mut self) {
        let Some((from, to, add)) = self.marquee.take() else {
            return;
        };
        let cage = self.lattice.state().get().clone();
        if !cage.active {
            return;
        }
        if !clayspace_app::input::is_a_marquee(from, to) {
            // A click that took hold of nothing. It clears the selection —
            // which is how a sculptor puts the manipulator away — unless the
            // add modifier is held, where it means "keep what you have".
            if !add {
                self.handle(Command::SelectLatticePoint(None));
            }
            return;
        }
        let Some(viewport) = self.viewport else {
            return;
        };
        let caught =
            clayspace_app::input::points_within(&self.camera, viewport, &cage.points, from, to);
        let selection = clayspace_app::input::selection_from_marquee(&cage.selection, &caught, add);
        self.handle(Command::SelectLatticePoints(selection));
    }

    /// Movement, while a rig drag is under way.
    fn carry_rig(&mut self, input: &ViewportInput) {
        if let Some(at) = input.pointer.and_then(|point| self.on_rig_plane(point)) {
            self.armature.drag(at);
            // Live, like a stroke: the surface follows the sphere
            // rather than appearing when the pointer comes up.
            self.sync_geometry();
            self.document_vm.touched();
        }
    }

    /// The wheel.
    fn zoom_the_view(&mut self, input: &ViewportInput) {
        if input.over_viewport && input.scroll != 0.0 {
            // What is under the pointer, so the zoom is aimed at it and stops
            // against it. `None` where the ray meets nothing — over empty
            // space there is nothing to stop at, and a wheel that refused to
            // move would read as broken.
            let focus = input
                .pointer
                .and_then(|point| self.pick_at(point))
                .map(|(hit, _)| hit);
            self.camera.zoom_toward(input.scroll, focus);
        }
    }

    /// Runs a layer operation, with the pointer saying it is working.
    ///
    /// Unbounded like a conversion: a repair walks the whole grid rather than
    /// what a brush reached.
    fn run_operation(&mut self, operation: LayerOperation) {
        let before = self.engine_undo_depth();
        let label = operation.label();
        let outcome = self.busy(|app| {
            app.timed(operation.label(), |app| {
                app.document
                    .with(|document| document.apply_operation(operation))
            })
        });
        // A repair refused on the representation it does not apply to — "this
        // one is SDF" — is the whole of what the caller gets back, and it used
        // to be printed and nothing else. The document was left byte-identical
        // and the answer said the opposite.
        if self.stated(outcome).is_some() {
            // Banked on the history Cmd+Z reads, as an armature edit's is.
            // A repair recorded its engine entry and pushed nothing here, so
            // the next Cmd+Z popped the PREVIOUS stroke's count and took the
            // repair back along with part of that stroke.
            self.sculpt
                .record_external_action(label, self.engine_undo_depth().saturating_sub(before));
            self.scene.refresh();
            self.document_vm.touched();
            self.sync_geometry();
            self.sync_mesh_layers();
            self.sync_mask();
        }
    }

    /// Rebuilds the active mesh layer's topology — DynaMesh.
    ///
    /// Run through the composition root rather than through the scene
    /// ViewModel's `dispatch` because it has an answer to carry back: a
    /// rebuild destroys the topology it replaces, and the outcome is the only
    /// account of what went with it.
    ///
    /// Held rather than shown once. A sculptor asks "did those two actually
    /// join?" after looking at the result, and the piece count is where that
    /// is answered — so it stays beside the button until the next rebuild
    /// replaces it.
    ///
    /// A refusal leaves the layer byte-identical, which the engine guarantees
    /// and which is what makes offering a resolution slider safe at all: the
    /// sculptor can ask for one the form will not survive and get a sentence
    /// rather than a half-rebuilt mesh.
    fn run_remesh(&mut self, key: clayspace_model::LayerKey) {
        let settings = self.remesh;
        let outcome =
            self.busy(|app| app.timed("remesh layer", |app| app.scene.remesh(key, settings)));
        // Shown by the scene ViewModel's refusal, which the shell already
        // reads: a rebuild refused for an unusable resolution is the same kind
        // of answer as a rename refused for an empty name. Stated here too, so
        // that every operation this file runs answers the same way.
        if let Some(outcome) = self.stated(outcome) {
            // Banked here because a rebuild does not pass through
            // `dispatch_to_models`: the outcome is a value the interface
            // shows, so the composition root runs it directly. The engine
            // records a rebuild as one entry and this file used to push
            // nothing for it, so the next Cmd+Z popped the PREVIOUS command's
            // count and spent it here: measured in the audit, the undo after a
            // rebuild removed two subtools the rebuild had never touched, and
            // the redo restored none of them.
            self.bank_edits(Command::RemeshLayer(key).label());
            self.remesh_outcome = Some(outcome);
            self.document_vm.touched();
            // The layer's triangles are new ones. The carried-geometry path
            // re-reads them from the document, and the statistics and the mask
            // both follow the same way a repair's do.
            self.sync_geometry();
            self.sync_mesh_layers();
            self.sync_mask();
        }
    }

    /// Opens the rename field on a layer, seeded with the name it has.
    ///
    /// Seeded rather than blank: renaming is usually a correction to what is
    /// there, and a field that clears itself makes the sculptor retype the
    /// part they were happy with.
    fn begin_rename(&mut self, key: clayspace_model::LayerKey) {
        let name = self
            .scene
            .scene()
            .get()
            .layer(key)
            .map(|layer| layer.name.clone());
        self.renaming = name.map(|name| (key, name));
    }

    /// Puts the field's name on the layer it was opened over.
    ///
    /// The refusals — an empty name, a voxel layer taking a name another one
    /// already answers to — belong to the model and are stated by it. The
    /// field stays open when one lands, so the sculptor can fix what they
    /// typed instead of having it discarded along with the reason.
    fn commit_rename(&mut self) {
        let Some((key, draft)) = self.renaming.clone() else {
            return;
        };
        match self.scene.rename(key, &draft) {
            Ok(()) => {
                // As for a rebuild: the rename is run from here rather than
                // dispatched, so the count is collected from here too.
                self.bank_edits(Command::CommitRenameLayer.label());
                self.renaming = None;
                self.document_vm.touched();
            }
            Err(e) => eprintln!("a camada não pôde ser renomeada: {e}"),
        }
    }

    /// Crosses the active layer, with the pointer saying it is working.
    ///
    /// Unbounded: the work follows the region and the resolution, not the
    /// edit, so it is one of the few things here that can take long enough to
    /// need saying. A refusal reaches the tool status the same way an engine
    /// refusal does.
    fn run_conversion(&mut self) {
        let settings = self.conversion;
        let before = self.engine_undo_depth();
        let outcome = self.busy(|app| {
            app.timed("converter", |app| {
                app.document.with(|document| {
                    let (direction, cell, blur) =
                        (settings.direction, settings.cell_size, settings.blur);
                    if settings.in_place {
                        document.convert_layer_in_place(direction, cell, blur)
                    } else {
                        document.convert_layer(direction, cell, blur)
                    }
                })
            })
        });
        // A crossing is priced before it is attempted and refused over the
        // budget — "needs 1 658 880 000 cells, past the 512 MB budget" — which
        // is a sentence a caller acts on by choosing a coarser cell. Printed
        // alone, the panel stayed open with nothing said and the agent that
        // asked was told the crossing had happened.
        if let Some(crossed) = self.stated(outcome) {
            // One undo for the whole crossing. The reported depth folds a
            // crossing's removal and reorder entries into one step, and one
            // `undo()` takes a whole crossing back, so this banks exactly one.
            // Measured before the fix: depth 1 after a stroke, still 1 after
            // the crossing, 0 after one Cmd+Z — which took the crossing and
            // most of the stroke with it.
            self.sculpt.record_external_action(
                Command::RunConversion.label(),
                self.engine_undo_depth().saturating_sub(before),
            );
            self.crossing_outcome = Some((settings.direction, crossed));
            self.show_convert = false;
            self.scene.refresh();
            self.sculpt.refresh_for_active_layer();
            self.document_vm.touched();
            self.settle_geometry();
        }
    }

    /// Carries out a shortcut's action.
    ///
    /// The table holds names rather than commands because some of these are
    /// not commands: `BrushSmaller` has to read the current size to make one,
    /// and the file actions and `Quit` reach for the platform rather than for
    /// a ViewModel. Everything that *is* a command goes through `handle`, so a
    /// shortcut and its menu item are the same dispatch.
    fn perform(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        let command = match action {
            Action::NewDocument => return self.new_document(),
            Action::OpenDocument => return self.open(),
            Action::Save => return self.save(false),
            Action::SaveAs => return self.save(true),
            Action::Quit => {
                // The same question the close button asks, for the same
                // reason.
                if self.document_vm.guard() == Guard::Clear || self.confirm_discarding_work() {
                    self.end_session();
                    event_loop.exit();
                }
                return;
            }
            // Only while there is a rig to take one out of. Bound rather than
            // guarded inside the table, because Delete means nothing else here
            // and a binding that changes meaning with the mode is the kind
            // that cannot be remapped sensibly.
            Action::RemoveZsphere if !self.rigging => return,
            Action::RemoveZsphere => Command::RemoveZsphere,
            Action::Undo => Command::Undo,
            Action::Redo => Command::Redo,
            Action::FrameAll => Command::FrameAll,
            Action::NextMaterial => Command::NextMaterial,
            Action::ToggleMaskPainting => Command::ToggleMaskPainting,
            Action::TogglePolyframe => Command::TogglePolyframe,
            Action::ViewPerspective => Command::SetViewPreset(ViewPresetKind::Perspective),
            Action::ViewFront => Command::SetViewPreset(ViewPresetKind::Front),
            Action::ViewSide => Command::SetViewPreset(ViewPresetKind::Side),
            Action::ViewTop => Command::SetViewPreset(ViewPresetKind::Top),
            Action::SymmetryX => Command::ToggleSymmetry(Axis::X),
            Action::SymmetryY => Command::ToggleSymmetry(Axis::Y),
            Action::SymmetryZ => Command::ToggleSymmetry(Axis::Z),
            Action::BrushSmaller => Command::SetBrushSize(self.sculpt.brush().get().size * 0.8),
            Action::BrushLarger => Command::SetBrushSize(self.sculpt.brush().get().size * 1.25),
            Action::ToggleSkinPreview => Command::ToggleSkinPreview,
            Action::ToggleArmatureEditing => Command::ToggleArmatureEditing,
            // Two commands rather than one, which is why these return here:
            // the key says which mode, and puts the widget up on the active
            // subtool if nothing has it yet.
            Action::TransformMove => return self.transform_mode(GizmoMode::Move),
            Action::TransformTurn => return self.transform_mode(GizmoMode::Rotate),
            Action::TransformScale => return self.transform_mode(GizmoMode::Scale),
            // Handled here rather than as a command, like Sair above it: what
            // the chrome is doing reaches no document, and `Command` lives in
            // the layer below the view that owns the regions.
            Action::ToggleFocus => {
                self.focus = !self.focus;
                return;
            }
        };
        self.handle(command);
    }

    /// Sets the manipulator's mode from the keyboard, raising the widget on
    /// the active subtool where nothing else has it.
    ///
    /// W, E and R are Maya's and Unity's keys, and pressing one there enters
    /// the mode rather than merely arming it for later — so a key pressed with
    /// no widget up puts the whole subtool's manipulator up in that mode,
    /// which is what the one chip in the options bar toggles. Where a cage, a
    /// curve or a placed object already owns the widget, the key changes that
    /// widget's mode and takes nothing away from it.
    fn transform_mode(&mut self, mode: GizmoMode) {
        self.handle(Command::SetGizmoMode(mode));
        if self.lattice.state().get().active {
            return;
        }
        if self.curve.state().get().active {
            self.handle(Command::SetGizmoTarget(Some(
                clayspace_model::GizmoTarget::Curve,
            )));
            return;
        }
        if self.objects.target().get().is_some() {
            return;
        }
        let Some(key) = self
            .scene
            .scene()
            .get()
            .active_layer()
            .map(|layer| layer.key)
        else {
            return;
        };
        self.handle(Command::SetGizmoTarget(Some(
            clayspace_model::GizmoTarget::Layer(key),
        )));
    }

    /// Dispatches a command to whichever ViewModel owns it.
    fn apply(&mut self, command: Command) {
        // Timed by the command's own name, so a stall is reported as the thing
        // the sculptor asked for rather than as an internal function. The
        // re-mesh inside it is timed separately and shows up beside it, which
        // is what says *which half* was slow.
        let label = command.label();
        if Self::blocks_visibly(&command) {
            return self.busy(|app| app.timed(label, |app| app.apply_now(command)));
        }
        self.timed(label, |app| app.apply_now(command));
    }

    /// One command, in the three phases it has always had.
    ///
    /// Named phases rather than one run of statements, because the order
    /// between them is the whole of what this function knows: every ViewModel
    /// sees the command first, the application's own state follows, and what
    /// has to be looked at again is settled last. Run in another order, a
    /// panel refreshes against a document the command has not reached yet.
    fn apply_now(&mut self, command: Command) {
        // A rig owns the same X mirror the sculptor normally uses, but it
        // authors the reflected ZSphere itself rather than asking the field
        // to duplicate a stroke. Keep that one control in the top bar and do
        // not write a field symmetry setting onto the armature layer.
        if self.rigging {
            if let Command::ToggleSymmetry(axis) = command {
                if axis == Axis::X {
                    self.armature
                        .set_symmetric(!*self.armature.symmetric().get());
                }
                self.request_redraw();
                return;
            }
        }
        self.dispatch_to_models(&command);
        self.apply_app_effects(&command);
        self.settle_after(&command);
        self.request_redraw();
    }

    /// Hands the command to every ViewModel that has an interest in it.
    ///
    /// **The scene goes first, and that is load-bearing.** It is the only
    /// ViewModel here that *moves* the active layer; every other one reads it.
    /// The sculpting ViewModel asks the document which representation is
    /// active to decide what the shelf offers and which brush settings to
    /// restore, the cage and the manipulator are handed that representation,
    /// and the mask is re-read from a document whose active layer has to be
    /// the new one. Dispatched after any of them, a `SelectLayer` left each
    /// follower set up for the subtool the sculptor had just *left*: the first
    /// stroke after every switch was made with the previous subtool's brush,
    /// tool and mirror — measured once as a new grid layer inheriting a field
    /// layer's size 100, so the first dab came out a metre across — and the
    /// next command was the first one to see a consistent document, which is
    /// why the error was always exactly one switch behind.
    fn dispatch_to_models(&mut self, command: &Command) {
        if let Err(e) = self.scene.dispatch(command) {
            eprintln!("{e}");
        }
        if let Err(e) = self.sculpt.dispatch(command.clone()) {
            // A refusal is not swallowed; the tool status carries the reason
            // to the options bar, and this records it for the log.
            eprintln!("{e}");
            // And it is kept, because the options bar is not a surface the
            // agent door can read: a stroke the ViewModel refused reported
            // `isError: false` all the way out to the client, which is what
            // made four tools with no verb on a field look like tools that
            // worked and did nothing.
            self.sculpt_refusal = Some(e);
        }
        self.mask.dispatch(command);
        // The representation is handed in rather than looked up: a cage's
        // resolution ceiling is the layer's, and the ViewModel may not reach
        // past its own interface to ask.
        self.lattice
            .dispatch(command, self.sculpt.active_representation());
        // The manipulator's commands reach both, and which of them acts is
        // decided by what has a target: a cage that is up owns the widget, and
        // a selected object owns it otherwise. They cannot both, because a
        // cage takes the selection away when it goes up.
        self.objects
            .dispatch(command, self.sculpt.active_representation());
        self.boolean.dispatch(command);
        // Every ViewModel that changed the document hands over what the change
        // cost, and the one that owns Cmd+Z banks each count as one action.
        // Once, here, rather than beside each dispatch: a command reaches one
        // of them, and the counts are taken in dispatch order so that one which
        // somehow reached two is still banked in the order it happened.
        self.bank_edits(command.label());
        // The whole-subtool manipulator lands on what a boolean left, exactly
        // as it lands on an inserted form: what arrived is a form to stand
        // somewhere, and the sculptor's next gesture is aiming it.
        if let Some(result) = self.boolean.take_result() {
            self.objects.dispatch(
                &Command::SetGizmoTarget(Some(clayspace_model::GizmoTarget::Layer(result))),
                self.sculpt.active_representation(),
            );
        }
        // The layer stack can change under a command — a conversion adds one,
        // a removal takes one away — and the operand list is drawn from it.
        if command.touches_document() {
            self.objects.refresh();
            self.objects.refresh_operands();
            // The same for the boolean panel's own list, which answers only
            // while it is open.
            self.boolean.refresh();
        }
        self.curve.dispatch(command);
        self.cut.dispatch(command);
    }

    /// The state that belongs to the application itself rather than to a
    /// ViewModel: which windows are open, and the operations the composition
    /// root runs.
    fn apply_app_effects(&mut self, command: &Command) {
        match command {
            Command::ToggleConvert => self.show_convert = !self.show_convert,
            Command::SetConversion(settings) => self.conversion = settings.sanitized(),
            Command::SetRemeshSettings(settings) => self.remesh = settings.sanitized(),
            Command::RemeshLayer(key) => self.run_remesh(*key),
            Command::RunConversion => self.run_conversion(),
            // Straight to the ViewModel that owns the job. Not `busy()` and
            // not `timed()`, unlike a conversion: this one runs off the
            // interface thread, so a busy cursor over it would be a lie about
            // where the work is and a timing around it would measure the
            // dispatch rather than the retopology.
            Command::SetRetopoSettings(_) | Command::RunRetopology | Command::CancelRetopology => {
                self.retopo.dispatch(command)
            }
            Command::SetUvSettings(_) | Command::RunUvAtlas | Command::CancelUvAtlas => {
                self.uv.dispatch(command)
            }
            // The destination is this layer's business: it owns the platform's
            // file panel, and a ViewModel that opened one could not be
            // exercised without a desktop.
            Command::ChooseBakeDestination => self.choose_bake_destination(),
            Command::SetConformSettings(_) | Command::RunConform | Command::CancelConform => {
                self.conform.dispatch(command)
            }
            Command::SetBakeSettings(_) | Command::RunBake | Command::CancelBake => {
                self.bake.dispatch(command)
            }
            Command::ToggleRepair => self.show_repair = !self.show_repair,
            Command::SculptLayer(op) => self.run_sculpt_layer_op(op.clone()),
            Command::MultiresLevel(op) => self.run_multires_level_op(*op),
            Command::MultiresSculptLayer(op) => self.run_multires_pass_op(op.clone()),
            Command::ToggleDeform => self.show_deform = !self.show_deform,
            Command::ToggleReferences => self.show_references = !self.show_references,
            Command::LoadReference(plane) => self.load_reference(*plane),
            Command::ClearReference(_) | Command::SetReferenceSettings(..) => {
                self.references.dispatch(command);
                self.save_references();
            }
            Command::SetSurfaceOpacity(opacity) => {
                self.surface_opacity = *opacity;
                if let Some(graphics) = self.graphics.as_mut() {
                    graphics.renderer.set_surface_opacity(*opacity);
                }
            }
            Command::SetDeform(settings) => self.deform = settings.sanitized(),
            // One undo step, which the engine's own path already makes: the
            // deformer records its vertex deltas exactly as a mesh stroke
            // does, so undo takes the whole deformation back.
            Command::RunDeform => self.run_operation(self.deform.operation()),
            Command::CloseHoles => self.run_operation(LayerOperation::CloseHoles { passes: 1 }),
            Command::FillVoids => self.run_operation(LayerOperation::FillVoids),
            Command::BeginRenameLayer(key) => self.begin_rename(*key),
            Command::EditLayerName(name) => {
                if let Some((_, draft)) = self.renaming.as_mut() {
                    *draft = name.clone();
                }
            }
            // Solo changes what the surface is made of without changing the
            // document, so the viewport is rebuilt here rather than by the
            // `touches_document` path — which would also mark the sculpture
            // unsaved for a way of looking at it.
            Command::SoloLayer(_) => {
                self.scene.refresh();
                self.sync_geometry();
            }
            Command::CommitRenameLayer => self.commit_rename(),
            Command::CancelRenameLayer => self.renaming = None,
            // The stack just changed under the field. Left open it would
            // commit onto a row that has moved, or onto one that is gone.
            Command::RemoveLayer(_) => self.renaming = None,
            _ => {}
        }
    }

    /// What has to be looked at again once the command has landed.
    ///
    /// A rig belongs to a layer, so choosing another layer changes which one —
    /// or whether there is one at all. History moves the rig as surely as it
    /// moves the surface, and the tree the viewport draws is read from the
    /// document rather than kept alongside it, so an undone rig edit has to be
    /// looked up again or the scaffolding keeps showing the shape that was
    /// just taken back.
    fn settle_after(&mut self, command: &Command) {
        if matches!(
            command,
            Command::SelectLayer(_) | Command::Undo | Command::Redo
        ) {
            self.refresh_rig();
            // Whether retopology is available belongs to the active subtool —
            // it rebuilds a mesh's topology, and a field is not a mesh — so it
            // is re-read wherever the active subtool can have changed. A
            // crossing changes it too, which the `touches_document` branch
            // below covers.
            self.retopo.refresh();
            self.uv.refresh();
            self.bake.refresh();
            self.conform.refresh();
        }
        if command.touches_document() {
            self.retopo.refresh();
            self.uv.refresh();
            self.bake.refresh();
            self.conform.refresh();
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
        // A manipulator drag on a placed object or a whole subtool moves the
        // clay, and its commands are not in `touches_document`: they were
        // listed beside the cage's, whose drag moves control points and not
        // the surface. Left there, the field moved under a picture that did
        // not — the arrow was dragged and nothing happened on screen, and the
        // next stroke, aimed by a ray through the moved field, landed beside
        // the drawn form. So the surface is re-meshed on every frame of such a
        // drag here, and the document is marked unsaved once, when it ends.
        match gizmo_geometry_update(
            command,
            self.manipulating_the_clay(),
            self.sculpt.active_representation(),
        ) {
            GizmoGeometryUpdate::None => {}
            GizmoGeometryUpdate::Incremental => {
                self.sync_geometry();
            }
            GizmoGeometryUpdate::Settle => {
                self.settle_geometry();
            }
        }
        if matches!(command, Command::EndGizmoDrag) && self.manipulating_the_clay() {
            self.scene.refresh();
            self.document_vm.touched();
        }
        // The coarse levels cannot be built mid-stroke: dirtying any child
        // drops its mip.
        // However a gesture ends, the hold ends with it — a cancelled one as
        // much as a finished one.
        if matches!(command, Command::EndStroke | Command::CancelStroke) {
            self.drag_anchor = None;
            // Partial requests can retain boundary copies from older
            // requests. The deferred flush checks whether synchronization
            // already replaced the complete surface, then compacts eligible
            // document geometry or rebuilds the remaining cases.
            if matches!(command, Command::EndStroke)
                && self.sculpt.active_representation() == Representation::Sdf
            {
                // OWED, NOT PAID. The brick-meshed surface the drag already
                // drew is what stays on screen for one more frame, and the
                // required compaction or rebuild lands on the next one.
                // `build_mips` below is deferred for its own reason and this
                // follows it.
                self.settle_owed = true;
                self.request_redraw();
            }
            self.build_mips();
        }
    }

    /// Looks the rig up again, and leaves rigging mode if there is none.
    fn refresh_rig(&mut self) {
        self.armature.refresh();
        self.rigging = self.rigging && self.armature.is_rigging();
    }

    /// The memory ledger, for the status area, the diagnostics and an agent.
    ///
    /// The engine is asked at most once a second, because asking it walks the
    /// whole brick cache — see [`MemoryMeter`], which is where that story is
    /// told. Each fresh reading is also checked against the process
    /// footprint, and a footprint the ledger does not explain is logged with
    /// its breakdown: that comparison is what notices memory nobody counts.
    fn memory_reading(&mut self, now: Instant) -> MemoryReading {
        let Self {
            memory_meter,
            document,
            graphics,
            footprint,
            footprint_watch,
            ..
        } = self;
        memory_meter.figures(now, || {
            let drawing = graphics
                .as_ref()
                .map(|graphics| clayspace_app::memory::drawing(&graphics.geometry, &graphics.gpu))
                .unwrap_or_default();
            let ledger =
                document.with(|document| clayspace_app::memory::ledger(document, drawing))?;
            let footprint = footprint.latest();
            if let Some(line) = footprint.and_then(|bytes| footprint_watch.observe(bytes, &ledger))
            {
                eprintln!("{line}");
            }
            Some(MemoryReading {
                ledger: Some(ledger),
                footprint,
            })
        })
    }

    /// The status area's meter: the whole figure in use, and the brick cache
    /// against the budget that bounds it.
    fn memory_figures(&mut self, now: Instant) -> clayspace_view::shell::MemoryFigures {
        let ledger = self.memory_reading(now).ledger.unwrap_or_default();
        clayspace_view::shell::MemoryFigures {
            in_use: ledger.in_use(),
            cache: ledger.cache_bytes,
            budget: ledger.cache_budget,
        }
    }

    fn redraw(&mut self) {
        let frame_started = Instant::now();
        let Some(window) = self.window.clone() else {
            return;
        };
        if self.graphics.is_none() {
            return;
        }
        self.pump_refill();
        self.flush_pending_settle();
        self.settle_quality(frame_started);
        // A retopology that has finished is placed here, before the interface
        // is built, so the frame that shows the new subtool is the frame that
        // learns about it. Never blocks: a job still running reports nothing.
        self.poll_jobs();

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
        let memory = self.memory_figures(frame_started);
        let document_name = self.document_vm.name().get().clone();
        let last = self.sculpt.last_action().get().clone();
        let history = *self.sculpt.history().get();

        // Only when the window that shows it is open. The report is rebuilt
        // every frame and the stroke section is the one part of it that costs
        // something to assemble.
        let diagnostics = self.diagnostics(if self.show_diagnostics {
            StrokeSection::Summarised
        } else {
            StrokeSection::Skipped
        });
        // Assembled for the format the last export used, so the panel says
        // something before a file has been chosen. Re-checked against the
        // actual extension when the write happens.
        //
        // Then whatever the last write turned out to be. The two halves are
        // kept apart on purpose: everything `for_export` says is derived from
        // the format and the settings and is therefore knowable in advance,
        // and everything after it is derived from the bytes and can only be
        // said afterwards. Concatenated rather than merged so the predictions
        // keep their order and the findings read as the news they are.
        let mut export_warnings =
            ExportWarning::for_export(Format::Obj, self.export, self.document.has_mesh_layers());
        export_warnings.extend(self.export_findings.iter().cloned());
        // Read once and borrowed into the state: the shell wants the loaded
        // stamp's name and the document is what holds it.
        let alpha_name = self.document.with(|document| document.alpha_name());
        let mut queue = CommandQueue::new();
        let mut viewport = None;
        let mut input = ViewportInput::default();
        let state = ShellState {
            door: self.agent.door().clone(),
            agent_ask: self.agent.ask(),
            agent_acted: self.agent.seconds_since_agent_acted(),
            agent_access: self.agent.showing_access(),
            strings: self.strings,
            shortcuts: &self.shortcuts,
            mask: *self.mask.state().get(),
            mask_gesture: *self.mask.gesture().get(),
            outline: self.mask.draft().get().as_ref(),
            cut_gesture: *self.cut.gesture().get(),
            cut: self.cut.draft().get().as_ref(),
            smooth_mode: *self.sculpt.smooth_mode().get(),
            offers_smooth_mode: self.sculpt.offers_smooth_mode(),
            armature: self.armature_state(),
            recent: self.recent.paths(),
            show_repair: self.show_repair,
            show_deform: self.show_deform,
            show_shapes: *self.objects.picking().get(),
            insert_as: *self.objects.insert_as().get(),
            copyable_subtools: self.objects.copyable().get(),
            mesh_operands: self.objects.mesh_operands().get(),
            mesh_operand: *self.objects.mesh_operand().get(),
            mesh_operand_cost: *self.objects.mesh_cost().get(),
            show_boolean: *self.boolean.open().get(),
            boolean: *self.boolean.settings().get(),
            boolean_operands: self.boolean.operands().get(),
            boolean_cost: *self.boolean.cost().get(),
            boolean_notice: self.boolean.notice().get().as_deref(),
            shape: *self.objects.shape().get(),
            shape_parameters: self.objects.parameters().get(),
            object_combine: *self.objects.combine().get(),
            objects: self.objects.objects().get(),
            selected_object: *self.objects.selected().get(),
            gizmo_mode: *self.objects.mode().get(),
            gizmo_target: *self.objects.target().get(),
            show_references: self.show_references,
            surface_opacity: self.surface_opacity,
            references: RefPlane::ALL.map(|plane| shell::ReferenceSlot {
                name: self
                    .references
                    .image(plane)
                    .map(|image| image.name.as_str()),
                settings: self.references.settings_for(plane),
            }),
            deform: self.deform,
            sculpt_cost: self.document.with(|document| document.sculpt_layer_cost()),
            // Asked of the document each frame the panel is open, so a repair
            // shows its own result rather than the state before it.
            repair: self
                .show_repair
                .then(|| self.document.with(|d| d.repair_report()))
                .flatten(),
            show_convert: self.show_convert,
            conversion: self.conversion,
            remesh: self.remesh,
            remesh_outcome: self.remesh_outcome,
            retopo: *self.retopo.settings().get(),
            retopo_outcome: *self.retopo.last().get(),
            retopo_unavailable: self.retopo.unavailable().get().clone(),
            retopo_progress: self
                .retopo
                .jobs()
                .progress()
                .get()
                .as_ref()
                .map(|progress| (progress.label.clone(), progress.fraction)),
            uv: *self.uv.settings().get(),
            uv_outcome: *self.uv.last().get(),
            uv_unavailable: self.uv.unavailable().get().clone(),
            uv_progress: self
                .uv
                .jobs()
                .progress()
                .get()
                .as_ref()
                .map(|progress| (progress.label.clone(), progress.fraction)),
            conform: *self.conform.settings().get(),
            conform_outcome: self.conform.last().get().clone(),
            conform_unavailable: self.conform.unavailable().get().clone(),
            conform_progress: self
                .conform
                .jobs()
                .progress()
                .get()
                .as_ref()
                .map(|progress| (progress.label.clone(), progress.fraction)),
            bake: self.bake.settings().get().clone(),
            bake_result: self.bake.last().get().clone(),
            bake_unavailable: self.bake.unavailable().get().clone(),
            bake_progress: self
                .bake
                .jobs()
                .progress()
                .get()
                .as_ref()
                .map(|progress| (progress.label.clone(), progress.fraction)),
            bake_into: self
                .bake_into
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            // Asked of the document, which is the only layer that can see the
            // bounds a region is measured against.
            conversion_cost: self
                .document
                .with(|d| d.conversion_cost(self.conversion.direction, self.conversion.cell_size)),
            show_import: self.show_import,
            show_export: self.show_export,
            import: self.import,
            export: self.export,
            export_warnings: &export_warnings,
            diagnostics: &diagnostics,
            show_diagnostics: self.show_diagnostics,
            diagnostics_copied: self.diagnostics_copied,
            attribution: ATTRIBUTION,
            show_attribution: self.show_attribution,
            extrude: *self.mask.extrude_settings().get(),
            mask_steps: *self.mask.steps().get(),
            voxel_display: self.document.with(|d| d.voxel_display()),
            voxel_blur: self.document.with(|d| d.voxel_blur()),
            curve: self.curve.state().get().clone(),
            curve_radius: *self.curve.radius().get(),
            lattice: self.lattice.state().get().clone(),
            lattice_divisions: *self.lattice.divisions().get(),
            // The engine's own preflight, asked per frame because it costs
            // microseconds and moves with every level added or removed.
            subdivision_cost: self.scene.subdivision_cost(),
            // The stack's own figures, and whether the pointer is still down.
            // The engine refuses a composition change while a gesture is open,
            // so the controls read that rather than discovering it.
            multires_cost: self
                .scene
                .scene()
                .get()
                .active_layer()
                .and_then(|layer| layer.multires.as_ref())
                .map(|hierarchy| hierarchy.cost(self.sculpt.is_stroking())),
            document_name: document_name.as_str(),
            modified: *self.document_vm.modified().get(),
            tool: *self.sculpt.tool().get(),
            representation: self.sculpt.active_representation(),
            brush: *self.sculpt.brush().get(),
            combine: *self.sculpt.combine().get(),
            colour: self.sculpt.colour().get(),
            alpha: alpha_name.as_deref(),
            // The options bar's one "why that did not happen" line, shared.
            //
            // A mask refusal used to be written into an Observable nobody
            // read, so extruding on a layer that has no field to extrude from
            // did nothing at all and said nothing at all. The mask's notice
            // comes first because it is raised by an explicit action, where a
            // tool status is a standing condition.
            // A refused reference joins them, and ahead of both: it is the
            // most recent explicit action, and a PNG that will not load is a
            // sentence naming what is wrong with *that* file.
            // An object refusal was the same Observable nobody read: a
            // re-shape, a re-combine, a removal and a refused transform each
            // wrote one and none of them reached the screen.
            // The operations the composition root runs itself were worse than
            // an Observable nobody read: they had none at all, and printed to
            // stderr instead.
            tool_status: tool_status(ToolStatusSources {
                document: self.document_vm.notice().get().as_deref(),
                operation: self.operation_refusal.get().as_deref(),
                reference: self.references.notice().get().as_deref(),
                mask: self.mask.notice().get().as_deref(),
                object: self.objects.notice().get().as_deref(),
                lattice: self.lattice.notice().get().as_deref(),
                curve: self.curve.notice().get().as_deref(),
                armature: self.armature.notice().get().as_deref(),
                scene: self.scene.refusal().get().as_deref(),
                sculpt: self.sculpt.tool_status().get().as_deref(),
            }),
            symmetry: self.active_symmetry(),
            scene: &scene,
            renaming: self
                .renaming
                .as_ref()
                .map(|(key, draft)| (*key, draft.as_str())),
            stats: *self.sculpt.stats().get(),
            view_preset: *self.sculpt.view_preset().get(),
            polyframe: *self.sculpt.polyframe().get(),
            // The renderer's own display state, read back rather than
            // mirrored: the material beside it is handled the same way, and a
            // second copy of a setting is a second thing to keep in step.
            viewport_profile: self.quality.profile(),
            collapsed: Panel::ALL.map(|panel| self.layout.is_collapsed(panel)),
            focus: self.focus,
            favourites: &self.favourites,
            // The same question the event loop asks to decide how long to
            // wait, asked of the same policy and the same clock, so the line a
            // sculptor reads and the write that happens cannot disagree.
            autosave_in: self
                .autosave
                .next_in(self.saved_at.elapsed(), *self.document_vm.modified().get()),
            studio_shading: self
                .graphics
                .as_ref()
                .is_some_and(|g| g.renderer.shading() == ShadingMode::Studio),
            cavity: self
                .graphics
                .as_ref()
                .is_some_and(|g| g.renderer.cavity() > 0.0),
            shadows: self.graphics.as_ref().is_some_and(|g| g.renderer.shadows()),
            material,
            matcap: self
                .graphics
                .as_ref()
                .map(|g| g.renderer.matcap())
                .unwrap_or_default(),
            materials: &materials,
            can_undo: history.can_undo,
            can_redo: history.can_redo,
            memory,
            backend: &backend,
            units: self.units,
            // The tool's name comes from the interface's own table where a
            // tool made the action; the label is the fallback for the ones no
            // tool made, and for the log, which has no language.
            last_action: (!last.label.is_empty()).then(|| {
                let named = last
                    .tool
                    .map(|tool| self.strings.tool(tool))
                    .unwrap_or(last.label.as_str());
                (named, last.changed)
            }),
        };

        // Read before the frame is built, because the closure below cannot
        // borrow `self` while the ShellState made from it is alive.
        let marquee = self
            .marquee
            .filter(|(from, to, _)| clayspace_app::input::is_a_marquee(*from, *to));
        // The layout the frame draws with, and where a drag on a splitter is
        // collected. Copied out rather than borrowed: `state` borrows `self`
        // for the frame, so the closure cannot reach `self.layout`.
        let layout = self.layout.clone();
        let focus = self.focus;
        // A region is drawn when the sculptor has not put it away *and* the
        // chrome has not been cleared. Focus asks the question a second time
        // rather than editing the layout, so what it hides it also gives back.
        let shown = |panel: Panel| !focus && !layout.is_collapsed(panel);
        let mut resized: [Option<f32>; 3] = [None; 3];
        let output = context.run(raw_input, |ctx| {
            egui::TopBottomPanel::top("menu")
                .exact_height(region::MENU_BAR)
                .show(ctx, |ui| shell::menu_bar(ui, &state, &mut queue));
            if !focus {
                egui::TopBottomPanel::top("options")
                    .exact_height(region::OPTIONS_BAR)
                    .show(ctx, |ui| shell::options_bar(ui, &state, &mut queue));
            }
            if !focus {
                egui::TopBottomPanel::bottom("status")
                    .exact_height(region::STATUS)
                    .show(ctx, |ui| shell::status_bar(ui, &state, &mut queue));
            }
            if shown(Panel::Shelf) {
                let height = egui::TopBottomPanel::bottom("shelf")
                    .resizable(true)
                    .default_height(layout.size(Panel::Shelf))
                    .height_range(Panel::Shelf.minimum()..=Panel::Shelf.maximum())
                    .show(ctx, |ui| {
                        egui::ScrollArea::horizontal()
                            .show(ui, |ui| shell::brush_shelf(ui, &state, &mut queue));
                    })
                    .response
                    .rect
                    .height();
                resized[2] = Some(height);
            }
            if !focus {
                egui::SidePanel::left("rail")
                    .exact_width(region::RAIL)
                    .resizable(false)
                    .show(ctx, |ui| shell::tool_rail(ui, &state, &mut queue));
            }
            // Resizable, and remembered. `layout` carries the width, the
            // bounds a drag is clamped to and whether the region is put away;
            // it was written with all of that and a pair of serialisers and
            // then drawn at a fixed width for the life of the application.
            //
            // A collapsed region is given no space at all rather than a narrow
            // one — `Layout::size` reports zero for it — and keeps the width
            // it had, so expanding returns the size a sculptor chose rather
            // than a default.
            if shown(Panel::Left) {
                let width = egui::SidePanel::left("left")
                    .resizable(true)
                    .default_width(layout.size(Panel::Left))
                    .width_range(Panel::Left.minimum()..=Panel::Left.maximum())
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .show(ui, |ui| shell::left_panel(ui, &state, &mut queue));
                    })
                    .response
                    .rect
                    .width();
                resized[0] = Some(width);
            }
            if shown(Panel::Right) {
                let width = egui::SidePanel::right("right")
                    .resizable(true)
                    .default_width(layout.size(Panel::Right))
                    .width_range(Panel::Right.minimum()..=Panel::Right.maximum())
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .show(ui, |ui| shell::right_panel(ui, &state, &mut queue));
                    })
                    .response
                    .rect
                    .width();
                resized[1] = Some(width);
            }
            shell::diagnostics_window(ctx, &state, &mut queue);
            shell::attribution_window(ctx, &state, &mut queue);
            shell::agent_access_window(ctx, &state, &mut queue);
            shell::agent_ask_window(ctx, &state, &mut queue);
            shell::convert_window(ctx, &state, &mut queue);
            shell::repair_window(ctx, &state, &mut queue);
            shell::deform_window(ctx, &state, &mut queue);
            shell::reference_window(ctx, &state, &mut queue);
            shell::import_window(ctx, &state, &mut queue);
            shell::export_window(ctx, &state, &mut queue);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    // Inside the central region rather than a panel of its
                    // own, so it spans the viewport it labels and stops at
                    // the inspectors either side.
                    // The menu bar stays: it is how a sculptor finds their way
                    // back out, and Tab is not discoverable from an empty
                    // window. Everything else goes.
                    if !focus {
                        shell::representation_bar(ui, &state, &mut queue);
                    }
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
                        // The rubber band, over the scene egui draws nothing
                        // into: the viewport's pixels come from wgpu, and this
                        // is the one thing about the gesture the interface
                        // layer draws itself.
                        if let Some((from, to, _)) = marquee {
                            shell::selection_box(
                                ui.painter(),
                                egui::Rect::from_two_pos(from, to).intersect(rect),
                            );
                        }
                        // And the mask outline, for the same reason and in the
                        // same place: a gesture's own feedback belongs over the
                        // scene and under the panels, because it is a picture
                        // of what the pointer has done so far rather than a
                        // control. The two never show at once — a cage owns the
                        // viewport while it is up, so the press that would draw
                        // one is the press that draws the other.
                        shell::outline_overlay(ui, rect, &state);
                        shell::cut_overlay(ui, rect, &state);
                        // And the transform readout, in the corner, while a
                        // manipulator is pointed at a placed object. Over the
                        // scene for the same reason as the two above: it
                        // belongs to what the pointer is doing rather than to
                        // a panel.
                        shell::transform_hud(ui, rect, &state);
                        // The brush and its numbers, while the bar that
                        // carries them is away.
                        if focus {
                            shell::brush_hud(ui, rect, &state);
                        }
                    });
                });
        });

        // A splitter that was dragged, stored and written out. Compared before
        // writing: egui reports the region's width every frame, and a file
        // rewritten on every frame of a session is a file written thousands of
        // times for the handful of drags a sculptor actually made.
        // A region put away or brought back, and a request to have them all
        // back at the design's own sizes. Read after the frame and taken out of
        // memory, so one click is one toggle rather than one per frame for as
        // long as the value sits there.
        let mut moved = false;
        // Read then removed, rather than `remove_temp`, which egui offers only
        // for a type with a default — and a `Panel` has no sensible one: there
        // is no "no region".
        let toggled = context.data(|data| data.get_temp::<Panel>(shell::panel_toggle_id()));
        if toggled.is_some() {
            context.data_mut(|data| data.remove::<Panel>(shell::panel_toggle_id()));
        }
        if let Some(panel) = toggled {
            self.layout.toggle(panel);
            moved = true;
        }
        // The menu's route into focus mode, beside the keyboard's.
        if context
            .data_mut(|data| data.remove_temp::<bool>(shell::focus_toggle_id()))
            .unwrap_or(false)
        {
            self.focus = !self.focus;
        }
        let reset = context
            .data_mut(|data| data.remove_temp::<bool>(shell::layout_reset_id()))
            .unwrap_or(false);
        if reset {
            self.layout.reset();
            moved = true;
        }
        for (index, panel) in Panel::ALL.into_iter().enumerate() {
            if let Some(size) = resized[index] {
                if (self.layout.stored_size(panel) - size).abs() > 0.5 {
                    self.layout.resize(panel, size);
                    moved = true;
                }
            }
        }
        if moved {
            if let Some(store) = self.store.as_ref() {
                store.save_layout(&self.layout);
            }
        }
        // A reset has to reach egui as well as the stored line. `default_width`
        // is a default: egui remembers each panel's width in its own memory
        // and keeps it, so Restaurar disposição moved the stored sizes and left
        // every panel exactly where it was. Dropping the remembered state is
        // what makes the default apply again on the next frame.
        if reset {
            for id in ["left", "right", "shelf"] {
                context.data_mut(|data| {
                    data.remove::<egui::containers::panel::PanelState>(egui::Id::new(id))
                });
            }
        }

        // What the View left in its own memory, applied to the governor that
        // owns it. The profile touches no document, so it never became a
        // command — and could not have, since `ViewportProfile` is a view type
        // and commands live under the view. Read after the frame, so the menu
        // that wrote it has finished.
        if let Some(chosen) =
            context.data(|data| data.get_temp::<ViewportProfile>(shell::viewport_profile_id()))
        {
            if chosen != self.quality.profile() {
                self.quality.set_profile(chosen, Instant::now());
                // Kept, so the next session opens where this one was left.
                if let Some(store) = self.store.as_ref() {
                    store.save_viewport_profile(chosen);
                }
            }
        }

        let scale = context.pixels_per_point();
        self.viewport = viewport;

        // `state` borrowed the scene and the strings for the frame. It is not
        // `Drop`, so calling `drop` on it did nothing but say so; letting the
        // binding end is what actually returns the borrows.
        let _ = &state;

        // A brush starred or unstarred, from its own menu.
        let starred = context
            .data(|data| data.get_temp::<clayspace_model::ToolKind>(shell::favourite_toggle_id()));
        if let Some(tool) = starred {
            context.data_mut(|data| {
                data.remove::<clayspace_model::ToolKind>(shell::favourite_toggle_id())
            });
            match self.favourites.iter().position(|starred| *starred == tool) {
                Some(at) => {
                    self.favourites.remove(at);
                }
                None => self.favourites.push(tool),
            }
            if let Some(store) = self.store.as_ref() {
                store.save_favourites(&self.favourites);
            }
        }

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
        // Where a placement would land: under the pointer on the surface,
        // and where the camera is looking when the pointer is off it. Set
        // before the frame's commands, so an `InsertShape` in this frame places
        // where the sculptor was looking when they asked — placing at the
        // origin puts a subtracting shape *inside* the form, cutting something
        // nobody can see.
        let placement = self
            .hover
            .map(|(position, _)| position)
            .unwrap_or_else(|| self.camera.target.into());
        self.objects.set_placement_point(Some(placement));
        let cursors = self.cursors();
        let scene_viewport = self.viewport.map(|rect| {
            [
                rect.min.x * scale,
                rect.min.y * scale,
                rect.width() * scale,
                rect.height() * scale,
            ]
        });
        self.sync_mesh_layers();
        self.sync_active_subtool();
        self.sync_cage();
        self.sync_mask();
        self.sync_lattice_view();
        self.sync_references();
        self.sync_symmetry_overlay();
        self.sync_armature_view();
        // After the frame's commands, so a camera move made in it is the one
        // decided against, and before the surface is drawn.
        self.update_detail();
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

        // The cursor is a ribbon a couple of pixels wide, so how wide that is
        // in the world depends on where the camera is and how tall the scene's
        // rectangle is. Both are known here and neither is known to the
        // renderer at the moment the geometry is built.
        let metric = clayspace_view::ScreenMetric::new(
            &self.camera,
            scene_viewport.map_or(graphics.surface.framebuffer().height as f32, |rect| rect[3]),
        );
        graphics
            .renderer
            .set_cursors(&graphics.gpu, &cursors, metric);
        graphics.renderer.set_scene_viewport(scene_viewport);
        // With the skin preview off, the surface is simply not drawn — the
        // scaffolding stands alone, which is what ZBrush shows while a rig is
        // being built. Presentation only: nothing in the document changes, so
        // toggling it back costs no re-mesh.
        let surface = if self.rigging && !self.skin_preview {
            &graphics.nothing
        } else {
            graphics.geometry.mesh()
        };
        // Presentation, so it is pushed rather than stored: the ViewModel owns
        // whether the polyframe is on and the renderer owns whether it draws,
        // and reading it here each frame is what keeps them from drifting.
        let polyframe = *self.sculpt.polyframe().get();
        let gpu = graphics.gpu.clone();
        graphics.renderer.set_polyframe(&gpu, polyframe);
        graphics.renderer.render(
            &graphics.gpu,
            &view,
            graphics.surface.framebuffer(),
            &self.camera,
            surface,
            // Vertex colour on, which it was not. A voxel layer's palette and a
            // mesh layer's colour attribute have reached the vertex buffer
            // through `sync_mesh_layers` all along, and the fragment stage has
            // always been able to modulate by them — the composition root was
            // writing the switch off on every frame, so Pintar could not change
            // a pixel on either representation.
            //
            // Unconditional rather than "when something carries colour",
            // because the modulation's identity is white and everything without
            // a colour is white: the field surface is meshed with colours off
            // and `read_mesh` fills its vertices with 1.0, and a mesh layer
            // with no colour attribute comes back the same. So a scene with no
            // colour in it renders as it did, to within the rasteriser's own
            // precision — `colour_modulation_leaves_a_field_surface_alone`
            // holds that, and says why it is not bit for bit.
            true,
        );
        // Kept rather than dropped: a whole-window capture replays these over
        // an offscreen target, which is what makes the picture an agent sees
        // the picture the sculptor is looking at.
        self.last_ui = paint_interface(graphics, &context, output, &view, scale);
        self.last_ppp = scale;
        frame.present();
    }

    /// Applies a command, plus the view-side effects no ViewModel owns.
    fn handle(&mut self, command: Command) {
        // Taken before the match, which moves the command: the arms that bank
        // an edit of their own need a name to bank it under, and the history
        // is what reads it back.
        let named = command.label();
        match command {
            Command::ToggleAgentDoor => {
                if self.agent.is_listening() {
                    // Remembered: a door a person shut stays shut when the
                    // application is opened again.
                    self.shut_the_door(true);
                    self.agent.show_access(false);
                } else if let Some(root) =
                    self.store.as_ref().map(|store| store.root().to_path_buf())
                {
                    let _ = clayspace_mcp::access::remember_door(&root, true);
                    match &self.agent_proxy {
                        Some(proxy) => {
                            self.agent_queue = JobQueue::new();
                            let proxy = proxy.clone();
                            self.open_the_door(proxy);
                        }
                        None => eprintln!("a porta do agente não pode ser reaberta nesta sessão"),
                    }
                }
                self.request_redraw();
            }
            Command::ShowAgentAccess(showing) => {
                self.agent.show_access(showing);
                self.request_redraw();
            }
            Command::AnswerAgentAsk(answer) => {
                self.agent.answer(answer);
                self.request_redraw();
            }
            Command::FrameAll => {
                self.frame_all();
                self.request_redraw();
            }
            Command::ToggleShading => {
                if let Some(graphics) = self.graphics.as_mut() {
                    let next = match graphics.renderer.shading() {
                        ShadingMode::MatCap => ShadingMode::Studio,
                        ShadingMode::Studio => ShadingMode::MatCap,
                    };
                    graphics.renderer.set_shading(next);
                    self.request_redraw();
                }
            }
            Command::ToggleCavity => {
                if let Some(graphics) = self.graphics.as_mut() {
                    let on = graphics.renderer.cavity() > 0.0;
                    graphics
                        .renderer
                        .set_cavity(if on { 0.0 } else { Renderer::CAVITY });
                    self.request_redraw();
                }
            }
            Command::ToggleShadows => {
                if let Some(graphics) = self.graphics.as_mut() {
                    let on = graphics.renderer.shadows();
                    graphics.renderer.set_shadows(!on);
                    self.request_redraw();
                }
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
            Command::ToggleImport => {
                self.show_import = !self.show_import;
                self.request_redraw();
            }
            Command::ToggleExport => {
                self.show_export = !self.show_export;
                self.request_redraw();
            }
            Command::SetImportSettings(settings) => {
                self.import = settings;
                self.request_redraw();
            }
            Command::SetExportSettings(settings) => {
                self.export = settings;
                self.request_redraw();
            }
            Command::RunImport => self.import_mesh(),
            Command::InsertMesh => self.insert_mesh_subtool(),
            Command::LoadAlpha => self.load_alpha(),
            Command::ClearAlpha => {
                self.document.with(|document| document.set_alpha(None));
                self.request_redraw();
            }
            Command::RunExport => self.export_mesh(),
            Command::SetVoxelDisplay(display, blur) => {
                // Display only. Nothing in the document is touched, so this
                // neither marks it modified nor enters the history — the same
                // bargain the display unit makes.
                if let Err(e) = self
                    .document
                    .with(|document| document.set_voxel_display(display, blur))
                {
                    eprintln!("a exibição de voxels não pôde ser alterada: {e}");
                }
                self.sync_mesh_layers();
                self.request_redraw();
            }
            Command::SetLocale(locale) => {
                // Presentation only, like the display unit: nothing in the
                // document is touched, so this neither marks it modified nor
                // enters the history. Written down straight away, because a
                // language chosen and lost on quit is worse than none.
                self.strings = Strings::for_locale(locale);
                if let Some(store) = &self.store {
                    store.save_locale(locale);
                }
                self.request_redraw();
            }
            Command::NextDisplayUnit => {
                // Presentation only. Nothing in the document is touched, so
                // this neither marks it modified nor enters the history.
                self.units.display = self.units.next_display();
                self.request_redraw();
            }
            Command::ToggleAttribution => {
                self.show_attribution = !self.show_attribution;
                self.request_redraw();
            }
            Command::ToggleDiagnostics => {
                self.show_diagnostics = !self.show_diagnostics;
                // The confirmation belongs to one visit. Leaving it set means
                // the window reopens claiming a copy that never happened.
                self.diagnostics_copied = false;
                self.request_redraw();
            }
            Command::CopyDiagnostics => {}
            Command::ExportProfile => self.export_profile(),
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
                let before = self.engine_undo_depth();
                self.armature.begin(at);
                // A rig gets a layer of its own and `begin_armature` turns
                // that layer's mirror off — the tree carries both halves
                // itself, so a layer mirror would reflect the placed node as
                // well. No `SelectLayer` announces the new layer, so without
                // this the options bar and the next stroke kept the mirror of
                // the subtool the sculptor was on and the first ZSphere hung a
                // second arm off the first.
                self.sculpt.refresh_for_active_layer();
                self.rigging = self.armature.is_rigging();
                self.after_armature_edit(named, before);
            }
            Command::ToggleArmatureEditing => {
                self.rigging = !self.rigging && self.armature.is_rigging();
                self.request_redraw();
            }
            Command::RemoveZsphere => {
                let before = self.engine_undo_depth();
                self.armature.remove_selected();
                self.after_armature_edit(named, before);
            }
            Command::ToggleSkinPreview => {
                self.skin_preview = !self.skin_preview;
                self.request_redraw();
            }
            Command::ToggleZsphereNegative => {
                let before = self.engine_undo_depth();
                let negative = !self.armature.selected_is_negative();
                self.armature.set_selected_negative(negative);
                self.after_armature_edit(named, before);
            }
            // Choosing changes nothing in the document, so it takes none of
            // the undo bookkeeping the rest of these do.
            Command::SelectZsphere(index) => {
                self.armature.select(index);
                self.request_redraw();
            }
            Command::AddZsphere { parent, at, radius } => {
                let before = self.engine_undo_depth();
                self.armature.add(parent, at, radius);
                self.after_armature_edit(named, before);
            }
            Command::InsertZsphere(child) => {
                let before = self.engine_undo_depth();
                self.armature.insert(child);
                self.after_armature_edit(named, before);
            }
            Command::MoveZsphere { index, to } => {
                let before = self.engine_undo_depth();
                self.armature.move_to(index, to);
                self.after_armature_edit(named, before);
            }
            Command::ResizeZsphere { index, radius } => {
                let before = self.engine_undo_depth();
                self.armature.resize(index, radius);
                self.after_armature_edit(named, before);
            }
            Command::ReparentZsphere { index, parent } => {
                let before = self.engine_undo_depth();
                self.armature.reparent(index, parent);
                self.after_armature_edit(named, before);
            }
            Command::SetSkinThickness(thickness) => {
                let before = self.engine_undo_depth();
                self.armature.set_skin(SkinSettings { thickness });
                self.after_armature_edit(named, before);
            }
            // The cage settles before the switch rather than after it: the
            // sculptor may say to stay, and a switch already made cannot be
            // taken back by answering the question that follows it.
            Command::SelectLayer(key) => {
                if self.resolve_a_standing_cage(key) {
                    self.apply(Command::SelectLayer(key));
                }
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
) -> Vec<egui::ClippedPrimitive> {
    let primitives = context.tessellate(output.shapes, pixels_per_point);
    for (id, delta) in &output.textures_delta.set {
        graphics.egui_renderer.update_texture(
            &graphics.gpu.device,
            &graphics.gpu.queue,
            *id,
            delta,
        );
    }

    let size = [
        graphics.surface.framebuffer().width,
        graphics.surface.framebuffer().height,
    ];
    paint_primitives(graphics, &primitives, size, pixels_per_point, target);

    for id in &output.textures_delta.free {
        graphics.egui_renderer.free_texture(id);
    }
    primitives
}

/// The interface's pass over a target, from primitives already tessellated.
///
/// Apart from `paint_interface` so that a whole-window capture can run the
/// *same* primitives over an offscreen target. Building a second interface for
/// the capture would be building an interface nobody drew, and the two would
/// come to disagree.
fn paint_primitives(
    graphics: &mut Graphics,
    primitives: &[egui::ClippedPrimitive],
    size_in_pixels: [u32; 2],
    pixels_per_point: f32,
    target: &wgpu::TextureView,
) {
    let descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels,
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
        primitives,
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
            .render(&mut pass.forget_lifetime(), primitives, &descriptor);
    }
    graphics.gpu.queue.submit(Some(encoder.finish()));
}

impl ApplicationHandler<AgentWake> for App {
    /// Sleeps until there is something to do, or until an autosave is due.
    ///
    /// `Wait` alone would be right for an application that only acts on input,
    /// and wrong for this one: the case autosave exists for is a sculptor who
    /// edits and then walks away, which produces no further events at all.
    /// A request arrived while the loop was asleep.
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: AgentWake) {
        self.serve_agent();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Here as well as in `user_event`, because the push and the wake are
        // two steps: a job that landed just after the loop decided to sleep
        // would otherwise wait for the next thing a person did.
        self.serve_agent();
        if self.quit_requested {
            self.quit_requested = false;
            if self.document_vm.guard() == Guard::Clear || self.confirm_discarding_work() {
                // Not remembered as shut: this is the application closing, not
                // a person closing the door, and the two must not be confused
                // or every quit would leave it shut for the next session.
                self.shut_the_door(false);
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
            WindowEvent::CloseRequested
                if self.document_vm.guard() == Guard::Clear || self.confirm_discarding_work() =>
            {
                // The last chance to keep the work. Closing over unsaved edits
                // without asking is the one mistake this application can make
                // that a user cannot undo.
                // Clearing the marker is what makes the *next* run silent.
                // Left behind, an ordinary quit looks like a crash.
                self.end_session();
                event_loop.exit();
            }
            WindowEvent::CloseRequested => {}

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

                // The bindings are the table's, not this handler's. The
                // platform difference lives inside `modifiers.command` — ⌘ on
                // macOS, Ctrl elsewhere — so there is one table for all three
                // and no `cfg!` on this path. See [`clayspace_app::keys`] for
                // what a handler of its own cost last time.
                let modifiers = self
                    .graphics
                    .as_ref()
                    .map(|g| g.egui_state.egui_ctx().input(|i| i.modifiers))
                    .unwrap_or_default();
                let Some(action) =
                    chord_for(code, modifiers).and_then(|chord| self.shortcuts.action(chord))
                else {
                    return;
                };
                self.perform(action, event_loop);
            }

            _ => {}
        }
    }
}

/// Every source that can explain "why that did not happen", in the order the
/// options bar prefers them.
///
/// Named rather than positional because the order *is* the behaviour: most of
/// these were Observables that nothing read, and each was found the same way —
/// an action refused, a sentence written, and no sentence on screen. A tuple
/// of `Option<&str>` would let a reorder pass review unnoticed.
struct ToolStatusSources<'a> {
    /// A save or an open that failed. First because it is the most recent
    /// explicit action there is, and the one whose silence costs work rather
    /// than a click: a save refused because a hierarchy's side-car could not
    /// be written went to stderr, and the sculptor was left looking at a
    /// document that had failed to save and did not say so.
    document: Option<&'a str>,
    /// An operation the composition root ran itself and had refused: a repair,
    /// a crossing, a pass of the active layer's stack, a rebuild. Second
    /// because it is the answer to the thing that was just asked for; it went
    /// to stderr, so a refused repair was a button that did nothing and said
    /// nothing.
    operation: Option<&'a str>,
    /// A PNG that will not load, which is a sentence naming what is wrong with
    /// *that* file.
    reference: Option<&'a str>,
    /// Extruding on a layer that has no field to extrude from.
    mask: Option<&'a str>,
    /// A re-shape, a re-combine, a removal or a refused transform.
    object: Option<&'a str>,
    /// A cage refused, dragged past what the layer will take, or applied onto
    /// a form that cannot be warped. The cage had no line of its own at all,
    /// so a refused apply threw away every drag the sculptor had made and the
    /// screen said nothing had happened.
    lattice: Option<&'a str>,
    /// A curve edit the model would not make.
    curve: Option<&'a str>,
    /// A ZSphere verb refused. It went to stderr from the composition root,
    /// which is the same as nowhere.
    armature: Option<&'a str>,
    /// A rebuild refused for an unusable resolution, and now a level refused
    /// for its peak. `run_remesh` has claimed this reaches the screen since it
    /// was written and it did not.
    scene: Option<&'a str>,
    /// A standing condition rather than an explicit action, so it comes last.
    sculpt: Option<&'a str>,
}

/// The options bar's one "why that did not happen" line.
///
/// A refusal raised by an explicit action beats a standing condition, and the
/// document's own beats every other explicit one.
fn tool_status<'a>(from: ToolStatusSources<'a>) -> Option<&'a str> {
    from.document
        .or(from.operation)
        .or(from.reference)
        .or(from.mask)
        .or(from.object)
        .or(from.lattice)
        .or(from.curve)
        .or(from.armature)
        .or(from.scene)
        .or(from.sculpt)
}

/// How many channels carry a refusal — a reason the command did not happen.
///
/// One per ViewModel that can refuse, plus the composition root's own.
/// [`App::refusal_channels`] is the list this counts, and the two have to
/// agree or that array does not build — which is the only thing standing
/// between a new panel and a refusal nobody reads.
const NOTICE_REFUSAL_CHANNELS: usize = 15;

/// How many channels carry a remark — something that did happen, said beside
/// the answer rather than in place of it.
const NOTICE_REMARK_CHANNELS: usize = 3;

/// How many channels a refusal or a remark can arrive on.
const NOTICE_CHANNELS: usize = NOTICE_REFUSAL_CHANNELS + NOTICE_REMARK_CHANNELS;

/// What the interface would have shown, out of the channels compared either
/// side of a command.
///
/// Free of `App` so the rule it encodes can be held by a test, because the
/// rule is not obvious from either end: the door decides a command was refused
/// by reading *these* channels and nothing else, so a refusal written anywhere
/// else — `eprintln!`, most of all — is one it answers success to. That is how
/// a repair on an SDF layer, a crossing priced past its budget and a second
/// `begin_recording` came to be reported to an agent as work that happened.
///
/// The first channel written wins, so `refusals` is in the order the answer
/// belongs to the command: an operation the composition root ran itself is the
/// most direct answer there is, and a panel's standing notice the least. A
/// remark does not compete that way — every one written is carried, because
/// two of them are two separate things that happened.
fn notices_written(
    refusals: [(bool, Option<&str>); NOTICE_REFUSAL_CHANNELS],
    remarks: [(bool, Option<&str>); NOTICE_REMARK_CHANNELS],
) -> (Option<String>, Vec<String>) {
    let refusal = refusals
        .into_iter()
        .find_map(|(written, said)| written.then_some(said).flatten())
        .map(str::to_string);
    // A remark is carried beside the answer rather than in place of it,
    // because what it reports did happen. A substituted tool did draw, and an
    // agent told "this was refused" would undo something that worked. The
    // mask's remark is the same shape: clearing a mask that freezes nothing
    // leaves the mask exactly as it was asked to be, so a caller told "this
    // was refused" would ask again, and one told nothing at all would go
    // looking for the entry in the history.
    let notices = remarks
        .into_iter()
        .filter_map(|(written, said)| written.then_some(said).flatten())
        .map(str::to_string)
        .collect();
    (refusal, notices)
}

/// A remark as an agent is told it.
///
/// The swap marker is for the shell, which localises it into "tool changed:
/// this layer has no verb for that one" — true, and all an artist looking at
/// the shelf needs, since the shelf shows which tool is in hand. An agent sees
/// no shelf. Handed the bare marker it learned that *something* was swapped
/// and not what for what, so the sentence it gets names both tools by the keys
/// it chooses them with.
fn remark_for_an_agent<'a>(
    said: Option<&'a str>,
    substitution: Option<&'a str>,
) -> Option<&'a str> {
    match said {
        Some(clayspace_vm::TOOL_SUBSTITUTED) => substitution.or(said),
        other => other,
    }
}

fn next_matcap(current: MatCap) -> MatCap {
    let all = MatCap::ALL;
    let index = all.iter().position(|m| *m == current).unwrap_or(0);
    all[(index + 1) % all.len()]
}

/// Whether the gesture the engine is holding belongs to the agent.
///
/// Its own type, and not a bare `bool`, because the bool had one owner and two
/// moments: it was raised on the *intent* to open a gesture and lowered only by
/// a close. A begin the ViewModel refused therefore left it raised for the rest
/// of the session, and [`App::holding_a_gesture`] reads it as "no person is
/// holding anything" — so one refused agent stroke stopped a **person's** real
/// stroke from ever being reported at the door again. Raising on the intent and
/// then settling against what actually opened is what closes that.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct AgentGesture {
    held: bool,
}

impl AgentGesture {
    /// What the agent means to do, before the command is applied — so the next
    /// call from the same agent is not refused its own stroke.
    ///
    /// Which verbs open a gesture and which close one is asked of the command
    /// vocabulary rather than listed here. The door needs the same three sets
    /// to decide what an agent holding a gesture may send, and two copies of
    /// one list is how a verb comes to be in one of them and not the other.
    fn opening(&mut self, command: &Command) {
        if command.opens_a_gesture() {
            self.held = true;
        } else if command.closes_a_gesture() {
            self.held = false;
        }
    }

    /// What actually happened, once the command has been applied.
    ///
    /// `opened` is whether the command was one that opens a gesture, and
    /// `open` whether anything is held now that it has run. A begin that was
    /// refused holds nothing, and an agent holding nothing must not go on
    /// masking a person's gesture.
    ///
    /// Every opening verb and not only a stroke's. A manipulator drag and a
    /// mask outline are opened the same way and can be refused the same way,
    /// and the flag left standing after one of those is the same defect
    /// wearing a different coat.
    fn settled(&mut self, opened: bool, open: bool) {
        if opened {
            self.held = open;
        }
    }

    fn held(self) -> bool {
        self.held
    }
}

/// Why the door refuses a stroke command outright, or `None` where it may go
/// through.
///
/// A sample or a close with no gesture open reaches a ViewModel that has
/// nothing to add it to and answers `Ok(())` — correct for a person, whose
/// pointer release arrives whether or not the press opened anything, and a
/// silent lie to an agent, which is told its stroke landed. An agent's begin
/// having been refused is exactly when this happens, so it is the case that
/// most needs saying.
fn stroke_needs_a_gesture(command: &Command, open: bool) -> Option<&'static str> {
    if open {
        return None;
    }
    match command {
        Command::ContinueStroke { .. } => Some(
            "no stroke is open, so there is nothing to add this sample to. \
             stroke.begin opens one, and it refuses where the tool has no verb \
             on the layer in hand.",
        ),
        Command::EndStroke => Some(
            "no stroke is open, so there is nothing to close. stroke.begin \
             opens one, and it refuses where the tool has no verb on the layer \
             in hand.",
        ),
        _ => None,
    }
}

/// How a ViewModel's refusal reaches an agent.
///
/// The code is the part an agent branches on, so an unavailable tool is not
/// folded in with an engine failure: one is answered by choosing another tool
/// or another layer, and the other is not answerable at all.
fn refusal_for(refused: &ModelError) -> Refusal {
    let code = match refused {
        ModelError::Unavailable(_) => RefusalCode::Unavailable,
        _ => RefusalCode::ModelRefused,
    };
    Refusal::new(code, refused.to_string())
}

/// What the agent-facing door can ask of the running application.
///
/// Every method here runs on the interface thread, between frames, because
/// that is the only thread that may touch a ViewModel or the document. The
/// door's own threads never reach past [`clayspace_mcp::JobQueue`].
/// The four passes [`Session::read`] is assembled from.
///
/// Inherent rather than part of the trait, because they are this application's
/// own decomposition of one trait method and nothing outside calls them. Each
/// is a run of independent `if`s: a section asked for is a section answered,
/// and no section's answer depends on another's.
impl App {
    /// What the document holds: its name, its tree and the forms in it.
    fn read_document(&mut self, query: &StateQuery, state: &mut StateReport) {
        if query.document {
            state.document = Some(report::document_state(
                self.document_vm.name().get(),
                *self.document_vm.modified().get(),
                self.document_vm.path().get().as_deref(),
                self.units.display.label(),
                &self.policy.diagnostics().document_format,
            ));
        }
        if query.scene {
            let scene = self.scene.scene().get().clone();
            let selected = self
                .objects
                .selected()
                .get()
                .map(|id| (id.layer.0, id.node));
            // Counted here because the object ViewModel is the only party that
            // can be asked: a layer summary carries its grid's passes and not
            // the forms placed in it.
            let placed = self.objects.objects().get().clone();
            state.scene = Some(report::scene_state(
                &scene,
                selected,
                |key| self.objects.layer_placement(key),
                |key| placed.iter().filter(|form| form.id.layer == key).count(),
            ));
        }
        if query.objects {
            state.objects = Some(report::object_state(
                self.objects.objects().get(),
                *self.objects.selected().get(),
            ));
        }
        if query.history {
            state.history = Some(report::history_state(
                self.sculpt.history().get(),
                // What the next step would take back, in each direction — not
                // the last thing that happened, which is what this used to
                // send and which after an undo is the undo.
                self.sculpt.next_undo().map(str::to_string),
                self.sculpt.next_redo().map(str::to_string),
                self.agent.from_agent() as usize,
            ));
        }
    }

    /// What the next stroke would be: the tool, the brush and how it combines.
    fn read_shelf(&mut self, query: &StateQuery, state: &mut StateReport) {
        if query.tool {
            state.tool = Some(report::tool_state(
                *self.sculpt.tool().get(),
                self.sculpt.brush().get(),
                *self.sculpt.symmetry().get(),
                self.sculpt.active_representation(),
                *self.sculpt.smooth_mode().get(),
                // The rig's mirror, and only while one is being edited: a
                // switch reported where it decides nothing is a switch an
                // agent will act on.
                self.rigging.then(|| *self.armature.symmetric().get()),
                self.sculpt.substitution(),
            ));
        }
        if query.brush {
            state.brush = Some(report::brush_state(self.sculpt.brush().get()));
        }
        if query.combine {
            state.combine = Some(report::combine_state(
                self.sculpt.combine().get(),
                self.objects.combine().get(),
            ));
        }
        if query.camera {
            let viewport = self
                .graphics
                .as_ref()
                .map(|graphics| {
                    let frame = graphics.surface.framebuffer();
                    [frame.width, frame.height]
                })
                .unwrap_or([0, 0]);
            state.camera = Some(report::camera_state(
                self.camera.eye().into(),
                self.camera.target.into(),
                self.camera.up().into(),
                self.camera.fov_y,
                viewport,
            ));
        }
    }

    /// What the panels hold, and what the last long operation came to.
    fn read_panels(&mut self, query: &StateQuery, state: &mut StateReport) {
        if query.mask {
            state.mask = Some(report::mask_state(
                self.mask.state().get(),
                None,
                *self.mask.steps().get(),
                *self.mask.gesture().get(),
            ));
        }
        if query.cage {
            state.cage = Some(report::cage_state(
                self.lattice.state().get(),
                *self.lattice.divisions().get(),
            ));
        }
        if query.deform {
            state.deform = Some(report::deform_state(&self.deform));
        }
        if query.outcomes {
            state.outcomes = Some(report::outcome_state(
                self.remesh_outcome.as_ref(),
                self.retopo.last().get().as_ref(),
                self.crossing_outcome,
            ));
        }
        if query.presentation {
            state.presentation = Some(report::presentation_state(
                self.focus,
                *self.sculpt.grid().get(),
                *self.sculpt.polyframe().get(),
                *self.sculpt.view_preset().get(),
                self.surface_opacity,
                self.rigging,
                self.skin_preview,
            ));
        }
        if query.references {
            state.references = Some(report::reference_state(|plane| {
                (
                    self.references.settings_for(plane),
                    self.references.path(plane).map(|path| path.to_path_buf()),
                )
            }));
        }
        if query.exchange {
            state.exchange = Some(report::exchange_state(
                &self.import,
                &self.export,
                &self.export_findings,
            ));
        }
        if query.jobs {
            state.jobs = Some(
                self.outstanding_work()
                    .into_iter()
                    .map(|item| clayspace_mcp::session::JobState {
                        label: item.what,
                        fraction: item.fraction,
                    })
                    .collect(),
            );
        }
    }

    /// What the session is costing. The one group that costs something to ask.
    fn read_diagnostics(&mut self, query: &StateQuery, state: &mut StateReport) {
        if !(query.memory || query.timing || query.backends || query.strokes) {
            return;
        }
        // Memory is read through the same meter the status area reads, before
        // the report that carries it is built, so an agent and a person cannot
        // disagree — and so an agent polling this does not put back the
        // per-call walk of the brick cache that the meter took out.
        let footprint = self.memory_reading(Instant::now()).footprint;
        // The stroke section is the one part of this report that costs
        // something to assemble, so it is assembled only where the agent asked
        // for it.
        let diagnostics = self.diagnostics(if query.strokes {
            StrokeSection::Summarised
        } else {
            StrokeSection::Skipped
        });
        if query.memory {
            state.memory = report::memory_state(&diagnostics, footprint);
        }
        if query.timing {
            // The GPU passes the renderer timed, summed. Zero where the
            // adapter does not offer timestamps, which is a stated absence
            // rather than a figure invented here.
            let frame = diagnostics
                .render
                .as_ref()
                .filter(|render| render.gpu_timing)
                .map(|render| render.gpu_passes.iter().map(|(_, ms)| ms).sum())
                .unwrap_or(0.0);
            state.timing = Some(report::timing_state(&self.stalls, frame));
        }
        if query.backends {
            state.backends = Some(report::backend_state(&diagnostics));
        }
        if query.strokes {
            // Where the last strokes spent their milliseconds, split across
            // the engine boundary. An agent that drives forty strokes and
            // reads this can say *which call* was slow, which is the one thing
            // a total cannot say.
            state.strokes = report::stroke_state(&diagnostics);
        }
    }
}

impl Session for App {
    /// One command, down the path a menu item's click takes.
    ///
    /// `handle` and nothing else: there is no second way in, which is what
    /// makes an agent's edit and a person's edit the same edit — one history
    /// entry, undone by the same undo, refused for the same reasons.
    fn apply(&mut self, command: Command) -> Result<Applied, Refusal> {
        let label = command.label().to_string();
        let touched = command.touches_document();
        let before = self.notice_occurrences();

        // Whose gesture this is, recorded before the command is applied so the
        // next call from the same agent is not refused its own stroke.
        let opened_a_gesture = command.opens_a_gesture();
        self.agent_gesture.opening(&command);

        // A sample or a close with nothing open, before it reaches a ViewModel
        // that would answer `Ok(())` to it.
        if let Some(why) = stroke_needs_a_gesture(&command, self.sculpt.is_stroking()) {
            return Err(Refusal::new(RefusalCode::Unavailable, why));
        }

        // Cleared rather than assumed empty: a refusal left over from a
        // person's own click is not this command's.
        self.sculpt_refusal = None;
        self.handle(command);
        self.agent_gesture
            .settled(opened_a_gesture, self.a_commanded_gesture_is_open());

        // The ViewModel's own refusal, which the interface answers by putting
        // it in the options bar and the door has to answer with an error — a
        // stroke reported as applied is one an agent goes on building on.
        if let Some(refused) = self.sculpt_refusal.take() {
            return Err(refusal_for(&refused));
        }

        let (refused, notices) = self.notices_since(before);
        if let Some(said) = refused {
            return Err(Refusal::new(RefusalCode::ModelRefused, said));
        }
        if touched {
            self.agent.acted();
        }

        let history = *self.sculpt.history().get();
        Ok(Applied {
            label,
            touched_document: touched,
            history_depth: history.depth,
            // What the next undo would take back, which after a command that
            // banked nothing is the command *before* it — not the label of the
            // one just applied, which is what the last action carries and what
            // this used to answer with.
            undoes: history
                .can_undo
                .then(|| self.sculpt.next_undo().map(str::to_string))
                .flatten(),
            notices,
        })
    }

    fn read(&mut self, query: StateQuery) -> StateReport {
        // Every read below goes through `Observable::get`, which does not mark
        // anything changed. An agent polling the session must not be the
        // reason an idle application never sleeps.
        //
        // Gathered in four passes rather than one long run of `if`s. There are
        // twenty sections now, and the grouping says where each answer comes
        // from: the document, the shelf, the panels, and the diagnostics —
        // which is also the only group that costs anything to ask for.
        let mut state = StateReport::default();
        self.read_document(&query, &mut state);
        self.read_shelf(&query, &mut state);
        self.read_panels(&query, &mut state);
        self.read_diagnostics(&query, &mut state);
        state
    }

    /// One more frame, through the renderer that draws the window.
    ///
    /// Not a copy of what was presented: a surface texture is not reliably
    /// `COPY_SRC` across backends. This renders again into an offscreen target
    /// with the same camera, shading, overlays, cursors and quality the window
    /// is using — they are the renderer's own state and are whatever the last
    /// frame set them to.
    fn capture(&mut self, request: CaptureRequest) -> Result<Frame, Refusal> {
        let outstanding = self.outstanding_work();
        let bare = self.rigging && !self.skin_preview;
        let primitives = self.last_ui.clone();
        let pixels_per_point = self.last_ppp;
        let camera = self.camera;

        let Some(graphics) = self.graphics.as_mut() else {
            return Err(Refusal::new(
                RefusalCode::Unavailable,
                "the window is not open yet, so there is nothing to draw",
            ));
        };

        let window = graphics.surface.framebuffer();
        let (window_width, window_height) = (window.width, window.height);
        let (width, height) = match request.what {
            // The interface's primitives are in the window's own pixels, so a
            // whole-window capture is at the window's size whatever was asked
            // for. The answer says which, which is why the size is reported
            // rather than assumed by the caller.
            CaptureWhat::Window => (window_width, window_height),
            CaptureWhat::Viewport => (
                request.width.unwrap_or(window_width).max(1),
                request.height.unwrap_or(window_height).max(1),
            ),
        };

        // In the *window's* format, not the offscreen default. The renderer's
        // pipelines carry the surface's format — `Bgra8UnormSrgb` on macOS —
        // and a pass whose attachment is a different one fails validation and
        // leaves the texture untouched. That failure is a transparent black
        // frame with the reason only in a log, which is why this is not a
        // detail: the whole capture comes back empty and says nothing.
        let format = graphics.surface.format();
        let target =
            clayspace_view::OffscreenTarget::with_format(&graphics.gpu, width, height, format);
        // The window's scene sits in a sub-rectangle; an offscreen target is
        // all scene. The next frame sets this again from the layout, so
        // nothing has to put it back.
        graphics.renderer.set_scene_viewport(None);
        let mesh = if bare {
            &graphics.nothing
        } else {
            graphics.geometry.mesh()
        };
        let image = target.capture(&graphics.gpu, &graphics.renderer, &camera, mesh, true);

        let image = match request.what {
            CaptureWhat::Viewport => image,
            CaptureWhat::Window => {
                paint_primitives(
                    graphics,
                    &primitives,
                    [width, height],
                    pixels_per_point,
                    target.view(),
                );
                target.read_back_public(&graphics.gpu)
            }
        };

        // The seam carries RGBA, so a blue-first target is put in order here.
        // On the interface thread rather than the connection's, because the
        // order is a property of the device and the seam should not make every
        // caller ask about it. A megapixel of swaps is well under a frame.
        let mut rows = image.pixels;
        if target.is_bgra() {
            for pixel in rows.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }

        Ok(Frame {
            width: image.width,
            height: image.height,
            rows,
            outstanding,
        })
    }

    fn settle(&mut self, budget: Duration) -> Settled {
        let started = Instant::now();
        let uploaded = self.uploaded_bytes();
        // Drain actual debt. A blocked live gesture cannot finish its settle
        // on this thread, so return its outstanding work when a pass makes no
        // progress instead of spinning until the budget expires.
        loop {
            self.poll_jobs();
            let before = self.outstanding_work();
            self.finish_pending_geometry();
            self.poll_jobs();
            let after = self.outstanding_work();
            if after.is_empty() || after == before || started.elapsed() >= budget {
                break;
            }
        }
        let outstanding = self.outstanding_work();
        Settled {
            quiet: outstanding.is_empty(),
            waited_millis: started.elapsed().as_millis() as u64,
            uploaded_bytes: self.uploaded_bytes().saturating_sub(uploaded),
            outstanding,
        }
    }

    fn measure(&mut self, command: Command) -> Result<Measured, Refusal> {
        let label = command.label().to_string();
        let started = Instant::now();
        let uploaded = self.uploaded_bytes();
        Session::apply(self, command)?;
        // Include required geometry, but do not manufacture a full rebuild
        // after the command already synchronized its dirty region.
        self.finish_pending_geometry();
        let took = started.elapsed();

        let diagnostics = self.policy.diagnostics();
        Ok(Measured {
            label,
            millis: took.as_secs_f64() * 1000.0,
            uploaded_bytes: self.uploaded_bytes().saturating_sub(uploaded),
            stalled: took > clayspace_model::FRAME,
            backend: diagnostics.active_backend,
            platform: diagnostics.platform,
            live_session: true,
        })
    }

    /// Raises the ask at the window, and answers with what is there.
    ///
    /// Never waits. An interface thread waiting thirty seconds for somebody to
    /// click is an interface that has stopped drawing, so this puts the
    /// question up and says `Pending`; the connection thread comes back.
    fn consent(&mut self, ask: &Consent) -> ConsentOutcome {
        let gate = report::gate_for_the_window(ask.gate);
        let root = self.store.as_ref().map(|store| store.root().to_path_buf());

        if let Some(root) = root.as_ref() {
            if clayspace_mcp::access::read_consents(root)
                .iter()
                .any(|tag| tag == gate.tag())
            {
                return ConsentOutcome::AlreadyRecorded;
            }
        }

        if self.agent.raise(AgentAsk {
            id: ask.id,
            gate,
            operation: ask.operation.clone(),
            client: ask.client.clone(),
            path: ask.path.as_ref().map(|path| path.display().to_string()),
        }) {
            self.request_redraw();
        }

        match self.agent.take_answer(ask.id) {
            None => ConsentOutcome::Pending,
            Some(AgentAnswer::No) => ConsentOutcome::Refused,
            Some(AgentAnswer::Yes) => ConsentOutcome::Granted,
            Some(AgentAnswer::Always) => {
                if let Some(root) = root.as_ref() {
                    let mut recorded = clayspace_mcp::access::read_consents(root);
                    let tag = gate.tag().to_string();
                    if !recorded.contains(&tag) {
                        recorded.push(tag);
                    }
                    let _ = clayspace_mcp::access::write_consents(root, &recorded);
                }
                ConsentOutcome::Granted
            }
        }
    }

    fn gesture_in_progress(&self) -> bool {
        self.holding_a_gesture()
    }

    fn agent_gesture_in_progress(&self) -> bool {
        App::agent_gesture_in_progress(self)
    }
}

/// Whether a frame should pay a settle the last stroke owed.
///
/// Pulled out of `App::flush_pending_settle` so the rule can be tested: `App`
/// lives in the binary and no integration test can reach it, and the rule has
/// one case that must not regress quietly.
///
/// **A gesture being open is the whole guard.** Settling replaces every key
/// with one whole-document mesh, so doing it under a live drag throws away the
/// preview that drag is showing and fights the transaction that owns it. The
/// debt is not cancelled by a gesture starting — it keeps, and the frame after
/// that gesture ends pays it.
fn settle_is_due(owed: bool, gesture_open: bool) -> bool {
    owed && !gesture_open
}

/// Writes the recovery file if one is due, and starts the interval again from
/// the moment the write finished.
///
/// Pulled out of `App::maybe_autosave` for the reason [`settle_is_due`] is:
/// `App` lives in the binary and no integration test can reach it, and the
/// ordering here is the whole of the fix.
///
/// **The clock is stamped after the write rather than before it.** Stamped
/// before, the interval ran *during* the save, so a document whose save took
/// longer than the interval was due again the instant it finished — measured,
/// a save of 146 s and the next one already due, with the window unusable in
/// between and no idle gap anywhere in the log. Afterwards, the interval is
/// what it reads as: time the application was not saving.
///
/// It is stamped whether the write succeeded or failed, and that is what keeps
/// the concern the early stamp was there for: a save that keeps failing waits
/// out a full interval like any other, instead of being retried on every
/// wake-up.
fn autosave_when_due<E>(
    policy: AutosavePolicy,
    clock: &mut Instant,
    modified: bool,
    gesture_open: bool,
    write: impl FnOnce() -> Result<(), E>,
) -> Option<Result<(), E>> {
    if !policy.is_due(clock.elapsed(), modified, gesture_open) {
        return None;
    }
    let outcome = write();
    *clock = Instant::now();
    Some(outcome)
}

#[cfg(test)]
mod memory_meter {
    use super::MemoryMeter;
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    /// A meter and a counted reading, so a test can say how many walks of the
    /// brick cache a run of frames paid for.
    fn frame(meter: &mut MemoryMeter, at: Instant, reads: &Cell<usize>) -> (u64, u64) {
        meter.figures(at, || {
            reads.set(reads.get() + 1);
            Some((7, 11))
        })
    }

    /// The defect: the status bar asked the engine for this on every redraw,
    /// and answering means walking the whole brick cache. Sixty frames of an
    /// application nobody is touching bought sixty identical answers, at about
    /// 200% CPU on a worked document (#167).
    #[test]
    fn brick_stats_are_not_recomputed_without_a_change() {
        let reads = Cell::new(0);
        let mut meter = MemoryMeter::default();
        let started = Instant::now();

        assert_eq!(frame(&mut meter, started, &reads), (7, 11));
        assert_eq!(
            frame(&mut meter, started + Duration::from_millis(16), &reads),
            (7, 11),
            "the second frame must show the first frame's figure"
        );
        assert_eq!(
            reads.get(),
            1,
            "two frames inside the interval walked the cache {} times",
            reads.get()
        );
    }

    /// A whole second of frames is still one walk, whatever the frame rate.
    #[test]
    fn a_second_of_frames_is_one_reading() {
        let reads = Cell::new(0);
        let mut meter = MemoryMeter::default();
        let started = Instant::now();
        for frames in 0..60 {
            frame(
                &mut meter,
                started + Duration::from_millis(frames * 16),
                &reads,
            );
        }
        assert_eq!(reads.get(), 1);
    }

    /// And the figure is not frozen: a change is on screen within a second of
    /// happening, which is what makes the meter honest.
    #[test]
    fn the_figure_is_read_again_once_it_has_aged_out() {
        let reads = Cell::new(0);
        let mut meter = MemoryMeter::default();
        let started = Instant::now();
        frame(&mut meter, started, &reads);
        frame(&mut meter, started + <MemoryMeter>::INTERVAL, &reads);
        assert_eq!(reads.get(), 2);
    }

    /// Opening another document does not wait out the clock: the figure on
    /// screen belongs to the document that was just closed.
    #[test]
    fn a_replaced_document_is_read_at_once() {
        let reads = Cell::new(0);
        let mut meter = MemoryMeter::default();
        let started = Instant::now();
        frame(&mut meter, started, &reads);
        meter.forget();
        frame(&mut meter, started + Duration::from_millis(16), &reads);
        assert_eq!(reads.get(), 2);
    }

    /// A cache that cannot answer has not thereby freed its memory, so the
    /// last figure stands rather than the meter dropping to zero — and the
    /// failed call still costs one reading rather than one per frame.
    #[test]
    fn a_reading_that_fails_keeps_the_last_figure() {
        let mut meter = MemoryMeter::default();
        let started = Instant::now();
        assert_eq!(meter.figures(started, || Some((3, 9))), (3, 9));
        let refusals = Cell::new(0);
        let mut refused = |at: Instant| {
            meter.figures(at, || {
                refusals.set(refusals.get() + 1);
                None
            })
        };
        assert_eq!(refused(started + <MemoryMeter>::INTERVAL), (3, 9));
        assert_eq!(refused(started + <MemoryMeter>::INTERVAL), (3, 9));
        assert_eq!(
            refusals.get(),
            1,
            "a cache that stopped answering must not be asked once a frame"
        );
    }
}

#[cfg(test)]
mod settle_deferral {
    use super::settle_is_due;

    #[test]
    fn a_frame_with_nothing_owed_settles_nothing() {
        assert!(!settle_is_due(false, false));
        assert!(!settle_is_due(false, true));
    }

    #[test]
    fn a_debt_is_paid_once_the_pointer_is_up() {
        assert!(settle_is_due(true, false));
    }

    #[test]
    fn a_debt_waits_while_a_gesture_is_drawing() {
        // The case the guard exists for. A settle here would replace the live
        // preview mid-drag with a whole-document mesh, which is both wrong on
        // screen and a document edit under an open transaction.
        assert!(
            !settle_is_due(true, true),
            "a settle owed by the last stroke must not run inside the next one"
        );
    }
}

#[cfg(test)]
mod autosave_clock {
    use super::autosave_when_due;
    use clayspace_model::AutosavePolicy;
    use std::time::{Duration, Instant};

    /// Short enough that a test can sleep through it twice.
    const EVERY: Duration = Duration::from_millis(20);

    fn policy() -> AutosavePolicy {
        AutosavePolicy { every: EVERY }
    }

    /// A clock that says an autosave is long overdue.
    fn overdue() -> Instant {
        Instant::now() - EVERY * 50
    }

    #[test]
    fn the_autosave_clock_starts_after_the_save() {
        // The defect. The clock was stamped before the write, so the interval
        // ran while the save did: a document whose save took longer than the
        // interval was due again the moment it finished, and the application
        // spent its time saving instead of being usable between saves.
        let mut clock = overdue();

        let written = autosave_when_due(policy(), &mut clock, true, false, || {
            std::thread::sleep(EVERY * 3);
            Ok::<(), ()>(())
        });

        assert_eq!(written, Some(Ok(())), "an overdue autosave did not run");
        assert!(
            !policy().is_due(clock.elapsed(), true, false),
            "a save that took longer than the interval was due again the \
             moment it finished"
        );
    }

    #[test]
    fn a_failed_autosave_waits_out_the_interval_like_any_other() {
        // What the early stamp was protecting, kept: a save that cannot be
        // written fails quickly, and must not then be retried on every wake-up
        // of the event loop.
        let mut clock = overdue();

        let written = autosave_when_due(policy(), &mut clock, true, false, || {
            Err::<(), &str>("o disco recusou")
        });

        assert_eq!(written, Some(Err("o disco recusou")));
        assert!(
            !policy().is_due(clock.elapsed(), true, false),
            "a failing autosave was due again immediately"
        );
    }

    #[test]
    fn autosave_is_skipped_during_a_gesture() {
        let mut clock = overdue();
        let before = clock;

        let written = autosave_when_due(policy(), &mut clock, true, true, || -> Result<(), ()> {
            panic!("the document was written under an open gesture")
        });

        assert!(written.is_none());
        assert_eq!(
            clock, before,
            "a skipped tick restarted the clock, so the autosave the gesture \
             held off was put off by another interval"
        );
    }
}

#[cfg(test)]
mod curve_press {
    use super::*;

    /// The bug this ordering exists to stop.
    ///
    /// A press on the line used to fall through to the append, which put a
    /// point at the far *end* of the curve and selected it. Two things came of
    /// that: the line jumped to a place nobody clicked, and a double-click
    /// could never work, because its first press had already added a stray
    /// point before the second could be read as a double.
    #[test]
    fn a_press_on_the_line_is_spent_rather_than_appending() {
        assert_eq!(
            App::curve_press_action(false, None, Some(7), false),
            CurvePress::Consume
        );
    }

    #[test]
    fn a_double_on_the_line_splits_the_span_under_it() {
        assert_eq!(
            App::curve_press_action(true, None, Some(7), false),
            CurvePress::Insert(7)
        );
    }

    /// A control point sits *on* the guide, so asking the line first would
    /// insert a second point coincident with every point double-clicked.
    #[test]
    fn a_double_on_a_control_point_takes_the_point() {
        assert_eq!(
            App::curve_press_action(true, Some(2), Some(7), false),
            CurvePress::Grab(2)
        );
    }

    #[test]
    fn a_press_on_nothing_puts_a_point_down() {
        assert_eq!(
            App::curve_press_action(false, None, None, false),
            CurvePress::Append
        );
        assert_eq!(
            App::curve_press_action(true, None, None, false),
            CurvePress::Append
        );
    }

    #[test]
    fn the_modifier_adds_a_point_to_the_selection_instead_of_grabbing_it() {
        assert_eq!(
            App::curve_press_action(false, Some(3), None, true),
            CurvePress::Toggle(3)
        );
    }
}

#[cfg(test)]
mod curve_stroke {
    use super::*;

    /// A drag lays a chain; a click lays one point.
    ///
    /// Nothing distinguishes the two except this: a press that never travels
    /// never reaches the spacing, so the same code path serves both.
    #[test]
    fn a_stationary_press_lays_no_second_point() {
        let at = [0.5, 1.0, 0.0];
        assert!(!App::curve_draw_reaches(at, at, 0.12));
        assert!(!App::curve_draw_reaches(at, [0.5, 1.01, 0.0], 0.12));
    }

    #[test]
    fn a_drag_lays_one_once_it_has_travelled_a_tube_width() {
        let from = [0.0, 0.0, 0.0];
        assert!(App::curve_draw_reaches(from, [0.3, 0.0, 0.0], 0.12));
    }

    /// Spacing in tube-widths, so a fat tube gets a coarse chain and a fine
    /// one keeps its detail — the same drag, two radii, two answers.
    #[test]
    fn a_thicker_tube_gets_a_coarser_chain() {
        let from = [0.0, 0.0, 0.0];
        let to = [0.1, 0.0, 0.0];
        assert!(
            App::curve_draw_reaches(from, to, 0.02),
            "a fine tube should have laid a point over this distance"
        );
        assert!(
            !App::curve_draw_reaches(from, to, 0.5),
            "a fat tube should not have"
        );
    }

    /// And a floor, so a tube of almost no thickness does not ask for a point
    /// per pixel.
    #[test]
    fn a_hairline_tube_still_has_a_floor() {
        let from = [0.0, 0.0, 0.0];
        assert!(!App::curve_draw_reaches(from, [0.001, 0.0, 0.0], 1e-6));
    }
}

#[cfg(test)]
mod double_press {
    use super::*;
    use std::time::{Duration, Instant};

    fn at(x: f32, y: f32) -> egui::Pos2 {
        egui::pos2(x, y)
    }

    #[test]
    fn a_first_press_is_never_a_double() {
        assert!(!App::is_double_press(None, Instant::now(), at(10.0, 10.0)));
    }

    #[test]
    fn two_presses_in_the_same_place_and_moment_are_a_double() {
        let now = Instant::now();
        let last = Some((now, at(10.0, 10.0)));
        assert!(App::is_double_press(
            last,
            now + Duration::from_millis(120),
            at(11.0, 11.0)
        ));
    }

    #[test]
    fn a_slow_second_press_is_two_presses() {
        let now = Instant::now();
        let last = Some((now, at(10.0, 10.0)));
        assert!(!App::is_double_press(
            last,
            now + Duration::from_millis(900),
            at(10.0, 10.0)
        ));
    }

    /// Two clicks in the same instant at opposite ends of the viewport are a
    /// coincidence, not a gesture — and on a curve the difference matters,
    /// because the second one would split a span nowhere near the first.
    #[test]
    fn a_second_press_somewhere_else_is_two_presses() {
        let now = Instant::now();
        let last = Some((now, at(10.0, 10.0)));
        assert!(!App::is_double_press(
            last,
            now + Duration::from_millis(50),
            at(400.0, 300.0)
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        gizmo_geometry_update, notices_written, refusal_for, remark_for_an_agent, running_jobs,
        stroke_needs_a_gesture, tool_status, AgentGesture, GizmoGeometryUpdate, ToolStatusSources,
        NOTICE_REFUSAL_CHANNELS, NOTICE_REMARK_CHANNELS,
    };
    use clayspace_mcp::RefusalCode;
    use clayspace_model::{ModelError, Representation, Unavailable};
    use clayspace_vm::{Command, Progress};

    #[test]
    fn running_jobs_are_outstanding_until_collected() {
        let retopo = Some(Progress {
            label: "retopology".into(),
            fraction: Some(0.4),
        });
        let absent = None;
        let jobs = running_jobs(&[&retopo, &absent, &absent, &absent]);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].what, "retopology");
        assert_eq!(jobs[0].fraction, Some(0.4));
        assert!(running_jobs(&[&absent, &absent, &absent, &absent]).is_empty());
    }

    fn begin() -> Command {
        Command::BeginStroke {
            position: [0.0; 3],
            pressure: 1.0,
            modifiers: Default::default(),
        }
    }

    fn sample() -> Command {
        Command::ContinueStroke {
            position: [0.1, 0.0, 0.0],
            pressure: 1.0,
        }
    }

    /// A begin the ViewModel refused opened nothing, and the flag that says
    /// "the gesture in progress is the agent's" must not stay up on the
    /// intent alone.
    ///
    /// Left up, `App::holding_a_gesture` answers "no person is holding
    /// anything" for the rest of the session — so a **person's** real stroke
    /// stops being reported at the door, and their edits stop being protected
    /// from an agent's. That is worse than the silence issue #127 reported.
    #[test]
    fn a_refused_begin_leaves_the_agent_holding_nothing() {
        let mut gesture = AgentGesture::default();
        gesture.opening(&begin());
        assert!(
            gesture.held(),
            "the intent stands while the command is applied, or the agent is \
             refused its own stroke"
        );
        gesture.settled(true, false);
        assert!(!gesture.held());
    }

    /// And so does a refused manipulator drag, which is the same defect
    /// wearing a different coat.
    ///
    /// The settle used to be asked only of a stroke's begin, so a drag or an
    /// outline the ViewModel opened nothing for left the flag up for the rest
    /// of the session. With the narrow exemption in force that is worse than
    /// it was: the door reads the flag to decide what an agent holding a
    /// gesture may send, so one refused drag would refuse the agent every
    /// changing command afterwards.
    #[test]
    fn a_refused_manipulator_drag_leaves_the_agent_holding_nothing() {
        let grab = Command::BeginGizmoDrag(
            clayspace_model::GizmoHandle::Centre,
            [0.0; 3],
            [0.0, 0.0, 1.0],
        );
        let mut gesture = AgentGesture::default();
        gesture.opening(&grab);
        assert!(gesture.held(), "the intent stands while it is applied");
        gesture.settled(grab.opens_a_gesture(), false);
        assert!(!gesture.held());

        // A lasso is the third one, and answers the same way.
        let mut gesture = AgentGesture::default();
        let lasso = Command::BeginMaskOutline([0.0; 2], false);
        gesture.opening(&lasso);
        gesture.settled(lasso.opens_a_gesture(), false);
        assert!(!gesture.held());
    }

    /// And a begin that did open one is still the agent's, or it could not
    /// finish the stroke it started.
    #[test]
    fn a_begin_that_opened_a_stroke_is_still_the_agent_s() {
        let mut gesture = AgentGesture::default();
        gesture.opening(&begin());
        gesture.settled(true, true);
        assert!(gesture.held());
        gesture.opening(&Command::EndStroke);
        assert!(!gesture.held());
    }

    /// A command that is not a begin settles nothing: a sample arriving while
    /// the agent holds a stroke leaves it holding one.
    #[test]
    fn a_sample_does_not_settle_who_holds_the_gesture() {
        let mut gesture = AgentGesture::default();
        gesture.opening(&begin());
        gesture.settled(true, true);
        gesture.opening(&sample());
        gesture.settled(false, true);
        assert!(gesture.held());
    }

    /// A sample or a close with nothing open is refused rather than answered
    /// `Ok`.
    ///
    /// The ViewModel returns `Ok(())` for both, which is right for a person —
    /// their pointer release arrives whether or not the press opened anything
    /// — and is what let an agent whose begin was refused go on stroking and
    /// be told twice more that it had worked.
    #[test]
    fn a_sample_or_a_close_with_no_stroke_open_is_refused() {
        assert!(stroke_needs_a_gesture(&sample(), false).is_some());
        assert!(stroke_needs_a_gesture(&Command::EndStroke, false).is_some());
        assert!(stroke_needs_a_gesture(&sample(), true).is_none());
        assert!(stroke_needs_a_gesture(&Command::EndStroke, true).is_none());
        // A begin is what opens one, and a cancel with nothing open is a
        // no-op an agent may safely repeat.
        assert!(stroke_needs_a_gesture(&begin(), false).is_none());
        assert!(stroke_needs_a_gesture(&Command::CancelStroke, false).is_none());
    }

    /// The refusal an agent gets carries the code it branches on.
    ///
    /// An unavailable tool is answerable — choose another tool, or another
    /// layer — and an engine failure is not, so they do not share a code.
    #[test]
    fn a_tool_with_no_verb_here_is_refused_as_unavailable() {
        // Raspar rather than Pinçar, which used to stand here: #201 gave Pinçar
        // a field verb — `clay_layer_magnify_surface` at a negative strength —
        // so it is no longer a tool a field has none of. Raspar is: its verbs
        // are the grid's, the mesh's and the hierarchy's, and it names them in
        // the refusal, which is what makes it an answerable one.
        let refusal = refusal_for(&ModelError::Unavailable(Unavailable::NoVerbHere {
            active: Representation::Sdf,
            verbs: clayspace_model::ToolKind::Raspar.verbs(),
            note: None,
        }));
        assert_eq!(refusal.code, RefusalCode::Unavailable);
        assert!(!refusal.message.is_empty(), "{refusal:?}");
        assert_eq!(
            refusal_for(&ModelError::engine("the engine said no")).code,
            RefusalCode::ModelRefused
        );
    }

    /// Nothing to explain, which is the state the options bar is in almost
    /// always.
    fn quiet() -> ToolStatusSources<'static> {
        ToolStatusSources {
            document: None,
            operation: None,
            reference: None,
            mask: None,
            object: None,
            lattice: None,
            curve: None,
            armature: None,
            scene: None,
            sculpt: None,
        }
    }

    /// A refusal on one channel, with every other channel silent.
    ///
    /// Built from the width the door actually reads rather than written out,
    /// so a channel added to `App::refusal_channels` does not turn a test of
    /// the rule into a test of an old channel count.
    fn only(channel: usize, said: &str) -> [(bool, Option<&str>); NOTICE_REFUSAL_CHANNELS] {
        let mut refusals = [(false, None); NOTICE_REFUSAL_CHANNELS];
        refusals[channel] = (true, Some(said));
        refusals
    }

    /// A save that failed reaches the options bar.
    ///
    /// It did not. `DocumentVm::notice` has said "the last failure, for the
    /// interface to show" since it was written, and the chain that builds the
    /// options bar never read it — so a save refused because a hierarchy's
    /// side-car could not be written printed to stderr, which no sculptor is
    /// looking at, and the document silently stayed unsaved. Removing
    /// `document` from `tool_status` fails here.
    #[test]
    fn a_failed_save_reaches_the_options_bar() {
        assert_eq!(
            tool_status(ToolStatusSources {
                document: Some("the side-car could not be written"),
                ..quiet()
            }),
            Some("the side-car could not be written")
        );
    }

    /// A refused rebuild reaches the options bar.
    ///
    /// The same hole in a second Observable: `run_remesh` has claimed this
    /// reaches the screen since it was written and nothing read it, so a
    /// rebuild refused for an unusable resolution — and now a level refused
    /// for its peak, which is this branch's own new refusal — went to stderr
    /// and the click looked like it had done nothing. Removing `scene` from
    /// `tool_status` fails here.
    #[test]
    fn a_refused_rebuild_reaches_the_options_bar() {
        assert_eq!(
            tool_status(ToolStatusSources {
                scene: Some("that resolution will not fit"),
                ..quiet()
            }),
            Some("that resolution will not fit")
        );
    }

    /// An explicit refusal beats a standing condition, and the document's beats
    /// every other explicit one.
    ///
    /// The order is the behaviour: a sculptor who has just been told a save
    /// failed must not have that replaced by a tool note that was already true
    /// before the click.
    #[test]
    fn the_most_recent_explicit_refusal_is_the_one_shown() {
        assert_eq!(
            tool_status(ToolStatusSources {
                document: Some("save"),
                operation: Some("operation"),
                reference: Some("reference"),
                mask: Some("mask"),
                object: Some("object"),
                lattice: Some("lattice"),
                curve: Some("curve"),
                armature: Some("armature"),
                scene: Some("scene"),
                sculpt: Some("standing"),
            }),
            Some("save")
        );
        assert_eq!(
            tool_status(ToolStatusSources {
                scene: Some("scene"),
                sculpt: Some("standing"),
                ..quiet()
            }),
            Some("scene")
        );
    }

    /// Seven silences say nothing rather than an empty line.
    #[test]
    fn nothing_refused_says_nothing() {
        assert_eq!(tool_status(quiet()), None);
    }

    /// A refused repair, crossing, pass or rebuild reaches the options bar.
    ///
    /// The third hole of the same shape, and the largest: about ten command
    /// families run through the composition root rather than through a
    /// ViewModel, and every one of them printed its refusal to stderr and had
    /// no Observable at all. A repair asked for on an SDF layer changed
    /// nothing and said nothing. Removing `operation` from `tool_status` fails
    /// here.
    #[test]
    fn a_refused_operation_reaches_the_options_bar() {
        assert_eq!(
            tool_status(ToolStatusSources {
                operation: Some("aplica-se a camadas de grade; esta é SDF"),
                ..quiet()
            }),
            Some("aplica-se a camadas de grade; esta é SDF")
        );
    }

    /// And so does a refused cage, which had no line at all.
    ///
    /// `LatticeViewModel::notice` has carried a refusal since it was written
    /// and the options bar never read it, so a cage refused on a layer that
    /// will not take one, a control point dragged past what the field allows
    /// and an apply the engine would not make were each a button that did
    /// nothing and said nothing. Dropping `lattice` from `tool_status` fails
    /// here.
    #[test]
    fn a_refused_cage_reaches_the_options_bar() {
        assert_eq!(
            tool_status(ToolStatusSources {
                lattice: Some("esta camada não aceita uma gaiola"),
                ..quiet()
            }),
            Some("esta camada não aceita uma gaiola")
        );
        assert_eq!(
            tool_status(ToolStatusSources {
                curve: Some("a curva não aceita outro ponto"),
                ..quiet()
            }),
            Some("a curva não aceita outro ponto")
        );
        assert_eq!(
            tool_status(ToolStatusSources {
                armature: Some("não há esfera 7 neste esqueleto"),
                ..quiet()
            }),
            Some("não há esfera 7 neste esqueleto")
        );
    }

    /// And it reaches the agent door, which reads no other surface.
    ///
    /// The door decides a command was refused by comparing the notice channels
    /// either side of it. An operation whose refusal went only to stderr was
    /// therefore answered `isError: false` with `touched_document: true` — an
    /// agent told a repair, a crossing or a pass had happened to a document
    /// that was left byte-identical. Dropping the first pair from
    /// `App::notices_since` fails here.
    #[test]
    fn a_refused_operation_reaches_the_agent_door() {
        let (refused, notices) = notices_written(
            only(0, "a operação foi recusada"),
            [(false, None); NOTICE_REMARK_CHANNELS],
        );
        assert_eq!(refused.as_deref(), Some("a operação foi recusada"));
        assert!(notices.is_empty());
    }

    /// And so does the last one, which is what the panels were added as.
    ///
    /// A cage, a curve, a boolean and a rig each wrote their refusal onto a
    /// channel the door did not read, so the answer was success-shaped: a
    /// boolean over a hierarchy produced no layer and said nothing, and a
    /// refused cage apply threw the drags away in silence. The rule is that
    /// *every* channel in the list is read, not the first few.
    #[test]
    fn a_refusal_on_the_last_channel_is_still_the_command_s_answer() {
        let (refused, notices) = notices_written(
            only(
                NOTICE_REFUSAL_CHANNELS - 1,
                "uma hierarquia não pode ser operando",
            ),
            [(false, None); NOTICE_REMARK_CHANNELS],
        );
        assert_eq!(
            refused.as_deref(),
            Some("uma hierarquia não pode ser operando")
        );
        assert!(notices.is_empty());
    }

    /// A sentence already on screen is not this command's refusal.
    ///
    /// The channel holds the last thing said on it for as long as it is shown,
    /// so what makes a refusal *this* command's is that the channel was
    /// written between the two readings — not that it holds words.
    #[test]
    fn a_sentence_left_over_from_the_last_command_is_not_this_one_s() {
        let mut refusals = [(false, None); NOTICE_REFUSAL_CHANNELS];
        refusals[0] = (false, Some("a operação foi recusada"));
        refusals[1] = (false, Some("essa camada é uma grade"));
        let (refused, notices) = notices_written(refusals, [(false, None); NOTICE_REMARK_CHANNELS]);
        assert_eq!(refused, None);
        assert!(notices.is_empty());
    }

    /// A remark is carried beside the answer rather than as a refusal.
    ///
    /// A substituted tool did happen, and an agent told "this was refused"
    /// would undo something that worked.
    #[test]
    fn a_substituted_tool_is_a_remark_and_not_a_refusal() {
        let (refused, notices) = notices_written(
            [(false, None); NOTICE_REFUSAL_CHANNELS],
            [
                (true, Some("Padrão no lugar de Raspar")),
                (false, None),
                (false, None),
            ],
        );
        assert_eq!(refused, None);
        assert_eq!(notices, vec!["Padrão no lugar de Raspar".to_string()]);
    }

    /// The swap reaches an agent as which tool stood in for which, not as the
    /// shell's marker; every other remark passes through as written.
    #[test]
    fn a_substitution_is_named_to_an_agent() {
        let described = clayspace_model::Substitution {
            chosen: clayspace_model::ToolKind::Raspar,
            standing_in: clayspace_model::ToolKind::Planar,
            representation: clayspace_model::Representation::Sdf,
        }
        .describe();
        assert_eq!(
            remark_for_an_agent(Some(clayspace_vm::TOOL_SUBSTITUTED), Some(&described)),
            Some(described.as_str())
        );
        assert_eq!(
            remark_for_an_agent(Some("a máscara não congelou nada"), Some(&described)),
            Some("a máscara não congelou nada"),
            "only the swap marker is rewritten"
        );
        // A marker with nothing held behind it is still said, rather than
        // dropped: a swap nobody names is better than one nobody reports.
        assert_eq!(
            remark_for_an_agent(Some(clayspace_vm::TOOL_SUBSTITUTED), None),
            Some(clayspace_vm::TOOL_SUBSTITUTED)
        );
    }

    /// Every remark written by one command is its own sentence.
    ///
    /// The remark channels do not compete the way the refusal channels do: a
    /// substituted tool, a mask that froze nothing and a size the field
    /// brought back inside its bounds are separate things that all happened,
    /// and dropping any of them leaves the caller looking in the history for
    /// something nobody mentioned.
    #[test]
    fn every_remark_written_is_carried_beside_the_answer() {
        let (refused, notices) = notices_written(
            [(false, None); NOTICE_REFUSAL_CHANNELS],
            [
                (true, Some("Padrão no lugar de Raspar")),
                (true, Some("a máscara não congelou nada")),
                (
                    true,
                    Some("radius reaches 4.08 here, not the 400 asked for"),
                ),
            ],
        );
        assert_eq!(refused, None);
        assert_eq!(
            notices,
            vec![
                "Padrão no lugar de Raspar".to_string(),
                "a máscara não congelou nada".to_string(),
                "radius reaches 4.08 here, not the 400 asked for".to_string(),
            ]
        );
    }

    #[test]
    fn an_sdf_gizmo_settles_when_the_drag_ends() {
        assert_eq!(
            gizmo_geometry_update(&Command::EndGizmoDrag, true, Representation::Sdf),
            GizmoGeometryUpdate::Settle
        );
    }

    #[test]
    fn an_sdf_gizmo_settles_while_it_is_moving() {
        assert_eq!(
            gizmo_geometry_update(
                &Command::DragGizmo([1.0, 0.0, 0.0], false),
                true,
                Representation::Sdf,
            ),
            GizmoGeometryUpdate::Settle
        );
    }

    #[test]
    fn carried_geometry_does_not_rebuild_the_sdf_surface() {
        for representation in [Representation::Voxel, Representation::Mesh] {
            assert_eq!(
                gizmo_geometry_update(&Command::EndGizmoDrag, true, representation),
                GizmoGeometryUpdate::Incremental
            );
        }
    }
}
