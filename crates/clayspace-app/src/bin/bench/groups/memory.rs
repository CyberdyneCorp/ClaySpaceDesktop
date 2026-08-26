//! Brick cache memory across repeated open, sculpt and close cycles.

use clayspace_app::Scene;
use clayspace_engine::BackendPolicy;
use clayspace_model::{SculptModel, ToolKind};

use crate::figures::Figure;
use crate::run::Run;
use crate::skip::Skip;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let mut after_each = Vec::new();
    let mut peak = 0u64;

    for _ in 0..3 {
        let Ok(mut document) = Scene::Reference.build(policy.clone()) else {
            return run.skip("memory", Skip::SceneWouldNotBuild);
        };
        let brush = Scene::Reference.brush();
        for sample in Scene::Reference.stroke(12) {
            if document
                .apply_stroke(ToolKind::Padrao, brush, &[sample], [false; 3])
                .is_err()
            {
                return run.skip("memory", Skip::EditRefused);
            }
        }
        if let Ok(stats) = document.cache().stats() {
            peak = peak.max(stats.memory_usage);
            run.insert_once("memory.budget", || {
                Figure::mb(stats.memory_budget.unwrap_or(0) as f64 / 1_048_576.0)
            });
        }
        drop(document);
        // What a fresh document costs, as the floor a cycle should return to.
        if let Ok(fresh) = Scene::Reference.build(policy.clone()) {
            if let Ok(stats) = fresh.cache().stats() {
                after_each.push(stats.memory_usage as f64 / 1_048_576.0);
            }
        }
    }

    run.insert("memory.peak", Figure::mb(peak as f64 / 1_048_576.0));
    let (Some(first), Some(last)) = (after_each.first(), after_each.last()) else {
        return run.skip("memory.baseline", Skip::CacheUnreadable);
    };
    run.insert("memory.baseline", Figure::mb(*first));
    // A cycle that does not return to its floor is a leak. Stated as a ratio
    // so the gate does not depend on the absolute figure.
    run.insert(
        "memory.drift",
        Figure::ratio(last / first.max(f64::MIN_POSITIVE), Some(1.10), 1.05),
    );
}
