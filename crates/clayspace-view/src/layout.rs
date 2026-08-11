//! Panel sizes and collapse state, and where they are remembered.
//!
//! Layout is not document state and never enters the undo history, but it is
//! state a user expects to survive a restart. It lives here, as plain values
//! the composition root persists.

use crate::shell::region;

/// One resizable region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    Left,
    Right,
    Shelf,
}

impl Panel {
    pub const ALL: [Panel; 3] = [Self::Left, Self::Right, Self::Shelf];

    /// The size the design specifies, which is also the reset target.
    pub fn default_size(self) -> f32 {
        match self {
            Self::Left => region::LEFT,
            Self::Right => region::RIGHT,
            Self::Shelf => region::SHELF,
        }
    }

    /// The narrowest a panel may be dragged before it stops being usable.
    pub fn minimum(self) -> f32 {
        match self {
            Self::Left | Self::Right => 160.0,
            Self::Shelf => 60.0,
        }
    }

    /// The widest, so a panel cannot swallow the viewport.
    pub fn maximum(self) -> f32 {
        match self {
            Self::Left | Self::Right => 420.0,
            Self::Shelf => 200.0,
        }
    }
}

/// How the regions are currently arranged.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    sizes: [f32; 3],
    collapsed: [bool; 3],
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            sizes: Panel::ALL.map(Panel::default_size),
            collapsed: [false; 3],
        }
    }
}

impl Layout {
    fn index(panel: Panel) -> usize {
        match panel {
            Panel::Left => 0,
            Panel::Right => 1,
            Panel::Shelf => 2,
        }
    }

    /// How wide or tall a panel is. A collapsed panel reports zero, because
    /// that is what the layout must actually give it.
    pub fn size(&self, panel: Panel) -> f32 {
        if self.is_collapsed(panel) {
            0.0
        } else {
            self.sizes[Self::index(panel)]
        }
    }

    /// The size it will return to when expanded.
    pub fn stored_size(&self, panel: Panel) -> f32 {
        self.sizes[Self::index(panel)]
    }

    pub fn is_collapsed(&self, panel: Panel) -> bool {
        self.collapsed[Self::index(panel)]
    }

    /// Resizes, clamped so a panel can neither vanish nor swallow the viewport.
    pub fn resize(&mut self, panel: Panel, size: f32) {
        let clamped = size.clamp(panel.minimum(), panel.maximum());
        self.sizes[Self::index(panel)] = clamped;
    }

    pub fn set_collapsed(&mut self, panel: Panel, collapsed: bool) {
        self.collapsed[Self::index(panel)] = collapsed;
    }

    pub fn toggle(&mut self, panel: Panel) {
        let index = Self::index(panel);
        self.collapsed[index] = !self.collapsed[index];
    }

    /// Returns every region to the design's size and expansion.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Serialises to a line the composition root can store.
    ///
    /// A hand-rolled format rather than a serialisation dependency: this is
    /// six numbers, and a malformed line must fall back to the default rather
    /// than fail to start.
    pub fn serialize(&self) -> String {
        format!(
            "{:.1},{:.1},{:.1},{},{},{}",
            self.sizes[0],
            self.sizes[1],
            self.sizes[2],
            self.collapsed[0] as u8,
            self.collapsed[1] as u8,
            self.collapsed[2] as u8,
        )
    }

    /// Reads a stored line, falling back to the default on anything malformed.
    ///
    /// A layout is a convenience; a corrupt one must never stop the
    /// application starting.
    pub fn deserialize(text: &str) -> Self {
        let parts: Vec<&str> = text.trim().split(',').collect();
        if parts.len() != 6 {
            return Self::default();
        }
        let mut layout = Self::default();
        for (index, panel) in Panel::ALL.iter().enumerate() {
            match parts[index].parse::<f32>() {
                Ok(size) if size.is_finite() => layout.resize(*panel, size),
                _ => return Self::default(),
            }
            layout.collapsed[index] = parts[index + 3] == "1";
        }
        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_designs_sizes() {
        let layout = Layout::default();
        assert_eq!(layout.size(Panel::Left), region::LEFT);
        assert_eq!(layout.size(Panel::Right), region::RIGHT);
        assert!(!layout.is_collapsed(Panel::Left));
    }

    #[test]
    fn a_panel_can_neither_vanish_nor_swallow_the_viewport() {
        let mut layout = Layout::default();
        layout.resize(Panel::Left, 0.0);
        assert!(layout.size(Panel::Left) >= Panel::Left.minimum());

        layout.resize(Panel::Left, 5000.0);
        assert!(layout.size(Panel::Left) <= Panel::Left.maximum());
    }

    #[test]
    fn collapsing_reports_zero_but_remembers_the_size() {
        let mut layout = Layout::default();
        layout.resize(Panel::Right, 300.0);
        layout.set_collapsed(Panel::Right, true);

        assert_eq!(
            layout.size(Panel::Right),
            0.0,
            "the layout must give it nothing"
        );
        assert_eq!(
            layout.stored_size(Panel::Right),
            300.0,
            "expanding must return the size the user set, not a default"
        );

        layout.toggle(Panel::Right);
        assert_eq!(layout.size(Panel::Right), 300.0);
    }

    #[test]
    fn reset_returns_every_region_at_once() {
        let mut layout = Layout::default();
        for panel in Panel::ALL {
            layout.resize(panel, panel.maximum());
            layout.set_collapsed(panel, true);
        }
        layout.reset();
        assert_eq!(layout, Layout::default());
    }

    #[test]
    fn a_layout_survives_a_round_trip() {
        let mut layout = Layout::default();
        layout.resize(Panel::Left, 280.0);
        layout.set_collapsed(Panel::Shelf, true);

        let restored = Layout::deserialize(&layout.serialize());
        assert_eq!(restored, layout, "the layout did not survive being stored");
    }

    #[test]
    fn a_corrupt_layout_falls_back_rather_than_failing() {
        for text in ["", "nonsense", "1,2,3", "a,b,c,0,0,0", "NaN,1,1,0,0,0"] {
            let layout = Layout::deserialize(text);
            assert_eq!(
                layout,
                Layout::default(),
                "a malformed layout ({text:?}) must not stop the application starting"
            );
        }
    }

    #[test]
    fn a_stored_size_outside_the_bounds_is_clamped_on_load() {
        // A layout written by a version with different bounds must still load.
        let layout = Layout::deserialize("9999.0,1.0,50.0,0,0,0");
        assert!(layout.size(Panel::Left) <= Panel::Left.maximum());
        assert!(layout.size(Panel::Right) >= Panel::Right.minimum());
    }
}
