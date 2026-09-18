//! The adaptive surface, at the engine boundary.
//!
//! The tier's claim is one sentence: connectivity changes under the brush, and
//! a host draws the result without re-uploading the model. Those are two
//! separate things to check and this file is written around both.
//! `a_stamp_changes_connectivity_and_says_so` is the first — it asks the report
//! what it split and compares the census before and after, rather than reading
//! `Ok(())`. The second is the copy that stays inside a pooled buffer, and it
//! is the one the renderer's correctness rests on: a copy running past a
//! chunk's own extent would corrupt whatever shares the pool, and the symptom
//! is geometry from somewhere else on the model.
//!
//! Everything else follows this crate's rule that a wrapper nobody runs is a
//! SAFETY comment nobody has checked: every entry point `dynamic.rs` wraps is
//! called at least once, and the assertions are about consequences — how far a
//! vertex moved, which chunks went dirty, what a refusal *named*.

use claycore::{
    DetailMode, DynamicDesc, DynamicError, DynamicSurface, DynamicTopology, MaintenanceKind,
    MaintenanceQueue, Mask, MeshBrush, MeshStamp, Pressure, SculptMemoryProfile, SculptStage,
    SurfaceKind, SurfaceView, WorldFrame,
};

// -- fixtures ---------------------------------------------------------------

/// A flat sheet of triangles, `divisions` quads a side, centred on the origin
/// and facing +y.
///
/// Built through `Mesh::from_triangles` rather than through a file, because
/// nothing here needs an importer and a temporary path is a race two tests in
/// one binary can lose. The sheet is open — it has a boundary — which is on
/// purpose: a half-edge structure expresses a boundary edge, and a fixture
/// that was closed would leave `boundary_edges` untested at zero.
fn sheet(divisions: usize, half: f32) -> claycore::Mesh {
    let stride = divisions + 1;
    let step = 2.0 * half / divisions as f32;
    let mut positions = Vec::with_capacity(stride * stride);
    for z in 0..stride {
        for x in 0..stride {
            positions.push([-half + step * x as f32, 0.0, -half + step * z as f32]);
        }
    }
    let mut indices = Vec::with_capacity(divisions * divisions * 6);
    for z in 0..divisions {
        for x in 0..divisions {
            let a = (z * stride + x) as u32;
            let (b, c, d) = (a + 1, a + stride as u32, a + stride as u32 + 1);
            // Wound so the sheet faces +y, which is what makes a Draw stamp
            // read as a bump rather than as a dent.
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    claycore::Mesh::from_triangles(&positions, &indices).expect("the sheet is a mesh")
}

fn adaptive(divisions: usize) -> DynamicSurface {
    DynamicSurface::from_mesh(&sheet(divisions, 2.0), DynamicDesc::default())
        .expect("a welded triangle sheet is an adaptive surface")
}

/// A Draw stamp in the middle of the sheet, big enough to reach several rows.
fn draw() -> MeshStamp<'static> {
    MeshStamp {
        verb: MeshBrush::Draw,
        center: [0.0, 0.0, 0.0],
        radius: 1.0,
        strength: 0.5,
        ..MeshStamp::default()
    }
}

/// Topology on, with the target edge short enough that the stamp has to split.
fn splitting_topology() -> DynamicTopology {
    DynamicTopology {
        enabled: true,
        detail_mode: DetailMode::World,
        target_edge_length: 0.1,
        ..DynamicTopology::default()
    }
}

// -- the lifecycle group ----------------------------------------------------

#[test]
fn a_sheet_becomes_a_half_edge_surface_that_counts_its_own_parts() {
    let surface = adaptive(8);
    let stats = surface.stats().expect("stats");

    assert_eq!(stats.faces, 8 * 8 * 2, "every quad is two triangles");
    assert_eq!(
        stats.vertices,
        9 * 9,
        "the sheet's corners weld to one vertex each"
    );
    assert!(
        stats.boundary_edges > 0,
        "an open sheet has a boundary, and a structure reporting none has \
         welded its rim to something"
    );
    assert_eq!(
        stats.dead_slots, 0,
        "a surface nothing has sculpted has no holes in its slot pools"
    );
    assert!(stats.bytes > 0, "the structure occupies memory");
    assert_eq!(
        stats.halfedges,
        stats.edges * 2,
        "a half-edge structure has two half-edges per edge, boundary included"
    );
}

#[test]
fn the_structure_validates_and_says_so_in_words() {
    let validation = adaptive(4).validate().expect("validate");
    assert!(
        validation.ok,
        "a sheet straight out of the converter is sound: {}",
        validation.message
    );
    assert!(
        !validation.message.is_empty(),
        "the summary is carried on the sound path too, so it is a sentence \
         somebody has read before the day it matters"
    );
}

#[test]
fn a_surface_round_trips_through_its_own_bytes() {
    let original = adaptive(6);
    let before = original.stats().expect("stats");
    let revision = original.revision().expect("revision");

    let bytes = original.serialize().expect("serialize");
    assert!(
        !bytes.is_empty(),
        "a surface with faces encodes to something"
    );

    let restored = DynamicSurface::deserialize(&bytes).expect("deserialize");
    let after = restored.stats().expect("stats");
    // Every structural figure, and deliberately not `bytes`: the arrays that
    // carry the structure are grown rather than reserved, so a surface read
    // back in one pass holds less capacity slack than one built by a
    // converter. The structure is the same; the allocator's headroom is not,
    // and asserting on it would fail on a change to nothing that matters.
    assert_eq!(
        (
            after.vertices,
            after.edges,
            after.halfedges,
            after.faces,
            after.boundary_edges,
            after.dead_slots
        ),
        (
            before.vertices,
            before.edges,
            before.halfedges,
            before.faces,
            before.boundary_edges,
            before.dead_slots
        ),
        "the reconstructed surface is the same structure, part for part"
    );
    assert_eq!(
        restored.revision().expect("revision"),
        revision,
        "generations are preserved, so a chunk index taken before a save \
         still resolves after a load"
    );
}

#[test]
fn a_truncated_blob_is_refused_rather_than_read() {
    let bytes = adaptive(4).serialize().expect("serialize");
    let half = &bytes[..bytes.len() / 2];
    assert!(
        DynamicSurface::deserialize(half).is_err(),
        "half a surface is not a surface, and reading one would be reading \
         past the buffer"
    );
}

#[test]
fn the_round_trip_back_to_a_mesh_is_triangles() {
    let surface = adaptive(4);
    let mesh = surface.to_mesh().expect("to_mesh");
    let faces = surface.stats().expect("stats").faces;

    assert_eq!(
        mesh.indices().len() as u64,
        faces * 3,
        "a dynamic surface is triangles, and the export is three indices a \
         face with no quad pairing re-derived"
    );
    assert!(
        mesh.vertex_count() >= 25,
        "the export splits a geometric vertex per distinct corner attribute, \
         so it is bounded below by the welded count"
    );
}

#[test]
fn a_degenerate_triangle_is_refused_by_name() {
    // Two corners at one point, so they weld to one class: a face with no area
    // and no meaningful normal. Refused at the boundary rather than left for
    // every operator downstream to guard against.
    //
    // `DynamicError::EmptyMesh` has no test of its own, and that is a fact
    // about this crate rather than an omission: `Mesh::from_triangles` refuses
    // to build a mesh with no triangles, so a handle with no faces cannot be
    // constructed here to hand to the converter.
    let positions = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
    let mesh = claycore::Mesh::from_triangles(&positions, &[0, 1, 2]).expect("a folded sliver");

    let refusal = DynamicSurface::from_mesh(&mesh, DynamicDesc::default())
        .expect_err("a triangle with no area is not a face");
    assert_eq!(
        refusal.reason,
        DynamicError::DegenerateTriangle,
        "the refusal names the model problem and not only the result code — \
         'a triangle has repeated or collinear corners' is a sentence a user \
         can act on and 'invalid argument' is not"
    );
    assert!(
        refusal.to_string().contains("repeated"),
        "and the display carries that sentence: {refusal}"
    );
}

#[test]
fn three_faces_on_one_edge_are_refused_rather_than_silently_dropped() {
    // A fan of three triangles sharing the edge 0-1. A half-edge surface
    // cannot express it, and dropping the third would change the model.
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.5, 1.0, 0.0],
        [0.5, -1.0, 0.0],
        [0.5, 0.0, 1.0],
    ];
    let indices = [0, 1, 2, 0, 1, 3, 0, 1, 4];
    let mesh = claycore::Mesh::from_triangles(&positions, &indices).expect("the fan is a mesh");

    let refusal = DynamicSurface::from_mesh(&mesh, DynamicDesc::default())
        .expect_err("three faces on one edge is not a half-edge surface");
    assert_eq!(refusal.reason, DynamicError::NonManifoldEdge);
}

#[test]
fn the_descriptor_takes_the_engines_own_defaults_for_what_it_leaves_unset() {
    // Both tolerances left unset must give exactly what the engine's own
    // defaults give, which is the whole claim of `*_defaults`-then-override.
    let unset = adaptive(4).stats().expect("stats");
    let explicit = DynamicSurface::from_mesh(
        &sheet(4, 2.0),
        DynamicDesc {
            weld_epsilon: Some(0.0),
            ..Default::default()
        },
    )
    .expect("exact welding is a policy, not a refusal")
    .stats()
    .expect("stats");

    assert_eq!(
        unset.vertices, explicit.vertices,
        "a sheet with no coincident corners welds the same either way; if \
         this ever differs, the default is no longer exact and the two \
         descriptors mean different things"
    );
}

// -- the sculpting group ----------------------------------------------------

#[test]
fn a_stamp_moves_vertices_and_advances_the_geometry_revision() {
    let mut surface = adaptive(8);
    let before = surface.revision().expect("revision");
    let mut sculptor = surface.sculptor().expect("sculptor");

    let report = sculptor
        .stamp(draw(), None, None)
        .expect("a Draw stamp in the middle of the sheet");

    assert!(
        report.moved_vertices > 0,
        "a Draw at the centre with a radius of 1 on a 4-unit sheet reaches \
         vertices; zero here is the brush not landing at all"
    );
    assert!(
        report.revision.geometry > before.geometry,
        "vertices moved, so the counter a redraw watches has to move"
    );
    assert!(
        report.dirty_max[1] > report.dirty_min[1],
        "the reported box has height: the stamp pushed the sheet off its own \
         plane"
    );
}

#[test]
fn a_stamp_changes_connectivity_and_says_so() {
    let mut surface = adaptive(8);
    let faces_before = surface.stats().expect("stats").faces;
    let topology = splitting_topology();
    let mut sculptor = surface.sculptor().expect("sculptor");

    let report = sculptor
        .stamp(draw(), Some(&topology), None)
        .expect("stamp with adaptive topology on");

    assert!(
        report.split_edges > 0,
        "a target edge of 0.1 under a brush of radius 1 on a sheet whose \
         edges are 0.5 long has to split; this is the whole representation"
    );
    assert!(
        report.revision.topology > 0,
        "connectivity changed, so the counter an index buffer watches moved"
    );
    assert!(
        sculptor.surface().stats().expect("stats").faces > faces_before,
        "splits create faces, and the census is where that is visible rather \
         than only in the report that claimed it"
    );
}

#[test]
fn topology_turned_off_is_a_pure_deformation() {
    let mut surface = adaptive(8);
    let before = surface.stats().expect("stats");
    let topology = DynamicTopology {
        enabled: false,
        ..DynamicTopology::default()
    };
    let mut sculptor = surface.sculptor().expect("sculptor");

    let report = sculptor
        .stamp(draw(), Some(&topology), None)
        .expect("stamp with topology off");

    assert!(report.moved_vertices > 0, "the brush still deforms");
    assert_eq!(
        (
            report.split_edges,
            report.collapsed_edges,
            report.flipped_edges
        ),
        (0, 0, 0),
        "with topology off the connectivity that went in comes out — this is \
         what lets a host offer the representation without offering remeshing"
    );
    assert_eq!(
        sculptor.surface().stats().expect("stats").faces,
        before.faces,
        "and the face count is the proof, not the report's own zeroes"
    );
}

#[test]
fn every_verb_the_adaptive_surface_declines_is_the_one_it_says() {
    // `MeshBrush::offered_by_adaptive` is a second copy of the engine's own
    // `dynamic_offers`, because the ABI has no entry point to ask. This is
    // what keeps the two from drifting: every verb is stamped, and the
    // refusals have to line up with the prediction exactly.
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");

    for verb in MeshBrush::ALL {
        let stamp = MeshStamp { verb, ..draw() };
        let accepted = sculptor.stamp(stamp, None, None).is_ok();
        assert_eq!(
            accepted,
            verb.offered_by_adaptive(),
            "{verb:?}: the wrapper and the engine disagree about whether an \
             adaptive surface offers this verb"
        );
    }
}

#[test]
fn a_declared_world_frame_is_read_back() {
    let mut surface = adaptive(4);
    let mut sculptor = surface.sculptor().expect("sculptor");

    assert_eq!(
        sculptor.world_frame().expect("world_frame"),
        None,
        "unset is the identity and reports itself undeclared, so a host that \
         has never heard of this is not opted in"
    );

    let frame = WorldFrame {
        position: [1.0, 2.0, 3.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: 2.0,
    };
    sculptor.set_world_frame(Some(frame)).expect("set");
    let read = sculptor
        .world_frame()
        .expect("world_frame")
        .expect("the session declares one now");
    assert_eq!(read.position, frame.position);
    assert_eq!(read.scale, frame.scale);

    sculptor.set_world_frame(None).expect("clear");
    assert_eq!(
        sculptor.world_frame().expect("world_frame"),
        None,
        "clearing returns the session to the identity"
    );
}

#[test]
fn a_frame_with_no_scale_is_refused_rather_than_approximated() {
    let mut surface = adaptive(4);
    let mut sculptor = surface.sculptor().expect("sculptor");
    let refused = sculptor.set_world_frame(Some(WorldFrame {
        scale: -1.0,
        ..WorldFrame::default()
    }));
    assert!(
        refused.is_err(),
        "a non-positive scale makes `radius` name nothing a spherical walk \
         can honour, so it is refused at the boundary"
    );
}

#[test]
fn automask_sources_are_set_and_cleared() {
    // Setting sources enables nothing on its own — the brush's own factor bits
    // decide that — so what this checks is that the handle is accepted and
    // that the stamps either side of it are the stamps they were.
    let cavity = Mask::new(0.1).expect("a mask to measure against");
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");

    sculptor
        .set_automask_sources(Some(&cavity))
        .expect("a cavity source is accepted");
    let with = sculptor.stamp(draw(), None, None).expect("stamp");
    assert!(
        with.moved_vertices > 0,
        "a source with no bit costs nothing"
    );

    sculptor
        .set_automask_sources(None)
        .expect("null clears both factors");
    let without = sculptor.stamp(draw(), None, None).expect("stamp");
    assert!(without.moved_vertices > 0);
}

// -- the chunk group --------------------------------------------------------

#[test]
fn a_stamp_marks_chunks_dirty_and_clear_dirty_clears_them() {
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");

    assert!(
        sculptor.chunk_count() > 0,
        "the sculptor owns the partition, and one over a sheet with faces is \
         not empty"
    );
    assert!(
        sculptor.dirty_chunks().expect("dirty_chunks").is_empty(),
        "nothing has been sculpted yet, so nothing is waiting to be uploaded"
    );

    sculptor.stamp(draw(), None, None).expect("stamp");
    let dirty = sculptor.dirty_chunks().expect("dirty_chunks");
    assert!(
        !dirty.is_empty(),
        "a stamp that moved vertices has to name the chunks it touched, or a \
         host redrawing from the dirty set draws the old surface"
    );
    assert!(
        dirty.len() <= sculptor.chunk_count(),
        "a dab cannot dirty more chunks than there are"
    );

    sculptor.clear_dirty().expect("clear_dirty");
    assert!(
        sculptor.dirty_chunks().expect("dirty_chunks").is_empty(),
        "clearing is all or nothing, and this is the 'nothing left' half"
    );
}

#[test]
fn a_chunk_copies_as_a_standalone_draw() {
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");
    sculptor.stamp(draw(), None, None).expect("stamp");

    let index = *sculptor
        .dirty_chunks()
        .expect("dirty_chunks")
        .first()
        .expect("a stamp dirtied at least one chunk") as usize;

    let info = sculptor.chunk_info(index).expect("chunk_info");
    let chunk = sculptor.copy_chunk(index).expect("copy_chunk");

    assert_eq!(chunk.positions.len(), info.vertex_count as usize);
    assert_eq!(chunk.normals.len(), chunk.positions.len());
    assert_eq!(chunk.indices.len(), info.index_count as usize);
    assert_eq!(
        chunk.indices.len() % 3,
        0,
        "the copy is triangles, so a host uploads it without regrouping"
    );
    assert!(
        chunk
            .indices
            .iter()
            .all(|&i| (i as usize) < chunk.positions.len()),
        "the indices are LOCAL to the chunk; one naming a vertex outside it \
         would draw geometry from wherever that index landed in the pool"
    );
    assert!(
        chunk.normals.iter().all(|n| {
            let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            (length - 1.0).abs() < 1e-3
        }),
        "normals come out unit, which is what a shader assumes without \
         renormalising"
    );
}

#[test]
fn copy_chunk_writes_nothing_past_the_buffers_it_is_given() {
    // The pooled-buffer case, which is the one a renderer actually runs: size
    // once to the largest chunk, copy into it every frame. A copy that ran
    // past the chunk's own extent would corrupt whatever shares the pool, and
    // the symptom is geometry from somewhere else on the model.
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");
    sculptor.stamp(draw(), None, None).expect("stamp");

    let info = sculptor.chunk_info(0).expect("chunk_info");
    let slack = 32;
    const GUARD_F: [f32; 3] = [f32::MAX, f32::MAX, f32::MAX];
    const GUARD_I: u32 = u32::MAX;

    let mut positions = vec![GUARD_F; info.vertex_count as usize + slack];
    let mut normals = vec![GUARD_F; info.vertex_count as usize + slack];
    let mut indices = vec![GUARD_I; info.index_count as usize + slack];

    let written = sculptor
        .copy_chunk_into(0, &mut positions, &mut normals, &mut indices)
        .expect("an oversized buffer is a pool, not an error");

    assert_eq!(written.vertex_count, info.vertex_count);
    assert_eq!(written.index_count, info.index_count);
    assert!(
        positions[written.vertex_count as usize..]
            .iter()
            .all(|p| *p == GUARD_F),
        "the tail of the position buffer was written into"
    );
    assert!(
        normals[written.vertex_count as usize..]
            .iter()
            .all(|n| *n == GUARD_F),
        "the tail of the normal buffer was written into"
    );
    assert!(
        indices[written.index_count as usize..]
            .iter()
            .all(|i| *i == GUARD_I),
        "the tail of the index buffer was written into"
    );
}

#[test]
fn a_chunk_too_small_to_hold_is_refused_rather_than_truncated() {
    let mut surface = adaptive(8);
    let sculptor = surface.sculptor().expect("sculptor");
    let info = sculptor.chunk_info(0).expect("chunk_info");
    assert!(info.vertex_count > 1, "the fixture's chunks hold something");

    let mut positions = vec![[0.0f32; 3]; 1];
    let mut normals = vec![[0.0f32; 3]; 1];
    let mut indices = vec![0u32; info.index_count as usize];
    assert!(
        sculptor
            .copy_chunk_into(0, &mut positions, &mut normals, &mut indices)
            .is_err(),
        "a short buffer is refused; a partial chunk that reported success \
         would be a hole in the drawn surface with nothing saying so"
    );
}

#[test]
fn a_surface_view_over_an_adaptive_sculptor_says_which_representation_it_is() {
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");
    sculptor.stamp(draw(), None, None).expect("stamp");

    let mut view = SurfaceView::over_dynamic(&mut sculptor).expect("a view over the sculptor");
    assert_eq!(
        view.kind(),
        SurfaceKind::Adaptive,
        "a host branching on the kind has to be told this one copies unwelded"
    );
    assert!(
        !view.dirty_chunks().expect("dirty_chunks").is_empty(),
        "the view drains the same dirty set the sculptor filled"
    );
}

// -- the memory and telemetry group -----------------------------------------

#[test]
fn a_session_answers_for_its_own_memory() {
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");
    sculptor.stamp(draw(), None, None).expect("stamp");

    let ledger = sculptor.memory_ledger().expect("memory_ledger");
    assert!(
        ledger.total > 0,
        "a sculptor holding a spatial index over a sheet costs something, and \
         a ledger reporting nothing is a ledger nobody filled"
    );

    let report = sculptor
        .trim(Pressure::Urgent, None)
        .expect("trim at urgent pressure");
    assert!(
        !report.pinned,
        "nothing was pinned, so the trim did what it reported rather than \
         only describing it"
    );
    assert!(
        sculptor.surface().validate().expect("validate").ok,
        "a trim releases rebuildable caches and never authoritative content, \
         so the structure is still sound afterwards"
    );
}

#[test]
fn a_held_pin_turns_a_trim_into_a_report() {
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");
    sculptor.stamp(draw(), None, None).expect("stamp");

    let mut pin = claycore::MemoryPin::new().expect("a pin");
    let hold = pin.hold().expect("hold it");
    let report = sculptor
        .trim(Pressure::Critical, Some(&hold))
        .expect("trim under a held pin");
    assert!(
        report.pinned,
        "a held pin makes the call a no-op that reports what it WOULD have \
         released — a host that cannot tell the two apart frees nothing and \
         believes it did"
    );
    drop(hold);
}

#[test]
fn the_arena_and_the_peaks_are_measured_from_the_first_stamp() {
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");

    let cold = sculptor.peak_telemetry().expect("peak_telemetry");
    assert_eq!(
        cold.workset_vertices, 0,
        "nothing has been gathered yet, and a peak reading non-zero before \
         the first stamp is a peak measuring something other than stamps"
    );

    sculptor
        .stamp(draw(), Some(&splitting_topology()), None)
        .expect("stamp");

    let warm = sculptor.peak_telemetry().expect("peak_telemetry");
    assert!(
        warm.workset_vertices > 0,
        "the stamp gathered a workset and the high-water mark has to hold it"
    );
    assert!(
        warm.topology_ops > 0,
        "the adaptive surface is the only representation that fills this one, \
         and a splitting stamp is exactly when it should"
    );

    let arena = sculptor.arena_stats().expect("arena_stats");
    assert!(
        arena.high_water_bytes > 0,
        "a stamp used scratch, so the arena has seen a footprint"
    );
    assert!(arena.capacity_bytes >= arena.high_water_bytes);

    sculptor.reset_peak_telemetry().expect("reset");
    assert_eq!(
        sculptor.peak_telemetry().expect("peak_telemetry"),
        cold,
        "a reset puts the session back to unmeasured, which is what makes the \
         next stamp comparable on its own"
    );
}

#[test]
fn the_index_reports_what_it_is_worth_and_a_rebuild_leaves_the_surface_sound() {
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");
    sculptor
        .stamp(draw(), Some(&splitting_topology()), None)
        .expect("stamp");

    let quality = sculptor.index_quality().expect("index_quality");
    assert!(
        quality.leaf_count > 0,
        "an index over a sheet with faces has leaves"
    );

    sculptor.rebuild_index().expect("rebuild_index");
    assert!(
        sculptor.surface().validate().expect("validate").ok,
        "a rebuild replaces a partition and touches no geometry"
    );
    assert!(
        sculptor.stamp(draw(), None, None).is_ok(),
        "and the brush still finds the surface through the new tree"
    );
}

#[test]
fn a_rebuild_is_requested_only_when_the_tree_asks_and_the_profile_allows() {
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");
    sculptor
        .stamp(draw(), Some(&splitting_topology()), None)
        .expect("stamp");

    let wants = sculptor
        .index_quality()
        .expect("index_quality")
        .wants_rebuild;
    let mut queue = MaintenanceQueue::new().expect("queue");

    let forbidden = SculptMemoryProfile {
        allow_index_rebuild: false,
        ..SculptMemoryProfile::defaults().expect("the engine's own defaults")
    };
    assert!(
        !sculptor
            .request_index_rebuild(Some(forbidden), 0, &mut queue)
            .expect("request"),
        "the engine measures and the HOST decides whether it has the room; a \
         profile that forbids a rebuild queues nothing however the tree feels"
    );
    assert!(queue.is_empty().expect("len"));

    let queued = sculptor
        .request_index_rebuild(None, 0, &mut queue)
        .expect("request under the default profile");
    assert_eq!(
        queued, wants,
        "with the profile allowing it, what is queued is exactly the tree's \
         own opinion"
    );
    assert_eq!(
        queue.has(MaintenanceKind::IndexRebuild, 0).expect("has"),
        wants,
        "and the item that lands is the one that was asked for"
    );
}

#[test]
fn the_stage_report_is_zero_until_it_is_enabled() {
    let mut surface = adaptive(8);
    let mut sculptor = surface.sculptor().expect("sculptor");

    sculptor
        .stamp(draw(), None, None)
        .expect("an unmeasured stamp");
    let off = sculptor.stage_report().expect("stage_report");
    assert_eq!(
        off.total_nanos(),
        0,
        "reading a report nothing enabled answers zeroes, which is the honest \
         figure rather than a refusal"
    );
    assert!(
        !off.stages.is_empty(),
        "the stage table is still named, so a host can lay out its columns \
         before it has measured anything"
    );

    sculptor.set_stage_report_enabled(true).expect("enable");
    sculptor
        .stamp(draw(), Some(&splitting_topology()), None)
        .expect("a measured stamp");

    let on = sculptor.stage_report().expect("stage_report");
    assert!(
        on.vertices_considered >= on.vertices_affected,
        "a workset holds everything the brush reached, including the rim \
         where the weight is zero"
    );
    assert!(
        on.splits > 0,
        "the splitting stamp was measured, and TOPOLOGY is the stage only \
         this representation fills"
    );
    assert!(
        on.stages
            .iter()
            .any(|(stage, timing)| *stage == SculptStage::Topology && timing.calls > 0),
        "and the stage table names it rather than only the roll-up"
    );

    sculptor.reset_stage_report().expect("reset");
    assert_eq!(
        sculptor.stage_report().expect("stage_report").splits,
        0,
        "a reset zeroes it so the next stamp is measured on its own"
    );
}

// -- the budget questions ---------------------------------------------------

#[test]
fn preflight_to_mesh_answers_before_a_conversion_is_attempted() {
    let surface = adaptive(8);
    let unbounded = surface.preflight_to_mesh(0).expect("no budget");

    assert!(
        unbounded.allowed,
        "a budget of zero means no budget, which is what a desktop host passes"
    );
    assert!(
        unbounded.peak_bytes >= unbounded.persistent_bytes,
        "the peak holds the source and the result at once, so it cannot be \
         below what remains"
    );

    let refused = surface
        .preflight_to_mesh(1)
        .expect("a budget question is answered, not refused");
    assert!(
        !refused.allowed,
        "one byte does not hold a mesh, and the point of asking is to hear so \
         before the allocation rather than after it"
    );
    assert_eq!(refused.error, claycore::BudgetError::OverBudget);

    // The answer has to be about this surface rather than a constant: a
    // preflight that does not move with the model is a preflight that is not
    // measuring one.
    let bigger = adaptive(16).preflight_to_mesh(0).expect("no budget");
    assert!(
        bigger.peak_bytes > unbounded.peak_bytes,
        "four times the faces costs more to export"
    );
}

#[test]
fn preflight_encode_prices_the_blob_that_lives_beside_the_surface() {
    let surface = adaptive(8);
    let predicted = surface.preflight_encode(0).expect("no budget");
    assert!(predicted.allowed);

    let actual = surface.serialize().expect("serialize").len() as u64;
    assert!(
        predicted.peak_bytes >= actual,
        "the estimate is a CEILING on purpose: a budget that errs low says \
         yes to an operation that does not fit. Predicted {} for {actual}",
        predicted.peak_bytes
    );
}

#[test]
fn a_mesh_prices_its_own_conversion_into_a_surface() {
    // The other half of the pair, which lives on `Mesh` because that is what
    // is in hand when the question is asked.
    let mesh = sheet(8, 2.0);
    let predicted = mesh.preflight_to_dynamic(0).expect("no budget");
    assert!(predicted.allowed);

    let actual = DynamicSurface::from_mesh(&mesh, DynamicDesc::default())
        .expect("the conversion the preflight priced")
        .stats()
        .expect("stats")
        .bytes;
    assert!(
        predicted.persistent_bytes >= actual,
        "what remains after the call is a ceiling on what the structure \
         reports afterwards. Predicted {} for {actual}",
        predicted.persistent_bytes
    );
}
