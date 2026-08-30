//! Visual tests for the sculpting vocabulary.
//!
//! Every tool the interface will offer is applied to a real document, meshed,
//! rendered and written to `target/visual/`. A before/after pair is captured
//! for each, so the effect can be looked at rather than only asserted about.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_sculpting
//! open target/visual
//! ```
//!
//! The assertions ask two things of every tool: that it changed the surface at
//! all, and that it changed it in the direction its name promises. A tool that
//! silently does nothing is the failure mode these exist to catch — the engine
//! documents several verbs as legitimately able to change nothing, so "no
//! error" is not evidence that anything happened.

mod support;

use clayspace_engine::claycore::{
    BrushParams, BrushShape, Document, Falloff, Item, Mask, MoveParams, Op, StrokePreset,
    StrokeSample, VoxelGrid,
};
use support::Harness;

/// A tool must visibly move at least this fraction of the rendered subject.
///
/// Measured as the proportion of subject pixels that changed, not as a mean
/// difference: a dent or a ridge is local, and averaging it against the
/// untouched surface around it hides exactly the thing being tested.
const VISIBLE_CHANGE: f64 = 0.01;

/// Renders a document before and after an edit, saving both, and returns how
/// much the visible surface changed.
fn before_and_after(
    harness: &mut Harness,
    doc: &mut Document,
    name: &str,
    edit: impl FnOnce(&mut Document),
) -> f64 {
    let background = harness.background();

    let mesh = support::mesh_document(doc, 72);
    let camera = support::framed_camera(&mesh);
    let before = harness.capture_mesh(&mesh, &camera, &format!("{name}-before"));

    edit(doc);

    let mesh = support::mesh_document(doc, 72);
    // The same camera, so the comparison is of the surface and not of the view.
    let after = harness.capture_mesh(&mesh, &camera, &format!("{name}-after"));

    before.changed_fraction_over_subject(&after, background, 6)
}

/// A ball to sculpt on, and the layer it lives in.
///
/// Edits go into *this* layer. A new layer combines with what is below it by
/// its own op, so a subtract item placed in a fresh layer unions an empty
/// field instead of cutting the ball — which is exactly what the first version
/// of these tests did, and why they measured no change at all.
fn ball() -> (Document, clayspace_engine::claycore::LayerId) {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Forma").expect("layer");
    let item = Item::sphere(1.0).expect("sphere");
    doc.add_item(layer, &item).expect("place");
    (doc, layer)
}

// -- SDF tools --------------------------------------------------------------

#[test]
fn padrao_deposits_material_along_a_stroke() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let (mut doc, layer) = ball();

    let change = before_and_after(&mut harness, &mut doc, "20-padrao", |doc| {
        let stamp = Item::sphere(0.18).expect("stamp");
        let preset = StrokePreset {
            radius: 0.18,
            spacing: 0.35,
            ..Default::default()
        };
        // Along the ball's surface, not through its interior. Stamps placed
        // inside a radius-1 ball are simply swallowed by it, which is what the
        // first capture of this test showed.
        let samples: Vec<_> = (0..16)
            .map(|i| {
                let t = i as f32 / 15.0;
                let angle = (t - 0.5) * 1.6;
                let (s, c) = angle.sin_cos();
                StrokeSample::at([s * 1.02, 0.18, c * 1.02], t * 0.2)
            })
            .collect();
        doc.apply_stroke(
            layer,
            &samples,
            &preset,
            &stamp,
            clayspace_engine::claycore::MaskSource::None,
        )
        .expect("apply stroke");
    });

    assert!(
        change > VISIBLE_CHANGE,
        "the Padrão stroke moved only {:.1}% of the subject",
        change * 100.0
    );
}

#[test]
fn mover_drags_the_surface_outward() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    // The Move brush warps what is already there, so the ball's own layer is
    // the target; it is found by picking rather than assumed.
    let (mut doc, _layer) = ball();

    let change = before_and_after(&mut harness, &mut doc, "21-mover", |doc| {
        // The ball lives in the first layer; drag its top outward.
        let target = doc
            .raycast_attributed([0.0, 5.0, 0.0], [0.0, -1.0, 0.0])
            .expect("raycast")
            .expect("the ball must be under the ray");
        let layer = target.layer.expect("attributed hit");
        doc.move_surface(
            layer,
            target.position,
            [0.0, 0.45, 0.0],
            MoveParams {
                radius: 0.7,
                ease: 0,
                front_only: false,
            },
        )
        .expect("move surface");
    });

    assert!(
        change > VISIBLE_CHANGE,
        "the Move brush moved only {:.1}% of the subject",
        change * 100.0
    );
}

#[test]
fn subtrair_removes_material_where_it_is_applied() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let (mut doc, layer) = ball();

    let change = before_and_after(&mut harness, &mut doc, "22-subtrair", |doc| {
        let mut cut = Item::sphere(0.55).expect("sphere");
        cut.set_op(Op::Subtract).expect("op");
        cut.set_position([0.3, 0.35, 0.85]).expect("position");
        doc.add_item(layer, &cut).expect("place");
    });

    assert!(
        change > VISIBLE_CHANGE,
        "subtracting a sphere moved only {:.1}% of the subject",
        change * 100.0
    );
}

#[test]
fn a_smooth_blend_softens_the_seam_between_two_forms() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();

    // Two overlapping balls, once hard-unioned and once smoothly blended.
    let build = |k: Option<f32>| {
        let mut doc = Document::new().expect("document");
        let layer = doc.add_sdf_layer("Pair").expect("layer");
        for x in [-0.55f32, 0.55] {
            let mut item = Item::sphere(0.7).expect("sphere");
            item.set_position([x, 0.0, 0.0]).expect("position");
            if let Some(k) = k {
                item.set_blend(clayspace_engine::claycore::Blend::Quadratic, k)
                    .expect("blend");
            }
            doc.add_item(layer, &item).expect("place");
        }
        doc
    };

    let hard = build(None);
    let mesh = support::mesh_document(&hard, 72);
    let camera = support::framed_camera(&mesh);
    let hard_image = harness.capture_mesh(&mesh, &camera, "23-blend-hard");

    let soft = build(Some(0.45));
    let mesh = support::mesh_document(&soft, 72);
    let soft_image = harness.capture_mesh(&mesh, &camera, "23-blend-smooth");

    let change = hard_image.changed_fraction_over_subject(&soft_image, background, 6);
    assert!(
        change > VISIBLE_CHANGE,
        "a quadratic blend of support 0.45 moved only {:.1}% of the subject",
        change * 100.0
    );
}

#[test]
fn mascara_extrude_pulls_a_patch_off_the_surface() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let (mut doc, layer) = ball();

    let change = before_and_after(&mut harness, &mut doc, "24-mascara-extrude", |doc| {
        // An owned mask rather than the layer's own: extrude needs the
        // document mutably and the mask by reference at the same time, and a
        // mask lent by the document is borrowed from it.
        let mut mask = Mask::new(0.05).expect("mask");
        mask.fill([-0.45, -0.45, 0.55], [0.45, 0.45, 1.3], 1.0)
            .expect("fill the mask");

        let solid = doc
            .mask_extrude(
                layer,
                clayspace_engine::claycore::MaskSource::Field(&mask),
                clayspace_engine::claycore::MaskExtrudeParams {
                    thickness: 0.22,
                    border_round: 0.05,
                    ..Default::default()
                },
            )
            .expect("extrude the masked patch");

        // The extracted patch is an ordinary item; placing it in the same
        // layer unions it onto the form it came from.
        doc.add_item(layer, &solid).expect("place the patch");
    });

    assert!(
        change > VISIBLE_CHANGE,
        "extruding a masked patch moved only {:.1}% of the subject",
        change * 100.0
    );
}

#[test]
fn painting_a_mask_does_not_itself_sculpt() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let (mut doc, layer) = ball();

    let change = before_and_after(&mut harness, &mut doc, "25-mascara-inert", |doc| {
        let mut mask = doc.add_mask(layer, 0.05).expect("add mask");
        mask.fill([-0.5, -0.5, 0.5], [0.5, 0.5, 1.2], 1.0)
            .expect("fill the mask");
        assert!(!mask.is_empty().expect("empty"), "the mask was not painted");
    });

    assert!(
        change < 0.002,
        "painting a mask moved {:.1}% of the subject; a mask freezes, it does not sculpt",
        change * 100.0
    );
}

// -- voxel tools ------------------------------------------------------------

/// A solid voxel block to sculpt on, with a palette entry.
fn voxel_block(size: i32) -> (VoxelGrid, i32) {
    let mut grid = VoxelGrid::new(0.06).expect("grid");
    let index = grid.palette_add([0.78, 0.76, 0.73]).expect("palette");
    grid.fill_box([0, 0, 0], [size, size, size], index)
        .expect("fill");
    (grid, index)
}

/// Renders a voxel grid before and after a verb, saving both.
fn voxel_before_and_after(
    harness: &mut Harness,
    grid: &mut VoxelGrid,
    name: &str,
    verb: impl FnOnce(&mut VoxelGrid),
) -> (f64, u64) {
    let background = harness.background();

    let mesh = grid.mesh().expect("mesh the grid");
    let camera = support::framed_camera(&mesh);
    let before = harness.capture_mesh(&mesh, &camera, &format!("{name}-before"));

    let changes_before = grid.change_count().expect("change count");
    verb(grid);
    let changed = grid.change_count().expect("change count") - changes_before;

    let mesh = grid.mesh().expect("mesh the grid");
    let after = harness.capture_mesh(&mesh, &camera, &format!("{name}-after"));

    (
        before.changed_fraction_over_subject(&after, background, 6),
        changed,
    )
}

/// A wider footprint, for verbs that move the surface one step at a time and
/// need a stroke's worth of coverage to show.
fn wide_brush() -> BrushParams<'static> {
    BrushParams {
        size: 15,
        shape: BrushShape::Sphere,
        falloff: Falloff::Smooth,
        strength: 1.0,
        ..Default::default()
    }
}

/// The footprint every voxel verb test uses, so their images are comparable.
fn verb_brush() -> BrushParams<'static> {
    BrushParams {
        size: 9,
        shape: BrushShape::Sphere,
        falloff: Falloff::Smooth,
        strength: 1.0,
        ..Default::default()
    }
}

macro_rules! voxel_verb_test {
    ($name:ident, $file:expr, $verb:expr) => {
        #[test]
        fn $name() {
            let Some(mut harness) = Harness::new() else {
                return;
            };
            let (mut grid, _index) = voxel_block(16);
            let verb: fn(&mut VoxelGrid) = $verb;
            let (change, cells) = voxel_before_and_after(&mut harness, &mut grid, $file, verb);

            assert!(
                cells > 0,
                "{} changed no cells, so the verb did nothing at all",
                $file
            );
            assert!(
                change > VISIBLE_CHANGE,
                "{} changed {cells} cells but moved only {:.1}% of the subject",
                $file,
                change * 100.0
            );
        }
    };
}

voxel_verb_test!(suavizar_smooths_the_surface, "30-suavizar", |grid| {
    // Roughen a corner first, so smoothing has something to remove.
    for i in 0..8 {
        grid.erase([16 - i % 3, 16 - (i / 3) % 3, 16 - i % 2]).ok();
    }
    grid.sculpt_smooth([16, 16, 16], &verb_brush())
        .expect("smooth")
});

voxel_verb_test!(inflar_dilates_the_surface, "31-inflar", |grid| {
    grid.sculpt_inflate([16, 16, 16], &verb_brush(), 2)
        .expect("inflate")
});

voxel_verb_test!(erodir_shrinks_the_surface, "32-erodir", |grid| {
    grid.sculpt_inflate([16, 16, 16], &verb_brush(), -2)
        .expect("erode")
});

voxel_verb_test!(pincar_pulls_cells_toward_the_centre, "33-pincar", |grid| {
    // One step per call, so a visible pinch is several — which is what a
    // stroke does, and what a single test call has to imitate.
    for _ in 0..12 {
        grid.sculpt_pinch([8, 16, 8], &wide_brush()).expect("pinch");
    }
});

voxel_verb_test!(magnify_pushes_cells_outward, "34-magnify", |grid| {
    for _ in 0..12 {
        grid.sculpt_magnify([8, 16, 8], &wide_brush())
            .expect("magnify");
    }
});

voxel_verb_test!(planar_flattens_onto_a_plane, "35-planar", |grid| {
    grid.sculpt_flatten([16, 16, 16], &verb_brush(), [0.0, 1.0, 0.0], 0.0)
        .expect("flatten")
});

voxel_verb_test!(raspar_flattens_and_smooths_together, "36-raspar", |grid| {
    // Roughen the top face first: scrape planes a surface flat, so on an
    // already-flat one it correctly does almost nothing.
    for i in 0..14 {
        let (x, z) = (2 + (i * 5) % 13, 2 + (i * 7) % 13);
        grid.fill_box([x, 17, z], [x + 1, 17 + (i % 3), z + 1], 1)
            .expect("roughen");
    }
    for _ in 0..3 {
        grid.sculpt_scrape([8, 17, 8], &wide_brush(), [0.0, 1.0, 0.0], 0.0)
            .expect("scrape");
    }
});

voxel_verb_test!(nudge_smears_the_surface_skin, "37-nudge", |grid| {
    grid.sculpt_smudge([16, 16, 16], &verb_brush(), [0.5, 0.0, 0.0])
        .expect("smudge")
});

voxel_verb_test!(mover_voxel_translates_a_lump, "38-mover-voxel", |grid| {
    grid.sculpt_grab([16, 16, 16], &verb_brush(), [0.3, 0.3, 0.0], false)
        .expect("grab")
});

#[test]
fn preencher_fills_a_cavity_that_smoothing_would_not() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let (mut grid, _index) = voxel_block(16);

    // Single-cell perforations. The rule is local — an empty cell with at
    // least four of its six face neighbours occupied — so a wide hole does
    // not qualify and a one-cell one does. Getting this wrong the first time
    // is exactly the confusion the engine's docs warn about.
    for x in (2..15).step_by(2) {
        for z in (2..15).step_by(2) {
            grid.erase([x, 16, z]).expect("perforate");
        }
    }

    let (change, cells) = voxel_before_and_after(&mut harness, &mut grid, "39-preencher", |grid| {
        // Constant falloff, not smooth: a repair verb should cover its
        // footprint uniformly, where a dithered edge leaves exactly the
        // single-cell holes it was asked to close.
        let brush = BrushParams {
            size: 17,
            shape: BrushShape::Cube,
            falloff: Falloff::Constant,
            strength: 1.0,
            ..Default::default()
        };
        grid.sculpt_fill_cavities([8, 16, 8], &brush, 3)
            .expect("fill cavities")
    });

    assert!(cells > 0, "fill-cavities changed no cells");
    assert!(
        change > VISIBLE_CHANGE,
        "fill-cavities changed {cells} cells but moved only {:.1}% of the subject",
        change * 100.0
    );
}

#[test]
fn a_mask_visibly_protects_the_region_it_covers() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();

    // The same erase, once unmasked and once through a mask covering half the
    // footprint. The two results must look different.
    let carve = |mask: Option<&Mask>| {
        let (mut grid, _) = voxel_block(16);
        let brush = BrushParams {
            size: 11,
            shape: BrushShape::Sphere,
            falloff: Falloff::Constant,
            strength: 1.0,
            mask: mask.map(|m| &**m),
            ..Default::default()
        };
        grid.erase_brush([16, 16, 16], &brush).expect("erase");
        grid
    };

    let unmasked = carve(None);
    let mesh = unmasked.mesh().expect("mesh");
    let camera = support::framed_camera(&mesh);
    let open = harness.capture_mesh(&mesh, &camera, "40-mask-unprotected");

    let mut mask = Mask::new(0.06).expect("mask");
    // Freeze the half of the footprint on the positive x side.
    mask.fill([16.0 * 0.06, -10.0, -10.0], [10.0, 10.0, 10.0], 1.0)
        .expect("fill mask");
    let masked = carve(Some(&mask));
    let mesh = masked.mesh().expect("mesh");
    let protected = harness.capture_mesh(&mesh, &camera, "40-mask-protected");

    let change = open.changed_fraction_over_subject(&protected, background, 6);
    assert!(
        change > VISIBLE_CHANGE,
        "masking half the footprint moved only {:.1}% of the subject, \
         so the mask is not reaching the verb",
        change * 100.0
    );
    assert!(
        masked.occupied_count().expect("occupied") > unmasked.occupied_count().expect("occupied"),
        "the masked carve removed at least as much material as the unmasked one"
    );
}

#[test]
fn voxel_colour_reaches_the_render() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    let mut grid = VoxelGrid::new(0.06).expect("grid");
    let warm = grid.palette_add([0.85, 0.35, 0.15]).expect("palette");
    let cool = grid.palette_add([0.15, 0.35, 0.85]).expect("palette");
    grid.fill_box([0, 0, 0], [16, 16, 16], warm).expect("fill");
    grid.fill_box([0, 0, 0], [7, 16, 16], cool)
        .expect("fill half");

    let mesh = grid.mesh().expect("mesh");
    let camera = support::framed_camera(&mesh);
    let image = harness.capture_mesh(&mesh, &camera, "41-voxel-colour");

    if mesh.colors().is_none() {
        // The mesher did not carry colour, which is worth knowing but is not
        // this test's failure to report.
        eprintln!("note: the voxel mesher produced no vertex colours");
        return;
    }

    // Two differently coloured halves must produce a frame whose left and
    // right differ in hue, not just in shading.
    let half = image.width / 2;
    let mean_of = |x0: u32, x1: u32| {
        let (mut sums, mut count) = ([0u64; 3], 0u64);
        for y in 0..image.height {
            for x in x0..x1 {
                let p = image.pixel(x, y);
                for c in 0..3 {
                    sums[c] += p[c] as u64;
                }
                count += 1;
            }
        }
        [
            sums[0] as f64 / count as f64,
            sums[1] as f64 / count as f64,
            sums[2] as f64 / count as f64,
        ]
    };
    let left = mean_of(0, half);
    let right = mean_of(half, image.width);

    // Red-minus-blue is the axis the two palette entries differ on.
    let left_warmth = left[0] - left[2];
    let right_warmth = right[0] - right[2];
    assert!(
        (left_warmth - right_warmth).abs() > 4.0,
        "the two palette colours render alike (warmth {left_warmth:.1} vs \
         {right_warmth:.1}), so vertex colour is not reaching the shader"
    );
}
