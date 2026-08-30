//! The live brush transactions, and the facts the application builds on.
//!
//! Smooth is adopted (see `clayspace-engine`'s `live` module); Move is wrapped
//! and measured but deliberately not adopted, so this file is the only thing
//! that runs it. That matters here for the reason `abi_surface.rs` gives: this
//! crate is the workspace's only `unsafe`, and a wrapper nothing calls is a
//! SAFETY comment nobody has checked.
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
