//! Whether a recorded pass survives a save and a reload.
//!
//! The whole value of a pass is that its strength stays adjustable *long after*
//! the strokes are finished — and "long after" runs past the end of a session
//! or it means nothing. A pass that flattened on save would leave a sculptor
//! with the right surface and no way to change their mind about it, which is
//! exactly the promise the feature makes and the one it would break.
//!
//! The format says it carries them: `.clayspace` minor 10 "adds sculpt layers
//! to the voxel payload", and the container treats that payload as opaque, so
//! the bump is a reader signal rather than a layout change. This measures the
//! claim rather than trusting it.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, DocumentModel, GestureSample, SceneModel, SculptLayerOp, SculptModel,
    ToolKind,
};

fn with_grid() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let source = document.scene().active.expect("a starting layer");
    document
        .convert_layer(Direction::SdfToVoxel, 0.04, 1)
        .expect("cross to a grid");
    document.remove_layer(source).expect("drop the field");
    document
}

fn stroke(document: &mut ClayDocument) -> bool {
    let brush = BrushSettings {
        size: 0.3,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    let samples: Vec<GestureSample> = (0..6)
        .map(|i| {
            let t = i as f32 / 5.0;
            GestureSample {
                position: [(t - 0.5) * 0.5, 0.0, 1.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])
        .map(|outcome| outcome.changed)
        .unwrap_or(false)
}

fn passes(document: &ClayDocument) -> Vec<clayspace_model::SculptLayer> {
    let scene = document.scene();
    scene
        .active
        .and_then(|key| scene.layer(key).cloned())
        .map(|layer| layer.sculpt_layers)
        .unwrap_or_default()
}

/// A document with one recorded pass, dialled to a value neither 0 nor 1 —
/// those two are exact by construction and would survive a flatten as well.
fn with_a_dialled_pass(name: &str) -> (ClayDocument, std::path::PathBuf, usize) {
    let mut document = with_grid();
    document
        .apply_sculpt_layer_op(SculptLayerOp::BeginRecording {
            name: "Detalhe".into(),
        })
        .expect("begin recording");
    assert!(
        stroke(&mut document),
        "the fixture stroke changed nothing, so there is no pass to dial"
    );
    document
        .apply_sculpt_layer_op(SculptLayerOp::EndRecording)
        .expect("end recording");
    document
        .apply_sculpt_layer_op(SculptLayerOp::SetStrength {
            index: 0,
            strength: 0.4,
        })
        .expect("dial the pass back");
    let cells = document.occupied_cells().expect("a grid counts its cells");

    let path = std::env::temp_dir().join(format!("clayspace-passes-{name}.clayspace"));
    let _ = std::fs::remove_file(&path);
    document.save(&path).expect("save");
    (document, path, cells)
}

#[test]
fn a_recorded_pass_and_its_strength_survive_a_reload() {
    let (_saved, path, cells) = with_a_dialled_pass("strength");

    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut reopened = ClayDocument::new(policy).expect("a document");
    reopened.open(&path).expect("reopen the document");

    // The grid has to be the active layer for the stack to be the one read.
    let key = reopened
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == clayspace_model::Representation::Voxel)
        .map(|layer| layer.key)
        .expect("the reopened document carries a voxel layer");
    reopened.set_active_layer(key).expect("activate the grid");

    let stack = passes(&reopened);
    assert_eq!(
        stack.len(),
        1,
        "the pass did not survive the reload; the sculpt is flattened and no \
         longer dialable"
    );
    assert!(
        (stack[0].strength - 0.4).abs() < 1e-3,
        "the pass came back at strength {} rather than the 0.4 it was saved at",
        stack[0].strength
    );
    assert_eq!(
        stack[0].name, "Detalhe",
        "the pass came back unnamed, so a sculptor cannot tell their passes apart"
    );
    assert_eq!(
        reopened.occupied_cells(),
        Some(cells),
        "the reloaded grid does not hold what the saved one did"
    );
    let _ = std::fs::remove_file(&path);
}

/// And it is still adjustable afterwards, which is the point — a strength that
/// reads back correctly and cannot be changed is a number, not a slider.
#[test]
fn a_reloaded_pass_can_still_be_dialled() {
    let (_saved, path, _) = with_a_dialled_pass("dial");
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut reopened = ClayDocument::new(policy).expect("a document");
    reopened.open(&path).expect("reopen");
    let key = reopened
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == clayspace_model::Representation::Voxel)
        .map(|layer| layer.key)
        .expect("a voxel layer");
    reopened.set_active_layer(key).expect("activate");
    if passes(&reopened).is_empty() {
        // The other test says why this matters; here it only means there is
        // nothing to dial.
        return;
    }

    let dialled_in = {
        reopened
            .apply_sculpt_layer_op(SculptLayerOp::SetStrength {
                index: 0,
                strength: 1.0,
            })
            .expect("dial up");
        reopened.occupied_cells().expect("a grid")
    };
    reopened
        .apply_sculpt_layer_op(SculptLayerOp::SetStrength {
            index: 0,
            strength: 0.0,
        })
        .expect("dial away");
    let dialled_away = reopened.occupied_cells().expect("a grid");

    assert_ne!(
        dialled_in, dialled_away,
        "a reloaded pass reports a strength and changes nothing when it moves, \
         so what came back is the number and not the recorded cells"
    );
    let _ = std::fs::remove_file(&path);
}
