//! The mirror this side *recorded* against the mirror the engine *has*.
//!
//! `clay_set_layer_mirror` writes a layer's mirror and the ABI has no call
//! that reads one back, so `Layer::mirror` is the only account of it there is.
//! An account that cannot be reconciled has to be abandoned when it might be
//! wrong, and there is exactly one moment it might be: `set_symmetry` records
//! the sculptor's choice without writing anything, and the *stroke* writes the
//! mirror — inside its own gesture, so that one undo spends the mirror along
//! with the rest of the stroke.
//!
//! That is deliberate and it is right. What it costs is that the same undo
//! reverts the engine's mirror and leaves this side still holding the value it
//! wrote. The guard in `point_the_mirror` then compares equal, takes its early
//! return, and the next stroke goes through a mirror the host believes it has
//! turned off — with the interface still showing the sculptor's setting, so
//! there is nothing to notice until the geometry is wrong.
//!
//! Measured before the fix: a dab made with symmetry **off** moved the far
//! side of the form by 0.28 units.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SceneModel, SculptModel, ToolKind};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far the surface stands from the origin along `direction`.
fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> Option<f32> {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    let origin = unit.map(|c| c * 4.0);
    let hit = document.pick(origin, unit.map(|c| -c))?;
    Some(hit.iter().map(|c| c * c).sum::<f32>().sqrt())
}

/// A stamping stroke on the +x pole, which the far side must not feel.
fn dab(document: &mut ClayDocument, symmetry: [bool; 3]) {
    let samples: Vec<GestureSample> = (0..3)
        .map(|i| GestureSample {
            position: [1.0, i as f32 * 0.02, 0.0],
            pressure: 1.0,
            time: i as f32,
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.35,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            symmetry,
        )
        .expect("a dab");
}

/// The far side stays put when the sculptor has turned symmetry off, even
/// after an undo has taken back the stroke that wrote the mirror.
#[test]
fn a_stroke_after_an_undone_mirror_edit_is_still_unmirrored() {
    let mut document = sphere();
    // The starting form carries `Layer::STARTING_SYMMETRY`, which is x. Asking
    // for none is therefore a real change, and the first stroke is what writes
    // it — putting a `clay_set_layer_mirror` inside that stroke's gesture.
    let off = [false; 3];
    SculptModel::set_symmetry(&mut document, off).expect("record the setting");

    let rested = radius_along(&document, [-1.0, 0.0, 0.0]).expect("the far pole");
    let before = document.history().depth;

    dab(&mut document, off);
    let unmirrored = radius_along(&document, [-1.0, 0.0, 0.0]).expect("the far pole");
    assert!(
        (unmirrored - rested).abs() < 1e-3,
        "the far side moved by {} on the FIRST unmirrored dab, so this test is \
         no longer about the undo",
        (unmirrored - rested).abs()
    );

    // Exactly what the dab recorded — the stroke and the mirror edit inside
    // it — so the starting form survives and the mirror does not.
    let recorded = document.history().depth.saturating_sub(before);
    assert!(
        recorded >= 2,
        "the dab recorded {recorded} entries; the mirror edit is supposed to \
         be one of them, and without it this test proves nothing"
    );
    for _ in 0..recorded {
        document.undo().expect("undo");
    }

    let reverted = radius_along(&document, [-1.0, 0.0, 0.0]).expect("the far pole");
    dab(&mut document, off);
    let again = radius_along(&document, [-1.0, 0.0, 0.0]).expect("the far pole");

    let moved = (again - reverted).abs();
    assert!(
        moved < 1e-3,
        "the far side moved by {moved} on a dab the sculptor asked to be \
         unmirrored. The undo took the engine's mirror back and this side went \
         on believing its own record of it, so `point_the_mirror` skipped the \
         call that would have turned it off"
    );
}

/// And the reverse: symmetry asked for, undone, and still honoured after.
///
/// Without this the fix could be "never mirror after an undo", which passes
/// the test above and is wrong in the other direction.
#[test]
fn a_stroke_after_an_undone_mirror_edit_is_still_mirrored_when_asked() {
    let mut document = sphere();
    let on = [true, false, false];
    // Off first, so that asking for it back is a change the stroke must write.
    SculptModel::set_symmetry(&mut document, [false; 3]).expect("record");
    dab(&mut document, [false; 3]);

    SculptModel::set_symmetry(&mut document, on).expect("record");
    let before = document.history().depth;
    dab(&mut document, on);
    let recorded = document.history().depth.saturating_sub(before);
    for _ in 0..recorded {
        document.undo().expect("undo");
    }

    let reverted = radius_along(&document, [-1.0, 0.0, 0.0]).expect("the far pole");
    dab(&mut document, on);
    let again = radius_along(&document, [-1.0, 0.0, 0.0]).expect("the far pole");

    assert!(
        (again - reverted).abs() > 1e-3,
        "the far side did not move on a MIRRORED dab: it read {reverted} and \
         then {again}. Forgetting the mirror after an undo must make the next \
         stroke write it, not skip it"
    );
}

/// The engine is asked what mirror a layer carries, rather than guessed at.
///
/// **This pins the contract, not a saving, and passes with the readback
/// reverted.** Said plainly because the issue claimed a saving that does not
/// exist: measured, first-stroke-after-undo dirties exactly as many bricks
/// with the readback as without it — 4312/4312 for Puxar, 3144/3728 for Mover,
/// 2852/2852 for Padrao, the same six numbers either way.
/// `mirror_for_dirtying` is consulted only by the curve and live-hook tail
/// paths, which a plain stroke does not enter, and by the time it is reached
/// `point_the_mirror` has already filled the cache.
///
/// So the readback is kept as correctness rather than as a performance fix,
/// and what is worth pinning is that the engine answers at all — which is what
/// makes ask-and-compare possible in place of forget-and-guess-safe.
///
/// ClayCore 0.105.0 added `clay_document_layer_mirror`, so an account that
/// could previously only be abandoned can now be reconciled. Two things follow
/// and both are asserted here:
///
/// - `point_the_mirror` on a layer whose mirror was forgotten reads it back
///   and skips the write when it already matches. Before, every first stroke
///   after a history step re-wrote a mirror the layer already carried.
/// - `mirror_for_dirtying` returns what the layer *has* rather than `[true;
///   3]`. That guess made the first stroke after every undo re-fill three
///   reflections of its region, on a layer that may carry no mirror at all.
///
/// Asserted through the engine's own answer rather than through our cache,
/// because the cache agreeing with itself is what the original fault was.
#[test]
fn a_forgotten_mirror_is_read_back_rather_than_assumed() {
    let mut document = sphere();
    let key = document.scene().active_layer().expect("a layer").key;
    let layer = document.layer_id(key).expect("its engine id");

    // An unmirrored stroke, which writes the layer's mirror as "off".
    document.set_symmetry([false; 3]).expect("symmetry off");
    dab(&mut document, [false; 3]);

    let (carried, _) = document
        .document()
        .layer_mirror(layer)
        .expect("an SDF layer answers what mirror it carries");
    assert_eq!(
        carried, [false; 3],
        "the stroke asked for no mirror and the engine says it has one"
    );

    // A history step, which is what forgets this side's account of it.
    document.undo().expect("undo");
    document.redo().expect("redo");

    let (after, _) = document
        .document()
        .layer_mirror(layer)
        .expect("still an SDF layer");
    assert_eq!(
        after, carried,
        "a step through the history changed what the engine says the layer's \
         mirror is, from {carried:?} to {after:?} — which is the fault this \
         file exists for, now visible directly instead of by its symptom"
    );
}

/// Every layer this application makes can be asked, so the guess is unreached.
///
/// `mirror_for_dirtying` still answers `[true; 3]` when the engine refuses,
/// and the header reserves that refusal for a layer that cannot express a
/// mirror. Measured, a mesh layer does not reach it: it answers with the
/// mirror off, which is the true answer and the one a host walking a stack
/// wants rather than a special case.
///
/// So the fallback is dead code on today's layer kinds — asserted rather than
/// assumed, because "the safe guess is rare" and "the safe guess never happens"
/// are different claims and only the measured one is worth writing down.
#[test]
fn every_layer_kind_answers_what_mirror_it_carries() {
    let mut document = sphere();
    let key = document.scene().active_layer().expect("a layer").key;
    let layer = document.layer_id(key).expect("its engine id");
    assert!(
        document.document().layer_mirror(layer).is_ok(),
        "the SDF layer refused to say what mirror it carries"
    );

    let mesh_key = document
        .add_mesh_layer("uma malha")
        .expect("a mesh layer to ask about");
    let mesh_layer = document.layer_id(mesh_key).expect("its engine id");
    assert_eq!(
        document.document().layer_mirror(mesh_layer).ok(),
        Some(([false; 3], 0.0)),
        "a mesh layer no longer answers with the mirror off; if it now refuses, \
         `mirror_for_dirtying` falls back to dirtying all three reflections of \
         every region on it, which is safe and wasteful"
    );
}
