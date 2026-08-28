//! What a boolean between two subtools leaves on screen.
//!
//! The engine composes layers by hard union, so before this the three
//! operations had no picture at all between two *subtools* — only between two
//! items inside one layer. These are the three, captured from the same pair of
//! forms and the same camera, so what changed between them is the operation.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_booleans
//! open target/visual
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BooleanOp, BooleanSettings, Combine, CombineSettings, LayerKey, ObjectModel, SceneModel, Shape,
};
use support::Harness;

/// How far along X the cutting form stands: overlapping the sphere in part, so
/// each of the three operations leaves something different behind.
const OFFSET: f32 = 0.45;

/// The resolution the captures bake at. The same order as the rest of the
/// application's crossings, and fine enough that the join is a join rather
/// than a staircase.
const CELL: f32 = 0.015;

fn adding() -> CombineSettings {
    CombineSettings {
        op: Combine::Add,
        ..CombineSettings::default()
    }
}

/// A sphere and a cylinder standing across it, each its own subtool.
fn a_pair() -> Option<(ClayDocument, LayerKey, LayerKey)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy).ok()?;
    let sphere = document
        .insert_shape_subtool(Shape::Sphere, &[0.6], [0.0; 3], adding())
        .ok()?
        .layer;
    let cylinder = document
        .insert_shape_subtool(Shape::Cylinder, &[0.28, 0.9], [OFFSET, 0.0, 0.0], adding())
        .ok()?
        .layer;
    Some((document, sphere, cylinder))
}

/// The brick surface, which is where composed SDF subtools show up.
fn meshed(gpu: &clayspace_view::Gpu, document: &mut ClayDocument) -> SurfaceGeometry {
    let mut geometry = SurfaceGeometry::new(gpu);
    geometry.rebuild(gpu, document).expect("mesh the forms");
    geometry
}

/// Runs one operation over a fresh pair and captures what it leaves.
///
/// A fresh document each time rather than one undone between captures: what is
/// being looked at is the result of the operation, and an undo that left
/// anything behind would show up as a difference between the pictures and be
/// read as the operations differing.
fn capture(harness: &mut Harness, op: BooleanOp, name: &str) -> Option<clayspace_view::Image> {
    let (mut document, base, tool) = a_pair()?;
    document
        .run_boolean(BooleanSettings {
            base: Some(base),
            tool: Some(tool),
            op,
            cell_size: CELL,
            consume: false,
        })
        .expect("the boolean runs");

    let gpu = harness.gpu.clone();
    let geometry = meshed(&gpu, &mut document);
    // Framed on the *pair's* extent rather than the result's, so all three
    // pictures are taken from the same place and can be held against each
    // other.
    let mut camera = clayspace_view::Camera::default();
    camera.frame_bounds([-0.75, -0.95, -0.75].into(), [0.95, 0.95, 0.75].into());
    Some(harness.capture(geometry.mesh(), &camera, false, name))
}

/// Union, subtraction and intersection over one pair: three operations, three
/// forms. Held against each other because a picture of the operation running
/// and doing nothing looks exactly like a picture of it working.
#[test]
fn the_three_booleans_each_leave_a_different_form() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(united) = capture(&mut harness, BooleanOp::Union, "booleans-union") else {
        return;
    };
    let cut = capture(&mut harness, BooleanOp::Subtract, "booleans-subtraction")
        .expect("a second capture on a harness that already built one");
    let shared = capture(&mut harness, BooleanOp::Intersect, "booleans-intersection")
        .expect("a third capture");

    assert!(
        united.mean_difference(&cut) > 0.001,
        "a union and a subtraction of the same pair drew the same picture"
    );
    assert!(
        cut.mean_difference(&shared) > 0.001,
        "a subtraction and an intersection of the same pair drew the same picture"
    );
    assert!(
        united.mean_difference(&shared) > 0.001,
        "a union and an intersection of the same pair drew the same picture"
    );
}

/// The operands are kept and hidden, so what the viewport shows after a
/// boolean is the result alone — not the result with its operands still
/// standing in it, which is what a boolean that forgot to hide them looks
/// like.
#[test]
fn what_the_viewport_shows_is_the_result_alone() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some((mut document, base, tool)) = a_pair() else {
        return;
    };
    let gpu = harness.gpu.clone();
    let mut camera = clayspace_view::Camera::default();
    camera.frame_bounds([-0.75, -0.95, -0.75].into(), [0.95, 0.95, 0.75].into());

    let before = {
        let geometry = meshed(&gpu, &mut document);
        harness.capture(geometry.mesh(), &camera, false, "booleans-operands")
    };
    document
        .run_boolean(BooleanSettings {
            base: Some(base),
            tool: Some(tool),
            op: BooleanOp::Subtract,
            cell_size: CELL,
            consume: false,
        })
        .expect("the boolean runs");
    let after = {
        let geometry = meshed(&gpu, &mut document);
        harness.capture(geometry.mesh(), &camera, false, "booleans-result")
    };

    assert!(
        before.mean_difference(&after) > 0.001,
        "the scene looks the same after the cut, so the operands are still \
         being drawn over the result"
    );
    assert!(
        document
            .scene()
            .layer(base)
            .is_some_and(|layer| !layer.visible),
        "the base operand is not hidden, which is why the picture would be wrong"
    );
}
