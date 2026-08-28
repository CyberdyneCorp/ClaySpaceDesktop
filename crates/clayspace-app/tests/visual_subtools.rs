//! The manipulator standing on a whole subtool.
//!
//! `GizmoTarget::Layer` has been implemented and tested at the engine boundary
//! since the objects work landed, and no control in the interface reached it: a
//! whole form could be moved from a test and not from the application. This
//! captures what a sculptor now sees when the manipulator is put on a subtool
//! — a widget on that subtool's middle rather than on a shape inside it, with
//! the second form outlined so the picture says which of the two it is on.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_subtools
//! open target/visual
//! ```

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Combine, CombineSettings, GizmoMode, GizmoTarget, LayerKey, ObjectModel, Representation,
    SceneModel, Shape,
};
use clayspace_view::{GizmoView, LatticeView};
use support::Harness;

/// How far along X the second subtool stands.
const APART: f32 = 2.2;

/// Two subtools: the starting form, and a sphere of its own standing beside it.
///
/// The second one is moved by its *layer* transform, which is what a
/// whole-subtool manipulator addresses — so the widget lands on the middle of
/// that form rather than at the world origin.
fn two_subtools() -> Option<(ClayDocument, LayerKey)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let second = document.add_layer("Segunda", Representation::Sdf).ok()?;
    document
        .place_object(
            Shape::Sphere,
            &[0.7],
            [0.0; 3],
            CombineSettings {
                op: Combine::Add,
                ..CombineSettings::default()
            },
        )
        .ok()?;
    document
        .set_layer_transform(second, [APART, 0.0, 0.0], 1.0)
        .ok()?;
    Some((document, second))
}

/// The brick surface, which is where two composed SDF subtools show up.
fn meshed(gpu: &clayspace_view::Gpu, document: &mut ClayDocument) -> SurfaceGeometry {
    let mut geometry = SurfaceGeometry::new(gpu);
    geometry.rebuild(gpu, document).expect("mesh the forms");
    geometry
}

/// The box a subtool occupies, as the engine reports it.
fn bounds_of(document: &ClayDocument, key: LayerKey) -> Option<([f32; 3], [f32; 3])> {
    document.layer_bounds(key)
}

/// How long the manipulator's arms are on a whole subtool.
///
/// `App::gizmo_reach`'s rule, and the numbers are its constants: a widget on a
/// form's *middle* is inside that form and depth-tested against it, so arms
/// that do not reach past the surface are drawn and never seen. The first
/// version of this test used the fixed object reach and captured three modes
/// that were pixel-for-pixel identical, all of them invisible.
fn reach_for(document: &ClayDocument, key: LayerKey) -> f32 {
    let Some((min, max)) = bounds_of(document, key) else {
        return 0.45;
    };
    let span = (0..3)
        .map(|axis| max[axis] - min[axis])
        .fold(0.0f32, f32::max);
    (span * 0.7).max(0.45)
}

#[test]
fn the_manipulator_sits_on_a_whole_subtools_middle() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((mut document, second)) = two_subtools() else {
        return;
    };
    let gpu = harness.gpu.clone();
    let geometry = meshed(&gpu, &mut document);
    let camera = support::framed(&document);

    let bare = {
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
        harness.capture(geometry.mesh(), &camera, false, "subtools-plain")
    };

    // Where the application puts it: the target's own transform, asked of the
    // model rather than worked out here, so the capture shows the pivot a drag
    // would actually resolve from.
    let target = GizmoTarget::Layer(second);
    let at = document
        .target_transform(target)
        .expect("a layer carries a transform");
    let reach = reach_for(&document, second);
    assert_eq!(
        at.position,
        [APART, 0.0, 0.0],
        "the manipulator would not sit on the second subtool at all"
    );

    harness.renderer.set_lattice(
        &harness.gpu,
        LatticeView {
            points: &[],
            edges: &[],
            selected: &[],
            gizmo: Some(GizmoView {
                pivot: at.position,
                mode: GizmoMode::Move,
                reach,
                hovered: None,
                view_axis: [0.0, 0.0, 1.0],
                // One scale factor on a layer as on an object: the engine's
                // transforms take one, so there is one handle for it.
                per_axis_scale: false,
            }),
            outline: bounds_of(&document, second),
            subtool_outline: None,
            handle: 0.0,
        },
    );
    let shown = harness.capture(geometry.mesh(), &camera, false, "subtools-manipulator");

    assert!(
        bare.mean_difference(&shown) > 0.001,
        "putting the manipulator on a whole subtool changed nothing on screen"
    );
}

/// Each of the three modes draws a different widget, so a sculptor can tell
/// which one is in force without reading the panel.
#[test]
fn each_manipulator_mode_draws_a_different_widget_on_a_subtool() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((mut document, second)) = two_subtools() else {
        return;
    };
    let gpu = harness.gpu.clone();
    let geometry = meshed(&gpu, &mut document);
    let camera = support::framed(&document);
    let at = document
        .target_transform(GizmoTarget::Layer(second))
        .expect("a transform");
    let reach = reach_for(&document, second);

    let shot = |harness: &mut Harness, mode: GizmoMode, name: &str| {
        harness.renderer.set_lattice(
            &harness.gpu,
            LatticeView {
                points: &[],
                edges: &[],
                selected: &[],
                gizmo: Some(GizmoView {
                    pivot: at.position,
                    mode,
                    reach,
                    hovered: None,
                    view_axis: [0.0, 0.0, 1.0],
                    per_axis_scale: false,
                }),
                outline: None,
                subtool_outline: None,
                handle: 0.0,
            },
        );
        harness.capture(geometry.mesh(), &camera, false, name)
    };

    let moving = shot(&mut harness, GizmoMode::Move, "subtools-manipulator-move");
    let turning = shot(&mut harness, GizmoMode::Rotate, "subtools-manipulator-turn");
    let scaling = shot(&mut harness, GizmoMode::Scale, "subtools-manipulator-scale");

    assert!(
        moving.mean_difference(&turning) > 0.001,
        "moving and turning drew the same widget; a manipulator buried inside \
         the form it sits on looks exactly like this"
    );
    assert!(
        turning.mean_difference(&scaling) > 0.001,
        "turning and scaling drew the same widget"
    );
}
