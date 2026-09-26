//! Retopology through the domain's own interface.
//!
//! The engine tests beside this exercise the two libraries directly. This one
//! goes through `RetopoModel`, which is what a ViewModel will call — so it is
//! the first test that says the feature is *reachable* rather than merely
//! implemented.

use clayspace_engine::{BackendPolicy, ClayDocument, EngineRetopologiser};
use clayspace_model::{
    Combine, CombineSettings, ConformModel, ConformOutcome, ConformResult, Direction, LayerKey,
    ObjectModel, QuadMethod, Representation, RetopoModel, RetopoSettings, Retopologiser,
    SceneModel, SculptModel, Shape,
};

/// A sculpt that has become a mesh: the starting form, crossed.
fn meshed() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    document.convert_layer(Direction::SdfToMesh, 0.05, 0).ok()?;
    Some(document)
}

#[test]
fn an_in_place_retopology_rebuilds_the_subtool() {
    let Some(mut document) = meshed() else {
        return;
    };
    let subtools_before = document.scene().layers.len();
    let history_before = document.history().depth;
    let key_before = document
        .scene()
        .active_layer()
        .expect("an active layer")
        .key;
    let name_before = document
        .scene()
        .active_layer()
        .expect("an active layer")
        .name
        .clone();
    let triangles_before = document.visible_mesh_geometry().3.len() / 3;

    document
        .can_retopologise()
        .expect("a crossed mesh subtool can be retopologised");
    let outcome = document
        .retopologise(RetopoSettings {
            target_quads: 600,
            in_place: true,
            ..RetopoSettings::default()
        })
        .expect("the retopology runs");

    println!(
        "{} triangles -> {} faces / {} triangles, {:.0}% quads",
        outcome.triangles_before,
        outcome.faces,
        outcome.triangles,
        outcome.quad_share() * 100.0
    );

    // **What says these are quads.** The engine carries quads beside its own
    // triangulation rather than instead of it, so a quad mesh reports fewer
    // faces than triangles where a triangle mesh reports exactly as many.
    assert!(
        outcome.is_quads(),
        "the result has {} faces and {} triangles, which is what a triangle \
         mesh looks like",
        outcome.faces,
        outcome.triangles
    );
    assert!(outcome.triangles_before > 0);

    // **On the subtool itself**, because that is what `in_place` asks for:
    // the subtool in front of the sculptor rebuilt, as ZBrush's ZRemesher,
    // its Dynamesh and this application's own Rebuild do it.
    assert_eq!(
        document.scene().layers.len(),
        subtools_before,
        "the retopology added a subtool instead of rebuilding this one"
    );
    assert_eq!(
        document
            .scene()
            .active_layer()
            .expect("an active layer")
            .key,
        key_before,
        "the active subtool changed, so the sculptor is no longer looking at \
         what they retopologised"
    );
    assert_eq!(
        document
            .scene()
            .active_layer()
            .expect("an active layer")
            .name,
        name_before,
        "the subtool was renamed; a rebuild of its topology is not a new thing"
    );

    // And it is a different surface than it was.
    let triangles_after = document.visible_mesh_geometry().3.len() / 3;
    assert_ne!(
        triangles_after, triangles_before,
        "{triangles_before} triangles before and after, so nothing was replaced"
    );

    // One undo entry, as a rebuild and a crossing already are — and it is
    // what a sculptor compares against now that there is no second subtool
    // to look at, which is the same answer ZBrush gives.
    assert_eq!(
        document.history().depth,
        history_before + 1,
        "rebuilding the topology reached the history as more than one thing a \
         sculptor did"
    );
    assert!(document.undo().expect("undo"), "nothing to undo");
    assert_eq!(
        document.scene().layers.len(),
        subtools_before,
        "undoing a rebuild changed the subtool count"
    );
    assert_eq!(
        document.visible_mesh_geometry().3.len() / 3,
        triangles_before,
        "one undo did not put the original topology back"
    );
}

/// A field subtool is refused by name rather than crossed for the sculptor.
///
/// Crossing one silently would be a representation change with its own cost
/// and its own undo entry, and a tool that says it rebuilds topology must not
/// perform one.
#[test]
fn a_field_subtool_is_refused_and_the_reason_names_the_representation() {
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form) else {
        return;
    };
    assert_eq!(document.active_representation(), Representation::Sdf);
    let refusal = document
        .can_retopologise()
        .expect_err("a field is not a mesh");
    assert!(
        refusal.contains("Sdf"),
        "the refusal does not say which representation is in the way: {refusal}"
    );
}

/// Every method the domain offers is one the engine accepts.
///
/// Walked rather than spot-checked: the list is the domain's and the
/// enumerants are the engine's, so a method added without a value behind it
/// fails on the row that was added rather than the first time a sculptor
/// picks it.
#[test]
fn every_quad_method_the_domain_offers_runs() {
    for method in QuadMethod::ALL {
        let Some(mut document) = meshed() else {
            return;
        };
        let outcome = document
            .retopologise(RetopoSettings {
                target_quads: 300,
                method,
                ..RetopoSettings::default()
            })
            .unwrap_or_else(|e| panic!("{} was offered and refused: {e}", method.label()));
        println!(
            "  {:<18} {} faces, {} triangles",
            method.label(),
            outcome.faces,
            outcome.triangles
        );
        assert!(outcome.faces > 0);
    }
}

/// The **placed layer** carries the quads, not just the engine's report.
///
/// This is the assertion that was missing, and its absence is why the feature
/// shipped looking broken. `a_mesh_subtool_becomes_quads_in_a_new_subtool`
/// asserts `outcome.is_quads()` — fewer faces than triangles — which is a fact
/// about the *retopologiser's* mesh. Nothing checked what arrived in the
/// document. ClayCore's C ABI has exactly one mesh constructor,
/// `clay_mesh_from_triangles`, so what crossed was the fan triangulation and
/// the quads had nowhere to live; the polyframe derived its wireframe from
/// those triangles and drew every diagonal. A 100%-quad retopology was
/// indistinguishable on screen from a triangle mesh, and a sculptor reported
/// exactly that.
///
/// So the faces are kept beside the layer and travel to the viewport on the
/// span. What this asserts is that they arrive, that they are the *quads* and
/// not the triangulation, and that the source layer beside them is untouched.
#[test]
fn the_placed_layer_carries_the_quads_the_viewport_will_draw() {
    let Some(mut document) = meshed() else {
        return;
    };
    let outcome = document
        .retopologise(RetopoSettings {
            target_quads: 600,
            ..RetopoSettings::default()
        })
        .expect("the retopology runs");
    assert!(outcome.is_quads(), "the fixture did not produce quads");

    let (positions, _, _, indices, spans) = document.visible_mesh_geometry();
    let authored: Vec<_> = spans.iter().filter(|s| s.edges.is_some()).collect();
    assert_eq!(
        authored.len(),
        1,
        "expected exactly one layer to carry authored faces, found {} of {}",
        authored.len(),
        spans.len()
    );
    let edges = authored[0].edges.as_ref().expect("the authored edges");

    assert_eq!(edges.len() % 2, 0, "two vertex indices per edge");
    assert!(!edges.is_empty(), "the placed layer carries no edges");

    // The quads, not their triangulation. A pure-quad mesh of F faces has 2F
    // edges; its fan triangulation has 3F — the same 2F plus one diagonal per
    // quad. So `< 3F` is what says the diagonals are absent, and a buffer
    // that had included them could not satisfy it.
    let faces = outcome.faces;
    assert!(
        edges.len() / 2 < faces * 3,
        "{} edges for {faces} faces — that is the triangulation's edge count, \
         so the polyframe would draw diagonals and the quads would not show",
        edges.len() / 2
    );

    // Rebased onto the shared buffer, so the viewport joins the right points.
    // Off-by-one here is a wireframe of the wrong shape rather than a crash,
    // which is why it is asserted rather than trusted.
    let vertices = positions.len() as u32;
    assert!(
        edges.iter().all(|&v| v < vertices),
        "an authored edge indexes past the {vertices} vertices uploaded"
    );
    let range = &authored[0].indices;
    assert!(
        (range.end as usize) <= indices.len(),
        "the span names indices the buffer does not have"
    );

    // And the interface can now say so. The Polygons row was drawn from the
    // triangle count, so it printed the same number twice and could not
    // express a quad mesh at all.
    let stats = document.stats();
    let reported = stats.faces.expect("a quad-carrying scene reports faces");
    assert!(
        reported < stats.triangles,
        "the scene reports {reported} faces and {} triangles, which is what a \
         triangle mesh reports",
        stats.triangles
    );
    println!(
        "  {} spans, {} authored edges, scene reports {reported} faces / {} triangles",
        spans.len(),
        edges.len() / 2,
        stats.triangles
    );
}

/// A retopologised subtool and an ordinary one draw correctly side by side.
///
/// The wireframe is decided **per subtool**, not per scene, and this is what
/// says so. A retopology rebuilds one subtool in place and leaves the others
/// alone, so a scene holding both is the ordinary case rather than a corner —
/// and an all-or-nothing rule (authored edges only when *every* visible layer
/// has them, or derived only when none does) would draw one of the two wrong.
///
/// Both halves are asserted because only the pair distinguishes the design
/// from a rule that happened to be right about one of them: exactly one span
/// carries authored faces and exactly one derives.
#[test]
fn a_retopologised_subtool_and_a_triangle_one_keep_their_own_wireframes() {
    let Some(mut document) = meshed() else {
        return;
    };
    // A second mesh subtool beside the first, so the scene is mixed.
    let Ok(second) = document.add_layer("Segunda", Representation::Sdf) else {
        return;
    };
    if document.set_active_layer(second).is_err() {
        return;
    }
    if document
        .place_object(
            Shape::Sphere,
            &[0.6],
            [0.0; 3],
            CombineSettings {
                op: Combine::Add,
                ..CombineSettings::default()
            },
        )
        .is_err()
    {
        return;
    }
    if document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .is_err()
    {
        return;
    }

    let retopologised = document
        .scene()
        .active_layer()
        .expect("an active layer")
        .key;
    document
        .retopologise(RetopoSettings {
            target_quads: 400,
            in_place: true,
            ..RetopoSettings::default()
        })
        .expect("the retopology runs");

    let (_, _, _, _, spans) = document.visible_mesh_geometry();
    assert_eq!(
        spans.len(),
        2,
        "the fixture wanted two visible mesh subtools, found {}",
        spans.len()
    );
    let with: Vec<_> = spans.iter().filter(|s| s.edges.is_some()).collect();
    let without: Vec<_> = spans.iter().filter(|s| s.edges.is_none()).collect();
    assert_eq!(
        with.len(),
        1,
        "expected one subtool carrying authored faces, found {}",
        with.len()
    );
    assert_eq!(
        without.len(),
        1,
        "expected one subtool whose faces are its triangles, found {}",
        without.len()
    );
    assert_eq!(
        with[0].layer, retopologised,
        "the authored faces are on the subtool that was not retopologised"
    );
}

/// The mean angle, in degrees, between each drawn vertex normal and the
/// triangles it is drawn with.
///
/// What lighting reads. A surface drawn with one normal everywhere — what a
/// retopology was drawn with, having come back with none — scores near ninety
/// on a closed form, and one lit by its own shape scores in single digits.
fn shading_error(positions: &[[f32; 3]], normals: &[[f32; 3]], indices: &[u32]) -> f32 {
    let (mut total, mut samples) = (0.0f32, 0u32);
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|i| positions[i as usize]);
        let (u, v) = (
            [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
            [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
        );
        let face = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let length = (face[0] * face[0] + face[1] * face[1] + face[2] * face[2]).sqrt();
        if length < 1e-12 {
            continue;
        }
        for &corner in triangle {
            let n = normals[corner as usize];
            let cos = (n[0] * face[0] + n[1] * face[1] + n[2] * face[2]) / length;
            total += cos.clamp(-1.0, 1.0).acos().to_degrees();
            samples += 1;
        }
    }
    total / samples.max(1) as f32
}

/// A retopology is drawn lit, and so is a hierarchy built over it.
///
/// Both came back flat: `clay_mesh_from_triangles` takes no normals, so the
/// layer a retopology lands in holds none, and a hierarchy exports a level's
/// normals only where its cage had them — the one absence, inherited at every
/// level. The viewport's stand-in was a single `+y` for every vertex, which
/// lights the whole form as one colour. Measured here as what the lighting
/// reads: how far the drawn normals sit from the triangles they light.
#[test]
fn a_retopology_and_a_hierarchy_built_from_it_are_lit() {
    let Some(mut document) = meshed() else {
        return;
    };
    let source = active_key(&document);
    document
        .retopologise(RetopoSettings {
            target_quads: 600,
            ..RetopoSettings::default()
        })
        .expect("the retopology runs");
    // Only the result is measured: the sculpt it was made from stays beside
    // it, lit by its own normals, and would hide a flat result.
    document
        .set_layer_visible(source, false)
        .expect("the source can be hidden");
    let (positions, normals, _, indices, _) = document.visible_mesh_geometry();
    let retopology = shading_error(&positions, &normals, &indices);
    assert!(
        retopology < 20.0,
        "the retopology's drawn normals sit {retopology:.1} degrees off its \
         triangles on average, which is a surface lit as one colour"
    );

    let settings = clayspace_model::ConversionSettings::default();
    document
        .convert_layer_in_place(Direction::MeshToMultires, settings.cell_size, settings.blur)
        .expect("a quad retopology of a closed form is a cage");
    document
        .apply_multires_level_op(clayspace_model::MultiresLevelOp::AddLevel)
        .expect("one level over it");
    let (positions, normals, _, indices, _) = document.visible_mesh_geometry();
    let hierarchy = shading_error(&positions, &normals, &indices);
    assert!(
        hierarchy < 20.0,
        "the hierarchy over the retopology draws its level {hierarchy:.1} \
         degrees off its triangles on average — unlit, like the cage it came from"
    );
}

fn active_key(document: &ClayDocument) -> LayerKey {
    document
        .scene()
        .active_layer()
        .expect("an active layer")
        .key
}

/// One subtool's drawn triangles and the box they fill, read from the spans
/// the viewport is handed.
fn drawn(document: &mut ClayDocument, layer: LayerKey) -> Option<(usize, [f32; 3], [f32; 3])> {
    let (positions, _, _, indices, spans) = document.visible_mesh_geometry();
    let span = spans.iter().find(|span| span.layer == layer)?;
    let range = span.indices.start as usize..span.indices.end as usize;
    let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
    for &vertex in &indices[range.clone()] {
        let point = positions[vertex as usize];
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    Some((range.len() / 3, min, max))
}

/// Retopologises the active mesh subtool with the defaults and checks the
/// production crossing's contract: a new fixed-mesh layer, standing where the
/// source stands, quads rather than triangles, the source untouched, and one
/// undo that takes the whole thing back.
fn retopologised_beside_the_source(mut document: ClayDocument, what: &str) {
    let source = active_key(&document);
    let subtools_before = document.scene().layers.len();
    let history_before = document.history().depth;
    let (triangles_before, min_before, max_before) =
        drawn(&mut document, source).expect("the source is drawn");

    let outcome = document
        .retopologise(RetopoSettings {
            target_quads: 600,
            ..RetopoSettings::default()
        })
        .unwrap_or_else(|e| panic!("{what}: the retopology runs: {e}"));
    assert!(outcome.is_quads(), "{what}: the result is not quads");

    let scene = document.scene();
    assert_eq!(
        scene.layers.len(),
        subtools_before + 1,
        "{what}: the result did not arrive as a new subtool"
    );
    let result = scene.active_layer().expect("an active layer");
    assert_ne!(result.key, source, "{what}: the source was rebuilt");
    assert_eq!(result.representation, Representation::Mesh);
    assert!(
        result.name.ends_with("quads"),
        "{what}: the result is called {:?}",
        result.name
    );
    let result = result.key;

    // The source, exactly as it was.
    let (triangles_after, min_after, max_after) =
        drawn(&mut document, source).expect("the source is still drawn");
    assert_eq!(
        triangles_after, triangles_before,
        "{what}: the source's topology changed"
    );
    assert_eq!(
        (min_after, max_after),
        (min_before, max_before),
        "{what}: the source moved"
    );

    // The result, where the source is: same box to within the surface the
    // quadrangulator re-samples.
    let (triangles, min, max) = drawn(&mut document, result).expect("the result is drawn");
    assert_eq!(
        triangles, outcome.triangles,
        "{what}: the layer holds a different mesh than the engine reported"
    );
    for axis in 0..3 {
        let extent = max_before[axis] - min_before[axis];
        let tolerance = extent * 0.1;
        assert!(
            (min[axis] - min_before[axis]).abs() <= tolerance
                && (max[axis] - max_before[axis]).abs() <= tolerance,
            "{what}: the result's box {min:?}..{max:?} does not match the source's \
             {min_before:?}..{max_before:?} on axis {axis}"
        );
    }

    // One thing a sculptor did, and one undo takes it back whole.
    assert_eq!(
        document.history().depth,
        history_before + 1,
        "{what}: the retopology reached the history as more than one entry"
    );
    assert!(document.undo().expect("undo"), "{what}: nothing to undo");
    assert_eq!(
        document.scene().layers.len(),
        subtools_before,
        "{what}: undo left the result layer standing"
    );
    assert_eq!(
        drawn(&mut document, source).map(|(triangles, _, _)| triangles),
        Some(triangles_before),
        "{what}: undo disturbed the source"
    );
}

/// SDF → mesh → retopology → a new fixed-mesh layer.
#[test]
fn a_field_sculpt_retopologises_into_a_new_fixed_mesh_layer() {
    let Some(document) = meshed() else {
        return;
    };
    retopologised_beside_the_source(document, "SDF");
}

/// Voxel → mesh → retopology → a new fixed-mesh layer.
#[test]
fn a_voxel_sculpt_retopologises_into_a_new_fixed_mesh_layer() {
    let Some(policy) = BackendPolicy::discover(None).ok() else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };
    document
        .convert_layer(Direction::SdfToVoxel, 0.05, 0)
        .expect("the starting form rasterises");
    document
        .convert_layer(Direction::VoxelToMesh, 0.05, 0)
        .expect("the grid meshes");
    retopologised_beside_the_source(document, "Voxel");
}

/// A result whose source moved while the work ran is not published.
///
/// The work runs off the interface thread and the sculpt stays editable, so
/// the revision read with the source is checked at publish time — here with
/// the new-layer placement, which has no engine compare-and-swap to lean on.
#[test]
fn a_stale_result_is_not_published() {
    let Some(mut document) = meshed() else {
        return;
    };
    let source = document.retopo_source().expect("the source is read");
    let subtools_before = document.scene().layers.len();

    // The sculpt moves while the "worker" runs: every vertex nudged, which is
    // a real edit through a real path and moves the layer's revision.
    let nudged: Vec<[f32; 3]> = source
        .positions
        .iter()
        .map(|p| [p[0] + 0.01, p[1], p[2]])
        .collect();
    document
        .apply_conform(&ConformResult {
            positions: nudged,
            outcome: ConformOutcome {
                moved_vertices: source.positions.len(),
                max_deviation: 0.01,
                rms_deviation: 0.01,
                flagged_count: 0,
                flagged_returned: 0,
            },
        })
        .expect("the source can be edited");
    assert_ne!(
        document.retopo_source_revision().ok(),
        Some(source.revision),
        "the edit did not move the source's revision, so this test proves nothing"
    );

    let result = EngineRetopologiser
        .run(
            &source,
            RetopoSettings {
                target_quads: 400,
                ..RetopoSettings::default()
            },
            &|_, _| {},
            &|| false,
        )
        .expect("the retopology runs");
    let refused = document.place_retopology(&result, RetopoSettings::default());
    assert!(refused.is_err(), "a stale retopology was published");
    assert_eq!(
        document.scene().layers.len(),
        subtools_before,
        "a refused publish still added a layer"
    );
}
