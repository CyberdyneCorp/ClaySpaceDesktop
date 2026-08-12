//! How wide does the relax kernel have to be to actually smooth?
//!
//! `clay_relax_params.radius_cells` is an averaging radius in *cells*. The
//! viewport samples at 0.02, so a radius of 1 averages over 0.02 of world —
//! while the bumps a 0.12 brush leaves are five to twelve cells across. A
//! kernel that narrow cannot see the feature it is being asked to remove, and
//! all that reaches the screen is the damage from resampling the region.
//!
//! ```sh
//! cargo test -p clayspace-app --test relax_sweep --release -- --nocapture
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::claycore::{RelaxParams, VolumeParams};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use clayspace_view::{Camera, Image};
use support::Harness;

const CELL: f32 = 0.02;
const BRUSH: f32 = 0.18;

/// Roughness *inside* a window, not across the whole subject.
///
/// Measured over the whole frame this is dominated by the step at the edge of
/// the treated region: a stronger relax digs a deeper depression, so a sharper
/// rim, so a higher score — which reads as "smoothing makes it rougher" and is
/// an artefact of where the metric looks. The window sits well inside the
/// region so it sees the surface rather than its boundary.
fn roughness_in(image: &Image, background: [u8; 4], half: u32) -> f64 {
    let (cx, cy) = (image.width / 2, image.height / 2);
    let mut total = 0u64;
    let mut counted = 0u64;
    for y in cy.saturating_sub(half).max(1)..(cy + half).min(image.height) {
        for x in cx.saturating_sub(half).max(1)..(cx + half).min(image.width) {
            let here = image.pixel(x, y);
            if (0..3).all(|c| here[c].abs_diff(background[c]) <= 12) {
                continue;
            }
            let left = image.pixel(x - 1, y);
            let up = image.pixel(x, y - 1);
            let step = (0..3)
                .map(|c| here[c].abs_diff(left[c]) as u64 + here[c].abs_diff(up[c]) as u64)
                .max()
                .unwrap_or(0);
            total += step;
            counted += 1;
        }
    }
    if counted == 0 {
        return 0.0;
    }
    total as f64 / counted as f64
}

/// A sphere with a row of bumps on it, and a camera framing them.
fn roughness(image: &Image, background: [u8; 4]) -> f64 {
    roughness_in(image, background, 45)
}

fn bumpy() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    for i in 0..14 {
        let t = i as f32 / 13.0;
        let angle = (t - 0.5) * 1.1;
        let (s, c) = angle.sin_cos();
        let wobble = if i % 2 == 0 { 0.04 } else { -0.02 };
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.12,
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: [s * (1.0 + wobble), 0.1 + wobble, c * (1.0 + wobble)],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .ok()?;
    }
    Some(document)
}

fn framed(document: &ClayDocument) -> Camera {
    let mut camera = Camera::default();
    match SculptModel::bounds(document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }
    camera
}

/// Bakes the region the bumps occupy, relaxes it with the given kernel, and
/// puts it back — the same three steps `relax_stroke` takes.
fn relax_with(
    harness: &mut Harness,
    radius_cells: i32,
    iterations: i32,
    strength: f32,
    name: Option<&str>,
) -> Option<f64> {
    relax_banded(
        harness,
        radius_cells,
        iterations,
        strength,
        None,
        None,
        name,
    )
}

#[allow(clippy::too_many_arguments)]
fn relax_banded(
    harness: &mut Harness,
    radius_cells: i32,
    iterations: i32,
    strength: f32,
    band: Option<f32>,
    padding: Option<f32>,
    name: Option<&str>,
) -> Option<f64> {
    use clayspace_engine::claycore::Op;

    let mut document = bumpy()?;
    let camera = framed(&document);

    let min = [-0.7f32, -0.2, 0.4];
    let max = [0.7f32, 0.5, 1.3];
    let centre = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];

    let mut volume = document
        .document()
        .volume_from_region(
            VolumeParams {
                cell_size: Some(CELL),
                band,
                padding,
                // The sweep is about what relax itself does to a baked
                // volume, so the replace stays hard here: a feather would
                // change the control — bake and put straight back — from a
                // no-op into a crossfade, and the point of the control is
                // that it touches nothing.
                feather: None,
            },
            min,
            max,
        )
        .ok()?;
    // `iterations == 0` is the control: bake the region and put it straight
    // back, touching nothing. Whatever that costs is the round trip's, not the
    // verb's.
    if iterations > 0 {
        volume
            .relax(&RelaxParams {
                strength,
                radius_cells,
                iterations,
                centre,
                region_radius: BRUSH,
                falloff: BRUSH * 0.5,
                mask: None,
            })
            .ok()?;
    }
    volume.set_op(Op::Replace).ok()?;

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    document.add_volume_for_test(volume).ok()?;
    geometry.rebuild(&harness.gpu, &mut document).ok()?;
    let image = harness.capture(
        geometry.mesh(),
        &camera,
        false,
        name.unwrap_or("relax-sweep"),
    );
    Some(roughness(&image, harness.background()))
}

#[test]
fn how_wide_the_kernel_has_to_be() {
    let Some(mut harness) = Harness::new() else {
        return;
    };

    // The untouched bumps, for a baseline.
    let Some(mut document) = bumpy() else {
        return;
    };
    let camera = framed(&document);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, &mut document).expect("mesh");
    let before = harness.capture(geometry.mesh(), &camera, false, "relax-00-before");
    let baseline = roughness(&before, harness.background());
    println!("\nbumps as drawn: roughness {baseline:.2}");
    println!("brush {BRUSH} is {:.0} cells at {CELL}\n", BRUSH / CELL);

    // The control first: bake and replace, no relax.
    if let Some(untouched) = relax_with(&mut harness, 1, 0, 0.0, Some("relax-roundtrip-only")) {
        println!(
            "  bake and replace with no relax at all: {untouched:.2} (bumps were {baseline:.2})\n"
        );
    }

    // Does the band explain the round trip's damage? The header says a volume
    // reports a bound rather than a distance past the band it was sampled
    // with, and the default is three cells.
    println!("  {:>10} {:>10} {:>12}", "band", "padding", "roundtrip");
    for band in [None, Some(0.06f32), Some(0.12), Some(0.24), Some(0.5)] {
        for padding in [None, Some(0.12f32)] {
            let name = format!(
                "relax-band{}-pad{}",
                band.map_or("d".to_string(), |b| format!("{b}")),
                padding.map_or("d".to_string(), |p| format!("{p}"))
            );
            let Some(rough) = relax_banded(&mut harness, 1, 0, 0.0, band, padding, Some(&name))
            else {
                return;
            };
            println!(
                "  {:>10} {:>10} {rough:>12.2}",
                band.map_or("default".to_string(), |b| format!("{b}")),
                padding.map_or("default".to_string(), |p| format!("{p}"))
            );
        }
    }
    println!();

    println!(
        "  {:>7} {:>6} {:>6} {:>10}",
        "radius", "iters", "str", "roughness"
    );
    let mut best: Option<(i32, i32, f64)> = None;
    for radius_cells in [1, 3, 6, 9] {
        for iterations in [1, 2, 4] {
            let name = format!("relax-r{radius_cells}-i{iterations}");
            let Some(rough) = relax_with(&mut harness, radius_cells, iterations, 0.65, Some(&name))
            else {
                return;
            };
            println!(
                "  {radius_cells:>7} {iterations:>6} {:>6.2} {rough:>10.2}",
                0.65
            );
            if best.as_ref().is_none_or(|(_, _, b)| rough < *b) {
                best = Some((radius_cells, iterations, rough));
            }
        }
    }

    let (radius, iterations, rough) = best.expect("a sweep was run");
    println!("\n  best: radius {radius}, {iterations} iterations -> {rough:.2} against a baseline of {baseline:.2}\n");

    // No kernel smooths, and the reason is not the kernel: baking the region
    // and putting it straight back — no verb at all — already costs almost the
    // whole difference (11.11 against the bumps' 2.95, where the best kernel
    // manages 11.20). `clay_item_volume_relax` is being asked to work through
    // a round trip that corrugates the surface on its own, filed as ClayCore
    // #67.
    //
    // So what is pinned here is the shape of the problem rather than a target:
    // if a kernel ever beats the round trip's own cost, the verb has started
    // doing something the round trip is hiding, and this test should become an
    // assertion that it smooths.
    let (untouched, _, _) = (
        relax_with(&mut harness, 1, 0, 0.0, None).unwrap_or(f64::INFINITY),
        0,
        0,
    );
    assert!(
        rough >= untouched * 0.9,
        "a kernel now beats the round trip's own damage ({rough:.2} against \
         {untouched:.2}) — ClayCore #67 may be fixed, and Suavizar should be \
         re-tuned to actually smooth"
    );
    println!(
        "  the round trip alone costs {untouched:.2}; the best kernel {rough:.2}. \
         Relax is not the problem — see ClayCore #67.\n"
    );
}
