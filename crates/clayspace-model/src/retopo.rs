//! Retopology, UV layout and baking: the second half of the pipeline.
//!
//! The domain's side of the seam with the retopology engine. No engine types
//! reach here, so these are testable with neither engine present — which is
//! the point of the layer and the reason the ViewModels can be exercised
//! without a compiled C++ library.

/// Which quadrangulator runs.
///
/// **No routing rule of our own.** The engine's authors record that choosing
/// per input by measuring both solvers "is still open work", so this offers the
/// choice and takes their default rather than inventing a threshold we cannot
/// defend with a measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuadMethod {
    /// QuadCover seamless-UV isoline extraction. The engine's own default.
    #[default]
    QuadCover,
    /// The ZRemesher-class track: the same field, solve and extraction, plus
    /// the explicit topology-layout stage that makes edge-loop structure a
    /// first-class artifact rather than an emergent consequence.
    ZRemesher,
    /// Max-matching over a smoothed cross field. Strongest on box and CAD
    /// geometry.
    FieldAligned,
    /// Instant-Meshes-style position-field extraction.
    InstantMeshes,
    /// Experimental integer parametrisation.
    Integer,
}

impl QuadMethod {
    pub const ALL: [QuadMethod; 5] = [
        Self::QuadCover,
        Self::ZRemesher,
        Self::FieldAligned,
        Self::InstantMeshes,
        Self::Integer,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::QuadCover => "QuadCover",
            Self::ZRemesher => "ZRemesher",
            Self::FieldAligned => "Alinhado ao campo",
            Self::InstantMeshes => "Instant Meshes",
            Self::Integer => "Inteiro",
        }
    }

    /// What a sculptor is choosing between, in one line each.
    pub fn hint(self) -> &'static str {
        match self {
            Self::QuadCover => "O padrão do motor. Isolinhas de um campo cruzado sem costuras.",
            Self::ZRemesher => {
                "Acrescenta a etapa de layout: onde ficam os anéis de aresta e \
                 as singularidades deixa de ser consequência e passa a ser \
                 resultado."
            }
            Self::FieldAligned => "Mais forte em geometria de caixa e CAD.",
            Self::InstantMeshes => "Extractor por campo de posições.",
            Self::Integer => "Experimental: parametrização inteira.",
        }
    }
}

/// What to ask a retopology for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetopoSettings {
    /// Quads to aim at. **Approached and never hit** — the engine searches
    /// over edge length rather than solving for a count, and says so.
    pub target_quads: u32,
    pub method: QuadMethod,
    /// Hold hard edges below this dihedral angle as features.
    pub sharp_edge_degrees: f32,
    /// Subdivide and relax onto the surface until the result is 100% quads,
    /// rather than leaving it quad-dominant.
    pub pure_quads: bool,
    /// 0 uniform, 1 fully curvature-adaptive.
    pub adaptivity: f32,
}

impl Default for RetopoSettings {
    /// 2000 quads, the engine's default solver, features held at 40 degrees.
    ///
    /// A first retopology of a sculpt wants enough quads to keep the form
    /// recognisable and few enough to be worth having done — and the number is
    /// the one a sculptor moves first, so it is a round one.
    fn default() -> Self {
        Self {
            target_quads: 2000,
            method: QuadMethod::QuadCover,
            sharp_edge_degrees: 40.0,
            pure_quads: true,
            adaptivity: 0.0,
        }
    }
}

impl RetopoSettings {
    /// The coarsest and finest the interface offers.
    ///
    /// The floor is where a retopology stops describing the form; the ceiling
    /// is where the search over edge length costs more than the sculptor's
    /// patience. Both are judgements about usefulness, and the engine states
    /// no limits of its own.
    pub const TARGET_QUADS: std::ops::RangeInclusive<u32> = 100..=100_000;

    pub fn sanitized(self) -> Self {
        Self {
            target_quads: self
                .target_quads
                .clamp(*Self::TARGET_QUADS.start(), *Self::TARGET_QUADS.end()),
            sharp_edge_degrees: self.sharp_edge_degrees.clamp(0.0, 180.0),
            adaptivity: self.adaptivity.clamp(0.0, 1.0),
            ..self
        }
    }
}

/// What a retopology came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetopoOutcome {
    pub triangles_before: usize,
    /// Faces the engine made: quads where it made quads.
    pub faces: usize,
    /// The triangulation of those faces, which is what a renderer draws.
    ///
    /// Carried beside the faces rather than instead of them, so a host that
    /// ignores the quads draws the mesh it always drew. `faces < triangles` is
    /// what says quads were produced at all.
    pub triangles: usize,
    pub vertices: usize,
}

impl RetopoOutcome {
    /// Whether the result is quads rather than triangles.
    ///
    /// Two triangles to a quad, so a quad mesh reports fewer faces than
    /// triangles where a triangle mesh reports exactly as many. This is the
    /// only observable that distinguishes them.
    pub fn is_quads(&self) -> bool {
        self.faces < self.triangles
    }

    /// The share of faces that are quads rather than triangles.
    ///
    /// A quad-dominant result is the ordinary output; `pure_quads` asks for
    /// 1.0. Derived rather than reported by the engine, so it is stated as
    /// what it is: two triangles make a quad, and the residue is triangles.
    pub fn quad_share(&self) -> f32 {
        if self.faces == 0 {
            return 0.0;
        }
        let quads = self.triangles.saturating_sub(self.faces);
        quads as f32 / self.faces as f32
    }
}

/// A surface on its way to the retopologiser, owned and thread-safe.
///
/// **Why this exists as a type.** The heavy work runs off the interface
/// thread, and the document cannot go with it: it is held in an `Rc<RefCell>`
/// by design, so that every ViewModel can reach it without a lock on the one
/// thread that ever touches it. So the operation is three steps rather than
/// one — read owned geometry here, retopologise on a worker, place the result
/// back on the interface thread — and this is the first step's product.
#[derive(Debug, Clone, PartialEq)]
pub struct RetopoSource {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// What the subtool is called, so the result can be named beside it
    /// without reaching back into a document on another thread.
    pub name: String,
}

/// What came back, owned and thread-safe, ready to be placed.
#[derive(Debug, Clone, PartialEq)]
pub struct RetopoResult {
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub outcome: RetopoOutcome,
    pub name: String,
}

/// The heavy middle, which is neither the document's nor the interface's.
///
/// A trait so that the ViewModel can run a retopology without depending on a
/// retopology engine — the layering rule this workspace enforces with a
/// script. The implementation lives beside the engines; the ViewModel holds
/// one of these and knows nothing else about it.
///
/// `Send + Sync` because it is called from a worker thread, and shared across
/// however many retopologies a session asks for.
pub trait Retopologiser: Send + Sync {
    /// Rebuilds the topology. Called on a worker thread.
    ///
    /// `progress` is advisory and may be ignored; `cancelled` is asked
    /// between stages and a `true` stops the run.
    fn run(
        &self,
        source: &RetopoSource,
        settings: RetopoSettings,
        progress: &dyn Fn(f32, &str),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RetopoResult, String>;
}

/// Retopologising a mesh subtool.
pub trait RetopoModel {
    /// Whether the active subtool can be retopologised, and why not if it
    /// cannot.
    fn can_retopologise(&self) -> Result<(), String>;

    /// Rebuilds the active mesh subtool's topology as quads, arriving as a
    /// **new subtool** beside the source.
    ///
    /// A retopology a sculptor cannot compare against the sculpt is one they
    /// cannot judge, and replacing the source is a decision that cannot be
    /// undone by looking at it.
    fn retopologise(
        &mut self,
        settings: RetopoSettings,
    ) -> Result<RetopoOutcome, crate::ModelError>;

    /// Reads the active mesh subtool out as owned geometry, for a retopology
    /// that will run off this thread.
    fn retopo_source(&mut self) -> Result<RetopoSource, crate::ModelError>;

    /// Places a finished retopology as a new subtool beside its source, in one
    /// undo entry.
    fn place_retopology(&mut self, result: &RetopoResult) -> Result<(), crate::ModelError>;
}

// -- UV ---------------------------------------------------------------------

/// What a UV layout is asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvSettings {
    /// How far a chart's normals may spread before it is cut.
    pub max_chart_angle_degrees: f32,
    /// Gutter between charts, in UV units.
    pub pack_margin: f32,
    /// The texture the density is stated against.
    pub texture_size: u32,
    /// Turn each chart onto its minimum-area box, which roughly doubles usable
    /// coverage on box-like meshes: a 45-degree diamond face becomes an
    /// axis-aligned square.
    pub reorient_charts: bool,
    /// Fold adjacent charts together where it costs no distortion, then again
    /// where it costs less than the bound below.
    pub merge_charts: bool,
    pub max_chart_distortion: f32,
}

impl Default for UvSettings {
    fn default() -> Self {
        Self {
            max_chart_angle_degrees: 66.0,
            pack_margin: 0.005,
            texture_size: 2048,
            reorient_charts: true,
            merge_charts: true,
            max_chart_distortion: 0.15,
        }
    }
}

impl UvSettings {
    pub const TEXTURE_SIZES: [u32; 5] = [512, 1024, 2048, 4096, 8192];

    pub fn sanitized(self) -> Self {
        Self {
            max_chart_angle_degrees: self.max_chart_angle_degrees.clamp(1.0, 180.0),
            pack_margin: self.pack_margin.clamp(0.0, 0.1),
            max_chart_distortion: self.max_chart_distortion.clamp(0.0, 1.0),
            ..self
        }
    }
}

/// What a layout came to.
///
/// Every figure is the engine's own and none is reduced to a verdict: a UV
/// layout is a trade a person judges, and a sculptor deciding whether to re-cut
/// wants the distortion and the coverage rather than a pass or a fail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvOutcome {
    pub charts: u32,
    pub seam_edges: usize,
    /// Conformal (angle) error, where 0 is angle-preserving.
    pub max_angle_distortion: f32,
    pub rms_angle_distortion: f32,
    /// Charts whose parameterisation turned inside out. **Non-zero is a defect
    /// in the layout**, not a quality figure, which is why it is reported
    /// separately from the distortion.
    pub flipped_charts: u32,
    pub dropped_charts: u32,
    /// The fraction of the UV square the chart *geometry* covers.
    pub packed_area: f32,
    /// The fraction its bounding boxes cover — the looser number, reported
    /// apart because the two are easy to confuse for one another.
    pub packed_box_area: f32,
    pub texel_density: f32,
}

/// A surface on its way to the UV stage, owned and thread-safe.
#[derive(Debug, Clone, PartialEq)]
pub struct UvSource {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub name: String,
}

/// What came back.
#[derive(Debug, Clone, PartialEq)]
pub struct UvResult {
    pub outcome: UvOutcome,
    pub name: String,
}

/// The UV stage, off the interface thread.
pub trait Unwrapper: Send + Sync {
    fn run(
        &self,
        source: &UvSource,
        settings: UvSettings,
        progress: &dyn Fn(f32, &str),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<UvResult, String>;
}

/// Laying out a mesh subtool's UVs.
pub trait UvModel {
    /// Whether the active subtool can be unwrapped, and why not if it cannot.
    fn can_unwrap(&self) -> Result<(), String>;

    /// Reads the active mesh subtool out for a layout that will run off this
    /// thread.
    fn uv_source(&mut self) -> Result<UvSource, crate::ModelError>;

    /// Records what the layout came to.
    ///
    /// **The UVs themselves stay in the retopology engine.** ClayCore's mesh
    /// layers carry no UV attribute, so writing them back would mean inventing
    /// one — and a layout that lives in two places is a layout that can
    /// disagree with itself. What crosses back is the report a sculptor judges
    /// the layout by; the atlas is written out at export, from the engine that
    /// holds it.
    fn record_uv(&mut self, result: &UvResult) -> Result<(), crate::ModelError>;
}

// -- baking -----------------------------------------------------------------

/// Which map to bake.
///
/// The four a **field** can answer. Every other map the engine offers needs a
/// high-poly target mesh and takes the raycast path; these four are the ones
/// this application can supply from ClayCore's own field, which is the half of
/// the pipeline no other host has been able to fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BakeMap {
    Normal,
    AmbientOcclusion,
    Curvature,
    Cavity,
}

impl BakeMap {
    pub const ALL: [BakeMap; 4] = [
        Self::Normal,
        Self::AmbientOcclusion,
        Self::Curvature,
        Self::Cavity,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normais",
            Self::AmbientOcclusion => "Oclusão ambiente",
            Self::Curvature => "Curvatura",
            Self::Cavity => "Cavidade",
        }
    }

    /// The file this map is written as, under the chosen stem.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::AmbientOcclusion => "ao",
            Self::Curvature => "curvature",
            Self::Cavity => "cavity",
        }
    }
}

/// What a bake is asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct BakeSettings {
    /// Which maps to write. Empty asks for nothing and is refused rather than
    /// treated as "all of them".
    pub maps: Vec<BakeMap>,
    pub size: u32,
    /// How far off the surface the cage sits. **Sphere-traced through the
    /// field** rather than intersected against triangles, which is what
    /// removes the cage-ray misses a tessellated high-poly produces at a seam.
    pub cage_distance: f32,
    /// Hemisphere samples for ambient occlusion.
    pub ao_samples: u32,
    pub ao_radius: f32,
}

impl Default for BakeSettings {
    fn default() -> Self {
        Self {
            maps: vec![BakeMap::Normal, BakeMap::AmbientOcclusion],
            size: 2048,
            cage_distance: 0.02,
            ao_samples: 64,
            ao_radius: 0.5,
        }
    }
}

impl BakeSettings {
    pub const SIZES: [u32; 5] = [512, 1024, 2048, 4096, 8192];

    pub fn sanitized(mut self) -> Self {
        self.maps.dedup();
        Self {
            cage_distance: self.cage_distance.clamp(0.0, 1.0),
            ao_samples: self.ao_samples.clamp(4, 1024),
            ao_radius: self.ao_radius.clamp(0.01, 10.0),
            ..self
        }
    }
}

/// One written map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedMap {
    pub map: BakeMap,
    pub path: std::path::PathBuf,
    pub width: u32,
    pub height: u32,
}

/// What a bake produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakeResult {
    pub written: Vec<BakedMap>,
    /// Maps that were asked for and refused, with the reason. Carried rather
    /// than collapsed into a failure: three maps written and one refused is a
    /// useful outcome and reporting it as an error would throw the three away.
    pub refused: Vec<(BakeMap, String)>,
}

/// The baking stage, off the interface thread.
///
/// Takes the geometry *and* a field: the field is what makes these four maps
/// answerable without a high-poly mesh, and it is the only reason this
/// application can offer them at all.
pub trait Baker: Send + Sync {
    fn run(
        &self,
        source: &UvSource,
        settings: &BakeSettings,
        into: &std::path::Path,
        progress: &dyn Fn(f32, &str),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<BakeResult, String>;
}

/// Baking a mesh subtool's maps.
pub trait BakeModel {
    /// Whether the active subtool can be baked, and why not if it cannot.
    fn can_bake(&self) -> Result<(), String>;
}

// -- conform ----------------------------------------------------------------

/// What a conform came to.
///
/// **Every figure here reaches the sculptor.** The engine completes and flags
/// rather than refusing or silently stretching, so a host that drops these
/// turns "it finished, and here is where it struggled" back into "it finished".
#[derive(Debug, Clone, PartialEq)]
pub struct ConformOutcome {
    pub moved_vertices: usize,
    /// The furthest any vertex had to travel to reach the new surface.
    pub max_deviation: f32,
    pub rms_deviation: f32,
    /// How many moved further than the threshold. May exceed the list below,
    /// which is bounded — a conform that flagged ten thousand says so rather
    /// than handing back the first hundred as though that were all of them.
    pub flagged_count: usize,
    pub flagged_returned: usize,
}

impl ConformOutcome {
    /// Whether anything moved further than the sculptor allowed.
    pub fn struggled(&self) -> bool {
        self.flagged_count > 0
    }
}

/// What a conform is asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConformSettings {
    /// How far a vertex may travel before it is worth naming, in world units.
    pub threshold: f32,
}

impl Default for ConformSettings {
    fn default() -> Self {
        Self { threshold: 0.05 }
    }
}

impl ConformSettings {
    pub fn sanitized(self) -> Self {
        Self {
            threshold: self.threshold.clamp(0.0001, 10.0),
        }
    }
}

/// A conform's two surfaces and where the result goes.
#[derive(Debug, Clone, PartialEq)]
pub struct ConformSource {
    /// The retopologised mesh, whose topology is preserved exactly.
    pub edit: UvSource,
    /// The sculpt as it is now.
    pub target: UvSource,
}

/// What came back: the moved vertices, and the report.
#[derive(Debug, Clone, PartialEq)]
pub struct ConformResult {
    pub positions: Vec<[f32; 3]>,
    pub outcome: ConformOutcome,
}

/// The conform stage, off the interface thread.
pub trait Conformer: Send + Sync {
    fn run(
        &self,
        source: &ConformSource,
        settings: ConformSettings,
        progress: &dyn Fn(f32, &str),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ConformResult, String>;
}

/// Re-snapping a retopologised subtool onto a sculpt that has moved.
pub trait ConformModel {
    /// Whether a conform can be run, and why not if it cannot.
    ///
    /// It needs **two** subtools — the retopologised mesh and the sculpt it
    /// came from — so the refusal has more to say than the other three tools'
    /// does.
    fn can_conform(&self) -> Result<(), String>;

    /// Reads both surfaces out for a conform that will run off this thread.
    fn conform_source(&mut self) -> Result<ConformSource, crate::ModelError>;

    /// Moves the active subtool's vertices onto the new surface.
    ///
    /// **Topology is preserved**, so this writes positions into the existing
    /// subtool rather than adding one — which is the opposite of what a
    /// retopology does, and deliberately: a conform is a correction to a mesh
    /// the sculptor already accepted, not a new candidate to compare.
    fn apply_conform(&mut self, result: &ConformResult) -> Result<(), crate::ModelError>;
}
