//! A UV layout, decided by the engine or cut along seams a sculptor chose.

use cyberremesh_sys as sys;

use crate::error::{check, Result};
use crate::mesh::Mesh;
use crate::remesh::Watcher;

/// What an atlas run is asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasParams {
    /// How far a chart's normals may spread before it is cut.
    pub max_chart_angle_degrees: f32,
    /// Gutter between charts, in UV units.
    pub pack_margin: f32,
    /// The texture the density is stated against.
    pub texture_size: u32,
    /// Rotate each chart onto its minimum-area bounding box, which roughly
    /// doubles usable coverage on box-like meshes — a 45-degree diamond face
    /// becomes an axis-aligned square.
    pub reorient_charts: bool,
    /// Fold adjacent charts together where it costs no distortion, then again
    /// where it costs less than `max_chart_distortion`.
    pub merge_charts: bool,
    pub max_chart_distortion: f32,
}

impl Default for AtlasParams {
    fn default() -> Self {
        // The engine's own defaults, read from it rather than restated here.
        let mut raw = sys::CyberAtlasParams::default();
        // SAFETY: a valid out-pointer; the call only writes.
        unsafe { sys::cyber_default_atlas_params(&mut raw) };
        Self {
            max_chart_angle_degrees: raw.maxChartAngleDegrees,
            pack_margin: raw.packMargin,
            texture_size: raw.textureSize.max(0) as u32,
            reorient_charts: raw.reorientCharts != 0,
            merge_charts: raw.mergeCharts != 0,
            max_chart_distortion: raw.maxChartDistortion,
        }
    }
}

impl AtlasParams {
    fn to_raw(self) -> sys::CyberAtlasParams {
        let mut raw = sys::CyberAtlasParams::default();
        // SAFETY: as `Default`. Filled from the engine first so a field this
        // application has no opinion about carries the engine's value rather
        // than a zero nobody chose.
        unsafe { sys::cyber_default_atlas_params(&mut raw) };
        raw.maxChartAngleDegrees = self.max_chart_angle_degrees;
        raw.packMargin = self.pack_margin;
        raw.textureSize = self.texture_size as _;
        raw.reorientCharts = i32::from(self.reorient_charts);
        raw.mergeCharts = i32::from(self.merge_charts);
        raw.maxChartDistortion = self.max_chart_distortion;
        raw
    }
}

/// What a layout came to.
///
/// Every figure here is the engine's own. They are carried up rather than
/// reduced to a verdict because a UV layout is a trade a person judges: a
/// sculptor deciding whether to re-cut wants the distortion and the coverage,
/// not a pass or a fail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Atlas {
    pub charts: u32,
    pub seam_edges: usize,
    /// Conformal (angle) error, where 0 is angle-preserving.
    pub max_angle_distortion: f32,
    pub rms_angle_distortion: f32,
    /// Charts whose parameterisation turned inside out. Non-zero is a defect
    /// in the layout rather than a quality figure.
    pub flipped_charts: u32,
    pub fallback_charts: u32,
    pub dropped_charts: u32,
    /// The fraction of the UV square the chart *geometry* covers.
    pub packed_area: f32,
    /// The fraction its bounding boxes cover, which is the looser number and
    /// is reported separately because the two are easy to confuse.
    pub packed_box_area: f32,
    pub texel_density: f32,
}

impl Atlas {
    fn of(raw: sys::CyberAtlasResult) -> Self {
        Self {
            charts: raw.chartCount.max(0) as u32,
            seam_edges: raw.seamEdges,
            max_angle_distortion: raw.maxAngleDistortion,
            rms_angle_distortion: raw.rmsAngleDistortion,
            flipped_charts: raw.flippedCharts.max(0) as u32,
            fallback_charts: raw.fallbackCharts.max(0) as u32,
            dropped_charts: raw.droppedCharts.max(0) as u32,
            packed_area: raw.packedArea,
            packed_box_area: raw.packedBoxArea,
            texel_density: raw.texelDensity,
        }
    }
}

/// Seams the engine decides, unwrapped and packed. Mesh in, atlas out.
///
/// Uses the **cancellable** entry point unconditionally. A UV atlas on a dense
/// mesh outlasts a frame, and this application's rule is that such work runs
/// off the interface thread with a way to stop it — so there is no reason to
/// reach for the uninterruptible twin.
pub fn atlas<W: Watcher>(mesh: &mut Mesh, params: AtlasParams, watcher: &mut W) -> Result<Atlas> {
    let raw_params = params.to_raw();
    let mut result = sys::CyberAtlasResult::default();
    let mut carrier: &mut dyn Watcher = watcher;
    let user = &mut carrier as *mut &mut dyn Watcher as *mut std::ffi::c_void;

    check(
        // SAFETY: a valid mesh the engine writes UVs into, a filled descriptor,
        // the two callbacks whose `user` is the carrier above — valid for exactly
        // this call — and an out-parameter written only on success.
        unsafe {
            sys::cyber_uv_atlas_cancellable(
                mesh.as_ptr(),
                &raw_params,
                Some(crate::remesh::on_progress),
                Some(crate::remesh::on_cancel),
                user,
                &mut result,
            )
        },
        "cyber_uv_atlas_cancellable",
    )?;
    Ok(Atlas::of(result))
}
