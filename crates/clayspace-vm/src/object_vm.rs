//! Placed shapes, as something the interface can offer, pick up and move.
//!
//! Separate from the sculpting ViewModel for the reason the cage is: a brush
//! asks what happens to the surface under the pointer, and an object asks what
//! happens to a thing standing in the scene. The sculptor's attention is on a
//! handle rather than on the clay.
//!
//! The manipulator's *state* lives here rather than in the document — which
//! mode is in force, and the gesture in flight. The cage keeps its own in the
//! model because a cage is document state; a drag is not, and the document has
//! no business remembering that a pointer is down.

use clayspace_model::{
    CombineSettings, GizmoDrag, GizmoHandle, GizmoMode, GizmoTarget, ObjectId, ObjectModel,
    Representation, SceneObject, Shape, Transform,
};

use crate::command::Command;
use crate::observable::Observable;

pub struct ObjectViewModel {
    model: Box<dyn ObjectModel>,

    /// The placed objects in the active layer, as the list draws them.
    objects: Observable<Vec<SceneObject>>,
    /// Which one is selected, if any.
    selected: Observable<Option<ObjectId>>,
    /// Whether the shape picker is open.
    picking: Observable<bool>,
    /// What the picker is set to, and what it is measured by.
    ///
    /// Held apart from any placed object, so the panel reads the same whether
    /// one is selected or not — the same arrangement the cage's divisions
    /// have.
    shape: Observable<Shape>,
    parameters: Observable<Vec<f32>>,
    /// How the next placement combines with what is under it.
    ///
    /// Its own value rather than the stroke's: `CombineSettings::default` is
    /// Add "because that is what placing a shape means", and a stroke starts
    /// at Relief.
    combine: Observable<CombineSettings>,
    /// What the manipulator is acting on.
    target: Observable<Option<GizmoTarget>>,
    mode: Observable<GizmoMode>,
    /// The mesh layer a placement would sample, when the picker is set to one
    /// rather than to a shape.
    mesh_operand: Observable<Option<clayspace_model::LayerKey>>,
    /// The mesh layers that could be placed, and what they are called.
    mesh_operands: Observable<Vec<(clayspace_model::LayerKey, String)>>,
    /// What the chosen crossing would cost — the conversion panel's own
    /// figures, for the same crossing at the same resolution.
    mesh_cost: Observable<Option<clayspace_model::Cost>>,
    /// Where a placement would land.
    ///
    /// Supplied by the composition root rather than decided here: it is where
    /// the pointer meets the surface, and failing that where the camera is
    /// looking, and a ViewModel can see neither. `None` before anything has
    /// told it, which places at the origin — a document with no camera yet.
    placement: Option<[f32; 3]>,
    /// Where the target would be if the surface had kept up.
    ///
    /// A live boolean is re-evaluated on every frame of a drag, and measured
    /// on the reference scene one frame costs about 21 ms against a 16.7 ms
    /// budget — affordable on that form and not on every form. When a frame
    /// overruns, the drag carries on against the surface as it last stood and
    /// the document is left until the pointer comes up: the object moves at
    /// the speed of the hand, and the clay catches up once.
    ///
    /// The same answer the region-based brushes already give — "they land when
    /// it comes up" — so it is a behaviour this application has rather than a
    /// new kind of lag.
    pending: Option<Transform>,
    /// Whether the last frame overran, so the rest of this gesture is drawn
    /// rather than evaluated.
    settling: bool,
    /// The gesture in flight, and where the target stood when it began.
    ///
    /// The transform is captured at the press so every frame resolves from it,
    /// which is what makes a wandering drag land where it settles rather than
    /// accumulating.
    drag: Option<(GizmoDrag, Transform)>,
    notice: Observable<Option<String>>,
}

impl ObjectViewModel {
    pub fn new(mut model: Box<dyn ObjectModel>) -> Self {
        let objects = model.objects();
        let selected = model.selected_object();
        Self {
            model,
            objects: Observable::new(objects),
            selected: Observable::new(selected),
            picking: Observable::new(false),
            shape: Observable::new(Shape::default()),
            parameters: Observable::new(Shape::default().defaults()),
            combine: Observable::new(CombineSettings::default()),
            target: Observable::new(None),
            mode: Observable::new(GizmoMode::default()),
            mesh_operand: Observable::new(None),
            mesh_operands: Observable::new(Vec::new()),
            mesh_cost: Observable::new(None),
            drag: None,
            pending: None,
            settling: false,
            placement: None,
            notice: Observable::new(None),
        }
    }

    pub fn objects(&self) -> &Observable<Vec<SceneObject>> {
        &self.objects
    }

    pub fn selected(&self) -> &Observable<Option<ObjectId>> {
        &self.selected
    }

    pub fn picking(&self) -> &Observable<bool> {
        &self.picking
    }

    pub fn shape(&self) -> &Observable<Shape> {
        &self.shape
    }

    pub fn parameters(&self) -> &Observable<Vec<f32>> {
        &self.parameters
    }

    pub fn combine(&self) -> &Observable<CombineSettings> {
        &self.combine
    }

    pub fn target(&self) -> &Observable<Option<GizmoTarget>> {
        &self.target
    }

    pub fn mode(&self) -> &Observable<GizmoMode> {
        &self.mode
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    pub fn mesh_operand(&self) -> &Observable<Option<clayspace_model::LayerKey>> {
        &self.mesh_operand
    }

    pub fn mesh_operands(&self) -> &Observable<Vec<(clayspace_model::LayerKey, String)>> {
        &self.mesh_operands
    }

    /// What placing the chosen mesh would cost, where one is chosen.
    ///
    /// Stated *before* it runs, on the same terms a conversion is: a crossing
    /// quantises the vertices and drops the edge loops that made the mesh
    /// worth keeping, and asking for consent to something unstated is not
    /// asking.
    pub fn mesh_cost(&self) -> &Observable<Option<clayspace_model::Cost>> {
        &self.mesh_cost
    }

    /// The cell a mesh operand is sampled at.
    ///
    /// The brick cache's own, so a first crossing lands at the resolution the
    /// rest of the application already works at — the same default the
    /// conversion panel starts from.
    pub const OPERAND_CELL: f32 = 0.02;

    /// Says where a placement would land.
    ///
    /// Called as the pointer moves, so the shape arrives where the sculptor
    /// was looking rather than at the origin — which for a subtracting shape
    /// means inside the form, where it cuts something invisible.
    pub fn set_placement_point(&mut self, at: Option<[f32; 3]>) {
        self.placement = at;
    }

    /// The handles the manipulator offers right now.
    ///
    /// Scale mode offers the centre alone on a target carrying an engine
    /// transform: those take one scale factor and not three, and an axis box
    /// would measure a stretch that could not be applied.
    pub fn handles(&self) -> Vec<GizmoHandle> {
        match self.target.get() {
            Some(GizmoTarget::Curve) | None => GizmoHandle::all_for(*self.mode.get()),
            Some(_) => GizmoHandle::all_for_transform(*self.mode.get()),
        }
    }

    /// Where the manipulator sits, if it is drawn at all.
    ///
    /// The pending transform while a drag is settling, so the widget follows
    /// the hand even when the surface underneath it has not caught up. The
    /// model's own answer otherwise.
    pub fn pivot(&mut self) -> Option<[f32; 3]> {
        if let Some(pending) = self.pending {
            return Some(pending.position);
        }
        let target = (*self.target.get())?;
        self.model.target_transform(target).map(|at| at.position)
    }

    /// Whether the surface is behind the hand, so the viewport can say so.
    pub fn settling(&self) -> bool {
        self.settling
    }

    /// How long a drag frame may take before the rest of the gesture is drawn
    /// rather than evaluated.
    ///
    /// One frame at sixty hertz. Not a budget the specification states for
    /// this — it states one for a *stroke* — but it is the number that decides
    /// whether a hand feels a drag stick, and there is no better one to use.
    const FRAME_BUDGET: std::time::Duration = std::time::Duration::from_millis(16);

    /// The object the options bar is describing, if one is selected.
    pub fn selected_object(&self) -> Option<SceneObject> {
        let id = (*self.selected.get())?;
        self.objects
            .get()
            .iter()
            .find(|object| object.id == id)
            .cloned()
    }

    /// Refreshes from the model, for when something else changed the layer.
    pub fn refresh(&mut self) {
        let objects = self.model.objects();
        self.objects.set_if_changed(objects);
        let selected = self.model.selected_object();
        self.selected.set_if_changed(selected);
        // A selection that history took away leaves the manipulator with
        // nothing to sit on — "a selection outlives the nodes in it", but not
        // the objects.
        if let Some(GizmoTarget::Object(id)) = *self.target.get() {
            if !self.objects.get().iter().any(|object| object.id == id) {
                self.target.set(None);
            }
        }
    }

    pub fn dispatch(&mut self, command: &Command, representation: Representation) {
        match command {
            Command::ToggleShapes => {
                let open = !*self.picking.get();
                self.picking.set(open);
            }
            Command::SetShape(shape) => {
                if self.shape.set_if_changed(*shape) {
                    // A different shape is measured by different things, so
                    // the numbers start again rather than being carried across
                    // and meaning something else.
                    self.parameters.set(shape.defaults());
                }
            }
            Command::SetShapeParameters(values) => {
                let shape = *self.shape.get();
                self.parameters.set_if_changed(shape.sanitised(values));
            }
            Command::PlaceShape => self.place(representation),
            Command::SetMeshOperand(from) => {
                self.mesh_operand.set(*from);
                // The costs, computed now rather than when the button is
                // pressed: a sculptor deciding whether to cross should be
                // reading them while they decide.
                let cost =
                    from.and_then(|from| self.model.mesh_operand_cost(from, Self::OPERAND_CELL));
                self.mesh_cost.set(cost);
            }
            Command::SelectObject(id) => {
                self.model.select_object(*id);
                // The manipulator follows the selection, which is what makes
                // picking a shape in the viewport put a widget on it.
                self.target.set(id.map(GizmoTarget::Object));
                self.refresh();
            }
            Command::SetObjectShape(shape, values) => {
                let Some(id) = *self.selected.get() else {
                    return;
                };
                self.report(|model| model.set_object_shape(id, *shape, values));
                self.refresh();
            }
            Command::SetObjectCombine(combine) => {
                let sanitised = combine.sanitized();
                // The picker's own setting follows, so the next shape placed
                // combines the way the last one was set to.
                self.combine.set(sanitised);
                let Some(id) = *self.selected.get() else {
                    return;
                };
                self.report(|model| model.set_object_combine(id, sanitised));
                self.refresh();
            }
            Command::RemoveObject => {
                let Some(id) = *self.selected.get() else {
                    return;
                };
                self.report(|model| model.remove_object(id));
                self.target.set(None);
                self.refresh();
            }
            Command::SetGizmoTarget(target) => {
                self.target.set(*target);
            }
            Command::SetGizmoMode(mode) => {
                self.mode.set_if_changed(*mode);
            }
            Command::BeginGizmoDrag(handle, anchor, view_axis) => {
                self.begin(*handle, *anchor, *view_axis)
            }
            Command::DragGizmo(to, snap) => self.drag_to(*to, *snap),
            Command::EndGizmoDrag => self.end(),
            _ => {}
        }
    }

    /// Refreshes the mesh layers that could be placed.
    ///
    /// Asked of the model rather than filtered from the layer stack here,
    /// because "could be placed" is more than "is a mesh".
    pub fn refresh_operands(&mut self) {
        let operands = self.model.mesh_operands();
        // A chosen operand that has gone takes its cost with it, rather than
        // leaving a price quoted for a layer nobody can see.
        if let Some(chosen) = *self.mesh_operand.get() {
            if !operands.iter().any(|(key, _)| *key == chosen) {
                self.mesh_operand.set(None);
                self.mesh_cost.set(None);
            }
        }
        self.mesh_operands.set_if_changed(operands);
    }

    fn place(&mut self, representation: Representation) {
        if representation != Representation::Sdf {
            // Said here as well as refused by the model, so the picker can
            // grey the button rather than offering an action that will fail.
            self.notice
                .set(Some(self.model.no_objects_here().to_string()));
            return;
        }
        let (shape, parameters, combine) = (
            *self.shape.get(),
            self.parameters.get().clone(),
            *self.combine.get(),
        );
        let at = self.placement.unwrap_or([0.0; 3]);
        // A mesh operand where one is chosen, and the picked shape otherwise.
        // The crossing's costs were stated when it was chosen; this is the
        // consent.
        let placed = match *self.mesh_operand.get() {
            Some(from) => self
                .model
                .place_mesh_object(from, Self::OPERAND_CELL, at, combine),
            None => self.model.place_object(shape, &parameters, at, combine),
        };
        match placed {
            Ok(id) => {
                self.notice.set_if_changed(None);
                self.target.set(Some(GizmoTarget::Object(id)));
            }
            Err(e) => self.notice.set(Some(e.to_string())),
        }
        self.refresh();
    }

    fn begin(&mut self, handle: GizmoHandle, anchor: [f32; 3], view_axis: [f32; 3]) {
        let Some(target) = *self.target.get() else {
            return;
        };
        let Some(at) = self.model.target_transform(target) else {
            return;
        };
        self.model.begin_target_drag(target);
        self.pending = None;
        self.settling = false;
        self.drag = Some((
            GizmoDrag {
                mode: *self.mode.get(),
                handle,
                pivot: at.position,
                anchor,
                view_axis,
            },
            at,
        ));
    }

    fn drag_to(&mut self, to: [f32; 3], snap: bool) {
        let (Some(target), Some((gesture, started))) = (*self.target.get(), self.drag) else {
            return;
        };
        let moved = gesture.resolve(started, to, snap);
        self.pending = Some(moved);
        if self.settling {
            // The surface is already behind the hand. Moving the object again
            // would only put it further behind, so the widget moves and the
            // clay waits.
            return;
        }

        let began = std::time::Instant::now();
        self.report(|model| model.set_target_transform(target, moved));
        // Measured rather than assumed: whether a live boolean keeps up
        // depends on the form, and a fixed answer would be wrong on half of
        // them.
        if began.elapsed() > Self::FRAME_BUDGET {
            self.settling = true;
        } else {
            self.pending = None;
        }
        self.refresh();
    }

    fn end(&mut self) {
        let target = *self.target.get();
        // What the hand asked for, applied once now that it has stopped.
        if let (Some(target), Some(pending)) = (target, self.pending.take()) {
            self.report(|model| model.set_target_transform(target, pending));
        }
        self.settling = false;
        if self.drag.take().is_some() {
            self.model.end_target_drag();
        }
        self.refresh();
    }

    /// Carries a refusal to the status area rather than dropping it.
    fn report(
        &mut self,
        act: impl FnOnce(&mut dyn ObjectModel) -> Result<(), clayspace_model::ModelError>,
    ) {
        match act(self.model.as_mut()) {
            Ok(()) => {
                self.notice.set_if_changed(None);
            }
            Err(e) => self.notice.set(Some(e.to_string())),
        }
    }
}
