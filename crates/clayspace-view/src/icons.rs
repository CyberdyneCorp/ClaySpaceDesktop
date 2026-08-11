//! One icon set, drawn rather than shipped.
//!
//! The design asks for a single line-based set sharing stroke weight, corner
//! treatment and optical size. Drawing them means that is enforceable — every
//! icon goes through the same painter with the same weight — where a folder of
//! SVGs would rely on whoever exported them.
//!
//! They are also deliberately few. An icon that has to be explained is worse
//! than the word it replaced, so anything ambiguous stays a label.

use crate::design::{size, Tokens};

/// The icons the interface uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// A layer or scene entry that is shown.
    Visible,
    /// One that is hidden.
    Hidden,
    /// A layer that refuses edits.
    Locked,
    /// A layer shown but not pickable.
    Ghost,
    /// Expands a tree entry.
    Collapsed,
    /// Collapses one.
    Expanded,
    /// Adds something.
    Add,
    /// Removes something.
    Remove,
}

impl Icon {
    pub const ALL: [Icon; 8] = [
        Self::Visible,
        Self::Hidden,
        Self::Locked,
        Self::Ghost,
        Self::Collapsed,
        Self::Expanded,
        Self::Add,
        Self::Remove,
    ];

    /// What a screen reader or a tooltip says.
    ///
    /// Every icon has one: state carried by shape alone is state a
    /// colour-blind or low-vision user cannot read.
    pub fn description(self) -> &'static str {
        match self {
            Self::Visible => "visível",
            Self::Hidden => "oculto",
            Self::Locked => "bloqueado",
            Self::Ghost => "fantasma",
            Self::Collapsed => "expandir",
            Self::Expanded => "recolher",
            Self::Add => "adicionar",
            Self::Remove => "remover",
        }
    }
}

/// The one stroke weight the set is drawn at.
pub const STROKE: f32 = 1.25;

/// Draws an icon centred in `rect`.
///
/// `tint` is passed rather than chosen so a caller can dim an icon at rest
/// without this module knowing about interaction states.
pub fn paint(painter: &egui::Painter, rect: egui::Rect, icon: Icon, tint: egui::Color32) {
    let centre = rect.center();
    // A shared optical size: every icon is drawn inside the same circle, so
    // none reads larger than another at the same nominal size.
    let unit = rect.width().min(rect.height()) * 0.5 * 0.72;
    let stroke = egui::Stroke::new(STROKE, tint);

    match icon {
        Icon::Visible => {
            painter.circle_stroke(centre, unit * 0.62, stroke);
            painter.circle_filled(centre, unit * 0.24, tint);
        }
        Icon::Hidden => {
            painter.circle_stroke(centre, unit * 0.62, stroke);
        }
        Icon::Locked => {
            // A shackle over a body, at the same weight as everything else.
            let body = egui::Rect::from_center_size(
                centre + egui::vec2(0.0, unit * 0.3),
                egui::vec2(unit * 1.1, unit * 0.85),
            );
            painter.rect_stroke(
                body,
                egui::epaint::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Middle,
            );
            painter.circle_stroke(centre + egui::vec2(0.0, -unit * 0.35), unit * 0.42, stroke);
        }
        Icon::Ghost => {
            // A dashed ring: present, but not something a ray will meet.
            let segments = 8;
            for i in 0..segments {
                if i % 2 == 1 {
                    continue;
                }
                let start = i as f32 / segments as f32 * std::f32::consts::TAU;
                let end = (i as f32 + 1.0) / segments as f32 * std::f32::consts::TAU;
                let points: Vec<egui::Pos2> = (0..=4)
                    .map(|step| {
                        let t = start + (end - start) * step as f32 / 4.0;
                        centre + egui::vec2(t.cos(), t.sin()) * unit * 0.62
                    })
                    .collect();
                painter.add(egui::Shape::line(points, stroke));
            }
        }
        Icon::Collapsed | Icon::Expanded => {
            // One chevron, rotated, so the two cannot drift apart.
            let (a, b, c) = if icon == Icon::Collapsed {
                (
                    egui::vec2(-unit * 0.3, -unit * 0.55),
                    egui::vec2(unit * 0.35, 0.0),
                    egui::vec2(-unit * 0.3, unit * 0.55),
                )
            } else {
                (
                    egui::vec2(-unit * 0.55, -unit * 0.3),
                    egui::vec2(0.0, unit * 0.35),
                    egui::vec2(unit * 0.55, -unit * 0.3),
                )
            };
            painter.add(egui::Shape::line(
                vec![centre + a, centre + b, centre + c],
                stroke,
            ));
        }
        Icon::Add => {
            painter.line_segment(
                [
                    centre - egui::vec2(unit * 0.6, 0.0),
                    centre + egui::vec2(unit * 0.6, 0.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    centre - egui::vec2(0.0, unit * 0.6),
                    centre + egui::vec2(0.0, unit * 0.6),
                ],
                stroke,
            );
        }
        Icon::Remove => {
            painter.line_segment(
                [
                    centre - egui::vec2(unit * 0.6, 0.0),
                    centre + egui::vec2(unit * 0.6, 0.0),
                ],
                stroke,
            );
        }
    }
}

/// Allocates space and draws an icon, returning the response.
pub fn button(ui: &mut egui::Ui, icon: Icon, active: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(size::ICON, size::ICON), egui::Sense::click());
    // Quiet at rest, brighter on hover: the same rule the rest of the
    // interface follows.
    let tint = if active || response.hovered() {
        Tokens::text()
    } else {
        Tokens::text_dim()
    };
    paint(ui.painter(), rect, icon, tint);
    response.on_hover_text(icon.description())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_can_be_described() {
        for icon in Icon::ALL {
            assert!(
                !icon.description().is_empty(),
                "{icon:?} has no description, so its state is unreadable \
                 to anyone who cannot see the shape"
            );
        }
    }

    #[test]
    fn every_icon_is_described_distinctly() {
        for (i, a) in Icon::ALL.iter().enumerate() {
            for b in Icon::ALL.iter().skip(i + 1) {
                assert_ne!(
                    a.description(),
                    b.description(),
                    "{a:?} and {b:?} say the same thing"
                );
            }
        }
    }

    #[allow(clippy::assertions_on_constants)] // see design.rs
    #[test]
    fn the_set_shares_one_stroke_weight() {
        // The weight is a constant rather than a per-icon choice, which is
        // what makes "one set" checkable rather than a hope.
        assert!(STROKE > 0.0);
        assert!(
            STROKE < 2.0,
            "a heavier stroke than this stops reading as a line-based set"
        );
    }

    #[test]
    fn visibility_states_are_a_pair() {
        // Shown and hidden must be tellable apart at a glance, so they are the
        // same ring with and without a centre rather than two unrelated marks.
        assert_ne!(Icon::Visible.description(), Icon::Hidden.description());
    }
}
