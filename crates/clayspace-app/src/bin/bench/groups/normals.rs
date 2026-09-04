//! What deferring a stroke's normal recomputation buys.
//!
//! ClayCore v0.78.0 puts this seam forward as the thing that makes a
//! per-pointer-event stamp affordable: a resolved stroke's dabs overlap, so
//! recomputing each dab's normals as it lands does the same vertices over and
//! over, while deferring sorts the touched classes, makes them unique, and
//! recomputes each one once at the end of the call. The final surface is
//! identical either way. Only the work is different.
//!
//! # Why this group reaches past the application
//!
//! Every other group in this harness drives `ClayDocument`, and this one drives
//! `claycore::MeshSculptor` directly. That is deliberate and it is the only way
//! the figure exists at all: **this application has no switch to turn off.**
//! `stroke_mesh` passes `defer_normals = true` to every resolved stroke and
//! arms the sculptor's member flag across a Grab, so an application-level
//! measurement can only ever produce the deferred arm. A pair needs both, and
//! the flag is an engine argument.
//!
//! The two arms therefore differ in exactly one boolean, on the same sculptor,
//! over the same mesh, along the same path, alternating so that anything
//! drifting on the machine drifts through both. Everything else is held
//! identical, including the record the stamps are noted into — see
//! [`Arm::run`].
//!
//! # What is deliberately outside the clock
//!
//! No viewport. The surface is not uploaded, meshed or drawn, and that is not
//! an oversight: this application re-reads and re-uploads a mesh layer whole
//! on every segment, which is ~17 ms on the mesh reference — `brush.mesh.
//! borrar` is 16.9965 ms for a verb that moves no vertex at all, which is that
//! floor with nothing on top of it. A figure taken through the screen would be
//! that floor plus a difference of well under a millisecond, and would be
//! measuring the upload. `brush.mesh.*` already prices the whole route; this
//! prices the seam.
//!
//! The record is reverted between samples, outside the clock, so every sample
//! starts from the same surface rather than from an increasingly deformed one.
//!
//! # What it measured, the first time it was run
//!
//! **Deferring cost more than it saved, by about fourteen per cent, in every
//! regime tried.** Recorded here because a figure that surprises its author is
//! the one most likely to be quietly re-tuned until it agrees with the release
//! notes, and because the next reader deserves the numbers rather than the
//! conclusion.
//!
//! On a 66,049-vertex sheet, one resolved stroke, the deferral was slower at
//! every stamp spacing from nine stamps per call to sixty-three — and the
//! ratio did not move: 1.14, 1.15, 1.14, 1.14. The de-duplication itself is
//! real and large: sixty-three stamps touching some two thousand classes each
//! reduce to 8,522 unique ones, so the flush recomputes a fifteenth of what
//! the per-stamp path recomputes. What did move with the stamp count was the
//! *difference* — about 0.07 ms per stamp, steadily — which is the signature
//! of a cost paid per stamp rather than once at the flush. `flush_normals`
//! sorts and uniques the accumulated list, and the list accumulates one entry
//! per class per *stamp*, not per unique class; at sixty-three stamps that is
//! a sort over a hundred and twenty thousand entries to arrive at eight
//! thousand. On this mesh the recompute it saves is cheaper than the
//! bookkeeping to defer it.
//!
//! Two things that does **not** license saying. It is one machine and one
//! shape of mesh, and a mesh with a costlier normal — more valence, more
//! attributes — moves the balance the other way. And it says nothing about
//! whether this application should stop deferring: `stroke_mesh` defers
//! because the alternative on the one verb that spans calls, Grab, is a
//! recompute per pointer event, and that is a different comparison from this
//! one. What it does license is not claiming the seam as a win in the upgrade
//! notes without a figure, which is what this figure is for.
//!
//! # The one thing this figure cannot answer
//!
//! Whether the application benefits. It sends roughly one dab per
//! `apply_stroke` call — `stamps_between_segments` is 1.0 on a mesh — and a
//! window holding one dab has nothing to de-duplicate. What this measures is
//! the seam's value *per resolved stroke*, over a call carrying a full path,
//! which is the shape the release notes describe and the shape the application
//! would move to if a segment ever carried more than a dab's travel.

use std::time::Instant;

use clayspace_engine::claycore::{
    Automask, Mesh, MeshBrush, MeshDeltas, MeshFalloff, MeshSculptor, MeshStamp, StrokePreset,
};

use crate::figures::{ms, Figure, Record};
use crate::run::Run;
use crate::skip::Skip;

/// Quads along each side of the sheet the stroke is drawn on.
///
/// 257 by 257 vertices, 131,072 triangles once the reader has triangulated the
/// quads — the same order as `mesh-reference`, so the figure is about a mesh a
/// sculptor would actually be working on.
const DIVISIONS: usize = 256;

/// How far the sheet reaches from its centre.
const HALF: f32 = 2.0;

/// The brush's reach, in the sheet's own units.
///
/// Chosen against the vertex spacing rather than picked: at `HALF` 2.0 over
/// 256 divisions a vertex is 0.0156 apart, so a disc of this radius covers
/// some three hundred of them and consecutive stamps along the path overlap
/// heavily. Overlap is the entire quantity the de-duplication is worth.
const RADIUS: f32 = 0.15;

/// How far the stroke travels. Long enough that one call carries a couple of
/// dozen stamps at the preset's own spacing.
const TRAVEL: f32 = 2.0;

/// How many positions the path is delivered as.
const PATH: usize = 24;

/// The weld the application builds every mesh sculptor at.
const WELD: f32 = 1e-4;

pub fn measure(run: &mut Run) {
    if !run.wants_group("normals") {
        return;
    }
    match pair() {
        Ok((deferred, direct)) => {
            let middle = |samples: &[f64]| {
                let mut sorted = samples.to_vec();
                sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
                crate::figures::quantile(&sorted, 0.5)
            };
            let ratio = middle(&deferred) / middle(&direct).max(f64::MIN_POSITIVE);
            run.timings("normals.deferred", Record::Repeatable, deferred);
            run.timings("normals.direct", Record::Repeatable, direct);
            // The headline, and the only figure here that survives a change of
            // machine: what a resolved stroke costs with the deferral against
            // what it costs without. Below one, deferring wins. No budget,
            // because nothing has promised a number — the release notes claim
            // a direction and this is the instrument for it.
            run.insert("normals.deferred_ratio", Figure::ratio(ratio, None, 1.5));
        }
        Err(why) => run.skip("normals", why),
    }
}

/// Which way the flag is set for one sample.
#[derive(Clone, Copy)]
enum Arm {
    /// One recompute at the end of the call, over the union of what the dabs
    /// touched.
    Deferred,
    /// One recompute per stamp, over what that stamp touched.
    Direct,
}

impl Arm {
    fn defers(self) -> bool {
        matches!(self, Self::Deferred)
    }

    /// One resolved stroke, timed, and then taken back.
    ///
    /// The revert is what makes the samples comparable: without it the tenth
    /// stroke lands on a sheet nine strokes have already raised, and the two
    /// arms would be measuring different surfaces by the end.
    ///
    /// The record is the one the stamps were noted into, which is not a detail
    /// to be tidied. The engine captures a vertex's normal the first time it
    /// sees it, so a flush into a *fresh* record would capture the already
    /// moved normals as the "before" and the revert would put the geometry
    /// back while leaving the shading where the stroke wrote it.
    fn run(self, sculptor: &mut MeshSculptor, path: &[[f32; 5]]) -> Result<f64, Skip> {
        let mut deltas = MeshDeltas::new().map_err(|_| Skip::EditRefused)?;
        let started = Instant::now();
        sculptor
            .apply_stroke(
                path,
                &preset(),
                stamp(),
                None,
                self.defers(),
                Some(&mut deltas),
            )
            .map_err(|_| Skip::EditRefused)?;
        let took = ms(started.elapsed());
        deltas.revert(sculptor).map_err(|_| Skip::EditRefused)?;
        Ok(took)
    }
}

/// The two arms, alternated over one sculptor.
fn pair() -> Result<(Vec<f64>, Vec<f64>), Skip> {
    let path = write_sheet().map_err(|_| Skip::SceneWouldNotBuild)?;
    let loaded = Mesh::load(&path);
    let _ = std::fs::remove_file(&path);
    // The sculptor holds a pointer into the mesh rather than a borrow of it,
    // so the mesh stays owned here for as long as the sculptor is used.
    let mut mesh = loaded.map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut sculptor = MeshSculptor::new(&mut mesh, WELD).map_err(|_| Skip::SceneWouldNotBuild)?;

    let stroke = stroke_path();
    // One of each before the clock. The first stroke over a mesh pays for
    // whatever the adjacency and the region walk build on their first use, and
    // it would land on whichever arm happened to go first.
    for arm in [Arm::Deferred, Arm::Direct] {
        arm.run(&mut sculptor, &stroke)?;
    }

    let mut deferred = Vec::new();
    let mut direct = Vec::new();
    for _ in 0..Record::Repeatable.samples() {
        // Alternated rather than run in two blocks: a machine that warms or
        // throttles over the length of the group would otherwise hand the
        // whole of that drift to one arm and call it a result.
        deferred.push(Arm::Deferred.run(&mut sculptor, &stroke)?);
        direct.push(Arm::Direct.run(&mut sculptor, &stroke)?);
    }
    Ok((deferred, direct))
}

/// The stroke both arms follow: a straight sweep across the middle of the
/// sheet, at full pressure.
fn stroke_path() -> Vec<[f32; 5]> {
    (0..PATH)
        .map(|n| {
            let t = n as f32 / (PATH - 1) as f32;
            let x = -TRAVEL / 2.0 + TRAVEL * t;
            [x, 0.0, 0.0, 1.0, t]
        })
        .collect()
}

fn preset() -> StrokePreset {
    StrokePreset {
        radius: RADIUS,
        // Everything else is the engine's own default, including the spacing
        // that decides how many stamps this path resolves into. A spacing
        // chosen here would be choosing the answer.
        ..StrokePreset::default()
    }
}

/// A plain Draw, named field by field for the reason `stroke_mesh` names its
/// own: a field added upstream should fail this call rather than be filled in
/// with a default nobody chose.
fn stamp() -> MeshStamp<'static> {
    MeshStamp {
        verb: MeshBrush::Draw,
        center: [0.0, 0.0, 0.0],
        radius: RADIUS,
        strength: 0.5,
        falloff: MeshFalloff::Smooth,
        direction: [0.0; 3],
        geodesic: true,
        colour: [1.0; 3],
        smooth_iterations: None,
        stamp_azimuth: 0.0,
        // No seed: the sweep is resolved by the engine's own stroke engine,
        // which walks from wherever each stamp lands. Handing it one picked
        // elsewhere is what `crates/clayspace-engine/src/seed.rs` is for and
        // is a different measurement.
        seed: None,
        automask: Automask::default(),
        alpha: None,
    }
}

/// Writes the sheet as an `.obj`, which is the only route a mesh has into this
/// crate — `claycore::Mesh` has no constructor from arrays.
fn write_sheet() -> std::io::Result<std::path::PathBuf> {
    let path =
        std::env::temp_dir().join(format!("clayspace-bench-sheet-{}.obj", std::process::id()));
    let mut text = String::new();
    let step = 2.0 * HALF / DIVISIONS as f32;
    for z in 0..=DIVISIONS {
        for x in 0..=DIVISIONS {
            text.push_str(&format!(
                "v {} 0 {}\n",
                -HALF + step * x as f32,
                -HALF + step * z as f32
            ));
        }
    }
    let stride = DIVISIONS + 1;
    for z in 0..DIVISIONS {
        for x in 0..DIVISIONS {
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
    std::fs::write(&path, text)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole figure is worth exactly the overlap between one call's dabs,
    /// so the path has to be long enough to hold several and the brush wide
    /// enough that they reach the same vertices.
    #[test]
    fn the_stroke_carries_more_than_one_dabs_worth_of_travel() {
        let stroke = stroke_path();
        assert_eq!(stroke.len(), PATH);
        let travelled = stroke[PATH - 1][0] - stroke[0][0];
        assert!(
            travelled > RADIUS * 4.0,
            "a window holding one dab has nothing to de-duplicate: {travelled}"
        );
    }

    /// The two arms differ in the flag and in nothing else.
    #[test]
    fn the_arms_are_one_boolean_apart() {
        assert!(Arm::Deferred.defers());
        assert!(!Arm::Direct.defers());
    }
}
