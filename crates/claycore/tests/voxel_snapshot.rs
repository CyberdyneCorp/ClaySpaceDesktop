//! A grid read out as plain data and converted elsewhere.
//!
//! The snapshot is what lets a grid-to-field crossing leave the interface
//! thread: the document stays behind, the cells travel. That is only worth
//! anything if what comes back is the field the in-document conversion would
//! have made, so that is what these compare, sample for sample.

use claycore::{Document, Item, VoxelField};

const CELL: f32 = 0.05;

/// A two-colour block with a notch cut out, so the field has corners, a
/// concavity and a colour boundary to disagree about.
fn sculpt(grid: &mut VoxelField) {
    let clay = grid.palette_add([0.8, 0.5, 0.3]).expect("a colour");
    let stone = grid.palette_add([0.3, 0.3, 0.35]).expect("a colour");
    grid.fill_box([-8, -6, -5], [8, 6, 5], clay)
        .expect("the block");
    grid.fill_box([-3, -3, 3], [3, 3, 5], 0).expect("the notch");
    grid.fill_box([0, -6, -5], [8, 6, 5], stone)
        .expect("the second colour");
}

/// Points through the block, its surface and the air around it.
fn probes() -> Vec<[f32; 3]> {
    let mut points = Vec::new();
    for i in -6..=6 {
        for j in -4..=4 {
            points.push([i as f32 * 0.09, j as f32 * 0.08, 0.27]);
            points.push([i as f32 * 0.09, 0.05, j as f32 * 0.09]);
        }
    }
    points
}

/// The field the document's own conversion makes of `build`'s grid.
fn converted_in_place(blur: i32, build: impl Fn(&mut VoxelField)) -> Vec<f32> {
    let mut doc = Document::new().expect("a document");
    {
        let (_, mut grid) = doc.add_voxel_layer("grid", CELL).expect("a grid");
        build(&mut grid);
    }
    doc.voxel_layer_to_sdf_layer("grid", "field", blur)
        .expect("convert in the document");
    doc.eval_points(None, &probes()).expect("evaluate")
}

/// The same grid, snapshotted, rebuilt and converted as a free-standing item.
fn converted_from_snapshot(blur: i32, build: impl Fn(&mut VoxelField)) -> Vec<f32> {
    let mut doc = Document::new().expect("a document");
    let snapshot = {
        let (_, mut grid) = doc.add_voxel_layer("grid", CELL).expect("a grid");
        build(&mut grid);
        grid.snapshot().expect("a snapshot")
    };
    // On another thread, as the application runs it: the snapshot goes over,
    // an item comes back.
    let item = std::thread::spawn(move || {
        let grid = snapshot.to_grid().expect("rebuild the grid");
        Item::volume_from_voxels(&grid, blur, 0).expect("convert the copy")
    })
    .join()
    .expect("the worker");
    let layer = doc.add_sdf_layer("field").expect("a field layer");
    doc.add_item(layer, &item).expect("place it");
    doc.eval_points(None, &probes()).expect("evaluate")
}

fn assert_same_field(direct: &[f32], detached: &[f32]) {
    assert_eq!(direct.len(), detached.len());
    for (i, (a, b)) in direct.iter().zip(detached).enumerate() {
        assert!(
            (a - b).abs() <= 1e-5,
            "probe {i}: the document converted to {a} and the snapshot to {b}"
        );
    }
}

#[test]
fn a_snapshot_converts_to_the_field_the_document_would_make() {
    for blur in [0, 1] {
        assert_same_field(
            &converted_in_place(blur, sculpt),
            &converted_from_snapshot(blur, sculpt),
        );
    }
}

/// A refined grid converts at its active level, and so does its snapshot.
#[test]
fn a_snapshot_reads_the_active_level() {
    let refined = |grid: &mut VoxelField| {
        sculpt(grid);
        grid.add_level().expect("a finer level");
        let index = grid.palette_add([0.1, 0.6, 0.2]).expect("a colour");
        grid.fill_box([-4, -4, 10], [4, 4, 14], index)
            .expect("detail at the finer level");
    };
    assert_same_field(
        &converted_in_place(1, refined),
        &converted_from_snapshot(1, refined),
    );
}

#[test]
fn a_snapshot_holds_every_occupied_cell_and_nothing_else() {
    let mut doc = Document::new().expect("a document");
    let (_, mut grid) = doc.add_voxel_layer("grid", CELL).expect("a grid");
    sculpt(&mut grid);
    let snapshot = grid.snapshot().expect("a snapshot");
    assert_eq!(snapshot.cell_count(), grid.occupied_count().expect("count"));
    let rebuilt = snapshot.to_grid().expect("rebuild");
    assert_eq!(
        rebuilt.occupied_count().expect("count"),
        snapshot.cell_count()
    );
    assert_eq!(
        rebuilt.bounds().expect("bounds"),
        grid.bounds().expect("bounds")
    );
    for cell in [[-8, -6, -5], [-1, 0, 0], [5, 2, 1], [0, 0, 4]] {
        let colour = |g: &VoxelField| {
            g.get(cell)
                .expect("read")
                .map(|index| g.palette_color(index).expect("colour"))
        };
        assert_eq!(colour(&rebuilt), colour(&grid), "cell {cell:?}");
    }
}

#[test]
fn an_empty_grid_snapshots_to_nothing() {
    let mut doc = Document::new().expect("a document");
    let (_, grid) = doc.add_voxel_layer("grid", CELL).expect("a grid");
    assert_eq!(grid.snapshot().expect("a snapshot").cell_count(), 0);
}
