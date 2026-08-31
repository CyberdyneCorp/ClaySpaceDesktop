//! The design palette, in the space the GPU actually wants.
//!
//! The design states colours as hex, which is sRGB. Render targets here are
//! sRGB-encoded, so a shader writes *linear* values and the hardware encodes
//! them. Passing the hex value straight through renders it far too bright —
//! `#3A3E45` arrives looking like mid grey — which is how the first overlays
//! ended up competing with the silhouette they are supposed to sit behind.
//!
//! # The four surfaces
//!
//! ```text
//! VIEWPORT  #1B1E22   the sculpt's ground
//! GROUND    #23262B   the application shell
//! PANEL     #2E3238   a panel on the shell
//! RAISED    #3A3E45   a selected row, a pressed control
//! ```
//!
//! Ordered darkest to lightest, and the order is the whole point: the sculpt
//! sits at the bottom of the ladder so the chrome recedes from it. One tone
//! served all four until the viewport was given its own, and a capture of the
//! shell showed what that cost — chrome and sculpt in a single flat band of
//! grey, with nothing to tell the eye where to look.

/// `#1B1E22` — the sculpting viewport's ground, and the renderer's clear
/// colour.
///
/// Darker than the shell around it. The separation between the sculpt and the
/// chrome is carried by this step and by nothing else: the design draws no
/// outline around the viewport, so if the two tones were equal there would be
/// no edge at all.
pub const VIEWPORT: [f32; 3] = [0.010960, 0.012983, 0.015996];
/// `#23262B` — the application shell's ground.
pub const GROUND: [f32; 3] = [0.016807, 0.019382, 0.024158];
/// `#2E3238` — a panel sitting on the shell's ground.
pub const PANEL: [f32; 3] = [0.027321, 0.031896, 0.039546];
/// `#3A3E45` — a raised surface: a selected row, a pressed control.
pub const RAISED: [f32; 3] = [0.042311, 0.048172, 0.059511];
/// `#262A2F` — the grid's minor lines.
///
/// Defined against `VIEWPORT`, which is the ground it is drawn on, rather than
/// against the shell's. It held `PANEL`'s value while the viewport and the
/// shell shared one tone; keeping it there when the viewport dropped would
/// have made the grid more prominent as an accident of a change that was about
/// the chrome.
pub const GRID_MINOR: [f32; 3] = [0.019382, 0.023153, 0.028426];
/// `#32363C` — the grid's axis lines. Against `VIEWPORT`, as the minor lines
/// are, and for the same reason.
pub const GRID_AXIS: [f32; 3] = [0.031896, 0.036889, 0.045186];
/// `#C9C4BD` — the primary foreground.
pub const FOREGROUND: [f32; 3] = [0.584078, 0.552011, 0.508881];
/// `#D9744A` — the sole accent. Reserved for active state — the active brush,
/// the layer being sculpted, an engaged toggle — and used at the scale of a
/// rail, a mark or a label rather than filling anything.
pub const ACCENT: [f32; 3] = [0.693872, 0.174647, 0.068478];

/// What each constant above was converted from, so the test can check them
/// rather than trusting that someone did the arithmetic correctly.
#[cfg(test)]
const SOURCES: [(&str, u32, [f32; 3]); 8] = [
    ("VIEWPORT", 0x1B1E22, VIEWPORT),
    ("GROUND", 0x23262B, GROUND),
    ("PANEL", 0x2E3238, PANEL),
    ("RAISED", 0x3A3E45, RAISED),
    ("GRID_MINOR", 0x262A2F, GRID_MINOR),
    ("GRID_AXIS", 0x32363C, GRID_AXIS),
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

    fn luminance(c: [f32; 3]) -> f32 {
        0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
    }

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

    #[allow(clippy::assertions_on_constants)] // see design.rs
    #[test]
    fn the_palette_is_dark_where_the_design_says_it_is() {
        for (name, color) in [
            ("viewport", VIEWPORT),
            ("ground", GROUND),
            ("panel", PANEL),
            ("raised", RAISED),
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

    /// The ladder, asserted as an order rather than as six hex values.
    ///
    /// The next retune has to keep the relationship rather than re-derive it:
    /// a change that lightens the viewport past the shell would leave the
    /// sculpt sitting *above* the chrome it is supposed to recede from, and
    /// four sorted constants are not a thing anyone notices going unsorted.
    #[test]
    fn the_surfaces_climb_from_the_viewport_to_the_raised_row() {
        let ladder = [
            ("viewport", VIEWPORT),
            ("ground", GROUND),
            ("panel", PANEL),
            ("raised", RAISED),
        ];
        for pair in ladder.windows(2) {
            let ((below, dark), (above, light)) = (pair[0], pair[1]);
            assert!(
                luminance(light) > luminance(dark),
                "{above} is not lighter than {below}, so the surface ladder \
                 no longer reads as one surface sitting on another"
            );
        }
    }

    #[test]
    fn the_grid_sits_above_the_viewport_but_below_the_foreground() {
        assert!(
            luminance(VIEWPORT) < luminance(GRID_MINOR),
            "the grid must be visible against the ground it is drawn on, \
             which is the viewport's and not the shell's"
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

    /// Encodes a linear channel back to the 8-bit sRGB the design states.
    fn byte(linear: f32) -> f32 {
        let c = linear.clamp(0.0, 1.0);
        let s = if c <= 0.0031308 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        s * 255.0
    }

    /// The grid keeps the distance above its ground that it was tuned for.
    ///
    /// It was chosen against `#23262B` and now stands on `#1B1E22`. Dropping
    /// the ground without dropping the lines would widen the step and make the
    /// grid *more* CAD-like, which is the opposite of where the design is
    /// going — so both moved together, and this says by how much.
    ///
    /// Measured in sRGB rather than in linear light. The two are not the same
    /// distance: down at these values the curve is steep, so an equal step in
    /// 8-bit sRGB is a much smaller one in linear, and asserting linear
    /// equality here failed on tones that look identically spaced. sRGB is
    /// where the design writes its colours and roughly where an eye judges
    /// them, so it is the space the invariant belongs in.
    #[test]
    fn the_grid_kept_its_step_when_the_ground_dropped() {
        // What the lines were before the viewport was given a ground of its
        // own, and what they stood on then.
        let was = [
            ("minor", srgb(0x2E, 0x32, 0x38), GRID_MINOR),
            ("axis", srgb(0x3A, 0x3E, 0x45), GRID_AXIS),
        ];
        for (name, before, now) in was {
            for i in 0..3 {
                let then = byte(before[i]) - byte(GROUND[i]);
                let today = byte(now[i]) - byte(VIEWPORT[i]);
                assert!(
                    (today - then).abs() < 1.0,
                    "the grid's {name} lines stand {today:.1} sRGB steps above \
                     their ground in channel {i}, where they stood {then:.1} \
                     above the old one — retuning the viewport means retuning \
                     the lines drawn on it"
                );
            }
        }
    }
}
