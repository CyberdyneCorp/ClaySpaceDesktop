//! Configures, builds and links CyberRemesher, then generates the raw bindings.
//!
//! Modelled on `claycore-sys`, which is the pattern this workspace already
//! uses for a vendored C++ engine, and diverging from it only where this engine
//! differs:
//!
//! - **CPU only, deliberately.** ClayCore holds the accelerated backend. Two
//!   engines contending for one CUDA device mid-stroke is a latency fault that
//!   cannot be read off a frame time, so the retopologiser is built without
//!   any GPU backend and its worker pool is capped by the application.
//! - **QuadCover is required rather than merely enabled.** A build without the
//!   in-process solver does not fail: it silently routes to the portable
//!   quadrangulator and produces genuinely different quads. `REQUIRE` turns
//!   that into a configure error.
//! - **No ABI number to assert.** This engine's `SOVERSION` is its project
//!   major, still 0, so `libcyber_capi.so.0` names every 0.x release. The pin
//!   is the submodule commit and `cyber_version` is informational.

use std::path::{Path, PathBuf};
use std::process::Command;

const REQUIRED_CMAKE: (u32, u32) = (3, 24);

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let engine = manifest
        .join("../../vendor/CyberRemesherAndUV")
        .canonicalize()
        .unwrap_or_else(|_| manifest.join("../../vendor/CyberRemesherAndUV"));

    check_submodule(&engine);
    check_submodule_revision(&engine, &manifest.join("../.."));
    check_cmake();

    let build = build_engine(&engine);
    emit_link_flags(&build);
    generate_bindings(&engine);
    emit_rerun_directives(&engine);
}

fn check_submodule(engine: &Path) {
    if engine.join("capi/include/cyber_capi.h").is_file() {
        return;
    }
    panic!(
        "\n\nCyberRemesher is missing at {}\n\n\
         The retopology engine is a git submodule and was not checked out. Run:\n\n    \
         git submodule update --init --recursive\n\n\
         (or clone with --recurse-submodules)\n",
        engine.display()
    );
}

/// Whether the checked-out engine is the revision this workspace pins.
///
/// The same guard `claycore-sys` carries, and for the same reason: a `git pull`
/// that moves the pin moves the *gitlink* and not the submodule's working copy,
/// so the tree compiles code written against one engine against the headers of
/// another. What that produces is a wall of `unresolved import` on generated
/// bindings, which names neither the cause nor the fix.
///
/// It matters more here than there. This engine's soname does not move between
/// minor releases — `libcyber_capi.so.0` for every 0.x — so there is no linker
/// error to fall back on when the revision is wrong.
fn check_submodule_revision(engine: &Path, workspace: &Path) {
    let Some(pinned) = git(workspace, &["rev-parse", "HEAD:vendor/CyberRemesherAndUV"]) else {
        return;
    };
    let Some(checked_out) = git(engine, &["rev-parse", "HEAD"]) else {
        return;
    };
    if pinned == checked_out {
        return;
    }
    // A pinned revision the submodule does not have is the strongest evidence
    // of the stale case: the pointer moved and nothing fetched it. Asked with
    // `cat-file -e` first, because `--is-ancestor` returns non-zero both for
    // "not an ancestor" and for "no such object" — the two are different facts
    // and only one of them is this one.
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

    let advice = if behind || !has_pinned {
        "The submodule is behind what this workspace pins. Run:\n\n    \
         git submodule update --init --recursive\n"
    } else {
        "The submodule is at a revision this workspace does not pin. If that is \
         deliberate, commit the new pin; if not, run:\n\n    \
         git submodule update --init --recursive\n"
    };
    panic!(
        "\n\nCyberRemesher is at the wrong revision.\n\n  \
         pinned:      {pinned}\n  checked out: {checked_out}\n\n{advice}"
    );
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn check_cmake() {
    let Some(version) = Command::new("cmake")
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
    else {
        panic!("\n\nCMake was not found, and the retopology engine is built with it.\n");
    };
    let numbers: Vec<u32> = version
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .find(|word| word.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(|v| v.split('.').filter_map(|p| p.parse().ok()).collect())
        .unwrap_or_default();
    if let (Some(&major), Some(&minor)) = (numbers.first(), numbers.get(1)) {
        if (major, minor) < REQUIRED_CMAKE {
            panic!(
                "\n\nCMake {major}.{minor} is older than the {}.{} the retopology \
                 engine requires.\n",
                REQUIRED_CMAKE.0, REQUIRED_CMAKE.1
            );
        }
    }
}

fn build_engine(engine: &Path) -> PathBuf {
    let mut cfg = cmake::Config::new(engine);
    // Every name here was read out of the engine's own `CMakeLists.txt` rather
    // than guessed from the pattern of the sibling engine's. Four of the first
    // set written from that pattern did not exist — the backends are
    // `CYBER_ENABLE_*` and not `CYBER_BACKEND_*`, and there is no werror or
    // examples switch at all. A `-D` for an option a project does not define
    // is silently ignored by CMake, so a wrong name here is not an error, it
    // is a setting that quietly does not apply.
    cfg.define("CYBER_BUILD_TESTS", "OFF")
        .define("CYBER_BUILD_CLI", "OFF")
        .define("CYBER_BUILD_NET", "OFF")
        .define("CYBER_BUILD_RENDER", "OFF")
        .define("CYBER_BUILD_APPS", "OFF")
        // What we do want: the C ABI facade, and the modules behind it.
        .define("CYBER_BUILD_CAPI", "ON")
        .define("CYBER_BUILD_UV", "ON")
        .define("CYBER_BUILD_RETOPO", "ON")
        .define("CYBER_BUILD_BAKECAGE", "ON")
        // **The whole point of this build, and it defaults OFF.** Enabled is
        // not enough on its own: without the solver the engine falls back to
        // the portable quadrangulator and produces genuinely different quads
        // with nothing to say so. `REQUIRE` — which also defaults OFF, and
        // lives in `cmake/QuadCoverSolver.cmake` rather than the top level —
        // turns a missing dependency into a configure error instead of a
        // silent fallback.
        .define("CYBER_WITH_QUADCOVER", "ON")
        // No GPU backend. ClayCore has the device; see the module comment.
        // All three already default OFF, and are stated so that a future
        // default flip does not quietly put a second engine on the card.
        .define("CYBER_ENABLE_CUDA", "OFF")
        .define("CYBER_ENABLE_METAL", "OFF")
        .define("CYBER_ENABLE_OPENCL", "OFF")
        .define("CMAKE_BUILD_TYPE", "Release")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        // **The shared target, not the static one**, and the first attempt
        // here got that wrong: `cyber_capi` is a STATIC archive holding only
        // `capi.cpp`, so linking it left every symbol it calls undefined —
        // `cyber::setMaxWorkerThreads()` and forty more. `cyber_capi_shared`
        // is the one the engine documents for consumers: it links the core,
        // the quadrangulator, UV, bake, retopo and the in-process solver
        // privately.
        //
        // **Why the archive is short, corrected.** This used to say the static
        // target lacked `CYBER_CAPI_WITH_UV` and so was "missing entry points
        // as well as symbols". That is false: `capi/CMakeLists.txt:21` sets
        // that definition on `cyber_capi`, and :86 sets the identical one on
        // `cyber_capi_shared` — the two artifacts compile the same surface.
        // What actually bites is line 14, where `cyber_capi` declares its
        // dependencies `PUBLIC`: that propagates through CMake's *link
        // interface* only, so hand-linking the archive from here gets
        // `capi.cpp` and nothing it calls. Same conclusion, different cause —
        // and the wrong cause would send the next reader to patch the
        // definitions instead of the linking.
        //
        // It is also the one with the linker version script, which matters
        // more here than in a single-engine host: it exports *only* `cyber_*`,
        // so the vendored Geogram, stb and tinygltf definitions inside it
        // cannot be interposed by ClayCore's own copies of the same
        // third-party code, and vice versa. Two statically linked engines
        // sharing a symbol namespace is a class of bug nobody wants to debug.
        .build_target("cyber_capi_shared");

    require_quadcover_where_it_is_supported(&mut cfg);

    let dst = cfg.build();
    dst.join("build")
}

/// Turn a missing QuadCover dependency into a configure error — on the
/// platforms where the engine's own project does that.
///
/// `CYBER_WITH_QUADCOVER=ON` above is not a guarantee: without OpenMP and TBB
/// the engine falls back to the portable quadrangulator and produces
/// genuinely different quads with nothing to say so. `REQUIRE` makes that a
/// `FATAL_ERROR` at `cmake/QuadCoverSolver.cmake:105`, which is the strongest
/// form available — it fails before there is anything to check, and there is
/// no runtime solver-name entry point in the C ABI to check with.
///
/// **Linux only, and that is the engine's own posture rather than ours.**
/// Their `release.yml` reads
/// `EXTRA: ${{ runner.os == 'Linux' && '-DCYBER_REQUIRE_QUADCOVER=ON' || '' }}`,
/// and their comment calls Linux "the only leg that compiles and exercises the
/// `CYBER_HAVE_QUADCOVER`" branches; every `REQUIRE` leg in their
/// `hardening.yml` is Linux too. Requiring it on macOS was this crate's first
/// attempt and it failed every macOS job — a requirement upstream does not
/// support is a requirement that only breaks the build.
///
/// So macOS takes `WITH` without `REQUIRE`, and the fallback is **announced
/// rather than silent**: a build there says which way it went, because the
/// thing that makes the fallback dangerous is that nobody knows it happened.
fn require_quadcover_where_it_is_supported(cfg: &mut cmake::Config) {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        cfg.define("CYBER_REQUIRE_QUADCOVER", "ON");
        return;
    }
    println!(
        "cargo:warning=CyberRemesher: QuadCover requested but not required on \
         {target_os} — the engine's own release workflow requires it on Linux \
         only. If OpenMP and TBB are absent here the build silently uses the \
         portable quadrangulator, which produces different quads."
    );
}

fn emit_link_flags(build: &Path) {
    let capi = build.join("capi");
    println!("cargo:rustc-link-search=native={}", capi.display());
    // The shared library links every module privately, so there are no
    // transitive targets to name here.
    println!("cargo:rustc-link-lib=dylib=cyber_capi");
    // And it has to be findable at run time, not only at link time. An rpath
    // into the build directory is what makes `cargo test` work without a
    // `LD_LIBRARY_PATH` incantation in every invocation; packaging ships the
    // library beside the binary and sets its own.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", capi.display());
}

fn generate_bindings(engine: &Path) {
    let header = engine.join("capi/include/cyber_capi.h");
    let bindings = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .clang_arg(format!("-I{}", engine.join("capi/include").display()))
        // Only this engine's own surface. Without it the bindings carry every
        // libc declaration the header transitively includes.
        .allowlist_function("cyber_.*")
        .allowlist_type("Cyber.*")
        .allowlist_var("CYBER_.*")
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .derive_debug(true)
        .derive_default(true)
        // **On, matching `claycore-sys`.** This said `false` and the saving was
        // build time, which is the wrong trade for a crate whose entire risk
        // model is that the pin moves and nothing says so. The generated
        // `bindgen_test_layout_*` assertions pin every struct's size, its
        // alignment and each field's offset, per target, *from the header* —
        // so unlike a hand-written manifest they cannot rot.
        //
        // They check placement and not type identity, so a `const float*`
        // swapped for a `const double*` of the same width still passes. That
        // is a real hole and it is the reason they are a partial guard rather
        // than the whole one; the engine's own team hit exactly it when
        // designing a layout manifest, along with a field landing in existing
        // trailing padding. A `cargo test` that fails when a struct moves is
        // still worth more than the seconds it costs.
        .layout_tests(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("the retopology engine's header could not be bound");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("the generated bindings could not be written");
}

fn emit_rerun_directives(engine: &Path) {
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rerun-if-changed={}",
        engine.join("capi/include/cyber_capi.h").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        engine.join("CMakeLists.txt").display()
    );
}
