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
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) view_normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    out.view_normal = (camera.view_rotation * vec4<f32>(input.normal, 0.0)).xyz;
    out.color = input.color;
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
    return vec4<f32>(lit * material.tint.rgb * modulation, 1.0);
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
