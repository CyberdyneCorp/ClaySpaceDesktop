//! What a stroke on a field leaves behind, as a sculptor sees it.
//!
//! Issue #128: Move Topológico on an SDF layer did not deform the surface, it
//! replaced a patch of it with a square of visible stair-stepping, hard-edged
//! against the untouched field. The defect was reported from ordinary use and
//! is visible in a frame, so it is measured in a frame here rather than in a
//! table.
//!
//! It was a hard `Op::Replace`, not the lattice the bake was sampled on:
//! `clay_item_volume_move_topological` rebuilds the volume and drops the
//! feather the bake asked for, so the placement tied with the field beneath it
//! and the shading rippled. `topological_move_stroke` bakes the region back out
//! feathered; this guard is what says so in a frame, and it is written over the
//! whole shelf rather than over that one row so the next tool that places a
//! hard replace fails here too.
//!
//! The measure is the mean neighbour-to-neighbour step over the lit pixels,
//! against the *same* number for an untouched sphere rendered in the same run.
//! A ratio rather than a level, because what a step is worth depends on the
//! device: only the comparison is ours.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_field_drag_quality --release -- --nocapture
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, Representation, SculptModel, ToolKind};
use clayspace_view::{Camera, Image};
use support::Harness;

/// The reported stroke: the brush, the intensity and the pull of the capture
/// in the issue.
const BRUSH: f32 = 0.35;
const PULL: f32 = 0.4;

/// How much rougher than an untouched sphere a stroked one may be.
///
/// Measured on this fixture, every tool the shelf offers on a field, as a
/// ratio against the untouched sphere in the same run:
///
/// | tool | ratio |
/// |------|------:|
/// | Suavizar, Relaxar | 1.01 |
/// | Planar, Polir | 1.02 |
/// | Mover | 1.10 |
/// | Camada | 1.13 |
/// | Inflar | 1.15 |
/// | Puxar, Vinco | 1.18 |
/// | **Mover Topológico**, repaired | **1.20** |
/// | Padrão | 1.30 |
/// | Argila | 1.31 |
/// | *Mover Topológico, as #128 shipped it* | *2.08* |
///
/// So this separates every brush that shapes the surface from the one that
/// placed a hard replace over it. The bar sits 15% above the worst tool that
/// shapes the surface and 39% below the one that did not; halving the margin
/// either way changes no verdict, which is what makes it a bar rather than a
/// tuned number.
const BAR: f64 = 1.5;

fn sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// The stroke, and its refusal if it refuses.
///
/// `None` only when there is no backend to sculpt on, which is the house idiom
/// for a machine without a device. A stroke that *errors* comes back as `Err`
/// and is reported: a tool this guard cannot render is a tool it is not
/// guarding, and swallowing that is how a guard quietly stops guarding.
fn stroked(tool: ToolKind) -> Option<Result<ClayDocument, String>> {
    let mut document = sphere()?;
    let samples: Vec<GestureSample> = (0..3)
        .map(|step| {
            let t = step as f32 / 2.0;
            GestureSample {
                position: [0.0, 0.0, 1.0 + t * PULL],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    let stroke = document.apply_stroke(
        tool,
        BrushSettings {
            size: BRUSH,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &samples,
        [false; 3],
    );
    Some(match stroke {
        Ok(_) => Ok(document),
        Err(error) => Err(error.to_string()),
    })
}

/// A camera on the touched cap rather than on the whole sphere.
///
/// Fixed rather than framed to the result: a tool that moves more material
/// would otherwise be looked at from further away, and the measure would be
/// reading its own framing instead of the surface.
fn close_up() -> Camera {
    let mut camera = Camera::default();
    camera.frame_bounds([-0.55, -0.55, 0.75].into(), [0.55, 0.55, 1.6].into());
    camera.yaw += 0.6;
    camera.pitch += 0.35;
    camera
}

/// Mean neighbour-to-neighbour step over the lit pixels.
///
/// Background is skipped, so a frame contributes its silhouette and nothing
/// else from the void; every frame here has one, which is why the comparison
/// is between frames rather than against zero.
fn roughness(image: &Image, background: [u8; 4]) -> f64 {
    let mut steps = 0u64;
    let mut counted = 0u64;
    for y in 1..image.height {
        for x in 1..image.width {
            let here = image.pixel(x, y);
            if (0..3).all(|c| here[c].abs_diff(background[c]) <= 12) {
                continue;
            }
            let left = image.pixel(x - 1, y);
            let up = image.pixel(x, y - 1);
            steps += (0..3)
                .map(|c| here[c].abs_diff(left[c]) as u64 + here[c].abs_diff(up[c]) as u64)
                .max()
                .unwrap_or(0);
            counted += 1;
        }
    }
    steps as f64 / counted.max(1) as f64
}

fn shot(harness: &mut Harness, name: &str, document: &mut ClayDocument) -> f64 {
    let background = harness.background();
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .rebuild(&harness.gpu, document)
        .expect("mesh the field");
    let image = harness.capture(geometry.mesh(), &close_up(), false, name);
    roughness(&image, background)
}

#[test]
fn no_brush_on_a_field_leaves_a_hard_replace_behind() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(mut clean) = sphere() else {
        return;
    };
    let baseline = shot(&mut harness, "field-quality-untouched", &mut clean);
    println!("\nuntouched sphere: {baseline:.2}\n");

    let mut offenders = Vec::new();
    for tool in ToolKind::for_representation(Representation::Sdf) {
        // Trim is drawn on the view frame rather than stroked across the
        // surface, and Máscara paints a freeze instead of moving anything:
        // neither has a stroked result for this to look at.
        if !tool.is_stroke_tool() || tool.is_mask_tool() {
            continue;
        }
        let Some(stroke) = stroked(tool) else {
            return;
        };
        let mut document = match stroke {
            Ok(document) => document,
            Err(error) => {
                println!("{:<20} refused: {error}", tool.label());
                offenders.push(format!("{} refused the stroke: {error}", tool.label()));
                continue;
            }
        };
        let name = format!("field-quality-{}", tool.key());
        let ratio = shot(&mut harness, &name, &mut document) / baseline;
        println!("{:<20} {ratio:.2}x", tool.label());
        if ratio > BAR {
            offenders.push(format!("{} at {ratio:.2}x", tool.label()));
        }
    }
    assert!(
        offenders.is_empty(),
        "the shelf offers these on a field and the stroke either refused or left \
         a surface more than {BAR}x rougher than the sphere it was drawn on, \
         which is what a hard replace looks like: {}",
        offenders.join(", ")
    );
}
