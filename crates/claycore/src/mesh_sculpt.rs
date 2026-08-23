//! Sculpting a mesh layer's own vertices, without changing its topology.
//!
//! The return trip: sculpt on SDF or voxels, quad-export, retopologize and UV
//! elsewhere, bring the mesh back and refine it *in place*. Before this the
//! only way to edit an imported mesh was to resample it onto a lattice, which
//! gives the sculpt back and keeps neither the edge loops nor the UVs — so a
//! model that had just been retopologized could not be touched without
//! spending the retopology.
//!
//! Every verb here holds one line above everything else: **topology never
//! changes.** No polygon is created, split or deleted, and `indices` and
//! `quads` come out byte for byte.
//!
//! A sculptor is a *stateful* object, unlike the rest of this crate's sculpting
//! surface. It builds vertex adjacency once and keeps it, which is what makes a
//! brush cost what its falloff reached rather than what the mesh holds — an
//! O(vertices) build the session pays once. Creating one per stroke would pay
//! it per stroke, which is the shape a latency budget exists to forbid.

use std::ptr::NonNull;

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::error::{check, ErrorKind, Result};
use crate::mask::Mask;
use crate::mesh::Mesh;
use crate::raw_failure;

/// The sixteen fixed-topology verbs.
///
/// Eleven classical ones, then relax, layer and nudge, and finally paint and
/// smear — the only two that move no vertex at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeshBrush {
    /// Drag the region by the stroke delta.
    Grab,
    /// Displace along the *region's* averaged normal, which is what makes it a
    /// rounded swell rather than a balloon.
    Draw,
    /// Displace along each vertex's own normal.
    Inflate,
    /// Laplacian average over the one-ring.
    Smooth,
    /// Signed: positive gathers, negative spreads.
    Pinch,
    /// Project onto a plane.
    Flatten,
    /// Draw's deposit, clamped to a plane.
    Clay,
    /// A tight negative draw and a pinch, in one stamp.
    Crease,
    /// Flatten cut-only and smooth, from one snapshot.
    Scrape,
    /// Smooth, gated by the dihedral angle — which is what still holds a
    /// corner up under a heavy pass.
    Polish,
    /// Grab, re-anchored along the drag.
    Snakehook,
    /// Slides vertices along the surface to even their spacing. Recovers a
    /// stretched *grab*; it cannot recover a deformation, because a taper
    /// leaves a cross-section with the same vertex count around a smaller
    /// circumference and no verb that slides vertices changes how many a ring
    /// has.
    Relax,
    /// A stroke that does not build up on itself.
    Layer,
    /// Drags the surface skin, leaving the interior.
    Nudge,
    /// Blends vertex colour toward a target. Moves no vertex.
    Paint,
    /// Drags existing vertex colour along the stroke. Moves no vertex.
    Smear,
}

impl MeshBrush {
    pub const ALL: [MeshBrush; 16] = [
        Self::Grab,
        Self::Draw,
        Self::Inflate,
        Self::Smooth,
        Self::Pinch,
        Self::Flatten,
        Self::Clay,
        Self::Crease,
        Self::Scrape,
        Self::Polish,
        Self::Snakehook,
        Self::Relax,
        Self::Layer,
        Self::Nudge,
        Self::Paint,
        Self::Smear,
    ];

    /// Whether the verb writes colour rather than moving vertices.
    ///
    /// Both refuse a mesh carrying no colour attribute rather than creating
    /// one: twelve bytes a vertex is a real cost to hide behind a stroke.
    pub fn writes_colour(self) -> bool {
        matches!(self, Self::Paint | Self::Smear)
    }

    /// Whether the verb is told *from where to where* rather than stamped.
    pub fn is_dragged(self) -> bool {
        matches!(self, Self::Grab | Self::Snakehook | Self::Nudge)
    }

    fn to_raw(self) -> i32 {
        (match self {
            Self::Grab => sys::clay_mesh_brush::CLAY_MESH_BRUSH_GRAB,
            Self::Draw => sys::clay_mesh_brush::CLAY_MESH_BRUSH_DRAW,
            Self::Inflate => sys::clay_mesh_brush::CLAY_MESH_BRUSH_INFLATE,
            Self::Smooth => sys::clay_mesh_brush::CLAY_MESH_BRUSH_SMOOTH,
            Self::Pinch => sys::clay_mesh_brush::CLAY_MESH_BRUSH_PINCH,
            Self::Flatten => sys::clay_mesh_brush::CLAY_MESH_BRUSH_FLATTEN,
            Self::Clay => sys::clay_mesh_brush::CLAY_MESH_BRUSH_CLAY,
            Self::Crease => sys::clay_mesh_brush::CLAY_MESH_BRUSH_CREASE,
            Self::Scrape => sys::clay_mesh_brush::CLAY_MESH_BRUSH_SCRAPE,
            Self::Polish => sys::clay_mesh_brush::CLAY_MESH_BRUSH_POLISH,
            Self::Snakehook => sys::clay_mesh_brush::CLAY_MESH_BRUSH_SNAKEHOOK,
            Self::Relax => sys::clay_mesh_brush::CLAY_MESH_BRUSH_RELAX,
            Self::Layer => sys::clay_mesh_brush::CLAY_MESH_BRUSH_LAYER,
            Self::Nudge => sys::clay_mesh_brush::CLAY_MESH_BRUSH_NUDGE,
            Self::Paint => sys::clay_mesh_brush::CLAY_MESH_BRUSH_PAINT,
            Self::Smear => sys::clay_mesh_brush::CLAY_MESH_BRUSH_SMEAR,
        }) as i32
    }
}

/// How a mesh brush's strength falls off across its radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeshFalloff {
    Constant,
    Linear,
    #[default]
    Smooth,
    Gaussian,
}

impl MeshFalloff {
    fn to_raw(self) -> i32 {
        (match self {
            Self::Constant => sys::clay_mesh_falloff::CLAY_MESH_FALLOFF_CONSTANT,
            Self::Linear => sys::clay_mesh_falloff::CLAY_MESH_FALLOFF_LINEAR,
            Self::Smooth => sys::clay_mesh_falloff::CLAY_MESH_FALLOFF_SMOOTH,
            Self::Gaussian => sys::clay_mesh_falloff::CLAY_MESH_FALLOFF_GAUSSIAN,
        }) as i32
    }
}

/// One mesh stamp.
#[derive(Debug, Clone, Copy)]
pub struct MeshStamp<'a> {
    pub verb: MeshBrush,
    /// In the mesh's own space.
    pub center: [f32; 3],
    pub radius: f32,
    /// Signed for every verb that has one, and scaled into world units by the
    /// radius — so a brush behaves the same at any size.
    pub strength: f32,
    pub falloff: MeshFalloff,
    /// What Grab and Snakehook move by. Ignored by the rest, and ignored by a
    /// stroke, which takes it from the motion between stamps.
    pub direction: [f32; 3],
    /// Measure the falloff *along the surface* rather than in a straight line.
    ///
    /// A brush on the upper lip must not drag the chin through a closed mouth.
    /// Flatten and Scrape want it off: they mean "everything under this disc",
    /// and a surface walk refuses to flatten across a groove.
    pub geodesic: bool,
    /// The colour Paint blends toward.
    pub colour: [f32; 3],
    /// A scalar stamp scaling this brush's per-vertex weight.
    ///
    /// Borrowed for the duration of the call — the engine copies nothing — so
    /// the lifetime is on the stamp rather than the samples being owned here.
    /// `None` leaves every verb exactly as it was, which is why it is the
    /// default and why an existing call site needs no change.
    ///
    /// It multiplies the *weight*, so it composes with every verb and every
    /// falloff at once, and it is sampled by the same kernel the SDF alpha
    /// uses — one stamp reads identically on a mesh and on a field.
    pub alpha: Option<AlphaStamp<'a>>,
}

/// A scalar stamp, borrowed for one call.
///
/// The engine decodes no images and copies no samples: this is a view of a
/// buffer the caller holds, and it must outlive the call it is passed to. Its
/// samples are `width * height` values in 0..=1, row-major with u fastest.
#[derive(Debug, Clone, Copy)]
pub struct AlphaStamp<'a> {
    pub samples: &'a [f32],
    pub width: i32,
    pub height: i32,
    /// The normal of the plane the stamp is projected in. All zeroes means the
    /// surface normal under the brush centre.
    pub direction: [f32; 3],
    /// Orients the stamp in that plane; any rough "up" works.
    pub tangent: [f32; 3],
    /// The square the stamp covers, in world units. Zero means the brush's own
    /// diameter.
    pub extent: f32,
}

impl AlphaStamp<'_> {
    /// Whether the samples fill the dimensions claimed.
    ///
    /// Checked before the pointer is handed over: the engine reads
    /// `width * height` floats out of it, so a shorter slice is a read past
    /// the end whatever the engine's own validation says about the
    /// dimensions.
    fn is_well_formed(&self) -> bool {
        self.width >= 2
            && self.height >= 2
            && (self.width as i64) * (self.height as i64) <= self.samples.len() as i64
    }
}

impl Default for MeshStamp<'_> {
    fn default() -> Self {
        Self {
            verb: MeshBrush::Draw,
            center: [0.0; 3],
            radius: 0.1,
            strength: 0.5,
            falloff: MeshFalloff::default(),
            direction: [0.0; 3],
            geodesic: true,
            colour: [1.0; 3],
            alpha: None,
        }
    }
}

impl MeshStamp<'_> {
    /// The engine's descriptor.
    ///
    /// The alpha pointer it carries borrows from `self`, so the result must
    /// not outlive the stamp it came from — every caller here passes it
    /// straight into one C call and drops it.
    fn as_raw(&self) -> sys::clay_mesh_brush_desc {
        let mut raw = sys::clay_mesh_brush_desc::sized();
        raw.verb = self.verb.to_raw();
        raw.center = self.center;
        raw.radius = self.radius;
        raw.strength = self.strength;
        raw.falloff = self.falloff.to_raw();
        raw.direction = self.direction;
        raw.geodesic = i32::from(self.geodesic);
        // Told to search rather than seeded. A caller that has already picked
        // knows the class and should say so — searching is a linear scan and
        // is the wrong thing to do per stamp on a large mesh — but a wrong
        // seed is worse than a slow one.
        raw.seed_class = sys::CLAY_MESH_NO_CLASS;
        raw.color = self.colour;
        // A malformed stamp is dropped rather than refused: the alpha is a
        // modulation, and losing it leaves the verb doing what it would have
        // done without one. Refusing the whole stroke because a texture was
        // the wrong shape would be a worse trade in the middle of a gesture.
        if let Some(alpha) = self.alpha.filter(AlphaStamp::is_well_formed) {
            raw.alpha = alpha.samples.as_ptr();
            raw.alpha_width = alpha.width;
            raw.alpha_height = alpha.height;
            raw.alpha_direction = alpha.direction;
            raw.alpha_tangent = alpha.tangent;
            raw.alpha_extent = alpha.extent;
        }
        raw
    }
}

/// A whole-form deformer, and the frame it acts in.
///
/// A different kind of thing from a brush, and deliberately so: no centre, no
/// radius, no falloff, because a deformer states something about the *form*
/// and a brush about a dab.
///
/// Applied as FORWARD point maps once per vertex — the opposite direction to
/// the SDF deformers of the same name, which must run backwards to answer
/// "where did the material at this point come from". Forwards is both the
/// easier direction and the exact one, so a tapered mesh and a tapered field
/// are the same shape.
///
/// There is deliberately no bend: its map folds distinct points onto the same
/// place past a gentle angle, so no forward map exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshDeform {
    /// The cross-section scale ramps across the span.
    Taper,
    /// Rotation about the axis ramps across the span.
    Twist,
}

impl MeshDeform {
    pub const ALL: [MeshDeform; 2] = [Self::Taper, Self::Twist];

    fn to_raw(self) -> i32 {
        (match self {
            Self::Taper => sys::clay_mesh_deform::CLAY_MESH_DEFORM_TAPER,
            Self::Twist => sys::clay_mesh_deform::CLAY_MESH_DEFORM_TWIST,
        }) as i32
    }
}

/// What a deformer does, and over what.
#[derive(Debug, Clone, Copy)]
pub struct MeshDeformer {
    pub verb: MeshDeform,
    pub origin: [f32; 3],
    pub axis: [f32; 3],
    /// How far along `axis` the ramp runs. Must be positive.
    pub span: f32,
    /// Taper: the cross-section scale at each end of the span. 1 and 1 is the
    /// identity.
    pub scale_start: f32,
    pub scale_end: f32,
    /// Twist: total rotation across the span, in radians. 0 is the identity.
    pub angle: f32,
}

impl Default for MeshDeformer {
    fn default() -> Self {
        Self {
            verb: MeshDeform::Taper,
            origin: [0.0; 3],
            axis: [0.0, 1.0, 0.0],
            span: 1.0,
            scale_start: 1.0,
            scale_end: 1.0,
            angle: 0.0,
        }
    }
}

impl MeshDeformer {
    fn to_raw(self) -> sys::clay_mesh_deform_desc {
        let mut raw = sys::clay_mesh_deform_desc::sized();
        raw.verb = self.verb.to_raw();
        raw.origin = self.origin;
        raw.axis = self.axis;
        raw.span = self.span;
        raw.scale_start = self.scale_start;
        raw.scale_end = self.scale_end;
        raw.angle = self.angle;
        raw
    }
}

/// A free-form deformation cage over a mesh — ZBrush's Gizmo, in effect.
///
/// The one ZBrush gizmo deformer that is not an SDF deformer here, and
/// deliberately: ZBrush and Blender both apply FFD *forward* to vertices,
/// which a mesh allows and an implicit field does not.
pub struct MeshLattice {
    raw: NonNull<sys::clay_mesh_lattice>,
}

impl MeshLattice {
    /// A cage over a box, with control points per axis.
    pub fn new(min: [f32; 3], max: [f32; 3], divisions: [i32; 3]) -> Result<Self> {
        // SAFETY: two arrays of three floats and three counts; returns an
        // owned handle or null.
        let raw = unsafe {
            sys::clay_mesh_lattice_create(
                min.as_ptr(),
                max.as_ptr(),
                divisions[0],
                divisions[1],
                divisions[2],
            )
        };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_mesh_lattice_create", ErrorKind::Backend))
    }

    /// Control points per axis, after the engine's own clamping.
    pub fn divisions(&self) -> Result<[i32; 3]> {
        let (mut nx, mut ny, mut nz) = (0, 0, 0);
        // SAFETY: valid handle, three out-parameters written on success.
        check(
            unsafe {
                sys::clay_mesh_lattice_divisions(self.raw.as_ptr(), &mut nx, &mut ny, &mut nz)
            },
            "clay_mesh_lattice_divisions",
        )?;
        Ok([nx, ny, nz])
    }

    /// Drags one control point.
    pub fn set_offset(&mut self, at: [i32; 3], offset: [f32; 3]) -> Result<()> {
        // SAFETY: valid handle, an index the entry point range-checks, and an
        // array of three floats.
        check(
            unsafe {
                sys::clay_mesh_lattice_set_offset(
                    self.raw.as_ptr(),
                    at[0],
                    at[1],
                    at[2],
                    offset.as_ptr(),
                )
            },
            "clay_mesh_lattice_set_offset",
        )
    }

    /// Where a control point is now — rest plus offset, which is what a gizmo
    /// draws.
    pub fn position(&self, at: [i32; 3]) -> Result<[f32; 3]> {
        let mut out = [0.0; 3];
        // SAFETY: valid handle, a range-checked index, and an array of three
        // floats written on success.
        check(
            unsafe {
                sys::clay_mesh_lattice_position(
                    self.raw.as_ptr(),
                    at[0],
                    at[1],
                    at[2],
                    out.as_mut_ptr(),
                )
            },
            "clay_mesh_lattice_position",
        )?;
        Ok(out)
    }

    /// Whether no control point has been dragged.
    ///
    /// Worth asking before applying one: an untouched cage moves every point
    /// by exactly zero, and paying for that over every vertex is a cost with
    /// nothing to show.
    pub fn is_identity(&self) -> Result<bool> {
        let mut identity = 0;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_mesh_lattice_is_identity(self.raw.as_ptr(), &mut identity) },
            "clay_mesh_lattice_is_identity",
        )?;
        Ok(identity != 0)
    }
}

impl Drop for MeshLattice {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once.
        unsafe { sys::clay_mesh_lattice_destroy(self.raw.as_ptr()) };
    }
}

/// A sparse, coalesced record of what a gesture moved.
///
/// The undo a mesh stroke cannot get from the edit list: a vertex displacement
/// is destructive and is not an edit item, so there is nothing in the document
/// to take back. This is what the engine offers instead, and it reverts *bit
/// exactly* rather than approximately — which is the difference between an
/// undo and something that mostly looks like one.
///
/// Coalesced across a gesture: a stroke that passes over the same vertex forty
/// times records where it started, once.
pub struct MeshDeltas {
    raw: NonNull<sys::clay_mesh_deltas>,
}

impl MeshDeltas {
    pub fn new() -> Result<Self> {
        // SAFETY: takes nothing and returns an owned handle or null.
        let raw = unsafe { sys::clay_mesh_deltas_create() };
        NonNull::new(raw)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_mesh_deltas_create", ErrorKind::Backend))
    }

    /// How many vertices the gesture moved.
    ///
    /// Zero for a gesture that reached nothing, which is what says a record is
    /// not worth keeping on the undo stack.
    pub fn vertex_count(&self) -> Result<usize> {
        let mut count = 0;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_mesh_deltas_vertex_count(self.raw.as_ptr(), &mut count) },
            "clay_mesh_deltas_vertex_count",
        )?;
        Ok(count)
    }

    /// Puts every recorded vertex back where it was.
    pub fn revert(&self, sculptor: &mut MeshSculptor) -> Result<()> {
        // SAFETY: both handles are valid and belong to the same mesh; the
        // record is read and the sculptor written, which is what it takes.
        check(
            unsafe { sys::clay_mesh_deltas_revert(self.raw.as_ptr(), sculptor.raw.as_ptr()) },
            "clay_mesh_deltas_revert",
        )
    }

    /// Puts the gesture back — the redo half.
    pub fn apply(&self, sculptor: &mut MeshSculptor) -> Result<()> {
        // SAFETY: as above.
        check(
            unsafe { sys::clay_mesh_deltas_apply(self.raw.as_ptr(), sculptor.raw.as_ptr()) },
            "clay_mesh_deltas_apply",
        )
    }
}

impl Drop for MeshDeltas {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once.
        unsafe { sys::clay_mesh_deltas_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for MeshDeltas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshDeltas")
            .field("vertices", &self.vertex_count().ok())
            .finish()
    }
}

/// Where a ray met a mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshHit {
    pub position: [f32; 3],
    /// World space, unit length.
    pub normal: [f32; 3],
    /// The weld class the ray landed on, ready to seed a stamp's surface walk.
    pub seed_class: u32,
}

/// A mesh layer's vertices, with the adjacency a brush needs.
///
/// Borrows nothing: the sculptor owns its own state and is handed the mesh at
/// construction. It goes stale when the mesh's geometry is replaced from
/// outside — an import, a reload, a conversion — which is what `refresh` is
/// for; a vertex count that changed needs a new one.
pub struct MeshSculptor {
    raw: NonNull<sys::clay_mesh_sculptor>,
}

impl MeshSculptor {
    /// Builds the adjacency for a mesh.
    ///
    /// `weld_epsilon` is relative to the mesh's bounding-box diagonal:
    /// vertices closer than that are one point of the surface, which is what
    /// lets a brush move a split seam as a seam rather than tearing it open.
    pub fn new(mesh: &mut Mesh, weld_epsilon: f32) -> Result<Self> {
        let mut sculptor = std::ptr::null_mut();
        // SAFETY: a valid mesh handle; the out-parameter is written only on
        // success.
        check(
            unsafe { sys::clay_mesh_sculptor_create(mesh.as_ptr(), weld_epsilon, &mut sculptor) },
            "clay_mesh_sculptor_create",
        )?;
        NonNull::new(sculptor)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_mesh_sculptor_create", ErrorKind::Backend))
    }

    /// A sculptor for a mesh layer of a document, by name.
    ///
    /// The layer's mesh is *borrowed*, and [`Mesh`] destroys what it holds on
    /// drop, so the handle never leaves this call — the same reason the
    /// conversions keep theirs internal.
    pub fn for_layer(document: &mut crate::Document, layer_name: &str, weld: f32) -> Result<Self> {
        let c_name = crate::cstring(layer_name, "clay_document_mesh_layer")?;
        let mut layer: sys::clay_layer_id = Default::default();
        let mut mesh = std::ptr::null_mut();
        // SAFETY: a valid document and a NUL-terminated name; both outputs are
        // written only on success.
        check(
            unsafe {
                sys::clay_document_mesh_layer(
                    document.as_ptr(),
                    c_name.as_ptr(),
                    &mut layer,
                    &mut mesh,
                )
            },
            "clay_document_mesh_layer",
        )?;
        let mut sculptor = std::ptr::null_mut();
        // SAFETY: the mesh belongs to this document and was just written; it
        // is not wrapped in an owning `Mesh`, so nothing here destroys it.
        check(
            unsafe { sys::clay_mesh_sculptor_create(mesh, weld, &mut sculptor) },
            "clay_mesh_sculptor_create",
        )?;
        NonNull::new(sculptor)
            .map(|raw| Self { raw })
            .ok_or_else(|| raw_failure("clay_mesh_sculptor_create", ErrorKind::Backend))
    }

    pub fn vertex_count(&self) -> Result<usize> {
        let mut count = 0;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_mesh_sculptor_vertex_count(self.raw.as_ptr(), &mut count) },
            "clay_mesh_sculptor_vertex_count",
        )?;
        Ok(count)
    }

    /// Welded classes: fewer than the vertex count exactly where the mesh has
    /// seams, which is how a host can tell it imported a split model.
    pub fn class_count(&self) -> Result<usize> {
        let mut count = 0;
        // SAFETY: as above.
        check(
            unsafe { sys::clay_mesh_sculptor_class_count(self.raw.as_ptr(), &mut count) },
            "clay_mesh_sculptor_class_count",
        )?;
        Ok(count)
    }

    /// One stamp. Returns how many welded classes moved.
    ///
    /// Zero for a stamp that reached nothing, that was fully masked, or whose
    /// settings amount to no displacement — all three are ordinary outcomes
    /// rather than failures.
    pub fn stamp(&mut self, stamp: MeshStamp<'_>, mask: Option<&Mask>) -> Result<usize> {
        let desc = stamp.as_raw();
        let mut moved = 0;
        // SAFETY: valid handle and a descriptor carrying its own size; the
        // mask is either a valid handle or null, which the entry point allows,
        // as is a null deltas.
        check(
            unsafe {
                sys::clay_mesh_sculptor_stamp(
                    self.raw.as_ptr(),
                    &desc,
                    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _),
                    std::ptr::null_mut(),
                    &mut moved,
                )
            },
            "clay_mesh_sculptor_stamp",
        )?;
        Ok(moved)
    }

    /// A whole gesture, resolved by the engine's own stroke engine.
    ///
    /// The fourth consumer of a resolved stroke, beside SDF nodes, voxels and
    /// masks — one set of spacing, pressure, jitter and taper semantics for all
    /// four. `samples` is position, pressure and time per sample.
    pub fn apply_stroke(
        &mut self,
        samples: &[[f32; 5]],
        preset: &crate::StrokePreset,
        stamp: MeshStamp<'_>,
        mask: Option<&Mask>,
        deltas: Option<&mut MeshDeltas>,
    ) -> Result<usize> {
        if samples.is_empty() {
            return Ok(0);
        }
        let desc = stamp.as_raw();
        let raw_preset = preset.to_raw();
        let mut applied = 0;
        // SAFETY: `samples` is `samples.len() * 5` floats, which is the layout
        // the entry point reads; both descriptors carry their own size; the
        // mask and the frame are nullable and the deltas are null. Normals are
        // not deferred, so the mesh is left consistent for the next read.
        check(
            unsafe {
                sys::clay_mesh_sculptor_apply_stroke(
                    self.raw.as_ptr(),
                    samples.as_ptr() as *const f32,
                    samples.len(),
                    &raw_preset,
                    &desc,
                    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _),
                    std::ptr::null(),
                    0,
                    deltas.map_or(std::ptr::null_mut(), |d| d.raw.as_ptr()),
                    &mut applied,
                )
            },
            "clay_mesh_sculptor_apply_stroke",
        )?;
        Ok(applied)
    }

    /// Where a ray meets this mesh, if it does.
    ///
    /// A field raycast could never see a mesh layer — it is not in the tape and
    /// not in the brick cache — so this is the only way a pointer finds one.
    /// The hit carries the weld class it landed on, which a caller should hand
    /// straight back as a stamp's seed so the surface walk starts where the
    /// finger did rather than searching the whole mesh for it.
    pub fn raycast(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Result<Option<MeshHit>> {
        let mut hit = sys::clay_mesh_hit::sized();
        // SAFETY: valid handle, two arrays of three floats, a null transform
        // (the mesh's own space), and a descriptor carrying its own size.
        check(
            unsafe {
                sys::clay_mesh_sculptor_raycast(
                    self.raw.as_ptr(),
                    origin.as_ptr(),
                    direction.as_ptr(),
                    std::ptr::null(),
                    &mut hit,
                )
            },
            "clay_mesh_sculptor_raycast",
        )?;
        Ok((hit.hit != 0).then_some(MeshHit {
            position: hit.position,
            normal: hit.normal,
            seed_class: hit.seed_class,
        }))
    }

    /// Applies a whole-form deformer to every vertex.
    ///
    /// Not a brush: it takes no position, because a deformer states something
    /// about the form rather than about a dab.
    pub fn deform(
        &mut self,
        deformer: MeshDeformer,
        mask: Option<&Mask>,
        deltas: Option<&mut MeshDeltas>,
    ) -> Result<usize> {
        let desc = deformer.to_raw();
        let mut moved = 0;
        // SAFETY: valid handle and a descriptor carrying its own size; the
        // mask and the record are both nullable.
        check(
            unsafe {
                sys::clay_mesh_sculptor_deform(
                    self.raw.as_ptr(),
                    &desc,
                    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _),
                    deltas.map_or(std::ptr::null_mut(), |d| d.raw.as_ptr()),
                    &mut moved,
                )
            },
            "clay_mesh_sculptor_deform",
        )?;
        Ok(moved)
    }

    /// Applies a cage to every vertex.
    pub fn apply_lattice(
        &mut self,
        lattice: &MeshLattice,
        deltas: Option<&mut MeshDeltas>,
    ) -> Result<usize> {
        let mut moved = 0;
        // SAFETY: both handles are valid; the record is nullable.
        check(
            unsafe {
                sys::clay_mesh_sculptor_lattice(
                    self.raw.as_ptr(),
                    lattice.raw.as_ptr(),
                    deltas.map_or(std::ptr::null_mut(), |d| d.raw.as_ptr()),
                    &mut moved,
                )
            },
            "clay_mesh_sculptor_lattice",
        )?;
        Ok(moved)
    }

    /// Updates the ray-query tree for the vertices the last stamp moved.
    ///
    /// The per-stamp call. Topology is fixed, so a stamp leaves the tree a
    /// valid partition of the same triangles with only its bounds stale, and
    /// refitting what the region touched is proportional to the *brush*.
    /// Measured upstream on ~130k triangles with an 800-triangle dab:
    /// 0.021 ms, against 34.9 ms to rebuild.
    pub fn refit(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_mesh_sculptor_refit(self.raw.as_ptr()) },
            "clay_mesh_sculptor_refit",
        )
    }

    /// Rebuilds the tree outright.
    ///
    /// What `refit` is not: after enough refitting the tree's partition stops
    /// being a good one even though it stays valid, and queries get slower.
    /// [`MeshSculptor::quality`] is how a host decides when to pay for this.
    pub fn refresh(&mut self) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe { sys::clay_mesh_sculptor_refresh(self.raw.as_ptr()) },
            "clay_mesh_sculptor_refresh",
        )
    }

    /// What the tree's queries currently cost, as a figure a host can watch.
    pub fn quality(&mut self) -> Result<f32> {
        let mut quality = 0.0;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_mesh_sculptor_quality(self.raw.as_ptr(), &mut quality) },
            "clay_mesh_sculptor_quality",
        )?;
        Ok(quality)
    }
}

impl Drop for MeshSculptor {
    fn drop(&mut self) {
        // SAFETY: owned handle, released exactly once.
        unsafe { sys::clay_mesh_sculptor_destroy(self.raw.as_ptr()) };
    }
}

impl std::fmt::Debug for MeshSculptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshSculptor")
            .field("vertices", &self.vertex_count().ok())
            .field("classes", &self.class_count().ok())
            .finish()
    }
}
