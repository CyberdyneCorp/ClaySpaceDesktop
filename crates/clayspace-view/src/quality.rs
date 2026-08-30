//! How much a frame is worth spending on.
//!
//! A sculpting application should not pay the same for a frame under a moving
//! pen and a frame nobody is touching. The frame under the pen is one of
//! hundreds in a stroke, it will be replaced in sixteen milliseconds, and no
//! decision the sculptor makes from it depends on the quality of its ambient
//! occlusion. The frame after the stroke is the one they actually read.
//!
//! Two things are separated here, because they answer to different people:
//!
//! - a **profile** is the sculptor's choice, and it says what an idle frame
//!   should look like;
//! - a **quality** is this module's choice, and it says what *this* frame gets
//!   given what the pointer is doing.
//!
//! The renderer is told the answer. It does not read pointer state, does not
//! own a clock and does not decide when a stroke has ended — a renderer that
//! discovered any of that would be a second place where "is the user
//! sculpting" is defined, and the two would disagree.
//!
//! Hysteresis is the other half. Raising quality on every pointer release
//! would rebuild the frame at full cost between two dabs of the same stroke,
//! which is worse than never raising it at all: the cost lands exactly where
//! the latency is being measured.

use std::time::{Duration, Instant};

/// How the sculpt is shaded.
///
/// Two modes, and the split is deliberate rather than a step toward replacing
/// one with the other. A MatCap carries its whole lighting environment in one
/// texture indexed by the view-space normal: it is a single fetch, it is stable
/// under a moving camera, and form reads from it better than from any light
/// rig. That is why it is the default and why it stays the default.
///
/// What it cannot show is a highlight *moving*. Its lighting is welded to the
/// camera, so orbiting the form orbits the light with it — which is exactly the
/// property that makes it good for reading form and useless for judging how a
/// surface will behave under a real light. Studio mode answers that one
/// question and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShadingMode {
    /// The default: a MatCap, indexed by the view-space normal.
    #[default]
    MatCap,
    /// A fixed three-light rig with a filmic curve over it, for checking how
    /// the form takes a real light.
    Studio,
}

impl ShadingMode {
    pub const ALL: [Self; 2] = [Self::MatCap, Self::Studio];

    pub fn label(self) -> &'static str {
        match self {
            Self::MatCap => "MatCap",
            Self::Studio => "Estúdio",
        }
    }
}

/// How the studio rig treats the surface.
///
/// Sculpt materials are dielectric and middling-rough — the defaults here are
/// what clay is, and the dials exist because a piece meant to be cast in bronze
/// is judged differently from one meant to be fired.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StudioMaterial {
    pub roughness: f32,
    pub metallic: f32,
    /// A multiplier before the filmic curve, so the whole rig can be taken up
    /// or down without changing its shape.
    pub exposure: f32,
}

impl Default for StudioMaterial {
    fn default() -> Self {
        Self {
            roughness: 0.62,
            metallic: 0.0,
            exposure: 1.0,
        }
    }
}

/// What an idle frame should look like. The sculptor's choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportProfile {
    /// The least the viewport can draw and still read as form. For a machine
    /// that is struggling, or a scene that is too large for the one it is on.
    Performance,
    /// The default: enough occlusion to read a fold, and nothing that costs
    /// latency under the pen.
    #[default]
    Sculpt,
    /// For looking rather than working. Everything on, and still dropped to
    /// the interactive tier the moment a stroke starts.
    Presentation,
}

impl ViewportProfile {
    pub const ALL: [Self; 3] = [Self::Performance, Self::Sculpt, Self::Presentation];

    /// The best this profile will ever draw.
    fn ceiling(self) -> ViewportQuality {
        match self {
            Self::Performance => ViewportQuality::Interactive,
            Self::Sculpt => ViewportQuality::Balanced,
            Self::Presentation => ViewportQuality::High,
        }
    }

    /// What it draws once the pointer has stopped but not yet settled.
    fn settled(self) -> ViewportQuality {
        match self {
            Self::Performance => ViewportQuality::Interactive,
            Self::Sculpt | Self::Presentation => ViewportQuality::Balanced,
        }
    }
}

/// What this frame gets. This module's choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViewportQuality {
    /// Under a moving pen or a moving camera.
    Interactive,
    /// Just after one stopped.
    Balanced,
    /// Nothing is happening.
    High,
}

impl ViewportQuality {
    /// Samples per occlusion pixel.
    ///
    /// Sixteen was the full-resolution figure, and is where the noise stopped
    /// changing what the composite produced. At half resolution the composite
    /// averages a wider neighbourhood in display pixels, so twelve reaches the
    /// same place — and under the pen, where the frame lives for sixteen
    /// milliseconds and no decision rests on it, eight does.
    pub fn ao_samples(self) -> u32 {
        match self {
            Self::Interactive => 8,
            Self::Balanced => 12,
            Self::High => 16,
        }
    }

    /// Whether the cavity term is drawn.
    ///
    /// Off under the pen at every profile. It is a second neighbourhood read
    /// over the whole frame, and it sharpens detail a sculptor judges when
    /// they stop rather than while they push.
    pub fn cavity(self) -> bool {
        matches!(self, Self::High)
    }

    /// Whether occlusion accumulates over frames.
    ///
    /// Never while anything is moving: the history is reprojected, and
    /// reprojection through a changing surface is what leaves a trail behind a
    /// brush.
    pub fn temporal(self) -> bool {
        matches!(self, Self::Balanced | Self::High)
    }
}

/// What the pointer is doing this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InteractionState {
    /// A stroke is in progress.
    pub sculpting: bool,
    /// The camera is being orbited, panned or zoomed.
    pub camera_moving: bool,
}

impl InteractionState {
    pub fn is_moving(self) -> bool {
        self.sculpting || self.camera_moving
    }
}

/// Chooses a quality from what the pointer is doing, and refuses to change its
/// mind too quickly.
#[derive(Debug, Clone)]
pub struct QualityGovernor {
    profile: ViewportProfile,
    quality: ViewportQuality,
    /// When the pointer last stopped, or `None` while it is moving.
    still_since: Option<Instant>,
}

impl QualityGovernor {
    /// How long after a gesture ends before the frame is worth redrawing
    /// better.
    ///
    /// Long enough to cover the gap between two dabs of one stroke, which is
    /// what a pointer that is being *used* looks like from here, and short
    /// enough that a sculptor who has genuinely stopped does not notice the
    /// wait.
    pub const SETTLE: Duration = Duration::from_millis(160);

    /// How long before nothing-is-happening is believed.
    pub const IDLE: Duration = Duration::from_millis(600);

    pub fn new(profile: ViewportProfile) -> Self {
        Self {
            profile,
            quality: profile.ceiling(),
            still_since: None,
        }
    }

    pub fn profile(&self) -> ViewportProfile {
        self.profile
    }

    /// Changes the profile, and re-reads the quality against it at once.
    ///
    /// Immediately rather than at the next settle, because choosing a profile
    /// is itself an idle act: the sculptor is in a menu, not in a stroke, and
    /// a change that takes half a second to show reads as not having worked.
    pub fn set_profile(&mut self, profile: ViewportProfile, now: Instant) {
        self.profile = profile;
        self.quality = self.decide(now);
    }

    /// Takes this frame's interaction state. Called once a frame.
    ///
    /// Returns whether the quality changed, so a caller can ask for the redraw
    /// that a rise in quality is only useful with — an application that draws
    /// on demand would otherwise settle to High and never show it.
    pub fn observe(&mut self, state: InteractionState, now: Instant) -> bool {
        if state.is_moving() {
            self.still_since = None;
        } else if self.still_since.is_none() {
            self.still_since = Some(now);
        }
        let quality = self.decide(now);
        let changed = quality != self.quality;
        self.quality = quality;
        changed
    }

    /// What this frame should be drawn at.
    pub fn quality(&self) -> ViewportQuality {
        self.quality
    }

    /// When the next rise would happen, so a caller drawing on demand knows
    /// how long to wait rather than polling.
    ///
    /// `None` once there is nothing further to rise to.
    pub fn next_change(&self, now: Instant) -> Option<Duration> {
        let still = self.still_since?;
        let waited = now.saturating_duration_since(still);
        [Self::SETTLE, Self::IDLE]
            .into_iter()
            .find(|at| waited < *at)
            .map(|at| at - waited)
            .filter(|_| self.quality < self.profile.ceiling())
    }

    fn decide(&self, now: Instant) -> ViewportQuality {
        let Some(still) = self.still_since else {
            return ViewportQuality::Interactive;
        };
        let waited = now.saturating_duration_since(still);
        let reached = if waited >= Self::IDLE {
            ViewportQuality::High
        } else if waited >= Self::SETTLE {
            self.profile.settled()
        } else {
            ViewportQuality::Interactive
        };
        // A profile is a ceiling, not a target: Presentation still starts at
        // Interactive under the pen, and Performance never leaves it.
        reached.min(self.profile.ceiling())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One frame's observation, `ms` after the sequence started.
    ///
    /// The base instant is shared across a sequence rather than taken per
    /// call: the governor measures how long the pointer has been still, so a
    /// helper that re-read the clock each time would restart that measurement
    /// on every frame and nothing would ever settle.
    fn at(
        governor: &mut QualityGovernor,
        base: Instant,
        state: InteractionState,
        ms: u64,
    ) -> ViewportQuality {
        governor.observe(state, base + Duration::from_millis(ms));
        governor.quality()
    }

    const SCULPTING: InteractionState = InteractionState {
        sculpting: true,
        camera_moving: false,
    };
    const STILL: InteractionState = InteractionState {
        sculpting: false,
        camera_moving: false,
    };

    /// A stroke drops the frame at once. Waiting even one frame would spend
    /// the first dab's latency on quality nobody asked for.
    #[test]
    fn a_stroke_drops_the_quality_immediately() {
        let mut governor = QualityGovernor::new(ViewportProfile::Presentation);
        let base = Instant::now();
        assert_eq!(governor.quality(), ViewportQuality::High);
        assert_eq!(
            at(&mut governor, base, SCULPTING, 0),
            ViewportQuality::Interactive
        );
    }

    /// And releasing does not raise it again, or the gap between two dabs of
    /// one stroke would be paid at full price — which puts the cost exactly
    /// where the latency is measured.
    #[test]
    fn a_pause_inside_a_stroke_does_not_raise_it() {
        let mut governor = QualityGovernor::new(ViewportProfile::Sculpt);
        let base = Instant::now();
        at(&mut governor, base, SCULPTING, 0);
        assert_eq!(
            at(&mut governor, base, STILL, 1),
            ViewportQuality::Interactive
        );
        assert_eq!(
            at(&mut governor, base, STILL, 100),
            ViewportQuality::Interactive,
            "a hundred milliseconds is the gap between two dabs, not the end of a stroke"
        );
        assert_eq!(
            at(&mut governor, base, SCULPTING, 120),
            ViewportQuality::Interactive
        );
    }

    /// A pointer that has genuinely stopped settles, and then rises.
    #[test]
    fn stillness_settles_and_then_rises() {
        let mut governor = QualityGovernor::new(ViewportProfile::Presentation);
        let base = Instant::now();
        at(&mut governor, base, SCULPTING, 0);
        assert_eq!(
            at(&mut governor, base, STILL, 1),
            ViewportQuality::Interactive
        );
        assert_eq!(
            at(&mut governor, base, STILL, 200),
            ViewportQuality::Balanced
        );
        assert_eq!(at(&mut governor, base, STILL, 700), ViewportQuality::High);
    }

    /// A profile is a ceiling. Performance never leaves the interactive tier
    /// however long the pointer is still, and Sculpt never reaches High.
    #[test]
    fn a_profile_is_a_ceiling_rather_than_a_target() {
        let base = Instant::now();
        let mut fast = QualityGovernor::new(ViewportProfile::Performance);
        at(&mut fast, base, STILL, 0);
        assert_eq!(
            at(&mut fast, base, STILL, 5_000),
            ViewportQuality::Interactive
        );

        let mut sculpt = QualityGovernor::new(ViewportProfile::Sculpt);
        at(&mut sculpt, base, STILL, 0);
        assert_eq!(
            at(&mut sculpt, base, STILL, 5_000),
            ViewportQuality::Balanced
        );
    }

    /// Moving the camera is a gesture too. A frame drawn while the view is
    /// swinging is seen for one frame.
    #[test]
    fn moving_the_camera_counts_as_interaction() {
        let mut governor = QualityGovernor::new(ViewportProfile::Presentation);
        let base = Instant::now();
        at(&mut governor, base, STILL, 0);
        at(&mut governor, base, STILL, 1_000);
        let moving = InteractionState {
            sculpting: false,
            camera_moving: true,
        };
        assert_eq!(
            at(&mut governor, base, moving, 1_001),
            ViewportQuality::Interactive
        );
    }

    /// The caller is told when the quality moved, because an application that
    /// draws on demand would otherwise settle to High and never show it.
    #[test]
    fn the_governor_says_when_a_redraw_is_worth_asking_for() {
        let mut governor = QualityGovernor::new(ViewportProfile::Sculpt);
        let start = Instant::now();
        assert!(
            governor.observe(SCULPTING, start),
            "High down to Interactive"
        );
        assert!(!governor.observe(SCULPTING, start + Duration::from_millis(16)));
        assert!(!governor.observe(STILL, start + Duration::from_millis(20)));
        assert!(
            governor.observe(STILL, start + Duration::from_millis(300)),
            "the settle is a change the caller has to redraw for"
        );
    }

    /// And how long until the next one, so it can wait rather than poll.
    #[test]
    fn it_says_how_long_until_the_next_rise() {
        let mut governor = QualityGovernor::new(ViewportProfile::Sculpt);
        let start = Instant::now();
        governor.observe(SCULPTING, start);
        governor.observe(STILL, start);
        let wait = governor
            .next_change(start)
            .expect("a settle is still to come");
        assert_eq!(wait, QualityGovernor::SETTLE);

        governor.observe(STILL, start + Duration::from_millis(300));
        assert_eq!(
            governor.next_change(start + Duration::from_millis(300)),
            None,
            "Sculpt has nothing above Balanced to rise to"
        );
    }

    /// Each tier costs less than the one above it, in every dimension. A tier
    /// that spent more on something would not be a tier.
    #[test]
    fn the_tiers_are_ordered_by_what_they_cost() {
        let tiers = [
            ViewportQuality::Interactive,
            ViewportQuality::Balanced,
            ViewportQuality::High,
        ];
        for pair in tiers.windows(2) {
            assert!(
                pair[0].ao_samples() < pair[1].ao_samples(),
                "{:?} takes as many occlusion samples as {:?}",
                pair[0],
                pair[1]
            );
            assert!(
                !pair[0].cavity() || pair[1].cavity(),
                "{:?} draws the cavity term and {:?} above it does not",
                pair[0],
                pair[1]
            );
            assert!(
                !pair[0].temporal() || pair[1].temporal(),
                "{:?} accumulates and {:?} above it does not",
                pair[0],
                pair[1]
            );
        }
        assert!(
            !ViewportQuality::Interactive.cavity(),
            "the cavity term must never be drawn under the pen"
        );
        assert!(!ViewportQuality::Interactive.temporal());
    }
}
