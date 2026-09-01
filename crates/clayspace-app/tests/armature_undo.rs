//! Undoing rig work the way the application produces it.
//!
//! There were already undo tests, and they passed, and rigging still did not
//! undo. They drove the *engine*, where one edit is one call — while the
//! application drives a ViewModel that edits once per pointer sample and then
//! has to bank the gesture as a single action. Nothing tested that seam, so
//! nothing noticed that the gesture path never recorded an action at all.
//!
//! These go through the ViewModels and the same history the interface reads.

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::SculptModel;
use clayspace_vm::{ArmatureViewModel, Command, Grab, SculptViewModel};

struct Rigging {
    document: SharedDocument,
    sculpt: SculptViewModel,
    armature: ArmatureViewModel,
}

impl Rigging {
    fn new() -> Option<Self> {
        let policy = BackendPolicy::discover(None).ok()?;
        let document = SharedDocument::new(ClayDocument::new(policy).ok()?);
        Some(Self {
            sculpt: SculptViewModel::new(Box::new(document.clone())),
            armature: ArmatureViewModel::new(Box::new(document.clone())),
            document,
        })
    }

    /// What the composition root reads to bank an action.
    fn depth(&self) -> usize {
        self.document.with(|d| SculptModel::history(d).depth)
    }

    /// One rig gesture, banked as the application banks it: measure the
    /// engine's depth either side, and record the difference as one action.
    fn gesture(&mut self, grab: Grab, from: [f32; 3], path: &[[f32; 3]]) {
        let before = self.depth();
        self.armature.press(grab, from);
        for at in path {
            self.armature.drag(*at);
        }
        self.armature.release();
        let entries = self.depth().saturating_sub(before);
        self.sculpt.record_external_action(entries);
    }

    fn begin(&mut self, at: [f32; 3]) {
        let before = self.depth();
        self.armature.begin(at);
        let entries = self.depth().saturating_sub(before);
        self.sculpt.record_external_action(entries);
    }

    fn undo(&mut self) {
        self.sculpt.dispatch(Command::Undo).expect("undo");
        self.armature.refresh();
    }

    fn redo(&mut self) {
        self.sculpt.dispatch(Command::Redo).expect("redo");
        self.armature.refresh();
    }

    fn spheres(&self) -> usize {
        self.armature
            .tree()
            .get()
            .as_ref()
            .map(|t| t.nodes.len())
            .unwrap_or(0)
    }
}

#[test]
fn growing_a_sphere_by_dragging_is_one_undo() {
    // The bug as reported: adding a ZSphere and pressing undo did nothing,
    // because a drag never banked an action. A gesture is many engine entries
    // — one per sample — and exactly one Cmd+Z.
    let Some(mut rig) = Rigging::new() else {
        return;
    };
    rig.begin([0.0, 0.0, 0.0]);
    rig.armature.set_symmetric(false);
    assert_eq!(rig.spheres(), 1);

    // Several samples, as a drag delivers them.
    rig.gesture(
        Grab::Grow(0),
        [0.0, 0.0, 0.0],
        &[
            [0.3, 0.0, 0.0],
            [0.5, 0.0, 0.0],
            [0.7, 0.0, 0.0],
            [0.9, 0.0, 0.0],
        ],
    );
    assert_eq!(rig.spheres(), 2, "the drag grew no sphere");
    assert!(rig.sculpt.history().get().can_undo, "nothing to undo");

    rig.undo();
    assert_eq!(
        rig.spheres(),
        1,
        "one undo did not take the whole gesture back"
    );
}

#[test]
fn a_mirrored_zsphere_stays_mirrored_while_dragging() {
    let Some(mut rig) = Rigging::new() else {
        return;
    };
    rig.begin([0.0, 0.0, 0.0]);

    rig.gesture(
        Grab::Grow(0),
        [0.0, 0.0, 0.0],
        &[[0.35, 0.0, 0.0], [0.7, 0.25, 0.0]],
    );

    let tree = rig.armature.tree().get().clone().expect("a tree");
    assert_eq!(tree.nodes.len(), 3, "the reflected child was not created");
    assert_eq!(tree.nodes[1].position, [0.7, 0.25, 0.0]);
    assert_eq!(tree.nodes[2].position, [-0.7, 0.25, 0.0]);
}

#[test]
fn an_inserted_mirrored_joint_keeps_its_branches_paired() {
    let Some(mut rig) = Rigging::new() else {
        return;
    };
    rig.begin([0.0, 0.0, 0.0]);
    rig.gesture(Grab::Grow(0), [0.0, 0.0, 0.0], &[[0.6, 0.0, 0.0]]);

    rig.gesture(
        Grab::Insert(1),
        [0.3, 0.0, 0.0],
        &[[0.3, 0.0, 0.0], [0.2, 0.25, 0.0]],
    );

    let tree = rig.armature.tree().get().clone().expect("a tree");
    assert_eq!(tree.nodes.len(), 5);
    assert_eq!(tree.nodes[3].position, [0.2, 0.25, 0.0]);
    assert_eq!(tree.nodes[4].position, [-0.2, 0.25, 0.0]);
    assert_eq!(tree.nodes[1].parent, 3);
    assert_eq!(tree.nodes[2].parent, 4);
}

#[test]
fn a_second_undo_takes_the_rig_itself() {
    let Some(mut rig) = Rigging::new() else {
        return;
    };
    rig.begin([0.0, 0.0, 0.0]);
    rig.armature.set_symmetric(false);
    rig.gesture(Grab::Grow(0), [0.0, 0.0, 0.0], &[[0.6, 0.0, 0.0]]);

    rig.undo();
    assert_eq!(rig.spheres(), 1);
    rig.undo();
    assert_eq!(rig.spheres(), 0, "the rig itself did not come back out");
    assert!(
        !rig.armature.is_rigging(),
        "an undone rig is still reported as one"
    );
}

#[test]
fn redo_restores_the_whole_gesture() {
    let Some(mut rig) = Rigging::new() else {
        return;
    };
    rig.begin([0.0, 0.0, 0.0]);
    rig.armature.set_symmetric(false);
    rig.gesture(
        Grab::Grow(0),
        [0.0, 0.0, 0.0],
        &[[0.4, 0.0, 0.0], [0.8, 0.0, 0.0]],
    );

    rig.undo();
    assert_eq!(rig.spheres(), 1);
    assert!(rig.sculpt.history().get().can_redo, "nothing to redo");

    rig.redo();
    assert_eq!(rig.spheres(), 2, "redo did not bring the gesture back");
    // And where the drag left it, not where it was created.
    let tree = rig.armature.tree().get().clone().expect("a tree");
    assert!(
        (tree.nodes[1].position[0] - 0.8).abs() < 1e-4,
        "it came back at {:?} rather than where the drag ended",
        tree.nodes[1].position
    );
}

#[test]
fn moving_a_subtree_undoes_as_one_action() {
    let Some(mut rig) = Rigging::new() else {
        return;
    };
    rig.begin([0.0, 0.0, 0.0]);
    rig.armature.set_symmetric(false);
    rig.gesture(Grab::Grow(0), [0.0, 0.0, 0.0], &[[0.6, 0.0, 0.0]]);
    rig.gesture(Grab::Grow(1), [0.6, 0.0, 0.0], &[[1.2, 0.0, 0.0]]);
    assert_eq!(rig.spheres(), 3);

    // Lift the middle sphere over several samples; the tip comes with it.
    rig.gesture(
        Grab::Move(1),
        [0.6, 0.0, 0.0],
        &[[0.6, 0.2, 0.0], [0.6, 0.4, 0.0], [0.6, 0.6, 0.0]],
    );
    let lifted = rig.armature.tree().get().clone().expect("a tree");
    assert!((lifted.nodes[1].position[1] - 0.6).abs() < 1e-4);
    assert!((lifted.nodes[2].position[1] - 0.6).abs() < 1e-4);

    rig.undo();
    let back = rig.armature.tree().get().clone().expect("a tree");
    assert_eq!(rig.spheres(), 3, "undo removed a sphere rather than a move");
    assert!(
        back.nodes[1].position[1].abs() < 1e-4,
        "the move did not undo: {:?}",
        back.nodes[1].position
    );
    assert!(
        back.nodes[2].position[1].abs() < 1e-4,
        "the subtree did not come back with it: {:?}",
        back.nodes[2].position
    );
}

#[test]
fn a_gesture_that_changed_nothing_banks_nothing() {
    // A press and release with no movement edits nothing, so it must not eat
    // a Cmd+Z — otherwise undo appears to do nothing at all.
    let Some(mut rig) = Rigging::new() else {
        return;
    };
    rig.begin([0.0, 0.0, 0.0]);
    rig.armature.set_symmetric(false);
    rig.gesture(Grab::Grow(0), [0.0, 0.0, 0.0], &[[0.6, 0.0, 0.0]]);

    let before = rig.spheres();
    rig.gesture(Grab::Move(1), [0.6, 0.0, 0.0], &[]);

    rig.undo();
    assert_eq!(
        rig.spheres(),
        before - 1,
        "the empty gesture consumed the undo that should have removed a sphere"
    );
}
