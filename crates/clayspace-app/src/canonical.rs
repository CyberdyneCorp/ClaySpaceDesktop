//! The canonical document: one file both platforms author from nothing.
//!
//! Task 8.2 asks whether a document saved on macOS and on Linux is the same
//! file, and CI answers it by authoring this on each and comparing digests.
//! The fixture lives here rather than in the binary so a test can reach it —
//! see `tests/canonical_document.rs`, which checks on one platform the
//! property the matrix checks across two.
//!
//! # Which verbs belong here
//!
//! **Only the ones whose record is the ask itself.** A document is an edit
//! list: for a stamping verb what goes in the file is the numbers the gesture
//! was given, so the bytes are the same wherever they were authored. Two
//! families are not like that and are deliberately absent:
//!
//! * **Inflar and Pinçar.** They are `clay_layer_magnify_surface`, and the
//!   stroke sinks each dab's centre half a radius along the field's own
//!   gradient before the engine records it (`sink_into_the_material`). That
//!   gradient is `clay_eval_gradients` — an evaluation, not an ask — and the
//!   engine "pins no FP flags for its own translation units and makes no
//!   cross-build promise" (`docs/05-claycore-library.md`). So the centre in
//!   the file carries whatever arm64-macOS and x86_64-Linux disagree about
//!   under contraction, and the digests part. Inflar stood here until it
//!   became a magnify (#201) and that is exactly what happened.
//! * **Suavizar, Planar, Polir, Relaxar and the topological drag.** They bake
//!   a resampled volume into the layer, so every sample they write is a
//!   computed float with the same problem, only more of it.
//!
//! Neither is a defect in those tools — a verb that has to ask where the
//! surface is cannot record only what it was told — and neither is a reason
//! to weaken the comparison. It is a reason to compare a document made of
//! verbs that can.

use clayspace_engine::{claycore, BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, DocumentModel, GestureSample, ModelError, Representation, SceneModel,
    SculptModel, ToolKind,
};

/// The strokes on the starting form: a verb and where it was made.
///
/// Chosen to exercise more than one verb and to be asymmetric, so a platform
/// that mirrors or truncates has somewhere to show it.
const BASE: [(ToolKind, [f32; 3]); 4] = [
    (ToolKind::Padrao, [0.30, 0.10, 0.50]),
    (ToolKind::Padrao, [-0.20, 0.40, 0.45]),
    // Argila where Inflar used to be: a second stamping verb — relief with
    // buildup accumulation — at the same asymmetric place, and one whose
    // record is its parameters. See the module note for why Inflar left.
    (ToolKind::Argila, [0.15, -0.35, 0.48]),
    // A drag of one sample, which is no drag: it reaches the baked route and
    // records nothing. Kept because the dispatch is worth crossing, and left
    // out of `RECORDED` for the same reason.
    (ToolKind::Mover, [0.40, 0.05, 0.42]),
];

/// The stroke on the second layer, so the file carries structure and not only
/// one blob.
const DETAIL: (ToolKind, [f32; 3]) = (ToolKind::Padrao, [0.0, 0.0, 0.60]);

/// The name the second layer is given.
const DETAIL_LAYER: &str = "Detalhe";

/// Every position the saved file has to carry **verbatim**, as the three
/// little-endian floats they were asked for.
///
/// This is the module note made checkable: a verb that computes where it acts
/// instead of recording where it was asked to drops its position out of the
/// bytes, and the test says so on whichever platform ran it.
pub const RECORDED: [[f32; 3]; 4] = [BASE[0].1, BASE[1].1, BASE[2].1, DETAIL.1];

/// A fresh document on the CPU backend, with the starting form in it.
///
/// The CPU backend explicitly. The document is an edit list and does not
/// depend on what evaluated it, but naming it here removes the question from a
/// comparison whose whole point is that nothing else varies.
pub fn document() -> Result<ClayDocument, ModelError> {
    let policy = BackendPolicy::from_available(vec![claycore::Backend::Cpu], None);
    ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
}

/// Authors the fixture into a document that has the starting form.
pub fn author(document: &mut ClayDocument) -> Result<(), ModelError> {
    for (tool, at) in BASE {
        stroke(document, tool, at)?;
    }

    let layer = document.add_layer(DETAIL_LAYER, Representation::Sdf)?;
    document.set_active_layer(layer)?;
    let (tool, at) = DETAIL;
    stroke(document, tool, at)
}

/// One dab, with the default brush and no symmetry.
fn stroke(document: &mut ClayDocument, tool: ToolKind, at: [f32; 3]) -> Result<(), ModelError> {
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
    Ok(())
}

/// Authors the fixture from nothing and writes it to `path`.
pub fn write(path: &std::path::Path) -> Result<(), ModelError> {
    let mut document = document()?;
    author(&mut document)?;
    document.save(path)
}
