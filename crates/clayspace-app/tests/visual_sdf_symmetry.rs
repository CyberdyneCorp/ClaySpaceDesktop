//! Every SDF brush, drawn with symmetry on, so the two sides can be looked at.
//!
//! `visual_brushes` already draws each brush doing its work. This asks the
//! other question — whether what it did came out on both sides — because that
//! was the fault: five of the nine bypassed the symmetry argument entirely,
//! and a sixth mirrored when it had not been asked to.
//!
//! Each brush writes one frame to `target/visual/sdf-sym-*`. Looking at them
//! is the point; the assertion only catches what can be stated without eyes —
//! that the left and right halves of the picture agree.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_sdf_symmetry
//! open target/visual
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, Representation, SculptModel, ToolKind};
use clayspace_view::{Camera, Image};
use support::Harness;

fn sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// Straight down −z, so the mirror plane is the middle column of the picture
/// and the two halves are directly comparable.
fn head_on() -> Camera {
    let mut camera = Camera::default();
    camera.frame_bounds([-1.4, -1.4, -1.4].into(), [1.4, 1.4, 1.4].into());
    camera.yaw = 0.0;
    camera.pitch = 0.0;
    camera
}

/// How much the *silhouette* disagrees with its own reflection, as a share of
/// the pixels either half covers.
///
/// The silhouette rather than the picture. A MatCap shades by the view-space
/// normal and is not itself left-right symmetric, so a perfectly symmetric
/// form renders as a very asymmetric image — measured, a plain mirrored dab
/// scored 0.58 by colour, which says nothing about the clay. What the two
/// halves of a symmetric form do share is their outline.
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

/// How many pixels two frames differ in.
fn how_many_differ(a: &Image, b: &Image) -> usize {
    a.pixels
        .chunks_exact(4)
        .zip(b.pixels.chunks_exact(4))
        .filter(|(x, y)| (0..3).any(|c| x[c].abs_diff(y[c]) > 12))
        .count()
}

/// A ridge along the limb, raised symmetrically.
///
/// The bake verbs — both relaxes and both planes — move the surface by about
/// 0.006 on a bare sphere, which no silhouette can show. Given a ridge to work
/// on they move it visibly, which is also the only case in which a sculptor
/// would reach for them. Raised with symmetry on, so the baseline the brushes
/// are compared against is itself symmetric.
fn roughen(document: &mut ClayDocument) {
    for step in 0..5 {
        let t = step as f32 / 4.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.2,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: [0.9, (t - 0.5) * 0.5, 0.4],
                    pressure: 1.0,
                    time: t,
                }],
                [true, false, false],
            )
            .expect("the ridge was refused");
    }
}

fn stroke(document: &mut ClayDocument, tool: ToolKind, symmetry: [bool; 3]) {
    let samples: Vec<GestureSample> = (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                // Near the limb rather than facing the camera: a bump
                // pointing at the eye does not change the outline, and the
                // outline is what this measures.
                position: [0.88 + (t - 0.5) * 0.1, (t - 0.5) * 0.35, 0.42],
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
                intensity: 0.9,
                ..BrushSettings::default()
            },
            &samples,
            symmetry,
        )
        .expect("the stroke was refused");
}

#[test]
fn every_sdf_brush_comes_out_on_both_sides() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let camera = head_on();
    let mut worst: Vec<(String, f64)> = Vec::new();

    // The untouched form, so each brush can be held to having done something.
    let Some(mut plain) = sphere() else {
        return;
    };
    roughen(&mut plain);
    let mut untouched = SurfaceGeometry::new(&harness.gpu);
    untouched.sync(&harness.gpu, &mut plain).expect("the mesh");
    let rest = harness.capture(untouched.mesh(), &camera, false, "sdf-sym-rest");

    for tool in ToolKind::for_representation(Representation::Sdf) {
        // Máscara paints the freeze and Trim is a shape drawn on the view
        // frame; neither displaces clay, so neither has two sides to compare.
        if tool.is_mask_tool() || tool == ToolKind::Trim {
            continue;
        }
        let Some(mut document) = sphere() else {
            return;
        };
        roughen(&mut document);
        stroke(&mut document, tool, [true, false, false]);

        let mut geometry = SurfaceGeometry::new(&harness.gpu);
        geometry
            .sync(&harness.gpu, &mut document)
            .expect("the mesh");
        let name = match tool {
            ToolKind::Padrao => "padrao",
            ToolKind::Inflar => "inflar",
            ToolKind::Suavizar => "suavizar",
            ToolKind::Mover => "mover",
            ToolKind::MoverTopologico => "mover-topologico",
            ToolKind::Planar => "planar",
            ToolKind::Argila => "argila",
            ToolKind::Vinco => "vinco",
            ToolKind::Camada => "camada",
            ToolKind::Puxar => "puxar",
            ToolKind::Polir => "polir",
            ToolKind::Relaxar => "relaxar",
            other => panic!("{other:?} has no name here"),
        };
        let image = harness.capture(geometry.mesh(), &camera, false, &format!("sdf-sym-{name}"));
        let off = asymmetry(&image);
        // It has to have changed the outline at all, or the comparison below
        // passes on a sphere nobody touched.
        let moved = how_many_differ(&rest, &image);
        assert!(
            moved > 200,
            "{:?} changed {moved} pixels of the outline, so there is nothing \
             to compare between the two sides",
            tool
        );
        println!("{:<10} asymmetry {:.4} moved {moved}", tool.label(), off);
        worst.push((tool.label().to_string(), off));
    }

    // A twentieth of the covered pixels. Not zero: the marching-cubes surface
    // is not itself symmetric — the same dab moves a different set of vertices
    // on either side of a plane — so the outline wobbles by a pixel here and
    // there. What this catches is a brush whose work reached one side only,
    // which changes the outline on that side and nowhere else.
    let failed: Vec<&(String, f64)> = worst.iter().filter(|(_, off)| *off > 0.05).collect();
    assert!(
        failed.is_empty(),
        "these brushes did not come out on both sides: {failed:?}. See \
         target/visual/sdf-sym-*.png"
    );
}

#[test]
fn a_brush_asked_for_no_symmetry_stays_on_one_side() {
    // The control, and the second fault: without it the test above passes on a
    // brush that mirrors whether or not it was asked to — which the snakehook
    // did, because the starting form turns X on and nothing told the layer
    // otherwise.
    let Some(harness) = Harness::new() else {
        return;
    };
    let camera = head_on();
    // The two whose work a silhouette can resolve. The bake verbs move the
    // surface by about 0.006 — visible as a change of shading, not of outline
    // — so one-sidedness there is measured rather than looked at:
    // `sdf_brushes.rs` holds all nine with a raycast, which is the instrument
    // that can see it.
    for tool in [ToolKind::Padrao, ToolKind::Puxar] {
        let Some(mut document) = sphere() else {
            return;
        };
        roughen(&mut document);
        stroke(&mut document, tool, [false; 3]);
        let mut geometry = SurfaceGeometry::new(&harness.gpu);
        geometry
            .sync(&harness.gpu, &mut document)
            .expect("the mesh");
        let image = harness.capture(
            geometry.mesh(),
            &camera,
            false,
            &format!("sdf-nosym-{}", tool.label().to_lowercase()),
        );
        let off = asymmetry(&image);
        assert!(
            off > 0.01,
            "{:?} came out symmetric with symmetry switched off ({off:.4}), so \
             it is mirroring whatever the layer was last told rather than what \
             the sculptor asked for",
            tool
        );
    }
}

/// The profile of a mark: how far the surface stands out, and how wide the
/// raised footprint is.
///
/// Measured off the field itself rather than off a picture. Rays are cast
/// inward toward the centre over a patch of directions around the stroke, and
/// each hit's distance from the centre is the surface's radius there — the
/// starting form is a unit sphere, so anything past 1 is the mark. A picture
/// can differ by four hundred pixels and still look like the same brush; a
/// peak and a width cannot.
fn profile(document: &ClayDocument) -> (f32, usize) {
    const SPAN: f32 = 0.55;
    const STEPS: i32 = 40;
    let mut peak = 0.0f32;
    let mut footprint = 0usize;
    for iu in -STEPS..=STEPS {
        for iv in -STEPS..=STEPS {
            let (u, v) = (
                iu as f32 / STEPS as f32 * SPAN,
                iv as f32 / STEPS as f32 * SPAN,
            );
            // Around the stroke, which runs near the +x limb at z = 0.42.
            let direction = {
                let d = [0.88, v, 0.42 + u];
                let length = d.iter().map(|c| c * c).sum::<f32>().sqrt();
                [d[0] / length, d[1] / length, d[2] / length]
            };
            let origin = std::array::from_fn(|i| direction[i] * 6.0);
            let inward = direction.map(|c| -c);
            let Some(hit) = document.pick(origin, inward) else {
                continue;
            };
            let radius = hit.iter().map(|c| c * c).sum::<f32>().sqrt();
            peak = peak.max(radius);
            if radius > 1.02 {
                footprint += 1;
            }
        }
    }
    (peak, footprint)
}

/// Padrão and Inflar leave different marks on a field.
/// They did not: the engine's equivalence table binds both to relief, and the
/// application passed the same stamp with the same amplitude and rim for
/// either, so two brushes on the shelf drew one thing. Inflar swells now — a
/// wider region, a wider rim, a little less lift — where Padrão ridges.
#[test]
fn padrao_and_inflar_leave_different_marks_on_a_field() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let camera = head_on();
    let mut marks = Vec::new();
    for (tool, name) in [
        (ToolKind::Padrao, "sdf-mark-padrao"),
        (ToolKind::Inflar, "sdf-mark-inflar"),
    ] {
        let Some(mut document) = sphere() else {
            return;
        };
        stroke(&mut document, tool, [false, false, false]);
        let measured = profile(&document);
        let mut geometry = SurfaceGeometry::new(&harness.gpu);
        geometry
            .sync(&harness.gpu, &mut document)
            .expect("the mesh");
        harness.capture(geometry.mesh(), &camera, false, name);
        marks.push(measured);
    }
    let (ridge_peak, ridge_width) = marks[0];
    let (swell_peak, swell_width) = marks[1];
    // Height over the linear size of the footprint: the slope of the mark,
    // which is what tells a ridge from a swell. Comparing peaks alone would
    // pass for a mark that is simply bigger — which is what the first attempt
    // at this produced, wider *and* taller, and what the numbers in
    // `INFLATE_LIFT` record.
    let slope = |peak: f32, width: usize| (peak - 1.0) / (width as f32).sqrt().max(1.0);
    let (ridge, swell) = (
        slope(ridge_peak, ridge_width),
        slope(swell_peak, swell_width),
    );
    eprintln!(
        "Padrão +{:.4} over {ridge_width} samples, slope {ridge:.5}; \
         Inflar +{:.4} over {swell_width} samples, slope {swell:.5}",
        ridge_peak - 1.0,
        swell_peak - 1.0
    );

    assert!(
        ridge_width > 20 && swell_width > 20,
        "one of the brushes barely marked the surface: Padrão {ridge_width}, Inflar {swell_width}"
    );
    assert!(
        swell_width as f32 >= ridge_width as f32 * 1.3,
        "Inflar's footprint ({swell_width}) is not half again as wide as Padrão's \
         ({ridge_width}); the two brushes leave one mark"
    );
    assert!(
        swell < ridge * 0.85,
        "Inflar's mark is as steep as Padrão's ({swell:.5} against {ridge:.5}); \
         it should swell rather than ridge"
    );
}
