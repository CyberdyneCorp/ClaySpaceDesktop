//! The live brush transactions, and the facts the application builds on.
//!
//! Both are adopted — see `clayspace-engine`'s `live` module, and its
//! `live_smooth.rs` and `live_move.rs` for what the application promises on
//! top of them. What is asserted here is the engine's half: the facts those
//! promises are built on, stated where they can be checked against an engine
//! upgrade rather than inferred from a surface that looks wrong.
//!
//! Move was wrapped a release before it was adopted, and the measurement in
//! `a_session_of_drags_steepens_by_the_drag_and_no_longer_by_the_segment` is
//! why it then was: a drag written per segment multiplies the layer's
//! Lipschitz bound once per segment.
//!
//! The assertions about the *preview's shape* are not decoration either. The
//! application relabels the preview's bricks into a cache of its own lattice
//! rather than resampling them, and that is only sound while the halo is one
//! shared sample wide and the lattices really are offset. If an engine upgrade
//! changes either, this file says so before the surface does.

use claycore::{
    Document, Item, LayerId, MoveParams, MoveTransaction, RelaxParams, SculptPolicy,
    SmoothTransaction,
};

fn sphere() -> (Document, LayerId) {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.add_item(layer, &Item::sphere(1.0).expect("sphere"))
        .expect("place");
    (doc, layer)
}

fn chain(doc: &Document, layer: LayerId) -> i32 {
    doc.field_report(layer, 0.0)
        .expect("field report")
        .longest_deformer_chain
}

fn drag_params() -> MoveParams {
    MoveParams {
        radius: 0.3,
        ease: 0,
        front_only: true,
    }
}

/// How far the surface stands along +x.
fn reach(doc: &Document) -> f32 {
    let mut low = 0.5f32;
    let mut high = 3.0f32;
    for _ in 0..40 {
        let mid = 0.5 * (low + high);
        let inside = doc.eval_points(None, &[[mid, 0.0, 0.0]]).expect("eval")[0] < 0.0;
        if inside {
            low = mid;
        } else {
            high = mid;
        }
    }
    0.5 * (low + high)
}

// -- Move -------------------------------------------------------------------

#[test]
fn a_drag_collapses_to_one_grab_where_segments_leave_one_each() {
    // As the application drags today: one call per segment, each adding a grab
    // to the chain it found.
    let (mut doc, layer) = sphere();
    for step in 0..10 {
        doc.move_surface(
            layer,
            [1.0, step as f32 * 0.02, 0.0],
            [0.02, 0.0, 0.0],
            drag_params(),
        )
        .expect("a segment of the drag");
    }
    let segmented = chain(&doc, layer);

    // The same drag through the transaction.
    let (mut doc, layer) = sphere();
    let mut tx = MoveTransaction::begin(&mut doc, layer, [1.0, 0.0, 0.0], drag_params(), None)
        .expect("begin");
    for step in 1..=10 {
        tx.update([0.02 * step as f32, 0.0, 0.0]).expect("update");
    }
    tx.commit().expect("commit");

    assert_eq!(
        segmented, 10,
        "a segmented drag leaves one grab per segment"
    );
    assert_eq!(
        chain(&doc, layer),
        1,
        "the transaction rebuilds the chain from what it captured at begin, so \
         a whole drag is one grab however many frames drew it"
    );
}

#[test]
fn a_drag_touches_nothing_until_it_commits() {
    let (mut doc, layer) = sphere();
    let before = reach(&doc);
    let mut tx = MoveTransaction::begin(&mut doc, layer, [1.0, 0.0, 0.0], drag_params(), None)
        .expect("begin");
    for step in 1..=5 {
        tx.update([0.05 * step as f32, 0.0, 0.0]).expect("update");
    }
    assert!(
        !tx.reached().expect("reached nodes").is_empty(),
        "a drag on the surface should reach the item under it"
    );
    // Dropped rather than committed: `Drop` is a cancel.
    drop(tx);

    assert!(
        (reach(&doc) - before).abs() < 1e-4,
        "an abandoned drag moved the surface; the document is supposed to be \
         untouched between begin and commit"
    );
}

#[test]
fn a_drag_is_measured_from_its_anchor_and_not_from_the_last_frame() {
    let apply = |steps: &[f32]| {
        let (mut doc, layer) = sphere();
        let mut tx = MoveTransaction::begin(&mut doc, layer, [1.0, 0.0, 0.0], drag_params(), None)
            .expect("begin");
        for total in steps {
            tx.update([*total, 0.0, 0.0]).expect("update");
        }
        tx.commit().expect("commit");
        reach(&doc)
    };

    let stepped = apply(&[0.1, 0.2, 0.5]);
    let straight = apply(&[0.5]);
    assert!(
        (stepped - straight).abs() < 1e-3,
        "0.1 then 0.2 then 0.5 ended at {stepped} where a single 0.5 ends at \
         {straight}: the updates composed instead of replacing"
    );
}

#[test]
fn a_drag_that_reaches_nothing_is_not_an_error() {
    let (mut doc, layer) = sphere();
    let tx = MoveTransaction::begin(&mut doc, layer, [8.0, 8.0, 8.0], drag_params(), None)
        .expect("pressing on empty space is not a failure");
    assert!(tx.reached().expect("reached nodes").is_empty());
}

// -- the preview's shape, which the application's relabelling rests on -------

/// A layer whose bounds are deliberately off the lattice.
fn off_lattice() -> (Document, LayerId) {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    let mut sphere = Item::sphere(0.523).expect("sphere");
    sphere.set_position([0.017, -0.031, 0.004]).expect("offset");
    doc.add_item(layer, &sphere).expect("place");
    (doc, layer)
}

#[test]
fn a_preview_brick_carries_one_shared_boundary_sample() {
    let (mut doc, layer) = off_lattice();
    let mut tx = SmoothTransaction::begin(&mut doc, layer, SculptPolicy::at(0.05)).expect("begin");
    tx.update(RelaxParams {
        strength: 1.0,
        radius_cells: 1,
        iterations: 1,
        centre: [0.5, 0.0, 0.0],
        region_radius: 0.25,
        falloff: 0.12,
        mask: None,
    })
    .expect("a dab");
    let delta = tx.take_preview().expect("take").expect("something changed");

    let bricks = &delta.bricks;
    let first = bricks.first().expect("at least one brick");
    // The stride between two neighbouring bricks' origins is the brick's own
    // span, so the *interior* is that span divided by the spacing — and the
    // samples are one more than that, the boundary sample shared with the
    // neighbour. Not a two-wide apron, which is what a brick cache reads back.
    let along_x = bricks
        .iter()
        .find(|b| b.key == [first.key[0] + 1, first.key[1], first.key[2]])
        .expect("a neighbour along x");
    let span = along_x.origin[0] - first.origin[0];
    let interior = (span / first.spacing).round() as u32;
    assert_eq!(
        first.sample_dim,
        interior + 1,
        "the preview's halo is not the one shared boundary sample the \
         application strips, so relabelling would copy the wrong samples"
    );
}

#[test]
fn the_preview_lattice_does_not_land_on_the_brick_lattice() {
    let (mut doc, layer) = off_lattice();
    let spacing = 0.05f32;
    let mut tx =
        SmoothTransaction::begin(&mut doc, layer, SculptPolicy::at(spacing)).expect("begin");
    tx.update(RelaxParams {
        strength: 1.0,
        radius_cells: 1,
        iterations: 1,
        centre: [0.5, 0.0, 0.0],
        region_radius: 0.25,
        falloff: 0.12,
        mask: None,
    })
    .expect("a dab");
    let delta = tx.take_preview().expect("take").expect("something changed");
    let first = delta.bricks.first().expect("at least one brick");

    // This is *why* the preview keeps a cache of its own. A brick cache's
    // lattice is anchored at the world origin; the preview's is anchored at
    // the layer's bounds less the padding, which lands between samples. One
    // scalar padding cannot align three axes whose bounds differ, so this is
    // not something the caller can ask its way out of.
    let off_by = (0..3).filter(|axis| {
        let in_samples = first.origin[*axis] / first.spacing;
        (in_samples - in_samples.round()).abs() > 1e-3
    });
    assert!(
        off_by.count() > 0,
        "the preview lattice landed on the brick lattice, which would make the \
         application's relabelling unnecessary — worth knowing, and worth \
         simplifying `clayspace_engine::live` for"
    );
}

#[test]
fn the_snapshot_path_hands_over_a_volume_the_caller_owns() {
    // The path the application measured and did not take: a fresh volume item
    // per frame re-pays the receiving document's preparation, ~30 ms at the
    // application's own sampling, where the delta above costs a few hundred
    // microseconds. Kept because it is what a host joining mid-gesture wants,
    // and run here because nothing else runs it.
    let (mut doc, layer) = off_lattice();
    let mut tx = SmoothTransaction::begin(&mut doc, layer, SculptPolicy::at(0.05)).expect("begin");
    tx.update(RelaxParams {
        strength: 1.0,
        radius_cells: 1,
        iterations: 1,
        centre: [0.5, 0.0, 0.0],
        region_radius: 0.25,
        falloff: 0.12,
        mask: None,
    })
    .expect("a dab");

    let item = tx.preview_item().expect("the preview so far");
    // A copy, so it outlives the transaction that made it rather than
    // pointing into a volume that goes on being relaxed.
    drop(tx);
    let mut scratch = Document::new().expect("scratch");
    let into = scratch.add_sdf_layer("Preview").expect("layer");
    scratch
        .add_item(into, &item)
        .expect("the snapshot is placeable");
    assert!(
        scratch.eval_points(None, &[[0.0, 0.0, 0.0]]).expect("eval")[0] < 0.0,
        "the snapshot did not carry the form it was taken of"
    );
}

#[test]
fn the_smooth_commit_installs_the_layer_as_one_volume() {
    // The half of the smooth transaction the application does not use, run
    // here because nothing else runs it — and asserted on the consequence
    // that is the reason it is not used: committing replaces everything the
    // layer held with a single resampled volume, whatever the stroke touched.
    //
    // On a layer of four stamps that is four items becoming one. Measured
    // through the visual gate it is also 7.82 roughness against 5.74 on one
    // backend (ClayCore#379), which is why the application previews with the
    // transaction and lays the stroke down with the bake it always used.
    //
    // Re-checked at v0.78.0: #379 is not in that release's fixes, its known
    // limits or its 146 new entry points, so the consolidation is still what
    // a commit does and the application still declines it.
    let (mut doc, layer) = sphere();
    for at in [-0.6f32, -0.2, 0.2, 0.6] {
        let mut stamp = Item::sphere(0.3).expect("stamp");
        stamp.set_position([at, 0.9, 0.0]).expect("place it");
        doc.add_item(layer, &stamp).expect("add");
    }
    assert_eq!(
        doc.field_report(layer, 0.0).expect("report").item_count,
        5,
        "the fixture did not build the edit list this is about"
    );

    let mut tx = SmoothTransaction::begin(&mut doc, layer, SculptPolicy::at(0.05)).expect("begin");
    tx.update(RelaxParams {
        strength: 1.0,
        radius_cells: 1,
        iterations: 2,
        centre: [0.6, 0.9, 0.0],
        region_radius: 0.25,
        falloff: 0.12,
        mask: None,
    })
    .expect("one dab, in one place");
    tx.commit().expect("commit");

    assert_eq!(
        doc.field_report(layer, 0.0).expect("report").item_count,
        1,
        "the commit was supposed to collapse the layer to the working volume; \
         if it no longer does, the reason for not using it may have gone"
    );
}

// -- what the application's live Move rests on ------------------------------

#[test]
fn a_preview_grab_can_be_drawn_and_taken_back_under_an_open_drag() {
    // The application draws a Move drag by writing the transaction's resolved
    // grabs onto the layer, sampling them into its brick cache and undoing
    // them again — the C ABI carries no `preview_layer`, so this is the only
    // door. Three things have to hold for that to be sound, and this asserts
    // all three rather than trusting them.
    let (mut doc, layer) = sphere();
    doc.enable_undo()
        .expect("undo, as an interactive host has it");
    let resting = reach(&doc);

    let mut tx = MoveTransaction::begin(&mut doc, layer, [1.0, 0.0, 0.0], drag_params(), None)
        .expect("begin");
    tx.update([0.2, 0.0, 0.0]).expect("update");
    let drawn: Vec<_> = tx
        .reached()
        .expect("reached")
        .into_iter()
        .flat_map(|node| {
            tx.grabs(node)
                .expect("grabs")
                .into_iter()
                .map(move |grab| (node, grab))
        })
        .collect();
    assert!(!drawn.is_empty(), "a drag on the surface reaches its item");

    let before = doc.undo_state().expect("undo state").undo_depth;
    for (node, grab) in &drawn {
        doc.add_grab(layer, *node, *grab).expect("draw the preview");
    }
    let depth = doc.undo_state().expect("undo state").undo_depth;

    // One: the drawn preview moves the surface, or there is nothing to look at.
    let previewed = reach(&doc);
    assert!(
        previewed > resting + 1e-3,
        "the preview left the surface at {previewed} where it rested at \
         {resting}: writing the resolved grabs drew nothing"
    );

    // Two: it is undoable, and each grab is its own entry — the application
    // spends exactly as many undos as it wrote.
    assert_eq!(
        depth - before,
        drawn.len(),
        "a drawn grab has to be one undo entry; the application takes its \
         preview back by spending one per grab"
    );
    for _ in before..depth {
        doc.undo().expect("take the preview back");
    }
    assert!(
        (reach(&doc) - resting).abs() < 1e-4,
        "undoing the preview did not put the surface back"
    );

    // Three: the commit still accepts the layer. It re-checks a stamp derived
    // from the layer's CONTENT, and the whole design depends on that stamp
    // coming back when the content does.
    tx.commit()
        .expect("a layer edited and restored is a layer that did not change");
    assert_eq!(
        chain(&doc, layer),
        1,
        "the commit writes the drag as one grab"
    );
    assert!(
        (reach(&doc) - previewed).abs() < 1e-4,
        "what the gesture previewed is not what its commit installed"
    );
}

#[test]
fn a_session_of_drags_steepens_by_the_drag_and_no_longer_by_the_segment() {
    // The measurement behind adopting the transaction. A drag arrives in
    // segments; the question is whether the field pays per segment or per
    // gesture, because the deformer chain's Lipschitz bound MULTIPLIES.
    const DRAGS: usize = 12;
    const SEGMENTS: usize = 6;

    let chain_after = |transactional: bool| {
        let (mut doc, layer) = sphere();
        for drag in 0..DRAGS {
            let base = drag as f32 * 0.03;
            if transactional {
                let mut tx =
                    MoveTransaction::begin(&mut doc, layer, [1.0, base, 0.0], drag_params(), None)
                        .expect("begin");
                for segment in 1..=SEGMENTS {
                    tx.update([0.01 * segment as f32, 0.0, 0.0])
                        .expect("update");
                }
                tx.commit().expect("commit");
            } else {
                // As the application dragged before: one call per segment,
                // each re-anchored where the last one stopped.
                for segment in 0..SEGMENTS {
                    doc.move_surface(
                        layer,
                        [1.0, base + segment as f32 * 0.01, 0.0],
                        [0.01, 0.0, 0.0],
                        drag_params(),
                    )
                    .expect("segment");
                }
            }
        }
        let report = doc.field_report(layer, 0.5).expect("report");
        (report.longest_deformer_chain, report.safe_step_scale)
    };

    let (segmented, segmented_step) = chain_after(false);
    let (transactional, transactional_step) = chain_after(true);

    assert_eq!(
        segmented as usize,
        DRAGS * SEGMENTS,
        "the segmented drag is supposed to leave one grab per segment"
    );
    assert_eq!(
        transactional as usize, DRAGS,
        "a transactional drag leaves one grab per GESTURE, however many \
         segments drew it"
    );
    assert!(
        transactional_step > segmented_step * 3.0,
        "the safe step scale should improve by roughly the segments-per-drag \
         factor: {transactional_step} against {segmented_step}"
    );
}
