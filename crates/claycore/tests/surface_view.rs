//! The one transport, at the engine boundary.
//!
//! Three properties are the whole reason this seam exists beside the shipped
//! per-representation ones, and each of the three has a test here written to
//! fail if the property is flattened away rather than to watch a call return
//! `Ok`:
//!
//! - **Four revisions, not one.** `a_dab_moves_geometry_and_leaves_the_index_-
//!   buffer_alone` is a host deciding not to re-upload an index buffer, which
//!   is a decision a single dirty counter cannot support.
//! - **An acknowledgement, not an all-or-nothing clear.**
//!   `half_a_drained_set_is_retired_and_the_rest_is_still_waiting` drains half
//!   a set, acknowledges exactly that half, and asks again: what comes back is
//!   the un-acked half and not the whole set, which is the frame-drop a host
//!   must survive without either re-uploading everything or losing a change.
//!   `a_chunk_that_changed_again_stays_dirty` is the other half of the same
//!   claim.
//! - **A stale readback says so.** `a_readback_says_when_the_plan_is_out_of_-
//!   date` asks for a revision the engine has moved past and reads both
//!   numbers back.

use claycore::{
    ChunkOptions, Mesh, MeshBrush, MeshStamp, Multires, MultiresDesc, SurfaceKind, SurfaceView,
};

// -- fixtures ---------------------------------------------------------------

/// A flat grid of quads: what a Catmull-Clark cage is supposed to be, and
/// large enough that a level of it partitions into several chunks.
fn cage(divisions: usize, name: &str) -> Mesh {
    let mut text = String::new();
    let half = 2.0f32;
    let step = 2.0 * half / divisions as f32;
    for z in 0..=divisions {
        for x in 0..=divisions {
            text.push_str(&format!(
                "v {} 0 {}\n",
                -half + step * x as f32,
                -half + step * z as f32
            ));
        }
    }
    let stride = divisions + 1;
    for z in 0..divisions {
        for x in 0..divisions {
            // Wound so the sheet faces +y, which makes a Draw stamp a bump.
            let a = z * stride + x + 1;
            text.push_str(&format!(
                "f {} {} {} {}\n",
                a,
                a + stride,
                a + stride + 1,
                a + 1
            ));
        }
    }
    let path =
        std::env::temp_dir().join(format!("claycore-view-{name}-{}.obj", std::process::id()));
    std::fs::write(&path, text).expect("write the cage");
    let mesh = Mesh::load(&path).expect("load the cage");
    let _ = std::fs::remove_file(&path);
    mesh
}

fn hierarchy(levels: u32, name: &str) -> Multires {
    let mesh = cage(8, name);
    let mut surface = Multires::from_mesh(&mesh, MultiresDesc::default()).expect("a hierarchy");
    for _ in 0..levels {
        surface.add_level().expect("subdivide");
    }
    surface
}

/// One Draw dab at a level, through a sculptor that is dropped afterwards so
/// the surface is free for a view again.
fn dab(surface: &mut Multires, level: u32, center: [f32; 3], radius: f32) {
    let mut sculptor = surface.sculptor().expect("sculptor");
    sculptor
        .surface_mut()
        .set_sculpt_level(level)
        .expect("bind the level");
    sculptor.begin_stroke().expect("begin");
    let moved = sculptor
        .stamp(
            MeshStamp {
                verb: MeshBrush::Draw,
                center,
                radius,
                strength: 0.3,
                geodesic: false,
                ..Default::default()
            },
            None,
        )
        .expect("stamp");
    assert!(moved.moved_vertices > 0, "the dab reached nothing to move");
}

/// A hierarchy whose level has been looked at once, with a clean dirty set.
///
/// A level's chunk table comes into existence when the level is first viewed,
/// and a stamp made before that has nothing to mark — a host that sculpts a
/// level it has never drawn sees an empty dirty set, which is correct and
/// surprising. Priming is what a real host does by drawing a frame; it is
/// explicit here so that an empty set in the tests below means the drain
/// worked rather than that the table was never built.
fn primed(levels: u32, level: u32, name: &str) -> Multires {
    let mut surface = hierarchy(levels, name);
    {
        let mut view = SurfaceView::over_multires(&mut surface, level).expect("a view");
        let count = view.chunk_count();
        let _ = view.chunk_infos_in_order(count).expect("infos");
        view.clear_dirty().expect("start clean");
    }
    surface
}

/// The dirty ids of one level, sorted, so two drains compare as sets.
fn dirty(surface: &mut Multires, level: u32) -> Vec<u32> {
    let mut view = SurfaceView::over_multires(surface, level).expect("a view");
    let mut ids = view.dirty_chunks().expect("dirty");
    ids.sort_unstable();
    ids
}

// -- the partition ----------------------------------------------------------

#[test]
fn a_flat_mesh_partitions_and_reports_a_chunk_that_can_be_copied() {
    let mesh = cage(8, "flat");
    let mut view = SurfaceView::over_mesh(&mesh, None).expect("a view over a mesh");

    assert_eq!(view.kind(), SurfaceKind::Fixed);
    assert!(view.kind().is_welded(), "a fixed mesh copies welded");

    let count = view.chunk_count();
    assert!(count > 0, "a mesh with faces partitioned into no chunks");

    let infos = view.chunk_infos_in_order(count).expect("infos");
    assert_eq!(infos.len(), count);
    assert!(infos.iter().any(|c| c.live), "no chunk of the mesh is live");

    // A static mesh's view carries no dirty set: it reports one partition and
    // nothing changes under it.
    assert!(
        view.dirty_chunks().expect("dirty").is_empty(),
        "a mesh nobody has sculpted reported changed chunks"
    );

    let first = infos.iter().find(|c| c.live).expect("a live chunk");
    let copy = view.copy_chunk(first.chunk, None).expect("copy");
    assert_eq!(
        copy.positions.len(),
        copy.readback.vertex_count as usize * 3,
        "three floats per vertex, and the buffer was sized from the engine's \
         own count"
    );
    assert_eq!(copy.normals.len(), copy.positions.len());
    assert_eq!(copy.indices.len(), copy.readback.index_count as usize);
    assert!(
        copy.indices.iter().all(|&i| i < copy.readback.vertex_count),
        "a welded chunk's indices are local to its own vertex list"
    );
    assert!(
        !copy.readback.stale,
        "a first readback of a chunk cannot be stale: nothing was asked for"
    );
}

/// A capacity query writes nothing and says what the chunk needs, so a host
/// sizes a pool once rather than growing it per frame.
#[test]
fn asking_what_a_chunk_needs_is_not_copying_it() {
    let mesh = cage(8, "capacity");
    let mut view = SurfaceView::over_mesh(&mesh, None).expect("a view");
    let count = view.chunk_count();
    let infos = view.chunk_infos_in_order(count).expect("infos");
    let first = infos.iter().find(|c| c.live).expect("a live chunk");

    let sized = view.chunk_capacity(first.chunk).expect("capacity");
    assert_eq!(
        (sized.vertex_count, sized.index_count),
        (first.vertex_count, first.index_count),
        "the info and the capacity query disagree about the same chunk"
    );

    let copy = view.copy_chunk(first.chunk, None).expect("copy");
    assert_eq!(
        (copy.readback.vertex_count, copy.readback.index_count),
        (sized.vertex_count, sized.index_count),
        "the copy needed something other than what the query promised, which \
         is the whole reason a host would size from it"
    );
}

/// The one case where the caller chooses the partition, because a fixed mesh
/// has no partitioner of its own.
#[test]
fn a_smaller_chunk_target_partitions_the_same_mesh_more_finely() {
    let mesh = cage(8, "options");

    let defaults = ChunkOptions::defaults().expect("the library's own");
    assert!(
        defaults.min_faces <= defaults.target_faces && defaults.target_faces <= defaults.max_faces,
        "the measured defaults are 0 < min <= target <= max"
    );

    let coarse = SurfaceView::over_mesh(&mesh, Some(defaults))
        .expect("a view")
        .chunk_count();
    let fine = SurfaceView::over_mesh(
        &mesh,
        Some(ChunkOptions {
            target_faces: 8,
            min_faces: 2,
            max_faces: 16,
        }),
    )
    .expect("a view")
    .chunk_count();

    assert!(
        fine > coarse,
        "eight faces a chunk gave {fine} chunks against {coarse} at the \
         library's own {} — the options did not reach the partitioner",
        defaults.target_faces
    );
}

#[test]
fn a_hierarchy_level_is_the_same_transport_over_a_different_surface() {
    let mut surface = hierarchy(2, "level");
    let mut view = SurfaceView::over_multires(&mut surface, 2).expect("a view");

    assert_eq!(view.kind(), SurfaceKind::Multires);
    assert!(view.kind().is_welded());
    assert!(view.chunk_count() > 0);
    drop(view);

    // A level that is gone is a refusal rather than an empty view: a host that
    // got one back would draw nothing and be told nothing.
    assert!(
        SurfaceView::over_multires(&mut surface, 9).is_err(),
        "a view was handed out over a level this hierarchy does not have"
    );
}

// -- four revisions, not one ------------------------------------------------

/// The decision a single dirty counter cannot support. A Draw dab moves
/// positions and creates no face, so a host that reads the four apart
/// re-uploads a vertex buffer and leaves the index buffer where it is.
#[test]
fn a_dab_moves_geometry_and_leaves_the_index_buffer_alone() {
    let mut surface = hierarchy(2, "revisions");

    let (chunk, before) = {
        let mut view = SurfaceView::over_multires(&mut surface, 2).expect("a view");
        view.clear_dirty().expect("start clean");
        let count = view.chunk_count();
        let infos = view.chunk_infos_in_order(count).expect("infos");
        let first = infos.iter().find(|c| c.live).expect("a live chunk");
        (first.chunk, first.revisions)
    };

    dab(&mut surface, 2, [0.0, 0.0, 0.0], 4.0);

    let mut view = SurfaceView::over_multires(&mut surface, 2).expect("a view");
    let after = view.chunk_infos(&[chunk]).expect("infos")[0];

    assert!(
        after.geometry_dirty,
        "the dab moved vertices of this chunk and it is not marked"
    );
    assert!(
        after.revisions.geometry > before.geometry,
        "the geometry counter did not move for a stamp that moved positions"
    );
    assert_eq!(
        after.revisions.topology, before.topology,
        "a Draw stamp creates no face, and a host that re-uploaded an index \
         buffer here would be paying for a change that did not happen"
    );
    assert!(
        !after.topology_dirty,
        "the topology is marked dirty for a stamp that changed no membership"
    );
    assert!(
        after.revision >= after.revisions.geometry,
        "the shipped single counter is the maximum of the four"
    );
}

// -- the acknowledgement ----------------------------------------------------

/// The frame a host drops. Drain half a set, acknowledge exactly that half,
/// and ask again: the un-acked half comes back and the acknowledged half does
/// not. Neither re-uploading everything nor losing a change.
#[test]
fn half_a_drained_set_is_retired_and_the_rest_is_still_waiting() {
    let mut surface = primed(2, 2, "half");
    dab(&mut surface, 2, [0.0, 0.0, 0.0], 4.0);

    let all = dirty(&mut surface, 2);
    assert!(
        all.len() >= 4,
        "the fixture has to give a set worth halving; it gave {}",
        all.len()
    );

    let (drained, waiting) = all.split_at(all.len() / 2);
    let clean = {
        let mut view = SurfaceView::over_multires(&mut surface, 2).expect("a view");
        // What a host actually does with the half it had budget for: copy it,
        // upload it, and acknowledge exactly what it copied.
        let acks: Vec<_> = drained
            .iter()
            .map(|&chunk| {
                let copy = view.copy_chunk(chunk, None).expect("copy");
                assert!(!copy.positions.is_empty(), "a dirty chunk copied empty");
                copy.ack()
            })
            .collect();
        view.acknowledge(&acks).expect("acknowledge")
    };
    assert_eq!(
        clean,
        drained.len(),
        "every chunk that was copied and had not moved since should be clean"
    );

    let left = dirty(&mut surface, 2);
    assert_eq!(
        left, waiting,
        "asking again after acknowledging half returned {:?} where the \
         un-acked half is {:?} — a host either re-uploads what it already \
         did or never hears about the rest",
        left, waiting
    );
}

/// The other half of the same claim, and the one a green suite would hide: a
/// chunk that changed *again* between the copy and the acknowledgement is not
/// retired, because the revision it was copied at is no longer current.
#[test]
fn a_chunk_that_changed_again_stays_dirty() {
    let mut surface = primed(2, 2, "restamp");
    dab(&mut surface, 2, [0.0, 0.0, 0.0], 4.0);

    let all = dirty(&mut surface, 2);
    let chunk = *all.first().expect("a dirty chunk");

    // The copy a host made, and then the frame it dropped.
    let stale_ack = {
        let mut view = SurfaceView::over_multires(&mut surface, 2).expect("a view");
        view.copy_chunk(chunk, None).expect("copy").ack()
    };

    // The surface moved again while that upload was in flight.
    dab(&mut surface, 2, [0.0, 0.0, 0.0], 4.0);

    let mut view = SurfaceView::over_multires(&mut surface, 2).expect("a view");
    let clean = view.acknowledge(&[stale_ack]).expect("acknowledge");
    assert_eq!(
        clean, 0,
        "a chunk that changed after it was copied was retired anyway, and the \
         change it was carrying is now lost with nothing saying so"
    );
    let mut left = view.dirty_chunks().expect("dirty");
    left.sort_unstable();
    assert!(
        left.contains(&chunk),
        "chunk {chunk} was acknowledged at a revision it has moved past and \
         is no longer waiting"
    );
}

/// The all-or-nothing form is still here, and still means what it says. It is
/// what a host that uploads everything in one frame wants, and exactly what a
/// host draining incrementally must not use.
#[test]
fn clearing_the_set_retires_everything_at_once() {
    let mut surface = primed(2, 2, "clear");

    dab(&mut surface, 2, [0.0, 0.0, 0.0], 4.0);
    assert!(!dirty(&mut surface, 2).is_empty());

    {
        let mut view = SurfaceView::over_multires(&mut surface, 2).expect("a view");
        view.clear_dirty().expect("clear");
    }
    assert!(
        dirty(&mut surface, 2).is_empty(),
        "the all-or-nothing clear left something behind"
    );
}

/// An empty acknowledgement is not an error and retires nothing — the
/// degenerate case of a budget that ran out before the first chunk.
#[test]
fn acknowledging_nothing_retires_nothing() {
    let mut surface = primed(2, 2, "empty");
    dab(&mut surface, 2, [0.0, 0.0, 0.0], 4.0);
    let before = dirty(&mut surface, 2);

    let mut view = SurfaceView::over_multires(&mut surface, 2).expect("a view");
    assert_eq!(view.acknowledge(&[]).expect("nothing"), 0);
    drop(view);

    assert_eq!(dirty(&mut surface, 2), before);
}

// -- staleness --------------------------------------------------------------

/// A superseded readback is identifiable rather than merely wrong. The data
/// written is current — this is not a failure — but a host applying an older
/// frame's plan can tell that its plan is out of date, which nothing in the
/// pixels would say.
#[test]
fn a_readback_says_when_the_plan_is_out_of_date() {
    let mut surface = primed(2, 2, "stale");

    dab(&mut surface, 2, [0.0, 0.0, 0.0], 4.0);
    let all = dirty(&mut surface, 2);
    let chunk = *all.first().expect("a dirty chunk");

    let seen = {
        let mut view = SurfaceView::over_multires(&mut surface, 2).expect("a view");
        let copy = view.copy_chunk(chunk, None).expect("copy");
        assert!(!copy.readback.stale);
        assert_eq!(
            copy.readback.requested, copy.readback.current,
            "a readback that asked for nothing echoes what it got: the pair \
             is equal on a fresh readback, which is what makes `stale` \
             readable without a special case for the first frame"
        );
        copy.readback.current
    };

    dab(&mut surface, 2, [0.0, 0.0, 0.0], 4.0);

    let mut view = SurfaceView::over_multires(&mut surface, 2).expect("a view");
    let again = view.copy_chunk(chunk, Some(seen)).expect("copy");
    assert!(
        again.readback.stale,
        "the engine moved on after the caller's snapshot and the readback did \
         not say so"
    );
    assert_eq!(
        again.readback.requested, seen,
        "the revision asked for has to come back beside what the engine is at \
         now, or a host cannot tell which of its plans this answers"
    );
    assert!(
        again.readback.current.geometry > seen.geometry,
        "the readback says stale and reports the same revision it was asked \
         for, which is a contradiction"
    );
    assert!(
        !again.positions.is_empty(),
        "a stale readback still writes the current data: it is a warning, not \
         a failure"
    );
}
