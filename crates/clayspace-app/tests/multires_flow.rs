//! From a fixed mesh to a hierarchy and back out, through the handles the
//! application itself holds (#214).
//!
//! The engine tests beside `clayspace-engine/tests/multires.rs` hold each step
//! on a document. This walks the production flow the way the running
//! application reaches it — the shared document every ViewModel holds, and the
//! scene ViewModel that prices Create Multires, moves the levels and reads the
//! checksum — so a step that works on the document and is not forwarded, or
//! not kept, fails here:
//!
//! ```text
//! retopology -> fixed mesh -> Create Multires -> fine and coarse sculpt -> bake
//! ```
//!
//! with the detail checksum read at every step, because "a coarse edit keeps
//! the fine detail" is the property the representation exists for and should
//! be verified rather than repeated.

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, GestureSample, HierarchySettings, LayerKey, MultiresLevelOp,
    Representation, RetopoModel, RetopoSettings, SceneModel, SculptModel, ToolKind,
};
use clayspace_vm::SceneViewModel;

/// The starting form crossed to a mesh and retopologised beside it: a fixed
/// mesh of quads, active, which is where Create Multires is offered.
fn a_retopologised_mesh() -> Option<SharedDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let mut shared = SharedDocument::new(document);
    shared
        .with(|document| document.convert_layer(Direction::SdfToMesh, 0.05, 0))
        .ok()?;
    shared.can_retopologise().ok()?;
    let outcome = shared
        .retopologise(RetopoSettings {
            target_quads: 600,
            in_place: false,
            ..RetopoSettings::default()
        })
        .expect("the retopology runs");
    assert!(outcome.is_quads(), "a fixed mesh of quads: {outcome:?}");
    Some(shared)
}

fn active(shared: &SharedDocument) -> LayerKey {
    shared.scene().active.expect("an active layer")
}

/// One dab at the top of the active layer, at the level the brush is bound to.
fn dab(shared: &mut SharedDocument, size: f32) {
    let key = active(shared);
    let (min, max) = shared.layer_bounds(key).expect("the layer has a box");
    let top = [(min[0] + max[0]) * 0.5, max[1], (min[2] + max[2]) * 0.5];
    shared.begin_gesture();
    let applied = shared.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: top,
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    );
    shared.end_gesture();
    assert!(
        applied.expect("the dab is applied").changed,
        "the dab moved nothing"
    );
}

#[test]
fn fixed_mesh_to_multires_to_a_two_level_sculpt_and_a_bake() {
    let Some(mut shared) = a_retopologised_mesh() else {
        return;
    };
    let mut scene = SceneViewModel::new(Box::new(shared.clone()));
    let mesh = active(&shared);
    assert_eq!(
        shared.scene().layer(mesh).map(|layer| layer.representation),
        Some(Representation::Mesh)
    );

    // Priced before anything is built, from the ViewModel the inspector reads.
    let plan = scene
        .hierarchy_plan()
        .expect("a mesh layer is offered Create Multires")
        .expect("a retopology is a cage");
    let settings = HierarchySettings {
        levels: 2,
        in_place: false,
    };
    plan.within(settings.levels)
        .expect("two levels over a 600-quad retopology fit the budget");
    println!(
        "Create Multires, two levels over {} cage faces: the document holds {}, \
         the hierarchy adds {}, the limit is {}",
        plan.cage_faces,
        plan.held_bytes,
        plan.hierarchy_bytes(settings.levels),
        plan.budget_bytes
    );
    let depth = shared.history_depth();

    let key = shared.create_hierarchy(settings).expect("Create Multires");
    scene.refresh();
    assert_eq!(shared.history_depth(), depth + 1, "one undo step");
    assert!(
        scene.hierarchy_plan().is_none(),
        "the hierarchy is active now, and it is not offered a second one"
    );
    let levels = |shared: &SharedDocument| {
        shared
            .scene()
            .layer(key)
            .and_then(|layer| layer.multires.as_ref())
            .map(|state| state.levels)
            .expect("the new row is a hierarchy")
    };
    assert_eq!(levels(&shared).count, 3);
    assert_eq!(levels(&shared).sculpt, 2, "both numbers on the top level");
    let checksum = |scene: &SceneViewModel| {
        scene
            .hierarchy_checksum(key)
            .expect("a hierarchy has a detail checksum")
    };
    let created = checksum(&scene);

    // Fine detail at the top level.
    dab(&mut shared, 0.08);
    let fine = checksum(&scene);
    assert_ne!(fine, created, "a stroke at level 2 writes level 2's detail");

    // A coarser level's own detail.
    scene
        .apply_level_op(MultiresLevelOp::SetSculptLevel(1))
        .expect("level 1");
    dab(&mut shared, 0.3);
    let coarse = checksum(&scene);
    assert_ne!(coarse, fine, "a stroke at level 1 writes level 1's detail");

    // The form under both, at the cage: the detail above rides on it.
    scene
        .apply_level_op(MultiresLevelOp::SetSculptLevel(0))
        .expect("the cage");
    dab(&mut shared, 0.5);
    assert_eq!(
        checksum(&scene),
        coarse,
        "a cage edit moved the form and left every level's detail as it was"
    );

    // The bake says what it keeps before it runs, and keeps it.
    scene.refresh();
    let bake = scene
        .scene()
        .get()
        .layer(key)
        .and_then(|layer| layer.multires.as_ref())
        .map(|state| state.bake())
        .expect("a bake report");
    assert_eq!(bake.level, 2, "the displayed level is what a bake takes");
    assert_eq!(bake.finer_levels_dropped, 0);
    assert!(!bake.loses_detail());
    let baked = shared
        .with(|document| document.convert_layer(Direction::MultiresToMesh, 0.02, 0))
        .expect("a level is a mesh");
    assert_eq!(
        shared
            .scene()
            .layer(baked)
            .map(|layer| layer.representation),
        Some(Representation::Mesh)
    );
    assert!(
        scene.hierarchy_checksum(baked).is_none(),
        "and it is a mesh: nothing under it to checksum"
    );
    assert!(
        scene.hierarchy_checksum(key).is_some(),
        "while the hierarchy it was baked from stays"
    );
}
