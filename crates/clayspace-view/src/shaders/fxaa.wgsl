// Anti-aliasing for a device that will not multisample.
//
// Display code only, like the rest of the shaders here.
//
// This is a fallback and is written as one. Multisampling is better in every
// way that matters to a sculptor: it works on the geometry rather than on the
// picture of it, so it cannot mistake a fine sculpted crease for a stair-step
// and smooth it away. FXAA can, and does. It runs *only* where the device
// refuses to multisample the surface format, which is where the alternative is
// not four samples but a stair-stepped silhouette against a flat ground — the
// most visible thing that can be wrong with a frame.
//
// The two are never run together. Four samples and a blur over the top is
// paying twice to lose detail once.
//
// The algorithm is the well-known one: find the luminance gradient across a
// pixel's neighbours, take the direction along the edge, and sample a short
// way along it either side. What it costs is five texture samples to find the
// edge and four to resolve it, all of them filtered, over the resolved target.

@group(0) @binding(0) var scene_color: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

/// How far along an edge the resolve reaches, in pixels.
///
/// Eight is the usual figure. Further blurs; nearer stops catching the shallow
/// edges that are the worst of the stair-stepping.
const SPAN_MAX: f32 = 8.0;

/// How much of the local contrast an edge has to be before it is one.
///
/// Below this the neighbourhood is flat enough that whatever gradient was
/// measured is noise, and blurring along it would soften a surface for nothing.
const EDGE_THRESHOLD: f32 = 0.125;

/// The floor under that, so a very dark neighbourhood is not treated as all
/// edge.
const EDGE_THRESHOLD_MIN: f32 = 0.0312;

/// Keeps the direction from running away where the gradient is tiny.
const REDUCE_MUL: f32 = 0.125;
const REDUCE_MIN: f32 = 1.0 / 128.0;

// One triangle rather than two: no seam along the diagonal, and three vertices
// instead of six. Positions come from the index, so it binds no buffer.
@vertex
fn fullscreen_vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}

/// Perceived brightness. The green weight dominates because the eye does.
fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.299, 0.587, 0.114));
}

fn sample_at(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(scene_color, scene_sampler, uv, 0.0).rgb;
}

@fragment
fn fxaa_fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(scene_color));
    let texel = vec2<f32>(1.0) / size;
    let uv = position.xy * texel;

    let middle = sample_at(uv);
    let north_west = luminance(sample_at(uv + vec2<f32>(-1.0, -1.0) * texel));
    let north_east = luminance(sample_at(uv + vec2<f32>(1.0, -1.0) * texel));
    let south_west = luminance(sample_at(uv + vec2<f32>(-1.0, 1.0) * texel));
    let south_east = luminance(sample_at(uv + vec2<f32>(1.0, 1.0) * texel));
    let here = luminance(middle);

    let lowest = min(here, min(min(north_west, north_east), min(south_west, south_east)));
    let highest = max(here, max(max(north_west, north_east), max(south_west, south_east)));
    let contrast = highest - lowest;

    // Flat enough that any gradient measured here is noise.
    if contrast < max(EDGE_THRESHOLD_MIN, highest * EDGE_THRESHOLD) {
        return vec4<f32>(middle, 1.0);
    }

    // The direction *along* the edge, which is perpendicular to the gradient.
    var direction = vec2<f32>(
        -((north_west + north_east) - (south_west + south_east)),
        (north_west + south_west) - (north_east + south_east),
    );
    let reduce = max(
        (north_west + north_east + south_west + south_east) * 0.25 * REDUCE_MUL,
        REDUCE_MIN,
    );
    let scale = 1.0 / (min(abs(direction.x), abs(direction.y)) + reduce);
    direction = clamp(
        direction * scale,
        vec2<f32>(-SPAN_MAX),
        vec2<f32>(SPAN_MAX),
    ) * texel;

    // Two samples close in, and two further out. The near pair is the answer
    // where the edge is shallow; the far pair carries a steep one, and is
    // discarded where it has wandered off the edge entirely.
    let near = 0.5 * (sample_at(uv + direction * (1.0 / 3.0 - 0.5))
        + sample_at(uv + direction * (2.0 / 3.0 - 0.5)));
    let far = near * 0.5
        + 0.25 * (sample_at(uv - direction * 0.5) + sample_at(uv + direction * 0.5));

    let far_luminance = luminance(far);
    return vec4<f32>(select(near, far, far_luminance >= lowest && far_luminance <= highest), 1.0);
}
