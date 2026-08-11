//! One document, two ViewModels.
//!
//! `SculptModel` and `SceneModel` are separate interfaces on purpose — a brush
//! and a layer panel are different concerns, and a double for one need not
//! implement the other. Both take ownership of what they are given, though,
//! and there is only one document.
//!
//! So the composition root shares it. This is the only place that does: no
//! layer below has any idea the document is shared, and the two ViewModels see
//! the interfaces they were written against.

use std::cell::RefCell;
use std::rc::Rc;

use clayspace_engine::ClayDocument;
use clayspace_model::{
    BrushSettings, DocumentModel, EditOutcome, GestureSample, HistoryState, LayerCost, LayerKey,
    ModelError, OpenError, Protection, Representation, Scene, SceneModel, SceneStats, SculptModel,
    ToolKind,
};

/// A handle to the one document.
#[derive(Clone)]
pub struct SharedDocument(Rc<RefCell<ClayDocument>>);

impl SharedDocument {
    pub fn new(document: ClayDocument) -> Self {
        Self(Rc::new(RefCell::new(document)))
    }

    /// Runs something against the document directly.
    ///
    /// For the composition root's own work — meshing, statistics — rather than
    /// for anything a ViewModel does.
    pub fn with<T>(&self, work: impl FnOnce(&mut ClayDocument) -> T) -> T {
        work(&mut self.0.borrow_mut())
    }
}

impl SculptModel for SharedDocument {
    fn active_representation(&self) -> Representation {
        self.0.borrow().active_representation()
    }

    fn active_layer_editable(&self) -> bool {
        self.0.borrow().active_layer_editable()
    }

    fn apply_stroke(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        self.0
            .borrow_mut()
            .apply_stroke(tool, brush, samples, symmetry)
    }

    fn pick(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]> {
        self.0.borrow().pick(origin, direction)
    }

    fn undo(&mut self) -> Result<bool, ModelError> {
        self.0.borrow_mut().undo()
    }

    fn redo(&mut self) -> Result<bool, ModelError> {
        self.0.borrow_mut().redo()
    }

    fn history(&self) -> HistoryState {
        self.0.borrow().history()
    }

    fn stats(&self) -> SceneStats {
        self.0.borrow().stats()
    }

    fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        self.0.borrow().bounds()
    }
}

impl SceneModel for SharedDocument {
    fn scene(&self) -> Scene {
        self.0.borrow().scene()
    }

    fn set_active_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        self.0.borrow_mut().set_active_layer(key)
    }

    fn set_layer_visible(&mut self, key: LayerKey, visible: bool) -> Result<(), ModelError> {
        self.0.borrow_mut().set_layer_visible(key, visible)
    }

    fn set_layer_protection(
        &mut self,
        key: LayerKey,
        protection: Protection,
    ) -> Result<(), ModelError> {
        self.0.borrow_mut().set_layer_protection(key, protection)
    }

    fn rename_layer(&mut self, key: LayerKey, name: &str) -> Result<(), ModelError> {
        self.0.borrow_mut().rename_layer(key, name)
    }

    fn add_layer(
        &mut self,
        name: &str,
        representation: Representation,
    ) -> Result<LayerKey, ModelError> {
        self.0.borrow_mut().add_layer(name, representation)
    }

    fn remove_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        self.0.borrow_mut().remove_layer(key)
    }

    fn move_layer(&mut self, key: LayerKey, index: usize) -> Result<(), ModelError> {
        self.0.borrow_mut().move_layer(key, index)
    }

    fn select_at(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<LayerKey> {
        self.0.borrow_mut().select_at(origin, direction)
    }

    fn set_layer_transform(
        &mut self,
        key: LayerKey,
        position: [f32; 3],
        scale: f32,
    ) -> Result<(), ModelError> {
        self.0.borrow_mut().set_layer_transform(key, position, scale)
    }

    fn layer_cost(&self, key: LayerKey) -> Result<LayerCost, ModelError> {
        self.0.borrow().layer_cost(key)
    }

    fn consolidate_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        self.0.borrow_mut().consolidate_layer(key)
    }

    fn add_mesh_layer(&mut self, name: &str) -> Result<LayerKey, ModelError> {
        self.0.borrow_mut().add_mesh_layer(name)
    }
}

impl DocumentModel for SharedDocument {
    fn save(&mut self, path: &std::path::Path) -> Result<(), ModelError> {
        self.0.borrow_mut().save(path)
    }

    fn open(&mut self, path: &std::path::Path) -> Result<(), OpenError> {
        self.0.borrow_mut().open(path)
    }

    fn reset(&mut self) -> Result<(), ModelError> {
        self.0.borrow_mut().reset()
    }
}
