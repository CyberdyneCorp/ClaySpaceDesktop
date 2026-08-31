//! Scene and layer behaviour, against a double.
//!
//! The rules the panels must obey — a refusal is stated rather than silent, a
//! ray that meets nothing clears the selection, a document keeps a layer to
//! sculpt on — checked with no engine behind them.

use std::cell::RefCell;
use std::rc::Rc;

use clayspace_model::{
    LayerCost, LayerKey, LayerSummary, ModelError, Protection, Representation, Scene, SceneModel,
    SceneNode,
};
use clayspace_vm::{Command, SceneViewModel, Watcher};

#[derive(Debug, Default)]
struct Calls {
    activated: Vec<LayerKey>,
    removed: Vec<LayerKey>,
    added: Vec<String>,
    moved: Vec<(LayerKey, usize)>,
}

struct FakeScene {
    calls: Rc<RefCell<Calls>>,
    layers: Vec<LayerSummary>,
    active: Option<LayerKey>,
    /// What the next raycast reports.
    hit: Option<LayerKey>,
    /// Set to refuse the next operation, as a locked layer or a last layer
    /// would.
    refuse: Option<&'static str>,
    /// Which layer is shown alone, and what the rest were before it was.
    solo: Option<(LayerKey, Vec<(LayerKey, bool)>)>,
}

impl FakeScene {
    fn new(calls: Rc<RefCell<Calls>>) -> Self {
        let layer = |id: u64, name: &str| LayerSummary {
            key: LayerKey(id),
            name: name.into(),
            representation: Representation::Sdf,
            visible: true,
            protection: Protection::default(),
            intensity: 100,
            health: None,
            voxel: None,
            sculpt_layers: Vec::new(),
        };
        Self {
            calls,
            layers: vec![layer(1, "Base"), layer(2, "Detalhe")],
            active: Some(LayerKey(1)),
            hit: Some(LayerKey(2)),
            refuse: None,
            solo: None,
        }
    }

    fn index(&self, key: LayerKey) -> Option<usize> {
        self.layers.iter().position(|layer| layer.key == key)
    }

    fn guard(&self) -> Result<(), ModelError> {
        match self.refuse {
            Some(why) => Err(ModelError::engine(why)),
            None => Ok(()),
        }
    }
}

impl SceneModel for FakeScene {
    fn scene(&self) -> Scene {
        Scene {
            nodes: self
                .layers
                .iter()
                .map(|layer| SceneNode {
                    key: layer.key,
                    name: layer.name.clone(),
                    depth: 0,
                    visible: layer.visible,
                    expandable: false,
                })
                .collect(),
            layers: self.layers.clone(),
            active: self.active,
            soloed: self.solo.as_ref().map(|(key, _)| *key),
        }
    }

    fn set_active_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        self.guard()?;
        self.calls.borrow_mut().activated.push(key);
        self.active = Some(key);
        Ok(())
    }

    fn set_layer_visible(&mut self, key: LayerKey, visible: bool) -> Result<(), ModelError> {
        self.guard()?;
        if let Some(index) = self.index(key) {
            self.layers[index].visible = visible;
        }
        Ok(())
    }

    fn set_solo(&mut self, key: Option<LayerKey>) -> Result<(), ModelError> {
        self.guard()?;
        let was: Vec<(LayerKey, bool)> = match self.solo.take() {
            Some((_, was)) => was,
            None => self
                .layers
                .iter()
                .map(|layer| (layer.key, layer.visible))
                .collect(),
        };
        match key {
            Some(alone) => {
                for layer in &mut self.layers {
                    layer.visible = layer.key == alone;
                }
                self.solo = Some((alone, was));
            }
            None => {
                for (key, visible) in was {
                    if let Some(index) = self.index(key) {
                        self.layers[index].visible = visible;
                    }
                }
            }
        }
        Ok(())
    }

    fn set_layer_protection(
        &mut self,
        key: LayerKey,
        protection: Protection,
    ) -> Result<(), ModelError> {
        self.guard()?;
        if let Some(index) = self.index(key) {
            self.layers[index].protection = protection;
        }
        Ok(())
    }

    fn rename_layer(&mut self, key: LayerKey, name: &str) -> Result<(), ModelError> {
        self.guard()?;
        if let Some(index) = self.index(key) {
            self.layers[index].name = name.to_string();
        }
        Ok(())
    }

    fn add_layer(
        &mut self,
        name: &str,
        representation: Representation,
    ) -> Result<LayerKey, ModelError> {
        self.guard()?;
        self.calls.borrow_mut().added.push(name.to_string());
        let key = LayerKey(self.layers.len() as u64 + 10);
        self.layers.push(LayerSummary {
            key,
            name: name.to_string(),
            representation,
            visible: true,
            protection: Protection::default(),
            intensity: 100,
            health: None,
            voxel: None,
            sculpt_layers: Vec::new(),
        });
        self.active = Some(key);
        Ok(key)
    }

    fn remove_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        self.guard()?;
        if self.layers.len() == 1 {
            return Err(ModelError::engine("a document keeps at least one layer"));
        }
        self.calls.borrow_mut().removed.push(key);
        self.layers.retain(|layer| layer.key != key);
        Ok(())
    }

    fn move_layer(&mut self, key: LayerKey, index: usize) -> Result<(), ModelError> {
        self.guard()?;
        self.calls.borrow_mut().moved.push((key, index));
        if let Some(from) = self.index(key) {
            let layer = self.layers.remove(from);
            self.layers.insert(index.min(self.layers.len()), layer);
        }
        Ok(())
    }

    fn layer_at(&mut self, _origin: [f32; 3], _direction: [f32; 3]) -> Option<LayerKey> {
        self.hit
    }

    fn set_layer_transform(
        &mut self,
        _key: LayerKey,
        _position: [f32; 3],
        _scale: f32,
    ) -> Result<(), ModelError> {
        self.guard()
    }

    fn layer_cost(&self, _key: LayerKey) -> Result<LayerCost, ModelError> {
        Ok(LayerCost {
            items: 12,
            safe_step_scale: 0.3,
            advises_consolidation: true,
            estimated_bytes: 4 * 1024 * 1024,
            consolidated: false,
        })
    }

    fn consolidate_layer(&mut self, _key: LayerKey) -> Result<(), ModelError> {
        self.guard()
    }

    fn add_mesh_layer(&mut self, name: &str) -> Result<LayerKey, ModelError> {
        self.guard()?;
        self.add_layer(name, Representation::Mesh)
    }
}

fn fixture() -> (SceneViewModel, Rc<RefCell<Calls>>) {
    let calls = Rc::new(RefCell::new(Calls::default()));
    (
        SceneViewModel::new(Box::new(FakeScene::new(calls.clone()))),
        calls,
    )
}

fn fixture_with(configure: impl FnOnce(&mut FakeScene)) -> (SceneViewModel, Rc<RefCell<Calls>>) {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeScene::new(calls.clone());
    configure(&mut model);
    (SceneViewModel::new(Box::new(model)), calls)
}

// -- selection ---------------------------------------------------------------

#[test]
fn selecting_a_layer_makes_it_active() {
    let (mut vm, calls) = fixture();
    vm.dispatch(&Command::SelectLayer(LayerKey(2)))
        .expect("select");

    assert_eq!(calls.borrow().activated, vec![LayerKey(2)]);
    assert_eq!(vm.scene().get().active, Some(LayerKey(2)));
}

#[test]
fn a_click_names_the_layer_the_ray_met() {
    let (mut vm, calls) = fixture();
    let hit = vm.layer_at([0.0, 0.0, -1.0], [0.0, 0.0, 1.0]);
    assert_eq!(hit, Some(LayerKey(2)));
    assert_eq!(
        vm.scene().get().active,
        Some(LayerKey(1)),
        "the pick activated on its own; activation belongs to SelectLayer"
    );
    assert!(
        calls.borrow().activated.is_empty(),
        "a pick that answers must not also mutate"
    );
}

#[test]
fn the_pick_and_the_stack_reach_the_same_activation() {
    let (mut vm, calls) = fixture();
    let hit = vm
        .layer_at([0.0, 0.0, -1.0], [0.0, 0.0, 1.0])
        .expect("a hit");
    vm.dispatch(&Command::SelectLayer(hit)).expect("activate");

    assert_eq!(calls.borrow().activated, vec![LayerKey(2)]);
    assert_eq!(vm.scene().get().active, Some(LayerKey(2)));
}

/// This is only half of "clicking empty space clears the selection": it says
/// what the ray answered, and the *clearing* is the object selection's, which
/// this ViewModel does not hold. The rule that does it is
/// `clayspace_app::input::selection_after`, tested beside `activation` — it
/// used to sit in the event loop where nothing could reach it.
#[test]
fn a_click_on_nothing_names_no_layer() {
    let (mut vm, _) = fixture_with(|model| model.hit = None);
    assert_eq!(
        vm.layer_at([0.0, 0.0, -1.0], [0.0, 0.0, 1.0]),
        None,
        "a ray that met nothing named a layer anyway"
    );
}

// -- layer operations --------------------------------------------------------

#[test]
fn a_new_layer_becomes_active_and_is_named_distinctly() {
    let (mut vm, calls) = fixture();
    vm.dispatch(&Command::AddLayer(Representation::Sdf))
        .expect("add");
    vm.dispatch(&Command::AddLayer(Representation::Sdf))
        .expect("add again");

    let names = calls.borrow().added.clone();
    assert_eq!(names.len(), 2);
    assert_ne!(
        names[0], names[1],
        "two new layers were given the same name"
    );
    assert_eq!(vm.scene().get().layers.len(), 4);
}

/// The specification: "the user adds a layer and chooses voxel — the new layer
/// is voxel-backed and the voxel tools are available on it without a
/// conversion step".
///
/// Every `Command::AddLayer` in the whole test tree passed `Representation::Sdf`
/// and the one test over the command inspected only the names, so nothing at
/// any level said what representation a chosen one produced.
#[test]
fn a_voxel_subtool_is_created_directly() {
    let (mut vm, _) = fixture();
    vm.dispatch(&Command::AddLayer(Representation::Voxel))
        .expect("add a grid");

    let scene = vm.scene().get();
    let arrived = scene
        .active_layer()
        .expect("the new layer is the active one");
    assert_eq!(
        arrived.representation,
        Representation::Voxel,
        "the choice was carried as far as the command and dropped after it"
    );
}

/// And the other half of the requirement: "the user adds a layer without
/// engaging the choice — an SDF layer is created, as before".
#[test]
fn the_default_stays_what_it_was() {
    let (mut vm, _) = fixture();
    vm.dispatch(&Command::AddLayer(Representation::Sdf))
        .expect("add");

    let scene = vm.scene().get();
    assert_eq!(
        scene
            .active_layer()
            .expect("the new layer is the active one")
            .representation,
        Representation::Sdf
    );
}

#[test]
fn hiding_a_layer_is_reflected_in_the_stack() {
    let (mut vm, _) = fixture();
    vm.dispatch(&Command::SetLayerVisible(LayerKey(1), false))
        .expect("hide");
    let scene = vm.scene().get();
    let layer = scene.layer(LayerKey(1)).expect("layer");
    assert!(!layer.visible);
    assert!(
        !layer.is_editable(),
        "a hidden layer must not accept edits: an edit nobody can see is not an edit"
    );
}

#[test]
fn removing_the_last_layer_is_refused_with_a_reason() {
    let (mut vm, _) = fixture_with(|model| {
        model.layers.truncate(1);
    });

    let error = vm
        .dispatch(&Command::RemoveLayer(LayerKey(1)))
        .expect_err("a document keeps a layer to sculpt on");
    assert!(error.to_string().contains("layer"), "{error}");
    assert!(
        vm.refusal().get().is_some(),
        "the refusal must be shown, not swallowed"
    );
    assert_eq!(vm.scene().get().layers.len(), 1);
}

#[test]
fn a_refusal_clears_once_something_succeeds() {
    let (mut vm, _) = fixture_with(|model| model.refuse = Some("locked"));
    let _ = vm.dispatch(&Command::SelectLayer(LayerKey(2)));
    assert!(vm.refusal().get().is_some());

    // The next operation succeeds, so the stale reason must go.
    vm.refresh();
    let (mut vm, _) = fixture();
    vm.dispatch(&Command::SelectLayer(LayerKey(2)))
        .expect("select");
    assert!(vm.refusal().get().is_none());
}

#[test]
fn reordering_moves_the_layer_and_reports_where() {
    let (mut vm, calls) = fixture();
    vm.reorder(LayerKey(1), 1).expect("reorder");
    assert_eq!(calls.borrow().moved, vec![(LayerKey(1), 1)]);
    assert_eq!(
        vm.scene().get().layers[1].key,
        LayerKey(1),
        "the layer did not move in the stack"
    );
}

#[test]
fn renaming_shows_immediately() {
    let (mut vm, _) = fixture();
    vm.rename(LayerKey(1), "Forma_principal").expect("rename");
    assert_eq!(
        vm.scene().get().layer(LayerKey(1)).map(|l| l.name.clone()),
        Some("Forma_principal".to_string())
    );
}

#[test]
fn soloing_shows_one_layer_and_releasing_brings_the_rest_back() {
    let (mut vm, calls) = fixture();
    vm.dispatch(&Command::SetLayerVisible(LayerKey(2), false))
        .expect("hide one by hand");
    let before: Vec<bool> = vm
        .scene()
        .get()
        .layers
        .iter()
        .map(|layer| layer.visible)
        .collect();

    vm.dispatch(&Command::SoloLayer(Some(LayerKey(2))))
        .expect("solo");
    let scene = vm.scene().get();
    assert_eq!(scene.soloed, Some(LayerKey(2)));
    assert!(scene.is_soloed(LayerKey(2)) && !scene.is_soloed(LayerKey(1)));
    assert_eq!(
        scene.layers.iter().map(|l| l.visible).collect::<Vec<_>>(),
        vec![false, true],
    );

    vm.dispatch(&Command::SoloLayer(None)).expect("release");
    let scene = vm.scene().get();
    assert_eq!(scene.soloed, None);
    assert_eq!(
        scene.layers.iter().map(|l| l.visible).collect::<Vec<_>>(),
        before,
        "releasing the solo did not put the sculptor's own pattern back"
    );
    assert!(
        calls.borrow().activated.is_empty(),
        "solo changed which layer is active; it is a viewing convenience"
    );
}

#[test]
fn a_refused_solo_is_stated_rather_than_swallowed() {
    let (mut vm, _) = fixture_with(|model| model.refuse = Some("bloqueada"));
    vm.dispatch(&Command::SoloLayer(Some(LayerKey(1))))
        .expect_err("the model refused");
    assert!(
        vm.refusal().get().is_some(),
        "the reason a solo was refused must reach the interface"
    );
}

// -- protection --------------------------------------------------------------

#[test]
fn the_active_layer_explains_why_it_refuses_edits() {
    let (mut vm, _) = fixture();
    assert!(
        vm.active_layer_refusal().is_none(),
        "an ordinary layer accepts edits"
    );

    vm.set_protection(
        LayerKey(1),
        Protection {
            ghost: false,
            locked: true,
        },
    )
    .expect("lock");
    let refusal = vm.active_layer_refusal();
    assert!(
        refusal.is_some_and(|r| r.contains("bloqueada")),
        "a locked layer must say so before a stroke is attempted"
    );
}

#[test]
fn a_hidden_active_layer_says_so_rather_than_reporting_a_lock() {
    let (mut vm, _) = fixture();
    vm.dispatch(&Command::SetLayerVisible(LayerKey(1), false))
        .expect("hide");
    let refusal = vm.active_layer_refusal().expect("a hidden layer refuses");
    assert!(
        refusal.contains("oculta"),
        "the reason must be the real one: {refusal}"
    );
}

// -- observability -----------------------------------------------------------

#[test]
fn reading_the_scene_does_not_schedule_a_redraw() {
    let (vm, _) = fixture();
    let mut watcher = Watcher::new();
    watcher.accept(vm.scene());

    for _ in 0..100 {
        let _ = vm.scene().get();
        let _ = vm.active_layer_refusal();
    }
    assert!(!watcher.take_change(vm.scene()));
}

#[test]
fn a_change_that_changes_nothing_reports_nothing() {
    let (mut vm, _) = fixture();
    vm.dispatch(&Command::SelectLayer(LayerKey(2)))
        .expect("select");
    let mut watcher = Watcher::new();
    watcher.accept(vm.scene());

    // The same selection again.
    vm.dispatch(&Command::SelectLayer(LayerKey(2)))
        .expect("select again");
    assert!(
        !watcher.take_change(vm.scene()),
        "reselecting the active layer scheduled a redraw"
    );
}

#[test]
fn commands_this_viewmodel_does_not_own_are_ignored() {
    let (mut vm, calls) = fixture();
    let mut watcher = Watcher::new();
    watcher.accept(vm.scene());

    vm.dispatch(&Command::Undo)
        .expect("undo is not a scene command");
    vm.dispatch(&Command::FrameAll).expect("nor is framing");

    assert!(calls.borrow().activated.is_empty());
    assert!(!watcher.take_change(vm.scene()));
}
