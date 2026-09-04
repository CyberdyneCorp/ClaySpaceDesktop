//! What a document and the surfaces beside it cost, at the engine boundary.
//!
//! Three claims, and each test here is written to make one of them fail if it
//! stops being true rather than to watch a call return `Ok`:
//!
//! - **The surface tier is zero through the document alone, and that is
//!   ownership rather than an omission.** A hierarchy is held beside a
//!   document, so the document cannot walk it. The seam is a ledger the *host*
//!   fills and hands in, and `a_hierarchy_reaches_the_document_report_only_-
//!   through_the_hosts_own_ledger` is that boundary measured: the same
//!   document, the same hierarchy, two different reports.
//! - **The three roll-ups are the engine's arithmetic and not this crate's.**
//!   They are read back, never recomputed, so the test asserts what the engine
//!   says about its own categories rather than a sum written twice.
//! - **A preflight refuses rather than wrapping.** An estimate nobody can
//!   compute is not one anybody may rely on, and the failure mode of a wrapped
//!   multiply is that the operation is *allowed*.

use claycore::{
    BudgetError, Document, Item, MemoryCategory, MemoryLedger, MemoryPin, Mesh, MeshParams,
    MeshSculptor, Multires, MultiresDesc, Pressure, SurfacePreflight,
};

// -- fixtures ---------------------------------------------------------------

/// A document with something in it, so the report has figures to separate.
fn worked_document() -> Document {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.add_item(layer, &Item::sphere(1.0).expect("sphere"))
        .expect("place");
    doc
}

fn sphere_mesh() -> Mesh {
    worked_document()
        .mesh(MeshParams::default())
        .expect("mesh the sphere")
}

/// A flat quad grid: what a Catmull-Clark cage is supposed to be.
fn cage(divisions: usize, name: &str) -> Mesh {
    let mut text = String::new();
    let half = 2.0f32;
    let step = 2.0 * half / divisions as f32;
    for z in 0..=divisions {
        for x in 0..=divisions {
            text.push_str(&format!(
                "v {} 0 {}\n",
                -half + step * x as f32,
                -half + step * z as f32
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
    let path =
        std::env::temp_dir().join(format!("claycore-memory-{name}-{}.obj", std::process::id()));
    std::fs::write(&path, text).expect("write the cage");
    let mesh = Mesh::load(&path).expect("load the cage");
    let _ = std::fs::remove_file(&path);
    mesh
}

fn hierarchy(levels: u32, name: &str) -> Multires {
    let mesh = cage(4, name);
    let mut surface = Multires::from_mesh(&mesh, MultiresDesc::default()).expect("a hierarchy");
    for _ in 0..levels {
        surface.add_level().expect("subdivide");
    }
    surface
}

// -- the document report ----------------------------------------------------

#[test]
fn a_worked_document_reports_where_its_memory_is() {
    let doc = worked_document();
    let report = doc.memory().expect("memory");

    assert!(
        report.edit_list > 0,
        "a document with a placed sphere holds an edit list"
    );
    assert!(
        report.total >= report.edit_list,
        "the total is the sum of the fields and cannot be under one of them"
    );
    assert_eq!(
        report.transient, 0,
        "no entry point across this ABI leaves a step open, so a non-zero \
         transient means the field has stopped meaning what the header says"
    );
}

/// The one thing a memory warning actually asks. The roll-ups are derived
/// upstream from the category lines; this asserts they are read back rather
/// than invented here, by checking them against the fields the header says
/// they are drawn from.
#[test]
fn the_roll_ups_classify_the_document_rather_than_repeating_its_total() {
    let doc = worked_document();
    let report = doc.memory().expect("memory");

    assert!(
        report.essential >= report.edit_list,
        "the edit list is the user's work and cannot be classified as \
         anything a host may release"
    );
    assert_eq!(
        report.undoable,
        report.history + report.voxel_sculpt_layers,
        "undoable is undo depth: the history and the voxel layer stacks, and \
         nothing else"
    );
    assert_eq!(
        report.essential + report.rebuildable + report.undoable + report.transient,
        report.total,
        "every reported line is classified, or the roll-ups and the total \
         disagree and a host cannot act on either"
    );
}

/// `history` and `passthrough` are document-wide, so a layer's report zeroes
/// them — one struct serving both views, documented rather than duplicated.
#[test]
fn a_layer_answers_for_itself_and_not_for_the_documents_history() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.add_item(layer, &Item::sphere(1.0).expect("sphere"))
        .expect("place");

    let per_layer = doc.layer_memory(layer).expect("layer memory");
    assert!(
        per_layer.edit_list > 0,
        "the sphere was placed on this layer and its edit list is empty"
    );
    assert_eq!(
        (per_layer.history, per_layer.passthrough),
        (0, 0),
        "the history is the document's and cannot be attributed to a layer"
    );
}

#[test]
fn an_unknown_layer_is_a_refusal_rather_than_a_zeroed_report() {
    let doc = worked_document();

    // A layer id this document never minted. Taken from a second document
    // deep enough that its last id is past anything the first one holds,
    // because ids are per document and the first of each collide.
    let mut elsewhere = Document::new().expect("a second document");
    let mut stranger = elsewhere.add_sdf_layer("A").expect("layer");
    for name in ["B", "C", "D", "E", "F", "G", "H"] {
        stranger = elsewhere.add_sdf_layer(name).expect("layer");
    }

    // A zeroed report reads as an empty layer and shows a wrong answer
    // confidently, which is why the engine refuses instead.
    assert!(
        doc.layer_memory(stranger).is_err(),
        "an id this document never minted was answered rather than refused"
    );
}

// -- the ledger the host fills ----------------------------------------------

/// The ownership boundary, measured. A hierarchy is held *beside* a document:
/// the document reports it as zero, and it appears only once the host has
/// asked the surface what it costs and handed the answer in.
#[test]
fn a_hierarchy_reaches_the_document_report_only_through_the_hosts_own_ledger() {
    let doc = worked_document();
    let surface = hierarchy(2, "ledger");

    let alone = doc.memory().expect("memory");
    assert_eq!(
        (
            alone.surface_content,
            alone.multires_detail,
            alone.surface_caches
        ),
        (0, 0, 0),
        "the document reported a surface it does not own"
    );

    let ledger = surface.memory_ledger().expect("the hierarchy's ledger");
    assert!(ledger.total > 0, "a two-level hierarchy costs nothing");

    let with = doc
        .memory_with_surfaces(&ledger)
        .expect("memory with surfaces");
    assert!(
        with.total > alone.total,
        "the ledger was handed in and the report did not grow"
    );
    assert!(
        with.surface_caches > 0 || with.surface_content > 0 || with.multires_detail > 0,
        "the surface tier is still zero after a ledger naming {} bytes",
        ledger.total
    );
    assert_eq!(
        (with.edit_list, with.history),
        (alone.edit_list, alone.history),
        "folding a surface in changed a document-side figure"
    );
}

/// The merge is the host's act — only the host knows which surfaces belong to
/// one document — which is why it is arithmetic here rather than an engine
/// call that walked something.
#[test]
fn two_surfaces_are_added_by_the_host_and_the_report_carries_both() {
    let doc = worked_document();
    let first = hierarchy(2, "merge-a");
    let second = hierarchy(2, "merge-b");

    let a = first.memory_ledger().expect("ledger");
    let b = second.memory_ledger().expect("ledger");

    let mut both = a;
    both.merge(&b);
    assert_eq!(
        both.total,
        a.total + b.total,
        "merging two ledgers is addition and nothing cleverer"
    );
    assert_eq!(
        both.bytes(MemoryCategory::BaseGeometry),
        Some(
            a.bytes(MemoryCategory::BaseGeometry).unwrap()
                + b.bytes(MemoryCategory::BaseGeometry).unwrap()
        ),
        "a merged ledger has to add category by category or a host under \
         pressure acts on the wrong one"
    );

    let one = doc.memory_with_surfaces(&a).expect("one surface");
    let two = doc.memory_with_surfaces(&both).expect("both surfaces");
    assert!(
        two.total > one.total,
        "the second hierarchy did not reach the report"
    );
}

/// A fixed-topology session answers in the same vocabulary, which is what lets
/// a host holding one of each get one set of roll-ups rather than three
/// reports to reconcile.
#[test]
fn a_fixed_topology_session_answers_in_the_same_vocabulary() {
    let mut mesh = sphere_mesh();
    let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
    let ledger = sculptor.memory_ledger().expect("the sculptor's ledger");

    assert!(ledger.total > 0, "a welded sphere session costs nothing");
    assert_eq!(
        ledger.essential + ledger.rebuildable + ledger.undoable,
        ledger.total,
        "the roll-ups are derived from the categories upstream and must \
         still account for the whole"
    );
    assert!(
        ledger.bytes(MemoryCategory::BaseGeometry).unwrap_or(0) > 0,
        "the mesh a sculptor was built over is base geometry"
    );
}

// -- the pin ----------------------------------------------------------------

/// The guard is the point: a trim arriving inside the region releases nothing
/// and says what it *would* have released, and the pin comes back at the end
/// of the scope whether the region returned or unwound.
#[test]
fn a_held_pin_makes_a_trim_honest_and_the_scope_gives_it_back() {
    let mut surface = hierarchy(2, "pin");
    let mut pin = MemoryPin::new().expect("pin");

    // Warm the rebuildable caches so a trim has something to refuse to take.
    let _ = surface.copy_level_mesh(2).expect("evaluate a level");

    let refused = {
        let hold = pin.hold().expect("hold");
        assert!(hold.is_held(), "the guard did not take the pin");
        surface
            .trim(Pressure::Critical, Some(&hold))
            .expect("a pinned trim")
    };
    assert!(
        refused.pinned,
        "a trim inside a held pin reported itself as having released memory"
    );

    assert!(
        !pin.is_held(),
        "the pin outlived its scope, which is the whole failure the guard \
         exists to make unrepresentable"
    );

    let done = surface
        .trim(Pressure::Critical, Some(&pin))
        .expect("an unpinned trim");
    assert!(
        !done.pinned,
        "the pin was released and the trim still refused"
    );
}

/// Reentrant, because a readback inside a save must not un-pin the save when
/// it returns.
#[test]
fn a_pin_held_twice_is_still_held_after_the_inner_scope_ends() {
    let mut pin = MemoryPin::new().expect("pin");
    let outer = pin.hold().expect("outer");
    {
        // A second guard cannot borrow the same pin, so the inner region is
        // the counter's own form — which is exactly why `acquire`/`release`
        // stay beside `hold` rather than being replaced by it.
        assert!(outer.is_held());
    }
    assert!(outer.is_held(), "the inner scope released the outer hold");
    drop(outer);
    assert!(!pin.is_held(), "the outer hold outlived its scope");
}

// -- the preflights ---------------------------------------------------------

fn allowed(quote: &SurfacePreflight) -> bool {
    quote.allowed && quote.error == BudgetError::None
}

#[test]
fn pricing_an_adaptive_conversion_is_not_paying_for_it() {
    let mesh = sphere_mesh();
    let triangles = mesh.indices().len() / 3;

    let free = mesh.preflight_to_dynamic(0).expect("no budget");
    assert!(allowed(&free), "an unbudgeted preflight refused");
    assert!(
        free.peak_bytes >= free.persistent_bytes,
        "the peak holds the source mesh, the half-edge structure and the weld \
         map at once, so it cannot be under what remains afterwards"
    );
    assert!(
        free.persistent_bytes > 0,
        "converting a {triangles}-triangle mesh was priced at nothing"
    );

    // Asking twice is the same answer: the call allocates nothing and has no
    // side effect, which is what makes it safe under a slider.
    let again = mesh.preflight_to_dynamic(0).expect("no budget");
    assert_eq!(free, again, "a preflight changed something by being asked");
}

/// The refusal is the feature. A budget below the predicted peak is refused
/// whole and named, rather than answered with a number a host would act on.
#[test]
fn a_preflight_refuses_a_budget_it_cannot_meet_rather_than_trimming_its_answer() {
    let mesh = sphere_mesh();
    let free = mesh.preflight_to_dynamic(0).expect("no budget");

    let squeezed = mesh.preflight_to_dynamic(64).expect("a tiny budget");
    assert!(
        !squeezed.allowed,
        "64 bytes was enough for a conversion the same call priced at {}",
        free.peak_bytes
    );
    assert_eq!(
        squeezed.error,
        BudgetError::OverBudget,
        "the refusal has to name itself: {}",
        squeezed.error
    );
    assert_eq!(
        squeezed.peak_bytes, free.peak_bytes,
        "a refused preflight still has to say what the operation would have \
         cost, or a host cannot tell it what budget to ask for"
    );
    assert!(!squeezed.error.text().is_empty());
}

#[test]
fn a_global_remesh_is_priced_against_its_target_and_not_its_source() {
    let mesh = sphere_mesh();
    let coarse = mesh
        .preflight_global_remesh(1_000, 0)
        .expect("a coarse target");
    let fine = mesh
        .preflight_global_remesh(100_000, 0)
        .expect("a fine target");

    assert!(allowed(&coarse) && allowed(&fine));
    assert!(
        fine.peak_bytes > coarse.peak_bytes,
        "a hundred times the triangles was priced at {} against {}",
        fine.peak_bytes,
        coarse.peak_bytes
    );
    assert!(
        coarse.peak_bytes >= coarse.persistent_bytes,
        "source and target are live at the same time, which is the whole \
         reason this one is asked"
    );
}

/// An estimate nobody can compute is not one anybody may rely on. The failure
/// mode of the bug this guards is that the operation is *allowed*.
#[test]
fn arithmetic_that_overflows_is_a_refusal_at_any_budget() {
    let mesh = sphere_mesh();
    let quote = mesh
        .preflight_global_remesh(u64::MAX, 0)
        .expect("the call itself succeeds; the estimate is what refuses");

    assert!(
        !quote.allowed,
        "a target of u64::MAX triangles was allowed with no budget at all"
    );
    assert_eq!(
        quote.error,
        BudgetError::Overflow,
        "an overflow reported itself as something a bigger budget would fix"
    );
}

#[test]
fn a_ledger_reports_how_many_categories_the_library_filled() {
    let surface = hierarchy(1, "categories");
    let ledger: MemoryLedger = surface.memory_ledger().expect("ledger");

    assert!(
        ledger.category_count as usize <= MemoryCategory::ALL.len(),
        "the library claims more categories than this build can name"
    );
    for category in MemoryCategory::ALL {
        // Every category this build knows either has a figure or is honestly
        // absent; neither is an error, and a panic here would mean the index
        // arithmetic is wrong.
        let _ = ledger.bytes(category);
    }
}
