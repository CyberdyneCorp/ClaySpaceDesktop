//! Placed objects and the manipulator, against a double.
//!
//! The rules the panels must obey, checked with no engine behind them: a
//! refusal is stated rather than silent, a drag is resolved from where it
//! began, the manipulator follows the selection, and scale mode offers only
//! what the engine can actually do.

use std::cell::RefCell;
use std::rc::Rc;

use clayspace_model::{
    Combine, CombineSettings, GizmoHandle, GizmoMode, GizmoTarget, ItemKind, LayerKey, ModelError,
    ObjectId, ObjectModel, Representation, SceneObject, Shape, Transform,
};
use clayspace_vm::{Command, ObjectViewModel, Picked, Watcher, ITEM_NOT_TRANSFORMABLE};

#[derive(Debug, Default)]
struct Calls {
    placed: Vec<(Shape, Vec<f32>)>,
    mesh_placed: Vec<LayerKey>,
    transforms: Vec<Transform>,
    removed: Vec<ObjectId>,
    combines: Vec<CombineSettings>,
    drags_begun: usize,
    drags_ended: usize,
}

struct FakeObjects {
    calls: Rc<RefCell<Calls>>,
    objects: Vec<SceneObject>,
    /// The mesh layers this document offers as operands.
    ///
    /// Shared so a test can take one away from under the ViewModel, which is
    /// the case worth checking: a price quoted for a layer nobody can see.
    meshes: Rc<RefCell<Vec<(LayerKey, String)>>>,
    selected: Option<ObjectId>,
    /// Set to refuse the next edit, as a locked layer would.
    refuse: Option<&'static str>,
    /// Set to make every transform overrun the frame, as a heavy form does.
    slow: bool,
    /// What a ray meets. `None` is empty space, which is the answer the
    /// interface must not confuse with an item it cannot move.
    hit: Option<ItemKind>,
}

impl FakeObjects {
    fn new(calls: Rc<RefCell<Calls>>) -> Self {
        Self {
            calls,
            objects: Vec::new(),
            meshes: Rc::new(RefCell::new(Vec::new())),
            selected: None,
            refuse: None,
            slow: false,
            hit: None,
        }
    }

    fn refusal(&self) -> ModelError {
        ModelError::Engine(self.refuse.unwrap_or("recusado").to_string())
    }
}

fn an_object(node: u32, shape: Shape, at: [f32; 3]) -> SceneObject {
    SceneObject {
        id: ObjectId {
            layer: LayerKey(1),
            node,
        },
        source: clayspace_model::ObjectSource::Shape(shape),
        parameters: shape.defaults(),
        combine: CombineSettings::default(),
        position: at,
        rotation_axis: [0.0, 1.0, 0.0],
        rotation_angle: 0.0,
        scale: 1.0,
    }
}

impl ObjectModel for FakeObjects {
    fn objects(&mut self) -> Vec<SceneObject> {
        self.objects.clone()
    }

    fn selected_object(&self) -> Option<ObjectId> {
        self.selected
    }

    fn select_object(&mut self, id: Option<ObjectId>) {
        self.selected = id;
    }

    fn place_object(
        &mut self,
        shape: Shape,
        parameters: &[f32],
        at: [f32; 3],
        combine: CombineSettings,
    ) -> Result<ObjectId, ModelError> {
        if self.refuse.is_some() {
            return Err(self.refusal());
        }
        self.calls
            .borrow_mut()
            .placed
            .push((shape, parameters.to_vec()));
        let node = self.objects.len() as u32 + 1;
        let mut object = an_object(node, shape, at);
        object.combine = combine;
        object.parameters = parameters.to_vec();
        self.objects.push(object.clone());
        self.selected = Some(object.id);
        Ok(object.id)
    }

    fn mesh_operands(&mut self) -> Vec<(LayerKey, String)> {
        self.meshes.borrow().clone()
    }

    fn mesh_operand_cost(
        &mut self,
        from: LayerKey,
        cell_size: f32,
    ) -> Option<clayspace_model::Cost> {
        self.meshes.borrow().iter().find(|(key, _)| *key == from)?;
        Some(clayspace_model::Cost::of(
            clayspace_model::Direction::MeshToSdf,
            cell_size,
            [1.0; 3],
        ))
    }

    fn place_mesh_object(
        &mut self,
        from: LayerKey,
        _cell_size: f32,
        at: [f32; 3],
        combine: CombineSettings,
    ) -> Result<ObjectId, ModelError> {
        if self.refuse.is_some() {
            return Err(self.refusal());
        }
        self.calls.borrow_mut().mesh_placed.push(from);
        let node = self.objects.len() as u32 + 1;
        let mut object = an_object(node, Shape::Box, at);
        object.source = clayspace_model::ObjectSource::Mesh {
            from,
            name: "Parafuso".into(),
        };
        object.parameters = Vec::new();
        object.combine = combine;
        self.objects.push(object.clone());
        self.selected = Some(object.id);
        Ok(object.id)
    }

    fn set_object_transform(
        &mut self,
        id: ObjectId,
        position: [f32; 3],
        rotation_axis: [f32; 3],
        rotation_angle: f32,
        scale: f32,
    ) -> Result<(), ModelError> {
        if self.refuse.is_some() {
            return Err(self.refusal());
        }
        let Some(object) = self.objects.iter_mut().find(|object| object.id == id) else {
            return Err(self.refusal());
        };
        object.position = position;
        object.rotation_axis = rotation_axis;
        object.rotation_angle = rotation_angle;
        object.scale = scale;
        Ok(())
    }

    fn set_object_shape(
        &mut self,
        id: ObjectId,
        shape: Shape,
        parameters: &[f32],
    ) -> Result<(), ModelError> {
        if self.refuse.is_some() {
            return Err(self.refusal());
        }
        let Some(object) = self.objects.iter_mut().find(|object| object.id == id) else {
            return Err(self.refusal());
        };
        object.source = clayspace_model::ObjectSource::Shape(shape);
        object.parameters = parameters.to_vec();
        Ok(())
    }

    fn set_object_combine(
        &mut self,
        id: ObjectId,
        combine: CombineSettings,
    ) -> Result<(), ModelError> {
        if self.refuse.is_some() {
            return Err(self.refusal());
        }
        self.calls.borrow_mut().combines.push(combine);
        if let Some(object) = self.objects.iter_mut().find(|object| object.id == id) {
            object.combine = combine;
        }
        Ok(())
    }

    fn remove_object(&mut self, id: ObjectId) -> Result<(), ModelError> {
        if self.refuse.is_some() {
            return Err(self.refusal());
        }
        self.calls.borrow_mut().removed.push(id);
        self.objects.retain(|object| object.id != id);
        if self.selected == Some(id) {
            self.selected = None;
        }
        Ok(())
    }

    fn target_transform(&mut self, target: GizmoTarget) -> Option<Transform> {
        match target {
            GizmoTarget::Object(id) => {
                let object = self.objects.iter().find(|object| object.id == id)?;
                Some(Transform {
                    position: object.position,
                    rotation_axis: object.rotation_axis,
                    rotation_angle: object.rotation_angle,
                    scale: object.scale,
                })
            }
            GizmoTarget::Layer(_) => Some(Transform::default()),
            GizmoTarget::Curve => None,
        }
    }

    fn set_target_transform(
        &mut self,
        target: GizmoTarget,
        transform: Transform,
    ) -> Result<(), ModelError> {
        if self.refuse.is_some() {
            return Err(self.refusal());
        }
        if self.slow {
            // Longer than a frame, which is what the ViewModel measures.
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        self.calls.borrow_mut().transforms.push(transform);
        match target {
            GizmoTarget::Object(id) => self.set_object_transform(
                id,
                transform.position,
                transform.rotation_axis,
                transform.rotation_angle,
                transform.scale,
            ),
            _ => Ok(()),
        }
    }

    fn pick_item(&mut self, _origin: [f32; 3], _direction: [f32; 3]) -> Option<ItemKind> {
        self.hit
    }

    fn pick_object(&mut self, _origin: [f32; 3], _direction: [f32; 3]) -> Option<ObjectId> {
        if self.hit != Some(ItemKind::Object) {
            return None;
        }
        self.objects.first().map(|object| object.id)
    }

    fn begin_target_drag(&mut self, _target: GizmoTarget) {
        self.calls.borrow_mut().drags_begun += 1;
    }

    fn end_target_drag(&mut self) {
        self.calls.borrow_mut().drags_ended += 1;
    }
}

fn viewmodel() -> (ObjectViewModel, Rc<RefCell<Calls>>) {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let vm = ObjectViewModel::new(Box::new(FakeObjects::new(calls.clone())));
    (vm, calls)
}

fn send(vm: &mut ObjectViewModel, command: Command) {
    vm.dispatch(&command, Representation::Sdf);
}

// -- the picker -------------------------------------------------------------

#[test]
fn a_shape_places_with_the_numbers_the_picker_has() {
    let (mut vm, calls) = viewmodel();
    send(&mut vm, Command::SetShape(Shape::Cylinder));
    send(&mut vm, Command::SetShapeParameters(vec![0.4, 1.2]));
    send(&mut vm, Command::PlaceShape);

    let placed = &calls.borrow().placed;
    assert_eq!(placed.len(), 1);
    assert_eq!(placed[0].0, Shape::Cylinder);
    assert_eq!(placed[0].1, vec![0.4, 1.2]);
}

/// A different shape is measured by different things, so its numbers start
/// again rather than being carried across and meaning something else.
#[test]
fn changing_the_shape_starts_its_own_numbers() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::SetShape(Shape::Cylinder));
    send(&mut vm, Command::SetShapeParameters(vec![0.4, 1.2]));
    send(&mut vm, Command::SetShape(Shape::Torus));

    assert_eq!(*vm.parameters().get(), Shape::Torus.defaults());
}

#[test]
fn a_size_out_of_range_is_brought_back_in() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::SetShape(Shape::Sphere));
    send(&mut vm, Command::SetShapeParameters(vec![-5.0]));
    assert!(
        vm.parameters().get()[0] > 0.0,
        "a size of nothing is not a small shape"
    );
}

#[test]
fn placing_on_a_grid_says_why_it_cannot() {
    let (mut vm, calls) = viewmodel();
    vm.dispatch(&Command::PlaceShape, Representation::Voxel);
    assert!(
        vm.notice().get().is_some(),
        "a refusal must be stated rather than silent"
    );
    assert!(calls.borrow().placed.is_empty());
}

/// The other half of the same rule, and the one the ViewModel cannot decide
/// for itself: a layer this build has no reason to think is closed, which the
/// model refuses anyway.
#[test]
fn a_placement_the_model_refuses_says_what_it_said() {
    const LOCKED: &str = "esta camada está bloqueada";

    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeObjects::new(calls.clone());
    model.refuse = Some(LOCKED);
    let mut vm = ObjectViewModel::new(Box::new(model));

    send(&mut vm, Command::PlaceShape);
    assert_eq!(
        vm.notice().get().as_deref(),
        Some(LOCKED),
        "the model's own words were dropped"
    );
    assert!(calls.borrow().placed.is_empty());
    assert!(
        vm.target().get().is_none(),
        "the manipulator went to an object that was never placed"
    );
}

#[test]
fn a_placed_shape_is_selected_and_the_manipulator_follows() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    let id = vm.selected().get().expect("selected on arrival");
    assert_eq!(*vm.target().get(), Some(GizmoTarget::Object(id)));
}

// -- the list and the selection ---------------------------------------------

#[test]
fn selecting_an_object_puts_the_manipulator_on_it() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    send(&mut vm, Command::PlaceShape);
    let first = vm.objects().get()[0].id;

    send(&mut vm, Command::SelectObject(Some(first)));
    assert_eq!(*vm.selected().get(), Some(first));
    assert_eq!(*vm.target().get(), Some(GizmoTarget::Object(first)));
}

#[test]
fn clearing_the_selection_takes_the_manipulator_away() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    send(&mut vm, Command::SelectObject(None));
    assert_eq!(*vm.target().get(), None);
}

#[test]
fn removing_the_selected_object_takes_the_manipulator_with_it() {
    let (mut vm, calls) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    send(&mut vm, Command::RemoveObject);

    assert_eq!(calls.borrow().removed.len(), 1);
    assert_eq!(*vm.target().get(), None);
    assert!(vm.objects().get().is_empty());
}

#[test]
fn the_list_is_watchable() {
    let (mut vm, _) = viewmodel();
    let mut watcher = Watcher::new();
    assert!(watcher.take_change(vm.objects()));
    assert!(!watcher.take_change(vm.objects()));
    send(&mut vm, Command::PlaceShape);
    assert!(
        watcher.take_change(vm.objects()),
        "a placement redraws the list"
    );
}

// -- the options bar --------------------------------------------------------

#[test]
fn an_operation_reaches_the_selected_object() {
    let (mut vm, calls) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    let subtract = CombineSettings {
        op: Combine::Subtract,
        ..CombineSettings::default()
    };
    send(&mut vm, Command::SetObjectCombine(subtract));

    assert_eq!(calls.borrow().combines.len(), 1);
    assert_eq!(calls.borrow().combines[0].op, Combine::Subtract);
    let object = vm.selected_object().expect("still selected");
    assert_eq!(object.combine.op, Combine::Subtract);
}

/// The choice sticks, so the next shape placed combines the way the last one
/// was set to — which is what a sculptor cutting six holes expects.
#[test]
fn the_operation_is_remembered_for_the_next_placement() {
    let (mut vm, _) = viewmodel();
    let subtract = CombineSettings {
        op: Combine::Subtract,
        ..CombineSettings::default()
    };
    send(&mut vm, Command::SetObjectCombine(subtract));
    send(&mut vm, Command::PlaceShape);

    let object = vm.selected_object().expect("selected");
    assert_eq!(object.combine.op, Combine::Subtract);
}

/// The seven operations that do nothing at zero cannot be set to zero — the
/// same rule a stroke's distance already follows.
#[test]
fn an_operation_that_needs_a_distance_cannot_be_given_none() {
    let (mut vm, calls) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    send(
        &mut vm,
        Command::SetObjectCombine(CombineSettings {
            op: Combine::Groove,
            radius: 0.0,
            ..CombineSettings::default()
        }),
    );
    let reached = calls.borrow().combines[0];
    assert!(
        reached.radius > 0.0,
        "a groove of no width is not a hard join, it is no operation"
    );
}

#[test]
fn exchanging_a_shape_keeps_the_object() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::SetShape(Shape::Box));
    send(&mut vm, Command::PlaceShape);
    let id = vm.selected().get().expect("selected");

    send(
        &mut vm,
        Command::SetObjectShape(Shape::Cylinder, Shape::Cylinder.defaults()),
    );
    let object = vm.selected_object().expect("still selected");
    assert_eq!(object.id, id, "it is the same object");
    assert_eq!(object.source.shape(), Some(Shape::Cylinder));
}

// -- the manipulator --------------------------------------------------------

#[test]
fn a_drag_is_one_gesture_however_many_frames_it_takes() {
    let (mut vm, calls) = viewmodel();
    send(&mut vm, Command::PlaceShape);

    send(
        &mut vm,
        Command::BeginGizmoDrag(GizmoHandle::Centre, [0.0; 3], [0.0, 0.0, 1.0]),
    );
    for step in 1..=5 {
        send(
            &mut vm,
            Command::DragGizmo([step as f32 * 0.1, 0.0, 0.0], false),
        );
    }
    send(&mut vm, Command::EndGizmoDrag);

    let calls = calls.borrow();
    assert_eq!(calls.drags_begun, 1, "one gesture opened");
    assert_eq!(calls.drags_ended, 1, "and one closed");
    assert_eq!(calls.transforms.len(), 5, "five frames of it");
}

/// Resolved from where the gesture began every frame, so a hand that wandered
/// lands where it settles.
#[test]
fn a_wandering_drag_lands_where_it_ends() {
    let (mut vm, calls) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    send(
        &mut vm,
        Command::BeginGizmoDrag(GizmoHandle::Centre, [0.0; 3], [0.0, 0.0, 1.0]),
    );
    send(&mut vm, Command::DragGizmo([5.0, 5.0, 5.0], false));
    send(&mut vm, Command::DragGizmo([1.0, 0.0, 0.0], false));
    send(&mut vm, Command::EndGizmoDrag);

    let last = *calls.borrow().transforms.last().expect("a frame");
    assert_eq!(
        last.position,
        [1.0, 0.0, 0.0],
        "the intermediate point left a trace"
    );
}

#[test]
fn a_drag_with_nothing_selected_does_nothing() {
    let (mut vm, calls) = viewmodel();
    send(
        &mut vm,
        Command::BeginGizmoDrag(GizmoHandle::Centre, [0.0; 3], [0.0, 0.0, 1.0]),
    );
    send(&mut vm, Command::DragGizmo([1.0, 0.0, 0.0], false));
    send(&mut vm, Command::EndGizmoDrag);

    let calls = calls.borrow();
    assert_eq!(calls.drags_begun, 0);
    assert!(calls.transforms.is_empty());
}

#[test]
fn scale_mode_offers_no_axis_handles_on_an_object() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    send(&mut vm, Command::SetGizmoMode(GizmoMode::Scale));
    assert_eq!(vm.handles(), vec![GizmoHandle::Centre]);
}

/// A cage keeps all three, because it scales its own control points rather
/// than carrying an engine transform.
#[test]
fn scale_mode_still_offers_axis_handles_on_a_cage() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::SetGizmoTarget(Some(GizmoTarget::Curve)));
    send(&mut vm, Command::SetGizmoMode(GizmoMode::Scale));
    assert_eq!(vm.handles().len(), 4);
}

#[test]
fn move_and_rotate_offer_what_they_always_did() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    for mode in [GizmoMode::Move, GizmoMode::Rotate] {
        send(&mut vm, Command::SetGizmoMode(mode));
        assert_eq!(vm.handles(), GizmoHandle::all_for(mode));
    }
}

/// A drag the model refuses has to reach the status area. Leaving the object
/// where it was and saying nothing reads as the manipulator having broken,
/// and the sculptor's next move is to drag it again.
#[test]
fn a_refused_drag_is_stated_rather_than_silent() {
    const LOCKED: &str = "esta camada está bloqueada";

    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeObjects::new(calls.clone());
    model.objects.push(an_object(1, Shape::Box, [0.0; 3]));
    model.selected = Some(model.objects[0].id);
    let id = model.objects[0].id;
    // The layer is locked from here on: every edit the drag asks for comes
    // back refused.
    model.refuse = Some(LOCKED);
    let mut vm = ObjectViewModel::new(Box::new(model));

    send(
        &mut vm,
        Command::SetGizmoTarget(Some(GizmoTarget::Object(id))),
    );
    send(
        &mut vm,
        Command::BeginGizmoDrag(GizmoHandle::Centre, [0.0; 3], [0.0, 0.0, 1.0]),
    );
    // Taking hold of a handle asks the model for nothing, so there is nothing
    // to refuse yet and nothing to say.
    assert!(vm.notice().get().is_none(), "nothing has gone wrong yet");

    send(&mut vm, Command::DragGizmo([1.0, 0.0, 0.0], false));
    assert_eq!(
        vm.notice().get().as_deref(),
        Some(LOCKED),
        "the refusal was dropped instead of stated"
    );

    // And letting go does not quietly clear it: the object never moved, so
    // what was last said about it is still true.
    send(&mut vm, Command::EndGizmoDrag);
    assert_eq!(vm.notice().get().as_deref(), Some(LOCKED));
    assert_eq!(
        vm.selected_object().expect("still there").position,
        [0.0; 3],
        "a refused drag moved the object anyway"
    );
}

/// A shape lands where the sculptor was looking, not at the origin.
///
/// Placing at the origin puts a subtracting shape *inside* the form, where it
/// cuts something nobody can see — which reads as the tool having done
/// nothing.
#[test]
fn a_shape_lands_where_the_pointer_is() {
    let (mut vm, _) = viewmodel();
    vm.set_placement_point(Some([0.4, 1.2, -0.3]));
    send(&mut vm, Command::PlaceShape);

    let object = vm.selected_object().expect("placed");
    assert_eq!(object.position, [0.4, 1.2, -0.3]);
}

#[test]
fn a_placement_with_nowhere_stated_lands_at_the_origin() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    let object = vm.selected_object().expect("placed");
    assert_eq!(object.position, [0.0; 3]);
}

// -- a custom object as an operand ------------------------------------------

/// Choosing a mesh states what the crossing costs and changes nothing yet.
/// Asking for consent to something unstated is not asking.
#[test]
fn choosing_a_mesh_states_its_cost_before_anything_happens() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeObjects::new(calls.clone());
    model.meshes = Rc::new(RefCell::new(vec![(LayerKey(4), "Parafuso".into())]));
    let mut vm = ObjectViewModel::new(Box::new(model));
    vm.refresh_operands();

    assert_eq!(vm.mesh_operands().get().len(), 1);
    assert!(
        vm.mesh_cost().get().is_none(),
        "nothing chosen, nothing quoted"
    );

    send(&mut vm, Command::SetMeshOperand(Some(LayerKey(4))));
    assert!(
        vm.mesh_cost().get().is_some(),
        "choosing a mesh should state what the crossing costs"
    );
    assert!(
        calls.borrow().placed.is_empty() && calls.borrow().mesh_placed.is_empty(),
        "choosing states the cost; it does not run the crossing"
    );
}

#[test]
fn placing_with_a_mesh_chosen_places_the_mesh() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeObjects::new(calls.clone());
    model.meshes = Rc::new(RefCell::new(vec![(LayerKey(4), "Parafuso".into())]));
    let mut vm = ObjectViewModel::new(Box::new(model));
    vm.refresh_operands();

    send(&mut vm, Command::SetMeshOperand(Some(LayerKey(4))));
    send(&mut vm, Command::PlaceShape);

    assert_eq!(calls.borrow().mesh_placed, vec![LayerKey(4)]);
    assert!(
        calls.borrow().placed.is_empty(),
        "the picked shape should not have been placed as well"
    );
}

/// Declining leaves no layer, no boolean and no change.
#[test]
fn declining_leaves_everything_alone() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeObjects::new(calls.clone());
    model.meshes = Rc::new(RefCell::new(vec![(LayerKey(4), "Parafuso".into())]));
    let mut vm = ObjectViewModel::new(Box::new(model));
    vm.refresh_operands();

    send(&mut vm, Command::SetMeshOperand(Some(LayerKey(4))));
    send(&mut vm, Command::SetMeshOperand(None));

    assert!(vm.mesh_cost().get().is_none(), "the quote goes with it");
    assert!(calls.borrow().mesh_placed.is_empty());
    assert!(vm.objects().get().is_empty(), "and nothing was placed");
}

/// A chosen operand whose layer has gone takes its price with it, rather than
/// leaving one quoted for a layer nobody can see.
#[test]
fn an_operand_that_disappears_stops_being_quoted() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeObjects::new(calls.clone());
    let meshes = Rc::new(RefCell::new(vec![(LayerKey(4), "Parafuso".into())]));
    model.meshes = meshes.clone();
    let mut vm = ObjectViewModel::new(Box::new(model));
    vm.refresh_operands();
    send(&mut vm, Command::SetMeshOperand(Some(LayerKey(4))));
    assert!(vm.mesh_cost().get().is_some(), "quoted while it exists");

    // Removed from under the ViewModel, as deleting the layer would.
    meshes.borrow_mut().clear();
    vm.refresh_operands();

    assert_eq!(*vm.mesh_operand().get(), None, "the choice went with it");
    assert!(vm.mesh_cost().get().is_none(), "and so did the price");
}

// -- when the surface cannot keep up ----------------------------------------

/// A live boolean is re-evaluated on every frame of a drag. Where that
/// overruns the frame, the object goes on moving at the speed of the hand and
/// the clay catches up once, when the pointer comes up — the same answer the
/// region-based brushes already give.
#[test]
fn a_drag_that_overruns_settles_when_the_pointer_comes_up() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeObjects::new(calls.clone());
    model.objects.push(an_object(1, Shape::Box, [0.0; 3]));
    let id = model.objects[0].id;
    model.selected = Some(id);
    // Every frame overruns.
    model.slow = true;
    let mut vm = ObjectViewModel::new(Box::new(model));

    send(&mut vm, Command::SelectObject(Some(id)));
    send(
        &mut vm,
        Command::BeginGizmoDrag(GizmoHandle::Centre, [0.0; 3], [0.0, 0.0, 1.0]),
    );
    for step in 1..=6 {
        send(
            &mut vm,
            Command::DragGizmo([step as f32 * 0.2, 0.0, 0.0], false),
        );
    }

    assert!(vm.settling(), "the surface should be behind the hand");
    // The widget follows the hand even so, which is what makes the drag usable
    // rather than sticky.
    assert_eq!(vm.pivot(), Some([1.2, 0.0, 0.0]));
    // One frame reached the document, not six.
    assert_eq!(
        calls.borrow().transforms.len(),
        1,
        "the drag kept re-evaluating a surface that could not keep up"
    );

    send(&mut vm, Command::EndGizmoDrag);
    assert!(!vm.settling(), "and it catches up when the hand stops");
    let last = *calls.borrow().transforms.last().expect("a frame");
    assert_eq!(
        last.position,
        [1.2, 0.0, 0.0],
        "the clay should end where the hand ended"
    );
    assert_eq!(calls.borrow().transforms.len(), 2, "once, not six times");
}

/// And a drag the surface keeps up with is not throttled: every frame reaches
/// the document, because that is what makes a live boolean live.
#[test]
fn a_drag_that_keeps_up_is_not_throttled() {
    let (mut vm, calls) = viewmodel();
    send(&mut vm, Command::PlaceShape);
    send(
        &mut vm,
        Command::BeginGizmoDrag(GizmoHandle::Centre, [0.0; 3], [0.0, 0.0, 1.0]),
    );
    for step in 1..=4 {
        send(
            &mut vm,
            Command::DragGizmo([step as f32 * 0.2, 0.0, 0.0], false),
        );
    }
    send(&mut vm, Command::EndGizmoDrag);

    assert!(!vm.settling());
    assert_eq!(
        calls.borrow().transforms.len(),
        4,
        "a surface that keeps up should be evaluated every frame"
    );
}

// -- what a press met -------------------------------------------------------

/// A press that meets a stroke says so.
///
/// The requirement is that the manipulator does not act on a stroke and that
/// "an attempt to select one for transformation SHALL say so rather than doing
/// nothing". `pick_object` answers `None` for a stroke and for empty space
/// alike, so the ViewModel asks the question that can tell them apart.
#[test]
fn a_press_on_a_stroke_says_why_it_carries_no_manipulator() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeObjects::new(calls.clone());
    model.hit = Some(ItemKind::Stroke);
    let mut vm = ObjectViewModel::new(Box::new(model));

    assert_eq!(
        vm.pick_at([0.0, 4.0, 0.0], [0.0, -1.0, 0.0]),
        Picked::NotTransformable
    );
    assert_eq!(
        vm.notice().get().as_deref(),
        Some(ITEM_NOT_TRANSFORMABLE),
        "a stroke that cannot be picked up must be said out loud"
    );
    assert_eq!(
        *vm.target().get(),
        None,
        "and no manipulator appears over it"
    );
}

/// A press that meets a placed object selects it and says nothing: the
/// manipulator arriving is the answer.
#[test]
fn a_press_on_an_object_leaves_no_notice() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeObjects::new(calls.clone());
    model.objects.push(an_object(1, Shape::Box, [0.0; 3]));
    let id = model.objects[0].id;
    model.hit = Some(ItemKind::Object);
    let mut vm = ObjectViewModel::new(Box::new(model));

    assert_eq!(
        vm.pick_at([0.0, 4.0, 0.0], [0.0, -1.0, 0.0]),
        Picked::Object(id)
    );
    assert!(
        vm.notice().get().is_none(),
        "nothing was refused, so nothing is said"
    );
}

/// And a press that meets nothing is neither. It belongs to whatever is behind
/// the objects — orbiting, or the brush.
#[test]
fn a_press_on_nothing_says_nothing() {
    let (mut vm, _) = viewmodel();
    assert_eq!(
        vm.pick_at([9.0, 9.0, 9.0], [0.0, 1.0, 0.0]),
        Picked::Nothing
    );
    assert!(vm.notice().get().is_none());
}

/// Every refusal reaches the notice, not just the ones the panel greys out.
///
/// Re-shaping, re-combining and removing all went through `report`, whose
/// whole purpose is to carry a refusal to the status area — and for as long as
/// nothing read the notice, all three failed in silence.
#[test]
fn a_refused_re_op_or_removal_is_stated_rather_than_silent() {
    for command in [
        Command::SetObjectShape(Shape::Sphere, vec![0.3]),
        Command::SetObjectCombine(CombineSettings::default()),
        Command::RemoveObject,
    ] {
        let calls = Rc::new(RefCell::new(Calls::default()));
        let mut model = FakeObjects::new(calls.clone());
        model.objects.push(an_object(1, Shape::Box, [0.0; 3]));
        model.selected = Some(model.objects[0].id);
        model.refuse = Some("esta camada está bloqueada");
        let mut vm = ObjectViewModel::new(Box::new(model));
        vm.refresh();

        send(&mut vm, command.clone());
        assert_eq!(
            vm.notice().get().as_deref(),
            Some("esta camada está bloqueada"),
            "a refused object edit must name what is wrong"
        );
    }
}
