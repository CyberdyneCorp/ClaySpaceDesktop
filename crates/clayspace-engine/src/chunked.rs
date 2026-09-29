//! An adaptive surface drawn chunk by chunk.
//!
//! A Dynamic stroke splits, collapses and flips triangles under the brush, so
//! the surface has no index buffer that stays valid for a stroke. Copying the
//! whole surface out whenever it moved — what this application did first — is
//! correct and costs what the *model* costs on every dab. The engine partitions
//! the surface into chunks and reports which ones a stamp touched
//! (`clay_dynamic_surface_dirty_chunks`), so the cost can follow the edit.
//!
//! Two pieces, kept apart because they answer different questions:
//!
//! - [`ChunkMirror`] is this side's copy of every chunk, drained from the
//!   engine's dirty set. Its buffers are kept and reused, so a stroke that
//!   does not grow a chunk allocates nothing, and every drain records *what
//!   kind* of change each chunk saw: a topology change needs its indices sent
//!   again, a geometry change needs only its vertices.
//! - [`Region`] is where those chunks sit in the carried buffer the viewport
//!   draws. Each chunk has a slot with headroom (the brick path's
//!   [`SlotMap`]), so a dab rewrites the slots of the chunks it touched and
//!   nothing else; a chunk that outgrows its slot moves to the region's spare
//!   room and leaves degenerate triangles behind.
//!
//! Chunks copy out of the engine as **unwelded triangles** — its own format,
//! because the topology is changing under the copy — and are welded here,
//! per chunk, on exact position and normal: a corner shared by six triangles
//! is one vertex again, which is what keeps the drawn buffer near the size a
//! whole welded export had rather than three corners per triangle. Whether a
//! chunk's indices changed is then read off the welded indices themselves, so
//! a chunk is re-sent with indices exactly when the triangles it draws are
//! different ones.
//!
//! The transport carries positions and normals only. There is no per-chunk
//! colour, which is why a coloured surface is still drawn whole (see
//! [`crate::adaptive`]).

use std::collections::{BTreeMap, HashMap};

use claycore::{ClayError, DynamicSculptor};

use crate::slots::{index_span, vertex_span, Blank, Slot, SlotMap};

/// A box, as `(min, max)`.
pub type Bounds = ([f32; 3], [f32; 3]);

/// One chunk as it was last copied. The vectors are kept across drains.
#[derive(Debug, Default)]
struct Mirrored {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
    bounds: Option<Bounds>,
}

impl Mirrored {
    fn is_live(&self) -> bool {
        !self.indices.is_empty()
    }

    fn clear(&mut self) {
        self.positions.clear();
        self.normals.clear();
        self.indices.clear();
        self.bounds = None;
    }
}

/// What one chunk needs sent again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkChange {
    /// The same triangles, moved: vertices and normals, no indices.
    Geometry,
    /// Different triangles: the chunk's indices and vertices both.
    Topology,
    /// The chunk no longer holds anything.
    Removed,
}

impl ChunkChange {
    /// Two changes to one chunk between uploads, as one.
    ///
    /// Whatever the last one left is what has to be drawn, and a slot that was
    /// emptied or re-cut needs its indices written whatever came after it.
    fn then(self, next: ChunkChange) -> ChunkChange {
        match (self, next) {
            (_, ChunkChange::Removed) => ChunkChange::Removed,
            (ChunkChange::Geometry, ChunkChange::Geometry) => ChunkChange::Geometry,
            _ => ChunkChange::Topology,
        }
    }
}

/// Buffers one copy is made through, kept and reused across copies.
#[derive(Debug, Default)]
struct Scratch {
    /// The chunk as the engine writes it: three corners per triangle.
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
    /// The welded indices being built, compared with the chunk's own.
    welded: Vec<u32>,
    /// Which welded vertex each distinct corner became.
    seen: HashMap<([u32; 3], [u32; 3]), u32>,
}

/// This side's copy of every chunk of one surface.
#[derive(Debug, Default)]
pub struct ChunkMirror {
    chunks: Vec<Mirrored>,
    /// Every chunk changed since the region last took them, merged per chunk.
    pending: BTreeMap<u32, ChunkChange>,
    scratch: Scratch,
    /// How many times a buffer had to grow, over the mirror's life.
    grown: u64,
}

impl ChunkMirror {
    /// Copies every chunk afresh and forgets what was pending.
    ///
    /// For a sculptor whose chunk table this mirror has not seen: one just
    /// made over a surface put back from bytes, or the first drawing of a
    /// surface. Clears the engine's dirty set too, since everything in it has
    /// just been copied.
    pub fn refill(&mut self, sculptor: &mut DynamicSculptor<'_>) -> Result<(), ClayError> {
        let count = sculptor.chunk_count();
        self.chunks.truncate(count);
        self.chunks.resize_with(count, Mirrored::default);
        for id in 0..count {
            self.copy(sculptor, id)?;
        }
        self.pending.clear();
        sculptor.clear_dirty()
    }

    /// Copies the chunks the engine marked dirty, and retires the dirty set.
    ///
    /// Returns how many chunks were copied. What each saw is kept until
    /// [`Region::patch`] takes it, so a caller that drains for another reason
    /// — the bounds after a stroke — does not lose it.
    pub fn drain(&mut self, sculptor: &mut DynamicSculptor<'_>) -> Result<usize, ClayError> {
        let dirty = sculptor.dirty_chunks()?;
        for &id in &dirty {
            let change = self.copy(sculptor, id as usize)?;
            let merged = match self.pending.get(&id) {
                Some(earlier) => earlier.then(change),
                None => change,
            };
            self.pending.insert(id, merged);
        }
        sculptor.clear_dirty()?;
        Ok(dirty.len())
    }

    /// Copies one chunk into its kept buffers and says what kind of change it
    /// was against what was there.
    fn copy(
        &mut self,
        sculptor: &DynamicSculptor<'_>,
        id: usize,
    ) -> Result<ChunkChange, ClayError> {
        if id >= self.chunks.len() {
            self.chunks.resize_with(id + 1, Mirrored::default);
        }
        let info = sculptor.chunk_info(id)?;
        if info.index_count == 0 {
            self.chunks[id].clear();
            return Ok(ChunkChange::Removed);
        }
        let scratch = &mut self.scratch;
        self.grown += fit(&mut scratch.positions, info.vertex_count as usize)
            + fit(&mut scratch.normals, info.vertex_count as usize)
            + fit(&mut scratch.indices, info.index_count as usize);
        let written = sculptor.copy_chunk_into(
            id,
            &mut scratch.positions,
            &mut scratch.normals,
            &mut scratch.indices,
        )?;
        scratch.positions.truncate(written.vertex_count as usize);
        scratch.normals.truncate(written.vertex_count as usize);
        scratch.indices.truncate(written.index_count as usize);
        let chunk = &mut self.chunks[id];
        self.grown += weld(scratch, chunk);
        chunk.bounds = Some((written.bounds_min, written.bounds_max));
        if scratch.welded == chunk.indices {
            return Ok(ChunkChange::Geometry);
        }
        // Copied rather than swapped, so the scratch keeps the capacity it
        // has grown to and the next copy does not grow it again.
        let capacity = chunk.indices.capacity();
        chunk.indices.clear();
        chunk.indices.extend_from_slice(&scratch.welded);
        self.grown += u64::from(chunk.indices.capacity() > capacity);
        Ok(ChunkChange::Topology)
    }

    /// Drops what is pending, for a layout that has just written every chunk.
    pub fn forget_pending(&mut self) {
        self.pending.clear();
    }

    /// The chunks that hold something, by id.
    fn live(&self) -> impl Iterator<Item = (u32, &Mirrored)> {
        self.chunks
            .iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.is_live())
            .map(|(id, chunk)| (id as u32, chunk))
    }

    /// How many times a chunk's buffer has had to grow.
    ///
    /// A stroke that does not grow the surface leaves this where it was: the
    /// buffers are sized once to what a chunk holds and copied into after.
    pub fn buffer_growths(&self) -> u64 {
        self.grown
    }

    /// Chunks that hold something.
    pub fn live_chunks(&self) -> usize {
        self.live().count()
    }

    /// The box every live chunk occupies, from the engine's own chunk bounds.
    pub fn bounds(&self) -> Option<Bounds> {
        self.live()
            .filter_map(|(_, chunk)| chunk.bounds)
            .reduce(|(min, max), (low, high)| {
                (
                    std::array::from_fn(|i| min[i].min(low[i])),
                    std::array::from_fn(|i| max[i].max(high[i])),
                )
            })
    }

    /// Every live chunk's triangles, for a caller walking them — a pick.
    pub fn triangles(&self) -> impl Iterator<Item = (&[[f32; 3]], &[u32], Option<Bounds>)> {
        self.live().map(|(_, chunk)| {
            (
                chunk.positions.as_slice(),
                chunk.indices.as_slice(),
                chunk.bounds,
            )
        })
    }

    /// Triangles and vertices the live chunks hold.
    pub fn census(&self) -> (usize, usize) {
        self.live()
            .fold((0, 0), |(triangles, vertices), (_, chunk)| {
                (
                    triangles + chunk.indices.len() / 3,
                    vertices + chunk.positions.len(),
                )
            })
    }
}

/// Sizes a kept buffer to `len`, and says whether that took an allocation.
fn fit<T: Copy + Default>(buffer: &mut Vec<T>, len: usize) -> u64 {
    let grew = u64::from(len > buffer.capacity());
    buffer.resize(len, T::default());
    grew
}

/// Welds the corners in `scratch` into `chunk`'s vertices, on exact position
/// and normal, leaving the welded indices in `scratch.welded` for the caller
/// to compare with the chunk's own. Returns how many buffers had to grow.
///
/// Vertices are numbered in order of first use, so the same triangles in the
/// same order weld to the same indices however far they moved.
fn weld(scratch: &mut Scratch, chunk: &mut Mirrored) -> u64 {
    let capacities = (
        chunk.positions.capacity(),
        scratch.welded.capacity(),
        scratch.seen.capacity(),
    );
    scratch.seen.clear();
    scratch.welded.clear();
    chunk.positions.clear();
    chunk.normals.clear();
    for &corner in &scratch.indices {
        let (position, normal) = (
            scratch.positions[corner as usize],
            scratch.normals[corner as usize],
        );
        let key = (position.map(f32::to_bits), normal.map(f32::to_bits));
        let next = chunk.positions.len() as u32;
        let vertex = *scratch.seen.entry(key).or_insert(next);
        if vertex == next {
            chunk.positions.push(position);
            chunk.normals.push(normal);
        }
        scratch.welded.push(vertex);
    }
    u64::from(chunk.positions.capacity() > capacities.0)
        + u64::from(scratch.welded.capacity() > capacities.1)
        + u64::from(scratch.seen.capacity() > capacities.2)
}

// -- where the chunks sit in the carried buffer ------------------------------

/// A run of vertices to write, starting at `first` in the carried buffer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VertexRun {
    pub first: u32,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
}

/// A run of indices to write, starting at `first`, already in the carried
/// buffer's numbering.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IndexRun {
    pub first: u32,
    pub indices: Vec<u32>,
}

/// What one surface's region needs written after a drain.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RegionRuns {
    pub vertices: Vec<VertexRun>,
    pub indices: Vec<IndexRun>,
    /// The box the rewritten chunks occupy, in the surface's own coordinates.
    pub bounds: Option<Bounds>,
    /// Chunks whose data is in these runs.
    pub chunks: usize,
    /// Triangles and vertices of surface the region drew before these runs,
    /// and after them.
    pub census: [(usize, usize); 2],
}

impl RegionRuns {
    fn grow_bounds(&mut self, bounds: Option<Bounds>) {
        let Some((low, high)) = bounds else {
            return;
        };
        self.bounds = Some(match self.bounds {
            None => (low, high),
            Some((min, max)) => (
                std::array::from_fn(|i| min[i].min(low[i])),
                std::array::from_fn(|i| max[i].max(high[i])),
            ),
        });
    }
}

/// One whole region, laid out afresh, in region-local numbering.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RegionBuffers {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Triangles and vertices that are surface rather than headroom.
    pub census: (usize, usize),
    /// Chunks written.
    pub chunks: usize,
}

/// The least spare room a region is laid out with, in triangles (and as
/// many vertices, which a welded chunk never exceeds): 16,384, about 0.8 MB
/// of vertex and index data.
const SPARE_TRIANGLES: u32 = 16_384;

/// The share of a region's drawn range that may be holes before a patch is
/// declined and the caller lays the region out again.
const MAX_WASTE: f32 = 0.5;

/// Where one surface's chunks sit in the carried buffer.
///
/// Padding — the headroom in each slot, the spare room at the end and the
/// slots a moved chunk left — is degenerate triangles on the region's first
/// vertex, which is a real vertex of the surface: a zero-area triangle draws
/// nothing, and one on a real vertex does not stretch a bounding box to the
/// origin the way a zeroed vertex would.
#[derive(Debug)]
pub struct Region {
    slots: SlotMap<u32>,
    /// Where the region starts in the carried buffer.
    vertex_base: u32,
    index_base: u32,
    /// The surface generation it was laid out for; a surface put back from
    /// bytes is a new chunk table, and a region over the old one is void.
    generation: u64,
    /// Triangles and vertices of surface the region draws now.
    census: (usize, usize),
}

impl Region {
    /// Lays every live chunk out afresh, with room to grow, starting at the
    /// given place in the carried buffer.
    ///
    /// `None` for a surface with no live chunk, which draws nothing.
    pub fn lay_out(
        mirror: &ChunkMirror,
        vertex_base: u32,
        index_base: u32,
        generation: u64,
    ) -> Option<(Self, RegionBuffers)> {
        let (vertices, indices) = mirror.live().fold((0u32, 0u32), |(v, i), (_, chunk)| {
            (
                v + vertex_span(chunk.positions.len() as u32),
                i + index_span(chunk.indices.len() as u32),
            )
        });
        if indices == 0 {
            return None;
        }
        // Spare room at the end, where a chunk that outgrows its slot moves
        // to: an eighth again, and never less than a stroke's worth of new
        // triangles, since a small coarse surface is exactly the one whose
        // first strokes refine the most. Past it the region is laid out again.
        let vertex_capacity = vertices + vertex_span((vertices / 8).max(SPARE_TRIANGLES));
        let index_capacity = indices + index_span((indices / 8).max(3 * SPARE_TRIANGLES));
        let mut region = Self {
            slots: SlotMap::new(vertex_capacity, index_capacity),
            vertex_base,
            index_base,
            generation,
            census: mirror.census(),
        };
        let (pad_position, pad_normal) = mirror
            .live()
            .next()
            .map(|(_, chunk)| (chunk.positions[0], chunk.normals[0]))?;
        let mut buffers = RegionBuffers {
            positions: vec![pad_position; vertex_capacity as usize],
            normals: vec![pad_normal; vertex_capacity as usize],
            indices: vec![0; index_capacity as usize],
            census: mirror.census(),
            chunks: mirror.live_chunks(),
        };
        for (id, chunk) in mirror.live() {
            let slot = region
                .slots
                .place(id, chunk.positions.len() as u32, chunk.indices.len() as u32)?
                .slot;
            write_chunk(&mut buffers, slot, chunk);
        }
        Some((region, buffers))
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The runs that bring the drawn region up to the mirror, taking what the
    /// mirror has pending.
    ///
    /// `None` when the region cannot take it — a chunk outgrew the spare
    /// room, or too much of the region is holes — and the caller lays the
    /// region out again from the mirror, which already holds everything.
    pub fn patch(&mut self, mirror: &mut ChunkMirror) -> Option<RegionRuns> {
        let pending = std::mem::take(&mut mirror.pending);
        let mut runs = RegionRuns::default();
        for (id, change) in pending {
            let chunk = &mirror.chunks[id as usize];
            match (change, self.slots.get(id)) {
                (ChunkChange::Removed, _) => {
                    if let Some(blank) = self.slots.remove(id) {
                        runs.indices.push(self.blank(blank));
                    }
                }
                (ChunkChange::Geometry, Some(slot)) => {
                    runs.vertices.push(self.vertex_run(slot, chunk));
                    runs.grow_bounds(chunk.bounds);
                    runs.chunks += 1;
                }
                _ => self.replace(id, chunk, &mut runs)?,
            }
        }
        runs.census = [self.census, mirror.census()];
        self.census = runs.census[1];
        (self.slots.waste() <= MAX_WASTE).then_some(runs)
    }

    /// A chunk whose triangles changed: placed again — where it was if it
    /// still fits — with its vertices, its whole index slot and any slot it
    /// left behind written.
    fn replace(&mut self, id: u32, chunk: &Mirrored, runs: &mut RegionRuns) -> Option<()> {
        let placed =
            self.slots
                .place(id, chunk.positions.len() as u32, chunk.indices.len() as u32)?;
        if let Some(blank) = placed.stranded {
            runs.indices.push(self.blank(blank));
        }
        let slot = placed.slot;
        runs.vertices.push(self.vertex_run(slot, chunk));
        let base = self.vertex_base + slot.vertex_base;
        let mut indices: Vec<u32> = chunk.indices.iter().map(|i| i + base).collect();
        indices.resize(slot.index_capacity as usize, self.vertex_base);
        runs.indices.push(IndexRun {
            first: self.index_base + slot.index_base,
            indices,
        });
        runs.grow_bounds(chunk.bounds);
        runs.chunks += 1;
        Some(())
    }

    fn vertex_run(&self, slot: Slot, chunk: &Mirrored) -> VertexRun {
        VertexRun {
            first: self.vertex_base + slot.vertex_base,
            positions: chunk.positions.clone(),
            normals: chunk.normals.clone(),
        }
    }

    /// A released span, as degenerate triangles on the region's first vertex.
    fn blank(&self, (start, end): Blank) -> IndexRun {
        IndexRun {
            first: self.index_base + start,
            indices: vec![self.vertex_base; (end - start) as usize],
        }
    }
}

/// Writes one chunk into a fresh region's buffers at its slot.
fn write_chunk(buffers: &mut RegionBuffers, slot: Slot, chunk: &Mirrored) {
    let at = slot.vertex_base as usize;
    let count = chunk.positions.len();
    buffers.positions[at..at + count].copy_from_slice(&chunk.positions);
    buffers.normals[at..at + count].copy_from_slice(&chunk.normals);
    let first = slot.index_base as usize;
    for (into, index) in buffers.indices[first..first + chunk.indices.len()]
        .iter_mut()
        .zip(&chunk.indices)
    {
        *into = index + slot.vertex_base;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(triangles: usize, at: f32) -> Mirrored {
        let positions: Vec<[f32; 3]> = (0..triangles * 3)
            .map(|i| [at + i as f32, (i % 3) as f32, 0.0])
            .collect();
        Mirrored {
            normals: vec![[0.0, 0.0, 1.0]; positions.len()],
            indices: (0..positions.len() as u32).collect(),
            bounds: Some(([at, 0.0, 0.0], [at + positions.len() as f32, 2.0, 0.0])),
            positions,
        }
    }

    fn mirror(sizes: &[usize]) -> ChunkMirror {
        ChunkMirror {
            chunks: sizes
                .iter()
                .enumerate()
                .map(|(id, &n)| chunk(n, id as f32 * 1000.0))
                .collect(),
            ..ChunkMirror::default()
        }
    }

    /// The non-degenerate triangles a buffer draws, as position triples.
    fn drawn(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[[u32; 3]; 3]> {
        let mut triangles: Vec<[[u32; 3]; 3]> = indices
            .chunks_exact(3)
            .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
            .map(|t| [t[0], t[1], t[2]].map(|i| positions[i as usize].map(f32::to_bits)))
            .collect();
        triangles.sort_unstable();
        triangles
    }

    fn apply(buffers: &mut RegionBuffers, runs: &RegionRuns) {
        for run in &runs.vertices {
            let at = run.first as usize;
            buffers.positions[at..at + run.positions.len()].copy_from_slice(&run.positions);
            buffers.normals[at..at + run.normals.len()].copy_from_slice(&run.normals);
        }
        for run in &runs.indices {
            let at = run.first as usize;
            buffers.indices[at..at + run.indices.len()].copy_from_slice(&run.indices);
        }
    }

    #[test]
    fn a_region_draws_every_live_chunk_and_nothing_else() {
        let mirror = mirror(&[4, 0, 7]);
        let (_, buffers) = Region::lay_out(&mirror, 0, 0, 0).expect("a region");
        assert_eq!(buffers.census, (11, 33));
        assert_eq!(drawn(&buffers.positions, &buffers.indices).len(), 11);
        assert_eq!(buffers.indices.len() % 3, 0, "whole triangles only");
    }

    #[test]
    fn a_geometry_change_writes_vertices_and_no_indices() {
        let mut mirror = mirror(&[4, 5, 6]);
        let (mut region, _) = Region::lay_out(&mirror, 0, 0, 0).expect("a region");
        mirror.chunks[1].positions[0][2] = 3.0;
        mirror.pending.insert(1, ChunkChange::Geometry);
        let runs = region.patch(&mut mirror).expect("it fits");
        assert_eq!(runs.chunks, 1);
        assert_eq!(runs.vertices.len(), 1);
        assert_eq!(runs.vertices[0].positions.len(), 15);
        assert!(runs.indices.is_empty(), "no index is sent for a move");
    }

    #[test]
    fn patched_regions_draw_what_a_fresh_layout_draws() {
        let mut mirror = mirror(&[4, 5, 6, 3, 4, 4, 4, 4, 4, 4, 4, 4]);
        let (mut region, mut buffers) = Region::lay_out(&mirror, 0, 0, 0).expect("a region");
        // Grow one chunk past its slot, shrink another, empty a third.
        mirror.chunks[0] = chunk(30, 0.0);
        mirror.chunks[2] = chunk(2, 2000.0);
        mirror.chunks[3].clear();
        for (id, change) in [
            (0, ChunkChange::Topology),
            (2, ChunkChange::Topology),
            (3, ChunkChange::Removed),
        ] {
            mirror.pending.insert(id, change);
        }
        let runs = region
            .patch(&mut mirror)
            .expect("it fits in the spare room");
        apply(&mut buffers, &runs);
        let (_, fresh) = Region::lay_out(&mirror, 0, 0, 0).expect("a region");
        assert_eq!(
            drawn(&buffers.positions, &buffers.indices),
            drawn(&fresh.positions, &fresh.indices),
        );
    }

    #[test]
    fn a_chunk_past_the_spare_room_asks_for_a_new_layout() {
        let mut mirror = mirror(&[4, 4]);
        let (mut region, _) = Region::lay_out(&mirror, 0, 0, 0).expect("a region");
        mirror.chunks[0] = chunk(40_000, 0.0);
        mirror.pending.insert(0, ChunkChange::Topology);
        assert!(region.patch(&mut mirror).is_none());
    }

    #[test]
    fn a_region_placed_later_in_the_buffer_numbers_from_there() {
        let mut mirror = mirror(&[4, 4]);
        let (mut region, _) = Region::lay_out(&mirror, 100, 300, 0).expect("a region");
        mirror.pending.insert(1, ChunkChange::Topology);
        let runs = region.patch(&mut mirror).expect("it fits");
        assert!(runs.vertices.iter().all(|run| run.first >= 100));
        assert!(runs.indices.iter().all(|run| run.first >= 300));
        assert!(runs
            .indices
            .iter()
            .flat_map(|run| &run.indices)
            .all(|&i| i >= 100));
    }

    #[test]
    fn changes_merge_to_what_the_last_one_left() {
        use ChunkChange::*;
        assert_eq!(Geometry.then(Geometry), Geometry);
        assert_eq!(Geometry.then(Topology), Topology);
        assert_eq!(Topology.then(Geometry), Topology);
        assert_eq!(Topology.then(Removed), Removed);
        assert_eq!(Removed.then(Geometry), Topology);
    }
}
