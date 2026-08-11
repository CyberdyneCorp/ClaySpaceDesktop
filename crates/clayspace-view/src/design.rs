//! The design system, as values the interface is built from.
//!
//! The design states a style budget rather than a set of components: 60
//! minimalism, 20 skeuomorphism, 10 space-UI, 10 HUD, with the skeuomorphic
//! share spent on the brush swatches and the pressure control and nowhere
//! else. That is a rule about restraint, so what lives here is a small,
//! closed set of tokens — and anything drawn outside them is drift.

use crate::palette;

/// The spacing scale, in logical pixels.
///
/// One scale for gaps, padding and control heights alike, so panels align
/// across regions without each choosing its own rhythm.
pub mod space {
    /// Hairline separation.
    pub const HAIR: f32 = 2.0;
    /// Between related controls.
    pub const TIGHT: f32 = 4.0;
    /// The default gap.
    pub const SNUG: f32 = 8.0;
    /// Between groups within a panel.
    pub const ROOMY: f32 = 12.0;
    /// Between sections.
    pub const SECTION: f32 = 20.0;
    /// A panel's inner padding.
    pub const PANEL: f32 = 14.0;
}

/// Control sizing, in logical pixels.
pub mod size {
    /// A row in a list — scene entries, layer entries.
    pub const ROW: f32 = 26.0;
    /// A slider or field.
    pub const CONTROL: f32 = 22.0;
    /// A tool button in the rail.
    pub const RAIL_BUTTON: f32 = 34.0;
    /// A brush swatch in the shelf.
    pub const SWATCH: f32 = 54.0;
    /// Icon side.
    pub const ICON: f32 = 16.0;
    /// Corner radius. Small: the design is flat, and a large radius reads as
    /// a card rather than a surface.
    pub const RADIUS: f32 = 3.0;
}

/// Type sizes, in logical pixels.
pub mod type_scale {
    /// Section headings — small, spaced, low contrast.
    pub const HEADING: f32 = 10.0;
    /// Ordinary labels.
    pub const LABEL: f32 = 12.0;
    /// Body and list rows.
    pub const BODY: f32 = 13.0;
    /// Numeric readouts, set in a monospaced face.
    pub const NUMERIC: f32 = 12.0;
    /// The window title.
    pub const TITLE: f32 = 14.0;
}

/// Converts a linear palette colour to an egui colour.
///
/// egui composites in sRGB space, so the linear tokens the renderer uses are
/// encoded here rather than passed through — the same conversion in the other
/// direction, and for the same reason.
pub fn color(linear: [f32; 3]) -> egui::Color32 {
    let encode = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.0031308 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0 + 0.5) as u8
    };
    egui::Color32::from_rgb(encode(linear[0]), encode(linear[1]), encode(linear[2]))
}

/// A colour from the palette, dimmed toward the ground.
///
/// How the "quiet until addressed" rule is expressed: an inactive control is
/// the same hue at lower contrast, not a different colour.
pub fn dim(linear: [f32; 3], amount: f32) -> egui::Color32 {
    let t = amount.clamp(0.0, 1.0);
    let mixed = [
        linear[0] * (1.0 - t) + palette::GROUND[0] * t,
        linear[1] * (1.0 - t) + palette::GROUND[1] * t,
        linear[2] * (1.0 - t) + palette::GROUND[2] * t,
    ];
    color(mixed)
}

/// The named colours the interface draws with.
///
/// A closed set: a component reaching past these is the drift the style budget
/// exists to prevent, and `no_literal_colors` in the shell tests says so.
pub struct Tokens;

impl Tokens {
    /// The application ground.
    pub fn ground() -> egui::Color32 {
        color(palette::GROUND)
    }

    /// A panel sitting on the ground.
    pub fn panel() -> egui::Color32 {
        color(palette::GRID_MINOR)
    }

    /// A raised surface — a selected row, a pressed control.
    pub fn raised() -> egui::Color32 {
        color(palette::GRID_AXIS)
    }

    /// Separators. Distinguished by tone, never by a drawn outline that
    /// competes for attention.
    pub fn rule() -> egui::Color32 {
        dim(palette::GRID_AXIS, 0.2)
    }

    /// Primary text.
    pub fn text() -> egui::Color32 {
        color(palette::FOREGROUND)
    }

    /// Secondary text — labels at rest.
    ///
    /// Dimmed to 0.37 rather than further. The contrast floor is what caps it:
    /// against a panel, 0.446 is the most that still clears 4.5:1 and 0.37
    /// leaves headroom for a lighter panel. The quiet-until-addressed rule
    /// does not override legibility, and the tests here decide which wins.
    pub fn text_dim() -> egui::Color32 {
        dim(palette::FOREGROUND, 0.37)
    }

    /// Section headings.
    ///
    /// Quieter than body text by *size and letter-spacing*, not by dropping
    /// below the floor — they are still text a user has to read.
    pub fn text_faint() -> egui::Color32 {
        dim(palette::FOREGROUND, 0.42)
    }

    /// The sole accent. Marks the active brush and active tool state, and
    /// nothing else.
    pub fn accent() -> egui::Color32 {
        color(palette::ACCENT)
    }
}

/// Relative luminance of an sRGB colour, per WCAG.
fn luminance(color: egui::Color32) -> f64 {
    let channel = |v: u8| {
        let c = v as f64 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

/// Contrast ratio between two colours, per WCAG.
///
/// The design asks for an interface that stays quiet; the floors are what stop
/// quiet becoming unreadable.
pub fn contrast(a: egui::Color32, b: egui::Color32) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    let (lighter, darker) = if x > y { (x, y) } else { (y, x) };
    (lighter + 0.05) / (darker + 0.05)
}

/// The floor for text and essential indicators.
pub const TEXT_CONTRAST_FLOOR: f64 = 4.5;
/// The floor for non-text indicators that carry state.
pub const INDICATOR_CONTRAST_FLOOR: f64 = 3.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_text_clears_the_contrast_floor_on_every_surface() {
        for (name, surface) in [
            ("ground", Tokens::ground()),
            ("panel", Tokens::panel()),
            ("raised", Tokens::raised()),
        ] {
            let ratio = contrast(Tokens::text(), surface);
            assert!(
                ratio >= TEXT_CONTRAST_FLOOR,
                "primary text on {name} is {ratio:.2}:1, below the {TEXT_CONTRAST_FLOOR}:1 floor"
            );
        }
    }

    #[test]
    fn secondary_text_clears_the_floor_where_it_is_used() {
        // Dim text is used on panels, not on the ground.
        let ratio = contrast(Tokens::text_dim(), Tokens::panel());
        assert!(
            ratio >= TEXT_CONTRAST_FLOOR,
            "secondary text is {ratio:.2}:1 against a panel, below the floor — \
             the quiet-until-addressed rule does not override legibility"
        );
    }

    #[test]
    fn headings_clear_the_floor_even_at_their_faintest() {
        let ratio = contrast(Tokens::text_faint(), Tokens::panel());
        assert!(
            ratio >= TEXT_CONTRAST_FLOOR,
            "section headings are {ratio:.2}:1, below the floor"
        );
    }

    #[test]
    fn the_accent_is_legible_where_state_depends_on_it() {
        for (name, surface) in [("ground", Tokens::ground()), ("panel", Tokens::panel())] {
            let ratio = contrast(Tokens::accent(), surface);
            assert!(
                ratio >= INDICATOR_CONTRAST_FLOOR,
                "the accent on {name} is {ratio:.2}:1, below the {INDICATOR_CONTRAST_FLOOR}:1 \
                 floor for an indicator that carries state"
            );
        }
    }

    #[test]
    fn surfaces_are_distinguishable_from_one_another() {
        // Panels are told apart from the ground by tone, so the tones must
        // actually differ.
        let steps = [Tokens::ground(), Tokens::panel(), Tokens::raised()];
        for pair in steps.windows(2) {
            assert!(
                luminance(pair[1]) > luminance(pair[0]),
                "two surface tones are not ordered, so a panel would not read \
                 as sitting on the ground"
            );
        }
    }

    #[test]
    fn the_scale_is_ordered_and_positive() {
        let scale = [
            space::HAIR,
            space::TIGHT,
            space::SNUG,
            space::ROOMY,
            space::SECTION,
        ];
        for pair in scale.windows(2) {
            assert!(pair[1] > pair[0], "the spacing scale is not ordered");
        }
        assert!(scale.iter().all(|v| *v > 0.0));
    }

    #[test]
    fn headings_are_smaller_than_body_text() {
        assert!(
            type_scale::HEADING < type_scale::BODY,
            "section headings must be set small and spaced, not as titles"
        );
        assert!(type_scale::LABEL <= type_scale::BODY);
    }

    #[test]
    fn dimming_moves_toward_the_ground_rather_than_to_grey() {
        // A control at rest is the same hue at lower contrast; shifting toward
        // neutral grey would read as a different colour rather than a quieter
        // one.
        let full = Tokens::accent();
        let quiet = dim(palette::ACCENT, 0.5);
        assert!(
            quiet.r() > quiet.b(),
            "the dimmed accent lost its hue: {quiet:?}"
        );
        assert!(luminance(quiet) < luminance(full));
    }
}
