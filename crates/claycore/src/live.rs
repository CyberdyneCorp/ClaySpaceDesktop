//! Live field brushes: a gesture that shows itself while it is happening.
//!
//! Smooth and Move cannot be spelled as stroke stamps. Smooth *bakes* — the
//! engine's relax takes a sampled volume and returns another — so a host with
//! nowhere to keep that volume between pointer events has to bake the layer
//! again per dab, and the only affordable implementation runs once when the
//! pointer comes up. Move warps every item it reaches, so written to the
//! document per event a drag churns revisions, tapes, caches and picking sixty
//! times a second to produce one edit.
//!
//! A transaction holds that transient state instead. [`SmoothTransaction`]
//! samples the layer **once**, relaxes its own working volume per dab and
//! installs that volume at commit; [`MoveTransaction`] walks the edit list
//! once and resolves the grabs of the items the drag reaches per frame.
//! Between begin and commit **the document does not change** — no nodes, no
//! deformers, no undo entries.
//!
//! What a host draws in the meantime is [`SmoothTransaction::take_preview`]:
//! the bricks whose samples are new since the last take. They are the engine's
//! own relaxed samples, so drawing them is not a second implementation of the
//! field — see `docs/architecture.md`.

use std::ptr::NonNull;

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::error::{check, ErrorKind, Result};
use crate::{raw_failure, Document, Item, LayerId, MoveParams, NodeId, RelaxParams};

/// How much field a live gesture is allowed to leave behind it.
///
/// `cell_size` is required and positive for Smooth: it is the spacing the
/// layer is sampled at when the gesture opens, and the engine has no intrinsic
/// scale to guess one from. The three budget criteria each disable at zero, so
/// an all-zero policy authorises nothing — which is why this has no `Default`
/// and is built through [`SculptPolicy::at`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SculptPolicy {
    pub cell_size: f32,
    /// Half-width of the band kept; `None` means three cells.
    pub band: Option<f32>,
    /// How far past the layer's bounds to sample; `None` means the band.
    pub padding: Option<f32>,
    /// Advise collapsing below this safe step scale; `None` disables.
    pub min_safe_step_scale: Option<f32>,
    /// Advise collapsing past this chain length; `None` disables.
    pub max_deformer_chain: Option<i32>,
    /// Advise collapsing past this item count; `None` disables.
    pub max_item_count: Option<i32>,
    /// Whether the engine may act on that advice by collapsing the layer,
    /// inside the gesture's own undo step.
    ///
    /// The whole opt-in for the destructive half: over budget without it, the
    /// commit reports [`SculptBudget::over_budget`] and changes nothing more.
    pub allow_consolidation: bool,
}

impl SculptPolicy {
    /// A policy at a sampling spacing, advising nothing.
    pub fn at(cell_size: f32) -> Self {
        Self {
            cell_size,
            band: None,
            padding: None,
            min_safe_step_scale: None,
            max_deformer_chain: None,
            max_item_count: None,
            allow_consolidation: false,
        }
    }

    fn to_raw(self) -> sys::clay_sculpt_policy {
        let mut raw = sys::clay_sculpt_policy::sized();
        raw.cell_size = self.cell_size;
        raw.band = self.band.unwrap_or(0.0);
        raw.padding = self.padding.unwrap_or(0.0);
        raw.min_safe_step_scale = self.min_safe_step_scale.unwrap_or(0.0);
        raw.max_deformer_chain = self.max_deformer_chain.unwrap_or(0);
        raw.max_item_count = self.max_item_count.unwrap_or(0);
        raw.allow_consolidation = i32::from(self.allow_consolidation);
        raw
    }
}

/// What one live update changed.
///
/// The bounds and the brick count are *geometric* — what the brush selected,
/// not which samples happened to move — so they are reproducible for a given
/// brush over a given lattice however much unrelated model surrounds it.
/// `changed` is the value question, answered separately: a dab whose weight
/// came out zero everywhere still selects its bricks and has nothing to
/// redraw.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SculptDirty {
    pub bounds: Option<([f32; 3], [f32; 3])>,
    pub touched_bricks: u64,
    pub changed: bool,
}

impl SculptDirty {
    fn from_raw(raw: &sys::clay_sculpt_dirty) -> Self {
        Self {
            bounds: (raw.has_bounds != 0).then_some((raw.bounds_min, raw.bounds_max)),
            touched_bricks: raw.touched_bricks,
            changed: raw.changed != 0,
        }
    }
}

/// What the budget said after a committed gesture, and what was done about it.
///
/// Measured *after* any consolidation, so it describes the layer the sculptor
/// is now looking at.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SculptBudget {
    /// At least one enabled criterion was crossed.
    pub over_budget: bool,
    /// The layer was collapsed, inside the gesture's own undo step.
    pub consolidated: bool,
    pub lipschitz: f32,
    pub safe_step_scale: f32,
    pub steepest_volume: f32,
    pub longest_deformer_chain: i32,
    pub item_count: i32,
}

impl SculptBudget {
    fn from_raw(raw: &sys::clay_sculpt_budget) -> Self {
        Self {
            over_budget: raw.over_budget != 0,
            consolidated: raw.consolidated != 0,
            lipschitz: raw.lipschitz,
            safe_step_scale: raw.safe_step_scale,
            steepest_volume: raw.steepest_volume,
            longest_deformer_chain: raw.longest_deformer_chain,
            item_count: raw.item_count,
        }
    }
}

/// One brick of a preview whose samples are new.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewBrick {
    /// The brick's lattice coordinate, in the *preview's* lattice.
    pub key: [i32; 3],
    /// World position of its first sample.
    pub origin: [f32; 3],
    /// World units between samples.
    pub spacing: f32,
    /// Samples per axis, **halo included**.
    pub sample_dim: u32,
    /// Where this brick's samples start in [`PreviewDelta::samples`].
    pub sample_offset: u64,
}

impl PreviewBrick {
    /// This brick's samples, x fastest over `sample_dim` cubed.
    pub fn samples<'a>(&self, delta: &'a PreviewDelta) -> &'a [f32] {
        let per = (self.sample_dim as usize).pow(3);
        let at = self.sample_offset as usize;
        &delta.samples[at..at + per]
    }
}

/// What is waiting to be drawn, without taking it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PreviewPending {
    /// Bumped by every update that changed the preview and by nothing else, so
    /// a host tells a duplicate read from a skipped frame and drops an upload
    /// it began against an older one.
    pub generation: u64,
    pub bricks: u64,
    pub sample_floats: u64,
    pub bounds: Option<([f32; 3], [f32; 3])>,
}

impl PreviewPending {
    pub fn is_empty(&self) -> bool {
        self.bricks == 0
    }
}

/// The bricks whose samples are new since the last take.
///
/// The delta *accumulates* until it is taken and is deduplicated by brick — a
/// dab materializes a brick and then relaxes it, so the same coordinate
/// arrives twice by construction — which is why a host that skips a frame
/// loses nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewDelta {
    /// The state the caller now holds. Taking clears the delta and not this.
    pub generation: u64,
    pub bricks: Vec<PreviewBrick>,
    /// Every brick's samples back to back; index them through
    /// [`PreviewBrick::samples`].
    pub samples: Vec<f32>,
    pub bounds: Option<([f32; 3], [f32; 3])>,
}

/// A live Smooth in progress.
///
/// Borrows nothing, on the same terms as [`crate::MeshSculptor`]: the handle
/// retains the document it was opened on, so that document must outlive it and
/// must not be edited while it is open. The engine guards the case that
/// matters — a commit whose layer moved underneath it is refused rather than
/// written over — and dropping the handle *is* a cancel, so an error path that
/// simply lets it go leaves no half a gesture behind.
pub struct SmoothTransaction {
    raw: NonNull<sys::clay_sdf_smooth_tx>,
    /// Set once commit or cancel has spent it, so `Drop` does not double-end
    /// a gesture the caller already finished.
    spent: bool,
}

impl SmoothTransaction {
    /// Opens a live Smooth on a layer.
    ///
    /// **This is the only evaluation of the source layer in the whole
    /// gesture**, and it is the sampling `clay_layer_consolidate` does. That
    /// is the pointer-down cost, and it is the trade the design makes: the
    /// whole finite layer once, so every dab afterwards costs what it touches.
    pub fn begin(doc: &mut Document, layer: LayerId, policy: SculptPolicy) -> Result<Self> {
        let raw = policy.to_raw();
        // SAFETY: a live document and layer, a sized policy descriptor, and no
        // cancellation token. Returns null on failure.
        let tx = unsafe {
            sys::clay_sdf_smooth_begin(doc.as_ptr(), layer.0, &raw, std::ptr::null_mut())
        };
        NonNull::new(tx)
            .map(|raw| Self { raw, spent: false })
            .ok_or_else(|| raw_failure("clay_sdf_smooth_begin", ErrorKind::InvalidArgument))
    }

    /// One live dab, relaxing the transaction's own working volume in place.
    ///
    /// Touches nothing in the document.
    pub fn update(&mut self, params: RelaxParams<'_>) -> Result<SculptDirty> {
        let raw = params.to_raw();
        let mut dirty = sys::clay_sculpt_dirty::sized();
        // SAFETY: a live transaction, a sized relax descriptor, no
        // cancellation token, and an out-parameter written only on success.
        check(
            unsafe {
                sys::clay_sdf_smooth_update(
                    self.raw.as_ptr(),
                    &raw,
                    std::ptr::null_mut(),
                    &mut dirty,
                )
            },
            "clay_sdf_smooth_update",
        )?;
        Ok(SculptDirty::from_raw(&dirty))
    }

    /// The preview so far, as a fresh volume item the caller owns.
    ///
    /// A **copy** of the working samples, which is honest about what it costs:
    /// the transaction goes on mutating its own volume, and handing out a view
    /// of something about to change under a compiled tape is the bug this
    /// refuses to offer.
    ///
    /// This is the whole working volume rather than the bricks that changed —
    /// see [`Self::take_preview`] for those — and it is what a host meshes
    /// when it wants the *engine* to decide where the previewed surface is.
    pub fn preview_item(&self) -> Result<Item> {
        let mut item = std::ptr::null_mut();
        // SAFETY: a live transaction and an out-parameter written only on
        // success; the item returned is owned by the caller.
        check(
            unsafe { sys::clay_sdf_smooth_preview_item(self.raw.as_ptr(), &mut item) },
            "clay_sdf_smooth_preview_item",
        )?;
        Item::from_raw(item, "clay_sdf_smooth_preview_item")
    }

    /// What is waiting to be drawn. Takes nothing, so it may be asked every
    /// frame to decide whether to bother.
    pub fn preview_pending(&self) -> Result<PreviewPending> {
        let mut info = sys::clay_sdf_preview_delta_info::sized();
        // SAFETY: a live transaction and a sized out-descriptor.
        check(
            unsafe { sys::clay_sdf_smooth_preview_delta_info(self.raw.as_ptr(), &mut info) },
            "clay_sdf_smooth_preview_delta_info",
        )?;
        Ok(PreviewPending {
            generation: info.generation,
            bricks: info.brick_count,
            sample_floats: info.sample_floats,
            bounds: (info.has_bounds != 0).then_some((info.bounds_min, info.bounds_max)),
        })
    }

    /// Takes the bricks whose samples are new, clearing the delta.
    ///
    /// `None` when nothing is waiting. The buffers are sized from
    /// [`Self::preview_pending`] in the same breath, because a short buffer
    /// takes *nothing*: a partial drain would strand bricks that no later call
    /// reports.
    pub fn take_preview(&mut self) -> Result<Option<PreviewDelta>> {
        let pending = self.preview_pending()?;
        if pending.is_empty() {
            return Ok(None);
        }
        let mut bricks = vec![sys::clay_sdf_preview_brick::default(); pending.bricks as usize];
        let mut samples = vec![0.0f32; pending.sample_floats as usize];
        let (mut got_bricks, mut got_samples) = (0u64, 0u64);
        // SAFETY: both buffers are sized from the info call above, and their
        // capacities are passed as element counts alongside them.
        check(
            unsafe {
                sys::clay_sdf_smooth_preview_delta_take(
                    self.raw.as_ptr(),
                    bricks.as_mut_ptr(),
                    bricks.len() as u64,
                    samples.as_mut_ptr(),
                    samples.len() as u64,
                    &mut got_bricks,
                    &mut got_samples,
                )
            },
            "clay_sdf_smooth_preview_delta_take",
        )?;
        bricks.truncate(got_bricks as usize);
        samples.truncate(got_samples as usize);
        Ok(Some(PreviewDelta {
            generation: pending.generation,
            bricks: bricks
                .into_iter()
                .map(|b| PreviewBrick {
                    key: b.key,
                    origin: b.origin,
                    spacing: b.spacing,
                    sample_dim: b.sample_dim,
                    sample_offset: b.sample_offset,
                })
                .collect(),
            samples,
            bounds: pending.bounds,
        }))
    }

    /// Installs the working volume as the layer's one item, as one undo step.
    ///
    /// **Never re-samples the layer**: the volume being installed is the one
    /// the dabs were applied to, which is the entire point. Refused, changing
    /// nothing, when the layer was edited, removed or protected since begin —
    /// and the transaction is spent either way.
    pub fn commit(&mut self) -> Result<SculptBudget> {
        let mut budget = sys::clay_sculpt_budget::sized();
        // SAFETY: a live transaction and a sized out-descriptor. The call
        // spends the transaction whether or not it succeeds, which `spent`
        // records so `Drop` does not cancel it a second time.
        let result = unsafe { sys::clay_sdf_smooth_commit(self.raw.as_ptr(), &mut budget) };
        self.spent = true;
        check(result, "clay_sdf_smooth_commit")?;
        Ok(SculptBudget::from_raw(&budget))
    }

    /// Discards the preview. The document was never touched, so this only ends
    /// the gesture.
    pub fn cancel(&mut self) {
        if !self.spent {
            // SAFETY: a live transaction, ended exactly once.
            unsafe { sys::clay_sdf_smooth_cancel(self.raw.as_ptr()) };
            self.spent = true;
        }
    }
}

impl Drop for SmoothTransaction {
    fn drop(&mut self) {
        self.cancel();
        // SAFETY: the handle is destroyed exactly once, after being ended.
        unsafe { sys::clay_sdf_smooth_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for SmoothTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmoothTransaction")
            .field("spent", &self.spent)
            .finish()
    }
}

/// One resolved grab of a live Move: the parameters
/// `clay_item_add_deformer(CLAY_DEFORM_GRAB, ...)` takes, in the node's own
/// frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewGrab {
    pub centre: [f32; 3],
    pub radius: f32,
    pub displacement: [f32; 3],
    pub ease: i32,
    pub front_only: bool,
}

/// A live Move drag in progress.
///
/// Holds its anchor and radius fixed and only grows the displacement, so which
/// items it reaches cannot change: the edit list is walked **once**, at begin,
/// and every frame after that costs the items it moves and nothing else.
///
/// Borrows nothing, on the same terms as [`SmoothTransaction`].
pub struct MoveTransaction {
    raw: NonNull<sys::clay_sdf_move_tx>,
    spent: bool,
}

impl MoveTransaction {
    /// Opens a live Move anchored at `centre`.
    ///
    /// A drag that reaches nothing succeeds with no affected nodes — the
    /// sculptor pressed on empty space, which is not an error.
    pub fn begin(
        doc: &mut Document,
        layer: LayerId,
        centre: [f32; 3],
        params: MoveParams,
        policy: Option<SculptPolicy>,
    ) -> Result<Self> {
        let raw_params = params.to_raw();
        let raw_policy = policy.map(SculptPolicy::to_raw);
        let policy_ptr = raw_policy.as_ref().map_or(std::ptr::null(), |p| p);
        // SAFETY: a live document and layer, a world-space anchor, and two
        // sized descriptors of which the policy may be declined with null.
        let tx = unsafe {
            sys::clay_sdf_move_begin(
                doc.as_ptr(),
                layer.0,
                centre.as_ptr(),
                &raw_params,
                policy_ptr,
            )
        };
        NonNull::new(tx)
            .map(|raw| Self { raw, spent: false })
            .ok_or_else(|| raw_failure("clay_sdf_move_begin", ErrorKind::InvalidArgument))
    }

    /// The drag so far, measured from the **anchor** — the total, never an
    /// increment on the last frame.
    ///
    /// Updates of 0.1, 0.2 then 0.5 end at exactly what a single fresh drag of
    /// 0.5 produces; a composition of the three would move the surface further
    /// than 0.5 ever asked for.
    pub fn update(&mut self, total_displacement: [f32; 3]) -> Result<SculptDirty> {
        let mut dirty = sys::clay_sculpt_dirty::sized();
        // SAFETY: a live transaction, three floats read, and an
        // out-parameter written only on success.
        check(
            unsafe {
                sys::clay_sdf_move_update(
                    self.raw.as_ptr(),
                    total_displacement.as_ptr(),
                    &mut dirty,
                )
            },
            "clay_sdf_move_update",
        )?;
        Ok(SculptDirty::from_raw(&dirty))
    }

    /// Which nodes the drag reaches. Fixed for the gesture — that is the
    /// point.
    pub fn reached(&self) -> Result<Vec<NodeId>> {
        let mut count = 0usize;
        // SAFETY: the size-query form, with a null buffer.
        check(
            unsafe {
                sys::clay_sdf_move_preview_nodes(
                    self.raw.as_ptr(),
                    std::ptr::null_mut(),
                    0,
                    &mut count,
                )
            },
            "clay_sdf_move_preview_nodes",
        )?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut nodes = vec![sys::clay_node_id::default(); count];
        // SAFETY: `nodes` is valid for the count the query just reported.
        check(
            unsafe {
                sys::clay_sdf_move_preview_nodes(
                    self.raw.as_ptr(),
                    nodes.as_mut_ptr(),
                    nodes.len(),
                    &mut count,
                )
            },
            "clay_sdf_move_preview_nodes",
        )?;
        nodes.truncate(count);
        Ok(nodes.into_iter().map(NodeId).collect())
    }

    /// The grabs the last update resolved for one reached node.
    ///
    /// One, unless the layer carries a mirror or a radial count, when it is
    /// one per **image** of the drag that reaches the node — a straddler takes
    /// the ball's grab and its reflection's, and a host that drew only the
    /// first would preview half the drag.
    pub fn grabs(&self, node: NodeId) -> Result<Vec<PreviewGrab>> {
        let mut count = 0usize;
        // SAFETY: a live transaction, a node it may or may not reach, and an
        // out-parameter written only on success.
        check(
            unsafe { sys::clay_sdf_move_preview_grab_count(self.raw.as_ptr(), node.0, &mut count) },
            "clay_sdf_move_preview_grab_count",
        )?;
        (0..count)
            .map(|index| {
                let mut centre = [0.0f32; 3];
                let mut radius = 0.0f32;
                let mut displacement = [0.0f32; 3];
                let (mut ease, mut front_only) = (0i32, 0i32);
                // SAFETY: an index the count above reported, and five
                // out-parameters valid for the writes the call makes.
                check(
                    unsafe {
                        sys::clay_sdf_move_preview_grab(
                            self.raw.as_ptr(),
                            node.0,
                            index,
                            centre.as_mut_ptr(),
                            &mut radius,
                            displacement.as_mut_ptr(),
                            &mut ease,
                            &mut front_only,
                        )
                    },
                    "clay_sdf_move_preview_grab",
                )?;
                Ok(PreviewGrab {
                    centre,
                    radius,
                    displacement,
                    ease,
                    front_only: front_only != 0,
                })
            })
            .collect()
    }

    /// Writes one deformer chain per affected node, all inside one undo step.
    ///
    /// The final chains are rebuilt from the chains captured at begin and the
    /// current total displacement, so a commit is what the preview showed even
    /// if the host never called update.
    pub fn commit(&mut self) -> Result<SculptBudget> {
        let mut budget = sys::clay_sculpt_budget::sized();
        // SAFETY: a live transaction and a sized out-descriptor; the call
        // spends the transaction whether or not it succeeds.
        let result = unsafe { sys::clay_sdf_move_commit(self.raw.as_ptr(), &mut budget) };
        self.spent = true;
        check(result, "clay_sdf_move_commit")?;
        Ok(SculptBudget::from_raw(&budget))
    }

    pub fn cancel(&mut self) {
        if !self.spent {
            // SAFETY: a live transaction, ended exactly once.
            unsafe { sys::clay_sdf_move_cancel(self.raw.as_ptr()) };
            self.spent = true;
        }
    }
}

impl Drop for MoveTransaction {
    fn drop(&mut self) {
        self.cancel();
        // SAFETY: the handle is destroyed exactly once, after being ended.
        unsafe { sys::clay_sdf_move_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for MoveTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MoveTransaction")
            .field("spent", &self.spent)
            .finish()
    }
}
