//! Palette-indexed voxel grids and the verbs that sculpt them.
//!
//! Every verb reads a snapshot of the region first, so a cell's outcome never
//! depends on a neighbour the same call already changed.
//!
//! # Telling a live edit from a dead one
//!
//! Many verbs can be a valid call that changes nothing — a sub-cell grab, a
//! dithered stamp that misses every cell it was offered, a footprint over
//! empty space. None of that is an error. Compare [`VoxelField::change_count`]
//! across the call to find out; `occupied_count` is not a substitute, because
//! grab and magnify move material without adding any.
//!
//! # Owned and borrowed
//!
//! As with masks, a grid created standalone is owned and a grid obtained from
//! a document layer is borrowed. [`VoxelGrid`] releases on drop;
//! [`VoxelGridRef`] cannot outlive its document and has no destroy operation.

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

use claycore_sys as sys;

use crate::brush::{BrushParams, StrokePreset, StrokeSample};
use crate::descriptor::Descriptor;
use crate::error::{check, ErrorKind, Result};
use crate::mask::{MaskExtrudeParams, MaskField};
use crate::mesh::Mesh;
use crate::{raw_failure, Document, LayerId};

/// A cell coordinate.
pub type Cell = [i32; 3];

/// What a pre-bake check found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairReport {
    /// Empty regions the outside cannot reach.
    pub enclosed_voids: usize,
    /// Their total size in cells.
    pub void_cells: usize,
    pub largest_void: usize,
    /// True when there are no enclosed voids at all.
    pub airtight: bool,
}

/// Every operation a voxel grid supports, regardless of who owns it.
#[repr(transparent)]
pub struct VoxelField {
    raw: NonNull<sys::clay_voxel_grid>,
}

impl VoxelField {
    pub(crate) fn as_ptr(&self) -> *mut sys::clay_voxel_grid {
        self.raw.as_ptr()
    }

    /// World units per cell at the active level.
    pub fn voxel_size(&self) -> Result<f32> {
        let mut value = 0.0f32;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_size(self.as_ptr(), &mut value) },
            "clay_voxel_size",
        )?;
        Ok(value)
    }

    /// Cells currently occupied.
    pub fn occupied_count(&self) -> Result<usize> {
        let mut count = 0usize;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_occupied_count(self.as_ptr(), &mut count) },
            "clay_voxel_occupied_count",
        )?;
        Ok(count)
    }

    /// Cell writes that actually changed a cell.
    ///
    /// Monotone, and meaningful only as a difference across a call. Pinch and
    /// magnify may revisit a cell within one call, so for those it is an upper
    /// bound rather than an exact tally.
    pub fn change_count(&self) -> Result<u64> {
        let mut count = 0u64;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_change_count(self.as_ptr(), &mut count) },
            "clay_voxel_change_count",
        )?;
        Ok(count)
    }

    /// The occupied region, when anything is occupied.
    pub fn bounds(&self) -> Result<Option<(Cell, Cell)>> {
        let (mut min, mut max) = ([0i32; 3], [0i32; 3]);
        let mut has = 0i32;
        // SAFETY: two three-int out-parameters and a flag.
        check(
            unsafe {
                sys::clay_voxel_bounds(self.as_ptr(), min.as_mut_ptr(), max.as_mut_ptr(), &mut has)
            },
            "clay_voxel_bounds",
        )?;
        Ok((has != 0).then_some((min, max)))
    }

    // -- palette ------------------------------------------------------------

    /// Adds a colour and returns its index.
    pub fn palette_add(&mut self, rgb: [f32; 3]) -> Result<i32> {
        let mut index = 0i32;
        // SAFETY: three-float input and a valid out-parameter.
        check(
            unsafe { sys::clay_voxel_palette_add(self.as_ptr(), rgb.as_ptr(), &mut index) },
            "clay_voxel_palette_add",
        )?;
        Ok(index)
    }

    /// How many colours the palette holds, counting the unused empty slot at
    /// index 0 — so a fresh grid reports 1.
    pub fn palette_size(&self) -> Result<usize> {
        let mut size = 0usize;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_palette_size(self.as_ptr(), &mut size) },
            "clay_voxel_palette_size",
        )?;
        Ok(size)
    }

    /// A palette entry's colour.
    pub fn palette_color(&self, index: i32) -> Result<[f32; 3]> {
        let mut rgb = [0.0f32; 3];
        // SAFETY: a three-float out-parameter.
        check(
            unsafe { sys::clay_voxel_palette_color(self.as_ptr(), index, rgb.as_mut_ptr()) },
            "clay_voxel_palette_color",
        )?;
        Ok(rgb)
    }

    // -- resolution levels --------------------------------------------------

    /// How many resolution levels the grid carries.
    ///
    /// The coarsest level is the one that was always there, so a grid with a
    /// single level behaves exactly as it did before multi-resolution existed.
    pub fn level_count(&self) -> Result<usize> {
        let mut count = 0usize;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_level_count(self.as_ptr(), &mut count) },
            "clay_voxel_level_count",
        )?;
        Ok(count)
    }

    /// Which level the verbs currently edit.
    pub fn active_level(&self) -> Result<usize> {
        let mut level = 0usize;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_active_level(self.as_ptr(), &mut level) },
            "clay_voxel_active_level",
        )?;
        Ok(level)
    }

    /// Chooses which level the verbs edit.
    pub fn set_active_level(&mut self, level: usize) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_voxel_set_active_level(self.as_ptr(), level) },
            "clay_voxel_set_active_level",
        )
    }

    /// Pushes a finer level and returns its index.
    ///
    /// Blocking a form out wants coarse cells and detailing wants fine ones;
    /// this is how to get the second without paying for it everywhere.
    pub fn add_level(&mut self) -> Result<usize> {
        let mut level = 0usize;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_add_level(self.as_ptr(), &mut level) },
            "clay_voxel_add_level",
        )?;
        Ok(level)
    }

    /// Discards the finest level.
    /// Refines a region rather than the whole grid.
    ///
    /// The point of the level stack: block out coarse, then pay for detail
    /// only where the detail goes. A whole-grid `add_level` pays for it
    /// everywhere, which on a large sculpt is most of the memory budget spent
    /// on the parts nobody is looking at.
    pub fn add_level_region(&mut self, min: [f32; 3], max: [f32; 3]) -> Result<usize> {
        let mut level = 0;
        // SAFETY: valid handle, two arrays of three floats, out-parameter
        // written on success.
        check(
            unsafe {
                sys::clay_voxel_add_level_region(
                    self.as_ptr(),
                    min.as_ptr(),
                    max.as_ptr(),
                    &mut level,
                )
            },
            "clay_voxel_add_level_region",
        )?;
        Ok(level)
    }

    pub fn drop_level(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_voxel_drop_level(self.as_ptr()) },
            "clay_voxel_drop_level",
        )
    }

    /// World units per cell at a given level.
    pub fn level_voxel_size(&self, level: usize) -> Result<f32> {
        let mut value = 0.0f32;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_level_voxel_size(self.as_ptr(), level, &mut value) },
            "clay_voxel_level_voxel_size",
        )?;
        Ok(value)
    }

    // -- direct cell edits --------------------------------------------------

    /// The palette index at a cell, or `None` where it is empty.
    ///
    /// Index 0 is the engine's empty slot — never added, never matched, never
    /// recoloured — so it is reported here as absence rather than as a colour.
    pub fn get(&self, cell: Cell) -> Result<Option<i32>> {
        let mut index = 0i32;
        // SAFETY: three-int input and a valid out-parameter.
        check(
            unsafe { sys::clay_voxel_get(self.as_ptr(), cell.as_ptr(), &mut index) },
            "clay_voxel_get",
        )?;
        Ok((index > 0).then_some(index))
    }

    /// Fills one cell.
    pub fn set(&mut self, cell: Cell, index: i32) -> Result<()> {
        // SAFETY: three-int input.
        check(
            unsafe { sys::clay_voxel_set(self.as_ptr(), cell.as_ptr(), index) },
            "clay_voxel_set",
        )
    }

    /// Empties one cell.
    pub fn erase(&mut self, cell: Cell) -> Result<()> {
        // SAFETY: three-int input.
        check(
            unsafe { sys::clay_voxel_erase(self.as_ptr(), cell.as_ptr()) },
            "clay_voxel_erase",
        )
    }

    /// Recolours an occupied cell without filling an empty one.
    pub fn paint(&mut self, cell: Cell, index: i32) -> Result<()> {
        // SAFETY: three-int input.
        check(
            unsafe { sys::clay_voxel_paint(self.as_ptr(), cell.as_ptr(), index) },
            "clay_voxel_paint",
        )
    }

    /// Fills an axis-aligned box of cells.
    pub fn fill_box(&mut self, a: Cell, b: Cell, index: i32) -> Result<()> {
        // SAFETY: two three-int inputs.
        check(
            unsafe { sys::clay_voxel_fill_box(self.as_ptr(), a.as_ptr(), b.as_ptr(), index) },
            "clay_voxel_fill_box",
        )
    }

    /// Fills a line of cells.
    pub fn fill_line(&mut self, a: Cell, b: Cell, index: i32) -> Result<()> {
        // SAFETY: two three-int inputs.
        check(
            unsafe { sys::clay_voxel_fill_line(self.as_ptr(), a.as_ptr(), b.as_ptr(), index) },
            "clay_voxel_fill_line",
        )
    }

    /// Fills with a brush footprint.
    pub fn set_brush(&mut self, cell: Cell, brush: &BrushParams<'_>, index: i32) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: three-int input and a descriptor with struct_size set.
        check(
            unsafe { sys::clay_voxel_set_brush(self.as_ptr(), cell.as_ptr(), &raw, index) },
            "clay_voxel_set_brush",
        )
    }

    /// Erases with a brush footprint.
    pub fn erase_brush(&mut self, cell: Cell, brush: &BrushParams<'_>) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: as `set_brush`.
        check(
            unsafe { sys::clay_voxel_erase_brush(self.as_ptr(), cell.as_ptr(), &raw) },
            "clay_voxel_erase_brush",
        )
    }

    /// Recolours with a brush footprint.
    pub fn paint_brush(&mut self, cell: Cell, brush: &BrushParams<'_>, index: i32) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: as `set_brush`.
        check(
            unsafe { sys::clay_voxel_paint_brush(self.as_ptr(), cell.as_ptr(), &raw, index) },
            "clay_voxel_paint_brush",
        )
    }

    // -- sculpting verbs ----------------------------------------------------

    /// Majority filter over the 26-neighbourhood: spurs dissolve, notches fill.
    /// Fills this grid from a document's field, over a region.
    ///
    /// SDF to voxel: cells whose centre evaluates inside are set, and colour
    /// comes from the tape's colour field by nearest palette entry.
    ///
    /// Lossy in the ways lattice sampling is, and the engine states them
    /// rather than hiding them: the surface moves by up to half a cell, a
    /// feature thinner than a cell can vanish, a sharp edge becomes a
    /// staircase at the cell size, and only the region passed is rasterized.
    /// A caller that wants those numbers rather than the prose can compute
    /// them from [`VoxelGrid::voxel_size`].
    pub fn rasterize(
        &mut self,
        document: &crate::Document,
        region: ([f32; 3], [f32; 3]),
    ) -> Result<()> {
        let (min, max) = region;
        // SAFETY: valid handles, and two arrays of three floats each, which is
        // what the entry point reads.
        check(
            unsafe {
                sys::clay_voxel_rasterize(
                    self.as_ptr(),
                    document.as_ptr() as *const _,
                    min.as_ptr(),
                    max.as_ptr(),
                )
            },
            "clay_voxel_rasterize",
        )
    }

    /// Fills this grid from a mesh's triangles, in one sampling.
    ///
    /// Not the same as meshing to a volume and rasterizing that. The detour
    /// pays two samplings — triangles into a narrow band, then the band into
    /// cells — and each places the surface within about half a cell of its own
    /// lattice, so the second quantises what the first already quantised. This
    /// asks the triangles directly, which is why a feature thinner than a cell
    /// survives here where the detour loses it, and why the model's vertex
    /// colours reach the palette at all: a distance field carries no colour.
    ///
    /// The region is not optional as it is for a document. A document may be
    /// unbounded; a mesh cannot be.
    pub fn rasterize_mesh(
        &mut self,
        mesh: &crate::Mesh,
        region: ([f32; 3], [f32; 3]),
    ) -> Result<()> {
        let (min, max) = region;
        // SAFETY: valid handles and two arrays of three floats.
        check(
            unsafe {
                sys::clay_voxel_rasterize_mesh(
                    self.as_ptr(),
                    mesh.as_ptr(),
                    min.as_ptr(),
                    max.as_ptr(),
                )
            },
            "clay_voxel_rasterize_mesh",
        )
    }

    pub fn sculpt_smooth(&mut self, cell: Cell, brush: &BrushParams<'_>) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: three-int input and a sized descriptor.
        check(
            unsafe { sys::clay_voxel_sculpt_smooth(self.as_ptr(), cell.as_ptr(), &raw) },
            "clay_voxel_sculpt_smooth",
        )
    }

    /// Dilates for a positive amount, erodes for a negative one.
    pub fn sculpt_inflate(
        &mut self,
        cell: Cell,
        brush: &BrushParams<'_>,
        amount: i32,
    ) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: as above.
        check(
            unsafe { sys::clay_voxel_sculpt_inflate(self.as_ptr(), cell.as_ptr(), &raw, amount) },
            "clay_voxel_sculpt_inflate",
        )
    }

    /// Pulls the surface onto a plane through the brush centre. Two-sided.
    pub fn sculpt_flatten(
        &mut self,
        cell: Cell,
        brush: &BrushParams<'_>,
        normal: [f32; 3],
        offset_cells: f32,
    ) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: as above, plus a three-float normal.
        check(
            unsafe {
                sys::clay_voxel_sculpt_flatten(
                    self.as_ptr(),
                    cell.as_ptr(),
                    &raw,
                    normal.as_ptr(),
                    offset_cells,
                )
            },
            "clay_voxel_sculpt_flatten",
        )
    }

    /// Moves surface cells one step toward the brush centre.
    pub fn sculpt_pinch(&mut self, cell: Cell, brush: &BrushParams<'_>) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: as above.
        check(
            unsafe { sys::clay_voxel_sculpt_pinch(self.as_ptr(), cell.as_ptr(), &raw) },
            "clay_voxel_sculpt_pinch",
        )
    }

    /// The inverse of pinch, sharing its walk so the two cannot drift apart.
    pub fn sculpt_magnify(&mut self, cell: Cell, brush: &BrushParams<'_>) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: as above.
        check(
            unsafe { sys::clay_voxel_sculpt_magnify(self.as_ptr(), cell.as_ptr(), &raw) },
            "clay_voxel_sculpt_magnify",
        )
    }

    /// Translates occupancy through the same inverse map the SDF grab uses.
    ///
    /// Resampling is nearest-cell and rounds per axis, so a displacement under
    /// half a cell on every axis moves nothing. A drag fed raw pointer deltas
    /// is dead until the caller accumulates them past the voxel size.
    pub fn sculpt_grab(
        &mut self,
        cell: Cell,
        brush: &BrushParams<'_>,
        displacement: [f32; 3],
        front_only: bool,
    ) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: as above, plus a three-float displacement.
        check(
            unsafe {
                sys::clay_voxel_sculpt_grab(
                    self.as_ptr(),
                    cell.as_ptr(),
                    &raw,
                    displacement.as_ptr(),
                    i32::from(front_only),
                )
            },
            "clay_voxel_sculpt_grab",
        )
    }

    /// Flattens *and* smooths from one snapshot. Calling both in sequence is
    /// not the same thing.
    pub fn sculpt_scrape(
        &mut self,
        cell: Cell,
        brush: &BrushParams<'_>,
        normal: [f32; 3],
        offset_cells: f32,
    ) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: as `sculpt_flatten`.
        check(
            unsafe {
                sys::clay_voxel_sculpt_scrape(
                    self.as_ptr(),
                    cell.as_ptr(),
                    &raw,
                    normal.as_ptr(),
                    offset_cells,
                )
            },
            "clay_voxel_sculpt_scrape",
        )
    }

    /// Drags surface material along a direction, leaving the interior.
    ///
    /// Grab moves a lump; smudge smears a skin.
    pub fn sculpt_smudge(
        &mut self,
        cell: Cell,
        brush: &BrushParams<'_>,
        displacement: [f32; 3],
    ) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: as `sculpt_grab`.
        check(
            unsafe {
                sys::clay_voxel_sculpt_smudge(
                    self.as_ptr(),
                    cell.as_ptr(),
                    &raw,
                    displacement.as_ptr(),
                )
            },
            "clay_voxel_sculpt_smudge",
        )
    }

    /// Fills narrow pockets.
    ///
    /// The rule is local, so it fills what is *narrow*, not what is enclosed:
    /// a through-hole wider than one cell does not qualify. Use
    /// [`Self::repair_fill_voids`] for sealed voids; neither substitutes for
    /// the other.
    pub fn sculpt_fill_cavities(
        &mut self,
        cell: Cell,
        brush: &BrushParams<'_>,
        passes: i32,
    ) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: as above.
        check(
            unsafe {
                sys::clay_voxel_sculpt_fill_cavities(self.as_ptr(), cell.as_ptr(), &raw, passes)
            },
            "clay_voxel_sculpt_fill_cavities",
        )
    }

    // -- sculpt layers ------------------------------------------------------

    /// Starts recording a sculpt layer.
    ///
    /// A pass that can be dialled back after it is made — ZBrush's layers, on a
    /// grid. Bracket any run of edits with begin and end, and the grid
    /// remembers what those edits *changed*, so their strength stays adjustable
    /// long after the strokes are finished. Not undo: undo is a stack you pop,
    /// a sculpt layer is a slider you keep.
    ///
    /// A layer stores what its pass did, not the brushes that did it, so
    /// dialling one replays recorded cells rather than re-running strokes — a
    /// pass whose result depended on the layer under it keeps what it recorded
    /// when that layer is dialled away.
    ///
    /// Rejected while a layer is already recording: a cell can only belong to
    /// one pass, so nesting has no meaning.
    pub fn begin_sculpt_layer(&mut self, name: Option<&str>) -> Result<usize> {
        let c_name = name
            .map(|name| crate::cstring(name, "clay_voxel_begin_sculpt_layer"))
            .transpose()?;
        let mut layer = 0usize;
        // SAFETY: valid handle; the name is NUL-terminated for as long as the
        // call runs, or null; `layer` is written only on success.
        check(
            unsafe {
                sys::clay_voxel_begin_sculpt_layer(
                    self.as_ptr(),
                    c_name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr()),
                    &mut layer,
                )
            },
            "clay_voxel_begin_sculpt_layer",
        )?;
        Ok(layer)
    }

    /// Stops recording. Edits after this belong to no layer, and a no-op when
    /// nothing is recording.
    pub fn end_sculpt_layer(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_voxel_end_sculpt_layer(self.as_ptr()) },
            "clay_voxel_end_sculpt_layer",
        )
    }

    /// Whether a layer is being recorded right now.
    pub fn recording_sculpt_layer(&self) -> Result<bool> {
        let mut recording = 0i32;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_recording_sculpt_layer(self.as_ptr(), &mut recording) },
            "clay_voxel_recording_sculpt_layer",
        )?;
        Ok(recording != 0)
    }

    pub fn sculpt_layer_count(&self) -> Result<usize> {
        let mut count = 0usize;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_sculpt_layer_count(self.as_ptr(), &mut count) },
            "clay_voxel_sculpt_layer_count",
        )?;
        Ok(count)
    }

    /// A layer's name, which may be empty.
    pub fn sculpt_layer_name(&self, layer: usize) -> Result<String> {
        let grid = self.as_ptr();
        crate::buffer::size_query_string("clay_voxel_sculpt_layer_name", |buffer, size| {
            // SAFETY: the two-call protocol, driven by the shared helper: a
            // null buffer asks the size and a sized one is filled.
            unsafe { sys::clay_voxel_sculpt_layer_name(grid, layer, buffer, size) }
        })
    }

    /// How many cells the pass changed — its cost, and whether it did anything.
    pub fn sculpt_layer_cell_count(&self, layer: usize) -> Result<usize> {
        let mut count = 0usize;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_sculpt_layer_cell_count(self.as_ptr(), layer, &mut count) },
            "clay_voxel_sculpt_layer_cell_count",
        )?;
        Ok(count)
    }

    pub fn sculpt_layer_strength(&self, layer: usize) -> Result<f32> {
        let mut strength = 0.0f32;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_sculpt_layer_strength(self.as_ptr(), layer, &mut strength) },
            "clay_voxel_sculpt_layer_strength",
        )?;
        Ok(strength)
    }

    /// Dials a layer up or down.
    ///
    /// On binary occupancy a fractional strength is a fraction of the *cells*,
    /// chosen by the same cell-coordinate hash the falloff brushes dither with:
    /// the same strength picks the same cells on every platform and every run,
    /// and raising it adds cells to the ones already showing rather than
    /// reshuffling. 0 and 1 are exact — the grid without the pass, and the pass
    /// applied directly.
    ///
    /// Clamped by the engine rather than rejected: a slider that overshoots is
    /// a caller being a caller.
    pub fn set_sculpt_layer_strength(&mut self, layer: usize, strength: f32) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_voxel_set_sculpt_layer_strength(self.as_ptr(), layer, strength) },
            "clay_voxel_set_sculpt_layer_strength",
        )
    }

    pub fn sculpt_layer_visible(&self, layer: usize) -> Result<bool> {
        let mut visible = 0i32;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_sculpt_layer_visible(self.as_ptr(), layer, &mut visible) },
            "clay_voxel_sculpt_layer_visible",
        )?;
        Ok(visible != 0)
    }

    pub fn set_sculpt_layer_visible(&mut self, layer: usize, visible: bool) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe {
                sys::clay_voxel_set_sculpt_layer_visible(self.as_ptr(), layer, i32::from(visible))
            },
            "clay_voxel_set_sculpt_layer_visible",
        )
    }

    /// Drops a layer and replays the ones above it.
    pub fn remove_sculpt_layer(&mut self, layer: usize) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_voxel_remove_sculpt_layer(self.as_ptr(), layer) },
            "clay_voxel_remove_sculpt_layer",
        )
    }

    /// Folds a layer into the one below at full strength, keeping the lower
    /// layer's name. Rejected for layer 0, which has nothing below it.
    pub fn merge_sculpt_layer_down(&mut self, layer: usize) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_voxel_merge_sculpt_layer_down(self.as_ptr(), layer) },
            "clay_voxel_merge_sculpt_layer_down",
        )
    }

    /// Moves a layer within the stack, sliding the rest along.
    ///
    /// Order is meaningful: where two passes touched the same cell, moving one
    /// past the other changes which value survives. The recorded diffs are
    /// replayed in the new order rather than the strokes re-run.
    pub fn move_sculpt_layer(&mut self, from: usize, to: usize) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_voxel_move_sculpt_layer(self.as_ptr(), from, to) },
            "clay_voxel_move_sculpt_layer",
        )
    }

    /// What one layer costs in memory: recorded cells plus the recording index.
    pub fn sculpt_layer_bytes(&self, layer: usize) -> Result<usize> {
        let mut bytes = 0usize;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_sculpt_layer_bytes(self.as_ptr(), layer, &mut bytes) },
            "clay_voxel_sculpt_layer_bytes",
        )?;
        Ok(bytes)
    }

    /// What the whole stack costs.
    ///
    /// Nothing is enforced, deliberately: a cap that silently stopped recording
    /// would leave a pass on the grid and un-dialable, which is a correctness
    /// bug wearing a memory limit's clothes. A host with a budget merges layers
    /// down — one entry per cell instead of two — or stops recording.
    pub fn sculpt_layers_bytes(&self) -> Result<usize> {
        let mut bytes = 0usize;
        // SAFETY: valid handle and out-parameter.
        check(
            unsafe { sys::clay_voxel_sculpt_layers_bytes(self.as_ptr(), &mut bytes) },
            "clay_voxel_sculpt_layers_bytes",
        )?;
        Ok(bytes)
    }

    /// A caller-supplied scalar stamp modulating per-cell strength.
    ///
    /// The engine decodes no images: a caller with an alpha has already loaded
    /// it.
    // Mirrors the C entry point's parameter list. Grouping them into a
    // struct here would mean a second shape to keep in step with the ABI.
    #[allow(clippy::too_many_arguments)]
    pub fn sculpt_carve_alpha(
        &mut self,
        cell: Cell,
        brush: &BrushParams<'_>,
        alpha: &[f32],
        width: i32,
        height: i32,
        direction: [f32; 3],
        index: i32,
    ) -> Result<()> {
        let raw = brush.to_raw();
        // SAFETY: `alpha` is `width * height` floats, which the caller states
        // and the engine validates; direction is three floats.
        check(
            unsafe {
                sys::clay_voxel_sculpt_carve_alpha(
                    self.as_ptr(),
                    cell.as_ptr(),
                    &raw,
                    alpha.as_ptr(),
                    width,
                    height,
                    direction.as_ptr(),
                    index,
                )
            },
            "clay_voxel_sculpt_carve_alpha",
        )
    }

    /// Applies a stroke, resolving it into stamps through the engine's stroke
    /// engine. Returns how many stamps landed.
    pub fn apply_stroke(
        &mut self,
        samples: &[StrokeSample],
        preset: &StrokePreset,
        index: i32,
        shape: crate::brush::BrushShape,
        falloff: crate::brush::Falloff,
        mask: Option<&MaskField>,
    ) -> Result<usize> {
        if samples.is_empty() {
            return Ok(0);
        }
        let flat = StrokeSample::flatten(samples);
        let raw_preset = preset.to_raw();
        let mut applied = 0usize;
        // SAFETY: `flat` is `samples.len() * 5` floats; the preset carries its
        // struct_size; the mask is either a valid handle or null.
        check(
            unsafe {
                sys::clay_voxel_apply_stroke(
                    self.as_ptr(),
                    flat.as_ptr(),
                    samples.len(),
                    &raw_preset,
                    index,
                    crate::brush::shape_raw(shape),
                    crate::brush::falloff_raw(falloff),
                    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _),
                    &mut applied,
                )
            },
            "clay_voxel_apply_stroke",
        )?;
        Ok(applied)
    }

    // -- repair -------------------------------------------------------------

    /// What a pre-bake check wants to know, without performing the fix.
    pub fn repair_report(&self) -> Result<RepairReport> {
        let mut raw = sys::clay_repair_report::sized();
        // SAFETY: valid handle and a descriptor with struct_size set.
        check(
            unsafe { sys::clay_voxel_repair_report(self.as_ptr(), &mut raw) },
            "clay_voxel_repair_report",
        )?;
        Ok(RepairReport {
            enclosed_voids: raw.enclosed_voids,
            void_cells: raw.void_cells,
            largest_void: raw.largest_void,
            airtight: raw.airtight != 0,
        })
    }

    /// Seals perforations. Only ever adds cells.
    pub fn repair_close_holes(&mut self, passes: i32, mask: Option<&MaskField>) -> Result<()> {
        // SAFETY: the mask is either a valid handle or null.
        check(
            unsafe {
                sys::clay_voxel_repair_close_holes(
                    self.as_ptr(),
                    passes,
                    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _),
                )
            },
            "clay_voxel_repair_close_holes",
        )
    }

    /// Fills every empty cell the outside cannot reach. Enclosure is decided,
    /// not guessed locally.
    pub fn repair_fill_voids(&mut self, mask: Option<&MaskField>) -> Result<()> {
        // SAFETY: as above.
        check(
            unsafe {
                sys::clay_voxel_repair_fill_voids(
                    self.as_ptr(),
                    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _),
                )
            },
            "clay_voxel_repair_fill_voids",
        )
    }

    // -- output -------------------------------------------------------------

    /// Meshes the grid.
    pub fn mesh(&self) -> Result<Mesh> {
        let mut mesh = std::ptr::null_mut();
        // SAFETY: valid handle; `mesh` written only on success.
        check(
            unsafe { sys::clay_voxel_mesh(self.as_ptr(), &mut mesh) },
            "clay_voxel_mesh",
        )?;
        Mesh::from_raw(mesh, "clay_voxel_mesh")
    }

    /// Meshes the grid as a rounded form rather than as boxes.
    ///
    /// Surface nets over occupancy sampled at cell centres: one vertex per
    /// surface cell, at the centroid of that cell's edge crossings. The
    /// greedy mesh [`VoxelGrid::mesh`] returns is the export path and is
    /// correct for hard-surface work; this is the one a sculpt is looked at
    /// through, because axis-aligned quads are the wrong picture of clay.
    ///
    /// `blur` is extra smoothing, in passes of a 3x3x3 box over occupancy,
    /// and the trade runs both ways: at 0 nothing is filtered and nothing can
    /// be lost, but the surface still terraces; at 1 it reads as clay and an
    /// isolated voxel falls under the isolevel and is gone. The engine
    /// defaults to 0 for that reason, and so does anything here that has a
    /// sculptor's thin detail to lose.
    ///
    /// A preview mesh: per the surface-nets contract a cell the surface
    /// crosses twice gets one vertex and the sheets pinch, so it is neither
    /// manifold nor watertight.
    ///
    /// Two things to know before drawing a sculpt with it, both measured on a
    /// 0.01 grid holding 429,098 cells:
    ///
    /// - It carries **no vertex normals**, where [`VoxelField::mesh`] does. A
    ///   host that lights by normal draws a flat silhouette from this and has
    ///   to compute its own.
    /// - It is whole-grid and has no chunked form, so an edit costs the model:
    ///   152 ms against the greedy mesh's 16 ms, and 3.3 ms for the chunks a
    ///   dab actually dirties. [`VoxelField::take_dirty_chunks`] and
    ///   [`VoxelField::mesh_chunks`] are the interactive path.
    pub fn mesh_smooth(&self, blur: i32) -> Result<Mesh> {
        let mut mesh = std::ptr::null_mut();
        // SAFETY: valid handle; `mesh` written only on success.
        check(
            unsafe { sys::clay_voxel_mesh_smooth(self.as_ptr(), blur, &mut mesh) },
            "clay_voxel_mesh_smooth",
        )?;
        Mesh::from_raw(mesh, "clay_voxel_mesh_smooth")
    }

    /// The chunk keys whose geometry a host must rebuild, drained.
    ///
    /// Capacity in, count out, and the remainder stays queued — the shape
    /// [`crate::BrickCache::take_dirty`] uses. Every public mutation reports
    /// through this: a write that changes a cell dirties its chunk, and one on
    /// a chunk face also dirties the chunk across it, whose exposed faces it
    /// changed. A chunk emptied to nothing is reported too, because that is
    /// the key whose geometry has to be *removed*.
    ///
    /// The engine's drain is all-or-nothing, so a key can be reported twice
    /// across two drains — re-meshing a chunk that did not change is wasted
    /// work, never a wrong surface.
    ///
    /// A grid just created from a file, rasterized, or given a level reports
    /// every chunk it wrote, so a first display and an incremental one are the
    /// same code path.
    pub fn take_dirty_chunks(&mut self, max: usize) -> Result<(Vec<[i32; 3]>, usize)> {
        if max == 0 {
            return Ok((Vec::new(), 0));
        }
        let mut keys = vec![[0i32; 3]; max];
        let mut count = max;
        let mut remaining = 0usize;
        // SAFETY: `keys` is valid for `max` triples of int32 and is laid out
        // as a packed array of them; `count` carries that capacity in and the
        // filled length out.
        check(
            unsafe {
                sys::clay_voxel_take_dirty_chunks(
                    self.as_ptr(),
                    keys.as_mut_ptr() as *mut i32,
                    &mut count,
                    &mut remaining,
                )
            },
            "clay_voxel_take_dirty_chunks",
        )?;
        keys.truncate(count);
        Ok((keys, remaining))
    }

    /// Meshes only the named chunks, and says what each one contributed.
    ///
    /// The surface over those chunks is exactly the one [`VoxelField::mesh`]
    /// describes — the exposure test reads the neighbour cell wherever it
    /// lives, including in a chunk that was not named. What differs is the
    /// merge: greedy quads clamped to a chunk boundary emit more, smaller
    /// quads over the identical surface, never a crack.
    ///
    /// The ranges *partition* the mesh, so a host may overwrite or drop one
    /// key's slice without consulting its neighbours' — unlike the brick
    /// cache, whose marching cells straddle a boundary. A key naming a chunk
    /// the grid no longer holds is not an error: its range is empty, and that
    /// is exactly the key whose geometry the host must drop.
    pub fn mesh_chunks(&self, keys: &[[i32; 3]]) -> Result<(Mesh, Vec<ChunkRange>)> {
        let mut ranges = vec![sys::clay_voxel_chunk_mesh_range::default(); keys.len()];
        let mut mesh = std::ptr::null_mut();
        // SAFETY: `keys` is a packed array of int32 triples of the length
        // given, `ranges` holds one element per key, and `mesh` is written
        // only on success.
        check(
            unsafe {
                sys::clay_voxel_mesh_chunks(
                    self.as_ptr(),
                    keys.as_ptr() as *const i32,
                    keys.len(),
                    ranges.as_mut_ptr(),
                    &mut mesh,
                )
            },
            "clay_voxel_mesh_chunks",
        )?;
        let mesh = Mesh::from_raw(mesh, "clay_voxel_mesh_chunks")?;
        Ok((
            mesh,
            ranges
                .into_iter()
                .map(|range| ChunkRange {
                    key: range.key,
                    vertex_first: range.vertex_first as usize,
                    vertex_count: range.vertex_count as usize,
                    index_first: range.index_first as usize,
                    index_count: range.index_count as usize,
                })
                .collect(),
        ))
    }

    // -- picking ------------------------------------------------------------

    /// The first occupied cell along a ray, and how far away it is.
    ///
    /// The direction is normalized by the engine, and the distance is to the
    /// entry point of that cell — so the world position of the hit is the
    /// origin plus the *unit* direction times the distance.
    ///
    /// `None` where the ray meets nothing, which is not an error: most rays a
    /// pointer casts miss.
    pub fn raycast(&self, origin: [f32; 3], direction: [f32; 3]) -> Result<Option<VoxelHit>> {
        let mut hit = 0i32;
        let mut cell = [0i32; 3];
        let mut face = 0i32;
        let mut adjacent = [0i32; 3];
        let mut distance = 0.0f32;
        // SAFETY: valid handle, two three-element inputs, and out-parameters
        // that are written only on a hit — `hit` always.
        check(
            unsafe {
                sys::clay_voxel_raycast(
                    self.as_ptr(),
                    origin.as_ptr(),
                    direction.as_ptr(),
                    &mut hit,
                    cell.as_mut_ptr(),
                    &mut face,
                    adjacent.as_mut_ptr(),
                    &mut distance,
                )
            },
            "clay_voxel_raycast",
        )?;
        Ok((hit != 0).then_some(VoxelHit {
            cell,
            adjacent,
            face,
            distance,
        }))
    }

    /// Pulls the masked patch off as a solid grid the caller owns.
    pub fn mask_extrude(&self, mask: &MaskField, params: MaskExtrudeParams) -> Result<VoxelGrid> {
        let raw_params = params.to_raw();
        let mut grid = std::ptr::null_mut();
        // SAFETY: both handles valid, descriptor sized; `grid` written only on
        // success and owned by the caller thereafter.
        check(
            unsafe {
                sys::clay_voxel_mask_extrude(self.as_ptr(), mask.as_ptr(), &raw_params, &mut grid)
            },
            "clay_voxel_mask_extrude",
        )?;
        VoxelGrid::from_raw(grid, "clay_voxel_mask_extrude")
    }
}

impl std::fmt::Debug for VoxelField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoxelField")
            .field("occupied", &self.occupied_count().unwrap_or(0))
            .finish()
    }
}

/// A voxel grid the caller owns, released on drop.
#[derive(Debug)]
pub struct VoxelGrid {
    inner: VoxelField,
}

// SAFETY: a grid is host memory the engine reaches only through this handle.
unsafe impl Send for VoxelGrid {}

impl VoxelGrid {
    /// Creates a standalone grid.
    pub fn new(voxel_size: f32) -> Result<Self> {
        // SAFETY: returns an owned handle or null.
        let raw = unsafe { sys::clay_voxel_grid_create(voxel_size) };
        Self::from_raw(raw, "clay_voxel_grid_create")
    }

    fn from_raw(raw: *mut sys::clay_voxel_grid, operation: &'static str) -> Result<Self> {
        NonNull::new(raw)
            .map(|raw| Self {
                inner: VoxelField { raw },
            })
            .ok_or_else(|| raw_failure(operation, ErrorKind::InvalidArgument))
    }
}

impl Deref for VoxelGrid {
    type Target = VoxelField;
    fn deref(&self) -> &VoxelField {
        &self.inner
    }
}

impl DerefMut for VoxelGrid {
    fn deref_mut(&mut self) -> &mut VoxelField {
        &mut self.inner
    }
}

impl Drop for VoxelGrid {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once. A grid borrowed from a
        // document is a `VoxelGridRef`, which has no `Drop`.
        unsafe { sys::clay_voxel_grid_destroy(self.inner.as_ptr()) };
    }
}

/// What one chunk contributed to a chunked mesh.
///
/// The ranges partition the mesh — no vertex is shared between two keys — so
/// one key's slice can be replaced or dropped on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkRange {
    pub key: [i32; 3],
    pub vertex_first: usize,
    pub vertex_count: usize,
    pub index_first: usize,
    pub index_count: usize,
}

/// What a picking ray met in a grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelHit {
    /// The first occupied cell along the ray.
    pub cell: [i32; 3],
    /// The empty neighbour across the face it was entered through, which is
    /// where a voxel placed by that click goes.
    pub adjacent: [i32; 3],
    /// Which face that was, as a `clay_voxel_face`.
    pub face: i32,
    /// World distance from the origin to the entry point.
    pub distance: f32,
}

/// A voxel grid belonging to a document, borrowed for reading.
///
/// A shared borrow, so only the grid's `&self` operations reach through it —
/// there is no `DerefMut`. The engine's lookup takes a mutable document handle
/// because one call serves reads and writes; nothing obtained this way can
/// write, which is what lets a question about a grid be asked from a `&self`
/// method that has no mutable document to offer.
#[derive(Debug)]
pub struct VoxelReader<'doc> {
    inner: VoxelField,
    _doc: PhantomData<&'doc Document>,
}

impl VoxelReader<'_> {
    fn from_raw(raw: *mut sys::clay_voxel_grid, operation: &'static str) -> Result<Self> {
        NonNull::new(raw)
            .map(|raw| Self {
                inner: VoxelField { raw },
                _doc: PhantomData,
            })
            .ok_or_else(|| raw_failure(operation, ErrorKind::NotFound))
    }
}

impl Deref for VoxelReader<'_> {
    type Target = VoxelField;
    fn deref(&self) -> &VoxelField {
        &self.inner
    }
}

/// A voxel grid belonging to a document, borrowed for as long as it lives.
///
/// Carries no destroy operation: the engine documents destroying a borrowed
/// handle as an error, and here it is not expressible.
#[derive(Debug)]
pub struct VoxelGridRef<'doc> {
    inner: VoxelField,
    _doc: PhantomData<&'doc mut Document>,
}

impl VoxelGridRef<'_> {
    fn from_raw(raw: *mut sys::clay_voxel_grid, operation: &'static str) -> Result<Self> {
        NonNull::new(raw)
            .map(|raw| Self {
                inner: VoxelField { raw },
                _doc: PhantomData,
            })
            .ok_or_else(|| raw_failure(operation, ErrorKind::NotFound))
    }
}

impl Deref for VoxelGridRef<'_> {
    type Target = VoxelField;
    fn deref(&self) -> &VoxelField {
        &self.inner
    }
}

impl DerefMut for VoxelGridRef<'_> {
    fn deref_mut(&mut self) -> &mut VoxelField {
        &mut self.inner
    }
}

impl Document {
    /// Adds a voxel layer and lends back its grid.
    pub fn add_voxel_layer(
        &mut self,
        name: &str,
        voxel_size: f32,
    ) -> Result<(LayerId, VoxelGridRef<'_>)> {
        let c_name = crate::cstring(name, "clay_document_add_voxel_layer")?;
        let mut layer: sys::clay_layer_id = Default::default();
        let mut grid = std::ptr::null_mut();
        // SAFETY: valid handle, NUL-terminated name, two out-parameters
        // written only on success.
        check(
            unsafe {
                sys::clay_document_add_voxel_layer(
                    self.as_ptr(),
                    c_name.as_ptr(),
                    voxel_size,
                    &mut layer,
                    &mut grid,
                )
            },
            "clay_document_add_voxel_layer",
        )?;
        Ok((
            LayerId(layer),
            VoxelGridRef::from_raw(grid, "clay_document_add_voxel_layer")?,
        ))
    }

    /// The grid a named voxel layer already carries.
    pub fn voxel_layer(&mut self, name: &str) -> Result<(LayerId, VoxelGridRef<'_>)> {
        let c_name = crate::cstring(name, "clay_document_voxel_layer")?;
        let mut layer: sys::clay_layer_id = Default::default();
        let mut grid = std::ptr::null_mut();
        // SAFETY: as above.
        check(
            unsafe {
                sys::clay_document_voxel_layer(
                    self.as_ptr(),
                    c_name.as_ptr(),
                    &mut layer,
                    &mut grid,
                )
            },
            "clay_document_voxel_layer",
        )?;
        Ok((
            LayerId(layer),
            VoxelGridRef::from_raw(grid, "clay_document_voxel_layer")?,
        ))
    }

    /// Borrows a voxel layer's grid for reading only.
    ///
    /// The same lookup as [`Document::voxel_layer`], through a shared borrow.
    /// It is spelled separately rather than by relaxing that one because the
    /// mutable borrow is what keeps two writable handles to one grid from
    /// existing; a reader cannot write, so any number of them are fine.
    pub fn voxel_reader(&self, name: &str) -> Result<(LayerId, VoxelReader<'_>)> {
        let c_name = crate::cstring(name, "clay_document_voxel_layer")?;
        let mut layer: sys::clay_layer_id = Default::default();
        let mut grid = std::ptr::null_mut();
        // SAFETY: as above. The handle the engine writes is borrowed from the
        // document and is never wrapped in anything that destroys it.
        check(
            unsafe {
                sys::clay_document_voxel_layer(
                    self.as_ptr(),
                    c_name.as_ptr(),
                    &mut layer,
                    &mut grid,
                )
            },
            "clay_document_voxel_layer",
        )?;
        Ok((
            LayerId(layer),
            VoxelReader::from_raw(grid, "clay_document_voxel_layer")?,
        ))
    }
}
