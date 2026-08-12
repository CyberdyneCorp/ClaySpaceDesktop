//! Opening, saving and starting over.
//!
//! Separate from [`crate::sculpt::SculptModel`] and [`crate::scene::SceneModel`]
//! for the reason those two are separate from each other: a brush panel has no
//! business being able to replace the document, and a test double for one need
//! not know how to write a file.

use std::path::{Path, PathBuf};

use crate::sculpt::ModelError;

/// Why a document could not be opened.
///
/// Distinguished from [`ModelError`] because the interface answers these
/// differently: a missing file is a mistake to correct, a newer format is a
/// wall, and a corrupt file is neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// Nothing at that path.
    NotFound(PathBuf),
    /// Written by a newer engine than this build carries.
    ///
    /// Named on its own because it is the one failure a user can act on
    /// without help: the document is fine, this application is behind. Telling
    /// them "could not be read" instead would send them looking for damage
    /// that is not there.
    TooNew {
        path: PathBuf,
        /// What the engine said, which carries the versions when it knows them.
        detail: String,
    },
    /// The engine refused it for any other reason.
    Unreadable { path: PathBuf, detail: String },
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "{} não existe", path.display()),
            Self::TooNew { detail, .. } => write!(
                f,
                "documento gravado por uma versão mais recente do motor: {detail}"
            ),
            Self::Unreadable { detail, .. } => write!(f, "documento ilegível: {detail}"),
        }
    }
}

impl std::error::Error for OpenError {}

/// The document as a thing on disk.
pub trait DocumentModel {
    /// Writes the document to `path`.
    fn save(&mut self, path: &Path) -> Result<(), ModelError>;

    /// Replaces everything with the document at `path`.
    ///
    /// All or nothing: a failed open leaves what was there. A sculptor who
    /// mistypes a filename must not lose their work to the attempt.
    fn open(&mut self, path: &Path) -> Result<(), OpenError>;

    /// Replaces everything with a fresh document and its starting form.
    fn reset(&mut self) -> Result<(), ModelError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_open_failure_says_something_a_user_can_act_on() {
        let cases = [
            OpenError::NotFound(PathBuf::from("/tmp/gone.clayspace")),
            OpenError::TooNew {
                path: PathBuf::from("/tmp/new.clayspace"),
                detail: "0.31 > 0.27".to_string(),
            },
            OpenError::Unreadable {
                path: PathBuf::from("/tmp/bad.clayspace"),
                detail: "truncated".to_string(),
            },
        ];
        for case in cases {
            let said = case.to_string();
            assert!(!said.is_empty(), "{case:?} says nothing");
            assert!(
                said.chars().any(|c| c.is_alphabetic()),
                "{case:?} says only punctuation"
            );
        }
    }

    #[test]
    fn a_newer_document_is_not_reported_as_damaged() {
        // The distinction this enum exists for. "Unreadable" sends a user
        // looking for corruption; "too new" sends them to update, which is
        // the thing that will actually work.
        let too_new = OpenError::TooNew {
            path: PathBuf::from("/tmp/new.clayspace"),
            detail: "0.31".to_string(),
        };
        let said = too_new.to_string();
        assert!(
            said.contains("recente"),
            "a newer-format refusal must say so: {said}"
        );
        assert!(
            !said.contains("ilegível"),
            "a newer-format refusal must not read as damage: {said}"
        );
    }
}
