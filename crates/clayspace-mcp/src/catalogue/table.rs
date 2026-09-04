//! What each group offers, and what each action takes.
//!
//! This is the table `describe` answers from and the table the schemas are
//! generated from. It is checked against [`super::actions`] in both
//! directions: every entry here must build a command, and every command must
//! land back on the entry that built it. A row that drifts from the builder is
//! a row an agent is misled by, and the round trip is what stops that being
//! possible.

use serde_json::{json, Map, Value};

use super::tags;

#[derive(Clone, Copy)]
pub enum Kind {
    Number,
    Integer,
    Boolean,
    Text,
    /// Text that names a file on this machine.
    Path,
    Vec2,
    Vec3,
    IVec3,
    Numbers,
    Indices,
    /// One of a named set, which `describe` and the schema both spell out.
    Choice(fn() -> Vec<&'static str>),
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::Integer => "whole number",
            Self::Boolean => "true or false",
            Self::Text => "text",
            Self::Path => "a path on this machine",
            Self::Vec2 => "two numbers",
            Self::Vec3 => "three numbers",
            Self::IVec3 => "three whole numbers",
            Self::Numbers => "a list of numbers",
            Self::Indices => "a list of whole numbers",
            Self::Choice(_) => "one of a set",
        }
    }

    pub fn choices(self) -> Option<Vec<&'static str>> {
        match self {
            Self::Choice(of) => Some(of()),
            _ => None,
        }
    }

    /// The JSON Schema fragment for this kind.
    pub fn schema(self) -> Value {
        match self {
            Self::Number => json!({ "type": "number" }),
            Self::Integer => json!({ "type": "integer" }),
            Self::Boolean => json!({ "type": "boolean" }),
            Self::Text | Self::Path => json!({ "type": "string" }),
            Self::Vec2 => json!({
                "type": "array", "items": { "type": "number" },
                "minItems": 2, "maxItems": 2,
            }),
            Self::Vec3 => json!({
                "type": "array", "items": { "type": "number" },
                "minItems": 3, "maxItems": 3,
            }),
            Self::IVec3 => json!({
                "type": "array", "items": { "type": "integer" },
                "minItems": 3, "maxItems": 3,
            }),
            Self::Numbers => json!({ "type": "array", "items": { "type": "number" } }),
            Self::Indices => json!({ "type": "array", "items": { "type": "integer" } }),
            Self::Choice(of) => json!({ "type": "string", "enum": of() }),
        }
    }
}

pub struct Arg {
    pub name: &'static str,
    pub kind: Kind,
    pub required: bool,
    pub about: &'static str,
}

pub struct ActionSpec {
    pub group: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub arguments: &'static [Arg],
    /// Arguments that build this action, as JSON. Shown by `describe`, and
    /// used by the round-trip test that keeps this table and the builder from
    /// drifting apart.
    pub example: &'static str,
}

impl ActionSpec {
    pub fn to_json(&self) -> Value {
        let arguments: Vec<Value> = self
            .arguments
            .iter()
            .map(|arg| {
                let mut object = Map::new();
                object.insert("name".into(), json!(arg.name));
                object.insert("kind".into(), json!(arg.kind.word()));
                object.insert("required".into(), json!(arg.required));
                object.insert("about".into(), json!(arg.about));
                if let Some(choices) = arg.kind.choices() {
                    object.insert("choices".into(), json!(choices));
                }
                Value::Object(object)
            })
            .collect();
        json!({
            "action": self.name,
            "summary": self.summary,
            "arguments": arguments,
            "example": serde_json::from_str::<Value>(self.example).unwrap_or(json!({})),
        })
    }
}

const fn r(name: &'static str, kind: Kind, about: &'static str) -> Arg {
    Arg {
        name,
        kind,
        required: true,
        about,
    }
}

const fn o(name: &'static str, kind: Kind, about: &'static str) -> Arg {
    Arg {
        name,
        kind,
        required: false,
        about,
    }
}

fn tools() -> Vec<&'static str> {
    tags::tags_of(&tags::tools())
}
fn shapes() -> Vec<&'static str> {
    tags::tags_of(&tags::shapes())
}
fn combines() -> Vec<&'static str> {
    tags::tags_of(&tags::combines())
}
fn blends() -> Vec<&'static str> {
    tags::tags_of(&tags::blends())
}
fn booleans() -> Vec<&'static str> {
    tags::tags_of(&tags::booleans())
}
fn planes() -> Vec<&'static str> {
    tags::tags_of(&tags::planes())
}
fn locales() -> Vec<&'static str> {
    tags::tags_of(&tags::locales())
}
fn falloffs() -> Vec<&'static str> {
    tags::tags_of(tags::FALLOFFS)
}
fn axes() -> Vec<&'static str> {
    tags::tags_of(tags::AXES)
}
fn gestures() -> Vec<&'static str> {
    tags::tags_of(tags::GESTURES)
}
fn joins() -> Vec<&'static str> {
    tags::tags_of(tags::JOINS)
}
fn profiles() -> Vec<&'static str> {
    tags::tags_of(tags::PROFILES)
}
fn insert_as() -> Vec<&'static str> {
    tags::tags_of(tags::INSERT_AS)
}
fn representations() -> Vec<&'static str> {
    tags::tags_of(tags::REPRESENTATIONS)
}
fn gizmo_modes() -> Vec<&'static str> {
    tags::tags_of(tags::GIZMO_MODES)
}
fn view_presets() -> Vec<&'static str> {
    tags::tags_of(tags::VIEW_PRESETS)
}
fn voxel_displays() -> Vec<&'static str> {
    tags::tags_of(tags::VOXEL_DISPLAYS)
}
fn directions() -> Vec<&'static str> {
    tags::tags_of(tags::DIRECTIONS)
}
fn import_as() -> Vec<&'static str> {
    tags::tags_of(tags::IMPORT_AS)
}
fn meshers() -> Vec<&'static str> {
    tags::tags_of(tags::MESHERS)
}
fn extrude_sides() -> Vec<&'static str> {
    tags::tags_of(tags::EXTRUDE_SIDES)
}
fn deform_verbs() -> Vec<&'static str> {
    tags::tags_of(tags::DEFORM_VERBS)
}
fn mask_ops() -> Vec<&'static str> {
    vec![
        "invert",
        "clear",
        "expand",
        "contract",
        "smooth",
        "invert_within_bounds",
    ]
}
fn gizmo_targets() -> Vec<&'static str> {
    vec!["none", "object", "layer", "curve"]
}
fn gizmo_handles() -> Vec<&'static str> {
    vec!["view", "x", "y", "z", "centre"]
}
fn pass_ops() -> Vec<&'static str> {
    vec![
        "begin_recording",
        "end_recording",
        "set_strength",
        "set_visible",
        "remove",
        "merge_down",
        "move",
    ]
}
fn level_ops() -> Vec<&'static str> {
    vec![
        "set_sculpt_level",
        "set_display_level",
        "subdivide",
        "remove_highest",
    ]
}
fn multires_pass_ops() -> Vec<&'static str> {
    vec![
        "add",
        "rename",
        "set_strength",
        "set_visible",
        "set_locked",
        "set_active",
        "move",
        "remove",
        "merge_down",
        "bake_to_base",
        "compact",
    ]
}

/// What each group is for, in the words `tools/list` shows.
pub const GROUPS: &[(&str, &str, &str)] = &[
    ("tool", "Ferramenta", "Which sculpting tool the brush is."),
    (
        "brush",
        "Pincel",
        "The brush's size, strength, shaping and colour, and symmetry.",
    ),
    (
        "stroke",
        "Traço",
        "A sculpting gesture: begin, continue, end. One gesture is one undo step.",
    ),
    (
        "mask",
        "Máscara",
        "Freezing and thawing parts of a surface, by op or by outline.",
    ),
    ("curve", "Curva", "A swept form along control points."),
    (
        "shape",
        "Forma",
        "The shape picker: what to place, and where it lands.",
    ),
    (
        "object",
        "Objeto",
        "A placed object within a layer: its shape and how it combines.",
    ),
    (
        "transform",
        "Manipulador",
        "Moving, rotating and scaling the selected target.",
    ),
    ("lattice", "Gaiola", "The deformation cage over a layer."),
    ("subtool", "Subferramenta", "Whole subtools."),
    (
        "boolean",
        "Booleana",
        "Resolving two subtools into a third.",
    ),
    (
        "layer",
        "Camada",
        "The scene's layers: selection, visibility, remeshing, naming.",
    ),
    ("passes", "Passes", "The pass stack on a grid layer."),
    (
        "hierarchy",
        "Hierarquia",
        "A subdivision hierarchy's levels and its pass stack.",
    ),
    ("document", "Documento", "New, open, save and quit."),
    ("exchange", "Troca", "Import and export."),
    ("repair", "Reparo", "Closing holes and filling voids."),
    ("convert", "Conversão", "Crossing between representations."),
    (
        "deform",
        "Deformação",
        "Taper and twist over a whole layer.",
    ),
    ("armature", "Esqueleto", "ZSpheres and the skin preview."),
    ("history", "Histórico", "Undo and redo."),
    (
        "view",
        "Vista",
        "How the scene is drawn and from which preset.",
    ),
    (
        "reference",
        "Referência",
        "Reference images on the three planes.",
    ),
    (
        "session",
        "Sessão",
        "Language, diagnostics and attribution.",
    ),
];

/// Every action, in every group.
pub const TABLE: &[ActionSpec] = &[
    // -- tool ---------------------------------------------------------------
    ActionSpec {
        group: "tool",
        name: "select",
        summary: "Takes up a sculpting tool.",
        arguments: &[r("tool", Kind::Choice(tools), "which tool")],
        example: r#"{"tool":"clay"}"#,
    },
    // -- brush --------------------------------------------------------------
    ActionSpec {
        group: "brush",
        name: "set_size",
        summary: "The brush's radius, in world units.",
        arguments: &[r("size", Kind::Number, "the radius")],
        example: r#"{"size":0.12}"#,
    },
    ActionSpec {
        group: "brush",
        name: "set_intensity",
        summary: "How hard a dab bites, 0..=1.",
        arguments: &[r("intensity", Kind::Number, "0 to 1")],
        example: r#"{"intensity":0.5}"#,
    },
    ActionSpec {
        group: "brush",
        name: "set_flow",
        summary: "How much of the intensity each dab along the stroke carries.",
        arguments: &[r("flow", Kind::Number, "0 to 1")],
        example: r#"{"flow":0.8}"#,
    },
    ActionSpec {
        group: "brush",
        name: "set_noise",
        summary: "How much the dab is broken up.",
        arguments: &[r("noise", Kind::Number, "0 to 1")],
        example: r#"{"noise":0.2}"#,
    },
    ActionSpec {
        group: "brush",
        name: "set_azimuth",
        summary: "The direction a directional tool works in, in degrees.",
        arguments: &[r("azimuth", Kind::Number, "degrees")],
        example: r#"{"azimuth":45.0}"#,
    },
    ActionSpec {
        group: "brush",
        name: "set_falloff",
        summary: "How a dab fades from its centre to its rim.",
        arguments: &[r("falloff", Kind::Choice(falloffs), "which curve")],
        example: r#"{"falloff":"smooth"}"#,
    },
    ActionSpec {
        group: "brush",
        name: "set_accumulate",
        summary: "Whether dabs pile up where a stroke crosses itself.",
        arguments: &[r("accumulate", Kind::Boolean, "on or off")],
        example: r#"{"accumulate":true}"#,
    },
    ActionSpec {
        group: "brush",
        name: "set_alpha",
        summary: "Whether the loaded alpha stamp shapes the dab.",
        arguments: &[r("alpha", Kind::Boolean, "on or off")],
        example: r#"{"alpha":false}"#,
    },
    ActionSpec {
        group: "brush",
        name: "set_colour",
        summary: "The colour a painting tool lays down.",
        arguments: &[r("rgb", Kind::Vec3, "red, green and blue, each 0 to 1")],
        example: r#"{"rgb":[0.8,0.3,0.2]}"#,
    },
    ActionSpec {
        group: "brush",
        name: "set_smoothing",
        summary: "How much the surface is relaxed as the stroke passes.",
        arguments: &[r("smoothing", Kind::Number, "0 to 1")],
        example: r#"{"smoothing":0.1}"#,
    },
    ActionSpec {
        group: "brush",
        name: "pick_recent_colour",
        summary: "Takes a colour back off the recent row.",
        arguments: &[r("index", Kind::Integer, "which of the recent colours")],
        example: r#"{"index":0}"#,
    },
    ActionSpec {
        group: "brush",
        name: "clear_alpha",
        summary: "Puts the alpha stamp away.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "brush",
        name: "toggle_symmetry",
        summary: "Turns mirroring across an axis on or off.",
        arguments: &[r("axis", Kind::Choice(axes), "which axis")],
        example: r#"{"axis":"x"}"#,
    },
    // -- stroke -------------------------------------------------------------
    ActionSpec {
        group: "stroke",
        name: "begin",
        summary: "Starts a gesture. The whole gesture becomes one undo step.",
        arguments: &[
            r("at", Kind::Vec3, "where, in world coordinates"),
            o("pressure", Kind::Number, "0 to 1; 1 where none is given"),
            o("smooth", Kind::Boolean, "hold the smoothing modifier"),
            o("invert", Kind::Boolean, "hold the inverting modifier"),
        ],
        example: r#"{"at":[0.0,0.1,0.5],"pressure":1.0}"#,
    },
    ActionSpec {
        group: "stroke",
        name: "continue",
        summary: "Adds a sample to the gesture in progress.",
        arguments: &[
            r("at", Kind::Vec3, "where, in world coordinates"),
            o("pressure", Kind::Number, "0 to 1"),
        ],
        example: r#"{"at":[0.05,0.1,0.5]}"#,
    },
    ActionSpec {
        group: "stroke",
        name: "end",
        summary: "Closes the gesture and applies it as one edit.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "stroke",
        name: "cancel",
        summary: "Drops the gesture without applying it.",
        arguments: &[],
        example: "{}",
    },
    // -- mask ---------------------------------------------------------------
    ActionSpec {
        group: "mask",
        name: "toggle_painting",
        summary: "Whether the pointer paints mask rather than sculpting.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "mask",
        name: "apply",
        summary: "An operation over the whole mask.",
        arguments: &[
            r("op", Kind::Choice(mask_ops), "which operation"),
            o(
                "steps",
                Kind::Integer,
                "how many, for expand, contract and smooth",
            ),
        ],
        example: r#"{"op":"expand","steps":2}"#,
    },
    ActionSpec {
        group: "mask",
        name: "set_gesture",
        summary: "Whether masking is brushed or drawn as an outline.",
        arguments: &[r("gesture", Kind::Choice(gestures), "which gesture")],
        example: r#"{"gesture":"lasso"}"#,
    },
    ActionSpec {
        group: "mask",
        name: "set_steps",
        summary: "The default number of steps for a mask operation.",
        arguments: &[r("steps", Kind::Integer, "how many")],
        example: r#"{"steps":2}"#,
    },
    ActionSpec {
        group: "mask",
        name: "begin_outline",
        summary: "Starts an outline, in the frame's own two coordinates.",
        arguments: &[
            r("at", Kind::Vec2, "where on the frame"),
            o("invert", Kind::Boolean, "thaw rather than freeze"),
        ],
        example: r#"{"at":[0.1,0.2]}"#,
    },
    ActionSpec {
        group: "mask",
        name: "extend_outline",
        summary: "Adds a point to the outline being drawn.",
        arguments: &[r("at", Kind::Vec2, "where on the frame")],
        example: r#"{"at":[0.3,0.2]}"#,
    },
    ActionSpec {
        group: "mask",
        name: "end_outline",
        summary: "Closes the outline and applies it through a frame in the world.",
        arguments: &[
            r("origin", Kind::Vec3, "the frame's origin"),
            r("right", Kind::Vec3, "the frame's right"),
            r("up", Kind::Vec3, "the frame's up"),
            r("forward", Kind::Vec3, "the frame's forward"),
            r("scale", Kind::Vec2, "the frame's extent"),
        ],
        example: r#"{"origin":[0,0,0],"right":[1,0,0],"up":[0,1,0],"forward":[0,0,1],"scale":[1,1]}"#,
    },
    ActionSpec {
        group: "mask",
        name: "cancel_outline",
        summary: "Drops the outline being drawn.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "mask",
        name: "set_extrude",
        summary: "What an extrusion of the mask would do, without doing it.",
        arguments: &[
            o("thickness", Kind::Number, "how far"),
            o("side", Kind::Choice(extrude_sides), "which way"),
            o("border_round", Kind::Number, "how round the border is"),
            o("border_smooth", Kind::Integer, "how many smoothing passes"),
        ],
        example: r#"{"thickness":0.05,"side":"outward"}"#,
    },
    ActionSpec {
        group: "mask",
        name: "extrude",
        summary: "Pushes the masked region out or in, as one edit.",
        arguments: &[
            o("thickness", Kind::Number, "how far"),
            o("side", Kind::Choice(extrude_sides), "which way"),
            o("border_round", Kind::Number, "how round the border is"),
            o("border_smooth", Kind::Integer, "how many smoothing passes"),
        ],
        example: r#"{"thickness":0.05}"#,
    },
    // -- curve --------------------------------------------------------------
    ActionSpec {
        group: "curve",
        name: "toggle",
        summary: "Starts a curve, or takes the one that is up down.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "curve",
        name: "add_point",
        summary: "Appends a control point.",
        arguments: &[
            r("at", Kind::Vec3, "where"),
            o("radius", Kind::Number, "how thick the tube is there"),
        ],
        example: r#"{"at":[0,0,0],"radius":0.1}"#,
    },
    ActionSpec {
        group: "curve",
        name: "select_point",
        summary: "Selects one control point, or clears the selection.",
        arguments: &[o("index", Kind::Integer, "which; omit to clear")],
        example: r#"{"index":0}"#,
    },
    ActionSpec {
        group: "curve",
        name: "toggle_point",
        summary: "Adds or removes one point from the selection.",
        arguments: &[r("index", Kind::Integer, "which")],
        example: r#"{"index":1}"#,
    },
    ActionSpec {
        group: "curve",
        name: "drag",
        summary: "Moves every selected point by a displacement.",
        arguments: &[r("by", Kind::Vec3, "the displacement")],
        example: r#"{"by":[0.0,0.1,0.0]}"#,
    },
    ActionSpec {
        group: "curve",
        name: "set_radius",
        summary: "How thick the tube is at the selected points.",
        arguments: &[r("radius", Kind::Number, "the radius")],
        example: r#"{"radius":0.08}"#,
    },
    ActionSpec {
        group: "curve",
        name: "set_join",
        summary: "How the curve passes through its points.",
        arguments: &[r("join", Kind::Choice(joins), "which join")],
        example: r#"{"join":"through"}"#,
    },
    ActionSpec {
        group: "curve",
        name: "set_profile",
        summary: "The cross-section swept along the curve.",
        arguments: &[r("profile", Kind::Choice(profiles), "which profile")],
        example: r#"{"profile":"circle"}"#,
    },
    ActionSpec {
        group: "curve",
        name: "remove_points",
        summary: "Takes the selected control points away.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "curve",
        name: "apply",
        summary: "Leaves the swept form and takes the curve down.",
        arguments: &[],
        example: "{}",
    },
    // -- shape --------------------------------------------------------------
    ActionSpec {
        group: "shape",
        name: "toggle_picker",
        summary: "Shows or hides the panel that offers the shapes.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "shape",
        name: "set",
        summary: "Which shape the picker is set to.",
        arguments: &[r("shape", Kind::Choice(shapes), "which shape")],
        example: r#"{"shape":"sphere"}"#,
    },
    ActionSpec {
        group: "shape",
        name: "set_parameters",
        summary: "The numbers for the shape the picker is set to.",
        arguments: &[r("parameters", Kind::Numbers, "the shape's own numbers")],
        example: r#"{"parameters":[0.5]}"#,
    },
    ActionSpec {
        group: "shape",
        name: "set_insert_as",
        summary: "Whether the next insertion makes a subtool or an object in the active layer.",
        arguments: &[r("as", Kind::Choice(insert_as), "subtool or object")],
        example: r#"{"as":"subtool"}"#,
    },
    ActionSpec {
        group: "shape",
        name: "set_mesh_operand",
        summary: "Which mesh layer would be placed as an operand, or none.",
        arguments: &[o("layer", Kind::Integer, "the layer's key; omit for none")],
        example: r#"{"layer":2}"#,
    },
    ActionSpec {
        group: "shape",
        name: "insert",
        summary: "Puts the picked form into the scene.",
        arguments: &[],
        example: "{}",
    },
    // -- object -------------------------------------------------------------
    ActionSpec {
        group: "object",
        name: "select",
        summary: "Selects a placed object, or clears the selection.",
        arguments: &[
            o("layer", Kind::Integer, "the object's layer; omit to clear"),
            o("node", Kind::Integer, "the object's node within that layer"),
        ],
        example: r#"{"layer":1,"node":3}"#,
    },
    ActionSpec {
        group: "object",
        name: "set_shape",
        summary: "Exchanges the selected object's shape, keeping where it stands.",
        arguments: &[
            r("shape", Kind::Choice(shapes), "the new shape"),
            o("parameters", Kind::Numbers, "its numbers"),
        ],
        example: r#"{"shape":"box","parameters":[0.2,0.2,0.2]}"#,
    },
    ActionSpec {
        group: "object",
        name: "set_combine",
        summary: "How the selected object meets what is under it.",
        arguments: &[
            o("op", Kind::Choice(combines), "which operation"),
            o("blend", Kind::Choice(blends), "the blend profile"),
            o("radius", Kind::Number, "the blend radius"),
        ],
        example: r#"{"op":"add","blend":"quadratic","radius":0.02}"#,
    },
    ActionSpec {
        group: "object",
        name: "remove",
        summary: "Takes the selected object away.",
        arguments: &[],
        example: "{}",
    },
    // -- transform ----------------------------------------------------------
    ActionSpec {
        group: "transform",
        name: "set_target",
        summary: "What the manipulator acts on.",
        arguments: &[
            r(
                "target",
                Kind::Choice(gizmo_targets),
                "none, object, layer or curve",
            ),
            o("layer", Kind::Integer, "the layer, for object and layer"),
            o("node", Kind::Integer, "the node, for object"),
        ],
        example: r#"{"target":"layer","layer":1}"#,
    },
    ActionSpec {
        group: "transform",
        name: "set_mode",
        summary: "Move, rotate or scale.",
        arguments: &[r("mode", Kind::Choice(gizmo_modes), "which mode")],
        example: r#"{"mode":"move"}"#,
    },
    ActionSpec {
        group: "transform",
        name: "begin_drag",
        summary: "Takes hold of a handle.",
        arguments: &[
            r("handle", Kind::Choice(gizmo_handles), "which handle"),
            r("anchor", Kind::Vec3, "where the drag starts, in the world"),
            o(
                "view_axis",
                Kind::Vec3,
                "the camera's forward; z where none is given",
            ),
        ],
        example: r#"{"handle":"x","anchor":[0,0,0]}"#,
    },
    ActionSpec {
        group: "transform",
        name: "drag",
        summary: "Moves the held handle to a point.",
        arguments: &[
            r("at", Kind::Vec3, "where, in the world"),
            o("invert", Kind::Boolean, "hold the inverting modifier"),
        ],
        example: r#"{"at":[0.1,0,0]}"#,
    },
    ActionSpec {
        group: "transform",
        name: "end_drag",
        summary: "Lets go. The whole drag is one undo step.",
        arguments: &[],
        example: "{}",
    },
    // -- lattice ------------------------------------------------------------
    ActionSpec {
        group: "lattice",
        name: "toggle",
        summary: "Puts a deformation cage around the active layer, or takes one down.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "lattice",
        name: "set_divisions",
        summary: "How many control points the cage has per axis.",
        arguments: &[r("divisions", Kind::IVec3, "per axis")],
        example: r#"{"divisions":[3,3,3]}"#,
    },
    ActionSpec {
        group: "lattice",
        name: "select_point",
        summary: "Selects one control point, or clears the selection.",
        arguments: &[o("index", Kind::Integer, "which; omit to clear")],
        example: r#"{"index":0}"#,
    },
    ActionSpec {
        group: "lattice",
        name: "toggle_point",
        summary: "Adds or removes one control point from the selection.",
        arguments: &[r("index", Kind::Integer, "which")],
        example: r#"{"index":4}"#,
    },
    ActionSpec {
        group: "lattice",
        name: "select_points",
        summary: "Replaces the selection with a set of control points.",
        arguments: &[r("indices", Kind::Indices, "which")],
        example: r#"{"indices":[0,1,2]}"#,
    },
    ActionSpec {
        group: "lattice",
        name: "drag",
        summary: "Moves the selected control points to a point.",
        arguments: &[r("to", Kind::Vec3, "where, in the world")],
        example: r#"{"to":[0.1,0.2,0.0]}"#,
    },
    ActionSpec {
        group: "lattice",
        name: "apply",
        summary: "Bakes the cage's deformation and takes the cage down.",
        arguments: &[],
        example: "{}",
    },
    // -- subtool ------------------------------------------------------------
    ActionSpec {
        group: "subtool",
        name: "copy",
        summary: "Copies a subtool into one of its own. A copy, not an instance.",
        arguments: &[r("layer", Kind::Integer, "which subtool")],
        example: r#"{"layer":1}"#,
    },
    // -- boolean ------------------------------------------------------------
    ActionSpec {
        group: "boolean",
        name: "toggle_panel",
        summary: "Opens or closes the panel that resolves a boolean.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "boolean",
        name: "set",
        summary: "What the panel is set to. States the cost; changes nothing.",
        arguments: &[
            o("base", Kind::Integer, "the first operand's layer"),
            o("tool", Kind::Integer, "the second operand's layer"),
            o("op", Kind::Choice(booleans), "union, subtract or intersect"),
            o("cell_size", Kind::Number, "the resolution to resolve at"),
            o("consume", Kind::Boolean, "whether the operands are used up"),
        ],
        example: r#"{"base":1,"tool":2,"op":"subtract"}"#,
    },
    ActionSpec {
        group: "boolean",
        name: "run",
        summary: "Resolves the boolean the panel is set to, as one undo step.",
        arguments: &[],
        example: "{}",
    },
    // -- layer --------------------------------------------------------------
    ActionSpec {
        group: "layer",
        name: "select",
        summary: "Makes a layer the active one.",
        arguments: &[r("layer", Kind::Integer, "the layer's key")],
        example: r#"{"layer":1}"#,
    },
    ActionSpec {
        group: "layer",
        name: "set_visible",
        summary: "Shows or hides a layer.",
        arguments: &[
            r("layer", Kind::Integer, "the layer's key"),
            r("visible", Kind::Boolean, "shown or hidden"),
        ],
        example: r#"{"layer":1,"visible":false}"#,
    },
    ActionSpec {
        group: "layer",
        name: "solo",
        summary: "Shows one layer alone, or ends soloing.",
        arguments: &[o("layer", Kind::Integer, "the layer's key; omit to end")],
        example: r#"{"layer":1}"#,
    },
    ActionSpec {
        group: "layer",
        name: "add",
        summary: "Adds a layer holding a representation.",
        arguments: &[r("representation", Kind::Choice(representations), "which")],
        example: r#"{"representation":"field"}"#,
    },
    ActionSpec {
        group: "layer",
        name: "remove",
        summary: "Takes a layer away.",
        arguments: &[r("layer", Kind::Integer, "the layer's key")],
        example: r#"{"layer":2}"#,
    },
    ActionSpec {
        group: "layer",
        name: "optimize",
        summary: "Consolidates a layer's storage.",
        arguments: &[r("layer", Kind::Integer, "the layer's key")],
        example: r#"{"layer":1}"#,
    },
    ActionSpec {
        group: "layer",
        name: "remesh",
        summary: "Rebuilds a layer's topology at the remesh settings.",
        arguments: &[r("layer", Kind::Integer, "the layer's key")],
        example: r#"{"layer":1}"#,
    },
    ActionSpec {
        group: "layer",
        name: "set_remesh",
        summary: "What a remesh would do. Replaces the whole settings block.",
        arguments: &[
            o("resolution", Kind::Integer, "the target resolution"),
            o("sharp", Kind::Boolean, "keep sharp features"),
            o("remove_loose_pieces", Kind::Boolean, "drop islands"),
            o(
                "follow_the_source",
                Kind::Boolean,
                "follow the source's density",
            ),
        ],
        example: r#"{"resolution":128,"sharp":true}"#,
    },
    ActionSpec {
        group: "layer",
        name: "begin_rename",
        summary: "Starts renaming a layer.",
        arguments: &[r("layer", Kind::Integer, "the layer's key")],
        example: r#"{"layer":1}"#,
    },
    ActionSpec {
        group: "layer",
        name: "edit_name",
        summary: "The name being typed.",
        arguments: &[r("name", Kind::Text, "the new name")],
        example: r#"{"name":"cabeça"}"#,
    },
    ActionSpec {
        group: "layer",
        name: "commit_rename",
        summary: "Keeps the new name.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "layer",
        name: "cancel_rename",
        summary: "Puts the old name back.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "layer",
        name: "set_combine",
        summary: "How the active layer meets what is under it.",
        arguments: &[
            o("op", Kind::Choice(combines), "which operation"),
            o("blend", Kind::Choice(blends), "the blend profile"),
            o("radius", Kind::Number, "the blend radius"),
        ],
        example: r#"{"op":"add"}"#,
    },
    // -- passes -------------------------------------------------------------
    ActionSpec {
        group: "passes",
        name: "grid",
        summary: "The pass stack on a grid layer.",
        arguments: &[
            r("op", Kind::Choice(pass_ops), "which operation"),
            o("name", Kind::Text, "for begin_recording"),
            o("index", Kind::Integer, "which pass"),
            o("strength", Kind::Number, "0 to 1, for set_strength"),
            o("visible", Kind::Boolean, "for set_visible"),
            o("from", Kind::Integer, "for move"),
            o("to", Kind::Integer, "for move"),
        ],
        example: r#"{"op":"set_strength","index":0,"strength":0.5}"#,
    },
    // -- hierarchy ----------------------------------------------------------
    ActionSpec {
        group: "hierarchy",
        name: "level",
        summary: "The levels of a subdivision hierarchy.",
        arguments: &[
            r("op", Kind::Choice(level_ops), "which operation"),
            o("level", Kind::Integer, "which level, for the two setters"),
        ],
        example: r#"{"op":"set_sculpt_level","level":2}"#,
    },
    ActionSpec {
        group: "hierarchy",
        name: "pass",
        summary: "The pass stack on a subdivision hierarchy.",
        arguments: &[
            r("op", Kind::Choice(multires_pass_ops), "which operation"),
            o("id", Kind::Integer, "which pass, as the engine minted it"),
            o("name", Kind::Text, "for add and rename"),
            o("strength", Kind::Number, "0 to 1"),
            o("visible", Kind::Boolean, "for set_visible"),
            o("locked", Kind::Boolean, "for set_locked"),
            o("to", Kind::Integer, "for move"),
        ],
        example: r#"{"op":"add","name":"rugas"}"#,
    },
    // -- document -----------------------------------------------------------
    ActionSpec {
        group: "document",
        name: "new",
        summary: "Starts a new document. Gated where the open one is unsaved.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "document",
        name: "open",
        summary: "Opens a document by path. Gated.",
        arguments: &[r("path", Kind::Path, "the file to open")],
        example: r#"{"path":"/Users/me/head.clayspace"}"#,
    },
    ActionSpec {
        group: "document",
        name: "save",
        summary: "Writes the document where it already is. Gated.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "document",
        name: "quit",
        summary: "Closes the application. Gated.",
        arguments: &[],
        example: "{}",
    },
    // -- exchange -----------------------------------------------------------
    ActionSpec {
        group: "exchange",
        name: "toggle_import",
        summary: "Opens or closes the import panel.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "exchange",
        name: "toggle_export",
        summary: "Opens or closes the export panel.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "exchange",
        name: "set_import",
        summary: "What an import would do. Replaces the whole settings block.",
        arguments: &[
            o("becomes", Kind::Choice(import_as), "reference or clay"),
            o("scale", Kind::Number, "how the source is scaled"),
            o("max_vertices", Kind::Integer, "the ceiling on vertices"),
            o("max_triangles", Kind::Integer, "the ceiling on triangles"),
        ],
        example: r#"{"becomes":"clay","scale":1.0}"#,
    },
    ActionSpec {
        group: "exchange",
        name: "set_export",
        summary: "What an export would do. Replaces the whole settings block.",
        arguments: &[
            o("mesher", Kind::Choice(meshers), "which mesher"),
            o("resolution", Kind::Number, "the meshing resolution"),
            o(
                "decimate_to",
                Kind::Number,
                "a triangle budget, or omit for none",
            ),
        ],
        example: r#"{"mesher":"watertight","resolution":0.02}"#,
    },
    ActionSpec {
        group: "exchange",
        name: "run_import",
        summary: "Runs the import the panel is set to. Gated.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "exchange",
        name: "run_export",
        summary: "Runs the export the panel is set to. Gated.",
        arguments: &[],
        example: "{}",
    },
    // -- repair -------------------------------------------------------------
    ActionSpec {
        group: "repair",
        name: "toggle_panel",
        summary: "Opens or closes the repair panel.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "repair",
        name: "close_holes",
        summary: "Closes the holes in the active mesh layer.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "repair",
        name: "fill_voids",
        summary: "Fills the enclosed voids in the active layer.",
        arguments: &[],
        example: "{}",
    },
    // -- convert ------------------------------------------------------------
    ActionSpec {
        group: "convert",
        name: "toggle_panel",
        summary: "Opens or closes the conversion panel.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "convert",
        name: "set",
        summary: "What a conversion would do. States the cost; changes nothing.",
        arguments: &[
            o("direction", Kind::Choice(directions), "which crossing"),
            o("cell_size", Kind::Number, "the resolution to cross at"),
            o("blur", Kind::Integer, "smoothing passes on the way"),
            o("in_place", Kind::Boolean, "replace the source layer"),
        ],
        example: r#"{"direction":"field-to-grid","cell_size":0.02}"#,
    },
    ActionSpec {
        group: "convert",
        name: "run",
        summary: "Runs the conversion the panel is set to, as one undo step.",
        arguments: &[],
        example: "{}",
    },
    // -- deform -------------------------------------------------------------
    ActionSpec {
        group: "deform",
        name: "toggle_panel",
        summary: "Opens or closes the deformation panel.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "deform",
        name: "set",
        summary: "What a deformation would do. Replaces the whole settings block.",
        arguments: &[
            o("verb", Kind::Choice(deform_verbs), "taper or twist"),
            o("axis", Kind::Vec3, "the axis it works along"),
            o("span", Kind::Number, "how far along the axis"),
            o(
                "scale_start",
                Kind::Number,
                "the factor at the start, for taper",
            ),
            o(
                "scale_end",
                Kind::Number,
                "the factor at the end, for taper",
            ),
            o("degrees", Kind::Number, "the turn, for twist"),
        ],
        example: r#"{"verb":"twist","degrees":45.0}"#,
    },
    ActionSpec {
        group: "deform",
        name: "run",
        summary: "Runs the deformation, as one undo step.",
        arguments: &[],
        example: "{}",
    },
    // -- armature -----------------------------------------------------------
    ActionSpec {
        group: "armature",
        name: "new",
        summary: "Starts an armature.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "armature",
        name: "toggle_editing",
        summary: "Whether the pointer edits the armature.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "armature",
        name: "remove_zsphere",
        summary: "Takes the selected ZSphere away.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "armature",
        name: "toggle_skin_preview",
        summary: "Whether the skin over the armature is drawn.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "armature",
        name: "toggle_negative",
        summary: "Whether the selected ZSphere subtracts rather than adds.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "armature",
        name: "select",
        summary: "Which ZSphere the other verbs act on, or none.",
        arguments: &[o("sphere", Kind::Integer, "its index; omit to clear")],
        example: r#"{"sphere":0}"#,
    },
    ActionSpec {
        group: "armature",
        name: "add",
        summary: "Grows a ZSphere out of another, at a point in the world. \
                  Mirrored where the armature's symmetry is on.",
        arguments: &[
            r("parent", Kind::Integer, "the sphere it grows from"),
            r("at", Kind::Vec3, "where the new one lands"),
            o(
                "radius",
                Kind::Number,
                "the armature's own default where none is given",
            ),
        ],
        example: r#"{"parent":0,"at":[0.0,0.4,0.0]}"#,
    },
    ActionSpec {
        group: "armature",
        name: "insert",
        summary: "Puts a ZSphere on the link between one and its parent.",
        arguments: &[r("sphere", Kind::Integer, "the child end of the link")],
        example: r#"{"sphere":1}"#,
    },
    ActionSpec {
        group: "armature",
        name: "move",
        summary: "Moves a ZSphere to a point, subtree and all.",
        arguments: &[
            r("sphere", Kind::Integer, "which"),
            r("to", Kind::Vec3, "where, in the world"),
        ],
        example: r#"{"sphere":1,"to":[0.2,0.5,0.0]}"#,
    },
    ActionSpec {
        group: "armature",
        name: "resize",
        summary: "How thick a ZSphere is.",
        arguments: &[
            r("sphere", Kind::Integer, "which"),
            r("radius", Kind::Number, "the radius"),
        ],
        example: r#"{"sphere":1,"radius":0.12}"#,
    },
    ActionSpec {
        group: "armature",
        name: "reparent",
        summary: "Hangs a ZSphere, and its subtree, under a different parent.",
        arguments: &[
            r("sphere", Kind::Integer, "which"),
            r("parent", Kind::Integer, "its new parent"),
        ],
        example: r#"{"sphere":2,"parent":0}"#,
    },
    ActionSpec {
        group: "armature",
        name: "set_skin_thickness",
        summary: "How thick the skin over the armature is.",
        arguments: &[r("thickness", Kind::Number, "the thickness")],
        example: r#"{"thickness":0.05}"#,
    },
    // -- history ------------------------------------------------------------
    ActionSpec {
        group: "history",
        name: "undo",
        summary: "Undoes the last edit.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "history",
        name: "redo",
        summary: "Redoes the last undone edit.",
        arguments: &[],
        example: "{}",
    },
    // -- view ---------------------------------------------------------------
    ActionSpec {
        group: "view",
        name: "set_preset",
        summary: "Moves the camera to a standard view.",
        arguments: &[r("preset", Kind::Choice(view_presets), "which view")],
        example: r#"{"preset":"front"}"#,
    },
    ActionSpec {
        group: "view",
        name: "frame_all",
        summary: "Frames the whole scene.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "view",
        name: "next_material",
        summary: "Takes the next material.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "view",
        name: "toggle_grid",
        summary: "Shows or hides the ground grid.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "view",
        name: "toggle_polyframe",
        summary: "Shows or hides the wireframe over the surface.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "view",
        name: "next_unit",
        summary: "Takes the next display unit.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "view",
        name: "toggle_shading",
        summary: "Switches the shading mode.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "view",
        name: "toggle_cavity",
        summary: "Shows or hides cavity shading.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "view",
        name: "toggle_shadows",
        summary: "Shows or hides shadows.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "view",
        name: "set_grid_display",
        summary: "How a grid layer is drawn, and how much it is smoothed.",
        arguments: &[
            r("display", Kind::Choice(voxel_displays), "boxes or smooth"),
            o(
                "blur_passes",
                Kind::Integer,
                "0 to 3; past 1 it eats detail",
            ),
        ],
        example: r#"{"display":"smooth","blur_passes":0}"#,
    },
    ActionSpec {
        group: "view",
        name: "set_surface_opacity",
        summary: "How solid the surface is drawn, for seeing what is behind it.",
        arguments: &[r("opacity", Kind::Number, "0.1 to 1")],
        example: r#"{"opacity":0.5}"#,
    },
    // -- reference ----------------------------------------------------------
    ActionSpec {
        group: "reference",
        name: "toggle_panel",
        summary: "Opens or closes the reference panel.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "reference",
        name: "clear",
        summary: "Takes the reference image off a plane.",
        arguments: &[r("plane", Kind::Choice(planes), "which plane")],
        example: r#"{"plane":"front"}"#,
    },
    ActionSpec {
        group: "reference",
        name: "set",
        summary: "How a plane's reference is shown. Replaces the whole settings block.",
        arguments: &[
            r("plane", Kind::Choice(planes), "which plane"),
            o("visible", Kind::Boolean, "shown or hidden"),
            o("opacity", Kind::Number, "0 to 1"),
            o("height", Kind::Number, "how tall it is drawn"),
            o("offset", Kind::Vec2, "where it sits on the plane"),
            o("depth", Kind::Number, "how far along the plane's normal"),
        ],
        example: r#"{"plane":"front","opacity":0.5}"#,
    },
    // -- session ------------------------------------------------------------
    ActionSpec {
        group: "session",
        name: "set_language",
        summary: "The language the interface is in.",
        arguments: &[r("language", Kind::Choice(locales), "which language")],
        example: r#"{"language":"pt-BR"}"#,
    },
    ActionSpec {
        group: "session",
        name: "toggle_attribution",
        summary: "Shows or hides what this build is made from.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "session",
        name: "toggle_diagnostics",
        summary: "Shows or hides the diagnostics window.",
        arguments: &[],
        example: "{}",
    },
    ActionSpec {
        group: "session",
        name: "copy_diagnostics",
        summary: "Copies the diagnostics report to the clipboard.",
        arguments: &[],
        example: "{}",
    },
];
