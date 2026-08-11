//! The scene the budgets are measured against, and the conditions they held in.
//!
//! The specification is explicit that a budget asserted against an unspecified
//! scene is not a budget. So the reference document is built here, from code,
//! deterministically — no fixture file to drift, and the same shape on every
//! machine — and every figure the benchmark reports carries the platform, the
//! backend, the engine version and the resolution it was taken at.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, ModelError, SculptModel, ToolKind};

/// What a set of figures was measured on.
///
/// Reported alongside every number. A latency without these is a number
/// without a claim: the same code is inside budget on one machine and outside
/// it on another, and comparing two runs that do not name their conditions is
/// how a performance gate starts lying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conditions {
    /// Which reference document, by name and revision.
    pub scene: String,
    /// `macos` or `linux`, as the build target reports it.
    pub platform: &'static str,
    /// `aarch64`, `x86_64`.
    pub architecture: &'static str,
    /// The backend the engine selected, not the one that was compiled in.
    pub backend: String,
    /// The engine actually linked.
    pub engine: String,
    /// Where the numbers came from — an offscreen target of this size.
    pub viewport: (u32, u32),
}

impl Conditions {
    pub fn describe(&self) -> String {
        format!(
            "{} on {}/{}, backend {}, engine {}, {}x{}",
            self.scene,
            self.platform,
            self.architecture,
            self.backend,
            self.engine,
            self.viewport.0,
            self.viewport.1
        )
    }
}

/// Which reference document to build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// The one the budgets are stated against.
    Reference,
    /// The same shape with roughly ten times the surface area, for checking
    /// that an edit's cost follows the edit rather than the document.
    TenTimesLarger,
}

impl Scene {
    /// A name that goes into the report, and changes when the scene does.
    ///
    /// The revision is part of it on purpose: comparing today's figures
    /// against a baseline taken on a different scene is worse than having no
    /// baseline, and this is what makes that visible instead of silent.
    pub fn name(self) -> &'static str {
        match self {
            // Bump the revision whenever `build` changes shape.
            Self::Reference => "reference-r1",
            Self::TenTimesLarger => "reference-10x-r1",
        }
    }

    /// The starting form's radius.
    ///
    /// Surface area goes with the square of the radius, so ten times the area
    /// is the square root of ten times the radius.
    fn radius(self) -> f32 {
        match self {
            Self::Reference => 1.0,
            Self::TenTimesLarger => 10.0f32.sqrt(),
        }
    }

    /// How many strokes are laid onto it.
    ///
    /// The same count for both: the point of the larger scene is more surface
    /// at the same edit density, not more editing.
    const STROKES: usize = 8;
    const SAMPLES_PER_STROKE: usize = 12;

    /// Builds the document. Deterministic: no clock, no randomness, no file.
    pub fn build(self, policy: BackendPolicy) -> Result<ClayDocument, ModelError> {
        let radius = self.radius();
        let mut document = ClayDocument::new(policy)?;
        document.add_starting_sphere(radius)?;

        // A band of strokes around the form, so the surface is not a bare
        // primitive: a dab on a sphere touches fewer bricks than a dab on
        // something that has been worked, and the budget should describe the
        // second.
        let brush = BrushSettings {
            size: 0.18 * radius,
            ..BrushSettings::default()
        };
        for stroke in 0..Self::STROKES {
            let band = (stroke as f32 / Self::STROKES as f32 - 0.5) * 1.2;
            let samples: Vec<GestureSample> = (0..Self::SAMPLES_PER_STROKE)
                .map(|i| {
                    let t = i as f32 / (Self::SAMPLES_PER_STROKE - 1) as f32;
                    let angle = (t - 0.5) * 1.4;
                    let (s, c) = angle.sin_cos();
                    let (sb, cb) = band.sin_cos();
                    GestureSample {
                        position: [
                            s * cb * radius * 1.01,
                            sb * radius * 1.01,
                            c * cb * radius * 1.01,
                        ],
                        pressure: 1.0,
                        time: t,
                    }
                })
                .collect();
            document.apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])?;
        }
        // Nothing is left pending: a benchmark that starts with the scene's own
        // construction still in the dirty set measures the construction.
        document.take_dirty_keys();
        Ok(document)
    }

    /// The brush the dab measurements use, scaled to the scene.
    ///
    /// Proportional to the form, because a brush that is a tenth of a small
    /// model and a hundredth of a large one is not the same tool.
    pub fn brush(self) -> BrushSettings {
        BrushSettings {
            size: 0.18 * self.radius(),
            ..BrushSettings::default()
        }
    }

    /// The brush the *locality* measurement uses, which does not scale.
    ///
    /// The requirement is that "the same small edit" costs the same on a large
    /// document as on a small one. Scaling the brush with the scene was tried
    /// and measures something else entirely: at ten times the surface area the
    /// radius is √10 larger, the influence volume some thirty times larger,
    /// and the ratio came out at 41 — a number about the brush, not about
    /// whether cost follows the edit.
    pub fn probe_brush() -> BrushSettings {
        BrushSettings {
            size: 0.18,
            ..BrushSettings::default()
        }
    }

    /// A stroke across the front of the form, as a drag delivers it.
    pub fn stroke(self, samples: usize) -> Vec<GestureSample> {
        let radius = self.radius();
        (0..samples)
            .map(|i| {
                let t = i as f32 / (samples.max(2) - 1) as f32;
                let angle = (t - 0.5) * 1.2;
                let (s, c) = angle.sin_cos();
                GestureSample {
                    position: [s * radius * 1.01, 0.1 * radius, c * radius * 1.01],
                    pressure: 1.0,
                    time: t,
                }
            })
            .collect()
    }
}

/// The conditions of the machine this is running on.
pub fn conditions(scene: Scene, policy: &BackendPolicy, viewport: (u32, u32)) -> Conditions {
    Conditions {
        scene: scene.name().to_string(),
        platform: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        backend: policy.active().to_string(),
        engine: clayspace_engine::claycore::version().to_string(),
        viewport,
    }
}
