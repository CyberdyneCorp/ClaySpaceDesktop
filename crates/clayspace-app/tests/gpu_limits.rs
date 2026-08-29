//! What happens when a surface is more than the graphics device can hold.
//!
//! A subtool scaled up a few times is ten million vertices at the field's
//! fixed resolution. `create_buffer` past the device's ceiling is a wgpu
//! validation error, and wgpu's default handler panics the process — which
//! is how a scale gesture ended a session. The renderer refuses the
//! reservation instead, and the composition root draws coarser.

mod support;

use clayspace_view::{GpuMesh, Vertex};
use support::Harness;

#[test]
fn a_reservation_past_the_device_ceiling_is_refused_not_fatal() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let ceiling = harness.gpu.max_buffer_size();
    eprintln!("the device's buffer ceiling is {ceiling} bytes");
    // One vertex more than the ceiling holds. A device whose ceiling is past
    // what a count can express has nothing to refuse, and the test says so
    // rather than overflowing into a wrong answer.
    let Ok(too_many) = usize::try_from(ceiling / Vertex::STRIDE as u64 + 1) else {
        eprintln!("skipping: the ceiling exceeds what a vertex count can address");
        return;
    };
    assert!(
        ceiling <= clayspace_view::Gpu::BUFFER_CAP,
        "the ceiling is not capped, so an adapter reporting u64::MAX would refuse nothing"
    );

    let mut mesh = GpuMesh::new(&harness.gpu);
    assert!(
        !GpuMesh::fits(&harness.gpu, too_many, 3),
        "{too_many} vertices should not fit a {ceiling}-byte ceiling"
    );
    // The refusal is the whole point: reaching here means no panic.
    assert!(
        !mesh.reserve(&harness.gpu, too_many, 3),
        "a reservation past the ceiling was accepted"
    );
    assert!(
        mesh.is_empty(),
        "the refused reservation left something behind"
    );

    // And one that fits is taken as before.
    assert!(mesh.reserve(&harness.gpu, 1024, 1024));
}
