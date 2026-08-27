//! The wrappers the application has not reached for yet, run at least once.
//!
//! This crate is the workspace's only `unsafe`, and a wrapper nothing calls is
//! a SAFETY comment nobody has checked: the pointer reasoning has never been
//! executed, so a wrong lifetime or a missing `struct_size` would sit there
//! looking correct. Everything asserted here was found by grepping the whole
//! workspace for callers and coming back empty.
//!
//! The assertions are about consequences rather than about `Ok(())` wherever
//! the engine offers one to read back — a call that returns success without
//! reaching the field is exactly the failure this file is written to catch.

use claycore::{
    BrushParams, BrushShape, Document, Falloff, Item, Mask, MeshParams, Primitive, StrokePreset,
    StrokeSample,
};

/// Whether a point is inside the surface.
fn inside(doc: &Document, at: [f32; 3]) -> bool {
    doc.eval_points(None, &[at]).expect("evaluate")[0] < 0.0
}

// -- item transforms --------------------------------------------------------

#[test]
fn scaling_an_item_scales_what_it_covers() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    let mut item = Item::sphere(0.5).expect("sphere");
    item.set_scale(2.0).expect("scale the sphere");
    doc.add_item(layer, &item).expect("place");

    // Outside the half-metre sphere, inside the scaled one.
    assert!(inside(&doc, [0.9, 0.0, 0.0]), "the scale did not reach");
    assert!(!inside(&doc, [1.1, 0.0, 0.0]), "it reached too far");
}

#[test]
fn rotating_an_item_turns_what_it_covers() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    // Long in x, thin in y and z, so a quarter turn about z is visible in the
    // field rather than symmetric and unreadable.
    let mut item = Item::of(Primitive::Box {
        half: [1.0, 0.2, 0.2],
    })
    .expect("box");
    item.set_rotation([0.0, 0.0, 1.0], std::f32::consts::FRAC_PI_2)
        .expect("rotate the box");
    doc.add_item(layer, &item).expect("place");

    assert!(
        inside(&doc, [0.0, 0.9, 0.0]),
        "the long axis did not turn into y"
    );
    assert!(
        !inside(&doc, [0.9, 0.0, 0.0]),
        "the long axis is still in x"
    );
}

#[test]
fn a_radial_repeat_puts_material_where_one_copy_had_none() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    // A capsule rather than a positioned sphere, because the repeat is applied
    // in the item's own space and `set_position` moves the result: a sphere
    // offset by the position is repeated at the origin and then carried away
    // whole, which looks exactly like a repeat that did nothing. The offset
    // has to be in the primitive.
    let mut item = Item::of(Primitive::Capsule {
        from: [0.9, 0.0, 0.0],
        to: [1.1, 0.0, 0.0],
        radius: 0.2,
    })
    .expect("capsule");
    item.set_repeat_radial(4, 0.0).expect("repeat radially");
    doc.add_item(layer, &item).expect("place");

    // The original stands at +x; a fourfold repeat about the origin puts
    // copies where a single capsule is nothing but empty space.
    assert!(inside(&doc, [1.0, 0.0, 0.0]), "the original is missing");
    assert!(
        inside(&doc, [-1.0, 0.0, 0.0]),
        "the radial repeat placed no copy opposite"
    );
    assert!(
        inside(&doc, [0.0, 0.0, 1.0]),
        "the radial repeat placed no copy a quarter turn along"
    );
}

#[test]
fn a_grid_repeat_puts_material_where_one_copy_had_none() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    let mut item = Item::sphere(0.2).expect("sphere");
    item.set_repeat_grid([1.0; 3], [2.0, 0.0, 0.0])
        .expect("repeat on a grid");
    doc.add_item(layer, &item).expect("place");

    assert!(inside(&doc, [0.0, 0.0, 0.0]), "the original is missing");
    assert!(
        inside(&doc, [1.0, 0.0, 0.0]),
        "the grid repeat placed no copy one cell along"
    );
}

#[test]
fn a_node_can_be_recoloured_after_it_is_placed() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    let item = Item::sphere(0.5).expect("sphere");
    let node = doc.add_item(layer, &item).expect("place");

    doc.set_node_color(layer, node, [0.2, 0.7, 0.4])
        .expect("recolour the node");

    // The colour is not readable back through the ABI, so what is checked is
    // that the call addresses a node: a node id the layer does not hold is
    // refused rather than quietly accepted.
    let absent = claycore::NodeId::restored(node.get() + 1000);
    assert!(
        doc.set_node_color(layer, absent, [1.0; 3]).is_err(),
        "recolouring a node that does not exist reported success"
    );
}

// -- voxels -----------------------------------------------------------------

#[test]
fn a_palette_entry_reads_back_the_colour_it_was_given() {
    let mut doc = Document::new().expect("document");
    let (_layer, mut grid) = doc.add_voxel_layer("Voxels", 0.1).expect("voxel layer");

    let index = grid.palette_add([0.25, 0.5, 0.75]).expect("palette");
    let colour = grid.palette_color(index).expect("read the palette entry");

    for (at, expected) in [0.25f32, 0.5, 0.75].iter().enumerate() {
        assert!(
            (colour[at] - expected).abs() < 1e-3,
            "channel {at} came back as {} rather than {expected}",
            colour[at]
        );
    }
}

#[test]
fn filling_a_line_writes_the_cells_between_its_ends() {
    let mut doc = Document::new().expect("document");
    let (_layer, mut grid) = doc.add_voxel_layer("Voxels", 0.1).expect("voxel layer");
    let index = grid.palette_add([1.0; 3]).expect("palette");

    // Diagonal, so a line is told apart from the box through the same two
    // ends: the box would fill all twenty-five cells of the square.
    grid.fill_line([0, 0, 0], [4, 4, 0], index)
        .expect("fill a line");

    assert_eq!(grid.get([0, 0, 0]).expect("read"), Some(index));
    assert_eq!(grid.get([2, 2, 0]).expect("read"), Some(index));
    assert_eq!(grid.get([4, 4, 0]).expect("read"), Some(index));
    assert_eq!(
        grid.get([4, 0, 0]).expect("read"),
        None,
        "a corner off the line was filled"
    );
    assert!(
        grid.occupied_count().expect("occupied") <= 9,
        "a five-cell diagonal filled {} cells — that is an area, not a line",
        grid.occupied_count().expect("occupied")
    );
}

#[test]
fn dropping_a_level_takes_the_grid_back_to_the_one_below() {
    let mut doc = Document::new().expect("document");
    let (_layer, mut grid) = doc.add_voxel_layer("Voxels", 0.2).expect("voxel layer");
    let index = grid.palette_add([1.0; 3]).expect("palette");
    grid.fill_box([0, 0, 0], [4, 4, 4], index).expect("fill");

    grid.add_level().expect("add a finer level");
    assert_eq!(grid.level_count().expect("levels"), 2);

    grid.drop_level().expect("drop the finest level");
    assert_eq!(
        grid.level_count().expect("levels"),
        1,
        "the finest level survived being dropped"
    );
}

#[test]
fn a_mesh_rasterises_straight_into_a_grid() {
    let mut source = Document::new().expect("document");
    let layer = source.add_sdf_layer("Base").expect("layer");
    let item = Item::sphere(0.5).expect("sphere");
    source.add_item(layer, &item).expect("place");
    let mesh = source
        .mesh(MeshParams {
            voxel_size: Some(0.05),
            ..Default::default()
        })
        .expect("mesh the sphere");

    let mut doc = Document::new().expect("document");
    let (_layer, mut grid) = doc.add_voxel_layer("Voxels", 0.05).expect("voxel layer");
    grid.palette_add([1.0; 3]).expect("palette");

    grid.rasterize_mesh(&mesh, ([-1.0; 3], [1.0; 3]))
        .expect("rasterise the mesh");

    assert!(
        grid.occupied_count().expect("occupied") > 0,
        "rasterising a sphere filled no cell at all"
    );
}

#[test]
fn a_sculpt_layer_is_recording_only_between_its_two_ends() {
    let mut doc = Document::new().expect("document");
    let (_layer, mut grid) = doc.add_voxel_layer("Voxels", 0.1).expect("voxel layer");
    let index = grid.palette_add([1.0; 3]).expect("palette");
    grid.fill_box([0, 0, 0], [4, 4, 4], index).expect("fill");

    assert!(
        !grid.recording_sculpt_layer().expect("recording"),
        "a grid reports recording before anything asked it to"
    );

    grid.begin_sculpt_layer(Some("Pass")).expect("begin");
    assert!(
        grid.recording_sculpt_layer().expect("recording"),
        "begin_sculpt_layer left nothing recording"
    );

    let brush = BrushParams {
        size: 5,
        ..Default::default()
    };
    // At the block's corner, where half the footprint is empty and dilation
    // has somewhere to go — an edit that changes nothing records nothing, and
    // a stack that costs nothing would say nothing about whether the pass is
    // being kept.
    grid.sculpt_inflate([4, 4, 4], &brush, 1).expect("inflate");
    let recorded = grid.sculpt_layers_bytes().expect("stack size");
    assert!(
        recorded > 0,
        "a recorded edit left the pass stack costing nothing"
    );

    grid.end_sculpt_layer().expect("end");
    assert!(
        !grid.recording_sculpt_layer().expect("recording"),
        "end_sculpt_layer left it recording"
    );
    assert_eq!(
        grid.sculpt_layers_bytes().expect("stack size"),
        recorded,
        "ending the pass changed what the stack costs"
    );
}

// -- masks and cages --------------------------------------------------------

#[test]
fn a_painted_mask_becomes_a_field_that_covers_where_it_was_painted() {
    let mut mask = Mask::new(0.05).expect("mask");
    let samples: Vec<StrokeSample> = (0..5)
        .map(|i| {
            let t = i as f32 / 4.0;
            StrokeSample {
                position: [(t - 0.5) * 0.2, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    mask.apply_stroke(
        &samples,
        &StrokePreset {
            radius: 0.3,
            ..Default::default()
        },
        1.0,
        BrushShape::default(),
        Falloff::default(),
    )
    .expect("paint the mask");

    let item = mask
        .to_field(0.5, 0.1, 0.1, 0.05)
        .expect("turn the mask into a field");

    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.add_item(layer, &item).expect("place the field");

    assert!(
        inside(&doc, [0.0, 0.0, 0.1]),
        "the field does not cover where the mask was painted"
    );
}

#[test]
fn a_cage_reports_the_reach_it_would_have_before_it_warps_anything() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    let item = Item::sphere(0.5).expect("sphere");
    doc.add_item(layer, &item).expect("place");

    let cage = claycore::GizmoCage::default();
    let offsets = vec![[0.0, 0.1, 0.0]; cage.point_count()];
    assert_eq!(
        doc.lattice_gizmo_reach(layer, cage, &offsets)
            .expect("ask what the cage would reach"),
        1,
        "a cage around the form did not count the node it holds"
    );

    let mut second = Item::sphere(0.3).expect("sphere");
    second.set_position([3.0, 0.0, 0.0]).expect("offset it");
    doc.add_item(layer, &second).expect("place the second");
    assert_eq!(
        doc.lattice_gizmo_reach(layer, cage, &offsets)
            .expect("ask again with two nodes"),
        2,
        "the count did not follow the layer's nodes"
    );

    // What "reaches nothing" means here is a cage whose control points have
    // not moved, and not a cage standing somewhere else: the engine counts the
    // nodes the warp would address, and it addresses the layer.
    let still = vec![[0.0f32; 3]; cage.point_count()];
    assert_eq!(
        doc.lattice_gizmo_reach(layer, cage, &still)
            .expect("ask with an unmoved cage"),
        0,
        "an unmoved cage claimed it would warp something"
    );

    let empty = doc.add_sdf_layer("Empty").expect("layer");
    assert_eq!(
        doc.lattice_gizmo_reach(empty, cage, &offsets)
            .expect("ask about a layer holding nothing"),
        0,
        "a layer holding nothing reported a reach"
    );

    // And the question left the field alone, which is the whole point of
    // asking it before applying.
    assert!(inside(&doc, [0.0, 0.0, 0.0]), "asking moved the form");
}

// -- the batched picks, and the reader that forwards them --------------------
//
// Added after a review found the promise in `lib.rs` overreaching: it said
// every wrapper is executed, and four entry points still had no caller and no
// test anywhere in the workspace. These are those, plus the `Reader`
// forwarders, so the sentence is true rather than aspirational.

#[test]
fn a_batch_of_rays_answers_each_one_separately() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.add_item(layer, &Item::sphere(0.5).expect("sphere"))
        .expect("place");

    // One ray down the axis into the sphere, one aimed past it entirely.
    let hits = doc
        .raycast_many(&[
            ([0.0, 0.0, 3.0], [0.0, 0.0, -1.0]),
            ([0.0, 5.0, 3.0], [0.0, 0.0, -1.0]),
        ])
        .expect("cast");

    assert_eq!(hits.len(), 2, "one answer per ray, in the order given");
    let front = hits[0].expect("the ray down the axis met the sphere");
    assert!(
        (front.position[2] - 0.5).abs() < 0.05,
        "met the surface at {:?} rather than the sphere's front face",
        front.position
    );
    assert!(
        front.normal[2] > 0.9,
        "the normal at the front face should point back down the ray, got {:?}",
        front.normal
    );
    assert!(hits[1].is_none(), "the ray aimed past it should miss");
}

#[test]
fn an_empty_batch_asks_the_engine_nothing() {
    // The early return exists so that an empty slice never becomes a null
    // pointer with a non-zero count, which is the shape of the bug the SAFETY
    // comment on this wrapper is reasoning about.
    let doc = Document::new().expect("document");
    assert!(doc.raycast_many(&[]).expect("cast none").is_empty());
    assert!(doc.snap_to_surface(&[]).expect("snap none").is_empty());
}

#[test]
fn snapping_moves_a_point_onto_the_surface() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.add_item(layer, &Item::sphere(0.5).expect("sphere"))
        .expect("place");

    // One point well outside the sphere and one well inside it: both should
    // land on the same surface, which is what distinguishes a snap from a
    // clamp.
    let snapped = doc
        .snap_to_surface(&[[1.5, 0.0, 0.0], [0.1, 0.0, 0.0]])
        .expect("snap");
    assert_eq!(snapped.len(), 2);
    for (at, point) in snapped.iter().enumerate() {
        let point = point.expect("both points are within reach of the surface");
        let radius =
            (point.position[0].powi(2) + point.position[1].powi(2) + point.position[2].powi(2))
                .sqrt();
        assert!(
            (radius - 0.5).abs() < 0.05,
            "point {at} landed {radius} from the centre, not on the 0.5 surface"
        );
    }
}

#[test]
fn the_reader_forwards_both_batches_to_the_document() {
    // `Reader` exists so a borrow can read without holding the document
    // mutably; these two methods are pure forwards, and the thing worth
    // asserting is that they answer the same as the document does.
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.add_item(layer, &Item::sphere(0.5).expect("sphere"))
        .expect("place");

    let rays = [([0.0, 0.0, 3.0], [0.0, 0.0, -1.0])];
    let points = [[1.5, 0.0, 0.0]];
    let reader = doc.reader();

    let direct = doc.raycast_many(&rays).expect("document")[0].expect("hit");
    let through = reader.raycast_many(&rays).expect("reader")[0].expect("hit");
    assert_eq!(direct.position, through.position);

    let direct = doc.snap_to_surface(&points).expect("document")[0].expect("snap");
    let through = reader.snap_to_surface(&points).expect("reader")[0].expect("snap");
    assert_eq!(direct.position, through.position);
}

#[test]
fn a_volume_item_reads_the_cells_the_grid_holds() {
    // `clay_item_volume_from_voxels` is the return leg of a crossing: cells
    // become a field item. Asserted on where the field is, not on `Ok` — a
    // wrapper that handed back an item built from nothing would pass that.
    let mut grid = claycore::VoxelGrid::new(0.05).expect("grid");
    grid.fill_box([-4, -4, -4], [4, 4, 4], 1)
        .expect("fill a block of cells");

    let item = Item::volume_from_voxels(&grid, 0, 1).expect("read the cells back");
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.add_item(layer, &item).expect("place the volume");

    // The block spans four cells of 0.05 either side of the origin, so the
    // centre is inside it and a point well beyond the corner is not.
    assert!(inside(&doc, [0.0, 0.0, 0.0]), "the volume has no inside");
    assert!(
        !inside(&doc, [1.0, 1.0, 1.0]),
        "the volume reaches far past the cells it was built from"
    );
}

#[test]
fn an_armature_edit_moves_the_subtree_it_names() {
    // `clay_layer_armature_edit` is the one wrapper the application chooses
    // not to call, and says why: reparenting has no op there, so a rig would
    // have two code paths to keep in step. The wrapper still exists, so its
    // pointer reasoning still has to be run at least once.
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Rig").expect("layer");

    // Two spheres in a line, the second a child of the first.
    let mut item = Item::armature().expect("armature");
    item.set_stroke_points(&[0.0, 0.0, 0.0, 0.25, 0.6, 0.0, 0.0, 0.25])
        .expect("two nodes, x/y/z/radius each");
    item.set_armature_parents(&[0, 0])
        .expect("the second under the first");
    let node = doc.add_item(layer, &item).expect("place the rig");

    // Well past the child at x = 0.6 and its 0.25 radius.
    let beyond = [1.4, 0.0, 0.0];
    assert!(!inside(&doc, beyond), "the rig already reaches {beyond:?}");

    doc.armature_edit(
        layer,
        node,
        claycore::ArmatureEdit::Move {
            delta: [0.7, 0.0, 0.0],
        },
        1,
        false,
    )
    .expect("move the child");

    assert!(
        inside(&doc, beyond),
        "the child did not move: {beyond:?} is still outside after a 0.7 shift \
         that should have carried it there"
    );
}
