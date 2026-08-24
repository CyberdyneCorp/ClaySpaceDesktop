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

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{Direction, GizmoMode, LatticeModel, LatticeState, SculptModel};
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
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for point in &cage.points {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    (0..3)
        .map(|axis| max[axis] - min[axis])
        .fold(0.0f32, f32::max)
        * 0.022
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
                    pivot,
                    mode,
                    reach: handle(&cage) * 12.0,
                    hovered: None,
                }),
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
