//! Retopology, off the interface thread.
//!
//! The heavy work is a `Retopologiser` this ViewModel is handed rather than one
//! it knows about, so it depends on the domain and not on a retopology engine —
//! the layering rule `tools/check_layering.py` enforces. What it owns is the
//! *sequencing*, and the sequencing is the interesting part:
//!
//! 1. read the source out of the document, **on this thread**, because the
//!    document is an `Rc<RefCell>` and cannot cross to a worker;
//! 2. retopologise **on a worker**, reporting progress and asking about
//!    cancellation between stages;
//! 3. place the result **on this thread**, in one undo entry — and discard it
//!    if the document has moved on since, which `JobRunner` already does.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clayspace_model::{RetopoModel, RetopoOutcome, RetopoResult, RetopoSettings, Retopologiser};

use crate::command::Command;
use crate::jobs::{Completion, JobRunner};
use crate::observable::Observable;

/// What a sculptor is looking at while they decide.
pub struct RetopoViewModel {
    model: Box<dyn RetopoModel>,
    engine: Arc<dyn Retopologiser>,
    settings: Observable<RetopoSettings>,
    /// Why the tool is unavailable, where it is.
    unavailable: Observable<Option<String>>,
    /// What the last retopology came to.
    last: Observable<Option<RetopoOutcome>>,
    notice: Observable<Option<String>>,
    jobs: JobRunner<RetopoResult>,
    /// Set from the interface thread and read from the worker.
    ///
    /// An `Arc<AtomicBool>` rather than a channel because the question is
    /// asked between stages and the answer is one bit: a worker that has to
    /// drain a queue to find out whether to stop is a worker that stops late.
    stop: Arc<AtomicBool>,
}

impl RetopoViewModel {
    pub fn new(model: Box<dyn RetopoModel>, engine: Arc<dyn Retopologiser>) -> Self {
        Self {
            model,
            engine,
            settings: Observable::new(RetopoSettings::default()),
            unavailable: Observable::new(None),
            last: Observable::new(None),
            notice: Observable::new(None),
            jobs: JobRunner::new(),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn settings(&self) -> &Observable<RetopoSettings> {
        &self.settings
    }

    pub fn unavailable(&self) -> &Observable<Option<String>> {
        &self.unavailable
    }

    pub fn last(&self) -> &Observable<Option<RetopoOutcome>> {
        &self.last
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    pub fn jobs(&self) -> &JobRunner<RetopoResult> {
        &self.jobs
    }

    pub fn is_running(&self) -> bool {
        self.jobs.is_running()
    }

    /// Re-reads whether the tool is available at all.
    ///
    /// Called when the active subtool changes: retopology rebuilds a mesh's
    /// topology, and a field is not a mesh.
    pub fn refresh(&mut self) {
        let reason = self.model.can_retopologise().err();
        self.unavailable.set_if_changed(reason);
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::SetRetopoSettings(settings) => {
                self.settings.set_if_changed(settings.sanitized());
            }
            Command::RunRetopology => self.start(),
            Command::CancelRetopology => {
                // Read by the worker between stages. The job is not abandoned
                // here: it finishes as cancelled and its result is discarded
                // like any other, so nothing is left half-placed.
                self.stop.store(true, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    fn start(&mut self) {
        if let Some(reason) = self.model.can_retopologise().err() {
            self.unavailable.set(Some(reason.clone()));
            self.notice.set(Some(reason));
            return;
        }
        if self.jobs.is_running() {
            self.notice
                .set(Some("uma retopologia já está em curso".to_string()));
            return;
        }

        // Step one, on this thread: the document cannot go to the worker.
        let source = match self.model.retopo_source() {
            Ok(source) => source,
            Err(e) => {
                self.notice.set(Some(e.to_string()));
                return;
            }
        };

        let settings = *self.settings.get();
        let engine = self.engine.clone();
        // Cleared here rather than on completion. A cancellation asked for
        // before a job exists cannot apply to it, and a flag left set would
        // silently kill the *next* run — which a test caught by asking for a
        // cancellation first and being surprised that the work completed.
        self.stop.store(false, Ordering::Relaxed);
        let stop = self.stop.clone();

        self.jobs.start("retopologia", move |reporter| {
            engine.run(
                &source,
                settings,
                // The engine's stage name is dropped rather than shown: `Reporter`
                // carries a fraction and the job's own label carries the words, and
                // a per-stage label would flicker through "cross field", "seamless
                // solve" and "isolines" faster than anyone can read one.
                &|fraction, _stage| reporter.report(fraction),
                &|| stop.load(Ordering::Relaxed),
            )
        });
    }

    /// Collects a finished retopology and places it. Once per frame.
    pub fn poll(&mut self) {
        match self.jobs.poll() {
            Some(Completion::Finished(result)) => match self.model.place_retopology(&result) {
                Ok(()) => {
                    self.last.set(Some(result.outcome));
                    self.notice.set(None);
                }
                Err(e) => self.notice.set(Some(e.to_string())),
            },
            Some(Completion::Failed(why)) => self.notice.set(Some(why)),
            // The document moved on while this ran, so the result describes a
            // scene that no longer exists. Dropped rather than placed — a
            // retopology of a subtool that has since been deleted would
            // otherwise arrive beside nothing.
            Some(Completion::Superseded) => {}
            None => {}
        }
    }
}
