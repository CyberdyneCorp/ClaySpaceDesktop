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

use clayspace_view::{Camera, GpuMesh, MatCap, Overlays, SymmetryAxis, ViewPreset};
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
    let (r, g, b) = (background[0] as i32, background[1] as i32, background[2] as i32);

    // The design calls for a desaturated ground so the material reads
    // truthfully. Any strong channel imbalance would tint the sculpt.
    let spread = [r - g, g - b, r - b].map(i32::abs).into_iter().max().unwrap();
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
    harness.renderer.set_matcap(&harness.gpu, MatCap::Terracotta);
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
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();

    // A deliberately asymmetric subject, so the presets cannot look alike by
    // accident the way a sphere would.
    let mut doc = support::sphere_document(1.0);
    {
        let layer = doc.add_sdf_layer("Arm").expect("layer");
        let mut arm = clayspace_model::claycore::Item::sphere(0.45).expect("sphere");
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
    let Some(mut harness) = Harness::new() else {
        return;
    };

    // Two identical spheres at different depths. Under perspective the far one
    // is smaller; under orthographic they match.
    let mut doc = clayspace_model::claycore::Document::new().expect("document");
    let layer = doc.add_sdf_layer("Pair").expect("layer");
    for z in [-1.6f32, 1.6] {
        let mut item = clayspace_model::claycore::Item::sphere(0.5).expect("sphere");
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

        let coverage = image.pixels_differing_from(background, 6) as f64
            / (image.width * image.height) as f64;
        assert!(
            (0.05..0.85).contains(&coverage),
            "a sphere of radius {radius} framed to {:.0}% of the frame",
            coverage * 100.0
        );
    }
}

#[test]
fn an_empty_document_renders_the_ground_without_failing() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();

    let doc = clayspace_model::claycore::Document::new().expect("document");

    // The engine refuses to mesh a document with nothing in it rather than
    // returning an empty mesh. That is a legitimate answer, and the viewport
    // has to treat it as "nothing to draw" and not as a failure.
    let refused = doc.mesh(clayspace_model::claycore::MeshParams {
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
            symmetry_plane: None,
        },
        3.0,
    );
    let bare = harness.capture(&gpu_mesh, &camera, false, "08-overlays-none");
    let subject = bare.pixels_differing_from(background, 6);

    harness.renderer.set_overlays(
        &harness.gpu,
        Overlays {
            grid: true,
            symmetry_plane: None,
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
            symmetry_plane: Some(SymmetryAxis::X),
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
            symmetry_plane: None,
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
            symmetry_plane: Some(SymmetryAxis::X),
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

    assert!(sculpt_count > 0, "no sculpt pixels were found to compare against");
    assert!(brightest_overlay > 0.0, "no overlay pixels were drawn at all");
    let sculpt_mean = sculpt_total / sculpt_count as f64;
    assert!(
        brightest_overlay < sculpt_mean,
        "the brightest overlay pixel ({brightest_overlay:.0}) is brighter than the \
         sculpt's average ({sculpt_mean:.0}), so the overlays are competing with \
         the silhouette instead of sitting behind it"
    );
}

#[test]
fn orbiting_changes_the_view_and_stays_stable_at_the_pole() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();

    let mut doc = support::sphere_document(1.0);
    {
        let layer = doc.add_sdf_layer("Nose").expect("layer");
        let mut nose = clayspace_model::claycore::Item::sphere(0.4).expect("sphere");
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
