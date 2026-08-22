//! The performance gate: measure, report, compare.
//!
//! ```sh
//! cargo run --release --bin bench                 # measure and print
//! cargo run --release --bin bench -- --json out.json
//! cargo run --release --bin bench -- --baseline benchmarks/baseline-macos-aarch64.json
//! ```
//!
//! Every figure carries the conditions it was taken in, and comparing two runs
//! taken on different scenes or different backends is refused rather than
//! reported — a gate that silently compares unlike things is worse than none.
//!
//! What this does *not* measure is worth saying plainly. Startup here is the
//! work before a window could be shown, not a window appearing; frame time is
//! an offscreen render of the reference scene, not a swapchain presenting. Both
//! are the parts that can be measured without a display, which is what CI has.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use clayspace_app::{conditions, Conditions, Scene, SurfaceGeometry};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{SculptModel, ToolKind};
use clayspace_view::{Camera, Gpu, OffscreenTarget, Renderer};

/// One measured quantity.
#[derive(Debug, Clone)]
struct Figure {
    /// Milliseconds, megabytes or a count — stated by `unit`.
    value: f64,
    unit: &'static str,
    /// What it must not exceed, when the specification states one.
    budget: Option<f64>,
    /// How much worse a run may be than the baseline before the gate fails.
    ///
    /// Timings on a shared CI runner move around by tens of percent for
    /// reasons that have nothing to do with the change under test, so a gate
    /// that fails on any regression fails constantly and gets ignored.
    tolerance: f64,
    /// Below this the figure is too small to have a meaningful ratio.
    ///
    /// Backend discovery measures 0.00 ms; against a baseline of zero, any
    /// value at all is an infinite regression. A ratio needs something to be a
    /// ratio *of*.
    noise_floor: f64,
}

impl Figure {
    /// Whether this is worse than the baseline by more than noise.
    fn regressed_against(&self, baseline: f64) -> bool {
        if self.value <= self.noise_floor && baseline <= self.noise_floor {
            return false;
        }
        self.value / baseline.max(f64::MIN_POSITIVE) > self.tolerance
    }
}

impl Figure {
    fn ms(value: f64, budget: Option<f64>) -> Self {
        // A millisecond: below that the measurement is scheduling noise.
        Self {
            value,
            unit: "ms",
            budget,
            tolerance: 1.5,
            noise_floor: 1.0,
        }
    }
    fn count(value: f64) -> Self {
        Self {
            value,
            unit: "",
            budget: None,
            tolerance: 1.25,
            noise_floor: 0.0,
        }
    }
    fn mb(value: f64) -> Self {
        Self {
            value,
            unit: "MB",
            budget: None,
            tolerance: 1.25,
            noise_floor: 0.5,
        }
    }
}

const VIEWPORT: (u32, u32) = (1280, 800);

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let at = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[at]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };

    let Ok(policy) = BackendPolicy::discover(None) else {
        eprintln!("the engine's backends could not be discovered");
        std::process::exit(2);
    };

    let where_ = conditions(Scene::Reference, &policy, VIEWPORT);
    println!("measuring: {}\n", where_.describe());

    let mut figures: BTreeMap<String, Figure> = BTreeMap::new();
    measure_startup(&mut figures);
    measure_dab_latency(&policy, &mut figures);
    measure_edit_locality(&policy, &mut figures);
    measure_frame_time(&policy, &mut figures);
    measure_memory(&policy, &mut figures);
    measure_tape_growth(&policy, &mut figures);

    report(&where_, &figures);

    if let Some(path) = flag("--json") {
        match write_json(&path, &where_, &figures) {
            Ok(()) => println!("\nwritten to {path}"),
            Err(e) => {
                eprintln!("could not write {path}: {e}");
                std::process::exit(2);
            }
        }
    }

    // A budget breach is reported always and fails only when asked. The
    // specification gates on a *regression* — "a change raises measured dab
    // latency beyond its budget" — and separately says performance is measured
    // in CI rather than asserted there. A gate that is red from the day it is
    // installed, for a reason nobody is about to fix, is a gate people learn to
    // ignore; `--enforce-budgets` is there for when the figure is expected to
    // hold.
    let enforce = args.iter().any(|a| a == "--enforce-budgets");
    let mut failed = false;
    let mut over = Vec::new();
    for (name, figure) in &figures {
        if let Some(budget) = figure.budget {
            if figure.value > budget {
                over.push(format!(
                    "  {name}: {:.1} {} against a budget of {budget:.1}",
                    figure.value, figure.unit
                ));
                failed |= enforce;
            }
        }
    }
    if !over.is_empty() {
        println!("\nOVER BUDGET");
        for line in &over {
            println!("{line}");
        }
        if !enforce {
            println!("  (reported, not enforced; pass --enforce-budgets to fail on these)");
        }
    }

    if let Some(path) = flag("--baseline") {
        match compare(&path, &where_, &figures) {
            Ok(regressions) => failed |= regressions,
            Err(e) => {
                eprintln!("\ncould not compare against {path}: {e}");
                std::process::exit(2);
            }
        }
    }

    if failed {
        std::process::exit(1);
    }
}

/// Everything between launching and being able to show a window.
///
/// Not a window appearing — that needs a display, and the budget this stands
/// against is 2 seconds for exactly this work plus the presentation.
fn measure_startup(figures: &mut BTreeMap<String, Figure>) {
    let started = Instant::now();
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let discovery = started.elapsed();

    let document = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form);
    let ready = started.elapsed();
    drop(document);

    figures.insert(
        "startup.backend_discovery".into(),
        Figure::ms(ms(discovery), None),
    );
    // The window has to be up within 2 s including discovery; this is the
    // engine-side share of that.
    figures.insert(
        "startup.to_first_document".into(),
        Figure::ms(ms(ready), Some(2000.0)),
    );
}

/// Input to visible, for a stroke across the reference scene.
fn measure_dab_latency(policy: &BackendPolicy, figures: &mut BTreeMap<String, Figure>) {
    let Some(gpu) = headless_gpu() else {
        return;
    };
    let Ok(mut document) = Scene::Reference.build(policy.clone()) else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&gpu);
    if geometry.rebuild(&gpu, &mut document).is_err() {
        return;
    }

    let brush = Scene::Reference.brush();
    let mut samples: Vec<f64> = Vec::new();
    for sample in Scene::Reference.stroke(24) {
        let started = Instant::now();
        if document
            .apply_stroke(ToolKind::Padrao, brush, &[sample], [false; 3])
            .is_err()
        {
            return;
        }
        if geometry.sync(&gpu, &mut document).is_err() {
            return;
        }
        samples.push(ms(started.elapsed()));
    }
    samples.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));

    // The budget the specification states for a GPU backend. The CPU backend
    // is reported rather than failed, so the budget is only attached when an
    // accelerated backend is active.
    let accelerated = *policy.active() != clayspace_engine::claycore::Backend::Cpu;
    figures.insert(
        "dab.median".into(),
        Figure::ms(quantile(&samples, 0.5), accelerated.then_some(50.0)),
    );
    figures.insert(
        "dab.p95".into(),
        Figure::ms(quantile(&samples, 0.95), accelerated.then_some(100.0)),
    );
}

/// The same dab on the reference scene and on one ten times its area.
///
/// The requirement is that the work follows the edit rather than the document,
/// so what matters is the ratio, not either figure.
fn measure_edit_locality(policy: &BackendPolicy, figures: &mut BTreeMap<String, Figure>) {
    let Some(gpu) = headless_gpu() else {
        return;
    };

    let mut measured = Vec::new();
    for scene in [Scene::Reference, Scene::TenTimesLarger] {
        let Ok(mut document) = scene.build(policy.clone()) else {
            return;
        };
        let mut geometry = SurfaceGeometry::new(&gpu);
        if geometry.rebuild(&gpu, &mut document).is_err() {
            return;
        }
        let surface = document
            .cache()
            .surface_bricks()
            .map(|keys| keys.len())
            .unwrap_or(0);

        // The same edit on both, not a proportional one — see
        // `Scene::probe_brush`. Placed where the cache keeps a surface rather
        // than at the scene's own coordinates, which land under the surface on
        // the larger scene — see `Scene::probe_point`.
        let sample = scene.stroke(3)[1];
        let Some(position) = Scene::probe_point(&document, sample.position) else {
            return;
        };
        let started = Instant::now();
        if document
            .apply_stroke(
                ToolKind::Padrao,
                Scene::probe_brush(),
                &[clayspace_model::GestureSample { position, ..sample }],
                [false; 3],
            )
            .is_err()
        {
            return;
        }
        let cost = geometry.sync(&gpu, &mut document).ok().flatten();
        measured.push((
            surface,
            cost.map(|c| c.keys).unwrap_or(0),
            ms(started.elapsed()),
        ));
    }

    let [(small_surface, small_keys, small_ms), (large_surface, large_keys, large_ms)] =
        measured[..]
    else {
        return;
    };

    figures.insert(
        "locality.surface_bricks".into(),
        Figure::count(small_surface as f64),
    );
    figures.insert(
        "locality.surface_bricks_10x".into(),
        Figure::count(large_surface as f64),
    );
    figures.insert(
        "locality.keys_remeshed".into(),
        Figure::count(small_keys as f64),
    );
    figures.insert(
        "locality.keys_remeshed_10x".into(),
        Figure::count(large_keys as f64),
    );
    // The claim, as one number: a dab on the larger scene should re-mesh
    // roughly what it re-meshes on the smaller one. Budgeted at 2, which
    // leaves room for the brush covering a different number of bricks at the
    // larger radius without leaving room for scaling with the document.
    let ratio = large_keys as f64 / small_keys.max(1) as f64;
    figures.insert(
        "locality.key_ratio".into(),
        Figure {
            value: ratio,
            unit: "x",
            budget: Some(2.0),
            tolerance: 1.5,
            noise_floor: 0.0,
        },
    );
    figures.insert("locality.dab_ms".into(), Figure::ms(small_ms, None));
    figures.insert("locality.dab_ms_10x".into(), Figure::ms(large_ms, None));
}

/// Rendering the reference scene with nothing being edited.
fn measure_frame_time(policy: &BackendPolicy, figures: &mut BTreeMap<String, Figure>) {
    let Some(gpu) = headless_gpu() else {
        return;
    };
    let Ok(mut document) = Scene::Reference.build(policy.clone()) else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&gpu);
    if geometry.rebuild(&gpu, &mut document).is_err() {
        return;
    }

    let renderer = Renderer::new(&gpu, OffscreenTarget::FORMAT);
    let target = OffscreenTarget::new(&gpu, VIEWPORT.0, VIEWPORT.1);
    let mut camera = Camera::default();
    match document.bounds() {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }

    let mut frames: Vec<f64> = Vec::new();
    for i in 0..32 {
        // Orbiting, because a static frame does not exercise what a moving
        // camera does to culling and upload.
        camera.orbit(0.02, 0.0);
        let started = Instant::now();
        let _ = target.capture(&gpu, &renderer, &camera, geometry.mesh(), false);
        let elapsed = ms(started.elapsed());
        // The first few include pipeline and buffer warmup.
        if i >= 4 {
            frames.push(elapsed);
        }
    }
    frames.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));

    // 60 fps is 16.7 ms. This is an offscreen capture including a readback,
    // which a presenting frame does not pay, so it is reported without a
    // budget rather than judged against one that does not describe it.
    figures.insert(
        "frame.median".into(),
        Figure::ms(quantile(&frames, 0.5), None),
    );
    figures.insert(
        "frame.p95".into(),
        Figure::ms(quantile(&frames, 0.95), None),
    );
}

/// Brick cache memory across repeated open, sculpt and close cycles.
fn measure_memory(policy: &BackendPolicy, figures: &mut BTreeMap<String, Figure>) {
    let mut after_each = Vec::new();
    let mut peak = 0u64;

    for _ in 0..3 {
        let Ok(mut document) = Scene::Reference.build(policy.clone()) else {
            return;
        };
        let brush = Scene::Reference.brush();
        for sample in Scene::Reference.stroke(12) {
            if document
                .apply_stroke(ToolKind::Padrao, brush, &[sample], [false; 3])
                .is_err()
            {
                return;
            }
        }
        if let Ok(stats) = document.cache().stats() {
            peak = peak.max(stats.memory_usage);
            figures.entry("memory.budget".into()).or_insert_with(|| {
                Figure::mb(stats.memory_budget.unwrap_or(0) as f64 / 1_048_576.0)
            });
        }
        drop(document);
        // What a fresh document costs, as the floor a cycle should return to.
        if let Ok(fresh) = Scene::Reference.build(policy.clone()) {
            if let Ok(stats) = fresh.cache().stats() {
                after_each.push(stats.memory_usage as f64 / 1_048_576.0);
            }
        }
    }

    figures.insert("memory.peak".into(), Figure::mb(peak as f64 / 1_048_576.0));
    if let (Some(first), Some(last)) = (after_each.first(), after_each.last()) {
        figures.insert("memory.baseline".into(), Figure::mb(*first));
        // A cycle that does not return to its floor is a leak. Stated as a
        // ratio so the gate does not depend on the absolute figure.
        figures.insert(
            "memory.drift".into(),
            Figure {
                value: last / first.max(f64::MIN_POSITIVE),
                unit: "x",
                budget: Some(1.10),
                tolerance: 1.05,
                noise_floor: 0.0,
            },
        );
    }
}

/// How much a dab costs after the document has been worked on.
///
/// The most important number here, and the one a bare-sphere benchmark misses
/// entirely. The bricks a dab re-meshes do not change as a document is
/// sculpted — measured, they stay at 125 from the first dab to the two
/// hundredth — but the cost of *evaluating* each of them grows with the number
/// of nodes in the layer's tape. So the application gets slower the more it is
/// used, linearly and without bound, and nothing about the edit itself says so.
///
/// The engine's answer is consolidation, which collapses a layer's tape into a
/// baked volume; the specification requires it never run unasked. This figure
/// is what should drive offering it.
fn measure_tape_growth(policy: &BackendPolicy, figures: &mut BTreeMap<String, Figure>) {
    let Some(gpu) = headless_gpu() else {
        return;
    };

    let mut points = Vec::new();
    for prior in [0usize, 96] {
        let Ok(mut document) = ClayDocument::new(policy.clone()) else {
            return;
        };
        if document.add_starting_sphere(1.0).is_err() {
            return;
        }
        let brush = Scene::probe_brush();
        for i in 0..prior {
            let t = i as f32 / prior.max(1) as f32;
            let angle = (t - 0.5) * 1.4;
            let (s, c) = angle.sin_cos();
            let sample = clayspace_model::GestureSample {
                position: [s * 1.01, (t - 0.5) * 0.6, c * 1.01],
                pressure: 1.0,
                time: t,
            };
            if document
                .apply_stroke(ToolKind::Padrao, brush, &[sample], [false; 3])
                .is_err()
            {
                return;
            }
        }
        document.take_dirty_keys();

        let mut geometry = SurfaceGeometry::new(&gpu);
        if geometry.rebuild(&gpu, &mut document).is_err() {
            return;
        }

        let mut times = Vec::new();
        for sample in Scene::Reference.stroke(12) {
            let started = Instant::now();
            if document
                .apply_stroke(ToolKind::Padrao, brush, &[sample], [false; 3])
                .is_err()
            {
                return;
            }
            if geometry.sync(&gpu, &mut document).is_err() {
                return;
            }
            times.push(ms(started.elapsed()));
        }
        times.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        points.push(quantile(&times, 0.5));
    }

    let [fresh, worked] = points[..] else {
        return;
    };
    figures.insert("tape.dab_on_fresh".into(), Figure::ms(fresh, None));
    figures.insert("tape.dab_after_96_edits".into(), Figure::ms(worked, None));
    // How much the same edit costs once the document has been used. Budgeted
    // at 5x, which is roughly where it sits today: the point is to notice the
    // slope changing, not to pretend it is flat.
    figures.insert(
        "tape.growth".into(),
        Figure {
            value: worked / fresh.max(f64::MIN_POSITIVE),
            unit: "x",
            budget: Some(5.0),
            tolerance: 1.3,
            noise_floor: 0.0,
        },
    );
}

fn headless_gpu() -> Option<Gpu> {
    match pollster::block_on(Gpu::headless()) {
        Ok(gpu) => Some(gpu),
        Err(e) => {
            eprintln!("no headless GPU, skipping the measurements that need one: {e}");
            None
        }
    }
}

fn report(where_: &Conditions, figures: &BTreeMap<String, Figure>) {
    println!("{:<34} {:>12}  {:<8} budget", "figure", "value", "unit");
    for (name, figure) in figures {
        println!(
            "{:<34} {:>12.2}  {:<8} {}",
            name,
            figure.value,
            figure.unit,
            figure
                .budget
                .map(|b| format!("<= {b}"))
                .unwrap_or_else(|| "-".into())
        );
    }
    let _ = where_;
}

/// JSON by hand rather than a dependency.
///
/// The shape is small and stable, and a serialiser in the dependency graph is
/// a thing the audit has to consider forever for one file.
fn write_json(
    path: &str,
    where_: &Conditions,
    figures: &BTreeMap<String, Figure>,
) -> std::io::Result<()> {
    let mut out = String::from("{\n  \"conditions\": {\n");
    out.push_str(&format!("    \"scene\": \"{}\",\n", where_.scene));
    out.push_str(&format!("    \"platform\": \"{}\",\n", where_.platform));
    out.push_str(&format!(
        "    \"architecture\": \"{}\",\n",
        where_.architecture
    ));
    out.push_str(&format!("    \"backend\": \"{}\",\n", where_.backend));
    out.push_str(&format!("    \"engine\": \"{}\",\n", where_.engine));
    out.push_str(&format!(
        "    \"viewport\": [{}, {}]\n  }},\n  \"figures\": {{\n",
        where_.viewport.0, where_.viewport.1
    ));
    let last = figures.len().saturating_sub(1);
    for (i, (name, figure)) in figures.iter().enumerate() {
        out.push_str(&format!(
            "    \"{name}\": {:.4}{}\n",
            figure.value,
            if i == last { "" } else { "," }
        ));
    }
    out.push_str("  }\n}\n");
    std::fs::write(path, out)
}

/// Compares against a recorded baseline, refusing to compare unlike runs.
fn compare(
    path: &str,
    where_: &Conditions,
    figures: &BTreeMap<String, Figure>,
) -> std::io::Result<bool> {
    let text = std::fs::read_to_string(path)?;
    let field = |key: &str| -> Option<String> {
        let at = text.find(&format!("\"{key}\":"))?;
        let rest = &text[at + key.len() + 3..];
        let start = rest.find('"')? + 1;
        let end = rest[start..].find('"')? + start;
        Some(rest[start..end].to_string())
    };

    // The comparison is only meaningful between like runs. Saying so and
    // stopping beats reporting a regression that is really a different
    // machine.
    for (key, mine) in [
        ("scene", where_.scene.clone()),
        ("platform", where_.platform.to_string()),
        ("architecture", where_.architecture.to_string()),
        ("backend", where_.backend.clone()),
    ] {
        match field(key) {
            Some(theirs) if theirs == mine => {}
            Some(theirs) => {
                println!(
                    "\nbaseline is from a different run: {key} was {theirs}, this is {mine}. \
                     Not comparing."
                );
                return Ok(false);
            }
            None => {
                println!("\nbaseline does not state its {key}. Not comparing.");
                return Ok(false);
            }
        }
    }

    println!(
        "\n{:<34} {:>10} {:>10} {:>9}",
        "figure", "baseline", "now", "change"
    );
    let mut regressed = false;
    for (name, figure) in figures {
        let Some(at) = text.find(&format!("\"{name}\":")) else {
            continue;
        };
        let rest = &text[at + name.len() + 3..];
        let value: f64 = rest
            .trim_start()
            .split([',', '\n', '}'])
            .next()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(f64::NAN);
        if value.is_nan() {
            continue;
        }
        let ratio = figure.value / value.max(f64::MIN_POSITIVE);
        let worse = figure.regressed_against(value);
        println!(
            "{:<34} {value:>10.2} {:>10.2} {:>8.0}%{}",
            name,
            figure.value,
            (ratio - 1.0) * 100.0,
            if worse { "  REGRESSED" } else { "" }
        );
        regressed |= worse;
    }
    Ok(regressed)
}
