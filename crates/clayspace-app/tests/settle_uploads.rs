//! What a settle and a mask refresh write to the GPU, and what they say they
//! spent.
//!
//! A release used to force a full relayout after compaction, so every dab and
//! every undo re-uploaded the whole layer; a mask refresh rewrote every key
//! even with no mask to draw; each agent capture built a target of its own;
//! and the settle breakdown hard-coded its upload to zero, so that time landed
//! in "rest" where nothing could name it. These pin each of those.

mod support;

use clayspace_app::{SettleRoute, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};
use clayspace_view::{Camera, CaptureTargets, OffscreenTarget, Vertex};
use support::Harness;

fn sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

fn stroke(document: &mut ClayDocument, tool: ToolKind, at: [f32; 3], size: f32) {
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size,
                intensity: 0.9,
                ..BrushSettings::default()
            },
            &[
                GestureSample {
                    position: at,
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [at[0] + 0.02, at[1] + 0.01, at[2]],
                    pressure: 1.0,
                    time: 0.01,
                },
            ],
            [false; 3],
        )
        .expect("a stroke");
}

/// What the whole surface costs to upload, vertices and indices together.
fn layer_bytes(geometry: &SurfaceGeometry) -> u64 {
    (geometry.vertex_count() * Vertex::STRIDE + geometry.triangle_count() * 3 * 4) as u64
}

/// A released dab writes the keys its separate requests shared, not the layer.
#[test]
fn a_dab_uploads_only_its_keys() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = sphere() else {
        return;
    };
    let gpu = &harness.gpu;
    let mut geometry = SurfaceGeometry::new(gpu);
    geometry.rebuild(gpu, &mut document).expect("initial");
    let layer = layer_bytes(&geometry);

    // Several syncs make separate partial requests, which is what leaves the
    // duplicates a release compacts.
    for step in 0..4 {
        let x = -0.1 + step as f32 * 0.05;
        stroke(&mut document, ToolKind::Padrao, [x, 0.0, 1.0], 0.2);
        geometry.sync(gpu, &mut document).expect("sync");
    }
    assert!(geometry.needs_settle(), "the fixture owes no settle");
    gpu.take_uploaded_bytes();
    geometry
        .settle_after_edit(gpu, &mut document)
        .expect("release");
    let uploaded = gpu.take_uploaded_bytes();
    let cost = geometry.last_settle().expect("settle telemetry");
    assert_eq!(cost.route, SettleRoute::Compact);
    assert!(!geometry.needs_settle());
    println!("release uploaded {uploaded} of a {layer}-byte layer");
    assert!(
        uploaded * 10 < layer,
        "a single-dab release uploaded {uploaded} bytes of a {layer}-byte layer: \
         it rewrote the surface instead of the keys the dab touched"
    );

    // And what it wrote is the surface a rebuild would draw.
    let mut reference = SurfaceGeometry::new(gpu);
    reference.rebuild(gpu, &mut document).expect("reference");
    let mut actual = geometry.stored_triangles_exact();
    actual.sort_unstable();
    let mut expected = reference.stored_triangles_exact();
    expected.sort_unstable();
    assert!(actual == expected, "compaction changed the triangle set");
    let mut camera = Camera::default();
    camera.frame_default();
    let patched = harness.capture(geometry.mesh(), &camera, true, "settle-patched");
    let rebuilt = harness.capture(reference.mesh(), &camera, true, "settle-rebuilt");
    let background = harness.background();
    assert!(patched.pixels_differing_from(background, 8) > 1000);
    let differing = patched
        .pixels
        .chunks_exact(4)
        .zip(rebuilt.pixels.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|i| a[i].abs_diff(b[i]) > 8))
        .count();
    assert!(
        differing * 1000 < (patched.width * patched.height) as usize,
        "the patched upload draws {differing} pixels differently from a rebuild"
    );

    // A second release with nothing left to compact writes nothing.
    geometry
        .settle_after_edit(gpu, &mut document)
        .expect("release again");
    assert_eq!(gpu.take_uploaded_bytes(), 0);
}

/// Clearing a mask that is not there is zeroes over zeroes: nothing to write.
#[test]
fn an_empty_mask_refresh_uploads_nothing() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = sphere() else {
        return;
    };
    let gpu = &harness.gpu;
    let mut geometry = SurfaceGeometry::new(gpu);
    geometry.rebuild(gpu, &mut document).expect("initial");

    gpu.take_uploaded_bytes();
    geometry.refresh_mask(gpu, &document);
    assert_eq!(
        gpu.take_uploaded_bytes(),
        0,
        "a refresh with no mask rewrote the surface"
    );

    // A painted mask is written, and only once.
    stroke(&mut document, ToolKind::Mascara, [0.0, 0.0, 1.0], 0.3);
    assert!(document.has_mask(), "the fixture froze nothing");
    geometry.refresh_mask(gpu, &document);
    let painted = gpu.take_uploaded_bytes();
    assert!(painted > 0, "a painted mask was not uploaded");
    assert!(
        painted < layer_bytes(&geometry),
        "a patch of mask rewrote the whole layer"
    );
    geometry.refresh_mask(gpu, &document);
    assert_eq!(
        gpu.take_uploaded_bytes(),
        0,
        "an unchanged mask was uploaded again"
    );
}

/// A run of captures at one size draws into one target.
#[test]
fn captures_reuse_their_target() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let gpu = &harness.gpu;
    let mut captures = CaptureTargets::default();
    for _ in 0..8 {
        let target = captures.get(gpu, 64, 48, OffscreenTarget::FORMAT);
        assert_eq!((target.width(), target.height()), (64, 48));
    }
    assert_eq!(captures.created(), 1, "a repeated capture allocated again");
    let target = captures.get(gpu, 32, 32, OffscreenTarget::FORMAT);
    assert_eq!((target.width(), target.height()), (32, 32));
    captures.get(gpu, 32, 32, wgpu::TextureFormat::Bgra8UnormSrgb);
    assert_eq!(
        captures.created(),
        3,
        "a new size or format must be honoured"
    );
}

/// The parts of a settle are measured, and they account for its total.
#[test]
fn the_settle_breakdown_sums_to_the_total() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = sphere() else {
        return;
    };
    let gpu = &harness.gpu;
    let mut geometry = SurfaceGeometry::new(gpu);
    geometry.rebuild(gpu, &mut document).expect("initial");
    stroke(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0], 0.25);
    geometry.sync(gpu, &mut document).expect("sync");
    geometry.settle(gpu, &mut document).expect("settle");

    let cost = geometry.last_settle().expect("settle telemetry");
    assert_eq!(cost.route, SettleRoute::Bricks);
    assert!(
        cost.upload_time > std::time::Duration::ZERO,
        "a settle that wrote the surface reported no upload time"
    );
    assert!(cost.split_time > std::time::Duration::ZERO);
    assert!(cost.prune_time > std::time::Duration::ZERO);
    assert!(
        cost.parts() <= cost.total_time,
        "the parts {:?} exceed the total {:?}",
        cost.parts(),
        cost.total_time
    );
    let rest = cost.total_time - cost.parts();
    assert!(
        rest <= cost.total_time / 4 + std::time::Duration::from_millis(5),
        "{rest:?} of a {:?} settle is unaccounted for",
        cost.total_time
    );
}

/// The empty route reports its own engine time, never the previous settle's.
#[test]
fn an_empty_settle_does_not_inherit_the_last_engine_time() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = sphere() else {
        return;
    };
    let gpu = &harness.gpu;
    let mut geometry = SurfaceGeometry::new(gpu);
    geometry.settle(gpu, &mut document).expect("a real settle");
    assert!(geometry.last_settle().unwrap().engine_mesh_time > std::time::Duration::ZERO);

    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let mut empty = ClayDocument::new(policy).expect("an empty document");
    geometry.settle(gpu, &mut empty).expect("an empty settle");
    let cost = geometry.last_settle().unwrap();
    assert_eq!(cost.route, SettleRoute::Empty);
    assert_eq!(cost.engine_mesh_time, std::time::Duration::ZERO);
    assert_eq!(cost.read_time, std::time::Duration::ZERO);
    assert!(cost.parts() <= cost.total_time);
}
