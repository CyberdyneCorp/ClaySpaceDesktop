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
//!
//! ## Drawing the rest of the document beside it
//!
//! A transaction previews **one layer**, and the viewport meshes the preview
//! instead of the document's own cache — so for as long as a second field
//! subtool was visible, everything but the layer being smoothed would have
//! vanished for the length of the drag. That is why the gesture used to refuse
//! to be live at all with more than one visible field subtool, and it was
//! filed upstream as ClayCore#378.
//!
//! ClayCore 0.78.0 answers it. `clay_brick_cache_eval_requests_excluding` is
//! the document evaluated over every visible SDF layer *except* one, which is
//! precisely the other half of what the preview holds. The engine composes
//! visible field layers by a hard union, so the composition is an elementwise
//! minimum and it is exact: nothing is blended and there is no seam.
//!
//! Two properties make it usable where the obvious route is not. It **edits
//! nothing and records no undo entry**, so it is legal between
//! `clay_sdf_smooth_begin` and its commit — where hiding the other layers,
//! sampling, and showing them again is three edits, and the commit correctly
//! refuses a layer that moved since it began. And it takes no seed and leaves
//! none, so it is priced like a stroke's first dab; the layers it excludes do
//! not move while the artist drags, so the whole-form pass is paid once at
//! pointer-down and every frame after it costs only the dab's own bricks.

use std::collections::HashSet;

use claycore::{
    Backend, BrickCache, BrickConfig, BrickKey, BrickRequest, BrickSubmit, Document, LayerId,
    MoveParams, MoveTransaction, RelaxParams, SculptPolicy, SmoothTransaction,
};
use clayspace_model::ModelError;

/// What a preview has to be composed with, where the document holds more than
/// the layer being previewed.
///
/// `None` on a document whose only visible field subtool is the one under the
/// brush, which is the common case and the one every brush figure is measured
/// on: there is nothing to compose, and the gesture runs exactly the path it
/// ran before this existed.
pub(crate) struct Rest {
    /// The layer the transaction holds. It is left out of every evaluation of
    /// the rest because it *is* the preview.
    excluded: LayerId,
    /// Where the other visible field subtools stand, in the world.
    ///
    /// The preview's own lattice covers the layer being smoothed and nothing
    /// else, so a subtool standing beside it falls outside every brick the
    /// transaction reports. These boxes are what the preview cache is widened
    /// to cover, once, at pointer-down.
    bounds: Vec<([f32; 3], [f32; 3])>,
    /// Where an evaluation runs. Chosen once, for a batch the size of the
    /// pointer-down pass, because that pass is what dominates: a dab's
    /// composition is a couple of dozen bricks and the whole form is a
    /// thousand.
    backend: Option<Backend>,
    /// Preview keys the transaction covers.
    ///
    /// The second pass widens the lattice over the *other* subtools, and a box
    /// that overlaps this one would otherwise re-fill a brick that already
    /// holds a composed sample with the rest of the document alone — which is
    /// the layer being smoothed disappearing from exactly where it is.
    covered: HashSet<BrickKey>,
}

impl Rest {
    pub(crate) fn new(
        excluded: LayerId,
        bounds: Vec<([f32; 3], [f32; 3])>,
        backend: Option<Backend>,
    ) -> Self {
        Self {
            excluded,
            bounds,
            backend,
            covered: HashSet::new(),
        }
    }
}

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
    /// The document beside the layer being previewed, where there is one.
    rest: Option<Rest>,
    brick_span: f32,
    config: BrickConfig,
}

impl LiveSmooth {
    /// Opens a gesture, or reports why it cannot be live.
    pub(crate) fn begin(
        document: &mut Document,
        layer: LayerId,
        config: BrickConfig,
        rest: Option<Rest>,
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
            rest,
            brick_span: config.voxel_size * config.dim as f32,
            config,
        };
        // Read rather than written: the transaction is open on `document` and
        // the evaluations below only ask it questions.
        live.prime(document)?;
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
    fn prime(&mut self, document: &Document) -> Result<(), ModelError> {
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
        self.absorb(document)?;
        self.widen_over_the_rest(document)?;
        // Primed, not drawn: nothing has been asked for yet, so the keys this
        // filled are the surface's starting state rather than an edit to it.
        self.dirty.clear();
        Ok(())
    }

    /// Covers the other visible field subtools, once, at pointer-down.
    ///
    /// The pass above fills the bricks the *transaction* reports, which are
    /// the layer being smoothed and nothing else — so a second subtool
    /// standing beside it has no brick in this lattice at all and would simply
    /// not be drawn. This marks where those subtools stand and fills what it
    /// drains from the document without the layer under the brush.
    ///
    /// A brick the transaction already covers is dropped rather than
    /// re-filled: it holds a composed sample, and the rest of the document
    /// alone would be that composition with the layer being smoothed taken out
    /// of it. Taking a request and not submitting it leaves the brick holding
    /// what it had, which is exactly right here and is the same property the
    /// band's neighbours rely on in [`Self::submit_round`].
    fn widen_over_the_rest(&mut self, document: &Document) -> Result<(), ModelError> {
        // No offset means nothing was primed — an empty layer — and there is
        // no lattice to widen yet.
        let (Some(rest), Some(offset)) = (self.rest.as_ref(), self.offset) else {
            return Ok(());
        };
        let bounds = rest.bounds.clone();
        for (min, max) in bounds {
            let low = std::array::from_fn(|axis| min[axis] - offset[axis]);
            let high = std::array::from_fn(|axis| max[axis] - offset[axis]);
            self.cache
                .mark_dirty(low, high)
                .map_err(ModelError::engine)?;
        }
        loop {
            let (requests, remaining) = self
                .cache
                .take_dirty(Self::DRAIN_BATCH)
                .map_err(ModelError::engine)?;
            if requests.is_empty() {
                break;
            }
            let wanted: Vec<BrickRequest> = requests
                .into_iter()
                .filter(|request| {
                    self.rest
                        .as_ref()
                        .is_some_and(|rest| !rest.covered.contains(&request.key()))
                })
                .collect();
            if !wanted.is_empty() {
                let values = self.the_rest_of_the_document(document, offset, &wanted)?;
                self.cache
                    .submit(&wanted, &values, None)
                    .map_err(ModelError::engine)?;
            }
            if remaining == 0 {
                break;
            }
        }
        Ok(())
    }

    /// Evaluates the document without the layer being previewed, over bricks
    /// this cache named in its own coordinates.
    ///
    /// The requests are translated on the way out and the originals are kept
    /// for the submission — see [`claycore::BrickRequest::translated`], and
    /// the module comment above for why this lattice is not the document's.
    fn the_rest_of_the_document(
        &self,
        document: &Document,
        offset: [f32; 3],
        requests: &[BrickRequest],
    ) -> Result<Vec<f32>, ModelError> {
        let rest = self
            .rest
            .as_ref()
            .expect("only called with a rest to compose");
        let moved: Vec<BrickRequest> = requests
            .iter()
            .map(|request| request.translated(offset))
            .collect();
        Ok(self
            .cache
            .eval_excluding(document, rest.excluded, rest.backend.as_ref(), &moved)
            .map_err(ModelError::engine)?
            .values)
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
    pub(crate) fn dab(
        &mut self,
        document: &Document,
        params: RelaxParams<'_>,
    ) -> Result<usize, ModelError> {
        self.transaction
            .update(params)
            .map_err(ModelError::engine)?;
        self.absorb(document)
    }

    /// Moves the bricks the transaction says are new into the preview cache.
    fn absorb(&mut self, document: &Document) -> Result<usize, ModelError> {
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
        // What the transaction covers, accumulated across the gesture: the
        // widening pass reads it to leave composed bricks alone.
        if let Some(rest) = self.rest.as_mut() {
            rest.covered.extend(wanted.keys().copied());
        }

        self.drain_into_cache(document, &delta, &wanted)
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
        document: &Document,
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
            accepted += self.submit_round(document, delta, wanted, requests)?;
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
        document: &Document,
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
        self.compose_the_rest(document, &submitted, &mut values)?;
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

    /// Folds the rest of the document into a batch the transaction produced.
    ///
    /// An elementwise minimum, and it is exact rather than an approximation:
    /// the engine composes visible field subtools by a hard union, so the
    /// smaller of the two distances at a sample *is* the document's distance
    /// there. Nothing is blended, so there is no seam to place.
    ///
    /// A no-op where nothing else is visible, which is the ordinary document
    /// and every figure the brushes are measured on.
    fn compose_the_rest(
        &self,
        document: &Document,
        submitted: &[BrickRequest],
        values: &mut [f32],
    ) -> Result<(), ModelError> {
        let (Some(_), Some(offset)) = (self.rest.as_ref(), self.offset) else {
            return Ok(());
        };
        if submitted.is_empty() {
            return Ok(());
        }
        let rest = self.the_rest_of_the_document(document, offset, submitted)?;
        for (preview, rest) in values.iter_mut().zip(rest) {
            *preview = preview.min(rest);
        }
        Ok(())
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
