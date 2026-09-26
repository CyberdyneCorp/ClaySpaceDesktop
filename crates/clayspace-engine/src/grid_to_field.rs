//! A grid-to-field crossing in three parts, so its middle can leave the
//! interface thread.
//!
//! Reading the grid and placing the field both need the document; converting
//! one into the other does not, and it is most of the cost — the volume is
//! sampled over the grid's box and a band around it and then redistanced. So
//! the crossing is split where the document stops being needed:
//!
//! 1. [`crate::ClayDocument::begin_grid_to_field`] reads the active grid out
//!    as plain data, on the interface thread.
//! 2. [`GridToField::convert`] rebuilds the grid and converts it, anywhere.
//! 3. [`crate::ClayDocument::finish_grid_to_field`] places the field, on the
//!    interface thread again, as one crossing and one undo step.
//!
//! The synchronous [`crate::ClayDocument::convert_layer`] runs the same three
//! parts back to back, so there is one conversion and two schedules for it.

use claycore::{GridSnapshot, Item, VoxelField};
use clayspace_model::{LayerKey, ModelError};

/// What the grid was when it was read, to tell whether it still is.
///
/// The change count moves with every cell write, undo and redo included, and
/// the level pair covers the one change that writes no cell: activating a
/// different level changes what the conversion reads without touching it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GridToken {
    changes: u64,
    level: usize,
    levels: usize,
}

impl GridToken {
    pub(crate) fn of(grid: &VoxelField) -> Result<Self, ModelError> {
        Ok(Self {
            changes: grid.change_count().map_err(ModelError::engine)?,
            level: grid.active_level().map_err(ModelError::engine)?,
            levels: grid.level_count().map_err(ModelError::engine)?,
        })
    }
}

/// A grid read out of the document, ready to be converted off the interface
/// thread.
#[derive(Debug)]
pub struct GridToField {
    pub(crate) source: LayerKey,
    pub(crate) token: GridToken,
    pub(crate) in_place: bool,
    pub(crate) blur: i32,
    pub(crate) snapshot: GridSnapshot,
}

impl GridToField {
    /// How many occupied cells the crossing converts.
    pub fn cell_count(&self) -> usize {
        self.snapshot.cell_count()
    }

    /// The conversion itself. Needs no document, so it runs wherever the
    /// caller puts it.
    pub fn convert(self) -> Result<FieldFromGrid, ModelError> {
        let grid = self.snapshot.to_grid().map_err(ModelError::engine)?;
        let item = Item::volume_from_voxels(&grid, self.blur, 0).map_err(ModelError::engine)?;
        Ok(FieldFromGrid {
            source: self.source,
            token: self.token,
            in_place: self.in_place,
            item,
        })
    }
}

/// A converted field, waiting to be placed beside (or instead of) its grid.
#[derive(Debug)]
pub struct FieldFromGrid {
    pub(crate) source: LayerKey,
    pub(crate) token: GridToken,
    pub(crate) in_place: bool,
    pub(crate) item: Item,
}

impl FieldFromGrid {
    /// Whether placing it replaces the grid it came from.
    pub fn in_place(&self) -> bool {
        self.in_place
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_send<T: Send>() {}

    /// The whole point of the split: both halves cross threads.
    #[test]
    fn both_halves_can_leave_the_interface_thread() {
        is_send::<GridToField>();
        is_send::<FieldFromGrid>();
    }
}
