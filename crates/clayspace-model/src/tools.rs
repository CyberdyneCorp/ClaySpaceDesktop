//! The sculpting vocabulary the interface offers.
//!
//! Every tool here names the engine verb it invokes. A tool with no engine
//! counterpart is not offered, and a label never binds to a verb that does
//! something adjacent to what it says: the mapping follows the engine's own
//! ZBrush-equivalence table rather than an invention of ours.
//!
//! Which tool reaches which representation is a *table* rather than a rule
//! written per tool. That is not a style preference. The rule it replaced said
//! every tool on a mesh layer is unavailable because "mesh layers are carried,
//! not sculpted", which was true of the engine when it was written and stopped
//! being true without anything here noticing — a `match` arm can only be read,
//! and a table can be checked against the engine's own vocabulary. `tools.rs`'s
//! own tests do exactly that, so a verb ClayCore has and this application does
//! not is a failing count rather than a silence.
//!
//! A tool with no verb on the active representation is **absent** rather than
//! offered and disabled. With four representations carrying substantially
//! different vocabularies, one list would be mostly disabled entries whatever
//! the active layer, all carrying the same sentence. A tool that *has* a verb
//! here and cannot be used right now — a locked layer, a hidden one, a missing
//! attribute — is still shown, disabled, with which of those it is.

/// Which representation a layer holds.
///
/// `Hash` because the interface keys widget ids off it, as it does off
/// `ToolKind` — an id derived from a `label()` would be an interface word
/// doing structural work, and the shell's own ratchet counts those.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Representation {
    /// An ordered edit list evaluated as a distance field.
    Sdf,
    /// A palette-indexed voxel grid.
    Voxel,
    /// Imported triangles, held verbatim.
    Mesh,
    /// A cage, a subdivision hierarchy over it, and detail stored per level.
    ///
    /// The one thing that distinguishes it from a mesh, and the reason it is a
    /// representation rather than a mode: what is stored above the cage is not
    /// a position but a *displacement in a frame carried up from the level
    /// below*. So moving the form at a coarse level moves the frames, and the
    /// wrinkles cut at a fine one ride on them instead of being smeared or
    /// re-projected. A mesh cannot express that, because a mesh has one level
    /// and nothing under it to move.
    ///
    /// Two consequences run through this crate. Where the brush writes and
    /// what the viewport draws are two independent numbers rather than one —
    /// see [`crate::multires::MultiresLevels`]. And a hierarchy stores where
    /// its vertices went and not what colour they are, which is why the two
    /// colour brushes reach a mesh and not this.
    Multires,
    /// Triangles whose connectivity adapts under the brush.
    ///
    /// The engine's DynamicSurface, and deliberately not a mode of
    /// [`Self::Mesh`]: a fixed mesh keeps its topology — the contract after a
    /// retopology — and this one splits and collapses edges where a stroke
    /// needs them, which is the contract while a form is still being found. A
    /// large Move on a mesh stretches the triangles it has; here it makes the
    /// ones it needs. Quads do not survive, and an undo has to restore
    /// connectivity rather than positions alone.
    ///
    /// Reported, persisted and offered as itself everywhere. Treating it as a
    /// mesh would hide what a crossing to it costs and what its history holds.
    Dynamic,
}

impl Representation {
    pub const ALL: [Representation; 5] = [
        Self::Sdf,
        Self::Voxel,
        Self::Mesh,
        Self::Multires,
        Self::Dynamic,
    ];

    /// The representations an *empty* layer can be created in.
    ///
    /// Two, not four. A mesh layer is made by carrying a mesh — there is no
    /// call anywhere that makes an empty one — so "add a layer and choose
    /// mesh" produced a row labelled mesh with a field layer behind it that
    /// nothing could ever put triangles into. The specification qualifies the
    /// offer, "SDF, voxel and mesh *where a mesh source is at hand*", and at
    /// the moment a layer is added out of nothing there is none: that route is
    /// the import, which makes its own layer.
    ///
    /// A hierarchy is out for the same reason and more sharply: it is built
    /// *from a cage*, `clay_multires_from_mesh` refuses rather than repairs
    /// one, and there is no call that makes an empty one at all. It arrives
    /// through [`crate::Direction::MeshToMultires`] or not at all.
    ///
    /// An adaptive surface is out for the first reason: it is read from a mesh
    /// by `clay_dynamic_surface_from_mesh`, and an empty one has nothing to
    /// read. It arrives through [`crate::Direction::MeshToDynamic`].
    pub const CREATABLE: [Representation; 2] = [Self::Sdf, Self::Voxel];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sdf => "SDF",
            Self::Voxel => "voxel",
            Self::Mesh => "mesh",
            Self::Multires => "multires",
            Self::Dynamic => "dynamic",
        }
    }

    /// A stable name for storage.
    ///
    /// Not [`Representation::label`], which is text a reader sees and may be
    /// reworded. This is what a saved document holds, so it never changes once
    /// written — the rule [`ToolKind::key`] follows. An adaptive surface is
    /// `"dynamic"` and never `"mesh"`: a row saved as a mesh would reopen as
    /// one, with its connectivity contract silently swapped.
    pub fn key(self) -> &'static str {
        match self {
            Self::Sdf => "sdf",
            Self::Voxel => "voxel",
            Self::Mesh => "mesh",
            Self::Multires => "multires",
            Self::Dynamic => "dynamic",
        }
    }

    /// The representation a stored key names, or `None` for one this build
    /// does not know.
    ///
    /// Refused rather than guessed: an unknown key is a document from a later
    /// build, and reading it as the nearest representation would open it as
    /// something it is not.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|representation| representation.key() == key)
    }

    /// Whether the layer holds its own vertices rather than a field or cells.
    ///
    /// The three that do are drawn from their own triangles, have no bricks in
    /// the field cache and are sculpted by the shared mesh brush descriptor.
    pub fn carries_vertices(self) -> bool {
        matches!(self, Self::Mesh | Self::Multires | Self::Dynamic)
    }
}

/// What a binding *means* to a sculptor, independent of how it runs.
///
/// A tool is one word on the shelf and up to four calls under it, and the
/// thing that makes those four one tool is this: they are the same act. Until
/// now that was asserted only by the shelf showing one button, which is not an
/// assertion at all — two rows could name two unrelated verbs and the
/// interface would present them as one tool with a single tooltip. Stated per
/// binding, it becomes a claim a test can hold: see
/// `a_tool_means_one_thing_wherever_it_is_offered`.
///
/// Deliberately coarse. It is not a second label — the label is the label —
/// and a vocabulary fine enough to tell Padrão from Camada would be a
/// vocabulary with one word per tool, which says nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticIntent {
    /// Moves the surface along its normal, by a deposit.
    SurfaceDisplace,
    /// Drags the surface sideways: what is there goes somewhere else.
    SurfaceMove,
    /// Averages the surface against itself.
    SurfaceSmooth,
    /// Takes the surface toward a plane.
    SurfaceFlatten,
    /// Cuts a narrow line into the surface, or raises one.
    SurfaceCrease,
    /// Gathers the surface toward the dab's centre.
    SurfacePinch,
    /// Puts material where there was none.
    VolumeAdd,
    /// Takes material away.
    VolumeRemove,
    /// Offsets the whole surface outward, each point along its own normal.
    VolumeInflate,
    /// Changes how finely the form is stored, rather than what it holds.
    TopologyRebuild,
    /// Writes colour and moves nothing.
    Paint,
    /// Freezes a region against every other verb.
    Mask,
    /// Changes how the layer is organised: a pass, a level, a stack.
    ///
    /// Not a sculpting act at all, and the one intent no tool on the shelf
    /// carries. It is here because [`Verbs`] is also how a refusal says where
    /// a *structural* operation applies — a grid's pass stack, a hierarchy's
    /// levels — and a row that could not name its intent would be a row back
    /// in prose.
    Structure,
}

impl SemanticIntent {
    /// For the diagnostics line. Not interface text: the report is read by
    /// this project's own maintainers and is pasted into issues in English.
    pub fn label(self) -> &'static str {
        match self {
            Self::SurfaceDisplace => "surface displace",
            Self::SurfaceMove => "surface move",
            Self::SurfaceSmooth => "surface smooth",
            Self::SurfaceFlatten => "surface flatten",
            Self::SurfaceCrease => "surface crease",
            Self::SurfacePinch => "surface pinch",
            Self::VolumeAdd => "volume add",
            Self::VolumeRemove => "volume remove",
            Self::VolumeInflate => "volume inflate",
            Self::TopologyRebuild => "topology rebuild",
            Self::Paint => "paint",
            Self::Mask => "mask",
            Self::Structure => "structure",
        }
    }
}

/// How a binding reaches the engine.
///
/// Which of the engine's families the call belongs to, which is a fact about
/// the ABI rather than about the tool: two bindings in one family take the
/// same kind of descriptor, cost the same kind of time and fail the same way.
/// A reader asking why the field's smooth costs what the mesh's does not is
/// asking this question, and until now the answer was in a comment.
///
/// The rule this enum is held to: a family nothing binds is a claim nothing
/// checks, so a family arrives with the first binding that needs it. The
/// dynamic-topology family arrived with [`Representation::Dynamic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionFamily {
    /// An operation applied to the accumulated field: `clay_layer_apply_stroke`
    /// carrying one of the engine's `CLAY_OP_*` operations.
    FieldCombineOp,
    /// A deformer over the accumulated field — a transaction, or a signed
    /// radial scale — which moves what is there rather than contributing to it.
    FieldDeformer,
    /// An item in the layer's ordered list: added, or its own geometry edited.
    FieldItemEdit,
    /// Sample the document into a volume, act on the samples, write back.
    ///
    /// The `_from` family, and its own rather than folded in with the rest of
    /// the field's vocabulary because the sampling is where both its accuracy
    /// and its cost come from: the engine says the bake-then-act pair differs
    /// from this "by accuracy, and it is not small".
    BakedFieldOperation,
    /// A verb on the voxel grid.
    VoxelVerb,
    /// A verb on the fixed-topology mesh sculptor.
    MeshVerb,
    /// A verb on the subdivision hierarchy.
    MultiresVerb,
    /// A verb on the adaptive surface, which may split and collapse edges
    /// around the deformation.
    DynamicVerb,
    /// The world-addressed mask, which belongs to no representation.
    ///
    /// Its own family rather than the field's, because belonging to no
    /// representation is exactly what a mask is: it is consulted by every verb
    /// on every representation, and filing it under the field would make the
    /// one row that is the same call on all four read as three borrowings of
    /// an SDF call.
    MaskField,
    /// Several engine verbs in a fixed order, standing in for one the engine
    /// has not.
    ///
    /// Nothing on the shelf is one today, and the family exists so that the
    /// first one does not have to arrive as an absence. The engine documents
    /// DamStandard on a grid as a recipe rather than a verb, and a composed
    /// tool that cannot be *described* is a tool that can only be left out —
    /// which is how the voxel Crease column came to be empty with the reason
    /// in a comment.
    Recipe,
}

impl ExecutionFamily {
    /// The prefix every entry point in this family carries, where the family
    /// is one the engine spells as a prefix.
    ///
    /// `None` for the four that are not: the field's vocabulary is spread
    /// across `clay_layer_*`, `clay_sdf_*`, `clay_item_*` and `clay_cut_*`, and
    /// a recipe names whatever its steps name. What this is for is
    /// `every_binding_is_filed_under_the_family_it_calls` — a mesh verb
    /// declared as a grid's is a row that reads plausibly and misleads every
    /// reader after it.
    pub fn prefix(self) -> Option<&'static str> {
        match self {
            Self::VoxelVerb => Some("clay_voxel_"),
            Self::MeshVerb => Some("clay_mesh_"),
            Self::MultiresVerb => Some("clay_multires_"),
            Self::DynamicVerb => Some("clay_dynamic_"),
            Self::MaskField => Some("clay_mask_"),
            Self::BakedFieldOperation => Some("clay_item_volume_"),
            Self::FieldCombineOp | Self::FieldDeformer | Self::FieldItemEdit | Self::Recipe => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::FieldCombineOp => "field combine op",
            Self::FieldDeformer => "field deformer",
            Self::FieldItemEdit => "field item edit",
            Self::BakedFieldOperation => "baked field operation",
            Self::VoxelVerb => "voxel verb",
            Self::MeshVerb => "mesh verb",
            Self::MultiresVerb => "multires verb",
            Self::DynamicVerb => "dynamic verb",
            Self::MaskField => "mask field",
            Self::Recipe => "recipe",
        }
    }
}

/// How well the call keeps the promise the tool's label makes.
///
/// The column this table most needed and least had. Every row was written as
/// though a binding either exists or does not, and several are neither: a
/// grid's Padrão deposits cells because occupancy is binary, a field's Padrão
/// is measurably an Inflate, a grid's Planar fills as well as cuts, a
/// hierarchy's eraser acts on one pass. Each of those was recorded in a
/// comment, where it could not reach the interface, the diagnostics or a test
/// — and the ones an artist hears about reached them only because somebody
/// separately wrote a [`ToolNote`].
///
/// Stated here, the relation between the two becomes checkable: a note is the
/// *sentence* for a binding that is not the native one, and a note on a
/// binding that is native would be a sentence about nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fidelity {
    /// The representation's own verb for the intent, doing what the label says.
    Native,
    /// The representation's verb does *more* than the intent, because of what
    /// it stores.
    ///
    /// Not a shortfall and not a defect: a grid's flatten is two-sided because
    /// a grid can fill, and a hierarchy's smooth picks a frequency because a
    /// hierarchy has frequencies. These are the rows where the representation
    /// is the reason to reach for it.
    Specialized,
    /// A useful stand-in: close enough to offer, different enough to say so.
    Approximation,
    /// Several verbs composed, where the engine has no single one.
    Recipe,
}

impl Fidelity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Specialized => "specialized",
            Self::Approximation => "approximation",
            Self::Recipe => "recipe",
        }
    }

    /// Whether this row does the plain reading of the tool's label and nothing
    /// else.
    ///
    /// What the note rule is asked in terms of: a row that is not the plain
    /// reading is a row a sculptor can be surprised by mid-stroke.
    pub fn is_the_plain_reading(self) -> bool {
        self == Self::Native
    }
}

/// One column of one row: what a tool calls on one representation, and what
/// kind of call it is.
///
/// The entry point was here all along; the other three were in the prose
/// around it. Moving them into the value is this type's whole point — prose
/// cannot drive the shelf, cannot reach the diagnostics line, and cannot fail
/// a test when the code stops matching it. Three separate defects came out of
/// that gap: two tools sharing a verb while claiming to differ, a note
/// describing a mode nothing selected, and a row naming a call nobody made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// The engine entry point, spelled for a reader — the call, and the
    /// operation or brush kind it carries, in brackets.
    ///
    /// [`entry_points`] is what picks the names back out of it, and
    /// `table_truth.rs` is what asks the linked engine whether each one is a
    /// symbol it has and whether the stroke reaches it.
    pub entry_point: &'static str,
    pub intent: SemanticIntent,
    pub family: ExecutionFamily,
    pub fidelity: Fidelity,
}

impl Binding {
    pub const fn new(
        entry_point: &'static str,
        intent: SemanticIntent,
        family: ExecutionFamily,
        fidelity: Fidelity,
    ) -> Self {
        Self {
            entry_point,
            intent,
            family,
            fidelity,
        }
    }

    /// The binding as the diagnostics report states one.
    pub fn describe(self) -> String {
        format!(
            "{} ({}, {}, {})",
            self.entry_point,
            self.intent.label(),
            self.family.label(),
            self.fidelity.label()
        )
    }

    /// Whether this is several verbs standing in for one.
    pub fn is_a_recipe(self) -> bool {
        self.fidelity == Fidelity::Recipe || self.family == ExecutionFamily::Recipe
    }
}

// The row shorthands. A column is written as the family it belongs to, so that
// a mesh verb filed under the grid is something a reader sees rather than an
// argument three deep — and `every_binding_is_filed_under_the_family_it_calls`
// is what holds the spelling to the symbol.
//
// `pub(crate)` rather than private: the object table in `shape.rs` is a row of
// the same kind and is written the same way. A crate that is not this one
// builds a [`Binding`] with [`Binding::new`].

pub(crate) const fn field_op(
    entry_point: &'static str,
    intent: SemanticIntent,
    fidelity: Fidelity,
) -> Option<Binding> {
    Some(Binding::new(
        entry_point,
        intent,
        ExecutionFamily::FieldCombineOp,
        fidelity,
    ))
}

pub(crate) const fn field_deformer(
    entry_point: &'static str,
    intent: SemanticIntent,
    fidelity: Fidelity,
) -> Option<Binding> {
    Some(Binding::new(
        entry_point,
        intent,
        ExecutionFamily::FieldDeformer,
        fidelity,
    ))
}

pub(crate) const fn field_item(
    entry_point: &'static str,
    intent: SemanticIntent,
    fidelity: Fidelity,
) -> Option<Binding> {
    Some(Binding::new(
        entry_point,
        intent,
        ExecutionFamily::FieldItemEdit,
        fidelity,
    ))
}

pub(crate) const fn baked_field(
    entry_point: &'static str,
    intent: SemanticIntent,
    fidelity: Fidelity,
) -> Option<Binding> {
    Some(Binding::new(
        entry_point,
        intent,
        ExecutionFamily::BakedFieldOperation,
        fidelity,
    ))
}

pub(crate) const fn voxel_verb(
    entry_point: &'static str,
    intent: SemanticIntent,
    fidelity: Fidelity,
) -> Option<Binding> {
    Some(Binding::new(
        entry_point,
        intent,
        ExecutionFamily::VoxelVerb,
        fidelity,
    ))
}

pub(crate) const fn mesh_verb(
    entry_point: &'static str,
    intent: SemanticIntent,
    fidelity: Fidelity,
) -> Option<Binding> {
    Some(Binding::new(
        entry_point,
        intent,
        ExecutionFamily::MeshVerb,
        fidelity,
    ))
}

pub(crate) const fn multires_verb(
    entry_point: &'static str,
    intent: SemanticIntent,
    fidelity: Fidelity,
) -> Option<Binding> {
    Some(Binding::new(
        entry_point,
        intent,
        ExecutionFamily::MultiresVerb,
        fidelity,
    ))
}

/// The adaptive surface's column.
///
/// Every brush it offers is the fixed sculptor's brush through one entry
/// point — `clay_dynamic_sculptor_apply_stroke` takes the same
/// `clay_mesh_brush_desc` — so the intent is the mesh row's and the fidelity
/// is the mesh row's too: the engine calls the shared kernels rather than a
/// copy of them.
pub(crate) const fn dynamic_verb(
    entry_point: &'static str,
    intent: SemanticIntent,
    fidelity: Fidelity,
) -> Option<Binding> {
    Some(Binding::new(
        entry_point,
        intent,
        ExecutionFamily::DynamicVerb,
        fidelity,
    ))
}

/// The mask's own column, which needs no arguments but the call.
///
/// One intent, one family and one fidelity wherever it is painted: a mask is
/// the same act on all four representations, which is the fact
/// `the_mask_is_the_same_call_wherever_it_is_painted` holds.
pub(crate) const fn mask_field(entry_point: &'static str) -> Option<Binding> {
    Some(Binding::new(
        entry_point,
        SemanticIntent::Mask,
        ExecutionFamily::MaskField,
        Fidelity::Native,
    ))
}

/// A pinned combination of existing engine verbs or parameters, standing in
/// for a native verb the engine does not have.
pub(crate) const fn recipe(entry_point: &'static str, intent: SemanticIntent) -> Option<Binding> {
    Some(Binding::new(
        entry_point,
        intent,
        ExecutionFamily::Recipe,
        Fidelity::Recipe,
    ))
}

/// What one tool invokes on each of the four representations.
///
/// A field is `None` where that representation has no verb for the tool. A
/// [`Binding`] is carried rather than a boolean so that "does this apply here"
/// and "what does it call" cannot disagree — they are one row — and rather
/// than the bare name it used to hold, so that *how* the call answers the
/// tool's label is in the value too instead of in the prose beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verbs {
    pub sdf: Option<Binding>,
    pub voxel: Option<Binding>,
    pub mesh: Option<Binding>,
    /// The hierarchy's column.
    ///
    /// Almost the mesh column, and that is the engine's doing rather than a
    /// convenience: `clay_multires_sculptor_stamp` takes a
    /// `clay_mesh_brush_desc` and runs the fixed sculptor over the active
    /// level's own mesh, so "the same verbs, the same falloffs, the same mask,
    /// the same alpha and the same automasking — because it is the same code".
    /// One brush runtime across the three representations is ClayCore #419,
    /// and this column is where that shows.
    ///
    /// It is not a copy of the mesh column, though, and the three places it
    /// differs are the three places a table beats a rule: the two colour
    /// brushes are absent, because a hierarchy stores where a vertex went and
    /// not what colour it is, and the smooth names a different entry point,
    /// because a smooth here picks which frequency it acts on.
    pub multires: Option<Binding>,
    /// The adaptive surface's column.
    ///
    /// The mesh column through `clay_dynamic_sculptor_apply_stroke`, less the
    /// one brush the engine declines: Layer deposits up to a ceiling measured
    /// against where each vertex stood when the stroke began, and an adaptive
    /// stroke creates vertices that did not exist then. That absence is a
    /// [`ToolNote`] rather than a gap — see [`ToolNote::DynamicHasNoLayer`].
    pub dynamic: Option<Binding>,
}

impl Verbs {
    /// The whole binding for a representation, or nothing where the tool has
    /// none there.
    pub fn on(self, representation: Representation) -> Option<Binding> {
        match representation {
            Representation::Sdf => self.sdf,
            Representation::Voxel => self.voxel,
            Representation::Mesh => self.mesh,
            Representation::Multires => self.multires,
            Representation::Dynamic => self.dynamic,
        }
    }

    /// Just the name, for the callers checking a string against the engine
    /// rather than reading the row.
    pub fn entry_point_on(self, representation: Representation) -> Option<&'static str> {
        self.on(representation).map(|binding| binding.entry_point)
    }

    /// How many representations this tool reaches.
    pub fn count(self) -> usize {
        Representation::ALL
            .into_iter()
            .filter(|representation| self.on(*representation).is_some())
            .count()
    }
}

/// The engine entry points a row of the table names.
///
/// A row is written for a reader — `clay_mesh_sculptor_apply_stroke (DRAW)`
/// says both which call runs and which brush it carries — so the names in it
/// have to be picked back out before anything can check them. The rule is the
/// narrowest one that works: a maximal run of `[A-Za-z0-9_]` beginning
/// `clay_`. The engine's *constants* are spelled `CLAY_OP_RELIEF` and are
/// deliberately not entry points; a qualifier in prose — `(cut-only)`,
/// `(clamped)` — names nothing and yields nothing.
///
/// A row that names more than one call spells each of them out in full. It is
/// tempting to abbreviate a family as `clay_sdf_move_begin/update/commit`, and
/// that costs exactly what this function exists to prevent: two of the three
/// names would not be there to check.
pub fn entry_points(verb: &str) -> Vec<&str> {
    let symbolic = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
    let mut named = Vec::new();
    let mut token: Option<usize> = None;
    for (at, ch) in verb.char_indices() {
        match (symbolic(ch), token) {
            (true, None) => token = Some(at),
            (false, Some(from)) => {
                push_entry_point(&mut named, &verb[from..at]);
                token = None;
            }
            _ => {}
        }
    }
    if let Some(from) = token {
        push_entry_point(&mut named, &verb[from..]);
    }
    named
}

fn push_entry_point<'a>(named: &mut Vec<&'a str>, token: &'a str) {
    if token.starts_with("clay_") && !named.contains(&token) {
        named.push(token);
    }
}

/// Every engine entry point the whole capability table names, deduplicated.
///
/// Both tables: the tools' and [`LayerOperation`]'s. What this is for is the
/// check the domain cannot make itself — `clayspace-model` may not link the
/// engine, so a verb here is a string and nothing in this crate can tell a
/// live symbol from one the engine renamed two releases ago. A crate that
/// depends on both takes this list and asks the bindings.
pub fn every_entry_point() -> std::collections::BTreeSet<&'static str> {
    let mut named = std::collections::BTreeSet::new();
    let mut rows = Vec::new();
    for tool in ToolKind::ALL {
        rows.push(tool.verbs());
    }
    for operation in LayerOperation::all() {
        rows.push(operation.verbs());
    }
    for row in rows {
        for representation in Representation::ALL {
            let Some(verb) = row.entry_point_on(representation) else {
                continue;
            };
            named.extend(entry_points(verb));
        }
    }
    named
}

/// Something done to a layer that a gesture cannot express.
///
/// The design calls this the second verb beside `apply_stroke`. A deformer
/// states something about the *form* — no centre, no radius, no falloff — and
/// a cage is dragged by its control points, so neither has a gesture to be
/// resolved from. Widening a stroke to carry them would make every caller and
/// every double handle cases that are not strokes, on the one path a latency
/// budget is measured against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerOperation {
    /// The cross-section scale ramps along an axis.
    Taper {
        axis: [f32; 3],
        span: f32,
        scale_start: f32,
        scale_end: f32,
    },
    /// Rotation about an axis ramps along it, in radians across the span.
    Twist {
        axis: [f32; 3],
        span: f32,
        angle: f32,
    },
    /// Seals perforations, by the same pocket rule the cavity fill uses.
    ///
    /// Voxel-only: a field has no holes to close — it is continuous — and a
    /// mesh's topology may not change, which closing one would.
    CloseHoles { passes: i32 },
    /// Fills every empty cell the outside cannot reach.
    ///
    /// A sealed void is invisible until something needs the model to be solid,
    /// which is why this is a *pre-bake* verb rather than a sculpting one.
    FillVoids,
    /// Refines the grid over a region, rather than everywhere.
    ///
    /// The point of a level stack: block out coarse, then pay for detail only
    /// where the detail goes.
    RefineRegion { min: [f32; 3], max: [f32; 3] },
    /// A free-form deformation cage, by the offset of one control point.
    ///
    /// Sent per drag rather than as a whole cage: the interface owns the cage
    /// and the document owns the vertices, and shipping the cage across on
    /// every drag would copy it for each control point moved.
    LatticeDrag {
        divisions: [i32; 3],
        at: [i32; 3],
        offset: [f32; 3],
    },
}

impl LayerOperation {
    /// One of each, with the arguments the application itself would send.
    ///
    /// For anything that has to exercise all of them — the performance gate
    /// measures one figure per entry, so an operation missing from here is an
    /// operation nobody is timing. `every_operation_is_in_all` is what keeps
    /// it complete: its `match` is exhaustive, so a variant added to this enum
    /// stops that test compiling until it is given arguments to be measured
    /// with.
    ///
    /// The deform panel's two verbs come through
    /// [`crate::DeformSettings::operation`] rather than being written out
    /// again here, so the figures describe what the panel actually sends.
    pub fn all() -> [Self; 6] {
        let deform = |verb| {
            crate::DeformSettings {
                verb,
                ..crate::DeformSettings::default()
            }
            .operation()
        };
        [
            deform(crate::DeformVerb::Taper),
            deform(crate::DeformVerb::Twist),
            // A corner of the smallest cage the panel offers, pulled by a
            // tenth of a unit.
            Self::LatticeDrag {
                divisions: [crate::lattice::MIN_DIVISIONS; 3],
                at: [0, 0, 0],
                offset: [0.1, 0.0, 0.0],
            },
            // One pass, which is what the menu item sends.
            Self::CloseHoles { passes: 1 },
            Self::FillVoids,
            // A region around the origin, well inside any reference subject.
            Self::RefineRegion {
                min: [-0.3, -0.3, -0.3],
                max: [0.3, 0.3, 0.3],
            },
        ]
    }

    /// What the history calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Taper { .. } => "taper",
            Self::Twist { .. } => "twist",
            Self::LatticeDrag { .. } => "lattice",
            Self::CloseHoles { .. } => "close holes",
            Self::FillVoids => "fill voids",
            Self::RefineRegion { .. } => "refine",
        }
    }

    /// The engine verb this operation invokes, per representation.
    ///
    /// So a refusal can say where the operation *does* apply rather than
    /// restating one representation's answer for all of them — which is what a
    /// hardcoded refusal did, telling a sculptor on a field that filling voids
    /// "applies to mesh layers".
    pub fn verbs(self) -> Verbs {
        // Every one of the six is `multires: None`, and that is six separate
        // absences rather than one. The three forward point maps have no
        // hierarchy entry point at all — there is no `clay_multires_*_deform`
        // and no `clay_multires_*_lattice` in the ABI, because a level above
        // the cage is *derived*, so there is nothing to push a vertex of
        // through a map and have the result survive the next evaluation. The
        // three grid repairs are grid repairs.
        //
        // `RefineRegion` is the one worth pausing on, because its own doc
        // sentence — "block out coarse, then pay for detail only where the
        // detail goes" — is the multiresolution idea word for word, and a
        // reader will reach for it. It is still `None`: a grid refines a
        // *region*, and a hierarchy subdivides a whole level, which is
        // [`crate::multires::MultiresLevels::subdivided`] and priced by
        // [`crate::multires::SubdivisionCost`] rather than by a `Cost` in
        // cells.
        match self {
            // The three forward point maps are `SurfaceMove` and not a
            // deformation intent of their own: what a taper, a twist and a
            // dragged cage all do is decide where each vertex goes, which is
            // the drag's intent applied to the whole form rather than under a
            // brush. A separate intent would be a word for "by an operation
            // rather than by a gesture", which is the distinction
            // [`LayerOperation`] already *is*.
            Self::Taper { .. } | Self::Twist { .. } => Verbs {
                sdf: None,
                voxel: None,
                mesh: mesh_verb(
                    "clay_mesh_sculptor_deform",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                multires: None,
                dynamic: None,
            },
            Self::LatticeDrag { .. } => Verbs {
                sdf: None,
                voxel: None,
                mesh: mesh_verb(
                    "clay_mesh_sculptor_lattice",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                multires: None,
                dynamic: None,
            },
            // Both repairs put material where there was none — the hole's
            // wall, the void's interior — which is why they are `VolumeAdd`
            // and not a repair intent. A verb that only ever closed what is
            // already closed would have nothing to do.
            Self::CloseHoles { .. } => Verbs {
                sdf: None,
                voxel: voxel_verb(
                    "clay_voxel_repair_close_holes",
                    SemanticIntent::VolumeAdd,
                    Fidelity::Native,
                ),
                mesh: None,
                multires: None,
                dynamic: None,
            },
            Self::FillVoids => Verbs {
                sdf: None,
                voxel: voxel_verb(
                    "clay_voxel_repair_fill_voids",
                    SemanticIntent::VolumeAdd,
                    Fidelity::Native,
                ),
                mesh: None,
                multires: None,
                dynamic: None,
            },
            // `TopologyRebuild` for the reason the doc sentence above gives
            // for it not being the hierarchy's: it changes how finely the form
            // is stored over a region and not what the form is.
            Self::RefineRegion { .. } => Verbs {
                sdf: None,
                voxel: voxel_verb(
                    "clay_voxel_add_level_region",
                    SemanticIntent::TopologyRebuild,
                    Fidelity::Native,
                ),
                mesh: None,
                multires: None,
                dynamic: None,
            },
        }
    }

    /// Which representations can accept it.
    ///
    /// Mesh only, for all three. Taper and twist exist on the SDF side as
    /// deformers on the edit list rather than as operations on a layer, and a
    /// cage is deliberately mesh-only: ZBrush and Blender both apply FFD
    /// forward to vertices, which a mesh allows and an implicit field does not.
    /// One lookup into [`LayerOperation::verbs`], so the two cannot disagree
    /// about where an operation applies.
    ///
    /// Taper, twist and the cage are mesh-only because they are forward point
    /// maps, which a mesh allows and an implicit field does not. The pre-bake
    /// verbs are voxel-only because a field is continuous and has no holes to
    /// close, and a mesh's topology may not change.
    pub fn applies_to(self, representation: Representation) -> bool {
        self.verbs().on(representation).is_some()
    }
}

/// What the active layer can accept right now.
///
/// Grouped rather than passed as loose flags because the list grows: a tool
/// can be unavailable for the representation, the protection, the visibility
/// or a missing attribute, and a call site that forgot one of those silently
/// offered a tool that would refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerState {
    pub representation: Representation,
    /// Whether the layer accepts edits at all — not locked, not ghosted.
    pub editable: bool,
    /// Whether the layer is drawn. An edit to a hidden layer lands where
    /// nothing shows it, which is indistinguishable from the tool not working.
    pub visible: bool,
    /// Whether the layer carries geometry a verb can reach.
    ///
    /// A mesh layer is recorded before its triangles arrive — the row exists
    /// so the rest of the application can talk about it — and until they do,
    /// there is nothing for a brush to move. Offering the sixteen mesh verbs
    /// on an empty row and letting each fail with "no mesh layer named X" is
    /// the shape this exists to prevent.
    pub carries_geometry: bool,
    /// Whether the next stroke would enter a pass rather than the form under
    /// them.
    ///
    /// A hierarchy's alone: it is the only representation whose stroke has two
    /// places it could land, and the row the sculptor selected is what decides
    /// which. Every other representation answers `false` and nothing reads it,
    /// because a tool gated on a pass has no meaning where there is no stack.
    ///
    /// Here rather than in the engine's refusal because a tool that cannot be
    /// used has to be *unselectable* before it is unusable — the same reason
    /// visibility and the missing cage are here. Erasing is the one verb that
    /// needs it: it takes the selected pass toward zero, so on the form there
    /// is nothing it could mean.
    pub stroke_lands_in_a_pass: bool,
}

impl LayerState {
    /// The common case: an ordinary editable, visible layer.
    ///
    /// With no pass selected, which is what a layer that is not a hierarchy
    /// always answers and what a fresh hierarchy answers too — a stack starts
    /// empty and a stroke lands in the form.
    pub fn editable(representation: Representation) -> Self {
        Self {
            representation,
            editable: true,
            visible: true,
            carries_geometry: true,
            stroke_lands_in_a_pass: false,
        }
    }

    /// The same, with the stroke landing in a selected pass.
    pub fn in_a_pass(representation: Representation) -> Self {
        Self {
            stroke_lands_in_a_pass: true,
            ..Self::editable(representation)
        }
    }
}

/// A tool the interface can offer.
///
/// Named as the interface names them, which is Portuguese, because the label
/// and the tool are the same thing to a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolKind {
    /// Displaces the accumulated surface along its normal. ZBrush's Standard.
    Padrao,
    /// Drags the assembled surface. Buds rather than stretches.
    Mover,
    /// The same drag, weighted by distance *along the material*.
    ///
    /// Its own tool rather than a modifier on [`ToolKind::Mover`]: the engine
    /// documents the two as different operations with different reach — the
    /// Euclidean drag is a deformer on each item it touches, this bakes a
    /// re-sampled volume — and measured on two fingers 0.32 apart joined only
    /// through a palm, a Euclidean drag at radius 0.5 pulls the far one and
    /// this does not. A modifier that silently changed which algorithm runs
    /// would hide that.
    MoverTopologico,
    /// Relief on the SDF side; dilation on the voxel side.
    Inflar,
    /// Relax on the SDF side; a majority filter on the voxel side.
    Suavizar,
    /// Freezes a region against every verb.
    Mascara,
    /// Snakehook: pulls a lobe out, adding material.
    Puxar,
    /// Flatten and smooth from one snapshot.
    Raspar,
    /// Flatten in cut-only mode, which is what keeps a facet crisp.
    Planar,
    /// Fills narrow pockets. Local, so it fills what is narrow, not enclosed.
    Preencher,
    /// Magnify with a negative strength.
    Pincar,
    /// A stroke with clamped accumulation.
    Camada,
    /// Smudge: drags the surface skin, leaving the interior.
    Nudge,
    /// hPolish: planes without filling.
    Polir,
    /// Relax, applied as a brush.
    Relaxar,
    /// The cut tool. The practitioners' ninety-percent tool.
    Trim,
    /// Draw's deposit clamped to a plane. Mesh-side, where the field's
    /// equivalent is a relief stroke against a flattened region.
    Argila,
    /// A tight negative draw and a pinch in one stamp. ZBrush's Crease.
    Vinco,
    /// Blends vertex colour toward the brush's own. Moves no vertex.
    Pintar,
    /// Drags existing vertex colour along the stroke. Moves no vertex.
    Borrar,
    /// Removes cells under the brush.
    ///
    /// Voxel-only, and for two different reasons: a mesh's topology may not
    /// change, and on the SDF side removing material is a subtracting edit
    /// rather than a brush.
    Apagar,
}

impl ToolKind {
    /// Whether the tool acts on the path rather than stamping along it.
    ///
    /// A stamping tool deposits at each position and a single position is a
    /// complete instruction. A dragging tool is told *from where to where*, so
    /// one position says nothing: [`ToolKind::Mover`] with a single sample has
    /// no displacement and moves nothing.
    ///
    /// This matters because a live stroke is sent in segments as the pointer
    /// travels. A segment for a dragging tool has to carry the position it
    /// started from, or every segment is a gesture of length zero — which is
    /// exactly what happened, and what
    /// `every_sdf_stroke_tool_changes_the_surface` caught.
    /// Whether the tool acts on a whole region rather than stamping into it.
    ///
    /// Suavizar, Relaxar, Planar and Polir sample the region a gesture covered
    /// into a volume, modify that volume, and replace the region with it. That
    /// is one operation on one region, and it does not decompose: applying it
    /// to each segment of a stroke stacks a replacement per segment over
    /// overlapping ground, and the seams between them read as a crumbling,
    /// blocky patch. Measured, a stroke applied in eight segments left the
    /// surface roughly twice as rough as the same stroke applied once.
    ///
    /// The cost is that these four cannot be sent segment by segment. On a
    /// field the two it offers are previewed instead — Suavizar through the
    /// engine's smoothing transaction, Planar by laying the gesture-so-far
    /// down, reading it and taking it back — and a layer that can preview
    /// neither lands them when the pointer comes up.
    pub fn is_region_based(self) -> bool {
        matches!(
            self,
            Self::Suavizar | Self::Relaxar | Self::Planar | Self::Polir
        )
    }

    /// Whether a gesture has to arrive **whole** rather than in segments.
    ///
    /// [`ToolKind::is_region_based`] is the part of this that does not depend
    /// on what is being sculpted. The other part does, and there is one:
    ///
    /// A drag on a **grid** does not decompose into a series of shorter drags.
    /// The engine's grab resamples occupancy through the inverse map, rounding
    /// per axis, and weights the displacement by the falloff across its
    /// region — so a one-cell drag moves the middle of the region one cell and
    /// its rim not at all, and inside solid material that is *no change at
    /// all*. Measured on a slab with a 0.35 drag: delivered whole it moved the
    /// material at every brush size tried; delivered as the eight segments a
    /// pointer makes, seven of the eight changed nothing and the eighth
    /// changed almost nothing.
    ///
    /// So the drag is held and applied once, from its anchor, which is the
    /// same trade the region tools make and for the same reason: it does not
    /// preview while the pointer moves, and it lands when the pointer comes
    /// up. The alternative — reverting the last segment and reapplying the
    /// whole gesture, which is what the mesh drag does — needs a record of
    /// what a voxel edit changed, and a grid has none.
    /// A hierarchy answers the mesh's way rather than the grid's, and the
    /// reason is the one the paragraph above gives for why the grid is the
    /// exception: the alternative to holding a drag is reverting the last
    /// segment and reapplying the whole gesture, which needs an exact record
    /// of what the last segment changed. A grid has none. A hierarchy has one
    /// — the layered stroke's cancel is defined to be exact, because a layered
    /// write is `L += dE` and the only exact restore is the recorded `before`
    /// values, so the record exists from the first stamp. So a drag on a
    /// hierarchy previews as it moves, as it does on a mesh.
    pub fn holds_the_whole_gesture(self, representation: Representation) -> bool {
        self.is_region_based() || (self == Self::Mover && representation == Representation::Voxel)
    }

    /// When an adaptive surface remeshes around this tool's deformation.
    ///
    /// Per verb rather than one setting, because brushes fail differently on
    /// topology too coarse for them: a Grab that remeshed first would refine
    /// triangles it is about to stretch, so it refines what the stretch made;
    /// a deposit onto coarse triangles is a smooth bump where the brush
    /// promised an edge, so Clay refines first; Snake Hook re-anchors between
    /// stamps and needs geometry on both sides of the pull.
    ///
    /// The engine owns the schedule (`default_timing` in ClayCore's local
    /// remesher) and applies it per stamp; this is the same table stated where
    /// the interface and the agent can read it, and the engine suite measures
    /// the two against each other. `None` for a tool with no Dynamic binding.
    pub fn remesh_timing(self) -> Option<RemeshTiming> {
        if !self.exists_on(Representation::Dynamic) || self.is_mask_tool() {
            return None;
        }
        Some(match self {
            Self::Mover | Self::Relaxar => RemeshTiming::AfterBrush,
            Self::Puxar => RemeshTiming::BeforeAndAfter,
            _ => RemeshTiming::BeforeBrush,
        })
    }

    pub fn is_path_driven(self) -> bool {
        matches!(
            self,
            Self::Mover | Self::MoverTopologico | Self::Puxar | Self::Nudge
        )
    }
}

/// When an adaptive surface remeshes relative to a stamp's deformation.
///
/// See [`ToolKind::remesh_timing`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RemeshTiming {
    /// Refine first, so the deformation has the geometry it needs.
    BeforeBrush,
    /// Deform first, then refine what the deformation stretched.
    AfterBrush,
    /// Both: the pull needs somewhere to stand and leaves a stretch behind.
    BeforeAndAfter,
}

impl RemeshTiming {
    pub fn label(self) -> &'static str {
        match self {
            Self::BeforeBrush => "before",
            Self::AfterBrush => "after",
            Self::BeforeAndAfter => "before and after",
        }
    }
}

/// A caveat about what a tool does on *one* representation in particular.
///
/// A tool's own sentence describes the intent, which is the same everywhere;
/// this is for the cases where the engine's verb on one representation differs
/// from the others in a way a sculptor will notice mid-stroke. Named here
/// rather than written as interface text because *which pairs carry a caveat*
/// is a fact about the engine's vocabulary, and the interface layer is not
/// allowed to know it. The wording lives with the other strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolNote {
    /// A grid's flatten fills hollows below the plane as well as taking
    /// material off above it, where the field and mesh verbs cut only.
    VoxelPlanarIsTwoSided,
    /// A hierarchy's smooth picks a frequency, where the other three have one.
    ///
    /// `clay_multires_sculpt_layer_stroke_smooth` takes a mode, and the split
    /// is representational rather than a setting: the hierarchy already stores
    /// the form and the detail in different arrays, so smoothing the positions,
    /// smoothing the coefficients and smoothing the form *with the detail
    /// carried through unchanged* are three different passes. The third is the
    /// one an artist correcting anatomy under pores is asking for, and it is
    /// the one that cannot exist on a flat mesh — there is nothing under the
    /// surface to smooth separately from it.
    MultiresSmoothChoosesAFrequency,
    /// A hierarchy carries no colour of its own, so the colour brushes are not
    /// offered on one.
    ///
    /// An absence worth explaining because it otherwise reads
    /// as an oversight: every other mesh brush is on this shelf, these two are
    /// missing, and the shelf cannot say why. What it does not do is leave the
    /// sculptor without a route — the cage's colours are subdivided all the way
    /// up, so painting before the hierarchy is built, or baking a level back to
    /// a mesh and painting that, both work.
    MultiresStoresNoColour,
    /// On a hierarchy, erasing takes the *selected pass* toward zero rather
    /// than removing material.
    ///
    /// The one tool whose meaning changes completely between two of the four
    /// columns, and the caveat is therefore not a nuance but the whole verb: on
    /// a grid Apagar clears the cells the brush covers, and on a hierarchy
    /// there are no cells to clear — the engine's entry point walks the
    /// selected pass's detail channel toward zero and touches neither the base
    /// nor any other pass. An artist who reads the grid's meaning onto the
    /// hierarchy expects the surface to be taken away and gets the pass's own
    /// deposit fading out instead, which is a surprise worth one sentence.
    MultiresEraseTakesThisPassToZero,
    /// A field's Standard is a relief stroke, and relief is an **Inflate**.
    ///
    /// The engine's own Standard preset — `clay_brush_frame`'s Draw column —
    /// displaces a stamp's footprint along **one averaged normal**. Relief
    /// offsets the accumulated field, and offsetting a distance moves every
    /// point of the isosurface along the field's own gradient: each point
    /// along *its own* normal, which is the Inflate column. ClayCore v0.120.0
    /// measured the two frames against relief and named it (#615, #618): the
    /// inflate reference sits 0.000 of the amplitude away on a sphere, a
    /// saddle and a bowl, the draw reference 0.017, 0.077 and 0.027.
    ///
    /// Which makes this the one note here about a tool that does what its
    /// label says *almost* everywhere. What decides how far off it is, is how
    /// far the normals under the stamp spread: on a form smooth at the brush's
    /// scale a few percent of the amplitude, and on a feature narrower than
    /// the brush the whole of it — a fin takes the stamp on its flanks as well
    /// as its top and comes out thicker, where a Standard would have drawn on
    /// the top and left the thickness alone.
    ///
    /// Worth a sculptor's attention rather than only a maintainer's, because
    /// the surprise arrives mid-stroke and looks like a brush that is too big:
    /// a ridge detailed onto a thin form fattens the form. The remedy is the
    /// one the note names — a brush smaller than the feature — and the faithful
    /// Standard the engine measured is not shipped, being a per-item warp per
    /// dab at nine times relief's cost.
    SdfStandardIsAnInflate,
    /// A grid's crease is a narrow, fixed erode recipe, not a sharpening verb.
    VoxelCreaseIsErodeRecipe,
    /// A grid has no colour-smear verb; its geometric smudge is Nudge.
    VoxelSmearHasNoColourVerb,
    /// Binary occupancy cannot hold Clay's clamped buildup.
    VoxelClayHasNoBuildup,
    /// An adaptive surface has no Layer brush, and that is structural.
    ///
    /// Layer deposits up to a ceiling measured from where each vertex stood
    /// when the stroke began. On an adaptive surface a split creates vertices
    /// mid-stroke that have no such reference, so the engine refuses the verb
    /// rather than letting it become Draw for the new vertices and Layer for
    /// the old ones. Stated, because a missing Layer on a shelf that carries
    /// every other mesh brush reads as an oversight.
    DynamicHasNoLayer,
    /// A field has one flatten, and it is Planar's.
    ///
    /// A note on an absence. The mesh's Polish is a flatten that smooths the
    /// high points it leaves, and the field's bake has no second pass to do
    /// that with: the row that used to be here was Planar's call with a
    /// qualifier nothing sent, and the two measured identical (#179, #203).
    SdfPolishIsPlanar,
    /// A field has no vertices to relax, so its Relax was its Smooth.
    ///
    /// A note on an absence, for the same reason: the row named Suavizar's
    /// relax under a second word, and the two gave identical results.
    SdfRelaxIsSmooth,
}

impl ToolNote {
    pub const ALL: [ToolNote; 11] = [
        Self::VoxelPlanarIsTwoSided,
        Self::MultiresSmoothChoosesAFrequency,
        Self::MultiresStoresNoColour,
        Self::MultiresEraseTakesThisPassToZero,
        Self::SdfStandardIsAnInflate,
        Self::VoxelCreaseIsErodeRecipe,
        Self::VoxelSmearHasNoColourVerb,
        Self::VoxelClayHasNoBuildup,
        Self::DynamicHasNoLayer,
        Self::SdfPolishIsPlanar,
        Self::SdfRelaxIsSmooth,
    ];
}

/// A tool the layer switched to does not carry, and the one standing in for it.
///
/// Held for as long as the stand-in is in hand, and not only for the moment it
/// arrived: a sculptor who moved to a field with Raspar and back to a grid gets
/// Raspar back, because they never chose Planar, and an agent reading `state`
/// after the switch has to be able to tell a tool it chose from one it was
/// given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Substitution {
    /// The tool the sculptor chose.
    pub chosen: ToolKind,
    /// The tool in hand instead, from [`ToolKind::substitute_on`].
    pub standing_in: ToolKind,
    /// Where `chosen` has no verb.
    pub representation: Representation,
}

impl Substitution {
    /// The substitution as an agent is told it, by the tools' stable keys.
    ///
    /// English and keyed rather than labelled, for the reason
    /// [`SemanticIntent::label`] is: the sentence crosses the agent door, where
    /// a tool is named by its key, and a translated label would be a name the
    /// caller cannot hand back.
    pub fn describe(self) -> String {
        format!(
            "tool '{}' has no verb on {} layers; '{}' stands in for it until a tool is chosen",
            self.chosen.key(),
            self.representation.label(),
            self.standing_in.key()
        )
    }
}

/// Why a tool cannot be used right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// The tool is driven by a different gesture than the one attempted.
    WrongGesture { needs: &'static str },
    /// The tool has no verb on this representation.
    ///
    /// The shelf answers this by not showing the tool, so a user should not
    /// meet it. It is what a caller gets for asking anyway.
    ///
    /// Carries the whole row rather than just the active representation, so
    /// the message can say where the tool *does* apply — which is the useful
    /// half, and what a bare "not here" loses.
    NoVerbHere {
        active: Representation,
        /// Boxed: a row is one binding per representation, and carried inline
        /// it made every `ModelError` as large as the whole table row.
        verbs: Box<Verbs>,
        /// Why the tool is missing *here in particular*, where saying so is
        /// worth more than the list of where it is not missing.
        ///
        /// Almost always `None`: "applies to voxel and mesh layers; this one
        /// is a field" answers the question for nearly every absence, and a
        /// second sentence on every refusal is a second sentence nobody reads.
        /// It is here for the absence that reads as an oversight rather than
        /// as a boundary — see [`ToolNote::MultiresStoresNoColour`].
        ///
        /// The wording lives with the other strings, as every [`ToolNote`]'s
        /// does; what is decided here is *which* absences carry one.
        note: Option<ToolNote>,
    },
    /// The layer is ghosted or locked.
    LayerProtected,
    /// The layer is hidden, so an edit would land where nothing is drawn.
    LayerHidden,
    /// A deformation cage is up around the layer.
    ///
    /// A cage owns the whole viewport while it stands — a press that misses a
    /// control point orbits rather than sculpting — and the rule was only ever
    /// enforced where the pointer is handled, so a caller that reached the
    /// ViewModel another way sculpted the very form the cage was there to
    /// bend. The stroke it left survived the cage being applied and was
    /// impossible to attribute afterwards.
    LayerCaged,
    /// The layer carries no attribute this tool needs — a mesh with no colour
    /// for a colour brush, say. Produced by the tools that require one.
    MissingAttribute { needs: &'static str },
    /// The tool writes into a pass and the sculptor has the form selected.
    ///
    /// A hierarchy's alone, and not the same refusal as a missing attribute:
    /// nothing is absent from the layer. The stack may be full of passes; the
    /// row that takes the stroke is simply the form under them, and this verb
    /// has nothing to say there.
    NeedsAPass,
}

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoVerbHere { active, verbs, .. } => {
                let on: Vec<&str> = Representation::ALL
                    .into_iter()
                    .filter(|r| verbs.on(*r).is_some())
                    .map(Representation::label)
                    .collect();
                match on.len() {
                    0 => write!(f, "has no verb on any representation"),
                    _ => write!(
                        f,
                        "applies to {} layers; this one is {}",
                        on.join(" and "),
                        active.label()
                    ),
                }
            }
            Self::WrongGesture { needs } => {
                write!(f, "draw {needs} rather than a stroke across the surface")
            }
            Self::LayerProtected => f.write_str("this layer is locked"),
            Self::LayerHidden => f.write_str("this layer is hidden"),
            Self::LayerCaged => f.write_str(
                "a deformation cage is up on this layer; apply it or take it down before sculpting",
            ),
            Self::MissingAttribute { needs } => {
                write!(f, "this layer carries no {needs}")
            }
            Self::NeedsAPass => {
                f.write_str("select a pass first; this verb acts on the selected pass alone")
            }
        }
    }
}

impl ToolKind {
    /// Every tool, in the order the brush shelf presents them.
    pub const ALL: [ToolKind; 21] = [
        Self::Padrao,
        Self::Inflar,
        Self::Suavizar,
        Self::Mover,
        Self::MoverTopologico,
        Self::Pincar,
        Self::Raspar,
        Self::Planar,
        Self::Preencher,
        Self::Camada,
        Self::Mascara,
        Self::Puxar,
        Self::Polir,
        Self::Relaxar,
        Self::Nudge,
        Self::Trim,
        Self::Argila,
        Self::Vinco,
        Self::Pintar,
        Self::Borrar,
        Self::Apagar,
    ];

    /// A stable name for storage.
    ///
    /// Not [`ToolKind::label`], which is interface text: it is Portuguese, it is
    /// translated, and a stored preference that used it would read differently
    /// in a different language. Not the position in [`ToolKind::ALL`] either —
    /// that is presentation order, and reordering the shelf would silently
    /// reinterpret what a sculptor had starred.
    ///
    /// The same reasoning as [`crate::Shape::key`], and the same shape.
    pub fn key(self) -> &'static str {
        match self {
            Self::Padrao => "standard",
            Self::Inflar => "inflate",
            Self::Suavizar => "smooth",
            Self::Mover => "move",
            Self::MoverTopologico => "move-topological",
            Self::Pincar => "pinch",
            Self::Raspar => "scrape",
            Self::Planar => "planar",
            Self::Preencher => "fill",
            Self::Camada => "layer",
            Self::Mascara => "mask",
            Self::Puxar => "snake-hook",
            Self::Polir => "polish",
            Self::Relaxar => "relax",
            Self::Nudge => "nudge",
            Self::Trim => "trim",
            Self::Argila => "clay",
            Self::Vinco => "crease",
            Self::Pintar => "paint",
            Self::Borrar => "smudge",
            Self::Apagar => "erase",
        }
    }

    /// The label the interface shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Padrao => "Padrão",
            Self::Mover => "Mover",
            Self::MoverTopologico => "Mover Topológico",
            Self::Inflar => "Inflar",
            Self::Suavizar => "Suavizar",
            Self::Mascara => "Máscara",
            Self::Puxar => "Puxar",
            Self::Raspar => "Raspar",
            Self::Planar => "Planar",
            Self::Preencher => "Preencher",
            Self::Pincar => "Pinçar",
            Self::Camada => "Camada",
            Self::Nudge => "Nudge",
            Self::Polir => "Polir",
            Self::Relaxar => "Relaxar",
            Self::Trim => "Trim",
            Self::Argila => "Argila",
            Self::Vinco => "Vinco",
            Self::Pintar => "Pintar",
            Self::Borrar => "Borrar",
            Self::Apagar => "Apagar",
        }
    }
    /// Every engine verb this tool names, for the diagnostics report.
    ///
    /// Derived from [`ToolKind::verbs`] rather than restated. It used to be a
    /// second `match`, which is two places to change when a binding moves and
    /// one place to forget.
    pub fn engine_verbs(self) -> String {
        let verbs = self.verbs();
        let mut named: Vec<&'static str> = Vec::new();
        for binding in Representation::ALL
            .map(|representation| verbs.on(representation))
            .into_iter()
            .flatten()
        {
            if !named.contains(&binding.entry_point) {
                named.push(binding.entry_point);
            }
        }
        named.join(" / ")
    }

    /// The engine verb this tool invokes on each representation.
    ///
    /// `None` where the representation has no verb for it. This is the table
    /// the shelf, the availability rules and the tests all read; nothing else
    /// may decide where a tool applies, or they can drift apart again.
    ///
    /// Each column is a [`Binding`] rather than a name, which is where the
    /// comments around these rows have been going: what a row says about
    /// itself in prose can be read by a person and by nothing else. The intent
    /// and the fidelity are the two that earn their place here immediately —
    /// they are what tell two rows naming one call apart, and what tie a
    /// [`ToolNote`] to the row it is the sentence for.
    pub fn verbs(self) -> Verbs {
        // Written out per tool rather than grouped, so that adding a verb on a
        // representation is an edit to one line and reading what a tool does
        // is one row.
        match self {
            // The grid's column is the deposit and not a sculpt verb, which is
            // not the obvious answer and is the one traced: occupancy is
            // binary, so "displace the surface along its normal" on a grid is
            // "set the cells the brush covers". The row said
            // `clay_voxel_sculpt_inflate` and was reading the *shape* of the
            // tool off its field counterpart rather than off the call.
            //
            // Two of the four are approximations and say so. The grid's is the
            // deposit standing in for a displacement; the field's is relief,
            // which the engine measured to be an Inflate rather than a
            // Standard — see Inflar's row, and the caveat this one carries.
            Self::Padrao => Verbs {
                sdf: field_op(
                    "clay_layer_apply_stroke (CLAY_OP_RELIEF)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Approximation,
                ),
                voxel: voxel_verb(
                    "clay_voxel_set_brush",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Approximation,
                ),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (DRAW)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (DRAW)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (DRAW)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Native,
                ),
            },
            // The field's column is relief, which it shares with Padrão, and
            // that sharing is the right way round rather than a gap: relief
            // offsets the accumulated field, and offsetting a distance moves
            // every point of the isosurface along the field's own gradient —
            // each point along its own normal, which is the Inflate frame.
            // ClayCore v0.120.0 measured it (#615, #618): against a
            // frame-isolated inflate reference relief sits 0.000 of the
            // amplitude on a sphere, a saddle and a bowl. **Relief is the SDF
            // Inflate.** It is Padrão that is the approximation here, which is
            // what `ToolNote::SdfStandardIsAnInflate` tells a sculptor and
            // what the two fidelities now say in the table itself: this row is
            // `Native` and Padrão's is an `Approximation` of a Standard the
            // engine can spell and does not ship. The mark the two leave
            // differs in its profile — wider and lower than Padrão's under
            // either Acumular setting — which is the last thing left for one
            // call under two labels to differ in, and what
            // `inflate_is_broader_and_lower_than_standard` measures.
            Self::Inflar => Verbs {
                sdf: field_op(
                    "clay_layer_apply_stroke (CLAY_OP_RELIEF)",
                    SemanticIntent::VolumeInflate,
                    Fidelity::Native,
                ),
                voxel: voxel_verb(
                    "clay_voxel_sculpt_inflate",
                    SemanticIntent::VolumeInflate,
                    Fidelity::Native,
                ),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (INFLATE)",
                    SemanticIntent::VolumeInflate,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (INFLATE)",
                    SemanticIntent::VolumeInflate,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (INFLATE)",
                    SemanticIntent::VolumeInflate,
                    Fidelity::Native,
                ),
            },
            // `_from` on the field's column, on this row and on the three
            // planing ones, and it is the engine's own distinction rather
            // than a spelling: `clay_item_volume_relax` relaxes a volume
            // somebody already baked, and this samples the document and
            // relaxes those samples. A baked volume reports a distance only
            // inside its band, so a facet moving further than the band comes
            // back placed against the bound — a wrong shape with `CLAY_OK`.
            // The row named the bake-then-relax pair, which is not the route.
            //
            // The hierarchy's column is the one row in this table that names
            // an entry point the application does not yet reach: every
            // hierarchy smooth goes through the stamp with `SMOOTH`, which is
            // the plain Laplacian, and the mode-taking call the row names is
            // never opened. That is #199, and it is a defect in the *code*
            // rather than in the row — the whole point of a hierarchy smooth
            // is the frequency it can pick. It is left standing, and
            // `table_truth.rs` pins the gap so the day #199 lands is the day a
            // test says so rather than a day nobody notices.
            //
            // The hierarchy's column is `Specialized` for exactly that reason,
            // and it is the same fact
            // `ToolNote::MultiresSmoothChoosesAFrequency` tells a sculptor: a
            // representation that stores the form and the detail in different
            // arrays has three smooths where a flat surface has one. The field
            // and the grid are `Native` — one smooth each, doing what the
            // label says.
            Self::Suavizar => Verbs {
                sdf: baked_field(
                    "clay_item_volume_relax_from",
                    SemanticIntent::SurfaceSmooth,
                    Fidelity::Native,
                ),
                voxel: voxel_verb(
                    "clay_voxel_sculpt_smooth",
                    SemanticIntent::SurfaceSmooth,
                    Fidelity::Native,
                ),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (SMOOTH)",
                    SemanticIntent::SurfaceSmooth,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculpt_layer_stroke_smooth",
                    SemanticIntent::SurfaceSmooth,
                    Fidelity::Specialized,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (SMOOTH)",
                    SemanticIntent::SurfaceSmooth,
                    Fidelity::Native,
                ),
            },
            // The one tool that is the same call on all four, because a
            // mask is not part of any of them: it is a world-addressed field
            // the verbs consult, and freezing a region of a mesh is the same
            // act as freezing a region of a field. A hierarchy takes it the
            // same way — the layer transform is used only to find each vertex
            // on the mask's own lattice.
            Self::Mascara => Verbs {
                sdf: mask_field("clay_mask_apply_stroke"),
                voxel: mask_field("clay_mask_apply_stroke"),
                mesh: mask_field("clay_mask_apply_stroke"),
                multires: mask_field("clay_mask_apply_stroke"),
                dynamic: mask_field("clay_mask_apply_stroke"),
            },
            // The grid's column is Padrão's, and the clamp has nowhere to
            // land: a clamped accumulation is a ceiling on how much a stroke
            // may deposit *over itself*, and a cell is set or it is not. So
            // the row names the deposit rather than repeating the field's
            // sentence about a ceiling a grid cannot have. Which makes it
            // Padrão's binding in every part, an approximation included — one
            // verb offered on the grid's shelf under two words, recorded as
            // such by
            // `no_two_tools_on_one_representation_share_an_entry_point_without_differing_parameters`
            // rather than passing unremarked as it did before.
            //
            // The field's column is `Native`: a layer of bounded height, which
            // is the clamp *and* a stamp half as deep as Padrão's. The clamp
            // alone was not a tool — with Acumular off Padrão clamps too, and
            // the two were one call to the byte (#179, #203). Where the deposit
            // *lands* is the question Padrão's caveat answers, and it is asked
            // of the tool whose whole claim is the shape of the mark.
            Self::Camada => Verbs {
                sdf: field_op(
                    "clay_layer_apply_stroke (CLAY_OP_RELIEF, clamped, half lift)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Native,
                ),
                voxel: voxel_verb(
                    "clay_voxel_set_brush",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Approximation,
                ),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (LAYER)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (LAYER)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Native,
                ),
                // Absent, and explained: see `ToolNote::DynamicHasNoLayer`.
                dynamic: None,
            },
            // Two verbs on a field, and the row names the one that runs.
            // A drag on an editable field layer is a transaction —
            // `clay_sdf_move_begin`, one `update` per pointer event, one
            // `commit` on release — which is what keeps a whole gesture to a
            // single grab. `clay_layer_move_surface_regions` is the fallback,
            // taken when no transaction could be opened, and naming only it
            // was stale for every drag a sculptor actually makes. The regions
            // variant and not the plain one: a drag has to say what it
            // invalidated, and under a mirror a caller reconstructing that box
            // gets it wrong.
            //
            // All four spelled out rather than abbreviated as
            // `clay_sdf_move_begin/update/commit`: a name that is not written
            // in full is a name nothing can look up — see [`entry_points`].
            Self::Mover => Verbs {
                sdf: field_deformer(
                    "clay_sdf_move_begin / clay_sdf_move_update / clay_sdf_move_commit \
                     (clay_layer_move_surface_regions when held)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                voxel: voxel_verb(
                    "clay_voxel_sculpt_grab",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_stamp (GRAB)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_stamp (GRAB)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (GRAB)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
            },
            // SDF only, and that is the engine's answer rather than a
            // shortcut. The verb bakes a re-sampled *volume*, which a grid has
            // no equivalent of — its cells are the volume — and a mesh's
            // geodesic Grab is a different thing wearing a similar
            // description: it walks the surface to weight a stamp, where this
            // re-samples a field with the move applied.
            //
            // `Specialized` rather than `Native`, and the distinction is the
            // one a sculptor reaches for this tool over Mover for: measuring
            // the falloff along the material is something only a
            // representation that can be re-sampled as a volume can do, and it
            // is why the row exists at all.
            Self::MoverTopologico => Verbs {
                sdf: baked_field(
                    "clay_item_volume_move_topological",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Specialized,
                ),
                voxel: None,
                mesh: None,
                // A hierarchy has no volume to bake either, and the geodesic
                // Grab it does have is `Mover`'s verb rather than this one.
                multires: None,
                dynamic: None,
            },
            // The field's column is an approximation, and the row can finally
            // say so. A snakehook on a mesh drags vertices and adds material
            // as it goes; a field has no such verb, so this grows a curve item
            // — a swept sphere chain — which reads as a tendril and is not the
            // same act at the surface. Close enough to offer and different
            // enough that a reader of this table should not take it for the
            // mesh's.
            Self::Puxar => Verbs {
                sdf: field_item(
                    "clay_item_set_curve_points (snakehook)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Approximation,
                ),
                voxel: None,
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (SNAKEHOOK)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (SNAKEHOOK)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (SNAKEHOOK)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
            },
            // Two-sided on a grid, cut-only on the other two, and the
            // difference is the engine's rather than a compromise: the voxel
            // verb fills hollows below the plane as well as taking material
            // off above it, and faking cut-only would mean reading occupancy
            // back and reapplying it — voxel math this application does not
            // do. The tooltip says which one a sculptor is holding.
            Self::Planar => Verbs {
                sdf: baked_field(
                    "clay_item_volume_flatten_from (cut-only)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
                voxel: voxel_verb(
                    "clay_voxel_sculpt_flatten (two-sided)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Specialized,
                ),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (FLATTEN)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (FLATTEN)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (FLATTEN)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
            },
            // No field column, and that is a decision rather than a gap (#203).
            // The field's only flatten is `clay_item_volume_flatten_from`, and
            // the row used to name it with `hPolish` beside it — a qualifier
            // nothing sent. Planar and Polir were the same bake with the same
            // parameters, and a sculptor measured them identical. The mesh's
            // POLISH is a flatten that also smooths the high points it leaves;
            // a field has no second pass to do that with, so it is Planar's
            // and `ToolNote::SdfPolishIsPlanar` says so.
            Self::Polir => Verbs {
                sdf: None,
                voxel: None,
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (POLISH)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (POLISH)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (POLISH)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
            },
            // No field column either (#203). A mesh's RELAX evens out where the
            // vertices sit and leaves the form alone; a field has no vertices
            // to redistribute, so the only thing the row could name is
            // Suavizar's relax — which it did, under a second word. See
            // `ToolNote::SdfRelaxIsSmooth`.
            Self::Relaxar => Verbs {
                sdf: None,
                voxel: None,
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (RELAX)",
                    SemanticIntent::SurfaceSmooth,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (RELAX)",
                    SemanticIntent::SurfaceSmooth,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (RELAX)",
                    SemanticIntent::SurfaceSmooth,
                    Fidelity::Native,
                ),
            },
            Self::Trim => Verbs {
                sdf: field_item(
                    "clay_cut_create",
                    SemanticIntent::VolumeRemove,
                    Fidelity::Native,
                ),
                voxel: None,
                mesh: None,
                multires: None,
                dynamic: None,
            },
            Self::Raspar => Verbs {
                sdf: None,
                voxel: voxel_verb(
                    "clay_voxel_sculpt_scrape",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (SCRAPE)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (SCRAPE)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (SCRAPE)",
                    SemanticIntent::SurfaceFlatten,
                    Fidelity::Native,
                ),
            },
            Self::Preencher => Verbs {
                sdf: None,
                voxel: voxel_verb(
                    "clay_voxel_sculpt_fill_cavities",
                    SemanticIntent::VolumeAdd,
                    Fidelity::Native,
                ),
                mesh: None,
                multires: None,
                dynamic: None,
            },
            // The field's column is a signed radial scale of the assembled
            // surface, taken at a negative strength: the region gathers toward
            // the dab's centre, which is what pinching is and which no
            // stroke op spells — relief and incise move the surface along its
            // own normal, and neither of those is a gather.
            //
            // The column was empty until the engine grew a resolver for it
            // (ClayCore #391): a per-item `CLAY_DEFORM_MAGNIFY` gathers one
            // piece of a smooth-unioned form and leaves the rest, with nothing
            // to show for it but a surface that came out wrong.
            //
            // The positive half of the same entry point is not on this table.
            // Inflar does not want it: relief already *is* the Inflate frame
            // (see that row), and a radial scale about a centre is a different
            // mark rather than a better one.
            Self::Pincar => Verbs {
                sdf: field_deformer(
                    "clay_layer_magnify_surface (negative strength)",
                    SemanticIntent::SurfacePinch,
                    Fidelity::Native,
                ),
                voxel: voxel_verb(
                    "clay_voxel_sculpt_pinch",
                    SemanticIntent::SurfacePinch,
                    Fidelity::Native,
                ),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (PINCH)",
                    SemanticIntent::SurfacePinch,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (PINCH)",
                    SemanticIntent::SurfacePinch,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (PINCH)",
                    SemanticIntent::SurfacePinch,
                    Fidelity::Native,
                ),
            },
            // Relief with buildup, which is what ClayBuildup *is*: the
            // engine's equivalence table maps Clay to relief along the stroke
            // plus buildup accumulation, and the difference from Padrão is the
            // accumulation and the spacing rather than another verb.
            Self::Argila => Verbs {
                sdf: field_op(
                    "clay_layer_apply_stroke (CLAY_OP_RELIEF, buildup)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Native,
                ),
                voxel: None,
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (CLAY)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (CLAY)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (CLAY)",
                    SemanticIntent::SurfaceDisplace,
                    Fidelity::Native,
                ),
            },
            // Incise, which the engine describes in the same sentence as the
            // tool: "a thin region gives the line — Crease and DamStandard".
            // Not a subtraction of spheres, which is what a Crease built out
            // of the general vocabulary would be — incise exists precisely to
            // displace the accumulated field inward without contributing a
            // primitive.
            //
            // On a grid, a narrow erosion is a pinned recipe over Inflate.
            Self::Vinco => Verbs {
                sdf: field_op(
                    "clay_layer_apply_stroke (CLAY_OP_INCISE)",
                    SemanticIntent::SurfaceCrease,
                    Fidelity::Native,
                ),
                voxel: recipe("clay_voxel_sculpt_inflate", SemanticIntent::SurfaceCrease),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (CREASE)",
                    SemanticIntent::SurfaceCrease,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (CREASE)",
                    SemanticIntent::SurfaceCrease,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (CREASE)",
                    SemanticIntent::SurfaceCrease,
                    Fidelity::Native,
                ),
            },
            // One tool, two bindings: "put colour here" is the same intent
            // whether the colour lands on a vertex or in a cell.
            //
            // Two and not three, which is the first of the places the
            // hierarchy's column is not the mesh's. A hierarchy stores where a vertex WENT — a displacement
            // read in the vertex's own transported frame — and nothing else:
            // `absorb_level_edit` is the one write path and it takes positions.
            // A paint stamp moves no vertex, so the stamp reports zero moved
            // and the write-back is skipped entirely; the colour it wrote lands
            // in the level's cache, which is rebuildable storage the engine
            // releases under pressure. The brush would appear to work and its
            // work would evaporate.
            //
            // The route that does work is the cage's: level 0's colours are
            // subdivided over their own connectivity all the way up, so paint
            // the cage as a mesh before building the hierarchy, or bake a level
            // back to a mesh and paint that. `ToolNote::MultiresStoresNoColour`
            // is where that is said to a sculptor.
            Self::Pintar => Verbs {
                sdf: None,
                voxel: voxel_verb(
                    "clay_voxel_paint_brush",
                    SemanticIntent::Paint,
                    Fidelity::Native,
                ),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (PAINT)",
                    SemanticIntent::Paint,
                    Fidelity::Native,
                ),
                multires: None,
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (PAINT)",
                    SemanticIntent::Paint,
                    Fidelity::Native,
                ),
            },
            // The one row whose two bindings are not two spellings of one
            // intent, and the exception is the representation's rather than
            // the tool's. A grid stores occupancy, so "remove what is here" is
            // clearing cells. A hierarchy stores *where a vertex went*, per
            // pass, so there is nothing to clear — what the same intent means
            // is the selected pass's detail walked toward zero, which the
            // engine's own comment calls "an eraser for THIS pass rather than
            // a flattening brush".
            //
            // The mesh column stays empty between them, and that is the
            // boundary rather than an omission: erasing a cell would change a
            // mesh's topology, and none of the sixteen fixed-topology brushes
            // may do that. The hierarchy's eraser changes no topology at all —
            // it writes a displacement channel the mesh tier does not have.
            //
            // `ToolNote::MultiresEraseTakesThisPassToZero` is where that is
            // said to a sculptor, and `Unavailable::NeedsAPass` is what they
            // are told when the form is selected rather than a pass.
            Self::Apagar => Verbs {
                sdf: None,
                voxel: voxel_verb(
                    "clay_voxel_erase_brush",
                    SemanticIntent::VolumeRemove,
                    Fidelity::Native,
                ),
                mesh: None,
                multires: multires_verb(
                    "clay_multires_sculpt_layer_stroke_erase",
                    SemanticIntent::VolumeRemove,
                    Fidelity::Specialized,
                ),
                dynamic: None,
            },
            // And the other half of the colour absence Pintar's comment
            // explains, two rows up.
            Self::Borrar => Verbs {
                sdf: None,
                voxel: None,
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (SMEAR)",
                    SemanticIntent::Paint,
                    Fidelity::Native,
                ),
                multires: None,
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (SMEAR)",
                    SemanticIntent::Paint,
                    Fidelity::Native,
                ),
            },
            Self::Nudge => Verbs {
                sdf: None,
                voxel: voxel_verb(
                    "clay_voxel_sculpt_smudge",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                mesh: mesh_verb(
                    "clay_mesh_sculptor_apply_stroke (NUDGE)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                multires: multires_verb(
                    "clay_multires_sculptor_apply_stroke (NUDGE)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
                dynamic: dynamic_verb(
                    "clay_dynamic_sculptor_apply_stroke (NUDGE)",
                    SemanticIntent::SurfaceMove,
                    Fidelity::Native,
                ),
            },
        }
    }

    /// What this tool binds to on `representation`, if it binds there at all.
    ///
    /// The lookup everything else in this file is written in terms of. A
    /// caller that wants only the engine's name asks [`ToolKind::verb_on`];
    /// one that wants to *say something about* the binding — the shelf's
    /// tooltip, the diagnostics line, the advanced help — reads the whole of
    /// it here rather than inferring it from the name, which is what the
    /// prose around the table used to be for.
    pub fn binding_on(self, representation: Representation) -> Option<Binding> {
        self.verbs().on(representation)
    }

    /// The verb this tool invokes on `representation`, if it has one there.
    pub fn verb_on(self, representation: Representation) -> Option<&'static str> {
        self.verbs().entry_point_on(representation)
    }

    /// Whether this tool exists at all on `representation`.
    ///
    /// What the shelf filters on. A tool that answers `false` is not shown for
    /// that layer, rather than shown disabled.
    pub fn exists_on(self, representation: Representation) -> bool {
        self.binding_on(representation).is_some()
    }

    /// The tools a representation can offer, in the shelf's own order.
    pub fn for_representation(representation: Representation) -> Vec<ToolKind> {
        Self::ALL
            .into_iter()
            .filter(|tool| tool.exists_on(representation))
            .collect()
    }

    /// The act this tool is, wherever it is offered.
    ///
    /// Read off the first binding it has, which is enough because every
    /// binding of one tool declares the same intent —
    /// `a_tool_means_one_thing_wherever_it_is_offered` holds that. `None` only
    /// for a tool bound nowhere, which the shelf would never show.
    pub fn intent(self) -> Option<SemanticIntent> {
        Representation::ALL
            .into_iter()
            .find_map(|representation| self.binding_on(representation))
            .map(|binding| binding.intent)
    }

    /// The tool that stands in for this one on `representation`.
    ///
    /// Itself wherever it is offered: a tool the new layer carries is kept,
    /// because the sculptor chose it and nothing about the switch unchose it.
    ///
    /// Where it is absent, the substitute is read off the capability table
    /// rather than out of a list written beside it, so a tool added to a row
    /// is a candidate the moment it is bound. In order:
    ///
    /// 1. a tool that means the **same act** here — the same
    ///    [`SemanticIntent`] — and whose binding is the representation's own
    ///    verb for it ([`Fidelity::Native`]). A grid has no topological drag,
    ///    so Mover Topológico falls to Mover; a field has no Raspar, so it
    ///    falls to Planar; a grid has no Polir, so it falls to Raspar, the
    ///    grid's own flatten, ahead of its two-sided Planar;
    /// 2. failing that, a tool meaning the same act in any fidelity — a
    ///    stand-in the table already says is close enough to offer;
    /// 3. failing both, the shelf's first tool on this representation, which
    ///    is Padrão everywhere — the plainest deposit there is, and the one a
    ///    sculptor told "your tool is not here" is least surprised to find in
    ///    hand.
    ///
    /// Two kinds of candidate are never a substitute, whatever their intent:
    ///
    /// - one driven by a **different gesture**. Trim and Apagar both take
    ///   material away, but Trim is a shape drawn on the frame and Apagar a
    ///   stroke over the surface, and a stand-in the sculptor's hand does not
    ///   already know how to use is not standing in for anything;
    /// - one that **refuses the layer until a pass is selected**. The removal
    ///   a hierarchy has is its eraser, which on the form row refuses and on a
    ///   pass row takes that pass's detail to zero — a stand-in that either
    ///   does nothing or does something else entirely.
    ///
    /// Deterministic: it depends on the tool and the representation alone,
    /// never on what the layer happens to be set to, so the same switch always
    /// lands on the same tool and the rule can be written down.
    pub fn substitute_on(self, representation: Representation) -> ToolKind {
        if self.exists_on(representation) {
            return self;
        }
        let offered = Self::for_representation(representation);
        let intent = self.intent();
        let binding = |tool: ToolKind| tool.binding_on(representation);
        let stands_in = |tool: &&ToolKind| {
            tool.is_stroke_tool() == self.is_stroke_tool()
                && !tool.needs_a_pass_on(representation)
                && binding(**tool).is_some_and(|here| Some(here.intent) == intent)
        };
        let native = |tool: &&ToolKind| {
            binding(**tool).is_some_and(|here| here.fidelity == Fidelity::Native)
        };
        offered
            .iter()
            .filter(stands_in)
            .find(native)
            .or_else(|| offered.iter().find(stands_in))
            .or_else(|| offered.first())
            .copied()
            .unwrap_or(self)
    }

    /// Whether this tool can be applied to a layer, and why not if it cannot.
    ///
    /// The absent case is still an error here, because a caller that asks
    /// about a tool the shelf never showed deserves an answer rather than a
    /// silent no-op. What the *shelf* does with it is not show the tool.
    pub fn availability(self, layer: LayerState) -> Result<(), Unavailable> {
        if !self.exists_on(layer.representation) {
            return Err(Unavailable::NoVerbHere {
                active: layer.representation,
                verbs: Box::new(self.verbs()),
                note: self.note_on(layer.representation),
            });
        }
        if !layer.editable {
            return Err(Unavailable::LayerProtected);
        }
        if !layer.visible {
            return Err(Unavailable::LayerHidden);
        }
        // A row whose geometry has not arrived. Two representations can be in
        // that state and two cannot: a field and a grid are both editable from
        // nothing, a mesh row is recorded before its triangles land, and a
        // hierarchy row is the same case one step further along — it is built
        // from a cage, so before the cage there is no level 0, and with no
        // level 0 there is no level for a stamp to bind to.
        //
        // What it says it wants differs, and that is the point of naming it
        // rather than saying "geometry": a sculptor told "this layer carries no
        // mesh" reaches for an import, and one told "this layer carries no
        // cage" reaches for the crossing that builds one.
        if !layer.carries_geometry {
            match layer.representation {
                Representation::Mesh => {
                    return Err(Unavailable::MissingAttribute { needs: "mesh" })
                }
                Representation::Multires => {
                    return Err(Unavailable::MissingAttribute { needs: "cage" })
                }
                // An adaptive row is made by reading a mesh, so it never
                // stands empty; the arm is here so that a row that somehow
                // did would say what it is missing rather than stroke nothing.
                Representation::Dynamic => {
                    return Err(Unavailable::MissingAttribute { needs: "surface" })
                }
                Representation::Sdf | Representation::Voxel => {}
            }
        }
        // Last, and after the conditions that are about the layer itself. A
        // hierarchy with the form selected is in no way broken — it is simply
        // pointed at the row this verb has nothing to say about — so a locked
        // or hidden layer must still report the lock or the hiding, which is
        // what a sculptor has to fix first anyway.
        if self.needs_a_pass_on(layer.representation) && !layer.stroke_lands_in_a_pass {
            return Err(Unavailable::NeedsAPass);
        }
        Ok(())
    }

    /// Whether this tool acts on the selected pass and so refuses the form.
    ///
    /// One pair, and it is the eraser on a hierarchy. The engine's erase walks
    /// *the target channel* toward zero, and with the form selected the target
    /// channel is the base detail — so the same gesture would take the whole
    /// form back toward the pure subdivision. That is a different operation
    /// with a different name (`restore`), it is destructive at a scale an
    /// eraser does not suggest, and nothing on the shelf would tell a sculptor
    /// which of the two they were about to get. The other verbs mean the same
    /// thing in either row and are offered in both.
    fn needs_a_pass_on(self, representation: Representation) -> bool {
        matches!(
            (self, representation),
            (Self::Apagar, Representation::Multires)
        )
    }

    /// What differs about this tool on this representation, if anything.
    ///
    /// Empty for almost every pair, and that is the point: a caveat on every
    /// row would be read by nobody. It is here for the one place where two
    /// representations of the same artist intent behave differently enough to
    /// surprise — and where faking agreement would mean doing arithmetic the
    /// engine does not offer.
    ///
    /// Answers for a pair whether or not the tool is *offered* on it. Notes
    /// for absent tools are carried into [`ToolKind::availability`]'s refusal,
    /// since a tool nobody can select is a tool nobody can hover.
    pub fn note_on(self, representation: Representation) -> Option<ToolNote> {
        match (self, representation) {
            (Self::Planar, Representation::Voxel) => Some(ToolNote::VoxelPlanarIsTwoSided),
            (Self::Suavizar, Representation::Multires) => {
                Some(ToolNote::MultiresSmoothChoosesAFrequency)
            }
            (Self::Pintar | Self::Borrar, Representation::Multires) => {
                Some(ToolNote::MultiresStoresNoColour)
            }
            (Self::Apagar, Representation::Multires) => {
                Some(ToolNote::MultiresEraseTakesThisPassToZero)
            }
            (Self::Padrao, Representation::Sdf) => Some(ToolNote::SdfStandardIsAnInflate),
            (Self::Vinco, Representation::Voxel) => Some(ToolNote::VoxelCreaseIsErodeRecipe),
            (Self::Borrar, Representation::Voxel) => Some(ToolNote::VoxelSmearHasNoColourVerb),
            (Self::Argila, Representation::Voxel) => Some(ToolNote::VoxelClayHasNoBuildup),
            (Self::Camada, Representation::Dynamic) => Some(ToolNote::DynamicHasNoLayer),
            (Self::Polir, Representation::Sdf) => Some(ToolNote::SdfPolishIsPlanar),
            (Self::Relaxar, Representation::Sdf) => Some(ToolNote::SdfRelaxIsSmooth),
            _ => None,
        }
    }

    /// Whether the tool writes vertex colour rather than moving the surface.
    ///
    /// Both refuse a mesh with no colour attribute rather than creating one:
    /// twelve bytes a vertex is a real cost to hide behind a stroke.
    pub fn writes_colour(self) -> bool {
        matches!(self, Self::Pintar | Self::Borrar)
    }

    /// Whether the tool needs a colour attribute that a layer may not carry.
    ///
    /// Only on a mesh. A grid's palette always exists, so painting a cell
    /// creates nothing that was not already there — the cost the mesh rule
    /// guards against is twelve bytes a *vertex*, which a grid does not pay.
    ///
    /// A hierarchy is deliberately not in this answer, and the omission is
    /// checked rather than assumed: the two colour brushes have no verb there
    /// at all, so a sculptor never reaches a state where the question "does
    /// this layer carry colour" is the one standing between them and the tool.
    /// `a_hierarchy_is_never_asked_for_a_colour_attribute` is what keeps the
    /// two answers from drifting apart.
    ///
    /// An adaptive surface asks it too: it carries colour exactly where the
    /// mesh it was read from did.
    pub fn needs_colour_attribute(self, representation: Representation) -> bool {
        self.writes_colour()
            && matches!(
                representation,
                Representation::Mesh | Representation::Dynamic
            )
    }

    /// Whether the tool paints a mask rather than moving the surface.
    pub fn is_mask_tool(self) -> bool {
        self == Self::Mascara
    }

    /// Whether this tool is driven by a stroke across the surface.
    ///
    /// Trim is not: its gesture is a shape drawn on the view frame, resolved
    /// into a prism that cuts through. Treating it as a surface stroke would
    /// be a tool that looks available and does something else.
    pub fn is_stroke_tool(self) -> bool {
        self != Self::Trim
    }
}

/// Which standard view is active. Mirrors the renderer's presets without the
/// ViewModel layer depending on the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewPresetKind {
    #[default]
    Perspective,
    Front,
    Side,
    Top,
}

impl ViewPresetKind {
    pub const ALL: [ViewPresetKind; 4] = [Self::Perspective, Self::Front, Self::Side, Self::Top];

    pub fn label(self) -> &'static str {
        match self {
            Self::Perspective => "Perspectiva",
            Self::Front => "Frontal",
            Self::Side => "Lateral",
            Self::Top => "Superior",
        }
    }
}

/// What a brush is set to.
///
/// Held per tool *and* per representation, so switching away and back returns
/// the settings the user left there rather than a default, and a layer of one
/// representation never starts out with a size set on another. What a slot
/// holds before anything is set is [`BrushSettings::default_for`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushSettings {
    /// Radius in **document units**, not pixels.
    ///
    /// The design's tool bar reads "Tamanho 38 px", which is a screen measure
    /// that maps through the zoom; the engine takes a world radius. Treating
    /// the number as world units directly gives a brush covering a third of a
    /// unit-sized model — and re-meshing what such a dab dirties costs several
    /// times the latency budget.
    pub size: f32,
    /// How hard each stamp bites, 0..=1.
    pub intensity: f32,
    /// How much of the stroke each stamp contributes, 0..=1.
    pub flow: f32,
    /// Shaping controls, which the design's brush panel exposes.
    pub shaping: Shaping,
    /// How the stroke varies along its own length.
    pub dynamics: Dynamics,
    /// What a drag does, which no other verb reads.
    pub drag: Drag,
    /// Whether this brush is modulated by the loaded alpha stamp.
    ///
    /// A flag rather than the samples: settings are held per tool and per
    /// representation and are copied on every read, and a stamp is megabytes.
    /// The document holds the one loaded stamp; this says whether *this* tool
    /// uses it — which is the right grain, because a sculptor wants the detail
    /// brush stamped and the blockout brush plain.
    pub alpha: bool,
    /// Whether this stroke takes material away rather than putting it there.
    ///
    /// Transient: set from the modifier held when the press landed, not stored
    /// with the tool. A brush that remembered it would come back inverted the
    /// next time it was chosen, which no reference does and nobody expects.
    pub invert: bool,
}

/// How a drag's pull falls off across its ball, by the engine's easing index.
///
/// `clay_ease` offers thirty-three curves and the C ABI gives them no names —
/// only `CLAY_EASE_LINEAR = 0` and `CLAY_EASE_COUNT = 33`. The indices here are
/// read from the engine's own `kernel/ease.h`, which is the only place the
/// order is stated.
///
/// **The easing argument runs from the centre outward, not inward**, and that
/// is what decides which name goes on which index. `cregion_weight` is
/// `cease(ease_type, 1 - d / radius)` — the argument is **1 at the centre and 0
/// at the rim** — so a curve that sits *below* linear in `t` sits below it near
/// the centre and concentrates the pull there. `ease_in_quad` gives
/// `(1 - d/r)²`, which is 0.25 where linear is 0.5: that is the **tight** one,
/// not the broad one. Naming these from the shape of `E(t)` without reading
/// what `t` is gets them exactly backwards.
///
/// A curated four rather than all thirty-three. The rest are the same shapes at
/// different exponents, and two families are actively unsafe to offer:
///
/// - **`back` and `elastic` go negative inside the ball**, which pushes
///   material the opposite way part of the way out.
/// - **`circ` carries a declared slope of about 70** against 1.0 for linear and
///   2.0 for the quads, because `E'(t) = t/sqrt(1 - t²)` is unbounded at the
///   endpoint — `ease_max_slope` in the engine's `bounds.cpp` evaluates it at a
///   guard rather than in closed form. That slope multiplies a grab's per-link
///   Lipschitz factor, and a chain multiplies those in turn. Offering it would
///   hand a sculptor a falloff that degrades a layer tens of times faster than
///   the one it replaced — the opposite of what [`Drag::front_only`]'s default
///   was changed to achieve, and over a far larger number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragFalloff {
    /// `ease_linear`. The weight falls off in proportion to distance.
    Linear,
    /// `ease_smoothstep`. Flat at the centre and at the rim, which is the
    /// falloff most sculpting applications use and what a hand expects.
    Smooth,
    /// `ease_out_quad` — **above** linear across the ball, so more of it
    /// travels together. Closer to moving a region than to pulling a point.
    Broad,
    /// `ease_in_quad` — **below** linear across the ball, so the pull
    /// concentrates near the centre and the rim barely moves.
    Tight,
}

impl DragFalloff {
    pub const ALL: [DragFalloff; 4] = [Self::Linear, Self::Smooth, Self::Broad, Self::Tight];

    /// The engine's easing index, from `clay/kernel/ease.h`.
    ///
    /// Paired with the declared slope `ease_max_slope` gives each, because that
    /// is what a chain of grabs multiplies: linear 1.0, smoothstep 1.5, and 2.0
    /// for both quads. Every curve offered here is within a factor of two of
    /// linear, which is the property that makes the set safe to expose.
    pub fn ease(self) -> i32 {
        match self {
            Self::Linear => 0,
            Self::Smooth => 1,
            Self::Broad => 4,
            Self::Tight => 3,
        }
    }
}

/// What a drag does, beyond its radius.
///
/// Held apart from [`Dynamics`] because the two are opposites: `Dynamics` is
/// everything a drag deliberately does *not* read, and this is the pair only a
/// drag reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drag {
    pub falloff: DragFalloff,
    /// Whether only the near side of a form travels.
    ///
    /// **Blender's "Front Faces Only" defaults off**, and so does this. With it
    /// on, a sculptor pulling a limb moves the surface facing them and leaves
    /// the far side behind, which is what a thin form needs and what a solid
    /// one does not. This application had it hardcoded on, so a form could
    /// never be dragged through.
    ///
    /// **And the gate is what degrades a layer, not the drag.** Eight dabs on
    /// a sphere leave a chain of 8 either way, and `safe_step_scale` reads
    /// **0.2421 with the gate on against 0.5999 with it off** — 2.5x less
    /// degraded for the same number of warps, which is enough to move the
    /// layer's own health report from `Deformers` to `None`. A front-only grab
    /// gates on the surface normal, so its weight field has a discontinuity in
    /// it and the declared Lipschitz bound has to cover the jump; a two-sided
    /// grab is smooth and does not.
    ///
    /// That is why the default is off rather than merely because Blender's is:
    /// the parity argument says a sculptor expects it, and the measurement
    /// says the other setting is the one that costs.
    pub front_only: bool,
}

impl Default for Drag {
    fn default() -> Self {
        Self {
            // Linear is what every drag in this application used, hardcoded,
            // before the control existed. Kept as the default so that turning
            // the control on changes nothing until a sculptor moves it.
            falloff: DragFalloff::Linear,
            front_only: false,
        }
    }
}

/// How a stroke varies along its own length.
///
/// [`Shaping`] is about one stamp's footprint. This is about how the footprint
/// changes between the press and the release: with how hard the sculptor is
/// pressing, and with how far along the stroke has travelled. Every field maps
/// to a `clay_stroke_preset` field the engine already resolves and
/// `clay_layer_apply_stroke` already consumes.
///
/// **Not for Move.** A drag anchors its region at the press and carries it by
/// the motion that follows, so a radius that changes mid-gesture is a
/// different region rather than a different brush — and on the engine's side a
/// pressure-driven radius currently costs 101x, because successive grabs
/// coalesce on bit-exact identity of centre and radius and a moving radius
/// matches nothing. The drag paths do not read this.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dynamics {
    /// How far pressure drives the radius, 0..=1. Zero disconnects it.
    pub pressure_size: f32,
    /// How far pressure drives the strength, 0..=1. Zero disconnects it.
    pub pressure_strength: f32,
    /// Exponent applied to pressure before either of the two above.
    ///
    /// 1 is linear. Above 1 the light end of the range gets finer, which is
    /// what a sculptor means by asking for more control over a soft touch;
    /// below 1 the brush reaches full strength sooner.
    pub pressure_curve: f32,
    /// Fraction of the stroke the radius ramps in over, 0..=1.
    pub taper_start: f32,
    /// Fraction of the stroke the radius ramps out over, 0..=1.
    pub taper_end: f32,
    /// Whether each stamp turns to follow the stroke's direction.
    ///
    /// The engine's `rotate_along_stroke`. Observable only where the stamp has
    /// something to orient — a round footprint looks the same at every angle —
    /// so it reads as inert until an alpha is loaded, the same caveat
    /// [`Shaping::azimuth`] carries.
    pub rake: bool,
}

impl Default for Dynamics {
    fn default() -> Self {
        // THE ENGINE'S OWN DEFAULTS, not zero.
        //
        // These fields were always being sent — by `StrokePreset::default()`,
        // which asks `clay_stroke_preset_defaults` for them. Plumbing them
        // means the host now decides their value, and a host that decided
        // "all off" would be changing every brush in the application while
        // claiming to add a control.
        //
        // `pressure_strength` is the one that matters: the engine defaults it
        // to **1**, so pressure has driven strength since before this control
        // existed. Defaulting it to 0 here disconnected pen pressure from
        // every stroke, silently, and the only thing that caught it was a
        // latency test on another machine.
        //
        // `pressure_size` at 0 and the tapers at 0 are the engine's values
        // too, and are kept by agreeing with it rather than by coincidence:
        // `the_defaults_are_the_engines_defaults` fails if either side moves.
        Self {
            pressure_size: 0.0,
            pressure_strength: 1.0,
            pressure_curve: 1.0,
            taper_start: 0.0,
            taper_end: 0.0,
            rake: false,
        }
    }
}

impl Dynamics {
    /// Clamped to what the engine accepts.
    pub fn sanitized(self) -> Self {
        Self {
            pressure_size: self.pressure_size.clamp(0.0, 1.0),
            pressure_strength: self.pressure_strength.clamp(0.0, 1.0),
            // A zero or negative exponent is not a curve; the engine's own
            // default is 1 and that is what an out-of-range value becomes.
            pressure_curve: if self.pressure_curve.is_finite() && self.pressure_curve > 0.0 {
                self.pressure_curve.clamp(0.1, 4.0)
            } else {
                1.0
            },
            taper_start: self.taper_start.clamp(0.0, 1.0),
            taper_end: self.taper_end.clamp(0.0, 1.0),
            rake: self.rake,
        }
    }
}

/// How a stamp is shaped, beyond its size and strength.
///
/// Every field maps to a stroke-preset or brush-parameter field the engine
/// already has. Nothing here is invented: a control with no engine counterpart
/// would be a promise the tool cannot keep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shaping {
    /// Positional jitter as a fraction of the radius — the design's "Ruído".
    pub noise: f32,
    /// How coverage falls off toward the footprint's edge — "Borda".
    pub falloff: Falloff,
    /// Whether overlapping stamps deposit twice — "Acumular".
    pub accumulate: bool,
    /// Lazy-mouse lag: 0 follows the pointer exactly — "Suavização".
    pub smoothing: f32,
    /// Mirror each stamp about the stroke — "Espelhamento".
    pub mirror: bool,
    /// How far each stamp is turned about its own facing, in RADIANS — the
    /// grain, "Grão".
    ///
    /// Maps to the engine's `stamp_azimuth`, which turns a stamp's in-plane
    /// axes about the direction it faces. It is what makes a rake, a chisel,
    /// clay strips and a turned alpha one number rather than four brushes.
    ///
    /// **Observable only where the stamp has something to orient.** A round
    /// footprint looks the same at every angle by construction, so this reads
    /// as inert until a stamp is loaded. It belongs with the rest of what
    /// shapes a stroke rather than with its size and strength, which is where
    /// a sculptor changes it — occasionally, and not mid-line.
    ///
    /// Zero is *no rotation at all* rather than a rotation by zero, and the
    /// engine branches on that, so zero is the default and what every brush
    /// that has never been turned keeps sending.
    pub azimuth: f32,
}

impl Default for Shaping {
    fn default() -> Self {
        // The design's brush panel shows Ruído at 15%. It starts at zero
        // instead: the engine's brick cache does not reproduce a stroke
        // jittered that far, so a default brush with it on sculpts a document
        // that never appears in the viewport. See `ClayDocument::preset`.
        Self {
            noise: 0.0,
            falloff: Falloff::Smooth,
            accumulate: true,
            smoothing: 0.25,
            mirror: false,
            azimuth: 0.0,
        }
    }
}

/// How coverage falls off toward a footprint's edge.
///
/// Mirrors the engine's set without the domain naming the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Falloff {
    /// Hard-edged, the usual brush.
    Constant,
    Linear,
    #[default]
    Smooth,
    Gaussian,
}

impl Falloff {
    pub const ALL: [Falloff; 4] = [Self::Constant, Self::Linear, Self::Smooth, Self::Gaussian];

    pub fn label(self) -> &'static str {
        match self {
            Self::Constant => "Dura",
            Self::Linear => "Linear",
            Self::Smooth => "Suave",
            Self::Gaussian => "Gaussiana",
        }
    }
}

impl Default for BrushSettings {
    fn default() -> Self {
        // Intensity and flow are the design's. The radius is not: it is the
        // smallest brush the viewport can actually show, with headroom.
        //
        // 0.08 was tried, reading the design's "Tamanho 38 px" as a detail
        // brush on a unit-scale model. It displaces about half of the brick
        // cache's 0.02 voxel, which marching cubes rounds away — so a click
        // changed the document and left the rendered mesh bit-identical
        // everywhere except the pole, where the grid happens to align.
        // Measured, the floor is 0.10; this sits well clear of it.
        Self {
            size: 0.18,
            intensity: 0.65,
            flow: 0.80,
            alpha: false,
            invert: false,
            shaping: Shaping::default(),
            dynamics: Dynamics::default(),
            drag: Drag::default(),
        }
    }
}

/// An angle brought inside one turn, and never a NaN.
///
/// `rem_euclid` answers NaN for a NaN and for an infinity, and the engine
/// builds a rotation basis out of this — so the one value that cannot be
/// allowed through is the one an unchecked division would produce.
fn turn_of(radians: f32) -> f32 {
    if !radians.is_finite() {
        return 0.0;
    }
    radians.rem_euclid(std::f32::consts::TAU)
}

impl BrushSettings {
    /// What a tool starts out set to on a layer of `representation`, before
    /// the sculptor has set anything there.
    ///
    /// The value a brush has on a representation it has never been used on,
    /// and the reason it is stated per representation rather than carried
    /// over: the one thing a carried-over value is guaranteed to be is right
    /// for somewhere else. That is how a grid layer came to be stroked at a
    /// field's size of 100, a dab a metre across (#162, #217).
    ///
    /// The four agree today, and each arm says why the number suits it rather
    /// than leaving the agreement to be assumed. A size is a radius in
    /// document units on every representation — the grid's footprint is
    /// converted from it, so the same number reaches as far everywhere — and
    /// what differs is where each representation stops being able to show it:
    pub fn default_for(representation: Representation) -> Self {
        match representation {
            // Above the floor a field can show at all. 0.10 is the measured
            // smallest radius whose dab survives marching cubes on the brick
            // cache's 0.02 voxel; 0.18 sits well clear of it. See `default`.
            Representation::Sdf => Self::default(),
            // Nine cells of radius on the default 0.02 grid, so a footprint of
            // nineteen across — odd, as every span is, and a third of the way
            // to the 63-cell ceiling past which a grid dab stops growing. A
            // size that left room above it for the sculptor to go bigger.
            Representation::Voxel => Self::default(),
            // A mesh, a hierarchy and an adaptive surface stamp over the
            // vertices in reach, with no floor of their own. They take the
            // field's size so that a crossing from a field lands with a brush
            // of the reach the sculptor had.
            Representation::Mesh | Representation::Multires | Representation::Dynamic => {
                Self::default()
            }
        }
    }

    /// Clamps to the ranges the engine accepts.
    ///
    /// A zero or negative radius is rejected by the engine, so it is clamped
    /// here rather than turned into an error the user cannot act on.
    pub fn sanitized(self) -> Self {
        Self {
            size: self.size.clamp(0.001, 100.0),
            intensity: self.intensity.clamp(0.0, 1.0),
            flow: self.flow.clamp(0.01, 1.0),
            shaping: Shaping {
                noise: self.shaping.noise.clamp(0.0, 1.0),
                smoothing: self.shaping.smoothing.clamp(0.0, 0.95),
                // A whole turn is the same grain as none, so the angle is
                // brought back inside one rather than clamped at the ends —
                // a clamp would make a dial run out of travel where an angle
                // has none to run out of.
                azimuth: turn_of(self.shaping.azimuth),
                ..self.shaping
            },
            dynamics: self.dynamics.sanitized(),
            drag: self.drag,
            alpha: self.alpha,
            invert: self.invert,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_falloff_has_a_distinct_label() {
        for (i, a) in Falloff::ALL.iter().enumerate() {
            for b in Falloff::ALL.iter().skip(i + 1) {
                assert_ne!(a.label(), b.label());
            }
        }
    }

    /// The `match` is the point: adding a variant to [`LayerOperation`] makes
    /// it non-exhaustive, and the compiler names the operation that has no
    /// arguments to be measured with. A list without this is a list that goes
    /// stale silently, which for the performance gate means an operation
    /// nobody is timing.
    #[test]
    fn every_operation_is_in_all() {
        let all = LayerOperation::all();
        for operation in all {
            match operation {
                LayerOperation::Taper { .. }
                | LayerOperation::Twist { .. }
                | LayerOperation::LatticeDrag { .. }
                | LayerOperation::CloseHoles { .. }
                | LayerOperation::FillVoids
                | LayerOperation::RefineRegion { .. } => {}
            }
        }
        let labels: std::collections::BTreeSet<&str> =
            all.iter().map(|operation| operation.label()).collect();
        assert_eq!(
            labels.len(),
            all.len(),
            "two entries in LayerOperation::all are the same operation"
        );
    }

    #[test]
    fn every_operation_in_all_applies_somewhere() {
        for operation in LayerOperation::all() {
            assert!(
                Representation::ALL
                    .into_iter()
                    .any(|representation| operation.applies_to(representation)),
                "{} applies to no representation, so nothing can measure it",
                operation.label()
            );
        }
    }

    /// Every row names at least one thing that *could* be an entry point.
    ///
    /// This is as far as the domain can go on its own and the limit is worth
    /// stating, because the assertion that used to be here read
    /// `verb.starts_with("clay_")` and was taken for the real check: it passes
    /// for a renamed entry point, for a withdrawn one, and for a verb the
    /// dispatch does not use. `clayspace-model` links no engine, so a name
    /// here is text and nothing in this crate can ask whether the symbol
    /// exists.
    ///
    /// The two checks that matter live where both halves are in scope, in
    /// `clayspace-engine/tests/table_truth.rs`: the name is a symbol the
    /// pinned bindings declare, and the name is the entry point a stroke on
    /// that pair actually reaches.
    #[test]
    fn every_row_of_the_table_names_at_least_one_entry_point() {
        for tool in ToolKind::ALL {
            for representation in Representation::ALL {
                let Some(verb) = tool.verb_on(representation) else {
                    continue;
                };
                assert!(
                    !entry_points(verb).is_empty(),
                    "{} on {} names no engine entry point at all: {verb}",
                    tool.label(),
                    representation.label()
                );
            }
        }
        for operation in LayerOperation::all() {
            for representation in Representation::ALL {
                let Some(verb) = operation.verbs().entry_point_on(representation) else {
                    continue;
                };
                assert!(
                    !entry_points(verb).is_empty(),
                    "{} on {} names no engine entry point at all: {verb}",
                    operation.label(),
                    representation.label()
                );
            }
        }
    }

    /// What the reader picks out of a row, on the three shapes a row takes.
    ///
    /// Worth its own test rather than being trusted, because everything
    /// downstream is only as good as this: a name this misses is a name
    /// nothing checks, and it would go missing *silently*.
    #[test]
    fn a_row_is_read_for_the_calls_it_names_and_nothing_else() {
        assert_eq!(
            entry_points("clay_mesh_sculptor_apply_stroke (DRAW)"),
            vec!["clay_mesh_sculptor_apply_stroke"],
            "a brush kind in brackets is not a call"
        );
        assert_eq!(
            entry_points("clay_layer_apply_stroke (CLAY_OP_RELIEF)"),
            vec!["clay_layer_apply_stroke"],
            "the engine's constants are spelled in capitals and are not calls"
        );
        assert_eq!(
            entry_points("clay_item_volume_flatten_from (cut-only, hPolish)"),
            vec!["clay_item_volume_flatten_from"],
            "a qualifier in prose names nothing"
        );
        assert_eq!(
            entry_points(
                "clay_sdf_move_begin / clay_sdf_move_update / clay_sdf_move_commit \
                 (clay_layer_move_surface_regions when held)"
            ),
            vec![
                "clay_sdf_move_begin",
                "clay_sdf_move_update",
                "clay_sdf_move_commit",
                "clay_layer_move_surface_regions",
            ],
            "a row that names four calls has to yield four"
        );
    }

    /// The diagnostics line lists each call once, however many columns name it.
    ///
    /// Máscara is the case: one call on all five representations, and a report
    /// that said so five times would read as five bindings.
    #[test]
    fn the_diagnostics_line_names_each_call_once() {
        assert_eq!(ToolKind::Mascara.engine_verbs(), "clay_mask_apply_stroke");
        let padrao = ToolKind::Padrao.engine_verbs();
        assert_eq!(
            padrao.matches("clay_").count(),
            5,
            "Padrão reaches five representations by five different calls: {padrao}"
        );
    }

    /// The list the check above this crate reads is the whole table.
    #[test]
    fn the_engine_entry_points_are_gathered_from_both_tables() {
        let named = every_entry_point();
        assert!(
            named.contains("clay_mask_apply_stroke"),
            "the mask's call is in the tools' table"
        );
        assert!(
            named.contains("clay_voxel_repair_fill_voids"),
            "the operations' table is gathered too, not only the tools'"
        );
        assert!(
            named.iter().all(|name| name.starts_with("clay_")),
            "something that is not an entry point was gathered: {named:?}"
        );
    }

    // -- the typed columns ---------------------------------------------------

    /// Every binding in both tables carries all four of its parts.
    ///
    /// Three of the four cannot be left out — the type will not build without
    /// them — and that is most of this change's point. What is left for a test
    /// is the part the type cannot hold: the name has to be a name, and the
    /// two ways a row can say "this is composed" have to say the same thing.
    /// A binding filed under [`ExecutionFamily::Recipe`] while claiming a
    /// fidelity of `Native` would read as a single verb everywhere but in the
    /// one place that matters.
    #[test]
    fn every_binding_has_metadata() {
        for (what, verbs) in every_row() {
            for representation in Representation::ALL {
                let Some(binding) = verbs.on(representation) else {
                    continue;
                };
                assert!(
                    !entry_points(binding.entry_point).is_empty(),
                    "{what} on {} names no engine entry point at all: {}",
                    representation.label(),
                    binding.entry_point
                );
                assert_eq!(
                    binding.family == ExecutionFamily::Recipe,
                    binding.fidelity == Fidelity::Recipe,
                    "{what} on {} is a recipe by one of its two columns and \
                     not by the other",
                    representation.label()
                );
            }
        }
    }

    /// A row is filed under the family whose calls it actually names.
    ///
    /// The mistake this catches is the quiet one: a hierarchy verb typed into
    /// the mesh column reads perfectly, passes every count, and tells every
    /// reader after it the wrong thing about which runtime the call belongs
    /// to. The engine spells five of the families as a prefix, so for those
    /// five the claim is checkable against the name itself.
    #[test]
    fn every_binding_is_filed_under_the_family_it_calls() {
        for (what, verbs) in every_row() {
            for representation in Representation::ALL {
                let Some(binding) = verbs.on(representation) else {
                    continue;
                };
                let Some(prefix) = binding.family.prefix() else {
                    continue;
                };
                for name in entry_points(binding.entry_point) {
                    assert!(
                        name.starts_with(prefix),
                        "{what} on {} is filed as a {} and calls {name}, which \
                         is not a {prefix}* call",
                        representation.label(),
                        binding.family.label()
                    );
                }
            }
        }
    }

    /// A tool is one act, whichever representation it lands on.
    ///
    /// This is the claim the shelf makes by showing one button with one
    /// tooltip for up to four calls, and until the intent was in the value
    /// nothing held it. What it forbids is a column quietly borrowed from a
    /// neighbouring verb because the name looked close enough — which is how
    /// the grid's Padrão came to name the inflate, reading the *shape* of the
    /// tool off its field counterpart rather than off the call.
    ///
    /// How the four differ is [`Fidelity`]'s question, and they do differ:
    /// saying they are one act is not saying they are one implementation.
    #[test]
    fn a_tool_means_one_thing_wherever_it_is_offered() {
        for tool in ToolKind::ALL {
            let mut claimed: Option<(SemanticIntent, Representation)> = None;
            for representation in Representation::ALL {
                let Some(binding) = tool.binding_on(representation) else {
                    continue;
                };
                match claimed {
                    None => claimed = Some((binding.intent, representation)),
                    Some((intent, first)) => assert_eq!(
                        binding.intent,
                        intent,
                        "{} means {} on {} and {} on {}; one button cannot \
                         carry two acts",
                        tool.label(),
                        binding.intent.label(),
                        representation.label(),
                        intent.label(),
                        first.label()
                    ),
                }
            }
        }
    }

    /// A switch never leaves a tool in hand that the layer cannot run, and a
    /// tool the layer can run is never swapped.
    #[test]
    fn every_substitute_is_a_tool_the_layer_carries() {
        for tool in ToolKind::ALL {
            for representation in Representation::ALL {
                let substitute = tool.substitute_on(representation);
                assert!(
                    substitute.exists_on(representation),
                    "{} on {} fell to {}, which is not on that shelf either",
                    tool.label(),
                    representation.label(),
                    substitute.label()
                );
                if tool.exists_on(representation) {
                    assert_eq!(
                        substitute,
                        tool,
                        "{} was swapped out of a layer that has it",
                        tool.label()
                    );
                }
                assert!(
                    !substitute.needs_a_pass_on(representation) || substitute == tool,
                    "{} on {} fell to {}, which refuses the form row",
                    tool.label(),
                    representation.label(),
                    substitute.label()
                );
            }
        }
    }

    /// The substitute is read off the table: the same act where the layer has
    /// one, and the plainest deposit where it has none.
    ///
    /// The rows named here are the ones a sculptor meets; a table edit that
    /// moves one of them fails here, and the documentation that lists them is
    /// what has to follow.
    #[test]
    fn a_substitute_means_the_same_act_where_the_layer_has_one() {
        use Representation::*;
        let cases = [
            (ToolKind::Raspar, Sdf, ToolKind::Planar),
            (ToolKind::MoverTopologico, Voxel, ToolKind::Mover),
            (ToolKind::Puxar, Voxel, ToolKind::Mover),
            (ToolKind::Relaxar, Voxel, ToolKind::Suavizar),
            (ToolKind::Polir, Voxel, ToolKind::Raspar),
            // A field's single flatten and single smooth, which is what the
            // two notes on those absences send a sculptor to.
            (ToolKind::Polir, Sdf, ToolKind::Planar),
            (ToolKind::Relaxar, Sdf, ToolKind::Suavizar),
            (ToolKind::Nudge, Sdf, ToolKind::Mover),
            (ToolKind::Borrar, Voxel, ToolKind::Pintar),
            // Both remove material, but one is a frame gesture and the other a
            // stroke, so neither stands in for the other.
            (ToolKind::Trim, Voxel, ToolKind::Padrao),
            (ToolKind::Apagar, Sdf, ToolKind::Padrao),
            // A mesh removes nothing, since its topology does not change.
            (ToolKind::Apagar, Mesh, ToolKind::Padrao),
            (ToolKind::Preencher, Sdf, ToolKind::Padrao),
            (ToolKind::Pintar, Sdf, ToolKind::Padrao),
        ];
        for (tool, representation, expected) in cases {
            assert_eq!(
                tool.substitute_on(representation),
                expected,
                "{} on {}",
                tool.label(),
                representation.label()
            );
        }
        for tool in ToolKind::ALL {
            for representation in Representation::ALL {
                let substitute = tool.substitute_on(representation);
                let same_act = ToolKind::for_representation(representation)
                    .into_iter()
                    .any(|other| {
                        other.intent() == tool.intent()
                            && other.is_stroke_tool() == tool.is_stroke_tool()
                            && !other.needs_a_pass_on(representation)
                    });
                if same_act {
                    assert_eq!(
                        substitute.intent(),
                        tool.intent(),
                        "{} on {} fell to {} although the layer has a tool for the same act",
                        tool.label(),
                        representation.label(),
                        substitute.label()
                    );
                }
            }
        }
    }

    /// Every representation's first tool is the one the fallback lands on,
    /// and it is the same one everywhere — so "no tool for that act here"
    /// always means the same thing to a sculptor.
    #[test]
    fn the_fallback_is_the_same_tool_on_every_representation() {
        for representation in Representation::ALL {
            assert_eq!(
                ToolKind::for_representation(representation).first(),
                Some(&ToolKind::Padrao),
                "{}",
                representation.label()
            );
        }
    }

    /// A default a representation cannot show is a first dab that does nothing
    /// visible, and one past its ceiling is a dab that does not grow with the
    /// control. Each representation's default sits inside both.
    #[test]
    fn every_default_brush_fits_its_representation() {
        const FIELD_FLOOR: f32 = 0.10;
        // 31 cells of radius on the default 0.02 grid: the 63-cell ceiling.
        const GRID_CEILING: f32 = 0.62;
        for representation in Representation::ALL {
            let brush = BrushSettings::default_for(representation);
            assert_eq!(brush, brush.sanitized(), "{}", representation.label());
            assert!(
                brush.size > FIELD_FLOOR && brush.size < GRID_CEILING,
                "{} starts at {}",
                representation.label(),
                brush.size
            );
            assert!(!brush.invert, "a brush does not start out inverted");
        }
    }

    /// Two tools on one representation are two tools.
    ///
    /// The defect this exists for: Padrão and Inflar on a field name the same
    /// call with the same operation, and the shelf offers both. That is
    /// *correct* — relief is the field's Inflate, and Padrão is the
    /// approximation of a Standard the engine does not ship — but under a
    /// table of bare strings it was indistinguishable from a row copied by
    /// mistake. With the intent and the fidelity in the value, the two rows
    /// differ where they should: same call, different claim.
    ///
    /// So what is forbidden is a pair whose bindings are *identical in every
    /// part*, because then there is nothing left that could make them two
    /// tools. Where that is nonetheless the truth it is named here, with the
    /// reason — the list is short, it is meant to shrink, and each entry is a
    /// tool the shelf offers twice under different words.
    #[test]
    fn no_two_tools_on_one_representation_share_an_entry_point_without_differing_parameters() {
        let mut excused = 0;
        for representation in Representation::ALL {
            for (i, a) in ToolKind::ALL.iter().enumerate() {
                for b in ToolKind::ALL.iter().skip(i + 1) {
                    let (Some(first), Some(second)) =
                        (a.binding_on(representation), b.binding_on(representation))
                    else {
                        continue;
                    };
                    if first != second {
                        if shared_on_purpose(*a, *b, representation).is_some() {
                            panic!(
                                "{} and {} on {} are recorded as one binding \
                                 and are no longer one. The list is meant to \
                                 shrink: take the entry out.",
                                a.label(),
                                b.label(),
                                representation.label()
                            );
                        }
                        continue;
                    }
                    excused += 1;
                    assert!(
                        shared_on_purpose(*a, *b, representation).is_some(),
                        "{} and {} are the same binding on {} — {} — so the \
                         shelf offers one verb under two words. Either they \
                         differ in something the row does not say, in which \
                         case say it, or one of them does not belong on that \
                         shelf.",
                        a.label(),
                        b.label(),
                        representation.label(),
                        first.describe()
                    );
                }
            }
        }
        assert_eq!(
            excused, 1,
            "the number of shelf entries that are one verb under two words \
             has moved; the list below says which one it is"
        );
    }

    /// The pairs that really are one binding, and why each one still stands.
    ///
    /// One left. The field's Suavizar/Relaxar pair was the other, and #203
    /// took Relaxar off the field's shelf with a note rather than keep a
    /// second word for one verb.
    fn shared_on_purpose(
        a: ToolKind,
        b: ToolKind,
        representation: Representation,
    ) -> Option<&'static str> {
        match (a, b, representation) {
            // Occupancy is binary, so the ceiling Camada exists to impose has
            // nowhere to land: both rows are the deposit. The pair differs on
            // every other representation and only here collapses.
            (ToolKind::Padrao, ToolKind::Camada, Representation::Voxel) => {
                Some("a grid cannot hold a clamped accumulation; both deposit")
            }
            _ => None,
        }
    }

    /// A caveat is the sentence for a row that is not the plain reading.
    ///
    /// The two halves of the same fact, written twice until now: a
    /// [`ToolNote`] told a sculptor that something here is not what the label
    /// suggests, and the row it hung off said nothing at all. Holding them
    /// together means a note cannot be attached to a row that does exactly
    /// what its label says — which would be a sentence about nothing — and a
    /// row whose fidelity is corrected to `Native` cannot keep its caveat.
    ///
    /// Only this direction. The converse is false on purpose: a grid's Padrão
    /// is an approximation and carries no note, because "a deposit, occupancy
    /// being binary" is what a grid *is* and a sculptor working one is not
    /// surprised by it. A note is for the surprise, not for every departure.
    #[test]
    fn a_note_never_hangs_off_a_row_that_does_what_its_label_says() {
        for tool in ToolKind::ALL {
            for representation in Representation::ALL {
                let Some(_) = tool.note_on(representation) else {
                    continue;
                };
                // The one note on an absence — a hierarchy stores no colour —
                // has no binding to speak for, and its whole point is that
                // there is none.
                let Some(binding) = tool.binding_on(representation) else {
                    continue;
                };
                assert!(
                    !binding.fidelity.is_the_plain_reading(),
                    "{} on {} carries a caveat and binds {}, which claims to \
                     do exactly what the label says. One of the two is wrong.",
                    tool.label(),
                    representation.label(),
                    binding.describe()
                );
            }
        }
    }

    /// Relief is the field's Inflate, and the field's Standard is the
    /// approximation — which is the way round the engine measured it.
    ///
    /// ClayCore v0.120.0 (#615, #618) put a frame-isolated inflate reference
    /// at 0.000 of the amplitude from relief on a sphere, a saddle and a bowl,
    /// and the draw reference at 0.017, 0.077 and 0.027. The two rows name the
    /// same call, so the only place that ordering can be written down is the
    /// fidelity — and [`ToolNote::SdfStandardIsAnInflate`] is the sentence
    /// that says it to a sculptor. This is what keeps the three from drifting
    /// apart the next time somebody tidies one of them.
    #[test]
    fn the_fields_standard_is_the_approximation_and_its_relief_is_the_inflate() {
        let standard = ToolKind::Padrao
            .binding_on(Representation::Sdf)
            .expect("Padrão reaches a field");
        let inflate = ToolKind::Inflar
            .binding_on(Representation::Sdf)
            .expect("Inflar reaches a field");

        assert_eq!(
            standard.entry_point, inflate.entry_point,
            "the two rows are relief, which is why the fidelity is the only \
             thing left that can order them"
        );
        assert_eq!(standard.intent, SemanticIntent::SurfaceDisplace);
        assert_eq!(standard.fidelity, Fidelity::Approximation);
        assert_eq!(inflate.intent, SemanticIntent::VolumeInflate);
        assert_eq!(
            inflate.fidelity,
            Fidelity::Native,
            "relief is the field's Inflate rather than a stand-in for one"
        );
        assert_eq!(
            ToolKind::Padrao.note_on(Representation::Sdf),
            Some(ToolNote::SdfStandardIsAnInflate),
            "the sculptor-facing half of the same fact"
        );
        assert_eq!(
            ToolKind::Inflar.note_on(Representation::Sdf),
            None,
            "the faithful row is the one with nothing to warn about"
        );
    }

    /// A composed tool is describable rather than absent.
    ///
    /// Voxel Crease uses a pinned erosion preset of Inflate. Its binding must
    /// identify the existing engine entry point and the recipe fidelity.
    #[test]
    fn a_recipe_is_expressible_and_is_marked_as_one() {
        let crease = ToolKind::Vinco
            .binding_on(Representation::Voxel)
            .expect("Crease is offered on a grid");
        assert!(crease.is_a_recipe());
        assert_eq!(crease.entry_point, "clay_voxel_sculpt_inflate");
        assert_eq!(crease.intent, SemanticIntent::SurfaceCrease);
        assert_eq!(
            ToolKind::Vinco.note_on(Representation::Voxel),
            Some(ToolNote::VoxelCreaseIsErodeRecipe)
        );
        assert_eq!(
            ToolKind::Borrar.note_on(Representation::Voxel),
            Some(ToolNote::VoxelSmearHasNoColourVerb)
        );
        assert_eq!(
            ToolKind::Argila.note_on(Representation::Voxel),
            Some(ToolNote::VoxelClayHasNoBuildup)
        );
        assert!(!ToolKind::Borrar.exists_on(Representation::Voxel));
        assert!(!ToolKind::Argila.exists_on(Representation::Voxel));
        let composed = recipe(
            "clay_layer_apply_stroke (CLAY_OP_INCISE) / clay_layer_apply_stroke \
             (CLAY_OP_RELIEF)",
            SemanticIntent::SurfaceCrease,
        )
        .expect("a recipe is a binding like any other");

        assert!(composed.is_a_recipe());
        assert_eq!(composed.fidelity, Fidelity::Recipe);
        assert_eq!(
            entry_points(composed.entry_point).len(),
            1,
            "a recipe's steps are read back out of it exactly as any row's \
             are, deduplicated the same way"
        );
        // And the table it would go into still answers for it: a `Verbs` with
        // a recipe in one column reaches that representation like any other.
        let verbs = Verbs {
            sdf: None,
            voxel: recipe("clay_voxel_set_brush", SemanticIntent::SurfaceCrease),
            mesh: None,
            multires: None,
            dynamic: None,
        };
        assert_eq!(verbs.count(), 1);
        assert!(verbs
            .on(Representation::Voxel)
            .is_some_and(Binding::is_a_recipe));
    }

    /// Both tables, so a check written for one is not quietly written for half
    /// of what the application binds.
    fn every_row() -> Vec<(String, Verbs)> {
        let mut rows: Vec<(String, Verbs)> = ToolKind::ALL
            .into_iter()
            .map(|tool| (tool.label().to_string(), tool.verbs()))
            .collect();
        rows.extend(
            LayerOperation::all()
                .into_iter()
                .map(|operation| (operation.label().to_string(), operation.verbs())),
        );
        rows.push(("an object".to_string(), crate::OBJECT_VERBS));
        rows
    }

    #[test]
    fn every_tool_has_a_distinct_label() {
        for (i, a) in ToolKind::ALL.iter().enumerate() {
            for b in ToolKind::ALL.iter().skip(i + 1) {
                assert_ne!(a.label(), b.label(), "two tools share a label");
            }
        }
    }

    #[test]
    fn a_voxel_only_tool_is_refused_on_an_sdf_layer_with_a_reason() {
        let error = ToolKind::Raspar
            .availability(LayerState::editable(Representation::Sdf))
            .expect_err("scrape is voxel-side");
        assert!(
            error.to_string().contains("voxel"),
            "the refusal must name what the tool needs: {error}"
        );
    }

    #[test]
    fn an_sdf_only_tool_is_refused_on_a_voxel_layer_with_a_reason() {
        // Mover used to be the example here and is now on all three, which is
        // the kind of drift this file's tables exist to make visible. The
        // topological drag takes its place: it bakes a re-sampled volume, and
        // a grid's cells *are* its volume.
        let error = ToolKind::MoverTopologico
            .availability(LayerState::editable(Representation::Voxel))
            .expect_err("the topological drag is field-side");
        assert!(error.to_string().contains("SDF"), "{error}");
    }

    #[test]
    fn switching_to_a_supporting_layer_re_enables_a_tool() {
        assert!(ToolKind::Raspar
            .availability(LayerState::editable(Representation::Sdf))
            .is_err());
        assert!(
            ToolKind::Raspar
                .availability(LayerState::editable(Representation::Voxel))
                .is_ok(),
            "the tool must become available without being reselected"
        );
    }

    /// Sixteen tools reach a mesh layer, which is the whole of the engine's
    /// fixed-topology vocabulary.
    ///
    /// This test used to assert the opposite — that none of the fifteen had a
    /// mesh binding — and was written to fail the day that stopped being true
    /// rather than to pass quietly. It did.
    ///
    /// Twelve of them are tools that already existed: a smooth is a smooth
    /// whichever representation it lands on, and the capability table is what
    /// lets one tool carry three bindings instead of the shelf carrying three
    /// tools. Four had no counterpart among the fifteen and are new.
    #[test]
    fn the_mesh_vocabulary_is_bound() {
        let mesh = ToolKind::for_representation(Representation::Mesh);
        // Máscara sits on the mesh shelf and is not one of the sixteen: it
        // writes no vertices, it paints the world-addressed field the sixteen
        // consult. Counted apart rather than lumped in, so a real seventeenth
        // brush would still be caught here.
        let brushes: Vec<ToolKind> = mesh.iter().copied().filter(|t| !t.is_mask_tool()).collect();
        assert_eq!(
            brushes.len(),
            16,
            "the engine has sixteen fixed-topology brushes and {} are bound",
            brushes.len()
        );
        for tool in &brushes {
            assert!(
                tool.verb_on(Representation::Mesh)
                    .is_some_and(|verb| verb.starts_with("clay_mesh_sculptor")),
                "{} claims a mesh verb that is not a mesh sculptor call",
                tool.label()
            );
        }
        // The two that deliberately have none: a cavity fill and a shape drawn
        // on the frame are not vertex verbs.
        for tool in [ToolKind::Preencher, ToolKind::Trim] {
            assert!(
                !tool.exists_on(Representation::Mesh),
                "{} was given a mesh binding it should not have",
                tool.label()
            );
        }
    }

    #[test]
    fn no_tool_is_offered_on_a_protected_layer() {
        for tool in ToolKind::ALL {
            for representation in [
                Representation::Sdf,
                Representation::Voxel,
                Representation::Multires,
            ] {
                if !tool.exists_on(representation) {
                    continue;
                }
                assert_eq!(
                    tool.availability(LayerState {
                        editable: false,
                        // With a pass selected, so that the eraser on a
                        // hierarchy reports the lock rather than the row it is
                        // pointed at. A protected layer is what a sculptor has
                        // to fix first either way.
                        ..LayerState::in_a_pass(representation)
                    }),
                    Err(Unavailable::LayerProtected),
                    "{} on a protected {} layer",
                    tool.label(),
                    representation.label()
                );
            }
        }
    }

    #[test]
    fn trim_is_not_a_stroke_tool() {
        assert!(
            !ToolKind::Trim.is_stroke_tool(),
            "Trim's gesture is a shape drawn on the frame, not a stroke"
        );
        for tool in ToolKind::ALL {
            if tool != ToolKind::Trim {
                assert!(tool.is_stroke_tool(), "{} is a stroke tool", tool.label());
            }
        }
    }

    #[test]
    fn every_tool_works_on_at_least_one_representation() {
        for tool in ToolKind::ALL {
            let usable = Representation::ALL
                .iter()
                .any(|r| tool.availability(LayerState::editable(*r)).is_ok());
            assert!(usable, "{} can never be used", tool.label());
        }
    }

    /// 1.4. Every tool answers for every representation, so a tool cannot be
    /// left out of the table and quietly become unavailable everywhere.
    #[test]
    fn the_table_answers_for_every_tool_on_every_representation() {
        for tool in ToolKind::ALL {
            let verbs = tool.verbs();
            assert!(
                verbs.count() > 0,
                "{} names no verb on any representation, so it can never be \
                 offered — either bind it or take it out of ALL",
                tool.label()
            );
            for representation in Representation::ALL {
                // The point is that this does not panic and does not disagree
                // with itself: `exists_on` and `verb_on` are one lookup.
                assert_eq!(
                    tool.exists_on(representation),
                    tool.verb_on(representation).is_some(),
                    "{} disagrees with itself on {}",
                    tool.label(),
                    representation.label()
                );
            }
        }
    }

    /// 1.4. The shelf's list and the availability rule are the same lookup, so
    /// they cannot drift into showing a tool that refuses or hiding one that
    /// would work.
    ///
    /// Asked of a layer with **nothing standing in the way**: editable,
    /// visible, carrying its geometry, and — where the representation offers a
    /// choice of row — with the row that takes a stroke selected. Every one of
    /// those is a condition a sculptor can put right in a click, and the shelf
    /// deliberately does not filter on any of them: a tool that vanished when
    /// a layer was locked would leave nobody to tell that it was the lock. What
    /// this holds is the other rule — that the *representation* half of the
    /// lookup is one lookup, so no tool is listed for a layer whose
    /// representation has no verb for it.
    #[test]
    fn the_shelf_and_the_availability_rule_agree() {
        for representation in Representation::ALL {
            let offered = ToolKind::for_representation(representation);
            for tool in ToolKind::ALL {
                let shown = offered.contains(&tool);
                let usable = tool
                    .availability(LayerState::in_a_pass(representation))
                    .is_ok();
                assert_eq!(
                    shown,
                    usable,
                    "{} is {} on {} but {} by availability",
                    tool.label(),
                    if shown { "shown" } else { "hidden" },
                    representation.label(),
                    if usable { "allowed" } else { "refused" }
                );
            }
        }
    }

    /// 1.5. What the application reaches, against what the engine has.
    ///
    /// Not an assertion that the numbers are equal — they are not, and closing
    /// that is what the rest of this change is for. It is an assertion that
    /// they are what we last looked at, so taking up an engine release that
    /// adds a verb fails here instead of passing in silence. That silence is
    /// exactly how "mesh layers are carried, not sculpted" outlived the fact
    /// it described.
    ///
    /// Update the figures **and** the coverage note when a phase lands.
    #[test]
    fn the_coverage_against_the_engine_is_what_we_last_measured() {
        // ClayCore 0.39.0, counted from `bindings/c/clay.h`.
        const ENGINE_MESH_BRUSHES: usize = 16;
        /// The ten `clay_voxel_sculpt_*` verbs. The paint and erase brushes
        /// are a separate family and are *not* in this count, so the number of
        /// tools reaching a voxel layer is legitimately larger than it.
        const ENGINE_VOXEL_SCULPT_VERBS: usize = 10;

        // Máscara is on all three shelves and is a brush on none of them; see
        // `the_mesh_vocabulary_is_bound`.
        let mesh = ToolKind::for_representation(Representation::Mesh)
            .iter()
            .filter(|t| !t.is_mask_tool())
            .count();
        let voxel = ToolKind::for_representation(Representation::Voxel).len();

        assert_eq!(
            mesh, ENGINE_MESH_BRUSHES,
            "the mesh vocabulary has moved: {mesh} of the engine's \
             {ENGINE_MESH_BRUSHES} fixed-topology brushes are bound. Update \
             this count and `docs/features.md` together."
        );
        // And the field, which `docs/features.md` states as a count too.
        let sdf = ToolKind::for_representation(Representation::Sdf).len();
        assert_eq!(
            sdf, 13,
            "the field vocabulary has moved: {sdf} tools reach an SDF layer. \
             Update this count and `docs/features.md` together."
        );
        // And the hierarchy, which is the mesh vocabulary less the two colour
        // brushes, plus the per-pass eraser, plus the mask — fifteen brushes
        // and Máscara.
        let multires_brushes = ToolKind::for_representation(Representation::Multires)
            .iter()
            .filter(|t| !t.is_mask_tool())
            .count();
        assert_eq!(
            multires_brushes,
            ENGINE_MESH_BRUSHES - 2 + 1,
            "the hierarchy's vocabulary has moved: {multires_brushes} brushes \
             reach a multires layer, of the engine's {ENGINE_MESH_BRUSHES} — \
             one brush runtime across the representations (ClayCore #419), \
             less Pintar and Borrar, which have no colour to write, and plus \
             Apagar, which is not a mesh brush at all: it takes the selected \
             pass's detail toward zero through the layered stroke. Update \
             this count and `docs/features.md` together."
        );
        assert_eq!(
            voxel, 14,
            "the voxel vocabulary has moved: {voxel} tools reach a voxel \
             layer. Twelve entries use sculpt verbs, including the Crease \
             recipe over Inflate, of the engine's \
             {ENGINE_VOXEL_SCULPT_VERBS} — Máscara is on the shelf and is a \
             brush on none of the three — and the other two are the paint and \
             erase brushes, which are a different family. Update this count \
             and `docs/features.md` together."
        );
        // And the adaptive surface: the engine's fifteen of sixteen — every
        // fixed-topology brush but Layer — and the mask.
        let dynamic_brushes = ToolKind::for_representation(Representation::Dynamic)
            .iter()
            .filter(|t| !t.is_mask_tool())
            .count();
        assert_eq!(
            dynamic_brushes,
            ENGINE_MESH_BRUSHES - 1,
            "the adaptive vocabulary has moved: {dynamic_brushes} brushes \
             reach a dynamic layer, of the engine's {ENGINE_MESH_BRUSHES}, \
             which offers all of them but Layer. Update this count and \
             `docs/features.md` together."
        );
    }

    /// Dynamic is a column like the other four in every lookup the table
    /// answers: the shelf, the binding, the note, the substitute and the
    /// availability rule.
    #[test]
    fn dynamic_participates_in_every_capability_lookup() {
        assert!(Representation::ALL.contains(&Representation::Dynamic));
        let shelf = ToolKind::for_representation(Representation::Dynamic);
        assert!(!shelf.is_empty(), "an adaptive layer offers a shelf");
        for tool in ToolKind::ALL {
            let binding = tool.binding_on(Representation::Dynamic);
            assert_eq!(binding.is_some(), shelf.contains(&tool));
            assert_eq!(
                tool.availability(LayerState::editable(Representation::Dynamic))
                    .is_ok(),
                binding.is_some(),
                "{} on dynamic: the shelf and the rule disagree",
                tool.label()
            );
            let Some(binding) = binding else {
                continue;
            };
            // Every adaptive binding but the mask is the engine's own adaptive
            // family, and means what the mesh row for the same tool means.
            if !tool.is_mask_tool() {
                assert_eq!(binding.family, ExecutionFamily::DynamicVerb);
                let mesh = tool
                    .binding_on(Representation::Mesh)
                    .expect("every adaptive brush is a mesh brush too");
                assert_eq!(binding.intent, mesh.intent, "{}", tool.label());
                assert_eq!(binding.fidelity, mesh.fidelity, "{}", tool.label());
            }
            assert!(tool.remesh_timing().is_some() || tool.is_mask_tool());
        }
        // A tool absent here falls back to one that is present, the same way
        // on every call.
        for tool in ToolKind::ALL {
            let stand_in = tool.substitute_on(Representation::Dynamic);
            assert!(stand_in.exists_on(Representation::Dynamic));
            assert_eq!(stand_in, tool.substitute_on(Representation::Dynamic));
        }
    }

    /// Layer has no adaptive binding, and the absence is explained rather than
    /// silent.
    #[test]
    fn layer_is_absent_on_dynamic() {
        assert!(!ToolKind::Camada.exists_on(Representation::Dynamic));
        assert!(ToolKind::Camada.exists_on(Representation::Mesh));
        assert_eq!(
            ToolKind::Camada.note_on(Representation::Dynamic),
            Some(ToolNote::DynamicHasNoLayer)
        );
        let refused = ToolKind::Camada
            .availability(LayerState::editable(Representation::Dynamic))
            .expect_err("Layer is not offered on an adaptive layer");
        assert!(matches!(
            refused,
            Unavailable::NoVerbHere {
                active: Representation::Dynamic,
                note: Some(ToolNote::DynamicHasNoLayer),
                ..
            }
        ));
        // It falls back to the deposit that means the same act.
        let stand_in = ToolKind::Camada.substitute_on(Representation::Dynamic);
        assert_eq!(
            stand_in.intent(),
            ToolKind::Camada.intent(),
            "Layer stands in as {}",
            stand_in.label()
        );
    }

    /// The per-verb remesh schedule the issue sets out and the engine applies.
    #[test]
    fn an_adaptive_brush_remeshes_where_its_verb_needs_it() {
        assert_eq!(
            ToolKind::Mover.remesh_timing(),
            Some(RemeshTiming::AfterBrush)
        );
        assert_eq!(
            ToolKind::Argila.remesh_timing(),
            Some(RemeshTiming::BeforeBrush)
        );
        assert_eq!(
            ToolKind::Puxar.remesh_timing(),
            Some(RemeshTiming::BeforeAndAfter)
        );
        assert_eq!(ToolKind::Camada.remesh_timing(), None);
        assert_eq!(ToolKind::Mascara.remesh_timing(), None);
        assert_eq!(ToolKind::Trim.remesh_timing(), None);
    }

    /// A representation's stored key names it and nothing else, and an
    /// adaptive surface is never stored as a mesh.
    #[test]
    fn every_representation_round_trips_through_its_key() {
        let mut seen = std::collections::BTreeSet::new();
        for representation in Representation::ALL {
            assert!(seen.insert(representation.key()), "two share a key");
            assert_eq!(
                Representation::from_key(representation.key()),
                Some(representation)
            );
        }
        assert_eq!(Representation::Dynamic.key(), "dynamic");
        assert_ne!(Representation::Dynamic.key(), Representation::Mesh.key());
        assert_ne!(
            Representation::Dynamic.label(),
            Representation::Mesh.label()
        );
        assert_eq!(Representation::from_key("adaptive-v2"), None);
    }

    #[test]
    fn brush_settings_are_clamped_to_what_the_engine_accepts() {
        let settings = BrushSettings {
            size: -5.0,
            intensity: 4.0,
            flow: 0.0,
            invert: false,
            // Out of range in both directions, so the clamp below has
            // something to do.
            dynamics: Dynamics {
                pressure_size: 3.0,
                pressure_strength: -1.0,
                pressure_curve: 0.0,
                taper_start: 9.0,
                taper_end: -2.0,
                rake: true,
            },
            // Carried through rather than clamped: both are already closed
            // sets — a named curve and a flag — so there is no out-of-range
            // value for `sanitized` to bring back.
            drag: Drag {
                falloff: DragFalloff::Tight,
                front_only: true,
            },
            shaping: Shaping {
                noise: 8.0,
                smoothing: 1.0,
                ..Default::default()
            },
            alpha: false,
        }
        .sanitized();
        assert!(settings.shaping.noise <= 1.0);
        assert!(
            settings.shaping.smoothing < 1.0,
            "a lag of exactly 1 would leave the stroke never reaching the pointer"
        );
        assert!(
            settings.size > 0.0,
            "a non-positive radius is rejected by the engine"
        );
        assert!(settings.intensity <= 1.0);
        assert!(settings.flow > 0.0);
    }

    /// The grain survives being sanitized, and it is asserted at a quarter
    /// turn rather than at zero on purpose.
    ///
    /// A default survives a field that has been dropped exactly as well as one
    /// that has been carried — which is how upstream's own round trip missed
    /// this very field going missing, every preset in their reference set
    /// having an azimuth of zero. So the value under test is one nothing would
    /// produce by accident.
    #[test]
    fn a_turned_brush_is_still_turned_after_it_is_sanitized() {
        let quarter = std::f32::consts::FRAC_PI_2;
        let settings = BrushSettings {
            shaping: Shaping {
                azimuth: quarter,
                ..Default::default()
            },
            ..Default::default()
        }
        .sanitized();
        assert_eq!(settings.shaping.azimuth, quarter);
    }

    /// A whole turn is no turn, and it comes back as the zero the engine
    /// treats specially rather than as a number just short of one revolution.
    #[test]
    fn a_whole_turn_of_grain_comes_back_to_none() {
        let settings = BrushSettings {
            shaping: Shaping {
                azimuth: std::f32::consts::TAU,
                ..Default::default()
            },
            ..Default::default()
        }
        .sanitized();
        assert_eq!(settings.shaping.azimuth, 0.0);
    }

    /// And a grain that is not a number at all becomes none, because the
    /// engine builds a rotation basis out of it and a NaN there is a stamp
    /// with no orientation rather than a stamp at a strange one.
    #[test]
    fn a_grain_that_is_not_a_number_is_no_grain() {
        for wrong in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let settings = BrushSettings {
                shaping: Shaping {
                    azimuth: wrong,
                    ..Default::default()
                },
                ..Default::default()
            }
            .sanitized();
            assert_eq!(settings.shaping.azimuth, 0.0, "{wrong} survived");
        }
    }

    // -- the fourth representation -------------------------------------------

    /// The hierarchy's shelf is the mesh's, less the two brushes that write a
    /// colour and plus the one eraser a stack of passes makes meaningful.
    ///
    /// Asserted as a *difference from the mesh column* rather than as a list of
    /// fifteen names, because that is the claim the engine actually makes:
    /// `clay_multires_sculptor_stamp` takes a `clay_mesh_brush_desc` and runs
    /// the fixed sculptor over the level's own mesh, so a verb that arrives on
    /// a mesh layer arrives here on the same day unless something about the
    /// hierarchy stops it. Writing the fifteen out would pass on the day a
    /// seventeenth mesh brush landed and nobody thought about this column.
    ///
    /// Both differences are named, and both are the representation's own.
    /// A hierarchy stores no colour, so two brushes have nothing to write; a
    /// hierarchy stores detail in channels that can be taken back one at a
    /// time, so one verb has something to do that a single-surface mesh has
    /// not. Neither is a mesh brush that was forgotten.
    #[test]
    fn a_hierarchy_sculpts_with_the_mesh_vocabulary_less_its_colour() {
        let mesh: Vec<ToolKind> = ToolKind::for_representation(Representation::Mesh);
        let multires: Vec<ToolKind> = ToolKind::for_representation(Representation::Multires);

        let missing: Vec<ToolKind> = mesh
            .iter()
            .copied()
            .filter(|tool| !multires.contains(tool))
            .collect();
        assert_eq!(
            missing,
            vec![ToolKind::Pintar, ToolKind::Borrar],
            "the hierarchy's shelf differs from the mesh's by something other \
             than the two colour brushes"
        );
        let extra: Vec<ToolKind> = multires
            .iter()
            .copied()
            .filter(|tool| !mesh.contains(tool))
            .collect();
        assert_eq!(
            extra,
            vec![ToolKind::Apagar],
            "the hierarchy was given a verb the mesh sculptor does not have, \
             and the per-pass eraser is the only one the representation earns: \
             it writes a displacement channel a mesh does not store"
        );
        for tool in &multires {
            let verb = tool.verb_on(Representation::Multires).expect("a verb");
            assert!(
                verb.starts_with("clay_multires_") || tool.is_mask_tool(),
                "{} claims a hierarchy verb that is not a hierarchy call: {verb}",
                tool.label()
            );
        }
    }

    /// The mask is one call on all four, and it has to stay that way.
    ///
    /// A mask is not part of any representation — it is a world-addressed field
    /// the verbs consult — so freezing a region of a hierarchy is the same act
    /// as freezing a region of a field, and the hierarchy takes it "exactly as
    /// every other representation takes one".
    #[test]
    fn the_mask_is_the_same_call_wherever_it_is_painted() {
        let verbs = ToolKind::Mascara.verbs();
        assert_eq!(verbs.count(), Representation::ALL.len());
        for representation in Representation::ALL {
            // The whole binding rather than the name alone: one call painted
            // four ways would satisfy a name comparison and would still be
            // four different acts under one button.
            assert_eq!(
                verbs.on(representation),
                Some(Binding::new(
                    "clay_mask_apply_stroke",
                    SemanticIntent::Mask,
                    ExecutionFamily::MaskField,
                    Fidelity::Native
                )),
                "the mask took a different route on {}",
                representation.label()
            );
        }
    }

    /// A hierarchy stores where its vertices went, not what colour they are, so
    /// the colour brushes are absent — and the refusal says which of those two
    /// it is rather than leaving a sculptor to read it as an oversight.
    #[test]
    fn a_colour_brush_on_a_hierarchy_is_refused_with_the_reason_and_not_only_the_list() {
        for tool in [ToolKind::Pintar, ToolKind::Borrar] {
            let error = tool
                .availability(LayerState::editable(Representation::Multires))
                .expect_err("a hierarchy carries no colour");
            match error {
                Unavailable::NoVerbHere { note, .. } => assert_eq!(
                    note,
                    Some(ToolNote::MultiresStoresNoColour),
                    "{} is refused with no reason beyond where it does apply",
                    tool.label()
                ),
                other => panic!("{} was refused for the wrong reason: {other}", tool.label()),
            }
            // And the list is still there, naming the route that does work.
            assert!(
                error.to_string().contains("mesh"),
                "the refusal must still say where the brush does apply: {error}"
            );
        }
    }

    /// The eraser on a hierarchy is offered for a pass and refused for the
    /// form, and the refusal is its own sentence.
    ///
    /// Not [`Unavailable::NoVerbHere`], which would send a sculptor to the
    /// grid, and not [`Unavailable::MissingAttribute`], which would claim the
    /// layer is short of something. Nothing is missing: the stack may be full
    /// of passes and the selected row is simply the form, where walking the
    /// target channel to zero would take the whole surface back toward the
    /// pure subdivision rather than lift one deposit.
    #[test]
    fn the_hierarchy_eraser_needs_a_pass_and_says_so() {
        ToolKind::Apagar
            .availability(LayerState::in_a_pass(Representation::Multires))
            .expect("with a pass selected, the eraser is what the pass is for");

        let refused = ToolKind::Apagar
            .availability(LayerState::editable(Representation::Multires))
            .expect_err("the form is not a pass");
        assert_eq!(refused, Unavailable::NeedsAPass);
        let said = refused.to_string();
        assert!(
            said.contains("pass"),
            "the refusal has to name what a sculptor must select: {said}"
        );

        // And the note, which is the other half: a sculptor who has selected a
        // pass still has to be told that erasing here is not the grid's verb.
        assert_eq!(
            ToolKind::Apagar.note_on(Representation::Multires),
            Some(ToolNote::MultiresEraseTakesThisPassToZero)
        );
        assert_eq!(
            ToolKind::Apagar.note_on(Representation::Voxel),
            None,
            "the grid's eraser means what its name says and needs no caveat"
        );
    }

    /// The pass rule reaches exactly one pair.
    ///
    /// Written as a walk rather than as a single assertion about Apagar,
    /// because the failure worth catching is the opposite one: a tool that
    /// quietly starts refusing the form on a hierarchy, or on a grid where
    /// there are no passes at all, is a tool that vanished from a shelf for a
    /// reason nobody stated.
    #[test]
    fn only_the_hierarchy_eraser_is_gated_on_a_pass() {
        for tool in ToolKind::ALL {
            for representation in Representation::ALL {
                if !tool.exists_on(representation) {
                    continue;
                }
                let gated = tool.availability(LayerState::editable(representation))
                    == Err(Unavailable::NeedsAPass);
                assert_eq!(
                    gated,
                    (tool, representation) == (ToolKind::Apagar, Representation::Multires),
                    "{} on {} is gated on a pass and should not be",
                    tool.label(),
                    representation.label()
                );
            }
        }
    }

    /// Only the deliberate colour, buildup, field-duplicate and
    /// adaptive-Layer absences carry a note.
    #[test]
    fn an_ordinary_absence_is_refused_without_a_second_sentence() {
        for tool in ToolKind::ALL {
            for representation in Representation::ALL {
                let Err(Unavailable::NoVerbHere { note, .. }) =
                    tool.availability(LayerState::editable(representation))
                else {
                    continue;
                };
                let expected = matches!(
                    (tool, representation),
                    (
                        ToolKind::Pintar | ToolKind::Borrar,
                        Representation::Multires
                    ) | (ToolKind::Borrar | ToolKind::Argila, Representation::Voxel)
                        | (ToolKind::Camada, Representation::Dynamic)
                        | (ToolKind::Polir | ToolKind::Relaxar, Representation::Sdf)
                );
                assert_eq!(
                    note.is_some(),
                    expected,
                    "{} on {} carries the wrong kind of refusal",
                    tool.label(),
                    representation.label()
                );
            }
        }
    }

    /// The colour rule is answered in one place, not two.
    ///
    /// `needs_colour_attribute` is about a mesh row that may or may not carry a
    /// colour attribute — a question with two answers. On a hierarchy there is
    /// no question, because the brush is not offered at all, and this pins the
    /// two answers together so that offering the brush later without giving it
    /// somewhere to write fails here.
    #[test]
    fn a_hierarchy_is_never_asked_for_a_colour_attribute() {
        for tool in ToolKind::ALL {
            assert!(
                !tool.needs_colour_attribute(Representation::Multires),
                "{} would ask a hierarchy for a colour attribute",
                tool.label()
            );
            if tool.writes_colour() {
                assert!(
                    !tool.exists_on(Representation::Multires),
                    "{} writes colour and is offered on a hierarchy, which \
                     stores none",
                    tool.label()
                );
            }
        }
    }

    /// A smooth on a hierarchy is the one tool that carries a caveat there, and
    /// it is a caveat about a choice the other three do not have.
    #[test]
    fn a_smooth_on_a_hierarchy_says_it_picks_a_frequency() {
        assert_eq!(
            ToolKind::Suavizar.note_on(Representation::Multires),
            Some(ToolNote::MultiresSmoothChoosesAFrequency)
        );
        assert_eq!(ToolKind::Suavizar.note_on(Representation::Mesh), None);
        assert_eq!(
            ToolKind::Suavizar.verb_on(Representation::Multires),
            Some("clay_multires_sculpt_layer_stroke_smooth"),
            "the note has to name a call that takes a mode"
        );
    }

    /// A drag on a hierarchy previews as it moves, as it does on a mesh — the
    /// grid is the only representation that holds one whole.
    #[test]
    fn only_a_grid_holds_a_drag_whole() {
        for representation in Representation::ALL {
            assert_eq!(
                ToolKind::Mover.holds_the_whole_gesture(representation),
                representation == Representation::Voxel,
                "Mover on {}",
                representation.label()
            );
        }
    }

    /// A hierarchy row before its cage arrives is the mesh row's case, and the
    /// refusal names what it is waiting for rather than "geometry".
    #[test]
    fn a_hierarchy_with_no_cage_yet_says_it_is_waiting_for_one() {
        let waiting = LayerState {
            carries_geometry: false,
            ..LayerState::editable(Representation::Multires)
        };
        assert_eq!(
            ToolKind::Padrao.availability(waiting),
            Err(Unavailable::MissingAttribute { needs: "cage" })
        );
        assert_eq!(
            ToolKind::Padrao.availability(LayerState {
                representation: Representation::Mesh,
                ..waiting
            }),
            Err(Unavailable::MissingAttribute { needs: "mesh" }),
            "the two are different sentences because they send a sculptor to \
             different places"
        );
        // And the two that are editable from nothing still are.
        for representation in [Representation::Sdf, Representation::Voxel] {
            assert!(ToolKind::Padrao
                .availability(LayerState {
                    representation,
                    ..waiting
                })
                .is_ok());
        }
    }

    /// A whole-form operation reaches no hierarchy, and the compiler is what
    /// says so: `LayerOperation::all`'s `match` is exhaustive, so an operation
    /// added without an answer for this column stops the file compiling.
    #[test]
    fn no_whole_form_operation_reaches_a_hierarchy() {
        for operation in LayerOperation::all() {
            assert!(
                !operation.applies_to(Representation::Multires),
                "{} claims a hierarchy verb; there is no clay_multires_* \
                 deformer in the ABI",
                operation.label()
            );
        }
    }
}
