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
    check_cmake();

    let backends = select_backends();
    let build_dir = build_engine(&engine, &backends);
    emit_link_flags(&build_dir, &backends);
    generate_bindings(&engine);
    emit_rerun_directives(&engine);
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
    let asked = |name: &str| std::env::var(format!("CARGO_FEATURE_{}", name.to_uppercase())).is_ok();

    let (metal_req, cuda_req, vulkan_req, opencl_req) = (
        asked("metal"),
        asked("cuda"),
        asked("vulkan"),
        asked("opencl"),
    );
    let any_requested = metal_req || cuda_req || vulkan_req || opencl_req;

    let mut b = Backends::default();

    if metal_req {
        require(target_os == "macos" && has_metal(), "metal", "the Metal toolchain (Xcode command line tools) on macOS");
        b.metal = true;
    }
    if cuda_req {
        require(has_cuda(), "cuda", "the CUDA toolkit (nvcc on PATH, or CUDA_PATH set)");
        b.cuda = true;
    }
    if vulkan_req {
        require(has_vulkan(), "vulkan", "the Vulkan SDK (VULKAN_SDK set, or vulkan discoverable by pkg-config)");
        b.vulkan = true;
    }
    if opencl_req {
        require(has_opencl(), "opencl", "an OpenCL runtime");
        b.opencl = true;
    }

    if !any_requested {
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
    println!("cargo:rustc-env=CLAYCORE_COMPILED_BACKENDS={}", names.join(","));
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
            roots.extend(entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()));
        }
    }

    let mut found = Vec::new();
    for root in &roots {
        collect_archives(root, &mut found, 0);
    }

    for dir in dedup(found.iter().filter_map(|p| p.parent().map(Path::to_path_buf))) {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }

    // claycore first: static link order matters for the transitive deps.
    println!("cargo:rustc-link-lib=static=claycore");
    for name in dedup(found.iter().filter_map(archive_stem)) {
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
        }
        _ => {}
    }
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

fn archive_stem(path: &PathBuf) -> Option<String> {
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
    println!("cargo:rerun-if-changed={}", engine.join("bindings/c/clay.h").display());
    println!("cargo:rerun-if-changed={}", engine.join("bindings/c/clay_c.cpp").display());
    println!("cargo:rerun-if-changed={}", engine.join("CMakeLists.txt").display());
    for dir in ["include", "src", "backends", "cmake"] {
        let p = engine.join(dir);
        if p.is_dir() {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
}
