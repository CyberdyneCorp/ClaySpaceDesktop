//! A document holding everything at once, saved and opened again.
//!
//! Each feature has a persistence test of its own beside the engine — a mask,
//! a recorded pass, a rig, a hierarchy — and each of those builds a document
//! holding that one thing. What none of them does is the audit's question:
//! build one document with all of it, save it, open it in a fresh session and
//! ask whether what came back is what was saved. Until the gated operations
//! could be finished without a file panel (#192) nobody could ask it over the
//! door either, so this is the test that stood unrun.
//!
//! The comparison is a digest of what the document says about itself — every
//! layer's name, representation, visibility, protection and placement, each
//! grid's recorded passes and their strengths, each hierarchy's levels and the
//! checksum of its detail, each layer's mask, the rig, and where the surface
//! stands along a fan of directions including both sides of a mirrored stroke.
//! Layer keys are left out: they name a row, and a reader is free to number
//! rows again.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use clayspace_engine::{claycore, BackendPolicy, ClayDocument};
use clayspace_model::{
    ArmatureModel, BrushSettings, Direction, DocumentModel, ExchangeModel, GestureSample,
    GizmoTarget, ImportSettings, LayerKey, LayerSummary, MaskModel, MultiresLevelOp, ObjectModel,
    Representation, SceneModel, SculptLayerOp, SculptModel, ToolKind,
};

fn policy() -> BackendPolicy {
    BackendPolicy::from_available(vec![claycore::Backend::Cpu], None)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("clayspace-round-trip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

/// A flat grid of quads, the cage a hierarchy is subdivided from. Imported
/// from a file because that is the only route a mesh layer has in.
fn cage_obj(path: &Path) {
    const DIVISIONS: usize = 4;
    const HALF: f32 = 0.5;
    let mut text = String::new();
    let step = 2.0 * HALF / DIVISIONS as f32;
    for z in 0..=DIVISIONS {
        for x in 0..=DIVISIONS {
            let _ = writeln!(
                text,
                "v {} -1.5 {}",
                -HALF + step * x as f32,
                -HALF + step * z as f32
            );
        }
    }
    let stride = DIVISIONS + 1;
    for z in 0..DIVISIONS {
        for x in 0..DIVISIONS {
            let a = z * stride + x + 1;
            let _ = writeln!(text, "f {} {} {} {}", a, a + stride, a + stride + 1, a + 1);
        }
    }
    std::fs::write(path, text).expect("write the cage");
}

fn stroke(document: &mut ClayDocument, tool: ToolKind, at: &[[f32; 3]], mirror: [bool; 3]) {
    let samples: Vec<GestureSample> = at
        .iter()
        .enumerate()
        .map(|(i, position)| GestureSample {
            position: *position,
            pressure: 1.0,
            time: i as f32 * 0.02,
        })
        .collect();
    let brush = BrushSettings {
        size: 0.25,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    document.begin_gesture();
    let outcome = document.apply_stroke(tool, brush, &samples, mirror);
    document.end_gesture();
    assert!(
        outcome.expect("the stroke is applied").changed,
        "{tool:?} changed nothing"
    );
}

/// The document the audit could not save: a mirrored stroke and a mask on the
/// field, a grid with a dialled pass placed off-centre, a rig, and a
/// hierarchy two levels deep with its sculpt level moved.
fn author() -> ClayDocument {
    let mut document = ClayDocument::new(policy())
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");

    // The field: a stroke mirrored across X, and a mask beside it.
    stroke(
        &mut document,
        ToolKind::Padrao,
        &[[0.35, 0.2, 0.9], [0.45, 0.25, 0.85]],
        [true, false, false],
    );
    stroke(
        &mut document,
        ToolKind::Mascara,
        &[[0.0, -0.4, 0.9]],
        [false; 3],
    );
    assert!(document.mask_state().present, "the mask was not painted");

    // A grid crossed from the field, with a recorded pass dialled part way,
    // renamed, placed away from the origin at a scale, and hidden.
    let field = document.scene().active.expect("the starting field");
    let grid = document
        .convert_layer(Direction::SdfToVoxel, 0.04, 1)
        .expect("cross to a grid");
    document.set_active_layer(grid).expect("activate the grid");
    document
        .apply_sculpt_layer_op(SculptLayerOp::BeginRecording {
            name: "Detalhe".into(),
        })
        .expect("begin recording");
    stroke(
        &mut document,
        ToolKind::Padrao,
        &[[0.0, 0.0, 1.0]],
        [false; 3],
    );
    document
        .apply_sculpt_layer_op(SculptLayerOp::EndRecording)
        .expect("end recording");
    document
        .apply_sculpt_layer_op(SculptLayerOp::SetStrength {
            index: 0,
            strength: 0.37,
        })
        .expect("dial the pass");
    document.rename_layer(grid, "Grade").expect("rename");
    document
        .set_layer_transform(grid, [0.6, 0.1, -0.2], 1.25)
        .expect("place the grid");
    document
        .set_layer_visible(grid, false)
        .expect("hide the grid");

    // A rig on a layer of its own.
    let rig = document
        .add_layer("Esqueleto", Representation::Sdf)
        .expect("a layer for the rig");
    document
        .set_active_layer(rig)
        .expect("activate the rig layer");
    document.begin_armature([0.0, 2.0, 0.0], 0.3).expect("root");
    let shoulder = document
        .add_zsphere(0, [0.5, 2.0, 0.0], 0.2, false)
        .expect("shoulder");
    document
        .add_zsphere(shoulder, [0.9, 2.1, 0.0], 0.15, false)
        .expect("elbow");

    // A hierarchy: a flat cage subdivided twice, sculpted, with the sculpt
    // level moved below the top so both numbers have to come back.
    let cage = scratch("cage.obj");
    cage_obj(&cage);
    document
        .import_mesh(&cage, ImportSettings::default())
        .expect("import the cage");
    let mesh = last_of(&document, Representation::Mesh).expect("the cage is a mesh layer");
    document.set_active_layer(mesh).expect("activate the cage");
    document
        .convert_layer_in_place(Direction::MeshToMultires, 0.04, 0)
        .expect("a flat quad grid is a cage");
    for _ in 0..2 {
        document
            .apply_multires_level_op(MultiresLevelOp::AddLevel)
            .expect("subdivide");
    }
    stroke(
        &mut document,
        ToolKind::Padrao,
        &[[0.0, -1.5, 0.0]],
        [false; 3],
    );
    document
        .apply_multires_level_op(MultiresLevelOp::SetSculptLevel(1))
        .expect("move the sculpt level");

    // A crossing and a new rig each hide what was there before them. The
    // field is shown again, so the surface probes read the mirrored stroke.
    document
        .set_layer_visible(field, true)
        .expect("show the field");
    document
}

fn last_of(document: &ClayDocument, representation: Representation) -> Option<LayerKey> {
    document
        .scene()
        .layers
        .iter()
        .rev()
        .find(|layer| layer.representation == representation)
        .map(|layer| layer.key)
}

/// Where the active layer's surface stands along a direction from the origin,
/// to the millimetre, or `-` where nothing is met.
fn reach(document: &ClayDocument, direction: [f32; 3]) -> String {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    SculptModel::pick(document, unit.map(|c| c * 4.0), unit.map(|c| -c))
        .map(|hit| format!("{:.3}", hit.iter().map(|c| c * c).sum::<f32>().sqrt()))
        .unwrap_or_else(|| "-".into())
}

/// The directions a field's surface is read along: both sides of the mirrored
/// stroke, the mask, and two that neither touched.
const PROBES: [[f32; 3]; 5] = [
    [0.4, 0.22, 0.87],
    [-0.4, 0.22, 0.87],
    [0.0, -0.4, 0.9],
    [0.0, 0.0, 1.0],
    [0.7, 0.0, 0.7],
];

fn round(values: &[f32]) -> String {
    values
        .iter()
        .map(|v| format!("{v:.4}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// What the document says about itself, in a form two documents can be
/// compared by.
///
/// Takes `&mut` because a mask, a rig and a pick are read off the active
/// layer, so each layer is made active in turn; the active layer is put back
/// after.
fn digest(document: &mut ClayDocument) -> String {
    let scene = document.scene();
    let mut out = String::new();
    for layer in &scene.layers {
        describe_layer(document, layer, &mut out);
        document.set_active_layer(layer.key).expect("activate");
        describe_active(document, layer.representation, &mut out);
    }
    if let Some(active) = scene.active {
        document
            .set_active_layer(active)
            .expect("restore the active layer");
    }
    out
}

/// What the layer summary and the layer's placement say.
fn describe_layer(document: &mut ClayDocument, layer: &LayerSummary, out: &mut String) {
    let _ = writeln!(
        out,
        "layer {:?} {:?} visible={} {:?}",
        layer.name, layer.representation, layer.visible, layer.protection
    );
    if let Some(at) = document.target_transform(GizmoTarget::Layer(layer.key)) {
        let _ = writeln!(
            out,
            "  at [{}] turn [{}] {:.4} scale [{}]",
            round(&at.position),
            round(&at.rotation_axis),
            at.rotation_angle,
            round(&at.scale)
        );
    }
    let placed = document
        .objects()
        .iter()
        .filter(|form| form.id.layer == layer.key)
        .count();
    let _ = writeln!(out, "  forms {placed}");
    for pass in &layer.sculpt_layers {
        let _ = writeln!(
            out,
            "  pass {:?} strength={:.4} visible={}",
            pass.name, pass.strength, pass.visible
        );
    }
    if let Some(hierarchy) = &layer.multires {
        let _ = writeln!(
            out,
            "  levels {:?} passes {:?} checksum {:?}",
            hierarchy.levels,
            hierarchy.sculpt_layers,
            document.hierarchy_checksum(layer.key)
        );
    }
}

/// What is read off the active layer: its mask, its rig, and — for a field —
/// where its surface stands.
fn describe_active(document: &ClayDocument, representation: Representation, out: &mut String) {
    let mask = document.mask_state();
    let _ = writeln!(
        out,
        "  mask present={} cells={}",
        mask.present, mask.painted_cells
    );
    for node in document.armature().map(|rig| rig.nodes).unwrap_or_default() {
        let _ = writeln!(
            out,
            "  zsphere [{}] r={:.4} parent={} negative={}",
            round(&node.position),
            node.radius,
            node.parent,
            node.negative
        );
    }
    if representation == Representation::Sdf {
        for direction in PROBES {
            let _ = writeln!(
                out,
                "  reach [{}] {}",
                round(&direction),
                reach(document, direction)
            );
        }
    }
}

#[test]
fn a_document_with_everything_in_it_comes_back_as_it_was_saved() {
    let mut authored = author();
    let path = scratch("everything.clayspace");
    authored.save(&path).expect("save");
    let saved = digest(&mut authored);

    // A different session opening the file, not the one that wrote it.
    let mut reopened = ClayDocument::new(policy()).expect("a document");
    reopened.open(&path).expect("reopen");
    let restored = digest(&mut reopened);

    for (what, needle) in [
        ("the grid's pass", "pass \"Detalhe\" strength=0.3700"),
        ("the grid's placement", "at [0.6000,0.1000,-0.2000]"),
        ("the mask", "mask present=true"),
        ("the rig", "zsphere [0.9000,2.1000,0.0000]"),
        ("the hierarchy", "levels"),
        ("the field", "reach [0.0000,0.0000,1.0000] 1."),
    ] {
        assert!(
            saved.contains(needle),
            "the fixture holds no {what}, so this compares nothing:\n{saved}"
        );
    }
    assert_eq!(
        saved, restored,
        "what came back is not what was saved\n--- saved\n{saved}\n--- reopened\n{restored}"
    );
}
