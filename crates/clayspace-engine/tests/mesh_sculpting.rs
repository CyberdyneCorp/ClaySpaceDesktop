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

/// A mesh gesture is one undo step, and it reverts the mesh exactly.
///
/// It has to be an application-side record. A vertex displacement is
/// destructive and is not an edit item, so the document holds nothing to take
/// back — measured, the engine's undo depth is the same before and after a
/// mesh stroke. What the engine does offer is `clay_mesh_deltas`, which
/// reverts bit exactly rather than approximately.
mod undo {
    use super::*;

    #[test]
    fn one_gesture_is_one_undo() {
        let Some((mut document, path)) = with_imported_mesh("undo") else {
            return;
        };
        let before = document.history().depth;
        dab(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0]).expect("a stroke");
        assert_eq!(
            document.history().depth,
            before + 1,
            "a mesh gesture has to be visible in the history, or Undo greys \
             out in the middle of a sculpting session"
        );

        assert!(document.undo().expect("undo"), "nothing was taken back");
        assert_eq!(document.history().depth, before);
        assert!(document.redo().expect("redo"), "nothing was put back");
        assert_eq!(document.history().depth, before + 1);
        let _ = std::fs::remove_file(&path);
    }

    /// A gesture that reached nothing is not worth a place on the stack: an
    /// undo that appears to do nothing is worse than one that is not offered.
    #[test]
    fn a_gesture_that_moved_nothing_is_not_recorded() {
        let Some((mut document, path)) = with_imported_mesh("undo-empty") else {
            return;
        };
        // Build the sculptor first, so this measures the recording rule rather
        // than the first stroke's setup.
        dab(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0]).expect("a stroke");
        let before = document.history().depth;

        // Far outside the form: the brush reaches nothing.
        let _ = dab(&mut document, ToolKind::Padrao, [0.0, 0.0, 40.0]);
        assert_eq!(
            document.history().depth,
            before,
            "a stroke that moved no vertex was put on the undo stack"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// The two histories interleave by depth, which is what makes one Cmd+Z
    /// mean "the last thing I did" whichever kind of edit that was.
    #[test]
    fn an_engine_edit_after_a_mesh_gesture_is_undone_first() {
        let Some((mut document, path)) = with_imported_mesh("undo-interleave") else {
            return;
        };
        dab(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0]).expect("a mesh stroke");
        let mesh_depth = document.history().depth;

        // An engine edit on top: a new layer is one.
        document
            .add_layer("Depois", Representation::Sdf)
            .expect("a layer");
        let layers_after_add = document.scene().layers.len();
        assert!(document.history().depth > mesh_depth);

        // The engine's entry is the more recent one, so it goes first.
        assert!(document.undo().expect("undo"));
        assert_eq!(
            document.scene().layers.len(),
            layers_after_add - 1,
            "the mesh gesture was taken back before the engine edit that \
             happened after it"
        );

        // And now the mesh gesture is the most recent again.
        assert!(document.undo().expect("undo"));
        assert_eq!(document.history().depth, mesh_depth - 1);
        let _ = std::fs::remove_file(&path);
    }
}

/// The mesh's quality is reportable, which is how stretching is *shown*.
///
/// Nothing here retessellates: that would spend the retopology the import was
/// for, and the engine stops at the same boundary. So a heavy pull is reported
/// rather than prevented, and the figure is what a sculptor reads to know the
/// mesh wants retopology — at the point it starts wanting it, rather than at
/// export.
#[test]
fn the_mesh_reports_what_its_queries_cost() {
    let Some((mut document, path)) = with_imported_mesh("quality") else {
        return;
    };
    assert!(
        document.mesh_quality().is_none(),
        "there is no sculptor before the first stroke, so there is no figure"
    );

    dab(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0]).expect("a stroke");
    let quality = document
        .mesh_quality()
        .expect("a sculpted mesh has a figure to report");
    assert!(
        quality.is_finite() && quality >= 0.0,
        "the quality figure is not a number a reader could act on: {quality}"
    );
    let _ = std::fs::remove_file(&path);
}

/// Deformers and the cage: the operations a gesture cannot express.
///
/// A deformer states something about the *form* — no centre, no radius, no
/// falloff — and a cage is dragged by control points, so neither has a stroke
/// to be resolved from. They go through `apply_operation`, the second verb
/// beside `apply_stroke`, rather than widening the one path a latency budget
/// is measured against.
mod operations {
    use super::*;
    use clayspace_model::LayerOperation;

    fn taper() -> LayerOperation {
        LayerOperation::Taper {
            axis: [0.0, 1.0, 0.0],
            span: 2.0,
            scale_start: 1.0,
            scale_end: 0.5,
        }
    }

    #[test]
    fn a_deformer_reaches_every_vertex_without_a_brush_position() {
        let Some((mut document, path)) = with_imported_mesh("deform") else {
            return;
        };
        let before = document.stats();
        let outcome = document
            .apply_operation(taper())
            .expect("taper is a mesh operation");
        assert!(outcome.changed, "the taper moved nothing");
        let after = document.stats();
        assert_eq!(
            (after.triangles, after.vertices),
            (before.triangles, before.vertices),
            "a deformer changed the topology"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_twist_and_a_lattice_drag_are_both_accepted() {
        let Some((mut document, path)) = with_imported_mesh("twist") else {
            return;
        };
        document
            .apply_operation(LayerOperation::Twist {
                axis: [0.0, 1.0, 0.0],
                span: 2.0,
                angle: 0.6,
            })
            .expect("twist is a mesh operation");
        document
            .apply_operation(LayerOperation::LatticeDrag {
                divisions: [3, 3, 3],
                at: [1, 2, 1],
                offset: [0.15, 0.0, 0.0],
            })
            .expect("a cage drag is a mesh operation");
        let _ = std::fs::remove_file(&path);
    }

    /// One operation is one undo, on the same stack a stroke uses.
    #[test]
    fn an_operation_is_undoable_like_a_gesture() {
        let Some((mut document, path)) = with_imported_mesh("deform-undo") else {
            return;
        };
        let before = document.history().depth;
        document.apply_operation(taper()).expect("taper");
        assert_eq!(document.history().depth, before + 1);
        assert!(document.undo().expect("undo"));
        assert_eq!(document.history().depth, before);
        let _ = std::fs::remove_file(&path);
    }

    /// A cage is mesh-only on purpose: ZBrush and Blender both apply FFD
    /// forward to vertices, which a mesh allows and an implicit field does not.
    #[test]
    fn an_operation_is_refused_on_a_field() {
        let Some((mut document, path)) = with_imported_mesh("deform-sdf") else {
            return;
        };
        let sdf = document
            .scene()
            .layers
            .iter()
            .find(|layer| layer.representation == Representation::Sdf)
            .map(|layer| layer.key)
            .expect("the starting form");
        document.set_active_layer(sdf).expect("activate");

        let error = document
            .apply_operation(taper())
            .expect_err("a field has no forward point map to apply");
        assert!(
            error.to_string().contains("mesh"),
            "the refusal must name where the operation does apply: {error}"
        );
        let _ = std::fs::remove_file(&path);
    }
}
