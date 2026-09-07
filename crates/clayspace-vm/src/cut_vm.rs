//! The cut, as something the interface draws and then lets go of.
//!
//! Separate from the sculpting ViewModel for the reason the mask's outline is:
//! a brush asks what happens to the clay under the pointer, and a cut asks what
//! shape was drawn over the whole view. The gesture belongs to the view frame,
//! not to the surface.

use clayspace_model::{CutGesture, CutModel, DrawnCut};

use crate::command::Command;
use crate::observable::Observable;

/// A cut being drawn: the points so far, and which shape they will become.
#[derive(Debug, Clone, PartialEq)]
pub struct CutDraft {
    /// The pointer's own record, in normalised device coordinates — every
    /// point for a line or a lasso, two corners for a rectangle.
    pub track: Vec<[f32; 2]>,
    pub gesture: CutGesture,
}

pub struct CutViewModel {
    model: Box<dyn CutModel>,
    /// The gesture the shelf has selected, which the next press will use.
    gesture: Observable<CutGesture>,
    /// The cut being drawn, or nothing between gestures.
    draft: Observable<Option<CutDraft>>,
    notice: Observable<Option<String>>,
}

/// How far the pointer must travel before a lasso keeps another point.
///
/// The same reason the mask's outline thins its own track: a pointer sampled
/// every frame over a slow drag leaves hundreds of points on top of each
/// other, and the tessellator has to walk all of them.
const SPACING: f32 = 0.004;

impl CutViewModel {
    pub fn new(model: Box<dyn CutModel>) -> Self {
        Self {
            model,
            gesture: Observable::new(CutGesture::default()),
            draft: Observable::new(None),
            notice: Observable::new(None),
        }
    }

    pub fn gesture(&self) -> &Observable<CutGesture> {
        &self.gesture
    }

    pub fn draft(&self) -> &Observable<Option<CutDraft>> {
        &self.draft
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::SetCutGesture(gesture) => {
                self.gesture.set_if_changed(*gesture);
            }
            Command::BeginCut(at) => {
                self.notice.set_if_changed(None);
                // The gesture is taken at the press and carried on the draft:
                // switching the shelf from Linha to Laço with the pointer down
                // would otherwise reinterpret the points already collected.
                let gesture = *self.gesture.get();
                self.draft.set(Some(CutDraft {
                    track: vec![*at],
                    gesture,
                }));
            }
            Command::ExtendCut(at) => {
                let Some(mut draft) = self.draft.get().clone() else {
                    return;
                };
                match draft.gesture {
                    // A line and a rectangle are each **two points**: where the
                    // press landed and where the pointer is now. Collecting
                    // every sample between them made a "line" follow the hand's
                    // own wobble, which is a lasso with two ends — and the
                    // engine closes it against the frame as though it were one.
                    // A straight cut has to be straight.
                    CutGesture::Line | CutGesture::Rectangle => {
                        draft.track.truncate(1);
                        draft.track.push(*at);
                    }
                    _ => {
                        let far = draft.track.last().is_none_or(|last| {
                            let step = [at[0] - last[0], at[1] - last[1]];
                            step[0] * step[0] + step[1] * step[1] >= SPACING * SPACING
                        });
                        if far {
                            draft.track.push(*at);
                        }
                    }
                }
                self.draft.set(Some(draft));
            }
            Command::EndCut(frame) => {
                let Some(draft) = self.draft.get().clone() else {
                    return;
                };
                self.draft.set(None);
                let cut = DrawnCut {
                    // Normalised device coordinates are what the viewport
                    // reports; the frame turns them into its own world units,
                    // because the engine has no viewport and does not want one.
                    track: draft.track.iter().map(|at| frame.from_ndc(*at)).collect(),
                    frame: *frame,
                    gesture: draft.gesture,
                };
                if let Err(refusal) = self.model.apply_cut(&cut) {
                    self.notice.set(Some(refusal.to_string()));
                }
            }
            Command::CancelCut => {
                self.draft.set(None);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clayspace_model::OutlineFrame;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct Cuts(Rc<RefCell<Vec<DrawnCut>>>);

    impl CutModel for Cuts {
        fn apply_cut(&mut self, cut: &DrawnCut) -> Result<(), clayspace_model::ModelError> {
            self.0.borrow_mut().push(cut.clone());
            Ok(())
        }
    }

    fn frame() -> OutlineFrame {
        OutlineFrame {
            origin: [0.0; 3],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            forward: [0.0, 0.0, -1.0],
            scale: [2.0, 2.0],
        }
    }

    fn fixture() -> (CutViewModel, Rc<RefCell<Vec<DrawnCut>>>) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        (CutViewModel::new(Box::new(Cuts(seen.clone()))), seen)
    }

    /// The gesture is fixed at the press, so changing the shelf mid-drag
    /// cannot reinterpret points already collected.
    #[test]
    fn the_gesture_is_taken_at_the_press() {
        let (mut vm, seen) = fixture();
        vm.dispatch(&Command::SetCutGesture(CutGesture::Lasso));
        vm.dispatch(&Command::BeginCut([-0.5, 0.0]));
        vm.dispatch(&Command::SetCutGesture(CutGesture::Line));
        vm.dispatch(&Command::ExtendCut([0.0, 0.4]));
        vm.dispatch(&Command::ExtendCut([0.5, 0.0]));
        vm.dispatch(&Command::EndCut(frame()));

        let seen = seen.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].gesture, CutGesture::Lasso);
    }

    /// The frame's scale is applied on the way out: what the viewport reported
    /// was normalised, and what the engine takes is world units.
    #[test]
    fn the_track_reaches_the_model_in_world_units() {
        let (mut vm, seen) = fixture();
        vm.dispatch(&Command::BeginCut([-0.5, 0.0]));
        vm.dispatch(&Command::ExtendCut([0.5, 0.0]));
        vm.dispatch(&Command::EndCut(frame()));

        let seen = seen.borrow();
        assert_eq!(seen[0].track.first(), Some(&[-1.0, 0.0]));
        assert_eq!(seen[0].track.last(), Some(&[1.0, 0.0]));
    }

    /// A rectangle is two corners however far the pointer wandered between
    /// them.
    #[test]
    fn a_rectangle_keeps_only_its_corners() {
        let (mut vm, seen) = fixture();
        vm.dispatch(&Command::SetCutGesture(CutGesture::Rectangle));
        vm.dispatch(&Command::BeginCut([-0.5, -0.5]));
        for at in [[0.0, 0.0], [0.2, 0.3], [0.5, 0.5]] {
            vm.dispatch(&Command::ExtendCut(at));
        }
        vm.dispatch(&Command::EndCut(frame()));

        assert_eq!(seen.borrow()[0].track.len(), 2);
    }

    /// A line is two points, not a traced path.
    ///
    /// Collecting every pointer sample made a "line" follow the hand's own
    /// wobble — and the engine then closed that wobble against the frame as
    /// though it were a curve, so a straight cut came out crooked. Reported
    /// from a session.
    #[test]
    fn a_line_keeps_only_where_it_began_and_where_it_is_now() {
        let (mut vm, seen) = fixture();
        vm.dispatch(&Command::SetCutGesture(CutGesture::Line));
        vm.dispatch(&Command::BeginCut([-0.5, 0.0]));
        // A hand that wandered on the way across.
        for at in [[-0.2, 0.08], [0.1, -0.06], [0.3, 0.05], [0.5, 0.0]] {
            vm.dispatch(&Command::ExtendCut(at));
        }
        vm.dispatch(&Command::EndCut(frame()));

        let seen = seen.borrow();
        assert_eq!(
            seen[0].track.len(),
            2,
            "a line kept {} points, so it is a traced path rather than a line",
            seen[0].track.len()
        );
        assert_eq!(seen[0].track.first(), Some(&[-1.0, 0.0]));
        assert_eq!(seen[0].track.last(), Some(&[1.0, 0.0]));
    }

    /// And a lasso is still traced, or it would be a straight line too.
    #[test]
    fn a_lasso_still_keeps_the_path_it_was_traced_along() {
        let (mut vm, seen) = fixture();
        vm.dispatch(&Command::SetCutGesture(CutGesture::Lasso));
        vm.dispatch(&Command::BeginCut([-0.5, 0.0]));
        for at in [[-0.2, 0.4], [0.3, 0.4], [0.5, 0.0], [0.0, -0.4]] {
            vm.dispatch(&Command::ExtendCut(at));
        }
        vm.dispatch(&Command::EndCut(frame()));
        assert!(
            seen.borrow()[0].track.len() > 2,
            "a lasso was thinned to its ends, which would make it a line"
        );
    }

    #[test]
    fn a_cancelled_cut_reaches_nothing() {
        let (mut vm, seen) = fixture();
        vm.dispatch(&Command::BeginCut([0.0, 0.0]));
        vm.dispatch(&Command::ExtendCut([0.5, 0.0]));
        vm.dispatch(&Command::CancelCut);
        vm.dispatch(&Command::EndCut(frame()));
        assert!(seen.borrow().is_empty());
        assert!(vm.draft().get().is_none());
    }

    /// A refusal is reported rather than swallowed: a cut that did nothing has
    /// to say so.
    #[test]
    fn a_refused_cut_leaves_a_notice() {
        struct Refuses;
        impl CutModel for Refuses {
            fn apply_cut(&mut self, _: &DrawnCut) -> Result<(), clayspace_model::ModelError> {
                Err(clayspace_model::ModelError::engine("não"))
            }
        }
        let mut vm = CutViewModel::new(Box::new(Refuses));
        vm.dispatch(&Command::BeginCut([0.0, 0.0]));
        vm.dispatch(&Command::ExtendCut([0.5, 0.0]));
        vm.dispatch(&Command::EndCut(frame()));
        assert!(vm.notice().get().is_some());
    }
}
