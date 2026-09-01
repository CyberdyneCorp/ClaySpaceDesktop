//! Rebuilding a mesh through a voxel field — what sculpting applications call
//! DynaMesh.
//!
//! Every *piece* of this was already in the engine: sample a mesh into a
//! field, sign it, march an isosurface, validate, transfer attributes. What
//! ABI 0.63.0 added is the *operation*, and with it the decisions the pieces
//! do not make — what a resolution means, what an open surface becomes, what
//! it costs before it is asked for. 0.64.0 put the same operation inside the
//! document, where it lands on a layer, replaces what was there and is one
//! step on the undo menu.
//!
//! It is destructive by construction and the engine says so rather than
//! implying it: overlapping shells fuse, self-intersections resolve, stretched
//! triangles disappear, density comes out uniform — and vertex and polygon
//! identity are gone, UVs are dropped rather than reprojected, and detail
//! finer than the voxel size may go with them. Everything in that list is
//! reported by [`RemeshReport`] rather than left for the caller to infer.

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::error::{check, Result};
use crate::{Document, LayerId};

/// How the sampling resolution is stated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resolution {
    /// World units per cell, the canonical form.
    VoxelSize(f32),
    /// The source's longest bounding extent divided by this, resolved before
    /// any sampling padding is applied. The form a slider wants: it means the
    /// same thing on a thumbnail and on a bust.
    LongestAxis(u32),
}

impl Default for Resolution {
    fn default() -> Self {
        Self::LongestAxis(128)
    }
}

/// Which isosurface the field is turned back into.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Surface {
    /// Marching tetrahedra: watertight and 2-manifold by construction.
    #[default]
    Smooth,
    /// Dual contouring, which holds a chamfer far closer at a third of the
    /// triangles — and is **experimental**: the engine does not claim the
    /// watertight guarantee for it, and measured it is not manifold at
    /// longest-axis 96.
    Sharp,
}

/// What becomes of a source that is not closed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OpenSurface {
    /// A typed failure and no mesh. The report still carries the source's
    /// boundary-edge count, which is the number to put in front of a user.
    #[default]
    Reject,
    /// Close it, then validate.
    Close,
    /// Proceed and report what came out.
    BestEffort,
}

/// What becomes of the specks a rebuild leaves behind.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum SmallComponents {
    #[default]
    Keep,
    /// Drop every component under this many cubic world units.
    RemoveBelowVolume(f32),
}

/// How a mesh is rebuilt.
///
/// [`Default`] is the engine's own, read out of
/// `clay_mesh_voxel_remesh_defaults` rather than transcribed — the header asks
/// callers not to transcribe them, and a default that drifted from the
/// engine's would be a silent behaviour change on an upgrade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemeshParams {
    pub resolution: Resolution,
    pub surface: Surface,
    pub open_surface: OpenSurface,
    pub small_components: SmallComponents,
    /// A clamped correction, skipped where the comparison stops meaning
    /// anything.
    pub preserve_volume: bool,
    /// Pull the rebuilt vertices back towards the source. A lerp, never a
    /// snap.
    pub projection: Option<Projection>,
    /// A source carrying none produces none.
    pub preserve_colors: bool,
    /// Refuse before allocating the field, the tree and the result. Zero is no
    /// caller budget, which still leaves the library's own ceiling.
    pub memory_budget_bytes: u64,
}

/// Pulling rebuilt vertices back onto the source surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    /// 0..1, a lerp towards the source.
    pub strength: f32,
    /// How far a vertex may be pulled, in voxels.
    pub max_distance_voxels: f32,
}

impl Default for RemeshParams {
    fn default() -> Self {
        let raw = Self::engine_defaults();
        Self::from_raw(&raw)
    }
}

impl RemeshParams {
    /// Rebuild at a given number of cells across the longest extent.
    pub fn at_longest_axis(resolution: u32) -> Self {
        Self {
            resolution: Resolution::LongestAxis(resolution),
            ..Self::default()
        }
    }

    fn engine_defaults() -> sys::clay_voxel_remesh_params {
        let mut raw = sys::clay_voxel_remesh_params::sized();
        // SAFETY: a sized descriptor the engine only writes into. The call
        // cannot fail for a correctly sized struct; a failure would leave the
        // zeroed descriptor, which `from_raw` reads as a coherent — if
        // unhelpful — request rather than as undefined state.
        let _ = unsafe { sys::clay_mesh_voxel_remesh_defaults(&mut raw) };
        raw
    }

    fn from_raw(raw: &sys::clay_voxel_remesh_params) -> Self {
        Self {
            resolution: if raw.resolution_mode
                == sys::clay_voxel_remesh_resolution::CLAY_VOXEL_REMESH_LONGEST_AXIS as i32
            {
                Resolution::LongestAxis(raw.longest_axis_resolution)
            } else {
                Resolution::VoxelSize(raw.voxel_size)
            },
            surface: if raw.surface_mode
                == sys::clay_voxel_remesh_surface::CLAY_VOXEL_REMESH_SHARP as i32
            {
                Surface::Sharp
            } else {
                Surface::Smooth
            },
            open_surface: match raw.open_surface_policy {
                x if x
                    == sys::clay_voxel_remesh_open_policy::CLAY_VOXEL_REMESH_OPEN_CLOSE as i32 =>
                {
                    OpenSurface::Close
                }
                x if x
                    == sys::clay_voxel_remesh_open_policy::CLAY_VOXEL_REMESH_OPEN_BEST_EFFORT
                        as i32 =>
                {
                    OpenSurface::BestEffort
                }
                _ => OpenSurface::Reject,
            },
            small_components: if raw.small_component_policy
                == sys::clay_voxel_remesh_component_policy::CLAY_VOXEL_REMESH_REMOVE_BELOW_VOLUME
                    as i32
            {
                SmallComponents::RemoveBelowVolume(raw.minimum_component_volume)
            } else {
                SmallComponents::Keep
            },
            preserve_volume: raw.preserve_volume != 0,
            projection: (raw.project_to_source != 0).then_some(Projection {
                strength: raw.projection_strength,
                max_distance_voxels: raw.max_projection_distance_voxels,
            }),
            preserve_colors: raw.preserve_colors != 0,
            memory_budget_bytes: raw.memory_budget_bytes,
        }
    }

    fn to_raw(self) -> sys::clay_voxel_remesh_params {
        // Started from the engine's defaults rather than from zero, so a field
        // this wrapper does not model keeps whatever the engine chose for it.
        // `build_multires_levels` is the one that matters today: it is
        // reserved, and non-zero is refused.
        let mut raw = Self::engine_defaults();
        match self.resolution {
            Resolution::VoxelSize(size) => {
                raw.resolution_mode =
                    sys::clay_voxel_remesh_resolution::CLAY_VOXEL_REMESH_VOXEL_SIZE as i32;
                raw.voxel_size = size;
            }
            Resolution::LongestAxis(count) => {
                raw.resolution_mode =
                    sys::clay_voxel_remesh_resolution::CLAY_VOXEL_REMESH_LONGEST_AXIS as i32;
                raw.longest_axis_resolution = count;
            }
        }
        raw.surface_mode = match self.surface {
            Surface::Smooth => sys::clay_voxel_remesh_surface::CLAY_VOXEL_REMESH_SMOOTH,
            Surface::Sharp => sys::clay_voxel_remesh_surface::CLAY_VOXEL_REMESH_SHARP,
        } as i32;
        raw.open_surface_policy = match self.open_surface {
            OpenSurface::Reject => {
                sys::clay_voxel_remesh_open_policy::CLAY_VOXEL_REMESH_OPEN_REJECT
            }
            OpenSurface::Close => sys::clay_voxel_remesh_open_policy::CLAY_VOXEL_REMESH_OPEN_CLOSE,
            OpenSurface::BestEffort => {
                sys::clay_voxel_remesh_open_policy::CLAY_VOXEL_REMESH_OPEN_BEST_EFFORT
            }
        } as i32;
        let (policy, volume) = match self.small_components {
            SmallComponents::Keep => (
                sys::clay_voxel_remesh_component_policy::CLAY_VOXEL_REMESH_KEEP_COMPONENTS,
                0.0,
            ),
            SmallComponents::RemoveBelowVolume(v) => (
                sys::clay_voxel_remesh_component_policy::CLAY_VOXEL_REMESH_REMOVE_BELOW_VOLUME,
                v,
            ),
        };
        raw.small_component_policy = policy as i32;
        raw.minimum_component_volume = volume;
        raw.preserve_volume = i32::from(self.preserve_volume);
        raw.project_to_source = i32::from(self.projection.is_some());
        if let Some(projection) = self.projection {
            raw.projection_strength = projection.strength;
            raw.max_projection_distance_voxels = projection.max_distance_voxels;
        }
        raw.preserve_colors = i32::from(self.preserve_colors);
        raw.memory_budget_bytes = self.memory_budget_bytes;
        raw
    }
}

/// What a rebuild would cost, before it is asked for.
///
/// Cheap enough for a resolution slider: the engine walks the source's
/// triangles and marks a brick lattice, allocating nothing proportional to the
/// sample count it predicts.
#[derive(Debug, Clone, PartialEq)]
pub struct RemeshEstimate {
    pub voxel_size: f32,
    pub grid_dimensions: [u32; 3],
    /// An **upper bound** on the narrow band, not a prediction: bricks are
    /// kept whose box comes within the band of a triangle's bounds, and some
    /// hold nothing near enough to store. [`RemeshReport::active_samples`] is
    /// what a run actually held and is never larger.
    pub active_samples: u64,
    pub memory_bytes: u64,
    pub triangles: std::ops::RangeInclusive<u64>,
    pub boundary_edges: u32,
    pub components: u32,
    pub open: bool,
    /// Sampled evidence that the source carries material thinner than a couple
    /// of voxels — material this resolution may delete.
    pub thin_features: bool,
    pub over_budget: bool,
}

/// What a rebuild did, including everything it destroyed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemeshReport {
    pub voxel_size: f32,
    pub source_vertices: u64,
    pub source_triangles: u64,
    pub result_vertices: u64,
    pub result_triangles: u64,
    pub source_volume: f64,
    pub result_volume: f64,
    pub relative_volume_error: f64,
    pub source_boundary_edges: u32,
    pub result_boundary_edges: u32,
    pub source_components: u32,
    pub result_components: u32,
    pub removed_components: u32,
    pub active_samples: u64,
    pub source_was_open: bool,
    pub result_watertight: bool,
    pub result_manifold: bool,
    pub result_oriented: bool,
    pub projected_to_source: bool,
    pub projected_vertices: u64,
    pub volume_corrected: bool,
    pub colors_transferred: bool,
    /// Always set where the source carried UVs, and not a failure: a spatially
    /// reprojected UV across a seam is a stretched layout that looks like a
    /// preserved one, so the operation does not pretend to keep them.
    pub uvs_dropped: bool,
    pub cancelled: bool,
}

/// A refused rebuild, and the numbers that explain it.
///
/// Boxed at every call site, which is not decoration: a report is a hundred
/// and sixty bytes of counts and every caller pays for it on the success path
/// too if it travels in the `Err` variant unboxed. The failure is the rare
/// branch and is the one that should carry the cost.
#[derive(Debug, Clone, PartialEq)]
pub struct RemeshRefusal {
    pub error: crate::ClayError,
    /// The engine fills this for a refusal wherever the numbers exist — an
    /// open-surface refusal carries the source's boundary-edge count, which is
    /// what a host puts in front of a user rather than the result code.
    pub report: RemeshReport,
}

impl std::fmt::Display for RemeshRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for RemeshRefusal {}

impl RemeshReport {
    fn from_raw(raw: sys::clay_voxel_remesh_report) -> Self {
        Self {
            voxel_size: raw.voxel_size,
            source_vertices: raw.source_vertices,
            source_triangles: raw.source_triangles,
            result_vertices: raw.result_vertices,
            result_triangles: raw.result_triangles,
            source_volume: raw.source_volume,
            result_volume: raw.result_volume,
            relative_volume_error: raw.relative_volume_error,
            source_boundary_edges: raw.source_boundary_edges,
            result_boundary_edges: raw.result_boundary_edges,
            source_components: raw.source_components,
            result_components: raw.result_components,
            removed_components: raw.removed_components,
            active_samples: raw.active_samples,
            source_was_open: raw.source_was_open != 0,
            result_watertight: raw.result_watertight != 0,
            result_manifold: raw.result_manifold != 0,
            result_oriented: raw.result_oriented != 0,
            projected_to_source: raw.projected_to_source != 0,
            projected_vertices: raw.projected_vertices,
            volume_corrected: raw.volume_corrected != 0,
            colors_transferred: raw.colors_transferred != 0,
            uvs_dropped: raw.uvs_dropped != 0,
            cancelled: raw.cancelled != 0,
        }
    }
}

impl crate::Mesh {
    /// What rebuilding this mesh would cost, without rebuilding it.
    ///
    /// Returns the same refusals the rebuild itself would for an unusable
    /// resolution or a request over budget.
    pub fn remesh_estimate(&self, params: RemeshParams) -> Result<RemeshEstimate> {
        let raw_params = params.to_raw();
        let mut raw = sys::clay_voxel_remesh_estimate::sized();
        // SAFETY: a borrowed mesh handle this call only reads, and two
        // descriptors carrying their own sizes.
        check(
            unsafe { sys::clay_mesh_voxel_remesh_estimate(self.as_ptr(), &raw_params, &mut raw) },
            "clay_mesh_voxel_remesh_estimate",
        )?;
        Ok(RemeshEstimate {
            voxel_size: raw.resolved_voxel_size,
            grid_dimensions: raw.grid_dimensions,
            active_samples: raw.estimated_active_samples,
            memory_bytes: raw.estimated_memory_bytes,
            triangles: raw.estimated_triangle_min..=raw.estimated_triangle_max,
            boundary_edges: raw.boundary_edge_count,
            components: raw.component_count,
            open: raw.has_open_boundaries != 0,
            thin_features: raw.thin_feature_warning != 0,
            over_budget: raw.exceeds_memory_budget != 0,
        })
    }

    /// Rebuilds this mesh through a voxel field, leaving it untouched.
    ///
    /// The pure form, with no document and no undo. [`Document::remesh_layer`]
    /// is what an interface calls.
    pub fn voxel_remesh(&self, params: RemeshParams) -> Result<(Self, RemeshReport)> {
        let raw_params = params.to_raw();
        let mut raw_report = sys::clay_voxel_remesh_report::sized();
        let mut out = std::ptr::null_mut();
        // SAFETY: a borrowed source the call never modifies, a sized
        // descriptor, no cancel token, and two out-parameters. `out` receives
        // a mesh this crate then owns and destroys on drop.
        check(
            unsafe {
                sys::clay_mesh_voxel_remesh(
                    self.as_ptr(),
                    &raw_params,
                    std::ptr::null_mut(),
                    &mut out,
                    &mut raw_report,
                )
            },
            "clay_mesh_voxel_remesh",
        )?;
        let mesh = Self::from_raw(out, "clay_mesh_voxel_remesh")?;
        Ok((mesh, RemeshReport::from_raw(raw_report)))
    }
}

impl Document {
    /// A mesh layer's geometry revision.
    ///
    /// Bumped every time a layer's triangles are replaced wholesale, and
    /// **not** by a sculpt: a brush moves vertices and leaves the topology
    /// alone, which is the fixed-topology contract and exactly the change a
    /// cache over that mesh survives. This exists for the change a cache does
    /// not survive — a rebuild swaps every vertex and every index, and an
    /// adjacency, a BVH or a live sculptor built over the old ones is wrong in
    /// a way nothing else detects. Through ABI 0.63.0 a rebuild landing on the
    /// same vertex and index counts passed every check there was.
    ///
    /// Zero for a layer that is not a mesh layer, or does not exist.
    pub fn mesh_layer_revision(&self, layer: LayerId) -> Result<u64> {
        let mut revision = 0;
        // SAFETY: valid handle and one out-parameter.
        check(
            unsafe {
                sys::clay_document_mesh_layer_revision(self.as_ptr(), layer.0, &mut revision)
            },
            "clay_document_mesh_layer_revision",
        )?;
        Ok(revision)
    }

    /// Rebuilds a mesh layer in place, as one undo step.
    ///
    /// Capture, rebuild, validate, replace, record. Nothing is written until
    /// the rebuild has succeeded *and* validated, so a refusal leaves the
    /// layer byte-identical — which is what lets an interface offer this
    /// against a resolution the source may turn out not to survive.
    ///
    /// The report is filled for a refusal too, wherever the numbers exist: an
    /// open-surface refusal carries the source's boundary-edge count, which is
    /// the number to show rather than the error code.
    pub fn remesh_layer(
        &mut self,
        layer: LayerId,
        params: RemeshParams,
    ) -> std::result::Result<RemeshReport, Box<RemeshRefusal>> {
        let raw_params = params.to_raw();
        let mut raw_report = sys::clay_voxel_remesh_report::sized();
        // SAFETY: valid handle, a sized descriptor, no cancel token, and one
        // out-parameter the engine fills on success and on the refusals whose
        // numbers exist.
        let result = unsafe {
            sys::clay_document_voxel_remesh_layer(
                self.as_ptr(),
                layer.0,
                &raw_params,
                std::ptr::null_mut(),
                &mut raw_report,
            )
        };
        let report = RemeshReport::from_raw(raw_report);
        match check(result, "clay_document_voxel_remesh_layer") {
            Ok(()) => Ok(report),
            Err(error) => Err(Box::new(RemeshRefusal { error, report })),
        }
    }
}
