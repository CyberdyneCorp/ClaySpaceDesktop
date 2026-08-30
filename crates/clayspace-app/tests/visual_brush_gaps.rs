//! The brushes bound in this change, drawn so they can be looked at.
//!
//! Every one of them was a verb the engine already had and the shelf did not
//! reach, so what these frames are for is the question the assertions cannot
//! settle: does it *look* like the tool it is named after. A crease that reads
//! as a gouge and a clay pat that reads as a ridge both pass a test that only
//! asks whether the surface moved.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_brush_gaps
//! open target/visual
//! ```

mod support;

use clayspace_app::geometry::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Colour, GestureSample, MaskModel, Representation, SculptModel, ToolKind,
};
use clayspace_view::{Camera, GpuMesh, Image};
use support::Harness;

/// The starting sphere, bare.
///
/// Bare on purpose. A ridge drawn on it first would give the planing tools
/// something to plane, and it also *hides* the marks these frames are for: a
/// crease cut into a raised bar reads as the bar, and measured that way Vinco
/// and the untouched form were 169 pixels apart on a 480-by-360 frame. The
/// tools shown here all put their own mark on clean clay.
fn field() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// A slab of clay across the mirror plane.
fn grid() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy).ok()?;
    document.add_voxel_layer("Voxels", 0.04).ok()?;
    for step in 0..21 {
        let t = step as f32 / 20.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    // A brush size is a footprint *span* in cells by the time
                    // it reaches the grid, so this is a tube about 0.5 thick
                    // rather than 1.0 — enough of a slab to read on screen.
                    size: 0.5,
                    intensity: 1.0,
                    // A hard edge, so the slab is solid. Every voxel verb
                    // dithers where its coverage is below full, so a smooth
                    // falloff leaves the rim of each dab perforated — which is
                    // right for a brush and wrong for a fixture, since the
                    // holes are what the frame ends up being about.
                    shaping: clayspace_model::Shaping {
                        falloff: clayspace_model::Falloff::Constant,
                        ..clayspace_model::Shaping::default()
                    },
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: [(t - 0.5) * 1.4, (t * 9.0).sin() * 0.1, 0.0],
                    pressure: 1.0,
                    time: t,
                }],
                [true, false, false],
            )
            .ok()?;
    }
    Some(document)
}

/// A stroke across the top of the field fixture.
fn across_the_field(document: &mut ClayDocument, tool: ToolKind, invert: bool) {
    let samples: Vec<GestureSample> = (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [(t - 0.5) * 0.6, 0.0, 1.02],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size: 0.18,
                intensity: 1.0,
                invert,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("the stroke was refused");
}

/// A drag across the slab, in +y so it is visible head-on.
fn drag_the_grid(document: &mut ClayDocument, tool: ToolKind) {
    let samples: Vec<GestureSample> = (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [0.35, t * 0.35, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document.begin_gesture();
    // Delivered whole, which is how the ViewModel delivers a drag on a grid:
    // `ToolKind::holds_the_whole_gesture` says why, and the short version is
    // that eight one-cell grabs are not one eight-cell grab.
    //
    // A wide brush, too. A grab carries the region it covers, and a brush
    // narrower than the slab moves solid material *inside* solid material,
    // which changes nothing anyone can see: measured on this fixture, a 0.3
    // brush moved not one vertex and a 1.0 brush moved four hundred.
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size: 1.0,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [true, false, false],
        )
        .expect("the drag was refused");
    document.end_gesture();
}

/// Head-on and close, framed on the slab rather than on a unit cube.
///
/// Framing the cube puts the camera four units back and draws the slab as a
/// smudge sixty pixels wide — measured, two paint strokes over it changed
/// twenty-two pixels, which says nothing about the paint.
fn head_on() -> Camera {
    let mut camera = Camera::default();
    camera.frame_bounds([-0.85, -0.4, -0.4].into(), [0.85, 0.4, 0.4].into());
    camera.yaw = 0.0;
    camera.pitch = 0.0;
    camera
}

/// Close on the cap of the sphere the field strokes are made across.
///
/// The world's up axis is y and the strokes are made at z = 1, so the mark
/// faces a camera at yaw and pitch zero. Tilted a little off that, because a
/// trough seen exactly face-on is a change in shading and nothing else, and
/// what these frames are for is looking at the shape.
fn on_the_cap() -> Camera {
    let mut camera = Camera::default();
    camera.frame_bounds([-0.45, -0.45, 0.55].into(), [0.45, 0.45, 1.15].into());
    camera.yaw = 0.0;
    camera.pitch = 0.4;
    camera
}

fn how_many_differ(a: &Image, b: &Image) -> usize {
    a.pixels
        .chunks_exact(4)
        .zip(b.pixels.chunks_exact(4))
        .filter(|(x, y)| (0..3).any(|c| x[c].abs_diff(y[c]) > 12))
        .count()
}

/// Draws a field document through the brick cache, as the viewport does.
fn capture_field(harness: &Harness, document: &mut ClayDocument, name: &str) -> Image {
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .sync(&harness.gpu, document)
        .expect("mesh the field");
    harness.capture(geometry.mesh(), &on_the_cap(), false, name)
}

/// Draws a carried layer — a grid or a mesh — with its colours.
fn capture_carried(harness: &Harness, document: &mut ClayDocument, name: &str) -> Image {
    let (vertices, indices) = support::viewport_geometry(document);
    let mut mesh = GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);
    harness.capture(&mesh, &head_on(), true, name)
}

#[test]
fn the_field_brushes_bound_in_this_change_leave_their_own_marks() {
    let Some(harness) = Harness::new() else {
        eprintln!("no adapter; skipping");
        return;
    };
    let Some(mut rest) = field() else {
        eprintln!("no engine; skipping");
        return;
    };
    let plain = capture_field(&harness, &mut rest, "gaps-field-rest");

    let mut marks = Vec::new();
    for (tool, invert, name) in [
        (ToolKind::Argila, false, "gaps-sdf-argila"),
        (ToolKind::Vinco, false, "gaps-sdf-vinco"),
        (ToolKind::Vinco, true, "gaps-sdf-vinco-invertido"),
        (
            ToolKind::MoverTopologico,
            false,
            "gaps-sdf-mover-topologico",
        ),
    ] {
        let Some(mut document) = field() else {
            return;
        };
        across_the_field(&mut document, tool, invert);
        let image = capture_field(&harness, &mut document, name);
        let moved = how_many_differ(&plain, &image);
        println!("{name:<32} {moved} pixels");
        marks.push((name, moved, image));
    }

    for (name, moved, _) in &marks {
        assert!(
            *moved > 150,
            "{name} changed {moved} pixels, so there is nothing to look at"
        );
    }
    // And they are not each other. A crease and a clay pat that render the
    // same picture are one brush wearing two names, which is exactly what a
    // shelf row with no binding behind it looks like.
    for (i, (name, _, image)) in marks.iter().enumerate() {
        for (other, _, against) in marks.iter().skip(i + 1) {
            let apart = how_many_differ(image, against);
            assert!(
                apart > 100,
                "{name} and {other} draw the same picture, {apart} pixels apart"
            );
        }
    }
}

#[test]
fn the_grid_brushes_bound_in_this_change_leave_their_own_marks() {
    let Some(harness) = Harness::new() else {
        eprintln!("no adapter; skipping");
        return;
    };
    let Some(mut rest) = grid() else {
        eprintln!("no engine; skipping");
        return;
    };
    let plain = capture_carried(&harness, &mut rest, "gaps-grid-rest");

    for (tool, name) in [
        (ToolKind::Mover, "gaps-voxel-mover"),
        (ToolKind::Planar, "gaps-voxel-planar"),
    ] {
        let Some(mut document) = grid() else {
            return;
        };
        drag_the_grid(&mut document, tool);
        let image = capture_carried(&harness, &mut document, name);
        let moved = how_many_differ(&plain, &image);
        println!("{name:<32} {moved} pixels");
        assert!(
            moved > 150,
            "{name} changed {moved} pixels, so there is nothing to look at"
        );
    }
}

#[test]
fn painting_a_grid_puts_colour_on_the_screen() {
    // The frame the colour work exists for. Before it the palette held one
    // entry, the tool painted cells the colour they already were, and the
    // composition root told the renderer to ignore vertex colour anyway — so
    // Pintar could not change a pixel by three separate routes.
    let Some(harness) = Harness::new() else {
        eprintln!("no adapter; skipping");
        return;
    };
    let Some(mut document) = grid() else {
        eprintln!("no engine; skipping");
        return;
    };
    let plain = capture_carried(&harness, &mut document, "gaps-paint-before");

    document.set_colour(Colour::new([0.75, 0.12, 0.10]));
    paint(&mut document, 0.0);
    document.set_colour(Colour::new([0.10, 0.20, 0.70]));
    paint(&mut document, 0.18);
    let painted = capture_carried(&harness, &mut document, "gaps-paint-after");

    let moved = how_many_differ(&plain, &painted);
    println!("two paint strokes changed {moved} pixels");
    assert!(
        moved > 500,
        "two paint strokes changed {moved} pixels, so the colour is not \
         reaching the screen"
    );
    // Redder somewhere and bluer somewhere, which is what two colours means.
    let reddest = |image: &Image| {
        image
            .pixels
            .chunks_exact(4)
            .map(|p| i32::from(p[0]) - i32::from(p[2]))
            .max()
            .unwrap_or(0)
    };
    let bluest = |image: &Image| {
        image
            .pixels
            .chunks_exact(4)
            .map(|p| i32::from(p[2]) - i32::from(p[0]))
            .max()
            .unwrap_or(0)
    };
    assert!(
        reddest(&painted) > reddest(&plain) + 20,
        "nothing came out red: {} against {}",
        reddest(&painted),
        reddest(&plain)
    );
    assert!(
        bluest(&painted) > bluest(&plain) + 20,
        "nothing came out blue: {} against {}",
        bluest(&painted),
        bluest(&plain)
    );
}

/// A paint stroke along the slab at a given height.
fn paint(document: &mut ClayDocument, y: f32) {
    let samples: Vec<GestureSample> = (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [(t - 0.5) * 1.0, y, 0.2],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Pintar,
            BrushSettings {
                size: 0.18,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("paint");
}

#[test]
fn a_mask_is_still_there_after_a_round_trip() {
    // The frame the mask migration exists for: painted, saved, reopened, and
    // still drawn over the same region. The mask reaches the viewport as a
    // per-vertex weight, which is what `mask_at` answers and what the shader
    // draws the frozen region from.
    let Some(harness) = Harness::new() else {
        eprintln!("no adapter; skipping");
        return;
    };
    let Some(mut document) = grid() else {
        eprintln!("no engine; skipping");
        return;
    };
    document
        .apply_stroke(
            ToolKind::Mascara,
            BrushSettings {
                size: 0.3,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: [0.3, 0.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("paint a mask");
    let painted = document.mask_state().painted_cells;
    assert!(painted > 0, "nothing was frozen");
    capture_masked(&harness, &mut document, "gaps-mask-painted");

    let dir = std::env::temp_dir().join("clayspace-visual-mask");
    std::fs::create_dir_all(&dir).expect("a place to save");
    let path = dir.join("masked.clay");
    let _ = std::fs::remove_file(&path);
    clayspace_model::DocumentModel::save(&mut document, &path).expect("save");

    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut reopened = ClayDocument::new(policy).expect("a document");
    clayspace_model::DocumentModel::open(&mut reopened, &path).expect("reopen");
    // Which subtool was active is not part of the document, so a reopened one
    // starts at the bottom of the stack. The mask belongs to the layer it was
    // painted on, so the layer has to be the one asked.
    let grid = clayspace_model::SceneModel::scene(&reopened)
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Voxel)
        .expect("the grid came back")
        .key;
    clayspace_model::SceneModel::set_active_layer(&mut reopened, grid).expect("activate the grid");
    capture_masked(&harness, &mut reopened, "gaps-mask-reopened");

    assert_eq!(
        reopened.mask_state().painted_cells,
        painted,
        "the reopened document's mask covers something else"
    );
}

/// The same carried capture, with the mask carried into the vertices — which
/// is how the viewport draws a frozen region.
fn capture_masked(harness: &Harness, document: &mut ClayDocument, name: &str) -> Image {
    let (mut vertices, indices) = support::viewport_geometry(document);
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
    if let Some(weights) = document.mask_at(&positions) {
        for (vertex, weight) in vertices.iter_mut().zip(weights) {
            vertex.mask = weight;
        }
    }
    let mut mesh = GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);
    harness.capture(&mesh, &head_on(), true, name)
}

/// The field surface is unchanged by turning vertex colour on.
///
/// The composition root now draws with the modulation enabled, unconditionally,
/// and this is the reason that is safe: the brick cache meshes without colour
/// and `read_mesh` fills those vertices with the identity, so a scene with no
/// colour in it renders bit for bit as it did.
#[test]
fn colour_modulation_leaves_a_field_surface_alone() {
    let Some(harness) = Harness::new() else {
        eprintln!("no adapter; skipping");
        return;
    };
    let Some(mut document) = field() else {
        eprintln!("no engine; skipping");
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("mesh the field");
    let off = harness.capture(
        geometry.mesh(),
        &on_the_cap(),
        false,
        "gaps-field-colour-off",
    );
    let on = harness.capture(geometry.mesh(), &on_the_cap(), true, "gaps-field-colour-on");
    assert_eq!(
        how_many_differ(&off, &on),
        0,
        "the field surface changed when vertex colour was enabled"
    );
}
