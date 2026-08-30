//! Every tool the table offers, on every representation it offers it on.
//!
//! `ToolKind::verbs` is the one place that says where a tool applies, and the
//! shelf, the availability check, the diagnostics report and the tests all read
//! it. That is what keeps them from drifting — and it is only worth anything if
//! a row added to the table is a row that *works*: a declaration with no
//! binding behind it is a tool offered on the shelf that refuses when it is
//! used, which is the exact failure the table replaced.
//!
//! So this walks the table rather than a hand-written list. Adding a
//! representation to a tool's row and forgetting the arm in the dispatch fails
//! here, on the row that was added.
//!
//! Two tools are excluded, and both by a property of their own rather than by
//! name: Trim is not a stroke — its gesture is a shape drawn on the view frame
//! — and Máscara paints a freeze rather than moving anything, so "did the
//! surface change" is the wrong question for it. `masking.rs` asks the right
//! one.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, GestureSample, MaskModel, Representation, SculptModel, ToolKind,
};

/// Where each fixture is worked, and where every stroke here is made.
fn over(representation: Representation) -> [f32; 3] {
    match representation {
        // The top of the starting sphere, and of the mesh carried off it.
        Representation::Sdf | Representation::Mesh => [0.0, 0.0, 1.0],
        // The middle of the slab.
        Representation::Voxel => [0.0, 0.0, 0.0],
    }
}

/// A form of the given representation, with something in it to sculpt.
///
/// The mesh fixture is carried off a **field** rather than off the grid, and
/// that is a choice the verbs force. A grid marches to greedy quads, so every
/// vertex sits on a right angle — and Polir is "smooth gated by dihedral
/// angle", with the engine's own gate set tight on purpose, so on a blocky mesh
/// it correctly refuses to touch anything. Testing a binding against a fixture
/// its verb is designed to decline measures the fixture.
fn worked(representation: Representation) -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    match representation {
        Representation::Voxel => {
            let mut document = ClayDocument::new(policy).expect("a document");
            document
                .add_voxel_layer("Voxels", 0.04)
                .expect("add a grid");
            // A wobbling slab across the mirror plane, so the reshaping verbs
            // have curvature to bite on and the mirrored half has material.
            for step in 0..21 {
                let t = step as f32 / 20.0;
                document
                    .apply_stroke(
                        ToolKind::Padrao,
                        BrushSettings {
                            size: 0.25,
                            intensity: 0.9,
                            ..BrushSettings::default()
                        },
                        &[GestureSample {
                            position: [(t - 0.5) * 1.6, (t * 9.0).sin() * 0.08, 0.0],
                            pressure: 1.0,
                            time: t,
                        }],
                        [false; 3],
                    )
                    .expect("deposit");
            }
            document
        }
        field => {
            let mut document = ClayDocument::new(policy)
                .and_then(ClayDocument::with_starting_form)
                .expect("a document with a starting form");
            // A ridge across the top, so the planing and smoothing verbs have
            // something to plane and smooth.
            for step in 0..7 {
                let t = step as f32 / 6.0;
                document
                    .apply_stroke(
                        ToolKind::Padrao,
                        BrushSettings {
                            size: 0.2,
                            intensity: 1.0,
                            ..BrushSettings::default()
                        },
                        &[GestureSample {
                            position: [(t - 0.5) * 0.5, 0.0, 1.0],
                            pressure: 1.0,
                            time: t,
                        }],
                        [false; 3],
                    )
                    .expect("deposit");
            }
            if field == Representation::Mesh {
                document
                    .convert_layer_in_place(
                        clayspace_model::conversion::Direction::SdfToMesh,
                        MESH_CELL,
                        0,
                    )
                    .expect("march the field into triangles");
                assert_eq!(
                    document.active_representation(),
                    Representation::Mesh,
                    "the conversion did not land"
                );
            }
            document
        }
    }
}

/// How finely the mesh fixture is marched.
///
/// Coarse: this walks every tool on every representation and then again
/// through a mask, and the marching is what the run costs.
const MESH_CELL: f32 = 0.05;

/// The tools that legitimately change nothing on these fixtures.
///
/// Three, and each is an honest absence rather than a dead binding. Pintar
/// paints the colour the material already carries unless a colour is chosen,
/// and Borrar drags existing colour along the stroke — over a uniformly
/// coloured form there is nothing to drag; `brush_colour.rs` is where the
/// colour question is actually asked. Preencher closes pockets, and these
/// fixtures have none; `voxel_tools.rs` gives it perforated material.
fn changes_nothing_here(tool: ToolKind) -> bool {
    tool.writes_colour() || tool == ToolKind::Preencher
}

/// The stroke every tool is given: a short drag across the material.
///
/// One shape for all of them, because the question is whether the *binding*
/// lands rather than whether a particular gesture suits a particular brush.
fn drag(document: &mut ClayDocument, tool: ToolKind, representation: Representation) -> bool {
    let at = over(representation);
    let samples: Vec<GestureSample> = (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [at[0] + (t - 0.5) * 0.5, at[1], at[2]],
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
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .unwrap_or_else(|e| {
            panic!(
                "{} on a {} layer: {e}",
                tool.label(),
                representation.label()
            )
        })
        .changed
}

/// Whether a tool is one this file strokes at all.
fn strokes(tool: ToolKind) -> bool {
    tool.is_stroke_tool() && !tool.is_mask_tool()
}

#[test]
fn every_declared_pair_lands() {
    let mut missed: Vec<String> = Vec::new();
    for representation in Representation::ALL {
        for tool in ToolKind::for_representation(representation) {
            if !strokes(tool) {
                continue;
            }
            let mut document = worked(representation);
            if !drag(&mut document, tool, representation) && !changes_nothing_here(tool) {
                missed.push(format!("{} on {}", tool.label(), representation.label()));
            }
        }
    }
    assert!(
        missed.is_empty(),
        "the table offers these and they changed nothing: {}",
        missed.join(", ")
    );
}

#[test]
fn a_tool_the_table_does_not_offer_is_refused_rather_than_run() {
    // The other half. A tool absent from a row must not fall through to some
    // neighbouring verb — the catch-all arm that did exactly that is what put
    // spheres under the planing tools.
    let mut refused = 0;
    for representation in Representation::ALL {
        for tool in ToolKind::ALL {
            if tool.exists_on(representation) {
                continue;
            }
            let mut document = worked(representation);
            let outcome = document.apply_stroke(
                tool,
                BrushSettings::default(),
                &[GestureSample {
                    position: over(representation),
                    pressure: 1.0,
                    time: 0.0,
                }],
                [false; 3],
            );
            assert!(
                outcome.is_err(),
                "{} is not offered on a {} layer and was applied anyway",
                tool.label(),
                representation.label()
            );
            refused += 1;
        }
    }
    assert!(refused > 0, "every tool is on every representation");
}

#[test]
fn a_frozen_region_resists_every_tool_a_grid_or_a_mesh_offers() {
    // The mask's own promise, walked over the same table. The field side is
    // `masking.rs`'s, which walks its own representation the same way; this is
    // the other two, where the newly bound rows live.
    //
    // Measured as a ratio against the same stroke unmasked, for the reason
    // that test gives: a tool that moves nothing anyway must not be mistaken
    // for one the mask stopped.
    let mut ignored: Vec<String> = Vec::new();
    for representation in [Representation::Voxel, Representation::Mesh] {
        for tool in ToolKind::for_representation(representation) {
            if !strokes(tool) {
                continue;
            }
            let mut free = worked(representation);
            let rest = drawn(&mut free);
            drag(&mut free, tool, representation);
            let unmasked = moved(representation, &rest, &drawn(&mut free));
            if unmasked <= 0.0 {
                continue;
            }

            let mut frozen = worked(representation);
            freeze(&mut frozen, representation);
            let before = drawn(&mut frozen);
            drag(&mut frozen, tool, representation);
            let masked = moved(representation, &before, &drawn(&mut frozen));
            if masked > unmasked * 0.25 {
                ignored.push(format!(
                    "{} on {} moved {masked:.4} through the mask against \
                     {unmasked:.4} without it",
                    tool.label(),
                    representation.label()
                ));
            }
        }
    }
    assert!(
        ignored.is_empty(),
        "these tools edited a frozen region: {}. A mask that most tools \
         respect is worse than none, because it invites reliance.",
        ignored.join("; ")
    );
}

/// Freezes the whole of where the stroke will land, with room to spare.
///
/// Painted along the stroke's own path rather than as one dab at its middle,
/// and with a brush twice the stroke's: the mask tool paints with a smooth
/// falloff by design, so a mask sized to the stroke freezes its middle and
/// half-freezes its ends — which reads as a mask half the tools ignore.
fn freeze(document: &mut ClayDocument, representation: Representation) {
    let at = over(representation);
    let path: Vec<GestureSample> = (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [at[0] + (t - 0.5) * 0.5, at[1], at[2]],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Mascara,
            BrushSettings {
                size: 0.5,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &path,
            [false; 3],
        )
        .expect("paint a mask");
    assert!(
        document.mask_state().is_active(),
        "the fixture froze nothing on a {} layer",
        representation.label()
    );
}

/// The vertices the viewport would upload.
fn drawn(document: &mut ClayDocument) -> Vec<[f32; 3]> {
    document.visible_mesh_geometry().0
}

/// How far the surface moved, in whatever units that representation has.
///
/// Two measures rather than one, and each is the representation's own nature
/// rather than a compromise between them.
///
/// A **mesh** has fixed topology by contract, so vertex `i` before and vertex
/// `i` after are the same vertex and the honest measure is how far the
/// furthest one travelled. Counting *how many* moved is the wrong instrument
/// here and measurably so: a mask painted with the tool's own smooth falloff
/// leaves its rim partly unfrozen, so a stroke through one still nudges
/// hundreds of rim vertices by a thousandth — measured, 583 of them moved,
/// while the furthest went 0.0019 against 0.0635 without the mask.
///
/// A **grid** is occupancy: a cell is set or it is not, its vertices appear
/// and disappear, and there is no vertex `i` to follow. So what is counted is
/// how much of what is drawn is somewhere it was not.
fn moved(representation: Representation, before: &[[f32; 3]], after: &[[f32; 3]]) -> f32 {
    match representation {
        Representation::Mesh => before
            .iter()
            .zip(after)
            .map(|(a, b)| {
                (0..3)
                    .map(|axis| (a[axis] - b[axis]).powi(2))
                    .sum::<f32>()
                    .sqrt()
            })
            .fold(0.0f32, f32::max),
        _ => {
            let held: std::collections::HashSet<[u32; 3]> =
                before.iter().map(|v| v.map(f32::to_bits)).collect();
            let fresh = after
                .iter()
                .filter(|v| !held.contains(&v.map(f32::to_bits)))
                .count();
            (after.len().abs_diff(before.len()) + fresh) as f32
        }
    }
}
