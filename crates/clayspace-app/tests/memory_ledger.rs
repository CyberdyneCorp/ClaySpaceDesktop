//! The memory ledger counts what this application holds to draw, and it
//! accounts for what the process is charged.
//!
//! An audited session reported 13 MB in use against a 26 GB footprint, and a
//! second 225–359 MB against up to 3.84 GB. The engine's ledger was not wrong,
//! it was partial — it walks the document and the surfaces handed to it — and
//! everything the viewport allocates was in no figure anyone read: the per-key
//! geometry it keeps, the buffers it uploads into, the staging a write takes,
//! the targets a frame is drawn into. These say that each of those now moves
//! the figure in use when it allocates, and gives it back when it is freed.

mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use clayspace_app::memory::{self, FOOTPRINT_MULTIPLE};
use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_view::{Camera, GpuMesh, OffscreenTarget, Vertex};
use support::Harness;

/// The system allocator, counting what is live.
///
/// So the geometry's figure can be checked against what was actually
/// allocated for it rather than against another estimate. Only this test
/// binary's Rust allocations pass through here — the engine allocates in C++
/// and the graphics driver in its own — which is exactly the share of the
/// process the store's figure claims to be.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call is forwarded unchanged to the system allocator; the
// counter is the only addition and it touches no memory the caller owns.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller's contract, passed through.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller's contract, passed through.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        LIVE.fetch_add(size, Ordering::Relaxed);
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller's contract, passed through.
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn live_heap() -> usize {
    LIVE.load(Ordering::Relaxed)
}

fn starting_form() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

fn framed() -> Camera {
    let mut camera = Camera::default();
    camera.frame_default();
    camera
}

#[test]
fn the_ledger_counts_surface_geometry() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = starting_form() else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    let before = memory::ledger(&document, memory::drawing(&geometry, &harness.gpu))
        .expect("the engine answers");

    let heap = live_heap();
    geometry
        .rebuild(&harness.gpu, &mut document)
        .expect("the starting form meshes");
    let allocated = live_heap().saturating_sub(heap) as f64;
    let after = memory::ledger(&document, memory::drawing(&geometry, &harness.gpu))
        .expect("the engine answers");

    let counted = after.drawing.geometry;
    assert!(counted > 0, "the starting form produced no geometry");
    assert!(
        after.in_use() >= before.in_use() + counted,
        "the figure in use moved by {} for {counted} bytes of geometry",
        after.in_use() - before.in_use()
    );
    // What the rebuild left allocated is the store and nothing else, so the
    // store's own figure should be that — within the hash tables' control
    // bytes and the buffers' small bookkeeping, which it does not count.
    let error = (counted as f64 - allocated).abs() / allocated;
    assert!(
        error < 0.15,
        "the store says it holds {counted} bytes; the rebuild left {allocated} allocated"
    );
}

#[test]
fn the_ledger_counts_gpu_buffers() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let gpu = &harness.gpu;
    let before = gpu.memory().buffers;

    let mut mesh = GpuMesh::new(gpu);
    assert!(mesh.reserve(gpu, 4096, 8192));
    let reserved = (4096 * Vertex::STRIDE + 8192 * 4) as u64;
    assert!(
        gpu.memory().buffers >= before + reserved,
        "{} bytes of buffers counted after reserving {reserved}",
        gpu.memory().buffers - before
    );

    // Grown into larger buffers, the smaller pair is given back as it is
    // dropped: what is counted is what is held, not what was ever allocated.
    assert!(mesh.reserve(gpu, 65_536, 131_072));
    let grown = (65_536 * Vertex::STRIDE + 131_072 * 4) as u64;
    let held = gpu.memory().buffers - before;
    assert!(
        held >= grown && held < grown + reserved,
        "{held} bytes counted for buffers of {grown}"
    );

    drop(mesh);
    assert_eq!(
        gpu.memory().buffers,
        before,
        "a dropped mesh's buffers are still counted"
    );
}

/// A write's staging is counted until the device can have finished with it,
/// and a wait on the device gives it all back.
#[test]
fn the_ledger_counts_staging_until_the_device_is_done_with_it() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let gpu = &harness.gpu;
    gpu.note_device_idle();
    let vertex = Vertex {
        position: [0.0; 3],
        normal: [0.0, 0.0, 1.0],
        color: [1.0; 3],
        mask: 0.0,
    };
    let vertices = vec![vertex; 300];
    let indices: Vec<u32> = (0..300).collect();
    let mut mesh = GpuMesh::new(gpu);
    mesh.upload(gpu, &vertices, &indices);
    let written = (300 * Vertex::STRIDE + 300 * 4) as u64;
    assert_eq!(gpu.memory().staging, written);

    harness.capture(&mesh, &framed(), true, "memory-ledger-staging");
    assert_eq!(
        gpu.memory().staging,
        0,
        "a capture waits on the device, which releases every write before it"
    );
}

#[test]
fn the_ledger_counts_capture_targets() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let gpu = &harness.gpu;
    let before = gpu.memory().targets;
    let target = OffscreenTarget::new(gpu, 256, 128);
    assert!(
        gpu.memory().targets >= before + 256 * 128 * 4,
        "a 256x128 capture target counted {} bytes",
        gpu.memory().targets - before
    );
    drop(target);
    assert_eq!(gpu.memory().targets, before);
}

/// The invariant the whole change is for: what the process is charged for a
/// document is what the ledger says the document holds.
///
/// The ten-times reference scene, built, meshed, uploaded and drawn, and the
/// footprint's growth over that held to [`FOOTPRINT_MULTIPLE`] times what the
/// ledger reports for it. Before this change the ledger was the engine's
/// figure alone — under a megabyte for this scene — against several hundred
/// megabytes of growth.
///
/// Growth from a baseline rather than the footprint whole, because what a
/// process costs at rest — code, driver, the first pipelines — is no
/// document's. And the scene is drawn once and thrown away *before* the
/// baseline is taken, because the first time a scene this size is drawn the
/// allocator and the driver grow pools that later ones reuse, and a freed
/// block the allocator has kept is charged to the process for as long as it
/// is kept. Measured on Apple silicon: the first round grew 2.4 times the
/// ledger and each round after it about 1.5 times.
#[test]
fn reported_memory_tracks_the_footprint() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let draw = |document: &mut ClayDocument| {
        let mut geometry = SurfaceGeometry::new(&harness.gpu);
        geometry
            .rebuild(&harness.gpu, document)
            .expect("the scene meshes");
        harness.capture(geometry.mesh(), &framed(), false, "memory-ledger-footprint");
        geometry
    };

    let Ok(mut warm) = Scene::TenTimesLarger.build(policy.clone()) else {
        return;
    };
    drop(draw(&mut warm));
    drop(warm);

    let Some(baseline) = memory::footprint() else {
        return;
    };
    let Ok(mut document) = Scene::TenTimesLarger.build(policy) else {
        return;
    };
    let geometry = draw(&mut document);
    let ledger = memory::ledger(&document, memory::drawing(&geometry, &harness.gpu))
        .expect("the engine answers");
    let grew = memory::footprint()
        .expect("read once already")
        .saturating_sub(baseline);
    let counted = ledger.in_use();
    println!(
        "footprint grew {grew}; ledger {counted} (engine {}, cache {}, {:?})",
        ledger.total, ledger.cache_bytes, ledger.drawing
    );

    assert!(
        grew <= counted * FOOTPRINT_MULTIPLE,
        "the process grew by {grew} bytes for a document the ledger says holds {counted}"
    );
    assert!(
        ledger.drawing.total() > ledger.total,
        "the scene's drawing should dwarf the engine's own figure, or this \
         fixture cannot tell the old ledger from the new one"
    );
}
