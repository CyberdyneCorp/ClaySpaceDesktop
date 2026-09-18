//! What cancelling a stroke costs a mesh layer, end to end.
//!
//! The ViewModel tests beside this one settle the arithmetic against a double.
//! This one settles it against the engine, because the arithmetic was wrong
//! *because* of what the engine does and nothing about that is visible from
//! the ViewModel's side: a mesh gesture is previewed while it is made and
//! banked as ONE record when it ends, however many segments drew it. Cancel
//! spent one undo per applied segment, so the first undo took the gesture back
//! and every one after it took back whatever was underneath — the gestures
//! already committed, and on a layer made a moment earlier, the layer.

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    ExchangeModel, ExportSettings, ImportSettings, Representation, SceneModel, ToolKind,
};
use clayspace_vm::{Command, SculptViewModel};

/// A document holding one mesh layer, which is the only route there is to one:
/// export the starting form and import it back.
fn with_a_mesh_layer() -> Option<SharedDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let path = std::env::temp_dir().join("clayspace-stroke-cancel.obj");
    let _ = std::fs::remove_file(&path);
    document
        .export_mesh(&path, ExportSettings::default())
        .ok()?;
    document
        .import_mesh(&path, ImportSettings::default())
        .ok()?;
    let _ = std::fs::remove_file(&path);
    let key = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)?;
    document.set_active_layer(key).ok()?;
    Some(SharedDocument::new(document))
}

/// The drawn geometry, quantised.
///
/// Quantised rather than compared as floats because the question is whether the
/// vertices came back, not whether they came back through the same arithmetic.
/// A mesh gesture's undo restores the values it recorded, so equality here is a
/// strong claim and a fair one.
fn digest(document: &SharedDocument) -> Vec<[i64; 3]> {
    document.with(|document| {
        let (positions, ..) = document.visible_mesh_geometry();
        positions
            .into_iter()
            .map(|position| position.map(|v| (f64::from(v) * 1e5).round() as i64))
            .collect()
    })
}

/// How many subtools the document has. No cancel may change it.
fn subtools(document: &SharedDocument) -> usize {
    document.with(|document| document.scene().layers.len())
}

/// Drags across the front of the layer, from wherever the ray lands.
///
/// `close` says whether the pointer comes up: an open gesture is what cancel is
/// for. `false` where no ray reached the surface, which is a fixture that
/// proves nothing rather than a failure.
fn drag(vm: &mut SculptViewModel, height: f32, close: bool) -> bool {
    let mut points = Vec::new();
    for step in 0..6 {
        let x = -0.4 + step as f32 * 0.16;
        if let Some(hit) = vm.pick([x, height, 4.0], [0.0, 0.0, -1.0]) {
            points.push(hit);
        }
    }
    let Some((first, rest)) = points.split_first() else {
        return false;
    };
    vm.dispatch(Command::BeginStroke {
        position: *first,
        pressure: 1.0,
        modifiers: Default::default(),
    })
    .expect("begin");
    for point in rest {
        vm.dispatch(Command::ContinueStroke {
            position: *point,
            pressure: 1.0,
        })
        .expect("continue");
    }
    if close {
        vm.dispatch(Command::EndStroke).expect("end");
    }
    true
}

#[test]
fn cancelling_a_mesh_stroke_leaves_the_committed_ones_standing() {
    let Some(document) = with_a_mesh_layer() else {
        return;
    };
    let mut vm = SculptViewModel::new(Box::new(document.clone()));
    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("tool");

    let present = subtools(&document);
    let bare = digest(&document);
    assert!(!bare.is_empty(), "the mesh layer is not being drawn");

    if !drag(&mut vm, -0.15, true) || !drag(&mut vm, 0.15, true) {
        return;
    }
    let committed = digest(&document);
    assert_ne!(
        committed, bare,
        "neither committed gesture moved a vertex, so this proves nothing"
    );

    if !drag(&mut vm, 0.0, false) {
        return;
    }
    vm.dispatch(Command::CancelStroke).expect("cancel");

    assert_eq!(
        digest(&document),
        committed,
        "cancelling the open gesture took back the gestures committed before it"
    );
    assert_eq!(
        subtools(&document),
        present,
        "cancelling a gesture removed a subtool"
    );
    assert_eq!(
        vm.history().get().depth,
        2,
        "the two committed gestures must still be there to undo"
    );
    assert!(
        !vm.history().get().can_redo,
        "a cancelled gesture is not an action the sculptor can ask back"
    );
}
