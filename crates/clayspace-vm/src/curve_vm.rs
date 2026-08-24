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
