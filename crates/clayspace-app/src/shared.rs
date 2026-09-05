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
use std::time::{Duration, Instant};

use crate::geometry::SyncCost;
use clayspace_engine::ClayDocument;
use clayspace_model::{
    Armature, ArmatureModel, BrushSettings, CombineSettings, CurveJoin, CurveModel, CurveProfile,
    CurveState, DocumentModel, EditOutcome, ExchangeModel, ExportSettings, ExtrudeSettings,
    GestureSample, GizmoHandle, GizmoMode, HistoryState, ImportSettings, LatticeModel,
    LatticeState, LayerCost, LayerKey, MaskModel, MaskOp, MaskState, ModelError, NodeIndex,
    OpenError, Phase, Protection, Representation, Scene, SceneModel, SceneStats, SculptModel,
    SkinSettings, StrokeProfile, ToolKind, Work,
};

/// A handle to the one document, and to what working on it has cost.
#[derive(Clone)]
pub struct SharedDocument {
    document: Rc<RefCell<ClayDocument>>,
    /// What every stroke through this handle has cost the engine.
    ///
    /// Held here rather than at the composition root because this is the one
    /// place every stroke passes through — the ViewModel, the reference
    /// builder and the benchmark harness all reach the document this way — so
    /// a clock here needs no timing code in the layers above and cannot be
    /// bypassed by a path that reaches the document another way.
    profile: Rc<RefCell<StrokeProfile>>,
}

impl SharedDocument {
    pub fn new(document: ClayDocument) -> Self {
        Self {
            document: Rc::new(RefCell::new(document)),
            profile: Rc::new(RefCell::new(StrokeProfile::default())),
        }
    }

    /// What this session has cost, phase by phase.
    ///
    /// Cloned rather than borrowed out: the caller is a report being rendered
    /// while a stroke may still be running, and handing out a `Ref` would make
    /// the report able to panic the sculpting path.
    ///
    /// **The clone is not free** — at four thousand retained samples across
    /// five phases and several tools it is the largest thing in the report —
    /// so anything that only needs to *read* the profile takes
    /// [`Self::with_profile`] instead. This exists for the file writer, which
    /// wants a value it can hold while it renders.
    pub fn profile(&self) -> StrokeProfile {
        self.profile.borrow().clone()
    }

    /// Reads the profile in place, without copying it.
    ///
    /// The borrow lives no longer than the closure, so nothing can hold it
    /// across a stroke.
    pub fn with_profile<T>(&self, read: impl FnOnce(&StrokeProfile) -> T) -> T {
        read(&self.profile.borrow())
    }

    /// Records a phase of the work a stroke caused.
    ///
    /// The re-mesh is measured by the composition root, which is what holds
    /// the surface; it hands its figures here so that both halves of a stroke
    /// end up in one place and the report has one thing to read.
    pub fn record_phase(&self, cause: &str, phase: Phase, took: Duration, work: Work) {
        self.profile.borrow_mut().record(cause, phase, took, work);
    }

    /// Folds what a re-mesh cost into the session's profile.
    ///
    /// These four figures have been measured on every dab of every real stroke
    /// since `SyncCost` existed, and were dropped on the floor by the caller.
    /// They are the half of a stroke that says whether the engine's meshing
    /// call or this application's own work around it is what a sculptor is
    /// waiting for.
    ///
    /// Here rather than at the call site so that the mapping from a cost onto
    /// its phases is written once: a fifth term added to `SyncCost` and
    /// forgotten in one of two places would be a phase silently missing from
    /// every report.
    pub fn record_remesh(&self, cause: &str, cost: SyncCost) {
        let work = Work::meshed(cost.keys, cost.triangles);
        for (phase, took) in [
            (Phase::EngineMesh, cost.engine_mesh_time),
            (Phase::Read, cost.read_time),
            (Phase::Split, cost.split_time),
            (Phase::Upload, cost.upload_time),
        ] {
            self.record_phase(cause, phase, took, work);
        }
    }

    /// Runs something against the document directly.
    ///
    /// For the composition root's own work — meshing, statistics — rather than
    /// for anything a ViewModel does.
    pub fn with<T>(&self, work: impl FnOnce(&mut ClayDocument) -> T) -> T {
        work(&mut self.document.borrow_mut())
    }
}

impl SculptModel for SharedDocument {
    fn active_representation(&self) -> Representation {
        self.document.borrow().active_representation()
    }

    fn active_layer_editable(&self) -> bool {
        self.document.borrow().active_layer_editable()
    }

    // Every provided method of `SculptModel` is forwarded here as well, and
    // the ones below are why. A default exists so a *double* that models one
    // representation need not spell out answers it has none for; this is not a
    // double, it is the one document, and inheriting a default means quietly
    // answering for it. `set_combine` was the one that showed it: the options
    // bar dispatched the command, the ViewModel called the model, the default
    // discarded it, and fourteen combine operations drew the same picture. Any
    // provided method added to the trait belongs here too.
    fn active_layer_carries_geometry(&self) -> bool {
        self.document.borrow().active_layer_carries_geometry()
    }

    fn active_layer_visible(&self) -> bool {
        self.document.borrow().active_layer_visible()
    }

    fn apply_operation(
        &mut self,
        operation: clayspace_model::LayerOperation,
    ) -> Result<EditOutcome, ModelError> {
        self.document.borrow_mut().apply_operation(operation)
    }

    fn symmetry(&self) -> [bool; 3] {
        self.document.borrow().symmetry()
    }

    fn set_symmetry(&mut self, symmetry: [bool; 3]) -> Result<(), ModelError> {
        self.document.borrow_mut().set_symmetry(symmetry)
    }

    fn set_combine(&mut self, combine: CombineSettings) {
        self.document.borrow_mut().set_combine(combine);
    }

    fn combine(&self) -> CombineSettings {
        self.document.borrow().combine()
    }

    fn set_colour(&mut self, colour: clayspace_model::Colour) {
        self.document.borrow_mut().set_colour(colour);
    }

    fn choose_recent_colour(&mut self, index: usize) -> bool {
        self.document.borrow_mut().choose_recent_colour(index)
    }

    fn colour_state(&self) -> clayspace_model::ColourState {
        self.document.borrow().colour_state()
    }

    /// The engine's half of a stroke, timed.
    ///
    /// Nothing of this application's runs inside the call: the engine applies
    /// the stroke and refills the bricks it dirtied, and this is what that
    /// costs. It is the purest ClayCore figure this application is in a
    /// position to hand over, and until now it was not measured at all.
    ///
    /// A refusal records nothing. An error is not a measurement of the engine
    /// doing the work, and folding one in would drag the phase's median
    /// towards whatever refusing costs.
    fn apply_stroke(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        let started = Instant::now();
        let outcome = self
            .document
            .borrow_mut()
            .apply_stroke(tool, brush, samples, symmetry);
        if let Ok(outcome) = &outcome {
            self.record_phase(
                tool.label(),
                Phase::EngineEdit,
                started.elapsed(),
                Work::bricks(outcome.dirty_bricks),
            );
        }
        outcome
    }

    fn pick(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]> {
        self.document.borrow().pick(origin, direction)
    }

    fn undo(&mut self) -> Result<bool, ModelError> {
        self.document.borrow_mut().undo()
    }

    fn redo(&mut self) -> Result<bool, ModelError> {
        self.document.borrow_mut().redo()
    }

    fn history(&self) -> HistoryState {
        self.document.borrow().history()
    }

    fn stats(&self) -> SceneStats {
        self.document.borrow().stats()
    }

    fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        self.document.borrow().bounds()
    }

    fn set_alpha(&mut self, alpha: Option<clayspace_model::Alpha>) {
        self.document.borrow_mut().set_alpha(alpha);
    }

    fn alpha_name(&self) -> Option<String> {
        self.document.borrow().alpha_name()
    }

    // The gesture hooks, which have to be forwarded like everything else.
    //
    // They are *provided* methods on the trait, so leaving them out is not a
    // compile error — it silently substitutes the double's answer, and the
    // ViewModel then believes a document that cannot preview anything. That is
    // how a dragging verb on a mesh came to stack segment on segment through
    // the application while `sculpt_session.rs` showed it replaying cleanly
    // from its anchor: the tests drive `ClayDocument`, and only the
    // application drives this.
    fn begin_gesture(&mut self) {
        self.document.borrow_mut().begin_gesture();
    }

    fn end_gesture(&mut self) {
        self.document.borrow_mut().end_gesture();
    }

    fn open_live_gesture(&mut self, tool: clayspace_model::ToolKind, symmetry: [bool; 3]) -> bool {
        self.document.borrow_mut().open_live_gesture(tool, symmetry)
    }

    fn close_live_gesture(&mut self) -> Result<usize, ModelError> {
        self.document.borrow_mut().close_live_gesture()
    }

    fn discard_live_gesture(&mut self) -> usize {
        self.document.borrow_mut().discard_live_gesture()
    }
}

impl SceneModel for SharedDocument {
    fn scene(&self) -> Scene {
        self.document.borrow().scene()
    }

    fn set_active_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        self.document.borrow_mut().set_active_layer(key)
    }

    fn set_layer_visible(&mut self, key: LayerKey, visible: bool) -> Result<(), ModelError> {
        self.document.borrow_mut().set_layer_visible(key, visible)
    }

    fn set_solo(&mut self, key: Option<LayerKey>) -> Result<(), ModelError> {
        self.document.borrow_mut().set_solo(key)
    }

    fn apply_sculpt_layer_op(
        &mut self,
        op: clayspace_model::SculptLayerOp,
    ) -> Result<(), ModelError> {
        self.document.borrow_mut().apply_sculpt_layer_op(op)
    }

    fn sculpt_layer_cost(&self) -> clayspace_model::SculptLayerCost {
        self.document.borrow().sculpt_layer_cost()
    }

    fn apply_multires_level_op(
        &mut self,
        op: clayspace_model::MultiresLevelOp,
    ) -> Result<(), ModelError> {
        self.document.borrow_mut().apply_multires_level_op(op)
    }

    fn apply_multires_sculpt_layer_op(
        &mut self,
        op: clayspace_model::MultiresSculptLayerOp,
    ) -> Result<(), ModelError> {
        self.document
            .borrow_mut()
            .apply_multires_sculpt_layer_op(op)
    }

    fn subdivision_cost(&self) -> Option<clayspace_model::SubdivisionCost> {
        self.document.borrow().subdivision_cost()
    }

    fn set_layer_protection(
        &mut self,
        key: LayerKey,
        protection: Protection,
    ) -> Result<(), ModelError> {
        self.document
            .borrow_mut()
            .set_layer_protection(key, protection)
    }

    fn rename_layer(&mut self, key: LayerKey, name: &str) -> Result<(), ModelError> {
        self.document.borrow_mut().rename_layer(key, name)
    }

    fn add_layer(
        &mut self,
        name: &str,
        representation: Representation,
    ) -> Result<LayerKey, ModelError> {
        self.document.borrow_mut().add_layer(name, representation)
    }

    fn remove_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        self.document.borrow_mut().remove_layer(key)
    }

    fn move_layer(&mut self, key: LayerKey, index: usize) -> Result<(), ModelError> {
        self.document.borrow_mut().move_layer(key, index)
    }

    fn layer_at(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<LayerKey> {
        self.document.borrow_mut().layer_at(origin, direction)
    }

    fn set_layer_transform(
        &mut self,
        key: LayerKey,
        position: [f32; 3],
        scale: f32,
    ) -> Result<(), ModelError> {
        self.document
            .borrow_mut()
            .set_layer_transform(key, position, scale)
    }

    fn layer_bounds(&self, key: LayerKey) -> Option<([f32; 3], [f32; 3])> {
        self.document.borrow().layer_bounds(key)
    }

    fn layer_cost(&self, key: LayerKey) -> Result<LayerCost, ModelError> {
        self.document.borrow().layer_cost(key)
    }

    fn consolidate_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        self.document.borrow_mut().consolidate_layer(key)
    }

    fn add_mesh_layer(&mut self, name: &str) -> Result<LayerKey, ModelError> {
        self.document.borrow_mut().add_mesh_layer(name)
    }

    fn remesh_layer(
        &mut self,
        key: LayerKey,
        settings: clayspace_model::RemeshSettings,
    ) -> Result<clayspace_model::RemeshOutcome, ModelError> {
        self.document.borrow_mut().remesh_layer(key, settings)
    }
}

impl DocumentModel for SharedDocument {
    fn save(&mut self, path: &std::path::Path) -> Result<(), ModelError> {
        self.document.borrow_mut().save(path)
    }

    fn open(&mut self, path: &std::path::Path) -> Result<(), OpenError> {
        self.document.borrow_mut().open(path)
    }

    fn reset(&mut self) -> Result<(), ModelError> {
        self.document.borrow_mut().reset()
    }
}

impl MaskModel for SharedDocument {
    fn mask_state(&self) -> MaskState {
        self.document.borrow().mask_state()
    }

    fn apply_mask_op(&mut self, op: MaskOp) -> Result<(), ModelError> {
        self.document.borrow_mut().apply_mask_op(op)
    }

    fn extrude_mask(&mut self, settings: ExtrudeSettings) -> Result<(), ModelError> {
        self.document.borrow_mut().extrude_mask(settings)
    }

    fn apply_outline(&mut self, outline: &clayspace_model::MaskOutline) -> Result<(), ModelError> {
        self.document.borrow_mut().apply_outline(outline)
    }
}

impl CurveModel for SharedDocument {
    fn curve(&self) -> CurveState {
        self.document.borrow().curve()
    }
    fn begin_curve(&mut self) {
        self.document.borrow_mut().begin_curve()
    }
    fn add_curve_point(&mut self, at: [f32; 3], radius: f32) -> Result<(), ModelError> {
        self.document.borrow_mut().add_curve_point(at, radius)
    }
    fn select_curve_point(&mut self, index: Option<usize>) {
        self.document.borrow_mut().select_curve_point(index)
    }
    fn toggle_curve_point(&mut self, index: usize) {
        self.document.borrow_mut().toggle_curve_point(index)
    }
    fn drag_curve(&mut self, by: [f32; 3]) -> Result<(), ModelError> {
        self.document.borrow_mut().drag_curve(by)
    }
    fn set_curve_radius(&mut self, radius: f32) -> Result<(), ModelError> {
        self.document.borrow_mut().set_curve_radius(radius)
    }
    fn set_curve_join(&mut self, join: CurveJoin) -> Result<(), ModelError> {
        self.document.borrow_mut().set_curve_join(join)
    }
    fn set_curve_profile(&mut self, profile: CurveProfile) -> Result<(), ModelError> {
        self.document.borrow_mut().set_curve_profile(profile)
    }
    fn remove_curve_points(&mut self) -> Result<(), ModelError> {
        self.document.borrow_mut().remove_curve_points()
    }
    fn apply_curve(&mut self) -> Result<(), ModelError> {
        self.document.borrow_mut().apply_curve()
    }
    fn cancel_curve(&mut self) {
        self.document.borrow_mut().cancel_curve()
    }
}

impl LatticeModel for SharedDocument {
    fn lattice(&self) -> LatticeState {
        self.document.borrow().lattice()
    }

    fn begin_lattice(&mut self, divisions: [i32; 3]) -> Result<(), ModelError> {
        self.document.borrow_mut().begin_lattice(divisions)
    }

    fn select_lattice_point(&mut self, index: Option<usize>) {
        self.document.borrow_mut().select_lattice_point(index)
    }

    fn toggle_lattice_point(&mut self, index: usize) {
        self.document.borrow_mut().toggle_lattice_point(index)
    }

    fn select_lattice_points(&mut self, indices: &[usize]) {
        self.document.borrow_mut().select_lattice_points(indices)
    }

    fn set_gizmo_mode(&mut self, mode: GizmoMode) {
        self.document.borrow_mut().set_gizmo_mode(mode)
    }

    fn begin_gizmo_drag(&mut self, handle: GizmoHandle, anchor: [f32; 3], view_axis: [f32; 3]) {
        self.document
            .borrow_mut()
            .begin_gizmo_drag(handle, anchor, view_axis)
    }

    fn drag_gizmo(&mut self, to: [f32; 3], snap: bool) -> Result<(), ModelError> {
        self.document.borrow_mut().drag_gizmo(to, snap)
    }

    fn end_gizmo_drag(&mut self) {
        self.document.borrow_mut().end_gizmo_drag()
    }

    fn drag_lattice_point(&mut self, to: [f32; 3]) -> Result<(), ModelError> {
        self.document.borrow_mut().drag_lattice_point(to)
    }

    fn apply_lattice(&mut self) -> Result<(), ModelError> {
        self.document.borrow_mut().apply_lattice()
    }

    fn cancel_lattice(&mut self) {
        self.document.borrow_mut().cancel_lattice()
    }
}

impl ArmatureModel for SharedDocument {
    fn armature(&self) -> Option<Armature> {
        self.document.borrow().armature()
    }

    fn begin_armature(&mut self, position: [f32; 3], radius: f32) -> Result<(), ModelError> {
        self.document.borrow_mut().begin_armature(position, radius)
    }

    fn add_zsphere(
        &mut self,
        parent: NodeIndex,
        position: [f32; 3],
        radius: f32,
        mirrored: bool,
    ) -> Result<NodeIndex, ModelError> {
        self.document
            .borrow_mut()
            .add_zsphere(parent, position, radius, mirrored)
    }

    fn move_zsphere(&mut self, index: NodeIndex, delta: [f32; 3]) -> Result<(), ModelError> {
        self.document.borrow_mut().move_zsphere(index, delta)
    }

    fn resize_zsphere(&mut self, index: NodeIndex, radius: f32) -> Result<(), ModelError> {
        self.document.borrow_mut().resize_zsphere(index, radius)
    }

    fn reparent_zsphere(
        &mut self,
        index: NodeIndex,
        new_parent: NodeIndex,
    ) -> Result<(), ModelError> {
        self.document
            .borrow_mut()
            .reparent_zsphere(index, new_parent)
    }

    fn remove_zsphere(&mut self, index: NodeIndex) -> Result<(), ModelError> {
        self.document.borrow_mut().remove_zsphere(index)
    }

    fn insert_zsphere(&mut self, child: NodeIndex) -> Result<NodeIndex, ModelError> {
        self.document.borrow_mut().insert_zsphere(child)
    }

    fn set_zsphere_negative(&mut self, index: NodeIndex, negative: bool) -> Result<(), ModelError> {
        self.document
            .borrow_mut()
            .set_zsphere_negative(index, negative)
    }

    fn set_skin(&mut self, skin: SkinSettings) -> Result<(), ModelError> {
        self.document.borrow_mut().set_skin(skin)
    }

    fn skin(&self) -> SkinSettings {
        self.document.borrow().skin()
    }
}

impl ExchangeModel for SharedDocument {
    fn import_mesh(
        &mut self,
        path: &std::path::Path,
        settings: ImportSettings,
    ) -> Result<(), ModelError> {
        self.document.borrow_mut().import_mesh(path, settings)
    }

    fn export_mesh(
        &mut self,
        path: &std::path::Path,
        settings: ExportSettings,
    ) -> Result<(), ModelError> {
        self.document.borrow_mut().export_mesh(path, settings)
    }

    fn has_mesh_layers(&self) -> bool {
        self.document.borrow().has_mesh_layers()
    }
}

use clayspace_model::ObjectModel;

impl ObjectModel for SharedDocument {
    fn objects(&mut self) -> Vec<clayspace_model::SceneObject> {
        self.document.borrow_mut().objects()
    }

    fn selected_object(&self) -> Option<clayspace_model::ObjectId> {
        self.document.borrow().selected_object()
    }

    fn select_object(&mut self, id: Option<clayspace_model::ObjectId>) {
        self.document.borrow_mut().select_object(id)
    }

    fn place_object(
        &mut self,
        shape: clayspace_model::Shape,
        parameters: &[f32],
        at: [f32; 3],
        combine: clayspace_model::CombineSettings,
    ) -> Result<clayspace_model::ObjectId, ModelError> {
        self.document
            .borrow_mut()
            .place_object(shape, parameters, at, combine)
    }

    // The six below are *provided* methods of `ObjectModel`, and forwarding
    // them is not optional for the same reason `SculptModel::set_combine` is
    // not: a default exists so a double that models one thing need not spell
    // out answers it has none for, and this is not a double — it is the one
    // document, so inheriting a default means quietly answering for it.
    //
    // The last three predate this change and were inherited, which is exactly
    // what it looked like: the shapes picker in the running application listed
    // no imported model to place, because `mesh_operands` answered with the
    // empty default and the placement behind it could only ever have refused.
    // Any provided method added to the trait belongs here too;
    // `tests/shared_forwarding.rs` is what says so out loud.
    fn insert_shape_subtool(
        &mut self,
        shape: clayspace_model::Shape,
        parameters: &[f32],
        at: [f32; 3],
        combine: clayspace_model::CombineSettings,
    ) -> Result<clayspace_model::Inserted, ModelError> {
        self.document
            .borrow_mut()
            .insert_shape_subtool(shape, parameters, at, combine)
    }

    fn copy_subtool(
        &mut self,
        from: LayerKey,
        cell_size: f32,
    ) -> Result<clayspace_model::Inserted, ModelError> {
        self.document.borrow_mut().copy_subtool(from, cell_size)
    }

    fn copyable_subtools(&mut self) -> Vec<(LayerKey, String)> {
        self.document.borrow_mut().copyable_subtools()
    }

    fn boolean_operands(&mut self) -> Vec<(LayerKey, String)> {
        self.document.borrow_mut().boolean_operands()
    }

    fn boolean_cell(&mut self, base: LayerKey, tool: LayerKey) -> Option<f32> {
        self.document.borrow_mut().boolean_cell(base, tool)
    }

    fn boolean_cost(
        &mut self,
        settings: clayspace_model::BooleanSettings,
    ) -> Option<clayspace_model::Cost> {
        self.document.borrow_mut().boolean_cost(settings)
    }

    fn run_boolean(
        &mut self,
        settings: clayspace_model::BooleanSettings,
    ) -> Result<clayspace_model::Inserted, ModelError> {
        self.document.borrow_mut().run_boolean(settings)
    }

    fn mesh_operands(&mut self) -> Vec<(LayerKey, String)> {
        self.document.borrow_mut().mesh_operands()
    }

    fn mesh_operand_cost(
        &mut self,
        from: LayerKey,
        cell_size: f32,
    ) -> Option<clayspace_model::Cost> {
        self.document
            .borrow_mut()
            .mesh_operand_cost(from, cell_size)
    }

    fn place_mesh_object(
        &mut self,
        from: LayerKey,
        cell_size: f32,
        at: [f32; 3],
        combine: clayspace_model::CombineSettings,
    ) -> Result<clayspace_model::ObjectId, ModelError> {
        self.document
            .borrow_mut()
            .place_mesh_object(from, cell_size, at, combine)
    }

    fn set_object_transform(
        &mut self,
        id: clayspace_model::ObjectId,
        position: [f32; 3],
        rotation_axis: [f32; 3],
        rotation_angle: f32,
        scale: [f32; 3],
    ) -> Result<(), ModelError> {
        self.document.borrow_mut().set_object_transform(
            id,
            position,
            rotation_axis,
            rotation_angle,
            scale,
        )
    }

    fn set_object_shape(
        &mut self,
        id: clayspace_model::ObjectId,
        shape: clayspace_model::Shape,
        parameters: &[f32],
    ) -> Result<(), ModelError> {
        self.document
            .borrow_mut()
            .set_object_shape(id, shape, parameters)
    }

    fn set_object_combine(
        &mut self,
        id: clayspace_model::ObjectId,
        combine: clayspace_model::CombineSettings,
    ) -> Result<(), ModelError> {
        self.document.borrow_mut().set_object_combine(id, combine)
    }

    fn remove_object(&mut self, id: clayspace_model::ObjectId) -> Result<(), ModelError> {
        self.document.borrow_mut().remove_object(id)
    }

    fn target_transform(
        &mut self,
        target: clayspace_model::GizmoTarget,
    ) -> Option<clayspace_model::Transform> {
        self.document.borrow_mut().target_transform(target)
    }

    fn set_target_transform(
        &mut self,
        target: clayspace_model::GizmoTarget,
        transform: clayspace_model::Transform,
    ) -> Result<(), ModelError> {
        self.document
            .borrow_mut()
            .set_target_transform(target, transform)
    }

    fn begin_target_drag(&mut self, target: clayspace_model::GizmoTarget) {
        self.document.borrow_mut().begin_target_drag(target)
    }

    fn end_target_drag(&mut self) {
        self.document.borrow_mut().end_target_drag()
    }

    fn pick_object(
        &mut self,
        origin: [f32; 3],
        direction: [f32; 3],
    ) -> Option<clayspace_model::ObjectId> {
        self.document.borrow_mut().pick_object(origin, direction)
    }

    fn pick_item(
        &mut self,
        origin: [f32; 3],
        direction: [f32; 3],
    ) -> Option<(clayspace_model::ItemKind, clayspace_model::LayerKey)> {
        self.document.borrow_mut().pick_item(origin, direction)
    }
}
