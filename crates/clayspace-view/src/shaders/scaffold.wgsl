// The scaffolding: the cage, the manipulator, an object's outline.
//
// Its own module rather than a fragment entry in `matcap.wgsl`, for the reason
// the studio's shadow has its own bind group. A bind group layout is part of a
// pipeline's layout, and this pass needs a depth texture that no other pipeline
// in the viewport samples. In the scene module the group it would take is
// already the shadow's, and two declarations cannot share a slot.
//
// `Camera` and `VertexInput` come from `common.wgsl`. Same uniform buffer, same
// vertex buffer, one definition.
//
// Display code only, like every shader here: no distance function, no combine
// operator, no deformer. `no_field_math_in_shaders` asserts it.

@group(0) @binding(0) var<uniform> camera: Camera;

/// What the scaffolding needs to know about the frame it is being drawn onto.
struct Xray {
    /// x: how many display pixels across one depth texel covers.
    /// y: the alpha a fragment keeps where the sculpt stands in front of it.
    /// z, w: unused, and here because a uniform's fields align to sixteen
    ///       bytes either way.
    params: vec4<f32>,
};

@group(1) @binding(0) var<uniform> xray: Xray;

/// The depth the sculpt wrote, at one sample per texel.
///
/// The frame's reduced depth — the buffer the occlusion kernel reads. Sampled
/// rather than tested: this pass binds no depth attachment and keeps its place
/// after the occlusion composite, where the scene's own depth buffer is
/// multisampled and this pipeline is not.
@group(1) @binding(1) var scene_depth: texture_2d<f32>;

struct ScaffoldOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn scaffold_vs(input: VertexInput) -> ScaffoldOutput {
    var out: ScaffoldOutput;
    out.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    // Opaque as it leaves the vertex stage. The material's alpha channel
    // carries the vertex-colour flag for the surface pipeline and means nothing
    // here, and reading it once made every overlay invisible on a mesh with no
    // vertex colours.
    out.color = vec4<f32>(input.color, 1.0);
    return out;
}

@fragment
fn scaffold_fs(input: ScaffoldOutput) -> @location(0) vec4<f32> {
    let span = max(xray.params.x, 1.0);
    let texel = vec2<i32>(input.clip_position.xy / span);
    let limit = vec2<i32>(textureDimensions(scene_depth)) - vec2<i32>(1);
    let sculpt = textureLoad(scene_depth, clamp(texel, vec2<i32>(0), limit), 0).r;

    // Reversed Z: a larger value is nearer, so the sculpt stands in front of
    // this fragment when its depth is the greater of the two.
    //
    // One comparison, and no separate test for "was anything drawn here". Under
    // this convention the cleared value is the *smallest* one, so an untouched
    // pixel can never be nearer than a fragment in front of the far plane, and
    // the two conditions are the same condition. A second test was written here
    // first and removed: putting the bug back left every test passing, which is
    // what a redundant guard looks like from the outside.
    //
    // It does mean this depends on the convention rather than on a value it
    // reads. `a_ghosted_surface_dims_nothing` is what holds the dependency, and
    // it was checked by inverting this comparison: a ghost writes no depth, so
    // reading the cleared buffer the other way round dims a ring that has
    // nothing in front of it, and that test fails.
    //
    // The surface is the only thing that writes depth — the grid, the symmetry
    // planes, the reference image and a ghosted surface all draw without it —
    // so scaffolding over any of those is drawn exactly as it always was.
    let behind = sculpt > input.clip_position.z;
    return vec4<f32>(input.color.rgb, input.color.a * select(1.0, xray.params.y, behind));
}
