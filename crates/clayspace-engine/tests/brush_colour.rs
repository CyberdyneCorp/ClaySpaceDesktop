//! The colour a colour brush paints with, and whether it reaches the surface.
//!
//! Pintar was on the shelf for both representations that can carry colour and
//! could not change a pixel on either. On a grid it resolved the palette entry
//! the deposit brush uses and painted that back onto cells that already had
//! it; on a mesh it left `MeshStamp::colour` at the engine's white default and
//! blended white into white. Nothing errored either time — a paint that
//! reports success and changes nothing is exactly the failure the tool table
//! exists to prevent, one layer further in.
//!
//! What is asserted here is the whole chain from the domain's colour to the
//! vertex colours the viewport uploads, because that is where the value has to
//! arrive: the palette index alone proved nothing, since the renderer was
//! separately told to ignore vertex colour.

use std::path::PathBuf;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Colour, DocumentModel, GestureSample, MaskModel, Representation, SculptModel,
    ToolKind,
};

const RED: Colour = Colour::new([0.8, 0.1, 0.1]);
const BLUE: Colour = Colour::new([0.1, 0.15, 0.75]);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("clayspace-brush-colour");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

/// A grid with a slab of material in it, across the mirror plane.
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
                    position: [(t - 0.5) * 1.6, 0.0, 0.0],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("deposit");
    }
    document
}

fn stroke(
    document: &mut ClayDocument,
    tool: ToolKind,
    symmetry: [bool; 3],
) -> clayspace_model::EditOutcome {
    let samples: Vec<GestureSample> = (0..9)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [0.2 + t * 0.5, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size: 0.2,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            symmetry,
        )
        .expect("the stroke was refused")
}

/// The vertices the viewport would upload, and their colours.
fn drawn(document: &mut ClayDocument) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
    let (positions, _, colours, ..) = document.visible_mesh_geometry();
    (positions, colours)
}

/// How many drawn vertices are within `SAME` of a colour.
fn wearing(colours: &[[f32; 3]], colour: Colour) -> usize {
    colours
        .iter()
        .filter(|c| Colour::new(**c).distance(colour) <= clayspace_model::ColourState::SAME)
        .count()
}

#[test]
fn painting_a_grid_puts_the_chosen_colour_on_the_surface() {
    let mut document = packed();
    let (before_positions, before_colours) = drawn(&mut document);
    assert_eq!(
        wearing(&before_colours, RED),
        0,
        "the fixture was already red"
    );

    document.set_colour(RED);
    assert!(stroke(&mut document, ToolKind::Pintar, [false; 3]).changed);

    let (after_positions, after_colours) = drawn(&mut document);
    assert!(
        wearing(&after_colours, RED) > 0,
        "the paint stroke changed no vertex colour"
    );
    assert_eq!(
        before_positions, after_positions,
        "painting moved the surface"
    );
}

#[test]
fn a_second_colour_paints_over_the_first() {
    let mut document = packed();
    document.set_colour(RED);
    stroke(&mut document, ToolKind::Pintar, [false; 3]);
    document.set_colour(BLUE);
    stroke(&mut document, ToolKind::Pintar, [false; 3]);

    let (_, colours) = drawn(&mut document);
    assert!(
        wearing(&colours, BLUE) > 0,
        "the second colour never landed"
    );
}

#[test]
fn the_same_colour_twice_adds_no_palette_entry() {
    let mut document = packed();
    document.set_colour(RED);
    stroke(&mut document, ToolKind::Pintar, [false; 3]);
    let (_, first) = drawn(&mut document);
    let painted = wearing(&first, RED);

    // A colour picker returns values a float apart as the pointer moves inside
    // one pixel. If the adapter matched exactly, this would add a second entry
    // and the grid would carry two reds nobody can tell apart.
    document.set_colour(Colour::new([
        RED.rgb[0] + 0.0005,
        RED.rgb[1],
        RED.rgb[2] - 0.0005,
    ]));
    stroke(&mut document, ToolKind::Pintar, [false; 3]);

    let (_, second) = drawn(&mut document);
    assert_eq!(
        wearing(&second, RED),
        painted,
        "a colour within the tolerance was stored as a different one"
    );
}

#[test]
fn a_structural_deposit_keeps_the_clay_tone() {
    // "Put material here" and "put *this colour* here" are different
    // instructions: a sculptor blocking out with a red swatch chosen would
    // otherwise find every dab red.
    let mut document = packed();
    document.set_colour(RED);
    assert!(stroke(&mut document, ToolKind::Padrao, [false; 3]).changed);
    let (_, colours) = drawn(&mut document);
    assert_eq!(
        wearing(&colours, RED),
        0,
        "an ordinary deposit took the paint colour"
    );
}

#[test]
fn a_fully_frozen_cell_keeps_the_colour_it_had() {
    let mut document = packed();
    // A wide mask over the stroke, so its 1.0 core covers material rather than
    // only its soft edge: the tool paints a mask with a smooth falloff by
    // design, and a cell at half a mask is half protected rather than frozen.
    document
        .apply_stroke(
            ToolKind::Mascara,
            BrushSettings {
                size: 0.45,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &(0..9)
                .map(|step| {
                    let t = step as f32 / 8.0;
                    GestureSample {
                        position: [0.2 + t * 0.5, 0.0, 0.0],
                        pressure: 1.0,
                        time: t,
                    }
                })
                .collect::<Vec<_>>(),
            [false; 3],
        )
        .expect("paint a mask");
    assert!(document.mask_state().present, "nothing was frozen");

    document.set_colour(RED);
    stroke(&mut document, ToolKind::Pintar, [false; 3]);

    let (positions, colours) = drawn(&mut document);
    // Asked of the mask itself rather than of a box drawn by hand: what the
    // requirement says is that a *fully* masked region is untouched, and only
    // the mask knows which vertices those are.
    let frozen = document.mask_at(&positions).expect("the mask reads back");
    let inside: Vec<usize> = frozen
        .iter()
        .enumerate()
        .filter(|(_, value)| **value >= 0.999)
        .map(|(at, _)| at)
        .collect();
    assert!(
        !inside.is_empty(),
        "the fixture has nothing under the mask's core"
    );
    let red_inside = inside
        .iter()
        .filter(|at| Colour::new(colours[**at]).distance(RED) <= clayspace_model::ColourState::SAME)
        .count();
    assert_eq!(
        red_inside,
        0,
        "{red_inside} of {} frozen vertices took the paint",
        inside.len()
    );
}

#[test]
fn undo_takes_the_paint_back_and_redo_puts_it_on_again() {
    // Whole-gesture grouping is the ViewModel's — it banks the entries a
    // gesture wrote and spends them together — so what is asserted here is the
    // half that belongs to the engine adapter: the colours a paint stroke
    // wrote are on the engine's own history and come back off it.
    let mut document = packed();
    let (_, before) = drawn(&mut document);
    document.set_colour(RED);
    stroke(&mut document, ToolKind::Pintar, [false; 3]);
    let (_, painted) = drawn(&mut document);
    let count = wearing(&painted, RED);
    assert!(count > 0, "nothing was painted");

    let mut steps = 0;
    while wearing(&drawn(&mut document).1, RED) > 0 {
        assert!(
            document.undo().expect("undo"),
            "the history ran out with paint still on the surface"
        );
        steps += 1;
        assert!(steps < 64, "undo is not converging");
    }
    let (_, undone) = drawn(&mut document);
    assert_eq!(undone.len(), before.len(), "undo changed the geometry");

    for _ in 0..steps {
        assert!(document.redo().expect("redo"), "the redo ran out");
    }
    let (_, redone) = drawn(&mut document);
    assert_eq!(
        wearing(&redone, RED),
        count,
        "redo put back a different colour"
    );
}

#[test]
fn a_painted_colour_survives_the_document() {
    let path = scratch("painted.clay");
    let mut document = packed();
    document.set_colour(RED);
    stroke(&mut document, ToolKind::Pintar, [false; 3]);
    let (_, saved) = drawn(&mut document);
    let painted = wearing(&saved, RED);
    assert!(painted > 0, "nothing was painted to save");

    document.save(&path).expect("save");

    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut reopened = ClayDocument::new(policy).expect("a document");
    reopened.open(&path).expect("reopen");
    let (_, restored) = drawn(&mut reopened);
    assert_eq!(
        wearing(&restored, RED),
        painted,
        "the reopened document carries different colours"
    );
}

#[test]
fn painting_a_mesh_blends_toward_the_chosen_colour() {
    // The mesh side had the same hole one layer up: the stamp's colour was the
    // engine's white default, so Pintar blended white into a white mesh.
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document
        .add_voxel_layer("Voxels", 0.04)
        .expect("add a grid");
    for step in 0..13 {
        let t = step as f32 / 12.0;
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.22,
                    intensity: 0.9,
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: [(t - 0.5) * 1.2, 0.0, 0.0],
                    pressure: 1.0,
                    time: t,
                }],
                [false; 3],
            )
            .expect("deposit");
    }
    // Carry the grid across as triangles, which is the only way a mesh layer
    // with a colour attribute comes to exist here.
    document
        .convert_layer_in_place(clayspace_model::conversion::Direction::VoxelToMesh, 0.04, 0)
        .expect("carry the grid across as a mesh");
    assert_eq!(
        document.active_representation(),
        Representation::Mesh,
        "the conversion did not land"
    );

    document.set_colour(RED);
    let outcome = stroke(&mut document, ToolKind::Pintar, [false; 3]);
    assert!(outcome.changed, "the mesh paint stroke changed nothing");

    let (_, colours) = drawn(&mut document);
    // A blend rather than a replacement, so the assertion is that the surface
    // moved *toward* red rather than that it arrived: the reddest vertex must
    // be redder than anything the fixture had.
    let reddest = colours
        .iter()
        .map(|c| c[0] - 0.5 * (c[1] + c[2]))
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        reddest > 0.15,
        "no vertex moved toward the brush colour; the reddest is {reddest}"
    );
}
