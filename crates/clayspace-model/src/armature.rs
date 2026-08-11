//! ZSpheres: a tree of spheres, skinned by a cone between each node and its
//! parent.
//!
//! The engine's primitive is the same one ZBrush made famous, and the feel
//! that matters is a small one: you drag a new sphere *out of* an existing one,
//! and everything under a sphere travels with it. A shoulder moves and the arm
//! comes along. That is what makes a ZSphere rig feel like a puppet rather than
//! a scatter of balls.
//!
//! The host holds the tree because the engine's parent array cannot be read
//! back — positions and radii can, the topology cannot. So this is the record,
//! and the engine is written to from it.

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
        });
        index
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

    fn set_skin(&mut self, skin: SkinSettings) -> Result<(), ModelError>;
    fn skin(&self) -> SkinSettings;
}

#[cfg(test)]
mod tests {
    use super::*;

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
