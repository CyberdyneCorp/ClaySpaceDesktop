//! Where the pointer lands on a subtool that has been stretched.
//!
//! A carried subtool is picked in its own coordinates: the ray is carried into
//! the frame the layer transform puts the content in, traced there, and the
//! answer carried back out. `Transform::into_local` carries a *point* by
//! turning it back and dividing by each factor — and a ray has two halves. The
//! origin went through that division and the direction did not, which was the
//! whole of the map while a layer transform took one factor and is half of it
//! now that it takes three.
//!
//! The half that is missing is the bearing, so the ray traced in the subtool's
//! own frame is not the ray that was asked. On a mesh it misses the form
//! altogether and `pick` answers `None`, which reads exactly like a pointer
//! that is not over the subtool: the brush ring never settles and the press
//! orbits instead of sculpting. On a broad surface it does worse — it finds
//! something, somewhere other than under the pointer.
//!
//! So the assertion here is the one a sculptor makes with their eyes: whatever
//! is picked has to lie **on the ray that asked for it**.

use clayspace_model::{
    BrushSettings, ConversionSettings, Direction, ExchangeModel, GestureSample, GizmoTarget,
    ImportSettings, MultiresLevelOp, ObjectModel, Representation, SceneModel, SculptModel,
    ToolKind, Transform,
};

use clayspace_engine::{BackendPolicy, ClayDocument};

/// A ray from off every axis, so a missing division cannot cancel out. Aimed
/// at the origin, which every fixture here is centred on.
const FROM: [f32; 3] = [4.0, 2.0, 4.0];
const TOWARD: [f32; 3] = [-4.0, -2.0, -4.0];

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far a point sits off the ray that asked for it, in world units.
fn off_the_ray(point: [f32; 3]) -> f32 {
    let length = TOWARD.iter().map(|a| a * a).sum::<f32>().sqrt();
    let unit: [f32; 3] = std::array::from_fn(|i| TOWARD[i] / length);
    let from: [f32; 3] = std::array::from_fn(|i| point[i] - FROM[i]);
    let along: f32 = (0..3).map(|i| from[i] * unit[i]).sum();
    let off: [f32; 3] = std::array::from_fn(|i| from[i] - unit[i] * along);
    off.iter().map(|a| a * a).sum::<f32>().sqrt()
}

/// Stretches the active subtool along one axis and picks it from off-axis.
///
/// The uniform scale beside it is the control: it goes through the very same
/// division on the origin, so a failure there would be the placement and not
/// the bearing.
fn the_pointer_finds_it_stretched(document: &mut ClayDocument, what: &str) {
    let key = document.scene().active.expect("an active layer");
    let mut stand = |scale: [f32; 3]| {
        document
            .set_target_transform(
                GizmoTarget::Layer(key),
                Transform {
                    scale,
                    ..Transform::default()
                },
            )
            .expect("a whole subtool stretches per axis");
        document.pick(FROM, TOWARD)
    };

    let uniform = stand([3.0, 3.0, 3.0]).unwrap_or_else(|| {
        panic!(
            "{what}: the control pick missed a uniformly scaled subtool, so \
                this fixture measures nothing"
        )
    });
    assert!(
        off_the_ray(uniform) < 1e-2,
        "{what}: even the control landed {:.4} off the ray",
        off_the_ray(uniform)
    );

    let stretched = stand([3.0, 1.0, 1.0]).unwrap_or_else(|| {
        panic!(
            "{what}: the pointer found nothing on a subtool squashed 3:1, which \
             reads exactly like a pointer that is not over the form — the ray's \
             bearing went through the rotation alone and points somewhere else \
             in a stretched frame"
        )
    });
    assert!(
        off_the_ray(stretched) < 1e-2,
        "{what}: the pointer landed {:.4} off the ray it was asked about, at \
         {stretched:?}",
        off_the_ray(stretched)
    );
}

/// A short stroke, which is what builds the sculpting session a mesh pick is
/// answered by.
fn dab(document: &mut ClayDocument, at: [f32; 3]) {
    document.begin_gesture();
    let _ = document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size: 0.2,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: at,
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    );
    document.end_gesture();
}

#[test]
fn a_stretched_mesh_subtool_is_found_under_the_pointer() {
    let mut document = sphere();
    let settings = ConversionSettings::default();
    document
        .convert_layer(Direction::SdfToMesh, settings.cell_size, settings.blur)
        .expect("into a mesh");
    // A mesh is picked through its sculpting session, which the first stroke
    // builds — without one the pick answers `None` for a reason that has
    // nothing to do with this.
    dab(&mut document, [0.0, 1.0, 0.0]);
    the_pointer_finds_it_stretched(&mut document, "mesh");
}

#[test]
fn a_stretched_grid_subtool_is_found_under_the_pointer() {
    let mut document = sphere();
    document
        .convert_layer(Direction::SdfToVoxel, 0.05, 0)
        .expect("into a grid");
    the_pointer_finds_it_stretched(&mut document, "grid");
}

#[test]
fn a_stretched_hierarchy_is_found_under_the_pointer() {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    let path =
        std::env::temp_dir().join(format!("clayspace-picking-cage-{}.obj", std::process::id()));
    cage_obj(&path, 8);
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import the cage");
    let _ = std::fs::remove_file(&path);
    let cage = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the cage is a mesh layer");
    document.set_active_layer(cage).expect("activate the cage");
    let settings = ConversionSettings::default();
    document
        .convert_layer_in_place(Direction::MeshToMultires, settings.cell_size, settings.blur)
        .expect("a flat quad grid is a cage");
    document
        .apply_multires_level_op(MultiresLevelOp::AddLevel)
        .expect("subdivide");
    // The third of the three paths that carry a ray in, and the one whose
    // triangles this side walks itself rather than handing to the engine.
    the_pointer_finds_it_stretched(&mut document, "hierarchy");
}

/// A flat grid of quads centred on the origin, which is what a Catmull-Clark
/// cage is supposed to be.
fn cage_obj(path: &std::path::Path, divisions: usize) {
    let mut text = String::new();
    let step = 4.0 / divisions as f32;
    for z in 0..=divisions {
        for x in 0..=divisions {
            text.push_str(&format!(
                "v {} 0 {}\n",
                -2.0 + step * x as f32,
                -2.0 + step * z as f32
            ));
        }
    }
    let stride = divisions + 1;
    for z in 0..divisions {
        for x in 0..divisions {
            let a = z * stride + x + 1;
            text.push_str(&format!(
                "f {} {} {} {}\n",
                a,
                a + stride,
                a + stride + 1,
                a + 1
            ));
        }
    }
    std::fs::write(path, text).expect("write the cage");
}
