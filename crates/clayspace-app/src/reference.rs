//! The scene the budgets are measured against, and the conditions they held in.
//!
//! The specification is explicit that a budget asserted against an unspecified
//! scene is not a budget. So the reference document is built here, from code,
//! deterministically — no fixture file to drift, and the same shape on every
//! machine — and every figure the benchmark reports carries the platform, the
//! backend, the engine version and the resolution it was taken at.

use std::collections::BTreeMap;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, GestureSample, ModelError, Representation, SceneModel, SculptModel,
    ToolKind,
};

/// What a set of figures was measured on.
///
/// Reported alongside every number. A latency without these is a number
/// without a claim: the same code is inside budget on one machine and outside
/// it on another, and comparing two runs that do not name their conditions is
/// how a performance gate starts lying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conditions {
    /// Which reference documents, by member and revision.
    ///
    /// A map rather than one name, because the suite is one scene per
    /// representation and a voxel verb has nowhere to land on an SDF
    /// document. Per member rather than one revision for the whole suite: a
    /// suite-wide revision has to be bumped by hand whenever any member
    /// changes, and forgetting to bump it is the exact mistake this field
    /// exists to prevent.
    pub scenes: BTreeMap<&'static str, &'static str>,
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
            self.scenes_described(),
            self.platform,
            self.architecture,
            self.backend,
            self.engine,
            self.viewport.0,
            self.viewport.1
        )
    }

    /// The suite, as one line: `reference-r1, reference-10x-r1`.
    pub fn scenes_described(&self) -> String {
        self.scenes
            .iter()
            .map(|(member, revision)| format!("{member}-{revision}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Which reference document to build.
///
/// One per representation, because a verb has nowhere to land otherwise: a
/// voxel brush cannot be measured on a field, and a mesh brush cannot be
/// measured on either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// The one the budgets are stated against.
    Reference,
    /// The same shape with roughly ten times the surface area, for checking
    /// that an edit's cost follows the edit rather than the document.
    TenTimesLarger,
    /// A worked grid, at the cell size a first crossing lands at.
    VoxelReference,
    /// The reference form marched into triangles, which is the route a mesh
    /// layer takes into a document without a file.
    MeshReference,
    /// A rasterized ball with a pocket sealed inside it and a channel bored
    /// through its shell.
    ///
    /// The pre-bake repairs have nothing to do on a solid grid, and a figure
    /// for closing no holes measures the check rather than the repair. This is
    /// the member with something wrong with it.
    VoxelPocked,
}

impl Scene {
    /// Every member of the suite.
    pub const ALL: [Scene; 5] = [
        Self::Reference,
        Self::TenTimesLarger,
        Self::VoxelReference,
        Self::MeshReference,
        Self::VoxelPocked,
    ];

    /// What this member is called, stably, across revisions.
    pub fn member(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::TenTimesLarger => "reference-10x",
            Self::VoxelReference => "voxel-reference",
            Self::MeshReference => "mesh-reference",
            Self::VoxelPocked => "voxel-pocked",
        }
    }

    /// What a figure measured on this member is measuring.
    pub fn representation(self) -> Representation {
        match self {
            Self::Reference | Self::TenTimesLarger => Representation::Sdf,
            Self::VoxelReference | Self::VoxelPocked => Representation::Voxel,
            Self::MeshReference => Representation::Mesh,
        }
    }

    /// The member to measure a given representation on.
    pub fn for_representation(representation: Representation) -> Scene {
        match representation {
            Representation::Sdf => Self::Reference,
            Representation::Voxel => Self::VoxelReference,
            Representation::Mesh => Self::MeshReference,
        }
    }

    /// Which revision of it this is.
    ///
    /// Part of the conditions on purpose: comparing today's figures against a
    /// baseline taken on a different scene is worse than having no baseline,
    /// and this is what makes that visible instead of silent. Bump it whenever
    /// `build` changes shape.
    pub fn revision(self) -> &'static str {
        match self {
            Self::Reference => "r1",
            Self::TenTimesLarger => "r1",
            Self::VoxelReference => "r1",
            Self::MeshReference => "r1",
            Self::VoxelPocked => "r1",
        }
    }

    /// The starting form's radius.
    ///
    /// Surface area goes with the square of the radius, so ten times the area
    /// is the square root of ten times the radius.
    fn radius(self) -> f32 {
        match self {
            Self::Reference | Self::VoxelReference | Self::MeshReference | Self::VoxelPocked => 1.0,
            Self::TenTimesLarger => 10.0f32.sqrt(),
        }
    }

    /// How many strokes are laid onto it.
    ///
    /// The same count for both: the point of the larger scene is more surface
    /// at the same edit density, not more editing.
    const STROKES: usize = 8;
    const SAMPLES_PER_STROKE: usize = 12;
    /// How many dabs the grid is packed with.
    const VOXEL_STROKES: usize = 17;
    /// The cell the damaged grid is rasterized at.
    const POCKED_CELL: f32 = 0.05;

    /// The cell a voxel reference is built at.
    ///
    /// The default a first crossing lands at, so a figure taken here describes
    /// the resolution a sculptor actually meets.
    pub const VOXEL_CELL: f32 = 0.02;

    /// Builds the document. Deterministic: no clock, no randomness, no file.
    pub fn build(self, policy: BackendPolicy) -> Result<ClayDocument, ModelError> {
        match self {
            Self::Reference | Self::TenTimesLarger => self.build_sdf(policy),
            Self::VoxelReference => Self::build_voxel(policy),
            Self::MeshReference => Self::build_mesh(policy),
            Self::VoxelPocked => Self::build_pocked(policy),
        }
    }

    /// A field, worked.
    fn build_sdf(self, policy: BackendPolicy) -> Result<ClayDocument, ModelError> {
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

    /// A grid, worked.
    ///
    /// A slab across x rather than a ball: a voxel verb wants material with a
    /// wobble in it — a curvature-seeking brush has nothing to bite on a
    /// primitive — and a slab gives every verb the same amount of it wherever
    /// along the stroke it lands.
    fn build_voxel(policy: BackendPolicy) -> Result<ClayDocument, ModelError> {
        let mut document = ClayDocument::new(policy)?;
        document.add_voxel_layer("Voxels", Self::VOXEL_CELL)?;
        let brush = BrushSettings {
            size: 0.25,
            intensity: 1.0,
            ..BrushSettings::default()
        };
        for step in 0..Self::VOXEL_STROKES {
            let t = step as f32 / (Self::VOXEL_STROKES - 1) as f32;
            document.apply_stroke(
                ToolKind::Padrao,
                brush,
                &[GestureSample {
                    position: [(t - 0.5) * 1.6, (t * 9.0).sin() * 0.08, 0.0],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )?;
        }
        document.take_dirty_keys();
        Ok(document)
    }

    /// A grid with something wrong with it.
    ///
    /// The starting form rasterized solid, then two removals: a pocket at the
    /// centre, which nothing outside can reach and which is what a fill-voids
    /// has to find, and a channel bored in from the surface, which is what a
    /// close-holes has to seal. Rasterized at a coarser cell than the other
    /// grid on purpose — a repair walks the whole lattice, and the figure
    /// wanted here is of a repair rather than of a resolution.
    fn build_pocked(policy: BackendPolicy) -> Result<ClayDocument, ModelError> {
        let mut document = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)?;
        document.convert_layer(Direction::SdfToVoxel, Self::POCKED_CELL, 1)?;
        let erase = BrushSettings {
            size: 0.2,
            intensity: 1.0,
            ..BrushSettings::default()
        };
        // The pocket: wholly inside, so the outside cannot reach it.
        document.apply_stroke(
            ToolKind::Apagar,
            erase,
            &[GestureSample {
                position: [0.0, 0.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )?;
        // The channel: from clear of the shell to just under it.
        let bore: Vec<GestureSample> = (0..6)
            .map(|i| {
                let t = i as f32 / 5.0;
                GestureSample {
                    position: [0.0, 1.2 - t * 0.8, 0.0],
                    pressure: 1.0,
                    time: t,
                }
            })
            .collect();
        document.apply_stroke(
            ToolKind::Apagar,
            BrushSettings {
                size: 0.08,
                ..erase
            },
            &bore,
            [false; 3],
        )?;
        document.take_dirty_keys();
        Ok(document)
    }

    /// Triangles, at a count the mesh brushes are actually used at.
    ///
    /// Marched from the reference field rather than imported from a file: the
    /// suite has to build the same shape on every machine, and a fixture on
    /// disk is a thing that drifts. The field it comes from is the reference
    /// scene, so the two are the same subject in two representations.
    fn build_mesh(policy: BackendPolicy) -> Result<ClayDocument, ModelError> {
        let mut document = Self::Reference.build_sdf(policy)?;
        let key = document.convert_layer(Direction::SdfToMesh, Self::VOXEL_CELL, 1)?;
        document.set_active_layer(key)?;
        document.take_dirty_keys();
        Ok(document)
    }

    /// The brush the dab measurements use, scaled to the scene.
    ///
    /// Proportional to the form, because a brush that is a tenth of a small
    /// model and a hundredth of a large one is not the same tool.
    pub fn brush(self) -> BrushSettings {
        match self {
            // The grid is a slab a quarter-unit thick; a brush much smaller
            // than that scrapes its surface rather than reshaping it.
            Self::VoxelReference => BrushSettings {
                size: 0.25,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            _ => BrushSettings {
                size: 0.18 * self.radius(),
                ..BrushSettings::default()
            },
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

    /// Where to put a probe dab so that it lands on the surface.
    ///
    /// The centre of the surface brick most aligned with `toward`, asked of the
    /// cache rather than derived from the scene's own coordinates. Those place
    /// the build strokes at `radius * 1.01` and displace outward by a brush
    /// proportional to the radius, so on the larger scene that coordinate is
    /// *under* the surface by the time the scene is built. A dab there dirties
    /// bricks and not one of them holds a lattice, which is a miss reported as
    /// an edit: `locality.key_ratio` read 0.00 against a budget of 2 for as
    /// long as the probe was placed that way, so the figure could not fail.
    ///
    /// A pick is no better. On the larger scene it answers a radius the cache
    /// keeps no surface brick at — the bricks along that ray go `Outside`
    /// straight to `Inside` — which is its own question and not this one.
    ///
    /// Deterministic: `surface_bricks` is documented as returning the cache's
    /// own order and not a stable one, so the choice is made by direction.
    pub fn probe_point(document: &ClayDocument, toward: [f32; 3]) -> Option<[f32; 3]> {
        let config = document.cache().config();
        let extent = config.voxel_size * config.dim as f32;
        let length = (0..3).map(|i| toward[i] * toward[i]).sum::<f32>().sqrt();
        if length <= f32::EPSILON {
            return None;
        }
        let direction: [f32; 3] = std::array::from_fn(|i| toward[i] / length);

        document
            .cache()
            .surface_bricks()
            .ok()?
            .iter()
            .map(|key| {
                let at: [f32; 3] = std::array::from_fn(|i| (key[i] as f32 + 0.5) * extent);
                let len = (0..3).map(|i| at[i] * at[i]).sum::<f32>().sqrt().max(1e-6);
                let alignment: f32 = (0..3).map(|i| at[i] / len * direction[i]).sum();
                (at, alignment)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(at, _)| at)
    }

    /// A stroke across the front of the form, as a drag delivers it.
    pub fn stroke(self, samples: usize) -> Vec<GestureSample> {
        let radius = self.radius();
        (0..samples)
            .map(|i| {
                let t = i as f32 / (samples.max(2) - 1) as f32;
                GestureSample {
                    position: self.along(t, radius),
                    pressure: 1.0,
                    time: t,
                }
            })
            .collect()
    }

    /// Where the stroke is at `t`, on the subject this member builds.
    ///
    /// Around the form for the two field scenes and the mesh marched from one;
    /// along the slab for the grid, which has no far side to travel round.
    fn along(self, t: f32, radius: f32) -> [f32; 3] {
        if self == Self::VoxelReference {
            return [(t - 0.5) * 1.2, 0.08, 0.0];
        }
        let angle = (t - 0.5) * 1.2;
        let (s, c) = angle.sin_cos();
        [s * radius * 1.01, 0.1 * radius, c * radius * 1.01]
    }

    /// Where to land a probe edit on this member.
    ///
    /// The field scenes ask the cache, because their own coordinates are not
    /// where the surface ended up — see [`Scene::probe_point`]. A grid and a
    /// mesh are built here from a path this module wrote, so the path is the
    /// answer and there is nothing to ask.
    pub fn probe(self, document: &ClayDocument) -> Option<[f32; 3]> {
        let midpoint = self.along(0.5, self.radius());
        match self.representation() {
            Representation::Sdf => Self::probe_point(document, midpoint),
            _ => Some(midpoint),
        }
    }

    /// How big the subject this member built came out.
    ///
    /// Measured in the unit each representation is actually made of, so that a
    /// member which silently stopped building the same thing is caught rather
    /// than measured. `None` where the document cannot say.
    pub fn size(self, document: &mut ClayDocument) -> Option<usize> {
        match self.representation() {
            Representation::Sdf => Some(document.surface_brick_count()),
            Representation::Voxel => document.occupied_cells(),
            Representation::Mesh => Some(document.visible_mesh_geometry().3.len() / 3),
        }
    }

    /// What [`Scene::size`] should come out at, and what that is counted in.
    ///
    /// Recorded rather than derived: the point is to notice the shape
    /// changing, which a formula that changes with it cannot do. Bump these
    /// with the member's revision.
    pub fn expected_size(self) -> (usize, &'static str) {
        match self {
            Self::Reference => (1049, "surface bricks"),
            Self::TenTimesLarger => (9466, "surface bricks"),
            Self::VoxelReference => (3070, "occupied cells"),
            Self::MeshReference => (296_216, "triangles"),
            Self::VoxelPocked => (33_543, "occupied cells"),
        }
    }
}

/// The conditions of the machine this is running on.
pub fn conditions(policy: &BackendPolicy, viewport: (u32, u32)) -> Conditions {
    Conditions {
        scenes: Scene::ALL
            .into_iter()
            .map(|scene| (scene.member(), scene.revision()))
            .collect(),
        platform: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        backend: policy.active().to_string(),
        engine: clayspace_engine::claycore::version().to_string(),
        viewport,
    }
}
