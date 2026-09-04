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
        .apply_stroke(layer, &samples, &preset, &stamp, claycore::MaskSource::None)
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
        .apply_stroke(layer, &coarse, &preset, &stamp, claycore::MaskSource::None)
        .expect("coarse");
    let b = doc
        .apply_stroke(layer, &fine, &preset, &stamp, claycore::MaskSource::None)
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

/// Sculpting a mesh moves its vertices and never its topology.
///
/// The line every one of the sixteen verbs holds above everything else, and the
/// reason a mesh layer is worth sculpting at all: a model that has just been
/// retopologized can be refined without spending the retopology.
mod mesh_sculpting {
    use claycore::{MeshBrush, MeshSculptor, MeshStamp};

    /// A mesh with enough vertices for a brush to reach several of them.
    fn sphere_mesh() -> claycore::Mesh {
        let (doc, _) = super::sphere_doc();
        doc.mesh(claycore::MeshParams::default()).expect("mesh it")
    }

    #[test]
    fn a_stamp_moves_vertices_and_leaves_the_indices_alone() {
        let mut mesh = sphere_mesh();
        let before_indices = mesh.indices().to_vec();
        let before_positions = mesh.positions().to_vec();
        assert!(!before_indices.is_empty(), "the fixture meshed nothing");

        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        let moved = sculptor
            .stamp(
                MeshStamp {
                    verb: MeshBrush::Draw,
                    center: [0.0, 0.0, 1.0],
                    radius: 0.5,
                    strength: 0.4,
                    ..MeshStamp::default()
                },
                None,
                None,
            )
            .expect("stamp");
        assert!(moved > 0, "the stamp reached nothing to move");

        assert_eq!(
            mesh.indices(),
            before_indices.as_slice(),
            "sculpting changed the topology, which is the one thing these \
             verbs may never do"
        );
        assert_ne!(
            mesh.positions(),
            before_positions.as_slice(),
            "the stamp reported vertices moved and none did"
        );
    }

    /// The adjacency is the whole reason a sculptor is a stateful object, and
    /// a welded count below the vertex count is how a host learns its import
    /// was split at a seam.
    #[test]
    fn welded_classes_are_reported_beside_the_vertex_count() {
        let mut mesh = sphere_mesh();
        let sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        let vertices = sculptor.vertex_count().expect("vertices");
        let classes = sculptor.class_count().expect("classes");
        assert!(vertices > 0);
        assert!(
            classes <= vertices,
            "there cannot be more welded classes than vertices"
        );
    }

    /// Every verb has to be expressible; a name in the enum that the engine
    /// refuses is a tool the interface would offer and could not run.
    #[test]
    fn every_verb_is_accepted_by_the_engine() {
        let mut mesh = sphere_mesh();
        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        for verb in MeshBrush::ALL {
            if verb.writes_colour() {
                // Refused on a mesh with no colour attribute, deliberately —
                // twelve bytes a vertex is not a cost to hide behind a stroke.
                continue;
            }
            sculptor
                .stamp(
                    MeshStamp {
                        verb,
                        center: [0.0, 0.0, 1.0],
                        radius: 0.4,
                        strength: 0.2,
                        direction: [0.0, 0.02, 0.0],
                        ..MeshStamp::default()
                    },
                    None,
                    None,
                )
                .unwrap_or_else(|e| panic!("{verb:?} was refused: {e}"));
        }
    }

    /// What the token buys, in one arrangement: a seed picked on the near
    /// face of the sphere, handed to a stamp centred on the far face.
    ///
    /// The seed is farther from the centre than the radius, so the surface
    /// walk answers an *empty* region and the dab is lost whole — and "nothing
    /// moved" reads exactly like a fully masked stroke. That is the failure,
    /// and it is silent. A stale token turns it into a rejection and a scan:
    /// one stamp slower, and the dab lands.
    ///
    /// The three tests below send this dab the same wrong class three times,
    /// claimed three different ways, and get three different outcomes.
    fn far_side_dab(sculptor: &mut MeshSculptor, seed: Option<claycore::MeshSeed>) -> usize {
        sculptor
            .stamp(
                MeshStamp {
                    verb: MeshBrush::Draw,
                    center: [0.0, 0.0, 1.0],
                    radius: 0.5,
                    strength: 0.2,
                    seed,
                    ..MeshStamp::default()
                },
                None,
                None,
            )
            .expect("stamp")
    }

    /// A class picked on the near face, and the token it was picked in.
    fn near_face_seed(mesh: &mut claycore::Mesh) -> claycore::MeshSeed {
        let mut picked = MeshSculptor::new(mesh, 1e-5).expect("the sculptor that picked");
        let hit = picked
            .raycast([0.0, 0.0, -5.0], [0.0, 0.0, 1.0])
            .expect("raycast")
            .expect("the ray missed the fixture");
        assert_eq!(
            hit.seed_revision,
            picked.seed_revision().expect("the sculptor's own token"),
            "a hit's token has to be the token of the sculptor that answered it, or \
             the two halves cannot be checked against each other at all"
        );
        hit.seed()
    }

    #[test]
    fn a_seed_from_a_retired_class_space_is_rejected_and_the_stamp_scans() {
        let mut mesh = sphere_mesh();
        let stale = near_face_seed(&mut mesh);

        // A second sculptor over the same mesh renumbers its weld classes.
        // This is what a host reaches by evicting a cached sculptor, by
        // removing a layer, or by reconciling after an undo — none of which
        // the artist does on purpose and none of which the seed can see.
        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("the sculptor stamped on");
        assert_ne!(
            sculptor.seed_revision().expect("the new token"),
            stale.revision,
            "a second sculptor kept the first one's class space, so this test is no \
             longer holding a stale seed"
        );
        assert_eq!(
            sculptor.stale_seeds_rejected().expect("the counter"),
            0,
            "a sculptor that has stamped nothing has rejected nothing"
        );

        let moved = far_side_dab(&mut sculptor, Some(stale));

        assert_eq!(
            sculptor.stale_seeds_rejected().expect("the counter"),
            1,
            "the stale token was taken at face value; this counter is the only thing \
             that can tell a refused seed from one that was accepted and happened to \
             be harmless"
        );
        assert!(
            moved > 0,
            "the dab was lost whole. Rejecting a stale seed costs a scan, not the \
             stamp — that is the entire trade"
        );
    }

    /// The failure the token exists to end, still reachable on demand: a zero
    /// token claims nothing, so the engine keeps the bounds check it always
    /// had, the wrong class passes it, and the dab is spent on nothing.
    ///
    /// This is what every stamp in this crate did before the field crossed the
    /// ABI, and what one still does when its caller has not picked. It is
    /// asserted rather than merely allowed because it is the compatibility
    /// contract: zero must keep behaving exactly this way.
    #[test]
    fn a_seed_claiming_no_class_space_keeps_the_bounds_check_and_loses_the_dab() {
        let mut mesh = sphere_mesh();
        let wrong_class = near_face_seed(&mut mesh).class;

        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        let unclaimed = claycore::MeshSeed {
            class: wrong_class,
            revision: 0,
        };

        assert_eq!(
            far_side_dab(&mut sculptor, Some(unclaimed)),
            0,
            "the walk reached the far face from a seed on the near one, so this \
             fixture is no longer demonstrating an empty region"
        );
        assert_eq!(
            sculptor.stale_seeds_rejected().expect("the counter"),
            0,
            "a seed that claims no class space cannot be stale — there is nothing \
             to compare it against"
        );
    }

    /// And the other direction: a token from this sculptor's own class space
    /// is current, so it is not rejected — the seed is simply wrong, which the
    /// engine neither detects nor pretends to. Nor is a stamp that seeds
    /// nothing at all.
    #[test]
    fn a_current_token_and_no_seed_at_all_are_neither_of_them_stale() {
        let mut mesh = sphere_mesh();
        let wrong_class = near_face_seed(&mut mesh).class;

        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        let current = claycore::MeshSeed {
            class: wrong_class,
            revision: sculptor.seed_revision().expect("this sculptor's token"),
        };

        assert_eq!(
            far_side_dab(&mut sculptor, Some(current)),
            0,
            "a current token does not make a wrong seed right, and the engine does \
             not claim it does"
        );
        assert_eq!(
            sculptor.stale_seeds_rejected().expect("the counter"),
            0,
            "a token this sculptor issued itself was counted as stale"
        );

        assert!(
            far_side_dab(&mut sculptor, None) > 0,
            "an unseeded dab scans, and a scan finds the far face"
        );
        assert_eq!(
            sculptor.stale_seeds_rejected().expect("the counter"),
            0,
            "a stamp that seeds nothing has no token to be stale"
        );
    }

    /// The stamp's grain turns with the azimuth, and it is observable only
    /// where there is something to orient.
    ///
    /// The upstream lesson is why this uses a quarter turn and not zero: their
    /// own round-trip test could not catch the field being dropped, because
    /// every preset in their reference set had an azimuth of zero and a
    /// default round-trips to the default whether the schema knows the field
    /// or not. A test written at the default value tests nothing.
    ///
    /// There is no brush preset persistence in this workspace to carry an
    /// azimuth through — `clay_brush_preset_serialize` is unbound and nothing
    /// above this crate saves a brush — so the round trip that exists to be
    /// tested is the descriptor's: a quarter turn set here has to reach the
    /// engine and move vertices that an unturned stamp left alone.
    #[test]
    fn a_quarter_turn_of_the_grain_lands_a_directional_alpha_somewhere_else() {
        /// A hard edge down the middle of the stamp: half of it deposits
        /// nothing, half of it deposits everything. A round kernel has no
        /// orientation, so a ridge is what makes the grain visible at all.
        fn ridge() -> Vec<f32> {
            const SIDE: usize = 16;
            (0..SIDE * SIDE)
                .map(|i| if i % SIDE < SIDE / 2 { 0.0 } else { 1.0 })
                .collect()
        }

        fn stamped(azimuth: f32, alpha: Option<&[f32]>) -> Vec<[f32; 3]> {
            let mut mesh = sphere_mesh();
            let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
            let moved = sculptor
                .stamp(
                    MeshStamp {
                        verb: MeshBrush::Draw,
                        center: [0.0, 0.0, -1.0],
                        radius: 0.6,
                        strength: 0.5,
                        stamp_azimuth: azimuth,
                        alpha: alpha.map(|samples| claycore::AlphaStamp {
                            samples,
                            width: 16,
                            height: 16,
                            // The surface normal under the centre, and a rough
                            // world-X up: the frame the grain then turns in.
                            direction: [0.0; 3],
                            tangent: [1.0, 0.0, 0.0],
                            extent: 0.0,
                        }),
                        ..MeshStamp::default()
                    },
                    None,
                    None,
                )
                .expect("stamp");
            assert!(moved > 0, "the stamp at azimuth {azimuth} reached nothing");
            mesh.positions().to_vec()
        }

        let moved_apart = |a: &[[f32; 3]], b: &[[f32; 3]]| {
            a.iter()
                .zip(b)
                .filter(|(p, q)| (0..3).any(|i| (p[i] - q[i]).abs() > 1e-6))
                .count()
        };

        let ridge = ridge();
        let unturned = stamped(0.0, Some(&ridge));
        let quartered = stamped(std::f32::consts::FRAC_PI_2, Some(&ridge));
        assert!(
            moved_apart(&unturned, &quartered) > 0,
            "a quarter turn left the alpha exactly where it was, which is what a \
             dropped field looks like"
        );

        // And the other half of the contract: the grain orients the stamp's
        // in-plane axes, so a round brush with nothing in those axes has
        // nothing to turn.
        assert_eq!(
            moved_apart(
                &stamped(0.0, None),
                &stamped(std::f32::consts::FRAC_PI_2, None)
            ),
            0,
            "the azimuth moved a stamp that carries no directional kernel, so it is \
             doing something other than turning the grain"
        );
    }
}

/// Normals a gesture deferred, and the obligation that comes with deferring.
///
/// The switch trades *when* the recompute happens for nothing at all about the
/// result — but only for a caller that flushes, and only for one that flushes
/// into the record the stamps were noted into. Both halves are measured here
/// rather than taken on the header's word, because both are silent when they
/// are wrong: a mesh shaded from where its vertices used to be reads as a
/// lighting bug, and an undo that restores post-gesture shading reads as
/// nothing at all until the form is turned to the light.
mod deferred_normals {
    use claycore::{MeshBrush, MeshDeltas, MeshSculptor, MeshStamp};

    fn sphere_mesh() -> claycore::Mesh {
        let (doc, _) = super::sphere_doc();
        doc.mesh(claycore::MeshParams::default()).expect("mesh it")
    }

    fn normals(mesh: &claycore::Mesh) -> Vec<[f32; 3]> {
        mesh.normals()
            .expect("the fixture carries normals")
            .to_vec()
    }

    fn positions(mesh: &claycore::Mesh) -> Vec<[f32; 3]> {
        mesh.positions().to_vec()
    }

    /// The dab every test here stamps, so that "deferred" and "not" differ in
    /// exactly one thing.
    fn dab(center: [f32; 3]) -> MeshStamp<'static> {
        MeshStamp {
            verb: MeshBrush::Draw,
            center,
            radius: 0.5,
            strength: 0.4,
            ..MeshStamp::default()
        }
    }

    /// Three overlapping dabs down one side, which is the shape the flush's
    /// de-duplication is worth anything on: the same classes, three times.
    const PATH: [[f32; 3]; 3] = [[0.0, 0.0, 1.0], [0.2, 0.05, 0.98], [0.4, 0.1, 0.9]];

    fn stamp_path(sculptor: &mut MeshSculptor, deltas: Option<&mut MeshDeltas>) {
        // One record across the three, the way a gesture holds one.
        let mut deltas = deltas;
        for at in PATH {
            sculptor
                .stamp(dab(at), None, deltas.as_deref_mut())
                .expect("stamp");
        }
    }

    /// What the gesture looks like with nothing deferred — the reference every
    /// assertion below is against.
    fn stamped_per_dab() -> claycore::Mesh {
        let mut mesh = sphere_mesh();
        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        stamp_path(&mut sculptor, None);
        drop(sculptor);
        mesh
    }

    /// Deferring holds the shading back; the flush lets it go, and what it lets
    /// go is *exactly* what recomputing per dab would have written.
    #[test]
    fn a_deferred_gesture_shades_from_where_the_vertices_were_until_it_is_flushed() {
        let settled = stamped_per_dab();

        let mut mesh = sphere_mesh();
        let untouched = normals(&mesh);
        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        sculptor.set_defer_normals(true).expect("defer");
        assert!(
            sculptor.defer_normals().expect("read the flag back"),
            "the flag did not survive being set, so nothing below is deferred"
        );
        stamp_path(&mut sculptor, None);
        drop(sculptor);

        assert_eq!(
            positions(&mesh),
            positions(&settled),
            "deferring moved the vertices somewhere else, which is the one \
             thing it is not allowed to change"
        );
        assert_eq!(
            normals(&mesh),
            untouched,
            "a deferred dab recomputed a normal anyway; there is then nothing \
             for the flush to be the only thing that does"
        );
        assert_ne!(
            normals(&settled),
            untouched,
            "the reference gesture recomputed nothing either, so this fixture \
             cannot tell a deferral from a no-op"
        );

        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a second sculptor");
        sculptor.flush_normals(None).expect("flush");
        drop(sculptor);
        // A sculptor built after the stamps has nothing deferred, so the mesh
        // is still shaded from before them: the pending set belongs to the
        // handle that deferred it, and this is what says so.
        assert_eq!(
            normals(&mesh),
            untouched,
            "a flush on a handle that deferred nothing recomputed something"
        );
    }

    /// The same gesture, flushed by the handle that owes it.
    #[test]
    fn the_flush_writes_exactly_what_recomputing_per_dab_would_have() {
        let settled = stamped_per_dab();

        let mut mesh = sphere_mesh();
        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        sculptor.set_defer_normals(true).expect("defer");
        stamp_path(&mut sculptor, None);
        sculptor.flush_normals(None).expect("flush");
        drop(sculptor);

        assert_eq!(
            normals(&mesh),
            normals(&settled),
            "the flush wrote different normals from the per-dab recompute, so \
             deferring is not the same gesture done later"
        );
    }

    /// The record half of the contract, and the reason the flush takes one at
    /// all: an undo has to put the shading back as well as the vertices.
    #[test]
    fn a_gesture_flushed_into_its_own_record_reverts_bit_exactly() {
        let mut mesh = sphere_mesh();
        let before_positions = positions(&mesh);
        let before_normals = normals(&mesh);

        let mut deltas = MeshDeltas::new().expect("a record");
        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        sculptor.set_defer_normals(true).expect("defer");
        stamp_path(&mut sculptor, Some(&mut deltas));
        sculptor.flush_normals(Some(&mut deltas)).expect("flush");
        deltas.revert(&mut sculptor).expect("revert");
        drop(sculptor);

        assert_eq!(
            positions(&mesh),
            before_positions,
            "the record did not put the vertices back"
        );
        assert_eq!(
            normals(&mesh),
            before_normals,
            "the record put the vertices back and left the shading where the \
             gesture had moved it"
        );
    }

    /// And the failure that makes the record argument load-bearing rather than
    /// decorative: flushed into a *fresh* record, the gesture's own record has
    /// never seen the recomputed normals, so reverting it restores the
    /// vertices and leaves the shading the gesture wrote.
    ///
    /// Measured rather than reasoned about, because it is the exact mistake a
    /// host makes by reaching for whichever record is nearest.
    #[test]
    fn a_gesture_flushed_into_someone_elses_record_does_not_revert_its_shading() {
        let mut mesh = sphere_mesh();
        let before_normals = normals(&mesh);

        let mut deltas = MeshDeltas::new().expect("the gesture's record");
        let mut elsewhere = MeshDeltas::new().expect("a record that is not it");
        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        sculptor.set_defer_normals(true).expect("defer");
        stamp_path(&mut sculptor, Some(&mut deltas));
        sculptor.flush_normals(Some(&mut elsewhere)).expect("flush");
        deltas.revert(&mut sculptor).expect("revert");
        drop(sculptor);

        assert_ne!(
            normals(&mesh),
            before_normals,
            "flushing into the wrong record reverted correctly anyway, which \
             would mean the record argument does not matter — and every \
             caller here is written as though it does"
        );
    }

    /// The other switch, and the reason it is not this one: a resolved stroke
    /// carries its own deferral, ends it itself, and gives the flag back the
    /// way it found it — because there the library knows where the stroke
    /// ended.
    #[test]
    fn a_resolved_stroke_settles_its_own_deferral_and_leaves_the_flag_alone() {
        fn stroked(defer: bool) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
            let mut mesh = sphere_mesh();
            let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
            let samples: Vec<[f32; 5]> = PATH
                .iter()
                .enumerate()
                .map(|(i, at)| [at[0], at[1], at[2], 1.0, i as f32])
                .collect();
            let applied = sculptor
                .apply_stroke(
                    &samples,
                    &claycore::StrokePreset::default(),
                    dab(PATH[0]),
                    None,
                    defer,
                    None,
                )
                .expect("stroke");
            assert!(applied > 0, "the stroke resolved to no stamps at all");
            drop(sculptor);
            (positions(&mesh), normals(&mesh))
        }

        let (settled_positions, settled_normals) = stroked(false);
        let (deferred_positions, deferred_normals) = stroked(true);
        assert_eq!(deferred_positions, settled_positions);
        assert_eq!(
            deferred_normals, settled_normals,
            "a stroke told to defer left its normals behind, so it is not \
             flushing at the end of the stroke it drove"
        );

        // And the half a host would be caught by: the argument does not leave
        // the member flag turned on behind it, nor turn it off.
        let mut mesh = sphere_mesh();
        let mut sculptor = MeshSculptor::new(&mut mesh, 1e-5).expect("a sculptor");
        sculptor.set_defer_normals(true).expect("defer");
        let samples: Vec<[f32; 5]> = PATH
            .iter()
            .enumerate()
            .map(|(i, at)| [at[0], at[1], at[2], 1.0, i as f32])
            .collect();
        sculptor
            .apply_stroke(
                &samples,
                &claycore::StrokePreset::default(),
                dab(PATH[0]),
                None,
                true,
                None,
            )
            .expect("stroke");
        assert!(
            sculptor.defer_normals().expect("read the flag back"),
            "the stroke's own argument overwrote the sculptor's flag instead \
             of restoring it, so the two switches are not independent"
        );
    }
}
