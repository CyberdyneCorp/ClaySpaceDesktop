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
//! 3. publish the result **on this thread**, in one undo entry — and only if
//!    the source still stands at the revision step one read. A sculpt that
//!    moved while the job ran gets nothing rather than a stale mesh.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    /// What the last retopology came to.
    last: Observable<Option<RetopoOutcome>>,
    notice: Observable<Option<String>>,
    jobs: JobRunner<RetopoResult>,
    /// The source revision and the settings the job in flight started from.
    ///
    /// The settings are the ones the run was *asked* with: a sculptor who
    /// flips `in_place` while a job runs has changed the next run, not where
    /// this one lands.
    started: Option<(u64, RetopoSettings)>,
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
            stop: Arc::new(AtomicBool::new(false)),
        }
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

    /// Collects a finished retopology and publishes it. Once per frame.
    pub fn poll(&mut self) {
        let Some(completion) = self.jobs.poll() else {
            return;
        };
        let started = self.started.take();
        match completion {
            Completion::Finished(result) => self.publish(result, started),
            // A cancelled run arrives here as the engine's refusal, and
            // nothing was placed: the document is exactly as it was.
            Completion::Failed(why) => self.notice.set(Some(why)),
            // The document moved on while this ran, so the result describes a
            // scene that no longer exists. Dropped rather than placed.
            Completion::Superseded => {}
        }
    }

    /// Places a finished result — if its source has not moved since the run
    /// started.
    ///
    /// Checked here, before the document is asked to change, so a stale
    /// result is refused by one rule whatever the model does with it: the
    /// source stayed strokeable the whole time the job ran, and a mesh made
    /// from a sculpt that no longer exists is not a retopology of this one.
    fn publish(&mut self, result: RetopoResult, started: Option<(u64, RetopoSettings)>) {
        let Some((revision, settings)) = started else {
            return;
        };
        let current = self.model.retopo_source_revision();
        if current.ok() != Some(revision) {
            self.notice.set(Some(
                "a camada de origem mudou enquanto a retopologia corria; \
                 o resultado foi descartado"
                    .to_string(),
            ));
            return;
        }
        match self.model.place_retopology(&result, settings) {
            Ok(()) => {
                // A layout that was asked for and refused is said out loud:
                // the quads were placed, and without it.
                let notice = match &result.outcome.uv {
                    RetopoUv::Failed(why) => {
                        Some(format!("os quads foram colocados sem UVs: {why}"))
                    }
                    _ => None,
                };
                let notice = result.outcome.guidance_warnings.first().cloned().or(notice);
                self.last.set(Some(result.outcome));
                self.notice.set(notice);
            }
            Err(e) => self.notice.set(Some(e.to_string())),
        }
    }
}
