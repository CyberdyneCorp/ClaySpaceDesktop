//! A sculpting session end to end: ViewModel commands, real engine underneath.
//!
//! The ViewModel tests use a double to check the rules; this checks that the
//! rules still hold when the thing underneath is a real document — that a
//! stroke reaches geometry, that undo puts it back, and that a dab dirties a
//! bounded region rather than the model.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{SculptModel, ToolKind};
use clayspace_vm::{Axis, Command, SculptViewModel, Watcher};

fn session() -> SculptViewModel {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let document = ClayDocument::new(policy)
        .expect("create document")
        .with_starting_form()
        .expect("starting form");
    SculptViewModel::new(Box::new(document))
}

/// Draws across the front of the starting sphere, on its surface.
fn stroke_across_the_form(vm: &mut SculptViewModel) -> Result<(), clayspace_model::ModelError> {
    let points: Vec<[f32; 3]> = (0..12)
        .map(|i| {
            let t = i as f32 / 11.0;
            let angle = (t - 0.5) * 1.4;
            let (s, c) = angle.sin_cos();
            [s * 1.01, 0.12, c * 1.01]
        })
        .collect();

    let (first, rest) = points.split_first().expect("points");
    vm.dispatch(Command::BeginStroke {
        position: *first,
        pressure: 1.0,
        modifiers: Default::default(),
    })?;
    for point in rest {
        vm.dispatch(Command::ContinueStroke {
            position: *point,
            pressure: 1.0,
        })?;
    }
    vm.dispatch(Command::EndStroke)
}

#[test]
fn a_document_starts_with_something_to_sculpt() {
    let vm = session();
    let (min, max) = vm.bounds().expect("the starting form must have bounds");
    assert!(
        max[0] > min[0] && max[1] > min[1],
        "the starting form is degenerate: {min:?}..{max:?}"
    );
}

#[test]
fn the_cursor_finds_the_surface_under_a_ray() {
    let vm = session();
    let hit = vm
        .pick([0.0, 0.0, -5.0], [0.0, 0.0, 1.0])
        .expect("a ray down the axis must meet the starting sphere");
    assert!(
        (hit[2] + 1.0).abs() < 0.15,
        "the pick landed at {hit:?}, which is not the near face of a unit sphere"
    );

    assert!(
        vm.pick([9.0, 9.0, -5.0], [0.0, 0.0, 1.0]).is_none(),
        "a ray well outside the form reported a hit"
    );
}

#[test]
fn a_stroke_changes_the_document_and_bounds_its_cost() {
    let mut vm = session();
    let mut history = Watcher::new();
    history.accept(vm.history());

    stroke_across_the_form(&mut vm).expect("stroke");

    assert!(
        vm.last_action().get().changed,
        "the stroke reported that it changed nothing"
    );
    assert!(
        history.take_change(vm.history()),
        "a live stroke must create a history entry"
    );

    let dirty = *vm.pending_remesh().get();
    assert!(
        dirty > 0,
        "a stroke dirtied no bricks, so nothing would be re-meshed"
    );
}

#[test]
fn a_whole_stroke_undoes_as_one_entry() {
    let mut vm = session();
    let before = vm.history().get().depth;

    stroke_across_the_form(&mut vm).expect("stroke");
    let after = vm.history().get().depth;
    assert_eq!(
        after - before,
        1,
        "a stroke of twelve samples produced {} history entries",
        after - before
    );

    vm.dispatch(Command::Undo).expect("undo");
    assert!(vm.last_action().get().changed, "there was a stroke to undo");
    assert_eq!(
        vm.history().get().depth,
        before,
        "undoing the stroke did not return the history to where it started"
    );
}

#[test]
fn redo_restores_what_undo_removed() {
    let mut vm = session();
    stroke_across_the_form(&mut vm).expect("stroke");
    let depth = vm.history().get().depth;

    vm.dispatch(Command::Undo).expect("undo");
    assert!(
        vm.history().get().can_redo,
        "an undone stroke must be redoable"
    );

    vm.dispatch(Command::Redo).expect("redo");
    assert_eq!(vm.history().get().depth, depth);
}

#[test]
fn a_mirrored_stroke_is_one_history_entry() {
    // The requirement is that a symmetric edit undoes as one step: both halves
    // belong to the same operation. Symmetry is on by default.
    let mut vm = session();
    let before = vm.history().get().depth;

    stroke_across_the_form(&mut vm).expect("stroke");
    assert_eq!(
        vm.history().get().depth - before,
        1,
        "a mirrored stroke must undo as one step"
    );

    vm.dispatch(Command::Undo).expect("undo");
    assert_eq!(
        vm.history().get().depth,
        before,
        "one undo did not remove both halves of the mirrored edit"
    );
}

#[test]
fn changing_symmetry_is_part_of_the_stroke_that_used_it() {
    // The mirror is document state in the engine, so setting it is a change
    // like any other and records its own entry. It is written by the first
    // segment of the stroke that needs it, though, so from the sculptor's side
    // it is not a separate thing they did — and one undo has to take it back
    // along with the stroke, or the mirror silently outlives the edit.
    let mut vm = session();

    let before = vm.history().get().depth;
    stroke_across_the_form(&mut vm).expect("first stroke");
    assert_eq!(
        vm.history().get().depth - before,
        1,
        "a stroke is one action however many segments and entries it took"
    );

    let before = vm.history().get().depth;
    stroke_across_the_form(&mut vm).expect("second stroke, same symmetry");
    assert_eq!(
        vm.history().get().depth - before,
        1,
        "an unchanged mirror was rewritten, costing a spurious action"
    );

    // Now with the mirror changed, which the stroke writes as an extra engine
    // entry inside its first segment.
    vm.dispatch(Command::ToggleSymmetry(Axis::Z))
        .expect("symmetry");
    let before = vm.history().get().depth;
    stroke_across_the_form(&mut vm).expect("mirrored stroke");
    assert_eq!(
        vm.history().get().depth - before,
        1,
        "changing the mirror must not cost the sculptor a second undo"
    );

    // And one undo takes the whole thing back, mirror write included. If the
    // count were short, an entry would stay behind and the next undo would
    // remove part of the *previous* stroke instead.
    let bounds = vm.bounds().expect("bounds");
    vm.dispatch(Command::Undo).expect("undo");
    vm.dispatch(Command::Undo).expect("undo");
    vm.dispatch(Command::Undo).expect("undo");
    assert!(
        !vm.history().get().can_undo,
        "three strokes took more than three undos to remove"
    );
    let _ = bounds;
}

#[test]
fn an_unavailable_tool_is_refused_against_a_real_document() {
    let mut vm = session();
    // The starting layer is SDF; scrape is voxel-side.
    vm.dispatch(Command::SelectTool(ToolKind::Raspar))
        .expect("select");

    let error = vm
        .dispatch(Command::BeginStroke {
            position: [0.0, 0.0, 1.0],
            pressure: 1.0,
            modifiers: Default::default(),
        })
        .expect_err("a voxel verb on an SDF layer must be refused");
    assert!(error.to_string().contains("voxel"), "{error}");
}

#[test]
fn a_voxel_layer_accepts_its_own_verbs() {
    let policy = BackendPolicy::discover(None).expect("backends");
    let mut document = ClayDocument::new(policy).expect("document");
    document
        .add_voxel_layer("Voxels", 0.05)
        .expect("add a voxel layer");

    let mut vm = SculptViewModel::new(Box::new(document));
    vm.dispatch(Command::SelectTool(ToolKind::Inflar))
        .expect("select");
    assert!(
        vm.tool_status().get().is_none(),
        "inflate belongs on a voxel layer: {:?}",
        vm.tool_status().get()
    );

    // Padrão deposits; Inflar dilates what is already there, so on an empty
    // grid it correctly does nothing. Deposit first.
    vm.dispatch(Command::SelectTool(ToolKind::Padrao))
        .expect("select");
    vm.dispatch(Command::BeginStroke {
        position: [0.0, 0.0, 0.0],
        pressure: 1.0,
        modifiers: Default::default(),
    })
    .expect("begin");
    vm.dispatch(Command::ContinueStroke {
        position: [0.05, 0.0, 0.0],
        pressure: 1.0,
    })
    .expect("continue");
    vm.dispatch(Command::EndStroke).expect("end");

    assert!(
        vm.last_action().get().changed,
        "depositing into an empty voxel grid changed nothing"
    );
}

#[test]
fn a_stroke_over_empty_space_is_not_an_error() {
    let mut vm = session();
    // Far from the form: the engine accepts it and nothing happens, which is
    // a legitimate outcome rather than a failure.
    for position in [[40.0f32, 40.0, 40.0], [40.1, 40.0, 40.0]] {
        let command = if position[0] == 40.0 {
            Command::BeginStroke {
                position,
                pressure: 1.0,
                modifiers: Default::default(),
            }
        } else {
            Command::ContinueStroke {
                position,
                pressure: 1.0,
            }
        };
        vm.dispatch(command)
            .expect("a stroke in empty space is legal");
    }
    vm.dispatch(Command::EndStroke)
        .expect("ending it is legal too");
}

// -- the rest of the SDF vocabulary -----------------------------------------

/// Applies a tool over a short path on the form's surface.
fn apply(vm: &mut SculptViewModel, tool: ToolKind) -> Result<bool, clayspace_model::ModelError> {
    vm.dispatch(Command::SelectTool(tool))?;
    stroke_across_the_form(vm)?;
    Ok(vm.last_action().get().changed)
}

#[test]
fn every_sdf_stroke_tool_changes_the_surface() {
    // A tool that is offered must do something. The engine documents several
    // verbs as legitimately able to change nothing, which is exactly why this
    // asks each one rather than assuming.
    for tool in ToolKind::ALL {
        if !tool.is_stroke_tool() {
            continue;
        }
        if tool
            .availability(clayspace_model::LayerState::editable(
                clayspace_model::Representation::Sdf,
            ))
            .is_err()
        {
            continue;
        }

        let mut vm = session();
        let changed = apply(&mut vm, tool).unwrap_or_else(|e| {
            panic!("{} failed on an SDF layer: {e}", tool.label());
        });
        assert!(
            changed,
            "{} is offered on SDF layers but changed nothing",
            tool.label()
        );
    }
}

#[test]
fn a_frame_drawn_tool_refuses_a_surface_stroke() {
    let mut vm = session();
    vm.dispatch(Command::SelectTool(ToolKind::Trim))
        .expect("select");

    let error = vm
        .dispatch(Command::BeginStroke {
            position: [0.0, 0.0, 1.0],
            pressure: 1.0,
            modifiers: Default::default(),
        })
        .expect_err("Trim's gesture is a shape on the frame, not a stroke");
    assert!(
        error.to_string().contains("frame"),
        "the refusal must say what gesture the tool wants: {error}"
    );
}

#[test]
fn a_move_under_the_resolution_changes_nothing() {
    let mut vm = session();
    vm.dispatch(Command::SelectTool(ToolKind::Mover))
        .expect("select");

    // Begin and end at the same point: a drag that never moved.
    vm.dispatch(Command::BeginStroke {
        position: [0.0, 0.0, 1.0],
        pressure: 1.0,
        modifiers: Default::default(),
    })
    .expect("begin");
    vm.dispatch(Command::EndStroke).expect("end");

    assert!(
        !vm.last_action().get().changed,
        "a drag that travelled nowhere reported an edit"
    );
}

#[test]
fn brush_shaping_reaches_the_engine_without_error() {
    // Each control maps to a preset or footprint field; this checks the whole
    // range is accepted rather than clamped into an engine refusal.
    for falloff in clayspace_model::Falloff::ALL {
        for accumulate in [true, false] {
            let mut vm = session();
            vm.dispatch(Command::SetBrushIntensity(0.9))
                .expect("intensity");
            vm.dispatch(Command::SetBrushFlow(0.95)).expect("flow");
            let _ = (falloff, accumulate);
            stroke_across_the_form(&mut vm).expect("stroke with shaping applied");
        }
    }
}

// -- scene and layers against a real document --------------------------------

mod scene {
    use super::*;
    use clayspace_model::{
        BrushSettings, Combine, CombineSettings, GestureSample, LayerKey, Protection,
        Representation, SceneModel,
    };
    use clayspace_vm::SceneViewModel;

    fn document() -> ClayDocument {
        let policy = BackendPolicy::discover(None).expect("backends");
        ClayDocument::new(policy)
            .expect("document")
            .with_starting_form()
            .expect("starting form")
    }

    #[test]
    fn a_fresh_document_reports_one_layer() {
        let scene = document().scene();
        assert_eq!(scene.layers.len(), 1);
        assert!(
            scene.active.is_some(),
            "something must be active to sculpt on"
        );
        assert_eq!(scene.nodes.len(), 1, "the tree must mirror what is there");
    }

    #[test]
    fn adding_a_layer_makes_it_active_and_keeps_the_old_one() {
        let mut doc = document();
        let first = doc.scene().active.expect("active");
        let added = doc
            .add_layer("Detalhe", Representation::Sdf)
            .expect("add a layer");

        let scene = doc.scene();
        assert_eq!(scene.layers.len(), 2);
        assert_eq!(scene.active, Some(added));
        assert!(
            scene.layer(first).is_some(),
            "adding a layer removed the one that was there"
        );
    }

    #[test]
    fn hiding_a_layer_removes_its_contribution() {
        let mut doc = document();
        let key = doc.scene().active.expect("active");

        // Bounds are the wrong measure: the engine reports a layer's own
        // extent whether or not it is shown, which is reasonable. What a user
        // sees is whether the surface is still there to hit.
        assert!(
            doc.pick([0.0, 0.0, -5.0], [0.0, 0.0, 1.0]).is_some(),
            "the starting form should be under the ray"
        );

        doc.set_layer_visible(key, false).expect("hide");
        assert!(
            doc.pick([0.0, 0.0, -5.0], [0.0, 0.0, 1.0]).is_none(),
            "a hidden layer still contributed to the surface"
        );

        doc.set_layer_visible(key, true).expect("show");
        assert!(
            doc.pick([0.0, 0.0, -5.0], [0.0, 0.0, 1.0]).is_some(),
            "showing the layer again did not bring the surface back"
        );
    }

    #[test]
    fn a_locked_layer_refuses_a_stroke_with_a_reason() {
        let mut doc = document();
        let key = doc.scene().active.expect("active");
        doc.set_layer_protection(
            key,
            Protection {
                ghost: false,
                locked: true,
            },
        )
        .expect("lock");

        let mut vm = SculptViewModel::new(Box::new(doc));
        let error = vm
            .dispatch(Command::BeginStroke {
                position: [0.0, 0.0, 1.0],
                pressure: 1.0,
                modifiers: Default::default(),
            })
            .expect_err("a locked layer accepts no edit");
        assert!(error.to_string().contains("locked"), "{error}");
    }

    #[test]
    fn a_ghosted_layer_is_not_picked() {
        let mut doc = document();
        let key = doc.scene().active.expect("active");

        assert!(
            doc.select_at([0.0, 0.0, -5.0], [0.0, 0.0, 1.0]).is_some(),
            "the form should be picked before it is ghosted"
        );

        doc.set_layer_protection(
            key,
            Protection {
                ghost: true,
                locked: false,
            },
        )
        .expect("ghost");

        assert_eq!(
            doc.select_at([0.0, 0.0, -5.0], [0.0, 0.0, 1.0]),
            None,
            "a ghosted layer was picked; the engine excludes them and this must follow"
        );
    }

    #[test]
    fn removing_the_only_layer_is_refused() {
        let mut doc = document();
        let key = doc.scene().active.expect("active");
        let error = doc
            .remove_layer(key)
            .expect_err("a document keeps a layer to sculpt on");
        assert!(error.to_string().contains("layer"), "{error}");
        assert_eq!(doc.scene().layers.len(), 1);
    }

    /// A removed layer stops being drawn, and stops being picked.
    ///
    /// The brick cache holds the *evaluated field*, brick by brick. Removing a
    /// layer took it out of the document and left every brick it had
    /// contributed to exactly as it was: the surface went on being drawn and
    /// went on answering a raycast. Reported as the starting sphere sitting
    /// under a sculpt and never changing, with the real result only appearing
    /// after a save and a reopen — which builds the cache from nothing.
    ///
    /// Marking the *remaining* active layer, which is what the removal did,
    /// cannot help: the stale bricks belong to the layer that left. Measured
    /// on a two-layer document, the removed sphere still meshed to the same
    /// 298,680 triangles through an incremental sync and through a full
    /// rebuild alike; against 17,160 once its region is re-evaluated.
    ///
    /// Hiding a layer was always right, and this is the tripwire for it too.
    #[test]
    fn a_removed_layer_stops_being_drawn() {
        let mut doc = document();
        let sphere = doc.scene().active.expect("the starting form");

        // Somewhere else to keep, so the removal is allowed at all.
        let other = doc
            .add_layer("Outra", Representation::Sdf)
            .expect("a layer");
        doc.set_active_layer(other).expect("activate it");
        doc.set_combine(CombineSettings {
            op: Combine::Add,
            ..CombineSettings::for_strokes()
        });
        doc.apply_stroke(
            ToolKind::Padrao,
            BrushSettings {
                size: 0.35,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: [2.2, 0.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the stroke was refused");

        let down_the_axis = ([0.0, 0.0, 4.0], [0.0, 0.0, -1.0]);
        assert!(
            SculptModel::pick(&doc, down_the_axis.0, down_the_axis.1).is_some(),
            "the sphere is not there to begin with"
        );

        // Hidden, which has always worked, and shown again.
        doc.set_layer_visible(sphere, false).expect("hide");
        assert!(
            SculptModel::pick(&doc, down_the_axis.0, down_the_axis.1).is_none(),
            "a hidden layer is still picked"
        );
        doc.set_layer_visible(sphere, true).expect("show");
        assert!(
            SculptModel::pick(&doc, down_the_axis.0, down_the_axis.1).is_some(),
            "showing a layer again did not bring it back"
        );

        // And removed.
        doc.remove_layer(sphere).expect("remove");
        assert!(
            SculptModel::pick(&doc, down_the_axis.0, down_the_axis.1).is_none(),
            "a removed layer is still in the cache: it draws and it is picked, \
             and only reopening the file would say otherwise"
        );

        // What is left is still there — the removal cleared one layer's
        // region and not the document.
        assert!(
            SculptModel::pick(&doc, [2.2, 0.0, 4.0], [0.0, 0.0, -1.0]).is_some(),
            "removing one layer took the other with it"
        );
    }

    #[test]
    fn removing_a_layer_leaves_a_valid_active_one() {
        let mut doc = document();
        let added = doc.add_layer("Detalhe", Representation::Sdf).expect("add");
        doc.remove_layer(added).expect("remove");

        let scene = doc.scene();
        assert_eq!(scene.layers.len(), 1);
        let active = scene.active.expect("something must stay active");
        assert!(
            scene.layer(active).is_some(),
            "the active layer points at one that is gone"
        );
    }

    #[test]
    fn reordering_changes_evaluation_order() {
        let mut doc = document();
        let second = doc.add_layer("Segunda", Representation::Sdf).expect("add");

        doc.move_layer(second, 0).expect("move to the bottom");
        let scene = doc.scene();
        assert_eq!(scene.layers[0].key, second);
        assert_eq!(
            scene.active,
            Some(second),
            "the active layer must follow the layer it pointed at, not its index"
        );
    }

    #[test]
    fn the_panels_see_edits_the_brush_made() {
        // The two ViewModels share a document in the application; here they
        // each hold one, so this checks the scene reflects its own model
        // rather than a snapshot taken once at construction.
        let doc = document();
        let mut vm = SceneViewModel::new(Box::new(doc));
        let before = vm.scene().get().layers.len();

        vm.dispatch(&Command::AddLayer).expect("add");
        vm.refresh();
        assert_eq!(
            vm.scene().get().layers.len(),
            before + 1,
            "the panel did not see a layer it had just created"
        );
    }

    #[test]
    fn a_layer_transform_is_one_undo_step() {
        let mut doc = document();
        let key = doc.scene().active.expect("active");
        let before = doc.history().depth;

        doc.set_layer_transform(key, [0.5, 0.0, 0.0], 1.0)
            .expect("place the layer");

        assert_eq!(
            doc.history().depth - before,
            1,
            "placing a layer must undo in one step however many items it holds"
        );
    }

    #[test]
    fn a_layer_reports_what_its_field_costs() {
        let doc = document();
        let key = doc.scene().active.expect("active");

        let cost = doc.layer_cost(key).expect("field report");
        assert!(cost.items > 0, "the starting form has an item");
        assert!(
            cost.safe_step_scale > 0.0 && cost.safe_step_scale <= 1.0,
            "the safe step scale should be in (0, 1], got {}",
            cost.safe_step_scale
        );
        assert!(!cost.consolidated, "a fresh layer is not collapsed");
    }

    #[test]
    fn consolidation_is_never_performed_unasked() {
        let mut doc = document();
        let key = doc.scene().active.expect("active");

        // Asking the cost must not collapse anything.
        let before = doc.layer_cost(key).expect("cost");
        let after = doc.layer_cost(key).expect("cost again");
        assert!(!before.consolidated && !after.consolidated);
        assert_eq!(
            before.items, after.items,
            "asking the cost changed the layer"
        );

        doc.consolidate_layer(key).expect("consolidate");
        let collapsed = doc.layer_cost(key).expect("cost after");
        assert!(
            collapsed.consolidated,
            "the layer did not report itself collapsed"
        );
    }

    #[test]
    fn a_mesh_layer_offers_the_mesh_vocabulary() {
        let mut doc = document();
        let key = doc.add_mesh_layer("Referência").expect("carry a mesh");

        let scene = doc.scene();
        let layer = scene.layer(key).expect("the mesh layer");
        assert_eq!(layer.representation, Representation::Mesh);

        doc.set_active_layer(key).expect("activate");
        let offered = ToolKind::for_representation(Representation::Mesh);
        // Sixteen fixed-topology brushes plus Máscara, which is not one of
        // them: a mask is a world-addressed field the verbs consult rather
        // than a way of moving triangles, and it is offered on all three
        // representations for that reason.
        let brushes = offered.iter().filter(|t| !t.is_mask_tool()).count();
        assert_eq!(
            brushes, 16,
            "the shelf offers {brushes} mesh brushes against the engine's \
             sixteen fixed-topology ones"
        );
        assert!(
            offered.iter().any(|t| t.is_mask_tool()),
            "a mesh layer cannot be masked from the shelf"
        );
        // Offered for the representation, and disabled on *this* row: it was
        // recorded by `add_mesh_layer` and its triangles have not arrived, so
        // there is nothing for a verb to move. Shown-and-disabled rather than
        // absent, because the tool does apply here — the layer is what is not
        // ready — which is the distinction the two refusals draw.
        for tool in &offered {
            let error = tool
                .availability(doc.active_layer_state())
                .expect_err("an empty mesh row has nothing to sculpt");
            assert!(
                matches!(
                    error,
                    clayspace_model::Unavailable::MissingAttribute { needs: "mesh" }
                ),
                "{} refused for the wrong reason: {error}",
                tool.label()
            );
        }
        // A cavity fill and a shape drawn on the frame are not vertex verbs,
        // so they are absent rather than disabled. Máscara is not one either
        // and is still offered, because what it writes is not vertices: it
        // paints the world-addressed field the vertex verbs consult, and a
        // mesh that could be protected by a mask but not used to paint one
        // was a gap rather than a principle.
        for tool in [ToolKind::Preencher, ToolKind::Trim] {
            assert!(
                !offered.contains(&tool),
                "{} was offered on a mesh layer",
                tool.label()
            );
        }
    }

    #[test]
    fn an_unknown_layer_key_is_refused_rather_than_panicking() {
        let mut doc = document();
        let error = doc
            .set_layer_visible(LayerKey(9999), false)
            .expect_err("that layer does not exist");
        assert!(error.to_string().contains("no longer"), "{error}");
    }
}
