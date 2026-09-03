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
    BrushShape, DetailStamp, DetailStampMode, Falloff, Mask, MemoryCategory, MemoryClass,
    MemoryPin, Mesh, MeshBrush, MeshStamp, Multires, MultiresDesc, MultiresError, Pressure,
    SculptLayerId, SculptLayerKind, SculptLayerStats, SculptMemoryProfile, SmoothMode,
    StrokePreset, StrokeSample, WriteDomain,
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

/// How far the surface stands off the sheet within one brush width of `at`.
///
/// The sheet is flat in xz, so distance is measured there and height is the y
/// the vertex was pushed to. It is what separates two bumps on one surface —
/// the assertion "this one came down and that one did not" needs to be able to
/// name them apart, and [`tallest`] cannot.
fn height_near(surface: &mut Multires, level: u32, at: [f32; 3]) -> f32 {
    positions(surface, level)
        .iter()
        .filter(|p| {
            let (dx, dz) = (p[0] - at[0], p[2] - at[2]);
            (dx * dx + dz * dz).sqrt() < 1.0
        })
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

// -- the sculpt layer stack -------------------------------------------------
//
// A second stack in this workspace wears the word "sculpt layer" — the voxel
// grid's, which `claycore::VoxelGrid` addresses by `usize` position. The two
// share the noun on purpose and share nothing else, so the tests below are
// written against the differences rather than the similarity: an id survives a
// reorder where a position does not, and a reorder here is *defined* to move
// no vertex where the grid's stack replays cell writes and is order-dependent.

/// A pass with detail on it, at `level`, on `count` consecutive vertices.
///
/// Written through the coefficient setter rather than a stroke because these
/// tests are about the stack's arithmetic and a stroke would put the brush's
/// falloff between the assertion and the thing being asserted.
fn pass_with_detail(
    surface: &mut Multires,
    name: &str,
    level: u32,
    vertices: std::ops::Range<u32>,
    height: f32,
) -> SculptLayerId {
    let id = surface.add_sculpt_layer(Some(name)).expect("a pass");
    for vertex in vertices {
        surface
            .set_sculpt_layer_detail(id, level, vertex, [0.0, 0.0, height])
            .expect("write a coefficient");
    }
    id
}

/// One Draw dab into whatever channel the stroke opened on.
fn layered_dab(stroke: &mut claycore::SculptLayerStroke<'_>, center: [f32; 3], strength: f32) {
    stroke
        .stamp(
            MeshStamp {
                verb: MeshBrush::Draw,
                center,
                radius: 1.0,
                strength,
                geodesic: false,
                ..Default::default()
            },
            None,
        )
        .expect("stamp");
}

/// A whole gesture into one channel, opened, stamped once and committed.
fn gesture(surface: &mut Multires, domain: WriteDomain, center: [f32; 3], strength: f32) -> usize {
    let mut stroke = surface.sculpt_layer_stroke().expect("a transaction");
    stroke.set_write_domain(domain).expect("choose the channel");
    stroke.begin().expect("open the gesture");
    layered_dab(&mut stroke, center, strength);
    stroke.commit().expect("commit")
}

// -- the stack's lifecycle --------------------------------------------------

#[test]
fn a_fresh_hierarchy_has_no_passes_and_writes_into_the_form_itself() {
    let surface = hierarchy(1, "empty-stack");

    assert_eq!(
        surface.sculpt_layer_count().expect("count"),
        0,
        "a hierarchy arrives with base detail and nothing over it"
    );
    assert_eq!(
        surface.active_sculpt_layer().expect("active"),
        SculptLayerId::BASE,
        "so the next write lands in the form under the passes — which is what \
         every stroke did before this stack existed"
    );
    assert!(SculptLayerId::BASE.is_base());
}

#[test]
fn a_pass_arrives_on_top_full_strength_visible_and_active() {
    let mut surface = hierarchy(1, "first-pass");
    let id = surface.add_sculpt_layer(Some("rugas")).expect("a pass");

    assert_eq!(surface.sculpt_layer_count().expect("count"), 1);
    assert_eq!(
        surface.active_sculpt_layer().expect("active"),
        id,
        "a new pass is made active, because the reason to make one is to draw \
         into it"
    );
    assert_eq!(surface.sculpt_layer_name(id).expect("name"), "rugas");

    let info = surface.sculpt_layer_info(id).expect("info");
    assert_eq!(info.id, id);
    assert_eq!(
        info.index, 0,
        "the only pass sits at the bottom of the stack"
    );
    assert_eq!(info.kind, SculptLayerKind::Sampled);
    assert_eq!(info.strength, 1.0);
    assert!(info.visible);
    assert!(!info.locked);
    assert_eq!(
        info.name_bytes as usize,
        "rugas".len() + 1,
        "the descriptor sizes the name buffer, terminator included, so a host \
         does not call twice"
    );
    assert_eq!(
        info.coverage_vertices, 0,
        "and it costs its coverage, which is nothing until something is drawn"
    );

    let unnamed = surface.add_sculpt_layer(None).expect("an unnamed pass");
    assert_eq!(
        surface.sculpt_layer_name(unnamed).expect("name"),
        "",
        "an unnamed pass is a pass, not a refusal"
    );
}

#[test]
fn the_stack_is_walked_by_position_and_held_by_id() {
    let mut surface = hierarchy(1, "ids");
    let bottom = surface.add_sculpt_layer(Some("um")).expect("pass");
    let middle = surface.add_sculpt_layer(Some("dois")).expect("pass");
    let top = surface.add_sculpt_layer(Some("tres")).expect("pass");

    assert_eq!(
        surface.sculpt_layer_ids().expect("the stack"),
        vec![bottom, middle, top],
        "positions run bottom-first, which is the order a host draws its list \
         in"
    );

    surface.move_sculpt_layer(top, 0).expect("slide it under");

    assert_eq!(
        surface.sculpt_layer_ids().expect("the stack"),
        vec![top, bottom, middle],
        "a reorder renumbers every position at or below the layer it moves"
    );
    assert_eq!(
        surface.sculpt_layer_info(bottom).expect("info").index,
        1,
        "so the pass that was at position 0 is at position 1 now — which is \
         exactly why a position must not be written into a file or held \
         across a drag"
    );
    assert_eq!(
        surface.sculpt_layer_name(bottom).expect("name"),
        "um",
        "while its id still names the same pass, because an id is minted once"
    );
    assert!(
        surface.sculpt_layer_id_at(3).is_err(),
        "past the end is a refusal and not a zero"
    );
}

#[test]
fn removing_a_pass_leaves_every_other_one_where_it_was() {
    let mut surface = hierarchy(1, "remove");
    let under = pass_with_detail(&mut surface, "baixo", 1, 8..16, 0.4);
    let doomed = pass_with_detail(&mut surface, "meio", 1, 8..16, 0.4);
    let over = pass_with_detail(&mut surface, "cima", 1, 8..16, 0.4);
    surface
        .set_sculpt_layer_strength(under, 0.25)
        .expect("dial the lower pass");

    let coefficient = surface
        .sculpt_layer_detail(over, 1, 12)
        .expect("the upper pass's coefficient");

    surface.remove_sculpt_layer(doomed).expect("discard");

    assert_eq!(
        surface.sculpt_layer_ids().expect("stack"),
        vec![under, over]
    );
    assert_eq!(
        surface
            .sculpt_layer_detail(over, 1, 12)
            .expect("still there"),
        coefficient,
        "removing a pass re-evaluates its coverage and nothing else: no stroke \
         is replayed and no other pass's coefficients change"
    );
    assert_eq!(
        surface.sculpt_layer_info(under).expect("info").strength,
        0.25,
        "nor its strength"
    );
    assert_eq!(
        surface
            .remove_sculpt_layer(SculptLayerId::from_raw(9_999))
            .expect_err("an id nobody minted")
            .reason,
        MultiresError::NoSuchSculptLayer,
        "and the refusal says which of the three sentences a host has to be \
         able to say"
    );
}

// -- the per-layer properties -----------------------------------------------

#[test]
fn strength_is_composition_and_not_a_scale_on_the_pen() {
    // The behaviour the header expects to be reported as a bug: a stroke into
    // a pass at half strength records its FULL contribution, so the surface
    // moves half as far as the pen asked for — and raising the slider
    // afterwards doubles what is on screen without replaying a stroke.
    let mut surface = hierarchy(2, "strength");
    surface.set_sculpt_level(2).expect("bind the fine level");
    let pass = surface.add_sculpt_layer(Some("meia")).expect("pass");
    surface
        .set_sculpt_layer_strength(pass, 0.5)
        .expect("half strength before the gesture");

    gesture(&mut surface, WriteDomain::Detail, [0.0, 0.0, 0.0], 0.8);
    let half = tallest(&mut surface, 2);
    assert!(half > 0.01, "the gesture reached the surface: {half}");

    surface
        .set_sculpt_layer_strength(pass, 1.0)
        .expect("raise the slider afterwards");
    let full = tallest(&mut surface, 2);

    assert!(
        (full / half - 2.0).abs() < 1e-3,
        "raising the slider from 0.5 to 1 doubles what is on screen — {full} \
         against {half} — because the pass stored what the pen asked for and \
         the slider is composition"
    );
}

#[test]
fn hiding_a_pass_removes_its_contribution_bit_for_bit() {
    let mut surface = hierarchy(1, "hidden");
    let pass = pass_with_detail(&mut surface, "escondida", 1, 8..24, 0.5);

    let with = positions(&mut surface, 1);
    surface
        .set_sculpt_layer_visible(pass, false)
        .expect("hide it");
    let without = positions(&mut surface, 1);
    surface
        .set_sculpt_layer_visible(pass, true)
        .expect("show it again");
    let back = positions(&mut surface, 1);

    assert_ne!(
        with, without,
        "hiding a pass with content changes the surface"
    );
    assert_eq!(
        back, with,
        "and showing it puts every vertex back exactly — invisible is exactly \
         zero rather than nearly zero, which is why a host may compare the two"
    );

    surface
        .set_sculpt_layer_strength(pass, 0.0)
        .expect("dial to nothing");
    assert_eq!(
        positions(&mut surface, 1),
        without,
        "and strength zero reaches the same surface by the other route"
    );
}

#[test]
fn a_lock_refuses_a_coefficient_write_and_permits_every_property_change() {
    // Stated rather than discovered: locking exists so an artist can keep
    // working over a finished pass, and a lock that also froze the name and
    // the slider would make "lock" mean "hide from the interface".
    let mut surface = hierarchy(2, "locked");
    surface.set_sculpt_level(2).expect("bind");
    let pass = surface.add_sculpt_layer(Some("pronta")).expect("pass");
    surface
        .set_sculpt_layer_locked(pass, true)
        .expect("lock it");

    let mut stroke = surface.sculpt_layer_stroke().expect("a transaction");
    stroke
        .set_write_domain(WriteDomain::Detail)
        .expect("into the pass");
    assert_eq!(
        stroke
            .begin()
            .expect_err("a locked pass takes no coefficients")
            .reason,
        MultiresError::SculptLayerLocked,
        "and the refusal names the lock rather than reading as an invalid \
         argument"
    );
    drop(stroke);

    surface
        .rename_sculpt_layer(pass, "ainda pronta")
        .expect("a rename moves no vertex");
    surface
        .set_sculpt_layer_strength(pass, 0.4)
        .expect("nor does a slider");
    surface
        .set_sculpt_layer_visible(pass, false)
        .expect("nor does hiding it");
    assert_eq!(
        surface.sculpt_layer_name(pass).expect("name"),
        "ainda pronta"
    );

    surface
        .set_sculpt_layer_locked(pass, false)
        .expect("unlock");
    let mut stroke = surface.sculpt_layer_stroke().expect("a transaction");
    stroke
        .set_write_domain(WriteDomain::Detail)
        .expect("domain");
    stroke.begin().expect("and now it opens");
    stroke.commit().expect("commit");
}

// -- the two pieces of arithmetic that are traps ----------------------------

#[test]
fn merging_down_holds_visual_parity_at_strength_zero() {
    // The trap: the naive merge solves L' = L_l + (s_u*m_u)/(s_l*m_l)*L_u,
    // which divides by the LOWER pass's strength — and zero is a state one
    // slider reaches. The stack stores the sum directly and sets the target's
    // composition to the identity it needs, so parity holds by construction.
    // Here it is measured, at zero and at a half.
    for (label, lower_strength) in [("zero", 0.0f32), ("half", 0.5), ("one", 1.0)] {
        let mut surface = hierarchy(1, &format!("merge-{label}"));
        let lower = pass_with_detail(&mut surface, "baixo", 1, 8..24, 0.5);
        let upper = pass_with_detail(&mut surface, "cima", 1, 12..28, 0.3);
        surface
            .set_sculpt_layer_strength(lower, lower_strength)
            .expect("dial the target");
        surface
            .set_sculpt_layer_strength(upper, 0.75)
            .expect("dial the pass being folded in");

        let before = positions(&mut surface, 1);
        surface
            .merge_sculpt_layer_down(upper)
            .expect("fold it into the pass below");
        let after = positions(&mut surface, 1);

        assert_eq!(
            before, after,
            "merge-down is defined by the surface it leaves, not by a \
             concatenation, so the evaluated surface is unchanged at lower \
             strength {lower_strength} — the case the naive arithmetic is \
             undefined at is {label}"
        );
        assert_eq!(
            surface.sculpt_layer_ids().expect("stack"),
            vec![lower],
            "and the folded pass is gone"
        );
        assert!(
            surface.sculpt_layer_info(lower).expect("info").strength > 0.0,
            "the target's composition was set to the identity the sum needs \
             rather than left where the slider was, which is the whole reason \
             nothing had to divide by a strength"
        );
    }
}

#[test]
fn a_pass_the_artist_never_masked_contributes_at_full_weight() {
    // A sparse mask's identity is 1 and not 0. An absent block means full
    // weight, so a mask nobody touched must not erase the pass it belongs to —
    // which is what a zero identity would do to every pass in the file.
    let mut untouched = hierarchy(1, "mask-untouched");
    let pass = pass_with_detail(&mut untouched, "sem mascara", 1, 8..24, 0.5);

    assert_eq!(
        untouched
            .sculpt_layer_mask(pass, 1, 12)
            .expect("read it back"),
        1.0,
        "a vertex nobody masked reads as full weight"
    );

    let mut written = hierarchy(1, "mask-written");
    let same = pass_with_detail(&mut written, "sem mascara", 1, 8..24, 0.5);
    for vertex in 0..48 {
        written
            .set_sculpt_layer_mask(same, 1, vertex, 1.0)
            .expect("say explicitly what the identity already says");
    }

    assert_eq!(
        positions(&mut untouched, 1),
        positions(&mut written, 1),
        "so a pass with no mask and a pass masked to 1 everywhere are the same \
         surface, vertex for vertex"
    );

    let flat = positions(&mut hierarchy(1, "mask-flat"), 1);
    assert_ne!(
        positions(&mut untouched, 1),
        flat,
        "and neither of them is the untouched cage, which is what an identity \
         of 0 would have made both of them"
    );

    untouched
        .set_sculpt_layer_mask(pass, 1, 12, 0.0)
        .expect("mask one vertex out");
    assert_ne!(
        positions(&mut untouched, 1),
        positions(&mut written, 1),
        "a mask written to zero does stop the pass contributing there — the \
         identity is 1 because it is the absence that means full weight, not \
         because the mask does nothing"
    );
}

#[test]
fn reordering_an_additive_stack_moves_nothing() {
    // The requirement says a reorder changes organisation and not geometry,
    // and the case that asserted it could not fail: `move_to` invalidates no
    // block on purpose, so the comparison was the composed cache against
    // itself. A randomised probe over 300 five-layer stacks caught 158 of them
    // moving by a last bit, because the blocks a later stroke recomposed
    // carried the new order while cached blocks carried the old — the surface
    // composed two ways at once.
    //
    // So this is that probe rather than a single reorder: several hundred
    // stacks of five passes at random strengths with random coefficients on
    // random vertices, reordered at random, compared BIT FOR BIT. A fixed seed
    // makes a failure reproducible; a floating tolerance would hide exactly
    // the last-bit disagreement the probe exists to find.
    const STACKS: usize = 300;
    const PASSES: usize = 5;

    let mut surface = hierarchy(1, "reorder-probe");
    let vertices = surface.level_counts(1).expect("counts").0 as u32;
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    let mut moved = Vec::new();
    for stack in 0..STACKS {
        let ids: Vec<SculptLayerId> = (0..PASSES)
            .map(|_| surface.add_sculpt_layer(None).expect("a pass"))
            .collect();
        for &id in &ids {
            for _ in 0..8 {
                let vertex = (next() % vertices as u64) as u32;
                let coefficient = |raw: u64| (raw % 2001) as f32 / 1000.0 - 1.0;
                let tbn = [
                    coefficient(next()),
                    coefficient(next()),
                    coefficient(next()),
                ];
                surface
                    .set_sculpt_layer_detail(id, 1, vertex, tbn)
                    .expect("write a coefficient");
            }
            let strength = (next() % 1001) as f32 / 1000.0;
            surface
                .set_sculpt_layer_strength(id, strength)
                .expect("dial it");
        }

        let before = positions(&mut surface, 1);
        for _ in 0..PASSES + 1 {
            let which = ids[(next() % PASSES as u64) as usize];
            let to = (next() % PASSES as u64) as usize;
            surface.move_sculpt_layer(which, to).expect("slide it");
        }
        let after = positions(&mut surface, 1);

        if before != after {
            let worst = before
                .iter()
                .zip(&after)
                .flat_map(|(a, b)| (0..3).map(move |i| (a[i] - b[i]).abs()))
                .fold(0.0f32, f32::max);
            moved.push((stack, worst));
        }

        for id in ids {
            surface.remove_sculpt_layer(id).expect("clear the stack");
        }
    }

    assert!(
        moved.is_empty(),
        "an additive stack commutes, so composition is invariant under exactly \
         the operation the requirement calls free — but {} of {STACKS} \
         randomised stacks moved, worst delta {:e}. The engine sums a block's \
         contributors in layer-id order to make this true; a stack that moves \
         here is a cached block composed in one order beside a recomposed one \
         in another.",
        moved.len(),
        moved.iter().map(|(_, d)| *d).fold(0.0f32, f32::max)
    );
}

// -- the layered stroke transaction -----------------------------------------

#[test]
fn a_gesture_enters_one_channel_fixed_when_it_opened() {
    // The first of the three reasons this is a transaction and not a loop of
    // stamps: a host that changes the active pass mid-stroke would otherwise
    // split one gesture across two channels.
    let mut surface = hierarchy(2, "one-channel");
    surface.set_sculpt_level(2).expect("bind");
    let first = surface.add_sculpt_layer(Some("uma")).expect("pass");
    let second = surface.add_sculpt_layer(Some("outra")).expect("pass");
    surface
        .set_active_sculpt_layer(first)
        .expect("draw into the first");

    let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
    stroke
        .set_write_domain(WriteDomain::Detail)
        .expect("domain");
    stroke.begin().expect("open");
    assert_eq!(stroke.target_layer().expect("target"), first);

    stroke
        .surface_mut()
        .set_active_sculpt_layer(second)
        .expect("a set-active moves no vertex, so it is permitted mid-gesture");
    layered_dab(&mut stroke, [0.0, 0.0, 0.0], 0.8);
    assert_eq!(
        stroke.target_layer().expect("target"),
        first,
        "and the channel is still the one the gesture opened on"
    );
    stroke.commit().expect("commit");
    drop(stroke);

    assert!(
        surface
            .sculpt_layer_info(first)
            .expect("info")
            .coverage_vertices
            > 0,
        "the stamps landed in the pass the gesture opened on"
    );
    assert_eq!(
        surface
            .sculpt_layer_info(second)
            .expect("info")
            .coverage_vertices,
        0,
        "and none of them reached the pass that became active mid-gesture"
    );
    assert_eq!(
        surface.active_sculpt_layer().expect("active"),
        first,
        "committing restores the stack's active pass to what the gesture found"
    );
}

#[test]
fn the_write_domain_is_chosen_by_the_caller_and_never_inferred() {
    // "Sculpt the pass I am working on" and "fix the form UNDER the passes
    // without disturbing them" are both ordinary and neither is a default.
    let mut surface = hierarchy(2, "domains");
    surface.set_sculpt_level(2).expect("bind");
    let pass = surface.add_sculpt_layer(Some("passe")).expect("pass");

    let form_before = surface.detail_checksum().expect("the form's checksum");
    let stack_before = surface.sculpt_layer_checksum().expect("the stack's");

    {
        let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
        stroke
            .set_write_domain(WriteDomain::Geometry)
            .expect("into the form");
        stroke.begin().expect("open");
        assert_eq!(
            stroke.target_layer().expect("target"),
            SculptLayerId::BASE,
            "the base is not a pass, and says so"
        );
        layered_dab(&mut stroke, [0.0, 0.0, 0.0], 0.8);
        stroke.commit().expect("commit");
    }

    assert_ne!(
        surface.detail_checksum().expect("checksum"),
        form_before,
        "a Geometry gesture writes the level's own detail"
    );
    assert_eq!(
        surface.sculpt_layer_checksum().expect("checksum"),
        stack_before,
        "and touches no pass — which is what the domain exists to make sayable"
    );
    assert_eq!(
        surface
            .sculpt_layer_info(pass)
            .expect("info")
            .coverage_vertices,
        0
    );

    surface
        .set_active_sculpt_layer(SculptLayerId::BASE)
        .expect("no active pass");
    let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
    stroke
        .set_write_domain(WriteDomain::Detail)
        .expect("domain");
    assert_eq!(
        stroke
            .begin()
            .expect_err("Detail with no active pass has nowhere to write")
            .reason,
        MultiresError::NoSuchSculptLayer,
        "and it refuses rather than silently writing the form the caller asked \
         not to touch"
    );
}

#[test]
fn a_cancel_restores_the_channel_exactly() {
    // A layered write is `L += dE`, so the only exact restore is the recorded
    // `before` values — which is why the record has to exist from the first
    // stamp rather than be reconstructed at the end.
    let mut surface = hierarchy(2, "cancel");
    surface.set_sculpt_level(2).expect("bind");
    surface.add_sculpt_layer(Some("passe")).expect("pass");
    gesture(&mut surface, WriteDomain::Detail, [0.6, 0.0, 0.6], 0.7);

    let checksum = surface.sculpt_layer_checksum().expect("checksum");
    let before = positions(&mut surface, 2);

    let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
    stroke
        .set_write_domain(WriteDomain::Detail)
        .expect("domain");
    stroke.begin().expect("open");
    layered_dab(&mut stroke, [-0.4, 0.0, -0.4], 0.9);
    layered_dab(&mut stroke, [0.0, 0.0, 0.0], 0.9);
    assert_ne!(
        positions(stroke.surface_mut(), 2),
        before,
        "the gesture is visible while it is open"
    );
    stroke.cancel().expect("discard");
    drop(stroke);

    assert_eq!(
        positions(&mut surface, 2),
        before,
        "and cancel restores the recorded values rather than recomputing them"
    );
    assert_eq!(
        surface.sculpt_layer_checksum().expect("checksum"),
        checksum,
        "down to the coefficients themselves"
    );
}

#[test]
fn a_dropped_gesture_cancels_rather_than_banking_half_of_it() {
    // A `?` between begin and commit, or a panic, leaves a transaction with a
    // held composition and a half-written channel. Committing would bank half
    // a gesture and destroying without either would leave the composition held
    // on a surface with no transaction left to release it, so the wrapper
    // cancels.
    let mut surface = hierarchy(2, "dropped");
    surface.set_sculpt_level(2).expect("bind");
    let pass = surface.add_sculpt_layer(Some("passe")).expect("pass");
    let before = positions(&mut surface, 2);

    {
        let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
        stroke
            .set_write_domain(WriteDomain::Detail)
            .expect("domain");
        stroke.begin().expect("open");
        layered_dab(&mut stroke, [0.0, 0.0, 0.0], 0.9);
        // Falls out of scope with the gesture open.
    }

    assert_eq!(
        positions(&mut surface, 2),
        before,
        "the half-gesture was discarded exactly"
    );
    surface.set_sculpt_layer_strength(pass, 0.5).expect(
        "and the composition hold came back with it — a slider that \
                 is still refused here would mean the hold outlived the \
                 transaction and nothing could release it",
    );
}

#[test]
fn a_composition_change_is_refused_while_a_gesture_is_open() {
    // A stamp reads the evaluated surface, so a slider moved between two
    // stamps would author one gesture against two different surfaces.
    // Refusing rather than deferring until commit is deliberate: a slider that
    // appears to move and then silently applies later is the worse surprise.
    let mut surface = hierarchy(2, "held");
    surface.set_sculpt_level(2).expect("bind");
    let pass = surface.add_sculpt_layer(Some("passe")).expect("pass");

    let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
    stroke
        .set_write_domain(WriteDomain::Detail)
        .expect("domain");
    stroke.begin().expect("open");

    let held = stroke.surface_mut();
    assert_eq!(
        held.set_sculpt_layer_strength(pass, 0.5)
            .expect_err("a slider is composition")
            .reason,
        MultiresError::SculptLayerStrokeOpen
    );
    assert_eq!(
        held.set_sculpt_layer_visible(pass, false)
            .expect_err("so is hiding it")
            .reason,
        MultiresError::SculptLayerStrokeOpen
    );
    assert_eq!(
        held.add_sculpt_layer(Some("outra"))
            .expect_err("and so is adding a pass")
            .reason,
        MultiresError::SculptLayerStrokeOpen
    );
    held.rename_sculpt_layer(pass, "renomeada")
        .expect("a rename moves no vertex, so it is permitted");
    held.set_sculpt_layer_locked(pass, true)
        .expect("nor does a lock");
    held.set_sculpt_layer_locked(pass, false).expect("unlock");

    stroke.commit().expect("commit");
    drop(stroke);
    surface
        .set_sculpt_layer_strength(pass, 0.5)
        .expect("and the slider is free again once the gesture closes");
}

#[test]
fn the_record_follows_the_vertices_reached_and_not_the_stamps_taken() {
    let mut surface = hierarchy(2, "record");
    surface.set_sculpt_level(2).expect("bind");
    surface.add_sculpt_layer(Some("passe")).expect("pass");

    let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
    stroke
        .set_write_domain(WriteDomain::Detail)
        .expect("domain");
    stroke.begin().expect("open");
    assert_eq!(stroke.stamps().expect("stamps"), 0);
    assert_eq!(stroke.record_size().expect("record"), 0);

    for _ in 0..12 {
        layered_dab(&mut stroke, [0.0, 0.0, 0.0], 0.2);
    }
    let stamps = stroke.stamps().expect("stamps");
    let entries = stroke.record_size().expect("record");
    assert_eq!(stamps, 12);
    assert!(
        entries > stamps,
        "twelve stamps over one place reach many vertices: {entries} entries \
         for {stamps} stamps"
    );

    let after = stroke.record_size().expect("record");
    for _ in 0..12 {
        layered_dab(&mut stroke, [0.0, 0.0, 0.0], 0.2);
    }
    assert_eq!(
        stroke.record_size().expect("record"),
        after,
        "and twelve more over the same place add no entries at all — the \
         record's size follows the vertices the gesture reached, so a hundred \
         stamps over one vertex is one entry"
    );
    assert_eq!(
        stroke.commit().expect("commit"),
        after,
        "which is the count commit reports"
    );
}

#[test]
fn a_gesture_that_changed_nothing_commits_an_empty_record() {
    let mut surface = hierarchy(2, "empty-gesture");
    surface.set_sculpt_level(2).expect("bind");
    surface.add_sculpt_layer(Some("passe")).expect("pass");

    let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
    stroke
        .set_write_domain(WriteDomain::Detail)
        .expect("domain");
    stroke.begin().expect("open");
    assert_eq!(
        stroke.commit().expect("commit"),
        0,
        "an empty record rather than a step"
    );
}

#[test]
fn erasing_takes_this_pass_toward_zero_and_leaves_the_form_alone() {
    let mut surface = hierarchy(2, "erase");
    surface.set_sculpt_level(2).expect("bind");
    let pass = surface.add_sculpt_layer(Some("passe")).expect("pass");

    // A bump in the form, away from where the eraser will run.
    gesture(&mut surface, WriteDomain::Geometry, [1.3, 0.0, 1.3], 0.8);
    // And a bump in the pass, at the origin.
    gesture(&mut surface, WriteDomain::Detail, [0.0, 0.0, 0.0], 0.8);

    let form = surface.detail_checksum().expect("the form's checksum");
    let over_the_pass = |surface: &mut Multires| height_near(surface, 2, [0.0, 0.0, 0.0]);
    let over_the_form = |surface: &mut Multires| height_near(surface, 2, [1.3, 0.0, 1.3]);
    let (pass_before, form_before) = (over_the_pass(&mut surface), over_the_form(&mut surface));
    assert!(
        pass_before > 0.1 && form_before > 0.1,
        "both bumps are there"
    );

    {
        let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
        stroke
            .set_write_domain(WriteDomain::Detail)
            .expect("domain");
        stroke.begin().expect("open");
        for _ in 0..12 {
            stroke
                .erase(
                    MeshStamp {
                        center: [0.0, 0.0, 0.0],
                        radius: 1.0,
                        strength: 1.0,
                        geodesic: false,
                        ..Default::default()
                    },
                    None,
                )
                .expect("erase");
        }
        stroke.commit().expect("commit");
    }

    assert!(
        over_the_pass(&mut surface) < pass_before * 0.5,
        "the eraser took the pass toward zero: {} against {pass_before}",
        over_the_pass(&mut surface)
    );
    assert_eq!(
        surface.detail_checksum().expect("checksum"),
        form,
        "and touched neither the form under it nor any other pass, which is \
         what makes it an eraser for THIS pass rather than a flattening brush"
    );
    assert!((over_the_form(&mut surface) - form_before).abs() < 1e-6);
    assert!(surface.sculpt_layer_info(pass).expect("info").bytes > 0);
}

#[test]
fn restoring_takes_the_form_toward_the_pure_subdivision_and_leaves_the_pass_alone() {
    let mut surface = hierarchy(2, "restore");
    surface.set_sculpt_level(2).expect("bind");
    surface.add_sculpt_layer(Some("passe")).expect("pass");
    gesture(&mut surface, WriteDomain::Geometry, [0.0, 0.0, 0.0], 0.8);
    gesture(&mut surface, WriteDomain::Detail, [1.3, 0.0, 1.3], 0.8);

    let stack = surface
        .sculpt_layer_checksum()
        .expect("the stack's checksum");
    let form = surface.detail_checksum().expect("the form's");
    let over_the_form = height_near(&mut surface, 2, [0.0, 0.0, 0.0]);
    let over_the_pass = height_near(&mut surface, 2, [1.3, 0.0, 1.3]);

    {
        let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
        stroke
            .set_write_domain(WriteDomain::Detail)
            .expect("domain");
        stroke.begin().expect("open");
        for _ in 0..12 {
            stroke
                .restore(
                    MeshStamp {
                        center: [0.0, 0.0, 0.0],
                        radius: 1.0,
                        strength: 1.0,
                        geodesic: false,
                        ..Default::default()
                    },
                    None,
                )
                .expect("restore");
        }
        stroke.commit().expect("commit");
    }

    assert!(
        height_near(&mut surface, 2, [0.0, 0.0, 0.0]) < over_the_form,
        "the form came back toward the pure subdivision"
    );
    assert_ne!(
        surface.detail_checksum().expect("checksum"),
        form,
        "which is a change to the level's own detail"
    );
    assert_eq!(
        surface.sculpt_layer_checksum().expect("checksum"),
        stack,
        "with every pass left alone — restore is the form's eraser and erase \
         is the pass's, and neither of them is undo"
    );
    assert!((height_near(&mut surface, 2, [1.3, 0.0, 1.3]) - over_the_pass).abs() < 1e-6);
}

#[test]
fn the_three_smooths_are_three_operations_and_not_one_filter() {
    // The split is representational: the hierarchy stores the form and the
    // detail apart, so these are three different arrays rather than three
    // settings of one pass. A plain Laplacian over pores removes the pores,
    // which is rarely what was asked.
    let smoothed = |mode: SmoothMode, name: &str| {
        let mut surface = hierarchy(2, name);
        surface.set_sculpt_level(2).expect("bind");
        surface.add_sculpt_layer(Some("passe")).expect("pass");
        let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
        stroke
            .set_write_domain(WriteDomain::Detail)
            .expect("domain");
        stroke.begin().expect("open");
        layered_dab(&mut stroke, [0.0, 0.0, 0.0], 0.8);
        for _ in 0..4 {
            stroke
                .smooth(
                    mode,
                    MeshStamp {
                        center: [0.0, 0.0, 0.0],
                        radius: 1.0,
                        strength: 1.0,
                        geodesic: false,
                        ..Default::default()
                    },
                    None,
                )
                .expect("smooth");
        }
        stroke.commit().expect("commit");
        drop(stroke);
        let height = tallest(&mut surface, 2);
        (height, surface.sculpt_layer_checksum().expect("checksum"))
    };

    let (geometry, geometry_hash) = smoothed(SmoothMode::Geometry, "smooth-geometry");
    let (detail_only, detail_hash) = smoothed(SmoothMode::DetailOnly, "smooth-detail");
    let (preserve, preserve_hash) = smoothed(SmoothMode::PreserveDetail, "smooth-preserve");

    assert!(
        geometry < 0.8 && detail_only < 0.8,
        "the two that act on the deposit lower it: {geometry} and {detail_only}"
    );
    assert!(
        (preserve - 0.8).abs() < 1e-3,
        "and the one that smooths the FORM with the detail re-applied \
         unchanged leaves the deposit standing at its full height: {preserve}. \
         The cage under it is already flat, so there is no form to take out — \
         which is the mode an artist correcting anatomy under pores is asking \
         for, and the one that is impossible on a flat mesh"
    );
    assert_ne!(geometry_hash, detail_hash);
    assert_ne!(detail_hash, preserve_hash);
    assert_ne!(geometry_hash, preserve_hash);
}

#[test]
fn a_detail_stamp_says_when_the_level_cannot_carry_it() {
    // The library that implied the resolution says when it does not have it: a
    // fine map across a small square carries features a coarse level cannot
    // represent, and applying it anyway produces a surface that looks like the
    // map through a blur — which reads as a bug in the map, or in the brush,
    // or in the artist's file, and is none of those.
    let mut surface = hierarchy(2, "detail-stamp");
    surface.set_sculpt_level(2).expect("bind");
    surface.add_sculpt_layer(Some("passe")).expect("pass");
    let brush = MeshStamp {
        center: [0.0, 0.0, 0.0],
        radius: 1.0,
        strength: 0.8,
        geodesic: false,
        ..Default::default()
    };
    let ramp: Vec<f32> = (0..32 * 32).map(|i| (i % 32) as f32 / 31.0).collect();

    let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
    stroke
        .set_write_domain(WriteDomain::Detail)
        .expect("domain");
    stroke.begin().expect("open");

    let (fine, _) = stroke
        .stamp_detail(
            DetailStamp {
                mode: DetailStampMode::Height,
                image: &ramp,
                width: 32,
                height: 32,
                amplitude: 0.2,
                bias: 0.0,
                center: [0.0, 0.0, 0.0],
                direction: [0.0; 3],
                tangent: [0.0; 3],
                extent: 1.0,
            },
            brush,
            None,
        )
        .expect("a height map lands");
    assert!(
        fine.under_resolved && fine.oversampling > 1.0,
        "32 samples across a square a quarter of the level's mean edge is more \
         detail than the level can carry, and it is reported: {fine:?}"
    );

    let coarse: Vec<f32> = (0..4 * 4).map(|i| i as f32 / 15.0).collect();
    let (wide, report) = stroke
        .stamp_detail(
            DetailStamp {
                mode: DetailStampMode::Height,
                image: &coarse,
                width: 4,
                height: 4,
                amplitude: 0.2,
                bias: 0.0,
                center: [0.0, 0.0, 0.0],
                direction: [0.0; 3],
                tangent: [0.0; 3],
                extent: 2.0,
            },
            brush,
            None,
        )
        .expect("and so does a coarse one");
    assert!(
        !wide.under_resolved && wide.oversampling < 1.0,
        "while a map the level can carry says so: {wide:?}"
    );
    assert!(report.moved_vertices > 0, "and it reached the surface");

    let vector: Vec<f32> = (0..3 * 8 * 8).map(|i| (i % 5) as f32 / 5.0).collect();
    stroke
        .stamp_detail(
            DetailStamp {
                mode: DetailStampMode::Vector,
                image: &vector,
                width: 8,
                height: 8,
                amplitude: 0.1,
                bias: 0.0,
                center: [0.0, 0.0, 0.0],
                direction: [0.0; 3],
                tangent: [0.0; 3],
                extent: 2.0,
            },
            brush,
            None,
        )
        .expect("three planes, read in each vertex's own transported frame");

    let truncated = vec![0.0f32; 8 * 8];
    assert_eq!(
        stroke
            .stamp_detail(
                DetailStamp {
                    mode: DetailStampMode::Vector,
                    image: &truncated,
                    width: 8,
                    height: 8,
                    amplitude: 0.1,
                    bias: 0.0,
                    center: [0.0, 0.0, 0.0],
                    direction: [0.0; 3],
                    tangent: [0.0; 3],
                    extent: 2.0,
                },
                brush,
                None,
            )
            .expect_err("one plane where three were claimed")
            .kind(),
        claycore::ErrorKind::InvalidArgument,
        "and a malformed image is refused before the pointer is handed over, \
         because the engine reads channels * width * height floats out of it \
         whatever its own validation says about the dimensions"
    );
    stroke.commit().expect("commit");
}

// -- merge, bake, compact, hold ---------------------------------------------

#[test]
fn baking_a_pass_into_the_form_holds_parity_and_ends_the_slider() {
    let mut surface = hierarchy(1, "bake");
    let pass = pass_with_detail(&mut surface, "passe", 1, 8..24, 0.5);
    let before = positions(&mut surface, 1);
    let form = surface.detail_checksum().expect("the form's checksum");

    surface
        .bake_sculpt_layer_to_base(pass)
        .expect("fold it into the form");

    assert_eq!(
        positions(&mut surface, 1),
        before,
        "the same statement merge-down makes, with the base as the target: \
         visual parity"
    );
    assert_eq!(surface.sculpt_layer_count().expect("count"), 0);
    assert_ne!(
        surface.detail_checksum().expect("checksum"),
        form,
        "and what was dialable is now the form"
    );

    // The other half of the same claim, at the strength the naive arithmetic
    // cannot express: a silenced pass baked into the form adds nothing,
    // because nothing is what it was contributing.
    let mut silenced = hierarchy(1, "bake-zero");
    let pass = pass_with_detail(&mut silenced, "calada", 1, 8..24, 0.5);
    silenced
        .set_sculpt_layer_strength(pass, 0.0)
        .expect("dial it to nothing");
    let before = positions(&mut silenced, 1);
    let form = silenced.detail_checksum().expect("checksum");

    silenced
        .bake_sculpt_layer_to_base(pass)
        .expect("bake at strength zero");

    assert_eq!(positions(&mut silenced, 1), before, "parity at zero too");
    assert_eq!(
        silenced.detail_checksum().expect("checksum"),
        form,
        "and the form is untouched, because a pass contributing nothing bakes \
         nothing — which is the surface it leaves, not a concatenation of \
         coefficients"
    );
    assert_eq!(silenced.sculpt_layer_count().expect("count"), 0);
}

#[test]
fn compacting_releases_what_a_pass_that_undid_itself_left_behind() {
    let mut surface = hierarchy(1, "compact-stack");
    let pass = pass_with_detail(&mut surface, "desfeita", 1, 8..48, 0.4);
    let worked = surface.sculpt_layer_info(pass).expect("info");
    assert!(worked.bytes > 0 && worked.coverage_vertices > 0);

    for vertex in 8..48 {
        surface
            .set_sculpt_layer_detail(pass, 1, vertex, [0.0; 3])
            .expect("undo it by hand");
    }
    assert_eq!(
        surface.sculpt_layer_info(pass).expect("info").bytes,
        worked.bytes,
        "the blocks are still allocated: a stroke that undid itself leaves \
         storage behind, and nothing releases it on the engine's own behalf"
    );

    let surface_before = positions(&mut surface, 1);
    let checksum = surface.sculpt_layer_checksum().expect("checksum");
    surface.compact_sculpt_layers().expect("compact");

    let compacted = surface.sculpt_layer_info(pass).expect("info");
    assert!(
        compacted.bytes < worked.bytes,
        "compaction releases every all-zero coefficient block: {} against {}",
        compacted.bytes,
        worked.bytes
    );
    assert_eq!(compacted.coverage_vertices, 0);
    assert_eq!(
        surface.sculpt_layer_checksum().expect("checksum"),
        checksum,
        "and changes nothing anybody can see"
    );
    assert_eq!(positions(&mut surface, 1), surface_before);
    assert_eq!(
        surface.sculpt_layer_ids().expect("stack"),
        vec![pass],
        "there is deliberately no cap on the storage, so compaction is a lever \
         a host pulls and never something that silently stops recording"
    );
}

#[test]
fn a_mask_written_back_to_its_identity_is_released_by_a_compaction() {
    let mut surface = hierarchy(1, "mask-storage");
    let pass = pass_with_detail(&mut surface, "passe", 1, 8..24, 0.4);
    let plain = surface.sculpt_layer_info(pass).expect("info").bytes;

    for vertex in 0..48 {
        surface
            .set_sculpt_layer_mask(pass, 1, vertex, 0.5)
            .expect("paint the mask");
    }
    let masked = surface.sculpt_layer_info(pass).expect("info").bytes;
    assert!(masked > plain, "a mask costs storage once it exists");

    for vertex in 0..48 {
        surface
            .set_sculpt_layer_mask(pass, 1, vertex, 1.0)
            .expect("write the identity back");
    }
    assert_eq!(
        surface.sculpt_layer_mask(pass, 1, 12).expect("read"),
        1.0,
        "the weight is the identity again"
    );
    surface.compact_sculpt_layers().expect("compact");
    assert!(
        surface.sculpt_layer_info(pass).expect("info").bytes < masked,
        "and the all-identity block is released — the same lever that releases \
         an all-zero coefficient block, because an absent mask block means \
         full weight and an absent coefficient block means none"
    );
}

#[test]
fn a_held_composition_refuses_a_slider_and_permits_a_rename() {
    // What the stroke transaction takes and releases on its own, offered to a
    // host driving the plain sculptor stamp by stamp.
    let mut surface = hierarchy(1, "hold");
    let pass = surface.add_sculpt_layer(Some("passe")).expect("pass");

    surface
        .hold_sculpt_layer_composition(true)
        .expect("take the hold");
    assert_eq!(
        surface
            .set_sculpt_layer_strength(pass, 0.5)
            .expect_err("a slider is composition")
            .reason,
        MultiresError::SculptLayerStrokeOpen,
        "the same refusal a gesture produces, because it is the same hold"
    );
    surface
        .rename_sculpt_layer(pass, "renomeada")
        .expect("a rename moves no vertex");

    surface
        .hold_sculpt_layer_composition(false)
        .expect("give it back");
    surface
        .set_sculpt_layer_strength(pass, 0.5)
        .expect("and the slider is free again");
}

// -- what changed, and what it cost -----------------------------------------

#[test]
fn the_three_stack_revisions_say_which_of_three_things_happened() {
    let mut surface = hierarchy(1, "stack-revisions");
    let pass = surface.add_sculpt_layer(Some("passe")).expect("pass");
    let start = surface.sculpt_layer_revision().expect("revisions");

    surface.rename_sculpt_layer(pass, "outra").expect("rename");
    let renamed = surface.sculpt_layer_revision().expect("revisions");
    assert!(renamed.metadata > start.metadata);
    assert_eq!(
        (renamed.composition, renamed.content),
        (start.composition, start.content),
        "a rename invalidates nothing, so a host keyed on the other two does \
         not re-evaluate a model because a pass was renamed"
    );

    surface
        .set_sculpt_layer_strength(pass, 0.5)
        .expect("dial it");
    let dialled = surface.sculpt_layer_revision().expect("revisions");
    assert!(dialled.composition > renamed.composition);
    assert_eq!(
        dialled.content, renamed.content,
        "a slider recomposes and writes no coefficient"
    );

    surface
        .set_sculpt_layer_detail(pass, 1, 12, [0.0, 0.0, 0.4])
        .expect("write one");
    let written = surface.sculpt_layer_revision().expect("revisions");
    assert!(written.content > dialled.content);

    surface
        .set_active_sculpt_layer(SculptLayerId::BASE)
        .expect("route the next write to the form");
    let routed = surface.sculpt_layer_revision().expect("revisions");
    assert!(routed.metadata > written.metadata);
    assert_eq!(
        (routed.composition, routed.content),
        (written.composition, written.content),
        "changing which pass is active is metadata: it moves nothing"
    );
}

#[test]
fn the_stack_checksum_and_the_forms_checksum_are_two_questions() {
    let mut surface = hierarchy(2, "two-checksums");
    surface.set_sculpt_level(2).expect("bind");
    surface.add_sculpt_layer(Some("passe")).expect("pass");
    let form = surface.detail_checksum().expect("the form's");
    let stack = surface.sculpt_layer_checksum().expect("the stack's");

    gesture(&mut surface, WriteDomain::Detail, [0.0, 0.0, 0.0], 0.8);
    assert_ne!(surface.sculpt_layer_checksum().expect("checksum"), stack);
    assert_eq!(
        surface.detail_checksum().expect("checksum"),
        form,
        "a pass is not the form: a host asks 'did the form change' and 'did a \
         pass change' separately, and only one of them moved"
    );

    let stack = surface.sculpt_layer_checksum().expect("checksum");
    gesture(&mut surface, WriteDomain::Geometry, [0.6, 0.0, 0.6], 0.8);
    assert_ne!(surface.detail_checksum().expect("checksum"), form);
    assert_eq!(
        surface.sculpt_layer_checksum().expect("checksum"),
        stack,
        "and the other way round"
    );
}

#[test]
fn the_stats_show_a_slider_costing_the_passs_blocks_and_not_the_levels() {
    // There is no other way to see this from outside: a correct
    // implementation and a quadratic one produce the same surface.
    let mut surface = hierarchy(2, "stats");
    let narrow = pass_with_detail(&mut surface, "estreita", 2, 40..44, 0.4);

    surface.reset_sculpt_layer_stats().expect("reset");
    assert_eq!(
        surface.sculpt_layer_stats().expect("stats"),
        SculptLayerStats::default(),
        "a reset zeroes all three"
    );

    surface
        .set_sculpt_layer_strength(narrow, 0.5)
        .expect("dial it");
    let _ = positions(&mut surface, 2);
    let dialled = surface.sculpt_layer_stats().expect("stats");

    assert!(dialled.compositions > 0, "the slider recomposed something");
    let (level_blocks, _) = surface.level_counts(0).expect("the cage's counts");
    assert!(
        dialled.blocks_recomposed < level_blocks,
        "but it recomposed the pass's allocated blocks and not the level's \
         {level_blocks}: {dialled:?}"
    );
    assert!(
        dialled.layer_blocks_visited <= dialled.blocks_recomposed,
        "and one pass over a handful of vertices is not summed over unrelated \
         geometry: {dialled:?}"
    );
}

#[test]
fn a_stack_costs_its_coverage_and_is_counted_apart_from_the_form() {
    let mut surface = hierarchy(2, "stack-memory");
    let empty = surface.memory().expect("memory");
    assert_eq!(
        empty.sculpt_layers, 0,
        "an empty stack costs nothing beside the form"
    );

    let narrow = pass_with_detail(&mut surface, "estreita", 2, 40..44, 0.4);
    let with_one = surface.memory().expect("memory");
    assert!(
        with_one.sculpt_layers > 0,
        "a pass with coefficients on it does"
    );
    assert!(
        with_one.sculpt_layers <= with_one.authoritative,
        "and it is counted in the authoritative figure, because it is the \
         user's work and never rebuildable: {with_one:?}"
    );

    let broad = pass_with_detail(&mut surface, "larga", 2, 40..400, 0.4);
    let with_two = surface.memory().expect("memory");
    assert!(
        with_two.sculpt_layers > with_one.sculpt_layers,
        "a pass costs its coverage, which is what makes a hundred passes over \
         one cheek affordable"
    );
    assert!(
        surface
            .sculpt_layer_info(broad)
            .expect("info")
            .coverage_vertices
            >= surface
                .sculpt_layer_info(narrow)
                .expect("info")
                .coverage_vertices
    );

    surface.remove_sculpt_layer(broad).expect("discard");
    surface.remove_sculpt_layer(narrow).expect("discard");
    surface.compact_sculpt_layers().expect("compact");
    assert_eq!(
        surface.memory().expect("memory").sculpt_layers,
        0,
        "and it all comes back"
    );
}

#[test]
fn a_stacks_vocabulary_is_the_engines_own() {
    for reason in [
        MultiresError::NoSuchSculptLayer,
        MultiresError::SculptLayerLocked,
        MultiresError::SculptLayerStrokeOpen,
    ] {
        assert!(!reason.text().is_empty(), "{reason:?}");
        assert_ne!(
            reason.text(),
            "unknown",
            "the three sentences a host UI has to be able to say are three \
             sentences the engine already writes: {reason:?}"
        );
    }
    assert_eq!(SculptLayerId::from_raw(7).get(), 7);
    assert_eq!(SculptLayerId::BASE.get(), 0);
    assert!(!SculptLayerId::from_raw(7).is_base());
}

#[test]
fn the_automatic_domain_takes_the_active_pass_or_the_form_under_it() {
    // The default, and the only one of the three that reads the stack: it is
    // what a host that has not decided means, and it decides once.
    let mut surface = hierarchy(2, "automatic");
    surface.set_sculpt_level(2).expect("bind");

    {
        let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
        stroke
            .set_write_domain(WriteDomain::Automatic)
            .expect("the default, said out loud");
        stroke.begin().expect("open");
        assert_eq!(
            stroke.target_layer().expect("target"),
            SculptLayerId::BASE,
            "with an empty stack there is no pass, so it is the form"
        );
        stroke.commit().expect("commit");
    }

    let pass = surface.add_sculpt_layer(Some("passe")).expect("pass");
    let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
    stroke
        .set_write_domain(WriteDomain::Automatic)
        .expect("domain");
    stroke.begin().expect("open");
    assert_eq!(
        stroke.target_layer().expect("target"),
        pass,
        "and with one, it is the active pass — the same call, a different \
         answer, resolved once at begin"
    );
    stroke.commit().expect("commit");
}

#[test]
fn a_freeze_gates_a_layered_stamp_as_it_gates_every_other_one() {
    // The same sixteen verbs, the same falloffs, the same mask and the same
    // automasking, because it is the same code.
    let mut mask = Mask::new(0.05).expect("mask");
    let samples: Vec<StrokeSample> = (0..9)
        .map(|i| {
            let t = i as f32 / 8.0;
            StrokeSample {
                position: [0.0, 0.0, -2.0 + 4.0 * t],
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

    let into_a_pass = |name: &str, freeze: Option<&Mask>| {
        let mut surface = hierarchy(2, name);
        surface.set_sculpt_level(2).expect("bind");
        surface.add_sculpt_layer(Some("passe")).expect("pass");
        let mut stroke = surface.sculpt_layer_stroke().expect("transaction");
        stroke
            .set_write_domain(WriteDomain::Detail)
            .expect("domain");
        stroke.begin().expect("open");
        stroke
            .stamp(
                MeshStamp {
                    verb: MeshBrush::Draw,
                    center: [0.0, 0.0, 0.0],
                    radius: 0.9,
                    strength: 0.8,
                    geodesic: false,
                    ..Default::default()
                },
                freeze.map(|m| &**m),
            )
            .expect("stamp");
        stroke.commit().expect("commit");
        drop(stroke);
        tallest(&mut surface, 2)
    };

    let ungated = into_a_pass("gate-open", None);
    let gated = into_a_pass("gate-frozen", Some(&mask));
    assert!(
        gated < ungated * 0.5,
        "the freeze held the pass down: {gated} against {ungated} with no mask \
         at all"
    );
}
