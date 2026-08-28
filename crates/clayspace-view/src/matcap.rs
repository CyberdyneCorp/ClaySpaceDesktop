//! MatCap materials.
//!
//! A MatCap is a sphere image indexed by the view-space normal, so it carries
//! its whole lighting environment in one texture and needs no light rig. The
//! built-ins are generated rather than shipped as assets: the design calls for
//! neutral greys in the viewport, and generating them keeps the binary
//! self-contained and every visual test reproducible.

/// One of the built-in materials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatCap {
    /// The default: neutral grey clay, lit from the upper left.
    #[default]
    GreyClay,
    /// Darker, for reading silhouette over surface detail.
    DarkClay,
    /// A cooler grey, closer to plaster.
    Plaster,
    /// Warm terracotta, the one material with any hue.
    Terracotta,
    /// A polished skin that exaggerates curvature, for checking form.
    Polished,
}

impl MatCap {
    /// Every built-in, in the order the material swatches present them.
    pub const ALL: [MatCap; 5] = [
        Self::GreyClay,
        Self::DarkClay,
        Self::Plaster,
        Self::Terracotta,
        Self::Polished,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::GreyClay => "MatCap Cinza 01",
            Self::DarkClay => "MatCap Cinza 02",
            Self::Plaster => "Gesso",
            Self::Terracotta => "Terracota",
            Self::Polished => "Polido",
        }
    }

    /// Base colour, specular strength, and specular tightness.
    fn recipe(self) -> ([f32; 3], f32, f32) {
        match self {
            Self::GreyClay => ([0.62, 0.61, 0.59], 0.30, 24.0),
            Self::DarkClay => ([0.34, 0.34, 0.35], 0.26, 20.0),
            Self::Plaster => ([0.72, 0.72, 0.74], 0.12, 10.0),
            Self::Terracotta => ([0.72, 0.42, 0.28], 0.24, 18.0),
            Self::Polished => ([0.58, 0.58, 0.60], 0.75, 64.0),
        }
    }

    /// Renders the sphere image as RGBA8.
    ///
    /// The image is a lit hemisphere: each texel stands for the normal that
    /// maps to it, so shading it once here is what lets the fragment shader be
    /// a single texture fetch.
    pub fn generate(self, size: u32) -> Vec<u8> {
        self.image(size, false)
    }

    /// The same sphere with nothing around it: the swatch the interface
    /// shows for this material.
    ///
    /// Outside the sphere the texels are transparent rather than the rim
    /// colour, and the edge is feathered over a texel, so the ball sits on
    /// whatever panel it is drawn on instead of in a dark square.
    pub fn swatch(self, size: u32) -> Vec<u8> {
        self.image(size, true)
    }

    fn image(self, size: u32, cut_out: bool) -> Vec<u8> {
        let (base, specular_strength, specular_power) = self.recipe();
        // Upper left, which is where the design's material previews are lit
        // from and where a sculptor expects a key light.
        let light = normalize([-0.45, 0.65, 0.61]);
        let view = [0.0f32, 0.0, 1.0];

        let mut pixels = Vec::with_capacity((size * size * 4) as usize);
        for y in 0..size {
            for x in 0..size {
                // Texel centres, mapped to [-1, 1].
                let nx = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                let ny = 1.0 - (y as f32 + 0.5) / size as f32 * 2.0;
                let r2 = nx * nx + ny * ny;

                if r2 > 1.0 {
                    if cut_out {
                        pixels.extend_from_slice(&[0, 0, 0, 0]);
                        continue;
                    }
                    // Outside the sphere. These texels are only reached by a
                    // normal facing directly away, so they take the darkest
                    // rim value rather than a background colour that would
                    // show as a hard edge on a silhouette.
                    let edge = (base[0] * 0.18, base[1] * 0.18, base[2] * 0.18);
                    pixels.extend_from_slice(&[
                        to_srgb8(edge.0),
                        to_srgb8(edge.1),
                        to_srgb8(edge.2),
                        255,
                    ]);
                    continue;
                }
                // Feathered over about a texel at the silhouette, for the
                // swatch only; the material texture is never seen edge-on.
                let alpha = if cut_out {
                    (((1.0 - r2.sqrt()) * size as f32 * 0.5).clamp(0.0, 1.0) * 255.0) as u8
                } else {
                    255
                };

                let nz = (1.0 - r2).sqrt();
                let normal = [nx, ny, nz];

                let diffuse = dot(normal, light).max(0.0);
                // A little ambient so the terminator does not read as black.
                let ambient = 0.22;
                // Wrapped light, which is what makes clay look like clay
                // rather than like plastic.
                let wrap = (dot(normal, light) * 0.5 + 0.5).max(0.0) * 0.35;

                let half = normalize([light[0] + view[0], light[1] + view[1], light[2] + view[2]]);
                let specular = dot(normal, half).max(0.0).powf(specular_power) * specular_strength;

                // A rim term lifts the silhouette away from the background.
                let rim = (1.0 - dot(normal, view).max(0.0)).powf(3.0) * 0.18;

                let intensity = ambient + diffuse * 0.72 + wrap;
                let rgb = [
                    base[0] * intensity + specular + rim,
                    base[1] * intensity + specular + rim,
                    base[2] * intensity + specular + rim,
                ];
                pixels.extend_from_slice(&[
                    to_srgb8(rgb[0]),
                    to_srgb8(rgb[1]),
                    to_srgb8(rgb[2]),
                    alpha,
                ]);
            }
        }
        pixels
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt();
    if length <= f32::EPSILON {
        return [0.0, 0.0, 1.0];
    }
    [v[0] / length, v[1] / length, v[2] / length]
}

/// Linear to 8-bit sRGB. The render target is sRGB, so the texture must be too
/// or the midtones drift.
fn to_srgb8(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_generates_a_complete_image() {
        for matcap in MatCap::ALL {
            let size = 64;
            let pixels = matcap.generate(size);
            assert_eq!(
                pixels.len(),
                (size * size * 4) as usize,
                "{matcap:?} produced the wrong number of texels"
            );
            assert!(
                pixels.chunks_exact(4).all(|p| p[3] == 255),
                "{matcap:?} produced a transparent texel"
            );
        }
    }

    #[test]
    fn the_lit_side_is_brighter_than_the_shadowed_side() {
        let size = 64;
        let pixels = MatCap::GreyClay.generate(size);
        let luminance_at = |x: u32, y: u32| {
            let i = ((y * size + x) * 4) as usize;
            pixels[i] as u32 + pixels[i + 1] as u32 + pixels[i + 2] as u32
        };

        // The key light is upper left, so the upper-left quadrant of the
        // sphere must read brighter than the lower right.
        let lit = luminance_at(size / 4, size / 4);
        let shadowed = luminance_at(size * 3 / 4, size * 3 / 4);
        assert!(
            lit > shadowed,
            "the lit side ({lit}) is not brighter than the shadowed side ({shadowed})"
        );
    }

    #[test]
    fn materials_are_distinguishable_from_one_another() {
        let images: Vec<_> = MatCap::ALL.iter().map(|m| m.generate(32)).collect();
        for (i, a) in images.iter().enumerate() {
            for (j, b) in images.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    a,
                    b,
                    "{:?} and {:?} generate identical images",
                    MatCap::ALL[i],
                    MatCap::ALL[j]
                );
            }
        }
    }
}
