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
    // x is how opaque the surface is drawn, y how far the silhouette is
    // darkened; the rest is padding, since a uniform's fields are aligned to
    // sixteen bytes either way.
    ghost: vec4<f32>,
    // Studio mode only: roughness, metallic, exposure.
    studio: vec4<f32>,
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

/// How far into the surface the contour darkening reaches.
///
/// A quarter of the way from edge-on to facing. Wider and the whole flank
/// dims, which is a tint rather than a contour; narrower and it becomes a line
/// drawn round the form, which is a cartoon outline and not what a sculptor
/// reads a silhouette from.
const CONTOUR_REACH: f32 = 0.35;

/// The material's own shading of a view-space normal.
///
/// The MatCap lookup, plus an optional darkening toward the silhouette. The
/// texture already fills the texels outside its sphere with a dark rim value,
/// which is a *fixed* contour baked into the material; this is the adjustable
/// one, and it is off unless the host asks for it — `material.ghost.y` is zero
/// by default, and at zero this multiplies by one.
///
/// Darkening rather than the brightening a fresnel term usually gives. On clay
/// the useful thing is for the contour to *read*, and a bright rim reads as
/// wet plastic; ZBrush's own materials do the same.
fn material_shading(n: vec3<f32>, color: vec3<f32>) -> vec3<f32> {
    let uv = vec2<f32>(n.x * 0.5 + 0.5, 0.5 - n.y * 0.5);
    let lit = textureSample(matcap_texture, matcap_sampler, uv).rgb;

    // Vertex colour modulates the material rather than replacing it, so a
    // palette-indexed voxel layer reads as coloured clay and not as flat paint.
    let modulation = mix(vec3<f32>(1.0), color, material.tint.a);

    let facing = clamp(n.z, 0.0, 1.0);
    let contour = mix(
        1.0 - material.ghost.y,
        1.0,
        smoothstep(0.0, CONTOUR_REACH, facing),
    );
    return lit * material.tint.rgb * modulation * contour;
}

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
    let shaded = material_shading(normalize(input.view_normal), input.color);

    // The frozen region, drawn over the shading rather than in place of it.
    let frozen = clamp(input.mask, 0.0, 1.0) * MASK_STRENGTH;
    return vec4<f32>(mix(shaded, MASK_COLOR, frozen), 1.0);
}

// ---------------------------------------------------------------------------
// Studio shading
// ---------------------------------------------------------------------------
//
// A second way to shade the same surface, offered *beside* MatCap and never in
// place of it. MatCap remains the default and remains the right default: it is
// one texture fetch, it is stable under a moving camera, and form reads from it
// better than from any light rig, which is why every sculpting application has
// one.
//
// What it cannot do is show a highlight *move*. A MatCap is indexed by the
// view-space normal, so its lighting is welded to the camera: orbit the form
// and the light orbits with it. That is exactly the property that makes it good
// for reading form and useless for judging how a surface will behave under a
// real light — which is a thing sculptors check before they call a piece
// finished.
//
// So the rig here is fixed in the *world*. The directions below are world-space
// constants taken into view space through the camera's own rotation, which is
// already in the uniform; orbiting therefore sweeps the key light across the
// form the way walking round a maquette does.
//
// Three lights and an ambient, which is a photographic studio and not a
// renderer: a key, a fill at a third of it from the other side, and a rim from
// behind to lift the silhouette. No shadow map, no environment probe, no
// clustered anything. If those are ever wanted they belong here, behind the
// same switch, and not in the sculpt path.

/// Where the key light stands, in world space.
const KEY_DIRECTION: vec3<f32> = vec3<f32>(-0.42, 0.78, 0.47);
const KEY_COLOR: vec3<f32> = vec3<f32>(1.0, 0.96, 0.90);
const KEY_INTENSITY: f32 = 3.1;

/// The fill, opposite and much weaker, so the shadow side reads without
/// flattening the form.
const FILL_DIRECTION: vec3<f32> = vec3<f32>(0.75, 0.12, 0.65);
const FILL_COLOR: vec3<f32> = vec3<f32>(0.82, 0.87, 1.0);
const FILL_INTENSITY: f32 = 0.85;

/// And the rim, from behind, which is what separates a dark form from a dark
/// ground.
const RIM_DIRECTION: vec3<f32> = vec3<f32>(0.12, 0.34, -0.93);
const RIM_COLOR: vec3<f32> = vec3<f32>(0.90, 0.93, 1.0);
const RIM_INTENSITY: f32 = 1.4;

/// A little sky, so the terminator does not read as black.
const STUDIO_AMBIENT: vec3<f32> = vec3<f32>(0.055, 0.06, 0.072);

/// Reflectance at normal incidence for a dielectric. Clay is not metal.
const DIELECTRIC_F0: f32 = 0.04;

/// A world-space direction, in view space.
///
/// The camera's rotation without its translation, which is what the uniform
/// already carries for the MatCap lookup. This is what fixes the rig in the
/// world rather than to the camera.
fn to_view(direction: vec3<f32>) -> vec3<f32> {
    return normalize((camera.view_rotation * vec4<f32>(normalize(direction), 0.0)).xyz);
}

/// GGX specular and Lambert diffuse for one light.
///
/// The standard microfacet terms, written out rather than pulled from a
/// library because there is no library here and because four lines of them is
/// less to keep working than a dependency.
fn studio_light(
    normal: vec3<f32>,
    view: vec3<f32>,
    direction: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    albedo: vec3<f32>,
    roughness: f32,
    metallic: f32,
) -> vec3<f32> {
    let light = to_view(direction);
    let incidence = max(dot(normal, light), 0.0);
    if incidence <= 0.0 {
        return vec3<f32>(0.0);
    }
    let half_vector = normalize(light + view);
    let alpha = max(roughness * roughness, 1.0e-3);
    let alpha2 = alpha * alpha;

    // Trowbridge–Reitz.
    let cos_half = max(dot(normal, half_vector), 0.0);
    let denominator = cos_half * cos_half * (alpha2 - 1.0) + 1.0;
    let distribution = alpha2 / (3.14159265 * denominator * denominator);

    // Smith's height-correlated visibility, Schlick-approximated.
    let k = alpha * 0.5;
    let view_term = max(dot(normal, view), 1.0e-4);
    let geometry = (incidence / (incidence * (1.0 - k) + k))
        * (view_term / (view_term * (1.0 - k) + k));

    // Fresnel. A metal takes its reflectance from its albedo; a dielectric
    // takes 4%, and clay is a dielectric.
    let f0 = mix(vec3<f32>(DIELECTRIC_F0), albedo, metallic);
    let fresnel = f0 + (vec3<f32>(1.0) - f0)
        * pow(1.0 - max(dot(half_vector, view), 0.0), 5.0);

    let specular = fresnel * (distribution * geometry / (4.0 * incidence * view_term));
    // Energy the specular lobe took is energy the diffuse one does not get,
    // and a metal has no diffuse at all.
    let diffuse = (vec3<f32>(1.0) - fresnel) * (1.0 - metallic) * albedo / 3.14159265;

    return (diffuse + specular) * color * (intensity * incidence);
}

/// The ACES filmic curve, Krzysztof Narkowicz's fit.
///
/// A curve rather than a clamp. Three lights over a specular lobe reach well
/// past one, and clipping them turns every highlight into a flat white patch
/// with a hard edge — which reads as a rendering fault rather than as a bright
/// surface. This rolls them off instead.
///
/// The target is sRGB-encoded by the hardware, so what comes out here is
/// linear and no gamma is applied.
fn tone_map(color: vec3<f32>) -> vec3<f32> {
    let x = max(color, vec3<f32>(0.0));
    let mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
    return clamp(mapped, vec3<f32>(0.0), vec3<f32>(1.0));
}

/// The whole rig, over one view-space normal.
fn studio_shading(n: vec3<f32>, color: vec3<f32>) -> vec3<f32> {
    // The camera looks down -z in view space, so the direction *to* the eye
    // from any visible point is +z.
    let view = vec3<f32>(0.0, 0.0, 1.0);
    let albedo = material.tint.rgb * mix(vec3<f32>(1.0), color, material.tint.a);
    let roughness = clamp(material.studio.x, 0.04, 1.0);
    let metallic = clamp(material.studio.y, 0.0, 1.0);

    var lit = STUDIO_AMBIENT * albedo;
    lit += studio_light(n, view, KEY_DIRECTION, KEY_COLOR, KEY_INTENSITY, albedo, roughness, metallic);
    lit += studio_light(n, view, FILL_DIRECTION, FILL_COLOR, FILL_INTENSITY, albedo, roughness, metallic);
    lit += studio_light(n, view, RIM_DIRECTION, RIM_COLOR, RIM_INTENSITY, albedo, roughness, metallic);

    return tone_map(lit * material.studio.z);
}

@fragment
fn fs_studio(input: VertexOutput) -> @location(0) vec4<f32> {
    let shaded = studio_shading(normalize(input.view_normal), input.color);
    let frozen = clamp(input.mask, 0.0, 1.0) * MASK_STRENGTH;
    return vec4<f32>(mix(shaded, MASK_COLOR, frozen), 1.0);
}

/// The same, drawn through, for when a cage is up in Studio mode.
@fragment
fn fs_studio_ghost(input: VertexOutput) -> @location(0) vec4<f32> {
    let shaded = studio_shading(normalize(input.view_normal), input.color);
    let frozen = clamp(input.mask, 0.0, 1.0) * MASK_STRENGTH;
    return vec4<f32>(mix(shaded, MASK_COLOR, frozen), material.ghost.x);
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
// The alpha comes from the uniform rather than a constant here, because the
// useful amount is the sculptor's to choose — a cage's control points and a
// photograph behind the silhouette want different numbers. What the host will
// not pass is zero: a surface faded to nothing is a surface turned off, and
// turning the layer off is what turning the layer off is for.
@fragment
fn fs_ghost(input: VertexOutput) -> @location(0) vec4<f32> {
    let shaded = material_shading(normalize(input.view_normal), input.color);
    let frozen = clamp(input.mask, 0.0, 1.0) * MASK_STRENGTH;
    return vec4<f32>(mix(shaded, MASK_COLOR, frozen), material.ghost.x);
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
