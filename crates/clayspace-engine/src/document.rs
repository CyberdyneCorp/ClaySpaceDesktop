//! The document, as the domain sees it.
//!
//! Implements [`SculptModel`] over a real ClayCore document, holding the brick
//! cache that makes a dab cost what it touched rather than what the model
//! holds.

use claycore::{
    Blend, BrickCache, BrickConfig, BrickKey, BrushParams, BrushShape, Document, Falloff, Item,
    LayerId, Mask, NodeId, Op, StrokePreset, VoxelGrid,
};
use clayspace_model::{
    BrushSettings, EditOutcome, GestureSample, HistoryState, ModelError, Representation,
    SceneStats, SculptModel, ToolKind,
};

use crate::backend::{BackendPolicy, Operation};

/// A layer the document holds, and what it is made of.
struct Layer {
    id: LayerId,
    representation: Representation,
    /// Voxel layers carry their own grid; SDF layers do not.
    grid: Option<VoxelGrid>,
    editable: bool,
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
}

impl ClayDocument {
    /// Builds a document with one SDF layer holding a starting form.
    pub fn new(policy: BackendPolicy) -> Result<Self, ModelError> {
        let mut document = Document::new().map_err(ModelError::engine)?;
        let id = document.add_sdf_layer("Forma").map_err(ModelError::engine)?;
        // Before undo starts recording: the starting mirror is part of making
        // the document, not something a user did. Setting it afterwards makes
        // the first stroke cost two undos where later ones cost one.
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
            voxel_size: 0.02,
            band_voxels: 3,
            memory_budget: Some(512 * 1024 * 1024),
            colors: false,
        })
        .map_err(ModelError::engine)?;

        let mut model = Self {
            document,
            layers: vec![Layer {
                id,
                representation: Representation::Sdf,
                grid: None,
                editable: true,
            }],
            active: 0,
            cache,
            policy,
            dirty: Vec::new(),
            stats: SceneStats::default(),
            mask: None,
            symmetry,
        };
        model.refresh_stats();
        Ok(model)
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
        // The document lends its grid, but the borrow would hold the document
        // for as long as the layer lives. A standalone grid the layer owns is
        // simpler here and gives the same behaviour to the tools.
        let id = self
            .document
            .add_sdf_layer(name)
            .map_err(ModelError::engine)?;
        let grid = VoxelGrid::new(voxel_size).map_err(ModelError::engine)?;
        self.layers.push(Layer {
            id,
            representation: Representation::Voxel,
            grid: Some(grid),
            editable: true,
        });
        self.active = self.layers.len() - 1;
        Ok(())
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

        let backend = self.policy.active().clone();
        let mut dirty = Vec::new();
        loop {
            let (requests, remaining) = self
                .cache
                .take_dirty(512)
                .map_err(ModelError::engine)?;
            if requests.is_empty() {
                break;
            }
            dirty.extend(requests.iter().map(|request| request.key()));
            self.cache
                .refill(&self.document, Some(&backend), &requests)
                .map_err(ModelError::engine)?;
            if remaining == 0 {
                break;
            }
        }

        dirty.sort();
        dirty.dedup();
        self.dirty = dirty;
        Ok(())
    }

    fn refresh_stats(&mut self) {
        // Counted from the cache rather than by meshing the document, which
        // would cost a full march on every edit.
        let bricks = self.cache.surface_bricks().map(|k| k.len()).unwrap_or(0);
        self.stats = SceneStats {
            // Reported once the viewport meshes; until then the brick count is
            // what is actually known.
            triangles: self.stats.triangles,
            vertices: self.stats.vertices,
            objects: self.layers.len().max(1),
        };
        let _ = bricks;
    }

    /// Records the geometry the viewport actually built, so the interface
    /// reports what is on screen rather than an estimate.
    pub fn record_geometry(&mut self, triangles: usize, vertices: usize) {
        self.stats.triangles = triangles;
        self.stats.vertices = vertices;
    }

    /// Turns the domain's brush settings into the engine's stroke preset.
    fn preset(&self, brush: BrushSettings, tool: ToolKind) -> StrokePreset {
        let brush = brush.sanitized();
        StrokePreset {
            radius: brush.size,
            // Flow is spacing: more flow means stamps closer together.
            spacing: (1.0 - brush.flow).clamp(0.05, 0.9),
            strength: brush.intensity,
            accumulation: if tool == ToolKind::Camada {
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

        let stamp = Item::sphere(brush.sanitized().size).map_err(ModelError::engine)?;
        let mut stamp = stamp;
        stamp
            .set_op(match tool {
                ToolKind::Padrao | ToolKind::Camada | ToolKind::Inflar => Op::Relief,
                ToolKind::Mascara => Op::Relief,
                _ => Op::Add,
            })
            .map_err(ModelError::engine)?;
        stamp
            .set_blend(Blend::Quadratic, brush.sanitized().size * 0.4)
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

    /// Applies a stroke to a voxel layer, using the tool's own verb.
    fn stroke_voxel(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
    ) -> Result<EditOutcome, ModelError> {
        let index = self.active;
        let voxel_size = {
            let layer = &self.layers[index];
            let grid = layer.grid.as_ref().expect("a voxel layer carries a grid");
            grid.voxel_size().map_err(ModelError::engine)?
        };
        // Split the borrows by field: the mask and the layers are disjoint,
        // but `&self` for one and `&mut self` for the other is not.
        let Self { layers, mask, .. } = self;
        let brush = brush.sanitized();
        let params = BrushParams {
            size: ((brush.size / voxel_size).round() as i32).clamp(1, 64),
            shape: BrushShape::Sphere,
            falloff: Falloff::Smooth,
            strength: brush.intensity,
            seed: 0,
            mask: mask.as_deref(),
        };

        let layer = &mut layers[index];
        let grid = layer.grid.as_mut().expect("a voxel layer carries a grid");

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
                ToolKind::Raspar => {
                    grid.sculpt_scrape(cell, &params, [0.0, 1.0, 0.0], 0.0)
                }
                ToolKind::Preencher => grid.sculpt_fill_cavities(cell, &params, 2),
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
        self.active_layer().editable
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
            Representation::Sdf => self.stroke_sdf(tool, brush, samples, symmetry),
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
