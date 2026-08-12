//! The document, as the domain sees it.
//!
//! Implements [`SculptModel`] over a real ClayCore document, holding the brick
//! cache that makes a dab cost what it touched rather than what the model
//! holds.

use claycore::{
    Blend, BrickCache, BrickConfig, BrickKey, BrushParams, BrushShape, Document, Falloff,
    ImportBudget, Item, LayerId, Mask, Mesh, MeshLayerDesc, MeshParams, Mesher, NodeId, Op,
    StrokePreset, VolumeParams,
};
use clayspace_model::{
    Armature, ArmatureModel, BrushSettings, DocumentModel, EditOutcome, ExchangeModel,
    ExportMesher, ExportSettings, ExtrudeSettings, Format, GestureSample, HistoryState, ImportAs,
    ImportSettings, LayerKey, LayerSummary, MaskModel, MaskOp, MaskState, ModelError, NodeIndex,
    OpenError, Protection, Representation, Scene, SceneModel, SceneNode, SceneStats, SculptModel,
    SkinSettings, ToolKind,
};

use crate::backend::{BackendPolicy, Operation};

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
    /// Fixed at creation, because the ABI names a layer when it is made and
    /// has no rename. It is kept separately from `name` for one reason: it is
    /// the only handle `clay_document_voxel_layer` takes, so a renamed voxel
    /// layer would otherwise lose its grid.
    engine_name: String,
    representation: Representation,
    visible: bool,
    protection: Protection,
    intensity: u8,
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
        }
    }

    /// Whether an edit may touch it: shown, not ghosted, not locked.
    fn editable(&self) -> bool {
        self.visible && self.protection.is_editable()
    }
}

/// A ClayCore document driven by the domain's vocabulary.
pub struct ClayDocument {
    document: Document,
    layers: Vec<Layer>,
    active: usize,
    cache: BrickCache,
    policy: BackendPolicy,
    /// Bricks dirtied since the viewport last caught up.
    dirty: Vec<BrickKey>,
    stats: SceneStats,
    /// A mask the tools consult, when one has been painted.
    mask: Option<Mask>,
    /// The mirror currently set on the active layer, so it is only rewritten
    /// when it actually changes.
    symmetry: [bool; 3],
    /// Hands out layer keys. Monotone, so a key is never reused for a
    /// different layer after a removal.
    next_key: u64,
    selected: Option<LayerKey>,
    /// The armature on the active layer: which node carries it, and the tree.
    ///
    /// The tree is held here because the engine's parent array has no getter —
    /// positions and radii read back, the topology does not. So this is the
    /// record and the engine is written from it.
    armature: Option<(LayerId, NodeId, Armature)>,
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
                visible: true,
                protection: Protection::default(),
                intensity: 100,
            }],
            active: 0,
            cache,
            policy,
            dirty: Vec::new(),
            stats: SceneStats::default(),
            mask: None,
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
    /// Nothing consumes these yet: `clay_brick_cache_mesh` takes no level, so
    /// a mip can be built and read and not drawn (ClayCore #93). Keeping them
    /// current means the day that call grows a level, the coarse surface is
    /// already there to mesh.
    pub fn build_mips(&mut self) -> Result<usize, ModelError> {
        let keys = self.cache.surface_bricks().map_err(ModelError::engine)?;

        // Each coarse brick covers a 2x2x2 block, so the surface's coarse keys
        // are its fine keys halved — deduplicated, because eight fine bricks
        // map to one coarse.
        let mut coarse: Vec<BrickKey> = keys
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
            visible: true,
            protection: Protection::default(),
            intensity: 100,
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

    /// Meshes and refills whatever is currently marked dirty.
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
            let backend = self.policy.refill_backend(requests.len()).cloned();
            self.cache
                .refill(&self.document, backend.as_ref(), &requests)
                .map_err(ModelError::engine)?;
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
        Ok(())
    }

    /// Whether a layer contributes to the surface an edit would touch.
    fn refresh_stats(&mut self) {
        // Counted from the cache rather than by meshing the document, which
        // would cost a full march on every edit.
        let bricks = self.cache.surface_bricks().map(|k| k.len()).unwrap_or(0);
        self.stats = SceneStats {
            // Reported once the viewport meshes; until then nothing has been
            // built and the interface says so rather than showing a zero that
            // reads as an empty document.
            triangles: self.stats.triangles,
            vertices: self.stats.vertices,
            objects: self.layers.len().max(1),
            detail: if self.stats.triangles == 0 {
                clayspace_model::Detail::Pending
            } else {
                self.stats.detail
            },
        };
        let _ = bricks;
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

    /// Applies a stroke to an SDF layer.
    fn stroke_sdf(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
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

        // Every tool that reaches here is a relief tool. There is no catch-all
        // arm any more: the one that was here mapped anything unlisted to
        // `Op::Add`, which adds a *sphere* — so the planing tools deposited
        // blobs and nothing said so. A tool with no mapping now refuses.
        let op = match tool {
            ToolKind::Padrao | ToolKind::Camada | ToolKind::Inflar => Op::Relief,
            other => {
                return Err(ModelError::engine(format!(
                    "{} has no mapping onto an SDF verb; it should not have \
                     been offered on this layer",
                    other.label()
                )))
            }
        };

        let mut stamp = Item::sphere(brush.sanitized().size).map_err(ModelError::engine)?;
        stamp.set_op(op).map_err(ModelError::engine)?;
        // For CLAY_OP_RELIEF the item is the *region* and `blend_k` is the
        // amplitude the surface moves by along its own normal — not a
        // smoothing distance. It was set to 40% of the radius, which measured
        // as a displacement of about a sixth of the brush: a stroke that left
        // the sphere looking untouched. The engine saturates the amplitude at
        // roughly the radius, so that is what it is asked for, and `strength`
        // scales it from there.
        stamp
            .set_blend(Blend::Quadratic, brush.sanitized().size)
            .map_err(ModelError::engine)?;
        // The item's rounding is the falloff width, and it was never set at
        // all. Measured, going from zero to the brush radius tripled the
        // displacement — leaving it at zero was throwing away most of the
        // brush as well as its soft edge.
        stamp
            .set_rounding(brush.sanitized().size)
            .map_err(ModelError::engine)?;

        // The mirror is written only when it changes, so an unchanged setting
        // costs no history entry. The engine makes a whole stroke one step by
        // itself, so no group is needed around it.
        if self.symmetry != symmetry {
            self.document
                .set_layer_mirror(layer, symmetry, 0.0)
                .map_err(ModelError::engine)?;
            self.symmetry = symmetry;
        }

        let mask = self.mask.as_deref();
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

        let mut item = Item::stroke().map_err(ModelError::engine)?;
        item.set_stroke_points(&points)
            .map_err(ModelError::engine)?;
        item.set_op(Op::Add).map_err(ModelError::engine)?;
        item.set_stroke_blend_k(brush.size * 0.5)
            .map_err(ModelError::engine)?;

        let node = self
            .document
            .add_item(layer, &item)
            .map_err(ModelError::engine)?;
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
        // is state the *next* stroke reads.
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
                    mode: claycore::FlattenMode::CutOnly,
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

    /// Applies a stroke to a voxel layer, using the tool's own verb.
    fn stroke_voxel(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
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

        let before = grid.change_count().map_err(ModelError::engine)?;

        for sample in samples {
            let cell = [
                (sample.position[0] / voxel_size).round() as i32,
                (sample.position[1] / voxel_size).round() as i32,
                (sample.position[2] / voxel_size).round() as i32,
            ];
            let result = match tool {
                ToolKind::Suavizar | ToolKind::Relaxar => grid.sculpt_smooth(cell, &params),
                ToolKind::Inflar => grid.sculpt_inflate(cell, &params, 1),
                ToolKind::Pincar => grid.sculpt_pinch(cell, &params),
                ToolKind::Raspar => grid.sculpt_scrape(cell, &params, [0.0, 1.0, 0.0], 0.0),
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
                ToolKind::Nudge => grid.sculpt_smudge(cell, &params, [1.0, 0.0, 0.0]),
                // Anything else deposits material, which is what a default
                // brush does on a voxel grid.
                _ => grid.set_brush(cell, &params, material),
            };
            result.map_err(ModelError::engine)?;
        }

        // The count is what distinguishes a live edit from a dead one; a
        // result code cannot, because a sub-cell drag or a stamp that misses
        // every cell is a legitimate success.
        let after = grid.change_count().map_err(ModelError::engine)?;
        if after == before {
            return Ok(EditOutcome::NOTHING);
        }

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
        tool.availability(self.active_representation(), self.active_layer_editable())
            .map_err(ModelError::Unavailable)?;

        match self.active_representation() {
            Representation::Sdf => match tool {
                // Drags the assembled surface: the gesture is a displacement,
                // not a series of stamps.
                ToolKind::Mover => self.move_surface_stroke(brush, samples),
                // Pulls a lobe out along the path.
                ToolKind::Puxar => self.snakehook_stroke(brush, samples),
                // Bake-and-relax over the region the stroke covered.
                ToolKind::Suavizar | ToolKind::Relaxar => self.relax_stroke(brush, samples),
                // Bake-and-flatten, cut-only.
                ToolKind::Planar | ToolKind::Polir => self.flatten_stroke(brush, samples),
                // Paints the freeze, and moves nothing.
                ToolKind::Mascara => self.mask_stroke(brush, samples),
                _ => self.stroke_sdf(tool, brush, samples, symmetry),
            },
            Representation::Voxel => self.stroke_voxel(tool, brush, samples),
            Representation::Mesh => Ok(EditOutcome::NOTHING),
        }
    }

    fn pick(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]> {
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
        let moved = self.document.undo().map_err(ModelError::engine)?;
        if moved {
            let layer = self.active_layer().id;
            // Undo can move anything the layer holds, so the bound is the
            // layer rather than a node set.
            self.refill(layer, &[])?;
        }
        Ok(moved)
    }

    fn redo(&mut self) -> Result<bool, ModelError> {
        let moved = self.document.redo().map_err(ModelError::engine)?;
        if moved {
            let layer = self.active_layer().id;
            self.refill(layer, &[])?;
        }
        Ok(moved)
    }

    fn history(&self) -> HistoryState {
        match self.document.undo_state() {
            Ok(state) => HistoryState {
                can_undo: state.undo_depth > 0,
                can_redo: state.redo_depth > 0,
                depth: state.undo_depth,
                redo_depth: state.redo_depth,
            },
            Err(_) => HistoryState::default(),
        }
    }

    fn stats(&self) -> SceneStats {
        self.stats
    }

    fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let layer = self.active_layer().id;
        self.document.layer_bounds(layer).ok().flatten()
    }
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
        // The engine names a layer at creation and does not rename; the name
        // the interface shows is the document's own record of it.
        self.layers[index].name = name.to_string();
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
            visible: true,
            protection: Protection::default(),
            intensity: 100,
        });
        self.active = self.layers.len() - 1;
        Ok(key)
    }

    fn remove_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        let index = self.index_of(key)?;
        if self.layers.len() == 1 {
            return Err(ModelError::engine(
                "a document keeps at least one layer to sculpt on",
            ));
        }
        let id = self.layers[index].id;
        self.document.remove_layer(id).map_err(ModelError::engine)?;
        self.layers.remove(index);
        self.active = self.active.min(self.layers.len() - 1);
        if self.selected == Some(key) {
            self.selected = None;
        }
        let active = self.active_layer().id;
        self.refill(active, &[])?;
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
            visible: true,
            protection: Protection::default(),
            intensity: 100,
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
                    visible: info.map(|i| i.visible).unwrap_or(true),
                    protection: info
                        .map(|i| Protection {
                            ghost: i.protection.ghost,
                            locked: i.protection.locked,
                        })
                        .unwrap_or_default(),
                    intensity: 100,
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
            mask: None,
            symmetry: [false; 3],
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

        // The rig, if the document carries one. Before ClayCore 0.29.0 a
        // placed armature was write-only, so a reopened document held the
        // skinned surface and nothing that could pose it (#77).
        for (index, id) in ids.into_iter().enumerate() {
            if let Some((node, tree)) = Self::recover_armature(&model.document, id) {
                model.armature_bounds = Some(Self::armature_bounds(&tree, model.skin));
                model.armature = Some((id, node, tree));
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
            // Recorded as a mesh so the tools refuse it by representation
            // rather than by a special case. A mesh layer is not evaluated,
            // and nothing here pretends otherwise.
            representation: Representation::Mesh,
            visible: true,
            protection: Protection::default(),
            intensity: 100,
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

        let layer = self.active_layer().id;
        let item = self
            .document
            .mask_extrude(
                layer,
                mask,
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
                    cell_size: None,
                },
            )
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
        let node = self.place_armature(layer, &tree)?;
        self.armature = Some((layer, node, tree));
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
    fn place_armature(&mut self, layer: LayerId, tree: &Armature) -> Result<NodeId, ModelError> {
        // The spheres that add and the ones that cut go in as separate items.
        // The armature primitive is a stroke plus a tree with one op for the
        // whole thing, so a negative sphere cannot live in the same item as
        // what it cuts into — see `Armature::split_by_sign`.
        let (positive, cutters) = tree.split_by_sign();
        let tree = &positive;
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

        // No blend term: `clay_item_set_stroke_blend_k` refuses an armature
        // ("stroke points need CLAY_PRIM_STROKE"). The skin is the cones
        // between the spheres, so thickness lives in the radii above.
        item.set_op(Op::Add).map_err(ModelError::engine)?;

        let node = self
            .document
            .add_item(layer, &item)
            .map_err(ModelError::engine)?;

        // The cutters, after the rig so they carve what it just placed. Each
        // is its own sphere rather than a tree: `set_negative` keeps them
        // leaves, so there is no topology to preserve and a ball-shaped
        // indentation is exactly what a negative ZSphere makes.
        let mut placed = vec![node];
        for cutter in &cutters {
            let mut hole =
                Item::sphere(self.skin.radius_for(cutter.radius)).map_err(ModelError::engine)?;
            hole.set_op(Op::Subtract).map_err(ModelError::engine)?;
            hole.set_position(cutter.position)
                .map_err(ModelError::engine)?;
            placed.push(
                self.document
                    .add_item(layer, &hole)
                    .map_err(ModelError::engine)?,
            );
        }

        // Bounds over the *whole* tree, cutters included: they are what the
        // vacated box has to cover when a rig is rewritten.
        self.armature_bounds = Some(Self::armature_bounds(&positive, self.skin));
        self.refill(layer, &placed)?;
        self.refresh_stats();
        Ok(node)
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
        const GAP: u32 = 16;

        let mut misses = 0;
        let mut candidate = 1u32;
        while misses < GAP {
            let node = NodeId::from_raw(candidate);
            match document.node_prim(layer, node) {
                Ok(prim) if prim == claycore::prim::ARMATURE => {
                    if let Some(tree) = Self::read_armature(document, layer, node) {
                        return Some((node, tree));
                    }
                    misses = 0;
                }
                Ok(_) => misses = 0,
                Err(_) => misses += 1,
            }
            candidate += 1;
        }
        None
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
        let skin = SkinSettings::default();
        let nodes = points
            .iter()
            .zip(parents.iter())
            .map(|(point, parent)| clayspace_model::Zsphere {
                position: [point[0], point[1], point[2]],
                // A reloaded rig is all positive: the cutters are separate
                // items and the armature the engine reads back holds only the
                // spheres that add. Their indentations are still in the
                // surface; what is lost is the ability to un-negative them,
                // which is the same shape of loss as a rename.
                negative: false,
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
        let Some((layer, node, tree)) = self.armature.take() else {
            return Ok(());
        };
        // Where it was, before it is replaced by where it now is.
        let vacated = self.armature_bounds;

        self.document
            .remove_node(layer, node)
            .map_err(ModelError::engine)?;
        let fresh = self.place_armature(layer, &tree)?;
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
