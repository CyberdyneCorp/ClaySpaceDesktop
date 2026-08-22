//! ZSpheres: a tree of spheres, skinned by a cone between each node and its
//! parent.
//!
//! The engine's primitive is the same one ZBrush made famous, and the feel
//! that matters is a small one: you drag a new sphere *out of* an existing one,
//! and everything under a sphere travels with it. A shoulder moves and the arm
//! comes along. That is what makes a ZSphere rig feel like a puppet rather than
//! a scatter of balls.
//!
//! The host holds the tree as the authoring record and writes the engine from
//! it. It no longer *has* to: ClayCore 0.29.0 made the parent array readable
//! (#77) and 0.30.0 the signs (#99), which is what makes a reopened rig
//! posable. Keeping the host copy is about authoring — radii are stored as
//! authored and scaled on the way out, so the thickness slider stays
//! reversible.

use crate::sculpt::ModelError;

/// A node's index in the armature. Stable for the life of a tree.
pub type NodeIndex = u32;

/// One sphere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zsphere {
    pub position: [f32; 3],
    pub radius: f32,
    /// Its parent, or itself when it is the root.
    pub parent: NodeIndex,
    /// Whether this sphere cuts into the rig rather than adding to it.
    ///
    /// ZBrush's negative ZSphere, which is made by pushing one inside its
    /// parent: it stops contributing skin and starts carving an indentation.
    ///
    /// Expressed to the engine as the node's own sign since ClayCore 0.30.0
    /// (#99). Before that the primitive carried one op for the whole item, so
    /// a negative had to be placed as a separate subtractive sphere — which
    /// left the membrane along its links uncut and lost the sign on reload,
    /// and is why only a leaf was allowed to be one.
    pub negative: bool,
}

impl Zsphere {
    pub fn is_root(&self, index: NodeIndex) -> bool {
        self.parent == index
    }
}

/// The tree, as the interface holds it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Armature {
    pub nodes: Vec<Zsphere>,
}

impl Armature {
    /// A tree with one sphere at the origin.
    pub fn rooted(position: [f32; 3], radius: f32) -> Self {
        Self {
            nodes: vec![Zsphere {
                position,
                radius,
                parent: 0,
                negative: false,
            }],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn get(&self, index: NodeIndex) -> Option<&Zsphere> {
        self.nodes.get(index as usize)
    }

    /// Every node under `index`, including itself.
    ///
    /// The set a move carries and a delete removes. Walked breadth-first from
    /// the node rather than by scanning for descendants repeatedly, so a deep
    /// chain costs the same as a wide one.
    pub fn subtree(&self, index: NodeIndex) -> Vec<NodeIndex> {
        let mut found = vec![index];
        let mut at = 0;
        while at < found.len() {
            let parent = found[at];
            for (i, node) in self.nodes.iter().enumerate() {
                let i = i as NodeIndex;
                // A root is its own parent, so the guard keeps it from being
                // its own child and looping forever.
                if node.parent == parent && i != parent {
                    found.push(i);
                }
            }
            at += 1;
        }
        found
    }

    /// The pairs a skin is built from: each node and the parent it hangs off.
    pub fn links(&self) -> Vec<(NodeIndex, NodeIndex)> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(i, node)| node.parent != *i as NodeIndex)
            .map(|(i, node)| (i as NodeIndex, node.parent))
            .collect()
    }

    /// Whether making `index` a child of `new_parent` would close a cycle.
    ///
    /// The engine refuses a cycle, and rightly — the field would depend on
    /// traversal order rather than on the tree. Catching it here means the
    /// interface can grey the drop out rather than let it fail.
    pub fn would_cycle(&self, index: NodeIndex, new_parent: NodeIndex) -> bool {
        index == new_parent || self.subtree(index).contains(&new_parent)
    }

    /// Adds a sphere under `parent`, returning its index.
    pub fn add_child(&mut self, parent: NodeIndex, position: [f32; 3], radius: f32) -> NodeIndex {
        let index = self.nodes.len() as NodeIndex;
        self.nodes.push(Zsphere {
            position,
            radius,
            parent,
            negative: false,
        });
        index
    }

    /// Puts a sphere on the link between a node and its parent.
    ///
    /// ZBrush's insert: you click the membrane rather than either end, and the
    /// new sphere takes the child's place in the chain — the child hangs off
    /// it, so everything below comes along. Inserting on a root's own link is
    /// refused, because a root has no link.
    ///
    /// Its radius is the mean of the two it sits between, which is what makes
    /// an inserted joint look like it belongs rather than like a bead.
    pub fn insert_on_link(&mut self, child: NodeIndex) -> Option<NodeIndex> {
        let node = *self.get(child)?;
        if node.is_root(child) {
            return None;
        }
        let parent = *self.get(node.parent)?;

        let midpoint = [
            (node.position[0] + parent.position[0]) * 0.5,
            (node.position[1] + parent.position[1]) * 0.5,
            (node.position[2] + parent.position[2]) * 0.5,
        ];
        let inserted = self.add_child(node.parent, midpoint, (node.radius + parent.radius) * 0.5);
        // The child now hangs off the new sphere rather than off its old
        // parent, which is what puts it *between* them rather than beside.
        self.nodes[child as usize].parent = inserted;
        Some(inserted)
    }

    /// Whether a sphere cuts rather than adds.
    ///
    /// One refusal, and it is structural: a root cannot cut, because there
    /// would be nothing left to cut into — the field would be the whole rig
    /// subtracted from nothing.
    ///
    /// A negative sphere used to have to be a leaf as well. That was never
    /// ZBrush's rule, only the old ABI's: a negative was placed as a separate
    /// subtractive item, so anything hanging off it would have been orphaned.
    /// Since ClayCore 0.30.0 the sign belongs to the node (#99) and a negative
    /// may carry children, which keep their own signs.
    pub fn set_negative(&mut self, index: NodeIndex, negative: bool) -> Result<(), ModelError> {
        let node = self
            .get(index)
            .copied()
            .ok_or_else(|| ModelError::engine("essa esfera não existe"))?;
        if negative && node.is_root(index) {
            return Err(ModelError::engine("a raiz não pode ser negativa"));
        }
        self.nodes[index as usize].negative = negative;
        Ok(())
    }

    /// One sign per node, in node order, for the engine's sign array.
    pub fn signs(&self) -> Vec<bool> {
        self.nodes.iter().map(|node| node.negative).collect()
    }

    /// Moves a node and everything under it.
    pub fn move_subtree(&mut self, index: NodeIndex, delta: [f32; 3]) {
        for i in self.subtree(index) {
            if let Some(node) = self.nodes.get_mut(i as usize) {
                for (axis, step) in delta.iter().enumerate() {
                    node.position[axis] += step;
                }
            }
        }
    }

    pub fn set_radius(&mut self, index: NodeIndex, radius: f32) {
        if let Some(node) = self.nodes.get_mut(index as usize) {
            node.radius = radius.max(0.001);
        }
    }

    /// Hangs `index` off a different parent.
    ///
    /// Refused rather than silently ignored when it would close a cycle: a
    /// reparent that quietly does nothing is worse than one that says why.
    pub fn reparent(&mut self, index: NodeIndex, new_parent: NodeIndex) -> Result<(), ModelError> {
        if self.would_cycle(index, new_parent) {
            return Err(ModelError::engine("isso faria a árvore fechar um ciclo"));
        }
        if self.nodes.get(new_parent as usize).is_none() {
            return Err(ModelError::engine("esse pai não existe"));
        }
        if let Some(node) = self.nodes.get_mut(index as usize) {
            node.parent = new_parent;
        }
        Ok(())
    }

    /// Removes a node and its subtree, returning what went.
    ///
    /// Indices are compacted, so anything held across this call must be
    /// remapped — which is why the removed set comes back rather than a bare
    /// success.
    pub fn remove(&mut self, index: NodeIndex) -> Vec<NodeIndex> {
        let mut removed = self.subtree(index);
        removed.sort_unstable();

        let keep: Vec<NodeIndex> = (0..self.nodes.len() as NodeIndex)
            .filter(|i| !removed.contains(i))
            .collect();
        // Old index to new, so surviving parents still point at their parents.
        let mut remap = vec![None; self.nodes.len()];
        for (new, old) in keep.iter().enumerate() {
            remap[*old as usize] = Some(new as NodeIndex);
        }

        let mut nodes = Vec::with_capacity(keep.len());
        for old in &keep {
            let mut node = self.nodes[*old as usize];
            node.parent = remap[node.parent as usize].unwrap_or_else(|| {
                // Its parent went with the subtree, which can only happen to a
                // node that was itself in it. Being defensive: make it a root
                // rather than leave a dangling index.
                remap[*old as usize].expect("kept")
            });
            nodes.push(node);
        }
        self.nodes = nodes;
        removed
    }

    /// The reflection of a node through x = 0, if it is off the plane.
    ///
    /// A node on the plane is its own reflection, which is the engine's rule
    /// for mirrored authoring and the one that stops a spine growing two of
    /// everything.
    pub fn mirrored_position(position: [f32; 3]) -> Option<[f32; 3]> {
        (position[0].abs() > 1e-4).then_some([-position[0], position[1], position[2]])
    }
}

/// How thick the skin is over the spheres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinSettings {
    /// A multiplier on every authored radius.
    ///
    /// The engine skins an armature with one sphere-swept cone per
    /// node-parent pair, and the cone is defined by the two radii — there is
    /// no separate smoothing term to reach for. `clay_item_set_stroke_blend_k`
    /// was tried and is refused: it needs `CLAY_PRIM_STROKE`.
    ///
    /// So thickness is exactly what it can honestly be: the tree stores the
    /// radii a sculptor authored, and this scales them on the way to the
    /// engine. Thinning a whole rig then does not lose the proportions
    /// between its joints, which is the thing a per-node edit would cost.
    pub thickness: f32,
}

impl Default for SkinSettings {
    fn default() -> Self {
        Self { thickness: 1.0 }
    }
}

impl SkinSettings {
    /// The radius the engine is given for an authored one.
    pub fn radius_for(self, authored: f32) -> f32 {
        (authored * self.thickness.clamp(0.05, 4.0)).max(0.001)
    }
}

/// Authoring a tree of spheres.
pub trait ArmatureModel {
    /// The tree as it stands, or `None` when the active layer has no armature.
    fn armature(&self) -> Option<Armature>;

    /// Starts one, replacing any the active layer had.
    fn begin_armature(&mut self, position: [f32; 3], radius: f32) -> Result<(), ModelError>;

    /// Adds a sphere under `parent`, mirroring it when asked.
    fn add_zsphere(
        &mut self,
        parent: NodeIndex,
        position: [f32; 3],
        radius: f32,
        mirrored: bool,
    ) -> Result<NodeIndex, ModelError>;

    fn move_zsphere(&mut self, index: NodeIndex, delta: [f32; 3]) -> Result<(), ModelError>;
    fn resize_zsphere(&mut self, index: NodeIndex, radius: f32) -> Result<(), ModelError>;
    fn reparent_zsphere(
        &mut self,
        index: NodeIndex,
        new_parent: NodeIndex,
    ) -> Result<(), ModelError>;
    fn remove_zsphere(&mut self, index: NodeIndex) -> Result<(), ModelError>;

    /// Puts a sphere on the link between `child` and its parent.
    fn insert_zsphere(&mut self, child: NodeIndex) -> Result<NodeIndex, ModelError>;

    /// Makes a sphere cut into the rig rather than add to it.
    fn set_zsphere_negative(&mut self, index: NodeIndex, negative: bool) -> Result<(), ModelError>;

    fn set_skin(&mut self, skin: SkinSettings) -> Result<(), ModelError>;
    fn skin(&self) -> SkinSettings;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sphere_inserted_on_a_link_takes_the_childs_place() {
        // ZBrush's insert: click the membrane, and the new sphere goes
        // *between* rather than beside — so the child, and everything under
        // it, hangs off the new one.
        let mut a = Armature::rooted([0.0, 0.0, 0.0], 0.4);
        let elbow = a.add_child(0, [1.0, 0.0, 0.0], 0.2);
        let hand = a.add_child(elbow, [1.6, 0.0, 0.0], 0.1);

        let inserted = a.insert_on_link(elbow).expect("a link to insert on");
        assert_eq!(a.nodes[elbow as usize].parent, inserted);
        assert_eq!(a.nodes[inserted as usize].parent, 0);
        assert_eq!(a.nodes[hand as usize].parent, elbow, "the hand was moved");

        // Midway, and sized between the two it sits between.
        assert_eq!(a.nodes[inserted as usize].position, [0.5, 0.0, 0.0]);
        assert!((a.nodes[inserted as usize].radius - 0.3).abs() < 1e-6);

        // And the subtree still walks: moving the root carries everything.
        assert_eq!(a.subtree(0).len(), 4);
    }

    #[test]
    fn a_root_has_no_link_to_insert_on() {
        let mut a = Armature::rooted([0.0, 0.0, 0.0], 0.4);
        assert!(a.insert_on_link(0).is_none());
    }

    #[test]
    fn a_negative_sphere_may_carry_children() {
        // The rule that went with #99. It was the old ABI's, not ZBrush's: a
        // negative used to be a separate subtractive item, so anything hanging
        // off it would have been orphaned.
        let mut a = Armature::rooted([0.0, 0.0, 0.0], 0.4);
        let socket = a.add_child(0, [0.0, 0.5, 0.0], 0.15);
        a.set_negative(socket, true).expect("a negative sphere");
        let under = a.add_child(socket, [0.0, 0.8, 0.0], 0.1);

        assert!(a.get(socket).expect("the socket").negative);
        assert!(
            !a.get(under).expect("its child").negative,
            "a new child is positive whatever its parent is"
        );
        assert_eq!(
            a.subtree(socket),
            vec![socket, under],
            "the child still hangs off the negative sphere"
        );
    }

    #[test]
    fn a_negative_sphere_can_be_given_children_after_the_fact() {
        let mut a = Armature::rooted([0.0, 0.0, 0.0], 0.4);
        let tip = a.add_child(0, [1.0, 0.0, 0.0], 0.2);
        let below = a.add_child(tip, [1.4, 0.0, 0.0], 0.1);
        // Making a node with children negative is now allowed, where it used
        // to be refused outright.
        a.set_negative(tip, true)
            .expect("a sphere with children can be negative");
        assert!(a.get(tip).expect("the tip").negative);
        assert!(!a.get(below).expect("its child").negative);
    }

    #[test]
    fn the_root_still_cannot_cut() {
        // The one refusal that is structural rather than an ABI limit: there
        // would be nothing left for the root to cut into.
        let mut a = Armature::rooted([0.0, 0.0, 0.0], 0.4);
        assert!(a.set_negative(0, true).is_err());
        assert!(!a.get(0).expect("the root").negative);
    }

    #[test]
    fn signs_are_one_per_node_in_node_order() {
        // What the engine's sign array is built from, so an off-by-one here
        // would carve the wrong sphere.
        let mut a = Armature::rooted([0.0, 0.0, 0.0], 0.4);
        let left = a.add_child(0, [-1.0, 0.0, 0.0], 0.2);
        let cut = a.add_child(0, [0.0, 0.5, 0.0], 0.15);
        a.add_child(0, [1.0, 0.0, 0.0], 0.2);
        a.set_negative(cut, true).expect("a negative sphere");

        assert_eq!(a.signs(), vec![false, false, true, false]);
        let _ = left;
    }

    /// A shoulder with an arm hanging off it, and a second branch.
    fn rig() -> Armature {
        let mut a = Armature::rooted([0.0, 0.0, 0.0], 0.3);
        let shoulder = a.add_child(0, [0.4, 0.2, 0.0], 0.2);
        let elbow = a.add_child(shoulder, [0.8, 0.2, 0.0], 0.15);
        a.add_child(elbow, [1.1, 0.2, 0.0], 0.1);
        a.add_child(0, [-0.4, 0.2, 0.0], 0.2);
        a
    }

    #[test]
    fn a_subtree_is_the_node_and_everything_under_it() {
        let a = rig();
        let mut arm = a.subtree(1);
        arm.sort_unstable();
        assert_eq!(arm, vec![1, 2, 3], "the arm is the shoulder and below");

        let mut all = a.subtree(0);
        all.sort_unstable();
        assert_eq!(all, vec![0, 1, 2, 3, 4], "the root carries everything");

        assert_eq!(a.subtree(3), vec![3], "a tip carries only itself");
    }

    #[test]
    fn moving_a_shoulder_takes_the_arm_with_it() {
        // The feel that makes a ZSphere rig a puppet rather than a scatter.
        let mut a = rig();
        let before: Vec<[f32; 3]> = a.nodes.iter().map(|n| n.position).collect();
        a.move_subtree(1, [0.0, 0.5, 0.0]);

        for i in [1, 2, 3] {
            assert_eq!(
                a.nodes[i].position[1],
                before[i][1] + 0.5,
                "node {i} did not travel with the shoulder"
            );
        }
        for i in [0, 4] {
            assert_eq!(a.nodes[i].position, before[i], "node {i} should not move");
        }
    }

    #[test]
    fn a_reparent_that_would_close_a_cycle_is_refused() {
        // The engine refuses cycles; catching it here lets the interface grey
        // the drop out rather than let the edit fail.
        let mut a = rig();
        assert!(a.would_cycle(1, 3), "the elbow's tip is under the shoulder");
        assert!(a.reparent(1, 3).is_err());
        assert!(a.would_cycle(1, 1), "a node cannot parent itself");
        assert!(a.reparent(1, 1).is_err());

        // Across branches is fine.
        assert!(a.reparent(4, 2).is_ok());
        assert_eq!(a.nodes[4].parent, 2);
    }

    #[test]
    fn removing_a_node_takes_its_subtree_and_keeps_the_rest_intact() {
        let mut a = rig();
        let removed = a.remove(1);
        assert_eq!(removed, vec![1, 2, 3]);
        assert_eq!(a.nodes.len(), 2, "the root and the other branch survive");

        // The survivor's parent still points at the root after compaction.
        assert!(a.nodes[0].is_root(0));
        assert_eq!(a.nodes[1].parent, 0, "the second branch lost its parent");
    }

    #[test]
    fn links_are_one_per_node_except_the_root() {
        let a = rig();
        assert_eq!(a.links().len(), a.nodes.len() - 1);
        assert!(
            !a.links().iter().any(|(child, _)| *child == 0),
            "the root hangs off nothing"
        );
    }

    #[test]
    fn a_sphere_on_the_mirror_plane_is_its_own_reflection() {
        assert_eq!(Armature::mirrored_position([0.0, 1.0, 0.0]), None);
        assert_eq!(
            Armature::mirrored_position([0.4, 0.2, 0.0]),
            Some([-0.4, 0.2, 0.0])
        );
    }

    #[test]
    fn skin_thickness_scales_the_rig_without_flattening_it() {
        // The proportions between joints have to survive: a rig thinned as a
        // whole is still the same rig, where per-node edits would lose that.
        let thin = SkinSettings { thickness: 0.5 };
        let thick = SkinSettings { thickness: 2.0 };
        assert!(thin.radius_for(0.2) < thick.radius_for(0.2));
        assert!(
            (thin.radius_for(0.4) / thin.radius_for(0.2) - 2.0).abs() < 1e-5,
            "scaling changed the ratio between two joints"
        );
        // And it is clamped, so a slider at either end still leaves geometry.
        assert!(SkinSettings { thickness: 0.0 }.radius_for(0.2) > 0.0);
    }
}
