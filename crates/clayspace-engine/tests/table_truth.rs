//! Whether the capability table describes the application that exists.
//!
//! `ToolKind::verbs` is the authority for what each tool does on each
//! representation: the shelf reads it, the availability rule reads it, the
//! diagnostics report prints it and the tool notes hang off it. Everything in
//! `tools.rs`'s own suite checks it **against itself** — that the shelf and the
//! refusal agree, that every tool reaches somewhere — which is worth having and
//! is a different question from the one here.
//!
//! The question here is whether the rows are *true*. Two ways they can stop
//! being true, and neither was catchable before:
//!
//!   * the name is not a symbol the engine has, because a pin move renamed or
//!     withdrew it. The guard this replaces asserted the string began `clay_`,
//!     which passes for a renamed entry point, a removed one, and a verb the
//!     dispatch does not use;
//!   * the name is a symbol, and it is not the one that runs. That was the
//!     multires smooth (#199), and it is the failure that made this file worth
//!     writing: a row can be plausible, well commented and describe a call
//!     nobody makes.
//!
//! This is the file that has to live here rather than in `clayspace-model`,
//! because answering either question needs the domain and the engine in scope
//! at once and the domain may link no engine.
//!
//! **Cost.** Every pair is a fixture built and a stroke made, and the mesh
//! fixture is marched out of a field. It is the same shape `tool_table.rs`
//! runs and for the same reason: a table walked by hand is a table with a row
//! nobody walked.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    entry_points, every_entry_point, BrushSettings, ConversionSettings, Direction, ExchangeModel,
    GestureSample, ImportSettings, MultiresLevelOp, MultiresSculptLayerOp, Representation,
    SceneModel, SculptModel, SmoothFrequency, ToolKind, ToolNote,
};
use std::collections::BTreeSet;

// -- the names ---------------------------------------------------------------

/// Every verb the table names is a symbol the pinned engine declares.
///
/// The bindings rather than the header: what `clay.h` declares and what this
/// build links are two questions, and the second is the one a row's claim rests
/// on. `claycore::ENTRY_POINTS` is generated from bindgen's own output for
/// exactly that reason.
///
/// This is what turns an engine rename into a failing test on the pin move that
/// does it, instead of into a row that goes on describing a call nobody makes.
#[test]
fn every_verb_the_table_names_is_a_symbol_the_engine_has() {
    let mut missing: Vec<&str> = Vec::new();
    for name in every_entry_point() {
        if !claycore::has_entry_point(name) {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "the capability table names what the pinned engine does not declare: \
         {}. Either the engine renamed it — correct the row — or the row was \
         written from a plan rather than from the ABI.",
        missing.join(", ")
    );
}

// -- the calls ---------------------------------------------------------------

/// Every offered pair reaches an entry point its row names.
///
/// **Why an intersection and not an equality.** The trace records every engine
/// call a stroke makes, and a stroke makes many that are nobody's verb: it asks
/// a grid its cell size, a palette its length, a sculptor for a refit. So what
/// is asserted is that at least one of the calls the row names was among them.
/// That is enough for the failure this exists to catch — a row naming a call
/// the dispatch never reaches has an empty intersection, whatever else ran.
///
/// A row that names several calls names them because several are reachable: a
/// field drag opens a transaction where it can and falls back to
/// `clay_layer_move_surface_regions` where it cannot, and one stroke takes one
/// of those routes.
#[test]
fn every_pair_calls_an_entry_point_its_row_names() {
    let mut wrong: Vec<String> = Vec::new();
    for representation in Representation::ALL {
        for tool in ToolKind::for_representation(representation) {
            if !strokes(tool) {
                continue;
            }
            let called = what_one_stroke_called(tool, representation);
            let named: BTreeSet<&str> = entry_points(
                tool.verb_on(representation)
                    .expect("the shelf offered it, so the row has a verb"),
            )
            .into_iter()
            .collect();
            let reached = !called.is_disjoint(&named);

            if !reached {
                wrong.push(format!(
                    "{} on {} names {named:?} and called none of them; it called {called:?}",
                    tool.label(),
                    representation.label()
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "these rows describe a call the stroke does not make:\n  {}\n\nThe row \
         is the shelf, the refusal and the diagnostics report. A row that is \
         not what runs is an interface promising something the engine call \
         does not keep.",
        wrong.join("\n  ")
    );
}

/// Every engine call one stroke of `tool` makes on a fresh fixture.
///
/// The fixture is built *before* the recording starts: building one is
/// thousands of engine calls and none of them is this tool's.
fn what_one_stroke_called(
    tool: ToolKind,
    representation: Representation,
) -> BTreeSet<&'static str> {
    let mut document = worked(representation);
    // One pair needs the fixture pointed at a pass rather than at the form
    // under them, and it is not a convenience: the hierarchy's eraser acts on
    // the selected pass and refuses the form, so with the default selection
    // there is no stroke to record. Every other tool on a hierarchy means the
    // same thing in either row, and the fixture leaves them in the form —
    // where they call `clay_multires_sculptor_*`, which is what their rows
    // name.
    if (tool, representation) == (ToolKind::Apagar, Representation::Multires) {
        document
            .apply_multires_sculpt_layer_op(MultiresSculptLayerOp::Add {
                name: "Poros".to_string(),
            })
            .expect("a hierarchy takes a pass");
    }
    let recording = claycore::trace::Recording::start();
    drag(&mut document, tool, representation);
    let called = recording.calls().into_iter().collect();
    drop(recording);
    called
}

/// Whether a tool is one this file strokes at all.
///
/// Trim is not: its gesture is a shape drawn on the view frame rather than a
/// stroke across the surface, so `apply_stroke` is not the call it makes.
/// Máscara is — it paints the world-addressed freeze the other verbs consult,
/// and `clay_mask_apply_stroke` is a row like any other.
fn strokes(tool: ToolKind) -> bool {
    tool.is_stroke_tool()
}

// -- the notes ---------------------------------------------------------------

/// Each note describes a difference; each difference is measured somewhere.
///
/// The `match` is the point. A note added to [`ToolNote`] stops this file
/// compiling until somebody names the test that proves it — which is the only
/// thing standing between a note and a sentence the interface tells an artist
/// because it reads well.
#[test]
fn every_tool_note_is_proved_here() {
    for note in ToolNote::ALL {
        let proof = match note {
            ToolNote::VoxelPlanarIsTwoSided => "a_grid_flatten_fills_as_well_as_cuts",
            ToolNote::MultiresSmoothChoosesAFrequency => "a_hierarchy_smooth_picks_a_frequency",
            ToolNote::MultiresStoresNoColour => "a_colour_brush_on_a_hierarchy_is_refused_for_real",
            ToolNote::MultiresEraseTakesThisPassToZero => {
                "erasing_is_a_different_verb_on_a_grid_and_on_a_hierarchy"
            }
        };
        assert!(!proof.is_empty(), "a note with no test naming it: {note:?}");
    }
}

/// A grid's flatten fills hollows below the plane as well as cutting above it.
///
/// Measured as cells that were **empty and are now occupied**, which is the one
/// thing a cut-only verb can never produce.
///
/// The contrast is the scrape rather than the same tool on a field, and that is
/// deliberate: both verbs are given the same plane — the engine takes a normal
/// rather than deriving one, and `stroke_voxel` hands both `+Y` through the
/// origin — so the only difference left between them is the two-sidedness the
/// note claims. A comparison against another representation would carry a
/// different fixture and a different plane with it.
#[test]
fn a_grid_flatten_fills_as_well_as_cuts() {
    let (filled, cut) = what_the_grid_verb_did(ToolKind::Planar);
    assert!(
        filled > 0 && cut > 0,
        "the grid's flatten is two-sided: it filled {filled} cells and cut \
         {cut}. A run that only cuts is the cut-only verb the other \
         representations have, and the note is then telling an artist about a \
         difference that is not there."
    );

    let (scraped_in, scraped_out) = what_the_grid_verb_did(ToolKind::Raspar);
    assert!(
        scraped_out > 0,
        "the fixture gave the scrape nothing to take off"
    );
    assert_eq!(
        scraped_in, 0,
        "the scrape filled {scraped_in} cells, so filling is not what \
         distinguishes the flatten and the note is measuring the fixture"
    );
}

/// How much material one stroke of `tool` put back, and how much it took.
fn what_the_grid_verb_did(tool: ToolKind) -> (usize, usize) {
    let mut document = worked(Representation::Voxel);
    let before = occupied(&document);
    drag(&mut document, tool, Representation::Voxel);
    let after = occupied(&document);
    (
        after.difference(&before).count(),
        before.difference(&after).count(),
    )
}

/// A hierarchy carries no colour, and the refusal is the engine's answer too.
///
/// The domain's own suite asserts the shelf does not offer the two colour
/// brushes on a hierarchy and that the refusal carries the note. What it cannot
/// say is whether the absence describes anything: this does, by making the same
/// brush work on a mesh — the route the note sends an artist down — and refuse
/// on a hierarchy built from that very cage.
#[test]
fn a_colour_brush_on_a_hierarchy_is_refused_for_real() {
    let mut mesh = worked(Representation::Mesh);
    mesh.apply_stroke(
        ToolKind::Pintar,
        painting(),
        &path_over(Representation::Mesh),
        [false; 3],
    )
    .expect("a mesh layer takes a colour brush, which is where the note sends an artist");

    let (mut hierarchy, _) = with_a_hierarchy_of(2);
    let refused = hierarchy.apply_stroke(
        ToolKind::Pintar,
        painting(),
        &path_over(Representation::Multires),
        [false; 3],
    );
    assert!(
        refused.is_err(),
        "a hierarchy took a colour brush. It stores where a vertex went and \
         not what colour it is, so the colour would land in the level's \
         rebuildable cache and evaporate — which is worse than a refusal."
    );
}

/// Apagar is one label over two operations, and the note is what says so.
///
/// Measured as the calls rather than as the surface, because what the note
/// warns about is precisely that the *operation* differs: a grid's eraser
/// clears the cells the brush covers, and a hierarchy has no cells to clear —
/// the same gesture opens the layered transaction and walks the selected
/// pass's displacement toward zero. A test comparing two surfaces would find
/// them both lower and conclude the two agreed.
#[test]
fn erasing_is_a_different_verb_on_a_grid_and_on_a_hierarchy() {
    let grid = what_one_stroke_called(ToolKind::Apagar, Representation::Voxel);
    assert!(
        grid.contains("clay_voxel_erase_brush"),
        "a grid's eraser no longer clears cells: {grid:?}"
    );

    let hierarchy = what_one_stroke_called(ToolKind::Apagar, Representation::Multires);
    assert!(
        hierarchy.contains("clay_multires_sculpt_layer_stroke_erase"),
        "the hierarchy's eraser did not reach the entry point that takes a \
         pass toward zero: {hierarchy:?}. Without it the note is describing \
         an operation nobody runs."
    );
    assert!(
        !hierarchy.contains("clay_voxel_erase_brush"),
        "the hierarchy's eraser reached the grid's verb, so the two columns \
         are one operation after all and the note is telling an artist about \
         a difference that is not there"
    );
}

/// The hierarchy smooth picks a frequency, and the choice changes the surface.
///
/// The note's claim is that this one verb has a mode where the other three
/// have none, so what proves it is a *difference between two modes* rather
/// than a call being reached: `clay_multires_sculpt_layer_stroke_smooth` being
/// opened is already asserted, once and for every row, by
/// `every_pair_calls_an_entry_point_its_row_names`. What that cannot say is
/// whether the mode it was handed meant anything.
///
/// So both modes are run on the same fixture and the detail is measured after
/// each. `FormWithDetail` — what a sculptor who has not chosen gets — leaves
/// the pores at their own height *while* the form under them moves, and `Form`
/// takes them off, which is the plain Laplacian a mesh has. Either half alone
/// would pass for the wrong reason: a smooth that reached nothing at all would
/// keep the detail perfectly.
///
/// The measurement is `multires.rs`'s, in the two tests that landed with the
/// routing — a pair of hierarchies alike but for a pass deposit, so the detail
/// is a subtraction rather than a guess about which part of one surface was
/// which. It is repeated here because this is where the note is proved, and
/// `every_tool_note_is_proved_here` names this test.
#[test]
fn a_hierarchy_smooth_picks_a_frequency() {
    let (mut pored, mut plain) = a_pored_pair();
    let before = detail_between(&mut pored, &mut plain);
    assert!(before > 1e-3, "the fixture deposited pores: {before}");
    let form_before = drawn(&mut plain);

    for document in [&mut pored, &mut plain] {
        smooth_at(document, SmoothFrequency::FormWithDetail);
    }
    let kept = detail_between(&mut pored, &mut plain);
    assert!(
        (kept - before).abs() < before * 0.05,
        "the default mode is supposed to carry the detail through unchanged, \
         and the pores went from {before} to {kept}. That is the operation the \
         note tells an artist a hierarchy can do and a flat mesh cannot."
    );
    let form_moved = travelled(&form_before, &drawn(&mut plain));
    assert!(
        form_moved > 1e-3,
        "and the form underneath did move ({form_moved}); a smooth that \
         reached nothing would have kept the detail just as well, and the note \
         would be describing a mode that does nothing."
    );

    let (mut pored, mut plain) = a_pored_pair();
    let before = detail_between(&mut pored, &mut plain);
    for document in [&mut pored, &mut plain] {
        smooth_at(document, SmoothFrequency::Form);
    }
    let removed = detail_between(&mut pored, &mut plain);
    assert!(
        removed < before * 0.9,
        "the other mode is the plain Laplacian over the positions, which takes \
         the pores off with the lump: {before} before, {removed} after. If \
         both modes leave the surface alike then the row's mode is not reaching \
         the engine and the note names a choice that is not offered."
    );
}

/// Two hierarchies alike in every way but a pass full of pores.
///
/// Two documents rather than one, and the second is not a spare: what a smooth
/// did to the *detail* is the difference between a hierarchy that has the pass
/// deposit and one that is alike in every other way. Both carry a pass, so both
/// strokes go into the same write domain and the comparison is between the
/// modes and nothing else.
fn a_pored_pair() -> (ClayDocument, ClayDocument) {
    let (mut pored, pored_key) = with_a_hierarchy_of(PORED_LEVELS);
    let (mut plain, plain_key) = with_a_hierarchy_of(PORED_LEVELS);

    // The form: a bump at the level the brush is bound to, so there is
    // something with curvature for a smooth to take out. A lump put on the cage
    // comes up through the subdivisions already smooth, and a test whose form
    // barely moves cannot tell "the form moved" from noise.
    for document in [&mut pored, &mut plain] {
        for _ in 0..3 {
            dab(document, [0.0, 0.0, 0.0], 1.2);
        }
    }

    // The pores: a pass of their own on both, filled on one. A pass is where
    // the engine keeps detail apart from the form, which is the whole subject.
    add_pass(&mut plain, plain_key);
    add_pass(&mut pored, pored_key);
    dab(&mut pored, PORES_AT, 0.3);
    (pored, plain)
}

/// Smooths the pored region of a hierarchy at the stated frequency.
fn smooth_at(document: &mut ClayDocument, mode: SmoothFrequency) {
    document.set_smooth_mode(mode);
    // Four passes over the same place: one dab of a smooth moves a surface by
    // very little, and a difference that small is one the tolerances above
    // could not tell from arithmetic noise.
    for _ in 0..4 {
        document.begin_gesture();
        let outcome = document.apply_stroke(
            ToolKind::Suavizar,
            BrushSettings {
                size: 1.2,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: PORES_AT,
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        );
        document.end_gesture();
        assert!(
            outcome.expect("the smooth is applied").changed,
            "the smooth reached the surface"
        );
    }
}

/// How far the tallest vertex of one surface stands from the other's.
fn detail_between(pored: &mut ClayDocument, plain: &mut ClayDocument) -> f32 {
    let (here, there) = (drawn(pored), drawn(plain));
    assert_eq!(
        here.len(),
        there.len(),
        "the two hierarchies are the same subject, so they are the same size"
    );
    tallest_difference(&here, &there)
}

/// How far the farthest vertex travelled between two pictures of one surface.
fn travelled(before: &[[f32; 3]], after: &[[f32; 3]]) -> f32 {
    assert_eq!(before.len(), after.len(), "the same surface, twice");
    tallest_difference(after, before)
}

/// The tallest per-vertex distance between two same-sized point sets.
fn tallest_difference(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(here, there)| {
            (0..3)
                .map(|axis| (here[axis] - there[axis]).powi(2))
                .sum::<f32>()
                .sqrt()
        })
        .fold(0.0f32, f32::max)
}

/// The triangles the viewport is handed for the whole document.
fn drawn(document: &mut ClayDocument) -> Vec<[f32; 3]> {
    document.visible_mesh_geometry().0
}

/// One dab of the default brush at the level the brush is bound to.
fn dab(document: &mut ClayDocument, at: [f32; 3], size: f32) {
    document.begin_gesture();
    let outcome = document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: at,
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    );
    document.end_gesture();
    assert!(
        outcome.expect("the dab is applied").changed,
        "the dab reached the surface"
    );
}

/// Adds a pass to the hierarchy, which is where detail is kept apart from form.
fn add_pass(document: &mut ClayDocument, key: clayspace_model::LayerKey) {
    document
        .apply_multires_sculpt_layer_op(MultiresSculptLayerOp::Add {
            name: "Poros".to_string(),
        })
        .expect("a hierarchy takes a pass");
    assert!(
        document
            .scene()
            .layer(key)
            .and_then(|layer| layer.multires.as_ref())
            .is_some_and(|state| !state.sculpt_layers.is_empty()),
        "the pass is on the stack the row draws"
    );
}

// -- fixtures ----------------------------------------------------------------

/// How finely the mesh fixture is marched. Coarse: the marching is what a run
/// of this file costs, and every pair on a mesh builds one.
const MESH_CELL: f32 = 0.05;

/// Where each fixture is worked, and where every stroke here is made.
fn over(representation: Representation) -> [f32; 3] {
    match representation {
        // The top of the starting sphere, and of the mesh carried off it.
        Representation::Sdf | Representation::Mesh => [0.0, 0.0, 1.0],
        // The middle of the slab, and of the flat cage.
        Representation::Voxel | Representation::Multires => [0.0, 0.0, 0.0],
    }
}

/// A form of the given representation, with something in it to sculpt.
///
/// The same fixtures `tool_table.rs` builds, and the mesh one is carried off a
/// **field** rather than off the grid for the reason that file gives: a grid
/// marches to greedy quads, and the verbs gated on dihedral angle correctly
/// decline a surface made entirely of right angles.
fn worked(representation: Representation) -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    match representation {
        Representation::Multires => with_a_hierarchy_of(2).0,
        Representation::Voxel => {
            let mut document = ClayDocument::new(policy).expect("a document");
            document
                .add_voxel_layer("Voxels", 0.04)
                .expect("add a grid");
            // A wobbling slab across the plane the planing verbs use, so a
            // flatten has material above it *and* hollows below it.
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
                    .convert_layer_in_place(Direction::SdfToMesh, MESH_CELL, 0)
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

/// How deep the hierarchy the pored pair is built on goes.
///
/// Deeper than the pairs walked by the table, and `multires.rs` uses the same
/// depth for the same reason: the pores have to land on a level finer than the
/// one the form bump is on, or there is no detail to keep apart from the form.
const PORED_LEVELS: u32 = 3;

/// Where the pores are deposited, and where the smooths are made.
///
/// Off the centre of the sheet, so the form bump under them is a slope rather
/// than a summit and a smooth that moves the form has somewhere to move it.
const PORES_AT: [f32; 3] = [0.45, 0.0, 0.45];

/// A document whose only layer is a hierarchy, `levels` deep over a flat cage.
///
/// Built the way `multires.rs` builds one, because it is the only route there
/// is: a hierarchy arrives through the crossing from a mesh and there is no
/// call anywhere that makes an empty one.
fn with_a_hierarchy_of(levels: u32) -> (ClayDocument, clayspace_model::LayerKey) {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    let path = scratch();
    cage_obj(&path, 4, 2.0);
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import the cage");
    let _ = std::fs::remove_file(&path);

    let cage = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the cage is a mesh layer");
    document.set_active_layer(cage).expect("activate the cage");

    let settings = ConversionSettings::default();
    let key = document
        .convert_layer_in_place(Direction::MeshToMultires, settings.cell_size, settings.blur)
        .expect("a flat quad grid is a cage");
    for _ in 0..levels {
        document
            .apply_multires_level_op(MultiresLevelOp::AddLevel)
            .expect("subdivide");
    }
    (document, key)
}

/// A path in the temporary directory that no other fixture can be handed.
///
/// Unique per call, not per name: the tests in one integration binary are
/// threads of one process, and a shared name is one test deleting the file
/// another is loading.
fn scratch() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "clayspace-table-truth-{}-{}.obj",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// A flat grid of quads, which is what a Catmull-Clark cage is supposed to be.
fn cage_obj(path: &std::path::Path, divisions: usize, half: f32) {
    let mut text = String::new();
    let step = 2.0 * half / divisions as f32;
    for z in 0..=divisions {
        for x in 0..=divisions {
            text.push_str(&format!(
                "v {} 0 {}\n",
                -half + step * x as f32,
                -half + step * z as f32
            ));
        }
    }
    let stride = divisions + 1;
    for z in 0..divisions {
        for x in 0..divisions {
            // Wound so the sheet faces +y, which makes a Draw stamp read as a
            // bump rather than a dent.
            let a = z * stride + x + 1;
            text.push_str(&format!(
                "f {} {} {} {}\n",
                a,
                a + stride,
                a + stride + 1,
                a + 1
            ));
        }
    }
    std::fs::write(path, text).expect("write the cage");
}

/// The stroke every tool is given: a short drag across the material.
///
/// One shape for all of them, because the question is which binding the
/// dispatch reaches rather than whether a particular gesture suits a particular
/// brush. A refusal is not swallowed: a tool the shelf offers and the dispatch
/// declines is the failure this file's sibling, `tool_table.rs`, exists for,
/// and hiding it here would make this one's message misleading.
fn drag(document: &mut ClayDocument, tool: ToolKind, representation: Representation) {
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size: 0.25,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &path_over(representation),
            [false; 3],
        )
        .unwrap_or_else(|e| {
            panic!(
                "{} on a {} layer: {e}",
                tool.label(),
                representation.label()
            )
        });
}

/// The samples of that drag.
fn path_over(representation: Representation) -> Vec<GestureSample> {
    let at = over(representation);
    (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [at[0] + (t - 0.5) * 0.5, at[1], at[2]],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// A brush a colour tool will actually write with.
fn painting() -> BrushSettings {
    BrushSettings {
        size: 0.25,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// Every occupied cell of the grid, so two states can be compared cell by cell.
fn occupied(document: &ClayDocument) -> BTreeSet<[i32; 3]> {
    let (_, reader) = document
        .document()
        .voxel_reader("Voxels")
        .expect("the grid reads back");
    let mut cells = BTreeSet::new();
    let Some((lo, hi)) = reader.bounds().expect("bounds") else {
        return cells;
    };
    for z in lo[2]..=hi[2] {
        for y in lo[1]..=hi[1] {
            for x in lo[0]..=hi[0] {
                if reader.get([x, y, z]).expect("a cell reads back").is_some() {
                    cells.insert([x, y, z]);
                }
            }
        }
    }
    cells
}
