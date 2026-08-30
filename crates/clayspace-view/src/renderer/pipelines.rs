//! What a pipeline is, said once.
//!
//! Everything about a draw except which shader runs: how it is culled, whether
//! it writes depth and which way that depth runs, whether it blends, and
//! whether it is multisampled at all.
//!
//! It is a struct because it used to be a boolean. The helper took a `cull`
//! flag and spent it on back-face culling *and* on depth writing, so a surface
//! that wanted one silently got the other — and neither could be tested,
//! because there was nothing to ask.
//!
//! The depth convention lives here too, for the same reason: it is one
//! decision that eight pipelines, a clear value, a wireframe bias and three
//! occlusion passes all have to agree with, and before it had a name they
//! agreed by coincidence.

use crate::gpu::{Framebuffer, Gpu};

use super::Vertex;

/// A pipeline for a pass with no geometry and no depth.
///
/// Single-sampled whatever the scene is: both of these run over the *resolved*
/// target, after the scene has been resolved into it.
pub(super) fn make_fullscreen_pipeline(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    fs: &str,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("fullscreen_vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fs),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        })
}

/// Which way depth runs, stated once.
///
/// Reversed: [`Camera::projection`] puts the *near* plane at 1 and the far
/// plane at 0, so nearer is greater and the buffer clears to zero. Floating
/// point crowds its precision near zero, and a conventional mapping spends
/// that on the far plane where nothing needs it — reversing the range puts the
/// precision where a sculptor is working, and makes it very nearly uniform
/// across the whole range besides.
///
/// Named rather than written at each pipeline because the convention is one
/// decision that eight pipelines, a clear value, a depth bias and three
/// occlusion passes all have to agree with. They agreed by coincidence before
/// this constant existed.
pub(super) const DEPTH_COMPARE: wgpu::CompareFunction = wgpu::CompareFunction::GreaterEqual;

/// What the depth buffer is cleared to: the far plane under [`DEPTH_COMPARE`].
pub(super) const DEPTH_CLEAR: f32 = 0.0;

/// The depth value nothing was drawn at, as the occlusion passes read it.
///
/// The same number as the clear, named separately because the two are
/// different claims: one is what the pass writes before drawing, the other is
/// what a later pass may conclude from finding it.
pub(super) const DEPTH_BACKGROUND: f32 = DEPTH_CLEAR;

pub(super) const NO_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: 0,
    slope_scale: 0.0,
    clamp: 0.0,
};

/// The polyframe's bias, toward the camera under [`DEPTH_COMPARE`].
///
/// A wireframe shares its vertices with the triangles it outlines, so without
/// one every line lands on exactly the same depth as the surface and the two
/// flicker against each other pixel by pixel. The slope term is what keeps a
/// steeply-angled triangle's edge from sinking into it.
///
/// Positive, because depth is reversed: toward the camera is *up* the range
/// now, and the bias that used to be negative would now push the lines behind
/// the surface they outline — which is the same flicker with an extra step.
pub(super) const WIRE_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: 2,
    slope_scale: 1.0,
    clamp: 0.0,
};

/// Everything about a pipeline except which shader it runs.
///
/// One struct rather than the pair of booleans it replaces, because the pair
/// lied: the old helper took a `cull` flag and spent it on back-face culling
/// *and* on depth writing, so a surface that wanted one silently got the
/// other. They are unrelated decisions — a ghost surface is culled and writes
/// no depth, a scaffold line is unculled and writes none — and each has to be
/// said out loud before reversed-Z and the transparent helpers start depending
/// on it.
#[derive(Debug, Clone, Copy)]
pub(super) struct PipelineState {
    pub(super) topology: wgpu::PrimitiveTopology,
    pub(super) cull_mode: Option<wgpu::Face>,
    /// Whether this pipeline runs in a pass that has a depth buffer at all.
    ///
    /// The scaffolding does not. It is drawn after the occlusion composite,
    /// into the resolved single-sampled target, precisely so that occlusion
    /// does not darken it — and a pass over that target has neither the scene's
    /// depth buffer, which is multisampled and cannot be attached to a
    /// single-sampled pipeline, nor any use for one, since the scaffolding
    /// compares `Always` regardless.
    pub(super) depth: bool,
    /// Whether it is multisampled, which is to say whether it draws into the
    /// scene's own target or into the resolved one.
    pub(super) multisampled: bool,
    /// Whether what this pipeline draws becomes the depth everything after it
    /// is tested against.
    pub(super) depth_write: bool,
    pub(super) depth_compare: wgpu::CompareFunction,
    pub(super) depth_bias: wgpu::DepthBiasState,
    /// `None` is opaque. The solid surface returns alpha 1 and so looked the
    /// same blended, but blending it told the driver the frame was one it
    /// could not reject fragments in, for nothing.
    pub(super) blend: Option<wgpu::BlendState>,
}

impl PipelineState {
    /// The solid sculpt: culled, writing depth, not blended.
    pub(super) fn opaque(topology: wgpu::PrimitiveTopology) -> Self {
        Self {
            topology,
            cull_mode: Some(wgpu::Face::Back),
            depth: true,
            multisampled: true,
            depth_write: true,
            depth_compare: DEPTH_COMPARE,
            depth_bias: NO_BIAS,
            blend: None,
        }
    }

    /// A helper drawn through: unculled, blended, and leaving the depth buffer
    /// alone so what is behind it still reads.
    pub(super) fn transparent(topology: wgpu::PrimitiveTopology) -> Self {
        Self {
            topology,
            cull_mode: None,
            depth: true,
            multisampled: true,
            depth_write: false,
            depth_compare: DEPTH_COMPARE,
            depth_bias: NO_BIAS,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        }
    }

    /// Scaffolding — the cage, the manipulator, an object's outline — drawn
    /// wherever it is, whatever stands in front of it.
    ///
    /// After the occlusion composite and into the resolved target, so it is
    /// drawn *on* the frame rather than *in* it: a manipulator standing over an
    /// occluded fold used to be darkened by that fold's occlusion, which made
    /// the handle a person is aiming at dimmer exactly where the form is
    /// deepest. It compares `Always` and writes no depth, so it has no use for
    /// the depth buffer it is now separated from.
    pub(super) fn scaffold(topology: wgpu::PrimitiveTopology) -> Self {
        Self {
            depth: false,
            multisampled: false,
            depth_compare: wgpu::CompareFunction::Always,
            ..Self::transparent(topology)
        }
    }

    /// Ink over the surface it outlines.
    pub(super) fn wire() -> Self {
        Self {
            depth_bias: WIRE_BIAS,
            ..Self::transparent(wgpu::PrimitiveTopology::LineList)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn make_pipeline(
    gpu: &Gpu,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    vs: &str,
    fs: &str,
    state: PipelineState,
) -> wgpu::RenderPipeline {
    // Read from the same place the framebuffer reads it, so the two cannot
    // disagree — a pipeline whose sample count differs from its attachment's
    // is a validation error at draw time rather than at creation.
    let samples = if state.multisampled {
        gpu.sample_count(format)
    } else {
        1
    };
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fs),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some(vs),
                buffers: &[Vertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fs),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: state.blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: state.topology,
                cull_mode: state.cull_mode,
                ..Default::default()
            },
            depth_stencil: state.depth.then(|| wgpu::DepthStencilState {
                format: Framebuffer::DEPTH_FORMAT,
                depth_write_enabled: state.depth_write,
                depth_compare: state.depth_compare,
                stencil: Default::default(),
                bias: state.depth_bias,
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            multiview: None,
            cache: None,
        })
}
