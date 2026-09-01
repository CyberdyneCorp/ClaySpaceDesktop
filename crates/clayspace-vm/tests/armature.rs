//! Rigging as a gesture.
//!
//! The engine tests cover what reaches the surface. These cover the part a
//! person actually touches: what a press means, what a drag grows, and what
//! the viewport is told to highlight. The rule being protected is ZBrush's —
//! one pointer, no mode to remember, and where you press decides what happens.

use clayspace_model::{Armature, ArmatureModel, ModelError, NodeIndex, SkinSettings};
use clayspace_vm::{ArmatureViewModel, Grab};

/// The tree, held honestly, with no engine behind it.
#[derive(Default)]
struct FakeRig {
    tree: Option<Armature>,
    skin: SkinSettings,
    refuse: bool,
}

impl FakeRig {
    fn boxed(self) -> Box<dyn ArmatureModel> {
        Box::new(self)
    }
}

impl ArmatureModel for FakeRig {
    fn armature(&self) -> Option<Armature> {
        self.tree.clone()
    }

    fn begin_armature(&mut self, position: [f32; 3], radius: f32) -> Result<(), ModelError> {
        self.tree = Some(Armature::rooted(position, radius));
        Ok(())
    }

    fn add_zsphere(
        &mut self,
        parent: NodeIndex,
        position: [f32; 3],
        radius: f32,
        mirrored: bool,
    ) -> Result<NodeIndex, ModelError> {
        if self.refuse {
            return Err(ModelError::engine("recusado"));
        }
        let tree = self.tree.as_mut().ok_or_else(no_rig)?;
        let added = tree.add_child(parent, position, radius);
        if mirrored {
            if let Some(reflected) = Armature::mirrored_position(position) {
                // The reflection hangs off the parent's own reflection where
                // there is one, so two arms end up on two shoulders.
                let mirror_parent =
                    Armature::mirrored_position(tree.nodes[parent as usize].position)
                        .and_then(|at| {
                            tree.nodes
                                .iter()
                                .position(|n| distance(n.position, at) < 1e-4)
                                .map(|i| i as NodeIndex)
                        })
                        .unwrap_or(parent);
                tree.add_child(mirror_parent, reflected, radius);
            }
        }
        Ok(added)
    }

    fn move_zsphere(&mut self, index: NodeIndex, delta: [f32; 3]) -> Result<(), ModelError> {
        let tree = self.tree.as_mut().ok_or_else(no_rig)?;
        tree.move_subtree(index, delta);
        Ok(())
    }

    fn resize_zsphere(&mut self, index: NodeIndex, radius: f32) -> Result<(), ModelError> {
        let tree = self.tree.as_mut().ok_or_else(no_rig)?;
        tree.set_radius(index, radius);
        Ok(())
    }

    fn reparent_zsphere(
        &mut self,
        index: NodeIndex,
        new_parent: NodeIndex,
    ) -> Result<(), ModelError> {
        let tree = self.tree.as_mut().ok_or_else(no_rig)?;
        tree.reparent(index, new_parent)
    }

    fn remove_zsphere(&mut self, index: NodeIndex) -> Result<(), ModelError> {
        let tree = self.tree.as_mut().ok_or_else(no_rig)?;
        // The engine's rule, kept here so the view model meets the same
        // refusal it meets in the application.
        if index == 0 {
            return Err(ModelError::engine("a raiz não pode ser removida"));
        }
        tree.remove(index);
        Ok(())
    }

    fn insert_zsphere(&mut self, child: NodeIndex) -> Result<NodeIndex, ModelError> {
        let tree = self.tree.as_mut().ok_or_else(no_rig)?;
        tree.insert_on_link(child)
            .ok_or_else(|| ModelError::engine("essa esfera não tem ligação"))
    }

    fn set_zsphere_negative(&mut self, index: NodeIndex, negative: bool) -> Result<(), ModelError> {
        let tree = self.tree.as_mut().ok_or_else(no_rig)?;
        tree.set_negative(index, negative)
    }

    fn set_skin(&mut self, skin: SkinSettings) -> Result<(), ModelError> {
        self.skin = skin;
        Ok(())
    }

    fn skin(&self) -> SkinSettings {
        self.skin
    }
}

fn no_rig() -> ModelError {
    ModelError::engine("não há armadura")
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

/// A view model over a rig with a root at the origin and a sphere out to +X.
fn rigged() -> ArmatureViewModel {
    let mut vm = ArmatureViewModel::new(FakeRig::default().boxed());
    vm.begin([0.0, 0.0, 0.0]);
    vm.set_symmetric(false);
    vm.press(Grab::Grow(0), [0.0, 0.0, 0.0]);
    vm.drag([1.0, 0.0, 0.0]);
    vm.release();
    vm
}

/// A ray from far along +Z pointing back at the origin plane.
fn ray_at(x: f32, y: f32) -> ([f32; 3], [f32; 3]) {
    ([x, y, 5.0], [0.0, 0.0, -1.0])
}

#[test]
fn there_is_no_rig_until_one_is_started() {
    let mut vm = ArmatureViewModel::new(FakeRig::default().boxed());
    assert!(!vm.is_rigging());

    vm.begin([0.0, 0.0, 0.0]);
    assert!(vm.is_rigging());
    // The root is selected, so the very next drag grows an arm rather than
    // needing a click first.
    assert_eq!(*vm.selected().get(), Some(0));
}

#[test]
fn dragging_out_of_a_sphere_grows_a_child() {
    let vm = rigged();
    let tree = vm.tree().get().clone().expect("a tree");
    assert_eq!(tree.nodes.len(), 2);
    assert_eq!(tree.nodes[1].parent, 0, "the child hangs off what grew it");
    assert_eq!(tree.nodes[1].position[0], 1.0);
    // And it is the new sphere that is selected, so a second drag continues
    // the chain — which is how a limb gets made.
    assert_eq!(*vm.selected().get(), Some(1));
}

#[test]
fn a_grow_gesture_creates_one_sphere_however_far_it_is_dragged() {
    // The child appears on the first movement and is then carried. Creating
    // one per movement would leave a bead trail behind the pointer.
    let mut vm = ArmatureViewModel::new(FakeRig::default().boxed());
    vm.begin([0.0, 0.0, 0.0]);
    vm.set_symmetric(false);

    vm.press(Grab::Grow(0), [0.0, 0.0, 0.0]);
    vm.drag([0.3, 0.0, 0.0]);
    vm.drag([0.6, 0.0, 0.0]);
    vm.drag([0.9, 0.0, 0.0]);
    vm.release();

    let tree = vm.tree().get().clone().expect("a tree");
    assert_eq!(tree.nodes.len(), 2, "the drag left a trail of spheres");
    assert_eq!(tree.nodes[1].position[0], 0.9, "the child did not follow");
}

#[test]
fn dragging_a_sphere_carries_what_hangs_off_it() {
    let mut vm = rigged();
    vm.press(Grab::Grow(1), [1.0, 0.0, 0.0]);
    vm.drag([2.0, 0.0, 0.0]);
    vm.release();

    // Now take hold of the middle sphere and lift.
    vm.press(Grab::Move(1), [2.0, 0.0, 0.0]);
    vm.drag([2.0, 1.0, 0.0]);
    vm.release();

    let tree = vm.tree().get().clone().expect("a tree");
    assert_eq!(tree.nodes[1].position[1], 1.0, "the shoulder did not move");
    assert_eq!(tree.nodes[2].position[1], 1.0, "the arm stayed behind");
    assert_eq!(tree.nodes[0].position[1], 0.0, "the root should not move");
}

#[test]
fn a_press_picks_the_sphere_nearest_the_eye() {
    // Rigs overlap constantly — a shoulder sits inside a torso. Picking the
    // far one would make the big sphere impossible to grab.
    let mut vm = ArmatureViewModel::new(FakeRig::default().boxed());
    vm.begin([0.0, 0.0, -1.0]);
    vm.set_symmetric(false);
    vm.press(Grab::Grow(0), [0.0, 0.0, -1.0]);
    vm.drag([0.0, 0.0, 1.0]);
    vm.release();

    let (origin, direction) = ray_at(0.0, 0.0);
    assert_eq!(vm.pick(origin, direction), Some(1), "picked the far sphere");
}

#[test]
fn a_press_on_nothing_is_not_a_gesture() {
    let mut vm = rigged();
    let before = vm.tree().get().clone();

    let (origin, direction) = ray_at(9.0, 9.0);
    let grab = vm.grab_at(origin, direction, false, false);
    assert_eq!(grab, Grab::Empty);

    vm.press(grab, [9.0, 9.0, 0.0]);
    vm.drag([9.0, 8.0, 0.0]);
    vm.release();
    assert_eq!(
        vm.tree().get().clone(),
        before,
        "empty space edited the rig"
    );
}

#[test]
fn the_modifier_decides_between_moving_and_growing() {
    let vm = rigged();
    let (origin, direction) = ray_at(0.0, 0.0);
    assert_eq!(vm.grab_at(origin, direction, false, false), Grab::Move(0));
    assert_eq!(vm.grab_at(origin, direction, true, false), Grab::Grow(0));
    assert_eq!(vm.grab_at(origin, direction, false, true), Grab::Resize(0));
    // Resizing wins over growing when both are held: the alternative is a
    // sphere that both appears and changes size in one gesture.
    assert_eq!(vm.grab_at(origin, direction, true, true), Grab::Resize(0));
}

#[test]
fn resizing_follows_the_distance_from_the_centre() {
    let mut vm = rigged();
    vm.press(Grab::Resize(0), [0.0, 0.0, 0.0]);
    vm.drag([0.0, 0.5, 0.0]);
    vm.release();

    let tree = vm.tree().get().clone().expect("a tree");
    assert!(
        (tree.nodes[0].radius - 0.5).abs() < 1e-5,
        "radius {} is not the drag distance",
        tree.nodes[0].radius
    );
}

#[test]
fn mirrored_authoring_grows_both_sides() {
    let mut vm = ArmatureViewModel::new(FakeRig::default().boxed());
    vm.begin([0.0, 0.0, 0.0]);
    assert!(*vm.symmetric().get(), "rigging starts symmetric");

    vm.press(Grab::Grow(0), [0.0, 0.0, 0.0]);
    vm.drag([0.4, 0.0, 0.0]);
    // A pointer drag has more than one sample. The first creates both
    // ZSpheres; every later one must preserve their reflection rather than
    // moving only the side that was initially grabbed.
    vm.drag([0.6, 0.2, 0.0]);
    vm.release();

    let tree = vm.tree().get().clone().expect("a tree");
    assert_eq!(tree.nodes.len(), 3, "the reflection was not grown");
    assert_eq!(tree.nodes[1].position, [0.6, 0.2, 0.0]);
    assert_eq!(tree.nodes[2].position, [-0.6, 0.2, 0.0]);
}

#[test]
fn inserting_into_a_mirrored_link_keeps_both_branches_symmetric() {
    let mut vm = ArmatureViewModel::new(FakeRig::default().boxed());
    vm.begin([0.0, 0.0, 0.0]);
    vm.press(Grab::Grow(0), [0.0, 0.0, 0.0]);
    vm.drag([0.6, 0.0, 0.0]);
    vm.release();

    // The pair created above owns links 0→1 and 0→2. Inserting into one
    // must insert into both, then carry the two new joints together.
    vm.press(Grab::Insert(1), [0.3, 0.0, 0.0]);
    vm.drag([0.3, 0.0, 0.0]);
    vm.drag([0.2, 0.25, 0.0]);
    vm.release();

    let tree = vm.tree().get().clone().expect("a tree");
    assert_eq!(
        tree.nodes.len(),
        5,
        "only one link received an inserted joint"
    );
    assert_eq!(tree.nodes[3].position, [0.2, 0.25, 0.0]);
    assert_eq!(tree.nodes[4].position, [-0.2, 0.25, 0.0]);
    assert_eq!(tree.nodes[1].parent, 3);
    assert_eq!(tree.nodes[2].parent, 4);
}

#[test]
fn the_highlight_follows_the_pointer() {
    let mut vm = rigged();
    let (origin, direction) = ray_at(1.0, 0.0);
    vm.hover(origin, direction);
    assert_eq!(*vm.hovered().get(), Some(1));

    let (origin, direction) = ray_at(9.0, 0.0);
    vm.hover(origin, direction);
    assert_eq!(*vm.hovered().get(), None);
}

#[test]
fn removing_the_selection_drops_it_rather_than_leaving_it_dangling() {
    // Indices compact on removal. A stale selection would take hold of
    // whatever moved into that slot, which is the worst kind of wrong.
    let mut vm = rigged();
    assert_eq!(*vm.selected().get(), Some(1));

    vm.remove_selected();
    assert_eq!(*vm.selected().get(), None);
    assert_eq!(vm.tree().get().clone().expect("a tree").nodes.len(), 1);
}

#[test]
fn a_selection_past_the_end_is_dropped_when_the_tree_shrinks() {
    let mut vm = rigged();
    vm.press(Grab::Grow(1), [1.0, 0.0, 0.0]);
    vm.drag([2.0, 0.0, 0.0]);
    vm.release();
    assert_eq!(*vm.selected().get(), Some(2));

    // Something else removes the middle sphere; the view model only finds out
    // when it looks again.
    vm.reparent_selected(0);
    vm.press(Grab::Move(1), [1.0, 0.0, 0.0]);
    vm.release();
    vm.remove_selected();

    let live = vm.tree().get().clone().expect("a tree").nodes.len();
    assert!(vm.selected().get().is_none_or(|i| (i as usize) < live));
}

#[test]
fn a_refusal_is_reported_rather_than_swallowed() {
    let mut vm = ArmatureViewModel::new(
        FakeRig {
            refuse: true,
            ..Default::default()
        }
        .boxed(),
    );
    vm.begin([0.0, 0.0, 0.0]);
    vm.press(Grab::Grow(0), [0.0, 0.0, 0.0]);
    vm.drag([1.0, 0.0, 0.0]);
    assert!(vm.notice().get().is_some(), "the refusal was silent");

    // And the next gesture that works clears it, so a stale complaint does not
    // sit in the status area forever.
    vm.release();
    vm.press(Grab::Move(0), [0.0, 0.0, 0.0]);
    vm.drag([0.1, 0.0, 0.0]);
    assert!(vm.notice().get().is_none());
}

#[test]
fn the_root_refuses_to_be_removed_and_says_so() {
    let mut vm = rigged();
    vm.press(Grab::Move(0), [0.0, 0.0, 0.0]);
    vm.release();
    vm.remove_selected();

    assert_eq!(vm.tree().get().clone().expect("a tree").nodes.len(), 2);
    assert!(vm.notice().get().is_some());
}

#[test]
fn the_skin_setting_goes_through_to_the_model() {
    let mut vm = rigged();
    vm.set_skin(SkinSettings { thickness: 1.8 });
    assert_eq!(vm.skin().get().thickness, 1.8);
}

#[test]
fn clicking_a_link_inserts_a_sphere_between_its_ends() {
    // ZBrush's insert: you aim at the membrane rather than either end, and the
    // new sphere takes the child's place in the chain.
    let mut vm = rigged();
    let tree = vm.tree().get().clone().expect("a tree");
    assert_eq!(tree.nodes.len(), 2);

    // A ray at the midpoint of the only link, which is neither sphere.
    let (origin, direction) = ray_at(0.5, 0.0);
    assert_eq!(vm.pick(origin, direction), None, "that ray hits a sphere");
    assert_eq!(vm.pick_link(origin, direction), Some(1));

    // Only while growing — a bare click on a link should not surprise anyone
    // into inserting a joint.
    assert_eq!(vm.grab_at(origin, direction, false, false), Grab::Empty);
    let grab = vm.grab_at(origin, direction, true, false);
    assert_eq!(grab, Grab::Insert(1));

    vm.press(grab, [0.5, 0.0, 0.0]);
    // The first movement inserts — on the link, which is where an insert
    // belongs — and the rest of the gesture carries it, exactly as growing a
    // child does. So placing it off the line takes a second sample.
    vm.drag([0.5, 0.0, 0.0]);
    vm.drag([0.5, 0.2, 0.0]);
    vm.release();

    let after = vm.tree().get().clone().expect("a tree");
    assert_eq!(after.nodes.len(), 3);
    // The child now hangs off the inserted sphere rather than off the root.
    assert_eq!(after.nodes[1].parent, 2);
    assert_eq!(after.nodes[2].parent, 0);
    // And the rest of the gesture placed it off the line it was inserted on.
    assert!(
        (after.nodes[2].position[1] - 0.2).abs() < 1e-5,
        "the inserted sphere did not follow the pointer: {:?}",
        after.nodes[2].position
    );
}

#[test]
fn a_sphere_that_carries_a_limb_can_still_cut() {
    // This used to be refused. The rule was the old ABI's, not ZBrush's: a
    // negative was placed as a separate subtractive item, so anything hanging
    // off it would have been orphaned. ClayCore 0.30.0 made the sign a
    // property of the node (#99), so a negative may carry children.
    let mut vm = rigged();
    // Grow a third, so the middle sphere carries a limb.
    vm.press(Grab::Grow(1), [1.0, 0.0, 0.0]);
    vm.drag([1.6, 0.0, 0.0]);
    vm.release();

    // The middle one carries the tip, and can cut anyway.
    vm.press(Grab::Move(1), [1.0, 0.0, 0.0]);
    vm.release();
    vm.set_selected_negative(true);
    assert!(
        vm.selected_is_negative(),
        "a sphere with a child was refused: {:?}",
        vm.notice().get()
    );
    assert!(vm.notice().get().is_none());

    // And the limb hanging off it kept its own sign, rather than being
    // dragged negative with its parent.
    let tree = vm.tree().get().clone().expect("a tree");
    assert!(
        !tree.get(2).expect("the tip").negative,
        "flipping a parent's sign dragged its child's with it"
    );

    // The leaf can cut too, independently.
    vm.press(Grab::Move(2), [1.6, 0.0, 0.0]);
    vm.release();
    vm.set_selected_negative(true);
    assert!(
        vm.selected_is_negative(),
        "a leaf could not be made negative"
    );
}

#[test]
fn the_root_cannot_cut() {
    // The one refusal that survives, and it is structural: there would be
    // nothing left for the root to cut into.
    let mut vm = rigged();
    vm.press(Grab::Move(0), [0.0, 0.0, 0.0]);
    vm.release();
    vm.set_selected_negative(true);
    assert!(!vm.selected_is_negative());
    assert!(
        vm.notice().get().is_some(),
        "making the root negative was accepted silently"
    );
}
