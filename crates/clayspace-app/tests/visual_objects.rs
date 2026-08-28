//! Whether a placed object reaches the screen, and whether it can be aimed.
//!
//! A subtracting object is *inside* the form: what a sculptor sees of a
//! cylinder bored through a head is the hole, and the cylinder itself is
//! behind the surface where nothing shows it. So the viewport has to say where
//! it is, or aiming one means dragging a manipulator and inferring the shape
//! from the cavity that appears.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_objects
//! open target/visual
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{Combine, CombineSettings, GizmoMode, GizmoTarget, ObjectModel, Shape};
use clayspace_view::{Camera, GizmoView, LatticeView};
use support::Harness;

/// A worked form with a cylinder bored through it.
fn bored() -> Option<(ClayDocument, clayspace_model::ObjectId)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let id = document
        .place_object(
            Shape::Cylinder,
            &[0.25, 1.6],
            [0.0, 0.4, 0.0],
            CombineSettings {
                op: Combine::Subtract,
                ..CombineSettings::default()
            },
        )
        .ok()?;
    Some((document, id))
}

/// The brick surface, which is where an SDF boolean shows up.
///
/// Not `visible_mesh_geometry`, which answers for the mesh and voxel layers: a
/// field's surface is assembled from the brick cache, and asking the other
/// question of an SDF document gets a blank frame — which is what the first
/// version of this test captured.
fn meshed(gpu: &clayspace_view::Gpu, document: &mut ClayDocument) -> SurfaceGeometry {
    let mut geometry = SurfaceGeometry::new(gpu);
    geometry.rebuild(gpu, document).expect("mesh the form");
    geometry
}

/// The box the interface outlines for an object, as `App::selected_outline`
/// computes it: the largest measurement it carries, scaled.
fn outline_of(object: &clayspace_model::SceneObject) -> ([f32; 3], [f32; 3]) {
    let reach = object
        .parameters
        .iter()
        .copied()
        .fold(0.0f32, f32::max)
        .max(1e-3)
        * object.scale;
    (
        std::array::from_fn(|i| object.position[i] - reach),
        std::array::from_fn(|i| object.position[i] + reach),
    )
}

#[test]
fn a_selected_object_is_outlined_where_it_stands() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((mut document, id)) = bored() else {
        return;
    };
    let camera = Camera::default();
    let gpu = harness.gpu.clone();
    let geometry = meshed(&gpu, &mut document);

    let listed = document.objects();
    let object = listed
        .iter()
        .find(|object| object.id == id)
        .expect("the cylinder");

    // Without the outline: the cavity is visible and the cylinder is not.
    harness.renderer.set_lattice(
        &harness.gpu,
        LatticeView {
            points: &[],
            edges: &[],
            selected: &[],
            gizmo: None,
            outline: None,
            subtool_outline: None,
            handle: 0.0,
        },
    );
    let bare = harness.capture(geometry.mesh(), &camera, false, "objects-unselected");

    harness.renderer.set_lattice(
        &harness.gpu,
        LatticeView {
            points: &[],
            edges: &[],
            selected: &[],
            gizmo: Some(GizmoView {
                pivot: object.position,
                mode: GizmoMode::Move,
                reach: 0.45,
                hovered: None,
                view_axis: [0.0, 0.0, 1.0],
                per_axis_scale: false,
            }),
            outline: Some(outline_of(object)),
            subtool_outline: None,
            handle: 0.0,
        },
    );
    let shown = harness.capture(geometry.mesh(), &camera, false, "objects-selected");

    assert!(
        bare.mean_difference(&shown) > 0.001,
        "selecting the object changed nothing on screen"
    );
}

/// Scale mode draws one handle on an object and four on a cage, because the
/// engine scales an object by one factor and a cage scales its own points.
#[test]
fn scale_mode_draws_fewer_handles_on_an_object_than_on_a_cage() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((mut document, _)) = bored() else {
        return;
    };
    let camera = Camera::default();
    let gpu = harness.gpu.clone();
    let geometry = meshed(&gpu, &mut document);

    let shot = |harness: &mut Harness, per_axis_scale: bool, name: &str| {
        harness.renderer.set_lattice(
            &harness.gpu,
            LatticeView {
                points: &[],
                edges: &[],
                selected: &[],
                gizmo: Some(GizmoView {
                    pivot: [0.0, 0.4, 0.0],
                    mode: GizmoMode::Scale,
                    reach: 0.6,
                    hovered: None,
                    view_axis: [0.0, 0.0, 1.0],
                    per_axis_scale,
                }),
                outline: None,
                subtool_outline: None,
                handle: 0.0,
            },
        );
        harness.capture(geometry.mesh(), &camera, false, name)
    };

    let cage_like = shot(&mut harness, true, "objects-scale-per-axis");
    let object_like = shot(&mut harness, false, "objects-scale-uniform");
    assert!(
        cage_like.mean_difference(&object_like) > 0.0005,
        "the two scale manipulators drew the same picture, so the axis boxes \
         are still there on an object"
    );
}

#[test]
fn the_manipulator_moves_the_object_it_is_on() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((mut document, id)) = bored() else {
        return;
    };
    let camera = Camera::default();
    let target = GizmoTarget::Object(id);

    let gpu = harness.gpu.clone();
    let before = {
        let geometry = meshed(&gpu, &mut document);
        harness.renderer.set_lattice(
            &harness.gpu,
            LatticeView {
                points: &[],
                edges: &[],
                selected: &[],
                gizmo: None,
                outline: None,
                subtool_outline: None,
                handle: 0.0,
            },
        );
        harness.capture(geometry.mesh(), &camera, false, "objects-bore-before")
    };

    let at = document.target_transform(target).expect("a transform");
    // Sideways rather than down. The bore is a vertical cylinder seen
    // head-on, so moving it along its own axis barely changes the silhouette —
    // which the first version of this test could not tell from the drag doing
    // nothing at all.
    let gesture = clayspace_model::GizmoDrag {
        mode: GizmoMode::Move,
        handle: clayspace_model::GizmoHandle::Axis(0),
        pivot: at.position,
        anchor: at.position,
        view_axis: [0.0, 0.0, 1.0],
    };
    document.begin_target_drag(target);
    document
        .set_target_transform(target, gesture.resolve(at, [0.65, 0.4, 0.0], false))
        .expect("drag");
    document.end_target_drag();

    let after = {
        let geometry = meshed(&gpu, &mut document);
        harness.capture(geometry.mesh(), &camera, false, "objects-bore-after")
    };

    assert!(
        before.mean_difference(&after) > 0.002,
        "dragging the manipulator did not move the bore on screen"
    );

    // And the field agrees with the picture, on both sides: the layer is
    // mirrored in X, so an object dragged off the plane cuts twice. Checked
    // here rather than left to the eye — one notch is easy to see and its
    // reflection is easy to miss.
    let field = document
        .document()
        .eval_points(None, &[[0.65, 0.4, 0.0], [-0.65, 0.4, 0.0]])
        .expect("evaluate");
    assert!(
        field[0] > 0.0 && field[1] > 0.0,
        "the bore should be cut on both sides of the mirror, got {field:?}"
    );
}
