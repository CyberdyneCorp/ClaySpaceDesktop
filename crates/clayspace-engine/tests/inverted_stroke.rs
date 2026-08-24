//! Holding the invert key, on each of the three representations.
//!
//! ZBrush and Blender both let a sculptor take clay away with the brush they
//! already have in hand rather than picking a second one. The key is held for
//! the gesture and the shelf never moves, so what the tests below check is
//! that the *same* verb, with one flag turned on, digs where it otherwise
//! builds — in a field, on a mesh, and in a grid, which are three different
//! mechanisms underneath: the combine operation is turned over, the mesh
//! preset's strength is negated, and the grid is erased instead of set.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Direction, GestureSample, SculptModel, ToolKind};

fn sphere() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

/// A short sweep across the near face of the form, taken from where it
/// actually is rather than from where a sphere would be — the second pass runs
/// on clay the first pass already moved.
fn sweep(document: &ClayDocument) -> Vec<GestureSample> {
    (0..=12)
        .filter_map(|step| {
            let t = step as f32 / 12.0;
            SculptModel::pick(document, [-0.3 + t * 0.6, 0.0, 4.0], [0.0, 0.0, -1.0]).map(|hit| {
                GestureSample {
                    position: hit,
                    pressure: 1.0,
                    time: t,
                }
            })
        })
        .collect()
}

fn brush(invert: bool) -> BrushSettings {
    BrushSettings {
        size: 0.18,
        intensity: 0.65,
        invert,
        ..BrushSettings::default()
    }
}

/// How far the surface stands from the centre, over the band the sweep ran
/// across. Reported as (nearest, furthest): building up raises the second,
/// digging in lowers the first.
///
/// Read by raycast rather than from the geometry buffer, because a field layer
/// is drawn from the brick cache and `visible_mesh_geometry` carries only the
/// layers the cache cannot hold — it answers with nothing at all for a
/// document that is pure SDF.
fn probed_band(document: &ClayDocument) -> (f32, f32) {
    (0..=24)
        .filter_map(|step| {
            let t = step as f32 / 24.0;
            SculptModel::pick(document, [-0.4 + t * 0.8, 0.0, 4.0], [0.0, 0.0, -1.0])
        })
        .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
        .fold((f32::MAX, 0.0f32), |(near, far), r| {
            (near.min(r), far.max(r))
        })
}

fn band(document: &mut ClayDocument) -> (f32, f32) {
    document
        .visible_mesh_geometry()
        .0
        .iter()
        .filter(|v| v[2] > 0.6 && v[0].abs() < 0.45)
        .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
        .fold((f32::MAX, 0.0f32), |(near, far), r| {
            (near.min(r), far.max(r))
        })
}

fn run(document: &mut ClayDocument, invert: bool) {
    for sample in sweep(document) {
        document
            .apply_stroke(ToolKind::Padrao, brush(invert), &[sample], [false; 3])
            .expect("the stroke was refused");
    }
}

#[test]
fn an_inverted_stroke_digs_into_a_field() {
    let Some(mut upright) = sphere() else {
        return;
    };
    let Some(mut inverted) = sphere() else {
        return;
    };

    run(&mut upright, false);
    run(&mut inverted, true);

    let (_, raised) = probed_band(&upright);
    let (cut, _) = probed_band(&inverted);
    assert!(
        raised > 1.01,
        "the upright sweep only reached {raised}, so there is no build-up to \
         compare an inverted one against"
    );
    assert!(
        cut < 0.99,
        "the inverted sweep left the nearest surface at {cut}: holding the \
         key added clay where the sculptor asked to take it away"
    );
}

#[test]
fn an_inverted_stroke_digs_into_a_mesh() {
    let Some(mut upright) = sphere() else {
        return;
    };
    let Some(mut inverted) = sphere() else {
        return;
    };
    for document in [&mut upright, &mut inverted] {
        document
            .convert_layer(Direction::SdfToMesh, 0.02, 0)
            .expect("into a mesh");
    }

    run(&mut upright, false);
    run(&mut inverted, true);

    let (_, raised) = band(&mut upright);
    let (cut, _) = band(&mut inverted);
    assert!(
        raised > 1.01,
        "the upright sweep only reached {raised} on a mesh, so there is no \
         build-up to compare an inverted one against"
    );
    assert!(
        cut < 0.99,
        "the inverted sweep left the nearest vertex at {cut}: the mesh preset \
         kept its sign, so the key did nothing"
    );
}

#[test]
fn an_inverted_stroke_erases_a_grid() {
    let Some(policy) = BackendPolicy::discover(None).ok() else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy) else {
        return;
    };
    document.add_voxel_layer("Voxels", 0.05).expect("a grid");

    let ridge: Vec<GestureSample> = (0..9)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [(t - 0.5) * 0.6, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    let packing = BrushSettings {
        size: 0.25,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    for sample in &ridge {
        document
            .apply_stroke(ToolKind::Padrao, packing, &[*sample], [false; 3])
            .expect("the deposit was refused");
    }
    let deposited = document.visible_mesh_geometry().3.len();
    assert!(
        deposited > 0,
        "nothing was deposited, so there is nothing to erase"
    );

    // The same verb, the same path, one flag turned on.
    for sample in &ridge {
        document
            .apply_stroke(
                ToolKind::Padrao,
                BrushSettings {
                    invert: true,
                    ..packing
                },
                &[*sample],
                [false; 3],
            )
            .expect("the erase was refused");
    }
    let left = document.visible_mesh_geometry().3.len();
    assert!(
        left * 4 < deposited,
        "the ridge went from {deposited} indices to {left}: holding the key \
         over a grid kept setting cells instead of clearing them"
    );
}
