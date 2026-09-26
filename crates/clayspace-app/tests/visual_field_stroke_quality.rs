//! What a stroke on a field leaves behind, as a sculptor sees it.
//!
//! **A gate on normal discontinuity, which no distance check covers.** Issue
//! #128: Move Topológico on an SDF layer put the surface in the right place —
//! 0.3832 of a 0.4 pull, correct to three decimals — and destroyed its normals,
//! replacing a patch with the lattice the bake was sampled on. Every gate this
//! application had compared distances, so all of them were green. The defect
//! was found by a sculptor looking at the screen.
//!
//! The measure is the mean neighbour-to-neighbour step over the lit pixels of a
//! rendered frame, against the *same* number for an untouched sphere rendered
//! in the same run. A ratio rather than a level, because what a step is worth
//! depends on the device: only the comparison is ours.
//!
//! The rule this exists to enforce, which generalises past this one tool: **for
//! every field a params struct carries, ask what quantity it controls and check
//! that something gates on it.** Feather controls normals; band controls extent;
//! cell size controls both. A suite that compares distances can only ever catch
//! the fields that move distances. ClayCore took the same rule upstream as their
//! #596 after this found their defect.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_field_stroke_quality --release -- --nocapture
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
/// Measured on this fixture against ClayCore v0.116.0, every tool the shelf
/// offers on a field, as a ratio against the untouched sphere in the same run:
///
/// | tool | ratio |
/// |------|------:|
/// | Suavizar | 1.01 |
/// | Planar | 1.02 |
/// | Pinçar | 1.07 |
/// | Mover | 1.12 |
/// | Camada | 1.13 |
/// | Inflar | 1.15 |
/// | Puxar, Vinco | 1.18 |
/// | Mover Topológico | 1.20 |
/// | Padrão | 1.30 |
/// | Argila | 1.31 |
///
/// Relaxar and Polir read 1.01 and 1.02 beside these two, being the same bakes,
/// and are off the field's shelf since #203.
///
/// Pinçar is on this table for the first time: #201 gave it a field verb — the
/// engine's radial scale at a negative strength — so a field has one more mark
/// for this to look at. It sits among the gentlest marks here, which is
/// what a scale of the assembled surface should leave: it moves the surface it
/// found rather than combining a new item with it. Every other reading is the
/// one this file already had, Inflar included — it is still relief with a wider
/// region — and Suavizar reads 1.01 beside Relaxar rather than 1.02 beside the
/// planing pair, which is the row being re-read rather than anything moving.
///
/// **Mover Topológico read 2.07 on v0.113.0**, which is the defect this file
/// was written for, and 1.20 here. The repair was one line upstream — the
/// `FieldVolume` overload of `field::move_topological` re-samples through a
/// callable and did not carry the source volume's feather onto the result, so
/// the caller's feather was discarded by the verb (ClayCore #593). Nothing
/// changed on this side.
///
/// The bar separates every brush that shapes a surface from one that replaced
/// it with a grid: the worst good tool sits 15% below it and the defect sat 38%
/// above. Halving the margin either way changes no verdict, which is what makes
/// it a bar rather than a tuned number.
const BAR: f64 = 1.5;

fn sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

fn stroked(tool: ToolKind) -> Option<ClayDocument> {
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
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size: BRUSH,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .ok()?;
    Some(document)
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
fn no_brush_on_a_field_leaves_the_lattice_behind() {
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
        let Some(mut document) = stroked(tool) else {
            return;
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
        "the shelf offers these on a field and the stroke leaves a surface more \
         than {BAR}x rougher than the sphere it was drawn on, which is what a \
         replaced lattice looks like: {}",
        offenders.join(", ")
    );
}
