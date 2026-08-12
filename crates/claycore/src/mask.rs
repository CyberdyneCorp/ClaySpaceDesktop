//! Mask fields: the regions an edit must not touch.
//!
//! A mask freezes what it covers against *every* verb, on either
//! representation, and survives a resolution change. It is painted along a
//! stroke like any other brush.
//!
//! # Owned and borrowed
//!
//! The engine lends a mask that belongs to a document and hands over one
//! created standalone. Destroying a borrowed handle would corrupt the
//! document, so the two are different types here: [`Mask`] owns and releases,
//! [`MaskRef`] borrows and cannot outlive its document. Both reach the same
//! operations through [`MaskField`], so nothing is duplicated.

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use claycore_sys as sys;

use crate::brush::BrushParams;
use crate::descriptor::Descriptor;
use crate::error::{check, ErrorKind, Result};
use crate::{raw_failure, Document, Item, LayerId};

/// Every operation a mask supports, regardless of who owns it.
///
/// Reached by dereferencing [`Mask`] or [`MaskRef`]; never constructed
/// directly, which is what keeps ownership in the two outer types.
#[repr(transparent)]
pub struct MaskField {
    raw: NonNull<sys::clay_mask>,
}

impl MaskField {
    pub(crate) fn as_ptr(&self) -> *mut sys::clay_mask {
        self.raw.as_ptr()
    }

    /// World units per mask cell.
    pub fn cell_size(&self) -> Result<f32> {
        let mut value = 0.0f32;
        // SAFETY: valid handle and a valid out-parameter.
        check(
            unsafe { sys::clay_mask_cell_size(self.as_ptr(), &mut value) },
            "clay_mask_cell_size",
        )?;
        Ok(value)
    }

    /// How many cells carry a non-zero value.
    pub fn painted_count(&self) -> Result<usize> {
        let mut count = 0usize;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_mask_painted_count(self.as_ptr(), &mut count) },
            "clay_mask_painted_count",
        )?;
        Ok(count)
    }

    /// Whether anything is masked at all.
    pub fn is_empty(&self) -> Result<bool> {
        let mut empty = 0i32;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_mask_empty(self.as_ptr(), &mut empty) },
            "clay_mask_empty",
        )?;
        Ok(empty != 0)
    }

    /// The mask value at a world point, interpolated.
    pub fn sample(&self, point: [f32; 3]) -> Result<f32> {
        let mut value = 0.0f32;
        // SAFETY: three-float input, valid out-parameter.
        check(
            unsafe { sys::clay_mask_sample(self.as_ptr(), point.as_ptr(), &mut value) },
            "clay_mask_sample",
        )?;
        Ok(value)
    }

    /// The mask value at many world points.
    pub fn sample_many(&self, points: &[[f32; 3]]) -> Result<Vec<f32>> {
        let mut values = vec![0.0f32; points.len()];
        if points.is_empty() {
            return Ok(values);
        }
        // SAFETY: input is `points.len() * 3` floats; output is one per point.
        check(
            unsafe {
                sys::clay_mask_sample_many(
                    self.as_ptr(),
                    points.as_ptr() as *const f32,
                    points.len(),
                    values.as_mut_ptr(),
                )
            },
            "clay_mask_sample_many",
        )?;
        Ok(values)
    }

    /// The cells the mask covers, when it covers any.
    pub fn bounds(&self) -> Result<Option<([i32; 3], [i32; 3])>> {
        let (mut min, mut max) = ([0i32; 3], [0i32; 3]);
        let mut has = 0i32;
        // SAFETY: two three-int out-parameters and a flag.
        check(
            unsafe {
                sys::clay_mask_bounds(self.as_ptr(), min.as_mut_ptr(), max.as_mut_ptr(), &mut has)
            },
            "clay_mask_bounds",
        )?;
        Ok((has != 0).then_some((min, max)))
    }

    /// Paints toward `target` with a brush footprint at a world point.
    pub fn paint(&mut self, point: [f32; 3], brush: &BrushParams<'_>, target: f32) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: valid handle, three-float point, descriptor with struct_size.
        check(
            unsafe { sys::clay_mask_paint(self.as_ptr(), point.as_ptr(), &raw, target) },
            "clay_mask_paint",
        )
    }

    /// Paints along a stroke, which is what makes masking the same gesture as
    /// sculpting.
    ///
    /// Returns how many stamps were applied.
    pub fn apply_stroke(
        &mut self,
        samples: &[crate::brush::StrokeSample],
        preset: &crate::brush::StrokePreset,
        target: f32,
        shape: crate::brush::BrushShape,
        falloff: crate::brush::Falloff,
    ) -> Result<usize> {
        if samples.is_empty() {
            return Ok(0);
        }
        let flat = crate::brush::StrokeSample::flatten(samples);
        let raw_preset = preset.to_raw();
        let mut applied = 0usize;
        // SAFETY: `flat` is `samples.len() * 5` floats as the entry point
        // expects; the preset carries its struct_size.
        check(
            unsafe {
                sys::clay_mask_apply_stroke(
                    self.as_ptr(),
                    flat.as_ptr(),
                    samples.len(),
                    &raw_preset,
                    target,
                    crate::brush::shape_raw(shape),
                    crate::brush::falloff_raw(falloff),
                    &mut applied,
                )
            },
            "clay_mask_apply_stroke",
        )?;
        Ok(applied)
    }

    /// Inverts every cell.
    pub fn invert(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_mask_invert(self.as_ptr()) },
            "clay_mask_invert",
        )
    }

    /// Unmasks everything.
    pub fn clear(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_mask_clear(self.as_ptr()) },
            "clay_mask_clear",
        )
    }

    /// Grows the masked region by whole cells.
    pub fn expand(&mut self, steps: i32) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_mask_expand(self.as_ptr(), steps) },
            "clay_mask_expand",
        )
    }

    /// Shrinks the masked region by whole cells.
    pub fn contract(&mut self, steps: i32) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_mask_contract(self.as_ptr(), steps) },
            "clay_mask_contract",
        )
    }

    /// Softens the mask's edges.
    pub fn smooth(&mut self, iterations: i32) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_mask_smooth(self.as_ptr(), iterations) },
            "clay_mask_smooth",
        )
    }

    /// Sets every cell in a world-space box.
    pub fn fill(&mut self, min: [f32; 3], max: [f32; 3], value: f32) -> Result<()> {
        // SAFETY: two three-float inputs.
        check(
            unsafe { sys::clay_mask_fill(self.as_ptr(), min.as_ptr(), max.as_ptr(), value) },
            "clay_mask_fill",
        )
    }

    /// Inverts within a box, which is how a bounded complement is expressed —
    /// an unbounded mask has no meaningful inverse.
    pub fn invert_within(&mut self, min: [f32; 3], max: [f32; 3]) -> Result<()> {
        // SAFETY: two three-float inputs.
        check(
            unsafe { sys::clay_mask_invert_within(self.as_ptr(), min.as_ptr(), max.as_ptr()) },
            "clay_mask_invert_within",
        )
    }

    /// Turns the mask into a distance field item.
    pub fn to_field(&self, threshold: f32, band: f32, pad: f32, cell_size: f32) -> Result<Item> {
        let mut item = std::ptr::null_mut();
        // SAFETY: valid handle; `item` is written only on success.
        check(
            unsafe {
                sys::clay_mask_to_field(self.as_ptr(), threshold, band, pad, cell_size, &mut item)
            },
            "clay_mask_to_field",
        )?;
        Item::from_raw(item, "clay_mask_to_field")
    }
}

impl std::fmt::Debug for MaskField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaskField")
            .field("painted", &self.painted_count().unwrap_or(0))
            .finish()
    }
}

/// A mask the caller owns, released on drop.
#[derive(Debug)]
pub struct Mask {
    inner: MaskField,
}

// SAFETY: a mask is host memory the engine reaches only through this handle.
unsafe impl Send for Mask {}

impl Mask {
    /// Creates a standalone mask.
    pub fn new(cell_size: f32) -> Result<Self> {
        // SAFETY: returns an owned handle or null.
        let raw = unsafe { sys::clay_mask_create(cell_size) };
        NonNull::new(raw)
            .map(|raw| Self {
                inner: MaskField { raw },
            })
            .ok_or_else(|| raw_failure("clay_mask_create", ErrorKind::InvalidArgument))
    }
}

impl Deref for Mask {
    type Target = MaskField;
    fn deref(&self) -> &MaskField {
        &self.inner
    }
}

impl DerefMut for Mask {
    fn deref_mut(&mut self) -> &mut MaskField {
        &mut self.inner
    }
}

impl Drop for Mask {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once. A borrowed mask is a
        // `MaskRef`, which has no `Drop`.
        unsafe { sys::clay_mask_destroy(self.inner.as_ptr()) };
    }
}

/// A mask belonging to a document, borrowed for as long as the document lives.
///
/// Carries no destroy operation, so the engine's "destroying a borrowed handle
/// is an error" is a case that cannot be written here.
#[derive(Debug)]
pub struct MaskRef<'doc> {
    inner: MaskField,
    _doc: PhantomData<&'doc mut Document>,
}

impl MaskRef<'_> {
    pub(crate) fn from_raw(raw: *mut sys::clay_mask, operation: &'static str) -> Result<Self> {
        NonNull::new(raw)
            .map(|raw| Self {
                inner: MaskField { raw },
                _doc: PhantomData,
            })
            .ok_or_else(|| raw_failure(operation, ErrorKind::NotFound))
    }
}

impl Deref for MaskRef<'_> {
    type Target = MaskField;
    fn deref(&self) -> &MaskField {
        &self.inner
    }
}

impl DerefMut for MaskRef<'_> {
    fn deref_mut(&mut self) -> &mut MaskField {
        &mut self.inner
    }
}

/// How a mask extrude leaves the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtrudeSide {
    #[default]
    Outward,
    Inward,
    Centred,
}

impl ExtrudeSide {
    fn raw(self) -> i32 {
        (match self {
            Self::Outward => sys::clay_extrude_side::CLAY_EXTRUDE_OUTWARD,
            Self::Inward => sys::clay_extrude_side::CLAY_EXTRUDE_INWARD,
            Self::Centred => sys::clay_extrude_side::CLAY_EXTRUDE_CENTRED,
        }) as i32
    }
}

/// ZBrush's Extract: the masked patch pulled off as a solid.
///
/// The mask *is* the region, so there is no radius to supply.
#[derive(Debug, Clone, Copy)]
pub struct MaskExtrudeParams {
    /// Wall thickness in world units. Must be positive.
    pub thickness: f32,
    pub side: ExtrudeSide,
    /// What counts as masked; `None` means 0.5.
    pub threshold: Option<f32>,
    /// Rounding radius on the rim; 0 is a hard edge.
    pub border_round: f32,
    /// Smoothing passes on a *copy* of the mask; the caller's is kept.
    pub border_smooth: i32,
    /// Sampling of the result; `None` means the mask's own.
    pub cell_size: Option<f32>,
}

impl Default for MaskExtrudeParams {
    fn default() -> Self {
        Self {
            thickness: 0.1,
            side: ExtrudeSide::default(),
            threshold: None,
            border_round: 0.0,
            border_smooth: 0,
            cell_size: None,
        }
    }
}

impl MaskExtrudeParams {
    pub(crate) fn to_raw(self) -> sys::clay_mask_extrude_params {
        let mut raw = sys::clay_mask_extrude_params::sized();
        raw.thickness = self.thickness;
        raw.side = self.side.raw();
        raw.threshold = self.threshold.unwrap_or(0.0);
        raw.border_round = self.border_round;
        raw.border_smooth = self.border_smooth;
        raw.cell_size = self.cell_size.unwrap_or(0.0);
        raw.band = 0.0;
        raw
    }
}

impl Document {
    /// Attaches a mask to a layer and lends it back.
    pub fn add_mask(&mut self, layer: LayerId, cell_size: f32) -> Result<MaskRef<'_>> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: valid handle; `raw` written only on success.
        check(
            unsafe { sys::clay_document_add_mask(self.as_ptr(), layer.0, cell_size, &mut raw) },
            "clay_document_add_mask",
        )?;
        MaskRef::from_raw(raw, "clay_document_add_mask")
    }

    /// The mask a layer already carries.
    pub fn mask(&mut self, layer: LayerId) -> Result<MaskRef<'_>> {
        let mut raw = std::ptr::null_mut();
        // SAFETY: valid handle; `raw` written only on success.
        check(
            unsafe { sys::clay_document_mask(self.as_ptr(), layer.0, &mut raw) },
            "clay_document_mask",
        )?;
        MaskRef::from_raw(raw, "clay_document_mask")
    }

    /// Pulls the masked patch off as a solid item.
    pub fn mask_extrude(
        &mut self,
        layer: LayerId,
        mask: &MaskField,
        params: MaskExtrudeParams,
    ) -> Result<Item> {
        let raw_params = params.to_raw();
        let mut item = std::ptr::null_mut();
        // SAFETY: all handles valid; the descriptor carries its struct_size.
        check(
            unsafe {
                sys::clay_document_mask_extrude(
                    self.as_ptr(),
                    layer.0,
                    mask.as_ptr(),
                    &raw_params,
                    &mut item,
                )
            },
            "clay_document_mask_extrude",
        )?;
        Item::from_raw(item, "clay_document_mask_extrude")
    }
}
