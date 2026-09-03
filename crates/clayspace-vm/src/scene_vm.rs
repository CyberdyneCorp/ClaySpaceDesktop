//! The scene and layer ViewModel.
//!
//! Kept apart from the sculpting ViewModel because a layer panel and a brush
//! are different concerns: a test double for one need not implement the other,
//! and the panels can be exercised without an engine at all.

use clayspace_model::{LayerKey, ModelError, Protection, Scene, SceneModel};

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
            Command::SoloLayer(key) => self.model.set_solo(*key),
            Command::AddLayer(representation) => {
                self.created += 1;
                let name = format!("Camada {}", self.created + 1);
                self.model.add_layer(&name, *representation).map(|_| ())
            }
            Command::RemoveLayer(key) => self.model.remove_layer(*key),
            Command::OptimizeLayer(key) => self.model.consolidate_layer(*key),
            // Dispatched by the composition root rather than here: the outcome
            // is a value the interface shows — what came out, and what the
            // rebuild destroyed on the way — and `dispatch` deals in
            // `Result<(), _>`. See `SceneViewModel::remesh`.
            Command::RemeshLayer(_) => return Ok(()),
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

    /// Which layer a ray meets, if it meets one.
    ///
    /// Answers rather than acts. The composition root turns the answer into
    /// `Command::SelectLayer`, so a viewport click and a stack row click reach
    /// `set_active_layer` by the same command instead of by two paths that can
    /// come to disagree.
    pub fn layer_at(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<LayerKey> {
        self.model.layer_at(origin, direction)
    }

    /// The box a layer's geometry occupies, where the model can say.
    ///
    /// For the composition root, which sizes the whole-subtool manipulator from
    /// it: a widget on a form's middle has to reach past that form to be seen.
    pub fn layer_bounds(&self, key: LayerKey) -> Option<([f32; 3], [f32; 3])> {
        self.model.layer_bounds(key)
    }

    /// Rebuilds a mesh layer's topology, and answers what that cost.
    ///
    /// Apart from [`SceneViewModel::dispatch`] because it has something to
    /// say: a rebuild always destroys the topology it replaces, and the
    /// outcome is the only account of what went with it. A command returning
    /// `Ok(())` would leave the interface to guess.
    pub fn remesh(
        &mut self,
        key: LayerKey,
        settings: clayspace_model::RemeshSettings,
    ) -> Result<clayspace_model::RemeshOutcome, ModelError> {
        match self.model.remesh_layer(key, settings) {
            Ok(outcome) => {
                self.refusal.set_if_changed(None);
                self.refresh();
                Ok(outcome)
            }
            Err(error) => {
                self.refusal.set_if_changed(Some(error.to_string()));
                Err(error)
            }
        }
    }

    /// Moves the active hierarchy's levels, or changes how many it has.
    ///
    /// Apart from [`SceneViewModel::dispatch`] for the reason
    /// [`SceneViewModel::remesh`] is apart from it: the refusal is the answer.
    /// A level is priced against a budget and refused over it rather than
    /// attempted, so "that level peaks at 3 GB, past the 2 GB budget" is the
    /// whole of what a sculptor gets back from asking — and a command that
    /// swallowed it would leave a button that does nothing and says nothing.
    ///
    /// Through [`SceneViewModel::finish`], so the refusal is cleared by the
    /// next scene command that works — the line beside the viewport belongs to
    /// the last thing that was asked rather than to the last thing that
    /// failed. It is here rather than in `dispatch` because the composition
    /// root has to know whether the picture changed: three of the four
    /// operations move a number and only one of them redraws anything.
    pub fn apply_level_op(
        &mut self,
        op: clayspace_model::MultiresLevelOp,
    ) -> Result<(), ModelError> {
        let outcome = self.model.apply_multires_level_op(op);
        self.finish(outcome)
    }

    /// Acts on the active hierarchy's stack of passes.
    ///
    /// Beside [`SceneViewModel::apply_level_op`] rather than inside
    /// [`SceneViewModel::dispatch`], and for the same two reasons. The refusal
    /// is half the operation — a locked pass, a slider moved while a stroke is
    /// still open, a merge with nothing under it — and the composition root
    /// has to know whether the picture moved, which for this stack is *three
    /// operations out of eleven*: an additive stack commutes, so a reorder
    /// moves no vertex, and a rename, a lock and a change of which pass is
    /// active move nothing either.
    ///
    /// Through [`SceneViewModel::finish`], so the reason lands on the same
    /// line every other scene refusal lands on and is cleared by the next
    /// command that works.
    pub fn apply_sculpt_layer_op(
        &mut self,
        op: clayspace_model::MultiresSculptLayerOp,
    ) -> Result<(), ModelError> {
        let outcome = self.model.apply_multires_sculpt_layer_op(op);
        self.finish(outcome)
    }

    /// What subdividing the active hierarchy once more would cost.
    ///
    /// Asked whenever the panel is drawn rather than held, because it moves
    /// with every level added or removed and the engine answers it in
    /// microseconds. `None` where the active layer is not a hierarchy.
    pub fn subdivision_cost(&self) -> Option<clayspace_model::SubdivisionCost> {
        self.model.subdivision_cost()
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
                // asks for a stated reason, not a silent no-op. It reaches the
                // screen through the options bar's one "why that did not
                // happen" line — which it did not until the hierarchy's levels
                // needed it, so a rebuild refused for an unusable resolution
                // set this and nothing read it.
                self.refusal.set_if_changed(Some(error.to_string()));
                Err(error)
            }
        }
    }
}
