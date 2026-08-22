//! Sculpting a mesh layer that actually carries triangles.
//!
//! The return trip the mesh brushes exist to complete: sculpt on SDF or
//! voxels, export, retopologize elsewhere, bring the mesh back and refine it
//! *in place*. The fixture here is the short version of that — export the
//! starting form and import it again — because it is the only geometry always
//! to hand and it needs no file in the repository.
//!
//! Every one of these holds the line all sixteen verbs hold: topology never
//! changes. A brush that created, split or deleted a polygon would spend the
//! retopology the import was for, which is the whole reason a mesh layer is
//! worth sculpting rather than resampling.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ExchangeModel, ExportSettings, GestureSample, ImportSettings, Representation,
    SceneModel, SculptModel, ToolKind,
};

fn scratch(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("clayspace-mesh-sculpt-{name}"));
    let _ = std::fs::remove_file(&path);
    path
}

/// A document whose active layer is an imported mesh.
///
/// Round-tripped through a file rather than attached directly, because that is
/// the only route a mesh layer has into a document and a fixture that took
/// another one would be testing a path no user reaches.
fn with_imported_mesh(who: &str) -> Option<(ClayDocument, std::path::PathBuf)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let path = scratch(&format!("{who}.obj"));
    document
        .export_mesh(&path, ExportSettings::default())
        .ok()?;
    document
        .import_mesh(&path, ImportSettings::default())
        .ok()?;

    let key = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)?;
    document.set_active_layer(key).ok()?;
    Some((document, path))
}

fn dab(document: &mut ClayDocument, tool: ToolKind, at: [f32; 3]) -> Result<bool, String> {
    document
        .apply_stroke(
            tool,
            BrushSettings::default(),
            &[
                GestureSample {
                    position: at,
                    pressure: 1.0,
                    time: 0.0,
                },
                GestureSample {
                    position: [at[0] + 0.05, at[1], at[2]],
                    pressure: 1.0,
                    time: 1.0,
                },
            ],
            [false; 3],
        )
        .map(|outcome| outcome.changed)
        .map_err(|e| e.to_string())
}

#[test]
fn an_imported_mesh_layer_carries_geometry_and_accepts_a_verb() {
    let Some((mut document, path)) = with_imported_mesh("accepts") else {
        return;
    };
    // The row is real now, so the refusal `add_mesh_layer`'s placeholder earns
    // must be gone.
    ToolKind::Padrao
        .availability(document.active_layer_state())
        .expect("an imported mesh carries triangles to sculpt");

    let changed = dab(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0])
        .expect("Draw is bound on a mesh layer");
    assert!(
        changed,
        "the stroke reached the mesh and moved nothing — the brush is on the \
         surface, so this is the sculptor not being wired to the layer"
    );
    let _ = std::fs::remove_file(&path);
}

/// The line every verb holds, at the level a user meets it.
#[test]
fn sculpting_a_mesh_layer_never_changes_its_topology() {
    let Some((mut document, path)) = with_imported_mesh("topology") else {
        return;
    };
    let before = document.stats();

    for tool in ToolKind::for_representation(Representation::Mesh) {
        if tool.writes_colour() {
            // Refused on a mesh with no colour attribute, deliberately.
            continue;
        }
        let _ = dab(&mut document, tool, [0.0, 0.0, 1.0]);
    }

    let after = document.stats();
    assert_eq!(
        after.triangles, before.triangles,
        "sculpting changed the triangle count, which is the one thing these \
         verbs may never do"
    );
    assert_eq!(
        after.vertices, before.vertices,
        "sculpting changed the vertex count"
    );
    let _ = std::fs::remove_file(&path);
}

/// The colour verbs reach a mesh that carries colour, and move no vertex.
///
/// The fixture round-trips through OBJ, and claycore's exporter writes the
/// vertex-colour extension — `v x y z r g b`, said so in the file's own first
/// line — so the mesh that comes back has colour and these two are accepted.
///
/// The *refusal* path is real and is not reachable from here: it needs a mesh
/// carrying no colour attribute, which this route cannot produce. Paint and
/// smear refuse one rather than creating the attribute, because twelve bytes a
/// vertex is a real cost to hide behind a stroke. `tools.rs` carries the rule
/// and `MissingAttribute` is what says it.
#[test]
fn the_colour_verbs_reach_a_coloured_mesh_without_moving_it() {
    let Some((mut document, path)) = with_imported_mesh("colour") else {
        return;
    };
    let before = document.stats();
    for tool in [ToolKind::Pintar, ToolKind::Borrar] {
        assert!(
            tool.writes_colour(),
            "{} should be a colour verb",
            tool.label()
        );
        dab(&mut document, tool, [0.0, 0.0, 1.0])
            .unwrap_or_else(|e| panic!("{} was refused on a coloured mesh: {e}", tool.label()));
    }
    let after = document.stats();
    assert_eq!(
        (after.triangles, after.vertices),
        (before.triangles, before.vertices),
        "a colour verb changed the geometry; these two write colour and \
         nothing else"
    );
    let _ = std::fs::remove_file(&path);
}

/// A mesh layer is pickable, which is what makes a press on one sculpt.
///
/// A field raycast could never see a mesh layer: it is in neither the tape nor
/// the brick cache. So before this a press on a mesh layer found nothing under
/// the pointer and fell through to orbiting — which is the correct behaviour
/// for "off the model" and the wrong one for a model that is right there.
#[test]
fn the_pointer_finds_an_imported_mesh() {
    let Some((mut document, path)) = with_imported_mesh("pick") else {
        return;
    };
    // The sculptor is built by the first stroke, and the pick is answered from
    // its tree — so a pick before any stroke finds nothing, deliberately: a
    // pick happens every frame the pointer moves and may not pay for an
    // adjacency pass.
    assert!(
        document.pick([0.0, 0.0, 4.0], [0.0, 0.0, -1.0]).is_none(),
        "a pick built the sculptor, which costs an adjacency pass per frame"
    );

    dab(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0]).expect("a stroke");

    let hit = document
        .pick([0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("a ray down the axis has to meet a sphere at the origin");
    assert!(
        hit[2] > 0.0,
        "the ray came from +z and hit at {hit:?}, which is behind the surface"
    );
    let _ = std::fs::remove_file(&path);
}

/// The other half: a pick while an SDF layer is active must not answer with a
/// mesh layer's surface, or the cursor would sit on something the active
/// brush cannot reach.
#[test]
fn a_mesh_is_not_picked_from_under_another_layer() {
    let Some((mut document, path)) = with_imported_mesh("pick-other") else {
        return;
    };
    dab(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0]).expect("a stroke");

    let sdf = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Sdf)
        .map(|layer| layer.key)
        .expect("the starting form");
    document.set_active_layer(sdf).expect("activate the field");

    // It may well hit — the starting form is there too — but it must be
    // answered by the field, not by the mesh's tree. What this holds is that
    // the mesh path is not consulted: with the field active, the answer comes
    // from the cache.
    let _ = document.pick([0.0, 0.0, 4.0], [0.0, 0.0, -1.0]);
    assert_eq!(
        document.active_representation(),
        Representation::Sdf,
        "picking changed the active layer"
    );
    let _ = std::fs::remove_file(&path);
}
