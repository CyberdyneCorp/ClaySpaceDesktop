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
    BrushSettings, EditOutcome, GestureSample, HistoryState, LayerKey, LayerSummary, ModelError,
    Protection, Representation, Scene, SceneModel, SceneNode, SceneStats, SculptModel, ToolKind,
};

use crate::backend::{BackendPolicy, Operation};

/// A layer the document holds, and what it is made of.
struct Layer {
    id: LayerId,
    /// A stable handle the interface uses. Engine ids are not guaranteed to
    /// survive an edit, so the interface is given one that is.
    key: LayerKey,
    name: String,
    representation: Representation,
    /// Voxel layers carry their own grid; SDF layers do not.
    grid: Option<VoxelGrid>,
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
                key: LayerKey(1),
                name: "Forma".to_string(),
                representation: Representation::Sdf,
                grid: None,
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
        let key = self.take_key();
        self.layers.push(Layer {
            id,
            key,
            name: name.to_string(),
            representation: Representation::Voxel,
            grid: Some(grid),
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
    fn preset(&self, brush: BrushSettings, tool: ToolKind) -> StrokePreset {
        let brush = brush.sanitized();
        StrokePreset {
            radius: brush.size,
            // Flow is spacing: more flow means stamps closer together.
            spacing: (1.0 - brush.flow).clamp(0.05, 0.9),
            strength: brush.intensity,
            // The design's Ruído, Suavização and Acumular, each landing on the
            // preset field the engine already has for it.
            jitter_position: brush.shaping.noise,
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
        let travelled = displacement
            .iter()
            .map(|d| d * d)
            .sum::<f32>()
            .sqrt();
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
        self.refill(layer, &[])?;
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

        // The path as control points, each carrying the radius at that point.
        // Tapering toward the tip is what makes it read as a pulled tendril
        // rather than a tube.
        let mut points = Vec::with_capacity(samples.len() * 4);
        for (index, sample) in samples.iter().enumerate() {
            let t = index as f32 / (samples.len() - 1) as f32;
            points.extend_from_slice(&sample.position);
            points.push(brush.size * (1.0 - 0.7 * t));
        }

        let mut item = Item::stroke().map_err(ModelError::engine)?;
        item.set_stroke_points(&points).map_err(ModelError::engine)?;
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

        let mut volume = self
            .document
            .volume_from_region(
                claycore::VolumeParams {
                    cell_size: Some(brush.size * 0.25),
                    ..Default::default()
                },
                min,
                max,
            )
            .map_err(ModelError::engine)?;

        volume
            .relax(&claycore::RelaxParams {
                strength: brush.intensity,
                radius_cells: 1,
                iterations: 2,
                centre,
                region_radius: brush.size,
                falloff: brush.size * 0.5,
                mask: self.mask.as_deref(),
            })
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
        let id = self
            .document
            .add_sdf_layer(name)
            .map_err(ModelError::engine)?;
        let key = self.take_key();
        let grid = match representation {
            Representation::Voxel => {
                Some(VoxelGrid::new(0.02).map_err(ModelError::engine)?)
            }
            _ => None,
        };
        self.layers.push(Layer {
            id,
            key,
            name: name.to_string(),
            representation,
            grid,
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
        self.document
            .remove_layer(id)
            .map_err(ModelError::engine)?;
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
            representation: Representation::Mesh,
            grid: None,
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
    fn layer_id(&self, key: LayerKey) -> Result<LayerId, ModelError> {
        self.index_of(key).map(|index| self.layers[index].id)
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
