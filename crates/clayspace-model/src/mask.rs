//! Freezing a region, and what can be done to the frozen region itself.
//!
//! A mask is not a tool. It is state the tools consult: a frozen region resists
//! every verb, and that is the whole of its contract. The operations here act
//! on the mask rather than on the surface, which is why they are a vocabulary
//! of their own rather than more entries in [`crate::tools::ToolKind`].

use crate::sculpt::ModelError;

/// Which way a masked patch is pulled when it is extruded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtrudeSide {
    /// Away from the surface.
    #[default]
    Outward,
    /// Into it.
    Inward,
    /// Half each way, so the original surface ends up in the middle.
    Centred,
}

impl ExtrudeSide {
    pub const ALL: [ExtrudeSide; 3] = [Self::Outward, Self::Inward, Self::Centred];

    pub fn label(self) -> &'static str {
        match self {
            Self::Outward => "Para fora",
            Self::Inward => "Para dentro",
            Self::Centred => "Centrado",
        }
    }
}

/// How a masked patch becomes a solid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtrudeSettings {
    /// Wall thickness in world units.
    pub thickness: f32,
    pub side: ExtrudeSide,
    /// Rounding on the rim; 0 is a hard edge.
    pub border_round: f32,
    /// Smoothing passes applied to a *copy* of the mask, so the painted mask
    /// survives the operation that consumed it.
    pub border_smooth: i32,
}

impl Default for ExtrudeSettings {
    fn default() -> Self {
        Self {
            thickness: 0.08,
            side: ExtrudeSide::Outward,
            border_round: 0.0,
            border_smooth: 0,
        }
    }
}

impl ExtrudeSettings {
    /// Clamps to what the engine accepts.
    ///
    /// Thickness must be positive; the engine refuses zero, and a refusal in
    /// the middle of a gesture is worse than a very thin wall.
    pub fn sanitized(self) -> Self {
        Self {
            thickness: self.thickness.clamp(0.001, 100.0),
            side: self.side,
            border_round: self.border_round.max(0.0),
            border_smooth: self.border_smooth.clamp(0, 16),
        }
    }
}

/// What can be done to a mask, as opposed to through one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskOp {
    /// Everything masked becomes unmasked and the reverse.
    Invert,
    /// Unmasks everything.
    Clear,
    /// Grows the masked region by whole cells.
    Expand(i32),
    /// Shrinks it.
    Contract(i32),
    /// Softens the boundary.
    Smooth(i32),
    /// Inverts only inside the region the mask already covers.
    ///
    /// The design calls this the bounded complement, and it is what makes
    /// "mask everything except this" one action rather than three.
    InvertWithinBounds,
}

impl MaskOp {
    pub fn label(self) -> &'static str {
        match self {
            Self::Invert => "Inverter",
            Self::Clear => "Limpar",
            Self::Expand(_) => "Expandir",
            Self::Contract(_) => "Contrair",
            Self::Smooth(_) => "Suavizar máscara",
            Self::InvertWithinBounds => "Complemento delimitado",
        }
    }

    /// Whether the operation needs a mask to exist first.
    ///
    /// Clearing nothing is not an error, and neither is inverting nothing —
    /// but the interface should not offer either as though something would
    /// happen.
    pub fn needs_a_mask(self) -> bool {
        !matches!(self, Self::Clear)
    }
}

/// What the interface shows about the mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MaskState {
    /// Whether a mask exists at all.
    pub present: bool,
    /// Cells with any value above zero.
    pub painted_cells: usize,
}

impl MaskState {
    /// Whether anything is actually frozen.
    pub fn is_active(self) -> bool {
        self.present && self.painted_cells > 0
    }
}

/// The mask, as something to be edited in its own right.
pub trait MaskModel {
    fn mask_state(&self) -> MaskState;

    /// Applies an operation to the mask itself.
    fn apply_mask_op(&mut self, op: MaskOp) -> Result<(), ModelError>;

    /// Pulls the masked patch off as a new layer.
    ///
    /// The mask is read, not consumed: a sculptor who extrudes and does not
    /// like the result should still have the region they painted.
    fn extrude_mask(&mut self, settings: ExtrudeSettings) -> Result<(), ModelError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mask_with_no_painted_cells_is_not_active() {
        // The distinction the interface needs: a mask that exists but freezes
        // nothing must not read as "masked", or every tool looks blocked.
        assert!(!MaskState {
            present: true,
            painted_cells: 0
        }
        .is_active());
        assert!(MaskState {
            present: true,
            painted_cells: 1
        }
        .is_active());
        assert!(!MaskState::default().is_active());
    }

    #[test]
    fn clearing_is_the_one_operation_that_needs_nothing() {
        for op in [
            MaskOp::Invert,
            MaskOp::Expand(1),
            MaskOp::Contract(1),
            MaskOp::Smooth(1),
            MaskOp::InvertWithinBounds,
        ] {
            assert!(op.needs_a_mask(), "{op:?} should need a mask");
        }
        assert!(!MaskOp::Clear.needs_a_mask());
    }

    #[test]
    fn every_operation_and_side_can_be_named() {
        for op in [
            MaskOp::Invert,
            MaskOp::Clear,
            MaskOp::Expand(1),
            MaskOp::Contract(1),
            MaskOp::Smooth(1),
            MaskOp::InvertWithinBounds,
        ] {
            assert!(!op.label().is_empty(), "{op:?} has no label");
        }
        for side in ExtrudeSide::ALL {
            assert!(!side.label().is_empty());
        }
    }

    #[test]
    fn extrude_settings_are_clamped_to_what_the_engine_accepts() {
        // A zero thickness is refused by the engine. Refusing mid-gesture is
        // worse than a very thin wall, so it is clamped rather than passed on.
        let sanitized = ExtrudeSettings {
            thickness: 0.0,
            side: ExtrudeSide::Inward,
            border_round: -1.0,
            border_smooth: 500,
        }
        .sanitized();
        assert!(sanitized.thickness > 0.0);
        assert!(sanitized.border_round >= 0.0);
        assert!(sanitized.border_smooth <= 16);
        assert_eq!(sanitized.side, ExtrudeSide::Inward);
    }
}
