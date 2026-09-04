//! Freezing a region, and what can be done to the frozen region itself.
//!
//! A mask is not a tool. It is state the tools consult: a frozen region resists
//! every verb, and that is the whole of its contract. The operations here act
//! on the mask rather than on the surface, which is why they are a vocabulary
//! of their own rather than more entries in [`crate::tools::ToolKind`].

use crate::outline::MaskOutline;
use crate::sculpt::ModelError;
use crate::Representation;

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

/// Whether the masked patch of a layer of this representation can be pulled off
/// as a wall.
///
/// A field is sampled and a grid has a verb of its own; a mesh has neither, and
/// the engine refuses it with "this layer has no field to extrude from". Asked
/// before the entry is offered, so what a sculptor meets is a grey menu item
/// with a reason rather than a click that does nothing — which is what it was.
///
/// A hierarchy is refused for exactly the mesh's reason and not for a new one:
/// `clay_document_mask_extrude` samples a *layer's field*, and a hierarchy has
/// none.
///
/// An exhaustive `match` rather than a `matches!`, which is what it was. Either
/// spelling reads the same today; they differ on the day a fifth representation
/// arrives, when one of them answers for it by falling off the end and the
/// other stops compiling until somebody decides. This question is not one to
/// inherit an answer to: the wrong default here offers a menu item that pulls a
/// wall off a layer with nothing to pull it from.
pub fn can_extrude(representation: Representation) -> bool {
    match representation {
        Representation::Sdf | Representation::Voxel => true,
        Representation::Mesh | Representation::Multires => false,
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

    /// How far the operation reaches, where that is a number at all.
    ///
    /// `None` for the three that have no amount: there is no inverting twice
    /// as much, and clearing is clearing.
    pub fn amount(self) -> Option<i32> {
        match self {
            Self::Expand(steps) | Self::Contract(steps) | Self::Smooth(steps) => Some(steps),
            Self::Invert | Self::Clear | Self::InvertWithinBounds => None,
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
    ///
    /// Distinct from freezing something, and the distinction earns its keep:
    /// a mask belongs to a layer of the engine's document — which is what
    /// makes it survive a save — and a document has no verb for detaching one,
    /// so Limpar empties the mask rather than removing it. "There is no mask"
    /// and "the mask is empty" are two different things to tell a sculptor,
    /// and this is the field that tells them apart.
    pub present: bool,
    /// Cells with any value above zero.
    pub painted_cells: usize,
}

impl MaskState {
    /// Whether anything is actually frozen.
    ///
    /// What the interface keys the mask panel on, rather than
    /// [`MaskState::present`]: a panel offering to invert, expand and extrude
    /// nothing is a row of controls that all refuse.
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

    /// Freezes or releases everything a shape drawn over the form encloses.
    ///
    /// Here rather than in [`MaskOp`] because it is not an operation *on* an
    /// existing region: it is how a region is drawn in the first place, the
    /// other way being the brush. Which gesture drew the shape — traced by
    /// hand or dragged as a box — is settled before it gets here. See
    /// [`crate::outline`].
    fn apply_outline(&mut self, outline: &MaskOutline) -> Result<(), ModelError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The extrude reaches a field and a grid, and neither of the two that
    /// carry vertices — because it samples a field, and neither of them has
    /// one. Written over `Representation::ALL` so that a representation added
    /// without an answer here fails rather than inheriting one.
    #[test]
    fn the_wall_is_pulled_off_a_field_and_never_off_vertices() {
        for representation in Representation::ALL {
            let expected = matches!(representation, Representation::Sdf | Representation::Voxel);
            assert_eq!(
                can_extrude(representation),
                expected,
                "extruding a mask on {}",
                representation.label()
            );
        }
    }

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
