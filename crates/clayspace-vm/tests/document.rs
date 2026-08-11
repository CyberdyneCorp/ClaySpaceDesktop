//! What the title bar says, and when it is safe to throw work away.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use clayspace_model::{DocumentModel, ModelError, OpenError};
use clayspace_vm::{DocumentViewModel, Guard};

#[derive(Default)]
struct Recorded {
    saved: Vec<PathBuf>,
    opened: Vec<PathBuf>,
    resets: usize,
}

/// A document that records what it was asked to do and can be told to refuse.
struct FakeDocument {
    recorded: Rc<RefCell<Recorded>>,
    save_fails: bool,
    open_fails: Option<OpenError>,
}

impl DocumentModel for FakeDocument {
    fn save(&mut self, path: &Path) -> Result<(), ModelError> {
        if self.save_fails {
            return Err(ModelError::engine("the disk is full"));
        }
        self.recorded.borrow_mut().saved.push(path.to_path_buf());
        Ok(())
    }

    fn open(&mut self, path: &Path) -> Result<(), OpenError> {
        if let Some(failure) = &self.open_fails {
            return Err(failure.clone());
        }
        self.recorded.borrow_mut().opened.push(path.to_path_buf());
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ModelError> {
        self.recorded.borrow_mut().resets += 1;
        Ok(())
    }
}

fn fixture() -> (DocumentViewModel, Rc<RefCell<Recorded>>) {
    let recorded = Rc::new(RefCell::new(Recorded::default()));
    let model = FakeDocument {
        recorded: Rc::clone(&recorded),
        save_fails: false,
        open_fails: None,
    };
    (
        DocumentViewModel::new(Box::new(model), "Sem título"),
        recorded,
    )
}

fn refusing_to_open(failure: OpenError) -> DocumentViewModel {
    let model = FakeDocument {
        recorded: Rc::new(RefCell::new(Recorded::default())),
        save_fails: false,
        open_fails: Some(failure),
    };
    DocumentViewModel::new(Box::new(model), "Sem título")
}

#[test]
fn a_new_document_is_untitled_and_unmodified() {
    let (vm, _) = fixture();
    assert_eq!(vm.name().get(), "Sem título");
    assert!(!*vm.modified().get());
    assert!(!vm.has_path());
    assert_eq!(vm.guard(), Guard::Clear);
}

#[test]
fn editing_makes_it_modified_and_saving_makes_it_clean() {
    let (mut vm, recorded) = fixture();
    vm.touched();
    assert!(*vm.modified().get());
    assert_eq!(vm.guard(), Guard::WouldLoseWork);

    vm.save_as(Path::new("/tmp/bust.clayspace")).expect("save");
    assert!(!*vm.modified().get(), "saving left it looking modified");
    assert_eq!(vm.guard(), Guard::Clear);
    assert_eq!(recorded.borrow().saved.len(), 1);
    assert_eq!(
        vm.name().get(),
        "bust",
        "the title bar shows the file's name"
    );
    assert!(vm.has_path());
}

#[test]
fn saving_over_the_known_path_needs_no_prompt() {
    let (mut vm, recorded) = fixture();
    vm.save_as(Path::new("/tmp/bust.clayspace"))
        .expect("save as");
    vm.touched();
    vm.save().expect("save");

    let recorded = recorded.borrow();
    assert_eq!(recorded.saved.len(), 2);
    assert_eq!(
        recorded.saved[1],
        PathBuf::from("/tmp/bust.clayspace"),
        "the second save went somewhere else"
    );
}

#[test]
fn saving_without_a_path_asks_rather_than_failing() {
    let (mut vm, recorded) = fixture();
    vm.touched();
    vm.save().expect("save with no path is not an error");

    assert!(recorded.borrow().saved.is_empty(), "it saved somewhere");
    assert!(
        vm.notice().get().is_some(),
        "it neither saved nor said anything"
    );
    assert!(
        *vm.modified().get(),
        "it reported clean without having written anything"
    );
}

#[test]
fn a_failed_save_does_not_look_like_a_saved_document() {
    // The dangerous failure: reporting clean, or adopting a path that could
    // not be written, would send the next save to the same bad place and let
    // the user close on work that was never stored.
    let model = FakeDocument {
        recorded: Rc::new(RefCell::new(Recorded::default())),
        save_fails: true,
        open_fails: None,
    };
    let mut vm = DocumentViewModel::new(Box::new(model), "Sem título");
    vm.touched();

    vm.save_as(Path::new("/read-only/bust.clayspace"))
        .expect_err("the save should have failed");

    assert!(
        *vm.modified().get(),
        "a failed save reported the document clean"
    );
    assert_eq!(vm.guard(), Guard::WouldLoseWork);
    assert!(!vm.has_path(), "a failed save adopted the path anyway");
    assert!(vm.notice().get().is_some(), "a failed save said nothing");
}

#[test]
fn opening_adopts_the_file_and_clears_the_flag() {
    let (mut vm, recorded) = fixture();
    vm.touched();
    vm.open(Path::new("/tmp/other.clayspace")).expect("open");

    assert_eq!(recorded.borrow().opened.len(), 1);
    assert_eq!(vm.name().get(), "other");
    assert!(!*vm.modified().get());
    assert_eq!(vm.guard(), Guard::Clear);
}

#[test]
fn a_failed_open_changes_nothing_about_the_document() {
    let mut vm = refusing_to_open(OpenError::NotFound(PathBuf::from("/tmp/gone.clayspace")));
    vm.touched();

    vm.open(Path::new("/tmp/gone.clayspace"))
        .expect_err("it should have refused");

    assert!(
        *vm.modified().get(),
        "a failed open cleared the modified flag, so the work now looks safe"
    );
    assert!(!vm.has_path(), "a failed open adopted the path");
    assert_eq!(vm.name().get(), "Sem título");
    assert!(vm.notice().get().is_some());
}

#[test]
fn a_newer_document_says_so_rather_than_reading_as_damage() {
    let mut vm = refusing_to_open(OpenError::TooNew {
        path: PathBuf::from("/tmp/new.clayspace"),
        detail: "0.31 > 0.27".to_string(),
    });
    vm.open(Path::new("/tmp/new.clayspace"))
        .expect_err("refused");

    let notice = vm.notice().get().clone().expect("a notice");
    assert!(
        notice.contains("recente"),
        "the notice does not say the document is newer: {notice}"
    );
}

#[test]
fn starting_a_new_document_forgets_the_file() {
    let (mut vm, recorded) = fixture();
    vm.save_as(Path::new("/tmp/bust.clayspace")).expect("save");
    vm.touched();

    vm.new_document().expect("new");

    assert_eq!(recorded.borrow().resets, 1);
    assert!(!vm.has_path(), "a new document kept the old file's path");
    assert_eq!(vm.name().get(), "Sem título");
    assert!(!*vm.modified().get());
    assert_eq!(vm.guard(), Guard::Clear);
}

#[test]
fn the_guard_is_what_stands_between_work_and_losing_it() {
    // Every path that discards the document consults this, so it is worth
    // stating on its own: only an unmodified document is safe to replace
    // without asking.
    let (mut vm, _) = fixture();
    assert_eq!(vm.guard(), Guard::Clear);
    vm.touched();
    assert_eq!(vm.guard(), Guard::WouldLoseWork);
    vm.save_as(Path::new("/tmp/bust.clayspace")).expect("save");
    assert_eq!(vm.guard(), Guard::Clear);
    vm.touched();
    assert_eq!(vm.guard(), Guard::WouldLoseWork);
    vm.new_document().expect("new");
    assert_eq!(vm.guard(), Guard::Clear);
}
