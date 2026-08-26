//! Whether the cage reaches the screen, and whether dragging it reaches the
//! form.
//!
//! A deformation cage is worked *in the viewport*: the sculptor's attention is
//! on a handle rather than on the clay, and every part of that can fail on its
//! own. The cage can be built and not drawn; drawn and not hit by the pointer;
//! hit and wired to nothing. So these run the whole path — build a cage,
//! assemble the overlay the viewport would upload, render a frame, and measure
//! the picture.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_lattice
//! open target/visual
//! ```

mod support;

/// A camera in front of the form. Only the outer ring reads it.
const LOOKING_DOWN_Z: [f32; 3] = [0.0, 0.0, 1.0];

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{Direction, GizmoHandle, GizmoMode, LatticeModel, LatticeState, SculptModel};
use clayspace_view::{Camera, GizmoView, Image, LatticeView, Vertex};
use support::Harness;

fn meshed() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    document.convert_layer(Direction::SdfToMesh, 0.03, 0).ok()?;
    Some(document)
}

fn framed(document: &ClayDocument) -> Camera {
    let mut camera = Camera::default();
    match SculptModel::bounds(document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }
    camera
}

/// The surface the viewport would draw.
fn surface(document: &mut ClayDocument) -> (Vec<Vertex>, Vec<u32>) {
    let (positions, normals, colors, indices) = document.visible_mesh_geometry();
    let vertices = positions
        .into_iter()
        .zip(normals)
        .zip(colors)
        .map(|((position, normal), color)| Vertex {
            position,
            normal,
            color,
            mask: 0.0,
        })
        .collect();
    (vertices, indices)
}

/// The handle size the application computes, spelled the same way.
fn handle(cage: &LatticeState) -> f32 {
    cage.rest_span * 0.022
}

fn how_many_differ(a: &Image, b: &Image) -> usize {
    a.pixels
        .chunks_exact(4)
        .zip(b.pixels.chunks_exact(4))
        .filter(|(x, y)| (0..3).any(|c| x[c].abs_diff(y[c]) > 12))
        .count()
}

#[test]
fn a_cage_is_drawn_around_the_form() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed() else {
        return;
    };
    let camera = framed(&document);
    let (vertices, indices) = surface(&mut document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);

    let bare = harness.capture(&mesh, &camera, false, "84-cage-none");

    document.begin_lattice([3, 3, 3]).expect("a cage");
    let cage = document.lattice();
    let edges = cage.edges();
    harness.renderer.set_lattice(
        &harness.gpu,
        LatticeView {
            points: &cage.points,
            edges: &edges,
            selected: &[],
            gizmo: None,
            outline: None,
            handle: handle(&cage),
        },
    );
    let caged = harness.capture(&mesh, &camera, false, "85-cage");

    let changed = how_many_differ(&bare, &caged);
    assert!(
        changed > 1500,
        "putting a cage up changed {changed} pixels — a cage that is built and \\
         not drawn is a deformer with no interface at all. See \\
         target/visual/85-cage.png"
    );

    // And it is scaffolding rather than a wall: it must not cover the form it
    // is wrapped around.
    let lit = |image: &Image| {
        let ground = image.pixel(4, 4);
        image
            .pixels
            .chunks_exact(4)
            .filter(|p| (0..3).any(|c| p[c].abs_diff(ground[c]) > 12))
            .count()
    };
    let covered = lit(&caged) as f64 / lit(&bare) as f64;
    assert!(
        covered < 1.6,
        "the cage brought the drawn area to {covered:.2}x the bare form, which \\
         is a box in front of the sculpture rather than a cage around it"
    );
}

#[test]
fn the_selected_point_is_told_apart_from_the_rest() {
    // Which point is in hand has to be legible without reading the colour,
    // which a sculptor watching the form is not doing.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed() else {
        return;
    };
    let camera = framed(&document);
    let (vertices, indices) = surface(&mut document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);

    document.begin_lattice([2, 2, 2]).expect("a cage");
    let cage = document.lattice();
    let edges = cage.edges();
    let draw = |harness: &mut Harness, selected: &[usize], name: &str| {
        harness.renderer.set_lattice(
            &harness.gpu,
            LatticeView {
                points: &cage.points,
                edges: &edges,
                selected,
                // No manipulator: this is about the handles themselves, and a
                // gizmo over them would be measuring its own pixels.
                gizmo: None,
                outline: None,
                handle: handle(&cage),
            },
        );
        harness.capture(&mesh, &camera, false, name)
    };
    let quiet = draw(&mut harness, &[], "86-cage-quiet");
    let picked = draw(&mut harness, &[7], "87-cage-selected");

    let changed = how_many_differ(&quiet, &picked);
    assert!(
        changed > 30,
        "selecting a control point changed {changed} pixels, so a sculptor \\
         cannot see which one is in hand. See target/visual/87-cage-selected.png"
    );
}

#[test]
fn dragging_the_cage_reaches_the_drawn_surface() {
    // The whole path, end to end: the form the viewport draws has to change
    // when the cage is applied, not only the numbers behind it.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed() else {
        return;
    };
    let camera = framed(&document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);

    let (vertices, indices) = surface(&mut document);
    mesh.upload(&harness.gpu, &vertices, &indices);
    let before = harness.capture(&mesh, &camera, false, "88-cage-before");

    document.begin_lattice([2, 2, 2]).expect("a cage");
    let cage = document.lattice();
    for (index, point) in cage.points.iter().enumerate() {
        if point[1] <= 0.0 {
            continue;
        }
        document.select_lattice_point(Some(index));
        document
            .drag_lattice_point([point[0], point[1] + 0.5, point[2]])
            .expect("the drag was refused");
    }
    document.apply_lattice().expect("the cage was refused");

    let (vertices, indices) = surface(&mut document);
    mesh.upload(&harness.gpu, &vertices, &indices);
    harness.renderer.set_lattice(
        &harness.gpu,
        LatticeView {
            points: &[],
            edges: &[],
            selected: &[],
            gizmo: None,
            outline: None,
            handle: 0.0,
        },
    );
    let after = harness.capture(&mesh, &camera, false, "89-cage-after");

    let changed = how_many_differ(&before, &after);
    assert!(
        changed > 3000,
        "bending the form through the cage changed {changed} pixels of the \\
         drawn surface. See target/visual/89-cage-after.png"
    );
}

// -- the manipulator ---------------------------------------------------------

#[test]
fn each_manipulator_mode_draws_its_own_handles() {
    // Shapes rather than colours alone carry the meaning — an arrow slides, a
    // ring turns, a box scales — because a person reaching for a handle is not
    // reading a legend, and because the three axis colours are the one part of
    // this a colour-blind sculptor cannot use. So the three modes have to be
    // told apart by their picture.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed() else {
        return;
    };
    let camera = framed(&document);
    let (vertices, indices) = surface(&mut document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);

    document.begin_lattice([2, 2, 2]).expect("a cage");
    let cage = document.lattice();
    let edges = cage.edges();
    let selected = [7usize];
    let pivot = cage.points[7];

    let shot = |harness: &mut Harness, mode: Option<GizmoMode>, name: &str| {
        harness.renderer.set_lattice(
            &harness.gpu,
            LatticeView {
                points: &cage.points,
                edges: &edges,
                selected: &selected,
                gizmo: mode.map(|mode| GizmoView {
                    view_axis: LOOKING_DOWN_Z,
                    per_axis_scale: true,
                    pivot,
                    mode,
                    reach: handle(&cage) * 12.0,
                    hovered: None,
                }),
                outline: None,
                handle: handle(&cage),
            },
        );
        harness.capture(&mesh, &camera, false, name)
    };

    let bare = shot(&mut harness, None, "94-gizmo-none");
    let move_ = shot(&mut harness, Some(GizmoMode::Move), "95-gizmo-move");
    let rotate = shot(&mut harness, Some(GizmoMode::Rotate), "96-gizmo-rotate");
    let scale = shot(&mut harness, Some(GizmoMode::Scale), "97-gizmo-scale");

    for (name, image) in [("Mover", &move_), ("Girar", &rotate), ("Escalar", &scale)] {
        let drawn = how_many_differ(&bare, image);
        assert!(
            drawn > 300,
            "{name} drew {drawn} pixels over the bare cage, which is a \
             manipulator that is not there. See target/visual/"
        );
    }

    // And the three are different pictures, not one picture in three colours.
    for (a, b, names) in [
        (&move_, &rotate, "Mover and Girar"),
        (&rotate, &scale, "Girar and Escalar"),
        (&move_, &scale, "Mover and Escalar"),
    ] {
        let apart = how_many_differ(a, b);
        assert!(
            apart > 200,
            "{names} differ by {apart} pixels, so the mode cannot be told from \
             the widget"
        );
    }
}

#[test]
fn the_manipulator_sits_on_the_middle_of_the_selection() {
    // The middle rather than the last point picked, so adding a point moves
    // the widget to where the selection is rather than leaving it on whichever
    // corner was clicked first.
    let Some(mut document) = meshed() else {
        return;
    };
    document.begin_lattice([2, 2, 2]).expect("a cage");
    document.select_lattice_point(Some(7));
    let one = document.lattice().pivot().expect("a middle");
    assert_eq!(one, document.lattice().points[7]);

    // The whole top face.
    let cage = document.lattice();
    let face: Vec<usize> = cage
        .points
        .iter()
        .enumerate()
        .filter(|(_, point)| point[1] > 0.0)
        .map(|(index, _)| index)
        .collect();
    document.select_lattice_point(None);
    for index in &face {
        document.toggle_lattice_point(*index);
    }
    let middle = document.lattice().pivot().expect("a middle");
    assert!(
        middle[0].abs() < 1e-4 && middle[2].abs() < 1e-4 && middle[1] > 1.0,
        "the top face's manipulator sits at {middle:?} rather than over the \
         middle of that face"
    );
}

#[test]
fn the_bend_reaches_the_screen_while_the_cage_is_being_dragged() {
    // The whole point of a preview, and every part of it can fail on its own:
    // the engine can deform and not raise its revision, and the revision can
    // move without the geometry the viewport reads changing. So this renders
    // mid-gesture, with nothing applied.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed() else {
        return;
    };
    let camera = framed(&document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);

    let (vertices, indices) = surface(&mut document);
    mesh.upload(&harness.gpu, &vertices, &indices);
    let rest = harness.capture(&mesh, &camera, false, "98-preview-rest");

    document.begin_lattice([2, 2, 2]).expect("a cage");
    let cage = document.lattice();
    for (index, point) in cage.points.iter().enumerate() {
        if point[1] > 0.0 {
            document.toggle_lattice_point(index);
        }
    }
    let pivot = document.lattice().pivot().expect("a middle");
    document.set_gizmo_mode(GizmoMode::Move);
    document.begin_gizmo_drag(GizmoHandle::Axis(1), pivot, LOOKING_DOWN_Z);
    document
        .drag_gizmo([pivot[0], pivot[1] + 0.5, pivot[2]], false)
        .expect("the drag was refused");

    // Still mid-gesture: nothing applied, nothing banked, the cage still up.
    assert!(document.lattice().active, "the cage came down on its own");
    let (vertices, indices) = surface(&mut document);
    mesh.upload(&harness.gpu, &vertices, &indices);
    let during = harness.capture(&mesh, &camera, false, "99-preview-during");

    let changed = how_many_differ(&rest, &during);
    assert!(
        changed > 3000,
        "the drawn surface changed {changed} pixels while the cage was being \
         dragged, so the sculptor is setting corners blind and looking at the \
         result only after Deformar. See target/visual/99-preview-during.png"
    );
}

// -- a field cage, previewed on the drawn surface ----------------------------
//
// The field route cannot preview itself: applying a cage writes a deformer
// into the document as an undoable edit and refills the layer's whole brick
// region, 68.8 ms measured. So the preview moves the vertices the viewport
// already holds, by the warp the engine supplies. Every part of that can fail
// on its own — the warp can be zero, the vertices can be moved without being
// uploaded, and the surface can be left bent after the cage comes down.

fn field() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()
}

#[test]
fn a_field_cage_bends_the_drawn_surface_while_it_is_dragged() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = field() else {
        return;
    };
    let camera = framed(&document);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry
        .sync(&harness.gpu, &mut document)
        .expect("the first mesh");
    let rest = harness.capture(geometry.mesh(), &camera, false, "100-field-cage-rest");

    document.begin_lattice([4, 4, 4]).expect("a cage");
    let cage = document.lattice();
    for (index, point) in cage.points.iter().enumerate() {
        if point[2] <= 0.0 {
            continue;
        }
        document.select_lattice_point(Some(index));
        document
            .drag_lattice_point([point[0], point[1], point[2] + 0.25])
            .expect("the drag was refused");
    }
    // Exactly what the application does when the cage's revision moves — and
    // deliberately without a `sync`, because a cage that has not been applied
    // has changed nothing for a re-mesh to find.
    geometry.preview_cage(&harness.gpu, &document);
    let during = harness.capture(geometry.mesh(), &camera, false, "101-field-cage-during");

    let changed = how_many_differ(&rest, &during);
    assert!(
        changed > 2000,
        "a field cage dragged a quarter of the way across its box changed \
         {changed} pixels of the drawn surface, so the sculptor pulls a corner \
         and watches nothing happen. See target/visual/101-field-cage-during.png"
    );
}

#[test]
fn taking_the_field_cage_down_puts_the_surface_back() {
    // The preview moves vertices the document knows nothing about, so nothing
    // else would ever put them back.
    let Some(harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = field() else {
        return;
    };
    let camera = framed(&document);
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    geometry.sync(&harness.gpu, &mut document).expect("mesh");
    let rest = harness.capture(geometry.mesh(), &camera, false, "102-field-cage-before");

    document.begin_lattice([4, 4, 4]).expect("a cage");
    let cage = document.lattice();
    for (index, point) in cage.points.iter().enumerate() {
        if point[2] > 0.0 {
            document.select_lattice_point(Some(index));
            document
                .drag_lattice_point([point[0], point[1], point[2] + 0.25])
                .expect("the drag was refused");
        }
    }
    geometry.preview_cage(&harness.gpu, &document);
    assert!(
        how_many_differ(
            &rest,
            &harness.capture(geometry.mesh(), &camera, false, "103-field-cage-shown")
        ) > 2000,
        "there was no preview to take back"
    );

    document.cancel_lattice();
    geometry.preview_cage(&harness.gpu, &document);
    let after = harness.capture(geometry.mesh(), &camera, false, "104-field-cage-cleared");
    assert_eq!(
        how_many_differ(&rest, &after),
        0,
        "abandoning the cage left the drawn surface bent, which is a picture \
         of a document that does not exist"
    );
}

#[test]
fn the_preview_tracks_what_the_engine_will_actually_do() {
    // The forward warp against the field's own inverse-map deformer. They are
    // not the same map, and the size of the difference is the whole question:
    // a preview that pointed the wrong way would be worse than none.
    let Some(mut previewed) = field() else {
        return;
    };
    previewed.begin_lattice([4, 4, 4]).expect("a cage");
    let at = SculptModel::pick(&previewed, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]).expect("surface");
    let cage = previewed.lattice();
    let drag = 0.25f32;
    for (index, point) in cage.points.iter().enumerate() {
        if point[2] > 0.0 {
            previewed.select_lattice_point(Some(index));
            previewed
                .drag_lattice_point([point[0], point[1], point[2] + drag])
                .expect("the drag was refused");
        }
    }
    let shown = previewed.cage_warp(&[at]).expect("a field cage warps")[0];
    let predicted = at[2] + shown[2];

    previewed.apply_lattice().expect("the cage was refused");
    let actual =
        SculptModel::pick(&previewed, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]).expect("surface")[2];

    let error = (actual - predicted).abs() / drag;
    assert!(
        error < 0.05,
        "the preview showed the surface at {predicted} and the engine put it \
         at {actual}, which is {:.0}% of the drag — a preview is allowed an \
         error budget an edit is not, but not that one",
        error * 100.0
    );
}

// -- what a cage takes over --------------------------------------------------
//
// Three things reported from using it. A cage is a mode, and none of these was
// treating it as one.

#[test]
fn the_form_is_drawn_through_while_a_cage_is_up() {
    // Half the control points are behind the form, and a solid surface hides
    // exactly the handles that need reaching. Blender's X-ray and ZBrush's
    // Ghost do the same thing for the same reason.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed() else {
        return;
    };
    let camera = framed(&document);
    let (vertices, indices) = surface(&mut document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);

    document.begin_lattice([2, 2, 2]).expect("a cage");
    let cage = document.lattice();
    let edges = cage.edges();
    let with_cage = |harness: &mut Harness, ghosted: bool, name: &str| {
        harness.renderer.set_ghosted(ghosted);
        harness.renderer.set_lattice(
            &harness.gpu,
            LatticeView {
                points: &cage.points,
                edges: &edges,
                selected: &[],
                gizmo: None,
                outline: None,
                handle: handle(&cage),
            },
        );
        harness.capture(&mesh, &camera, false, name)
    };
    let solid = with_cage(&mut harness, false, "105-cage-solid");
    let ghosted = with_cage(&mut harness, true, "106-cage-ghosted");
    harness.renderer.set_ghosted(false);

    let changed = how_many_differ(&solid, &ghosted);
    assert!(
        changed > 4000,
        "the form was drawn the same way with a cage up and without one, so \
         the control points behind it are still hidden ({changed} pixels). See \
         target/visual/106-cage-ghosted.png"
    );

    // Seen through, not turned off: the form is still readable as a form.
    let ground = ghosted.pixel(4, 4);
    let visible = ghosted
        .pixels
        .chunks_exact(4)
        .filter(|p| (0..3).any(|c| p[c].abs_diff(ground[c]) > 12))
        .count();
    let was = solid
        .pixels
        .chunks_exact(4)
        .filter(|p| (0..3).any(|c| p[c].abs_diff(ground[c]) > 12))
        .count();
    assert!(
        visible as f64 > was as f64 * 0.8,
        "the ghosted form covers {visible} pixels against {was} solid, which \
         is a surface turned off rather than one seen through"
    );
}

#[test]
fn a_handle_keeps_its_size_when_another_point_is_dragged() {
    // Reported: selecting a point and moving it made every other handle grow.
    // The size came from the cage's *current* extent, so hauling one corner
    // out inflated the whole set — and the targets a sculptor was aiming at
    // swelled under the pointer as they worked.
    let Some(mut document) = meshed() else {
        return;
    };
    document.begin_lattice([2, 2, 2]).expect("a cage");
    let before = handle(&document.lattice());

    let cage = document.lattice();
    document.select_lattice_point(Some(7));
    let point = cage.points[7];
    document
        .drag_lattice_point([point[0] + 3.0, point[1] + 3.0, point[2] + 3.0])
        .expect("the drag was refused");

    let after = handle(&document.lattice());
    assert!(
        (after - before).abs() < 1e-6,
        "a corner hauled three units out took the handle size from {before} to \
         {after}"
    );
}

#[test]
fn the_outer_ring_is_drawn_and_faces_the_camera() {
    // ZBrush's outermost ring. It turns the selection in the frame the sculptor
    // is looking at it from, so it must be a circle seen square on from the
    // camera — whichever way the camera is turned — and it must sit outside the
    // three axis rings rather than among them.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed() else {
        return;
    };
    let (vertices, indices) = surface(&mut document);
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);

    document.begin_lattice([2, 2, 2]).expect("a cage");
    let cage = document.lattice();
    let edges = cage.edges();
    let selected = [7usize];
    let pivot = cage.points[7];

    // Two cameras a long way apart, so "faces the eye" is a claim with teeth.
    for (name, azimuth) in [("front", 0.0f32), ("corner", 0.9f32)] {
        let mut camera = framed(&document);
        camera.orbit(azimuth, 0.4);
        let eye: [f32; 3] = camera.eye().into();
        let away: [f32; 3] = std::array::from_fn(|i| eye[i] - pivot[i]);
        let length = away.iter().map(|c| c * c).sum::<f32>().sqrt();
        let view_axis: [f32; 3] = std::array::from_fn(|i| away[i] / length);

        let with = |harness: &mut Harness, mode: GizmoMode, capture: &str| {
            harness.renderer.set_lattice(
                &harness.gpu,
                LatticeView {
                    points: &cage.points,
                    edges: &edges,
                    selected: &selected,
                    gizmo: Some(GizmoView {
                        view_axis,
                        pivot,
                        mode,
                        reach: handle(&cage) * 12.0,
                        hovered: None,
                        per_axis_scale: true,
                    }),
                    outline: None,
                    handle: handle(&cage),
                },
            );
            harness.capture(&mesh, &camera, false, capture)
        };

        // A rotate manipulator draws more than a move one: three rings and the
        // outer one against three shafts and a centre.
        let moving = with(
            &mut harness,
            GizmoMode::Move,
            &format!("120-outer-move-{name}"),
        );
        let turning = with(
            &mut harness,
            GizmoMode::Rotate,
            &format!("121-outer-rotate-{name}"),
        );
        let changed = how_many_differ(&moving, &turning);
        assert!(
            changed > 500,
            "the two modes drew nearly the same thing from {name} ({changed} \
             pixels). See target/visual/121-outer-rotate-{name}.png"
        );
    }
}

/// Every control point on the far side of one axis — a whole face, which is
/// what the manipulator exists for.
fn select_the_far_face(document: &mut ClayDocument, axis: usize) {
    let face: Vec<usize> = document
        .lattice()
        .points
        .iter()
        .enumerate()
        .filter(|(_, point)| point[axis] > 0.0)
        .map(|(index, _)| index)
        .collect();
    for index in face {
        document.toggle_lattice_point(index);
    }
}

#[test]
fn turning_a_face_visibly_turns_the_cage_on_screen() {
    // What I should have checked before saying rotation worked: not that the
    // arithmetic turns a point, but that dragging a ring the way a hand does
    // moves what is drawn. Reported twice as "nothing happens", and both
    // causes are visible here or not at all.
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some(mut document) = meshed() else {
        return;
    };
    let camera = framed(&document);
    let facing = [0.0, 0.0, 1.0];

    document.begin_lattice([2, 2, 2]).expect("a cage");
    // A whole face, not one point: one point's middle is itself, and turning
    // it about itself is exactly no movement.
    select_the_far_face(&mut document, 1);
    assert!(
        document.lattice().can_transform(),
        "a face should be enough to turn"
    );
    let pivot = document.lattice().pivot().expect("a middle");

    let shot = |harness: &mut Harness, document: &mut ClayDocument, name: &str| {
        let cage = document.lattice();
        let edges = cage.edges();
        harness.renderer.set_lattice(
            &harness.gpu,
            LatticeView {
                points: &cage.points,
                edges: &edges,
                selected: &cage.selection,
                gizmo: cage.pivot().map(|pivot| GizmoView {
                    view_axis: facing,
                    per_axis_scale: true,
                    pivot,
                    mode: cage.mode,
                    reach: handle(&cage) * 12.0,
                    hovered: None,
                }),
                outline: None,
                handle: handle(&cage),
            },
        );
        let (vertices, indices) = surface(document);
        let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
        mesh.upload(&harness.gpu, &vertices, &indices);
        harness.capture(&mesh, &camera, false, name)
    };

    document.set_gizmo_mode(GizmoMode::Rotate);
    let before = shot(&mut harness, &mut document, "122-turn-before");

    // The drag, routed through the same plane choice the application uses.
    let handle_grabbed = GizmoHandle::Axis(1);
    let normal = clayspace_model::drag_plane(GizmoMode::Rotate, handle_grabbed, facing, facing);
    let (across, other) = clayspace_model::perpendicular_frame(normal);
    let anchor: [f32; 3] = std::array::from_fn(|i| pivot[i] + across[i]);
    let to: [f32; 3] = std::array::from_fn(|i| pivot[i] + other[i]);
    document.begin_gizmo_drag(handle_grabbed, anchor, facing);
    document
        .drag_gizmo(to, false)
        .expect("the drag was refused");

    let after = shot(&mut harness, &mut document, "123-turn-after");

    let changed = how_many_differ(&before, &after);
    assert!(
        changed > 2000,
        "a quarter turn of a whole face changed {changed} pixels, which is not \
         a turn anybody would see. Compare target/visual/122-turn-before.png \
         and 123-turn-after.png"
    );
}
