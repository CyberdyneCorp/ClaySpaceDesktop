//! Comparing a run against a recorded baseline.
//!
//! Three things can be true of a figure the baseline holds: this run measured
//! it, this run said why it could not, or this run did not produce it and did
//! not say why. The third is the one a performance gate exists to catch — a
//! measurement that quietly stopped running looks exactly like a measurement
//! that did not regress — so it fails the gate.

use clayspace_app::Conditions;

use crate::json::{self, Baseline};
use crate::load::Load;
use crate::run::Run;
use crate::skip::Skip;

/// Compares against a recorded baseline, refusing to compare unlike runs.
pub fn compare(
    path: &str,
    where_: &Conditions,
    load: Option<&Load>,
    run: &Run,
) -> std::io::Result<bool> {
    let baseline = json::read(path)?;
    if let Some(refusal) = unlike(where_, &baseline) {
        println!("\n{refusal} Not comparing.");
        return Ok(false);
    }

    if let Some(note) = across_engines(where_, &baseline) {
        println!("\n{note}");
    }
    let regressed = table(&baseline, run);
    let missing = missing(&baseline, run);
    // Said after the table rather than before it: a regression on a busy box
    // is still worth reading, it just is not yet worth acting on. Whoever
    // sees red needs this line next to the red, not scrolled off above it.
    if regressed {
        if let Some(note) = noise(load, &baseline) {
            println!("\n{note}");
        }
    }
    Ok(regressed || missing)
}

/// Says out loud when the two runs were taken against different engines.
///
/// `unlike` deliberately does **not** refuse on the engine: a comparison
/// across two pins is the whole point of an upgrade measurement, and refusing
/// it would leave the one question the gate is best placed to answer with no
/// instrument. But an unannounced one is a trap — every percentage in the
/// table below then folds an engine change into whatever was being tested — so
/// it is named above the table rather than left for a reader to notice in the
/// file.
///
/// The revision and not only the version, because two builds can both say
/// 0.78.0 and differ by a commit, and the version alone cannot say which pair
/// a figure came from.
fn across_engines(where_: &Conditions, baseline: &Baseline) -> Option<String> {
    let theirs = match baseline.revision.as_deref() {
        Some(revision) => format!("{} ({revision})", baseline.engine),
        None => format!("{} (revision not recorded)", baseline.engine),
    };
    let mine = format!("{} ({})", where_.engine, where_.revision);
    (theirs != mine).then(|| {
        format!(
            "Note: the baseline was recorded against engine {theirs} and this run \
             is engine {mine}. Every change below is that difference plus whatever \
             else moved."
        )
    })
}

/// Why a regression reported here might be the machine rather than the code.
fn noise(load: Option<&Load>, baseline: &Baseline) -> Option<String> {
    match load {
        Some(load) if !load.is_quiet() => Some(format!(
            "Note: this run was measured at {}. Re-run on a quiet machine \
             before believing the regressions above.",
            load.describe()
        )),
        // The other direction, and the one that is easy to miss: a quiet run
        // can look like a regression because the *baseline* was recorded on a
        // busy box and so is faster than the machine really is.
        _ => match baseline.load_per_core {
            Some(per_core) if per_core >= 0.25 => Some(format!(
                "Note: the baseline was recorded at {per_core:.2} load per core, \
                 which is not a quiet machine. It may be the baseline that is \
                 wrong, not this run."
            )),
            _ => None,
        },
    }
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
            "{name:<40} {value:>10.2} {:>10.2} {:>8.0}%{}{}",
            figure.value,
            (ratio - 1.0) * 100.0,
            if worse { "  REGRESSED" } else { "" },
            inside_the_spread(baseline, name, figure.value)
        );
        regressed |= worse;
    }
    regressed
}

/// Whether this run's value lands inside the range the baseline's own samples
/// covered, where the baseline recorded one.
///
/// Reported and **not** subtracted from the verdict, deliberately. A within-run
/// spread is the smaller half of the noise — the run-to-run variance from a
/// graphics card's clock state is larger, and no single process can sample it —
/// so a range that happens to swallow a change is evidence the change is small,
/// not proof it is nothing. Letting it silence a regression would trade a gate
/// that sometimes cries wolf for one that sometimes says nothing, which is the
/// worse of the two failures.
///
/// What it is for is the other direction: a 10 % move that lands inside a
/// baseline whose twelve samples spanned 40 % was never a measurement anyone
/// could act on, and now the table says so instead of leaving the reader to
/// guess. That is exactly the adjudication ClayCore's own release notes say
/// their gate could not make.
fn inside_the_spread(baseline: &Baseline, name: &str, value: f64) -> &'static str {
    match baseline.spread.get(name) {
        Some(spread) if spread.covers(value) => "  (inside the baseline's own spread)",
        _ => "",
    }
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
    let mut changed = Vec::new();
    for name in absent {
        // A stated reason is not on its own an excuse. `skip.rs` opens by
        // drawing the distinction this turns on — "a measurement that could
        // not run on this machine, which is fine, or a measurement that
        // quietly stopped running, which is the thing a performance gate
        // exists to catch" — and until now the code did not act on it: any
        // reason at all moved a figure into the accounted column. So an engine
        // that started refusing every edit recorded `EditRefused` against
        // every brush, dropped every brush figure, and passed.
        //
        // What separates the two is not which reason it is but whether the
        // baseline gave the same one. A machine without a GPU says so on every
        // run; an engine that broke this week says something new.
        match (reason_for(run, name), reason_recorded(baseline, name)) {
            // The machine's inability is an excuse on its own: it is true
            // whatever the code does, and a baseline recorded on a machine
            // that *could* is the normal case, not a signal.
            (Some(now), _) if now.is_the_machine() => {
                accounted.push(format!("  {name:<38} {}", now.reason()))
            }
            (Some(now), Some(before)) if now.reason() == before => {
                accounted.push(format!("  {name:<38} {}", now.reason()))
            }
            (Some(now), Some(before)) => changed.push(format!(
                "  {name:<38} was {before:?}, now {:?}",
                now.reason()
            )),
            (Some(now), None) => {
                changed.push(format!("  {name:<38} newly skipped: {}", now.reason()))
            }
            (None, _) => unaccounted.push(format!("  {name}")),
        }
    }

    if !accounted.is_empty() {
        println!("\nNOT MEASURED THIS RUN");
        for line in &accounted {
            println!("{line}");
        }
    }
    if !changed.is_empty() {
        println!("\nSTOPPED BEING MEASURED — skipped now, not skipped then");
        for line in &changed {
            println!("{line}");
        }
        println!("  the baseline measured these; a reason that appeared since is");
        println!("  something breaking, not a machine that cannot.");
    }
    if unaccounted.is_empty() {
        return !changed.is_empty();
    }
    println!("\nMISSING — in the baseline, not measured, and no reason given");
    for line in &unaccounted {
        println!("{line}");
    }
    println!("  a measurement that stopped running is what this gate is for.");
    true
}

/// The reason the baseline recorded for a figure, if it recorded one.
///
/// Prefix-matched exactly as `reason_for` matches this run's skips, so that
/// the two sides of the comparison agree on what a group name covers.
fn reason_recorded<'a>(baseline: &'a Baseline, figure: &str) -> Option<&'a str> {
    baseline
        .skipped
        .iter()
        .filter(|(prefix, _)| covers(prefix, figure))
        .map(|(_, reason)| reason.as_str())
        .next()
}

/// The stated reason a figure of this name is not here, if there is one.
fn reason_for(run: &Run, name: &str) -> Option<Skip> {
    run.skips()
        .iter()
        .find(|(prefix, _)| covers(prefix, name))
        .map(|(_, why)| *why)
}

/// Whether a skip recorded under `prefix` accounts for a figure called `name`.
fn covers(prefix: &str, name: &str) -> bool {
    name == prefix || name.starts_with(&format!("{prefix}."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::Figure;
    use std::collections::BTreeMap;

    fn conditions() -> Conditions {
        Conditions {
            scenes: [("reference", "r1")].into_iter().collect(),
            platform: "linux",
            architecture: "x86_64",
            backend: "cuda".into(),
            engine: "0.39.0".into(),
            revision: "v0.39.0-0-gdeadbee".into(),
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
            revision: Some("v0.39.0-0-gdeadbee".into()),
            figures: figures
                .iter()
                .map(|(name, value)| (name.to_string(), *value))
                .collect(),
            spread: BTreeMap::new(),
            skipped: BTreeMap::new(),
            load_per_core: None,
        }
    }

    #[test]
    fn a_busy_run_gets_a_caveat_next_to_its_regressions() {
        let load = Load {
            one_minute: 15.0,
            cores: 24,
        };
        let note = noise(Some(&load), &baseline(&[])).expect("a busy run is caveated");
        assert!(note.contains("quiet machine"), "{note}");
    }

    #[test]
    fn a_quiet_run_against_a_busy_baseline_blames_the_baseline() {
        // The failure mode that reads as a regression and is not one: the
        // baseline was recorded on a loaded box, so it is faster than the
        // machine can really go, and every honest run after it looks slow.
        let mut recorded = baseline(&[]);
        recorded.load_per_core = Some(0.6);
        let quiet = Load {
            one_minute: 1.0,
            cores: 24,
        };
        let note = noise(Some(&quiet), &recorded).expect("a busy baseline is caveated");
        assert!(note.contains("baseline that is"), "{note}");
    }

    #[test]
    fn two_quiet_runs_say_nothing() {
        let mut recorded = baseline(&[]);
        recorded.load_per_core = Some(0.05);
        let quiet = Load {
            one_minute: 1.0,
            cores: 24,
        };
        assert_eq!(noise(Some(&quiet), &recorded), None);
    }

    #[test]
    fn an_engine_that_started_refusing_every_edit_fails_the_gate() {
        // The failure this gate exists for, and the one it used to pass: a
        // change makes `apply_stroke` refuse, every brush group records
        // `EditRefused`, every brush figure vanishes, and a reason was given
        // for each — so the old accounting called them all excused.
        let mut baseline = baseline(&[("brush.sdf.padrao.mean", 10.0)]);
        baseline.skipped = BTreeMap::new();
        let mut run = Run::new(None);
        run.skip("brush.sdf.padrao", Skip::EditRefused);
        assert!(
            missing(&baseline, &run),
            "a figure the baseline measured, skipped now for a reason the \
             baseline never gave, has to fail"
        );
    }

    #[test]
    fn a_machine_that_never_could_does_not_fail_the_gate() {
        // The other half of the same distinction: a runner with no GPU said so
        // when the baseline was recorded and says so now. Nothing broke.
        let mut baseline = baseline(&[("render.frame.mean", 4.0)]);
        baseline.skipped = [("render".to_string(), "no headless GPU".to_string())]
            .into_iter()
            .collect();
        let mut run = Run::new(None);
        run.skip("render", Skip::NoHeadlessGpu);
        assert!(!missing(&baseline, &run));
    }

    #[test]
    fn a_skip_whose_reason_changed_fails_the_gate() {
        // Same figure, still absent, different story. "No GPU" becoming "the
        // engine refused the edit" is the engine breaking on a machine that
        // was always able to run it.
        let mut baseline = baseline(&[("brush.sdf.padrao.mean", 10.0)]);
        baseline.skipped = [(
            "brush.sdf.padrao".to_string(),
            "no headless GPU".to_string(),
        )]
        .into_iter()
        .collect();
        let mut run = Run::new(None);
        run.skip("brush.sdf.padrao", Skip::EditRefused);
        assert!(missing(&baseline, &run));
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

    /// An engine change is not a refusal — the upgrade measurement this gate
    /// is most useful for is exactly a comparison across two pins — but it is
    /// never silent either.
    #[test]
    fn a_comparison_across_engine_pins_is_allowed_and_announced() {
        let mut theirs = baseline(&[]);
        theirs.engine = "0.73.0".into();
        theirs.revision = Some("v0.73.0-0-gc0ffee".into());
        assert_eq!(unlike(&conditions(), &theirs), None, "still comparable");
        let note = across_engines(&conditions(), &theirs).expect("announced");
        assert!(note.contains("0.73.0"), "{note}");
        assert!(note.contains("0.39.0"), "{note}");
    }

    /// Two builds of the same version are two engines, and only the revision
    /// can say so.
    #[test]
    fn the_same_version_from_a_different_commit_is_still_announced() {
        let mut theirs = baseline(&[]);
        theirs.revision = Some("v0.39.0-4-gfeedbee".into());
        let note = across_engines(&conditions(), &theirs).expect("announced");
        assert!(note.contains("gfeedbee"), "{note}");
    }

    #[test]
    fn the_same_engine_says_nothing() {
        assert_eq!(across_engines(&conditions(), &baseline(&[])), None);
    }

    /// A baseline older than the field is not silently treated as a match:
    /// nothing in it says which build it was taken against.
    #[test]
    fn a_baseline_that_records_no_revision_is_announced_rather_than_assumed() {
        let mut theirs = baseline(&[]);
        theirs.revision = None;
        let note = across_engines(&conditions(), &theirs).expect("announced");
        assert!(note.contains("revision not recorded"), "{note}");
    }

    /// The adjudication the release notes say their own gate could not make.
    /// It annotates and does not excuse: the verdict is the tolerance's, and a
    /// within-run range is only the smaller half of the noise.
    #[test]
    fn a_change_inside_the_baselines_own_range_is_marked_as_such() {
        let mut theirs = baseline(&[("brush.mesh.camada.mean", 19.0)]);
        theirs.spread.insert(
            "brush.mesh.camada.mean".into(),
            crate::figures::Spread {
                n: 12,
                min: 17.0,
                median: 19.0,
                p95: 22.5,
                max: 23.4,
            },
        );
        assert!(!inside_the_spread(&theirs, "brush.mesh.camada.mean", 21.0).is_empty());
        assert!(inside_the_spread(&theirs, "brush.mesh.camada.mean", 30.0).is_empty());
        // A figure the baseline said nothing about is annotated with nothing.
        assert!(inside_the_spread(&theirs, "dab.median", 21.0).is_empty());
    }

    #[test]
    fn a_skip_covers_the_figures_named_under_it() {
        assert!(covers("dab", "dab.median"));
        assert!(covers("memory.baseline", "memory.baseline"));
        assert!(!covers("dab", "dabble.median"));
        assert!(!covers("tape", "dab.median"));
    }
}
