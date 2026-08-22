//! Renaming a layer, as ClayCore 0.30.0 made it saveable (#92).
//!
//! The ABI named a layer when it was made and had nothing to change it, so the
//! name the interface showed was a record kept *beside* the document. It
//! looked right for the whole session and came back as the creation name on
//! the next open — a loss made visible, rather than caused, by #69 making
//! names readable at all.
//!
//! What is still the host's problem is in `a_voxel_layer_keeps_its_grid`:
//! nothing upstream makes names unique, and a voxel grid is reachable only by
//! name.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, DocumentModel, GestureSample, Representation, SceneModel, SculptModel, ToolKind,
};

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy).ok()
}

fn scratch(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("clayspace-rename-{name}.clayspace"));
    let _ = std::fs::remove_file(&path);
    path
}

/// The name the document reports for a key, as the interface would show it.
fn name_of(document: &ClayDocument, key: clayspace_model::LayerKey) -> Option<String> {
    document
        .scene()
        .layers
        .into_iter()
        .find(|layer| layer.key == key)
        .map(|layer| layer.name)
}

/// Every layer's name, in stack order.
fn names(document: &ClayDocument) -> Vec<String> {
    document
        .scene()
        .layers
        .into_iter()
        .map(|layer| layer.name)
        .collect()
}

#[test]
fn a_rename_survives_a_round_trip() {
    // The whole of #92 from this side.
    let Some(mut document) = document() else {
        return;
    };
    let key = document
        .add_layer("Camada", Representation::Sdf)
        .expect("a layer");
    document
        .rename_layer(key, "Orelha esquerda")
        .expect("rename");
    assert_eq!(name_of(&document, key).as_deref(), Some("Orelha esquerda"));

    let path = scratch("roundtrip");
    document.save(&path).expect("save");
    let mut reopened = document;
    reopened.open(&path).expect("open");

    let names = names(&reopened);
    assert!(
        names.iter().any(|n| n == "Orelha esquerda"),
        "the rename was lost on reload; the layers came back as {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "Camada"),
        "the layer came back under the name it was created with: {names:?}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_empty_name_is_refused() {
    // What a cleared text field submits. The document's name would be the only
    // one left to lose, so this is refused rather than accepted as a blank.
    let Some(mut document) = document() else {
        return;
    };
    let key = document
        .add_layer("Camada", Representation::Sdf)
        .expect("a layer");

    assert!(document.rename_layer(key, "").is_err());
    assert!(
        document.rename_layer(key, "   ").is_err(),
        "whitespace is an empty name with extra steps"
    );
    assert_eq!(
        name_of(&document, key).as_deref(),
        Some("Camada"),
        "a refused rename changed the name anyway"
    );
}

#[test]
fn a_name_is_trimmed_rather_than_stored_with_its_padding() {
    let Some(mut document) = document() else {
        return;
    };
    let key = document
        .add_layer("Camada", Representation::Sdf)
        .expect("a layer");
    document.rename_layer(key, "  Cabeça  ").expect("rename");
    assert_eq!(name_of(&document, key).as_deref(), Some("Cabeça"));
}

#[test]
fn two_sdf_layers_may_share_a_name() {
    // Names are not unique upstream and are not made unique here: nothing
    // about an SDF layer is looked up by name, so refusing a duplicate would
    // buy a guarantee that costs the artist a natural thing to do.
    let Some(mut document) = document() else {
        return;
    };
    let first = document
        .add_layer("A", Representation::Sdf)
        .expect("a layer");
    let second = document
        .add_layer("B", Representation::Sdf)
        .expect("another");

    document.rename_layer(second, "Detalhe").expect("rename");
    document
        .rename_layer(first, "Detalhe")
        .expect("a duplicate name is allowed on SDF layers");

    assert_eq!(name_of(&document, first).as_deref(), Some("Detalhe"));
    assert_eq!(name_of(&document, second).as_deref(), Some("Detalhe"));
}

#[test]
fn a_voxel_layer_keeps_its_grid_across_a_rename() {
    // The reason `engine_name` exists. A grid is fetched by name — the ABI has
    // no id-addressed accessor — so a rename that did not write through would
    // leave the grid unreachable.
    let Some(mut document) = document() else {
        return;
    };
    let key = document
        .add_layer("Volume", Representation::Voxel)
        .expect("a voxel layer");
    document.set_active_layer(key).expect("select it");
    document.rename_layer(key, "Bloco").expect("rename");

    assert_eq!(
        document.active_representation(),
        Representation::Voxel,
        "the renamed layer stopped being a voxel layer"
    );

    // Depositing is what actually fetches the grid, so a stale engine name
    // shows up here as a refusal rather than as a wrong name.
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.25,
                intensity: 0.9,
                ..Default::default()
            },
            &[GestureSample {
                position: [0.0, 0.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the grid became unreachable after the rename");
}

#[test]
fn a_voxel_layer_will_not_take_another_voxel_layers_name() {
    // The one uniqueness rule, enforced only where it can actually go wrong.
    // Two voxel layers sharing a name shadow one another's grid, because the
    // lookup answers with the first in stack order — so a stroke would land on
    // the wrong layer's volume.
    let Some(mut document) = document() else {
        return;
    };
    let first = document
        .add_layer("Volume A", Representation::Voxel)
        .expect("a voxel layer");
    let second = document
        .add_layer("Volume B", Representation::Voxel)
        .expect("another voxel layer");

    assert!(
        document.rename_layer(second, "Volume A").is_err(),
        "two voxel layers were allowed to share a name, which shadows a grid"
    );
    assert_eq!(name_of(&document, second).as_deref(), Some("Volume B"));

    // An SDF layer carrying the name is not the problem, so it is not refused.
    let sdf = document
        .add_layer("Superfície", Representation::Sdf)
        .expect("an sdf layer");
    document
        .rename_layer(sdf, "Volume A")
        .expect("an SDF layer may share a voxel layer's name");
    let _ = first;
}
