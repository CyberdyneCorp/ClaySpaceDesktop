//! What a length means, and what it is shown in.
//!
//! Two separate questions, and conflating them is the classic way to lose a
//! model's scale. The *working* unit says what one engine unit measures — it
//! is a property of the document, chosen once, and changing it would rescale
//! everything in it. The *display* unit is what a readout is written in, and
//! changing it changes nothing but the text.
//!
//! The specification asks for presentation-only switching. That is the whole
//! of it: `display` is free to change at any time, `working` is not.

/// A unit of length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Millimetre,
    Centimetre,
    Metre,
    Inch,
}

impl Unit {
    pub const ALL: [Unit; 4] = [Self::Millimetre, Self::Centimetre, Self::Metre, Self::Inch];

    /// The abbreviation, which is the same in both locales this ships with.
    pub fn label(self) -> &'static str {
        match self {
            Self::Millimetre => "mm",
            Self::Centimetre => "cm",
            Self::Metre => "m",
            Self::Inch => "in",
        }
    }

    /// How many metres one of these is.
    ///
    /// Everything converts through metres rather than through each other, so
    /// there is one conversion table and not sixteen.
    pub fn in_metres(self) -> f32 {
        match self {
            Self::Millimetre => 0.001,
            Self::Centimetre => 0.01,
            Self::Metre => 1.0,
            Self::Inch => 0.0254,
        }
    }

    /// How many decimals a readout in this unit is worth.
    ///
    /// A sculpt is a few hundred millimetres across. Showing it in metres to
    /// two decimals rounds an entire head to "0.15", so the coarser the unit
    /// the more decimals it earns.
    pub fn decimals(self) -> usize {
        match self {
            Self::Millimetre => 1,
            Self::Centimetre => 2,
            Self::Metre => 3,
            Self::Inch => 2,
        }
    }
}

/// The document's scale, and what readouts are written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Units {
    /// What one engine unit measures. A property of the document.
    pub working: Unit,
    /// What lengths are shown in. Presentation only.
    pub display: Unit,
}

impl Default for Units {
    fn default() -> Self {
        // A sculpt is a hand-sized thing and the design's status bar says mm,
        // so one engine unit is one centimetre and lengths read in
        // millimetres: the starting sphere is then 1000 mm rather than 10.
        Self {
            working: Unit::Centimetre,
            display: Unit::Millimetre,
        }
    }
}

impl Units {
    /// An engine length, in the display unit.
    pub fn to_display(&self, engine: f32) -> f32 {
        engine * self.working.in_metres() / self.display.in_metres()
    }

    /// A length typed in the display unit, back in engine units.
    pub fn from_display(&self, shown: f32) -> f32 {
        shown * self.display.in_metres() / self.working.in_metres()
    }

    /// An engine length as a readout, with its unit.
    pub fn format(&self, engine: f32) -> String {
        format!(
            "{:.*} {}",
            self.display.decimals(),
            self.to_display(engine),
            self.display.label()
        )
    }

    /// The next display unit, for a control that cycles.
    pub fn next_display(&self) -> Unit {
        let index = Unit::ALL
            .iter()
            .position(|candidate| *candidate == self.display)
            .unwrap_or(0);
        Unit::ALL[(index + 1) % Unit::ALL.len()]
    }
}

/// A document that has a scale.
pub trait UnitsModel {
    fn units(&self) -> Units;
    /// Changes what readouts are written in. Never touches the geometry.
    fn set_display_unit(&mut self, unit: Unit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_the_display_unit_changes_no_length() {
        // The property the specification asks for, stated directly: the same
        // engine value means the same physical size whatever it is shown in.
        let mut units = Units::default();
        let engine = 12.5f32;
        let in_metres = units.to_display(engine) * units.display.in_metres();

        for unit in Unit::ALL {
            units.display = unit;
            let shown = units.to_display(engine);
            assert!(
                (shown * unit.in_metres() - in_metres).abs() < 1e-4,
                "{unit:?} changed the length: {shown} {} is not {in_metres} m",
                unit.label()
            );
        }
    }

    #[test]
    fn a_length_round_trips_through_the_display_unit() {
        for unit in Unit::ALL {
            let units = Units {
                working: Unit::Centimetre,
                display: unit,
            };
            let engine = 3.75f32;
            let back = units.from_display(units.to_display(engine));
            assert!(
                (back - engine).abs() < 1e-4,
                "{unit:?} did not round trip: {back} against {engine}"
            );
        }
    }

    #[test]
    fn the_default_reads_a_hand_sized_sculpt_in_whole_millimetres() {
        // One engine unit is a centimetre, so the starting sphere's radius of
        // 0.5 is 5 mm — a number a person can hold in their head.
        let units = Units::default();
        assert_eq!(units.format(0.5), "5.0 mm");
        assert_eq!(units.format(10.0), "100.0 mm");
    }

    #[test]
    fn a_coarser_unit_earns_more_decimals() {
        // Two decimals of metres rounds a whole head to 0.15.
        let mut units = Units::default();
        units.display = Unit::Metre;
        assert_eq!(units.format(15.0), "0.150 m");
        units.display = Unit::Inch;
        assert_eq!(units.format(2.54), "1.00 in");
    }

    #[test]
    fn cycling_visits_every_unit_and_returns() {
        let mut units = Units::default();
        let start = units.display;
        let mut seen = vec![start];
        for _ in 1..Unit::ALL.len() {
            units.display = units.next_display();
            assert!(!seen.contains(&units.display), "cycling repeated a unit");
            seen.push(units.display);
        }
        units.display = units.next_display();
        assert_eq!(units.display, start, "cycling did not come back around");
    }

    #[test]
    fn the_working_unit_scales_what_the_display_unit_reads() {
        // A document authored in metres reads a thousand times larger in mm
        // than one authored in millimetres, for the same engine value.
        let millimetre = Units {
            working: Unit::Millimetre,
            display: Unit::Millimetre,
        };
        let metre = Units {
            working: Unit::Metre,
            display: Unit::Millimetre,
        };
        assert!((millimetre.to_display(1.0) - 1.0).abs() < 1e-4);
        // Not an exact comparison: 1.0 / 0.001 in f32 is 999.99994, and a
        // conversion table that goes through metres will always leave a
        // residue like that. What matters is that a readout rounds right.
        assert!((metre.to_display(1.0) - 1000.0).abs() < 1e-2);
        assert_eq!(metre.format(1.0), "1000.0 mm");
    }
}
