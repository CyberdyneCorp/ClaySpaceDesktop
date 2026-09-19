//! Every voxel brush: does it work, does it take a sign, does it mirror.
//!
//! The same three questions asked of the SDF shelf in `sdf_brushes.rs`, and
//! they found a different answer here. Symmetry already worked — a grid has no
//! layer mirror either, so its strokes are reflected the way a mesh's are, and
//! that landed with the mesh's. What was missing was the **sign**.
//!
//! Three of the engine's voxel verbs come in documented pairs and only one
//! half of each was ever asked for:
//!
//! - `clay_voxel_sculpt_inflate` — *"amount > 0 dilates, < 0 erodes"*, and the
//!   binding passed a hard `1`.
//! - `clay_voxel_sculpt_magnify` — *"pinch's inverse, sharing its walk so the
//!   two cannot drift apart"*, wrapped in `claycore` and reached by nothing.
//! - `erase_brush` against `set_brush` — Apagar is the one tool whose upright
//!   verb is the removal, so its opposite is the deposit.
//!
//! A fourth looked like a pair and is not. Turning the scrape's normal over
//! moved 2580 indices to 2568 — both directions remove, because the normal
//! there is a fixed up-vector rather than the surface's own, so flipping it
//! scrapes some other face rather than reversing the verb. It is left unbound.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, GestureSample, Representation, SculptModel, ToolKind};

/// A slab of material across the whole of x.
///
/// Across, not on one side: half the voxel verbs *reshape* what is there
/// rather than depositing, so a fixture packed only where the stroke lands
/// gives the mirrored copy an empty grid to work on — and every reshaping
/// brush then reads as "symmetry does nothing", which is the fixture's fault
/// rather than the brush's.
fn packed() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document
        .add_voxel_layer("Voxels", 0.05)
        .expect("add a grid");
    let brush = BrushSettings {
        size: 0.25,
        intensity: 0.9,
        ..BrushSettings::default()
    };
    for step in 0..17 {
        let t = step as f32 / 16.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                brush,
                &[GestureSample {
                    // A wobble, so a tool that only acts where the surface has
                    // curvature has something to bite on.
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

/// What the grid draws on the far side of the mirror plane, exactly.
///
/// The vertex *positions* rather than their count, and that is the difference
/// between a metric that works for every brush and one that works for most.
/// A drag translates occupancy: it carries a lump across the far side without
/// changing how many vertices are there, so counting them reads a mirrored
/// Mover as having done nothing — measured, 266 against 267.
fn far_side(document: &mut ClayDocument) -> Vec<[u32; 3]> {
    let (positions, ..) = document.visible_mesh_geometry();
    let mut out: Vec<[u32; 3]> = positions
        .iter()
        .filter(|v| v[0] < -0.05)
        .map(|v| v.map(f32::to_bits))
        .collect();
    out.sort_unstable();
    out
}

fn indices(document: &mut ClayDocument) -> usize {
    document.visible_mesh_geometry().3.len()
}

/// Occupied cells, which is what "took material away" means.
///
/// The sign tests below used to count mesh indices, and that proxy cannot say
/// whether material went. A dab that carves THROUGH a thin slab opens hole
/// walls with more surface than the top it removed, so the index count rises
/// while material falls: measured once the grid footprint matched the brush
/// radius, Padrão held inverted took 3324 cells to 2230 and indices from 11928
/// to 12552. The brush was right and the assertion read it as a deposit.
fn cells(document: &ClayDocument) -> usize {
    let (_, reader) = document
        .document()
        .voxel_reader("Voxels")
        .expect("the grid reads back");
    reader.occupied_count().expect("a count")
}

/// The highest occupied cell in the column at `x`, in world units.
///
/// The slab is deposited along a wobble, so where its top stands at a given
/// `x` is a property of the fixture rather than a number worth spelling out.
fn top_at(document: &ClayDocument, x: f32) -> f32 {
    const CELL: f32 = 0.05;
    let (_, reader) = document
        .document()
        .voxel_reader("Voxels")
        .expect("the grid reads back");
    let column = (x / CELL).round() as i32;
    let mut top = 0;
    for y in -40..40 {
        if reader
            .get([column, y, 0])
            .expect("a cell reads back")
            .is_some()
        {
            top = top.max(y);
        }
    }
    top as f32 * CELL
}

/// Where the gesture starts, for tools that reshape what is under the pointer.
///
/// Inside the slab, which is where every verb but one wants to be: a scrape,
/// a pinch, a smooth and a deposit all have material to work on there.
const ANCHOR: [f32; 3] = [0.35, 0.0, 0.0];

fn stroke(document: &mut ClayDocument, tool: ToolKind, invert: bool, symmetry: [bool; 3]) -> bool {
    // A drag is the exception, and it is the fixture's business rather than
    // the brush's. A grab is an inverse map whose weight falls to the rim, so
    // an anchor buried half a ball inside the slab puts the slab's own surface
    // exactly where the weight is ~0: those cells sample themselves, the
    // boundary does not move, and the drag writes nothing. `voxel_grab_taper`
    // records the same fixture fault and the same cure — a press lands ON the
    // surface, so that is where a drag here starts, and it pulls straight up
    // and out rather than sliding along material into material.
    //
    // It read as a working drag until the move to ClayCore v0.120.0 only
    // because the grab was quietly delivering the next falloff's curve, whose
    // support reached a shade wider. Nothing about the brush changed here.
    let dragging = tool == ToolKind::Mover;
    let from = if dragging {
        [ANCHOR[0], top_at(document, ANCHOR[0]) + 0.05, ANCHOR[2]]
    } else {
        ANCHOR
    };
    let travel = if dragging {
        [0.0, 0.4, 0.0]
    } else {
        [0.4, 0.0, 0.0]
    };
    let samples: Vec<GestureSample> = (0..9)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: std::array::from_fn(|axis| from[axis] + t * travel[axis]),
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

/// The brushes that change what the grid draws.
///
/// Máscara paints the freeze, Pintar colours cells rather than moving them,
/// and Preencher closes holes — which this slab has none of, and which
/// `voxel_tools.rs` covers on material that does.
const SHAPING: [ToolKind; 9] = [
    ToolKind::Padrao,
    ToolKind::Inflar,
    ToolKind::Suavizar,
    ToolKind::Pincar,
    ToolKind::Raspar,
    ToolKind::Camada,
    ToolKind::Nudge,
    // Both bound to verbs the engine has had all along and nothing reached.
    ToolKind::Planar,
    ToolKind::Mover,
];

/// The brushes with an opposite, and what holding the key means.
const SIGNED: [ToolKind; 5] = [
    ToolKind::Padrao,
    ToolKind::Inflar,
    ToolKind::Pincar,
    ToolKind::Camada,
    ToolKind::Apagar,
];

#[test]
fn the_shaping_brushes_are_the_ones_the_shelf_offers() {
    // So the lists above cannot drift from the vocabulary they stand for.
    let offered = ToolKind::for_representation(Representation::Voxel);
    for tool in SHAPING.iter().chain(SIGNED.iter()) {
        assert!(offered.contains(tool), "{tool:?} is not offered on a grid");
    }
    let unshaped: Vec<&str> = offered
        .iter()
        .filter(|tool| !SHAPING.contains(tool) && **tool != ToolKind::Apagar)
        .map(|tool| tool.label())
        .collect();
    assert_eq!(
        unshaped,
        vec!["Preencher", "Máscara", "Pintar"],
        "the shelf offers a grid brush this file says nothing about"
    );
}

#[test]
fn every_shaping_brush_changes_the_grid() {
    let mut base = packed();
    let rest = indices(&mut base);
    for tool in SHAPING.iter().chain([ToolKind::Apagar].iter()) {
        let mut document = packed();
        let changed = stroke(&mut document, *tool, false, [false; 3]);
        let after = indices(&mut document);
        assert!(changed, "{tool:?} reported no change");
        assert_ne!(
            after, rest,
            "{tool:?} reported a change and drew the same {rest} indices"
        );
    }
}

#[test]
fn every_shaping_brush_mirrors_when_it_is_asked_to() {
    let mut base = packed();
    let rest = far_side(&mut base);
    for tool in SHAPING.iter().chain([ToolKind::Apagar].iter()) {
        let mut document = packed();
        stroke(&mut document, *tool, false, [true, false, false]);
        let there = far_side(&mut document);
        let moved =
            there.len().abs_diff(rest.len()) + there.iter().filter(|v| !rest.contains(v)).count();
        assert!(
            moved > 5,
            "{tool:?} with X symmetry moved {moved} of the far side's \
             {} vertices",
            rest.len()
        );
    }
}

#[test]
fn no_brush_mirrors_when_it_is_not_asked_to() {
    // The control. Without it the test above passes on a brush that reaches
    // both halves whatever it was told.
    let mut base = packed();
    let rest = far_side(&mut base);
    for tool in ToolKind::for_representation(Representation::Voxel) {
        let mut document = packed();
        stroke(&mut document, tool, false, [false; 3]);
        assert_eq!(
            far_side(&mut document),
            rest,
            "{tool:?} with symmetry off still reached the far side"
        );
    }
}

// -- the sign ----------------------------------------------------------------

#[test]
fn the_depositing_brushes_take_material_away_when_inverted() {
    let rest = cells(&packed());
    for tool in [ToolKind::Padrao, ToolKind::Camada] {
        let mut up = packed();
        let mut down = packed();
        stroke(&mut up, tool, false, [false; 3]);
        stroke(&mut down, tool, true, [false; 3]);
        assert!(
            cells(&up) > rest,
            "{tool:?} did not deposit: {} cells from {rest}",
            cells(&up)
        );
        assert!(
            cells(&down) < rest,
            "{tool:?} held inverted left {} cells from {rest}, so it added \
             material where the sculptor asked to take it away",
            cells(&down)
        );
    }
}

#[test]
fn inflating_inverted_erodes() {
    // "amount > 0 dilates, < 0 erodes", says the engine. The binding passed a
    // hard 1, so only the dilating half was ever reachable.
    let rest = cells(&packed());
    let mut out = packed();
    let mut inward = packed();
    stroke(&mut out, ToolKind::Inflar, false, [false; 3]);
    stroke(&mut inward, ToolKind::Inflar, true, [false; 3]);
    assert!(cells(&out) > rest, "Inflar did not dilate");
    assert!(
        cells(&inward) < rest,
        "Inflar held inverted left {} cells from {rest} rather than eroding",
        cells(&inward)
    );
}

#[test]
fn pinching_inverted_spreads() {
    // Magnify is pinch's inverse and the engine says so outright — "sharing
    // its walk so the two cannot drift apart", the pair the SDF side spells as
    // one signed strength. It was wrapped in `claycore` and reached by
    // nothing.
    let mut base = packed();
    let rest = indices(&mut base);
    let mut inward = packed();
    let mut outward = packed();
    stroke(&mut inward, ToolKind::Pincar, false, [false; 3]);
    stroke(&mut outward, ToolKind::Pincar, true, [false; 3]);
    let (pinched, spread) = (indices(&mut inward), indices(&mut outward));
    assert_ne!(pinched, rest, "Pinçar did nothing");
    assert!(
        spread > pinched,
        "Pinçar drew {pinched} indices upright and {spread} held; magnify \
         pushes the surface out where pinch pulls it in, so the held one has \
         to be the larger"
    );
}

#[test]
fn erasing_inverted_deposits() {
    // The one tool whose upright verb is the removal, so its opposite runs the
    // other way round from every other brush's.
    let rest = cells(&packed());
    let mut gone = packed();
    let mut put = packed();
    stroke(&mut gone, ToolKind::Apagar, false, [false; 3]);
    stroke(&mut put, ToolKind::Apagar, true, [false; 3]);
    assert!(cells(&gone) < rest, "Apagar did not erase");
    assert!(
        cells(&put) > rest,
        "Apagar held inverted left {} cells from {rest} rather than \
         depositing",
        cells(&put)
    );
}

#[test]
fn the_brushes_with_no_opposite_are_left_alone_by_the_key() {
    // Stated rather than left as an absence. A majority filter has no sign to
    // turn — the same reason smoothing has none on a field or a mesh — and a
    // smudge's direction already *is* its sign. Scraping looked like a pair
    // and is not: turning its normal over moved 2580 indices to 2568, both
    // directions removing, because the normal there is a fixed up-vector
    // rather than the surface's own.
    for tool in ToolKind::for_representation(Representation::Voxel) {
        if SIGNED.contains(&tool) {
            continue;
        }
        let mut up = packed();
        let mut held = packed();
        stroke(&mut up, tool, false, [false; 3]);
        stroke(&mut held, tool, true, [false; 3]);
        assert_eq!(
            indices(&mut up),
            indices(&mut held),
            "{tool:?} answered to the invert key, which means it has an \
             opposite this file does not know about"
        );
    }
}

// -- the two that are not about geometry -------------------------------------

#[test]
fn painting_a_grid_with_the_colour_it_already_is_changes_nothing() {
    // The half of the old gap that was never the binding's fault. Pintar
    // colours cells that are already there, so a vertex *count* says nothing
    // about it — and painting a cell the colour it already carries is not a
    // change however the tool is wired.
    //
    // What matters is that it says so rather than reporting success. A tool
    // that claims to have done something is the kind that gets trusted.
    //
    // The other half — that nothing in the application could choose any other
    // colour — is closed: `brush_colour.rs` paints red and measures the
    // pixels.
    let mut document = packed();
    let before: Vec<[f32; 3]> = document.visible_mesh_geometry().2;
    let changed = stroke(&mut document, ToolKind::Pintar, false, [false; 3]);
    let after: Vec<[f32; 3]> = document.visible_mesh_geometry().2;

    assert_eq!(
        before.iter().zip(&after).filter(|(a, b)| a != b).count(),
        0,
        "painting the clay tone onto clay changed a colour"
    );
    assert!(
        !changed,
        "Pintar reported a change while changing no colour and no cell"
    );
}

#[test]
fn masking_a_grid_moves_no_material() {
    // The mask freezes a region; a tool that moved clay while painting one
    // would be the defect.
    let mut document = packed();
    let rest = indices(&mut document);
    stroke(&mut document, ToolKind::Mascara, false, [false; 3]);
    assert_eq!(
        indices(&mut document),
        rest,
        "painting a mask on a grid moved material"
    );
}
