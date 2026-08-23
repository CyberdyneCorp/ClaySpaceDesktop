//! What every mesh verb does to a surface, and whether it wrecks it.
//!
//! Written after a side-by-side against Blender's sculpt brushes, driven over
//! MCP on a matched sphere with the same brush radius in world units, the same
//! strength and the same stroke. Blender's brushes leave the surface exactly as
//! smooth as they found it — the mean angle between adjacent vertex normals is
//! unchanged to two decimals for every one of them. Three of ours multiplied it:
//!
//!   verb     before   after   Blender's equivalent
//!   Inflar    1.00x   5.04x        1.00x
//!   Pinçar    1.00x   9.41x        1.00x
//!   Vinco     1.00x   3.71x        1.00x
//!
//! and three more moved nothing at all at any size, intensity or stroke length,
//! where Blender's moved 5% of the mesh. The causes are in the commit; this is
//! the measurement that has to keep holding.
//!
//! ```sh
//! cargo test -p clayspace-app --test visual_mesh_verbs
//! open target/visual
//! ```

mod support;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, GestureSample, Representation, SculptModel, ToolKind,
};
use clayspace_view::{Camera, Vertex};
use support::Harness;

fn meshed() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    document.convert_layer(Direction::SdfToMesh, 0.03, 0).ok()?;
    Some(document)
}

/// Mean angle between the normals of adjacent vertices.
///
/// The same measure computed on the Blender side over its edges, so the two
/// numbers mean the same thing. It reads a surface's *smoothness*: a verb that
/// deposits a swell leaves it where it was, and one that drives neighbouring
/// vertices apart raises it.
fn roughness(positions: &[[f32; 3]], indices: &[u32]) -> f64 {
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    for triangle in indices.chunks_exact(3) {
        let (a, b, c) = (
            positions[triangle[0] as usize],
            positions[triangle[1] as usize],
            positions[triangle[2] as usize],
        );
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let face = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for &i in triangle {
            for (axis, add) in normals[i as usize].iter_mut().zip(face) {
                *axis += add;
            }
        }
    }
    for n in normals.iter_mut() {
        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
        for axis in n.iter_mut() {
            *axis /= length;
        }
    }
    let (mut total, mut count) = (0.0f64, 0usize);
    for triangle in indices.chunks_exact(3) {
        for (a, b) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let (x, y) = (normals[a as usize], normals[b as usize]);
            total += (x[0] * y[0] + x[1] * y[1] + x[2] * y[2])
                .clamp(-1.0, 1.0)
                .acos() as f64;
            count += 1;
        }
    }
    total / count.max(1) as f64
}

/// A drag across the front, every sample picked as the interface picks them.
fn drag(document: &ClayDocument, span: f32, steps: usize) -> Vec<GestureSample> {
    (0..=steps)
        .filter_map(|step| {
            let t = step as f32 / steps as f32;
            let x = -span * 0.5 + t * span;
            SculptModel::pick(document, [x, 0.0, 4.0], [0.0, 0.0, -1.0]).map(|hit| GestureSample {
                position: hit,
                pressure: 1.0,
                time: t,
            })
        })
        .collect()
}

fn capture(harness: &Harness, document: &mut ClayDocument, name: &str) {
    let (positions, normals, colors, indices) = document.visible_mesh_geometry();
    let vertices: Vec<Vertex> = positions
        .into_iter()
        .zip(normals)
        .zip(colors)
        .map(|((position, normal), color)| Vertex {
            position,
            normal,
            color,
        })
        .collect();
    let mut mesh = clayspace_view::GpuMesh::new(&harness.gpu);
    mesh.upload(&harness.gpu, &vertices, &indices);
    let mut camera = Camera::default();
    camera.frame_bounds([-1.5f32; 3].into(), [1.5f32; 3].into());
    harness.capture(&mesh, &camera, false, name);
}

/// The ceiling every mesh verb has to stay under.
///
/// Blender's brushes all sit at 1.00. This is the band ours occupy once the
/// accumulation default, the polish gate and the nudge direction are right —
/// the worst is Pinçar at 1.83, which gathers vertices together and is
/// *supposed* to raise it somewhat.
const ROUGHNESS_CEILING: f64 = 2.0;

#[test]
fn no_mesh_verb_shreds_the_surface() {
    let Some(harness) = Harness::new() else {
        return;
    };
    // The settings a sculptor reaches for to move a whole form.
    let brush = BrushSettings {
        size: 1.073,
        intensity: 0.65,
        ..BrushSettings::default()
    };

    let mut wrecked = Vec::new();
    for tool in ToolKind::for_representation(Representation::Mesh) {
        let Some(mut document) = meshed() else {
            return;
        };
        let samples = drag(&document, 0.9, 12);
        let (before, _, _, before_indices) = document.visible_mesh_geometry();
        let smooth = roughness(&before, &before_indices);

        document
            .apply_stroke(tool, brush, &samples, [false; 3])
            .expect("the verb was refused on a mesh layer");

        let (after, _, _, after_indices) = document.visible_mesh_geometry();
        let ratio = roughness(&after, &after_indices) / smooth.max(1e-9);
        capture(
            &harness,
            &mut document,
            &format!("73-verb-{}", tool.label().to_lowercase()),
        );
        if ratio > ROUGHNESS_CEILING {
            wrecked.push(format!("{} at {ratio:.2}x", tool.label()));
        }
    }
    assert!(
        wrecked.is_empty(),
        "these verbs drove the surface into noise: {}. Blender's equivalents \
         all leave it at 1.00x — see target/visual/73-verb-*.png",
        wrecked.join(", ")
    );
}

/// Every verb that says it moves geometry actually moves some.
///
/// Nudge, Polir and Camada each moved nothing, for three separate reasons, and
/// the shelf offered all three as though they worked. The two colour verbs are
/// excluded: they move no vertex by definition.
#[test]
fn every_geometry_verb_moves_something() {
    let brush = BrushSettings {
        size: 0.5,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    let mut dead = Vec::new();
    for tool in ToolKind::for_representation(Representation::Mesh) {
        if matches!(tool, ToolKind::Pintar | ToolKind::Borrar) {
            continue;
        }
        let Some(mut document) = meshed() else {
            return;
        };
        // Polir smooths what has a corner, so it is given one to work on —
        // the same courtesy a sculptor extends by using it after a crease.
        if tool == ToolKind::Polir {
            let sharp = BrushSettings {
                size: 0.25,
                intensity: 1.0,
                ..BrushSettings::default()
            };
            let cut = drag(&document, 0.9, 16);
            document
                .apply_stroke(ToolKind::Vinco, sharp, &cut, [false; 3])
                .expect("the crease was refused");
        }
        let samples = drag(&document, 0.9, 16);
        let before = document.visible_mesh_geometry().0;
        document
            .apply_stroke(tool, brush, &samples, [false; 3])
            .expect("the verb was refused");
        let after = document.visible_mesh_geometry().0;
        let furthest = before
            .iter()
            .zip(&after)
            .map(|(a, b)| {
                ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt()
            })
            .fold(0.0f32, f32::max);
        if furthest < 1e-4 {
            dead.push(tool.label());
        }
    }
    assert!(
        dead.is_empty(),
        "these verbs are on the shelf and move nothing: {dead:?}"
    );
}
