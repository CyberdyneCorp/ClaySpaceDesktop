// What more than one shader here needs, defined once.
//
// WGSL has no include, so this is prepended to the sources that use it when
// their modules are created. That is a small piece of machinery and it is
// worth being clear about why it exists rather than copying twelve lines: the
// copies were identical, and the failure mode of two identical copies is that
// one of them is edited.
//
// Keep this to things that are genuinely shared. A definition that only one
// shader uses belongs in that shader, next to the reasoning for it.

// A triangle covering the whole target, for a pass with no geometry.
//
// One triangle rather than two: no seam along the diagonal, and three vertices
// instead of six. Positions come from the vertex index, so it binds no buffer.
@vertex
fn fullscreen_vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}

// The camera, and one vertex of the geometry drawn with it.
//
// The structs are shared and the bindings are not: each module declares its
// own `@group(0) @binding(0)`, because the occlusion and anti-aliasing passes
// bind something else entirely at that slot. Shared because the *layout* is
// what two modules must agree about — a field added to one copy and not the
// other reads a uniform buffer wrongly and draws something almost right, which
// is the failure this file exists to prevent.

struct Camera {
    view_projection: mat4x4<f32>,
    // The rotation part of the view matrix, used to take normals into view
    // space for the MatCap lookup.
    view_rotation: mat4x4<f32>,
};

/// One vertex, as `Vertex::layout` describes it on the Rust side.
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    // How frozen this vertex is, 0 to 1.
    @location(3) mask: f32,
};
