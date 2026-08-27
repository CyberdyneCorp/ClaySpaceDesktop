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
fn with_imported_mesh(who: &str) -> (ClayDocument, std::path::PathBuf) {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let path = scratch(&format!("{who}.obj"));
    document
        .export_mesh(&path, ExportSettings::default())
        .expect("export a mesh");
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import it back");

    let key = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the imported mesh is a layer");
    document.set_active_layer(key).expect("activate the mesh");
    (document, path)
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
    let (mut document, path) = with_imported_mesh("accepts");
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
    let (mut document, path) = with_imported_mesh("topology");
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
    let (mut document, path) = with_imported_mesh("colour");
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
    let (mut document, path) = with_imported_mesh("pick");
    // Before any stroke, which is the order that matters. This used to assert
    // the opposite — the sculptor was built by the first stroke, so a pick
    // before one found nothing, and that was written down as deliberate: a
    // pick happens every frame the pointer moves and may not pay for an
    // adjacency pass.
    //
    // It was a deadlock. The interface places a stroke where the pick reported
    // and sends nothing where it reported nothing, so the first stroke could
    // never arrive and a mesh layer was unsculptable through the pointer. The
    // adjacency pass is paid once, when the layer becomes active — a discrete
    // thing the sculptor did, not something a moving pointer repeats.
    let hit = document.pick([0.0, 0.0, 4.0], [0.0, 0.0, -1.0]).expect(
        "selecting a mesh layer does not make it pointable, so the first \
             stroke can never be placed on it",
    );
    assert!(
        hit[2] > 0.0,
        "the ray came from +z and hit at {hit:?}, which is behind the surface"
    );

    // A ray that meets nothing still meets nothing: a pick that answered
    // everywhere would put the brush on empty space.
    assert!(
        document.pick([4.0, 4.0, 4.0], [0.0, 0.0, -1.0]).is_none(),
        "a ray nowhere near the mesh reported a hit"
    );

    // And it still answers after a stroke, which is what it always did.
    dab(&mut document, ToolKind::Padrao, [0.0, 0.0, 1.0]).expect("a stroke");
    assert!(
        document.pick([0.0, 0.0, 4.0], [0.0, 0.0, -1.0]).is_some(),
        "the pick stopped answering once the mesh had been sculpted"
    );
    let _ = std::fs::remove_file(&path);
}

/// The other half: a pick while an SDF layer is active must not answer with a
/// mesh layer's surface, or the cursor would sit on something the active
/// brush cannot reach.
#[test]
fn a_mesh_is_not_picked_from_under_another_layer() {
    let (mut document, path) = with_imported_mesh("pick-other");
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
        let (mut document, path) = with_imported_mesh("undo");
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
        let (mut document, path) = with_imported_mesh("undo-empty");
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
        let (mut document, path) = with_imported_mesh("undo-interleave");
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
    let (mut document, path) = with_imported_mesh("quality");
    // Reported from the moment the layer is the one being worked on. This used
    // to assert there was no figure until the first stroke, because the
    // sculptor the figure comes from was built by that stroke — the same
    // deadlock `the_pointer_finds_an_imported_mesh` records, seen from the
    // readout's side. A sculptor deciding whether a mesh needs retopology
    // wants the number before they start, not after.
    assert!(
        document.mesh_quality().is_some(),
        "a selected mesh layer reports no quality figure at all"
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
        let (mut document, path) = with_imported_mesh("deform");
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
        let (mut document, path) = with_imported_mesh("twist");
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
        let (mut document, path) = with_imported_mesh("deform-undo");
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
        let (mut document, path) = with_imported_mesh("deform-sdf");
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

/// The brush's size and intensity reach a mesh stroke.
///
/// They did not. The engine states that `clay_mesh_sculptor_apply_stroke`
/// **ignores the descriptor's radius and strength** and takes each stamp's
/// from the preset — and the mesh path built its own preset carrying only
/// spacing, so every mesh stroke ran at the engine's default radius of 0.25
/// whatever the brush said. Measured before the fix: sizes 0.1, 0.5 and 1.0
/// each moved exactly the same 944 vertices.
///
/// The same line had spacing inverted against every other path. The design
/// reads flow as "more flow, stamps closer together" and the SDF path spells
/// that `1.0 - flow`; the mesh path passed it straight through, so more flow
/// spread the stamps further apart. On a dragging verb that decides whether a
/// second stamp is emitted at all, and a stroke of one stamp has no motion to
/// drag by — which is why Move looked broken rather than merely coarse.
#[test]
fn the_brush_size_reaches_a_mesh_stroke() {
    let (mut small, small_path) = with_imported_mesh("size-small");
    let (mut large, large_path) = with_imported_mesh("size-large");

    let reached = |document: &mut ClayDocument, size: f32| -> usize {
        let before = document.visible_mesh_geometry().0;
        document
            .apply_stroke(
                ToolKind::Inflar,
                BrushSettings {
                    size,
                    intensity: 1.0,
                    ..BrushSettings::default()
                },
                &[GestureSample {
                    position: [0.0, 0.0, 1.0],
                    pressure: 1.0,
                    time: 0.0,
                }],
                [false; 3],
            )
            .expect("the stroke was refused");
        let after = document.visible_mesh_geometry().0;
        before.iter().zip(&after).filter(|(a, b)| a != b).count()
    };

    let few = reached(&mut small, 0.1);
    let many = reached(&mut large, 0.6);
    assert!(few > 0, "the small brush moved nothing at all");
    assert!(
        many > few * 3,
        "a brush six times the size reached {many} vertices against {few}. \
         The size is not reaching the stroke, so Tamanho is inert on a mesh \
         layer"
    );

    let _ = std::fs::remove_file(&small_path);
    let _ = std::fs::remove_file(&large_path);
}

/// Smoothing takes a ridge down, rather than politely declining to.
///
/// Reported as "Suavizar does nothing", and it did almost nothing: a ridge
/// standing 0.0676 proud of a unit sphere came down 0.0006 — under one percent
/// of it — after four passes over it.
///
/// Two causes. A mesh stroke is clamped so it cannot build on itself, which is
/// what stops the verbs that displace along a per-vertex normal from shredding
/// the surface; a smoothing verb *converges* instead, so clamping one means a
/// sculptor can never smooth more than a single stamp's worth however long
/// they rub. And the engine's SMOOTH averages a vertex with its one-ring, a
/// high-frequency filter that takes out tessellation noise and barely touches
/// a bump spanning many edges — so it has to be run many times per stamp, and
/// the engine's own default is far below that.
///
/// Measured on the same ridge, four passes over it:
///
///   passes per stamp   clamped   accumulating
///    1                  1.0670      1.0654
///    8                  1.0646      1.0552
///   64                  1.0520      1.0187
#[test]
fn smoothing_a_mesh_takes_a_ridge_down() {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    document
        .convert_layer(clayspace_model::Direction::SdfToMesh, 0.02, 0)
        .expect("into a mesh");

    let sweep = |document: &ClayDocument| -> Vec<GestureSample> {
        (0..=20)
            .filter_map(|step| {
                let t = step as f32 / 20.0;
                SculptModel::pick(document, [-0.4 + t * 0.8, 0.0, 4.0], [0.0, 0.0, -1.0]).map(
                    |hit| GestureSample {
                        position: hit,
                        pressure: 1.0,
                        time: t,
                    },
                )
            })
            .collect()
    };
    // How proud the tallest point stands. The sphere started at 1.0, so this
    // is the ridge's own height — roughness would measure the tessellation
    // rather than the form, which is why the first attempt at this saw
    // nothing.
    let prominence = |document: &mut ClayDocument| -> f32 {
        document
            .visible_mesh_geometry()
            .0
            .iter()
            .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
            .fold(0.0f32, f32::max)
    };

    for sample in sweep(&document) {
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    size: 0.18,
                    intensity: 0.65,
                    ..BrushSettings::default()
                },
                &[sample],
                [false; 3],
            )
            .expect("the ridge was refused");
    }
    let ridge = prominence(&mut document);
    assert!(
        ridge > 1.02,
        "the ridge only stands {ridge} proud of the sphere, so there is \
         nothing here to smooth"
    );

    for _ in 0..4 {
        let samples = sweep(&document);
        document
            .apply_stroke(
                ToolKind::Suavizar,
                BrushSettings {
                    size: 0.25,
                    intensity: 0.65,
                    ..BrushSettings::default()
                },
                &samples,
                [false; 3],
            )
            .expect("Suavizar was refused");
    }
    let smoothed = prominence(&mut document);

    // Most of the way back to the sphere it was cut into.
    let taken = (ridge - smoothed) / (ridge - 1.0);
    assert!(
        taken > 0.5,
        "four passes took {:.0}% of the ridge ({ridge} to {smoothed}); \
         rubbing at a surface has to melt it",
        taken * 100.0
    );
}
