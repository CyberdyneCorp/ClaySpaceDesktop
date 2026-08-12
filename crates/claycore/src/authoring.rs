//! Building a document up: item settings, layer settings, and undo.
//!
//! Everything here mutates, so everything takes `&mut Document`. The undo
//! vocabulary is the engine's: enabling it is a choice, and a group makes a
//! run of edits undo as one step — which is how a symmetric edit or a whole
//! stroke becomes a single entry in a history a user reads.

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::document::ArmatureEdit;
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
/// What a layer holds, which decides which verbs can reach it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerRepresentation {
    Sdf,
    Voxel,
    Mesh,
}

impl LayerRepresentation {
    fn from_raw(raw: i32) -> Self {
        match raw {
            r if r == sys::clay_layer_representation::CLAY_LAYER_VOXEL as i32 => Self::Voxel,
            r if r == sys::clay_layer_representation::CLAY_LAYER_MESH as i32 => Self::Mesh,
            // An unknown value from a newer engine is treated as the edit tree
            // rather than refused: the layer is still there and still
            // evaluates, and guessing wrong costs a tool availability rather
            // than a document.
            _ => Self::Sdf,
        }
    }
}

/// Everything about one layer that has a fixed size.
///
/// Added in ClayCore 0.29.0 (#69). Before it, a reopened document knew ids and
/// protection and nothing else — so names were regenerated, voxel layers were
/// mistaken for SDF ones, and stack order was lost, which is the half that
/// could make a reopened document evaluate differently from the one saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerInfo {
    pub id: LayerId,
    pub representation: LayerRepresentation,
    /// Position in evaluation order — what `clay_document_move_layer` sets.
    pub stack_index: i32,
    pub visible: bool,
    pub protection: Protection,
}

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
    /// One edit to a placed armature's tree.
    ///
    /// One undo step whatever the edit: the command underneath is a whole-tree
    /// replace, which for an armature of tens of nodes costs less than granular
    /// bookkeeping and has an exact inverse.
    ///
    /// `mirrored` applies to [`ArmatureEdit::AddChild`] only. It adds the
    /// reflection through x = 0 in the same step, under the mirror of the
    /// parent where there is one — a node on the plane is its own reflection
    /// and is added once.
    pub fn armature_edit(
        &mut self,
        layer: LayerId,
        node: NodeId,
        edit: ArmatureEdit,
        target: u32,
        mirrored: bool,
    ) -> Result<()> {
        let (op, value, radius) = match edit {
            ArmatureEdit::AddChild { position, radius } => (0, position, radius),
            // A delta, and the target's whole subtree travels with it: an arm
            // hangs from a shoulder.
            ArmatureEdit::Move { delta } => (1, delta, 0.0),
            ArmatureEdit::SetRadius { radius } => (2, [0.0; 3], radius),
            // The target and everything under it.
            ArmatureEdit::Delete => (3, [0.0; 3], 0.0),
        };
        // SAFETY: valid handles and a three-float value the entry point reads
        // according to `op`.
        check(
            unsafe {
                sys::clay_layer_armature_edit(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    op,
                    target,
                    value.as_ptr(),
                    radius,
                    i32::from(mirrored),
                )
            },
            "clay_layer_armature_edit",
        )
    }

    /// A placed armature's parent array, one index per node.
    ///
    /// The topology half, and the one that makes a reloaded rig posable: the
    /// indices are the ones [`Document::armature_edit`] takes, so a host reads
    /// the tree, picks a subtree and edits by index. Positions alone cannot
    /// be turned back into a rig — a branch is not recoverable by guessing.
    ///
    /// Added in ClayCore 0.29.0, closing #77.
    pub fn armature_parents(&self, layer: LayerId, node: NodeId) -> Result<Vec<u32>> {
        let mut count: usize = 0;
        // A first call with a null buffer asks how many there are.
        // SAFETY: the count is the only out-parameter written on this call.
        check(
            unsafe {
                sys::clay_layer_armature_parents(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    std::ptr::null_mut(),
                    &mut count,
                )
            },
            "clay_layer_armature_parents",
        )?;
        if count == 0 {
            return Ok(Vec::new());
        }

        let mut parents = vec![0u32; count];
        // SAFETY: `parents` is sized to the count the call above reported.
        check(
            unsafe {
                sys::clay_layer_armature_parents(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    parents.as_mut_ptr(),
                    &mut count,
                )
            },
            "clay_layer_armature_parents",
        )?;
        parents.truncate(count);
        Ok(parents)
    }

    /// A group's children, in order.
    ///
    /// A node that is not a group is refused, which is also how a host that
    /// reloaded a document tells a group from an item.
    pub fn children(&self, layer: LayerId, node: NodeId) -> Result<Vec<NodeId>> {
        let mut count: usize = 0;
        // SAFETY: the count is the only out-parameter written on this call.
        check(
            unsafe {
                sys::clay_layer_children(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    std::ptr::null_mut(),
                    &mut count,
                )
            },
            "clay_layer_children",
        )?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut ids = vec![0u32; count];
        // SAFETY: `ids` is sized to the count the call above reported.
        check(
            unsafe {
                sys::clay_layer_children(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    ids.as_mut_ptr(),
                    &mut count,
                )
            },
            "clay_layer_children",
        )?;
        ids.truncate(count);
        Ok(ids.into_iter().map(NodeId).collect())
    }

    /// Which primitive a placed node carries.
    ///
    /// The dual of `clay_layer_children`: between the two, every node answers
    /// exactly one question. A group carries no primitive and is refused.
    ///
    /// Added in ClayCore 0.29.0. Before it, a host that reopened a document
    /// could not tell a rig from a stroke.
    pub fn node_prim(&self, layer: LayerId, node: NodeId) -> Result<i32> {
        let mut prim = 0;
        // SAFETY: a valid handle and an out-parameter written on success.
        check(
            unsafe { sys::clay_layer_node_prim(self.as_ptr(), layer.0, node.0, &mut prim) },
            "clay_layer_node_prim",
        )?;
        Ok(prim)
    }

    /// A placed stroke or armature's control points, as `x y z r` quadruples.
    ///
    /// The engine's note: reading is not editing, so a ghosted, locked or
    /// hidden layer answers normally.
    pub fn stroke_points(&self, layer: LayerId, node: NodeId) -> Result<Vec<[f32; 4]>> {
        let mut count: usize = 0;
        // A first call with a null buffer asks how many there are.
        // SAFETY: every out-parameter is optional and passed as null bar the
        // count, which the entry point fills.
        check(
            unsafe {
                sys::clay_layer_stroke_points(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    std::ptr::null_mut(),
                    &mut count,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            "clay_layer_stroke_points",
        )?;
        if count == 0 {
            return Ok(Vec::new());
        }

        let mut flat = vec![0.0f32; count * 4];
        // SAFETY: `flat` is sized to the count the call above reported.
        check(
            unsafe {
                sys::clay_layer_stroke_points(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    flat.as_mut_ptr(),
                    &mut count,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            "clay_layer_stroke_points",
        )?;
        Ok(flat
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect())
    }

    /// The layers this document holds, discovered by probing.
    ///
    /// The C ABI has no enumeration: a host knows the layers it created and,
    /// after `clay_document_load`, knows nothing. So this asks `clay_layer_bounds`
    /// for consecutive ids and keeps the ones that answer, stopping after a run
    /// of misses long enough to clear any gap left by a removal.
    ///
    /// It recovers ids and nothing else. Names, visibility, representation and
    /// stack order have no getters, so a document that comes back from disk
    /// comes back anonymous and in creation order rather than the order it was
    /// saved in. Filed as ClayCore #69; when enumeration lands this goes.
    /// Every layer, in **stack order** — index 0 is evaluated first.
    ///
    /// Until ClayCore 0.29.0 there was no enumeration at all, and this probed
    /// consecutive ids for one that answered `clay_layer_bounds`, tolerating a
    /// run of eight misses before giving up. That guessed: a document with
    /// nine removals in a row came back short, and the order was the id order
    /// rather than the evaluation order — so a reopened document could
    /// evaluate differently from the one saved. See ClayCore #69.
    pub fn layer_ids(&self) -> Result<Vec<LayerId>> {
        let mut count: usize = 0;
        // SAFETY: a valid handle and one out-parameter.
        check(
            unsafe { sys::clay_document_layer_count(self.as_ptr(), &mut count) },
            "clay_document_layer_count",
        )?;

        let mut ids = Vec::with_capacity(count);
        for index in 0..count {
            let mut id = 0;
            // SAFETY: the index is below the count the call above reported.
            check(
                unsafe { sys::clay_document_layer_at(self.as_ptr(), index, &mut id) },
                "clay_document_layer_at",
            )?;
            ids.push(LayerId(id));
        }
        Ok(ids)
    }

    /// Everything about one layer that has a fixed size.
    pub fn layer_info(&self, layer: LayerId) -> Result<LayerInfo> {
        let mut raw = sys::clay_layer_info::sized();
        // SAFETY: a sized output descriptor the library fills.
        check(
            unsafe { sys::clay_document_layer_info(self.as_ptr(), layer.0, &mut raw) },
            "clay_document_layer_info",
        )?;
        Ok(LayerInfo {
            id: LayerId(raw.id),
            representation: LayerRepresentation::from_raw(raw.representation),
            stack_index: raw.stack_index,
            visible: raw.visible != 0,
            protection: Protection {
                ghost: raw.ghost != 0,
                locked: raw.locked != 0,
            },
        })
    }

    /// The layer's name.
    ///
    /// A string rather than a [`LayerInfo`] field because it is the one layer
    /// property without a fixed size.
    pub fn layer_name(&self, layer: LayerId) -> Result<String> {
        let mut size: usize = 0;
        // A first call with a null buffer asks how many bytes, NUL included.
        // SAFETY: the size is the only out-parameter written on this call.
        check(
            unsafe {
                sys::clay_layer_name(self.as_ptr(), layer.0, std::ptr::null_mut(), &mut size)
            },
            "clay_layer_name",
        )?;
        if size <= 1 {
            return Ok(String::new());
        }

        let mut bytes = vec![0u8; size];
        // SAFETY: `bytes` is sized to what the call above asked for.
        check(
            unsafe {
                sys::clay_layer_name(
                    self.as_ptr(),
                    layer.0,
                    bytes.as_mut_ptr() as *mut std::os::raw::c_char,
                    &mut size,
                )
            },
            "clay_layer_name",
        )?;
        // The engine writes UTF-8 and NUL-terminates; the name is trusted to
        // be what a host set, so a lossy conversion cannot lose anything a
        // round trip put there.
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
    }

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
