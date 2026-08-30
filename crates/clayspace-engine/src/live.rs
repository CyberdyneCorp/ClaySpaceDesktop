//! The live half of the field brushes.
//!
//! Suavizar and Relaxar *bake*: the engine's relax takes a sampled volume and
//! returns another, so a stroke split into segments stacks a replacement per
//! segment until the surface crumbles. That is why they were held whole and
//! arrived only when the pointer came up — the sculptor smoothed blind.
//!
//! ClayCore 0.60.0's transaction holds the volume between pointer events
//! instead: the layer is sampled **once**, every dab relaxes that retained
//! volume in place, and the commit installs the volume the dabs were applied
//! to. Between begin and commit the document does not change at all.
//!
//! ## Drawing what has not been written down
//!
//! The transaction hands over the bricks whose samples are new, which is not a
//! picture. Turning them into one without a second implementation of the field
//! is the whole problem this module solves, and it solves it by **relabelling
//! rather than resampling**.
//!
//! The preview's lattice has the spacing we asked for and an origin of its own
//! — the layer's bounds, less the padding — which does not land on the brick
//! cache's lattice and cannot be made to: one `padding` cannot align three
//! axes whose bounds have different remainders. Interpolating onto the cache's
//! lattice would put our arithmetic between the engine and the surface, which
//! `docs/architecture.md` exists to prevent.
//!
//! So the preview keeps its own cache, and preview brick `K` is stored as that
//! cache's brick `K`. The two lattices are then the same lattice in a world
//! translated by the preview's origin, and the translation is undone on the
//! vertices the engine meshes. Nothing is interpolated and nothing is
//! averaged: the samples that reach the mesher are the samples the transaction
//! computed.

use claycore::{
    BrickCache, BrickConfig, BrickKey, BrickSubmit, Document, LayerId, MoveParams, MoveTransaction,
    RelaxParams, SculptPolicy, SmoothTransaction,
};
use clayspace_model::ModelError;

/// The surface the viewport draws while a live gesture is running.
pub struct LiveSurface<'a> {
    /// Meshed instead of the document's own cache.
    pub cache: &'a BrickCache,
    /// Added to every vertex the mesher produces, which is what puts the
    /// relabelled lattice back where the sculptor is looking.
    pub offset: [f32; 3],
}

/// A live Smooth in progress, and the surface it is drawing.
pub(crate) struct LiveSmooth {
    transaction: SmoothTransaction,
    /// The preview's own lattice, held as a cache so the *engine* meshes it.
    cache: BrickCache,
    /// Where the preview lattice's brick `[0, 0, 0]` starts, in the world.
    ///
    /// Learned from the first delta rather than assumed: every brick reports
    /// its own origin, and they all agree on this one.
    offset: Option<[f32; 3]>,
    /// Preview keys the viewport has not drawn yet.
    dirty: Vec<BrickKey>,
    brick_span: f32,
    config: BrickConfig,
}

impl LiveSmooth {
    /// Opens a gesture, or reports why it cannot be live.
    pub(crate) fn begin(
        document: &mut Document,
        layer: LayerId,
        config: BrickConfig,
    ) -> Result<Self, ModelError> {
        let transaction =
            SmoothTransaction::begin(document, layer, SculptPolicy::at(config.voxel_size))
                .map_err(ModelError::engine)?;
        let cache = BrickCache::new(config).map_err(ModelError::engine)?;
        let mut live = Self {
            transaction,
            cache,
            offset: None,
            dirty: Vec::new(),
            brick_span: config.voxel_size * config.dim as f32,
            config,
        };
        live.prime()?;
        Ok(live)
    }

    /// Fills the preview cache with the whole surface, once.
    ///
    /// The delta reports the bricks whose bytes are *new*, which after the
    /// first dab is the dab's neighbourhood and never the rest of the form —
    /// so a preview cache fed only by dabs holds a patch, and a viewport
    /// drawing it would show a patch of a sculpture floating on its own.
    ///
    /// A pass of zero strength over the whole volume is how the whole of it is
    /// asked for: `region_radius` of zero relaxes everywhere, and a strength
    /// of zero moves no sample — the engine reports `changed: false` and hands
    /// over every brick. This is the pointer-down cost, paid once, and it buys
    /// a preview that is the whole form from the first frame.
    fn prime(&mut self) -> Result<(), ModelError> {
        self.transaction
            .update(RelaxParams {
                strength: 0.0,
                radius_cells: 1,
                iterations: 1,
                centre: [0.0; 3],
                region_radius: 0.0,
                falloff: 0.0,
                mask: None,
            })
            .map_err(ModelError::engine)?;
        self.absorb()?;
        // Primed, not drawn: nothing has been asked for yet, so the keys this
        // filled are the surface's starting state rather than an edit to it.
        self.dirty.clear();
        Ok(())
    }

    /// The surface to draw.
    ///
    /// `None` only before the preview has a lattice to be drawn in, which
    /// priming gives it — so in practice, only if the layer had nothing in it.
    pub(crate) fn surface(&self) -> Option<LiveSurface<'_>> {
        self.offset.map(|offset| LiveSurface {
            cache: &self.cache,
            offset,
        })
    }

    pub(crate) fn take_dirty(&mut self) -> Vec<BrickKey> {
        std::mem::take(&mut self.dirty)
    }

    /// One dab, and the preview it leaves behind.
    pub(crate) fn dab(&mut self, params: RelaxParams<'_>) -> Result<usize, ModelError> {
        self.transaction
            .update(params)
            .map_err(ModelError::engine)?;
        self.absorb()
    }

    /// Moves the bricks the transaction says are new into the preview cache.
    fn absorb(&mut self) -> Result<usize, ModelError> {
        let Some(delta) = self
            .transaction
            .take_preview()
            .map_err(ModelError::engine)?
        else {
            return Ok(0);
        };
        let Some(first) = delta.bricks.first() else {
            return Ok(0);
        };
        // Every brick reports an origin, and they agree on where the lattice
        // starts; taking it from one is taking it from all of them.
        self.offset.get_or_insert(std::array::from_fn(|axis| {
            first.origin[axis] - first.key[axis] as f32 * self.brick_span
        }));

        let mut wanted = std::collections::HashMap::with_capacity(delta.bricks.len());
        for (at, brick) in delta.bricks.iter().enumerate() {
            wanted.insert(brick.key, at);
            self.mark(brick.key)?;
        }

        self.drain_into_cache(&delta, &wanted)
    }

    /// Moves every brick the delta has samples for out of the dirty pool.
    ///
    /// Drained to the bottom, in batches, because marking is not exact: the
    /// cache marks every brick a region *reaches*, band included, so asking
    /// for one brick dirties its neighbours too. Taking a bounded slice of
    /// that pool once returned mostly bricks the delta had no samples for and
    /// left the ones it did have waiting — the preview filled a fraction of
    /// the form and then stopped moving.
    ///
    /// A brick taken and not submitted keeps whatever it already held, which
    /// for a neighbour dragged in by the band is exactly right: this gesture
    /// has not changed it.
    fn drain_into_cache(
        &mut self,
        delta: &claycore::PreviewDelta,
        wanted: &std::collections::HashMap<BrickKey, usize>,
    ) -> Result<usize, ModelError> {
        let mut accepted = 0;
        loop {
            let (requests, remaining) = self
                .cache
                .take_dirty(Self::DRAIN_BATCH)
                .map_err(ModelError::engine)?;
            if requests.is_empty() {
                break;
            }
            accepted += self.submit_round(delta, wanted, requests)?;
            if remaining == 0 {
                break;
            }
        }
        Ok(accepted)
    }

    /// One batch: fill the samples for the bricks this delta knows about, and
    /// record which the cache accepted.
    fn submit_round(
        &mut self,
        delta: &claycore::PreviewDelta,
        wanted: &std::collections::HashMap<BrickKey, usize>,
        requests: Vec<claycore::BrickRequest>,
    ) -> Result<usize, ModelError> {
        let mut submitted = Vec::with_capacity(requests.len());
        let mut values = Vec::with_capacity(requests.len() * self.samples_per_brick());
        for request in requests {
            let Some(&at) = wanted.get(&request.key()) else {
                continue;
            };
            self.strip_halo(&delta.bricks[at], delta, &mut values);
            submitted.push(request);
        }
        let outcomes = self
            .cache
            .submit(&submitted, &values, None)
            .map_err(ModelError::engine)?;
        let mut accepted = 0;
        for (request, outcome) in submitted.iter().zip(outcomes) {
            if outcome == BrickSubmit::Accepted {
                self.dirty.push(request.key());
                accepted += 1;
            }
        }
        Ok(accepted)
    }

    /// Marks exactly one brick, by naming a box well inside it.
    ///
    /// The cache marks every brick a region *intersects*, so a box on the
    /// brick's own bounds would take its neighbours with it — and a neighbour
    /// marked without being submitted stays dirty for the rest of the gesture.
    fn mark(&mut self, key: BrickKey) -> Result<(), ModelError> {
        let low: [f32; 3] = std::array::from_fn(|axis| (key[axis] as f32 + 0.25) * self.brick_span);
        let high: [f32; 3] = std::array::from_fn(|axis| low[axis] + self.brick_span * 0.5);
        self.cache.mark_dirty(low, high).map_err(ModelError::engine)
    }

    /// Copies a preview brick's interior into the cache's own layout.
    ///
    /// The preview stores `dim + 1` samples per axis — the brick's own cells
    /// plus the boundary sample it shares with its neighbour — and the cache
    /// stores `dim`. The shared sample arrives again in the neighbour's record,
    /// so dropping it here loses nothing.
    fn strip_halo(
        &self,
        brick: &claycore::PreviewBrick,
        delta: &claycore::PreviewDelta,
        into: &mut Vec<f32>,
    ) {
        let samples = brick.samples(delta);
        let stride = brick.sample_dim as usize;
        let dim = self.config.dim as usize;
        for z in 0..dim {
            for y in 0..dim {
                let row = (z * stride + y) * stride;
                into.extend_from_slice(&samples[row..row + dim]);
            }
        }
    }

    /// How many bricks are drained per round. The application's own refill
    /// uses the same batch, for the same reason: a round is a submission.
    const DRAIN_BATCH: usize = 512;

    fn samples_per_brick(&self) -> usize {
        (self.config.dim as usize).pow(3)
    }
}

// -- Move --------------------------------------------------------------------

/// A world-space box to re-fill, or nothing where the drag reached nothing.
///
/// Named because it is the whole vocabulary between [`LiveMove`] and the
/// document that owns the brick cache: a live drag never hands over samples,
/// only where to go and read them.
pub(crate) type Region = Option<([f32; 3], [f32; 3])>;

/// A live Move drag in progress.
///
/// Move degrades the field by a second mechanism, and the one `LiveSmooth`
/// answers does not describe it. A drag appends a `grab` to the deformer chain
/// of every item it reaches, and the engine's Lipschitz bound for a chain is
/// the *product* of its links — so the safe step scale decays by a constant
/// factor per drag and the marcher's cost rises with it. Measured on the
/// application's own starting form, twelve segmented drags took the step scale
/// from 0.264 to below the float's ability to report it, and a dab from 5.2 ms
/// to 26 ms.
///
/// The transaction is the engine's answer: the edit list is walked **once**, at
/// begin, every frame after that costs only the items the drag moves, and the
/// commit writes one grab per item however many frames drew it —
/// `a_drag_collapses_to_one_grab_where_segments_leave_one_each` holds it to
/// that.
///
/// ## Drawing a drag the document does not carry
///
/// Between begin and commit the document is untouched, which is what makes the
/// transaction cheap and also means there is nothing to mesh: `LiveSmooth`
/// draws its preview from the bricks the transaction hands over, and a Move
/// transaction hands over no samples at all. ClayCore's C++ class exposes a
/// `preview_layer()` for exactly this — a private copy of the layer with the
/// affected chains replaced, "so the host compiles, draws and picks it exactly
/// as it does the real one" — but the **C ABI does not carry it**, and this
/// crate can only reach the C ABI. See `docs/roadmap.md`, under *Known costs
/// and escape routes*, for the ask.
///
/// What the ABI does offer is the resolved grabs, `clay_sdf_move_preview_grab`,
/// "so a host can reproduce the preview through machinery it already has". So
/// that is what this does, once per segment:
///
///   1. ask the transaction for the grabs the *current total* displacement
///      resolves to, and write them onto the layer;
///   2. let the caller re-fill the brick cache, which samples the dragged
///      surface out of the document;
///   3. take those grabs straight back off it, leaving the document exactly as
///      the transaction left it at begin.
///
/// Step 3 is what makes the commit legal. A commit re-checks a stamp derived
/// from the layer's *content* and refuses a layer that moved underneath it, so
/// the preview has to be gone before the commit — `undo` restores the content
/// byte for byte and the stamp with it, which
/// `a_preview_grab_can_be_drawn_and_taken_back_under_an_open_drag` asserts
/// rather than assumes.
///
/// The cache keeps what the document gave up. Nothing re-fills a brick until
/// something marks it dirty, so the dragged samples stay in the cache and on
/// screen after step 4 — the same trade `LiveSmooth` makes with its own
/// lattice, reached a different way.
pub(crate) struct LiveMove {
    transaction: MoveTransaction,
    layer: LayerId,
    /// Where the drag was anchored. Every update is measured from here: the
    /// engine takes the **total**, never an increment, and a composition of
    /// increments moves the surface further than the drag ever asked for.
    anchor: [f32; 3],
    /// How many preview grabs are on the undo stack waiting to be taken back.
    drawn: usize,
    /// The box the last preview dirtied, so the next one can clear it: a drag
    /// that moves on leaves the surface it vacated to be re-filled from the
    /// document, and only this remembers where that was.
    previewed: Region,
}

impl LiveMove {
    pub(crate) fn begin(
        document: &mut Document,
        layer: LayerId,
        anchor: [f32; 3],
        params: MoveParams,
    ) -> Result<Self, ModelError> {
        let transaction = MoveTransaction::begin(document, layer, anchor, params, None)
            .map_err(ModelError::engine)?;
        Ok(Self {
            transaction,
            layer,
            anchor,
            drawn: 0,
            previewed: None,
        })
    }

    pub(crate) fn anchor(&self) -> [f32; 3] {
        self.anchor
    }

    /// Advances the drag to `position` and draws the preview onto the layer.
    ///
    /// Returns the box to re-fill: the union of where the last preview was and
    /// where this one is, so the clay the drag has moved off is restored from
    /// the document in the same pass that draws where it moved to.
    ///
    /// The caller must re-fill that box and then call [`Self::settle`], in that
    /// order. Between the two the layer carries the drag and the brick cache
    /// does not; after them the cache carries it and the layer does not, which
    /// is the state the whole gesture is spent in.
    pub(crate) fn drag(
        &mut self,
        document: &mut Document,
        position: [f32; 3],
    ) -> Result<Region, ModelError> {
        let total = std::array::from_fn(|axis| position[axis] - self.anchor[axis]);
        let dirty = self.transaction.update(total).map_err(ModelError::engine)?;
        self.draw(document)?;
        let region = match (self.previewed, dirty.bounds) {
            (Some(was), Some(now)) => Some(union(was, now)),
            (some, None) | (None, some) => some,
        };
        self.previewed = dirty.bounds;
        Ok(region)
    }

    /// Takes the preview back off the layer, once the cache has read it.
    ///
    /// Called at the end of every segment rather than at the start of the next
    /// one, so that a segment leaves the engine's undo depth exactly where it
    /// found it. The ViewModel counts a live segment's history by that
    /// difference; a segment that left its preview behind would be counted as
    /// having written it, and cancelling the gesture would then spend one undo
    /// per segment against history the gesture never made.
    pub(crate) fn settle(&mut self, document: &mut Document) -> Result<(), ModelError> {
        self.take_back(document)
    }

    /// Writes the current total's grabs onto the layer.
    fn draw(&mut self, document: &mut Document) -> Result<(), ModelError> {
        for node in self.transaction.reached().map_err(ModelError::engine)? {
            // One grab under no symmetry, and one per image of the drag that
            // reaches the node under a mirror — a straddler takes the ball's
            // grab and its reflection's, and drawing only the first would
            // preview half the drag.
            for grab in self.transaction.grabs(node).map_err(ModelError::engine)? {
                document
                    .add_grab(self.layer, node, grab)
                    .map_err(ModelError::engine)?;
                self.drawn += 1;
            }
        }
        Ok(())
    }

    /// Takes the drawn grabs back off the layer.
    ///
    /// Through the undo stack, which is the only door the C ABI has: there is
    /// no remove-deformer call. Each grab was its own entry, so this spends
    /// exactly as many as it wrote.
    fn take_back(&mut self, document: &mut Document) -> Result<(), ModelError> {
        for _ in 0..std::mem::take(&mut self.drawn) {
            document.undo().map_err(ModelError::engine)?;
        }
        Ok(())
    }

    /// Installs the drag as one grab per item, in one undo step.
    ///
    /// Returns the entries the commit recorded and the box to re-fill, which is
    /// where the last preview stood: the cache holds the drag there and the
    /// document now holds it too, so one pass makes them agree.
    pub(crate) fn commit(mut self, document: &mut Document) -> Result<(usize, Region), ModelError> {
        // Owed even though every settled segment left nothing behind: a
        // gesture whose last segment failed between `drag` and `settle` has a
        // preview on the layer, and a commit refuses a layer that changed
        // since begin.
        self.take_back(document)?;
        let before = depth(document);
        self.transaction.commit().map_err(ModelError::engine)?;
        let recorded = depth(document).saturating_sub(before);
        Ok((recorded, self.previewed))
    }

    /// Abandons the drag, leaving the document as it was before it began.
    ///
    /// Returns the box the preview occupied, which still has to be re-filled:
    /// the document never carried the drag but the cache does.
    pub(crate) fn cancel(mut self, document: &mut Document) -> Result<Region, ModelError> {
        self.take_back(document)?;
        // Dropping the transaction is a cancel; being explicit says so.
        self.transaction.cancel();
        Ok(self.previewed)
    }
}

fn depth(document: &Document) -> usize {
    document
        .undo_state()
        .map(|state| state.undo_depth)
        .unwrap_or(0)
}

fn union(a: ([f32; 3], [f32; 3]), b: ([f32; 3], [f32; 3])) -> ([f32; 3], [f32; 3]) {
    (
        std::array::from_fn(|axis| a.0[axis].min(b.0[axis])),
        std::array::from_fn(|axis| a.1[axis].max(b.1[axis])),
    )
}
