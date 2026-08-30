//! The performance gate: measure, report, compare.
//!
//! ```sh
//! cargo run --release --bin bench                 # measure and print
//! cargo run --release --bin bench -- --json out.json
//! cargo run --release --bin bench -- --baseline benchmarks/baseline-linux-x86_64.json
//! cargo run --release --bin bench -- --only brush.voxel
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

mod compare;
mod figures;
mod groups;
mod json;
mod load;
mod report;
mod run;
mod skip;

use clayspace_app::conditions;
use clayspace_engine::BackendPolicy;

use figures::Figure;
use groups::VIEWPORT;
use run::Run;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };

    let filter = flag("--only");
    let json_path = flag("--json");
    // A baseline recorded from a subset reports every omitted figure as
    // missing on the next comparison, which is now a gate failure — and a
    // confusing one, since nothing regressed.
    if filter.is_some() && json_path.is_some() {
        eprintln!("--only measures a subset; a baseline has to be recorded from a whole run");
        std::process::exit(2);
    }

    let Ok(policy) = BackendPolicy::discover(None) else {
        eprintln!("the engine's backends could not be discovered");
        std::process::exit(2);
    };

    let where_ = conditions(&policy, VIEWPORT);
    println!("measuring: {}\n", where_.describe());

    // Sampled before the warm-up: once this process is sculpting, the load is
    // mostly this process, and says nothing about who else is competing.
    let load = load::Load::sample();
    match &load {
        Some(load) if load.is_quiet() => println!("machine: {}\n", load.describe()),
        Some(load) => println!(
            "machine: {} — busy. Figures below are measured against other work \
             on this box; read a surprise with that in mind.\n",
            load.describe()
        ),
        None => println!("machine: load unavailable on this platform\n"),
    }

    // Before anything is timed, and before the clock on the first group
    // starts: what a cold graphics card costs is larger than any regression
    // this gate is looking for. See `groups::warmup`.
    println!("warming up\n");
    groups::warmup::run(&policy);

    let mut run = Run::new(filter);
    measure_everything(&policy, &mut run);

    report::report(&run);

    if let Some(path) = json_path {
        if let Some(load) = load.filter(|l| l.too_busy_to_record()) {
            if !args.iter().any(|a| a == "--allow-busy") {
                eprintln!(
                    "\nrefusing to record a baseline: {}. A baseline taken against \
                     other work stays wrong for every run that compares to it. Wait \
                     for the machine, or pass --allow-busy if you mean it.",
                    load.describe()
                );
                std::process::exit(2);
            }
            eprintln!("\nrecording a baseline anyway: {}", load.describe());
        }
        match json::write(&path, &where_, load.as_ref(), &run) {
            Ok(()) => println!("\nwritten to {path}"),
            Err(e) => {
                eprintln!("could not write {path}: {e}");
                std::process::exit(2);
            }
        }
    }

    let enforce = args.iter().any(|a| a == "--enforce-budgets");
    let mut failed = report_budgets(&run, enforce);

    if let Some(path) = flag("--baseline") {
        match compare::compare(&path, &where_, load.as_ref(), &run) {
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

/// Every group, in the order the report reads best in: the specification's own
/// five first, then the vocabulary.
///
/// One line per group, and the string is the prefix every figure the group
/// emits is named under — which is what lets `--only` decide whether to run a
/// group before it builds a scene for it.
fn measure_everything(policy: &BackendPolicy, run: &mut Run) {
    run.group("startup", groups::startup::measure);
    run.group("dab", |run| groups::dab::measure(policy, run));
    run.group("locality", |run| groups::locality::measure(policy, run));
    run.group("frame", |run| groups::render::measure(policy, run));
    run.group("render", |run| groups::render::measure_passes(policy, run));
    run.group("msaa", |run| groups::render::measure_msaa(policy, run));
    run.group("memory", |run| groups::memory::measure(policy, run));
    run.group("tape", |run| groups::tape::measure(policy, run));
    run.group("brush", |run| groups::brushes::measure(policy, run));
    run.group("op", |run| groups::operations::measure(policy, run));
    run.group("object", |run| groups::objects::measure(policy, run));
    run.group("subtool", |run| groups::subtool::measure(policy, run));
    run.group("authoring", |run| groups::authoring::measure(policy, run));
    run.group("convert", |run| groups::convert::measure(policy, run));
    run.group("bake", |run| groups::bake::measure(policy, run));
    run.group("mask", |run| groups::mask::measure(policy, run));
    run.group("history", |run| groups::history::measure(policy, run));
}

/// A budget breach is reported always and fails only when asked.
///
/// The specification gates on a *regression* — "a change raises measured dab
/// latency beyond its budget" — and separately says performance is measured in
/// CI rather than asserted there. A gate that is red from the day it is
/// installed, for a reason nobody is about to fix, is a gate people learn to
/// ignore; `--enforce-budgets` is there for when the figure is expected to
/// hold.
fn report_budgets(run: &Run, enforce: bool) -> bool {
    let over: Vec<String> = run
        .figures()
        .iter()
        .filter_map(|(name, figure)| over_budget(name, figure))
        .collect();

    if over.is_empty() {
        return false;
    }
    println!("\nOVER BUDGET");
    for line in &over {
        println!("{line}");
    }
    if !enforce {
        println!("  (reported, not enforced; pass --enforce-budgets to fail on these)");
    }
    enforce
}

fn over_budget(name: &str, figure: &Figure) -> Option<String> {
    let budget = figure.budget?;
    (figure.value > budget).then(|| {
        format!(
            "  {name}: {:.1} {} against a budget of {budget:.1}",
            figure.value, figure.unit
        )
    })
}
