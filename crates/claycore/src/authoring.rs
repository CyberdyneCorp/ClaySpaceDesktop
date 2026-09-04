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
use crate::mask::MaskField;
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

/// Where a layer stands.
///
/// Generic in its scale so the two readers share one shape: `f32` from
/// [`Document::layer_transform`], `[f32; 3]` from
/// [`Document::layer_transform_nonuniform`]. One type rather than two, because
/// a manipulator that reads either wants the same four things back and the
/// only difference between them is how many numbers the scale takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerTransform<S> {
    pub position: [f32; 3],
    /// Unit length. A layer with no rotation answers an arbitrary axis and a
    /// zero angle, so read the two together.
    pub rotation_axis: [f32; 3],
    /// Radians.
    pub rotation_angle: f32,
    pub scale: S,
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

    /// Appends an alpha stamp to the item's deformer chain.
    ///
    /// A caller-supplied scalar image read as a distance offset, under the
    /// same radial falloff the noise and blob deformers use — pores, fabric,
    /// scales, stitching.
    ///
    /// A *deformer* rather than a primitive, which is the engine's design and
    /// not an accident of the API: an item shaped like the stamp would add
    /// material in the stamp's shape, where an alpha modulates a surface
    /// already there. So it offsets the distance and the surface moves along
    /// its own normal.
    ///
    /// The engine decodes no images. `samples` is `width * height` scalars in
    /// row-major order, and loading a PNG into that shape is the host's.
    ///
    /// `centre`, `direction` and `tangent` place and orient the stamp;
    /// `extent` is how far across it reaches in world units, `radius` the
    /// falloff's own reach, `amplitude` how far the surface moves, and `ease`
    /// an easing index.
    ///
    /// Refused, leaving the item unchanged: a width or height below 2 (there
    /// is nothing to interpolate between), a non-positive extent, or a
    /// `samples` shorter than the dimensions claim.
    // Mirrors the C entry point's parameter list, as `sculpt_carve_alpha`
    // does. Grouping them into a struct would be a second shape to keep in
    // step with the ABI for no gain.
    #[allow(clippy::too_many_arguments)]
    pub fn add_alpha(
        &mut self,
        samples: &[f32],
        width: i32,
        height: i32,
        centre: [f32; 3],
        direction: [f32; 3],
        tangent: [f32; 3],
        extent: f32,
        radius: f32,
        amplitude: f32,
        ease: i32,
    ) -> Result<()> {
        // Checked here rather than left to the engine: the C call reads
        // `width * height` floats out of the pointer, so a slice shorter than
        // that is a read past the end whatever the engine's own validation
        // says about the dimensions.
        let claimed = (width as i64) * (height as i64);
        if width < 2 || height < 2 || claimed > samples.len() as i64 {
            return Err(crate::raw_failure(
                "clay_item_add_alpha",
                crate::ErrorKind::InvalidArgument,
            ));
        }
        // SAFETY: valid handle; `samples` holds at least `width * height`
        // floats, checked above; the three arrays are three floats each.
        check(
            unsafe {
                sys::clay_item_add_alpha(
                    self.as_ptr(),
                    samples.as_ptr(),
                    width,
                    height,
                    centre.as_ptr(),
                    direction.as_ptr(),
                    tangent.as_ptr(),
                    extent,
                    radius,
                    amplitude,
                    ease,
                )
            },
            "clay_item_add_alpha",
        )
    }

    /// Gates this item by a painted mask, so it does not act where the mask
    /// protects.
    ///
    /// What makes masking protect a surface from *any* operation rather than
    /// only from a brush. Masks gate authoring elsewhere — a voxel edit
    /// consumes one per cell as it writes, an SDF stroke consumes one when it
    /// becomes items — but neither touches an item already in the edit list, so
    /// a mask over an ear had never done anything about the next boolean.
    ///
    /// The mask is *measured* rather than stored: what the item carries is the
    /// signed distance to the region at or above `threshold`. That is what
    /// gives the gate a Lipschitz bound worth having, and it means painted
    /// softness is re-derived from `width` rather than preserved.
    ///
    /// `width` is how far the protection fades across, in world units; zero is
    /// clamped rather than honoured, because a step in the field has no finite
    /// bound and nothing could march it. `threshold` at or below zero means
    /// 0.5.
    ///
    /// The gate is copied, so the mask may change or be dropped afterwards.
    /// Refused, leaving the item ungated, when the mask is empty or nothing
    /// reaches the threshold — a gate that protects nothing and reports success
    /// is harder to notice than a failure.
    pub fn set_gate(&mut self, mask: &MaskField, threshold: f32, width: f32) -> Result<()> {
        // SAFETY: valid item handle; `mask` is a live mask this call only
        // reads, and the engine copies what it needs before returning.
        check(
            unsafe {
                sys::clay_item_set_gate(self.as_ptr(), mask.as_ptr() as *const _, threshold, width)
            },
            "clay_item_set_gate",
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

    /// Undoes the last entry, and says what it reached.
    ///
    /// The same step [`Self::undo`] takes, plus the world-space bound of what
    /// it applied — which is the difference between re-meshing the region an
    /// undo changed and re-meshing the whole layer. Without it the narrowest
    /// region a host can name afterwards is the layer's own, measured here at
    /// 1045 keys and 141 ms against the 18 keys and 3.6 ms of the dab being
    /// taken back.
    ///
    /// The bound may be **looser** than what changed and never tighter, so it
    /// is safe to dirty. The engine's warning is worth repeating: do not try
    /// to work the region out by diffing the layer's nodes across the call
    /// instead — "an undone move, resize or colour edit keeps its node id, the
    /// diff sees nothing, and under-dirtying leaves stale bricks at a blend
    /// seam".
    pub fn undo_bound(&mut self) -> Result<Undone> {
        self.step_bound(true)
    }

    /// Redoes the last undone entry, and says what it reached.
    pub fn redo_bound(&mut self) -> Result<Undone> {
        self.step_bound(false)
    }

    fn step_bound(&mut self, backwards: bool) -> Result<Undone> {
        let mut moved = 0i32;
        let (mut min, mut max) = ([0.0f32; 3], [0.0f32; 3]);
        let (mut has_bounds, mut infinite) = (0i32, 0i32);
        let (call, name): (unsafe extern "C" fn(_, _, _, _, _, _) -> _, _) = if backwards {
            (sys::clay_document_undo_bound, "clay_document_undo_bound")
        } else {
            (sys::clay_document_redo_bound, "clay_document_redo_bound")
        };
        // SAFETY: a valid handle and five out-parameters, each valid for the
        // writes the entry point makes; the two bound arrays are three floats
        // as it requires.
        check(
            unsafe {
                call(
                    self.as_ptr(),
                    &mut moved,
                    min.as_mut_ptr(),
                    max.as_mut_ptr(),
                    &mut has_bounds,
                    &mut infinite,
                )
            },
            name,
        )?;
        Ok(Undone {
            moved: moved != 0,
            reached: match (has_bounds != 0, infinite != 0) {
                (false, _) => Influence::Nothing,
                (true, true) => Influence::Everything,
                (true, false) => Influence::Box { min, max },
            },
        })
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

    /// Opens an undo group, for a caller that cannot use [`Self::undo_group`].
    ///
    /// Prefer the closure form: it cannot leave a group open, and an open
    /// group silently swallows every later edit into it. This pair exists for
    /// the case the closure cannot express — a caller whose body needs a
    /// mutable borrow of something that also owns this document, which the
    /// closure's `&mut Self` argument makes impossible. Such a caller must
    /// close the group on every path, including the failing one.
    pub fn begin_undo_group(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_document_begin_undo_group(self.as_ptr()) },
            "clay_document_begin_undo_group",
        )
    }

    /// Closes the group [`Self::begin_undo_group`] opened.
    pub fn end_undo_group(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_document_end_undo_group(self.as_ptr()) },
            "clay_document_end_undo_group",
        )
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

    /// Renames a layer — the setter for the name [`Document::layer_name`]
    /// reads back.
    ///
    /// A command like every other layer edit, so one rename is one undo step
    /// and a ghosted or locked layer refuses it. `NULL` and the empty string
    /// are refused: an empty name is what a cleared text field submits, and
    /// the document's name would be the only one left to lose.
    ///
    /// Names are **not** unique, here or at creation, and nothing enforces
    /// one. [`Document::voxel_layer`] and its mesh counterpart answer with the
    /// *first* layer in stack order carrying the name, so renaming a voxel
    /// layer onto a name already in use shadows the other layer's grid. There
    /// is no id-addressed accessor for a grid, so a host that renames voxel
    /// layers has to keep those names distinct itself.
    ///
    /// Added in ClayCore 0.30.0, closing #92.
    pub fn set_layer_name(&mut self, layer: LayerId, name: &str) -> Result<()> {
        let c_name = crate::cstring(name, "clay_document_set_layer_name")?;
        // SAFETY: valid handle and a NUL-terminated name that outlives the call.
        check(
            unsafe { sys::clay_document_set_layer_name(self.as_ptr(), layer.0, c_name.as_ptr()) },
            "clay_document_set_layer_name",
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
            // `radius` carries the sign here rather than a radius; anything
            // other than +1 or -1 is refused.
            ArmatureEdit::SetSign { negative } => (4, [0.0; 3], if negative { -1.0 } else { 1.0 }),
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

    /// A placed armature's signs, one per node, parallel to the parents.
    ///
    /// `true` means the node cuts rather than adds. Signs stored shorter than
    /// the nodes read back positive-padded — the reading compilation makes —
    /// so a rig saved before signs existed comes back all-positive rather than
    /// failing.
    ///
    /// Added in ClayCore 0.30.0, closing #99.
    pub fn armature_signs(&self, layer: LayerId, node: NodeId) -> Result<Vec<bool>> {
        let mut count: usize = 0;
        // A first call with a null buffer asks how many there are.
        // SAFETY: the count is the only out-parameter written on this call.
        check(
            unsafe {
                sys::clay_layer_armature_signs(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    std::ptr::null_mut(),
                    &mut count,
                )
            },
            "clay_layer_armature_signs",
        )?;
        if count == 0 {
            return Ok(Vec::new());
        }

        let mut signs = vec![0i8; count];
        // SAFETY: `signs` is sized to the count the call above reported.
        check(
            unsafe {
                sys::clay_layer_armature_signs(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    signs.as_mut_ptr(),
                    &mut count,
                )
            },
            "clay_layer_armature_signs",
        )?;
        signs.truncate(count);
        Ok(signs.into_iter().map(|s| s < 0).collect())
    }

    /// How many top-level nodes a layer holds.
    ///
    /// A layer with no SDF content — a voxel or a mesh layer — counts zero
    /// rather than failing, which is the same reading the evaluation entry
    /// points make of it.
    ///
    /// Added in ClayCore 0.30.0, closing #91.
    pub fn layer_node_count(&self, layer: LayerId) -> Result<usize> {
        let mut count: usize = 0;
        // SAFETY: valid handle; the count is the only out-parameter.
        check(
            unsafe { sys::clay_layer_node_count(self.as_ptr(), layer.0, &mut count) },
            "clay_layer_node_count",
        )?;
        Ok(count)
    }

    /// The top-level node at `index`, in the layer's evaluation order.
    ///
    /// Index 0 is evaluated first. An index at or beyond the count is
    /// `NotFound`, so a host walks to the end without a sentinel.
    ///
    /// Top level only, on purpose: this is the sibling of
    /// [`Document::children`], which descends. [`Document::layer_nodes`] pairs
    /// the two.
    ///
    /// Added in ClayCore 0.30.0, closing #91.
    pub fn layer_node_at(&self, layer: LayerId, index: usize) -> Result<NodeId> {
        let mut node: sys::clay_node_id = Default::default();
        // SAFETY: valid handle; the node id is the only out-parameter.
        check(
            unsafe { sys::clay_layer_node_at(self.as_ptr(), layer.0, index, &mut node) },
            "clay_layer_node_at",
        )?;
        Ok(NodeId(node))
    }

    /// Every node in a layer, groups descended into, in evaluation order.
    ///
    /// The pair the ABI documents: enumerate the roots, ask what each one is,
    /// and recurse through the ones that answer as groups. A layer's own root
    /// is not a group and carries no node id, which is why the two calls have
    /// to exist separately.
    ///
    /// This replaces probing ids upward and tolerating a run of misses. Ids
    /// are not dense — a removal leaves a gap and nothing bounds how long one
    /// can be — so a probe loses every node past the longest gap it happened
    /// to tolerate, and no value of "long enough" is defensible.
    pub fn layer_nodes(&self, layer: LayerId) -> Result<Vec<NodeId>> {
        let mut found = Vec::new();
        for index in 0..self.layer_node_count(layer)? {
            let node = self.layer_node_at(layer, index)?;
            self.collect_nodes(layer, node, &mut found);
        }
        Ok(found)
    }

    /// `node` and everything under it, depth-first in evaluation order.
    ///
    /// A node that is not a group has no children to ask for, and the refusal
    /// is how that is known — so it is a leaf here rather than an error.
    fn collect_nodes(&self, layer: LayerId, node: NodeId, found: &mut Vec<NodeId>) {
        found.push(node);
        let Ok(children) = self.children(layer, node) else {
            return;
        };
        for child in children {
            self.collect_nodes(layer, child, found);
        }
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

    /// Every layer, in **stack order** — index 0 is evaluated first.
    ///
    /// Until ClayCore 0.29.0 there was no enumeration at all, and this probed
    /// consecutive ids for one that answered `clay_layer_bounds`, tolerating a
    /// run of eight misses before giving up. That guessed: a document with
    /// nine removals in a row came back short, and the order was the id order
    /// rather than the evaluation order — so a reopened document could
    /// evaluate differently from the one saved. See ClayCore #69, which
    /// enumeration closed: this asks `clay_document_layer_count` and
    /// `clay_document_layer_at`, and [`Self::layer_info`] and
    /// [`Self::layer_name`] answer for what each layer is. Re-checked at
    /// v0.78.0 — what is still write-only is a *node*, one level down, which
    /// is `tests/claycore_repros.rs` and a different ask.
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

    /// Places a whole layer, with a scale per axis.
    ///
    /// The whole-layer half of [`Self::set_node_transform_nonuniform`], which
    /// has placed a *node* per axis since ABI 0.54.0. A ZBrush-style gizmo
    /// scales per axis — the three boxes on the arms — on a placed object and
    /// on a whole subtool alike, and a layer that took one factor is why a
    /// host had to hide those boxes in scale mode for the subtool case.
    ///
    /// Composed innermost in the layer's own frame, before its rotation and
    /// translation, exactly as a node's is in its own:
    ///
    /// ```text
    /// world = layer.xform * diag(layer_scale) * node.xform * diag(node_scale)
    /// ```
    ///
    /// so `[1.0; 3]` is the identity and a document that never calls this
    /// compiles byte-identical tape.
    ///
    /// # What a non-uniform scale costs
    ///
    /// The evaluated distance is multiplied back by the product of the
    /// smallest component of each per-axis scale in the composition, which
    /// never overestimates the true distance — so the field stays a
    /// conservative bound, stays 1-Lipschitz, and the safe step scale does not
    /// move. What goes is *exactness*: the tape reports itself inexact, as it
    /// does for any non-uniform scale. A world radius mapped inward is divided
    /// by the *largest* component instead — the dual, so a gesture never
    /// reaches outside the region it named.
    ///
    /// One thing is refused rather than approximated:
    /// [`Document::lattice_gizmo`](crate::Document::lattice_gizmo) returns no
    /// warps for a layer carrying a per-axis scale. A cage records its
    /// item-to-cage placement as a rigid transform, and on a squashed layer
    /// the map it needs is a general affine one — the layer's diagonal sits
    /// *between* the two placements — so placing a cage through the narrower
    /// record would warp every item in a space it does not occupy, silently. A
    /// host that gets nothing back should offer the uniform gizmo.
    ///
    /// This call and [`Self::set_layer_transform`] each write the *whole*
    /// transform. The ABI does no partial updates, so the uniform one collapses
    /// a per-axis scale rather than leaving it alone, and a caller that wants
    /// to move a squashed layer without unsquashing it comes here.
    pub fn set_layer_transform_nonuniform(
        &mut self,
        layer: LayerId,
        position: [f32; 3],
        rotation_axis: [f32; 3],
        rotation_angle: f32,
        scale: [f32; 3],
    ) -> Result<()> {
        // SAFETY: valid handle and three three-float inputs.
        check(
            unsafe {
                sys::clay_document_set_layer_transform_nonuniform(
                    self.as_ptr(),
                    layer.0,
                    position.as_ptr(),
                    rotation_axis.as_ptr(),
                    rotation_angle,
                    scale.as_ptr(),
                )
            },
            "clay_document_set_layer_transform_nonuniform",
        )
    }

    /// Where a layer stands, when one factor can say it.
    ///
    /// New in ABI 0.74.0, and it answers a question the boundary could not
    /// previously be asked: until this, the ABI set a layer transform and did
    /// not read one back, so a host that wanted to know where its own subtool
    /// was had to remember what it had written.
    ///
    /// Refuses a layer carrying three different factors with
    /// [`ErrorKind::InvalidArgument`], exactly as the node-level reader
    /// refuses a squashed node: one float cannot express three, the uniform
    /// factor alone describes a differently-shaped subtool, and a
    /// read-change-write through [`Self::set_layer_transform`] would round the
    /// artist's squash away. A manipulator that does not want to branch reads
    /// [`Self::layer_transform_nonuniform`] instead.
    ///
    /// [`ErrorKind::InvalidArgument`]: crate::ErrorKind::InvalidArgument
    pub fn layer_transform(&self, layer: LayerId) -> Result<LayerTransform<f32>> {
        let (mut position, mut rotation_axis) = ([0.0f32; 3], [0.0f32; 3]);
        let (mut rotation_angle, mut scale) = (0.0f32, 0.0f32);
        // SAFETY: valid handle; every out-parameter is valid for one write of
        // its type, and the two arrays are three floats each as declared.
        check(
            unsafe {
                sys::clay_document_layer_transform(
                    self.as_ptr(),
                    layer.0,
                    position.as_mut_ptr(),
                    rotation_axis.as_mut_ptr(),
                    &mut rotation_angle,
                    &mut scale,
                )
            },
            "clay_document_layer_transform",
        )?;
        Ok(LayerTransform {
            position,
            rotation_axis,
            rotation_angle,
            scale,
        })
    }

    /// Where a layer stands, per axis.
    ///
    /// Answers the *product* of the layer's two scales, so a layer placed
    /// through [`Self::set_layer_transform`] answers `(s, s, s)` rather than
    /// `(1, 1, 1)` with the factor hidden somewhere the caller cannot see —
    /// which is what lets one manipulator read this and never branch.
    pub fn layer_transform_nonuniform(&self, layer: LayerId) -> Result<LayerTransform<[f32; 3]>> {
        let (mut position, mut rotation_axis) = ([0.0f32; 3], [0.0f32; 3]);
        let (mut rotation_angle, mut scale) = (0.0f32, [0.0f32; 3]);
        // SAFETY: as above, with a three-float scale out-parameter.
        check(
            unsafe {
                sys::clay_document_layer_transform_nonuniform(
                    self.as_ptr(),
                    layer.0,
                    position.as_mut_ptr(),
                    rotation_axis.as_mut_ptr(),
                    &mut rotation_angle,
                    scale.as_mut_ptr(),
                )
            },
            "clay_document_layer_transform_nonuniform",
        )?;
        Ok(LayerTransform {
            position,
            rotation_axis,
            rotation_angle,
            scale,
        })
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

    /// The same, with a scale per axis.
    ///
    /// Present in the ABI since 0.54.0 and bound by nothing here until now,
    /// which is why every transform in this application took one factor: the
    /// engine has been able to squash a capsule into a slot for six minor
    /// versions and the wrapper never offered it.
    ///
    /// The engine is exact about what it costs, and it is not what one would
    /// guess. The scale is applied innermost, in the node's own local frame,
    /// and the field stays 1-Lipschitz — so the safe step scale is unchanged
    /// and a marcher takes the steps it always did. What is lost is
    /// *exactness*: the value becomes a bound on the distance rather than the
    /// distance, short by at most the ratio of the largest axis to the
    /// smallest, and never an overestimate. That matters to a consumer that
    /// reads the value *as* a distance and to nothing else. A uniform value
    /// here, `[1.0; 3]` included, keeps the field exact and compiles to
    /// identical tape.
    ///
    /// Every component must be positive. A zero collapses the item onto a
    /// plane and has no inverse; a negative one mirrors it, which the layer
    /// mirror already expresses and which would silently flip the winding of a
    /// boolean.
    ///
    /// This call and [`Self::set_node_transform`] each write the *whole*
    /// transform, which settles what the uniform one does to a node carrying a
    /// per-axis scale: it collapses it. That is the ABI's own rule — it does
    /// not do partial updates — so a caller that wants to move a squashed node
    /// without unsquashing it comes here rather than there.
    pub fn set_node_transform_nonuniform(
        &mut self,
        layer: LayerId,
        node: NodeId,
        position: [f32; 3],
        rotation_axis: [f32; 3],
        rotation_angle: f32,
        scale: [f32; 3],
    ) -> Result<()> {
        // SAFETY: valid handle and three three-float inputs.
        check(
            unsafe {
                sys::clay_layer_set_transform_nonuniform(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    position.as_ptr(),
                    rotation_axis.as_ptr(),
                    rotation_angle,
                    scale.as_ptr(),
                )
            },
            "clay_layer_set_transform_nonuniform",
        )
    }

    /// Replaces an existing node's shape.
    ///
    /// The engine is explicit that this keeps what belongs to the node rather
    /// than to the primitive — "its deformers, repetition, profile and stroke
    /// belong to the node, not to the primitive, and survive the edit" — and
    /// so do its transform, its operation and its place in the order. That is
    /// the whole reason to have this rather than a remove and an add.
    pub fn set_node_prim(
        &mut self,
        layer: LayerId,
        node: NodeId,
        primitive: crate::Primitive,
    ) -> Result<()> {
        let params = primitive.params();
        // SAFETY: valid handle, and a slice whose length is passed beside it.
        check(
            unsafe {
                sys::clay_layer_set_prim(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    primitive.prim(),
                    params.as_ptr(),
                    params.len(),
                )
            },
            "clay_layer_set_prim",
        )
    }

    /// Changes how an existing node combines with what is under it.
    ///
    /// `rounding` must not be negative. A group takes `CLAY_OP_INLINE` and an
    /// item does not; [`Op`] has no inline variant, so that refusal is not
    /// reachable from here.
    pub fn set_node_op_blend(
        &mut self,
        layer: LayerId,
        node: NodeId,
        op: Op,
        blend: Blend,
        blend_k: f32,
        rounding: f32,
    ) -> Result<()> {
        // SAFETY: valid handle and four scalars the entry point range-checks.
        check(
            unsafe {
                sys::clay_layer_set_op_blend(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    op.raw(),
                    blend.raw(),
                    blend_k,
                    rounding,
                )
            },
            "clay_layer_set_op_blend",
        )
    }

    /// The box an edit to this node has to dirty.
    ///
    /// Not the geometry bound: this is dilated by rounding and blend support,
    /// because a node blended into its siblings reaches past its own shape.
    ///
    /// Three answers, kept three. Collapsing them into an `Option` was tried
    /// and is wrong in the worst available way: [`Influence::Nothing`] and
    /// [`Influence::Everything`] are opposite instructions, and a caller given
    /// one `None` for both either leaves a stale surface behind a moved
    /// intersect or refills the document every time a hidden node is touched.
    pub fn node_influence_bound(&self, layer: LayerId, node: NodeId) -> Result<Influence> {
        let (mut min, mut max) = ([0.0f32; 3], [0.0f32; 3]);
        let (mut has, mut infinite) = (0i32, 0i32);
        // SAFETY: valid handle, two three-float out-parameters and two flags.
        check(
            unsafe {
                sys::clay_layer_node_influence_bound(
                    self.as_ptr(),
                    layer.0,
                    node.0,
                    min.as_mut_ptr(),
                    max.as_mut_ptr(),
                    &mut has,
                    &mut infinite,
                )
            },
            "clay_layer_node_influence_bound",
        )?;
        Ok(match (has != 0, infinite != 0) {
            (false, _) => Influence::Nothing,
            (true, true) => Influence::Everything,
            (true, false) => Influence::Box { min, max },
        })
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

/// What an edit to a node reaches.
///
/// The engine's three states, as three: "*out_has_bounds 0 — nothing to dirty;
/// 1, *out_infinite 0 — the finite box; 1, *out_infinite 1 — unbounded, and
/// the honest response is to dirty everything".
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Influence {
    /// Nothing to dirty. A node the layer does not hold, or a hidden one —
    /// "a selection outlives the nodes in it", so this is an answer rather
    /// than a failure.
    Nothing,
    /// This box, already dilated by rounding and blend support.
    Box { min: [f32; 3], max: [f32; 3] },
    /// No finite box exists, so everything is dirty. Reached by "a non-local
    /// op (intersect, the spatial morphs) anywhere in the subtree, an infinite
    /// grid repeat, or an unbounded primitive (a plane, an infinite
    /// cylinder)" — so an ordinary cube placed with [`Op::Intersect`] answers
    /// this way, and it is a normal path rather than an edge case.
    Everything,
}

/// What a step through the history moved, and where.
///
/// `reached` is [`Influence::Nothing`] both for a step that had nothing to
/// take back and for one that cannot change the field — the engine reports a
/// rename that way — so `moved` is the one to ask about whether anything
/// happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Undone {
    pub moved: bool,
    pub reached: Influence,
}

/// Whether undo is recording, and how much there is to undo or redo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UndoState {
    pub enabled: bool,
    pub undo_depth: usize,
    pub redo_depth: usize,
}
