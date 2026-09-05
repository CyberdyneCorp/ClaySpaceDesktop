//! The profile, as one file somebody else can read.
//!
//! Written for a reader who has none of what produced it: not the machine, not
//! the document, not the conversation. Everything this project has ever had to
//! reconstruct by hand in an upstream issue — which engine build, which
//! backend, what fell back, what the adapter was, how big the thing being
//! measured was — is in here, because a follow-up question is a round trip and
//! a round trip is where a performance report dies.
//!
//! Two rules run through the whole file.
//!
//! **Nothing unmeasured is written as a zero.** A phase that never ran, a
//! backend that was never timed and an adapter with no timestamps are all
//! `null`. A zero would read as *free*, which is the reading that sends
//! somebody looking in the wrong place.
//!
//! **Nothing the sculptor named comes out.** No document path, no layer name.
//! A layer is its representation and its position. This is enforced by what is
//! *collected* rather than by a pass over the output: a redaction step is a
//! step that can be forgotten when a field is added, and a shape that never
//! holds a name cannot leak one.

use clayspace_model::{Diagnostics, Phase, Samples, SceneStats, StrokeProfile, ToolProfile};

use crate::json::Json;

/// The version of this file's own shape.
///
/// Here so that a reader written against one revision can say it does not know
/// a later one, rather than silently reading a field that has changed meaning.
const FORMAT: u64 = 1;

/// One layer, as much of it as a performance report is entitled to.
///
/// No name, deliberately — see the module's second rule.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerShape {
    /// Its position in the stack, bottom-up, which is how a reader refers to
    /// it in the absence of a name.
    pub index: usize,
    pub representation: String,
    pub visible: bool,
    /// Recorded passes on it, where it is a grid.
    pub sculpt_layers: usize,
    /// Levels, where it is a hierarchy.
    pub multires_levels: usize,
    /// Items in the edit list, where it is a field.
    pub items: Option<i64>,
    /// Whether the engine has collapsed that list.
    pub consolidated: Option<bool>,
    /// World units per cell, where it is a grid.
    pub cell_size: Option<f64>,
    /// Cells holding anything, where it is a grid.
    pub occupied: Option<u64>,
}

/// What was being sculpted when the figures were taken.
///
/// A duration without this is not comparable with any other duration: eleven
/// milliseconds over a thousand bricks and eleven over eighty are the same
/// number and opposite facts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DocumentShape {
    pub layers: Vec<LayerShape>,
    pub triangles: u64,
    pub vertices: u64,
    pub objects: u64,
    /// Which detail the counts above describe. A count given without it reads
    /// as a smaller model than the one on screen.
    pub detail: String,
}

impl DocumentShape {
    /// Reads a scene into the shape, taking none of what the sculptor named.
    pub fn of(scene: &clayspace_model::Scene, stats: SceneStats) -> Self {
        let layers = scene
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| LayerShape {
                index,
                representation: layer.representation.label().to_string(),
                visible: layer.visible,
                sculpt_layers: layer.sculpt_layers.len(),
                multires_levels: layer
                    .multires
                    .as_ref()
                    .map(|state| state.levels.count as usize)
                    .unwrap_or(0),
                items: layer.health.map(|health| i64::from(health.items)),
                consolidated: layer.health.map(|health| health.consolidated),
                cell_size: layer.voxel.map(|voxel| f64::from(voxel.cell_size)),
                occupied: layer.voxel.map(|voxel| voxel.occupied as u64),
            })
            .collect();
        Self {
            layers,
            triangles: stats.triangles as u64,
            vertices: stats.vertices as u64,
            objects: stats.objects as u64,
            detail: format!("{:?}", stats.detail).to_lowercase(),
        }
    }
}

/// What the default name of an exported profile is.
///
/// Beside [`EXTENSIONS`] rather than typed at the call site, because a save
/// dialog whose default name does not match the filter it offers is a dialog
/// that argues with itself — the same reason the alpha and reference dialogs
/// take their filters from the domain rather than repeating them.
pub const FILE_NAME: &str = "perfil.json";

/// What the save dialog offers to write.
pub const EXTENSIONS: [&str; 1] = ["json"];

/// What the export owes a person before it writes anything.
///
/// The decision, separated from the dialog that carries it. A native dialog
/// cannot be driven headlessly; *whether one is owed at all* is a property of
/// the build, and that is the part worth holding a test against — the failure
/// this guards is the warning and the file's own marker drifting apart, so
/// that a file stamped `"timings_comparable": false` is written by a build
/// that asked nobody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Write it. An optimised build's durations stand on their own.
    Nothing,
    /// Say what these timings are worth first, and write only on a yes.
    WarnTimingsAreNotComparable,
}

/// Whether the export must ask before it writes.
///
/// Derived from [`timings_comparable`] rather than from a second `cfg!` of its
/// own, so the question the person is asked and the claim the file makes
/// cannot disagree.
pub fn ask_before_writing() -> Ask {
    if timings_comparable() {
        Ask::Nothing
    } else {
        Ask::WarnTimingsAreNotComparable
    }
}

/// Whether a duration taken from this build may be compared with one taken
/// anywhere else.
///
/// An unoptimised build runs this work about two and a half times slower —
/// `sculpt_latency.rs` refuses to assert a budget against one for exactly that
/// reason — so a duration from one is a fact about the build profile and not
/// about the engine. The file says so rather than being withheld: the
/// identifying half of a profile is just as true in a debug build, and that is
/// often the build somebody is running when they hit the thing worth
/// reporting.
pub fn timings_comparable() -> bool {
    !cfg!(debug_assertions)
}

/// Which build produced the file.
pub fn build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

/// Writes a rendered profile to a path, whole or not at all.
///
/// A half-written file is one somebody attaches to an issue and nobody can
/// read, and a truncated JSON document fails in the reader rather than at the
/// moment it was produced — by which time whoever could have said what
/// happened has moved on. So a failed write takes its own remains with it.
pub fn write(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Err(e) = std::fs::write(path, text) {
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    Ok(())
}

/// The whole profile, as one JSON document.
pub fn render(
    diagnostics: &Diagnostics,
    profile: &StrokeProfile,
    document: &DocumentShape,
) -> String {
    let mut json = Json::new();
    json.integer("profile_format", FORMAT);
    json.string("written_by", &diagnostics.app_version);
    json.string("build", build_profile());
    json.boolean("timings_comparable", timings_comparable());
    conditions(&mut json, diagnostics);
    shape(&mut json, document, diagnostics);
    stroke(&mut json, profile);
    session(&mut json, diagnostics);
    rendering(&mut json, diagnostics);
    refill(&mut json, diagnostics);
    json.finish()
}

/// What the numbers were taken under. Without this, none of them compares
/// with anything.
fn conditions(json: &mut Json, diagnostics: &Diagnostics) {
    json.object("conditions");
    json.string("engine", &diagnostics.engine_version);
    // The version is not an identity: two builds both saying 0.78.0 can differ
    // by a commit, and this project has already filed a round of issues
    // against a stale engine for want of this line.
    json.string("engine_revision", &diagnostics.engine_revision);
    json.string("document_format", &diagnostics.document_format);
    // Already the operating system and the architecture together, as the
    // build target reports them.
    json.string("platform", &diagnostics.platform);
    json.array("backends");
    for backend in &diagnostics.backends {
        json.item(backend);
    }
    json.end();
    json.string("active_backend", &diagnostics.active_backend);
    json.string("selection", &diagnostics.selection);
    json.maybe_string("renderer", diagnostics.renderer.as_deref());
    json.end();
}

/// What was being sculpted, and what it was resident as.
fn shape(json: &mut Json, document: &DocumentShape, diagnostics: &Diagnostics) {
    json.object("document");
    json.integer("triangles", document.triangles);
    json.integer("vertices", document.vertices);
    json.integer("objects", document.objects);
    json.string("detail", &document.detail);
    json.array("layers");
    for layer in &document.layers {
        json.element();
        json.integer("index", layer.index as u64);
        json.string("representation", &layer.representation);
        json.boolean("visible", layer.visible);
        json.integer("sculpt_layers", layer.sculpt_layers as u64);
        json.integer("multires_levels", layer.multires_levels as u64);
        json.maybe_number("items", layer.items.map(|items| items as f64));
        json.maybe_number("cell_size", layer.cell_size);
        json.maybe_number("occupied_cells", layer.occupied.map(|cells| cells as f64));
        match layer.consolidated {
            Some(consolidated) => json.boolean("consolidated", consolidated),
            None => json.null("consolidated"),
        }
        json.end();
    }
    json.end();
    memory(json, diagnostics);
    json.end();
}

/// Where the document's memory is, in the terms that decide what may be let go
/// of. A total answers the wrong question.
fn memory(json: &mut Json, diagnostics: &Diagnostics) {
    let Some(memory) = &diagnostics.memory else {
        return json.null("memory");
    };
    json.object("memory");
    json.integer("essential", memory.essential);
    json.integer("rebuildable", memory.rebuildable);
    json.integer("undoable", memory.undoable);
    json.integer("total", memory.total);
    json.integer("surfaces_asked", memory.surfaces as u64);
    json.integer("surface_bytes", memory.surface_bytes);
    json.end();
}

/// The point of the file: which of a stroke's milliseconds were the engine's.
fn stroke(json: &mut Json, profile: &StrokeProfile) {
    json.object("stroke");
    json.object("across_tools");
    phases(json, &profile.across_tools());
    json.end();
    json.array("by_tool");
    for (tool, measured) in profile.tools() {
        json.element();
        json.string("tool", tool);
        phases(json, measured);
        json.end();
    }
    json.end();
    json.end();
}

/// Every phase, including the ones with nothing in them.
///
/// A section listing only the phases that ran would leave a reader unable to
/// tell a phase that cost nothing from a phase this build does not measure.
fn phases(json: &mut Json, measured: &ToolProfile) {
    json.array("phases");
    for phase in Phase::ALL {
        json.element();
        json.string("phase", phase.label());
        json.string("side", if phase.is_engine() { "engine" } else { "ours" });
        json.maybe_string("entry_point", phase.entry_point());
        distribution(json, &measured.phase(phase));
        json.end();
    }
    json.end();
}

/// One phase's samples: a distribution, never a mean.
///
/// The tail is what a sculptor is complaining about, and a mean is the
/// statistic that hides it. `seen` and `retained` are separate numbers because
/// the quantiles describe the retained window and a reader is entitled to know
/// which population they came from.
fn distribution(json: &mut Json, samples: &Samples) {
    // One sort for all three, as the report takes them.
    let summary = samples.summary();
    json.integer("seen", samples.seen());
    json.integer("retained", samples.retained() as u64);
    json.maybe_number("median_ms", summary.map(|s| milliseconds(s.median)));
    json.maybe_number("p95_ms", summary.map(|s| milliseconds(s.p95)));
    json.maybe_number("worst_ms", summary.map(|s| milliseconds(s.worst)));
    json.object("work");
    let work = samples.work();
    json.integer("bricks", work.bricks as u64);
    json.integer("keys", work.keys as u64);
    json.integer("triangles", work.triangles as u64);
    json.end();
}

/// What this session observed that no benchmark can: what actually stalled,
/// and what actually fell back.
fn session(json: &mut Json, diagnostics: &Diagnostics) {
    json.array("stalls");
    for stall in &diagnostics.stalls {
        json.item(stall);
    }
    json.end();
    json.array("fallbacks");
    for fallback in &diagnostics.fallbacks {
        json.element();
        json.string("operation", &fallback.operation);
        json.string("declined_by", &fallback.declined_by);
        json.end();
    }
    json.end();
}

/// What the viewport cost, from the device's own clock.
fn rendering(json: &mut Json, diagnostics: &Diagnostics) {
    let Some(render) = &diagnostics.render else {
        return json.null("rendering");
    };
    json.object("rendering");
    json.integer("viewport_width", u64::from(render.viewport[0]));
    json.integer("viewport_height", u64::from(render.viewport[1]));
    json.integer("samples", u64::from(render.samples));
    json.integer("draw_calls", u64::from(render.draw_calls));
    json.integer("culled", u64::from(render.culled));
    json.integer("triangles", render.triangles);
    json.integer("lines", render.lines);
    json.integer("uploaded_bytes", render.uploaded_bytes);
    // An adapter with no timestamps is not an adapter whose passes are free,
    // so the passes are `null` rather than an empty list of zeroes.
    json.boolean("gpu_timestamps", render.gpu_timing);
    if !render.gpu_timing {
        json.null("gpu_passes");
        return json.end();
    }
    json.array("gpu_passes");
    for (pass, ms) in &render.gpu_passes {
        json.element();
        json.string("pass", pass);
        json.number("ms", f64::from(*ms));
        json.end();
    }
    json.end();
    json.end();
}

/// The evidence the refill routing is decided on.
///
/// Only this application is in a position to observe it, and until now it was
/// visible to nothing but a test. It is the figure behind this project's
/// sharpest engine finding — that an accelerated backend can be several times
/// *slower* than the CPU on a given machine.
fn refill(json: &mut Json, diagnostics: &Diagnostics) {
    let Some(refill) = &diagnostics.refill else {
        return json.null("refill");
    };
    json.object("refill");
    json.string("accelerated", &refill.accelerated);
    json.maybe_number("cpu_ns_per_brick", refill.cpu);
    json.maybe_number("accelerated_ns_per_brick", refill.accelerated_cost);
    json.end();
}

fn milliseconds(took: std::time::Duration) -> f64 {
    took.as_secs_f64() * 1000.0
}
