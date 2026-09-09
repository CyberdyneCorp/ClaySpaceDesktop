//! Triangle in, quads out.

use cyberremesh_sys as sys;

use crate::error::{check, Error, Result};
use crate::mesh::Mesh;

/// Which quadrangulator runs.
///
/// The engine's own default is `QuadCover`, and this offers no routing rule of
/// its own: the engine's authors record that choosing per input by measuring
/// both solvers "is still open work", and a threshold invented here would be a
/// judgement we cannot defend with a measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuadMethod {
    /// QuadCover seamless-UV isoline extraction. The engine's shipping default.
    #[default]
    QuadCover,
    /// The ZRemesher-class track: the QuadCover path plus the explicit
    /// topology-layout stage, which makes edge-loop structure a first-class
    /// artifact rather than an emergent consequence of the field.
    ///
    /// Structurally the same field, solve and extraction — the difference is
    /// the layout stage and the tracing options *the layout* wants rather than
    /// the ones the shipped quantizer's guided rounding wants. That is why it
    /// is a separate method and not a flag.
    ZRemesher,
    /// Max-matching over a smoothed cross field. Strongest on box and CAD
    /// geometry, ~95%+ quad-dominant.
    FieldAligned,
    /// Instant-Meshes-style position-field extraction.
    InstantMeshes,
    /// Experimental integer parametrisation.
    Integer,
}

impl QuadMethod {
    /// Every method this build offers, in the order the interface shows them.
    pub const ALL: [QuadMethod; 5] = [
        Self::QuadCover,
        Self::ZRemesher,
        Self::FieldAligned,
        Self::InstantMeshes,
        Self::Integer,
    ];

    /// The engine's own enumerant.
    ///
    /// From the generated constants and not from the numbers written in the
    /// header's prose beside them: `quadMethod` is a plain `int` with its
    /// values documented in a comment, so there is no enum for a compiler to
    /// check and a literal here would be a fourth place the mapping lives.
    fn raw(self) -> i32 {
        match self {
            Self::QuadCover => sys::CYBER_QUAD_QUADCOVER as i32,
            Self::ZRemesher => sys::CYBER_QUAD_ZREMESHER as i32,
            Self::FieldAligned => sys::CYBER_QUAD_FIELD_ALIGNED as i32,
            Self::InstantMeshes => sys::CYBER_QUAD_INSTANT_MESHES as i32,
            Self::Integer => sys::CYBER_QUAD_INTEGER as i32,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::QuadCover => "QuadCover",
            Self::ZRemesher => "ZRemesher",
            Self::FieldAligned => "Alinhado ao campo",
            Self::InstantMeshes => "Instant Meshes",
            Self::Integer => "Inteiro",
        }
    }
}

/// What to ask the retopologiser for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemeshParams {
    /// Quads to aim at. Approached and never hit — the engine searches over
    /// edge length rather than solving for a count.
    pub target_quads: u32,
    pub method: QuadMethod,
    /// Hold hard edges below this dihedral angle as features.
    pub sharp_edge_degrees: f32,
    /// Push the result to 100% quads through subdivision and
    /// surface-projected relaxation, rather than leaving it quad-dominant.
    pub pure_quads: bool,
    /// Let density follow curvature.
    pub adaptivity: f32,
}

impl Default for RemeshParams {
    fn default() -> Self {
        // Read out of the engine rather than restated, so a default that moves
        // upstream moves here — except the fields this application has an
        // opinion about, which are named below it.
        let mut raw = sys::CyberRemeshParams::default();
        // SAFETY: a valid out-pointer; the call only writes.
        unsafe { sys::cyber_default_params(&mut raw) };
        Self {
            target_quads: raw.targetQuads.max(0) as u32,
            method: QuadMethod::default(),
            sharp_edge_degrees: raw.sharpEdgeDegrees,
            pure_quads: raw.pureQuads != 0,
            adaptivity: raw.adaptivity,
        }
    }
}

impl RemeshParams {
    fn to_raw(self) -> sys::CyberRemeshParams {
        let mut raw = sys::CyberRemeshParams::default();
        // SAFETY: as `Default`. Filled from the engine first so that any field
        // this application does not set carries the engine's value rather than
        // a zero — the struct is never extended in place, but a zero in a
        // field we did not think about is still a choice nobody made.
        unsafe { sys::cyber_default_params(&mut raw) };
        raw.targetQuads = self.target_quads as _;
        raw.quadMethod = self.method.raw();
        raw.sharpEdgeDegrees = self.sharp_edge_degrees;
        raw.pureQuads = i32::from(self.pure_quads);
        raw.adaptivity = self.adaptivity;
        raw
    }
}

/// How a long retopology reports and how it is stopped.
///
/// The engine calls both from its own worker thread, so anything here must be
/// reachable from one. `progress` is advisory; returning `true` from `cancel`
/// stops the run and the call returns the engine's cancelled status rather
/// than a mesh.
pub trait Watcher {
    fn progress(&mut self, _fraction: f32, _stage: &str) {}
    fn cancelled(&mut self) -> bool {
        false
    }
}

/// A watcher that neither reports nor cancels.
pub struct Unwatched;
impl Watcher for Unwatched {}

// SAFETY: `user` is the address of `carrier`, which outlives the call;
// the engine documents both callbacks as invoked only during it, and
// reentrantly, and this reconstructs the reference rather than aliasing a
// second mutable one.
pub(crate) unsafe extern "C" fn on_progress(
    fraction: f32,
    stage: *const std::os::raw::c_char,
    user: *mut std::ffi::c_void,
) {
    let Some(carrier) = (user as *mut &mut dyn Watcher).as_mut() else {
        return;
    };
    let stage = if stage.is_null() {
        String::new()
    } else {
        std::ffi::CStr::from_ptr(stage)
            .to_string_lossy()
            .into_owned()
    };
    carrier.progress(fraction, &stage);
}

// SAFETY: as `on_progress`.
pub(crate) unsafe extern "C" fn on_cancel(user: *mut std::ffi::c_void) -> std::os::raw::c_int {
    let Some(carrier) = (user as *mut &mut dyn Watcher).as_mut() else {
        return 0;
    };
    i32::from(carrier.cancelled())
}

/// Retopologises a triangle mesh to quads.
pub fn remesh<W: Watcher>(mesh: &Mesh, params: RemeshParams, watcher: &mut W) -> Result<Mesh> {
    let raw_params = params.to_raw();
    let mut out = std::ptr::null_mut();

    // The trait object the two C callbacks reach the watcher through. Its
    // address is passed as the engine's `user` and is valid for exactly the
    // duration of the call below, which is the only time either callback runs.
    let mut carrier: &mut dyn Watcher = watcher;
    let user = &mut carrier as *mut &mut dyn Watcher as *mut std::ffi::c_void;

    check(
        // SAFETY: a valid input mesh, a filled descriptor, two callbacks whose
        // `user` is the carrier above, and an out-parameter written only on
        // success.
        unsafe {
            sys::cyber_remesh(
                mesh.as_ptr(),
                &raw_params,
                Some(on_progress),
                Some(on_cancel),
                user,
                &mut out,
            )
        },
        "cyber_remesh",
    )?;
    Mesh::from_raw(out, "cyber_remesh")
}

/// Whether a status is the engine's "the caller stopped it".
///
/// Distinguished from a failure because it is not one: a cancelled retopology
/// is the sculptor changing their mind, and reporting it as an error would put
/// a refusal in front of them for something they asked for.
pub fn was_cancelled(error: &Error) -> bool {
    error.status.contains("CANCEL")
}

#[cfg(test)]
mod tests {
    use super::QuadMethod;

    /// The five offered methods name five different quadrangulators.
    ///
    /// `raw` reads the generated constants, so the *values* cannot drift from
    /// the header — but nothing there stops two of them from becoming equal.
    /// If a future header aliased one method to another, every method would
    /// still run and `every_offered_quad_method_runs` would still pass, while
    /// the interface offered the sculptor two entries that did the same thing.
    /// That is the failure this catches, and it needs no engine to catch it.
    #[test]
    fn every_offered_method_is_a_distinct_quadrangulator() {
        for (i, a) in QuadMethod::ALL.iter().enumerate() {
            for b in &QuadMethod::ALL[i + 1..] {
                assert_ne!(
                    a.raw(),
                    b.raw(),
                    "{} and {} both map to {} — the interface would offer the \
                     same quadrangulator twice",
                    a.label(),
                    b.label(),
                    a.raw()
                );
            }
        }
    }
}
