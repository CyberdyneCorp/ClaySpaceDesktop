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
    BrickCache, BrickConfig, BrickKey, BrickSubmit, Document, LayerId, RelaxParams, SculptBudget,
    SculptPolicy, SmoothTransaction,
};
use clayspace_model::{LayerKey, ModelError};

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
    /// Which layer the gesture belongs to, so a subtool switch cannot commit
    /// it onto the wrong one.
    layer: LayerKey,
    /// Preview keys the viewport has not drawn yet.
    dirty: Vec<BrickKey>,
    /// Whether any dab has moved a sample, so a gesture that touched nothing
    /// commits nothing.
    changed: bool,
    brick_span: f32,
    config: BrickConfig,
}

impl LiveSmooth {
    /// Opens a gesture, or reports why it cannot be live.
    pub(crate) fn begin(
        document: &mut Document,
        layer: LayerId,
        key: LayerKey,
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
            layer: key,
            dirty: Vec::new(),
            changed: false,
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

    pub(crate) fn layer(&self) -> LayerKey {
        self.layer
    }

    pub(crate) fn changed(&self) -> bool {
        self.changed
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
        let dirty = self
            .transaction
            .update(params)
            .map_err(ModelError::engine)?;
        self.changed |= dirty.changed;
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

    /// Installs the gesture as one undo step.
    pub(crate) fn commit(&mut self) -> Result<SculptBudget, ModelError> {
        self.transaction.commit().map_err(ModelError::engine)
    }
}
