//! The work a document owes itself between two interactions.
//!
//! Three claims, and the value of each of them is that it is a mechanism
//! rather than a habit, so each is measured here rather than read:
//!
//! - **The gate.** A drain is refused while a gesture is open, because there
//!   is no queue to drain one from — a `StrokeScope` holds it — and it is
//!   drainable again the moment the gesture ends, including when the gesture
//!   ends by being reopened over rather than finished.
//! - **The budget.** A queue holding more than the moment can afford leaves
//!   the rest behind rather than overrunning, and what it left is still there,
//!   still counting how often it has been asked for.
//! - **The pin.** Taken for exactly as long as a pointer is down, and given
//!   back on every way out — the pointer coming up, a press arriving over an
//!   unfinished drag, a cage applied, a cage abandoned, and a cage put back
//!   exactly where it started.
//!
//! None of what is queued here is correctness. Every one of these tests can be
//! made to pass by a document that services nothing, so each says what it
//! measured and not merely that nothing broke.

use std::time::Duration;

use clayspace_engine::claycore::MaintenanceKind;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ExchangeModel, ExportSettings, GestureSample, ImportSettings, LatticeModel,
    Representation, SceneModel, SculptModel, ToolKind,
};

/// Long enough that nothing in these fixtures runs out of it, and stated as a
/// figure rather than as the document's own budget so that a change to that
/// constant cannot quietly change what these tests are asserting.
const GENEROUS: Duration = Duration::from_secs(30);

fn scratch(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("clayspace-maintenance-{name}"));
    let _ = std::fs::remove_file(&path);
    path
}

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// A document whose active layer is an imported mesh, which is the one
/// representation this application can produce maintenance for.
fn with_imported_mesh(who: &str) -> (ClayDocument, std::path::PathBuf) {
    let mut document = sphere();
    let path = scratch(&format!("{who}.obj"));
    document
        .export_mesh(&path, ExportSettings::default())
        .expect("export a mesh");
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import it back");
    let key = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the imported mesh is a layer");
    document.set_active_layer(key).expect("activate the mesh");
    (document, path)
}

fn drag(document: &mut ClayDocument, tool: ToolKind, at: [f32; 3]) {
    document.begin_gesture();
    for step in 0..6 {
        let along = [at[0] + step as f32 * 0.05, at[1], at[2]];
        let _ = document.apply_stroke(
            tool,
            BrushSettings::default(),
            &[
                GestureSample {
                    position: at,
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: along,
                    pressure: 1.0,
                    time: 1.0,
                },
            ],
            [false; 3],
        );
    }
}

fn queued_of(document: &ClayDocument, kind: MaintenanceKind) -> Option<u32> {
    document
        .maintenance_queued()
        .iter()
        .find(|item| item.kind == kind)
        .map(|item| item.requests)
}

// -- the gate ---------------------------------------------------------------

/// The whole reason the queue holds a stroke flag: a drain wired to the wrong
/// moment finds out by nothing happening rather than by a stutter the artist
/// will blame on the brush.
#[test]
fn nothing_is_drained_while_a_gesture_is_open_and_everything_after_it() {
    let (mut document, path) = with_imported_mesh("gate");

    document.begin_gesture();
    document.request_maintenance(MaintenanceKind::IndexRebuild, 7, 0);
    assert_eq!(
        document.maintenance_queued().len(),
        1,
        "a request made during a gesture did not reach the queue at all"
    );
    assert_eq!(
        document.drain_maintenance(GENEROUS),
        0,
        "the queue was drained with a finger on the glass"
    );
    assert_eq!(
        document.maintenance_queued().len(),
        1,
        "the gate let an item go even though it serviced none"
    );

    document.end_gesture();
    assert_eq!(
        document.maintenance_queued().len(),
        0,
        "the pointer came up and the queue was still holding its work"
    );
    let _ = std::fs::remove_file(&path);
}

/// A drag asks for the same rebuild on every segment, which is exactly what
/// the queue's fold is for — and it is what makes the request safe to make
/// from inside a stroke.
#[test]
fn a_drag_asks_on_every_segment_and_leaves_one_entry() {
    let (mut document, path) = with_imported_mesh("fold");

    drag(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0]);
    let asked = queued_of(&document, MaintenanceKind::IndexRebuild)
        .expect("a mesh drag asked for nothing at all");
    assert!(
        asked >= 6,
        "six segments left {asked} askings recorded, so the segments are not \
         reaching the queue"
    );
    assert_eq!(
        document.maintenance_queued().len(),
        1,
        "a drag left one entry per segment rather than one entry"
    );

    document.end_gesture();
    assert!(
        document.maintenance_queued().is_empty(),
        "the drag's request outlived the moment that was supposed to service it"
    );
    let _ = std::fs::remove_file(&path);
}

/// A press that arrives while a drag is still open — a lost pointer release,
/// a window-manager grab, a focus change — opens a second gesture over the
/// first. The gate is a count of nothing: it must not take two closes to
/// reopen, or one lost release would stop this document doing maintenance for
/// the life of the process.
#[test]
fn a_gesture_reopened_over_an_open_one_still_ends_drainable() {
    let (mut document, path) = with_imported_mesh("reopened");

    document.begin_gesture();
    document.request_maintenance(MaintenanceKind::IndexRebuild, 1, 0);
    document.begin_gesture();
    document.end_gesture();

    assert!(
        document.maintenance_queued().is_empty(),
        "the gate was left shut, so nothing will ever be serviced again"
    );
    let _ = std::fs::remove_file(&path);
}

// -- the budget -------------------------------------------------------------

/// A moment that cannot afford everything does what it can and leaves the rest
/// where it was — which is the difference between a budget and a drop.
#[test]
fn a_drain_leaves_behind_what_the_budget_will_not_cover() {
    let mut document = sphere();

    // Two the moment can afford, then one nothing could, then one it could.
    // The costly one is what the loop must stop at: `take_next` hands out the
    // head of the queue, so stepping past it would ask for the same item
    // forever.
    document.request_maintenance(MaintenanceKind::ChunkCompaction, 1, 0);
    document.request_maintenance(MaintenanceKind::DetailPromotion, 2, 0);
    document.request_maintenance(MaintenanceKind::SlotPoolCompaction, 3, 60_000);
    document.request_maintenance(MaintenanceKind::DetailPromotion, 4, 0);
    assert_eq!(document.maintenance_queued().len(), 4);

    let serviced = document.drain_maintenance(Duration::from_millis(5));
    assert_eq!(
        serviced, 2,
        "a five-millisecond moment serviced {serviced} items, one of which \
         said it wanted sixty"
    );
    let left: Vec<_> = document
        .maintenance_queued()
        .iter()
        .map(|item| (item.kind, item.target))
        .collect();
    assert_eq!(
        left,
        vec![
            (MaintenanceKind::SlotPoolCompaction, 3),
            (MaintenanceKind::DetailPromotion, 4),
        ],
        "the drain did not stop where the budget ran out"
    );

    // Declining is not dropping: the same work is still there, and a moment
    // that can afford it does it.
    assert_eq!(document.drain_maintenance(Duration::from_secs(1)), 2);
    assert!(document.maintenance_queued().is_empty());
}

/// A budget of nothing is a budget, not a special case.
#[test]
fn a_moment_with_no_room_services_nothing_and_loses_nothing() {
    let mut document = sphere();
    document.request_maintenance(MaintenanceKind::ChunkCompaction, 1, 0);

    assert_eq!(document.drain_maintenance(Duration::ZERO), 0);
    assert_eq!(
        document.maintenance_queued().len(),
        1,
        "an item was taken by a drain that had no time to do it"
    );
    // And the asking is still counted, which is how a host sees what it is
    // starving.
    document.request_maintenance(MaintenanceKind::ChunkCompaction, 1, 0);
    assert_eq!(
        queued_of(&document, MaintenanceKind::ChunkCompaction),
        Some(2)
    );
}

/// An item nothing here produces is still completed rather than left, because
/// a head item nobody will ever service blocks everything behind it.
#[test]
fn an_item_this_document_cannot_service_does_not_block_the_ones_behind_it() {
    let mut document = sphere();
    document.request_maintenance(MaintenanceKind::SlotPoolCompaction, 1, 0);
    document.request_maintenance(MaintenanceKind::ChunkCompaction, 2, 0);

    assert_eq!(document.drain_maintenance(GENEROUS), 2);
    assert!(document.maintenance_queued().is_empty());
}

// -- what the drain actually does -------------------------------------------

/// The one kind this application can produce, end to end: a mesh drag refits
/// the ray-query tree on every segment, asks for a rebuild, and the moment the
/// pointer comes up decides whether the tree has drifted far enough from what
/// it scored when it was built to be worth paying for one.
#[test]
fn a_mesh_drag_asks_for_a_rebuild_and_the_moment_after_it_decides() {
    let (mut document, path) = with_imported_mesh("rebuild");
    let built = document
        .mesh_quality()
        .expect("a selected mesh layer reports what its tree costs");
    assert!(
        built.is_finite() && built >= 0.0,
        "there is no figure to measure a drift against: {built}"
    );

    for _ in 0..4 {
        drag(&mut document, ToolKind::Mover, [0.0, 0.0, 1.0]);
        document.end_gesture();
    }

    // Whether a rebuild was paid for is the engine's measurement and not this
    // test's to demand — it is the whole point that a host declines one. What
    // must hold either way is that the drag asked, the moment answered, and
    // nothing was left queued.
    assert!(
        document.maintenance_queued().is_empty(),
        "four drags left work nobody serviced"
    );
    let after = document
        .mesh_quality()
        .expect("the tree still reports what it costs");
    assert!(
        after.is_finite() && after >= 0.0,
        "the tree stopped reporting a figure a reader could act on: {after}"
    );
    let _ = std::fs::remove_file(&path);
}

/// Before anything has been paid for there is no figure to weigh a rebuild
/// against, and the request says so rather than guessing. What happens after
/// the first one is measured in `maintenance.rs`, where a rebuild can be
/// priced without waiting for a tree to decay.
#[test]
fn a_rebuild_nobody_has_paid_for_is_filed_as_unknown() {
    let (mut document, path) = with_imported_mesh("priced");
    assert_eq!(
        document.measured_rebuild_micros(),
        None,
        "a document that has rebuilt nothing already claims to know the cost"
    );

    drag(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0]);
    let filed = document
        .maintenance_queued()
        .first()
        .map(|item| item.estimated_micros)
        .expect("a mesh drag asked for nothing at all");
    assert_eq!(
        filed, 0,
        "the drag filed {filed} µs for a rebuild this machine has never made"
    );
    document.end_gesture();
    let _ = std::fs::remove_file(&path);
}

// -- the pin ----------------------------------------------------------------

/// A trim landing mid-drag is the one place its cost is certain to be paid by
/// the artist. The pin is what makes that impossible, and it is worth nothing
/// unless it comes back on *every* way out.
#[test]
fn the_pin_is_held_for_a_gesture_and_given_back_on_every_exit() {
    let (mut document, path) = with_imported_mesh("pin");
    assert!(
        !document.memory_pinned(),
        "a document with no gesture open is already holding the pin"
    );

    // The pointer comes up.
    document.begin_gesture();
    assert!(document.memory_pinned(), "a gesture opened without the pin");
    document.end_gesture();
    assert!(
        !document.memory_pinned(),
        "a committed gesture kept the pin"
    );

    // A drag that begins twice without ending, which a lost pointer release
    // produces: the pin is a count, so a second open must not leave one held.
    document.begin_gesture();
    document.begin_gesture();
    document.end_gesture();
    assert!(
        !document.memory_pinned(),
        "a gesture reopened over an open one left the pin held forever"
    );
    let _ = std::fs::remove_file(&path);
}

/// The cage is a second gesture lifecycle that never goes through
/// `begin_gesture`, and it is the one a hand-written release would have
/// missed.
#[test]
fn a_cage_takes_the_pin_and_gives_it_back_whether_it_is_applied_or_abandoned() {
    let (mut document, path) = with_imported_mesh("cage-pin");

    document.begin_lattice([2, 2, 2]).expect("a cage");
    lift_the_top(&mut document, 0.4);
    assert!(
        document.memory_pinned(),
        "a cage being dragged is a gesture and did not take the pin"
    );
    document.apply_lattice().expect("the cage was refused");
    assert!(!document.memory_pinned(), "an applied cage kept the pin");

    document.begin_lattice([2, 2, 2]).expect("a cage");
    lift_the_top(&mut document, 0.4);
    assert!(document.memory_pinned());
    document.cancel_lattice();
    assert!(!document.memory_pinned(), "an abandoned cage kept the pin");
    let _ = std::fs::remove_file(&path);
}

/// A cage dragged and then put back where it started is the identity, and it
/// is also a gesture that had a preview up — the one exit that used to leave
/// `previewing` set, which now means the gate shut and the pin held for the
/// rest of the session.
#[test]
fn a_cage_dragged_back_to_where_it_started_still_ends_its_gesture() {
    let (mut document, path) = with_imported_mesh("cage-identity");
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let rest = document.lattice().points.clone();

    lift_the_top(&mut document, 0.4);
    assert!(document.memory_pinned(), "there was no preview to put back");
    // Every point back to exactly the position it was drawn at.
    for (index, point) in rest.iter().enumerate() {
        document.select_lattice_point(Some(index));
        document.drag_lattice_point(*point).expect("the drag");
    }
    document.apply_lattice().expect("the cage was refused");

    assert!(
        !document.memory_pinned(),
        "a cage put back where it started kept the pin"
    );
    assert!(
        document.maintenance_queued().is_empty(),
        "and left the gate shut, so nothing will ever be serviced again"
    );
    let _ = std::fs::remove_file(&path);
}

/// Drags every control point above the middle up by `by`.
fn lift_the_top(document: &mut ClayDocument, by: f32) {
    let cage = document.lattice();
    for (index, point) in cage.points.iter().enumerate() {
        if point[1] <= 0.0 {
            continue;
        }
        document.select_lattice_point(Some(index));
        let mut to = *point;
        to[1] += by;
        document
            .drag_lattice_point(to)
            .expect("the drag was refused");
    }
}
