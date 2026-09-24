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
    /// **The ABI number now exists, and this asserts on it.** Until v0.9.0 the
    /// engine's `SOVERSION` was its project major, still 0, so
    /// `libcyber_capi.so.0` named v0.7.0 and v0.8.0 alike — a mismatched
    /// library linked without complaint and surfaced later as behaviour rather
    /// than as an error. The note that used to sit here said "the day one
    /// exists, assert on it here instead", and v0.9.0 is that day:
    /// `CYBER_ABI_VERSION_MAJOR`/`_MINOR` are 2 and 1, and `cyber_abi_check`
    /// applies the compatibility rule rather than leaving a caller to compare
    /// numbers by hand — which its own header tells you not to do.
    ///
    /// The release version is still asserted below it. The two answer different
    /// questions: the ABI check says this library can serve a client compiled
    /// against these headers, and the release number says the submodule is the
    /// one this workspace pins. A library could satisfy the first and still be
    /// the wrong build.
    /// The library can serve a client compiled against the headers we built with.
    ///
    /// `cyber_abi_check` takes the numbers as the CALLING translation unit saw
    /// them — which is the whole point of the call, in the header's own words:
    /// "the header you built with answers, not the header you are reading now."
    /// Comparing `cyber_abi_version` against our constants by hand would ask
    /// the second question, and the header says not to.
    ///
    /// It never aborts or exits, because "a library that kills its host is
    /// unusable inside a DCC". So the reaction is ours, and here it is a failed
    /// test rather than a refusal at runtime: a build that links the wrong
    /// library should not reach a sculptor at all.
    #[test]
    fn the_linked_library_can_serve_this_client() {
        // SAFETY: two plain ints in, a status out. The call takes no pointers
        // and no handles, its header states it cannot abort or exit, and it is
        // safe to call before any document exists.
        let status = unsafe {
            cyberremesh_sys::cyber_abi_check(
                cyberremesh_sys::CYBER_ABI_VERSION_MAJOR as i32,
                cyberremesh_sys::CYBER_ABI_VERSION_MINOR as i32,
            )
        };
        assert_eq!(
            status,
            cyberremesh_sys::CyberStatus::CYBER_OK,
            "the linked retopology library refused the ABI these headers \
             compiled against ({}.{}), so it is not a build this client can \
             use however the release number reads",
            cyberremesh_sys::CYBER_ABI_VERSION_MAJOR,
            cyberremesh_sys::CYBER_ABI_VERSION_MINOR,
        );
    }

    #[test]
    fn the_pinned_engine_is_the_one_this_workspace_expects() {
        let found = version();
        assert_eq!(
            (found.major, found.minor),
            (0, 10),
            "the retopology engine reports {found}, and this workspace pins \
             v0.10.0. The submodule and this constant disagree — the pin is the \
             submodule commit, and the soname is not what would have caught it"
        );
    }
}
