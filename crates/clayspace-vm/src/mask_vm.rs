//! The mask, as something the interface can act on.
//!
//! Separate from the sculpting ViewModel because the two answer different
//! questions. A brush asks "what happens to the surface"; these ask "what
//! happens to the region that is protected from brushes".

#[cfg(test)]
use clayspace_model::ExtrudeSide;
use clayspace_model::{
    ExtrudeSettings, MaskGesture, MaskModel, MaskOp, MaskState, OutlineDraft, OutlineFrame,
    OutlineMode,
};

use crate::command::Command;
use crate::observable::Observable;

pub struct MaskViewModel {
    model: Box<dyn MaskModel>,

    state: Observable<MaskState>,
    /// What an extrusion would use, as the panel has it set.
    extrude: Observable<ExtrudeSettings>,
    /// How far Expandir, Contrair and Suavizar máscara reach.
    ///
    /// One control for the three because they are one question — how much —
    /// even though the engine measures the first two in cells and the third in
    /// passes. The menu shows the number beside each so the unit is never in
    /// doubt.
    steps: Observable<i32>,
    /// Which gesture the mask brush makes.
    gesture: Observable<MaskGesture>,
    /// The outline being drawn, in normalised device coordinates.
    ///
    /// Held here rather than in the composition root because the viewport
    /// draws it: an overlay read from ViewModel state is one a headless test
    /// can assert on, and the first version of this kept the points beside the
    /// event loop where nothing could.
    draft: Observable<Option<OutlineDraft>>,
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
            steps: Observable::new(1),
            gesture: Observable::new(MaskGesture::default()),
            draft: Observable::new(None),
            notice: Observable::new(None),
        }
    }

    pub fn state(&self) -> &Observable<MaskState> {
        &self.state
    }

    pub fn extrude_settings(&self) -> &Observable<ExtrudeSettings> {
        &self.extrude
    }

    pub fn steps(&self) -> &Observable<i32> {
        &self.steps
    }

    pub fn gesture(&self) -> &Observable<MaskGesture> {
        &self.gesture
    }

    /// The outline being drawn, for the viewport to trace.
    pub fn draft(&self) -> &Observable<Option<OutlineDraft>> {
        &self.draft
    }

    /// Whether a press over the viewport should draw a shape rather than
    /// paint.
    ///
    /// Asked with the tool in hand rather than answered from state here: the
    /// gesture belongs to the mask brush, and an outline drawn while a
    /// sculpting tool is selected would be a mode nobody chose.
    pub fn draws_an_outline(&self, painting_the_mask: bool) -> bool {
        painting_the_mask && self.gesture.get().draws_an_outline()
    }

    /// The operation with the panel's amount filled in.
    ///
    /// Asked here rather than in the View so a menu and a shortcut cannot come
    /// to different answers about how far Expandir reaches.
    pub fn sized(&self, op: MaskOp) -> MaskOp {
        let steps = *self.steps.get();
        match op {
            MaskOp::Expand(_) => MaskOp::Expand(steps),
            MaskOp::Contract(_) => MaskOp::Contract(steps),
            MaskOp::Smooth(_) => MaskOp::Smooth(steps),
            // Invert, the bounded complement and Clear have no amount: there
            // is no "invert twice as much".
            other => other,
        }
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

    /// Carries the drawn outline onto the frame it was drawn over and applies
    /// it.
    ///
    /// The draft is taken down whatever happens. A gesture that is refused has
    /// still ended, and an outline left on the screen after the pointer came
    /// up would be a line the sculptor cannot get rid of.
    fn close_the_outline(&mut self, frame: OutlineFrame) {
        let Some(draft) = self.draft.get().clone() else {
            return;
        };
        self.draft.set(None);
        let outline = draft.onto(frame);
        if !outline.encloses_anything() {
            // Not a refusal: a click with a drawn gesture in hand is how a
            // sculptor finds out what it does, and it should do nothing
            // quietly.
            return;
        }
        match self.model.apply_outline(&outline) {
            Ok(()) => {
                self.notice.set_if_changed(None);
            }
            Err(e) => self.notice.set(Some(e.to_string())),
        }
        self.refresh();
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::SetMaskSteps(steps) => {
                self.steps.set_if_changed((*steps).clamp(1, 16));
            }
            Command::SetMaskGesture(gesture) => {
                if self.gesture.set_if_changed(*gesture) {
                    // A gesture abandoned by changing gesture: the outline on
                    // screen was drawn for the mode that has just been left.
                    self.draft.set(None);
                }
            }
            Command::BeginMaskOutline(at, thaw) => {
                let mode = if *thaw {
                    OutlineMode::Thaw
                } else {
                    OutlineMode::Freeze
                };
                self.notice.set_if_changed(None);
                // The gesture the draft was begun with is carried on it rather
                // than read back later: switching from Laço to Retângulo with
                // the pointer down would otherwise reinterpret the points
                // already collected.
                let gesture = *self.gesture.get();
                self.draft.set(Some(OutlineDraft::new(*at, mode, gesture)));
            }
            Command::ExtendMaskOutline(at) => {
                // Read, extended, put back: `Observable` hands out a shared
                // reference, and the draft is what the overlay is drawn from,
                // so it has to be replaced rather than mutated in place.
                if let Some(mut draft) = self.draft.get().clone() {
                    draft.extend(*at);
                    self.draft.set(Some(draft));
                }
            }
            Command::CancelMaskOutline => {
                self.draft.set(None);
            }
            Command::EndMaskOutline(frame) => self.close_the_outline(*frame),
            Command::SetExtrudeSettings(settings) => {
                self.extrude.set_if_changed(settings.sanitized());
            }
            Command::ApplyMaskOp(op) => {
                // The panel's amount filled in here rather than by the View,
                // so a menu entry and a shortcut cannot come to different
                // answers about how far Expandir reaches.
                let op = self.sized(*op);
                if !self.can_apply(op) {
                    self.notice.set(Some("não há máscara para editar".into()));
                    return;
                }
                match self.model.apply_mask_op(op) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use clayspace_model::ModelError;

    #[derive(Default)]
    struct Recorded {
        ops: Vec<MaskOp>,
        extrusions: Vec<ExtrudeSettings>,
        outlines: Vec<clayspace_model::MaskOutline>,
    }

    struct FakeMask {
        recorded: Rc<RefCell<Recorded>>,
        cells: usize,
        /// Set to make the model refuse, as a mesh layer with no field to
        /// extrude from does.
        refuse: Option<&'static str>,
    }

    impl FakeMask {
        fn refusal(&self) -> Option<ModelError> {
            self.refuse.map(ModelError::engine)
        }
    }

    impl MaskModel for FakeMask {
        fn mask_state(&self) -> MaskState {
            MaskState {
                present: self.cells > 0,
                painted_cells: self.cells,
            }
        }

        fn apply_mask_op(&mut self, op: MaskOp) -> Result<(), ModelError> {
            if let Some(refusal) = self.refusal() {
                return Err(refusal);
            }
            self.recorded.borrow_mut().ops.push(op);
            Ok(())
        }

        fn extrude_mask(&mut self, settings: ExtrudeSettings) -> Result<(), ModelError> {
            if let Some(refusal) = self.refusal() {
                return Err(refusal);
            }
            self.recorded.borrow_mut().extrusions.push(settings);
            Ok(())
        }

        fn apply_outline(
            &mut self,
            outline: &clayspace_model::MaskOutline,
        ) -> Result<(), ModelError> {
            if let Some(refusal) = self.refusal() {
                return Err(refusal);
            }
            self.recorded.borrow_mut().outlines.push(outline.clone());
            Ok(())
        }
    }

    fn frame() -> clayspace_model::OutlineFrame {
        clayspace_model::OutlineFrame {
            origin: [0.0, 0.0, 0.0],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            forward: [0.0, 0.0, -1.0],
            scale: [2.0, 1.0],
        }
    }

    /// Draws a square outline over the viewport, as a drag would.
    fn draw_a_square(vm: &mut MaskViewModel, thaw: bool) {
        vm.dispatch(&Command::BeginMaskOutline([-0.5, -0.5], thaw));
        for at in [[0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]] {
            vm.dispatch(&Command::ExtendMaskOutline(at));
        }
        vm.dispatch(&Command::EndMaskOutline(frame()));
    }

    fn fixture() -> (MaskViewModel, Rc<RefCell<Recorded>>) {
        let recorded = Rc::new(RefCell::new(Recorded::default()));
        let model = FakeMask {
            recorded: recorded.clone(),
            cells: 4096,
            refuse: None,
        };
        (MaskViewModel::new(Box::new(model)), recorded)
    }

    #[test]
    fn the_panels_amount_reaches_the_operation() {
        // The menu dispatched `Expand(1)` and nothing could change the 1, so
        // expanding a mask by four cells meant clicking four times.
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::SetMaskSteps(4));
        for op in [MaskOp::Expand(1), MaskOp::Contract(1), MaskOp::Smooth(1)] {
            vm.dispatch(&Command::ApplyMaskOp(op));
        }
        assert_eq!(
            recorded.borrow().ops,
            vec![MaskOp::Expand(4), MaskOp::Contract(4), MaskOp::Smooth(4)]
        );
    }

    #[test]
    fn an_operation_with_no_amount_is_passed_through() {
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::SetMaskSteps(7));
        for op in [MaskOp::Invert, MaskOp::InvertWithinBounds, MaskOp::Clear] {
            vm.dispatch(&Command::ApplyMaskOp(op));
        }
        assert_eq!(
            recorded.borrow().ops,
            vec![MaskOp::Invert, MaskOp::InvertWithinBounds, MaskOp::Clear],
            "an amount was invented for an operation that has none"
        );
    }

    #[test]
    fn the_amount_is_clamped_to_what_the_engine_accepts() {
        let (mut vm, _) = fixture();
        vm.dispatch(&Command::SetMaskSteps(0));
        assert_eq!(*vm.steps().get(), 1, "zero steps is not an operation");
        vm.dispatch(&Command::SetMaskSteps(-3));
        assert_eq!(*vm.steps().get(), 1, "a negative expand is a contract");
        vm.dispatch(&Command::SetMaskSteps(9999));
        assert_eq!(*vm.steps().get(), 16);
    }

    #[test]
    fn the_extrusion_settings_are_held_and_sanitized() {
        // Every one of these but the side was unreachable: the ViewModel held
        // an `ExtrudeSettings` that no command could write to, so an extrusion
        // was always 0.08 thick with a hard rim.
        let (mut vm, _) = fixture();
        vm.dispatch(&Command::SetExtrudeSettings(ExtrudeSettings {
            thickness: 0.25,
            side: ExtrudeSide::Inward,
            border_round: 0.04,
            border_smooth: 6,
        }));
        let held = *vm.extrude_settings().get();
        assert!((held.thickness - 0.25).abs() < 1e-6);
        assert_eq!(held.side, ExtrudeSide::Inward);
        assert_eq!(held.border_smooth, 6);

        // And a thickness the engine refuses outright is clamped rather than
        // becoming a refusal in the middle of a gesture.
        vm.dispatch(&Command::SetExtrudeSettings(ExtrudeSettings {
            thickness: 0.0,
            ..held
        }));
        assert!(vm.extrude_settings().get().thickness > 0.0);
    }

    #[test]
    fn an_operation_is_refused_when_there_is_no_mask_to_edit() {
        let recorded = Rc::new(RefCell::new(Recorded::default()));
        let model = FakeMask {
            recorded: recorded.clone(),
            cells: 0,
            refuse: None,
        };
        let mut vm = MaskViewModel::new(Box::new(model));
        vm.dispatch(&Command::ApplyMaskOp(MaskOp::Invert));
        assert!(
            recorded.borrow().ops.is_empty(),
            "it reached the model anyway"
        );
        assert!(vm.notice().get().is_some(), "the refusal said nothing");

        // Clearing nothing is not a refusal: the entry is always there and
        // pressing it should do the obvious nothing.
        vm.dispatch(&Command::ApplyMaskOp(MaskOp::Clear));
        assert_eq!(recorded.borrow().ops, vec![MaskOp::Clear]);
    }

    /// The ViewModel's own guard is not the only refusal there is. A mask can
    /// be present and painted and the model still say no — a mesh layer has
    /// no field to extrude from — and that answer has to arrive in the sculptor's
    /// own words rather than being replaced by silence.
    #[test]
    fn a_refusal_from_the_model_arrives_in_its_own_words() {
        const NO_FIELD: &str = "uma camada de malha não tem campo para extrudar";

        let recorded = Rc::new(RefCell::new(Recorded::default()));
        let model = FakeMask {
            recorded: recorded.clone(),
            cells: 4096,
            refuse: Some(NO_FIELD),
        };
        let mut vm = MaskViewModel::new(Box::new(model));

        vm.dispatch(&Command::ExtrudeMask(ExtrudeSettings::default()));
        assert_eq!(
            vm.notice().get().as_deref(),
            Some(NO_FIELD),
            "the model's refusal was dropped on the way to the status area"
        );

        vm.dispatch(&Command::ApplyMaskOp(MaskOp::Expand(1)));
        assert_eq!(vm.notice().get().as_deref(), Some(NO_FIELD));
    }

    /// And the other direction: a run that succeeds takes the last refusal
    /// down, so the status area is not still explaining something that has
    /// since been put right.
    #[test]
    fn a_run_that_works_clears_what_the_last_one_said() {
        let (mut vm, _) = fixture();
        vm.notice.set(Some("de antes".into()));

        vm.dispatch(&Command::ApplyMaskOp(MaskOp::Invert));
        assert!(vm.notice().get().is_none(), "a stale refusal was left up");

        vm.notice.set(Some("de antes".into()));
        vm.dispatch(&Command::ExtrudeMask(ExtrudeSettings::default()));
        assert!(vm.notice().get().is_none());
    }

    #[test]
    fn an_outline_reaches_the_model_carried_onto_its_frame() {
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::SetMaskGesture(MaskGesture::Lasso));
        draw_a_square(&mut vm, false);

        let outlines = &recorded.borrow().outlines;
        assert_eq!(outlines.len(), 1, "the outline never reached the model");
        let outline = &outlines[0];
        assert_eq!(outline.mode, OutlineMode::Freeze);
        assert_eq!(outline.outline.len(), 4);
        // Normalised device coordinates on the way in, world units on the way
        // out: the frame is what carries them across, and an outline still
        // measured in screen units would freeze a region a fraction of the
        // size it was drawn.
        assert_eq!(outline.outline[0], [-1.0, -0.5]);
        assert!(vm.draft().get().is_none(), "the outline was left on screen");
    }

    #[test]
    fn the_modifier_held_at_the_press_decides_what_the_outline_does() {
        // Latched at the press, as a stroke's modifiers are: a key taken up
        // half way round would change what the outline means under the hand
        // that is drawing it.
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::SetMaskGesture(MaskGesture::Lasso));
        draw_a_square(&mut vm, true);
        assert_eq!(recorded.borrow().outlines[0].mode, OutlineMode::Thaw);
    }

    #[test]
    fn the_outline_is_there_to_be_drawn_while_it_is_being_made() {
        let (mut vm, _) = fixture();
        assert!(vm.draft().get().is_none());
        vm.dispatch(&Command::BeginMaskOutline([0.0, 0.0], false));
        assert_eq!(vm.draft().get().as_ref().expect("a draft").track.len(), 1);
        vm.dispatch(&Command::ExtendMaskOutline([0.5, 0.0]));
        assert_eq!(vm.draft().get().as_ref().expect("a draft").track.len(), 2);
    }

    #[test]
    fn a_box_dragged_corner_to_corner_reaches_the_model_as_four_corners() {
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::SetMaskGesture(MaskGesture::Rectangle));
        vm.dispatch(&Command::BeginMaskOutline([-0.5, -0.5], false));
        // The hand wandered on the way; a box is still its two corners.
        for at in [[0.1, 0.3], [0.5, 0.5]] {
            vm.dispatch(&Command::ExtendMaskOutline(at));
        }
        vm.dispatch(&Command::EndMaskOutline(frame()));

        let outlines = &recorded.borrow().outlines;
        assert_eq!(outlines.len(), 1, "the box never reached the model");
        assert_eq!(
            outlines[0].outline.len(),
            4,
            "a rectangle arrived as something other than four corners"
        );
        // The frame's scale is 2 in x and 1 in y, as `frame` sets it.
        assert_eq!(outlines[0].outline[0], [-1.0, -0.5]);
        assert_eq!(outlines[0].outline[2], [1.0, 0.5]);
    }

    #[test]
    fn the_gesture_the_draft_began_with_is_the_one_it_keeps() {
        // Switching gesture with the pointer down would reinterpret the points
        // already collected — a traced path suddenly read as two corners.
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::SetMaskGesture(MaskGesture::Lasso));
        vm.dispatch(&Command::BeginMaskOutline([-0.5, -0.5], false));
        for at in [[0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]] {
            vm.dispatch(&Command::ExtendMaskOutline(at));
        }
        // A gesture change mid-drag takes the draft down rather than
        // reinterpreting it, so there is nothing left to end.
        vm.dispatch(&Command::SetMaskGesture(MaskGesture::Rectangle));
        vm.dispatch(&Command::EndMaskOutline(frame()));
        assert!(recorded.borrow().outlines.is_empty());
    }

    #[test]
    fn an_abandoned_outline_leaves_neither_a_line_nor_a_mask() {
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::BeginMaskOutline([-0.5, -0.5], false));
        vm.dispatch(&Command::ExtendMaskOutline([0.5, -0.5]));
        vm.dispatch(&Command::ExtendMaskOutline([0.5, 0.5]));
        vm.dispatch(&Command::CancelMaskOutline);
        assert!(vm.draft().get().is_none(), "the line was left up");

        // And ending after abandoning does nothing, which is what a release
        // after Escape is.
        vm.dispatch(&Command::EndMaskOutline(frame()));
        assert!(recorded.borrow().outlines.is_empty());
    }

    #[test]
    fn a_click_with_a_drawn_gesture_in_hand_does_nothing_and_says_nothing() {
        // An outline of one point encloses nothing. It is how a sculptor finds
        // out what a tool does, and it must not put a refusal on the screen.
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::BeginMaskOutline([0.0, 0.0], false));
        vm.dispatch(&Command::EndMaskOutline(frame()));
        assert!(recorded.borrow().outlines.is_empty());
        assert!(vm.notice().get().is_none());
        assert!(vm.draft().get().is_none());
    }

    #[test]
    fn the_gesture_is_the_mask_brushs_and_no_other_tools() {
        let (mut vm, _) = fixture();
        assert_eq!(*vm.gesture().get(), MaskGesture::Brush);
        assert!(!vm.draws_an_outline(true), "the brush drew an outline");

        for gesture in [MaskGesture::Lasso, MaskGesture::Rectangle] {
            vm.dispatch(&Command::SetMaskGesture(gesture));
            assert!(vm.draws_an_outline(true), "{gesture:?} drew nothing");
            assert!(
                !vm.draws_an_outline(false),
                "a sculpting tool drew a {gesture:?}"
            );
        }
    }

    #[test]
    fn leaving_a_drawn_gesture_takes_a_half_drawn_outline_with_it() {
        let (mut vm, recorded) = fixture();
        vm.dispatch(&Command::SetMaskGesture(MaskGesture::Lasso));
        vm.dispatch(&Command::BeginMaskOutline([-0.5, -0.5], false));
        vm.dispatch(&Command::ExtendMaskOutline([0.5, -0.5]));
        vm.dispatch(&Command::SetMaskGesture(MaskGesture::Brush));
        assert!(vm.draft().get().is_none(), "the outline outlived its mode");
        vm.dispatch(&Command::EndMaskOutline(frame()));
        assert!(recorded.borrow().outlines.is_empty());
    }

    #[test]
    fn a_refused_outline_says_why_and_takes_its_line_down() {
        const NO_EXTENT: &str = "esta subferramenta não tem extensão";
        let recorded = Rc::new(RefCell::new(Recorded::default()));
        let model = FakeMask {
            recorded: recorded.clone(),
            cells: 0,
            refuse: Some(NO_EXTENT),
        };
        let mut vm = MaskViewModel::new(Box::new(model));
        vm.dispatch(&Command::SetMaskGesture(MaskGesture::Lasso));
        draw_a_square(&mut vm, false);
        assert_eq!(vm.notice().get().as_deref(), Some(NO_EXTENT));
        assert!(vm.draft().get().is_none(), "the outline was left on screen");
    }
}
