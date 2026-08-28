//! What a scene of several subtools costs: switching between them, showing one
//! alone, and resolving a boolean between two of them.
//!
//! `subtool.activate` is the figure with a promise attached, and it is the
//! reason this group exists. Activation used to be a stack-row click and is now
//! also a viewport press, and on a **mesh** subtool it arms the mesh sculptor —
//! a weld and an adjacency pass over every triangle the layer carries. The
//! specification states the bound it has to hold: "no engine operation SHALL
//! block the interface thread for more than 16 ms", and activation is an engine
//! operation on a click, with no busy cursor in front of it.
//!
//! It misses, by ten times, and the figure is reported over budget rather than
//! silently. The design's stated fallback — arm the sculptor on the first dab
//! instead of on activation — was tried and does not work: with no sculptor a
//! mesh layer answers no pick, the interface sends no stroke where the pick
//! reported nothing, and so the first dab never arrives.
//! `the_pointer_finds_an_imported_mesh` in
//! `clayspace-engine/tests/mesh_sculpting.rs` is that deadlock, and it fails
//! the moment the arming comes out. docs/roadmap.md carries the whole reading,
//! and what would actually remove the cost.
//!
//! The sculptor is cached, one layer at a time, so a fixture that switched
//! between a mesh subtool and a field one would pay for the pass once and
//! measure nothing afterwards. The fixture here holds **two** mesh subtools and
//! alternates between them, which evicts the cache on every switch — the worst
//! case, and a real one: going back and forth between two carried meshes is
//! what a sculptor does with them.
//!
//! The screen is deliberately outside the clock for activation, and only for
//! activation. Choosing a subtool moves no geometry — the active one is drawn
//! by a per-draw tint over buffers that did not change, which is why the
//! renderer takes the active key apart from the buffer — so a refresh timed
//! here would price a re-upload the application does not perform. Solo and the
//! boolean both move the surface, so they are timed the way an edit is: from
//! the call to the surface arriving.
//!
//! Nothing but activation carries a budget. Solo is a re-mesh priced like an
//! edit, and the specification states no budget for an edit that is not a dab;
//! the copy and the boolean are bakes behind a stated cost and a confirmation,
//! in the class of the crossings they borrow their cost vocabulary from, none
//! of which carry one either. A number invented here would be a promise nobody
//! made.

use std::time::Instant;

use clayspace_app::Scene;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BooleanOp, BooleanSettings, CombineSettings, Direction, LayerKey, ObjectModel, Representation,
    SceneModel, SculptModel, Shape, ToolKind,
};
use clayspace_view::Gpu;

use crate::figures::{ms, Figure, Record};
use crate::groups::headless_gpu;
use crate::groups::visible::Screen;
use crate::run::Run;
use crate::skip::Skip;

/// What the specification allows an engine operation to hold the interface
/// thread for. The only budget in this group, and it is stated rather than
/// chosen here.
const INTERFACE_BLOCK_MS: f64 = 16.0;

/// Where the subtools a solo hides are put — clear of the reference form, so
/// the figure is about hiding layers rather than about a surface that grew.
const ASIDE: f32 = 1.8;

/// Where the boolean's second operand stands: overlapping the reference form,
/// since a subtraction that misses removes nothing and prices nothing.
const OVERLAPPING: [f32; 3] = [0.9, 0.0, 0.0];

pub fn measure(policy: &BackendPolicy, run: &mut Run) {
    let Some(gpu) = headless_gpu() else {
        return run.skip("subtool", Skip::NoHeadlessGpu);
    };
    activate(&gpu, policy, run);
    solo(&gpu, policy, run);
    solo_undo(&gpu, policy, run);
    copy(&gpu, policy, run);
    boolean(&gpu, policy, run);
}

/// Switching the sculpt target, between two heavy mesh subtools and onto a
/// field one.
fn activate(gpu: &Gpu, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("subtool.activate") {
        return;
    }
    match switches(gpu, policy) {
        Ok((mesh, field)) => {
            // The mesh figure is the one the budget is about: a field subtool
            // arms nothing, and is measured beside it so the difference the
            // sculptor pays for a carried mesh can be read off the pair.
            budgeted(run, "subtool.activate.mesh", mesh);
            budgeted(run, "subtool.activate.sdf", field);
        }
        Err(why) => run.skip("subtool.activate", why),
    }
}

/// Two mesh subtools and a field one, activated in turn.
///
/// The second mesh layer is a second crossing of the same field rather than a
/// copy of the first: `copy_subtool` bakes into a volume, which would give the
/// document another *field* layer and nothing to arm.
fn switches(gpu: &Gpu, policy: &BackendPolicy) -> Result<(Vec<f64>, Vec<f64>), Skip> {
    let scene = Scene::MeshReference;
    let mut document = scene
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let field = layer_of(&document, Representation::Sdf)?;
    let first = layer_of(&document, Representation::Mesh)?;
    document
        .set_active_layer(field)
        .map_err(|_| Skip::EditRefused)?;
    let second = document
        .convert_layer(Direction::SdfToMesh, Scene::VOXEL_CELL, 1)
        .map_err(|_| Skip::EditRefused)?;

    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let mut onto_mesh = Vec::new();
    let mut onto_field = Vec::new();
    for step in 0..Record::Repeatable.samples() {
        // Alternated, so no switch onto a mesh finds that mesh's sculptor
        // already built.
        let mesh = if step % 2 == 0 { first } else { second };
        onto_mesh.push(switch(&mut document, mesh)?);
        onto_field.push(switch(&mut document, field)?);
    }
    Ok((onto_mesh, onto_field))
}

/// One activation, with nothing else in the clock.
fn switch(document: &mut ClayDocument, key: LayerKey) -> Result<f64, Skip> {
    let started = Instant::now();
    document
        .set_active_layer(key)
        .map_err(|_| Skip::EditRefused)?;
    Ok(ms(started.elapsed()))
}

/// The first layer of a representation the document holds.
fn layer_of(document: &ClayDocument, representation: Representation) -> Result<LayerKey, Skip> {
    document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == representation)
        .map(|layer| layer.key)
        .ok_or(Skip::SceneWouldNotBuild)
}

/// Showing one subtool alone and putting the scene back.
fn solo(gpu: &Gpu, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("subtool.solo") {
        return;
    }
    match rounds(gpu, policy) {
        Ok(samples) => run.timings("subtool.solo", Record::Repeatable, samples),
        Err(why) => run.skip("subtool.solo", why),
    }
}

/// Solo and release, repeatedly, on a scene of three subtools.
///
/// The round trip rather than either half: what a sculptor spends looking at
/// one form on its own is engaging *and* releasing, and the two are not the
/// same cost — engaging hides two layers, releasing shows them.
fn rounds(gpu: &Gpu, policy: &BackendPolicy) -> Result<Vec<f64>, Skip> {
    let (mut document, mut screen, subject) = aside(gpu, policy)?;
    (0..Record::Repeatable.samples())
        .map(|_| {
            let started = Instant::now();
            document
                .set_solo(Some(subject))
                .map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, &mut document)?;
            document.set_solo(None).map_err(|_| Skip::EditRefused)?;
            screen.refresh(gpu, &mut document)?;
            Ok(ms(started.elapsed()))
        })
        .collect()
}

/// What a ⌘Z after a released solo costs.
///
/// The hops are the point. Solo has no journal pause to hide behind, so its
/// visibility commands are on the engine's stack and undo steps over them
/// before it reaches the edit underneath — this is what that stepping costs on
/// top of the undo it is standing in front of, and `history.undo` is what it
/// should be read against.
///
/// One-shot, on a document rebuilt for each sample: an undo that has already
/// run is not the undo a sculptor presses.
fn solo_undo(gpu: &Gpu, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("subtool.solo_undo") {
        return;
    }
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| hopped_undo(gpu, policy))
        .collect();
    match samples {
        Ok(samples) => run.timings("subtool.solo_undo", Record::OneShot, samples),
        Err(why) => run.skip("subtool.solo_undo", why),
    }
}

fn hopped_undo(gpu: &Gpu, policy: &BackendPolicy) -> Result<f64, Skip> {
    let (mut document, mut screen, subject) = aside(gpu, policy)?;
    document
        .set_active_layer(subject)
        .map_err(|_| Skip::EditRefused)?;
    let dab = Scene::Reference.stroke(1);
    document
        .apply_stroke(ToolKind::Padrao, Scene::Reference.brush(), &dab, [false; 3])
        .map_err(|_| Skip::EditRefused)?;
    screen.refresh(gpu, &mut document)?;
    document
        .set_solo(Some(subject))
        .map_err(|_| Skip::EditRefused)?;
    document.set_solo(None).map_err(|_| Skip::EditRefused)?;

    let started = Instant::now();
    let moved = document.undo().map_err(|_| Skip::EditRefused)?;
    screen.refresh(gpu, &mut document)?;
    let took = ms(started.elapsed());
    // An undo that reached the solo's own commands and stopped there took back
    // no edit, which is the defect the hops exist to prevent — reported as a
    // refusal rather than timed.
    moved.then_some(took).ok_or(Skip::EditRefused)
}

/// The reference form with two more subtools standing beside it.
///
/// Three, because two is the case where hiding the others is hiding one. The
/// key returned is the reference layer's: it is the one with a surface worth
/// re-meshing when it comes back.
fn aside(gpu: &Gpu, policy: &BackendPolicy) -> Result<(ClayDocument, Screen, LayerKey), Skip> {
    let mut document = Scene::Reference
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let subject = document.scene().active.ok_or(Skip::SceneWouldNotBuild)?;
    for side in [-ASIDE, ASIDE] {
        document
            .insert_shape_subtool(
                Shape::Sphere,
                &Shape::Sphere.defaults(),
                [side, 0.0, 0.0],
                CombineSettings::default(),
            )
            .map_err(|_| Skip::EditRefused)?;
    }
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;
    Ok((document, screen, subject))
}

/// Copying a subtool, which is one bake and nothing else.
///
/// Here so the boolean beside it can be read: a boolean is two bakes over a
/// region that holds both operands, plus a layer to put them in, and without
/// this there is no way to tell a change in the sampling from a change in
/// everything around it.
fn copy(gpu: &Gpu, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("subtool.copy") {
        return;
    }
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| copied(gpu, policy))
        .collect();
    match samples {
        Ok(samples) => run.timings("subtool.copy", Record::OneShot, samples),
        Err(why) => run.skip("subtool.copy", why),
    }
}

fn copied(gpu: &Gpu, policy: &BackendPolicy) -> Result<f64, Skip> {
    let mut document = Scene::Reference
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let source = document.scene().active.ok_or(Skip::SceneWouldNotBuild)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let started = Instant::now();
    document
        // The cell the copy control itself asks for, so the figure prices what
        // a sculptor is charged rather than a resolution chosen here.
        .copy_subtool(source, clayspace_vm::ObjectViewModel::OPERAND_CELL)
        .map_err(|_| Skip::EditRefused)?;
    screen.refresh(gpu, &mut document)?;
    Ok(ms(started.elapsed()))
}

/// Resolving a boolean between two subtools.
fn boolean(gpu: &Gpu, policy: &BackendPolicy, run: &mut Run) {
    if !run.wants_group("subtool.boolean") {
        return;
    }
    let samples: Result<Vec<f64>, Skip> = (0..Record::OneShot.samples())
        .map(|_| resolved(gpu, policy))
        .collect();
    match samples {
        Ok(samples) => run.timings("subtool.boolean", Record::OneShot, samples),
        Err(why) => run.skip("subtool.boolean", why),
    }
}

/// A sphere subtracted out of the reference form, at the resolution the panel
/// itself defaults to.
///
/// A *sculpted* operand rather than two primitives, because the bake samples
/// the evaluated field and a worked field is what it is expensive over. The
/// cell comes from `boolean_cell` rather than from a constant here so that the
/// figure prices what a sculptor is offered when they open the panel.
fn resolved(gpu: &Gpu, policy: &BackendPolicy) -> Result<f64, Skip> {
    let mut document = Scene::Reference
        .build(policy.clone())
        .map_err(|_| Skip::SceneWouldNotBuild)?;
    let base = document.scene().active.ok_or(Skip::SceneWouldNotBuild)?;
    let tool = document
        .insert_shape_subtool(
            Shape::Sphere,
            &Shape::Sphere.defaults(),
            OVERLAPPING,
            CombineSettings::default(),
        )
        .map_err(|_| Skip::EditRefused)?
        .layer;
    let cell = document
        .boolean_cell(base, tool)
        .ok_or(Skip::NoRegionToConvertInto)?;
    let mut screen = Screen::new(gpu);
    screen.prime(gpu, &mut document)?;

    let settings = BooleanSettings {
        base: Some(base),
        tool: Some(tool),
        op: BooleanOp::Subtract,
        cell_size: cell,
        consume: false,
    };
    let started = Instant::now();
    document
        .run_boolean(settings)
        .map_err(|_| Skip::EditRefused)?;
    screen.refresh(gpu, &mut document)?;
    Ok(ms(started.elapsed()))
}

/// Records a repeatable measurement against the interface-thread bound.
///
/// [`Record::figures`] leaves every figure without a budget, which is the right
/// default — the specification states one for a dab and for startup and for
/// nothing else in this suite. Activation is the third: it is an engine
/// operation on a click, so the 16 ms bound applies to it as written.
fn budgeted(run: &mut Run, prefix: &str, samples: Vec<f64>) {
    for (name, figure) in Record::Repeatable.figures(prefix, samples) {
        run.insert(
            name,
            Figure {
                budget: Some(INTERFACE_BLOCK_MS),
                ..figure
            },
        );
    }
}
