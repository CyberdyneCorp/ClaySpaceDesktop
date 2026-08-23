//! What a mask does, and does not do, to an operation that crosses it.
//!
//! A mask gates *authoring*: an SDF stroke consumes one when it becomes items,
//! so a brush does not deposit where the mask protects. It does not gate the
//! item once it is in the edit list, so a subtracting stroke crossing a
//! protected region takes the material anyway.
//!
//! `clay_item_set_gate` is the entry point that would close that, and in
//! ClayCore 0.39.0 it is accepted and inert — measured at the engine boundary
//! in `claycore/tests/mask_gate.rs`, with a mask sampling 1.0 at the cut's own
//! centre, at every width and threshold tried. So these tests hold the
//! behaviour as it is rather than as it should be, and the tripwire that says
//! when it changes lives one layer down where the measurement is unambiguous.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, GestureSample, MaskModel, SculptModel, ToolKind,
};

fn document() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.3,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// A drag across the front of the form, passing through the origin.
fn arc() -> Vec<GestureSample> {
    (0..8)
        .map(|i| {
            let t = i as f32 / 7.0;
            GestureSample {
                position: [(t - 0.5) * 0.8, 0.0, 1.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// Paints a mask over the middle of that path.
fn mask_the_middle(document: &mut ClayDocument) -> bool {
    let samples: Vec<GestureSample> = (0..5)
        .map(|i| {
            let t = i as f32 / 4.0;
            GestureSample {
                position: [(t - 0.5) * 1.2, 0.0, 1.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    let painted = BrushSettings {
        size: 0.25,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    document
        .apply_stroke(ToolKind::Mascara, painted, &samples, [false; 3])
        .is_ok()
        && document.mask_state().present
}

fn centre(document: &ClayDocument) -> Option<f32> {
    document
        .pick([0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .map(|hit| hit[2])
}

/// A mask does not yet protect a region from a subtracting edit that crosses
/// it, and this says so rather than leaving it unstated.
///
/// Held as an equality on purpose. The tempting version of this test asserts
/// that the protected region survives, fails, and gets marked `#[ignore]` —
/// which records nothing and is never looked at again. This records the
/// behaviour, and it fails the moment the behaviour improves.
#[test]
fn a_mask_does_not_yet_protect_a_region_from_a_subtracting_edit() {
    let (Some(mut open), Some(mut protected)) = (document(), document()) else {
        return;
    };
    let start = centre(&open).expect("the starting form is under the ray");
    assert!(
        mask_the_middle(&mut protected),
        "the mask stroke painted nothing, so there is nothing to protect"
    );
    assert!(
        protected.mask_state().painted_cells > 1000,
        "the mask is too sparse for the comparison to say anything"
    );

    let cutting = CombineSettings {
        op: Combine::Subtract,
        radius: 0.0,
        ..Default::default()
    };
    open.set_combine(cutting);
    protected.set_combine(cutting);

    let _ = open.apply_stroke(ToolKind::Padrao, brush(), &arc(), [false; 3]);
    let _ = protected.apply_stroke(ToolKind::Padrao, brush(), &arc(), [false; 3]);

    let cut = centre(&open).expect("a surface");
    let kept = centre(&protected).expect("a surface");
    assert!(
        cut < start - 0.01,
        "the subtraction did not cut into the form ({start} -> {cut}), so the \
         comparison says nothing about the mask"
    );
    assert_eq!(
        cut, kept,
        "a mask has started protecting a region from a subtracting edit. That \
         is good news and it is what clay_item_set_gate was wrapped for: \
         claycore's mask_gate test and stroke_sdf's removed gate call should \
         both change with it."
    );
}

/// A mask still gates *authoring*, which is the half that does work: a brush
/// does not deposit where the mask protects. Held so that closing the gap does
/// not quietly cost the protection that already exists.
///
/// The mask has to be wider than the brush for that to be a question with an
/// answer, which is the whole of what was wrong with the earlier version —
/// see the assertion. Measured at the centre of a protected strip against an
/// unmasked document taking the same stroke: 1.0005 against 1.1400, on a
/// sphere that started at 1.0. The protection is close to total.
#[test]
fn a_mask_still_keeps_a_brush_from_depositing() {
    let (Some(mut open), Some(mut protected)) = (document(), document()) else {
        return;
    };
    assert!(mask_the_middle(&mut protected));

    let _ = open.apply_stroke(ToolKind::Padrao, brush(), &arc(), [false; 3]);
    let _ = protected.apply_stroke(ToolKind::Padrao, brush(), &arc(), [false; 3]);

    let raised = centre(&open).expect("a surface");
    let held = centre(&protected).expect("a surface");
    // A real margin, not a sign test. This assertion used to read `held <
    // raised` over a mask 0.2 wide under a brush of radius 0.3 — the brush
    // reached across the protected strip from both sides, so the centre was
    // deposited into either way and the two differed by 7e-7 on a 0.14
    // deposit. It passed on rounding, and the first change to touch the stroke
    // path in any way flipped the last bits and broke it.
    //
    // The mask is wider than the brush now, which is the only arrangement in
    // which "the brush did not deposit here" is a question with an answer —
    // and asked properly the answer is emphatic: 1.0005 against 1.1400, which
    // is the sphere it started as.
    assert!(
        held < raised - 0.005,
        "the mask did not keep the brush from depositing ({held} against an \
         unmasked {raised}), so masking protects nothing at all"
    );
}

/// And a stroke on an unmasked document is unaffected by any of this.
#[test]
fn an_unmasked_document_strokes_as_it_always_did() {
    let Some(mut document) = document() else {
        return;
    };
    assert!(!document.mask_state().present);
    document.set_combine(CombineSettings {
        op: Combine::Subtract,
        radius: 0.0,
        ..Default::default()
    });
    let outcome = document
        .apply_stroke(ToolKind::Padrao, brush(), &arc(), [false; 3])
        .expect("a stroke on an unmasked document must not be refused");
    assert!(outcome.changed, "the stroke did nothing");
}
