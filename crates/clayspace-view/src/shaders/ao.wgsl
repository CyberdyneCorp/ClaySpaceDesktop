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

struct Ao {
    projection: mat4x4<f32>,
    inverse_projection: mat4x4<f32>,
    /// Where the scene sits in the target, in pixels: origin then size.
    viewport: vec4<f32>,
    /// radius (view units), intensity, bias (view units), sample count.
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> ao: Ao;
@group(0) @binding(1) var depth_texture: texture_depth_multisampled_2d;

// One triangle rather than two: no seam along the diagonal, and three vertices
// instead of six. Positions come from the index, so it binds no buffer.
@vertex
fn fullscreen_vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}

/// The view-space position a pixel's depth stands for.
///
/// Sample 0 of the multisampled buffer, not a resolve: depth cannot be
/// resolved, and averaging depth across an edge would invent a surface halfway
/// between the two that met there.
fn view_position(coord: vec2<i32>) -> vec3<f32> {
    let depth = textureLoad(depth_texture, coord, 0);
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5) - ao.viewport.xy) / ao.viewport.zw;
    // Depth is already 0..1 here — glam's `perspective_rh` is the wgpu
    // convention rather than OpenGL's -1..1, so it goes into clip space as it
    // comes out of the buffer.
    let clip = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let view = ao.inverse_projection * clip;
    return view.xyz / view.w;
}

/// A per-pixel rotation for the sample pattern.
///
/// Without it every pixel samples the same directions and the result bands
/// along the kernel's own geometry, which reads as a pattern pressed into the
/// clay. With it the error becomes noise instead, which the composite pass
/// averages away.
fn rotation(coord: vec2<f32>) -> f32 {
    return fract(sin(dot(coord, vec2<f32>(12.9898, 78.233))) * 43758.5453) * 6.2831853;
}

@fragment
fn ao_fs(@builtin(position) position: vec4<f32>) -> @location(0) f32 {
    let coord = vec2<i32>(position.xy);
    let depth = textureLoad(depth_texture, coord, 0);
    // Nothing was drawn here. The surface is the only thing that writes depth
    // — the grid, the symmetry plane and the membrane all draw without it — so
    // this is exactly "not the sculpt", and the ground stays as it was.
    if depth >= 1.0 {
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

    // An orthonormal frame around the normal. The axis to cross against is
    // chosen away from the normal so the cross never degenerates.
    let helper = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(normal.y) > 0.9);
    let tangent = normalize(cross(helper, normal));
    let bitangent = cross(normal, tangent);

    let turn = rotation(position.xy);
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
        let at_coord = vec2<i32>(ao.viewport.xy + uv * ao.viewport.zw);
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

@group(0) @binding(0) var ao_texture: texture_2d<f32>;

// The occlusion, averaged and multiplied onto what has already been drawn.
//
// The blur is the other half of the rotated kernel: the rotation turns the
// pattern into noise, and a box over the neighbourhood turns the noise back
// into the value it was sampling. Four by four because that is the scale the
// rotation varies over.
//
// Multiplied through the blend state rather than here — the pass reads no
// colour, so it can run over the resolved target without a copy of it.
@fragment
fn composite_fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(ao_texture));
    let coord = vec2<i32>(position.xy);
    var total = 0.0;
    for (var y = -2; y <= 1; y = y + 1) {
        for (var x = -2; x <= 1; x = x + 1) {
            // Clamped rather than wrapped: `textureLoad` answers zero outside
            // the texture, and zero is total occlusion, so an unclamped read
            // would draw a black border around the viewport.
            let at = clamp(coord + vec2<i32>(x, y), vec2<i32>(0), size - vec2<i32>(1));
            total = total + textureLoad(ao_texture, at, 0).r;
        }
    }
    return vec4<f32>(vec3<f32>(total / 16.0), 1.0);
}
