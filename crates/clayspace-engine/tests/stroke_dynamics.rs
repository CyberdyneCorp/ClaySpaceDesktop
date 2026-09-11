//! The stroke controls the engine resolves and nothing was sending.
//!
//! `clay_stroke_preset` carries twelve fields. `clay_stroke_resolve` resolves
//! all twelve and `clay_layer_apply_stroke` consumes all twelve, and this host
//! built a preset that set five of them and took the engine's defaults for the
//! rest. Every brush therefore ran with pressure disconnected, no taper and no
//! rake, for the life of the application — not as a decision, but because the
//! fields were never plumbed.
//!
//! These assert the values *arrive*, by the only evidence that cannot be faked
//! from this side: the same gesture, sent twice, puts the surface in two
//! different places. A test that read the preset back would pass against a
//! preset nobody passed to the engine.
//!
//! **Move is deliberately absent.** A drag anchors its region at the press,
//! so a radius that changes mid-gesture is a different region rather than a
//! different brush, and the drag paths build no preset at all. See
//! `Dynamics`'s own note and ClayCore #532/#533.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Dynamics, GestureSample, SculptModel, ToolKind};

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

/// A stroke across the +z pole, with pressure rising along it.
///
/// Rising rather than constant, because a pressure control that is honoured
/// and a pressure control that is ignored are the same thing under a constant.
fn drawn(dynamics: Dynamics) -> ClayDocument {
    let mut document = sphere();
    let samples: Vec<GestureSample> = (0..=10)
        .map(|i| {
            let t = i as f32 / 10.0;
            GestureSample {
                position: [(t - 0.5) * 0.7, 0.0, 1.0],
                // 0.1 at the press, 1.0 at the release.
                pressure: 0.1 + 0.9 * t,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.25,
                intensity: 1.0,
                dynamics,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("a stroke");
    document
}

/// Where the stroke started, which is the end a start-taper thins.
const AT_THE_START: [f32; 3] = [-0.35, 0.0, 1.0];

fn moved(dynamics: Dynamics, probe: [f32; 3]) -> f32 {
    radius_along(&drawn(dynamics), probe).expect("the surface is there")
}

/// Pressure drives the radius, and a stroke that ignores it is a different
/// stroke.
#[test]
fn pressure_reaches_the_radius() {
    let ignored = moved(Dynamics::default(), AT_THE_START);
    let honoured = moved(
        Dynamics {
            pressure_size: 1.0,
            ..Dynamics::default()
        },
        AT_THE_START,
    );
    assert!(
        (honoured - ignored).abs() > 1e-4,
        "the light end of a pressure ramp put the surface in the same place \
         with pressure_size at 1.0 as at 0.0 ({honoured} against {ignored}), \
         so the field is not reaching the engine"
    );
}

/// And the strength, which is a separate field and separately plumbed.
#[test]
fn pressure_reaches_the_strength() {
    // Both ends spelled out. `Dynamics::default()` carries the ENGINE's
    // default for this field, which is 1 — pressure has driven strength since
    // before the control existed — so a test that used the default as its
    // "off" arm would be comparing 1 against 1 and passing on nothing.
    let disconnected = moved(
        Dynamics {
            pressure_strength: 0.0,
            ..Dynamics::default()
        },
        AT_THE_START,
    );
    let honoured = moved(
        Dynamics {
            pressure_strength: 1.0,
            ..Dynamics::default()
        },
        AT_THE_START,
    );
    assert!(
        (honoured - disconnected).abs() > 1e-4,
        "pressure_strength at 1.0 and at 0.0 put the surface in the same \
         place: {honoured} against {disconnected}"
    );
}

/// The curve bends what pressure means, so it must not be a no-op either.
///
/// Asserted against a stroke that *is* pressure-driven: with `pressure_size`
/// at zero there is nothing for an exponent to bend, and a curve test that
/// passed there would be testing nothing.
#[test]
fn the_pressure_curve_bends_what_pressure_means() {
    // Strength held OFF so the curve is observed through one channel only.
    // With both size and strength pressure-driven the two responses move
    // together and the probe reads their combination, which was enough to hide
    // the difference entirely.
    let linear = moved(
        Dynamics {
            pressure_size: 1.0,
            pressure_strength: 0.0,
            pressure_curve: 1.0,
            ..Dynamics::default()
        },
        AT_THE_START,
    );
    let bent = moved(
        Dynamics {
            pressure_size: 1.0,
            pressure_strength: 0.0,
            pressure_curve: 3.0,
            ..Dynamics::default()
        },
        AT_THE_START,
    );
    assert!(
        (bent - linear).abs() > 1e-4,
        "an exponent of 3 and an exponent of 1 shaped the same stroke: \
         {bent} against {linear}"
    );
}

/// A taper thins the end of the stroke it names.
#[test]
fn a_taper_reaches_the_stroke() {
    let untapered = moved(Dynamics::default(), AT_THE_START);
    let tapered = moved(
        Dynamics {
            taper_start: 1.0,
            ..Dynamics::default()
        },
        AT_THE_START,
    );
    assert!(
        (tapered - untapered).abs() > 1e-4,
        "a full start-taper left the start of the stroke where an untapered \
         one did: {tapered} against {untapered}"
    );
}

/// Every field survives `sanitized`, which is the other way a control reaches
/// the engine as nothing.
///
/// The clamp is where a plumbed field quietly becomes the default again — it
/// is what `MAX_JITTER` does to Ruído, deliberately, and what must not happen
/// to these by accident.
#[test]
fn the_clamp_passes_a_usable_setting_through_unchanged() {
    let asked = Dynamics {
        pressure_size: 0.75,
        pressure_strength: 0.5,
        pressure_curve: 2.0,
        taper_start: 0.3,
        taper_end: 0.4,
        rake: true,
    };
    assert_eq!(
        asked.sanitized(),
        asked,
        "a setting inside every range came back changed"
    );
    // And an impossible one is corrected rather than passed on.
    let absurd = Dynamics {
        pressure_size: 5.0,
        pressure_strength: -1.0,
        pressure_curve: 0.0,
        taper_start: 9.0,
        taper_end: -3.0,
        rake: false,
    }
    .sanitized();
    assert_eq!(absurd.pressure_size, 1.0);
    assert_eq!(absurd.pressure_strength, 0.0);
    assert_eq!(
        absurd.pressure_curve, 1.0,
        "a zero exponent is not a curve; the engine's own default is 1"
    );
    assert_eq!(absurd.taper_start, 1.0);
    assert_eq!(absurd.taper_end, 0.0);
}

/// The host's defaults are the engine's defaults, field for field.
///
/// **This is the test that was missing, and its absence shipped a regression.**
///
/// These six fields were always reaching the engine — `StrokePreset::default()`
/// asks `clay_stroke_preset_defaults` for them, so every brush ran with the
/// engine's values. Plumbing them moved the decision to this side, and the
/// first version of `Dynamics::default()` set them all to zero on the reasoning
/// that "off" is the safe default for a new control.
///
/// It is not. `pressure_strength` defaults to **1** in the engine: pressure has
/// driven stroke strength since before the control existed. Zeroing it
/// disconnected pen pressure from every brush in the application, silently,
/// while the commit claimed a brush nobody had touched would behave exactly as
/// it did. Nothing in the suite noticed; a latency test on another machine did,
/// for the wrong reason.
///
/// So the two are pinned together. If the engine changes a default this fails
/// and someone decides deliberately, which is the only safe way for a value
/// like this to move.
#[test]
fn the_defaults_are_the_engines_defaults() {
    let engine = claycore::StrokePreset::default();
    let host = Dynamics::default();

    assert_eq!(
        host.pressure_size, engine.pressure_size,
        "pressure_size: host {} against the engine's {}",
        host.pressure_size, engine.pressure_size
    );
    assert_eq!(
        host.pressure_strength, engine.pressure_strength,
        "pressure_strength: host {} against the engine's {}. This is the one \
         that was wrong — the engine drives strength from pressure by default \
         and a zero here turns that off for every brush",
        host.pressure_strength, engine.pressure_strength
    );
    assert_eq!(
        host.pressure_curve, engine.pressure_curve,
        "pressure_curve: host {} against the engine's {}",
        host.pressure_curve, engine.pressure_curve
    );
    assert_eq!(host.taper_start, engine.taper_start);
    assert_eq!(host.taper_end, engine.taper_end);
    assert_eq!(host.rake, engine.rotate_along_stroke);
}

/// And an untouched brush strokes exactly as it did before the control existed.
///
/// The property the test above protects, asserted end to end rather than field
/// by field: a default brush and one whose dynamics are spelled out as the
/// engine's own values put the surface in the same place.
#[test]
fn an_untouched_brush_strokes_as_it_always_did() {
    let engine = claycore::StrokePreset::default();
    let spelled_out = Dynamics {
        pressure_size: engine.pressure_size,
        pressure_strength: engine.pressure_strength,
        pressure_curve: engine.pressure_curve,
        taper_start: engine.taper_start,
        taper_end: engine.taper_end,
        rake: engine.rotate_along_stroke,
    };
    let untouched = moved(Dynamics::default(), AT_THE_START);
    let explicit = moved(spelled_out, AT_THE_START);
    assert!(
        (untouched - explicit).abs() < 1e-6,
        "a brush left alone strokes differently from one told to use the \
         engine's own values: {untouched} against {explicit}"
    );
}
