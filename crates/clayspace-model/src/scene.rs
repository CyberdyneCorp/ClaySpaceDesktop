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
    /// What this layer's field costs to evaluate, where it has one.
    ///
    /// `None` for a mesh or a grid, which hold no edit list to steepen. Read
    /// from the engine's own report, which is cheap — 33 µs against the 287 ms
    /// of the byte estimate beside it, which is why the estimate is asked for
    /// only when the sculptor is deciding rather than on every refresh.
    pub health: Option<FieldHealth>,
    /// What the layer's grid is made of, where it is one.
    ///
    /// `None` for a field or a mesh, which have no cells. Read from the engine
    /// as the health is — `clay_voxel_size` and `clay_voxel_occupied_count`,
    /// both bound in `claycore` and until now used only inside the adapter, so
    /// the interface could say a layer was voxels and not how coarse they were.
    pub voxel: Option<VoxelStats>,
    /// The recorded passes on this layer, bottom-up.
    ///
    /// Empty for every representation but a grid, and for a grid nobody has
    /// recorded a pass on. Nested here rather than kept in a panel of its own
    /// because a sculpt layer is *part of* the layer it was recorded on — it
    /// has no meaning apart from that grid, and a second stack elsewhere would
    /// have to repeat which layer each entry belongs to.
    pub sculpt_layers: Vec<SculptLayer>,
}

/// What a field layer's edit list costs, and whether the engine advises
/// collapsing it.
///
/// A chain of edits steepens the field it produces: each bake resamples what
/// the last one left, until a ray march has to take many small steps and every
/// dab pays for it. The engine measures that and says when it is worth
/// collapsing the layer into one volume — which is expensive and destructive
/// enough that it is the sculptor's decision, never something taken quietly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldHealth {
    /// How many items the layer's edit list holds.
    pub items: i32,
    /// Multiply a distance by this before stepping along a ray. A low value
    /// means the field has steepened and a march takes many small steps.
    pub safe_step_scale: f32,
    /// Whether the engine advises collapsing the layer.
    pub advises_consolidation: bool,
    /// Whether it is already collapsed.
    pub consolidated: bool,
}

/// What a grid layer is made of.
///
/// Two numbers a sculptor can act on. The cell size is the one that decides
/// what detail is possible at all — a feature finer than a cell cannot be
/// sculpted, and the answer to "why will this not take a crease" is usually
/// here. The occupied count is what a grid costs, and the thing that grows
/// when a form is refined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelStats {
    /// World units per cell.
    pub cell_size: f32,
    /// How many cells hold anything.
    pub occupied: usize,
}

/// A recorded pass on a voxel layer.
///
/// A pass a sculptor can dial back after making it. Not undo: undo is a stack
/// you pop, this is a slider you keep — and what it stores is what the pass
/// *changed*, not the brushes that changed it, so dialling one replays cells
/// rather than re-running strokes.
#[derive(Debug, Clone, PartialEq)]
pub struct SculptLayer {
    /// Its position in the stack, which is what every operation addresses it
    /// by. Layers composite bottom-up, so a higher index wins where two
    /// overlap.
    pub index: usize,
    /// What the sculptor called it. May be empty.
    pub name: String,
    /// How far the pass is dialled in, 0..=1.
    pub strength: f32,
    pub visible: bool,
    /// How many cells the pass changed — its cost, and whether it did
    /// anything at all.
    pub cells: usize,
    /// What it occupies: recorded cells plus the recording index.
    pub bytes: usize,
}

impl SculptLayer {
    /// What the interface shows when the pass has no name.
    ///
    /// Numbered from one rather than zero: the index is an implementation
    /// detail of the stack and a sculptor counting passes starts at one.
    pub fn display_name(&self) -> String {
        if self.name.is_empty() {
            format!("Passe {}", self.index + 1)
        } else {
            self.name.clone()
        }
    }

    /// Whether the pass recorded anything.
    ///
    /// A pass that changed no cell is not a mistake — a sculptor may have
    /// started recording and thought better of it — but it is worth showing
    /// differently from one that did, since dialling it does nothing.
    pub fn is_empty(&self) -> bool {
        self.cells == 0
    }
}

impl LayerSummary {
    /// Whether an edit may touch this layer.
    pub fn is_editable(&self) -> bool {
        self.visible && self.protection.is_editable()
    }
}

/// Something done to a recorded pass.
///
/// One enum rather than a method per verb on the model, for the reason
/// [`crate::LayerOperation`] is one: they all address a pass by its index in a
/// grid's stack, and a trait growing eight near-identical methods makes every
/// double implement eight refusals.
#[derive(Debug, Clone, PartialEq)]
pub enum SculptLayerOp {
    /// Starts recording. Edits until [`Self::EndRecording`] belong to the new
    /// pass.
    BeginRecording {
        name: String,
    },
    /// Stops recording. Edits after it belong to no pass.
    EndRecording,
    /// Dials a pass up or down, 0..=1.
    SetStrength {
        index: usize,
        strength: f32,
    },
    SetVisible {
        index: usize,
        visible: bool,
    },
    Remove {
        index: usize,
    },
    /// Folds a pass into the one below at full strength.
    MergeDown {
        index: usize,
    },
    /// Moves a pass within the stack. Order decides which pass wins where two
    /// touched the same cell.
    Move {
        from: usize,
        to: usize,
    },
}

impl SculptLayerOp {
    /// What the history calls it.
    pub fn label(&self) -> &'static str {
        match self {
            Self::BeginRecording { .. } => "begin pass",
            Self::EndRecording => "end pass",
            Self::SetStrength { .. } => "pass strength",
            Self::SetVisible { .. } => "pass visibility",
            Self::Remove { .. } => "remove pass",
            Self::MergeDown { .. } => "merge pass down",
            Self::Move { .. } => "reorder pass",
        }
    }

    /// Whether this changes the surface rather than only the stack.
    ///
    /// Beginning and ending a recording change nothing that is drawn — they
    /// decide where the *next* edits are filed. Everything else replays cells.
    pub fn changes_the_surface(&self) -> bool {
        !matches!(self, Self::BeginRecording { .. } | Self::EndRecording)
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

/// What a grid's recorded passes cost together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SculptLayerCost {
    pub layers: usize,
    /// Recorded cells plus the recording index, across the stack.
    ///
    /// Nothing is enforced against it, deliberately: a cap that silently
    /// stopped recording would leave a pass on the grid and un-dialable, which
    /// is a correctness bug wearing a memory limit's clothes. The number is
    /// shown so a sculptor can merge down — one entry per cell instead of two —
    /// or stop recording.
    pub bytes: usize,
    /// Whether a pass is being recorded right now.
    pub recording: bool,
}

impl SculptLayerCost {
    /// The cost in whole megabytes, for a readout.
    pub fn megabytes(&self) -> f32 {
        self.bytes as f32 / (1024.0 * 1024.0)
    }

    /// Above which the interface says the stack is worth merging.
    ///
    /// Not a limit and not enforced — see [`Self::bytes`]. A quarter of a
    /// gigabyte is where a stack is large enough that a sculptor should know
    /// about it, and small enough that saying so is not nagging.
    pub const WORTH_MENTIONING: usize = 256 * 1024 * 1024;

    pub fn worth_merging(&self) -> bool {
        self.bytes > Self::WORTH_MENTIONING
    }
}

/// What the interface knows about the document's structure.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub nodes: Vec<SceneNode>,
    pub layers: Vec<LayerSummary>,
    /// Which layer edits go to, and which one is selected.
    ///
    /// One field and not two. There used to be a `selected` beside it, set by
    /// the viewport pick, while `active` was set by the stack row — two
    /// mutations of one idea, and they drifted the moment a click resolved to a
    /// layer the stack had not chosen. The spec settles it: "there is one
    /// active layer, not a picked one and a sculpted one".
    pub active: Option<LayerKey>,
    /// Which layer is being shown alone, while one is.
    ///
    /// Apart from `active` because they are different questions and the spec
    /// keeps them apart: soloing a layer shows it alone without making it the
    /// thing a brush lands on. The stack reads this to say which row's solo is
    /// engaged, since the visibility flags alone cannot tell a solo from a
    /// sculptor who hid three layers by hand.
    pub soloed: Option<LayerKey>,
}

impl Scene {
    pub fn active_layer(&self) -> Option<&LayerSummary> {
        let key = self.active?;
        self.layers.iter().find(|layer| layer.key == key)
    }

    /// Whether this layer is the one being shown alone.
    pub fn is_soloed(&self, key: LayerKey) -> bool {
        self.soloed == Some(key)
    }

    pub fn layer(&self, key: LayerKey) -> Option<&LayerSummary> {
        self.layers.iter().find(|layer| layer.key == key)
    }
}

/// What a layer's field costs, and whether collapsing it is advised.
///
/// Reported before consolidation is offered, because it is expensive and the
/// user decides — never performed unasked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerCost {
    /// How many items the layer's edit list holds.
    pub items: i32,
    /// Multiply a distance by this before stepping along a ray. A low value
    /// means the field has steepened and a march takes many small steps.
    pub safe_step_scale: f32,
    /// Whether the engine advises collapsing the layer.
    pub advises_consolidation: bool,
    /// What consolidating would occupy, in bytes.
    pub estimated_bytes: u64,
    /// Whether it is already collapsed.
    pub consolidated: bool,
}

/// What the interface can ask of the scene.
///
/// Separate from [`crate::SculptModel`] because a scene panel and a brush are
/// different concerns, and a test double for one need not implement the other.
pub trait SceneModel {
    fn scene(&self) -> Scene;

    fn set_active_layer(&mut self, key: LayerKey) -> Result<(), crate::ModelError>;
    fn set_layer_visible(&mut self, key: LayerKey, visible: bool) -> Result<(), crate::ModelError>;

    /// Shows one layer alone, or releases the solo and puts back the
    /// visibility every layer had before it.
    ///
    /// The state to be in rather than a toggle. A toggle is answered from what
    /// the caller believes is shown, and the interface and the document
    /// disagreeing about that is exactly how a solo is left engaged over a
    /// scene that is no longer soloed.
    ///
    /// Solo is a viewing convenience: it does not change which layer is active
    /// and it adds nothing the sculptor has to undo.
    ///
    /// Provided, so a double with no visibility of its own says it has none
    /// rather than pretending to have soloed something.
    fn set_solo(&mut self, key: Option<LayerKey>) -> Result<(), crate::ModelError> {
        let _ = key;
        Err(crate::ModelError::engine(
            "mostrar uma camada sozinha precisa de um documento",
        ))
    }
    fn set_layer_protection(
        &mut self,
        key: LayerKey,
        protection: Protection,
    ) -> Result<(), crate::ModelError>;
    fn rename_layer(&mut self, key: LayerKey, name: &str) -> Result<(), crate::ModelError>;
    fn add_layer(
        &mut self,
        name: &str,
        representation: Representation,
    ) -> Result<LayerKey, crate::ModelError>;
    fn remove_layer(&mut self, key: LayerKey) -> Result<(), crate::ModelError>;

    /// Acts on the active layer's recorded passes.
    ///
    /// Provided, so a double that models no grids refuses rather than spelling
    /// out seven refusals it never reaches.
    fn apply_sculpt_layer_op(&mut self, op: SculptLayerOp) -> Result<(), crate::ModelError> {
        let _ = op;
        Err(crate::ModelError::engine(
            "passes são gravados em uma camada de voxels",
        ))
    }

    /// What the active layer's passes cost, and whether one is recording.
    fn sculpt_layer_cost(&self) -> SculptLayerCost {
        SculptLayerCost::default()
    }
    /// Moves a layer to a position in the stack, which is its evaluation order.
    fn move_layer(&mut self, key: LayerKey, index: usize) -> Result<(), crate::ModelError>;

    /// Which layer a ray meets, honouring ghost and lock.
    ///
    /// A question and not a mutation: the caller activates what it answers by
    /// issuing `SelectLayer`, so the viewport and the stack row reach
    /// [`SceneModel::set_active_layer`] by the same command. Two pickers
    /// writing activation is how the picked layer and the sculpted one came to
    /// disagree.
    ///
    /// `&mut self` because the attributed raycast compiles the document, which
    /// is not a `&self` operation however much it reads like one.
    fn layer_at(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<LayerKey>;

    /// Places a whole layer. One undoable step, however many items it holds.
    fn set_layer_transform(
        &mut self,
        key: LayerKey,
        position: [f32; 3],
        scale: f32,
    ) -> Result<(), crate::ModelError>;

    /// The box a layer's geometry occupies, where the engine can say.
    ///
    /// A fact about the document rather than about any widget: the interface
    /// decides what to do with it — outline a subtool, or size a manipulator so
    /// its arms reach past the form they sit in the middle of. `None` for a
    /// layer holding nothing, and for one whose extent the engine does not
    /// report.
    ///
    /// Provided, so a double that models no geometry answers "cannot say"
    /// rather than inventing a box.
    fn layer_bounds(&self, key: LayerKey) -> Option<([f32; 3], [f32; 3])> {
        let _ = key;
        None
    }

    /// What a layer's field costs, for the consolidation flow.
    fn layer_cost(&self, key: LayerKey) -> Result<LayerCost, crate::ModelError>;

    /// Collapses a layer into one volume. Only ever called after the cost has
    /// been shown and accepted.
    fn consolidate_layer(&mut self, key: LayerKey) -> Result<(), crate::ModelError>;

    /// Carries a mesh the document holds but never sculpts.
    fn add_mesh_layer(&mut self, name: &str) -> Result<LayerKey, crate::ModelError>;
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
            health: None,
            voxel: None,
            sculpt_layers: Vec::new(),
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
                    health: None,
                    voxel: None,
                    sculpt_layers: Vec::new(),
                },
                LayerSummary {
                    key: LayerKey(2),
                    name: "B".into(),
                    representation: Representation::Voxel,
                    visible: true,
                    protection: Protection::default(),
                    intensity: 70,
                    health: None,
                    voxel: None,
                    sculpt_layers: Vec::new(),
                },
            ],
            active: Some(LayerKey(2)),
            soloed: None,
        };
        assert_eq!(scene.active_layer().map(|l| l.name.as_str()), Some("B"));
        assert_eq!(scene.layer(LayerKey(1)).map(|l| l.intensity), Some(100));
        assert!(scene.layer(LayerKey(9)).is_none());
    }
}
