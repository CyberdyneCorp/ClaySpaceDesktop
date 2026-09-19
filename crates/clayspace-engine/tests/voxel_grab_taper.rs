//! Whether a grid drag PULLS or shoves.
//!
//! ClayCore's grab is an inverse map: for each cell in the footprint it asks
//! where that material came from, `p - displacement * w`, where `w` is the
//! region weight at that cell (`kernel/deform.h`). So the falloff does not only
//! decide which cells are written — it shapes the pull itself. A weight of 1
//! across the ball translates the whole neighbourhood rigidly; a weight that
//! falls to the rim drags the surface into a bulge, which is what a Move brush
//! is for.
//!
//! Measured on the surface rather than on the parameters: the topmost occupied
//! cell over the drag's centre against the topmost over a column near the rim.
//!
//! **This file was armed against ClayCore 0.117.0 and it fired on the move to
//! v0.120.0.** `clay_voxel_sculpt_grab` had been passing the falloff enum
//! where a curve index was expected, so every name delivered the next one's
//! curve and the `Constant` this application asks for arrived as Linear —
//! which is to say the drag tapered by accident. v0.120.0 makes the names
//! honest, an honest `Constant` weights 1 across the whole ball, and the
//! assertion below caught the block being shoved: the rim rose 5 cells against
//! the centre's 6, where it had risen 1 against 4.
//!
//! The answer was to ask for the curve this brush always wanted rather than to
//! loosen the assertion — see `dragging_footprint` in `clayspace-engine`, which
//! is where the reasoning and the before-and-after figures live. The margins
//! chosen below stay as they are: they were picked so that a rigid pull fails
//! by a margin rather than at a bound, and that is still what they are for.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

const CELL: f32 = 0.05;
const RADIUS: f32 = 0.4;
/// Where the rim probe stands, as a fraction of the drag's radius.
const RIM: f32 = 0.9;
/// How far the drag lifts, in world units.
///
/// WITHIN the footprint's own radius, deliberately. The grab is an inverse map
/// that writes only inside its footprint, so material can never be carried
/// further than the ball reaches: a 0.3 lift on a 0.2-radius ball moved the
/// surface not at all (centre 3 -> 3), which is the "small blob" in #139.
const LIFT: f32 = 0.3;
// 0.3, not 0.15. This test was the tripwire for ClayCore 0.117.0, where a
// Constant grab stops tapering, and it had to fail on a rigid pull by a margin
// rather than at its bound. At 0.15 the centre rose 2 cells and the rim 1 — the
// assertion `rim * 2 <= centre` held exactly, and it held that way twice, first
// at an even footprint span and again at the odd one. A cell of rounding either
// way would have flipped it without the pull changing at all.

fn slab() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document.add_voxel_layer("Voxels", CELL).expect("a grid");
    let brush = BrushSettings {
        size: 0.3,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    for step in 0..25 {
        let t = step as f32 / 24.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                brush,
                &[GestureSample {
                    position: [(t - 0.5) * 1.8, 0.0, 0.0],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("deposit");
    }
    document
}

/// The highest occupied cell in the column at `x`, in cells.
fn surface_at(document: &ClayDocument, x: f32) -> i32 {
    let (_, reader) = document
        .document()
        .voxel_reader("Voxels")
        .expect("the grid reads back");
    let column = (x / CELL).round() as i32;
    let mut top = i32::MIN;
    for y in -40..40 {
        if reader
            .get([column, y, 0])
            .expect("a cell reads back")
            .is_some()
        {
            top = top.max(y);
        }
    }
    top
}

/// Occupied cells in the whole grid.
fn occupied(document: &ClayDocument) -> usize {
    let (_, reader) = document
        .document()
        .voxel_reader("Voxels")
        .expect("the grid reads back");
    reader.occupied_count().expect("a count")
}

/// A drag anchored ON THE SURFACE, which is where a press lands.
///
/// Anchored at the slab's middle instead, the only cells above its top sit at
/// the ball's rim where the weight is ~0, so they sample themselves and the
/// surface cannot rise — measured, centre 3 -> 3 for both a 0.3 and a 0.15
/// lift. That was the fixture's fault, not the brush's.
fn drag_up(document: &mut ClayDocument, at: f32, from_height: f32) -> bool {
    let samples: Vec<GestureSample> = (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [at, from_height + t * LIFT, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    SculptModel::begin_gesture(document);
    let outcome = document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                size: RADIUS,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("the drag was refused");
    SculptModel::end_gesture(document);
    outcome.changed
}

#[test]
fn a_grid_drag_pulls_a_bulge_rather_than_shoving_a_block() {
    let mut document = slab();
    let cells_before = occupied(&document);
    let centre_before = surface_at(&document, 0.0);
    // Near the rim of the footprint: inside it, so it is dragged, but where a
    // tapered pull is weakest.
    //
    // At 0.9 of the radius, not 0.8. This test was the tripwire for ClayCore
    // 0.117.0, where a Constant grab stops tapering and pulls rigidly; at 0.8
    // it passed exactly at its bound (rim +1 against centre +2), so a change in
    // how cells round could flip it either way without the pull changing at
    // all. Further out the taper is weaker and the margin is real.
    let rim_before = surface_at(&document, RADIUS * RIM);

    // The press lands on the surface, one cell above the top.
    let changed = drag_up(&mut document, 0.0, (centre_before + 1) as f32 * CELL);

    let cells_after = occupied(&document);
    let centre_after = surface_at(&document, 0.0);
    let rim_after = surface_at(&document, RADIUS * RIM);
    let (centre_rise, rim_rise) = (centre_after - centre_before, rim_after - rim_before);
    eprintln!(
        "changed {changed}; cells {cells_before} -> {cells_after}; \
         centre {centre_before} -> {centre_after} (+{centre_rise}), \
         rim {rim_before} -> {rim_after} (+{rim_rise})"
    );

    assert!(changed, "the drag reported no edit at all");
    assert!(
        centre_rise > 0,
        "the drag lifted nothing at its own centre: {centre_before} -> {centre_after}"
    );
    // The property: a pull tapers. Rigid translation raises the rim as much as
    // the centre, which is a block being shoved rather than clay being drawn.
    assert!(
        rim_rise * 2 <= centre_rise,
        "the rim rose {rim_rise} against the centre's {centre_rise}, so the \
         drag translated its whole ball instead of tapering to the rim"
    );
}

/// How far a drag reaches, against how far it was asked to go.
///
/// The grab is an inverse map with finite support: a cell outside the footprint
/// is never written, and inside it the sample comes from `p - displacement * w`
/// with `w` falling to 0 at the rim. So the bulge cannot outrun the ball,
/// however far the pointer travels — which is the "small blob" a sculptor sees
/// when the brush is small and the drag is long (#139).
#[test]
fn a_drag_cannot_outrun_its_own_radius() {
    let reached: Vec<(i32, i32)> = [0.05f32, 0.1, 0.2, 0.4, 0.8]
        .iter()
        .map(|l| reach(*l))
        .collect();
    assert!(
        reached.iter().any(|(_, rise)| *rise > 0),
        "no drag moved the surface at all, so this measures nothing"
    );
}

/// One drag straight up off a fresh slab: what it asked for and what it got,
/// both in cells.
fn reach(lift: f32) -> (i32, i32) {
    let mut document = slab();
    let top = surface_at(&document, 0.0);
    let samples: Vec<GestureSample> = (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [0.0, (top + 1) as f32 * CELL + t * lift, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    SculptModel::begin_gesture(&mut document);
    document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                size: RADIUS,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("the drag was refused");
    SculptModel::end_gesture(&mut document);
    let rise = surface_at(&document, 0.0) - top;
    let asked_cells = (lift / CELL).round() as i32;
    eprintln!(
        "asked {lift:.2} ({asked_cells} cells), surface rose {rise} cells, \
         radius is {} cells",
        // `2 * round(size / cell) + 1` across, so `round(size / cell)` either side.
        (RADIUS / CELL).round() as i32
    );
    (asked_cells, rise)
}

/// The regression for the v0.120.0 pin move, read at the centre instead of at
/// the rim.
///
/// The test above it reads the taper across the ball; this one reads what the
/// taper COSTS the drag, which is the half a sculptor feels. A pull weighted 1
/// everywhere carries the column under the pointer the whole way it was asked
/// to go until the ask outruns the ball — a block sliding, not clay being
/// drawn. A pull that falls to the rim cannot: the cells above the anchor
/// sample from progressively further down the bulge, and the surface arrives
/// short.
///
/// Held at a drag of exactly the brush's own radius, which is where the two
/// readings are furthest apart and where neither sits on a bound. Measured on
/// this fixture: **4 cells against the 8 asked** with the tapering pull this
/// application asks `dragging_footprint` for, and **8 of 8** with the rigid one
/// v0.120.0 gives a caller that asks for `Constant` by name. Two thirds is
/// between them with two cells of margin either side.
///
/// It is a second reading of one property rather than a second property, and
/// that is deliberate: a margin picked at one rounding can flip without the
/// pull changing at all, which is the trap the two comments above record
/// falling into. Two readings of the pull fail together or not at all.
#[test]
fn a_drag_as_long_as_its_brush_arrives_short_of_the_ask() {
    let (asked, rise) = reach(RADIUS);
    assert!(
        rise > 0,
        "the drag lifted nothing at all, so there is no reach to measure"
    );
    assert!(
        rise * 4 <= asked * 3,
        "a drag of {asked} cells — the brush's own radius — carried the \
         surface {rise} cells, which is more than two thirds of the way. A \
         grab that arrives where it was asked to is translating its ball \
         rigidly rather than drawing a bulge out of it: that is what an \
         honest `Constant` falloff does since ClayCore v0.120.0, and it is why \
         a grid drag asks for the taper by name"
    );
}
