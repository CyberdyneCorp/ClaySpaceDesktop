//! The scene and layer ViewModel.
//!
//! Kept apart from the sculpting ViewModel because a layer panel and a brush
//! are different concerns: a test double for one need not implement the other,
//! and the panels can be exercised without an engine at all.

use clayspace_model::{LayerKey, ModelError, Protection, Representation, Scene, SceneModel};

use crate::command::Command;
use crate::observable::Observable;

/// What the scene tree and layer stack read.
pub struct SceneViewModel {
    model: Box<dyn SceneModel>,
    scene: Observable<Scene>,
    /// Why the last operation was refused, when it was.
    refusal: Observable<Option<String>>,
    /// How many new layers have been created, so each gets a distinct name.
    created: usize,
}

impl SceneViewModel {
    pub fn new(model: Box<dyn SceneModel>) -> Self {
        let scene = model.scene();
        Self {
            model,
            scene: Observable::new(scene),
            refusal: Observable::new(None),
            created: 0,
        }
    }

    pub fn scene(&self) -> &Observable<Scene> {
        &self.scene
    }

    /// Why the last operation was refused. `None` means the last one worked.
    pub fn refusal(&self) -> &Observable<Option<String>> {
        &self.refusal
    }

    /// Whether the active layer accepts edits, and why not if it does not.
    ///
    /// The interface shows this before a stroke is attempted rather than
    /// letting the refusal arrive as a surprise mid-gesture.
    pub fn active_layer_refusal(&self) -> Option<String> {
        let layer = self.scene.get().active_layer()?;
        if !layer.visible {
            return Some("esta camada está oculta".to_string());
        }
        layer.protection.refusal().map(ToString::to_string)
    }

    /// Applies a scene command. Commands it does not own are ignored.
    pub fn dispatch(&mut self, command: &Command) -> Result<(), ModelError> {
        let outcome = match command {
            Command::SelectLayer(key) => self.model.set_active_layer(*key),
            Command::SetLayerVisible(key, visible) => self.model.set_layer_visible(*key, *visible),
            Command::AddLayer => {
                self.created += 1;
                let name = format!("Camada {}", self.created + 1);
                self.model.add_layer(&name, Representation::Sdf).map(|_| ())
            }
            Command::RemoveLayer(key) => self.model.remove_layer(*key),
            // Not this ViewModel's business.
            _ => return Ok(()),
        };

        self.finish(outcome)
    }

    /// Sets a layer's protection.
    pub fn set_protection(
        &mut self,
        key: LayerKey,
        protection: Protection,
    ) -> Result<(), ModelError> {
        let outcome = self.model.set_layer_protection(key, protection);
        self.finish(outcome)
    }

    pub fn rename(&mut self, key: LayerKey, name: &str) -> Result<(), ModelError> {
        let outcome = self.model.rename_layer(key, name);
        self.finish(outcome)
    }

    /// Moves a layer in the stack, which is its evaluation order.
    pub fn reorder(&mut self, key: LayerKey, index: usize) -> Result<(), ModelError> {
        let outcome = self.model.move_layer(key, index);
        self.finish(outcome)
    }

    /// Selects whatever a ray meets.
    ///
    /// A ray that meets nothing clears the selection rather than leaving it on
    /// the previous target.
    pub fn select_at(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<LayerKey> {
        let selected = self.model.select_at(origin, direction);
        self.refresh();
        selected
    }

    /// Re-reads the scene from the model.
    ///
    /// Called after anything that could have changed it, including edits the
    /// sculpting ViewModel made.
    pub fn refresh(&mut self) {
        let scene = self.model.scene();
        self.scene.set_if_changed(scene);
    }

    fn finish(&mut self, outcome: Result<(), ModelError>) -> Result<(), ModelError> {
        match outcome {
            Ok(()) => {
                self.refusal.set_if_changed(None);
                self.refresh();
                Ok(())
            }
            Err(error) => {
                // A refusal is shown rather than swallowed: the specification
                // asks for a stated reason, not a silent no-op.
                self.refusal.set_if_changed(Some(error.to_string()));
                Err(error)
            }
        }
    }
}
