//! The mask, as something the interface can act on.
//!
//! Separate from the sculpting ViewModel because the two answer different
//! questions. A brush asks "what happens to the surface"; these ask "what
//! happens to the region that is protected from brushes".

use clayspace_model::{ExtrudeSettings, MaskModel, MaskOp, MaskState};

use crate::command::Command;
use crate::observable::Observable;

pub struct MaskViewModel {
    model: Box<dyn MaskModel>,

    state: Observable<MaskState>,
    /// What an extrusion would use, as the panel has it set.
    extrude: Observable<ExtrudeSettings>,
    /// The last refusal, for the status area.
    notice: Observable<Option<String>>,
}

impl MaskViewModel {
    pub fn new(model: Box<dyn MaskModel>) -> Self {
        let state = model.mask_state();
        Self {
            model,
            state: Observable::new(state),
            extrude: Observable::new(ExtrudeSettings::default()),
            notice: Observable::new(None),
        }
    }

    pub fn state(&self) -> &Observable<MaskState> {
        &self.state
    }

    pub fn extrude_settings(&self) -> &Observable<ExtrudeSettings> {
        &self.extrude
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    /// Whether an operation would do anything right now.
    ///
    /// The interface uses this to disable rather than hide: a menu whose
    /// entries come and go is harder to learn than one whose entries are
    /// sometimes grey.
    pub fn can_apply(&self, op: MaskOp) -> bool {
        !op.needs_a_mask() || self.state.get().is_active()
    }

    /// Refreshes from the model, for when something else changed the mask.
    ///
    /// Painting the mask goes through the sculpting ViewModel — it is a stroke
    /// like any other — so this ViewModel does not see it happen and has to be
    /// told to look again.
    pub fn refresh(&mut self) {
        let state = self.model.mask_state();
        self.state.set_if_changed(state);
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::ApplyMaskOp(op) => {
                if !self.can_apply(*op) {
                    self.notice.set(Some("não há máscara para editar".into()));
                    return;
                }
                match self.model.apply_mask_op(*op) {
                    Ok(()) => {
                        self.notice.set_if_changed(None);
                    }
                    Err(e) => self.notice.set(Some(e.to_string())),
                }
                self.refresh();
            }
            Command::ExtrudeMask(settings) => {
                match self.model.extrude_mask(*settings) {
                    Ok(()) => {
                        self.notice.set_if_changed(None);
                    }
                    Err(e) => self.notice.set(Some(e.to_string())),
                }
                // Extruding reads the mask rather than consuming it, but the
                // count is re-read anyway: assuming it is unchanged would make
                // this ViewModel the one place that believes something about
                // the model without asking.
                self.refresh();
            }
            _ => {}
        }
    }
}
