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

use std::num::NonZeroU32;
use std::ptr::NonNull;

use claycore_sys as sys;

use crate::descriptor::Descriptor;
use crate::error::{check, ErrorKind, Result};
use crate::mask::MaskField;
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

/// The gates a brush applies to *itself*.
///
/// Not the freeze a sculptor paints — that is a [`Mask`](crate::Mask), and it
/// is a separate factor. These are the rules a brush follows without being
/// told: do not cross onto a face pointing the other way, do not drag the
/// mesh's open border, stay in the polygroup this stroke started in, protect
/// the crevices. The engine composes them into the per-vertex weight by
/// multiplication and applies them last, so a stamp asking for none of them is
/// bit-identical to one from before automasking existed — which is why
/// [`Default`] is "none" and why an existing call site needs no change.
///
/// **Three of the five factors cross the C ABI and two do not.** Cavity needs
/// a field to measure cavity from and surface-group needs the document's group
/// lattice, and both are callbacks on the C++ side that a flat descriptor
/// cannot carry. The header is explicit that setting their bits from C is
/// *inert rather than an error*, and ClayCore v0.78.0 names the pair among its
/// known limits, unchanged from v0.73.0. They are surfaced here anyway, and
/// deliberately: a factor that silently does nothing is worse hidden than
/// named, and `two_of_the_five_automask_factors_are_declared_and_inert` in
/// `tests/mesh_automask.rs` is the tripwire that says so out loud and fails
/// the day the descriptor carrying their inputs lands.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Automask {
    /// How far the surface may turn from the brush's own facing before the
    /// gate closes, in **radians** — full strength up to this angle, zero at
    /// twice it.
    ///
    /// `None` leaves the factor off entirely. `Some(0.0)` asks for it at the
    /// engine's own default angle, which is what a zero in the descriptor
    /// reads as.
    pub normal_angle: Option<f32>,
    /// Reach only what the stroke's own starting region is connected to.
    pub topology_connected: bool,
    /// How many rings of fade to leave at an open border. `None` leaves the
    /// factor off.
    ///
    /// **Non-zero, because zero cannot cross this ABI.** The engine's own
    /// vocabulary has a rings count of zero meaning a hard stop at the border
    /// itself rather than a fade into it — a distinct setting, and one
    /// `clay_c.cpp` discards: it copies the field only `if
    /// (d.automask_boundary_rings > 0)`, so a zero written here would leave
    /// the engine's default of **two rings of fade** standing with the factor
    /// bit set and nothing saying so. Measured on a 16x16 sheet, `Some(0)`
    /// used to move the surface by exactly what `Some(2)` did, to the bit. A
    /// host that needs the hard stop has to gate the border itself.
    pub boundary_rings: Option<NonZeroU32>,
    /// How much of the measured cavity to apply, in `0..=1`.
    ///
    /// **Inert from C.** See the type's own note: the input this measures
    /// against does not cross the ABI, so the bit is accepted and nothing
    /// happens. Kept so the vocabulary is complete and the gap is nameable.
    ///
    /// A strength of zero is the factor contributing nothing, which is what
    /// the factor being absent already means — so it is written as absent. The
    /// descriptor cannot carry it either way: `clay_c.cpp` copies the field
    /// only `if (d.automask_cavity_strength > 0.0f)`, so a zero asked for here
    /// would arrive as the engine's default of full strength.
    pub cavity_strength: Option<f32>,
    /// Stay inside the polygroup the stroke started in.
    ///
    /// **Inert from C**, for the same reason as [`Self::cavity_strength`].
    pub surface_group: bool,
}

impl Automask {
    /// Writes the four descriptor fields this covers.
    ///
    /// The bit set is assembled from what is actually asked for rather than
    /// carried as a separate field, so a caller cannot set a factor's bit and
    /// leave its parameter unset, or vice versa.
    fn write_into(&self, raw: &mut sys::clay_mesh_brush_desc) {
        let mut factors = 0u32;
        if let Some(angle) = self.normal_angle {
            factors |= sys::clay_automask_factor::CLAY_AUTOMASK_NORMAL_ANGLE;
            raw.automask_normal_angle = angle;
        }
        if self.topology_connected {
            factors |= sys::clay_automask_factor::CLAY_AUTOMASK_TOPOLOGY_CONNECTED;
        }
        if let Some(rings) = self.boundary_rings {
            factors |= sys::clay_automask_factor::CLAY_AUTOMASK_BOUNDARY;
            raw.automask_boundary_rings = rings.get() as i32;
        }
        // Only where the engine will keep it. Both of these fields are copied
        // across a `> 0` guard on the other side, so writing a factor's bit
        // beside a parameter that guard discards is exactly the state this
        // function's own contract says cannot happen: the bit set and the
        // parameter unset, with the engine's default standing in for what was
        // asked for.
        if let Some(strength) = self.cavity_strength.filter(|s| *s > 0.0) {
            factors |= sys::clay_automask_factor::CLAY_AUTOMASK_CAVITY;
            raw.automask_cavity_strength = strength;
        }
        if self.surface_group {
            factors |= sys::clay_automask_factor::CLAY_AUTOMASK_SURFACE_GROUP;
        }
        raw.automask_factors = factors;
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
    /// How many Laplacian passes a smoothing verb runs, 1..=64.
    ///
    /// `None` leaves the engine's own default. One pass averages a vertex with
    /// its one-ring, which smooths at the scale of a single edge rather than
    /// at the scale of the brush — on a dense mesh that is a change a sculptor
    /// cannot see.
    pub smooth_iterations: Option<i32>,
    /// How far the stamp's in-plane axes are turned about its own facing, in
    /// radians.
    ///
    /// The grain. It is what makes a rake, a chisel, clay strips, a
    /// directional scratch and a rotated alpha one axis over a frame rather
    /// than five code paths: a stroke resolver that knows the direction of
    /// travel sets this, and no verb has to know that it did.
    ///
    /// Zero is *no rotation at all* rather than a rotation by zero, and the
    /// engine branches on that: turning a basis by cos 0 and sin 0 leaves a
    /// `-0.0` where an unrotated axis has `+0.0`. Zero is therefore the
    /// default and what every existing caller keeps sending.
    ///
    /// Observable only through an alpha or a directional kernel — a round
    /// brush has nothing to orient.
    pub stamp_azimuth: f32,
    /// Where the surface walk starts, and the class space that was picked in.
    ///
    /// `None` tells the engine to search, which is a linear scan and the wrong
    /// thing to do per stamp on a large mesh. A caller that has already picked
    /// knows the answer and should say so — see [`MeshSeed`] for why the class
    /// alone is not enough to say it with.
    pub seed: Option<MeshSeed>,
    /// The gates the brush applies to itself.
    ///
    /// [`Automask::default()`] is no gate at all, which the engine documents
    /// as bit-identical to a descriptor from before automasking existed.
    pub automask: Automask,
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
            smooth_iterations: None,
            stamp_azimuth: 0.0,
            seed: None,
            automask: Automask::default(),
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
    pub(crate) fn as_raw(&self) -> sys::clay_mesh_brush_desc {
        // The engine's defaults first, then what this stamp means — which is
        // the arrangement `clay_mesh_brush_defaults` exists for: "so a host
        // fills in what it means and takes the rest".
        //
        // Starting from a zeroed descriptor took *nothing* instead, and the
        // fields this type does not name are not all harmlessly zero:
        //
        //   polish_angle       0 is a fully closed gate, so POLISH smoothed
        //                      nothing anywhere — measured, it moved not one
        //                      vertex even across a crease cut for it
        //   layer_height       0 is a zero ceiling, so LAYER deposited almost
        //                      nothing — measured at 0.0086 against DRAW's
        //                      0.6778 on the same stroke
        //   smooth_iterations  documented as 1..MAX, and 0 is neither
        //
        // A failure to read them is not fatal: the zeroed descriptor is what
        // this did before, and it still carries a valid struct_size.
        let mut raw = sys::clay_mesh_brush_desc::sized();
        // SAFETY: a valid versioned descriptor out-parameter, whose
        // struct_size is set above as the boundary requires.
        let _ = unsafe { sys::clay_mesh_brush_defaults(&mut raw) };
        raw.verb = self.verb.to_raw();
        raw.center = self.center;
        raw.radius = self.radius;
        raw.strength = self.strength;
        raw.falloff = self.falloff.to_raw();
        raw.direction = self.direction;
        raw.geodesic = i32::from(self.geodesic);
        raw.stamp_azimuth = self.stamp_azimuth;
        self.automask.write_into(&mut raw);
        // The class and the token travel together or not at all. Without a
        // seed the engine searches: a linear scan, and the wrong thing to do
        // per stamp on a large mesh — but a wrong seed is worse than a slow
        // one, and a zero token is how a caller says it claims nothing and
        // keeps the bounds check that was always there.
        match self.seed {
            Some(seed) => {
                raw.seed_class = seed.class;
                raw.seed_revision = seed.revision;
            }
            None => {
                raw.seed_class = sys::CLAY_MESH_NO_CLASS;
                raw.seed_revision = 0;
            }
        }
        raw.color = self.colour;
        if let Some(iterations) = self.smooth_iterations {
            raw.smooth_iterations = iterations.clamp(1, 64);
        }
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

    /// What the cage moves a point by — exactly zero everywhere for an
    /// untouched cage.
    ///
    /// The forward warp, read without applying it. A host that draws a surface
    /// it did not get from this lattice — a field's, marched from the brick
    /// cache — can show what the cage would do to it by displacing the
    /// vertices it already has.
    pub fn displacement(&self, point: [f32; 3]) -> Result<[f32; 3]> {
        let mut out = [0.0; 3];
        // SAFETY: valid handle, an array of three floats in and three out,
        // written on success.
        check(
            unsafe {
                sys::clay_mesh_lattice_displacement(
                    self.raw.as_ptr(),
                    point.as_ptr(),
                    out.as_mut_ptr(),
                )
            },
            "clay_mesh_lattice_displacement",
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

/// Where a stamp's surface walk starts.
///
/// The class and the token it was numbered in, and the pair is the point: a
/// seed is an INDEX, and an index outlives the numbering it was taken from. A
/// class picked against one sculptor is comfortably in bounds against the
/// next, so a bounds check sees nothing — and what a stale seed costs is not a
/// slightly misplaced dab. The surface walk returns an *empty* region when the
/// seed lies farther than the radius from the centre, so the stamp is lost
/// whole and "nothing moved" reads exactly like a fully masked stroke.
///
/// So the two are one value here rather than two fields a caller could carry
/// half of. Sending the token turns that silence into a rejected seed and a
/// scan — one stamp slower, and correct — which
/// [`MeshSculptor::stale_seeds_rejected`] counts so a host sees it happen
/// rather than infers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshSeed {
    /// The weld class to start the walk at.
    pub class: u32,
    /// The class space that class was picked in.
    pub revision: u64,
}

/// Where a ray met a mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshHit {
    pub position: [f32; 3],
    /// World space, unit length.
    pub normal: [f32; 3],
    /// The weld class the ray landed on, ready to seed a stamp's surface walk.
    pub seed_class: u32,
    /// The class space [`Self::seed_class`] was picked in.
    ///
    /// Handed back beside the class rather than fetched separately, because
    /// keeping one without the other is keeping the half that cannot be
    /// checked.
    pub seed_revision: u64,
}

impl MeshHit {
    /// The seed this hit picked, ready to hand to a stamp.
    pub fn seed(&self) -> MeshSeed {
        MeshSeed {
            class: self.seed_class,
            revision: self.seed_revision,
        }
    }
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
    /// The raw handle, for sibling modules in this crate only.
    pub(crate) fn as_ptr(&self) -> *mut sys::clay_mesh_sculptor {
        self.raw.as_ptr()
    }

    pub fn class_count(&self) -> Result<usize> {
        let mut count = 0;
        // SAFETY: as above.
        check(
            unsafe { sys::clay_mesh_sculptor_class_count(self.raw.as_ptr(), &mut count) },
            "clay_mesh_sculptor_class_count",
        )?;
        Ok(count)
    }

    /// The token this sculptor's weld classes are numbered in.
    ///
    /// Constant for the life of the handle: the adjacency is built once and
    /// nothing rebuilds it, so vertices moving under a stroke leave the token
    /// alone and a seed picked at pointer-down stays valid for every stamp of
    /// the gesture — which is exactly when re-picking would be wasted work.
    /// What retires a token is a *new* sculptor, which is why this is worth
    /// storing beside a class rather than assuming.
    ///
    /// [`MeshHit`] hands the same token back beside the class it picked, so a
    /// caller that picks needs this only where it seeds a stamp without one.
    pub fn seed_revision(&self) -> Result<u64> {
        let mut revision = 0u64;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_mesh_sculptor_seed_revision(self.raw.as_ptr(), &mut revision) },
            "clay_mesh_sculptor_seed_revision",
        )?;
        Ok(revision)
    }

    /// How many stamps rejected a seed because its token did not match, over
    /// the life of this sculptor.
    ///
    /// The number that makes a rejection observable. Without it a seed that
    /// was refused and one that was accepted and happened to be harmless are
    /// the same event from outside, and neither a host nor a test can tell
    /// them apart.
    pub fn stale_seeds_rejected(&self) -> Result<usize> {
        let mut count = 0;
        // SAFETY: as above.
        check(
            unsafe { sys::clay_mesh_sculptor_stale_seeds_rejected(self.raw.as_ptr(), &mut count) },
            "clay_mesh_sculptor_stale_seeds_rejected",
        )?;
        Ok(count)
    }

    /// One stamp. Returns how many welded classes moved.
    ///
    /// Zero for a stamp that reached nothing, that was fully masked, or whose
    /// settings amount to no displacement — all three are ordinary outcomes
    /// rather than failures.
    pub fn stamp(
        &mut self,
        stamp: MeshStamp<'_>,
        mask: Option<&MaskField>,
        deltas: Option<&mut MeshDeltas>,
    ) -> Result<usize> {
        let desc = stamp.as_raw();
        let mut moved = 0;
        // SAFETY: valid handle and a descriptor carrying its own size; the
        // mask and the deltas are each either a valid handle or null, both of
        // which the entry point allows.
        check(
            unsafe {
                sys::clay_mesh_sculptor_stamp(
                    self.raw.as_ptr(),
                    &desc,
                    mask.map_or(std::ptr::null(), |m| m.as_ptr() as *const _),
                    deltas.map_or(std::ptr::null_mut(), |d| d.raw.as_ptr()),
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
    ///
    /// `defer_normals` is **not** [`MeshSculptor::set_defer_normals`] and the
    /// two must not be read as one switch. This one is scoped to the call: the
    /// entry point sets the sculptor's flag to this argument for the length of
    /// the stroke it is resolving, recomputes once at the end into `deltas`,
    /// and restores whatever the flag was — because here the library knows
    /// where the stroke ended. So it costs a caller no obligation, and it is
    /// what de-duplicates the overlapping dabs of one resolved stroke. The
    /// member flag is the other thing entirely: it spans calls, it is what
    /// [`MeshSculptor::stamp`] and the two whole-form verbs read, and a caller
    /// that sets it owes the flush itself.
    pub fn apply_stroke(
        &mut self,
        samples: &[[f32; 5]],
        preset: &crate::StrokePreset,
        stamp: MeshStamp<'_>,
        mask: Option<&MaskField>,
        defer_normals: bool,
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
        // mask and the frame are nullable and the deltas are null.
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
                    i32::from(defer_normals),
                    deltas.map_or(std::ptr::null_mut(), |d| d.raw.as_ptr()),
                    &mut applied,
                )
            },
            "clay_mesh_sculptor_apply_stroke",
        )?;
        Ok(applied)
    }

    /// Whether stamps leave their normals for a flush instead of recomputing
    /// them as they go.
    pub fn defer_normals(&self) -> Result<bool> {
        let mut defer = 0;
        // SAFETY: valid handle, out-parameter written on success.
        check(
            unsafe { sys::clay_mesh_sculptor_defer_normals(self.raw.as_ptr(), &mut defer) },
            "clay_mesh_sculptor_defer_normals",
        )?;
        Ok(defer != 0)
    }

    /// Leaves the normals of everything stamped from here on for a later
    /// [`MeshSculptor::flush_normals`], instead of recomputing them per stamp.
    ///
    /// What it buys is the recompute of a gesture's overlapping dabs done once
    /// instead of once per dab; what it costs is that the shading lags the
    /// geometry until the flush, and the *obligation*:
    ///
    /// **A caller that defers must flush.** Nothing flushes on its own. The
    /// sculptor does not know where a gesture ends, and guessing at it — a
    /// timer, a stamp with no predecessor — would flush mid-drag, which is the
    /// whole of what deferring exists to avoid. This wrapper does not guess
    /// either: it is a plain switch, and the thing that makes the flush
    /// unskippable belongs with whatever owns the gesture. In this workspace
    /// that is `clayspace_engine`'s `LiveMesh`, which holds the record and the
    /// handle as one value so that dropping it settles.
    ///
    /// The final state is exact either way. Deferring changes *when* the work
    /// happens and nothing about the result, which is what keeps a committed
    /// sculpt from being a function of machine speed.
    ///
    /// [`MeshSculptor::apply_stroke`]'s own argument is a different thing —
    /// see it for why.
    pub fn set_defer_normals(&mut self, defer: bool) -> Result<()> {
        // SAFETY: valid handle.
        check(
            unsafe {
                sys::clay_mesh_sculptor_set_defer_normals(self.raw.as_ptr(), i32::from(defer))
            },
            "clay_mesh_sculptor_set_defer_normals",
        )
    }

    /// Recomputes the normals deferred since the last flush.
    ///
    /// Coalesced: the classes are sorted and made unique first, so a gesture
    /// that passed over the same vertex forty times recomputes it once. That
    /// de-duplication is the entire performance argument, and it is worth
    /// exactly the overlap between the dabs of one deferred window.
    ///
    /// **`deltas` must be the record the stamps were noted into**, or the
    /// deferred gesture's undo is not exact: the record captures a vertex's
    /// normal the first time it is seen, so a flush into a *fresh* record
    /// captures the already-moved normals as the "before" and the undo then
    /// restores post-gesture shading. `None` is correct only where the stamps
    /// recorded into no record either.
    ///
    /// A no-op where nothing was deferred, which is what makes it safe to call
    /// on every exit rather than only on the ones that deferred something.
    pub fn flush_normals(&mut self, deltas: Option<&mut MeshDeltas>) -> Result<()> {
        // SAFETY: valid handle; the record is either a valid handle or null,
        // both of which the entry point allows.
        check(
            unsafe {
                sys::clay_mesh_sculptor_flush_normals(
                    self.raw.as_ptr(),
                    deltas.map_or(std::ptr::null_mut(), |d| d.raw.as_ptr()),
                )
            },
            "clay_mesh_sculptor_flush_normals",
        )
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
            seed_revision: hit.seed_revision,
        }))
    }

    /// Applies a whole-form deformer to every vertex.
    ///
    /// Not a brush: it takes no position, because a deformer states something
    /// about the form rather than about a dab.
    pub fn deform(
        &mut self,
        deformer: MeshDeformer,
        mask: Option<&MaskField>,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A factor's bit is set only where the parameter beside it will survive
    /// the crossing.
    ///
    /// `clay_c.cpp` copies `automask_boundary_rings` and
    /// `automask_cavity_strength` into the engine's settings only where each
    /// is greater than zero, and leaves its own default standing otherwise —
    /// two rings of fade, and full cavity strength. So a descriptor carrying a
    /// bit beside a zero does not ask for nothing, it asks for whatever the
    /// engine already had, with the bit set so nothing looks wrong. Measured
    /// on a 16x16 unit sheet before this was repaired, one Draw stamp under
    /// `boundary_rings: Some(0)` moved the surface to the bit exactly as
    /// `Some(2)` did, and sat on the wrong side of `Some(1)`.
    ///
    /// Rings are a `NonZeroU32` now, so half of this is the type's; the other
    /// half is a strength of zero, which is written as the factor being absent
    /// because a factor contributing nothing and an absent factor are the same
    /// surface.
    #[test]
    fn no_factor_bit_is_set_beside_a_parameter_the_abi_would_discard() {
        let asked = [
            Automask {
                cavity_strength: Some(0.0),
                ..Automask::default()
            },
            Automask {
                cavity_strength: Some(-1.0),
                ..Automask::default()
            },
            Automask {
                cavity_strength: Some(0.5),
                boundary_rings: NonZeroU32::new(3),
                ..Automask::default()
            },
            Automask {
                boundary_rings: NonZeroU32::new(1),
                ..Automask::default()
            },
        ];
        for automask in asked {
            let mut raw = sys::clay_mesh_brush_desc::sized();
            automask.write_into(&mut raw);
            if raw.automask_factors & sys::clay_automask_factor::CLAY_AUTOMASK_CAVITY != 0 {
                assert!(
                    raw.automask_cavity_strength > 0.0,
                    "the cavity bit is set beside {}, which the engine discards for its own \
                     default of full strength",
                    raw.automask_cavity_strength
                );
            }
            if raw.automask_factors & sys::clay_automask_factor::CLAY_AUTOMASK_BOUNDARY != 0 {
                assert!(
                    raw.automask_boundary_rings > 0,
                    "the boundary bit is set beside {} rings, which the engine discards for its \
                     own default of two",
                    raw.automask_boundary_rings
                );
            }
        }
    }
}
