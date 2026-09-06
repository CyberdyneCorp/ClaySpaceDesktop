//! Cuts the minimal repro for the swept-curve pinhole, for ClayCore.
//!
//! Authors the curve items **directly** rather than through `Puxar`, so
//! nothing of our brush — the taper, the segmenting, the replay — is in the
//! picture. What is left is one question: does the engine's own mesh of a
//! document holding fat swept curves have holes in it?
//!
//! Writes the smallest reproducing document it finds, and prints the control
//! points, so the report travels with something that can be opened.
//!
//! Not for keeping.
//!
//! ```sh
//! cargo test -p clayspace-app --release --test snakehook_pinhole_repro -- --nocapture
//! ```

mod support;

use clayspace_engine::claycore::{Document, Item, Op, PointType};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::SculptModel;
use clayspace_view::{Camera, Image};
use support::Harness;

const POINT_KIND: PointType = PointType::Spline;
/// The brush the failing case used, and the radius it holds when a pull is
/// long enough that nothing tapers — which is the regime that pinholes.
const RADIUS: f32 = 0.12;

fn pinholes(image: &Image, background: [u8; 4]) -> Vec<(u32, u32)> {
    const REACH: i32 = 6;
    let is_background = |x: i32, y: i32| {
        if x < 0 || y < 0 || x >= image.width as i32 || y >= image.height as i32 {
            return true;
        }
        let p = image.pixel(x as u32, y as u32);
        p.iter()
            .zip(background)
            .all(|(a, b)| (i32::from(*a) - i32::from(b)).abs() <= 6)
    };
    let directions = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    let mut found = Vec::new();
    for y in 0..image.height as i32 {
        for x in 0..image.width as i32 {
            if !is_background(x, y) {
                continue;
            }
            if directions.iter().all(|(dx, dy)| {
                (1..=REACH).any(|step| !is_background(x + dx * step, y + dy * step))
            }) {
                found.push((x as u32, y as u32));
            }
        }
    }
    found
}

/// One tendril's control points: `visual_holes`'s own path, at a fixed radius.
fn tendril(index: usize, of: usize) -> Vec<f32> {
    let angle = index as f32 / of as f32 * std::f32::consts::TAU;
    let (sin, cos) = angle.sin_cos();
    let mut points = Vec::new();
    for step in 0..10 {
        let out = 1.0 + step as f32 * 0.08;
        points.extend_from_slice(&[cos * out * 0.7, sin * out * 0.7, out * 0.5, RADIUS]);
    }
    points
}

/// A sphere with `count` fat swept curves on it, built through the raw ABI.
fn document_with(count: usize) -> Option<(Document, ([f32; 3], [f32; 3]))> {
    let mut document = Document::new().ok()?;
    let layer = match document.layer_ids().ok()?.first() {
        Some(id) => *id,
        None => document.add_sdf_layer("Escultura").ok()?,
    };
    document.add_item(layer, &Item::sphere(1.0).ok()?).ok()?;
    for index in 0..count {
        let mut item = Item::stroke().ok()?;
        item.set_curve_points(&tendril(index, 6), POINT_KIND).ok()?;
        item.set_op(Op::Add).ok()?;
        item.set_stroke_blend_k(RADIUS * 0.5).ok()?;
        document.add_item(layer, &item).ok()?;
    }
    // Framed on what is actually there. The first cut of this probe framed a
    // bare starting form instead, which puts the capture at a different scale
    // and projection from the run that found the holes — and a two-pixel
    // artifact survives or vanishes on framing alone.
    let bounds = document.layer_bounds(layer).ok()??;
    Some((document, bounds))
}

#[test]
fn cut_the_smallest_document_that_still_shows_the_holes() {
    let Some(mut harness) = Harness::new() else {
        eprintln!("no gpu harness; skipping");
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let _ = policy;
    let background = harness.background();

    let mut smallest = None;
    for count in 1..=6 {
        let Some((document, bounds)) = document_with(count) else {
            continue;
        };
        let mut camera = Camera::default();
        camera.frame_bounds(bounds.0.into(), bounds.1.into());
        let mesh = support::mesh_document(&document, 96);
        let image = harness.capture_mesh(&mesh, &camera, &format!("repro-{count}-tendrils"));
        let holes = pinholes(&image, background);
        println!(
            "{count} tendril(s): {} pinholes in the engine's own mesh",
            holes.len()
        );
        if !holes.is_empty() && smallest.is_none() {
            smallest = Some((count, document, holes));
        }
    }

    let Some((count, document, holes)) = smallest else {
        println!("no count between 1 and 6 reproduced it");
        return;
    };
    let path = std::env::temp_dir().join("claycore-swept-curve-pinhole.clayspace");
    match document.save(&path) {
        Ok(()) => println!("\nwrote {count}-tendril repro to {}", path.display()),
        Err(e) => println!("\ncould not write the repro: {e}"),
    }
    println!("pinholes at {:?}", &holes[..holes.len().min(8)]);
    println!(
        "\ncontrol points of tendril 0 (x, y, z, radius), spline, blend k = {}:",
        RADIUS * 0.5
    );
    for point in tendril(0, 6).chunks(4) {
        println!(
            "  {:>9.5} {:>9.5} {:>9.5}   r {:.5}",
            point[0], point[1], point[2], point[3]
        );
    }
}
