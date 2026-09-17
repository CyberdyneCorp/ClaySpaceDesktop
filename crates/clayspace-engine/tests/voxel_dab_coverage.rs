//! What a default grid stroke leaves behind: solid material, or a pepper of holes.
//!
//! Occupancy is binary, so ClayCore resolves a weight between 0 and 1 by
//! dithering against a hash of the cell coordinate and the brush's `seed`
//! (`voxel/dither.h`). The application sent `seed: 0` on every dab, and the
//! threshold is a pure function of the cell and the seed — so every dab of a
//! stroke skipped the SAME cells, and no amount of overlap filled them in. At
//! the shelf's defaults (intensity 0.65, a smooth falloff) that is most of the
//! footprint, which is why every voxel brush left the same crust (#139).
//!
//! The property here is about the CORE of the footprint, not its rim: a soft
//! rim is what a falloff is for, and on a binary grid a fractional rim can only
//! be spelled as partial coverage. What a sculptor cannot use is a stroke whose
//! centre is full of holes.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Falloff, GestureSample, SculptModel, ToolKind};

const CELL: f32 = 0.05;
/// Big enough that the footprint has an interior to speak about.
///
/// The engine's footprint is `round(size / cell)` cells ACROSS — a diameter —
/// so a brush of radius r deposits a ball of radius r/2 in world units. A 0.25
/// brush gives 2.5 cells of radius, whose inner half is 169 cells and says
/// nothing.
const RADIUS: f32 = 0.5;
/// The centre line of the stroke, in world units.
const FROM: f32 = -0.3;
const TO: f32 = 0.3;

fn grid() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document.add_voxel_layer("Voxels", CELL).expect("a grid");
    document
}

fn stroke(document: &mut ClayDocument, intensity: f32, falloff: Falloff) {
    let samples: Vec<GestureSample> = (0..=12)
        .map(|step| {
            let t = step as f32 / 12.0;
            GestureSample {
                position: [FROM + t * (TO - FROM), 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    let mut brush = BrushSettings {
        size: RADIUS,
        intensity,
        ..BrushSettings::default()
    };
    brush.shaping.falloff = falloff;
    document
        .apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])
        .expect("the stroke was refused");
}

/// Empty cells within half the brush radius of the stroke's centre line.
///
/// Half, because that is where the smooth falloff still reads above 0.8 and a
/// deposit is unambiguously meant: counting to the rim would count the falloff
/// itself as a defect.
fn holes_in_the_core(document: &ClayDocument) -> (usize, usize) {
    let (_, reader) = document
        .document()
        .voxel_reader("Voxels")
        .expect("the grid reads back");
    // Derived from the engine's own rule rather than from the brush radius:
    // `2 * round(size / cell) + 1` cells ACROSS, an odd span centred on the dab's
    // cell now that a grid brush's size is read as the radius it always claimed
    // to be, so `round(size / cell)` of radius either side, and the inner 60% of
    // that is where a deposit is unambiguously meant.
    //
    // Left on the old rule — `round(size / cell)` across — this core stayed put
    // while the dab doubled around it. The guard kept passing while asking about
    // half as much of the footprint, which is the failure it exists to catch.
    let across = 2 * (RADIUS / CELL).round() as i32 + 1;
    let span = ((across as f32 / 2.0) * 0.6) as i32;
    let (mut empty, mut total) = (0usize, 0usize);
    let from = (FROM / CELL).round() as i32;
    let to = (TO / CELL).round() as i32;
    for x in from..=to {
        for y in -span..=span {
            for z in -span..=span {
                // A ball around the centre line, in cells.
                if y * y + z * z > span * span {
                    continue;
                }
                total += 1;
                if reader.get([x, y, z]).expect("a cell reads back").is_none() {
                    empty += 1;
                }
            }
        }
    }
    (empty, total)
}

/// Occupied cells anywhere the stroke reached.
fn cells_written(document: &ClayDocument) -> usize {
    let (_, reader) = document
        .document()
        .voxel_reader("Voxels")
        .expect("the grid reads back");
    reader.occupied_count().expect("a count")
}

#[test]
fn a_default_grid_stroke_leaves_its_core_solid() {
    // The control first, and it runs on the same code: at full strength with a
    // flat falloff every weight is 1, nothing is dithered, and the core comes
    // out solid. That is what makes the default arm below a statement about the
    // dither rather than about the fixture's geometry.
    let mut flat = grid();
    stroke(&mut flat, 1.0, Falloff::Constant);
    let (empty, total) = holes_in_the_core(&flat);
    assert!(
        total > 200,
        "the core is too small to say anything: {total} cells"
    );
    assert_eq!(
        empty, 0,
        "the control itself is holed: {empty} of {total} core cells empty at \
         full strength with a flat falloff, so this fixture cannot speak about \
         dithering"
    );

    // And the shelf's own defaults, which is what a sculptor actually holds.
    let mut default = grid();
    stroke(
        &mut default,
        BrushSettings::default().intensity,
        Falloff::Smooth,
    );
    let (empty, total) = holes_in_the_core(&default);
    let holed = empty as f32 / total as f32;
    eprintln!(
        "default stroke: {empty} of {total} core cells empty ({:.1}%)",
        holed * 100.0
    );
    assert!(
        holed < 0.02,
        "a default stroke left {empty} of {total} core cells empty ({:.1}%): \
         every dab dithers against the same fixed seed, so the cells one dab \
         skips are skipped by all of them",
        holed * 100.0
    );
}

/// Intensity still does something, and it is the bite rather than the porosity.
///
/// The fix writes the footprint solid, so the obvious way to get that wrong
/// later is to stop reading intensity at all — which no assertion above would
/// notice, because a solid core is exactly what an ignored intensity produces.
#[test]
fn a_lighter_grid_brush_takes_a_smaller_bite() {
    let mut light = grid();
    stroke(&mut light, 0.3, Falloff::Smooth);
    let light_cells = cells_written(&light);

    let mut heavy = grid();
    stroke(&mut heavy, 1.0, Falloff::Smooth);
    let heavy_cells = cells_written(&heavy);

    eprintln!("light stroke {light_cells} cells, full stroke {heavy_cells}");
    assert!(light_cells > 0, "the light stroke deposited nothing at all");
    assert!(
        heavy_cells > light_cells * 2,
        "intensity barely changed the bite: {light_cells} cells at 0.3 against \
         {heavy_cells} at 1.0, so intensity is no longer reaching the footprint"
    );
}
