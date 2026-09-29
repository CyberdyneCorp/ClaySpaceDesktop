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
    ImportSettings, LayerKey, ModelError, Refusal, Representation, SceneModel, SculptModel,
    ToolKind, ToolNote, Unavailable,
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

/// The triangles the viewport is handed, as (corner positions, triangle
/// count).
///
/// Degenerate triangles are left out — an adaptive surface is drawn in chunk
/// slots whose headroom is zero-area triangles — and the rest are put in a
/// canonical order, each starting at its smallest corner with its winding
/// kept, so two drawings of the same surface compare equal whatever the
/// vertex numbering or the chunk layout.
fn drawn(document: &mut ClayDocument) -> (Vec<[f32; 3]>, usize) {
    let (positions, _, _, indices, _) = document.visible_mesh_geometry();
    let mut triangles: Vec<[[f32; 3]; 3]> = indices
        .chunks_exact(3)
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
        .map(|t| {
            let corners = [t[0], t[1], t[2]].map(|i| positions[i as usize]);
            let first = (0..3)
                .min_by(|&a, &b| corners[a].partial_cmp(&corners[b]).expect("finite"))
                .unwrap_or(0);
            std::array::from_fn(|k| corners[(first + k) % 3])
        })
        .collect();
    triangles.sort_by(|a, b| a.partial_cmp(b).expect("finite positions"));
    let count = triangles.len();
    (triangles.into_iter().flatten().collect(), count)
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

// -- the explicit crossing (#208) -------------------------------------------

/// How far a vertex may move across Mesh → Dynamic → Mesh: neither crossing
/// samples anything, so the only movement is float round-off in the weld.
const ROUND_TRIP_TOLERANCE: f32 = 1e-5;

/// The distinct vertex positions a drawing holds, in a stable order, so two
/// drawings of the same form compare equal whatever the vertex numbering.
fn form(document: &mut ClayDocument) -> Vec<[f32; 3]> {
    let (mut positions, _) = drawn(document);
    positions.sort_by(|a, b| a.partial_cmp(b).expect("finite positions"));
    positions.dedup();
    positions
}

fn assert_same_form(before: &[[f32; 3]], after: &[[f32; 3]]) {
    assert_eq!(before.len(), after.len(), "the vertex count moved");
    for (a, b) in before.iter().zip(after) {
        let moved = (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f32::max);
        assert!(
            moved <= ROUND_TRIP_TOLERANCE,
            "a vertex moved {moved} across the round trip: {a:?} became {b:?}"
        );
    }
}

fn in_place(document: &mut ClayDocument, direction: Direction) -> Result<LayerKey, ModelError> {
    let settings = ConversionSettings::default();
    document.convert_layer_in_place(direction, settings.cell_size, settings.blur)
}

fn beside(document: &mut ClayDocument, direction: Direction) -> Result<LayerKey, ModelError> {
    let settings = ConversionSettings::default();
    document.convert_layer(direction, settings.cell_size, settings.blur)
}

fn rows(document: &ClayDocument) -> Vec<(LayerKey, Representation)> {
    document
        .scene()
        .layers
        .iter()
        .map(|layer| (layer.key, layer.representation))
        .collect()
}

/// Mesh → Dynamic → Mesh gives back the form it was given, within
/// [`ROUND_TRIP_TOLERANCE`], with the triangle count it had.
#[test]
fn mesh_to_dynamic_and_back_preserves_the_form() {
    let (mut document, _) = with_a_mesh("round-trip-form");
    let (_, triangles) = drawn(&mut document);
    let before = form(&mut document);

    in_place(&mut document, Direction::MeshToDynamic).expect("into Dynamic");
    assert_same_form(&before, &form(&mut document));
    let key = in_place(&mut document, Direction::DynamicToMesh).expect("frozen");

    assert_eq!(representation_of(&document, key), Representation::Mesh);
    assert_eq!(drawn(&mut document).1, triangles);
    assert_same_form(&before, &form(&mut document));
}

/// Each crossing is one undo step, and undo puts back the representation it
/// left with its geometry intact — never an empty layer of the new one.
#[test]
fn a_conversion_is_one_undo_step() {
    let (mut document, mesh) = with_a_mesh("one-step");
    let fixed = drawn(&mut document);
    let depth = document.history().depth;

    let surface = in_place(&mut document, Direction::MeshToDynamic).expect("into Dynamic");
    assert_eq!(document.history().depth, depth + 1);
    assert!(dab(&mut document, ToolKind::Padrao), "the sculpt lands");
    let sculpted = drawn(&mut document);
    let frozen = in_place(&mut document, Direction::DynamicToMesh).expect("frozen");
    assert_eq!(document.history().depth, depth + 3);
    assert_eq!(
        drawn(&mut document),
        sculpted,
        "the freeze keeps the sculpt"
    );

    // Back through the freeze: the adaptive surface, as the stroke left it.
    assert!(document.undo().expect("undo the freeze"));
    assert_eq!(
        representation_of(&document, surface),
        Representation::Dynamic
    );
    assert!(document.scene().layer(frozen).is_none());
    assert_eq!(document.dynamic_diagnostics().held, 1);
    assert_eq!(drawn(&mut document), sculpted);

    // Back through the stroke and the crossing: the mesh, exactly.
    assert!(document.undo().expect("undo the stroke"));
    assert!(document.undo().expect("undo the crossing"));
    assert_eq!(document.history().depth, depth);
    assert_eq!(representation_of(&document, mesh), Representation::Mesh);
    assert!(document.scene().layer(surface).is_none());
    assert_eq!(document.scene().active, Some(mesh));
    assert_eq!(document.dynamic_diagnostics().held, 0);
    assert_eq!(drawn(&mut document), fixed);

    // And forward again, one step per crossing.
    assert!(document.redo().expect("redo the crossing"));
    assert_eq!(
        representation_of(&document, surface),
        Representation::Dynamic
    );
    assert!(document.redo().expect("redo the stroke"));
    assert!(document.redo().expect("redo the freeze"));
    assert_eq!(representation_of(&document, frozen), Representation::Mesh);
    assert_eq!(drawn(&mut document), sculpted);
}

/// A crossing past the budget is refused with the engine's estimate and the
/// limit, and changes nothing: not the rows, not the selection, not the
/// history.
#[test]
fn an_over_budget_conversion_is_refused() {
    for direction in [Direction::MeshToDynamic, Direction::DynamicToMesh] {
        let (mut document, key) = match direction {
            Direction::MeshToDynamic => with_a_mesh("over-budget-in"),
            _ => with_a_surface("over-budget-out"),
        };
        let settings = ConversionSettings::default();
        let price = document
            .conversion_cost(direction, settings.cell_size)
            .and_then(|cost| cost.surface)
            .expect("an adaptive crossing is priced by the engine");
        assert!(price.peak_bytes > 0);
        assert_eq!(
            price.budget_bytes,
            clayspace_engine::adaptive::CROSSING_BUDGET
        );

        let (before, depth) = (rows(&document), document.history().depth);
        let geometry = drawn(&mut document);
        document.set_surface_budget(price.peak_bytes / 2);

        for run in [in_place, beside] {
            let refused = run(&mut document, direction).expect_err("past the budget");
            match refused {
                ModelError::Conversion(Refusal::CrossingOverBudget {
                    direction: said,
                    peak_bytes,
                    budget_bytes,
                    ..
                }) => {
                    assert_eq!(said, direction);
                    assert_eq!(peak_bytes, price.peak_bytes, "the engine's estimate");
                    assert_eq!(budget_bytes, price.peak_bytes / 2, "the limit");
                }
                other => panic!("{direction:?} refused for the wrong reason: {other}"),
            }
            assert_eq!(rows(&document), before, "{direction:?} changed the rows");
            assert_eq!(document.scene().active, Some(key));
            assert_eq!(document.history().depth, depth);
            assert_eq!(drawn(&mut document), geometry);
        }

        // The same crossing at the default budget runs.
        document.set_surface_budget(clayspace_engine::adaptive::CROSSING_BUDGET);
        assert!(in_place(&mut document, direction).is_ok());
    }
}

/// Transform, name and visibility survive Mesh → Dynamic → Mesh in place.
#[test]
fn transform_name_and_visibility_survive_the_round_trip() {
    use clayspace_model::{GizmoTarget, ObjectModel};
    let (mut document, mesh) = with_a_mesh("identity");
    let name = document.scene().layer(mesh).expect("the row").name.clone();
    document
        .set_layer_transform(mesh, [1.0, 2.0, 3.0], 1.5)
        .expect("place the sheet");
    document.set_layer_visible(mesh, false).expect("hide it");
    let placed = document
        .target_transform(GizmoTarget::Layer(mesh))
        .expect("a placed layer");

    let surface = in_place(&mut document, Direction::MeshToDynamic).expect("into Dynamic");
    let row = document.scene().layer(surface).expect("the row").clone();
    assert!(!row.visible, "a hidden layer stays hidden");
    assert_eq!(
        document.target_transform(GizmoTarget::Layer(surface)),
        Some(placed),
        "the surface stands where the mesh stood"
    );

    let frozen = in_place(&mut document, Direction::DynamicToMesh).expect("frozen");
    let row = document.scene().layer(frozen).expect("the row").clone();
    assert_eq!(row.name, name, "the round trip gives the name back");
    assert!(!row.visible);
    assert_eq!(
        document.target_transform(GizmoTarget::Layer(frozen)),
        Some(placed)
    );
}

/// The default crossing adds a layer and keeps the original sculpt beside
/// it, standing where the original stands.
#[test]
fn the_default_crossing_keeps_the_original() {
    use clayspace_model::{GizmoTarget, ObjectModel};
    let (mut document, mesh) = with_a_mesh("beside");
    document
        .set_layer_transform(mesh, [0.5, 0.0, -0.5], 2.0)
        .expect("place the sheet");
    let placed = document.target_transform(GizmoTarget::Layer(mesh));
    let surface = beside(&mut document, Direction::MeshToDynamic).expect("into Dynamic");
    assert_eq!(representation_of(&document, mesh), Representation::Mesh);
    assert_eq!(
        representation_of(&document, surface),
        Representation::Dynamic
    );
    assert_eq!(
        document.target_transform(GizmoTarget::Layer(surface)),
        placed
    );
}

/// No brush changes what a layer is: every tool, stroked on every
/// representation the document holds, leaves each row the representation it
/// was — a tool with no binding is refused, never satisfied by a crossing.
#[test]
fn no_tool_converts_a_layer() {
    let (mut document, mesh) = with_a_mesh("no-tool-converts");
    for direction in [
        Direction::MeshToVoxel,
        Direction::MeshToMultires,
        Direction::MeshToDynamic,
    ] {
        document.set_active_layer(mesh).expect("back to the mesh");
        beside(&mut document, direction).expect("a crossing beside the mesh");
    }
    let before = rows(&document);
    let held: std::collections::BTreeSet<&str> = before
        .iter()
        .map(|(_, representation)| representation.label())
        .collect();
    assert_eq!(held.len(), Representation::ALL.len(), "one row of each");

    let brush = BrushSettings {
        size: 0.6,
        intensity: 0.5,
        ..BrushSettings::default()
    };
    let samples = [
        GestureSample {
            position: [-0.2, 0.0, 0.0],
            pressure: 1.0,
            time: 0.0,
        },
        GestureSample {
            position: [0.2, 0.0, 0.0],
            pressure: 1.0,
            time: 0.1,
        },
    ];
    for (key, representation) in before.clone() {
        document.set_active_layer(key).expect("activate");
        for tool in ToolKind::ALL {
            document.begin_gesture();
            let _ = document.apply_stroke(tool, brush, &samples, [false; 3]);
            document.end_gesture();
            assert_eq!(
                document.active_representation(),
                representation,
                "{} changed a {} layer",
                tool.label(),
                representation.label()
            );
        }
    }
    assert_eq!(rows(&document), before, "no stroke added or crossed a row");
}
