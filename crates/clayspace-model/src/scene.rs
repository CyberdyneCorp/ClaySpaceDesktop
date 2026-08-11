//! The scene and its layers, as the interface presents them.
//!
//! Plain values, refreshed from the document rather than derived by the
//! interface. The View reads these; it never asks the engine what a layer is.

use crate::tools::Representation;

/// Identifies a layer, opaquely.
///
/// The interface passes it back to name a layer without knowing what the
/// engine calls one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerKey(pub u64);

/// Whether a layer is shown, pickable and editable.
///
/// The three states are distinct: a ghosted layer is visible but neither
/// pickable nor editable, while a locked one is still pickable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Protection {
    pub ghost: bool,
    pub locked: bool,
}

impl Protection {
    pub fn is_editable(self) -> bool {
        !self.ghost && !self.locked
    }

    pub fn is_pickable(self) -> bool {
        !self.ghost
    }

    /// What the interface says when an edit is refused.
    pub fn refusal(self) -> Option<&'static str> {
        if self.ghost {
            Some("esta camada está fantasma")
        } else if self.locked {
            Some("esta camada está bloqueada")
        } else {
            None
        }
    }
}

/// One layer, as the layer stack shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerSummary {
    pub key: LayerKey,
    pub name: String,
    pub representation: Representation,
    pub visible: bool,
    pub protection: Protection,
    /// 0..=100, as the design's stack displays it.
    pub intensity: u8,
}

impl LayerSummary {
    /// Whether an edit may touch this layer.
    pub fn is_editable(&self) -> bool {
        self.visible && self.protection.is_editable()
    }
}

/// One entry in the scene tree.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneNode {
    pub key: LayerKey,
    pub name: String,
    /// How deep in the tree, for indentation.
    pub depth: usize,
    pub visible: bool,
    /// Whether it has children that could be shown.
    pub expandable: bool,
}

/// What the interface knows about the document's structure.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub nodes: Vec<SceneNode>,
    pub layers: Vec<LayerSummary>,
    /// Which layer edits go to.
    pub active: Option<LayerKey>,
    /// What a click last selected.
    pub selected: Option<LayerKey>,
}

impl Scene {
    pub fn active_layer(&self) -> Option<&LayerSummary> {
        let key = self.active?;
        self.layers.iter().find(|layer| layer.key == key)
    }

    pub fn layer(&self, key: LayerKey) -> Option<&LayerSummary> {
        self.layers.iter().find(|layer| layer.key == key)
    }
}

/// What the interface can ask of the scene.
///
/// Separate from [`crate::SculptModel`] because a scene panel and a brush are
/// different concerns, and a test double for one need not implement the other.
pub trait SceneModel {
    fn scene(&self) -> Scene;

    fn set_active_layer(&mut self, key: LayerKey) -> Result<(), crate::ModelError>;
    fn set_layer_visible(&mut self, key: LayerKey, visible: bool)
        -> Result<(), crate::ModelError>;
    fn set_layer_protection(
        &mut self,
        key: LayerKey,
        protection: Protection,
    ) -> Result<(), crate::ModelError>;
    fn rename_layer(&mut self, key: LayerKey, name: &str) -> Result<(), crate::ModelError>;
    fn add_layer(&mut self, name: &str, representation: Representation)
        -> Result<LayerKey, crate::ModelError>;
    fn remove_layer(&mut self, key: LayerKey) -> Result<(), crate::ModelError>;
    /// Moves a layer to a position in the stack, which is its evaluation order.
    fn move_layer(&mut self, key: LayerKey, index: usize) -> Result<(), crate::ModelError>;

    /// What a click at a ray selects, honouring ghost and lock.
    fn select_at(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<LayerKey>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_protection_states_are_distinct() {
        let plain = Protection::default();
        assert!(plain.is_editable() && plain.is_pickable());
        assert!(plain.refusal().is_none());

        let ghost = Protection {
            ghost: true,
            locked: false,
        };
        assert!(!ghost.is_pickable(), "a ghosted layer is not pickable");
        assert!(!ghost.is_editable());
        assert!(ghost.refusal().is_some(), "a refusal must be explainable");

        let locked = Protection {
            ghost: false,
            locked: true,
        };
        assert!(locked.is_pickable(), "a locked layer is still pickable");
        assert!(!locked.is_editable());
    }

    #[test]
    fn a_hidden_layer_is_not_editable_whatever_its_protection() {
        let layer = LayerSummary {
            key: LayerKey(1),
            name: "Base".into(),
            representation: Representation::Sdf,
            visible: false,
            protection: Protection::default(),
            intensity: 100,
        };
        assert!(
            !layer.is_editable(),
            "editing something that is not shown would be an edit nobody can see"
        );
    }

    #[test]
    fn a_scene_finds_its_active_layer() {
        let scene = Scene {
            nodes: Vec::new(),
            layers: vec![
                LayerSummary {
                    key: LayerKey(1),
                    name: "A".into(),
                    representation: Representation::Sdf,
                    visible: true,
                    protection: Protection::default(),
                    intensity: 100,
                },
                LayerSummary {
                    key: LayerKey(2),
                    name: "B".into(),
                    representation: Representation::Voxel,
                    visible: true,
                    protection: Protection::default(),
                    intensity: 70,
                },
            ],
            active: Some(LayerKey(2)),
            selected: None,
        };
        assert_eq!(scene.active_layer().map(|l| l.name.as_str()), Some("B"));
        assert_eq!(scene.layer(LayerKey(1)).map(|l| l.intensity), Some(100));
        assert!(scene.layer(LayerKey(9)).is_none());
    }
}
