//! Autosave and recovery, against a real document.
//!
//! The unit tests cover the rules and the disk separately. This is the path a
//! crash actually takes: work is made, an autosave is written, the session
//! never closes, and the next run gets the work back.

use std::path::PathBuf;

use clayspace_app::{SessionStore, SharedDocument};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, Recovery, SculptModel, ToolKind};
use clayspace_vm::DocumentViewModel;

/// A store in a directory of its own, removed when the test ends.
struct Scratch(SessionStore);

impl Scratch {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("clayspace-recovery-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        Self(SessionStore::at(root))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.root());
    }
}

fn fresh() -> Option<SharedDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    Some(SharedDocument::new(document))
}

/// One dab, so the document differs from anything on disk.
fn sculpt(document: &mut SharedDocument) {
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: [0.0, 0.0, 0.55],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("a dab");
}

#[test]
fn work_lost_to_a_crash_comes_back() {
    let Some(mut document) = fresh() else {
        return;
    };
    let scratch = Scratch::new("roundtrip");
    let store = &scratch.0;
    store.begin_session();

    let mut vm = DocumentViewModel::new(Box::new(document.clone()), "Sem título");
    sculpt(&mut document);
    vm.touched();
    let raised = document
        .pick([0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("the dab raised the surface");

    vm.autosave_to(&store.autosave_path()).expect("autosave");

    // And now the process dies: no end_session.
    assert_eq!(
        store.recovery(),
        Recovery::Available(store.autosave_path()),
        "the work was not offered back"
    );

    // The next run.
    let Some(next) = fresh() else {
        return;
    };
    let mut next_vm = DocumentViewModel::new(Box::new(next.clone()), "Sem título");
    let path = store.recovery().path().map(PathBuf::from).expect("a path");
    next_vm.recover(&path, "Recuperado").expect("recover");

    let recovered = next
        .pick([0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("a surface");
    assert!(
        (recovered[2] - raised[2]).abs() < 1e-3,
        "the recovered surface is not the one that was lost: {} against {}",
        recovered[2],
        raised[2]
    );

    // Recovered work is unsaved work: it must not look like a saved document,
    // and ⌘S must ask rather than overwrite the recovery file.
    assert!(*next_vm.modified().get());
    assert!(!next_vm.has_path());
}

#[test]
fn a_clean_exit_leaves_the_next_run_silent() {
    let Some(mut document) = fresh() else {
        return;
    };
    let scratch = Scratch::new("clean-exit");
    let store = &scratch.0;
    store.begin_session();

    let mut vm = DocumentViewModel::new(Box::new(document.clone()), "Sem título");
    sculpt(&mut document);
    vm.touched();
    vm.autosave_to(&store.autosave_path()).expect("autosave");

    store.end_session();
    assert_eq!(store.recovery(), Recovery::Nothing);
    assert!(
        !store.autosave_path().exists(),
        "an ordinary quit left a recovery file behind"
    );
}

#[test]
fn an_autosave_is_only_written_where_there_is_work_to_lose() {
    // The policy says when; this checks the two ends of it against a real
    // document, because "modified" is the flag the whole feature turns on.
    let Some(mut document) = fresh() else {
        return;
    };
    let scratch = Scratch::new("policy");
    let store = &scratch.0;
    // The store makes its own directory when a session opens; this test saves
    // a document into it directly, so it needs that to have happened.
    store.begin_session();
    let policy = clayspace_model::AutosavePolicy::default();

    let mut vm = DocumentViewModel::new(Box::new(document.clone()), "Sem título");
    assert!(
        !policy.is_due(policy.every, *vm.modified().get()),
        "a fresh document was autosaved"
    );

    sculpt(&mut document);
    vm.touched();
    assert!(policy.is_due(policy.every, *vm.modified().get()));

    vm.save_as(&store.root().join("meu.clayspace"))
        .expect("save");
    assert!(
        !policy.is_due(policy.every, *vm.modified().get()),
        "a saved document was still being autosaved"
    );
}
