//! What the title bar says, and what happens on save, open and new.
//!
//! Kept apart from the sculpting and scene ViewModels because it owns a
//! different question: not what the document contains, but whether what it
//! contains is safe to lose.

use std::path::{Path, PathBuf};

use clayspace_model::{DocumentModel, ModelError, OpenError};

use crate::command::Command;
use crate::observable::Observable;

/// Whether unsaved work would be lost, and what to do about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    /// Nothing to lose; the caller may proceed.
    Clear,
    /// There are unsaved edits. The caller must ask before proceeding.
    WouldLoseWork,
}

/// The document as a file: where it lives and whether it is behind.
pub struct DocumentViewModel {
    model: Box<dyn DocumentModel>,

    /// Where it was last saved or opened from, if anywhere.
    path: Observable<Option<PathBuf>>,
    /// What to show in the title bar.
    name: Observable<String>,
    /// Whether there are edits the file does not have.
    modified: Observable<bool>,
    /// The most recent failure, for the interface to show.
    notice: Observable<Option<String>>,

    /// What a document with no path is called.
    untitled: &'static str,
}

impl DocumentViewModel {
    pub fn new(model: Box<dyn DocumentModel>, untitled: &'static str) -> Self {
        Self {
            model,
            path: Observable::new(None),
            name: Observable::new(untitled.to_string()),
            modified: Observable::new(false),
            notice: Observable::new(None),
            untitled,
        }
    }

    pub fn path(&self) -> &Observable<Option<PathBuf>> {
        &self.path
    }

    pub fn name(&self) -> &Observable<String> {
        &self.name
    }

    pub fn modified(&self) -> &Observable<bool> {
        &self.modified
    }

    /// The last failure, for the status area. Cleared by the next success.
    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    /// Whether the document has a path to save over without asking.
    pub fn has_path(&self) -> bool {
        self.path.get().is_some()
    }

    /// Whether proceeding would discard work.
    ///
    /// Asked before opening, before starting a new document, and before
    /// quitting. The ViewModel does not decide what to do about it — that is a
    /// question for a person, and the answer arrives as one of the commands
    /// below.
    pub fn guard(&self) -> Guard {
        if *self.modified.get() {
            Guard::WouldLoseWork
        } else {
            Guard::Clear
        }
    }

    /// Records that the document was edited.
    ///
    /// Driven from the composition root rather than inferred here, because
    /// this ViewModel does not see the sculpting commands and inferring it
    /// from the ones it does see would mean guessing.
    pub fn touched(&mut self) {
        self.modified.set_if_changed(true);
    }

    /// Saves over the known path, or reports that there is none.
    pub fn save(&mut self) -> Result<(), ModelError> {
        let Some(path) = self.path.get().clone() else {
            // Not an error: the caller's job is to ask for a path and come
            // back. Saying so plainly beats a failure the interface has to
            // interpret.
            self.notice.set(Some("escolha onde salvar".to_string()));
            return Ok(());
        };
        self.save_as(&path)
    }

    pub fn save_as(&mut self, path: &Path) -> Result<(), ModelError> {
        match self.model.save(path) {
            Ok(()) => {
                self.adopt(path);
                self.modified.set_if_changed(false);
                self.notice.set_if_changed(None);
                Ok(())
            }
            Err(e) => {
                // The path is *not* adopted on failure: a document that failed
                // to save must not look saved, and must not quietly retarget
                // the next save at a file it could not write.
                self.notice
                    .set(Some(format!("não foi possível salvar: {e}")));
                Err(e)
            }
        }
    }

    /// Opens a document, replacing what is loaded.
    ///
    /// The caller is responsible for having asked about unsaved work first —
    /// see [`DocumentViewModel::guard`].
    pub fn open(&mut self, path: &Path) -> Result<(), OpenError> {
        match self.model.open(path) {
            Ok(()) => {
                self.adopt(path);
                self.modified.set_if_changed(false);
                self.notice.set_if_changed(None);
                Ok(())
            }
            Err(e) => {
                self.notice.set(Some(e.to_string()));
                Err(e)
            }
        }
    }

    /// Starts a new document.
    pub fn new_document(&mut self) -> Result<(), ModelError> {
        self.model.reset()?;
        self.path.set(None);
        self.name.set(self.untitled.to_string());
        self.modified.set_if_changed(false);
        self.notice.set_if_changed(None);
        Ok(())
    }

    /// Whether a command is one that makes the document differ from its file.
    ///
    /// Undo counts: a document undone back to what the file holds is still
    /// reported as modified. Tracking that exactly would mean comparing
    /// against the file, and claiming "saved" when it is not is the more
    /// expensive mistake of the two.
    pub fn is_an_edit(command: &Command) -> bool {
        command.touches_document()
    }

    fn adopt(&mut self, path: &Path) {
        self.name.set(
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.untitled.to_string()),
        );
        self.path.set(Some(path.to_path_buf()));
    }
}
