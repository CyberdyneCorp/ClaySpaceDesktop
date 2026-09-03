//! The multiresolution surface, at the engine boundary.
//!
//! The tier's whole claim is one sentence and it is the one this file is
//! written around: sculpt detail at a fine level, change the form underneath
//! it at a coarse one, and the detail is still there and still oriented.
//! `detail_survives_a_change_to_the_form_beneath_it` is that sentence measured
//! rather than read — it builds two identical hierarchies, sculpts only one of
//! them, moves the cage of both by the same amount, and then asks what is left
//! between them.
//!
//! Everything else here follows this crate's rule that a wrapper nobody runs
//! is a SAFETY comment nobody has checked: every entry point `multires.rs`
//! wraps is called at least once, and the assertions are about consequences —
//! how far a vertex moved, which patches went dirty, what a refusal *named* —
//! rather than about `Ok(())`, because a call that reports success without
//! reaching the surface is the failure this file exists to catch.

use claycore::{
    BrushShape, Falloff, Mask, MemoryCategory, MemoryClass, MemoryPin, Mesh, MeshBrush, MeshStamp,
    Multires, MultiresDesc, MultiresError, Pressure, SculptMemoryProfile, StrokePreset,
    StrokeSample,
};

// -- fixtures ---------------------------------------------------------------

/// A flat grid of quads, which is what a Catmull-Clark cage is supposed to be.
///
/// It goes through a file because the C ABI builds a mesh from an importer or
/// from the mesher and offers no way to hand it arrays. Note what the header
/// says happens on the way back in: the readers are unchanged, so a quad file
/// comes back as *triangles*. That is fine for a cage — the subdivision rule
/// is defined over faces of any arity — and it is why the level-0 face count
/// below is the triangle count and not the quad count.
fn cage(divisions: usize, half: f32, name: &str) -> Mesh {
    let mut text = String::new();
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
            // Wound so the sheet faces +y, which is what makes a Draw stamp
            // read as a bump rather than as a dent.
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
    let path = std::env::temp_dir().join(format!(
        "claycore-multires-{name}-{}.obj",
        std::process::id()
    ));
    std::fs::write(&path, text).expect("write the cage");
    let mesh = Mesh::load(&path).expect("load the cage");
    let _ = std::fs::remove_file(&path);
    mesh
}

fn hierarchy_within(levels: u32, budget: u64, name: &str) -> Multires {
    let mesh = cage(4, 2.0, name);
    let mut surface = Multires::from_mesh(
        &mesh,
        MultiresDesc {
            memory_budget: budget,
            ..Default::default()
        },
    )
    .expect("the cage is a hierarchy of one level");
    for _ in 0..levels {
        surface.add_level().expect("subdivide");
    }
    surface
}

fn hierarchy(levels: u32, name: &str) -> Multires {
    hierarchy_within(levels, 0, name)
}

fn positions(surface: &mut Multires, level: u32) -> Vec<[f32; 3]> {
    surface
        .copy_level_mesh(level)
        .expect("a level is a mesh")
        .positions()
        .to_vec()
}

/// One Draw dab at a level, through a sculptor that is dropped afterwards.
fn dab(surface: &mut Multires, level: u32, center: [f32; 3], radius: f32, strength: f32) {
    let mut sculptor = surface.sculptor().expect("sculptor");
    sculptor
        .surface_mut()
        .set_sculpt_level(level)
        .expect("bind the level");
    sculptor.begin_stroke().expect("begin");
    sculptor
        .stamp(
            MeshStamp {
                verb: MeshBrush::Draw,
                center,
                radius,
                strength,
                // A straight-line falloff: a surface walk on a flat sheet
                // answers the same thing and costs more to reason about.
                geodesic: false,
                ..Default::default()
            },
            None,
        )
        .expect("stamp");
}

fn norm(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn difference(a: &[[f32; 3]], b: &[[f32; 3]]) -> Vec<[f32; 3]> {
    assert_eq!(a.len(), b.len(), "two levels of the same shape");
    a.iter()
        .zip(b.iter())
        .map(|(a, b)| [a[0] - b[0], a[1] - b[1], a[2] - b[2]])
        .collect()
}

/// The vertex that moved furthest, and how far.
fn furthest(deltas: &[[f32; 3]]) -> (usize, f32) {
    deltas
        .iter()
        .enumerate()
        .map(|(i, d)| (i, norm(*d)))
        .fold(
            (0, 0.0),
            |best, next| if next.1 > best.1 { next } else { best },
        )
}

fn tallest(surface: &mut Multires, level: u32) -> f32 {
    positions(surface, level)
        .iter()
        .map(|p| p[1].abs())
        .fold(0.0f32, f32::max)
}

// -- building a hierarchy ---------------------------------------------------

#[test]
fn a_new_hierarchy_is_its_cage_and_nothing_else() {
    let mesh = cage(4, 2.0, "fresh");
    let surface = Multires::from_mesh(&mesh, MultiresDesc::default()).expect("hierarchy");

    assert_eq!(
        surface.level_count(),
        1,
        "from_mesh builds one level — the cage — and adding levels is a \
         separate and priced operation"
    );
    assert_eq!(surface.sculpt_level().expect("sculpt level"), 0);
    assert_eq!(surface.display_level().expect("display level"), 0);

    let (vertices, faces) = surface.level_counts(0).expect("the cage's counts");
    assert_eq!(vertices, mesh.vertex_count() as u64, "the cage is the mesh");
    assert_eq!(
        faces,
        mesh.index_count() as u64 / 3,
        "and its faces are the triangles the importer handed back"
    );
}

#[test]
fn a_cage_that_is_not_manifold_is_refused_rather_than_repaired() {
    // One edge shared by three faces: the subdivision rules have no meaning
    // there, and a conversion that quietly welded it would change the
    // retopology somebody paid for without saying so.
    let path = std::env::temp_dir().join(format!(
        "claycore-multires-nonmanifold-{}.obj",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 0 -1 0\nv 0 0 1\nf 1 2 3\nf 1 2 4\nf 1 2 5\n",
    )
    .expect("write");
    let mesh = Mesh::load(&path).expect("load");
    let _ = std::fs::remove_file(&path);

    let refusal = Multires::from_mesh(&mesh, MultiresDesc::default())
        .expect_err("three faces on one edge is not a cage");
    assert_eq!(
        refusal.reason,
        MultiresError::NonManifold,
        "the refusal names the model problem and not just 'invalid argument': \
         {refusal}"
    );
    assert!(
        refusal.reason.text().contains("edge"),
        "and the engine's own sentence says which edge problem: {}",
        refusal.reason.text()
    );
}

#[test]
fn a_cage_with_no_faces_is_refused() {
    let path = std::env::temp_dir().join(format!(
        "claycore-multires-facesless-{}.obj",
        std::process::id()
    ));
    std::fs::write(&path, "v 0 0 0\nv 1 0 0\nv 0 1 0\n").expect("write");
    let mesh = Mesh::load(&path).expect("load");
    let _ = std::fs::remove_file(&path);
    assert_eq!(mesh.index_count(), 0, "the fixture is what it claims to be");

    let refusal =
        Multires::from_mesh(&mesh, MultiresDesc::default()).expect_err("no faces is no cage");
    assert_eq!(refusal.reason, MultiresError::EmptyBase);
}

// -- levels -----------------------------------------------------------------

#[test]
fn each_level_multiplies_the_one_below_by_four() {
    let surface = hierarchy(3, "quadrupling");
    assert_eq!(surface.level_count(), 4);

    let faces = |level| surface.level_counts(level).expect("counts").1;
    assert_eq!(
        faces(2),
        4 * faces(1),
        "Catmull-Clark splits every face into four"
    );
    assert_eq!(faces(3), 4 * faces(2));
}

#[test]
fn the_preflight_prices_a_level_without_building_it() {
    let mut surface = hierarchy(1, "preflight");
    let before = surface.level_count();

    let quote = surface.preflight_add_level().expect("preflight");
    assert!(quote.allowed, "no budget was declared, so nothing refuses");
    assert_eq!(
        surface.level_count(),
        before,
        "the preflight is arithmetic on the level below: it allocates nothing \
         and has no side effects"
    );
    assert_eq!(quote.level, before, "it prices the level that would appear");
    assert!(
        quote.peak_bytes >= quote.persistent_bytes,
        "the high-water mark during the call is never below what remains \
         after it — the peak is the figure that kills an application"
    );

    let level = surface.add_level().expect("subdivide");
    assert_eq!(level, quote.level);
    let (vertices, faces) = surface.level_counts(level).expect("counts");
    assert_eq!(
        (vertices, faces),
        (quote.vertices, quote.faces),
        "the quote described the level that was actually built"
    );
}

#[test]
fn subdividing_takes_both_levels_to_the_new_one() {
    let mut surface = hierarchy(1, "subdivide");
    surface.set_display_level(0).expect("look at the cage");

    let level = surface.add_level().expect("subdivide");
    assert_eq!(
        (
            surface.sculpt_level().expect("sculpt"),
            surface.display_level().expect("display")
        ),
        (level, level),
        "which is what an artist means by 'subdivide'"
    );
}

#[test]
fn where_the_brush_writes_and_what_is_drawn_are_independent() {
    let mut surface = hierarchy(3, "independent");

    surface.set_sculpt_level(1).expect("sculpt at 1");
    surface.set_display_level(3).expect("draw at 3");
    assert_eq!(surface.sculpt_level().expect("sculpt"), 1);
    assert_eq!(
        surface.display_level().expect("display"),
        3,
        "moving the broad form while watching the pores is the workflow the \
         tier exists for, so one level must not drag the other"
    );

    surface.set_sculpt_level(2).expect("sculpt at 2");
    assert_eq!(surface.display_level().expect("display"), 3);
}

#[test]
fn a_level_that_does_not_exist_is_refused() {
    let mut surface = hierarchy(1, "outofrange");
    assert!(surface.set_sculpt_level(9).is_err());
    assert!(surface.set_display_level(9).is_err());
    assert!(surface.level_counts(9).is_err());
    assert_eq!(
        surface.sculpt_level().expect("sculpt"),
        1,
        "a refused level leaves the one that was bound alone"
    );
}

#[test]
fn the_cage_alone_has_nothing_above_it_to_remove() {
    let mut surface = hierarchy(1, "remove");
    surface.remove_highest_level().expect("drop level 1");
    assert_eq!(surface.level_count(), 1);

    let refusal = surface
        .remove_highest_level()
        .expect_err("the cage is not removable");
    assert_eq!(refusal.reason, MultiresError::NoLevelToRemove);
    assert_eq!(surface.level_count(), 1, "and the refusal changed nothing");
}

#[test]
fn a_declared_budget_refuses_a_level_before_it_is_built() {
    // Two kilobytes is far under what the first level of even this cage costs.
    let mut surface = hierarchy_within(0, 2048, "budget");

    let quote = surface.preflight_add_level().expect("preflight");
    assert!(!quote.allowed);
    assert_eq!(quote.error, MultiresError::OverBudget);

    let refusal = surface.add_level().expect_err("over budget");
    assert_eq!(refusal.reason, MultiresError::OverBudget);
    assert_eq!(
        surface.level_count(),
        1,
        "build-then-publish: a refusal leaves the surface exactly as it was \
         rather than one level into a state nothing knows how to read"
    );
}

// -- what the tier is for ---------------------------------------------------

#[test]
fn detail_survives_a_change_to_the_form_beneath_it() {
    // P(n) = S(n) + Frame(n) * D(n). What is stored at level 3 is a
    // displacement in a frame carried up from the level below, so moving the
    // level below moves the frame and the wrinkle rides on it. Measured here
    // against a twin that got the cage edit and never got the wrinkle: what
    // is left between the two IS Frame(n) * D(n).
    let mut sculpted = hierarchy(3, "detail-a");
    let mut plain = hierarchy(3, "detail-b");

    let pristine = positions(&mut sculpted, 3);
    assert_eq!(
        pristine,
        positions(&mut plain, 3),
        "the two hierarchies start identical, or nothing below means anything"
    );

    // A wrinkle at level 3, deliberately off-centre so the cage edit below it
    // tilts the surface under it rather than lifting it straight up.
    dab(&mut sculpted, 3, [0.8, 0.0, 0.0], 0.6, 0.6);

    let detail_before = difference(&positions(&mut sculpted, 3), &pristine);
    let (peak, height) = furthest(&detail_before);
    assert!(
        height > 0.1,
        "the fixture has to actually deposit something: it moved {height}"
    );
    let checksum = sculpted.detail_checksum().expect("checksum");
    let base_before = sculpted.revision().expect("revision").base;

    // Now move the skull under the wrinkle — on both, identically.
    dab(&mut sculpted, 0, [-1.0, 0.0, 0.0], 3.0, 1.2);
    dab(&mut plain, 0, [-1.0, 0.0, 0.0], 3.0, 1.2);

    assert_eq!(
        sculpted.detail_checksum().expect("checksum"),
        checksum,
        "editing the cage rewrites the cage, not the detail above it"
    );
    assert!(
        sculpted.revision().expect("revision").base > base_before,
        "while the hierarchy's shape did move, which is the counter a host \
         re-uploads an index buffer on"
    );

    let after = positions(&mut sculpted, 3);
    let twin = positions(&mut plain, 3);
    assert_ne!(
        after, twin,
        "the sculpted hierarchy is still not the plain one"
    );
    assert_ne!(
        twin, pristine,
        "and the cage edit genuinely reached level 3 on both"
    );

    let detail_after = difference(&after, &twin);
    let (peak_after, height_after) = furthest(&detail_after);

    assert_eq!(
        peak_after, peak,
        "the wrinkle is still on the same vertex it was cut into"
    );
    assert!(
        (height_after - height).abs() < 1e-3 * height,
        "and it is still the same depth: {height} before the cage moved, \
         {height_after} after"
    );

    let turned = dot(detail_before[peak], detail_after[peak]) / (height * height_after);
    assert!(
        turned < 0.9,
        "and it turned with the surface rather than staying pointed the way \
         the world is: cos {turned}. A displacement stored in world space \
         would come back at cos 1.0 and would be lying flat across a form \
         that has rolled underneath it"
    );
}

// -- the level sculptor -----------------------------------------------------

#[test]
fn a_stamp_moves_the_level_it_is_bound_to_and_says_which() {
    let mut surface = hierarchy(3, "stamp");
    let before = surface.revision().expect("revision");

    let report = {
        let mut sculptor = surface.sculptor().expect("sculptor");
        sculptor.surface_mut().set_sculpt_level(3).expect("bind");
        sculptor.begin_stroke().expect("begin");
        sculptor
            .stamp(
                MeshStamp {
                    verb: MeshBrush::Draw,
                    center: [0.0, 0.0, 0.0],
                    radius: 0.6,
                    strength: 0.5,
                    geodesic: false,
                    ..Default::default()
                },
                None,
            )
            .expect("stamp")
    };

    assert_eq!(report.level, 3);
    assert!(report.moved_vertices > 0, "the dab reached the surface");
    assert!(
        report.revisions.detail > before.detail && report.revisions.evaluated > before.evaluated,
        "detail changed and the drawn surface moved"
    );
    assert_eq!(
        report.revisions.base, before.base,
        "but the hierarchy's shape did not — three counters, because one \
         cannot say which of the three happened"
    );
}

#[test]
fn a_mask_gates_a_stamp_on_a_hierarchy_as_it_does_everywhere_else() {
    let mut mask = Mask::new(0.05).expect("mask");
    let samples: Vec<StrokeSample> = (0..9)
        .map(|i| {
            let t = i as f32 / 8.0;
            StrokeSample {
                position: [1.2, 0.0, -2.0 + 4.0 * t],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    mask.apply_stroke(
        &samples,
        &StrokePreset {
            radius: 1.0,
            ..Default::default()
        },
        1.0,
        BrushShape::Sphere,
        Falloff::Smooth,
    )
    .expect("paint the freeze");
    assert!(
        mask.sample([1.2, 0.0, 0.0]).expect("sample") > 0.9,
        "the freeze covers where the dab is about to land"
    );

    let dab_here = |surface: &mut Multires, mask: Option<&Mask>| {
        let mut sculptor = surface.sculptor().expect("sculptor");
        sculptor.surface_mut().set_sculpt_level(3).expect("bind");
        sculptor.begin_stroke().expect("begin");
        sculptor
            .stamp(
                MeshStamp {
                    verb: MeshBrush::Draw,
                    center: [1.2, 0.0, 0.0],
                    radius: 0.9,
                    strength: 0.5,
                    geodesic: false,
                    ..Default::default()
                },
                mask.map(|m| &**m),
            )
            .expect("stamp");
    };

    let mut open = hierarchy(3, "ungated");
    let mut frozen = hierarchy(3, "gated");
    dab_here(&mut open, None);
    dab_here(&mut frozen, Some(&mask));

    let ungated = tallest(&mut open, 3);
    let gated = tallest(&mut frozen, 3);
    assert!(
        gated < ungated * 0.5,
        "the freeze held the surface down: {gated} against {ungated} with no \
         mask at all"
    );
}

#[test]
fn a_stroke_is_resolved_into_spaced_stamps_by_the_same_engine_as_a_mesh() {
    let mut surface = hierarchy(3, "stroke");
    let before = surface.revision().expect("revision");
    let path: Vec<[f32; 5]> = (0..9)
        .map(|i| {
            let t = i as f32 / 8.0;
            [-1.5 + 3.0 * t, 0.0, 0.0, 1.0, t]
        })
        .collect();

    let (applied, report) = {
        let mut sculptor = surface.sculptor().expect("sculptor");
        sculptor.surface_mut().set_sculpt_level(3).expect("bind");
        sculptor.begin_stroke().expect("begin");
        sculptor
            .apply_stroke(
                &path,
                &StrokePreset {
                    radius: 0.4,
                    ..Default::default()
                },
                MeshStamp {
                    verb: MeshBrush::Draw,
                    radius: 0.4,
                    strength: 0.5,
                    geodesic: false,
                    ..Default::default()
                },
                None,
                false,
            )
            .expect("stroke")
    };

    assert!(
        applied > 1,
        "a nine-sample path across three units at a 0.4 radius is more than \
         one dab; it resolved into {applied}"
    );
    assert_eq!(
        report.revisions.detail - before.detail,
        applied as u64,
        "the whole call accumulates into the report's revisions, one per stamp"
    );

    let (_, moved) = furthest(&difference(
        &positions(&mut surface, 3),
        &positions(&mut hierarchy(3, "stroke-twin"), 3),
    ));
    assert!(moved > 0.05, "and the stroke left a ridge {moved} deep");

    let (none, _) = {
        let mut sculptor = surface.sculptor().expect("sculptor");
        sculptor
            .apply_stroke(
                &[],
                &StrokePreset::default(),
                MeshStamp::default(),
                None,
                false,
            )
            .expect("an empty stroke is not an error")
    };
    assert_eq!(
        none, 0,
        "and an empty path is nothing rather than a refusal"
    );
}

#[test]
fn the_seed_token_changes_on_every_rebind() {
    // The stale-seed hazard, stated by the header and measured here: a
    // hierarchy renumbers its classes on every rebind, and a seed picked
    // before one is in bounds, wrong, and silent — the walk finds nothing
    // within the radius and the dab is lost rather than misplaced.
    let mut surface = hierarchy(3, "seed");
    let mut sculptor = surface.sculptor().expect("sculptor");

    sculptor.surface_mut().set_sculpt_level(3).expect("bind 3");
    let fine = sculptor.seed_revision().expect("token");
    sculptor.surface_mut().set_sculpt_level(1).expect("bind 1");
    let coarse = sculptor.seed_revision().expect("token");
    sculptor.surface_mut().set_sculpt_level(3).expect("bind 3");
    let again = sculptor.seed_revision().expect("token");

    assert!(fine > 0, "a bound level has a class space to name");
    assert_ne!(fine, coarse, "a level change renumbers");
    assert_ne!(
        fine, again,
        "and coming back to the same level does not restore the old \
         numbering: {fine}, {coarse}, {again}. A host caching a seed across a \
         rebind has to carry this token with it"
    );
}

#[test]
fn deferred_normals_are_stale_until_they_are_flushed() {
    let mut surface = hierarchy(3, "defer");
    let patch;
    let deferred;
    {
        let mut sculptor = surface.sculptor().expect("sculptor");
        assert!(
            !sculptor.defer_normals().expect("read"),
            "nothing is deferred until a host asks for it"
        );
        sculptor.set_defer_normals(true).expect("defer");
        assert!(sculptor.defer_normals().expect("read"));

        sculptor.surface_mut().set_sculpt_level(3).expect("bind");
        sculptor.begin_stroke().expect("begin");
        sculptor
            .stamp(
                MeshStamp {
                    verb: MeshBrush::Draw,
                    center: [0.0, 0.0, 0.0],
                    radius: 0.8,
                    strength: 0.8,
                    geodesic: false,
                    ..Default::default()
                },
                None,
            )
            .expect("stamp");

        patch = sculptor.surface().dirty_blocks().expect("dirty")[0];
        deferred = sculptor
            .surface_mut()
            .copy_block(patch, 3)
            .expect("block")
            .normals;
        sculptor.flush_normals().expect("flush");
    }

    let flushed = surface.copy_block(patch, 3).expect("block").normals;
    let changed = deferred
        .iter()
        .zip(flushed.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 0,
        "a host that defers must flush: {changed} of {} normals in this block \
         were still the ones from before the dab",
        flushed.len()
    );
}

#[test]
fn the_session_peak_records_the_worked_set_and_can_be_reset() {
    let mut surface = hierarchy(3, "peak");
    let mut sculptor = surface.sculptor().expect("sculptor");

    let idle = sculptor.peak_telemetry().expect("telemetry");
    assert_eq!(
        idle.workset_vertices, 0,
        "nothing has been worked on yet, which is the truth rather than a \
         placeholder"
    );
    assert_eq!(
        sculptor.arena_stats().expect("arena").capacity_bytes,
        0,
        "and before the first stamp there is no bound level to own an arena"
    );

    sculptor.surface_mut().set_sculpt_level(3).expect("bind");
    sculptor.begin_stroke().expect("begin");
    sculptor
        .stamp(
            MeshStamp {
                verb: MeshBrush::Draw,
                center: [0.0, 0.0, 0.0],
                radius: 0.8,
                strength: 0.5,
                geodesic: false,
                ..Default::default()
            },
            None,
        )
        .expect("stamp");

    let worked = sculptor.peak_telemetry().expect("telemetry");
    assert!(
        worked.workset_vertices > 0,
        "the high-water mark rose with the dab's footprint"
    );
    let _ = sculptor.arena_stats().expect("arena");

    sculptor.reset_peak_telemetry().expect("reset");
    assert_eq!(
        sculptor
            .peak_telemetry()
            .expect("telemetry")
            .workset_vertices,
        0,
        "and a host tuning a profile can start the measurement again"
    );
}

// -- the changed-block transport --------------------------------------------

#[test]
fn a_stamp_dirties_the_patches_beneath_it_and_a_clear_forgets_them() {
    let mut surface = hierarchy(3, "dirty");
    assert!(
        surface.dirty_blocks().expect("dirty").is_empty(),
        "a hierarchy nobody has touched owes the host no upload"
    );

    dab(&mut surface, 3, [0.0, 0.0, 0.0], 0.6, 0.5);

    let dirty = surface.dirty_blocks().expect("dirty");
    assert!(!dirty.is_empty(), "the dab landed somewhere");
    assert!(
        dirty.len() < surface.level_counts(0).expect("counts").1 as usize,
        "and not everywhere: {} patches of {} — a transport that answered \
         'all of it' would be no transport at all",
        dirty.len(),
        surface.level_counts(0).expect("counts").1
    );
    assert_eq!(
        surface.dirty_block_count(),
        dirty.len(),
        "the count query and the list agree"
    );

    surface.clear_dirty().expect("clear");
    assert!(surface.dirty_blocks().expect("dirty").is_empty());
    assert_eq!(surface.dirty_block_count(), 0);
}

#[test]
fn a_block_is_copied_whole_with_indices_local_to_itself() {
    let mut surface = hierarchy(3, "blocks");
    dab(&mut surface, 3, [0.0, 0.0, 0.0], 0.6, 0.5);
    let patch = surface.dirty_blocks().expect("dirty")[0];

    let quoted = surface.block_info(patch, 3).expect("info");
    let block = surface.copy_block(patch, 3).expect("copy");

    assert_eq!(
        block.info, quoted,
        "the query and the copy build the same block, so the two cannot \
         disagree about what a block contains"
    );
    assert_eq!(block.info.patch, patch);
    assert_eq!(block.info.level, 3);
    assert_eq!(block.positions.len(), quoted.vertex_count as usize);
    assert_eq!(block.normals.len(), quoted.vertex_count as usize);
    assert_eq!(block.indices.len(), quoted.index_count as usize);
    assert_eq!(block.indices.len() % 3, 0, "triangles");
    assert!(
        block.indices.iter().all(|&i| i < block.info.vertex_count),
        "the indices are local to the block, so a host uploads it as a \
         standalone draw"
    );

    let level = positions(&mut surface, 3);
    assert!(
        block.positions.iter().all(|p| level.contains(p)),
        "and every vertex it hands back is one of the level's own"
    );
    assert!(
        block.positions.len() < level.len(),
        "a block is part of a level, not all of it: {} of {}",
        block.positions.len(),
        level.len()
    );
}

// -- persistence ------------------------------------------------------------

#[test]
fn a_hierarchy_round_trips_through_its_own_bytes() {
    let mut surface = hierarchy(3, "encode");
    dab(&mut surface, 3, [0.5, 0.0, -0.5], 0.6, 0.7);
    surface.set_sculpt_level(2).expect("sculpt at 2");
    surface.set_display_level(3).expect("draw at 3");

    let bytes = surface.serialize().expect("serialize");
    assert!(!bytes.is_empty());

    let mut back = Multires::deserialize(&bytes).expect("deserialize");
    assert_eq!(back.level_count(), surface.level_count());
    assert_eq!(
        back.detail_checksum().expect("checksum"),
        surface.detail_checksum().expect("checksum"),
        "the detail came back — it is the one thing in the stream that does \
         not follow from something else"
    );
    assert_eq!(
        (
            back.sculpt_level().expect("sculpt"),
            back.display_level().expect("display")
        ),
        (2, 3),
        "and so did which levels were active"
    );
    assert_eq!(
        positions(&mut back, 3),
        positions(&mut surface, 3),
        "the face lists and every evaluated position are derived and are not \
         written, so this equality is the proof that they were rebuilt right"
    );
}

#[test]
fn a_truncated_blob_is_refused_rather_than_decoded() {
    let surface = hierarchy(2, "truncated");
    let bytes = surface.serialize().expect("serialize");

    assert!(Multires::deserialize(&bytes[..bytes.len() / 2]).is_err());
    assert!(Multires::deserialize(&[]).is_err());
}

#[test]
fn the_encode_preflight_refuses_a_budget_it_cannot_meet() {
    let surface = hierarchy(3, "encode-cost");

    let free = surface.preflight_encode(0).expect("no budget");
    assert!(
        free.allowed,
        "zero means no budget, which is what a desktop passes"
    );
    assert_eq!(free.error, claycore::BudgetError::None);
    assert!(free.peak_bytes >= free.persistent_bytes);
    assert!(
        free.peak_bytes > 0,
        "the blob is a second copy of everything and it exists while the \
         surface still does"
    );

    let squeezed = surface.preflight_encode(16).expect("tiny budget");
    assert!(!squeezed.allowed);
    assert_eq!(squeezed.error, claycore::BudgetError::OverBudget);
    assert_eq!(
        squeezed.peak_bytes, free.peak_bytes,
        "the figures are what they are; the budget only decides the verdict"
    );
}

// -- memory -----------------------------------------------------------------

#[test]
fn the_memory_report_adds_up_the_way_it_says_it_does() {
    let mut surface = hierarchy(3, "memory");
    dab(&mut surface, 3, [0.0, 0.0, 0.0], 0.6, 0.5);
    let memory = surface.memory().expect("memory");

    assert_eq!(
        memory.authoritative,
        memory.base + memory.topology + memory.detail + memory.sculpt_layers,
        "authoritative is the user's work and none of it is droppable"
    );
    assert_eq!(
        memory.rebuildable,
        memory.evaluated + memory.runtime_index + memory.chunk_index + memory.composed,
        "and rebuildable is everything that reconstructs bit-identically"
    );
    assert_eq!(memory.total, memory.authoritative + memory.rebuildable);
    assert!(
        memory.detail > 0,
        "there is a wrinkle on this surface and it costs something"
    );
    assert_eq!(memory.resident_levels, surface.level_count());
}

#[test]
fn the_ledger_says_the_same_thing_in_the_shared_vocabulary() {
    let mut surface = hierarchy(3, "ledger");
    dab(&mut surface, 3, [0.0, 0.0, 0.0], 0.6, 0.5);

    let memory = surface.memory().expect("memory");
    let ledger = surface.memory_ledger().expect("ledger");

    assert_eq!(ledger.essential, memory.authoritative);
    assert_eq!(ledger.rebuildable, memory.rebuildable);
    assert_eq!(ledger.total, memory.total);
    assert_eq!(
        ledger.undoable, 0,
        "undo depth is the host's policy, never the engine's"
    );
    assert_eq!(
        ledger.bytes(MemoryCategory::BaseGeometry),
        Some(memory.base)
    );
    assert_eq!(
        ledger.bytes(MemoryCategory::Topology),
        Some(memory.topology)
    );
    assert_eq!(
        ledger.bytes(MemoryCategory::MultiresDetail),
        Some(memory.detail)
    );
    assert_eq!(
        ledger.essential,
        MemoryCategory::ALL[..5]
            .iter()
            .filter_map(|c| ledger.bytes(*c))
            .sum::<u64>(),
        "the first five categories are the ones releasing destroys work"
    );
}

#[test]
fn releasing_the_caches_changes_nothing_that_matters() {
    let mut surface = hierarchy(3, "drop-caches");
    dab(&mut surface, 3, [0.0, 0.0, 0.0], 0.6, 0.5);

    let checksum = surface.detail_checksum().expect("checksum");
    let before = positions(&mut surface, 3);

    surface.drop_inactive_caches().expect("drop");

    assert_eq!(
        surface.detail_checksum().expect("checksum"),
        checksum,
        "the checksum is how a host proves this to itself rather than taking \
         the sentence on trust"
    );
    assert_eq!(
        positions(&mut surface, 3),
        before,
        "and rebuilding them reproduces the surface bit for bit"
    );
}

#[test]
fn a_trim_releases_caches_and_never_work() {
    let mut surface = hierarchy(3, "trim");
    dab(&mut surface, 3, [0.0, 0.0, 0.0], 0.6, 0.5);

    let checksum = surface.detail_checksum().expect("checksum");
    let before = positions(&mut surface, 3);
    let held = surface.memory().expect("memory");

    let report = surface
        .trim(Pressure::Critical, None)
        .expect("release everything rebuildable");

    assert_eq!(
        report.pressure,
        Pressure::Critical,
        "the pressure is echoed"
    );
    assert!(!report.pinned);
    assert!(
        report.total_released > 0,
        "at the last stop before the operating system kills the process, \
         everything rebuildable goes"
    );
    for category in [
        MemoryCategory::BaseGeometry,
        MemoryCategory::Topology,
        MemoryCategory::MultiresDetail,
        MemoryCategory::SculptLayers,
        MemoryCategory::Masks,
        MemoryCategory::UndoHistory,
    ] {
        assert_eq!(
            report.released(category),
            Some(0),
            "{} is the user's work or the host's policy and a trim never \
             touches it",
            category.text()
        );
    }
    assert!(
        surface.memory().expect("memory").rebuildable < held.rebuildable,
        "and the caches genuinely went"
    );
    assert_eq!(surface.detail_checksum().expect("checksum"), checksum);
    assert_eq!(
        positions(&mut surface, 3),
        before,
        "the next read pays to rebuild what it needs, and gets the same thing"
    );
}

#[test]
fn a_held_pin_makes_a_trim_a_question_rather_than_an_answer() {
    let mut surface = hierarchy(3, "pin");
    dab(&mut surface, 3, [0.0, 0.0, 0.0], 0.6, 0.5);
    let held = surface.memory().expect("memory");

    let mut pin = MemoryPin::new().expect("pin");
    assert!(!pin.is_held());
    pin.acquire().expect("acquire");
    assert!(pin.is_held());

    let asked = surface.trim(Pressure::Critical, Some(&pin)).expect("trim");
    assert!(
        asked.pinned,
        "a save is running, so nothing may move under it"
    );
    assert!(
        asked.total_released > 0,
        "and the figures are what it WOULD have released, which is the honest \
         answer rather than a zero"
    );
    assert_eq!(
        surface.memory().expect("memory").rebuildable,
        held.rebuildable,
        "nothing was actually released"
    );

    pin.release().expect("release");
    assert!(!pin.is_held());
    let done = surface.trim(Pressure::Critical, Some(&pin)).expect("trim");
    assert!(!done.pinned);
    assert!(surface.memory().expect("memory").rebuildable < held.rebuildable);
}

#[test]
fn a_pin_is_a_counter_rather_than_a_flag() {
    // A readback inside a save must not un-pin the save when it returns.
    let mut pin = MemoryPin::new().expect("pin");
    pin.acquire().expect("the save takes it");
    pin.acquire().expect("the readback inside it takes it too");
    pin.release().expect("the readback returns");
    assert!(pin.is_held(), "the save is still running");
    pin.release().expect("the save finishes");
    assert!(!pin.is_held());
    pin.release()
        .expect("releasing a pin nobody acquired is harmless rather than an underflow");
    assert!(!pin.is_held());
}

#[test]
fn the_memory_profile_is_read_back_as_it_was_declared() {
    let defaults = SculptMemoryProfile::defaults().expect("the library's own");
    assert_eq!(
        defaults.class,
        MemoryClass::Full,
        "no budget is what a desktop host and every existing caller get"
    );
    assert_eq!(defaults.cache_budget, 0, "and every byte field is advisory");

    let mut surface = hierarchy(3, "profile");
    assert_eq!(surface.memory_profile().expect("profile"), defaults);

    let constrained = SculptMemoryProfile {
        class: MemoryClass::Constrained,
        cache_budget: 8 * 1024 * 1024,
        max_resident_levels: 2,
        defer_normals_in_stroke: true,
        ..defaults
    };
    surface
        .set_memory_profile(constrained)
        .expect("declare a budget");
    assert_eq!(
        surface.memory_profile().expect("profile"),
        constrained,
        "the profile is set on the hierarchy alone because it is the \
         representation that holds levels"
    );
}

#[test]
fn compacting_the_layer_storage_leaves_the_detail_alone() {
    let mut surface = hierarchy(3, "compact");
    dab(&mut surface, 3, [0.0, 0.0, 0.0], 0.6, 0.5);
    let checksum = surface.detail_checksum().expect("checksum");
    let before = positions(&mut surface, 3);

    surface.compact_sculpt_layers().expect("compact");

    assert_eq!(surface.detail_checksum().expect("checksum"), checksum);
    assert_eq!(
        positions(&mut surface, 3),
        before,
        "reclaiming the storage a pass that undid itself left behind is not \
         allowed to move a vertex"
    );
}

// -- the vocabulary ---------------------------------------------------------

#[test]
fn every_refusal_has_a_sentence_of_its_own() {
    let named = [
        MultiresError::EmptyBase,
        MultiresError::IndexOutOfRange,
        MultiresError::DegenerateFace,
        MultiresError::NonManifold,
        MultiresError::LevelOutOfRange,
        MultiresError::NoLevelToRemove,
        MultiresError::OverBudget,
        MultiresError::Cancelled,
        MultiresError::DetailPresent,
        MultiresError::DepthLimit,
        MultiresError::Decode,
        MultiresError::NoSuchSculptLayer,
        MultiresError::SculptLayerLocked,
        MultiresError::SculptLayerStrokeOpen,
        MultiresError::CapacityOverflow,
    ];
    for reason in named {
        let text = reason.text();
        assert!(!text.is_empty(), "{reason:?} says nothing");
        assert_ne!(
            text, "unknown",
            "{reason:?} is a value this build knows and the engine should \
             have a sentence for it"
        );
    }

    let mut sentences: Vec<&str> = named.iter().map(|e| e.text()).collect();
    sentences.sort_unstable();
    sentences.dedup();
    assert_eq!(
        sentences.len(),
        named.len(),
        "the three sentences a host UI has to be able to say are three \
         different sentences"
    );

    assert_eq!(
        MultiresError::None.to_string(),
        MultiresError::None.text(),
        "and the display is the engine's own words"
    );
    assert!(!MultiresError::Unknown(9_999).text().is_empty());
}

#[test]
fn the_budget_refusals_are_two_different_things() {
    assert_ne!(
        claycore::BudgetError::OverBudget.text(),
        claycore::BudgetError::Overflow.text(),
        "over budget is 'the answer is no'; an overflow is 'nobody can \
         compute this', which is a refusal at any budget including none"
    );
    assert!(!claycore::BudgetError::None.text().is_empty());
    assert!(!claycore::BudgetError::Unknown(9_999).text().is_empty());
}

#[test]
fn the_memory_vocabulary_is_the_engines_own() {
    for category in MemoryCategory::ALL {
        assert!(!category.text().is_empty(), "{category:?}");
        assert_ne!(category.text(), "unknown", "{category:?}");
    }
    let mut indices: Vec<usize> = MemoryCategory::ALL.iter().map(|c| c.index()).collect();
    indices.sort_unstable();
    assert_eq!(
        indices,
        (0..MemoryCategory::ALL.len()).collect::<Vec<_>>(),
        "the categories index a dense array and each one owns a slot"
    );

    for pressure in Pressure::ALL {
        assert!(!pressure.text().is_empty(), "{pressure:?}");
    }
    for class in [
        MemoryClass::Full,
        MemoryClass::Constrained,
        MemoryClass::Minimal,
    ] {
        assert!(!class.text().is_empty(), "{class:?}");
    }
}
