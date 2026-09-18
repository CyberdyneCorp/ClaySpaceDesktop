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
}

impl Representation {
    pub const ALL: [Representation; 4] = [Self::Sdf, Self::Voxel, Self::Mesh, Self::Multires];

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
    pub const CREATABLE: [Representation; 2] = [Self::Sdf, Self::Voxel];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sdf => "SDF",
            Self::Voxel => "voxel",
            Self::Mesh => "mesh",
            Self::Multires => "multires",
        }
    }
}

/// What one tool invokes on each of the four representations.
///
/// A field is `None` where that representation has no verb for the tool. The
/// engine's name is carried rather than a boolean so that "does this apply
/// here" and "what does it call" cannot disagree — they are one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verbs {
    pub sdf: Option<&'static str>,
    pub voxel: Option<&'static str>,
    pub mesh: Option<&'static str>,
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
    pub multires: Option<&'static str>,
}

impl Verbs {
    pub fn on(self, representation: Representation) -> Option<&'static str> {
        match representation {
            Representation::Sdf => self.sdf,
            Representation::Voxel => self.voxel,
            Representation::Mesh => self.mesh,
            Representation::Multires => self.multires,
        }
    }

    /// How many representations this tool reaches.
    pub fn count(self) -> usize {
        [self.sdf, self.voxel, self.mesh, self.multires]
            .into_iter()
            .filter(Option::is_some)
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
            let Some(verb) = row.on(representation) else {
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
            Self::Taper { .. } | Self::Twist { .. } => Verbs {
                sdf: None,
                voxel: None,
                mesh: Some("clay_mesh_sculptor_deform"),
                multires: None,
            },
            Self::LatticeDrag { .. } => Verbs {
                sdf: None,
                voxel: None,
                mesh: Some("clay_mesh_sculptor_lattice"),
                multires: None,
            },
            Self::CloseHoles { .. } => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_repair_close_holes"),
                mesh: None,
                multires: None,
            },
            Self::FillVoids => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_repair_fill_voids"),
                mesh: None,
                multires: None,
            },
            Self::RefineRegion { .. } => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_add_level_region"),
                mesh: None,
                multires: None,
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
    /// The cost is that these four do not preview while the pointer moves.
    /// They land when it comes up.
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

    pub fn is_path_driven(self) -> bool {
        matches!(
            self,
            Self::Mover | Self::MoverTopologico | Self::Puxar | Self::Nudge
        )
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
    /// The one note here attached to a tool that is **absent** rather than
    /// present. It is worth the exception because the absence otherwise reads
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
}

impl ToolNote {
    pub const ALL: [ToolNote; 5] = [
        Self::VoxelPlanarIsTwoSided,
        Self::MultiresSmoothChoosesAFrequency,
        Self::MultiresStoresNoColour,
        Self::MultiresEraseTakesThisPassToZero,
        Self::SdfStandardIsAnInflate,
    ];
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
        verbs: Verbs,
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
        for verb in [verbs.sdf, verbs.voxel, verbs.mesh, verbs.multires]
            .into_iter()
            .flatten()
        {
            if !named.contains(&verb) {
                named.push(verb);
            }
        }
        named.join(" / ")
    }

    /// The engine verb this tool invokes on each representation.
    ///
    /// `None` where the representation has no verb for it. This is the table
    /// the shelf, the availability rules and the tests all read; nothing else
    /// may decide where a tool applies, or they can drift apart again.
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
            Self::Padrao => Verbs {
                sdf: Some("clay_layer_apply_stroke (CLAY_OP_RELIEF)"),
                voxel: Some("clay_voxel_set_brush"),
                mesh: Some("clay_mesh_sculptor_apply_stroke (DRAW)"),
                multires: Some("clay_multires_sculptor_apply_stroke (DRAW)"),
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
            // what `ToolNote::SdfStandardIsAnInflate` tells a sculptor, and
            // the two rows differ in the footprint because that is the only
            // thing left for them to differ in.
            Self::Inflar => Verbs {
                sdf: Some("clay_layer_apply_stroke (CLAY_OP_RELIEF)"),
                voxel: Some("clay_voxel_sculpt_inflate"),
                mesh: Some("clay_mesh_sculptor_apply_stroke (INFLATE)"),
                multires: Some("clay_multires_sculptor_apply_stroke (INFLATE)"),
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
            Self::Suavizar => Verbs {
                sdf: Some("clay_item_volume_relax_from"),
                voxel: Some("clay_voxel_sculpt_smooth"),
                mesh: Some("clay_mesh_sculptor_apply_stroke (SMOOTH)"),
                multires: Some("clay_multires_sculpt_layer_stroke_smooth"),
            },
            // The one tool that is the same call on all four, because a
            // mask is not part of any of them: it is a world-addressed field
            // the verbs consult, and freezing a region of a mesh is the same
            // act as freezing a region of a field. A hierarchy takes it the
            // same way — the layer transform is used only to find each vertex
            // on the mask's own lattice.
            Self::Mascara => Verbs {
                sdf: Some("clay_mask_apply_stroke"),
                voxel: Some("clay_mask_apply_stroke"),
                mesh: Some("clay_mask_apply_stroke"),
                multires: Some("clay_mask_apply_stroke"),
            },
            // The grid's column is Padrão's, and the clamp has nowhere to
            // land: a clamped accumulation is a ceiling on how much a stroke
            // may deposit *over itself*, and a cell is set or it is not. So
            // the row names the deposit rather than repeating the field's
            // sentence about a ceiling a grid cannot have.
            Self::Camada => Verbs {
                sdf: Some("clay_layer_apply_stroke (clamped accumulation)"),
                voxel: Some("clay_voxel_set_brush"),
                mesh: Some("clay_mesh_sculptor_apply_stroke (LAYER)"),
                multires: Some("clay_multires_sculptor_apply_stroke (LAYER)"),
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
                sdf: Some(
                    "clay_sdf_move_begin / clay_sdf_move_update / clay_sdf_move_commit \
                     (clay_layer_move_surface_regions when held)",
                ),
                voxel: Some("clay_voxel_sculpt_grab"),
                mesh: Some("clay_mesh_sculptor_stamp (GRAB)"),
                multires: Some("clay_multires_sculptor_stamp (GRAB)"),
            },
            // SDF only, and that is the engine's answer rather than a
            // shortcut. The verb bakes a re-sampled *volume*, which a grid has
            // no equivalent of — its cells are the volume — and a mesh's
            // geodesic Grab is a different thing wearing a similar
            // description: it walks the surface to weight a stamp, where this
            // re-samples a field with the move applied.
            Self::MoverTopologico => Verbs {
                sdf: Some("clay_item_volume_move_topological"),
                voxel: None,
                mesh: None,
                // A hierarchy has no volume to bake either, and the geodesic
                // Grab it does have is `Mover`'s verb rather than this one.
                multires: None,
            },
            Self::Puxar => Verbs {
                sdf: Some("clay_item_set_curve_points (snakehook)"),
                voxel: None,
                mesh: Some("clay_mesh_sculptor_apply_stroke (SNAKEHOOK)"),
                multires: Some("clay_multires_sculptor_apply_stroke (SNAKEHOOK)"),
            },
            // Two-sided on a grid, cut-only on the other two, and the
            // difference is the engine's rather than a compromise: the voxel
            // verb fills hollows below the plane as well as taking material
            // off above it, and faking cut-only would mean reading occupancy
            // back and reapplying it — voxel math this application does not
            // do. The tooltip says which one a sculptor is holding.
            Self::Planar => Verbs {
                sdf: Some("clay_item_volume_flatten_from (cut-only)"),
                voxel: Some("clay_voxel_sculpt_flatten (two-sided)"),
                mesh: Some("clay_mesh_sculptor_apply_stroke (FLATTEN)"),
                multires: Some("clay_multires_sculptor_apply_stroke (FLATTEN)"),
            },
            Self::Polir => Verbs {
                sdf: Some("clay_item_volume_flatten_from (cut-only, hPolish)"),
                voxel: None,
                mesh: Some("clay_mesh_sculptor_apply_stroke (POLISH)"),
                multires: Some("clay_multires_sculptor_apply_stroke (POLISH)"),
            },
            Self::Relaxar => Verbs {
                sdf: Some("clay_item_volume_relax_from"),
                voxel: None,
                mesh: Some("clay_mesh_sculptor_apply_stroke (RELAX)"),
                multires: Some("clay_multires_sculptor_apply_stroke (RELAX)"),
            },
            Self::Trim => Verbs {
                sdf: Some("clay_cut_create"),
                voxel: None,
                mesh: None,
                multires: None,
            },
            Self::Raspar => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_sculpt_scrape"),
                mesh: Some("clay_mesh_sculptor_apply_stroke (SCRAPE)"),
                multires: Some("clay_multires_sculptor_apply_stroke (SCRAPE)"),
            },
            Self::Preencher => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_sculpt_fill_cavities"),
                mesh: None,
                multires: None,
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
                sdf: Some("clay_layer_magnify_surface (negative strength)"),
                voxel: Some("clay_voxel_sculpt_pinch"),
                mesh: Some("clay_mesh_sculptor_apply_stroke (PINCH)"),
                multires: Some("clay_multires_sculptor_apply_stroke (PINCH)"),
            },
            // Relief with buildup, which is what ClayBuildup *is*: the
            // engine's equivalence table maps Clay to relief along the stroke
            // plus buildup accumulation, and the difference from Padrão is the
            // accumulation and the spacing rather than another verb.
            Self::Argila => Verbs {
                sdf: Some("clay_layer_apply_stroke (CLAY_OP_RELIEF, buildup)"),
                voxel: None,
                mesh: Some("clay_mesh_sculptor_apply_stroke (CLAY)"),
                multires: Some("clay_multires_sculptor_apply_stroke (CLAY)"),
            },
            // Incise, which the engine describes in the same sentence as the
            // tool: "a thin region gives the line — Crease and DamStandard".
            // Not a subtraction of spheres, which is what a Crease built out
            // of the general vocabulary would be — incise exists precisely to
            // displace the accumulated field inward without contributing a
            // primitive.
            //
            // Voxel is left absent. The engine documents DamStandard there as
            // a *recipe* rather than a verb, and a preset that borrows a name
            // is not worth a shelf entry until somebody has looked at it.
            Self::Vinco => Verbs {
                sdf: Some("clay_layer_apply_stroke (CLAY_OP_INCISE)"),
                voxel: None,
                mesh: Some("clay_mesh_sculptor_apply_stroke (CREASE)"),
                multires: Some("clay_multires_sculptor_apply_stroke (CREASE)"),
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
                voxel: Some("clay_voxel_paint_brush"),
                mesh: Some("clay_mesh_sculptor_apply_stroke (PAINT)"),
                multires: None,
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
                voxel: Some("clay_voxel_erase_brush"),
                mesh: None,
                multires: Some("clay_multires_sculpt_layer_stroke_erase"),
            },
            // And the other half of the colour absence Pintar's comment
            // explains, two rows up.
            Self::Borrar => Verbs {
                sdf: None,
                voxel: None,
                mesh: Some("clay_mesh_sculptor_apply_stroke (SMEAR)"),
                multires: None,
            },
            Self::Nudge => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_sculpt_smudge"),
                mesh: Some("clay_mesh_sculptor_apply_stroke (NUDGE)"),
                multires: Some("clay_multires_sculptor_apply_stroke (NUDGE)"),
            },
        }
    }

    /// The verb this tool invokes on `representation`, if it has one there.
    pub fn verb_on(self, representation: Representation) -> Option<&'static str> {
        self.verbs().on(representation)
    }

    /// Whether this tool exists at all on `representation`.
    ///
    /// What the shelf filters on. A tool that answers `false` is not shown for
    /// that layer, rather than shown disabled.
    pub fn exists_on(self, representation: Representation) -> bool {
        self.verb_on(representation).is_some()
    }

    /// The tools a representation can offer, in the shelf's own order.
    pub fn for_representation(representation: Representation) -> Vec<ToolKind> {
        Self::ALL
            .into_iter()
            .filter(|tool| tool.exists_on(representation))
            .collect()
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
                verbs: self.verbs(),
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
    /// Answers for a pair whether or not the tool is *offered* on it. Four of
    /// the five notes describe a tool that is there, and one describes one
    /// that is not — [`ToolKind::availability`] carries that one into the
    /// refusal, since a tool nobody can select is a tool nobody can hover.
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
    pub fn needs_colour_attribute(self, representation: Representation) -> bool {
        self.writes_colour() && representation == Representation::Mesh
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
/// Held per tool, so switching away and back returns the settings the user
/// left rather than a default.
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
                let Some(verb) = operation.verbs().on(representation) else {
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
    /// Máscara is the case: one call on all four representations, and a report
    /// that said so four times would read as four bindings.
    #[test]
    fn the_diagnostics_line_names_each_call_once() {
        assert_eq!(ToolKind::Mascara.engine_verbs(), "clay_mask_apply_stroke");
        let padrao = ToolKind::Padrao.engine_verbs();
        assert_eq!(
            padrao.matches("clay_").count(),
            4,
            "Padrão reaches four representations by four different calls: {padrao}"
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
            sdf, 15,
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
            voxel, 13,
            "the voxel vocabulary has moved: {voxel} tools reach a voxel \
             layer. Eleven of them are sculpt verbs, of the engine's \
             {ENGINE_VOXEL_SCULPT_VERBS} — Máscara is on the shelf and is a \
             brush on none of the three — and the other two are the paint and \
             erase brushes, which are a different family. Update this count \
             and `docs/features.md` together."
        );
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
            assert_eq!(
                verbs.on(representation),
                Some("clay_mask_apply_stroke"),
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

    /// Every other absence carries no note, which is what keeps the one that
    /// does worth reading.
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
                    )
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
