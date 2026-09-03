//! Placed objects and the manipulator, against a double.
//!
//! The rules the panels must obey, checked with no engine behind them: a
//! refusal is stated rather than silent, a drag is resolved from where it
//! began, the manipulator follows the selection, and scale mode offers only
//! what the engine can actually do.

use std::cell::RefCell;
use std::rc::Rc;

use clayspace_model::{
    Combine, CombineSettings, GizmoHandle, GizmoTarget, InsertAs, ItemKind, LayerKey, ModelError,
    ObjectId, ObjectModel, Representation, SceneObject, Shape, Transform,
};
use clayspace_vm::{Command, ObjectViewModel, Picked, Watcher, ITEM_NOT_TRANSFORMABLE};

/// The subtool this fake attributes every hit to.
///
/// `pick_item` answers the layer alongside the kind — one attributed raycast
/// answering both of a press's questions — so a fake has to name one.
const HIT_LAYER: LayerKey = LayerKey(1);

#[derive(Debug, Default)]
struct Calls {
    placed: Vec<(Shape, Vec<f32>)>,
    /// The shapes inserted as subtools of their own, with where each stood.
    inserted: Vec<(Shape, [f32; 3])>,
    /// The subtools copied, in order.
    copied: Vec<LayerKey>,
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
    /// The subtools a copy could be made from. Shared for the same reason
    /// `meshes` is, and it grows as insertions arrive.
    subtools: Rc<RefCell<Vec<(LayerKey, String)>>>,
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
            subtools: Rc::new(RefCell::new(Vec::new())),
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
        scale: [1.0; 3],
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

    fn insert_shape_subtool(
        &mut self,
        shape: Shape,
        parameters: &[f32],
        at: [f32; 3],
        combine: CombineSettings,
    ) -> Result<clayspace_model::Inserted, ModelError> {
        if self.refuse.is_some() {
            return Err(self.refusal());
        }
        self.calls.borrow_mut().inserted.push((shape, at));
        // A subtool of its own, so the object it holds belongs to a layer that
        // was not there before. The double numbers them as the document does.
        let layer = LayerKey(self.subtools.borrow().len() as u64 + 2);
        self.subtools
            .borrow_mut()
            .push((layer, shape.label().to_string()));
        let node = self.objects.len() as u32 + 1;
        let mut object = an_object(node, shape, [0.0; 3]);
        object.id = ObjectId { layer, node };
        object.combine = combine;
        object.parameters = parameters.to_vec();
        self.objects.push(object.clone());
        // The subtool is the selection, not the item inside it — the document
        // answers the same way, and for the reason it records there.
        self.selected = None;
        Ok(clayspace_model::Inserted {
            layer,
            object: Some(object.id),
        })
    }

    fn copy_subtool(
        &mut self,
        from: LayerKey,
        _cell_size: f32,
    ) -> Result<clayspace_model::Inserted, ModelError> {
        if self.refuse.is_some() {
            return Err(self.refusal());
        }
        self.calls.borrow_mut().copied.push(from);
        let layer = LayerKey(self.subtools.borrow().len() as u64 + 2);
        self.subtools.borrow_mut().push((layer, "Cópia".into()));
        // No object row: a copy carries a baked volume rather than one of the
        // offered shapes, which is what the document answers too.
        Ok(clayspace_model::Inserted {
            layer,
            object: None,
        })
    }

    fn copyable_subtools(&mut self) -> Vec<(LayerKey, String)> {
        self.subtools.borrow().clone()
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
        scale: [f32; 3],
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

    fn pick_item(
        &mut self,
        _origin: [f32; 3],
        _direction: [f32; 3],
    ) -> Option<(ItemKind, LayerKey)> {
        self.hit.map(|kind| (kind, HIT_LAYER))
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

/// Puts the picked shape into the *active layer*, as an object.
///
/// Two commands rather than one, because the insert control now offers two
/// destinations and its default is a subtool of its own. These are the rules
/// for the other destination, so they say so rather than relying on whichever
/// one happens to be the default.
fn place(vm: &mut ObjectViewModel) {
    send(vm, Command::SetInsertAs(InsertAs::Object));
    send(vm, Command::InsertShape);
}

// -- the insert control ------------------------------------------------------

/// The specification says inserting as a subtool is the default and placing
/// into the active layer is the other choice. A default that has to be set is
/// not one, so this presses the button with nothing set first.
#[test]
fn a_form_arrives_as_a_subtool_unless_the_other_destination_is_chosen() {
    let (mut vm, calls) = viewmodel();
    assert_eq!(*vm.insert_as().get(), InsertAs::Subtool);

    send(&mut vm, Command::SetShape(Shape::Cylinder));
    send(&mut vm, Command::InsertShape);

    assert_eq!(
        calls.borrow().inserted.len(),
        1,
        "the default destination did not make a subtool"
    );
    assert_eq!(calls.borrow().inserted[0].0, Shape::Cylinder);
    assert!(
        calls.borrow().placed.is_empty(),
        "the form went into the active layer instead"
    );
}

/// The other destination, and the one the old command always took.
#[test]
fn choosing_the_active_subtool_places_an_object_instead() {
    let (mut vm, calls) = viewmodel();
    send(&mut vm, Command::SetInsertAs(InsertAs::Object));
    send(&mut vm, Command::InsertShape);

    assert_eq!(calls.borrow().placed.len(), 1);
    assert!(calls.borrow().inserted.is_empty());
}

/// A form put into the scene to be worked on its own is a whole subtool, so
/// the manipulator addresses the subtool rather than the item inside it — which
/// is what a sculptor's next gesture is for.
#[test]
fn an_inserted_subtool_gets_the_whole_subtool_manipulator() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::InsertShape);
    let target = *vm.target().get();
    assert!(
        matches!(target, Some(GizmoTarget::Layer(_))),
        "the manipulator sat on the item rather than on the subtool: {target:?}"
    );
    assert!(
        vm.selected().get().is_none(),
        "the item inside the new subtool was selected too, which hides the \
         whole-subtool controls the sculptor needs to aim it"
    );
}

/// A grid has no ordered list to put an item in, and that refuses the *object*
/// destination only: the specification says inserting the same primitive as its
/// own subtool remains available.
#[test]
fn a_grid_takes_a_subtool_even_though_it_refuses_an_object() {
    let (mut vm, calls) = viewmodel();
    vm.dispatch(&Command::InsertShape, Representation::Voxel);

    assert_eq!(
        calls.borrow().inserted.len(),
        1,
        "the grid refused a subtool it has no business refusing"
    );
    assert!(
        vm.notice().get().is_none(),
        "an insertion that worked left a refusal on screen: {:?}",
        vm.notice().get()
    );
}

/// A chosen mesh operand is a crossing into an ordered list, so it places
/// whatever the destination chips say — there is no list in a layer that does
/// not exist yet, and a picker that quietly did something else would be lying
/// about what the button does.
#[test]
fn a_chosen_mesh_operand_is_placed_rather_than_made_a_subtool() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let model = FakeObjects::new(calls.clone());
    let meshes = model.meshes.clone();
    meshes.borrow_mut().push((LayerKey(7), "Parafuso".into()));
    let mut vm = ObjectViewModel::new(Box::new(model));

    send(&mut vm, Command::SetMeshOperand(Some(LayerKey(7))));
    send(&mut vm, Command::InsertShape);

    assert_eq!(calls.borrow().mesh_placed, vec![LayerKey(7)]);
    assert!(calls.borrow().inserted.is_empty());
}

/// Copying reaches the model with the layer the control named, and leaves the
/// manipulator on what arrived.
#[test]
fn copying_a_subtool_reaches_the_model_and_selects_the_copy() {
    let (mut vm, calls) = viewmodel();
    send(&mut vm, Command::InsertShape);
    let source = calls.borrow().inserted.len();
    assert_eq!(source, 1);
    let original = match *vm.target().get() {
        Some(GizmoTarget::Layer(key)) => key,
        other => panic!("no subtool to copy: {other:?}"),
    };

    send(&mut vm, Command::CopySubtool(original));
    assert_eq!(calls.borrow().copied, vec![original]);
    assert!(
        matches!(*vm.target().get(), Some(GizmoTarget::Layer(key)) if key != original),
        "the manipulator stayed on the original rather than following the copy"
    );
}

/// A refused copy says what the model said rather than failing silently, and
/// leaves the manipulator where it was.
#[test]
fn a_refused_copy_says_what_the_model_said() {
    const EMPTY: &str = "esta camada não tem extensão para copiar; está vazia";

    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeObjects::new(calls.clone());
    model.refuse = Some(EMPTY);
    let mut vm = ObjectViewModel::new(Box::new(model));

    send(&mut vm, Command::CopySubtool(LayerKey(3)));
    assert_eq!(vm.notice().get().as_deref(), Some(EMPTY));
    assert!(vm.target().get().is_none());
}

/// The copy control reads the model rather than the layer stack, because
/// "could be copied" is more than "is a layer".
#[test]
fn the_copy_control_is_refreshed_from_the_model() {
    let (mut vm, _) = viewmodel();
    assert!(vm.copyable().get().is_empty());

    send(&mut vm, Command::InsertShape);
    vm.refresh_operands();
    assert_eq!(
        vm.copyable().get().len(),
        1,
        "a subtool that just arrived is not offered for copying"
    );
}

// -- the picker -------------------------------------------------------------

#[test]
fn a_shape_places_with_the_numbers_the_picker_has() {
    let (mut vm, calls) = viewmodel();
    send(&mut vm, Command::SetShape(Shape::Cylinder));
    send(&mut vm, Command::SetShapeParameters(vec![0.4, 1.2]));
    place(&mut vm);

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
    vm.dispatch(
        &Command::SetInsertAs(InsertAs::Object),
        Representation::Voxel,
    );
    vm.dispatch(&Command::InsertShape, Representation::Voxel);
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

    place(&mut vm);
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
    place(&mut vm);
    let id = vm.selected().get().expect("selected on arrival");
    assert_eq!(*vm.target().get(), Some(GizmoTarget::Object(id)));
}

// -- the list and the selection ---------------------------------------------

#[test]
fn selecting_an_object_puts_the_manipulator_on_it() {
    let (mut vm, _) = viewmodel();
    place(&mut vm);
    place(&mut vm);
    let first = vm.objects().get()[0].id;

    send(&mut vm, Command::SelectObject(Some(first)));
    assert_eq!(*vm.selected().get(), Some(first));
    assert_eq!(*vm.target().get(), Some(GizmoTarget::Object(first)));
}

#[test]
fn clearing_the_selection_takes_the_manipulator_away() {
    let (mut vm, _) = viewmodel();
    place(&mut vm);
    send(&mut vm, Command::SelectObject(None));
    assert_eq!(*vm.target().get(), None);
}

#[test]
fn removing_the_selected_object_takes_the_manipulator_with_it() {
    let (mut vm, calls) = viewmodel();
    place(&mut vm);
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
    place(&mut vm);
    assert!(
        watcher.take_change(vm.objects()),
        "a placement redraws the list"
    );
}

// -- the options bar --------------------------------------------------------

#[test]
fn an_operation_reaches_the_selected_object() {
    let (mut vm, calls) = viewmodel();
    place(&mut vm);
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
    place(&mut vm);

    let object = vm.selected_object().expect("selected");
    assert_eq!(object.combine.op, Combine::Subtract);
}

/// The seven operations that do nothing at zero cannot be set to zero — the
/// same rule a stroke's distance already follows.
#[test]
fn an_operation_that_needs_a_distance_cannot_be_given_none() {
    let (mut vm, calls) = viewmodel();
    place(&mut vm);
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
    place(&mut vm);
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
    place(&mut vm);

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
    place(&mut vm);
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

/// A placed object stretches per axis, and so does a whole subtool.
///
/// This assertion has been turned around twice and both turns were an engine
/// capability the interface had not caught up with. It first said the centre
/// handle alone, on everything, because nothing had bound
/// `clay_item_set_scale_nonuniform`. It then said a node stretched and a layer
/// did not, because `clay_document_set_layer_transform` took one factor.
/// ClayCore 0.74.0 gave the layer transform a per-axis form (#373), so the
/// remaining half went too, and the manipulator is now one widget with three
/// boxes on it whatever it is pointed at.
///
/// Nothing is dropped: the flag is still a question with a false answer —
/// pointed at nothing, there is no stretch to offer — which is what keeps the
/// three boxes from being drawn over an empty selection.
#[test]
fn a_placed_object_and_a_whole_subtool_both_stretch_per_axis() {
    let (mut vm, _) = viewmodel();
    place(&mut vm);
    assert!(
        vm.per_axis_scale(),
        "a placed object is a node, and a node's transform takes three factors"
    );

    send(
        &mut vm,
        Command::SetGizmoTarget(Some(GizmoTarget::Layer(clayspace_model::LayerKey(1)))),
    );
    assert!(
        vm.per_axis_scale(),
        "a whole subtool is a layer, and a layer's transform has taken three \
         factors since ClayCore 0.74.0"
    );

    send(&mut vm, Command::SetGizmoTarget(None));
    assert!(
        !vm.per_axis_scale(),
        "pointed at nothing, there is no stretch to offer"
    );
}

/// A curve's control points are a point set, which scales per axis the way a
/// cage's do — it carries no engine transform at all.
#[test]
fn a_curve_stretches_per_axis() {
    let (mut vm, _) = viewmodel();
    send(&mut vm, Command::SetGizmoTarget(Some(GizmoTarget::Curve)));
    assert!(vm.per_axis_scale());
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
    place(&mut vm);

    let object = vm.selected_object().expect("placed");
    assert_eq!(object.position, [0.4, 1.2, -0.3]);
}

#[test]
fn a_placement_with_nowhere_stated_lands_at_the_origin() {
    let (mut vm, _) = viewmodel();
    place(&mut vm);
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
    place(&mut vm);

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
    place(&mut vm);
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
        Picked::NotTransformable(HIT_LAYER),
        "a stroke names the subtool it was laid on, which is what the press \
         goes on to activate"
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

/// Reaching for a brush leaves the transform mode.
///
/// The whole-subtool manipulator is a mode: while it is up a press on the clay
/// moves the form and leaves no stroke, and the brush ring is not drawn. So
/// choosing a brush — from the shelf, or with the mask key — puts the widget
/// away, rather than the next press dragging the subtool with nothing on
/// screen having changed.
#[test]
fn choosing_a_brush_puts_the_whole_subtool_manipulator_away() {
    for command in [
        Command::SelectTool(clayspace_model::ToolKind::Padrao),
        Command::ToggleMaskPainting,
    ] {
        let (mut vm, _) = viewmodel();
        send(&mut vm, Command::InsertShape);
        assert!(
            matches!(*vm.target().get(), Some(GizmoTarget::Layer(_))),
            "an inserted subtool should have the manipulator to begin with"
        );

        send(&mut vm, command.clone());
        assert_eq!(
            *vm.target().get(),
            None,
            "{command:?} left the whole-subtool manipulator up"
        );
    }
}

/// And a placed object keeps its own, because choosing a brush is not
/// unselecting what is placed — the object's manipulator follows the
/// selection, and the selection has not changed.
#[test]
fn choosing_a_brush_leaves_a_selected_objects_manipulator_alone() {
    let (mut vm, _) = viewmodel();
    place(&mut vm);
    let target = *vm.target().get();
    assert!(
        matches!(target, Some(GizmoTarget::Object(_))),
        "a placed object should have the manipulator to begin with, got {target:?}"
    );

    send(
        &mut vm,
        Command::SelectTool(clayspace_model::ToolKind::Padrao),
    );
    assert_eq!(
        *vm.target().get(),
        target,
        "choosing a brush took the manipulator off a selected object"
    );
}
