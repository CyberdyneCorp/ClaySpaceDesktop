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
//! Deliberately not a test: a test would have to decide what the right hash
//! is, and the right hash changes with the engine. What CI compares is one
//! platform's answer against another's, in the same commit.

#![forbid(unsafe_code)]

use clayspace_engine::{claycore, BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, DocumentModel, GestureSample, Representation, SceneModel, SculptModel, ToolKind,
};

/// Strokes chosen to exercise more than one verb and to be asymmetric, so a
/// platform that mirrors or truncates has somewhere to show it.
fn author(document: &mut ClayDocument) -> Result<(), Box<dyn std::error::Error>> {
    for (tool, at) in [
        (ToolKind::Padrao, [0.30f32, 0.10, 0.50]),
        (ToolKind::Padrao, [-0.20, 0.40, 0.45]),
        (ToolKind::Inflar, [0.15, -0.35, 0.48]),
        (ToolKind::Mover, [0.40, 0.05, 0.42]),
    ] {
        document.apply_stroke(
            tool,
            BrushSettings::default(),
            &[GestureSample {
                position: at,
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )?;
    }

    // A second layer, so the file carries structure and not only one blob.
    let layer = document.add_layer("Detalhe", Representation::Sdf)?;
    document.set_active_layer(layer)?;
    document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings::default(),
        &[GestureSample {
            position: [0.0, 0.0, 0.60],
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    )?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: canonical <path>");
        std::process::exit(2);
    };

    // The CPU backend explicitly. The document is an edit list and does not
    // depend on what evaluated it, but naming it here removes the question
    // from a comparison whose whole point is that nothing else varies.
    let policy = BackendPolicy::from_available(vec![claycore::Backend::Cpu], None);
    let mut document = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)?;
    author(&mut document)?;
    document.save(std::path::Path::new(&path))?;

    let bytes = std::fs::metadata(&path)?.len();
    println!("{path}: {bytes} bytes");
    Ok(())
}
