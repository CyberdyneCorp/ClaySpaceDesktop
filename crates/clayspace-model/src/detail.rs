//! Which level of detail the viewport should be drawing.
//!
//! Deciding *when* a coarser level is worth it is host policy and lives here,
//! testable without a GPU or an engine. Drawing one is the engine adapter's:
//! it keeps the mips built and hands over the coarse keys that have one, and
//! the composition root meshes them at level 1. The join was blocked until
//! ClayCore 0.30.0 gave the meshing call a level (#93).
//!
//! # Not a subdivision level
//!
//! This module and [`crate::multires`] share the English word "detail" and
//! share nothing else, and the wrong one is one letter away in an import list.
//! What is here is a *rendering economy*: how far away a form has to be before
//! the viewport draws it off a coarser mip, with the sculpt identical either
//! way. A hierarchy's levels are the sculpt — level 4 holds displacements level
//! 1 does not — which is why one of these is a policy and the other is state.
//!
//! A hierarchy has a level-of-detail question too, and it is this one: its
//! display level is what a policy like this would move. Nothing here does that
//! yet, and the reason is worth writing down rather than discovering — the
//! policy below is stated in model extents and brick counts, and a hierarchy
//! has neither.
//!
//! The rule that matters is hysteresis. A single threshold flickers: a camera
//! resting exactly on it swaps the whole surface between levels every frame
//! the pointer twitches, which is far worse than always being coarse. So the
//! distance at which detail is *dropped* is further out than the one at which
//! it is restored, and the gap between them is the whole design.

use crate::sculpt::Detail;

/// When to drop detail, and when to bring it back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetailPolicy {
    /// Beyond this many model-extents away, drop to the mip.
    pub drop_beyond: f32,
    /// Within this many, restore. Must be the smaller of the two.
    pub restore_within: f32,
    /// Never drop detail below this many surface bricks, however far away.
    ///
    /// A small model is cheap at full resolution however distant it is, and
    /// coarsening one buys nothing while making it visibly worse.
    pub floor: usize,
}

impl Default for DetailPolicy {
    fn default() -> Self {
        Self {
            // Measured against the reference scene rather than chosen: at
            // three extents the model is small enough on screen that a mip's
            // doubled spacing is under a pixel, and at two it is not.
            drop_beyond: 3.0,
            restore_within: 2.5,
            // Below this the whole surface meshes inside a frame anyway, so
            // there is nothing to buy.
            floor: 2048,
        }
    }
}

impl DetailPolicy {
    /// The level to draw, given where the camera is and what is on screen.
    ///
    /// `current` is what is being drawn now, and it is what makes this
    /// hysteretic rather than a threshold: the answer depends on where you
    /// already are, which is the whole point.
    pub fn decide(
        &self,
        current: Detail,
        distance_in_extents: f32,
        surface_bricks: usize,
    ) -> Detail {
        // An incoherent policy has no band at all — the two bounds cross, so
        // "further than drop" and "nearer than restore" overlap and every
        // distance in the overlap is both. Rather than pick one and flicker,
        // a misconfigured policy draws everything: being coarse is a
        // performance choice and being full is a correctness one, so the cost
        // of the mistake should be speed.
        if !self.is_coherent() || surface_bricks < self.floor {
            return Detail::Full;
        }
        // Nothing meshed yet is not a level to hold on to.
        let current = match current {
            Detail::Pending => Detail::Full,
            held => held,
        };
        if distance_in_extents <= self.restore_within {
            Detail::Full
        } else if distance_in_extents > self.drop_beyond {
            Detail::Reduced
        } else {
            // Between the two bounds nothing changes, which is the band that
            // stops a resting camera flickering.
            current
        }
    }

    /// Whether the policy is coherent: restoring must happen nearer than
    /// dropping, or the band is inverted and every frame flips.
    pub fn is_coherent(&self) -> bool {
        self.restore_within < self.drop_beyond
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUSY: usize = 10_000;

    #[test]
    fn a_distant_model_drops_to_the_mip() {
        let policy = DetailPolicy::default();
        assert_eq!(policy.decide(Detail::Full, 10.0, BUSY), Detail::Reduced);
    }

    #[test]
    fn a_near_model_is_drawn_whole() {
        let policy = DetailPolicy::default();
        assert_eq!(policy.decide(Detail::Reduced, 0.5, BUSY), Detail::Full);
    }

    #[test]
    fn the_band_between_the_bounds_changes_nothing() {
        // The property the whole type exists for. A camera resting in the gap
        // keeps whatever it had, so a twitch cannot swap the surface.
        let policy = DetailPolicy::default();
        let between = (policy.restore_within + policy.drop_beyond) * 0.5;
        assert_eq!(policy.decide(Detail::Full, between, BUSY), Detail::Full);
        assert_eq!(
            policy.decide(Detail::Reduced, between, BUSY),
            Detail::Reduced
        );
    }

    #[test]
    fn creeping_across_the_band_does_not_flicker() {
        // Walk the camera out past the drop and back in past the restore, one
        // small step at a time, and count how often the answer changes. Two is
        // the honest minimum: out once, back once.
        let policy = DetailPolicy::default();
        let mut detail = Detail::Full;
        let mut changes = 0;

        let walk = |from: f32, to: f32, detail: &mut Detail, changes: &mut usize| {
            let steps = 200;
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                let distance = from + (to - from) * t;
                let next = policy.decide(*detail, distance, BUSY);
                if next != *detail {
                    *changes += 1;
                    *detail = next;
                }
            }
        };
        walk(0.0, 6.0, &mut detail, &mut changes);
        walk(6.0, 0.0, &mut detail, &mut changes);

        assert_eq!(
            changes, 2,
            "the level changed {changes} times over one round trip"
        );
        assert_eq!(detail, Detail::Full, "it did not come back");
    }

    #[test]
    fn a_small_model_is_never_coarsened() {
        // However far away. A model that meshes inside a frame at full
        // resolution has nothing to gain and a visible amount to lose.
        let policy = DetailPolicy::default();
        assert_eq!(policy.decide(Detail::Reduced, 100.0, 10), Detail::Full);
        assert_eq!(
            policy.decide(Detail::Full, 100.0, policy.floor - 1),
            Detail::Full
        );
    }

    #[test]
    fn an_incoherent_policy_is_recognisable() {
        assert!(DetailPolicy::default().is_coherent());
        let inverted = DetailPolicy {
            drop_beyond: 1.0,
            restore_within: 5.0,
            ..Default::default()
        };
        assert!(!inverted.is_coherent());
    }

    #[test]
    fn an_inverted_policy_errs_toward_showing_everything() {
        // Misconfiguration should cost speed, not correctness: with the bounds
        // crossed, every distance resolves to full detail rather than
        // flip-flopping.
        let inverted = DetailPolicy {
            drop_beyond: 1.0,
            restore_within: 5.0,
            ..Default::default()
        };
        for distance in [0.0f32, 0.5, 1.0, 2.0, 5.0, 50.0] {
            assert_eq!(
                inverted.decide(Detail::Reduced, distance, BUSY),
                Detail::Full,
                "at {distance} extents"
            );
        }
    }

    #[test]
    fn nothing_meshed_yet_is_not_a_level_to_hold() {
        // `Pending` is the state before the first mesh, not a level the
        // hysteresis band should preserve — holding it would leave a document
        // reporting "nothing" for as long as the camera sat in the gap.
        let policy = DetailPolicy::default();
        let between = (policy.restore_within + policy.drop_beyond) * 0.5;
        assert_eq!(policy.decide(Detail::Pending, between, BUSY), Detail::Full);
    }
}
