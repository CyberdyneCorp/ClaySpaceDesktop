// MatCap shading.
//
// This file contains display code only. There is deliberately no signed
// distance function, combine operator, blend profile or deformer here: the
// surface reaching this shader was meshed by the engine, and re-implementing
// any of the field math in WGSL is the drift this project is built to avoid.
// `no_field_math_in_shaders` asserts it.

struct Camera {
    view_projection: mat4x4<f32>,
    // The rotation part of the view matrix, used to take normals into view
    // space for the MatCap lookup.
    view_rotation: mat4x4<f32>,
};

struct Material {
    // rgb tint, a = 1 when the mesh carries vertex colours.
    tint: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<uniform> material: Material;
@group(0) @binding(2) var matcap_texture: texture_2d<f32>;
@group(0) @binding(3) var matcap_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    // How frozen this vertex is, 0 to 1.
    @location(3) mask: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) mask: f32,
};

// What a frozen region reads as, and how far the blend goes at full mask.
//
// A dark neutral rather than a hue: the mask says "this will not move", and a
// colour would read as paint on the clay. Short of 1 on purpose — the surface
// under the mask stays legible, so a sculptor can still see the form they are
// protecting, which is how both references draw it.
const MASK_COLOR: vec3<f32> = vec3<f32>(0.11, 0.12, 0.15);
const MASK_STRENGTH: f32 = 0.72;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    out.view_normal = (camera.view_rotation * vec4<f32>(input.normal, 0.0)).xyz;
    out.color = input.color;
    out.mask = input.mask;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // A MatCap is indexed by the view-space normal: the xy of the unit normal
    // maps onto the sphere image, which is why it needs no light position.
    let n = normalize(input.view_normal);
    let uv = vec2<f32>(n.x * 0.5 + 0.5, 0.5 - n.y * 0.5);
    let lit = textureSample(matcap_texture, matcap_sampler, uv).rgb;

    // Vertex colour modulates the material rather than replacing it, so a
    // palette-indexed voxel layer reads as coloured clay and not as flat paint.
    let modulation = mix(vec3<f32>(1.0), input.color, material.tint.a);
    let shaded = lit * material.tint.rgb * modulation;

    // The frozen region, drawn over the shading rather than in place of it.
    let frozen = clamp(input.mask, 0.0, 1.0) * MASK_STRENGTH;
    return vec4<f32>(mix(shaded, MASK_COLOR, frozen), 1.0);
}

// Overlays — the grid and the symmetry plane — are drawn flat, in their own
// colour, with no shading of any kind. They must never compete with the
// silhouette.
struct OverlayOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn overlay_vs(input: VertexInput) -> OverlayOutput {
    var out: OverlayOutput;
    out.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    // Opaque. The material's alpha channel carries the vertex-colour flag for
    // the surface pipeline and means nothing here; reading it made overlays
    // invisible whenever the mesh had no vertex colours.
    out.color = vec4<f32>(input.color, 1.0);
    return out;
}

@fragment
fn overlay_fs(input: OverlayOutput) -> @location(0) vec4<f32> {
    return input.color;
}

// The ZSphere membrane: the surface a rig would make, shown while it is being
// built. Translucent on purpose — you have to see the spheres and the links
// *through* it, or it is just the skin again with worse shading.
//
// Its own fragment entry rather than an alpha channel on the vertex, because
// `Vertex` carries three floats of colour and widening it would cost every
// mesh in the application a quarter more memory for one overlay's benefit.
@fragment
fn membrane_fs(input: OverlayOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color.rgb, 0.30);
}

// The same surface, drawn through. While a deformation cage is up the sculptor
// is aiming at control points, and half of them are behind the form — a solid
// surface hides exactly the handles that need reaching. Blender's X-ray and
// ZBrush's Ghost do the same thing for the same reason.
//
// The alpha is high enough that the form is still readable as a form: this is
// a surface seen through, not a surface turned off.
const GHOST_ALPHA: f32 = 0.42;

@fragment
fn fs_ghost(input: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(input.view_normal);
    let uv = vec2<f32>(n.x * 0.5 + 0.5, 0.5 - n.y * 0.5);
    let lit = textureSample(matcap_texture, matcap_sampler, uv).rgb;
    let modulation = mix(vec3<f32>(1.0), input.color, material.tint.a);
    let shaded = lit * material.tint.rgb * modulation;
    let frozen = clamp(input.mask, 0.0, 1.0) * MASK_STRENGTH;
    return vec4<f32>(mix(shaded, MASK_COLOR, frozen), GHOST_ALPHA);
}

// The polyframe: the mesh's own edges, drawn over it.
//
// Its own fragment entry rather than the overlay's, which takes the vertex
// colour — a mesh layer's vertices are near-white, so a wireframe drawn that
// way is white on white. This is a fixed dark line instead, translucent so a
// dense mesh reads as a tone rather than filling in solid.
//
// The colour is here rather than in the theme because it is not a surface the
// theme tokens describe: it is ink over whatever material is beneath, and it
// has to stay legible against every matcap rather than against one panel.
@fragment
fn wire_fs(input: OverlayOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.10, 0.11, 0.13, 0.55);
}


// A reference image, on the plane behind the sculpt.
//
// Its own vertex entry rather than the surface's, because it carries texture
// coordinates and nothing else does. The uv rides in the vertex `color`'s
// first two channels and the opacity in the third: a reference is the only
// thing drawn with this pipeline, so a whole extra attribute on every vertex
// of every mesh in the scene would be paid by the surface to serve a quad.
struct ReferenceOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
};

@vertex
fn reference_vs(input: VertexInput) -> ReferenceOutput {
    var out: ReferenceOutput;
    out.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    out.uv = input.color.xy;
    out.opacity = input.color.z;
    return out;
}

@fragment
fn reference_fs(input: ReferenceOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(matcap_texture, matcap_sampler, input.uv);
    // The file's own alpha times the sculptor's opacity, so a cut-out stays a
    // cut-out and a photograph fades evenly.
    return vec4<f32>(texel.rgb, texel.a * input.opacity);
}
