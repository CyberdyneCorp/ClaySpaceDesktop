//! An adaptive surface the document holds beside one of its mesh layers, and
//! the file that keeps it.
//!
//! # Two objects, as with a hierarchy
//!
//! A `clay_dynamic_surface` is not a `clay_layer_id`: it is a free-standing
//! owning handle read from a mesh, and `clay_document_save` has never heard of
//! it. So an adaptive row is a real mesh layer in the `.clayspace` — its name,
//! its place in the stack, its transform, its mask — plus an [`Adaptive`] held
//! here. The mesh layer keeps the triangles the surface was read from; the
//! surface is what the brush reshapes, what the viewport draws and what the
//! side-car saves. See [`crate::multires`], which made the same arrangement
//! first and whose side-car format this one shares under its own header.
//!
//! # One sculptor for the surface's life
//!
//! The surface is held in a [`DynamicSession`] with the sculptor bound to it,
//! rather than a sculptor being made per stroke segment. The sculptor owns the
//! spatial index and the chunk table, so a sculptor per segment paid the index
//! build per segment — 117 ms over 100,352 triangles and 1.50 s over 1,002,528
//! on the pinned engine — and started every segment with an empty dirty set,
//! which left nothing for the viewport to upload incrementally.
//!
//! # Drawing
//!
//! Chunk by chunk ([`crate::chunked`]): the engine's dirty set is drained into
//! a mirror, and only the chunks a stroke touched are sent again — their
//! indices only when their topology changed. A surface that carries vertex
//! colour is the exception and is still copied whole when it moves: the chunk
//! transport copies positions, normals and indices and no attribute, so a
//! chunked colour surface would lose its paint.
//!
//! # A missing side-car
//!
//! Opens, and the row comes back as the mesh layer it demonstrably is, with
//! the loss named in the diagnostics report — the hierarchy's rule, for the
//! hierarchy's reasons. What never happens is the other direction: a row is
//! promoted to Dynamic only by a record naming it.
//!
//! # History
//!
//! One gesture is one [`Record`] in the document's one ordered history,
//! however many segments and mirrors drew it. Which kind is decided when the
//! gesture opens:
//!
//! - **A closed surface** records the engine's reversible topology delta
//!   (`clay_dynamic_delta`, captured by
//!   `clay_dynamic_sculptor_apply_stroke_recorded`): every vertex, half-edge,
//!   edge and face the stroke created, deleted or rewrote, with both ends. A
//!   replay is bit-exact over what `to_mesh` exports, so undo and redo restore
//!   connectivity and positions exactly, and the record costs what the stroke
//!   reached rather than the surface's whole serialized size.
//! - **A surface with an open boundary** records its bytes before the
//!   gesture, as every adaptive gesture did before the delta was carried. On
//!   the pinned engine (ClayCore v0.120.1) a revert of a stroke that reached
//!   the boundary gives back the right triangles and leaves a live boundary
//!   half-edge whose `next` is a dead slot — `clay_dynamic_surface_validate`
//!   says "half-edge N has a dead next" — so the delta is not used there
//!   until the engine records boundary links. Measured and pinned by
//!   `claycore`'s `a_revert_at_an_open_boundary_leaves_a_dead_next`.
//!
//! The engine's two rules the delta is held to: replay is **last in, first
//! out**, which the document's single ordered stack already is; and **a
//! record never outlives its surface handle**, so a surface put back from
//! bytes starts a new chain. That only happens on an open surface, whose
//! records are all snapshots.

use claycore::{
    DynamicDelta, DynamicDesc, DynamicSculptor, DynamicSession, DynamicSurface, DynamicTopology,
};
use clayspace_model::{CageFault, ModelError, Refusal};

use crate::chunked::{Bounds, ChunkMirror, Region, RegionBuffers, RegionRuns};

/// What a crossing into or out of an adaptive surface may peak at, on top of
/// what the document already holds.
///
/// The hierarchy's figure, for the hierarchy's reason: zero would be "no
/// budget", and then there is no refusal to offer — a host finds out what a
/// crossing costs by running out of memory during it. Two gigabytes is past
/// what a crossing of any mesh this application imports reaches, so the
/// refusal is for the model that genuinely does not fit rather than a limit a
/// sculptor meets in ordinary work. A host on a constrained device lowers it
/// through `ClayDocument::set_surface_budget`.
pub const CROSSING_BUDGET: u64 = crate::multires::LEVEL_BUDGET;

/// The triangles the viewport is drawing for a coloured surface, and the
/// surface state they were copied at.
struct Drawn {
    watched: (claycore::SurfaceRevision, u64),
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

/// How a surface reaches the viewport.
enum Drawing {
    /// Copied whole whenever it moves: a surface carrying vertex colour,
    /// which the chunk transport cannot carry.
    Whole(Option<Drawn>),
    /// Chunk by chunk, following the engine's dirty set.
    Chunked(Chunked),
}

/// The chunked drawing's state.
#[derive(Default)]
struct Chunked {
    mirror: ChunkMirror,
    /// The surface generation the mirror holds, or `None` before the first
    /// copy. A different one is a new chunk table to copy whole.
    mirrored: Option<u64>,
    /// Where the chunks sit in the carried buffer, as the last full build
    /// laid them out. `None` for a surface that build did not draw.
    region: Option<Region>,
}

/// What a surface's region needs after its chunks were drained.
#[derive(Debug, PartialEq)]
pub enum RegionPatch {
    /// The last full build did not draw this surface — hidden, or drawn
    /// whole — so there is nothing of it to patch.
    Undrawn,
    /// These runs bring the drawn region up to the surface.
    Runs(RegionRuns),
    /// The region cannot take the change in place; lay it out again.
    Rebuild,
}

/// An adaptive surface, and everything this side has to remember about it.
pub struct Adaptive {
    session: DynamicSession,
    /// Whether the surface carries a colour attribute at all. Fixed for the
    /// surface's life: no stroke adds one, and a colour brush is refused over
    /// a surface without it.
    coloured: bool,
    drawing: Drawing,
    /// How many times this side has replaced the surface underneath itself.
    ///
    /// A surface put back from bytes is a new identity whose revisions start
    /// again, so the engine's counters alone cannot tell a redo from nothing
    /// having happened. This is monotone across every restore.
    generation: u64,
    open: Option<OpenGesture>,
    /// What rebuilding this surface's index cost the last time, for the
    /// maintenance queue to weigh the next request against. `None` before
    /// the first, which is filed with no estimate so it can be measured.
    rebuild_micros: Option<u64>,
}

/// A gesture that is open, and what it has done so far.
struct OpenGesture {
    /// What the gesture enters the history as, and what a dragging verb is
    /// taken back by before it is laid down again from its anchor.
    record: Record,
    /// Whether any segment changed the surface, so a stroke that reached
    /// nothing does not become an undo step.
    changed: bool,
}

/// What it takes to put one adaptive gesture back. See the module's own note
/// for which a gesture gets.
pub enum Record {
    /// The engine's reversible topology delta. *Symmetric*: the same record
    /// reverts and re-applies, so it travels between the stacks unchanged.
    Delta(DynamicDelta),
    /// The surface's bytes on the other side of the step. One *state*, so the
    /// record that goes the other way is the state the step leaves.
    Snapshot(Vec<u8>),
}

impl Record {
    /// What the record holds in memory: the figure the history budget is
    /// spent in. The engine's own memory ledger counts neither kind.
    pub fn weight(&self) -> usize {
        match self {
            Self::Delta(delta) => delta.stats().map_or(0, |stats| {
                usize::try_from(stats.resident_bytes).unwrap_or(usize::MAX)
            }),
            Self::Snapshot(bytes) => bytes.len(),
        }
    }
}

/// Which way a history step goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replay {
    /// To where the gesture found the surface: an undo.
    Revert,
    /// To where the gesture left it: a redo.
    Apply,
}

/// One mirror of one segment: the resolved path and the stamp it carries.
pub type Pass<'a> = (Vec<[f32; 5]>, claycore::MeshStamp<'a>);

impl Adaptive {
    /// Wraps a surface the caller has already built, binding the sculptor
    /// that will serve it for as long as it is held.
    ///
    /// Asks once whether it carries colour, which is a whole export: paid
    /// here, where the surface was just read or loaded whole anyway, and
    /// never again for this surface.
    pub fn holding(surface: DynamicSurface) -> Result<Self, ModelError> {
        let coloured = surface
            .to_mesh()
            .map(|mesh| mesh.colors().is_some())
            .map_err(ModelError::engine)?;
        let session = DynamicSession::new(surface).map_err(ModelError::engine)?;
        Ok(Self {
            session,
            coloured,
            drawing: if coloured {
                Drawing::Whole(None)
            } else {
                Drawing::Chunked(Chunked::default())
            },
            generation: 0,
            open: None,
            rebuild_micros: None,
        })
    }

    /// How every surface this application builds reads its mesh: the engine's
    /// own weld and seam tolerances.
    pub fn desc() -> DynamicDesc {
        DynamicDesc::default()
    }

    /// The topology policy a stroke runs with.
    ///
    /// The engine's own defaults — brush-relative detail, split and collapse
    /// on — and deliberately not a setting yet. When should the remesh run is
    /// answered per verb by the engine, which is the part a sculptor would
    /// otherwise get wrong; how fine is a control that arrives with its panel.
    pub fn topology() -> DynamicTopology {
        DynamicTopology::default()
    }

    /// Reads a mesh into a surface, or says in the domain's words why not.
    pub fn from_mesh(mesh: &claycore::Mesh) -> Result<Self, ModelError> {
        let surface = DynamicSurface::from_mesh(mesh, Self::desc()).map_err(refused)?;
        Self::holding(surface)
    }

    pub fn surface(&self) -> &DynamicSurface {
        self.session.surface()
    }

    /// Lends the surface's sculptor for a stroke. See
    /// [`DynamicSession::with_sculptor`].
    pub fn with_sculptor<R>(&mut self, f: impl for<'a> FnOnce(&mut DynamicSculptor<'a>) -> R) -> R {
        self.session.with_sculptor(f)
    }

    /// The half-edge census: what the inspector and the agent are told.
    pub fn stats(&self) -> claycore::DynamicStats {
        self.surface().stats().unwrap_or_default()
    }

    /// What the viewport watches: the engine's three revisions and this
    /// side's own generation.
    pub fn watched(&self) -> (claycore::SurfaceRevision, u64) {
        (
            self.surface().revision().unwrap_or_default(),
            self.generation,
        )
    }

    /// Whether the surface is drawn chunk by chunk rather than whole.
    pub fn is_chunked(&self) -> bool {
        matches!(self.drawing, Drawing::Chunked(_))
    }

    /// A number that moves whenever what is drawn moves, for a redraw hash.
    pub fn drawn_revision(&self) -> u64 {
        let (revision, generation) = self.watched();
        revision
            .topology
            .wrapping_add(revision.geometry)
            .wrapping_add(revision.attributes)
            .wrapping_add(generation.wrapping_mul(0x9E37_79B9))
    }

    /// The part of [`drawn_revision`](Self::drawn_revision) a region patch
    /// cannot follow, so a change to it means the carried buffer is built
    /// again: everything for a surface drawn whole, and only the generation
    /// — a new chunk table — for one drawn in chunks.
    pub fn layout_revision(&self) -> u64 {
        match self.drawing {
            Drawing::Whole(_) => self.drawn_revision(),
            Drawing::Chunked(_) => self.generation.wrapping_mul(0x9E37_79B9),
        }
    }

    /// The part a region patch does follow: the engine's revisions, for a
    /// surface drawn in chunks, and nothing for one drawn whole.
    pub fn chunk_revision(&self) -> u64 {
        match self.drawing {
            Drawing::Whole(_) => 0,
            Drawing::Chunked(_) => self.drawn_revision(),
        }
    }

    /// A coloured surface's triangles, copied whole and kept until the
    /// surface moves; `None` for a surface drawn in chunks.
    #[allow(clippy::type_complexity)]
    pub fn whole_triangles(&mut self) -> Option<(&[[f32; 3]], &[[f32; 3]], &[[f32; 3]], &[u32])> {
        let watched = self.watched();
        let Drawing::Whole(drawn) = &mut self.drawing else {
            return None;
        };
        if !matches!(drawn, Some(held) if held.watched == watched) {
            let mesh = self.session.surface().to_mesh().ok()?;
            let positions = mesh.positions().to_vec();
            let count = positions.len();
            *drawn = Some(Drawn {
                watched,
                normals: mesh.normals_or_derived(),
                colors: mesh
                    .colors()
                    .map(<[[f32; 3]]>::to_vec)
                    .unwrap_or_else(|| vec![[1.0; 3]; count]),
                indices: mesh.indices().to_vec(),
                positions,
            });
        }
        let drawn = drawn.as_ref()?;
        Some((
            &drawn.positions,
            &drawn.normals,
            &drawn.colors,
            &drawn.indices,
        ))
    }

    /// Brings the chunk mirror up to the surface: every chunk for a surface
    /// it has not mirrored, the dirty ones otherwise. Nothing for a surface
    /// drawn whole.
    fn sync_chunks(&mut self) -> Result<(), ModelError> {
        let generation = self.generation;
        let Drawing::Chunked(chunked) = &mut self.drawing else {
            return Ok(());
        };
        let mirror = &mut chunked.mirror;
        if chunked.mirrored == Some(generation) {
            self.session
                .with_sculptor(|sculptor| mirror.drain(sculptor))
                .map_err(ModelError::engine)?;
        } else {
            self.session
                .with_sculptor(|sculptor| mirror.refill(sculptor))
                .map_err(ModelError::engine)?;
            chunked.mirrored = Some(generation);
        }
        Ok(())
    }

    /// Lays the whole surface out as a region of the carried buffer starting
    /// at the given place, and remembers where; `None` for a surface drawn
    /// whole, or one with nothing to draw.
    pub fn lay_out_region(&mut self, vertex_base: u32, index_base: u32) -> Option<RegionBuffers> {
        if let Err(e) = self.sync_chunks() {
            eprintln!("a superfície adaptativa não pôde ser copiada: {e}");
        }
        let generation = self.generation;
        let Drawing::Chunked(chunked) = &mut self.drawing else {
            return None;
        };
        let (region, buffers) =
            Region::lay_out(&chunked.mirror, vertex_base, index_base, generation)?;
        // Everything is in the fresh layout, so nothing is pending for it.
        chunked.mirror.forget_pending();
        chunked.region = Some(region);
        Some(buffers)
    }

    /// Forgets where the last full build put the region, for a build that
    /// is about to lay the carried buffer out again.
    pub fn forget_region(&mut self) {
        if let Drawing::Chunked(chunked) = &mut self.drawing {
            chunked.region = None;
        }
    }

    /// Drains the chunks the engine marked dirty and says what the drawn
    /// region needs for them.
    pub fn patch_region(&mut self) -> RegionPatch {
        let drawn = matches!(&self.drawing, Drawing::Chunked(chunked) if chunked.region.is_some());
        if !drawn {
            return RegionPatch::Undrawn;
        }
        if self.sync_chunks().is_err() {
            return RegionPatch::Rebuild;
        }
        let generation = self.generation;
        let Drawing::Chunked(chunked) = &mut self.drawing else {
            return RegionPatch::Undrawn;
        };
        let Some(region) = chunked.region.as_mut() else {
            return RegionPatch::Undrawn;
        };
        if region.generation() != generation {
            return RegionPatch::Rebuild;
        }
        match region.patch(&mut chunked.mirror) {
            Some(runs) => RegionPatch::Runs(runs),
            None => RegionPatch::Rebuild,
        }
    }

    /// Whether the engine says the spatial index has degraded enough to be
    /// worth rebuilding (`clay_dynamic_sculptor_index_quality`). Its opinion,
    /// read between strokes; the maintenance queue decides when.
    pub fn wants_index_rebuild(&self) -> bool {
        self.session
            .sculptor()
            .index_quality()
            .is_ok_and(|quality| quality.wants_rebuild)
    }

    /// What the last index rebuild cost, in microseconds; zero before one.
    pub fn rebuild_micros(&self) -> u64 {
        self.rebuild_micros.unwrap_or(0)
    }

    /// Rebuilds the spatial index, between strokes.
    ///
    /// A rebuild clears the engine's dirty set and renumbers the chunks, so
    /// the drawn region no longer describes the chunk table: this counts as a
    /// new generation, and the next upload lays the surface out afresh.
    pub fn rebuild_index(&mut self) -> Result<(), ModelError> {
        let started = std::time::Instant::now();
        self.session
            .with_sculptor(|sculptor| sculptor.rebuild_index())
            .map_err(ModelError::engine)?;
        self.rebuild_micros = Some(started.elapsed().as_micros() as u64);
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    /// How many times the chunk mirror has had to grow a buffer; zero for a
    /// surface drawn whole.
    pub fn buffer_growths(&self) -> u64 {
        match &self.drawing {
            Drawing::Chunked(chunked) => chunked.mirror.buffer_growths(),
            Drawing::Whole(_) => 0,
        }
    }

    /// Whether the surface carries vertex colour for the colour brushes to
    /// write.
    ///
    /// A surface read from an uncoloured mesh carries none, and the engine's
    /// paint over one remeshes and colours nothing — a stroke that changes the
    /// topology and not what it was for. So the colour brushes are refused
    /// there, as they are on a mesh with no colour attribute.
    pub fn carries_colour(&self) -> bool {
        self.coloured
    }

    /// Where a ray meets the triangles last drawn, in the surface's own
    /// coordinates. A pick is a question and takes `&self`, so it walks what
    /// was copied rather than copying again.
    pub fn pick(
        &self,
        origin: [f32; 3],
        direction: [f32; 3],
        nearest: impl Fn([f32; 3], [f32; 3], &[[f32; 3]], &[u32]) -> Option<[f32; 3]>,
    ) -> Option<[f32; 3]> {
        match &self.drawing {
            Drawing::Whole(drawn) => {
                let drawn = drawn.as_ref()?;
                nearest(origin, direction, &drawn.positions, &drawn.indices)
            }
            Drawing::Chunked(chunked) => chunked
                .mirror
                .triangles()
                .filter(|(_, _, bounds)| bounds.is_none_or(|b| ray_meets_box(origin, direction, b)))
                .filter_map(|(positions, indices, _)| {
                    nearest(origin, direction, positions, indices)
                })
                .min_by(|a, b| {
                    along(origin, direction, *a).total_cmp(&along(origin, direction, *b))
                }),
        }
    }

    /// The box the surface occupies, in its own coordinates.
    pub fn bounds(&mut self) -> Option<Bounds> {
        if self.is_chunked() {
            if let Err(e) = self.sync_chunks() {
                eprintln!("a superfície adaptativa não pôde ser copiada: {e}");
            }
            let Drawing::Chunked(chunked) = &self.drawing else {
                return None;
            };
            return chunked.mirror.bounds();
        }
        let (positions, ..) = self.whole_triangles()?;
        let first = *positions.first()?;
        Some(positions.iter().fold((first, first), |(min, max), point| {
            (
                std::array::from_fn(|i| min[i].min(point[i])),
                std::array::from_fn(|i| max[i].max(point[i])),
            )
        }))
    }

    /// The surface as an ordinary mesh, priced by the engine before it is
    /// paid for.
    pub fn to_mesh(&self) -> Result<claycore::Mesh, ModelError> {
        let priced = self
            .surface()
            .preflight_to_mesh(0)
            .map_err(ModelError::engine)?;
        if !priced.allowed {
            return Err(ModelError::engine(format!(
                "a superfície adaptativa ocupa cerca de {} MB como malha, além do que cabe aqui",
                priced.peak_bytes / (1024 * 1024)
            )));
        }
        self.surface().to_mesh().map_err(ModelError::engine)
    }

    /// The surface as bytes, priced before it allocates.
    pub fn bytes(&self, budget: u64) -> Result<Vec<u8>, ModelError> {
        let priced = self
            .surface()
            .preflight_encode(budget)
            .map_err(ModelError::engine)?;
        if !priced.allowed {
            return Err(ModelError::engine(format!(
                "a superfície adaptativa ocupa cerca de {} MB, além do que cabe aqui",
                priced.persistent_bytes / (1024 * 1024)
            )));
        }
        self.surface().serialize().map_err(ModelError::engine)
    }

    /// A connectivity-and-position digest of the surface as it exports.
    ///
    /// What "restored exactly" is checked against: the engine promises a
    /// replay bit-exact over `to_mesh`, and explicitly not over the serialized
    /// bytes, which keep the slots an undone stroke allocated.
    pub fn digest(&self) -> Result<u64, ModelError> {
        let mesh = self.surface().to_mesh().map_err(ModelError::engine)?;
        Ok(mesh_digest(mesh.positions(), mesh.indices()))
    }

    /// Puts the surface back to bytes taken from it earlier.
    ///
    /// A new identity, whose revisions start again — hence the generation —
    /// onto which no earlier delta replays, and so a new sculptor, whose
    /// index is built here and whose chunk table the mirror copies whole on
    /// its next drain. The mirror's buffers are kept for it.
    fn restore(&mut self, bytes: &[u8]) -> Result<(), ModelError> {
        let surface = DynamicSurface::deserialize(bytes).map_err(ModelError::engine)?;
        self.session = DynamicSession::new(surface).map_err(ModelError::engine)?;
        if let Drawing::Whole(drawn) = &mut self.drawing {
            *drawn = None;
        }
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    /// Whether the surface has an open boundary, which decides the kind of
    /// record a gesture gets. See the module's own note.
    fn is_open(&self) -> bool {
        self.stats().boundary_edges > 0
    }

    // -- the gesture ---------------------------------------------------------

    /// Opens the gesture's record, if this is the first segment to reach it.
    pub fn open_gesture(&mut self) -> Result<(), ModelError> {
        if self.open.is_none() {
            let record = if self.is_open() {
                Record::Snapshot(self.bytes(0)?)
            } else {
                Record::Delta(DynamicDelta::new().map_err(ModelError::engine)?)
            };
            self.open = Some(OpenGesture {
                record,
                changed: false,
            });
        }
        Ok(())
    }

    /// Strokes one segment, every mirror of it, into the open gesture, and
    /// says whether anything changed.
    ///
    /// Through the sculptor the surface keeps for its whole life, so the index
    /// is not rebuilt per segment and the chunks this stroke dirties stay in
    /// the set the viewport drains. Each pass is captured into the gesture's
    /// delta where it has one: a delta that ends where the surface stands is
    /// *continued*, so the segments and mirrors of one gesture are one record.
    pub fn stroke(
        &mut self,
        passes: &[Pass<'_>],
        preset: &claycore::StrokePreset,
        topology: &DynamicTopology,
        mask: Option<&claycore::MaskField>,
    ) -> Result<bool, ModelError> {
        self.open_gesture()?;
        let mut delta = match self.open.as_mut().map(|open| &mut open.record) {
            Some(Record::Delta(delta)) => Some(delta),
            _ => None,
        };
        let changed = self.session.with_sculptor(|sculptor| {
            let mut changed = false;
            for (path, stamp) in passes {
                let (applied, _) = match delta.as_deref_mut() {
                    Some(delta) => sculptor.apply_stroke_recorded(
                        path,
                        preset,
                        *stamp,
                        Some(topology),
                        mask,
                        delta,
                    ),
                    None => sculptor.apply_stroke(path, preset, *stamp, Some(topology), mask),
                }
                .map_err(ModelError::engine)?;
                changed |= applied > 0;
            }
            Ok::<bool, ModelError>(changed)
        })?;
        if let Some(open) = self.open.as_mut() {
            open.changed |= changed;
        }
        Ok(changed)
    }

    /// Takes the open gesture back to where it started, for a dragging verb
    /// that lays itself down again from its anchor.
    pub fn replay_from_the_anchor(&mut self) -> Result<(), ModelError> {
        let Some(mut open) = self.open.take() else {
            return Ok(());
        };
        let back = match &mut open.record {
            Record::Delta(delta) => self
                .replay(delta, Replay::Revert)
                .and_then(|()| delta.clear().map_err(ModelError::engine)),
            Record::Snapshot(bytes) => self.restore(bytes),
        };
        open.changed = false;
        self.open = Some(open);
        back
    }

    /// The record the gesture leaves behind, and the gesture closed; `None`
    /// for one that changed nothing.
    pub fn close_gesture(&mut self) -> Option<Record> {
        self.open
            .take()
            .filter(|open| open.changed)
            .map(|open| open.record)
    }

    pub fn gesture_is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Steps one record in a direction, and hands back the record the other
    /// stack should hold.
    ///
    /// A delta travels unchanged. A snapshot is one state, so what goes the
    /// other way is the state this step is leaving, taken on the way past.
    pub fn step(&mut self, record: Record, way: Replay) -> Result<Record, ModelError> {
        match record {
            Record::Delta(delta) => {
                self.replay(&delta, way)?;
                Ok(Record::Delta(delta))
            }
            Record::Snapshot(bytes) => {
                let leaving = self.bytes(0)?;
                self.restore(&bytes)?;
                Ok(Record::Snapshot(leaving))
            }
        }
    }

    /// Puts the surface at one end of a delta, through the surface's own
    /// sculptor.
    ///
    /// The one the strokes use, so the replay keeps its index following the
    /// surface and marks the chunks it touched in the dirty set the viewport
    /// drains: an undo is patched like a stroke rather than laid out again. A
    /// delta from another surface, or one replayed out of order, is refused by
    /// the engine before anything is written.
    fn replay(&mut self, delta: &DynamicDelta, way: Replay) -> Result<(), ModelError> {
        self.session
            .with_sculptor(|sculptor| match way {
                Replay::Revert => delta.revert(sculptor),
                Replay::Apply => delta.apply(sculptor),
            })
            .map_err(ModelError::engine)
    }
}

/// FNV-1a over triangle indices and then position bits.
///
/// Bits rather than values, because the claim is bit-exactness: a restore
/// that brought a coordinate back through different arithmetic is not the
/// restore the engine promises.
pub fn mesh_digest(positions: &[[f32; 3]], indices: &[u32]) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    let words = indices
        .iter()
        .copied()
        .chain(positions.iter().flatten().map(|v| v.to_bits()));
    words
        .flat_map(u32::to_le_bytes)
        .fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(PRIME)
        })
}

/// How far along a ray a point on it lies.
fn along(origin: [f32; 3], direction: [f32; 3], point: [f32; 3]) -> f32 {
    (0..3).map(|i| (point[i] - origin[i]) * direction[i]).sum()
}

/// Whether a ray (forward of its origin) passes through a box — the slab
/// test, so a pick walks only the chunks it could hit.
fn ray_meets_box(origin: [f32; 3], direction: [f32; 3], (low, high): Bounds) -> bool {
    // Widened a little, so a hit the triangle test accepts on a face of the
    // box is not refused by the box.
    let min = low.map(|v| v - 1e-4 * (1.0 + v.abs()));
    let max = high.map(|v| v + 1e-4 * (1.0 + v.abs()));
    let (mut near, mut far) = (0.0f32, f32::INFINITY);
    for axis in 0..3 {
        if direction[axis].abs() < 1e-12 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return false;
            }
            continue;
        }
        let inverse = 1.0 / direction[axis];
        let (a, b) = (
            (min[axis] - origin[axis]) * inverse,
            (max[axis] - origin[axis]) * inverse,
        );
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    near <= far
}

impl std::fmt::Debug for Adaptive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Adaptive")
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

/// A refused read, in the domain's words where the engine named a model
/// problem a sculptor can go and mend.
pub fn refused(refused: claycore::DynamicRefusal) -> ModelError {
    match refused.reason {
        claycore::DynamicError::EmptyMesh => ModelError::Conversion(Refusal::SourceEmpty),
        claycore::DynamicError::NonManifoldEdge => ModelError::Conversion(Refusal::NotAdaptive {
            fault: CageFault::NonManifold,
        }),
        claycore::DynamicError::DegenerateTriangle => {
            ModelError::Conversion(Refusal::NotAdaptive {
                fault: CageFault::DegenerateFace,
            })
        }
        _ => ModelError::engine(refused.to_string()),
    }
}

// -- the side-car ------------------------------------------------------------

/// Where the adaptive surfaces live for a document at `path`.
pub fn sidecar_for(path: &std::path::Path) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".dynamic");
    path.with_file_name(name)
}

/// The first line of the file: the representation's stored key and a format
/// number, so a later format can be told from this one.
const HEADER: &[u8] = b"clayspace-dynamic 1\n";

/// Writes every surface the document holds, or removes the file when there
/// are none — see [`crate::multires::write_hierarchies`].
pub fn write_surfaces(
    path: &std::path::Path,
    surfaces: &[crate::multires::Saved],
) -> std::io::Result<()> {
    crate::multires::write_records(path, HEADER, surfaces)
}

/// Reads the surfaces back, reporting what could not be read.
pub fn read_surfaces(path: &std::path::Path) -> crate::multires::SideCar {
    crate::multires::read_records(path, HEADER)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(divisions: u32) -> claycore::Mesh {
        let stride = divisions + 1;
        let positions: Vec<[f32; 3]> = (0..stride)
            .flat_map(|z| (0..stride).map(move |x| [x as f32 * 0.5, 0.0, z as f32 * 0.5]))
            .collect();
        let indices: Vec<u32> = (0..divisions)
            .flat_map(|z| (0..divisions).map(move |x| z * stride + x))
            .flat_map(|a| [a, a + stride, a + 1, a + 1, a + stride, a + stride + 1])
            .collect();
        claycore::Mesh::from_triangles(&positions, &indices).expect("a sheet")
    }

    /// A closed ball: `rings` bands of `segments` quads split in two, with a
    /// fan at each pole.
    fn ball(rings: u32, segments: u32) -> claycore::Mesh {
        let mut positions = vec![[0.0, 1.0, 0.0]];
        for ring in 1..rings {
            let phi = std::f32::consts::PI * ring as f32 / rings as f32;
            for seg in 0..segments {
                let theta = std::f32::consts::TAU * seg as f32 / segments as f32;
                positions.push([phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()]);
            }
        }
        positions.push([0.0, -1.0, 0.0]);
        let bottom = positions.len() as u32 - 1;
        let at = |ring: u32, seg: u32| 1 + (ring - 1) * segments + seg % segments;
        let mut indices = Vec::new();
        for seg in 0..segments {
            indices.extend([0, at(1, seg + 1), at(1, seg)]);
            indices.extend([bottom, at(rings - 1, seg), at(rings - 1, seg + 1)]);
        }
        for ring in 1..rings - 1 {
            for seg in 0..segments {
                let (a, b) = (at(ring, seg), at(ring, seg + 1));
                let (c, d) = (at(ring + 1, seg), at(ring + 1, seg + 1));
                indices.extend([a, b, d, a, d, c]);
            }
        }
        claycore::Mesh::from_triangles(&positions, &indices).expect("a ball")
    }

    /// A Draw stroke along +x through `from`, into the open gesture.
    fn draw(adaptive: &mut Adaptive, from: [f32; 3]) -> bool {
        let path: Vec<[f32; 5]> = (0..=6)
            .map(|step| {
                [
                    from[0] + step as f32 * 0.1,
                    from[1],
                    from[2],
                    1.0,
                    step as f32,
                ]
            })
            .collect();
        let stamp = claycore::MeshStamp {
            verb: claycore::MeshBrush::Draw,
            radius: 0.6,
            strength: 0.8,
            ..claycore::MeshStamp::default()
        };
        adaptive
            .stroke(
                &[(path, stamp)],
                &claycore::StrokePreset::default(),
                &Adaptive::topology(),
                None,
            )
            .expect("the stroke")
    }

    fn assert_valid(adaptive: &Adaptive) {
        let validation = adaptive.surface().validate().expect("validate");
        assert!(validation.ok, "{}", validation.message);
    }

    #[test]
    fn a_restore_moves_the_generation_and_voids_the_drawn_region() {
        let mut adaptive = Adaptive::from_mesh(&sheet(4)).expect("a sheet reads");
        assert!(
            adaptive.is_chunked(),
            "an uncoloured sheet is drawn in chunks"
        );
        let before = adaptive.drawn_revision();
        let layout = adaptive.layout_revision();
        assert!(adaptive.lay_out_region(0, 0).is_some());
        assert!(
            matches!(
                adaptive.patch_region(),
                RegionPatch::Runs(runs) if runs.vertices.is_empty() && runs.indices.is_empty()
            ),
            "nothing moved, so nothing is written"
        );
        let bytes = adaptive.bytes(0).expect("bytes");
        adaptive.restore(&bytes).expect("restore");
        assert_eq!(adaptive.patch_region(), RegionPatch::Rebuild);
        assert_ne!(adaptive.drawn_revision(), before);
        assert_ne!(adaptive.layout_revision(), layout);
    }

    #[test]
    fn an_index_rebuild_renumbers_the_chunks_and_voids_the_region() {
        let mut adaptive = Adaptive::from_mesh(&sheet(16)).expect("a sheet reads");
        adaptive.lay_out_region(0, 0).expect("a region");
        let layout = adaptive.layout_revision();
        adaptive.rebuild_index().expect("rebuild");
        assert_ne!(
            adaptive.layout_revision(),
            layout,
            "the viewport lays it out again"
        );
        assert_eq!(adaptive.patch_region(), RegionPatch::Rebuild);
        let region = adaptive.lay_out_region(0, 0).expect("a region");
        assert_eq!(region.census.0 as u64, adaptive.stats().faces);
        assert!(adaptive.rebuild_micros() > 0, "and the cost was measured");
    }

    #[test]
    fn a_closed_surface_records_a_delta_and_an_open_one_a_snapshot() {
        let mut closed = Adaptive::from_mesh(&ball(8, 12)).expect("a ball reads");
        assert!(draw(&mut closed, [-0.3, 1.0, 0.0]));
        assert!(matches!(closed.close_gesture(), Some(Record::Delta(_))));

        let mut open = Adaptive::from_mesh(&sheet(4)).expect("a sheet reads");
        assert!(draw(&mut open, [1.0, 0.0, 1.0]));
        assert!(matches!(open.close_gesture(), Some(Record::Snapshot(_))));
    }

    /// Undo and redo land on the same digests however many times they are
    /// taken, and the structure stays valid at both ends — for both kinds.
    #[test]
    fn undo_redo_cycles_converge_on_either_kind_of_record() {
        for (mesh, from) in [(ball(8, 12), [-0.3, 1.0, 0.0]), (sheet(4), [1.0, 0.0, 1.0])] {
            let mut adaptive = Adaptive::from_mesh(&mesh).expect("reads");
            let before = adaptive.digest().expect("digest");
            assert!(draw(&mut adaptive, from));
            let mut record = adaptive.close_gesture().expect("a record");
            let after = adaptive.digest().expect("digest");
            assert_ne!(before, after, "the stroke changed the surface");
            assert!(record.weight() > 0, "the record is priced");
            for _ in 0..3 {
                record = adaptive.step(record, Replay::Revert).expect("undo");
                assert_eq!(adaptive.digest().expect("digest"), before);
                assert_valid(&adaptive);
                record = adaptive.step(record, Replay::Apply).expect("redo");
                assert_eq!(adaptive.digest().expect("digest"), after);
                assert_valid(&adaptive);
            }
        }
    }

    /// A dragging verb takes its segment back before laying the gesture down
    /// again, so the anchor it replays from is the surface the gesture found.
    #[test]
    fn a_replay_from_the_anchor_leaves_the_surface_as_the_gesture_found_it() {
        let mut adaptive = Adaptive::from_mesh(&ball(8, 12)).expect("a ball reads");
        let before = adaptive.digest().expect("digest");
        let revision = adaptive.drawn_revision();
        assert!(draw(&mut adaptive, [-0.3, 1.0, 0.0]));
        adaptive.replay_from_the_anchor().expect("replay");
        assert!(adaptive.gesture_is_open(), "the gesture is still open");
        assert_eq!(adaptive.digest().expect("digest"), before);
        assert_ne!(adaptive.drawn_revision(), revision, "the viewport knows");
        assert!(adaptive.close_gesture().is_none(), "nothing left to bank");
        assert_valid(&adaptive);
    }

    /// A delta replays through the surface's own sculptor, so an undo and a
    /// redo on a closed surface mark the chunks they touched and are patched
    /// into the drawn region like a stroke, rather than laying it out again.
    #[test]
    fn a_delta_undo_and_redo_patch_the_drawn_region_in_place() {
        let mut adaptive = Adaptive::from_mesh(&ball(96, 128)).expect("a ball reads");
        assert!(adaptive.is_chunked());
        adaptive.lay_out_region(0, 0).expect("a region");
        let before = adaptive.digest().expect("digest");
        assert!(draw(&mut adaptive, [-0.3, 1.0, 0.0]));
        let mut record = adaptive.close_gesture().expect("a record");
        assert!(matches!(record, Record::Delta(_)));
        assert!(matches!(adaptive.patch_region(), RegionPatch::Runs(_)));
        let layout = adaptive.layout_revision();

        for way in [Replay::Revert, Replay::Apply] {
            let revision = adaptive.drawn_revision();
            record = adaptive.step(record, way).expect("the step");
            assert_ne!(adaptive.drawn_revision(), revision, "the viewport knows");
            assert_eq!(adaptive.layout_revision(), layout, "and need not re-lay");
            let patch = adaptive.patch_region();
            let RegionPatch::Runs(runs) = patch else {
                panic!("the region should take the {way:?} in place: {patch:?}");
            };
            assert!(runs.chunks > 0, "the replay dirtied the chunks it touched");
            assert_eq!(runs.census[1].0 as u64, adaptive.stats().faces);
            assert_valid(&adaptive);
        }
        record = adaptive.step(record, Replay::Revert).expect("undo");
        assert!(matches!(record, Record::Delta(_)));
        assert_eq!(adaptive.digest().expect("digest"), before);
    }

    #[test]
    fn a_delta_does_not_replay_onto_another_surface() {
        let mut adaptive = Adaptive::from_mesh(&ball(8, 12)).expect("a ball reads");
        assert!(draw(&mut adaptive, [-0.3, 1.0, 0.0]));
        let record = adaptive.close_gesture().expect("a record");
        let mut other = Adaptive::from_mesh(&ball(8, 12)).expect("a ball reads");
        let untouched = other.digest().expect("digest");
        assert!(other.step(record, Replay::Revert).is_err());
        assert_eq!(other.digest().expect("digest"), untouched);
    }

    #[test]
    fn an_empty_mesh_is_refused_in_the_domains_words() {
        let empty = claycore::Mesh::from_triangles(&[], &[]);
        let Ok(empty) = empty else {
            // The engine refuses an empty mesh at construction, which is the
            // same answer one step earlier.
            return;
        };
        assert!(matches!(
            Adaptive::from_mesh(&empty),
            Err(ModelError::Conversion(Refusal::SourceEmpty))
        ));
    }

    #[test]
    fn the_side_car_round_trips_under_its_own_header() {
        let path = std::env::temp_dir().join(format!(
            "clayspace-adaptive-sidecar-{}.clayspace",
            std::process::id()
        ));
        let sidecar = sidecar_for(&path);
        assert!(sidecar.to_string_lossy().ends_with(".clayspace.dynamic"));
        let adaptive = Adaptive::from_mesh(&sheet(2)).expect("a sheet reads");
        let bytes = adaptive.bytes(0).expect("bytes");
        write_surfaces(
            &sidecar,
            &[crate::multires::Saved {
                position: 3,
                bytes: bytes.clone(),
            }],
        )
        .expect("write");
        let read = read_surfaces(&sidecar);
        assert!(read.faults.is_empty());
        assert_eq!(read.records.len(), 1);
        assert_eq!(read.records[0].position, 3);
        assert_eq!(read.records[0].bytes, bytes);
        // A hierarchy side-car reader does not accept this file.
        assert!(!crate::multires::read_hierarchies(&sidecar)
            .faults
            .is_empty());
        write_surfaces(&sidecar, &[]).expect("remove");
        assert!(!sidecar.exists());
    }
}
