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

/// Which seamless-UV solver the loaded library was built with.
///
/// **This is not cosmetic.** The in-process QuadCover field is the shipping
/// default quadrangulator, and a build without it does not fail — it routes to
/// the portable solver and produces genuinely different quads. The difference
/// is invisible until output quality is worse and nobody knows why, which is
/// why it is readable at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Solver {
    /// `native+geogram` — the in-process QuadCover field. What we require.
    NativeAndGeogram,
    /// `native` — the portable fallback.
    Native,
    /// Something this build of the wrapper does not recognise, carried rather
    /// than mapped onto either: a solver name we cannot read is a fact about
    /// the library and guessing at it would be the one mistake that matters.
    Unknown,
}

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
