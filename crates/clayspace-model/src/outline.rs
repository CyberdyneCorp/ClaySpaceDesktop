//! An outline drawn over the form, and the region of space it encloses.
//!
//! ZBrush's mask lasso and mask rect: drag a shape over the model and
//! everything the shape covers is frozen — through the form, front and back,
//! because the gesture is made on the *screen* and not on the surface. That is
//! the whole idea, and everything here exists to turn a screen outline into
//! cells of a world-addressed mask.
//!
//! The two gestures differ in exactly one place, [`OutlineDraft`]: a lasso
//! accumulates every point the pointer passed through, and a rectangle keeps
//! the corner it started at and replaces the other. Past that they are the
//! same list of points, and nothing downstream — the containment test, the
//! traversal, the engine — can tell them apart or needs to.
//!
//! # Why a prism and not a cone
//!
//! A shape drawn under a perspective camera sweeps a converging wedge, and a
//! region defined by one depends on where the camera was standing: the same
//! outline over the same form freezes differently from two paces back. The
//! engine's own cut tool refuses that for the same reason — "a trim is a
//! straight cut, as it is in ZBrush and 3DCoat" — so the outline is carried
//! onto a **frame**, an origin and an orthonormal basis, and swept straight
//! along the view direction. [`OutlineFrame`] is that frame, and it is the same
//! description `clay_cut_desc` takes, so the trim tool can be given the same
//! gesture later without inventing a second vocabulary for it.
//!
//! # Why a path and not a set of cells
//!
//! A mask cell can be written one call at a time, and a document-owned mask
//! snapshots itself on every one of those calls so the edit can be undone —
//! measured at about four milliseconds each on a mask covering a million
//! cells. Five thousand of them is twenty-one seconds, which is not a feature.
//!
//! The one entry point that writes many cells for one snapshot is the stroke:
//! a polyline, walked by arc length, stamping as it goes. So the region is
//! delivered as a **path that visits it** — [`coverage_path`] — and the whole
//! gesture is one stamp run, one snapshot, one undo entry. The path must never
//! leave the region, because everything it passes over is frozen too, which is
//! what the traversal here is careful about.
//!
//! # What the pitch buys, and what it does not
//!
//! The lattice the path is walked on is aligned to the **camera**, because
//! that is where the outline was drawn, and a brush footprint is aligned to the
//! **world**. So the footprint has to reach half the pitch's *diagonal* to
//! cover the lattice from any angle rather than half its side: sized to half a
//! side, the two tile only when the camera happens to face down an axis, and
//! from anywhere else the region comes out speckled with cells no stamp
//! reached.
//!
//! That is why [`lattice_pitch`] is a constant rather than a dial. Opening the
//! pitch by two divides the stamps by eight and multiplies the cells each one
//! writes by eight, so what a gesture costs is the region's volume in mask cells
//! and nothing else — about 2.7 writes per cell of it, at about 140 nanoseconds
//! a write. The pitch buys only how coarsely the region's edge is quantised, so
//! it is fixed at the finest worth walking and a region too large to write at
//! all is refused: see [`CELL_CEILING`].

/// What a drawn outline does to what it encloses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutlineMode {
    /// Freeze it.
    #[default]
    Freeze,
    /// Release it, which is what the same gesture does with the invert
    /// modifier held.
    Thaw,
}

impl OutlineMode {
    pub const ALL: [OutlineMode; 2] = [Self::Freeze, Self::Thaw];

    /// The mask value the enclosed cells are painted toward.
    pub fn target(self) -> f32 {
        match self {
            Self::Freeze => 1.0,
            Self::Thaw => 0.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Freeze => "Congelar",
            Self::Thaw => "Liberar",
        }
    }
}

/// Which gesture the mask brush makes.
///
/// Three ways of saying the same thing to the same mask, so it is one setting
/// rather than three tools: ZBrush keeps them together in the stroke palette
/// beside the freehand drag for exactly that reason, and a second tool would
/// need a second answer to every availability question the first one already
/// answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaskGesture {
    /// A drag across the surface, painting where it touches.
    #[default]
    Brush,
    /// A shape traced freehand over the form, freezing what it encloses.
    Lasso,
    /// A box dragged from corner to corner, square to the screen.
    ///
    /// Not a special case of the lasso but a different gesture with the same
    /// outcome: a hand cannot draw a straight edge, and "everything above this
    /// line" is what a mask is most often wanted for.
    Rectangle,
}

impl MaskGesture {
    pub const ALL: [MaskGesture; 3] = [Self::Brush, Self::Lasso, Self::Rectangle];

    /// Whether the gesture draws a shape on the view frame rather than
    /// painting where the pointer touches the surface.
    pub fn draws_an_outline(self) -> bool {
        !matches!(self, Self::Brush)
    }

    /// Whether the pointer's whole path is the outline, or only its two ends.
    ///
    /// The one place the two drawn gestures differ: a lasso accumulates every
    /// point the pointer passed through, and a rectangle keeps the corner it
    /// started at and replaces the other with wherever the pointer is now.
    pub fn traces_the_path(self) -> bool {
        matches!(self, Self::Lasso)
    }

    /// Whether the outline closes itself across a gap the sculptor can see.
    ///
    /// A lasso does — the last point joins the first, and the interface shows
    /// where — and a rectangle does not: four corners are four edges, and
    /// drawing one of them faint would suggest it is less certain than the
    /// other three.
    pub fn closes_a_gap(self) -> bool {
        matches!(self, Self::Lasso)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Brush => "Pincel",
            Self::Lasso => "Laço",
            Self::Rectangle => "Retângulo",
        }
    }
}

/// The plane an outline was drawn on, in world terms.
///
/// `right`, `up` and `forward` are unit and mutually perpendicular; `origin`
/// is where the outline's `(0, 0)` sits, and `scale` says how many world units
/// one unit of normalised device coordinate is worth on each axis. The
/// interface knows all four because it needed them to draw the overlay; the
/// domain takes them rather than a camera, which it has no business knowing
/// about.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutlineFrame {
    pub origin: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    /// World units per unit of normalised device coordinate, x and y.
    pub scale: [f32; 2],
}

impl OutlineFrame {
    /// The world point at `(x, y)` on the frame, `t` along the sweep.
    pub fn at(&self, on_frame: [f32; 2], t: f32) -> [f32; 3] {
        std::array::from_fn(|axis| {
            self.origin[axis]
                + self.right[axis] * on_frame[0]
                + self.up[axis] * on_frame[1]
                + self.forward[axis] * t
        })
    }

    /// Where a world point sits on the frame, and how far along the sweep.
    pub fn of(&self, point: [f32; 3]) -> ([f32; 2], f32) {
        let away: [f32; 3] = std::array::from_fn(|axis| point[axis] - self.origin[axis]);
        (
            [dot(away, self.right), dot(away, self.up)],
            dot(away, self.forward),
        )
    }

    /// A point of the drawn outline, in the frame's own world units.
    ///
    /// Normalised device coordinates are what the viewport reports and what
    /// the overlay is drawn from; the frame is measured in world units,
    /// because the engine has no viewport and does not want one.
    pub fn from_ndc(&self, ndc: [f32; 2]) -> [f32; 2] {
        [ndc[0] * self.scale[0], ndc[1] * self.scale[1]]
    }
}

/// An outline drawn over the form, and what it should do to the mask.
///
/// The outline is closed implicitly: the last point joins the first, exactly
/// as `CLAY_CUT_POLYGON` closes one.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskOutline {
    /// The outline in the frame's world units, in the order it was drawn.
    pub outline: Vec<[f32; 2]>,
    pub frame: OutlineFrame,
    pub mode: OutlineMode,
}

/// The fewest points an outline can enclose anything with.
pub const FEWEST_POINTS: usize = 3;

impl MaskOutline {
    /// Whether the outline encloses anything at all.
    ///
    /// A click that did not become a drag, or a drag that went back and forth
    /// along one line, encloses nothing — and freezing nothing while saying it
    /// froze something is worse than saying the gesture was too small.
    pub fn encloses_anything(&self) -> bool {
        self.outline.len() >= FEWEST_POINTS && self.area().abs() > f32::EPSILON
    }

    /// Twice the signed area, by the shoelace sum. Sign is the winding.
    fn area(&self) -> f32 {
        let mut sum = 0.0;
        for (a, b) in self.edges() {
            sum += a[0] * b[1] - b[0] * a[1];
        }
        sum
    }

    /// Every edge of the closed outline, the last one joining back to the
    /// first.
    fn edges(&self) -> impl Iterator<Item = ([f32; 2], [f32; 2])> + '_ {
        let outline = &self.outline;
        (0..outline.len()).map(move |i| (outline[i], outline[(i + 1) % outline.len()]))
    }

    /// Whether a point on the frame is inside the outline.
    ///
    /// The even-odd crossing rule, which is what a hand-drawn lasso wants: an
    /// outline that crosses itself leaves the overlap unfrozen rather than
    /// doubly frozen, and a sculptor who loops back over their own line sees
    /// the hole they drew.
    pub fn encloses(&self, at: [f32; 2]) -> bool {
        let mut inside = false;
        for (a, b) in self.edges() {
            // Half-open in y, so a vertex exactly on the ray is counted once.
            if (a[1] > at[1]) != (b[1] > at[1]) {
                let span = b[1] - a[1];
                let crossing = a[0] + (at[1] - a[1]) / span * (b[0] - a[0]);
                if at[0] < crossing {
                    inside = !inside;
                }
            }
        }
        inside
    }

    /// The outline's own bounding rectangle on the frame.
    pub fn bounds(&self) -> Option<([f32; 2], [f32; 2])> {
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for point in &self.outline {
            for axis in 0..2 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        }
        (min[0] <= max[0]).then_some((min, max))
    }
}

/// How far apart two consecutive outline points have to be to both be kept.
///
/// In normalised device coordinates, so it is about a thousandth of the
/// viewport's width. A pointer reports a position every frame whether it moved
/// or not, and an outline of four thousand coincident points is four thousand
/// edges every containment test walks.
pub const OUTLINE_SPACING: f32 = 0.004;

/// The outline as the pointer draws it, before it is carried onto a frame.
///
/// Kept in normalised device coordinates because that is what the overlay is
/// drawn from and what the viewport reports; the frame is not known until the
/// gesture ends.
///
/// `track` is what the pointer did, and it is not the outline: for a rectangle
/// it is two corners, and [`OutlineDraft::corners`] is what turns them into the
/// four the shape actually has. Everything downstream — the overlay, the
/// containment test, the region — reads the corners, so a shape drawn one way
/// and freezing another is not expressible.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OutlineDraft {
    /// The pointer's own record: every point for a lasso, two corners for a
    /// rectangle.
    pub track: Vec<[f32; 2]>,
    pub mode: OutlineMode,
    /// Which gesture drew it, which decides how `track` becomes an outline.
    pub gesture: MaskGesture,
}

impl OutlineDraft {
    pub fn new(at: [f32; 2], mode: OutlineMode, gesture: MaskGesture) -> Self {
        Self {
            track: vec![at],
            mode,
            gesture,
        }
    }

    /// Carries the outline to where the pointer is now.
    ///
    /// A lasso grows by a point, unless the pointer has not moved far enough
    /// to be worth one; a rectangle keeps the corner it started at and moves
    /// the other, because a box is its two corners however far the hand
    /// wandered between them.
    pub fn extend(&mut self, at: [f32; 2]) {
        if !self.gesture.traces_the_path() {
            self.track.truncate(1);
            self.track.push(at);
            return;
        }
        let far_enough = self.track.last().is_none_or(|last| {
            let (dx, dy) = (at[0] - last[0], at[1] - last[1]);
            dx * dx + dy * dy >= OUTLINE_SPACING * OUTLINE_SPACING
        });
        if far_enough {
            self.track.push(at);
        }
    }

    /// The outline itself, in normalised device coordinates.
    ///
    /// The lasso's is what the pointer traced. The rectangle's is the four
    /// corners of the box between the point pressed and the point now, square
    /// to the **screen** rather than to the world — it is drawn on the screen,
    /// and a box that came out lozenge-shaped because the camera was turned
    /// would be a box nobody could aim.
    pub fn corners(&self) -> Vec<[f32; 2]> {
        if self.gesture.traces_the_path() {
            return self.track.clone();
        }
        let (Some(from), Some(to)) = (self.track.first(), self.track.last()) else {
            return Vec::new();
        };
        if from == to {
            // A press that has not moved. One point is not a box, and saying so
            // here keeps `encloses_anything` the only place that decides it.
            return vec![*from];
        }
        vec![
            [from[0], from[1]],
            [to[0], from[1]],
            [to[0], to[1]],
            [from[0], to[1]],
        ]
    }

    /// The gesture carried onto the frame it was drawn over.
    pub fn onto(&self, frame: OutlineFrame) -> MaskOutline {
        MaskOutline {
            outline: self
                .corners()
                .iter()
                .map(|ndc| frame.from_ndc(*ndc))
                .collect(),
            frame,
            mode: self.mode,
        }
    }
}

/// One column of the region: where it is on the frame, and the sweep it
/// covers.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Column {
    on_frame: [f32; 2],
    /// The sweep interval this column covers, along the frame's forward axis.
    span: (f32, f32),
}

/// The order to visit every column of the region in, one run per connected
/// piece of it.
///
/// The traversal is a depth-first walk of the lattice, four-connected: each
/// column is entered at whichever end of the sweep the walk is currently at,
/// swept to the other end, and left from there. Backtracking moves to the
/// column it came from at the end it is already at, which is one short segment
/// between neighbouring columns rather than a jump across the region.
///
/// That is the property the whole thing rests on. A stamp lands everywhere the
/// path goes, so a connector that cut across the outline would freeze a stripe
/// the sculptor did not draw — which is what a plain back-and-forth over the
/// rows does the first time a lasso is drawn with a concave side.
///
/// **One run per piece**, and that is the same property again: an outline drawn
/// as a figure of eight encloses two regions with nothing between them, and a
/// path that stepped from one to the other would freeze the waist. A run for
/// each is what freezes both without freezing what joins them.
fn walk(columns: &[Vec<Option<Column>>]) -> Vec<Vec<[i32; 2]>> {
    let rows = columns.len();
    let width = columns.first().map_or(0, Vec::len);
    let mut visited = vec![vec![false; width]; rows];
    let mut runs = Vec::new();
    for start in every_column(columns) {
        if visited[start[1] as usize][start[0] as usize] {
            continue;
        }
        runs.push(walk_from(columns, &mut visited, start));
    }
    runs
}

/// One connected piece, from a column the walk has not reached yet.
fn walk_from(
    columns: &[Vec<Option<Column>>],
    visited: &mut [Vec<bool>],
    start: [i32; 2],
) -> Vec<[i32; 2]> {
    let mut run = vec![start];
    let mut stack = vec![start];
    visited[start[1] as usize][start[0] as usize] = true;

    while let Some(&at) = stack.last() {
        match unvisited_neighbour(columns, visited, at) {
            Some(next) => {
                visited[next[1] as usize][next[0] as usize] = true;
                stack.push(next);
                run.push(next);
            }
            None => {
                stack.pop();
                if let Some(&back) = stack.last() {
                    run.push(back);
                }
            }
        }
    }
    run
}

/// Every column of the grid, in row-major order.
fn every_column(columns: &[Vec<Option<Column>>]) -> Vec<[i32; 2]> {
    columns
        .iter()
        .enumerate()
        .flat_map(|(y, row)| {
            row.iter()
                .enumerate()
                .filter(|(_, column)| column.is_some())
                .map(move |(x, _)| [x as i32, y as i32])
        })
        .collect()
}

/// A neighbouring column the walk has not been to yet.
fn unvisited_neighbour(
    columns: &[Vec<Option<Column>>],
    visited: &[Vec<bool>],
    at: [i32; 2],
) -> Option<[i32; 2]> {
    const STEPS: [[i32; 2]; 4] = [[1, 0], [0, 1], [-1, 0], [0, -1]];
    STEPS.iter().find_map(|step| {
        let next = [at[0] + step[0], at[1] + step[1]];
        let (x, y) = (
            usize::try_from(next[0]).ok()?,
            usize::try_from(next[1]).ok()?,
        );
        columns.get(y)?.get(x)?.as_ref()?;
        (!visited[y][x]).then_some(next)
    })
}

/// How many cell writes one gesture may cost before it is refused.
///
/// Measured: about 140 nanoseconds a write, so this is a little under two
/// seconds on the largest region that is allowed through at all. An ordinary
/// outline over an ordinary subtool costs a small fraction of it and never
/// meets this; what it stops is one thrown around a subtool tens of units across,
/// where the region runs to hundreds of millions of cells and the gesture would
/// appear to hang.
pub const CELL_CEILING: f64 = 12_000_000.0;

/// The lattice pitch an outline is walked at, in world units.
///
/// Two mask cells, always, and not a dial. The pitch decides how coarsely the
/// region's edge is quantised and — this is the part that is not obvious — it
/// does **not** decide what the gesture costs. The footprint has to cover the
/// lattice, so its radius scales with the pitch; opening the pitch by two
/// divides the stamps by eight and multiplies the cells each one writes by
/// eight. What a gesture costs is the region's volume in mask cells and nothing
/// else, so there is nothing to trade the edge against and no reason to make
/// the pitch anything but the finest that is worth walking.
pub fn lattice_pitch(cell: f32) -> f32 {
    2.0 * cell
}

/// How much of the footprint's writing lands on cells already written.
///
/// The lattice is aligned to the **camera**, because that is where the outline
/// was drawn, and a brush footprint is aligned to the **world**. A ball has to
/// reach half the pitch's diagonal to cover a lattice cell from any angle
/// rather than half its side, so the balls overlap and every cell of the region
/// is written about two and three quarter times. That is the standing cost of
/// drawing the region in a frame the engine does not share; a footprint that
/// tiled would write each cell once.
pub const COVERING: f64 = 2.72;

/// How many cell writes freezing this region would cost.
///
/// The estimate the ceiling is read against, and deliberately an over-estimate
/// of the region: the outline's own rectangle clipped to the subtool's extent,
/// swept through it, rather than the outline's area. Working out the exact
/// figure means walking the lattice, which is the work being decided about.
pub fn cells_to_write(outline: &MaskOutline, bounds: ([f32; 3], [f32; 3]), cell: f32) -> f64 {
    let Some((min, max)) = outline.bounds() else {
        return 0.0;
    };
    if cell <= 0.0 || !cell.is_finite() {
        return f64::INFINITY;
    }
    let (frame_min, frame_max) = projected_bounds(&outline.frame, bounds);
    let width = (max[0].min(frame_max[0]) - min[0].max(frame_min[0])).max(0.0) as f64;
    let height = (max[1].min(frame_max[1]) - min[1].max(frame_min[1])).max(0.0) as f64;
    let depth: f64 = (0..3)
        .map(|axis| (bounds.1[axis] - bounds.0[axis]) * outline.frame.forward[axis])
        .map(|span| span.abs() as f64)
        .sum();
    let cell = cell as f64;
    COVERING * (width / cell) * (height / cell) * (depth / cell)
}

/// The world paths that cover everything the outline encloses within `bounds`.
///
/// `spacing` is the lattice pitch in world units — see [`lattice_pitch`], and
/// [`crate::outline`] for why the footprint that follows a path has to be wider
/// than half of it. Each returned path is a polyline for the engine's stroke
/// walker: the caller stamps along it.
///
/// **One path per connected piece.** An outline can enclose two regions with
/// nothing between them, and a single path across both would freeze the gap.
///
/// `None` where the outline encloses nothing inside the bounds, which is an
/// ordinary outcome — a shape drawn beside the form rather than over it.
pub fn coverage_path(
    outline: &MaskOutline,
    bounds: ([f32; 3], [f32; 3]),
    spacing: f32,
) -> Option<Vec<Vec<[f32; 3]>>> {
    let grid = OutlineGrid::of(outline, bounds, spacing)?;
    let paths: Vec<Vec<[f32; 3]>> = walk(&grid.columns)
        .into_iter()
        .map(|run| grid.sweep_along(outline, &run))
        .filter(|path| !path.is_empty())
        .collect();
    (!paths.is_empty()).then_some(paths)
}

/// The lattice of columns the outline encloses, and where each one sweeps.
struct OutlineGrid {
    columns: Vec<Vec<Option<Column>>>,
}

impl OutlineGrid {
    fn at(&self, cell: [i32; 2]) -> Option<&Column> {
        let (x, y) = (
            usize::try_from(cell[0]).ok()?,
            usize::try_from(cell[1]).ok()?,
        );
        self.columns.get(y)?.get(x)?.as_ref()
    }

    /// The world polyline for one run of the walk.
    ///
    /// Alternating ends: a column is entered where the walk stands and left at
    /// the other end, so consecutive columns are joined by one short segment
    /// instead of a return trip back down the sweep.
    fn sweep_along(&self, outline: &MaskOutline, run: &[[i32; 2]]) -> Vec<[f32; 3]> {
        let mut path = Vec::with_capacity(run.len() * 2);
        let mut at_far = false;
        for step in run {
            let Some(column) = self.at(*step) else {
                continue;
            };
            let (near, far) = column.span;
            let (enter, leave) = if at_far { (far, near) } else { (near, far) };
            path.push(outline.frame.at(column.on_frame, enter));
            path.push(outline.frame.at(column.on_frame, leave));
            at_far = !at_far;
        }
        path
    }

    /// Lays the lattice over the outline's own rectangle, clipped to where the
    /// bounds project onto the frame.
    ///
    /// Both clips matter. Without the outline's rectangle a shape drawn over a
    /// corner of a large form walks the whole form; without the bounds a shape
    /// drawn well off the form walks a region with nothing in it.
    fn of(outline: &MaskOutline, bounds: ([f32; 3], [f32; 3]), spacing: f32) -> Option<Self> {
        if spacing <= 0.0 || !spacing.is_finite() {
            return None;
        }
        let (outline_min, outline_max) = outline.bounds()?;
        let (frame_min, frame_max) = projected_bounds(&outline.frame, bounds);
        let min: [f32; 2] = std::array::from_fn(|a| outline_min[a].max(frame_min[a]));
        let max: [f32; 2] = std::array::from_fn(|a| outline_max[a].min(frame_max[a]));
        if min[0] > max[0] || min[1] > max[1] {
            return None;
        }

        let steps: [usize; 2] =
            std::array::from_fn(|a| (((max[a] - min[a]) / spacing).ceil() as usize).max(1) + 1);
        let mut columns = Vec::with_capacity(steps[1]);
        let mut any = false;
        for y in 0..steps[1] {
            let mut row = Vec::with_capacity(steps[0]);
            for x in 0..steps[0] {
                let on_frame = [min[0] + x as f32 * spacing, min[1] + y as f32 * spacing];
                let column = outline
                    .encloses(on_frame)
                    .then(|| sweep(&outline.frame, on_frame, bounds))
                    .flatten()
                    .map(|span| Column { on_frame, span });
                any |= column.is_some();
                row.push(column);
            }
            columns.push(row);
        }
        any.then_some(Self { columns })
    }
}

/// Where the ray through `on_frame` enters and leaves the box.
///
/// The ordinary slab test. `None` where the ray misses, which is what makes a
/// shape drawn over the empty half of a form cost nothing there.
fn sweep(
    frame: &OutlineFrame,
    on_frame: [f32; 2],
    bounds: ([f32; 3], [f32; 3]),
) -> Option<(f32, f32)> {
    let from = frame.at(on_frame, 0.0);
    let (mut near, mut far) = (f32::NEG_INFINITY, f32::INFINITY);
    for (axis, at) in from.iter().enumerate() {
        let direction = frame.forward[axis];
        if direction.abs() < 1e-9 {
            // Parallel to this pair of faces: either the whole ray is between
            // them or none of it is.
            if *at < bounds.0[axis] || *at > bounds.1[axis] {
                return None;
            }
            continue;
        }
        let a = (bounds.0[axis] - at) / direction;
        let b = (bounds.1[axis] - at) / direction;
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far).then_some((near, far))
}

/// The rectangle the box covers on the frame.
fn projected_bounds(frame: &OutlineFrame, bounds: ([f32; 3], [f32; 3])) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for corner in 0..8 {
        let point: [f32; 3] = std::array::from_fn(|axis| {
            if corner & (1 << axis) == 0 {
                bounds.0[axis]
            } else {
                bounds.1[axis]
            }
        });
        let (on_frame, _) = frame.of(point);
        for axis in 0..2 {
            min[axis] = min[axis].min(on_frame[axis]);
            max[axis] = max[axis].max(on_frame[axis]);
        }
    }
    (min, max)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> OutlineFrame {
        OutlineFrame {
            origin: [0.0, 0.0, 0.0],
            right: [1.0, 0.0, 0.0],
            up: [0.0, 1.0, 0.0],
            forward: [0.0, 0.0, 1.0],
            scale: [1.0, 1.0],
        }
    }

    fn square(half: f32) -> MaskOutline {
        MaskOutline {
            outline: vec![[-half, -half], [half, -half], [half, half], [-half, half]],
            frame: frame(),
            mode: OutlineMode::Freeze,
        }
    }

    #[test]
    fn an_outline_that_encloses_nothing_says_so() {
        // A click, and a drag that went out and came straight back: both are
        // gestures a sculptor makes by accident, and neither freezes anything.
        for outline in [
            vec![],
            vec![[0.0, 0.0]],
            vec![[0.0, 0.0], [1.0, 0.0]],
            vec![[0.0, 0.0], [1.0, 0.0], [0.5, 0.0]],
        ] {
            let lasso = MaskOutline {
                outline,
                frame: frame(),
                mode: OutlineMode::Freeze,
            };
            assert!(
                !lasso.encloses_anything(),
                "{:?} was taken for a region",
                lasso.outline
            );
        }
        assert!(square(1.0).encloses_anything());
    }

    #[test]
    fn containment_follows_the_outline() {
        let lasso = square(1.0);
        assert!(lasso.encloses([0.0, 0.0]));
        assert!(lasso.encloses([-0.9, 0.9]));
        assert!(!lasso.encloses([1.5, 0.0]));
        assert!(!lasso.encloses([0.0, -2.0]));
    }

    /// The even-odd rule, which is the one a hand-drawn lasso wants: a loop
    /// drawn back over itself leaves the overlap alone.
    #[test]
    fn a_hole_drawn_inside_the_outline_is_not_enclosed() {
        // A square with a smaller square traced inside it in the same
        // direction, joined into one outline.
        let lasso = MaskOutline {
            outline: vec![
                [-2.0, -2.0],
                [2.0, -2.0],
                [2.0, 2.0],
                [-2.0, 2.0],
                [-2.0, -2.0],
                // Back in, around a smaller square.
                [-1.0, -1.0],
                [-1.0, 1.0],
                [1.0, 1.0],
                [1.0, -1.0],
                [-1.0, -1.0],
            ],
            frame: frame(),
            mode: OutlineMode::Freeze,
        };
        assert!(lasso.encloses([1.5, 0.0]), "the ring should be enclosed");
        assert!(!lasso.encloses([0.0, 0.0]), "the hole should not be");
    }

    #[test]
    fn the_draft_drops_points_the_pointer_did_not_move_between() {
        let mut draft = OutlineDraft::new([0.0, 0.0], OutlineMode::Freeze, MaskGesture::Lasso);
        for _ in 0..100 {
            draft.extend([0.0, 0.0]);
        }
        assert_eq!(draft.track.len(), 1, "a still pointer grew the outline");
        draft.extend([0.5, 0.0]);
        assert_eq!(draft.track.len(), 2);
    }

    #[test]
    fn a_rectangle_is_its_two_corners_however_far_the_hand_wandered() {
        // A box is where it began and where the pointer is now. Accumulating
        // the path between them would make a rectangle that had been dragged
        // out and back a polygon of everywhere the hand went.
        let mut draft =
            OutlineDraft::new([-0.5, -0.5], OutlineMode::Freeze, MaskGesture::Rectangle);
        for at in [[0.1, 0.2], [-0.9, 0.7], [0.5, 0.5]] {
            draft.extend(at);
        }
        assert_eq!(draft.track.len(), 2, "the path was kept");
        assert_eq!(
            draft.corners(),
            vec![[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]],
            "the corners are not the box between the two points"
        );
    }

    #[test]
    fn a_rectangle_is_square_to_the_screen_from_either_corner() {
        // Dragged up and to the left rather than down and to the right: the
        // same box, and a winding the containment test does not care about.
        let mut draft = OutlineDraft::new([0.5, 0.5], OutlineMode::Freeze, MaskGesture::Rectangle);
        draft.extend([-0.5, -0.5]);
        let outline = MaskOutline {
            outline: draft.corners(),
            frame: frame(),
            mode: OutlineMode::Freeze,
        };
        assert!(outline.encloses_anything());
        assert!(outline.encloses([0.0, 0.0]));
        assert!(!outline.encloses([0.7, 0.0]));
        assert!(!outline.encloses([0.0, -0.7]));
    }

    #[test]
    fn a_rectangle_that_never_moved_encloses_nothing() {
        let draft = OutlineDraft::new([0.2, 0.2], OutlineMode::Freeze, MaskGesture::Rectangle);
        assert_eq!(draft.corners(), vec![[0.2, 0.2]]);
        let outline = MaskOutline {
            outline: draft.corners(),
            frame: frame(),
            mode: OutlineMode::Freeze,
        };
        assert!(!outline.encloses_anything());
    }

    #[test]
    fn a_draft_carried_onto_a_frame_is_measured_in_world_units() {
        let mut draft = OutlineDraft::new([-1.0, -1.0], OutlineMode::Thaw, MaskGesture::Lasso);
        draft.extend([1.0, -1.0]);
        draft.extend([1.0, 1.0]);
        let frame = OutlineFrame {
            scale: [2.0, 1.5],
            ..frame()
        };
        let lasso = draft.onto(frame);
        assert_eq!(lasso.mode, OutlineMode::Thaw);
        assert_eq!(lasso.outline[0], [-2.0, -1.5]);
        assert_eq!(lasso.outline[2], [2.0, 1.5]);
    }

    #[test]
    fn the_path_stays_inside_the_outline() {
        // The property everything else rests on: a stamp lands everywhere the
        // path goes, so a path that cuts across the outline freezes a stripe
        // nobody drew.
        let lasso = MaskOutline {
            // A C, whose opening a back-and-forth traversal would cross.
            outline: vec![
                [-1.0, -1.0],
                [1.0, -1.0],
                [1.0, -0.6],
                [-0.6, -0.6],
                [-0.6, 0.6],
                [1.0, 0.6],
                [1.0, 1.0],
                [-1.0, 1.0],
            ],
            frame: frame(),
            mode: OutlineMode::Freeze,
        };
        let bounds = ([-2.0, -2.0, -2.0], [2.0, 2.0, 2.0]);
        let paths = coverage_path(&lasso, bounds, 0.1).expect("a path");
        assert_eq!(paths.len(), 1, "a C is one connected region");
        let path = &paths[0];
        assert!(path.len() > 2);
        for point in path {
            let (on_frame, _) = lasso.frame.of(*point);
            assert!(
                lasso.encloses(on_frame),
                "the path left the outline at {on_frame:?}"
            );
        }
    }

    #[test]
    fn the_path_visits_every_column_the_outline_encloses() {
        let lasso = square(0.5);
        let bounds = ([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let spacing = 0.1;
        let paths = coverage_path(&lasso, bounds, spacing).expect("a path");

        // Every lattice point inside the outline has to appear on the path, or
        // the region has a hole in it that no amount of footprint will close.
        let visited: Vec<[f32; 2]> = paths
            .iter()
            .flatten()
            .map(|p| lasso.frame.of(*p).0)
            .collect();
        let grid = OutlineGrid::of(&lasso, bounds, spacing).expect("a grid");
        for row in &grid.columns {
            for column in row.iter().flatten() {
                assert!(
                    visited.iter().any(|at| {
                        (at[0] - column.on_frame[0]).abs() < 1e-4
                            && (at[1] - column.on_frame[1]).abs() < 1e-4
                    }),
                    "the path never reached {:?}",
                    column.on_frame
                );
            }
        }
    }

    #[test]
    fn the_path_sweeps_the_whole_depth_of_the_bounds() {
        // Through the form, front and back, which is what makes this ZBrush's
        // lasso rather than a surface brush wearing its name.
        let lasso = square(0.5);
        let bounds = ([-1.0, -1.0, -3.0], [1.0, 1.0, 4.0]);
        let paths = coverage_path(&lasso, bounds, 0.25).expect("a path");
        let depths: Vec<f32> = paths.iter().flatten().map(|p| p[2]).collect();
        let near = depths.iter().copied().fold(f32::INFINITY, f32::min);
        let far = depths.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!((near - -3.0).abs() < 1e-4, "the sweep started at {near}");
        assert!((far - 4.0).abs() < 1e-4, "the sweep ended at {far}");
    }

    #[test]
    fn two_regions_with_nothing_between_them_are_two_runs() {
        // An outline can enclose two pieces with nothing between them — a
        // figure of eight, or a stroke that crossed itself twice. One run
        // across both would step over the gap, and everything the step passed
        // over would freeze: the sculptor drew *around* that, not through it.
        //
        // Built as a lattice rather than as an outline, because what is being
        // asserted is the walk: an outline that produces two pieces at one
        // pitch produces one at another, and the property is about neither.
        let column = |x: usize, y: usize| Column {
            on_frame: [x as f32, y as f32],
            span: (0.0, 1.0),
        };
        let filled = [[true, false, true], [true, false, true]];
        let columns: Vec<Vec<Option<Column>>> = filled
            .iter()
            .enumerate()
            .map(|(y, row)| {
                row.iter()
                    .enumerate()
                    .map(|(x, on)| on.then(|| column(x, y)))
                    .collect()
            })
            .collect();

        let runs = walk(&columns);
        assert_eq!(runs.len(), 2, "the two pieces came back as one run");
        for run in &runs {
            let column = run[0][0];
            assert!(
                run.iter().all(|at| at[0] == column),
                "a run crossed the gap: {run:?}"
            );
        }
    }

    #[test]
    fn a_lasso_drawn_beside_the_form_covers_nothing() {
        let lasso = square(0.5);
        let bounds = ([10.0, 10.0, 10.0], [11.0, 11.0, 11.0]);
        assert!(coverage_path(&lasso, bounds, 0.1).is_none());
    }

    #[test]
    fn a_frame_round_trips_a_point() {
        let frame = OutlineFrame {
            origin: [1.0, 2.0, 3.0],
            right: [0.0, 0.0, 1.0],
            up: [0.0, 1.0, 0.0],
            forward: [-1.0, 0.0, 0.0],
            scale: [1.0, 1.0],
        };
        let point = frame.at([0.25, -0.5], 2.0);
        let (on_frame, t) = frame.of(point);
        assert!((on_frame[0] - 0.25).abs() < 1e-5);
        assert!((on_frame[1] - -0.5).abs() < 1e-5);
        assert!((t - 2.0).abs() < 1e-5);
    }

    #[test]
    fn an_ordinary_lasso_is_well_inside_what_it_may_cost() {
        // A lasso over a subtool the size of the ones this application opens
        // with must never meet the ceiling: it is there for the gesture that
        // would hang, not as a budget ordinary work is measured against.
        let lasso = square(0.5);
        let bounds = ([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let cells = cells_to_write(&lasso, bounds, 0.02);
        assert!(cells > 0.0);
        assert!(
            cells < CELL_CEILING * 0.5,
            "an ordinary lasso costs {cells} writes, most of what is allowed"
        );
    }

    #[test]
    fn a_form_too_large_to_freeze_at_once_is_over_the_ceiling() {
        // The gesture that would otherwise appear to hang: a lasso around the
        // whole of a subtool a hundred units across, at a two-centimetre cell.
        let lasso = MaskOutline {
            outline: vec![[-50.0, -50.0], [50.0, -50.0], [50.0, 50.0], [-50.0, 50.0]],
            frame: frame(),
            mode: OutlineMode::Freeze,
        };
        let bounds = ([-50.0, -50.0, -50.0], [50.0, 50.0, 50.0]);
        assert!(cells_to_write(&lasso, bounds, 0.02) > CELL_CEILING);
    }

    #[test]
    fn a_smaller_outline_over_the_same_form_costs_less() {
        // What the refusal asks a sculptor to do has to actually help.
        let bounds = ([-5.0, -5.0, -5.0], [5.0, 5.0, 5.0]);
        let whole = cells_to_write(&square(5.0), bounds, 0.02);
        let part = cells_to_write(&square(1.0), bounds, 0.02);
        assert!(part < whole / 4.0, "{part} against {whole}");
    }

    #[test]
    fn every_gesture_answers_all_three_questions_about_itself() {
        // Two of the answers coincide for the gestures there are today, and
        // they are still two questions: a circle would trace no path and close
        // no gap, and a polygon clicked corner by corner would close a gap
        // without tracing one. Pinned per gesture so a fourth has to answer
        // both rather than inherit whichever the last one happened to give.
        let table = [
            (MaskGesture::Brush, false, false, false),
            (MaskGesture::Lasso, true, true, true),
            (MaskGesture::Rectangle, true, false, false),
        ];
        assert_eq!(table.len(), MaskGesture::ALL.len(), "a gesture has no row");
        for (gesture, draws, traces, closes) in table {
            assert_eq!(gesture.draws_an_outline(), draws, "{gesture:?} draws");
            assert_eq!(gesture.traces_the_path(), traces, "{gesture:?} traces");
            assert_eq!(gesture.closes_a_gap(), closes, "{gesture:?} closes");
            assert!(!gesture.label().is_empty());
        }
    }

    #[test]
    fn the_two_modes_paint_toward_the_two_ends() {
        assert_eq!(OutlineMode::Freeze.target(), 1.0);
        assert_eq!(OutlineMode::Thaw.target(), 0.0);
        for mode in OutlineMode::ALL {
            assert!(!mode.label().is_empty());
        }
    }
}
