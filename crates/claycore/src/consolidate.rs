//! What a layer's field costs, and collapsing it when that cost is too high.
//!
//! An edit list grows, and each bake steepens the field it produces. The
//! engine measures both and can collapse a layer into one redistanced volume —
//! but it is expensive, so the cost is reported *before* it is paid and the
//! caller decides.

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::error::{check, Result};
use crate::{Document, LayerId};

/// What a layer's field costs to evaluate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldReport {
    /// The compiled layer's declared Lipschitz bound.
    pub lipschitz: f32,
    /// Multiply a distance by this before stepping along a ray. A low value
    /// means a ray march takes many small steps.
    pub safe_step_scale: f32,
    /// The largest sample Lipschitz among volume items.
    pub steepest_volume: f32,
    pub longest_deformer_chain: i32,
    pub item_count: i32,
    /// Whether the engine advises consolidating, given the threshold asked
    /// about.
    pub advises_consolidation: bool,
}

/// How a layer would be collapsed.
///
/// `cell_size` is required rather than optional, because the engine cannot
/// supply one: "a layer has no intrinsic scale to derive one from the way a
/// mesh's bounds give one". A caller that has a brick cache already knows the
/// scale it works at.
#[derive(Debug, Clone, Copy)]
pub struct ConsolidationParams {
    /// Sample spacing. Must be positive.
    pub cell_size: f32,
    /// Half-width of the band kept; `None` means three cells.
    pub band: Option<f32>,
    /// How far past the layer's bounds to sample; `None` means the band.
    pub padding: Option<f32>,
    /// Skip the redistancing pass. The engine's own note is that six passes
    /// hold the declared Lipschitz at the square root of three instead of
    /// letting it reach 32, so skipping is rarely what a caller wants.
    pub skip_redistance: bool,
}

impl ConsolidationParams {
    /// Collapse at a given sample spacing, with the engine's defaults for the
    /// rest.
    pub fn at(cell_size: f32) -> Self {
        Self {
            cell_size,
            band: None,
            padding: None,
            skip_redistance: false,
        }
    }

    fn to_raw(self) -> sys::clay_consolidation_params {
        let mut raw = sys::clay_consolidation_params::sized();
        raw.cell_size = self.cell_size;
        raw.band = self.band.unwrap_or(0.0);
        raw.padding = self.padding.unwrap_or(0.0);
        raw.skip_redistance = i32::from(self.skip_redistance);
        raw
    }
}

/// What consolidating would produce, and cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConsolidationCost {
    pub cell_size: f32,
    pub band: f32,
    pub brick_count: u64,
    pub sample_count: u64,
    /// Memory the collapsed volume would occupy.
    pub bytes: u64,
    /// How fast the stored samples vary.
    pub sample_lipschitz: f32,
    /// What the compiler will declare for them.
    pub lipschitz: f32,
    pub safe_step_scale: f32,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

impl ConsolidationCost {
    fn from_raw(raw: sys::clay_consolidation_cost) -> Self {
        Self {
            cell_size: raw.cell_size,
            band: raw.band,
            brick_count: raw.brick_count,
            sample_count: raw.sample_count,
            bytes: raw.bytes,
            sample_lipschitz: raw.sample_lipschitz,
            lipschitz: raw.lipschitz,
            safe_step_scale: raw.safe_step_scale,
            bounds_min: raw.bounds_min,
            bounds_max: raw.bounds_max,
        }
    }
}

impl Document {
    /// What a layer's field costs.
    ///
    /// `advise_below_step_scale` is the threshold under which the engine
    /// reports that consolidation is worth considering.
    pub fn field_report(
        &self,
        layer: LayerId,
        advise_below_step_scale: f32,
    ) -> Result<FieldReport> {
        let mut raw = sys::clay_field_report::sized();
        // SAFETY: valid handle and a descriptor carrying its struct_size.
        check(
            unsafe {
                sys::clay_layer_field_report(
                    self.as_ptr(),
                    layer.0,
                    advise_below_step_scale,
                    &mut raw,
                )
            },
            "clay_layer_field_report",
        )?;
        Ok(FieldReport {
            lipschitz: raw.lipschitz,
            safe_step_scale: raw.safe_step_scale,
            steepest_volume: raw.steepest_volume,
            longest_deformer_chain: raw.longest_deformer_chain,
            item_count: raw.item_count,
            advises_consolidation: raw.advises_consolidation != 0,
        })
    }

    /// What consolidating a layer would cost, without doing it.
    ///
    /// The interface shows this and waits: consolidation is expensive and
    /// irreversible except through undo, so it is never performed unasked.
    pub fn consolidation_cost(
        &self,
        layer: LayerId,
        params: ConsolidationParams,
        region: Option<([f32; 3], [f32; 3])>,
    ) -> Result<ConsolidationCost> {
        let raw_params = params.to_raw();
        let mut raw = sys::clay_consolidation_cost::sized();
        let (min, max) = region.unzip_or_null();
        // SAFETY: valid handle; the region is either two three-float arrays or
        // two null pointers, which the entry point permits.
        check(
            unsafe {
                sys::clay_layer_consolidation_cost(
                    self.as_ptr(),
                    layer.0,
                    &raw_params,
                    min,
                    max,
                    &mut raw,
                )
            },
            "clay_layer_consolidation_cost",
        )?;
        Ok(ConsolidationCost::from_raw(raw))
    }

    /// Collapses a layer into one redistanced volume.
    pub fn consolidate(
        &mut self,
        layer: LayerId,
        params: ConsolidationParams,
        region: Option<([f32; 3], [f32; 3])>,
    ) -> Result<ConsolidationCost> {
        let raw_params = params.to_raw();
        let mut raw = sys::clay_consolidation_cost::sized();
        let (min, max) = region.unzip_or_null();
        // SAFETY: as `consolidation_cost`, and the document is uniquely
        // borrowed for the mutation.
        check(
            unsafe {
                sys::clay_layer_consolidate(self.as_ptr(), layer.0, &raw_params, min, max, &mut raw)
            },
            "clay_layer_consolidate",
        )?;
        Ok(ConsolidationCost::from_raw(raw))
    }

    /// Whether a layer is already consolidated, and what it cost.
    pub fn consolidation_state(&self, layer: LayerId) -> Result<Option<ConsolidationCost>> {
        let mut consolidated = 0i32;
        let mut raw = sys::clay_consolidation_cost::sized();
        // SAFETY: valid handle, a flag and a sized descriptor.
        check(
            unsafe {
                sys::clay_layer_consolidation_state(
                    self.as_ptr(),
                    layer.0,
                    &mut consolidated,
                    &mut raw,
                )
            },
            "clay_layer_consolidation_state",
        )?;
        Ok((consolidated != 0).then(|| ConsolidationCost::from_raw(raw)))
    }
}

/// Splits an optional region into the two pointers the engine takes.
///
/// Both or neither: the engine rejects one without the other, so this makes
/// passing one impossible.
trait RegionPointers {
    fn unzip_or_null(&self) -> (*const f32, *const f32);
}

impl RegionPointers for Option<([f32; 3], [f32; 3])> {
    fn unzip_or_null(&self) -> (*const f32, *const f32) {
        match self {
            Some((min, max)) => (min.as_ptr(), max.as_ptr()),
            None => (std::ptr::null(), std::ptr::null()),
        }
    }
}
