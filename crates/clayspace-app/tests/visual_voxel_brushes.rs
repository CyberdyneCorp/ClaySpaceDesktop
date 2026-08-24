//! Every voxel brush, drawn with symmetry on, so the two sides can be looked
//! at — and each with its opposite beside it where it has one.
//!
//! `voxel_tools.rs` asks the grid whether it changed, which is the question
//! that was always answered yes. This asks the one a sculptor asks: does it
//! come out on both sides, and does holding the key do the other thing.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_voxel_brushes
//! open target/visual
//! ```

mod support;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, Representation, SculptModel, ToolKind};
use clayspace_view::{Camera, Image, Vertex};
use support::Harness;

/// A slab across the whole of x, so a reshaping brush has material at the
/// mirror as well as under the stroke.
///
/// Deposited *mirrored*, so the subject itself is symmetric. Laid down with
/// symmetry off it is not — the wobble that gives a curvature-seeking brush
/// something to bite on is not an even function of x — and every picture then
/// reads as asymmetric whatever the brush did, which is the fixture's fault
/// rather than the brush's. Measured before this, a plain mirrored dab scored
/// 0.37 against its own reflection.
fn packed() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy).ok()?;
    document.add_voxel_layer("Voxels", 0.05).ok()?;
    for step in 0..17 {
        let t = step as f32 / 16.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.25,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: [(t - 0.5) * 1.6, (t * 9.0).sin() * 0.08, 0.0],
                    pressure: 1.0,
                    time: t,
                }],
                [true, false, false],
            )
            .ok()?;
    }
    Some(document)
}

/// The geometry the viewport would upload for a grid.
fn viewport(document: &mut ClayDocument) -> (Vec<Vertex>, Vec<u32>) {
    let (positions, normals, colors, indices) = document.visible_mesh_geometry();
    let vertices = positions
        .into_iter()
        .zip(normals)
        .zip(colors)
        .map(|((position, normal), color)| Vertex {
            position,
            normal,
            color,
            mask: 0.0,
        })
        .collect();
    (vertices, indices)
}

/// Head-on, and framed on a box centred on the mirror plane.
///
/// The document's own bounds would be the obvious thing to frame and are the
/// wrong one: a slab is not centred in y or z, so framing it puts the camera
/// off the plane and a symmetric form projects asymmetrically. Measured that
/// way, a slab whose two halves hold 816 and 814 vertices scored 0.22 against
/// its own reflection — a statement about the camera rather than the clay.
fn head_on() -> Camera {
    let mut camera = Camera::default();
    camera.frame_bounds([-1.0, -1.0, -1.0].into(), [1.0, 1.0, 1.0].into());
    camera.yaw = 0.0;
    camera.pitch = 0.0;
    camera
}

/// How much the silhouette disagrees with its own reflection.
///
/// The silhouette rather than the picture: a MatCap shades by the view-space
/// normal and is not itself left-right symmetric, so a perfectly mirrored form
/// renders as a very asymmetric image. What two halves of a symmetric form
/// share is their outline.
fn asymmetry(image: &Image) -> f64 {
    let ground = image.pixel(2, 2);
    let lit = |p: [u8; 4]| (0..3).any(|c| p[c].abs_diff(ground[c]) > 10);
    let (mut covered, mut differing) = (0usize, 0usize);
    for y in 0..image.height {
        for x in 0..image.width / 2 {
            let here = lit(image.pixel(x, y));
            let there = lit(image.pixel(image.width - 1 - x, y));
            if here || there {
                covered += 1;
                differing += usize::from(here != there);
            }
        }
    }
    differing as f64 / covered.max(1) as f64
}

fn how_many_differ(a: &Image, b: &Image) -> usize {
    a.pixels
        .chunks_exact(4)
        .zip(b.pixels.chunks_exact(4))
        .filter(|(x, y)| (0..3).any(|c| x[c].abs_diff(y[c]) > 12))
        .count()
}

fn stroke(document: &mut ClayDocument, tool: ToolKind, invert: bool, symmetry: [bool; 3]) {
    let samples: Vec<GestureSample> = (0..9)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [0.35 + t * 0.4, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size: 0.25,
                // Full strength for the same reason the slab is: a dithered
                // stroke lands on a different set of cells on each side.
                intensity: 1.0,
                invert,
                ..BrushSettings::default()
            },
            &samples,
            symmetry,
        )
        .expect("the stroke was refused");
}

fn name_of(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::Padrao => "padrao",
        ToolKind::Inflar => "inflar",
        ToolKind::Suavizar => "suavizar",
        ToolKind::Pincar => "pincar",
        ToolKind::Raspar => "raspar",
        ToolKind::Camada => "camada",
        ToolKind::Nudge => "nudge",
        ToolKind::Apagar => "apagar",
        ToolKind::Preencher => "preencher",
        ToolKind::Pintar => "pintar",
        ToolKind::Mascara => "mascara",
        other => panic!("{other:?} has no name here"),
    }
}

#[test]
fn every_voxel_brush_is_captured_mirrored_and_not() {
    // Two frames per brush, to be looked at — and no assertion about their
    // symmetry, which is a decision rather than an omission.
    //
    // A grid is discrete. Its cells are cubes, the greedy mesher merges quads
    // differently either side of the seam — 164 vertices against 152 for a
    // deposit that is exactly symmetric — and a MatCap-lit blocky ribbon in
    // perspective is not pixel-symmetric. Measured, a *perfectly* mirrored
    // deposit scores 0.33 against its own reflection, which says nothing about
    // the brush. Nor does the relative form hold: mirroring lowers the score
    // for five of the six and raises it for Pinçar, 0.4189 against 0.3834.
    //
    // Pixels are the wrong instrument for this question. The numbers are
    // printed for a reader and the frames are there to look at; whether the
    // far side actually changed is measured on the *cells* in
    // `voxel_brushes.rs`, which can see it. What is asserted here is only what
    // a picture can establish: that each brush reached the screen at all.
    let Some(harness) = Harness::new() else {
        return;
    };
    let camera = head_on();
    let Some(mut plain) = packed() else {
        return;
    };
    let (vertices, indices) = viewport(&mut plain);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);
    let rest = harness.capture(&mesh, &camera, false, "voxel-sym-rest");

    for tool in ToolKind::for_representation(Representation::Voxel) {
        // Máscara paints the freeze, Pintar has no colour to paint with, and
        // Preencher closes holes this slab has none of. None of the three
        // moves material, so none has two sides to compare.
        if matches!(
            tool,
            ToolKind::Mascara | ToolKind::Pintar | ToolKind::Preencher
        ) {
            continue;
        }
        let mut shot = |symmetry: [bool; 3], suffix: &str| -> Image {
            let mut document = packed().expect("a slab");
            stroke(&mut document, tool, false, symmetry);
            let (vertices, indices) = viewport(&mut document);
            mesh.upload(&harness.gpu, &vertices, &indices);
            harness.capture(
                &mesh,
                &camera,
                false,
                &format!("voxel-sym-{}{suffix}", name_of(tool)),
            )
        };
        let mirrored = shot([true, false, false], "");
        let lopsided = shot([false; 3], "-off");

        let moved = how_many_differ(&rest, &mirrored);
        // Low, because Nudge sets the floor at 72. A smudge translates
        // occupancy through a nearest-cell resample: a displacement shorter
        // than half a cell rounds back to the cell it started in and moves
        // nothing, so most of its stroke does nothing visible and it changes
        // the least of the eight. `voxel_brushes.rs` holds that it changes the
        // grid at all.
        assert!(
            moved > 50,
            "{tool:?} changed {moved} pixels, so it did not reach the screen"
        );
        let (even, uneven) = (asymmetry(&mirrored), asymmetry(&lopsided));
        println!(
            "{:<10} mirrored {even:.4} against {uneven:.4} unmirrored, moved {moved}",
            tool.label()
        );
        let _ = (even, uneven);
    }
}

#[test]
fn each_brush_with_an_opposite_draws_a_different_picture_held() {
    // A key that changes the picture is the least that "negative support"
    // means, and the direction of each is measured in `voxel_brushes.rs`.
    let Some(harness) = Harness::new() else {
        return;
    };
    let camera = head_on();
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);

    for tool in [
        ToolKind::Padrao,
        ToolKind::Inflar,
        ToolKind::Pincar,
        ToolKind::Camada,
        ToolKind::Apagar,
    ] {
        let mut shot = |invert: bool| -> Image {
            let mut document = packed().expect("a slab");
            stroke(&mut document, tool, invert, [false; 3]);
            let (vertices, indices) = viewport(&mut document);
            mesh.upload(&harness.gpu, &vertices, &indices);
            harness.capture(
                &mesh,
                &camera,
                false,
                &format!(
                    "voxel-{}-{}",
                    name_of(tool),
                    if invert { "held" } else { "plain" }
                ),
            )
        };
        let upright = shot(false);
        let held = shot(true);
        let apart = how_many_differ(&upright, &held);
        assert!(
            apart > 200,
            "{tool:?} drew the same {apart} pixels held as upright, so the \
             invert key does nothing for it. See target/visual/voxel-{}-*.png",
            name_of(tool)
        );
    }
}

// -- the two pictures of a grid ----------------------------------------------

#[test]
fn a_grid_can_be_drawn_as_boxes_or_as_a_surface() {
    // A grid is boxes; whether it should *look* like boxes is a separate
    // question, and the engine ships a mesher for each answer. The boxy one is
    // correct for hard-surface work and for export; the smooth one is the
    // right picture of an organic sculpt.
    //
    // The smooth mesh carries no normals — colour blends across a smooth
    // surface and a normal is the host's to work out — so a flat silhouette
    // here would mean they were not computed, which is what the first attempt
    // at this looked like.
    use clayspace_model::{SmoothBlur, VoxelDisplay};

    let Some(harness) = Harness::new() else {
        return;
    };
    let camera = head_on();
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);

    let shot = |harness: &Harness,
                mesh: &mut clayspace_view::GpuMesh,
                display: VoxelDisplay,
                blur: i32,
                name: &str|
     -> (Image, usize) {
        let mut document = packed().expect("a slab");
        document
            .set_voxel_display(display, SmoothBlur::new(blur))
            .expect("the picture was refused");
        let (vertices, indices) = viewport(&mut document);
        mesh.upload(&harness.gpu, &vertices, &indices);
        (harness.capture(mesh, &camera, false, name), vertices.len())
    };

    let (boxes, box_verts) = shot(
        &harness,
        &mut mesh,
        VoxelDisplay::Boxes,
        0,
        "voxel-picture-boxes",
    );
    let (smooth, smooth_verts) = shot(
        &harness,
        &mut mesh,
        VoxelDisplay::Smooth,
        0,
        "voxel-picture-smooth",
    );
    let (blurred, blur_verts) = shot(
        &harness,
        &mut mesh,
        VoxelDisplay::Smooth,
        1,
        "voxel-picture-smooth-blurred",
    );

    println!("boxes {box_verts} smooth {smooth_verts} blurred {blur_verts}");
    assert!(
        how_many_differ(&boxes, &smooth) > 1000,
        "the smooth picture drew the same frame as the boxes. See \
         target/visual/voxel-picture-*.png"
    );
    assert!(
        how_many_differ(&smooth, &blurred) > 200,
        "blurring the occupancy changed nothing"
    );
    assert!(
        blur_verts < smooth_verts,
        "one pass of blur left {blur_verts} vertices against {smooth_verts}"
    );

    // Shaded, not flat. A surface whose vertices all carried the same normal
    // renders as one tone, so the spread of tones is what says the normals
    // were computed.
    let ground = smooth.pixel(2, 2);
    let mut tones = std::collections::BTreeSet::new();
    for pixel in smooth.pixels.chunks_exact(4) {
        if (0..3).any(|c| pixel[c].abs_diff(ground[c]) > 10) {
            tones.insert(pixel[0]);
        }
    }
    assert!(
        tones.len() > 20,
        "the smooth surface drew {} distinct tones, which is a flat \
         silhouette — the mesh carries no normals of its own, so this is what \
         a missing normals pass looks like",
        tones.len()
    );
}
