//! An adaptive surface as a subtool: crossed into, sculpted, taken back,
//! saved and crossed out of.
//!
//! The representation's claim is that connectivity changes under the brush,
//! so the central measurement here is a triangle count rather than a vertex
//! position: a Draw dab on a coarse sheet has to *add* triangles, and the same
//! dab on a mesh layer cannot. Everything else guards a seam the
//! representation has only because of how it is owned — a
//! `clay_dynamic_surface` is a handle `clay_document_save` has never heard of —
//! so the surface travels in a side-car and its history is its own bytes.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ConversionSettings, Direction, DocumentModel, ExchangeModel, GestureSample,
    ImportSettings, LayerKey, ModelError, Representation, SceneModel, SculptModel, ToolKind,
    ToolNote, Unavailable,
};

// -- fixtures ---------------------------------------------------------------

/// A path in the temporary directory no other fixture can be handed.
fn scratch(name: &str, extension: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "clayspace-dynamic-{name}-{}-{}.{extension}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// A flat sheet of quads facing +y, coarse enough that a brush has to refine,
/// grey where it carries vertex colour at all.
fn sheet_obj(path: &std::path::Path, divisions: usize, half: f32, coloured: bool) {
    let mut text = String::new();
    let step = 2.0 * half / divisions as f32;
    let colour = if coloured { " 0.5 0.5 0.5" } else { "" };
    for z in 0..=divisions {
        for x in 0..=divisions {
            text.push_str(&format!(
                "v {} 0 {}{colour}\n",
                -half + step * x as f32,
                -half + step * z as f32
            ));
        }
    }
    let stride = divisions + 1;
    for z in 0..divisions {
        for x in 0..divisions {
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
    std::fs::write(path, text).expect("write the sheet");
}

/// A document holding one imported mesh sheet, active.
fn with_a_mesh(who: &str) -> (ClayDocument, LayerKey) {
    with_a_sheet(who, false)
}

fn with_a_sheet(who: &str, coloured: bool) -> (ClayDocument, LayerKey) {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    let path = scratch(who, "obj");
    sheet_obj(&path, 8, 2.0, coloured);
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import the sheet");
    let _ = std::fs::remove_file(&path);
    let mesh = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the sheet is a mesh layer");
    document.set_active_layer(mesh).expect("activate the sheet");
    (document, mesh)
}

/// The same sheet, crossed in place into an adaptive surface.
fn with_a_surface(who: &str) -> (ClayDocument, LayerKey) {
    crossed(with_a_mesh(who).0)
}

fn crossed(mut document: ClayDocument) -> (ClayDocument, LayerKey) {
    let settings = ConversionSettings::default();
    let key = document
        .convert_layer_in_place(Direction::MeshToDynamic, settings.cell_size, settings.blur)
        .expect("a welded sheet is an adaptive surface");
    (document, key)
}

fn stroke(document: &mut ClayDocument, tool: ToolKind, from: [f32; 3], to: [f32; 3]) -> bool {
    document.begin_gesture();
    let samples: Vec<GestureSample> = (0..=6)
        .map(|step| {
            let t = step as f32 / 6.0;
            GestureSample {
                position: std::array::from_fn(|i| from[i] + (to[i] - from[i]) * t),
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    let outcome = document.apply_stroke(
        tool,
        BrushSettings {
            size: 0.6,
            intensity: 0.8,
            ..BrushSettings::default()
        },
        &samples,
        [false; 3],
    );
    document.end_gesture();
    outcome.expect("the stroke is applied").changed
}

fn dab(document: &mut ClayDocument, tool: ToolKind) -> bool {
    stroke(document, tool, [-0.2, 0.0, 0.0], [0.2, 0.0, 0.0])
}

/// The triangles the viewport is handed, as (positions, triangle count).
fn drawn(document: &mut ClayDocument) -> (Vec<[f32; 3]>, usize) {
    let (positions, _, _, indices, _) = document.visible_mesh_geometry();
    (positions, indices.len() / 3)
}

fn representation_of(document: &ClayDocument, key: LayerKey) -> Representation {
    document
        .scene()
        .layer(key)
        .map(|layer| layer.representation)
        .expect("the row is there")
}

// -- the representation -----------------------------------------------------

/// A crossed layer is Dynamic in the scene, is the active layer, and offers
/// the Dynamic shelf — never reported as the mesh it was read from.
#[test]
fn a_mesh_crosses_into_a_layer_that_is_reported_as_dynamic() {
    let (document, key) = with_a_surface("reported");
    assert_eq!(representation_of(&document, key), Representation::Dynamic);
    assert_eq!(document.active_representation(), Representation::Dynamic);
    assert_eq!(
        document
            .scene()
            .layers
            .iter()
            .filter(|layer| layer.representation == Representation::Mesh)
            .count(),
        0,
        "an in-place crossing leaves no mesh row behind, and the surface is \
         not counted as one"
    );
    assert!(ToolKind::for_representation(Representation::Dynamic).contains(&ToolKind::Padrao));
    assert_eq!(document.dynamic_diagnostics().held, 1);
}

/// An empty adaptive layer cannot be made out of nothing.
#[test]
fn an_empty_dynamic_layer_is_refused() {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    assert!(document
        .add_layer("Vazia", Representation::Dynamic)
        .is_err());
}

/// A Draw stroke on an adaptive layer adds triangles; the same stroke on the
/// mesh it was read from moves vertices and adds none.
#[test]
fn a_stamp_on_a_dynamic_layer_changes_triangle_count_where_its_verb_remeshes() {
    let (mut mesh, _) = with_a_mesh("fixed");
    let (_, fixed_before) = drawn(&mut mesh);
    assert!(dab(&mut mesh, ToolKind::Padrao), "the mesh stroke lands");
    let (_, fixed_after) = drawn(&mut mesh);
    assert_eq!(
        fixed_before, fixed_after,
        "a fixed mesh keeps its topology under the brush"
    );

    let (mut surface, _) = with_a_surface("adaptive");
    let (_, before) = drawn(&mut surface);
    assert!(
        dab(&mut surface, ToolKind::Padrao),
        "the adaptive stroke lands"
    );
    let (_, after) = drawn(&mut surface);
    assert!(
        after > before,
        "Draw refines before it deposits, so a coarse sheet gains triangles \
         under it: {before} before, {after} after"
    );
}

/// Move remeshes after its drag, so a pull that stretches the sheet leaves
/// geometry in the stretch.
#[test]
fn a_move_refines_what_it_stretched() {
    let (mut surface, _) = with_a_surface("move");
    let (_, before) = drawn(&mut surface);
    assert!(
        stroke(
            &mut surface,
            ToolKind::Mover,
            [0.0, 0.0, 0.0],
            [0.0, 1.2, 0.0]
        ),
        "the drag lands"
    );
    let (positions, after) = drawn(&mut surface);
    assert!(
        positions.iter().any(|p| p[1] > 0.1),
        "the drag lifted the sheet"
    );
    assert!(
        after > before,
        "and the remesh after it made triangles for the stretch: {before} \
         before, {after} after"
    );
}

/// Layer is refused on an adaptive layer, with the note that says why, and
/// nothing changes.
#[test]
fn layer_is_refused_on_a_dynamic_layer_and_changes_nothing() {
    let (mut surface, _) = with_a_surface("no-layer");
    let (before, _) = drawn(&mut surface);
    let refused = surface.apply_stroke(
        ToolKind::Camada,
        BrushSettings::default(),
        &[GestureSample {
            position: [0.0; 3],
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    );
    assert!(matches!(
        refused,
        Err(ModelError::Unavailable(Unavailable::NoVerbHere {
            active: Representation::Dynamic,
            note: Some(ToolNote::DynamicHasNoLayer),
            ..
        }))
    ));
    assert_eq!(drawn(&mut surface).0, before);
}

// -- history ----------------------------------------------------------------

/// A topology-changing gesture is one undo, and the undo restores the
/// connectivity as well as the positions.
#[test]
fn a_dynamic_gesture_is_one_undo_that_restores_connectivity() {
    let (mut surface, _) = with_a_surface("undo");
    let flat = drawn(&mut surface);
    assert!(dab(&mut surface, ToolKind::Padrao));
    let sculpted = drawn(&mut surface);
    assert_ne!(flat.1, sculpted.1, "the stroke changed the triangle count");
    let watched = surface.mesh_revision();

    assert!(surface.undo().expect("undo"));
    assert_eq!(drawn(&mut surface), flat, "back to the sheet, exactly");
    assert_ne!(surface.mesh_revision(), watched, "and the viewport knows");

    assert!(surface.redo().expect("redo"));
    assert_eq!(drawn(&mut surface), sculpted, "and forward again, exactly");
}

/// A gesture that reached nothing is not an undo step.
#[test]
fn a_gesture_that_reached_nothing_is_not_an_undo_step() {
    let (mut surface, _) = with_a_surface("nothing");
    let depth = surface.history().depth;
    assert!(!stroke(
        &mut surface,
        ToolKind::Padrao,
        [40.0, 40.0, 40.0],
        [41.0, 40.0, 40.0]
    ));
    assert_eq!(surface.history().depth, depth);
}

// -- persistence ------------------------------------------------------------

/// A saved adaptive layer reopens as Dynamic, with the surface it had.
#[test]
fn a_dynamic_layer_survives_save_and_load() {
    let (mut surface, _) = with_a_surface("round-trip");
    assert!(dab(&mut surface, ToolKind::Padrao));
    let sculpted = drawn(&mut surface);

    let path = scratch("round-trip", "clayspace");
    surface.save(&path).expect("save");
    let sidecar = clayspace_engine::adaptive::sidecar_for(&path);
    assert!(
        sidecar.exists(),
        "the surface is written beside the document"
    );

    let policy = BackendPolicy::discover(None).expect("backends");
    let mut reopened = ClayDocument::new(policy).expect("a document");
    reopened.open(&path).expect("reopen");
    let row = reopened
        .scene()
        .layers
        .last()
        .map(|layer| layer.representation)
        .expect("the row");
    assert_eq!(
        row,
        Representation::Dynamic,
        "never persisted or reopened as a mesh"
    );
    assert_eq!(
        drawn(&mut reopened),
        sculpted,
        "with the same triangles, connectivity and all"
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&sidecar);
}

/// Without its side-car the row opens as the mesh it was read from, rather
/// than the document being refused.
#[test]
fn a_missing_side_car_opens_the_mesh_the_surface_was_read_from() {
    let (mut surface, _) = with_a_surface("no-side-car");
    let read_from = drawn(&mut surface);
    assert!(dab(&mut surface, ToolKind::Padrao));

    let path = scratch("no-side-car", "clayspace");
    surface.save(&path).expect("save");
    std::fs::remove_file(clayspace_engine::adaptive::sidecar_for(&path))
        .expect("take the side-car away");

    let policy = BackendPolicy::discover(None).expect("backends");
    let mut reopened = ClayDocument::new(policy).expect("a document");
    reopened.open(&path).expect("the document still opens");
    let row = reopened
        .scene()
        .layers
        .last()
        .map(|layer| layer.representation)
        .expect("the row");
    assert_eq!(row, Representation::Mesh);
    assert_eq!(drawn(&mut reopened).1, read_from.1);
    assert!(reopened.dynamic_diagnostics().lost.is_empty());
    let _ = std::fs::remove_file(&path);
}

// -- crossing out -----------------------------------------------------------

/// Dynamic → Mesh keeps what the brush made, as a fixed mesh.
#[test]
fn a_surface_crosses_back_into_a_mesh_that_keeps_the_sculpt() {
    let (mut surface, _) = with_a_surface("back");
    assert!(dab(&mut surface, ToolKind::Padrao));
    let (_, sculpted) = drawn(&mut surface);
    let settings = ConversionSettings::default();
    let key = surface
        .convert_layer_in_place(Direction::DynamicToMesh, settings.cell_size, settings.blur)
        .expect("an adaptive surface bakes to a mesh");
    assert_eq!(representation_of(&surface, key), Representation::Mesh);
    assert_eq!(drawn(&mut surface).1, sculpted);
    assert_eq!(surface.dynamic_diagnostics().held, 0);
}

// -- colour -----------------------------------------------------------------

/// The colour brushes are refused on a surface that carries no colour, and
/// the refusal costs nothing: over one, the engine's paint would remesh and
/// colour nothing.
#[test]
fn a_colour_brush_is_refused_on_a_surface_with_no_colour() {
    let (mut surface, _) = with_a_surface("uncoloured");
    let before = drawn(&mut surface);
    let depth = surface.history().depth;
    for tool in [ToolKind::Pintar, ToolKind::Borrar] {
        surface.begin_gesture();
        let refused = surface.apply_stroke(
            tool,
            BrushSettings::default(),
            &[GestureSample {
                position: [0.0; 3],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        );
        surface.end_gesture();
        assert!(
            matches!(
                refused,
                Err(ModelError::Unavailable(
                    Unavailable::MissingAttribute { .. }
                ))
            ),
            "{} on an uncoloured surface: {refused:?}",
            tool.label()
        );
    }
    assert_eq!(drawn(&mut surface), before, "and nothing was remeshed");
    assert_eq!(surface.history().depth, depth);
}

/// Over a coloured surface, Paint writes colour.
#[test]
fn paint_colours_a_coloured_surface() {
    let (mut surface, _) = crossed(with_a_sheet("coloured", true).0);
    let grey = |document: &mut ClayDocument| {
        document
            .visible_mesh_geometry()
            .2
            .iter()
            .filter(|c| (c[0] - 0.5).abs() > 0.01)
            .count()
    };
    assert_eq!(grey(&mut surface), 0, "the sheet came across grey");
    assert!(dab(&mut surface, ToolKind::Pintar), "the paint lands");
    assert!(grey(&mut surface) > 0, "and wrote colour");
}
