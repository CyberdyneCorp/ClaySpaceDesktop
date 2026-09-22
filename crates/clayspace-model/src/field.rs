//! What the field can hold, and what asking it to hold more would cost.
//!
//! A crossing is priced before it runs — [`crate::Cost::within`] refuses a
//! resolution whose grid does not fit the budget — and a hierarchy's next
//! level is priced the same way against
//! [`crate::SubdivisionCost::within`]. The two commands that can multiply a
//! document's size by an order of magnitude in one go were not priced at all:
//! a shape's parameters were capped at a fixed 10.0 that stood for nothing,
//! and a curve's radius had no upper bound whatsoever. So the cheapest thing
//! to type was the most expensive thing to run, and the application found out
//! by running it — millions of triangles from one insert, tens of seconds and
//! gigabytes from one radius.
//!
//! This is the price, and it is the brick cache's, because the brick cache is
//! what the viewport draws from. An edit dirties a region, every brick in that
//! region is visited, and every brick that turns out to hold surface is
//! stored. So the honest bound on a *single* edit is the cache itself: a
//! region the cache could not hold even if it were all surface is a region the
//! cache cannot finish, and no sculptor asked for that.

/// How the brick cache is laid out, and what it may spend.
///
/// Carried as data rather than read from the engine, for the reason every
/// other number in this crate is carried: the domain may not depend on the
/// engine. `clayspace-engine` builds one of these from the cache it actually
/// created, and `a_budget_describes_the_cache_it_prices` over there is what
/// keeps [`FieldBudget::DEFAULT`] and that cache's tuning in step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldBudget {
    /// World units between lattice samples.
    pub voxel_size: f32,
    /// Lattice samples per brick axis.
    pub brick_dim: u32,
    /// Bytes of brick payload the cache may hold.
    pub budget_bytes: u64,
}

impl FieldBudget {
    /// How every document's cache is tuned, as the static parameter tables
    /// have to assume it before any document exists.
    ///
    /// A placed shape is priced against the cache the document actually
    /// carries; a shape *parameter*'s range is offered by a panel that may be
    /// drawn before one is open, so it is bounded by this.
    pub const DEFAULT: Self = Self {
        voxel_size: 0.02,
        brick_dim: 8,
        budget_bytes: 512 * 1024 * 1024,
    };

    /// Bytes one lattice sample costs, for the budget refusal.
    ///
    /// A distance, and the bookkeeping around it. Approximate on purpose, the
    /// same way `BYTES_PER_CELL` is on the conversion path: it decides whether
    /// to refuse a region, not what to allocate.
    const BYTES_PER_SAMPLE: u64 = 4;

    /// How far a brick's lattice is padded on each side when it is filled.
    ///
    /// One. A brick meshed without an apron does not join the brick beside it,
    /// so what a refill actually materializes is the padded lattice and not
    /// the bare one — `BrickConfig::samples_per_brick` over in `claycore` is
    /// the same arithmetic, asked by the callers that read brick data back.
    /// Pricing the bare lattice would under-count every brick by more than
    /// half, which on this path is the direction that loses sessions.
    const APRON: u64 = 1;

    /// How far one brick reaches, in world units.
    pub fn brick_span(&self) -> f32 {
        self.voxel_size.max(f32::EPSILON) * self.brick_dim.max(1) as f32
    }

    /// What one brick's padded lattice costs to fill.
    pub fn bytes_per_brick(&self) -> u64 {
        let side = u64::from(self.brick_dim.max(1)) + 2 * Self::APRON;
        side * side * side * Self::BYTES_PER_SAMPLE
    }

    /// How many bricks the cache would hold if every one of them held surface.
    ///
    /// The worst case, deliberately, because that is what a bound on a single
    /// edit is for: an edit whose region *could* come back all surface is one
    /// the cache might be asked to hold entirely, and the sculptor finds out
    /// which after the minutes have been spent.
    pub fn budget_bricks(&self) -> u64 {
        (self.budget_bytes / self.bytes_per_brick()).max(1)
    }

    /// How many bricks a region of this size covers.
    ///
    /// An extent that is not a finite number answers the largest count there
    /// is, so a NaN radius that survived every other check is refused here
    /// rather than counted as nothing — `as u64` reads NaN as zero, which
    /// would make the most dangerous input look like the cheapest.
    pub fn bricks(&self, extent: [f32; 3]) -> u64 {
        let span = self.brick_span();
        extent.iter().fold(1u64, |bricks, reach| {
            if !reach.is_finite() {
                return u64::MAX;
            }
            bricks.saturating_mul((reach.max(0.0) / span).ceil().max(1.0) as u64)
        })
    }

    /// Whether a region of this size is one the cache could hold.
    pub fn within(&self, extent: [f32; 3]) -> Result<(), FieldRefusal> {
        let bricks = self.bricks(extent);
        let budget_bricks = self.budget_bricks();
        if bricks > budget_bricks {
            return Err(FieldRefusal::RegionOverBudget {
                bricks,
                budget_bricks,
            });
        }
        Ok(())
    }

    /// The largest half-extent a cube-shaped region may reach.
    ///
    /// What bounds a shape parameter, since every parameter this application
    /// offers is a half-extent or a radius. Floored to a whole number of
    /// bricks so the bound is a size the cache can actually be asked for
    /// rather than one that rounds up past itself.
    pub fn largest_half_extent(&self) -> f32 {
        let across = (self.budget_bricks() as f64).cbrt().floor() as f32;
        across * self.brick_span() * 0.5
    }
}

/// Why an edit the field would have to carry was refused.
///
/// Its own vocabulary rather than [`crate::Refusal`], which states why a
/// *crossing* was refused: a sculptor told "that resolution needs too many
/// cells" goes to the resolution control, and there is no resolution control
/// on a shape picker. What they are being told here is that the form itself is
/// larger than the document can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRefusal {
    /// The region the edit would fill is larger than the cache could hold.
    RegionOverBudget { bricks: u64, budget_bricks: u64 },
}

impl std::fmt::Display for FieldRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RegionOverBudget {
                bricks,
                budget_bricks,
            } => write!(
                f,
                "that fills {bricks} blocks of the field, past the \
                 {budget_bricks} this document can hold"
            ),
        }
    }
}

/// A number brought inside what the field can hold, and what was asked for.
///
/// Reported rather than swallowed. A control that answers a different number
/// than it was handed, and says nothing about it, is one a sculptor sets
/// again — and an agent reading the state back finds a value it never typed
/// with nothing to say where it came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clamped {
    /// Which number this is, by the stable key a panel looks it up under.
    pub key: &'static str,
    pub asked: f32,
    pub used: f32,
}

impl Clamped {
    /// Whether the bound actually changed the answer.
    ///
    /// A NaN asked for reads as moved, which is right: it was replaced by the
    /// default, and that is exactly the substitution a sculptor needs told.
    pub fn moved(&self) -> bool {
        self.asked != self.used
    }
}

impl std::fmt::Display for Clamped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { key, asked, used } = self;
        write!(f, "{key} reaches {used} here, not the {asked} asked for")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tuning every other number here is derived from, stated once so a
    /// reader can check the arithmetic in the comments against it.
    #[test]
    fn the_default_budget_is_the_cache_it_describes() {
        let budget = FieldBudget::DEFAULT;
        assert_eq!(budget.bytes_per_brick(), 10 * 10 * 10 * 4);
        assert_eq!(budget.budget_bricks(), 134_217);
        assert!((budget.brick_span() - 0.16).abs() < 1e-6);
        assert!((budget.largest_half_extent() - 4.08).abs() < 1e-3);
    }

    #[test]
    fn a_region_the_cache_can_hold_is_allowed_and_a_larger_one_is_not() {
        let budget = FieldBudget::DEFAULT;
        let largest = budget.largest_half_extent();
        assert_eq!(budget.within([largest * 2.0; 3]), Ok(()));
        assert!(matches!(
            budget.within([largest * 2.0 + budget.brick_span(); 3]),
            Err(FieldRefusal::RegionOverBudget { .. })
        ));
    }

    /// A thin, tall form is priced per axis rather than as the cube of its
    /// longest side, which is what makes a long thin cylinder affordable.
    #[test]
    fn a_region_is_priced_per_axis() {
        let budget = FieldBudget::DEFAULT;
        let tall = [0.2, 10.0, 0.2];
        assert!(budget.bricks(tall) < budget.bricks([10.0; 3]));
        assert_eq!(budget.within(tall), Ok(()));
    }

    /// `as u64` reads NaN as zero, so a number that is not a number has to be
    /// caught before the cast rather than after it.
    #[test]
    fn a_region_that_is_not_a_number_is_refused() {
        let budget = FieldBudget::DEFAULT;
        assert!(budget.within([f32::NAN, 1.0, 1.0]).is_err());
        assert!(budget.within([f32::INFINITY, 1.0, 1.0]).is_err());
    }

    /// The smallest region is one brick and not none: a flat region is still
    /// somewhere the cache has to look.
    #[test]
    fn an_empty_region_still_costs_a_brick() {
        assert_eq!(FieldBudget::DEFAULT.bricks([0.0; 3]), 1);
    }
}
