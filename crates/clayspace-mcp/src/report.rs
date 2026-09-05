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
    BrushSettings, Diagnostics, FrameLog, LayerKey, MaskState as DomainMask, Representation, Scene,
    ToolKind, Transform,
};
use clayspace_vm::AgentGate;

use crate::session::{
    BackendState, CameraState, DocumentState, FallbackState, GateKind, HistoryState, LayerState,
    MaskState, MemoryPart, MemoryState, PhaseCostState, SceneState, StallState, StrokeCostState,
    TimingState, ToolState,
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
) -> SceneState {
    SceneState {
        layers: scene
            .layers
            .iter()
            .map(|layer| {
                let stands = placement(layer.key);
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
                    objects: layer.sculpt_layers.len(),
                }
            })
            .collect(),
        active_layer: scene.active.map(|key| key.0),
        selected_object: selected.map(|(layer, node)| layer << 32 | node as u64),
    }
}

pub fn tool_state(
    tool: ToolKind,
    brush: &BrushSettings,
    symmetry: [bool; 3],
    representation: Representation,
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

pub fn history_state(
    state: &clayspace_model::HistoryState,
    undoes: Option<String>,
    redoes: Option<String>,
    from_agent: usize,
) -> HistoryState {
    HistoryState {
        depth: state.depth,
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
pub fn mask_state(state: &DomainMask, cells_in_layer: Option<usize>) -> MaskState {
    MaskState {
        present: state.present,
        coverage: cells_in_layer
            .and_then(|whole| (whole > 0).then(|| state.painted_cells as f32 / whole as f32)),
        inverted: false,
    }
}

/// The memory ledger, by the part of the document that holds it.
///
/// The engine's own accounting, not an estimate kept here — the same figures
/// the status area shows, so an agent and a person cannot disagree about them.
pub fn memory_state(diagnostics: &Diagnostics, budget: u64) -> Option<MemoryState> {
    let memory = diagnostics.memory.as_ref()?;
    Some(MemoryState {
        in_use_bytes: memory.total,
        budget_bytes: budget,
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

    #[test]
    fn a_representation_reaches_the_wire_in_english() {
        assert_eq!(representation_tag(Representation::Sdf), "field");
        assert_eq!(representation_tag(Representation::Voxel), "grid");
        assert_eq!(representation_tag(Representation::Multires), "hierarchy");
    }

    #[test]
    fn the_scene_tree_carries_what_the_panel_shows() {
        let state = scene_state(&a_scene(), None, |_| None);
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
        let state = scene_state(&a_scene(), None, |_| None);
        assert_eq!(state.layers[0].translation, [0.0; 3]);
        assert_eq!(state.layers[0].scale, [1.0; 3]);
    }

    #[test]
    fn a_layer_the_engine_places_carries_where_it_stands() {
        let state = scene_state(&a_scene(), None, |_| {
            Some(Transform {
                position: [1.0, 2.0, 3.0],
                rotation_axis: [0.0, 1.0, 0.0],
                rotation_angle: 0.5,
                scale: [2.0, 1.0, 1.0],
            })
        });
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
        };
        let state = tool_state(
            ToolKind::Argila,
            &brush,
            [true, false, true],
            Representation::Sdf,
        );
        assert_eq!(state.tool, "clay");
        assert_eq!(state.falloff, "gaussian");
        assert_eq!(state.symmetry, vec!["x", "z"]);
        assert_eq!(state.representation, "field");
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
        assert_eq!(mask_state(&mask, None).coverage, None);
        assert_eq!(mask_state(&mask, Some(0)).coverage, None);
        assert_eq!(mask_state(&mask, Some(400)).coverage, Some(0.25));
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
        let state = memory_state(&diagnostics, 1024).unwrap();
        assert_eq!(state.in_use_bytes, 60);
        assert_eq!(state.budget_bytes, 1024);
        assert_eq!(state.parts.len(), 4);
        assert!(state.parts.iter().any(|part| part.bytes == 30));
    }

    #[test]
    fn a_build_with_no_ledger_reports_none_rather_than_zero() {
        assert!(memory_state(&Diagnostics::default(), 1024).is_none());
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

    /// The wire's gates and the window's gates are two enumerations of one
    /// idea, and a consent recorded under one tag has to be the consent asked
    /// for under the other.
    #[test]
    fn the_two_gate_enumerations_agree_tag_for_tag() {
        assert!(gates_agree());
    }
}
