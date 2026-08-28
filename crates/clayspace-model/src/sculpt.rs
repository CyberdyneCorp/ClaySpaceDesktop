//! What a ViewModel is allowed to ask of the document.
//!
//! Expressed as a trait so the ViewModel layer can be exercised against a
//! double: no engine, no GPU, no window. The engine-backed implementation
//! lives beside it, and the two are interchangeable by construction.

use crate::tools::{BrushSettings, Representation, ToolKind};

/// One sample of a sculpting gesture, as the input device reported it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GestureSample {
    pub position: [f32; 3],
    /// Reported pressure, normally 0..=1. Devices without a pressure axis
    /// report 1.
    pub pressure: f32,
    /// Seconds since the stroke began.
    pub time: f32,
}

/// What a completed edit did, as far as the interface needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditOutcome {
    /// Whether anything actually changed.
    ///
    /// The engine documents several verbs as legitimately able to change
    /// nothing — a sub-cell drag, a stamp that misses every cell — so a
    /// successful call is not evidence that an edit happened. A no-op adds no
    /// history entry and does not mark the document modified.
    pub changed: bool,
    /// Bricks the edit dirtied, which is what the viewport must re-mesh.
    pub dirty_bricks: usize,
}

impl EditOutcome {
    pub const NOTHING: Self = Self {
        changed: false,
        dirty_bricks: 0,
    };
}

/// What the interface reports about the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SceneStats {
    pub triangles: usize,
    pub vertices: usize,
    pub objects: usize,
    /// Which detail the counts describe.
    ///
    /// Carried because the counts are of what is *drawn*, and the viewport may
    /// be showing a reduced level of detail. A number presented without saying
    /// which resolution it belongs to reads as a smaller model.
    pub detail: Detail,
}

/// Which detail level a count describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Detail {
    /// The full-resolution surface.
    #[default]
    Full,
    /// A reduced level, shown while the camera is far away.
    Reduced,
    /// Nothing has been meshed yet.
    Pending,
}

impl Detail {
    /// How the interface qualifies a count.
    ///
    /// `None` for the full-resolution case, because a count that needs no
    /// qualification should not carry one.
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::Full => None,
            Self::Reduced => Some("detalhe reduzido"),
            Self::Pending => Some("ainda não gerado"),
        }
    }
}

/// Whether undo and redo have anything to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HistoryState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub depth: usize,
    /// How many entries can be redone.
    pub redo_depth: usize,
}

/// One entry in the history, as the panel shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// What the operation was, in the user's terms.
    pub label: String,
    /// Whether it is behind the current position — that is, undone.
    pub undone: bool,
}

/// The operations a ViewModel performs on a document.
///
/// Deliberately small: it is the surface the interface needs, not the engine's.
/// Everything here is expressed in domain terms, so no ClayCore type crosses
/// into the layers above.
pub trait SculptModel {
    /// The representation of the layer edits currently go to.
    fn active_representation(&self) -> Representation;

    /// Whether the active layer accepts edits at all.
    fn active_layer_editable(&self) -> bool;

    /// Whether the active layer carries geometry a verb can reach.
    ///
    /// Provided, like visibility, so a double that does not model it says yes
    /// and behaves as it always did.
    fn active_layer_carries_geometry(&self) -> bool {
        true
    }

    /// Whether the active layer is drawn.
    ///
    /// An edit to a hidden layer lands where nothing shows it, which a
    /// sculptor cannot tell apart from the tool not working. Provided rather
    /// than required so a double that does not model visibility says "visible"
    /// and behaves as it always did.
    fn active_layer_visible(&self) -> bool {
        true
    }

    /// What the active layer can accept right now, as one value.
    ///
    /// Assembled here so that a call site cannot forget one of the conditions
    /// and offer a tool that would refuse.
    fn active_layer_state(&self) -> crate::tools::LayerState {
        crate::tools::LayerState {
            representation: self.active_representation(),
            editable: self.active_layer_editable(),
            visible: self.active_layer_visible(),
            carries_geometry: self.active_layer_carries_geometry(),
        }
    }

    /// Applies something a gesture cannot express.
    ///
    /// The second verb beside `apply_stroke` — see [`crate::LayerOperation`]
    /// for why the two are separate rather than one widened call. Provided, so
    /// a double that models no operations refuses them rather than having to
    /// spell out a refusal it never reaches.
    fn apply_operation(
        &mut self,
        operation: crate::LayerOperation,
    ) -> Result<EditOutcome, ModelError> {
        let _ = operation;
        Err(ModelError::Unavailable(crate::Unavailable::NoVerbHere {
            active: self.active_representation(),
            verbs: crate::Verbs {
                sdf: None,
                voxel: None,
                mesh: None,
            },
        }))
    }

    /// Applies part or all of a gesture with the given tool.
    ///
    /// A whole gesture at once lets the engine's stroke engine decide stamp
    /// spacing from arc length rather than from how many samples the device
    /// delivered. A live stroke cannot wait for the whole gesture, so it sends
    /// segments as the pointer moves, each long enough that the spacing still
    /// has something to work with. Every call is its own entry in the
    /// document's history; collapsing a gesture's segments back into one
    /// undo is the ViewModel's job, because the engine's own undo grouping
    /// does not do it — measured, a group of three strokes left seven entries
    /// and undoing twice reverted none of them.
    fn apply_stroke(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError>;

    /// The symmetry axes the active subtool is set to.
    ///
    /// Per layer rather than per document, because the mirror is: the engine
    /// stores one plane per layer, and a sculptor who turns symmetry off to
    /// work one ear has not said anything about the subtool beside it. The
    /// interface reads this whenever the active subtool changes, so the
    /// toggles show the incoming subtool's own setting instead of carrying
    /// the outgoing one's along.
    fn symmetry(&self) -> [bool; 3] {
        [false; 3]
    }

    /// Points the active subtool's mirror at these axes.
    ///
    /// Defaulted so a double that models no mirror ignores it rather than
    /// spelling out a refusal.
    fn set_symmetry(&mut self, symmetry: [bool; 3]) -> Result<(), ModelError> {
        let _ = symmetry;
        Ok(())
    }

    /// How the next SDF edit combines with what is under it.
    ///
    /// State rather than a fifth argument to `apply_stroke`: the choice is made
    /// once in the options bar and then holds across every stroke, exactly as
    /// symmetry does, and threading it through each call would put the same
    /// value in every call site whether it was doing anything or not. Defaulted
    /// so a double that models one representation refuses nothing new.
    fn set_combine(&mut self, combine: crate::CombineSettings) {
        let _ = combine;
    }

    fn combine(&self) -> crate::CombineSettings {
        crate::CombineSettings::for_strokes()
    }

    /// Loads or clears the alpha stamp every brush with `alpha` set uses.
    ///
    /// One stamp for the document rather than one per tool: a stamp is
    /// megabytes and a sculptor works with one at a time. Which tools use it
    /// is [`BrushSettings::alpha`]. Defaulted so a double that models no
    /// stamps ignores it rather than spelling out a refusal.
    fn set_alpha(&mut self, alpha: Option<crate::Alpha>) {
        let _ = alpha;
    }

    /// What the loaded stamp is called, if one is loaded.
    ///
    /// The name rather than the samples: the interface needs to say which
    /// stamp is in use and has no business holding megabytes to do it.
    fn alpha_name(&self) -> Option<String> {
        None
    }

    /// Where a ray meets the surface, if anywhere.
    fn pick(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]>;

    fn undo(&mut self) -> Result<bool, ModelError>;
    fn redo(&mut self) -> Result<bool, ModelError>;
    fn history(&self) -> HistoryState;

    fn stats(&self) -> SceneStats;

    /// The document's bounds, for framing.
    /// A gesture is open; what follows previews it rather than banking it.
    ///
    /// A dragging verb on a mesh is laid down again from its anchor on every
    /// segment, so the model has to know a gesture is in progress in order to
    /// take back what the last segment did. Provided, because only the mesh
    /// path has anything to do with it.
    fn begin_gesture(&mut self) {}

    /// It is over: what was previewed becomes an edit, and one undo takes the
    /// whole gesture back however many segments drew it.
    fn end_gesture(&mut self) {}

    fn bounds(&self) -> Option<([f32; 3], [f32; 3])>;
}

/// Why an operation on the document failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// The tool cannot apply to the active layer.
    Unavailable(crate::tools::Unavailable),
    /// The engine refused, with its own description.
    Engine(String),
    /// A crossing between representations was refused, with which of the
    /// reasons it was.
    Conversion(crate::conversion::Refusal),
    /// A boolean between two subtools was refused, naming the operand and the
    /// cause.
    ///
    /// Its own variant rather than an `Engine` string, because the interface
    /// has to be able to say *which* of the two subtools is the problem and a
    /// sentence the adapter formatted cannot be asked that afterwards.
    Boolean(crate::boolean::BooleanRefusal),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(why) => write!(f, "{why}"),
            Self::Engine(why) => f.write_str(why),
            Self::Conversion(why) => write!(f, "{why}"),
            Self::Boolean(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for ModelError {}

impl ModelError {
    /// Wraps whatever the engine said.
    ///
    /// The adapter performs this conversion, because the domain does not know
    /// what an engine is. The message is the engine's own detail: a result
    /// code is not something a user can act on.
    pub fn engine(detail: impl std::fmt::Display) -> Self {
        Self::Engine(detail.to_string())
    }
}
