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

// -- what the written mesh turned out to be ---------------------------------

/// The export path asks the engine whether what it wrote is sound.
///
/// The regression this exists for. `export_mesh` called `mesh_combined` and
/// then `save`, and nothing between them looked at the result — while
/// `Mesh::validate` sat bound in `crates/claycore` with no caller on this path
/// at all. So an export could be non-manifold and nothing said so, which is
/// the class of defect that breaks a slicer or a boolean engine while a
/// viewport shows nothing wrong.
///
/// Asserted as a *clean* export saying nothing, because that is the half that
/// can be pinned on every engine: a warning that fires on a sound mesh would
/// be noise on every export a sculptor ever makes, and noise is how a real
/// warning gets ignored. The other half — that an unsound mesh does speak — is
/// `an_unsound_mesh_is_reported` below and the unit tests on
/// `ExportWarning::for_written_mesh`.
#[test]
fn a_sound_export_says_nothing_about_itself() {
    let mut document = document();
    let path = scratch("sound.obj");
    let findings = document
        .export_mesh(&path, ExportSettings::default())
        .expect("export");
    let _ = std::fs::remove_file(&path);

    assert!(
        findings.is_empty(),
        "the default export of a plain sphere reported {findings:?}; a warning \
         on a sound mesh appears on every export and teaches a sculptor to \
         ignore the panel"
    );
}

/// The export panel's own default decimation writes a non-manifold mesh.
///
/// **The case this whole change exists for.** Tick "decimate" in the export
/// panel and the ratio starts at 0.5; leave the resolution at its default
/// 0.02; keep the Watertight mesher, which is the one that carries no caveat
/// and therefore promises a 2-manifold. Marching tetrahedra produces one by
/// construction — and then decimation takes it apart.
///
/// ClayCore #567 made decimation check its own result and retry with a
/// different choice of collapses. Where nothing clean fits at the requested
/// size it returns the requested size and reports the pinch rather than
/// repairing it, because at an aggressive ratio merging sheets is what the
/// ratio *means*. This is that case, on the most ordinary settings the
/// application offers.
///
/// So this test is not a guard against a regression that might happen. It
/// pins a defect that is live at the pinned engine, and pins that the
/// application now says so instead of writing the file in silence.
#[test]
fn the_default_decimation_is_reported_as_not_manifold() {
    let mut document = document();
    let path = scratch("default-decimation.obj");
    let findings = document
        .export_mesh(
            &path,
            ExportSettings {
                decimate_to: Some(0.5),
                ..Default::default()
            },
        )
        .expect("export");
    let _ = std::fs::remove_file(&path);

    assert!(
        findings.iter().any(|w| w.message.contains("manifold")),
        "the export panel's default decimation reported {findings:?}; either \
         the validator is no longer called on the written mesh, or ClayCore \
         has started finding a clean collapse set at this ratio — the second \
         is worth knowing and worth deleting this test for"
    );
}

/// A gentler decimation is sound, so the warning is not simply "you decimated".
///
/// The companion to the test above, and the one that keeps it honest. If the
/// finding fired on every decimated export it would be a restatement of the
/// setting rather than an observation about the file, and a sculptor would
/// learn to ignore it. At 0.25 the same sphere comes back clean.
#[test]
fn a_gentler_decimation_is_still_sound() {
    let mut document = document();
    let path = scratch("gentle-decimation.obj");
    let findings = document
        .export_mesh(
            &path,
            ExportSettings {
                decimate_to: Some(0.25),
                ..Default::default()
            },
        )
        .expect("export");
    let _ = std::fs::remove_file(&path);

    assert!(
        findings.is_empty(),
        "a watertight sphere decimated to 25% came back as {findings:?}"
    );
}
