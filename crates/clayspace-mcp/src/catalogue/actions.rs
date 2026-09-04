//! The vocabulary, and the one match that keeps it honest.
//!
//! Two directions. [`build`] turns a group, an action and its JSON arguments
//! into a [`Command`] — the same value a menu item pushes. [`home_of`] turns a
//! `Command` back into where it lives, and is **exhaustive with no wildcard
//! arm**: a new variant added to `Command` does not compile until somebody has
//! decided which group it belongs to, or has said in
//! [`Home::NotOffered`] why it does not.
//!
//! That is the whole anti-drift mechanism, and it is why this file is written
//! by hand rather than derived. A derive would accept a variant nobody
//! exposed, silently, which is exactly the failure a tool surface over a
//! two-hundred-verb application has.

use clayspace_model::{
    BooleanSettings, Colour, CombineSettings, ConversionSettings, DeformSettings, ExportSettings,
    ExtrudeSettings, GizmoHandle, GizmoTarget, ImportSettings, LayerKey, MaskOp, MultiresLevelOp,
    MultiresSculptLayerId, MultiresSculptLayerOp, ObjectId, OutlineFrame, ReferenceSettings,
    RemeshSettings, SculptLayerOp, SmoothBlur, StrokeModifiers, SurfaceOpacity,
};
use clayspace_vm::Command;

use super::args::Args;
use super::tags;
use crate::session::{Refusal, RefusalCode};

/// Where a command lives on the tool surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Home {
    In(&'static str, &'static str),
    /// Real, and deliberately not offered. The string is the reason, and it is
    /// what an agent is told when it asks why.
    NotOffered(&'static str),
}

impl Home {
    pub fn group(self) -> Option<&'static str> {
        match self {
            Self::In(group, _) => Some(group),
            Self::NotOffered(_) => None,
        }
    }

    pub fn action(self) -> Option<&'static str> {
        match self {
            Self::In(_, action) => Some(action),
            Self::NotOffered(_) => None,
        }
    }
}

/// The reason every file-dialog command is not offered, written once.
const OPENS_A_PANEL: &str =
    "this opens a file panel on the sculptor's own screen, which an agent cannot \
     answer and a person did not ask for";

/// Which group and action a command belongs to.
///
/// Exhaustive on purpose. Do not add a wildcard arm.
pub fn home_of(command: &Command) -> Home {
    use Command::*;
    match command {
        // -- tool -----------------------------------------------------------
        SelectTool(_) => Home::In("tool", "select"),

        // -- brush ----------------------------------------------------------
        SetBrushSize(_) => Home::In("brush", "set_size"),
        SetBrushIntensity(_) => Home::In("brush", "set_intensity"),
        SetBrushFlow(_) => Home::In("brush", "set_flow"),
        SetBrushNoise(_) => Home::In("brush", "set_noise"),
        SetBrushAzimuth(_) => Home::In("brush", "set_azimuth"),
        SetBrushFalloff(_) => Home::In("brush", "set_falloff"),
        SetBrushAccumulate(_) => Home::In("brush", "set_accumulate"),
        SetBrushAlpha(_) => Home::In("brush", "set_alpha"),
        SetBrushColour(_) => Home::In("brush", "set_colour"),
        SetBrushSmoothing(_) => Home::In("brush", "set_smoothing"),
        PickRecentColour(_) => Home::In("brush", "pick_recent_colour"),
        ClearAlpha => Home::In("brush", "clear_alpha"),
        ToggleSymmetry(_) => Home::In("brush", "toggle_symmetry"),
        LoadAlpha => Home::NotOffered(OPENS_A_PANEL),

        // -- stroke ---------------------------------------------------------
        BeginStroke { .. } => Home::In("stroke", "begin"),
        ContinueStroke { .. } => Home::In("stroke", "continue"),
        EndStroke => Home::In("stroke", "end"),
        CancelStroke => Home::In("stroke", "cancel"),

        // -- mask -----------------------------------------------------------
        ToggleMaskPainting => Home::In("mask", "toggle_painting"),
        ApplyMaskOp(_) => Home::In("mask", "apply"),
        SetMaskGesture(_) => Home::In("mask", "set_gesture"),
        SetMaskSteps(_) => Home::In("mask", "set_steps"),
        BeginMaskOutline(..) => Home::In("mask", "begin_outline"),
        ExtendMaskOutline(_) => Home::In("mask", "extend_outline"),
        EndMaskOutline(_) => Home::In("mask", "end_outline"),
        CancelMaskOutline => Home::In("mask", "cancel_outline"),
        SetExtrudeSettings(_) => Home::In("mask", "set_extrude"),
        ExtrudeMask(_) => Home::In("mask", "extrude"),

        // -- curve ----------------------------------------------------------
        ToggleCurve => Home::In("curve", "toggle"),
        AddCurvePoint(..) => Home::In("curve", "add_point"),
        SelectCurvePoint(_) => Home::In("curve", "select_point"),
        ToggleCurvePoint(_) => Home::In("curve", "toggle_point"),
        DragCurve(_) => Home::In("curve", "drag"),
        SetCurveRadius(_) => Home::In("curve", "set_radius"),
        SetCurveJoin(_) => Home::In("curve", "set_join"),
        SetCurveProfile(_) => Home::In("curve", "set_profile"),
        RemoveCurvePoints => Home::In("curve", "remove_points"),
        ApplyCurve => Home::In("curve", "apply"),

        // -- shape ----------------------------------------------------------
        ToggleShapes => Home::In("shape", "toggle_picker"),
        SetShape(_) => Home::In("shape", "set"),
        SetShapeParameters(_) => Home::In("shape", "set_parameters"),
        SetInsertAs(_) => Home::In("shape", "set_insert_as"),
        SetMeshOperand(_) => Home::In("shape", "set_mesh_operand"),
        InsertShape => Home::In("shape", "insert"),
        InsertMesh => Home::NotOffered(OPENS_A_PANEL),

        // -- object ---------------------------------------------------------
        SelectObject(_) => Home::In("object", "select"),
        SetObjectShape(..) => Home::In("object", "set_shape"),
        SetObjectCombine(_) => Home::In("object", "set_combine"),
        RemoveObject => Home::In("object", "remove"),

        // -- transform ------------------------------------------------------
        SetGizmoTarget(_) => Home::In("transform", "set_target"),
        SetGizmoMode(_) => Home::In("transform", "set_mode"),
        BeginGizmoDrag(..) => Home::In("transform", "begin_drag"),
        DragGizmo(..) => Home::In("transform", "drag"),
        EndGizmoDrag => Home::In("transform", "end_drag"),

        // -- lattice --------------------------------------------------------
        ToggleLattice => Home::In("lattice", "toggle"),
        SetLatticeDivisions(_) => Home::In("lattice", "set_divisions"),
        SelectLatticePoint(_) => Home::In("lattice", "select_point"),
        ToggleLatticePoint(_) => Home::In("lattice", "toggle_point"),
        SelectLatticePoints(_) => Home::In("lattice", "select_points"),
        DragLatticePoint(_) => Home::In("lattice", "drag"),
        ApplyLattice => Home::In("lattice", "apply"),

        // -- subtool --------------------------------------------------------
        CopySubtool(_) => Home::In("subtool", "copy"),

        // -- boolean --------------------------------------------------------
        ToggleBoolean => Home::In("boolean", "toggle_panel"),
        SetBoolean(_) => Home::In("boolean", "set"),
        RunBoolean => Home::In("boolean", "run"),

        // -- layer ----------------------------------------------------------
        SelectLayer(_) => Home::In("layer", "select"),
        SetLayerVisible(..) => Home::In("layer", "set_visible"),
        SoloLayer(_) => Home::In("layer", "solo"),
        AddLayer(_) => Home::In("layer", "add"),
        RemoveLayer(_) => Home::In("layer", "remove"),
        OptimizeLayer(_) => Home::In("layer", "optimize"),
        RemeshLayer(_) => Home::In("layer", "remesh"),
        SetRemeshSettings(_) => Home::In("layer", "set_remesh"),
        BeginRenameLayer(_) => Home::In("layer", "begin_rename"),
        EditLayerName(_) => Home::In("layer", "edit_name"),
        CommitRenameLayer => Home::In("layer", "commit_rename"),
        CancelRenameLayer => Home::In("layer", "cancel_rename"),
        SetCombine(_) => Home::In("layer", "set_combine"),

        // -- passes and levels ----------------------------------------------
        SculptLayer(_) => Home::In("passes", "grid"),
        MultiresLevel(_) => Home::In("hierarchy", "level"),
        MultiresSculptLayer(_) => Home::In("hierarchy", "pass"),

        // -- document -------------------------------------------------------
        NewDocument => Home::In("document", "new"),
        OpenRecent(_) => Home::In("document", "open"),
        Save => Home::In("document", "save"),
        Quit => Home::In("document", "quit"),
        OpenDocument => {
            Home::NotOffered("this opens a file panel; document.open takes the path instead")
        }
        SaveAs => Home::NotOffered(
            "this opens a file panel; document.save writes where the document already is",
        ),

        // -- exchange -------------------------------------------------------
        ToggleImport => Home::In("exchange", "toggle_import"),
        ToggleExport => Home::In("exchange", "toggle_export"),
        SetImportSettings(_) => Home::In("exchange", "set_import"),
        SetExportSettings(_) => Home::In("exchange", "set_export"),
        RunImport => Home::In("exchange", "run_import"),
        RunExport => Home::In("exchange", "run_export"),

        // -- repair ---------------------------------------------------------
        ToggleRepair => Home::In("repair", "toggle_panel"),
        CloseHoles => Home::In("repair", "close_holes"),
        FillVoids => Home::In("repair", "fill_voids"),

        // -- convert --------------------------------------------------------
        ToggleConvert => Home::In("convert", "toggle_panel"),
        SetConversion(_) => Home::In("convert", "set"),
        RunConversion => Home::In("convert", "run"),

        // -- deform ---------------------------------------------------------
        ToggleDeform => Home::In("deform", "toggle_panel"),
        SetDeform(_) => Home::In("deform", "set"),
        RunDeform => Home::In("deform", "run"),

        // -- armature -------------------------------------------------------
        NewArmature => Home::In("armature", "new"),
        ToggleArmatureEditing => Home::In("armature", "toggle_editing"),
        RemoveZsphere => Home::In("armature", "remove_zsphere"),
        ToggleSkinPreview => Home::In("armature", "toggle_skin_preview"),
        ToggleZsphereNegative => Home::In("armature", "toggle_negative"),
        SetSkinThickness(_) => Home::In("armature", "set_skin_thickness"),
        SelectZsphere(_) => Home::In("armature", "select"),
        AddZsphere { .. } => Home::In("armature", "add"),
        InsertZsphere(_) => Home::In("armature", "insert"),
        MoveZsphere { .. } => Home::In("armature", "move"),
        ResizeZsphere { .. } => Home::In("armature", "resize"),
        ReparentZsphere { .. } => Home::In("armature", "reparent"),

        // -- history --------------------------------------------------------
        Undo => Home::In("history", "undo"),
        Redo => Home::In("history", "redo"),

        // -- view -----------------------------------------------------------
        SetViewPreset(_) => Home::In("view", "set_preset"),
        FrameAll => Home::In("view", "frame_all"),
        NextMaterial => Home::In("view", "next_material"),
        ToggleGrid => Home::In("view", "toggle_grid"),
        TogglePolyframe => Home::In("view", "toggle_polyframe"),
        NextDisplayUnit => Home::In("view", "next_unit"),
        ToggleShading => Home::In("view", "toggle_shading"),
        ToggleCavity => Home::In("view", "toggle_cavity"),
        ToggleShadows => Home::In("view", "toggle_shadows"),
        SetVoxelDisplay(..) => Home::In("view", "set_grid_display"),
        SetSurfaceOpacity(_) => Home::In("view", "set_surface_opacity"),

        // -- reference ------------------------------------------------------
        ToggleReferences => Home::In("reference", "toggle_panel"),
        ClearReference(_) => Home::In("reference", "clear"),
        SetReferenceSettings(..) => Home::In("reference", "set"),
        LoadReference(_) => Home::NotOffered(OPENS_A_PANEL),

        // -- session --------------------------------------------------------
        SetLocale(_) => Home::In("session", "set_language"),

        // -- the door itself ------------------------------------------------
        //
        // Not offered, and this is the sharpest case of the rule rather than
        // an oversight: an agent that could open its own door, shut it, or
        // answer the permission it is being asked for would have made the
        // gate a formality. These three belong to the person at the window.
        ToggleAgentDoor | ShowAgentAccess(_) | AnswerAgentAsk(_) => Home::NotOffered(
            "the door and its permissions belong to the person at the window; an \
             agent that could answer its own consent would not be gated at all",
        ),

        ToggleAttribution => Home::In("session", "toggle_attribution"),
        ToggleDiagnostics => Home::In("session", "toggle_diagnostics"),
        CopyDiagnostics => Home::In("session", "copy_diagnostics"),
    }
}

fn unknown(group: &str, action: &str, offered: &[&str]) -> Refusal {
    Refusal::new(
        RefusalCode::UnknownAction,
        format!(
            "{group} has no action {action}; it offers {}",
            offered.join(", ")
        ),
    )
}

/// The actions a group offers, for a refusal that names them.
pub fn actions_of(group: &str) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = super::table::TABLE
        .iter()
        .filter(|spec| spec.group == group)
        .map(|spec| spec.name)
        .collect();
    found.sort_unstable();
    found
}

/// One tool call, as a command.
pub fn build(group: &str, action: &str, args: &Args<'_>) -> Result<Command, Refusal> {
    use Command as C;
    let command = match (group, action) {
        // -- tool -----------------------------------------------------------
        ("tool", "select") => C::SelectTool(args.choice("tool", &tags::tools())?),

        // -- brush ----------------------------------------------------------
        ("brush", "set_size") => C::SetBrushSize(args.number("size")?),
        ("brush", "set_intensity") => C::SetBrushIntensity(args.number("intensity")?),
        ("brush", "set_flow") => C::SetBrushFlow(args.number("flow")?),
        ("brush", "set_noise") => C::SetBrushNoise(args.number("noise")?),
        ("brush", "set_azimuth") => C::SetBrushAzimuth(args.number("azimuth")?),
        ("brush", "set_falloff") => C::SetBrushFalloff(args.choice("falloff", tags::FALLOFFS)?),
        ("brush", "set_accumulate") => C::SetBrushAccumulate(args.boolean("accumulate")?),
        ("brush", "set_alpha") => C::SetBrushAlpha(args.boolean("alpha")?),
        ("brush", "set_colour") => C::SetBrushColour(Colour::new(args.vec3("rgb")?)),
        ("brush", "set_smoothing") => C::SetBrushSmoothing(args.number("smoothing")?),
        ("brush", "pick_recent_colour") => C::PickRecentColour(args.index("index")?),
        ("brush", "clear_alpha") => C::ClearAlpha,
        ("brush", "toggle_symmetry") => C::ToggleSymmetry(args.choice("axis", tags::AXES)?),

        // -- stroke ---------------------------------------------------------
        ("stroke", "begin") => C::BeginStroke {
            position: args.vec3("at")?,
            pressure: args.number_or("pressure", 1.0)?,
            modifiers: StrokeModifiers {
                smooth: args.boolean_or("smooth", false)?,
                invert: args.boolean_or("invert", false)?,
            },
        },
        ("stroke", "continue") => C::ContinueStroke {
            position: args.vec3("at")?,
            pressure: args.number_or("pressure", 1.0)?,
        },
        ("stroke", "end") => C::EndStroke,
        ("stroke", "cancel") => C::CancelStroke,

        // -- mask -----------------------------------------------------------
        ("mask", "toggle_painting") => C::ToggleMaskPainting,
        ("mask", "apply") => C::ApplyMaskOp(mask_op(args)?),
        ("mask", "set_gesture") => C::SetMaskGesture(args.choice("gesture", tags::GESTURES)?),
        ("mask", "set_steps") => C::SetMaskSteps(args.integer("steps")? as i32),
        ("mask", "begin_outline") => {
            C::BeginMaskOutline(args.vec2("at")?, args.boolean_or("invert", false)?)
        }
        ("mask", "extend_outline") => C::ExtendMaskOutline(args.vec2("at")?),
        ("mask", "end_outline") => C::EndMaskOutline(OutlineFrame {
            origin: args.vec3("origin")?,
            right: args.vec3("right")?,
            up: args.vec3("up")?,
            forward: args.vec3("forward")?,
            scale: args.vec2("scale")?,
        }),
        ("mask", "cancel_outline") => C::CancelMaskOutline,
        ("mask", "set_extrude") => C::SetExtrudeSettings(extrude(args)?),
        ("mask", "extrude") => C::ExtrudeMask(extrude(args)?),

        // -- curve ----------------------------------------------------------
        ("curve", "toggle") => C::ToggleCurve,
        ("curve", "add_point") => {
            C::AddCurvePoint(args.vec3("at")?, args.number_or("radius", 0.1)?)
        }
        ("curve", "select_point") => C::SelectCurvePoint(optional_index(args, "index")?),
        ("curve", "toggle_point") => C::ToggleCurvePoint(args.index("index")?),
        ("curve", "drag") => C::DragCurve(args.vec3("by")?),
        ("curve", "set_radius") => C::SetCurveRadius(args.number("radius")?),
        ("curve", "set_join") => C::SetCurveJoin(args.choice("join", tags::JOINS)?),
        ("curve", "set_profile") => C::SetCurveProfile(args.choice("profile", tags::PROFILES)?),
        ("curve", "remove_points") => C::RemoveCurvePoints,
        ("curve", "apply") => C::ApplyCurve,

        // -- shape ----------------------------------------------------------
        ("shape", "toggle_picker") => C::ToggleShapes,
        ("shape", "set") => C::SetShape(args.choice("shape", &tags::shapes())?),
        ("shape", "set_parameters") => C::SetShapeParameters(args.number_list("parameters")?),
        ("shape", "set_insert_as") => C::SetInsertAs(args.choice("as", tags::INSERT_AS)?),
        ("shape", "set_mesh_operand") => {
            C::SetMeshOperand(args.optional_layer("layer")?.map(LayerKey))
        }
        ("shape", "insert") => C::InsertShape,

        // -- object ---------------------------------------------------------
        ("object", "select") => C::SelectObject(optional_object(args)?),
        ("object", "set_shape") => C::SetObjectShape(
            args.choice("shape", &tags::shapes())?,
            args.number_list_or_empty("parameters")?,
        ),
        ("object", "set_combine") => C::SetObjectCombine(combine(args)?),
        ("object", "remove") => C::RemoveObject,

        // -- transform ------------------------------------------------------
        ("transform", "set_target") => C::SetGizmoTarget(gizmo_target(args)?),
        ("transform", "set_mode") => C::SetGizmoMode(args.choice("mode", tags::GIZMO_MODES)?),
        ("transform", "begin_drag") => C::BeginGizmoDrag(
            gizmo_handle(args)?,
            args.vec3("anchor")?,
            args.vec3_or("view_axis", [0.0, 0.0, 1.0])?,
        ),
        ("transform", "drag") => C::DragGizmo(args.vec3("at")?, args.boolean_or("invert", false)?),
        ("transform", "end_drag") => C::EndGizmoDrag,

        // -- lattice --------------------------------------------------------
        ("lattice", "toggle") => C::ToggleLattice,
        ("lattice", "set_divisions") => C::SetLatticeDivisions(args.ivec3("divisions")?),
        ("lattice", "select_point") => C::SelectLatticePoint(optional_index(args, "index")?),
        ("lattice", "toggle_point") => C::ToggleLatticePoint(args.index("index")?),
        ("lattice", "select_points") => C::SelectLatticePoints(args.index_list("indices")?),
        ("lattice", "drag") => C::DragLatticePoint(args.vec3("to")?),
        ("lattice", "apply") => C::ApplyLattice,

        // -- subtool --------------------------------------------------------
        ("subtool", "copy") => C::CopySubtool(LayerKey(args.layer("layer")?)),

        // -- boolean --------------------------------------------------------
        ("boolean", "toggle_panel") => C::ToggleBoolean,
        ("boolean", "set") => C::SetBoolean(BooleanSettings {
            base: args.optional_layer("base")?.map(LayerKey),
            tool: args.optional_layer("tool")?.map(LayerKey),
            op: args.choice_or("op", &tags::booleans(), BooleanSettings::default().op)?,
            cell_size: args.number_or("cell_size", BooleanSettings::default().cell_size)?,
            consume: args.boolean_or("consume", BooleanSettings::default().consume)?,
        }),
        ("boolean", "run") => C::RunBoolean,

        // -- layer ----------------------------------------------------------
        ("layer", "select") => C::SelectLayer(LayerKey(args.layer("layer")?)),
        ("layer", "set_visible") => {
            C::SetLayerVisible(LayerKey(args.layer("layer")?), args.boolean("visible")?)
        }
        ("layer", "solo") => C::SoloLayer(args.optional_layer("layer")?.map(LayerKey)),
        ("layer", "add") => C::AddLayer(args.choice("representation", tags::REPRESENTATIONS)?),
        ("layer", "remove") => C::RemoveLayer(LayerKey(args.layer("layer")?)),
        ("layer", "optimize") => C::OptimizeLayer(LayerKey(args.layer("layer")?)),
        ("layer", "remesh") => C::RemeshLayer(LayerKey(args.layer("layer")?)),
        ("layer", "set_remesh") => C::SetRemeshSettings(RemeshSettings {
            resolution: args
                .integer_or("resolution", RemeshSettings::default().resolution as i64)?
                as u32,
            sharp: args.boolean_or("sharp", RemeshSettings::default().sharp)?,
            remove_loose_pieces: args.boolean_or(
                "remove_loose_pieces",
                RemeshSettings::default().remove_loose_pieces,
            )?,
            follow_the_source: args.boolean_or(
                "follow_the_source",
                RemeshSettings::default().follow_the_source,
            )?,
        }),
        ("layer", "begin_rename") => C::BeginRenameLayer(LayerKey(args.layer("layer")?)),
        ("layer", "edit_name") => C::EditLayerName(args.text("name")?),
        ("layer", "commit_rename") => C::CommitRenameLayer,
        ("layer", "cancel_rename") => C::CancelRenameLayer,
        ("layer", "set_combine") => C::SetCombine(combine(args)?),

        // -- passes and levels ----------------------------------------------
        ("passes", "grid") => C::SculptLayer(sculpt_layer_op(args)?),
        ("hierarchy", "level") => C::MultiresLevel(level_op(args)?),
        ("hierarchy", "pass") => C::MultiresSculptLayer(multires_pass_op(args)?),

        // -- document -------------------------------------------------------
        ("document", "new") => C::NewDocument,
        ("document", "open") => C::OpenRecent(args.text("path")?.into()),
        ("document", "save") => C::Save,
        ("document", "quit") => C::Quit,

        // -- exchange -------------------------------------------------------
        ("exchange", "toggle_import") => C::ToggleImport,
        ("exchange", "toggle_export") => C::ToggleExport,
        ("exchange", "set_import") => C::SetImportSettings(ImportSettings {
            becomes: args.choice_or(
                "becomes",
                tags::IMPORT_AS,
                ImportSettings::default().becomes,
            )?,
            scale: args.number_or("scale", ImportSettings::default().scale)?,
            max_vertices: args.integer_or(
                "max_vertices",
                ImportSettings::default().max_vertices as i64,
            )? as u64,
            max_triangles: args.integer_or(
                "max_triangles",
                ImportSettings::default().max_triangles as i64,
            )? as u64,
        }),
        ("exchange", "set_export") => C::SetExportSettings(ExportSettings {
            mesher: args.choice_or("mesher", tags::MESHERS, ExportSettings::default().mesher)?,
            resolution: args.number_or("resolution", ExportSettings::default().resolution)?,
            decimate_to: match args.number_or("decimate_to", f32::NAN)? {
                value if value.is_nan() => None,
                value => Some(value),
            },
        }),
        ("exchange", "run_import") => C::RunImport,
        ("exchange", "run_export") => C::RunExport,

        // -- repair ---------------------------------------------------------
        ("repair", "toggle_panel") => C::ToggleRepair,
        ("repair", "close_holes") => C::CloseHoles,
        ("repair", "fill_voids") => C::FillVoids,

        // -- convert --------------------------------------------------------
        ("convert", "toggle_panel") => C::ToggleConvert,
        ("convert", "set") => C::SetConversion(ConversionSettings {
            direction: args.choice_or(
                "direction",
                tags::DIRECTIONS,
                ConversionSettings::default().direction,
            )?,
            cell_size: args.number_or("cell_size", ConversionSettings::default().cell_size)?,
            blur: args.integer_or("blur", ConversionSettings::default().blur as i64)? as i32,
            in_place: args.boolean_or("in_place", ConversionSettings::default().in_place)?,
        }),
        ("convert", "run") => C::RunConversion,

        // -- deform ---------------------------------------------------------
        ("deform", "toggle_panel") => C::ToggleDeform,
        ("deform", "set") => C::SetDeform(DeformSettings {
            verb: args.choice_or("verb", tags::DEFORM_VERBS, DeformSettings::default().verb)?,
            axis: args.vec3_or("axis", DeformSettings::default().axis)?,
            span: args.number_or("span", DeformSettings::default().span)?,
            scale_start: args.number_or("scale_start", DeformSettings::default().scale_start)?,
            scale_end: args.number_or("scale_end", DeformSettings::default().scale_end)?,
            degrees: args.number_or("degrees", DeformSettings::default().degrees)?,
        }),
        ("deform", "run") => C::RunDeform,

        // -- armature -------------------------------------------------------
        ("armature", "new") => C::NewArmature,
        ("armature", "toggle_editing") => C::ToggleArmatureEditing,
        ("armature", "remove_zsphere") => C::RemoveZsphere,
        ("armature", "toggle_skin_preview") => C::ToggleSkinPreview,
        ("armature", "toggle_negative") => C::ToggleZsphereNegative,
        ("armature", "set_skin_thickness") => C::SetSkinThickness(args.number("thickness")?),
        ("armature", "select") => C::SelectZsphere(match args.integer_or("sphere", -1)? {
            index if index < 0 => None,
            index => Some(index as u32),
        }),
        ("armature", "add") => C::AddZsphere {
            parent: args.integer("parent")? as u32,
            at: args.vec3("at")?,
            radius: match args.number_or("radius", f32::NAN)? {
                radius if radius.is_nan() => None,
                radius => Some(radius),
            },
        },
        ("armature", "insert") => C::InsertZsphere(args.integer("sphere")? as u32),
        ("armature", "move") => C::MoveZsphere {
            index: args.integer("sphere")? as u32,
            to: args.vec3("to")?,
        },
        ("armature", "resize") => C::ResizeZsphere {
            index: args.integer("sphere")? as u32,
            radius: args.number("radius")?,
        },
        ("armature", "reparent") => C::ReparentZsphere {
            index: args.integer("sphere")? as u32,
            parent: args.integer("parent")? as u32,
        },

        // -- history --------------------------------------------------------
        ("history", "undo") => C::Undo,
        ("history", "redo") => C::Redo,

        // -- view -----------------------------------------------------------
        ("view", "set_preset") => C::SetViewPreset(args.choice("preset", tags::VIEW_PRESETS)?),
        ("view", "frame_all") => C::FrameAll,
        ("view", "next_material") => C::NextMaterial,
        ("view", "toggle_grid") => C::ToggleGrid,
        ("view", "toggle_polyframe") => C::TogglePolyframe,
        ("view", "next_unit") => C::NextDisplayUnit,
        ("view", "toggle_shading") => C::ToggleShading,
        ("view", "toggle_cavity") => C::ToggleCavity,
        ("view", "toggle_shadows") => C::ToggleShadows,
        ("view", "set_grid_display") => C::SetVoxelDisplay(
            args.choice("display", tags::VOXEL_DISPLAYS)?,
            SmoothBlur::new(args.integer_or("blur_passes", 0)? as i32),
        ),
        ("view", "set_surface_opacity") => {
            C::SetSurfaceOpacity(SurfaceOpacity::new(args.number("opacity")?))
        }

        // -- reference ------------------------------------------------------
        ("reference", "toggle_panel") => C::ToggleReferences,
        ("reference", "clear") => C::ClearReference(args.choice("plane", &tags::planes())?),
        ("reference", "set") => C::SetReferenceSettings(
            args.choice("plane", &tags::planes())?,
            ReferenceSettings {
                visible: args.boolean_or("visible", ReferenceSettings::default().visible)?,
                opacity: args.number_or("opacity", ReferenceSettings::default().opacity)?,
                height: args.number_or("height", ReferenceSettings::default().height)?,
                offset: {
                    let fallback = ReferenceSettings::default().offset;
                    match args.vec2("offset") {
                        Ok(offset) => offset,
                        Err(_) => fallback,
                    }
                },
                depth: args.number_or("depth", ReferenceSettings::default().depth)?,
            },
        ),

        // -- session --------------------------------------------------------
        ("session", "set_language") => C::SetLocale(args.choice("language", &tags::locales())?),
        ("session", "toggle_attribution") => C::ToggleAttribution,
        ("session", "toggle_diagnostics") => C::ToggleDiagnostics,
        ("session", "copy_diagnostics") => C::CopyDiagnostics,

        _ => return Err(unknown(group, action, &actions_of(group))),
    };
    Ok(command)
}

fn optional_index(args: &Args<'_>, name: &str) -> Result<Option<usize>, Refusal> {
    match args.integer_or(name, -1)? {
        value if value < 0 => Ok(None),
        value => Ok(Some(value as usize)),
    }
}

fn optional_object(args: &Args<'_>) -> Result<Option<ObjectId>, Refusal> {
    match args.optional_layer("layer")? {
        None => Ok(None),
        Some(layer) => Ok(Some(ObjectId {
            layer: LayerKey(layer),
            node: args.integer("node")? as u32,
        })),
    }
}

fn gizmo_target(args: &Args<'_>) -> Result<Option<GizmoTarget>, Refusal> {
    const TARGETS: &[(&str, u8)] = &[("none", 0), ("object", 1), ("layer", 2), ("curve", 3)];
    match args.choice("target", TARGETS)? {
        0 => Ok(None),
        1 => Ok(Some(GizmoTarget::Object(ObjectId {
            layer: LayerKey(args.layer("layer")?),
            node: args.integer("node")? as u32,
        }))),
        2 => Ok(Some(GizmoTarget::Layer(LayerKey(args.layer("layer")?)))),
        _ => Ok(Some(GizmoTarget::Curve)),
    }
}

fn gizmo_handle(args: &Args<'_>) -> Result<GizmoHandle, Refusal> {
    const HANDLES: &[(&str, u8)] = &[("view", 0), ("x", 1), ("y", 2), ("z", 3), ("centre", 4)];
    Ok(match args.choice("handle", HANDLES)? {
        0 => GizmoHandle::View,
        1 => GizmoHandle::Axis(0),
        2 => GizmoHandle::Axis(1),
        3 => GizmoHandle::Axis(2),
        _ => GizmoHandle::Centre,
    })
}

fn mask_op(args: &Args<'_>) -> Result<MaskOp, Refusal> {
    const OPS: &[(&str, u8)] = &[
        ("invert", 0),
        ("clear", 1),
        ("expand", 2),
        ("contract", 3),
        ("smooth", 4),
        ("invert_within_bounds", 5),
    ];
    let steps = || args.integer_or("steps", 1).map(|value| value as i32);
    Ok(match args.choice("op", OPS)? {
        0 => MaskOp::Invert,
        1 => MaskOp::Clear,
        2 => MaskOp::Expand(steps()?),
        3 => MaskOp::Contract(steps()?),
        4 => MaskOp::Smooth(steps()?),
        _ => MaskOp::InvertWithinBounds,
    })
}

fn extrude(args: &Args<'_>) -> Result<ExtrudeSettings, Refusal> {
    let fallback = ExtrudeSettings::default();
    Ok(ExtrudeSettings {
        thickness: args.number_or("thickness", fallback.thickness)?,
        side: args.choice_or("side", tags::EXTRUDE_SIDES, fallback.side)?,
        border_round: args.number_or("border_round", fallback.border_round)?,
        border_smooth: args.integer_or("border_smooth", fallback.border_smooth as i64)? as i32,
    })
}

fn combine(args: &Args<'_>) -> Result<CombineSettings, Refusal> {
    let fallback = CombineSettings::default();
    Ok(CombineSettings {
        op: args.choice_or("op", &tags::combines(), fallback.op)?,
        blend: args.choice_or("blend", &tags::blends(), fallback.blend)?,
        radius: args.number_or("radius", fallback.radius)?,
    })
}

fn sculpt_layer_op(args: &Args<'_>) -> Result<SculptLayerOp, Refusal> {
    const OPS: &[(&str, u8)] = &[
        ("begin_recording", 0),
        ("end_recording", 1),
        ("set_strength", 2),
        ("set_visible", 3),
        ("remove", 4),
        ("merge_down", 5),
        ("move", 6),
    ];
    Ok(match args.choice("op", OPS)? {
        0 => SculptLayerOp::BeginRecording {
            name: args.text("name")?,
        },
        1 => SculptLayerOp::EndRecording,
        2 => SculptLayerOp::SetStrength {
            index: args.index("index")?,
            strength: args.number("strength")?,
        },
        3 => SculptLayerOp::SetVisible {
            index: args.index("index")?,
            visible: args.boolean("visible")?,
        },
        4 => SculptLayerOp::Remove {
            index: args.index("index")?,
        },
        5 => SculptLayerOp::MergeDown {
            index: args.index("index")?,
        },
        _ => SculptLayerOp::Move {
            from: args.index("from")?,
            to: args.index("to")?,
        },
    })
}

fn level_op(args: &Args<'_>) -> Result<MultiresLevelOp, Refusal> {
    const OPS: &[(&str, u8)] = &[
        ("set_sculpt_level", 0),
        ("set_display_level", 1),
        ("subdivide", 2),
        ("remove_highest", 3),
    ];
    Ok(match args.choice("op", OPS)? {
        0 => MultiresLevelOp::SetSculptLevel(args.integer("level")? as u32),
        1 => MultiresLevelOp::SetDisplayLevel(args.integer("level")? as u32),
        2 => MultiresLevelOp::AddLevel,
        _ => MultiresLevelOp::RemoveHighestLevel,
    })
}

fn multires_pass_op(args: &Args<'_>) -> Result<MultiresSculptLayerOp, Refusal> {
    const OPS: &[(&str, u8)] = &[
        ("add", 0),
        ("rename", 1),
        ("set_strength", 2),
        ("set_visible", 3),
        ("set_locked", 4),
        ("set_active", 5),
        ("move", 6),
        ("remove", 7),
        ("merge_down", 8),
        ("bake_to_base", 9),
        ("compact", 10),
    ];
    let id = || args.layer("id").map(MultiresSculptLayerId::new);
    Ok(match args.choice("op", OPS)? {
        0 => MultiresSculptLayerOp::Add {
            name: args.text("name")?,
        },
        1 => MultiresSculptLayerOp::Rename {
            id: id()?,
            name: args.text("name")?,
        },
        2 => MultiresSculptLayerOp::SetStrength {
            id: id()?,
            strength: args.number("strength")?,
        },
        3 => MultiresSculptLayerOp::SetVisible {
            id: id()?,
            visible: args.boolean("visible")?,
        },
        4 => MultiresSculptLayerOp::SetLocked {
            id: id()?,
            locked: args.boolean("locked")?,
        },
        5 => MultiresSculptLayerOp::SetActive { id: id()? },
        6 => MultiresSculptLayerOp::Move {
            id: id()?,
            to: args.index("to")?,
        },
        7 => MultiresSculptLayerOp::Remove { id: id()? },
        8 => MultiresSculptLayerOp::MergeDown { id: id()? },
        9 => MultiresSculptLayerOp::BakeToBase { id: id()? },
        _ => MultiresSculptLayerOp::Compact,
    })
}
