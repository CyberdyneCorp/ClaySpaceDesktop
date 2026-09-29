//! Application-side release cost after a local dab, against three scene sizes.

use clayspace_app::{Scene, SurfaceGeometry};
use clayspace_engine::BackendPolicy;
use clayspace_model::{GestureSample, SculptModel, ToolKind};
use clayspace_view::Gpu;

use crate::figures::{ms, quantile, Figure};
use crate::groups::headless_gpu;
use crate::run::Run;
use crate::skip::Skip;

struct SettleSamples {
    triangles: usize,
    overheads: Vec<f64>,
    prunes: Vec<f64>,
    uploads: Vec<f64>,
}

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("settle", Skip::NoHeadlessGpu);
    };
    for (name, radius) in [
        ("small", Some(0.42)),
        ("medium", None),
        ("large", Some(1.8)),
    ] {
        let scene = Scene::Reference;
        match scene_cost(&gpu, policy, scene, radius) {
            Ok(SettleSamples {
                triangles,
                mut overheads,
                mut prunes,
                mut uploads,
            }) => {
                overheads.sort_by(f64::total_cmp);
                prunes.sort_by(f64::total_cmp);
                uploads.sort_by(f64::total_cmp);
                let budget = (triangles < 300_000).then_some(30.0);
                let overhead = format!("settle.{name}.overhead.median");
                run.insert(
                    overhead.clone(),
                    Figure::ms(quantile(&overheads, 0.5), budget),
                );
                run.spread(&overhead, &overheads);
                run.insert(
                    format!("settle.{name}.triangles"),
                    Figure::count(triangles as f64),
                );
                run.insert(
                    format!("settle.{name}.prune.median"),
                    Figure::ms(quantile(&prunes, 0.5), None),
                );
                run.insert(
                    format!("settle.{name}.upload.median"),
                    Figure::ms(quantile(&uploads, 0.5), None),
                );
            }
            Err(why) => return run.skip(format!("settle.{name}"), why),
        }
    }
}

fn scene_cost(
    gpu: &Gpu,
    policy: &BackendPolicy,
    scene: Scene,
    radius: Option<f32>,
) -> Result<SettleSamples, Skip> {
    let mut document = match radius {
        Some(radius) => Scene::build_worked_sdf_at_radius(policy.clone(), radius),
        None => scene.build(policy.clone()),
    }
    .map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut geometry = SurfaceGeometry::new(gpu);
    geometry
        .rebuild(gpu, &mut document)
        .map_err(|_| Skip::SurfaceWouldNotMesh)?;
    let triangles = geometry.triangle_count();
    let mut overheads = Vec::new();
    let mut prunes = Vec::new();
    let mut uploads = Vec::new();
    for sample in scene.stroke(12) {
        let position =
            Scene::probe_point(&document, sample.position).ok_or(Skip::NoSurfaceUnderProbe)?;
        document
            .apply_stroke(
                ToolKind::Padrao,
                Scene::probe_brush(),
                &[GestureSample { position, ..sample }],
                [false; 3],
            )
            .map_err(|_| Skip::EditRefused)?;
        geometry
            .sync(gpu, &mut document)
            .map_err(|_| Skip::SurfaceWouldNotMesh)?;
        geometry
            .settle_after_edit(gpu, &mut document)
            .map_err(|_| Skip::SurfaceWouldNotMesh)?;
        let cost = geometry.last_settle().expect("the release ran");
        overheads.push(ms(cost.total_time.saturating_sub(cost.engine_mesh_time)));
        prunes.push(ms(cost.prune_time));
        uploads.push(ms(cost.upload_time));
    }
    Ok(SettleSamples {
        triangles,
        overheads,
        prunes,
        uploads,
    })
}
