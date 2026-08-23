//! The Move brush on a mesh layer, and whether its falloff is topological.
//!
//! Two questions, and they have different answers.
//!
//! **Is Move available?** Yes, and it always was: the capability table binds
//! `ToolKind::Mover` to `clay_mesh_sculptor_stamp (GRAB)`. What it was not was
//! *reachable* — a mesh layer could not be sculpted with the pointer at all
//! until the sculptor was armed on selection (`to_mesh.rs`), so no mesh verb
//! could be used, Move included.
//!
//! **Is Move Topological available?** Also yes, and it is the only kind there
//! is. The engine's mesh brush descriptor carries a `geodesic` flag —
//! "a brush on the upper lip must not drag the chin through a closed mouth" —
//! and it defaults on. This application sets it for every verb except Flatten
//! and Scrape, which mean "everything under this disc". So Move on a mesh is
//! ZBrush's Move Topological, and there is no plain Euclidean Move to choose.
//!
//! `clay_item_volume_move_topological` is a different thing and is not this:
//! it takes an *item carrying a volume* and is refused on anything else, so it
//! belongs to the SDF side.
//!
//! The second question is the one worth a measurement, and the engine states
//! the experiment: two parts close in space and far along the material. A
//! horseshoe is that — its tips are 0.71 apart through the air and 2.36 apart
//! around the arc.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Combine, CombineSettings, Direction, GestureSample, Representation, SculptModel,
    ToolKind, Verbs,
};

/// Where the arc's two ends sit, in the plane it is drawn in.
const NEAR_TIP: [f32; 3] = [0.5, 0.0, 0.0];
const FAR_TIP: [f32; 3] = [0.0, 0.0, 0.5];

/// A horseshoe, meshed.
///
/// Three quarters of a circle of radius 0.5, so the two ends are
/// `sqrt(0.5) = 0.707` apart through the air and `0.75 * 2 * pi * 0.5 = 2.36`
/// apart around the material. A drag at radius 1.0 reaches the far tip through
/// space and cannot reach it along the surface, which is the whole difference
/// between the two falloffs.
fn horseshoe() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy).ok()?;
    // The stroke default displaces a surface along its normal, and an empty
    // layer has none. Additive is what deposits into nothing.
    document.set_combine(CombineSettings {
        op: Combine::Add,
        ..CombineSettings::for_strokes()
    });
    let brush = BrushSettings {
        size: 0.22,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    for step in 0..=48 {
        let t = step as f32 / 48.0;
        let angle = -t * std::f32::consts::TAU * 0.75;
        document
            .apply_stroke(
                ToolKind::Padrao,
                brush,
                &[GestureSample {
                    position: [angle.cos() * 0.5, 0.0, angle.sin() * 0.5],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .ok()?;
    }
    document.convert_layer(Direction::SdfToMesh, 0.02, 0).ok()?;
    Some(document)
}

/// The vertices within `reach` of a point, and where they are.
fn near(positions: &[[f32; 3]], point: [f32; 3], reach: f32) -> Vec<usize> {
    positions
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            let d: f32 = (0..3).map(|i| (p[i] - point[i]).powi(2)).sum();
            d.sqrt() <= reach
        })
        .map(|(i, _)| i)
        .collect()
}

/// How far the listed vertices moved, at the most.
fn furthest(before: &[[f32; 3]], after: &[[f32; 3]], which: &[usize]) -> f32 {
    which
        .iter()
        .map(|&i| {
            let d: f32 = (0..3)
                .map(|axis| (after[i][axis] - before[i][axis]).powi(2))
                .sum();
            d.sqrt()
        })
        .fold(0.0, f32::max)
}

#[test]
fn the_capability_table_binds_move_to_a_mesh_verb() {
    let verbs: Verbs = ToolKind::Mover.verbs();
    assert_eq!(
        verbs.on(Representation::Mesh),
        Some("clay_mesh_sculptor_stamp (GRAB)"),
        "Move claims no mesh binding, so the shelf would not offer it"
    );
    // And the shelf is built from the same table, so a mesh layer offers it.
    assert!(
        ToolKind::for_representation(Representation::Mesh).contains(&ToolKind::Mover),
        "the mesh shelf does not offer Move"
    );
}

#[test]
fn move_drags_a_mesh_layers_vertices() {
    let Some(mut document) = horseshoe() else {
        return;
    };
    let before = document.visible_mesh_geometry().0;
    assert!(!before.is_empty(), "the horseshoe meshed to nothing");

    // Two samples, because a drag is a motion: Grab takes its direction from
    // the travel between stamps, and one position says nothing.
    let outcome = document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                size: 0.3,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[
                GestureSample {
                    position: NEAR_TIP,
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [NEAR_TIP[0], NEAR_TIP[1] + 0.4, NEAR_TIP[2]],
                    pressure: 1.0,
                    time: 1.0,
                },
            ],
            [false; 3],
        )
        .expect("Move was refused on a mesh layer");
    assert!(outcome.changed, "Move moved nothing");

    let after = document.visible_mesh_geometry().0;
    assert_eq!(
        after.len(),
        before.len(),
        "Move changed the vertex count; no mesh verb may change topology"
    );
    let moved = furthest(&before, &after, &near(&before, NEAR_TIP, 0.3));
    assert!(
        moved > 0.02,
        "the vertices under the brush moved {moved}, which is not a drag"
    );
}

/// The falloff is measured along the material, not through the air.
///
/// This is what "Move Topological" *means*, and the only way to see it is a
/// form with parts close in space and far along the surface. The brush reaches
/// 1.0 — past the 0.71 between the tips through the air, and well short of the
/// 2.36 around the arc. A Euclidean falloff would drag the far tip with the
/// near one; a surface walk cannot get there.
#[test]
fn move_does_not_drag_what_is_near_in_space_and_far_along_the_surface() {
    let Some(mut document) = horseshoe() else {
        return;
    };
    let before = document.visible_mesh_geometry().0;

    let near_tip = near(&before, NEAR_TIP, 0.25);
    let far_tip = near(&before, FAR_TIP, 0.25);
    assert!(
        near_tip.len() > 20 && far_tip.len() > 20,
        "the fixture does not have two tips to compare ({} and {} vertices)",
        near_tip.len(),
        far_tip.len()
    );

    document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                // Past the gap through the air, far short of the way around.
                size: 1.0,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[
                GestureSample {
                    position: NEAR_TIP,
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [NEAR_TIP[0], NEAR_TIP[1] + 0.6, NEAR_TIP[2]],
                    pressure: 1.0,
                    time: 1.0,
                },
            ],
            [false; 3],
        )
        .expect("Move was refused");

    let after = document.visible_mesh_geometry().0;
    let dragged = furthest(&before, &after, &near_tip);
    let spared = furthest(&before, &after, &far_tip);

    assert!(
        dragged > 0.05,
        "the tip under the brush moved only {dragged}"
    );
    assert!(
        spared < dragged * 0.2,
        "the far tip moved {spared} while the near one moved {dragged}. They \
         are 0.71 apart through the air and 2.36 apart around the arc, so a \
         brush reaching 1.0 drags both only if the falloff is Euclidean — \
         this is the topological weighting failing to be topological"
    );
}

#[test]
fn a_move_on_a_mesh_can_be_taken_back() {
    let Some(mut document) = horseshoe() else {
        return;
    };
    let before = document.visible_mesh_geometry().0;

    document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                size: 0.3,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[
                GestureSample {
                    position: NEAR_TIP,
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [NEAR_TIP[0], NEAR_TIP[1] + 0.4, NEAR_TIP[2]],
                    pressure: 1.0,
                    time: 1.0,
                },
            ],
            [false; 3],
        )
        .expect("Move was refused");
    let moved = document.visible_mesh_geometry().0;
    let shifted = furthest(&before, &moved, &(0..before.len()).collect::<Vec<_>>());
    assert!(
        shifted > 0.01,
        "nothing to undo: the stroke moved {shifted}"
    );

    assert!(document.undo().expect("undo"), "undo reported no move");
    let back = document.visible_mesh_geometry().0;
    let worst = furthest(&before, &back, &(0..before.len()).collect::<Vec<_>>());
    assert!(
        worst < 1e-4,
        "undo left the mesh {worst} from where it started; a mesh gesture is \
         recorded as vertex deltas and has to come back exactly"
    );
}
