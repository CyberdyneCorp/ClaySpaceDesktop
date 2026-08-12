//! Which evaluation backends this machine actually has.
//!
//! Discovery is a runtime question, not a build one. A backend can be compiled
//! in and still fail to register — a CUDA build on a machine whose driver is
//! unavailable, say — so the only trustworthy answer comes from the engine.
//!
//! Ranking the answer is a policy decision and lives above this layer; this
//! module reports what is there.

use std::ffi::c_char;

use claycore_sys as sys;

use crate::buffer::size_query_string;
use crate::error::Result;

/// A backend the engine has registered on this machine.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Backend {
    /// Always present. Defines correctness: every other backend is held to it
    /// by the engine's parity suite.
    Cpu,
    Metal,
    Cuda,
    Vulkan,
    OpenCl,
    /// A backend this build does not know by name, carried verbatim so that a
    /// newer engine's backend is reportable rather than invisible.
    Other(String),
}

impl Backend {
    /// The name the engine's evaluation entry points expect.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Cpu => "cpu",
            Self::Metal => "metal",
            Self::Cuda => "cuda",
            Self::Vulkan => "vulkan",
            Self::OpenCl => "opencl",
            Self::Other(name) => name,
        }
    }

    fn parse(name: &str) -> Self {
        match name {
            "cpu" => Self::Cpu,
            "metal" => Self::Metal,
            "cuda" => Self::Cuda,
            "vulkan" => Self::Vulkan,
            "opencl" => Self::OpenCl,
            other => Self::Other(other.to_string()),
        }
    }
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Every backend the engine registered on this machine.
///
/// The CPU backend is compiled in unconditionally by the engine, so this never
/// returns an empty list on a working build.
pub fn backends() -> Result<Vec<Backend>> {
    let list = size_query_string(
        "clay_list_backends",
        |buffer: *mut c_char, size: *mut usize| {
            // SAFETY: the size-query helper passes either a null buffer with a
            // valid size out-parameter, or a buffer of at least `*size` bytes.
            unsafe { sys::clay_list_backends(buffer, size) }
        },
    )?;

    Ok(list
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(Backend::parse)
        .collect())
}

/// Accelerated backends compiled into this build.
///
/// Distinct from [`backends`]: this is what the build selected, which is a
/// necessary but not sufficient condition for a backend being available.
pub fn compiled_backends() -> Vec<Backend> {
    sys::COMPILED_BACKENDS
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(Backend::parse)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_is_always_registered() {
        let found = backends().expect("backend discovery must succeed");
        assert!(
            found.contains(&Backend::Cpu),
            "the engine compiles the CPU backend in unconditionally, but discovery returned {found:?}"
        );
    }

    #[test]
    fn every_compiled_backend_is_named_by_the_engine() {
        // A backend compiled in may legitimately fail to register at runtime,
        // so this asserts only that the two vocabularies agree — not that the
        // sets match.
        for backend in compiled_backends() {
            assert!(
                !matches!(backend, Backend::Other(_)),
                "build selected a backend this wrapper cannot name: {backend}"
            );
        }
    }
}
