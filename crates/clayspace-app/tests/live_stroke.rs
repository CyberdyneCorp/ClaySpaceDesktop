//! Does the smoothing brush actually show itself *while* the pointer is down?
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
