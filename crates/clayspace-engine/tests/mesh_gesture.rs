//! A stamping drag on a mesh has to build up, not replace itself.
//!
//! While a gesture is open the model previews it, and a *dragging* verb is
//! laid down again from its anchor on every segment — so what the last segment
//! did is taken back first, or the preview stacks segment on segment.
//!
//! A *stamping* verb is not delivered that way. The ViewModel sends it only
//! what is new (`ActiveStroke::pending` against `whole`), so there is nothing
//! to take back: taking the last segment back anyway erases the stroke as fast
//! as it is drawn, and the drag leaves only its final dab. Held together by
//! `MeshDeltas` coalescing — "a stroke that passes over the same vertex forty
//! times records where it started, once" — so continuing one record across the
//! segments still leaves the gesture as one undo.
//!
//! This went unseen because `SharedDocument` never forwarded `begin_gesture`,
//! so nothing in the running application ever previewed a mesh gesture at all.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

fn mesh_form() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    document
        .convert_layer(clayspace_model::Direction::SdfToMesh, 0.02, 0)
        .expect("into a mesh");
    document
}

/// Where the surface stands under a ray straight down -Z at (x, y).
fn height_at(document: &ClayDocument, x: f32, y: f32) -> Option<f32> {
    SculptModel::pick(document, [x, y, 4.0], [0.0, 0.0, -1.0]).map(|hit| hit[2])
}

/// The furthest any vertex is from where it was.
fn furthest_apart(before: &[[f32; 3]], after: &[[f32; 3]]) -> f32 {
    before
        .iter()
        .zip(after)
        .map(|(a, b)| (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0f32, f32::max))
        .fold(0.0f32, f32::max)
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.12,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// Four spots along the front of the form, far enough apart not to overlap.
const SPOTS: [(f32, f32); 4] = [(-0.30, 0.0), (-0.10, 0.0), (0.10, 0.0), (0.30, 0.0)];

#[test]
fn every_dab_of_a_drag_stays_on_a_mesh() {
    let mut document = mesh_form();
    let before: Vec<f32> = SPOTS
        .iter()
        .map(|(x, y)| height_at(&document, *x, *y).expect("the ray met the form"))
        .collect();

    // One gesture, delivered the way the ViewModel delivers a stamping tool:
    // each segment carries only the samples the model has not seen.
    document.begin_gesture();
    for (x, y) in SPOTS {
        let at = height_at(&document, x, y).expect("the ray met the form");
        document
            .apply_stroke(
                ToolKind::Padrao,
                brush(),
                &[GestureSample {
                    position: [x, y, at],
                    pressure: 1.0,
                    time: 0.0,
                }],
                [false; 3],
            )
            .expect("a segment");
    }
    document.end_gesture();

    let after: Vec<f32> = SPOTS
        .iter()
        .map(|(x, y)| height_at(&document, *x, *y).expect("the ray met the form"))
        .collect();

    let raised: Vec<bool> = before
        .iter()
        .zip(&after)
        .map(|(was, now)| now - was > 1e-3)
        .collect();
    assert_eq!(
        raised,
        vec![true; SPOTS.len()],
        "only some of the drag survived it — heights {before:?} became {after:?}. \
         A stamping drag that takes its last segment back keeps just the final dab, \
         which is a stroke that needs clicking rather than dragging"
    );
}

#[test]
fn a_drag_on_a_mesh_is_still_one_undo() {
    let mut document = mesh_form();
    let before = document.visible_mesh_geometry().0;

    document.begin_gesture();
    for (x, y) in SPOTS {
        let at = height_at(&document, x, y).expect("the ray met the form");
        document
            .apply_stroke(
                ToolKind::Padrao,
                brush(),
                &[GestureSample {
                    position: [x, y, at],
                    pressure: 1.0,
                    time: 0.0,
                }],
                [false; 3],
            )
            .expect("a segment");
    }
    document.end_gesture();

    let during = document.visible_mesh_geometry().0;
    assert!(
        furthest_apart(&before, &during) > 1e-2,
        "the drag moved nothing, so undoing it proves nothing"
    );

    let depth = document.history().depth;
    assert!(document.undo().expect("undo"), "there was nothing to undo");
    assert_eq!(
        document.history().depth,
        depth - 1,
        "the drag left more than one thing to take back — four segments \
         continuing one record is what keeps it to one"
    );

    // Bit exact, which is what `MeshDeltas` promises and what tells an undo
    // from something that mostly looks like one. Measured on the drawn
    // vertices rather than through a raycast: the sculptor's own ray query
    // keeps a residue of an undone stroke that the drawn surface does not,
    // which is a fault of its own and older than this.
    assert_eq!(
        furthest_apart(&before, &document.visible_mesh_geometry().0),
        0.0,
        "one undo did not put every vertex back where it was"
    );
}
