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
    BrushSettings, CombineSettings, Direction, GestureSample, ObjectModel, SceneModel, SculptModel,
    Shape, ToolKind,
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
