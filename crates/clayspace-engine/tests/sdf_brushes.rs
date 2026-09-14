//! Every SDF brush: does it work, does it take a sign, does it mirror.
//!
//! Reported as "Smooth has no symmetry on SDF", and true — but it was five
//! brushes rather than one, and there was a second fault pointing the other
//! way.
//!
//! Symmetry on a field is the **layer mirror**, which reflects a layer's
//! *items*. Six tools bypassed `stroke_sdf` and so were never handed the axes
//! at all, and of those:
//!
//! - **Five rewrite the field** rather than adding an item — the surface drag,
//!   both relaxes and both planes. The mirror cannot reach those even when it
//!   is on: measured, a relax with X mirrored took the surface under the
//!   stroke from 1.1467 to 1.1409 and left its reflection at 1.1467 exactly.
//!   Their strokes are reflected instead, the way a mesh's and a grid's are.
//! - **One adds items** — the snakehook — so the mirror does reach it. Its
//!   fault was the opposite: never *setting* the mirror, it inherited whatever
//!   was last asked for, and the starting form turns X on. A snakehook with
//!   symmetry switched **off** came out on both sides.
//!
//! On the sign: it belongs to the verbs that have one. Depositing has an
//! opposite and so does planing; smoothing does not, and neither does the
//! direction of a drag — those are the same rules the mesh brushes follow.
//!
//! # Read a brush with a field query, not with a pick
//!
//! Every assertion in this file measures the surface with
//! [`SculptModel::pick`], which marches. That is the right instrument for
//! *"did the surface move where a sculptor is looking"* and the wrong one for
//! *"did this verb change"*, and the difference cost two hours at the v0.84.0
//! pin.
//!
//! What happened: ClayCore #447 changed picking to walk the brick cache
//! analytically rather than sphere-trace it, and a resting unit sphere went
//! from reading 1.006707 to 1.000296. Two of this file's tests then failed,
//! and the first diagnosis — taken with a *second* pick reading — was that the
//! brush had got stronger. Measured with `clay_eval_points` instead, over 2,197
//! points about the stroke, **the field moves 0.0400000 on both pins for one
//! stroke and 0.1198471 on both for four dabs, to the last digit.** The verb is
//! bit-identical across the release. The pick was the whole difference.
//!
//! The tell was there and was read backwards: on v0.78.0 the pick returned
//! `1.0350003` before and after, *exactly*. An instrument giving bit-identical
//! readings either side of a four-hundredths field change is quantising, not
//! reporting a weak effect.
//!
//! So: if a threshold in this file ever needs re-deriving, derive it against a
//! no-op case and **read that case with a field query rather than a pick** —
//! otherwise the number measures whichever marcher the pinned engine happens
//! to ship.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, Representation, SculptModel, ToolKind};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far the surface stands from the centre along a direction.
fn reach(document: &ClayDocument, direction: [f32; 3]) -> f32 {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    SculptModel::pick(document, unit.map(|c| c * 4.0), unit.map(|c| -c))
        .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
        .unwrap_or(f32::NAN)
}

const AT: [f32; 3] = [0.6, 0.0, 0.8];
const MIRRORED: [f32; 3] = [-0.6, 0.0, 0.8];

/// The four verbs that sample a region rather than stamp into it.
///
/// The engine adapter's own grouping — "Suavizar, Relaxar, Planar and Polir do
/// not stamp: they sample a region" — and they are grouped here for the reason
/// they are grouped there: each averages toward something the neighbourhood
/// already is.
///
/// They need something to smooth. A pristine sphere is already the smoothest
/// thing there is, so asking one of these to move it is asking it to do the
/// job it exists *not* to do — and it took ClayCore v0.84.0 to make that
/// visible. Before it, a pick against the resting sphere read **1.006707**
/// where the surface is at 1.0; after it walks the brick cache analytically
/// rather than sphere-tracing it, the same pick reads **1.000296**. A smooth
/// of a pristine sphere then correctly moves it 3e-4, under the 1e-3 this file
/// asks of a brush — and the reason it cleared that bar before was six
/// thousandths of measurement error, not six thousandths of clay.
///
/// So these are given a bump to take out, which is what they are for. Measured
/// across the pin, on the fixture below: a dab 0.037 proud, and the smoothing
/// took back **0.0084 on v0.78.0 and 0.0109 on v0.84.0** — stronger, not
/// weaker.
const SMOOTHING_BRUSHES: [ToolKind; 4] = [
    ToolKind::Suavizar,
    ToolKind::Relaxar,
    ToolKind::Planar,
    ToolKind::Polir,
];

/// A sphere with something on it worth smoothing, at `where_`.
fn bumped_at(where_: [f32; 3]) -> ClayDocument {
    let mut document = sphere();
    let samples = [GestureSample {
        position: where_,
        pressure: 1.0,
        time: 0.0,
    }];
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.12,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("the fixture's own dab was refused");
    document
}

/// A bump under the stroke.
fn bumped() -> ClayDocument {
    bumped_at(AT)
}

/// A bump where the *mirror* is expected to land, which is what the symmetry
/// test needs a smoothing verb to have something to do about.
fn bumped_mirrored() -> ClayDocument {
    bumped_at(MIRRORED)
}

fn stroke(document: &mut ClayDocument, tool: ToolKind, invert: bool, symmetry: [bool; 3]) -> bool {
    let samples: Vec<GestureSample> = (0..=6)
        .map(|step| {
            let t = step as f32 / 6.0;
            GestureSample {
                position: [AT[0] + (t - 0.5) * 0.2, AT[1], AT[2]],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size: 0.25,
                intensity: 0.9,
                invert,
                ..BrushSettings::default()
            },
            &samples,
            symmetry,
        )
        .expect("the stroke was refused")
        .changed
}

/// The brushes that move the surface. Máscara paints the freeze and Trim is a
/// shape drawn on the view frame, so neither is a stroke that displaces clay.
const SURFACE_BRUSHES: [ToolKind; 12] = [
    ToolKind::Padrao,
    ToolKind::Inflar,
    ToolKind::Suavizar,
    ToolKind::Mover,
    ToolKind::MoverTopologico,
    ToolKind::Planar,
    ToolKind::Camada,
    ToolKind::Puxar,
    ToolKind::Polir,
    ToolKind::Relaxar,
    ToolKind::Argila,
    ToolKind::Vinco,
];

#[test]
fn the_surface_brushes_are_the_ones_the_shelf_offers() {
    // So the list above cannot drift from the vocabulary it stands for.
    let offered = ToolKind::for_representation(Representation::Sdf);
    for tool in SURFACE_BRUSHES {
        assert!(
            offered.contains(&tool),
            "{tool:?} is not offered on a field"
        );
    }
    let missing: Vec<&str> = offered
        .iter()
        .filter(|tool| !SURFACE_BRUSHES.contains(tool))
        .map(|tool| tool.label())
        .collect();
    assert_eq!(
        missing,
        vec!["Máscara", "Trim"],
        "the shelf offers a field brush this file says nothing about"
    );
}

#[test]
fn every_surface_brush_moves_the_surface() {
    for tool in SURFACE_BRUSHES {
        // A smoothing verb is asked to flatten a bump; every other verb is
        // asked to mark a resting sphere. Same assertion, and each on a
        // surface where it has something to do.
        let smoothing = SMOOTHING_BRUSHES.contains(&tool);
        let mut document = if smoothing { bumped() } else { sphere() };
        let rest = reach(&document, AT);
        let changed = stroke(&mut document, tool, false, [false; 3]);
        let after = reach(&document, AT);
        assert!(changed, "{tool:?} reported no change");
        assert!(
            (after - rest).abs() > 1e-3,
            "{tool:?} reported a change and left the surface at {after} from \
             {rest}"
        );
    }
}

#[test]
fn every_surface_brush_mirrors_when_it_is_asked_to() {
    // The reported fault, and it was five brushes rather than one.
    for tool in SURFACE_BRUSHES {
        // A smoothing verb needs a bump on the *far* side to take out, or the
        // mirror has nothing to show for itself — see `SMOOTHING_BRUSHES`.
        let smoothing = SMOOTHING_BRUSHES.contains(&tool);
        let mut document = if smoothing {
            bumped_mirrored()
        } else {
            sphere()
        };
        let rest = reach(&document, MIRRORED);
        stroke(&mut document, tool, false, [true, false, false]);
        let there = reach(&document, MIRRORED);
        assert!(
            (there - rest).abs() > 1e-3,
            "{tool:?} with X symmetry left the far side at {there} from \
             {rest}, so the symmetry buttons do nothing for it"
        );
    }
}

#[test]
fn the_two_sides_come_out_the_same() {
    // Mirrored, not merely touched. The planes are given room for the bake's
    // own asymmetry — a flatten fits a plane to the region it finds, and the
    // two regions are reflections rather than copies.
    let _ = sphere();
    for tool in SURFACE_BRUSHES {
        let mut document = sphere();
        stroke(&mut document, tool, false, [true, false, false]);
        let (here, there) = (reach(&document, AT), reach(&document, MIRRORED));
        assert!(
            (here - there).abs() < 0.01,
            "{tool:?} left {here} where the stroke was made and {there} at its \
             mirror"
        );
    }
}

#[test]
fn a_brush_does_not_mirror_when_it_is_not_asked_to() {
    // The fault pointing the other way, and the one that would have gone on
    // being invisible: the tools that bypassed `stroke_sdf` never *set* the
    // mirror, so they inherited whatever was last asked for. The starting form
    // turns X on, so a snakehook with symmetry switched off came out on both
    // sides — measured, +x and -x both at 1.4625.
    let base = sphere();
    let rest = reach(&base, MIRRORED);
    for tool in SURFACE_BRUSHES {
        let mut document = sphere();
        stroke(&mut document, tool, false, [false; 3]);
        let there = reach(&document, MIRRORED);
        assert!(
            (there - rest).abs() < 1e-3,
            "{tool:?} with symmetry off still reached the far side, leaving it \
             at {there} from {rest}"
        );
    }
}

// -- the sign ----------------------------------------------------------------

/// The brushes with an opposite, and what holding the invert key means.
///
/// Depositing has one and planing has one. Smoothing does not — an inverted
/// smooth is not a thing either reference offers, and sharpening is a
/// different verb rather than a smooth turned over. Nor does a drag: its
/// direction *is* its sign, and inverting it is dragging the other way.
/// Argila and Vinco are here too, and they are the pair the engine names: the
/// relief and the incise are each other's opposite, so building up clay
/// inverts to cutting in and a crease inverts to the ridge it would have cut.
/// `sdf_named_brushes.rs` measures both directions.
const SIGNED: [ToolKind; 7] = [
    ToolKind::Padrao,
    ToolKind::Inflar,
    ToolKind::Camada,
    ToolKind::Planar,
    ToolKind::Polir,
    ToolKind::Argila,
    ToolKind::Vinco,
];

#[test]
fn the_depositing_brushes_take_material_away_when_inverted() {
    let base = sphere();
    let rest = reach(&base, AT);
    for tool in [
        ToolKind::Padrao,
        ToolKind::Inflar,
        ToolKind::Camada,
        // Argila is relief like the other three, so it inverts to the incise —
        // Vinco is not here because it is the incise *upright*, and its
        // inverse raises rather than cuts.
        ToolKind::Argila,
    ] {
        let mut up = sphere();
        let mut down = sphere();
        stroke(&mut up, tool, false, [false; 3]);
        stroke(&mut down, tool, true, [false; 3]);
        let (raised, cut) = (reach(&up, AT), reach(&down, AT));
        assert!(
            raised > rest + 1e-3,
            "{tool:?} did not build up: {raised} from {rest}"
        );
        assert!(
            cut < rest - 1e-3,
            "{tool:?} held inverted left the surface at {cut} from {rest}, so \
             it added clay where the sculptor asked to take it away"
        );
    }
}

#[test]
fn planing_inverted_fills_instead_of_cutting() {
    // The other half of a planing tool, and the one thing "negative planing"
    // can mean: cut-only shaves the high ground and must not fill the dents it
    // is meant to reveal; fill-only does exactly the opposite. The engine has
    // had a mode for each all along and only one was ever asked for.
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let bump = [0.6f32, 0.0, 0.8];
    let dent = [0.6f32, 0.35, 0.7];
    let dented = || -> ClayDocument {
        let mut document =
            ClayDocument::new(BackendPolicy::discover(None).expect("discover backends"))
                .and_then(ClayDocument::with_starting_form)
                .expect("a document with a starting form");
        // A bump, and beside it a dent, so a cut and a fill have different
        // work to do and can be told apart.
        for (at, invert) in [(bump, false), (dent, true)] {
            document
                .apply_stroke(
                    ToolKind::Padrao,
                    BrushSettings {
                        size: 0.22,
                        intensity: 0.9,
                        invert,
                        ..BrushSettings::default()
                    },
                    &[GestureSample {
                        position: at,
                        pressure: 1.0,
                        time: 0.0,
                    }],
                    [false; 3],
                )
                .expect("sculpt");
        }
        document
    };
    let _ = policy;
    let base = dented();
    let (was_bump, was_dent) = (reach(&base, bump), reach(&base, dent));

    for tool in [ToolKind::Planar, ToolKind::Polir] {
        for invert in [false, true] {
            let mut document = dented();
            let samples: Vec<GestureSample> = (0..=6)
                .map(|step| {
                    let t = step as f32 / 6.0;
                    GestureSample {
                        position: [0.6, t * 0.35, 0.8 - t * 0.1],
                        pressure: 1.0,
                        time: t,
                    }
                })
                .collect();
            document
                .apply_stroke(
                    tool,
                    BrushSettings {
                        size: 0.3,
                        intensity: 0.9,
                        invert,
                        ..BrushSettings::default()
                    },
                    &samples,
                    [false; 3],
                )
                .expect("the stroke was refused");
            let (bump_now, dent_now) = (reach(&document, bump), reach(&document, dent));
            if invert {
                assert!(
                    dent_now > was_dent + 1e-3,
                    "{tool:?} inverted left the hollow at {dent_now} from \
                     {was_dent}; filling it is the whole of what it is for"
                );
                assert!(
                    bump_now >= was_bump - 1e-4,
                    "{tool:?} inverted cut the high ground to {bump_now} from \
                     {was_bump}, which is the upright verb's job"
                );
            } else {
                assert!(
                    bump_now < was_bump - 1e-4,
                    "{tool:?} left the high ground at {bump_now} from \
                     {was_bump}"
                );
                assert!(
                    dent_now <= was_dent + 1e-4,
                    "{tool:?} filled the hollow to {dent_now} from {was_dent}, \
                     which a planing tool must not do — it is meant to reveal \
                     the dents, not close them"
                );
            }
        }
    }
}

#[test]
fn the_brushes_with_no_opposite_are_left_alone_by_the_key() {
    // Stated rather than left as an absence. An inverted smooth is not a thing
    // either reference offers, and a drag's direction already *is* its sign —
    // so holding the key over one of these does nothing, on purpose, and the
    // same rule the mesh brushes follow.
    let _ = sphere();
    for tool in SURFACE_BRUSHES {
        if SIGNED.contains(&tool) {
            continue;
        }
        let mut up = sphere();
        let mut held = sphere();
        stroke(&mut up, tool, false, [false; 3]);
        stroke(&mut held, tool, true, [false; 3]);
        assert!(
            (reach(&up, AT) - reach(&held, AT)).abs() < 1e-4,
            "{tool:?} answered to the invert key, which means it has an \
             opposite this file does not know about"
        );
    }
}
