//! Turning what the ViewModels already carry into what crosses the seam.
//!
//! The `Session` implementation lives in the composition root, because it
//! needs the whole application. Everything that does not need the whole
//! application lives here instead — and *here* rather than there because this
//! crate builds and tests with no window, no GPU and no C++ engine, which is
//! where every mistake in a conversion would otherwise only show up.
//!
//! The rule these follow: where the application cannot answer, the report says
//! so. A layer the engine will not place reads as the identity and a mask with
//! nothing to measure against reports no share, because a number invented here
//! is a number an agent would act on.

use std::time::Duration;

use clayspace_model::{
    BrushSettings, CombineSettings, DeformSettings, Diagnostics, FrameLog, LayerKey,
    MaskState as DomainMask, Representation, Scene, ToolKind, Transform,
};
use clayspace_vm::AgentGate;

use crate::catalogue::tags;
use crate::session::{
    BackendState, BrushState, CageState, CameraState, CombineSetting, CombineState,
    CrossingOutcomeState, DeformState, DocumentState, DragState, DynamicsState, ExchangeState,
    ExportState, FallbackState, GateKind, GridState, HierarchyPassState, HierarchyState,
    HistoryState, ImportState, LayerState, MaskState, MemoryPart, MemoryState, ObjectState,
    OutcomeState, PassState, PhaseCostState, PresentationState, ReferenceState, RemeshOutcomeState,
    RetopoOutcomeState, SceneState, StallState, StrokeCostState, TimingState, ToolState,
};

/// How many agent jobs the interface thread does between two frames.
///
/// A bound rather than "everything waiting": a burst from an agent should
/// delay itself rather than starve the redraw, and eight is enough that a
/// stroke's begin, its samples and its end land in one pass.
pub const JOBS_PER_FRAME: usize = 8;

/// How long a settle may hold the interface thread in one job.
pub const SETTLE_SLICE: Duration = Duration::from_millis(250);

/// The word the wire uses for a representation.
///
/// English and stable, like every other tag on the wire: `Representation`'s
/// own `label()` is interface text and is translated.
pub fn representation_tag(representation: Representation) -> &'static str {
    match representation {
        Representation::Sdf => "field",
        Representation::Voxel => "grid",
        Representation::Mesh => "mesh",
        Representation::Multires => "hierarchy",
    }
}

pub fn document_state(
    name: &str,
    modified: bool,
    path: Option<&std::path::Path>,
    unit: &str,
    format: &str,
) -> DocumentState {
    DocumentState {
        name: name.to_string(),
        modified,
        path: path.map(|path| path.display().to_string()),
        unit: unit.to_string(),
        format: format.to_string(),
    }
}

/// The scene tree, with each layer's placement where the engine can be asked
/// for one.
///
/// The placement is not on the layer summary — it is read back from the engine
/// through the object ViewModel — so it arrives here as a closure rather than
/// as a field, and a layer the engine will not answer for reports the identity
/// rather than a guess.
pub fn scene_state(
    scene: &Scene,
    selected: Option<(u64, u32)>,
    mut placement: impl FnMut(LayerKey) -> Option<Transform>,
    mut objects_in: impl FnMut(LayerKey) -> usize,
) -> SceneState {
    SceneState {
        layers: scene
            .layers
            .iter()
            .map(|layer| layer_state(layer, placement(layer.key), objects_in(layer.key)))
            .collect(),
        active_layer: scene.active.map(|key| key.0),
        selected_object: selected.map(object_id),
        soloed: scene.soloed.map(|key| key.0),
    }
}

/// The number an agent compares a placed form against.
///
/// One value out of the two that name it, because a caller holding them apart
/// would be doing the packing itself and the two spellings would drift.
pub fn object_id((layer, node): (u64, u32)) -> u64 {
    layer << 32 | node as u64
}

fn layer_state(
    layer: &clayspace_model::LayerSummary,
    stands: Option<Transform>,
    objects: usize,
) -> LayerState {
    LayerState {
        key: layer.key.0,
        name: layer.name.clone(),
        representation: representation_tag(layer.representation).to_string(),
        visible: layer.visible,
        locked: !layer.protection.is_editable(),
        translation: stands.map(|at| at.position).unwrap_or([0.0; 3]),
        rotation: stands
            .map(|at| {
                [
                    at.rotation_axis[0],
                    at.rotation_axis[1],
                    at.rotation_axis[2],
                    at.rotation_angle,
                ]
            })
            .unwrap_or([0.0, 0.0, 0.0, 0.0]),
        scale: stands.map(|at| at.scale).unwrap_or([1.0; 3]),
        // Counted by the caller, which is the only party that can ask the
        // document. This used to be `sculpt_layers.len()`, which is a grid's
        // *recorded passes* under a name that says objects — so a field layer
        // holding a dozen placed shapes reported none, and a grid reported its
        // passes as though they were shapes. The passes are below, named.
        objects,
        passes: layer.sculpt_layers.iter().map(pass_state).collect(),
        grid: layer.voxel.map(|voxel| GridState {
            cell_size: voxel.cell_size,
            occupied_cells: voxel.occupied,
        }),
        hierarchy: layer.multires.as_ref().map(hierarchy_state),
    }
}

fn pass_state(pass: &clayspace_model::SculptLayer) -> PassState {
    PassState {
        index: pass.index,
        name: pass.name.clone(),
        strength: pass.strength,
        visible: pass.visible,
        cells: pass.cells,
    }
}

fn hierarchy_state(state: &clayspace_model::MultiresState) -> HierarchyState {
    HierarchyState {
        levels: state.levels.count,
        sculpt_level: state.levels.sculpt,
        display_level: state.levels.display,
        write_domain: write_domain_tag(state.write_domain).to_string(),
        // The base is the form *under* the passes rather than a pass, and the
        // engine spells it zero — so it is absent here rather than reported as
        // an id, which an agent would go looking for in the list below.
        active_pass: (!state.active_sculpt_layer.is_base())
            .then(|| state.active_sculpt_layer.raw()),
        passes: state
            .sculpt_layers
            .iter()
            .map(|pass| HierarchyPassState {
                id: pass.id.raw(),
                index: pass.index,
                name: pass.name.clone(),
                strength: pass.strength,
                visible: pass.visible,
                locked: pass.locked,
            })
            .collect(),
    }
}

fn write_domain_tag(domain: clayspace_model::WriteDomain) -> &'static str {
    match domain {
        clayspace_model::WriteDomain::Automatic => "automatic",
        clayspace_model::WriteDomain::Geometry => "geometry",
        clayspace_model::WriteDomain::Detail => "detail",
    }
}

pub fn tool_state(
    tool: ToolKind,
    brush: &BrushSettings,
    symmetry: [bool; 3],
    representation: Representation,
    smooth_mode: clayspace_model::SmoothFrequency,
    rig_mirror: Option<bool>,
) -> ToolState {
    ToolState {
        tool: tool.key().to_string(),
        radius: brush.size,
        strength: brush.intensity,
        falloff: falloff_tag(brush.shaping.falloff).to_string(),
        symmetry: ["x", "y", "z"]
            .iter()
            .zip(symmetry)
            .filter(|(_, on)| *on)
            .map(|(axis, _)| axis.to_string())
            .collect(),
        representation: representation_tag(representation).to_string(),
        // Sent only where it decides something. A frequency reported beside a
        // field or a mesh would read as a setting an agent could act on, and
        // the three smooths exist only on a hierarchy — see
        // `SmoothFrequency::is_offered_on`.
        smooth_mode: clayspace_model::SmoothFrequency::is_offered_on(representation)
            .then(|| smooth_mode.key().to_string()),
        // The rig's own mirror, and only while one is being edited. It is a
        // second switch rather than a fourth axis: `symmetry` decides what a
        // brush stamps, this decides whether a new ZSphere gets a partner, and
        // reporting the first alone told an agent symmetry was off on a rig
        // that was mirroring every sphere it added.
        rig_mirror,
    }
}

fn falloff_tag(falloff: clayspace_model::Falloff) -> &'static str {
    match falloff {
        clayspace_model::Falloff::Constant => "constant",
        clayspace_model::Falloff::Linear => "linear",
        clayspace_model::Falloff::Smooth => "smooth",
        clayspace_model::Falloff::Gaussian => "gaussian",
    }
}

/// Everything the brush panel holds beside the size and strength.
///
/// The grain crosses in degrees although the document keeps radians, for the
/// reason the camera's field of view does: an agent reasons in degrees, and a
/// number it has to convert before it can compare two strokes is a number it
/// will compare unconverted.
pub fn brush_state(brush: &BrushSettings) -> BrushState {
    BrushState {
        flow: brush.flow,
        noise: brush.shaping.noise,
        accumulate: brush.shaping.accumulate,
        smoothing: brush.shaping.smoothing,
        stroke_mirror: brush.shaping.mirror,
        grain_degrees: brush.shaping.azimuth.to_degrees(),
        alpha: brush.alpha,
        invert: brush.invert,
        dynamics: DynamicsState {
            pressure_size: brush.dynamics.pressure_size,
            pressure_strength: brush.dynamics.pressure_strength,
            pressure_curve: brush.dynamics.pressure_curve,
            taper_start: brush.dynamics.taper_start,
            taper_end: brush.dynamics.taper_end,
            rake: brush.dynamics.rake,
        },
        drag: DragState {
            falloff: tags::tag_of(tags::DRAG_FALLOFFS, brush.drag.falloff).to_string(),
            front_only: brush.drag.front_only,
        },
    }
}

/// How a stroke and a placed form each meet what is already there.
///
/// Both, because they are two settings held by two ViewModels. An agent that
/// set the placement's operation and read the stroke's back had no way to see
/// why its next stroke still added.
pub fn combine_state(stroke: &CombineSettings, placement: &CombineSettings) -> CombineState {
    CombineState {
        stroke: combine_setting(stroke),
        placement: combine_setting(placement),
    }
}

fn combine_setting(settings: &CombineSettings) -> CombineSetting {
    CombineSetting {
        op: settings.op.key().to_string(),
        blend: settings.blend.key().to_string(),
        radius: settings.radius,
    }
}

pub fn camera_state(
    eye: [f32; 3],
    target: [f32; 3],
    up: [f32; 3],
    fov_y_radians: f32,
    viewport: [u32; 2],
) -> CameraState {
    CameraState {
        eye,
        target,
        up,
        fov_degrees: fov_y_radians.to_degrees(),
        viewport,
    }
}

/// The history, as the caller about to press undo reads it.
///
/// `undoes` and `redoes` are the *next step in each direction*, not the last
/// thing that happened. The two differ exactly where it matters: after an undo
/// the last thing that happened is the undo, and after a cancelled stroke it
/// is the tool that made the stroke the cancel took back — and both were
/// reported as the thing the next undo would revert.
pub fn history_state(
    state: &clayspace_model::HistoryState,
    undoes: Option<String>,
    redoes: Option<String>,
    from_agent: usize,
) -> HistoryState {
    HistoryState {
        depth: state.depth,
        redo_depth: state.redo_depth,
        // Guarded by what the history says it can do rather than by whether a
        // name arrived: a label left over from a stack that has since been
        // emptied would name a step nothing would take.
        undoes: state.can_undo.then_some(undoes).flatten(),
        redoes: state.can_redo.then_some(redoes).flatten(),
        from_agent,
    }
}

/// The mask, and how much of the layer it holds.
///
/// The engine counts painted cells and does not answer a share, so the
/// coverage is `None` where nothing says what the whole is. A ratio invented
/// here would be a number an agent would act on.
///
/// **`present` answers "is anything frozen", not "does a mask exist".** They
/// are two different questions inside the application — a document has no verb
/// for detaching a mask, so Limpar empties one and it stays attached — and
/// only one of them is a question about the document an agent is editing. It
/// reported the other, so a clear was followed by `present: true` and an agent
/// went on believing a region was still protected against the stroke it was
/// about to make.
pub fn mask_state(
    state: &DomainMask,
    cells_in_layer: Option<usize>,
    steps: i32,
    gesture: clayspace_model::MaskGesture,
) -> MaskState {
    MaskState {
        present: state.is_active(),
        coverage: cells_in_layer
            .and_then(|whole| (whole > 0).then(|| state.painted_cells as f32 / whole as f32)),
        inverted: false,
        painted_cells: state.painted_cells,
        steps,
        gesture: tags::tag_of(tags::GESTURES, gesture).to_string(),
    }
}

/// The cage, where one is up.
///
/// The divisions are reported whether or not one is, because they are what the
/// *next* cage would be built with — but nothing else below means anything
/// while `active` is false, which is why the flag leads.
pub fn cage_state(state: &clayspace_model::LatticeState, divisions: [i32; 3]) -> CageState {
    CageState {
        active: state.active,
        divisions,
        points: state.points.len(),
        selected_points: state.selection.len(),
        mode: tags::tag_of(tags::GIZMO_MODES, state.mode).to_string(),
    }
}

/// What the deform panel would do if it were run.
///
/// Both ends and the angle, whichever verb is in hand. A field the verb does
/// not read is still a field a caller set, and hiding it would leave a caller
/// that set a taper and switched to a twist unable to see where its numbers
/// went.
pub fn deform_state(settings: &DeformSettings) -> DeformState {
    DeformState {
        verb: tags::tag_of(tags::DEFORM_VERBS, settings.verb).to_string(),
        axis: settings.axis,
        span: settings.span,
        scale_start: settings.scale_start,
        scale_end: settings.scale_end,
        degrees: settings.degrees,
    }
}

/// The forms a sculptor has placed, by the id the document knows them as.
///
/// The id is what makes the section worth having: a caller that placed three
/// cylinders can say which of them it just moved, which a count cannot.
pub fn object_state(
    objects: &[clayspace_model::SceneObject],
    selected: Option<clayspace_model::ObjectId>,
) -> Vec<ObjectState> {
    objects
        .iter()
        .map(|object| ObjectState {
            id: object_id((object.id.layer.0, object.id.node)),
            layer: object.id.layer.0,
            node: object.id.node,
            source: object_source_tag(&object.source),
            parameters: object.parameters.clone(),
            position: object.position,
            rotation: [
                object.rotation_axis[0],
                object.rotation_axis[1],
                object.rotation_axis[2],
                object.rotation_angle,
            ],
            scale: object.scale,
            selected: selected == Some(object.id),
        })
        .collect()
}

/// What a placed form is, in a word that survives a translation.
///
/// A mesh placed as a form carries the name of the layer it was sampled from,
/// which is a sculptor's own word and is therefore reported as the layer's
/// name rather than pretending to be a stable tag.
fn object_source_tag(source: &clayspace_model::ObjectSource) -> String {
    match source {
        clayspace_model::ObjectSource::Shape(shape) => shape.key().to_string(),
        clayspace_model::ObjectSource::Mesh { name, .. } => format!("mesh:{name}"),
    }
}

/// What the last rebuild, retopology and crossing came to.
///
/// Each absent until one has run. "It has not been done" and "it was done and
/// changed nothing" are different answers, and an agent verifying a rebuild
/// branches on the difference.
pub fn outcome_state(
    remesh: Option<&clayspace_model::RemeshOutcome>,
    retopology: Option<&clayspace_model::RetopoOutcome>,
    crossing: Option<(clayspace_model::Direction, LayerKey)>,
) -> OutcomeState {
    OutcomeState {
        remesh: remesh.map(|outcome| RemeshOutcomeState {
            triangles_before: outcome.triangles_before,
            triangles_after: outcome.triangles_after,
            voxel_size: outcome.voxel_size,
            pieces: outcome.pieces,
            pieces_removed: outcome.pieces_removed,
            watertight: outcome.watertight,
            uvs_dropped: outcome.uvs_dropped,
        }),
        retopology: retopology.map(|outcome| RetopoOutcomeState {
            triangles_before: outcome.triangles_before,
            triangles: outcome.triangles,
            vertices: outcome.vertices,
            faces: outcome.faces,
            quads: outcome.is_quads(),
        }),
        crossing: crossing.map(|(direction, layer)| CrossingOutcomeState {
            direction: tags::tag_of(tags::DIRECTIONS, direction).to_string(),
            layer: layer.0,
        }),
    }
}

/// How the viewport is presented, as opposed to what it holds.
///
/// None of it enters the document or the history, and all of it changes what a
/// capture looks like — so an agent comparing two frames needs it before it
/// reads a difference as a defect.
pub fn presentation_state(
    focus: bool,
    grid: bool,
    polyframe: bool,
    view_preset: clayspace_model::ViewPresetKind,
    surface_opacity: clayspace_model::SurfaceOpacity,
    rigging: bool,
    skin_preview: bool,
) -> PresentationState {
    PresentationState {
        focus,
        grid,
        polyframe,
        view_preset: tags::tag_of(tags::VIEW_PRESETS, view_preset).to_string(),
        surface_opacity: surface_opacity.get(),
        rigging,
        skin_preview,
    }
}

/// The reference images, plane by plane.
///
/// Every plane, including the ones carrying no picture: "there is no side
/// image" is the answer to half the questions asked of this section, and a
/// list that simply omitted the plane would make it indistinguishable from a
/// plane the report forgot.
pub fn reference_state(
    mut plane: impl FnMut(
        clayspace_model::RefPlane,
    ) -> (
        clayspace_model::ReferenceSettings,
        Option<std::path::PathBuf>,
    ),
) -> Vec<ReferenceState> {
    clayspace_model::RefPlane::ALL
        .into_iter()
        .map(|which| {
            let (settings, path) = plane(which);
            ReferenceState {
                plane: tags::tag_of(tags::planes().as_slice(), which).to_string(),
                placed: path.is_some(),
                path: path.map(|path| path.display().to_string()),
                visible: settings.visible,
                opacity: settings.opacity,
                height: settings.height,
                offset: settings.offset,
                depth: settings.depth,
            }
        })
        .collect()
}

/// What an import or an export would be given.
pub fn exchange_state(
    import: &clayspace_model::ImportSettings,
    export: &clayspace_model::ExportSettings,
    findings: &[clayspace_model::ExportWarning],
) -> ExchangeState {
    ExchangeState {
        import: ImportState {
            becomes: tags::tag_of(tags::IMPORT_AS, import.becomes).to_string(),
            scale: import.scale,
            max_vertices: import.max_vertices,
            max_triangles: import.max_triangles,
        },
        export: ExportState {
            mesher: tags::tag_of(tags::MESHERS, export.mesher).to_string(),
            resolution: export.resolution,
            decimate_to: export.decimate_to,
            // What the bytes turned out to be, which is the one thing in the
            // export panel that cannot be derived from the settings beside it.
            findings: findings
                .iter()
                .map(|warning| warning.message.clone())
                .collect(),
        },
    }
}

/// The memory ledger, by the part of the document that holds it.
///
/// The engine's own accounting, not an estimate kept here — the same figures
/// the status area shows, so an agent and a person cannot disagree about them.
pub fn memory_state(
    diagnostics: &Diagnostics,
    cache_bytes: u64,
    budget: u64,
) -> Option<MemoryState> {
    let memory = diagnostics.memory.as_ref()?;
    Some(MemoryState {
        in_use_bytes: memory.total,
        budget_bytes: budget,
        // The status area's own figure, sent beside the ledger rather than
        // reconciled with it. They count different things — this is the brick
        // cache the budget bounds, above is the whole document with its
        // surfaces — and reporting one as though it were the other is how a
        // status area reading 0.00 GB and a report of 359 MB came to look like
        // a defect in one of them.
        cache_bytes,
        parts: vec![
            MemoryPart {
                part: "essencial".into(),
                bytes: memory.essential,
            },
            MemoryPart {
                part: "reconstruível".into(),
                bytes: memory.rebuildable,
            },
            MemoryPart {
                part: "desfazível".into(),
                bytes: memory.undoable,
            },
            MemoryPart {
                part: "superfícies".into(),
                bytes: memory.surface_bytes,
            },
        ],
    })
}

pub fn timing_state(log: &FrameLog, frame_millis: f32) -> TimingState {
    TimingState {
        frame_millis,
        stalls: log
            .stalls()
            .iter()
            .map(|stall| StallState {
                operation: stall.operation.clone(),
                millis: stall.took.as_secs_f64() * 1000.0,
                count: stall.count,
            })
            .collect(),
    }
}

pub fn backend_state(diagnostics: &Diagnostics) -> BackendState {
    BackendState {
        active: diagnostics.active_backend.clone(),
        registered: diagnostics.backends.clone(),
        engine_version: diagnostics.engine_version.clone(),
        engine_revision: diagnostics.engine_revision.clone(),
        platform: diagnostics.platform.clone(),
        fallbacks: diagnostics
            .fallbacks
            .iter()
            .map(|fallback| FallbackState {
                operation: fallback.operation.clone(),
                declined_by: fallback.declined_by.clone(),
            })
            .collect(),
    }
}

/// Where the last strokes spent their milliseconds.
///
/// `None` where the report carries no stroke section, which is what a report
/// assembled without one looks like — the composition root only summarises the
/// profile when something is going to read it, because summarising costs
/// nearly a millisecond once a session has been worked and recording costs
/// eighteen nanoseconds. Absent and *nothing was measured* are different
/// answers: a session nobody sculpted in still reports every phase, with no
/// samples in it.
pub fn stroke_state(diagnostics: &Diagnostics) -> Option<StrokeCostState> {
    let stroke = diagnostics.stroke.as_ref()?;
    Some(StrokeCostState {
        tools_measured: stroke.tools,
        phases: stroke.phases.iter().map(phase_cost_state).collect(),
        // Always true. A figure taken with a window open is evidence, not a
        // baseline, and nothing here may write one.
        live_session: true,
    })
}

fn phase_cost_state(phase: &clayspace_model::PhaseCost) -> PhaseCostState {
    let millis = |took: Option<std::time::Duration>| took.map(|d| d.as_secs_f64() * 1000.0);
    PhaseCostState {
        phase: phase.phase.clone(),
        side: if phase.engine { "engine" } else { "ours" }.to_string(),
        entry_point: phase.entry_point.clone(),
        samples: phase.samples,
        median_ms: millis(phase.median),
        p95_ms: millis(phase.p95),
        worst_ms: millis(phase.worst),
        keys: phase.keys,
        triangles: phase.triangles,
        bricks: phase.bricks,
    }
}

#[cfg(test)]
mod stroke_tests {
    use super::*;
    use clayspace_model::{Phase, StrokeDiagnostics, StrokeProfile, Work};

    fn worked() -> Diagnostics {
        let mut profile = StrokeProfile::default();
        for step in 0..12 {
            profile.record(
                "Padrão",
                Phase::EngineEdit,
                Duration::from_micros(500 + step * 10),
                Work::bricks(27),
            );
            profile.record(
                "Padrão",
                Phase::EngineMesh,
                Duration::from_micros(6_000 + step * 40),
                Work::meshed(27, 9_000),
            );
        }
        Diagnostics {
            stroke: Some(StrokeDiagnostics::of(&profile)),
            ..Diagnostics::default()
        }
    }

    /// The whole point of the section: an agent that drove the strokes can say
    /// *which call* was slow, which a total spanning both sides cannot.
    #[test]
    fn an_agent_reads_which_side_of_the_engine_boundary_the_time_went_to() {
        let state = stroke_state(&worked()).expect("a stroke section");
        assert_eq!(state.tools_measured, 1);

        let edit = state
            .phases
            .iter()
            .find(|phase| phase.phase == "engine edit")
            .expect("the engine's edit");
        assert_eq!(edit.side, "engine");
        assert_eq!(edit.entry_point.as_deref(), Some("stroke and brick refill"));
        assert_eq!(edit.samples, 12);
        assert!(edit.median_ms.is_some_and(|ms| ms > 0.0));
        assert_eq!(edit.bricks, 324);

        let upload = state
            .phases
            .iter()
            .find(|phase| phase.phase == "upload")
            .expect("our upload");
        assert_eq!(upload.side, "ours");
        assert!(upload.entry_point.is_none());
    }

    /// A zero would read as *free*, which is the reading that sends an agent
    /// looking in the wrong place.
    #[test]
    fn a_phase_that_never_ran_carries_no_figure_rather_than_a_zero() {
        let state = stroke_state(&worked()).expect("a stroke section");
        let upload = state
            .phases
            .iter()
            .find(|phase| phase.phase == "upload")
            .expect("our upload");
        assert_eq!(upload.samples, 0);
        assert_eq!(upload.median_ms, None);
        assert_eq!(upload.p95_ms, None);
        assert_eq!(upload.worst_ms, None);
    }

    /// The spec is blunt about it: a figure taken with a window open is
    /// evidence, not a baseline.
    #[test]
    fn every_figure_says_it_came_from_a_live_session() {
        assert!(stroke_state(&worked()).expect("a section").live_session);
    }

    /// Absent and *nothing was measured* are different answers. The first is a
    /// report assembled without the section, because summarising costs
    /// something and nobody asked; the second still lists every phase.
    #[test]
    fn a_report_assembled_without_the_section_has_none() {
        assert!(stroke_state(&Diagnostics::default()).is_none());

        let empty = Diagnostics {
            stroke: Some(StrokeDiagnostics::of(&StrokeProfile::default())),
            ..Diagnostics::default()
        };
        let state = stroke_state(&empty).expect("a section with nothing in it");
        assert_eq!(state.tools_measured, 0);
        assert_eq!(state.phases.len(), Phase::ALL.len());
        assert!(state.phases.iter().all(|phase| phase.samples == 0));
    }
}

/// The gate, as the ViewModel names it.
///
/// Two enumerations rather than one because `clayspace-vm` does not depend on
/// the agent-facing crate — that edge runs the other way — and the composition
/// root is where the two meet, as it is for every other pair like this.
pub fn gate_for_the_window(gate: GateKind) -> AgentGate {
    match gate {
        GateKind::Overwrite => AgentGate::Overwrite,
        GateKind::Export => AgentGate::Export,
        GateKind::Open => AgentGate::Open,
        GateKind::DiscardUnsaved => AgentGate::DiscardUnsaved,
        GateKind::IrreversibleRemoval => AgentGate::IrreversibleRemoval,
        GateKind::Quit => AgentGate::Quit,
    }
}

/// The two enumerations agree, which is asserted rather than assumed.
pub fn gates_agree() -> bool {
    GateKind::ALL
        .iter()
        .all(|gate| gate.tag() == gate_for_the_window(*gate).tag())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clayspace_model::{Falloff, Protection, Shaping};

    fn a_scene() -> Scene {
        Scene {
            nodes: Vec::new(),
            layers: vec![clayspace_model::LayerSummary {
                key: LayerKey(3),
                name: "cabeça".into(),
                representation: Representation::Multires,
                visible: true,
                protection: Protection {
                    ghost: false,
                    locked: true,
                },
                intensity: 255,
                health: None,
                voxel: None,
                sculpt_layers: Vec::new(),
                multires: None,
            }],
            active: Some(LayerKey(3)),
            soloed: None,
        }
    }

    /// The mask as the panel holds it, with the two settings the section
    /// gained. The coverage is what these tests are about.
    fn a_mask(state: &DomainMask, cells_in_layer: Option<usize>) -> MaskState {
        mask_state(
            state,
            cells_in_layer,
            1,
            clayspace_model::MaskGesture::Brush,
        )
    }

    #[test]
    fn a_representation_reaches_the_wire_in_english() {
        assert_eq!(representation_tag(Representation::Sdf), "field");
        assert_eq!(representation_tag(Representation::Voxel), "grid");
        assert_eq!(representation_tag(Representation::Multires), "hierarchy");
    }

    #[test]
    fn the_scene_tree_carries_what_the_panel_shows() {
        let state = scene_state(&a_scene(), None, |_| None, |_| 0);
        assert_eq!(state.layers.len(), 1);
        assert_eq!(state.layers[0].key, 3);
        assert_eq!(state.layers[0].name, "cabeça");
        assert_eq!(state.layers[0].representation, "hierarchy");
        assert!(state.layers[0].locked, "a locked layer reads as locked");
        assert_eq!(state.active_layer, Some(3));
    }

    /// A layer the engine will not answer for reports the identity rather than
    /// a guess. Every reopened subtool believing it was at the origin is a
    /// defect this project has already shipped once.
    #[test]
    fn a_layer_with_no_placement_reads_as_the_identity() {
        let state = scene_state(&a_scene(), None, |_| None, |_| 0);
        assert_eq!(state.layers[0].translation, [0.0; 3]);
        assert_eq!(state.layers[0].scale, [1.0; 3]);
    }

    #[test]
    fn a_layer_the_engine_places_carries_where_it_stands() {
        let state = scene_state(
            &a_scene(),
            None,
            |_| {
                Some(Transform {
                    position: [1.0, 2.0, 3.0],
                    rotation_axis: [0.0, 1.0, 0.0],
                    rotation_angle: 0.5,
                    scale: [2.0, 1.0, 1.0],
                })
            },
            |_| 0,
        );
        assert_eq!(state.layers[0].translation, [1.0, 2.0, 3.0]);
        assert_eq!(state.layers[0].scale, [2.0, 1.0, 1.0]);
        assert_eq!(state.layers[0].rotation, [0.0, 1.0, 0.0, 0.5]);
    }

    #[test]
    fn a_tool_reaches_the_wire_by_its_stable_key() {
        let brush = BrushSettings {
            size: 0.2,
            intensity: 0.6,
            flow: 1.0,
            dynamics: Default::default(),
            shaping: Shaping {
                noise: 0.0,
                falloff: Falloff::Gaussian,
                accumulate: false,
                smoothing: 0.0,
                mirror: false,
                azimuth: 0.0,
            },
            alpha: false,
            invert: false,
            drag: clayspace_model::Drag::default(),
        };
        let state = tool_state(
            ToolKind::Argila,
            &brush,
            [true, false, true],
            Representation::Sdf,
            clayspace_model::SmoothFrequency::default(),
            None,
        );
        assert_eq!(state.tool, "clay");
        assert_eq!(state.falloff, "gaussian");
        assert_eq!(state.symmetry, vec!["x", "z"]);
        assert_eq!(state.representation, "field");
        assert_eq!(
            state.smooth_mode, None,
            "a field has one smooth, so a frequency beside it would be a \
             setting an agent could act on and nothing would read"
        );
    }

    /// And on the one representation that has three of them, the mode is
    /// there, in the word the action takes back.
    #[test]
    fn a_hierarchy_reports_which_frequency_a_smooth_would_act_on() {
        let state = tool_state(
            ToolKind::Suavizar,
            &BrushSettings::default(),
            [false; 3],
            Representation::Multires,
            clayspace_model::SmoothFrequency::DetailOnly,
            None,
        );
        assert_eq!(state.smooth_mode.as_deref(), Some("detail_only"));
    }

    #[test]
    fn a_camera_reports_degrees_because_an_agent_reasons_in_them() {
        let state = camera_state(
            [0.0, 0.0, 3.0],
            [0.0; 3],
            [0.0, 1.0, 0.0],
            std::f32::consts::FRAC_PI_4,
            [800, 600],
        );
        assert!(
            (state.fov_degrees - 45.0).abs() < 0.01,
            "{}",
            state.fov_degrees
        );
        assert_eq!(state.viewport, [800, 600]);
    }

    #[test]
    fn a_history_that_cannot_undo_says_so_rather_than_naming_an_edit() {
        let state = history_state(
            &clayspace_model::HistoryState {
                can_undo: false,
                can_redo: true,
                depth: 0,
                redo_depth: 2,
            },
            Some("argila".into()),
            Some("suavizar".into()),
            4,
        );
        assert_eq!(state.undoes, None);
        assert_eq!(state.redoes.as_deref(), Some("suavizar"));
        assert_eq!(state.from_agent, 4);
    }

    #[test]
    fn a_mask_with_no_whole_to_measure_against_states_no_share() {
        let mask = DomainMask {
            present: true,
            painted_cells: 100,
        };
        assert_eq!(a_mask(&mask, None).coverage, None);
        assert_eq!(a_mask(&mask, Some(0)).coverage, None);
        assert_eq!(a_mask(&mask, Some(400)).coverage, Some(0.25));
    }

    #[test]
    fn memory_is_reported_by_the_part_that_holds_it() {
        let diagnostics = Diagnostics {
            memory: Some(clayspace_model::MemoryDiagnostics {
                essential: 10,
                rebuildable: 20,
                undoable: 30,
                total: 60,
                surfaces: 2,
                surface_bytes: 40,
            }),
            ..Diagnostics::default()
        };
        let state = memory_state(&diagnostics, 7, 1024).unwrap();
        assert_eq!(state.in_use_bytes, 60);
        assert_eq!(state.budget_bytes, 1024);
        assert_eq!(
            state.cache_bytes, 7,
            "the status area's own figure travels beside the ledger, because \
             the two count different things and were read as one"
        );
        assert_eq!(state.parts.len(), 4);
        assert!(state.parts.iter().any(|part| part.bytes == 30));
    }

    #[test]
    fn a_build_with_no_ledger_reports_none_rather_than_zero() {
        assert!(memory_state(&Diagnostics::default(), 7, 1024).is_none());
    }

    #[test]
    fn a_stall_reaches_the_wire_with_its_count() {
        let mut log = FrameLog::default();
        log.record("exportar", Duration::from_millis(80));
        log.record("exportar", Duration::from_millis(90));
        let state = timing_state(&log, 8.3);
        assert_eq!(state.stalls.len(), 1);
        assert_eq!(state.stalls[0].operation, "exportar");
        assert_eq!(state.stalls[0].count, 2);
        assert!((state.frame_millis - 8.3).abs() < 0.001);
    }

    #[test]
    fn the_backends_and_every_fallback_reach_the_wire() {
        let diagnostics = Diagnostics {
            active_backend: "metal".into(),
            backends: vec!["cpu".into(), "metal".into()],
            fallbacks: vec![clayspace_model::Fallback {
                operation: "remesh".into(),
                declined_by: "metal".into(),
            }],
            ..Diagnostics::default()
        };
        let state = backend_state(&diagnostics);
        assert_eq!(state.active, "metal");
        assert_eq!(state.registered.len(), 2);
        assert_eq!(state.fallbacks[0].operation, "remesh");
    }

    /// The defect the history section existed to have: `undoes` named the last
    /// thing that *happened*, so after an undo it read "undo" and after a
    /// cancelled clay stroke it read the tool that made the stroke the cancel
    /// had already taken back. What crosses now is the next step in each
    /// direction, with a depth for both.
    #[test]
    fn history_labels_name_the_next_step() {
        let state = history_state(
            &clayspace_model::HistoryState {
                can_undo: true,
                can_redo: true,
                depth: 3,
                redo_depth: 2,
            },
            Some("Argila".into()),
            Some("Suavizar".into()),
            0,
        );
        assert_eq!(state.undoes.as_deref(), Some("Argila"));
        assert_eq!(state.redoes.as_deref(), Some("Suavizar"));
        assert_eq!(state.depth, 3);
        assert_eq!(
            state.redo_depth, 2,
            "an agent that undid twice has to be able to tell how many redoes \
             put it back"
        );
    }

    /// A count is not a name. A history that can redo but cannot say what
    /// reports the depth and no label, rather than inventing one.
    #[test]
    fn a_redo_with_no_name_still_reports_its_depth() {
        let state = history_state(
            &clayspace_model::HistoryState {
                can_undo: false,
                can_redo: true,
                depth: 0,
                redo_depth: 1,
            },
            None,
            None,
            0,
        );
        assert_eq!(state.redoes, None);
        assert_eq!(state.redo_depth, 1);
    }

    /// `objects` used to be the length of `sculpt_layers`, which is a grid's
    /// *recorded passes*. So a field layer holding shapes reported none, and a
    /// grid reported its passes under a name that says objects.
    #[test]
    fn a_layer_counts_placed_forms_and_reports_its_passes_apart() {
        let mut scene = a_scene();
        scene.layers[0].representation = Representation::Voxel;
        scene.layers[0].voxel = Some(clayspace_model::VoxelStats {
            cell_size: 0.05,
            occupied: 4_096,
        });
        scene.layers[0].sculpt_layers = vec![clayspace_model::SculptLayer {
            index: 0,
            name: "pass".into(),
            strength: 0.5,
            visible: true,
            cells: 128,
            bytes: 2_048,
        }];

        let state = scene_state(&scene, None, |_| None, |_| 2);
        assert_eq!(
            state.layers[0].objects, 2,
            "two forms placed in the layer, whatever its passes"
        );
        assert_eq!(state.layers[0].passes.len(), 1);
        assert_eq!(state.layers[0].passes[0].cells, 128);
        let grid = state.layers[0].grid.as_ref().expect("a grid's cells");
        assert_eq!(grid.occupied_cells, 4_096);
    }

    /// Nothing said the scene was soloed, so an agent that soloed a layer and
    /// read the tree back saw only that two layers had become invisible —
    /// which is what a sculptor hiding them by hand also looks like.
    #[test]
    fn a_soloed_scene_says_which_layer_is_shown_alone() {
        let mut scene = a_scene();
        scene.soloed = Some(LayerKey(3));
        assert_eq!(scene_state(&scene, None, |_| None, |_| 0).soloed, Some(3));
        assert_eq!(
            scene_state(&a_scene(), None, |_| None, |_| 0).soloed,
            None,
            "a scene nobody soloed says nothing rather than naming a layer"
        );
    }

    /// A hierarchy's levels and passes, which nothing reported: an agent that
    /// subdivided could not tell which level its next stroke would land on.
    #[test]
    fn a_hierarchy_reports_its_levels_and_the_passes_above_them() {
        let mut scene = a_scene();
        scene.layers[0].multires = Some(clayspace_model::MultiresState {
            levels: clayspace_model::MultiresLevels {
                count: 4,
                sculpt: 2,
                display: 3,
            },
            sculpt_layers: vec![clayspace_model::MultiresSculptLayer {
                id: clayspace_model::MultiresSculptLayerId::new(7),
                index: 0,
                name: "detalhe".into(),
                strength: 1.0,
                visible: true,
                locked: false,
                masked: false,
                coverage_vertices: 0,
                bytes: 0,
            }],
            active_sculpt_layer: clayspace_model::MultiresSculptLayerId::new(7),
            write_domain: clayspace_model::WriteDomain::Detail,
        });

        let state = scene_state(&scene, None, |_| None, |_| 0);
        let hierarchy = state.layers[0]
            .hierarchy
            .as_ref()
            .expect("a hierarchy's levels");
        assert_eq!(hierarchy.levels, 4);
        assert_eq!(hierarchy.sculpt_level, 2);
        assert_eq!(hierarchy.display_level, 3);
        assert_eq!(hierarchy.write_domain, "detail");
        assert_eq!(hierarchy.active_pass, Some(7));
        assert_eq!(hierarchy.passes[0].id, 7);
    }

    /// The base is the form under the passes rather than a pass, so it is
    /// absent rather than reported as an id an agent would look for in the
    /// list.
    #[test]
    fn a_hierarchy_writing_the_form_names_no_active_pass() {
        let mut scene = a_scene();
        scene.layers[0].multires = Some(clayspace_model::MultiresState {
            levels: clayspace_model::MultiresLevels {
                count: 1,
                sculpt: 0,
                display: 0,
            },
            sculpt_layers: Vec::new(),
            active_sculpt_layer: clayspace_model::MultiresSculptLayerId::BASE,
            write_domain: clayspace_model::WriteDomain::Geometry,
        });
        let state = scene_state(&scene, None, |_| None, |_| 0);
        let hierarchy = state.layers[0].hierarchy.as_ref().expect("a hierarchy");
        assert_eq!(hierarchy.active_pass, None);
    }

    /// The rig's mirror is a second switch, not a fourth axis. Reported only
    /// while a rig is being edited, for the reason the smooth frequency is
    /// reported only on a hierarchy.
    #[test]
    fn the_rig_mirror_travels_beside_the_brush_symmetry() {
        let rigging = tool_state(
            ToolKind::Padrao,
            &BrushSettings::default(),
            [false; 3],
            Representation::Sdf,
            clayspace_model::SmoothFrequency::default(),
            Some(true),
        );
        assert!(rigging.symmetry.is_empty(), "the brush's own mirror is off");
        assert_eq!(
            rigging.rig_mirror,
            Some(true),
            "and the rig's is on, which is what nothing could see"
        );

        let sculpting = tool_state(
            ToolKind::Padrao,
            &BrushSettings::default(),
            [false; 3],
            Representation::Sdf,
            clayspace_model::SmoothFrequency::default(),
            None,
        );
        assert_eq!(sculpting.rig_mirror, None);
    }

    /// The brush panel, which had no section at all: an agent could set the
    /// flow, the grain or the six dynamics and read none of them back.
    #[test]
    fn the_brush_panel_crosses_whole() {
        let brush = BrushSettings {
            flow: 0.75,
            shaping: Shaping {
                noise: 0.1,
                falloff: Falloff::Linear,
                accumulate: false,
                smoothing: 0.3,
                mirror: true,
                azimuth: std::f32::consts::FRAC_PI_2,
            },
            dynamics: clayspace_model::Dynamics {
                pressure_size: 0.5,
                pressure_strength: 0.25,
                pressure_curve: 1.5,
                taper_start: 0.1,
                taper_end: 0.2,
                rake: true,
            },
            drag: clayspace_model::Drag {
                falloff: clayspace_model::DragFalloff::Tight,
                front_only: true,
            },
            alpha: true,
            invert: true,
            ..BrushSettings::default()
        };
        let state = brush_state(&brush);
        assert_eq!(state.flow, 0.75);
        assert!(state.stroke_mirror);
        assert!(
            (state.grain_degrees - 90.0).abs() < 0.001,
            "the grain crosses in degrees, as the camera's field of view does: {}",
            state.grain_degrees
        );
        assert!(state.dynamics.rake);
        assert_eq!(state.dynamics.pressure_curve, 1.5);
        assert_eq!(state.drag.falloff, "tight");
        assert!(state.drag.front_only);
        assert!(state.alpha && state.invert);
    }

    /// Two combines, because there are two settings. An agent that set the
    /// placement's operation and read the stroke's back saw no change.
    #[test]
    fn a_stroke_and_a_placement_report_their_own_combine() {
        let state = combine_state(
            &clayspace_model::CombineSettings {
                op: clayspace_model::Combine::Add,
                blend: clayspace_model::BlendProfile::Quadratic,
                radius: 0.1,
            },
            &clayspace_model::CombineSettings {
                op: clayspace_model::Combine::Subtract,
                blend: clayspace_model::BlendProfile::Hard,
                radius: 0.0,
            },
        );
        assert_eq!(state.stroke.op, "add");
        assert_eq!(state.stroke.blend, "quadratic");
        assert_eq!(state.placement.op, "subtract");
        assert_eq!(state.placement.radius, 0.0);
    }

    /// The mask's steps decide what an expand, a contract and a smooth do, and
    /// nothing reported them — so a contract that took more than was asked for
    /// had nowhere to be looked up.
    #[test]
    fn the_mask_reports_its_steps_its_gesture_and_its_frozen_cells() {
        let state = mask_state(
            &DomainMask {
                present: true,
                painted_cells: 250,
            },
            Some(1_000),
            4,
            clayspace_model::MaskGesture::Lasso,
        );
        assert_eq!(state.steps, 4);
        assert_eq!(state.gesture, "lasso");
        assert_eq!(state.painted_cells, 250);
        assert_eq!(state.coverage, Some(0.25));
    }

    /// Nothing said whether a cage was standing, so an agent that put one up
    /// and dragged it could not tell a refused cage from a cage with nothing
    /// selected.
    #[test]
    fn a_standing_cage_reports_its_points_and_what_is_in_hand() {
        let state = cage_state(
            &clayspace_model::LatticeState {
                active: true,
                divisions: [2, 2, 2],
                points: vec![[0.0; 3]; 27],
                selection: vec![0, 1, 2],
                mode: clayspace_model::GizmoMode::Rotate,
                ..Default::default()
            },
            [2, 2, 2],
        );
        assert!(state.active);
        assert_eq!(state.points, 27);
        assert_eq!(state.selected_points, 3);
        assert_eq!(state.mode, "rotate");
    }

    /// Both ends and the angle, whichever verb is in hand: a caller that set a
    /// taper and switched to a twist has to be able to see where its numbers
    /// went.
    #[test]
    fn the_deform_panel_reports_every_parameter_it_holds() {
        let state = deform_state(&DeformSettings {
            verb: clayspace_model::DeformVerb::Twist,
            axis: [0.0, 1.0, 0.0],
            span: 0.8,
            scale_start: 1.0,
            scale_end: 0.4,
            degrees: 45.0,
        });
        assert_eq!(state.verb, "twist");
        assert_eq!(state.axis, [0.0, 1.0, 0.0]);
        assert_eq!(state.degrees, 45.0);
        assert_eq!(
            state.scale_end, 0.4,
            "a field the verb does not read is still a field the caller set"
        );
    }

    /// The forms a sculptor placed, by the id the document knows them as —
    /// which is what lets an agent say *which* of three cylinders it moved.
    #[test]
    fn a_placed_form_carries_the_id_the_selection_is_compared_against() {
        let id = clayspace_model::ObjectId {
            layer: LayerKey(3),
            node: 7,
        };
        let objects = vec![clayspace_model::SceneObject {
            id,
            source: clayspace_model::ObjectSource::Shape(clayspace_model::Shape::Cylinder),
            parameters: vec![0.5, 1.0],
            combine: clayspace_model::CombineSettings::default(),
            position: [1.0, 0.0, 0.0],
            rotation_axis: [0.0, 1.0, 0.0],
            rotation_angle: 0.25,
            scale: [1.0; 3],
        }];

        let state = object_state(&objects, Some(id));
        assert_eq!(state[0].source, "cylinder");
        assert_eq!(state[0].node, 7);
        assert!(state[0].selected);
        assert_eq!(
            state[0].id,
            scene_state(&a_scene(), Some((3, 7)), |_| None, |_| 0)
                .selected_object
                .expect("a selection"),
            "the packed id and the scene's selected object are the same number"
        );
    }

    /// A selection is not the same as a list, and an agent comparing them has
    /// to get `false` rather than a panic where nothing is selected.
    #[test]
    fn a_form_nobody_selected_says_so() {
        let objects = vec![clayspace_model::SceneObject {
            id: clayspace_model::ObjectId {
                layer: LayerKey(1),
                node: 0,
            },
            source: clayspace_model::ObjectSource::Mesh {
                from: LayerKey(2),
                name: "parafuso".into(),
            },
            parameters: Vec::new(),
            combine: clayspace_model::CombineSettings::default(),
            position: [0.0; 3],
            rotation_axis: [0.0, 1.0, 0.0],
            rotation_angle: 0.0,
            scale: [1.0; 3],
        }];
        let state = object_state(&objects, None);
        assert!(!state[0].selected);
        assert_eq!(state[0].source, "mesh:parafuso");
    }

    /// "It has not been done" and "it was done and changed nothing" are
    /// different answers, and an agent verifying a rebuild branches on which.
    #[test]
    fn an_operation_that_has_not_run_reports_nothing_rather_than_zero() {
        let empty = outcome_state(None, None, None);
        assert_eq!(empty, OutcomeState::default());

        let state = outcome_state(
            Some(&clayspace_model::RemeshOutcome {
                triangles_before: 100,
                triangles_after: 80,
                voxel_size: 0.02,
                pieces: 2,
                pieces_removed: 1,
                watertight: true,
                uvs_dropped: false,
            }),
            None,
            Some((clayspace_model::Direction::SdfToMesh, LayerKey(9))),
        );
        let remesh = state.remesh.expect("a rebuild");
        assert_eq!(remesh.triangles_after, 80);
        assert_eq!(
            remesh.pieces, 2,
            "more than one piece after a rebuild meant to fuse is the answer \
             to why it did not join"
        );
        let crossing = state.crossing.expect("a crossing");
        assert_eq!(crossing.direction, "field-to-mesh");
        assert_eq!(crossing.layer, 9);
    }

    /// The chrome and the fade change what a capture looks like without
    /// touching the document, so an agent comparing two frames needs them
    /// before it reads a difference as a defect.
    #[test]
    fn how_the_viewport_is_presented_crosses_with_the_rest() {
        let state = presentation_state(
            true,
            false,
            true,
            clayspace_model::ViewPresetKind::Front,
            clayspace_model::SurfaceOpacity::new(0.5),
            true,
            false,
        );
        assert!(state.focus && !state.grid && state.polyframe);
        assert_eq!(state.view_preset, "front");
        assert!((state.surface_opacity - 0.5).abs() < 0.001);
        assert!(state.rigging && !state.skin_preview);
    }

    /// Every plane, including the ones carrying nothing: "there is no side
    /// image" is the answer to half the questions asked of this section, and a
    /// list that omitted the plane would make it look forgotten.
    #[test]
    fn every_reference_plane_is_reported_whether_or_not_it_holds_a_picture() {
        let state = reference_state(|plane| {
            let settings = clayspace_model::ReferenceSettings {
                visible: true,
                opacity: 0.4,
                ..Default::default()
            };
            let path = (plane == clayspace_model::RefPlane::Front)
                .then(|| std::path::PathBuf::from("/tmp/frente.png"));
            (settings, path)
        });
        assert_eq!(state.len(), 3);
        assert_eq!(state[0].plane, "front");
        assert!(state[0].placed);
        assert_eq!(state[0].path.as_deref(), Some("/tmp/frente.png"));
        assert!(!state[1].placed, "the side plane carries nothing");
        assert_eq!(state[1].path, None);
    }

    /// What an import or an export would be given, plus what the last export
    /// turned out to be — the one thing in that panel that cannot be derived
    /// from the settings beside it.
    #[test]
    fn the_exchange_settings_and_the_last_export_s_findings_cross() {
        let state = exchange_state(
            &clayspace_model::ImportSettings {
                becomes: clayspace_model::ImportAs::Clay,
                scale: 2.0,
                max_vertices: 10,
                max_triangles: 20,
            },
            &clayspace_model::ExportSettings {
                mesher: clayspace_model::ExportMesher::Sharp,
                resolution: 0.01,
                decimate_to: Some(0.5),
            },
            &[clayspace_model::ExportWarning {
                message: "não é estanque".into(),
            }],
        );
        assert_eq!(state.import.becomes, "clay");
        assert_eq!(state.import.scale, 2.0);
        assert_eq!(state.export.mesher, "sharp");
        assert_eq!(state.export.decimate_to, Some(0.5));
        assert_eq!(state.export.findings, vec!["não é estanque".to_string()]);
    }
    /// The wire's gates and the window's gates are two enumerations of one
    /// idea, and a consent recorded under one tag has to be the consent asked
    /// for under the other.
    #[test]
    fn the_two_gate_enumerations_agree_tag_for_tag() {
        assert!(gates_agree());
    }
}
