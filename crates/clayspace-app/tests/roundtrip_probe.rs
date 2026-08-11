use clayspace_engine::claycore::{Document, Item, Op, VolumeParams};

/// Radius of the surface along a direction, from the document itself.
fn radius(doc: &Document, dir: [f32; 3]) -> Option<f32> {
    let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let u = dir.map(|c| c / n);
    doc.raycast(u.map(|c| c * 4.0), u.map(|c| -c))
        .ok()
        .flatten()
        .map(|h| (h.position[0].powi(2) + h.position[1].powi(2) + h.position[2].powi(2)).sqrt())
}

/// Deviation introduced by baking a region and replacing it with itself.
fn roundtrip(cell: f32) -> (f64, f64, usize) {
    let mut doc = Document::new().unwrap();
    let layer = doc.add_sdf_layer("L").unwrap();
    doc.add_item(layer, &Item::sphere(1.0).unwrap()).unwrap();

    // A grid of directions through the region about to be replaced.
    let dirs: Vec<[f32; 3]> = (-12..=12)
        .flat_map(|i| {
            (-6..=6).map(move |j| {
                let (x, y) = (i as f32 * 0.04, j as f32 * 0.04);
                [x, y, (1.0f32 - x * x - y * y).max(0.1).sqrt()]
            })
        })
        .collect();
    let before: Vec<Option<f32>> = dirs.iter().map(|d| radius(&doc, *d)).collect();

    let volume = doc
        .volume_from_region(
            VolumeParams {
                cell_size: Some(cell),
                ..Default::default()
            },
            [-0.7, -0.4, 0.4],
            [0.7, 0.4, 1.3],
        )
        .unwrap();
    let mut volume = volume;
    volume.set_op(Op::Replace).unwrap();
    doc.add_item(layer, &volume).unwrap();

    let mut worst = 0.0f64;
    let mut total = 0.0f64;
    let mut counted = 0usize;
    for (d, was) in dirs.iter().zip(&before) {
        let (Some(was), Some(now)) = (was, radius(&doc, *d)) else {
            continue;
        };
        let gap = (*was as f64 - now as f64).abs();
        worst = worst.max(gap);
        total += gap;
        counted += 1;
    }
    (worst, total / counted.max(1) as f64, counted)
}

#[test]
fn baking_a_region_and_replacing_it_changes_the_surface() {
    println!(
        "\n  {:>10} {:>12} {:>12}  {:>8}",
        "cell", "worst dev", "mean dev", "probes"
    );
    for cell in [0.04f32, 0.02, 0.01, 0.005] {
        let (worst, mean, n) = roundtrip(cell);
        println!("  {cell:>10} {worst:>12.5} {mean:>12.5}  {n:>8}");
    }
    println!();
}
