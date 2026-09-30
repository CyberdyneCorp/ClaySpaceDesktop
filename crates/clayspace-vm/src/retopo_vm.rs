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
//! 3. hold the result **on this thread** as a preview — only if the source
//!    still stands at the revision step one read. A sculpt that moved while
//!    the job ran gets nothing rather than a stale mesh.
//! 4. place it, in one undo entry, when the sculptor or an agent **accepts**
//!    it — or drop it, touching nothing, when they **discard** it.
//!
//! **What happens to a held preview.** It is not in the document: it enters no
//! history, sets no modified mark and is not saved, so a save leaves it held
//! and a new, opened or closed document drops it. It stays acceptable only
//! while its source stands at the revision it was made from — any stroke, undo
//! or redo that moves the source drops it with a notice, since what it
//! describes no longer exists. Starting another run drops it as well: the
//! document records one retopology source at a time, and the new run is
//! the sculptor's answer to the old preview.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use std::rc::Rc;

use clayspace_model::{
    DensityDab, FlowGuide, RetopoGuidance, RetopoModel, RetopoOutcome, RetopoResult,
    RetopoSettings, RetopoToolState, RetopoUv, Retopologiser,
};

use crate::command::{Command, RetopoEdit};
use crate::jobs::{Completion, JobRunner};
use crate::observable::Observable;

/// What a sculptor is looking at while they decide.
pub struct RetopoViewModel {
    model: Box<dyn RetopoModel>,
    engine: Arc<dyn Retopologiser>,
    settings: Observable<RetopoSettings>,
    tool: Observable<RetopoToolState>,
    guidance: Observable<RetopoGuidance>,
    guide_draft: Observable<Vec<[f32; 3]>>,
    guidance_undo: Vec<RetopoGuidance>,
    guidance_redo: Vec<RetopoGuidance>,
    gesture_before: Option<RetopoGuidance>,
    /// Why the tool is unavailable, where it is.
    unavailable: Observable<Option<String>>,
    /// What the last retopology came to: the held result's while one is
    /// held, the placed result's once it is accepted, and nothing once a held
    /// result is discarded or dropped.
    last: Observable<Option<RetopoOutcome>>,
    notice: Observable<Option<String>>,
    jobs: JobRunner<RetopoResult>,
    /// The source revision and the settings the job in flight started from.
    ///
    /// The settings are the ones the run was *asked* with: a sculptor who
    /// flips `in_place` while a job runs has changed the next run, not where
    /// this one lands.
    started: Option<(u64, RetopoSettings)>,
    /// A finished result waiting to be accepted or discarded, with the source
    /// revision and the settings it was made from.
    held: Option<Held>,
    /// The held result, for the viewport and the panel to draw. `None` when
    /// nothing is held.
    preview: Observable<Option<Rc<RetopoResult>>>,
    /// Set from the interface thread and read from the worker.
    ///
    /// An `Arc<AtomicBool>` rather than a channel because the question is
    /// asked between stages and the answer is one bit: a worker that has to
    /// drain a queue to find out whether to stop is a worker that stops late.
    stop: Arc<AtomicBool>,
}

impl RetopoViewModel {
    pub fn new(model: Box<dyn RetopoModel>, engine: Arc<dyn Retopologiser>) -> Self {
        let guidance = model.retopo_guidance();
        Self {
            model,
            engine,
            settings: Observable::new(RetopoSettings::default()),
            tool: Observable::new(RetopoToolState::default()),
            guidance: Observable::new(guidance),
            guide_draft: Observable::new(Vec::new()),
            guidance_undo: Vec::new(),
            guidance_redo: Vec::new(),
            gesture_before: None,
            unavailable: Observable::new(None),
            last: Observable::new(None),
            notice: Observable::new(None),
            jobs: JobRunner::new(),
            started: None,
            held: None,
            preview: Observable::new(None),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The finished result held for accept or discard, if there is one.
    ///
    /// Its `revision` moves whenever a preview is held, accepted, discarded
    /// or dropped, which is what a viewport rebuilds its picture on.
    pub fn preview(&self) -> &Observable<Option<Rc<RetopoResult>>> {
        &self.preview
    }

    /// Whether a finished result is waiting to be accepted or discarded.
    pub fn is_holding(&self) -> bool {
        self.held.is_some()
    }

    /// Drops a held preview because the document it was made from has been
    /// replaced — a new document, an opened one, or the same one reverted.
    ///
    /// Said nowhere: the preview was never part of the document, so there is
    /// no work to warn about losing, and the sculptor has just asked for a
    /// different document.
    pub fn forget_document(&mut self) {
        self.release();
    }

    pub fn settings(&self) -> &Observable<RetopoSettings> {
        &self.settings
    }

    pub fn tool(&self) -> &Observable<RetopoToolState> {
        &self.tool
    }

    pub fn guidance(&self) -> &Observable<RetopoGuidance> {
        &self.guidance
    }

    pub fn guide_draft(&self) -> &Observable<Vec<[f32; 3]>> {
        &self.guide_draft
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
        self.guidance.set_if_changed(self.model.retopo_guidance());
        let reason = self.model.can_retopologise().err();
        self.unavailable.set_if_changed(reason);
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::SetRetopoSettings(settings) => {
                self.settings.set_if_changed(settings.sanitized());
            }
            Command::SetRetopoTool(tool) => {
                if self.tool.get().mode != tool.mode {
                    self.guide_draft.set_if_changed(Vec::new());
                }
                self.tool.set_if_changed(tool.sanitized());
            }
            Command::EditRetopo(edit) => self.edit(edit),
            Command::RunRetopology => self.start(),
            Command::AcceptRetopology => self.accept(),
            Command::DiscardRetopology => self.discard(),
            Command::CancelRetopology => {
                // Read by the worker between stages. The job is not abandoned
                // here: it finishes as cancelled and its result is discarded
                // like any other, so nothing is left half-placed.
                self.stop.store(true, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    fn edit(&mut self, edit: &RetopoEdit) {
        match edit {
            RetopoEdit::AddGuidePoint(point) => {
                if !point.iter().all(|value| value.is_finite()) {
                    return;
                }
                let mut draft = self.guide_draft.get().clone();
                if draft.last().is_none_or(|last| last != point) {
                    draft.push(*point);
                    self.guide_draft.set(draft);
                }
            }
            RetopoEdit::FinishGuide => {
                let points = self.guide_draft.get().clone();
                self.guide_draft.set(Vec::new());
                if points.len() < 2 {
                    return;
                }
                let tool = *self.tool.get();
                let mut guidance = self.guidance.get().clone();
                guidance.guides.push(FlowGuide {
                    points,
                    strength: tool.guide_strength,
                    radius: tool.guide_radius,
                    mode: tool.guide_mode,
                    closed: false,
                });
                self.commit_guidance(guidance);
            }
            RetopoEdit::BeginGesture => {
                self.gesture_before = Some(self.guidance.get().clone());
            }
            RetopoEdit::EndGesture => {
                if let Some(before) = self.gesture_before.take() {
                    if before != *self.guidance.get() {
                        self.guidance_undo.push(before);
                        self.guidance_redo.clear();
                    }
                }
            }
            RetopoEdit::MoveGuidePoint {
                guide,
                point,
                position,
            } => {
                if !position.iter().all(|value| value.is_finite()) {
                    return;
                }
                let mut guidance = self.guidance.get().clone();
                let Some(target) = guidance
                    .guides
                    .get_mut(*guide)
                    .and_then(|g| g.points.get_mut(*point))
                else {
                    return;
                };
                *target = *position;
                self.commit_guidance(guidance);
            }
            RetopoEdit::DeleteGuide(index) => {
                let mut guidance = self.guidance.get().clone();
                if *index >= guidance.guides.len() {
                    return;
                }
                guidance.guides.remove(*index);
                self.commit_guidance(guidance);
            }
            RetopoEdit::SetGuide {
                index,
                mode,
                strength,
                radius,
            } => {
                let mut guidance = self.guidance.get().clone();
                let Some(guide) = guidance.guides.get_mut(*index) else {
                    return;
                };
                guide.mode = *mode;
                guide.strength = *strength;
                guide.radius = *radius;
                self.commit_guidance(guidance);
            }
            RetopoEdit::PaintDensity(position) => {
                let tool = *self.tool.get();
                let mut guidance = self.guidance.get().clone();
                guidance.density.push(DensityDab {
                    position: *position,
                    radius: tool.density_radius,
                    multiplier: tool.density_multiplier,
                });
                self.commit_guidance(guidance);
            }
            RetopoEdit::Undo => {
                if let Some(previous) = self.guidance_undo.pop() {
                    self.guidance_redo.push(self.guidance.get().clone());
                    self.install_guidance(previous);
                }
            }
            RetopoEdit::Redo => {
                if let Some(next) = self.guidance_redo.pop() {
                    self.guidance_undo.push(self.guidance.get().clone());
                    self.install_guidance(next);
                }
            }
        }
    }

    fn commit_guidance(&mut self, guidance: RetopoGuidance) {
        if !guidance.valid() || guidance == *self.guidance.get() {
            return;
        }
        if self.gesture_before.is_none() {
            self.guidance_undo.push(self.guidance.get().clone());
            self.guidance_redo.clear();
        }
        self.install_guidance(guidance);
    }

    fn install_guidance(&mut self, guidance: RetopoGuidance) {
        self.model.set_retopo_guidance(guidance.clone());
        self.guidance.set(guidance);
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
        // A new run is the answer to the preview in front of the sculptor, and
        // the document records one retopology source at a time.
        self.release();

        // Step one, on this thread: the document cannot go to the worker.
        let source = match self.model.retopo_source() {
            Ok(source) => source,
            Err(e) => {
                self.notice.set(Some(e.to_string()));
                return;
            }
        };

        let settings = *self.settings.get();
        self.started = Some((source.revision, settings));
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

    /// Collects a finished retopology and holds it as a preview, and drops a
    /// held preview whose source has moved. Once per frame.
    pub fn poll(&mut self) {
        self.drop_if_stale();
        let Some(completion) = self.jobs.poll() else {
            return;
        };
        let started = self.started.take();
        match completion {
            Completion::Finished(result) => self.hold(result, started),
            // A cancelled run arrives here as the engine's refusal, and
            // nothing was placed: the document is exactly as it was.
            Completion::Failed(why) => self.notice.set(Some(why)),
            // The document moved on while this ran, so the result describes a
            // scene that no longer exists. Dropped rather than placed.
            Completion::Superseded => {}
        }
    }

    /// Holds a finished result as a preview — if its source has not moved
    /// since the run started.
    ///
    /// Checked here, before anything is shown, so a stale result is refused
    /// by one rule whatever the model does with it: the source stayed
    /// strokeable the whole time the job ran, and a mesh made from a sculpt
    /// that no longer exists is not a retopology of this one.
    fn hold(&mut self, result: RetopoResult, started: Option<(u64, RetopoSettings)>) {
        let Some((revision, settings)) = started else {
            return;
        };
        if !self.source_stands_at(revision) {
            self.model.discard_retopology();
            self.notice.set(Some(
                "a camada de origem mudou enquanto a retopologia corria; \
                 o resultado foi descartado"
                    .to_string(),
            ));
            return;
        }
        // A layout that was asked for and refused is said out loud: the
        // quads are there, and without it.
        let notice = match &result.outcome.uv {
            RetopoUv::Failed(why) => Some(format!("os quads não têm UVs: {why}")),
            _ => None,
        };
        let notice = result.outcome.guidance_warnings.first().cloned().or(notice);
        self.last.set(Some(result.outcome.clone()));
        self.notice.set(notice);
        self.held = Some(Held { revision, settings });
        self.preview.set(Some(Rc::new(result)));
    }

    /// Places the held result, exactly as it would have been placed when the
    /// job landed: one undo entry, beside the source or over it.
    fn accept(&mut self) {
        let (Some(held), Some(result)) = (self.held.take(), self.preview.get().clone()) else {
            self.notice.set(Some(
                "não há retopologia à espera de ser aceite".to_string(),
            ));
            return;
        };
        self.preview.set(None);
        if !self.source_stands_at(held.revision) {
            self.model.discard_retopology();
            self.last.set(None);
            self.notice.set(Some(STALE_PREVIEW.to_string()));
            return;
        }
        match self.model.place_retopology(&result, held.settings) {
            Ok(()) => self.notice.set(None),
            Err(e) => {
                self.last.set(None);
                self.notice.set(Some(e.to_string()));
            }
        }
    }

    /// Drops the held result. Nothing in the document changes.
    fn discard(&mut self) {
        if self.held.is_none() {
            self.notice.set(Some(
                "não há retopologia à espera de ser descartada".to_string(),
            ));
            return;
        }
        self.release();
        self.notice.set(None);
    }

    /// Drops the held preview whose source has moved or gone, and says so.
    fn drop_if_stale(&mut self) {
        let Some(held) = &self.held else {
            return;
        };
        if self.source_stands_at(held.revision) {
            return;
        }
        self.release();
        self.notice.set(Some(STALE_PREVIEW.to_string()));
    }

    /// Lets go of a held preview, if there is one, of the source the
    /// document recorded for it, and of its outcome.
    ///
    /// The outcome goes too: it was reported while the result was held, and
    /// left behind with nothing held it would read as a placed result, which
    /// is exactly what a discarded one is not.
    fn release(&mut self) {
        if self.held.take().is_some() {
            self.model.discard_retopology();
            self.last.set(None);
        }
        if self.preview.get().is_some() {
            self.preview.set(None);
        }
    }

    fn source_stands_at(&mut self, revision: u64) -> bool {
        self.model.retopo_source_revision().ok() == Some(revision)
    }
}

/// Said when a held preview's source moves before it is accepted.
const STALE_PREVIEW: &str = "a camada de origem mudou; a pré-visualização da \
                             retopologia foi descartada";

/// What a held preview was made from.
#[derive(Debug, Clone, Copy)]
struct Held {
    /// The source's revision when the run read it.
    revision: u64,
    /// The settings the run was asked with — which decide where it lands.
    settings: RetopoSettings,
}
