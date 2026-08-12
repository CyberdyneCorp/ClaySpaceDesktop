//! Work that outlasts a frame.
//!
//! Meshing at export resolution, baking, consolidation, import and save can
//! all take longer than 16 ms, and the specification says the interface stays
//! responsive while they run. They go here.
//!
//! Two properties matter and are what the tests check. The interface thread is
//! never blocked, and a result that arrives for a state which has since moved
//! on is **discarded** rather than allowed to overwrite the newer one — a
//! stale export writing itself over a newer document is the failure this
//! exists to prevent.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use crate::observable::Observable;

/// How far along a job is.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    /// What the job is doing, in the user's terms.
    pub label: String,
    /// 0..=1 where the job can say, `None` where it cannot.
    pub fraction: Option<f32>,
}

/// What a job produced.
#[derive(Debug, PartialEq)]
pub enum Completion<T> {
    Finished(T),
    Failed(String),
    /// The document moved on while this ran, so its result was dropped.
    Superseded,
}

/// A completion without its payload, for the status area.
///
/// The payload goes to whoever called `poll`; the interface only needs to know
/// what happened. Keeping them apart means a job can produce something that is
/// not cloneable — a mesh, a file handle — without that constraining the
/// observable state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Finished,
    Failed,
    Superseded,
}

/// A monotonically increasing stamp identifying document state.
///
/// A job records the generation it started against; a result whose generation
/// is behind the current one is stale by definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Generation(u64);

impl Generation {
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

struct Finished<T> {
    generation: Generation,
    outcome: Result<T, String>,
}

/// Runs one job at a time off the interface thread.
///
/// One at a time deliberately: the operations this exists for are expensive,
/// and several at once would compete for the same cores the interface needs.
pub struct JobRunner<T: Send + 'static> {
    progress: Observable<Option<Progress>>,
    /// Bumped whenever the document changes, so a job in flight can be told
    /// its result no longer applies.
    generation: Generation,
    running: Option<Generation>,
    results: Option<Receiver<Finished<T>>>,
    last: Observable<Option<Outcome>>,
    last_error: Observable<Option<String>>,
}

impl<T: Send + 'static> Default for JobRunner<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + 'static> JobRunner<T> {
    pub fn new() -> Self {
        Self {
            progress: Observable::new(None),
            generation: Generation::default(),
            running: None,
            results: None,
            last: Observable::new(None),
            last_error: Observable::new(None),
        }
    }

    /// Progress of the job in flight, if any.
    pub fn progress(&self) -> &Observable<Option<Progress>> {
        &self.progress
    }

    /// What happened to the last job, without its payload.
    pub fn last(&self) -> &Observable<Option<Outcome>> {
        &self.last
    }

    /// Why the last job failed, when it did.
    pub fn last_error(&self) -> &Observable<Option<String>> {
        &self.last_error
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// Records that the document changed, invalidating anything in flight.
    pub fn invalidate(&mut self) {
        self.generation = self.generation.next();
    }

    /// Starts `work` on a worker thread.
    ///
    /// Returns false when a job is already running: the caller decides whether
    /// to wait or to cancel, rather than having a queue grow behind their back.
    pub fn start(
        &mut self,
        label: impl Into<String>,
        work: impl FnOnce(&dyn Reporter) -> Result<T, String> + Send + 'static,
    ) -> bool {
        if self.running.is_some() {
            return false;
        }

        let label = label.into();
        let generation = self.generation;
        let (sender, receiver) = std::sync::mpsc::channel();
        let shared = Arc::new(Mutex::new(Progress {
            label: label.clone(),
            fraction: Some(0.0),
        }));

        self.progress.set(Some(
            shared.lock().expect("progress is not poisoned").clone(),
        ));
        self.running = Some(generation);
        self.results = Some(receiver);

        let reporter = ChannelReporter {
            shared: shared.clone(),
        };
        std::thread::spawn(move || {
            let outcome = work(&reporter);
            // A closed channel means the runner was dropped; nothing to report.
            let _ = sender.send(Finished {
                generation,
                outcome,
            });
        });
        true
    }

    /// Collects a finished job, if one has finished.
    ///
    /// Called once per frame. Never blocks: a job still running simply reports
    /// nothing, which is what keeps the interface thread free.
    pub fn poll(&mut self) -> Option<Completion<T>> {
        let receiver = self.results.as_ref()?;
        let finished = match receiver.try_recv() {
            Ok(finished) => finished,
            Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // The worker died without sending, which is a panic in the
                // job rather than a result.
                self.running = None;
                self.results = None;
                self.progress.set(None);
                let why = "the job stopped unexpectedly".to_string();
                self.last.set(Some(Outcome::Failed));
                self.last_error.set(Some(why.clone()));
                return Some(Completion::Failed(why));
            }
        };

        self.running = None;
        self.results = None;
        self.progress.set(None);

        let completion = if finished.generation != self.generation {
            // The document moved on. Whatever this produced describes a state
            // that no longer exists.
            Completion::Superseded
        } else {
            match finished.outcome {
                Ok(value) => Completion::Finished(value),
                Err(why) => Completion::Failed(why),
            }
        };

        self.last.set(Some(match &completion {
            Completion::Finished(_) => Outcome::Finished,
            Completion::Failed(_) => Outcome::Failed,
            Completion::Superseded => Outcome::Superseded,
        }));
        self.last_error.set(match &completion {
            Completion::Failed(why) => Some(why.clone()),
            _ => None,
        });
        Some(completion)
    }
}

/// How a job reports its progress.
pub trait Reporter: Send + Sync {
    fn report(&self, fraction: f32);
}

struct ChannelReporter {
    shared: Arc<Mutex<Progress>>,
}

impl Reporter for ChannelReporter {
    fn report(&self, fraction: f32) {
        if let Ok(mut progress) = self.shared.lock() {
            progress.fraction = Some(fraction.clamp(0.0, 1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Waits for the runner to produce something, without blocking on it.
    fn drain<T: Send + 'static>(runner: &mut JobRunner<T>) -> Completion<T> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(completion) = runner.poll() {
                return completion;
            }
            assert!(Instant::now() < deadline, "the job never finished");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn a_job_runs_and_reports_its_result() {
        let mut runner = JobRunner::new();
        assert!(runner.start("export", |_| Ok(42)));
        assert!(runner.is_running());
        assert_eq!(drain(&mut runner), Completion::Finished(42));
        assert!(!runner.is_running());
    }

    #[test]
    fn polling_never_blocks() {
        let mut runner = JobRunner::new();
        runner.start("slow", |_| {
            std::thread::sleep(Duration::from_millis(200));
            Ok(1)
        });

        // A frame's worth of polls must all return promptly while the job runs.
        let started = Instant::now();
        for _ in 0..50 {
            let _ = runner.poll();
        }
        assert!(
            started.elapsed() < Duration::from_millis(16),
            "fifty polls took {:?}, so the interface thread is being blocked",
            started.elapsed()
        );
        drain(&mut runner);
    }

    #[test]
    fn a_result_for_a_superseded_document_is_discarded() {
        let mut runner = JobRunner::new();
        runner.start("export", |_| {
            std::thread::sleep(Duration::from_millis(20));
            Ok("geometry")
        });

        // The document changes while the job runs.
        runner.invalidate();

        assert_eq!(
            drain(&mut runner),
            Completion::Superseded,
            "a result describing a state that no longer exists was accepted"
        );
    }

    #[test]
    fn a_result_that_is_still_current_is_kept() {
        let mut runner = JobRunner::new();
        runner.start("export", |_| Ok("geometry"));
        assert_eq!(drain(&mut runner), Completion::Finished("geometry"));
    }

    #[test]
    fn a_failing_job_reports_why() {
        let mut runner = JobRunner::new();
        runner.start("export", |_| {
            Err::<(), _>("the file could not be written".into())
        });
        match drain(&mut runner) {
            Completion::Failed(why) => assert!(why.contains("file")),
            other => panic!("expected a failure, got {other:?}"),
        }
    }

    #[test]
    fn a_panicking_job_does_not_take_the_application_with_it() {
        let mut runner = JobRunner::<()>::new();
        runner.start("bad", |_| panic!("something went wrong in the worker"));
        match drain(&mut runner) {
            Completion::Failed(_) => {}
            other => panic!("expected a failure, got {other:?}"),
        }
        assert!(!runner.is_running(), "the runner stayed busy after a panic");
    }

    #[test]
    fn only_one_job_runs_at_a_time() {
        let mut runner = JobRunner::new();
        assert!(runner.start("first", |_| {
            std::thread::sleep(Duration::from_millis(50));
            Ok(1)
        }));
        assert!(
            !runner.start("second", |_| Ok(2)),
            "a second job was accepted while the first was running"
        );
        drain(&mut runner);
        assert!(runner.start("third", |_| Ok(3)), "the runner stayed busy");
        drain(&mut runner);
    }

    #[test]
    fn progress_is_observable_while_a_job_runs() {
        let mut runner = JobRunner::new();
        runner.start("export", |reporter| {
            for step in 0..=10 {
                reporter.report(step as f32 / 10.0);
                std::thread::sleep(Duration::from_millis(2));
            }
            Ok(())
        });

        assert!(
            runner.progress().get().is_some(),
            "a running job reported no progress for the interface to show"
        );
        drain(&mut runner);
        assert!(
            runner.progress().get().is_none(),
            "progress was left on screen after the job finished"
        );
    }
}
