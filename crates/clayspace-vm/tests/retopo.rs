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
    placed: Vec<RetopoResult>,
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
        })
    }

    fn place_retopology(&mut self, result: &RetopoResult) -> Result<(), ModelError> {
        self.subtool
            .lock()
            .expect("not poisoned")
            .placed
            .push(result.clone());
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
fn a_cancelled_retopology_places_nothing() {
    let (release, hold) = std::sync::mpsc::channel();
    let (mut vm, subtool) = fixture(Double {
        runs: AtomicU32::new(0),
        fail: None,
        watch_cancel: true,
        hold: Some(std::sync::Mutex::new(hold)),
    });
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
