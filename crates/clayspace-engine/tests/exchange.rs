//! Geometry in and out, against a real engine.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    ExchangeModel, ExportMesher, ExportSettings, Format, ImportAs, ImportSettings, SceneModel,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

fn scratch(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("clayspace-exchange-{name}"));
    let _ = std::fs::remove_file(&path);
    path
}

/// Exports the starting form, which is the only geometry always to hand.
///
/// `who` names the caller. Tests run in parallel and a shared filename had
/// them deleting each other's files — which showed up as a torn OBJ and an
/// export that had apparently written nothing.
fn exported(
    document: &mut ClayDocument,
    who: &str,
    extension: &str,
    settings: ExportSettings,
) -> Option<std::path::PathBuf> {
    let path = scratch(&format!("{who}.{extension}"));
    document.export_mesh(&path, settings).ok()?;
    Some(path)
}

#[test]
fn a_document_exports_to_a_file_that_can_be_read_back() {
    let mut document = document();
    let path =
        exported(&mut document, "roundtrip", "obj", ExportSettings::default()).expect("export");
    assert!(path.is_file(), "nothing was written");
    assert!(
        std::fs::metadata(&path).expect("metadata").len() > 1024,
        "the file is too small to hold a sphere"
    );

    // Read back through the importer, which is the only check that says the
    // file is a mesh rather than bytes.
    let mut reopened = document;
    reopened
        .import_mesh(&path, ImportSettings::default())
        .expect("re-import");
    assert!(
        reopened.has_mesh_layers(),
        "the round trip produced no mesh layer"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn every_writable_format_is_actually_written() {
    let mut document = document();
    for format in Format::ALL {
        let path = exported(
            &mut document,
            "formats",
            format.extension(),
            ExportSettings::default(),
        );
        let path = path.unwrap_or_else(|| panic!("{format:?} did not export"));
        assert!(path.is_file(), "{format:?} wrote nothing");
        let _ = std::fs::remove_file(&path);
    }
}

#[test]
fn glb_is_written_and_refused_on_the_way_in() {
    // The engine's asymmetry, met at the boundary rather than inside a
    // decoder: `clay_mesh_save` takes .glb and `clay_mesh_load` does not.
    let mut document = document();
    let path = exported(&mut document, "glb", "glb", ExportSettings::default()).expect("export");
    let refusal = document
        .import_mesh(&path, ImportSettings::default())
        .expect_err("glb imported; drop the special case");
    assert!(
        format!("{refusal}").contains("GLB"),
        "the refusal does not name the format: {refusal}"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_unknown_extension_is_refused_by_name() {
    let mut document = document();
    let path = scratch("out.blend");
    assert!(document
        .export_mesh(&path, ExportSettings::default())
        .is_err());
    assert!(document
        .import_mesh(&path, ImportSettings::default())
        .is_err());
}

#[test]
fn a_reference_import_is_carried_and_a_clay_import_is_sculptable() {
    let mut document = document();
    let path =
        exported(&mut document, "imports", "obj", ExportSettings::default()).expect("export");
    let layers_before = document.scene().layers.len();

    document
        .import_mesh(
            &path,
            ImportSettings {
                becomes: ImportAs::Reference,
                ..Default::default()
            },
        )
        .expect("reference import");
    assert_eq!(document.scene().layers.len(), layers_before + 1);
    assert!(document.has_mesh_layers());

    document
        .import_mesh(
            &path,
            ImportSettings {
                becomes: ImportAs::Clay,
                ..Default::default()
            },
        )
        .expect("clay import");
    let scene = document.scene();
    assert_eq!(scene.layers.len(), layers_before + 2);
    // Clay is an SDF layer: the tools have to be able to reach it.
    assert!(
        scene
            .layers
            .iter()
            .filter(|layer| layer.representation == clayspace_model::Representation::Sdf)
            .count()
            > 1,
        "the clay import did not produce a sculptable layer"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_budget_smaller_than_the_file_refuses_before_allocating() {
    // The guardrail the engine documents: checked against the file's declared
    // counts, so a hostile file claiming a billion triangles never allocates.
    let mut document = document();
    let path = exported(&mut document, "budget", "obj", ExportSettings::default()).expect("export");

    let refusal = document.import_mesh(
        &path,
        ImportSettings {
            max_vertices: 8,
            max_triangles: 8,
            ..Default::default()
        },
    );
    assert!(refusal.is_err(), "an eight-vertex budget accepted a sphere");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_coarser_export_is_a_smaller_file() {
    // Resolution reaches the engine rather than being carried and ignored.
    let mut document = document();
    let fine = exported(
        &mut document,
        "resolution",
        "obj",
        ExportSettings {
            resolution: 0.02,
            ..Default::default()
        },
    )
    .expect("fine");
    let fine_size = std::fs::metadata(&fine).expect("metadata").len();
    let _ = std::fs::remove_file(&fine);

    let coarse = exported(
        &mut document,
        "resolution",
        "obj",
        ExportSettings {
            resolution: 0.12,
            ..Default::default()
        },
    )
    .expect("coarse");
    let coarse_size = std::fs::metadata(&coarse).expect("metadata").len();
    let _ = std::fs::remove_file(&coarse);

    assert!(
        coarse_size < fine_size,
        "resolution did not reach the mesher: {coarse_size} against {fine_size}"
    );
}

#[test]
fn decimation_reaches_the_mesher() {
    let mut document = document();
    let whole =
        exported(&mut document, "decimate", "obj", ExportSettings::default()).expect("whole");
    let whole_size = std::fs::metadata(&whole).expect("metadata").len();
    let _ = std::fs::remove_file(&whole);

    let cut = exported(
        &mut document,
        "decimate",
        "obj",
        ExportSettings {
            decimate_to: Some(0.25),
            ..Default::default()
        },
    )
    .expect("decimated");
    let cut_size = std::fs::metadata(&cut).expect("metadata").len();
    let _ = std::fs::remove_file(&cut);

    assert!(
        cut_size < whole_size,
        "decimation did not reach the mesher: {cut_size} against {whole_size}"
    );
}

#[test]
fn every_mesher_produces_a_file() {
    let mut document = document();
    for mesher in ExportMesher::ALL {
        let path = exported(
            &mut document,
            "meshers",
            "obj",
            ExportSettings {
                mesher,
                ..Default::default()
            },
        );
        let path = path.unwrap_or_else(|| panic!("{mesher:?} did not export"));
        assert!(
            std::fs::metadata(&path).expect("metadata").len() > 512,
            "{mesher:?} wrote an empty file"
        );
        let _ = std::fs::remove_file(&path);
    }
}

#[test]
fn a_reference_layer_reaches_the_exported_file() {
    // The whole reason export goes through `mesh_combined`: meshing the field
    // alone would silently leave every imported reference out of the file.
    let mut document = document();
    let source =
        exported(&mut document, "combined", "obj", ExportSettings::default()).expect("source");
    let field_only = std::fs::metadata(&source).expect("metadata").len();

    document
        .import_mesh(
            &source,
            ImportSettings {
                becomes: ImportAs::Reference,
                ..Default::default()
            },
        )
        .expect("import");

    let combined =
        exported(&mut document, "combined", "obj", ExportSettings::default()).expect("combined");
    let combined_size = std::fs::metadata(&combined).expect("metadata").len();

    assert!(
        combined_size > field_only,
        "the reference layer was left out of the export: {combined_size} against {field_only}"
    );
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&combined);
}
