//! The document, as the domain sees it.
//!
//! Implements [`SculptModel`] over a real ClayCore document, holding the brick
//! cache that makes a dab cost what it touched rather than what the model
//! holds.

use claycore::{
    Blend, BrickCache, BrickConfig, BrickKey, BrushParams, BrushShape, ClayError, Document,
    Falloff, ImportBudget, Influence, Item, LayerId, Mesh, MeshLayerDesc, MeshParams, Mesher,
    NodeId, Op, StrokePreset, VolumeParams,
};
use clayspace_model::{
    Alpha, Armature, ArmatureModel, BlendProfile, BooleanOp, BooleanRefusal, BooleanSettings,
    BrushSettings, Combine, CombineSettings, ConversionSettings, Cost, CurveJoin, CurveModel,
    CurvePoint, CurveProfile, CurveState, Direction, DocumentModel, EditOutcome, ExchangeModel,
    ExportMesher, ExportSettings, ExtrudeSettings, Format, GestureSample, GizmoDrag, GizmoHandle,
    GizmoMode, GizmoTarget, HistoryState, ImportAs, ImportSettings, Inserted, ItemKind,
    LatticeModel, LatticeState, LayerKey, LayerSummary, MaskModel, MaskOp, MaskOutline, MaskState,
    ModelError, NodeIndex, ObjectId, ObjectModel, OpenError, Protection, Refusal, Representation,
    Scene, SceneModel, SceneNode, SceneStats, SculptModel, Shape, SkinSettings, SmoothBlur,
    ToolKind, VoxelDisplay, OBJECT_VERBS,
};

use crate::backend::{BackendPolicy, Operation};
use crate::objects::{kind_of, primitive_of, union, PlacedObject};

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

/// Where a ray first meets a triangle list, or nothing.
///
/// Möller-Trumbore, front and back faces alike: a sculptor working inside a
/// form still means the surface under the pointer. The direction need not be a
/// unit vector — the parameter is measured along whatever was handed in, and
/// the point is derived from it rather than from a distance.
fn nearest_triangle(
    origin: [f32; 3],
    direction: [f32; 3],
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Option<[f32; 3]> {
    let sub = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| a[i] - b[i]) };
    let cross = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f32; 3], b: [f32; 3]| -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] };

    let mut nearest: Option<f32> = None;
    for triangle in indices.chunks_exact(3) {
        let (Some(a), Some(b), Some(c)) = (
            positions.get(triangle[0] as usize),
            positions.get(triangle[1] as usize),
            positions.get(triangle[2] as usize),
        ) else {
            continue;
        };
        let (ab, ac) = (sub(*b, *a), sub(*c, *a));
        let pvec = cross(direction, ac);
        let det = dot(ab, pvec);
        // Parallel to the plane, which includes a degenerate face.
        if det.abs() < 1e-12 {
            continue;
        }
        let inverse = 1.0 / det;
        let tvec = sub(origin, *a);
        let u = dot(tvec, pvec) * inverse;
        if !(-1e-6..=1.0 + 1e-6).contains(&u) {
            continue;
        }
        let qvec = cross(tvec, ab);
        let v = dot(direction, qvec) * inverse;
        if v < -1e-6 || u + v > 1.0 + 1e-6 {
            continue;
        }
        let t = dot(ac, qvec) * inverse;
        // Behind the eye is not in front of it.
        if t <= 1e-6 {
            continue;
        }
        if nearest.is_none_or(|best| t < best) {
            nearest = Some(t);
        }
    }
    let t = nearest?;
    Some(std::array::from_fn(|i| origin[i] + direction[i] * t))
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

/// Where the engine says a layer stands, as the domain spells a placement.
///
/// The *per-axis* reader, always. The single-factor one refuses a layer
/// carrying three different factors with `INVALID_ARGUMENT` rather than
/// averaging them away, so a squashed subtool would answer nothing at all
/// through it; the per-axis one reports the product of the layer's two scales,
/// so a layer placed uniformly reads `(s, s, s)` and no caller has to branch.
///
/// A free function rather than a method because `from_file` asks it of a
/// document that has no `ClayDocument` around it yet, and that is the call the
/// question exists for: until ABI 0.74.0 a reopened layer had to be assumed to
/// stand at the origin.
fn placement_of(document: &Document, id: LayerId) -> Option<clayspace_model::Transform> {
    let standing = document.layer_transform_nonuniform(id).ok()?;
    Some(clayspace_model::Transform {
        position: standing.position,
        rotation_axis: standing.rotation_axis,
        rotation_angle: standing.rotation_angle,
        scale: standing.scale,
    })
}

/// A layer the document holds, and what it is made of.
struct Layer {
    id: LayerId,
    /// Where the whole layer stands.
    ///
    /// A **cache of the engine's answer**, and no longer a second account of
    /// it. It was written here because the ABI set a layer transform and would
    /// not read one back, so the only record of where a subtool stood was the
    /// one the host kept — which is why every route that placed a layer also
    /// snapshotted the whole stack against an undo depth, and why a document
    /// reopened from a file came back believing every layer stood at the
    /// origin however far it had been dragged.
    ///
    /// `clay_document_layer_transform_nonuniform` (ABI 0.74.0) answers the
    /// question. `engine_layer_transform` asks it, `resync_layer_transforms`
    /// refreshes this after history moves the engine, and `from_file` fills it
    /// on open. This is kept because it is read per stroke segment — by
    /// `carried_placement`, on the path a dab takes — and a round trip through
    /// the ABI per segment to learn a number that has not changed is a cost
    /// with nothing to buy.
    transform: clayspace_model::Transform,
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
    /// The box this layer's triangles occupy, in the mesh's *own* coordinates.
    ///
    /// Cached for the reason `voxel_bounds` is, and answering the same question
    /// for the third representation: `clay_layer_bounds` reports a layer's SDF
    /// extent and a carried mesh has none, so the question had no answer at all
    /// — the whole-subtool manipulator sized itself to a default on every mesh
    /// subtool and Frame All framed nothing. Reading the triangles back needs a
    /// mutable borrow of the document and `layer_bounds` takes a shared one,
    /// which is why it is remembered rather than asked.
    ///
    /// Before the layer transform, because the transform moves under it: the
    /// vertices are what the engine holds and where they *stand* is
    /// [`ClayDocument::layer_placement`]'s answer.
    mesh_bounds: Option<([f32; 3], [f32; 3])>,
    /// The engine's geometry revision for this mesh layer, as last seen.
    ///
    /// Bumped by the engine every time the layer's triangles are replaced
    /// wholesale and by nothing else — notably **not** by a sculpt, which
    /// moves vertices and leaves the topology alone. That is the distinction
    /// this side needs: a brush is exactly the change a mesh sculptor's
    /// adjacency and BVH survive, and a rebuild is exactly the change they do
    /// not.
    ///
    /// Remembered rather than asked at the point of use, because the moment
    /// that matters is one where nothing calls: undoing a remesh replaces the
    /// triangles from inside the engine's own history, and the only account
    /// this side gets of it is that the number moved. See
    /// [`ClayDocument::settle_geometry_revisions`]. Zero for every layer that
    /// is not a mesh layer.
    ///
    /// Named for the engine's own term, and deliberately not for
    /// [`ClayDocument::mesh_revision`], which is a different number about a
    /// different thing: that one tells the viewport when to upload again and
    /// counts every drawn layer together, including the brush strokes this one
    /// is defined not to move.
    geometry_revision: u64,
    /// The subdivision hierarchy standing on this layer's cage, where the
    /// layer is one.
    ///
    /// **Beside the layer and not inside it**, because that is what the engine
    /// offers: a `clay_multires` is a free-standing owning handle that took a
    /// copy of the cage on the way in, and `clay_document_save` has never
    /// heard of it. So a hierarchy row is a real mesh layer in the document —
    /// which is where its name, its place in the stack, its transform, its
    /// mask and its save come from — plus this. See [`crate::multires`] for
    /// what follows from that, and for why the side-car beside the document is
    /// the only thing in the world that knows this row was ever a hierarchy.
    ///
    /// `None` for every other representation, and for a hierarchy row only
    /// between a document being read and its side-car being applied.
    multires: Option<crate::multires::Hierarchy>,
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
    /// The mirror this subtool is worked with.
    ///
    /// Per layer because the engine's mirror is: `clay_set_layer_mirror` takes
    /// a layer, and one number for the whole document meant the mirror was
    /// re-pointed at whichever layer was active — so turning symmetry off to
    /// work one ear turned it off on every other subtool too, and coming back
    /// found it still off.
    symmetry: [bool; 3],
    /// What the engine was last told this layer's mirror is.
    ///
    /// Recorded rather than read: the ABI sets a layer mirror and has no call
    /// that reads one back, so this is the only account of it there is — the
    /// same reason the layer transform is kept here.
    ///
    /// Held apart from the setting above because the two change at different
    /// moments and for different reasons. The setting is the sculptor's and
    /// costs nothing; writing the mirror is an *edit*, with its own entry in
    /// the engine's history, and it belongs inside the stroke that needs it —
    /// where the ViewModel counts it and one undo spends it along with the
    /// rest of the gesture. Written at the toggle instead, it would sit on the
    /// engine's stack unaccounted, and the next undo would spend itself on the
    /// mirror and leave part of the stroke standing.
    mirror: [bool; 3],
    // The frozen region painted on this subtool is *not* here, and that is
    // the change: it belongs to the layer inside the engine's own document,
    // where `clay_document_add_mask` attaches it and `clay_document_save`
    // writes it, so it survives a save and a reopen — which a mask kept
    // beside the document never could. What a caller needs is asked of the
    // document by the layer's identity: see `ClayDocument::active_mask` and
    // `claycore::MaskSource` for why that took an API change rather than a
    // field move.
    /// The rig this subtool carries: the nodes it placed, and the tree behind
    /// them.
    ///
    /// The tree is held here because the engine's parent array has no getter —
    /// positions and radii read back, the topology does not. So this is the
    /// record and the engine is written from it.
    ///
    /// One node since ClayCore 0.30.0 (#99), because the signs made the rig a
    /// single item again. It stays a list because rewriting is defined over
    /// whatever was placed: when a negative sphere was a second subtractive
    /// item, tracking only the armature's own node left the cutters behind on
    /// each rewrite, and an edited rig accumulated a subtraction per edit.
    armature: Option<(Vec<NodeId>, Armature)>,
    /// The box this subtool's rig last occupied.
    ///
    /// Kept because an edit that *shrinks* a rig leaves surface behind
    /// otherwise: the new node's own region is refilled when it is placed, and
    /// the bricks the old one used are never told anything changed. Removing an
    /// arm left the arm on screen.
    armature_bounds: Option<([f32; 3], [f32; 3])>,
}

impl Layer {
    /// X on, as the design asks, on every subtool a document gains.
    ///
    /// This was off for the whole of 0.26 and 0.27: `clay_set_layer_mirror`
    /// stored the plane, but per-item participation defaulted to *excluded*,
    /// so the sequence every host writes — set the mirror, add items —
    /// mirrored nothing, and a sculptor would have watched half of every
    /// stroke vanish. ClayCore 0.28.0 makes participation default to
    /// mirrored (#60), and `claycore_repros.rs` is what noticed.
    const STARTING_SYMMETRY: [bool; 3] = [true, false, false];

    /// A layer as every route makes one, before anything read back from the
    /// engine is written over it.
    ///
    /// Most of a new row is the same wherever it is created: it stands at the
    /// origin, is shown, unprotected, at full intensity, and has neither a
    /// grid nor a recorded pass yet. That was spelled out at five sites, so a
    /// field added to `Layer` was five edits and a route that quietly differed
    /// looked exactly like one that did not. Here it is once, and the two
    /// routes that genuinely differ — a reopened document, and a mesh row that
    /// has just been given its triangles — say so by overriding a named field.
    fn new(id: LayerId, key: LayerKey, name: &str, representation: Representation) -> Self {
        Self {
            id,
            transform: clayspace_model::Transform::default(),
            key,
            name: name.to_string(),
            engine_name: name.to_string(),
            representation,
            carries_geometry: representation != Representation::Mesh,
            visible: true,
            protection: Protection::default(),
            intensity: 100,
            voxel_bounds: None,
            mesh_bounds: None,
            geometry_revision: 0,
            voxel_chunks: std::collections::BTreeMap::new(),
            multires: None,
            sculpt_layers: Vec::new(),
            symmetry: Self::STARTING_SYMMETRY,
            // A layer the engine has just made carries no mirror — axes
            // 0/0/0 is what "off" is — so that is what it has been told, and
            // the first stroke that wants the setting above is what writes it.
            mirror: [false; 3],
            armature: None,
            armature_bounds: None,
        }
    }

    fn summary(&self) -> LayerSummary {
        LayerSummary {
            key: self.key,
            name: self.name.clone(),
            representation: self.representation,
            visible: self.visible,
            protection: self.protection,
            intensity: self.intensity,
            // Filled by the document, which is the only thing that can ask the
            // engine — see `ClayDocument::field_health`.
            health: None,
            voxel: None,
            sculpt_layers: self.sculpt_layers.clone(),
            // `None` and not a default state: a hierarchy with one level and
            // no passes is a real thing an interface draws controls for, and
            // every layer claiming to be one would be worse than none of them
            // being. So this is filled exactly where a hierarchy is held.
            multires: self
                .multires
                .as_ref()
                .map(crate::multires::Hierarchy::state),
        }
    }

    /// Whether an edit may touch it: shown, not ghosted, not locked.
    fn editable(&self) -> bool {
        self.visible && self.protection.is_editable()
    }
}

/// A ClayCore document driven by the domain's vocabulary.
/// Everything one segment of a stroke on carried geometry is made of.
///
/// One assembly serving both representations that carry their own vertices,
/// because it *is* one assembly: `clay_multires_sculptor_apply_stroke` takes
/// the same `clay_mesh_brush_desc` and the same `clay_stroke_preset` as
/// `clay_mesh_sculptor_apply_stroke`, and the header says why in as many words
/// — "the same verbs, the same falloffs, the same mask, the same alpha and the
/// same automasking as a mesh layer, because it is the same code". A hierarchy
/// spelling this out for itself would be sixteen brushes waiting to drift from
/// the sixteen beside them, and every measurement written into the comments
/// below would go on describing only one of the two.
struct CarriedStroke<'a> {
    /// Spacing, taper and jitter — and where a resolved stroke's radius and
    /// strength actually come from.
    preset: StrokePreset,
    /// The descriptor a single stamp uses, and the one a resolver starts from.
    stamp: claycore::MeshStamp<'a>,
    /// The path, five floats to a sample.
    points: Vec<[f32; 5]>,
    /// The whole travel, which is what Grab carries its region by.
    gesture: [f32; 3],
}

/// Builds one, from a gesture already carried into the geometry's own frame.
///
/// `brush` is sanitized and already divided by the subtool's scale, and
/// `preset` is [`ClayDocument::preset`]'s — which this may still clamp, since
/// the accumulation rule below is a fact about the verbs rather than about
/// brushes.
fn carried_stroke<'a>(
    verb: claycore::MeshBrush,
    brush: BrushSettings,
    samples: &[GestureSample],
    preset: StrokePreset,
    alpha: Option<&'a Alpha>,
    chosen: clayspace_model::Colour,
) -> CarriedStroke<'a> {
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
    let mut preset = preset;
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
        let stamp = preset.spacing * brush.size * brush.intensity * ClayDocument::NUDGE_PUSH;
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
        // What Paint blends toward. Left at the engine's white default
        // before this, so the one brush whose whole job is colour had
        // nothing to say: a white blend over a white mesh is a stroke that
        // changes nothing. Smear reads no colour of its own — it drags the
        // colour already there — so this is carried for both and read by
        // one.
        colour: chosen.rgb,
        smooth_iterations: Some(ClayDocument::SMOOTH_PASSES),
        // The grain, as the sculptor set it. It turns the stamp's
        // in-plane axes about the direction it faces, which is why the
        // fixed world-X tangent the alpha block hands over below is not
        // the thing that decides a turned stamp's orientation — that
        // tangent picks the plane's zero, and this turns the stamp within
        // it. Zero, the default, is no rotation at all and is what every
        // brush that has never been turned keeps sending.
        stamp_azimuth: brush.shaping.azimuth,
        // Filled in per mirror by the caller, because a seed is a place
        // as much as it is a class: the pick that recorded one stood
        // where the unmirrored stamp stands, and a reflected copy of it
        // stands somewhere the walk cannot start from. See
        // [`crate::seed`]. A hierarchy fills none at all — a class picked
        // off the cage names a numbering the bound level does not share,
        // and a seed spent against the wrong numbering is in bounds,
        // wrong and silent.
        seed: None,
        // No automask. This application offers none of the five factors
        // as a brush setting, so the default — no gate at all, which the
        // engine documents as bit-identical to a descriptor from before
        // automasking existed — is what every stamp sends and what every
        // figure in the performance baseline was measured with.
        //
        // Named rather than defaulted for the reason below, and because
        // ClayCore v0.78.0 is the release in which a factor set here
        // started reaching the adaptive representation as well as this
        // one. When a backface gate or a border gate is offered, this is
        // the line it arrives on and `Shaping` is where it is chosen.
        automask: claycore::Automask::default(),
        // Every field is named now that the colour is one of them, so
        // there is no `..MeshStamp::default()` here: a field added
        // upstream should fail this call rather than be filled in
        // silently with an engine default nobody chose.
        // Flatten and Scrape mean "everything under this disc", and a
        // surface walk refuses to flatten across a groove — which is not
        // what either verb says.
        geodesic: !matches!(
            verb,
            claycore::MeshBrush::Flatten | claycore::MeshBrush::Scrape
        ),
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
    CarriedStroke {
        preset,
        stamp,
        points,
        gesture,
    }
}

/// A mesh gesture in flight: the record its stamps are written into, and the
/// sculptor that owes those stamps a normal recomputation.
///
/// The two are one value because the engine's contract binds them, and binds
/// them in a way that is silent when it is broken:
///
///   * nothing flushes on its own — the sculptor does not know where a gesture
///     ends, and guessing at it would flush mid-drag, which is the whole of
///     what deferring exists to avoid;
///   * a host that defers **must** flush, or the form keeps the shading it had
///     before the drag;
///   * and the flush has to be handed the *same* record the stamps were noted
///     into, or the gesture's undo puts the vertices back and leaves the
///     shading where the gesture wrote it.
///
/// So the record is held beside the handle that owes it rather than beside the
/// layer key, which makes the pairing the only one expressible — and makes
/// `Drop` the last exit. A gesture that ends by unwinding out of a `?`, by
/// being replaced when the pointer lands on another subtool, or by a document
/// going away under it settles on the way past instead of leaving a mesh
/// shaded from where its vertices used to be.
struct LiveMesh {
    layer: LayerKey,
    /// Shared with `mesh_sculptors`, so an eviction or a layer removed during
    /// the drag cannot take the flush's handle away from the gesture that owes
    /// it. See [`crate::sculptors`].
    sculptor: crate::sculptors::SharedSculptor,
    /// `None` only between [`LiveMesh::finish`] taking the record out and the
    /// value being dropped, which is why every reader goes through
    /// [`LiveMesh::deltas`].
    deltas: Option<claycore::MeshDeltas>,
}

impl LiveMesh {
    fn new(
        layer: LayerKey,
        sculptor: crate::sculptors::SharedSculptor,
        deltas: claycore::MeshDeltas,
    ) -> Self {
        Self {
            layer,
            sculptor,
            deltas: Some(deltas),
        }
    }

    /// The record the gesture's stamps are noted into.
    fn deltas(&mut self) -> &mut claycore::MeshDeltas {
        self.deltas
            .as_mut()
            .expect("a live gesture always holds its record until it is finished")
    }

    /// Recomputes whatever the sculptor deferred, into this record, and puts
    /// the deferral back down.
    ///
    /// Idempotent, and cheap where nothing was deferred: the engine returns
    /// immediately on an empty pending set, which is what makes it safe to
    /// settle on every exit rather than only on the ones that deferred
    /// something.
    fn settle(&mut self) -> Result<(), ModelError> {
        // `try_borrow_mut` rather than `borrow_mut`: this runs from `Drop`,
        // and a panic there during an unwind aborts the process. A sculptor
        // already borrowed is a caller settling from inside its own stamp,
        // which is a bug in this file rather than something a user can reach.
        let Ok(mut sculptor) = self.sculptor.try_borrow_mut() else {
            debug_assert!(
                false,
                "a live gesture settled while its sculptor was borrowed"
            );
            return Err(ModelError::engine(
                "a escultora estava em uso quando o gesto foi encerrado",
            ));
        };
        sculptor
            .flush_normals(self.deltas.as_mut())
            .map_err(ModelError::engine)?;
        sculptor
            .set_defer_normals(false)
            .map_err(ModelError::engine)
    }

    /// Ends the gesture and hands back what it recorded.
    ///
    /// Settles first, so the record handed on is the exact one — the flush
    /// notes the normals it changed, and an undo has to put the shading back
    /// as well as the vertices.
    fn finish(mut self) -> (LayerKey, claycore::MeshDeltas) {
        if let Err(e) = self.settle() {
            // Reported rather than propagated: the gesture is over either way,
            // and dropping the record here would lose the sculptor's work as
            // well as its shading.
            eprintln!("as normais adiadas não puderam ser recalculadas: {e}");
        }
        let deltas = self
            .deltas
            .take()
            .expect("a live gesture always holds its record until it is finished");
        (self.layer, deltas)
    }
}

impl Drop for LiveMesh {
    /// The last exit, and the reason this is a type rather than a call at the
    /// end of each path that ends a stroke.
    fn drop(&mut self) {
        if let Err(e) = self.settle() {
            eprintln!("as normais adiadas não puderam ser recalculadas: {e}");
        }
    }
}

/// Which way through the history a step is going.
///
/// A `MeshDeltas` needs to be told, because one record serves both directions.
/// A hierarchy's record does not: it is a state, and putting a state back is
/// the same operation whichever side it was reached from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Back,
    Forward,
}

/// One gesture on geometry the host holds, and where it sits against the
/// engine's own history.
///
/// Both representations the host carries are here rather than in two stacks
/// beside each other, and that is not tidiness. Each record orders itself
/// against the engine's history by the depth it was made at, and two stacks
/// ordering themselves against the same depth cannot order themselves against
/// each other — a session that sculpted a mesh subtool and a hierarchy in turn
/// would have two records both answering "newest" and an undo would take back
/// whichever was asked first.
struct MeshGesture {
    layer: LayerKey,
    what: GestureRecord,
    /// The engine's undo depth when this was recorded. See `mesh_undo`.
    engine_depth: usize,
}

/// What it takes to put one gesture back.
///
/// The two are different in kind, and the difference is the ABI's rather than
/// this application's.
enum GestureRecord {
    /// A fixed-topology mesh gesture, taken back by the engine's own exact
    /// record: `MeshDeltas` notes a vertex's position, normal and colour the
    /// first time a stamp sees it, so reverting is bit-exact and costs the
    /// vertices the gesture touched.
    Deltas(claycore::MeshDeltas),
    /// A hierarchy gesture, taken back by putting the hierarchy's whole
    /// serialized state back.
    ///
    /// **There is no delta record for one.** clay.h says so twice, unprompted
    /// — of `clay_multires_sculptor_apply_stroke` and of the layered stroke
    /// transaction alike: *"the record itself does not cross this ABI yet,
    /// which is stated here rather than left to be discovered"*, and a host
    /// that wants a layered gesture in an undo stack is pointed at pyclay or
    /// the C++ `SculptLayerDelta`. Of the three ways out — reconstructing a
    /// delta from `dirty_blocks` and `copy_block`, which the ABI offers no way
    /// to write back through; keeping the gesture's transaction open, whose
    /// exact cancel is the one thing that *is* offered but which has no
    /// resolved-stroke verb on it; and holding the bytes — only the third is
    /// exact and reachable.
    ///
    /// So this is what it says: the hierarchy as it stood on the other side of
    /// this step. Measured on the pinned engine, that is 710 KB and 1.39 ms to
    /// take at level 4 over a 16×16 cage, and 8.15 ms to put back. The bytes
    /// are what [`crate::multires::HISTORY_BYTES`] bounds.
    Hierarchy(Vec<u8>),
}

/// One crossing, and the layer whose presence in the scene follows it.
struct Crossing {
    /// The layer the crossing added, hidden while the crossing is undone.
    layer: LayerId,
    /// The engine's undo depth when this was recorded. See `mesh_undo`.
    engine_depth: usize,
    /// How many engine entries the crossing left behind.
    ///
    /// One for an ordinary crossing. An in-place one also removes the layer
    /// it read and moves the result into its row, and the engine records each
    /// of those separately — a group does not swallow them. So the count is
    /// measured rather than assumed, taken back together, and discounted from
    /// the depth the interface reports: a sculptor made one crossing and has
    /// one thing to undo.
    steps: usize,
}

/// A rebuild of a mesh layer's topology, and where it sits in the engine's
/// history.
///
/// Recorded because the engine's own signal does not cover the case that
/// matters most. `clay_document_mesh_layer_revision` is documented as bumped
/// "every time a layer's triangles are replaced wholesale", and the reason
/// given for it existing is the cache that a wholesale replacement invalidates
/// — an adjacency, a BVH, a live sculptor. Measured on ClayCore 0.73.0, it is
/// bumped by the rebuild and **not by history moving over one**: a layer
/// attached at revision 1 and rebuilt to revision 2 comes back to its original
/// 119,100 triangles under undo, and to the rebuilt 37,752 under redo, at
/// revision 2 throughout. So the one moment the number was added for is the
/// one moment it says nothing.
///
/// Held here in the way this file already holds a crossing: by the engine
/// depth the step sits at, so a history move across it is recognisable. The
/// alternative — dropping every mesh sculptor on every undo — puts the weld
/// back on the interface thread for a step that usually touched no mesh at
/// all, which is the cost `crate::sculptors` exists to avoid.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rebuild {
    layer: LayerKey,
    /// The engine's undo depth after the rebuild was recorded, as a crossing
    /// records its own.
    engine_depth: usize,
}

/// A layer shown alone, and what the rest looked like before it was.
///
/// The snapshot is the whole of what a release needs: the engine's contract is
/// that "a hidden layer contributes nothing to the field; showing it again
/// restores the original field exactly", so putting the recorded flags back
/// puts the scene back.
#[derive(Debug, Clone, PartialEq)]
struct Solo {
    layer: LayerKey,
    /// Every layer's visibility at the moment the solo began.
    was: Vec<(LayerKey, bool)>,
}

/// A batch of visibility commands the host issued for its own reasons, and
/// where it sits in the engine's history.
///
/// Solo and the hide-and-restore a bake needs are ways of *looking* at the
/// document, but the engine has no journal pause — once undo is enabled every
/// command is recorded, `SetLayerVisibleCmd` among them — and the merged SDF
/// surface cannot drop a layer any other way than engine visibility. So the
/// entries are made and then stepped over: undo hops a whole gesture the way
/// it already hops between `mesh_undo` and the engine's own stack, and for the
/// same reason — depth is what says which record is the more recent one.
struct VisibilityGesture {
    /// The engine's undo depths the batch produced, ascending. See `mesh_undo`.
    depths: Vec<usize>,
    /// What was shown alone before the batch, and after it.
    ///
    /// Carried so that hopping the gesture in either direction restores the
    /// gesture as well as the flags it wrote: a solo whose commands undo has
    /// taken back is a solo the document is no longer in, and an indicator
    /// saying otherwise would describe a state that had left.
    before: Option<Solo>,
    after: Option<Solo>,
}

/// Which slice of the carried buffer one layer's triangles occupy.
///
/// The viewport draws every visible voxel and mesh layer from a single
/// concatenated buffer, so without this nothing downstream can say where one
/// subtool ends and the next begins — which is what an active-subtool cue has
/// to know before it can tint one of them and leave the rest alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedSpan {
    pub layer: LayerKey,
    /// Positions into the index buffer, not into the vertex buffer: what a
    /// draw call takes is a range of indices.
    pub indices: std::ops::Range<u32>,
}

/// The one buffer every carried layer is concatenated into.
///
/// The four parallel vectors travel together everywhere, and the rebasing that
/// joins one layer's indices onto what is already there is the one step that
/// must not be got wrong twice — a voxel grid and a mesh layer used to spell
/// it out separately. Named here so there is a single `append`.
#[derive(Default)]
struct CarriedBuffer {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl CarriedBuffer {
    fn with_capacity(vertices: usize, triangles: usize) -> Self {
        Self {
            positions: Vec::with_capacity(vertices),
            normals: Vec::with_capacity(vertices),
            colors: Vec::with_capacity(vertices),
            indices: Vec::with_capacity(triangles),
        }
    }

    /// Appends one layer's triangles, shifting its indices past the vertices
    /// already collected.
    fn append(
        &mut self,
        positions: &[[f32; 3]],
        normals: &[[f32; 3]],
        colors: &[[f32; 3]],
        indices: &[u32],
    ) {
        let base = self.positions.len() as u32;
        self.indices.extend(indices.iter().map(|i| i + base));
        self.positions.extend_from_slice(positions);
        self.normals.extend_from_slice(normals);
        self.colors.extend_from_slice(colors);
    }
}

pub struct ClayDocument {
    // -- what must go before the document ------------------------------------
    //
    // Fields drop in declaration order, and these two hold BORROWED engine
    // pointers into `document`'s own meshes: `MeshSculptor::for_layer` is
    // handed the layer's `clay_mesh` and keeps it, and `LiveMesh::drop`
    // recomputes the normals its gesture deferred, which reads that mesh. A
    // document dropped mid-drag would otherwise free the meshes first and
    // leave the flush reading storage that had gone — measured, a segmentation
    // fault inside the engine rather than a refusal, because a borrowed handle
    // has nothing left to check against.
    //
    // So they are declared here, above the thing they point into, and nothing
    // that borrows from `document` may be declared below it.
    /// The gesture being previewed on a mesh layer, and what it has moved.
    ///
    /// A dragging verb is laid down again from its anchor on every segment, so
    /// what the last segment did has to be taken back first — this is the
    /// record that takes it back. Promoted to the undo stack when the gesture
    /// ends, so a drag is still one undo however many segments drew it.
    ///
    /// It carries the sculptor as well as the record: see [`LiveMesh`].
    live_mesh: Option<LiveMesh>,
    /// In a cell because a *pick* needs it and a pick is a question.
    ///
    /// The sculptor answers a raycast from its own tree and may refit it while
    /// doing so, which is a mutation — but `SculptModel::pick` takes `&self`,
    /// and widening that so every caller of a question must hold a mutable
    /// borrow would be the tail wagging the dog. Casting the borrow away was
    /// the other option and `forbid(unsafe_code)` refused it, correctly: the
    /// C call takes a non-const sculptor because it really may write.
    mesh_sculptors: std::cell::RefCell<crate::sculptors::Sculptors>,
    /// What the last pick against a mesh layer learned, for the stroke that
    /// follows it.
    ///
    /// In a cell for the reason the sculptors are: it is written by a pick,
    /// and a pick is a question. See [`crate::seed`] for why a class is kept
    /// with the numbering it was picked in rather than on its own.
    pub(crate) picked_seed: std::cell::Cell<Option<crate::seed::PickedSeed>>,

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
    /// Whether a gesture is open and should be previewed rather than banked.
    ///
    /// Written only by [`ClayDocument::set_previewing`], because two other
    /// things follow it exactly — see [`crate::maintenance`].
    previewing: bool,
    /// The work this document owes itself between two interactions, the gate
    /// that keeps it from happening during one, and the pin a gesture holds.
    maintenance: crate::maintenance::Maintenance,
    /// Bumped by every preview, so the viewport knows to look again.
    ///
    /// A preview banks nothing, so nothing else about the document changes and
    /// the number the viewport watches would sit still while the drag was
    /// visibly moving the surface.
    live_generation: u64,
    /// The live field gesture in progress, and the surface it is drawing.
    ///
    /// While this is set the viewport meshes the *preview's* cache instead of
    /// the document's, because the document deliberately has not changed yet.
    live_smooth: Option<crate::live::LiveSmooth>,
    /// The Move drag in progress, when one is being previewed rather than
    /// written per segment. See [`crate::live::LiveMove`] for what writing one
    /// per segment costs the field.
    live_move: Option<crate::live::LiveMove>,
    /// A Move gesture that may be live but has no anchor yet.
    ///
    /// `open_live_gesture` is told the tool and the symmetry and not where the
    /// pointer went down, and a drag is anchored there — so the refusals are
    /// answered at pointer-down and the transaction begins on the first
    /// segment, which is the first thing that carries a position.
    live_move_armed: bool,
    /// History entries opening the live gesture recorded before it began.
    live_opening_entries: usize,
    /// The gesture the preview has been showing, kept so that closing it can
    /// lay the stroke down the way every smoothing stroke was laid down
    /// before there was a preview.
    ///
    /// See [`ClayDocument::close_live_gesture`] for why the transaction's own
    /// commit is not used.
    live_gesture: Option<(ToolKind, BrushSettings, [bool; 3], Vec<GestureSample>)>,
    /// Bumped whenever the surface the viewport should mesh from changes
    /// identity — a live gesture opening, committing or being abandoned.
    ///
    /// The two caches do not share a key space: a preview key and a document
    /// key of the same coordinate name different bricks. So the viewport
    /// cannot patch its way from one to the other and has to lay the surface
    /// out again, which is what watching this number tells it to do.
    surface_epoch: u64,
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
    /// The curve being placed or edited, and the node its sweep is placed as.
    ///
    /// The node is held so an edit *replaces* the sweep rather than adding
    /// another beside it — the same reason a snakehook gesture holds its
    /// tendril, and the same entry point.
    curve: Option<Curve>,
    /// The tendril a snakehook gesture is pulling, while one is open.
    ///
    /// Held so the segments of one drag *grow* a single curve rather than
    /// leaving a trail of them: a segment that added its own item restarted
    /// the taper, which beaded the tendril into a string of spheres.
    live_hook: Option<(LayerId, claycore::NodeId)>,
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
    /// The mesh sculptors built so far, bounded and least recently used
    /// first — see [`crate::sculptors`] for why there is more than one.
    ///
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
    /// Crossings undo can take back whole, newest last.
    ///
    /// A crossing is a layer plus what fills it. Since
    /// `unify-the-undo-history` the engine records the filling, but layer
    /// creation is still not something it takes back — so an engine undo on
    /// its own empties the new layer and leaves it standing, which is the
    /// shape the undo group around `convert_layer` was added to prevent and
    /// can no longer prevent by itself. Measured across the pin, one undo of
    /// the same crossing: 0.39.0 left the layer's 3,952 vertices alone,
    /// 0.52.2 left the layer in the list at zero.
    ///
    /// Interleaved by depth exactly as `mesh_undo` is, and for the same
    /// reason: any engine edit since has raised the depth, which makes that
    /// edit the more recent one.
    crossing_undo: Vec<Crossing>,
    /// Where this session's mesh rebuilds sit in the engine's history.
    ///
    /// Never pruned by a history step, in either direction: a rebuild undone
    /// can be redone, and both directions replace the layer's triangles. What
    /// clears it is the engine truncating the redo stack, which is the only
    /// event that makes a recorded depth unreachable. See [`Rebuild`].
    rebuilds: Vec<Rebuild>,
    crossing_redo: Vec<Crossing>,
    /// Layers an undone crossing has taken off the scene.
    ///
    /// Hidden rather than removed, and the difference is forced by the
    /// engine: "every editing entry point records its own inverse", removal
    /// included, so removing the layer would itself be an undo step. That is
    /// not a theoretical objection — it was tried and measured: a second undo
    /// brought the emptied layer back, and a redo then built a third one
    /// beside it.
    ///
    /// So the engine's history takes the filling back and puts it again,
    /// which it does exactly and for free, and the only thing left to the
    /// host is whether the layer is in the scene. `reconcile_layers` skips
    /// these and `save` drops them, so nothing outside this type sees one.
    suppressed: std::collections::HashSet<LayerId>,
    /// Layers the document no longer holds, kept in case history brings one
    /// back.
    ///
    /// A removal is undoable, and the engine puts the layer back with the id it
    /// had. What the engine cannot put back is everything this side keeps about
    /// it: its `LayerKey`, its mask, its mirror, where it stands, its meshed
    /// chunks. Rebuilt from what the document can answer, a restored layer got a
    /// freshly minted key and a default of each — measured after a consuming
    /// boolean, the two operands came back as new keys with the painted mask
    /// gone, symmetry back at the default, and every `PlacedObject` row still
    /// filed under the keys that had gone, so the rows vanished and the
    /// whole-subtool manipulator drew at the origin while the engine held the
    /// real transform.
    ///
    /// Held until the layer comes back or the session ends. Bounded by how many
    /// layers a session removes, which is what makes carrying the chunks along
    /// affordable — and carrying them is what lets a restored grid draw without
    /// waiting for the next edit to dirty it.
    retired: std::collections::HashMap<LayerId, Layer>,
    /// Rows whose hierarchy this session could not put back, by name.
    ///
    /// A `.clayspace` carries a hierarchy's cage and nothing standing on it,
    /// so the side-car beside it is the only thing that knows a row was ever a
    /// hierarchy. When a record for a row is found and cannot be honoured, the
    /// row opens as the mesh layer the document says it is — which is honest,
    /// and is quiet. This is what makes it loud: the names reach the
    /// diagnostics report, which is the text a sculptor pastes when they ask
    /// why their sculpt came back flat.
    hierarchies_lost: Vec<String>,
    /// The layer being shown alone, while one is.
    solo: Option<Solo>,
    /// Visibility batches the history hops rather than stops on, newest last,
    /// and the redo side of the same. See [`VisibilityGesture`].
    visibility_undo: Vec<VisibilityGesture>,
    visibility_redo: Vec<VisibilityGesture>,
    /// How much the engine held on its redo side when a history step was last
    /// taken.
    ///
    /// The engine truncates its redo stack the moment a new command lands, and
    /// says so no other way. A drop against this is that signal — and so is a
    /// redo stack that has gone empty, since a visibility gesture is hopped by
    /// redoing the engine's own entries and there are none.
    ///
    /// Without it a gesture left on the redo side went on matching
    /// `depths.first() == engine_undo_depth() + 1` whenever the depth happened
    /// to return to that value. Measured: solo, undo, an ordinary dab, undo,
    /// redo — the redo was spent putting the dab back through the *hop*, so
    /// `resync_objects`, `resync_layer_transforms` and `resync_armature` were
    /// all skipped for it, and the interface was left showing a solo engaged
    /// over a scene in which every layer was visible.
    redo_room: usize,
    /// How the next SDF edit combines with what is under it.
    combine: CombineSettings,
    /// What the colour brushes paint with, and the colours before it.
    ///
    /// One value for the document rather than one per tool or per layer: it is
    /// what the sculptor is painting with now, and every colour brush picks up
    /// the same one. See `clayspace_model::colour` for why it is not in
    /// `BrushSettings`.
    colour: clayspace_model::ColourState,
    /// The one alpha stamp loaded, which every brush with `alpha` set uses.
    alpha: Option<Alpha>,
    /// The voxel drag being made, while one is. Opened by the first segment
    /// and dropped when the gesture ends.
    voxel_grab: Option<VoxelGrab>,
    /// Whether a pass is being recorded on the active grid.
    ///
    /// Mirrored here rather than read back per frame: the engine answers per
    /// *grid* and the shell asks about the document, so switching to a layer
    /// with no grid at all would otherwise need a borrow to say "no".
    recording_pass: bool,
    /// Hands out layer keys. Monotone, so a key is never reused for a
    /// different layer after a removal.
    next_key: u64,
    skin: SkinSettings,
    /// The placed objects, and everything about them the engine will not read
    /// back. See `crate::objects` for why this exists at all.
    objects: Vec<PlacedObject>,
    /// Which one the manipulator and the options bar are addressing.
    selected_object: Option<ObjectId>,
    /// Whether a manipulator gesture is open, and on what.
    ///
    /// While one is, every transform written goes into a single undo group, so
    /// a drag is one entry however many frames it took.
    dragging: Option<GizmoTarget>,
    /// The table as it stood at each of the engine's undo depths.
    ///
    /// The engine reverts an object's transform and has no way to tell the
    /// table it did, so the table follows by depth — the same way `mesh_undo`
    /// interleaves with the engine's history, and for the same reason. An
    /// object edit records the table on both sides of itself, so undoing
    /// across it finds the state before and redoing finds the state after.
    /// A stroke raises the depth without touching objects and records
    /// nothing, which leaves the table alone: correct, because it did not
    /// change.
    object_states: std::collections::BTreeMap<usize, Vec<PlacedObject>>,
    // There is no `layer_states` beside this, and there was until ABI 0.74.0.
    // A layer's placement had exactly the object table's problem — written to
    // the engine, cached here, reverted by an undo the engine could not report
    // — and exactly the object table's answer, a snapshot per undo depth. The
    // difference now is that `clay_document_layer_transform_nonuniform` reads
    // the placement back, so `resync_layer_transforms` asks the engine instead
    // of consulting a copy. A node's transform has no such reader (#69 covers
    // the layer level only), which is why the object table is still here.
}

impl ClayDocument {
    /// Builds a document with one SDF layer holding a starting form.
    pub fn new(policy: BackendPolicy) -> Result<Self, ModelError> {
        let mut document = Document::new().map_err(ModelError::engine)?;
        let id = document
            .add_sdf_layer("Forma")
            .map_err(ModelError::engine)?;
        // Set before undo starts recording: the starting mirror is part of
        // making the document, not something a user did.
        document
            .set_layer_mirror(id, Layer::STARTING_SYMMETRY, 0.0)
            .map_err(ModelError::engine)?;
        document.enable_undo().map_err(ModelError::engine)?;

        let cache = BrickCache::new(Self::BRICK_CONFIG).map_err(ModelError::engine)?;

        let mut model = Self {
            document,
            layers: vec![Layer {
                // Written above, before undo started recording.
                mirror: Layer::STARTING_SYMMETRY,
                ..Layer::new(id, LayerKey(1), "Forma", Representation::Sdf)
            }],
            active: 0,
            cache,
            policy,
            combine: CombineSettings::for_strokes(),
            colour: clayspace_model::ColourState::default(),
            alpha: None,
            voxel_grab: None,
            recording_pass: false,
            dirty: Vec::new(),
            stats: SceneStats::default(),
            carried: (0, 0),
            live_mesh: None,
            previewing: false,
            maintenance: crate::maintenance::Maintenance::new(),
            live_generation: 0,
            live_smooth: None,
            live_move: None,
            live_move_armed: false,
            live_opening_entries: 0,
            live_gesture: None,
            surface_epoch: 0,
            meshed_chunks: 0,
            surface_brick_count: 0,
            mesh_sculptors: std::cell::RefCell::default(),
            picked_seed: std::cell::Cell::default(),
            mesh_undo: Vec::new(),
            mesh_redo: Vec::new(),
            crossing_undo: Vec::new(),
            rebuilds: Vec::new(),
            crossing_redo: Vec::new(),
            suppressed: std::collections::HashSet::new(),
            retired: std::collections::HashMap::new(),
            hierarchies_lost: Vec::new(),
            solo: None,
            visibility_undo: Vec::new(),
            visibility_redo: Vec::new(),
            redo_room: 0,
            curve: None,
            live_hook: None,
            lattice: None,
            voxel_display: VoxelDisplay::default(),
            voxel_blur: SmoothBlur::default(),
            voxel_smooth: std::collections::BTreeMap::new(),
            cage_revision: 0,
            mask_revision: 0,
            next_key: 2,
            skin: SkinSettings::default(),
            objects: Vec::new(),
            selected_object: None,
            dragging: None,
            object_states: std::collections::BTreeMap::new(),
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
        let node = self
            .document
            .add_item(layer, &body)
            .map_err(ModelError::engine)?;
        self.record_starting_form(node, radius);
        self.refill(layer, &[])?;
        self.refresh_stats();
        Ok(())
    }

    /// Records the opening sphere as the placed object it is.
    ///
    /// Deliberate rather than incidental. The starting form always *was* a
    /// placed sphere; nothing but the absence of an object model made it
    /// special, and a sculptor who wants to make the thing they are working on
    /// bigger should be able to select it and say so. It can also be deleted,
    /// which is what removing an object means and is undoable like any other.
    fn record_starting_form(&mut self, node: NodeId, radius: f32) {
        let key = self.layers[0].key;
        self.objects.push(PlacedObject::new(
            key,
            node,
            clayspace_model::ObjectSource::Shape(Shape::Sphere),
            Shape::Sphere.sanitised(&[radius]),
            CombineSettings::default(),
            [0.0; 3],
        ));
        self.remember_objects_after();
    }

    /// Places a starting sphere so there is something to sculpt on.
    pub fn with_starting_form(mut self) -> Result<Self, ModelError> {
        let layer = self.layers[0].id;
        let body = Item::sphere(1.0).map_err(ModelError::engine)?;
        let node = self
            .document
            .add_item(layer, &body)
            .map_err(ModelError::engine)?;
        self.record_starting_form(node, 1.0);
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

    /// The cache the viewport meshes, and where its lattice sits in the world.
    ///
    /// The document's own, and a live gesture's preview while one is drawing.
    /// The offset is what puts a relabelled preview lattice back under the
    /// sculptor's pointer — see `crate::live` for why the preview has a
    /// lattice of its own rather than sharing this one.
    pub fn drawn_cache(&self) -> (&BrickCache, [f32; 3]) {
        match self.live_surface() {
            Some(live) => (live.cache, live.offset),
            None => (&self.cache, [0.0; 3]),
        }
    }

    /// What a layer's field costs, for the row that reports it.
    ///
    /// The engine's report alone — **33 µs** on a 97-item layer — where
    /// [`SceneModel::layer_cost`] beside it also estimates what collapsing
    /// would occupy, and that estimate is **287 ms**. The scene is assembled
    /// on every refresh, so only the cheap half belongs in it; the estimate is
    /// asked for when the sculptor is deciding, which is once.
    ///
    /// `None` for a mesh or a grid: neither holds an edit list, so neither has
    /// a field to steepen.
    fn field_health(&self, layer: &Layer) -> Option<clayspace_model::FieldHealth> {
        if layer.representation != Representation::Sdf {
            return None;
        }
        let report = self.document.field_report(layer.id, 0.5).ok()?;
        Some(clayspace_model::FieldHealth {
            items: report.item_count,
            safe_step_scale: report.safe_step_scale,
            advises_consolidation: report.advises_consolidation,
            consolidated: self
                .document
                .consolidation_state(layer.id)
                .ok()
                .flatten()
                .is_some(),
        })
    }

    /// What a grid layer is made of, where the layer is one.
    ///
    /// Read beside the field's health and on the same terms: cheap, asked per
    /// scene rather than per frame, and `None` where the question does not
    /// apply. `clay_voxel_size` and `clay_voxel_occupied_count` have been
    /// bound in `claycore` throughout and were read only inside this adapter,
    /// so the interface could say a layer held voxels and not how coarse they
    /// were — which is the one number that decides what detail a grid can hold
    /// at all.
    fn voxel_stats(&self, layer: &Layer) -> Option<clayspace_model::VoxelStats> {
        if layer.representation != Representation::Voxel {
            return None;
        }
        // The reader, which borrows the document immutably — a scene query has
        // no business taking the exclusive borrow the writable grid wants.
        let (_, grid) = self.document.voxel_reader(&layer.engine_name).ok()?;
        Some(clayspace_model::VoxelStats {
            cell_size: grid.voxel_size().ok()?,
            occupied: grid.occupied_count().ok()?,
        })
    }

    /// Keys dirtied since the last call, cleared as they are handed over.
    ///
    /// The viewport meshes exactly these and patches their ranges, which is
    /// what keeps a dab's cost proportional to what it touched.
    /// The bricks waiting to be re-meshed, without draining them.
    ///
    /// For asking questions about the set the viewport is about to be handed —
    /// `take_dirty_keys` empties it, which a diagnostic must not do.
    pub fn dirty_keys(&self) -> &[BrickKey] {
        &self.dirty
    }

    pub fn take_dirty_keys(&mut self) -> Vec<BrickKey> {
        // The preview's, while one is up: the document's own cache is not the
        // surface being drawn, and its dirty set is empty anyway because a
        // live gesture writes nothing to the document.
        match self.live_smooth.as_mut() {
            Some(live) => live.take_dirty(),
            None => std::mem::take(&mut self.dirty),
        }
    }

    pub fn policy(&self) -> &BackendPolicy {
        &self.policy
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
        self.adopt_engine_layer(id, name, Representation::Voxel)?;
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

    /// Refuses unless the active subtool carries a mask with something in it.
    ///
    /// Split out because the caller needs the answer before it knows which
    /// route it is taking, and holding a borrow of the mask across that choice
    /// is what it cannot do: two of the three routes take the document
    /// mutably.
    fn a_mask_worth_extruding(&self) -> Result<(), ModelError> {
        match self.active_mask() {
            None => Err(ModelError::engine("não há máscara para extrudar")),
            Some(mask) if mask.painted_count().unwrap_or(0) == 0 => {
                Err(ModelError::engine("a máscara está vazia"))
            }
            Some(_) => Ok(()),
        }
    }

    /// The frozen region the active subtool carries, for a verb to consult.
    ///
    /// Held by the *document*, and asked for by the layer's identity. The
    /// lease borrows the document **shared**, so it can be held across another
    /// read of the same document — which is what the relax, flatten and mesh
    /// paths need, and what a `MaskRef` taken out of `&mut Document` could
    /// never do.
    fn active_mask(&self) -> Option<claycore::MaskLease<'_>> {
        self.document.layer_mask(self.active_layer().id)
    }

    /// The same, for the entry points that hold the document *mutably*.
    ///
    /// They cannot be handed a mask lent out of the document they are about to
    /// edit, so they are handed its name instead and resolve it themselves.
    /// See [`claycore::MaskSource`].
    fn active_mask_source(&self) -> claycore::MaskSource<'static> {
        claycore::MaskSource::Layer(self.active_layer().id)
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

    /// Re-meshes what a step through the history actually reached.
    ///
    /// An undo used to dirty the whole active layer, because that was the
    /// narrowest region there was to name: the engine reverted whatever it
    /// reverted and would not say where. Measured on 1043 surface bricks, that
    /// cost 1045 keys and 141 ms to take back a dab that cost 18 keys and
    /// 3.6 ms — about forty times the edit it reverses.
    ///
    /// `clay_document_undo_bound` (ABI 0.40.0) reports the world box instead.
    /// It is a *world* region and not a layer's, which is also more correct
    /// than what it replaces: an undo of an edit on some other subtool used to
    /// re-mesh the active one and leave the layer that changed stale.
    ///
    /// The engine's own warning against the alternative is worth keeping here:
    /// the region cannot be worked out by diffing the layer's nodes across the
    /// call, because "an undone move, resize or colour edit keeps its node id,
    /// the diff sees nothing, and under-dirtying leaves stale bricks at a
    /// blend seam".
    fn refill_what_a_step_reached(&mut self, reached: Influence) -> Result<(), ModelError> {
        match reached {
            // A step that cannot change the field — a rename — reports this,
            // and so does one that changed nothing.
            Influence::Nothing => Ok(()),
            Influence::Box { min, max } => self.refill_region(min, max),
            // A non-local op anywhere in the subtree, an infinite repeat, an
            // unbounded primitive: there is no finite box and the honest
            // response is the one that was taken unconditionally before.
            Influence::Everything => {
                let layer = self.active_layer().id;
                self.refill(layer, &[])
            }
        }
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
        self.cross(direction, cell_size, blur, false)
    }

    /// The same crossing, with the source replaced by what it produced.
    ///
    /// The layer it read leaves as the result arrives, and the result takes
    /// its place in the stack — what a sculptor means by converting *this*
    /// layer, rather than gaining a second one beside the first.
    ///
    /// Still one undo. The removal happens inside the group the crossing
    /// already opens, so the engine takes the filling and the removal back
    /// together and the source comes back with it; a removal outside the
    /// group would have needed two.
    ///
    /// The result keeps its derived name — `Forma · voxel` rather than
    /// `Forma` — because that name says what the layer now holds, and because
    /// a voxel grid is reachable only by name (ClayCore #365): handing the
    /// result the source's name would put two layers through one grid for as
    /// long as an undo kept both in the document.
    pub fn convert_layer_in_place(
        &mut self,
        direction: Direction,
        cell_size: f32,
        blur: i32,
    ) -> Result<LayerKey, ModelError> {
        self.cross(direction, cell_size, blur, true)
    }

    fn cross(
        &mut self,
        direction: Direction,
        cell_size: f32,
        blur: i32,
        in_place: bool,
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

        // Made unique here as well, and this is the path that most needs it:
        // the crossing is what actually creates voxel layers, and a grid is
        // reachable only by name (ClayCore #365). Crossing one source twice
        // gave two layers called "Forma · voxel", both resolving to the first
        // grid — so a stroke aimed at the second wrote into the first, the
        // chunks were meshed from the wrong grid, and `rename_layer` refused to
        // untangle it because the name it would set was already taken.
        let name = self.unique_layer_name(&format!("{} · {}", source.name, direction.to().label()));
        // Where the source stands, so the result can take its place, and its
        // key, so it can be removed once the result is filled from it.
        let (replacing, at) = (source.key, self.active);
        // Bracketed, because a crossing is several engine edits — the layer,
        // then whatever fills it — and a sculptor asked for one thing. Without
        // the group, undo took back the filling and left the empty layer
        // standing, which is the shape `a_crossing_is_taken_back_by_undo` in
        // `clayspace-engine/tests/conversion.rs` caught.
        let depth_before = self.engine_undo_depth();
        self.document
            .begin_undo_group()
            .map_err(ModelError::engine)?;
        let made = match direction {
            Direction::SdfToVoxel => self.rasterize_to_voxels(&name, cell_size),
            Direction::VoxelToSdf => self.voxels_to_sdf(&name, blur),
            Direction::MeshToVoxel => self.mesh_to_voxels(&name, cell_size),
            Direction::MeshToSdf => self.mesh_to_sdf(&name, cell_size),
            Direction::SdfToMesh => self.sdf_to_mesh(&name, cell_size),
            Direction::VoxelToMesh => self.voxels_to_mesh(&name),
            Direction::MeshToMultires => self.mesh_to_multires(&name),
            Direction::MultiresToMesh => self.multires_to_mesh(&name),
        };
        // The source leaves and the result takes its row here rather than
        // after the group: it keeps the entries adjacent, which is what lets
        // them be taken back together.
        let replaced = match (in_place, &made) {
            (true, Ok(made)) => self
                .remove_layer(replacing)
                .and_then(|()| self.move_layer(*made, at)),
            _ => Ok(()),
        };
        // Closed on the failing path too: a group left open swallows every
        // edit after it into one undo step, which is a worse bug than the one
        // that opened it.
        let closed = self.document.end_undo_group().map_err(ModelError::engine);
        let made = made?;
        closed?;
        replaced?;
        // Recorded so undo takes the whole crossing back rather than emptying
        // the layer it just made. See `crossing_undo`.
        if let Ok(row) = self.index_of(made) {
            let layer = self.layers[row].id;
            let engine_depth = self.engine_undo_depth();
            self.crossing_undo.push(Crossing {
                layer,
                engine_depth,
                // Measured across the whole crossing rather than assumed to be
                // one: an in-place crossing removes a layer and moves another,
                // and neither goes into the group.
                steps: engine_depth.saturating_sub(depth_before).max(1),
            });
            self.crossing_redo.clear();
        }
        Ok(made)
    }

    /// The engine entries the recorded crossings hold past their first: what
    /// the depth must not count, since each crossing is one thing a sculptor
    /// did.
    fn crossing_entries(crossings: &[Crossing]) -> usize {
        crossings.iter().map(|c| c.steps.saturating_sub(1)).sum()
    }

    /// Whether the newest crossing is more recent than the newest engine
    /// entry, and so the thing an undo should take back.
    fn crossing_is_newest(&self) -> bool {
        self.crossing_undo
            .last()
            .is_some_and(|crossing| crossing.engine_depth == self.engine_undo_depth())
    }

    /// Takes one crossing back: the engine takes back the filling, and the
    /// layer it filled leaves the scene.
    fn undo_crossing(&mut self) -> Result<bool, ModelError> {
        let Some(crossing) = self.crossing_undo.pop() else {
            return Ok(false);
        };
        // Every entry the crossing left, so an in-place one gives back the
        // layer it read as well as the filling it made.
        for _ in 0..crossing.steps {
            self.document.undo().map_err(ModelError::engine)?;
        }
        self.suppressed.insert(crossing.layer);
        self.crossing_redo.push(crossing);
        self.after_crossing_history()
    }

    /// Puts one crossing back, filling and all.
    fn redo_crossing(&mut self) -> Result<bool, ModelError> {
        let Some(crossing) = self.crossing_redo.pop() else {
            return Ok(false);
        };
        for _ in 0..crossing.steps {
            self.document.redo().map_err(ModelError::engine)?;
        }
        self.suppressed.remove(&crossing.layer);
        self.crossing_undo.push(crossing);
        self.after_crossing_history()
    }

    /// The bookkeeping either direction needs: the scene changed shape, and
    /// what the layer covered is stale either way.
    fn after_crossing_history(&mut self) -> Result<bool, ModelError> {
        self.reconcile_layers();
        let layer = self.active_layer().id;
        self.refill(layer, &[])?;
        self.resync_armature();
        Ok(true)
    }

    /// Every layer's visibility as it stands.
    fn visibility_snapshot(&self) -> Vec<(LayerKey, bool)> {
        self.layers
            .iter()
            .map(|layer| (layer.key, layer.visible))
            .collect()
    }

    /// Writes a visibility pattern and files what it cost the engine's history
    /// as one gesture undo hops over.
    ///
    /// A flag already set is left alone rather than written again: the engine
    /// records the command whether or not it changes anything — measured, a
    /// second hide of a hidden layer raises the undo depth — and an entry that
    /// changes nothing is still an entry to step over.
    fn write_visibility(
        &mut self,
        wanted: &[(LayerKey, bool)],
        after: Option<Solo>,
    ) -> Result<(), ModelError> {
        let before = self.solo.clone();
        let mut depths = Vec::new();
        let outcome = self.write_each_visibility(wanted, &mut depths);
        // The state only reaches the one asked for if every flag did. A batch
        // that failed halfway is a batch whose caller is about to restore.
        if outcome.is_ok() {
            self.solo = after;
        }
        // Recorded even when it failed halfway, because half a batch is still
        // entries in the engine's history and undo has to step over those too.
        if !depths.is_empty() {
            self.visibility_undo.push(VisibilityGesture {
                depths,
                before,
                after: self.solo.clone(),
            });
            self.visibility_redo.clear();
        }
        outcome
    }

    /// The writes themselves, recording each depth as it lands.
    ///
    /// Apart from [`Self::write_visibility`] so that the record is kept by a
    /// caller that owns it however this ends — including where a layer refuses
    /// midway.
    fn write_each_visibility(
        &mut self,
        wanted: &[(LayerKey, bool)],
        depths: &mut Vec<usize>,
    ) -> Result<(), ModelError> {
        for &(key, visible) in wanted {
            // A layer the snapshot names and the document no longer has: undo
            // may have taken a crossing back since. Skipped rather than
            // refused, because there is nothing to put back and refusing would
            // strand the layers that are still there.
            let Ok(index) = self.index_of(key) else {
                continue;
            };
            if self.layers[index].visible == visible {
                continue;
            }
            SceneModel::set_layer_visible(self, key, visible)?;
            depths.push(self.engine_undo_depth());
        }
        Ok(())
    }

    /// Runs `body` with this visibility pattern in force, and puts back what
    /// every layer had however it ends.
    ///
    /// The restore owns the exit. `body` returning an error, or returning
    /// early, or refusing before it has done anything, all arrive here the
    /// same way — no *return* from this function leaves the document showing
    /// what the operation wanted rather than what the sculptor set. That is not
    /// a theoretical care: baking one subtool alone means hiding the sculptor's
    /// whole scene, and a bake that refuses halfway would otherwise leave it
    /// hidden.
    ///
    /// A panic unwinding out of `body` is the one exit this does not cover, and
    /// it cannot be from here: the restore needs `&mut self` and `body` is
    /// holding it, so there is no `Drop` guard to hang it on. Nothing in this
    /// application catches one — a panic ends the process — so the document a
    /// half-restored visibility would be left in is a document nobody goes on
    /// to use. Said out loud because the promise above is otherwise read as
    /// covering it.
    fn with_visibility<T>(
        &mut self,
        wanted: &[(LayerKey, bool)],
        body: impl FnOnce(&mut Self) -> Result<T, ModelError>,
    ) -> Result<T, ModelError> {
        let was = self.visibility_snapshot();
        // The solo is a fact about the scene, not about the window this opens:
        // an operation that hides everything but one layer has not released a
        // solo, and the flags it borrowed are given back below.
        let solo = self.solo.clone();
        if let Err(e) = self.write_visibility(wanted, solo.clone()) {
            let _ = self.write_visibility(&was, solo);
            return Err(e);
        }
        let outcome = body(self);
        let restored = self.write_visibility(&was, solo);
        // The body's failure is the one worth reporting; the restore's is
        // reported only if the body succeeded. Ordered as `convert_layer`
        // orders its group, and for the same reason.
        let value = outcome?;
        restored?;
        Ok(value)
    }

    /// Runs `body` with only these layers shown, and restores the rest
    /// afterwards.
    ///
    /// The primitive the subtool boolean bakes through: `clay_item_volume_from_document`
    /// samples the whole document's field, and the engine's contract is that a
    /// hidden layer "contributes nothing to the field; showing it again
    /// restores the original field exactly" — so baking one subtool alone *is*
    /// hiding the others around the bake.
    ///
    /// Public because that caller wants it and because the restore is a
    /// promise worth testing on its own, with an operation that fails inside.
    pub fn with_only_visible<T>(
        &mut self,
        shown: &[LayerKey],
        body: impl FnOnce(&mut Self) -> Result<T, ModelError>,
    ) -> Result<T, ModelError> {
        let wanted: Vec<(LayerKey, bool)> = self
            .layers
            .iter()
            .map(|layer| (layer.key, shown.contains(&layer.key)))
            .collect();
        self.with_visibility(&wanted, body)
    }

    /// Whether the newest thing in the engine's history is a visibility
    /// gesture this side made.
    ///
    /// True when no engine edit has landed since — any that had would have
    /// raised the depth past the last one the gesture recorded.
    fn visibility_is_newest(&self) -> bool {
        self.visibility_undo
            .last()
            .and_then(|gesture| gesture.depths.last())
            .is_some_and(|depth| *depth == self.engine_undo_depth())
    }

    /// Steps back over every visibility gesture sitting on top of the history.
    ///
    /// Hopped rather than stopped on: the spec says a solo "SHALL NOT change
    /// which layer is active or add entries to the undo history", and the
    /// engine gives no way to keep the commands out of the journal, so the
    /// only place the promise can be kept is here. A solo and its release
    /// cancel exactly — the hides are taken back and then the shows are, and
    /// what the sculptor set is what remains — so a ⌘Z after a released solo
    /// reaches the edit underneath it.
    fn hop_visibility_back(&mut self) -> Result<(), ModelError> {
        while self.visibility_is_newest() {
            let Some(gesture) = self.visibility_undo.pop() else {
                break;
            };
            for _ in &gesture.depths {
                if !self.document.undo().map_err(ModelError::engine)? {
                    break;
                }
            }
            self.solo = gesture.before.clone();
            self.visibility_redo.push(gesture);
            self.after_visibility_history()?;
        }
        Ok(())
    }

    /// The mirror: steps forward over the gestures a hop back put away.
    fn hop_visibility_forward(&mut self) -> Result<(), ModelError> {
        while self
            .visibility_redo
            .last()
            .and_then(|gesture| gesture.depths.first())
            .is_some_and(|depth| *depth == self.engine_undo_depth() + 1)
        {
            let Some(gesture) = self.visibility_redo.pop() else {
                break;
            };
            for _ in &gesture.depths {
                if !self.document.redo().map_err(ModelError::engine)? {
                    break;
                }
            }
            self.solo = gesture.after.clone();
            self.visibility_undo.push(gesture);
            self.after_visibility_history()?;
        }
        Ok(())
    }

    /// What either direction owes once the engine has moved the flags.
    fn after_visibility_history(&mut self) -> Result<(), ModelError> {
        // `reconcile_layers` re-reads what the document now shows, so the eye
        // in the stack follows the hop rather than sitting where the gesture
        // left it.
        self.reconcile_layers();
        let layer = self.active_layer().id;
        // A hidden layer contributes nothing to the field, so the surface is a
        // different surface either way and the bound is the whole layer.
        self.refill(layer, &[])?;
        Ok(())
    }

    /// Drops the visibility gestures a new edit has invalidated, and records
    /// what the engine holds forward now.
    ///
    /// Run on both sides of every history step, which is the only moment this
    /// side can look. See [`ClayDocument::redo_room`] for what the two
    /// conditions mean and for the defect that has no other floor under it:
    /// unlike a mesh gesture or a crossing, a visibility gesture is hopped
    /// *without being asked for*, so a stale one does not merely answer a
    /// question wrongly — it silently spends a step the sculptor meant for
    /// their own work.
    fn settle_history_room(&mut self) {
        let room = self
            .document
            .undo_state()
            .map(|state| state.redo_depth)
            .unwrap_or(0);
        if room == 0 || room < self.redo_room {
            self.visibility_redo.clear();
        }
        self.redo_room = room;
    }

    /// Lets go of a solo whose subtool has left the document.
    ///
    /// The visibility the solo borrowed is given back rather than merely
    /// forgotten: what the solo hid is still hidden, and the row that would
    /// release it is the one that was just removed. Measured before this —
    /// solo the second of two subtools, remove it, and the document reported
    /// `soloed Some(LayerKey(2))` over a scene whose only remaining layer was
    /// hidden, with the viewport blank and no control anywhere that could put
    /// it back.
    ///
    /// A removal that is not the soloed one only prunes the snapshot, so the
    /// pattern `save` writes describes the layers the document still has.
    fn release_solo_of(&mut self, gone: LayerKey) -> Result<(), ModelError> {
        let Some(solo) = self.solo.clone() else {
            return Ok(());
        };
        if solo.layer != gone {
            if let Some(held) = &mut self.solo {
                held.was.retain(|(key, _)| *key != gone);
            }
            return Ok(());
        }
        self.write_visibility(&solo.was, None)
    }

    fn undo_step(&mut self) -> Result<bool, ModelError> {
        // A mesh gesture is asked about *before* the hop as well as after.
        //
        // It records the engine's depth and does not raise it, so a solo
        // engaged before the stroke ends at exactly the depth the gesture
        // remembers and both answer "newest" — and only one of them can be.
        // The stroke is: had the solo come after it, its writes would have
        // carried the depth past what the gesture recorded. Hopping first
        // stepped over the solo and then undid the engine entry *underneath*
        // the stroke — measured, a dab on a soloed mesh subtool undone once
        // released the solo and took back the import that made the layer, and
        // the stroke's own gesture was stranded at a depth the engine would
        // never return to.
        if self.mesh_gesture_is_newest() {
            return self.undo_mesh_gesture();
        }
        // Solo, and the hide-and-restore a bake borrows, sit on top of the
        // engine's history without being anything the sculptor did. Stepped
        // over here, so that what follows is asked of the newest *edit*
        // rather than of a way of looking at the scene.
        self.hop_visibility_back()?;
        // Whichever history holds the more recent edit answers. See
        // `mesh_undo` for why depth is what orders them.
        if self.mesh_gesture_is_newest() {
            return self.undo_mesh_gesture();
        }
        // A crossing sits on its own engine entry, so it is tested the same
        // way and before the plain path: undoing only the engine's half would
        // leave the layer it made standing and empty.
        if self.crossing_is_newest() {
            return self.undo_crossing();
        }
        let stepped = self.document.undo_bound().map_err(ModelError::engine)?;
        let moved = stepped.moved;
        if moved {
            self.reconcile_layers();
            self.refill_what_a_step_reached(stepped.reached)?;
            self.resync_armature();
            // The engine reverted whatever it reverted and cannot tell the
            // object table it did; the table follows by depth, and so does
            // where each layer stands.
            self.resync_objects();
            self.resync_layer_transforms();
        }
        Ok(moved)
    }

    fn redo_step(&mut self) -> Result<bool, ModelError> {
        // The mirror of `undo`'s first check, and it is first here for the same
        // reason. A mesh undo moves no engine depth, so a solo undone under the
        // stroke leaves its gesture sitting at depth + 1 and the hop's guard is
        // satisfied by a gesture that is not what was taken back last. Measured:
        // the redo went to the solo, the engine depth moved past what the mesh
        // gesture recorded, and the stroke could never be put back — the
        // interface said "nothing to redo" over a stroke it still held.
        if self.mesh_redo_is_next() {
            return self.redo_mesh_gesture();
        }
        // The mirror of the hop in `undo`, and it runs on both sides of the
        // step: a gesture may be the next entry forward — a solo taken back
        // with no edit under it — and more of them may sit above whatever is
        // redone here.
        self.hop_visibility_forward()?;
        // The mirror of `undo`: a mesh gesture on the redo stack recorded at
        // the current engine depth is the one that was taken back last.
        if self.mesh_redo_is_next() {
            return self.redo_mesh_gesture();
        }
        // The mirror: an undone crossing's entry is the next one forward.
        if self
            .crossing_redo
            .last()
            .is_some_and(|crossing| crossing.engine_depth == self.engine_undo_depth() + 1)
        {
            return self.redo_crossing();
        }
        let stepped = self.document.redo_bound().map_err(ModelError::engine)?;
        let moved = stepped.moved;
        if moved {
            self.reconcile_layers();
            self.refill_what_a_step_reached(stepped.reached)?;
            self.resync_armature();
            // The engine reverted whatever it reverted and cannot tell the
            // object table it did; the table follows by depth, and so does
            // where each layer stands.
            self.resync_objects();
            self.resync_layer_transforms();
        }
        // And the gestures that sat above the entry just put back.
        self.hop_visibility_forward()?;
        Ok(moved)
    }

    /// How many of the engine's entries are gestures rather than edits.
    ///
    /// Subtracted from the depth the interface is shown: solo adds nothing the
    /// sculptor would have to undo, and a history that counted its commands
    /// would offer an Undo that takes back a way of looking at the scene.
    fn visibility_entries(stack: &[VisibilityGesture]) -> usize {
        stack.iter().map(|gesture| gesture.depths.len()).sum()
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

    fn mesh_to_sdf(&mut self, name: &str, cell_size: f32) -> Result<LayerKey, ModelError> {
        let engine_name = self.active_layer().engine_name.clone();
        // The chosen cell rather than `VolumeParams::default()`, whose `None`
        // "picks from the source's own size". The crossing samples onto a
        // lattice either way — see `Direction::chooses_resolution` — so the
        // resolution the panel states its costs for has to be the resolution
        // it is done at, or the figures describe a different crossing.
        let layer = self
            .document
            .mesh_layer_to_sdf_layer(&engine_name, name, Self::bake_volume(cell_size))
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

    /// The active mesh layer, taken as the cage of a subdivision hierarchy.
    ///
    /// The one crossing that samples nothing: the cage *is* the mesh, kept
    /// vertex for vertex as level 0. What it does instead of losing accuracy
    /// is refuse — `clay_multires_from_mesh` will not repair a non-manifold
    /// edge or weld away a degenerate face, because a conversion that quietly
    /// mended a cage would change retopology somebody paid for without saying
    /// so, and a cage is precisely the thing whose topology is the work.
    ///
    /// The row that comes out carries the cage *the hierarchy welded*, taken
    /// back off it as level 0, rather than the triangles that went in. Those
    /// two can differ — a cage is welded on the way in — and the row has to
    /// hold what the hierarchy is actually standing on, or the layer saved to
    /// the file and the layer the sculpt is stored against would be two
    /// different meshes.
    fn mesh_to_multires(&mut self, name: &str) -> Result<LayerKey, ModelError> {
        let source = self.active_layer();
        if !source.carries_geometry {
            return Err(ModelError::Conversion(Refusal::SourceEmpty));
        }
        let id = source.id;
        let mut hierarchy = self
            .document
            .multires_from_mesh_layer(id, crate::multires::Hierarchy::desc())
            .map_err(
                |refused| match crate::multires::cage_fault(refused.reason) {
                    Some(fault) => ModelError::Conversion(Refusal::NotACage { fault }),
                    None if refused.reason == claycore::MultiresError::EmptyBase => {
                        ModelError::Conversion(Refusal::SourceEmpty)
                    }
                    None => ModelError::engine(refused.to_string()),
                },
            )?;
        let cage = hierarchy.copy_level_mesh(0).map_err(ModelError::engine)?;
        let key = self.attach_meshed_layer(cage, name)?;
        if let Ok(index) = self.index_of(key) {
            self.layers[index].representation = Representation::Multires;
            self.layers[index].multires = Some(crate::multires::Hierarchy::holding(hierarchy));
        }
        // A hierarchy is drawn from its display level and never from the cage
        // its layer holds, so the box every manipulator sizes itself to comes
        // from there too.
        self.refresh_multires_bounds(key);
        // The mesh sculptor `attach_meshed_layer` armed is for the cage, and
        // nothing sculpts the cage directly once a hierarchy stands on it: a
        // stamp goes through the hierarchy's own sculptor at the bound level.
        // Left standing it would answer the pick with the cage's triangles.
        self.mesh_sculptors.borrow_mut().forget(key);
        Ok(key)
    }

    /// The active hierarchy's display level, baked into an ordinary mesh.
    ///
    /// A level *is* a mesh, so nothing is resampled here either. What goes is
    /// everything under the level — the cage, the levels between, and the
    /// detail stored per level in its own transported frame — so afterwards
    /// the vertices are where they were and there is nothing beneath them left
    /// to move.
    fn multires_to_mesh(&mut self, name: &str) -> Result<LayerKey, ModelError> {
        let index = self.active;
        let Some(hierarchy) = self.layers[index].multires.as_mut() else {
            return Err(ModelError::Conversion(Refusal::SourceEmpty));
        };
        let level = hierarchy.levels().display;
        let baked = hierarchy
            .surface_mut()
            .copy_level_mesh(level)
            .map_err(ModelError::engine)?;
        if baked.index_count() == 0 {
            return Err(ModelError::Conversion(Refusal::SourceEmpty));
        }
        self.attach_meshed_layer(baked, name)
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
        self.layers.push(Layer::new(id, key, name, representation));
        // Through the one activation call rather than by assigning the index:
        // a new layer becoming the sculpt target is the same fact a stack click
        // states, and everything activation owes is owed here too — arming a
        // mesh for sculpting among it.
        self.set_active_layer(key)?;
        Ok(key)
    }

    /// What every direction owes once its new layer exists.
    fn after_conversion(&mut self, key: LayerKey) -> Result<LayerKey, ModelError> {
        self.reconcile_layers();
        // Where the new grid is, if it is one. `clay_layer_bounds` reports a
        // layer's *SDF* extent and a grid has none, so this cache is the only
        // account of a voxel layer's box there is — and it was refreshed by a
        // voxel stroke and by opening a file, but not by the crossing that
        // creates the grid. A rasterized layer therefore reported no extent
        // until the first dab landed on it: Frame All framed the default box,
        // and a boolean naming it as an operand refused it as empty.
        self.refresh_sculpt_layers(key)?;
        // A mesh layer has no bricks and is not evaluated, so there is nothing
        // to refill for one — the viewport draws it through the carried-layer
        // path instead. Marking it dirty would ask the cache to mark a layer
        // whose field is empty.
        // A hierarchy's layer is a mesh layer too — it holds the cage — so it
        // has no bricks and no field for the same reason.
        if matches!(
            self.active_layer().representation,
            Representation::Mesh | Representation::Multires
        ) {
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

    /// How every document's brick cache is tuned.
    ///
    /// One constant rather than a literal per constructor: a document that is
    /// opened has to mesh the way a document that is made does, and the two
    /// used to be hand-copied — re-tuning `dim` in one of them would have left
    /// every *opened* document on the old size with nothing to catch it.
    ///
    /// 8-cell bricks. 16 was tried: it covers the surface in a third as many
    /// keys but each holds eight times the cells, and a dilated dirty set then
    /// meshes more cells overall — 64 ms against 39 ms on the same edit.
    const BRICK_CONFIG: BrickConfig = BrickConfig {
        dim: 8,
        voxel_size: Self::VOXEL_SIZE,
        band_voxels: 3,
        memory_budget: Some(512 * 1024 * 1024),
        colors: false,
    };

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
    ///
    /// Compared against the *layer's* record and not one number for the
    /// document. With one number, a switch of subtool left it holding the
    /// outgoing subtool's axes, so a stroke on the incoming one that asked for
    /// the same axes wrote nothing and mirrored against whatever plane that
    /// layer happened to carry.
    fn point_the_mirror(&mut self, symmetry: [bool; 3]) -> Result<(), ModelError> {
        let index = self.active;
        if self.layers[index].mirror == symmetry {
            return Ok(());
        }
        let layer = self.layers[index].id;
        self.document
            .set_layer_mirror(layer, symmetry, 0.0)
            .map_err(ModelError::engine)?;
        self.layers[index].mirror = symmetry;
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
        // Kept whole and unreflected, because the commit reflects it again.
        if self.live_smooth.is_some() && matches!(tool, ToolKind::Suavizar | ToolKind::Relaxar) {
            self.live_gesture
                .get_or_insert_with(|| (tool, brush, symmetry, Vec::new()))
                .3
                .extend_from_slice(samples);
        }
        // The live drag is not reflected here, and that is the engine's rule
        // rather than an omission: `clay_sdf_move_*` reflects the drag into
        // every image the layer emits of it — one grab per image, which
        // `LiveMove::draw` writes — where the baked verbs below have to be
        // reflected by hand because the layer mirror cannot reach them.
        if tool == ToolKind::Mover && (self.live_move.is_some() || self.live_move_armed) {
            return self.live_move_drag(brush, samples);
        }
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
                // The same gesture with the reach measured through the
                // material instead of through space.
                ToolKind::MoverTopologico => self.topological_move_stroke(brush, &reflected)?,
                // Bake-and-relax over the region the stroke covered.
                ToolKind::Suavizar | ToolKind::Relaxar if self.live_smooth.is_some() => {
                    self.live_relax_dab(brush, &reflected)?
                }
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

    /// Which of the field's three routes a tool takes, once the gesture is in
    /// the layer's own frame.
    fn field_stroke(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        match tool {
            // The verbs that rewrite the field rather than adding an item.
            // The layer mirror cannot reach those, so their strokes are
            // reflected instead — see `baked_stroke`.
            ToolKind::Mover
            | ToolKind::MoverTopologico
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
        self.point_the_mirror(symmetry)?;
        // Every tool that reaches here combines a stamp with the surface.
        // There is no catch-all arm: the one that was here mapped anything
        // unlisted to `Op::Add`, which adds a *sphere* — so the planing tools
        // deposited blobs and nothing said so. A tool with no mapping refuses.
        let Some(recipe) = sdf_recipe(tool) else {
            return Err(ModelError::engine(format!(
                "{} has no mapping onto an SDF verb; it should not have been \
                 offered on this layer",
                tool.label()
            )));
        };
        let layer = self.active_layer().id;
        // The shared preset, then the two fields a named brush states for
        // itself. Spacing is scaled rather than replaced so Fluxo still does
        // something on a brush with a dense stroke of its own.
        let shared = self.preset(brush, tool);
        let preset = StrokePreset {
            spacing: (shared.spacing * recipe.spacing).clamp(0.05, 0.9),
            accumulation: recipe.accumulation.unwrap_or(shared.accumulation),
            ..shared
        };
        let stroke: Vec<claycore::StrokeSample> = samples
            .iter()
            .map(|s| claycore::StrokeSample {
                position: s.position,
                pressure: s.pressure,
                time: s.time,
            })
            .collect();

        // Turned over where the modifier is held and the operation has an
        // opposite: Add becomes Subtract, Emboss becomes Engrave, Relief
        // becomes Incise. An operation with no opposite — Intersect, Replace,
        // a seam — is left as it is rather than quietly becoming some other
        // verb, which is what `inverted` answering `None` means.
        let combine = {
            let panel = self.combine.sanitized();
            // A *named* brush sets its own operation and ignores the panel,
            // the way Camada already forces clamped accumulation whatever
            // Acumular says: Vinco is the incise and Argila is the relief, and
            // a Vinco set to Subtrair would be a tool that is not Vinco. The
            // three general strokes take what the panel is set to, because
            // shaping them is what the panel is for.
            let settings = match recipe.op {
                Some(op) => clayspace_model::CombineSettings { op, ..panel },
                None => panel,
            };
            match brush.invert.then(|| settings.op.inverted()).flatten() {
                Some(op) => clayspace_model::CombineSettings { op, ..settings },
                None => settings,
            }
        };

        // The engine's own equivalence table binds Padrão and Inflar to the
        // same op on a field — relief moves the surface along its own normal,
        // which is what both do — so the two came out identical: the same
        // stamp, the same amplitude, the same rim. What tells them apart in
        // ZBrush is the profile. Standard raises a ridge that follows the
        // falloff; Inflate swells the whole footprint, broader and lower at
        // the rim. So Inflar takes a wider region with a wider rim and asks
        // for a little less lift — the swell — and Padrão keeps the standard
        // clay mapping, k = rounding = radius: the ridge.
        let size = brush.sanitized().size;
        let (region, lift) = (size * recipe.reach, recipe.lift);
        let mut stamp = Item::sphere(region).map_err(ModelError::engine)?;
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
        // The panel's join width is a world distance and the stamp is placed
        // in the layer's own frame, so a scaled subtool takes it scaled — the
        // same conversion `apply_stroke` makes to the brush radius. `size` is
        // already in that frame, having come through it.
        let carried_scale = self
            .carried_placement(self.active_layer().key)
            .map(|transform| transform.largest_scale())
            .unwrap_or(1.0);
        let join = combine.radius / carried_scale;
        let distance = if combine.op.displaces_along_the_normal() {
            lift * if join > 0.0 { join } else { size }
        } else {
            join
        };
        stamp
            .set_blend(engine_blend(combine.blend), distance)
            .map_err(ModelError::engine)?;
        // The item's rounding is the falloff width, and it was never set at
        // all. Measured, going from zero to the brush radius tripled the
        // displacement — leaving it at zero was throwing away most of the
        // brush as well as its soft edge.
        stamp.set_rounding(region).map_err(ModelError::engine)?;

        // No alpha here, and `alpha_for` is what says so rather than a
        // condition repeated at this call site. A field takes one as a
        // deformer on an item, and `clay_layer_apply_stroke` uses its item as
        // a template scaled per stamp — the deformer chain does not travel
        // with it. Measured, and recorded in
        // `claycore/tests/alpha_deformer.rs`.

        let mask = self.active_mask_source();

        // The stamp is gated by the layer's own mask, which is what makes a
        // mask protect a surface from the *operation* and not only from the
        // brush. The mask a stroke consumes keeps a stamp from being
        // deposited where it protects; it says nothing about the boolean each
        // deposited stamp then performs, so a subtraction crossing a masked
        // ear used to take the ear anyway.
        //
        // Set on the *template* and correct for every stamp, because the gate
        // is in world space and does not travel with the item: the engine's
        // header is explicit that "the region it protects is where you
        // painted it and stays there whatever `clay_item_set_position` ...
        // then do to the gated item". So this is unlike the alpha above,
        // which is in the item's own frame and is why a stroke cannot carry
        // one.
        //
        // Until ClayCore 0.67.0 the gate was placed by the transform of the
        // item it protected, so a stamp with a placement — which every stamp
        // in a stroke has — carried its protection off to somewhere the
        // sculptor had not painted, and the call looked inert at every
        // threshold and width. That is CyberdyneCorp/ClayCore#394, fixed in
        // the 0.73.0 pin, and it is why this call was removed and is now
        // back. `claycore/tests/mask_gate.rs` measures it at the boundary.
        //
        // Refusal is not failure. The engine refuses a gate that would
        // protect nothing — an empty mask, or one no cell of which reaches
        // the threshold — rather than reporting a success that does nothing,
        // and an ungated stamp is exactly right in that case.
        //
        // And only for the operations that can take material away, which is a
        // correction rather than an optimisation dressed as one. The engine
        // *measures* the mask into a signed distance on every `set_gate`, and
        // a stroke arrives as one call per dab: measured, that is 3.8 ms on a
        // 36,000-cell mask, every dab, for a mask that has not changed. Gating
        // unconditionally took `mask.gated_ratio` from 0.92 — a frozen region
        // costing a stroke nothing — to 8.00, and the benchmark refused the
        // change for it.
        //
        // What makes the narrowing correct rather than a trade is that the
        // additive half was never the gap. Authoring gating already keeps a
        // stamp from being deposited where the mask protects, close to totally
        // — `a_mask_still_keeps_a_brush_from_depositing` measured 1.0005
        // against an unmasked 1.1400 and passed before any of this. The gap
        // the engine names is the item already in the edit list whose reach
        // *removes*: "a mask over an ear has never done anything about the
        // next boolean." So the gate goes where the boolean is.
        if combine.op.takes_material_away() {
            if let Some(painted) = self.document.layer_mask(layer) {
                let _ = stamp.set_gate(&painted, Self::GATE_THRESHOLD, Self::GATE_WIDTH);
            }
        }

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

    /// The paint level a gated stamp treats as protected.
    ///
    /// Half, which is also what the engine takes for a threshold at or below
    /// zero — spelled rather than defaulted, because the number decides what
    /// a half-pressure edge of the mask brush means and that is the
    /// application's decision to have made.
    const GATE_THRESHOLD: f32 = 0.5;

    /// How far a stamp's protection fades across, in world units.
    ///
    /// The engine measures the mask into a signed distance and derives the
    /// falloff from this rather than from the brush edge that painted it, so
    /// this is the only control over how hard the protected boundary is. It
    /// trades against the march: the header's own note is that "a WIDE gate
    /// costs almost no step scale and a NARROW one costs honestly", because
    /// a narrow fade is a steep field.
    ///
    /// Four cells of the brick cache. One or two cells is a boundary the
    /// viewport cannot resolve anyway, paid for in step scale; much wider and
    /// the protection bleeds visibly into material the sculptor left unpainted.
    const GATE_WIDTH: f32 = Self::VOXEL_SIZE * 4.0;

    /// How much wider than the brush Inflar's region and rim are, against
    /// Padrão's. Wide enough that the swell reads as a swell beside the
    /// ridge, not so wide that a stroke reaches things the sculptor did not
    /// brush.
    const INFLATE_REACH: f32 = 1.35;
    /// How much of the standard lift Inflar asks for.
    ///
    /// Measured rather than chosen, on the starting form with a 0.25 brush,
    /// as the peak height above the sphere and the footprint area a raycast
    /// grid finds above it:
    ///
    ///   binding                       peak    footprint   height/width
    ///   Padrão, k = rounding = r     +0.180      1179        0.0053
    ///   Inflar at 0.8 of the lift    +0.238      1939        0.0054
    ///   Inflar at 0.32 of the lift   +0.173      1772        0.0041
    ///
    /// The middle row is why this is not 0.8: a wider region under buildup
    /// accumulation lifts each point through more stamps, so the mark came
    /// out wider *and* taller — the same ridge drawn with a bigger brush,
    /// which is not what Inflate means. At 0.32 the footprint is half again
    /// as wide as Padrão's at a fifth less slope: a swell rather than a ridge.
    const INFLATE_LIFT: f32 = 0.32;

    /// Argila's footprint and stroke, against Padrão's.
    ///
    /// Clay is Standard with buildup on and a denser stroke — that is what
    /// separates the two in ZBrush, and it is what separates them here rather
    /// than a second engine verb. A little wider than Padrão because a pat of
    /// clay is broader than a ridge, and less lift per stamp because buildup
    /// adds them together: at Padrão's own lift a single pass already reaches
    /// the engine's amplitude ceiling and a second adds nothing, which is a
    /// bigger brush rather than a different one.
    ///
    /// Measured on the starting form with a 0.2 brush at three tenths
    /// intensity, taking the surface height under the stroke:
    ///
    ///   passes   Argila   Camada
    ///   one      1.0961    —
    ///   two      1.1400   1.0455
    ///
    /// Which is the distinction the accumulation is for: a second pass of
    /// Argila adds 0.044, and two passes of the clamped tool reach less than
    /// one pass of the building one.
    const CLAY_REACH: f32 = 1.15;
    const CLAY_LIFT: f32 = 0.55;
    /// Stamps closer together than Fluxo alone asks for: buildup is what makes
    /// clay read as clay, and buildup needs overlap.
    const CLAY_SPACING: f32 = 0.5;

    /// Vinco's footprint and stroke.
    ///
    /// Narrow, because the line *is* the brush: the engine's note on incise
    /// says "a thin region gives the line", and a crease at the full brush
    /// radius is a gouge. Full lift, so the trough goes as deep as the brush
    /// allows, and tight spacing so it is continuous rather than a row of pits.
    ///
    /// The number is measured rather than chosen. On the starting form with a
    /// 0.2 brush, taking how far the surface has moved at each distance to the
    /// side of the stroke — Padrão inverted in the last column, as the widest
    /// cut the same brush can make:
    ///
    ///   aside   reach 0.35   reach 0.5   reach 0.6   reach 0.7   Padrão
    ///   0.00      −0.020      −0.020      −0.100      −0.100     −0.100
    ///   0.03      −0.027      −0.020      −0.045      −0.099     −0.099
    ///   0.05      −0.004      −0.028      −0.044      −0.063     −0.099
    ///   0.08       0.000       0.000      −0.001      −0.008     −0.073
    ///   0.11       0.000       0.000       0.000       0.000     −0.026
    ///   0.14       0.000       0.000       0.000       0.000      0.000
    ///
    /// Below 0.6 the trough is a fifth of the depth the same brush can cut —
    /// one cell of the brick cache, which is a line nobody can see. At 0.7 it
    /// is as deep and nearly as wide, which is a gouge with a crease's name.
    /// At 0.6 it is the full depth in three fifths of the width, which is what
    /// DamStandard is for.
    const CREASE_REACH: f32 = 0.6;
    const CREASE_LIFT: f32 = 1.0;
    const CREASE_SPACING: f32 = 0.35;

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

    /// One segment of a live Move drag.
    ///
    /// Where [`Self::move_surface_stroke`] writes a grab per segment — and so
    /// multiplies the layer's Lipschitz bound by one factor per segment — this
    /// advances one transaction and redraws its preview. The document carries
    /// no part of the drag until the pointer comes up; what the sculptor sees
    /// is the brick cache, filled from a preview written and immediately taken
    /// back.
    fn live_move_drag(
        &mut self,
        brush: BrushSettings,
        samples: &[GestureSample],
    ) -> Result<EditOutcome, ModelError> {
        let brush = brush.sanitized();
        let Some(last) = samples.last() else {
            return Ok(EditOutcome::NOTHING);
        };

        // The first segment is what carries the anchor, so it is what opens
        // the transaction. `samples[0]` is the press: a drag is sent from the
        // point the pointer went down, which is what `is_path_driven` means.
        if self.live_move.is_none() {
            self.live_move_armed = false;
            let layer = self.active_layer().id;
            let anchor = samples[0].position;
            let live = crate::live::LiveMove::begin(
                &mut self.document,
                layer,
                anchor,
                claycore::MoveParams {
                    radius: brush.size.max(1e-3),
                    ease: 0,
                    front_only: true,
                },
            )?;
            self.live_move = Some(live);
        }

        let Some(live) = self.live_move.as_mut() else {
            return Ok(EditOutcome::NOTHING);
        };
        // A drag under the resolution moves nothing, and redrawing a preview
        // of nothing spends two document edits per pointer event to no effect.
        let anchor = live.anchor();
        let travelled = (0..3)
            .map(|axis| (last.position[axis] - anchor[axis]).powi(2))
            .sum::<f32>()
            .sqrt();
        if travelled < 1e-4 {
            return Ok(EditOutcome::NOTHING);
        }

        // Draw, sample, take back — in that order and inside one segment, so
        // the segment leaves the engine's undo depth exactly where it found it.
        // See `LiveMove::settle` for what counting it otherwise would cost.
        let region = live.drag(&mut self.document, last.position)?;
        let filled = self.refill_preview(region);
        let settled = self
            .live_move
            .as_mut()
            .map(|live| live.settle(&mut self.document))
            .unwrap_or(Ok(()));
        // The preview comes off the layer even when the fill failed: leaving it
        // there would fail the commit as well, turning one bad segment into a
        // lost gesture.
        filled.and(settled)?;
        Ok(EditOutcome {
            changed: true,
            dirty_bricks: self.dirty.len(),
        })
    }

    /// Re-fills the region a live Move preview reached.
    ///
    /// Nothing but this marks those bricks, which is what leaves the drag on
    /// screen after the preview has been taken back off the document: a brick
    /// keeps what it was last given until something asks for it again.
    fn refill_preview(&mut self, region: Option<([f32; 3], [f32; 3])>) -> Result<(), ModelError> {
        match region {
            Some((min, max)) => self.refill_region(min, max),
            // The engine reports no bounds when the drag reached nothing.
            None => Ok(()),
        }
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
        let live: Vec<&GestureSample> = match self.active_mask() {
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
        // Both borrows of the document are shared, which is what lets the
        // layer's own mask be read while the layer is sampled.
        let mask = self.active_mask();
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
                    mask: mask.as_deref(),
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

    // -- the live half of the region tools ---------------------------------

    /// Whether a region gesture would be shown while it is being made.
    ///
    /// One condition now: the layer has to be a field the sculptor may edit,
    /// because the transaction refuses a protected one.
    ///
    /// It used to be two. The brick cache holds the hard union of every
    /// visible SDF layer and the engine attributes no brick to the layer it
    /// came from, while a transaction previews one layer alone — so with a
    /// second field subtool in the document the preview was the layer under
    /// the brush and nothing else, and the rest of the scene would have
    /// vanished for the length of the drag. The gesture fell back to being
    /// held whole and applied on release: correct, just not live. It was filed
    /// upstream as ClayCore#378 and ClayCore 0.78.0 answers it — the document
    /// can now be evaluated over every visible SDF layer *except* one, which
    /// is the other half of what the preview holds, and `crate::live` composes
    /// the two. See [`Self::the_rest_beside_the_preview`].
    fn live_smooth_is_possible(&self) -> bool {
        let active = self.active_layer();
        active.representation == Representation::Sdf && active.protection.is_editable()
    }

    /// What the preview has to be drawn beside, or `None` where it is the
    /// whole scene on its own.
    ///
    /// `None` is the ordinary document and the one every brush figure is
    /// measured on, and it takes exactly the path it took before the gesture
    /// could compose anything: there is no second subtool to lose, so nothing
    /// is evaluated and nothing is composed.
    ///
    /// The boxes are the *other* subtools' bounds rather than the whole
    /// scene's, so a preview lattice is widened over what is actually there
    /// instead of over the empty space between two forms.
    fn the_rest_beside_the_preview(&self) -> Option<crate::live::Rest> {
        let active = self.active_layer().key;
        let others: Vec<&Layer> = self
            .layers
            .iter()
            .filter(|layer| {
                layer.key != active && layer.visible && layer.representation == Representation::Sdf
            })
            .collect();
        if others.is_empty() {
            return None;
        }
        let bounds = others
            .iter()
            .filter_map(|layer| self.document.layer_bounds(layer.id).ok().flatten())
            .collect();
        // Routed for the pointer-down pass, which is the whole form and the
        // batch that dominates; a dab's composition is a couple of dozen
        // bricks either way.
        let backend = self
            .policy
            .refill_backend(self.surface_brick_count.max(1))
            .cloned();
        Some(crate::live::Rest::new(
            self.active_layer().id,
            bounds,
            backend,
        ))
    }

    /// Whether a Move drag can be previewed on the active layer.
    ///
    /// Unlike [`Self::live_smooth_is_possible`] this does not care how many
    /// field subtools are visible. A relax preview is drawn from a lattice of
    /// the transaction's own, which holds one layer and cannot compose the
    /// rest of the document; a Move preview is drawn from the document's *own*
    /// brick cache, which already holds the union of every visible SDF layer.
    /// The drag is written into the document to be sampled and taken back
    /// again — see [`crate::live::LiveMove`] — so what the cache reads is the
    /// whole scene with the drag in it.
    fn live_move_is_possible(&self) -> bool {
        let active = self.active_layer();
        active.representation == Representation::Sdf && active.protection.is_editable()
    }

    /// Answers the refusals a Move drag can be refused for without a position,
    /// and points the mirror while nothing is holding the layer.
    fn arm_live_move(&mut self, symmetry: [bool; 3]) -> bool {
        if self.live_move.is_some() || self.live_move_armed || !self.live_move_is_possible() {
            return false;
        }
        // Before the transaction opens, never during it: a commit refuses a
        // layer that changed since begin, and the mirror is such a change.
        let before = self.engine_undo_depth();
        if self.point_the_mirror(symmetry).is_err() {
            return false;
        }
        self.live_move_armed = true;
        self.live_opening_entries = self.engine_undo_depth().saturating_sub(before);
        true
    }

    /// Opens a live gesture for a tool that would otherwise be held whole.
    ///
    /// Reports whether it is open, which is what tells the ViewModel to send
    /// segments as they are made instead of holding the whole stroke.
    pub fn open_live_gesture(&mut self, tool: ToolKind, symmetry: [bool; 3]) -> bool {
        // Move is the other verb the transaction was built for, and it opens
        // by a different door: a drag is anchored where the pointer went down
        // and `open_live_gesture` is not told where that is, so the transaction
        // begins on the gesture's first segment. What happens here is the half
        // that can happen without a position — the refusals, and pointing the
        // mirror before anything is holding the layer.
        if tool == ToolKind::Mover {
            return self.arm_live_move(symmetry);
        }
        // Two of the four region tools, because the transaction is a *relax*:
        // Planar and Polir flatten, which is a different verb with no live
        // form in this release, and they stay held.
        if !matches!(tool, ToolKind::Suavizar | ToolKind::Relaxar) {
            return false;
        }
        if self.live_smooth.is_some() || !self.live_smooth_is_possible() {
            return false;
        }
        // Before the transaction opens, never during it. `baked_stroke` points
        // the mirror on every segment, and the first segment of a gesture that
        // changed it would be an edit to the layer the transaction is holding
        // — which the commit then refuses, correctly, as a preview computed
        // against a document that has since moved.
        let before = self.engine_undo_depth();
        if self.point_the_mirror(symmetry).is_err() {
            return false;
        }
        // Pointing it is an edit of its own, and one this gesture caused. It is
        // counted here so that closing or abandoning the gesture spends it —
        // the held path counts it inside its first segment, and a symmetry
        // change that outlived the stroke that asked for it would be a
        // difference between the two paths a sculptor could feel.
        let opening = self.engine_undo_depth().saturating_sub(before);
        let id = self.active_layer().id;
        let rest = self.the_rest_beside_the_preview();
        match crate::live::LiveSmooth::begin(&mut self.document, id, Self::BRICK_CONFIG, rest) {
            Ok(live) => {
                self.live_smooth = Some(live);
                self.live_opening_entries = opening;
                self.surface_epoch = self.surface_epoch.wrapping_add(1);
                true
            }
            // A refusal is not an error the sculptor should see: the gesture
            // simply goes down the path it took before there was a live one.
            Err(_) => false,
        }
    }

    /// Ends the live gesture, installing what it previewed.
    ///
    /// Returns how many history entries the commit recorded, so a gesture that
    /// is abandoned afterwards knows how much to take back.
    pub fn close_live_gesture(&mut self) -> Result<usize, ModelError> {
        if self.live_move.is_some() || self.live_move_armed {
            return self.close_live_move();
        }
        let Some(live) = self.live_smooth.take() else {
            return Ok(0);
        };
        self.surface_epoch = self.surface_epoch.wrapping_add(1);
        let opening = std::mem::take(&mut self.live_opening_entries);
        let gesture = self.live_gesture.take();
        // Dropped rather than committed, and that is the decision this method
        // exists to record.
        //
        // `clay_sdf_smooth_commit` installs the working volume as the layer's
        // ONE item — it consolidates the whole subtool, every stroke. On this
        // machine that measures slightly *better* than the bake it replaces
        // (roughness 5.74 against 5.83 on the reference roughened surface),
        // and on the Metal runner it measures 7.82 against a ceiling of 6.00,
        // moving 2458 pixels where the same stroke moves 205 here: the whole
        // surface shifts. Planar and Polir, which are baked the old way, are
        // identical on both platforms, so it is the consolidation and not the
        // measurement. Filed upstream as ClayCore#379.
        //
        // Even where it measures well it is a heavy thing to do on every
        // stroke: it discards the layer's edit list and re-samples the whole
        // subtool at the cache's cell size, so repeated smoothing compounds
        // the resampling. So the preview is what the transaction is used for,
        // and the stroke is laid down by the path that was always used.
        //
        // The cost is that the preview and the result are not the same
        // arithmetic: the preview relaxes cumulatively per dab, the bake makes
        // one pass over the whole gesture. Measured on the same surface they
        // land within 0.09 of each other in roughness, which is the difference
        // between 5.74 and 5.83 — visible in numbers, not on the clay.
        drop(live);
        let Some((tool, brush, symmetry, samples)) = gesture else {
            return Ok(opening);
        };
        if samples.is_empty() {
            return Ok(opening);
        }
        let before = self.engine_undo_depth();
        self.baked_stroke(tool, brush, &samples, symmetry)?;
        let recorded = opening + self.engine_undo_depth().saturating_sub(before);
        self.refresh_stats();
        Ok(recorded)
    }

    /// Abandons the live gesture. The document was never touched, so there is
    /// nothing to take back — only the preview to stop drawing.
    /// Returns the entries the *opening* recorded, which an abandoned gesture
    /// still has to take back: the preview wrote nothing, but pointing the
    /// layer's mirror did.
    pub fn discard_live_gesture(&mut self) -> usize {
        if self.live_move.is_some() || self.live_move_armed {
            return self.discard_live_move();
        }
        if self.live_smooth.take().is_none() {
            return 0;
        }
        self.surface_epoch = self.surface_epoch.wrapping_add(1);
        self.live_gesture = None;
        std::mem::take(&mut self.live_opening_entries)
    }

    /// Installs the drag as one grab per item, and reports what it recorded.
    fn close_live_move(&mut self) -> Result<usize, ModelError> {
        self.live_move_armed = false;
        let opening = std::mem::take(&mut self.live_opening_entries);
        let Some(live) = self.live_move.take() else {
            // Armed and never dragged: the press opened nothing, so only the
            // mirror it pointed is owed back.
            return Ok(opening);
        };
        self.surface_epoch = self.surface_epoch.wrapping_add(1);
        let (recorded, region) = live.commit(&mut self.document)?;
        // After the commit, so the bricks are filled from the document that
        // now carries the drag rather than from the one that briefly did.
        self.refill_preview(region)?;
        self.refresh_stats();
        Ok(opening + recorded)
    }

    /// Abandons the drag. The document never carried it, so only the preview
    /// has to be taken off the screen.
    fn discard_live_move(&mut self) -> usize {
        self.live_move_armed = false;
        let opening = std::mem::take(&mut self.live_opening_entries);
        let Some(live) = self.live_move.take() else {
            return opening;
        };
        self.surface_epoch = self.surface_epoch.wrapping_add(1);
        // Both halves are best-effort: this is the path an error already took,
        // and failing to clean up a preview must not replace the first error
        // with a second one.
        if let Ok(region) = live.cancel(&mut self.document) {
            let _ = self.refill_preview(region);
        }
        opening
    }

    pub fn live_gesture_is_open(&self) -> bool {
        self.live_smooth.is_some() || self.live_move.is_some()
    }

    /// Whether a gesture is open and being previewed rather than banked.
    ///
    /// Public for the same reason `live_gesture_is_open` is: the hooks that
    /// set it are *provided* trait methods, so a model that forgets to forward
    /// them compiles and silently answers for a document that cannot preview
    /// anything. It is only observable from outside if something can ask.
    pub fn is_previewing(&self) -> bool {
        self.previewing
    }

    /// The surface the viewport should mesh, while a live gesture is drawing
    /// one.
    pub fn live_surface(&self) -> Option<crate::LiveSurface<'_>> {
        self.live_smooth.as_ref().and_then(|live| live.surface())
    }

    /// Which surface the viewport's stored geometry belongs to.
    pub fn surface_epoch(&self) -> u64 {
        self.surface_epoch
    }

    /// One live dab of the smoothing brush, relaxing the retained volume.
    ///
    /// The region and the falloff are `relax_stroke`'s, so the live gesture
    /// and the one it replaces smooth the same clay by the same amount — see
    /// the measurements there for why one pass at the brush's own radius is
    /// what this asks for.
    fn live_relax_dab(
        &mut self,
        brush: BrushSettings,
        samples: &[GestureSample],
    ) -> Result<EditOutcome, ModelError> {
        let brush = brush.sanitized();
        let Some(last) = samples.last() else {
            return Ok(EditOutcome::NOTHING);
        };
        // Split by field: the lease reads the *document* and the transaction is
        // a sibling field, which the borrow checker can see only when both are
        // named. `active_mask` would take the whole of `self`.
        let layer = self.active_layer().id;
        let Self {
            document,
            live_smooth,
            ..
        } = self;
        let mask = document.layer_mask(layer);
        let mask = mask.as_deref();
        let Some(live) = live_smooth.as_mut() else {
            return Ok(EditOutcome::NOTHING);
        };
        let dirty_bricks = live.dab(
            document,
            claycore::RelaxParams {
                strength: brush.intensity,
                radius_cells: 1,
                iterations: 2,
                centre: last.position,
                region_radius: brush.size,
                falloff: brush.size * 0.5,
                mask,
            },
        )?;
        Ok(EditOutcome {
            changed: true,
            dirty_bricks,
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
        // In the layer's own frame, as a sculpting stroke is, because that is
        // the frame every consumer of this mask actually reads it in.
        //
        // The ABI calls the lattice world-addressed — `clay_mask_sample` takes
        // "a world position" — and `clay_mesh_sculptor_apply_stroke` even
        // takes a `mesh_to_world` for mapping local vertices onto it. But the
        // field stroke composes no layer transform when it gates a stamp:
        // measured, a stamp at layer-local zero on a layer standing at x = 3
        // is gated against the cells at zero and not against the ones at
        // three. We pass NULL for that `mesh_to_world`, so the mesh sculptor
        // reads it in the layer's frame too, and the readback below is carried
        // to match. One frame for all four, which is what makes the freeze
        // mean the same thing everywhere; painted in world coordinates it sat
        // beside the form it was meant to protect on any moved subtool, and
        // did nothing at all.
        let placement = self.active_content_placement();
        let carried = Self::carried_samples(&placement, samples);
        let samples = carried.as_deref().unwrap_or(samples);
        let mut brush = brush.sanitized();
        if let Some(transform) = &placement {
            brush.size /= transform.largest_scale();
        }
        let preset = self.preset(brush, ToolKind::Mascara);
        let stroke: Vec<claycore::StrokeSample> = samples
            .iter()
            .map(|s| claycore::StrokeSample {
                position: s.position,
                pressure: s.pressure,
                time: s.time,
            })
            .collect();

        // Attached to the *layer*, inside the document, so that saving the
        // document saves the mask. The cell size is the cache's own spacing,
        // not a fraction of the brush.
        //
        // A quarter of the brush was tried: at the default brush that is a 0.1
        // cell, coarser than anything the surface can express, and
        // `clay_document_mask_extrude` refuses a wall thinner than a cell — so
        // a mask painted with a large brush could not be extruded at any
        // sensible thickness. Matching the voxel size makes a mask as fine as
        // the thing it freezes.
        let layer = self.active_layer().id;
        let painted = self
            .document
            .ensure_layer_mask(layer, Self::VOXEL_SIZE)
            .map_err(ModelError::engine)?
            .apply_stroke(
                &stroke,
                &preset,
                brush.intensity,
                BrushShape::Sphere,
                Falloff::Smooth,
            )
            .map_err(ModelError::engine)?;

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

    /// Stamps the mask along each path of a drawn region.
    ///
    /// Returns how many stamps landed. One `clay_mask_apply_stroke` per path,
    /// because that call is the only one that writes many cells for one entry
    /// in the undo history — see [`clayspace_model::outline`].
    fn freeze_along(
        &mut self,
        paths: Vec<Vec<[f32; 3]>>,
        target: f32,
        spacing: f32,
    ) -> Result<usize, ModelError> {
        let layer = self.active_layer().id;
        let preset = outline_preset(spacing);
        let mut painted = 0usize;
        for path in paths {
            let samples: Vec<claycore::StrokeSample> = path
                .into_iter()
                .map(|position| claycore::StrokeSample {
                    position,
                    pressure: 1.0,
                    time: 0.0,
                })
                .collect();
            painted += self
                .document
                .ensure_layer_mask(layer, Self::VOXEL_SIZE)
                .map_err(ModelError::engine)?
                .apply_stroke(
                    &samples,
                    &preset,
                    target,
                    // A ball, and the radius is what makes it cover: a cube of
                    // the same reach writes twice the cells for the same
                    // region, all of them in corners that overshoot it.
                    BrushShape::Sphere,
                    // And hard-edged: the region's edge is where the sculptor
                    // drew it, not a gradient away from it.
                    Falloff::Constant,
                )
                .map_err(ModelError::engine)?;
        }
        Ok(painted)
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
        // As in `relax_stroke`: two shared borrows of the document, so the
        // layer's own mask can be read while the layer is sampled.
        let mask = self.active_mask();
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
                    mask: mask.as_deref(),
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

    /// Move Topológico: a drag whose reach is measured along the material.
    ///
    /// Beside `flatten_stroke` and `relax_stroke` rather than beside
    /// `move_surface_stroke`, and that placement is the whole design. The
    /// Euclidean drag emits a warp per item and touches no samples; this one
    /// **bakes** — the engine re-samples the volume with the move applied —
    /// which is what lets it weigh a point by how far it is *through the clay*
    /// rather than through the air. Two parts of a form close in space and far
    /// along the surface therefore move independently, which is the whole
    /// reason the verb exists and what a Euclidean drag at the same radius
    /// cannot do.
    ///
    /// It costs a bake, so it is the tool to reach for when the cheap drag
    /// pulls something it should not — which is the engine's own advice.
    fn topological_move_stroke(
        &mut self,
        brush: BrushSettings,
        samples: &[GestureSample],
    ) -> Result<EditOutcome, ModelError> {
        let brush = brush.sanitized();
        let (first, last) = (samples[0], samples[samples.len() - 1]);
        let displacement: [f32; 3] =
            std::array::from_fn(|axis| last.position[axis] - first.position[axis]);
        let travelled = displacement.iter().map(|d| d * d).sum::<f32>().sqrt();
        // A drag under the resolution moves nothing, and reporting it as an
        // edit would bake the whole region to record a gesture that did not
        // land. The same floor `move_surface_stroke` uses.
        if travelled < 1e-4 {
            return Ok(EditOutcome::NOTHING);
        }

        let layer = self.active_layer().id;
        let anchor = first.position;
        // The ball the reach could walk within, from the anchor and from where
        // the drag takes it. A shorter box would place the moved material
        // against the volume's bound rather than against the surface, and a
        // longer one costs accuracy elsewhere: everything inside the box is
        // re-approximated at the bake's cell size, so measured on the starting
        // form, padding the box by the drag's own length as well moved the
        // surface on the *far side* of the sphere by 0.0015 where the box that
        // covers exactly the drag's reach moves it by 0.0003.
        let reach = brush.size.max(1e-3);
        let mut min = [0.0f32; 3];
        let mut max = [0.0f32; 3];
        for axis in 0..3 {
            let a = anchor[axis];
            let b = a + displacement[axis];
            min[axis] = a.min(b) - reach;
            max[axis] = a.max(b) + reach;
        }
        let cell = Self::bake_cell_size(brush.size);
        Self::grown_for_feather(&mut min, &mut max, cell);

        // Baked first and moved second, because there is no
        // `clay_item_volume_move_topological_from`: the verb takes an item
        // carrying a volume. The band has to cover the drag, which is what the
        // box above is sized for.
        let mut volume = self
            .document
            .volume_from_region(Self::bake_volume(cell), min, max)
            .map_err(ModelError::engine)?;
        volume
            .move_topological(&claycore::TopologicalMoveParams {
                anchor,
                radius: reach,
                // Scaled by Intensidade, as every other brush is: the engine
                // takes the displacement whole and has no strength of its own
                // here, so this is where the slider has to act.
                displacement: displacement.map(|axis| axis * brush.intensity),
                ease: 0,
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

        // The sculptor holds this layer's vertices as the engine does, and the
        // layer transform moves only where they are *drawn* — see
        // `carried_placement`. So a gesture aimed at the form on screen is
        // carried back into those coordinates before anything at all is derived
        // from it, and the brush with it: a subtool scaled to half its size
        // wants half the radius against the vertices it actually has.
        let placement = self.carried_placement(key);
        let carried = Self::carried_samples(&placement, samples);
        let samples = carried.as_deref().unwrap_or(samples);

        let mut brush = brush.sanitized();
        if let Some(transform) = &placement {
            brush.size /= transform.largest_scale();
        }
        // Read before the sculptor is borrowed mutably. A mesh takes an alpha
        // by a third route — the brush descriptor's own block — and it is not
        // gated on a combine operation, which is the SDF side's vocabulary.
        let alpha = self.alpha_for(brush, Combine::Relief).cloned();
        let alpha = alpha.as_ref();
        // Read for the same reason, one line later: the sculptor borrows the
        // document and this does not.
        let chosen = self.colour.current().sanitized();
        let CarriedStroke {
            preset,
            stamp,
            points,
            gesture,
        } = carried_stroke(
            verb,
            brush,
            samples,
            self.preset(brush, tool),
            alpha,
            chosen,
        );

        // Recorded per gesture, because that is the unit a sculptor thinks in
        // and the unit `mesh-sculpting` specifies: one gesture, one undo.
        //
        // How the last segment is dealt with depends on how the next one
        // arrives, and the two have to agree. A **dragging** verb is laid down
        // again from its anchor on every segment — `replays_from_the_anchor`
        // in the ViewModel — so what the last one did is taken back first, or
        // the preview stacks segment on segment. A **stamping** verb is sent
        // only the samples the model has not seen, so there is nothing to take
        // back: reverting anyway erases the stroke as fast as it is drawn and
        // leaves a drag with only its final dab, which reads as a brush that
        // needs clicking rather than dragging.
        //
        // Continuing the record rather than starting one is what keeps a
        // stamping drag to a single undo: `MeshDeltas` coalesces, so a stroke
        // passing over the same vertex forty times still records where it
        // started, once.
        //
        // A gesture that was open on another subtool is dropped here rather
        // than carried, and dropping one settles it — see [`LiveMesh`].
        let Some(sculptor) = self.sculptor_for(key) else {
            return Ok(EditOutcome::NOTHING);
        };
        let held = self.live_mesh.take().filter(|live| live.layer == key);
        let (mut live, mut previous) = match held {
            Some(mut held) if tool.is_path_driven() => {
                // Settled before it is handed over to be reverted, and this is
                // the ordering the exactness rests on: the flush recomputes
                // the segment's normals *into the record that is about to put
                // them back*, so what the revert restores is what a gesture
                // that deferred nothing would have left. Deferring across the
                // boundary would leave the last segment's flush recomputing
                // classes the earlier ones only ever moved and took back,
                // which is a mesh shaded from geometry no longer there.
                held.settle()?;
                (
                    LiveMesh::new(
                        key,
                        sculptor.clone(),
                        claycore::MeshDeltas::new().map_err(ModelError::engine)?,
                    ),
                    Some(held),
                )
            }
            Some(held) => (held, None),
            None => (
                LiveMesh::new(
                    key,
                    sculptor.clone(),
                    claycore::MeshDeltas::new().map_err(ModelError::engine)?,
                ),
                None,
            ),
        };
        // Read before the sculptor is borrowed: the lease reads the document
        // and both are shared borrows of `self`, so the two sit side by side.
        let mask = self.active_mask();
        // What the pick already worked out, if it is still worth anything to
        // this call. Asked once for the two shapes a mesh stroke takes: Grab
        // makes one stamp at the descriptor's own radius, and everything else
        // resolves a path whose stamps take theirs from the preset. Either
        // answer may be `None`, which is the scan every stamp here did before
        // — slower, and never wrong.
        let picked = self.picked_seed.get();
        let stamp_seed = picked.and_then(|it| it.for_stamp(key, stamp.center, stamp.radius));
        let stroke_seed = picked.and_then(|it| it.for_stroke(key, &points, &preset));
        let moved = {
            let mut sculptor = sculptor.borrow_mut();
            if let Some(previous) = &mut previous {
                previous
                    .deltas()
                    .revert(&mut sculptor)
                    .map_err(ModelError::engine)?;
            }
            // Normals held back for the length of this segment's engine calls.
            //
            // What it buys is the recompute of overlapping dabs done once
            // instead of once per dab; what it costs is that the form shades
            // from where its vertices were until the flush below. The flag is
            // put back down by that flush and by nothing else, so no call
            // outside this block — a whole-form deformer, a cage, an undo's
            // revert — can find it standing.
            sculptor
                .set_defer_normals(true)
                .map_err(ModelError::engine)?;
            let deltas = live.deltas();
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
                                seed: mirror.is_identity().then_some(stamp_seed).flatten(),
                                ..stamp
                            },
                            mask.as_deref(),
                            Some(deltas),
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
                                seed: mirror.is_identity().then_some(stroke_seed).flatten(),
                                ..stamp
                            },
                            mask.as_deref(),
                            // The stroke resolver's own deferral, which is a
                            // different thing from the flag above and is why
                            // both are set: it is scoped to this call, and the
                            // library recomputes once at the end of the stroke
                            // it drove — into this same record — because there
                            // it knows where the stroke ended. That is what
                            // collapses a resolved stroke's overlapping dabs
                            // into one recompute; the flag above is what does
                            // the same for Grab's mirrored stamps, which no
                            // resolver drives.
                            true,
                            Some(deltas),
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

        // The flush the deferral above owes, into the record its stamps were
        // noted into. `LiveMesh::settle` is the only thing in this crate that
        // recomputes deferred normals, and `LiveMesh::drop` calls it too, so a
        // `?` anywhere above leaves by this same door.
        live.settle()?;

        // A refit keeps the tree valid and says nothing about whether it is
        // still a good partition. Whether it has stopped being one is read
        // once between strokes rather than here, where a drag would pay for
        // the reading on every pointer move.
        self.request_index_rebuild(key);

        // A gesture that reached nothing is not worth a place on the stack,
        // and putting one there would make an undo appear to do nothing.
        let reached = live.deltas().vertex_count().map_err(ModelError::engine)? > 0;
        if self.previewing {
            // Held rather than banked. The gesture is still open, and every
            // segment replaces the last — one drag is one undo however many
            // segments drew it.
            if reached {
                self.live_mesh = Some(live);
            }
            self.live_generation = self.live_generation.wrapping_add(1);
        } else if reached {
            let engine_depth = self.engine_undo_depth();
            let (layer, deltas) = live.finish();
            self.mesh_undo.push(MeshGesture {
                layer,
                what: GestureRecord::Deltas(deltas),
                engine_depth,
            });
            // A new edit ends the redo line, exactly as the engine's own does.
            self.mesh_redo.clear();
        }
        // The vertices moved, so the box `layer_bounds` answers from is stale.
        self.refresh_mesh_bounds(key);
        Ok(EditOutcome {
            changed: moved > 0,
            // A mesh layer is not in the brick cache at all, so nothing was
            // dirtied and nothing needs re-meshing — the viewport reads the
            // layer's own triangles.
            dirty_bricks: 0,
        })
    }

    /// A stroke on a hierarchy, at the level the brush is bound to.
    ///
    /// The verbs are the mesh's verbs and the descriptor is the mesh's
    /// descriptor — see [`carried_stroke`] — so what is different here is
    /// entirely about the two things a hierarchy has that a mesh layer does
    /// not: a level the stamp binds to, and no undo record.
    ///
    /// **No seed, and the reason is measured rather than assumed.** A mesh
    /// stroke hands the engine the weld class its pick landed on so the
    /// surface walk starts at the finger instead of searching the mesh, and
    /// the class travels with a token naming the numbering it came from. A
    /// hierarchy renumbers that space on **every bind** — measured on the
    /// pinned engine, the token reads 1, then 3 after one dab, then 4, 5, 6, 7
    /// as caches are dropped, trimmed and levels rebound — and this
    /// application binds a fresh sculptor per segment, because the wrapper
    /// borrows the surface for the sculptor's whole life and a gesture spans
    /// frames. So there is never a numbering that outlives the pick, and a
    /// seed carried across one would be *in bounds*, wrong and silent: the
    /// walk starts nowhere near the brush, `geodesic_region` comes back empty,
    /// and the dab is lost rather than misplaced. `seed: None` is the correct
    /// answer here and not a stub.
    ///
    /// **What is held and compared instead** is what a host actually caches
    /// across a rebuild: the triangles the viewport draws and the number that
    /// says they are stale. See [`crate::multires::Hierarchy::watched`], which
    /// is the engine's evaluated counter and this side's own generation
    /// together, because the engine's alone restarts at one every time a
    /// hierarchy is put back from bytes.
    ///
    /// **The record.** The ABI carries no delta for a hierarchy gesture, so the
    /// hierarchy's own serialized bytes are taken once, on the first segment
    /// that reaches the surface. They are two things at once: what a dragging
    /// verb is laid down again from, and what the gesture enters the undo
    /// history as. See [`GestureRecord::Hierarchy`].
    fn stroke_multires(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        let Some(verb) = mesh_verb(tool) else {
            return Ok(EditOutcome::NOTHING);
        };
        let index = self.active;
        let key = self.layers[index].key;

        // The gesture carried into the hierarchy's own coordinates, and the
        // brush with it — the same conversion a carried mesh makes, for the
        // same reason: the layer transform moves where the form is drawn and
        // the hierarchy holds its vertices where they were built.
        let placement = self.carried_placement(key);
        let carried = Self::carried_samples(&placement, samples);
        let samples = carried.as_deref().unwrap_or(samples);
        let mut brush = brush.sanitized();
        if let Some(transform) = &placement {
            brush.size /= transform.largest_scale();
        }
        // Both read before anything is borrowed mutably.
        let alpha = self.alpha_for(brush, Combine::Relief).cloned();
        let chosen = self.colour.current().sanitized();
        let preset = self.preset(brush, tool);

        // Taken out of the layer for the length of the stroke, because the
        // freeze is read off `self` and the hierarchy is inside it — and a
        // shared borrow of the document and an exclusive borrow of one of its
        // layers cannot stand together. Put back on every path below,
        // including the failing ones, which is why the work is a block whose
        // result is unwrapped afterwards rather than a run of `?`.
        let Some(mut hierarchy) = self.layers[index].multires.take() else {
            return Ok(EditOutcome::NOTHING);
        };
        let stroked = self.stroke_into(
            &mut hierarchy,
            verb,
            tool,
            brush,
            samples,
            symmetry,
            preset,
            alpha.as_ref(),
            chosen,
        );
        self.layers[index].multires = Some(hierarchy);
        let moved = stroked?;

        // What is drawn moved, so the box every manipulator sizes itself to
        // has moved with it.
        self.refresh_multires_bounds(key);
        if self.previewing {
            // Held rather than banked, exactly as a mesh gesture is: the
            // gesture is still open and every segment replaces the last, so
            // one drag is one undo however many segments drew it.
            self.live_generation = self.live_generation.wrapping_add(1);
        } else {
            self.bank_multires_gesture(key);
        }
        Ok(EditOutcome {
            // A hierarchy is not in the brick cache, so nothing was dirtied
            // and nothing needs re-meshing — the viewport reads the display
            // level's own triangles.
            changed: moved > 0,
            dirty_bricks: 0,
        })
    }

    /// The engine half of [`ClayDocument::stroke_multires`], with the
    /// hierarchy held apart from the document.
    ///
    /// Split out so the layer gets its hierarchy back whatever happens here,
    /// and so this function is one thing: take the last segment back, record
    /// where the gesture started, and stamp.
    #[allow(clippy::too_many_arguments)]
    fn stroke_into(
        &self,
        hierarchy: &mut crate::multires::Hierarchy,
        verb: claycore::MeshBrush,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
        preset: StrokePreset,
        alpha: Option<&Alpha>,
        chosen: clayspace_model::Colour,
    ) -> Result<u64, ModelError> {
        // A dragging verb is laid down again from its anchor on every segment,
        // so what the last one did is taken back first — or the preview stacks
        // segment on segment. The take-back is the recorded bytes, which is the
        // only exact one the ABI offers: measured on the pinned engine, 8.15 ms
        // to put back a level-4 hierarchy over a 16x16 cage.
        if tool.is_path_driven() && hierarchy.gesture_is_open() {
            hierarchy.replay_from_the_anchor()?;
        }
        hierarchy.open_gesture()?;

        let CarriedStroke {
            preset,
            stamp,
            points,
            gesture,
        } = carried_stroke(verb, brush, samples, preset, alpha, chosen);
        // Read before the sculptor is taken: the lease reads the document.
        let mask = self.active_mask();

        // Where it lands: the form under the passes, or the pass the sculptor
        // has selected. The two are different entry points and not a flag —
        // see [`ClayDocument::stamp_into_a_pass`].
        if hierarchy.stamps_into_a_pass() {
            let moved = Self::stamp_into_a_pass(
                hierarchy,
                verb,
                &stamp,
                &points,
                gesture,
                symmetry,
                mask.as_deref(),
            )?;
            hierarchy.note_gesture_moved(moved);
            return Ok(moved);
        }

        let mut sculptor = hierarchy
            .surface_mut()
            .sculptor()
            .map_err(ModelError::engine)?;

        // Clears the record `MeshBrush::Layer` measures its ceiling against,
        // so a second stroke over the same place deposits from the surface as
        // that stroke found it rather than from where the last one stopped.
        sculptor.begin_stroke().map_err(ModelError::engine)?;
        let mut moved = 0;
        for mirror in mirrors(symmetry) {
            let moved_here = if verb == claycore::MeshBrush::Grab {
                // One stamp at the point the gesture took hold of, carrying
                // that region by the whole drag. Not a resolved stroke, for
                // the reason `stroke_mesh` spells out: a stroke walks the
                // path and takes the brush centre with it, so a drag that
                // leaves the surface reaches no material at all.
                sculptor
                    .stamp(
                        claycore::MeshStamp {
                            direction: mirror.vector(gesture),
                            center: mirror.point(stamp.center),
                            ..stamp
                        },
                        mask.as_deref(),
                    )
                    .map_err(ModelError::engine)?
                    .moved_vertices
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
                        mask.as_deref(),
                        // The resolver's own deferral: it recomputes once at
                        // the end of the stroke it drove, where it knows the
                        // stroke ended, which is what collapses a resolved
                        // stroke's overlapping dabs into one recompute.
                        true,
                    )
                    .map_err(ModelError::engine)?
                    .1
                    .moved_vertices
            };
            moved += moved_here;
        }
        // No refit and no index rebuild. A hierarchy's level mesh is rebuilt
        // from the authoritative detail whenever it is read, so there is no
        // tree of this side's that a stamp leaves stale — the fixed sculptor
        // inside the level is the engine's own and lives and dies with the
        // bind.
        drop(sculptor);
        // What this segment reached, so the gesture knows whether it is worth
        // an undo step. A stroke that returned early above notes nothing and
        // so banks nothing, which is the right answer for a refused gesture:
        // it is not that the record is missing, it is that there is no edit.
        hierarchy.note_gesture_moved(moved);
        Ok(moved)
    }

    /// The same stroke, into the pass the sculptor has selected.
    ///
    /// A separate function because it is a different entry point and not a
    /// flag: a write into a pass goes through the layered stroke transaction,
    /// which begins, stamps and commits. The transaction is what fixes the
    /// target channel at pointer-down — so changing the active pass mid-drag
    /// cannot split one gesture across two — and what holds the composition
    /// for the length of the gesture, so a stamp is not summing the whole
    /// stack again between dabs.
    ///
    /// **The path is stamped sample by sample rather than resolved**, and that
    /// is the one place a pass stroke differs from a stroke into the form. The
    /// transaction offers stamps and no resolver — clay.h carries
    /// `clay_multires_sculpt_layer_stroke_stamp` and no `_apply_stroke` beside
    /// it — so what a resolved stroke would do with the preset's spacing,
    /// taper and jitter does not happen here. The samples that arrive are
    /// already about one dab's travel apart, because
    /// `SculptViewModel::stamps_between_segments` spaced them before they were
    /// sent, so the coverage is right; what is missing is the jitter and the
    /// taper, and a pass stroke is that much more even than the same stroke
    /// into the form.
    ///
    /// The pressure of each sample reaches the stamp's strength, since nothing
    /// else applies it once the resolver is out of the path.
    ///
    /// A refusal from `begin` is the honest one to surface: it names a locked
    /// pass, which is the sculptor's own doing and the sentence they need.
    #[allow(clippy::too_many_arguments)]
    fn stamp_into_a_pass(
        hierarchy: &mut crate::multires::Hierarchy,
        verb: claycore::MeshBrush,
        stamp: &claycore::MeshStamp<'_>,
        points: &[[f32; 5]],
        gesture: [f32; 3],
        symmetry: [bool; 3],
        mask: Option<&claycore::MaskField>,
    ) -> Result<u64, ModelError> {
        let mut stroke = hierarchy
            .surface_mut()
            .sculpt_layer_stroke()
            .map_err(ModelError::engine)?;
        // Detail rather than Automatic, though the active pass makes the two
        // the same thing here: Detail refuses to open where there is no active
        // pass, and Automatic would quietly write the form instead. The
        // caller has already established there is one, so this is the refusal
        // standing behind that check rather than a second opinion about it.
        stroke
            .set_write_domain(claycore::WriteDomain::Detail)
            .map_err(ModelError::engine)?;
        stroke
            .begin()
            .map_err(|refused| ModelError::engine(refused.to_string()))?;

        let mut moved = 0;
        for mirror in mirrors(symmetry) {
            let stamped = if verb == claycore::MeshBrush::Grab {
                // One stamp carrying its region by the whole drag, exactly as
                // the form's path does and for the same reason.
                stroke
                    .stamp(
                        claycore::MeshStamp {
                            direction: mirror.vector(gesture),
                            center: mirror.point(stamp.center),
                            ..*stamp
                        },
                        mask,
                    )
                    .map_err(ModelError::engine)?
                    .moved_vertices
            } else {
                let mut here = 0;
                for sample in points {
                    let at = mirror.point([sample[0], sample[1], sample[2]]);
                    here += stroke
                        .stamp(
                            claycore::MeshStamp {
                                direction: mirror.vector(stamp.direction),
                                center: at,
                                strength: stamp.strength * sample[3],
                                ..*stamp
                            },
                            mask,
                        )
                        .map_err(ModelError::engine)?
                        .moved_vertices;
                }
                here
            };
            moved += stamped;
        }
        // Committing rather than letting the transaction fall out of scope:
        // `Drop` cancels, which is right for an unwind and wrong for a stroke
        // that finished. The entry count it answers with is the record's, and
        // that record does not cross the ABI — this application's undo holds
        // the hierarchy's serialized bytes instead, taken before the gesture
        // opened.
        stroke.commit().map_err(ModelError::engine)?;
        Ok(moved)
    }

    /// Banks the open hierarchy gesture on the active layer, if there is one.
    ///
    /// Into the same stack a mesh gesture goes into, ordered against the
    /// engine's history by the same depth. See [`MeshGesture`] for why the two
    /// cannot be two stacks.
    fn bank_multires_gesture(&mut self, key: LayerKey) {
        let Ok(index) = self.index_of(key) else {
            return;
        };
        let Some(hierarchy) = self.layers[index].multires.as_mut() else {
            return;
        };
        let Some(bytes) = hierarchy.close_gesture() else {
            return;
        };
        let engine_depth = self.engine_undo_depth();
        self.mesh_undo.push(MeshGesture {
            layer: key,
            what: GestureRecord::Hierarchy(bytes),
            engine_depth,
        });
        // A new edit ends the redo line, exactly as the engine's own does.
        self.mesh_redo.clear();
        self.trim_gesture_history();
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
        // The ray in the grid's own coordinates, and the hit carried back out
        // of them — `clay_voxel_raycast` knows nothing of the layer transform,
        // the same reason the drawing places the cells itself. Without the
        // pair, a moved grid was drawn in one place and picked in another.
        let placement = self.carried_placement(layer.key);
        let (start, along) = match &placement {
            Some(transform) => (
                Self::into_local(transform, origin),
                // Turned *and* divided, which is the same map `into_local`
                // makes on a point without the position. Only the bearing
                // matters — the distance the hit reports is measured along
                // whatever this is — but a bearing is not a rotation's to
                // carry alone once the three factors part company.
                Self::direction_into_local(transform, direction),
            ),
            None => (origin, direction),
        };
        let (_, grid) = self.document.voxel_reader(&layer.engine_name).ok()?;
        let hit = grid.raycast(start, along).ok().flatten()?;
        let length = along.iter().map(|axis| axis * axis).sum::<f32>().sqrt();
        if length <= f32::EPSILON {
            return None;
        }
        let met: [f32; 3] = std::array::from_fn(|i| start[i] + along[i] / length * hit.distance);
        Some(match &placement {
            Some(transform) => Self::into_world(transform, met),
            None => met,
        })
    }
    /// Where a ray meets the active hierarchy's display level.
    ///
    /// Walked rather than traced through a partition, and that is a decision
    /// rather than an omission. The other two representations are picked
    /// through a tree the engine keeps: a grid's own raycast, and a mesh
    /// layer's `MeshSculptor`, whose adjacency and BVH are built once and
    /// survive every stroke because a mesh's topology never changes. Neither
    /// holds here. A hierarchy's level mesh is *regenerated* from the
    /// authoritative detail whenever the surface moves, so a tree over it
    /// would have to be rebuilt after every dab — the weld is the expensive
    /// part, and paying it per dab to save a walk per frame is the wrong way
    /// round.
    ///
    /// So the walk is over the triangles the viewport was already handed,
    /// which cost nothing to have: roughly 24,000 of them at level 3 over a
    /// 16x16 cage. It is linear, and a hierarchy deep enough for that to show
    /// is one where a partition rebuilt per dab would show far more.
    fn pick_active_multires(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]> {
        let key = self.active_layer().key;
        // The ray carried into the hierarchy's own coordinates and the answer
        // carried back out, as every carried representation does it.
        let placement = self.carried_placement(key);
        let (start, along) = match &placement {
            Some(transform) => (
                Self::into_local(transform, origin),
                Self::direction_into_local(transform, direction),
            ),
            None => (origin, direction),
        };
        let index = self.index_of(key).ok()?;
        let (positions, indices) = self.layers[index].multires.as_ref()?.drawn_triangles()?;
        let met = nearest_triangle(start, along, positions, indices)?;
        Some(match &placement {
            Some(transform) => Self::into_world(transform, met),
            None => met,
        })
    }

    /// A gesture written in a moved mesh subtool's own coordinates.
    ///
    /// `None` where the subtool stands at the origin unturned, which is the
    /// common case and the one that copies nothing.
    fn carried_samples(
        placement: &Option<clayspace_model::Transform>,
        samples: &[GestureSample],
    ) -> Option<Vec<GestureSample>> {
        let transform = placement.as_ref()?;
        Some(
            samples
                .iter()
                .map(|sample| GestureSample {
                    position: Self::into_local(transform, sample.position),
                    ..*sample
                })
                .collect(),
        )
    }

    /// Where a ray meets the active mesh layer's triangles.
    ///
    /// Answered by the sculptor's own tree, through the cell that field is
    /// held in — see there for why.
    fn pick_active_mesh(&self, origin: [f32; 3], direction: [f32; 3]) -> Option<[f32; 3]> {
        let key = self.active_layer().key;
        // The sculptor knows the vertices as the engine holds them, so a ray
        // aimed at where the subtool is *drawn* has to be carried back into
        // those coordinates and the answer carried out again. Without it a
        // moved mesh subtool draws in one place and picks in another.
        let placement = self.carried_placement(key);
        let (origin, direction) = match &placement {
            Some(transform) => (
                Self::into_local(transform, origin),
                Self::direction_into_local(transform, direction),
            ),
            None => (origin, direction),
        };
        let Some(sculptor) = self.sculptor_for(key) else {
            // Not built yet, and a pick cannot build it — that costs an
            // adjacency pass and a pick happens every frame the pointer moves.
            // The first stroke builds it; until then the pointer finds nothing
            // on this layer, which reads as the cursor not settling rather
            // than as a wrong answer.
            return None;
        };
        let mut sculptor = sculptor.borrow_mut();
        let hit = sculptor.raycast(origin, direction).ok().flatten()?;
        // What the ray already learned, kept for the stroke that follows it.
        // The class it names and the numbering that class belongs to travel
        // together — see [`crate::seed`] for what carrying one without the
        // other costs — and the position is kept in the mesh's own space,
        // which is the space a stamp's centre arrives in.
        self.picked_seed.set(Some(crate::seed::PickedSeed {
            layer: key,
            at: hit.position,
            seed: hit.seed(),
        }));
        Some(match &placement {
            Some(transform) => Self::into_world(transform, hit.position),
            None => hit.position,
        })
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
    ///
    /// The walk is in layer order and already rebases each layer's indices, so
    /// it is also the only place that can say which run of the buffer belongs
    /// to which layer: the spans come out of the same loop rather than being
    /// reconstructed afterwards from a concatenation that has forgotten its
    /// seams.
    #[allow(clippy::type_complexity)]
    pub fn visible_mesh_geometry(
        &mut self,
    ) -> (
        Vec<[f32; 3]>,
        Vec<[f32; 3]>,
        Vec<[f32; 3]>,
        Vec<u32>,
        Vec<CarriedSpan>,
    ) {
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
        let mut carried = CarriedBuffer::with_capacity(vertices, triangles);

        let drawn: Vec<(usize, Representation, String)> = self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.carries_geometry && layer.visible)
            .filter(|(_, layer)| layer.representation != Representation::Sdf)
            .map(|(index, layer)| (index, layer.representation, layer.engine_name.clone()))
            .collect();

        let mut spans: Vec<CarriedSpan> = Vec::with_capacity(drawn.len());
        for (index, representation, name) in drawn {
            let layer = self.layers[index].key;
            let first = carried.indices.len() as u32;
            match representation {
                Representation::Voxel => self.append_voxel_layer(index, &mut carried),
                // From the display level, never from the cage the layer holds:
                // the sculpt stands off the cage and drawing the cage would
                // draw the form as it was before anybody touched it.
                Representation::Multires => self.append_multires_layer(index, &mut carried),
                _ => self.append_mesh_layer(layer, &name, &mut carried),
            }
            // A layer that contributed nothing gets no span: an empty range is
            // an empty draw call, and a cue that has to skip it is a cue with
            // an exception in it.
            let last = carried.indices.len() as u32;
            if last > first {
                spans.push(CarriedSpan {
                    layer,
                    indices: first..last,
                });
            }
        }

        // What the viewport was handed, so the interface can count what is on
        // screen rather than only what the brick cache built. A mesh or voxel
        // layer draws triangles the surface cache knows nothing about, and the
        // panel used to report a sculpted grid as an empty document.
        self.carried = (carried.indices.len() / 3, carried.positions.len());
        let CarriedBuffer {
            positions,
            normals,
            colors,
            indices,
        } = carried;
        (positions, normals, colors, indices, spans)
    }

    /// Appends one carried mesh layer's triangles, standing where its layer
    /// transform puts them.
    ///
    /// The engine holds a carried mesh's vertices and never moves them: a layer
    /// transform moves what the *tape* evaluates, and a mesh layer contributes
    /// nothing to it. So the whole-subtool manipulator reaches a mesh here or
    /// nowhere — measured, a mesh subtool dragged five units along X drew its
    /// first vertex exactly where it drew it before.
    fn append_mesh_layer(&mut self, layer: LayerKey, name: &str, carried: &mut CarriedBuffer) {
        let Ok((mut positions, mut normals, colors, indices)) = self.document.read_mesh_layer(name)
        else {
            return;
        };
        if let Some(transform) = self.carried_placement(layer) {
            for point in &mut positions {
                *point = Self::into_world(&transform, *point);
            }
            // Through the inverse transpose and not the rotation alone. A
            // layer transform took one factor until ABI 0.74.0 and takes
            // three now, and a normal is the one thing a stretch does not
            // carry the way it carries a point: measured on a meshed starting
            // form at scale [1,4,1], the drawn vertex normals sat a mean 20.9
            // degrees off the triangles they belong to, against 1.5 for the
            // ordinary faceting floor.
            for normal in &mut normals {
                *normal = transform.normal_into_world(*normal);
            }
        }
        carried.append(&positions, &normals, &colors, &indices);
    }

    /// Appends one hierarchy's display level, standing where its layer
    /// transform puts it.
    ///
    /// The level mesh is held on the hierarchy and re-copied only when the
    /// surface has moved under it — `clay_multires_copy_level_mesh` is 3.16 ms
    /// for level 4's 98,817 vertices on the pinned engine, which is a cost a
    /// resting frame must not pay and a frame after a dab must.
    fn append_multires_layer(&mut self, index: usize, carried: &mut CarriedBuffer) {
        let placement = self.carried_placement(self.layers[index].key);
        let Some(hierarchy) = self.layers[index].multires.as_mut() else {
            return;
        };
        let Some((positions, normals, colors, indices)) = hierarchy.level_mesh() else {
            return;
        };
        Self::append_placed(carried, &placement, positions, normals, colors, indices);
    }

    /// Appends one voxel layer's triangles to the carried buffer.
    ///
    /// Two sources for one layer and only ever one of them: the smooth surface
    /// where one has been built, and the per-chunk boxes otherwise.
    fn append_voxel_layer(&self, index: usize, carried: &mut CarriedBuffer) {
        // Standing where its layer transform puts it, which is the host's to
        // do: the engine holds the placement and composes it wherever the
        // *document* answers — `clay_layer_bounds` on a moved grid reports the
        // moved box — but every voxel entry point is in the grid's own
        // coordinates, so a grid is drawn where its cells are unless this
        // places it. Without this the manipulator on a voxel subtool moved the
        // widget and left the form standing, which is exactly what a carried
        // mesh did before `append_mesh_layer` placed one.
        let placement = self.carried_placement(self.layers[index].key);
        // The smooth picture, where one has been built. Whole-grid and so a
        // single splice, unlike the chunked boxes below.
        if let Some((_, smooth)) = self.voxel_smooth.get(&self.layers[index].key) {
            Self::append_placed(
                carried,
                &placement,
                &smooth.positions,
                &smooth.normals,
                &smooth.colors,
                &smooth.indices,
            );
            return;
        }
        // Spliced from what was meshed per chunk. The ranges partition the
        // mesh, so concatenating them is the whole of the join — there is no
        // seam to weld, unlike the brick cache's.
        for chunk in self.layers[index].voxel_chunks.values() {
            Self::append_placed(
                carried,
                &placement,
                &chunk.positions,
                &chunk.normals,
                &chunk.colors,
                &chunk.indices,
            );
        }
    }

    /// Appends geometry, moved by a placement where there is one.
    ///
    /// The copy is what a placement costs, so an unplaced layer — every grid
    /// until one is dragged — appends the engine's own buffers untouched.
    fn append_placed(
        carried: &mut CarriedBuffer,
        placement: &Option<clayspace_model::Transform>,
        positions: &[[f32; 3]],
        normals: &[[f32; 3]],
        colors: &[[f32; 3]],
        indices: &[u32],
    ) {
        let Some(transform) = placement else {
            carried.append(positions, normals, colors, indices);
            return;
        };
        let placed: Vec<[f32; 3]> = positions
            .iter()
            .map(|point| Self::into_world(transform, *point))
            .collect();
        // Through the inverse transpose, for the reason `append_mesh_layer`
        // states: a stretched frame does not carry a normal the way it carries
        // a point, and this is the path a grid and a hierarchy are drawn by.
        let turned: Vec<[f32; 3]> = normals
            .iter()
            .map(|normal| transform.normal_into_world(*normal))
            .collect();
        carried.append(&placed, &turned, colors, indices);
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
        // The points are where the surface is *drawn*; the mask is stored
        // where the layer's own content is. On a moved subtool those are two
        // places, so the question is carried back the way the stroke that
        // painted it was — otherwise the viewport draws the frozen region
        // beside the form it protects.
        let placement = self.active_content_placement();
        let carried: Option<Vec<[f32; 3]>> = placement.as_ref().map(|transform| {
            points
                .iter()
                .map(|p| Self::into_local(transform, *p))
                .collect()
        });
        let points = carried.as_deref().unwrap_or(points);
        let mask = self.active_mask()?;
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
        //
        // And where each stands. A layer transform moves no vertex in the
        // engine and touches no grid — the placement is applied on the way
        // out, in `append_mesh_layer` — so without this the number sat still
        // while a whole mesh subtool was being dragged: the manipulator moved,
        // the form did not, and a mesh subtool could not be transformed from
        // the application at all. The field side has no such gate; its surface
        // is re-meshed from the bricks the move dirtied.
        let carried = self
            .layers
            .iter()
            .filter(|layer| layer.representation != Representation::Sdf)
            .fold(0xcbf2_9ce4_8422_2325u64, |hash, layer| {
                let shown = u64::from(layer.visible && layer.carries_geometry);
                let hash = (hash ^ (layer.key.0 << 1 | shown)).wrapping_mul(0x1000_0000_01b3);
                let at = layer.transform;
                at.position
                    .into_iter()
                    .chain(at.rotation_axis)
                    .chain([at.rotation_angle])
                    .chain(at.scale)
                    .fold(hash, |hash, number| {
                        (hash ^ u64::from(number.to_bits())).wrapping_mul(0x1000_0000_01b3)
                    })
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
        // And every hierarchy's own evaluated revision, which the engine moves
        // whenever the drawn surface moved for any reason at all. Without it
        // the number sits still through a dab on a hierarchy: a hierarchy is
        // not in the brick cache, its layer's triangles are the cage and the
        // cage does not move, and nothing else here counts anything a stamp
        // touched — so the surface would change and the viewport would go on
        // drawing what it uploaded last.
        let hierarchies = self.layers.iter().fold(0u64, |sum, layer| {
            let Some(hierarchy) = layer.multires.as_ref() else {
                return sum;
            };
            let (evaluated, generation) = hierarchy.watched();
            let levels = hierarchy.levels();
            sum.wrapping_add(evaluated)
                .wrapping_mul(31)
                // This side's own generation, because the engine's counter
                // restarts at one whenever a hierarchy is put back from bytes
                // — so an undo and the redo after it would leave the same
                // number over two different surfaces. See
                // `crate::multires::Hierarchy::generation`.
                .wrapping_add(generation)
                .wrapping_mul(31)
                // The display level too. Moving it re-meshes nothing inside the
                // engine — the levels are all there — so no revision moves,
                // and the viewport would keep the level it had.
                .wrapping_add(u64::from(levels.display))
                .wrapping_mul(31)
                .wrapping_add(u64::from(levels.count))
        });
        let meshes = (self.mesh_undo.len() as u64) << 32 | self.mesh_redo.len() as u64;
        meshes
            .wrapping_mul(31)
            .wrapping_add(grids)
            .wrapping_add(carried)
            .wrapping_add(hierarchies.wrapping_mul(4_000_037))
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

    /// What the active mesh layer's ray-query tree costs.
    ///
    /// The engine's own words for the figure: the expected number of triangle
    /// tests a random ray must make. Lower is better, and it is only
    /// meaningful **against the same tree's own history** — a tree that has
    /// been refitted through a hundred dabs, against what it scored when it
    /// was built. It says nothing across two models, and it is not a measure
    /// of how stretched the triangles are, which is what this used to claim.
    ///
    /// Nothing here retessellates, because that would spend the retopology the
    /// import was for; what a decayed figure argues for is a rebuild of the
    /// tree, which is queued rather than done — see `crate::maintenance`.
    ///
    /// `None` where the active layer is not a sculpted mesh.
    pub fn mesh_quality(&self) -> Option<f32> {
        let key = self.active_layer().key;
        self.sculptor_for(key)
            .and_then(|sculptor| sculptor.borrow_mut().quality().ok())
    }

    /// Stamps that were handed a seed from a numbering that had been retired,
    /// and scanned instead.
    ///
    /// Summed over the sculptors this document is holding, which is what makes
    /// it a live reading rather than a session total: a sculptor rebuilt after
    /// an eviction starts its own count at zero, and the figure falls with it.
    /// That is the honest shape for a diagnostic — it answers "is this
    /// happening to the meshes in hand", which is the question a reader
    /// watching a brush behave oddly is actually asking.
    ///
    /// Zero is the normal reading and says nothing is wrong. A figure that
    /// climbs says a pick's seed keeps outliving the numbering it was taken
    /// in — the engine catching, one stamp at a time, what it was given the
    /// token to catch. See [`crate::seed`] for what it would cost if it could
    /// not.
    pub fn stale_seeds_rejected(&self) -> usize {
        self.mesh_sculptors
            .borrow()
            .values()
            .filter_map(|sculptor| sculptor.borrow().stale_seeds_rejected().ok())
            .sum()
    }

    /// How many mesh sculptors are held, so the figure above can be read.
    ///
    /// A count of zero and a rejection count of zero are the same number and
    /// different facts.
    pub fn mesh_sculptors_held(&self) -> usize {
        self.mesh_sculptors.borrow().len()
    }

    // -- what this document costs -------------------------------------------

    /// The ledger for the surfaces this document is holding *beside* itself.
    ///
    /// A [`claycore::MeshSculptor`] and a [`crate::multires::Hierarchy`] are
    /// both owning handles the host keeps next to its document rather than
    /// inside it — the engine cannot walk either, so
    /// [`claycore::Document::memory`] reports the whole surface tier as zero
    /// and that is ownership rather than an omission. Only this side knows
    /// which of them belong to this document, so only this side can fill the
    /// ledger, and the engine's API says so by taking one.
    ///
    /// **Both**, and that is the whole of what this walk is for. A mesh
    /// sculpting session is held in one map and a hierarchy on its own layer,
    /// so a roll-up that walks only the map answers for a hierarchy-holding
    /// document exactly what it answers for an empty one: measured, an 8x8
    /// cage subdivided six times reported `total 24444, rebuildable 0,
    /// surfaces 0` at zero levels and at six alike, while the hierarchy beside
    /// it held 26,233,592 bytes of which 15,742,640 were rebuildable. That is
    /// the omission the entry point taking a ledger exists to prevent, and it
    /// is worse than a small answer: `surfaces` is what tells "there are none"
    /// from "the host never filled it", so zero there was a claim as well as a
    /// figure.
    ///
    /// Returns how many surfaces were asked as well as what they answered,
    /// because zero surfaces and a zero ledger are the same number and
    /// different facts.
    ///
    /// The ledger is accumulated onto the *first* answer rather than onto a
    /// default one, and that is not tidiness: merging carries the shorter of
    /// the two category counts, so folding into a zeroed ledger would report
    /// every category as unfilled however many surfaces were added to it.
    pub fn surface_ledger(&self) -> Result<(usize, claycore::MemoryLedger), ModelError> {
        let mut ledger: Option<claycore::MemoryLedger> = None;
        let mut surfaces = 0;
        let mut fold = |one: claycore::MemoryLedger| {
            surfaces += 1;
            match ledger.as_mut() {
                Some(into) => into.merge(&one),
                None => ledger = Some(one),
            }
        };
        for sculptor in self.mesh_sculptors.borrow().values() {
            fold(
                sculptor
                    .borrow_mut()
                    .memory_ledger()
                    .map_err(ModelError::engine)?,
            );
        }
        for layer in &self.layers {
            if let Some(hierarchy) = layer.multires.as_ref() {
                fold(hierarchy.memory_ledger()?);
            }
        }
        Ok((surfaces, ledger.unwrap_or_default()))
    }

    /// Where this document's memory is, with the surfaces beside it folded in.
    ///
    /// [`claycore::Document::memory_with_surfaces`] rather than the plain
    /// roll-up, because the plain one omits every surface the host owns — and
    /// a mesh-sculpting session over a few million triangles is comfortably
    /// the largest thing an artist is holding. A figure that leaves it out is
    /// not a smaller answer to the same question, it is an answer to a
    /// different one.
    pub fn memory(&self) -> Result<claycore::MemoryReport, ModelError> {
        let (_, surfaces) = self.surface_ledger()?;
        self.document
            .memory_with_surfaces(&surfaces)
            .map_err(ModelError::engine)
    }

    /// The same figures as the diagnostics report carries them.
    ///
    /// `None` where the engine refused the question, which is what keeps a
    /// report that is opened *because* something has gone wrong from being the
    /// thing that cannot be opened.
    pub fn memory_diagnostics(&self) -> Option<clayspace_model::MemoryDiagnostics> {
        let (surfaces, ledger) = self.surface_ledger().ok()?;
        let report = self.document.memory_with_surfaces(&ledger).ok()?;
        Some(clayspace_model::MemoryDiagnostics {
            essential: report.essential,
            rebuildable: report.rebuildable,
            undoable: report.undoable,
            total: report.total,
            surfaces,
            surface_bytes: ledger.total,
        })
    }

    // -- work that is not required for correctness ---------------------------

    /// Opens or shuts a gesture, and everything that follows exactly from it.
    ///
    /// `previewing` is written nowhere else, because two other things track it
    /// and the whole value of them is that they cannot come apart from it: the
    /// maintenance queue's gate, which must be shut for exactly as long as a
    /// pointer is down, and the memory pin, which must be held for exactly as
    /// long and given back on every way out. Both were reachable from five
    /// separate assignments before this; now there is one door.
    fn set_previewing(&mut self, open: bool) {
        self.previewing = open;
        if open {
            self.maintenance.open_gesture();
        } else {
            self.maintenance.close_gesture();
        }
    }

    /// Queues work that would make the next interaction cheaper, or folds it
    /// into the identical request already queued.
    ///
    /// Nothing is done here. What is queued is a request, and a request this
    /// document never services leaves the form exactly where it is — which is
    /// what makes it safe to call from inside a stroke, where a drag asks for
    /// the same rebuild on every segment and the queue keeps one entry.
    ///
    /// `estimated_micros` is what the caller believes it will cost, and zero
    /// means "unknown" exactly as the engine means it — which is what most
    /// callers honestly have, and what the budget lets through once so that
    /// there is something to measure.
    pub fn request_maintenance(
        &mut self,
        kind: claycore::MaintenanceKind,
        target: u32,
        estimated_micros: u64,
    ) {
        self.maintenance
            .request_costing(kind, target, estimated_micros);
    }

    /// What is queued, in queue order.
    pub fn maintenance_queued(&self) -> Vec<claycore::MaintenanceItem> {
        self.maintenance.queued()
    }

    /// The pin every trim this document takes is handed.
    ///
    /// A trim releases what is rebuildable, and the engine prices what that
    /// costs the interaction after it rather than asserting it: 0.62–2.04x at
    /// Warning and 13–182x at Critical, growing with the model. A pin is what
    /// keeps that cost from landing in the middle of a drag — a trim taken
    /// while one is held releases nothing and reports what it *would* have
    /// released, so a memory warning stays honest without a surface going out
    /// from under a gesture in flight.
    ///
    /// Nothing in this document trims yet: a trim reaches a hierarchy or an
    /// adaptive surface, and this application holds neither. What this
    /// accessor buys before then is that the first one cannot be written
    /// without a pin to hand it.
    pub fn memory_pin(&self) -> Option<&claycore::MemoryPin> {
        self.maintenance.pin()
    }

    /// What an index rebuild has been measured to cost on this machine, or
    /// `None` before the first one.
    ///
    /// The engine carries no machine model and says so, so the first rebuild
    /// is filed with no estimate and timed, and every request after it is
    /// weighed against the budget using this.
    pub fn measured_rebuild_micros(&self) -> Option<u64> {
        self.maintenance.measured_rebuild_micros()
    }

    /// Whether a gesture is holding the memory pin.
    ///
    /// The balance this document has to keep: held for exactly as long as a
    /// pointer is down, and given back on every way out including the ones
    /// that unwind.
    pub fn memory_pinned(&self) -> bool {
        self.maintenance.is_pinned()
    }

    /// The between-strokes moment: a budgeted drain of the maintenance queue.
    ///
    /// Reached from every way a gesture ends — the pointer coming up, the
    /// gesture cancelled, a cage applied or abandoned — because those are the
    /// moments where a stall belongs to nobody, and the queue itself refuses
    /// to be drained anywhere else.
    fn settle_between_strokes(&mut self) {
        self.drain_maintenance(crate::maintenance::Maintenance::BUDGET);
    }

    /// Does what the queue holds until the budget is spent, and reports how
    /// many items were serviced.
    ///
    /// The four lines the C header describes, with the one policy decision it
    /// leaves to a host written out: an item is *started* only if what is left
    /// of the budget covers the estimate it carries. An item with no estimate
    /// is started once — that is how this host learns what one costs, and it
    /// is the only overrun this loop can produce. An item that has been
    /// measured and does not fit stops the drain rather than being stepped
    /// over, because `take_next` hands out the head of the queue and a loop
    /// that skipped it would ask for the same item again forever.
    ///
    /// Declining is not dropping. `take_next` peeks and `complete` removes, so
    /// what the budget did not reach is still there next time, with its own
    /// count of the asking climbing where a host is starving it.
    ///
    /// Refused outright while a gesture is open. That is the gate, and it is a
    /// mechanism rather than a convention: there is no queue to take work from
    /// while a [`claycore::StrokeScope`] holds it.
    pub fn drain_maintenance(&mut self, budget: std::time::Duration) -> usize {
        let Some(mut queue) = self.maintenance.take_for_drain() else {
            return 0;
        };
        let started = std::time::Instant::now();
        let mut serviced = 0;
        loop {
            let left = budget.saturating_sub(started.elapsed());
            let item = match queue.take_next() {
                Ok(Some(item)) => item,
                // Empty. It cannot mean the gate here — the queue is in this
                // loop's hand, which is what taking it out was for.
                Ok(None) => break,
                Err(e) => {
                    eprintln!("a fila de manutenção não pôde ser lida: {e}");
                    break;
                }
            };
            if !crate::maintenance::Maintenance::affordable(&item, left) {
                break;
            }
            self.perform_maintenance(item);
            match queue.complete(item.kind, item.target) {
                Ok(_) => serviced += 1,
                Err(e) => {
                    // Left queued, which is the safe direction — the work was
                    // done, and doing it again costs time rather than
                    // correctness — but the loop has to stop, or it would take
                    // the same head item forever.
                    eprintln!("um item de manutenção não pôde ser encerrado: {e}");
                    break;
                }
            }
        }
        self.maintenance.put_back(queue);
        serviced
    }

    /// Does one item.
    ///
    /// Only one kind is reachable here. The other four name an adaptive
    /// surface's chunk arena, a hierarchy's detail field, its slot pools and a
    /// deferred normal flush: this application holds neither an adaptive
    /// surface nor a hierarchy, and its deferred normals are settled by the
    /// gesture that deferred them rather than queued — `LiveMesh` owes the
    /// flush to the handle that owes it, and settles on `Drop`, which is a
    /// stronger guarantee than a queue entry for the one item that is not
    /// optional. An item of a kind
    /// nobody here produces is still *completed* rather than left, because a
    /// head item nothing will ever service blocks everything behind it.
    fn perform_maintenance(&mut self, item: claycore::MaintenanceItem) {
        if item.kind == claycore::MaintenanceKind::IndexRebuild {
            self.rebuild_mesh_index(LayerKey(u64::from(item.target)));
        }
    }

    /// Rebuilds a mesh layer's ray-query tree, if it has drifted far enough
    /// from what it scored when it was built to be worth it.
    ///
    /// The engine measures and the host decides, and this is the deciding. A
    /// stroke refits, which keeps the tree a valid partition of the same
    /// triangles at a cost proportional to the brush; what refitting does not
    /// do is keep that partition a *good* one, and after enough of it queries
    /// get slower with nothing saying so. `quality` is what says so — the
    /// expected number of triangle tests a random ray must make — and it is
    /// only meaningful against the same tree's own history, which is why the
    /// figure a tree scored when it was built is kept beside it.
    ///
    /// Nothing happens where the sculptor has gone. A layer whose sculptor was
    /// evicted, removed or rebuilt has no decayed tree to speak of: the one it
    /// has next is new.
    fn rebuild_mesh_index(&mut self, layer: LayerKey) {
        let Some(sculptor) = self.sculptor_for(layer) else {
            return;
        };
        // `try_borrow_mut` rather than `borrow_mut`: a drain runs at the
        // moment a gesture ended, and a sculptor still borrowed there is a bug
        // in this file rather than something a user can reach.
        let Ok(mut sculptor) = sculptor.try_borrow_mut() else {
            debug_assert!(false, "a tree was rebuilt while its sculptor was borrowed");
            return;
        };
        let Ok(quality) = sculptor.quality() else {
            return;
        };
        if !self.maintenance.has_decayed(layer, quality) {
            // Declined, and the reading it was declined on is what that cost —
            // which is not a rebuild's cost and is deliberately not recorded
            // as one.
            return;
        }
        let started = std::time::Instant::now();
        if let Err(e) = sculptor.refresh() {
            eprintln!("a árvore de consulta não pôde ser reconstruída: {e}");
            return;
        }
        let took = started.elapsed();
        let rebuilt = sculptor.quality().ok();
        drop(sculptor);

        self.maintenance.note_rebuild_cost(took);
        if let Some(rebuilt) = rebuilt {
            // A new tree, so a new figure to measure the next drift against.
            self.maintenance.note_baseline(layer, rebuilt);
        }
    }

    /// Asks for a mesh layer's tree to be rebuilt between strokes.
    ///
    /// Called from every path that writes through a mesh sculptor. It is a
    /// *request*: whether the tree has actually decayed is read once at the
    /// drain rather than once per segment, because the reading walks the tree
    /// and a drag would pay for it on every pointer move to learn something
    /// that only changes slowly.
    ///
    /// The queue's target is a `u32` and a layer key is a `u64`, which is a
    /// truncation this document cannot reach: keys are handed out one per
    /// layer ever made, so aliasing one would take four billion subtools in a
    /// single session.
    fn request_index_rebuild(&mut self, layer: LayerKey) {
        self.maintenance
            .request(claycore::MaintenanceKind::IndexRebuild, layer.0 as u32);
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

    /// The mirror on the redo side: whether the newest undone mesh gesture is
    /// the next thing forward.
    fn mesh_redo_is_next(&self) -> bool {
        self.mesh_redo
            .last()
            .is_some_and(|gesture| gesture.engine_depth == self.engine_undo_depth())
    }

    /// Takes back one carried gesture, bit exactly.
    fn undo_mesh_gesture(&mut self) -> Result<bool, ModelError> {
        let Some(gesture) = self.mesh_undo.pop() else {
            return Ok(false);
        };
        let Some(gesture) = self.step_gesture(gesture, Step::Back)? else {
            return Ok(true);
        };
        self.mesh_redo.push(gesture);
        Ok(true)
    }

    /// Puts one back.
    fn redo_mesh_gesture(&mut self) -> Result<bool, ModelError> {
        let Some(gesture) = self.mesh_redo.pop() else {
            return Ok(false);
        };
        let Some(gesture) = self.step_gesture(gesture, Step::Forward)? else {
            return Ok(true);
        };
        self.mesh_undo.push(gesture);
        Ok(true)
    }

    /// Applies one carried gesture in a direction, and hands back the record
    /// the other stack should hold.
    ///
    /// `None` where the layer the record belongs to has left the document:
    /// there is nothing to put back, and dropping the record is the whole of
    /// the answer.
    ///
    /// The two representations differ in what "the other stack's record" is,
    /// and it is worth naming. A `MeshDeltas` is *symmetric* — the same record
    /// reverts and re-applies — so it travels unchanged. A hierarchy's record
    /// is one **state**, so the record that goes the other way has to be the
    /// state this step is leaving: it is taken here, on the way past, which is
    /// why one blob is held per step rather than a before and an after.
    fn step_gesture(
        &mut self,
        gesture: MeshGesture,
        step: Step,
    ) -> Result<Option<MeshGesture>, ModelError> {
        let Ok(index) = self.index_of(gesture.layer) else {
            return Ok(None);
        };
        let MeshGesture {
            layer,
            what,
            engine_depth,
        } = gesture;
        let what = match what {
            GestureRecord::Deltas(deltas) => {
                let engine_name = self.layers[index].engine_name.clone();
                self.ensure_mesh_sculptor(layer, &engine_name)?;
                let Some(sculptor) = self.sculptor_for(layer) else {
                    return Ok(None);
                };
                {
                    let mut sculptor = sculptor.borrow_mut();
                    let stepped = match step {
                        Step::Back => deltas.revert(&mut sculptor),
                        Step::Forward => deltas.apply(&mut sculptor),
                    };
                    stepped.map_err(ModelError::engine)?;
                    sculptor.refit().map_err(ModelError::engine)?;
                }
                self.refresh_mesh_bounds(layer);
                GestureRecord::Deltas(deltas)
            }
            GestureRecord::Hierarchy(bytes) => {
                let Some(hierarchy) = self.layers[index].multires.as_mut() else {
                    return Ok(None);
                };
                // Taken before the restore rather than after: what the other
                // stack owes is the state this step is leaving.
                let leaving = hierarchy.bytes(0)?;
                hierarchy.restore(&bytes)?;
                self.refresh_multires_bounds(layer);
                GestureRecord::Hierarchy(leaving)
            }
        };
        Ok(Some(MeshGesture {
            layer,
            what,
            engine_depth,
        }))
    }

    /// Drops the oldest hierarchy records until the history fits its budget.
    ///
    /// A hierarchy's record is its whole serialized state, which is exact and
    /// is not small — see [`GestureRecord::Hierarchy`]. Bounded rather than
    /// unbounded, and from the *old* end, so what a session loses is the
    /// ability to walk back past a point rather than the ability to take back
    /// what it just did.
    fn trim_gesture_history(&mut self) {
        let weigh = |stack: &[MeshGesture]| -> usize {
            stack
                .iter()
                .map(|gesture| match &gesture.what {
                    GestureRecord::Hierarchy(bytes) => bytes.len(),
                    GestureRecord::Deltas(_) => 0,
                })
                .sum()
        };
        while weigh(&self.mesh_undo) + weigh(&self.mesh_redo) > crate::multires::HISTORY_BYTES {
            if self.mesh_undo.is_empty() {
                break;
            }
            self.mesh_undo.remove(0);
        }
    }

    /// Builds the sculptor for a mesh layer, or keeps the one already built.
    ///
    /// Kept *per layer*: a sculptor holds adjacency for the mesh it was given,
    /// so one is never carried across layers — but several are held at once,
    /// which is what makes going back to a mesh subtool a lookup rather than
    /// the weld again. See [`crate::sculptors`].
    fn ensure_mesh_sculptor(&mut self, key: LayerKey, engine_name: &str) -> Result<(), ModelError> {
        if self.mesh_sculptors.borrow().holds(key) {
            return Ok(());
        }
        // Relative to the bounding-box diagonal: vertices closer than this are
        // one point of the surface, which is what lets a brush move a split
        // seam as a seam rather than tearing it open.
        const WELD: f32 = 1e-4;
        let mut sculptor = claycore::MeshSculptor::for_layer(&mut self.document, engine_name, WELD)
            .map_err(ModelError::engine)?;
        // What this tree scores while it is new, which is the only figure a
        // later reading of it means anything against: the engine is explicit
        // that the number compares a tree with its own history and never with
        // another model's. Read here rather than at the first stroke, because
        // this is the one moment the partition is known to be a good one.
        let quality = sculptor.quality().ok();
        self.mesh_sculptors
            .borrow_mut()
            .insert(key, std::rc::Rc::new(std::cell::RefCell::new(sculptor)));
        if let Some(quality) = quality {
            self.maintenance.note_baseline(key, quality);
        }
        let standing: Vec<LayerKey> = self.layers.iter().map(|layer| layer.key).collect();
        self.maintenance
            .retain_baselines(|layer| layer == key || standing.contains(&layer));
        Ok(())
    }

    /// The sculptor for a layer, shared, or `None` where none has been built.
    ///
    /// Handed out by reference count rather than borrowed, so that a caller
    /// may keep it past the borrow it was taken under — which is what a
    /// gesture holding one across the frames of a drag needs. Asking counts as
    /// a use, exactly as [`crate::sculptors::Held::get_mut`] says.
    fn sculptor_for(&self, key: LayerKey) -> Option<crate::sculptors::SharedSculptor> {
        self.mesh_sculptors.borrow_mut().get_mut(key).cloned()
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

    /// The Move brush on a grid: a drag, accumulated past the cell size.
    ///
    /// Its own route rather than an arm of `stroke_voxel`'s loop, for two
    /// reasons that are really one. A drag is a single instruction over the
    /// whole gesture — a stamp per sample would be a *series* of grabs each
    /// anchored where the last stopped, which reaches nearly twice as far and
    /// moves less. And it has to remember what it has already done between
    /// segments, which the loop has nowhere to keep.
    ///
    /// The anchor is where the press landed. The displacement sent is the part
    /// of the gesture the grid has *not* yet been moved by, quantised to whole
    /// cells: `clay_voxel_sculpt_grab` rounds per axis, so anything smaller
    /// rounds to nothing, and a slow drag fed raw deltas moves nothing at all
    /// however far it travels.
    fn voxel_grab_stroke(
        &mut self,
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
        // A grid with no scale has no cell to accumulate past, so there is no
        // displacement this can quantise. NaN answers false here too, which is
        // the point of asking it this way round.
        if voxel_size <= 0.0 || !voxel_size.is_finite() {
            return Ok(EditOutcome::NOTHING);
        }

        // Opened by whichever segment arrives first, because the press itself
        // does not reach here: a dragging tool needs two samples and the first
        // segment is where it gets them.
        let gesture = *self.voxel_grab.get_or_insert(VoxelGrab {
            anchor: samples[0].position,
            emitted: [0.0; 3],
        });
        let last = samples[samples.len() - 1].position;
        // Whole cells, from the anchor, less what the grid has already been
        // moved by. Rounded rather than truncated so the record matches what
        // the engine does with the number: it rounds too, and a caller that
        // truncated would lag the grid by up to a cell for the whole gesture.
        let steps: [f32; 3] = std::array::from_fn(|axis| {
            ((last[axis] - gesture.anchor[axis] - gesture.emitted[axis]) / voxel_size).round()
        });
        if steps.iter().all(|step| *step == 0.0) {
            // Under a cell on every axis. Not an edit, and reporting one would
            // put an entry in the history for a gesture that has not landed.
            return Ok(EditOutcome::NOTHING);
        }
        let displacement: [f32; 3] = std::array::from_fn(|axis| steps[axis] * voxel_size);

        let brush = brush.sanitized();
        // The grid and the layer's own mask out of one borrow. The two used to
        // come from different places — the document and a field beside it —
        // which is exactly what made the mask unsaveable; see
        // `Document::voxel_layer_masked`.
        let claycore::MaskedGrid { mut grid, mask, .. } = self
            .document
            .voxel_layer_masked(&engine_name)
            .map_err(ModelError::engine)?;
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
        let before = grid.change_count().map_err(ModelError::engine)?;

        for mirror in mirrors(symmetry) {
            let at = mirror.point(gesture.anchor);
            let cell = [
                (at[0] / voxel_size).round() as i32,
                (at[1] / voxel_size).round() as i32,
                (at[2] / voxel_size).round() as i32,
            ];
            // Both the centre and the *direction*, which is the half that is
            // easy to forget: reflecting the anchor alone drags the mirrored
            // copy the same way in world space rather than as a reflection, so
            // a drag away from the plane pulls one side out and pushes the
            // other in.
            grid.sculpt_grab(
                cell,
                &params,
                mirror.vector(displacement),
                // As on the other two representations: Mover does not carry
                // the far side of a form along with the near one.
                true,
            )
            .map_err(ModelError::engine)?;
        }

        let after = grid.change_count().map_err(ModelError::engine)?;
        let _ = grid;
        // Recorded whether or not the grid moved. The displacement was whole
        // cells and it was sent; a drag over empty space changes nothing and
        // must still not be sent twice.
        if let Some(open) = self.voxel_grab.as_mut() {
            for (emitted, sent) in open.emitted.iter_mut().zip(displacement) {
                *emitted += sent;
            }
        }
        if after == before {
            return Ok(EditOutcome::NOTHING);
        }

        let key = self.active_layer().key;
        self.refresh_sculpt_layers(key)?;
        Ok(EditOutcome {
            changed: true,
            dirty_bricks: 1,
        })
    }

    /// Applies a stroke to a voxel layer, using the tool's own verb.
    fn stroke_voxel(
        &mut self,
        tool: ToolKind,
        brush: BrushSettings,
        samples: &[GestureSample],
        symmetry: [bool; 3],
    ) -> Result<EditOutcome, ModelError> {
        // In the grid's own coordinates, as the field and mesh routes are in
        // theirs: every voxel entry point addresses cells from the grid's
        // origin, and the layer transform is the host's to compose. Carried
        // before the drag branch as well as the stamping loop, so a gesture
        // that spans both is measured in one frame.
        let placement = self.carried_placement(self.active_layer().key);
        let carried = Self::carried_samples(&placement, samples);
        let samples = carried.as_deref().unwrap_or(samples);
        let brush = match &placement {
            // A subtool scaled to half its size wants half the radius against
            // the cells it actually holds.
            Some(transform) => BrushSettings {
                size: brush.size / transform.largest_scale(),
                ..brush
            },
            None => brush,
        };

        // A drag is one instruction over the whole gesture rather than a stamp
        // per sample, and its own state has to survive between segments, so it
        // takes its own route rather than an arm in the loop below.
        if tool == ToolKind::Mover {
            return self.voxel_grab_stroke(brush, samples, symmetry);
        }
        let index = self.active;
        let engine_name = self.layers[index].engine_name.clone();
        let voxel_size = {
            let (_, grid) = self
                .document
                .voxel_layer(&engine_name)
                .map_err(ModelError::engine)?;
            grid.voxel_size().map_err(ModelError::engine)?
        };
        // Read before the document is borrowed, for the same reason the alpha
        // below is.
        let chosen = self.colour.current();
        // The grid and the layer's own mask out of one borrow — see
        // `Document::voxel_layer_masked`. What used to be here was a split of
        // `self` by field, because the mask lived beside the document rather
        // than inside it.
        let claycore::MaskedGrid { mut grid, mask, .. } = self
            .document
            .voxel_layer_masked(&engine_name)
            .map_err(ModelError::engine)?;
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

        // Index 0 is the engine's empty slot, so a fresh grid has no colour to
        // deposit and every set would write emptiness.
        //
        // Two indices, because "put material here" and "put *this colour*
        // here" are different instructions. A structural deposit keeps the
        // neutral clay tone whatever the swatch says — a sculptor blocking out
        // in red would otherwise find every dab red — and only the colour
        // brush resolves the chosen colour.
        let material = if grid.palette_size().map_err(ModelError::engine)? > 1 {
            1
        } else {
            grid.palette_add(clayspace_model::Colour::CLAY.rgb)
                .map_err(ModelError::engine)?
        };
        let painted = if tool.writes_colour() {
            palette_entry(&mut grid, chosen)?
        } else {
            material
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
                    // Two-sided, which is what the grid's flatten is: material
                    // above the plane goes *and* hollows below it fill. The
                    // SDF and mesh sides of this tool are cut-only, and that
                    // difference is stated in the tooltip rather than faked —
                    // reproducing cut-only here would mean reading occupancy
                    // back and reapplying it, which is voxel math this
                    // application does not do.
                    //
                    // The same plane normal Raspar uses, and mirrored with the
                    // stroke for the same reason: the engine takes a normal
                    // rather than deriving one, and two verbs that plane the
                    // same surface must not disagree about where the plane is.
                    // No inverse bound: the engine defines none, and a
                    // two-sided verb has no side to swap.
                    ToolKind::Planar => {
                        grid.sculpt_flatten(cell, &params, mirror.vector([0.0, 1.0, 0.0]), 0.0)
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
                    ToolKind::Pintar => grid.paint_brush(cell, &params, painted),
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
            // A field layer's transform moves what the tape evaluates, so the
            // form is drawn where the manipulator put it and a ray picks it
            // there — while the stamps a stroke deposits go into the layer's
            // own frame, which the transform then moves *again*. Measured: a
            // subtool dragged three units along X was sculpted three units
            // past the pointer and the surface under the brush never moved.
            // So the gesture is carried back into that frame before anything
            // at all is derived from it, and the brush with it — a subtool
            // scaled to half its size wants half the radius against the items
            // it actually holds. The same conversion a carried mesh has always
            // made, in the one place both field routes pass through, so the
            // baked verbs and the stamping ones cannot disagree about where a
            // stroke landed.
            //
            // The mirror follows, and rightly: reflected in the layer's own
            // frame, symmetry is about the subtool's axis rather than the
            // world's, which is what the engine's own layer mirror does to the
            // items on the stamping route.
            Representation::Sdf => {
                let key = self.active_layer().key;
                let placement = self.carried_placement(key);
                let carried = Self::carried_samples(&placement, samples);
                let samples = carried.as_deref().unwrap_or(samples);
                let brush = match &placement {
                    Some(transform) => BrushSettings {
                        size: brush.size / transform.largest_scale(),
                        ..brush
                    },
                    None => brush,
                };
                self.field_stroke(tool, brush, samples, symmetry)
            }
            Representation::Voxel => self.stroke_voxel(tool, brush, samples, symmetry),
            Representation::Mesh => self.stroke_mesh(tool, brush, samples, symmetry),
            Representation::Multires => self.stroke_multires(tool, brush, samples, symmetry),
        }
    }

    fn symmetry(&self) -> [bool; 3] {
        self.active_layer().symmetry
    }

    fn set_symmetry(&mut self, symmetry: [bool; 3]) -> Result<(), ModelError> {
        // Recorded, not written. The engine's mirror is pointed by the stroke
        // that uses it — see `point_the_mirror` and the note on `Layer::mirror`
        // for why the entry has to land inside a gesture rather than beside
        // one.
        self.layers[self.active].symmetry = symmetry;
        Ok(())
    }

    fn set_combine(&mut self, combine: CombineSettings) {
        self.combine = combine.sanitized();
    }

    fn combine(&self) -> CombineSettings {
        self.combine
    }

    fn set_colour(&mut self, colour: clayspace_model::Colour) {
        self.colour.choose(colour);
    }

    fn choose_recent_colour(&mut self, index: usize) -> bool {
        self.colour.choose_recent(index)
    }

    fn colour_state(&self) -> clayspace_model::ColourState {
        self.colour.clone()
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
                    note: None,
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
        // Read before the sculptors are borrowed: both are `&self`, and the
        // lease has to outlive the calls that consult it.
        let mask = self.active_mask();
        let moved = {
            let Some(sculptor) = self.sculptor_for(key) else {
                return Ok(EditOutcome::NOTHING);
            };
            let mut sculptor = sculptor.borrow_mut();
            let sculptor = &mut *sculptor;
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
                    mask.as_deref(),
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
                    mask.as_deref(),
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
        // Taper, twist and a lattice laid as an operation each move most of
        // the mesh, which is the other case the engine names for a rebuild.
        self.request_index_rebuild(key);

        if deltas.vertex_count().map_err(ModelError::engine)? > 0 {
            self.mesh_undo.push(MeshGesture {
                layer: key,
                what: GestureRecord::Deltas(deltas),
                engine_depth: self.engine_undo_depth(),
            });
            self.mesh_redo.clear();
        }
        self.refresh_mesh_bounds(key);
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
        // And a hierarchy is in neither, for the third time. It is picked
        // against the level the viewport is drawing rather than against the
        // cage its layer holds: the sculpt stands off the cage, so a ray
        // stopped at the cage would put the brush ring under the surface by
        // however much detail there is.
        if self.active_representation() == Representation::Multires {
            return self.pick_active_multires(origin, direction);
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
        // On both sides of the step, because between two of them is the only
        // moment this side can look at what the engine holds forward — and a
        // redo stack the engine truncated is the only word there is that an
        // edit landed since the last one.
        self.settle_history_room();
        let moved = self.undo_step();
        self.settle_history_room();
        moved
    }

    fn redo(&mut self) -> Result<bool, ModelError> {
        self.settle_history_room();
        let moved = self.redo_step();
        self.settle_history_room();
        moved
    }

    fn history(&self) -> HistoryState {
        // Both histories, because the menu and the shortcut ask this one
        // question and a mesh gesture is as undoable as an engine entry. A
        // depth that counted only the engine's would grey out Undo in the
        // middle of a mesh sculpting session.
        //
        // And minus what solo left there. Those are entries the engine holds
        // and the sculptor never made: counted, the panel would say a fresh
        // document had three things to take back because someone looked at one
        // subtool on its own.
        let hopped = Self::visibility_entries(&self.visibility_undo);
        let hopped_forward = Self::visibility_entries(&self.visibility_redo);
        // And what an in-place crossing spent past its one step, for the same
        // reason: the sculptor made one crossing, not three edits.
        let folded = Self::crossing_entries(&self.crossing_undo);
        let folded_forward = Self::crossing_entries(&self.crossing_redo);
        match self.document.undo_state() {
            Ok(state) => {
                let undo_depth = state.undo_depth.saturating_sub(hopped + folded);
                let redo_depth = state
                    .redo_depth
                    .saturating_sub(hopped_forward + folded_forward);
                HistoryState {
                    can_undo: undo_depth > 0 || !self.mesh_undo.is_empty(),
                    can_redo: redo_depth > 0 || !self.mesh_redo.is_empty(),
                    depth: undo_depth + self.mesh_undo.len(),
                    redo_depth: redo_depth + self.mesh_redo.len(),
                }
            }
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
        self.set_previewing(true);
        // A drag is anchored where the press landed, so the last one's anchor
        // must not be lying around when the next one opens.
        self.voxel_grab = None;
    }

    fn open_live_gesture(&mut self, tool: ToolKind, symmetry: [bool; 3]) -> bool {
        ClayDocument::open_live_gesture(self, tool, symmetry)
    }

    fn close_live_gesture(&mut self) -> Result<usize, ModelError> {
        ClayDocument::close_live_gesture(self)
    }

    fn discard_live_gesture(&mut self) -> usize {
        ClayDocument::discard_live_gesture(self)
    }

    fn end_gesture(&mut self) {
        self.set_previewing(false);
        // The tendril is finished; the next pull is its own.
        self.live_hook = None;
        // As is the drag.
        self.voxel_grab = None;
        // What the preview was holding becomes the edit. One record for the
        // whole drag, because every segment replaced the last rather than
        // adding to it.
        //
        // `finish` settles first: a gesture that deferred its normals owes the
        // record the recomputation before the record becomes an undo entry.
        if let Some(live) = self.live_mesh.take() {
            let engine_depth = self.engine_undo_depth();
            let (layer, deltas) = live.finish();
            self.mesh_undo.push(MeshGesture {
                layer,
                what: GestureRecord::Deltas(deltas),
                engine_depth,
            });
            self.mesh_redo.clear();
        }
        // And the hierarchy's, which is held on the layer rather than beside
        // the document: a hierarchy gesture has no `MeshDeltas` to hold, so
        // there is nothing here for `live_mesh` to have been carrying.
        let hierarchies: Vec<LayerKey> = self
            .layers
            .iter()
            .filter(|layer| {
                layer
                    .multires
                    .as_ref()
                    .is_some_and(crate::multires::Hierarchy::gesture_is_open)
            })
            .map(|layer| layer.key)
            .collect();
        for key in hierarchies {
            self.bank_multires_gesture(key);
        }
        // The pointer is up. Whatever the drag made necessary, and this
        // document can afford, is done now — on a budget, because this is the
        // only moment where a stall belongs to nobody.
        self.settle_between_strokes();
    }

    fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        // The active layer's, which is the same question `layer_bounds` answers
        // for any of them — one implementation, so Frame All and the widgets
        // that size themselves to a subtool cannot come to disagree.
        SceneModel::layer_bounds(self, self.active_layer().key)
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

/// The palette index a colour paints with, adding an entry only where the grid
/// has none close enough.
///
/// A grid stores indices; the colour lives in the palette. Resolving here
/// rather than at the call site is what keeps the two questions "which colour"
/// and "which slot" apart — the domain names a colour and the engine adapter
/// finds the slot.
///
/// Matched within [`clayspace_model::ColourState::SAME`] rather than exactly.
/// A colour wheel returns values a float apart as the pointer moves inside one
/// pixel, so an exact match would add an entry per stroke and a palette of
/// eight identical reds is a palette nobody can use. Two colours that round to
/// the same `#RRGGBB` are one colour.
///
/// Index 0 is the empty slot and is skipped: painting with it would erase.
///
/// A full palette falls back to the nearest entry rather than failing the
/// stroke. The engine caps a palette at 255, and "the closest colour you
/// already have" is a degradation a sculptor can see and work around, where a
/// refused stroke mid-gesture is not.
fn palette_entry(
    grid: &mut claycore::VoxelGridRef<'_>,
    colour: clayspace_model::Colour,
) -> Result<i32, ModelError> {
    let colour = colour.sanitized();
    let size = grid.palette_size().map_err(ModelError::engine)?;
    let mut nearest = (f32::INFINITY, 1i32);
    for index in 1..size as i32 {
        let entry =
            clayspace_model::Colour::new(grid.palette_color(index).map_err(ModelError::engine)?);
        let apart = entry.distance(colour);
        if apart <= clayspace_model::ColourState::SAME {
            return Ok(index);
        }
        if apart < nearest.0 {
            nearest = (apart, index);
        }
    }
    match grid.palette_add(colour.rgb) {
        Ok(index) => Ok(index),
        // The palette is full. There is always a nearest entry to fall back
        // on, because a full palette is not an empty one.
        Err(_) if size > 1 => Ok(nearest.1),
        Err(e) => Err(ModelError::engine(e)),
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

/// A curve being placed, and the sweep it is showing.
struct Curve {
    layer: LayerKey,
    points: Vec<CurvePoint>,
    selection: Vec<usize>,
    join: CurveJoin,
    profile: CurveProfile,
    /// The placed sweep, once there are enough points to have one.
    node: Option<claycore::NodeId>,
}

impl Curve {
    /// The guide as the engine takes it: x, y, z, radius per point.
    fn guide(&self) -> Vec<f32> {
        self.points
            .iter()
            .flat_map(|point| {
                [
                    point.position[0],
                    point.position[1],
                    point.position[2],
                    point.radius,
                ]
            })
            .collect()
    }
}

/// How a join reaches the engine.
fn point_type(join: CurveJoin) -> claycore::PointType {
    match join {
        CurveJoin::Corners => claycore::PointType::Hard,
        CurveJoin::Through => claycore::PointType::Spline,
        CurveJoin::Rounded => claycore::PointType::BSpline,
    }
}

/// How many of the two parameters a profile actually reads.
fn profile_params(profile: claycore::Profile) -> usize {
    match profile {
        claycore::Profile::Box => 2,
        _ => 1,
    }
}

/// The profile, and a parameter block sized for it.
///
/// The values are overwritten with the radius at that end of the guide: a
/// swept profile carries its own size, because the guide's per-point radius
/// reaches only the chain primitive.
fn profile_of(profile: CurveProfile) -> (claycore::Profile, [f32; 2]) {
    match profile {
        // Never reached: a round tube is a swept-sphere chain instead, which
        // takes a radius per point where this primitive takes one per end.
        CurveProfile::Circle => (claycore::Profile::Circle, [1.0, 1.0]),
        CurveProfile::Square => (claycore::Profile::Box, [1.0, 1.0]),
        CurveProfile::Hexagon => (claycore::Profile::Hexagon, [1.0, 1.0]),
        CurveProfile::Triangle => (claycore::Profile::Triangle, [1.0, 1.0]),
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
    /// Whether this is the stroke as it was drawn, rather than a reflection.
    ///
    /// Asked by anything that is true of the place the sculptor pointed at and
    /// not of its copies — the pick's own seed being the one that matters, since
    /// a class reflected through the origin names a class on the other side.
    fn is_identity(self) -> bool {
        !self.0.iter().any(|axis| *axis)
    }

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

/// A voxel drag in progress: where it was anchored, and how far the grid has
/// actually been moved.
///
/// `emitted` is what the *grid* did rather than what the pointer asked for,
/// which is the whole point of holding it. `clay_voxel_sculpt_grab` resamples
/// occupancy nearest-cell and rounds per axis — the engine's own note says a
/// drag fed raw pointer deltas "is dead until the caller accumulates them past
/// the voxel size" — and unlike the SDF drag, which takes a *total*
/// displacement from a fixed anchor and is idempotent, this one translates
/// occupancy destructively: two calls compose. So the difference between what
/// has been asked for and what has been done is the only thing that can be
/// sent, and it is quantised to whole cells *before* it is recorded, or the
/// record would drift from the grid by up to half a cell on every emission.
#[derive(Debug, Clone, Copy)]
struct VoxelGrab {
    /// Where the press landed. Fixed for the gesture, so the region dragged is
    /// the one that was under the pointer rather than one that chases it.
    anchor: [f32; 3],
    emitted: [f32; 3],
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

/// What a named SDF brush *is*: an operation, an accumulation and a footprint.
///
/// Here rather than on `ToolKind` for the reason [`mesh_verb`] is: the domain
/// names the verb as text and this is where the text becomes a call. The
/// domain's table declares that Vinco reaches `CLAY_OP_INCISE` on a field;
/// this is what incise means for that brush.
///
/// `None` for a tool with no SDF stamp mapping, which is what makes the
/// refusal in `stroke_sdf` a lookup rather than a second list to keep in step.
#[derive(Debug, Clone, Copy)]
struct SdfRecipe {
    /// The operation the tool *is*, or `None` to take the Combinar panel's.
    ///
    /// Set for the named brushes and clear for the three general strokes,
    /// which is the same split ZBrush makes: Standard, Layer and Inflate are
    /// shaped by their settings, and Clay and Crease are what they are.
    op: Option<clayspace_model::Combine>,
    /// The accumulation the tool *is*, or `None` to take the brush's Acumular.
    accumulation: Option<claycore::Accumulation>,
    /// The stamp's region and rim, as a multiple of the brush radius.
    reach: f32,
    /// How much of the standard lift the stamp asks for.
    lift: f32,
    /// What the tool does to the spacing the Flow slider asked for.
    ///
    /// A multiplier rather than a replacement, so Fluxo still does something
    /// on a brush with a dense stroke of its own: a fixed spacing would be a
    /// slider that moved and changed nothing.
    spacing: f32,
}

fn sdf_recipe(tool: ToolKind) -> Option<SdfRecipe> {
    let plain = SdfRecipe {
        op: None,
        accumulation: None,
        reach: 1.0,
        lift: 1.0,
        spacing: 1.0,
    };
    Some(match tool {
        // The general strokes: the panel shapes them.
        ToolKind::Padrao | ToolKind::Camada => plain,
        // Same op, different profile. See `INFLATE_REACH`/`INFLATE_LIFT` for
        // the measurements behind the two numbers.
        ToolKind::Inflar => SdfRecipe {
            reach: ClayDocument::INFLATE_REACH,
            lift: ClayDocument::INFLATE_LIFT,
            ..plain
        },
        // ClayBuildup: relief along the stroke with buildup accumulation,
        // which is exactly what the engine's equivalence table says Clay is.
        // Not a new primitive named Clay — an item shaped like a pat would
        // *add* a pat, where relief displaces the surface already there.
        ToolKind::Argila => SdfRecipe {
            op: Some(clayspace_model::Combine::Relief),
            accumulation: Some(claycore::Accumulation::Buildup),
            reach: ClayDocument::CLAY_REACH,
            lift: ClayDocument::CLAY_LIFT,
            spacing: ClayDocument::CLAY_SPACING,
        },
        // Crease / DamStandard: "a thin region gives the line", in the
        // engine's own words. The narrow region is the whole brush.
        ToolKind::Vinco => SdfRecipe {
            op: Some(clayspace_model::Combine::Incise),
            accumulation: Some(claycore::Accumulation::Buildup),
            reach: ClayDocument::CREASE_REACH,
            lift: ClayDocument::CREASE_LIFT,
            spacing: ClayDocument::CREASE_SPACING,
        },
        _ => return None,
    })
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
        // And no mesh binding for the topological drag: it bakes a re-sampled
        // volume, and a mesh's geodesic Grab is a different operation wearing
        // a similar description. Inventing the mapping because one exists
        // nearby is exactly what the table is for preventing.
        ToolKind::Mascara
        | ToolKind::Preencher
        | ToolKind::Trim
        | ToolKind::Apagar
        | ToolKind::MoverTopologico => return None,
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
            layers: self
                .layers
                .iter()
                .map(|layer| LayerSummary {
                    health: self.field_health(layer),
                    voxel: self.voxel_stats(layer),
                    ..layer.summary()
                })
                .collect(),
            active: self.layers.get(self.active).map(|layer| layer.key),
            soloed: self.solo.as_ref().map(|solo| solo.layer),
        }
    }

    /// The one place activation changes.
    ///
    /// Every route to it — the stack row, the viewport click, a layer that has
    /// just been created — arrives as `Command::SelectLayer`, so there is no
    /// second writer that could leave the picked layer and the sculpted one
    /// disagreeing.
    fn set_active_layer(&mut self, key: LayerKey) -> Result<(), ModelError> {
        let index = self.index_of(key)?;
        if index == self.active {
            return Ok(());
        }
        self.drop_a_foreign_cage(key);
        self.active = index;
        // The mask, the mirror and the rig belong to the subtool, so all three
        // change with it and none of them is re-pointed at the incoming one.
        // The mirror in particular: the engine already holds one per layer,
        // set when the sculptor toggled it there, so activation has nothing to
        // write — it used to, and that is what carried one subtool's symmetry
        // onto the next.
        //
        // The frozen region is drawn, and a different subtool's mask is a
        // different picture, so the viewport is told to look again. Nothing in
        // the surface moved, which is exactly why the mask carries a counter
        // of its own.
        self.mask_revision = self.mask_revision.wrapping_add(1);
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

    /// Shows one subtool alone, or releases the solo and puts the scene back.
    ///
    /// Nothing here touches `active`: the spec asks for a viewing convenience,
    /// and a sculptor who solos a layer to look at it has not said they want
    /// to sculpt on it.
    fn set_solo(&mut self, key: Option<LayerKey>) -> Result<(), ModelError> {
        let Some(key) = key else {
            let Some(solo) = self.solo.clone() else {
                return Ok(());
            };
            return self.write_visibility(&solo.was, None);
        };
        // Refused before anything is hidden, so a solo on a layer that has
        // gone leaves the scene as it was.
        self.index_of(key)?;

        // What to restore is what stood before the *first* solo, not what the
        // one already engaged left behind — otherwise soloing a second subtool
        // would make the first solo's hiding permanent. Layers that arrived
        // since keep what they have now, since the older snapshot has nothing
        // to say about them.
        let mut was = self.visibility_snapshot();
        if let Some(solo) = &self.solo {
            for (key, visible) in &solo.was {
                if let Some(entry) = was.iter_mut().find(|(known, _)| known == key) {
                    entry.1 = *visible;
                }
            }
        }

        let wanted: Vec<(LayerKey, bool)> = self
            .layers
            .iter()
            .map(|layer| (layer.key, layer.key == key))
            .collect();
        let before = self.visibility_snapshot();
        let outcome = self.write_visibility(&wanted, Some(Solo { layer: key, was }));
        // `write_visibility` states that "a batch that failed halfway is a
        // batch whose caller is about to restore", and this is that caller.
        // Each flag goes through `set_layer_visible`, which refills and can
        // genuinely fail; without the restore the scene was left with some
        // layers hidden and some not, and `self.solo` stayed `None` — so the
        // interface showed no solo engaged and offered nothing that would put
        // the rest of the scene back.
        if outcome.is_err() {
            let _ = self.write_visibility(&before, None);
        }
        outcome
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
        // A mesh layer is made by *carrying* a mesh, and there is no engine
        // call that makes an empty one — `attach_mesh_layer` takes the
        // triangles. Asked for one anyway, this used to fall through to
        // `add_sdf_layer` and then record the row as a mesh: the sculptor got a
        // row labelled "Malha" backed by a field layer that nothing could ever
        // put triangles into, offering the mesh vocabulary over nothing, and
        // active on arrival. The specification qualifies the offer — "SDF,
        // voxel and mesh *where a mesh source is at hand*" — and here there is
        // none, so this says so instead of making the dead row.
        if representation == Representation::Mesh {
            return Err(ModelError::engine(
                "uma camada de malha vem de uma malha importada; use Ficheiro → Importar",
            ));
        }
        // The same dead row, one representation further along, and it is the
        // one the `_` arm below would have made. A hierarchy is built from a
        // cage: `clay_multires_from_mesh` takes the triangles and there is no
        // call that makes an empty one at all. Asked for one, this would have
        // fallen through to `add_sdf_layer` and recorded the row as a
        // hierarchy — which is verbatim the defect the paragraph above
        // describes, so it is refused in the same place and for the same
        // reason rather than being discovered again later.
        if representation == Representation::Multires {
            return Err(ModelError::engine(
                "uma hierarquia de subdivisão vem de uma malha; converta uma \
                 camada de malha para multirresolução",
            ));
        }
        // Made unique before the engine sees it. A voxel layer's grid is
        // reachable only by name (ClayCore #365), so two of them sharing one
        // shadow each other; `rename_layer` refuses a collision a sculptor
        // typed, and this is the collision nobody typed.
        let name = &self.unique_layer_name(name);
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
        self.adopt_engine_layer(id, name, representation)
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
                        // A hierarchy has a pass stack too, and it is NOT this
                        // one: `SculptLayerOp` addresses a pass by its position
                        // in a grid's stack, and the hierarchy's is addressed
                        // by an id that a reorder does not renumber. The two
                        // are `clayspace_model::SculptLayerOp` and
                        // `clayspace_model::MultiresSculptLayerOp`, and this
                        // column stays empty so that pointing one at the other
                        // is a refusal rather than an off-by-one.
                        multires: None,
                    },
                    note: None,
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

    /// Moves the active hierarchy's levels, or changes how many it has.
    ///
    /// Three of the four move a number and cost nothing. The fourth allocates,
    /// and it is the one this method exists to price: a level multiplies faces
    /// by four, so a 20k-quad cage is 5.1M faces at level 4 and 20.5M at level
    /// 5 — and it is the **peak** during the build rather than what remains
    /// after it that ends a session on a constrained machine.
    ///
    /// `clay_multires_add_level` is build-then-publish: it prices the level
    /// against the budget the hierarchy was built with, refuses over it, and
    /// leaves the hierarchy exactly as deep as it was. The refusal is asked for
    /// here as well, one call earlier, so that what a sculptor reads is a
    /// [`Refusal`] naming the peak and the budget rather than an engine result
    /// code — and so the interface can grey the control before it is pressed.
    fn apply_multires_level_op(
        &mut self,
        op: clayspace_model::MultiresLevelOp,
    ) -> Result<(), ModelError> {
        use clayspace_model::MultiresLevelOp as Op;

        let key = self.active_layer().key;
        let index = self.active;
        if self.layers[index].multires.is_none() {
            // Named rather than generic, exactly as a grid's pass stack is: a
            // sculptor on a field or a mesh needs to know that levels are a
            // hierarchy's, not that "this failed".
            return Err(ModelError::Unavailable(
                clayspace_model::Unavailable::NoVerbHere {
                    active: self.layers[index].representation,
                    verbs: clayspace_model::Verbs {
                        sdf: None,
                        voxel: None,
                        mesh: None,
                        multires: Some("clay_multires_add_level"),
                    },
                    note: None,
                },
            ));
        }
        if let Some(refusal) = self.layers[index].protection.refusal() {
            return Err(ModelError::engine(refusal));
        }
        let hierarchy = self.layers[index]
            .multires
            .as_mut()
            .expect("checked just above");
        match op {
            Op::SetSculptLevel(level) => hierarchy.set_sculpt_level(level)?,
            Op::SetDisplayLevel(level) => hierarchy.set_display_level(level)?,
            Op::AddLevel => {
                let level = hierarchy.add_level()?;
                // What an artist means by subdividing is to work finer, so
                // both numbers move to the level that arrived — which is also
                // what the engine does, so a host that left the display where
                // it was would draw a surface the engine had moved on from.
                hierarchy.set_display_level(level)?;
                hierarchy.set_sculpt_level(level)?;
            }
            Op::RemoveHighestLevel => hierarchy.remove_highest_level()?,
        }
        // The drawn level changed, or the surface under it did.
        self.refresh_multires_bounds(key);
        self.refresh_stats();
        Ok(())
    }

    /// Acts on the active hierarchy's stack of passes.
    ///
    /// Not an undo entry, for the reason a grid's pass operations are not one:
    /// a pass is a control that stays adjustable long after the strokes that
    /// filled it, and a sculptor whose next undo took back a slider rather
    /// than the work would have to choose between the two.
    ///
    /// Two refusals are stated here rather than left to the engine, because
    /// both are sentences a sculptor can act on. A layer that is not a
    /// hierarchy has no stack at all — the same shape
    /// [`ClayDocument::apply_multires_level_op`] refuses in. And a composition
    /// change **while a gesture is open** is refused by the engine anyway: a
    /// stamp reads the evaluated surface, so a slider moved between two stamps
    /// would author one gesture against two different surfaces. Saying so here
    /// means the message names the stroke rather than an engine code, and
    /// means the three operations that move no vertex — a rename, a lock and a
    /// change of which pass is active — go through as the domain says they do.
    fn apply_multires_sculpt_layer_op(
        &mut self,
        op: clayspace_model::MultiresSculptLayerOp,
    ) -> Result<(), ModelError> {
        let key = self.active_layer().key;
        let index = self.active;
        if self.layers[index].multires.is_none() {
            return Err(ModelError::Unavailable(
                clayspace_model::Unavailable::NoVerbHere {
                    active: self.layers[index].representation,
                    verbs: clayspace_model::Verbs {
                        sdf: None,
                        voxel: None,
                        mesh: None,
                        multires: Some("clay_multires_add_sculpt_layer"),
                    },
                    note: None,
                },
            ));
        }
        if let Some(refusal) = self.layers[index].protection.refusal() {
            return Err(ModelError::engine(refusal));
        }
        let hierarchy = self.layers[index]
            .multires
            .as_mut()
            .expect("checked just above");
        if op.needs_the_stroke_closed() && hierarchy.gesture_is_open() {
            return Err(ModelError::engine(
                "termine o traço antes de mexer na composição dos passes",
            ));
        }
        hierarchy.apply_sculpt_layer_op(&op)?;
        if op.changes_the_surface() {
            // Only where the form moved. A reorder, a rename and a change of
            // which pass is active move nothing at all — the stack is additive
            // and therefore commutes — and re-deriving a hierarchy's bounds
            // for one of those would be paying for a picture that has not
            // changed.
            self.refresh_multires_bounds(key);
        }
        self.refresh_stats();
        Ok(())
    }

    fn subdivision_cost(&self) -> Option<clayspace_model::SubdivisionCost> {
        self.active_layer().multires.as_ref()?.subdivision_cost()
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
        // The mesh a sculptor was built over has just left the document, and
        // the engine answers every call on one of those with a refusal. A
        // removal history brings back is rebuilt from the geometry it comes
        // back with, which is why this is dropped rather than retired
        // alongside the row.
        self.mesh_sculptors.borrow_mut().forget(key);
        let retired = self.layers.remove(index);
        // Kept, because a removal is undoable and everything this side knows
        // about the layer is not — see `retired`.
        self.retired.insert(id, retired);
        // The sculpt target follows the layer it pointed at rather than the
        // *index* it sat on. Every row above the one removed shifts down by
        // one, and clamping alone left `active` where it was: removing the
        // first of three while the second was active moved the sculpt target
        // to the third — and with it the mask, the mirror and the rig, since
        // all three are the active subtool's now.
        if self.active > index {
            self.active -= 1;
        }
        self.active = self.active.min(self.layers.len() - 1);
        // A solo naming a layer that has gone is a solo no row can release:
        // the control is drawn per stack row and the soloed row is the one that
        // left, so the rest of the scene stayed hidden with no way back.
        self.release_solo_of(key)?;
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
        // The narrow route: a position and a size, keeping whatever rotation
        // the layer already has. It writes the same remembered transform the
        // manipulator does, so a layer moved by dragging reads back as moved
        // and the two cannot disagree about where it is.
        let index = self.index_of(key)?;
        let turned = self.layers[index].transform;
        self.place_layer(
            key,
            clayspace_model::Transform {
                position,
                // One number, because this route's callers have one — a
                // subtool stood somewhere at a size. The layer transform takes
                // three since ABI 0.74.0 and the manipulator writes three; a
                // uniform triple here is a placement that happens not to
                // stretch, not a claim that it cannot.
                scale: [scale.max(1e-4); 3],
                ..turned
            },
        )
    }

    fn layer_bounds(&self, key: LayerKey) -> Option<([f32; 3], [f32; 3])> {
        let index = self.index_of(key).ok()?;
        let layer = &self.layers[index];
        // A grid says where it is itself. `clay_layer_bounds` answers with a
        // layer's SDF extent, which a voxel layer does not have — it reported
        // nothing for one however much material was in it, so Frame All framed
        // the default box over a sculpt that was somewhere else.
        //
        // Placed, as the mesh arm below is: the cells are measured where the
        // grid holds them and the layer transform is what stands them
        // somewhere. Unplaced, this was the box the whole-subtool manipulator
        // sized and centred itself on, so the widget sat on a moved grid's old
        // position and Frame All framed where the sculpt had been.
        if layer.representation == Representation::Voxel {
            let measured = layer.voxel_bounds?;
            return Some(match self.carried_placement(key) {
                Some(transform) => Self::placed_box(&transform, measured),
                None => measured,
            });
        }
        // And a carried mesh says where it is itself, for the same reason: it
        // holds no SDF content either, so the engine reported nothing for one
        // however many triangles were in it — which left the whole-subtool
        // manipulator on a mesh sized to a default and Frame All framing
        // nothing. Placed, because the vertices are remembered where the engine
        // holds them and the layer transform moves them.
        //
        // A hierarchy answers from the same field and for the same reason. Its
        // layer holds the cage and carries no SDF content either, and what is
        // cached there is the *display level's* box — see
        // `refresh_multires_bounds` for why it cannot be the cage's.
        if matches!(
            layer.representation,
            Representation::Mesh | Representation::Multires
        ) {
            let measured = layer.mesh_bounds?;
            return Some(match self.carried_placement(key) {
                Some(transform) => Self::placed_box(&transform, measured),
                None => measured,
            });
        }
        self.document.layer_bounds(layer.id).ok().flatten()
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

    fn remesh_layer(
        &mut self,
        key: LayerKey,
        settings: clayspace_model::RemeshSettings,
    ) -> Result<clayspace_model::RemeshOutcome, ModelError> {
        let index = self.index_of(key)?;
        let layer = &self.layers[index];
        // Refused by representation rather than left to the engine's
        // NOT_FOUND, because "essa camada não é uma malha" is the sentence a
        // sculptor can act on and a result code is not. A field steepens and
        // is consolidated; a grid has cells and is resampled; only a mesh has
        // topology to rebuild.
        if layer.representation != Representation::Mesh {
            return Err(ModelError::engine(
                "remalhar reconstrói a topologia de uma malha; esta camada não é uma",
            ));
        }
        if !layer.carries_geometry {
            return Err(ModelError::engine(
                "esta camada de malha ainda não carrega triângulos",
            ));
        }
        if let Some(refusal) = layer.protection.refusal() {
            return Err(ModelError::engine(refusal));
        }
        let id = layer.id;

        // Dropped *before* the rebuild rather than after it. The sculptor
        // holds an adjacency and a BVH over triangles the rebuild is about to
        // replace, and while the engine refuses a stale one rather than
        // reading freed storage — it compares the layer's geometry revision
        // since ABI 0.64.0, which is what catches a rebuild landing on the
        // same vertex and index counts — a refusal arriving on the sculptor's
        // next stroke is a failure this side can simply not create.
        self.mesh_sculptors.borrow_mut().forget(key);

        let settings = settings.sanitized();
        // The form's longest extent, which is what turns the sculptor's switch
        // into the world-unit volume the engine's threshold is stated in. Read
        // before the rebuild, since afterwards it describes the result.
        let extent = self.layer_bounds(key).map(|(min, max)| {
            (0..3)
                .map(|axis| max[axis] - min[axis])
                .fold(0.0f32, f32::max)
        });
        let report = self
            .document
            .remesh_layer(id, Self::remesh_params(settings, extent))
            // The engine fills the report for a refusal too wherever the
            // numbers exist — an open-surface refusal carries the source's
            // boundary-edge count — so the count is what the sentence says
            // rather than the result code. The layer is byte-identical after
            // one, which is what makes offering a resolution safe.
            .map_err(|refused| {
                let report = refused.report;
                if report.source_was_open && report.source_boundary_edges > 0 {
                    ModelError::engine(format!(
                        "a malha está aberta em {} arestas e não pôde ser \
                         remalhada: {}",
                        report.source_boundary_edges, refused.error
                    ))
                } else {
                    ModelError::engine(refused.error)
                }
            })?;

        // The rebuild replaced every vertex and index, so what this side
        // remembers about the layer's geometry is now about a mesh that no
        // longer exists. Both are re-read here rather than left to the next
        // caller: the box sizes the manipulator and frames the view, and the
        // revision is what tells an undo of this rebuild that it has to do the
        // same again.
        self.refresh_mesh_bounds(key);
        self.settle_geometry_revisions();
        // Where this rebuild sits in the engine's history, so a step across it
        // in either direction is recognisable later. The revision alone cannot
        // do that — see [`Rebuild`] for the measurement.
        self.rebuilds.push(Rebuild {
            layer: key,
            engine_depth: self.engine_undo_depth(),
        });
        // Ready for the pointer on the frame this returns, as a crossing is.
        // Without it the first stroke after a rebuild has no sculptor, the
        // pick that would place it answers nothing, and the press orbits the
        // camera instead — which is the failure `to_mesh.rs` records for the
        // crossing and is reachable the same way here.
        //
        // The *active* layer, which is this one wherever the interface asked:
        // the control issues `SelectLayer` before `RemeshLayer` for the same
        // reason a conversion does. Arming `key` unconditionally would pay the
        // weld for a layer nobody is about to touch, and arming nothing would
        // leave the layer that is about to be touched without one.
        self.arm_mesh_sculptor();
        self.refresh_stats();

        Ok(clayspace_model::RemeshOutcome {
            triangles_before: report.source_triangles,
            triangles_after: report.result_triangles,
            voxel_size: report.voxel_size,
            pieces: report.result_components,
            pieces_removed: report.removed_components,
            watertight: report.result_watertight,
            uvs_dropped: report.uvs_dropped,
        })
    }

    fn add_mesh_layer(&mut self, name: &str) -> Result<LayerKey, ModelError> {
        // Carried, not sculpted: the layer is recorded so the tools can refuse
        // it by representation rather than by a special case.
        let id = self
            .document
            .add_sdf_layer(name)
            .map_err(ModelError::engine)?;
        let key = self.take_key();
        // A mesh row is recorded before its triangles arrive, which is what
        // `Layer::new` already says for this representation.
        self.layers
            .push(Layer::new(id, key, name, Representation::Mesh));
        Ok(key)
    }

    fn layer_at(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<LayerKey> {
        // Attributed, because a pick has to name what it met. The engine
        // excludes ghosted layers from picking, so honouring ghost is not
        // something this has to reimplement — a ray through a ghost answers
        // with whatever stands behind it.
        let hit = self
            .document
            .raycast_attributed(origin, direction)
            .ok()
            .flatten()?;
        let id = hit.layer?;
        self.layers
            .iter()
            .find(|layer| layer.id == id)
            .map(|layer| layer.key)
    }
}

/// The scene operations that reach further into the engine.
impl ClayDocument {
    pub fn layer_id(&self, key: LayerKey) -> Result<LayerId, ModelError> {
        self.index_of(key).map(|index| self.layers[index].id)
    }

    /// The engine's rebuild parameters for what the interface offers.
    ///
    /// Four controls become a parameter block of fourteen, and the rest are
    /// left at whatever `clay_mesh_voxel_remesh_defaults` says rather than
    /// transcribed here — the header asks callers not to transcribe them, and
    /// a value copied out on the day this was written is one that stops
    /// following the engine silently.
    ///
    /// Two of the four are stated rather than passed through. Colours are
    /// always carried across where the source has them, because a rebuild that
    /// dropped a sculptor's polypaint would be a repair that costs the work;
    /// the engine produces none where there were none, so there is nothing to
    /// decide. And an open surface is **closed** rather than refused: a
    /// sculptor reaching for this has a form that has gone wrong, and refusing
    /// the one operation that fixes it because it is not watertight is the
    /// interface being right about the wrong thing. The outcome says what came
    /// out, which is where "it closed a hole you wanted" is visible.
    fn remesh_params(
        settings: clayspace_model::RemeshSettings,
        extent: Option<f32>,
    ) -> claycore::RemeshParams {
        claycore::RemeshParams {
            resolution: claycore::Resolution::LongestAxis(settings.resolution),
            surface: if settings.sharp {
                claycore::Surface::Sharp
            } else {
                claycore::Surface::Smooth
            },
            open_surface: claycore::OpenSurface::Close,
            // What counts as a speck is a policy about forms rather than a
            // translation of the engine's vocabulary, so the number comes from
            // the model, where it is derived from the form's own extent and
            // unit-tested. `None` is "remove nothing", which covers both the
            // switch being off and there being no extent to scale by.
            small_components: match settings.loose_piece_volume(extent) {
                Some(volume) => claycore::SmallComponents::RemoveBelowVolume(volume),
                None => claycore::SmallComponents::Keep,
            },
            preserve_volume: true,
            projection: settings.follow_the_source.then_some(claycore::Projection {
                // A lerp and never a snap. Most of the way back to the source,
                // which recovers the detail the sampling rounded off; not all
                // of it, because a full pull reintroduces the self-intersecting
                // geometry the rebuild was asked to remove.
                strength: 0.8,
                // Beyond a cell or so the nearest point on the source is not
                // the corresponding one, and pulling a vertex there is how a
                // projection turns a clean rebuild into a spiky one.
                max_distance_voxels: 1.5,
            }),
            preserve_colors: true,
            ..claycore::RemeshParams::default()
        }
    }

    /// Re-reads every mesh layer's geometry revision, and drops what a change
    /// invalidates.
    ///
    /// The engine bumps a mesh layer's revision when its triangles are
    /// replaced wholesale and by nothing else — a brush moves vertices and
    /// leaves the topology alone, which is exactly the change a sculptor's
    /// adjacency and BVH survive. So a moved revision is the one signal that
    /// what this side holds over that layer is now about a mesh that is gone.
    ///
    /// Asked here rather than at each site that could rebuild one, because the
    /// site that matters calls nothing on this side at all: **undoing** a
    /// remesh puts the old triangles back from inside the engine's own
    /// history, and the number moving is the only account of it there is. A
    /// same-count rebuild is the case no other check catches — neither the
    /// pointer nor the counts move — and it is why the engine grew this number
    /// in ABI 0.64.0.
    fn settle_geometry_revisions(&mut self) {
        // Every layer a rebuild in this session could have put back or taken
        // away with the depth history now stands at. Both directions replace
        // the triangles, and the engine's revision reports neither, so this is
        // the only account there is. Cheap: the list holds one entry per
        // rebuild a sculptor has actually made, which is a handful per
        // session, and the depth is a field read.
        let depth = self.engine_undo_depth();
        let crossed: Vec<LayerKey> = self
            .rebuilds
            .iter()
            .filter(|rebuild| {
                // At the rebuild's own depth the layer holds the rebuilt
                // triangles; one step below it holds what they replaced. Both
                // are reachable by a single step from the other, so both are
                // the moment a cache over the layer stops describing it.
                rebuild.engine_depth == depth || rebuild.engine_depth == depth + 1
            })
            .map(|rebuild| rebuild.layer)
            .collect();
        for key in crossed {
            // Unconditional, unlike the revision path below: this is the case
            // where nothing observable moved. Bounded by there having been a
            // rebuild on this layer at all — a document nobody has run one on
            // pays nothing, which is what keeps an ordinary undo from putting
            // the weld back on the interface thread.
            self.mesh_sculptors.borrow_mut().forget(key);
            self.refresh_mesh_bounds(key);
        }

        let mesh_layers: Vec<(LayerKey, LayerId)> = self
            .layers
            .iter()
            .filter(|layer| layer.representation == Representation::Mesh)
            .map(|layer| (layer.key, layer.id))
            .collect();
        for (key, id) in mesh_layers {
            let Ok(revision) = self.document.mesh_layer_revision(id) else {
                continue;
            };
            let Ok(index) = self.index_of(key) else {
                continue;
            };
            if self.layers[index].geometry_revision == revision {
                continue;
            }
            self.layers[index].geometry_revision = revision;
            // Zero is "this layer has no geometry the engine will name", which
            // is where a mesh row stands before its triangles arrive. Nothing
            // was ever built over that, so there is nothing to drop and no box
            // to re-measure.
            if revision == 0 {
                continue;
            }
            self.mesh_sculptors.borrow_mut().forget(key);
            self.refresh_mesh_bounds(key);
        }
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

    /// Writes every hierarchy this document holds, beside the document.
    ///
    /// Priced before it allocates — see [`crate::multires::Hierarchy::bytes`]
    /// — and written whole, so a document that has lost its last hierarchy
    /// leaves no side-car behind to promote a mesh layer back into one on the
    /// next open.
    fn write_hierarchies(&self, path: &std::path::Path) -> Result<(), ModelError> {
        let held: Vec<crate::multires::Saved> = self
            .layers
            .iter()
            .enumerate()
            .filter_map(|(position, layer)| {
                let hierarchy = layer.multires.as_ref()?;
                Some(
                    hierarchy
                        .bytes(0)
                        .map(|bytes| crate::multires::Saved { position, bytes }),
                )
            })
            .collect::<Result<_, _>>()?;
        let sidecar = crate::multires::sidecar_for(path);
        crate::multires::write_hierarchies(&sidecar, &held).map_err(|e| {
            ModelError::engine(format!(
                "as hierarquias não puderam ser gravadas em {sidecar:?}: {e}"
            ))
        })
    }

    /// Puts every hierarchy the side-car holds back on the row it belongs to.
    ///
    /// A record naming a row that is not there, or one the engine will not
    /// reconstruct, drops **that row** and keeps the rest — the rule
    /// [`crate::objects::read_table`] states, and right here for the same
    /// reason: one unreadable record must not cost a document. What is
    /// different is what a dropped record means. A dropped object row loses
    /// which shape can be picked up again; a dropped hierarchy loses every
    /// level above the cage, so the row is left as the mesh layer it
    /// demonstrably now is and the loss is named where a sculptor can read it.
    ///
    /// Named for a record that could not be honoured **and** for a file that
    /// could not be parsed into records at all. The second was silent before:
    /// a `.multires` one byte short, or with a header this build does not
    /// know, produced an empty list that reads exactly like a document which
    /// never held a hierarchy, so every level above every cage went and the
    /// report said `0 lost`. A damaged file cannot say which rows it was
    /// holding — that is what being damaged means — so it is named as the file
    /// rather than by row.
    fn read_hierarchies(&mut self, path: &std::path::Path) {
        let side_car = crate::multires::read_hierarchies(&crate::multires::sidecar_for(path));
        for fault in &side_car.faults {
            self.hierarchies_lost.push(match fault {
                crate::multires::SideCarFault::Unreadable(e) => {
                    format!("o arquivo de hierarquias não pôde ser lido: {e}")
                }
                crate::multires::SideCarFault::UnknownFormat => {
                    "o arquivo de hierarquias está num formato desconhecido".to_string()
                }
                crate::multires::SideCarFault::Truncated { read } => format!(
                    "o arquivo de hierarquias termina no meio de um registro, depois de {read}"
                ),
            });
        }
        for record in side_car.records {
            let Some(layer) = self.layers.get_mut(record.position) else {
                self.hierarchies_lost
                    .push(format!("linha {}", record.position + 1));
                continue;
            };
            match claycore::Multires::deserialize(&record.bytes) {
                Ok(surface) => {
                    layer.representation = Representation::Multires;
                    layer.multires = Some(crate::multires::Hierarchy::holding(surface));
                }
                Err(e) => {
                    eprintln!(
                        "a hierarquia da camada {:?} não pôde ser reconstruída: {e}",
                        layer.name
                    );
                    self.hierarchies_lost.push(layer.name.clone());
                }
            }
        }
        // A row that the document says is a mesh layer, that the side-car did
        // not mention, and that this session has no hierarchy for, is a mesh
        // layer. That is the honest answer and it is also the silent one, so
        // the only rows named as lost are the ones a record was found for and
        // could not be honoured — a side-car that is missing altogether says
        // nothing here, because nothing in the file distinguishes a document
        // that never held a hierarchy from one whose side-car went missing.
    }

    /// Releases every hierarchy's rebuildable level caches.
    ///
    /// The host's answer to memory pressure for this representation. A
    /// hierarchy's levels are cached per level and the caches are *derived*:
    /// the engine reproduces the surface bit-identically when it rebuilds one,
    /// so releasing them costs time and no work. `MemoryDiagnostics`'s
    /// `rebuildable` is the figure this acts on.
    ///
    /// It is here rather than on a timer because dropping a cache the next dab
    /// will rebuild is a cost with nothing to buy — this is for the moment the
    /// operating system says there is no more, which this application does not
    /// yet listen for. Until it does, the caller is the regression test that
    /// proves a dab still lands across one: the release **moves the numbering
    /// the level's weld classes are in**, which is exactly the case ClayCore
    /// v0.78.0 fixed and describes as not a crash — the stamp writing into
    /// released storage, the level rebuilt from the authoritative detail
    /// before it was read back, and the dab simply not there.
    pub fn release_hierarchy_caches(&mut self) -> Result<(), ModelError> {
        for layer in &mut self.layers {
            let Some(hierarchy) = layer.multires.as_mut() else {
                continue;
            };
            hierarchy
                .surface_mut()
                .drop_inactive_caches()
                .map_err(ModelError::engine)?;
        }
        Ok(())
    }

    /// What the hierarchies cost this session, and which of them were lost.
    pub fn multires_diagnostics(&self) -> clayspace_model::MultiresDiagnostics {
        clayspace_model::MultiresDiagnostics {
            held: self
                .layers
                .iter()
                .filter(|layer| layer.multires.is_some())
                .count(),
            lost: self.hierarchies_lost.clone(),
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
        // An undone crossing leaves an emptied layer in the engine that the
        // scene does not show, because the engine holds its filling on the
        // redo stack and removing it would be an undo step of its own. A file
        // has no redo stack, so what cannot be redone should not be written:
        // these go before the write, and the history they cost is history the
        // save is not preserving anyway.
        for layer in std::mem::take(&mut self.suppressed) {
            let _ = self.document.remove_layer(layer);
        }
        self.crossing_redo.clear();
        // A solo is a way of looking at the document, not part of it. Written
        // as it stands, the file would reopen with everything but one subtool
        // hidden — and so would the crash recovery, which is the copy nobody
        // gets to check before trusting it. So the real pattern goes down and
        // the solo is put back around the write.
        match self.solo.clone() {
            Some(solo) => self.with_visibility(&solo.was, |doc| {
                doc.document.save(path).map_err(ModelError::engine)
            })?,
            None => self.document.save(path).map_err(ModelError::engine)?,
        }
        // The object table, beside it. A failure to write it is reported and
        // does not fail the save: the sculpture is in the `.clay` and losing
        // the bookkeeping is not losing the work. It would be a poor trade to
        // tell a sculptor their document did not save because a side-car
        // could not be written.
        let sidecar = crate::objects::sidecar_for(path);
        if let Err(e) = crate::objects::write_table(&sidecar, &self.objects) {
            eprintln!("os objetos colocados não puderam ser registrados em {sidecar:?}: {e}");
        }
        // The hierarchies, beside it — and this one **fails the save**.
        //
        // Read that against the four lines above it, because the two look like
        // the same thing and are not. The object table is bookkeeping: the
        // sculpture is in the `.clay` and losing which of its shapes can be
        // picked up again is not losing the work, so a failure there is
        // reported and swallowed. Here the side-car *is* the work. A
        // `.clayspace` carries a hierarchy's cage and nothing standing on it —
        // the C header says so in writing — so a save whose side-car did not
        // land has written a file that looks complete and holds a flat sheet
        // where a sculpt was. Telling a sculptor their document did not save
        // is much the lesser harm.
        self.write_hierarchies(path)?;
        Ok(())
    }

    fn open(&mut self, path: &std::path::Path) -> Result<(), OpenError> {
        // Built completely before anything here is touched. A failed open must
        // leave the sculptor's work exactly as it was — losing it to a
        // mistyped filename would be the worst bug this application could
        // have.
        let mut opened = Self::from_file(path, self.policy.clone())?;
        // Objects the document itself cannot describe. Read after the document
        // rather than during it, because a missing or unreadable side-car is
        // not a failed open: the sculpture is all there, and what is lost is
        // which of its shapes can be picked up again.
        opened.objects = crate::objects::read_table(&crate::objects::sidecar_for(path));
        // The hierarchies are *not* overlaid here, unlike the objects. They
        // are applied inside `from_file`, before the passes that measure each
        // layer, because a hierarchy changes what the row **is**: the engine
        // reports its layer as a mesh layer, so until the side-car is read
        // there is nothing anywhere that knows the row was ever a hierarchy,
        // and every pass that asks a layer what representation it is would
        // have got the wrong answer.
        opened.object_states.clear();
        opened.remember_objects_after();
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
                    // Read as off rather than as a fresh layer's X.
                    //
                    // The ABI sets a layer mirror and has no call that reads
                    // one back, and the file keeps whatever was saved — so
                    // neither of these can be made true here by asking.
                    // Writing the fresh-layer default over what was loaded
                    // would be worse than assuming: the mirror is a property
                    // evaluation reads, so a layer saved unmirrored would come
                    // back mirrored and the form itself would change on
                    // reopen. Off is what a reopened document has always
                    // recorded, and the first stroke that wants otherwise
                    // writes through and makes both true.
                    symmetry: [false; 3],
                    // Where it stands, read back rather than assumed.
                    //
                    // Until ABI 0.74.0 there was no call that answered this,
                    // so every layer came back believing it stood at the
                    // origin unturned and unscaled. On a field layer the
                    // engine still evaluated the tape where the layer really
                    // was, so the form was drawn correctly and everything the
                    // host derives from the placement was wrong: the
                    // whole-subtool manipulator sat in empty space, a mirrored
                    // dab reflected through the world's plane instead of the
                    // layer's, and a mask painted in world coordinates missed
                    // the cells it was meant to protect. On a carried mesh or
                    // a grid, where the *host* applies the placement, the
                    // subtool itself came back at the origin.
                    transform: placement_of(&document, *id).unwrap_or_default(),
                    // A layer that was never named comes back empty rather
                    // than absent, and an unnamed row in the stack is worse to
                    // work with than a numbered one.
                    ..Layer::new(
                        *id,
                        LayerKey(index as u64 + 1),
                        &engine_name,
                        representation,
                    )
                }
            })
            .collect();

        let cache = BrickCache::new(Self::BRICK_CONFIG).map_err(|e| unreadable(e.to_string()))?;

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
            maintenance: crate::maintenance::Maintenance::new(),
            live_generation: 0,
            live_smooth: None,
            live_move: None,
            live_move_armed: false,
            live_opening_entries: 0,
            live_gesture: None,
            surface_epoch: 0,
            meshed_chunks: 0,
            surface_brick_count: 0,
            mesh_sculptors: std::cell::RefCell::default(),
            picked_seed: std::cell::Cell::default(),
            mesh_undo: Vec::new(),
            mesh_redo: Vec::new(),
            crossing_undo: Vec::new(),
            rebuilds: Vec::new(),
            crossing_redo: Vec::new(),
            suppressed: std::collections::HashSet::new(),
            retired: std::collections::HashMap::new(),
            hierarchies_lost: Vec::new(),
            solo: None,
            visibility_undo: Vec::new(),
            visibility_redo: Vec::new(),
            redo_room: 0,
            curve: None,
            live_hook: None,
            lattice: None,
            voxel_display: VoxelDisplay::default(),
            voxel_blur: SmoothBlur::default(),
            voxel_smooth: std::collections::BTreeMap::new(),
            cage_revision: 0,
            mask_revision: 0,
            combine: CombineSettings::for_strokes(),
            colour: clayspace_model::ColourState::default(),
            alpha: None,
            voxel_grab: None,
            recording_pass: false,
            next_key,
            skin: SkinSettings::default(),
            objects: Vec::new(),
            selected_object: None,
            dragging: None,
            object_states: std::collections::BTreeMap::new(),
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

        // The hierarchies, before anything below asks a layer what it is.
        //
        // This is the one side-car that changes a row's representation rather
        // than decorating it, so it cannot be overlaid after the fact the way
        // the object table is: `clay_document_layer_info` reports a
        // hierarchy's layer as a **mesh** layer, because that is what it is —
        // it holds the cage — and there is no `LayerRepresentation` value for
        // a hierarchy at all. So until this runs, nothing anywhere knows the
        // row was ever one.
        model.read_hierarchies(path);

        // And where every carried mesh's triangles are, for the same reason:
        // that box is cached on the layer and a layer rebuilt from a file
        // starts without one, so a reopened mesh subtool would report no
        // extent and take a manipulator sized to a default. A hierarchy is
        // measured from the level it draws rather than from the cage its layer
        // holds, which is the other reason the side-car has to have been read
        // by now.
        let carried: Vec<(LayerKey, Representation)> = model
            .layers
            .iter()
            .filter(|layer| {
                matches!(
                    layer.representation,
                    Representation::Mesh | Representation::Multires
                )
            })
            .map(|layer| (layer.key, layer.representation))
            .collect();
        for (key, representation) in carried {
            match representation {
                Representation::Multires => model.refresh_multires_bounds(key),
                _ => model.refresh_mesh_bounds(key),
            }
        }

        // Every rig the document carries, each onto the subtool that holds it.
        // Before ClayCore 0.29.0 a placed armature was write-only, so a
        // reopened document held the skinned surface and nothing that could
        // pose it (#77). Recovering all of them rather than the first is what
        // makes two rigs survive a reopen: the record is per layer now, so a
        // second one no longer overwrites the first.
        let mut first_rig = None;
        for (index, id) in ids.into_iter().enumerate() {
            let Some((node, tree)) = Self::recover_armature(&model.document, id) else {
                continue;
            };
            model.layers[index].armature_bounds = Some(Self::armature_bounds(&tree, model.skin));
            // One node, which is the whole rig: since ClayCore 0.30.0 the
            // signs travel with it, so there are no separate cutter items
            // left behind for a reader to miss (#99).
            model.layers[index].armature = Some((vec![node], tree));
            first_rig = first_rig.or(Some(index));
        }
        // And the first rigged subtool becomes the active one.
        //
        // `armature()` answers only for the active subtool — deliberately, so
        // switching subtools cannot hand the next click someone else's rig —
        // so reopening a document that holds a rig should put you on one.
        if let Some(index) = first_rig {
            model.active = index;
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

        // Made unique against the stack, because importing the same file twice
        // is a thing sculptors do and two layers sharing a name shadow one
        // another's grid once either is crossed to voxels (ClayCore #365).
        let name = self.unique_layer_name(
            &path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Importado".to_string()),
        );

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
        //
        // A hierarchy contributes its **cage**, which is what its layer holds,
        // and not the level it is drawn at. That is a known gap rather than an
        // oversight: keeping the layer's triangles in step with the display
        // level would mean a wholesale geometry replacement — one engine undo
        // entry each — on every gesture or every save, and either would put a
        // document edit inside something a sculptor did not ask to be an edit.
        // The route that exports a sculpt is the crossing that bakes a level
        // out to a mesh, which is one step and says what it gives up.
        let mesh = self
            .document
            .mesh_combined(params)
            .map_err(ModelError::engine)?;
        mesh.save(path).map_err(ModelError::engine)
    }

    fn has_mesh_layers(&self) -> bool {
        // A hierarchy's row counts: its layer *is* a mesh layer — it holds the
        // cage — so it is one of the layers `mesh_combined` reaches and one of
        // the things this question is asked in order to decide about.
        self.layers.iter().any(|layer| {
            matches!(
                layer.representation,
                Representation::Mesh | Representation::Multires
            )
        })
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
            // This is the call that gives a mesh row its triangles, so it is
            // where the row becomes sculptable. `add_mesh_layer` records a row
            // with none, and the mesh verbs are unavailable on it until this
            // has run — which is why this overrides what `Layer::new` assumes
            // of a mesh row.
            carries_geometry: true,
            // Recorded as a mesh so the tools reach it by representation
            // rather than by a special case. A mesh layer is not evaluated,
            // and nothing here pretends otherwise.
            ..Layer::new(id, key, name, Representation::Mesh)
        });
        // Where the triangles are, which is the only account of a carried
        // mesh's extent there is — the engine answers `clay_layer_bounds` from
        // a layer's SDF content and this layer has none.
        self.refresh_mesh_bounds(key);
        // The imported form is the one the sculptor just asked for, so it is
        // the one they are working on: the spec says an inserted subtool
        // "arrives selected". Through the one activation call rather than by
        // assigning the index, so everything activation owes — arming the mesh
        // sculptor among it — is owed here too.
        self.set_active_layer(key)?;
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

    /// What the active grid's resolution levels have, coarsest first: the
    /// chunks each has storage for, and whether it covers its parent whole.
    ///
    /// The companion to `occupied_cells`, and the read that makes
    /// [`clayspace_model::LayerOperation::RefineRegion`] checkable at all: an
    /// unrefined chunk reads its parent, so a level pushed over a region holds
    /// the *same solid* as one pushed everywhere and both report the same
    /// occupied count. Both numbers are needed — a whole level allocates its
    /// chunks lazily, so a fresh one is as cheap as a region and only the flag
    /// tells them apart, while the flag alone would not notice a "region" that
    /// had been widened to cover the grid.
    pub fn level_storage(&mut self) -> Option<Vec<(usize, bool)>> {
        let layer = self.active_layer();
        if layer.representation != Representation::Voxel {
            return None;
        }
        let engine_name = layer.engine_name.clone();
        let (_, grid) = self.document.voxel_layer(&engine_name).ok()?;
        (0..grid.level_count().ok()?)
            .map(|level| {
                Some((
                    grid.level_chunk_count(level).ok()?,
                    grid.level_is_whole(level).ok()?,
                ))
            })
            .collect()
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
        let index = self.active;
        let engine_name = self.layers[index].engine_name.clone();
        // A literal name is a collision waiting for the second extrusion, and a
        // collision shadows a grid (ClayCore #365) — so this goes through the
        // same derivation every other layer-creating route does.
        let name = self.unique_layer_name("Extrusão");
        // The grid and the layer's own mask out of one borrow, which is what
        // `voxel_layer_masked` exists for: the grid takes the document
        // exclusively, and the mask is inside the same document.
        let extruded = {
            let claycore::MaskedGrid { grid, mask, .. } = self
                .document
                .voxel_layer_masked(&engine_name)
                .map_err(ModelError::engine)?;
            let mask = mask.expect("checked by the caller");
            grid.mask_extrude(&mask, extrude_params(settings))
                .map_err(ModelError::engine)?
        };
        let id = self
            .document
            .voxel_to_layer(&extruded, &name, 0)
            .map_err(ModelError::engine)?;
        let key = self.adopt_engine_layer(id, &name, Representation::Sdf)?;
        self.after_conversion(key)?;
        self.stay_on_the_masked_subtool(index)
    }

    /// Puts the sculptor back on the subtool they were masking.
    ///
    /// An extrusion arrives as a row of its own, and creating a row activates
    /// it — which, now that a mask belongs to the subtool it was painted on,
    /// would make the mask look consumed: the panel would be reading the
    /// mask of the shell that was just made, and there is none. The promise is
    /// the opposite one, and it is the reason Extrudar reads the mask rather
    /// than taking it: an extrusion you do not like is thrown away without
    /// painting the mask again.
    fn stay_on_the_masked_subtool(&mut self, index: usize) -> Result<(), ModelError> {
        let key = self.layers[index].key;
        self.set_active_layer(key)
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

impl CurveModel for ClayDocument {
    fn curve(&self) -> CurveState {
        let Some(curve) = self.curve.as_ref() else {
            return CurveState::default();
        };
        CurveState {
            active: true,
            points: curve.points.clone(),
            selection: curve.selection.clone(),
            join: curve.join,
            profile: curve.profile,
        }
    }

    fn begin_curve(&mut self) {
        self.cancel_curve();
        self.curve = Some(Curve {
            layer: self.active_layer().key,
            points: Vec::new(),
            selection: Vec::new(),
            join: CurveJoin::default(),
            profile: CurveProfile::default(),
            node: None,
        });
    }

    fn add_curve_point(&mut self, at: [f32; 3], radius: f32) -> Result<(), ModelError> {
        let Some(curve) = self.curve.as_mut() else {
            return Ok(());
        };
        curve.points.push(CurvePoint {
            position: at,
            radius: radius.max(1e-3),
        });
        // The new point is the one in hand: a person who just placed it is
        // most likely to want to move it.
        curve.selection = vec![curve.points.len() - 1];
        self.reshape_curve()
    }

    fn select_curve_point(&mut self, index: Option<usize>) {
        let Some(curve) = self.curve.as_mut() else {
            return;
        };
        let count = curve.points.len();
        curve.selection = index.filter(|at| *at < count).into_iter().collect();
    }

    fn toggle_curve_point(&mut self, index: usize) {
        let Some(curve) = self.curve.as_mut() else {
            return;
        };
        if index >= curve.points.len() {
            return;
        }
        match curve.selection.binary_search(&index) {
            Ok(at) => {
                curve.selection.remove(at);
            }
            Err(at) => curve.selection.insert(at, index),
        }
    }

    fn drag_curve(&mut self, by: [f32; 3]) -> Result<(), ModelError> {
        let Some(curve) = self.curve.as_mut() else {
            return Ok(());
        };
        for index in curve.selection.clone() {
            let Some(point) = curve.points.get_mut(index) else {
                continue;
            };
            for (at, step) in point.position.iter_mut().zip(by) {
                *at += step;
            }
        }
        self.reshape_curve()
    }

    fn drag_curve_points(
        &mut self,
        drag: clayspace_model::GizmoDrag,
        to: [f32; 3],
        snap: bool,
    ) -> Result<(), ModelError> {
        let Some(curve) = self.curve.as_mut() else {
            return Ok(());
        };
        // Each point mapped through the same arithmetic the cage uses, which
        // is what makes a turn about the selection's middle mean the same
        // thing on a curve as on a cage — and what gets a curve turn and scale
        // without a second implementation of either.
        for index in curve.selection.clone() {
            let Some(point) = curve.points.get_mut(index) else {
                continue;
            };
            point.position = drag.apply(point.position, to, snap);
        }
        self.reshape_curve()
    }

    fn set_curve_radius(&mut self, radius: f32) -> Result<(), ModelError> {
        let Some(curve) = self.curve.as_mut() else {
            return Ok(());
        };
        let radius = radius.max(1e-3);
        // The selection where there is one, and the whole curve where there is
        // not: setting a thickness with nothing picked means the tube, not
        // nothing.
        if curve.selection.is_empty() {
            for point in curve.points.iter_mut() {
                point.radius = radius;
            }
        } else {
            for index in curve.selection.clone() {
                if let Some(point) = curve.points.get_mut(index) {
                    point.radius = radius;
                }
            }
        }
        self.reshape_curve()
    }

    fn set_curve_join(&mut self, join: CurveJoin) -> Result<(), ModelError> {
        if let Some(curve) = self.curve.as_mut() {
            curve.join = join;
        }
        self.reshape_curve()
    }

    fn set_curve_profile(&mut self, profile: CurveProfile) -> Result<(), ModelError> {
        // The profile is the item's, not the guide's, so this cannot be a
        // point-list replace — the sweep is placed again from scratch.
        self.retire_curve_node()?;
        if let Some(curve) = self.curve.as_mut() {
            curve.profile = profile;
        }
        self.reshape_curve()
    }

    fn remove_curve_points(&mut self) -> Result<(), ModelError> {
        let Some(curve) = self.curve.as_mut() else {
            return Ok(());
        };
        if curve.selection.is_empty() {
            return Ok(());
        }
        let doomed = std::mem::take(&mut curve.selection);
        for index in doomed.iter().rev() {
            if *index < curve.points.len() {
                curve.points.remove(*index);
            }
        }
        // A guide below two points has nothing to sweep along, and the engine
        // refuses to cut one there rather than ignoring it. Taking the sweep
        // down is the honest answer: the curve is still being placed.
        if curve.points.len() < clayspace_model::FEWEST_POINTS {
            self.retire_curve_node()?;
            return Ok(());
        }
        self.reshape_curve()
    }

    fn apply_curve(&mut self) -> Result<(), ModelError> {
        // The sweep is already placed; applying is letting go of the curve
        // that shaped it. What stays behind is an ordinary item in the layer.
        self.curve = None;
        Ok(())
    }

    fn cancel_curve(&mut self) {
        if let Err(e) = self.retire_curve_node() {
            eprintln!("a curva não pôde ser removida: {e}");
        }
        self.curve = None;
    }
}

impl ClayDocument {
    /// Places the sweep, or replaces the guide of the one already placed.
    ///
    /// Replacing rather than adding, for the reason a snakehook gesture grows
    /// one tendril: a curve edited by dragging a point would otherwise leave a
    /// sweep behind on every move.
    fn reshape_curve(&mut self) -> Result<(), ModelError> {
        let Some(curve) = self.curve.as_ref() else {
            return Ok(());
        };
        if curve.points.len() < clayspace_model::FEWEST_POINTS {
            return Ok(());
        }
        let index = self.index_of(curve.layer)?;
        let layer = self.layers[index].id;
        let guide = curve.guide();
        let kind = point_type(curve.join);

        if let Some(node) = curve.node {
            self.document
                .set_layer_stroke_points(layer, node, &guide, kind, Self::CURVE_TOLERANCE)
                .map_err(ModelError::engine)?;
            return self.refill(layer, &[node]);
        }

        let mut item = self.curve_item(curve, &guide, kind)?;
        item.set_op(Op::Add).map_err(ModelError::engine)?;

        let node = self
            .document
            .add_item(layer, &item)
            .map_err(ModelError::engine)?;
        if let Some(curve) = self.curve.as_mut() {
            curve.node = Some(node);
        }
        self.refill(layer, &[node])
    }

    /// The item a curve sweeps, which is a different primitive depending on
    /// the section asked for.
    ///
    /// A **round** tube is a swept-sphere chain — `CLAY_PRIM_STROKE`, the
    /// snakehook's primitive — because that one takes a radius *per point* and
    /// so tapers along its whole length, which is what a sculptor drags a
    /// thickness for.
    ///
    /// Any other section is `CLAY_PRIM_SWEPT`, which carries a real profile
    /// along the guide. Measured, that primitive **ignores the guide's
    /// per-point radius entirely** — a tube swept with radii of 0.05, 0.15 and
    /// 0.4 reached 2.901 every time, the unit circle's size — because its
    /// thickness comes from the profile parameters instead. So the thickness
    /// there is the *first* point's at one end and the *last* point's at the
    /// other, interpolated between: the engine needs two or more profiles and
    /// spreads them evenly along the guide, which is a taper and not a radius
    /// per point.
    fn curve_item(
        &self,
        curve: &Curve,
        guide: &[f32],
        kind: claycore::PointType,
    ) -> Result<claycore::Item, ModelError> {
        if curve.profile == CurveProfile::Circle {
            let mut item = claycore::Item::stroke().map_err(ModelError::engine)?;
            item.set_curve_points(guide, kind)
                .map_err(ModelError::engine)?;
            // The chain's own smoothing, so consecutive spans meet without a
            // crease where the radius steps.
            let thinnest = curve
                .points
                .iter()
                .map(|point| point.radius)
                .fold(f32::MAX, f32::min);
            item.set_stroke_blend_k(thinnest * 0.5)
                .map_err(ModelError::engine)?;
            return Ok(item);
        }

        let (profile, mut params) = profile_of(curve.profile);
        let mut item = claycore::Item::swept(0.0).map_err(ModelError::engine)?;
        // Two, because the engine refuses fewer — "a loft or sweep needs two
        // or more profiles" — and because two is what a taper is.
        for radius in [
            curve.points.first().map_or(0.1, |point| point.radius),
            curve.points.last().map_or(0.1, |point| point.radius),
        ] {
            for value in params.iter_mut() {
                *value = radius;
            }
            item.add_profile(profile, &params[..profile_params(profile)])
                .map_err(ModelError::engine)?;
        }
        item.set_curve_points(guide, kind)
            .map_err(ModelError::engine)?;
        Ok(item)
    }

    /// Takes the placed sweep back out, leaving the curve's points alone.
    fn retire_curve_node(&mut self) -> Result<(), ModelError> {
        let Some(curve) = self.curve.as_mut() else {
            return Ok(());
        };
        let Some(node) = curve.node.take() else {
            return Ok(());
        };
        let key = curve.layer;
        let index = self.index_of(key)?;
        let layer = self.layers[index].id;
        // The region *before* the removal, because after it the layer no
        // longer reaches where the tube was and marking its extent would leave
        // those bricks holding a sweep that is gone. The same reason removing
        // a layer captures its bounds first.
        let vacated = self.document.layer_bounds(layer).ok().flatten();
        self.document
            .remove_node(layer, node)
            .map_err(ModelError::engine)?;
        match vacated {
            Some((min, max)) => {
                // Padded, because a brick the surface only grazes still holds
                // a piece of it and a box drawn exactly on the bounds can miss
                // the outermost one.
                let pad = self.cache.config().voxel_size * Self::BRICK_MARGIN;
                self.refill(layer, &[])?;
                self.refill_region(
                    std::array::from_fn(|i| min[i] - pad),
                    std::array::from_fn(|i| max[i] + pad),
                )
            }
            None => self.refill(layer, &[]),
        }
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

    fn select_lattice_points(&mut self, indices: &[usize]) {
        let Some(cage) = self.lattice.as_mut() else {
            return;
        };
        let count = cage.point_count();
        let mut selection: Vec<usize> = indices.iter().copied().filter(|at| *at < count).collect();
        // Kept sorted and without repeats, as a Shift-click's selection is:
        // `is_selected` is a search rather than a scan, and the pivot is the
        // same wherever the box was drawn from.
        selection.sort_unstable();
        selection.dedup();
        cage.selection = selection;
        self.cage_revision = self.cage_revision.wrapping_add(1);
    }

    fn set_gizmo_mode(&mut self, mode: GizmoMode) {
        if let Some(cage) = self.lattice.as_mut() {
            cage.mode = mode;
        }
        self.cage_revision = self.cage_revision.wrapping_add(1);
    }

    fn begin_gizmo_drag(&mut self, handle: GizmoHandle, anchor: [f32; 3], view_axis: [f32; 3]) {
        let state = self.lattice();
        let Some(drag) = state.drag_from(handle, anchor, view_axis) else {
            return;
        };
        let Some(cage) = self.lattice.as_mut() else {
            return;
        };
        let held = cage.selection.iter().map(|at| cage.position(*at)).collect();
        cage.dragging = Some((drag, held));
    }

    fn drag_gizmo(&mut self, to: [f32; 3], snap: bool) -> Result<(), ModelError> {
        let Some(cage) = self.lattice.as_mut() else {
            return Ok(());
        };
        let Some((drag, held)) = cage.dragging.as_ref() else {
            return Ok(());
        };
        let (drag, held) = (*drag, held.clone());
        for (at, was) in cage.selection.clone().iter().zip(held) {
            let now = drag.apply(was, to, snap);
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
            // A cage dragged and then dragged back to exactly where it started
            // is the identity *and* had a preview up, so this is a gesture
            // ending like any other. It used to leave `previewing` set, which
            // now means it would also leave the maintenance gate shut and the
            // memory pin held for the rest of the session.
            self.set_previewing(false);
            self.settle_between_strokes();
            return Ok(());
        }
        let laid = match cage.representation {
            // The preview is taken back and the cage laid down once more, this
            // time banked. Not "keep what is on screen": a preview holds the
            // deltas of one pass, and turning that into the edit would leave
            // the undo stack describing a gesture rather than a deformation.
            Representation::Mesh => {
                self.set_previewing(false);
                self.bend_mesh(&cage)
            }
            _ => self.bend_field(&cage),
        };
        // A cage drag is a gesture like any other, and this is where it ends.
        self.settle_between_strokes();
        laid
    }

    fn cancel_lattice(&mut self) {
        self.cage_revision = self.cage_revision.wrapping_add(1);
        // Whatever the preview is showing goes with the cage. Abandoning one
        // and leaving the form bent would be the opposite of what Esc means
        // everywhere else here.
        self.discard_cage_preview();
        self.set_previewing(false);
        self.lattice = None;
        self.settle_between_strokes();
    }
}

impl ClayDocument {
    /// Takes down a cage that belongs to a subtool other than `incoming`.
    ///
    /// A cage is a transient authoring gesture, not per-subtool state: it is
    /// sized to what one form contains, and that box means nothing around
    /// another. So it does not travel. The sculptor is asked to apply or drop
    /// it before the switch is dispatched — `LatticeViewModel` holds that
    /// question — and this is the floor under that: a cage that reaches the
    /// switch unresolved is dropped, preview and all, rather than reappearing
    /// around a form it was never fitted to.
    fn drop_a_foreign_cage(&mut self, incoming: LayerKey) {
        if self
            .lattice
            .as_ref()
            .is_some_and(|cage| cage.layer != incoming)
        {
            self.cancel_lattice();
        }
    }

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
        // Through the one cache both this and `layer_bounds` read, so the cage
        // and the whole-subtool manipulator cannot come to different answers
        // about how big the form is or where it stands.
        let key = self.active_layer().key;
        self.refresh_mesh_bounds(key);
        SceneModel::layer_bounds(self, key)
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

        let lattice = match self.carried_placement(cage.layer) {
            Some(transform) => Self::carried_cage_lattice(cage, &transform)?,
            None => Self::cage_lattice(cage)?,
        };

        let Some(sculptor) = self.sculptor_for(cage.layer) else {
            return Ok(());
        };
        let mut live = LiveMesh::new(
            cage.layer,
            sculptor.clone(),
            claycore::MeshDeltas::new().map_err(ModelError::engine)?,
        );
        // What the last preview did, taken back before the cage is laid down
        // again from the mesh as it was. The lattice is *absolute* — offsets
        // from rest, evaluated against the original vertices — so applying it
        // over a surface a previous preview already bent would compound the
        // deformation on every pointer move.
        let mut previous = self
            .live_mesh
            .take()
            .filter(|held| held.layer == cage.layer);
        let moved = {
            let mut sculptor = sculptor.borrow_mut();
            if let Some(previous) = &mut previous {
                previous
                    .deltas()
                    .revert(&mut sculptor)
                    .map_err(ModelError::engine)?;
            }
            // Not deferred, and deliberately: the cage is one whole-mesh call
            // per pointer move, so there is a single recompute either way and
            // deferring would buy a stale shading for nothing. The record is
            // still held beside the handle, so if that ever changes the flush
            // is already guaranteed.
            sculptor
                .apply_lattice(&lattice, Some(live.deltas()))
                .map_err(ModelError::engine)?
        };
        // A cage moves every vertex, which the engine names as the case a
        // rebuild is the right call after rather than a refit. Asked for
        // rather than done: every pointer move comes through here.
        self.request_index_rebuild(cage.layer);
        if self.previewing {
            // Held rather than banked. The cage is still up and every drag
            // replaces the last, so bending a form is one undo however many
            // times the sculptor adjusted a corner on the way.
            if moved > 0 {
                self.live_mesh = Some(live);
            }
            // What tells the viewport to look again: a mesh layer is not in
            // the brick cache, so nothing else about this edit would.
            self.live_generation = self.live_generation.wrapping_add(1);
        } else if moved > 0 {
            let engine_depth = self.engine_undo_depth();
            let (layer, deltas) = live.finish();
            self.mesh_undo.push(MeshGesture {
                layer,
                what: GestureRecord::Deltas(deltas),
                engine_depth,
            });
            self.mesh_redo.clear();
        }
        self.refresh_mesh_bounds(cage.layer);
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

    /// The same cage, written in a moved mesh subtool's own coordinates.
    ///
    /// The sculptor drags the cage's corners where the form is *drawn*, and
    /// the sculptor's vertices are where the engine holds them — see
    /// [`ClayDocument::carried_placement`]. So the box is carried back and each
    /// control point is given what the drawn cage would displace the point it
    /// stands over by, turned and scaled the same way back.
    ///
    /// Exact for a subtool that was moved or resized, which is what the
    /// whole-subtool manipulator writes most of the time. A *turned* one is
    /// resampled onto the enclosing axis-aligned box, since a lattice takes one
    /// — the error is the same order as the preview's own, and the alternative
    /// is a cage that misses the vertices altogether.
    fn carried_cage_lattice(
        cage: &Cage,
        transform: &clayspace_model::Transform,
    ) -> Result<claycore::MeshLattice, ModelError> {
        let drawn = Self::cage_lattice(cage)?;
        let (min, max) = Self::box_through((cage.min, cage.max), |point| {
            Self::into_local(transform, point)
        });
        let mut carried =
            claycore::MeshLattice::new(min, max, cage.divisions).map_err(ModelError::engine)?;
        let [nx, ny, _] = cage.divisions.map(|n| n as usize);
        for at in 0..cage.point_count() {
            let coordinate = [
                (at % nx) as i32,
                ((at / nx) % ny) as i32,
                (at / (nx * ny)) as i32,
            ];
            let rest = carried.position(coordinate).map_err(ModelError::engine)?;
            let moved = drawn
                .displacement(Self::into_world(transform, rest))
                .map_err(ModelError::engine)?;
            if moved.iter().all(|axis| *axis == 0.0) {
                continue;
            }
            // Turned back and divided component by component, because the
            // frame this displacement is being carried into may stretch each
            // axis by a different amount — the same division
            // `Transform::into_local` makes, on a vector rather than a point,
            // and now the same call the three pick paths make.
            carried
                .set_offset(coordinate, Self::direction_into_local(transform, moved))
                .map_err(ModelError::engine)?;
        }
        Ok(carried)
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
            self.set_previewing(true);
            if let Err(e) = self.bend_mesh(&cage) {
                eprintln!("a gaiola não pôde ser pré-visualizada: {e}");
            }
        }
        self.lattice = Some(cage);
    }

    /// Takes back whatever a preview is showing, leaving the form as it was.
    fn discard_cage_preview(&mut self) {
        let Some(mut live) = self.live_mesh.take() else {
            return;
        };
        // Settled before the revert, for the reason the stroke path settles
        // before its own: what the record puts back has to include whatever
        // the flush recomputed, or the offsets come off and the shading does
        // not.
        if let Err(e) = live.settle() {
            eprintln!("as normais adiadas não puderam ser recalculadas: {e}");
        }
        let reverted = {
            // Against the sculptor for the layer the preview was laid on,
            // which the gesture carries itself: the single slot this replaced
            // was whatever had last been built, so a preview outliving a
            // switch reverted its offsets into another subtool's vertices.
            let sculptor = live.sculptor.clone();
            let mut sculptor = sculptor.borrow_mut();
            live.deltas().revert(&mut sculptor)
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
        // Said rather than discovered. `clay_layer_lattice_gizmo` returns no
        // warps at all for a layer carrying a per-axis scale — a cage records
        // its item-to-cage placement as a rigid transform, and on a squashed
        // layer the map it needs is a general affine one, so placing a cage
        // through the narrower record would warp every item in a space it does
        // not occupy. The engine refuses rather than approximating, and
        // without this the refusal would arrive as "the cage reached nothing
        // in this layer", which is true and tells the sculptor nothing they
        // can act on.
        if !self.layers[index].transform.is_uniformly_scaled() {
            return Err(ModelError::engine(
                "a gaiola não deforma um subtool esticado por eixo: \
                 volte a escala aos três fatores iguais para usá-la",
            ));
        }
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
        match self.active_mask() {
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
        let layer = self.active_layer().id;
        // Cleared rather than dropped: the mask belongs to the layer inside
        // the document, which has no verb for taking one away. An empty mask
        // freezes nothing, which is what Limpar means, and `mask_state`
        // reports it as absent so the panel closes exactly as it did.
        if matches!(op, MaskOp::Clear) {
            if let Some(mut mask) = self.document.layer_mask_mut(layer) {
                mask.clear().map_err(ModelError::engine)?;
            }
            return Ok(());
        }

        // Refused where nothing is frozen, which is the same refusal as before
        // and now has to be spelled out: a document-owned mask stays attached
        // once it exists, so "carries a mask" and "freezes something" are no
        // longer the same question, and every one of these operations is about
        // a region there has to be.
        if !self.mask_state().is_active() {
            return Err(ModelError::engine("não há máscara para editar"));
        }
        let Some(mut mask) = self.document.layer_mask_mut(layer) else {
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
        let index = self.active;
        let source = index;
        self.a_mask_worth_extruding()?;

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
            // A hierarchy has no field either, and the route out is the same
            // one a mesh takes.
            Representation::Multires => {
                return Err(ModelError::engine(
                    "uma hierarquia de subdivisão não tem campo para extrudar; \
                     converta um nível para malha e depois para SDF",
                ))
            }
            Representation::Sdf => {}
        }

        let layer = self.layers[index].id;
        // Named rather than handed over: the extrusion holds the document
        // mutably, and the mask is one of that document's own. See
        // `claycore::MaskSource`.
        let item = self
            .document
            .mask_extrude(
                layer,
                claycore::MaskSource::Layer(layer),
                extrude_params(settings),
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
        self.stay_on_the_masked_subtool(source)
    }

    /// Freezes what a shape drawn over the form encloses — ZBrush's mask
    /// lasso and mask rect, which arrive here as the same thing.
    ///
    /// The region is a prism: the outline swept straight along the view
    /// direction, through the subtool and out the other side, so the far
    /// surface freezes with the near one exactly as ZBrush's does. What bounds
    /// the sweep is the subtool's own extent — a mask lattice is unbounded, and
    /// the engine's advice for every bounded mask operation is that the caller
    /// supplies the finite region, "from a grid's bounds or an item's".
    ///
    /// Delivered as a **stroke** rather than as cells — one per connected
    /// piece of the region, which is almost always one. A document-owned mask
    /// snapshots itself for the undo history on every call that writes to it,
    /// so the region has to arrive in as few of them as it can: see
    /// [`clayspace_model::outline`], where the path that visits it is built.
    fn apply_outline(&mut self, outline: &MaskOutline) -> Result<(), ModelError> {
        if !outline.encloses_anything() {
            return Err(ModelError::engine(
                "o laço não fechou uma região; arraste em volta do que quer congelar",
            ));
        }
        let key = self.active_layer().key;
        let Some(bounds) = SceneModel::layer_bounds(self, key) else {
            return Err(ModelError::engine(
                "esta subferramenta não tem extensão para o laço percorrer",
            ));
        };
        // Grown by the footprint, because `clay_layer_bounds` is deliberately
        // tight: a surface sitting exactly on the box would be swept along its
        // face and freeze on one side of the cell only.
        let margin = Self::VOXEL_SIZE * 2.0;
        let bounds = (
            std::array::from_fn(|axis| bounds.0[axis] - margin),
            std::array::from_fn(|axis| bounds.1[axis] + margin),
        );

        // Refused rather than started, where the region runs to hundreds of
        // millions of cells. What an outline costs is the volume it sweeps and
        // the pitch cannot trade that away, so the only honest answers are a
        // reason and something to do about it.
        if clayspace_model::cells_to_write(outline, bounds, Self::VOXEL_SIZE)
            > clayspace_model::CELL_CEILING
        {
            return Err(ModelError::engine(
                "a região do laço é grande demais para congelar de uma vez; \
                 desenhe um laço menor",
            ));
        }

        let spacing = clayspace_model::lattice_pitch(Self::VOXEL_SIZE);
        let Some(paths) = clayspace_model::coverage_path(outline, bounds, spacing) else {
            // An outline drawn beside the form rather than over it. Not a
            // refusal: the sculptor missed, and the mask is what it was.
            return Ok(());
        };

        // One stroke per connected piece of the region, and a group around
        // them where there is more than one: an outline can enclose two pieces
        // with nothing between them, a single path across both would freeze
        // the gap, and a sculptor undoing a gesture means the gesture.
        let grouped = paths.len() > 1;
        if grouped {
            self.document
                .begin_undo_group()
                .map_err(ModelError::engine)?;
        }
        let painted = self.freeze_along(paths, outline.mode.target(), spacing);
        if grouped {
            self.document.end_undo_group().map_err(ModelError::engine)?;
        }
        let painted = painted?;

        // As a mask stroke does: nothing in the surface moved, so no brick is
        // dirty and the viewport would keep drawing the region as it was.
        if painted > 0 {
            self.mask_revision = self.mask_revision.wrapping_add(1);
        }
        Ok(())
    }
}

/// The stamp run a drawn region is painted with.
///
/// Written out rather than taken from the brush in hand: none of it is a
/// sculptor's choice. The step along the path is the lattice pitch, and every
/// shaping field is off — a jittered or tapered run would be an outline that
/// is not the one drawn.
///
/// The footprint reaches half the pitch's **diagonal** rather than half its
/// side, and the difference is what the region looks like. A brush footprint is
/// axis-aligned in the world; the lattice is aligned to the *camera*, because
/// that is where the outline was drawn. Sized to half the pitch, the two tile
/// only when the camera happens to face down an axis, and from anywhere else
/// the region comes out speckled with cells no stamp reached — visible as
/// white flecks all over the frozen patch.
///
/// A ball rather than a cube, and it is worth 40% of the gesture: a cube of the
/// same reach writes 5.8 cells for every cell of the region against a ball's
/// 2.7, all of the difference in corners that overshoot it. Measured on the
/// reference form, an outline around the whole of it: 1191 ms against 800 on
/// the same machine, and `mask.outline` takes the figure on a quiet one.
fn outline_preset(spacing: f32) -> StrokePreset {
    StrokePreset {
        // √3/2, with a little to spare against the arc-length walk landing its
        // stamps a fraction off the lattice it was built from.
        radius: spacing * 0.9,
        // Spacing is a fraction of the diameter, and the diameter is the pitch.
        spacing: 1.0,
        strength: 1.0,
        pressure_size: 0.0,
        pressure_strength: 0.0,
        pressure_curve: 1.0,
        jitter_position: 0.0,
        jitter_size: 0.0,
        jitter_rotation: 0.0,
        seed: 0,
        rotate_along_stroke: false,
        taper_start: 0.0,
        taper_end: 0.0,
        steady: 0.0,
        // The path crosses itself where it backtracks, and a region frozen
        // twice must not be frozen harder than one frozen once.
        accumulation: claycore::Accumulation::Clamped,
    }
}

impl ArmatureModel for ClayDocument {
    fn armature(&self) -> Option<Armature> {
        // The active subtool's own rig and no other. A document may carry one
        // per layer, and a click edits the one belonging to what is being
        // worked on — an armature on the subtool beside it is not.
        let (_, tree) = self.active_layer().armature.as_ref()?;
        Some(tree.clone())
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

        // And with symmetry off, unlike every other new subtool.
        //
        // A rig does its own mirroring: `add_zsphere` places the reflected
        // node itself, because the host holds the topology and the tree has to
        // carry both halves for either to be posable. A layer mirror would
        // reflect the placed item *as well*, so a stroke on the rig's own
        // subtool at the fresh-subtool default would hang a second left arm
        // off the first one.
        self.set_symmetry([false; 3])?;

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
        let index = self.index_of(key)?;
        self.layers[index].armature = Some((placed?, tree));
        Ok(())
    }

    fn add_zsphere(
        &mut self,
        parent: NodeIndex,
        position: [f32; 3],
        radius: f32,
        mirrored: bool,
    ) -> Result<NodeIndex, ModelError> {
        let Some((_, tree)) = self.layers[self.active].armature.as_mut() else {
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
                if let Some((_, tree)) = self.layers[self.active].armature.as_mut() {
                    tree.add_child(mirror_parent, reflected, radius);
                }
            }
        }

        self.rewrite_armature()?;
        Ok(index)
    }

    fn move_zsphere(&mut self, index: NodeIndex, delta: [f32; 3]) -> Result<(), ModelError> {
        let Some((_, tree)) = self.layers[self.active].armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        tree.move_subtree(index, delta);
        self.rewrite_armature()
    }

    fn resize_zsphere(&mut self, index: NodeIndex, radius: f32) -> Result<(), ModelError> {
        let Some((_, tree)) = self.layers[self.active].armature.as_mut() else {
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
        let Some((_, tree)) = self.layers[self.active].armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        tree.reparent(index, new_parent)?;
        self.rewrite_armature()
    }

    fn remove_zsphere(&mut self, index: NodeIndex) -> Result<(), ModelError> {
        let Some((_, tree)) = self.layers[self.active].armature.as_mut() else {
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
        let Some((_, tree)) = self.layers[self.active].armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        let inserted = tree
            .insert_on_link(child)
            .ok_or_else(|| ModelError::engine("essa esfera não tem ligação"))?;
        self.rewrite_armature()?;
        Ok(inserted)
    }

    fn set_zsphere_negative(&mut self, index: NodeIndex, negative: bool) -> Result<(), ModelError> {
        let Some((_, tree)) = self.layers[self.active].armature.as_mut() else {
            return Err(ModelError::engine("não há armadura nesta camada"));
        };
        tree.set_negative(index, negative)?;
        self.rewrite_armature()
    }

    fn set_skin(&mut self, skin: SkinSettings) -> Result<(), ModelError> {
        self.skin = skin;
        if self.active_layer().armature.is_some() {
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
        // vacated box has to cover when a rig is rewritten. On the rig's own
        // layer, because that is where the rig is.
        let bounds = Self::armature_bounds(tree, self.skin);
        if let Some(row) = self.layers.iter_mut().find(|row| row.id == layer) {
            row.armature_bounds = Some(bounds);
        }
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
            // An undone crossing's layer is still in the engine — the engine
            // holds its filling on the redo stack — but it is not in the
            // scene until the crossing is put back.
            if self.suppressed.contains(id) {
                kept.remove(id);
                continue;
            }
            // A layer this side removed and history has brought back: the
            // engine restores it with the id it had, and everything the host
            // knows about it — its key, its mask, its mirror, where it stands,
            // its meshed chunks — is only recoverable from the record kept when
            // it left. Rebuilt from the document instead, a restored operand
            // came back under a new `LayerKey`, so the mask and the symmetry
            // were gone, every `PlacedObject` row was still filed under a key
            // that no longer existed, and the whole-subtool manipulator drew at
            // the origin while the engine held the real transform.
            if let Some(mut known) = kept.remove(id).or_else(|| {
                self.retired.remove(id).inspect(|back| {
                    // Coming back is the one path where a key that is still
                    // ours has geometry we did not watch arrive: the engine
                    // rebuilds the layer's mesh from the redo stack, and a
                    // sculptor held over the old one would answer "the mesh
                    // this sculptor was built over is no longer in its
                    // document" for the rest of the session.
                    self.mesh_sculptors.borrow_mut().forget(back.key);
                })
            }) {
                // Visibility is the one fact about a surviving layer that
                // history moves under it. `SetLayerVisibleCmd` is journaled
                // like everything else and the engine reverts it exactly, but
                // it cannot tell this side that it did — so the eye in the
                // stack sat where the command left it while the surface showed
                // the layer undo had brought back.
                if let Ok(info) = self.document.layer_info(*id) {
                    known.visible = info.visible;
                }
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
            let representation = match info.map(|i| i.representation) {
                Some(claycore::LayerRepresentation::Voxel) => Representation::Voxel,
                Some(claycore::LayerRepresentation::Mesh) => Representation::Mesh,
                _ => Representation::Sdf,
            };
            let key = self.take_key();
            rebuilt.push(Layer {
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
                // A layer that comes back is one a redo rebuilt, so anything
                // it carried has to be read out of the document again. The rig
                // is the only part the document can answer for — a mask and a
                // mirror are host state, and a redone creation starts them
                // where a fresh layer starts them.
                armature: Self::recover_armature(&self.document, *id)
                    .map(|(node, tree)| (vec![node], tree)),
                ..Layer::new(*id, key, &name, representation)
            });
        }

        self.layers = rebuilt;
        // Whatever the document no longer holds joins the record above rather
        // than being dropped: a step forward over the removal takes it away
        // again, and a step back brings it once more to the same layer it was.
        self.retired.extend(kept);
        // A sculptor outlives an undo that left its layer alone — which is
        // what keeps taking back a mesh stroke from paying the weld again —
        // but not one that took the layer out of the scene.
        let standing: Vec<LayerKey> = self.layers.iter().map(|layer| layer.key).collect();
        self.mesh_sculptors
            .borrow_mut()
            .retain(|key| standing.contains(&key));
        // The layer that was active, if it is still there; otherwise the last
        // one, which is where a removal leaves you in every panel of this kind.
        self.active = active_id
            .and_then(|id| self.layers.iter().position(|layer| layer.id == id))
            .unwrap_or_else(|| self.layers.len().saturating_sub(1));
        // A mesh layer the engine has just put back carries triangles and no
        // record of where they are, since that box is the host's.
        let unmeasured: Vec<LayerKey> = self
            .layers
            .iter()
            .filter(|layer| {
                layer.representation == Representation::Mesh && layer.mesh_bounds.is_none()
            })
            .map(|layer| layer.key)
            .collect();
        for key in unmeasured {
            self.refresh_mesh_bounds(key);
        }
        // And a mesh layer whose triangles history has just *replaced* — an
        // undone or redone rebuild — keeps its box and its key and changes
        // underneath both. Only the engine's revision says so. See
        // `settle_geometry_revisions`.
        self.settle_geometry_revisions();
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
    ///
    /// Every layer that carries one, because a document may carry several: a
    /// history step reaches whichever rig its edit belonged to, and re-reading
    /// only the active subtool's would leave the others describing shapes the
    /// engine no longer holds. Layers that carry none are skipped, so the cost
    /// is one probe per rig rather than one per layer.
    fn resync_armature(&mut self) {
        let rigged: Vec<(usize, LayerId)> = self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.armature.is_some())
            .map(|(index, layer)| (index, layer.id))
            .collect();
        for (index, layer) in rigged {
            // Where the rig was before history moved it. Refilling the layer
            // alone is not enough: a rig that shrank leaves surface outside its
            // new bounds, and nothing marks those bricks — the same debt a
            // rewrite pays with `refill_region`.
            let vacated = self.layers[index].armature_bounds;
            match Self::recover_armature(&self.document, layer) {
                Some((node, tree)) => {
                    self.layers[index].armature_bounds =
                        Some(Self::armature_bounds(&tree, self.skin));
                    self.layers[index].armature = Some((vec![node], tree));
                }
                // Undone past the rig's own creation: there is no armature now,
                // and saying so is what stops the next click editing a ghost.
                None => {
                    self.layers[index].armature = None;
                    self.layers[index].armature_bounds = None;
                }
            }
            if let Some((min, max)) = vacated {
                if let Err(e) = self.refill_region(min, max) {
                    // Not fatal: the geometry is stale rather than wrong, and
                    // the next edit or settle clears it. Worth saying, though.
                    eprintln!("a região da armadura não pôde ser remalhada: {e}");
                }
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
        let index = self.active;
        let Some((nodes, tree)) = self.layers[index].armature.take() else {
            return Ok(());
        };
        let layer = self.layers[index].id;
        // Where it was, before it is replaced by where it now is.
        let vacated = self.layers[index].armature_bounds;

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
        self.layers[index].armature = Some((fresh, tree));

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
        let (_, tree) = self.active_layer().armature.as_ref()?;
        let node = tree.get(index)?;
        let target = Armature::mirrored_position(node.position)?;
        tree.nodes
            .iter()
            .position(|other| (0..3).all(|axis| (other.position[axis] - target[axis]).abs() < 1e-4))
            .map(|i| i as NodeIndex)
    }
}

// -- placed objects ---------------------------------------------------------

impl ClayDocument {
    /// Where a node reaches, as the cache needs to know it.
    ///
    /// `None` means no finite box exists and the whole layer is what has to be
    /// refilled — which an ordinary shape placed with `Intersect` reaches,
    /// since the engine drops the bound for a non-local op anywhere in the
    /// subtree.
    fn node_bound(&self, layer: LayerId, node: NodeId) -> Option<([f32; 3], [f32; 3])> {
        match self.document.node_influence_bound(layer, node) {
            Ok(claycore::Influence::Box { min, max }) => Some((min, max)),
            // A node with nothing to dirty contributes nothing to a union, so
            // it answers as an empty box at the origin rather than as "the
            // whole layer". This is the *dirtying* question only — see
            // `node_centre`, which must not treat the origin as an answer.
            Ok(claycore::Influence::Nothing) => Some(([0.0; 3], [0.0; 3])),
            Ok(claycore::Influence::Everything) | Err(_) => None,
        }
    }

    /// Refills what an object edit reached, or the layer where it reached too
    /// far to say.
    /// Writes a layer's transform to the engine, each factor floored so a form
    /// can never be scaled to nothing.
    ///
    /// The per-axis call for every layer transform and not only for a
    /// stretched one, which is the same rule the object path already follows
    /// and for the same reason: the ABI does no partial updates, so each of
    /// the two setters writes the *whole* transform and the uniform one
    /// collapses a squash rather than leaving it alone. One call for both
    /// means a subtool cannot be quietly unsquashed by being moved. A uniform
    /// triple costs nothing — the engine is explicit that it keeps the field
    /// exact and compiles identical tape.
    fn write_layer_transform(
        &mut self,
        id: LayerId,
        transform: clayspace_model::Transform,
    ) -> Result<(), ModelError> {
        self.document
            .set_layer_transform_nonuniform(
                id,
                transform.position,
                transform.rotation_axis,
                transform.rotation_angle,
                std::array::from_fn(|axis| transform.scale[axis].max(1e-4)),
            )
            .map_err(ModelError::engine)
    }

    fn refill_bound(
        &mut self,
        layer: LayerId,
        bound: Option<([f32; 3], [f32; 3])>,
    ) -> Result<(), ModelError> {
        match bound {
            Some((min, max)) => self.refill_region(min, max),
            None => self.refill(layer, &[]),
        }
    }

    fn object_index(&self, id: ObjectId) -> Option<usize> {
        self.objects.iter().position(|object| object.id() == id)
    }

    /// Records the table on both sides of an edit, so history can find it.
    ///
    /// Before and after, because undoing across an object edit lands on the
    /// depth the edit started from and redoing lands on the one it ended at.
    fn remember_objects_before(&mut self) {
        let depth = self.engine_undo_depth();
        self.object_states.insert(depth, self.objects.clone());
    }

    fn remember_objects_after(&mut self) {
        let depth = self.engine_undo_depth();
        self.object_states.insert(depth, self.objects.clone());
    }

    /// Brings the table back to what it was at the engine's current depth.
    ///
    /// Called after an undo or a redo has moved the engine. A depth nothing
    /// recorded leaves the table alone, which is right: the entry that moved
    /// was not an object edit.
    /// Brings the cached layer transforms back to what the engine holds.
    ///
    /// **This used to be a snapshot table.** Every route that placed a layer
    /// recorded the whole stack against the engine's undo depth, and this
    /// looked the depth up again after a step and copied the row back — six
    /// call sites and a `BTreeMap` whose only purpose was to reconstruct an
    /// answer nobody could ask for. `clay_document_layer_transform_nonuniform`
    /// asks for it. So the engine is the one account of where a layer stands
    /// and this reads it, which also means a placement made by any route the
    /// application does not know about is followed rather than overwritten.
    ///
    /// A layer the engine will not answer for is left alone. That is the same
    /// judgement the table made for a depth it had not recorded: the cache is
    /// a copy of an answer, and a copy of no answer is worse than the last
    /// good one.
    fn resync_layer_transforms(&mut self) {
        for index in 0..self.layers.len() {
            let id = self.layers[index].id;
            if let Some(standing) = self.engine_layer_transform(id) {
                self.layers[index].transform = standing;
            }
        }
    }

    /// Where the engine says a layer stands.
    fn engine_layer_transform(&self, id: LayerId) -> Option<clayspace_model::Transform> {
        placement_of(&self.document, id)
    }

    fn resync_objects(&mut self) {
        let depth = self.engine_undo_depth();
        if let Some(table) = self.object_states.get(&depth) {
            self.objects = table.clone();
            // A selection outlives the nodes in it — but not the ones history
            // has taken away.
            if self
                .selected_object
                .is_some_and(|id| self.object_index(id).is_none())
            {
                self.selected_object = None;
            }
        }
    }

    /// Puts a whole layer somewhere, and remembers where.
    ///
    /// One engine call, so one undo step however many items the layer holds.
    fn place_layer(
        &mut self,
        key: LayerKey,
        transform: clayspace_model::Transform,
    ) -> Result<(), ModelError> {
        let index = self.index_of(key)?;
        let id = self.layers[index].id;
        // Where it was, before it stops being there. Refilling only where the
        // layer now stands left the surface it used to make standing where it
        // had been: the arrow was dragged, nothing moved, and the next stroke
        // re-meshed a handful of bricks around the pointer into a second form
        // with holes in it beside the first. The object path had learnt the
        // same lesson (`set_object_transform`); this is the layer's turn.
        let before = self.layer_bounds(key);
        let previous = self.layers[index].transform;
        self.write_layer_transform(id, transform)?;
        self.layers[index].transform = transform;
        let after = self.layer_bounds(key);
        // Both extents; the whole layer where either is unknown, because a
        // layer transform moves everything the layer holds.
        if let Err(refused) = self.refill_bound(id, union(before, after)) {
            // The cache would not track the region — a subtool scaled past
            // what it can hold — so the field must not stay where the picture
            // cannot follow it. Put the transform back and say why; the
            // manipulator keeps following the hand and the clay stays at the
            // last size the cache accepted.
            self.write_layer_transform(id, previous)?;
            self.layers[index].transform = previous;
            return Err(refused);
        }
        Ok(())
    }

    /// Where a carried mesh layer stands, when that is anywhere but the
    /// origin.
    ///
    /// A mesh layer is *carried* rather than evaluated: its triangles are the
    /// engine's own vertex arrays, and `clay_document_set_layer_transform`
    /// moves what the tape evaluates — which for this layer is nothing.
    /// Measured: a mesh subtool moved five units along X drew its first vertex
    /// at exactly where it drew it before. So the transform the whole-subtool
    /// manipulator writes reaches a mesh only if the host applies it, and every
    /// crossing between world space and those vertices goes through this and
    /// the two conversions below — otherwise a subtool would be drawn in one
    /// place and sculpted in another.
    ///
    /// `None` where the layer stands at the origin unturned and unscaled, which
    /// is every mesh subtool until one is dragged: the conversion is then the
    /// identity and skipping it keeps the common path free.
    fn carried_placement(&self, key: LayerKey) -> Option<clayspace_model::Transform> {
        let index = self.index_of(key).ok()?;
        let transform = self.layers[index].transform;
        (transform != clayspace_model::Transform::default()).then_some(transform)
    }

    /// Where the *active* layer's content stands against the world, when that
    /// is anywhere but where the layer holds it.
    ///
    /// All three representations now, and the same answer for each: the
    /// transform moves an SDF layer's tape, the host moves a carried mesh's
    /// vertices and a grid's cells, so on any of them a world point is carried
    /// in before it can address the layer's own content. It answered "nowhere"
    /// on a grid while a grid could not be moved, which was right then and
    /// would put a mask off its cells now.
    fn active_content_placement(&self) -> Option<clayspace_model::Transform> {
        self.carried_placement(self.active_layer().key)
    }

    /// A point of a carried layer, standing where the layer transform puts it.
    fn into_world(transform: &clayspace_model::Transform, point: [f32; 3]) -> [f32; 3] {
        transform.into_world(point)
    }

    /// The way back: a world point in the layer's own coordinates.
    fn into_local(transform: &clayspace_model::Transform, point: [f32; 3]) -> [f32; 3] {
        transform.into_local(point)
    }

    /// The same for a direction: turned back and divided, without a position.
    ///
    /// The three pick paths all want this and all had the rotation alone,
    /// which was the whole of it while a layer transform took one factor. A
    /// ray divided on its origin and not on its bearing points somewhere else
    /// in a stretched frame, and a pick that misses reads exactly like a
    /// pointer that is not over the form.
    fn direction_into_local(
        transform: &clayspace_model::Transform,
        direction: [f32; 3],
    ) -> [f32; 3] {
        transform.direction_into_local(direction)
    }

    /// The box that holds a box once every corner of it has been moved.
    ///
    /// All eight corners, because a turned box is not axis aligned and taking
    /// the two named corners alone would report a smaller box than the form
    /// occupies.
    fn box_through((min, max): Bounds, map: impl Fn([f32; 3]) -> [f32; 3]) -> Bounds {
        let mut held_min = [f32::MAX; 3];
        let mut held_max = [f32::MIN; 3];
        for corner in 0..8 {
            let point = map(std::array::from_fn(|axis| {
                if corner >> axis & 1 == 0 {
                    min[axis]
                } else {
                    max[axis]
                }
            }));
            for axis in 0..3 {
                held_min[axis] = held_min[axis].min(point[axis]);
                held_max[axis] = held_max[axis].max(point[axis]);
            }
        }
        (held_min, held_max)
    }

    /// A carried mesh's own box, standing where the layer transform puts it.
    fn placed_box(transform: &clayspace_model::Transform, bounds: Bounds) -> Bounds {
        Self::box_through(bounds, |point| Self::into_world(transform, point))
    }

    /// Re-reads a mesh layer's own box after something moved its vertices.
    ///
    /// Every mesh edit lands in the sculptor's vertex arrays and nothing else
    /// notices, so the cache `layer_bounds` answers from is only right if the
    /// edits say so. Cheap: one read of the layer's positions.
    fn refresh_mesh_bounds(&mut self, key: LayerKey) {
        let Ok(index) = self.index_of(key) else {
            return;
        };
        if self.layers[index].representation != Representation::Mesh {
            return;
        }
        let engine_name = self.layers[index].engine_name.clone();
        let measured =
            self.document
                .read_mesh_layer(&engine_name)
                .ok()
                .and_then(|(positions, ..)| {
                    let first = *positions.first()?;
                    Some(positions.iter().fold((first, first), |(min, max), point| {
                        (
                            std::array::from_fn(|axis| min[axis].min(point[axis])),
                            std::array::from_fn(|axis| max[axis].max(point[axis])),
                        )
                    }))
                });
        self.layers[index].mesh_bounds = measured;
    }

    /// The same question for a hierarchy, answered from what is *drawn*.
    ///
    /// Not from the cage. A hierarchy's layer holds the cage and the sculpt
    /// stands off it — that is the whole of the representation — so the box a
    /// manipulator sizes itself to and Frame All frames is the display level's,
    /// and the cage's would be the box the form had before anybody touched it.
    /// Written into the same field a mesh layer's box lives in, because it is
    /// the same question about the same row.
    fn refresh_multires_bounds(&mut self, key: LayerKey) {
        let Ok(index) = self.index_of(key) else {
            return;
        };
        let Some(hierarchy) = self.layers[index].multires.as_mut() else {
            return;
        };
        let measured = hierarchy.bounds();
        self.layers[index].mesh_bounds = measured;
    }

    /// The engine half of a placement: the item, then where it goes.
    ///
    /// Split out so the undo group around it has one thing to bracket and one
    /// place to fail.
    fn place_item(
        &mut self,
        layer: LayerId,
        shape: Shape,
        parameters: &[f32],
        at: [f32; 3],
        combine: CombineSettings,
    ) -> Result<NodeId, ModelError> {
        let mut item =
            claycore::Item::of(primitive_of(shape, parameters)).map_err(ModelError::engine)?;
        item.set_op(engine_op(combine.op))
            .map_err(ModelError::engine)?;
        item.set_blend(engine_blend(combine.blend), combine.radius)
            .map_err(ModelError::engine)?;
        let node = self
            .document
            .add_item(layer, &item)
            .map_err(ModelError::engine)?;
        // Placed through the node transform rather than by building the item
        // at `at`: an item's creation position and its node transform are the
        // same slot, so everything about where an object stands goes through
        // one call — the one the manipulator drives.
        //
        // An earlier version of this comment claimed the engine mishandles
        // undo across the two, which it does not. Checked directly: an item
        // built at 0.9, retransformed to -0.5 and undone once comes back to
        // 0.9. The symptom that suggested otherwise was ours — an object's
        // position was being read from the node's influence bound, which under
        // the layer mirror covers the reflection too and centres between the
        // pair.
        self.document
            .set_node_transform(layer, node, at, [0.0, 1.0, 0.0], 0.0, 1.0)
            .map_err(ModelError::engine)?;
        Ok(node)
    }

    /// What a copied subtool is called, before the number that makes it unique.
    const COPY_SUFFIX: &'static str = "cópia";

    /// A name no other layer in the document is using.
    ///
    /// A voxel layer's grid is reachable only by name — the ABI has no
    /// id-addressed accessor, and the lookup answers with the first layer in
    /// stack order carrying it (ClayCore #365) — so two layers sharing a name
    /// shadow one another's grid and a stroke lands on the wrong one. The
    /// rename path already refuses a collision for that reason; an insertion
    /// cannot refuse, because the sculptor never typed the name, so it derives
    /// one instead.
    ///
    /// Every representation, not only voxels: a sculptor who inserts three
    /// spheres and then crosses one to a grid would otherwise create the
    /// collision after the fact.
    fn unique_layer_name(&self, base: &str) -> String {
        let base = if base.trim().is_empty() {
            "Subtool"
        } else {
            base.trim()
        };
        if !self.layer_name_taken(base) {
            return base.to_string();
        }
        // From two, because the first one carries the bare name: "Esfera",
        // "Esfera 2", "Esfera 3" is how a sculptor counts them.
        let mut ordinal = 2_u32;
        loop {
            let candidate = format!("{base} {ordinal}");
            if !self.layer_name_taken(&candidate) {
                return candidate;
            }
            ordinal += 1;
        }
    }

    /// Both names a layer answers to: the one shown and the one a grid is
    /// fetched with. They agree unless a rename was refused halfway.
    fn layer_name_taken(&self, candidate: &str) -> bool {
        self.layers
            .iter()
            .any(|layer| layer.name == candidate || layer.engine_name == candidate)
    }

    /// Samples one subtool alone into a volume item.
    ///
    /// `clay_item_volume_from_document` samples the *whole* document's field,
    /// and the engine's contract is that a hidden layer "contributes nothing to
    /// the field; showing it again restores the original field exactly" — so
    /// baking one subtool alone is hiding the others around the bake, which is
    /// what [`ClayDocument::with_only_visible`] is for. Every exit path
    /// restores, including the one where the bake refuses.
    ///
    /// Public because the subtool boolean bakes each of its operands exactly
    /// this way; a copy is the same operation with one operand.
    pub fn bake_subtool(&mut self, key: LayerKey, cell: f32) -> Result<Item, ModelError> {
        let cell = cell.max(1e-4);
        let (min, max) = self.operand_bounds(key).ok_or_else(|| {
            ModelError::engine("esta camada não tem extensão para copiar; está vazia")
        })?;
        // Padded by the band, because the box is where the field is *sampled*
        // and a surface lying exactly on a face has no room for the band the
        // mesher needs on both sides of it.
        let band = Self::feather_for(cell);
        let min: [f32; 3] = std::array::from_fn(|axis| min[axis] - band);
        let max: [f32; 3] = std::array::from_fn(|axis| max[axis] + band);
        // Through the operand's three routes rather than straight to the
        // document sampler. A *grid* is not one of the sampler's: measured,
        // `clay_item_volume_from_document` over a document whose only shown
        // layer is a voxel one refuses with "invalid argument (empty
        // document)", so every attempt to copy a grid was a hard refusal from a
        // control that offered it.
        self.bake_operand(key, cell, (min, max))
    }

    /// The same, sampled over a region the caller chose.
    ///
    /// The boolean bakes both of its operands over one region — the pair's
    /// box, padded by the band — rather than each over its own. Two reasons,
    /// and neither is that a volume item goes wrong outside its lattice: it
    /// reads as *outside* there, which is measured in
    /// `an_intersection_with_a_grid_keeps_only_what_both_hold` and is what
    /// makes a grid operand, which has no region to be given, work at all.
    ///
    /// The first is that both halves of the result then sit on the *same*
    /// lattice — same origin, same cell — so the surfaces they carry meet
    /// cell-for-cell at the join rather than beating against each other at
    /// half a cell of phase. The second is that a cost is stated for one
    /// region before the operation runs, and sampling two of a different size
    /// would be pricing something other than what was done.
    fn bake_subtool_over(
        &mut self,
        key: LayerKey,
        cell: f32,
        min: [f32; 3],
        max: [f32; 3],
    ) -> Result<Item, ModelError> {
        let cell = cell.max(1e-4);
        self.with_only_visible(&[key], |doc| {
            doc.document
                .volume_from_region(
                    VolumeParams {
                        cell_size: Some(cell),
                        // No feather: the volume is added into a layer of its
                        // own with `Op::Add`, and the engine ignores the
                        // feather for every op but replace — the same bargain
                        // an imported mesh makes.
                        ..Default::default()
                    },
                    min,
                    max,
                )
                .map_err(ModelError::engine)
        })
    }

    /// Creates a subtool and fills it, as one thing the sculptor asked for.
    ///
    /// The bracket every insertion shares. Creating the layer and putting the
    /// form in it are two engine edits, and without the group one step back
    /// takes the form away and leaves an empty subtool standing — the same
    /// shape `place_object` already brackets for.
    ///
    /// The layer is the active one when this returns, because
    /// [`ClayDocument::adopt_engine_layer`] routes through the one activation
    /// call: a subtool arrives selected, which is what makes the next dab land
    /// on it.
    fn insert_subtool(
        &mut self,
        name: &str,
        fill: impl FnOnce(&mut Self, LayerId) -> Result<NodeId, ModelError>,
    ) -> Result<(LayerKey, LayerId, NodeId), ModelError> {
        self.remember_objects_before();
        self.document
            .begin_undo_group()
            .map_err(ModelError::engine)?;
        let made = (|| -> Result<(LayerKey, LayerId, NodeId), ModelError> {
            let id = self
                .document
                .add_sdf_layer(name)
                .map_err(ModelError::engine)?;
            let key = self.adopt_engine_layer(id, name, Representation::Sdf)?;
            let node = fill(self, id)?;
            Ok((key, id, node))
        })();
        // Closed on the failing path too: a group left open swallows every
        // edit after it into one undo step.
        let closed = self.document.end_undo_group().map_err(ModelError::engine);
        let made = made?;
        closed?;
        Ok(made)
    }

    /// What every insertion owes once its subtool is standing.
    ///
    /// The whole layer rather than the node's bound: nothing about it was there
    /// before, which is the same reason a crossing's new layer is refilled
    /// whole.
    fn settle_subtool(&mut self, layer: LayerId) -> Result<(), ModelError> {
        self.remember_objects_after();
        self.refill(layer, &[])?;
        self.refresh_stats();
        Ok(())
    }

    /// Stands a whole subtool where the sculptor pointed.
    ///
    /// The *layer* moves and the form sits at its middle, rather than the form
    /// sitting off-centre in a layer that stays at the origin. That is what
    /// leaves the whole-subtool manipulator on the form it addresses:
    /// `GizmoTarget::Layer` reads the layer's transform, and a layer left at
    /// the origin would put the widget in empty space.
    ///
    /// Written through the engine directly rather than through `place_layer`,
    /// because this runs inside the insertion's undo group and `place_layer`
    /// would snapshot and refill in the middle of it.
    fn stand_subtool_at(&mut self, layer: LayerId, at: [f32; 3]) -> Result<(), ModelError> {
        // The per-axis call here too, for the reason `write_layer_transform`
        // gives: an insertion writes a whole transform, and the uniform setter
        // would be the one route that could unsquash a layer behind the
        // manipulator's back.
        self.document
            .set_layer_transform_nonuniform(layer, at, [0.0, 1.0, 0.0], 0.0, [1.0; 3])
            .map_err(ModelError::engine)?;
        if let Some(known) = self.layers.iter_mut().find(|known| known.id == layer) {
            known.transform = clayspace_model::Transform {
                position: at,
                rotation_axis: [0.0, 1.0, 0.0],
                rotation_angle: 0.0,
                scale: [1.0; 3],
            };
        }
        Ok(())
    }

    /// The active layer, when it is one an object can live in.
    fn layer_for_objects(&self) -> Result<(LayerKey, LayerId), ModelError> {
        let layer = self.active_layer();
        if layer.representation != Representation::Sdf {
            return Err(ModelError::Unavailable(
                clayspace_model::Unavailable::NoVerbHere {
                    active: layer.representation,
                    verbs: OBJECT_VERBS,
                    note: None,
                },
            ));
        }
        Ok((layer.key, layer.id))
    }
}

/// A box in world space, as every one of these calls hands one over.
type Bounds = ([f32; 3], [f32; 3]);

/// Resolving a boolean between two subtools.
///
/// The engine composes layers by hard union, so there is no live boolean
/// between two of them (ClayCore #321). What there is: a hidden layer
/// "contributes nothing to the field", and `clay_item_volume_from_document`
/// samples what is left — so each operand can be baked alone and the two
/// volumes combined in a subtool of their own.
impl ClayDocument {
    /// What the interface calls an operand.
    fn operand_name(&self, key: LayerKey) -> Result<String, ModelError> {
        let index = self.index_of(key)?;
        Ok(self.layers[index].name.clone())
    }

    /// Whether an operand may take part, and why not when it may not.
    ///
    /// Ghost and lock both refuse. A ghosted subtool is not pickable and a
    /// locked one is protected against editing; consuming either — or baking
    /// it into a result that then stands in for it — is the edit the
    /// protection was set to prevent.
    fn operand_is_free(&self, key: LayerKey) -> Result<(), ModelError> {
        let index = self.index_of(key)?;
        let layer = &self.layers[index];
        if layer.protection.is_editable() {
            return Ok(());
        }
        Err(ModelError::Boolean(BooleanRefusal::Protected {
            operand: layer.name.clone(),
            ghost: layer.protection.ghost,
        }))
    }

    /// The box an operand occupies, whatever it is made of.
    ///
    /// A mesh layer is carried rather than evaluated, so it has no SDF extent
    /// to report and its own triangles are the only account of where it is —
    /// the same measurement [`ObjectModel::mesh_operand_cost`] takes for the
    /// same reason.
    fn operand_bounds(&mut self, key: LayerKey) -> Option<Bounds> {
        let index = self.index_of(key).ok()?;
        // Re-measured first, because the cache the answer comes from is only
        // as fresh as the last edit that refreshed it and a boolean prices the
        // region it is about to sample.
        if self.layers[index].representation == Representation::Mesh {
            self.refresh_mesh_bounds(key);
        }
        SceneModel::layer_bounds(self, key)
    }

    /// The same, refusing by name where there is nothing there.
    fn operand_extent(&mut self, key: LayerKey) -> Result<Bounds, ModelError> {
        let operand = self.operand_name(key)?;
        self.operand_bounds(key)
            .ok_or(ModelError::Boolean(BooleanRefusal::Empty { operand }))
    }

    /// Both operands' boxes, once both are able to take part at all.
    fn boolean_extents(
        &mut self,
        base: LayerKey,
        tool: LayerKey,
    ) -> Result<(Bounds, Bounds), ModelError> {
        self.operand_is_free(base)?;
        self.operand_is_free(tool)?;
        Ok((self.operand_extent(base)?, self.operand_extent(tool)?))
    }

    /// The region both operands are sampled over: the pair's box, padded by
    /// the band. See [`ClayDocument::bake_subtool_over`] for why it is one
    /// region and not two.
    fn boolean_region(base: Bounds, tool: Bounds, cell: f32) -> Bounds {
        let band = Self::feather_for(cell);
        (
            std::array::from_fn(|axis| base.0[axis].min(tool.0[axis]) - band),
            std::array::from_fn(|axis| base.1[axis].max(tool.1[axis]) + band),
        )
    }

    /// Whether the two forms' boxes meet at all.
    ///
    /// A box test, which is what can be answered before anything is sampled:
    /// two boxes that do not touch hold two forms that certainly do not, which
    /// is the case the specification names — "two subtools standing apart".
    fn boxes_meet(base: Bounds, tool: Bounds) -> bool {
        (0..3).all(|axis| base.0[axis] <= tool.1[axis] && tool.0[axis] <= base.1[axis])
    }

    /// What the pair costs at this resolution.
    fn boolean_cost_over(region: Bounds, cell: f32) -> Cost {
        let extent: [f32; 3] =
            std::array::from_fn(|axis| (region.1[axis] - region.0[axis]).max(0.0));
        // The same crossing the conversion panel prices: a field sampled onto
        // a lattice. `MeshToSdf` and `SdfToVoxel` compute the same figures —
        // both choose a resolution and neither ends in a fixed topology — so
        // one direction prices a pair whatever the two are made of.
        Cost::of(Direction::SdfToVoxel, cell, extent)
    }

    /// Refuses a pair the document's memory budget will not hold.
    fn boolean_fits_the_budget(&self, region: Bounds, cell: f32) -> Result<(), ModelError> {
        let budget = self
            .cache
            .stats()
            .ok()
            .and_then(|stats| stats.memory_budget)
            .unwrap_or(u64::MAX);
        Self::boolean_cost_over(region, cell)
            .within(budget, Self::BYTES_PER_CELL)
            .map_err(|refusal| match refusal {
                Refusal::OverBudget {
                    cells,
                    budget_bytes,
                } => ModelError::Boolean(BooleanRefusal::OverBudget {
                    cells,
                    budget_bytes,
                }),
                other => ModelError::Conversion(other),
            })
    }

    /// The cell an operand is worked at.
    ///
    /// A grid says so itself; a field and a mesh are worked at the brick
    /// cache's cell, which is the resolution the rest of the application
    /// already samples at.
    fn operand_detail(&mut self, key: LayerKey) -> f32 {
        let working = self.cache.config().voxel_size;
        let Ok(index) = self.index_of(key) else {
            return working;
        };
        if self.layers[index].representation != Representation::Voxel {
            return working;
        }
        let engine_name = self.layers[index].engine_name.clone();
        self.document
            .voxel_layer(&engine_name)
            .ok()
            .and_then(|(_, grid)| grid.voxel_size().ok())
            .filter(|cell| *cell > 0.0)
            .unwrap_or(working)
    }

    /// One operand as a volume item, whatever it is made of.
    ///
    /// Three routes because there are three, and the specification says the
    /// crossing each one needs is "performed as part of the operation rather
    /// than demanded of the sculptor beforehand" — so this is where each is
    /// performed:
    ///
    /// - A **field** is sampled out of the document with the rest of the scene
    ///   hidden, over the pair's region.
    /// - A **grid** is read back through `clay_item_volume_from_voxels`, which
    ///   is what `clay_voxel_to_layer` does in a loop. It does not reach the
    ///   document's field at all — measured, sampling a document whose only
    ///   shown layer is a grid refuses with "empty document" — so the region
    ///   has nothing to say here and the grid's own cells are the extent.
    /// - A **mesh** takes the crossing `place_mesh_object` already pays, for
    ///   the same reason: a mesh layer is carried rather than evaluated.
    fn bake_operand(
        &mut self,
        key: LayerKey,
        cell: f32,
        region: Bounds,
    ) -> Result<Item, ModelError> {
        let index = self.index_of(key)?;
        let engine_name = self.layers[index].engine_name.clone();
        match self.layers[index].representation {
            Representation::Sdf => self.bake_subtool_over(key, cell, region.0, region.1),
            Representation::Voxel => {
                let (_, grid) = self
                    .document
                    .voxel_layer(&engine_name)
                    .map_err(ModelError::engine)?;
                // Index zero is every occupied cell as one item, which is what
                // a boolean wants: the palette carries colour and a boolean is
                // about form. The blur is the conversion panel's own default —
                // what an organic sculpt wants — so a grid read here and a grid
                // crossed through the panel come back the same shape.
                Item::volume_from_voxels(&grid, ConversionSettings::default().blur, 0)
                    .map_err(ModelError::engine)
            }
            Representation::Mesh => self
                .document
                .mesh_layer_as_volume(&engine_name, Self::bake_volume(cell))
                .map_err(ModelError::engine),
            // Refused rather than routed through the mesh arm beside it, and
            // the difference is the whole point: a hierarchy's *layer* holds
            // the cage, so `mesh_layer_as_volume` on it would sample the form
            // as it stood before anybody sculpted — a boolean against a
            // subtool the sculptor can see, using geometry they cannot. The
            // route that works is to bake a level out first, which is one
            // crossing and says what it costs.
            Representation::Multires => Err(ModelError::engine(
                "uma hierarquia de subdivisão não entra numa booleana; \
                 converta um nível para malha primeiro",
            )),
        }
    }

    /// What becomes of the operands once the result is standing.
    ///
    /// Hidden by default: keeping them is what makes the boolean recoverable,
    /// since the result is baked and the operands cannot be re-edited through
    /// it. Removed only where the sculptor said so, and either way inside the
    /// result's own undo group, so one step back takes the whole operation.
    fn retire_operands(
        &mut self,
        base: LayerKey,
        tool: LayerKey,
        consume: bool,
    ) -> Result<(), ModelError> {
        for operand in [base, tool] {
            if consume {
                SceneModel::remove_layer(self, operand)?;
            } else {
                SceneModel::set_layer_visible(self, operand, false)?;
            }
        }
        Ok(())
    }

    /// Bakes both operands and stands the result in a subtool of its own.
    ///
    /// The bakes happen before the undo group opens, exactly as a copy's does:
    /// the hide-and-restore around each sampling writes visibility commands of
    /// its own, and the sculptor's one step back has to reach the boolean
    /// rather than the flags the bake borrowed.
    fn build_boolean(
        &mut self,
        settings: BooleanSettings,
        pair: (LayerKey, LayerKey),
        region: Bounds,
        name: &str,
    ) -> Result<Inserted, ModelError> {
        let (base, tool) = pair;
        let cell = settings.cell_size;
        let mut first = self.bake_operand(base, cell, region)?;
        // What the result is made of, always. The operation is the *second*
        // operand's, which is the whole of what the sculptor chose.
        first.set_op(Op::Add).map_err(ModelError::engine)?;
        let mut second = self.bake_operand(tool, cell, region)?;
        second
            .set_op(engine_op(settings.op.combine()))
            .map_err(ModelError::engine)?;

        // Where a *carried* operand stands. `clay_item_volume_from_mesh` reads
        // the vertices the engine holds, and a mesh layer's own transform never
        // reaches those — see `carried_placement` — so a moved mesh operand
        // would otherwise be crossed in at the origin, cutting somewhere the
        // sculptor cannot see it.
        let placed = [base, tool].map(|operand| {
            self.index_of(operand)
                .ok()
                .filter(|index| self.layers[*index].representation == Representation::Mesh)
                .and_then(|_| self.carried_placement(operand))
        });

        let consume = settings.consume;
        let (key, layer, _) = self.insert_subtool(name, move |doc, layer| {
            let first_node = doc
                .document
                .add_item(layer, &first)
                .map_err(ModelError::engine)?;
            let node = doc
                .document
                .add_item(layer, &second)
                .map_err(ModelError::engine)?;
            for (item, transform) in [first_node, node].into_iter().zip(placed) {
                let Some(transform) = transform else {
                    continue;
                };
                // Per axis: this is a *layer's* placement being written onto
                // a node, and a layer's placement stretches since ABI 0.74.0.
                // The uniform setter would have rounded a squashed operand
                // back to round on the way into the result.
                doc.document
                    .set_node_transform_nonuniform(
                        layer,
                        item,
                        transform.position,
                        transform.rotation_axis,
                        transform.rotation_angle,
                        std::array::from_fn(|axis| transform.scale[axis].max(1e-4)),
                    )
                    .map_err(ModelError::engine)?;
            }
            // Inside the group, so hiding or consuming the operands is part of
            // the one thing the sculptor asked for.
            doc.retire_operands(base, tool, consume)?;
            Ok(node)
        })?;
        // No object row: what stands in the result is a pair of sampled
        // volumes rather than one of the offered shapes, so there is nothing
        // for the shape controls to measure. The subtool is the selection.
        self.selected_object = None;
        self.settle_subtool(layer)?;
        Ok(Inserted {
            layer: key,
            object: None,
        })
    }
}

impl ObjectModel for ClayDocument {
    fn objects(&mut self) -> Vec<clayspace_model::SceneObject> {
        let Ok((key, layer)) = self.layer_for_objects() else {
            return Vec::new();
        };
        // Listed from the table and filtered by the layer, rather than walked
        // from the layer and filtered by primitive. The primitive cannot tell
        // them apart: a stamping stroke deposits `Item::sphere` per stamp, so
        // walking a worked layer would offer a row per stamp — see
        // `objects::kind_of` for the decision this reverses.
        //
        // Filtered by what the layer still holds, so a node history has taken
        // away drops out of the list even before the table follows.
        let Ok(nodes) = self.document.layer_nodes(layer) else {
            return Vec::new();
        };
        self.objects
            .iter()
            .filter(|object| object.layer == key && nodes.contains(&object.node))
            .map(PlacedObject::presented)
            .collect()
    }

    fn selected_object(&self) -> Option<ObjectId> {
        self.selected_object
    }

    fn select_object(&mut self, id: Option<ObjectId>) {
        self.selected_object = id;
    }

    fn place_object(
        &mut self,
        shape: Shape,
        parameters: &[f32],
        at: [f32; 3],
        combine: CombineSettings,
    ) -> Result<ObjectId, ModelError> {
        let (key, layer) = self.layer_for_objects()?;
        let parameters = shape.sanitised(parameters);

        self.remember_objects_before();
        // Bracketed, because placing is two engine edits — the item, then
        // where it goes — and a sculptor asked for one thing. Without the
        // group, one undo took back the placement and left the item standing
        // at the origin, which is the same shape `convert_layer` brackets for.
        self.document
            .begin_undo_group()
            .map_err(ModelError::engine)?;
        let placed = self.place_item(layer, shape, &parameters, at, combine);
        // Closed on the failing path too: a group left open swallows every
        // edit after it into one undo step.
        let closed = self.document.end_undo_group().map_err(ModelError::engine);
        let node = placed?;
        closed?;
        let object = PlacedObject::new(
            key,
            node,
            clayspace_model::ObjectSource::Shape(shape),
            parameters,
            combine,
            at,
        );
        let id = object.id();
        self.objects.push(object);
        self.selected_object = Some(id);
        self.remember_objects_after();

        let bound = self.node_bound(layer, node);
        self.refill_bound(layer, bound)?;
        Ok(id)
    }

    fn insert_shape_subtool(
        &mut self,
        shape: Shape,
        parameters: &[f32],
        at: [f32; 3],
        combine: CombineSettings,
    ) -> Result<Inserted, ModelError> {
        let parameters = shape.sanitised(parameters);
        // The shape's own word for itself, made unique against the stack. Not
        // "Camada 4": a sculptor looking for the cylinder they inserted looks
        // for a cylinder.
        let name = self.unique_layer_name(shape.label());

        let placed = parameters.clone();
        let (key, layer, node) = self.insert_subtool(&name, move |doc, layer| {
            // At the layer's own origin, and the layer stands where the
            // sculptor pointed — see `stand_subtool_at`.
            let node = doc.place_item(layer, shape, &placed, [0.0; 3], combine)?;
            doc.stand_subtool_at(layer, at)?;
            Ok(node)
        })?;

        let object = PlacedObject::new(
            key,
            node,
            clayspace_model::ObjectSource::Shape(shape),
            parameters,
            combine,
            // Where it stands inside its subtool, which is the middle: the
            // layer carries the position. Recording `at` here would report the
            // offset twice, once in the node and once in the layer.
            [0.0; 3],
        );
        let id = object.id();
        self.objects.push(object);
        // The *subtool* is what arrived, so the subtool is the selection —
        // "a new subtool holds the sphere, it is the active subtool". Leaving
        // the item selected too would put two manipulators over one thing: the
        // panel hides the whole-subtool controls whenever an object is
        // selected, so a form inserted to be aimed would arrive with no way to
        // aim it. The row is in the list and one click selects it.
        self.selected_object = None;
        self.settle_subtool(layer)?;
        Ok(Inserted {
            layer: key,
            object: Some(id),
        })
    }

    fn copy_subtool(&mut self, from: LayerKey, cell_size: f32) -> Result<Inserted, ModelError> {
        let index = self.index_of(from)?;
        let source = self.layers[index].name.clone();
        // Baked before anything is created, and outside the undo group below:
        // the hide-and-restore around the sampling writes visibility commands
        // of its own, and the sculptor's one step back has to reach the copy
        // rather than the flags the bake borrowed.
        let mut item = self.bake_subtool(from, cell_size)?;
        item.set_op(Op::Add).map_err(ModelError::engine)?;

        let name = self.unique_layer_name(&format!("{source} {}", Self::COPY_SUFFIX));
        let (key, layer, _) = self.insert_subtool(&name, move |doc, layer| {
            doc.document
                .add_item(layer, &item)
                .map_err(ModelError::engine)
        })?;
        // No object row: what stands in the copy is a sampled volume, not one
        // of the offered shapes, so there is nothing for the shape controls to
        // measure. The subtool itself is the selection, and the whole-subtool
        // manipulator is what moves it.
        self.selected_object = None;
        self.settle_subtool(layer)?;
        Ok(Inserted {
            layer: key,
            object: None,
        })
    }

    fn copyable_subtools(&mut self) -> Vec<(LayerKey, String)> {
        // What the bake can actually sample: a layer with an extent. An empty
        // one would copy to an empty subtool, and a mesh layer is carried
        // rather than evaluated, so neither contributes a field to sample.
        self.layers
            .iter()
            .filter(|layer| layer.representation != Representation::Mesh)
            .map(|layer| (layer.key, layer.name.clone()))
            .filter(|(key, _)| SceneModel::layer_bounds(self, *key).is_some())
            .collect()
    }

    fn boolean_operands(&mut self) -> Vec<(LayerKey, String)> {
        // Every representation, because every one of them can be an operand —
        // a mesh through the crossing `bake_operand` performs for it. Protected
        // subtools stay on the list so the refusal can name them, which is what
        // the specification asks for; empty ones do not, because there is
        // nothing in them to combine.
        let named: Vec<(LayerKey, String)> = self
            .layers
            .iter()
            .map(|layer| (layer.key, layer.name.clone()))
            .collect();
        named
            .into_iter()
            .filter(|(key, _)| self.operand_bounds(*key).is_some())
            .collect()
    }

    fn boolean_cell(&mut self, base: LayerKey, tool: LayerKey) -> Option<f32> {
        self.index_of(base).ok()?;
        self.index_of(tool).ok()?;
        // The finer of the two, so the coarser operand does not decide how much
        // of the finer one survives.
        Some(self.operand_detail(base).min(self.operand_detail(tool)))
    }

    fn boolean_cost(&mut self, settings: BooleanSettings) -> Option<Cost> {
        let settings = settings.sanitized();
        let (base, tool) = settings.pair()?;
        // The boxes rather than the refusals: a panel prices what it can and
        // the refusal is the run's to state, so a ghosted operand still shows
        // what the operation would cost if it were not.
        let region = Self::boolean_region(
            self.operand_bounds(base)?,
            self.operand_bounds(tool)?,
            settings.cell_size,
        );
        Some(Self::boolean_cost_over(region, settings.cell_size))
    }

    fn run_boolean(&mut self, settings: BooleanSettings) -> Result<Inserted, ModelError> {
        let settings = settings.sanitized();
        let pair = settings
            .pair()
            .ok_or(ModelError::Boolean(BooleanRefusal::NotAPair))?;
        let (base, tool) = pair;
        let (base_box, tool_box) = self.boolean_extents(base, tool)?;
        // An intersection of two forms that do not meet is nothing, and the
        // specification says to say so rather than to make an empty subtool.
        if settings.op == BooleanOp::Intersect && !Self::boxes_meet(base_box, tool_box) {
            return Err(ModelError::Boolean(BooleanRefusal::NoOverlap {
                base: self.operand_name(base)?,
                tool: self.operand_name(tool)?,
            }));
        }
        let region = Self::boolean_region(base_box, tool_box, settings.cell_size);
        self.boolean_fits_the_budget(region, settings.cell_size)?;

        // Named for what made it, in a mark that reads the same in every
        // language the interface is offered in.
        let name = self.unique_layer_name(&format!(
            "{} {} {}",
            self.operand_name(base)?,
            settings.op.mark(),
            self.operand_name(tool)?
        ));
        self.build_boolean(settings, pair, region, &name)
    }

    fn mesh_operands(&mut self) -> Vec<(LayerKey, String)> {
        self.layers
            .iter()
            .filter(|layer| {
                layer.representation == Representation::Mesh
                    && layer.carries_geometry
                    // A protected layer refuses every edit naming it, and a
                    // crossing reads rather than writes — but offering one
                    // whose source a sculptor has locked reads as ignoring
                    // the lock. Left out, as the tool shelf leaves out a verb
                    // the layer has no route for.
                    && !layer.protection.locked
            })
            .map(|layer| (layer.key, layer.name.clone()))
            .collect()
    }

    fn mesh_operand_cost(&mut self, from: LayerKey, cell_size: f32) -> Option<Cost> {
        let index = self.index_of(from).ok()?;
        if self.layers[index].representation != Representation::Mesh {
            return None;
        }
        // The mesh's own bounds, which is what a crossing of it covers — not
        // `bounds()`, which answers for the *active* layer and would price the
        // wrong model. Through `operand_bounds` because the boolean asks the
        // same question of the same layer, and two answers to "where is that
        // model" is one more than there should be.
        let (min, max) = self.operand_bounds(from)?;
        let extent: [f32; 3] = std::array::from_fn(|i| (max[i] - min[i]).max(0.0));
        // The conversion panel's own computation, for the same crossing at the
        // same resolution — because it is the same crossing.
        Some(Cost::of(Direction::MeshToSdf, cell_size, extent))
    }

    fn place_mesh_object(
        &mut self,
        from: LayerKey,
        cell_size: f32,
        at: [f32; 3],
        combine: CombineSettings,
    ) -> Result<ObjectId, ModelError> {
        let (key, layer) = self.layer_for_objects()?;
        let source_index = self.index_of(from)?;
        if self.layers[source_index].representation != Representation::Mesh {
            return Err(ModelError::Conversion(Refusal::WrongSource {
                needs: Representation::Mesh,
                active: self.layers[source_index].representation,
            }));
        }
        let engine_name = self.layers[source_index].engine_name.clone();
        let name = self.layers[source_index].name.clone();

        self.remember_objects_before();
        // Bracketed, as a placement is: sampling the mesh and standing it
        // somewhere are two edits and one thing a sculptor asked for.
        self.document
            .begin_undo_group()
            .map_err(ModelError::engine)?;
        let placed = (|| -> Result<NodeId, ModelError> {
            let mut item = self
                .document
                .mesh_layer_as_volume(&engine_name, Self::bake_volume(cell_size))
                .map_err(ModelError::engine)?;
            item.set_op(engine_op(combine.op))
                .map_err(ModelError::engine)?;
            item.set_blend(engine_blend(combine.blend), combine.radius)
                .map_err(ModelError::engine)?;
            let node = self
                .document
                .add_item(layer, &item)
                .map_err(ModelError::engine)?;
            self.document
                .set_node_transform(layer, node, at, [0.0, 1.0, 0.0], 0.0, 1.0)
                .map_err(ModelError::engine)?;
            Ok(node)
        })();
        let closed = self.document.end_undo_group().map_err(ModelError::engine);
        let node = placed?;
        closed?;

        // The source layer is untouched: what stands in this layer is a copy,
        // sampled onto a lattice. The mesh is still a mesh and still sculptable
        // with the sixteen fixed-topology brushes.
        let object = PlacedObject::new(
            key,
            node,
            clayspace_model::ObjectSource::Mesh { from, name },
            Vec::new(),
            combine,
            at,
        );
        let id = object.id();
        self.objects.push(object);
        self.selected_object = Some(id);
        self.remember_objects_after();

        let bound = self.node_bound(layer, node);
        self.refill_bound(layer, bound)?;
        Ok(id)
    }

    fn set_object_transform(
        &mut self,
        id: ObjectId,
        position: [f32; 3],
        rotation_axis: [f32; 3],
        rotation_angle: f32,
        scale: [f32; 3],
    ) -> Result<(), ModelError> {
        let at = self
            .object_index(id)
            .ok_or_else(|| self.no_objects_here())?;
        let layer = self.layer_id(id.layer)?;
        let node = self.objects[at].node;

        // Where it was, before it stops being there. Refilling only the
        // destination leaves the surface it used to cut still cut.
        let before = self.node_bound(layer, node);
        // Inside a gesture the group is already open and the table was already
        // recorded at its start; snapshotting per frame would key thirty
        // states to one undo depth and keep only the last.
        let gesturing = self.dragging.is_some();
        if !gesturing {
            self.remember_objects_before();
        }
        // The per-axis call, always — not only when the three differ. The ABI
        // does not do partial updates: each of the two writes the *whole*
        // transform, so the uniform one applied to a node carrying a stretch
        // would collapse it. Using one call for both means a move can never
        // quietly unsquash what it moves. A uniform value costs nothing: the
        // engine says `(1, 1, 1)` and any other uniform triple keeps the field
        // exact and compiles to identical tape.
        self.document
            .set_node_transform_nonuniform(
                layer,
                node,
                position,
                rotation_axis,
                rotation_angle,
                scale,
            )
            .map_err(ModelError::engine)?;

        let object = &mut self.objects[at];
        object.position = position;
        object.rotation_axis = rotation_axis;
        object.rotation_angle = rotation_angle;
        object.scale = scale;
        if !gesturing {
            self.remember_objects_after();
        }

        let after = self.node_bound(layer, node);
        self.refill_bound(layer, union(before, after))
    }

    fn set_object_shape(
        &mut self,
        id: ObjectId,
        shape: Shape,
        parameters: &[f32],
    ) -> Result<(), ModelError> {
        let at = self
            .object_index(id)
            .ok_or_else(|| self.no_objects_here())?;
        let layer = self.layer_id(id.layer)?;
        let node = self.objects[at].node;
        let parameters = shape.sanitised(parameters);

        let before = self.node_bound(layer, node);
        self.remember_objects_before();
        // The engine keeps what belongs to the node rather than to the
        // primitive, so the transform and the operation survive this.
        self.document
            .set_node_prim(layer, node, primitive_of(shape, &parameters))
            .map_err(ModelError::engine)?;

        let object = &mut self.objects[at];
        object.source = clayspace_model::ObjectSource::Shape(shape);
        object.parameters = parameters;
        self.remember_objects_after();

        let after = self.node_bound(layer, node);
        self.refill_bound(layer, union(before, after))
    }

    fn set_object_combine(
        &mut self,
        id: ObjectId,
        combine: CombineSettings,
    ) -> Result<(), ModelError> {
        let at = self
            .object_index(id)
            .ok_or_else(|| self.no_objects_here())?;
        let layer = self.layer_id(id.layer)?;
        let node = self.objects[at].node;

        let before = self.node_bound(layer, node);
        self.remember_objects_before();
        self.document
            .set_node_op_blend(
                layer,
                node,
                engine_op(combine.op),
                engine_blend(combine.blend),
                combine.radius,
                0.0,
            )
            .map_err(ModelError::engine)?;
        self.objects[at].combine = combine;
        self.remember_objects_after();

        // An operation is the one edit that can take the bound away entirely:
        // turning a subtraction into an intersection makes it non-local.
        let after = self.node_bound(layer, node);
        self.refill_bound(layer, union(before, after))
    }

    fn remove_object(&mut self, id: ObjectId) -> Result<(), ModelError> {
        let at = self
            .object_index(id)
            .ok_or_else(|| self.no_objects_here())?;
        let layer = self.layer_id(id.layer)?;
        let node = self.objects[at].node;

        // Taken before the node is: what it reached is what has to be refilled
        // once it is gone, and afterwards there is nothing to ask.
        let bound = self.node_bound(layer, node);
        self.remember_objects_before();
        self.document
            .remove_node(layer, node)
            .map_err(ModelError::engine)?;
        self.objects.remove(at);
        if self.selected_object == Some(id) {
            self.selected_object = None;
        }
        self.remember_objects_after();

        self.refill_bound(layer, bound)
    }

    fn target_transform(&mut self, target: GizmoTarget) -> Option<clayspace_model::Transform> {
        match target {
            GizmoTarget::Object(id) => {
                let at = self.object_index(id)?;
                let object = &self.objects[at];
                Some(clayspace_model::Transform {
                    position: object.position,
                    rotation_axis: object.rotation_axis,
                    rotation_angle: object.rotation_angle,
                    scale: object.scale,
                })
            }
            GizmoTarget::Layer(key) => {
                let index = self.index_of(key).ok()?;
                Some(self.layers[index].transform)
            }
            // A curve's points belong to the application while it is being
            // authored, so a curve is transformed through the point path the
            // cage already uses rather than through an engine transform.
            GizmoTarget::Curve => None,
        }
    }

    fn begin_target_drag(&mut self, target: GizmoTarget) {
        // A gesture already open is closed first: one left open would swallow
        // every edit after it into a single undo step, which is a worse bug
        // than the one this exists to fix.
        if self.dragging.is_some() {
            self.end_target_drag();
        }
        // The table before the gesture, so an undo of the whole drag finds the
        // state it started from rather than the state one frame in. A drag on
        // a whole subtool needs no such record: the engine holds where a layer
        // stands and answers for it, so an undo restores the placement itself
        // and `resync_layer_transforms` reads it back.
        self.remember_objects_before();
        if self.document.begin_undo_group().is_ok() {
            self.dragging = Some(target);
        }
    }

    fn end_target_drag(&mut self) {
        if self.dragging.take().is_none() {
            return;
        }
        let _ = self.document.end_undo_group();
        self.remember_objects_after();
    }

    fn set_target_transform(
        &mut self,
        target: GizmoTarget,
        transform: clayspace_model::Transform,
    ) -> Result<(), ModelError> {
        match target {
            GizmoTarget::Object(id) => self.set_object_transform(
                id,
                transform.position,
                transform.rotation_axis,
                transform.rotation_angle,
                transform.scale,
            ),
            // A mesh layer takes the same route: it is a layer, and the engine
            // composes a layer transform for one exactly as for a field.
            // `clay_mesh_transform` is for a bake that needs the moved
            // vertices, not for standing a layer somewhere.
            GizmoTarget::Layer(key) => self.place_layer(key, transform),
            GizmoTarget::Curve => Err(ModelError::Unavailable(
                clayspace_model::Unavailable::WrongGesture {
                    needs: "os pontos da curva",
                },
            )),
        }
    }

    fn pick_item(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<(ItemKind, LayerKey)> {
        let hit = self
            .document
            .raycast_attributed(origin, direction)
            .ok()
            .flatten()?;
        let (layer, node) = (hit.layer?, hit.node?);
        let prim = self.document.node_prim(layer, node).ok()?;
        // The table first, then the primitive. A stamping stroke deposits
        // spheres, so the primitive alone would call every stamp an object;
        // the table knows which nodes were placed, and `kind_of` answers for
        // the rest — which is what tells a rig from a curve from a stroke, and
        // so what lets the interface say *why* a click cannot be transformed
        // rather than doing nothing.
        let key = self
            .layers
            .iter()
            .find(|candidate| candidate.id == layer)
            .map(|candidate| candidate.key)?;
        let placed = self.object_index(ObjectId {
            layer: key,
            node: node.get(),
        });
        let kind = match placed {
            Some(_) => ItemKind::Object,
            None => match kind_of(prim) {
                // An item nobody placed and whose primitive says "object" is a
                // stroke's stamp: there is nothing else it can be.
                ItemKind::Object => ItemKind::Stroke,
                other => other,
            },
        };
        // The layer travels with the kind because this raycast already
        // attributed it: a press needs both answers and used to pay for a
        // second attributed raycast to get the second one.
        Some((kind, key))
    }

    fn pick_object(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Option<ObjectId> {
        // The attributing raycast, which is not the cheap path — it compiles
        // the document and a tape per candidate item. On a click and never on
        // a hover.
        let hit = self
            .document
            .raycast_attributed(origin, direction)
            .ok()
            .flatten()?;
        let (layer, node) = (hit.layer?, hit.node?);
        // A hit is attributed to "the item whose field is closest at the hit
        // point, so a subtract item is attributed the surface it carved" —
        // which is why clicking the wall of a hole selects the shape that cut
        // it. A stroke or a rig attributes too, and neither is an object: the
        // table is what says which, since a stamp and a placed sphere are the
        // same primitive.
        let key = self
            .layers
            .iter()
            .find(|candidate| candidate.id == layer)
            .map(|candidate| candidate.key)?;
        let id = ObjectId {
            layer: key,
            node: node.get(),
        };
        self.object_index(id).map(|_| id)
    }
}

#[cfg(test)]
mod live_mesh_guard {
    use super::*;

    /// A gesture that deferred its normals and was never settled by anyone.
    ///
    /// Every path this crate takes through `stroke_mesh` settles before it
    /// returns, so `Drop` is the exit nothing else covers: a `?` unwinding out
    /// of a stamp, a mask lease refused, the gesture replaced when the pointer
    /// lands on another subtool. There is no way to reach that from outside
    /// the crate — which is exactly why it is tested from inside it, by
    /// building the gesture, deferring, stamping, and simply letting the value
    /// go out of scope.
    #[test]
    fn dropping_a_gesture_recomputes_what_it_deferred() {
        let policy = crate::BackendPolicy::discover(None).expect("discover backends");
        let mut document = ClayDocument::new(policy)
            .and_then(ClayDocument::with_starting_form)
            .expect("a document with a starting form");
        document
            .convert_layer(clayspace_model::Direction::SdfToMesh, 0.03, 0)
            .expect("into a mesh");

        let key = document.active_layer().key;
        let engine_name = document.active_layer().engine_name.clone();
        document
            .ensure_mesh_sculptor(key, &engine_name)
            .expect("a sculptor");
        let sculptor = document.sculptor_for(key).expect("the sculptor just built");

        let (before_positions, before_normals, ..) = document.visible_mesh_geometry();
        assert!(!before_positions.is_empty(), "the fixture carries no mesh");

        {
            let mut live = LiveMesh::new(
                key,
                sculptor.clone(),
                claycore::MeshDeltas::new().expect("a record"),
            );
            // Declared after `live`, so it is released first and the drop
            // below can take the sculptor for itself.
            let mut held = sculptor.borrow_mut();
            held.set_defer_normals(true).expect("defer");
            let moved = held
                .stamp(
                    claycore::MeshStamp {
                        verb: claycore::MeshBrush::Draw,
                        center: [0.0, 0.0, 1.0],
                        radius: 0.5,
                        strength: 0.5,
                        ..claycore::MeshStamp::default()
                    },
                    None,
                    Some(live.deltas()),
                )
                .expect("stamp");
            assert!(moved > 0, "the stamp reached nothing to defer");
            // Neither settled nor finished: the gesture ends by going out of
            // scope, which is the whole subject of this test.
        }

        assert!(
            !sculptor
                .borrow()
                .defer_normals()
                .expect("read the flag back"),
            "the dropped gesture left the sculptor deferring, so the next \
             thing to stamp this mesh would defer with nobody owing a flush"
        );

        let (after_positions, after_normals, ..) = document.visible_mesh_geometry();
        let mut moved = 0;
        let mut stale = 0;
        for i in 0..before_positions.len() {
            if before_positions[i] != after_positions[i] {
                moved += 1;
                if before_normals[i] == after_normals[i] {
                    stale += 1;
                }
            }
        }
        assert!(moved > 0, "nothing moved, so nothing was deferred");
        assert!(
            stale * 4 < moved,
            "{stale} of {moved} moved vertices still carry the normal they \
             had before the stamp: dropping the gesture did not recompute \
             what it deferred"
        );
    }
}
