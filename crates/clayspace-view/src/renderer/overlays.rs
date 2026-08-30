//! The geometry the viewport draws that is not the sculpt.
//!
//! The grid, the symmetry planes, the brush cursor, the ZSphere rig and its
//! membrane, the lattice cage, the manipulator and the orientation gizmo. Every
//! one of them is built here as plain vertices and indices and handed to the
//! renderer like any other mesh, which is what lets them be drawn by the same
//! pipelines and captured by the same tests.
//!
//! Its own file because none of it touches the renderer's state: these are
//! functions from a description of a thing to the triangles or lines that draw
//! it, and they were seven hundred lines in the middle of a file about
//! pipelines and passes. What decides *which* of them a frame draws, and in
//! what order, stays next to the frame.

use glam::Vec3;

use super::{
    ArmatureView, BrushCursor, GizmoView, LatticeView, Overlays, SymmetryAxis, Vertex, ACTIVE_TINT,
};
use crate::palette;
use clayspace_model::{GizmoHandle, GizmoMode};

/// Builds the grid and symmetry-plane line geometry.
///
/// Overlays are drawn low-contrast and behind the sculpt in visual weight, and
/// are excluded from every export — they exist only in this function.
pub(super) fn overlay_geometry(overlays: Overlays, extent: f32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let mut line = |a: Vec3, b: Vec3, color: [f32; 3]| {
        let base = vertices.len() as u32;
        vertices.push(Vertex {
            position: a.into(),
            normal: [0.0, 1.0, 0.0],
            color,
            mask: 0.0,
        });
        vertices.push(Vertex {
            position: b.into(),
            normal: [0.0, 1.0, 0.0],
            color,
            mask: 0.0,
        });
        indices.extend_from_slice(&[base, base + 1]);
    };

    if overlays.grid {
        let steps = 20;
        let step = extent * 2.0 / steps as f32;
        // One and two steps up from the ground. Written in linear, because
        // the target encodes: passing the design's hex values straight through
        // renders them several times too bright.
        let minor = palette::GRID_MINOR;
        let axis = palette::GRID_AXIS;
        for i in 0..=steps {
            let t = -extent + i as f32 * step;
            let color = if i == steps / 2 { axis } else { minor };
            line(Vec3::new(t, 0.0, -extent), Vec3::new(t, 0.0, extent), color);
            line(Vec3::new(-extent, 0.0, t), Vec3::new(extent, 0.0, t), color);
        }
    }

    for axis in [SymmetryAxis::X, SymmetryAxis::Y, SymmetryAxis::Z] {
        if !overlays.symmetry_planes[axis as usize] {
            continue;
        }
        // The accent, because the symmetry plane is tool state rather than
        // scene furniture — but dimmed, since a reference overlay must not be
        // the brightest thing on screen. At 0.25 over an eight-by-eight grid
        // it was: the capture showed a bright orange wall with the sculpt
        // behind it. Four steps was still a lattice of orange across the
        // form on a running build, with the camera inside the plane's extent.
        // Two steps is the plane's outline and its two centre lines — the
        // mirror's axis where it meets the floor, and its edge — which says
        // "the mirror is here" and puts nothing across the clay. Six lines
        // can afford a little more light than forty: still a fifth of the
        // accent, nowhere near the active brush's ring.
        let color = palette::dimmed(palette::ACCENT, 0.22);
        let steps = 2;
        let step = extent * 2.0 / steps as f32;
        for i in 0..=steps {
            let t = -extent + i as f32 * step;
            let (a, b, c, d) = match axis {
                SymmetryAxis::X => (
                    Vec3::new(0.0, t, -extent),
                    Vec3::new(0.0, t, extent),
                    Vec3::new(0.0, -extent, t),
                    Vec3::new(0.0, extent, t),
                ),
                SymmetryAxis::Y => (
                    Vec3::new(t, 0.0, -extent),
                    Vec3::new(t, 0.0, extent),
                    Vec3::new(-extent, 0.0, t),
                    Vec3::new(extent, 0.0, t),
                ),
                SymmetryAxis::Z => (
                    Vec3::new(t, -extent, 0.0),
                    Vec3::new(t, extent, 0.0),
                    Vec3::new(-extent, t, 0.0),
                    Vec3::new(extent, t, 0.0),
                ),
            };
            line(a, b, color);
            line(c, d, color);
        }
    }

    (vertices, indices)
}

/// A ring on the surface, plus a mark at its centre.
///
/// The accent colour, because this is the active brush — the one thing the
/// design reserves it for.
pub(super) fn cursor_geometry(cursor: BrushCursor) -> (Vec<Vertex>, Vec<u32>) {
    const SEGMENTS: usize = 48;

    let centre = Vec3::from(cursor.position);
    let normal = {
        let n = Vec3::from(cursor.normal);
        if n.length_squared() > 1e-6 {
            n.normalize()
        } else {
            Vec3::Y
        }
    };
    // Any pair perpendicular to the normal will do; picking the axis least
    // aligned with it avoids a degenerate cross product.
    let reference = if normal.x.abs() < 0.9 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let u = normal.cross(reference).normalize() * cursor.radius;
    let v = normal.cross(u).normalize() * cursor.radius;

    // A mirror is where the stroke also lands, not where the hand is. Dimming
    // it keeps the two readable as different things at a glance.
    let color = if cursor.mirrored {
        palette::dimmed(palette::ACCENT, 0.45)
    } else {
        palette::ACCENT
    };
    let mut vertices = Vec::with_capacity(SEGMENTS + 4);
    let mut indices = Vec::with_capacity(SEGMENTS * 2 + 4);

    for i in 0..SEGMENTS {
        let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let (s, c) = angle.sin_cos();
        // Lifted a hair along the normal so the ring is not swallowed by the
        // surface it sits on.
        let point = centre + u * c + v * s + normal * (cursor.radius * 0.02);
        vertices.push(Vertex {
            position: point.into(),
            normal: normal.into(),
            color,
            mask: 0.0,
        });
        indices.push(i as u32);
        indices.push(((i + 1) % SEGMENTS) as u32);
    }

    // A small cross at the centre, so the exact point is readable when the
    // ring is large.
    let tick = cursor.radius * 0.12;
    let base = vertices.len() as u32;
    for (a, b) in [
        (u.normalize() * tick, -u.normalize() * tick),
        (v.normalize() * tick, -v.normalize() * tick),
    ] {
        let offset = normal * (cursor.radius * 0.02);
        for point in [centre + a + offset, centre + b + offset] {
            vertices.push(Vertex {
                position: point.into(),
                normal: normal.into(),
                color,
                mask: 0.0,
            });
        }
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base + 3]);

    (vertices, indices)
}

/// A tapered sleeve along each link, which is the membrane a rig would skin
/// into.
///
/// ZBrush shows this while a rig is being built and shows it translucent, so
/// the chain reads through its own surface. Eight sides is enough at the size
/// a link is drawn — this is a hint about where the skin will go, not the skin.
pub(super) fn membrane_geometry(view: &ArmatureView<'_>) -> (Vec<Vertex>, Vec<u32>) {
    const SIDES: usize = 8;
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for (child, parent) in view.links {
        let (Some((a, ra)), Some((b, rb))) = (
            view.spheres.get(*child as usize),
            view.spheres.get(*parent as usize),
        ) else {
            continue;
        };
        let (from, to) = (Vec3::from(*a), Vec3::from(*b));
        let axis = to - from;
        let length = axis.length();
        if length < 1e-5 {
            continue;
        }
        let forward = axis / length;
        // Any vector not along the axis gives a frame to sweep the ring in.
        let aside = if forward.x.abs() < 0.9 {
            Vec3::X
        } else {
            Vec3::Y
        };
        let u = forward.cross(aside).normalize();
        let v = forward.cross(u);

        let colour = palette::dimmed(palette::ACCENT, 0.5);
        let base = vertices.len() as u32;
        for side in 0..SIDES {
            let angle = side as f32 / SIDES as f32 * std::f32::consts::TAU;
            let (s, c) = angle.sin_cos();
            let offset = u * c + v * s;
            // Slightly inside each sphere, so the sleeve meets them rather
            // than poking out of their silhouettes.
            for (centre, radius) in [(from, *ra), (to, *rb)] {
                vertices.push(Vertex {
                    position: (centre + offset * radius * 0.72).into(),
                    normal: offset.into(),
                    color: colour,
                    mask: 0.0,
                });
            }
        }
        for side in 0..SIDES {
            let next = (side + 1) % SIDES;
            let (a0, a1) = (base + side as u32 * 2, base + side as u32 * 2 + 1);
            let (b0, b1) = (base + next as u32 * 2, base + next as u32 * 2 + 1);
            // Both windings, because the sleeve is seen from inside as often
            // as outside and this pipeline does not cull.
            indices.extend_from_slice(&[a0, a1, b1, a0, b1, b0]);
        }
    }

    (vertices, indices)
}

/// Three rings and a cross per sphere, and a line per link.
///
/// Three rings rather than one: a single ring lies in the view plane and a rig
/// then reads as flat, which is exactly the information a rig has to convey.
/// The cage: a line along every edge, and a box at every control point.
///
/// Line topology, drawn by the overlay pipeline. The handles are boxes rather
/// than spheres because a box reads as a *handle* — something to grab — where a
/// sphere at this size reads as a bead on a wire, and because twelve lines cost
/// what one sphere's ring costs.
/// The three axis colours, which every application that has a manipulator
/// spells the same way: x red, y green, z blue.
pub(super) const AXIS_COLOURS: [[f32; 3]; 3] =
    [[0.85, 0.24, 0.24], [0.36, 0.76, 0.30], [0.28, 0.45, 0.88]];

/// The manipulator: three axes and, where the mode has one, a centre.
///
/// Line topology like the cage, and shapes rather than colours alone carry the
/// meaning — an arrow slides, a ring turns, a box scales — because a person
/// reaching for a handle is not reading a legend, and because the three
/// colours are the one part of this a colour-blind sculptor cannot use.
pub(super) fn gizmo_geometry_for(
    view: GizmoView,
    emit: &mut impl FnMut(Vec3, Vec3, [f32; 3]),
    triangle: &mut impl FnMut(Vec3, Vec3, Vec3, [f32; 3]),
) {
    const RING_SEGMENTS: usize = 40;
    let pivot = Vec3::from(view.pivot);

    // Drawn heavier than the cage it stands on. A line is one pixel wide
    // whatever the device, and a one-pixel manipulator over a shaded form is
    // a thing to squint for; ZBrush's is a handle. Each stroke is laid down
    // `HANDLE_WEIGHT` times, stepped *across itself in the screen plane* —
    // perpendicular both to the stroke and to the eye — so it widens the same
    // way from every angle, and a box's edges thicken rather than hatch.
    let eye = normalized(Vec3::from(view.view_axis)).unwrap_or(Vec3::Z);
    let step = view.reach * HANDLE_STEP;
    let mut segment = |from: Vec3, to: Vec3, colour: [f32; 3]| {
        // A stroke pointing straight at the eye has no across; any direction
        // in the screen plane widens it as well as another.
        let across = normalized(eye.cross(to - from)).unwrap_or_else(|| frame_about(eye).0);
        for i in 0..HANDLE_WEIGHT {
            let t = i as f32 - (HANDLE_WEIGHT - 1) as f32 * 0.5;
            let offset = across * (t * step);
            emit(from + offset, to + offset, colour);
        }
    };
    let lit = |operation: GizmoMode, handle: GizmoHandle, base: [f32; 3]| {
        if view.hovered == Some((operation, handle)) {
            [1.0, 0.85, 0.4]
        } else {
            base
        }
    };
    let ring = |centre: Vec3,
                across: Vec3,
                other: Vec3,
                radius: f32,
                colour: [f32; 3],
                segment: &mut dyn FnMut(Vec3, Vec3, [f32; 3])| {
        for step in 0..RING_SEGMENTS {
            let angle = |at: usize| at as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
            let at = |a: f32| centre + (across * a.cos() + other * a.sin()) * radius;
            segment(at(angle(step)), at(angle(step + 1)), colour);
        }
    };

    // One widget, every operation: ZBrush's Gizmo 3D. Along each axis an
    // arrow that slides, a ring that turns and — where a stretch can be
    // applied per axis — a box that scales, so the operation is chosen by the
    // handle grabbed rather than by a mode set first. Three modes drew three
    // different widgets once, and the chips became a step a sculptor had to
    // take before every move.
    for (operation, handle) in GizmoHandle::combined(view.per_axis_scale) {
        let Some(index) = handle.axis_index() else {
            continue;
        };
        let colour = lit(operation, handle, AXIS_COLOURS[index]);
        let mut unit = Vec3::ZERO;
        unit[index] = 1.0;
        let (u, v) = ((index + 1) % 3, (index + 2) % 3);
        let mut across = Vec3::ZERO;
        across[u] = 1.0;
        let mut other = Vec3::ZERO;
        other[v] = 1.0;
        match operation {
            GizmoMode::Move => {
                // A cone at the tip: a handle, not a hint of one. The shaft
                // stops where the cone starts so it does not show through the
                // base.
                let tip = pivot + unit * view.reach;
                let head = view.reach * 0.2;
                segment(pivot, tip - unit * head, colour);
                cone(tip, unit, head, head * 0.4, colour, triangle);
            }
            GizmoMode::Rotate => {
                // A ring in the plane perpendicular to the axis, inside the
                // arrows' reach so the two are told apart by radius as well as
                // by shape.
                ring(
                    pivot,
                    across,
                    other,
                    view.reach * RING_REACH,
                    colour,
                    &mut segment,
                );
            }
            GizmoMode::Scale => {
                // A box on the shaft, short of the ring.
                let at = pivot + unit * (view.reach * SCALE_BOX_REACH);
                solid_cube(at, view.reach * 0.07, colour, triangle);
            }
        }
    }

    // The centre: a solid block at the pivot, which reads as a centre from
    // any angle. What it does is the mode's — a slide, or a uniform scale.
    let centre_operation = GizmoHandle::centre_operation(view.mode);
    let colour = lit(centre_operation, GizmoHandle::Centre, CENTRE_COLOUR);
    solid_cube(pivot, view.reach * 0.12, colour, triangle);

    // The outer ring: ZBrush's, and the one a sculptor reaches for most.
    // Outside the arrows at `VIEW_RING_REACH` — among the axis rings it would
    // be a fourth thing to tell apart at the same radius, and the whole point
    // of this one is that it is the easy target. And the four corner brackets
    // that frame it in the screen plane: they say "this is the widget's
    // extent" and are grabbed by nothing.
    let (across, other) = frame_about(eye);
    let colour = lit(GizmoMode::Rotate, GizmoHandle::View, VIEW_RING_COLOUR);
    ring(
        pivot,
        across,
        other,
        view.reach * VIEW_RING_REACH,
        colour,
        &mut segment,
    );
    let half = view.reach * BRACKET_REACH;
    let arm = view.reach * 0.22;
    for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let corner = pivot + (across * sx + other * sy) * half;
        segment(corner, corner - across * (sx * arm), BRACKET_COLOUR);
        segment(corner, corner - other * (sy * arm), BRACKET_COLOUR);
    }
}

/// How far out the axis rings sit, against the arrows' reach.
pub const RING_REACH: f32 = 0.8;
/// Where a per-axis scale box sits along its arrow, against the reach.
pub const SCALE_BOX_REACH: f32 = 0.55;
/// Half the side of the corner-bracket square, against the reach.
pub const BRACKET_REACH: f32 = 1.42;
/// The centre block's colour: not an axis colour, and not the outer ring's.
pub(super) const CENTRE_COLOUR: [f32; 3] = [0.82, 0.78, 0.42];
/// The brackets, quiet: they frame the widget and are not a handle.
pub(super) const BRACKET_COLOUR: [f32; 3] = [0.55, 0.55, 0.58];

/// How far out the outer ring sits, against an axis ring's reach.
pub const VIEW_RING_REACH: f32 = 1.28;

/// How many passes each manipulator stroke is drawn in.
pub(super) const HANDLE_WEIGHT: usize = 3;
/// How far apart those passes sit, against the manipulator's reach.
pub(super) const HANDLE_STEP: f32 = 0.006;

/// Not one of the three axis colours: the outer ring belongs to no axis, and
/// borrowing red, green or blue would say it did.
pub(super) const VIEW_RING_COLOUR: [f32; 3] = [0.82, 0.78, 0.42];

/// A unit vector, or `None` where there is no direction to have.
pub(super) fn normalized(v: Vec3) -> Option<Vec3> {
    (v.length() > 1e-6).then(|| v / v.length())
}

/// Two unit vectors spanning the plane perpendicular to an axis.
///
/// The domain's, in this crate's vector type. One implementation rather than
/// two: the ring is *drawn* from this frame and *dragged* on a plane built
/// from the same one, and two copies could disagree.
pub fn frame_about(axis: Vec3) -> (Vec3, Vec3) {
    let (across, other) = clayspace_model::perpendicular_frame(axis.into());
    (across.into(), other.into())
}

/// The twelve edges of a cube, spelled as the four along each axis.
pub(super) fn cube(
    centre: Vec3,
    size: f32,
    colour: [f32; 3],
    segment: &mut impl FnMut(Vec3, Vec3, [f32; 3]),
) {
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        for corner in 0..4 {
            let mut from = [0.0f32; 3];
            from[u] = if corner & 1 == 0 { -size } else { size };
            from[v] = if corner & 2 == 0 { -size } else { size };
            from[axis] = -size;
            let mut to = from;
            to[axis] = size;
            segment(centre + Vec3::from(from), centre + Vec3::from(to), colour);
        }
    }
}

/// The twelve edges of an axis-aligned box.
///
/// Two callers now — a selected object and the active subtool — and the corner
/// arithmetic is the part that is easy to get subtly wrong, so it is written
/// once.
pub(super) fn outline_box(
    (min, max): ([f32; 3], [f32; 3]),
    colour: [f32; 3],
    segment: &mut impl FnMut(Vec3, Vec3, [f32; 3]),
) {
    let corner = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { min[0] } else { max[0] },
            if i & 2 == 0 { min[1] } else { max[1] },
            if i & 4 == 0 { min[2] } else { max[2] },
        )
    };
    // Every pair of corners differing in one bit, which is every pair one axis
    // apart.
    for a in 0..8usize {
        for bit in [1usize, 2, 4] {
            let b = a | bit;
            if b != a {
                segment(corner(a), corner(b), colour);
            }
        }
    }
}

/// Where the solid handles are lit from, in world space.
///
/// The overlay shader draws vertex colour as it is, so what makes a cone read
/// as a cone is baked here: each face is the handle's colour, darkened by how
/// far it turns from this light. Upper left and toward the eye, as the
/// material previews are lit — but fixed in the world, because the handles
/// are world-aligned and a light that turned with the camera would flatten
/// whichever face happened to face it.
pub(super) const HANDLE_LIGHT: Vec3 = Vec3::new(-0.4, 0.7, 0.6);

/// One face's colour under `HANDLE_LIGHT`, never darker than a little over
/// half, so the shadowed side of a red cone is still red.
pub(super) fn shaded(colour: [f32; 3], a: Vec3, b: Vec3, c: Vec3) -> [f32; 3] {
    let normal = (b - a).cross(c - a);
    let facing = normalized(normal)
        .map(|n| n.dot(HANDLE_LIGHT.normalize()).abs())
        .unwrap_or(0.0);
    let light = 0.55 + 0.45 * facing;
    [colour[0] * light, colour[1] * light, colour[2] * light]
}

/// A cone with its tip at `tip`, pointing along `axis`, `length` long and
/// `radius` wide at the base, closed with a disc.
pub(super) fn cone(
    tip: Vec3,
    axis: Vec3,
    length: f32,
    radius: f32,
    colour: [f32; 3],
    triangle: &mut impl FnMut(Vec3, Vec3, Vec3, [f32; 3]),
) {
    const SEGMENTS: usize = 12;
    let (across, other) = frame_about(axis);
    let base = tip - axis * length;
    let rim = |i: usize| {
        let a = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        base + (across * a.cos() + other * a.sin()) * radius
    };
    for i in 0..SEGMENTS {
        let (p, q) = (rim(i), rim(i + 1));
        triangle(tip, p, q, colour);
        triangle(base, q, p, colour);
    }
}

/// The six faces of a cube, two triangles each.
pub(super) fn solid_cube(
    centre: Vec3,
    size: f32,
    colour: [f32; 3],
    triangle: &mut impl FnMut(Vec3, Vec3, Vec3, [f32; 3]),
) {
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        for side in [-1.0f32, 1.0] {
            let corner = |du: f32, dv: f32| {
                let mut p = [0.0f32; 3];
                p[axis] = side * size;
                p[u] = du * size;
                p[v] = dv * size;
                centre + Vec3::from(p)
            };
            let (a, b, c, d) = (
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            );
            triangle(a, b, c, colour);
            triangle(a, c, d, colour);
        }
    }
}

/// What the cage overlay uploads: the lines, and the solid handles.
pub(super) struct LatticeGeometry {
    pub(super) lines: (Vec<Vertex>, Vec<u32>),
    pub(super) solids: (Vec<Vertex>, Vec<u32>),
}

pub(super) fn lattice_geometry(view: LatticeView<'_>) -> LatticeGeometry {
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut solid_vertices: Vec<Vertex> = Vec::new();
    let mut solid_indices: Vec<u32> = Vec::new();

    let mut segment = |from: Vec3, to: Vec3, color: [f32; 3]| {
        let base = vertices.len() as u32;
        for position in [from, to] {
            vertices.push(Vertex {
                position: position.into(),
                normal: [0.0, 1.0, 0.0],
                color,
                mask: 0.0,
            });
        }
        indices.push(base);
        indices.push(base + 1);
    };
    let mut triangle = |a: Vec3, b: Vec3, c: Vec3, colour: [f32; 3]| {
        let base = solid_vertices.len() as u32;
        let color = shaded(colour, a, b, c);
        for position in [a, b, c] {
            solid_vertices.push(Vertex {
                position: position.into(),
                normal: [0.0, 1.0, 0.0],
                color,
                mask: 0.0,
            });
        }
        solid_indices.extend_from_slice(&[base, base + 1, base + 2]);
    };

    // A selected object's box, quieter still than the cage: it says where a
    // shape is, and a bright one would read as the shape itself.
    const OUTLINE: [f32; 3] = [0.52, 0.62, 0.72];
    /// The active SDF subtool's box, in the same hue its carried siblings are
    /// tinted with — one cue, two mechanisms, and a second colour would read as
    /// a second fact. Dimmed to sit a little below the object outline: which
    /// subtool is active is standing state, and the box a sculptor just put an
    /// object into is the more urgent of the two.
    const SUBTOOL_OUTLINE: [f32; 3] = palette::dimmed(ACTIVE_TINT, 0.68);
    if let Some(box_) = view.outline {
        outline_box(box_, OUTLINE, &mut segment);
    }
    if let Some(box_) = view.subtool_outline {
        outline_box(box_, SUBTOOL_OUTLINE, &mut segment);
    }

    // The cage itself, quiet: it is a frame of reference, and a bright one
    // would compete with the form it is wrapped around.
    const CAGE: [f32; 3] = [0.62, 0.45, 0.28];
    const POINT: [f32; 3] = [0.78, 0.60, 0.38];
    const SELECTED: [f32; 3] = [1.0, 0.72, 0.30];

    for (from, to) in view.edges {
        let (Some(a), Some(b)) = (
            view.points.get(*from as usize),
            view.points.get(*to as usize),
        ) else {
            continue;
        };
        segment(Vec3::from(*a), Vec3::from(*b), CAGE);
    }

    for (index, point) in view.points.iter().enumerate() {
        let selected = view.selected.binary_search(&index).is_ok();
        let color = if selected { SELECTED } else { POINT };
        // Bigger when it is the one in hand, so which point is being dragged
        // is legible without reading the colour — which a sculptor looking at
        // the form is not doing.
        let size = view.handle * if selected { 1.6 } else { 1.0 };
        let centre = Vec3::from(*point);
        cube(centre, size, color, &mut segment);
    }

    // The manipulator last, so it draws over the cage it acts on.
    if let Some(gizmo) = view.gizmo {
        gizmo_geometry_for(gizmo, &mut segment, &mut triangle);
    }

    LatticeGeometry {
        lines: (vertices, indices),
        solids: (solid_vertices, solid_indices),
    }
}

pub(super) fn armature_geometry(view: ArmatureView<'_>) -> (Vec<Vertex>, Vec<u32>) {
    const SEGMENTS: usize = 24;
    /// How far outside the skin the hoops sit.
    ///
    /// At a joint the skin *is* the sphere, so a hoop at the same radius is
    /// coincident with the surface it exists to annotate and vanishes into it.
    /// The first version drew rings flush and the rig was invisible over its
    /// own skin — 0.097 of the frame covered with the scaffolding on, and
    /// 0.097 with it off.
    const PROUD: f32 = 1.05;
    /// And a floor, so a small sphere is still ringed rather than swallowed by
    /// the surface's own thickness.
    const MARGIN: f32 = 0.01;
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut ring = |centre: Vec3, radius: f32, axis: usize, color: [f32; 3]| {
        let base = vertices.len() as u32;
        for i in 0..SEGMENTS {
            let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let (s, c) = angle.sin_cos();
            let offset = match axis {
                0 => Vec3::new(0.0, c, s),
                1 => Vec3::new(c, 0.0, s),
                _ => Vec3::new(c, s, 0.0),
            };
            vertices.push(Vertex {
                position: (centre + offset * radius).into(),
                normal: [0.0, 1.0, 0.0],
                color,
                mask: 0.0,
            });
            indices.push(base + i as u32);
            indices.push(base + ((i + 1) % SEGMENTS) as u32);
        }
    };

    for (index, (position, radius)) in view.spheres.iter().enumerate() {
        let index = index as u32;
        // The selected sphere is the accent at full strength; the root is
        // distinguished so a rig has a readable origin; the rest are quiet.
        let color = if view.selected == Some(index) {
            palette::ACCENT
        } else if view.root == Some(index) {
            palette::dimmed(palette::ACCENT, 0.7)
        } else {
            palette::dimmed(palette::FOREGROUND, 0.55)
        };
        let centre = Vec3::from(*position);
        let hoop = radius * PROUD + MARGIN;
        for axis in 0..3 {
            ring(centre, hoop, axis, color);
        }
    }

    // A line down each link, so the tree's shape is visible where the spheres
    // are far apart.
    for (child, parent) in view.links {
        let (Some((a, _)), Some((b, _))) = (
            view.spheres.get(*child as usize),
            view.spheres.get(*parent as usize),
        ) else {
            continue;
        };
        let color = palette::dimmed(palette::ACCENT, 0.45);
        let base = vertices.len() as u32;
        for point in [Vec3::from(*a), Vec3::from(*b)] {
            vertices.push(Vertex {
                position: point.into(),
                normal: [0.0, 1.0, 0.0],
                color,
                mask: 0.0,
            });
        }
        indices.push(base);
        indices.push(base + 1);
    }

    (vertices, indices)
}

/// How much of the frame's height the gizmo occupies.
pub(super) const GIZMO_FRACTION: f32 = 0.18;
/// How many lines each half-axis of the navigation gizmo is drawn as.
pub(super) const GIZMO_BUNDLE: usize = 5;
/// How far the copies sit from the axis, in the gizmo's own units.
pub(super) const GIZMO_ROD: f32 = 0.018;

/// The three labelled axes, drawn as lines from the origin.
///
/// Each axis takes a distinct hue so the orientation is readable at a glance,
/// and the negative half is drawn dimmer so front and back are separable.
pub(super) fn gizmo_geometry() -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let axes = [
        (Vec3::X, [0.85f32, 0.22, 0.24]),
        (Vec3::Y, [0.36, 0.72, 0.32]),
        (Vec3::Z, [0.28, 0.48, 0.88]),
    ];

    // Each half-axis is a bundle of `GIZMO_BUNDLE` lines — the axis and four
    // copies stepped a little along the other two axes — so it reads as a rod
    // from every angle rather than as a hairline. A line is one pixel wide
    // whatever the device; the manipulator thickens itself the same way.
    let offsets = |direction: Vec3| -> [Vec3; GIZMO_BUNDLE] {
        let (across, other) = frame_about(direction);
        [
            Vec3::ZERO,
            across * GIZMO_ROD,
            -across * GIZMO_ROD,
            other * GIZMO_ROD,
            -other * GIZMO_ROD,
        ]
    };
    for (direction, color) in axes {
        for (end, shade) in [(direction, 1.0f32), (-direction, 0.25)] {
            let tint = [color[0] * shade, color[1] * shade, color[2] * shade];
            for offset in offsets(direction) {
                let base = vertices.len() as u32;
                vertices.push(Vertex {
                    position: offset.into(),
                    normal: [0.0, 1.0, 0.0],
                    color: tint,
                    mask: 0.0,
                });
                vertices.push(Vertex {
                    position: (end * 0.9 + offset).into(),
                    normal: [0.0, 1.0, 0.0],
                    color: tint,
                    mask: 0.0,
                });
                indices.extend_from_slice(&[base, base + 1]);
            }
        }
    }

    (vertices, indices)
}
