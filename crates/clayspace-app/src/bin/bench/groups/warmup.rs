//! Bringing the machine to the state the figures are taken in.
//!
//! Not a measurement. It exists because the first figures of a run were being
//! taken while the GPU was still ramping its clocks, and what that costs is
//! larger than anything this gate is meant to detect: measured on an RTX 5060,
//! `brush.sdf.inflar.median` read 11.9 ms starting from an idle 330 MHz and
//! 7.8 ms when a previous run had left the card boosted — a 55 % swing with
//! nothing changed but what the machine had been doing beforehand. A baseline
//! recorded in the second state fails every run taken in the first.
//!
//! So every run does the same work before it records anything. It is discarded
//! and it is not optional: a figure taken on a cold card and compared against
//! one taken on a hot card is the same mistake as comparing two machines,
//! which this gate already refuses to do.

use std::time::{Duration, Instant};

use clayspace_app::Scene;
use clayspace_engine::BackendPolicy;
use clayspace_model::{SculptModel, ToolKind};

use crate::groups::headless_gpu;
use crate::groups::visible::Screen;

/// Long enough for a boosting card to settle, short beside a five-minute run.
const FOR: Duration = Duration::from_secs(5);

/// Sculpts the reference scene until `FOR` has passed, discarding everything.
///
/// Silent about failure on purpose: everything it could fail at, the groups
/// that follow fail at too, and they say so with a reason. A warm-up that
/// reported its own troubles would report each of them twice.
pub fn run(policy: &BackendPolicy) {
    let Some(gpu) = headless_gpu() else {
        return;
    };
    let Ok(mut document) = Scene::Reference.build(policy.clone()) else {
        return;
    };
    let mut screen = Screen::new(&gpu);
    if screen.prime(&gpu, &mut document).is_err() {
        return;
    }

    let brush = Scene::Reference.brush();
    let started = Instant::now();
    while started.elapsed() < FOR {
        for sample in Scene::Reference.stroke(12) {
            if document
                .apply_stroke(ToolKind::Padrao, brush, &[sample], [true, false, false])
                .is_err()
            {
                return;
            }
            if screen.refresh(&gpu, &mut document).is_err() {
                return;
            }
        }
    }
}
