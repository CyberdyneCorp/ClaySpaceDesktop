//! First SDF layer drag: the hand moves a retained surface, then the field settles.

mod support;

use std::time::Instant;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_model::{GizmoTarget, ObjectModel, SceneModel, Transform};
use clayspace_view::ShadingMode;
use support::Harness;

#[test]
fn first_layer_drag_previews_and_commits_the_same_form() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = Scene::Reference.build(policy) else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, &mut document).unwrap();
    let camera = support::framed(&document);
    let started = Instant::now();
    let before = harness.capture(geometry.mesh(), &camera, false, "gizmo-before");
    let baseline_ms = started.elapsed().as_secs_f64() * 1000.0;
    let target = GizmoTarget::Layer(document.scene().active_layer().unwrap().key);
    let initial = document.target_transform(target).unwrap();
    let moved = Transform {
        position: [0.2, 0.0, 0.0],
        ..initial
    };

    let started = Instant::now();
    harness.renderer.set_surface_preview(Some((initial, moved)));
    let preview_ms = started.elapsed().as_secs_f64() * 1000.0;
    eprintln!("first gizmo preview: {preview_ms:.1} ms");
    let started = Instant::now();
    let preview = harness.capture(geometry.mesh(), &camera, false, "gizmo-preview");
    let render_ms = started.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "offscreen frame with readback: baseline {baseline_ms:.1} ms, preview {render_ms:.1} ms"
    );
    assert!(preview.mean_difference(&before) > 0.005);
    harness.renderer.set_surface_preview(Some((initial, moved)));
    let repeated = harness.capture(geometry.mesh(), &camera, false, "gizmo-repeat");
    assert!(preview.mean_difference(&repeated) < 0.001);

    harness.renderer.set_shading(ShadingMode::Studio);
    let studio_preview = harness.capture(geometry.mesh(), &camera, false, "gizmo-studio-preview");
    harness.renderer.set_shading(ShadingMode::MatCap);
    assert!(studio_preview.mean_difference(&before) > 0.005);

    harness.renderer.set_surface_preview(None);
    document.begin_target_drag(target);
    document.set_target_transform(target, moved).unwrap();
    geometry.settle(&harness.gpu, &mut document).unwrap();
    document.end_target_drag();
    let final_frame = harness.capture(geometry.mesh(), &camera, false, "gizmo-committed");
    let difference = preview.mean_difference(&final_frame);
    eprintln!("preview to committed mean pixel difference: {difference:.4}");
    assert!(
        difference < 0.04,
        "preview diverged from committed surface: {difference}"
    );
}
