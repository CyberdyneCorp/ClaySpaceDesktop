//! Putting pictures on the device.
//!
//! Two of them, and they want opposite things from a mip chain. A MatCap is
//! generated from a recipe, so every level can be *rendered* at its own size
//! and no filtering is involved at all. A reference image is somebody's
//! photograph, so its levels have to be filtered — and filtered in linear
//! colour, which is the half of it that is usually got wrong.

use wgpu::util::DeviceExt;

use crate::gpu::Gpu;
use crate::matcap::MatCap;

pub(super) fn upload_matcap(gpu: &Gpu, matcap: MatCap) -> wgpu::TextureView {
    const SIZE: u32 = 256;
    // Every level rendered from the material's own recipe at that level's
    // size, rather than the coarser levels being filtered down from the finest.
    //
    // Downsampling would be wrong twice over. The image is stored sRGB-encoded,
    // so averaging its bytes averages in the wrong space and darkens every
    // level; and a MatCap is a *function of the normal* sampled on a grid, so
    // the honest coarse version is that function sampled coarsely — which the
    // recipe can produce exactly. It costs a few hundred microseconds once per
    // material change, which is a click.
    //
    // Why they are needed at all: a subtool far enough away that its normals
    // vary by more than a texel between neighbouring pixels samples the
    // material at random, and the shading sparkles as the camera moves. That
    // is the case mipmaps exist for, and the texture had none.
    let levels = SIZE.ilog2() + 1;
    let mut pixels = Vec::new();
    for level in 0..levels {
        pixels.extend(matcap.generate((SIZE >> level).max(1)));
    }
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("matcap"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::MipMajor,
        &pixels,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// One reference image, as the viewport is given it.
#[derive(Debug, Clone, Copy)]
pub struct Reference<'a> {
    /// RGBA, `width * height * 4` bytes.
    pub pixels: &'a [u8],
    pub width: u32,
    pub height: u32,
    /// Where the quad sits, bottom-left first and anticlockwise.
    pub corners: [[f32; 3]; 4],
    pub opacity: f32,
}

/// Puts a reference image on a texture, with a mip chain.
///
/// Unlike a MatCap there is no recipe to re-render a coarse level from: a
/// reference is somebody's photograph. So the levels are filtered here, in
/// *linear* colour — decoded, averaged, re-encoded. Averaging the sRGB bytes
/// directly is the usual mistake and it darkens every level, which on a
/// reference reads as the opacity dial being wrong at a distance.
pub(super) fn upload_reference(
    gpu: &Gpu,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> wgpu::TextureView {
    let chain = mip_chain(pixels, width, height);
    let levels = chain.len() as u32;
    let data: Vec<u8> = chain.concat();
    let texture = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("reference"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // sRGB, like the matcap beside it: a photograph stored as sRGB and
            // sampled as linear comes out washed out, which on a reference
            // reads as the opacity being wrong.
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::MipMajor,
        &data,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// An RGBA8 sRGB image and every mip level below it, each halved and rounded
/// up, down to one texel.
///
/// Alpha is averaged directly and colour is averaged premultiplied by it, so a
/// cut-out reference does not bleed the colour of its transparent texels into
/// its edge as the levels get coarser.
pub(super) fn mip_chain(pixels: &[u8], width: u32, height: u32) -> Vec<Vec<u8>> {
    let mut levels = vec![pixels.to_vec()];
    let (mut w, mut h) = (width, height);
    while w > 1 || h > 1 {
        let source = levels.last().expect("the chain starts with level zero");
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = Vec::with_capacity((nw * nh * 4) as usize);
        for y in 0..nh {
            for x in 0..nw {
                let mut colour = [0.0f32; 3];
                let mut alpha = 0.0f32;
                let mut taken = 0.0f32;
                for dy in 0..2 {
                    for dx in 0..2 {
                        let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                        let at = ((sy * w + sx) * 4) as usize;
                        let a = source[at + 3] as f32 / 255.0;
                        for c in 0..3 {
                            colour[c] += from_srgb8(source[at + c]) * a;
                        }
                        alpha += a;
                        taken += 1.0;
                    }
                }
                // Back out of the premultiply. Where the whole block was
                // transparent there is no colour to recover and none to show.
                let weight = if alpha > 0.0 { 1.0 / alpha } else { 0.0 };
                next.extend_from_slice(&[
                    to_srgb8(colour[0] * weight),
                    to_srgb8(colour[1] * weight),
                    to_srgb8(colour[2] * weight),
                    (alpha / taken * 255.0 + 0.5) as u8,
                ]);
            }
        }
        levels.push(next);
        (w, h) = (nw, nh);
    }
    levels
}

/// 8-bit sRGB to linear, for filtering that has to happen in linear colour.
pub(super) fn from_srgb8(value: u8) -> f32 {
    let c = value as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear back to 8-bit sRGB.
pub(super) fn to_srgb8(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0 + 0.5) as u8
}
