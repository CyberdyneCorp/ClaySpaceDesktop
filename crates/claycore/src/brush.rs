//! Brush footprints and stroke presets.
//!
//! These are shared: the same [`BrushParams`] shapes a voxel sculpt verb and a
//! mask paint, and the same [`StrokePreset`] drives a stroke on either
//! representation. Keeping one definition is what lets a tool mean the same
//! thing wherever it is applied.

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::mask::MaskField;

/// The footprint's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrushShape {
    Cube,
    #[default]
    Sphere,
}

/// How coverage falls off toward the footprint's edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Falloff {
    /// Hard-edged, the usual brush.
    #[default]
    Constant,
    Linear,
    /// Smoothstep.
    Smooth,
    Gaussian,
}

/// The engine takes shape and falloff as loose ints on the stroke entry
/// points, rather than inside a descriptor.
pub(crate) fn shape_raw(shape: BrushShape) -> i32 {
    shape.raw()
}

pub(crate) fn falloff_raw(falloff: Falloff) -> i32 {
    falloff.raw()
}

impl BrushShape {
    pub(crate) fn raw(self) -> i32 {
        (match self {
            Self::Cube => sys::clay_brush_shape::CLAY_BRUSH_SHAPE_CUBE,
            Self::Sphere => sys::clay_brush_shape::CLAY_BRUSH_SHAPE_SPHERE,
        }) as i32
    }
}

impl Falloff {
    pub(crate) fn raw(self) -> i32 {
        (match self {
            Self::Constant => sys::clay_brush_falloff::CLAY_BRUSH_FALLOFF_CONSTANT,
            Self::Linear => sys::clay_brush_falloff::CLAY_BRUSH_FALLOFF_LINEAR,
            Self::Smooth => sys::clay_brush_falloff::CLAY_BRUSH_FALLOFF_SMOOTH,
            Self::Gaussian => sys::clay_brush_falloff::CLAY_BRUSH_FALLOFF_GAUSSIAN,
        }) as i32
    }
}

/// One brush dab's footprint.
///
/// Where a mask is given, the effective weight is scaled by `1 - mask`, so a
/// fully masked cell is untouched by *every* verb rather than by a hand-picked
/// few.
#[derive(Debug, Clone, Copy)]
pub struct BrushParams<'mask> {
    /// Cells the footprint spans per axis. Must be positive.
    pub size: i32,
    pub shape: BrushShape,
    pub falloff: Falloff,
    /// Coverage multiplier. At or above 1 the brush is at full strength.
    pub strength: f32,
    /// Dither seed, so a dithered stamp is reproducible.
    pub seed: u32,
    /// A mask that freezes part of the footprint.
    ///
    /// Taken by shared reference so that either an owned [`crate::Mask`] or a
    /// document's [`crate::MaskRef`] can be passed, both dereferencing here.
    pub mask: Option<&'mask MaskField>,
}

impl Default for BrushParams<'_> {
    fn default() -> Self {
        Self {
            size: 4,
            shape: BrushShape::default(),
            falloff: Falloff::default(),
            strength: 1.0,
            seed: 0,
            mask: None,
        }
    }
}

impl BrushParams<'_> {
    pub(crate) fn to_raw(&self) -> sys::clay_brush_params {
        let mut raw = sys::clay_brush_params::sized();
        raw.size = self.size;
        raw.shape = self.shape.raw();
        raw.falloff = self.falloff.raw();
        raw.strength = self.strength;
        raw.seed = self.seed;
        raw.mask = self.mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _);
        raw
    }
}

/// How overlapping stamps combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Accumulation {
    /// Overlapping stamps deposit twice — ZBrush's buildup.
    Buildup,
    /// Each stamp is limited to its own depth.
    #[default]
    Clamped,
}

impl Accumulation {
    fn raw(self) -> i32 {
        (match self {
            Self::Buildup => sys::clay_accumulation::CLAY_ACCUMULATION_BUILDUP,
            Self::Clamped => sys::clay_accumulation::CLAY_ACCUMULATION_CLAMPED,
        }) as i32
    }
}

/// One sample along a stroke, as the input device reported it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeSample {
    pub position: [f32; 3],
    /// Reported pressure, normally in `0..=1`.
    pub pressure: f32,
    /// Seconds since the stroke began.
    pub time: f32,
}

impl StrokeSample {
    /// A sample at full pressure, for input without a pressure axis.
    pub fn at(position: [f32; 3], time: f32) -> Self {
        Self {
            position,
            pressure: 1.0,
            time,
        }
    }

    /// The engine takes samples as flat `xyzpt` quintuples.
    pub(crate) fn flatten(samples: &[Self]) -> Vec<f32> {
        let mut flat = Vec::with_capacity(samples.len() * 5);
        for s in samples {
            flat.extend_from_slice(&s.position);
            flat.push(s.pressure);
            flat.push(s.time);
        }
        flat
    }
}

/// How a drag becomes a series of stamps.
///
/// Spacing follows arc length rather than sample count, so a fast drag and a
/// slow one over the same path deposit the same stamps.
#[derive(Debug, Clone, Copy)]
pub struct StrokePreset {
    /// World units. Must be positive.
    pub radius: f32,
    /// Stamp spacing as a fraction of the diameter.
    pub spacing: f32,
    pub strength: f32,
    /// How far pressure drives radius; 0 disconnects it.
    pub pressure_size: f32,
    /// How far pressure drives strength; 0 disconnects it.
    pub pressure_strength: f32,
    /// Exponent applied to pressure before either.
    pub pressure_curve: f32,
    /// Positional jitter as a fraction of the radius.
    pub jitter_position: f32,
    /// Size jitter as a fraction of the radius.
    pub jitter_size: f32,
    /// Rotational jitter, radians.
    pub jitter_rotation: f32,
    /// Jitter is a hash of the stamp index and this, so it is reproducible.
    pub seed: u32,
    pub rotate_along_stroke: bool,
    /// Fraction of the stroke the radius ramps in over.
    pub taper_start: f32,
    pub taper_end: f32,
    /// Lazy-mouse lag: 0 follows exactly, toward 1 lags more.
    pub steady: f32,
    pub accumulation: Accumulation,
}

impl Default for StrokePreset {
    fn default() -> Self {
        // The engine's own defaults, asked for rather than restated.
        let mut raw = sys::clay_stroke_preset::sized();
        // SAFETY: a valid versioned descriptor out-parameter.
        let _ = unsafe { sys::clay_stroke_preset_defaults(&mut raw) };
        Self {
            radius: raw.radius,
            spacing: raw.spacing,
            strength: raw.strength,
            pressure_size: raw.pressure_size,
            pressure_strength: raw.pressure_strength,
            pressure_curve: raw.pressure_curve,
            jitter_position: raw.jitter_position,
            jitter_size: raw.jitter_size,
            jitter_rotation: raw.jitter_rotation,
            seed: raw.seed,
            rotate_along_stroke: raw.rotate_along_stroke != 0,
            taper_start: raw.taper_start,
            taper_end: raw.taper_end,
            steady: raw.steady,
            accumulation: if raw.accumulation
                == sys::clay_accumulation::CLAY_ACCUMULATION_BUILDUP as i32
            {
                Accumulation::Buildup
            } else {
                Accumulation::Clamped
            },
        }
    }
}

impl StrokePreset {
    pub(crate) fn to_raw(&self) -> sys::clay_stroke_preset {
        let mut raw = sys::clay_stroke_preset::sized();
        raw.radius = self.radius;
        raw.spacing = self.spacing;
        raw.strength = self.strength;
        raw.pressure_size = self.pressure_size;
        raw.pressure_strength = self.pressure_strength;
        raw.pressure_curve = self.pressure_curve;
        raw.jitter_position = self.jitter_position;
        raw.jitter_size = self.jitter_size;
        raw.jitter_rotation = self.jitter_rotation;
        raw.seed = self.seed;
        raw.rotate_along_stroke = i32::from(self.rotate_along_stroke);
        raw.taper_start = self.taper_start;
        raw.taper_end = self.taper_end;
        raw.steady = self.steady;
        raw.accumulation = self.accumulation.raw();
        raw
    }
}

// The stroke preset is versioned by the engine, so it must carry struct_size.
// SAFETY: bindgen output for a header struct beginning with `uint32_t
// struct_size`.
unsafe impl Descriptor for sys::clay_stroke_preset {}
// SAFETY: as above.
unsafe impl Descriptor for sys::clay_brush_params {}
// SAFETY: as above.
unsafe impl Descriptor for sys::clay_repair_report {}
// SAFETY: as above.
unsafe impl Descriptor for sys::clay_mask_extrude_params {}
