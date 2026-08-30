//! The cage, as something the interface can put up, drag and apply.
//!
//! Separate from the sculpting ViewModel for the same reason the mask is: a
//! brush asks "what happens to the surface under the pointer", and a cage asks
//! "what happens to the whole form". The sculptor's attention is on a handle
//! rather than on the clay, and the two answer different questions.

use clayspace_model::{LatticeModel, LatticeState, ModelError, Representation};

use crate::command::Command;
use crate::observable::Observable;

pub struct LatticeViewModel {
    model: Box<dyn LatticeModel>,

    state: Observable<LatticeState>,
    /// What a fresh cage would be built with, as the panel has it set.
    ///
    /// Held apart from the cage itself so the panel reads the same whether one
    /// is up or not — and because changing it *replaces* the cage rather than
    /// resizing one, which would move points the sculptor never touched.
    divisions: Observable<[i32; 3]>,
    /// The last refusal, for the status area.
    notice: Observable<Option<String>>,
}

impl LatticeViewModel {
    pub fn new(model: Box<dyn LatticeModel>) -> Self {
        let state = model.lattice();
        Self {
            model,
            state: Observable::new(state),
            divisions: Observable::new([3, 3, 3]),
            notice: Observable::new(None),
        }
    }

    pub fn state(&self) -> &Observable<LatticeState> {
        &self.state
    }

    pub fn divisions(&self) -> &Observable<[i32; 3]> {
        &self.divisions
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    /// Refreshes from the model, for when something else changed the layer.
    ///
    /// A cage belongs to the layer it was put around, so anything that changes
    /// which layer is active takes it down — and this ViewModel has to be told
    /// to look rather than assume it is still up.
    pub fn refresh(&mut self) {
        let state = self.model.lattice();
        self.state.set_if_changed(state);
    }

    pub fn dispatch(&mut self, command: &Command, representation: Representation) {
        match command {
            Command::ToggleLattice => {
                if self.state.get().active {
                    self.model.cancel_lattice();
                } else {
                    self.raise(
                        |model, divisions| model.begin_lattice(divisions),
                        representation,
                    );
                }
                self.refresh();
            }
            Command::SetLatticeDivisions(divisions) => {
                let clamped = clayspace_model::clamp_divisions(*divisions, representation);
                if !self.divisions.set_if_changed(clamped) {
                    return;
                }
                // A cage already up is *replaced* rather than resized: carrying
                // the drags across a change of resolution would move points
                // nobody touched, and the new grid does not line up with the
                // old one anyway.
                if self.state.get().active {
                    self.raise(
                        |model, divisions| model.begin_lattice(divisions),
                        representation,
                    );
                    self.refresh();
                }
            }
            Command::SelectLatticePoint(index) => {
                self.model.select_lattice_point(*index);
                self.refresh();
            }
            Command::ToggleLatticePoint(index) => {
                self.model.toggle_lattice_point(*index);
                self.refresh();
            }
            Command::SelectLatticePoints(indices) => {
                self.model.select_lattice_points(indices);
                self.refresh();
            }
            Command::SetGizmoMode(mode) => {
                self.model.set_gizmo_mode(*mode);
                self.refresh();
            }
            Command::BeginGizmoDrag(handle, anchor, view_axis) => {
                self.model.begin_gizmo_drag(*handle, *anchor, *view_axis);
                self.refresh();
            }
            Command::DragGizmo(to, snap) => {
                if let Err(e) = self.model.drag_gizmo(*to, *snap) {
                    self.notice.set(Some(e.to_string()));
                }
                self.refresh();
            }
            Command::EndGizmoDrag => {
                self.model.end_gizmo_drag();
                self.refresh();
            }
            Command::DragLatticePoint(to) => {
                if let Err(e) = self.model.drag_lattice_point(*to) {
                    self.notice.set(Some(e.to_string()));
                }
                self.refresh();
            }
            Command::ApplyLattice => {
                match self.model.apply_lattice() {
                    Ok(()) => {
                        self.notice.set_if_changed(None);
                    }
                    Err(e) => self.notice.set(Some(e.to_string())),
                }
                self.refresh();
            }
            // The model takes a standing cage down when the active subtool
            // changes — it was sized to a form that is no longer the one being
            // worked — so this has to look again rather than go on drawing
            // control points around something else. The sculptor was already
            // asked what became of it before the command was dispatched.
            Command::SelectLayer(_) => self.refresh(),
            _ => {}
        }
    }

    /// Puts a cage up, carrying a refusal to the status area rather than
    /// dropping it.
    fn raise(
        &mut self,
        build: impl FnOnce(&mut dyn LatticeModel, [i32; 3]) -> Result<(), ModelError>,
        representation: Representation,
    ) {
        let divisions = clayspace_model::clamp_divisions(*self.divisions.get(), representation);
        self.divisions.set_if_changed(divisions);
        match build(self.model.as_mut(), divisions) {
            Ok(()) => {
                self.notice.set_if_changed(None);
            }
            Err(e) => self.notice.set(Some(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clayspace_model::{GizmoHandle, GizmoMode};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct Recorded {
        raised: Vec<[i32; 3]>,
        dragged: Vec<[f32; 3]>,
        applied: usize,
        cancelled: usize,
    }

    struct FakeCage {
        recorded: Rc<RefCell<Recorded>>,
        state: LatticeState,
    }

    impl LatticeModel for FakeCage {
        fn lattice(&self) -> LatticeState {
            self.state.clone()
        }

        fn begin_lattice(&mut self, divisions: [i32; 3]) -> Result<(), ModelError> {
            self.recorded.borrow_mut().raised.push(divisions);
            let count = divisions.iter().map(|n| *n as usize).product();
            self.state = LatticeState {
                active: true,
                divisions,
                points: vec![[0.0; 3]; count],
                selection: Vec::new(),
                mode: GizmoMode::default(),
                rest_span: 2.0,
                touched: false,
            };
            Ok(())
        }

        fn select_lattice_points(&mut self, indices: &[usize]) {
            let mut selection = indices.to_vec();
            selection.sort_unstable();
            selection.dedup();
            self.state.selection = selection;
        }

        fn select_lattice_point(&mut self, index: Option<usize>) {
            self.state.selection = index.into_iter().collect();
        }

        fn toggle_lattice_point(&mut self, index: usize) {
            match self.state.selection.binary_search(&index) {
                Ok(at) => {
                    self.state.selection.remove(at);
                }
                Err(at) => self.state.selection.insert(at, index),
            }
        }

        fn set_gizmo_mode(&mut self, mode: GizmoMode) {
            self.state.mode = mode;
        }

        fn begin_gizmo_drag(&mut self, _: GizmoHandle, _: [f32; 3], _: [f32; 3]) {}

        fn drag_gizmo(&mut self, _: [f32; 3], _: bool) -> Result<(), ModelError> {
            self.state.touched = true;
            Ok(())
        }

        fn end_gizmo_drag(&mut self) {}

        fn drag_lattice_point(&mut self, to: [f32; 3]) -> Result<(), ModelError> {
            self.recorded.borrow_mut().dragged.push(to);
            self.state.touched = true;
            Ok(())
        }

        fn apply_lattice(&mut self) -> Result<(), ModelError> {
            self.recorded.borrow_mut().applied += 1;
            self.state = LatticeState::default();
            Ok(())
        }

        fn cancel_lattice(&mut self) {
            self.recorded.borrow_mut().cancelled += 1;
            self.state = LatticeState::default();
        }
    }

    fn fixture() -> (LatticeViewModel, Rc<RefCell<Recorded>>) {
        let recorded = Rc::new(RefCell::new(Recorded::default()));
        let model = FakeCage {
            recorded: recorded.clone(),
            state: LatticeState::default(),
        };
        (LatticeViewModel::new(Box::new(model)), recorded)
    }

    #[test]
    fn the_cage_goes_up_and_comes_down() {
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::ToggleLattice, Representation::Mesh);
        assert!(vm.state().get().active, "the cage did not go up");

        vm.dispatch(&Command::ToggleLattice, Representation::Mesh);
        assert!(!vm.state().get().active, "the cage did not come down");
        assert_eq!(
            recorded.borrow().cancelled,
            1,
            "taking a cage down applied it instead, which bends the form a \
             sculptor was abandoning"
        );
        assert_eq!(recorded.borrow().applied, 0);
    }

    #[test]
    fn the_divisions_are_the_representations_own_ceiling() {
        // The panel offers one control for both routes and the ceilings are
        // different, so a sculptor who runs the slider up on a field means "as
        // fine as this can go" rather than "fail".
        let (mut vm, _) = fixture();
        vm.dispatch(&Command::SetLatticeDivisions([32; 3]), Representation::Mesh);
        assert_eq!(*vm.divisions().get(), [32; 3]);

        vm.dispatch(&Command::SetLatticeDivisions([32; 3]), Representation::Sdf);
        assert_eq!(
            *vm.divisions().get(),
            [4; 3],
            "a field cage went past the four points per axis the engine takes"
        );
    }

    #[test]
    fn changing_the_divisions_replaces_a_cage_already_up() {
        // Rather than resizing one. Carrying the drags across a change of
        // resolution would move points nobody touched, and the new grid does
        // not line up with the old one anyway.
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::ToggleLattice, Representation::Mesh);
        vm.dispatch(&Command::SelectLatticePoint(Some(0)), Representation::Mesh);
        vm.dispatch(
            &Command::DragLatticePoint([1.0, 0.0, 0.0]),
            Representation::Mesh,
        );
        assert!(vm.state().get().touched);

        vm.dispatch(&Command::SetLatticeDivisions([5; 3]), Representation::Mesh);
        assert_eq!(
            recorded.borrow().raised,
            vec![[3, 3, 3], [5, 5, 5]],
            "the cage was not rebuilt at the new resolution"
        );
        assert!(
            !vm.state().get().touched,
            "the old drags survived a change of resolution"
        );
    }

    #[test]
    fn changing_the_divisions_with_no_cage_up_builds_nothing() {
        // Setting up before putting a cage around anything is ordinary, and it
        // must not raise one behind the sculptor's back.
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::SetLatticeDivisions([6; 3]), Representation::Mesh);
        assert_eq!(*vm.divisions().get(), [6; 3]);
        assert!(recorded.borrow().raised.is_empty());
        assert!(!vm.state().get().active);
    }

    #[test]
    fn a_grid_is_refused_readably_rather_than_silently() {
        struct NoCage;
        impl LatticeModel for NoCage {
            fn lattice(&self) -> LatticeState {
                LatticeState::default()
            }
            fn begin_lattice(&mut self, _: [i32; 3]) -> Result<(), ModelError> {
                Err(ModelError::engine("uma camada de voxels não aceita"))
            }
            fn select_lattice_point(&mut self, _: Option<usize>) {}
            fn select_lattice_points(&mut self, _: &[usize]) {}
            fn toggle_lattice_point(&mut self, _: usize) {}
            fn set_gizmo_mode(&mut self, _: GizmoMode) {}
            fn begin_gizmo_drag(&mut self, _: GizmoHandle, _: [f32; 3], _: [f32; 3]) {}
            fn drag_gizmo(&mut self, _: [f32; 3], _: bool) -> Result<(), ModelError> {
                Ok(())
            }
            fn end_gizmo_drag(&mut self) {}
            fn drag_lattice_point(&mut self, _: [f32; 3]) -> Result<(), ModelError> {
                Ok(())
            }
            fn apply_lattice(&mut self) -> Result<(), ModelError> {
                Ok(())
            }
            fn cancel_lattice(&mut self) {}
        }
        let mut vm = LatticeViewModel::new(Box::new(NoCage));
        vm.dispatch(&Command::ToggleLattice, Representation::Voxel);
        assert!(!vm.state().get().active);
        assert!(
            vm.notice().get().is_some(),
            "the refusal went nowhere, so the button does nothing and says \
             nothing — which is what the extrude entry did"
        );
    }
}
