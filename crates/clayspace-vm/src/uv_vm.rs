//! A UV layout, off the interface thread.
//!
//! The same three-step sequence retopology uses, and for the same reason: the
//! document is an `Rc<RefCell>` that cannot cross to a worker. Read the source
//! here, unwrap there, record the report here.
//!
//! **What comes back is a report and not a mesh.** ClayCore's mesh layers carry
//! no UV attribute, so the atlas stays in the engine that computed it and this
//! carries up the figures a sculptor judges the layout by. A layout living in
//! two places is a layout that can disagree with itself.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clayspace_model::{Unwrapper, UvModel, UvOutcome, UvResult, UvSettings};

use crate::command::Command;
use crate::jobs::{Completion, JobRunner};
use crate::observable::Observable;

pub struct UvViewModel {
    model: Box<dyn UvModel>,
    engine: Arc<dyn Unwrapper>,
    settings: Observable<UvSettings>,
    unavailable: Observable<Option<String>>,
    last: Observable<Option<UvOutcome>>,
    notice: Observable<Option<String>>,
    jobs: JobRunner<UvResult>,
    stop: Arc<AtomicBool>,
}

impl UvViewModel {
    pub fn new(model: Box<dyn UvModel>, engine: Arc<dyn Unwrapper>) -> Self {
        Self {
            model,
            engine,
            settings: Observable::new(UvSettings::default()),
            unavailable: Observable::new(None),
            last: Observable::new(None),
            notice: Observable::new(None),
            jobs: JobRunner::new(),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn settings(&self) -> &Observable<UvSettings> {
        &self.settings
    }

    pub fn unavailable(&self) -> &Observable<Option<String>> {
        &self.unavailable
    }

    pub fn last(&self) -> &Observable<Option<UvOutcome>> {
        &self.last
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    pub fn jobs(&self) -> &JobRunner<UvResult> {
        &self.jobs
    }

    pub fn is_running(&self) -> bool {
        self.jobs.is_running()
    }

    pub fn refresh(&mut self) {
        let reason = self.model.can_unwrap().err();
        self.unavailable.set_if_changed(reason);
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::SetUvSettings(settings) => {
                self.settings.set_if_changed(settings.sanitized());
            }
            Command::RunUvAtlas => self.start(),
            Command::CancelUvAtlas => self.stop.store(true, Ordering::Relaxed),
            _ => {}
        }
    }

    fn start(&mut self) {
        if let Some(reason) = self.model.can_unwrap().err() {
            self.unavailable.set(Some(reason.clone()));
            self.notice.set(Some(reason));
            return;
        }
        if self.jobs.is_running() {
            self.notice
                .set(Some("um desdobramento já está em curso".to_string()));
            return;
        }
        let source = match self.model.uv_source() {
            Ok(source) => source,
            Err(e) => {
                self.notice.set(Some(e.to_string()));
                return;
            }
        };

        let settings = *self.settings.get();
        let engine = self.engine.clone();
        // Cleared on start rather than on completion, for the reason the
        // retopology's is: a cancellation asked for before a job exists cannot
        // apply to it, and a flag left set would silently kill the next run.
        self.stop.store(false, Ordering::Relaxed);
        let stop = self.stop.clone();

        self.jobs.start("desdobramento UV", move |reporter| {
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
            Some(Completion::Finished(result)) => match self.model.record_uv(&result) {
                Ok(()) => {
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
