//! Visual tests for the viewport: shading, camera, presets and overlays.
//!
//! Each renders a real frame and writes it to `target/visual/` for inspection.
//! Run them and look:
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_rendering
//! open target/visual
//! ```

mod support;

use clayspace_view::{
    mirrored_cursors, BrushCursor, Camera, GpuMesh, MatCap, Overlays, ViewPreset,
};
use support::Harness;

/// Enough of the frame changed that something was clearly drawn, rather than a
/// stray pixel or a rounding difference.
const DREW_SOMETHING: usize = 1_000;

#[test]
fn a_sphere_is_drawn_against_the_neutral_ground() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();

    let doc = support::sphere_document(1.0);
    let mesh = support::mesh_document(&doc, 64);
    let camera = support::framed_camera(&mesh);
    let image = harness.capture_mesh(&mesh, &camera, "01-sphere");

    let drawn = image.pixels_differing_from(background, 6);
    assert!(
        drawn > DREW_SOMETHING,
        "only {drawn} pixels differ from the background, so nothing was drawn"
    );

    // The subject should be framed, not filling or lost in the frame.
    let coverage = drawn as f64 / (image.width * image.height) as f64;
    assert!(
        (0.05..0.85).contains(&coverage),
        "the sphere covers {:.0}% of the frame, which is not a framed subject",
        coverage * 100.0
    );

    // The centre is the sphere; the corner is the ground.
    let centre = image.pixel(image.width / 2, image.height / 2);
    assert!(
        centre[0].abs_diff(background[0]) > 6,
        "the centre of the frame is still the background colour"
    );
}

#[test]
fn the_ground_is_neutral_and_desaturated() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let background = harness.background();
    let (r, g, b) = (
        background[0] as i32,
        background[1] as i32,
        background[2] as i32,
    );

    // The design calls for a desaturated ground so the material reads
    // truthfully. Any strong channel imbalance would tint the sculpt.
    let spread = [r - g, g - b, r - b]
        .map(i32::abs)
        .into_iter()
        .max()
        .unwrap();
    assert!(
        spread <= 12,
        "the viewport ground {background:?} is tinted (channel spread {spread}), \
         which shifts the apparent value of the material"
    );
    assert!(
        r < 80 && g < 80 && b < 80,
        "the viewport ground {background:?} is too light to sit behind a grey sculpt"
    );
}

#[test]
fn every_matcap_renders_and_they_differ() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();
    let doc = support::sphere_document(1.0);
    let mesh = support::mesh_document(&doc, 64);
    let camera = support::framed_camera(&mesh);
    let gpu_mesh = harness.upload(&mesh);

    let mut captured = Vec::new();
    for matcap in MatCap::ALL {
        harness.renderer.set_matcap(&harness.gpu, matcap);
        let name = format!("02-matcap-{:?}", matcap).to_lowercase();
        let image = harness.capture(&gpu_mesh, &camera, false, &name);
        assert!(
            image.pixels_differing_from(background, 6) > DREW_SOMETHING,
            "{matcap:?} drew nothing"
        );
        captured.push((matcap, image));
    }

    for (i, (a_name, a)) in captured.iter().enumerate() {
        for (b_name, b) in captured.iter().skip(i + 1) {
            let difference = a.mean_difference_over_subject(b, background, 6);
            assert!(
                difference > 1.5,
                "{a_name:?} and {b_name:?} render almost identically \
                 (mean difference {difference:.2} over the subject), so the \
                 material is not reaching the shader"
            );
        }
    }
}

#[test]
fn changing_material_does_not_change_geometry() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();
    let doc = support::sphere_document(1.0);
    let mesh = support::mesh_document(&doc, 64);
    let camera = support::framed_camera(&mesh);
    let gpu_mesh = harness.upload(&mesh);

    harness.renderer.set_matcap(&harness.gpu, MatCap::GreyClay);
    let grey = harness.capture(&gpu_mesh, &camera, false, "03-silhouette-grey");
    harness
        .renderer
        .set_matcap(&harness.gpu, MatCap::Terracotta);
    let warm = harness.capture(&gpu_mesh, &camera, false, "03-silhouette-terracotta");

    // Different colours, same silhouette: the count of non-background pixels
    // is the shape, and it must not move when only the material changes.
    let a = grey.pixels_differing_from(background, 6);
    let b = warm.pixels_differing_from(background, 6);
    let drift = a.abs_diff(b) as f64 / a as f64;
    assert!(
        drift < 0.02,
        "changing material moved the silhouette by {:.1}% ({a} vs {b} pixels)",
        drift * 100.0
    );
}

#[test]
fn each_view_preset_shows_a_different_face() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let background = harness.background();

    // A deliberately asymmetric subject, so the presets cannot look alike by
    // accident the way a sphere would.
    let mut doc = support::sphere_document(1.0);
    {
        let layer = doc.add_sdf_layer("Arm").expect("layer");
        let mut arm = clayspace_engine::claycore::Item::sphere(0.45).expect("sphere");
        arm.set_position([1.1, 0.35, 0.0]).expect("position");
        doc.add_item(layer, &arm).expect("place");
    }
    let mesh = support::mesh_document(&doc, 64);
    let gpu_mesh = harness.upload(&mesh);

    let mut captured = Vec::new();
    for preset in ViewPreset::ALL {
        let mut camera = support::framed_camera(&mesh);
        camera.apply_preset(preset);
        let name = format!("04-view-{:?}", preset).to_lowercase();
        let image = harness.capture(&gpu_mesh, &camera, false, &name);
        assert!(
            image.pixels_differing_from(background, 6) > DREW_SOMETHING,
            "{preset:?} drew nothing"
        );
        captured.push((preset, image));
    }

    for (i, (a_name, a)) in captured.iter().enumerate() {
        for (b_name, b) in captured.iter().skip(i + 1) {
            let difference = a.mean_difference_over_subject(b, background, 6);
            assert!(
                difference > 1.0,
                "{a_name:?} and {b_name:?} render almost identically \
                 (mean difference {difference:.2} over the subject)"
            );
        }
    }
}

#[test]
fn an_orthographic_preset_removes_the_perspective_divide() {
    let Some(harness) = Harness::new() else {
        return;
    };

    // Two identical spheres at different depths. Under perspective the far one
    // is smaller; under orthographic they match.
    let mut doc = clayspace_engine::claycore::Document::new().expect("document");
    let layer = doc.add_sdf_layer("Pair").expect("layer");
    for z in [-1.6f32, 1.6] {
        let mut item = clayspace_engine::claycore::Item::sphere(0.5).expect("sphere");
        item.set_position([z * 0.9, 0.0, z]).expect("position");
        doc.add_item(layer, &item).expect("place");
    }
    let mesh = support::mesh_document(&doc, 64);
    let gpu_mesh = harness.upload(&mesh);

    let mut perspective = support::framed_camera(&mesh);
    perspective.apply_preset(ViewPreset::Perspective);
    let a = harness.capture(&gpu_mesh, &perspective, false, "05-projection-perspective");

    let mut ortho = support::framed_camera(&mesh);
    ortho.apply_preset(ViewPreset::Top);
    let b = harness.capture(&gpu_mesh, &ortho, false, "05-projection-orthographic");

    assert!(
        a.mean_difference(&b) > 1.0,
        "the orthographic view is indistinguishable from the perspective one"
    );
}

#[test]
fn framing_puts_the_subject_in_view_at_any_scale() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();

    // Frame-all must work on a tiny subject and a huge one alike; a fixed
    // default distance would lose both.
    for (radius, name) in [(0.02f32, "06-frame-tiny"), (40.0, "06-frame-huge")] {
        let doc = support::sphere_document(radius);
        let mesh = support::mesh_document(&doc, 48);
        let camera = support::framed_camera(&mesh);
        let image = harness.capture_mesh(&mesh, &camera, name);

        let coverage =
            image.pixels_differing_from(background, 6) as f64 / (image.width * image.height) as f64;
        assert!(
            (0.05..0.85).contains(&coverage),
            "a sphere of radius {radius} framed to {:.0}% of the frame",
            coverage * 100.0
        );
    }
}

#[test]
fn an_empty_document_renders_the_ground_without_failing() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let background = harness.background();

    let doc = clayspace_engine::claycore::Document::new().expect("document");

    // The engine refuses to mesh a document with nothing in it rather than
    // returning an empty mesh. That is a legitimate answer, and the viewport
    // has to treat it as "nothing to draw" and not as a failure.
    let refused = doc.mesh(clayspace_engine::claycore::MeshParams {
        resolution: 32,
        ..Default::default()
    });
    assert!(
        refused.is_err(),
        "an empty document was meshed; the viewport's empty case needs revisiting"
    );

    let mut camera = Camera::default();
    camera.frame_default();
    let empty = GpuMesh::new(&harness.gpu);
    let image = harness.capture(&empty, &camera, false, "07-empty-document");
    assert!(
        image.pixels_differing_from(background, 6) < 100,
        "an empty viewport drew something"
    );
}

#[test]
fn overlays_draw_behind_the_sculpt_and_can_be_turned_off() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();
    let doc = support::sphere_document(1.0);
    let mesh = support::mesh_document(&doc, 64);
    let camera = support::framed_camera(&mesh);
    let gpu_mesh = harness.upload(&mesh);

    harness.renderer.set_overlays(
        &harness.gpu,
        Overlays {
            grid: false,
            symmetry_planes: [false; 3],
        },
        3.0,
    );
    let bare = harness.capture(&gpu_mesh, &camera, false, "08-overlays-none");
    let subject = bare.pixels_differing_from(background, 6);

    harness.renderer.set_overlays(
        &harness.gpu,
        Overlays {
            grid: true,
            symmetry_planes: [false; 3],
        },
        3.0,
    );
    let with_grid = harness.capture(&gpu_mesh, &camera, false, "08-overlays-grid");
    assert!(
        with_grid.pixels_differing_from(background, 6) > subject,
        "turning the grid on drew nothing"
    );

    harness.renderer.set_overlays(
        &harness.gpu,
        Overlays {
            grid: true,
            symmetry_planes: [true, false, false],
        },
        3.0,
    );
    let with_symmetry = harness.capture(&gpu_mesh, &camera, false, "08-overlays-symmetry");
    assert!(
        with_symmetry.mean_difference(&with_grid) > 0.1,
        "the symmetry plane drew nothing on top of the grid"
    );

    // Turning them back off must restore the bare frame exactly.
    harness.renderer.set_overlays(
        &harness.gpu,
        Overlays {
            grid: false,
            symmetry_planes: [false; 3],
        },
        3.0,
    );
    let restored = harness.capture(&gpu_mesh, &camera, false, "08-overlays-restored");
    assert!(
        restored.mean_difference(&bare) < 0.01,
        "turning overlays off did not restore the frame"
    );
}

#[test]
fn overlays_stay_dimmer_than_the_sculpt() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();
    let doc = support::sphere_document(1.0);
    let mesh = support::mesh_document(&doc, 64);
    let camera = support::framed_camera(&mesh);
    let gpu_mesh = harness.upload(&mesh);

    harness.renderer.set_overlays(
        &harness.gpu,
        Overlays {
            grid: true,
            symmetry_planes: [true, false, false],
        },
        3.0,
    );
    let image = harness.capture(&gpu_mesh, &camera, false, "10-overlay-weight");

    // The spec says overlays sit behind the sculpt in visual weight and never
    // obscure the silhouette. Measured: the brightest overlay pixel must stay
    // below the sculpt's own average, or it is competing for attention.
    //
    // This is the assertion the first version of the renderer lacked, and it
    // is why the grid shipped several times too bright: the palette's hex
    // values were written into a linear target without conversion.
    let luminance = |p: [u8; 4]| 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
    let background_luminance = luminance(background);

    // The sculpt occupies the middle of a framed view; the corners are ground
    // and overlay only.
    let mut sculpt_total = 0.0;
    let mut sculpt_count = 0usize;
    let mut brightest_overlay: f64 = 0.0;
    let centre = (image.width as f64 / 2.0, image.height as f64 / 2.0);
    let sculpt_radius = image.height as f64 * 0.28;

    for y in 0..image.height {
        for x in 0..image.width {
            let p = image.pixel(x, y);
            let l = luminance(p);
            if l <= background_luminance + 2.0 {
                continue;
            }
            let dx = x as f64 - centre.0;
            let dy = y as f64 - centre.1;
            if (dx * dx + dy * dy).sqrt() < sculpt_radius {
                sculpt_total += l;
                sculpt_count += 1;
            } else {
                brightest_overlay = brightest_overlay.max(l);
            }
        }
    }

    assert!(
        sculpt_count > 0,
        "no sculpt pixels were found to compare against"
    );
    assert!(
        brightest_overlay > 0.0,
        "no overlay pixels were drawn at all"
    );
    let sculpt_mean = sculpt_total / sculpt_count as f64;
    assert!(
        brightest_overlay < sculpt_mean,
        "the brightest overlay pixel ({brightest_overlay:.0}) is brighter than the \
         sculpt's average ({sculpt_mean:.0}), so the overlays are competing with \
         the silhouette instead of sitting behind it"
    );
}

#[test]
fn the_gizmo_reports_the_camera_orientation() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();
    let doc = support::sphere_document(1.0);
    let mesh = support::mesh_document(&doc, 64);
    let gpu_mesh = harness.upload(&mesh);

    let mut camera = support::framed_camera(&mesh);
    harness.renderer.show_gizmo = false;
    let without = harness.capture(&gpu_mesh, &camera, false, "11-gizmo-off");

    harness.renderer.show_gizmo = true;
    let with = harness.capture(&gpu_mesh, &camera, false, "11-gizmo-on");

    // The gizmo is deliberately small, so a mean over the whole frame is the
    // wrong measure — it is a few hundred pixels in 172,800. Count the pixels
    // in the corner it occupies instead.
    // Past the driver's own noise rather than bit-identical — see
    // `support::RENDER_NOISE`. Written as `!=`, this counted 90 pixels on a
    // macOS runner whose largest difference was four levels out of 255, and
    // called it the gizmo drawing over the sculpt.
    let changed_in = |a: &clayspace_view::Image, b: &clayspace_view::Image, x0, y0, x1, y1| {
        support::differing_pixels_within(a, b, x0, y0, x1, y1)
    };

    let corner = (with.width * 3 / 4, 0, with.width, with.height / 4);
    let in_corner = changed_in(&with, &without, corner.0, corner.1, corner.2, corner.3);
    assert!(
        in_corner > 20,
        "the gizmo drew only {in_corner} pixels in the corner it should occupy"
    );

    // And nowhere else: it must not overlap the sculpt.
    let middle = changed_in(
        &with,
        &without,
        with.width / 4,
        with.height / 3,
        with.width / 2,
        with.height * 2 / 3,
    );
    assert_eq!(
        middle, 0,
        "the gizmo drew over the sculpt instead of staying in its corner"
    );

    // Orienting the camera must reorient it.
    camera.orbit(1.4, 0.4);
    let turned = harness.capture(&gpu_mesh, &camera, false, "11-gizmo-turned");
    let gizmo_moved = changed_in(&turned, &with, corner.0, corner.1, corner.2, corner.3);
    assert!(
        gizmo_moved > 20,
        "the gizmo did not follow the camera ({gizmo_moved} pixels changed)"
    );

    harness.renderer.show_gizmo = false;
    let _ = background;
}

#[test]
fn the_brush_cursor_follows_the_surface_and_clears_off_it() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();
    let doc = support::sphere_document(1.0);
    let mesh = support::mesh_document(&doc, 64);
    let camera = support::framed_camera(&mesh);
    let gpu_mesh = harness.upload(&mesh);

    harness.renderer.set_cursors(&harness.gpu, &[]);
    let without = harness.capture(&gpu_mesh, &camera, false, "12-cursor-off");

    // On the near face of the sphere, where the camera can see it.
    harness.renderer.set_cursors(
        &harness.gpu,
        &[BrushCursor {
            position: [0.0, 0.0, 1.0],
            normal: [0.0, 0.0, 1.0],
            radius: 0.35,
            mirrored: false,
        }],
    );
    let with = harness.capture(&gpu_mesh, &camera, false, "12-cursor-on");

    let drawn = {
        let mut count = 0usize;
        for y in 0..with.height {
            for x in 0..with.width {
                if with.pixel(x, y) != without.pixel(x, y) {
                    count += 1;
                }
            }
        }
        count
    };
    assert!(drawn > 100, "the brush cursor drew only {drawn} pixels");

    // A larger brush must read as a larger ring.
    harness.renderer.set_cursors(
        &harness.gpu,
        &[BrushCursor {
            position: [0.0, 0.0, 1.0],
            normal: [0.0, 0.0, 1.0],
            radius: 0.7,
            mirrored: false,
        }],
    );
    let larger = harness.capture(&gpu_mesh, &camera, false, "12-cursor-large");
    assert!(
        larger.mean_difference(&with) > 0.01,
        "changing the brush size did not change the cursor"
    );

    // Off the surface, it must clear rather than hang at some depth.
    harness.renderer.set_cursors(&harness.gpu, &[]);
    let cleared = harness.capture(&gpu_mesh, &camera, false, "12-cursor-cleared");
    // By pixels past the noise rather than by a mean: the cursor is a thin
    // ring, so its whole signal is a mean of 0.059 levels and the residue of
    // *not* drawing it measured 0.0058 on a macOS runner — a sixth of the
    // signal, and six times the bound this used to carry. Counted in pixels
    // the two are 273 against nothing at all.
    assert_eq!(
        support::differing_pixels(&cleared, &without),
        0,
        "clearing the cursor left it on screen"
    );

    let _ = background;
}

#[test]
fn orbiting_changes_the_view_and_stays_stable_at_the_pole() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let background = harness.background();

    let mut doc = support::sphere_document(1.0);
    {
        let layer = doc.add_sdf_layer("Nose").expect("layer");
        let mut nose = clayspace_engine::claycore::Item::sphere(0.4).expect("sphere");
        nose.set_position([0.0, 0.2, 1.0]).expect("position");
        doc.add_item(layer, &nose).expect("place");
    }
    let mesh = support::mesh_document(&doc, 64);
    let gpu_mesh = harness.upload(&mesh);

    let mut camera = support::framed_camera(&mesh);
    let front = harness.capture(&gpu_mesh, &camera, false, "09-orbit-start");

    camera.orbit(1.2, 0.0);
    let turned = harness.capture(&gpu_mesh, &camera, false, "09-orbit-turned");
    assert!(
        front.mean_difference(&turned) > 1.0,
        "orbiting did not change the view"
    );

    // Driven hard into the pole, where an unclamped pitch degenerates the view
    // matrix and the frame goes blank or black.
    for _ in 0..40 {
        camera.orbit(0.0, 0.5);
    }
    let pole = harness.capture(&gpu_mesh, &camera, false, "09-orbit-pole");
    assert!(
        pole.pixels_differing_from(background, 6) > DREW_SOMETHING,
        "the view degenerated at the pole and drew nothing"
    );
}

#[test]
fn symmetry_shows_every_place_the_stroke_will_land() {
    // A cursor that shows one ring while the engine deposits two is telling
    // the user the wrong thing about the next click. This captures what the
    // mirrors actually look like so the cue can be judged, not just counted.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let doc = support::sphere_document(1.0);
    let mesh = support::mesh_document(&doc, 64);
    let camera = support::framed_camera(&mesh);
    let gpu_mesh = harness.upload(&mesh);

    // Off to one side, so a mirror through x = 0 lands somewhere visibly
    // different rather than on top of the original.
    let pointer = BrushCursor {
        position: [0.62, 0.30, 0.72],
        normal: [0.62, 0.30, 0.72],
        radius: 0.28,
        mirrored: false,
    };

    harness
        .renderer
        .set_overlays(&harness.gpu, Overlays::default(), 2.0);
    harness
        .renderer
        .set_cursors(&harness.gpu, &mirrored_cursors(pointer, [false; 3]));
    let alone = harness.capture(&gpu_mesh, &camera, false, "16-symmetry-cursor-off");

    let gpu = harness.gpu.clone();
    harness.renderer.set_overlays(
        &gpu,
        Overlays {
            grid: true,
            symmetry_planes: [true, false, false],
        },
        2.0,
    );
    harness
        .renderer
        .set_cursors(&gpu, &mirrored_cursors(pointer, [true, false, false]));
    let mirrored = harness.capture(&gpu_mesh, &camera, false, "16-symmetry-cursor-x");

    assert!(
        mirrored.mean_difference(&alone) > 0.001,
        "turning symmetry on drew nothing new, so there is no cue that the \
         stroke lands twice"
    );

    harness.renderer.set_overlays(
        &gpu,
        Overlays {
            grid: true,
            symmetry_planes: [true, true, false],
        },
        2.0,
    );
    harness
        .renderer
        .set_cursors(&gpu, &mirrored_cursors(pointer, [true, true, false]));
    let both = harness.capture(&gpu_mesh, &camera, false, "16-symmetry-cursor-xy");
    assert!(
        both.mean_difference(&mirrored) > 0.001,
        "the second mirror plane changed nothing on screen"
    );
}
