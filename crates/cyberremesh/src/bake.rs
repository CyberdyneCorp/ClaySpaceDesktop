//! Maps baked from a field rather than from a high-poly mesh.
//!
//! **This is the half of the pipeline only this application can supply.** The
//! retopology engine's `CyberFieldEvaluator` is three C callbacks and a
//! `void*`, and its field-sampled maps have always been empty for want of a
//! volumetric engine to plug in. ClayCore answers all three. With one
//! attached, the cage ray is sphere-traced through the actual surface instead
//! of a tessellation of it, and normals come from exact gradients rather than
//! interpolated vertex normals — no high-poly mesh needed at all.
//!
//! ## The two conversions, which are the whole point of this module
//!
//! Both are documented on both sides and both produce plausible-looking wrong
//! output if skipped:
//!
//! **Occlusion is inverted.** Their `occlusion` callback returns **openness**,
//! where 1 is fully open. ClayCore's `CLAY_MEASURE_OCCLUSION` is occlusion,
//! where 1 is fully enclosed. Passing ours through unconverted bakes an
//! inverted ambient occlusion map — dark where it should be light, everywhere.
//!
//! **Curvature is a different quantity, not a different scale.** Theirs is
//! signed mean curvature in `1/length`; ClayCore's `CLAY_MEASURE_CURVATURE` is
//! a saturated `[0,1]` masking value. They are not convertible into one
//! another, so this does not offer a curvature callback at all and leaves the
//! engine to derive it from the gradient, which is what its authors instruct.

use cyberremesh_sys as sys;

use crate::error::{check, Error, Result};
use crate::mesh::Mesh;

/// Which map to bake.
///
/// Only the four a field can answer. Every other map the engine offers still
/// needs a target mesh and takes the raycast path, whose output the engine
/// documents as bit-identical — so they are not absent here, they are a
/// different call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldMap {
    Normal,
    AmbientOcclusion,
    Curvature,
    Cavity,
}

impl FieldMap {
    pub const ALL: [FieldMap; 4] = [
        Self::Normal,
        Self::AmbientOcclusion,
        Self::Curvature,
        Self::Cavity,
    ];

    fn raw(self) -> sys::CyberBakeMap::Type {
        match self {
            Self::Normal => sys::CyberBakeMap::CYBER_BAKE_NORMAL,
            Self::AmbientOcclusion => sys::CyberBakeMap::CYBER_BAKE_AO,
            Self::Curvature => sys::CyberBakeMap::CYBER_BAKE_CURVATURE,
            Self::Cavity => sys::CyberBakeMap::CYBER_BAKE_CAVITY,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normais",
            Self::AmbientOcclusion => "Oclusão ambiente",
            Self::Curvature => "Curvatura",
            Self::Cavity => "Cavidade",
        }
    }
}

/// A field the baker can sample.
///
/// Implemented over ClayCore in `clayspace-engine`; the trait is here so this
/// crate carries no dependency on a volumetric engine, which is the rule both
/// engines state about themselves.
///
/// Every method is called from the thread that called [`bake_field`], for the
/// duration of that call and no longer.
///
/// **No `Send` bound, deliberately.** `cyber_bake_field` is synchronous: it
/// invokes these callbacks and returns, so the field never crosses a thread
/// boundary and requiring it to be sendable would force an implementor holding
/// a non-`Sync` handle — a field document, for instance — to write `unsafe` to
/// promise something the call does not need. The engine's own note that the
/// callbacks run on "the baking thread" is about reentrancy within the call,
/// which a shared reference already satisfies.
pub trait Field {
    /// Signed distance at a point. Negative inside.
    fn distance(&self, at: [f32; 3]) -> f32;

    /// The unit gradient at a point.
    fn gradient(&self, at: [f32; 3]) -> [f32; 3];

    /// **Openness** in `[0, 1]`, where 1 is fully open.
    ///
    /// Named for what the engine wants rather than for what ClayCore answers,
    /// so that the inversion is performed by whoever implements this and is
    /// visible in the implementation rather than buried in this crate. An
    /// implementation that returns ClayCore's occlusion unchanged bakes an
    /// inverted map that looks plausible.
    fn openness(&self, at: [f32; 3], normal: [f32; 3], radius: f32) -> f32;
}

/// What a bake is asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BakeParams {
    pub width: u32,
    pub height: u32,
    /// How far off the surface the cage sits. Sphere-traced through the field
    /// rather than intersected against triangles, which is what removes the
    /// cage-ray misses a tessellated high-poly produces at a seam.
    pub cage_distance: f32,
    /// Hemisphere samples for ambient occlusion.
    pub ao_samples: u32,
    /// How far an occlusion ray looks.
    pub ao_radius: f32,
    /// The range curvature is normalised over.
    pub curvature_range: f32,
}

impl Default for BakeParams {
    fn default() -> Self {
        let mut raw = sys::CyberBakeParams::default();
        // SAFETY: a valid out-pointer; the call only writes.
        unsafe { sys::cyber_default_bake_params(&mut raw) };
        Self {
            width: raw.width.max(0) as u32,
            height: raw.height.max(0) as u32,
            cage_distance: raw.cageDistance,
            ao_samples: raw.aoSamples.max(0) as u32,
            ao_radius: raw.aoRadius,
            curvature_range: raw.curvatureRange,
        }
    }
}

impl BakeParams {
    fn to_raw(self) -> sys::CyberBakeParams {
        let mut raw = sys::CyberBakeParams::default();
        // SAFETY: as `Default`.
        unsafe { sys::cyber_default_bake_params(&mut raw) };
        raw.width = self.width as _;
        raw.height = self.height as _;
        raw.cageDistance = self.cage_distance;
        raw.aoSamples = self.ao_samples as _;
        raw.aoRadius = self.ao_radius;
        raw.curvatureRange = self.curvature_range;
        raw
    }
}

/// A baked map.
pub struct Image {
    raw: std::ptr::NonNull<sys::CyberImage>,
}

// SAFETY: the engine reaches this allocation only through this handle.
unsafe impl Send for Image {}

impl Image {
    pub fn width(&self) -> u32 {
        // SAFETY: a valid handle; the call only reads.
        unsafe { sys::cyber_image_width(self.raw.as_ptr()) as u32 }
    }

    pub fn height(&self) -> u32 {
        // SAFETY: a valid handle; the call only reads.
        unsafe { sys::cyber_image_height(self.raw.as_ptr()) as u32 }
    }

    pub fn channels(&self) -> u32 {
        // SAFETY: a valid handle; the call only reads.
        unsafe { sys::cyber_image_channels(self.raw.as_ptr()) as u32 }
    }

    /// Writes the map as a PNG.
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let c_path = std::ffi::CString::new(path.as_ref().to_string_lossy().as_ref())
            .map_err(|_| Error::misuse("cyber_image_save_png", "the path contains a NUL"))?;
        check(
            // SAFETY: a valid handle and a NUL-terminated path outliving the call.
            unsafe { sys::cyber_image_save_png(self.raw.as_ptr(), c_path.as_ptr()) },
            "cyber_image_save_png",
        )
    }

    /// The pixels, copied out.
    ///
    /// **Floats, one per channel, not bytes.** A baked map is float data —
    /// which matters for a normal map, where eight bits a channel is visible
    /// banding, and for curvature, which is signed. Assuming bytes here would
    /// have read a quarter of the buffer and called it an image; the C
    /// signature is what said otherwise.
    pub fn pixels(&self) -> Vec<f32> {
        let count = (self.width() * self.height() * self.channels()) as usize;
        let mut pixels = vec![0.0f32; count];
        // SAFETY: the destination holds exactly the float count passed, and
        // the engine writes no more than it is told it may.
        let written =
            unsafe { sys::cyber_image_copy_pixels(self.raw.as_ptr(), pixels.as_mut_ptr(), count) };
        pixels.truncate(written);
        pixels
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        // SAFETY: an owned handle, released exactly once; every reader above
        // copies out rather than holding a pointer in.
        unsafe { sys::cyber_image_free(self.raw.as_ptr()) };
    }
}

/// Bakes a map by sampling a field, with no high-poly mesh.
pub fn bake_field<F: Field>(
    low: &Mesh,
    map: FieldMap,
    params: BakeParams,
    field: &F,
) -> Result<Image> {
    let raw_params = params.to_raw();

    // The trait object the three C callbacks reach the field through, valid
    // for exactly the duration of the call below.
    let carrier: &dyn Field = field;
    let mut carrier = carrier;
    let user = &mut carrier as *mut &dyn Field as *mut std::ffi::c_void;

    // SAFETY for all three: `user` is the address of `carrier`, which outlives
    // the call; the engine documents them as invoked only during it and
    // requires them reentrant, which a shared reference satisfies.
    unsafe extern "C" fn distance(user: *mut std::ffi::c_void, p: *const f32) -> f32 {
        let Some(field) = (user as *mut &dyn Field).as_ref() else {
            return 0.0;
        };
        field.distance(read3(p))
    }

    unsafe extern "C" fn gradient(user: *mut std::ffi::c_void, p: *const f32, out: *mut f32) {
        let Some(field) = (user as *mut &dyn Field).as_ref() else {
            return;
        };
        let g = field.gradient(read3(p));
        if !out.is_null() {
            std::ptr::copy_nonoverlapping(g.as_ptr(), out, 3);
        }
    }

    unsafe extern "C" fn occlusion(
        user: *mut std::ffi::c_void,
        p: *const f32,
        n: *const f32,
        radius: f32,
    ) -> f32 {
        let Some(field) = (user as *mut &dyn Field).as_ref() else {
            return 1.0;
        };
        // The trait's method is named `openness` for the engine's own
        // convention, so no inversion happens here: it is the implementor's,
        // and visible in their code rather than hidden in this shim.
        field.openness(read3(p), read3(n), radius)
    }

    let evaluator = sys::CyberFieldEvaluator {
        distance: Some(distance),
        gradient: Some(gradient),
        occlusion: Some(occlusion),
        user,
    };

    let mut out = std::ptr::null_mut();
    check(
        // SAFETY: a valid low-poly mesh; a NULL high-poly, which the engine
        // documents as permitted for exactly these four maps; a filled descriptor;
        // an evaluator whose `user` outlives the call; an out-parameter written
        // only on success.
        unsafe {
            sys::cyber_bake_field(
                low.as_ptr(),
                std::ptr::null(),
                map.raw(),
                &raw_params,
                &evaluator,
                &mut out,
            )
        },
        "cyber_bake_field",
    )?;
    std::ptr::NonNull::new(out)
        .map(|raw| Image { raw })
        .ok_or_else(|| Error::misuse("cyber_bake_field", "the engine returned no image"))
}

/// SAFETY: `p` is either null or three readable floats, which is what every
/// callback above is handed by the engine.
unsafe fn read3(p: *const f32) -> [f32; 3] {
    if p.is_null() {
        return [0.0; 3];
    }
    [*p, *p.add(1), *p.add(2)]
}
