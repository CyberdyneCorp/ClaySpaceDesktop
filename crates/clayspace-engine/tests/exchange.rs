//! Geometry in and out, against a real engine.

use clayspace_engine::claycore;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    ExchangeModel, ExportMesher, ExportSettings, ExportWarningKind, Format, ImportAs,
    ImportSettings, SceneModel,
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

/// What the export reports agrees with what the mesh actually is.
///
/// **The wiring, asserted platform-independently** — and the second design of
/// this test, because the first one was wrong in a way only CI could show.
///
/// It originally pinned the export panel's own default as a live defect: tick
/// decimate, the slider lands on 0.5, and the watertight mesher — the one
/// carrying no caveat and therefore promising a 2-manifold — returned one that
/// was not. That reproduced here, deterministically, and ClayCore reproduced it
/// to the triangle and filed their #575.
///
/// **It does not reproduce on Linux.** Same source, same engine, same ratio;
/// the export comes back clean and the assertion fails. Which is the same
/// finding as ClayCore's, one level further out: they showed the collapse
/// sequence is chaotic under tiny input perturbations — three meshes of one
/// sphere gave 0, 31 and 58 pinched ratios of 76 — and a different architecture
/// is exactly such a perturbation. A pinch is a transient state of the
/// simplification, created by one collapse and removed by a later one, so
/// *where* the bands fall is a property of one mesh on one machine.
///
/// So no test may assert that a particular ratio is unsound. What is true
/// everywhere is the thing this application is actually responsible for: when
/// the mesh it wrote is unsound, it says so, and when it is sound, it does not.
///
/// Asserted by agreement against an independent validation of the same
/// parameters, which is what makes it a test of *our* wiring rather than of the
/// engine's decimator. It cannot pass by accident: an `export_mesh` that
/// returned `Ok(vec![])` regardless fails on the first ratio that pinches, and
/// one that cried wolf fails on the first that does not.
#[test]
fn what_the_export_reports_is_what_the_mesh_is() {
    let mut document = document();
    let mut checked = 0;
    let mut pinched = 0;

    for ratio in [0.25f32, 0.4, 0.5, 0.6, 0.75, 0.9] {
        let settings = ExportSettings {
            decimate_to: Some(ratio),
            ..Default::default()
        };

        // What the mesh IS, asked of the engine directly with the same
        // parameters the export uses.
        let truth = document
            .document()
            .mesh(claycore::MeshParams {
                voxel_size: Some(ClayDocument::VOXEL_SIZE),
                resolution: 128,
                decimate_ratio: settings.decimate_to,
                mesher: claycore::Mesher::MarchingTetrahedra,
            })
            .expect("mesh the control")
            .validation_report(0)
            .expect("validate the control");

        // What the export SAYS about the file it just wrote.
        let path = scratch(&format!("agreement-{ratio}.obj"));
        let findings = document.export_mesh(&path, settings).expect("export");
        let _ = std::fs::remove_file(&path);

        let sound = truth.manifold && truth.watertight;
        assert_eq!(
            findings.is_empty(),
            sound,
            "at ratio {ratio} the mesh is manifold={} watertight={} \
             ({} non-manifold edges, {} boundary edges) and the export reported \
             {findings:?}",
            truth.manifold,
            truth.watertight,
            truth.non_manifold_edges,
            truth.boundary_edges,
        );
        checked += 1;
        pinched += usize::from(!sound);
    }

    assert_eq!(checked, 6, "the sweep did not run");
    // Not an assertion about the engine — a note in the output, so a reader of
    // a CI log can see which side of ClayCore #575 this machine falls on.
    println!("{pinched} of {checked} ratios pinched on this platform");
}

/// An export that is open says so, on every platform.
///
/// The companion that closes the one hole in
/// `what_the_export_reports_is_what_the_mesh_is`. That test asserts agreement
/// across a sweep of decimation ratios, which is the right invariant — but on a
/// machine where *no* ratio pinches, the half that matters is vacuous: an
/// `export_mesh` returning `Ok(vec![])` regardless would agree with a mesh that
/// is sound at every ratio, and pass. That is exactly the platform CI runs on.
///
/// So this makes an unsound export on purpose and by a route that has nothing
/// to do with ClayCore #575's chaotic collapse sequence. A single triangle is a
/// mesh with three boundary edges: not watertight, on any architecture, by
/// construction rather than by luck.
///
/// It reaches the export because `mesh_combined` deliberately includes every
/// visible mesh layer — meshing the field alone would silently leave imported
/// geometry out of the file — so the concatenation of a closed sphere and an
/// open sheet is open.
#[test]
fn an_open_mesh_layer_makes_the_export_say_it_is_not_closed() {
    let mut document = document();

    // Three vertices, one triangle, three edges each carrying one face.
    let sheet = claycore::Mesh::from_triangles(
        &[[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, 1.0, 0.0]],
        &[0, 1, 2],
    )
    .expect("a single triangle is a mesh");
    let path = scratch("open-sheet.obj");
    sheet.save(&path).expect("write the sheet");
    document
        .import_mesh(
            &path,
            ImportSettings {
                becomes: ImportAs::Reference,
                ..Default::default()
            },
        )
        .expect("import it as a mesh layer");
    let _ = std::fs::remove_file(&path);

    let out = scratch("open-export.obj");
    let findings = document
        .export_mesh(&out, ExportSettings::default())
        .expect("export");
    let _ = std::fs::remove_file(&out);

    assert!(
        findings
            .iter()
            .any(|warning| warning.kind == ExportWarningKind::OpenBoundary(3)),
        "a document carrying an open mesh layer exported without saying the \
         result is not closed; got {findings:?}. Either the validator is no \
         longer called on the written mesh, or mesh_combined has stopped \
         including mesh layers — and the second would silently drop imported \
         geometry from every export"
    );
}
