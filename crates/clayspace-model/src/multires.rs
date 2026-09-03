//! A subdivision hierarchy: the levels it holds, and the passes stacked on it.
//!
//! Two things an artist manipulates, and neither of them is the surface.
//!
//! # Two levels, not one
//!
//! Where the brush writes and what the viewport draws are **independent
//! numbers**. That is not an implementation detail to be tidied away into one
//! "current level": moving the broad form at level 1 while watching the pores
//! at level 5 is the workflow the whole representation exists for, and a model
//! that collapsed the two into a single number could not express it — it could
//! only offer "sculpt coarse and look coarse" or "sculpt fine and look fine",
//! which are the two things a plain mesh already does.
//!
//! [`MultiresLevels`] therefore carries both, and every transition here moves
//! one of them or says which it moves. The sharpest consequence is that
//! **changing where you sculpt redraws nothing** — see
//! [`MultiresLevelOp::changes_what_is_drawn`].
//!
//! # Not [`crate::detail`]
//!
//! The two modules share the English word and share nothing else, and the
//! collision is worth naming because the wrong one is one letter away.
//! [`crate::DetailPolicy`] is camera-distance level-of-detail: how far away a
//! form has to be before the *viewport* draws it off a coarser mip, with
//! hysteresis so a resting camera does not flicker. It is a rendering
//! economy and the sculpt is identical either way.
//!
//! A hierarchy's levels are the sculpt. Level 4 holds displacements level 1
//! does not, and dropping to level 1 is not a cheaper picture of the same
//! surface — it is a different surface, which is why one of these is a policy
//! and the other is state.
//!
//! # The stack, and the noun it shares
//!
//! ClayCore calls the hierarchy's passes *sculpt layers*, which is the same
//! word [`crate::SculptLayer`] already spends on a voxel grid's, and upstream
//! does that on purpose: the artist's statement is identical — a named pass you
//! keep, as against undo, which is a stack you pop — so the two read alike and
//! neither reads like a brush called Camada. That is a shared name and not a
//! collision, and inventing a third word here would cost the interface the one
//! row widget that can draw both stacks.
//!
//! What the two do **not** share is how a pass is addressed, and reusing one
//! addressing for the other is a defect the C header documents itself against:
//!
//! | | [`crate::SculptLayer`], a grid's | [`MultiresSculptLayer`], a hierarchy's |
//! |---|---|---|
//! | addressed by | `index: usize` | [`MultiresSculptLayerId`], minted once |
//! | a reorder | renumbers every position at or below it | renumbers nothing |
//! | order | replays cell writes, so it **is** the result | additive, so it changes organisation and not geometry |
//! | opened by | begin-recording / end-recording | an active pass and a [`WriteDomain`] |
//!
//! So the grid's stack keeps its bare `usize` and this one is addressed by a
//! newtype that cannot be built out of a stack position by accident. An index
//! survives here only as [`MultiresSculptLayer::index`], which is draw order
//! and says so.
//!
//! # Strength is composition, not a scale on the pen
//!
//! The behaviour most likely to be reported as a bug, so it is written down
//! rather than left in the engine's header. A stroke into a pass at strength
//! 0.5 records its **full** contribution and the surface moves half as far as
//! the pen asked for; raising the slider to 1 afterwards doubles what is on
//! screen and replays no stroke. Nothing anywhere divides by a strength — which
//! is also why [`MultiresSculptLayerOp::MergeDown`] and
//! [`MultiresSculptLayerOp::BakeToBase`] are defined by the surface they leave
//! rather than by adding coefficients together, an arithmetic that is undefined
//! at zero and zero is a place one slider reaches.
//!
//! # What is deliberately not modelled here
//!
//! The per-vertex coefficients and the per-vertex mask. Neither crosses the C
//! ABI as anything a host can hold — a mask is read and written one vertex at a
//! time — and a domain type carrying twenty million floats would be a copy of
//! the surface wearing a summary's name. What is here instead is what a
//! sculptor acts on: whether a pass *has* a mask
//! ([`MultiresSculptLayer::masked`]) and what it covers
//! ([`MultiresSculptLayer::coverage_vertices`]).

use crate::conversion::Refusal;

// -- the two levels ----------------------------------------------------------

/// Which level the brush writes on, and which one the viewport draws.
///
/// Both, always, because they are two answers. See the module's opening
/// paragraph for why collapsing them is not a simplification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiresLevels {
    /// How many levels the hierarchy holds. Never zero: the cage is level 0
    /// and a hierarchy without one is not a hierarchy.
    pub count: u32,
    /// The level a stamp lands on.
    pub sculpt: u32,
    /// The level the viewport draws.
    pub display: u32,
}

impl MultiresLevels {
    /// The cage. Level zero everywhere, and the only level a freshly built
    /// hierarchy has — `clay_multires_from_mesh` builds one level and adding
    /// more is a separate, priced operation.
    pub const CAGE: u32 = 0;

    /// How deep the pinned engine will go.
    ///
    /// `MultiresSurface::kMaxLevels` in `vendor/ClayCore/include/clay/mesh/
    /// multires.h`, which is **not** reported across the C ABI — there is no
    /// entry point that answers "how deep may I go", only one that refuses.
    /// So this is a number read out of the engine this build is pinned to, and
    /// it is here to grey a button rather than to enforce anything: the engine
    /// refuses past its own ceiling before allocating, and that refusal stands
    /// whatever this says.
    ///
    /// Twelve levels of Catmull-Clark is a factor of four to the twelfth over
    /// the cage, so this ceiling is one nobody reaches by accident.
    pub const DEEPEST: u32 = 12;

    /// A hierarchy that is only its cage — what a crossing from a mesh makes.
    pub fn just_the_cage() -> Self {
        Self {
            count: 1,
            sculpt: Self::CAGE,
            display: Self::CAGE,
        }
    }

    /// The highest level that exists.
    pub fn highest(self) -> u32 {
        self.count.max(1) - 1
    }

    /// Both numbers brought inside the levels that exist.
    ///
    /// Clamped rather than refused for the reason every `sanitized` here
    /// clamps: these are read back from a surface that may have lost its top
    /// level since the interface last looked, and a sculptor who removed a
    /// level meant "show me what is left" rather than "fail".
    pub fn sanitized(self) -> Self {
        let count = self.count.max(1);
        let highest = count - 1;
        Self {
            count,
            sculpt: self.sculpt.min(highest),
            display: self.display.min(highest),
        }
    }

    pub fn is_only_the_cage(self) -> bool {
        self.count <= 1
    }

    /// Whether the brush is writing on the cage itself.
    ///
    /// Worth asking on its own: a stamp at level 0 moves the cage, which moves
    /// the frames every level above stores its detail in, so the detail rides
    /// along. That is the representation working, and it is also the one edit
    /// that reaches every level at once.
    pub fn sculpting_the_cage(self) -> bool {
        self.sanitized().sculpt == Self::CAGE
    }

    /// Whether what is being drawn is what is being written.
    ///
    /// False is the interesting state and not an error: it is the workflow.
    /// But it is also the answer to "why is my dab so much softer than the
    /// brush ring", so an interface has something to say when it is false.
    pub fn draws_what_it_sculpts(self) -> bool {
        let levels = self.sanitized();
        levels.sculpt == levels.display
    }

    /// The sculpt level moved, and nothing else.
    pub fn with_sculpt(self, level: u32) -> Self {
        Self {
            sculpt: level,
            ..self
        }
        .sanitized()
    }

    /// The display level moved, and nothing else.
    pub fn with_display(self, level: u32) -> Self {
        Self {
            display: level,
            ..self
        }
        .sanitized()
    }

    /// One level added, with both numbers moved to it.
    ///
    /// Moving both is what an artist means by "subdivide" — they subdivide in
    /// order to work finer — and it is what `clay_multires_add_level` does, so
    /// a host that left the display where it was would show a surface the
    /// engine had already moved on from.
    ///
    /// Refuses at the ceiling. It cannot refuse for cost, because cost is a
    /// property of the surface rather than of these three numbers; that is
    /// [`SubdivisionCost::within`], and both refusals are the same vocabulary.
    pub fn subdivided(self) -> Result<Self, Refusal> {
        let here = self.sanitized();
        if here.count >= Self::DEEPEST {
            return Err(Refusal::DepthLimit { levels: here.count });
        }
        let top = here.count;
        Ok(Self {
            count: here.count + 1,
            sculpt: top,
            display: top,
        })
    }

    /// The highest level dropped, with the detail on it.
    ///
    /// `None` where there is nothing to drop, which is the cage — the engine's
    /// own `NO_LEVEL_TO_REMOVE`. Destructive: what came off is not recoverable
    /// from what is left, which is why [`MultiresLevelOp::is_destructive`]
    /// exists and why an interface confirms before calling it.
    pub fn coarsened(self) -> Option<Self> {
        let here = self.sanitized();
        if here.is_only_the_cage() {
            return None;
        }
        Some(
            Self {
                count: here.count - 1,
                ..here
            }
            .sanitized(),
        )
    }

    /// Every level, coarsest first — the order a level picker lists them in.
    pub fn all(self) -> Vec<u32> {
        (0..self.sanitized().count).collect()
    }
}

/// What adding one more level would cost.
///
/// Read from the engine's own preflight rather than computed here, with one
/// exception noted on [`SubdivisionCost::faces_after`]: Catmull-Clark multiplies
/// faces by four, so a 20k-quad cage is 5.1M faces at level 4 and 20.5M at level
/// 5, and the figure that matters on a constrained device is the **peak** during
/// the build rather than what remains after it.
///
/// Not a [`crate::Cost`]. That type states what a crossing *loses* — how far the
/// surface moves, what feature vanishes, whether sharp edges survive — and a
/// subdivision loses none of those; it costs memory and nothing else. Filling a
/// `Cost` in with zeroes and a cell count that means nothing would be the same
/// mistake `Direction::chooses_resolution` records having made once already.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubdivisionCost {
    /// The level that would come into existence.
    pub level: u32,
    pub vertices: u64,
    pub faces: u64,
    /// What remains held after the level is built.
    pub persistent_bytes: u64,
    /// The high-water mark during the build. The number that ends a session.
    pub peak_bytes: u64,
}

impl SubdivisionCost {
    /// What one Catmull-Clark step does to a face count.
    ///
    /// A quad becomes four quads, so this is exact for a quad cage and a lower
    /// bound for one carrying triangles or n-gons, where the first step also
    /// quadrangulates. Used for the projection a slider shows before the engine
    /// is asked; the engine's own preflight is what a refusal is taken from.
    pub const FACES_MULTIPLY_BY: u64 = 4;

    /// The faces `steps` further subdivisions would produce from `faces`.
    ///
    /// Saturating, and that is the whole of why it is a function rather than a
    /// `*` at the call site. The failure of an unchecked multiply here is not
    /// that the projection is wrong: it is that a wrapped product is a *small*
    /// number, so a subdivision that cannot be represented at all reads as one
    /// that fits comfortably, and every budget check downstream agrees. The
    /// engine names its own version of that refusal `CAPACITY_OVERFLOW` and
    /// says the same thing about it — the one refusal in its list that is about
    /// arithmetic rather than about the model.
    ///
    /// It does not overflow within the depths the engine allows: twelve steps
    /// is a factor of about seventeen million, which sixty-four bits carries
    /// from any cage a machine can hold. `steps` is a caller's number, though,
    /// and a projection asked for a depth nobody can reach should saturate
    /// rather than wrap.
    pub fn faces_after(faces: u64, steps: u32) -> u64 {
        (0..steps).fold(faces, |faces, _| {
            faces.saturating_mul(Self::FACES_MULTIPLY_BY)
        })
    }

    /// Whether the peak fits a byte budget.
    ///
    /// The peak and not the persistent figure, and that is the whole content of
    /// this method: a level that fits once it is built and does not fit while
    /// it is being built is a level that cannot be added.
    pub fn within(&self, budget_bytes: u64) -> Result<(), Refusal> {
        if self.peak_bytes > budget_bytes {
            return Err(Refusal::LevelOverBudget {
                peak_bytes: self.peak_bytes,
                budget_bytes,
            });
        }
        Ok(())
    }

    /// How much more the build holds than it leaves behind.
    ///
    /// What a sculptor is being warned about when a subdivision that "fits"
    /// refuses. `1.0` where the two are equal; never below it.
    pub fn peak_over_persistent(&self) -> f32 {
        if self.persistent_bytes == 0 {
            return 1.0;
        }
        (self.peak_bytes as f32 / self.persistent_bytes as f32).max(1.0)
    }
}

// -- the stack ---------------------------------------------------------------

/// Names one pass on a hierarchy's stack, for as long as the pass exists.
///
/// **An id, never a position.** Sliding a pass through the stack changes the
/// position of every pass at or below it, so a position handed to the interface,
/// written into a file or held across a drag names a different pass afterwards.
/// Ids come from a counter the engine serializes, so a save, a load and a
/// reorder leave every one of them exactly where it was.
///
/// There is deliberately no way to make one out of a `usize`.
/// [`MultiresSculptLayerId::new`] takes the `u64` the engine minted, which a
/// stack position does not coerce into, and [`MultiresSculptLayer::index`] —
/// the only position in this module — is documented as draw order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MultiresSculptLayerId(u64);

impl MultiresSculptLayerId {
    /// No pass: the form under them.
    ///
    /// What an empty stack's active pass reads as, and what routes the next
    /// stroke into the level's own detail — the cage at level 0. Zero, which is
    /// the engine's `CLAY_NO_SCULPT_LAYER`, and never an id a pass can be given.
    pub const BASE: Self = Self(0);

    /// The id the engine minted for this pass.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Whether this names the form under the passes rather than a pass.
    pub const fn is_base(self) -> bool {
        self.0 == Self::BASE.0
    }
}

/// One pass on a hierarchy, as the layer stack shows it.
///
/// A pass a sculptor can dial back after making it — the same statement
/// [`crate::SculptLayer`] makes about a grid, and a different implementation of
/// it. See the module doc's table for the four ways they differ.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiresSculptLayer {
    /// What names it, now and after every reorder.
    pub id: MultiresSculptLayerId,
    /// Where it sits in the stack, bottom-first.
    ///
    /// **Draw order, and nothing else.** It is not how the pass is addressed
    /// and it is not what decides the result: passes here are additive and
    /// therefore commute, so this says where the row goes in a list and never
    /// which pass wins. Valid until the next structural change to the stack.
    pub index: usize,
    /// What the sculptor called it. May be empty.
    pub name: String,
    /// How far the pass is dialled in, 0..=1.
    ///
    /// Composition and not a scale on the pen — see the module doc.
    pub strength: f32,
    /// Whether it contributes at all. Invisible is *exactly* zero rather than
    /// nearly zero, so hiding a pass and comparing is an exact test.
    pub visible: bool,
    /// Whether its stored coefficients refuse to be written.
    ///
    /// A lock refuses a sculpt write, a merge and a bake, and permits every
    /// property change — the name, the slider, the visibility and the mask are
    /// all still the sculptor's. A lock that froze those would make "lock" mean
    /// "hide from the interface", which is a different feature.
    pub locked: bool,
    /// Whether the pass carries a stored mask of its own.
    ///
    /// A different question from the brush's freeze, and the two are easy to
    /// confuse: the freeze says where a brush may write and is gone when the
    /// pointer comes up; this says where a *stored* pass contributes and is
    /// saved with it. Its identity is 1 — a pass with no mask and a pass masked
    /// to 1 everywhere are the same surface — so this is "has one stored",
    /// which is what an interface needs to offer clearing it.
    pub masked: bool,
    /// How many vertices it covers.
    ///
    /// A pass costs its coverage and not the model, which is what makes a
    /// hundred passes over one cheek affordable, and what makes this the honest
    /// figure to show beside the bytes.
    pub coverage_vertices: u64,
    /// What its coefficients and mask occupy.
    pub bytes: usize,
}

impl MultiresSculptLayer {
    /// What the interface shows when the pass has no name.
    ///
    /// Numbered from one, as a grid's passes are, and off `index` because that
    /// is what a sculptor is counting — the rows in front of them. Not off the
    /// id: ids are minted from a counter that never reuses a value, so a
    /// sculptor who made and deleted nine passes would meet "Passe 10" as their
    /// second.
    pub fn display_name(&self) -> String {
        if self.name.is_empty() {
            format!("Passe {}", self.index + 1)
        } else {
            self.name.clone()
        }
    }

    /// Whether the pass has anything stored on it.
    pub fn is_empty(&self) -> bool {
        self.coverage_vertices == 0
    }

    /// Whether the pass moves the surface as things stand.
    ///
    /// Both halves matter and they fail differently: a hidden pass is one click
    /// from contributing, and a pass at zero strength is one drag from it.
    pub fn contributes(&self) -> bool {
        self.visible && self.strength > 0.0
    }

    /// Whether a stroke aimed at this pass would be taken.
    pub fn accepts_a_stroke(&self) -> bool {
        !self.locked
    }
}

/// Where the next stroke on a hierarchy lands.
///
/// Chosen rather than inferred, because the two cases are opposite and neither
/// is a default: "sculpt the pass I am working on" and "fix the form *under* the
/// passes without disturbing them" are both ordinary things to want, and a rule
/// that picked one would be wrong half the time.
///
/// Fixed when the gesture opens rather than read again per stamp, so that
/// changing the active pass mid-stroke cannot split one gesture across two
/// channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WriteDomain {
    /// The active pass where there is one, and the form under them where there
    /// is not. What a stroke did before a stack existed, and what it should keep
    /// doing for a sculptor who has not made one.
    #[default]
    Automatic,
    /// The form under the passes: the cage at level 0, the level's own detail
    /// above it. Every pass is left exactly as it was.
    Geometry,
    /// The active pass. Refuses to open where there is none, rather than
    /// quietly writing the form the sculptor asked not to touch.
    Detail,
}

impl WriteDomain {
    pub const ALL: [WriteDomain; 3] = [Self::Automatic, Self::Geometry, Self::Detail];

    /// Which channel a stroke would enter, given the active pass.
    ///
    /// `None` where the gesture would be refused, which is the one combination
    /// that has no answer: [`WriteDomain::Detail`] with nothing active. A
    /// refusal here rather than a fallback to the base, because a sculptor who
    /// asked for "the pass" and got "the form" has damaged the thing they said
    /// to leave alone, and would find out later.
    pub fn resolve(self, active: MultiresSculptLayerId) -> Option<MultiresSculptLayerId> {
        match self {
            Self::Automatic => Some(active),
            Self::Geometry => Some(MultiresSculptLayerId::BASE),
            Self::Detail if active.is_base() => None,
            Self::Detail => Some(active),
        }
    }
}

/// Something done to a hierarchy's pass stack.
///
/// One enum rather than a method per verb, for the reason
/// [`crate::SculptLayerOp`] is one — and a *different* enum from it, for the
/// reason the module doc's table gives. Pointing one at the other would compile
/// and would address the wrong pass.
#[derive(Debug, Clone, PartialEq)]
pub enum MultiresSculptLayerOp {
    /// A new empty pass on top, at full strength, visible, made active.
    Add {
        name: String,
    },
    Rename {
        id: MultiresSculptLayerId,
        name: String,
    },
    /// Dials a pass up or down, 0..=1. Replays no stroke.
    SetStrength {
        id: MultiresSculptLayerId,
        strength: f32,
    },
    SetVisible {
        id: MultiresSculptLayerId,
        visible: bool,
    },
    SetLocked {
        id: MultiresSculptLayerId,
        locked: bool,
    },
    /// Routes the next stroke. [`MultiresSculptLayerId::BASE`] sends it into
    /// the form under the passes.
    SetActive {
        id: MultiresSculptLayerId,
    },
    /// Slides a pass through the stack. Organisation, and never geometry.
    Move {
        id: MultiresSculptLayerId,
        to: usize,
    },
    Remove {
        id: MultiresSculptLayerId,
    },
    /// Folds a pass into the one below it and discards it.
    MergeDown {
        id: MultiresSculptLayerId,
    },
    /// The same fold with the form under the passes as the target.
    BakeToBase {
        id: MultiresSculptLayerId,
    },
    /// Releases the storage a stroke that undid itself left behind.
    ///
    /// Never during a pointer event: it walks every stored block of every pass,
    /// which is proportional to the stack rather than to the dab.
    Compact,
}

impl MultiresSculptLayerOp {
    /// What the history calls it.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Add { .. } => "add pass",
            Self::Rename { .. } => "rename pass",
            Self::SetStrength { .. } => "pass strength",
            Self::SetVisible { .. } => "pass visibility",
            Self::SetLocked { .. } => "lock pass",
            Self::SetActive { .. } => "active pass",
            Self::Move { .. } => "reorder pass",
            Self::Remove { .. } => "remove pass",
            Self::MergeDown { .. } => "merge pass down",
            Self::BakeToBase { .. } => "bake pass into the form",
            Self::Compact => "compact passes",
        }
    }

    /// Which pass it is aimed at, where it is aimed at one.
    pub fn targets(&self) -> Option<MultiresSculptLayerId> {
        match self {
            Self::Rename { id, .. }
            | Self::SetStrength { id, .. }
            | Self::SetVisible { id, .. }
            | Self::SetLocked { id, .. }
            | Self::SetActive { id }
            | Self::Move { id, .. }
            | Self::Remove { id }
            | Self::MergeDown { id }
            | Self::BakeToBase { id } => Some(*id),
            Self::Add { .. } | Self::Compact => None,
        }
    }

    /// Whether the evaluated surface moves.
    ///
    /// **A reorder does not**, and that is the load-bearing answer in this
    /// file. Passes here are additive, so a sum is a sum whatever order it is
    /// written in — the engine defines a reorder as moving nothing, and
    /// `claycore/tests/multires.rs` measures that rather than taking it on
    /// trust: three hundred randomised five-pass stacks, each slid about six
    /// times and compared bit for bit, and none of them moved. It is worth
    /// knowing that the same probe caught 158 of 300 moving before the release
    /// that fixed it — the property is real and it was not free. An interface that
    /// treated this as a geometry edit would re-evaluate, re-upload and bank an
    /// undo step for a drag that changed a list, on a representation where that
    /// list is millions of vertices wide.
    ///
    /// Neither does a merge or a bake, and for a different reason: both are
    /// *defined* by visual parity — the surface after equals the surface
    /// before, at any strength including zero — so what they change is the
    /// stack, not the form. See [`MultiresSculptLayerOp::is_destructive`],
    /// which is the question an interface should be asking about those two.
    ///
    /// Nor does adding a pass, which arrives empty and covers nothing.
    pub fn changes_the_surface(&self) -> bool {
        matches!(
            self,
            Self::SetStrength { .. } | Self::SetVisible { .. } | Self::Remove { .. }
        )
    }

    /// Whether the interface should confirm before sending it.
    ///
    /// Not the same set as [`Self::changes_the_surface`], and the two disagree
    /// in both directions, which is why they are two questions. A merge leaves
    /// the surface identical and takes away a slider that cannot be got back; a
    /// strength change moves the whole surface and is one drag from being
    /// undone by hand.
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::Remove { .. } | Self::MergeDown { .. } | Self::BakeToBase { .. }
        )
    }

    /// Whether it has to wait for an open gesture to finish.
    ///
    /// A stroke reads the evaluated surface, which includes every visible pass,
    /// so a slider moved between two stamps would author one gesture against
    /// two different surfaces. The engine refuses these rather than deferring
    /// them, and refusing is right: a control that appears to move and then
    /// silently applies later is the worse surprise.
    ///
    /// The three that are allowed through are the three that move no vertex —
    /// a rename, a lock and a change of which pass is active.
    pub fn needs_the_stroke_closed(&self) -> bool {
        !matches!(
            self,
            Self::Rename { .. } | Self::SetLocked { .. } | Self::SetActive { .. }
        )
    }

    /// Whether a lock on the target pass refuses it.
    ///
    /// A lock guards *coefficients*. So it stops the two operations that
    /// rewrite them and lets every property change through, including
    /// unlocking — a lock a sculptor could not undo from the row that shows it
    /// would be a trap.
    pub fn refused_by_a_lock(&self) -> bool {
        matches!(self, Self::MergeDown { .. } | Self::BakeToBase { .. })
    }
}

/// What a hierarchy's passes cost together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MultiresSculptLayerCost {
    pub layers: usize,
    /// Coefficients and masks across the stack.
    ///
    /// Nothing is enforced against it, for the reason a grid's stack enforces
    /// nothing against its own: a cap that silently stopped recording would
    /// leave the pass on the surface and un-dialable, which is a correctness
    /// bug wearing a memory limit's clothes. The number is shown so a sculptor
    /// can compact, merge, bake or delete — which are the four levers, in
    /// increasing order of what they cost.
    pub bytes: usize,
    /// Vertices covered across the stack. Sums coverage, so a vertex two passes
    /// both reach is counted twice — which is what it costs.
    pub coverage_vertices: u64,
    /// Whether a gesture is holding the composition right now.
    ///
    /// Not "recording". A grid's stack has a recording mode — edits between a
    /// begin and an end belong to the new pass — and this one has none: there
    /// is an active pass and a write domain, both standing facts rather than a
    /// state to enter and leave. What this says is that a stroke is open, which
    /// is why the composition controls are refusing.
    pub stroke_open: bool,
}

impl MultiresSculptLayerCost {
    /// The cost in whole megabytes, for a readout.
    pub fn megabytes(&self) -> f32 {
        self.bytes as f32 / (1024.0 * 1024.0)
    }

    /// Above which the interface says the stack is worth compacting.
    ///
    /// Not a limit and not enforced. A quarter of a gigabyte is where a stack
    /// is large enough that a sculptor should know about it and small enough
    /// that saying so is not nagging — the same figure a grid's stack uses, and
    /// deliberately so: two thresholds for one sentence would read as two
    /// different warnings.
    pub const WORTH_MENTIONING: usize = 256 * 1024 * 1024;

    pub fn worth_compacting(&self) -> bool {
        self.bytes > Self::WORTH_MENTIONING
    }
}

/// Everything about a layer that is a hierarchy rather than a mesh.
///
/// Hung off [`crate::LayerSummary`] for the reason a grid's passes are nested
/// under the layer they were recorded on: none of it means anything apart from
/// the layer it belongs to, and a second panel elsewhere would have to repeat
/// which layer each row was about.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiresState {
    pub levels: MultiresLevels,
    /// The stack, bottom-first.
    pub sculpt_layers: Vec<MultiresSculptLayer>,
    /// Where the next stroke lands. [`MultiresSculptLayerId::BASE`] is the form
    /// under the passes, and is what an empty stack reads as.
    pub active_sculpt_layer: MultiresSculptLayerId,
    pub write_domain: WriteDomain,
}

impl MultiresState {
    /// A hierarchy freshly built from a cage: one level, no passes.
    pub fn just_the_cage() -> Self {
        Self {
            levels: MultiresLevels::just_the_cage(),
            sculpt_layers: Vec::new(),
            active_sculpt_layer: MultiresSculptLayerId::BASE,
            write_domain: WriteDomain::default(),
        }
    }

    pub fn sculpt_layer(&self, id: MultiresSculptLayerId) -> Option<&MultiresSculptLayer> {
        self.sculpt_layers.iter().find(|pass| pass.id == id)
    }

    /// The pass the next stroke would enter, where it is a pass.
    pub fn active(&self) -> Option<&MultiresSculptLayer> {
        self.sculpt_layer(self.active_sculpt_layer)
    }

    /// Which channel a stroke would enter right now, or `None` where it would
    /// be refused.
    pub fn stroke_lands_in(&self) -> Option<MultiresSculptLayerId> {
        self.write_domain.resolve(self.active_sculpt_layer)
    }

    /// What the passes contribute to the evaluated surface, as a **sum**.
    ///
    /// `E = B + Σ strength · mask(v) · pass(v)`, so this is a *set* of weighted
    /// terms and never a sequence. Answered in id order rather than stack order
    /// for exactly that reason: in draw order the two would agree today, and a
    /// caller folding them would be one refactor away from having built an
    /// ordering rule into a representation that does not have one.
    ///
    /// The per-vertex mask is not in the weight because it is per vertex. What
    /// is here is the part that is one number for the whole pass.
    pub fn composition(&self) -> Vec<(MultiresSculptLayerId, f32)> {
        let mut terms: Vec<(MultiresSculptLayerId, f32)> = self
            .sculpt_layers
            .iter()
            .map(|pass| {
                // Hidden is exactly zero rather than nearly zero, which is what
                // makes hiding a pass and comparing an exact test.
                let weight = if pass.visible { pass.strength } else { 0.0 };
                (pass.id, weight)
            })
            .collect();
        terms.sort_by_key(|(id, _)| *id);
        terms
    }

    /// The stack with one pass slid to a new position.
    ///
    /// **Organisation only.** Every pass keeps its id, its strength, its
    /// visibility, its lock and its coverage; what changes is `index`, which is
    /// draw order. `composition` is identical before and after, and that is
    /// asserted rather than assumed.
    ///
    /// A position past the end is the end, rather than a refusal: this is what
    /// a drag in a list means, and a drag released below the last row means the
    /// bottom.
    pub fn reordered(&self, id: MultiresSculptLayerId, to: usize) -> Self {
        let mut passes = self.sculpt_layers.clone();
        let Some(from) = passes.iter().position(|pass| pass.id == id) else {
            return self.clone();
        };
        let pass = passes.remove(from);
        passes.insert(to.min(passes.len()), pass);
        for (index, pass) in passes.iter_mut().enumerate() {
            pass.index = index;
        }
        Self {
            sculpt_layers: passes,
            ..self.clone()
        }
    }

    /// What the whole stack costs, and whether a gesture is holding it.
    pub fn cost(&self, stroke_open: bool) -> MultiresSculptLayerCost {
        MultiresSculptLayerCost {
            layers: self.sculpt_layers.len(),
            bytes: self.sculpt_layers.iter().map(|pass| pass.bytes).sum(),
            coverage_vertices: self
                .sculpt_layers
                .iter()
                .map(|pass| pass.coverage_vertices)
                .sum(),
            stroke_open,
        }
    }

    /// The levels and the strengths brought inside what they may be, and the
    /// draw order re-derived from the order the passes are held in.
    ///
    /// The active pass falls back to the form under the passes where it names
    /// one that is no longer there — a stack read back after a removal is
    /// exactly that case, and pointing a stroke at a pass that does not exist
    /// is the refusal this exists to avoid meeting.
    pub fn sanitized(mut self) -> Self {
        self.levels = self.levels.sanitized();
        for (index, pass) in self.sculpt_layers.iter_mut().enumerate() {
            pass.index = index;
            pass.strength = pass.strength.clamp(0.0, 1.0);
        }
        if !self.active_sculpt_layer.is_base()
            && !self
                .sculpt_layers
                .iter()
                .any(|pass| pass.id == self.active_sculpt_layer)
        {
            self.active_sculpt_layer = MultiresSculptLayerId::BASE;
        }
        self
    }
}

/// Something done to a hierarchy's levels.
///
/// Apart from [`MultiresSculptLayerOp`] because they are different questions
/// about different things: one is about how finely the surface is stored, the
/// other about the passes stacked on it. A sculptor can subdivide without ever
/// making a pass, and can make a dozen passes without ever subdividing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiresLevelOp {
    /// Moves where the brush writes. Moves nothing that is drawn.
    SetSculptLevel(u32),
    /// Moves what the viewport draws. Moves nothing that is stored.
    SetDisplayLevel(u32),
    /// One more level, with both numbers moved to it.
    AddLevel,
    /// The highest level and its detail, gone.
    RemoveHighestLevel,
}

impl MultiresLevelOp {
    pub const ALL: [MultiresLevelOp; 4] = [
        Self::SetSculptLevel(0),
        Self::SetDisplayLevel(0),
        Self::AddLevel,
        Self::RemoveHighestLevel,
    ];

    /// What the history calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::SetSculptLevel(_) => "sculpt level",
            Self::SetDisplayLevel(_) => "display level",
            Self::AddLevel => "subdivide",
            Self::RemoveHighestLevel => "remove the highest level",
        }
    }

    /// Whether the viewport has to redraw.
    ///
    /// **Choosing where to sculpt does not**, and that is the answer this
    /// predicate exists for. It is the surprising one — moving a level feels
    /// like it should show something — and it is the one that makes the two
    /// numbers worth keeping apart: a sculptor can drop to the cage to move a
    /// jaw while still watching the pores, and nothing re-meshes when they do.
    pub fn changes_what_is_drawn(self) -> bool {
        !matches!(self, Self::SetSculptLevel(_))
    }

    /// Whether the interface should confirm before sending it.
    ///
    /// One of the four. Removing the highest level takes the detail on it, and
    /// nothing left afterwards reconstructs what came off.
    pub fn is_destructive(self) -> bool {
        matches!(self, Self::RemoveHighestLevel)
    }

    /// Whether it costs memory that has to be priced first.
    ///
    /// Only the one that allocates. The others move a number.
    pub fn needs_pricing(self) -> bool {
        matches!(self, Self::AddLevel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(raw: u64, index: usize) -> MultiresSculptLayer {
        MultiresSculptLayer {
            id: MultiresSculptLayerId::new(raw),
            index,
            name: String::new(),
            strength: 1.0,
            visible: true,
            locked: false,
            masked: false,
            coverage_vertices: 100,
            bytes: 1024,
        }
    }

    fn stack() -> MultiresState {
        MultiresState {
            levels: MultiresLevels {
                count: 4,
                sculpt: 1,
                display: 3,
            },
            sculpt_layers: vec![pass(7, 0), pass(11, 1), pass(3, 2)],
            active_sculpt_layer: MultiresSculptLayerId::new(11),
            write_domain: WriteDomain::Automatic,
        }
    }

    // -- the two levels ------------------------------------------------------

    /// The property the whole type exists for: the two numbers move
    /// independently, and moving one leaves the other exactly where it was.
    #[test]
    fn where_the_brush_writes_and_what_is_drawn_move_apart() {
        let levels = MultiresLevels {
            count: 6,
            sculpt: 1,
            display: 5,
        };
        assert!(!levels.draws_what_it_sculpts());

        let coarser = levels.with_sculpt(0);
        assert_eq!(coarser.display, 5, "moving the brush moved the viewport");
        assert!(coarser.sculpting_the_cage());

        let nearer = levels.with_display(2);
        assert_eq!(nearer.sculpt, 1, "moving the viewport moved the brush");
        assert_eq!(nearer.display, 2);
    }

    /// And the interface can say so, which is what an artist needs when a dab
    /// lands softer than the brush ring suggested.
    #[test]
    fn a_hierarchy_can_say_it_is_not_drawing_what_it_sculpts() {
        let same = MultiresLevels {
            count: 3,
            sculpt: 2,
            display: 2,
        };
        assert!(same.draws_what_it_sculpts());
        assert!(!same.with_display(0).draws_what_it_sculpts());
    }

    /// Subdividing moves both, because that is what an artist means by it —
    /// they subdivide in order to work finer.
    #[test]
    fn subdividing_moves_both_numbers_to_the_new_level() {
        let levels = MultiresLevels {
            count: 3,
            sculpt: 0,
            display: 1,
        };
        let finer = levels.subdivided().expect("room for another level");
        assert_eq!(finer.count, 4);
        assert_eq!(finer.sculpt, 3);
        assert_eq!(finer.display, 3);
    }

    #[test]
    fn a_hierarchy_stops_at_the_depth_the_engine_stops_at() {
        let deep = MultiresLevels {
            count: MultiresLevels::DEEPEST,
            sculpt: 0,
            display: 0,
        };
        assert_eq!(
            deep.subdivided(),
            Err(Refusal::DepthLimit {
                levels: MultiresLevels::DEEPEST
            })
        );
        // One short of it still goes.
        let nearly = MultiresLevels {
            count: MultiresLevels::DEEPEST - 1,
            ..deep
        };
        assert!(nearly.subdivided().is_ok());
    }

    /// Dropping the top level brings both numbers back inside what is left,
    /// rather than leaving either pointing at a level that is gone.
    #[test]
    fn removing_a_level_brings_both_numbers_back_inside_the_hierarchy() {
        let levels = MultiresLevels {
            count: 4,
            sculpt: 3,
            display: 3,
        };
        let coarser = levels.coarsened().expect("something to remove");
        assert_eq!(coarser.count, 3);
        assert_eq!(coarser.sculpt, 2);
        assert_eq!(coarser.display, 2);

        assert_eq!(
            MultiresLevels::just_the_cage().coarsened(),
            None,
            "the cage is not a level to remove"
        );
    }

    #[test]
    fn a_hierarchy_read_back_short_a_level_clamps_rather_than_refusing() {
        let stale = MultiresLevels {
            count: 2,
            sculpt: 9,
            display: 5,
        }
        .sanitized();
        assert_eq!((stale.sculpt, stale.display), (1, 1));
        // And a count of zero is not a hierarchy; the cage is level 0.
        let empty = MultiresLevels {
            count: 0,
            sculpt: 0,
            display: 0,
        }
        .sanitized();
        assert_eq!(empty.count, 1);
        assert!(empty.is_only_the_cage());
        assert_eq!(empty.all(), vec![0]);
    }

    // -- what a level costs --------------------------------------------------

    /// Four faces per face per step, and the arithmetic saturates rather than
    /// wrapping — the failure mode of an unchecked multiply here is that the
    /// operation reads as *affordable*.
    ///
    /// The last case is past any depth the engine allows, and is the one that
    /// tells the two arithmetics apart: wrapping would answer with a number
    /// small enough to pass a budget check.
    #[test]
    fn a_face_count_quadruples_and_never_wraps_into_a_small_number() {
        assert_eq!(SubdivisionCost::faces_after(20_000, 0), 20_000);
        assert_eq!(SubdivisionCost::faces_after(20_000, 4), 5_120_000);
        assert_eq!(SubdivisionCost::faces_after(20_000, 5), 20_480_000);
        assert_eq!(
            SubdivisionCost::faces_after(u64::MAX / 2, 40),
            u64::MAX,
            "a face count that overflowed would report as affordable"
        );
    }

    /// A level is priced on its peak, because the peak is what ends a session.
    #[test]
    fn a_level_is_refused_on_what_it_holds_at_its_worst() {
        let level = SubdivisionCost {
            level: 5,
            vertices: 20_000_000,
            faces: 20_480_000,
            // Fits comfortably once it is built...
            persistent_bytes: 400 * 1024 * 1024,
            // ...and does not, while it is being built.
            peak_bytes: 900 * 1024 * 1024,
        };
        assert!(level.within(1024 * 1024 * 1024).is_ok());
        let error = level
            .within(512 * 1024 * 1024)
            .expect_err("the peak is past the budget");
        assert_eq!(
            error,
            Refusal::LevelOverBudget {
                peak_bytes: level.peak_bytes,
                budget_bytes: 512 * 1024 * 1024,
            }
        );
        assert!(
            level.peak_over_persistent() > 2.0,
            "the interface has to be able to say how much more the build holds"
        );
    }

    // -- the stack -----------------------------------------------------------

    /// The one property everything else about this stack rests on: sliding a
    /// pass through it changes organisation and not geometry.
    ///
    /// Held two ways, because either alone passes for the wrong reason. The
    /// composition is compared as a set of weighted terms — that is the sum the
    /// surface is — and every pass is compared field for field except its draw
    /// order, so a reorder that quietly dropped a strength or a lock fails here
    /// rather than looking like a successful reorder.
    #[test]
    fn sliding_a_pass_through_the_stack_moves_no_vertex() {
        let mut before = stack();
        before.sculpt_layers[0].strength = 0.25;
        before.sculpt_layers[1].visible = false;
        before.sculpt_layers[2].locked = true;

        let after = before.reordered(MultiresSculptLayerId::new(3), 0);

        assert_eq!(
            after.composition(),
            before.composition(),
            "a reorder changed what the passes sum to"
        );
        for pass in &before.sculpt_layers {
            let moved = after.sculpt_layer(pass.id).expect("the pass survived");
            assert_eq!(
                (
                    moved.strength,
                    moved.visible,
                    moved.locked,
                    moved.coverage_vertices
                ),
                (
                    pass.strength,
                    pass.visible,
                    pass.locked,
                    pass.coverage_vertices
                ),
                "the reorder changed pass {:?}",
                pass.id
            );
        }
        // And the draw order did move, or nothing was reordered at all.
        assert_eq!(
            after.sculpt_layers.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![
                MultiresSculptLayerId::new(3),
                MultiresSculptLayerId::new(7),
                MultiresSculptLayerId::new(11),
            ]
        );
        assert_eq!(
            after
                .sculpt_layers
                .iter()
                .map(|p| p.index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2],
            "draw order was not re-derived"
        );
    }

    /// And the op itself says so, which is what stops an interface treating a
    /// list drag as an edit to millions of vertices.
    #[test]
    fn a_reorder_is_not_a_geometry_edit_and_a_grids_reorder_is() {
        let reorder = MultiresSculptLayerOp::Move {
            id: MultiresSculptLayerId::new(7),
            to: 0,
        };
        assert!(!reorder.changes_the_surface());
        assert!(!reorder.is_destructive());

        // The contrast that makes the two types worth having. A grid's stack
        // replays cell writes, so its order IS the result and moving a pass
        // through it changes what is drawn.
        assert!(crate::SculptLayerOp::Move { from: 2, to: 0 }.changes_the_surface());
    }

    /// An id survives a reorder and a stack position does not — which is the
    /// whole reason this stack is addressed by one.
    #[test]
    fn an_id_names_the_same_pass_after_a_reorder_and_a_position_does_not() {
        let before = stack();
        let bottom = before.sculpt_layers[0].id;
        assert_eq!(before.sculpt_layer(bottom).map(|p| p.index), Some(0));

        let after = before.reordered(bottom, 2);
        assert_eq!(
            after.sculpt_layer(bottom).map(|p| p.id),
            Some(bottom),
            "the id stopped naming its pass"
        );
        assert_eq!(
            after.sculpt_layer(bottom).map(|p| p.index),
            Some(2),
            "position 0 now names a different pass, which is why nothing \
             addresses one by it"
        );
        assert_ne!(after.sculpt_layers[0].id, bottom);
    }

    /// A pass dialled to nothing and a pass hidden are the same contribution,
    /// and neither is "almost none".
    #[test]
    fn a_hidden_pass_contributes_exactly_nothing() {
        let mut state = stack();
        state.sculpt_layers[0].visible = false;
        state.sculpt_layers[1].strength = 0.0;

        let weights: Vec<f32> = state.composition().into_iter().map(|(_, w)| w).collect();
        assert!(weights.contains(&0.0));
        assert_eq!(weights.iter().filter(|w| **w == 0.0).count(), 2);
        assert!(!state.sculpt_layers[0].contributes());
        assert!(!state.sculpt_layers[1].contributes());
        assert!(state.sculpt_layers[2].contributes());
    }

    /// The base is not a pass, and nothing can mint an id that collides with
    /// it.
    #[test]
    fn the_form_under_the_passes_is_not_one_of_them() {
        assert!(MultiresSculptLayerId::BASE.is_base());
        assert_eq!(MultiresSculptLayerId::BASE.raw(), 0);
        assert!(!MultiresSculptLayerId::new(1).is_base());

        let empty = MultiresState::just_the_cage();
        assert!(empty.active().is_none());
        assert_eq!(
            empty.stroke_lands_in(),
            Some(MultiresSculptLayerId::BASE),
            "a stroke on a hierarchy with no passes goes into the form"
        );
    }

    /// Where a stroke lands is chosen, and the one combination with no answer
    /// is refused rather than quietly redirected.
    #[test]
    fn a_stroke_told_to_enter_a_pass_refuses_when_there_is_none() {
        let base = MultiresSculptLayerId::BASE;
        let pass = MultiresSculptLayerId::new(11);

        assert_eq!(WriteDomain::Automatic.resolve(base), Some(base));
        assert_eq!(WriteDomain::Automatic.resolve(pass), Some(pass));
        assert_eq!(WriteDomain::Geometry.resolve(pass), Some(base));
        assert_eq!(WriteDomain::Detail.resolve(pass), Some(pass));
        assert_eq!(
            WriteDomain::Detail.resolve(base),
            None,
            "asking for the pass and getting the form damages what the \
             sculptor said to leave alone"
        );

        let mut state = stack();
        state.write_domain = WriteDomain::Geometry;
        assert_eq!(state.stroke_lands_in(), Some(base));
    }

    /// A lock refuses a write and permits every property change, including the
    /// one that lifts it.
    #[test]
    fn a_lock_guards_the_coefficients_and_nothing_else() {
        let id = MultiresSculptLayerId::new(11);
        for op in [
            MultiresSculptLayerOp::MergeDown { id },
            MultiresSculptLayerOp::BakeToBase { id },
        ] {
            assert!(op.refused_by_a_lock(), "{}", op.label());
        }
        for op in [
            MultiresSculptLayerOp::Rename {
                id,
                name: "Pele".into(),
            },
            MultiresSculptLayerOp::SetStrength { id, strength: 0.4 },
            MultiresSculptLayerOp::SetVisible { id, visible: false },
            MultiresSculptLayerOp::SetLocked { id, locked: false },
            MultiresSculptLayerOp::SetActive { id },
            MultiresSculptLayerOp::Move { id, to: 0 },
        ] {
            assert!(
                !op.refused_by_a_lock(),
                "{} is refused by a lock, so a locked pass cannot be dialled, \
                 renamed or unlocked from its own row",
                op.label()
            );
        }
        let locked = MultiresSculptLayer {
            locked: true,
            ..pass(11, 0)
        };
        assert!(!locked.accepts_a_stroke());
    }

    /// The three composition controls that stay live during a gesture are the
    /// three that move no vertex.
    #[test]
    fn only_what_moves_nothing_is_allowed_through_an_open_stroke() {
        let id = MultiresSculptLayerId::new(7);
        let allowed = [
            MultiresSculptLayerOp::Rename {
                id,
                name: "Rugas".into(),
            },
            MultiresSculptLayerOp::SetLocked { id, locked: true },
            MultiresSculptLayerOp::SetActive { id },
        ];
        for op in &allowed {
            assert!(!op.needs_the_stroke_closed(), "{}", op.label());
            assert!(!op.changes_the_surface(), "{}", op.label());
        }
        for op in [
            MultiresSculptLayerOp::Add {
                name: String::new(),
            },
            MultiresSculptLayerOp::SetStrength { id, strength: 0.5 },
            MultiresSculptLayerOp::SetVisible { id, visible: false },
            MultiresSculptLayerOp::Move { id, to: 0 },
            MultiresSculptLayerOp::Remove { id },
            MultiresSculptLayerOp::MergeDown { id },
            MultiresSculptLayerOp::BakeToBase { id },
            MultiresSculptLayerOp::Compact,
        ] {
            assert!(op.needs_the_stroke_closed(), "{}", op.label());
        }
    }

    /// "Changes the surface" and "needs confirming" are two questions, and they
    /// disagree in both directions.
    #[test]
    fn a_merge_leaves_the_surface_alone_and_still_cannot_be_undone_by_hand() {
        let id = MultiresSculptLayerId::new(3);
        let merge = MultiresSculptLayerOp::MergeDown { id };
        assert!(!merge.changes_the_surface(), "a merge is defined by parity");
        assert!(
            merge.is_destructive(),
            "and it takes a slider away for good"
        );

        let dial = MultiresSculptLayerOp::SetStrength { id, strength: 0.5 };
        assert!(dial.changes_the_surface());
        assert!(!dial.is_destructive());
    }

    /// Every operation is named distinctly, so a history entry says which one
    /// it was.
    #[test]
    fn every_pass_operation_has_its_own_name() {
        let id = MultiresSculptLayerId::new(1);
        let all = [
            MultiresSculptLayerOp::Add {
                name: String::new(),
            },
            MultiresSculptLayerOp::Rename {
                id,
                name: String::new(),
            },
            MultiresSculptLayerOp::SetStrength { id, strength: 1.0 },
            MultiresSculptLayerOp::SetVisible { id, visible: true },
            MultiresSculptLayerOp::SetLocked { id, locked: true },
            MultiresSculptLayerOp::SetActive { id },
            MultiresSculptLayerOp::Move { id, to: 0 },
            MultiresSculptLayerOp::Remove { id },
            MultiresSculptLayerOp::MergeDown { id },
            MultiresSculptLayerOp::BakeToBase { id },
            MultiresSculptLayerOp::Compact,
        ];
        let names: std::collections::BTreeSet<&str> = all.iter().map(|op| op.label()).collect();
        assert_eq!(names.len(), all.len(), "two operations share a name");
        // And every one that names a pass says which.
        for op in &all {
            let named = op.targets().is_some();
            let expected = !matches!(
                op,
                MultiresSculptLayerOp::Add { .. } | MultiresSculptLayerOp::Compact
            );
            assert_eq!(named, expected, "{}", op.label());
        }
    }

    /// A stack read back after a removal does not leave a stroke aimed at a
    /// pass that is gone.
    #[test]
    fn an_active_pass_that_is_no_longer_there_falls_back_to_the_form() {
        let mut state = stack();
        state.sculpt_layers.retain(|pass| pass.id.raw() != 11);
        let settled = state.sanitized();
        assert_eq!(settled.active_sculpt_layer, MultiresSculptLayerId::BASE);
        assert_eq!(settled.stroke_lands_in(), Some(MultiresSculptLayerId::BASE));
        // Draw order was re-derived over what is left.
        assert_eq!(
            settled
                .sculpt_layers
                .iter()
                .map(|p| p.index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn a_strength_outside_its_range_is_brought_back_inside_it() {
        let mut state = stack();
        state.sculpt_layers[0].strength = 4.0;
        state.sculpt_layers[1].strength = -1.0;
        let settled = state.sanitized();
        assert_eq!(settled.sculpt_layers[0].strength, 1.0);
        assert_eq!(settled.sculpt_layers[1].strength, 0.0);
    }

    #[test]
    fn a_pass_with_no_name_is_counted_from_one() {
        let unnamed = pass(42, 2);
        assert_eq!(unnamed.display_name(), "Passe 3");
        let named = MultiresSculptLayer {
            name: "Poros".into(),
            ..pass(42, 2)
        };
        assert_eq!(named.display_name(), "Poros");
    }

    #[test]
    fn the_stack_reports_what_it_costs_and_when_it_is_worth_acting_on() {
        let state = stack();
        let cost = state.cost(false);
        assert_eq!(cost.layers, 3);
        assert_eq!(cost.bytes, 3 * 1024);
        assert_eq!(cost.coverage_vertices, 300);
        assert!(!cost.stroke_open);
        assert!(!cost.worth_compacting());

        let heavy = MultiresSculptLayerCost {
            bytes: MultiresSculptLayerCost::WORTH_MENTIONING + 1,
            ..cost
        };
        assert!(heavy.worth_compacting());
        // A whole megabyte past the threshold rather than a byte past it: the
        // readout is an `f32` and a byte does not survive the division.
        let heavier = MultiresSculptLayerCost {
            bytes: MultiresSculptLayerCost::WORTH_MENTIONING + 4 * 1024 * 1024,
            ..cost
        };
        assert!(heavier.megabytes() > 256.0, "{}", heavier.megabytes());
    }

    /// A hierarchy's stack has no recording mode, and the cost says so by not
    /// having the field.
    ///
    /// Held as a test rather than only as a comment because the temptation is
    /// to copy `SculptLayerCost` across: a grid's passes are opened and closed,
    /// and these are chosen. What replaces "recording" is "a stroke is open",
    /// which is a different fact — it is why the sliders are refusing, not
    /// where the next edit is filed.
    #[test]
    fn a_stroke_being_open_is_not_a_recording_mode() {
        let state = stack();
        assert!(state.cost(true).stroke_open);
        // Whether a stroke is open changes nothing about where an edit lands;
        // that is the active pass and the write domain, which stand on their
        // own.
        assert_eq!(
            state.cost(true).layers,
            state.cost(false).layers,
            "the stack is the same stack whether or not a gesture is open"
        );
        assert_eq!(
            state.stroke_lands_in(),
            Some(MultiresSculptLayerId::new(11))
        );
    }

    // -- the level operations ------------------------------------------------

    /// Choosing where to sculpt redraws nothing. The one answer here worth a
    /// test of its own.
    #[test]
    fn moving_the_brush_to_another_level_redraws_nothing() {
        assert!(!MultiresLevelOp::SetSculptLevel(2).changes_what_is_drawn());
        for op in [
            MultiresLevelOp::SetDisplayLevel(2),
            MultiresLevelOp::AddLevel,
            MultiresLevelOp::RemoveHighestLevel,
        ] {
            assert!(op.changes_what_is_drawn(), "{}", op.label());
        }
    }

    #[test]
    fn only_the_level_that_allocates_is_priced_and_only_the_one_that_destroys_is_confirmed() {
        let priced: Vec<&str> = MultiresLevelOp::ALL
            .into_iter()
            .filter(|op| op.needs_pricing())
            .map(|op| op.label())
            .collect();
        assert_eq!(priced, vec!["subdivide"]);

        let confirmed: Vec<&str> = MultiresLevelOp::ALL
            .into_iter()
            .filter(|op| op.is_destructive())
            .map(|op| op.label())
            .collect();
        assert_eq!(confirmed, vec!["remove the highest level"]);
    }

    #[test]
    fn every_level_operation_has_its_own_name() {
        let names: std::collections::BTreeSet<&str> =
            MultiresLevelOp::ALL.iter().map(|op| op.label()).collect();
        assert_eq!(names.len(), MultiresLevelOp::ALL.len());
    }
}
