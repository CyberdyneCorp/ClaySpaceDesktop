//! Where the last pick landed, so the next stamp does not have to look.
//!
//! A mesh brush has to know which weld class it is standing on before it can
//! walk the surface, and it has two ways to find out. It can be told — the
//! pick that placed the cursor already hit a triangle and knows the answer —
//! or it can search, which the engine states is "a linear scan over the mesh
//! and the wrong thing to do per stamp on a large one". This module is the
//! first of those: what a pick learned, kept until the stroke that follows it
//! can use it.
//!
//! **A class alone is not usable, and this is the whole reason the type exists
//! rather than a bare `u32`.** A weld class is an INDEX into a numbering the
//! sculptor built, and this application retires sculptors constantly: the LRU
//! evicts the coldest of four, a removed layer gives one up, an undo's
//! reconciliation prunes them, and a re-mesh drops one deliberately before
//! replacing the triangles under it. Every one of those hands back a new
//! numbering, and an index taken from the old one is comfortably in bounds
//! against the new — so a bounds check sees nothing. What it costs is not a
//! slightly misplaced dab. `geodesic_region` returns an EMPTY region when its
//! seed lies farther than the radius from the stamp's centre, so a stale seed
//! loses the stamp whole, and "nothing moved" reads exactly like a fully
//! masked stroke.
//!
//! So the class travels with the token of the numbering it was picked in —
//! `claycore::MeshSeed` — and a token from a retired numbering is refused by
//! the engine and the stamp falls back to the scan it would have done anyway.
//! One stamp slower, and correct. `ClayDocument::stale_seeds_rejected` is
//! where a reader can watch it happen.
//!
//! **The second half is reach, and it is not the engine's problem.** A seed is
//! only useful to a stamp standing where the pick stood: hand a valid seed to
//! a stamp centred half a form away and the walk starts outside its own radius
//! and comes back empty — the same lost dab, from a seed nothing is wrong
//! with. So every question this module answers is "is this stamp near enough
//! to what was picked", and the answer is `None` wherever it cannot be shown
//! to be, which puts the stamp back on the scan.

use clayspace_model::LayerKey;

use claycore::{MeshSeed, StrokePreset};

/// What a pick learned, and where it learned it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PickedSeed {
    /// The layer whose sculptor answered the pick.
    pub(crate) layer: LayerKey,
    /// Where the ray met the surface, in the MESH's own space.
    ///
    /// The same space a stamp's centre is in, so the two are comparable
    /// without carrying a transform around: `pick_active_mesh` already
    /// carries the ray in and the answer out, and this is taken from the
    /// inward half.
    pub(crate) at: [f32; 3],
    /// The class, and the numbering it was picked in.
    pub(crate) seed: MeshSeed,
}

/// How much of a stamp's reach is left for the step from the picked point to
/// the vertex it named.
///
/// A ray meets a triangle's *interior*; the class it reports is at one of that
/// triangle's corners. So the picked point and the class are up to an edge
/// apart, and the walk measures from the class. Half the reach is kept back
/// to cover that step, which is enough wherever the brush is wider than the
/// triangles under it — and where it is not, the engine's own fallback of
/// "nearest class to the centre" reaches nothing either, so the seed is not
/// what made the dab empty.
const SPARE_FOR_THE_STEP: f32 = 0.5;

impl PickedSeed {
    /// The seed for one stamp, or `None` where it cannot be shown to help.
    ///
    /// The radius is the stamp's own — the direct stamp path honours the
    /// descriptor's, which a resolved stroke does not.
    pub(crate) fn for_stamp(
        &self,
        layer: LayerKey,
        centre: [f32; 3],
        radius: f32,
    ) -> Option<MeshSeed> {
        self.within(layer, &[centre], radius * SPARE_FOR_THE_STEP)
    }

    /// The seed for a resolved stroke over `path`, or `None`.
    ///
    /// A resolved stroke ignores the descriptor's radius and takes each
    /// stamp's from the preset, and it moves the centre along the path — so
    /// what has to be shown here is that *every* stamp the call can produce is
    /// still within reach of one seed. Two things decide that, and both are
    /// read from the preset rather than assumed:
    ///
    /// - the smallest radius a stamp can take. Pressure, taper and size jitter
    ///   all move it, none of the three is used here today, and rather than
    ///   deriving a bound from a response curve this refuses outright when any
    ///   of them is armed. A refusal is the scan, which is what every stamp did
    ///   before the seed crossed this boundary; a bound guessed wrong is a lost
    ///   dab.
    /// - how far jitter can carry a stamp off the path, which is stated as a
    ///   fraction of the radius and so comes straight off the reach.
    pub(crate) fn for_stroke(
        &self,
        layer: LayerKey,
        path: &[[f32; 5]],
        preset: &StrokePreset,
    ) -> Option<MeshSeed> {
        let reach = reach_of(preset)?;
        let centres: Vec<[f32; 3]> = path.iter().map(|s| [s[0], s[1], s[2]]).collect();
        self.within(layer, &centres, reach * SPARE_FOR_THE_STEP)
    }

    /// Whether this seed stands close enough to every one of `centres`.
    fn within(&self, layer: LayerKey, centres: &[[f32; 3]], reach: f32) -> Option<MeshSeed> {
        if self.layer != layer || reach <= 0.0 || centres.is_empty() {
            return None;
        }
        centres
            .iter()
            .all(|centre| distance(self.at, *centre) <= reach)
            .then_some(self.seed)
    }
}

/// The radius every stamp a preset resolves is guaranteed to reach, or `None`
/// where that cannot be stated.
fn reach_of(preset: &StrokePreset) -> Option<f32> {
    let shrinks = preset.pressure_size != 0.0
        || preset.taper_start != 0.0
        || preset.taper_end != 0.0
        || preset.jitter_size != 0.0;
    if shrinks {
        return None;
    }
    let off_path = preset.jitter_position.clamp(0.0, 1.0);
    Some(preset.radius * (1.0 - off_path))
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3)
        .map(|axis| (a[axis] - b[axis]).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picked() -> PickedSeed {
        PickedSeed {
            layer: LayerKey(1),
            at: [0.0, 0.0, 1.0],
            seed: MeshSeed {
                class: 7,
                revision: 42,
            },
        }
    }

    fn preset(radius: f32) -> StrokePreset {
        StrokePreset {
            radius,
            jitter_position: 0.0,
            jitter_size: 0.0,
            pressure_size: 0.0,
            taper_start: 0.0,
            taper_end: 0.0,
            ..Default::default()
        }
    }

    fn path(points: &[[f32; 3]]) -> Vec<[f32; 5]> {
        points
            .iter()
            .map(|p| [p[0], p[1], p[2], 1.0, 0.0])
            .collect()
    }

    #[test]
    fn a_stamp_where_the_pick_landed_takes_the_seed() {
        assert_eq!(
            picked().for_stamp(LayerKey(1), [0.0, 0.0, 1.0], 0.2),
            Some(picked().seed)
        );
    }

    /// The failure this gate exists for: a valid seed handed to a stamp
    /// standing outside its own radius starts the walk out of reach, and the
    /// region comes back empty — a dab lost to a seed with nothing wrong with
    /// it.
    #[test]
    fn a_stamp_out_of_the_seeds_reach_scans_instead() {
        assert_eq!(picked().for_stamp(LayerKey(1), [0.0, 0.0, -1.0], 0.2), None);
    }

    /// A class belongs to the numbering one sculptor built, and every layer
    /// has its own. Carrying one across is a wrong seed that no token can
    /// catch, because the token is current on the layer it was issued for.
    #[test]
    fn a_seed_does_not_cross_to_another_layer() {
        assert_eq!(picked().for_stamp(LayerKey(2), [0.0, 0.0, 1.0], 0.2), None);
    }

    #[test]
    fn a_stroke_that_stays_under_the_pick_takes_the_seed() {
        let path = path(&[[0.0, 0.0, 1.0], [0.02, 0.0, 1.0]]);
        assert_eq!(
            picked().for_stroke(LayerKey(1), &path, &preset(0.5)),
            Some(picked().seed)
        );
    }

    /// One sample beyond reach is enough: the stroke resolver walks the centre
    /// along the path with the same seed for every stamp, so the seed has to
    /// serve the whole path or none of it.
    #[test]
    fn a_stroke_that_travels_past_the_reach_scans_instead() {
        let path = path(&[[0.0, 0.0, 1.0], [0.9, 0.0, 1.0]]);
        assert_eq!(picked().for_stroke(LayerKey(1), &path, &preset(0.5)), None);
    }

    /// Jitter moves a stamp off the path by a fraction of the radius, so it
    /// comes off the reach rather than being hoped away.
    #[test]
    fn jitter_comes_off_the_reach() {
        let at = [0.4, 0.0, 1.0];
        let mut jittered = preset(1.0);
        jittered.jitter_position = 0.5;
        assert_eq!(
            picked().for_stroke(LayerKey(1), &path(&[at]), &preset(1.0)),
            Some(picked().seed),
            "a unit radius, half of it kept back for the step to the vertex,              still reaches four tenths"
        );
        assert_eq!(
            picked().for_stroke(LayerKey(1), &path(&[at]), &jittered),
            None,
            "with half the radius spent on jitter it does not"
        );
    }

    /// Pressure and taper both shrink a stamp's radius below the preset's, and
    /// a reach derived from the larger one would be a lost dab rather than a
    /// slow one. Neither is used by this application today; the refusal is
    /// what keeps that from becoming a silent assumption.
    #[test]
    fn anything_that_can_shrink_a_stamp_gives_up_the_seed() {
        let here = path(&[[0.0, 0.0, 1.0]]);
        for arm in [
            |p: &mut StrokePreset| p.pressure_size = 1.0,
            |p: &mut StrokePreset| p.taper_start = 0.2,
            |p: &mut StrokePreset| p.taper_end = 0.2,
            |p: &mut StrokePreset| p.jitter_size = 0.2,
        ] {
            let mut preset = preset(0.5);
            arm(&mut preset);
            assert_eq!(
                picked().for_stroke(LayerKey(1), &here, &preset),
                None,
                "a preset that can shrink a stamp still handed its seed over"
            );
        }
    }
}

/// The defect the token exists to end, driven through this application's own
/// operations rather than argued about.
///
/// Inside the crate rather than in `tests/`, for the reason the dropped-gesture
/// guard in `document.rs` is: the "before" half cannot be reached from outside.
/// A stroke takes whatever seed the pick recorded, and there is no public way
/// to hand it a class with its token struck off — which is exactly the state
/// every host was in before ABI minor 75, and exactly what has to be shown
/// losing the dab for the fix to mean anything.
#[cfg(test)]
mod the_silent_empty_dab {
    use super::*;

    use clayspace_model::{
        BrushSettings, Direction, GestureSample, RemeshSettings, SceneModel, SculptModel, ToolKind,
    };

    use crate::{BackendPolicy, ClayDocument};

    /// What one run of the sequence came to.
    #[derive(Debug)]
    struct Run {
        /// Whether the dab after the rebuild moved anything at all.
        moved: bool,
        /// Seeds the engine refused for naming a numbering that had gone.
        rejected: usize,
    }

    /// Pick, rebuild the mesh under the pick, dab where the pick landed.
    ///
    /// The rebuild is `remesh_layer`, which is this application's Remalhar and
    /// is the sharpest of the four things it does that retire a numbering: it
    /// gives the sculptor up on purpose, replaces every vertex and index, and
    /// arms a new sculptor before it returns — so the pointer is over a form
    /// whose classes are numbered by nobody the pick ever spoke to. The other
    /// three (the LRU's eviction, a removed layer, an undo's reconciliation)
    /// retire a numbering the same way and are simply harder to make land on
    /// one line.
    ///
    /// `keep_the_token` is the whole experiment. With it, the seed says which
    /// numbering it came from and the engine can tell that it has gone. Without
    /// it — a bare class, which is all a host could send before minor 75 — the
    /// class is in bounds against the new numbering, passes the only check
    /// there is, names a vertex somewhere else entirely, and the walk starts
    /// outside its own radius.
    fn run(keep_the_token: bool) -> Run {
        let policy = BackendPolicy::discover(None).expect("discover backends");
        let mut document = ClayDocument::new(policy)
            .and_then(ClayDocument::with_starting_form)
            .expect("a document with a starting form");
        document
            .convert_layer(Direction::SdfToMesh, 0.05, 0)
            .expect("cross the starting form into a mesh");

        // The crossing arms the sculptor itself — it has to, or the pick that
        // would place the first stroke answers nothing and the press orbits
        // the camera instead.
        let key = document.scene().active.expect("the crossing made a layer");

        // The pick the interface makes on every frame the pointer is over the
        // form: a ray down the +Z axis, meeting the near face.
        let at = document
            .pick([0.0, 0.0, 5.0], [0.0, 0.0, -1.0])
            .expect("the ray missed the form");
        let picked = document
            .picked_seed
            .get()
            .expect("the pick recorded no seed at all");

        document
            .remesh_layer(key, RemeshSettings::default())
            .expect("the rebuild was refused");

        assert!(
            document.picked_seed.get() == Some(picked),
            "the rebuild cleared the seed, so this no longer arranges a stale one"
        );
        if !keep_the_token {
            document.picked_seed.set(Some(PickedSeed {
                seed: MeshSeed {
                    revision: 0,
                    ..picked.seed
                },
                ..picked
            }));
        }

        let before = document.stale_seeds_rejected();
        assert_eq!(before, 0, "nothing had been rejected before the dab");

        let travel = 0.01;
        let moved = document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings::default(),
                &[
                    GestureSample {
                        position: at,
                        pressure: 1.0,
                        time: 0.0,
                    },
                    GestureSample {
                        position: [at[0] + travel, at[1], at[2]],
                        pressure: 1.0,
                        time: 1.0,
                    },
                ],
                [false; 3],
            )
            .expect("the dab was refused");

        Run {
            moved: moved.changed,
            rejected: document.stale_seeds_rejected(),
        }
    }

    /// With the token, the engine catches the retired numbering, spends one
    /// scan, and the dab lands.
    #[test]
    fn a_seed_carrying_its_token_costs_a_scan_and_keeps_the_dab() {
        let run = run(true);
        assert_eq!(
            run.rejected, 1,
            "the seed was taken at face value. The counter is the only thing \
             that can tell a refused seed from one that happened to be \
             harmless, which is why it is asserted rather than the dab alone"
        );
        assert!(
            run.moved,
            "the dab was lost even with the token, so the rejection did not \
             fall back to the scan it is supposed to"
        );
    }

    /// And without it, the same sequence is a stroke that does nothing — no
    /// error, no refusal, no rejection to count. This is the failure, and the
    /// reason it is worth a test is that it is indistinguishable from a fully
    /// masked stroke from every side except this one.
    #[test]
    fn a_seed_that_lost_its_token_loses_the_dab_and_says_nothing() {
        let run = run(false);
        assert_eq!(
            run.rejected, 0,
            "a seed claiming no numbering cannot be stale — there is nothing \
             to compare it against, which is the whole problem"
        );
        assert!(
            !run.moved,
            "the stale class happened to land inside the brush on this form, \
             so the fixture no longer demonstrates the empty region. Move the \
             pick or shrink the brush until it does — a test that passes \
             because the defect was lucky is worse than none"
        );
    }
}
