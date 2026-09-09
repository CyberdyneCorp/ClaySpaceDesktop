//! Baking from a field, and the inversion that would look fine and be wrong.

use claycore::{Document, Item, MeasureParams, MeshParams, Mesher, Op, SurfaceMeasure};
use cyberremesh::{bake_field, remesh, BakeParams, Field, FieldMap, Mesh, RemeshParams, Unwatched};

/// ClayCore's field, answering the three callbacks the baker wants.
///
/// This is the correspondence the whole integration exists for, and it is
/// where the inversion lives — deliberately here, in an implementation a
/// reader can see, rather than inside the wrapper where it would be invisible.
struct ClayField {
    document: Document,
}

impl Field for ClayField {
    fn distance(&self, at: [f32; 3]) -> f32 {
        self.document
            .eval_points(None, &[at])
            .ok()
            .and_then(|v| v.first().copied())
            .unwrap_or(0.0)
    }

    fn gradient(&self, at: [f32; 3]) -> [f32; 3] {
        self.document
            .eval_gradients(None, &[at])
            .ok()
            .and_then(|v| v.first().copied())
            .unwrap_or([0.0, 1.0, 0.0])
    }

    /// **The inversion.**
    ///
    /// The baker wants *openness*, where 1 is fully open. ClayCore answers
    /// *occlusion*, where 1 is fully enclosed. `1.0 - x` is the whole of the
    /// conversion and skipping it bakes an ambient occlusion map that is dark
    /// where it should be light, everywhere, while looking entirely plausible.
    fn openness(&self, at: [f32; 3], normal: [f32; 3], radius: f32) -> f32 {
        let occlusion = self
            .document
            .measure_points(
                SurfaceMeasure::Occlusion,
                &[at],
                MeasureParams {
                    direction: normal,
                    ..MeasureParams::occlusion(radius, 32)
                },
            )
            .ok()
            .and_then(|v| v.first().copied())
            .unwrap_or(0.0);
        1.0 - occlusion
    }
}

fn scene() -> Option<(ClayField, Mesh)> {
    let mut document = Document::new().ok()?;
    let layer = document.add_sdf_layer("corpo").ok()?;
    for x in [-0.35f32, 0.35] {
        let mut lobe = Item::sphere(0.5).ok()?;
        lobe.set_op(Op::Add).ok()?;
        lobe.set_position([x, 0.0, 0.0]).ok()?;
        document.add_item(layer, &lobe).ok()?;
    }
    let mesh = document
        .mesh(MeshParams {
            voxel_size: Some(0.04),
            mesher: Mesher::MarchingTetrahedra,
            ..MeshParams::default()
        })
        .ok()?;
    let positions: Vec<f32> = mesh.positions().iter().flat_map(|p| *p).collect();
    let normals: Vec<f32> = mesh
        .normals()
        .map(|n| n.iter().flat_map(|n| *n).collect())
        .unwrap_or_default();
    let across = Mesh::from_handoff(
        &positions,
        &normals,
        mesh.indices(),
        claycore::HANDOFF_VERSION,
        "ClaySpaceDesktop",
    )
    .ok()?;
    // Retopologised first, because a bake wants a low-poly with a UV layout
    // and the sculpt is neither.
    let low = remesh(
        &across,
        RemeshParams {
            target_quads: 400,
            ..RemeshParams::default()
        },
        &mut Unwatched,
    )
    .ok()?;
    Some((ClayField { document }, low))
}

#[test]
fn every_field_map_bakes_with_no_high_poly_mesh() {
    cyberremesh::set_max_worker_threads(4).expect("the cap");
    let Some((field, mut low)) = scene() else {
        println!("no engine backend; skipping");
        return;
    };
    // A layout, because a bake writes into one.
    let atlas = cyberremesh::atlas(&mut low, Default::default(), &mut Unwatched)
        .expect("the low-poly unwraps");
    println!(
        "atlas: {} charts, {:.4} max angle distortion, {} flipped",
        atlas.charts, atlas.max_angle_distortion, atlas.flipped_charts
    );

    let params = BakeParams {
        width: 128,
        height: 128,
        ..BakeParams::default()
    };
    for map in FieldMap::ALL {
        let image = bake_field(&low, map, params, &field)
            .unwrap_or_else(|e| panic!("{} refused: {e}", map.label()));
        let pixels = image.pixels();
        let finite = pixels.iter().filter(|v| v.is_finite()).count();
        println!(
            "  {:<18} {}x{} x{} ch, {finite}/{} finite",
            map.label(),
            image.width(),
            image.height(),
            image.channels(),
            pixels.len()
        );
        assert_eq!(
            image.width(),
            128,
            "{} baked at the wrong size",
            map.label()
        );
        assert!(
            !pixels.is_empty(),
            "{} produced an empty image",
            map.label()
        );
        assert_eq!(
            finite,
            pixels.len(),
            "{} produced non-finite pixels, which render as plausible garbage",
            map.label()
        );
    }
}

/// The field really is being sampled, and it is ours — and the inversion is
/// load-bearing.
///
/// **The first version of this test could not fail.** It measured openness at
/// `[0, 0, 0]` and `[0.85, 0, 0]`, called them "crease" and "flank", and both
/// returned exactly 1.000: the first point is deep *inside* two overlapping
/// lobes, and occlusion at an interior point is not a measurement of anything.
/// So `crease <= flank` held because the two were equal, which is the vacuous
/// guard this project keeps finding in other people's code.
///
/// Occlusion is a property of a point **on** the surface with a normal, so the
/// fixture now takes both from the meshed surface itself and asserts the two
/// are *different* before comparing them.
#[test]
fn the_field_the_baker_samples_is_claycores() {
    let Some((field, _)) = scene() else {
        return;
    };
    let inside = field.distance([0.0, 0.0, 0.0]);
    let outside = field.distance([0.0, 5.0, 0.0]);
    assert!(
        inside < 0.0,
        "the field says the middle of two overlapping lobes is not inside it"
    );
    assert!(
        outside > 0.0,
        "the field says a point five units away is inside"
    );

    // Two real surface points: one in the saddle where the lobes meet, which
    // is enclosed, and one on an outer flank, which is open. Found by walking
    // the surface rather than guessed at, because guessing is what produced a
    // test that could not fail.
    let crease = surface_point(&field, [0.0, 1.0, 0.0]);
    let flank = surface_point(&field, [1.0, 0.0, 0.0]);
    let (crease_open, flank_open) = (
        field.openness(crease.0, crease.1, 0.6),
        field.openness(flank.0, flank.1, 0.6),
    );
    println!(
        "openness: saddle {crease_open:.3} at {:?}, flank {flank_open:.3} at {:?}",
        crease.0, flank.0
    );

    // **The fixture discriminates, asserted before it is relied on.** Without
    // this the comparison below passes on two equal numbers and says nothing.
    assert!(
        (crease_open - flank_open).abs() > 0.01,
        "the two probe points report the same openness ({crease_open:.3}), so \
         this fixture cannot tell a correct conversion from an inverted one"
    );
    assert!(
        crease_open < flank_open,
        "the saddle reports {crease_open:.3} open against the flank's \
         {flank_open:.3}. Openness runs the other way from ClayCore's \
         occlusion, so this is what a missing `1.0 -` looks like — and an AO \
         map baked from it is dark where it should be light, everywhere, while \
         looking entirely plausible"
    );
}

/// Walks from outside the form along `-direction` until the field turns
/// negative, and returns that surface point with its outward normal.
fn surface_point(field: &ClayField, direction: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let mut at = std::array::from_fn(|axis| direction[axis] * 3.0);
    for _ in 0..2000 {
        let distance = field.distance(at);
        if distance <= 0.001 {
            break;
        }
        // A sphere trace, capped so a ray that never lands still terminates.
        let step = distance.max(0.002);
        at = std::array::from_fn(|axis| at[axis] - direction[axis] * step);
    }
    (at, field.gradient(at))
}
