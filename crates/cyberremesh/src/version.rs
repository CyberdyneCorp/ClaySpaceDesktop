//! Which engine this is, and which quadrangulator it got.

use cyberremesh_sys as sys;

/// The engine's project version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// NO `Solver` TYPE HERE, and the absence is the finding rather than an
// omission.
//
// One stood here: an enum of `NativeAndGeogram` / `Native` / `Unknown` whose
// doc said the in-process QuadCover field is "readable at all" because a build
// without it silently routes to the portable quadrangulator. The reasoning was
// right and the type was **dead**: nothing constructed it, because
// `cyber_capi.h` exposes no solver-name entry point at all — `grep -n solver`
// over the header returns prose and nothing else. So the doc asserted a
// capability the ABI does not offer, which is worse than having no type: a
// reader would go looking for the reader function.
//
// What actually guarantees the solver is build-time, and it is stronger than a
// runtime string would be. `cmake/QuadCoverSolver.cmake:105` turns a missing
// in-process field into `FATAL_ERROR` when `CYBER_REQUIRE_QUADCOVER=ON`, which
// `cyberremesh-sys/build.rs` sets — so a build that cannot have QuadCover does
// not produce a library that quietly quadrangulates differently, it fails to
// configure. A string check would have run after the fact; this one runs
// before there is anything to check.
//
// The CLI does print `seamless-uv-solver native+geogram`, but that is the
// CLI's own report and not the library we link, so reading it would be a
// measurement of the wrong artifact.

/// The engine's version.
pub fn version() -> Version {
    let (mut major, mut minor, mut patch) = (0, 0, 0);
    // SAFETY: three valid out-pointers; the call writes each and reads nothing.
    unsafe { sys::cyber_version(&mut major, &mut minor, &mut patch) };
    Version {
        major: major as u32,
        minor: minor as u32,
        patch: patch as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pin is the submodule commit, and this is the informational check
    /// beside it.
    ///
    /// **There is no ABI number to assert on.** The engine's `SOVERSION` is its
    /// project major, still 0, so `libcyber_capi.so.0` names v0.7.0 and v0.8.0
    /// alike — a mismatched library links without complaint and surfaces later
    /// as behaviour rather than as an error. So this is weaker than the
    /// `EXPECTED_ABI` assertion the sibling engine gets, and it is weaker
    /// *because of the engine* rather than by choice. Its authors know and have
    /// put an ABI-level number to their user; the day one exists, assert on it
    /// here instead.
    #[test]
    fn the_pinned_engine_is_the_one_this_workspace_expects() {
        let found = version();
        assert_eq!(
            (found.major, found.minor),
            (0, 8),
            "the retopology engine reports {found}, and this workspace pins \
             v0.8.0. The submodule and this constant disagree — the pin is the \
             submodule commit, and there is no soname change to have caught it"
        );
    }
}
