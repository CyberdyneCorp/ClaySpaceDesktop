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
    /// What the layer operations have cost the history, one count per
    /// operation, waiting for the ViewModel that owns Cmd+Z to bank them. See
    /// [`crate::Unbanked`].
    unbanked: crate::Unbanked,
}

impl SceneViewModel {
    pub fn new(model: Box<dyn SceneModel>) -> Self {
        let scene = model.scene();
        Self {
            model,
            scene: Observable::new(scene),
            refusal: Observable::new(None),
            created: 0,
            unbanked: crate::Unbanked::default(),
        }
    }

    pub fn scene(&self) -> &Observable<Scene> {
        &self.scene
    }

    /// Why the last operation was refused. `None` means the last one worked.
    pub fn refusal(&self) -> &Observable<Option<String>> {
        &self.refusal
    }

    /// What the layer operations have cost the history, one count each.
    ///
    /// Taken rather than read, for the reason
    /// [`crate::MaskViewModel::take_unbanked_actions`] is taken: the ViewModel
    /// that owns Cmd+Z banks each count as one action, and a count banked
    /// twice is one undo too many.
    pub fn take_unbanked_actions(&mut self) -> Vec<usize> {
        self.unbanked.take()
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
        match command {
            // Which layer is active, what is drawn and which one is shown
            // alone are ways of *looking* at the scene rather than edits, and
            // the specification keeps them out of the history: a sculptor
            // whose next undo took back a click on a row would have to choose
            // between navigating and working. They do not go through `edit`.
            Command::SelectLayer(key) => {
                let outcome = self.model.set_active_layer(*key);
                self.finish(outcome)
            }
            Command::SetLayerVisible(key, visible) => {
                let outcome = self.model.set_layer_visible(*key, *visible);
                self.finish(outcome)
            }
            Command::SoloLayer(key) => {
                let outcome = self.model.set_solo(*key);
                self.finish(outcome)
            }
            Command::AddLayer(representation) => {
                self.created += 1;
                let name = format!("Camada {}", self.created + 1);
                let representation = *representation;
                self.edit(move |model| model.add_layer(&name, representation).map(|_| ()))
            }
            Command::RemoveLayer(key) => {
                let key = *key;
                self.edit(move |model| model.remove_layer(key))
            }
            Command::OptimizeLayer(key) => {
                let key = *key;
                self.edit(move |model| model.consolidate_layer(key))
            }
            // Dispatched by the composition root rather than here: the outcome
            // is a value the interface shows — what came out, and what the
            // rebuild destroyed on the way — and `dispatch` deals in
            // `Result<(), _>`. See `SceneViewModel::remesh`.
            Command::RemeshLayer(_) => Ok(()),
            // Not this ViewModel's business.
            _ => Ok(()),
        }
    }

    /// Sets a layer's protection.
    pub fn set_protection(
        &mut self,
        key: LayerKey,
        protection: Protection,
    ) -> Result<(), ModelError> {
        self.edit(move |model| model.set_layer_protection(key, protection))
    }

    pub fn rename(&mut self, key: LayerKey, name: &str) -> Result<(), ModelError> {
        let name = name.to_string();
        self.edit(move |model| model.rename_layer(key, &name))
    }

    /// Moves a layer in the stack, which is its evaluation order.
    pub fn reorder(&mut self, key: LayerKey, index: usize) -> Result<(), ModelError> {
        self.edit(move |model| model.move_layer(key, index))
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
        let before = self.model.history_depth();
        let rebuilt = self.model.remesh_layer(key, settings);
        self.unbanked.record(before, self.model.history_depth());
        match rebuilt {
            Ok(outcome) => {
                self.refusal.set_if_changed(None);
                self.refresh();
                Ok(outcome)
            }
            Err(error) => {
                // Announced for the reason `finish` announces: asking a grid
                // layer to re-mesh says the same sentence every time, and a
                // refusal that only registers when the words change is a
                // refusal the second attempt never hears.
                self.refusal.announce(Some(error.to_string()));
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

    /// Applies one layer operation and banks what it cost as one action.
    ///
    /// **Every entry point that changes the layer stack comes through here**,
    /// which is the point of it: an operation added without a history entry is
    /// a defect nobody sees until an undo takes back something else. Measured
    /// on a session — add a subtool, insert a shape, bend it through a cage,
    /// one undo — the undo spent the stroke's count on the layer's entries and
    /// the subtool left the document.
    ///
    /// What it cost is read from the history either side rather than assumed
    /// to be one: consolidating a layer folds a whole list of nodes away, and
    /// an operation the engine recorded nothing for banks nothing. A refusal
    /// is therefore free without having to say so.
    ///
    /// The three commands that change only how the scene is *looked at* —
    /// which layer is active, what is drawn, and which is shown alone — stay
    /// out, as does a hierarchy's stack of levels and passes: those stay
    /// adjustable long after the strokes that filled them, and a sculptor
    /// whose next undo took back a slider rather than the work would have to
    /// choose between the two.
    fn edit(
        &mut self,
        run: impl FnOnce(&mut dyn SceneModel) -> Result<(), ModelError>,
    ) -> Result<(), ModelError> {
        let before = self.model.history_depth();
        let outcome = run(self.model.as_mut());
        self.unbanked.record(before, self.model.history_depth());
        self.finish(outcome)
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
                //
                // Announced rather than set only when the words move. A
                // reader that tells a refusal from a success by watching this
                // channel — the agent door does — would read the second
                // "essa camada é uma grade" as no refusal at all, and answer
                // the same impossible request with success. The line beside
                // the viewport still does not redraw for a repeat: the
                // revision only moves when the sentence does.
                self.refusal.announce(Some(error.to_string()));
                Err(error)
            }
        }
    }
}
