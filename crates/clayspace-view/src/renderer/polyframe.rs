//! The polyframe's line list, and how it follows a chunk patch.
//!
//! The lines of an ordinary subtool are its triangles' unique edges, packed:
//! an edge shared by two triangles is emitted once, since the lines are drawn
//! translucent and an edge emitted twice would read darker than a boundary.
//! Packing makes every line's position depend on every triangle before it, so
//! a subtool whose triangles change has its whole line list derived again.
//!
//! An adaptive surface's span is laid out in chunk slots instead
//! ([`MeshSpan::chunked`]), and a stroke rewrites whole slots in place. Its
//! lines are laid out in the same slots: each index position owns two line
//! positions, so each triangle owns six — its three edges, or a degenerate
//! stand-in where the edge is not its to draw. That keeps a slot's lines at a
//! fixed place and derivable from the slot alone, so a patch that rewrites a
//! slot's triangles rewrites exactly its lines beside them.
//!
//! Deduplicating within a slot is the same as deduplicating over the span,
//! because slots never share a vertex: each chunk carries its own copy of the
//! vertices its triangles reference. So a patched line list equals the one a
//! fresh derivation would give for the same slots.

use std::collections::HashSet;
use std::ops::Range;

use super::MeshSpan;

/// A line list and where in it each chunked span's lines start.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lines {
    /// Two vertex indices per line.
    pub indices: Vec<u32>,
    pub layout: LineLayout,
}

/// Where the lines of each chunked span sit, so a patch to its triangles can
/// be followed by a patch to its lines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LineLayout {
    slotted: Vec<Slotted>,
}

/// One chunked span: its index range, and where its lines begin.
#[derive(Debug, Clone, PartialEq)]
struct Slotted {
    indices: Range<u32>,
    lines: u32,
}

impl LineLayout {
    /// The lines for a run of rewritten indices, and where in the line list
    /// they go; `None` where the run does not lie whole triangles deep inside
    /// one chunked span, and the lines have to be derived again.
    pub fn patch(&self, first: u32, run: &[u32]) -> Option<(u32, Vec<u32>)> {
        let end = first as usize + run.len();
        let span = self
            .slotted
            .iter()
            .find(|span| span.indices.start <= first && end <= span.indices.end as usize)?;
        let offset = first - span.indices.start;
        if offset % 3 != 0 || run.len() % 3 != 0 {
            return None;
        }
        Some((span.lines + 2 * offset, slotted_lines(run)))
    }
}

/// The line list for the triangles in `indices`, per span.
///
/// **Authored edges where a subtool has them, derived where it does not.**
/// Deriving from a triangle list cannot recover a quad: a quad's two
/// triangles share a diagonal that is not one of its four edges, and the
/// triangle list does not say which of each triangle's three edges is the
/// invented one. So a retopologised layer drawn by derivation showed every
/// diagonal and a 100%-quad mesh read as triangles — reported from a session,
/// and the reason spans carry `edges` at all.
///
/// Mixed scenes are the ordinary case rather than a corner: a retopology
/// places its result *beside* the source, so the very first thing a sculptor
/// sees is one layer of each kind. An all-or-nothing rule would have failed
/// exactly there.
///
/// A chunked span is laid out slot by slot; see the module notes.
pub fn lines(indices: &[u32], spans: &[MeshSpan]) -> Lines {
    let mut packed = Packed::with_capacity(indices.len());
    let mut slotted = Vec::new();

    if spans.is_empty() {
        // No spans is every caller that predates them, and a whole-buffer
        // derivation is what they got before.
        packed.derive(indices);
        return Lines {
            indices: packed.lines,
            layout: LineLayout::default(),
        };
    }

    for span in spans {
        let range = span.indices.start as usize..span.indices.end as usize;
        match (&span.edges, indices.get(range)) {
            (Some(authored), _) => {
                for edge in authored.chunks_exact(2) {
                    packed.push(edge[0], edge[1]);
                }
            }
            (None, Some(triangles)) if span.chunked => {
                slotted.push(Slotted {
                    indices: span.indices.clone(),
                    lines: packed.lines.len() as u32,
                });
                packed.lines.extend(slotted_lines(triangles));
            }
            (None, Some(triangles)) => packed.derive(triangles),
            (None, None) => {}
        }
    }
    Lines {
        indices: packed.lines,
        layout: LineLayout { slotted },
    }
}

/// Unique edges, packed.
struct Packed {
    seen: HashSet<(u32, u32)>,
    lines: Vec<u32>,
}

impl Packed {
    fn with_capacity(indices: usize) -> Self {
        Self {
            seen: HashSet::with_capacity(indices),
            lines: Vec::with_capacity(indices),
        }
    }

    /// Ordered, so the same edge reached from either of its two faces is the
    /// same key.
    fn push(&mut self, a: u32, b: u32) {
        let key = if a < b { (a, b) } else { (b, a) };
        if self.seen.insert(key) {
            self.lines.extend([key.0, key.1]);
        }
    }

    fn derive(&mut self, triangles: &[u32]) {
        for triangle in triangles.chunks_exact(3) {
            self.push(triangle[0], triangle[1]);
            self.push(triangle[1], triangle[2]);
            self.push(triangle[2], triangle[0]);
        }
    }
}

/// Six line indices per triangle, in triangle order: each of its three edges
/// that no earlier triangle of the run drew, and a zero-length line on its
/// first corner in place of each one that was drawn already or that has no
/// length — which is every edge of the degenerate triangles padding a slot.
///
/// On its first corner, a real vertex of the surface, rather than on vertex
/// zero: wherever a rasteriser puts a point for a zero-length line, it is
/// then on a vertex the polyframe already draws lines through.
fn slotted_lines(triangles: &[u32]) -> Vec<u32> {
    let mut seen = HashSet::with_capacity(triangles.len());
    let mut lines = Vec::with_capacity(triangles.len() * 2);
    for triangle in triangles.chunks_exact(3) {
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]];
        for (from, to) in [(a, b), (b, c), (c, a)] {
            let key = if from < to { (from, to) } else { (to, from) };
            if from != to && seen.insert(key) {
                lines.extend([key.0, key.1]);
            } else {
                lines.extend([a, a]);
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use clayspace_model::LayerKey;

    /// The drawn lines — zero-length ones left out — as ordered pairs, sorted.
    fn drawn(lines: &[u32]) -> Vec<(u32, u32)> {
        let mut pairs: Vec<(u32, u32)> = lines
            .chunks_exact(2)
            .filter(|line| line[0] != line[1])
            .map(|line| (line[0].min(line[1]), line[0].max(line[1])))
            .collect();
        pairs.sort_unstable();
        pairs
    }

    /// Two quads, each in a slot of its own with its own vertices, and a
    /// padding triangle at the end of each slot.
    fn slots() -> Vec<u32> {
        vec![
            0, 1, 2, 0, 2, 3, 0, 0, 0, // slot one: vertices 0..4
            4, 5, 6, 4, 6, 7, 4, 4, 4, // slot two: vertices 4..8
        ]
    }

    #[test]
    fn a_chunked_span_draws_the_same_edges_packed_lines_would() {
        let indices = slots();
        let span = MeshSpan::new(LayerKey(1), 0..indices.len() as u32);
        let packed = lines(&indices, std::slice::from_ref(&span));
        let chunked = lines(&indices, &[span.chunked(true)]);
        assert_eq!(chunked.indices.len(), indices.len() * 2, "two per index");
        assert_eq!(drawn(&chunked.indices), drawn(&packed.indices));
        assert_eq!(drawn(&packed.indices).len(), 10, "five edges a quad");
    }

    #[test]
    fn a_rewritten_slot_patches_exactly_its_own_lines() {
        let mut indices = slots();
        let span = MeshSpan::new(LayerKey(1), 0..indices.len() as u32).chunked(true);
        let mut built = lines(&indices, std::slice::from_ref(&span));

        // The second slot re-cut into a fan of three triangles.
        let run = [4, 5, 6, 4, 6, 7, 4, 7, 5];
        indices[9..18].copy_from_slice(&run);
        let (first, patch) = built.layout.patch(9, &run).expect("inside the span");
        assert_eq!(first, 18);
        let at = first as usize;
        built.indices[at..at + patch.len()].copy_from_slice(&patch);

        let fresh = lines(&indices, &[span]);
        assert_eq!(built.indices, fresh.indices);
    }

    #[test]
    fn a_run_outside_a_chunked_span_or_off_a_triangle_is_not_patched() {
        let indices = slots();
        let plain = MeshSpan::new(LayerKey(1), 0..9);
        let chunked = MeshSpan::new(LayerKey(2), 9..18).chunked(true);
        let built = lines(&indices, &[plain, chunked]);
        assert!(built.layout.patch(0, &indices[0..9]).is_none(), "packed");
        assert!(
            built.layout.patch(10, &indices[10..13]).is_none(),
            "misaligned"
        );
        assert!(built
            .layout
            .patch(9, &[4, 5, 6, 4, 6, 7, 4, 4, 4, 4, 4, 4])
            .is_none());
        assert!(built.layout.patch(9, &indices[9..18]).is_some());
    }
}
