//! Comparing a run against a recorded baseline.
//!
//! Three things can be true of a figure the baseline holds: this run measured
//! it, this run said why it could not, or this run did not produce it and did
//! not say why. The third is the one a performance gate exists to catch — a
//! measurement that quietly stopped running looks exactly like a measurement
//! that did not regress — so it fails the gate.

use clayspace_app::Conditions;

use crate::json::{self, Baseline};
use crate::run::Run;

/// Compares against a recorded baseline, refusing to compare unlike runs.
pub fn compare(path: &str, where_: &Conditions, run: &Run) -> std::io::Result<bool> {
    let baseline = json::read(path)?;
    if let Some(refusal) = unlike(where_, &baseline) {
        println!("\n{refusal} Not comparing.");
        return Ok(false);
    }

    let regressed = table(&baseline, run);
    let missing = missing(&baseline, run);
    Ok(regressed || missing)
}

/// Why these two runs cannot be compared, if they cannot.
///
/// The comparison is only meaningful between like runs. Saying so and stopping
/// beats reporting a regression that is really a different machine.
fn unlike(where_: &Conditions, baseline: &Baseline) -> Option<String> {
    if baseline.scenes.is_empty() {
        return Some("baseline does not state its scenes.".into());
    }
    for (member, revision) in &where_.scenes {
        match baseline.scenes.get(*member) {
            Some(theirs) if theirs == revision => {}
            Some(theirs) => {
                return Some(format!(
                    "baseline was recorded on {member}-{theirs}, this run is on \
                     {member}-{revision}."
                ))
            }
            None => return Some(format!("baseline does not have the scene {member}.")),
        }
    }
    for member in baseline.scenes.keys() {
        if !where_.scenes.contains_key(member.as_str()) {
            return Some(format!(
                "baseline was recorded on a scene this run does not have: {member}."
            ));
        }
    }

    [
        ("platform", where_.platform, baseline.platform.as_str()),
        (
            "architecture",
            where_.architecture,
            baseline.architecture.as_str(),
        ),
        ("backend", &where_.backend, baseline.backend.as_str()),
    ]
    .into_iter()
    .find(|(_, mine, theirs)| mine != theirs)
    .map(|(key, mine, theirs)| {
        format!("baseline is from a different run: {key} was {theirs}, this is {mine}.")
    })
}

/// Every figure this run measured, against what the baseline says.
fn table(baseline: &Baseline, run: &Run) -> bool {
    println!(
        "\n{:<40} {:>10} {:>10} {:>9}",
        "figure", "baseline", "now", "change"
    );
    let mut regressed = false;
    for (name, figure) in run.figures() {
        let Some(&value) = baseline.figures.get(name) else {
            println!("{name:<40} {:>10} {:>10.2}      new", "-", figure.value);
            continue;
        };
        let ratio = figure.value / value.max(f64::MIN_POSITIVE);
        let worse = figure.regressed_against(value);
        println!(
            "{name:<40} {value:>10.2} {:>10.2} {:>8.0}%{}",
            figure.value,
            (ratio - 1.0) * 100.0,
            if worse { "  REGRESSED" } else { "" }
        );
        regressed |= worse;
    }
    regressed
}

/// The figures the baseline has and this run does not.
///
/// Returns whether any of them is unaccounted for.
fn missing(baseline: &Baseline, run: &Run) -> bool {
    if run.is_filtered() {
        println!("\nfiltered run: not checking for figures the baseline has and this run skipped");
        return false;
    }

    let absent: Vec<&String> = baseline
        .figures
        .keys()
        .filter(|name| !run.figures().contains_key(*name))
        .collect();
    if absent.is_empty() {
        return false;
    }

    let mut unaccounted = Vec::new();
    let mut accounted = Vec::new();
    for name in absent {
        match reason_for(run, name) {
            Some(reason) => accounted.push(format!("  {name:<38} {reason}")),
            None => unaccounted.push(format!("  {name}")),
        }
    }

    if !accounted.is_empty() {
        println!("\nNOT MEASURED THIS RUN");
        for line in &accounted {
            println!("{line}");
        }
    }
    if unaccounted.is_empty() {
        return false;
    }
    println!("\nMISSING — in the baseline, not measured, and no reason given");
    for line in &unaccounted {
        println!("{line}");
    }
    println!("  a measurement that stopped running is what this gate is for.");
    true
}

/// The stated reason a figure of this name is not here, if there is one.
fn reason_for(run: &Run, name: &str) -> Option<&'static str> {
    run.skips()
        .iter()
        .find(|(prefix, _)| covers(prefix, name))
        .map(|(_, why)| why.reason())
}

/// Whether a skip recorded under `prefix` accounts for a figure called `name`.
fn covers(prefix: &str, name: &str) -> bool {
    name == prefix || name.starts_with(&format!("{prefix}."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::Figure;
    use crate::skip::Skip;
    use std::collections::BTreeMap;

    fn conditions() -> Conditions {
        Conditions {
            scenes: [("reference", "r1")].into_iter().collect(),
            platform: "linux",
            architecture: "x86_64",
            backend: "cuda".into(),
            engine: "0.39.0".into(),
            viewport: (1280, 800),
        }
    }

    fn baseline(figures: &[(&str, f64)]) -> Baseline {
        Baseline {
            scenes: [("reference".to_string(), "r1".to_string())]
                .into_iter()
                .collect(),
            platform: "linux".into(),
            architecture: "x86_64".into(),
            backend: "cuda".into(),
            engine: "0.39.0".into(),
            figures: figures
                .iter()
                .map(|(name, value)| (name.to_string(), *value))
                .collect(),
            skipped: BTreeMap::new(),
        }
    }

    #[test]
    fn a_regression_fails() {
        let mut run = Run::new(None);
        run.insert("dab.median", Figure::ms(10.0, None));
        assert!(table(&baseline(&[("dab.median", 2.0)]), &run));
    }

    #[test]
    fn holding_steady_does_not() {
        let mut run = Run::new(None);
        run.insert("dab.median", Figure::ms(2.1, None));
        assert!(!table(&baseline(&[("dab.median", 2.0)]), &run));
    }

    #[test]
    fn a_figure_that_stopped_being_measured_fails() {
        let run = Run::new(None);
        assert!(missing(&baseline(&[("dab.median", 2.0)]), &run));
    }

    #[test]
    fn a_figure_this_machine_cannot_measure_does_not() {
        let mut run = Run::new(None);
        run.skip("dab", Skip::NoHeadlessGpu);
        assert!(!missing(&baseline(&[("dab.median", 2.0)]), &run));
    }

    #[test]
    fn a_filtered_run_checks_nothing_for_missing() {
        let run = Run::new(Some("tape".into()));
        assert!(!missing(&baseline(&[("dab.median", 2.0)]), &run));
    }

    #[test]
    fn a_scene_at_another_revision_refuses() {
        let mut theirs = baseline(&[]);
        theirs.scenes.insert("reference".into(), "r2".into());
        let refusal = unlike(&conditions(), &theirs).expect("refused");
        assert!(refusal.contains("reference-r2"), "{refusal}");
    }

    #[test]
    fn a_scene_the_baseline_does_not_have_refuses() {
        let mut theirs = baseline(&[]);
        theirs.scenes.remove("reference");
        theirs.scenes.insert("voxel-reference".into(), "r1".into());
        let refusal = unlike(&conditions(), &theirs).expect("refused");
        assert!(refusal.contains("reference"), "{refusal}");
    }

    #[test]
    fn a_baseline_without_scenes_refuses() {
        let mut theirs = baseline(&[]);
        theirs.scenes.clear();
        let refusal = unlike(&conditions(), &theirs).expect("refused");
        assert!(refusal.contains("does not state its scenes"), "{refusal}");
    }

    #[test]
    fn another_backend_refuses() {
        let mut theirs = baseline(&[]);
        theirs.backend = "cpu".into();
        let refusal = unlike(&conditions(), &theirs).expect("refused");
        assert!(refusal.contains("backend"), "{refusal}");
    }

    #[test]
    fn a_like_run_is_compared() {
        assert_eq!(unlike(&conditions(), &baseline(&[])), None);
    }

    #[test]
    fn a_skip_covers_the_figures_named_under_it() {
        assert!(covers("dab", "dab.median"));
        assert!(covers("memory.baseline", "memory.baseline"));
        assert!(!covers("dab", "dabble.median"));
        assert!(!covers("tape", "dab.median"));
    }
}
