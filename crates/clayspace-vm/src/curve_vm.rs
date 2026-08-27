//! The curve, as something the interface can place, shape and let go of.
//!
//! Separate from the sculpting ViewModel for the reason the mask and the cage
//! are: a brush asks what happens to the clay under the pointer, and a curve
//! asks where its own points are. The sculptor's attention is on a handle.

use clayspace_model::{CurveJoin, CurveModel, CurveProfile, CurveState};

use crate::command::Command;
use crate::observable::Observable;

pub struct CurveViewModel {
    model: Box<dyn CurveModel>,

    state: Observable<CurveState>,
    /// The thickness a new point is given, as the panel has it set.
    radius: Observable<f32>,
    /// The last refusal, for the status area.
    notice: Observable<Option<String>>,
}

/// What a tube is by default, in document units.
const DEFAULT_RADIUS: f32 = 0.12;

impl CurveViewModel {
    pub fn new(model: Box<dyn CurveModel>) -> Self {
        let state = model.curve();
        Self {
            model,
            state: Observable::new(state),
            radius: Observable::new(DEFAULT_RADIUS),
            notice: Observable::new(None),
        }
    }

    pub fn state(&self) -> &Observable<CurveState> {
        &self.state
    }

    pub fn radius(&self) -> &Observable<f32> {
        &self.radius
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    /// Refreshes from the model, for when something else changed the layer.
    pub fn refresh(&mut self) {
        let state = self.model.curve();
        self.state.set_if_changed(state);
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::ToggleCurve => {
                if self.state.get().active {
                    self.model.cancel_curve();
                } else {
                    self.model.begin_curve();
                }
            }
            Command::AddCurvePoint(at, radius) => {
                self.report(|model| model.add_curve_point(*at, *radius));
            }
            Command::SelectCurvePoint(index) => self.model.select_curve_point(*index),
            Command::ToggleCurvePoint(index) => self.model.toggle_curve_point(*index),
            Command::DragCurve(by) => self.report(|model| model.drag_curve(*by)),
            Command::SetCurveRadius(radius) => {
                // Held as well as applied: it is what the *next* point is
                // given, and a panel that forgot it would hand every new point
                // the default however the last one was set.
                self.radius.set_if_changed(radius.max(1e-3));
                self.report(|model| model.set_curve_radius(*radius));
            }
            Command::SetCurveJoin(join) => self.report(|model| model.set_curve_join(*join)),
            Command::SetCurveProfile(profile) => {
                self.report(|model| model.set_curve_profile(*profile));
            }
            Command::RemoveCurvePoints => self.report(|model| model.remove_curve_points()),
            Command::ApplyCurve => self.report(|model| model.apply_curve()),
            _ => return,
        }
        self.refresh();
    }

    /// Runs an edit, carrying a refusal to the status area rather than
    /// dropping it.
    fn report(
        &mut self,
        edit: impl FnOnce(&mut dyn CurveModel) -> Result<(), clayspace_model::ModelError>,
    ) {
        match edit(self.model.as_mut()) {
            Ok(()) => {
                self.notice.set_if_changed(None);
            }
            Err(e) => self.notice.set(Some(e.to_string())),
        }
    }

    /// What a fresh curve would be built with.
    pub fn defaults(&self) -> (CurveJoin, CurveProfile) {
        let state = self.state.get();
        (state.join, state.profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use clayspace_model::{CurvePoint, ModelError};

    /// A curve the ViewModel can talk to, which can be told to refuse.
    struct FakeCurve {
        state: CurveState,
        /// Set to refuse the next edit, as a locked layer would.
        refuse: Option<&'static str>,
        edits: Rc<RefCell<usize>>,
    }

    impl FakeCurve {
        fn refused(&self) -> Option<ModelError> {
            self.refuse.map(ModelError::engine)
        }

        /// Every edit the ViewModel routes through `report` behaves the same
        /// way, so they share one body: refuse where told to, count the run
        /// otherwise.
        fn edit(&mut self) -> Result<(), ModelError> {
            match self.refused() {
                Some(refusal) => Err(refusal),
                None => {
                    *self.edits.borrow_mut() += 1;
                    Ok(())
                }
            }
        }
    }

    impl CurveModel for FakeCurve {
        fn curve(&self) -> CurveState {
            self.state.clone()
        }

        fn begin_curve(&mut self) {
            self.state.active = true;
        }

        fn add_curve_point(&mut self, at: [f32; 3], radius: f32) -> Result<(), ModelError> {
            self.edit()?;
            self.state.points.push(CurvePoint {
                position: at,
                radius,
            });
            Ok(())
        }

        fn select_curve_point(&mut self, index: Option<usize>) {
            self.state.selection = index.into_iter().collect();
        }

        fn toggle_curve_point(&mut self, index: usize) {
            match self.state.selection.binary_search(&index) {
                Ok(at) => {
                    self.state.selection.remove(at);
                }
                Err(at) => self.state.selection.insert(at, index),
            }
        }

        fn drag_curve(&mut self, _by: [f32; 3]) -> Result<(), ModelError> {
            self.edit()
        }

        fn set_curve_radius(&mut self, _radius: f32) -> Result<(), ModelError> {
            self.edit()
        }

        fn set_curve_join(&mut self, join: CurveJoin) -> Result<(), ModelError> {
            self.edit()?;
            self.state.join = join;
            Ok(())
        }

        fn set_curve_profile(&mut self, profile: CurveProfile) -> Result<(), ModelError> {
            self.edit()?;
            self.state.profile = profile;
            Ok(())
        }

        fn remove_curve_points(&mut self) -> Result<(), ModelError> {
            self.edit()
        }

        fn apply_curve(&mut self) -> Result<(), ModelError> {
            self.edit()?;
            self.state.active = false;
            Ok(())
        }

        fn cancel_curve(&mut self) {
            self.state = CurveState::default();
        }
    }

    fn fixture(refuse: Option<&'static str>) -> (CurveViewModel, Rc<RefCell<usize>>) {
        let edits = Rc::new(RefCell::new(0));
        let model = FakeCurve {
            state: CurveState {
                active: true,
                ..CurveState::default()
            },
            refuse,
            edits: edits.clone(),
        };
        (CurveViewModel::new(Box::new(model)), edits)
    }

    /// Every edit a cage takes goes through one `report`, so every one of them
    /// has to carry a refusal to the status area. A curve that will not take a
    /// point and says nothing reads as the click having missed.
    #[test]
    fn every_refused_edit_is_stated_rather_than_silent() {
        const LOCKED: &str = "esta camada está bloqueada";

        let edits = [
            Command::AddCurvePoint([0.0, 0.0, 0.0], 0.1),
            Command::DragCurve([0.1, 0.0, 0.0]),
            Command::SetCurveRadius(0.2),
            Command::SetCurveJoin(CurveJoin::Corners),
            Command::SetCurveProfile(CurveProfile::Square),
            Command::RemoveCurvePoints,
            Command::ApplyCurve,
        ];

        for command in edits {
            let (mut vm, ran) = fixture(Some(LOCKED));
            vm.dispatch(&command);
            assert_eq!(
                vm.notice().get().as_deref(),
                Some(LOCKED),
                "{command:?} dropped the refusal"
            );
            assert_eq!(*ran.borrow(), 0, "{command:?} reached the model anyway");
        }
    }

    /// And an edit that works takes the last refusal down, so the status area
    /// is not still explaining something that has since been put right.
    #[test]
    fn an_edit_that_works_clears_what_the_last_one_said() {
        let (mut vm, ran) = fixture(None);
        vm.notice.set(Some("de antes".into()));

        vm.dispatch(&Command::AddCurvePoint([0.0, 0.0, 0.0], 0.1));
        assert!(vm.notice().get().is_none(), "a stale refusal was left up");
        assert_eq!(*ran.borrow(), 1);
    }

    /// The panel's radius is what the *next* point is given, so it is held
    /// here as well as applied — and held above nothing, because a tube of no
    /// thickness is not a tube.
    #[test]
    fn the_radius_is_held_for_the_next_point_and_kept_above_nothing() {
        let (mut vm, _) = fixture(None);
        vm.dispatch(&Command::SetCurveRadius(0.35));
        assert!((*vm.radius().get() - 0.35).abs() < 1e-6);

        vm.dispatch(&Command::SetCurveRadius(-1.0));
        assert!(*vm.radius().get() > 0.0, "a radius of nothing is not thin");
    }
}
