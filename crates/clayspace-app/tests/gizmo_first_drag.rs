//! First subtool drag: an SDF layer's hand moves a retained surface, then the
//! field settles; a carried subtool moves its own triangles and nothing else.

mod support;

use std::time::Instant;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_model::{GizmoTarget, ObjectModel, Representation, SceneModel, Transform, FRAME};
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

/// The first drag frame of a whole mesh subtool fits one frame (#196, D14).
///
/// The mesh reference scene keeps the field it was crossed from under the
/// mesh. A layer transform moves no brick, but placing the mesh used to refill
/// its box, so the first frame of the drag — and every frame after it —
/// re-meshed the overlapping field: 45–54 ms of surface sync per frame on a
/// workstation. The frame's work here is what the application does for one
/// `DragGizmo` on a carried layer: the document edit, the brick surface sync
/// and the carried buffer rebuild and upload. The offscreen readback is timed
/// apart, because a window presents rather than reading back.
#[test]
fn first_mesh_subtool_drag_frame_fits_the_frame_budget() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = Scene::MeshReference.build(policy) else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, &mut document).unwrap();
    let upload_carried = |harness: &mut Harness, document: &mut clayspace_engine::ClayDocument| {
        let (vertices, indices, spans) = support::viewport_layers(document);
        harness
            .renderer
            .set_mesh_layers(&harness.gpu, &vertices, &indices, &spans);
    };
    upload_carried(&mut harness, &mut document);
    let camera = support::framed(&document);
    let before = harness.capture(geometry.mesh(), &camera, true, "gizmo-mesh-before");

    let (key, representation) = document
        .scene()
        .active_layer()
        .map(|layer| (layer.key, layer.representation))
        .unwrap();
    assert_eq!(representation, Representation::Mesh);
    let target = GizmoTarget::Layer(key);
    let initial = document.target_transform(target).unwrap();
    let revision = document.mesh_revision();

    let started = Instant::now();
    document.begin_target_drag(target);
    document
        .set_target_transform(
            target,
            Transform {
                position: [0.4, 0.0, 0.0],
                ..initial
            },
        )
        .unwrap();
    let edit = started.elapsed();
    let sync = geometry.sync(&harness.gpu, &mut document).unwrap();
    let synced = started.elapsed();
    assert_ne!(
        document.mesh_revision(),
        revision,
        "the carried buffer follows"
    );
    upload_carried(&mut harness, &mut document);
    let frame = started.elapsed();
    let moved = harness.capture(geometry.mesh(), &camera, true, "gizmo-mesh-moved");
    document.end_target_drag();
    eprintln!(
        "first mesh subtool drag frame: edit {:.2} ms, surface sync {:.2} ms, \
         carried rebuild {:.2} ms, total {:.2} ms",
        edit.as_secs_f64() * 1000.0,
        (synced - edit).as_secs_f64() * 1000.0,
        (frame - synced).as_secs_f64() * 1000.0,
        frame.as_secs_f64() * 1000.0,
    );

    assert!(
        sync.is_none(),
        "moving a mesh subtool re-meshed field bricks it cannot change: {sync:?}"
    );
    assert!(
        frame < FRAME,
        "the first mesh subtool drag frame took {frame:?}, over the {FRAME:?} budget"
    );
    assert!(
        moved.mean_difference(&before) > 0.005,
        "the mesh subtool was not drawn where it was moved"
    );
}
