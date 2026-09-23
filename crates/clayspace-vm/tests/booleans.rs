//! The boolean panel, against a double.
//!
//! The rules the panel must obey with no engine behind it: nothing reaches the
//! document until it is confirmed, the price on screen follows the settings
//! that would be run, a newly chosen pair brings its own resolution and the
//! sculptor's own survives everything else, and a refusal is stated rather
//! than swallowed.

use std::cell::RefCell;
use std::rc::Rc;

use clayspace_model::{
    BooleanOp, BooleanRefusal, BooleanSettings, Cost, Direction, Inserted, LayerKey, ModelError,
    ObjectModel,
};
use clayspace_vm::{BooleanViewModel, Command};

#[derive(Debug, Default)]
struct Calls {
    /// Every boolean actually run, in order.
    run: Vec<BooleanSettings>,
    /// How often the panel asked what a pair would cost.
    priced: usize,
}

struct FakeBooleans {
    calls: Rc<RefCell<Calls>>,
    /// What the document offers as an operand. Shared so a test can take one
    /// away from under the panel.
    operands: Rc<RefCell<Vec<(LayerKey, String)>>>,
    /// The cell each operand is worked at, by key.
    detail: Vec<(LayerKey, f32)>,
    /// Set to refuse the next run, as a ghosted operand would.
    refuse: Option<BooleanRefusal>,
    /// Operands the document will not take at all, and why — a hierarchy,
    /// say. Asked when the panel is set, not only when it runs.
    inadmissible: Vec<(LayerKey, BooleanRefusal)>,
    /// The layer the next result arrives on.
    next: u64,
}

impl FakeBooleans {
    fn new(calls: Rc<RefCell<Calls>>) -> Self {
        Self {
            calls,
            operands: Rc::new(RefCell::new(vec![
                (LayerKey(1), "Esfera".into()),
                (LayerKey(2), "Cilindro".into()),
                (LayerKey(3), "Grade".into()),
            ])),
            // The grid is worked finer than the fields, which is what makes a
            // pair including it default finer.
            detail: vec![
                (LayerKey(1), 0.02),
                (LayerKey(2), 0.02),
                (LayerKey(3), 0.005),
            ],
            refuse: None,
            inadmissible: Vec::new(),
            next: 90,
        }
    }
}

impl ObjectModel for FakeBooleans {
    fn admit_boolean_operand(&mut self, operand: LayerKey) -> Result<(), ModelError> {
        match self.inadmissible.iter().find(|(key, _)| *key == operand) {
            Some((_, refusal)) => Err(ModelError::Boolean(refusal.clone())),
            None => Ok(()),
        }
    }

    fn boolean_operands(&mut self) -> Vec<(LayerKey, String)> {
        self.operands.borrow().clone()
    }

    fn boolean_cell(&mut self, base: LayerKey, tool: LayerKey) -> Option<f32> {
        let cell = |key: LayerKey| {
            self.detail
                .iter()
                .find(|(candidate, _)| *candidate == key)
                .map(|(_, cell)| *cell)
        };
        Some(cell(base)?.min(cell(tool)?))
    }

    fn boolean_cost(&mut self, settings: BooleanSettings) -> Option<Cost> {
        settings.pair()?;
        self.calls.borrow_mut().priced += 1;
        Some(Cost::of(
            Direction::SdfToVoxel,
            settings.cell_size,
            [1.0; 3],
        ))
    }

    fn run_boolean(&mut self, settings: BooleanSettings) -> Result<Inserted, ModelError> {
        if let Some(refusal) = self.refuse.clone() {
            return Err(ModelError::Boolean(refusal));
        }
        self.calls.borrow_mut().run.push(settings);
        self.next += 1;
        let layer = LayerKey(self.next);
        self.operands.borrow_mut().push((layer, "Resultado".into()));
        Ok(Inserted {
            layer,
            object: None,
        })
    }
}

fn panel() -> (BooleanViewModel, Rc<RefCell<Calls>>) {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let model = FakeBooleans::new(calls.clone());
    let mut vm = BooleanViewModel::new(Box::new(model));
    vm.dispatch(&Command::ToggleBoolean);
    (vm, calls)
}

fn a_pair(op: BooleanOp) -> BooleanSettings {
    BooleanSettings {
        base: Some(LayerKey(1)),
        tool: Some(LayerKey(2)),
        op,
        ..BooleanSettings::default()
    }
}

/// "It SHALL NOT run a boolean the sculptor has not confirmed." Choosing the
/// operands, the operation and the resolution reaches the document with
/// nothing at all.
#[test]
fn nothing_runs_until_it_is_confirmed() {
    let (mut vm, calls) = panel();

    vm.dispatch(&Command::SetBoolean(a_pair(BooleanOp::Subtract)));
    vm.dispatch(&Command::SetBoolean(BooleanSettings {
        cell_size: 0.01,
        ..a_pair(BooleanOp::Subtract)
    }));
    assert!(
        calls.borrow().run.is_empty(),
        "setting the panel up ran a boolean"
    );
    assert!(
        calls.borrow().priced > 0,
        "the panel never asked what it would cost, so nothing was stated"
    );

    vm.dispatch(&Command::RunBoolean);
    assert_eq!(calls.borrow().run.len(), 1, "confirming ran no boolean");
    assert_eq!(calls.borrow().run[0].op, BooleanOp::Subtract);
    assert_eq!(calls.borrow().run[0].cell_size, 0.01);
}

/// The price follows the settings that would be run. A figure that lagged
/// behind the slider is a figure quoted for a different operation.
#[test]
fn the_stated_cost_follows_the_resolution() {
    let (mut vm, _) = panel();
    vm.dispatch(&Command::SetBoolean(BooleanSettings {
        cell_size: 0.05,
        ..a_pair(BooleanOp::Union)
    }));
    let coarse = vm.cost().get().expect("a chosen pair has a price");

    vm.dispatch(&Command::SetBoolean(BooleanSettings {
        cell_size: 0.01,
        ..a_pair(BooleanOp::Union)
    }));
    let fine = vm.cost().get().expect("still a price");

    assert!(fine.cells > coarse.cells, "the stated cost did not follow");
    assert!(fine.surface_movement < coarse.surface_movement);
}

/// "The default SHALL follow the operands' own detail rather than a fixed
/// constant" — and after that the number is the sculptor's.
#[test]
fn a_new_pair_brings_its_own_resolution_and_then_leaves_it_alone() {
    let (mut vm, _) = panel();
    vm.dispatch(&Command::SetBoolean(a_pair(BooleanOp::Union)));
    assert_eq!(
        vm.settings().get().cell_size,
        0.02,
        "two field subtools did not take the working cell"
    );

    // The grid is worked at 0.005, so a pair including it defaults finer.
    vm.dispatch(&Command::SetBoolean(BooleanSettings {
        tool: Some(LayerKey(3)),
        ..*vm.settings().get()
    }));
    assert_eq!(
        vm.settings().get().cell_size,
        0.005,
        "the default did not follow the operands' own detail"
    );

    let chosen = BooleanSettings {
        cell_size: 0.04,
        ..*vm.settings().get()
    };
    vm.dispatch(&Command::SetBoolean(chosen));
    vm.dispatch(&Command::SetBoolean(BooleanSettings {
        op: BooleanOp::Intersect,
        ..*vm.settings().get()
    }));
    assert_eq!(
        vm.settings().get().cell_size,
        0.04,
        "changing the operation took the sculptor's resolution back to a default"
    );
}

/// A refusal reaches the panel rather than being dropped, and the scene the
/// panel describes is unchanged: the pair it was set to is still set.
#[test]
fn a_refusal_is_stated_and_the_panel_stays_as_it_was() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeBooleans::new(calls.clone());
    model.refuse = Some(BooleanRefusal::Protected {
        operand: "Cilindro".into(),
        ghost: true,
    });
    let mut vm = BooleanViewModel::new(Box::new(model));
    vm.dispatch(&Command::ToggleBoolean);
    vm.dispatch(&Command::SetBoolean(a_pair(BooleanOp::Subtract)));

    vm.dispatch(&Command::RunBoolean);

    let notice = vm
        .notice()
        .get()
        .clone()
        .expect("a refusal must be sayable");
    assert!(
        notice.contains("Cilindro"),
        "the refusal reached the panel without the operand's name: {notice}"
    );
    assert!(
        vm.is_ready(),
        "a refused boolean cleared the pair, so the sculptor cannot fix it and \
         press again"
    );
    assert!(
        *vm.open().get(),
        "a refused boolean closed the panel that has the refusal in it"
    );
}

/// A refusal belongs to the visit it was raised in. Left standing, a sentence
/// about a ghosted cylinder would greet the next pair chosen.
#[test]
fn reopening_the_panel_leaves_the_last_refusal_behind() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeBooleans::new(calls);
    model.refuse = Some(BooleanRefusal::Empty {
        operand: "Vazia".into(),
    });
    let mut vm = BooleanViewModel::new(Box::new(model));
    vm.dispatch(&Command::ToggleBoolean);
    vm.dispatch(&Command::SetBoolean(a_pair(BooleanOp::Union)));
    vm.dispatch(&Command::RunBoolean);
    assert!(vm.notice().get().is_some());

    vm.dispatch(&Command::ToggleBoolean);
    vm.dispatch(&Command::ToggleBoolean);
    assert!(
        vm.notice().get().is_none(),
        "the refusal from the last visit is still on the panel"
    );
}

/// The result is handed to the composition root once, so the manipulator lands
/// on the subtool that just arrived and does not land on it again next frame.
#[test]
fn the_result_is_offered_to_the_manipulator_once() {
    let (mut vm, _) = panel();
    vm.dispatch(&Command::SetBoolean(a_pair(BooleanOp::Subtract)));
    vm.dispatch(&Command::RunBoolean);

    assert_eq!(vm.take_result(), Some(LayerKey(91)));
    assert_eq!(vm.take_result(), None, "the result was offered twice");
    assert!(
        !vm.is_ready(),
        "the operands the boolean consumed or hid are still chosen, so pressing \
         again would run it over a pair that is no longer there"
    );
}

/// A subtool that has gone takes its place in the panel with it — and its
/// price. A pair naming a layer nobody can see is a pair that can only be
/// refused.
#[test]
fn an_operand_that_has_gone_is_dropped_from_the_panel() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let model = FakeBooleans::new(calls);
    let operands = model.operands.clone();
    let mut vm = BooleanViewModel::new(Box::new(model));
    vm.dispatch(&Command::ToggleBoolean);
    vm.dispatch(&Command::SetBoolean(a_pair(BooleanOp::Union)));
    assert!(vm.is_ready());

    operands.borrow_mut().retain(|(key, _)| *key != LayerKey(2));
    vm.refresh();

    assert_eq!(
        vm.settings().get().tool,
        None,
        "a subtool that has gone is still chosen"
    );
    assert!(!vm.is_ready());
    assert!(
        vm.cost().get().is_none(),
        "a price is still quoted for a pair that no longer exists"
    );
}

/// A panel nobody is looking at does not ask the document anything: answering
/// measures every subtool's extent, and the composition root calls this after
/// every edit.
#[test]
fn a_closed_panel_asks_the_document_nothing() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let model = FakeBooleans::new(calls.clone());
    let operands = model.operands.clone();
    let mut vm = BooleanViewModel::new(Box::new(model));

    operands.borrow_mut().push((LayerKey(4), "Nova".into()));
    vm.refresh();
    assert_eq!(
        vm.operands().get().len(),
        3,
        "a closed panel re-read the scene it is not showing"
    );

    vm.dispatch(&Command::ToggleBoolean);
    assert_eq!(
        vm.operands().get().len(),
        4,
        "opening the panel did not re-read what is in the scene"
    );
}

/// An operand the document will not take is refused when it is chosen, by
/// name, and the panel keeps the pair it had.
///
/// The audit's hierarchy was accepted by the panel, priced, shown as ready,
/// and refused a minute later by a run nobody heard. Refused at the choice,
/// the sentence arrives while the sculptor is still choosing.
#[test]
fn an_invalid_operand_is_refused_at_set() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeBooleans::new(calls.clone());
    model.inadmissible = vec![(
        LayerKey(3),
        BooleanRefusal::Hierarchy {
            operand: "Cabeça".into(),
        },
    )];
    let mut vm = BooleanViewModel::new(Box::new(model));
    vm.dispatch(&Command::ToggleBoolean);
    vm.dispatch(&Command::SetBoolean(a_pair(BooleanOp::Subtract)));
    let chosen = *vm.settings().get();

    vm.dispatch(&Command::SetBoolean(BooleanSettings {
        tool: Some(LayerKey(3)),
        ..chosen
    }));

    let notice = vm
        .notice()
        .get()
        .clone()
        .expect("a hierarchy chosen as an operand said nothing");
    assert!(
        notice.contains("Cabeça"),
        "the refusal named nobody: {notice}"
    );
    assert_eq!(
        *vm.settings().get(),
        chosen,
        "the refused operand was taken into the panel anyway"
    );

    vm.dispatch(&Command::RunBoolean);
    assert_eq!(
        calls.borrow().run[0].tool,
        Some(LayerKey(2)),
        "the run was over the refused operand rather than the pair left standing"
    );
}

/// The same subtool as base and tool is refused when it is chosen, rather than
/// silently leaving a panel with no pair.
#[test]
fn the_same_subtool_twice_is_refused_at_set() {
    let (mut vm, calls) = panel();
    vm.dispatch(&Command::SetBoolean(a_pair(BooleanOp::Union)));
    let priced = calls.borrow().priced;

    vm.dispatch(&Command::SetBoolean(BooleanSettings {
        tool: Some(LayerKey(1)),
        ..*vm.settings().get()
    }));

    assert!(
        vm.notice().get().is_some(),
        "choosing one subtool for both roles said nothing"
    );
    assert_eq!(vm.settings().get().tool, Some(LayerKey(2)));
    assert!(
        vm.is_ready(),
        "a refused choice took the standing pair away"
    );
    assert_eq!(
        calls.borrow().priced,
        priced,
        "a refused choice was priced as though it had been taken"
    );
}

/// A choice that is taken clears the refusal the last one raised, so the line
/// on the panel is about the pair on the panel.
#[test]
fn a_good_choice_clears_the_last_refusal() {
    let (mut vm, _) = panel();
    vm.dispatch(&Command::SetBoolean(BooleanSettings {
        base: Some(LayerKey(1)),
        tool: Some(LayerKey(1)),
        ..BooleanSettings::default()
    }));
    assert!(vm.notice().get().is_some());

    vm.dispatch(&Command::SetBoolean(a_pair(BooleanOp::Union)));
    assert!(
        vm.notice().get().is_none(),
        "a refusal about a pair nobody holds any more is still on the panel"
    );
    assert!(vm.is_ready());
}

/// A refused run reports on the notice channel every time it is asked, not
/// only the first: the door counts writes, and a second identical refusal that
/// did not write would read as a success.
#[test]
fn a_refused_boolean_reports() {
    let calls = Rc::new(RefCell::new(Calls::default()));
    let mut model = FakeBooleans::new(calls);
    model.refuse = Some(BooleanRefusal::Formless {
        operand: "Relevo".into(),
    });
    let mut vm = BooleanViewModel::new(Box::new(model));
    vm.dispatch(&Command::ToggleBoolean);
    vm.dispatch(&Command::SetBoolean(a_pair(BooleanOp::Subtract)));

    for attempt in 1..=2 {
        let before = vm.notice().occurrences();
        vm.dispatch(&Command::RunBoolean);
        assert!(
            vm.notice().occurrences() > before,
            "attempt {attempt} was refused without writing the channel the door reads"
        );
        let notice = vm.notice().get().clone().unwrap_or_default();
        assert!(notice.contains("Relevo"), "attempt {attempt}: {notice}");
    }
    assert_eq!(vm.take_result(), None);
}
