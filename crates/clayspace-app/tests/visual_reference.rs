//! Reference images in the viewport: drawn, faded, and always behind the clay.
//!
//! Measured on the pixels rather than asserted about the state, because every
//! claim here is about what the sculptor sees. A reference the renderer holds
//! and never draws would pass a state test and fail the sculptor.

mod support;

use clayspace_model::{RefPlane, ReferenceSettings, SurfaceOpacity};
use clayspace_view::{Camera, GpuMesh, Image, Reference};
use support::Harness;

/// A flat green picture, opaque, so it is unmistakable against the background
/// and against the clay.
fn green(width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .flat_map(|_| [20u8, 200, 60, 255])
        .collect()
}

/// How many pixels differ from the renderer's own background.
fn covered(image: &Image, background: [u8; 4]) -> usize {
    image
        .pixels
        .chunks_exact(4)
        .filter(|p| {
            p.iter()
                .zip(background)
                .any(|(a, b)| (i32::from(*a) - i32::from(b)).abs() > 6)
        })
        .count()
}

/// How many pixels are the reference's own green.
fn greens(image: &Image) -> usize {
    image
        .pixels
        .chunks_exact(4)
        .filter(|p| p[1] > 120 && p[0] < 80 && p[2] < 110)
        .count()
}

/// The mean colour of the frame, which is what an opacity moves.
fn mean(image: &Image) -> [f32; 3] {
    let mut sums = [0f64; 3];
    let count = (image.pixels.len() / 4) as f64;
    for pixel in image.pixels.chunks_exact(4) {
        for (sum, channel) in sums.iter_mut().zip(pixel) {
            *sum += f64::from(*channel);
        }
    }
    sums.map(|s| (s / count) as f32)
}

/// A camera looking square at the front plane, from in front of it.
fn front_camera() -> Camera {
    let mut camera = Camera::default();
    camera.frame_bounds([-1.5, -1.5, -1.5].into(), [1.5, 1.5, 1.5].into());
    camera
}

fn place(harness: &mut Harness, plane: RefPlane, settings: ReferenceSettings, side: u32) {
    let pixels = green(side, side);
    let corners = settings.corners(plane, 1.0);
    harness.renderer.set_reference(
        &harness.gpu,
        plane as usize,
        Some(Reference {
            pixels: &pixels,
            width: side,
            height: side,
            corners,
            opacity: settings.opacity,
        }),
    );
}

#[test]
fn a_placed_reference_reaches_the_screen() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let background = harness.background();
    let empty = GpuMesh::new(&harness.gpu);
    let camera = front_camera();

    let before = harness.capture(&empty, &camera, false, "reference-empty");
    assert_eq!(
        covered(&before, background),
        0,
        "the frame was not empty to begin with"
    );

    place(
        &mut harness,
        RefPlane::Front,
        ReferenceSettings {
            opacity: 1.0,
            ..ReferenceSettings::default()
        },
        16,
    );
    let after = harness.capture(&empty, &camera, false, "reference-front");
    assert!(
        covered(&after, background) > 1000,
        "a placed reference covered {} pixels",
        covered(&after, background)
    );
}

#[test]
fn the_opacity_is_what_it_says() {
    // Not a state test: the number in the panel has to reach the blend, and
    // the only place that shows is the pixels.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let empty = GpuMesh::new(&harness.gpu);
    let camera = front_camera();
    let background = harness.background();

    let mut frames = Vec::new();
    for (opacity, name) in [(1.0, "opaque"), (0.5, "half"), (0.0, "clear")] {
        place(
            &mut harness,
            RefPlane::Front,
            ReferenceSettings {
                opacity,
                ..ReferenceSettings::default()
            },
            16,
        );
        frames.push(harness.capture(&empty, &camera, false, &format!("reference-{name}")));
    }

    let green_of = |image: &Image| mean(image)[1];
    assert!(
        green_of(&frames[0]) > green_of(&frames[1]),
        "opaque {} was no stronger than half {}",
        green_of(&frames[0]),
        green_of(&frames[1])
    );
    assert!(
        green_of(&frames[1]) > green_of(&frames[2]),
        "half {} was no stronger than clear {}",
        green_of(&frames[1]),
        green_of(&frames[2])
    );
    assert_eq!(
        covered(&frames[2], background),
        0,
        "a reference at zero opacity was still on screen"
    );
}

#[test]
fn the_clay_is_always_in_front_of_the_reference() {
    // Whichever side of the plane the camera is on. A guide that occludes the
    // form it is guiding has stopped being a guide, so this is drawn first and
    // writes no depth — and the camera swinging round must not change it.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let document = support::sphere_document(0.9);
    let mesh = support::mesh_document(&document, 48);
    let form = harness.upload(&mesh);

    place(
        &mut harness,
        RefPlane::Front,
        ReferenceSettings {
            opacity: 1.0,
            // In front of where the camera will be on the second look, which
            // is what makes this a test rather than a coincidence.
            depth: 1.2,
            height: 4.0,
            ..ReferenceSettings::default()
        },
        16,
    );

    for (name, azimuth) in [("front", 0.0f32), ("behind", std::f32::consts::PI)] {
        let mut camera = front_camera();
        camera.orbit(azimuth, 0.0);
        let image = harness.capture(&form, &camera, false, &format!("reference-depth-{name}"));

        // The reference is on screen at all, so a renderer that quietly
        // stopped drawing one cannot pass this by leaving the frame empty.
        assert!(
            greens(&image) > 1000,
            "no reference was drawn from {name}, so the rest proves nothing"
        );

        // The middle of the frame is the sphere. It is lit clay, not flat
        // green, on both looks.
        let centre = image.pixel(image.width / 2, image.height / 2);
        assert!(
            centre[1] < 150 || centre[0] > 100,
            "the reference covered the form from {name}: {centre:?}"
        );
    }
}

#[test]
fn each_plane_is_drawn_where_it_belongs() {
    // Three pictures on three planes, seen from a corner: all three reach the
    // screen, and none of them replaces another.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let empty = GpuMesh::new(&harness.gpu);
    let background = harness.background();
    let mut camera = front_camera();
    camera.orbit(0.7, 0.5);

    let mut covers = Vec::new();
    for plane in RefPlane::ALL {
        place(
            &mut harness,
            plane,
            ReferenceSettings {
                opacity: 1.0,
                ..ReferenceSettings::default()
            },
            16,
        );
        let image = harness.capture(
            &empty,
            &camera,
            false,
            &format!("reference-plane-{}", plane.tag()),
        );
        covers.push(covered(&image, background));
    }

    // Each plane added area rather than replacing the last: three quads at
    // different angles cannot overlap exactly.
    assert!(covers[0] > 0, "the front plane drew nothing");
    assert!(
        covers[1] > covers[0],
        "the side plane added nothing: {covers:?}"
    );
    assert!(
        covers[2] > covers[1],
        "the top plane added nothing: {covers:?}"
    );

    // And taking one away takes its pixels with it.
    harness
        .renderer
        .set_reference(&harness.gpu, RefPlane::Top as usize, None);
    let cleared = harness.capture(&empty, &camera, false, "reference-plane-cleared");
    assert!(
        covered(&cleared, background) < covers[2],
        "clearing a plane left it on screen"
    );
}

#[test]
fn dialling_the_model_back_lets_the_reference_show_through_it() {
    // The point of the whole pair of features: a photograph behind the form,
    // and enough of the form taken away to trace against it. A state test
    // would pass on a renderer that held the number and drew solid anyway.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let document = support::sphere_document(0.9);
    let mesh = support::mesh_document(&document, 48);
    let form = harness.upload(&mesh);
    let camera = front_camera();

    // A green sheet directly behind the sphere, big enough that the sphere
    // sits well inside it.
    place(
        &mut harness,
        RefPlane::Front,
        ReferenceSettings {
            opacity: 1.0,
            height: 4.0,
            ..ReferenceSettings::default()
        },
        16,
    );

    let solid = harness.capture(&form, &camera, false, "reference-model-solid");
    let solid_centre = solid.pixel(solid.width / 2, solid.height / 2);

    harness
        .renderer
        .set_surface_opacity(SurfaceOpacity::new(0.25));
    let faded = harness.capture(&form, &camera, false, "reference-model-faded");
    let faded_centre = faded.pixel(faded.width / 2, faded.height / 2);

    // The middle of the frame is the sphere over the sheet. Solid, it is lit
    // clay; faded, the green behind it comes through.
    assert!(
        i32::from(faded_centre[1]) - i32::from(faded_centre[0])
            > i32::from(solid_centre[1]) - i32::from(solid_centre[0]) + 20,
        "the reference did not come through the clay: solid {solid_centre:?}, \
         faded {faded_centre:?}"
    );

    // And the form is still a form: dialled back is not turned off, so the
    // frame still differs from one with no clay in it at all.
    let empty = GpuMesh::new(&harness.gpu);
    let without = harness.capture(&empty, &camera, false, "reference-model-absent");
    let differing = faded
        .pixels
        .chunks_exact(4)
        .zip(without.pixels.chunks_exact(4))
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .any(|(x, y)| (i32::from(*x) - i32::from(*y)).abs() > 8)
        })
        .count();
    assert!(
        differing > 1000,
        "a faded surface was indistinguishable from no surface at all \
         ({differing} pixels differ)"
    );
}

#[test]
fn a_solid_model_hides_what_is_behind_it() {
    // The control for the test above: with the dial left alone, the reference
    // is behind the clay and stays there.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let document = support::sphere_document(0.9);
    let mesh = support::mesh_document(&document, 48);
    let form = harness.upload(&mesh);
    let camera = front_camera();
    place(
        &mut harness,
        RefPlane::Front,
        ReferenceSettings {
            opacity: 1.0,
            height: 4.0,
            ..ReferenceSettings::default()
        },
        16,
    );

    harness.renderer.set_surface_opacity(SurfaceOpacity::SOLID);
    let image = harness.capture(&form, &camera, false, "reference-model-opaque");
    let centre = image.pixel(image.width / 2, image.height / 2);
    assert!(
        greens(&image) > 1000,
        "the reference was not on screen, so this proves nothing"
    );
    assert!(
        i32::from(centre[1]) - i32::from(centre[0]) < 20,
        "a solid model let the reference through: {centre:?}"
    );
}

#[test]
fn a_cage_draws_the_surface_through_whatever_the_dial_says() {
    // Regression. The renderer chose the ghost pipeline from the *effective*
    // opacity and then wrote the *dial* into the uniform, so raising a cage
    // over a solid surface selected the see-through pipeline and drew it at
    // alpha 1.0 — ghosting that looked exactly like no ghosting.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let document = support::sphere_document(0.9);
    let mesh = support::mesh_document(&document, 48);
    let form = harness.upload(&mesh);
    let camera = front_camera();
    place(
        &mut harness,
        RefPlane::Front,
        ReferenceSettings {
            opacity: 1.0,
            height: 4.0,
            ..ReferenceSettings::default()
        },
        16,
    );

    harness.renderer.set_surface_opacity(SurfaceOpacity::SOLID);
    let solid = harness.capture(&form, &camera, false, "reference-cage-solid");
    harness.renderer.set_ghosted(true);
    let caged = harness.capture(&form, &camera, false, "reference-cage-ghosted");
    harness.renderer.set_ghosted(false);

    let centre = |image: &Image| image.pixel(image.width / 2, image.height / 2);
    let (solid_centre, caged_centre) = (centre(&solid), centre(&caged));
    assert!(
        i32::from(caged_centre[1]) - i32::from(caged_centre[0])
            > i32::from(solid_centre[1]) - i32::from(solid_centre[0]) + 20,
        "a cage over a solid surface drew it solid anyway: solid \
         {solid_centre:?}, caged {caged_centre:?}"
    );
}

#[test]
fn a_cage_does_not_make_a_faint_surface_more_solid() {
    // The other half of the same rule: the cage is a ceiling, not a setting.
    // A sculptor who dialled the clay back further than the cage would has
    // said what they want, and raising a cage is not an argument with it.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let document = support::sphere_document(0.9);
    let mesh = support::mesh_document(&document, 48);
    let form = harness.upload(&mesh);
    let camera = front_camera();
    place(
        &mut harness,
        RefPlane::Front,
        ReferenceSettings {
            opacity: 1.0,
            height: 4.0,
            ..ReferenceSettings::default()
        },
        16,
    );

    // Fainter than the cage's own 0.42.
    harness
        .renderer
        .set_surface_opacity(SurfaceOpacity::new(0.15));
    let alone = harness.capture(&form, &camera, false, "reference-faint-alone");
    harness.renderer.set_ghosted(true);
    let caged = harness.capture(&form, &camera, false, "reference-faint-caged");
    harness.renderer.set_ghosted(false);

    // A block at the centre rather than the single pixel there.
    //
    // One pixel decided this, and nothing established it was on the clay. If
    // the sphere ever stopped covering the frame centre — a camera change, a
    // framing change, a surface that failed to draw at all — both reads become
    // the background, the difference becomes zero, and the test reports that
    // the cage changed nothing about a surface that was not there.
    let block = |image: &Image| {
        let (cx, cy) = (image.width / 2, image.height / 2);
        let mut pixels = Vec::new();
        for y in cy.saturating_sub(16)..(cy + 16).min(image.height) {
            for x in cx.saturating_sub(16)..(cx + 16).min(image.width) {
                pixels.push(image.pixel(x, y));
            }
        }
        pixels
    };
    let (a, c) = (block(&alone), block(&caged));

    // On the subject, checked before anything is concluded from it.
    let background = harness.background();
    let on_clay = a
        .iter()
        .filter(|p| (0..3).any(|i| p[i].abs_diff(background[i]) > 12))
        .count();
    assert!(
        on_clay * 2 > a.len(),
        "the block this test reads is mostly background ({on_clay} of {} \
         pixels are on the form), so it cannot tell whether a cage changed \
         the surface — see target/visual/reference-faint-alone.png",
        a.len()
    );

    let difference = a
        .iter()
        .zip(&c)
        .map(|(x, y)| {
            (0..3)
                .map(|i| i32::from(x[i]) - i32::from(y[i]))
                .map(i32::abs)
                .sum::<i32>()
        })
        .max()
        .unwrap_or(0);
    assert!(
        difference < 12,
        "raising a cage changed a deliberately faint surface: the worst pixel \
         of the centre block moved by {difference}"
    );
}
