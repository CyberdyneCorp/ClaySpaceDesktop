//! The document, as the domain sees it.
//!
//! Implements [`SculptModel`] over a real ClayCore document, holding the brick
//! cache that makes a dab cost what it touched rather than what the model
//! holds.

use claycore::{
    Blend, BrickCache, BrickConfig, BrickKey, BrushParams, BrushShape, ClayError, Document,
    Falloff, ImportBudget, Item, LayerId, Mask, Mesh, MeshLayerDesc, MeshParams, Mesher, NodeId,
    Op, StrokePreset, VolumeParams,
};
use clayspace_model::{
    Alpha, Armature, ArmatureModel, BlendProfile, BrushSettings, Combine, CombineSettings, Cost,
    Direction, DocumentModel, EditOutcome, ExchangeModel, ExportMesher, ExportSettings,
    ExtrudeSettings, Format, GestureSample, GizmoDrag, GizmoHandle, GizmoMode, HistoryState,
    ImportAs, ImportSettings, LatticeModel, LatticeState, LayerKey, LayerSummary, MaskModel,
    MaskOp, MaskState, ModelError, NodeIndex, OpenError, Protection, Refusal, Representation,
    Scene, SceneModel, SceneNode, SceneStats, SculptModel, SkinSettings, SmoothBlur, ToolKind,
    VoxelDisplay,
};

use crate::backend::{BackendPolicy, Operation};

/// The engine's op for a combine operation.
///
/// Exhaustive rather than defaulted: an unlisted arm falling through to
/// `Op::Add` is exactly the bug the tool table carries a note about, where a
/// planing tool deposited spheres and nothing said so.
fn engine_op(op: Combine) -> Op {
    match op {
        Combine::Add => Op::Add,
        Combine::Subtract => Op::Subtract,
        Combine::Intersect => Op::Intersect,
        Combine::Paint => Op::Paint,
        Combine::Groove => Op::Groove,
        Combine::Tongue => Op::Tongue,
        Combine::Pipe => Op::Pipe,
        Combine::Engrave => Op::Engrave,
        Combine::Emboss => Op::Emboss,
        Combine::Inset => Op::Inset,
        Combine::Shell => Op::Shell,
        Combine::Replace => Op::Replace,
        Combine::Relief => Op::Relief,
        Combine::Incise => Op::Incise,
    }
}

/// A unit vector pointing away from the origin through `point`.
///
/// Stands in for the surface normal where none is to hand: on a form built out
/// from the origin it is close enough to orient a stamp's plane, and it is
/// never zero-length, which is what the voxel carve refuses.
fn outward(point: [f32; 3]) -> [f32; 3] {
    let length = (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt();
    if length < 1e-6 {
        return [0.0, 0.0, 1.0];
    }
    [point[0] / length, point[1] / length, point[2] / length]
}

fn engine_blend(profile: BlendProfile) -> Blend {
    match profile {
        BlendProfile::Hard => Blend::Hard,
        BlendProfile::Quadratic => Blend::Quadratic,
        BlendProfile::Cubic => Blend::Cubic,
        BlendProfile::Circular => Blend::Circular,
        BlendProfile::Chamfer => Blend::Chamfer,
    }
}

/// One chunk's triangles, as the viewport wants them.
///
/// Indices are relative to this chunk's own first vertex, so a chunk can be
/// replaced or dropped without touching its neighbours' — which is what the
/// engine's ranges promise: a voxel face belongs to exactly one cell in
/// exactly one chunk, so there is nothing to weld across a seam.
#[derive(Debug, Default)]
struct ChunkGeometry {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

/// A layer the document holds, and what it is made of.
struct Layer {
    id: LayerId,
    /// A stable handle the interface uses. Engine ids are not guaranteed to
    /// survive an edit, so the interface is given one that is.
    key: LayerKey,
    /// What the interface shows, which a rename changes.
    name: String,
    /// What the *document* calls this layer.
    ///
    /// Equal to `name` since ClayCore 0.30.0 gave the ABI a rename (#92), and
    /// kept as its own field because it is a different thing: it is the only
    /// handle `clay_document_voxel_layer` takes, and a name is not a key
    /// anything upstream enforces. Renaming writes both, so a renamed voxel
    /// layer keeps its grid.
    engine_name: String,
    representation: Representation,
    /// Whether a mesh row's triangles have arrived.
    ///
    /// A mesh layer is recorded before its mesh is attached, so the rest of
    /// the application can talk about it; only `attach_reference` makes it
    /// real. Always true for the other two, which are editable from nothing.
    carries_geometry: bool,
    visible: bool,
    protection: Protection,
    intensity: u8,
    /// Where this layer's grid is, in world space. `None` for the other two
    /// representations and for an empty grid.
    ///
    /// Cached for the reason the pass stack is: reading a grid needs a mutable
    /// borrow of the document and `bounds` takes a shared one. Without it the
    /// question had no answer at all — `layer_bounds` reports a layer's SDF
    /// extent and a voxel layer has no SDF content, so Frame All on a sculpted
    /// grid framed the default box and the conversion panel measured the
    /// region as zero.
    voxel_bounds: Option<([f32; 3], [f32; 3])>,
    /// This layer's grid as triangles, one entry per chunk.
    ///
    /// Kept per chunk so an edit costs the edit. Meshing a grid whole after
    /// every stroke is what it used to do, and it does not scale: measured on
    /// a 0.01 grid, one 3.2 ms dab cost **309 ms** to re-mesh, against a 50 ms
    /// budget, and rising with the sculpt. Draining the engine's own
    /// dirty-chunk set and re-meshing only those costs 3.3 ms and does not
    /// rise.
    voxel_chunks: std::collections::BTreeMap<[i32; 3], ChunkGeometry>,
    /// The recorded passes on this layer, bottom-up.
    ///
    /// Cached rather than read on demand, for the same reason the armature
    /// tree is: reading a grid's stack needs a mutable borrow of the document
    /// and `scene` takes a shared one. Refreshed by
    /// [`ClayDocument::refresh_sculpt_layers`] after anything that could change
    /// it, so a stale stack is a missed call rather than a silent drift.
    sculpt_layers: Vec<clayspace_model::SculptLayer>,
}

impl Layer {
    fn summary(&self) -> LayerSummary {
        LayerSummary {
            key: self.key,
            name: self.name.clone(),
            representation: self.representation,
            visible: self.visible,
            protection: self.protection,
            intensity: self.intensity,
            sculpt_layers: self.sculpt_layers.clone(),
        }
    }

    /// Whether an edit may touch it: shown, not ghosted, not locked.
    fn editable(&self) -> bool {
        self.visible && self.protection.is_editable()
    }
}

/// A ClayCore document driven by the domain's vocabulary.
/// One mesh gesture, and where it sits against the engine's own history.
struct MeshGesture {
    layer: LayerKey,
    deltas: claycore::MeshDeltas,
    /// The engine's undo depth when this was recorded. See `mesh_undo`.
    engine_depth: usize,
}

pub struct ClayDocument {
    document: Document,
    layers: Vec<Layer>,
    active: usize,
    cache: BrickCache,
    policy: BackendPolicy,
    /// Bricks dirtied since the viewport last caught up.
    dirty: Vec<BrickKey>,
    stats: SceneStats,
    /// Chunk keys re-meshed by the last refresh.
    ///
    /// A measurement rather than bookkeeping: it is what says an edit costs
    /// the edit, and it is what a test can assert on without timing anything.
    meshed_chunks: usize,
    /// The gesture being previewed on a mesh layer, and what it has moved.
    ///
    /// A dragging verb is laid down again from its anchor on every segment, so
    /// what the last segment did has to be taken back first — this is the
    /// record that takes it back. Promoted to the undo stack when the gesture
    /// ends, so a drag is still one undo however many segments drew it.
    live_mesh: Option<(LayerKey, claycore::MeshDeltas)>,
    /// Whether a gesture is open and should be previewed rather than banked.
    previewing: bool,
    /// Bumped by every preview, so the viewport knows to look again.
    ///
    /// A preview banks nothing, so nothing else about the document changes and
    /// the number the viewport watches would sit still while the drag was
    /// visibly moving the surface.
    live_generation: u64,
    /// Triangles and vertices the *carried* layers handed the viewport.
    ///
    /// Kept apart from `stats` because the two are recorded at different
    /// moments by different parts of the viewport — the surface cache reports
    /// after it meshes, the carried layers when they are assembled — and a
    /// single field would have each overwrite the other's contribution.
    carried: (usize, usize),
    /// Bricks the surface occupies, refreshed with the stats.
    ///
    /// Kept because the detail policy needs a size and asking the cache for
    /// the whole key list every frame would cost more than the policy saves.
    surface_brick_count: usize,
    /// The cage around the form, while one is up.
    ///
    /// Held here rather than in a ViewModel because the *offsets* are the
    /// engine's business — the interface drags a point in the world and this
    /// is what knows the box that point belongs to.
    lattice: Option<Cage>,
    /// Which picture of a voxel layer the viewport draws, and how much the
    /// occupancy is filtered before the smooth one is taken.
    ///
    /// Display only: nothing here changes a cell, and the engine keeps it an
    /// argument rather than grid state for exactly that reason.
    voxel_display: VoxelDisplay,
    voxel_blur: SmoothBlur,
    /// The smooth mesh of each voxel layer, while that is the picture being
    /// drawn.
    ///
    /// Whole-grid and held apart from `voxel_chunks`, because it is not
    /// chunked and cannot be: `clay_voxel_mesh_chunks` is the greedy mesher
    /// alone. Rebuilt when a gesture settles rather than while it is made.
    /// Keyed by layer, and carrying the grid's change count at the moment it
    /// was built — so a frame in which nothing moved costs one comparison
    /// rather than a whole-grid re-mesh.
    voxel_smooth: std::collections::BTreeMap<LayerKey, (u64, ChunkGeometry)>,
    /// The tendril a snakehook gesture is pulling, while one is open.
    ///
    /// Held so the segments of one drag *grow* a single curve rather than
    /// leaving a trail of them: a segment that added its own item restarted
    /// the taper, which beaded the tendril into a string of spheres.
    live_hook: Option<(LayerId, claycore::NodeId)>,
    /// A mask the tools consult, when one has been painted.
    mask: Option<Mask>,
    /// Changes whenever the cage does — its points, its selection or its
    /// resolution.
    cage_revision: u64,
    /// Changes whenever the mask does.
    ///
    /// A mask stroke moves no clay and dirties no brick, which is right — it
    /// is state the *next* stroke reads — but it does change what the viewport
    /// should be drawing, and the surface's own revision cannot say so. The
    /// counter is what lets the frozen region be shown without re-sampling
    /// every vertex on every frame.
    mask_revision: u64,
    /// The sculptor for the mesh layer being sculpted, and which layer it is.
    ///
    /// One at a time rather than one per layer: the adjacency is the expensive
    /// part and a sculptor is only useful for the layer under the pointer, so
    /// holding every mesh layer's would pay for meshes nobody is touching.
    /// Built on the first stroke against a layer and dropped when the active
    /// mesh layer changes.
    /// In a cell because a *pick* needs it and a pick is a question.
    ///
    /// The sculptor answers a raycast from its own tree and may refit it while
    /// doing so, which is a mutation — but `SculptModel::pick` takes `&self`,
    /// and widening that so every caller of a question must hold a mutable
    /// borrow would be the tail wagging the dog. Casting the borrow away was
    /// the other option and `forbid(unsafe_code)` refused it, correctly: the
    /// C call takes a non-const sculptor because it really may write.
    mesh_sculptor: std::cell::RefCell<Option<(LayerKey, claycore::MeshSculptor)>>,
    /// Mesh gestures, newest last, and the redo side of the same.
    ///
    /// A second history beside the engine's, which the design deferred and
    /// this is the revisit of. A vertex displacement is destructive and is not
    /// an edit item, so the document holds nothing to take back — measured,
    /// the engine's undo depth is the same before and after a mesh stroke. The
    /// engine does offer the machinery: `clay_mesh_deltas` reverts a gesture
    /// bit exactly.
    ///
    /// The two histories interleave by *depth*. Each record remembers the
    /// engine's undo depth when it was made, and an undo reverts the mesh
    /// gesture only when that depth still matches — any engine edit since has
    /// raised it, so the engine's entry is the more recent one and goes first.
    /// Undoing that engine entry lowers the depth back, and the mesh gesture
    /// becomes the most recent again.
    mesh_undo: Vec<MeshGesture>,
    mesh_redo: Vec<MeshGesture>,
    /// The mirror currently set on the active layer, so it is only rewritten
    /// when it actually changes.
    symmetry: [bool; 3],
    /// How the next SDF edit combines with what is under it.
    combine: CombineSettings,
    /// The one alpha stamp loaded, which every brush with `alpha` set uses.
    alpha: Option<Alpha>,
    /// Whether a pass is being recorded on the active grid.
    ///
    /// Mirrored here rather than read back per frame: the engine answers per
    /// *grid* and the shell asks about the document, so switching to a layer
    /// with no grid at all would otherwise need a borrow to say "no".
    recording_pass: bool,
    /// Hands out layer keys. Monotone, so a key is never reused for a
    /// different layer after a removal.
    next_key: u64,
    selected: Option<LayerKey>,
    /// The armature on the active layer: which node carries it, and the tree.
    ///
    /// The tree is held here because the engine's parent array has no getter —
    /// positions and radii read back, the topology does not. So this is the
    /// record and the engine is written from it.
    /// The rig: its layer, the nodes it placed, and the tree behind them.
    ///
    /// One node since ClayCore 0.30.0 (#99), because the signs made the rig a
    /// single item again. It stays a list because rewriting is defined over
    /// whatever was placed: when a negative sphere was a second subtractive
    /// item, tracking only the armature's own node left the cutters behind on
    /// each rewrite, and an edited rig accumulated a subtraction per edit.
    armature: Option<(LayerId, Vec<NodeId>, Armature)>,
    /// The box the placed armature last occupied.
    ///
    /// Kept because an edit that *shrinks* a rig leaves surface behind
    /// otherwise: the new node's own region is refilled when it is placed, and
    /// the bricks the old one used are never told anything changed. Removing an
    /// arm left the arm on screen.
    armature_bounds: Option<([f32; 3], [f32; 3])>,
    skin: SkinSettings,
}

impl ClayDocument {
    /// Builds a document with one SDF layer holding a starting form.
    pub fn new(policy: BackendPolicy) -> Result<Self, ModelError> {
        let mut document = Document::new().map_err(ModelError::engine)?;
        let id = document
            .add_sdf_layer("Forma")
            .map_err(ModelError::engine)?;
        // X, as the design asks for.
        //
        // This was off for the whole of 0.26 and 0.27: `clay_set_layer_mirror`
        // stored the plane, but per-item participation defaulted to *excluded*,
        // so the sequence every host writes — set the mirror, add items —
        // mirrored nothing, and a sculptor would have watched half of every
        // stroke vanish. ClayCore 0.28.0 makes participation default to
        // mirrored (#60), and `claycore_repros.rs` is what noticed.
        //
        // Set before undo starts recording either way: the starting mirror is
        // part of making the document, not something a user did.
        let symmetry = [true, false, false];
        document
            .set_layer_mirror(id, symmetry, 0.0)
            .map_err(ModelError::engine)?;
        document.enable_undo().map_err(ModelError::engine)?;

        let cache = BrickCache::new(BrickConfig {
            // 8-cell bricks. 16 was tried: it covers the surface in a third
            // as many keys but each holds eight times the cells, and a dilated
            // dirty set then meshes more cells overall — 64 ms against 39 ms
            // on the same edit.
            dim: 8,
            voxel_size: Self::VOXEL_SIZE,
            band_voxels: 3,
            memory_budget: Some(512 * 1024 * 1024),
            colors: false,
        })
        .map_err(ModelError::engine)?;

        let mut model = Self {
            document,
            layers: vec![Layer {
                id,
                key: LayerKey(1),
                name: "Forma".to_string(),
                engine_name: "Forma".to_string(),
                representation: Representation::Sdf,
                carries_geometry: true,
                visible: true,
                protection: Protection::default(),
                intensity: 100,
                voxel_bounds: None,
                voxel_chunks: std::collections::BTreeMap::new(),
                sculpt_layers: Vec::new(),
            }],
            active: 0,
            cache,
            policy,
            combine: CombineSettings::for_strokes(),
            alpha: None,
            recording_pass: false,
            dirty: Vec::new(),
            stats: SceneStats::default(),
            carried: (0, 0),
            live_mesh: None,
            previewing: false,
            live_generation: 0,
            meshed_chunks: 0,
            surface_brick_count: 0,
            mesh_sculptor: std::cell::RefCell::new(None),
            mesh_undo: Vec::new(),
            mesh_redo: Vec::new(),
            live_hook: None,
            lattice: None,
            voxel_display: VoxelDisplay::default(),
            voxel_blur: SmoothBlur::default(),
            voxel_smooth: std::collections::BTreeMap::new(),
            mask: None,
            cage_revision: 0,
            mask_revision: 0,
            symmetry,
            next_key: 2,
            selected: None,
            armature: None,
            armature_bounds: None,
            skin: SkinSettings::default(),
        };
        model.refresh_stats();
        Ok(model)
    }

    /// Places a sphere of the given radius in the first layer.
    ///
    /// Separate from [`ClayDocument::with_starting_form`] because the
    /// benchmark's reference scenes differ only in scale, and building them
    /// through the same path as the application keeps them honest.
    pub fn add_starting_sphere(&mut self, radius: f32) -> Result<(), ModelError> {
        let layer = self.layers[0].id;
        let body = Item::sphere(radius).map_err(ModelError::engine)?;
        self.document
            .add_item(layer, &body)
            .map_err(ModelError::engine)?;
        self.refill(layer, &[])?;
        self.refresh_stats();
        Ok(())
    }

    /// Places a starting sphere so there is something to sculpt on.
    pub fn with_starting_form(mut self) -> Result<Self, ModelError> {
        let layer = self.layers[0].id;
        let body = Item::sphere(1.0).map_err(ModelError::engine)?;
        self.document
            .add_item(layer, &body)
            .map_err(ModelError::engine)?;
        self.refill(layer, &[])?;
        self.refresh_stats();
        Ok(self)
    }

    /// The engine document, for the viewport's own meshing.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The brick cache the viewport re-meshes from.
    /// Builds the mips covering the surface, and says how many are ready.
    ///
    /// The half of level of detail that is ours to do. A coarse brick is
    /// buildable only when all eight of its children are evaluated *and*
    /// clean, so this is called when a gesture ends rather than during one —
    /// dirtying any child drops its mip, and rebuilding them mid-stroke would
    /// be work thrown away on the next sample.
    ///
    /// What consumes them is [`ClayDocument::drawable_coarse_keys`], since
    /// ClayCore 0.30.0 gave the meshing call a level (#93). Building them here
    /// is still what makes a coarse surface available the moment the camera
    /// asks for one.
    pub fn build_mips(&mut self) -> Result<usize, ModelError> {
        let coarse = self.coarse_keys().map_err(ModelError::engine)?;

        let mut built = 0;
        for key in coarse {
            // `false` is an ordinary "not yet" — some child is dirty or
            // unevaluated — rather than a failure, and is the common answer
            // while a stroke is still settling.
            if self.cache.build_mip(key).map_err(ModelError::engine)? {
                built += 1;
            }
        }
        Ok(built)
    }

    /// Whether a coarse region has a mip to draw.
    pub fn coarse_lod(&self, coarse_key: BrickKey) -> Result<i32, ModelError> {
        self.cache
            .current_lod(coarse_key)
            .map_err(ModelError::engine)
    }

    /// The coarse keys covering the surface, deduplicated.
    ///
    /// Each coarse brick covers a 2×2×2 block, so these are the surface's fine
    /// keys halved — eight of them map to one, hence the dedup.
    fn coarse_keys(&self) -> Result<Vec<BrickKey>, ClayError> {
        let mut coarse: Vec<BrickKey> = self
            .cache
            .surface_bricks()?
            .iter()
            .map(|key| {
                [
                    key[0].div_euclid(2),
                    key[1].div_euclid(2),
                    key[2].div_euclid(2),
                ]
            })
            .collect();
        coarse.sort_unstable();
        coarse.dedup();
        Ok(coarse)
    }

    /// The coarse keys that actually have a mip, ready to be meshed at level 1.
    ///
    /// Filtered rather than handed over whole, because meshing a level refuses
    /// a coarse key with no valid mip rather than skipping it: one child left
    /// dirty by the last stroke would otherwise fail the whole coarse surface.
    /// A short list is an ordinary answer — it means the rest of the surface
    /// is only available at full resolution.
    pub fn drawable_coarse_keys(&self) -> Result<Vec<BrickKey>, ClayError> {
        let mut drawable = Vec::new();
        for key in self.coarse_keys()? {
            if self.cache.current_lod(key)? == 1 {
                drawable.push(key);
            }
        }
        Ok(drawable)
    }

    /// How many bricks the surface currently occupies.
    ///
    /// The size input to the detail policy, which never coarsens a model small
    /// enough to mesh inside a frame anyway.
    pub fn surface_brick_count(&self) -> usize {
        self.surface_brick_count
    }

    /// The cache, for the few callers that need to build a mip.
    pub fn cache_mut(&mut self) -> &mut BrickCache {
        &mut self.cache
    }

    pub fn cache(&self) -> &BrickCache {
        &self.cache
    }

    /// Keys dirtied since the last call, cleared as they are handed over.
    ///
    /// The viewport meshes exactly these and patches their ranges, which is
    /// what keeps a dab's cost proportional to what it touched.
    pub fn take_dirty_keys(&mut self) -> Vec<BrickKey> {
        std::mem::take(&mut self.dirty)
    }

    pub fn policy(&self) -> &BackendPolicy {
        &self.policy
    }

    pub fn policy_mut(&mut self) -> &mut BackendPolicy {
        &mut self.policy
    }

    /// Adds a voxel layer and makes it active.
    pub fn add_voxel_layer(&mut self, name: &str, voxel_size: f32) -> Result<(), ModelError> {
        // Through the document, so the grid is *in* the document.
        //
        // This used to make an SDF layer and keep a standalone `VoxelGrid`
        // beside it, on the grounds that borrowing the document's grid would
        // hold the document for as long as the layer lived. It gave the tools
        // the same behaviour and cost the sculptor their work: the grid was
        // never part of the document, so nothing voxel survived a save, and
        // the engine reported the layer as SDF because that is what it was.
        // The borrow is taken per stroke instead, which is short enough not to
        // fight anything.
        let (id, _) = self
            .document
            .add_voxel_layer(name, voxel_size)
            .map_err(ModelError::engine)?;
        let key = self.take_key();
        self.layers.push(Layer {
            id,
            key,
            name: name.to_string(),
            engine_name: name.to_string(),
            representation: Representation::Voxel,
            carries_geometry: true,
            visible: true,
            protection: Protection::default(),
            intensity: 100,
            voxel_bounds: None,
            voxel_chunks: std::collections::BTreeMap::new(),
            sculpt_layers: Vec::new(),
        });
        self.active = self.layers.len() - 1;
        Ok(())
    }

    fn take_key(&mut self) -> LayerKey {
        let key = LayerKey(self.next_key);
        self.next_key += 1;
        key
    }

    fn index_of(&self, key: LayerKey) -> Result<usize, ModelError> {
        self.layers
            .iter()
            .position(|layer| layer.key == key)
            .ok_or_else(|| ModelError::engine("that layer is no longer in the document"))
    }

    fn active_layer(&self) -> &Layer {
        &self.layers[self.active]
    }

    /// Refills the cache for what an edit reached, recording exactly which
    /// keys were dirty.
    ///
    /// Marking by *node* rather than by layer is what keeps this bounded. A
    /// layer's extent is the union of everything in it, which for content
    /// spread far apart spans more bricks than any cache can hold — the engine
    /// refuses such a region rather than attempting it, and rightly.
    ///
    /// The dirty set comes from the cache's own drain, not from diffing its
    /// surface bricks before and after. The first version diffed, which after
    /// the initial fill finds nothing new and so fell back to re-meshing every
    /// surface brick: 1043 keys per dab instead of the influence bound, and a
    /// 267 ms dab against a 50 ms budget.
    /// Refills the cache for a bounded box of world space.
    ///
    /// For edits the engine reports as a count rather than as nodes — the
    /// surface move is the one — where marking by layer would be correct but
    /// ruinous. `Mover` did exactly that: every segment of a drag re-meshed
    /// the whole surface, 5.6 seconds a segment against a 50 ms budget.
    fn refill_region(&mut self, min: [f32; 3], max: [f32; 3]) -> Result<(), ModelError> {
        self.cache
            .mark_dirty(min, max)
            .map_err(ModelError::engine)?;
        self.drain_dirty()
    }

    fn refill(&mut self, layer: LayerId, nodes: &[NodeId]) -> Result<(), ModelError> {
        if nodes.is_empty() {
            self.cache
                .mark_dirty_layer(&self.document, layer)
                .map_err(ModelError::engine)?;
        } else {
            self.cache
                .mark_dirty_nodes(&self.document, layer, nodes)
                .map_err(ModelError::engine)?;
        }

        self.drain_dirty()
    }

    /// How many bytes a voxel cell costs, for the budget refusal.
    ///
    /// A palette index and the bookkeeping around it. Approximate on purpose:
    /// it decides whether to refuse a resolution, not what to allocate.
    const BYTES_PER_CELL: u64 = 4;

    /// The region a conversion would cover, and what it would cost there.
    ///
    /// Asked before the conversion runs, so the interface can state the losses
    /// while the sculptor is still choosing the resolution. `None` where the
    /// source has no bounds and the direction needs a region — which is itself
    /// the answer, and `convert_layer` refuses with it.
    pub fn conversion_cost(&self, direction: Direction, cell_size: f32) -> Option<Cost> {
        let extent = match self.bounds() {
            Some((min, max)) => std::array::from_fn(|i| (max[i] - min[i]).max(0.0)),
            None if direction.needs_region() => return None,
            None => [0.0; 3],
        };
        Some(Cost::of(direction, cell_size, extent))
    }

    /// Crosses the active layer to another representation, as a new layer.
    ///
    /// A new layer rather than a replacement, always. One direction discards
    /// the procedural history and the other quantises onto a lattice, so the
    /// source staying where it is *is* the way back: undo works until the
    /// session ends, and a layer works after it.
    ///
    /// `blur` filters the lattice on the way out of a grid — 0 keeps the
    /// terracing and loses nothing, 1 is what an organic sculpt wants.
    pub fn convert_layer(
        &mut self,
        direction: Direction,
        cell_size: f32,
        blur: i32,
    ) -> Result<LayerKey, ModelError> {
        let source = self.active_layer();
        if source.representation != direction.from() {
            // Not a tool refusal — there is no tool here — so it is stated as
            // what it is: this crossing starts somewhere else.
            return Err(ModelError::Conversion(Refusal::WrongSource {
                needs: direction.from(),
                active: source.representation,
            }));
        }
        let Some(cost) = self.conversion_cost(direction, cell_size) else {
            return Err(ModelError::Conversion(Refusal::UnboundedRegion));
        };
        cost.within(
            self.cache
                .stats()
                .ok()
                .and_then(|s| s.memory_budget)
                .unwrap_or(u64::MAX),
            Self::BYTES_PER_CELL,
        )
        .map_err(ModelError::Conversion)?;

        let name = format!("{} · {}", source.name, direction.to().label());
        // Bracketed, because a crossing is several engine edits — the layer,
        // then whatever fills it — and a sculptor asked for one thing. Without
        // the group, undo took back the filling and left the empty layer
        // standing, which is the shape `a_conversion_is_one_undo_step` caught.
        self.document
            .begin_undo_group()
            .map_err(ModelError::engine)?;
        let made = match direction {
            Direction::SdfToVoxel => self.rasterize_to_voxels(&name, cell_size),
            Direction::VoxelToSdf => self.voxels_to_sdf(&name, blur),
            Direction::MeshToVoxel => self.mesh_to_voxels(&name, cell_size),
            Direction::MeshToSdf => self.mesh_to_sdf(&name),
            Direction::SdfToMesh => self.sdf_to_mesh(&name, cell_size),
            Direction::VoxelToMesh => self.voxels_to_mesh(&name),
        };
        // Closed on the failing path too: a group left open swallows every
        // edit after it into one undo step, which is a worse bug than the one
        // that opened it.
        let closed = self.document.end_undo_group().map_err(ModelError::engine);
        let made = made?;
        closed?;
        Ok(made)
    }

    fn rasterize_to_voxels(&mut self, name: &str, cell_size: f32) -> Result<LayerKey, ModelError> {
        let Some((min, max)) = self.bounds() else {
            return Err(ModelError::Conversion(Refusal::UnboundedRegion));
        };
        self.add_voxel_layer(name, cell_size)?;
        let key = self.active_layer().key;
        let engine_name = self.active_layer().engine_name.clone();
        self.document
            .rasterize_into_voxel_layer(&engine_name, (min, max))
            .map_err(ModelError::engine)?;
        self.after_conversion(key)
    }

    fn voxels_to_sdf(&mut self, name: &str, blur: i32) -> Result<LayerKey, ModelError> {
        let engine_name = self.active_layer().engine_name.clone();
        // Scoped rather than dropped: the grid carries an exclusive borrow of
        // the document, and the conversion below needs the document back.
        let occupied = {
            let (_, grid) = self
                .document
                .voxel_layer(&engine_name)
                .map_err(ModelError::engine)?;
            grid.occupied_count().map_err(ModelError::engine)?
        };
        if occupied == 0 {
            return Err(ModelError::Conversion(Refusal::SourceEmpty));
        }
        // One volume item per palette entry, which is what carries the colour
        // across: a distance field has none in it.
        let layer = self
            .document
            .voxel_layer_to_sdf_layer(&engine_name, name, blur)
            .map_err(ModelError::engine)?;
        let key = self.adopt_engine_layer(layer, name, Representation::Sdf)?;
        self.after_conversion(key)
    }

    fn mesh_to_voxels(&mut self, name: &str, cell_size: f32) -> Result<LayerKey, ModelError> {
        let Some((min, max)) = self.bounds() else {
            return Err(ModelError::Conversion(Refusal::UnboundedRegion));
        };
        let engine_name = self.active_layer().engine_name.clone();
        self.add_voxel_layer(name, cell_size)?;
        let key = self.active_layer().key;
        let target = self.active_layer().engine_name.clone();
        self.document
            .rasterize_mesh_into_voxel_layer(&engine_name, &target, (min, max))
            .map_err(ModelError::engine)?;
        self.after_conversion(key)
    }

    fn mesh_to_sdf(&mut self, name: &str) -> Result<LayerKey, ModelError> {
        let engine_name = self.active_layer().engine_name.clone();
        let layer = self
            .document
            .mesh_layer_to_sdf_layer(&engine_name, name, VolumeParams::default())
            .map_err(ModelError::engine)?;
        let key = self.adopt_engine_layer(layer, name, Representation::Sdf)?;
        self.after_conversion(key)
    }

    /// Marches the active layer's field into triangles, on a layer of its own.
    ///
    /// The engine meshes a *document*, not a layer — `clay_document_mesh` takes
    /// no layer id and there is no layer-scoped mesher. So the other SDF layers
    /// are hidden across the call and put back afterwards. That is exact rather
    /// than approximate: the engine states that a hidden layer contributes
    /// nothing to the field and that showing it again restores the field
    /// exactly, and it is measured — the starting sphere alone meshes to 57,650
    /// vertices bounded at ±1, the same document with a blob on a second layer
    /// to 44,462 bounded past 1.3, and restoring gives the first answer back.
    ///
    /// Only SDF layers are hidden. A voxel or mesh layer carries no SDF content,
    /// so neither reaches this mesher and hiding one would change what the
    /// viewport draws for no reason.
    ///
    /// Marching tetrahedra rather than surface nets: what comes out is going to
    /// be sculpted and eventually exported, and this is the one the engine
    /// makes watertight and 2-manifold by construction. Nets is the preview
    /// mesher and is half the vertices, which is a saving on something a
    /// sculptor is about to spend an afternoon on.
    fn sdf_to_mesh(&mut self, name: &str, cell_size: f32) -> Result<LayerKey, ModelError> {
        if self.bounds().is_none() {
            return Err(ModelError::Conversion(Refusal::UnboundedRegion));
        }
        let source = self.active_layer().id;
        let hidden: Vec<LayerId> = self
            .layers
            .iter()
            .filter(|layer| layer.id != source)
            .filter(|layer| layer.representation == Representation::Sdf && layer.visible)
            .map(|layer| layer.id)
            .collect();

        let meshed = self.meshed_alone(&hidden, cell_size);
        // Put back before the result is unwrapped. A failed mesh that left the
        // document's other layers hidden would be a conversion that quietly
        // erased the rest of the sculpt.
        for id in &hidden {
            self.document
                .set_layer_visible(*id, true)
                .map_err(ModelError::engine)?;
        }
        let mesh = meshed?;
        if mesh.index_count() == 0 {
            return Err(ModelError::Conversion(Refusal::SourceEmpty));
        }
        self.attach_meshed_layer(mesh, name)
    }

    /// Hides `hidden`, meshes what is left, and hands the mesh back.
    ///
    /// Separated so the restore above runs whether this succeeds or not.
    fn meshed_alone(&mut self, hidden: &[LayerId], cell_size: f32) -> Result<Mesh, ModelError> {
        for id in hidden {
            self.document
                .set_layer_visible(*id, false)
                .map_err(ModelError::engine)?;
        }
        self.document
            .mesh(MeshParams {
                voxel_size: Some(cell_size),
                mesher: Mesher::MarchingTetrahedra,
                ..MeshParams::default()
            })
            .map_err(ModelError::engine)
    }

    /// The active grid's exposed faces as triangles, on a layer of its own.
    ///
    /// The greedy mesh, which is what the grid *is* — merged quads per axis
    /// slice, with the palette colour on the face and a normal per vertex. The
    /// rounded mesher is not used here for the reason the viewport does not use
    /// it either: it carries no vertex normals, so what came out would render
    /// as a flat silhouette and every mesh verb would work on a surface the
    /// sculptor cannot see.
    fn voxels_to_mesh(&mut self, name: &str) -> Result<LayerKey, ModelError> {
        let engine_name = self.active_layer().engine_name.clone();
        let mesh = {
            let (_, grid) = self
                .document
                .voxel_reader(&engine_name)
                .map_err(ModelError::engine)?;
            if grid.occupied_count().map_err(ModelError::engine)? == 0 {
                return Err(ModelError::Conversion(Refusal::SourceEmpty));
            }
            grid.mesh().map_err(ModelError::engine)?
        };
        self.attach_meshed_layer(mesh, name)
    }

    /// Attaches a mesh this application produced as a new layer.
    ///
    /// The same call an import uses, so a converted mesh and an imported one
    /// are the same kind of thing from here on — the mesh verbs reach both, the
    /// quality readout measures both, and a save writes both. No import scale
    /// and no ceiling: the geometry came from this document rather than from a
    /// file, so there is no unit to resolve and nothing untrusted to bound.
    fn attach_meshed_layer(&mut self, mesh: Mesh, name: &str) -> Result<LayerKey, ModelError> {
        let id = self
            .document
            .attach_mesh_layer(
                &mesh,
                &MeshLayerDesc {
                    name: name.to_string(),
                    max_vertices: 0,
                    max_triangles: 0,
                    import_scale: 1.0,
                },
            )
            .map_err(ModelError::engine)?;
        let key = self.adopt_engine_layer(id, name, Representation::Mesh)?;
        // Adopted with triangles already in it, unlike `add_mesh_layer`, which
        // records a row an import fills later. The mesh verbs are available on
        // this the moment the crossing returns, which is the whole point of it.
        if let Some(layer) = self.layers.iter_mut().find(|layer| layer.key == key) {
            layer.carries_geometry = true;
        }
        let made = self.after_conversion(key)?;
        // Ready for the pointer on the frame the crossing returns, rather than
        // after a stroke that could not be placed.
        self.arm_mesh_sculptor();
        Ok(made)
    }

    /// Registers a layer the engine made on its own.
    ///
    /// The conversions that end in SDF hand back a `LayerId` the engine
    /// created — `clay_voxel_to_layer` builds one item per palette entry, and
    /// the mesh crossing builds a volume item — so the layer exists in the
    /// document before this side has a row for it.
    fn adopt_engine_layer(
        &mut self,
        id: LayerId,
        name: &str,
        representation: Representation,
    ) -> Result<LayerKey, ModelError> {
        let key = self.take_key();
        self.layers.push(Layer {
            id,
            key,
            name: name.to_string(),
            engine_name: name.to_string(),
            representation,
            carries_geometry: representation != Representation::Mesh,
            visible: true,
            protection: Protection::default(),
            intensity: 100,
            voxel_bounds: None,
            voxel_chunks: std::collections::BTreeMap::new(),
            sculpt_layers: Vec::new(),
        });
        self.active = self.layers.len() - 1;
        Ok(key)
    }

    /// What every direction owes once its new layer exists.
    fn after_conversion(&mut self, key: LayerKey) -> Result<LayerKey, ModelError> {
        self.reconcile_layers();
        // A mesh layer has no bricks and is not evaluated, so there is nothing
        // to refill for one — the viewport draws it through the carried-layer
        // path instead. Marking it dirty would ask the cache to mark a layer
        // whose field is empty.
        if self.active_layer().representation == Representation::Mesh {
            self.refresh_stats();
            return Ok(key);
        }
        // The whole new layer is dirty; nothing about it was there before.
        let layer = self.active_layer().id;
        self.refill(layer, &[])?;
        Ok(key)
    }

    /// Meshes and refills whatever is currently marked dirty.
    ///
    /// Refreshes the statistics on the way out, because this is the one place
    /// every edit passes through. They used to be refreshed by a handful of
    /// whole-document operations only — opening, the starting form, a bake, a
    /// rig placement — and by nothing a sculptor does continuously, so
    /// `surface_brick_count` stayed at whatever the starting form produced for
    /// the rest of the session. It is what the level-of-detail policy is asked
    /// to decide on, including its "never coarsen under 2048 surface bricks"
    /// floor, so a model sculpted past that floor was still being measured as
    /// if it were the sphere it started as.
    fn drain_dirty(&mut self) -> Result<(), ModelError> {
        // Routed per batch rather than once for the whole drain: a stroke's
        // last iteration is often a handful of residual bricks, and those are
        // cheaper on the CPU than the fixed cost of a device submission.
        // `refill_backend` holds the threshold, and `backend_choice.rs` fails
        // if the measured ratio ever flips back.
        let mut dirty = Vec::new();
        loop {
            let (requests, remaining) = self.cache.take_dirty(512).map_err(ModelError::engine)?;
            if requests.is_empty() {
                break;
            }
            dirty.extend(requests.iter().map(|request| request.key()));
            // The first eligible batch of a session is split: a slice on the
            // CPU, the rest on the accelerated backend. That is what turns the
            // routing from a constant into a measurement, and it costs a
            // fraction of one batch rather than a startup probe — which would
            // be paid by every machine, including the ones the constant is
            // already right for.
            if self.policy.needs_refill_calibration()
                && requests.len() >= 3 * Self::CALIBRATION_SLICE
            {
                // Two equal slices, one per backend, and then the remainder is
                // routed on what they cost. Equal because the comparison is
                // per brick; small because whichever backend loses only ever
                // runs the slice, so the calibration cannot cost more than a
                // few milliseconds even where one backend is several times
                // slower than the other.
                let slice = Self::CALIBRATION_SLICE;
                // The accelerated backend runs once before it is timed. The
                // first call into a device in a process pays for the context
                // and for compiling its pipelines — on a machine whose toolkit
                // is older than its GPU, that is a PTX JIT — and charging a
                // one-time cost to the per-brick rate made CUDA measure 21x
                // slower than the CPU where a warm sweep says 4x. Wrong in the
                // direction that happened to be right here, which is the worst
                // kind of wrong to leave in.
                self.timed_refill(Some(self.active_backend()), &requests[..slice])?;
                self.policy.forget_refill_costs();

                self.timed_refill(None, &requests[slice..2 * slice])?;
                self.timed_refill(Some(self.active_backend()), &requests[2 * slice..3 * slice])?;
                let rest = &requests[3 * slice..];
                let backend = self.policy.refill_backend(rest.len()).cloned();
                self.timed_refill(backend, rest)?;
            } else {
                let backend = self.policy.refill_backend(requests.len()).cloned();
                self.timed_refill(backend, &requests)?;
            }
            if remaining == 0 {
                break;
            }
        }

        // Accumulated, not assigned. This set is pending work for the
        // viewport and is only emptied by `take_dirty_keys`. Overwriting it
        // dropped every edit that landed between two frames: the viewport
        // re-meshed the last dab's neighbourhood and left the rest of the
        // stroke as it was, which drew a closed outline of stale geometry
        // around the edit. `visual_incremental` shows it.
        self.dirty.extend(dirty);
        self.dirty.sort();
        self.dirty.dedup();
        self.refresh_stats();
        Ok(())
    }

    /// Bricks per slice when calibrating the two backends against each other.
    ///
    /// Three slices are used — a warm-up, then one timed on each backend — so
    /// a batch has to hold three of these before it is worth splitting. That
    /// means a session that never refills a hundred bricks at once keeps the
    /// constant, which is the right trade: it is exactly the case where the
    /// routing decision is cheap to get wrong.
    ///
    /// Big enough that a device submission's fixed cost is amortised roughly
    /// as it would be in a real batch — measured at 8 and 64 bricks on two
    /// machines, the ratio between the backends was stable, so a slice this
    /// size predicts a large batch well. Small enough that the losing backend
    /// costs a couple of milliseconds to find out.
    const CALIBRATION_SLICE: usize = 32;

    /// The accelerated backend, for the calibration split.
    fn active_backend(&self) -> claycore::Backend {
        self.policy.active().clone()
    }

    /// Refills a batch on `backend` and tells the policy what it cost.
    ///
    /// Every refill is timed, so the routing keeps following the machine
    /// rather than being decided once. The clock is around the engine call and
    /// nothing else.
    fn timed_refill(
        &mut self,
        backend: Option<claycore::Backend>,
        requests: &[claycore::BrickRequest],
    ) -> Result<(), ModelError> {
        if requests.is_empty() {
            return Ok(());
        }
        let started = std::time::Instant::now();
        self.cache
            .refill(&self.document, backend.as_ref(), requests)
            .map_err(ModelError::engine)?;
        self.policy
            .record_refill(backend.as_ref(), requests.len(), started.elapsed());
        Ok(())
    }

    /// Whether a layer contributes to the surface an edit would touch.
    fn refresh_stats(&mut self) {
        // Read from the cache's own counter rather than by enumerating its
        // keys. `surface_bricks` is a size query plus a copy of every stored
        // key — a megabyte of allocation on a worked model to learn one
        // number — and `stats` keeps that number as it classifies.
        self.surface_brick_count = self
            .cache
            .stats()
            .map(|stats| stats.surface_bricks as usize)
            .unwrap_or(self.surface_brick_count);
        self.stats = SceneStats {
            // The surface cache's own counts, as it recorded them. What the
            // *interface* is told is these plus the carried layers', which
            // `stats` composes — classifying "nothing has been built yet" from
            // the field alone called a document holding one sculpted grid
            // empty.
            triangles: self.stats.triangles,
            vertices: self.stats.vertices,
            objects: self.layers.len().max(1),
            detail: self.stats.detail,
        };
    }

    /// Records the geometry the viewport actually built, so the interface
    /// reports what is on screen rather than an estimate.
    pub fn record_geometry(
        &mut self,
        triangles: usize,
        vertices: usize,
        detail: clayspace_model::Detail,
    ) {
        self.stats.triangles = triangles;
        self.stats.vertices = vertices;
        self.stats.detail = detail;
    }

    /// Turns the domain's brush settings into the engine's stroke preset.
    /// Adds a prepared volume to the active layer. For tests that need to
    /// drive the bake-and-replace path with parameters the tools do not
    /// expose, so a sweep can find the ones that work.
    pub fn add_volume_for_test(&mut self, volume: Item) -> Result<(), ModelError> {
        let layer = self.active_layer().id;
        let node = self
            .document
            .add_item(layer, &volume)
            .map_err(ModelError::engine)?;
        self.refill(layer, &[node])
    }

    /// The spacing a bake-and-replace tool samples the document at.
    ///
    /// Suavizar, Relaxar, Planar and Polir do not stamp: they sample a region
    /// into a volume, modify it, and add it back with `Op::Replace`. Whatever
    /// they do in between, the replacement can be no finer than this — so
    /// sampling coarser than the brick cache draws at replaces a region of the
    /// surface with a blockier version of itself, which is what made those
    /// four crumble.
    pub fn bake_cell_size(brush_size: f32) -> f32 {
        let _ = brush_size;
        Self::VOXEL_SIZE
    }

    /// The brick cache's sampling, which is what the viewport draws.
    pub const VOXEL_SIZE: f32 = 0.02;

    /// The most positional jitter we pass through to the engine.
    ///
    /// Zero, which means the design's Ruído control does not reach the engine.
    ///
    /// This was set after measuring a document/brick-cache disagreement on a
    /// jittered stroke at 0.02 voxels with a 3-voxel band. It does **not**
    /// reproduce at 0.01 voxels with a 6-voxel band, where the two agree to
    /// within 0.002 — so the disagreement is about the narrow band being too
    /// thin to carry the displacement, not about jitter, and the ClayCore bug
    /// this once claimed does not exist. `claycore_repros.rs` holds the
    /// measurement.
    ///
    /// It stays at zero for now because the cache we run is the thin-band one
    /// and a stroke that vanishes is the worst failure this tool can have. The
    /// honest fix is a band wide enough for the brush, not a clamp; that is
    /// open work, and raising this is what should happen once it is done.
    pub const MAX_JITTER: f32 = 0.0;

    fn preset(&self, brush: BrushSettings, tool: ToolKind) -> StrokePreset {
        let brush = brush.sanitized();
        StrokePreset {
            radius: brush.size,
            // Flow is spacing: more flow means stamps closer together.
            spacing: (1.0 - brush.flow).clamp(0.05, 0.9),
            strength: brush.intensity,
            // The design's Ruído, Suavização and Acumular, each landing on the
            // preset field the engine already has for it.
            // Clamped to `MAX_JITTER`, because the engine's two evaluators
            // disagree about a jittered stroke: it shows up in
            // `Document::raycast` but not in the brick cache — not even in a
            // cache built from scratch afterwards, so it is the brick
            // evaluation itself and not the dirty marking. The viewport meshes
            // from the cache, so such a stroke is invisible: the document
            // grows, undo fills up, and the screen never changes. That is what
            // shipped, with Ruído defaulting to 0.15.
            //
            // The clamp lives here rather than in the domain because it is a
            // fact about this engine, not about brushes.
            jitter_position: brush.shaping.noise.min(Self::MAX_JITTER),
            steady: brush.shaping.smoothing,
            accumulation: if tool == ToolKind::Camada || !brush.shaping.accumulate {
                // Camada is the clamped-accumulation tool by definition, and
                // turning Acumular off means the same thing.
                claycore::Accumulation::Clamped
            } else {
                claycore::Accumulation::Buildup
            },
            ..Default::default()
        }
    }

    /// The loaded stamp, if this brush is set to use one and it is accepted
    /// here.
    ///
    /// One place asks the domain whether an alpha applies, so the three stroke
    /// paths cannot come to different answers about it.
    fn alpha_for(&self, brush: BrushSettings, op: Combine) -> Option<&Alpha> {
        if !brush.alpha {
            return None;
        }
        clayspace_model::AlphaSupport::of(self.active_representation(), op)
            .accepted()
            .then_some(self.alpha.as_ref())
            .flatten()
    }

    /// Points the layer's mirror at the axes the sculptor asked for.
    ///
    /// Written only when it changes, so an unchanged setting costs no history
    /// entry. The engine makes a whole stroke one step by itself, so no group
    /// is needed around it.
    ///
    /// Called for *every* SDF stroke rather than only the item-adding ones.
    /// The tools that bake — relax, flatten, the surface drag — used to bypass
    /// this, so the mirror kept whatever it was last set to: the starting form
    /// turns X on, and a snakehook with symmetry switched **off** still came
    /// out on both sides because nothing had told the layer otherwise.
    fn point_the_mirror(&mut self, symmetry: [bool; 3]) -> Result<(), ModelError> {
        if self.symmetry == symmetry {
            return Ok(());
        }
        let layer = self.active_layer().id;
        self.document
            .set_layer_mirror(layer, symmetry, 0.0)
            .map_err(ModelError::engine)?;
        self.symmetry = symmetry;
        Ok(())
    }

    /// A stroke whose verb rewrites the field rather than adding an item.
    ///
    /// The layer mirror reflects a layer's *items*, so it cannot reach these:
    /// measured, a relax with the mirror on changed the surface under the
    /// stroke from 1.1467 to 1.1409 and left its reflection at 1.1467
    /// exactly. They are mirrored the way a mesh stroke is — the stroke
    /// itself is reflected and run again — which is also the only mechanism
    /// available on the other two representations.
    fn baked_stroke(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        // The mirror is still pointed where the sculptor asked, because these
        // verbs share a layer with the ones it does reach.
        self.point_the_mirror(symmetry)?;
        let mut outcome = EditOutcome::NOTHING;
        for mirror in mirrors(symmetry) {
            let reflected: Vec<GestureSample> = samples
                .iter()
                .map(|sample| GestureSample {
                    position: mirror.point(sample.position),
                    ..*sample
                })
                .collect();
            let one = match tool {
                // Drags the assembled surface: the gesture is a displacement,
                // not a series of stamps.
                ToolKind::Mover => self.move_surface_stroke(brush, &reflected)?,
                // Bake-and-relax over the region the stroke covered.
                ToolKind::Suavizar | ToolKind::Relaxar => self.relax_stroke(brush, &reflected)?,
                // Bake-and-flatten, cut-only.
                _ => self.flatten_stroke(brush, &reflected)?,
            };
            outcome = EditOutcome {
                changed: outcome.changed || one.changed,
                dirty_bricks: outcome.dirty_bricks + one.dirty_bricks,
            };
        }
        Ok(outcome)
    }

    /// Applies a stroke to an SDF layer.
    fn stroke_sdf(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        self.point_the_mirror(symmetry)?;
        let layer = self.active_layer().id;
        let preset = self.preset(brush, tool);
        let stroke: Vec<claycore::StrokeSample> = samples
            .iter()
            .map(|s| claycore::StrokeSample {
                position: s.position,
                pressure: s.pressure,
                time: s.time,
            })
            .collect();

        // Every tool that reaches here combines a stamp with the surface.
        // There is no catch-all arm: the one that was here mapped anything
        // unlisted to `Op::Add`, which adds a *sphere* — so the planing tools
        // deposited blobs and nothing said so. A tool with no mapping refuses.
        match tool {
            ToolKind::Padrao | ToolKind::Camada | ToolKind::Inflar => {}
            other => {
                return Err(ModelError::engine(format!(
                    "{} has no mapping onto an SDF verb; it should not have \
                     been offered on this layer",
                    other.label()
                )))
            }
        }
        // Turned over where the modifier is held and the operation has an
        // opposite: Add becomes Subtract, Emboss becomes Engrave, Relief
        // becomes Incise. An operation with no opposite — Intersect, Replace,
        // a seam — is left as it is rather than quietly becoming some other
        // verb, which is what `inverted` answering `None` means.
        let combine = {
            let settings = self.combine.sanitized();
            match brush.invert.then(|| settings.op.inverted()).flatten() {
                Some(op) => clayspace_model::CombineSettings { op, ..settings },
                None => settings,
            }
        };

        let mut stamp = Item::sphere(brush.sanitized().size).map_err(ModelError::engine)?;
        stamp
            .set_op(engine_op(combine.op))
            .map_err(ModelError::engine)?;
        // The blend distance is two different quantities depending on the op,
        // which is why the model marks which family this one is in.
        //
        // For the displacing ops the item is the *region* and `blend_k` is the
        // amplitude the surface moves by along its own normal — not a
        // smoothing distance. It was once set to 40% of the radius, which
        // measured as a displacement of about a sixth of the brush: a stroke
        // that left the sphere looking untouched. The engine saturates the
        // amplitude at roughly the radius, so that is what it is asked for
        // when the sculptor has not asked for less, and `strength` scales it
        // from there. For every other op it is the width of the join, and the
        // sculptor's own zero means a hard one.
        let distance = if combine.op.displaces_along_the_normal() {
            if combine.radius > 0.0 {
                combine.radius
            } else {
                brush.sanitized().size
            }
        } else {
            combine.radius
        };
        stamp
            .set_blend(engine_blend(combine.blend), distance)
            .map_err(ModelError::engine)?;
        // The item's rounding is the falloff width, and it was never set at
        // all. Measured, going from zero to the brush radius tripled the
        // displacement — leaving it at zero was throwing away most of the
        // brush as well as its soft edge.
        stamp
            .set_rounding(brush.sanitized().size)
            .map_err(ModelError::engine)?;

        // No alpha here, and `alpha_for` is what says so rather than a
        // condition repeated at this call site. A field takes one as a
        // deformer on an item, and `clay_layer_apply_stroke` uses its item as
        // a template scaled per stamp — the deformer chain does not travel
        // with it. Measured, and recorded in
        // `claycore/tests/alpha_deformer.rs`.

        let mask = self.mask.as_deref();

        // No gate on the stamp, and that is a measurement rather than an
        // omission. `clay_item_set_gate` is what would make a mask protect a
        // surface from an *operation* rather than only from a brush — a mask
        // over an ear keeps a stroke from depositing there and, without it,
        // does nothing about the boolean the next stroke performs across the
        // region. The wrapper exists and matches the documented contract, and
        // the engine accepts the call and does nothing: measured with a mask
        // sampling 1.0 at the cut's own centre and 65,752 cells painted, a
        // subtraction eats the protected region at every width and threshold
        // tried, and never refuses. `claycore/tests/mask_gate.rs` holds that,
        // and this comes back the day it fails.

        let nodes = self
            .document
            .apply_stroke(layer, &stroke, &preset, &stamp, mask)
            .map_err(ModelError::engine)?;

        if nodes.is_empty() {
            return Ok(EditOutcome::NOTHING);
        }

        self.refill(layer, &nodes)?;
        Ok(EditOutcome {
            changed: true,
            dirty_bricks: self.dirty.len(),
        })
    }

    /// The Move brush: a drag rather than a stamp.
    ///
    /// Nudges form rather than growing it — the engine is explicit that a
    /// large pull buds rather than stretches, which is why Puxar exists.
    fn move_surface_stroke(
        &mut self,
        brush: BrushSettings,
        samples: &[GestureSample],
    ) -> Result<EditOutcome, ModelError> {
        let (first, last) = (samples[0], samples[samples.len() - 1]);
        let displacement = [
            last.position[0] - first.position[0],
            last.position[1] - first.position[1],
            last.position[2] - first.position[2],
        ];
        // A drag under the resolution moves nothing; reporting that as an edit
        // would put an entry in the history for a gesture that did not land.
        let travelled = displacement.iter().map(|d| d * d).sum::<f32>().sqrt();
        if travelled < 1e-4 {
            return Ok(EditOutcome::NOTHING);
        }

        let layer = self.active_layer().id;
        let brush = brush.sanitized();
        let applied = self
            .document
            .move_surface(
                layer,
                first.position,
                displacement,
                claycore::MoveParams {
                    radius: brush.size.max(1e-3),
                    ease: 0,
                    front_only: true,
                },
            )
            .map_err(ModelError::engine)?;

        if applied == 0 {
            return Ok(EditOutcome::NOTHING);
        }
        // The box the move can have touched: the brush around where it started
        // and around where it ended, and nothing else. `move_surface` reports a
        // count rather than nodes, which is why this is computed here rather
        // than asked for.
        let reach = brush.size + travelled;
        let mut min = [0.0f32; 3];
        let mut max = [0.0f32; 3];
        for axis in 0..3 {
            let a = first.position[axis];
            let b = a + displacement[axis];
            min[axis] = a.min(b) - reach;
            max[axis] = a.max(b) + reach;
        }
        self.refill_region(min, max)?;
        Ok(EditOutcome {
            changed: true,
            dirty_bricks: self.dirty.len(),
        })
    }

    /// Snakehook: a tendril along the drawn path, adding material.
    fn snakehook_stroke(
        &mut self,
        brush: BrushSettings,
        samples: &[GestureSample],
    ) -> Result<EditOutcome, ModelError> {
        if samples.len() < 2 {
            return Ok(EditOutcome::NOTHING);
        }
        let brush = brush.sanitized();
        let layer = self.active_layer().id;

        // The mask, honoured here rather than by the engine.
        //
        // A mask reaches an SDF edit inside the stroke engine, where a stamp
        // in a frozen region emits nothing. This verb does not go through the
        // stroke engine — it authors a curve item and adds it — so
        // `clay_layer_add_item` has nowhere to take a mask and the frozen
        // region would be pulled like any other. Sampling the mask along the
        // path and dropping the frozen samples is the same rule applied where
        // this verb can apply it.
        let live: Vec<&GestureSample> = match self.mask.as_ref() {
            Some(mask) => {
                let positions: Vec<[f32; 3]> = samples.iter().map(|s| s.position).collect();
                let frozen = mask.sample_many(&positions).map_err(ModelError::engine)?;
                samples
                    .iter()
                    .zip(frozen)
                    .filter(|(_, value)| *value < 0.5)
                    .map(|(sample, _)| sample)
                    .collect()
            }
            None => samples.iter().collect(),
        };
        if live.len() < 2 {
            // All of it, or all but a point, was frozen.
            return Ok(EditOutcome::NOTHING);
        }

        // The path as control points, each carrying the radius at that point.
        // Tapering toward the tip is what makes it read as a pulled tendril
        // rather than a tube.
        let mut points = Vec::with_capacity(live.len() * 4);
        for (index, sample) in live.iter().enumerate() {
            let t = index as f32 / (live.len() - 1) as f32;
            points.extend_from_slice(&sample.position);
            points.push(brush.size * (1.0 - 0.7 * t));
        }

        // The curve this gesture is already pulling, grown rather than joined.
        //
        // A drag arrives in segments. A segment that authored its own item
        // left a *trail* of tendrils, each restarting the taper from full
        // width — which is the string of beads a curving pull came out as.
        // Measured on one such pull, the thickness along it wobbled by 0.210
        // where a single curve wobbles by 0.137, and that 0.137 is the taper
        // itself.
        if let Some((held, node)) = self.live_hook.filter(|(held, _)| *held == layer) {
            self.document
                .set_layer_stroke_points(held, node, &points, POINT_KIND, Self::CURVE_TOLERANCE)
                .map_err(ModelError::engine)?;
            self.refill(layer, &[node])?;
            return Ok(EditOutcome {
                changed: true,
                dirty_bricks: 1,
            });
        }

        let mut item = Item::stroke().map_err(ModelError::engine)?;
        // Catmull-Rom rather than the default hard corners. A stroke's points
        // are straight-joined by default, which is right for a chain authored
        // point by point and wrong for a tendril pulled along a curving drag:
        // every pointer sample becomes a kink, and the swept sphere bulges at
        // each one. A spline passes *through* the points, so the tendril is
        // the path the pointer took.
        item.set_curve_points(&points, POINT_KIND)
            .map_err(ModelError::engine)?;
        item.set_op(Op::Add).map_err(ModelError::engine)?;
        item.set_stroke_blend_k(brush.size * 0.5)
            .map_err(ModelError::engine)?;

        let node = self
            .document
            .add_item(layer, &item)
            .map_err(ModelError::engine)?;
        // Held only while a gesture is open; `end_gesture` lets it go, so the
        // next pull starts its own tendril.
        if self.previewing {
            self.live_hook = Some((layer, node));
        }
        self.refill(layer, &[node])?;
        Ok(EditOutcome {
            changed: true,
            dirty_bricks: self.dirty.len(),
        })
    }

    /// Smooth on the field side: sample the region into a volume, relax it,
    /// and place the result.
    ///
    /// The engine is explicit that this bakes — relax works on a sampled
    /// volume rather than on the live edit list.
    fn relax_stroke(
        &mut self,
        brush: BrushSettings,
        samples: &[GestureSample],
    ) -> Result<EditOutcome, ModelError> {
        let brush = brush.sanitized();
        let layer = self.active_layer().id;

        // The region the stroke covered, grown by the brush radius.
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for sample in samples {
            for axis in 0..3 {
                min[axis] = min[axis].min(sample.position[axis] - brush.size);
                max[axis] = max[axis].max(sample.position[axis] + brush.size);
            }
        }

        let centre = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];

        // One pass, at the brush's own radius about the gesture's centre.
        //
        // Three shapes were measured, on a deliberately bumpy surface, scored
        // by how much neighbouring pixels disagree — a smoothing tool should
        // leave that lower than it found it (4.9 before, in these units):
        //
        //   one pass at the brush radius   7   <- this
        //   one pass over the whole gesture  13
        //   one pass per sample              11
        //
        // Widening the region or repeating the pass both make it worse, which
        // is not what one would guess. It is measured rather than reasoned,
        // and the reason is not yet understood — see the note in
        // `visual_bake_tools`.
        let cell = Self::bake_cell_size(brush.size);
        // The verb still acts at the brush's radius about the gesture; only
        // the sampled box grows, so the crossfade has untouched clay to land
        // in.
        let (mut min, mut max) = (min, max);
        Self::grown_for_feather(&mut min, &mut max, cell);
        let mut volume = self
            .document
            .relax_region(
                &claycore::RelaxParams {
                    strength: brush.intensity,
                    radius_cells: 1,
                    iterations: 2,
                    centre,
                    region_radius: brush.size,
                    falloff: brush.size * 0.5,
                    mask: self.mask.as_deref(),
                },
                Self::bake_volume(cell),
                min,
                max,
            )
            .map_err(ModelError::engine)?;

        volume.set_op(Op::Replace).map_err(ModelError::engine)?;
        let node = self
            .document
            .add_item(layer, &volume)
            .map_err(ModelError::engine)?;
        self.refill(layer, &[node])?;
        Ok(EditOutcome {
            changed: true,
            dirty_bricks: self.dirty.len(),
        })
    }

    /// Paints the mask along the stroke — Máscara.
    ///
    /// Freezes a region against every verb, which is what a mask is for. It
    /// was mapped onto `Op::Relief` and deformed the surface instead: the tool
    /// that is supposed to protect the clay was denting it, and
    /// [`ToolKind::engine_verb`] said `clay_mask_apply_stroke` all along.
    fn mask_stroke(
        &mut self,
        brush: BrushSettings,
        samples: &[GestureSample],
    ) -> Result<EditOutcome, ModelError> {
        let brush = brush.sanitized();
        let preset = self.preset(brush, ToolKind::Mascara);
        let stroke: Vec<claycore::StrokeSample> = samples
            .iter()
            .map(|s| claycore::StrokeSample {
                position: s.position,
                pressure: s.pressure,
                time: s.time,
            })
            .collect();

        if self.mask.is_none() {
            // The cache's own spacing, not a fraction of the brush.
            //
            // A quarter of the brush was tried: at the default brush that is a
            // 0.1 cell, coarser than anything the surface can express, and
            // `clay_document_mask_extrude` refuses a wall thinner than a cell —
            // so a mask painted with a large brush could not be extruded at any
            // sensible thickness. Matching the voxel size makes a mask as fine
            // as the thing it freezes.
            self.mask = Some(Mask::new(Self::VOXEL_SIZE).map_err(ModelError::engine)?);
        }

        let painted = {
            let mask = self.mask.as_mut().expect("just created");
            mask.apply_stroke(
                &stroke,
                &preset,
                brush.intensity,
                BrushShape::Sphere,
                Falloff::Smooth,
            )
            .map_err(ModelError::engine)?
        };

        // Nothing in the surface moved, and nothing needs re-meshing: a mask
        // is state the *next* stroke reads. The viewport still has to be told,
        // because it draws the frozen region — and a surface that has not
        // moved reports no dirty brick, so the mask carries its own counter.
        if painted > 0 {
            self.mask_revision = self.mask_revision.wrapping_add(1);
        }
        Ok(EditOutcome {
            changed: painted > 0,
            dirty_bricks: 0,
        })
    }

    /// Pulls the region the stroke covered onto a plane — Planar and Polir.
    ///
    /// Both were reaching for `clay_item_volume_flatten`, as
    /// [`ToolKind::engine_verb`] says. It was not bound, and they fell through
    /// a `_ => Op::Add` arm that added a sphere instead: a planing tool that
    /// deposited a blob. The catch-all is gone with them.
    ///
    /// Cut-only, because a planing tool must remove what stands proud without
    /// filling the hollows it is meant to reveal — two-sided flatten is a
    /// different verb with a different name.
    fn flatten_stroke(
        &mut self,
        brush: BrushSettings,
        samples: &[GestureSample],
    ) -> Result<EditOutcome, ModelError> {
        let brush = brush.sanitized();
        let layer = self.active_layer().id;

        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for sample in samples {
            for axis in 0..3 {
                min[axis] = min[axis].min(sample.position[axis] - brush.size);
                max[axis] = max[axis].max(sample.position[axis] + brush.size);
            }
        }
        let centre = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];

        // The plane the stroke defines: through the middle of what it covered,
        // facing the way the surface does there. Without a surface normal to
        // read, the outward direction from the centre of the region is the
        // best available answer and is right for a convex form.
        let normal = {
            let length =
                (centre[0] * centre[0] + centre[1] * centre[1] + centre[2] * centre[2]).sqrt();
            if length < 1e-5 {
                [0.0, 1.0, 0.0]
            } else {
                [centre[0] / length, centre[1] / length, centre[2] / length]
            }
        };

        // Sampled and flattened in one step, straight from the document.
        //
        // Baking with `volume_from_region` and then flattening the result was
        // the first version, because `clay_item_volume_flatten_from` did not
        // exist when this was written — it arrived in 0.27.0. The engine's own
        // note on the difference: a volume reports a distance only inside the
        // band it carries and a lower bound outside it, so a facet moving
        // further than the band is placed against the bound and "a wrong shape
        // [is] returned with CLAY_OK". A document has no band.
        //
        // One pass covering everything the gesture touched, for the same
        // reason relax does. The plane stays put: a planing tool cuts to one
        // plane, and that is what makes a facet.
        let reach = (0..3)
            .map(|axis| (max[axis] - min[axis]) * 0.5)
            .fold(0.0f32, f32::max);
        // As for relax: the box grows so the crossfade lands outside what the
        // verb touched, and the verb's own region_radius is unchanged.
        let cell = Self::bake_cell_size(brush.size);
        let (mut min, mut max) = (min, max);
        Self::grown_for_feather(&mut min, &mut max, cell);
        let mut volume = self
            .document
            .flatten_region(
                &claycore::FlattenParams {
                    plane_point: centre,
                    plane_normal: normal,
                    strength: brush.intensity,
                    centre,
                    // Required positive: with no region the engine replaces
                    // the shape with a half-space, and a ball comes back a box.
                    region_radius: reach + brush.size,
                    falloff: brush.size * 0.5,
                    // Cut-only is what a planing tool wants: it must not fill
                    // the dents it is meant to reveal. Held, the invert key
                    // asks for the other half of that — fill the hollows and
                    // leave the high ground — which is the one thing "negative
                    // planing" can mean and the one the engine already has a
                    // mode for.
                    mode: if brush.invert {
                        claycore::FlattenMode::FillOnly
                    } else {
                        claycore::FlattenMode::CutOnly
                    },
                    mask: self.mask.as_deref(),
                },
                Self::bake_volume(cell),
                min,
                max,
            )
            .map_err(ModelError::engine)?;

        volume.set_op(Op::Replace).map_err(ModelError::engine)?;
        let node = self
            .document
            .add_item(layer, &volume)
            .map_err(ModelError::engine)?;
        self.refill(layer, &[node])?;
        Ok(EditOutcome {
            changed: true,
            dirty_bricks: self.dirty.len(),
        })
    }

    /// A stroke against a mesh layer's own vertices.
    ///
    /// The engine's fourth stroke consumer. What makes it unlike the other
    /// three is that it needs a *sculptor* — the adjacency a brush walks —
    /// which is expensive to build and cheap to keep, so it is built on the
    /// first stroke against a layer and held until the layer changes.
    fn stroke_mesh(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        let Some(verb) = mesh_verb(tool) else {
            // The capability table says this tool has no mesh binding and the
            // shelf does not offer it; reaching here means something asked
            // anyway.
            return Ok(EditOutcome::NOTHING);
        };
        let key = self.active_layer().key;
        let engine_name = self.active_layer().engine_name.clone();
        self.ensure_mesh_sculptor(key, &engine_name)?;

        let brush = brush.sanitized();
        // Read before the sculptor is borrowed mutably. A mesh takes an alpha
        // by a third route — the brush descriptor's own block — and it is not
        // gated on a combine operation, which is the SDF side's vocabulary.
        let alpha = self.alpha_for(brush, Combine::Relief).cloned();
        let alpha = alpha.as_ref();
        // The shared preset, which is where a mesh stroke's radius and
        // strength have to come from: the engine states that
        // `clay_mesh_sculptor_apply_stroke` IGNORES the descriptor's radius
        // and strength and takes each stamp's from the preset. This used to
        // build its own carrying only `spacing`, so a mesh stroke ran at the
        // engine's default radius of 0.25 whatever the brush said — measured,
        // sizes 0.1, 0.5 and 1.0 all moved the same 944 vertices, and
        // Intensidade was inert the same way.
        //
        // Spacing was also inverted here against every other path: the design
        // reads flow as "more flow, stamps closer together", and this passed
        // it straight through so more flow spread them further apart. On Move
        // that is what decides whether a drag emits a second stamp at all, and
        // a drag that emits one stamp has no motion to drag by.
        let mut preset = self.preset(brush, tool);
        // A mesh stroke does not build on itself, whatever the brush says.
        //
        // Not a preference: the mesh verbs that displace along a *per-vertex*
        // normal read the normals the previous stamp just moved, so building
        // up feeds a stamp's own output back into its next input. Measured
        // against Blender's brushes on a matched sphere — same radius in world
        // units, same strength, same stroke — as the mean angle between
        // adjacent vertex normals, before against after:
        //
        //   verb     building up   clamped   Blender
        //   Inflar      5.04x       1.18x     1.00x
        //   Pinçar      9.41x       1.83x     1.00x
        //   Vinco       3.71x       1.34x     1.00x
        //   Padrão      1.11x       1.08x     1.00x
        //
        // Padrão is the control and barely moves either way: it uses the
        // *region's* averaged normal, so there is nothing to feed back.
        //
        // Here rather than in `Shaping::default` because it is a fact about
        // these verbs and not about brushes — the same reason `MAX_JITTER`
        // lives beside the preset. The field and the grid are unaffected, and
        // Acumular still means what it means there.
        // A mesh stroke does not build on itself — except when it is
        // *converging*.
        //
        // The clamp is here because the verbs that displace along a
        // per-vertex normal read the normals the previous stamp just moved, so
        // building up feeds a stamp's output into its own next input and the
        // surface shreds. A smoothing verb has the opposite character: it
        // averages toward the neighbourhood, so running it again moves less
        // each time and converges. Clamping one of those means a sculptor can
        // never smooth more than a single stamp's worth however long they rub,
        // which is what "Suavizar does nothing" turned out to be — measured on
        // a ridge 0.0676 proud of a unit sphere, four passes took it to 1.0670
        // clamped and 1.0187 accumulating.
        if !matches!(
            verb,
            claycore::MeshBrush::Smooth | claycore::MeshBrush::Relax | claycore::MeshBrush::Polish
        ) {
            preset.accumulation = claycore::Accumulation::Clamped;
        }
        // Where the gesture travelled, which is what a verb that pushes along
        // the surface has to be told.
        //
        // `apply_stroke` derives a direction for GRAB and SNAKEHOOK from the
        // motion between stamps and for nothing else — so NUDGE, which
        // projects the drag into each vertex's tangent plane, was handed the
        // descriptor's default of all zeroes and pushed material nowhere. It
        // moved not one vertex at any size, intensity or stroke length, while
        // Blender's equivalent moved 5% of the mesh on the same stroke.
        //
        // Harmless for the two verbs that ignore it, and right for a single
        // stamp, which reads the descriptor's direction whatever the verb.
        // The whole gesture, which is what Grab carries its region by, scaled
        // by the intensity.
        //
        // Scaled here because the descriptor's `strength` weights the falloff
        // rather than the displacement, so a Grab was carrying its region the
        // gesture's whole length whatever Intensidade said. Blender's Grab
        // carries it by the drag *times* the strength — measured, a 1.737 drag
        // at 0.65 moves its furthest vertex 1.129, which is exactly the
        // product — and matching that is what makes the slider mean the same
        // thing in both.
        let gesture = {
            let (first, last) = (samples[0].position, samples[samples.len() - 1].position);
            [
                (last[0] - first[0]) * brush.intensity,
                (last[1] - first[1]) * brush.intensity,
                (last[2] - first[2]) * brush.intensity,
            ]
        };
        // One stamp's worth of it, not the whole gesture. The engine resolves
        // the path into stamps a spacing apart and applies the descriptor's
        // direction at each one, so handing it the gesture's full travel
        // applies that travel once per stamp — measured, a 0.9 drag pushed the
        // surface 1.82 where Blender's Nudge pushed 0.16. A spacing is what
        // the motion between two stamps actually is, which is the same
        // quantity GRAB drags by.
        let travel = {
            let (first, last) = (samples[0].position, samples[samples.len() - 1].position);
            let step = [last[0] - first[0], last[1] - first[1], last[2] - first[2]];
            let length = step.iter().map(|axis| axis * axis).sum::<f32>().sqrt();
            // Scaled by the intensity here because the engine does not: a
            // stamp's strength weights the verbs that displace, and NUDGE
            // moves by the vector it is handed. Measured before this, the
            // Intensidade slider moved the surface 0.5753 at 0.2, at 0.65 and
            // at 1.0 — the same number three times.
            let stamp = preset.spacing * brush.size * brush.intensity * Self::NUDGE_PUSH;
            if length > f32::EPSILON {
                std::array::from_fn(|i| step[i] / length * stamp.min(length))
            } else {
                [0.0; 3]
            }
        };
        let stamp = claycore::MeshStamp {
            verb,
            direction: travel,
            center: samples[0].position,
            // The radius is carried even though a resolved stroke replaces it
            // per stamp, because the same descriptor is what a single stamp
            // uses and one that disagreed with the preset would be a trap for
            // the next caller.
            radius: brush.size,
            // The strength is not merely carried: a resolved stroke
            // *multiplies* it by each stamp's own, so this is where a mesh
            // stroke's sign lives.
            //
            // Which is why holding the invert key turns this over rather than
            // the preset's strength. The preset's is contracted to [0, 1] and
            // the stroke resolver drops any stamp whose strength is not
            // positive, so a negative preset strength is not a dig — it is
            // nothing at all, which is what it measured as: a full sweep with
            // the key held moved no vertex and reported no change.
            strength: if brush.invert {
                -brush.intensity
            } else {
                brush.intensity
            },
            falloff: match brush.shaping.falloff {
                clayspace_model::Falloff::Constant => claycore::MeshFalloff::Constant,
                clayspace_model::Falloff::Linear => claycore::MeshFalloff::Linear,
                clayspace_model::Falloff::Smooth => claycore::MeshFalloff::Smooth,
                clayspace_model::Falloff::Gaussian => claycore::MeshFalloff::Gaussian,
            },
            // A stamp scaling the per-vertex weight, borrowed for the call.
            // The same kernel the SDF alpha uses, so one texture reads
            // identically on a mesh and on a field.
            alpha: alpha.map(|alpha| claycore::AlphaStamp {
                samples: &alpha.samples,
                width: alpha.width as i32,
                height: alpha.height as i32,
                // All zeroes: the surface normal under the brush centre,
                // which is what a detail stamp on a mesh wants.
                direction: [0.0; 3],
                tangent: [1.0, 0.0, 0.0],
                // Zero: the brush's own diameter.
                extent: 0.0,
            }),
            smooth_iterations: Some(Self::SMOOTH_PASSES),
            // Flatten and Scrape mean "everything under this disc", and a
            // surface walk refuses to flatten across a groove — which is not
            // what either verb says.
            geodesic: !matches!(
                verb,
                claycore::MeshBrush::Flatten | claycore::MeshBrush::Scrape
            ),
            ..claycore::MeshStamp::default()
        };
        let points: Vec<[f32; 5]> = samples
            .iter()
            .map(|s| {
                [
                    s.position[0],
                    s.position[1],
                    s.position[2],
                    s.pressure,
                    s.time,
                ]
            })
            .collect();

        // Recorded per gesture, because that is the unit a sculptor thinks in
        // and the unit `mesh-sculpting` specifies: one gesture, one undo.
        let mut deltas = claycore::MeshDeltas::new().map_err(ModelError::engine)?;
        // What the last segment of this gesture did, taken back before the
        // whole gesture is laid down again from its anchor. Without this a
        // preview would stack segment on segment, which is the crease the
        // whole-gesture delivery exists to avoid.
        let previous = self
            .live_mesh
            .take()
            .filter(|(layer, _)| *layer == key)
            .map(|(_, deltas)| deltas);
        let moved = {
            let mut held = self.mesh_sculptor.borrow_mut();
            let Some((_, sculptor)) = held.as_mut() else {
                return Ok(EditOutcome::NOTHING);
            };
            if let Some(previous) = &previous {
                previous.revert(sculptor).map_err(ModelError::engine)?;
            }
            // Every reflection the enabled axes call for, the unmirrored
            // stroke among them. Two axes give four dabs and three give eight,
            // which is what both references do — measured in Blender on a
            // 64x32 sphere, one dab moved 82 vertices on +x with symmetry off,
            // 82 on each side with x on, and 161 on each of four quadrants
            // with x and y on.
            //
            // All of them into the *same* `MeshDeltas`, so a symmetric gesture
            // is one undo and the preview's revert takes every copy back
            // together.
            let mut moved = 0;
            for mirror in mirrors(symmetry) {
                let moved_here = if verb == claycore::MeshBrush::Grab {
                    // One stamp at the point the gesture took hold of, carrying
                    // that region by the whole drag — which is what Grab is, in
                    // Blender and in ZBrush both.
                    //
                    // Not a resolved stroke. `apply_stroke` walks the path and
                    // moves the brush centre along it, so a drag that leaves the
                    // surface takes the centre with it and the later stamps reach
                    // no material at all: measured, a 120-pixel drag carried the
                    // centre 2.118 from a unit sphere's middle and left a dent
                    // where a lobe should have come out. A single stamp reads the
                    // descriptor's own radius, strength and direction — which a
                    // stroke ignores — so the region is the one under the anchor
                    // and the displacement is the gesture's, whole.
                    //
                    // Snakehook and Nudge stay on the stroke path deliberately:
                    // one re-anchors on every stamp so its region walks with the
                    // pull, and the other pushes along the surface. Neither is a
                    // region carried somewhere.
                    sculptor
                        .stamp(
                            claycore::MeshStamp {
                                direction: mirror.vector(gesture),
                                center: mirror.point(stamp.center),
                                ..stamp
                            },
                            self.mask.as_ref(),
                            Some(&mut deltas),
                        )
                        .map_err(ModelError::engine)?
                } else {
                    let path: Vec<[f32; 5]> = points
                        .iter()
                        .map(|sample| {
                            let at = mirror.point([sample[0], sample[1], sample[2]]);
                            [at[0], at[1], at[2], sample[3], sample[4]]
                        })
                        .collect();
                    sculptor
                        .apply_stroke(
                            &path,
                            &preset,
                            claycore::MeshStamp {
                                direction: mirror.vector(stamp.direction),
                                center: mirror.point(stamp.center),
                                ..stamp
                            },
                            self.mask.as_ref(),
                            Some(&mut deltas),
                        )
                        .map_err(ModelError::engine)?
                };
                moved += moved_here;
            }
            // Refit rather than refresh: topology is fixed, so the ray-query
            // tree stays a valid partition and only its bounds went stale,
            // which is proportional to the brush instead of to the mesh.
            sculptor.refit().map_err(ModelError::engine)?;
            moved
        };

        // A gesture that reached nothing is not worth a place on the stack,
        // and putting one there would make an undo appear to do nothing.
        let reached = deltas.vertex_count().map_err(ModelError::engine)? > 0;
        if self.previewing {
            // Held rather than banked. The gesture is still open, and every
            // segment replaces the last — one drag is one undo however many
            // segments drew it.
            if reached {
                self.live_mesh = Some((key, deltas));
            }
            self.live_generation = self.live_generation.wrapping_add(1);
        } else if reached {
            self.mesh_undo.push(MeshGesture {
                layer: key,
                deltas,
                engine_depth: self.engine_undo_depth(),
            });
            // A new edit ends the redo line, exactly as the engine's own does.
            self.mesh_redo.clear();
        }
        Ok(EditOutcome {
            changed: moved > 0,
            // A mesh layer is not in the brick cache at all, so nothing was
            // dirtied and nothing needs re-meshing — the viewport reads the
            // layer's own triangles.
            dirty_bricks: 0,
        })
    }

    /// Where a ray meets the active layer's grid.
    ///
    /// Through a read-only borrow of the grid, which is what lets this answer
    /// from a `&self` method: the engine's lookup takes a mutable document
    /// handle because one call serves reads and writes, and a picking ray
    /// writes nothing.
    ///
    /// The engine reports the distance to the entry face of the first occupied
    /// cell, along the direction it normalized — so the position is the origin
    /// plus the *unit* direction times that distance, and a caller passing an
    /// unnormalized direction still gets the right point.
    fn pick_active_grid(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]> {
        let layer = self.active_layer();
        let (_, grid) = self.document.voxel_reader(&layer.engine_name).ok()?;
        let hit = grid.raycast(origin, direction).ok().flatten()?;
        let length = direction.iter().map(|axis| axis * axis).sum::<f32>().sqrt();
        if length <= f32::EPSILON {
            return None;
        }
        Some(std::array::from_fn(|i| {
            origin[i] + direction[i] / length * hit.distance
        }))
    }
    /// Where a ray meets the active mesh layer's triangles.
    ///
    /// Answered by the sculptor's own tree, through the cell that field is
    /// held in — see there for why.
    fn pick_active_mesh(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]> {
        let key = self.active_layer().key;
        let mut held = self.mesh_sculptor.borrow_mut();
        let (built_for, sculptor) = held.as_mut()?;
        if *built_for != key {
            // Not built yet, and a pick cannot build it — that costs an
            // adjacency pass and a pick happens every frame the pointer moves.
            // The first stroke builds it; until then the pointer finds nothing
            // on this layer, which reads as the cursor not settling rather
            // than as a wrong answer.
            return None;
        }
        sculptor
            .raycast(origin, direction)
            .ok()
            .flatten()
            .map(|hit| hit.position)
    }

    /// What is wrong with the active voxel layer, before anything is repaired.
    ///
    /// `None` where the active layer is not a grid. Asked separately from the
    /// repair itself, and asked first: a repair changes the sculpt, and a
    /// sculptor who cannot see what it would change is being asked to consent
    /// to something unstated.
    pub fn repair_report(&mut self) -> Option<clayspace_model::RepairReport> {
        if self.active_representation() != Representation::Voxel {
            return None;
        }
        let engine_name = self.active_layer().engine_name.clone();
        let (_, grid) = self.document.voxel_layer(&engine_name).ok()?;
        let report = grid.repair_report().ok()?;
        Some(clayspace_model::RepairReport {
            enclosed_voids: report.enclosed_voids,
            void_cells: report.void_cells,
            largest_void: report.largest_void,
            airtight: report.airtight,
        })
    }

    /// The pre-bake verbs and the level stack, which reach a grid directly.
    fn apply_voxel_operation(
        &mut self,
        operation: clayspace_model::LayerOperation,
    ) -> Result<EditOutcome, ModelError> {
        let engine_name = self.active_layer().engine_name.clone();
        let layer_id = self.active_layer().id;
        {
            let (_, mut grid) = self
                .document
                .voxel_layer(&engine_name)
                .map_err(ModelError::engine)?;
            match operation {
                clayspace_model::LayerOperation::CloseHoles { passes } => grid
                    .repair_close_holes(passes.clamp(1, 16), None)
                    .map_err(ModelError::engine)?,
                clayspace_model::LayerOperation::FillVoids => {
                    grid.repair_fill_voids(None).map_err(ModelError::engine)?
                }
                clayspace_model::LayerOperation::RefineRegion { min, max } => {
                    grid.add_level_region(min, max)
                        .map(|_| ())
                        .map_err(ModelError::engine)?;
                }
                _ => return Ok(EditOutcome::NOTHING),
            }
        }
        // The whole layer may have moved: a repair is not bounded by a brush.
        self.refill(layer_id, &[])?;
        Ok(EditOutcome {
            changed: true,
            dirty_bricks: self.dirty.len(),
        })
    }

    /// What fraction of a stamp's spacing NUDGE pushes by.
    ///
    /// A calibration, and stated as one. NUDGE projects the drag into *each
    /// vertex's own* tangent plane, so neighbouring vertices on a curved cap
    /// are pushed in diverging directions and a large push shears them apart.
    /// Measured as the mean angle between adjacent vertex normals, against the
    /// same surface before the stroke:
    ///
    ///   push        surface moved   roughness
    ///   1 spacing       0.776         12.23x
    ///   1/2 spacing     0.361          7.18x
    ///   0.15 spacing    0.049          1.43x
    ///
    /// Blender's Nudge moves 0.164 on the same stroke at 1.00x, so ours is
    /// rougher than its equivalent at any given displacement — that is the
    /// engine's tangent-plane push and not something a factor here can undo.
    /// This keeps it inside the band every other mesh verb now sits in.
    /// Turning the surface walk off does not help: measured at 7.18x either
    /// way.
    const NUDGE_PUSH: f32 = 0.15;

    /// How many Laplacian passes a smoothing verb runs per stamp.
    ///
    /// The engine's SMOOTH averages a vertex with its *one-ring*, which is a
    /// high-frequency filter: it takes out tessellation noise and barely
    /// touches a bump that spans many edges. To smooth at the scale of the
    /// brush it has to be run many times, and the engine's own default is far
    /// below what that needs.
    ///
    /// Measured on a ridge standing 0.0676 proud of a unit sphere, four
    /// smoothing passes over it, with the sculptor's accumulation on:
    ///
    ///   passes per stamp   ridge left   cost at a 0.18 brush
    ///    1                   1.0654            —
    ///    8                   1.0552          4.0 ms
    ///   16                   1.0466            —
    ///   32                   1.0343          4.7 ms
    ///   64                   1.0187          5.4 ms
    ///
    /// The engine's ceiling, and cheap at it: the passes are a fraction of the
    /// cost of finding the region in the first place. At 64 a single stroke
    /// takes about a quarter of the ridge, so rubbing melts it — which is what
    /// smoothing does in Blender and in ZBrush, and what it conspicuously did
    /// not do here.
    const SMOOTH_PASSES: i32 = 64;

    /// Cells of margin around a removed layer's bounds.
    ///
    /// One brick's worth and then some: the cache marks bricks that *overlap*
    /// the box, and a surface sitting on the bounds contributes to the brick
    /// beyond them.
    const BRICK_MARGIN: f32 = 16.0;

    /// Chunk keys drained from a grid in one go.
    ///
    /// The engine stages the whole queue on the first call after a large edit
    /// and holds it until the drain finishes, so this bounds the loop's
    /// iterations rather than its memory. A stroke dirties single figures.
    const VOXEL_CHUNK_BATCH: usize = 1024;
    /// How far a curve's span may sit from its chord before it is split again.
    ///
    /// A property of the document rather than of the viewer — two builds have
    /// to agree on what a document means — so it is a constant here and not a
    /// display setting.
    const CURVE_TOLERANCE: f32 = 0.002;

    /// The triangles of every visible mesh and voxel layer, for the viewport.
    ///
    /// Neither representation has bricks, so the surface built from the cache
    /// cannot contain either: the cache holds the document's SDF field, and a
    /// voxel layer carries no SDF content — the engine says so outright, and a
    /// document holding nothing but a sculpted grid meshed to zero triangles
    /// because of it. This is the second geometry source, and it is combined
    /// across layers because the viewport draws one buffer: the indices of
    /// each layer are shifted past the vertices already collected.
    ///
    /// Hidden layers are left out rather than uploaded and skipped — the point
    /// of hiding one is not to pay for it.
    #[allow(clippy::type_complexity)]
    pub fn visible_mesh_geometry(
        &mut self,
    ) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<u32>) {
        // Every grid first, visible or not: the dirty set is the engine's and
        // draining it is what keeps a chunk's geometry in step with its cells.
        // Skipping a hidden layer would leave its keys queued, and showing it
        // again would then re-mesh the whole backlog in one frame.
        self.meshed_chunks = 0;
        for index in 0..self.layers.len() {
            if let Err(e) = self.refresh_voxel_chunks(index) {
                eprintln!("a camada de voxels não pôde ser remalhada: {e}");
            }
        }
        // And the smooth surface, where that is the picture. Here beside the
        // chunks rather than left to the caller: this method's job is to hand
        // back what the viewport draws, and a consumer that did not know to
        // ask would silently get the boxes instead. Cheap when nothing moved —
        // the grid's change count is compared first.
        if let Err(e) = self.resmooth_voxels() {
            eprintln!("a malha suave não pôde ser reconstruída: {e}");
        }

        // Sized once from what the chunks hold, so assembling a worked grid
        // is a copy rather than a dozen reallocations of a growing buffer.
        let (mut vertices, mut triangles) = (0, 0);
        for layer in &self.layers {
            for chunk in layer.voxel_chunks.values() {
                vertices += chunk.positions.len();
                triangles += chunk.indices.len();
            }
        }
        let mut positions = Vec::with_capacity(vertices);
        let mut normals = Vec::with_capacity(vertices);
        let mut colors = Vec::with_capacity(vertices);
        let mut indices = Vec::with_capacity(triangles);

        let drawn: Vec<(usize, Representation, String)> = self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.carries_geometry && layer.visible)
            .filter(|(_, layer)| layer.representation != Representation::Sdf)
            .map(|(index, layer)| (index, layer.representation, layer.engine_name.clone()))
            .collect();

        for (index, representation, name) in drawn {
            if representation == Representation::Voxel {
                // The smooth picture, where one has been built. Whole-grid and
                // so a single splice, unlike the chunked boxes below.
                if let Some((_, smooth)) = self.voxel_smooth.get(&self.layers[index].key) {
                    let base = positions.len() as u32;
                    indices.extend(smooth.indices.iter().map(|i| i + base));
                    positions.extend_from_slice(&smooth.positions);
                    normals.extend_from_slice(&smooth.normals);
                    colors.extend_from_slice(&smooth.colors);
                    continue;
                }
                // Spliced from what was meshed per chunk. The ranges partition
                // the mesh, so concatenating them is the whole of the join —
                // there is no seam to weld, unlike the brick cache's.
                for chunk in self.layers[index].voxel_chunks.values() {
                    let base = positions.len() as u32;
                    indices.extend(chunk.indices.iter().map(|i| i + base));
                    positions.extend_from_slice(&chunk.positions);
                    normals.extend_from_slice(&chunk.normals);
                    colors.extend_from_slice(&chunk.colors);
                }
                continue;
            }
            let Ok((p, n, c, i)) = self.document.read_mesh_layer(&name) else {
                continue;
            };
            let base = positions.len() as u32;
            indices.extend(i.into_iter().map(|index| index + base));
            positions.extend(p);
            normals.extend(n);
            colors.extend(c);
        }

        // What the viewport was handed, so the interface can count what is on
        // screen rather than only what the brick cache built. A mesh or voxel
        // layer draws triangles the surface cache knows nothing about, and the
        // panel used to report a sculpted grid as an empty document.
        self.carried = (indices.len() / 3, positions.len());
        (positions, normals, colors, indices)
    }

    /// Brings one voxel layer's cached chunks in line with its grid.
    ///
    /// The engine keeps the dirty set: a write that changes a cell dirties its
    /// chunk, and one on a chunk face also dirties the chunk across it, whose
    /// exposed faces it changed. Draining it and re-meshing only those keys is
    /// what makes an edit cost the edit. A grid loaded from a file or given a
    /// level reports every chunk it wrote, so the first display and an
    /// incremental one are this same path.
    ///
    /// A key whose chunk a stroke emptied comes back with an empty range —
    /// that is precisely the key whose geometry has to be *dropped*, so it is
    /// removed rather than stored as nothing.
    fn refresh_voxel_chunks(&mut self, index: usize) -> Result<(), ModelError> {
        if self.layers[index].representation != Representation::Voxel {
            return Ok(());
        }
        let engine_name = self.layers[index].engine_name.clone();
        // Split by field: the layer list and the document are disjoint, but
        // `&mut self` for one while the other is borrowed is not.
        let Self {
            document,
            layers,
            meshed_chunks: meshed,
            ..
        } = self;
        let (_, mut grid) = document
            .voxel_layer(&engine_name)
            .map_err(ModelError::engine)?;

        loop {
            let (keys, remaining) = grid
                .take_dirty_chunks(Self::VOXEL_CHUNK_BATCH)
                .map_err(ModelError::engine)?;
            if keys.is_empty() {
                break;
            }
            let (mesh, ranges) = grid.mesh_chunks(&keys).map_err(ModelError::engine)?;
            *meshed += keys.len();
            let positions = mesh.positions();
            let normals = mesh.normals();
            let colors = mesh.colors();
            let indices = mesh.indices();

            for range in ranges {
                let chunks = &mut layers[index].voxel_chunks;
                if range.index_count == 0 {
                    chunks.remove(&range.key);
                    continue;
                }
                let vertices = range.vertex_first..range.vertex_first + range.vertex_count;
                let span = range.index_first..range.index_first + range.index_count;
                let base = range.vertex_first as u32;
                chunks.insert(
                    range.key,
                    ChunkGeometry {
                        positions: positions[vertices.clone()].to_vec(),
                        // The greedy mesher supplies both. The fallbacks are
                        // what a mesh layer missing them gets, and are here so
                        // a future mesher that omits one still draws.
                        normals: normals
                            .map(|n| n[vertices.clone()].to_vec())
                            .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; range.vertex_count]),
                        colors: colors
                            .map(|c| c[vertices].to_vec())
                            .unwrap_or_else(|| vec![[1.0; 3]; range.vertex_count]),
                        // Rebased onto this chunk's own first vertex, so the
                        // slice stands alone and can be spliced anywhere.
                        indices: indices[span].iter().map(|i| i - base).collect(),
                    },
                );
            }

            if remaining == 0 {
                break;
            }
        }
        Ok(())
    }

    /// Rebuilds the smooth mesh of every voxel layer.
    ///
    /// Whole-grid, because the smooth picture cannot be meshed a chunk at a
    /// time: `clay_voxel_mesh_chunks` is the greedy mesher alone, and the
    /// engine says why — greedy quads are axis-aligned and exact, so clamping
    /// their merge to a chunk boundary emits more, smaller quads over the
    /// identical surface and never a crack, while surface nets place a vertex
    /// from a cell's *neighbourhood* and would tear.
    ///
    /// So this is a settle: called when a gesture ends rather than while it is
    /// made. Measured on the reference grid, 16.8 ms against 1.5 ms for the
    /// greedy whole-grid mesh — and the incremental greedy path a stroke
    /// actually uses is 3.3 ms a dab.
    pub fn resmooth_voxels(&mut self) -> Result<(), ModelError> {
        if self.voxel_display != VoxelDisplay::Smooth {
            self.voxel_smooth.clear();
            return Ok(());
        }
        let grids: Vec<(LayerKey, String)> = self
            .layers
            .iter()
            .filter(|layer| layer.representation == Representation::Voxel)
            .map(|layer| (layer.key, layer.engine_name.clone()))
            .collect();
        let blur = self.voxel_blur.passes();
        for (key, engine_name) in grids {
            let (changes, mesh) = {
                let (_, grid) = self
                    .document
                    .voxel_layer(&engine_name)
                    .map_err(ModelError::engine)?;
                let changes = grid.change_count().map_err(ModelError::engine)?;
                // Nothing has moved since this was built, so there is nothing
                // to rebuild. This is what lets the call sit on the frame path
                // rather than only on a settle: a whole-grid mesh is 17 to 21
                // ms and a comparison is nothing, so the cost is paid when the
                // sculptor changes something and not otherwise.
                if self
                    .voxel_smooth
                    .get(&key)
                    .is_some_and(|(built, _)| *built == changes)
                {
                    continue;
                }
                (changes, grid.mesh_smooth(blur).map_err(ModelError::engine)?)
            };
            if mesh.vertex_count() == 0 {
                self.voxel_smooth.remove(&key);
                continue;
            }
            self.voxel_smooth
                .insert(key, (changes, smooth_geometry(&mesh)));
        }
        Ok(())
    }

    /// Which picture of a voxel layer the viewport draws.
    pub fn voxel_display(&self) -> VoxelDisplay {
        self.voxel_display
    }

    pub fn voxel_blur(&self) -> SmoothBlur {
        self.voxel_blur
    }

    /// Changes the picture, and rebuilds it.
    ///
    /// Rebuilt here rather than left for the next settle, because a sculptor
    /// who asks for the other picture is asking to see it now.
    pub fn set_voxel_display(
        &mut self,
        display: VoxelDisplay,
        blur: SmoothBlur,
    ) -> Result<(), ModelError> {
        if self.voxel_display == display && self.voxel_blur == blur {
            return Ok(());
        }
        self.voxel_display = display;
        self.voxel_blur = blur;
        // Dropped rather than compared: the filtering changed, so the stored
        // mesh is stale even though no cell moved and its change count is the
        // one it was built at.
        self.voxel_smooth.clear();
        self.resmooth_voxels()
    }

    /// How many chunks the last assembly re-meshed.
    ///
    /// Zero on a frame where no grid changed, and a handful after a dab — the
    /// whole point of draining the engine's dirty set rather than meshing the
    /// grid. Exposed so a test can hold it to that without measuring time,
    /// which on a shared machine measures the machine.
    pub fn meshed_chunks(&self) -> usize {
        self.meshed_chunks
    }

    /// A number that changes when the carried geometry does.
    ///
    /// So the viewport can tell whether its copy is stale without comparing
    /// the triangles. A mesh gesture is the only thing that moves a mesh
    /// layer's vertices, and every one of those lands on the undo stack — so
    /// the two stack depths say it, and an undo changes the answer as surely
    /// as a stroke does.
    ///
    /// A grid says it itself: the engine counts every change to one, so the
    /// counts are read rather than a revision being bumped at each of the
    /// dozen sites that can touch a grid. A site that forgot to bump would
    /// leave the viewport showing the sculpt as it was before the edit, which
    /// is exactly the failure this number exists to prevent.
    /// Changes whenever the mask does.
    ///
    /// The counterpart to [`Self::mesh_revision`] for the one piece of state
    /// that is drawn but is not geometry. A mask stroke moves no clay and
    /// dirties no brick, so nothing the surface reports would tell the
    /// viewport to look again.
    pub fn mask_revision(&self) -> u64 {
        self.mask_revision
    }

    /// How frozen each of these points is, or `None` when nothing is masked.
    ///
    /// `None` rather than a run of zeroes so the caller can skip the work
    /// entirely — which is the common case, and the case where sampling every
    /// vertex of the surface would be pure waste.
    pub fn mask_at(&self, points: &[[f32; 3]]) -> Option<Vec<f32>> {
        let mask = self.mask.as_ref()?;
        if mask.is_empty().unwrap_or(true) {
            return None;
        }
        mask.sample_many(points).ok()
    }

    pub fn mesh_revision(&mut self) -> u64 {
        // Which layers this path draws at all, and whether each is shown.
        //
        // Adding a mesh layer moves no vertex and touches no grid, so without
        // this the number did not change when one appeared — and the viewport,
        // which uploads only when it changes, never uploaded it. A crossing
        // into a mesh drew nothing: what stayed on screen was the *field* the
        // source layer still contributed, and removing that source left an
        // empty viewport with 62,576 vertices sitting unuploaded. The first
        // stroke moved a vertex, changed the number the old way, and the mesh
        // appeared — which is exactly how it was reported.
        let carried = self
            .layers
            .iter()
            .filter(|layer| layer.representation != Representation::Sdf)
            .fold(0xcbf2_9ce4_8422_2325u64, |hash, layer| {
                let shown = u64::from(layer.visible && layer.carries_geometry);
                (hash ^ (layer.key.0 << 1 | shown)).wrapping_mul(0x1000_0000_01b3)
            });

        let names: Vec<String> = self
            .layers
            .iter()
            .filter(|layer| layer.representation == Representation::Voxel)
            .map(|layer| layer.engine_name.clone())
            .collect();
        let grids = names.iter().fold(0u64, |sum, name| {
            let counted = self
                .document
                .voxel_layer(name)
                .ok()
                .and_then(|(_, grid)| grid.change_count().ok())
                .unwrap_or(0);
            sum.wrapping_add(counted)
        });
        let meshes = (self.mesh_undo.len() as u64) << 32 | self.mesh_redo.len() as u64;
        meshes
            .wrapping_mul(31)
            .wrapping_add(grids)
            .wrapping_add(carried)
            // A preview banks nothing, so without this the number would sit
            // still while the drag was visibly moving the surface.
            .wrapping_add(self.live_generation.wrapping_mul(1_000_003))
            // The frozen region is drawn on these layers too, and a mask
            // stroke moves none of their vertices — so without this a mask
            // painted on a mesh or a grid would be invisible on exactly the
            // layer it was painted on.
            .wrapping_add(self.mask_revision.wrapping_mul(2_000_003))
            // And which picture of a grid is drawn. A settle rebuilds the
            // smooth mesh without touching a cell, so nothing the grid reports
            // would tell the viewport to look again.
            .wrapping_add(
                self.voxel_smooth
                    .values()
                    .fold(0u64, |sum, (_, mesh)| {
                        sum.wrapping_add(mesh.positions.len() as u64)
                    })
                    .wrapping_mul(3_000_017),
            )
    }

    /// How stretched the active mesh layer's triangles are.
    ///
    /// Sculpting a mesh stretches what is there — a large grab does, and
    /// snakehook does it to the extreme — and nothing here retessellates,
    /// because that would spend the retopology the import was for. So the
    /// stretch is *reported* rather than prevented, and a sculptor learns the
    /// mesh wants retopology at the point it starts wanting it instead of at
    /// export.
    ///
    /// `None` where the active layer is not a sculpted mesh.
    pub fn mesh_quality(&self) -> Option<f32> {
        let key = self.active_layer().key;
        let mut held = self.mesh_sculptor.borrow_mut();
        let (built_for, sculptor) = held.as_mut()?;
        (*built_for == key)
            .then(|| sculptor.quality().ok())
            .flatten()
    }

    /// The engine's own undo depth, which is what the two histories order by.
    fn engine_undo_depth(&self) -> usize {
        self.document
            .undo_state()
            .map(|state| state.undo_depth)
            .unwrap_or(0)
    }

    /// Whether the newest mesh gesture is more recent than the newest engine
    /// entry.
    ///
    /// True when no engine edit has landed since it was recorded: any that had
    /// would have raised the depth past what the record remembers.
    fn mesh_gesture_is_newest(&self) -> bool {
        self.mesh_undo
            .last()
            .is_some_and(|gesture| gesture.engine_depth == self.engine_undo_depth())
    }

    /// Takes back one mesh gesture, bit exactly.
    fn undo_mesh_gesture(&mut self) -> Result<bool, ModelError> {
        let Some(gesture) = self.mesh_undo.pop() else {
            return Ok(false);
        };
        let engine_name = self
            .layers
            .iter()
            .find(|layer| layer.key == gesture.layer)
            .map(|layer| layer.engine_name.clone());
        let Some(engine_name) = engine_name else {
            // The layer it belongs to is gone, so there is nothing to put
            // back. Dropping the record is the whole of the answer.
            return Ok(true);
        };
        self.ensure_mesh_sculptor(gesture.layer, &engine_name)?;
        {
            let mut held = self.mesh_sculptor.borrow_mut();
            let Some((_, sculptor)) = held.as_mut() else {
                return Ok(false);
            };
            gesture
                .deltas
                .revert(sculptor)
                .map_err(ModelError::engine)?;
            sculptor.refit().map_err(ModelError::engine)?;
        }
        self.mesh_redo.push(gesture);
        Ok(true)
    }

    /// Puts one back.
    fn redo_mesh_gesture(&mut self) -> Result<bool, ModelError> {
        let Some(gesture) = self.mesh_redo.pop() else {
            return Ok(false);
        };
        let engine_name = self
            .layers
            .iter()
            .find(|layer| layer.key == gesture.layer)
            .map(|layer| layer.engine_name.clone());
        let Some(engine_name) = engine_name else {
            return Ok(true);
        };
        self.ensure_mesh_sculptor(gesture.layer, &engine_name)?;
        {
            let mut held = self.mesh_sculptor.borrow_mut();
            let Some((_, sculptor)) = held.as_mut() else {
                return Ok(false);
            };
            gesture.deltas.apply(sculptor).map_err(ModelError::engine)?;
            sculptor.refit().map_err(ModelError::engine)?;
        }
        self.mesh_undo.push(gesture);
        Ok(true)
    }

    /// Builds the sculptor for a mesh layer, or keeps the one already built.
    ///
    /// Rebuilt when the layer changes: a sculptor holds adjacency for the mesh
    /// it was given, so carrying one across layers would move the wrong
    /// vertices.
    fn ensure_mesh_sculptor(&mut self, key: LayerKey, engine_name: &str) -> Result<(), ModelError> {
        if matches!(self.mesh_sculptor.borrow().as_ref(), Some((held, _)) if *held == key) {
            return Ok(());
        }
        // Relative to the bounding-box diagonal: vertices closer than this are
        // one point of the surface, which is what lets a brush move a split
        // seam as a seam rather than tearing it open.
        const WELD: f32 = 1e-4;
        let sculptor = claycore::MeshSculptor::for_layer(&mut self.document, engine_name, WELD)
            .map_err(ModelError::engine)?;
        *self.mesh_sculptor.borrow_mut() = Some((key, sculptor));
        Ok(())
    }

    /// Builds the mesh sculptor for the active layer, if it needs one.
    ///
    /// Called when a layer becomes the one being worked on, which is the
    /// moment the adjacency pass is worth paying for: it is a discrete thing
    /// the sculptor did, not something a moving pointer repeats.
    ///
    /// It has to happen *before* the first stroke, and that is the whole
    /// reason this exists. A pick against a mesh layer is answered by the
    /// sculptor's own raycast, and it used to refuse until the sculptor was
    /// built — which the first stroke did. But the interface places a stroke
    /// at what the pick reported and sends nothing when it reports nothing, so
    /// the first stroke could never arrive: a mesh layer was unsculptable
    /// through the pointer, imported or converted, and the press orbited the
    /// camera instead. `to_mesh.rs` is the regression.
    ///
    /// A failure is swallowed rather than raised. Selecting a layer is not an
    /// edit and must not fail because of one, and the stroke path builds the
    /// sculptor itself and reports properly if it cannot.
    fn arm_mesh_sculptor(&mut self) {
        let layer = self.active_layer();
        if layer.representation != Representation::Mesh || !layer.carries_geometry {
            return;
        }
        let (key, engine_name) = (layer.key, layer.engine_name.clone());
        if let Err(e) = self.ensure_mesh_sculptor(key, &engine_name) {
            eprintln!("a malha não pôde ser preparada para escultura: {e}");
        }
    }

    /// Applies a stroke to a voxel layer, using the tool's own verb.
    fn stroke_voxel(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        let index = self.active;
        let engine_name = self.layers[index].engine_name.clone();
        let voxel_size = {
            let (_, grid) = self
                .document
                .voxel_layer(&engine_name)
                .map_err(ModelError::engine)?;
            grid.voxel_size().map_err(ModelError::engine)?
        };
        // Split the borrows by field: the mask, the layers and the document
        // are disjoint, but `&self` for one and `&mut self` for another is
        // not.
        let Self { document, mask, .. } = self;
        let brush = brush.sanitized();
        let params = BrushParams {
            size: ((brush.size / voxel_size).round() as i32).clamp(1, 64),
            shape: BrushShape::Sphere,
            falloff: match brush.shaping.falloff {
                clayspace_model::Falloff::Constant => Falloff::Constant,
                clayspace_model::Falloff::Linear => Falloff::Linear,
                clayspace_model::Falloff::Smooth => Falloff::Smooth,
                clayspace_model::Falloff::Gaussian => Falloff::Gaussian,
            },
            strength: brush.intensity,
            seed: 0,
            mask: mask.as_deref(),
        };

        // Borrowed for the length of this stroke and no longer, which is what
        // makes the document able to own it.
        let (_, mut grid) = document
            .voxel_layer(&engine_name)
            .map_err(ModelError::engine)?;

        // Index 0 is the engine's empty slot, so a fresh grid has no colour to
        // deposit and every set would write emptiness.
        let material = if grid.palette_size().map_err(ModelError::engine)? > 1 {
            1
        } else {
            grid.palette_add([0.78, 0.76, 0.73])
                .map_err(ModelError::engine)?
        };

        // Read before the loop: `alpha_for` borrows the document, and the
        // grid is borrowed mutably for the duration of the strokes.
        let alpha = self
            .alpha
            .as_ref()
            .filter(|_| brush.alpha)
            .filter(|_| {
                clayspace_model::AlphaSupport::of(Representation::Voxel, self.combine.op).accepted()
            })
            .cloned();
        let alpha = alpha.as_ref();

        let before = grid.change_count().map_err(ModelError::engine)?;

        // The same reflections a mesh stroke takes, and for the same reason: a
        // grid has no layer mirror either — `clay_set_layer_mirror` reflects a
        // layer's *items*, and a grid has cells. The mirror plane is the one
        // the cell lattice already puts at coordinate zero.
        let mirrors = mirrors(symmetry);
        for sample in samples {
            for mirror in &mirrors {
                let at = mirror.point(sample.position);
                let cell = [
                    (at[0] / voxel_size).round() as i32,
                    (at[1] / voxel_size).round() as i32,
                    (at[2] / voxel_size).round() as i32,
                ];
                let result = match tool {
                    // An alpha carve is its own entry point rather than a
                    // parameter on the others: the engine has no alpha on the
                    // ordinary voxel verbs, so a brush set to use a stamp carves
                    // with it. That is what the stamp is for on a grid — pores and
                    // fabric cut into a surface already there — and a tool that
                    // deposits would have nothing to modulate.
                    _ if alpha.is_some() => {
                        let alpha = alpha.expect("checked in the guard");
                        grid.sculpt_carve_alpha(
                            cell,
                            &params,
                            &alpha.samples,
                            alpha.width as i32,
                            alpha.height as i32,
                            // Unlike the mesh brush's block, this entry point
                            // refuses a zero-length direction outright — measured:
                            // "a null or empty grid, or a zero-length direction".
                            // So the stamp's plane is oriented by the outward
                            // normal of a roughly convex form, which is the
                            // direction from the origin to the sample.
                            outward(at),
                            material,
                        )
                    }
                    // A majority filter over the neighbourhood: spurs
                    // dissolve, notches fill. It has no sign to turn — the
                    // same reason smoothing has none on a field or a mesh.
                    ToolKind::Suavizar | ToolKind::Relaxar => grid.sculpt_smooth(cell, &params),
                    // "amount > 0 dilates, < 0 erodes", says the engine, and
                    // only the dilating half was ever asked for.
                    ToolKind::Inflar => {
                        grid.sculpt_inflate(cell, &params, if brush.invert { -1 } else { 1 })
                    }
                    // Magnify is pinch's inverse and the engine says so
                    // outright — "sharing its walk so the two cannot drift
                    // apart", the pair the SDF side spells as one signed
                    // strength. Held, the key reaches the other half.
                    ToolKind::Pincar if brush.invert => grid.sculpt_magnify(cell, &params),
                    ToolKind::Pincar => grid.sculpt_pinch(cell, &params),
                    // No opposite bound, deliberately. Turning the scrape's
                    // normal over looks like one and is not: measured on a
                    // slab, both directions remove material and differ by 12
                    // indices of 2580. The normal here is a fixed up-vector
                    // rather than the surface's own, so flipping it scrapes
                    // some other face rather than reversing the verb — and a
                    // guess dressed as a feature is worse than an honest
                    // absence.
                    ToolKind::Raspar => {
                        grid.sculpt_scrape(cell, &params, mirror.vector([0.0, 1.0, 0.0]), 0.0)
                    }
                    // At full strength, whatever Intensidade says.
                    //
                    // Every voxel verb dithers its writes against a hash of the
                    // cell coordinate when strength is below 1 — that is how a
                    // soft stamp works on binary occupancy. For a *repair* verb
                    // that is incoherent: Preencher closes a one-cell hole or it
                    // does not, and dithering means it scatters the very repairs
                    // it was asked to make. Measured, with the same perforated
                    // material: 0 cells closed at the default intensity, 6 at
                    // full strength. `voxel_tools.rs` is the regression.
                    ToolKind::Preencher => {
                        let solid = BrushParams {
                            strength: 1.0,
                            ..params
                        };
                        grid.sculpt_fill_cavities(cell, &solid, 2)
                    }
                    // The smudge direction turns over with the stroke, or the
                    // mirrored half would be dragged the same way in world space
                    // rather than as a reflection.
                    ToolKind::Nudge => {
                        grid.sculpt_smudge(cell, &params, mirror.vector([1.0, 0.0, 0.0]))
                    }
                    // Colours cells that are already there rather than depositing
                    // any: a grid's palette always exists, so this creates nothing
                    // that was not already stored — unlike on a mesh, where the
                    // colour attribute is twelve bytes a vertex and is refused
                    // rather than created.
                    ToolKind::Pintar => grid.paint_brush(cell, &params, material),
                    // The one tool whose upright verb is the removal, so its
                    // opposite is the deposit rather than the other way round.
                    ToolKind::Apagar if brush.invert => grid.set_brush(cell, &params, material),
                    ToolKind::Apagar => grid.erase_brush(cell, &params),
                    // Anything else deposits material, which is what a default
                    // brush does on a voxel grid — or takes it away, where the
                    // invert modifier is held. Occupancy is binary, so there is no
                    // sign to turn over here as there is on a field and on a mesh:
                    // the opposite of putting a cell there is removing it, which is
                    // the verb Apagar already names.
                    _ if brush.invert => grid.erase_brush(cell, &params),
                    _ => grid.set_brush(cell, &params, material),
                };
                result.map_err(ModelError::engine)?;
            }
        }

        // The count is what distinguishes a live edit from a dead one; a
        // result code cannot, because a sub-cell drag or a stamp that misses
        // every cell is a legitimate success.
        let after = grid.change_count().map_err(ModelError::engine)?;
        if after == before {
            return Ok(EditOutcome::NOTHING);
        }
        // The grid's borrow of the document ends here, so the refresh below
        // can take its own.
        let _ = grid;

        // What the panel knows about this grid is out of date the moment the
        // stroke lands: a stroke made while a pass is recording grows that
        // pass, and every stroke moves where the grid is. Both are re-read
        // here.
        //
        // Unconditional, where the pass stack alone used to be refreshed only
        // while recording. Off a recording there is no stack to walk, so this
        // costs one lookup and two counters — and the extent has to be right
        // whether or not a pass is being recorded, because Frame All does not
        // know about passes.
        let key = self.active_layer().key;
        self.refresh_sculpt_layers(key)?;

        Ok(EditOutcome {
            changed: true,
            // Voxel layers are meshed whole for now; the brick cache tracks
            // the SDF side.
            dirty_bricks: 1,
        })
    }
}

impl SculptModel for ClayDocument {
    fn active_representation(&self) -> Representation {
        self.active_layer().representation
    }

    fn active_layer_carries_geometry(&self) -> bool {
        self.active_layer().carries_geometry
    }

    fn active_layer_editable(&self) -> bool {
        self.active_layer().editable()
    }

    fn apply_stroke(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        if samples.is_empty() {
            return Ok(EditOutcome::NOTHING);
        }
        // The refusal belongs to the domain; repeating it here would let the
        // two disagree.
        tool.availability(self.active_layer_state())
            .map_err(ModelError::Unavailable)?;

        // Before the representation is asked, because a mask does not belong
        // to one. It is a world-addressed field of its own that every layer
        // consults, and `mask_stroke` touches no layer at all — so routing it
        // through the three arms only gave each of them a chance to get it
        // wrong. Two of them did: on a grid it fell through to `set_brush` and
        // *deposited clay* where the sculptor asked to freeze a region, and on
        // a mesh the tool table refused it outright though `stroke_mesh` has
        // been passing the mask to the engine all along.
        if tool == ToolKind::Mascara {
            return self.mask_stroke(brush, samples);
        }

        match self.active_representation() {
            Representation::Sdf => match tool {
                // The verbs that rewrite the field rather than adding an item.
                // The layer mirror cannot reach those, so their strokes are
                // reflected instead — see `baked_stroke`.
                ToolKind::Mover
                | ToolKind::Suavizar
                | ToolKind::Relaxar
                | ToolKind::Planar
                | ToolKind::Polir => self.baked_stroke(tool, brush, samples, symmetry),
                // Pulls a lobe out along the path, as items — so the layer
                // mirror does reach it, and pointing the mirror is the whole
                // of what symmetry means here.
                ToolKind::Puxar => {
                    self.point_the_mirror(symmetry)?;
                    self.snakehook_stroke(brush, samples)
                }
                _ => self.stroke_sdf(tool, brush, samples, symmetry),
            },
            Representation::Voxel => self.stroke_voxel(tool, brush, samples, symmetry),
            Representation::Mesh => self.stroke_mesh(tool, brush, samples, symmetry),
        }
    }

    fn set_combine(&mut self, combine: CombineSettings) {
        self.combine = combine.sanitized();
    }

    fn combine(&self) -> CombineSettings {
        self.combine
    }

    fn set_alpha(&mut self, alpha: Option<Alpha>) {
        self.alpha = alpha;
    }

    fn alpha_name(&self) -> Option<String> {
        self.alpha.as_ref().map(|alpha| alpha.name.clone())
    }

    fn apply_operation(
        &mut self,
        operation: clayspace_model::LayerOperation,
    ) -> Result<EditOutcome, ModelError> {
        if !operation.applies_to(self.active_representation()) {
            // The operation's own row, so the refusal names where it applies
            // rather than restating one representation's answer for all of
            // them — which told a sculptor on a field that filling voids
            // "applies to mesh layers".
            return Err(ModelError::Unavailable(
                clayspace_model::Unavailable::NoVerbHere {
                    active: self.active_representation(),
                    verbs: operation.verbs(),
                },
            ));
        }
        // The voxel operations reach a grid rather than a sculptor, and none
        // of them needs one built.
        if matches!(
            operation,
            clayspace_model::LayerOperation::CloseHoles { .. }
                | clayspace_model::LayerOperation::FillVoids
                | clayspace_model::LayerOperation::RefineRegion { .. }
        ) {
            return self.apply_voxel_operation(operation);
        }
        let layer = self.active_layer();
        if !layer.carries_geometry {
            return Err(ModelError::Unavailable(
                clayspace_model::Unavailable::MissingAttribute { needs: "mesh" },
            ));
        }
        let key = layer.key;
        let engine_name = layer.engine_name.clone();
        let layer_id = layer.id;
        self.ensure_mesh_sculptor(key, &engine_name)?;

        // Recorded like a stroke, because it is one edit to a sculptor and one
        // thing a user did.
        let mut deltas = claycore::MeshDeltas::new().map_err(ModelError::engine)?;
        let moved = {
            let mut held = self.mesh_sculptor.borrow_mut();
            let Some((_, sculptor)) = held.as_mut() else {
                return Ok(EditOutcome::NOTHING);
            };
            let moved = match operation {
                clayspace_model::LayerOperation::Taper {
                    axis,
                    span,
                    scale_start,
                    scale_end,
                } => sculptor.deform(
                    claycore::MeshDeformer {
                        verb: claycore::MeshDeform::Taper,
                        axis,
                        span,
                        scale_start,
                        scale_end,
                        ..claycore::MeshDeformer::default()
                    },
                    self.mask.as_ref(),
                    Some(&mut deltas),
                ),
                clayspace_model::LayerOperation::Twist { axis, span, angle } => sculptor.deform(
                    claycore::MeshDeformer {
                        verb: claycore::MeshDeform::Twist,
                        axis,
                        span,
                        angle,
                        ..claycore::MeshDeformer::default()
                    },
                    self.mask.as_ref(),
                    Some(&mut deltas),
                ),
                clayspace_model::LayerOperation::LatticeDrag {
                    divisions,
                    at,
                    offset,
                } => {
                    // The cage is built here from the layer's own bounds and
                    // the one drag being applied. Holding a cage across drags
                    // would mean the document owning a piece of interface
                    // state; rebuilding it is cheap next to walking the
                    // vertices, which happens either way.
                    // The layer's own bounds, falling back to a unit box for
                    // a layer the engine reports none for — a cage has to be
                    // somewhere, and a box around the origin is where a
                    // sculptor would expect to find one.
                    let (min, max) = self
                        .document
                        .layer_bounds(layer_id)
                        .ok()
                        .flatten()
                        .unwrap_or(([-1.0; 3], [1.0; 3]));
                    let mut lattice = claycore::MeshLattice::new(min, max, divisions)
                        .map_err(ModelError::engine)?;
                    lattice.set_offset(at, offset).map_err(ModelError::engine)?;
                    sculptor.apply_lattice(&lattice, Some(&mut deltas))
                }
                // Routed above, before a sculptor was asked for: these reach a
                // grid and none of them needs one.
                clayspace_model::LayerOperation::CloseHoles { .. }
                | clayspace_model::LayerOperation::FillVoids
                | clayspace_model::LayerOperation::RefineRegion { .. } => {
                    return Ok(EditOutcome::NOTHING)
                }
            }
            .map_err(ModelError::engine)?;
            sculptor.refit().map_err(ModelError::engine)?;
            moved
        };

        if deltas.vertex_count().map_err(ModelError::engine)? > 0 {
            self.mesh_undo.push(MeshGesture {
                layer: key,
                deltas,
                engine_depth: self.engine_undo_depth(),
            });
            self.mesh_redo.clear();
        }
        Ok(EditOutcome {
            changed: moved > 0,
            dirty_bricks: 0,
        })
    }

    fn pick(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]> {
        // A mesh layer is in neither the tape nor the brick cache, so a field
        // raycast could never see one — which is why a press on a mesh layer
        // used to orbit. It is answered by the layer's own triangles instead,
        // and only while that layer is the active one: the pointer means
        // "sculpt this" there, and picking a mesh from under an SDF layer
        // would put the cursor on something the brush cannot reach.
        if self.active_representation() == Representation::Mesh {
            return self.pick_active_mesh(origin, direction);
        }
        // A grid is in neither either, for the same reason and with the same
        // consequence: a press on a voxel layer orbited instead of sculpting,
        // because the field a raycast marches carries no voxel content. The
        // engine picks a grid itself.
        if self.active_representation() == Representation::Voxel {
            return self.pick_active_grid(origin, direction);
        }
        // Against the cache rather than the document: the cost is the ray's
        // path through the band rather than a march against the whole tape.
        self.cache
            .raycast(origin, direction)
            .ok()
            .flatten()
            .map(|hit| hit.position)
            .or_else(|| {
                self.document
                    .raycast(origin, direction)
                    .ok()
                    .flatten()
                    .map(|hit| hit.position)
            })
    }

    fn undo(&mut self) -> Result<bool, ModelError> {
        // Whichever history holds the more recent edit answers. See
        // `mesh_undo` for why depth is what orders them.
        if self.mesh_gesture_is_newest() {
            return self.undo_mesh_gesture();
        }
        let moved = self.document.undo().map_err(ModelError::engine)?;
        if moved {
            self.reconcile_layers();
            let layer = self.active_layer().id;
            // Undo can move anything the layer holds, so the bound is the
            // layer rather than a node set.
            self.refill(layer, &[])?;
            self.resync_armature();
        }
        Ok(moved)
    }

    fn redo(&mut self) -> Result<bool, ModelError> {
        // The mirror of `undo`: a mesh gesture on the redo stack recorded at
        // the current engine depth is the one that was taken back last.
        if self
            .mesh_redo
            .last()
            .is_some_and(|gesture| gesture.engine_depth == self.engine_undo_depth())
        {
            return self.redo_mesh_gesture();
        }
        let moved = self.document.redo().map_err(ModelError::engine)?;
        if moved {
            self.reconcile_layers();
            let layer = self.active_layer().id;
            self.refill(layer, &[])?;
            self.resync_armature();
        }
        Ok(moved)
    }

    fn history(&self) -> HistoryState {
        // Both histories, because the menu and the shortcut ask this one
        // question and a mesh gesture is as undoable as an engine entry. A
        // depth that counted only the engine's would grey out Undo in the
        // middle of a mesh sculpting session.
        match self.document.undo_state() {
            Ok(state) => HistoryState {
                can_undo: state.undo_depth > 0 || !self.mesh_undo.is_empty(),
                can_redo: state.redo_depth > 0 || !self.mesh_redo.is_empty(),
                depth: state.undo_depth + self.mesh_undo.len(),
                redo_depth: state.redo_depth + self.mesh_redo.len(),
            },
            Err(_) => HistoryState::default(),
        }
    }

    fn stats(&self) -> SceneStats {
        // The surface built from the brick cache, plus the layers carried
        // beside it. Reported together because they are drawn together: a
        // sculptor counting polygons wants what is on screen, and a mesh or
        // voxel layer is on screen without being in the cache.
        let (triangles, vertices) = (
            self.stats.triangles + self.carried.0,
            self.stats.vertices + self.carried.1,
        );
        SceneStats {
            triangles,
            vertices,
            objects: self.stats.objects,
            // Reported once something has been meshed; until then the
            // interface says so rather than showing a zero that reads as an
            // empty document.
            detail: if triangles == 0 {
                clayspace_model::Detail::Pending
            } else {
                self.stats.detail
            },
        }
    }

    fn begin_gesture(&mut self) {
        self.previewing = true;
    }

    fn end_gesture(&mut self) {
        self.previewing = false;
        // The tendril is finished; the next pull is its own.
        self.live_hook = None;
        // What the preview was holding becomes the edit. One record for the
        // whole drag, because every segment replaced the last rather than
        // adding to it.
        if let Some((layer, deltas)) = self.live_mesh.take() {
            self.mesh_undo.push(MeshGesture {
                layer,
                deltas,
                engine_depth: self.engine_undo_depth(),
            });
            self.mesh_redo.clear();
        }
    }

    fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let layer = self.active_layer();
        // A grid says where it is itself. `layer_bounds` answers with a
        // layer's SDF extent, which a voxel layer does not have — it reported
        // nothing for one however much material was in it, so Frame All framed
        // the default box over a sculpt that was somewhere else.
        if layer.representation == Representation::Voxel {
            return layer.voxel_bounds;
        }
        self.document.layer_bounds(layer.id).ok().flatten()
    }
}

/// The cage the interface is dragging, and the box it belongs to.
///
/// Offsets rather than positions, because that is what both engine routes
/// take and what makes an untouched cage exactly the identity. Positions are
/// derived on the way out, for the viewport and the pointer.
struct Cage {
    /// Which layer it was put around. A cage outlives neither the layer nor a
    /// change of active layer.
    layer: LayerKey,
    representation: Representation,
    min: [f32; 3],
    max: [f32; 3],
    divisions: [i32; 3],
    /// One displacement per control point, x fastest — the engine's order on
    /// both routes.
    offsets: Vec<[f32; 3]>,
    /// The points under the sculptor's hand, ascending and deduped.
    selection: Vec<usize>,
    mode: GizmoMode,
    /// The manipulator drag in progress, and where every selected point was
    /// when it started.
    ///
    /// The starting positions are kept because a drag is resolved from its
    /// anchor every frame rather than accumulated: transforming what the last
    /// frame produced compounds a rotation into a spiral and a scale into a
    /// runaway.
    dragging: Option<(GizmoDrag, Vec<[f32; 3]>)>,
}

impl Cage {
    /// Where a control point rests, before anything was dragged.
    ///
    /// An axis with a single division would divide by zero; the engine clamps
    /// divisions to at least two, and so does the domain, so the midpoint
    /// fallback is defensive rather than reachable.
    fn rest(&self, index: usize) -> [f32; 3] {
        let [nx, ny, nz] = self.divisions.map(|n| n.max(1) as usize);
        let (i, j, k) = (index % nx, (index / nx) % ny, index / (nx * ny));
        let along = |axis: usize, at: usize, n: usize| {
            let (lo, hi) = (self.min[axis], self.max[axis]);
            if n < 2 {
                (lo + hi) * 0.5
            } else {
                lo + (hi - lo) * at as f32 / (n - 1) as f32
            }
        };
        [along(0, i, nx), along(1, j, ny), along(2, k, nz)]
    }

    /// Where a control point is now.
    fn position(&self, index: usize) -> [f32; 3] {
        let rest = self.rest(index);
        let offset = self.offsets.get(index).copied().unwrap_or([0.0; 3]);
        std::array::from_fn(|axis| rest[axis] + offset[axis])
    }

    fn point_count(&self) -> usize {
        self.divisions
            .iter()
            .map(|n| (*n).max(0) as usize)
            .product()
    }

    /// Whether nothing has been dragged.
    fn is_identity(&self) -> bool {
        self.offsets
            .iter()
            .all(|offset| offset.iter().all(|axis| *axis == 0.0))
    }
}

/// The smooth mesh, in the layout the viewport holds — normals included.
///
/// `clay_voxel_mesh_smooth` carries positions, indices and per-vertex colours
/// and **no normals**: colour blends across a smooth surface, which has no
/// facet to hold one palette entry, but a normal is the host's to work out.
/// Without them the surface renders as a flat silhouette, which is what the
/// first attempt at this looked like.
///
/// Area-weighted, which is the ordinary thing and the right one here: the
/// cross product of two edges is twice the triangle's area, so summing it
/// unnormalised weights each face by how much surface it actually is.
fn smooth_geometry(mesh: &claycore::Mesh) -> ChunkGeometry {
    let positions = mesh.positions().to_vec();
    let indices = mesh.indices().to_vec();
    let colors = mesh
        .colors()
        .map(<[[f32; 3]]>::to_vec)
        .unwrap_or_else(|| vec![[1.0; 3]; positions.len()]);

    let mut normals = vec![[0.0f32; 3]; positions.len()];
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = [
            positions[triangle[0] as usize],
            positions[triangle[1] as usize],
            positions[triangle[2] as usize],
        ];
        let u: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
        let v: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
        let face = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for at in triangle {
            for axis in 0..3 {
                normals[*at as usize][axis] += face[axis];
            }
        }
    }
    for normal in normals.iter_mut() {
        let length = normal.iter().map(|c| c * c).sum::<f32>().sqrt();
        if length > 1e-9 {
            for axis in normal.iter_mut() {
                *axis /= length;
            }
        } else {
            // A vertex every one of whose faces cancelled. Nothing points
            // anywhere, and up is as good an answer as any — the alternative
            // is a zero normal, which shades as a hole.
            *normal = [0.0, 1.0, 0.0];
        }
    }

    ChunkGeometry {
        positions,
        normals,
        colors,
        indices,
    }
}

/// How a pulled tendril's points join.
///
/// Catmull-Rom, which passes through them: the curve is the path the pointer
/// took rather than a chain of straight spans between its samples.
const POINT_KIND: claycore::PointType = claycore::PointType::Spline;

/// One reflection of a stroke, through the planes of some subset of the axes.
///
/// A mesh has no layer mirror to lean on — `clay_set_layer_mirror` reflects a
/// layer's *items*, and a mesh layer has vertices instead — so symmetry here
/// is what it is in Blender and ZBrush: the stroke itself is mirrored and
/// applied again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mirror([bool; 3]);

impl Mirror {
    /// A point reflected through the planes this mirror names.
    ///
    /// Through the mesh's own origin, which is where both references put the
    /// symmetry plane and where the layer mirror puts it on a field.
    fn point(self, at: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|axis| if self.0[axis] { -at[axis] } else { at[axis] })
    }

    /// The same for a direction.
    ///
    /// A reflection is its own inverse and fixes the plane, so a vector
    /// reflects exactly as a point does — but it is spelled separately because
    /// forgetting it is the bug that makes a mirrored Grab pull the wrong way.
    fn vector(self, direction: [f32; 3]) -> [f32; 3] {
        self.point(direction)
    }
}

/// Every reflection a set of enabled axes calls for, the identity first.
///
/// Two axes give four and three give eight: the full subset lattice, which is
/// what a sculptor means by "symmetric in x and y" — the four quadrants, not
/// the two halves twice.
fn mirrors(symmetry: [bool; 3]) -> Vec<Mirror> {
    let mut out = vec![Mirror([false; 3])];
    for axis in 0..3 {
        if !symmetry[axis] {
            continue;
        }
        // Each new axis doubles the set: everything so far, and everything so
        // far reflected once more.
        out.extend(
            out.clone()
                .into_iter()
                .map(|Mirror(mut axes)| {
                    axes[axis] = true;
                    Mirror(axes)
                })
                .collect::<Vec<_>>(),
        );
    }
    out
}

/// Which engine verb a tool invokes on a mesh layer.
///
/// Here rather than on `ToolKind`, because `clayspace-model` is the domain and
/// may not depend on the engine — `tools/check_layering.py` is what keeps that
/// true. The domain's table names the verb as text and this is where the text
/// becomes a call, which is the same split every other representation uses.
fn mesh_verb(tool: ToolKind) -> Option<claycore::MeshBrush> {
    use claycore::MeshBrush;
    Some(match tool {
        ToolKind::Padrao => MeshBrush::Draw,
        ToolKind::Inflar => MeshBrush::Inflate,
        ToolKind::Suavizar => MeshBrush::Smooth,
        ToolKind::Camada => MeshBrush::Layer,
        ToolKind::Mover => MeshBrush::Grab,
        ToolKind::Puxar => MeshBrush::Snakehook,
        ToolKind::Planar => MeshBrush::Flatten,
        ToolKind::Polir => MeshBrush::Polish,
        ToolKind::Relaxar => MeshBrush::Relax,
        ToolKind::Raspar => MeshBrush::Scrape,
        ToolKind::Pincar => MeshBrush::Pinch,
        ToolKind::Nudge => MeshBrush::Nudge,
        ToolKind::Argila => MeshBrush::Clay,
        ToolKind::Vinco => MeshBrush::Crease,
        ToolKind::Pintar => MeshBrush::Paint,
        ToolKind::Borrar => MeshBrush::Smear,
        // No mesh binding: a mask stroke, a cavity fill and a frame-drawn cut
        // are not fixed-topology vertex verbs.
        // No mesh binding: a mask stroke, a cavity fill and a frame-drawn cut
        // are not fixed-topology vertex verbs, and erasing a cell would change
        // a mesh's topology, which none of these sixteen may do.
        ToolKind::Mascara | ToolKind::Preencher | ToolKind::Trim | ToolKind::Apagar => return None,
    })
}

/// Kept so the routing type is visible to readers of this module's imports.
const _: fn(Operation) -> &'static str = Operation::label;

impl SceneModel for ClayDocument {
    fn scene(&self) -> Scene {
        // The tree mirrors the layer list for now: the engine's group
        // structure is reachable through the C ABI but the document here
        // builds no groups, so a flat tree is the truthful picture rather
        // than an invented hierarchy.
        let nodes = self
            .layers
            .iter()
            .map(|layer| SceneNode {
                key: layer.key,
                name: layer.name.clone(),
                depth: 0,
                visible: layer.visible,
                expandable: false,
            })
            .collect();

        Scene {
            nodes,
            layers: self.layers.iter().map(Layer::summary).collect(),
            active: self.layers.get(self.active).map(|layer| layer.key),
            selected: self.selected,
        }
    }

    fn set_active_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        self.active = self.index_of(key)?;
        self.selected = Some(key);
        self.arm_mesh_sculptor();
        Ok(())
    }

    fn set_layer_visible(&mut self, key: LayerKey, visible: bool) -> Result<(), ModelError> {
        let index = self.index_of(key)?;
        let id = self.layers[index].id;
        self.document
            .set_layer_visible(id, visible)
            .map_err(ModelError::engine)?;
        self.layers[index].visible = visible;
        // Hiding a layer removes its contribution, so the surface moves.
        self.refill(id, &[])?;
        Ok(())
    }

    fn set_layer_protection(
        &mut self,
        key: LayerKey,
        protection: Protection,
    ) -> Result<(), ModelError> {
        let index = self.index_of(key)?;
        let id = self.layers[index].id;
        self.document
            .set_layer_protection(
                id,
                claycore::Protection {
                    ghost: protection.ghost,
                    locked: protection.locked,
                },
            )
            .map_err(ModelError::engine)?;
        self.layers[index].protection = protection;
        Ok(())
    }

    fn rename_layer(&mut self, key: LayerKey, name: &str) -> Result<(), ModelError> {
        let index = self.index_of(key)?;
        // Refused here as well as by the engine, so the message is the one the
        // interface can show. An empty name is what a cleared text field
        // submits.
        let name = name.trim();
        if name.is_empty() {
            return Err(ModelError::engine("uma camada precisa de um nome"));
        }

        // A voxel layer's grid is reachable only by name — the ABI has no
        // id-addressed accessor — and the lookup answers with the first layer
        // in stack order carrying it. So two voxel layers sharing a name would
        // shadow one another's grid, and a stroke would land on the wrong one.
        // Nothing upstream enforces this, which is why it is enforced here and
        // only where it can actually go wrong.
        if self.layers[index].representation == Representation::Voxel
            && self.layers.iter().enumerate().any(|(other, layer)| {
                other != index
                    && layer.representation == Representation::Voxel
                    && layer.engine_name == name
            })
        {
            return Err(ModelError::engine(
                "já existe uma camada de voxels com esse nome",
            ));
        }

        // Since ClayCore 0.30.0 the rename reaches the document, so it is
        // saved rather than kept beside it and lost (#92). One command, so one
        // undo step, on the same history as everything else.
        self.document
            .set_layer_name(self.layers[index].id, name)
            .map_err(ModelError::engine)?;
        self.layers[index].name = name.to_string();
        // Kept in step, because it is the handle a voxel grid is fetched with.
        self.layers[index].engine_name = name.to_string();
        Ok(())
    }

    fn add_layer(
        &mut self,
        name: &str,
        representation: Representation,
    ) -> Result<LayerKey, ModelError> {
        // A voxel representation is a different call, not a flag: the grid
        // has to be the document's or it is not saved with it.
        let id = match representation {
            Representation::Voxel => self
                .document
                .add_voxel_layer(name, Self::VOXEL_SIZE)
                .map(|(id, _)| id),
            _ => self.document.add_sdf_layer(name),
        }
        .map_err(ModelError::engine)?;
        let key = self.take_key();
        self.layers.push(Layer {
            id,
            key,
            name: name.to_string(),
            engine_name: name.to_string(),
            representation,
            carries_geometry: representation != Representation::Mesh,
            visible: true,
            protection: Protection::default(),
            intensity: 100,
            voxel_bounds: None,
            voxel_chunks: std::collections::BTreeMap::new(),
            sculpt_layers: Vec::new(),
        });
        self.active = self.layers.len() - 1;
        Ok(key)
    }

    fn apply_sculpt_layer_op(
        &mut self,
        op: clayspace_model::SculptLayerOp,
    ) -> Result<(), ModelError> {
        use clayspace_model::SculptLayerOp as Op;

        let layer = self.active_layer();
        if layer.representation != Representation::Voxel {
            // Named rather than generic: a sculptor on a field or a mesh needs
            // to know a pass is a grid's, not that "this failed".
            return Err(ModelError::Unavailable(
                clayspace_model::Unavailable::NoVerbHere {
                    active: layer.representation,
                    verbs: clayspace_model::Verbs {
                        sdf: None,
                        voxel: Some("clay_voxel_begin_sculpt_layer"),
                        mesh: None,
                    },
                },
            ));
        }
        let key = layer.key;
        let engine_name = layer.engine_name.clone();
        let mut recording = self.recording_pass;
        {
            let (_, mut grid) = self
                .document
                .voxel_layer(&engine_name)
                .map_err(ModelError::engine)?;
            match &op {
                Op::BeginRecording { name } => {
                    let name = (!name.is_empty()).then_some(name.as_str());
                    grid.begin_sculpt_layer(name).map_err(ModelError::engine)?;
                    recording = true;
                }
                Op::EndRecording => {
                    grid.end_sculpt_layer().map_err(ModelError::engine)?;
                    recording = false;
                }
                Op::SetStrength { index, strength } => grid
                    .set_sculpt_layer_strength(*index, *strength)
                    .map_err(ModelError::engine)?,
                Op::SetVisible { index, visible } => grid
                    .set_sculpt_layer_visible(*index, *visible)
                    .map_err(ModelError::engine)?,
                Op::Remove { index } => grid
                    .remove_sculpt_layer(*index)
                    .map_err(ModelError::engine)?,
                Op::MergeDown { index } => grid
                    .merge_sculpt_layer_down(*index)
                    .map_err(ModelError::engine)?,
                Op::Move { from, to } => grid
                    .move_sculpt_layer(*from, *to)
                    .map_err(ModelError::engine)?,
            }
        }

        self.recording_pass = recording;
        self.refresh_sculpt_layers(key)?;
        // Everything but starting and stopping a recording replays cells, so
        // the surface has changed and the viewport has to re-mesh it. Starting
        // one decides where the *next* edits are filed and draws nothing new.
        if op.changes_the_surface() {
            let layer_id = self
                .layers
                .iter()
                .find(|layer| layer.key == key)
                .map(|layer| layer.id);
            if let Some(id) = layer_id {
                self.refill(id, &[])?;
            }
        }
        Ok(())
    }

    fn sculpt_layer_cost(&self) -> clayspace_model::SculptLayerCost {
        let layer = self.active_layer();
        if layer.representation != Representation::Voxel {
            return clayspace_model::SculptLayerCost::default();
        }
        clayspace_model::SculptLayerCost {
            layers: layer.sculpt_layers.len(),
            bytes: layer.sculpt_layers.iter().map(|pass| pass.bytes).sum(),
            recording: self.recording_pass,
        }
    }

    fn remove_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        let index = self.index_of(key)?;
        if self.layers.len() == 1 {
            return Err(ModelError::engine(
                "a document keeps at least one layer to sculpt on",
            ));
        }
        let id = self.layers[index].id;
        // Where it was, asked while it is still there to ask.
        //
        // The cache holds the *evaluated field*, brick by brick. Removing a
        // layer takes it out of the document and leaves every brick it
        // contributed to exactly as it was, so the surface goes on being drawn
        // and goes on being picked — measured, a sphere removed from a
        // two-layer document still answered a raycast at [0, 0, 1] and still
        // meshed to the same 298,680 triangles, through an incremental sync
        // and through a full rebuild alike. Only reopening the file looked
        // right, because that builds the cache from nothing.
        //
        // Marking the *remaining* active layer is not enough and never was:
        // the stale bricks belong to the layer that left.
        let region = self.document.layer_bounds(id).ok().flatten().or_else(|| {
            // A grid keeps its extent here rather than in the engine, which
            // reports a layer's SDF bounds and a voxel layer has none.
            self.layers[index].voxel_bounds
        });

        self.document.remove_layer(id).map_err(ModelError::engine)?;
        self.layers.remove(index);
        self.active = self.active.min(self.layers.len() - 1);
        if self.selected == Some(key) {
            self.selected = None;
        }
        let active = self.active_layer().id;
        self.refill(active, &[])?;
        // Re-evaluated against the document as it is now, which is what drops
        // what the removed layer left behind. After the refill above, so the
        // two cannot fight over the same bricks.
        if let Some((min, max)) = region {
            // Padded, because a brick the surface only grazes still holds a
            // piece of it and a box drawn exactly on the bounds can miss the
            // outermost one.
            let pad = self.cache.config().voxel_size * Self::BRICK_MARGIN;
            let min = std::array::from_fn(|i| min[i] - pad);
            let max = std::array::from_fn(|i| max[i] + pad);
            self.refill_region(min, max)?;
        }
        Ok(())
    }

    fn move_layer(&mut self, key: LayerKey, index: usize) -> Result<(), ModelError> {
        let from = self.index_of(key)?;
        let to = index.min(self.layers.len().saturating_sub(1));
        if from == to {
            return Ok(());
        }
        let id = self.layers[from].id;
        self.document
            .move_layer(id, to as i32)
            .map_err(ModelError::engine)?;

        let layer = self.layers.remove(from);
        self.layers.insert(to, layer);
        // The active index follows the layer it pointed at.
        self.active = self
            .layers
            .iter()
            .position(|layer| layer.key == key)
            .unwrap_or(self.active.min(self.layers.len() - 1));
        let active = self.active_layer().id;
        self.refill(active, &[])?;
        Ok(())
    }

    fn set_layer_transform(
        &mut self,
        key: LayerKey,
        position: [f32; 3],
        scale: f32,
    ) -> Result<(), ModelError> {
        let id = self.layer_id(key)?;
        // One call, so one undo step however many items the layer holds.
        self.document
            .set_layer_transform(id, position, [0.0, 1.0, 0.0], 0.0, scale.max(1e-4))
            .map_err(ModelError::engine)?;
        self.refill(id, &[])?;
        Ok(())
    }

    fn layer_cost(&self, key: LayerKey) -> Result<clayspace_model::LayerCost, ModelError> {
        let id = self.layer_id(key)?;
        // The threshold below which the engine advises collapsing. Its own
        // note is that a chain of bakes steepens the field until a march takes
        // many small steps; this is where that becomes visible.
        let report = self
            .document
            .field_report(id, 0.5)
            .map_err(ModelError::engine)?;
        let state = self
            .document
            .consolidation_state(id)
            .map_err(ModelError::engine)?;
        let estimate = match state {
            Some(cost) => cost.bytes,
            None => self
                .document
                .consolidation_cost(id, self.consolidation_params(), None)
                .map(|cost| cost.bytes)
                .unwrap_or(0),
        };

        Ok(clayspace_model::LayerCost {
            items: report.item_count,
            safe_step_scale: report.safe_step_scale,
            advises_consolidation: report.advises_consolidation,
            estimated_bytes: estimate,
            consolidated: state.is_some(),
        })
    }

    fn consolidate_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        let id = self.layer_id(key)?;
        self.document
            .consolidate(id, self.consolidation_params(), None)
            .map_err(ModelError::engine)?;
        self.refill(id, &[])?;
        Ok(())
    }

    fn add_mesh_layer(&mut self, name: &str) -> Result<LayerKey, ModelError> {
        // Carried, not sculpted: the layer is recorded so the tools can refuse
        // it by representation rather than by a special case.
        let id = self
            .document
            .add_sdf_layer(name)
            .map_err(ModelError::engine)?;
        let key = self.take_key();
        self.layers.push(Layer {
            id,
            key,
            name: name.to_string(),
            engine_name: name.to_string(),
            representation: Representation::Mesh,
            carries_geometry: false,
            visible: true,
            protection: Protection::default(),
            intensity: 100,
            voxel_bounds: None,
            voxel_chunks: std::collections::BTreeMap::new(),
            sculpt_layers: Vec::new(),
        });
        Ok(key)
    }

    fn select_at(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<LayerKey> {
        // Attributed, because a selection has to name what it selected. The
        // engine excludes ghosted layers from picking, so honouring ghost is
        // not something this has to reimplement.
        let hit = self
            .document
            .raycast_attributed(origin, direction)
            .ok()
            .flatten();

        self.selected = hit.and_then(|hit| hit.layer).and_then(|id| {
            self.layers
                .iter()
                .find(|layer| layer.id == id)
                .map(|layer| layer.key)
        });
        self.selected
    }
}

/// The scene operations that reach further into the engine.
impl ClayDocument {
    pub fn layer_id(&self, key: LayerKey) -> Result<LayerId, ModelError> {
        self.index_of(key).map(|index| self.layers[index].id)
    }

    /// How a bake-and-replace verb samples the document.
    ///
    /// The feather is the whole of ClayCore #67. A hard `CLAY_OP_REPLACE`
    /// holds *both* fields live at the boundary: the baked volume ties with
    /// the field beneath it at every sample plane, and branch-switching
    /// between two fields that touch ripples the normals at the cell
    /// wavelength. The zero set was exact and the shading was not, which is
    /// why Suavizar, Relaxar, Planar and Polir corrugated everything they
    /// touched. With a feather the inside is the volume, the outside is the
    /// original field, and the two crossfade.
    ///
    /// One band is the engine's stated sweet spot, and the band defaults to
    /// three cells — so the feather is three cells too. Wider costs the
    /// document's safe step scale; narrower brings the tie back.
    fn bake_volume(cell: f32) -> claycore::VolumeParams {
        claycore::VolumeParams {
            cell_size: Some(cell),
            feather: Some(Self::feather_for(cell)),
            ..Default::default()
        }
    }

    /// The crossfade margin, and how far the sampled box must grow to hold it.
    ///
    /// One band — the engine's stated sweet spot, and the band defaults to
    /// three cells.
    fn feather_for(cell: f32) -> f32 {
        cell * 3.0
    }

    /// Grows a bake region so the crossfade lands in clay the verb never
    /// reached.
    ///
    /// The feather is measured *inward* from the box faces, so a box sized to
    /// the verb's own reach spends its whole margin crossfading away the very
    /// thing the verb did. Measured: Suavizar and Relaxar went from changing
    /// 15% of the subject to changing nothing at all. Padding by twice the
    /// feather puts the whole crossfade outside the verb's reach, which is
    /// what the engine means by "bake with a band that covers the verb".
    fn grown_for_feather(min: &mut [f32; 3], max: &mut [f32; 3], cell: f32) {
        let margin = Self::feather_for(cell) * 2.0;
        for axis in 0..3 {
            min[axis] -= margin;
            max[axis] += margin;
        }
    }

    /// The spacing a collapse samples at.
    ///
    /// Taken from the brick cache, which is the one place that knows the scale
    /// this document is being worked at. The engine cannot supply it: a layer
    /// has no intrinsic scale the way a mesh's bounds give one.
    fn consolidation_params(&self) -> claycore::ConsolidationParams {
        claycore::ConsolidationParams::at(self.cache.config().voxel_size)
    }
}

impl DocumentModel for ClayDocument {
    fn save(&mut self, path: &std::path::Path) -> Result<(), ModelError> {
        self.document.save(path).map_err(ModelError::engine)
    }

    fn open(&mut self, path: &std::path::Path) -> Result<(), OpenError> {
        // Built completely before anything here is touched. A failed open must
        // leave the sculptor's work exactly as it was — losing it to a
        // mistyped filename would be the worst bug this application could
        // have.
        let opened = Self::from_file(path, self.policy.clone())?;
        *self = opened;
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ModelError> {
        let fresh = Self::new(self.policy.clone()).and_then(Self::with_starting_form)?;
        *self = fresh;
        Ok(())
    }
}

impl ClayDocument {
    /// Reads a document from disk into a complete model.
    fn from_file(path: &std::path::Path, policy: BackendPolicy) -> Result<Self, OpenError> {
        let unreadable = |detail: String| OpenError::Unreadable {
            path: path.to_path_buf(),
            detail,
        };

        let document = Document::open(path).map_err(|e| match e.kind() {
            claycore::ErrorKind::NotFound => OpenError::NotFound(path.to_path_buf()),
            // The one failure a user can act on without help: the document is
            // fine and this build is behind.
            claycore::ErrorKind::ForwardVersion => OpenError::TooNew {
                path: path.to_path_buf(),
                detail: e.to_string(),
            },
            _ => unreadable(e.to_string()),
        })?;

        let ids = document
            .layer_ids()
            .map_err(|e| unreadable(e.to_string()))?;
        if ids.is_empty() {
            return Err(unreadable("it holds no layers".to_string()));
        }

        // Everything a layer is, read back rather than regenerated.
        //
        // `layer_ids` answers in stack order — evaluation order — which is the
        // half that matters for correctness: a document reopened in id order
        // could evaluate differently from the one saved. Names, visibility and
        // representation used to be lost too, so a reopened document came back
        // anonymous with every layer treated as SDF. ClayCore 0.29.0 exposes
        // all of it (#69).
        let layers: Vec<Layer> = ids
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let info = document.layer_info(*id).ok();
                // The document's own name is both what the interface shows and
                // the key `clay_document_voxel_layer` takes. A layer that was
                // never named comes back empty rather than absent, and an
                // unnamed row in the stack is worse to work with than a
                // numbered one.
                let engine_name = document
                    .layer_name(*id)
                    .ok()
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| format!("Camada {}", index + 1));
                let representation = match info.map(|i| i.representation) {
                    Some(claycore::LayerRepresentation::Voxel) => Representation::Voxel,
                    Some(claycore::LayerRepresentation::Mesh) => Representation::Mesh,
                    _ => Representation::Sdf,
                };
                Layer {
                    id: *id,
                    key: LayerKey(index as u64 + 1),
                    // A layer that was never named comes back empty rather
                    // than absent, and an unnamed row in the stack is worse to
                    // work with than a numbered one.
                    name: engine_name.clone(),
                    engine_name,
                    representation,
                    // Read back from the engine, which reports Mesh only for a
                    // layer that carries one — an unattached row exists on
                    // this side alone and never survives a reload.
                    carries_geometry: true,
                    visible: info.map(|i| i.visible).unwrap_or(true),
                    protection: info
                        .map(|i| Protection {
                            ghost: i.protection.ghost,
                            locked: i.protection.locked,
                        })
                        .unwrap_or_default(),
                    intensity: 100,
                    voxel_bounds: None,
                    voxel_chunks: std::collections::BTreeMap::new(),
                    sculpt_layers: Vec::new(),
                }
            })
            .collect();

        let cache = BrickCache::new(BrickConfig {
            dim: 8,
            voxel_size: Self::VOXEL_SIZE,
            band_voxels: 3,
            memory_budget: Some(512 * 1024 * 1024),
            colors: false,
        })
        .map_err(|e| unreadable(e.to_string()))?;

        let next_key = layers.len() as u64 + 1;
        let mut model = Self {
            document,
            layers,
            active: 0,
            cache,
            policy,
            dirty: Vec::new(),
            stats: SceneStats::default(),
            carried: (0, 0),
            live_mesh: None,
            previewing: false,
            live_generation: 0,
            meshed_chunks: 0,
            surface_brick_count: 0,
            mesh_sculptor: std::cell::RefCell::new(None),
            mesh_undo: Vec::new(),
            mesh_redo: Vec::new(),
            live_hook: None,
            lattice: None,
            voxel_display: VoxelDisplay::default(),
            voxel_blur: SmoothBlur::default(),
            voxel_smooth: std::collections::BTreeMap::new(),
            mask: None,
            cage_revision: 0,
            mask_revision: 0,
            symmetry: [false; 3],
            combine: CombineSettings::for_strokes(),
            alpha: None,
            recording_pass: false,
            next_key,
            selected: None,
            armature: None,
            armature_bounds: None,
            skin: SkinSettings::default(),
        };

        // Undo starts recording from here: opening is not something the user
        // did to the document, and it must not be undoable back into an empty
        // one.
        model
            .document
            .enable_undo()
            .map_err(|e| unreadable(e.to_string()))?;

        let ids: Vec<LayerId> = model.layers.iter().map(|layer| layer.id).collect();
        for id in ids.clone() {
            model
                .refill(id, &[])
                .map_err(|e| unreadable(e.to_string()))?;
        }

        // The recorded passes on every grid the document carries.
        //
        // Refreshed here for the same reason the rig is recovered here: the
        // stack is cached on the layer, and a layer rebuilt from a file starts
        // with an empty one. Without this a reopened document showed no passes
        // and the sculpt read as flattened — the format carries them since
        // `.clayspace` minor 10, and the whole promise of a pass is that its
        // strength stays adjustable past the end of a session.
        let keys: Vec<LayerKey> = model
            .layers
            .iter()
            .filter(|layer| layer.representation == Representation::Voxel)
            .map(|layer| layer.key)
            .collect();
        for key in keys {
            model
                .refresh_sculpt_layers(key)
                .map_err(|e| unreadable(e.to_string()))?;
        }

        // The rig, if the document carries one. Before ClayCore 0.29.0 a
        // placed armature was write-only, so a reopened document held the
        // skinned surface and nothing that could pose it (#77).
        for (index, id) in ids.into_iter().enumerate() {
            if let Some((node, tree)) = Self::recover_armature(&model.document, id) {
                model.armature_bounds = Some(Self::armature_bounds(&tree, model.skin));
                // One node, which is the whole rig: since ClayCore 0.30.0 the
                // signs travel with it, so there are no separate cutter items
                // left behind for a reader to miss (#99).
                model.armature = Some((id, vec![node], tree));
                // And that layer becomes the active one.
                //
                // `armature()` answers only for the active layer — deliberately,
                // so switching layers cannot hand the next click someone else's
                // rig — so recovering a tree onto an inactive layer recovers it
                // into somewhere nothing can see it. Reopening a document that
                // holds a rig should put you on the rig.
                model.active = index;
                break;
            }
        }

        model.refresh_stats();
        Ok(model)
    }
}

impl ExchangeModel for ClayDocument {
    fn import_mesh(
        &mut self,
        path: &std::path::Path,
        settings: ImportSettings,
    ) -> Result<(), ModelError> {
        // The format is checked before the engine is asked, so an unreadable
        // one is refused by name rather than by a decoder error naming a
        // library the sculptor has never heard of.
        match Format::of(path) {
            Some(format) if format.can_import() => {}
            Some(format) => {
                return Err(ModelError::engine(format!(
                    "o motor não lê {}; ele grava esse formato mas não o importa",
                    format.extension().to_uppercase()
                )))
            }
            None => return Err(ModelError::engine("formato desconhecido")),
        }

        // The budget is checked against the file's declared counts before
        // anything is allocated, which is the point: a malformed file can
        // claim a billion triangles.
        let mesh = Mesh::load_within(
            path,
            ImportBudget {
                max_vertices: settings.max_vertices,
                max_triangles: settings.max_triangles,
            },
        )
        .map_err(ModelError::engine)?;

        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Importado".to_string());

        match settings.becomes {
            ImportAs::Reference => self.attach_reference(&mesh, &name, settings),
            ImportAs::Clay => self.sample_into_clay(&mesh, &name, settings),
        }
    }

    fn export_mesh(
        &mut self,
        path: &std::path::Path,
        settings: ExportSettings,
    ) -> Result<(), ModelError> {
        if Format::of(path).is_none() {
            return Err(ModelError::engine("formato desconhecido"));
        }
        let params = MeshParams {
            voxel_size: Some(settings.resolution.max(1e-4)),
            resolution: 128,
            decimate_ratio: settings.decimate_to,
            mesher: match settings.mesher {
                ExportMesher::Watertight => Mesher::MarchingTetrahedra,
                ExportMesher::Fast => Mesher::SurfaceNets,
                ExportMesher::Sharp => Mesher::DualContouring,
            },
        };
        // Combined rather than `mesh`: the field alone would silently leave
        // every imported reference layer out of the file.
        let mesh = self
            .document
            .mesh_combined(params)
            .map_err(ModelError::engine)?;
        mesh.save(path).map_err(ModelError::engine)
    }

    fn has_mesh_layers(&self) -> bool {
        self.layers
            .iter()
            .any(|layer| layer.representation == Representation::Mesh)
    }
}

impl ClayDocument {
    /// Carries a mesh verbatim, on a layer of its own.
    fn attach_reference(
        &mut self,
        mesh: &Mesh,
        name: &str,
        settings: ImportSettings,
    ) -> Result<(), ModelError> {
        let id = self
            .document
            .attach_mesh_layer(
                mesh,
                &MeshLayerDesc {
                    name: name.to_string(),
                    max_vertices: settings.max_vertices,
                    max_triangles: settings.max_triangles,
                    import_scale: settings.scale,
                },
            )
            .map_err(ModelError::engine)?;

        let key = self.take_key();
        self.layers.push(Layer {
            id,
            key,
            name: name.to_string(),
            engine_name: name.to_string(),
            // Recorded as a mesh so the tools reach it by representation
            // rather than by a special case. A mesh layer is not evaluated,
            // and nothing here pretends otherwise.
            representation: Representation::Mesh,
            // This is the call that gives a mesh row its triangles, so it is
            // where the row becomes sculptable. `add_mesh_layer` records a row
            // with none, and the mesh verbs are unavailable on it until this
            // has run.
            carries_geometry: true,
            visible: true,
            protection: Protection::default(),
            intensity: 100,
            voxel_bounds: None,
            voxel_chunks: std::collections::BTreeMap::new(),
            sculpt_layers: Vec::new(),
        });
        self.refresh_stats();
        Ok(())
    }

    /// Samples a mesh into a field, on a layer of its own, so it can be
    /// sculpted from then on.
    fn sample_into_clay(
        &mut self,
        mesh: &Mesh,
        name: &str,
        settings: ImportSettings,
    ) -> Result<(), ModelError> {
        let mut item = Item::volume_from_mesh(
            mesh,
            VolumeParams {
                // The cache's own cell size, scaled the way the geometry is:
                // sampling finer than the brick cache can hold would cost time
                // for detail that is discarded on the first refill.
                cell_size: Some(Self::VOXEL_SIZE / settings.scale.max(1e-3)),
                band: None,
                padding: None,
                // No feather: an imported mesh is placed with `Op::Add`, and
                // the engine ignores the feather for every op but replace.
                feather: None,
            },
        )
        .map_err(ModelError::engine)?;
        item.set_op(Op::Add).map_err(ModelError::engine)?;

        let layer = self.add_layer(name, Representation::Sdf)?;
        let id = self.layer_id(layer)?;
        let node = self
            .document
            .add_item(id, &item)
            .map_err(ModelError::engine)?;
        self.refill(id, &[node])?;
        self.refresh_stats();
        Ok(())
    }
}

impl ClayDocument {
    /// How many cells the active grid holds, when the active layer is one.
    ///
    /// The only direct read of a grid's contents the interface has. A raycast
    /// marches the document's *field*, which a voxel layer is not in, so
    /// without this the only way to see what a grid holds is to cross it back
    /// into a field — which adds a layer and changes the thing being measured.
    /// Used by the sculpt-layer panel to say whether a pass is doing anything,
    /// and by the tests that hold that dialling one replays cells.
    pub fn occupied_cells(&mut self) -> Option<usize> {
        let layer = self.active_layer();
        if layer.representation != Representation::Voxel {
            return None;
        }
        let engine_name = layer.engine_name.clone();
        let (_, grid) = self.document.voxel_layer(&engine_name).ok()?;
        grid.occupied_count().ok()
    }

    /// Re-reads the active grid's recorded passes into the layer's cache.
    ///
    /// Called after anything that could change the stack. Cached because
    /// reading it needs a mutable borrow of the document and `scene` takes a
    /// shared one — the same reason the armature tree is kept here.
    fn refresh_sculpt_layers(&mut self, key: LayerKey) -> Result<(), ModelError> {
        let Some(index) = self.layers.iter().position(|layer| layer.key == key) else {
            return Ok(());
        };
        if self.layers[index].representation != Representation::Voxel {
            return Ok(());
        }
        let engine_name = self.layers[index].engine_name.clone();
        let (_, grid) = self
            .document
            .voxel_layer(&engine_name)
            .map_err(ModelError::engine)?;

        let count = grid.sculpt_layer_count().map_err(ModelError::engine)?;
        let mut stack = Vec::with_capacity(count);
        for layer in 0..count {
            stack.push(clayspace_model::SculptLayer {
                index: layer,
                name: grid.sculpt_layer_name(layer).unwrap_or_default(),
                strength: grid.sculpt_layer_strength(layer).unwrap_or(1.0),
                visible: grid.sculpt_layer_visible(layer).unwrap_or(true),
                cells: grid.sculpt_layer_cell_count(layer).unwrap_or(0),
                bytes: grid.sculpt_layer_bytes(layer).unwrap_or(0),
            });
        }
        // Cell (x, y, z) covers [x, x+1) per axis, so the far corner is one
        // cell past the last occupied one.
        let size = grid.voxel_size().unwrap_or(0.0);
        let extent = grid.bounds().ok().flatten().filter(|_| size > 0.0).map(
            |(min, max): ([i32; 3], [i32; 3])| {
                (
                    std::array::from_fn(|i| min[i] as f32 * size),
                    std::array::from_fn(|i| (max[i] + 1) as f32 * size),
                )
            },
        );

        self.layers[index].sculpt_layers = stack;
        self.layers[index].voxel_bounds = extent;
        Ok(())
    }
}

impl ClayDocument {
    /// Pulls the masked patch off a *grid*, as a layer of its own.
    ///
    /// `clay_voxel_mask_extrude` rather than the document's: a grid already
    /// knows which of its cells are on its surface, so resampling it into a
    /// field would cost a conversion and lose the palette. The engine states
    /// the two agree to within a voxel.
    ///
    /// What comes back is a grid the caller owns, and it becomes an SDF layer
    /// — the same kind of row the field path produces, so "Extrudar" means one
    /// thing whatever it was run on. Unblurred: a wall is a thickness, and
    /// rounding it off is the rim controls' job rather than the crossing's.
    fn extrude_from_grid(&mut self, settings: ExtrudeSettings) -> Result<(), ModelError> {
        let engine_name = self.active_layer().engine_name.clone();
        // Split by field, because the grid borrows the document exclusively
        // and the mask is a sibling.
        let Self { document, mask, .. } = self;
        let mask = mask.as_ref().expect("checked by the caller");
        let extruded = {
            let (_, grid) = document
                .voxel_layer(&engine_name)
                .map_err(ModelError::engine)?;
            grid.mask_extrude(mask, extrude_params(settings))
                .map_err(ModelError::engine)?
        };
        let id = document
            .voxel_to_layer(&extruded, "Extrusão", 0)
            .map_err(ModelError::engine)?;
        let key = self.adopt_engine_layer(id, "Extrusão", Representation::Sdf)?;
        self.after_conversion(key)?;
        Ok(())
    }
}

/// The engine's extrusion parameters, spelled once.
///
/// Both verbs take the same descriptor and disagreeing about it would be the
/// kind of drift that shows up as "the wall is a different thickness on a
/// grid".
fn extrude_params(settings: ExtrudeSettings) -> claycore::MaskExtrudeParams {
    claycore::MaskExtrudeParams {
        thickness: settings.thickness,
        side: match settings.side {
            clayspace_model::ExtrudeSide::Outward => claycore::ExtrudeSide::Outward,
            clayspace_model::ExtrudeSide::Inward => claycore::ExtrudeSide::Inward,
            clayspace_model::ExtrudeSide::Centred => claycore::ExtrudeSide::Centred,
        },
        threshold: None,
        border_round: settings.border_round,
        border_smooth: settings.border_smooth,
        // The grid's own resolution is the only one available there, and the
        // field path has always taken the default.
        cell_size: None,
    }
}

impl LatticeModel for ClayDocument {
    fn lattice(&self) -> LatticeState {
        let Some(cage) = self.lattice.as_ref() else {
            return LatticeState::default();
        };
        LatticeState {
            active: true,
            divisions: cage.divisions,
            points: (0..cage.point_count())
                .map(|at| cage.position(at))
                .collect(),
            selection: cage.selection.clone(),
            mode: cage.mode,
            rest_span: (0..3)
                .map(|axis| cage.max[axis] - cage.min[axis])
                .fold(0.0f32, f32::max),
            touched: !cage.is_identity(),
        }
    }

    fn begin_lattice(&mut self, divisions: [i32; 3]) -> Result<(), ModelError> {
        let layer = self.active_layer();
        let (key, representation) = (layer.key, layer.representation);
        if !clayspace_model::can_be_caged(representation) {
            return Err(ModelError::engine(
                "uma camada de voxels não aceita uma gaiola; \
                 converta-a para SDF ou malha primeiro",
            ));
        }
        // Sized to what the layer actually contains rather than to a fixed
        // box: a cage that does not enclose the form has control points with
        // nothing under them, and the corners a sculptor reaches for first
        // would be the ones that do least.
        let Some((min, max)) = self.caged_bounds(representation) else {
            return Err(ModelError::engine("a camada está vazia"));
        };
        // A little proud of the surface, so the cage is grabbable rather than
        // buried in the clay it is wrapped around — and so a corner point is
        // outside the form it moves, which is where ZBrush and Blender both
        // put it.
        const MARGIN: f32 = 0.05;
        let pad = (0..3)
            .map(|axis| max[axis] - min[axis])
            .fold(0.0f32, f32::max)
            * MARGIN;
        let divisions = clayspace_model::clamp_divisions(divisions, representation);
        let count = divisions.iter().map(|n| *n as usize).product();
        self.cage_revision = self.cage_revision.wrapping_add(1);
        self.lattice = Some(Cage {
            layer: key,
            representation,
            min: std::array::from_fn(|axis| min[axis] - pad),
            max: std::array::from_fn(|axis| max[axis] + pad),
            divisions,
            offsets: vec![[0.0; 3]; count],
            selection: Vec::new(),
            mode: GizmoMode::default(),
            dragging: None,
        });
        Ok(())
    }

    fn select_lattice_point(&mut self, index: Option<usize>) {
        let Some(cage) = self.lattice.as_mut() else {
            return;
        };
        let count = cage.point_count();
        cage.selection = index.filter(|at| *at < count).into_iter().collect();
        self.cage_revision = self.cage_revision.wrapping_add(1);
    }

    fn toggle_lattice_point(&mut self, index: usize) {
        let Some(cage) = self.lattice.as_mut() else {
            return;
        };
        if index >= cage.point_count() {
            return;
        }
        // Kept sorted, so `is_selected` is a search rather than a scan and the
        // pivot is the same wherever the points were clicked from.
        match cage.selection.binary_search(&index) {
            Ok(at) => {
                cage.selection.remove(at);
            }
            Err(at) => cage.selection.insert(at, index),
        }
        self.cage_revision = self.cage_revision.wrapping_add(1);
    }

    fn set_gizmo_mode(&mut self, mode: GizmoMode) {
        if let Some(cage) = self.lattice.as_mut() {
            cage.mode = mode;
        }
        self.cage_revision = self.cage_revision.wrapping_add(1);
    }

    fn begin_gizmo_drag(&mut self, handle: GizmoHandle, anchor: [f32; 3]) {
        let state = self.lattice();
        let Some(drag) = state.drag_from(handle, anchor) else {
            return;
        };
        let Some(cage) = self.lattice.as_mut() else {
            return;
        };
        let held = cage.selection.iter().map(|at| cage.position(*at)).collect();
        cage.dragging = Some((drag, held));
    }

    fn drag_gizmo(&mut self, to: [f32; 3]) -> Result<(), ModelError> {
        let Some(cage) = self.lattice.as_mut() else {
            return Ok(());
        };
        let Some((drag, held)) = cage.dragging.as_ref() else {
            return Ok(());
        };
        let (drag, held) = (*drag, held.clone());
        for (at, was) in cage.selection.clone().iter().zip(held) {
            let now = drag.apply(was, to);
            let rest = cage.rest(*at);
            cage.offsets[*at] = std::array::from_fn(|axis| now[axis] - rest[axis]);
        }
        self.cage_revision = self.cage_revision.wrapping_add(1);
        self.preview_cage();
        Ok(())
    }

    fn end_gizmo_drag(&mut self) {
        if let Some(cage) = self.lattice.as_mut() {
            cage.dragging = None;
        }
    }

    fn drag_lattice_point(&mut self, to: [f32; 3]) -> Result<(), ModelError> {
        let Some(cage) = self.lattice.as_mut() else {
            return Ok(());
        };
        // The one point in hand. A direct drag moves exactly what was grabbed
        // — a selection of several is what the manipulator is for, and moving
        // them all with one pointer would be a gizmo without the handles.
        let &[index] = cage.selection.as_slice() else {
            return Ok(());
        };
        // The offset from rest rather than an accumulation, so a drag ends
        // where the pointer ends however many frames it took and a stutter
        // does not compound.
        let rest = cage.rest(index);
        cage.offsets[index] = std::array::from_fn(|axis| to[axis] - rest[axis]);
        self.cage_revision = self.cage_revision.wrapping_add(1);
        self.preview_cage();
        Ok(())
    }

    fn apply_lattice(&mut self) -> Result<(), ModelError> {
        let Some(cage) = self.lattice.take() else {
            return Ok(());
        };
        // An untouched cage is exactly the identity, and applying one pays for
        // a pass over every vertex — or, on a field, a deformer per item — to
        // move everything by zero.
        self.cage_revision = self.cage_revision.wrapping_add(1);
        if cage.is_identity() {
            self.discard_cage_preview();
            return Ok(());
        }
        match cage.representation {
            // The preview is taken back and the cage laid down once more, this
            // time banked. Not "keep what is on screen": a preview holds the
            // deltas of one pass, and turning that into the edit would leave
            // the undo stack describing a gesture rather than a deformation.
            Representation::Mesh => {
                self.previewing = false;
                self.bend_mesh(&cage)
            }
            _ => self.bend_field(&cage),
        }
    }

    fn cancel_lattice(&mut self) {
        self.cage_revision = self.cage_revision.wrapping_add(1);
        // Whatever the preview is showing goes with the cage. Abandoning one
        // and leaving the form bent would be the opposite of what Esc means
        // everywhere else here.
        self.discard_cage_preview();
        self.previewing = false;
        self.lattice = None;
    }
}

impl ClayDocument {
    /// The box to wrap a cage around the active layer with.
    ///
    /// `bounds` answers from the layer's *SDF* extent, which a mesh layer does
    /// not have — it reported nothing for one however many triangles were in
    /// it, so the first cage over a mesh was refused as an empty layer. A mesh
    /// layer is measured from its own vertices, which is the only place its
    /// extent lives.
    fn caged_bounds(&mut self, representation: Representation) -> Option<([f32; 3], [f32; 3])> {
        if representation != Representation::Mesh {
            return self.bounds();
        }
        let name = self.active_layer().engine_name.clone();
        let (positions, _, _, _) = self.document.read_mesh_layer(&name).ok()?;
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for vertex in &positions {
            for axis in 0..3 {
                min[axis] = min[axis].min(vertex[axis]);
                max[axis] = max[axis].max(vertex[axis]);
            }
        }
        (!positions.is_empty()).then_some((min, max))
    }

    /// Bends a mesh layer through the cage, forward.
    ///
    /// Forward is why this exists on a mesh at all: a mesh already knows where
    /// its vertices are, so nothing here inverts, iterates or approximates.
    /// Recorded through `MeshDeltas` like a stroke, so the whole cage is one
    /// undo — which is the unit a sculptor thinks in, having bent the form
    /// once.
    fn bend_mesh(&mut self, cage: &Cage) -> Result<(), ModelError> {
        let index = self.index_of(cage.layer)?;
        let engine_name = self.layers[index].engine_name.clone();
        self.ensure_mesh_sculptor(cage.layer, &engine_name)?;

        let lattice = Self::cage_lattice(cage)?;

        let mut deltas = claycore::MeshDeltas::new().map_err(ModelError::engine)?;
        // What the last preview did, taken back before the cage is laid down
        // again from the mesh as it was. The lattice is *absolute* — offsets
        // from rest, evaluated against the original vertices — so applying it
        // over a surface a previous preview already bent would compound the
        // deformation on every pointer move.
        let previous = self
            .live_mesh
            .take()
            .filter(|(layer, _)| *layer == cage.layer)
            .map(|(_, deltas)| deltas);
        let moved = {
            let mut held = self.mesh_sculptor.borrow_mut();
            let Some((_, sculptor)) = held.as_mut() else {
                return Ok(());
            };
            if let Some(previous) = &previous {
                previous.revert(sculptor).map_err(ModelError::engine)?;
            }
            sculptor
                .apply_lattice(&lattice, Some(&mut deltas))
                .map_err(ModelError::engine)?
        };
        if self.previewing {
            // Held rather than banked. The cage is still up and every drag
            // replaces the last, so bending a form is one undo however many
            // times the sculptor adjusted a corner on the way.
            if moved > 0 {
                self.live_mesh = Some((cage.layer, deltas));
            }
            // What tells the viewport to look again: a mesh layer is not in
            // the brick cache, so nothing else about this edit would.
            self.live_generation = self.live_generation.wrapping_add(1);
        } else if moved > 0 {
            self.mesh_undo.push(MeshGesture {
                layer: cage.layer,
                deltas,
                engine_depth: self.engine_undo_depth(),
            });
            self.mesh_redo.clear();
        }
        self.refresh_stats();
        Ok(())
    }

    /// The cage as a claycore lattice, with every drag placed on it.
    ///
    /// One builder for the two things that need one — applying a cage to a
    /// mesh, and reading the warp back to preview one on a field — so the two
    /// cannot come to different answers about where a sculptor's corner drag
    /// went.
    fn cage_lattice(cage: &Cage) -> Result<claycore::MeshLattice, ModelError> {
        let mut lattice = claycore::MeshLattice::new(cage.min, cage.max, cage.divisions)
            .map_err(ModelError::engine)?;
        // The engine may have clamped the divisions it accepted, so the drags
        // are placed by *its* grid rather than by ours — a cage that disagreed
        // would put a sculptor's corner drag on some interior point.
        let accepted = lattice.divisions().map_err(ModelError::engine)?;
        if accepted != cage.divisions {
            return Err(ModelError::engine(format!(
                "o motor aceitou uma gaiola {accepted:?} onde esta é {:?}",
                cage.divisions
            )));
        }
        for at in 0..cage.point_count() {
            let offset = cage.offsets[at];
            if offset.iter().all(|axis| *axis == 0.0) {
                continue;
            }
            let [nx, ny, _] = cage.divisions.map(|n| n as usize);
            let coordinate = [
                (at % nx) as i32,
                ((at / nx) % ny) as i32,
                (at / (nx * ny)) as i32,
            ];
            lattice
                .set_offset(coordinate, offset)
                .map_err(ModelError::engine)?;
        }
        Ok(lattice)
    }

    /// What the cage would move each of these points by.
    ///
    /// `None` when there is no cage up, when it is untouched, or when the
    /// active layer is a mesh — a mesh previews by being deformed, and asking
    /// for displacements there would be the same work twice.
    ///
    /// This is the *forward* warp, and the field's own deformer is the inverse
    /// one. They are not the same map: the engine states the difference is
    /// under 1.5% of the drag, being a term proportional to how the basis
    /// varies along the displacement. That is a preview's error budget and not
    /// an edit's, which is exactly the trade a preview is for — the surface
    /// that lands on Deformar is the engine's, computed the engine's way.
    pub fn cage_warp(&self, points: &[[f32; 3]]) -> Option<Vec<[f32; 3]>> {
        let cage = self.lattice.as_ref()?;
        if cage.representation == Representation::Mesh || cage.is_identity() {
            return None;
        }
        let lattice = Self::cage_lattice(cage).ok()?;
        points
            .iter()
            .map(|point| lattice.displacement(*point).ok())
            .collect()
    }

    /// Changes whenever the cage does.
    ///
    /// The counterpart to `mask_revision` for the other thing that is drawn
    /// and is not geometry. A cage moves no clay until it is applied, so
    /// nothing the surface reports would tell the viewport to warp what it is
    /// already holding.
    pub fn cage_revision(&self) -> u64 {
        self.cage_revision
    }

    /// Shows what the cage would do, without committing to it.
    ///
    /// Only on a mesh. The forward route deforms vertices the sculptor already
    /// has, so a preview is one pass and taking it back is one more. The field
    /// route writes a lattice deformer into the document as an undoable edit
    /// and refills the layer's whole brick region, which is not a thing to do
    /// on every pointer move — there the cage moves live and the surface
    /// follows when it is applied.
    fn preview_cage(&mut self) {
        let Some(cage) = self.lattice.take() else {
            return;
        };
        if cage.representation == Representation::Mesh && !cage.is_identity() {
            self.previewing = true;
            if let Err(e) = self.bend_mesh(&cage) {
                eprintln!("a gaiola não pôde ser pré-visualizada: {e}");
            }
        }
        self.lattice = Some(cage);
    }

    /// Takes back whatever a preview is showing, leaving the form as it was.
    fn discard_cage_preview(&mut self) {
        let Some((_, deltas)) = self.live_mesh.take() else {
            return;
        };
        let reverted = {
            let mut held = self.mesh_sculptor.borrow_mut();
            match held.as_mut() {
                Some((_, sculptor)) => deltas.revert(sculptor),
                None => Ok(()),
            }
        };
        if let Err(e) = reverted {
            eprintln!("a pré-visualização da gaiola não pôde ser desfeita: {e}");
        }
        self.live_generation = self.live_generation.wrapping_add(1);
        self.refresh_stats();
    }

    /// Bends a field layer through the cage.
    ///
    /// A different mechanism, and the reason the two ceilings differ: the
    /// engine resolves this into one lattice deformer per item, evaluated at
    /// every sample, where the mesh route evaluates once per vertex. It is one
    /// undo step of the engine's own.
    fn bend_field(&mut self, cage: &Cage) -> Result<(), ModelError> {
        let index = self.index_of(cage.layer)?;
        let id = self.layers[index].id;
        let placed = claycore::GizmoCage {
            // The cage is already in world coordinates, so it is placed at the
            // origin unrotated and unscaled and spans the box itself. Carrying
            // the placement in the box rather than in the transform is what
            // keeps the point a sculptor dragged and the point the engine
            // evaluates the same point.
            position: [0.0; 3],
            axis: [0.0; 3],
            angle: 0.0,
            scale: 1.0,
            min: cage.min,
            max: cage.max,
            divisions: cage.divisions,
        };
        let applied = self
            .document
            .lattice_gizmo(id, placed, &cage.offsets)
            .map_err(ModelError::engine)?;
        if applied == 0 {
            return Err(ModelError::engine(
                "a gaiola não alcançou nada nesta camada",
            ));
        }
        // Every item of the layer moved, so the whole of it is dirty.
        self.refill(id, &[])?;
        Ok(())
    }
}

impl MaskModel for ClayDocument {
    fn mask_state(&self) -> MaskState {
        match &self.mask {
            Some(mask) => MaskState {
                present: true,
                painted_cells: mask.painted_count().unwrap_or(0),
            },
            None => MaskState::default(),
        }
    }

    fn apply_mask_op(&mut self, op: MaskOp) -> Result<(), ModelError> {
        // Bumped up front, and whatever the operation turns out to do: every
        // one of them changes what is frozen, and a viewport that missed one
        // would keep drawing the mask as it was. A redundant re-sample costs a
        // buffer write; a missed one is a lie on the screen.
        self.mask_revision = self.mask_revision.wrapping_add(1);
        // Clearing a mask that was never painted is a no-op rather than a
        // refusal: the menu entry is always there, and pressing it on an empty
        // mask should do the obvious nothing.
        if matches!(op, MaskOp::Clear) {
            self.mask = None;
            return Ok(());
        }

        let Some(mask) = self.mask.as_mut() else {
            return Err(ModelError::engine("não há máscara para editar"));
        };

        match op {
            MaskOp::Invert => mask.invert().map_err(ModelError::engine),
            MaskOp::Expand(steps) => mask.expand(steps.max(1)).map_err(ModelError::engine),
            MaskOp::Contract(steps) => mask.contract(steps.max(1)).map_err(ModelError::engine),
            MaskOp::Smooth(passes) => mask.smooth(passes.max(1)).map_err(ModelError::engine),
            MaskOp::InvertWithinBounds => {
                // Bounded by what the mask already covers, which is the whole
                // point: inverting a sparse mask over infinite space would
                // freeze the universe.
                let Some((min, max)) = mask.bounds().map_err(ModelError::engine)? else {
                    // Nothing painted, so nothing to be the complement of.
                    return Ok(());
                };
                // `bounds` answers in cells and `invert_within` asks in world
                // units. The box is grown by a cell on each side so the
                // boundary cells are inside it rather than on its face.
                let cell = mask.cell_size().map_err(ModelError::engine)?;
                let low = min.map(|c| (c - 1) as f32 * cell);
                let high = max.map(|c| (c + 1) as f32 * cell);
                mask.invert_within(low, high).map_err(ModelError::engine)
            }
            MaskOp::Clear => unreachable!("handled above"),
        }
    }

    fn extrude_mask(&mut self, settings: ExtrudeSettings) -> Result<(), ModelError> {
        let settings = settings.sanitized();
        let Some(mask) = self.mask.as_ref() else {
            return Err(ModelError::engine("não há máscara para extrudar"));
        };
        if mask.painted_count().unwrap_or(0) == 0 {
            return Err(ModelError::engine("a máscara está vazia"));
        }

        // Three representations, two verbs, one of them absent.
        //
        // `clay_document_mask_extrude` samples a *layer's field*, so it refuses
        // a mesh and a grid alike — "this layer has no field to extrude from",
        // which is what a sculptor got: nothing happened and nothing said why.
        // A grid has its own verb and it was never bound. A mesh has neither,
        // and the honest answer there is the route that does work.
        match self.active_representation() {
            Representation::Voxel => return self.extrude_from_grid(settings),
            Representation::Mesh => {
                return Err(ModelError::engine(
                    "uma camada de malha não tem campo para extrudar; \
                     converta-a para SDF primeiro",
                ))
            }
            Representation::Sdf => {}
        }

        let layer = self.active_layer().id;
        let item = self
            .document
            .mask_extrude(layer, mask, extrude_params(settings))
            .map_err(ModelError::engine)?;

        // Into a layer of its own. An extrusion is a new piece of geometry, not
        // an edit to the one it came from, and putting it in its own layer is
        // what lets it be moved, hidden or thrown away afterwards.
        let key = self.add_layer("Extrusão", Representation::Sdf)?;
        let index = self.index_of(key)?;
        let id = self.layers[index].id;
        let node = self
            .document
            .add_item(id, &item)
            .map_err(ModelError::engine)?;
        self.refill(id, &[node])?;
        self.refresh_stats();
        Ok(())
    }
}

impl ArmatureModel for ClayDocument {
    fn armature(&self) -> Option<Armature> {
        let (layer, _, tree) = self.armature.as_ref()?;
        // Only while it still belongs to the layer being worked on: an
        // armature on a hidden layer is not the one a click should edit.
        (*layer == self.active_layer().id).then(|| tree.clone())
    }

    fn begin_armature(&mut self, position: [f32; 3], radius: f32) -> Result<(), ModelError> {
        // A rig gets a layer of its own.
        //
        // It used to go on the active layer, which in the application is the
        // starting form — so the first ZSphere unioned into a sphere that was
        // already there, and rigging looked and behaved like ordinary
        // sculpting with a lump in the middle. The visual test did not catch
        // it because it built on an empty document; the application never has
        // one.
        //
        // A layer is also the right unit: in ZBrush a ZSphere armature is its
        // own tool, not something added to the model you were sculpting, and
        // giving it a layer is how that reads here — visible, hideable, and
        // removable without touching the sculpt.
        let key = self.add_layer("Armadura", Representation::Sdf)?;
        let layer = self.layer_id(key)?;

        // And everything else steps out of the way.
        //
        // In ZBrush a ZSphere armature is its own *tool*: you are not looking
        // at the model you were sculpting while you build one. Here the
        // starting form is a sphere of radius 1 at the origin, so a rig grown
        // at the origin is simply inside it — the first thing anyone tries
        // produces a lump and no visible rig.
        //
        // Hidden rather than removed: the sculpt is still in the document,
        // still in the layer stack, and one click brings it back. Removing it
        // would be a destructive answer to a presentation problem.
        let others: Vec<LayerKey> = self
            .layers
            .iter()
            .filter(|other| other.key != key)
            .map(|other| other.key)
            .collect();
        for other in others {
            self.set_layer_visible(other, false)?;
        }

        let tree = Armature::rooted(position, radius);
        // Grouped for the same reason a rewrite is: making a rig adds a layer
        // and places an item, and one Cmd+Z should take both back.
        self.document
            .begin_undo_group()
            .map_err(ModelError::engine)?;
        let placed = self.place_armature(layer, &tree);
        self.document.end_undo_group().map_err(ModelError::engine)?;
        self.armature = Some((layer, placed?, tree));
        Ok(())
    }

    fn add_zsphere(
        &mut self,
        parent: NodeIndex,
        position: [f32; 3],
        radius: f32,
        mirrored: bool,
    ) -> Result<NodeIndex, ModelError> {
        let Some((_, _, tree)) = self.armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        if tree.get(parent).is_none() {
            return Err(ModelError::engine("essa esfera não existe"));
        }
        let index = tree.add_child(parent, position, radius);

        // The reflection, in the same edit. The engine does this itself for a
        // placed armature; the tree is mirrored here to match, since the host
        // holds the topology.
        if mirrored {
            if let Some(reflected) = Armature::mirrored_position(position) {
                // Under the mirror of the parent where there is one, which is
                // what keeps two arms hanging off two shoulders rather than
                // both off the same one.
                let mirror_parent = self.mirror_of(parent).unwrap_or(parent);
                if let Some((_, _, tree)) = self.armature.as_mut() {
                    tree.add_child(mirror_parent, reflected, radius);
                }
            }
        }

        self.rewrite_armature()?;
        Ok(index)
    }

    fn move_zsphere(&mut self, index: NodeIndex, delta: [f32; 3]) -> Result<(), ModelError> {
        let Some((_, _, tree)) = self.armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        tree.move_subtree(index, delta);
        self.rewrite_armature()
    }

    fn resize_zsphere(&mut self, index: NodeIndex, radius: f32) -> Result<(), ModelError> {
        let Some((_, _, tree)) = self.armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        tree.set_radius(index, radius);
        self.rewrite_armature()
    }

    fn reparent_zsphere(
        &mut self,
        index: NodeIndex,
        new_parent: NodeIndex,
    ) -> Result<(), ModelError> {
        // Reparenting has no entry point of its own — the tree edits are add,
        // move, set-radius and delete — so it is done by rewriting the whole
        // node, which is what the engine does underneath for every one of them
        // anyway.
        let Some((_, _, tree)) = self.armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        tree.reparent(index, new_parent)?;
        self.rewrite_armature()
    }

    fn remove_zsphere(&mut self, index: NodeIndex) -> Result<(), ModelError> {
        let Some((_, _, tree)) = self.armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        if tree.nodes.len() <= 1 {
            return Err(ModelError::engine(
                "a armadura ficaria sem raiz; remova a camada",
            ));
        }
        if index == 0 {
            return Err(ModelError::engine("a raiz não pode ser removida"));
        }
        tree.remove(index);
        self.rewrite_armature()
    }

    fn insert_zsphere(&mut self, child: NodeIndex) -> Result<NodeIndex, ModelError> {
        let Some((_, _, tree)) = self.armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        let inserted = tree
            .insert_on_link(child)
            .ok_or_else(|| ModelError::engine("essa esfera não tem ligação"))?;
        self.rewrite_armature()?;
        Ok(inserted)
    }

    fn set_zsphere_negative(&mut self, index: NodeIndex, negative: bool) -> Result<(), ModelError> {
        let Some((_, _, tree)) = self.armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        tree.set_negative(index, negative)?;
        self.rewrite_armature()
    }

    fn set_skin(&mut self, skin: SkinSettings) -> Result<(), ModelError> {
        self.skin = skin;
        if self.armature.is_some() {
            self.rewrite_armature()?;
        }
        Ok(())
    }

    fn skin(&self) -> SkinSettings {
        self.skin
    }
}

impl ClayDocument {
    /// Builds the item and places it, returning the node that carries it.
    /// Places a rig and returns every node it made — the armature, and one
    /// subtractive sphere per negative.
    fn place_armature(
        &mut self,
        layer: LayerId,
        tree: &Armature,
    ) -> Result<Vec<NodeId>, ModelError> {
        // One item for the whole rig, signs included. Until ClayCore 0.30.0 the
        // armature primitive carried one op for the whole item, so a negative
        // sphere had to be placed as a second subtractive item over the same
        // layer — which cut a ball-shaped hole but left the membrane along its
        // links drawn, lost the sign on reload, and forced negatives to be
        // leaves. #99 made the sign a property of the node, so all of that
        // goes away and the rig is one item again.
        let mut item = Item::armature().map_err(ModelError::engine)?;

        // Radii scaled on the way out. The tree keeps what was authored, so
        // moving the thickness slider is reversible and does not quietly
        // rewrite the rig.
        let points: Vec<f32> = tree
            .nodes
            .iter()
            .flat_map(|n| {
                [
                    n.position[0],
                    n.position[1],
                    n.position[2],
                    self.skin.radius_for(n.radius),
                ]
            })
            .collect();
        item.set_stroke_points(&points)
            .map_err(ModelError::engine)?;

        let parents: Vec<u32> = tree.nodes.iter().map(|n| n.parent).collect();
        item.set_armature_parents(&parents)
            .map_err(ModelError::engine)?;

        // The sign half. The engine builds the positive armature minus the
        // negative one, so a link between two nodes of different signs does
        // not exist — which is the membrane cut — and a carve never sweeps a
        // positive parent's radius.
        item.set_armature_signs(&tree.signs())
            .map_err(ModelError::engine)?;

        // No blend term: `clay_item_set_stroke_blend_k` refuses an armature
        // ("stroke points need CLAY_PRIM_STROKE"). The skin is the cones
        // between the spheres, so thickness lives in the radii above.
        item.set_op(Op::Add).map_err(ModelError::engine)?;

        let node = self
            .document
            .add_item(layer, &item)
            .map_err(ModelError::engine)?;
        let placed = vec![node];

        // Bounds over the whole tree, negatives included: they are what the
        // vacated box has to cover when a rig is rewritten.
        self.armature_bounds = Some(Self::armature_bounds(tree, self.skin));
        self.refill(layer, &placed)?;
        self.refresh_stats();
        Ok(placed)
    }

    /// Brings the layer list back in line with the document.
    ///
    /// Undo moves layers as well as geometry — starting a rig adds one, so
    /// undoing past that removes it — and this list is the host's own record.
    /// Left alone it kept a layer the document no longer had, and the next
    /// refill asked the engine to mark a layer that was not there.
    ///
    /// Keys are preserved for ids that survived, because a `LayerKey` is the
    /// stable handle the interface holds and renumbering it would move the
    /// selection out from under a panel. A layer that comes *back* — a redo of
    /// its creation — is rebuilt from what the document says it is, which is
    /// only answerable at all since ClayCore 0.29.0 (#69).
    fn reconcile_layers(&mut self) {
        let Ok(ids) = self.document.layer_ids() else {
            return;
        };
        let active_id = self.layers.get(self.active).map(|layer| layer.id);

        // Moved out rather than cloned. A surviving layer carries its meshed
        // chunks, which are megabytes on a worked grid, and this runs on every
        // undo — copying them would make taking one step back cost more than
        // the step did.
        let mut kept: std::collections::HashMap<LayerId, Layer> = std::mem::take(&mut self.layers)
            .into_iter()
            .map(|layer| (layer.id, layer))
            .collect();

        let mut rebuilt = Vec::with_capacity(ids.len());
        for id in &ids {
            if let Some(known) = kept.remove(id) {
                rebuilt.push(known);
                continue;
            }
            let info = self.document.layer_info(*id).ok();
            let name = self
                .document
                .layer_name(*id)
                .ok()
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("Camada {}", rebuilt.len() + 1));
            rebuilt.push(Layer {
                id: *id,
                key: self.take_key(),
                name: name.clone(),
                engine_name: name,
                representation: match info.map(|i| i.representation) {
                    Some(claycore::LayerRepresentation::Voxel) => Representation::Voxel,
                    Some(claycore::LayerRepresentation::Mesh) => Representation::Mesh,
                    _ => Representation::Sdf,
                },
                // As above: the engine's own answer, so a Mesh row here has
                // triangles behind it.
                carries_geometry: true,
                visible: info.map(|i| i.visible).unwrap_or(true),
                protection: info
                    .map(|i| Protection {
                        ghost: i.protection.ghost,
                        locked: i.protection.locked,
                    })
                    .unwrap_or_default(),
                intensity: 100,
                voxel_bounds: None,
                voxel_chunks: std::collections::BTreeMap::new(),
                sculpt_layers: Vec::new(),
            });
        }

        self.layers = rebuilt;
        // The layer that was active, if it is still there; otherwise the last
        // one, which is where a removal leaves you in every panel of this kind.
        self.active = active_id
            .and_then(|id| self.layers.iter().position(|layer| layer.id == id))
            .unwrap_or_else(|| self.layers.len().saturating_sub(1));
    }

    /// Re-reads the rig from the document after history moved underneath it.
    ///
    /// The tree is host state and undo is the engine's, so an undone rig edit
    /// would otherwise leave the two disagreeing — the document holding one
    /// shape and this holding the one that was just taken back, with the next
    /// drag written against the wrong indices.
    ///
    /// Re-reading rather than keeping a parallel stack of snapshots: since
    /// ClayCore 0.29.0 the document can be asked what the tree is (#77), so it
    /// stays the single source of truth and there is no second history to keep
    /// in step with the first.
    fn resync_armature(&mut self) {
        let Some(layer) = self.armature.as_ref().map(|(l, _, _)| *l) else {
            return;
        };
        // Where the rig was before history moved it. Refilling the layer alone
        // is not enough: a rig that shrank leaves surface outside its new
        // bounds, and nothing marks those bricks — the same debt a rewrite
        // pays with `refill_region`.
        let vacated = self.armature_bounds;
        match Self::recover_armature(&self.document, layer) {
            Some((node, tree)) => {
                self.armature_bounds = Some(Self::armature_bounds(&tree, self.skin));
                self.armature = Some((layer, vec![node], tree));
            }
            // Undone past the rig's own creation: there is no armature now,
            // and saying so is what stops the next click editing a ghost.
            None => {
                self.armature = None;
                self.armature_bounds = None;
            }
        }
        if let Some((min, max)) = vacated {
            if let Err(e) = self.refill_region(min, max) {
                // Not fatal: the geometry is stale rather than wrong, and the
                // next edit or settle clears it. Worth saying, though.
                eprintln!("a região da armadura não pôde ser remalhada: {e}");
            }
        }
    }

    /// Finds a layer's armature and reads its tree back.
    ///
    /// Node ids are probed rather than enumerated, because nothing in the ABI
    /// lists a layer's nodes: `clay_layer_children` answers for a group and a
    /// layer's root is not one. The probe is a *checkable* guess, unlike the
    /// one that used to find layers — `clay_layer_node_prim` says exactly what
    /// each id carries, so a hit is certain and only a miss is possible. What
    /// it can miss is a rig placed beyond a long run of removed nodes, which
    /// costs the tree and not the surface.
    fn recover_armature(document: &Document, layer: LayerId) -> Option<(NodeId, Armature)> {
        // Enumerated since ClayCore 0.30.0 (#91). This used to probe ids
        // upward and give up after sixteen consecutive misses, which is a
        // guess about how long a gap can be: ids are not dense, a removal
        // leaves a gap, and nothing bounds one — so the probe lost every node
        // past the longest run it happened to tolerate, and no value of
        // "long enough" was defensible.
        document
            .layer_nodes(layer)
            .ok()?
            .into_iter()
            .filter(|node| {
                document
                    .node_prim(layer, *node)
                    .is_ok_and(|prim| prim == claycore::prim::ARMATURE)
            })
            .find_map(|node| Some((node, Self::read_armature(document, layer, node)?)))
    }

    /// The tree behind a placed armature node.
    ///
    /// Radii are divided by the skin thickness on the way in, because
    /// `place_armature` multiplies by it on the way out — the tree keeps what
    /// was authored so the thickness slider stays reversible. A document is
    /// loaded with the default thickness, so this is a division by one today
    /// and correct if that ever stops being true.
    fn read_armature(document: &Document, layer: LayerId, node: NodeId) -> Option<Armature> {
        let points = document.stroke_points(layer, node).ok()?;
        let parents = document.armature_parents(layer, node).ok()?;
        if points.is_empty() || parents.len() != points.len() {
            return None;
        }
        // The signs, which ClayCore 0.30.0 made readable (#99). A rig saved
        // before signs existed reads back positive-padded rather than failing,
        // and so does one whose signs are all positive — the engine stores the
        // reading compilation makes, so a short array is padded here the same
        // way it is there.
        let signs = document.armature_signs(layer, node).unwrap_or_default();
        let skin = SkinSettings::default();
        let nodes = points
            .iter()
            .zip(parents.iter())
            .enumerate()
            .map(|(index, (point, parent))| clayspace_model::Zsphere {
                position: [point[0], point[1], point[2]],
                negative: signs.get(index).copied().unwrap_or(false),
                radius: if skin.thickness > 0.0 {
                    point[3] / skin.thickness
                } else {
                    point[3]
                },
                parent: *parent,
            })
            .collect();
        Some(Armature { nodes })
    }

    /// The box a tree occupies, spheres and all.
    fn armature_bounds(tree: &Armature, skin: SkinSettings) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for node in &tree.nodes {
            let r = skin.radius_for(node.radius);
            for axis in 0..3 {
                min[axis] = min[axis].min(node.position[axis] - r);
                max[axis] = max[axis].max(node.position[axis] + r);
            }
        }
        if !min[0].is_finite() {
            return ([0.0; 3], [0.0; 3]);
        }
        (min, max)
    }

    /// Replaces the placed armature with what the tree now says.
    ///
    /// Every edit goes through here rather than through
    /// `clay_layer_armature_edit`, for one reason: reparenting has no op there,
    /// and a rig that could do four of its five edits one way and the fifth
    /// another would be two code paths to keep in step. The engine's own
    /// implementation of those ops is a whole-tree replace, so this costs what
    /// they cost.
    fn rewrite_armature(&mut self) -> Result<(), ModelError> {
        let Some((layer, nodes, tree)) = self.armature.take() else {
            return Ok(());
        };
        // Where it was, before it is replaced by where it now is.
        let vacated = self.armature_bounds;

        // One undoable action, however many engine commands it takes. A rig
        // edit is a remove and a place — and a place is several items once
        // there are negatives — so without the group a single drag would need
        // four undos to come back.
        self.document
            .begin_undo_group()
            .map_err(ModelError::engine)?;
        let result = (|| -> Result<Vec<NodeId>, ModelError> {
            for node in &nodes {
                self.document
                    .remove_node(layer, *node)
                    .map_err(ModelError::engine)?;
            }
            self.place_armature(layer, &tree)
        })();
        self.document.end_undo_group().map_err(ModelError::engine)?;

        let fresh = result?;
        self.armature = Some((layer, fresh, tree));

        // An edit that shrinks the rig leaves its old surface behind
        // otherwise: placing the new node refills the region it occupies, and
        // nothing tells the bricks the old one used that anything changed.
        if let Some((min, max)) = vacated {
            self.refill_region(min, max)?;
        }
        Ok(())
    }

    /// The node reflecting `index` through x = 0, if the tree holds one.
    fn mirror_of(&self, index: NodeIndex) -> Option<NodeIndex> {
        let (_, _, tree) = self.armature.as_ref()?;
        let node = tree.get(index)?;
        let target = Armature::mirrored_position(node.position)?;
        tree.nodes
            .iter()
            .position(|other| (0..3).all(|axis| (other.position[axis] - target[axis]).abs() < 1e-4))
            .map(|i| i as NodeIndex)
    }
}
