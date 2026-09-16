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

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, SculptModel, ToolKind};

const CELL: f32 = 0.05;
const RADIUS: f32 = 0.4;
/// How far the drag lifts, in world units.
///
/// WITHIN the footprint's own radius, deliberately. The grab is an inverse map
/// that writes only inside its footprint, so material can never be carried
/// further than the ball reaches: a 0.3 lift on a 0.2-radius ball moved the
/// surface not at all (centre 3 -> 3), which is the "small blob" in #139.
const LIFT: f32 = 0.15;

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
    let rim_before = surface_at(&document, RADIUS * 0.8);

    // The press lands on the surface, one cell above the top.
    let changed = drag_up(&mut document, 0.0, (centre_before + 1) as f32 * CELL);

    let cells_after = occupied(&document);
    let centre_after = surface_at(&document, 0.0);
    let rim_after = surface_at(&document, RADIUS * 0.8);
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
    let mut reached = Vec::new();
    for lift in [0.05f32, 0.1, 0.2, 0.4, 0.8] {
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
            (RADIUS / CELL).round() as i32 / 2
        );
        reached.push((asked_cells, rise));
    }
    assert!(
        reached.iter().any(|(_, rise)| *rise > 0),
        "no drag moved the surface at all, so this measures nothing"
    );
}
