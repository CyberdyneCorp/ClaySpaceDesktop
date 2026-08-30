//! The colour a colour brush paints with.
//!
//! One value for the session rather than one per tool, and that is a decision
//! rather than an omission. Brush *size* belongs to the tool and to the
//! representation — this application already stores it that way, so a small
//! detail brush stays small when the blockout brush is made large. A *colour*
//! is the opposite: it is what the sculptor is painting with at this moment,
//! and every colour brush picks up the same one. ZBrush works that way and so
//! does every application with a palette in it.
//!
//! Which is also why it is not in [`crate::BrushSettings`]: that struct is
//! copied per tool and per representation on every read, and a colour stored
//! there would mean Pintar and Borrar disagreeing about the current colour on
//! a mesh and again on a grid — four values for one question.

/// A colour, in the linear RGB the engine's palettes and vertex colours use.
///
/// No alpha. A voxel palette entry is three floats and a mesh paint stamp
/// blends toward three floats; there is nothing here for a fourth to mean, and
/// carrying one would be a control that does nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colour {
    pub rgb: [f32; 3],
}

impl Colour {
    pub const fn new(rgb: [f32; 3]) -> Self {
        Self { rgb }
    }

    /// The colour a fresh grid deposits, and where the swatch starts.
    ///
    /// The same neutral tone `stroke_voxel` has always added to an empty
    /// palette, so opening the swatch shows the colour the clay already is
    /// rather than proposing a change nobody asked for.
    pub const CLAY: Self = Self::new([0.78, 0.76, 0.73]);

    /// Clamped to the unit cube, and every channel finite.
    ///
    /// A colour reaches the engine as a palette entry, which is stored and
    /// serialized; a NaN written there comes back in every document that
    /// referenced it.
    pub fn sanitized(self) -> Self {
        Self {
            rgb: self.rgb.map(|c| {
                if c.is_finite() {
                    c.clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }),
        }
    }

    /// How far apart two colours are, as the largest per-channel difference.
    ///
    /// Chebyshev rather than Euclidean because the question this answers is
    /// "would anyone see the difference", and a channel that is off by a tenth
    /// is visible whether or not the other two agree.
    pub fn distance(self, other: Self) -> f32 {
        (0..3)
            .map(|c| (self.rgb[c] - other.rgb[c]).abs())
            .fold(0.0, f32::max)
    }

    /// `#RRGGBB`, for the swatch's label and for anything that writes a colour
    /// down.
    pub fn hex(self) -> String {
        let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!(
            "#{:02X}{:02X}{:02X}",
            byte(self.rgb[0]),
            byte(self.rgb[1]),
            byte(self.rgb[2])
        )
    }

    /// The colour a `#RRGGBB` string names, if it names one.
    pub fn from_hex(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#').unwrap_or(text);
        if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| {
            u8::from_str_radix(&digits[at..at + 2], 16)
                .ok()
                .map(f32::from)
        };
        Some(Self::new([
            channel(0)? / 255.0,
            channel(2)? / 255.0,
            channel(4)? / 255.0,
        ]))
    }
}

impl Default for Colour {
    fn default() -> Self {
        Self::CLAY
    }
}

/// The current colour and the ones just before it.
///
/// The recent list is what makes a two-colour pass workable without a saved
/// palette: paint red, paint blue, and red is one click away. It is a *stack of
/// distinct colours*, so choosing one already in the list moves it to the front
/// rather than growing a run of duplicates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColourState {
    current: Colour,
    recent: Vec<Colour>,
}

impl ColourState {
    /// How many previous colours are kept.
    ///
    /// Six: enough for the two or three a pass actually uses plus room to go
    /// back, and short enough to sit in a row beside the swatch without
    /// becoming a palette editor, which is a different feature.
    pub const RECENT: usize = 6;

    /// How close two colours must be to count as the same one.
    ///
    /// A colour picker hands back values that differ in the last bits of a
    /// float as the pointer moves within one pixel of the wheel. Without a
    /// tolerance the recent list fills with six shades of the same red and a
    /// grid's palette grows an entry per stroke. Half a step of an eight-bit
    /// channel: two colours that round to the same `#RRGGBB` are the same
    /// colour, and any two that do not are different.
    pub const SAME: f32 = 0.5 / 255.0;

    pub fn current(&self) -> Colour {
        self.current
    }

    pub fn recent(&self) -> &[Colour] {
        &self.recent
    }

    /// Chooses a colour, remembering the one it replaced.
    ///
    /// The outgoing colour goes into the list rather than the incoming one:
    /// the current colour is already shown in the swatch, and a list that
    /// repeated it would spend an entry saying what the swatch says.
    pub fn choose(&mut self, colour: Colour) {
        let colour = colour.sanitized();
        if colour.distance(self.current) <= Self::SAME {
            return;
        }
        let outgoing = self.current;
        self.current = colour;
        // Neither end of the swap stays in the list twice: the outgoing colour
        // is put at the front, and the incoming one leaves it — it is in the
        // swatch now, and a recent square repeating the current colour is a
        // click that does nothing.
        self.recent.retain(|kept| {
            kept.distance(outgoing) > Self::SAME && kept.distance(colour) > Self::SAME
        });
        self.recent.insert(0, outgoing);
        self.recent.truncate(Self::RECENT);
    }

    /// Chooses the `index`th recent colour, if there is one there.
    ///
    /// Returns whether anything changed, so a caller does not schedule a
    /// redraw for a click on nothing.
    pub fn choose_recent(&mut self, index: usize) -> bool {
        let Some(colour) = self.recent.get(index).copied() else {
            return false;
        };
        self.choose(colour);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_round_trips_through_its_hex() {
        let colour = Colour::new([0.72, 0.42, 0.28]);
        let back = Colour::from_hex(&colour.hex()).expect("its own hex parses");
        assert!(
            colour.distance(back) <= ColourState::SAME,
            "{} came back as {}",
            colour.hex(),
            back.hex()
        );
    }

    #[test]
    fn a_malformed_hex_names_no_colour() {
        for text in ["", "#12345", "#1234567", "#GGGGGG", "red"] {
            assert_eq!(Colour::from_hex(text), None, "{text} was accepted");
        }
    }

    #[test]
    fn a_colour_is_clamped_and_finite() {
        let wild = Colour::new([f32::NAN, 2.0, -1.0]).sanitized();
        assert_eq!(wild.rgb, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn choosing_remembers_what_it_replaced() {
        let mut state = ColourState::default();
        let first = state.current();
        let red = Colour::new([1.0, 0.0, 0.0]);
        state.choose(red);
        assert_eq!(state.current(), red);
        assert_eq!(state.recent(), &[first]);
    }

    #[test]
    fn choosing_the_same_colour_again_costs_no_entry() {
        let mut state = ColourState::default();
        let red = Colour::new([1.0, 0.0, 0.0]);
        state.choose(red);
        state.choose(red);
        state.choose(Colour::new([1.0, 0.001, 0.0]));
        assert_eq!(
            state.recent().len(),
            1,
            "the list grew for a colour nobody could tell apart"
        );
    }

    #[test]
    fn a_colour_chosen_again_moves_up_rather_than_appearing_twice() {
        let mut state = ColourState::default();
        let red = Colour::new([1.0, 0.0, 0.0]);
        let blue = Colour::new([0.0, 0.0, 1.0]);
        state.choose(red);
        state.choose(blue);
        state.choose(red);
        // Blue is now the outgoing colour and red is current, so the list is
        // blue then the clay it started from — with no red in it, because red
        // is in the swatch.
        assert_eq!(state.current(), red);
        assert_eq!(state.recent(), &[blue, Colour::CLAY]);
    }

    #[test]
    fn the_recent_list_stops_at_its_limit() {
        let mut state = ColourState::default();
        for step in 0..(ColourState::RECENT + 4) {
            let value = step as f32 / 32.0;
            state.choose(Colour::new([value, 0.0, 0.0]));
        }
        assert_eq!(state.recent().len(), ColourState::RECENT);
    }

    #[test]
    fn picking_a_recent_colour_makes_it_current() {
        let mut state = ColourState::default();
        let clay = state.current();
        state.choose(Colour::new([1.0, 0.0, 0.0]));
        assert!(state.choose_recent(0));
        assert_eq!(state.current(), clay);
        assert!(
            !state.choose_recent(9),
            "an index past the list reported a change"
        );
    }
}
