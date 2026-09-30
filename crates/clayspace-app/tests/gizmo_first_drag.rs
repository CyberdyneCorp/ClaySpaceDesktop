//! First subtool drag: an SDF layer's hand moves a retained surface, then the
//! field settles; a carried subtool moves its own triangles and nothing else;
//! a placed object is drawn as its own surface until the release writes it.

mod support;

use std::time::Instant;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_engine::ClayDocument;
use clayspace_model::{
    CombineSettings, GizmoTarget, ObjectId, ObjectModel, PosedPreview, Representation, SceneModel,
    SculptModel, Shape, Transform, FRAME,
};
use clayspace_view::{offscreen::Image, Camera, ShadingMode};
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

/// The reference scene with one sphere placed on its flank, under the
/// application's default X symmetry, so the sphere has a reflected twin.
fn reference_with_a_placed_sphere(policy: BackendPolicy) -> Option<(ClayDocument, ObjectId)> {
    let mut document = Scene::Reference.build(policy).ok()?;
    let id = document
        .place_object(
            Shape::Sphere,
            &[0.3],
            [0.8, 0.35, 0.45],
            CombineSettings::default(),
        )
        .expect("place a sphere on the reference form");
    document.take_dirty_keys();
    Some((document, id))
}

/// Where the sphere is dragged: up and back, and squashed on the way, so the
/// release refills a nonuniformly scaled item.
fn drag_frames(initial: Transform) -> (Transform, Transform) {
    let halfway = Transform {
        position: [0.8, 0.55, 0.65],
        ..initial
    };
    let moved = Transform {
        position: [0.8, 0.75, 0.85],
        scale: [1.2, 0.9, 1.0],
        ..initial
    };
    (halfway, moved)
}

/// The drag as main made it: the first frame is evaluated live and its
/// surface synced, and the release writes where the hand stopped.
fn drag_live(
    harness: &Harness,
    document: &mut ClayDocument,
    target: GizmoTarget,
    frames: &[Transform],
) -> SurfaceGeometry {
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, document).unwrap();
    document.begin_target_drag(target);
    for &frame in frames {
        document.set_target_transform(target, frame).unwrap();
        geometry.sync(&harness.gpu, document).unwrap();
    }
    geometry.settle(&harness.gpu, document).unwrap();
    document.end_target_drag();
    geometry
}

/// The drag with the preview: the press meshes the object alone, the frame
/// poses and draws it, and only the release writes the document.
fn drag_previewed(
    harness: &mut Harness,
    document: &mut ClayDocument,
    id: ObjectId,
    to: Transform,
) -> PosedPreview {
    let target = GizmoTarget::Object(id);
    document.begin_target_drag(target);
    let posed = document
        .object_preview(id)
        .expect("a placed sphere has a surface of its own")
        .posed(to);
    harness
        .renderer
        .set_object_preview(&harness.gpu, Some(&posed));
    posed
}

fn release(
    harness: &mut Harness,
    document: &mut ClayDocument,
    geometry: &mut SurfaceGeometry,
    target: GizmoTarget,
    to: Transform,
) {
    harness.renderer.set_object_preview(&harness.gpu, None);
    document.set_target_transform(target, to).unwrap();
    geometry.settle(&harness.gpu, document).unwrap();
    document.end_target_drag();
}

fn sorted_triangles(geometry: &SurfaceGeometry) -> Vec<[[u32; 10]; 3]> {
    let mut triangles = geometry.stored_triangles_exact();
    triangles.sort_unstable();
    triangles
}

/// Where the posed preview lands on screen, widened by the reach of the
/// occlusion pass and the multisampled edge.
fn screen_box(posed: &PosedPreview, camera: &Camera) -> (u32, u32, u32, u32) {
    const REACH: f32 = 6.0;
    let (width, height) = (Harness::WIDTH as f32, Harness::HEIGHT as f32);
    let view_projection = camera.view_projection(width / height);
    let (mut x0, mut y0, mut x1, mut y1) = (width, height, 0.0f32, 0.0f32);
    for &position in &posed.positions {
        let ndc = view_projection.project_point3(position.into());
        let (x, y) = ((ndc.x + 1.0) * 0.5 * width, (1.0 - ndc.y) * 0.5 * height);
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    let clamp = |v: f32, limit: f32| v.clamp(0.0, limit) as u32;
    (
        clamp(x0 - REACH, width),
        clamp(y0 - REACH, height),
        clamp(x1 + REACH, width),
        clamp(y1 + REACH, height),
    )
}

/// Pixels that changed anywhere but inside `area`.
fn changed_outside(a: &Image, b: &Image, area: (u32, u32, u32, u32)) -> usize {
    let (x0, y0, x1, y1) = area;
    support::differing_pixels(a, b) - support::differing_pixels_within(a, b, x0, y0, x1, y1)
}

/// The first drag frame of a placed object in a field draws the object alone
/// and refills nothing (#196, D14).
///
/// On main the first frame wrote the move and re-meshed everything its old
/// and new bounds reached before the manipulator's adaptive deferral could
/// start. Here the press meshes the object's own primitive once, each drag
/// frame poses and uploads it, and the field is written once, on release,
/// through the same edit. Both paths are timed in the same run, on the
/// default backend, the way #300 and #312 measured theirs.
///
/// Held: the drag frame leaves the brick cache and the document alone and,
/// with the press, fits one frame; nothing but the dragged object and its
/// mirror twin changes on screen; and the released surface is the live
/// path's. The bit-level comparison of that last one is
/// `a_released_object_drag_is_bit_identical_to_the_live_path`.
#[test]
fn the_first_object_drag_frame_draws_the_object_alone() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let (Some((mut live, id)), Some((mut previewed, _))) = (
        reference_with_a_placed_sphere(policy.clone()),
        reference_with_a_placed_sphere(policy),
    ) else {
        return;
    };
    let target = GizmoTarget::Object(id);
    let initial = previewed.target_transform(target).unwrap();
    let (halfway, moved) = drag_frames(initial);

    // Main's first frame, measured on its own.
    let mut live_geometry = SurfaceGeometry::new(&harness.gpu);
    live_geometry.rebuild(&harness.gpu, &mut live).unwrap();
    let started = Instant::now();
    live.begin_target_drag(target);
    live.set_target_transform(target, halfway).unwrap();
    let live_edit = started.elapsed();
    let live_sync = live_geometry.sync(&harness.gpu, &mut live).unwrap();
    let live_frame = started.elapsed();
    live.set_target_transform(target, moved).unwrap();
    live_geometry.settle(&harness.gpu, &mut live).unwrap();
    live.end_target_drag();

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, &mut previewed).unwrap();
    let camera = support::framed(&previewed);
    let before = harness.capture(geometry.mesh(), &camera, false, "object-drag-before");
    let started = Instant::now();
    let posed = drag_previewed(&mut harness, &mut previewed, id, moved);
    let sync = geometry.sync(&harness.gpu, &mut previewed).unwrap();
    let frame = started.elapsed();
    let during = harness.capture(geometry.mesh(), &camera, false, "object-drag-preview");
    eprintln!(
        "first placed-object drag frame: main edit {:.2} ms + surface sync {:.2} ms \
         ({} keys) = {:.2} ms; preview press and frame {:.2} ms ({} triangles posed)",
        live_edit.as_secs_f64() * 1000.0,
        (live_frame - live_edit).as_secs_f64() * 1000.0,
        live_sync.map_or(0, |cost| cost.keys),
        live_frame.as_secs_f64() * 1000.0,
        frame.as_secs_f64() * 1000.0,
        posed.indices.len() / 3,
    );

    // (a) The drag frame refilled nothing, moved nothing in the document and
    // fits the frame; main's did all three.
    assert!(
        live_sync.is_some_and(|cost| cost.keys > 0),
        "the live path should re-mesh what the move reached: {live_sync:?}"
    );
    assert!(
        sync.is_none(),
        "the previewed drag frame re-meshed field bricks: {sync:?}"
    );
    assert_eq!(
        previewed.target_transform(target),
        Some(initial),
        "the document moved before the release"
    );
    assert!(
        frame < FRAME,
        "the press and first previewed frame took {frame:?}, over the {FRAME:?} budget"
    );

    // (c) Only the dragged object changes on screen: the field, the rest of
    // the form and the sphere's own old image stay exactly where they were.
    let area = screen_box(&posed, &camera);
    assert_eq!(
        changed_outside(&before, &during, area),
        0,
        "the preview moved something other than the dragged object"
    );
    assert!(
        support::differing_pixels(&before, &during) > 200,
        "the dragged object was not drawn where the hand took it"
    );

    // The release writes the move once and leaves the live path's surface.
    release(&mut harness, &mut previewed, &mut geometry, target, moved);
    assert_eq!(
        geometry.stored_triangles(),
        live_geometry.stored_triangles(),
        "the released surface is not the live path's"
    );
    let released = harness.capture(geometry.mesh(), &camera, false, "object-drag-released");
    let live_image = harness.capture(live_geometry.mesh(), &camera, false, "object-drag-live");
    assert_eq!(support::differing_pixels(&released, &live_image), 0);
    eprintln!(
        "object preview to released mean pixel difference: {:.4}",
        during.mean_difference(&released)
    );

    // And the drag is still one thing to take back.
    previewed.undo().unwrap();
    assert_eq!(previewed.target_transform(target), Some(initial));
}

/// The release lands exactly where the live path lands, bit for bit (#196,
/// D14).
///
/// On the CPU backend, which refills deterministically. The accelerated one
/// does not: two identical live drags on Metal differed in the last bits of
/// 7–77 of 349,744 triangles from run to run, all within 1/4096 of each other,
/// which is the backend and not the drag — and is why the default-backend
/// test above compares quantised triangles.
#[test]
fn a_released_object_drag_is_bit_identical_to_the_live_path() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let cpu =
        || BackendPolicy::from_available(vec![clayspace_engine::claycore::Backend::Cpu], None);
    let (Some((mut live, id)), Some((mut previewed, _))) = (
        reference_with_a_placed_sphere(cpu()),
        reference_with_a_placed_sphere(cpu()),
    ) else {
        return;
    };
    let target = GizmoTarget::Object(id);
    let (halfway, moved) = drag_frames(previewed.target_transform(target).unwrap());
    let live_geometry = drag_live(&harness, &mut live, target, &[halfway, moved]);

    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.rebuild(&harness.gpu, &mut previewed).unwrap();
    drag_previewed(&mut harness, &mut previewed, id, moved);
    release(&mut harness, &mut previewed, &mut geometry, target, moved);

    assert_eq!(
        previewed.objects(),
        live.objects(),
        "the object table differs"
    );
    let (ours, theirs) = (
        sorted_triangles(&geometry),
        sorted_triangles(&live_geometry),
    );
    assert_eq!(ours.len(), theirs.len());
    assert!(
        ours == theirs,
        "{} of {} released triangles differ from the live path's",
        ours.iter()
            .filter(|t| theirs.binary_search(t).is_err())
            .count(),
        ours.len()
    );
    let camera = support::framed(&previewed);
    let released = harness.capture(geometry.mesh(), &camera, false, "object-drag-cpu-released");
    let live_image = harness.capture(live_geometry.mesh(), &camera, false, "object-drag-cpu-live");
    assert_eq!(support::differing_pixels(&released, &live_image), 0);
}
