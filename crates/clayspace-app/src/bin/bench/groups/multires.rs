//! What a subdivision hierarchy costs: to build, to deepen, to sculpt, and to
//! keep a stack of passes on.
//!
//! # Why this group builds its own subject
//!
//! Every other group takes a `Scene` member, and this one deliberately does
//! not. `Scene::for_representation(Representation::Multires)` answers `None`,
//! and `reference.rs` says why in its own words: a member is a *subject* with
//! a recorded size and a revision, it goes into `conditions.scenes`, and
//! `compare::unlike` refuses on the first scene whose revision it does not
//! recognise — so adding one would stop every committed baseline comparing on
//! the day it landed. That is the wrong trade for a group whose figures nobody
//! has a baseline for yet.
//!
//! So the cage is written here, exactly as `clayspace-engine/tests/multires.rs`
//! writes its own: a flat grid of quads, saved as an `.obj` and imported,
//! because importing a file is the only route a mesh layer has into a document
//! and a fixture taking another one would measure a path no sculptor reaches.
//! The reader triangulates the quads on the way in, which is fine — the
//! subdivision rule is defined over faces of any arity.
//!
//! # What the sizes are, and why
//!
//! [`DIVISIONS`] squared quads at the cage, [`LEVELS`] levels above it. Four
//! levels over a sixteen-by-sixteen cage lands at roughly a hundred thousand
//! quads, which is the same order as `mesh-reference`'s 296,216 triangles —
//! deliberately, so that a hierarchy figure and a mesh figure can be read
//! against each other rather than being a small thing beside a large one.
//!
//! # Two figures that are timed differently from the rest, on purpose
//!
//! `multires.reorder` times **the operation alone**, with no surface refresh
//! after it. The engine defines a reorder as moving no vertex — the passes are
//! additive, so a sum is a sum whatever order it is written in, and
//! `claycore/tests/multires.rs` measures that over three hundred randomised
//! stacks rather than taking it on trust. This application drops its drawn
//! copy on *any* stack operation, so a figure that included the refresh would
//! price that policy and not the reorder, and the one claim worth guarding
//! here would be invisible underneath it. `multires.compose` next to it is the
//! same shape of operation *with* the refresh, because a strength change does
//! move the surface and what a sculptor pays for a slider is the surface
//! arriving.
//!
//! `multires.drop_caches` is the release **and the dab after it**, together.
//! The release on its own is close to free; what it costs is the level the
//! next stamp has to rebuild before it can write into it, and a figure for the
//! free half would be a figure for nothing. Read it against
//! `multires.stamp.mean`, which is the same dab with the caches still warm.

use std::time::Instant;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ConversionSettings, Direction, DocumentModel, ExchangeModel, GestureSample,
    ImportSettings, LayerKey, MultiresLevelOp, MultiresSculptLayerId, MultiresSculptLayerOp,
    Representation, SceneModel, SculptModel, ToolKind,
};
use clayspace_view::Gpu;

use crate::figures::{ms, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

/// Quads along each side of the cage.
const DIVISIONS: usize = 16;

/// How far the cage reaches from its centre.
const HALF: f32 = 2.0;

/// How deep the fixture is subdivided.
///
/// Four, because each level multiplies faces by four and the fourth is what
/// takes this subject to the same order of magnitude as the mesh member. It is
/// also the level `add_level` is timed *arriving at*, which is the expensive
/// one and the one a sculptor hesitates over.
const LEVELS: u32 = 4;

/// A dab wide enough to reach a few thousand vertices at the finest level, and
/// narrow enough that it is a dab rather than a whole-form deformation.
const DAB_RADIUS: f32 = 0.6;

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("multires", Skip::NoHeadlessGpu);
    };
    let cage = match write_cage() {
        Ok(cage) => cage,
        Err(_) => return run.skip("multires", Skip::SceneWouldNotBuild),
    };

    from_mesh(&cage, policy, run);
    add_level(&cage, policy, run);
    stamp(&gpu, &cage, policy, run);
    compose(&gpu, &cage, policy, run);
    reorder(&cage, policy, run);
    fold(&gpu, &cage, policy, run);
    serialize(&gpu, &cage, policy, run);
    drop_caches(&gpu, &cage, policy, run);

    let _ = std::fs::remove_file(&cage);
}

// -- the fixture -------------------------------------------------------------

/// Writes the cage this group builds every hierarchy from.
///
/// Once, and reused: the file is the same every time, and rewriting it per
/// sample would put a disk in the middle of a benchmark that is not measuring
/// one.
fn write_cage() -> std::io::Result<std::path::PathBuf> {
    let path =
        std::env::temp_dir().join(format!("clayspace-bench-cage-{}.obj", std::process::id()));
    let mut text = String::new();
    let step = 2.0 * HALF / DIVISIONS as f32;
    for z in 0..=DIVISIONS {
        for x in 0..=DIVISIONS {
            text.push_str(&format!(
                "v {} 0 {}\n",
                -HALF + step * x as f32,
                -HALF + step * z as f32
            ));
        }
    }
    let stride = DIVISIONS + 1;
    for z in 0..DIVISIONS {
        for x in 0..DIVISIONS {
            // Wound so the sheet faces +y, which makes a Draw stamp read as a
            // bump rather than a dent.
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
    std::fs::write(&path, text)?;
    Ok(path)
}

/// A document whose only layer is the cage, as a mesh layer, activated.
fn with_the_cage(cage: &std::path::Path, policy: &BackendPolicy) -> Result<ClayDocument, Skip> {
    let mut document = ClayDocument::new(policy.clone()).map_err(|_| Skip::SceneWouldNotBuild)?;
    document
        .import_mesh(cage, ImportSettings::default())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
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
    Ok(document)
}

/// The same cage, crossed into a hierarchy `levels` deep.
fn with_a_hierarchy(
    cage: &std::path::Path,
    policy: &BackendPolicy,
    levels: u32,
) -> Result<(ClayDocument, LayerKey), Skip> {
    let mut document = with_the_cage(cage, policy)?;
    let settings = ConversionSettings::default();
    let key = document
        .convert_layer_in_place(Direction::MeshToMultires, settings.cell_size, settings.blur)
        .map_err(|_| Skip::EditRefused)?;
    for _ in 0..levels {
        document
            .apply_multires_level_op(MultiresLevelOp::AddLevel)
            .map_err(|_| Skip::EditRefused)?;
    }
    Ok((document, key))
}

/// One dab at the level the brush is bound to.
fn dab(document: &mut ClayDocument, at: [f32; 3]) -> Result<(), Skip> {
    document.begin_gesture();
    let applied = document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size: DAB_RADIUS,
            intensity: 1.0,
            ..BrushSettings::default()
        },
        &[GestureSample {
            position: at,
            pressure: 1.0,
            time: 0.0,
        }],
        // Unmirrored. A mirrored dab is two stamps, and every figure here is
        // about what one costs.
        [false; 3],
    );
    document.end_gesture();
    applied.map(|_| ()).map_err(|_| Skip::EditRefused)
}

/// Where the `n`th dab of a sweep lands, walking the sheet rather than
/// stacking on one spot.
fn along(n: usize, of: usize) -> [f32; 3] {
    let t = n as f32 / of.max(1) as f32;
    let angle = t * std::f32::consts::TAU;
    let reach = HALF * 0.55;
    [angle.cos() * reach, 0.0, angle.sin() * reach]
}

/// Adds a pass and answers its id.
fn add_pass(
    document: &mut ClayDocument,
    key: LayerKey,
    name: &str,
) -> Result<MultiresSculptLayerId, Skip> {
    document
        .apply_multires_sculpt_layer_op(MultiresSculptLayerOp::Add {
            name: name.to_string(),
        })
        .map_err(|_| Skip::EditRefused)?;
    passes(document, key)
        .last()
        .copied()
        .ok_or(Skip::EditRefused)
}

/// The stack, bottom-first, as the layer row would draw it.
fn passes(document: &ClayDocument, key: LayerKey) -> Vec<MultiresSculptLayerId> {
    document
        .scene()
        .layer(key)
        .and_then(|layer| layer.multires.as_ref())
        .map(|state| state.sculpt_layers.iter().map(|pass| pass.id).collect())
        .unwrap_or_default()
}

// -- building and deepening --------------------------------------------------

/// Crossing a cage into a hierarchy.
///
/// One-shot on a document rebuilt per sample: the crossing consumes the mesh
/// layer it read, so a second one has nothing to cross.
fn from_mesh(cage: &std::path::Path, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("multires.from_mesh") {
        return;
    }
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| {
            let mut document = with_the_cage(cage, policy)?;
            let settings = ConversionSettings::default();
            let started = Instant::now();
            document
                .convert_layer_in_place(
                    Direction::MeshToMultires,
                    settings.cell_size,
                    settings.blur,
                )
                .map_err(|_| Skip::EditRefused)?;
            Ok(ms(started.elapsed()))
        })
        .collect();
    match samples {
        Ok(samples) => run.timings("multires.from_mesh", Record::OneShot, samples),
        Err(why) => run.skip("multires.from_mesh", why),
    }
}

/// One more level, priced and then taken.
///
/// The *last* level rather than the first, which is the one worth a figure: a
/// level multiplies faces by four, so the fourth costs as much as the three
/// under it together and it is the one a sculptor hesitates over. The
/// hierarchy is rebuilt to `LEVELS - 1` per sample, because a level added is a
/// level that stays.
fn add_level(cage: &std::path::Path, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("multires.add_level") {
        return;
    }
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| {
            let (mut document, _) = with_a_hierarchy(cage, policy, LEVELS - 1)?;
            let started = Instant::now();
            document
                .apply_multires_level_op(MultiresLevelOp::AddLevel)
                .map_err(|_| Skip::EditRefused)?;
            Ok(ms(started.elapsed()))
        })
        .collect();
    match samples {
        Ok(samples) => run.timings("multires.add_level", Record::OneShot, samples),
        Err(why) => run.skip("multires.add_level", why),
    }
}

// -- sculpting ---------------------------------------------------------------

/// A dab at the sculpt level, and the same dab into a pass.
///
/// Measured as a pair on the same subject and in the same function, because
/// the difference between them is the whole question: what does routing a
/// stroke into a keepable pass cost over writing it into the form? Anything
/// that moved both would move both figures and leave the difference where it
/// was.
fn stamp(gpu: &Gpu, cage: &std::path::Path, policy: &BackendPolicy, run: &mut Run) {
    for (prefix, into_a_pass) in [("multires.stamp", false), ("multires.pass_stroke", true)] {
        if !run.wants_group(prefix) {
            continue;
        }
        match dabs(gpu, cage, policy, into_a_pass) {
            Ok(samples) => run.timings(prefix, Record::Repeatable, samples),
            Err(why) => run.skip(prefix, why),
        }
    }
}

fn dabs(
    gpu: &Gpu,
    cage: &std::path::Path,
    policy: &BackendPolicy,
    into_a_pass: bool,
) -> Result<Vec<f64>, Skip> {
    let (mut document, key) = with_a_hierarchy(cage, policy, LEVELS)?;
    if into_a_pass {
        add_pass(&mut document, key, "Rugas")?;
    }
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;
    // One dab before the clock. Priming draws the display level, which is not
    // the same as writing to the sculpt level: the storage a stamp writes into
    // comes into being on the first stamp, and measured, that made the first
    // of twelve samples 112 ms against the eleven after it at 11 to 13. That
    // is a real cost and it has its own figure — `multires.drop_caches`
    // releases exactly this and pays for it again — so leaving it in here
    // would be counting it twice and would put an outlier in the mean of a
    // figure meant to describe a dab in the middle of a session.
    dab(&mut document, along(0, 1))?;
    screen.refresh(gpu, &mut document)?;

    let count = Record::Repeatable.samples();
    (0..count)
        .map(|n| {
            let started = Instant::now();
            dab(&mut document, along(n, count))?;
            screen.refresh(gpu, &mut document)?;
            Ok(ms(started.elapsed()))
        })
        .collect()
}

// -- the stack ---------------------------------------------------------------

/// Moving a pass's slider, and the surface arriving.
///
/// The composition, which is the thing a stack of passes is for: a strength
/// change replays no stroke and re-evaluates the sum. The refresh is inside
/// the clock because until the surface arrives nothing has happened that a
/// sculptor can see.
fn compose(gpu: &Gpu, cage: &std::path::Path, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("multires.compose") {
        return;
    }
    match strengths(gpu, cage, policy) {
        Ok(samples) => run.timings("multires.compose", Record::Repeatable, samples),
        Err(why) => run.skip("multires.compose", why),
    }
}

fn strengths(gpu: &Gpu, cage: &std::path::Path, policy: &BackendPolicy) -> Result<Vec<f64>, Skip> {
    let (mut document, key) = with_a_hierarchy(cage, policy, LEVELS)?;
    let pass = add_pass(&mut document, key, "Rugas")?;
    dab(&mut document, along(0, 4))?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let count = Record::Repeatable.samples();
    (0..count)
        .map(|n| {
            // Never the same value twice running: a strength set to what it
            // already is would be an operation the engine can answer without
            // touching the sum.
            let strength = 0.25 + 0.75 * (n % 4) as f32 / 3.0;
            let started = Instant::now();
            document
                .apply_multires_sculpt_layer_op(MultiresSculptLayerOp::SetStrength {
                    id: pass,
                    strength,
                })
                .map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, &mut document)?;
            Ok(ms(started.elapsed()))
        })
        .collect()
}

/// Sliding a pass through the stack, which must stay free.
///
/// The figure exists to notice it stopping being free. Upstream defines a
/// reorder as moving no vertex and fixed a release in which 158 of 300
/// randomised stacks did move, so the property is real and it was not free —
/// which is exactly the kind of thing that comes back. See the module note for
/// why there is no refresh inside this clock.
fn reorder(cage: &std::path::Path, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("multires.reorder") {
        return;
    }
    match slides(cage, policy) {
        Ok(samples) => run.timings("multires.reorder", Record::Repeatable, samples),
        Err(why) => run.skip("multires.reorder", why),
    }
}

fn slides(cage: &std::path::Path, policy: &BackendPolicy) -> Result<Vec<f64>, Skip> {
    let (mut document, key) = with_a_hierarchy(cage, policy, LEVELS)?;
    // Five, so a slide has somewhere to go and the renumbering it would cause
    // on an index-addressed stack has room to be wrong in.
    for n in 0..5 {
        let pass = add_pass(&mut document, key, &format!("Passe {n}"))?;
        document
            .apply_multires_sculpt_layer_op(MultiresSculptLayerOp::SetActive { id: pass })
            .map_err(|_| Skip::EditRefused)?;
        dab(&mut document, along(n, 5))?;
    }
    let stack = passes(&document, key);
    let top = *stack.last().ok_or(Skip::EditRefused)?;
    let bottom = *stack.first().ok_or(Skip::EditRefused)?;

    let count = Record::Repeatable.samples();
    (0..count)
        .map(|n| {
            // Alternated end to end, so every sample moves a pass past the
            // whole stack rather than one place.
            let (id, to) = if n % 2 == 0 { (top, 0) } else { (bottom, 4) };
            let started = Instant::now();
            document
                .apply_multires_sculpt_layer_op(MultiresSculptLayerOp::Move { id, to })
                .map_err(|_| Skip::EditRefused)?;
            Ok(ms(started.elapsed()))
        })
        .collect()
}

/// Folding a pass away: into the one below it, and into the form itself.
///
/// The two together, because they are the same shape of operation with
/// different targets and the pair is what says whether the base is a special
/// case. Both are defined by visual parity — the surface after equals the
/// surface before — so the refresh is outside the clock, as it is in the
/// `bake` group next door: it is there to catch a failure to re-mesh, not to
/// be timed.
fn fold(gpu: &Gpu, cage: &std::path::Path, policy: &BackendPolicy, run: &mut Run) {
    for (prefix, to_base) in [
        ("multires.merge_down", false),
        ("multires.bake_to_base", true),
    ] {
        if !run.wants_group(prefix) {
            continue;
        }
        let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
            .map(|_| folded(gpu, cage, policy, to_base))
            .collect();
        match samples {
            Ok(samples) => run.timings(prefix, Record::OneShot, samples),
            Err(why) => run.skip(prefix, why),
        }
    }
}

fn folded(
    gpu: &Gpu,
    cage: &std::path::Path,
    policy: &BackendPolicy,
    to_base: bool,
) -> Result<f64, Skip> {
    let (mut document, key) = with_a_hierarchy(cage, policy, LEVELS)?;
    // Two passes either way. A merge needs one to fold into; a bake does not,
    // but giving it the same stack keeps the pair a pair.
    for n in 0..2 {
        let pass = add_pass(&mut document, key, &format!("Passe {n}"))?;
        document
            .apply_multires_sculpt_layer_op(MultiresSculptLayerOp::SetActive { id: pass })
            .map_err(|_| Skip::EditRefused)?;
        dab(&mut document, along(n, 2))?;
    }
    let top = *passes(&document, key).last().ok_or(Skip::EditRefused)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let op = if to_base {
        MultiresSculptLayerOp::BakeToBase { id: top }
    } else {
        MultiresSculptLayerOp::MergeDown { id: top }
    };
    let started = Instant::now();
    document
        .apply_multires_sculpt_layer_op(op)
        .map_err(|_| Skip::EditRefused)?;
    let took = ms(started.elapsed());
    screen.refresh(gpu, &mut document)?;
    Ok(took)
}

// -- carrying it out of the session ------------------------------------------

/// Saving a document that holds a hierarchy.
///
/// The whole save, not the encode alone, and that is the honest figure: a
/// `.clayspace` carries a hierarchy's cage and nothing standing on it, so the
/// sculpt travels in a side-car written inside the same call and there is no
/// route a sculptor takes that pays for one without the other. What the figure
/// is for is the autosave clock — this runs on the interface thread every two
/// minutes — so what matters is the total.
fn serialize(gpu: &Gpu, cage: &std::path::Path, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("multires.serialize") {
        return;
    }
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| saved(gpu, cage, policy))
        .collect();
    match samples {
        Ok(samples) => run.timings("multires.serialize", Record::OneShot, samples),
        Err(why) => run.skip("multires.serialize", why),
    }
}

fn saved(gpu: &Gpu, cage: &std::path::Path, policy: &BackendPolicy) -> Result<f64, Skip> {
    let (mut document, key) = with_a_hierarchy(cage, policy, LEVELS)?;
    let pass = add_pass(&mut document, key, "Rugas")?;
    document
        .apply_multires_sculpt_layer_op(MultiresSculptLayerOp::SetActive { id: pass })
        .map_err(|_| Skip::EditRefused)?;
    dab(&mut document, along(0, 2))?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let path = std::env::temp_dir().join(format!(
        "clayspace-bench-hierarchy-{}.clayspace",
        std::process::id()
    ));
    let started = Instant::now();
    let written = document.save(&path).map_err(|_| Skip::EditRefused);
    let took = ms(started.elapsed());
    let _ = std::fs::remove_file(&path);
    // Two side-cars beside every saved document: the object table and the
    // hierarchies. Both are files, and both are taken away with the document
    // so a second sample does not find one waiting.
    let _ = std::fs::remove_file(clayspace_engine::multires::sidecar_for(&path));
    let _ = std::fs::remove_file(clayspace_engine::objects::sidecar_for(&path));
    written.map(|()| took)
}

/// Releasing the rebuildable level caches, and the dab that pays for it.
///
/// The host's answer to memory pressure for this representation: a level cache
/// is derived, so releasing one costs time and no work. The time is all in
/// what comes next, which is why the dab is inside the clock — and why this is
/// the figure to read against `multires.stamp.mean`, the same dab with the
/// caches still warm.
fn drop_caches(gpu: &Gpu, cage: &std::path::Path, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("multires.drop_caches") {
        return;
    }
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| released(gpu, cage, policy))
        .collect();
    match samples {
        Ok(samples) => run.timings("multires.drop_caches", Record::OneShot, samples),
        Err(why) => run.skip("multires.drop_caches", why),
    }
}

fn released(gpu: &Gpu, cage: &std::path::Path, policy: &BackendPolicy) -> Result<f64, Skip> {
    let (mut document, _) = with_a_hierarchy(cage, policy, LEVELS)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let started = Instant::now();
    document
        .release_hierarchy_caches()
        .map_err(|_| Skip::EditRefused)?;
    dab(&mut document, along(0, 1))?;
    screen.refresh(gpu, &mut document)?;
    Ok(ms(started.elapsed()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cage has to be a grid of quads, because a Catmull-Clark cage is
    /// one and `clay_multires_from_mesh` refuses rather than repairs.
    #[test]
    fn the_cage_is_a_closed_grid_of_quads() {
        let path = write_cage().expect("the cage is written");
        let text = std::fs::read_to_string(&path).expect("the cage is read back");
        let _ = std::fs::remove_file(&path);
        let vertices = text.lines().filter(|line| line.starts_with("v ")).count();
        let faces: Vec<&str> = text.lines().filter(|line| line.starts_with("f ")).collect();
        assert_eq!(vertices, (DIVISIONS + 1) * (DIVISIONS + 1));
        assert_eq!(faces.len(), DIVISIONS * DIVISIONS);
        for face in faces {
            assert_eq!(
                face.split_whitespace().count(),
                5,
                "a quad is four corners and the keyword: {face}"
            );
        }
    }

    /// The sweep walks the sheet rather than stacking every dab on one spot,
    /// and stays inside the cage — a dab off the edge moves nothing and would
    /// measure an empty stamp.
    #[test]
    fn the_sweep_stays_on_the_sheet_and_does_not_stack() {
        let count = Record::Repeatable.samples();
        let points: Vec<[f32; 3]> = (0..count).map(|n| along(n, count)).collect();
        for point in &points {
            assert!(point[0].abs() < HALF, "{point:?}");
            assert!(point[2].abs() < HALF, "{point:?}");
        }
        assert_ne!(points[0], points[1]);
    }
}
