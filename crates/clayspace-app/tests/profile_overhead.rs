//! What the measuring itself costs.
//!
//! Not a gate — `sculpt_latency` owns the budget. This answers the question a
//! profiler always has to answer about itself: whether the instrument is heavy
//! enough to be part of what it is measuring.
//!
//! ```sh
//! cargo test -p clayspace-app --test profile_overhead --release -- --nocapture
//! ```

use std::time::{Duration, Instant};

use clayspace_model::{Phase, StrokeDiagnostics, StrokeProfile, Work, RETAINED};

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// A session's worth of samples: three tools, every phase, windows full.
fn worked(tools: usize) -> StrokeProfile {
    let mut profile = StrokeProfile::default();
    for tool in 0..tools {
        let name = format!("tool {tool}");
        for step in 0..RETAINED {
            for phase in Phase::ALL {
                profile.record(
                    &name,
                    phase,
                    Duration::from_micros(500 + (step as u64 % 97)),
                    Work::meshed(27, 9_000),
                );
            }
        }
    }
    profile
}

#[test]
fn recording_a_phase_is_free_beside_the_work_it_measures() {
    let mut profile = StrokeProfile::default();
    // Warm the windows, so this measures the steady state rather than the
    // growth of a `Vec`.
    for _ in 0..RETAINED {
        profile.record(
            "Padrão",
            Phase::EngineEdit,
            Duration::from_micros(1),
            Work::NONE,
        );
    }

    let runs = 100_000;
    let started = Instant::now();
    for _ in 0..runs {
        profile.record(
            "Padrão",
            Phase::EngineEdit,
            Duration::from_micros(1),
            Work::bricks(27),
        );
    }
    let each = started.elapsed() / runs;
    println!("record_phase: {:.3} µs each", each.as_secs_f64() * 1e6);

    // A dab records five of these. The dab itself is milliseconds; five of
    // these must not be a term in that at all.
    assert!(
        each < Duration::from_micros(5),
        "recording a phase costs {:.3} µs, which is no longer free beside a dab",
        each.as_secs_f64() * 1e6
    );
}

/// The one that matters. `Diagnostics` is rebuilt **every frame** by the
/// composition root — deliberately, because a cached report goes stale exactly
/// when a fallback happens. Anything expensive folded into it is paid sixty
/// times a second whether or not a person has the window open.
#[test]
fn reading_the_profile_for_a_report_is_what_costs() {
    for tools in [1, 3] {
        let profile = worked(tools);

        let runs = 20;

        // What the composition root used to do: clone the profile out from
        // behind its cell, then summarise it, every frame.
        let started = Instant::now();
        for _ in 0..runs {
            let taken = profile.clone();
            std::hint::black_box(StrokeDiagnostics::of(&taken));
        }
        let cloned = started.elapsed() / runs;

        // What it does now, where something is going to read the section: the
        // profile is borrowed in place and each window is sorted once.
        let started = Instant::now();
        for _ in 0..runs {
            std::hint::black_box(StrokeDiagnostics::of(&profile));
        }
        let borrowed = started.elapsed() / runs;

        println!(
            "{tools} tool(s), windows full: {:.3} ms cloned, {:.3} ms borrowed",
            ms(cloned),
            ms(borrowed)
        );
    }
}

/// And what it costs when the session has barely started, which is the case a
/// cheap-looking measurement is usually taken in.
#[test]
fn a_barely_worked_session_hides_the_cost() {
    let mut profile = StrokeProfile::default();
    for _ in 0..24 {
        for phase in Phase::ALL {
            profile.record("Padrão", phase, Duration::from_micros(600), Work::NONE);
        }
    }
    let runs = 200;
    let started = Instant::now();
    for _ in 0..runs {
        let taken = profile.clone();
        std::hint::black_box(StrokeDiagnostics::of(&taken));
    }
    println!(
        "24 dabs, one tool: {:.4} ms per frame to summarise",
        ms(started.elapsed() / runs)
    );
}

/// And what it holds. The only cost of recording that is not time.
///
/// Bounded by construction — the window is a ring — but bounded is not free,
/// and the ceiling is worth knowing before anyone proposes switching the
/// recording off to save it.
#[test]
fn what_the_windows_hold_is_bounded_and_small() {
    let each = std::mem::size_of::<Duration>();
    let per_phase = RETAINED * each;
    let per_tool = per_phase * Phase::ALL.len();
    // Every tool the shelf offers, all of them worked until their windows
    // fill, which is the worst a session can reach.
    let tools = 21;
    println!(
        "a full window is {} KiB; a tool with all five is {} KiB; \
         all {tools} tools is {:.1} MiB",
        per_phase / 1024,
        per_tool / 1024,
        (per_tool * tools) as f64 / (1024.0 * 1024.0)
    );
    assert!(
        per_tool * tools < 16 * 1024 * 1024,
        "the ceiling has grown past what a diagnostic may quietly hold"
    );
}
