//! What the surface *is* at a point.

use claycore_sys as sys;

use crate::descriptor::Descriptor;

// SAFETY: generated from the engine header, `#[repr(C)]`, and its first field
// is the `uint32_t struct_size` the engine reads to know which version of the
// descriptor it was handed.
unsafe impl Descriptor for sys::clay_measure_params {}

/// Which property of the surface to measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceMeasure {
    /// The magnitude of the field's Laplacian: anywhere the surface bends.
    ///
    /// **A saturated `[0, 1]` masking value**, not a curvature in physical
    /// units. A retopology engine's curvature is signed mean curvature in
    /// `1/length`; the two are different quantities and substituting one for
    /// the other is wrong rather than merely rescaled.
    Curvature,
    /// Concave only — crevices, folds, seams.
    Cavity,
    /// Convex only — hard edges, ridges.
    Convexity,
    /// Agreement with a direction: "facing up".
    NormalDirection,
    /// How enclosed a point is: the blocked fraction of a hemisphere around
    /// the normal, within the ray length.
    ///
    /// **0 is open sky and 1 is fully enclosed** — occlusion, not lighting.
    /// The greater number is the darker place. Tools disagree about which way
    /// this runs, which is why the engine states it and why a host feeding a
    /// baker that wants *openness* must invert.
    Occlusion,
    /// Material behind the surface: the distance inward to where the field
    /// turns positive again, over the ray length. 1 is thicker than the probe
    /// could see.
    Thickness,
}

impl SurfaceMeasure {
    pub(crate) fn to_raw(self) -> sys::clay_surface_measure::Type {
        match self {
            Self::Curvature => sys::clay_surface_measure::CLAY_MEASURE_CURVATURE,
            Self::Cavity => sys::clay_surface_measure::CLAY_MEASURE_CAVITY,
            Self::Convexity => sys::clay_surface_measure::CLAY_MEASURE_CONVEXITY,
            Self::NormalDirection => sys::clay_surface_measure::CLAY_MEASURE_NORMAL_DIR,
            Self::Occlusion => sys::clay_surface_measure::CLAY_MEASURE_OCCLUSION,
            Self::Thickness => sys::clay_surface_measure::CLAY_MEASURE_THICKNESS,
        }
    }
}

/// How to measure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasureParams {
    /// Sampling step for the derivative, in world units. `None` takes the
    /// engine's own.
    pub h: Option<f32>,
    /// Scales the result.
    pub scale: Option<f32>,
    /// The direction `NormalDirection` is measured against.
    pub direction: [f32; 3],
    pub threshold: Option<f32>,
    /// How far an occlusion or thickness ray looks.
    pub ray_length: Option<f32>,
    /// Hemisphere samples. More is smoother and slower.
    pub ray_count: Option<i32>,
    pub falloff: Option<f32>,
    /// Fixed, because the hemisphere pattern is rotated by a hash of the point
    /// and this: same seed, same bits, every backend and every run.
    pub seed: u32,
}

impl Default for MeasureParams {
    fn default() -> Self {
        Self {
            h: None,
            scale: None,
            direction: [0.0, 1.0, 0.0],
            threshold: None,
            ray_length: None,
            ray_count: None,
            falloff: None,
            seed: 0,
        }
    }
}

impl MeasureParams {
    /// Occlusion within a radius, which is the shape a baker asks for.
    pub fn occlusion(ray_length: f32, ray_count: i32) -> Self {
        Self {
            ray_length: Some(ray_length),
            ray_count: Some(ray_count),
            ..Self::default()
        }
    }

    pub(crate) fn to_raw(self) -> sys::clay_measure_params {
        let mut raw = sys::clay_measure_params::sized();
        // Zero on a field means the engine's own default, as everywhere else
        // in this ABI, so `None` is written as zero rather than guessed at.
        raw.h = self.h.unwrap_or(0.0);
        raw.scale = self.scale.unwrap_or(0.0);
        raw.direction = self.direction;
        raw.threshold = self.threshold.unwrap_or(0.0);
        raw.ray_length = self.ray_length.unwrap_or(0.0);
        raw.ray_count = self.ray_count.unwrap_or(0);
        raw.falloff = self.falloff.unwrap_or(0.0);
        raw.seed = self.seed;
        raw
    }
}
