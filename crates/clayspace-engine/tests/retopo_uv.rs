//! Optional UVs on a retopology, from the run to the file.
//!
//! The chain the production workflow promises: retopo → quads → UV → a new
//! fixed-mesh layer that carries the layout through a save, an undo and an
//! export. Each link is checked against what the engine's layer holds, not
//! against the report the run returned.

use clayspace_engine::claycore;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Direction, DocumentModel, ExchangeModel, ExportSettings, ExportWarningKind, LayerKey,
    RetopoModel, RetopoSettings, RetopoUv, SceneModel, SculptModel, UvSettings,
};

fn meshed() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    document.convert_layer(Direction::SdfToMesh, 0.05, 0).ok()?;
    Some(document)
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("clayspace-retopo-uv");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

fn active_key(document: &ClayDocument) -> LayerKey {
    document
        .scene()
        .active_layer()
        .expect("an active layer")
        .key
}

fn with_uvs() -> RetopoSettings {
    RetopoSettings {
        target_quads: 600,
        // The merge pass trial-unwraps chart unions; off keeps the test in
        // the milliseconds, and it is not what is under test.
        uv: Some(UvSettings {
            max_chart_distortion: 0.0,
            ..UvSettings::default()
        }),
        ..RetopoSettings::default()
    }
}

#[test]
fn an_accepted_retopology_keeps_its_uvs_through_save_undo_and_export() {
    let Some(mut document) = meshed() else {
        return;
    };
    let source = active_key(&document);
    let outcome = document
        .retopologise(with_uvs())
        .expect("the retopology runs");
    let report = match &outcome.uv {
        RetopoUv::Laid(report) => *report,
        other => panic!("UVs were asked for and the outcome is {other:?}"),
    };
    println!(
        "{} faces, {} vertices, {} charts, {} seam edges, {:.0}% packed",
        outcome.faces,
        outcome.vertices,
        report.charts,
        report.seam_edges,
        report.packed_area * 100.0
    );
    assert!(report.charts > 0, "a layout with no charts");
    assert!(outcome.is_quads(), "laying out UVs cost the quads");

    // What the layer holds, not what the run reported.
    let placed = active_key(&document);
    assert_ne!(placed, source);
    let uvs = document
        .layer_uvs(placed)
        .expect("the layer is readable")
        .expect("the accepted mesh carries no UVs");
    assert_eq!(uvs.len(), outcome.vertices, "not one UV per vertex");
    assert!(
        uvs.iter()
            .all(|uv| (0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1])),
        "a UV fell outside the unit square the atlas packs into"
    );
    // A seam duplicates the vertices on it and nothing else, so a layout
    // cannot have split more than a fraction of the mesh.
    assert!(
        outcome.vertices < outcome.faces * 2,
        "{} vertices for {} faces: the seams split the mesh apart",
        outcome.vertices,
        outcome.faces
    );
    assert_eq!(
        document.layer_uvs(source).expect("the source is readable"),
        None,
        "the sculpt was given the result's UVs"
    );

    // A save and an open.
    let path = scratch("accepted.clayspace");
    document.save(&path).expect("save");
    let mut reopened = meshed().expect("a second document");
    reopened.open(&path).expect("open");
    let name = document
        .scene()
        .active_layer()
        .expect("active")
        .name
        .clone();
    let key = reopened
        .scene()
        .layers
        .iter()
        .find(|layer| layer.name == name)
        .expect("the result came back")
        .key;
    assert_eq!(
        reopened.layer_uvs(key).expect("readable"),
        Some(uvs.clone()),
        "the UVs did not survive a save and an open"
    );

    // An undo and a redo.
    assert!(document.undo().expect("undo"), "nothing to undo");
    assert!(document.redo().expect("redo"), "nothing to redo");
    let redone = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.name == name)
        .expect("redo brought the result back")
        .key;
    assert_eq!(
        document.layer_uvs(redone).expect("readable"),
        Some(uvs.clone()),
        "the UVs did not survive an undo and a redo"
    );

    // An export beside the visible sculpt: the meshed field carries no UVs,
    // and the engine drops an attribute any input lacks, so the file has none
    // — and the export says so rather than writing it quietly.
    let file = scratch("beside.obj");
    let warnings = document
        .export_mesh(&file, ExportSettings::default())
        .expect("export");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.kind == ExportWarningKind::DroppedUvs),
        "the layout was dropped from the file and nothing said so: {warnings:?}"
    );

    // An export of that mesh alone: everything else hidden, so the file is
    // the result and carries its layout.
    let others: Vec<LayerKey> = document
        .scene()
        .layers
        .iter()
        .map(|layer| layer.key)
        .filter(|&key| key != redone)
        .collect();
    for key in others {
        document
            .set_layer_visible(key, false)
            .expect("a layer hides");
    }
    let file = scratch("accepted.obj");
    let warnings = document
        .export_mesh(&file, ExportSettings::default())
        .expect("export");
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.kind == ExportWarningKind::DroppedUvs),
        "{warnings:?}"
    );
    let written = claycore::Mesh::load(&file).expect("the file reads back");
    let written_uvs = written.uvs().expect("the exported file carries no UVs");
    assert_eq!(written_uvs, &uvs[..], "the file carries a different layout");
}

#[test]
fn a_retopology_not_asked_for_uvs_carries_none() {
    let Some(mut document) = meshed() else {
        return;
    };
    let outcome = document
        .retopologise(RetopoSettings {
            target_quads: 600,
            ..RetopoSettings::default()
        })
        .expect("the retopology runs");
    assert_eq!(outcome.uv, RetopoUv::NotRequested);
    let placed = active_key(&document);
    assert_eq!(
        document.layer_uvs(placed).expect("readable"),
        None,
        "a layout nobody asked for was generated"
    );
}

#[test]
fn an_in_place_retopology_with_uvs_carries_them_on_the_source() {
    let Some(mut document) = meshed() else {
        return;
    };
    let source = active_key(&document);
    let outcome = document
        .retopologise(RetopoSettings {
            in_place: true,
            ..with_uvs()
        })
        .expect("the retopology runs");
    assert!(outcome.uv.carries_uvs(), "{:?}", outcome.uv);
    let uvs = document.layer_uvs(source).expect("readable");
    assert_eq!(uvs.map(|uvs| uvs.len()), Some(outcome.vertices));
}
