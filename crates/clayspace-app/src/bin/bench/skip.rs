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
    /// The adapter has no timestamp queries, so per-pass GPU time cannot be
    /// measured on it. The frame timings around it are still real.
    NoGpuTimestamps,
    /// The viewport being measured is larger than the device will allocate.
    ViewportTooLarge,
    /// The source layer states no bounds, so a crossing that rasterizes into a
    /// region has nowhere to stop.
    ///
    /// Reported rather than folded into `EditRefused` because it is the one
    /// refusal here that is a gap rather than a machine: `ClayDocument::bounds`
    /// answers for the *active* layer, a mesh layer has no SDF extent to
    /// answer with, and a grid was given a `voxel_bounds` of its own for
    /// exactly this reason while a mesh was not. Every run says so.
    NoRegionToConvertInto,
    /// This representation has no reference member to measure on.
    ///
    /// Stated rather than left out because the two are not the same thing. The
    /// brush group derives itself from `Representation::ALL` crossed with the
    /// verb table, so a representation added to the domain mints its whole
    /// family of figure names the day it lands — and if `reference.rs` has no
    /// member that builds one, those figures would simply not appear, which is
    /// the silence a performance gate exists to catch. This says out loud, on
    /// every run, that the family is unmeasured and why.
    ///
    /// It is a property of the code and not of the machine, so it fails the
    /// gate the moment a baseline recorded *with* the member goes missing it.
    NoReferenceScene,
}

impl Skip {
    /// Whether this is the machine's inability rather than the code's.
    ///
    /// The split this module's opening paragraph describes, made usable. A
    /// machine with no adapter cannot render whatever the code does, and says
    /// so on every run, so a figure it drops is not evidence of anything and
    /// never fails the gate — a developer without a GPU who sees red learns to
    /// ignore red.
    ///
    /// Everything else is a property of the code: a tool with no gesture, an
    /// operation off this representation, an edit the engine refused. Those
    /// are excused only when the baseline gave the *same* reason, because a
    /// reason that appeared since the baseline was recorded is something
    /// breaking. See `compare::missing`.
    pub const fn is_the_machine(self) -> bool {
        matches!(
            self,
            Self::NoHeadlessGpu | Self::NoBackends | Self::NoGpuTimestamps | Self::ViewportTooLarge
        )
    }

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
            Self::NoReferenceScene => "no reference member for this representation",
            Self::NoGpuTimestamps => "the adapter reports no GPU timestamps",
            Self::ViewportTooLarge => "the device will not allocate a target this large",
            Self::NoRegionToConvertInto => "the source layer states no bounds to convert within",
        }
    }
}
