//! Authoring, voxels, masks, strokes and undo — headlessly.
//!
//! Completes the bridge suite: everything a sculpting session does to a
//! document, exercised without a window or a GPU.

use claycore::{
    Blend, BrushParams, BrushShape, Document, Falloff, Item, Mask, Op, Protection, StrokePreset,
    StrokeSample, VoxelGrid,
};

fn sphere_doc() -> (Document, claycore::LayerId) {
    let mut doc = Document::new().expect("create document");
    let layer = doc.add_sdf_layer("Base").expect("add layer");
    let item = Item::sphere(1.0).expect("sphere");
    doc.add_item(layer, &item).expect("place");
    (doc, layer)
}

// -- authoring --------------------------------------------------------------

#[test]
fn item_settings_reach_the_field() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");

    let mut base = Item::sphere(1.0).expect("sphere");
    base.set_op(Op::Add).expect("op");
    doc.add_item(layer, &base).expect("place base");

    // A subtracted sphere offset along x must hollow the near side.
    let mut cut = Item::sphere(0.6).expect("sphere");
    cut.set_op(Op::Subtract).expect("op");
    cut.set_position([1.0, 0.0, 0.0]).expect("position");
    doc.add_item(layer, &cut).expect("place cut");

    let inside_cut = doc.eval_points(None, &[[1.0, 0.0, 0.0]]).expect("evaluate")[0];
    assert!(
        inside_cut > 0.0,
        "the subtracted region should read as outside the surface, got {inside_cut}"
    );
}

#[test]
fn a_blend_widens_the_seam() {
    // Two spheres apart enough that a hard union leaves a gap between them,
    // while a wide smooth blend bridges it.
    let build = |blend: Option<(Blend, f32)>| {
        let mut doc = Document::new().expect("document");
        let layer = doc.add_sdf_layer("Base").expect("layer");
        for x in [-0.8f32, 0.8] {
            let mut item = Item::sphere(0.6).expect("sphere");
            item.set_position([x, 0.0, 0.0]).expect("position");
            if let Some((b, k)) = blend {
                item.set_blend(b, k).expect("blend");
            }
            doc.add_item(layer, &item).expect("place");
        }
        doc.eval_points(None, &[[0.0, 0.0, 0.0]]).expect("evaluate")[0]
    };

    let hard = build(None);
    let smooth = build(Some((Blend::Quadratic, 0.5)));
    assert!(
        smooth < hard,
        "a smooth blend should pull the midpoint closer to the surface: {smooth} vs {hard}"
    );
}

#[test]
fn layer_protection_states_are_distinct() {
    let (mut doc, layer) = sphere_doc();

    let ghost = Protection {
        ghost: true,
        locked: false,
    };
    doc.set_layer_protection(layer, ghost).expect("set ghost");
    let read = doc.layer_protection(layer).expect("read protection");
    assert_eq!(read, ghost);
    assert!(!read.is_pickable(), "a ghosted layer must not be pickable");
    assert!(!read.is_editable(), "a ghosted layer must not be editable");

    let locked = Protection {
        ghost: false,
        locked: true,
    };
    doc.set_layer_protection(layer, locked).expect("set locked");
    let read = doc.layer_protection(layer).expect("read protection");
    assert!(read.is_pickable(), "a locked layer stays pickable");
    assert!(!read.is_editable(), "a locked layer is not editable");
}

#[test]
fn a_ghosted_layer_is_not_picked() {
    let (mut doc, layer) = sphere_doc();
    assert!(
        doc.raycast_attributed([0.0, 0.0, -5.0], [0.0, 0.0, 1.0])
            .expect("raycast")
            .is_some(),
        "the sphere should be picked before it is ghosted"
    );

    doc.set_layer_protection(
        layer,
        Protection {
            ghost: true,
            locked: false,
        },
    )
    .expect("ghost the layer");

    let hit = doc
        .raycast_attributed([0.0, 0.0, -5.0], [0.0, 0.0, 1.0])
        .expect("raycast");
    assert!(
        hit.is_none(),
        "a ghosted layer must not be picked, got {hit:?}"
    );
}

#[test]
fn hiding_a_layer_removes_its_contribution() {
    let (mut doc, layer) = sphere_doc();
    let centre = [[0.0f32, 0.0, 0.0]];

    let visible = doc.eval_points(None, &centre).expect("evaluate")[0];
    assert!(visible < 0.0, "the origin is inside the sphere");

    doc.set_layer_visible(layer, false).expect("hide");
    let hidden = doc.eval_points(None, &centre).expect("evaluate")[0];
    assert!(
        hidden > visible,
        "hiding the only layer must remove its contribution: {hidden} vs {visible}"
    );
}

// -- undo -------------------------------------------------------------------

#[test]
fn undo_and_redo_restore_the_document() {
    let mut doc = Document::new().expect("document");
    doc.enable_undo().expect("enable undo");
    let layer = doc.add_sdf_layer("Base").expect("layer");

    let item = Item::sphere(1.0).expect("sphere");
    doc.add_item(layer, &item).expect("place");

    let probe = [[0.0f32, 0.0, 0.0]];
    let with = doc.eval_points(None, &probe).expect("evaluate")[0];
    assert!(with < 0.0);

    assert!(doc.undo().expect("undo"), "there was an edit to undo");
    let without = doc.eval_points(None, &probe).expect("evaluate")[0];
    assert!(without > with, "undo did not remove the sphere");

    assert!(doc.redo().expect("redo"), "there was an edit to redo");
    let again = doc.eval_points(None, &probe).expect("evaluate")[0];
    assert_eq!(again, with, "redo did not restore the sphere exactly");
}

#[test]
fn a_group_undoes_as_one_step() {
    let mut doc = Document::new().expect("document");
    doc.enable_undo().expect("enable undo");
    let layer = doc.add_sdf_layer("Base").expect("layer");

    let before = doc.undo_state().expect("state").undo_depth;

    doc.undo_group(|doc| {
        for x in [-0.5f32, 0.0, 0.5] {
            let mut item = Item::sphere(0.3)?;
            item.set_position([x, 0.0, 0.0])?;
            doc.add_item(layer, &item)?;
        }
        Ok(())
    })
    .expect("grouped edits");

    let after = doc.undo_state().expect("state").undo_depth;
    assert_eq!(
        after - before,
        1,
        "three edits inside a group must produce one history entry, not {}",
        after - before
    );

    doc.undo().expect("undo the group");
    let value = doc.eval_points(None, &[[0.0, 0.0, 0.0]]).expect("evaluate")[0];
    assert!(value > 0.0, "undoing the group left geometry behind");
}

#[test]
fn undo_state_reports_what_is_available() {
    let mut doc = Document::new().expect("document");
    let fresh = doc.undo_state().expect("state");
    assert!(!fresh.enabled, "undo is opt-in and starts off");

    doc.enable_undo().expect("enable");
    assert!(doc.undo_state().expect("state").enabled);
    assert!(
        !doc.undo().expect("undo on an empty stack"),
        "nothing to undo yet"
    );
}

// -- voxels -----------------------------------------------------------------

#[test]
fn a_voxel_layer_lends_a_grid_that_the_document_owns() {
    let mut doc = Document::new().expect("document");
    let (_layer, mut grid) = doc.add_voxel_layer("Voxels", 0.1).expect("add voxel layer");

    let index = grid.palette_add([0.8, 0.5, 0.3]).expect("palette");
    grid.fill_box([0, 0, 0], [4, 4, 4], index).expect("fill");

    assert_eq!(grid.occupied_count().expect("occupied"), 125);
    assert_eq!(grid.get([2, 2, 2]).expect("get"), Some(index));
    assert_eq!(grid.get([9, 9, 9]).expect("get"), None);
}

#[test]
fn sculpt_verbs_change_cells_and_report_it() {
    let mut grid = VoxelGrid::new(0.1).expect("grid");
    let index = grid.palette_add([1.0, 1.0, 1.0]).expect("palette");
    grid.fill_box([0, 0, 0], [8, 8, 8], index).expect("fill");

    let brush = BrushParams {
        size: 5,
        shape: BrushShape::Sphere,
        falloff: Falloff::Constant,
        strength: 1.0,
        ..Default::default()
    };

    // At the block's corner half the footprint is empty, so dilation has
    // somewhere to go. At the centre of a solid block it would correctly do
    // nothing, which is the no-op case the next test covers.
    let before = grid.change_count().expect("change count");
    grid.sculpt_inflate([8, 8, 8], &brush, 1).expect("inflate");
    let after = grid.change_count().expect("change count");

    assert!(
        after > before,
        "inflate at the block's surface should change cells: {before} -> {after}"
    );

    // And the interior really is a no-op: a fully occupied footprint has
    // nothing to dilate into.
    let settled = grid.change_count().expect("change count");
    grid.sculpt_inflate([4, 4, 4], &brush, 1)
        .expect("inflate the interior");
    assert_eq!(
        grid.change_count().expect("change count"),
        settled,
        "dilating a fully occupied footprint should change nothing"
    );
}

#[test]
fn a_verb_that_changes_nothing_is_not_an_error() {
    let mut grid = VoxelGrid::new(0.1).expect("grid");
    let index = grid.palette_add([1.0, 1.0, 1.0]).expect("palette");
    grid.fill_box([0, 0, 0], [4, 4, 4], index).expect("fill");

    let brush = BrushParams {
        size: 3,
        ..Default::default()
    };
    let before = grid.change_count().expect("change count");

    // A footprint far from any material. The engine reports success; the
    // change count is what says nothing happened.
    grid.sculpt_smooth([500, 500, 500], &brush)
        .expect("smooth over empty space");

    assert_eq!(
        grid.change_count().expect("change count"),
        before,
        "a footprint over empty space must not change any cell"
    );
}

#[test]
fn a_sub_cell_grab_moves_nothing() {
    let mut grid = VoxelGrid::new(1.0).expect("grid");
    let index = grid.palette_add([1.0, 1.0, 1.0]).expect("palette");
    grid.fill_box([0, 0, 0], [4, 4, 4], index).expect("fill");

    let brush = BrushParams {
        size: 6,
        ..Default::default()
    };
    let before = grid.change_count().expect("change count");

    // Under half a cell on every axis: rounding is per axis, so this is dead.
    grid.sculpt_grab([2, 2, 2], &brush, [0.1, 0.1, 0.1], false)
        .expect("grab");

    assert_eq!(
        grid.change_count().expect("change count"),
        before,
        "a displacement under half a cell on every axis must move nothing — \
         this is why a drag needs accumulating before it is handed over"
    );
}

#[test]
fn resolution_levels_stack_without_re_authoring() {
    let mut grid = VoxelGrid::new(0.2).expect("grid");
    let index = grid.palette_add([1.0, 1.0, 1.0]).expect("palette");
    grid.fill_box([0, 0, 0], [4, 4, 4], index).expect("fill");

    assert_eq!(grid.level_count().expect("levels"), 1);
    let coarse_occupied = grid.occupied_count().expect("occupied");

    let level = grid.add_level().expect("add level");
    assert_eq!(grid.level_count().expect("levels"), 2);
    grid.set_active_level(level).expect("activate finer level");
    assert_eq!(grid.active_level().expect("active"), level);

    let fine = grid.level_voxel_size(level).expect("fine size");
    let coarse = grid.level_voxel_size(0).expect("coarse size");
    assert!(
        fine < coarse,
        "the added level must be finer: {fine} vs {coarse}"
    );

    grid.set_active_level(0).expect("back to coarse");
    assert_eq!(
        grid.occupied_count().expect("occupied"),
        coarse_occupied,
        "adding a finer level must not disturb the coarse one"
    );
}

#[test]
fn repair_reports_an_enclosed_void() {
    let mut grid = VoxelGrid::new(0.1).expect("grid");
    let index = grid.palette_add([1.0, 1.0, 1.0]).expect("palette");
    grid.fill_box([0, 0, 0], [6, 6, 6], index).expect("shell");
    grid.fill_box([2, 2, 2], [4, 4, 4], -1).ok();
    // Carve the interior out cell by cell so the void is genuinely sealed.
    for x in 2..=4 {
        for y in 2..=4 {
            for z in 2..=4 {
                grid.erase([x, y, z]).expect("erase interior");
            }
        }
    }

    let report = grid.repair_report().expect("repair report");
    assert!(
        report.enclosed_voids > 0 && !report.airtight,
        "a hollowed block has a sealed void: {report:?}"
    );

    grid.repair_fill_voids(None).expect("fill voids");
    let after = grid.repair_report().expect("repair report");
    assert!(
        after.airtight,
        "filling voids should leave it airtight: {after:?}"
    );
}

// -- masks ------------------------------------------------------------------

#[test]
fn a_mask_freezes_what_it_covers() {
    let mut grid = VoxelGrid::new(0.1).expect("grid");
    let index = grid.palette_add([1.0, 1.0, 1.0]).expect("palette");
    grid.fill_box([0, 0, 0], [8, 8, 8], index).expect("fill");

    let mut mask = Mask::new(0.1).expect("mask");
    // Freeze the whole region the brush will cover.
    mask.fill([-10.0, -10.0, -10.0], [10.0, 10.0, 10.0], 1.0)
        .expect("fill mask");
    assert!(
        !mask.is_empty().expect("empty"),
        "the mask should be painted"
    );

    let brush = BrushParams {
        size: 6,
        mask: Some(&mask),
        ..Default::default()
    };

    let before = grid.change_count().expect("change count");
    grid.erase_brush([4, 4, 4], &brush)
        .expect("erase under a full mask");
    assert_eq!(
        grid.change_count().expect("change count"),
        before,
        "a fully masked region must be untouched by every verb"
    );
}

#[test]
fn mask_edits_compose() {
    let mut mask = Mask::new(0.1).expect("mask");
    assert!(
        mask.is_empty().expect("empty"),
        "a fresh mask covers nothing"
    );

    mask.fill([0.0, 0.0, 0.0], [0.5, 0.5, 0.5], 1.0)
        .expect("fill");
    let painted = mask.painted_count().expect("painted");
    assert!(painted > 0);

    mask.expand(1).expect("expand");
    assert!(
        mask.painted_count().expect("painted") > painted,
        "expanding should cover more cells"
    );

    mask.clear().expect("clear");
    assert!(
        mask.is_empty().expect("empty"),
        "clear should unmask everything"
    );
}

#[test]
fn a_mask_is_painted_along_a_stroke_like_any_brush() {
    let mut mask = Mask::new(0.05).expect("mask");
    let preset = StrokePreset {
        radius: 0.2,
        spacing: 0.25,
        ..Default::default()
    };
    let samples: Vec<_> = (0..10)
        .map(|i| StrokeSample::at([i as f32 * 0.05, 0.0, 0.0], i as f32 * 0.01))
        .collect();

    let applied = mask
        .apply_stroke(&samples, &preset, 1.0, BrushShape::Sphere, Falloff::Smooth)
        .expect("paint the mask along a stroke");

    assert!(applied > 0, "the stroke deposited no stamps");
    assert!(
        !mask.is_empty().expect("empty"),
        "painting along a stroke should mask something"
    );
}

// -- strokes on a layer -----------------------------------------------------

#[test]
fn a_stroke_resolves_into_ordinary_edits() {
    let mut doc = Document::new().expect("document");
    doc.enable_undo().expect("enable undo");
    let layer = doc.add_sdf_layer("Base").expect("layer");

    let stamp = Item::sphere(0.15).expect("stamp");
    let preset = StrokePreset {
        radius: 0.15,
        spacing: 0.5,
        ..Default::default()
    };
    let samples: Vec<_> = (0..12)
        .map(|i| StrokeSample::at([i as f32 * 0.1 - 0.6, 0.0, 0.0], i as f32 * 0.016))
        .collect();

    let nodes = doc
        .apply_stroke(layer, &samples, &preset, &stamp, None)
        .expect("apply stroke");

    assert!(
        !nodes.is_empty(),
        "a stroke over 1.2 units deposited no stamps"
    );

    // The stroke ran along x, so the axis should now be inside the surface.
    let value = doc.eval_points(None, &[[0.0, 0.0, 0.0]]).expect("evaluate")[0];
    assert!(
        value < 0.0,
        "the stroke left nothing on its own path: {value}"
    );
}

#[test]
fn spacing_follows_arc_length_not_sample_count() {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    let stamp = Item::sphere(0.1).expect("stamp");
    let preset = StrokePreset {
        radius: 0.1,
        spacing: 0.5,
        ..Default::default()
    };

    // The same path, sampled coarsely and finely. Arc-length spacing means the
    // stamp count should be driven by the distance, not the sample count.
    let coarse: Vec<_> = (0..5)
        .map(|i| StrokeSample::at([i as f32 * 0.25, 0.0, 0.0], i as f32 * 0.05))
        .collect();
    let fine: Vec<_> = (0..41)
        .map(|i| StrokeSample::at([i as f32 * 0.025, 0.0, 0.0], i as f32 * 0.005))
        .collect();

    let a = doc
        .apply_stroke(layer, &coarse, &preset, &stamp, None)
        .expect("coarse");
    let b = doc
        .apply_stroke(layer, &fine, &preset, &stamp, None)
        .expect("fine");

    let (a, b) = (a.len() as i32, b.len() as i32);
    assert!(
        (a - b).abs() <= 2,
        "eight times the samples over the same path produced {a} vs {b} stamps, \
         so spacing is following sample count rather than arc length"
    );
}

#[test]
fn the_move_brush_drags_the_assembled_surface() {
    let (mut doc, layer) = sphere_doc();

    let applied = doc
        .move_surface(
            layer,
            [0.0, 1.0, 0.0],
            [0.0, 0.3, 0.0],
            claycore::MoveParams {
                radius: 0.6,
                ease: 0,
                front_only: false,
            },
        )
        .expect("move surface");

    assert!(applied > 0, "the Move brush warped no items");

    // The top of the sphere was at y = 1; dragging it up should leave the
    // point just above it inside the surface.
    let value = doc.eval_points(None, &[[0.0, 1.1, 0.0]]).expect("evaluate")[0];
    assert!(
        value < 0.0,
        "the surface was not dragged upward: {value} at y = 1.1"
    );
}

#[test]
fn a_move_can_be_previewed_without_applying_it() {
    let (doc, layer) = sphere_doc();
    let probe = [[0.0f32, 1.1, 0.0]];
    let before = doc.eval_points(None, &probe).expect("evaluate")[0];

    let nodes = doc
        .move_surface_preview(
            layer,
            [0.0, 1.0, 0.0],
            [0.0, 0.3, 0.0],
            claycore::MoveParams {
                radius: 0.6,
                ease: 0,
                front_only: false,
            },
            16,
        )
        .expect("preview");

    assert!(!nodes.is_empty(), "the preview named no affected nodes");
    assert_eq!(
        doc.eval_points(None, &probe).expect("evaluate")[0],
        before,
        "previewing a move must not change the document"
    );
}

// -- concurrent reads -------------------------------------------------------

#[test]
fn several_threads_read_one_document_and_agree() {
    let (doc, _layer) = sphere_doc();
    let points: Vec<[f32; 3]> = (0..64)
        .map(|i| {
            let t = i as f32 / 64.0 * std::f32::consts::TAU;
            [t.cos() * 1.5, t.sin() * 1.5, 0.0]
        })
        .collect();

    let expected = doc
        .eval_points(None, &points)
        .expect("single-threaded reference");
    let reader = doc.reader();

    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    reader
                        .eval_points(None, &points)
                        .expect("concurrent evaluation")
                })
            })
            .collect();

        for handle in handles {
            assert_eq!(
                handle.join().expect("thread"),
                expected,
                "a concurrent reader disagreed with a single-threaded one"
            );
        }
    });
}

#[test]
fn a_reader_reports_the_safe_step_scale() {
    let (doc, _layer) = sphere_doc();
    let scale = doc.reader().safe_step_scale().expect("safe step scale");
    assert!(
        scale > 0.0 && scale <= 1.0,
        "the Lipschitz safety factor should be in (0, 1], got {scale}"
    );
}
