//! How one edit meets the surface already there, and how softly.
//!
//! An SDF layer is an ordered list of edits, and every one of them combines
//! with what is beneath it. Which way it combines and how sharply the join is
//! made are two separate choices — a groove made with a hard blend and the
//! same groove made with a circular one are the same *operation* and different
//! shapes — so they are two controls rather than one list of their product.
//!
//! Named here rather than borrowed from the engine because the domain may not
//! depend on it; the adapter is where these become calls, exactly as the tool
//! table's verbs are.

/// How an edit meets what is under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Combine {
    /// Union: the two become one shape.
    #[default]
    Add,
    /// The edit is taken away from what is there.
    Subtract,
    /// Only where both are.
    Intersect,
    /// Colour without moving the surface.
    Paint,
    /// A rounded channel where the two meet.
    Groove,
    /// The ridge a groove would have cut, kept instead.
    Tongue,
    /// A round bead along the join.
    Pipe,
    /// Cut in, as a deboss.
    Engrave,
    /// Raised, as an emboss.
    Emboss,
    /// Set into the surface, with a lip.
    Inset,
    /// Hollowed to a wall of its own thickness.
    Shell,
    /// Replaces what is under it rather than combining.
    Replace,
    /// Displaces the accumulated surface along its normal — ZBrush's Standard
    /// and ClayBuildup, and what an ordinary stroke uses.
    Relief,
    /// The same, cutting in: a thin region gives the line — Crease and
    /// DamStandard.
    Incise,
}

impl Combine {
    pub const ALL: [Combine; 14] = [
        Self::Add,
        Self::Subtract,
        Self::Intersect,
        Self::Paint,
        Self::Groove,
        Self::Tongue,
        Self::Pipe,
        Self::Engrave,
        Self::Emboss,
        Self::Inset,
        Self::Shell,
        Self::Replace,
        Self::Relief,
        Self::Incise,
    ];

    /// The operations the options bar offers for a stroke on a field.
    ///
    /// Every entry of [`Self::ALL`] except Paint, which is real in the engine
    /// and cannot work here — now for one reason rather than two. There *is* a
    /// brush colour (see [`crate::colour`]), and it reaches the two
    /// representations that carry one; what is still missing is the other
    /// half. The brick cache meshes the surface with colours off and nothing
    /// in the surface path carries a colour to the GPU, so what a Paint stroke
    /// wrote would not be drawn. Measured at four radii, the surface under one
    /// does not move and no pixel changes. Offering it would be offering a
    /// control that silently does nothing, so it is left out until the colour
    /// reaches the rendered geometry; `Combine::Paint` stays in the vocabulary
    /// because the operation exists and the mapping onto the engine has to
    /// stay complete.
    pub fn offered_for_strokes() -> Vec<Combine> {
        Self::ALL
            .into_iter()
            .filter(|op| *op != Self::Paint)
            .collect()
    }

    /// The label the interface shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Add => "Unir",
            Self::Subtract => "Subtrair",
            Self::Intersect => "Interseção",
            Self::Paint => "Pintar",
            Self::Groove => "Sulco",
            Self::Tongue => "Lingueta",
            Self::Pipe => "Tubo",
            Self::Engrave => "Gravar",
            Self::Emboss => "Relevar",
            Self::Inset => "Embutir",
            Self::Shell => "Casca",
            Self::Replace => "Substituir",
            Self::Relief => "Relevo",
            Self::Incise => "Incisar",
        }
    }

    /// A stable name, for anything that has to write an operation down.
    ///
    /// Not [`Combine::label`], which is interface text and translated.
    pub fn key(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Intersect => "intersect",
            Self::Paint => "paint",
            Self::Groove => "groove",
            Self::Tongue => "tongue",
            Self::Pipe => "pipe",
            Self::Engrave => "engrave",
            Self::Emboss => "emboss",
            Self::Inset => "inset",
            Self::Shell => "shell",
            Self::Replace => "replace",
            Self::Relief => "relief",
            Self::Incise => "incise",
        }
    }

    /// The operation a key names, if it names one.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|op| op.key() == key)
    }

    /// Whether the operation moves the surface at all.
    ///
    /// Paint does not, which is what makes it the one that can be applied to a
    /// finished form without reshaping it.
    pub fn moves_the_surface(self) -> bool {
        self != Self::Paint
    }

    /// Whether the operation displaces the accumulated surface along its own
    /// normal rather than combining a shape with it.
    ///
    /// This is what an ordinary sculpting stroke does, and it changes what the
    /// blend radius *means*: for these the engine reads it as the amplitude the
    /// surface moves by, and for every other op it is the width of the join.
    /// One number, two meanings, so the interface has to label it differently —
    /// and the adapter has to fill it differently.
    pub fn displaces_along_the_normal(self) -> bool {
        matches!(self, Self::Relief | Self::Incise)
    }

    /// Whether the operation does nothing at all without a distance.
    ///
    /// For most operations the radius rounds a join that would otherwise be
    /// sharp, and zero means a hard join. For these seven it *is* the effect —
    /// a groove is the channel where the item meets the surface given a width,
    /// an engrave is a cut given a depth, a shell is a wall given a thickness.
    /// Measured against the starting form with a 0.28 brush, taking the height
    /// of the surface under the stroke:
    ///
    /// | operation | r=0 | r=0.05 | r=0.15 |
    /// |-----------|-----|--------|--------|
    /// | Engrave   | −0.0000 | −0.0470 | −0.2371 |
    /// | Emboss    | +0.0002 | +0.0475 | +0.1400 |
    /// | Shell     | +0.0008 | +0.2033 | +0.3047 |
    ///
    /// Groove, Tongue, Pipe and Inset move nothing whatever at zero. So for
    /// these the slider must not be able to reach zero: that is not a hard
    /// join, it is no operation, and a sculptor who lands there sees a tool
    /// that appears broken with nothing to say why.
    pub fn needs_a_distance(self) -> bool {
        matches!(
            self,
            Self::Groove
                | Self::Tongue
                | Self::Pipe
                | Self::Inset
                | Self::Engrave
                | Self::Emboss
                | Self::Shell
        )
    }

    /// Whether a blend profile changes anything for this operation.
    ///
    /// A profile describes how a *join* is rounded. Replace makes no join —
    /// it discards what was there — and Paint touches no surface to join, so
    /// offering a profile beside either is offering a control that does
    /// nothing.
    /// The same operation with its sign turned over, where that means
    /// anything.
    ///
    /// What a sculptor holding the invert modifier expects: the brush takes
    /// material away rather than putting it there. Add and Subtract are each
    /// other's answer; Emboss raises where Engrave cuts, and Tongue fills
    /// where Groove hollows.
    ///
    /// `None` where the operation has no opposite to offer — Intersect,
    /// Replace and Paint say nothing about adding or removing, and inverting
    /// one would have to invent a meaning rather than turn one over. The
    /// stroke is then left as it is rather than quietly becoming a different
    /// verb.
    pub fn inverted(self) -> Option<Self> {
        Some(match self {
            Self::Add => Self::Subtract,
            Self::Subtract => Self::Add,
            Self::Engrave => Self::Emboss,
            Self::Emboss => Self::Engrave,
            Self::Groove => Self::Tongue,
            Self::Tongue => Self::Groove,
            Self::Incise => Self::Relief,
            Self::Relief => Self::Incise,
            // Pipe and Inset are seams rather than deposits, and Shell is a
            // thickness; none of them has an opposite that is still itself.
            Self::Intersect
            | Self::Replace
            | Self::Paint
            | Self::Pipe
            | Self::Inset
            | Self::Shell => return None,
        })
    }

    pub fn takes_a_blend(self) -> bool {
        !matches!(self, Self::Replace | Self::Paint)
    }
}

/// How sharply the join between two shapes is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlendProfile {
    /// No rounding: the join is the exact minimum of the two.
    #[default]
    Hard,
    Quadratic,
    Cubic,
    Circular,
    /// A flat bevel rather than a round one.
    Chamfer,
}

impl BlendProfile {
    /// A stable name, on the same terms as [`Combine::key`].
    pub fn key(self) -> &'static str {
        match self {
            Self::Hard => "hard",
            Self::Quadratic => "quadratic",
            Self::Cubic => "cubic",
            Self::Circular => "circular",
            Self::Chamfer => "chamfer",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|profile| profile.key() == key)
    }

    pub const ALL: [BlendProfile; 5] = [
        Self::Hard,
        Self::Quadratic,
        Self::Cubic,
        Self::Circular,
        Self::Chamfer,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Hard => "Dura",
            Self::Quadratic => "Quadrática",
            Self::Cubic => "Cúbica",
            Self::Circular => "Circular",
            Self::Chamfer => "Chanfro",
        }
    }
}

/// What an SDF edit is set to combine with.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CombineSettings {
    pub op: Combine,
    pub blend: BlendProfile,
    /// How wide the blend reaches, in document units. Zero is a hard join
    /// whatever the profile says.
    pub radius: f32,
}

impl CombineSettings {
    /// What a stroke starts as.
    ///
    /// Relief rather than the `Default` union: a brush dragged over a surface
    /// raises it, and adding a *sphere* at every stamp is what a union would
    /// do. `Default` stays Add because that is what placing a shape means, and
    /// the two contexts genuinely want different starting points.
    pub fn for_strokes() -> Self {
        Self {
            op: Combine::Relief,
            blend: BlendProfile::Quadratic,
            radius: 0.0,
        }
    }

    /// What the radius control is called, given the operation.
    ///
    /// It is the same number in both cases and it is not the same quantity,
    /// so a single fixed label would be wrong half the time.
    pub fn radius_label(self) -> &'static str {
        if self.op.displaces_along_the_normal() {
            "Amplitude"
        } else {
            "Suavidade"
        }
    }

    /// What a blend radius may be.
    ///
    /// Bounded because the join is a distance and an unbounded one swallows
    /// the shapes it is joining.
    pub const RADIUS_RANGE: std::ops::RangeInclusive<f32> = 0.0..=0.5;

    /// The smallest distance worth giving one of those operations, in
    /// document units.
    ///
    /// Below this the channel a groove cuts is thinner than a cell and the
    /// operation reads as broken rather than subtle. Measured, an engrave at
    /// 0.05 moves the surface by 0.047 — plainly visible — so this is well
    /// inside the range that does something.
    pub const MIN_DISTANCE: f32 = 0.02;

    /// What the radius may be, given the operation.
    ///
    /// Narrower than [`Self::RADIUS_RANGE`] for the operations whose whole
    /// effect is the distance they are given: zero there is not a hard join,
    /// it is no operation at all, and the slider should not express it.
    pub fn radius_range(self) -> std::ops::RangeInclusive<f32> {
        if self.op.needs_a_distance() {
            Self::MIN_DISTANCE..=*Self::RADIUS_RANGE.end()
        } else {
            Self::RADIUS_RANGE
        }
    }

    pub fn sanitized(mut self) -> Self {
        let range = self.radius_range();
        self.radius = self.radius.clamp(*range.start(), *range.end());
        if !self.op.takes_a_blend() {
            self.radius = 0.0;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operation_has_a_distinct_label() {
        for (i, a) in Combine::ALL.iter().enumerate() {
            for b in Combine::ALL.iter().skip(i + 1) {
                assert_ne!(a.label(), b.label(), "two operations share a label");
            }
        }
    }

    /// A profile describes how a join is rounded, so an operation that makes
    /// no join must not be offered one.
    #[test]
    fn the_operations_that_make_no_join_take_no_blend() {
        assert!(
            !Combine::Replace.takes_a_blend(),
            "replace discards the join"
        );
        assert!(!Combine::Paint.takes_a_blend(), "paint touches no surface");
        for op in Combine::ALL {
            if op != Combine::Replace && op != Combine::Paint {
                assert!(op.takes_a_blend(), "{} makes a join", op.label());
            }
        }
    }

    /// And a radius set before choosing such an operation is dropped rather
    /// than carried invisibly into an edit it cannot affect.
    #[test]
    fn a_radius_is_dropped_where_it_would_do_nothing() {
        let settings = CombineSettings {
            op: Combine::Replace,
            blend: BlendProfile::Circular,
            radius: 0.2,
        }
        .sanitized();
        assert_eq!(settings.radius, 0.0);
    }

    #[test]
    fn a_radius_is_kept_where_it_bites() {
        let settings = CombineSettings {
            op: Combine::Groove,
            blend: BlendProfile::Circular,
            radius: 0.2,
        }
        .sanitized();
        assert_eq!(settings.radius, 0.2);
    }

    /// The seven whose whole effect is the distance move nothing at a radius
    /// of zero — measured, not assumed — so the setting they arrive with has
    /// to be one that does something.
    #[test]
    fn a_distance_driven_operation_cannot_be_set_to_do_nothing() {
        for op in Combine::ALL.into_iter().filter(|op| op.needs_a_distance()) {
            let settings = CombineSettings {
                op,
                radius: 0.0,
                ..Default::default()
            }
            .sanitized();
            assert!(
                settings.radius >= CombineSettings::MIN_DISTANCE,
                "{} arrived at a radius of {}, which cuts nothing",
                op.label(),
                settings.radius
            );
        }
    }

    /// And the ops that are a shape rather than a join keep the hard zero,
    /// because for them it means a hard join rather than nothing at all.
    #[test]
    fn a_shape_operation_keeps_its_hard_join() {
        let settings = CombineSettings {
            op: Combine::Subtract,
            radius: 0.0,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(settings.radius, 0.0);
    }

    #[test]
    fn a_radius_is_bounded() {
        let wide = CombineSettings {
            op: Combine::Add,
            radius: 50.0,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(wide.radius, *CombineSettings::RADIUS_RANGE.end());
    }

    /// A stroke that starts as a union stamps spheres along the path instead
    /// of raising the surface — the bug the tool table already carries a note
    /// about, in the one place a default could reintroduce it.
    #[test]
    fn a_stroke_starts_displacing_rather_than_adding() {
        assert_eq!(CombineSettings::for_strokes().op, Combine::Relief);
        assert!(CombineSettings::for_strokes()
            .op
            .displaces_along_the_normal());
    }

    /// The radius is an amplitude for one family and a join width for the
    /// other, so the label has to follow the operation.
    #[test]
    fn the_radius_is_named_for_what_it_measures() {
        let relief = CombineSettings {
            op: Combine::Relief,
            ..Default::default()
        };
        let groove = CombineSettings {
            op: Combine::Groove,
            ..Default::default()
        };
        assert_ne!(relief.radius_label(), groove.radius_label());
        assert_eq!(relief.radius_label(), "Amplitude");
    }

    #[test]
    fn only_the_two_displacing_ops_move_along_the_normal() {
        for op in Combine::ALL {
            assert_eq!(
                op.displaces_along_the_normal(),
                matches!(op, Combine::Relief | Combine::Incise),
                "{op:?}"
            );
        }
    }

    /// A control that cannot work is not offered — and the reason it is not
    /// is that there is nothing to paint *with*, so this has to be revisited
    /// the day a brush colour exists rather than left as a permanent hole.
    #[test]
    fn paint_is_not_offered_where_it_has_no_colour_to_deposit() {
        let offered = Combine::offered_for_strokes();
        assert!(!offered.contains(&Combine::Paint));
        assert_eq!(
            offered.len(),
            Combine::ALL.len() - 1,
            "something other than paint was dropped from the shelf"
        );
    }

    #[test]
    fn only_paint_leaves_the_surface_alone() {
        for op in Combine::ALL {
            assert_eq!(op.moves_the_surface(), op != Combine::Paint, "{op:?}");
        }
    }
}

/// What a held modifier does to the stroke about to be made.
///
/// Sampled when the press lands and held for the gesture, as ZBrush and
/// Blender both do: a modifier caught and released mid-drag would change the
/// verb under the sculptor's hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StrokeModifiers {
    /// Smooth instead, whatever tool is selected.
    ///
    /// Shift, in both references. It is a *substitution* rather than a setting:
    /// the shelf still shows the tool that was chosen, because letting go of
    /// the key returns to it.
    pub smooth: bool,
    /// Take material away rather than put it there.
    ///
    /// Blender's Ctrl. ZBrush spells it Alt, which this application cannot:
    /// Alt already forces the drag to orbit, which is ZBrush's own rule and
    /// the one that leaves a trackpad able to turn the model.
    pub invert: bool,
}

impl StrokeModifiers {
    /// Whether either of them is asking for anything.
    pub fn any(self) -> bool {
        self.smooth || self.invert
    }

    /// The tool a stroke actually uses.
    ///
    /// Smoothing wins over inverting where both are held: an inverted smooth
    /// is not a thing either reference offers, and sharpening is a different
    /// verb rather than a smooth with its sign turned over.
    pub fn tool(self, chosen: crate::ToolKind) -> crate::ToolKind {
        if self.smooth {
            crate::ToolKind::Suavizar
        } else {
            chosen
        }
    }
}

#[cfg(test)]
mod modifier_tests {
    use super::*;

    /// Turning an operation over twice is where it started.
    #[test]
    fn inverting_twice_is_the_operation_again() {
        for op in Combine::ALL {
            let Some(other) = op.inverted() else { continue };
            assert_eq!(
                other.inverted(),
                Some(op),
                "{op:?} inverts to {other:?}, which does not invert back to it"
            );
            assert_ne!(other, op, "{op:?} inverts to itself");
        }
    }

    /// And the ones with no opposite say so rather than inventing one.
    #[test]
    fn an_operation_with_no_opposite_says_so() {
        for op in [
            Combine::Intersect,
            Combine::Replace,
            Combine::Paint,
            Combine::Shell,
        ] {
            assert_eq!(
                op.inverted(),
                None,
                "{op:?} claims an opposite; inverting it would be inventing a \
                 meaning rather than turning one over"
            );
        }
        // The one a sculptor reaches for most has one.
        assert_eq!(Combine::Add.inverted(), Some(Combine::Subtract));
    }

    /// Smoothing wins over inverting, and neither held is the chosen tool.
    #[test]
    fn the_modifiers_choose_the_tool() {
        use crate::ToolKind;
        let none = StrokeModifiers::default();
        assert!(!none.any());
        assert_eq!(none.tool(ToolKind::Padrao), ToolKind::Padrao);

        let smooth = StrokeModifiers {
            smooth: true,
            invert: false,
        };
        assert_eq!(smooth.tool(ToolKind::Padrao), ToolKind::Suavizar);

        // Inverting alone leaves the tool alone: it is the *brush* that turns
        // over, not the verb.
        let invert = StrokeModifiers {
            smooth: false,
            invert: true,
        };
        assert_eq!(invert.tool(ToolKind::Padrao), ToolKind::Padrao);

        // Both: an inverted smooth is not a thing either reference offers, and
        // sharpening is a different verb rather than a smooth turned over.
        let both = StrokeModifiers {
            smooth: true,
            invert: true,
        };
        assert_eq!(both.tool(ToolKind::Padrao), ToolKind::Suavizar);
    }
}
