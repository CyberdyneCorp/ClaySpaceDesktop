//! Gestures that become edits.
//!
//! These are the resolvers: a stroke becomes stamps, a drag becomes a
//! field-level move, a region becomes a relaxed or flattened volume. All of
//! them produce ordinary edits, so undo, coalescing and serialization apply to
//! them unchanged.

use claycore_sys as sys;

use crate::brush::{StrokePreset, StrokeSample};
use crate::descriptor::Descriptor;
use crate::error::{check, Result};
use crate::mask::MaskField;
use crate::{Document, Item, LayerId, NodeId};

/// How many stamps a stroke resolves into, without applying it.
///
/// Pure: the engine documents it as giving the same answer for the same input,
/// which is what makes it usable for sizing a buffer.
pub fn resolve_stroke(samples: &[StrokeSample], preset: &StrokePreset) -> Result<usize> {
    if samples.is_empty() {
        return Ok(0);
    }
    let flat = StrokeSample::flatten(samples);
    let raw_preset = preset.to_raw();
    let mut count = 0usize;
    // SAFETY: a null stamp buffer selects the counting form, which this entry
    // point does support.
    check(
        unsafe {
            sys::clay_stroke_resolve(
                flat.as_ptr(),
                samples.len(),
                &raw_preset,
                std::ptr::null_mut(),
                &mut count,
            )
        },
        "clay_stroke_resolve",
    )?;
    Ok(count)
}

/// How a Move drag falls off across its region.
#[derive(Debug, Clone, Copy)]
pub struct MoveParams {
    /// The drag's radius in world units. Must be positive.
    pub radius: f32,
    /// Falloff curve index across the region; 0 is linear.
    pub ease: i32,
    /// Do not drag the far side of a form.
    pub front_only: bool,
}

impl Default for MoveParams {
    fn default() -> Self {
        Self {
            radius: 0.25,
            ease: 0,
            front_only: true,
        }
    }
}

impl MoveParams {
    fn to_raw(self) -> sys::clay_move_params {
        let mut raw = sys::clay_move_params::sized();
        raw.radius = self.radius;
        raw.ease = self.ease;
        raw.front_only = i32::from(self.front_only);
        raw
    }
}

/// How a region is relaxed.
///
/// A `region_radius` of zero relaxes everywhere, which is a filter rather than
/// a brush — the engine says so, and it is worth repeating at the call site.
#[derive(Debug, Clone, Copy)]
pub struct RelaxParams<'mask> {
    /// How much of the smoothed value to take per pass, in `0..=1`.
    pub strength: f32,
    /// Averaging radius in cells.
    pub radius_cells: i32,
    pub iterations: i32,
    pub centre: [f32; 3],
    /// Zero relaxes the whole volume.
    pub region_radius: f32,
    /// Taper at the region's edge, widened by the engine if too narrow to hide
    /// the seam.
    pub falloff: f32,
    pub mask: Option<&'mask MaskField>,
}

impl Default for RelaxParams<'_> {
    fn default() -> Self {
        Self {
            strength: 0.5,
            radius_cells: 1,
            iterations: 1,
            centre: [0.0; 3],
            region_radius: 0.0,
            falloff: 0.0,
            mask: None,
        }
    }
}

/// Which side of the plane a flatten acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlattenMode {
    /// Material on the normal's side goes and hollows on the other fill.
    #[default]
    TwoSided,
    /// Only removes. This is what a planing tool wants: it must not fill the
    /// dents it is meant to reveal.
    CutOnly,
    /// Only fills.
    FillOnly,
}

/// Pulling a sampled volume onto a plane — the Flatten verb on the SDF side.
#[derive(Debug, Clone, Copy)]
pub struct FlattenParams<'mask> {
    /// A point on the plane to flatten onto.
    pub plane_point: [f32; 3],
    /// Unit normal; material on this side is the side that moves.
    pub plane_normal: [f32; 3],
    /// 1 puts the surface on the plane, 0 changes nothing.
    pub strength: f32,
    pub centre: [f32; 3],
    /// Required positive. With no region the engine replaces the shape with a
    /// half-space rather than flattening it — the header's words, and it means
    /// a ball comes back as a box.
    pub region_radius: f32,
    /// Taper at the region's edge; widened by the engine when too narrow.
    pub falloff: f32,
    pub mode: FlattenMode,
    pub mask: Option<&'mask MaskField>,
}

impl Default for FlattenParams<'_> {
    fn default() -> Self {
        Self {
            plane_point: [0.0; 3],
            plane_normal: [0.0, 1.0, 0.0],
            strength: 1.0,
            centre: [0.0; 3],
            // No sensible default: the engine refuses zero, and a silent
            // stand-in would be the box the header warns about.
            region_radius: 0.0,
            falloff: 0.0,
            mode: FlattenMode::TwoSided,
            mask: None,
        }
    }
}

impl FlattenParams<'_> {
    fn to_raw(self) -> sys::clay_flatten_params {
        let mut raw = sys::clay_flatten_params::sized();
        raw.plane_point = self.plane_point;
        raw.plane_normal = self.plane_normal;
        raw.strength = self.strength;
        raw.centre = self.centre;
        raw.region_radius = self.region_radius;
        raw.falloff = self.falloff;
        raw.mode = match self.mode {
            FlattenMode::TwoSided => 0,
            FlattenMode::CutOnly => 1,
            FlattenMode::FillOnly => 2,
        };
        raw.mask = self
            .mask
            .map_or(std::ptr::null(), |m| m.as_ptr() as *const _);
        raw
    }
}

impl RelaxParams<'_> {
    fn to_raw(self) -> sys::clay_relax_params {
        let mut raw = sys::clay_relax_params::sized();
        raw.strength = self.strength;
        raw.radius_cells = self.radius_cells;
        raw.iterations = self.iterations;
        raw.centre = self.centre;
        raw.region_radius = self.region_radius;
        raw.falloff = self.falloff;
        raw.mask = self
            .mask
            .map_or(std::ptr::null(), |m| m.as_ptr() as *const _);
        raw
    }
}

/// How a document region is sampled into a volume.
#[derive(Debug, Clone, Copy, Default)]
pub struct VolumeParams {
    /// Sample spacing; `None` picks from the source's own size.
    pub cell_size: Option<f32>,
    /// Half-width of the band kept; `None` means three cells.
    pub band: Option<f32>,
    /// How far past the bounds to sample; `None` means the band.
    pub padding: Option<f32>,
}

impl VolumeParams {
    fn to_raw(self) -> sys::clay_volume_params {
        let mut raw = sys::clay_volume_params::sized();
        raw.cell_size = self.cell_size.unwrap_or(0.0);
        raw.band = self.band.unwrap_or(0.0);
        raw.padding = self.padding.unwrap_or(0.0);
        raw
    }
}

impl Document {
    /// Applies a stroke to a layer, resolving it into ordinary edits.
    ///
    /// `item` is the stamp shape each sample deposits. Returns the nodes the
    /// stroke created, which is what an undo group needs to know about.
    ///
    /// The node buffer is sized with [`resolve_stroke`], which is pure. The
    /// apply entry point is emphatically **not** a size-query call — the
    /// engine's header says it "applies the stroke exactly once, however it is
    /// called" — so calling it twice to learn the count deposits the stroke
    /// twice.
    pub fn apply_stroke(
        &mut self,
        layer: LayerId,
        samples: &[StrokeSample],
        preset: &StrokePreset,
        item: &Item,
        mask: Option<&MaskField>,
    ) -> Result<Vec<NodeId>> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }
        let flat = StrokeSample::flatten(samples);
        let raw_preset = preset.to_raw();
        let mask_ptr = mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _);

        // An upper bound: a masked stamp emits no node, so the stroke may
        // create fewer than this.
        let capacity = resolve_stroke(samples, preset)?;
        let mut nodes = vec![sys::clay_node_id::default(); capacity];
        let mut count = capacity;

        // SAFETY: `flat` is `samples.len() * 5` floats; `nodes` holds
        // `capacity` ids and `count` carries that capacity in.
        check(
            unsafe {
                sys::clay_layer_apply_stroke(
                    self.as_ptr(),
                    layer.0,
                    flat.as_ptr(),
                    samples.len(),
                    &raw_preset,
                    item.as_ptr(),
                    mask_ptr,
                    if capacity == 0 {
                        std::ptr::null_mut()
                    } else {
                        nodes.as_mut_ptr()
                    },
                    &mut count,
                )
            },
            "clay_layer_apply_stroke",
        )?;

        // `count` reports the true total even when the buffer was smaller.
        nodes.truncate(count.min(capacity));
        Ok(nodes.into_iter().map(NodeId).collect())
    }

    /// Drags the assembled surface — the Move brush.
    ///
    /// Nudges form rather than growing it: a large pull buds rather than
    /// stretches. Use a snakehook stroke to pull a lobe out. Returns how many
    /// items were warped.
    pub fn move_surface(
        &mut self,
        layer: LayerId,
        centre: [f32; 3],
        displacement: [f32; 3],
        params: MoveParams,
    ) -> Result<usize> {
        let raw = params.to_raw();
        let mut applied = 0usize;
        // SAFETY: two three-float inputs, a sized descriptor, valid handle.
        check(
            unsafe {
                sys::clay_layer_move_surface(
                    self.as_ptr(),
                    layer.0,
                    centre.as_ptr(),
                    displacement.as_ptr(),
                    &raw,
                    &mut applied,
                )
            },
            "clay_layer_move_surface",
        )?;
        Ok(applied)
    }

    /// Which nodes a Move would touch, without touching them.
    ///
    /// Lets a host draw the affected region before the user commits.
    pub fn move_surface_preview(
        &self,
        layer: LayerId,
        centre: [f32; 3],
        displacement: [f32; 3],
        params: MoveParams,
        capacity: usize,
    ) -> Result<Vec<NodeId>> {
        let raw = params.to_raw();
        let mut nodes = vec![sys::clay_node_id::default(); capacity];
        let mut count = 0usize;
        // SAFETY: `nodes` holds `capacity` ids and the engine is told so.
        check(
            unsafe {
                sys::clay_layer_move_surface_preview(
                    self.as_ptr(),
                    layer.0,
                    centre.as_ptr(),
                    displacement.as_ptr(),
                    &raw,
                    nodes.as_mut_ptr(),
                    capacity,
                    &mut count,
                )
            },
            "clay_layer_move_surface_preview",
        )?;
        nodes.truncate(count.min(capacity));
        Ok(nodes.into_iter().map(NodeId).collect())
    }

    /// Flattens a region sampled straight from the document.
    ///
    /// The engine's own words: the difference from bake-then-flatten "is
    /// accuracy, and it is not small" — a volume reports a distance only
    /// inside its band and a bound outside it, so a facet moving further than
    /// the band is placed against the bound and a wrong shape comes back with
    /// `CLAY_OK`. A document has no band.
    ///
    /// Returns a new item carrying the result; the document is untouched.
    pub fn flatten_region(
        &self,
        flatten: &FlattenParams<'_>,
        volume: VolumeParams,
        min: [f32; 3],
        max: [f32; 3],
    ) -> Result<Item> {
        let raw_flatten = flatten.to_raw();
        let raw_volume = volume.to_raw();
        let mut item = std::ptr::null_mut();
        // SAFETY: two sized descriptors, two three-float bounds, and an
        // out-parameter written only on success.
        check(
            unsafe {
                sys::clay_item_volume_flatten_from(
                    self.as_ptr(),
                    &raw_flatten,
                    &raw_volume,
                    min.as_ptr(),
                    max.as_ptr(),
                    &mut item,
                )
            },
            "clay_item_volume_flatten_from",
        )?;
        Item::from_raw(item, "clay_item_volume_flatten_from")
    }

    /// Samples a region of the document into a volume item.
    ///
    /// This is the baking step the relax and flatten verbs work on: they act
    /// on a sampled volume, not on the live edit list.
    pub fn volume_from_region(
        &self,
        params: VolumeParams,
        min: [f32; 3],
        max: [f32; 3],
    ) -> Result<Item> {
        let raw = params.to_raw();
        let mut item = std::ptr::null_mut();
        // SAFETY: sized descriptor, two three-float bounds, out-parameter
        // written only on success.
        check(
            unsafe {
                sys::clay_item_volume_from_document(
                    self.as_ptr(),
                    &raw,
                    min.as_ptr(),
                    max.as_ptr(),
                    &mut item,
                )
            },
            "clay_item_volume_from_document",
        )?;
        Item::from_raw(item, "clay_item_volume_from_document")
    }
}

impl Item {
    /// Relaxes a sampled volume in place — the Smooth verb on the SDF side.
    ///
    /// Only valid on an item carrying a volume, which is what
    /// [`Document::volume_from_region`] produces.
    pub fn relax(&mut self, params: &RelaxParams<'_>) -> Result<()> {
        let raw = params.to_raw();
        // SAFETY: valid handle and a sized descriptor.
        check(
            unsafe { sys::clay_item_volume_relax(self.as_ptr(), &raw) },
            "clay_item_volume_relax",
        )
    }

    /// Pulls a sampled volume onto a plane — the Flatten verb on the SDF side.
    ///
    /// Only valid on an item carrying a volume, the same as
    /// [`Item::relax`]. Bound late: the planing tools were reaching for it and
    /// finding nothing, so they fell through to adding a sphere.
    pub fn flatten(&mut self, params: &FlattenParams<'_>) -> Result<()> {
        let raw = params.to_raw();
        // SAFETY: valid handle and a sized descriptor.
        check(
            unsafe { sys::clay_item_volume_flatten(self.as_ptr(), &raw) },
            "clay_item_volume_flatten",
        )
    }
}
