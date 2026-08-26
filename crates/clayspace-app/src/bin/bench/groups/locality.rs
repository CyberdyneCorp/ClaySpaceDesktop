//! The same dab on the reference scene and on one ten times its area.
//!
//! The requirement is that the work follows the edit rather than the document,
//! so what matters is the ratio, not either figure.

use std::time::Instant;

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_model::{GestureSample, SculptModel, ToolKind};
use clayspace_view::Gpu;

use crate::figures::{ms, Figure};
use crate::groups::headless_gpu;
use crate::run::Run;
use crate::skip::Skip;

/// What one scene's probe dab cost, and what it touched.
struct Probe {
    surface_bricks: usize,
    keys_remeshed: usize,
    dab_ms: f64,
}

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("locality", Skip::NoHeadlessGpu);
    };

    let mut measured = Vec::new();
    for scene in [Scene::Reference, Scene::TenTimesLarger] {
        match probe(&gpu, policy, scene) {
            Ok(probe) => measured.push(probe),
            Err(why) => return run.skip("locality", why),
        }
    }
    let Ok([small, large]) = <[Probe; 2]>::try_from(measured) else {
        return run.skip("locality", Skip::SceneWouldNotBuild);
    };

    run.insert(
        "locality.surface_bricks",
        Figure::count(small.surface_bricks as f64),
    );
    run.insert(
        "locality.surface_bricks_10x",
        Figure::count(large.surface_bricks as f64),
    );
    run.insert(
        "locality.keys_remeshed",
        Figure::count(small.keys_remeshed as f64),
    );
    run.insert(
        "locality.keys_remeshed_10x",
        Figure::count(large.keys_remeshed as f64),
    );
    // The claim, as one number: a dab on the larger scene should re-mesh
    // roughly what it re-meshes on the smaller one. Budgeted at 2, which
    // leaves room for the brush covering a different number of bricks at the
    // larger radius without leaving room for scaling with the document.
    let ratio = large.keys_remeshed as f64 / small.keys_remeshed.max(1) as f64;
    run.insert("locality.key_ratio", Figure::ratio(ratio, Some(2.0), 1.5));
    run.insert("locality.dab_ms", Figure::ms(small.dab_ms, None));
    run.insert("locality.dab_ms_10x", Figure::ms(large.dab_ms, None));
}

/// One probe dab on one scene.
///
/// The same edit on both, not a proportional one — see `Scene::probe_brush`.
/// Placed where the cache keeps a surface rather than at the scene's own
/// coordinates, which land under the surface on the larger scene — see
/// `Scene::probe_point`.
fn probe(gpu: &Gpu, policy: &BackendPolicy, scene: Scene) -> Result<Probe, Skip> {
    let mut document = scene
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut geometry = SurfaceGeometry::new(gpu);
    geometry
        .rebuild(gpu, &mut document)
        .map_err(|_| Skip::SurfaceWouldNotMesh)?;
    let surface_bricks = document
        .cache()
        .surface_bricks()
        .map(|keys| keys.len())
        .unwrap_or(0);

    let sample = scene.stroke(3)[1];
    let position =
        Scene::probe_point(&document, sample.position).ok_or(Skip::NoSurfaceUnderProbe)?;

    let started = Instant::now();
    document
        .apply_stroke(
            ToolKind::Padrao,
            Scene::probe_brush(),
            &[GestureSample { position, ..sample }],
            [false; 3],
        )
        .map_err(|_| Skip::EditRefused)?;
    let cost = geometry
        .sync(gpu, &mut document)
        .map_err(|_| Skip::SurfaceWouldNotMesh)?;

    Ok(Probe {
        surface_bricks,
        keys_remeshed: cost.map(|c| c.keys).unwrap_or(0),
        dab_ms: ms(started.elapsed()),
    })
}
