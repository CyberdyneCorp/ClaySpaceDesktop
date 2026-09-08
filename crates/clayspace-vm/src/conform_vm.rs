//! Re-snapping a retopologised mesh onto a sculpt that has moved.
//!
//! The same three-step sequence as the other three, with one difference that
//! matters: the result is written **into** the active subtool rather than
//! arriving beside it. A conform is a correction to a mesh the sculptor has
//! already accepted, not a new candidate to compare — so it moves vertices and
//! keeps the topology, which is the whole point of reaching for it instead of
//! retopologising again.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clayspace_model::{ConformModel, ConformOutcome, ConformResult, ConformSettings, Conformer};

use crate::command::Command;
use crate::jobs::{Completion, JobRunner};
use crate::observable::Observable;

pub struct ConformViewModel {
    model: Box<dyn ConformModel>,
    engine: Arc<dyn Conformer>,
    settings: Observable<ConformSettings>,
    unavailable: Observable<Option<String>>,
    last: Observable<Option<ConformOutcome>>,
    notice: Observable<Option<String>>,
    jobs: JobRunner<ConformResult>,
    stop: Arc<AtomicBool>,
}

impl ConformViewModel {
    pub fn new(model: Box<dyn ConformModel>, engine: Arc<dyn Conformer>) -> Self {
        Self {
            model,
            engine,
            settings: Observable::new(ConformSettings::default()),
            unavailable: Observable::new(None),
            last: Observable::new(None),
            notice: Observable::new(None),
            jobs: JobRunner::new(),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn settings(&self) -> &Observable<ConformSettings> {
        &self.settings
    }

    pub fn unavailable(&self) -> &Observable<Option<String>> {
        &self.unavailable
    }

    pub fn last(&self) -> &Observable<Option<ConformOutcome>> {
        &self.last
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    pub fn jobs(&self) -> &JobRunner<ConformResult> {
        &self.jobs
    }

    pub fn is_running(&self) -> bool {
        self.jobs.is_running()
    }

    pub fn refresh(&mut self) {
        let reason = self.model.can_conform().err();
        self.unavailable.set_if_changed(reason);
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::SetConformSettings(settings) => {
                self.settings.set_if_changed(settings.sanitized());
            }
            Command::RunConform => self.start(),
            Command::CancelConform => self.stop.store(true, Ordering::Relaxed),
            _ => {}
        }
    }

    fn start(&mut self) {
        if let Some(reason) = self.model.can_conform().err() {
            self.unavailable.set(Some(reason.clone()));
            self.notice.set(Some(reason));
            return;
        }
        if self.jobs.is_running() {
            self.notice
                .set(Some("uma conformação já está em curso".to_string()));
            return;
        }
        let source = match self.model.conform_source() {
            Ok(source) => source,
            Err(e) => {
                self.notice.set(Some(e.to_string()));
                return;
            }
        };

        let settings = *self.settings.get();
        let engine = self.engine.clone();
        self.stop.store(false, Ordering::Relaxed);
        let stop = self.stop.clone();

        self.jobs.start("conformação", move |reporter| {
            engine.run(
                &source,
                settings,
                &|fraction, _stage| reporter.report(fraction),
                &|| stop.load(Ordering::Relaxed),
            )
        });
    }

    pub fn poll(&mut self) {
        match self.jobs.poll() {
            Some(Completion::Finished(result)) => match self.model.apply_conform(&result) {
                Ok(()) => {
                    // The report is kept whether or not anything was flagged.
                    // "It finished" and "it finished, and here is where it
                    // struggled" are different outcomes, and the engine
                    // completes-and-flags precisely so a host can tell them
                    // apart — dropping the figures turns the second back into
                    // the first.
                    self.last.set(Some(result.outcome));
                    self.notice.set(None);
                }
                Err(e) => self.notice.set(Some(e.to_string())),
            },
            Some(Completion::Failed(why)) => self.notice.set(Some(why)),
            Some(Completion::Superseded) | None => {}
        }
    }
}
