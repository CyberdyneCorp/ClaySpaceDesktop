//! Inflate and Pinch on a field, which are one engine verb with two signs.
//!
//! What this file pins is the change #201 made and the reason it was made.
//! Before it, `Inflar` and `Padrao` both applied `clay_layer_apply_stroke` in
//! the Relief family — relief moves the surface along its own normal, which is
//! what Standard *is* — so the two brushes differed only in footprint and
//! lift, and the audit measured the consequence: Inflar left a **taller** mark
//! than Padrão where an inflate should be broader and lower (#179). `Pincar`
//! had no field verb at all, because a per-item `CLAY_DEFORM_MAGNIFY` gathers
//! one piece of a smooth-unioned form and leaves the others (ClayCore #391).
//!
//! `clay_layer_magnify_surface` answers both: a signed radial scale resolved
//! against every item the region reaches. Positive swells, negative gathers.
//!
//! # Read the surface with a pick, and read it where the mark is
//!
//! Every measurement here uses [`SculptModel::pick`], as `sdf_brushes.rs`
//! does, and takes a *profile* across the stroke rather than a single reading
//! under it. A swell and a ridge can peak at the same height and be different
//! marks; what separates them is how far the mark reaches for the height it
//! has, and one reading cannot see that.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, Representation, SculptModel, ToolKind};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Where the strokes in this file are made, on the unit starting form.
const AT: [f32; 3] = [0.6, 0.0, 0.8];

/// How far out the profile is taken, and at what spacing. Past 0.27 every
/// brush measured here has returned the surface to rest.
const ASIDE: [f32; 9] = [0.0, 0.03, 0.06, 0.09, 0.12, 0.15, 0.18, 0.21, 0.24];

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.25,
        intensity: 0.9,
        ..BrushSettings::default()
    }
}

/// How far the surface stands from the centre along a direction.
fn reach(document: &ClayDocument, direction: [f32; 3]) -> f32 {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    SculptModel::pick(document, unit.map(|c| c * 4.0), unit.map(|c| -c))
        .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
        .unwrap_or(f32::NAN)
}

/// A short stroke across the form, which is what every brush here is given.
fn samples() -> Vec<GestureSample> {
    (0..=6)
        .map(|step| {
            let t = step as f32 / 6.0;
            GestureSample {
                position: [AT[0] + (t - 0.5) * 0.2, AT[1], AT[2]],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

fn stroke_with(document: &mut ClayDocument, tool: ToolKind, brush: BrushSettings) {
    document
        .apply_stroke(tool, brush, &samples(), [false; 3])
        .expect("the stroke was refused");
}

fn stroke(document: &mut ClayDocument, tool: ToolKind) {
    stroke_with(document, tool, brush());
}

/// How far the surface moved at each distance to the side of the stroke.
///
/// Sideways along +y, which is across the stroke rather than along it: the
/// stroke runs in x, so a profile taken along x would measure the taper at its
/// ends instead of the shape of the mark.
fn profile(document: &ClayDocument, rest: &[f32; 9]) -> [f32; 9] {
    std::array::from_fn(|i| reach(document, [AT[0], ASIDE[i], AT[2]]) - rest[i])
}

fn at_rest() -> [f32; 9] {
    let base = sphere();
    std::array::from_fn(|i| reach(&base, [AT[0], ASIDE[i], AT[2]]))
}

/// How far out the mark still stands proud of the resting surface.
fn width(marked: &[f32; 9], floor: f32) -> f32 {
    ASIDE
        .iter()
        .zip(marked)
        .filter(|(_, lift)| lift.abs() > floor)
        .map(|(aside, _)| *aside)
        .fold(0.0, f32::max)
}

// -- the two verbs -----------------------------------------------------------

/// The measurement #179 asked for, the other way round.
///
/// The audit measured Inflar leaving a taller mark than Padrão — 75 px against
/// 50, over six reproductions — which is what binding both to relief gets you.
/// An inflate swells: it should spend the material it moves on *width* rather
/// than on height. Both numbers are asserted, because either alone passes for
/// the wrong reason — a weaker brush is lower and no broader, and a bigger one
/// is broader and no lower.
#[test]
fn sdf_inflate_is_broader_and_lower_than_standard() {
    let rest = at_rest();

    let mut standard = sphere();
    stroke(&mut standard, ToolKind::Padrao);
    let ridge = profile(&standard, &rest);

    let mut inflated = sphere();
    stroke(&mut inflated, ToolKind::Inflar);
    let swell = profile(&inflated, &rest);

    // A tenth of a millimetre on a unit form: below this the pick is
    // measuring its own marcher rather than the clay.
    const FLOOR: f32 = 1e-3;
    assert!(
        swell[0] > FLOOR,
        "Inflar left no mark at all: {:?}",
        swell[0]
    );
    assert!(
        swell[0] < ridge[0],
        "Inflar peaked at {} where Padrão peaked at {}, which is #179 again: \
         an inflate that is taller than standard is standard with a bigger \
         brush",
        swell[0],
        ridge[0]
    );
    let (broad, narrow) = (width(&swell, FLOOR), width(&ridge, FLOOR));
    assert!(
        broad > narrow,
        "Inflar reached {broad} to the side where Padrão reached {narrow}; it \
         is lower without being broader, which is a weaker brush rather than \
         a different one"
    );
}

/// Pinch gathers the surface toward the stroke.
///
/// Which is one mark and not two readings: material leaves the flanks of the
/// region and arrives under the stroke, so the middle stands proud and the rim
/// falls away. A tool that only raised the middle would be a weak inflate, and
/// one that only lowered the rim would be a trench.
#[test]
fn sdf_pinch_gathers_the_surface() {
    let rest = at_rest();
    let mut document = sphere();
    stroke(&mut document, ToolKind::Pincar);
    let gathered = profile(&document, &rest);

    assert!(
        gathered[0] > 1e-3,
        "Pinçar left the surface under the stroke at {}, so nothing gathered \
         there",
        gathered[0]
    );
    let rim = gathered[ASIDE.len() - 1];
    assert!(
        rim < 0.0,
        "Pinçar left the rim of its region at {rim}, so the material under \
         the stroke came from nowhere"
    );
}

/// The invert key gives each tool its own opposite, not the other tool.
///
/// Which is not the obvious answer, and the obvious answer is wrong: these two
/// are the two signs of one engine verb, so an inverted Inflar looks
/// arithmetically like a Pinçar — and is not one, because the tools differ in
/// where the dab sits as well as in the sign. Inflar's dabs are sunk into the
/// material, which is what makes the scale a swell; keeping that and turning
/// the sign over gives a **deflate**. Pinçar's dabs sit on the surface, which
/// is what makes the scale a gather; turning its sign over **spreads**.
///
/// The same pair the grid's column already names for these two tools.
#[test]
fn inverting_a_magnify_brush_gives_its_own_opposite() {
    let rest = at_rest();

    let mut deflated = sphere();
    stroke_with(
        &mut deflated,
        ToolKind::Inflar,
        BrushSettings {
            invert: true,
            ..brush()
        },
    );
    let sunk = profile(&deflated, &rest);
    assert!(
        sunk[0] < -1e-3,
        "an inverted Inflar left the surface at {} under the stroke, so it \
         did not deflate",
        sunk[0]
    );

    let mut gathered = sphere();
    stroke(&mut gathered, ToolKind::Pincar);
    let mut spread = sphere();
    stroke_with(
        &mut spread,
        ToolKind::Pincar,
        BrushSettings {
            invert: true,
            ..brush()
        },
    );
    let (pinched, held) = (profile(&gathered, &rest), profile(&spread, &rest));
    let rim = ASIDE.len() - 2;
    assert!(
        pinched[0] > 0.0 && pinched[rim] < 0.0,
        "the upright pinch did not gather: {pinched:?}"
    );
    // A spread has no gather in it, which is the whole of what separates the
    // two: the material leaves the stroke instead of arriving at it, so the
    // surface rises across the footprint and nowhere falls.
    assert!(
        held.iter().all(|moved| *moved > -1e-4),
        "an inverted Pinçar left {held:?}, which still gathers somewhere"
    );
    assert!(
        held[rim] > 1e-3,
        "an inverted Pinçar left the rim of its region at {}, so nothing was \
         spread out to it",
        held[rim]
    );
}

// -- what the surface resolver is for ----------------------------------------

/// A form made of two smooth-unioned items, which is what every sculpt this
/// application makes actually is.
///
/// Built with the stroke vocabulary rather than by hand, so the blend is the
/// one the application produces: a Padrão dab on each side of the starting
/// form leaves items the layer unions.
fn blended_form() -> ClayDocument {
    let mut document = sphere();
    for side in [-1.0f32, 1.0] {
        let dab = [GestureSample {
            position: [side * 0.45, 0.0, 0.9],
            pressure: 1.0,
            time: 0.0,
        }];
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.3,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &dab,
                [false; 3],
            )
            .expect("the fixture's own dab was refused");
    }
    document
}

/// The whole reason the engine grew a surface magnify (ClayCore #391).
///
/// A `CLAY_DEFORM_MAGNIFY` put on one picked item scales that item's own field
/// and leaves the rest, so on a blended form the surface gathers on one side
/// of the blend and not the other — and nothing errors. A magnify centred on
/// the blend has to move **both** contributors.
#[test]
fn a_magnify_across_a_blend_moves_both_items() {
    let probes: [[f32; 3]; 2] = [[-0.45, 0.0, 0.9], [0.45, 0.0, 0.9]];
    let base = blended_form();
    let before: Vec<f32> = probes.iter().map(|at| reach(&base, *at)).collect();

    let mut document = blended_form();
    // One dab, centred between the two items and wide enough to reach both.
    document
        .apply_stroke(
            ToolKind::Inflar,
            BrushSettings {
                size: 0.7,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the stroke was refused");

    let after: Vec<f32> = probes.iter().map(|at| reach(&document, *at)).collect();
    for (side, (was, now)) in probes.iter().zip(before.iter().zip(after.iter())) {
        assert!(
            now - was > 1e-3,
            "the magnify left {side:?} at {now} from {was}; it scaled one \
             contributor of the blend and not the other, which is the defect \
             the surface resolver exists to fix"
        );
    }
    // And symmetrically, because the two sides are reflections of each other
    // and the gesture is centred between them.
    assert!(
        (after[0] - after[1]).abs() < 0.01,
        "the magnify left the two sides of the blend at {} and {}",
        after[0],
        after[1]
    );
}

// -- what a gesture costs the history ----------------------------------------

/// A stroke is one thing a sculptor did, so it is one thing to take back.
///
/// The engine already makes each `clay_layer_magnify_surface` one undo step
/// however many items it warped. A stroke lays down a dab per step of the
/// brush's spacing, so without a group around the gesture a single pass would
/// leave a row of entries in the history panel — one per dab, which is the
/// implementation showing through.
///
/// Sent the way the interface sends it: neither of these tools opens a live
/// gesture, so the ViewModel holds the whole stroke and delivers it once when
/// the pointer comes up.
#[test]
fn a_magnify_gesture_is_one_undo_step() {
    for tool in [ToolKind::Inflar, ToolKind::Pincar] {
        let mut document = sphere();
        // One stroke first, so that the layer's mirror already points where
        // this gesture wants it. Pointing it is an edit of its own and lands
        // in the history beside the stroke that asked for it — true of every
        // field verb, and nothing to do with the grouping measured here.
        stroke(&mut document, tool);
        let before = SculptModel::history(&document).depth;

        // Long enough to lay down several dabs at this brush's spacing, so
        // that one entry is a claim about the group rather than about there
        // having been only one call.
        let drawn: Vec<GestureSample> = (0..=20)
            .map(|step| {
                let t = step as f32 / 20.0;
                GestureSample {
                    position: [AT[0] + (t - 0.5) * 0.9, AT[1], AT[2]],
                    pressure: 1.0,
                    time: t,
                }
            })
            .collect();

        document.begin_gesture();
        let outcome = document
            .apply_stroke(tool, brush(), &drawn, [false; 3])
            .expect("the stroke was refused");
        document.end_gesture();
        assert!(outcome.changed, "{tool:?} reported no change");

        let after = SculptModel::history(&document).depth;
        assert_eq!(
            after - before,
            1,
            "{tool:?} left {} history entries for one stroke",
            after - before
        );
        assert!(
            SculptModel::undo(&mut document).expect("undo"),
            "{tool:?} left nothing to undo"
        );
        assert_eq!(
            SculptModel::history(&document).depth,
            before,
            "one undo did not take the whole {tool:?} stroke back"
        );
    }
}

// -- the shelf ---------------------------------------------------------------

/// Pinch is on the shelf for a field layer, which it was not before.
#[test]
fn pinch_is_offered_on_a_field() {
    let offered = ToolKind::for_representation(Representation::Sdf);
    assert!(
        offered.contains(&ToolKind::Pincar),
        "Pinçar is still missing from the field's shelf"
    );
    assert_eq!(
        ToolKind::Pincar.verb_on(Representation::Sdf),
        Some("clay_layer_magnify_surface (negative strength)"),
        "the capability table and the shelf disagree about what Pinçar does \
         on a field"
    );
}
