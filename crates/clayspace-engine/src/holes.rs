//! Through-holes in a grid: openings that let the outside into a hollow.
//!
//! The engine's `repair_close_holes` fills by a local rule — an empty cell
//! with four of its six faces occupied is a perforation — which seals a
//! single-cell pinhole and nothing wider. A hole two cells across has two
//! occupied faces per cell and never qualifies, so a perforated shell kept
//! every opening a sculptor could actually see, and the repair said nothing
//! about it.
//!
//! What makes an opening a *hole* rather than a dent is not local, so this
//! decides it the way `repair_fill_voids` decides enclosure — by flooding:
//!
//! 1. Grow the solid by `reach` cells. Every opening up to `2 * reach` across
//!    is bridged.
//! 2. Flood the outside of the grown solid. Whatever deep empty space it no
//!    longer reaches is a hollow the bridges sealed off.
//! 3. Keep only hollows the outside reached *before* growing — a hollow that
//!    was already sealed is a void, and filling it is `fill_voids`' job.
//! 4. Shrink both sides back by `reach`: what is left between the outside and
//!    the hollow, touching both, is the plug. One plug is one hole.
//!
//! A groove, a dent, the gap between two fingers or the hole of a ring all
//! fail step 2 or step 4 — nothing is sealed off behind them — so this adds
//! material only where it closes an opening into a hollow.

use claycore::VoxelField;

/// A dense copy of a grid's cells over its occupied bounds, padded.
///
/// Read once and worked on in memory: the flood fills visit every cell of the
/// window several times, and each of those as a call across the engine
/// boundary would cost more than the analysis.
pub(crate) struct Cells {
    lo: [i32; 3],
    dims: [usize; 3],
    /// The palette index at each cell, `0` where it is empty.
    index: Vec<i32>,
}

/// Cells of empty margin around what is occupied, past the reach.
///
/// The outside flood starts from the window's border, so the border has to be
/// empty even after the solid is grown.
const MARGIN: i32 = 2;

impl Cells {
    /// A window over `dims` cells from `lo`, filled from `at`.
    pub(crate) fn new(lo: [i32; 3], dims: [usize; 3], mut at: impl FnMut([i32; 3]) -> i32) -> Self {
        let mut index = Vec::with_capacity(dims[0] * dims[1] * dims[2]);
        for z in 0..dims[2] {
            for y in 0..dims[1] {
                for x in 0..dims[0] {
                    index.push(at([lo[0] + x as i32, lo[1] + y as i32, lo[2] + z as i32]));
                }
            }
        }
        Self { lo, dims, index }
    }

    /// The grid's occupied bounds, padded enough for `reach`. `None` for an
    /// empty grid, which has nothing to perforate.
    pub(crate) fn read(grid: &VoxelField, reach: usize) -> claycore::Result<Option<Self>> {
        let Some((min, max)) = grid.bounds()? else {
            return Ok(None);
        };
        let pad = reach as i32 + MARGIN;
        let lo = min.map(|v| v - pad);
        let dims = std::array::from_fn(|axis| (max[axis] - min[axis] + 1 + 2 * pad) as usize);
        Self::read_window(grid, lo, dims).map(Some)
    }

    /// The same window, read again — after the grid has been changed.
    pub(crate) fn reread(&self, grid: &VoxelField) -> claycore::Result<Self> {
        Self::read_window(grid, self.lo, self.dims)
    }

    fn read_window(grid: &VoxelField, lo: [i32; 3], dims: [usize; 3]) -> claycore::Result<Self> {
        let mut failure = None;
        let cells = Self::new(lo, dims, |cell| match grid.get(cell) {
            Ok(index) => index.unwrap_or(0),
            Err(e) => {
                failure.get_or_insert(e);
                0
            }
        });
        match failure {
            Some(e) => Err(e),
            None => Ok(cells),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.index.len()
    }

    /// The grid coordinate of a window index.
    pub(crate) fn cell(&self, i: usize) -> [i32; 3] {
        let [nx, ny, _] = self.dims;
        [
            self.lo[0] + (i % nx) as i32,
            self.lo[1] + ((i / nx) % ny) as i32,
            self.lo[2] + (i / (nx * ny)) as i32,
        ]
    }

    pub(crate) fn index_at(&self, i: usize) -> i32 {
        self.index[i]
    }

    fn solid(&self) -> Vec<bool> {
        self.index.iter().map(|&v| v != 0).collect()
    }

    /// Up to six face neighbours of `i` that lie inside the window.
    fn faces(&self, i: usize) -> impl Iterator<Item = usize> {
        let [nx, ny, nz] = self.dims;
        let (x, y, z) = (i % nx, (i / nx) % ny, i / (nx * ny));
        let plane = nx * ny;
        [
            (x > 0).then(|| i - 1),
            (x + 1 < nx).then(|| i + 1),
            (y > 0).then(|| i - nx),
            (y + 1 < ny).then(|| i + nx),
            (z > 0).then(|| i - plane),
            (z + 1 < nz).then(|| i + plane),
        ]
        .into_iter()
        .flatten()
    }

    fn on_border(&self, i: usize) -> bool {
        let [nx, ny, nz] = self.dims;
        let (x, y, z) = (i % nx, (i / nx) % ny, i / (nx * ny));
        x == 0 || y == 0 || z == 0 || x + 1 == nx || y + 1 == ny || z + 1 == nz
    }

    /// Every cell within `r` of a set one, by the box (Chebyshev) distance.
    ///
    /// Separable: a box is three one-dimensional windows, and each is a
    /// running count, so the cost does not grow with `r`.
    fn grow(&self, set: &[bool], r: usize) -> Vec<bool> {
        let mut out = set.to_vec();
        let strides = [1, self.dims[0], self.dims[0] * self.dims[1]];
        for (n, stride) in self.dims.into_iter().zip(strides) {
            let mut next = out.clone();
            let mut prefix = vec![0u32; n + 1];
            for start in (0..self.len()).filter(|&i| (i / stride) % n == 0) {
                // Prefix counts along this line.
                for k in 0..n {
                    prefix[k + 1] = prefix[k] + u32::from(out[start + k * stride]);
                }
                for k in 0..n {
                    let (a, b) = (k.saturating_sub(r), (k + r + 1).min(n));
                    next[start + k * stride] = prefix[b] > prefix[a];
                }
            }
            out = next;
        }
        out
    }

    /// The `open` cells the window's border reaches through `open` cells.
    fn flood_from_border(&self, open: &[bool]) -> Vec<bool> {
        let mut reached = vec![false; self.len()];
        let mut queue: Vec<usize> = (0..self.len())
            .filter(|&i| open[i] && self.on_border(i))
            .collect();
        for &i in &queue {
            reached[i] = true;
        }
        while let Some(i) = queue.pop() {
            for n in self.faces(i) {
                if open[n] && !reached[n] {
                    reached[n] = true;
                    queue.push(n);
                }
            }
        }
        reached
    }

    /// The face-connected pieces of `set`.
    fn pieces(&self, set: &[bool]) -> Vec<Vec<usize>> {
        let mut seen = vec![false; self.len()];
        let mut pieces = Vec::new();
        for start in 0..self.len() {
            if !set[start] || seen[start] {
                continue;
            }
            seen[start] = true;
            let (mut piece, mut queue) = (Vec::new(), vec![start]);
            while let Some(i) = queue.pop() {
                piece.push(i);
                for n in self.faces(i) {
                    if set[n] && !seen[n] {
                        seen[n] = true;
                        queue.push(n);
                    }
                }
            }
            pieces.push(piece);
        }
        pieces
    }

    /// The plugs that would close every opening up to `2 * reach` cells across
    /// that leads into a hollow, one per hole.
    pub(crate) fn through_holes(&self, reach: usize) -> Vec<Vec<usize>> {
        let solid = self.solid();
        let empty: Vec<bool> = solid.iter().map(|&s| !s).collect();
        let grown = self.grow(&solid, reach);
        let outside_grown = self.flood_from_border(&grown.iter().map(|&g| !g).collect::<Vec<_>>());
        let outside = self.flood_from_border(&empty);
        // Deep empty space the grown solid seals off, which the outside did
        // reach before it grew: a hollow entered through an opening.
        let hollow: Vec<bool> = (0..self.len())
            .map(|i| !grown[i] && !outside_grown[i] && outside[i])
            .collect();
        if !hollow.contains(&true) {
            return Vec::new();
        }
        // Both sides shrunk back to the solid's own surface.
        let inside = self.grow(&hollow, reach);
        let beyond = self.grow(&outside_grown, reach);
        let between: Vec<bool> = (0..self.len())
            .map(|i| empty[i] && outside[i] && !inside[i] && !beyond[i])
            .collect();
        let touches = |piece: &[usize], side: &[bool]| {
            piece
                .iter()
                .any(|&i| self.faces(i).any(|n| side[n] && empty[n]))
        };
        self.pieces(&between)
            .into_iter()
            .filter(|piece| touches(piece, &inside) && touches(piece, &beyond))
            .collect()
    }

    /// The colour a plug cell takes: its occupied neighbours' commonest.
    ///
    /// The same rule the engine's repairs use, so a closed hole does not show
    /// as a patch of some arbitrary palette entry in an otherwise uniform
    /// shell.
    fn enclosing_colour(&self, i: usize) -> i32 {
        let [nx, ny, nz] = self.dims;
        let (x, y, z) = (i % nx, (i / nx) % ny, i / (nx * ny));
        let mut counts = std::collections::BTreeMap::<i32, usize>::new();
        for dz in -1i64..=1 {
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (px, py, pz) = (x as i64 + dx, y as i64 + dy, z as i64 + dz);
                    let inside = (0..nx as i64).contains(&px)
                        && (0..ny as i64).contains(&py)
                        && (0..nz as i64).contains(&pz);
                    if !inside {
                        continue;
                    }
                    let v = self.index[(px + py * nx as i64 + pz * (nx * ny) as i64) as usize];
                    if v != 0 {
                        *counts.entry(v).or_default() += 1;
                    }
                }
            }
        }
        counts
            .into_iter()
            .max_by_key(|&(v, n)| (n, std::cmp::Reverse(v)))
            .map_or(0, |(v, _)| v)
    }

    /// Closes every hole up to `2 * reach` across, narrowest first, and says
    /// how many it found and how many are left.
    ///
    /// Narrowest first because a small hollow behind a small hole is only
    /// sealed off at a small reach: at a large one the grown solid fills the
    /// hollow too and there is nothing deep left to find.
    pub(crate) fn close_through_holes(&mut self, reach: usize) -> Closing {
        let mut closing = Closing::default();
        for r in 1..=reach {
            for plug in self.through_holes(r) {
                closing.found += 1;
                let colours: Vec<i32> = plug.iter().map(|&i| self.enclosing_colour(i)).collect();
                // A plug cell with no occupied neighbour takes the plug's own
                // colour, which some other cell of it found on the wall.
                let fallback = colours.iter().copied().find(|&c| c != 0).unwrap_or(1);
                for (&i, colour) in plug.iter().zip(colours) {
                    let colour = if colour == 0 { fallback } else { colour };
                    self.index[i] = colour;
                    closing.cells.push(i);
                }
            }
        }
        closing.remaining = (1..=reach).map(|r| self.through_holes(r).len()).sum();
        closing
    }
}

/// What closing the through-holes did.
#[derive(Debug, Default)]
pub(crate) struct Closing {
    /// Holes found, each closed by one plug.
    pub(crate) found: usize,
    /// Holes still found once every plug is in.
    pub(crate) remaining: usize,
    /// The window indices the plugs filled.
    pub(crate) cells: Vec<usize>,
}

/// The face-connected pieces of the cells `after` holds and `before` did not.
///
/// How the engine's own pinhole pass is counted in holes rather than cells:
/// one pinhole is one cell, and a row of them sealed together is one opening.
pub(crate) fn added_pieces(before: &Cells, after: &Cells) -> usize {
    debug_assert_eq!(before.dims, after.dims);
    let added: Vec<bool> = (0..after.len())
        .map(|i| after.index[i] != 0 && before.index[i] == 0)
        .collect();
    after.pieces(&added).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: i32 = 16;

    /// A window over `[0, N)³` with a margin, holding whatever `solid` says.
    fn window(solid: impl Fn([i32; 3]) -> bool) -> Cells {
        let pad = 6;
        let size = (N + 2 * pad) as usize;
        Cells::new([-pad; 3], [size; 3], |c| i32::from(solid(c)))
    }

    /// A hollow cube: a one-cell wall around a 12³ hollow.
    fn shell(c: [i32; 3]) -> bool {
        let inside = |v: i32| (0..N).contains(&v);
        let wall = |v: i32| v == 0 || v == N - 1;
        c.iter().all(|&v| inside(v)) && c.iter().any(|&v| wall(v))
    }

    /// The shell with a `w`×`w` hole through its `z = 0` wall.
    fn pierced(w: i32) -> impl Fn([i32; 3]) -> bool {
        move |c| {
            let lo = (N - w) / 2;
            let hole = c[2] == 0 && (lo..lo + w).contains(&c[0]) && (lo..lo + w).contains(&c[1]);
            shell(c) && !hole
        }
    }

    fn occupied(cells: &Cells) -> usize {
        cells.index.iter().filter(|&&v| v != 0).count()
    }

    #[test]
    fn a_hole_into_a_hollow_is_one_hole_and_is_closed_exactly() {
        for w in [2, 3, 4] {
            let mut cells = window(pierced(w));
            let before = occupied(&cells);
            let closing = cells.close_through_holes(2);
            assert_eq!(closing.found, 1, "a {w}×{w} hole");
            assert_eq!(closing.remaining, 0, "a {w}×{w} hole");
            assert_eq!(
                closing.cells.len(),
                (w * w) as usize,
                "the plug for a {w}×{w} hole is the hole and nothing else"
            );
            assert_eq!(occupied(&cells), before + (w * w) as usize);
            // What was closed is the shell that was pierced.
            let whole = window(shell);
            assert_eq!(cells.index, whole.index, "a {w}×{w} hole");
        }
    }

    #[test]
    fn a_hole_past_the_reach_is_left_open() {
        let mut cells = window(pierced(8));
        let closing = cells.close_through_holes(2);
        assert_eq!(closing.found, 0);
        assert!(closing.cells.is_empty());
    }

    #[test]
    fn a_sealed_hollow_is_a_void_and_not_a_hole() {
        let mut cells = window(shell);
        let closing = cells.close_through_holes(3);
        assert_eq!(closing.found, 0, "fill_voids is what fills a sealed hollow");
        assert!(closing.cells.is_empty());
    }

    #[test]
    fn a_ring_keeps_its_hole() {
        // A square ring, two cells thick, around a 3×3 hole: the hole is an
        // opening, but it leads nowhere, so there is nothing to close.
        let ring = |c: [i32; 3]| {
            let inside = |v: i32| (4..12).contains(&v);
            let hole = (6..9).contains(&c[0]) && (6..9).contains(&c[1]);
            inside(c[0]) && inside(c[1]) && (0..2).contains(&c[2]) && !hole
        };
        let mut cells = window(ring);
        let closing = cells.close_through_holes(3);
        assert_eq!(closing.found, 0);
        assert!(closing.cells.is_empty());
    }

    #[test]
    fn a_groove_is_left_alone() {
        // A solid block with a two-cell groove cut across its top.
        let grooved = |c: [i32; 3]| {
            let inside = c.iter().all(|&v| (2..14).contains(&v));
            let groove = c[2] >= 10 && (7..9).contains(&c[0]);
            inside && !groove
        };
        let mut cells = window(grooved);
        let closing = cells.close_through_holes(3);
        assert_eq!(closing.found, 0);
        assert!(closing.cells.is_empty());
    }

    #[test]
    fn two_holes_are_counted_as_two() {
        let twice = |c: [i32; 3]| {
            let first = c[2] == 0 && (3..5).contains(&c[0]) && (3..5).contains(&c[1]);
            let second = c[0] == N - 1 && (9..12).contains(&c[1]) && (9..12).contains(&c[2]);
            shell(c) && !first && !second
        };
        let mut cells = window(twice);
        let closing = cells.close_through_holes(2);
        assert_eq!(closing.found, 2);
        assert_eq!(closing.remaining, 0);
        assert_eq!(closing.cells.len(), 4 + 9);
    }

    #[test]
    fn a_plug_takes_the_colour_of_its_wall() {
        let mut cells = Cells::new([-6; 3], [(N + 12) as usize; 3], |c| {
            if pierced(3)(c) {
                7
            } else {
                0
            }
        });
        let closing = cells.close_through_holes(2);
        assert!(closing.cells.iter().all(|&i| cells.index_at(i) == 7));
    }

    #[test]
    fn pinholes_are_counted_by_piece() {
        let before = window(pierced(2));
        let after = window(shell);
        assert_eq!(added_pieces(&before, &after), 1);
        assert_eq!(added_pieces(&after, &after), 0);
    }
}
