//! Holding a mesh sculptor for more than one subtool.
//!
//! The document used to hold exactly one. That was right while there was one
//! thing to sculpt, and wrong the moment a scene became a list of subtools: a
//! second carried mesh evicted the first, so going back and forth between two
//! of them paid a weld and an adjacency pass over every triangle on every
//! switch — 160 ms against the 16 ms the specification allows an engine
//! operation to hold the interface thread, and paid on a viewport click as
//! well as a stack row. `subtool.activate.mesh` is the figure, and the bound
//! it now holds is what `benchmarks/` gates.
//!
//! Holding several buys back that cost and introduces one bug class in its
//! place: a sculptor outliving the mesh it was built over. The engine refuses
//! rather than reads freed storage — its handle "remembers what it was built
//! over and every call checks that the answer has not changed" — so what would
//! reach a sculptor is not a crash but a brush that quietly stops working.
//! These are the paths where that could happen.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, ExchangeModel, ExportSettings, GestureSample, ImportSettings,
    LayerKey, Representation, SceneModel, SculptModel, ToolKind,
};

fn scratch(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("clayspace-sculptor-cache-{name}"));
    let _ = std::fs::remove_file(&path);
    path
}

/// A document holding two carried meshes and the field they came from.
///
/// Two, because one is the case that always worked: with a single carried mesh
/// the one slot was never evicted, and the cost this is about only appears
/// when a second mesh subtool exists to switch to.
fn two_meshes(who: &str) -> (ClayDocument, LayerKey, LayerKey, LayerKey) {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let field = document.scene().active.expect("a starting layer");

    let path = scratch(&format!("{who}.obj"));
    document
        .export_mesh(&path, ExportSettings::default())
        .expect("export the starting form");
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import it back as a subtool");
    let first = mesh_layers(&document)[0];

    document.set_active_layer(field).expect("back to the field");
    let second = document
        .convert_layer(Direction::SdfToMesh, 0.05, 1)
        .expect("cross the field to a second mesh");
    let _ = std::fs::remove_file(&path);
    (document, field, first, second)
}

fn mesh_layers(document: &ClayDocument) -> Vec<LayerKey> {
    document
        .scene()
        .layers
        .iter()
        .filter(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .collect()
}

/// One dab, reported as whether it moved anything.
///
/// A stale sculptor is a refusal rather than a wrong answer, so an `Err` here
/// is the shape the defect would take.
fn dab(document: &mut ClayDocument, at: [f32; 3]) -> Result<bool, String> {
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[
                GestureSample {
                    position: at,
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [at[0] + 0.05, at[1], at[2]],
                    pressure: 1.0,
                    time: 1.0,
                },
            ],
            [false; 3],
        )
        .map(|outcome| outcome.changed)
        .map_err(|error| error.to_string())
}

/// Both mesh subtools stay sculptable however often the sculptor moves between
/// them — which is what holding one each is for.
#[test]
fn two_mesh_subtools_are_both_sculptable_across_switches() {
    let (mut document, field, first, second) = two_meshes("both");
    for round in 0..3 {
        for (which, key) in [("first", first), ("second", second)] {
            document.set_active_layer(key).expect("activate a mesh");
            dab(&mut document, [0.0, 0.0, 1.0])
                .unwrap_or_else(|e| panic!("round {round}, the {which} mesh refused a dab: {e}"));
        }
        // Through the field as well, since that is the switch a sculptor
        // actually makes: work a form, adjust the field, come back.
        document
            .set_active_layer(field)
            .expect("activate the field");
    }
}

/// A mesh subtool removed and brought back by history is sculptable again.
///
/// The sharp edge of holding several: the layer comes back under the key it
/// left with — the whole point of the retired record — while the engine
/// restores its mesh. A sculptor kept across that would be built over
/// geometry the document may no longer hold, and every call on one of those
/// refuses.
///
/// This holds the *outcome* rather than the mechanism, and it passes with the
/// eviction on that path taken out: measured, the restored layer keeps the
/// same mesh behind its handle, so the sculptor still resolves. The eviction
/// stays because nothing in the ABI promises that — see `sculptors::forget`.
#[test]
fn a_mesh_subtool_history_brings_back_can_be_sculpted() {
    let (mut document, _field, first, _second) = two_meshes("restored");
    document.set_active_layer(first).expect("activate the mesh");
    dab(&mut document, [0.0, 0.0, 1.0]).expect("a dab before it is removed");

    document
        .remove_layer(first)
        .expect("remove the mesh subtool");
    assert!(
        !mesh_layers(&document).contains(&first),
        "the subtool should have left the scene"
    );

    assert!(document.undo().expect("undo"), "the removal is undoable");
    assert!(
        mesh_layers(&document).contains(&first),
        "the subtool comes back under the key it left with"
    );

    document
        .set_active_layer(first)
        .expect("activate the restored subtool");
    dab(&mut document, [0.0, 0.0, 1.0])
        .expect("a restored mesh subtool takes a dab rather than refusing one");
}

/// Removing one mesh subtool leaves the other's sculptor alone.
///
/// The eviction that matters is by key, not "whatever was held": a removal
/// that cleared the lot would put the cost back on the next switch, quietly.
#[test]
fn removing_one_mesh_subtool_does_not_disturb_the_other() {
    let (mut document, _field, first, second) = two_meshes("neighbour");
    for key in [first, second] {
        document.set_active_layer(key).expect("activate");
        dab(&mut document, [0.0, 0.0, 1.0]).expect("a dab on each");
    }
    document.remove_layer(first).expect("remove the first");
    document
        .set_active_layer(second)
        .expect("activate the survivor");
    dab(&mut document, [0.0, 0.0, 1.0]).expect("the survivor still sculpts");
}

/// Undoing an ordinary mesh stroke does not cost the layer its sculptor.
///
/// Reconciliation runs on every undo and is where layers that left the scene
/// give up what this side held for them. A stroke takes no layer out of the
/// scene, so a sculptor that did not survive one would put the 160 ms back on
/// the next dab after every ⌘Z — the cost this change removes, restored by the
/// same change's own bookkeeping.
#[test]
fn undoing_a_mesh_stroke_keeps_the_sculptor() {
    let (mut document, _field, first, _second) = two_meshes("undo");
    document.set_active_layer(first).expect("activate the mesh");
    assert!(
        dab(&mut document, [0.0, 0.0, 1.0]).expect("a first dab"),
        "the dab should have moved vertices"
    );
    assert!(document.undo().expect("undo"), "the stroke is undoable");
    dab(&mut document, [0.0, 0.0, 1.0]).expect("the mesh still sculpts after an undo");
    assert!(
        document.mesh_quality().is_some(),
        "the layer still has a sculptor to report from"
    );
}
