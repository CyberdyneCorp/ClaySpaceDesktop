//! The work this document owes itself between two interactions.
//!
//! A sculpt runtime accumulates jobs that make the *next* interaction cheaper
//! and *this* one slower — rebuilding a ray-query tree whose partition has
//! decayed under a drag's worth of refits being the one this application can
//! actually produce. None of it is correctness: a job declined, deferred
//! forever or done differently leaves the form exactly where it was. So none
//! of it is the engine's decision either, and [`claycore::MaintenanceQueue`]
//! holds *requests* rather than doing anything.
//!
//! What this module adds on top of that queue is the four decisions the
//! engine deliberately leaves to a host: when the work may happen, how much of
//! it a moment may afford, what it costs on this machine, and what must not be
//! given back while a pointer is down.
//!
//! # The gate is shut for the length of a gesture
//!
//! A pointer event is not a maintenance window. The queue refuses to hand
//! anything out while a stroke is open, which is a mechanism rather than a
//! convention — but the stroke this host means is a *drag*, and a drag opens
//! on a press and closes on a release that arrives as its own event several
//! frames later. So the gate is held by a [`claycore::StrokeScope`], which
//! owns the queue for as long as the gesture is up and shuts on `Drop`: a
//! gesture abandoned, unwound past, or taken down with the document leaves the
//! queue drainable rather than shut for the life of the process.
//!
//! [`State`] is that gate made total. Between gestures there is a
//! queue and it drains; during one there is a scope and it only folds; and if
//! the engine ever refuses to make a queue at all there is [`State::Absent`],
//! which is a document that sculpts exactly the same and simply never rebuilds
//! an index.
//!
//! # The budget is the host's, because the host holds the clock
//!
//! The drain is a take/complete pair rather than a callback for the reason the
//! C header gives: the budget loop is four lines in the caller's own language,
//! and the caller is the one that knows what moment it is. Here it is
//! [`Maintenance::BUDGET`], spent at the moment a gesture ends.
//!
//! # The estimate is measured on this machine, not guessed
//!
//! The engine carries no machine model and says so — a rebuild is O(triangles)
//! and nothing in the library turns that into microseconds. So the first
//! rebuild is filed with no estimate and *timed*, and every request after it
//! carries what this machine actually took. That is what makes the budget mean
//! something rather than being a number the loop reads and no item is ever
//! weighed against.
//!
//! # The pin a gesture holds
//!
//! A trim releases what is rebuildable, and what a trim costs the dab after it
//! is priced rather than asserted: the engine's own recovery benchmark reports
//! 0.62–2.04x at Warning and 13–182x at Critical, growing with the model. So a
//! trim that lands in the middle of a drag is the one place its cost is
//! certain to be paid by the artist rather than by an idle moment, and the
//! advice that falls out of those figures is to prefer Warning mid-drag or to
//! hold a pin until the stroke ends. [`claycore::MemoryPin`] is that pin: a
//! trim taken while it is held releases nothing and reports what it *would*
//! have released, so a memory warning stays honest without a surface going out
//! from under a gesture in flight.
//!
//! It is held here, beside the gate, because both are the same two moments and
//! binding them to the same pair of calls is what makes the release
//! unmissable. Every trim this document takes is handed
//! [`Maintenance::pin`], so a trim that ignored it would have to go out of its
//! way to; today there is no such trim, because this application holds neither
//! a hierarchy nor an adaptive surface and those are the two surfaces a trim
//! reaches. What the pin buys before then is that the first one cannot be
//! added without one, and that the balance is already measured.

use std::time::Duration;

use clayspace_model::LayerKey;

/// The queue, the gate, and what this document has measured about servicing
/// it.
pub(crate) struct Maintenance {
    state: State,
    /// What each mesh layer's ray-query tree scored when it was built.
    ///
    /// The engine's quality figure is only meaningful against the same tree's
    /// own history — never against another model's — so a rebuild is decided
    /// by how far a tree has drifted from where it started rather than by any
    /// absolute number. Written whenever a sculptor is built and whenever one
    /// is rebuilt, which are the two moments a tree is new.
    baselines: std::collections::BTreeMap<LayerKey, f32>,
    /// What an index rebuild cost this machine, the last time one was paid
    /// for. `None` until the first one, which is filed with no estimate
    /// precisely so that there is something to measure.
    rebuild_micros: Option<u64>,
    /// Held for the length of a gesture, so a trim arriving mid-drag reports
    /// rather than releases. `None` only where the engine refused to make one.
    pin: Option<claycore::MemoryPin>,
}

/// Where the queue is, which is also whether it may be drained.
enum State {
    /// Between gestures. Drainable.
    Between(claycore::MaintenanceQueue),
    /// A gesture is open: the gate is shut, and requests fold into it.
    InGesture(claycore::StrokeScope),
    /// The engine would not make a queue, or would not open the gate on one.
    ///
    /// Nothing here is correctness, so this is a resting place rather than a
    /// failure: the document sculpts the same and never rebuilds an index.
    Absent,
}

impl Maintenance {
    /// How long a drain may hold the interface thread.
    ///
    /// Chosen against the 16 ms the specification allows an engine operation
    /// to hold that thread — the bound `subtool.activate` is measured against
    /// — and halved, because the moment this runs at already pays for a
    /// surface refresh and a mip chain beside it. Eight milliseconds is half a
    /// frame at 60 Hz, spent where the pointer is up and nothing is being
    /// dragged, which is the only moment where a stall is the artist's to
    /// spend rather than a stutter in the middle of a stroke.
    pub(crate) const BUDGET: Duration = Duration::from_millis(8);

    /// How far a tree may drift from what it scored when it was built before a
    /// rebuild is worth asking for.
    ///
    /// Half again as many triangle tests as the tree started at. The bar is
    /// deliberately well clear of noise: the engine's own measurement is that
    /// a rebuild produced a better tree in one of five deformations and a
    /// dramatically worse one in two, so a marginal figure is not a reason to
    /// pay for one.
    const DECAY: f32 = 1.5;

    pub(crate) fn new() -> Self {
        Self {
            state: match claycore::MaintenanceQueue::new() {
                Ok(queue) => State::Between(queue),
                // Reported and carried on. A document that cannot keep a list
                // of optional work is a document that does the optional work
                // never, which is a supported way to run.
                Err(e) => {
                    eprintln!("a fila de manutenção não pôde ser criada: {e}");
                    State::Absent
                }
            },
            baselines: std::collections::BTreeMap::new(),
            rebuild_micros: None,
            pin: match claycore::MemoryPin::new() {
                Ok(pin) => Some(pin),
                Err(e) => {
                    eprintln!("o pino de memória não pôde ser criado: {e}");
                    None
                }
            },
        }
    }

    /// Opens the gate for a gesture, if it is not already open.
    ///
    /// Idempotent: a cage preview re-arms itself on every pointer move, and a
    /// gesture already up must not be reopened underneath the scope holding
    /// it.
    pub(crate) fn open_gesture(&mut self) {
        if matches!(self.state, State::InGesture(_)) {
            return;
        }
        self.hold_pin(true);
        self.state = match std::mem::replace(&mut self.state, State::Absent) {
            State::Between(queue) => match queue.into_stroke() {
                Ok(scope) => State::InGesture(scope),
                Err(e) => {
                    eprintln!("o portão da manutenção não pôde ser fechado: {e}");
                    State::Absent
                }
            },
            other => other,
        };
    }

    /// Shuts the gate. Idempotent, for the same reason as [`Self::open_gesture`].
    pub(crate) fn close_gesture(&mut self) {
        self.state = match std::mem::replace(&mut self.state, State::Absent) {
            State::InGesture(scope) => State::Between(scope.end()),
            other => other,
        };
        self.hold_pin(false);
    }

    /// Takes the pin, or gives it back — once either way.
    ///
    /// The engine's pin is a *count*, and a reentrant one, because a readback
    /// inside a save must not un-pin the save when it returns. A gesture is
    /// not that shape: it takes the pin once and gives it back once, and a
    /// press arriving over an unfinished drag must not leave it held forever.
    /// So the balance is kept here, against whether the pin is held, rather
    /// than against the gate — which would lose it on the one path where the
    /// gate refuses to shut and the pin has already been taken.
    fn hold_pin(&mut self, hold: bool) {
        let Some(pin) = self.pin.as_mut() else {
            return;
        };
        if pin.is_held() == hold {
            return;
        }
        let moved = if hold { pin.acquire() } else { pin.release() };
        if let Err(e) = moved {
            eprintln!("o pino de memória não pôde ser movido: {e}");
        }
    }

    /// Whether the gate is shut — which is to say, whether a drain would be
    /// refused.
    pub(crate) fn in_gesture(&self) -> bool {
        matches!(self.state, State::InGesture(_))
    }

    fn queue_mut(&mut self) -> Option<&mut claycore::MaintenanceQueue> {
        match &mut self.state {
            State::Between(queue) => Some(queue),
            State::InGesture(scope) => Some(scope),
            State::Absent => None,
        }
    }

    /// Queues a request, or folds it into the identical one already queued.
    ///
    /// Safe from a stamp: a drag asks for the same rebuild on every segment
    /// and the queue keeps one entry, counting the asking.
    pub(crate) fn request(&mut self, kind: claycore::MaintenanceKind, target: u32) {
        let estimate = match kind {
            // What this machine measured last time, or nothing at all, which
            // is what the first one is filed with so that there is a rebuild
            // to time. See the module note.
            claycore::MaintenanceKind::IndexRebuild => self.rebuild_micros.unwrap_or(0),
            _ => 0,
        };
        self.request_costing(kind, target, estimate);
    }

    /// The same, with an estimate the caller has of its own.
    ///
    /// Zero is "unknown", exactly as the engine means it, and is what most
    /// callers honestly have.
    pub(crate) fn request_costing(
        &mut self,
        kind: claycore::MaintenanceKind,
        target: u32,
        estimated_micros: u64,
    ) {
        let Some(queue) = self.queue_mut() else {
            return;
        };
        if let Err(e) = queue.request(kind, target, estimated_micros) {
            eprintln!("um pedido de manutenção foi recusado: {e}");
        }
    }

    /// What is queued, in queue order.
    ///
    /// Reads the queue rather than the gate, so it answers the same during a
    /// gesture as between two — which is what makes it useful for saying that
    /// a drag's worth of asking folded into one entry.
    pub(crate) fn queued(&self) -> Vec<claycore::MaintenanceItem> {
        match &self.state {
            State::Between(queue) => queue.items().unwrap_or_default(),
            State::InGesture(scope) => scope.items().unwrap_or_default(),
            State::Absent => Vec::new(),
        }
    }

    /// Takes the queue out for the length of a drain, and puts it back.
    ///
    /// The drain performs work against the rest of the document — a rebuild is
    /// a call on a sculptor the document holds — so the loop cannot run while
    /// this borrows the field it lives in. Taking it out is what lets the
    /// caller hold `&mut ClayDocument` for the body.
    pub(crate) fn take_for_drain(&mut self) -> Option<claycore::MaintenanceQueue> {
        match std::mem::replace(&mut self.state, State::Absent) {
            State::Between(queue) => Some(queue),
            // The gate. A gesture is open, so there is nothing a drain may do
            // and the scope stays exactly where it was.
            other => {
                self.state = other;
                None
            }
        }
    }

    pub(crate) fn put_back(&mut self, queue: claycore::MaintenanceQueue) {
        self.state = State::Between(queue);
    }

    /// Whether an item may be started with `left` of the budget remaining.
    ///
    /// An item with no estimate is started once — that is how this host learns
    /// what one costs. An item that has been measured is weighed, and one that
    /// does not fit stops the drain rather than being skipped: `take_next`
    /// hands out the head of the queue, so a loop that stepped past it would
    /// ask for the same item again and spin.
    pub(crate) fn affordable(item: &claycore::MaintenanceItem, left: Duration) -> bool {
        !left.is_zero() && Duration::from_micros(item.estimated_micros) <= left
    }

    /// Records what a tree scored the moment it was built or rebuilt.
    pub(crate) fn note_baseline(&mut self, layer: LayerKey, quality: f32) {
        self.baselines.insert(layer, quality);
    }

    /// Whether a tree has drifted far enough from its own starting figure to
    /// be worth rebuilding.
    ///
    /// `false` where nothing was recorded, which is the honest answer rather
    /// than a cautious one: with no figure to compare against there is no
    /// drift, and the engine is explicit that the number means nothing against
    /// another tree.
    pub(crate) fn has_decayed(&self, layer: LayerKey, quality: f32) -> bool {
        self.baselines
            .get(&layer)
            .is_some_and(|built| quality > built * Self::DECAY)
    }

    /// Keeps only the figures of layers the document still holds.
    ///
    /// A stale figure is harmless to a rebuild — one is only read where a
    /// sculptor is still there, and building one writes a fresh figure over
    /// the old — so this is about the map staying the size of the document
    /// rather than the size of its history.
    pub(crate) fn retain_baselines(&mut self, keep: impl Fn(LayerKey) -> bool) {
        self.baselines.retain(|layer, _| keep(*layer));
    }

    /// Takes this machine's figure for a rebuild, so the next request can be
    /// weighed against the budget instead of guessed at.
    pub(crate) fn note_rebuild_cost(&mut self, took: Duration) {
        self.rebuild_micros = Some(took.as_micros().min(u128::from(u64::MAX)) as u64);
    }

    /// What a rebuild has been measured to cost here, or `None` before the
    /// first one.
    pub(crate) fn measured_rebuild_micros(&self) -> Option<u64> {
        self.rebuild_micros
    }

    /// The pin every trim this document takes is handed.
    ///
    /// `None` only where the engine refused to make one, which a trim reads as
    /// "unpinned" and is the same answer as not asking.
    pub(crate) fn pin(&self) -> Option<&claycore::MemoryPin> {
        self.pin.as_ref()
    }

    /// Whether the pin is held, which is the same question as whether a
    /// gesture is open — and the reason it is worth asking separately is that
    /// the two are only the same while the balance holds.
    pub(crate) fn is_pinned(&self) -> bool {
        self.pin.as_ref().is_some_and(claycore::MemoryPin::is_held)
    }
}

impl std::fmt::Debug for Maintenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Maintenance")
            .field("in_gesture", &self.in_gesture())
            .field("queued", &self.queued().len())
            .field("rebuild_micros", &self.rebuild_micros)
            .field("pinned", &self.is_pinned())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The engine carries no machine model and says so, so what a rebuild
    /// costs here is learned by paying for one: the first request is filed
    /// with no estimate, and every one after it carries what was measured.
    #[test]
    fn a_rebuild_is_priced_by_having_been_paid_for_once() {
        let mut maintenance = Maintenance::new();
        assert_eq!(maintenance.measured_rebuild_micros(), None);

        maintenance.request(claycore::MaintenanceKind::IndexRebuild, 1);
        assert_eq!(
            maintenance.queued().first().map(|i| i.estimated_micros),
            Some(0),
            "the first rebuild was filed with a figure nobody had measured"
        );

        maintenance.note_rebuild_cost(Duration::from_micros(41_000));
        maintenance.request(claycore::MaintenanceKind::IndexRebuild, 2);
        assert_eq!(
            maintenance
                .queued()
                .iter()
                .find(|i| i.target == 2)
                .map(|i| i.estimated_micros),
            Some(41_000),
            "the second rebuild was still filed as unknown"
        );
    }

    /// The budget weighs an estimate and lets an unmeasured item through
    /// once — which is the only overrun the loop can produce, and is how a
    /// host learns what to weigh next time.
    #[test]
    fn an_unmeasured_item_is_started_and_a_measured_one_is_weighed() {
        let unknown = |micros| claycore::MaintenanceItem {
            kind: claycore::MaintenanceKind::IndexRebuild,
            target: 0,
            requests: 1,
            estimated_micros: micros,
        };
        let budget = Duration::from_millis(8);

        assert!(Maintenance::affordable(&unknown(0), budget));
        assert!(Maintenance::affordable(&unknown(8_000), budget));
        assert!(!Maintenance::affordable(&unknown(8_001), budget));
        assert!(
            !Maintenance::affordable(&unknown(0), Duration::ZERO),
            "a moment with nothing left started work anyway"
        );
    }

    /// A tree is measured against its own history and never against a number.
    #[test]
    fn drift_is_read_against_what_the_same_tree_scored_when_it_was_built() {
        let mut maintenance = Maintenance::new();
        let layer = LayerKey(3);
        assert!(
            !maintenance.has_decayed(layer, 9_000.0),
            "a tree with no history of its own was called decayed on a figure \
             that means nothing without one"
        );

        maintenance.note_baseline(layer, 10.0);
        assert!(!maintenance.has_decayed(layer, 14.9));
        assert!(maintenance.has_decayed(layer, 15.1));

        maintenance.retain_baselines(|held| held != layer);
        assert!(!maintenance.has_decayed(layer, 15.1));
    }

    /// The gate, and the pin that follows it exactly.
    #[test]
    fn a_gesture_shuts_the_gate_and_takes_the_pin_and_both_come_back() {
        let mut maintenance = Maintenance::new();
        assert!(!maintenance.in_gesture());
        assert!(!maintenance.is_pinned());
        assert!(maintenance.take_for_drain().is_some());
        maintenance.put_back(claycore::MaintenanceQueue::new().expect("queue"));

        maintenance.open_gesture();
        assert!(maintenance.in_gesture());
        assert!(maintenance.is_pinned());
        assert!(
            maintenance.take_for_drain().is_none(),
            "the gate handed the queue out with a gesture open"
        );
        // A lost pointer release, then a press: the pin is a count, so the
        // second open must not leave one held after the close.
        maintenance.open_gesture();

        maintenance.close_gesture();
        assert!(!maintenance.in_gesture());
        assert!(!maintenance.is_pinned());
        // And shutting one that was never open takes nothing away.
        maintenance.close_gesture();
        assert!(!maintenance.is_pinned());
        assert!(maintenance.take_for_drain().is_some());
    }
}
