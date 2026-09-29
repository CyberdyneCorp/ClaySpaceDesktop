//! An adaptive surface drawn chunk by chunk (#209).
//!
//! What a stroke sends to the viewport follows what the engine marked dirty,
//! and what kind of change each chunk saw: a move sends vertices, a remesh
//! sends indices as well. These tests drive the surface through its own
//! sculptor so the topology policy can be pinned — the document's strokes
//! always run the engine's default, which remeshes — and read the patch the
//! drawn region is brought up to date with.

use clayspace_engine::adaptive::{Adaptive, RegionPatch};
use clayspace_engine::chunked::RegionRuns;
use clayspace_engine::claycore::{self, DynamicTopology, MeshBrush, MeshStamp};

/// A flat sheet of triangles facing +y, `divisions` quads a side.
fn sheet(divisions: usize, half: f32) -> claycore::Mesh {
    let stride = divisions + 1;
    let step = 2.0 * half / divisions as f32;
    let positions: Vec<[f32; 3]> = (0..stride)
        .flat_map(|z| {
            (0..stride).map(move |x| [-half + step * x as f32, 0.0, -half + step * z as f32])
        })
        .collect();
    let indices: Vec<u32> = (0..divisions)
        .flat_map(|z| (0..divisions).map(move |x| (z * stride + x) as u32))
        .flat_map(|a| {
            let (b, c, d) = (a + 1, a + stride as u32, a + stride as u32 + 1);
            [a, c, b, b, c, d]
        })
        .collect();
    claycore::Mesh::from_triangles(&positions, &indices).expect("a sheet")
}

fn dab(radius: f32) -> MeshStamp<'static> {
    MeshStamp {
        verb: MeshBrush::Draw,
        center: [0.0, 0.0, 0.0],
        radius,
        strength: 0.5,
        ..MeshStamp::default()
    }
}

/// One stamp through the surface's own sculptor, returning how many chunks
/// the engine marked dirty.
fn stamp(adaptive: &mut Adaptive, radius: f32, topology: &DynamicTopology) -> usize {
    adaptive.with_sculptor(|sculptor| {
        sculptor
            .stamp(dab(radius), Some(topology), None)
            .expect("the stamp lands");
        sculptor.dirty_chunks().expect("the dirty set").len()
    })
}

fn runs(patch: RegionPatch) -> RegionRuns {
    match patch {
        RegionPatch::Runs(runs) => runs,
        other => panic!("the region should take the change in place: {other:?}"),
    }
}

fn drawn(size: usize) -> Adaptive {
    let mut adaptive = Adaptive::from_mesh(&sheet(size, 2.0)).expect("a sheet reads");
    assert!(
        adaptive.is_chunked(),
        "an uncoloured surface is drawn in chunks"
    );
    adaptive.lay_out_region(0, 0).expect("a region");
    adaptive
}

/// A move with the topology held sends the chunks it moved, their vertices
/// only: the triangles are the same triangles.
#[test]
fn a_geometry_change_uploads_no_indices() {
    let mut adaptive = drawn(64);
    let still = DynamicTopology {
        enabled: false,
        ..DynamicTopology::default()
    };
    let dirty = stamp(&mut adaptive, 0.4, &still);
    assert!(dirty > 0, "the dab touched something");

    let runs = runs(adaptive.patch_region());
    assert_eq!(runs.chunks, dirty, "exactly the chunks the engine marked");
    assert_eq!(runs.vertices.len(), dirty);
    assert!(
        runs.indices.is_empty(),
        "no index is sent when the topology did not move: {} runs",
        runs.indices.len()
    );
    assert_eq!(runs.census[0], runs.census[1], "and nothing was added");
}

/// A remeshing dab sends indices for the chunks it re-cut, and still only
/// for chunks the engine marked.
#[test]
fn a_topology_change_uploads_indices_for_the_chunks_it_recut() {
    let mut adaptive = drawn(64);
    let splitting = DynamicTopology {
        enabled: true,
        detail_mode: claycore::DetailMode::World,
        target_edge_length: 0.04,
        ..DynamicTopology::default()
    };
    let dirty = stamp(&mut adaptive, 0.25, &splitting);
    let runs = runs(adaptive.patch_region());
    assert!(!runs.indices.is_empty(), "a re-cut chunk sends its indices");
    assert!(runs.chunks <= dirty);
    assert!(
        runs.census[1].0 > runs.census[0].0,
        "the dab made triangles"
    );
}

/// Chunk buffers are kept: a stroke that does not grow the surface
/// allocates nothing on this side, and a second drain of the same chunks
/// reuses what the first sized.
#[test]
fn a_stroke_that_does_not_grow_the_surface_allocates_nothing() {
    let mut adaptive = drawn(64);
    let growths = adaptive.buffer_growths();
    let still = DynamicTopology {
        enabled: false,
        ..DynamicTopology::default()
    };
    for _ in 0..4 {
        stamp(&mut adaptive, 0.4, &still);
        let runs = runs(adaptive.patch_region());
        assert!(runs.indices.is_empty(), "no slot moved, none was re-cut");
    }
    assert_eq!(
        adaptive.buffer_growths(),
        growths,
        "four moves of the same chunks grew no buffer"
    );
}

/// Nothing marked, nothing sent: a region with no dirty chunk patches to
/// empty runs, which is what a resting frame and a camera move cost.
#[test]
fn a_surface_nobody_touched_sends_nothing() {
    let mut adaptive = drawn(32);
    let runs = runs(adaptive.patch_region());
    assert!(runs.vertices.is_empty() && runs.indices.is_empty());
    assert_eq!(runs.chunks, 0);
}

/// A dab that grows the surface past the region's spare room is not
/// squeezed in: the region asks to be laid out again, and the new layout
/// draws every face the surface holds.
#[test]
fn a_growth_past_the_spare_room_is_laid_out_again() {
    let mut adaptive = drawn(64);
    let dense = DynamicTopology {
        enabled: true,
        detail_mode: claycore::DetailMode::World,
        target_edge_length: 0.008,
        max_ops_per_stamp: 1_000_000,
        ..DynamicTopology::default()
    };
    stamp(&mut adaptive, 0.8, &dense);
    assert_eq!(adaptive.patch_region(), RegionPatch::Rebuild);
    let region = adaptive.lay_out_region(0, 0).expect("a region");
    assert_eq!(region.census.0 as u64, adaptive.stats().faces);
}
