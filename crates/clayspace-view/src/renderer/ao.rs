//! What the occlusion passes are made of.
//!
//! The uniform they read, the bind groups they hold, the sample kernel they
//! walk, and the figures that decide how far a fold has to close before it
//! darkens. The passes themselves stay beside the frame they are part of —
//! what runs when, and in what order, is the renderer's business — and this is
//! everything that has an answer without a frame in front of it.
//!
//! Which is also what makes the kernel testable. Its directions used to be
//! computed in the shader, per sample per pixel, from the sample index alone;
//! here they can be asserted to be unit, hemispherical and evenly spread
//! without a device.

use glam::Vec3;

use crate::gpu::{Framebuffer, Gpu};

/// The occlusion passes' bind groups, and the framebuffer they read.
///
/// Held rather than built per frame. All three groups name texture views the
/// framebuffer owns, and a framebuffer is replaced only when the viewport is
/// resized — so rebuilding them every frame was three descriptor writes per
/// frame to say the same thing. [`Framebuffer::id`] is what makes the
/// staleness question answerable at all: wgpu gives a texture view no identity
/// to compare.
pub(super) struct AoResources {
    pub(super) framebuffer: u64,
    pub(super) reduce: wgpu::BindGroup,
    pub(super) ao: wgpu::BindGroup,
    pub(super) composite: wgpu::BindGroup,
    /// The post-process pass's view of the scene. `None` where the scene was
    /// drawn straight into the caller's target, which is where there is no
    /// post-process pass to run.
    pub(super) antialias: Option<wgpu::BindGroup>,
}

impl AoResources {
    pub(super) fn new(
        gpu: &Gpu,
        framebuffer: &Framebuffer,
        layouts: (
            &wgpu::BindGroupLayout,
            &wgpu::BindGroupLayout,
            &wgpu::BindGroupLayout,
        ),
        ao_buffer: &wgpu::Buffer,
        fxaa: (&wgpu::BindGroupLayout, &wgpu::Sampler),
    ) -> Self {
        let (reduce_layout, ao_layout, composite_layout) = layouts;
        let (fxaa_layout, fxaa_sampler) = fxaa;
        let uniform = wgpu::BindGroupEntry {
            binding: 0,
            resource: ao_buffer.as_entire_binding(),
        };
        let scene_depth = wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureView(framebuffer.depth_view()),
        };
        let reduced = wgpu::BindGroupEntry {
            binding: 2,
            resource: wgpu::BindingResource::TextureView(framebuffer.reduced_depth_view()),
        };
        let occlusion = wgpu::BindGroupEntry {
            binding: 3,
            resource: wgpu::BindingResource::TextureView(framebuffer.occlusion_view()),
        };
        Self {
            framebuffer: framebuffer.id(),
            reduce: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ao depth reduction"),
                layout: reduce_layout,
                entries: &[uniform.clone(), scene_depth.clone()],
            }),
            ao: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ao"),
                layout: ao_layout,
                entries: &[uniform.clone(), reduced.clone()],
            }),
            composite: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ao composite"),
                layout: composite_layout,
                entries: &[uniform, scene_depth, reduced, occlusion],
            }),
            antialias: framebuffer.antialias_view().map(|view| {
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("fxaa"),
                    layout: fxaa_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(fxaa_sampler),
                        },
                    ],
                })
            }),
        }
    }
}

/// What the occlusion pass needs to turn a depth buffer into a shadowing term.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct AoUniform {
    pub(super) projection: [[f32; 4]; 4],
    pub(super) inverse_projection: [[f32; 4]; 4],
    /// Where the scene sits in the target, in full-resolution pixels.
    pub(super) viewport: [f32; 4],
    /// The occlusion target's size, then its reciprocal. Distinct from the
    /// viewport because the kernel runs below display resolution.
    pub(super) ao_size: [f32; 4],
    /// radius, intensity, bias, sample count.
    pub(super) params: [f32; 4],
    /// Samples per scene pixel, display pixels per occlusion pixel, the
    /// upsample's depth sharpness, and the depth nothing was drawn at.
    pub(super) reduce: [f32; 4],
    /// Cavity strength, and the reach of its neighbourhood in view units.
    /// Zero strength is the term switched off, which costs the composite a
    /// branch and nothing else.
    pub(super) cavity: [f32; 4],
    /// The sample kernel, in the tangent frame. See [`ao_kernel`].
    pub(super) kernel: [[f32; 4]; AO_KERNEL],
}

/// How many samples the kernel holds room for.
///
/// The highest quality tier's count. Stated here and in `ao.wgsl`, which the
/// uniform-size test holds together.
pub(super) const AO_KERNEL: usize = 16;

/// The occlusion sample directions, in the tangent frame around the normal.
///
/// Computed on the host because none of it depends on the pixel. The loop used
/// to take a square root, a sine, a cosine and an interpolation *per sample per
/// pixel* to arrive at a direction that is a function of the sample index
/// alone; at half resolution and twelve samples that is a quarter of a million
/// transcendentals a frame at 1080p, computing the same sixteen numbers over
/// and over.
///
/// What is left in the shader is a rotation by the pixel's own turn, which is
/// two multiplies and an add against a precomputed pair.
///
/// The distribution is unchanged, and it is worth saying what it is. The
/// samples advance by the golden angle, so successive ones land as far from
/// each other as they can and a short loop still covers the hemisphere evenly.
/// The planar radius is a square root, so they spread over the disc's *area*
/// rather than crowding its centre. And the distance along the ray grows with
/// the index, so the near field is sampled as densely as the far one rather
/// than every sample sitting on the rim.
pub(super) fn ao_kernel(count: usize) -> [[f32; 4]; AO_KERNEL] {
    /// The golden angle, in radians.
    const GOLDEN: f32 = 2.399_963_2;
    /// The shortest a sample's reach gets, as a fraction of the radius.
    const NEAREST: f32 = 0.15;

    let count = count.clamp(1, AO_KERNEL);
    std::array::from_fn(|i| {
        // Entries past the count are never read — the shader loops to `count`
        // — and are left as the first one rather than as zero, which would be
        // a direction of no length if one ever were.
        let i = i.min(count - 1);
        let t = (i as f32 + 0.5) / count as f32;
        let planar = t.sqrt();
        let up = (1.0 - planar * planar).max(0.0).sqrt();
        let angle = i as f32 * GOLDEN;
        [
            angle.cos() * planar,
            angle.sin() * planar,
            up,
            NEAREST + (1.0 - NEAREST) * t * t,
        ]
    })
}

/// How far an occluder can be and still count, as a fraction of the radius of
/// what is being drawn.
///
/// A world-space reach rather than a screen-space one, so a fold darkens by
/// how deep it is rather than by how much of the window it happens to cover —
/// and a *fraction* rather than an absolute figure, so the same form at any
/// scale shades the same way. It was 0.08 view units, tuned against the
/// reference form whose starting sphere has radius 1; expressed against that
/// form's own radius the number is unchanged and every other model's is
/// finally right. An imported mesh a hundredth of that size got no visible
/// occlusion at all, and one a hundred times it got total occlusion, neither
/// of which is a property of the shape.
pub(super) const AO_RADIUS_FRACTION: f32 = 0.08;
/// How much of the surface's own colour full occlusion takes away.
pub(super) const AO_INTENSITY: f32 = 0.85;
/// The depth difference below which an occluder is the surface itself, as a
/// fraction of the radius.
///
/// Without it a flat surface occludes itself everywhere, from the difference
/// between a sample's own depth and the depth of the pixel it projects to. A
/// fraction for the reason the radius is one: the bias that stops a surface
/// self-occluding at one scale lets it self-occlude at another. 0.05 of the
/// radius is the 0.004 the reference form was tuned to.
pub(super) const AO_BIAS_FRACTION: f32 = 0.05;
/// How sharply the upsample rejects a neighbour whose depth differs, per
/// occlusion radius of difference.
///
/// The number that decides whether occlusion crosses a silhouette. Too low and
/// the average runs over the edge, which is the halo the box blur produced;
/// too high and the term degenerates to a nearest-neighbour lookup, which
/// brings the kernel's noise back at display resolution.
pub(super) const AO_DEPTH_SHARPNESS: f32 = 4.0;

/// The radius of what is being drawn, for the occlusion figures above.
///
/// Half the longest side of the box the geometry occupies: for the reference
/// form, whose starting sphere spans −1 to 1, exactly 1 — which is what makes
/// the fractions above the numbers the pass was already tuned to.
///
/// One is the fallback rather than zero. A frame with nothing in it has no
/// scale to speak of, and a radius of zero would divide the depth sharpness by
/// nothing.
pub(super) fn form_radius(bounds: Option<(Vec3, Vec3)>) -> f32 {
    let Some((min, max)) = bounds else {
        return 1.0;
    };
    let extent = (max - min).max_element() * 0.5;
    if extent.is_finite() && extent > 1e-6 {
        extent
    } else {
        1.0
    }
}
