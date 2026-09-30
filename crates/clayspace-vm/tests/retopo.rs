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
    ModelError, RetopoGuidance, RetopoModel, RetopoOutcome, RetopoResult, RetopoSettings,
    RetopoSource, RetopoUv, Retopologiser, UvOutcome, UvSettings,
};
use clayspace_vm::{Command, RetopoEdit, RetopoViewModel};

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
    guidance: RetopoGuidance,
    /// How many times a recorded source was forgotten unplaced.
    discarded: u32,
}

struct Doubles {
    subtool: Arc<std::sync::Mutex<Subtool>>,
}

impl RetopoModel for Doubles {
    fn retopo_guidance(&self) -> RetopoGuidance {
        self.subtool.lock().expect("not poisoned").guidance.clone()
    }

    fn set_retopo_guidance(&mut self, guidance: RetopoGuidance) {
        self.subtool.lock().expect("not poisoned").guidance = guidance;
    }

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
            guidance: subtool.guidance.clone(),
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

    fn discard_retopology(&mut self) {
        self.subtool.lock().expect("not poisoned").discarded += 1;
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
    /// Why a layout asked for is refused, when the test wants it refused.
    uv_refusal: Option<String>,
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
        let (uv, uvs) = match (settings.uv, &self.uv_refusal) {
            (None, _) => (RetopoUv::NotRequested, Vec::new()),
            (Some(_), Some(why)) => (RetopoUv::Failed(why.clone()), Vec::new()),
            (Some(_), None) => (
                RetopoUv::Laid(UvOutcome {
                    charts: 1,
                    seam_edges: 0,
                    max_angle_distortion: 0.0,
                    rms_angle_distortion: 0.0,
                    flipped_charts: 0,
                    fallback_charts: 0,
                    dropped_charts: 0,
                    packed_area: 0.5,
                    packed_box_area: 0.5,
                    texel_density: 1.0,
                }),
                vec![[0.0; 2]; source.positions.len()],
            ),
        };
        Ok(RetopoResult {
            positions: source.positions.clone(),
            indices: source.indices.clone(),
            // A double, so the authored edges are the triangulation's own —
            // these tests are about the ViewModel's job (dispatch, progress,
            // cancellation, placement) and not about what a quadrangulator
            // returns. The engine test that asserts the edges are *not* the
            // triangulation is `the_edges_are_the_quads_and_not_their_triangulation`.
            edges: Vec::new(),
            normals: Vec::new(),
            uvs,
            outcome: RetopoOutcome {
                triangles_before: 1,
                faces: 1,
                triangles: 2,
                vertices: source.positions.len(),
                uv,
                guidance_warnings: Vec::new(),
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
            uv_refusal: None,
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

/// Waits for the job to land and accepts the preview it leaves.
fn settle_and_accept(vm: &mut RetopoViewModel) {
    settle(vm);
    assert!(vm.is_holding(), "the landed run left no preview to accept");
    vm.dispatch(&Command::AcceptRetopology);
}

#[test]
fn the_source_is_read_here_and_the_result_is_placed_here() {
    let (mut vm, subtool) = fixture(Double {
        runs: AtomicU32::new(0),
        fail: None,
        watch_cancel: false,
        hold: None,
        uv_refusal: None,
    });
    vm.dispatch(&Command::SetRetopoSettings(RetopoSettings {
        target_quads: 750,
        ..RetopoSettings::default()
    }));
    vm.dispatch(&Command::RunRetopology);
    settle_and_accept(&mut vm);

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
    assert_eq!(vm.last().get().as_ref().map(|o| o.faces), Some(1));
}

/// An unavailable subtool is refused before any work starts.
#[test]
fn an_unavailable_subtool_is_refused_without_running_anything() {
    let (mut vm, subtool) = fixture(Double {
        runs: AtomicU32::new(0),
        fail: None,
        watch_cancel: false,
        hold: None,
        uv_refusal: None,
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
        uv_refusal: None,
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
        uv_refusal: None,
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
    // Landed, and held rather than placed.
    assert!(vm.is_holding());
    assert!(subtool.lock().expect("not poisoned").placed.is_empty());
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
    settle_and_accept(&mut vm);

    assert_eq!(
        subtool.lock().expect("not poisoned").in_place,
        vec![false],
        "the default run did not land as a new layer"
    );
}

fn plain(uv_refusal: Option<&str>) -> Double {
    Double {
        runs: AtomicU32::new(0),
        fail: None,
        watch_cancel: false,
        hold: None,
        uv_refusal: uv_refusal.map(str::to_string),
    }
}

#[test]
fn uv_generation_is_optional() {
    // Off unless asked for: the default run neither asks for nor places UVs.
    let (mut vm, subtool) = fixture(plain(None));
    vm.dispatch(&Command::RunRetopology);
    settle_and_accept(&mut vm);
    {
        let subtool = subtool.lock().expect("not poisoned");
        assert!(subtool.placed[0].uvs.is_empty(), "UVs nobody asked for");
    }
    assert_eq!(
        vm.last().get().as_ref().map(|o| o.uv.clone()),
        Some(RetopoUv::NotRequested)
    );

    // Asked for, the placed result carries them and the report reaches the
    // sculptor.
    vm.dispatch(&Command::SetRetopoSettings(RetopoSettings {
        uv: Some(UvSettings::default()),
        ..RetopoSettings::default()
    }));
    vm.dispatch(&Command::RunRetopology);
    settle_and_accept(&mut vm);
    let subtool = subtool.lock().expect("not poisoned");
    assert_eq!(subtool.placed.len(), 2);
    assert_eq!(
        subtool.placed[1].uvs.len(),
        subtool.placed[1].positions.len()
    );
    assert!(vm.last().get().as_ref().is_some_and(|o| o.uv.carries_uvs()));
    assert_eq!(*vm.notice().get(), None);
}

#[test]
fn a_failed_uv_run_leaves_the_mesh() {
    let (mut vm, subtool) = fixture(plain(Some("o atlas recusou a malha")));
    vm.dispatch(&Command::SetRetopoSettings(RetopoSettings {
        uv: Some(UvSettings::default()),
        ..RetopoSettings::default()
    }));
    vm.dispatch(&Command::RunRetopology);
    settle(&mut vm);
    // The reason is said while the preview is held, before anything is placed.
    let held = vm.notice().get().clone().expect("the failure is reported");
    assert!(held.contains("o atlas recusou a malha"), "{held}");
    vm.dispatch(&Command::AcceptRetopology);

    // The quads are still placed, without UVs.
    let subtool = subtool.lock().expect("not poisoned");
    assert_eq!(
        subtool.placed.len(),
        1,
        "a refused layout cost the retopology"
    );
    assert!(subtool.placed[0].uvs.is_empty());
    // And the reason is stated, never a UV-carrying success.
    let outcome = vm.last().get().clone().expect("an outcome");
    assert!(!outcome.uv.carries_uvs());
}

#[test]
fn guidance_edits_stay_out_of_sculpt_geometry_and_are_undoable() {
    let (mut vm, subtool) = fixture(plain(None));
    let before_revision = subtool.lock().unwrap().revision;
    vm.dispatch(&Command::EditRetopo(RetopoEdit::BeginGesture));
    vm.dispatch(&Command::EditRetopo(RetopoEdit::AddGuidePoint([
        0.0, 0.0, 0.0,
    ])));
    vm.dispatch(&Command::EditRetopo(RetopoEdit::AddGuidePoint([
        1.0, 0.0, 0.0,
    ])));
    vm.dispatch(&Command::EditRetopo(RetopoEdit::FinishGuide));
    vm.dispatch(&Command::EditRetopo(RetopoEdit::EndGesture));
    vm.dispatch(&Command::EditRetopo(RetopoEdit::PaintDensity([
        0.2, 0.0, 0.0,
    ])));
    assert_eq!(vm.guidance().get().guides.len(), 1);
    assert_eq!(vm.guidance().get().density.len(), 1);
    assert_eq!(subtool.lock().unwrap().revision, before_revision);

    vm.dispatch(&Command::EditRetopo(RetopoEdit::Undo));
    assert!(vm.guidance().get().density.is_empty());
    vm.dispatch(&Command::EditRetopo(RetopoEdit::Undo));
    assert!(vm.guidance().get().guides.is_empty());
    vm.dispatch(&Command::EditRetopo(RetopoEdit::Redo));
    assert_eq!(vm.guidance().get().guides.len(), 1);
    assert_eq!(subtool.lock().unwrap().revision, before_revision);
}

// -- the held preview ---------------------------------------------------------

/// A landed run is held, not placed, and accepting it places exactly the
/// result the job returned — the same call, with the same settings, that
/// placing it on landing made.
#[test]
fn a_landed_run_is_held_until_it_is_accepted() {
    let (mut vm, subtool) = fixture(plain(None));
    vm.dispatch(&Command::SetRetopoSettings(RetopoSettings {
        target_quads: 640,
        uv: Some(UvSettings::default()),
        ..RetopoSettings::default()
    }));
    vm.dispatch(&Command::RunRetopology);
    settle(&mut vm);

    let held = vm.preview().get().clone().expect("the result is held");
    assert!(
        subtool.lock().unwrap().placed.is_empty(),
        "placed on landing"
    );
    assert!(
        held.uvs.len() == held.positions.len(),
        "the layout is held too"
    );
    assert!(
        vm.last().get().is_some(),
        "the report waits with the preview"
    );

    vm.dispatch(&Command::AcceptRetopology);
    let subtool = subtool.lock().unwrap();
    assert_eq!(subtool.placed, vec![(*held).clone()]);
    assert_eq!(subtool.in_place, vec![false]);
    assert_eq!(subtool.discarded, 0);
    assert!(vm.preview().get().is_none() && !vm.is_holding());
    assert_eq!(*vm.notice().get(), None);
}

/// Discarding places nothing and forgets the source the document recorded,
/// so the result cannot be placed afterwards.
#[test]
fn a_discarded_preview_places_nothing() {
    let (mut vm, subtool) = fixture(plain(None));
    vm.dispatch(&Command::RunRetopology);
    settle(&mut vm);
    let revision = vm.preview().revision();

    vm.dispatch(&Command::DiscardRetopology);
    assert!(vm.preview().get().is_none() && !vm.is_holding());
    assert!(
        vm.preview().revision() > revision,
        "the viewport was not told"
    );
    assert_eq!(*vm.notice().get(), None);

    // And accepting afterwards finds nothing to place.
    vm.dispatch(&Command::AcceptRetopology);
    let subtool = subtool.lock().unwrap();
    assert!(subtool.placed.is_empty());
    assert_eq!(subtool.discarded, 1);
    assert!(vm.notice().get().is_some(), "an empty accept said nothing");
}

/// Accept and discard with nothing held are refused aloud, so an agent does
/// not read an answer as an action taken.
#[test]
fn accept_or_discard_with_nothing_held_is_refused() {
    let (mut vm, subtool) = fixture(plain(None));
    vm.dispatch(&Command::AcceptRetopology);
    assert!(vm
        .notice()
        .get()
        .as_deref()
        .is_some_and(|n| n.contains("aceite")));
    vm.dispatch(&Command::DiscardRetopology);
    assert!(vm
        .notice()
        .get()
        .as_deref()
        .is_some_and(|n| n.contains("descartada")));
    let subtool = subtool.lock().unwrap();
    assert!(subtool.placed.is_empty());
    assert_eq!(subtool.discarded, 0);
}

/// A stroke, an undo or a redo that moves the source drops the preview on the
/// next frame, with a notice, and there is nothing left to accept.
#[test]
fn a_preview_whose_source_moves_is_dropped() {
    let (mut vm, subtool) = fixture(plain(None));
    vm.dispatch(&Command::RunRetopology);
    settle(&mut vm);
    assert!(vm.is_holding());

    subtool.lock().unwrap().revision += 1;
    vm.poll();
    assert!(!vm.is_holding() && vm.preview().get().is_none());
    assert!(
        vm.notice()
            .get()
            .as_deref()
            .is_some_and(|n| n.contains("mudou")),
        "the drop was not said: {:?}",
        vm.notice().get()
    );
    vm.dispatch(&Command::AcceptRetopology);
    let subtool = subtool.lock().unwrap();
    assert!(subtool.placed.is_empty());
    assert_eq!(subtool.discarded, 1);
}

/// Accepting in the same frame the source moved — before a poll has seen it —
/// is refused by the same check, not placed against a sculpt that moved.
#[test]
fn a_stale_preview_is_not_accepted() {
    let (mut vm, subtool) = fixture(plain(None));
    vm.dispatch(&Command::RunRetopology);
    settle(&mut vm);
    subtool.lock().unwrap().revision += 1;
    vm.dispatch(&Command::AcceptRetopology);
    assert!(subtool.lock().unwrap().placed.is_empty());
    assert!(vm
        .notice()
        .get()
        .as_deref()
        .is_some_and(|n| n.contains("mudou")));
    assert!(!vm.is_holding());
}

/// A new run drops the held preview before it reads its source, and the
/// result it lands is the one held next.
#[test]
fn a_new_run_drops_the_held_preview() {
    let (mut vm, subtool) = fixture(plain(None));
    vm.dispatch(&Command::RunRetopology);
    settle(&mut vm);
    let first = vm.preview().get().clone().expect("held");

    vm.dispatch(&Command::SetRetopoSettings(RetopoSettings {
        target_quads: 900,
        ..RetopoSettings::default()
    }));
    vm.dispatch(&Command::RunRetopology);
    assert!(!vm.is_holding(), "the old preview outlived a new run");
    assert_eq!(subtool.lock().unwrap().discarded, 1);
    settle(&mut vm);
    let second = vm.preview().get().clone().expect("the new result is held");
    assert_ne!(first.name, second.name);
    assert!(second.name.ends_with("900"));
    assert!(subtool.lock().unwrap().placed.is_empty());
}

/// A replaced document drops the preview without a word: it was never part
/// of the document, so there is no work to warn about.
#[test]
fn a_replaced_document_drops_the_preview_silently() {
    let (mut vm, subtool) = fixture(plain(None));
    vm.dispatch(&Command::RunRetopology);
    settle(&mut vm);
    vm.forget_document();
    assert!(!vm.is_holding() && vm.preview().get().is_none());
    assert_eq!(*vm.notice().get(), None);
    assert!(subtool.lock().unwrap().placed.is_empty());
}
