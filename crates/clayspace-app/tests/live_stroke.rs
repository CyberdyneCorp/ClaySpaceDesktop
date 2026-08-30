//! Do the live brushes actually show themselves *while* the pointer is down?
//!
//! The engine-level tests drive `ClayDocument` directly, so they prove the
//! transaction and its preview work. They cannot prove that a pointer reaches
//! them: the decision to send segments instead of holding the whole gesture is
//! the ViewModel's, and it is made from what the model answers when the stroke
//! opens. This drives the same chain the application runs — pick, begin,
//! continue — and looks at the surface the viewport would draw *before* the
//! stroke ends.

use clayspace_app::{ray_at, SharedDocument};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{SculptModel, ToolKind};
use clayspace_view::Camera;
use clayspace_vm::{Command, SculptViewModel};

fn viewport() -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(278.0, 92.0), egui::pos2(1032.0, 688.0))
}

struct Fixture {
    document: SharedDocument,
    sculpt: SculptViewModel,
    camera: Camera,
}

impl Fixture {
    fn new() -> Option<Self> {
        let policy = BackendPolicy::discover(None).ok()?;
        let document = ClayDocument::new(policy)
            .and_then(ClayDocument::with_starting_form)
            .ok()?;
        let document = SharedDocument::new(document);
        let sculpt = SculptViewModel::new(Box::new(document.clone()));
        let mut camera = Camera::default();
        match SculptModel::bounds(&document) {
            Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
            None => camera.frame_default(),
        }
        Some(Self {
            document,
            sculpt,
            camera,
        })
    }

    /// How steep the active layer's field has become. A deformer chain
    /// multiplies, so this is what a drag written per segment destroys.
    fn step_scale(&self) -> f32 {
        self.document.with(|document| {
            let key = clayspace_model::SceneModel::scene(document)
                .active_layer()
                .expect("an active layer")
                .key;
            clayspace_model::SceneModel::layer_cost(document, key)
                .expect("layer cost")
                .safe_step_scale
        })
    }

    fn pick(&self, point: egui::Pos2) -> Option<[f32; 3]> {
        let (origin, direction) = ray_at(&self.camera, viewport(), point)?;
        self.sculpt.pick(origin, direction)
    }

    /// A hash of the surface the viewport would draw, which is the preview's
    /// while a live gesture is drawing one.
    fn drawn(&self) -> (usize, u64) {
        self.document.with(|document| {
            let (cache, offset) = document.drawn_cache();
            let live = document.live_gesture_is_open();
            let (mesh, _) = cache
                .mesh(
                    (!live).then(|| document.document()),
                    clayspace_engine::claycore::BrickMeshParams {
                        gradient_normals: false,
                        colors: false,
                        gradient_eps: None,
                    },
                    &[],
                )
                .expect("mesh the drawn surface");
            let mut hash = 1469598103934665603u64;
            for position in mesh.positions() {
                for (axis, channel) in position.iter().enumerate() {
                    for byte in (((channel + offset[axis]) * 4096.0).round() as i32).to_le_bytes() {
                        hash ^= byte as u64;
                        hash = hash.wrapping_mul(1099511628211);
                    }
                }
            }
            (mesh.vertex_count(), hash)
        })
    }
}

#[test]
fn smoothing_shows_itself_before_the_stroke_ends() {
    let Some(mut fixture) = Fixture::new() else {
        eprintln!("no backend; skipped");
        return;
    };
    fixture
        .sculpt
        .dispatch(Command::SelectTool(ToolKind::Suavizar))
        .expect("choose the smoothing brush");

    // Across the middle of the viewport, which the framing puts on the model.
    let path: Vec<egui::Pos2> = (0..8)
        .map(|step| egui::pos2(600.0 + step as f32 * 14.0, 380.0))
        .collect();

    let before = fixture.drawn();
    let mut during: Vec<(usize, u64)> = Vec::new();
    let mut landed = 0;
    for point in &path {
        let Some(position) = fixture.pick(*point) else {
            continue;
        };
        let command = if landed == 0 {
            Command::BeginStroke {
                position,
                pressure: 1.0,
                modifiers: Default::default(),
            }
        } else {
            Command::ContinueStroke {
                position,
                pressure: 1.0,
            }
        };
        fixture.sculpt.dispatch(command).expect("a pointer sample");
        landed += 1;
        during.push(fixture.drawn());
    }
    assert!(landed >= 4, "the stroke never reached the model");

    assert!(
        during.iter().any(|state| *state != before),
        "the surface the viewport draws never changed while the pointer was \
         down: the smoothing brush is still arriving only when the stroke ends"
    );

    fixture.sculpt.dispatch(Command::EndStroke).expect("end");
    let after = fixture.drawn();
    assert_ne!(
        after, before,
        "the stroke changed nothing at all, so the check above proved nothing"
    );
}

/// Drags the pointer straight across the model in `segments` steps and returns
/// how steep the field ended up, plus whether the surface moved on the way.
fn drag_across(segments: usize) -> Option<(f32, bool, bool)> {
    let mut fixture = Fixture::new()?;
    fixture
        .sculpt
        .dispatch(Command::SelectTool(ToolKind::Mover))
        .expect("choose the move brush");

    // The same gesture whatever it is cut into: one travel, more or fewer
    // samples along it. Cutting a drag more finely used to cost the field a
    // grab per cut, which is the whole point of the comparison.
    const FROM: f32 = 600.0;
    const TRAVEL: f32 = 81.0;

    let before = fixture.drawn();
    let rested = fixture.step_scale();
    let mut followed = false;
    let mut clean_while_down = true;
    let mut landed = 0;
    for step in 0..=segments {
        let x = FROM + TRAVEL * step as f32 / segments as f32;
        let Some(position) = fixture.pick(egui::pos2(x, 380.0)) else {
            continue;
        };
        let command = if landed == 0 {
            Command::BeginStroke {
                position,
                pressure: 1.0,
                modifiers: Default::default(),
            }
        } else {
            Command::ContinueStroke {
                position,
                pressure: 1.0,
            }
        };
        fixture.sculpt.dispatch(command).expect("a pointer sample");
        landed += 1;
        followed |= fixture.drawn() != before;
        clean_while_down &= fixture.step_scale() == rested;
    }
    if landed < 4 {
        return None;
    }
    fixture.sculpt.dispatch(Command::EndStroke).expect("end");
    assert_ne!(
        fixture.drawn(),
        before,
        "the drag changed nothing at all, so nothing else here proves anything"
    );
    Some((fixture.step_scale(), followed, clean_while_down))
}

#[test]
fn a_drag_shows_itself_and_costs_the_field_the_same_however_it_is_cut() {
    // Move is live by a different route than Suavizar. The transaction hands
    // over no samples to mesh, so the drag is drawn by writing its resolved
    // grabs onto the layer, sampling them into the brick cache and undoing
    // them inside the same segment. Two things have to be true of that, and
    // only the ViewModel can show them: the surface follows the pointer, and
    // the document carries no part of the drag until it commits.
    let Some((coarse, followed, clean)) = drag_across(4) else {
        eprintln!("no backend, or the drag missed the model; skipped");
        return;
    };
    assert!(
        followed,
        "the surface the viewport draws never followed the pointer: a Move \
         that only appears on release is the regression this catches"
    );
    assert!(
        clean,
        "the layer's field moved while the pointer was down, so a preview \
         grab was left on it — the document is supposed to carry no part of a \
         drag until it commits"
    );

    // And the cost is the gesture's, not the segments'. A drag written per
    // segment multiplies the layer's Lipschitz bound once per segment, so
    // delivering the same drag three times more finely used to cost three
    // times the chain — measured on this form, a tenth of the step scale.
    let Some((fine, _, _)) = drag_across(12) else {
        return;
    };
    assert!(
        (coarse - fine).abs() < coarse * 0.05,
        "the same drag left the safe step scale at {coarse} in four segments \
         and {fine} in twelve: the gesture is being written per segment again"
    );
}

#[test]
fn the_gesture_hooks_reach_the_document() {
    let Some(fixture) = Fixture::new() else {
        eprintln!("no backend; skipped");
        return;
    };
    // These are *provided* methods on `SculptModel`, so a model that does not
    // forward them compiles and answers with the trait's default. The shared
    // document did not forward `begin_gesture` or `end_gesture` at all, which
    // left a dragging verb on a mesh stacking segment on segment through the
    // application while the engine's own tests showed it replaying cleanly
    // from its anchor. Nothing failed to build and nothing said so.
    // A clone of the shared handle, so the document can be asked what the
    // hooks did while the trait object still holds it.
    let mut model = fixture.document.clone();
    let model: &mut dyn SculptModel = &mut model;

    model.begin_gesture();
    assert!(
        fixture.document.with(|document| document.is_previewing()),
        "begin_gesture did not reach the document"
    );
    model.end_gesture();
    assert!(
        !fixture.document.with(|document| document.is_previewing()),
        "end_gesture did not reach the document"
    );

    assert!(
        model.open_live_gesture(ToolKind::Suavizar, [true, false, false]),
        "open_live_gesture did not reach the document"
    );
    assert!(fixture
        .document
        .with(|document| document.live_gesture_is_open()));
    model.discard_live_gesture();
    assert!(!fixture
        .document
        .with(|document| document.live_gesture_is_open()));
}
