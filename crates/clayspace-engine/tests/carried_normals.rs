//! What a stretched subtool is shaded by.
//!
//! A carried subtool — a mesh layer, a grid, a hierarchy — is placed by *this*
//! side and not by the tape: the engine holds its vertices where they were
//! built, and the drawing moves them through the layer transform on the way to
//! the viewport. Positions go through `Transform::into_world`, which stretches
//! each axis by its own factor since a layer transform grew three of them.
//!
//! A normal does not go the same way. A surface normal transforms by the
//! **inverse transpose** of the map its surface goes through — `R * diag(1/s)`
//! where the surface goes through `R * diag(s)` — and rotating one alone is
//! right only while the three factors agree. clay.h says so in a line, about
//! the layer transform the engine composes for itself: "rotating a normal is
//! right for a similarity and tilts every one of them off the surface under a
//! squash."
//!
//! Nothing errors when this is wrong. The geometry is uploaded correctly and
//! only the lighting describes a different form — highlights and the
//! silhouette terminator in the wrong place on a subtool that is drawn in the
//! right one — which is the readout a sculptor judges form by and the one no
//! assertion about *where* anything is can catch.

use clayspace_model::{
    ConversionSettings, Direction, GizmoTarget, ObjectModel, SceneModel, Transform,
};

use clayspace_engine::{BackendPolicy, ClayDocument};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far, in degrees, the shipped vertex normals sit from the surface they
/// are shipped with.
///
/// Measured against the geometry as *drawn*: the plane of each drawn triangle
/// against each of the three normals drawn with it. That makes the comparison
/// self-contained — it asks whether the lighting describes the form on the
/// screen, which is the only question a sculptor can see the answer to.
///
/// It never reaches zero. A smooth normal on a faceted surface disagrees with
/// every face it touches, and that floor is what the identity case measures so
/// the stretched case has something honest to be compared against.
fn how_far_the_shading_is_from_the_form(document: &mut ClayDocument) -> (f32, f32) {
    let (positions, normals, _, indices, _) = document.visible_mesh_geometry();
    assert!(indices.len() >= 3, "the fixture drew no triangles");
    let mut total = 0.0f32;
    let mut worst = 0.0f32;
    let mut samples = 0u32;
    for triangle in indices.chunks_exact(3) {
        let corner = |at: u32| positions[at as usize];
        let (a, b, c) = (
            corner(triangle[0]),
            corner(triangle[1]),
            corner(triangle[2]),
        );
        let edge = |from: [f32; 3], to: [f32; 3]| std::array::from_fn(|i| to[i] - from[i]);
        let (u, v): ([f32; 3], [f32; 3]) = (edge(a, b), edge(a, c));
        let face = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let Some(face) = unit(face) else {
            continue;
        };
        for at in triangle {
            let Some(normal) = unit(normals[*at as usize]) else {
                continue;
            };
            let dot: f32 = (0..3).map(|i| face[i] * normal[i]).sum();
            let angle = dot.abs().min(1.0).acos().to_degrees();
            total += angle;
            worst = worst.max(angle);
            samples += 1;
        }
    }
    assert!(samples > 0, "nothing was measured");
    (total / samples as f32, worst)
}

fn unit(v: [f32; 3]) -> Option<[f32; 3]> {
    let length = v.iter().map(|a| a * a).sum::<f32>().sqrt();
    (length > 1e-9).then(|| std::array::from_fn(|i| v[i] / length))
}

/// A stretched mesh subtool is shaded by the surface it is drawn as.
///
/// Measured on a meshed starting form: at an identity scale the shipped
/// normals sit a mean 1.5 degrees off the triangles they belong to, which is
/// the ordinary faceting floor. Squashed 4:1 along one axis and shaded from
/// normals that were only rotated, that became a mean 20.9 degrees and a worst
/// case of 39 — the whole subtool lighting as if it had never been squashed.
#[test]
fn a_stretched_mesh_subtool_is_lit_by_the_form_it_draws() {
    let mut document = sphere();
    let settings = ConversionSettings::default();
    document
        .convert_layer(Direction::SdfToMesh, settings.cell_size, settings.blur)
        .expect("into a mesh");
    let key = document.scene().active.expect("an active layer");

    let (upright, _) = how_far_the_shading_is_from_the_form(&mut document);
    document
        .set_target_transform(
            GizmoTarget::Layer(key),
            Transform {
                scale: [1.0, 4.0, 1.0],
                ..Transform::default()
            },
        )
        .expect("a whole subtool stretches per axis");

    let (positions, ..) = document.visible_mesh_geometry();
    let tallest = positions
        .iter()
        .fold(0.0f32, |so_far, point| so_far.max(point[1].abs()));
    assert!(
        tallest > 2.0,
        "the stretch never reached the drawn positions, so this measures \
         nothing: {tallest}"
    );

    let (stretched, worst) = how_far_the_shading_is_from_the_form(&mut document);
    assert!(
        stretched < upright + 1.0,
        "the drawn normals are a mean {stretched:.2} degrees off the drawn \
         surface, against {upright:.2} unstretched (worst {worst:.2}): a \
         normal went through the rotation alone, which is right for a \
         similarity and tilts every one of them off the surface under a squash"
    );
}

/// The same for a grid, which is drawn by the shared placement path.
///
/// `append_placed` serves the voxel and hierarchy arms alike, so this is the
/// half of the repair that the mesh case does not reach.
#[test]
fn a_stretched_grid_subtool_is_lit_by_the_form_it_draws() {
    let mut document = sphere();
    document
        .convert_layer(Direction::SdfToVoxel, 0.05, 0)
        .expect("into a grid");
    let key = document.scene().active.expect("an active layer");

    let (upright, _) = how_far_the_shading_is_from_the_form(&mut document);
    document
        .set_target_transform(
            GizmoTarget::Layer(key),
            Transform {
                scale: [3.0, 1.0, 1.0],
                ..Transform::default()
            },
        )
        .expect("a whole subtool stretches per axis");
    let (stretched, worst) = how_far_the_shading_is_from_the_form(&mut document);
    assert!(
        stretched < upright + 1.0,
        "the grid's drawn normals are a mean {stretched:.2} degrees off the \
         drawn surface, against {upright:.2} unstretched (worst {worst:.2})"
    );
}
