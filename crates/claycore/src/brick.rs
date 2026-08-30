//! The sparse brick cache: the engine's incremental view of the field.
//!
//! The cache is what makes a brush dab cost what it touched. An edit marks the
//! bricks its influence bound reaches; draining yields plain-data requests the
//! caller evaluates and submits; meshing takes a key subset and reports which
//! vertices and indices each key produced, so a renderer patches sub-ranges
//! rather than rebuilding.
//!
//! The cache drives nothing itself — no threads, no queues, no refill loop.
//! Scheduling belongs to the caller.

use std::ptr::NonNull;

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::error::{check, ErrorKind, Result};
use crate::mesh::Mesh;
use crate::{raw_failure, Backend, Document, LayerId};

/// A brick coordinate.
pub type BrickKey = [i32; 3];

/// What a brick holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrickState {
    /// Uniformly inside; no lattice is allocated, which is why empty space
    /// costs nothing.
    Inside,
    /// Uniformly outside; likewise implicit.
    Outside,
    /// The band crosses it, so it stores an fp16 lattice.
    Surface,
    /// The cache holds nothing for this key. Its samples are left untouched by
    /// a read — the one state where the buffer is not written.
    Missing,
}

impl BrickState {
    fn from_raw(value: i32) -> Self {
        match value as u32 {
            sys::clay_brick_state::CLAY_BRICK_INSIDE => Self::Inside,
            sys::clay_brick_state::CLAY_BRICK_OUTSIDE => Self::Outside,
            sys::clay_brick_state::CLAY_BRICK_SURFACE => Self::Surface,
            _ => Self::Missing,
        }
    }
}

/// How the cache is laid out.
#[derive(Debug, Clone, Copy)]
pub struct BrickConfig {
    /// Lattice samples per brick axis; the engine accepts 8 or 16.
    pub dim: i32,
    /// World units between lattice samples.
    pub voxel_size: f32,
    /// Half-width of the kept band, in voxels.
    pub band_voxels: i32,
    /// Bytes of surface-brick payload the cache may hold; `None` is unlimited.
    pub memory_budget: Option<u64>,
    /// Carry an RGBA8 colour lattice beside the distances, so a colour atlas
    /// can be uploaded without meshing.
    ///
    /// Chosen here rather than per call because a colour lattice has to be
    /// evaluated to exist. With it set, refilling requires colours and reading
    /// may return them; without it, both refuse to deal in colour at all. It
    /// costs two bytes per sample more than the distance alone inside the same
    /// budget, so a colour cache holds roughly a third of the bricks.
    pub colors: bool,
}

impl Default for BrickConfig {
    fn default() -> Self {
        // The engine's own defaults, read from it rather than restated here.
        let mut raw = sys::clay_brick_config::sized();
        // SAFETY: a valid descriptor out-parameter with struct_size set.
        let _ = unsafe { sys::clay_brick_config_defaults(&mut raw) };
        Self {
            dim: raw.dim,
            voxel_size: raw.voxel_size,
            band_voxels: raw.band_voxels,
            memory_budget: (raw.memory_budget != 0).then_some(raw.memory_budget),
            colors: raw.colors != 0,
        }
    }
}

impl BrickConfig {
    fn to_raw(self) -> sys::clay_brick_config {
        let mut raw = sys::clay_brick_config::sized();
        raw.dim = self.dim;
        raw.voxel_size = self.voxel_size;
        raw.band_voxels = self.band_voxels;
        raw.memory_budget = self.memory_budget.unwrap_or(0);
        raw.colors = i32::from(self.colors);
        raw
    }

    /// Samples per brick, at a given apron width.
    pub fn samples_per_brick(&self, apron: i32) -> usize {
        let padded = (self.dim + 2 * apron).max(0) as usize;
        padded * padded * padded
    }
}

/// What the cache is holding, for a progress bar or a budget decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrickStats {
    pub tracked_bricks: u64,
    pub surface_bricks: u64,
    pub dirty_bricks: u64,
    pub memory_usage: u64,
    /// What the cache was created with; `None` is unlimited.
    pub memory_budget: Option<u64>,
}

/// Which vertices and indices one key contributed to a subset mesh.
///
/// Vertices are welded on canonical lattice-edge keys and the weld spans brick
/// seams, so a triangle in one key's index range may reference a vertex in an
/// *earlier* key's vertex range. A range may be overwritten; it may not be
/// freed in isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrickMeshRange {
    pub key: BrickKey,
    pub vertex_first: u32,
    pub vertex_count: u32,
    pub index_first: u32,
    pub index_count: u32,
}

/// How to shade a subset mesh.
#[derive(Debug, Clone, Copy)]
pub struct BrickMeshParams {
    /// Gradient normals and colours are attributes of the field, so both
    /// require a document. Asking for either without one is refused rather
    /// than quietly downgraded.
    pub gradient_normals: bool,
    pub colors: bool,
    /// Tetrahedron-tap half-width; `None` takes the engine's default.
    pub gradient_eps: Option<f32>,
}

impl Default for BrickMeshParams {
    fn default() -> Self {
        Self {
            gradient_normals: true,
            colors: false,
            gradient_eps: None,
        }
    }
}

impl BrickMeshParams {
    fn to_raw(self) -> sys::clay_brick_mesh_params {
        let mut raw = sys::clay_brick_mesh_params::sized();
        raw.normals = (if self.gradient_normals {
            sys::clay_normal_mode::CLAY_NORMAL_GRADIENT
        } else {
            sys::clay_normal_mode::CLAY_NORMAL_FACE
        }) as i32;
        raw.colors = i32::from(self.colors);
        raw.gradient_eps = self.gradient_eps.unwrap_or(0.0);
        raw
    }
}

/// One brick's worth of work, as the cache hands it out.
///
/// Opaque by design: `generation` is what submit checks, and the caller is not
/// meant to interpret it.
#[derive(Debug, Clone, Copy)]
pub struct BrickRequest(pub(crate) sys::clay_brick_request);

impl BrickRequest {
    pub fn key(&self) -> BrickKey {
        self.0.key
    }
}

/// Samples read back from the cache.
#[derive(Clone)]
pub struct BrickSamples {
    /// `dim + 2 * apron` cubed IEEE binary16 values per key, at a fixed
    /// stride: brick *i* occupies `values[i * stride..]` whatever its state.
    /// These are the engine's own bits, unconverted — what a GPU upload wants.
    pub values: Vec<u16>,
    /// Optional RGBA8 lattice, same stride in texels.
    pub colors: Option<Vec<u8>>,
    pub states: Vec<BrickState>,
    /// Samples per brick along one axis, apron included.
    pub padded_dim: i32,
}

impl std::fmt::Debug for BrickSamples {
    /// Summarised rather than dumped: a read of a few bricks is already tens
    /// of thousands of samples.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrickSamples")
            .field("bricks", &self.states.len())
            .field("padded_dim", &self.padded_dim)
            .field("samples", &self.values.len())
            .field("has_colors", &self.colors.is_some())
            .finish()
    }
}

/// What became of one submitted brick. Only [`Self::Accepted`] changed the
/// cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrickSubmit {
    Accepted,
    /// Re-dirtied while the request was in flight, or no longer tracked.
    /// Expected, and not an error: drop the values and wait for the request
    /// the next [`BrickCache::take_dirty`] hands over.
    Stale,
    /// Storing it would put the cache over its memory budget. Everything
    /// already stored stays valid and the ceiling is never breached; the brick
    /// simply stays unevaluated.
    ///
    /// Worth reading rather than discarding: a cache silently refusing every
    /// brick looks exactly like a document with no surface in it.
    BudgetExceeded,
}

impl BrickSubmit {
    fn from_raw(raw: i32) -> Self {
        match raw {
            x if x == sys::clay_brick_submit::CLAY_BRICK_SUBMIT_STALE as i32 => Self::Stale,
            x if x == sys::clay_brick_submit::CLAY_BRICK_SUBMIT_BUDGET_EXCEEDED as i32 => {
                Self::BudgetExceeded
            }
            _ => Self::Accepted,
        }
    }
}

/// The engine's sparse, dirty-tracked view of a document's field.
pub struct BrickCache {
    raw: NonNull<sys::clay_brick_cache>,
    config: BrickConfig,
}

// SAFETY: the cache is host memory with no interior threading; the engine
// starts none. Calls on one handle remain the caller's to serialize, which the
// absence of `Sync` enforces.
unsafe impl Send for BrickCache {}

impl BrickCache {
    /// Creates a cache with the given layout.
    pub fn new(config: BrickConfig) -> Result<Self> {
        let raw_config = config.to_raw();
        // SAFETY: a valid versioned descriptor; returns an owned handle or null.
        let raw = unsafe { sys::clay_brick_cache_create(&raw_config) };
        NonNull::new(raw)
            .map(|raw| Self { raw, config })
            .ok_or_else(|| raw_failure("clay_brick_cache_create", ErrorKind::InvalidArgument))
    }

    pub fn config(&self) -> BrickConfig {
        self.config
    }

    /// What the cache currently holds.
    pub fn stats(&self) -> Result<BrickStats> {
        let mut raw = sys::clay_brick_stats::sized();
        // SAFETY: valid handle and a descriptor with struct_size set.
        check(
            unsafe { sys::clay_brick_cache_stats(self.raw.as_ptr(), &mut raw) },
            "clay_brick_cache_stats",
        )?;
        Ok(BrickStats {
            tracked_bricks: raw.tracked_bricks,
            surface_bricks: raw.surface_bricks,
            dirty_bricks: raw.dirty_bricks,
            memory_usage: raw.memory_usage,
            memory_budget: (raw.memory_budget != 0).then_some(raw.memory_budget),
        })
    }

    /// Marks every brick intersecting a world-space region.
    pub fn mark_dirty(&mut self, min: [f32; 3], max: [f32; 3]) -> Result<()> {
        // SAFETY: two three-float inputs as the entry point requires.
        check(
            unsafe {
                sys::clay_brick_cache_mark_dirty(self.raw.as_ptr(), min.as_ptr(), max.as_ptr())
            },
            "clay_brick_cache_mark_dirty",
        )
    }

    /// Marks what specific nodes reach.
    ///
    /// Bounded by the influence of those nodes rather than by the layer's
    /// extent, which is what keeps a dab's cost proportional to the dab. A
    /// layer whose content is spread far apart spans more bricks than any
    /// cache can hold, and marking it whole is refused rather than attempted.
    pub fn mark_dirty_nodes(
        &mut self,
        doc: &Document,
        layer: LayerId,
        nodes: &[crate::NodeId],
    ) -> Result<usize> {
        if nodes.is_empty() {
            return Ok(0);
        }
        let raw: Vec<sys::clay_node_id> = nodes.iter().map(|n| n.0).collect();
        let mut marked = 0usize;
        // SAFETY: valid handles; `raw` holds `nodes.len()` ids.
        check(
            unsafe {
                sys::clay_brick_cache_mark_dirty_nodes(
                    self.raw.as_ptr(),
                    doc.as_ptr(),
                    layer.0,
                    raw.as_ptr(),
                    raw.len(),
                    &mut marked,
                )
            },
            "clay_brick_cache_mark_dirty_nodes",
        )?;
        Ok(marked)
    }

    /// Marks everything a whole layer reaches.
    pub fn mark_dirty_layer(&mut self, doc: &Document, layer: LayerId) -> Result<()> {
        // SAFETY: both handles are valid; the document is only read.
        check(
            unsafe {
                sys::clay_brick_cache_mark_dirty_layer(self.raw.as_ptr(), doc.as_ptr(), layer.0)
            },
            "clay_brick_cache_mark_dirty_layer",
        )
    }

    /// Takes up to `max` pending requests. Returns the requests and how many
    /// remain queued.
    ///
    /// This is not a size query: the engine refuses a null buffer here, so the
    /// caller chooses a batch size and drains in rounds.
    pub fn take_dirty(&mut self, max: usize) -> Result<(Vec<BrickRequest>, usize)> {
        if max == 0 {
            return Ok((Vec::new(), self.stats()?.dirty_bricks as usize));
        }
        let mut raw = vec![sys::clay_brick_request::default(); max];
        let mut count = max;
        let mut remaining = 0usize;
        // SAFETY: `raw` is valid for `max` elements, `count` carries that
        // capacity in and the filled length out.
        check(
            unsafe {
                sys::clay_brick_cache_take_dirty(
                    self.raw.as_ptr(),
                    raw.as_mut_ptr(),
                    &mut count,
                    &mut remaining,
                )
            },
            "clay_brick_cache_take_dirty",
        )?;
        raw.truncate(count);
        Ok((raw.into_iter().map(BrickRequest).collect(), remaining))
    }

    /// Every brick that stores samples.
    pub fn surface_bricks(&self) -> Result<Vec<BrickKey>> {
        let mut count = 0usize;
        // SAFETY: the size-query form, with a null buffer.
        check(
            unsafe {
                sys::clay_brick_cache_surface_bricks(
                    self.raw.as_ptr(),
                    std::ptr::null_mut(),
                    &mut count,
                )
            },
            "clay_brick_cache_surface_bricks",
        )?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut keys = vec![[0i32; 3]; count];
        // SAFETY: `keys` holds `count * 3` int32, which is what is asked for.
        check(
            unsafe {
                sys::clay_brick_cache_surface_bricks(
                    self.raw.as_ptr(),
                    keys.as_mut_ptr() as *mut i32,
                    &mut count,
                )
            },
            "clay_brick_cache_surface_bricks",
        )?;
        keys.truncate(count);
        Ok(keys)
    }

    /// What each named key holds, without reading a sample.
    ///
    /// `read_bricks` with only `out_states`, which the C boundary allows —
    /// what it refuses is a call that would write nothing at all. That matters
    /// because the payload is `dim³` fp16 per key: asking 2940 keys for their
    /// state through the full read would allocate a couple of hundred
    /// megabytes to answer a question about 2940 enum values.
    ///
    /// Cheaper than `surface_bricks` where the question is about a known set
    /// rather than about the whole cache — that one is a size query plus a
    /// copy of *every* stored key, and a caller filtering a dab's worth of
    /// dirty keys would pay for the entire surface to learn about a dozen.
    pub fn states(&self, keys: &[BrickKey]) -> Result<Vec<BrickState>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let mut states = vec![0i32; keys.len()];
        // SAFETY: `keys` is `keys.len() * 3` int32 and `states` is one int32
        // per key. The value and colour buffers are null with zero capacity,
        // which this entry point permits as long as `out_states` is not also
        // null. `apron` is 0: no payload is being read for it to pad.
        check(
            unsafe {
                sys::clay_brick_cache_read_bricks(
                    self.raw.as_ptr(),
                    0,
                    keys.as_ptr() as *const i32,
                    keys.len(),
                    0,
                    states.as_mut_ptr(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0,
                )
            },
            "clay_brick_cache_read_bricks",
        )?;
        Ok(states.into_iter().map(BrickState::from_raw).collect())
    }

    /// Reads whole bricks in their stored fp16 form, ready for texture upload.
    ///
    /// `apron` pads each brick with samples from its neighbours so that
    /// hardware filtering across a brick seam has data to read. One voxel
    /// suffices for sampling distance; shading from central-difference
    /// gradients wants two.
    pub fn read_bricks(
        &self,
        keys: &[BrickKey],
        lod: i32,
        apron: i32,
        with_colors: bool,
    ) -> Result<BrickSamples> {
        let padded_dim = self.config.dim + 2 * apron;
        let per_brick = self.config.samples_per_brick(apron);
        let mut values = vec![0u16; per_brick * keys.len()];
        let mut states = vec![0i32; keys.len()];
        let mut colors = with_colors.then(|| vec![0u8; per_brick * keys.len() * 4]);

        let (colors_ptr, colors_cap) = match colors.as_mut() {
            Some(buf) => (buf.as_mut_ptr(), buf.len()),
            None => (std::ptr::null_mut(), 0),
        };

        // SAFETY: `keys` is `keys.len() * 3` int32; `values` and `states` are
        // sized to the counts passed; colours are either a matching buffer or
        // a null pointer with zero capacity, which the entry point permits.
        check(
            unsafe {
                sys::clay_brick_cache_read_bricks(
                    self.raw.as_ptr(),
                    lod,
                    keys.as_ptr() as *const i32,
                    keys.len(),
                    apron,
                    states.as_mut_ptr(),
                    values.as_mut_ptr(),
                    values.len(),
                    colors_ptr,
                    colors_cap,
                )
            },
            "clay_brick_cache_read_bricks",
        )?;

        Ok(BrickSamples {
            values,
            colors,
            states: states.into_iter().map(BrickState::from_raw).collect(),
            padded_dim,
        })
    }

    /// Builds the coarse brick covering a 2×2×2 block of full-resolution ones.
    ///
    /// Subsampled rather than evaluated: same lattice size, twice the spacing.
    /// Buildable only when all eight children are evaluated *and* clean, so
    /// `false` is an ordinary "not yet" rather than a failure. Dirtying any
    /// child drops the mip, which is what stops one being downsampled from
    /// stale data.
    pub fn build_mip(&mut self, coarse_key: BrickKey) -> Result<bool> {
        let mut built = 0;
        // SAFETY: a three-int key and an out-parameter written by the call.
        check(
            unsafe {
                sys::clay_brick_cache_build_mip(self.raw.as_ptr(), coarse_key.as_ptr(), &mut built)
            },
            "clay_brick_cache_build_mip",
        )?;
        Ok(built != 0)
    }

    /// Whether a valid mip exists for a coarse key — the cheap way to ask
    /// before reading one.
    pub fn current_lod(&self, coarse_key: BrickKey) -> Result<i32> {
        let mut lod = 0;
        // SAFETY: a three-int key and an out-parameter written by the call.
        check(
            unsafe {
                sys::clay_brick_cache_current_lod(self.raw.as_ptr(), coarse_key.as_ptr(), &mut lod)
            },
            "clay_brick_cache_current_lod",
        )?;
        Ok(lod)
    }

    /// Meshes a subset of the cache's surface bricks.
    ///
    /// `keys` empty means every surface brick. The returned ranges say which
    /// vertices and indices each key produced — read [`BrickMeshRange`] before
    /// using them to manage buffer memory.
    pub fn mesh(
        &self,
        doc: Option<&Document>,
        params: BrickMeshParams,
        keys: &[BrickKey],
    ) -> Result<(Mesh, Vec<BrickMeshRange>)> {
        self.mesh_lod(doc, params, 0, keys)
    }

    /// The same, at a level of the cache's lattice.
    ///
    /// `lod` is 0 for the full-resolution bricks — where this is exactly
    /// [`BrickCache::mesh`] — or 1 for their mips. At level 1 `keys` names
    /// *coarse* keys, the 2×2×2 block keys [`BrickCache::build_mip`] and
    /// [`BrickCache::current_lod`] take, and empty still means "every brick
    /// this level stores".
    ///
    /// Level 1 **refuses** colours and gradient normals rather than
    /// downgrading them, so `params` must have both off: a coarse vertex sits
    /// on the mip's surface rather than the field's, far enough off it that a
    /// per-brick culled tape and the whole document's no longer agree. Face
    /// normals come from the triangles and work at every level.
    ///
    /// A level that was never built is an error rather than an empty mesh,
    /// because an empty mesh already means "no surface bricks" — an ordinary
    /// state of a session, where a missing mip is a "not yet". Ask
    /// [`BrickCache::current_lod`] before naming a coarse key.
    pub fn mesh_lod(
        &self,
        doc: Option<&Document>,
        params: BrickMeshParams,
        lod: i32,
        keys: &[BrickKey],
    ) -> Result<(Mesh, Vec<BrickMeshRange>)> {
        let raw_params = params.to_raw();
        let doc_ptr = doc.map_or(std::ptr::null(), |d| d.as_ptr() as *const _);
        let mut ranges = vec![sys::clay_brick_mesh_range::default(); keys.len()];
        let mut mesh = std::ptr::null_mut();

        let (keys_ptr, key_count, ranges_ptr) = if keys.is_empty() {
            (std::ptr::null(), 0, std::ptr::null_mut())
        } else {
            (keys.as_ptr() as *const i32, keys.len(), ranges.as_mut_ptr())
        };

        // SAFETY: keys and ranges are either both present and equally sized,
        // or both null with a zero count, which selects "every surface brick".
        check(
            unsafe {
                sys::clay_brick_cache_mesh_lod(
                    self.raw.as_ptr(),
                    doc_ptr,
                    &raw_params,
                    lod,
                    keys_ptr,
                    key_count,
                    ranges_ptr,
                    &mut mesh,
                )
            },
            "clay_brick_cache_mesh_lod",
        )?;

        let mesh = Mesh::from_raw(mesh, "clay_brick_cache_mesh_lod")?;
        let ranges = ranges
            .into_iter()
            .map(|r| BrickMeshRange {
                key: r.key,
                vertex_first: r.vertex_first,
                vertex_count: r.vertex_count,
                index_first: r.index_first,
                index_count: r.index_count,
            })
            .collect();
        Ok((mesh, ranges))
    }

    /// Evaluates a drained batch and submits the results back.
    ///
    /// The two halves are one call here because splitting them invites the
    /// caller to submit results for requests they did not evaluate. Scheduling
    /// across threads is still theirs: this runs where it is called.
    pub fn refill(
        &mut self,
        doc: &Document,
        backend: Option<&Backend>,
        requests: &[BrickRequest],
    ) -> Result<usize> {
        if requests.is_empty() {
            return Ok(0);
        }

        let raw: Vec<sys::clay_brick_request> = requests.iter().map(|r| r.0).collect();
        let per_brick = self.config.samples_per_brick(0);
        let mut values = vec![0.0f32; per_brick * requests.len()];
        // A colour cache requires colours on submit, so they are evaluated
        // alongside the distances rather than left for a second pass.
        let mut colors = self
            .config
            .colors
            .then(|| vec![0.0f32; per_brick * requests.len() * 3]);
        let (colors_ptr, colors_cap) = match colors.as_mut() {
            Some(buf) => (buf.as_mut_ptr(), buf.len()),
            None => (std::ptr::null_mut(), 0),
        };

        let name = backend
            .map(|b| crate::cstring(b.as_str(), "clay_brick_cache_eval_requests"))
            .transpose()?;
        let name_ptr = name.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());

        // SAFETY: `raw` and `values` are sized to the request count and the
        // configured brick volume; colours are declined with a null pointer.
        check(
            unsafe {
                sys::clay_brick_cache_eval_requests(
                    doc.as_ptr(),
                    name_ptr,
                    raw.as_ptr(),
                    raw.len(),
                    values.as_mut_ptr(),
                    values.len(),
                    colors_ptr,
                    colors_cap,
                )
            },
            "clay_brick_cache_eval_requests",
        )?;

        let outcomes = self.submit(requests, &values, colors.as_deref())?;
        Ok(outcomes
            .iter()
            .filter(|o| **o == BrickSubmit::Accepted)
            .count())
    }

    /// Stores samples the caller produced, instead of evaluating a document.
    ///
    /// This is [`Self::refill`] with the evaluation left to the caller, which
    /// is what a live brush preview needs: the samples it draws come from a
    /// transaction's working volume, and the document they would otherwise be
    /// evaluated from is deliberately unchanged until the gesture commits.
    ///
    /// `values` is `dim^3` floats per request in the grids' own order, x
    /// fastest — **no apron**, unlike what [`Self::read_bricks`] gives back —
    /// and the length is checked exactly by the engine.
    pub fn submit(
        &mut self,
        requests: &[BrickRequest],
        values: &[f32],
        colors: Option<&[f32]>,
    ) -> Result<Vec<BrickSubmit>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let raw: Vec<sys::clay_brick_request> = requests.iter().map(|r| r.0).collect();
        let mut results = vec![0i32; requests.len()];
        // SAFETY: the request array and both sample buffers are passed with
        // their own lengths, which the engine checks exactly against the
        // configured brick volume; `results` has one slot per request, and the
        // count of accepted ones is declined with a null — it is what this
        // returns, per request, rather than as a total.
        check(
            unsafe {
                sys::clay_brick_cache_submit(
                    self.raw.as_ptr(),
                    raw.as_ptr(),
                    raw.len(),
                    values.as_ptr(),
                    values.len(),
                    colors.map_or(std::ptr::null(), <[f32]>::as_ptr),
                    colors.map_or(0, <[f32]>::len),
                    results.as_mut_ptr(),
                    std::ptr::null_mut(),
                )
            },
            "clay_brick_cache_submit",
        )?;
        Ok(results.into_iter().map(BrickSubmit::from_raw).collect())
    }

    /// Refills everything currently dirty, in rounds of `batch`.
    ///
    /// A convenience over [`Self::take_dirty`] and [`Self::refill`] for callers
    /// that do not need to interleave the drain with anything else.
    pub fn refill_all(
        &mut self,
        doc: &Document,
        backend: Option<&Backend>,
        batch: usize,
    ) -> Result<usize> {
        let mut total = 0;
        loop {
            let (requests, remaining) = self.take_dirty(batch)?;
            if requests.is_empty() {
                return Ok(total);
            }
            total += self.refill(doc, backend, &requests)?;
            if remaining == 0 {
                return Ok(total);
            }
        }
    }

    /// Casts one ray against the cached bricks.
    pub fn raycast(
        &self,
        origin: [f32; 3],
        direction: [f32; 3],
    ) -> Result<Option<crate::pick::Hit>> {
        let mut hit = 0i32;
        let mut t = 0.0f32;
        let (mut position, mut normal) = ([0.0f32; 3], [0.0f32; 3]);
        // SAFETY: three-float inputs and out-parameters as declared.
        check(
            unsafe {
                sys::clay_brick_cache_raycast(
                    self.raw.as_ptr(),
                    origin.as_ptr(),
                    direction.as_ptr(),
                    &mut hit,
                    &mut t,
                    position.as_mut_ptr(),
                    normal.as_mut_ptr(),
                )
            },
            "clay_brick_cache_raycast",
        )?;
        Ok((hit != 0).then_some(crate::pick::Hit {
            t,
            position,
            normal,
            layer: None,
            node: None,
        }))
    }
}

impl Drop for BrickCache {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once.
        unsafe { sys::clay_brick_cache_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for BrickCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrickCache")
            .field("config", &self.config)
            .finish()
    }
}
