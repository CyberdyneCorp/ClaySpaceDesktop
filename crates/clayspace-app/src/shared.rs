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
    Armature, ArmatureModel, BrushSettings, CombineSettings, CurveJoin, CurveModel, CurveProfile,
    CurveState, DocumentModel, EditOutcome, ExchangeModel, ExportSettings, ExtrudeSettings,
    GestureSample, GizmoHandle, GizmoMode, HistoryState, ImportSettings, LatticeModel,
    LatticeState, LayerCost, LayerKey, MaskModel, MaskOp, MaskState, ModelError, NodeIndex,
    OpenError, Protection, Representation, Scene, SceneModel, SceneStats, SculptModel,
    SkinSettings, ToolKind,
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

    // Every provided method of `SculptModel` is forwarded here as well, and
    // the ones below are why. A default exists so a *double* that models one
    // representation need not spell out answers it has none for; this is not a
    // double, it is the one document, and inheriting a default means quietly
    // answering for it. `set_combine` was the one that showed it: the options
    // bar dispatched the command, the ViewModel called the model, the default
    // discarded it, and fourteen combine operations drew the same picture. Any
    // provided method added to the trait belongs here too.
    fn active_layer_carries_geometry(&self) -> bool {
        self.0.borrow().active_layer_carries_geometry()
    }

    fn active_layer_visible(&self) -> bool {
        self.0.borrow().active_layer_visible()
    }

    fn apply_operation(
        &mut self,
        operation: clayspace_model::LayerOperation,
    ) -> Result<EditOutcome, ModelError> {
        self.0.borrow_mut().apply_operation(operation)
    }

    fn set_combine(&mut self, combine: CombineSettings) {
        self.0.borrow_mut().set_combine(combine);
    }

    fn combine(&self) -> CombineSettings {
        self.0.borrow().combine()
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
        self.0
            .borrow_mut()
            .set_layer_transform(key, position, scale)
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

impl MaskModel for SharedDocument {
    fn mask_state(&self) -> MaskState {
        self.0.borrow().mask_state()
    }

    fn apply_mask_op(&mut self, op: MaskOp) -> Result<(), ModelError> {
        self.0.borrow_mut().apply_mask_op(op)
    }

    fn extrude_mask(&mut self, settings: ExtrudeSettings) -> Result<(), ModelError> {
        self.0.borrow_mut().extrude_mask(settings)
    }
}

impl CurveModel for SharedDocument {
    fn curve(&self) -> CurveState {
        self.0.borrow().curve()
    }
    fn begin_curve(&mut self) {
        self.0.borrow_mut().begin_curve()
    }
    fn add_curve_point(&mut self, at: [f32; 3], radius: f32) -> Result<(), ModelError> {
        self.0.borrow_mut().add_curve_point(at, radius)
    }
    fn select_curve_point(&mut self, index: Option<usize>) {
        self.0.borrow_mut().select_curve_point(index)
    }
    fn toggle_curve_point(&mut self, index: usize) {
        self.0.borrow_mut().toggle_curve_point(index)
    }
    fn drag_curve(&mut self, by: [f32; 3]) -> Result<(), ModelError> {
        self.0.borrow_mut().drag_curve(by)
    }
    fn set_curve_radius(&mut self, radius: f32) -> Result<(), ModelError> {
        self.0.borrow_mut().set_curve_radius(radius)
    }
    fn set_curve_join(&mut self, join: CurveJoin) -> Result<(), ModelError> {
        self.0.borrow_mut().set_curve_join(join)
    }
    fn set_curve_profile(&mut self, profile: CurveProfile) -> Result<(), ModelError> {
        self.0.borrow_mut().set_curve_profile(profile)
    }
    fn remove_curve_points(&mut self) -> Result<(), ModelError> {
        self.0.borrow_mut().remove_curve_points()
    }
    fn apply_curve(&mut self) -> Result<(), ModelError> {
        self.0.borrow_mut().apply_curve()
    }
    fn cancel_curve(&mut self) {
        self.0.borrow_mut().cancel_curve()
    }
}

impl LatticeModel for SharedDocument {
    fn lattice(&self) -> LatticeState {
        self.0.borrow().lattice()
    }

    fn begin_lattice(&mut self, divisions: [i32; 3]) -> Result<(), ModelError> {
        self.0.borrow_mut().begin_lattice(divisions)
    }

    fn select_lattice_point(&mut self, index: Option<usize>) {
        self.0.borrow_mut().select_lattice_point(index)
    }

    fn toggle_lattice_point(&mut self, index: usize) {
        self.0.borrow_mut().toggle_lattice_point(index)
    }

    fn set_gizmo_mode(&mut self, mode: GizmoMode) {
        self.0.borrow_mut().set_gizmo_mode(mode)
    }

    fn begin_gizmo_drag(&mut self, handle: GizmoHandle, anchor: [f32; 3]) {
        self.0.borrow_mut().begin_gizmo_drag(handle, anchor)
    }

    fn drag_gizmo(&mut self, to: [f32; 3]) -> Result<(), ModelError> {
        self.0.borrow_mut().drag_gizmo(to)
    }

    fn end_gizmo_drag(&mut self) {
        self.0.borrow_mut().end_gizmo_drag()
    }

    fn drag_lattice_point(&mut self, to: [f32; 3]) -> Result<(), ModelError> {
        self.0.borrow_mut().drag_lattice_point(to)
    }

    fn apply_lattice(&mut self) -> Result<(), ModelError> {
        self.0.borrow_mut().apply_lattice()
    }

    fn cancel_lattice(&mut self) {
        self.0.borrow_mut().cancel_lattice()
    }
}

impl ArmatureModel for SharedDocument {
    fn armature(&self) -> Option<Armature> {
        self.0.borrow().armature()
    }

    fn begin_armature(&mut self, position: [f32; 3], radius: f32) -> Result<(), ModelError> {
        self.0.borrow_mut().begin_armature(position, radius)
    }

    fn add_zsphere(
        &mut self,
        parent: NodeIndex,
        position: [f32; 3],
        radius: f32,
        mirrored: bool,
    ) -> Result<NodeIndex, ModelError> {
        self.0
            .borrow_mut()
            .add_zsphere(parent, position, radius, mirrored)
    }

    fn move_zsphere(&mut self, index: NodeIndex, delta: [f32; 3]) -> Result<(), ModelError> {
        self.0.borrow_mut().move_zsphere(index, delta)
    }

    fn resize_zsphere(&mut self, index: NodeIndex, radius: f32) -> Result<(), ModelError> {
        self.0.borrow_mut().resize_zsphere(index, radius)
    }

    fn reparent_zsphere(
        &mut self,
        index: NodeIndex,
        new_parent: NodeIndex,
    ) -> Result<(), ModelError> {
        self.0.borrow_mut().reparent_zsphere(index, new_parent)
    }

    fn remove_zsphere(&mut self, index: NodeIndex) -> Result<(), ModelError> {
        self.0.borrow_mut().remove_zsphere(index)
    }

    fn insert_zsphere(&mut self, child: NodeIndex) -> Result<NodeIndex, ModelError> {
        self.0.borrow_mut().insert_zsphere(child)
    }

    fn set_zsphere_negative(&mut self, index: NodeIndex, negative: bool) -> Result<(), ModelError> {
        self.0.borrow_mut().set_zsphere_negative(index, negative)
    }

    fn set_skin(&mut self, skin: SkinSettings) -> Result<(), ModelError> {
        self.0.borrow_mut().set_skin(skin)
    }

    fn skin(&self) -> SkinSettings {
        self.0.borrow().skin()
    }
}

impl ExchangeModel for SharedDocument {
    fn import_mesh(
        &mut self,
        path: &std::path::Path,
        settings: ImportSettings,
    ) -> Result<(), ModelError> {
        self.0.borrow_mut().import_mesh(path, settings)
    }

    fn export_mesh(
        &mut self,
        path: &std::path::Path,
        settings: ExportSettings,
    ) -> Result<(), ModelError> {
        self.0.borrow_mut().export_mesh(path, settings)
    }

    fn has_mesh_layers(&self) -> bool {
        self.0.borrow().has_mesh_layers()
    }
}
