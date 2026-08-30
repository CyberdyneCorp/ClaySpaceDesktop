// Screen-space ambient occlusion, over the scene's own depth buffer.
//
// Display code only, like `matcap.wgsl`: there is no signed distance function,
// combine operator or blend profile here. This reads the depth the mesh wrote
// and nothing about the field that produced the mesh.
//
// Why it exists: a MatCap is indexed by the view-space normal alone, so two
// points sharing a normal shade identically whether one sits on an open flank
// or at the bottom of a fold. Form without detail — which is what makes an
// unlit sculpt read as a blob.
//
// Why it reads DEPTH rather than the normal it could have taken from the
// vertex: the reference form is about seven triangles per covered pixel, so
// the interpolated normal field is piecewise-linear below the scale a screen
// derivative measures, and `dpdx` of it reports where the triangle edges are.
// Depth does not have that problem — positions are shared across an edge, so
// the depth buffer is a continuous function of screen position however finely
// the surface is tessellated. The normal used below is *derived* from it for
// the same reason.
//
// THREE PASSES, and why it is not one.
//
//   scene depth (multisampled, full resolution)
//        │  reduce_fs — closest covered sample of each block
//        ▼
//   reduced depth (single sample, half resolution)
//        │  ao_fs — the hemisphere kernel, at a quarter of the pixels
//        ▼
//   occlusion (R8, half resolution)
//        │  composite_fs — depth-aware upsample, multiplied on
//        ▼
//   the resolved frame
//
// The kernel is the expensive part and it does not need display resolution:
// occlusion is a low-frequency term, and running it at half resolution is a
// quarter of the samples. What it *does* need is an upsample that knows where
// the edges are. The pass this replaces averaged a 4×4 box of occlusion with
// no regard for depth, which blurs the shading of a near surface onto a far
// one across their shared silhouette — the halo that gives a screen-space
// effect away. Here every neighbour is weighted by how close its depth is to
// the pixel being shaded, so the average stops at the edge.
//
// The reduction takes the CLOSEST depth of the block rather than the average.
// An average of a foreground and a background that met at a silhouette
// describes a surface halfway between them, which is not there and occludes
// nothing.

struct Ao {
    projection: mat4x4<f32>,
    inverse_projection: mat4x4<f32>,
    /// Where the scene sits in the target, in full-resolution pixels: origin
    /// then size.
    viewport: vec4<f32>,
    /// The occlusion target's size in pixels, then its reciprocal.
    ao_size: vec4<f32>,
    /// radius (view units), intensity, bias (view units), sample count.
    params: vec4<f32>,
    /// x: samples per pixel of the scene depth buffer.
    /// y: full-resolution pixels one occlusion pixel spans.
    /// z: how sharply the upsample rejects a neighbour by depth, per view unit.
    /// w: the depth value nothing was drawn at — zero, the far plane, since
    ///    the viewport draws with a reversed depth range.
    reduce: vec4<f32>,
    /// x: cavity strength, zero when the term is off.
    /// y: how far the cavity term reaches, in view units.
    cavity: vec4<f32>,
};

@group(0) @binding(0) var<uniform> ao: Ao;

/// How many display pixels across one occlusion pixel covers.
///
/// A constant rather than `ao.reduce.y`, which carries the same number, so the
/// reduction's loop bounds are compile-time. That was done in the hope of an
/// unroll and it bought nothing measurable — 0.26 ms at 3840×2160 either way —
/// which is itself worth recording: this pass is bound by its multisampled
/// depth loads, sixteen of them per occlusion pixel, and not by the loop
/// around them. It stays because bounds a reader can see are better than
/// bounds carried in a uniform, not because it is faster.
///
/// The float in the uniform stays too: the coordinate arithmetic wants it as a
/// float, and converting a constant is free.
///
/// `the_shader_and_the_framebuffer_agree_on_the_occlusion_scale` holds this to
/// [`Framebuffer::AO_SCALE`], which is what decides the target's size.
const AO_SPAN: i32 = 2;

// The scene's own depth, as the pass that wrote it left it. Bound by the
// reduction and by the composite, which needs a full-resolution depth to weigh
// its neighbours against.
//
// Declared multisampled here and rewritten to `texture_depth_2d` when the
// device draws the scene with one sample. The two differ in this line and
// nowhere else: `textureLoad` takes the same arguments either way, and the
// sample index a single-sampled load reads as a mip level is only ever zero.
@group(0) @binding(1) var scene_depth: texture_depth_multisampled_2d;

/// The reduction's output, and the occlusion pass's input.
@group(0) @binding(2) var reduced_depth: texture_2d<f32>;

/// The occlusion pass's output, and the composite's input.
@group(0) @binding(3) var occlusion_texture: texture_2d<f32>;

// One triangle rather than two: no seam along the diagonal, and three vertices
// instead of six. Positions come from the index, so it binds no buffer.
@vertex
fn fullscreen_vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}

/// Which of two depths is nearer the camera.
///
/// The one place the depth convention is written down for this file. The
/// viewport draws reversed — the near plane is 1 and the far plane 0 — so
/// nearer is *greater*, and the reduction keeps the maximum. Under a
/// conventional range this and the background test below would be the only two
/// lines in the pass to change.
fn closer(a: f32, b: f32) -> f32 {
    return max(a, b);
}

/// Whether nothing was drawn at this depth.
///
/// The surface is the only thing that writes depth — the grid, the symmetry
/// plane and the membrane all draw without it — so this is exactly "not the
/// sculpt", and the ground stays as it was.
fn is_background(depth: f32) -> bool {
    return depth <= ao.reduce.w;
}

/// The view-space distance a raw depth stands for.
///
/// Only the depth row of the inverse projection matters, so this is the
/// reconstruction of a point on the view axis rather than of the pixel's own
/// position — which is what a depth *comparison* wants. Linear, so a
/// difference in it means the same thing near the camera and far from it,
/// which raw depth emphatically does not.
fn view_distance(depth: f32) -> f32 {
    let view = ao.inverse_projection * vec4<f32>(0.0, 0.0, depth, 1.0);
    return view.z / view.w;
}

// ---------------------------------------------------------------------------
// Pass 1 — reduction
// ---------------------------------------------------------------------------

/// Full-resolution multisampled depth in, single-sampled half-resolution out.
///
/// This is also what frees occlusion from multisampling. The kernel used to
/// bind the scene's depth buffer directly, which meant it could only be bound
/// as `texture_depth_multisampled_2d` and only ran where the device would
/// multisample — a device that would not got no occlusion at all, for a reason
/// that was about a binding rather than about rendering.
@fragment
fn reduce_fs(@builtin(position) position: vec4<f32>) -> @location(0) f32 {
    let samples = i32(ao.reduce.x);
    let limit = vec2<i32>(textureDimensions(scene_depth)) - vec2<i32>(1);
    let base = vec2<i32>(position.xy) * AO_SPAN;

    var depth = ao.reduce.w;
    for (var y = 0; y < AO_SPAN; y = y + 1) {
        for (var x = 0; x < AO_SPAN; x = x + 1) {
            let at = clamp(base + vec2<i32>(x, y), vec2<i32>(0), limit);
            for (var s = 0; s < samples; s = s + 1) {
                depth = closer(depth, textureLoad(scene_depth, at, s));
            }
        }
    }
    return depth;
}

// ---------------------------------------------------------------------------
// Pass 2 — the kernel, at the occlusion resolution
// ---------------------------------------------------------------------------

fn load_reduced(coord: vec2<i32>) -> f32 {
    let limit = vec2<i32>(textureDimensions(reduced_depth)) - vec2<i32>(1);
    return textureLoad(reduced_depth, clamp(coord, vec2<i32>(0), limit), 0).r;
}

/// The view-space position an occlusion pixel stands for.
///
/// The pixel covers a block of the full-resolution frame, so it is
/// reconstructed at that block's centre — reconstructing at its corner would
/// shift the whole occlusion field by half a block, which reads as the shading
/// sliding off the form.
fn view_position(coord: vec2<i32>) -> vec3<f32> {
    let span = ao.reduce.y;
    let full = (vec2<f32>(coord) + vec2<f32>(0.5)) * span;
    let uv = (full - ao.viewport.xy) / ao.viewport.zw;
    // Depth is already 0..1 here — glam's `perspective_rh` is the wgpu
    // convention rather than OpenGL's -1..1, so it goes into clip space as it
    // comes out of the buffer.
    let clip = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, load_reduced(coord), 1.0);
    let view = ao.inverse_projection * clip;
    return view.xyz / view.w;
}

/// A per-pixel rotation for the sample pattern.
///
/// Without it every pixel samples the same directions and the result bands
/// along the kernel's own geometry, which reads as a pattern pressed into the
/// clay. With it the error becomes noise instead, which the composite pass
/// averages away.
///
/// An integer hash rather than the usual `fract(sin(dot(…)) * …)`: that one
/// costs a transcendental per pixel to produce a pattern with visible
/// structure on some drivers, and this costs a handful of integer operations
/// to produce a better one. The constants are Chris Wellons's `lowbias32`.
fn rotation(coord: vec2<i32>) -> f32 {
    var v = u32(coord.x) * 73856093u ^ u32(coord.y) * 19349663u;
    v = v ^ (v >> 16u);
    v = v * 0x7feb352du;
    v = v ^ (v >> 15u);
    v = v * 0x846ca68bu;
    v = v ^ (v >> 16u);
    // The top 24 bits, scaled into a turn. The low bits of a hash are the
    // least well distributed, and a rotation only needs a fraction.
    return f32(v >> 8u) * (6.2831853 / 16777216.0);
}

@fragment
fn ao_fs(@builtin(position) position: vec4<f32>) -> @location(0) f32 {
    let coord = vec2<i32>(position.xy);
    if is_background(load_reduced(coord)) {
        return 1.0;
    }

    let origin = view_position(coord);
    // The normal from the depth buffer's own slope. Screen y grows downward
    // and view y upward, so the cross product can come out facing away; the
    // camera looks down -z, and a visible surface faces +z.
    var normal = normalize(cross(dpdx(origin), dpdy(origin)));
    if normal.z < 0.0 {
        normal = -normal;
    }

    let radius = ao.params.x;
    let intensity = ao.params.y;
    let bias = ao.params.z;
    let count = i32(ao.params.w);
    let span = ao.reduce.y;

    // An orthonormal frame around the normal. The axis to cross against is
    // chosen away from the normal so the cross never degenerates.
    let helper = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(normal.y) > 0.9);
    let tangent = normalize(cross(helper, normal));
    let bitangent = cross(normal, tangent);

    let turn = rotation(coord);
    // The golden angle, so successive samples land as far from each other as
    // they can and a short loop still covers the hemisphere evenly.
    let golden = 2.39996323;

    var occlusion = 0.0;
    for (var i = 0; i < count; i = i + 1) {
        let t = (f32(i) + 0.5) / f32(count);
        let angle = f32(i) * golden + turn;
        // sqrt so the samples spread over the disc's area rather than
        // crowding the centre, and a shortened radius near the pole.
        let planar = sqrt(t);
        let up = sqrt(1.0 - planar * planar);
        let direction = tangent * (cos(angle) * planar)
            + bitangent * (sin(angle) * planar)
            + normal * up;
        // Sample lengths grow with the index, so the near field is sampled as
        // densely as the far one rather than every sample sitting on the rim.
        let at = origin + direction * (radius * mix(0.15, 1.0, t * t));

        let clip = ao.projection * vec4<f32>(at, 1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.xy / clip.w;
        let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
            continue;
        }
        // Back into occlusion pixels: the projection lands in the scene's
        // full-resolution rectangle, and this pass is a block of it wide.
        let at_coord = vec2<i32>((ao.viewport.xy + uv * ao.viewport.zw) / span);
        let surface = view_position(at_coord);

        // View space looks down -z, so a *greater* z is nearer the camera. The
        // sample is occluded when the surface in front of it is nearer than it
        // is by more than the bias.
        if surface.z > at.z + bias {
            // Only where the occluder is within reach. Without this a distant
            // silhouette darkens everything in front of it, which is the halo
            // that gives cheap occlusion away.
            let reach = clamp(radius / max(abs(origin.z - surface.z), 1e-5), 0.0, 1.0);
            occlusion = occlusion + smoothstep(0.0, 1.0, reach);
        }
    }

    return clamp(1.0 - (occlusion / f32(count)) * intensity, 0.0, 1.0);
}

// ---------------------------------------------------------------------------
// Pass 3 — depth-aware upsample, multiplied onto the frame
// ---------------------------------------------------------------------------

/// The nearest depth this full-resolution pixel was drawn at.
///
/// The closest of its samples rather than sample zero. At a silhouette, sample
/// zero may describe the background while most of the pixel's coverage is
/// foreground; taking the nearest keeps the pixel on the side of the edge the
/// eye sees it on, and stops the occlusion flickering along outlines as the
/// camera moves.
fn scene_depth_at(coord: vec2<i32>) -> f32 {
    let samples = i32(ao.reduce.x);
    let limit = vec2<i32>(textureDimensions(scene_depth)) - vec2<i32>(1);
    let at = clamp(coord, vec2<i32>(0), limit);
    var depth = ao.reduce.w;
    for (var s = 0; s < samples; s = s + 1) {
        depth = closer(depth, textureLoad(scene_depth, at, s));
    }
    return depth;
}

fn load_occlusion(coord: vec2<i32>) -> f32 {
    let limit = vec2<i32>(textureDimensions(occlusion_texture)) - vec2<i32>(1);
    return textureLoad(occlusion_texture, clamp(coord, vec2<i32>(0), limit), 0).r;
}

/// How much this pixel sits in a crease, from the shape of its neighbourhood.
///
/// Occlusion answers the same question at its own radius and says nothing
/// about anything finer, and a MatCap says nothing about the neighbourhood at
/// all — it is indexed by the local normal, so a point on an open flank and a
/// point at the bottom of a groove shade identically if their normals agree.
/// Most of the detail in a finished sculpt is finer than the occlusion radius,
/// and this is what makes it read.
///
/// The measure is the classic one: reconstruct the neighbours, take the plane
/// through the centre, and ask how far each neighbour rises *above* it. On a
/// flat surface they sit in it and the term is zero; in a groove they rise and
/// it is positive; on a ridge they fall away and `max` takes it to zero, which
/// is right — a ridge is not a cavity.
///
/// Neighbours further than the reach are dropped rather than counted. Without
/// that, every silhouette has a background pixel one step away, the background
/// is a long way behind, and the term paints a dark line round the form.
fn cavity_at(center: vec2<i32>, reach: f32) -> f32 {
    let here = view_position(center);
    var neighbours = array<vec3<f32>, 4>(
        view_position(center + vec2<i32>(1, 0)),
        view_position(center - vec2<i32>(1, 0)),
        view_position(center + vec2<i32>(0, 1)),
        view_position(center - vec2<i32>(0, 1)),
    );
    var normal = normalize(cross(neighbours[0] - neighbours[1], neighbours[2] - neighbours[3]));
    if normal.z < 0.0 {
        normal = -normal;
    }

    var cavity = 0.0;
    var taken = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        let delta = neighbours[i] - here;
        let distance = length(delta);
        if distance < 1e-7 || distance > reach {
            continue;
        }
        cavity = cavity + max(dot(normal, delta / distance), 0.0);
        taken = taken + 1.0;
    }
    return select(0.0, cavity / taken, taken > 0.0);
}

// The occlusion, brought back to display resolution and multiplied onto what
// has already been drawn.
//
// Multiplied through the blend state rather than here — the pass reads no
// colour, so it can run over the resolved target without a copy of it.
@fragment
fn composite_fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let coord = vec2<i32>(position.xy);
    let depth = scene_depth_at(coord);
    if is_background(depth) {
        return vec4<f32>(1.0);
    }
    let here = view_distance(depth);
    let sharpness = ao.reduce.z;

    // Where this pixel sits in the occlusion image, as a continuous
    // coordinate: the occlusion texel centred on full-resolution block
    // (i + 0.5) * span sits at i, so the half-texel shifts cancel to this.
    let span = ao.reduce.y;
    let at = (vec2<f32>(coord) + vec2<f32>(0.5)) / span - vec2<f32>(0.5);
    let base = vec2<i32>(floor(at));

    var total = 0.0;
    var weight = 0.0;
    // Three by three, which at half resolution covers the six-pixel
    // neighbourhood the per-pixel rotation varies over — the same job the box
    // average used to do, now with the depth term deciding which of the nine
    // are the same surface as this pixel.
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let neighbour = base + vec2<i32>(x, y);
            let offset = vec2<f32>(neighbour) - at;
            // Distance in occlusion pixels, falling off over about one of
            // them: near enough to a bilinear tap at the centre, and wide
            // enough to average the kernel's noise away.
            let spatial = exp(-0.5 * dot(offset, offset));
            let delta = abs(view_distance(load_reduced(neighbour)) - here);
            let similarity = exp(-delta * sharpness);
            let w = spatial * similarity;
            total = total + load_occlusion(neighbour) * w;
            weight = weight + w;
        }
    }

    // Where nothing nearby shares this pixel's depth — a lone foreground
    // sliver, or the rim of the scene's rectangle — the nearest occlusion
    // pixel is a better answer than an average of nine surfaces that are not
    // this one, and far better than the black an unweighted sum would give.
    var value = select(
        load_occlusion(vec2<i32>(round(at))),
        total / weight,
        weight > 1.0e-4,
    );

    // And the crease term over it, when the frame is worth it. Read at the
    // occlusion resolution — the positions are already reconstructed there,
    // and a curvature taken from the full-resolution depth would be reading
    // the mesh's own tessellation rather than its form.
    if ao.cavity.x > 0.0 {
        value = value * (1.0 - cavity_at(base, ao.cavity.y) * ao.cavity.x);
    }
    return vec4<f32>(vec3<f32>(clamp(value, 0.0, 1.0)), 1.0);
}
