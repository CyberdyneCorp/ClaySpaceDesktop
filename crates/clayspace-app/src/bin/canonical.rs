//! Writes the canonical document, for the byte-identical check.
//!
//! Task 8.2 asks whether a document saved on macOS and on Linux is the same
//! file. Answering it needs one document that both platforms can build from
//! nothing — no dialog, no assets, no randomness — so this is that document,
//! defined once and used by CI on both.
//!
//! ```sh
//! cargo run -p clayspace-app --bin canonical -- /tmp/canonical.clayspace
//! shasum -a 256 /tmp/canonical.clayspace
//! ```
//!
//! The fixture itself is `clayspace_app::canonical`, which says which verbs
//! belong in it and why the ones that are missing are missing. This is the
//! command around it.
//!
//! Deliberately not a test: a test would have to decide what the right hash
//! is, and the right hash changes with the engine. What CI compares is one
//! platform's answer against another's, in the same commit.

#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: canonical <path>");
        std::process::exit(2);
    };

    let path = std::path::PathBuf::from(path);
    clayspace_app::canonical::write(&path)?;

    let bytes = std::fs::metadata(&path)?.len();
    println!("{}: {bytes} bytes", path.display());
    Ok(())
}
