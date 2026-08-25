//! Why a figure is not here.
//!
//! The distinction the gate turns on. A figure the baseline has and this run
//! does not is one of two things: a measurement that could not run on this
//! machine, which is fine, or a measurement that quietly stopped running,
//! which is the thing a performance gate exists to catch. Without a stated
//! reason the two are indistinguishable, and `let ... else { return; }` was
//! producing the second while looking like the first.
//!
//! The reasons are fixed rather than formatted, so that a skip can be compared
//! across runs and across machines.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// No adapter this process can render on.
    NoHeadlessGpu,
    /// The engine reported no usable backend.
    NoBackends,
    /// The reference document would not build.
    SceneWouldNotBuild,
    /// The surface would not mesh, so nothing could be timed.
    SurfaceWouldNotMesh,
    /// The engine refused the edit being measured.
    EditRefused,
    /// The probe found no surface to land the edit on.
    NoSurfaceUnderProbe,
    /// The brick cache would not report its statistics.
    CacheUnreadable,
    /// The tool has no gesture this harness can synthesise.
    NoGestureForTool,
    /// The operation is not reachable on this representation.
    NotOnThisRepresentation,
    /// The source layer states no bounds, so a crossing that rasterizes into a
    /// region has nowhere to stop.
    ///
    /// Reported rather than folded into `EditRefused` because it is the one
    /// refusal here that is a gap rather than a machine: `ClayDocument::bounds`
    /// answers for the *active* layer, a mesh layer has no SDF extent to
    /// answer with, and a grid was given a `voxel_bounds` of its own for
    /// exactly this reason while a mesh was not. Every run says so.
    NoRegionToConvertInto,
}

impl Skip {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::NoHeadlessGpu => "no headless GPU",
            Self::NoBackends => "no backend could be discovered",
            Self::SceneWouldNotBuild => "the reference scene would not build",
            Self::SurfaceWouldNotMesh => "the surface would not mesh",
            Self::EditRefused => "the engine refused the edit",
            Self::NoSurfaceUnderProbe => "no surface under the probe point",
            Self::CacheUnreadable => "the brick cache would not report its statistics",
            Self::NoGestureForTool => "no gesture this harness can synthesise",
            Self::NotOnThisRepresentation => "not reachable on this representation",
            Self::NoRegionToConvertInto => "the source layer states no bounds to convert within",
        }
    }
}
