//! The design palette, in the space the GPU actually wants.
//!
//! The design states colours as hex, which is sRGB. Render targets here are
//! sRGB-encoded, so a shader writes *linear* values and the hardware encodes
//! them. Passing the hex value straight through renders it far too bright —
//! `#3A3E45` arrives looking like mid grey — which is how the first overlays
//! ended up competing with the silhouette they are supposed to sit behind.

/// `#23262B` — the viewport ground and the application's primary surface.
pub const GROUND: [f32; 3] = [0.016807, 0.019382, 0.024158];
/// `#2E3238` — one step up from the ground; the grid's minor lines.
pub const GRID_MINOR: [f32; 3] = [0.027321, 0.031896, 0.039546];
/// `#3A3E45` — the raised surface tone; the grid's axis lines.
pub const GRID_AXIS: [f32; 3] = [0.042311, 0.048172, 0.059511];
/// `#C9C4BD` — the primary foreground.
pub const FOREGROUND: [f32; 3] = [0.584078, 0.552011, 0.508881];
/// `#D9744A` — the sole accent. Reserved for the active brush and active tool
/// state, and used at reduced intensity where it must not dominate.
pub const ACCENT: [f32; 3] = [0.693872, 0.174647, 0.068478];

/// What each constant above was converted from, so the test can check them
/// rather than trusting that someone did the arithmetic correctly.
#[cfg(test)]
const SOURCES: [(&str, u32, [f32; 3]); 5] = [
    ("GROUND", 0x23262B, GROUND),
    ("GRID_MINOR", 0x2E3238, GRID_MINOR),
    ("GRID_AXIS", 0x3A3E45, GRID_AXIS),
    ("FOREGROUND", 0xC9C4BD, FOREGROUND),
    ("ACCENT", 0xD9744A, ACCENT),
];

/// The accent, dimmed.
///
/// The symmetry plane is tool state, so it earns the accent hue — but at full
/// intensity it reads as the brightest thing on screen, which is not what a
/// reference overlay should be.
pub const fn dimmed(color: [f32; 3], factor: f32) -> [f32; 3] {
    [color[0] * factor, color[1] * factor, color[2] * factor]
}

/// Converts one 8-bit sRGB channel to linear.
///
/// The palette constants are precomputed with this, and
/// `the_constants_match_their_hex` re-derives them here so a hand-edited
/// constant cannot drift from the hex it claims to be.
pub fn channel(value: u8) -> f32 {
    let c = value as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts an 8-bit sRGB triple to linear.
pub fn srgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [channel(r), channel(g), channel(b)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_constants_match_their_hex() {
        for (name, hex, constant) in SOURCES {
            let (r, g, b) = ((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
            let expected = srgb(r, g, b);
            for i in 0..3 {
                assert!(
                    (constant[i] - expected[i]).abs() < 1e-5,
                    "{name} channel {i} is {} but #{hex:06X} converts to {}",
                    constant[i],
                    expected[i]
                );
            }
        }
    }

    #[test]
    fn the_palette_is_dark_where_the_design_says_it_is() {
        for (name, color) in [
            ("ground", GROUND),
            ("grid minor", GRID_MINOR),
            ("grid axis", GRID_AXIS),
        ] {
            let max = color[0].max(color[1]).max(color[2]);
            assert!(max < 0.1, "{name} is too light in linear space: {max}");
        }
        assert!(
            FOREGROUND[0] > 0.5,
            "the foreground must be light enough to read against the ground"
        );
    }

    #[test]
    fn the_grid_sits_above_the_ground_but_below_the_foreground() {
        let luminance = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        assert!(
            luminance(GROUND) < luminance(GRID_MINOR),
            "the grid must be visible against the ground"
        );
        assert!(
            luminance(GRID_MINOR) < luminance(GRID_AXIS),
            "the axis lines must read as stronger than the minor ones"
        );
        assert!(
            luminance(GRID_AXIS) < luminance(FOREGROUND) * 0.2,
            "the grid must sit far below the foreground, not compete with it"
        );
    }
}
