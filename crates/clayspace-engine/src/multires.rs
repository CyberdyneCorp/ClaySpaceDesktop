//! A subdivision hierarchy the document holds beside one of its mesh layers,
//! and the file that keeps it.
//!
//! # Why there are two objects and not one
//!
//! A `clay_multires` is not a `clay_layer_id`. It is a free-standing owning
//! handle that took a **copy** of the cage on the way in, so the document it
//! was built from does not know it exists and cannot be made to — ClayCore
//! v0.78.0 calls this the largest integration cost in the release and states
//! it in the C header rather than leaving it to be found: *"a `.clayspace`
//! does not carry a multires hierarchy or an adaptive surface. Both are opaque
//! and owning by design and live beside the document; `clay_multires_serialize`
//! gives you the bytes and where they go is the host's decision. A host that
//! saves only the document saves only the cage's layer, not the sculpt on it."*
//!
//! So a hierarchy row in this application is **two things**: a real mesh layer
//! in the `.clayspace`, holding the cage, and a [`Hierarchy`] held here. The
//! layer is what gives the row a name, a place in the stack, a transform, a
//! mask and a save; the hierarchy is what gives it levels and detail. Neither
//! is optional and neither can stand for the other.
//!
//! That split decides the shape of everything below it. In particular the
//! side-car is not bookkeeping the way [`crate::objects`]'s table is: the
//! engine reports a hierarchy's layer as a **mesh** layer (there is no
//! `LayerRepresentation::Multires`), so the side-car is the only thing in the
//! world that knows a row was a hierarchy at all. Losing it does not lose a
//! convenience; it loses every level above the cage.
//!
//! # What a missing side-car does
//!
//! It opens, and the row comes back as the mesh layer it demonstrably is.
//!
//! The three answers available were: refuse the whole document, open it and
//! keep calling the row a hierarchy, or open it and call the row what is
//! actually there. The first throws away a cage somebody made over a file this
//! application wrote beside it and may never have copied. The second is the
//! worst of the three — a hierarchy that has silently lost every level is
//! indistinguishable from one that never had any, and the sculptor finds out
//! by subdividing on top of nothing. The third is honest at the cost of being
//! quiet, so it is made loud twice: the representation itself changes, which
//! the layer row, the workspace bar and the inspector all draw differently,
//! and the loss is named in the diagnostics report — the text a sculptor
//! pastes when they ask why their sculpt came back flat.
//!
//! # Why one file rather than a directory
//!
//! The survey that preceded this specified `<path>.multires/` with one blob
//! per layer and an index, so that an autosave could skip a hierarchy whose
//! checksum had not moved. Measured on the pinned engine, `clay_multires_serialize`
//! is **1.39 ms for 710 KB** at level 4 over a 16×16 cage — the skip saves a
//! millisecond on a clock that runs every two minutes, and it costs a
//! directory that Save-As has to copy and that every removal has to prune. One
//! file is rewritten whole, cannot hold a blob for a layer that no longer
//! exists, and cannot half-exist.

use claycore::{Multires, MultiresDesc};
use clayspace_model::{
    CageFault, ModelError, MultiresLevels, MultiresSculptLayer, MultiresSculptLayerId,
    MultiresSculptLayerOp, MultiresState, Refusal, SubdivisionCost, WriteDomain,
};

/// How deep the level meshes and detail may reach before a hierarchy declines
/// to subdivide further.
///
/// Zero is what `clay_multires_desc` calls no budget at all, and is what a
/// desktop host is meant to pass. This is not zero because the refusal is the
/// feature: `clay_multires_preflight_add_level` prices a level and
/// `clay_multires_add_level` refuses over budget **rather than attempting it**,
/// build-then-publish, leaving the hierarchy exactly as deep as it was. A host
/// that declares no budget has no refusal to offer and finds out what a level
/// costs by running out of memory during it.
///
/// Two gigabytes: past anything a sculpt on a reasonable cage reaches, and
/// well under what a subdivision one step too far asks for — a level
/// multiplies faces by four, so the step that does not fit misses by a factor
/// rather than by a margin.
pub const LEVEL_BUDGET: u64 = 2 * 1024 * 1024 * 1024;

/// What the multires half of the undo history may hold, in bytes.
///
/// The ABI carries no delta record for a hierarchy gesture — clay.h says so
/// twice, unprompted, of `clay_multires_sculptor_apply_stroke` and of the
/// layered stroke transaction alike — so what this application records is the
/// hierarchy's own serialized bytes on the other side of the step. That is
/// exact and it is not small, so it is bounded: the oldest records are dropped
/// once the total passes this, and a gesture whose record was dropped is one
/// undo no longer reaches.
///
/// Measured rather than guessed at: a level-4 hierarchy over a 16×16 cage
/// after one dab serializes to 710 KB, so this is roughly three hundred and
/// fifty gestures at that size and many thousands at the sizes a first
/// subdivision reaches.
pub const HISTORY_BYTES: usize = 256 * 1024 * 1024;

/// The level mesh the viewport is drawing, and what it was copied at.
///
/// Held because copying one is not free — 3.16 ms for level 4's 98,817
/// vertices on the pinned engine — and because a frame in which nothing moved
/// must not pay for it. What is beside the triangles is the whole of the
/// invalidation: the level a host asked for, and [`Hierarchy::watched`].
struct Drawn {
    level: u32,
    /// [`Hierarchy::watched`], as it stood when these triangles were copied.
    watched: (u64, u64),
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

/// A hierarchy, and everything this side has to remember about it.
pub struct Hierarchy {
    surface: Multires,
    drawn: Option<Drawn>,
    /// How many times this side has replaced the surface underneath itself.
    ///
    /// **Not the engine's revision, and it exists because the engine's cannot
    /// answer this one.** A hierarchy put back from bytes is a *new* handle
    /// whose three counters start again at one, so a document that dabs, undoes
    /// and redoes walks its evaluated revision 1 -> 3 -> 1 -> 1: the same
    /// number on either side of a redo, over two different surfaces. Anything
    /// watching the engine's counter alone — the viewport's redraw hash above
    /// all — would conclude nothing had happened and go on drawing what it
    /// uploaded last, so a redo would appear not to have happened.
    ///
    /// This is monotone across every restore, which is the half the engine's
    /// counter cannot be.
    generation: u64,
    /// The hierarchy's bytes as they stood before the gesture in progress.
    ///
    /// Taken once, on the first segment that reaches the surface, and used for
    /// two things that are the same thing: a dragging verb is laid down again
    /// from its anchor on every segment and needs the last one taken back
    /// first, and the gesture as a whole needs an exact record to enter the
    /// undo history with.
    open: Option<Vec<u8>>,
}

impl Hierarchy {
    /// Wraps a hierarchy the caller has already built.
    pub fn holding(surface: Multires) -> Self {
        Self {
            surface,
            drawn: None,
            generation: 0,
            open: None,
        }
    }

    /// The descriptor every hierarchy this application builds is built with.
    pub fn desc() -> MultiresDesc {
        MultiresDesc {
            memory_budget: LEVEL_BUDGET,
            ..MultiresDesc::default()
        }
    }

    pub fn surface_mut(&mut self) -> &mut Multires {
        &mut self.surface
    }

    /// Where the brush writes, what is drawn, and how many levels there are.
    pub fn levels(&self) -> MultiresLevels {
        MultiresLevels {
            count: self.surface.level_count().max(1),
            sculpt: self.surface.sculpt_level().unwrap_or(0),
            display: self.surface.display_level().unwrap_or(0),
        }
        .sanitized()
    }

    /// What the layer stack and the inspector are shown.
    ///
    /// The whole of it, read back from the engine every time rather than
    /// mirrored here: a pass's strength, visibility and lock are the engine's
    /// values, a merge and a bake rewrite the stack wholesale, and a restore
    /// from bytes replaces every id there was. A host-side copy would be one
    /// undo away from describing a stack that is not there.
    ///
    /// The write domain is [`WriteDomain::Automatic`] and there is no control
    /// that changes it, which is a decision and not an omission. `Automatic`
    /// means "the active pass, or the form where there is none", and the
    /// interface expresses the choice by which row is selected — the form has
    /// a row of its own under the passes. A separate three-way control beside
    /// that would be a second way to say the same thing, and the two would
    /// disagree the first time one of them was changed.
    pub fn state(&self) -> MultiresState {
        MultiresState {
            levels: self.levels(),
            sculpt_layers: self.sculpt_layers(),
            active_sculpt_layer: self.active_pass(),
            write_domain: WriteDomain::Automatic,
        }
        .sanitized()
    }

    /// What the viewport watches: the engine's evaluated counter and this
    /// side's own generation, which are two halves of one question.
    ///
    /// The engine moves `evaluated` whenever the drawn surface moved for any
    /// reason at all, which covers a stamp, a level change and a trim — and
    /// covers nothing that happens when the whole surface is replaced from
    /// bytes, because the replacement starts counting again. See
    /// [`Hierarchy::generation`].
    pub fn watched(&self) -> (u64, u64) {
        let evaluated = self.surface.revision().unwrap_or_default().evaluated;
        (evaluated, self.generation)
    }

    /// What the viewport draws, copied from the display level and kept until
    /// the surface moves under it.
    #[allow(clippy::type_complexity)]
    pub fn level_mesh(&mut self) -> Option<(&[[f32; 3]], &[[f32; 3]], &[[f32; 3]], &[u32])> {
        let level = self.levels().display;
        let watched = self.watched();
        let stale =
            !matches!(&self.drawn, Some(drawn) if drawn.level == level && drawn.watched == watched);
        if stale {
            let mesh = self.surface.copy_level_mesh(level).ok()?;
            let positions = mesh.positions().to_vec();
            let count = positions.len();
            self.drawn = Some(Drawn {
                level,
                watched,
                normals: mesh
                    .normals()
                    .map(<[[f32; 3]]>::to_vec)
                    .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; count]),
                colors: mesh
                    .colors()
                    .map(<[[f32; 3]]>::to_vec)
                    .unwrap_or_else(|| vec![[1.0; 3]; count]),
                indices: mesh.indices().to_vec(),
                positions,
            });
        }
        let drawn = self.drawn.as_ref()?;
        Some((
            &drawn.positions,
            &drawn.normals,
            &drawn.colors,
            &drawn.indices,
        ))
    }

    /// The triangles the viewport was last handed, for a caller that may not
    /// rebuild them.
    ///
    /// `None` until something has drawn or measured this hierarchy. A pick is
    /// a *question* — `SculptModel::pick` takes `&self` — so it may read the
    /// cache and may not fill it, and every path that moves the surface fills
    /// it on the way past: a crossing, a stroke and a history step all end in
    /// [`crate::document`]'s `refresh_multires_bounds`, which asks for the
    /// level mesh to measure it.
    pub fn drawn_triangles(&self) -> Option<(&[[f32; 3]], &[u32])> {
        let drawn = self.drawn.as_ref()?;
        Some((&drawn.positions, &drawn.indices))
    }

    /// The box the display level occupies, in the hierarchy's own coordinates.
    pub fn bounds(&mut self) -> Option<([f32; 3], [f32; 3])> {
        let (positions, ..) = self.level_mesh()?;
        let first = *positions.first()?;
        Some(positions.iter().fold((first, first), |(min, max), point| {
            (
                std::array::from_fn(|i| min[i].min(point[i])),
                std::array::from_fn(|i| max[i].max(point[i])),
            )
        }))
    }

    /// What subdividing once more would cost, from the engine's own preflight.
    ///
    /// The peak is the figure that ends a session rather than the persistent
    /// one, so both travel and the refusal is stated against the peak. `None`
    /// where the engine will not price it at all.
    pub fn subdivision_cost(&self) -> Option<SubdivisionCost> {
        let preflight = self.surface.preflight_add_level().ok()?;
        Some(SubdivisionCost {
            level: preflight.level,
            vertices: preflight.vertices,
            faces: preflight.faces,
            persistent_bytes: preflight.persistent_bytes,
            peak_bytes: preflight.peak_bytes,
        })
    }

    /// One more level, priced before it is attempted.
    ///
    /// The price is taken from the engine and the refusal is stated in the
    /// domain's own vocabulary, so what a sculptor reads is what is wrong
    /// rather than a result code. Refusing here as well as inside the engine
    /// is not belt and braces: `add_level` reports one refusal for every
    /// reason it has, and the two a sculptor can act on — the budget and the
    /// ceiling — are different sentences.
    pub fn add_level(&mut self) -> Result<u32, ModelError> {
        if let Some(cost) = self.subdivision_cost() {
            cost.within(LEVEL_BUDGET).map_err(ModelError::Conversion)?;
        }
        if self.levels().count > MultiresLevels::DEEPEST {
            return Err(ModelError::Conversion(Refusal::DepthLimit {
                levels: self.levels().count,
            }));
        }
        let level = self
            .surface
            .add_level()
            .map_err(|refused| ModelError::engine(refused.to_string()))?;
        // A level is added build-then-publish, so the count moved and so did
        // both numbers the engine keeps — but the surface this side is drawing
        // from is the old one until it is asked again.
        self.drawn = None;
        Ok(level)
    }

    /// The highest level and the detail on it, gone.
    pub fn remove_highest_level(&mut self) -> Result<(), ModelError> {
        self.surface
            .remove_highest_level()
            .map_err(|refused| ModelError::engine(refused.to_string()))?;
        self.drawn = None;
        Ok(())
    }

    pub fn set_sculpt_level(&mut self, level: u32) -> Result<(), ModelError> {
        let level = level.min(self.levels().highest());
        self.surface
            .set_sculpt_level(level)
            .map_err(ModelError::engine)
    }

    pub fn set_display_level(&mut self, level: u32) -> Result<(), ModelError> {
        let level = level.min(self.levels().highest());
        self.surface
            .set_display_level(level)
            .map_err(ModelError::engine)
    }

    // -- the gesture ---------------------------------------------------------

    /// The hierarchy as it stands, as bytes, priced before it allocates.
    ///
    /// `clay_multires_preflight_encode` answers in microseconds and is asked
    /// first on every path that serializes — the side-car and the undo record
    /// alike — because the alternative to pricing an encode is discovering its
    /// size by allocating it.
    ///
    /// Note what the preflight is and is not. It is a **budget verdict**: on
    /// the pinned engine its `persistent_bytes` reads 21,392 against a blob
    /// that turns out to be 25,448 on one fixture and 1,589,696 against
    /// 1,460,304 on another, so it is neither a ceiling nor a floor and no
    /// buffer may be sized from it. `clay_multires_serialize` sizes itself.
    pub fn bytes(&self, budget: u64) -> Result<Vec<u8>, ModelError> {
        let priced = self
            .surface
            .preflight_encode(budget)
            .map_err(ModelError::engine)?;
        if !priced.allowed {
            return Err(ModelError::engine(format!(
                "a hierarquia ocupa cerca de {} MB, além do que cabe aqui",
                priced.persistent_bytes / (1024 * 1024)
            )));
        }
        self.surface.serialize().map_err(ModelError::engine)
    }

    /// Puts the hierarchy back to bytes taken from it earlier.
    ///
    /// A whole new handle, which is the only exact restore the ABI offers and
    /// is why everything cached above is dropped with it. The revisions start
    /// again from one, so a token held across this names a numbering that no
    /// longer exists — which is exactly what [`Hierarchy::rebound`] is for.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), ModelError> {
        self.surface = Multires::deserialize(bytes).map_err(ModelError::engine)?;
        // Both lines are about the same fact: what this side was agreeing with
        // is gone. The drawn triangles were the old surface's, and only this
        // side's own generation can say that two surfaces both reading
        // `evaluated == 1` are not the same surface.
        self.drawn = None;
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    /// Records where the surface stood, if this is the first segment to reach
    /// it, and hands back the bytes a replaying segment must restore from.
    pub fn open_gesture(&mut self) -> Result<(), ModelError> {
        if self.open.is_none() {
            self.open = Some(self.bytes(0)?);
        }
        Ok(())
    }

    /// Takes the open gesture back to where it started, for a dragging verb
    /// that lays itself down again from its anchor.
    pub fn replay_from_the_anchor(&mut self) -> Result<(), ModelError> {
        let Some(bytes) = self.open.take() else {
            return Ok(());
        };
        self.restore(&bytes)?;
        self.open = Some(bytes);
        Ok(())
    }

    /// The record the gesture leaves behind, and the gesture closed.
    pub fn close_gesture(&mut self) -> Option<Vec<u8>> {
        self.open.take()
    }

    pub fn gesture_is_open(&self) -> bool {
        self.open.is_some()
    }

    // -- the pass stack ------------------------------------------------------

    /// The stack, bottom-first, as the layer row draws it.
    ///
    /// Every failure is a pass dropped from the list rather than a refused
    /// read, which is [`crate::objects`]'s rule for the same reason: this is
    /// asked once per frame off the layer summary, and a stack that refused to
    /// describe itself because one row would not answer would take the whole
    /// interface down with it.
    ///
    /// `masked` is `false` for every pass and that is the honest answer rather
    /// than a stub. The ABI reports a pass's bytes — coefficients *and* mask
    /// together — and offers no flag saying whether a mask block is stored;
    /// the only reader is per vertex. This application cannot author one
    /// either: `clay_multires_set_sculpt_layer_mask` writes a weight one
    /// vertex at a time, and the freeze this application paints is a volume
    /// rather than a per-vertex weight. So the row draws the mask badge from
    /// this field and it never lights, which is the state of things — an
    /// indicator wired to a value nobody can move is better than a control
    /// that pretends to.
    fn sculpt_layers(&self) -> Vec<MultiresSculptLayer> {
        let Ok(ids) = self.surface.sculpt_layer_ids() else {
            return Vec::new();
        };
        let mut passes: Vec<MultiresSculptLayer> = ids
            .into_iter()
            .filter_map(|id| {
                let info = self.surface.sculpt_layer_info(id).ok()?;
                Some(MultiresSculptLayer {
                    id: MultiresSculptLayerId::new(id.get()),
                    index: info.index as usize,
                    // A pass with no name of its own is empty here rather than
                    // numbered here: the number a sculptor reads is the row's
                    // position, which is `display_name`'s business and moves
                    // when the stack is reordered.
                    name: self.surface.sculpt_layer_name(id).unwrap_or_default(),
                    strength: info.strength,
                    visible: info.visible,
                    locked: info.locked,
                    masked: false,
                    coverage_vertices: info.coverage_vertices,
                    bytes: info.bytes as usize,
                })
            })
            .collect();
        // Bottom-first, by the engine's own index rather than by the order the
        // ids came back in. `MultiresState::sanitized` re-derives `index` from
        // this order, so sorting here is what makes the two agree.
        passes.sort_by_key(|pass| pass.index);
        passes
    }

    /// Which pass the next stroke would enter, or the form under them.
    pub fn active_pass(&self) -> MultiresSculptLayerId {
        self.surface
            .active_sculpt_layer()
            .map_or(MultiresSculptLayerId::BASE, |id| {
                MultiresSculptLayerId::new(id.get())
            })
    }

    /// Whether the next stamp lands on a pass rather than in the form.
    ///
    /// [`WriteDomain::Automatic`] resolved against the active pass — see
    /// [`Hierarchy::state`] for why that is the only domain this application
    /// sends.
    pub fn stamps_into_a_pass(&self) -> bool {
        !self.active_pass().is_base()
    }

    /// Acts on the stack.
    ///
    /// Every operation is the engine's, including the two a host is most
    /// tempted to do itself. A reorder is `clay_multires_move_sculpt_layer`
    /// rather than a `Vec::rotate`, because the stack the interface draws is
    /// read back from the engine and a host-side order would last exactly
    /// until the next frame; a merge is `clay_multires_merge_sculpt_layer_down`
    /// rather than the obvious arithmetic, because the obvious arithmetic
    /// divides by the lower pass's strength and zero is a state one slider
    /// reaches.
    pub fn apply_sculpt_layer_op(&mut self, op: &MultiresSculptLayerOp) -> Result<(), ModelError> {
        use MultiresSculptLayerOp as Op;

        let engine = |id: MultiresSculptLayerId| claycore::SculptLayerId::from_raw(id.raw());
        let refused = |refusal: claycore::MultiresRefusal| ModelError::engine(refusal.to_string());
        match op {
            Op::Add { name } => {
                let name = name.trim();
                self.surface
                    .add_sculpt_layer((!name.is_empty()).then_some(name))
                    .map(|_| ())
                    .map_err(refused)?;
            }
            Op::Rename { id, name } => self
                .surface
                .rename_sculpt_layer(engine(*id), name)
                .map_err(refused)?,
            Op::SetStrength { id, strength } => self
                .surface
                .set_sculpt_layer_strength(engine(*id), strength.clamp(0.0, 1.0))
                .map_err(refused)?,
            Op::SetVisible { id, visible } => self
                .surface
                .set_sculpt_layer_visible(engine(*id), *visible)
                .map_err(refused)?,
            Op::SetLocked { id, locked } => self
                .surface
                .set_sculpt_layer_locked(engine(*id), *locked)
                .map_err(refused)?,
            Op::SetActive { id } => self
                .surface
                .set_active_sculpt_layer(engine(*id))
                .map_err(refused)?,
            Op::Move { id, to } => self
                .surface
                .move_sculpt_layer(engine(*id), *to)
                .map_err(refused)?,
            Op::Remove { id } => self
                .surface
                .remove_sculpt_layer(engine(*id))
                .map_err(refused)?,
            Op::MergeDown { id } => self
                .surface
                .merge_sculpt_layer_down(engine(*id))
                .map_err(refused)?,
            Op::BakeToBase { id } => self
                .surface
                .bake_sculpt_layer_to_base(engine(*id))
                .map_err(refused)?,
            Op::Compact => self
                .surface
                .compact_sculpt_layers()
                .map_err(ModelError::engine)?,
        }
        // The evaluated surface may have moved under the triangles this side
        // is holding — a strength, a visibility, a removal and a bake all do —
        // so the copy is given up rather than reasoned about per operation.
        // Dropping it on a rename costs one level-mesh copy and gets the rule
        // down to one line.
        self.drawn = None;
        Ok(())
    }
}

impl std::fmt::Debug for Hierarchy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hierarchy")
            .field("levels", &self.levels())
            .finish_non_exhaustive()
    }
}

/// What a refused build says about the mesh it was handed.
///
/// The engine refuses rather than repairs — a conversion that quietly welded a
/// face would change retopology somebody paid for without saying so — and the
/// two faults it names are the two a sculptor can go and mend. Everything else
/// arrives as the engine's own sentence.
pub fn cage_fault(error: claycore::MultiresError) -> Option<CageFault> {
    match error {
        claycore::MultiresError::NonManifold => Some(CageFault::NonManifold),
        claycore::MultiresError::DegenerateFace => Some(CageFault::DegenerateFace),
        _ => None,
    }
}

// -- the side-car ------------------------------------------------------------

/// Where the hierarchies live for a document at `path`.
pub fn sidecar_for(path: &std::path::Path) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".multires");
    path.with_file_name(name)
}

/// The first line of the file, so a later format can be told from this one.
const HEADER: &[u8] = b"clayspace-multires 1\n";

/// One hierarchy as the side-car keeps it.
pub struct Saved {
    /// Which row it belongs to, counted from the bottom of the stack.
    ///
    /// A **position** and not a `LayerKey`, which is the one identity choice
    /// here worth arguing. A key is minted at run time and is not written into
    /// the `.clayspace` at all, so a reopened document mints fresh ones and a
    /// side-car keyed by them would match nothing. A name would survive, but
    /// names are not unique upstream and are not made unique here — the
    /// document's own rename test says so. What does survive exactly is stack
    /// order: `clay_document_layer_ids` answers in it, it is evaluation order
    /// so the format has to preserve it, and it is precisely what a reopened
    /// document mints its keys from.
    pub position: usize,
    pub bytes: Vec<u8>,
}

/// Writes every hierarchy the document holds, one after another.
///
/// Each record is an ASCII line naming the layer and the length, then that
/// many raw bytes. Text for the parts a person may have to read and binary for
/// the part nobody can, which is the same division [`crate::objects`] makes for
/// the same reason — one shape, one writer, and a serialisation dependency is
/// something a licence audit carries forever. The line grows only at its tail,
/// so a build that predates a field reads the fields it knows and stops.
pub fn write_hierarchies(path: &std::path::Path, hierarchies: &[Saved]) -> std::io::Result<()> {
    // Removed rather than left standing when there is nothing to write. A
    // stale side-car beside a document that no longer holds a hierarchy would
    // promote a mesh layer back into one on the next open, using a blob for a
    // cage that has since changed.
    if hierarchies.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        };
    }
    let mut out = Vec::from(HEADER);
    for saved in hierarchies {
        out.extend_from_slice(format!("{} {}\n", saved.position, saved.bytes.len()).as_bytes());
        out.extend_from_slice(&saved.bytes);
    }
    std::fs::write(path, out)
}

/// Reads the hierarchies back, dropping any record this build cannot make
/// sense of.
///
/// A truncated record drops **that layer** and keeps the rest, which is
/// [`crate::objects::read_table`]'s rule and is right for the same reason: one
/// unreadable row should not cost a document. What is different here is what a
/// dropped row means — a lost hierarchy is a lost sculpt rather than lost
/// bookkeeping — so the caller is told which layers it did not get rather than
/// being handed a shorter list to notice for itself.
pub fn read_hierarchies(path: &std::path::Path) -> Vec<Saved> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let Some(mut rest) = bytes.strip_prefix(HEADER) else {
        return Vec::new();
    };
    let mut saved = Vec::new();
    while !rest.is_empty() {
        let Some(newline) = rest.iter().position(|byte| *byte == b'\n') else {
            break;
        };
        let Ok(line) = std::str::from_utf8(&rest[..newline]) else {
            break;
        };
        let mut fields = line.split_whitespace();
        let (Some(position), Some(length)) = (fields.next(), fields.next()) else {
            break;
        };
        let (Ok(position), Ok(length)) = (position.parse::<usize>(), length.parse::<usize>())
        else {
            break;
        };
        let body = &rest[newline + 1..];
        if body.len() < length {
            // The tail is short, so this record and everything that would have
            // followed it are gone. Stopping rather than guessing.
            break;
        }
        saved.push(Saved {
            position,
            bytes: body[..length].to_vec(),
        });
        rest = &body[length..];
    }
    saved
}
