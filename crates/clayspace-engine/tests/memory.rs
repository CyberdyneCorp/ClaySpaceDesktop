//! Where a document's memory is, and which part of it may be let go.
//!
//! A single total is the wrong answer to the question a memory warning asks.
//! Under pressure a sculptor does not need to know how big the document is,
//! they need to know **which part** — their own work, which is never released;
//! caches, which reconstruct identically and cost only a stall; or undo depth,
//! which is this application's own policy and nobody else's.
//!
//! The second thing these hold is the one that is silent when it is wrong. A
//! mesh-sculpting session is an owning handle this application keeps *beside*
//! its document rather than inside it, so the engine's plain roll-up reports
//! the whole surface tier as zero — correctly, because it cannot walk what it
//! does not own. A host that stops there publishes a figure that omits the
//! largest thing on the machine. `ClayDocument::memory` asks each session what
//! it costs and hands the ledger back to the engine, and
//! `a_mesh_session_reaches_the_report_and_the_plain_roll_up_misses_it` is what
//! says so by measuring both.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, CombineSettings, ConversionSettings, Direction, ExchangeModel, GestureSample,
    ImportSettings, MultiresLevelOp, ObjectModel, Representation, SceneModel, SculptModel, Shape,
    ToolKind,
};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// The starting form crossed into a mesh and then sculpted, which is what
/// builds a sculpting session for the layer — the surface this application
/// actually holds beside its document.
fn worked_mesh() -> ClayDocument {
    let mut document = sphere();
    document
        .convert_layer(Direction::SdfToMesh, 0.02, 0)
        .expect("into a mesh");
    dab(&mut document, [0.9, 0.0, 0.0]);
    assert_eq!(
        document.mesh_sculptors_held(),
        1,
        "the fixture never built a sculpting session, so there is no surface \
         to report"
    );
    document
}

/// A short stroke on the active layer — enough to make the document build a
/// sculpting session for it, which is the surface being reported.
fn dab(document: &mut ClayDocument, at: [f32; 3]) {
    let brush = BrushSettings {
        size: 0.25,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    let samples: Vec<GestureSample> = (0..6)
        .map(|step| GestureSample {
            position: [at[0], at[1] + step as f32 * 0.05 - 0.1, at[2]],
            pressure: 1.0,
            time: step as f32 * 0.016,
        })
        .collect();
    document
        .apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])
        .expect("a dab on the mesh");
}

/// The three roll-ups are not a second opinion about the total; they are a
/// partition of it. If they do not account for the whole, a host cannot act on
/// either the parts or the sum.
#[test]
fn the_three_roll_ups_account_for_the_whole_the_engine_reports() {
    for document in [sphere(), worked_mesh()] {
        let report = document.memory().expect("the document's memory");
        assert!(report.total > 0, "a worked document costs nothing");
        assert_eq!(
            report.essential + report.rebuildable + report.undoable + report.transient,
            report.total,
            "the roll-ups and the total disagree: essential {} + rebuildable \
             {} + undoable {} + transient {} against a total of {}",
            report.essential,
            report.rebuildable,
            report.undoable,
            report.transient,
            report.total
        );
        assert!(
            report.essential >= report.edit_list,
            "the edit list is the user's work and cannot be classified as \
             anything a host may release"
        );
    }
}

/// The headline. Same document, same instant, two reports — and the plain one
/// is missing the session.
#[test]
fn a_mesh_session_reaches_the_report_and_the_plain_roll_up_misses_it() {
    let document = worked_mesh();

    // What the engine can answer on its own. The surface tier is zero here
    // because a sculpting session is held beside the document, not in it.
    let plain = document
        .document()
        .memory()
        .expect("the engine's own roll-up");
    assert_eq!(
        (
            plain.surface_content,
            plain.sculpt_layers,
            plain.surface_caches
        ),
        (0, 0, 0),
        "the engine reported a surface it does not own"
    );

    let (surfaces, ledger) = document.surface_ledger().expect("the host's own ledger");
    assert_eq!(surfaces, 1, "one mesh subtool, one sculpting session");
    assert!(
        ledger.total > 0,
        "a welded session over a marched sphere costs nothing"
    );

    let whole = document.memory().expect("the document's memory");
    assert!(
        whole.total > plain.total,
        "folding {} bytes of session in did not grow the report: {} against {}",
        ledger.total,
        whole.total,
        plain.total
    );
    assert_eq!(
        whole.total - plain.total,
        ledger.total,
        "the report grew by something other than what the ledger named"
    );
    assert!(
        whole.surface_content > 0 || whole.surface_caches > 0,
        "the surface tier is still zero after a ledger naming {} bytes",
        ledger.total
    );
    // And nothing document-side moved, which is what says the ledger was added
    // rather than allowed to reinterpret the document.
    assert_eq!(
        (whole.edit_list, whole.history, whole.mesh_layers),
        (plain.edit_list, plain.history, plain.mesh_layers)
    );
}

/// A document holding no surface is not a different code path: it asks, gets
/// nothing, and reports what the engine reports.
#[test]
fn a_document_holding_no_surface_reports_what_the_engine_alone_would() {
    let document = sphere();
    assert_eq!(document.mesh_sculptors_held(), 0);

    let (surfaces, ledger) = document.surface_ledger().expect("ledger");
    assert_eq!((surfaces, ledger.total), (0, 0));
    assert_eq!(
        document.memory().expect("with surfaces").total,
        document.document().memory().expect("plain").total,
        "an empty ledger changed the answer, so the fold is not neutral"
    );
}

/// What the diagnostics report carries, and the row that makes a zero
/// readable.
///
/// A surface tier of zero is the right answer on a document holding no
/// surface, and it is also exactly what a host that never filled the ledger
/// would print. The count is what tells the two apart, so it is reported even
/// when it is zero.
#[test]
fn the_diagnostics_say_how_many_surfaces_were_asked_as_well_as_what_they_cost() {
    let none = sphere()
        .memory_diagnostics()
        .expect("a document can always be asked what it costs");
    assert_eq!((none.surfaces, none.surface_bytes), (0, 0));
    assert!(none.essential > 0, "the starting form is somebody's work");

    let document = worked_mesh();
    let held = document.memory_diagnostics().expect("diagnostics");
    assert_eq!(held.surfaces, 1);
    assert!(held.surface_bytes > 0);

    // Read from the engine's report rather than recomputed, which is the whole
    // discipline: a second derivation on this side is a second thing that can
    // be right about the build it was written against and wrong about the next.
    let report = document.memory().expect("memory");
    assert_eq!(
        (held.essential, held.rebuildable, held.undoable, held.total),
        (
            report.essential,
            report.rebuildable,
            report.undoable,
            report.total
        )
    );
}

/// A second mesh subtool is a second session, and both are the host's to fold
/// in — the engine merges no ledgers, because only this side knows which
/// surfaces belong to which document.
#[test]
fn a_second_mesh_subtool_is_a_second_session_and_both_are_counted() {
    let mut document = worked_mesh();
    let (first_count, first) = document.surface_ledger().expect("ledger");
    assert_eq!(first_count, 1);

    document
        .add_layer("Segundo", clayspace_model::Representation::Sdf)
        .expect("a second field subtool");
    document
        .place_object(
            Shape::Sphere,
            &[0.5],
            [2.0, 0.0, 0.0],
            CombineSettings::default(),
        )
        .expect("something in it to cross");
    document
        .convert_layer(Direction::SdfToMesh, 0.04, 0)
        .expect("into a second mesh");
    dab(&mut document, [2.4, 0.0, 0.0]);

    let (surfaces, both) = document.surface_ledger().expect("ledger");
    assert_eq!(
        surfaces, 2,
        "the second sculpting session was not asked for its ledger"
    );
    assert!(
        both.total > first.total,
        "two sessions did not cost more than one: {} against {}",
        both.total,
        first.total
    );
    assert!(
        document.memory().expect("memory").total
            > document.document().memory().expect("plain").total,
        "neither session reached the document's report"
    );
}

/// A flat quad grid written out, which is the only route a cage has into a
/// document — and the shape Catmull-Clark wants.
fn cage_obj(path: &std::path::Path, divisions: usize) {
    let mut text = String::new();
    let step = 4.0 / divisions as f32;
    for z in 0..=divisions {
        for x in 0..=divisions {
            text.push_str(&format!(
                "v {} 0 {}\n",
                -2.0 + step * x as f32,
                -2.0 + step * z as f32
            ));
        }
    }
    let stride = divisions + 1;
    for z in 0..divisions {
        for x in 0..divisions {
            let a = z * stride + x + 1;
            text.push_str(&format!(
                "f {} {} {} {}\n",
                a,
                a + stride,
                a + stride + 1,
                a + 1
            ));
        }
    }
    std::fs::write(path, text).expect("write the cage");
}

/// A document whose only carried layer is a subdivision hierarchy.
fn with_a_hierarchy() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    let path =
        std::env::temp_dir().join(format!("clayspace-memory-cage-{}.obj", std::process::id()));
    cage_obj(&path, 8);
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import the cage");
    let _ = std::fs::remove_file(&path);
    let cage = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the cage is a mesh layer");
    document.set_active_layer(cage).expect("activate the cage");
    let settings = ConversionSettings::default();
    document
        .convert_layer_in_place(Direction::MeshToMultires, settings.cell_size, settings.blur)
        .expect("a flat quad grid is a cage");
    document
}

/// A hierarchy is a surface the host holds too, and it is counted like one.
///
/// The other kind of surface this application owns, and the one the roll-up
/// used to walk straight past: a mesh sculpting session lives in a map beside
/// the document and a hierarchy lives on its own layer, so a walk over the map
/// alone answered for a hierarchy-holding document exactly what it answered
/// for an empty one. Measured before this was repaired, an 8x8 cage reported
/// `surfaces 0` and a byte-identical total at zero levels and at six, while
/// the hierarchy beside it held 26 MB of which 15.7 MB were rebuildable — the
/// figure `release_hierarchy_caches` exists to act on, reported as zero.
///
/// So this measures the thing that was byte-identical: what the document says
/// it costs, before and after four subdivisions.
#[test]
fn a_hierarchy_reaches_the_report_and_moves_it_when_it_deepens() {
    let mut document = with_a_hierarchy();

    let (surfaces, ledger) = document.surface_ledger().expect("ledger");
    assert_eq!(
        surfaces, 1,
        "the hierarchy was never asked what it costs, so `surfaces` reads as \
         'this document holds none' when it holds one"
    );
    assert!(ledger.total > 0, "and it answered with nothing");

    let flat = document.memory_diagnostics().expect("a report");
    for _ in 0..4 {
        document
            .apply_multires_level_op(MultiresLevelOp::AddLevel)
            .expect("subdivide");
    }
    let deep = document.memory_diagnostics().expect("a report");

    assert!(
        deep.surface_bytes > flat.surface_bytes,
        "four subdivisions cost the report nothing: {} against {}",
        deep.surface_bytes,
        flat.surface_bytes
    );
    assert!(
        deep.total > flat.total,
        "and the document's own total did not move either: {} against {}",
        deep.total,
        flat.total
    );
    assert!(
        deep.rebuildable > 0,
        "a subdivided hierarchy holds level caches the engine reproduces \
         bit-identically, which is what `release_hierarchy_caches` acts on — \
         reporting none of them says there is nothing to let go"
    );
    assert!(
        deep.total > document.document().memory().expect("plain").total,
        "the hierarchy did not reach the document's report at all"
    );
}
