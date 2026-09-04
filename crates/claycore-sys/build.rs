//! Configures, builds and links ClayCore, then generates the raw bindings.
//!
//! Every failure here is reported before CMake or the linker gets a chance to
//! produce a less legible one: a missing submodule, a CMake older than the
//! engine requires, or an accelerated backend asked for on a machine that
//! cannot build it.

use std::path::{Path, PathBuf};
use std::process::Command;

const REQUIRED_CMAKE: (u32, u32) = (3, 24);

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let engine = manifest
        .join("../../vendor/ClayCore")
        .canonicalize()
        .unwrap_or_else(|_| manifest.join("../../vendor/ClayCore"));

    check_submodule(&engine);
    check_submodule_revision(&engine, &manifest.join("../.."));
    record_revision(&engine);
    check_cmake();

    let backends = select_backends();
    let build_dir = build_engine(&engine, &backends);
    emit_link_flags(&build_dir, &backends);
    generate_bindings(&engine);
    emit_rerun_directives(&engine);
}

/// Stamps the vendored engine's revision into the build.
///
/// The version string alone is not enough to identify an engine: two builds
/// can both say 0.27.3 and differ by a commit. This project has already filed
/// a round of issues against a stale engine for exactly that reason, so the
/// revision travels with the binary and into the diagnostics report.
///
/// `--long` so the hash is always present: without it a checkout sitting
/// exactly on a tag reports only the tag, which is the version we already
/// have and not the commit we wanted.
///
/// A source tree with no git — a vendored tarball, a package build — reports
/// so rather than failing: the revision is a diagnostic, not a requirement.
fn record_revision(engine: &Path) {
    let described = Command::new("git")
        .args(["-C"])
        .arg(engine)
        .args(["describe", "--always", "--dirty", "--tags", "--long"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|revision| !revision.is_empty())
        .unwrap_or_else(|| "unknown (no git checkout)".to_string());
    println!("cargo::rustc-env=CLAYCORE_REVISION={described}");
}

/// A clone without `--recurse-submodules` is the most likely first failure.
/// Say so, with the command that fixes it.
fn check_submodule(engine: &Path) {
    if engine.join("bindings/c/clay.h").is_file() {
        return;
    }
    panic!(
        "\n\nClayCore is missing at {}\n\n\
         The engine is a git submodule and was not checked out. Run:\n\n    \
         git submodule update --init --recursive\n\n\
         (or clone with --recurse-submodules)\n",
        engine.display()
    );
}

/// Whether the checked-out engine is the revision this workspace pins.
///
/// A `git pull` that moves the pin moves the *gitlink* and not the submodule's
/// working copy, so the tree ends up compiling code written against one engine
/// against the headers of another. What that produces is a wall of
/// `unresolved import` on generated bindings — `sys::clay_multires_error`,
/// `sys::clay_maintenance_kind` — which names neither the cause nor the fix,
/// and has cost this repository an afternoon twice now.
///
/// **Stale is not the same as deliberate.** `just engine-pin` checks the
/// submodule out at a new tag and tells you to rebuild *before* the new pointer
/// is committed, so during that workflow the working copy is ahead of the
/// gitlink on purpose. Failing there would break the one command whose whole
/// job is to move the pin.
///
/// The two are told apart by direction: a checkout that is an *ancestor* of the
/// pinned revision is behind it, which is the `git pull` case and a hard error.
/// Anything else — ahead, or on an unrelated branch — is somebody moving the
/// engine on purpose, and gets a warning it can ignore.
///
/// Every git failure here is a reason to say nothing. A source tree with no
/// git, a vendored tarball, a package build: none of them can be stale, because
/// none of them has a pin to be stale against.
fn check_submodule_revision(engine: &Path, workspace: &Path) {
    let Some(pinned) = git(workspace, &["rev-parse", "HEAD:vendor/ClayCore"]) else {
        return;
    };
    let Some(checked_out) = git(engine, &["rev-parse", "HEAD"]) else {
        return;
    };
    if pinned == checked_out {
        return;
    }

    // A pinned revision the submodule does not even have is the strongest
    // evidence of the stale case there is: the pointer moved and nothing
    // fetched it. `--is-ancestor` cannot answer for an object it lacks, so this
    // is asked first rather than left to fail into the warning below.
    // Asked by exit status rather than through `git`, which reads an empty
    // stdout as a failure — and `cat-file -e` says yes by saying nothing.
    let has_pinned = Command::new("git")
        .arg("-C")
        .arg(engine)
        .args(["cat-file", "-e", &format!("{pinned}^{{commit}}")])
        .status()
        .is_ok_and(|status| status.success());
    let behind = has_pinned
        && Command::new("git")
            .arg("-C")
            .arg(engine)
            .args(["merge-base", "--is-ancestor", &checked_out, &pinned])
            .status()
            .is_ok_and(|status| status.success());

    if behind || !has_pinned {
        panic!(
            "\n\nClayCore is checked out at the wrong revision.\n\n    \
             this workspace pins {pinned}\n    \
             vendor/ClayCore is at  {checked_out}\n\n\
             The pin moved and the submodule's working copy did not follow — a\n\
             `git pull` moves the pointer, not the checkout. Run:\n\n    \
             git submodule update --init --recursive\n\n\
             Without this the generated bindings are the old engine's, and the\n\
             failure is a wall of `unresolved import` naming neither cause nor\n\
             fix.\n"
        );
    }

    println!(
        "cargo::warning=vendor/ClayCore is at {checked_out}, ahead of the pinned \
         {pinned} — building against the checkout. Commit the new pointer, or run \
         `git submodule update --init --recursive` to go back."
    );
}

/// One git command, or `None` for any reason it did not answer.
fn git(at: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(at)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn check_cmake() {
    let out = Command::new("cmake").arg("--version").output();
    let stdout = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => panic!(
            "\n\ncmake was not found on PATH.\n\n\
             ClayCore is a C++20 CMake project and is built as part of this crate.\n\
             Install CMake {}.{} or newer.\n",
            REQUIRED_CMAKE.0, REQUIRED_CMAKE.1
        ),
    };

    let version = stdout
        .split_whitespace()
        .find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .unwrap_or("0.0.0");
    let mut parts = version.split('.').filter_map(|p| p.parse::<u32>().ok());
    let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));

    if (major, minor) < REQUIRED_CMAKE {
        panic!(
            "\n\ncmake {version} is too old.\n\n\
             ClayCore requires CMake {}.{} or newer.\n",
            REQUIRED_CMAKE.0, REQUIRED_CMAKE.1
        );
    }
}

#[derive(Default)]
struct Backends {
    metal: bool,
    cuda: bool,
    vulkan: bool,
    opencl: bool,
}

impl Backends {
    /// Names as the engine's runtime registry reports them, best first for
    /// this platform. The CPU backend is always compiled in and is not listed
    /// here because it is not a build decision.
    fn accelerated_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.metal {
            names.push("metal");
        }
        if self.cuda {
            names.push("cuda");
        }
        if self.vulkan {
            names.push("vulkan");
        }
        if self.opencl {
            names.push("opencl");
        }
        names
    }
}

/// An explicitly requested backend whose toolchain is absent is a hard error:
/// silently dropping it would produce a binary that cannot register the
/// backend the caller asked for. With no features named, probe the platform.
fn select_backends() -> Backends {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let asked =
        |name: &str| std::env::var(format!("CARGO_FEATURE_{}", name.to_uppercase())).is_ok();

    // A way to ask for no accelerated backend at all.
    //
    // Probing the platform is right for a developer build, and wrong for the
    // CI row named "CPU only": on macOS the probe finds Metal and the row
    // silently tests the same configuration as the row next to it. The CPU
    // backend is compiled into the engine unconditionally, so this asks for
    // nothing rather than for something absent.
    println!("cargo:rerun-if-env-changed=CLAYCORE_CPU_ONLY");
    let cpu_only = std::env::var_os("CLAYCORE_CPU_ONLY").is_some();

    let (metal_req, cuda_req, vulkan_req, opencl_req) = (
        asked("metal"),
        asked("cuda"),
        asked("vulkan"),
        asked("opencl"),
    );
    let any_requested = metal_req || cuda_req || vulkan_req || opencl_req;

    let mut b = Backends::default();

    if cpu_only && any_requested {
        // Contradictory. Refusing beats picking one silently: whichever way it
        // went, the build would not be the one someone asked for.
        panic!(
            "\n\nCLAYCORE_CPU_ONLY is set and an accelerated backend feature was \
             also requested.\n\nDrop one of the two.\n"
        );
    }

    if metal_req {
        require(
            target_os == "macos" && has_metal(),
            "metal",
            "the Metal toolchain (Xcode command line tools) on macOS",
        );
        b.metal = true;
    }
    if cuda_req {
        require(
            has_cuda(),
            "cuda",
            "the CUDA toolkit (nvcc on PATH, or CUDA_PATH set)",
        );
        b.cuda = true;
    }
    if vulkan_req {
        require(
            has_vulkan(),
            "vulkan",
            "the Vulkan SDK (VULKAN_SDK set, or vulkan discoverable by pkg-config)",
        );
        b.vulkan = true;
    }
    if opencl_req {
        require(has_opencl(), "opencl", "an OpenCL runtime");
        b.opencl = true;
    }

    if !any_requested && !cpu_only {
        // Default: take what this machine can actually build.
        match target_os.as_str() {
            "macos" => b.metal = has_metal(),
            "linux" => {
                b.cuda = has_cuda();
                b.vulkan = has_vulkan();
            }
            _ => {}
        }
    }

    let names = b.accelerated_names();
    if names.is_empty() {
        println!("cargo:warning=ClayCore: building CPU-only (no accelerated backend selected or detected)");
    }
    // Consumed by the crate to report what was compiled in, distinct from what
    // the engine registers at runtime.
    println!(
        "cargo:rustc-env=CLAYCORE_COMPILED_BACKENDS={}",
        names.join(",")
    );
    b
}

fn require(ok: bool, feature: &str, needs: &str) {
    if !ok {
        panic!(
            "\n\nFeature `{feature}` was requested but {needs} was not found.\n\n\
             Either install it, or build without the `{feature}` feature.\n\
             The CPU backend is always compiled in, so a CPU-only build always works.\n"
        );
    }
}

fn has_metal() -> bool {
    Command::new("xcrun")
        .args(["-sdk", "macosx", "--find", "metal"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn has_cuda() -> bool {
    std::env::var_os("CUDA_PATH").is_some()
        || Command::new("nvcc")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

fn has_vulkan() -> bool {
    std::env::var_os("VULKAN_SDK").is_some()
        || Command::new("pkg-config")
            .args(["--exists", "vulkan"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
}

fn has_opencl() -> bool {
    Command::new("pkg-config")
        .args(["--exists", "OpenCL"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn build_engine(engine: &Path, b: &Backends) -> PathBuf {
    let flag = |on: bool| if on { "ON" } else { "OFF" };

    let mut cfg = cmake::Config::new(engine);
    cfg.define("CLAY_BUILD_TESTS", "OFF")
        .define("CLAY_BUILD_BENCHMARKS", "OFF")
        .define("CLAY_BUILD_PYTHON", "OFF")
        // The engine treats warnings as errors for its own CI. A consumer
        // pinning a revision should not fail on a warning from a newer
        // compiler than that revision was written against.
        .define("CLAY_WERROR", "OFF")
        .define("CLAY_BACKEND_METAL", flag(b.metal))
        .define("CLAY_BACKEND_CUDA", flag(b.cuda))
        .define("CLAY_BACKEND_VULKAN", flag(b.vulkan))
        .define("CLAY_BACKEND_OPENCL", flag(b.opencl))
        .define("CMAKE_BUILD_TYPE", "Release")
        // There are no install rules in the engine, so build the target and
        // link out of the build tree.
        .build_target("claycore");

    if b.cuda {
        // The engine compiles its CUDA with separable compilation on, so each
        // device object carries a reference to `__cudaRegisterLinkedBinary_*`
        // that only the device link (`nvcc -dlink`) defines. CMake runs that
        // link when one of *its own* targets consumes the library — the
        // engine's shared library, benchmarks and tests all do, which is why
        // its own build is fine. Here the final link is rustc's, and nothing
        // ever device-links, so every binary in the workspace failed with
        // `undefined symbol: __cudaRegisterLinkedBinary_..._clay_kernels_cu_...`.
        //
        // This asks CMake to device-link into the archive itself, which is
        // what the property is for: a static library whose consumer is not
        // CMake. `cmake_device_link.o` becomes a member of `libclaycore.a` and
        // defines the symbol.
        cfg.define("CMAKE_CUDA_RESOLVE_DEVICE_SYMBOLS", "ON");
    }

    let dst = cfg.build();
    dst.join("build")
}

/// Static archives, found rather than assumed: the engine's dependencies are
/// FetchContent subprojects whose output paths belong to CMake, not to us.
fn emit_link_flags(build_dir: &Path, b: &Backends) {
    let mut roots = vec![build_dir.to_path_buf()];
    let deps = build_dir.join("_deps");
    if deps.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&deps) {
            roots.extend(
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_dir()),
            );
        }
    }

    let mut found = Vec::new();
    for root in &roots {
        collect_archives(root, &mut found, 0);
    }

    for dir in dedup(
        found
            .iter()
            .filter_map(|p| p.parent().map(Path::to_path_buf)),
    ) {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }

    // claycore first: static link order matters for the transitive deps.
    println!("cargo:rustc-link-lib=static=claycore");
    for name in dedup(found.iter().map(PathBuf::as_path).filter_map(archive_stem)) {
        if name != "claycore" {
            println!("cargo:rustc-link-lib=static={name}");
        }
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target_os.as_str() {
        "macos" => {
            println!("cargo:rustc-link-lib=c++");
            if b.metal {
                println!("cargo:rustc-link-lib=framework=Metal");
                println!("cargo:rustc-link-lib=framework=Foundation");
            }
        }
        "linux" => {
            println!("cargo:rustc-link-lib=stdc++");
            println!("cargo:rustc-link-lib=pthread");
            // The engine links these itself with `target_link_libraries`, but
            // `claycore` is a STATIC library: that records a usage requirement
            // for CMake consumers and puts nothing in the archive. The final
            // link is rustc's, so the loader has to be named here or the
            // backend compiles and then fails to link — which is exactly what
            // the Linux Vulkan row did, with a page of `undefined symbol:
            // vkDestroyPipeline`.
            if b.vulkan {
                println!("cargo:rustc-link-lib=vulkan");
            }
            if b.opencl {
                println!("cargo:rustc-link-lib=OpenCL");
            }
            if b.cuda {
                // The toolkit is rarely on the default search path.
                for root in ["CUDA_PATH", "CUDA_HOME"]
                    .iter()
                    .filter_map(|k| std::env::var(k).ok())
                {
                    for dir in ["lib64", "lib"] {
                        let path = Path::new(&root).join(dir);
                        if path.is_dir() {
                            println!("cargo:rustc-link-search=native={}", path.display());
                        }
                    }
                }
                println!("cargo:rustc-link-lib=cudart");
                // The device-link object CMAKE_CUDA_RESOLVE_DEVICE_SYMBOLS
                // produces references the device runtime's fatbin wrapper, so
                // the archive that defines it has to be on the link line too.
                // Static only — there is no shared cudadevrt — and it is the
                // second half of the fix: without it the undefined symbol just
                // moves from `__cudaRegisterLinkedBinary_*` to
                // `__fatbinwrap_*_cuda_device_runtime_cu_*`.
                //
                // The search paths above only cover a toolkit install. A
                // distribution package puts nvcc in /usr/bin and the archive
                // in the compiler's own library directory, where the toolkit
                // roots never point — so ask the compiler, which is the one
                // that knows.
                if let Some(dir) = cudadevrt_dir() {
                    println!("cargo:rustc-link-search=native={}", dir.display());
                }
                println!("cargo:rustc-link-lib=static=cudadevrt");
            }
        }
        _ => {}
    }
}

/// Where `libcudadevrt.a` lives, asked of the C compiler.
///
/// `-print-file-name` answers with the name unchanged when it knows nothing,
/// so an answer is only useful if it is a path that exists.
fn cudadevrt_dir() -> Option<PathBuf> {
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let out = Command::new(cc)
        .arg("-print-file-name=libcudadevrt.a")
        .output()
        .ok()?;
    let path = PathBuf::from(String::from_utf8(out.stdout).ok()?.trim());
    path.is_file()
        .then(|| path.parent().map(Path::to_path_buf))?
}

fn collect_archives(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_archives(&path, out, depth + 1);
        } else if path.extension().is_some_and(|e| e == "a") {
            out.push(path);
        }
    }
}

fn archive_stem(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    Some(stem.strip_prefix("lib").unwrap_or(stem).to_string())
}

fn dedup<T: Ord>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut v: Vec<T> = items.into_iter().collect();
    v.sort();
    v.dedup();
    v
}

fn generate_bindings(engine: &Path) {
    let header = engine.join("bindings/c/clay.h");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

    bindgen::Builder::default()
        .header(header.to_string_lossy())
        .allowlist_item("clay_.*")
        .allowlist_item("CLAY_.*")
        // The engine's descriptor structs are versioned by `struct_size`, so
        // the safe layer needs Default to fill them from size_of.
        .derive_default(true)
        .derive_debug(true)
        .derive_copy(true)
        .prepend_enum_name(false)
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .layout_tests(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen failed to generate bindings for clay.h")
        .write_to_file(out.join("bindings.rs"))
        .expect("failed to write bindings.rs");
}

fn emit_rerun_directives(engine: &Path) {
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rerun-if-changed={}",
        engine.join("bindings/c/clay.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        engine.join("bindings/c/clay_c.cpp").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        engine.join("CMakeLists.txt").display()
    );
    for dir in ["include", "src", "backends", "cmake"] {
        let p = engine.join(dir);
        if p.is_dir() {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
}
