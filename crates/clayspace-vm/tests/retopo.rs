//! Retopology's sequencing, with no engine present.
//!
//! The ViewModel is handed a `Retopologiser` rather than knowing one, so this
//! exercises the whole sequence — read the source here, work over there, place
//! the result here — against a double. That is the point of the trait: the
//! layering rule forbids this crate from reaching a retopology engine, and the
//! sequencing is what can actually go wrong.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use clayspace_model::{
    ModelError, RetopoModel, RetopoOutcome, RetopoResult, RetopoSettings, RetopoSource,
    Retopologiser,
};
use clayspace_vm::{Command, RetopoViewModel};

/// A document that records what was asked of it.
#[derive(Default)]
struct Subtool {
    available: Option<String>,
    sources_read: u32,
    /// The source's geometry revision. Moved by a test to stand for a stroke
    /// landing on the sculpt while the job runs.
    revision: u64,
    /// What was placed, and whether it was asked to replace the source.
    placed: Vec<RetopoResult>,
    in_place: Vec<bool>,
}

struct Doubles {
    subtool: Arc<std::sync::Mutex<Subtool>>,
}

impl RetopoModel for Doubles {
    fn can_retopologise(&self) -> Result<(), String> {
        match self.subtool.lock().expect("not poisoned").available.clone() {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }

    fn retopologise(&mut self, _settings: RetopoSettings) -> Result<RetopoOutcome, ModelError> {
        unreachable!("the ViewModel uses the three-step path, not this one")
    }

    fn retopo_source(&mut self) -> Result<RetopoSource, ModelError> {
        let mut subtool = self.subtool.lock().expect("not poisoned");
        if let Some(reason) = subtool.available.clone() {
            return Err(ModelError::engine(reason));
        }
        subtool.sources_read += 1;
        Ok(RetopoSource {
            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: Vec::new(),
            indices: vec![0, 1, 2],
            name: "Forma · mesh".to_string(),
            revision: subtool.revision,
        })
    }

    fn retopo_source_revision(&mut self) -> Result<u64, ModelError> {
        Ok(self.subtool.lock().expect("not poisoned").revision)
    }

    fn place_retopology(
        &mut self,
        result: &RetopoResult,
        settings: RetopoSettings,
    ) -> Result<(), ModelError> {
        let mut subtool = self.subtool.lock().expect("not poisoned");
        subtool.placed.push(result.clone());
        subtool.in_place.push(settings.in_place);
        Ok(())
    }
}

/// A retopologiser that counts its runs and can be told to fail or to notice a
/// cancellation.
struct Double {
    runs: AtomicU32,
    fail: Option<String>,
    watch_cancel: bool,
    /// Held by the worker until the test releases it, so a cancellation can be
    /// asked for *while* the run is in flight rather than before it — which is
    /// the only moment a cancellation means anything.
    hold: Option<std::sync::Mutex<std::sync::mpsc::Receiver<()>>>,
}

impl Retopologiser for Double {
    fn run(
        &self,
        source: &RetopoSource,
        settings: RetopoSettings,
        progress: &dyn Fn(f32, &str),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RetopoResult, String> {
        self.runs.fetch_add(1, Ordering::Relaxed);
        progress(0.5, "campo cruzado");
        if let Some(hold) = &self.hold {
            // Blocks until the test has dispatched its cancellation, so the
            // check below is deterministic rather than a race this test has to
            // win.
            let _ = hold.lock().expect("not poisoned").recv();
        }
        if self.watch_cancel && cancelled() {
            return Err("a retopologia foi cancelada".to_string());
        }
        if let Some(why) = &self.fail {
            return Err(why.clone());
        }
        Ok(RetopoResult {
            positions: source.positions.clone(),
            indices: source.indices.clone(),
            // A double, so the authored edges are the triangulation's own —
            // these tests are about the ViewModel's job (dispatch, progress,
            // cancellation, placement) and not about what a quadrangulator
            // returns. The engine test that asserts the edges are *not* the
            // triangulation is `the_edges_are_the_quads_and_not_their_triangulation`.
            edges: Vec::new(),
            outcome: RetopoOutcome {
                triangles_before: 1,
                faces: 1,
                triangles: 2,
                vertices: source.positions.len(),
            },
            name: format!("{} · quads · {}", source.name, settings.target_quads),
        })
    }
}

/// A retopologiser that holds its worker until the test releases it.
fn held(watch_cancel: bool) -> (Double, std::sync::mpsc::Sender<()>) {
    let (release, hold) = std::sync::mpsc::channel();
    (
        Double {
            runs: AtomicU32::new(0),
            fail: None,
            watch_cancel,
            hold: Some(std::sync::Mutex::new(hold)),
        },
        release,
    )
}

fn fixture(engine: Double) -> (RetopoViewModel, Arc<std::sync::Mutex<Subtool>>) {
    let subtool = Arc::new(std::sync::Mutex::new(Subtool::default()));
    let vm = RetopoViewModel::new(
        Box::new(Doubles {
            subtool: subtool.clone(),
        }),
        Arc::new(engine),
    );
    (vm, subtool)
}

/// Waits for the job to land, which is what a frame loop does.
fn settle(vm: &mut RetopoViewModel) {
    for _ in 0..2000 {
        vm.poll();
        if !vm.is_running() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("the retopology never finished");
}

#[test]
fn the_source_is_read_here_and_the_result_is_placed_here() {
    let (mut vm, subtool) = fixture(Double {
        runs: AtomicU32::new(0),
        fail: None,
        watch_cancel: false,
        hold: None,
    });
    vm.dispatch(&Command::SetRetopoSettings(RetopoSettings {
        target_quads: 750,
        ..RetopoSettings::default()
    }));
    vm.dispatch(&Command::RunRetopology);
    settle(&mut vm);

    let subtool = subtool.lock().expect("not poisoned");
    assert_eq!(
        subtool.sources_read, 1,
        "the source was not read on the calling thread exactly once"
    );
    assert_eq!(subtool.placed.len(), 1, "the result was not placed");
    // The settings reached the worker, which is what says the sequence carried
    // more than a default.
    assert!(
        subtool.placed[0].name.ends_with("750"),
        "the worker ran with settings the ViewModel did not send: {}",
        subtool.placed[0].name
    );
    assert_eq!(vm.last().get().map(|o| o.faces), Some(1));
}

/// An unavailable subtool is refused before any work starts.
#[test]
fn an_unavailable_subtool_is_refused_without_running_anything() {
    let (mut vm, subtool) = fixture(Double {
        runs: AtomicU32::new(0),
        fail: None,
        watch_cancel: false,
        hold: None,
    });
    subtool.lock().expect("not poisoned").available = Some("esta camada é Sdf".to_string());
    vm.refresh();
    assert!(vm.unavailable().get().is_some());

    vm.dispatch(&Command::RunRetopology);
    assert!(
        !vm.is_running(),
        "a refused retopology started a job anyway"
    );
    assert_eq!(
        subtool.lock().expect("not poisoned").sources_read,
        0,
        "the source was read for a subtool that cannot be retopologised"
    );
    assert!(vm.notice().get().is_some(), "the refusal said nothing");
}

/// A cancellation reaches the worker mid-run, and nothing is placed.
///
/// **The first version of this test was wrong and the ViewModel was right.**
/// It dispatched `CancelRetopology` *before* `RunRetopology`, to avoid racing
/// the worker, and the run completed and was placed. That is correct: `start`
/// clears the stop flag, because a cancellation asked for before a job exists
/// cannot apply to it and a stale flag would silently kill the next run. So
/// the fixture now holds the worker until the cancellation has actually been
/// dispatched, which is deterministic *and* exercises the real path.
#[test]
fn a_cancelled_run_publishes_nothing() {
    let (engine, release) = held(true);
    let (mut vm, subtool) = fixture(engine);
    vm.dispatch(&Command::RunRetopology);
    assert!(vm.is_running(), "the job did not start");

    vm.dispatch(&Command::CancelRetopology);
    release.send(()).expect("the worker is waiting");
    settle(&mut vm);

    assert!(
        subtool.lock().expect("not poisoned").placed.is_empty(),
        "a cancelled retopology was placed"
    );
    assert!(vm.notice().get().is_some());
    assert!(
        vm.last().get().is_none(),
        "a cancelled run reported an outcome"
    );
}

/// A failure reaches the notice and places nothing.
#[test]
fn a_refused_retopology_reports_and_places_nothing() {
    let (mut vm, subtool) = fixture(Double {
        runs: AtomicU32::new(0),
        fail: Some("o campo cruzado não convergiu".to_string()),
        watch_cancel: false,
        hold: None,
    });
    vm.dispatch(&Command::RunRetopology);
    settle(&mut vm);

    assert!(subtool.lock().expect("not poisoned").placed.is_empty());
    assert_eq!(
        vm.notice().get().as_deref(),
        Some("o campo cruzado não convergiu")
    );
}

/// Two runs at once are refused rather than queued.
#[test]
fn a_second_retopology_is_refused_while_one_runs() {
    let (mut vm, _subtool) = fixture(Double {
        runs: AtomicU32::new(0),
        fail: None,
        watch_cancel: false,
        hold: None,
    });
    vm.dispatch(&Command::RunRetopology);
    // Without polling, so the first is still in flight.
    vm.dispatch(&Command::RunRetopology);
    assert!(
        vm.notice().get().is_some(),
        "a second retopology was accepted while one was running, so a queue \
         is growing behind the sculptor's back"
    );
    settle(&mut vm);
}

/// A run is a job: it is running, its progress is readable while it runs, and
/// it stops being outstanding only once it has landed.
///
/// What `jobs`, `outstanding` and `wait` read. A run that did not show here was
/// a run an agent could not see, and `wait` reported the session quiet while
/// the retopology was still working.
#[test]
fn a_retopo_run_is_a_job() {
    let (engine, release) = held(false);
    let (mut vm, subtool) = fixture(engine);
    vm.dispatch(&Command::RunRetopology);
    assert!(vm.is_running(), "the run did not start a job");

    // The double reports half-way before it blocks; a poll forwards it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        vm.poll();
        let fraction = vm.jobs().progress().get().as_ref().and_then(|p| p.fraction);
        if fraction == Some(0.5) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the run's progress never left the worker: {fraction:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(vm.is_running(), "the job stopped before it was released");
    assert!(subtool.lock().expect("not poisoned").placed.is_empty());

    release.send(()).expect("the worker is waiting");
    settle(&mut vm);
    assert!(
        vm.jobs().progress().get().is_none(),
        "progress outlived the job"
    );
    assert_eq!(subtool.lock().expect("not poisoned").placed.len(), 1);
}

/// A source that moved while the job ran gets nothing.
///
/// The work runs off the interface thread and the sculpt stays strokeable, so
/// a result made from the sculpt as it was must not be published against the
/// sculpt as it is.
#[test]
fn a_stale_result_is_not_published() {
    let (engine, release) = held(false);
    let (mut vm, subtool) = fixture(engine);
    vm.dispatch(&Command::RunRetopology);
    // A stroke lands on the source while the worker is busy.
    subtool.lock().expect("not poisoned").revision += 1;
    release.send(()).expect("the worker is waiting");
    settle(&mut vm);

    assert!(
        subtool.lock().expect("not poisoned").placed.is_empty(),
        "a retopology of a sculpt that has since moved was published"
    );
    assert!(
        vm.last().get().is_none(),
        "a discarded run reported an outcome"
    );
    assert!(
        vm.notice()
            .get()
            .as_deref()
            .is_some_and(|notice| notice.contains("mudou")),
        "the discard was not said: {:?}",
        vm.notice().get()
    );
}

/// Where a result lands is decided by the settings the run was asked with.
///
/// A new layer is the default; flipping `in_place` while a job runs changes the
/// next run, not where this one lands.
#[test]
fn a_run_lands_where_it_was_asked_to() {
    let (engine, release) = held(false);
    let (mut vm, subtool) = fixture(engine);
    vm.dispatch(&Command::RunRetopology);
    vm.dispatch(&Command::SetRetopoSettings(RetopoSettings {
        in_place: true,
        ..RetopoSettings::default()
    }));
    release.send(()).expect("the worker is waiting");
    settle(&mut vm);

    assert_eq!(
        subtool.lock().expect("not poisoned").in_place,
        vec![false],
        "the default run did not land as a new layer"
    );
}
