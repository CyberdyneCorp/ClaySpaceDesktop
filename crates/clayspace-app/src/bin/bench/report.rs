//! The table the benchmark prints.

use std::time::Duration;

use crate::run::Run;

/// The conditions are printed by the caller, before anything is measured, so
/// that a run which never finishes still says what it was measuring.
pub fn report(run: &Run) {
    println!(
        "{:<40} {:>12}  {:<8} {:<10} samples",
        "figure", "value", "unit", "budget"
    );
    for (name, figure) in run.figures() {
        println!(
            "{:<40} {:>12.2}  {:<8} {:<10} {}",
            name,
            figure.value,
            figure.unit,
            figure
                .budget
                .map(|b| format!("<= {b}"))
                .unwrap_or_else(|| "-".into()),
            // Printed beside the number it was reduced from, so that a figure
            // taken once is visibly a figure taken once. A blank here is the
            // measurement saying it has one observation and no idea how far
            // that one might have been from the next.
            run.spreads()
                .get(name)
                .map(|spread| spread.describe())
                .unwrap_or_else(|| "-".into())
        );
    }

    skipped(run);
    durations(run);
}

/// What is not in the table, and why.
///
/// Printed even when empty is not worth it, but printed loudly when it is not:
/// a skip that is really a gap is a figure nobody is comparing.
fn skipped(run: &Run) {
    if run.skips().is_empty() {
        return;
    }
    println!("\nSKIPPED");
    for (prefix, why) in run.skips() {
        println!("  {prefix:<38} {}", why.reason());
    }
}

/// What the run cost, per group and in total.
///
/// So that a group which has quietly become the expensive one is visible in
/// the output rather than in a CI job's wall clock.
fn durations(run: &Run) {
    if run.durations().is_empty() {
        return;
    }
    println!("\n{:<40} {:>12}", "group", "seconds");
    let mut total = Duration::ZERO;
    for (title, took) in run.durations() {
        println!("{title:<40} {:>12.2}", took.as_secs_f64());
        total += *took;
    }
    println!("{:<40} {:>12.2}", "total", total.as_secs_f64());
}
