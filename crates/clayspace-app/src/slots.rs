//! Where each brick's geometry sits in the GPU buffers.
//!
//! Without this the buffer is concatenated and written whole on every dab, so
//! the upload costs what the *model* costs rather than what the *edit* costs.
//! That is fine while meshing dominates and indefensible once it does not: on
//! the reference stroke, meshing a dab fell to 1.7 ms and rewriting the buffer
//! stayed at 3.1 ms, which is the moment the incremental path stopped being
//! incremental in the only place it still was.
//!
//! So each key gets a reserved span and keeps it. A dab writes only the spans
//! it touched, and everything else on screen is left alone.
//!
//! Two things make that safe. Geometry is already split per key with a local
//! vertex table — the engine welds vertices across brick seams, but by the
//! time it reaches here each key carries its own copy of whatever its
//! triangles reference, so a span can move without disturbing a neighbour.
//! And spans are allocated with headroom, because a re-meshed brick's vertex
//! count wobbles by a few either way and an exact fit would relocate on almost
//! every dab.
//!
//! A span that no longer fits is abandoned where it is and re-homed at the
//! end. The abandoned indices are made degenerate rather than removed, since
//! the surface is one draw call over one index buffer and a zero-area triangle
//! is the cheapest thing that draws nothing. Abandoned space accumulates, so
//! the caller compacts — a full rebuild — once enough of the buffer is holes.

use std::collections::HashMap;

use clayspace_engine::claycore::BrickKey;

/// One key's reserved span in each buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub vertex_base: u32,
    pub vertex_capacity: u32,
    pub index_base: u32,
    pub index_capacity: u32,
}

/// A half-open index range that must be filled with degenerate triangles.
pub type Blank = (u32, u32);

/// Where a key's geometry should be written, and what that displaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub slot: Slot,
    /// The span the key used to occupy, if this moved it.
    pub stranded: Option<Blank>,
}

/// The buffer layout: which key owns which span, and how much is holes.
#[derive(Debug, Default)]
pub struct SlotMap {
    slots: HashMap<BrickKey, Slot>,
    /// One past the highest index ever allocated — what the draw call covers.
    index_high: u32,
    vertex_high: u32,
    index_capacity: u32,
    vertex_capacity: u32,
    stranded_indices: u32,
}

/// Index spans are rounded to this, and it is a multiple of three on purpose.
///
/// The surface is one draw call over one index buffer, so the rasteriser
/// groups indices into triangles by position from the start of the range — it
/// has no idea spans exist. A span whose length is not a multiple of three
/// shifts every triangle boundary after it, and each following span gets its
/// triangles built from a mix of the previous span's padding and its own
/// indices. The surface still draws, and it is wrong everywhere at once:
/// speckle across the whole model rather than damage anywhere nameable.
const INDEX_GRAIN: u32 = 192;

/// Vertices have no such constraint — nothing groups them — so their spans
/// round to something smaller and waste less.
const VERTEX_GRAIN: u32 = 64;

/// Spare room in a span, so a brick re-meshing a few vertices larger stays put.
///
/// A quarter, rounded up to the grain so tiny spans still get some. A dab that
/// changes a brick's vertex count by a percent or two then costs one range
/// write instead of a relocation.
fn with_headroom(needed: u32, grain: u32) -> u32 {
    let padded = needed + needed / 4;
    padded.next_multiple_of(grain).max(grain)
}

impl SlotMap {
    /// An empty layout over buffers of the given capacity.
    pub fn new(vertex_capacity: u32, index_capacity: u32) -> Self {
        Self {
            vertex_capacity,
            index_capacity,
            ..Default::default()
        }
    }

    /// How many indices the draw call must cover.
    pub fn index_count(&self) -> u32 {
        self.index_high
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_high
    }

    /// The fraction of the drawn index range that is holes.
    ///
    /// Holes are degenerate triangles, so they cost vertex shading on every
    /// frame while an edit costs a write once. Past some point the trade stops
    /// being worth it and the caller compacts.
    pub fn waste(&self) -> f32 {
        if self.index_high == 0 {
            return 0.0;
        }
        self.stranded_indices as f32 / self.index_high as f32
    }

    pub fn get(&self, key: BrickKey) -> Option<Slot> {
        self.slots.get(&key).copied()
    }

    /// Reserve room for a key's geometry, moving it only if it no longer fits.
    ///
    /// `None` means the buffers are full and the caller must rebuild.
    pub fn place(&mut self, key: BrickKey, vertices: u32, indices: u32) -> Option<Placed> {
        if let Some(slot) = self.slots.get(&key) {
            if vertices <= slot.vertex_capacity && indices <= slot.index_capacity {
                return Some(Placed {
                    slot: *slot,
                    stranded: None,
                });
            }
        }

        let slot = Slot {
            vertex_base: self.vertex_high,
            vertex_capacity: with_headroom(vertices, VERTEX_GRAIN),
            index_base: self.index_high,
            index_capacity: with_headroom(indices, INDEX_GRAIN),
        };
        if slot.vertex_base + slot.vertex_capacity > self.vertex_capacity
            || slot.index_base + slot.index_capacity > self.index_capacity
        {
            return None;
        }
        self.vertex_high += slot.vertex_capacity;
        self.index_high += slot.index_capacity;

        let stranded = self.strand(key);
        self.slots.insert(key, slot);
        Some(Placed { slot, stranded })
    }

    /// Give up a key's span entirely: the surface has left that brick.
    pub fn remove(&mut self, key: BrickKey) -> Option<Blank> {
        self.strand(key)
    }

    /// Drop a key's span and count it as a hole.
    fn strand(&mut self, key: BrickKey) -> Option<Blank> {
        let old = self.slots.remove(&key)?;
        self.stranded_indices += old.index_capacity;
        Some((old.index_base, old.index_base + old.index_capacity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(x: i32) -> BrickKey {
        [x, 0, 0]
    }

    /// Room enough that nothing in these tests runs out.
    fn roomy() -> SlotMap {
        SlotMap::new(1 << 20, 1 << 20)
    }

    #[test]
    fn spans_do_not_overlap() {
        // The property everything else depends on: two keys never share a
        // byte, or one dab silently corrupts another brick.
        let mut map = roomy();
        let mut spans: Vec<(u32, u32)> = Vec::new();
        for i in 0..32 {
            let placed = map.place(key(i), 100 + i as u32, 300).expect("room");
            spans.push((
                placed.slot.vertex_base,
                placed.slot.vertex_base + placed.slot.vertex_capacity,
            ));
        }
        spans.sort_unstable();
        for pair in spans.windows(2) {
            assert!(
                pair[0].1 <= pair[1].0,
                "spans {:?} and {:?} overlap",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn a_brick_that_grows_a_little_stays_put() {
        // The whole reason for headroom. A re-meshed brick's vertex count
        // wobbles; if every wobble relocated, the buffer would fill with holes
        // in a few strokes and compact constantly.
        let mut map = roomy();
        let first = map.place(key(0), 100, 300).expect("room").slot;
        for size in [101, 105, 110, 120] {
            let again = map.place(key(0), size, 300).expect("room");
            assert_eq!(again.slot, first, "{size} vertices relocated the brick");
            assert_eq!(again.stranded, None);
        }
    }

    #[test]
    fn a_brick_that_outgrows_its_span_moves_and_leaves_a_hole() {
        let mut map = roomy();
        let first = map.place(key(0), 100, 300).expect("room").slot;
        let moved = map.place(key(0), 5_000, 15_000).expect("room");

        assert_ne!(moved.slot.vertex_base, first.vertex_base, "it did not move");
        assert_eq!(
            moved.stranded,
            Some((first.index_base, first.index_base + first.index_capacity)),
            "the abandoned span was not reported, so it would still be drawn"
        );
        assert!(map.waste() > 0.0);
    }

    #[test]
    fn a_removed_brick_reports_the_span_to_blank() {
        let mut map = roomy();
        let slot = map.place(key(0), 100, 300).expect("room").slot;
        assert_eq!(
            map.remove(key(0)),
            Some((slot.index_base, slot.index_base + slot.index_capacity))
        );
        assert_eq!(map.get(key(0)), None);
        // And removing it twice is not a second hole.
        assert_eq!(map.remove(key(0)), None);
    }

    #[test]
    fn a_full_buffer_asks_to_be_rebuilt_rather_than_overrunning() {
        // Returning a span past the end of the buffer would be a GPU write out
        // of bounds, so running out has to be a refusal.
        let mut map = SlotMap::new(256, 256);
        assert!(map.place(key(0), 100, 100).is_some());
        assert!(
            map.place(key(1), 10_000, 10_000).is_none(),
            "it handed out a span the buffer cannot hold"
        );
    }

    #[test]
    fn waste_is_the_share_of_the_drawn_range_that_is_holes() {
        let mut map = roomy();
        assert_eq!(map.waste(), 0.0, "an empty layout wastes nothing");
        map.place(key(0), 100, 300).expect("room");
        assert_eq!(map.waste(), 0.0, "a fresh span is not a hole");

        // Move it, and everything it used to occupy is a hole.
        map.place(key(0), 9_000, 9_000).expect("room");
        let stranded = map.waste() * map.index_count() as f32;
        assert!(stranded > 0.0 && map.waste() < 1.0, "waste {}", map.waste());
    }

    #[test]
    fn index_spans_stay_on_triangle_boundaries() {
        // The regression. Indices are grouped into triangles by position from
        // the start of the draw range, so a span that begins or ends off a
        // multiple of three re-cuts every triangle after it. It rendered as
        // speckle over the entire surface — no missing region, no seam, just
        // wrong everywhere.
        let mut map = roomy();
        for i in 0..24 {
            let placed = map.place(key(i), 40 + i as u32 * 7, 91 + i as u32 * 13);
            let slot = placed.expect("room").slot;
            assert_eq!(slot.index_base % 3, 0, "span {i} starts mid-triangle");
            assert_eq!(slot.index_capacity % 3, 0, "span {i} ends mid-triangle");
        }
        assert_eq!(
            map.index_count() % 3,
            0,
            "the draw range is not whole triangles"
        );
    }

    #[test]
    fn a_moved_span_still_lands_on_a_triangle_boundary() {
        let mut map = roomy();
        map.place(key(0), 100, 300).expect("room");
        map.place(key(1), 100, 300).expect("room");
        let moved = map.place(key(0), 9_000, 27_000).expect("room").slot;
        assert_eq!(moved.index_base % 3, 0);
        assert_eq!(moved.index_capacity % 3, 0);
    }

    #[test]
    fn the_drawn_range_covers_every_live_span() {
        // The draw call is one range, so any span past its end is invisible.
        let mut map = roomy();
        for i in 0..16 {
            let placed = map.place(key(i), 100, 300).expect("room");
            assert!(
                placed.slot.index_base + placed.slot.index_capacity <= map.index_count(),
                "a span was allocated past what the draw call covers"
            );
        }
    }
}
