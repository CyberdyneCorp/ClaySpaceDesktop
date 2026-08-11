//! Building a document up: item settings, layer settings, and undo.
//!
//! Everything here mutates, so everything takes `&mut Document`. The undo
//! vocabulary is the engine's: enabling it is a choice, and a group makes a
//! run of edits undo as one step — which is how a symmetric edit or a whole
//! stroke becomes a single entry in a history a user reads.

use claycore_sys as sys;

use crate::error::{check, Result};
use crate::{Document, Item, LayerId, NodeId};

/// How an item combines with what is already there.
///
/// Beyond the three set operations the engine carries the extended modes and
/// the two whose item is a *region* rather than geometry: relief displaces the
/// accumulated surface along its normal, incise cuts the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Op {
    /// Union.
    #[default]
    Add,
    Subtract,
    Intersect,
    /// Colours without changing the surface.
    Paint,
    Groove,
    Tongue,
    Pipe,
    Engrave,
    Emboss,
    Inset,
    Shell,
    /// Replaces rather than combining.
    Replace,
    /// Displaces the accumulated surface along its normal — ZBrush's Standard
    /// and ClayBuildup.
    Relief,
    /// The same op cutting in; a thin region gives the line — Crease and
    /// DamStandard.
    Incise,
}

impl Op {
    fn raw(self) -> i32 {
        use sys::clay_op as o;
        (match self {
            Self::Add => o::CLAY_OP_ADD,
            Self::Subtract => o::CLAY_OP_SUBTRACT,
            Self::Intersect => o::CLAY_OP_INTERSECT,
            Self::Paint => o::CLAY_OP_PAINT,
            Self::Groove => o::CLAY_OP_GROOVE,
            Self::Tongue => o::CLAY_OP_TONGUE,
            Self::Pipe => o::CLAY_OP_PIPE,
            Self::Engrave => o::CLAY_OP_ENGRAVE,
            Self::Emboss => o::CLAY_OP_EMBOSS,
            Self::Inset => o::CLAY_OP_INSET,
            Self::Shell => o::CLAY_OP_SHELL,
            Self::Replace => o::CLAY_OP_REPLACE,
            Self::Relief => o::CLAY_OP_RELIEF,
            Self::Incise => o::CLAY_OP_INCISE,
        }) as i32
    }
}

/// How a combine's seam is shaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Blend {
    #[default]
    Hard,
    Quadratic,
    Cubic,
    Circular,
    Chamfer,
}

impl Blend {
    fn raw(self) -> i32 {
        (match self {
            Self::Hard => sys::clay_blend::CLAY_BLEND_HARD,
            Self::Quadratic => sys::clay_blend::CLAY_BLEND_QUADRATIC,
            Self::Cubic => sys::clay_blend::CLAY_BLEND_CUBIC,
            Self::Circular => sys::clay_blend::CLAY_BLEND_CIRCULAR,
            Self::Chamfer => sys::clay_blend::CLAY_BLEND_CHAMFER,
        }) as i32
    }
}

/// Whether a layer is shown, pickable and editable.
///
/// The three states are distinct on purpose: a ghosted layer is visible but
/// neither pickable nor editable, while a locked one is still pickable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Protection {
    pub ghost: bool,
    pub locked: bool,
}

impl Protection {
    /// Whether an edit may touch this layer at all.
    pub fn is_editable(&self) -> bool {
        !self.ghost && !self.locked
    }

    /// Whether a pick ray may select it.
    pub fn is_pickable(&self) -> bool {
        !self.ghost
    }
}

impl Item {
    /// Rotates about an axis, in radians. The axis need not be normalized.
    pub fn set_rotation(&mut self, axis: [f32; 3], radians: f32) -> Result<()> {
        // SAFETY: valid handle and a three-float axis.
        check(
            unsafe { sys::clay_item_set_rotation(self.as_ptr(), axis.as_ptr(), radians) },
            "clay_item_set_rotation",
        )
    }

    /// Scales uniformly. Must be positive.
    pub fn set_scale(&mut self, scale: f32) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_item_set_scale(self.as_ptr(), scale) },
            "clay_item_set_scale",
        )
    }

    /// How this item combines with what is below it.
    pub fn set_op(&mut self, op: Op) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_item_set_op(self.as_ptr(), op.raw()) },
            "clay_item_set_op",
        )
    }

    /// The seam profile and its support width.
    pub fn set_blend(&mut self, blend: Blend, k: f32) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_item_set_blend(self.as_ptr(), blend.raw(), k) },
            "clay_item_set_blend",
        )
    }

    /// Rounds the primitive's own edges.
    pub fn set_rounding(&mut self, rounding: f32) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_item_set_rounding(self.as_ptr(), rounding) },
            "clay_item_set_rounding",
        )
    }

    /// The item's colour in the field.
    pub fn set_color(&mut self, rgb: [f32; 3]) -> Result<()> {
        // SAFETY: valid handle and a three-float colour.
        check(
            unsafe { sys::clay_item_set_color(self.as_ptr(), rgb.as_ptr()) },
            "clay_item_set_color",
        )
    }

    /// Repeats radially about the origin.
    pub fn set_repeat_radial(&mut self, count: i32, offset: f32) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_item_set_repeat_radial(self.as_ptr(), count, offset) },
            "clay_item_set_repeat_radial",
        )
    }

    /// Repeats on a grid.
    pub fn set_repeat_grid(&mut self, spacing: [f32; 3], counts: [f32; 3]) -> Result<()> {
        // SAFETY: valid handle and two three-float inputs.
        check(
            unsafe {
                sys::clay_item_set_repeat_grid(self.as_ptr(), spacing.as_ptr(), counts.as_ptr())
            },
            "clay_item_set_repeat_grid",
        )
    }
}

impl Document {
    // -- undo ---------------------------------------------------------------

    /// Turns on undo recording.
    ///
    /// Opt-in, because recording costs memory a headless pipeline has no use
    /// for. An interactive host turns it on once, at startup.
    pub fn enable_undo(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_document_enable_undo(self.as_ptr()) },
            "clay_document_enable_undo",
        )
    }

    /// Undoes the last entry. Returns whether anything was undone.
    pub fn undo(&mut self) -> Result<bool> {
        let mut undone = 0i32;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_document_undo(self.as_ptr(), &mut undone) },
            "clay_document_undo",
        )?;
        Ok(undone != 0)
    }

    /// Redoes the last undone entry. Returns whether anything was redone.
    pub fn redo(&mut self) -> Result<bool> {
        let mut redone = 0i32;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_document_redo(self.as_ptr(), &mut redone) },
            "clay_document_redo",
        )?;
        Ok(redone != 0)
    }

    /// Whether undo is on, and how deep each stack is.
    pub fn undo_state(&self) -> Result<UndoState> {
        let mut enabled = 0i32;
        let (mut undo_depth, mut redo_depth) = (0usize, 0usize);
        // SAFETY: valid handle and three out-parameters.
        check(
            unsafe {
                sys::clay_document_undo_state(
                    self.as_ptr(),
                    &mut enabled,
                    &mut undo_depth,
                    &mut redo_depth,
                )
            },
            "clay_document_undo_state",
        )?;
        Ok(UndoState {
            enabled: enabled != 0,
            undo_depth,
            redo_depth,
        })
    }

    /// Runs `edits` so that everything inside undoes as one step.
    ///
    /// Taking a closure rather than exposing begin and end separately means an
    /// early return cannot leave a group open — which would silently swallow
    /// every later edit into it.
    pub fn undo_group<T>(&mut self, edits: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_document_begin_undo_group(self.as_ptr()) },
            "clay_document_begin_undo_group",
        )?;
        let result = edits(self);
        // SAFETY: valid handle; closed whether or not the body succeeded.
        let closed = check(
            unsafe { sys::clay_document_end_undo_group(self.as_ptr()) },
            "clay_document_end_undo_group",
        );
        // The body's failure is the more informative one, so it wins.
        result.and_then(|value| closed.map(|()| value))
    }

    // -- layers -------------------------------------------------------------

    /// Removes a layer and everything in it.
    pub fn remove_layer(&mut self, layer: LayerId) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_document_remove_layer(self.as_ptr(), layer.0) },
            "clay_document_remove_layer",
        )
    }

    /// Moves a layer to a position in the stack, which is its evaluation order.
    pub fn move_layer(&mut self, layer: LayerId, index: i32) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_document_move_layer(self.as_ptr(), layer.0, index) },
            "clay_document_move_layer",
        )
    }

    /// Shows or hides a layer.
    pub fn set_layer_visible(&mut self, layer: LayerId, visible: bool) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe {
                sys::clay_document_set_layer_visible(self.as_ptr(), layer.0, i32::from(visible))
            },
            "clay_document_set_layer_visible",
        )
    }

    /// Sets whether a layer is ghosted or locked.
    pub fn set_layer_protection(&mut self, layer: LayerId, protection: Protection) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe {
                sys::clay_document_set_layer_protection(
                    self.as_ptr(),
                    layer.0,
                    i32::from(protection.ghost),
                    i32::from(protection.locked),
                )
            },
            "clay_document_set_layer_protection",
        )
    }

    /// A layer's protection state.
    pub fn layer_protection(&self, layer: LayerId) -> Result<Protection> {
        let (mut ghost, mut locked) = (0i32, 0i32);
        // SAFETY: valid handle and two out-parameters.
        check(
            unsafe {
                sys::clay_document_layer_protection(self.as_ptr(), layer.0, &mut ghost, &mut locked)
            },
            "clay_document_layer_protection",
        )?;
        Ok(Protection {
            ghost: ghost != 0,
            locked: locked != 0,
        })
    }

    /// Places a whole layer.
    pub fn set_layer_transform(
        &mut self,
        layer: LayerId,
        position: [f32; 3],
        rotation_axis: [f32; 3],
        rotation_angle: f32,
        scale: f32,
    ) -> Result<()> {
        // SAFETY: valid handle and two three-float inputs.
        check(
            unsafe {
                sys::clay_document_set_layer_transform(
                    self.as_ptr(),
                    layer.0,
                    position.as_ptr(),
                    rotation_axis.as_ptr(),
                    rotation_angle,
                    scale,
                )
            },
            "clay_document_set_layer_transform",
        )
    }

    /// Mirrors a layer about the given axes, with a seam blend width.
    pub fn set_layer_mirror(
        &mut self,
        layer: LayerId,
        axes: [bool; 3],
        mirror_k: f32,
    ) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe {
                sys::clay_set_layer_mirror(
                    self.as_ptr(),
                    layer.0,
                    i32::from(axes[0]),
                    i32::from(axes[1]),
                    i32::from(axes[2]),
                    mirror_k,
                )
            },
            "clay_set_layer_mirror",
        )
    }

    /// A layer's world bounds, when it has any.
    pub fn layer_bounds(&self, layer: LayerId) -> Result<Option<([f32; 3], [f32; 3])>> {
        let (mut min, mut max) = ([0.0f32; 3], [0.0f32; 3]);
        let mut has = 0i32;
        // SAFETY: valid handle, two three-float out-parameters and a flag.
        check(
            unsafe {
                sys::clay_layer_bounds(
                    self.as_ptr(),
                    layer.0,
                    min.as_mut_ptr(),
                    max.as_mut_ptr(),
                    &mut has,
                )
            },
            "clay_layer_bounds",
        )?;
        Ok((has != 0).then_some((min, max)))
    }

    // -- placed nodes -------------------------------------------------------

    /// Removes a placed node.
    pub fn remove_node(&mut self, layer: LayerId, node: NodeId) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_remove_node(self.as_ptr(), layer.0, node.0) },
            "clay_remove_node",
        )
    }

    /// Re-places an existing node.
    pub fn set_node_transform(
        &mut self,
        layer: LayerId,
        node: NodeId,
        position: [f32; 3],
        rotation_axis: [f32; 3],
        rotation_angle: f32,
        scale: f32,
    ) -> Result<()> {
        // SAFETY: valid handle and two three-float inputs.
        check(
            unsafe {
                sys::clay_layer_set_transform(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    position.as_ptr(),
                    rotation_axis.as_ptr(),
                    rotation_angle,
                    scale,
                )
            },
            "clay_layer_set_transform",
        )
    }

    /// Recolours an existing node.
    pub fn set_node_color(&mut self, layer: LayerId, node: NodeId, rgb: [f32; 3]) -> Result<()> {
        // SAFETY: valid handle and a three-float colour.
        check(
            unsafe { sys::clay_layer_set_color(self.as_ptr(), layer.0, node.0, rgb.as_ptr()) },
            "clay_layer_set_color",
        )
    }
}

/// Whether undo is recording, and how much there is to undo or redo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UndoState {
    pub enabled: bool,
    pub undo_depth: usize,
    pub redo_depth: usize,
}
