//! Booleans between two subtools, and what one costs before it runs.
//!
//! The engine composes the layers of a document by hard union
//! (`clay/scene/tape.h`), so there is no live boolean between two of them —
//! that is ClayCore #321, filed and open. What can be built today is a
//! *resolved* boolean: each operand is sampled into a volume, the two volumes
//! are combined in a subtool of their own, and what comes out is an ordinary
//! subtool that can be sculpted, moved and used as an operand again.
//!
//! Resolved rather than live is the whole of what the vocabulary here has to
//! be honest about. The result is sampled onto a lattice, so it costs what
//! every other crossing in this application costs and is priced with the same
//! [`crate::Cost`]; and the operands are kept, because a sculptor who can
//! still reach the cylinder can move it and run the boolean again while one
//! whose cylinder was consumed cannot.

use crate::{Combine, LayerKey};

/// Which boolean.
///
/// Three, and not [`Combine`]'s fourteen: a groove, a shell and an emboss are
/// ways one *edit* meets the surface under it, and none of them is a thing to
/// ask of two whole forms. Offering them here would be offering an operation
/// with nothing to say about what it did to a pair of subtools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BooleanOp {
    /// The two become one form.
    #[default]
    Union,
    /// The second is taken out of the first.
    Subtract,
    /// Only where both are.
    Intersect,
}

impl BooleanOp {
    /// Every operation, in the order the panel offers them.
    pub const ALL: [BooleanOp; 3] = [Self::Union, Self::Subtract, Self::Intersect];

    /// The fallback name, in the domain's own language.
    ///
    /// The localised one comes from the view's table, indexed by position in
    /// [`BooleanOp::ALL`], as a shape's and a brush's do.
    pub fn label(self) -> &'static str {
        match self {
            Self::Union => "União",
            Self::Subtract => "Subtração",
            Self::Intersect => "Interseção",
        }
    }

    /// A stable name, for anything that has to write an operation down.
    pub fn key(self) -> &'static str {
        match self {
            Self::Union => "union",
            Self::Subtract => "subtract",
            Self::Intersect => "intersect",
        }
    }

    /// The mark a result carries in its name.
    ///
    /// A symbol rather than a word, because the name of a subtool is read in
    /// whatever language the interface is in and this one reads the same in
    /// all three.
    pub fn mark(self) -> &'static str {
        match self {
            Self::Union => "∪",
            Self::Subtract => "−",
            Self::Intersect => "∩",
        }
    }

    /// How the second operand is added to the result.
    ///
    /// The first is always [`Combine::Add`] — it is what the result is made
    /// of — and this is the only choice the sculptor makes.
    pub fn combine(self) -> Combine {
        match self {
            Self::Union => Combine::Add,
            Self::Subtract => Combine::Subtract,
            Self::Intersect => Combine::Intersect,
        }
    }

    /// Whether swapping the operands leaves the result unchanged.
    ///
    /// False for subtraction, which is the reason the interface has to *name*
    /// which subtool is being cut and which is doing the cutting: "A minus B"
    /// is the whole of what the sculptor is choosing there.
    pub fn is_symmetric(self) -> bool {
        self != Self::Subtract
    }
}

/// What the boolean panel is set to.
///
/// The operands, the operation and the resolution are the whole of the
/// decision; the cost follows from them, so this is what a command carries and
/// the cost is recomputed rather than sent alongside — the same arrangement
/// [`crate::ConversionSettings`] has, and for the same reason: two values that
/// can disagree eventually do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BooleanSettings {
    /// What is being cut, and what is doing the cutting.
    ///
    /// Named rather than "first" and "second" because subtraction is not
    /// symmetric and the words are what the sculptor is choosing between.
    pub base: Option<LayerKey>,
    pub tool: Option<LayerKey>,
    pub op: BooleanOp,
    /// The cell both operands are sampled at.
    pub cell_size: f32,
    /// Whether the operands are removed rather than hidden.
    ///
    /// False by default and deliberately: keeping them is what makes the
    /// boolean recoverable, since the result is baked and its operands cannot
    /// be re-edited through it.
    pub consume: bool,
}

impl Default for BooleanSettings {
    fn default() -> Self {
        Self {
            base: None,
            tool: None,
            op: BooleanOp::default(),
            // The brick cache's own cell, which is where every other crossing
            // in this application starts. The operands' own detail replaces it
            // as soon as a pair is chosen — see `ObjectModel::boolean_cell`.
            cell_size: 0.02,
            consume: false,
        }
    }
}

impl BooleanSettings {
    pub fn sanitized(mut self) -> Self {
        self.cell_size = self.cell_size.clamp(
            *crate::ConversionSettings::CELL_RANGE.start(),
            *crate::ConversionSettings::CELL_RANGE.end(),
        );
        self
    }

    /// The two operands, where two distinct ones have been chosen.
    ///
    /// `None` while the panel is half filled in, which is what stops a
    /// confirm button from being offered for an operation that has no second
    /// operand — and what stops a subtool being booleaned with itself.
    pub fn pair(&self) -> Option<(LayerKey, LayerKey)> {
        let (base, tool) = (self.base?, self.tool?);
        (base != tool).then_some((base, tool))
    }
}

/// Why a boolean was refused.
///
/// Each one names the operand it is about, because "that cannot be done" over
/// a pair of subtools leaves the sculptor to work out which of the two is the
/// problem — and the answer decides what they do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanRefusal {
    /// One of the two carries nothing to sample.
    Empty { operand: String },
    /// One of the two is ghosted or locked.
    Protected { operand: String, ghost: bool },
    /// An intersection of two forms that do not meet, which is nothing.
    NoOverlap { base: String, tool: String },
    /// The pair at this resolution does not fit the document's budget.
    OverBudget { cells: u64, budget_bytes: u64 },
    /// Two operands were not chosen, or the same one was chosen twice.
    NotAPair,
}

impl std::fmt::Display for BooleanRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty { operand } => write!(
                f,
                "o subtool «{operand}» está vazio, então não há o que combinar"
            ),
            Self::Protected { operand, ghost } => write!(
                f,
                "o subtool «{operand}» está {} e não pode participar",
                if *ghost { "fantasma" } else { "bloqueado" }
            ),
            Self::NoOverlap { base, tool } => write!(
                f,
                "«{base}» e «{tool}» não se encontram, então a interseção não \
                 deixaria nada"
            ),
            Self::OverBudget {
                cells,
                budget_bytes,
            } => write!(
                f,
                "essa resolução precisa de {cells} células, além do orçamento \
                 de {} MB",
                budget_bytes / (1024 * 1024)
            ),
            Self::NotAPair => {
                f.write_str("uma operação booleana precisa de dois subtools diferentes")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property the whole panel is arranged around: subtraction is the one
    /// operation where the order of the two operands is the decision.
    #[test]
    fn subtraction_is_the_one_that_is_not_symmetric() {
        assert!(!BooleanOp::Subtract.is_symmetric());
        assert!(BooleanOp::Union.is_symmetric());
        assert!(BooleanOp::Intersect.is_symmetric());
    }

    #[test]
    fn each_operation_reaches_a_distinct_combine() {
        let mut seen: Vec<Combine> = Vec::new();
        for op in BooleanOp::ALL {
            assert!(!seen.contains(&op.combine()), "{op:?} shares a combine");
            seen.push(op.combine());
        }
        assert_eq!(BooleanOp::Union.combine(), Combine::Add);
        assert_eq!(BooleanOp::Subtract.combine(), Combine::Subtract);
        assert_eq!(BooleanOp::Intersect.combine(), Combine::Intersect);
    }

    #[test]
    fn every_key_and_mark_is_distinct() {
        let keys: std::collections::BTreeSet<&str> =
            BooleanOp::ALL.iter().map(|op| op.key()).collect();
        assert_eq!(keys.len(), BooleanOp::ALL.len());
        let marks: std::collections::BTreeSet<&str> =
            BooleanOp::ALL.iter().map(|op| op.mark()).collect();
        assert_eq!(marks.len(), BooleanOp::ALL.len());
    }

    /// Keeping the operands is what makes the boolean recoverable, so it is
    /// what an untouched panel is set to.
    #[test]
    fn the_operands_are_kept_unless_the_sculptor_says_otherwise() {
        assert!(!BooleanSettings::default().consume);
        assert_eq!(BooleanSettings::default().op, BooleanOp::Union);
    }

    #[test]
    fn a_half_filled_panel_has_no_pair_to_run() {
        let mut settings = BooleanSettings::default();
        assert_eq!(settings.pair(), None, "nothing chosen is not a pair");
        settings.base = Some(LayerKey(1));
        assert_eq!(settings.pair(), None, "one operand is not a pair");
        settings.tool = Some(LayerKey(1));
        assert_eq!(
            settings.pair(),
            None,
            "a subtool booleaned with itself is not a pair"
        );
        settings.tool = Some(LayerKey(2));
        assert_eq!(settings.pair(), Some((LayerKey(1), LayerKey(2))));
    }

    #[test]
    fn a_resolution_outside_the_range_is_brought_back_in() {
        let settings = BooleanSettings {
            cell_size: 0.0,
            ..Default::default()
        }
        .sanitized();
        assert!(settings.cell_size >= *crate::ConversionSettings::CELL_RANGE.start());
    }

    /// A refusal that does not name the operand leaves the sculptor to guess
    /// which of the two they have to fix.
    #[test]
    fn every_refusal_names_what_it_is_about() {
        let empty = BooleanRefusal::Empty {
            operand: "Cilindro".into(),
        };
        assert!(empty.to_string().contains("Cilindro"));
        let ghosted = BooleanRefusal::Protected {
            operand: "Esfera".into(),
            ghost: true,
        };
        assert!(ghosted.to_string().contains("Esfera"));
        assert!(ghosted.to_string().contains("fantasma"));
        let locked = BooleanRefusal::Protected {
            operand: "Esfera".into(),
            ghost: false,
        };
        assert!(locked.to_string().contains("bloqueado"));
        let apart = BooleanRefusal::NoOverlap {
            base: "Esfera".into(),
            tool: "Caixa".into(),
        };
        assert!(apart.to_string().contains("Esfera") && apart.to_string().contains("Caixa"));
        let budget = BooleanRefusal::OverBudget {
            cells: 9,
            budget_bytes: 512 * 1024 * 1024,
        };
        assert!(budget.to_string().contains("512 MB"));
        assert!(!BooleanRefusal::NotAPair.to_string().is_empty());
    }
}
