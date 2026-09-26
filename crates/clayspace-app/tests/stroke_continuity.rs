//! A stroke is one ridge however sparsely it was sampled.
//!
//! An agent draws with a handful of `stroke/continue` samples far apart, and
//! every one of them is enough travel to send a segment. Each segment used to
//! start at its own new sample, so the engine laid a stamp there and nothing
//! between: at size 0.12 the stroke came out as a chain of blobs (#178). This
//! drives the real ViewModel over the real document, the way the agent door
//! does, and compares the ridge a sparse stroke raises with the one a dense
//! stroke along the same arc raises.

use clayspace_app::SharedDocument;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::SculptModel;
use clayspace_vm::{Command, SculptViewModel};

/// Distance from the centre of the starting sphere to its surface, looking in
/// from `angle` radians off +z in the xz plane.
fn radius(document: &SharedDocument, angle: f32) -> f32 {
    let direction = [angle.sin(), 0.0, angle.cos()];
    let hit = SculptModel::pick(document, direction.map(|c| c * 4.0), direction.map(|c| -c))
        .expect("the sphere is under the ray");
    hit.iter().map(|c| c * c).sum::<f32>().sqrt()
}

/// The lift along the arc a stroke of `samples` samples was drawn over, from
/// -0.3 to 0.3 rad, and the rest radius it is measured from.
fn ridge(samples: usize) -> Option<Vec<f32>> {
    let policy = BackendPolicy::discover(None).ok()?;
    let document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let document = SharedDocument::new(document);
    let mut sculpt = SculptViewModel::new(Box::new(document.clone()));
    sculpt
        .dispatch(Command::SetBrushSize(0.12))
        .expect("the size");
    let rest = radius(&document, 0.0);

    // An arc over the pole, sampled evenly: on the surface at every sample,
    // as a pick would put it.
    let arc: Vec<[f32; 3]> = (0..samples)
        .map(|i| {
            let angle = -0.4 + 0.8 * i as f32 / (samples - 1) as f32;
            [rest * angle.sin(), 0.0, rest * angle.cos()]
        })
        .collect();
    let (first, others) = arc.split_first()?;
    sculpt
        .dispatch(Command::BeginStroke {
            position: *first,
            pressure: 1.0,
            modifiers: Default::default(),
        })
        .expect("begin");
    for position in others {
        sculpt
            .dispatch(Command::ContinueStroke {
                position: *position,
                pressure: 1.0,
            })
            .expect("continue");
    }
    sculpt.dispatch(Command::EndStroke).expect("end");

    Some(
        (0..=24)
            .map(|i| radius(&document, -0.3 + 0.025 * i as f32) - rest)
            .collect(),
    )
}

#[test]
fn a_sparse_stroke_has_no_gaps() {
    // Samples 0.1 rad apart — close enough that the chord between two of them
    // stays within 0.0013 of the sphere, so what is measured is the stamping
    // and not the straight line an agent drew between its own samples.
    let Some(sparse) = ridge(9) else {
        eprintln!("skipping: no engine backend");
        return;
    };
    let dense = ridge(161).expect("the same backend");

    let low = |ridge: &[f32]| ridge.iter().copied().fold(f32::MAX, f32::min);
    let (sparse_low, dense_low) = (low(&sparse), low(&dense));
    assert!(
        dense_low > 0.01,
        "the dense stroke is not a ridge to compare against: {dense:?}"
    );
    // The stated tolerance: nowhere along the stroke does the sparse ridge
    // sink below 80% of the dense one's lowest point. A chain of beads sinks
    // to the untouched surface between them.
    assert!(
        sparse_low > dense_low * 0.8,
        "the sparse stroke sank to {sparse_low} between its samples, where the \
         dense one never went below {dense_low}: {sparse:?}"
    );
}
