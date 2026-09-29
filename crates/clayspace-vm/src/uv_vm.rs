//! A UV layout, off the interface thread.
//!
//! The same three-step sequence retopology uses, and for the same reason: the
//! document is an `Rc<RefCell>` that cannot cross to a worker. Read the source
//! here, unwrap there, record the report here.
//!
//! **What comes back is a report and not a mesh.** The atlas stays in the
//! engine that computed it and this carries up the figures a sculptor judges
//! the layout by: a layout written onto a subtool still being shaped is one the
//! next stroke invalidates. A retopology asked for UVs is where a layout is
//! kept on the layer — see `RetopoSettings::uv`.
//!
//! **A layout kept on a layer is drawn from here too.** Whether the active
//! subtool carries one, and how the viewport shows it — its material, a
//! checker, or a checker tinted by island — are held beside the report, so the
//! panel offers the display only where there is a layout to display.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clayspace_model::{Unwrapper, UvDisplay, UvModel, UvOutcome, UvResult, UvSettings};

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
    display: Observable<UvDisplay>,
    carries_uvs: Observable<bool>,
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
            display: Observable::new(UvDisplay::default()),
            carries_uvs: Observable::new(false),
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

    /// How a layer carrying UVs is drawn, as the sculptor last chose.
    ///
    /// Kept when the active subtool carries no layout, so choosing one that
    /// does shows it the way it was asked for last time.
    pub fn display(&self) -> &Observable<UvDisplay> {
        &self.display
    }

    /// Whether the active subtool carries a UV layout to display.
    pub fn carries_uvs(&self) -> &Observable<bool> {
        &self.carries_uvs
    }

    /// What the viewport should draw: the chosen display where the active
    /// subtool carries a layout, and nothing where it does not.
    pub fn shown_display(&self) -> UvDisplay {
        if *self.carries_uvs.get() {
            *self.display.get()
        } else {
            UvDisplay::Off
        }
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
        let carries = self.model.active_layer_carries_uvs();
        self.carries_uvs.set_if_changed(carries);
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::SetUvSettings(settings) => {
                self.settings.set_if_changed(settings.sanitized());
            }
            Command::RunUvAtlas => self.start(),
            Command::CancelUvAtlas => self.stop.store(true, Ordering::Relaxed),
            Command::SetUvDisplay(display) => {
                self.display.set_if_changed(*display);
            }
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
