//! One shadow map, for the studio rig.
//!
//! Presentation only. MatCap does not cast — its lighting is welded to the
//! camera, so a shadow from it would swing round the form as the view moved,
//! which is worse than none. The studio rig's key light is fixed in the world,
//! which is what makes a shadow from it mean something.
//!
//! One map, one light, one orthographic fit. The review's own instruction is to
//! start with a single well-fitted directional map and measure before reaching
//! for cascades, and a form on a turntable is the case a single map is for: the
//! subject is bounded, the light does not move, and the depth range the map has
//! to cover is the diameter of what is being looked at.
//!
//! What it buys is self-shadowing. A key light on an unshadowed form lights the
//! inside of every fold as brightly as the flank beside it, which is the same
//! failure a MatCap has and the reason occlusion exists — except that occlusion
//! is a local term and cannot say that an arm is between the light and a chest.

use glam::{Mat4, Vec3};

use crate::gpu::Gpu;

/// The map's side, in texels.
///
/// Two thousand and forty-eight, which is the figure to start from. It is 16 MB
/// of `Depth32Float` and it is allocated only when the studio rig is first
/// asked for, so a session that never leaves MatCap never pays for it.
pub(super) const SHADOW_SIZE: u32 = 2048;

/// How much light a fully shadowed fragment keeps.
///
/// Not zero. A shadow that reaches black is a hole in the form: the ambient
/// term and the fill light are still reaching it, and a sculptor reading a
/// shape needs the shadowed side to stay legible. This is what the key light's
/// own contribution is multiplied down to.
pub(super) const SHADOW_DEPTH: f32 = 0.18;

/// What the studio shadow pass needs, and what the studio shader reads back.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct ShadowUniform {
    pub(super) light_view_projection: [[f32; 4]; 4],
    /// The normal offset in world units, the map's size in texels, whether the
    /// map holds anything, and how much light a shadowed fragment keeps.
    pub(super) params: [f32; 4],
}

/// The map, and the pipeline that fills it.
pub(super) struct ShadowMap {
    pub(super) depth: wgpu::TextureView,
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) uniform: wgpu::Buffer,
    /// What the studio pipelines bind: the light's matrix, the map, and the
    /// comparison sampler.
    pub(super) sampled: wgpu::BindGroup,
    /// What the pass that *fills* the map binds: the light's matrix, and
    /// deliberately not the map.
    ///
    /// Two groups over one buffer rather than one group used twice, because a
    /// texture cannot be written by a pass that also has it bound for reading
    /// — which is not a rule about tidiness but about what a pass may do to a
    /// resource at once, and wgpu refuses the whole command buffer over it.
    pub(super) casting: wgpu::BindGroup,
}

impl ShadowMap {
    /// The bind group layout, on its own.
    ///
    /// Built eagerly and separately from the map, because a pipeline's layout
    /// is part of the pipeline: the studio pipelines are created with the
    /// renderer and have to name this, while the sixteen megabytes of depth
    /// behind it should not be allocated by a session that never leaves
    /// MatCap.
    pub(super) fn sample_layout(gpu: &Gpu) -> wgpu::BindGroupLayout {
        gpu.device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("studio shadow"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // A comparison sampler, so the depth test happens as
                        // part of the fetch: the hardware compares four texels
                        // and returns how many passed, which is what makes a
                        // three-by-three filter nine fetches rather than
                        // thirty-six.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                ],
            })
    }

    /// The map itself, and the pass that fills it.
    ///
    /// `scene` is the layout the studio pipelines' group 0 uses. The pass here
    /// reads nothing from it — its vertex stage takes the light's matrix and
    /// the vertex buffer — but a pipeline's layout has to describe every group
    /// its bindings sit in, and the shadow bindings are group 1 because that is
    /// where the *shader* that samples them puts them.
    /// The layout the pass that fills the map binds.
    ///
    /// The light's matrix alone. Its entry point reads nothing else, and a
    /// layout that named the map would have the pass holding the texture it is
    /// writing.
    pub(super) fn cast_layout(gpu: &Gpu) -> wgpu::BindGroupLayout {
        gpu.device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("studio shadow cast"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            })
    }

    pub(super) fn new(
        gpu: &Gpu,
        shader: &wgpu::ShaderModule,
        scene: &wgpu::BindGroupLayout,
        sample_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let cast_layout = Self::cast_layout(gpu);
        let depth = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("studio shadow map"),
                size: wgpu::Extent3d {
                    width: SHADOW_SIZE,
                    height: SHADOW_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::gpu::Framebuffer::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("studio shadow"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("studio shadow"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            // Greater, because the map is rendered with the same reversed depth
            // range as everything else here: a fragment is lit when it is at
            // least as near the light as what the map recorded.
            compare: Some(wgpu::CompareFunction::GreaterEqual),
            ..Default::default()
        });

        let sampled = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("studio shadow"),
            layout: sample_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let casting = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("studio shadow cast"),
            layout: &cast_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        // Depth and nothing else: no fragment stage, no colour target, and so
        // no shading to compute for a pass whose whole output is a number per
        // texel. Culling the *front* faces rather than the back is the usual
        // trick — what the map records is then the far side of the form, which
        // is a long way behind the surface being tested and therefore does not
        // shadow it by accident.
        let pipeline =
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("studio shadow"),
                    layout: Some(&gpu.device.create_pipeline_layout(
                        &wgpu::PipelineLayoutDescriptor {
                            label: Some("studio shadow"),
                            bind_group_layouts: &[scene, &cast_layout],
                            push_constant_ranges: &[],
                        },
                    )),
                    vertex: wgpu::VertexState {
                        module: shader,
                        entry_point: Some("shadow_vs"),
                        buffers: &[super::Vertex::layout()],
                        compilation_options: Default::default(),
                    },
                    fragment: None,
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        cull_mode: Some(wgpu::Face::Front),
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: crate::gpu::Framebuffer::DEPTH_FORMAT,
                        depth_write_enabled: true,
                        depth_compare: super::DEPTH_COMPARE,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                    multisample: Default::default(),
                    multiview: None,
                    cache: None,
                });

        Self {
            depth,
            pipeline,
            uniform,
            sampled,
            casting,
        }
    }
}

/// An orthographic projection from the key light that just contains `bounds`.
///
/// Fitted to the subject rather than to a fixed volume, which is what "one
/// well-fitted map" means and what makes 2048 texels enough: the map's
/// resolution on the form is its side divided by the form's diameter, so a
/// fit that wastes half its area on empty space halves the shadow's sharpness.
///
/// Reversed, like every other projection here — near at one, far at zero — so
/// that the comparison in the shader reads the same way round as the depth test
/// that produced the map.
pub(super) fn light_projection(bounds: Option<(Vec3, Vec3)>, direction: Vec3) -> (Mat4, f32) {
    let (min, max) = bounds.unwrap_or((Vec3::splat(-1.0), Vec3::ONE));
    let centre = (min + max) * 0.5;
    // The bounding *sphere*, so the fit does not change as the light swings
    // round a box: a projection sized to the box's silhouette would breathe as
    // the form turned, and the shadow's sharpness would breathe with it.
    let radius = ((max - min).length() * 0.5).max(1e-4);

    let direction = direction.normalize_or_zero();
    let direction = if direction.length_squared() < 0.5 {
        Vec3::Y
    } else {
        direction
    };
    let eye = centre + direction * (radius * 2.0);
    // Any axis not parallel to the light will do; this picks the one furthest
    // from it so the cross never degenerates.
    let up = if direction.y.abs() > 0.9 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    #[allow(deprecated)]
    let view = Mat4::look_at_rh(eye, centre, up);
    #[allow(deprecated)]
    let projection = Mat4::orthographic_rh(
        -radius,
        radius,
        -radius,
        radius,
        // Swapped, which is what reverses the range.
        radius * 4.0,
        0.0,
    );

    // One texel of the map, in world units, which is the scale the normal
    // offset has to be measured in: below it the surface is thinner than the
    // map can see and shadows itself; far above it the contact shadow lifts off
    // the thing casting it. Two texels is the usual place to sit.
    let texel = radius * 2.0 / SHADOW_SIZE as f32;
    (projection * view, texel * 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fit contains the subject, from any direction the light comes from.
    #[test]
    fn the_light_sees_the_whole_form() {
        let bounds = (Vec3::new(-1.0, -0.4, -1.0), Vec3::new(1.0, 2.2, 1.0));
        for direction in [
            Vec3::new(-0.42, 0.78, 0.47),
            Vec3::Y,
            -Vec3::Y,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.3, -0.9, 0.2),
        ] {
            let (matrix, _) = light_projection(Some(bounds), direction);
            for corner in corners(bounds) {
                let clip = matrix * corner.extend(1.0);
                let ndc = clip.truncate() / clip.w;
                assert!(
                    ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 && (0.0..=1.0).contains(&ndc.z),
                    "{direction} put {corner} at {ndc}, outside the map"
                );
            }
        }
    }

    /// And the range is reversed, like every other projection here — a
    /// comparison sampler set to `GreaterEqual` over a map rendered the other
    /// way round shadows everything or nothing.
    #[test]
    fn the_light_projection_is_reversed() {
        let direction = Vec3::new(0.0, 1.0, 0.0);
        let bounds = (Vec3::splat(-1.0), Vec3::ONE);
        let (matrix, _) = light_projection(Some(bounds), direction);
        let depth_at = |point: Vec3| {
            let clip = matrix * point.extend(1.0);
            clip.z / clip.w
        };
        assert!(
            depth_at(Vec3::new(0.0, 1.0, 0.0)) > depth_at(Vec3::new(0.0, -1.0, 0.0)),
            "the top of the form is further from a light above it than the bottom"
        );
    }

    /// The fit follows the subject's size, which is what keeps the map's
    /// resolution on the form rather than on the space around it.
    #[test]
    fn the_offset_follows_the_form() {
        let (_, small) = light_projection(Some((Vec3::splat(-0.01), Vec3::splat(0.01))), Vec3::Y);
        let (_, large) = light_projection(Some((Vec3::splat(-100.0), Vec3::splat(100.0))), Vec3::Y);
        assert!(small > 0.0 && large > small * 1_000.0);
    }

    /// Nothing drawn is still a projection rather than a division by zero.
    #[test]
    fn an_empty_scene_still_fits() {
        let (matrix, offset) = light_projection(None, Vec3::ZERO);
        assert!(matrix.determinant().is_finite() && matrix.determinant() != 0.0);
        assert!(offset.is_finite() && offset > 0.0);
    }

    fn corners((min, max): (Vec3, Vec3)) -> Vec<Vec3> {
        let mut out = Vec::new();
        for x in [min.x, max.x] {
            for y in [min.y, max.y] {
                for z in [min.z, max.z] {
                    out.push(Vec3::new(x, y, z));
                }
            }
        }
        out
    }
}
