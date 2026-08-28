//! The boolean panel: two subtools, an operation, a resolution and a price.
//!
//! Its own ViewModel rather than more state on [`crate::ObjectViewModel`],
//! for the reason the manipulator is kept apart from the brush: a boolean is
//! not a thing the sculptor does to the clay under the pointer, it is a
//! decision made about two forms with the cost in view — and nothing about it
//! reaches the document until it is confirmed.
//!
//! The cost is recomputed from the settings on every change rather than
//! carried beside them, so the figures on screen and the operation the button
//! would run cannot come to disagree.

use clayspace_model::{BooleanSettings, Cost, LayerKey, ObjectModel};

use crate::command::Command;
use crate::observable::Observable;

pub struct BooleanViewModel {
    model: Box<dyn ObjectModel>,
    /// Whether the panel is open.
    open: Observable<bool>,
    /// The operands, the operation and the resolution.
    settings: Observable<BooleanSettings>,
    /// The subtools that could take part, and what they are called.
    operands: Observable<Vec<(LayerKey, String)>>,
    /// What the chosen pair would cost at the chosen resolution.
    cost: Observable<Option<Cost>>,
    /// Why the last attempt was refused, when it was.
    notice: Observable<Option<String>>,
    /// The subtool the last boolean left.
    ///
    /// Held so the composition root can put the manipulator on it: the result
    /// is a form to stand somewhere, exactly as an inserted one is.
    result: Option<LayerKey>,
}

impl BooleanViewModel {
    pub fn new(mut model: Box<dyn ObjectModel>) -> Self {
        let operands = model.boolean_operands();
        Self {
            model,
            open: Observable::new(false),
            settings: Observable::new(BooleanSettings::default()),
            operands: Observable::new(operands),
            cost: Observable::new(None),
            notice: Observable::new(None),
            result: None,
        }
    }

    pub fn open(&self) -> &Observable<bool> {
        &self.open
    }

    pub fn settings(&self) -> &Observable<BooleanSettings> {
        &self.settings
    }

    pub fn operands(&self) -> &Observable<Vec<(LayerKey, String)>> {
        &self.operands
    }

    /// What the chosen pair would cost — the crossing figures the conversion
    /// panel already states, for the same kind of crossing.
    pub fn cost(&self) -> &Observable<Option<Cost>> {
        &self.cost
    }

    pub fn notice(&self) -> &Observable<Option<String>> {
        &self.notice
    }

    /// The subtool the last boolean left, taken once.
    ///
    /// Taken rather than read, because it is a thing that *happened*: the
    /// composition root acts on it in the frame it arrives and must not act on
    /// it again in the next one.
    pub fn take_result(&mut self) -> Option<LayerKey> {
        self.result.take()
    }

    /// Whether there is a pair to run at all.
    ///
    /// What the confirm button reads. A panel half filled in offers nothing to
    /// press rather than a button that can only be refused.
    pub fn is_ready(&self) -> bool {
        self.settings.get().pair().is_some()
    }

    pub fn dispatch(&mut self, command: &Command) {
        match command {
            Command::ToggleBoolean => {
                let open = !*self.open.get();
                self.open.set(open);
                if open {
                    // What is in the scene may have changed since it was last
                    // looked at, and a panel offering a subtool that has gone
                    // is a panel offering a refusal.
                    self.reread();
                    // And the refusal the *last* attempt raised goes with the
                    // panel it was raised in: a sentence about a ghosted
                    // cylinder outliving the visit it belongs to reads as a
                    // refusal of whatever is chosen next.
                    self.notice.set_if_changed(None);
                }
            }
            Command::SetBoolean(settings) => self.settle(*settings),
            Command::RunBoolean => self.run(),
            _ => {}
        }
    }

    /// Re-reads what could take part, when there is a panel to read it into.
    ///
    /// Guarded on the panel being open because the composition root calls this
    /// after every edit, and answering it is not free: asking what could take
    /// part measures every subtool's extent, and a mesh subtool's is its
    /// triangles. A panel nobody is looking at must not put that on every dab.
    pub fn refresh(&mut self) {
        if *self.open.get() {
            self.reread();
        }
    }

    /// The re-read itself, for the moments that need it whether or not the
    /// panel is open: opening it, and finishing a boolean.
    fn reread(&mut self) {
        let operands = self.model.boolean_operands();
        let mut settings = *self.settings.get();
        let still_there = |key: Option<LayerKey>| {
            key.filter(|key| operands.iter().any(|(candidate, _)| candidate == key))
        };
        settings.base = still_there(settings.base);
        settings.tool = still_there(settings.tool);
        self.operands.set_if_changed(operands);
        // Through `settle` rather than set directly, so a price quoted for a
        // subtool nobody can see any more goes with it.
        self.settle(settings);
    }

    /// Takes the panel's new settings and re-prices them.
    ///
    /// A newly chosen pair brings its own resolution with it — the
    /// specification asks for a default that "follows the operands' own detail
    /// rather than a fixed constant" — and after that the number is the
    /// sculptor's, which is why it is only re-derived when the pair changes.
    fn settle(&mut self, settings: BooleanSettings) {
        let was = self.settings.get().pair();
        let mut settings = settings.sanitized();
        if settings.pair() != was {
            if let Some((base, tool)) = settings.pair() {
                if let Some(cell) = self.model.boolean_cell(base, tool) {
                    settings.cell_size = cell;
                }
            }
            settings = settings.sanitized();
        }
        let cost = self.model.boolean_cost(settings);
        self.settings.set_if_changed(settings);
        self.cost.set(cost);
    }

    /// Runs the boolean the panel is set to. The consent, not the choosing.
    fn run(&mut self) {
        let settings = *self.settings.get();
        match self.model.run_boolean(settings) {
            Ok(inserted) => {
                self.notice.set_if_changed(None);
                self.result = Some(inserted.layer);
                // The operands are hidden or gone and a subtool has arrived,
                // so what the panel offers is not what it offered a moment
                // ago. The chosen pair goes with them: pressing the button
                // twice would otherwise run the same boolean over operands
                // that are no longer in the scene.
                self.settings.set(BooleanSettings {
                    base: None,
                    tool: None,
                    ..settings
                });
                self.open.set(false);
                self.reread();
            }
            Err(e) => self.notice.set(Some(e.to_string())),
        }
    }
}
