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
