//! Baking maps from the field, off the interface thread.
//!
//! The same sequence as retopology and UV, with one addition: the `Baker` is
//! made **per run** rather than held, because it carries a snapshot of the
//! document and a snapshot taken once at startup would bake the shape the
//! session opened with.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clayspace_model::{BakeModel, BakeResult, BakeSettings, Baker, UvModel};

use crate::command::Command;
use crate::jobs::{Completion, JobRunner};
use crate::observable::Observable;

/// Makes a baker that carries the document as it is now.
///
/// A closure rather than a stored `Baker`: the snapshot has to be taken on the
/// interface thread at the moment the sculptor asks, and this is the only
/// shape that lets the ViewModel ask for one without knowing what a snapshot
/// is.
pub type Snapshotter = Box<dyn Fn() -> Result<Arc<dyn Baker>, String>>;

pub struct BakeViewModel {
    model: Box<dyn BakeModel>,
    source: Box<dyn UvModel>,
    snapshot: Snapshotter,
    settings: Observable<BakeSettings>,
    unavailable: Observable<Option<String>>,
    last: Observable<Option<BakeResult>>,
    notice: Observable<Option<String>>,
    jobs: JobRunner<BakeResult>,
    stop: Arc<AtomicBool>,
    /// Where the maps go, chosen through the platform's file panel by the
    /// composition root — the ViewModel never opens one.
    into: Option<std::path::PathBuf>,
}

impl BakeViewModel {
    pub fn new(model: Box<dyn BakeModel>, source: Box<dyn UvModel>, snapshot: Snapshotter) -> Self {
        Self {
            model,
            source,
            snapshot,
            settings: Observable::new(BakeSettings::default()),
            unavailable: Observable::new(None),
            last: Observable::new(None),
            notice: Observable::new(None),
            jobs: JobRunner::new(),
            stop: Arc::new(AtomicBool::new(false)),
            into: None,
        }
    }

    pub fn settings(&self) -> &Observable<BakeSettings> {
        &self.settings
    }

    pub fn unavailable(&self) -> &Observable<Option<String>> {
        &self.unavailable
    }

    pub fn last(&self) -> &Observable<Option<BakeResult>> {
        &self.last
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    pub fn jobs(&self) -> &JobRunner<BakeResult> {
        &self.jobs
    }

    pub fn is_running(&self) -> bool {
        self.jobs.is_running()
    }

    pub fn refresh(&mut self) {
        let reason = self.model.can_bake().err();
        self.unavailable.set_if_changed(reason);
    }

    /// Where the maps will go. Set by the composition root from its file panel.
    pub fn bake_into(&mut self, path: std::path::PathBuf) {
        self.into = Some(path);
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::SetBakeSettings(settings) => {
                self.settings.set_if_changed(settings.clone().sanitized());
            }
            Command::RunBake => self.start(),
            Command::CancelBake => self.stop.store(true, Ordering::Relaxed),
            _ => {}
        }
    }

    fn start(&mut self) {
        if let Some(reason) = self.model.can_bake().err() {
            self.unavailable.set(Some(reason.clone()));
            self.notice.set(Some(reason));
            return;
        }
        if self.jobs.is_running() {
            self.notice
                .set(Some("uma cozedura já está em curso".to_string()));
            return;
        }
        let Some(into) = self.into.clone() else {
            self.notice
                .set(Some("escolha onde gravar os mapas".to_string()));
            return;
        };
        if self.settings.get().maps.is_empty() {
            // Refused rather than read as "all of them": an empty set is a
            // sculptor who has not chosen yet, and guessing would write four
            // files nobody asked for.
            self.notice
                .set(Some("nenhum mapa foi escolhido".to_string()));
            return;
        }

        let source = match self.source.uv_source() {
            Ok(source) => source,
            Err(e) => {
                self.notice.set(Some(e.to_string()));
                return;
            }
        };
        // Taken here, on this thread, and at the moment the sculptor asked —
        // so the maps describe the shape they were looking at rather than
        // whatever it becomes while the bake runs.
        let baker = match (self.snapshot)() {
            Ok(baker) => baker,
            Err(e) => {
                self.notice.set(Some(e));
                return;
            }
        };

        let settings = self.settings.get().clone();
        self.stop.store(false, Ordering::Relaxed);
        let stop = self.stop.clone();

        self.jobs.start("cozedura de mapas", move |reporter| {
            baker.run(
                &source,
                &settings,
                &into,
                &|fraction, _stage| reporter.report(fraction),
                &|| stop.load(Ordering::Relaxed),
            )
        });
    }

    pub fn poll(&mut self) {
        match self.jobs.poll() {
            Some(Completion::Finished(result)) => {
                // A partial bake is reported as what it is. Three maps written
                // and one refused is a useful outcome and collapsing it into a
                // failure would throw the three away.
                if result.refused.is_empty() {
                    self.notice.set(None);
                } else {
                    self.notice.set(Some(
                        result
                            .refused
                            .iter()
                            .map(|(map, why)| format!("{}: {why}", map.label()))
                            .collect::<Vec<_>>()
                            .join("; "),
                    ));
                }
                self.last.set(Some(result));
            }
            Some(Completion::Failed(why)) => self.notice.set(Some(why)),
            Some(Completion::Superseded) | None => {}
        }
    }
}
