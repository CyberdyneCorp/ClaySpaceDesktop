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
//! offered and disabled. With three representations carrying substantially
//! different vocabularies, one list would be mostly disabled entries whatever
//! the active layer, all carrying the same sentence. A tool that *has* a verb
//! here and cannot be used right now — a locked layer, a hidden one, a missing
//! attribute — is still shown, disabled, with which of those it is.

/// Which representation a layer holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Representation {
    /// An ordered edit list evaluated as a distance field.
    Sdf,
    /// A palette-indexed voxel grid.
    Voxel,
    /// Imported triangles, held verbatim.
    Mesh,
}

impl Representation {
    pub const ALL: [Representation; 3] = [Self::Sdf, Self::Voxel, Self::Mesh];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sdf => "SDF",
            Self::Voxel => "voxel",
            Self::Mesh => "mesh",
        }
    }
}

/// What one tool invokes on each of the three representations.
///
/// A field is `None` where that representation has no verb for the tool. The
/// engine's name is carried rather than a boolean so that "does this apply
/// here" and "what does it call" cannot disagree — they are one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verbs {
    pub sdf: Option<&'static str>,
    pub voxel: Option<&'static str>,
    pub mesh: Option<&'static str>,
}

impl Verbs {
    pub fn on(self, representation: Representation) -> Option<&'static str> {
        match representation {
            Representation::Sdf => self.sdf,
            Representation::Voxel => self.voxel,
            Representation::Mesh => self.mesh,
        }
    }

    /// How many representations this tool reaches.
    pub fn count(self) -> usize {
        [self.sdf, self.voxel, self.mesh]
            .into_iter()
            .filter(Option::is_some)
            .count()
    }
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
        match self {
            Self::Taper { .. } | Self::Twist { .. } => Verbs {
                sdf: None,
                voxel: None,
                mesh: Some("clay_mesh_sculptor_deform"),
            },
            Self::LatticeDrag { .. } => Verbs {
                sdf: None,
                voxel: None,
                mesh: Some("clay_mesh_sculptor_lattice"),
            },
            Self::CloseHoles { .. } => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_repair_close_holes"),
                mesh: None,
            },
            Self::FillVoids => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_repair_fill_voids"),
                mesh: None,
            },
            Self::RefineRegion { .. } => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_add_level_region"),
                mesh: None,
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
}

impl LayerState {
    /// The common case: an ordinary editable, visible layer.
    pub fn editable(representation: Representation) -> Self {
        Self {
            representation,
            editable: true,
            visible: true,
            carries_geometry: true,
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

    pub fn is_path_driven(self) -> bool {
        matches!(self, Self::Mover | Self::Puxar | Self::Nudge)
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
        verbs: Verbs,
    },
    /// The layer is ghosted or locked.
    LayerProtected,
    /// The layer is hidden, so an edit would land where nothing is drawn.
    LayerHidden,
    /// The layer carries no attribute this tool needs — a mesh with no colour
    /// for a colour brush, say. Produced by the tools that require one.
    MissingAttribute { needs: &'static str },
}

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoVerbHere { active, verbs } => {
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
        }
    }
}

impl ToolKind {
    /// Every tool, in the order the brush shelf presents them.
    pub const ALL: [ToolKind; 20] = [
        Self::Padrao,
        Self::Inflar,
        Self::Suavizar,
        Self::Mover,
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

    /// The label the interface shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Padrao => "Padrão",
            Self::Mover => "Mover",
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
        for verb in [verbs.sdf, verbs.voxel, verbs.mesh].into_iter().flatten() {
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
            Self::Padrao => Verbs {
                sdf: Some("clay_layer_apply_stroke (CLAY_OP_RELIEF)"),
                voxel: Some("clay_voxel_sculpt_inflate"),
                mesh: Some("clay_mesh_sculptor_stamp (DRAW)"),
            },
            Self::Inflar => Verbs {
                sdf: Some("clay_layer_apply_stroke (CLAY_OP_RELIEF)"),
                voxel: Some("clay_voxel_sculpt_inflate"),
                mesh: Some("clay_mesh_sculptor_stamp (INFLATE)"),
            },
            Self::Suavizar => Verbs {
                sdf: Some("clay_item_volume_relax"),
                voxel: Some("clay_voxel_sculpt_smooth"),
                mesh: Some("clay_mesh_sculptor_stamp (SMOOTH)"),
            },
            // The one tool that is the same call on all three, because a
            // mask is not part of any of them: it is a world-addressed field
            // the verbs consult, and freezing a region of a mesh is the same
            // act as freezing a region of a field.
            Self::Mascara => Verbs {
                sdf: Some("clay_mask_apply_stroke"),
                voxel: Some("clay_mask_apply_stroke"),
                mesh: Some("clay_mask_apply_stroke"),
            },
            Self::Camada => Verbs {
                sdf: Some("clay_layer_apply_stroke (clamped accumulation)"),
                voxel: Some("clay_voxel_sculpt_inflate (clamped)"),
                mesh: Some("clay_mesh_sculptor_stamp (LAYER)"),
            },
            Self::Mover => Verbs {
                sdf: Some("clay_layer_move_surface"),
                voxel: None,
                mesh: Some("clay_mesh_sculptor_stamp (GRAB)"),
            },
            Self::Puxar => Verbs {
                sdf: Some("clay_item_set_curve_points (snakehook)"),
                voxel: None,
                mesh: Some("clay_mesh_sculptor_stamp (SNAKEHOOK)"),
            },
            Self::Planar => Verbs {
                sdf: Some("clay_item_volume_flatten (cut-only)"),
                voxel: None,
                mesh: Some("clay_mesh_sculptor_stamp (FLATTEN)"),
            },
            Self::Polir => Verbs {
                sdf: Some("clay_item_volume_flatten (cut-only, hPolish)"),
                voxel: None,
                mesh: Some("clay_mesh_sculptor_stamp (POLISH)"),
            },
            Self::Relaxar => Verbs {
                sdf: Some("clay_item_volume_relax"),
                voxel: None,
                mesh: Some("clay_mesh_sculptor_stamp (RELAX)"),
            },
            Self::Trim => Verbs {
                sdf: Some("clay_cut_create"),
                voxel: None,
                mesh: None,
            },
            Self::Raspar => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_sculpt_scrape"),
                mesh: Some("clay_mesh_sculptor_stamp (SCRAPE)"),
            },
            Self::Preencher => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_sculpt_fill_cavities"),
                mesh: None,
            },
            Self::Pincar => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_sculpt_pinch"),
                mesh: Some("clay_mesh_sculptor_stamp (PINCH)"),
            },
            Self::Argila => Verbs {
                sdf: None,
                voxel: None,
                mesh: Some("clay_mesh_sculptor_stamp (CLAY)"),
            },
            Self::Vinco => Verbs {
                sdf: None,
                voxel: None,
                mesh: Some("clay_mesh_sculptor_stamp (CREASE)"),
            },
            // One tool, two bindings: "put colour here" is the same intent
            // whether the colour lands on a vertex or in a cell.
            Self::Pintar => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_paint_brush"),
                mesh: Some("clay_mesh_sculptor_stamp (PAINT)"),
            },
            Self::Apagar => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_erase_brush"),
                mesh: None,
            },
            Self::Borrar => Verbs {
                sdf: None,
                voxel: None,
                mesh: Some("clay_mesh_sculptor_stamp (SMEAR)"),
            },
            Self::Nudge => Verbs {
                sdf: None,
                voxel: Some("clay_voxel_sculpt_smudge"),
                mesh: Some("clay_mesh_sculptor_stamp (NUDGE)"),
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
            });
        }
        if !layer.editable {
            return Err(Unavailable::LayerProtected);
        }
        if !layer.visible {
            return Err(Unavailable::LayerHidden);
        }
        // A mesh row with no triangles yet. Only the mesh side can be empty in
        // this sense: a field and a grid are both editable from nothing.
        if layer.representation == Representation::Mesh && !layer.carries_geometry {
            return Err(Unavailable::MissingAttribute { needs: "mesh" });
        }
        Ok(())
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
        }
    }
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
                ..self.shaping
            },
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

    #[test]
    fn every_tool_names_an_engine_verb() {
        for tool in ToolKind::ALL {
            let verb = tool.engine_verbs();
            assert!(
                verb.starts_with("clay_"),
                "{} does not name an engine entry point: {verb}",
                tool.label()
            );
        }
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
        let error = ToolKind::Mover
            .availability(LayerState::editable(Representation::Voxel))
            .expect_err("the move brush is field-side");
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
            for representation in [Representation::Sdf, Representation::Voxel] {
                if !tool.exists_on(representation) {
                    continue;
                }
                assert_eq!(
                    tool.availability(LayerState {
                        representation,
                        editable: false,
                        visible: true,
                        carries_geometry: true,
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
    #[test]
    fn the_shelf_and_the_availability_rule_agree() {
        for representation in Representation::ALL {
            let offered = ToolKind::for_representation(representation);
            for tool in ToolKind::ALL {
                let shown = offered.contains(&tool);
                let usable = tool
                    .availability(LayerState::editable(representation))
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
        assert_eq!(
            voxel, 11,
            "the voxel vocabulary has moved: {voxel} tools reach a voxel \
             layer. Nine of them are sculpt verbs, of the engine's \
             {ENGINE_VOXEL_SCULPT_VERBS}; the other two are the paint and \
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
}
