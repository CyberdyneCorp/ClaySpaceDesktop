## ADDED Requirements

### Requirement: A stale local CMake cache explains how to recover
Before configuring CyberRemesher, the build SHALL reject a cached absolute
macOS SDK path or CMake executable that no longer exists, and a cached source
directory from another worktree. The error SHALL name the stale path and the
command that cleans the affected crate. A clean checkout SHALL run the
workspace clippy gate with the available CMake on `PATH`.

#### Scenario: An Xcode upgrade removes the cached SDK
- **WHEN** the CyberRemesher CMake cache names an SDK directory that no longer exists
- **THEN** the build stops with the cached path and `cargo clean -p cyberremesh-sys` as the recovery action

#### Scenario: A different worktree reuses the cache
- **WHEN** the cache names a source directory different from this checkout's pinned submodule
- **THEN** the build stops before CMake runs and names the old directory
