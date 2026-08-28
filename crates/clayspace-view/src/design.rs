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
    /// What a layer row keeps for its right-hand side: the representation tag,
    /// the intensity, and a protection icon where there is one.
    ///
    /// A name is bounded by it rather than by egui's own layout, which hands a
    /// right-to-left group whatever is left — and that is everything when the
    /// name in front of it was never bounded.
    pub const LAYER_ROW_TAIL: f32 = 84.0;

    /// A slider or field.
    pub const CONTROL: f32 = 22.0;
    /// A brush swatch in the shelf.
    pub const SWATCH: f32 = 54.0;
    /// The active brush's ball at the head of the options bar: what the bar's
    /// height leaves after its padding.
    pub const BADGE: f32 = 42.0;
    /// Icon side, in a list row.
    pub const ICON: f32 = 16.0;
    /// A button on the tool rail: the rail's width less a margin a side.
    pub const RAIL_BUTTON: f32 = 34.0;
    /// Icon side on a chip, where it stands for the whole control rather
    /// than annotating a row, and where two discs at sixteen pixels were a
    /// smudge. As tall as the control's own padding allows.
    pub const CHIP_ICON: f32 = 20.0;
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
/// exists to prevent, and `no_literal_colors` below says so by reading the
/// crate's own source.
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

    /// The tint that leaves an image as it is.
    ///
    /// Not a colour on screen: egui multiplies a drawn image by a tint, and
    /// this is the one that changes nothing. Named here so the material
    /// preview does not have to write a colour down to draw its own texture.
    pub fn untinted() -> egui::Color32 {
        egui::Color32::WHITE
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

    /// How many domain `label()` calls the shell still draws.
    ///
    /// A ratchet, not a clean bill. `Strings` is where interface words live —
    /// `tool_names` says so, and `combine.rs` says "Not `Combine::label`,
    /// which is interface text and translated" beside a `label` that was not.
    /// The combine picker, the blend profiles, the extrude sides and the voxel
    /// display are in the table now; these are not, so a sculptor on English
    /// or Spanish still meets Portuguese in them.
    ///
    /// The number may go **down** freely. It going *up* means a new control
    /// was wired to a domain label instead of to the table, which is the
    /// mistake this exists to stop repeating while the backlog is worked off.
    /// Fixing one is: add an array to `Strings` keyed off the enum's `::ALL`,
    /// fill all three locales, add an accessor, and call it here.
    const LABELS_STILL_DRAWN: usize = 18;

    #[test]
    fn the_shell_draws_no_new_untranslated_labels() {
        let shell = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shell.rs");
        let text = std::fs::read_to_string(&shell).expect("the shell's source");
        let drawn = text.matches(".label()").count();
        assert!(
            drawn <= LABELS_STILL_DRAWN,
            "the shell draws {drawn} domain labels, up from {LABELS_STILL_DRAWN}. \
             A new control was wired to a domain `label()` rather than to \
             `Strings`, so it will read in Portuguese on every other locale — \
             see `Strings::combine_name` for the shape to follow"
        );
        assert!(
            drawn >= LABELS_STILL_DRAWN,
            "the shell draws {drawn} domain labels, down from \
             {LABELS_STILL_DRAWN} — lower `LABELS_STILL_DRAWN` to {drawn} so \
             the ratchet holds the ground that was just taken"
        );
    }

    /// What a component writes above a `Color32::from_*` that shades a colour
    /// it was handed rather than inventing one. Spelled out once so the
    /// marker and the test cannot drift apart.
    const DERIVED: &str = "derived from a token";

    /// Every colour on screen comes from a named token.
    ///
    /// The design-system requirement as a test rather than as prose. It has to
    /// read the source, because that is where the drift is: a hand-mixed grey
    /// looks like a token in a capture and stops looking like one the day the
    /// token moves, so no rendered frame can catch it.
    ///
    /// `design.rs` and `palette.rs` are where colours are made and are
    /// therefore exempt. Everywhere else may shade a colour it was *given* —
    /// the material previews light their sphere from the upper left — and says
    /// so on the line above.
    #[test]
    fn no_literal_colors() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        for entry in std::fs::read_dir(&src).expect("the crate's own source") {
            let path = entry.expect("entry").path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned();
            if path.extension().is_none_or(|ext| ext != "rs")
                || name == "design.rs"
                || name == "palette.rs"
            {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read the source");
            let lines: Vec<&str> = text.lines().collect();
            for (at, line) in lines.iter().enumerate() {
                // Both halves of "written down": a constructor call, and a
                // named constant. Matching only `from_` left `Color32::RED`
                // and its siblings walking through a test whose own paragraph
                // above says every colour comes from a token — verified by
                // mutation, an inserted `Color32::RED` kept this green.
                let written_down = line.contains("Color32::from_")
                    || line
                        .split("Color32::")
                        .skip(1)
                        .any(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()));
                if !written_down {
                    continue;
                }
                let derived = at
                    .checked_sub(1)
                    .is_some_and(|above| lines[above].contains(DERIVED));
                if !derived {
                    offenders.push(format!("{name}:{}", at + 1));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "a colour was written down rather than taken from a token, at {}; \
             add it to `Tokens`, or mark the line \"{DERIVED}\" if it shades \
             one it was given",
            offenders.join(", ")
        );
    }

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

    // These compare constants, which clippy notices. They stay as tests rather
    // than `const _: () = assert!(..)` because the message is the point: when
    // one fails it should say what rule of the design was broken, and a const
    // assertion cannot carry one.
    #[allow(clippy::assertions_on_constants)]
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
