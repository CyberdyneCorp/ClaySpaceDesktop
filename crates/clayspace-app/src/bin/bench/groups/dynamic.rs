//! What a Dynamic stroke costs to draw (#209): the steady dab, and the bytes
//! it sends.
//!
//! # The subject
//!
//! A flat quad sheet crossed into an adaptive surface, at two sizes of the
//! same density — 100,352 and 1,002,528 triangles — so the only difference
//! between them is how much surface the brush did *not* touch. Written here,
//! as the `multires` group writes its cage and for its reason: a `Scene`
//! member would enter `conditions.scenes` and stop every committed baseline
//! comparing on the day it landed.
//!
//! # The figures
//!
//! - `dynamic.dab_<size>`: one dab of an open stroke and the surface
//!   arriving, as the application draws it — the chunks it dirtied written in
//!   place. Inside one gesture, as a drag is: the first segment of a gesture
//!   also serializes the whole surface as its undo record (the bounded
//!   snapshot #210 replaces with the engine's reversible delta), and that is
//!   the gesture's cost rather than the steady dab's, so it is taken before
//!   the clock.
//!   Held to the frame budget on the 100,352-triangle fixture; reported on
//!   the large one, whose stamp is the engine's cost and grew with the
//!   surface when measured (10.5 ms against 15.1 ms mean on the same spots).
//! - `dynamic.upload_<size>`: the drawing half alone, the patch built and
//!   written, held to the frame budget at both sizes. This is the part this
//!   application owns.
//! - `dynamic.upload_kb_<size>`: what one dab sent, in kilobytes.
//! - `dynamic.upload_scaling`: the large sheet's bytes per dab against the
//!   small one's. Proportional to the dirty chunks is near one; proportional
//!   to the model would be near ten.

use std::time::Instant;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ConversionSettings, Direction, ExchangeModel, GestureSample, ImportSettings,
    Representation, SceneModel, SculptModel, ToolKind,
};
use clayspace_view::Gpu;

use crate::figures::{mean, ms, Figure, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

/// What the specification allows the interface thread for a frame.
const FRAME_BUDGET_MS: f64 = 16.0;

/// How far the large sheet's bytes per dab may exceed the small one's.
const SCALING_BUDGET: f64 = 2.0;

/// Spacing of the sheet's vertices, the same at both sizes.
const STEP: f32 = 4.0 / 224.0;

/// Quads per side, the name each size's figures carry, and the budget its
/// whole dab is held to.
const SIZES: [(usize, &str, Option<f64>); 2] =
    [(224, "100k", Some(FRAME_BUDGET_MS)), (708, "1m", None)];

/// A dab a few dozen chunks wide.
const DAB_RADIUS: f32 = 0.3;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("dynamic", Skip::NoHeadlessGpu);
    };
    let mut sent = Vec::new();
    for (divisions, name, dab_budget) in SIZES {
        match dabs(&gpu, policy, divisions) {
            Ok(measured) => {
                record(
                    run,
                    &format!("dynamic.dab_{name}"),
                    &measured.dab,
                    dab_budget,
                );
                record(
                    run,
                    &format!("dynamic.upload_{name}"),
                    &measured.upload,
                    Some(FRAME_BUDGET_MS),
                );
                run.insert(
                    format!("dynamic.upload_kb_{name}"),
                    Figure::count(mean(&measured.bytes) / 1024.0),
                );
                sent.push(mean(&measured.bytes));
            }
            Err(why) => run.skip(format!("dynamic.dab_{name}"), why),
        }
    }
    if let [small, large] = sent.as_slice() {
        run.insert(
            "dynamic.upload_scaling",
            Figure::ratio(large / small.max(1.0), Some(SCALING_BUDGET), 1.5),
        );
    }
}

/// One size's samples.
struct Dabs {
    dab: Vec<f64>,
    upload: Vec<f64>,
    bytes: Vec<f64>,
}

/// A repeatable timing's figures, with a budget attached where one holds.
fn record(run: &mut Run, prefix: &str, samples: &[f64], budget: Option<f64>) {
    for (label, figure) in Record::Repeatable.figures(prefix, samples) {
        run.spread(&label, samples);
        run.insert(label, Figure { budget, ..figure });
    }
}

fn dabs(gpu: &Gpu, policy: &BackendPolicy, divisions: usize) -> Result<Dabs, Skip> {
    let mut document = surface(policy, divisions)?;
    let mut screen = Screen::new(gpu);
    screen.refresh(gpu, &mut document)?;
    document.begin_gesture();
    // One segment before the clock: it takes the gesture's undo record, and
    // the first drain after the first drawing sizes the scratch it reuses.
    dab(&mut document, along(0, 1))?;
    screen.refresh(gpu, &mut document)?;

    let count = Record::Repeatable.samples();
    let mut measured = Dabs {
        dab: Vec::with_capacity(count),
        upload: Vec::with_capacity(count),
        bytes: Vec::with_capacity(count),
    };
    for n in 0..count {
        let started = Instant::now();
        dab(&mut document, along(n, count))?;
        let drawing = Instant::now();
        screen.refresh(gpu, &mut document)?;
        measured.upload.push(ms(drawing.elapsed()));
        measured.dab.push(ms(started.elapsed()));
        measured.bytes.push(screen.adaptive_bytes() as f64);
    }
    document.end_gesture();
    Ok(measured)
}

/// The sheet, imported and crossed into an adaptive surface in place.
fn surface(policy: &BackendPolicy, divisions: usize) -> Result<ClayDocument, Skip> {
    let path = std::env::temp_dir().join(format!(
        "clayspace-bench-dynamic-{divisions}-{}.obj",
        std::process::id()
    ));
    std::fs::write(&path, sheet(divisions)).map_err(|_| Skip::SceneWouldNotBuild)?;
    let mut document = ClayDocument::new(policy.clone()).map_err(|_| Skip::SceneWouldNotBuild)?;
    let imported = document.import_mesh(&path, ImportSettings::default());
    let _ = std::fs::remove_file(&path);
    imported.map_err(|_| Skip::SceneWouldNotBuild)?;
    let mesh = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .ok_or(Skip::SceneWouldNotBuild)?;
    document
        .set_active_layer(mesh)
        .map_err(|_| Skip::EditRefused)?;
    let settings = ConversionSettings::default();
    document
        .convert_layer_in_place(Direction::MeshToDynamic, settings.cell_size, settings.blur)
        .map_err(|_| Skip::EditRefused)?;
    Ok(document)
}

/// A flat quad sheet facing +y, as OBJ text.
fn sheet(divisions: usize) -> String {
    use std::fmt::Write;
    let half = STEP * divisions as f32 / 2.0;
    let mut text = String::new();
    for z in 0..=divisions {
        for x in 0..=divisions {
            let _ = writeln!(
                text,
                "v {} 0 {}",
                -half + STEP * x as f32,
                -half + STEP * z as f32
            );
        }
    }
    let stride = divisions + 1;
    for z in 0..divisions {
        for x in 0..divisions {
            let a = z * stride + x + 1;
            let _ = writeln!(text, "f {} {} {} {}", a, a + stride, a + stride + 1, a + 1);
        }
    }
    text
}

/// One Draw dab, a segment of the gesture the caller holds open.
fn dab(document: &mut ClayDocument, at: [f32; 3]) -> Result<(), Skip> {
    let applied = document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size: DAB_RADIUS,
            intensity: 0.8,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: at,
            pressure: 1.0,
            time: 0.0,
        }],
        [false; 3],
    );
    applied.map(|_| ()).map_err(|_| Skip::EditRefused)
}

/// Where the `n`th dab lands: a ring inside the small sheet, so both sizes
/// are dabbed on the same spots.
fn along(n: usize, of: usize) -> [f32; 3] {
    let angle = n as f32 / of.max(1) as f32 * std::f32::consts::TAU;
    [angle.cos() * 1.0, 0.0, angle.sin() * 1.0]
}
