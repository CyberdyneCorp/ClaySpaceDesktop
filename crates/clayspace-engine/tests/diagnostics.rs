//! The report a bug filed from this application should carry.

use claycore::Backend;
use clayspace_engine::{BackendPolicy, Operation, SelectionReason};

/// A policy whose active backend does not depend on the host.
///
/// `from_available` ranks by *this platform's* preference order, and Metal is
/// not in Linux's — so a fixture built from `[Cpu, Metal]` picks Metal on
/// macOS and Cpu on Linux, and any assertion about the active backend is a
/// statement about the runner. An explicit override removes the question.
fn policy() -> BackendPolicy {
    BackendPolicy::from_available(vec![Backend::Cpu, Backend::Metal], Some(Backend::Metal))
}

#[test]
fn the_report_identifies_the_build_and_the_machine() {
    let report = policy().diagnostics();
    assert!(report.app_version.contains(env!("CARGO_PKG_VERSION")));
    assert!(report.engine_version.contains("claycore"));
    assert!(!report.engine_revision.is_empty());
    assert!(report.platform.contains(std::env::consts::OS));
    assert_eq!(report.backends, vec!["cpu", "metal"]);
    assert_eq!(report.active_backend, "metal");
    assert_eq!(policy().reason(), SelectionReason::Override);
}

#[test]
fn the_engine_revision_is_a_real_one_where_there_is_a_checkout() {
    // The failure this catches is the one that already happened: issues filed
    // against 0.26.0 while upstream was on 0.27.3, because nothing in the
    // build said which commit was linked.
    let revision = policy().diagnostics().engine_revision;
    assert!(
        revision.starts_with("unknown") || revision.chars().any(|c| c.is_ascii_hexdigit()),
        "the revision is neither a hash nor an honest 'unknown': {revision}"
    );
}

#[test]
fn a_manual_choice_reads_differently_from_an_automatic_one() {
    // Built without an override, so this one *is* the platform's ranking —
    // which is the point of the comparison, and why it does not assert which
    // backend won.
    let automatic = BackendPolicy::from_available(vec![Backend::Cpu, Backend::Metal], None);
    let mut chosen = automatic.clone();
    chosen.set_override(Backend::Cpu).expect("cpu is available");

    assert_eq!(chosen.diagnostics().active_backend, "cpu");
    assert_eq!(chosen.reason(), SelectionReason::Override);
    assert_ne!(
        chosen.diagnostics().selection,
        automatic.diagnostics().selection
    );
}

#[test]
fn an_override_this_machine_cannot_honour_says_so_in_the_report() {
    let policy = BackendPolicy::from_available(vec![Backend::Cpu], Some(Backend::Cuda));
    let report = policy.diagnostics();
    assert_eq!(report.active_backend, "cpu");
    assert_eq!(policy.reason(), SelectionReason::OverrideUnavailable);
    assert!(
        report.selection.contains("indisponível"),
        "a silently ignored override is the hardest kind to diagnose: {}",
        report.selection
    );
}

#[test]
fn a_fallback_reaches_the_report() {
    let mut policy = policy();
    // A backend that declines an operation routes to the CPU for that one
    // operation and records it once.
    let _ = policy.route(Operation::Raycast, |backend| {
        if *backend == Backend::Cpu {
            Ok(())
        } else {
            Err(claycore::ClayError::for_testing(
                claycore::ErrorKind::Unsupported,
                "clay_raycast",
            ))
        }
    });

    let report = policy.diagnostics();
    assert_eq!(report.fallbacks.len(), 1);
    assert_eq!(report.fallbacks[0].operation, "raycast");
    assert_eq!(report.fallbacks[0].declined_by, "metal");
    assert!(report.to_report().contains("metal declined raycast"));
}

#[test]
fn the_report_is_pasteable() {
    let text = policy().diagnostics().to_report();
    assert!(text.lines().count() >= 6, "too thin to be worth pasting");
    for line in text.lines() {
        assert!(line.contains(": "), "unparseable line: {line}");
    }
}
