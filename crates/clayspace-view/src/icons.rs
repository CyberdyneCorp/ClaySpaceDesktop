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
    /// The manipulator's three modes: what a drag on it does. The same
    /// shapes the viewport draws — an arrow slides, a ring turns, a box
    /// scales — so the chip and the handle say the same thing.
    Move,
    Turn,
    Scale,
    /// The three booleans, as the two discs every textbook draws them with:
    /// the outline of both, the lens where they overlap, and the crescent one
    /// leaves when the other is taken from it.
    Union,
    Subtract,
    Intersect,
    /// The two deformations: a section that narrows along an axis, and one
    /// that turns along it.
    Taper,
    Twist,
    /// The tool rail's buttons: what the pointer does, what the view shows,
    /// which panels are up, and the two steps of history.
    MaskPaint,
    Frame,
    Polyframe,
    Shapes,
    Cage,
    Reference,
    Curve,
    Undo,
    Redo,
    /// The three representations, told apart by shape and never by hue: a
    /// field is nested contours around a form, a grid is cells, and a mesh is
    /// triangles. Drawn because the representation bar states each one with an
    /// icon *and* its name — the design's rule that a discriminator is never
    /// colour alone applies hardest here, where the three are equals.
    FieldRepresentation,
    VoxelRepresentation,
    MeshRepresentation,
}

impl Icon {
    pub const ALL: [Icon; 28] = [
        Self::Visible,
        Self::Hidden,
        Self::Locked,
        Self::Ghost,
        Self::Collapsed,
        Self::Expanded,
        Self::Add,
        Self::Remove,
        Self::Move,
        Self::Turn,
        Self::Scale,
        Self::Union,
        Self::Subtract,
        Self::Intersect,
        Self::Taper,
        Self::Twist,
        Self::MaskPaint,
        Self::Frame,
        Self::Polyframe,
        Self::Shapes,
        Self::Cage,
        Self::Reference,
        Self::Curve,
        Self::Undo,
        Self::Redo,
        Self::FieldRepresentation,
        Self::VoxelRepresentation,
        Self::MeshRepresentation,
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
            Self::Move => "mover",
            Self::Turn => "girar",
            Self::Scale => "escalar",
            Self::Union => "unir",
            Self::Subtract => "subtrair",
            Self::Intersect => "interseção",
            Self::Taper => "afunilar",
            Self::Twist => "torcer",
            Self::MaskPaint => "pintar máscara",
            Self::Frame => "enquadrar",
            Self::Polyframe => "malha",
            Self::Shapes => "formas",
            Self::Cage => "gaiola",
            Self::Reference => "referência",
            Self::Curve => "curva",
            Self::Undo => "desfazer",
            Self::Redo => "refazer",
            Self::FieldRepresentation => "campo de distância",
            Self::VoxelRepresentation => "grade de voxels",
            Self::MeshRepresentation => "malha de triângulos",
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
        Icon::Move => {
            // Four arrows from the centre: free in every direction.
            for direction in [
                egui::vec2(1.0, 0.0),
                egui::vec2(-1.0, 0.0),
                egui::vec2(0.0, 1.0),
                egui::vec2(0.0, -1.0),
            ] {
                arrow(
                    painter,
                    centre,
                    centre + direction * unit * 0.85,
                    unit * 0.3,
                    stroke,
                );
            }
        }
        Icon::Turn => {
            // Most of a ring, with an arrowhead where it ends.
            let radius = unit * 0.78;
            let start = 0.7 * std::f32::consts::PI;
            let sweep = 1.6 * std::f32::consts::PI;
            let points: Vec<egui::Pos2> = (0..=24)
                .map(|step| {
                    let a = start + sweep * step as f32 / 24.0;
                    centre + egui::vec2(a.cos(), a.sin()) * radius
                })
                .collect();
            let end = points[24];
            let before = points[22];
            painter.add(egui::Shape::line(points, stroke));
            arrow(painter, before, end, unit * 0.5, stroke);
        }
        Icon::Scale => {
            // A small box, and the pull that would make it a big one.
            let box_side = unit * 0.5;
            let corner = centre + egui::vec2(-unit * 0.75, unit * 0.25);
            painter.rect_stroke(
                egui::Rect::from_min_size(corner, egui::vec2(box_side, box_side)),
                egui::epaint::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Middle,
            );
            arrow(
                painter,
                corner + egui::vec2(box_side, 0.0),
                centre + egui::vec2(unit * 0.8, -unit * 0.8),
                unit * 0.32,
                stroke,
            );
        }
        Icon::Union | Icon::Subtract | Icon::Intersect => {
            boolean(painter, centre, unit, icon, stroke)
        }
        Icon::Taper => {
            painter.add(egui::Shape::closed_line(
                vec![
                    centre + egui::vec2(-unit * 0.7, unit * 0.65),
                    centre + egui::vec2(unit * 0.7, unit * 0.65),
                    centre + egui::vec2(unit * 0.25, -unit * 0.65),
                    centre + egui::vec2(-unit * 0.25, -unit * 0.65),
                ],
                stroke,
            ));
        }
        Icon::Twist => {
            // Two edges of a band, crossing as it turns half over.
            for sign in [-1.0f32, 1.0] {
                let points: Vec<egui::Pos2> = (0..=16)
                    .map(|step| {
                        let t = step as f32 / 16.0;
                        let x = sign
                            * (t * std::f32::consts::PI).sin()
                            * unit
                            * 0.6
                            * if t < 0.5 { 1.0 } else { -1.0 };
                        centre + egui::vec2(x, (t - 0.5) * unit * 1.4)
                    })
                    .collect();
                painter.add(egui::Shape::line(points, stroke));
            }
        }
        Icon::MaskPaint => {
            // The shelf's mask mark, boxed: a frozen region, hatched.
            let half = unit * 0.62;
            painter.rect_stroke(
                egui::Rect::from_center_size(centre, egui::vec2(half * 2.0, half * 2.0)),
                egui::epaint::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Middle,
            );
            for i in 0..3 {
                let x = -half + (i as f32 + 0.5) * half * 0.66;
                painter.line_segment(
                    [
                        centre + egui::vec2(x, half),
                        centre + egui::vec2(x + half * 0.9, half * 0.1),
                    ],
                    stroke,
                );
            }
        }
        Icon::Frame => {
            // Four corners, and nothing between them: what framing does.
            let reach = unit * 0.7;
            let arm = unit * 0.36;
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = centre + egui::vec2(sx * reach, sy * reach);
                painter.line_segment([corner, corner - egui::vec2(sx * arm, 0.0)], stroke);
                painter.line_segment([corner, corner - egui::vec2(0.0, sy * arm)], stroke);
            }
        }
        Icon::Polyframe => {
            // A face split in two: the edges a mesh is drawn with.
            let (a, b, c) = (
                centre + egui::vec2(-unit * 0.7, unit * 0.55),
                centre + egui::vec2(unit * 0.7, unit * 0.55),
                centre + egui::vec2(0.0, -unit * 0.65),
            );
            painter.add(egui::Shape::closed_line(vec![a, b, c], stroke));
            painter.line_segment([c, (a + b.to_vec2()) / 2.0], stroke);
        }
        Icon::Shapes => {
            // A cube, seen from a corner: six sides around, three edges in.
            let radius = unit * 0.72;
            let at = |degrees: f32| {
                let a = degrees.to_radians();
                centre + egui::vec2(a.cos(), a.sin()) * radius
            };
            let hexagon: Vec<egui::Pos2> = (0..6).map(|i| at(30.0 + 60.0 * i as f32)).collect();
            painter.add(egui::Shape::closed_line(hexagon, stroke));
            for degrees in [90.0, 210.0, 330.0] {
                painter.line_segment([centre, at(degrees)], stroke);
            }
        }
        Icon::Cage => {
            // A box divided into nine: the cage's control points at the
            // crossings.
            let half = unit * 0.65;
            painter.rect_stroke(
                egui::Rect::from_center_size(centre, egui::vec2(half * 2.0, half * 2.0)),
                egui::epaint::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Middle,
            );
            for third in [-1.0f32, 1.0] {
                let at = third * half / 3.0;
                painter.line_segment(
                    [
                        centre + egui::vec2(at, -half),
                        centre + egui::vec2(at, half),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        centre + egui::vec2(-half, at),
                        centre + egui::vec2(half, at),
                    ],
                    stroke,
                );
            }
        }
        Icon::Reference => {
            // A picture: its frame, and a hill in it.
            let (w, h) = (unit * 0.72, unit * 0.55);
            painter.rect_stroke(
                egui::Rect::from_center_size(centre, egui::vec2(w * 2.0, h * 2.0)),
                egui::epaint::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Middle,
            );
            painter.add(egui::Shape::line(
                vec![
                    centre + egui::vec2(-w * 0.85, h * 0.7),
                    centre + egui::vec2(-w * 0.25, -h * 0.25),
                    centre + egui::vec2(w * 0.1, h * 0.2),
                    centre + egui::vec2(w * 0.4, -h * 0.1),
                    centre + egui::vec2(w * 0.85, h * 0.7),
                ],
                stroke,
            ));
        }
        Icon::Curve => {
            // An S through two points: what is placed, and what is swept.
            let points: Vec<egui::Pos2> = (0..=16)
                .map(|step| {
                    let t = step as f32 / 16.0;
                    let x = (t - 0.5) * unit * 1.3;
                    let y = -(t * std::f32::consts::TAU).sin() * unit * 0.45;
                    centre + egui::vec2(x, y)
                })
                .collect();
            let (first, last) = (points[0], points[16]);
            painter.add(egui::Shape::line(points, stroke));
            painter.circle_filled(first, unit * 0.14, tint);
            painter.circle_filled(last, unit * 0.14, tint);
        }
        Icon::Undo | Icon::Redo => {
            // An arc over the top with its head at the end it goes back to.
            use std::f32::consts::PI;
            let (from, to) = if icon == Icon::Undo {
                (2.0 * PI - 0.25, PI + 0.3)
            } else {
                (PI + 0.25, 2.0 * PI - 0.3)
            };
            let radius = unit * 0.6;
            let middle = centre + egui::vec2(0.0, unit * 0.15);
            let points: Vec<egui::Pos2> = (0..=20)
                .map(|step| {
                    let a = from + (to - from) * step as f32 / 20.0;
                    middle + egui::vec2(a.cos(), a.sin()) * radius
                })
                .collect();
            let (before, end) = (points[18], points[20]);
            painter.add(egui::Shape::line(points, stroke));
            arrow(painter, before, end, unit * 0.45, stroke);
        }
        Icon::FieldRepresentation => {
            // Nested contours: the same form at three distances from its
            // surface, which is what a distance field is a picture of.
            for scale in [1.0f32, 0.66, 0.33] {
                painter.circle_stroke(centre, unit * 0.66 * scale, stroke);
            }
        }
        Icon::VoxelRepresentation => {
            // Four cells. A grid rather than one box, because a single square
            // is what half this set already is.
            let cell = unit * 0.54;
            for (x, y) in [(-1.0f32, -1.0f32), (0.0, -1.0), (-1.0, 0.0), (0.0, 0.0)] {
                let corner = centre + egui::vec2(x * cell, y * cell);
                painter.rect_stroke(
                    egui::Rect::from_min_size(corner, egui::vec2(cell, cell)),
                    egui::epaint::CornerRadius::ZERO,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }
        Icon::MeshRepresentation => {
            // A triangle subdivided by its own edge midpoints — four faces,
            // which is what a mesh is a picture of.
            //
            // Not a triangle with a line dropped from its apex: that is a
            // warning sign, and it read as one in the representation bar
            // beside two icons that are plainly objects.
            let top = centre + egui::vec2(0.0, -unit * 0.66);
            let left = centre + egui::vec2(-unit * 0.72, unit * 0.5);
            let right = centre + egui::vec2(unit * 0.72, unit * 0.5);
            painter.add(egui::Shape::closed_line(vec![top, left, right], stroke));
            let midpoint = |a: egui::Pos2, b: egui::Pos2| a + (b - a) * 0.5;
            painter.add(egui::Shape::closed_line(
                vec![
                    midpoint(top, left),
                    midpoint(left, right),
                    midpoint(right, top),
                ],
                stroke,
            ));
        }
    }
}

/// A shaft with an open head, from `from` to `to`.
fn arrow(
    painter: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    head: f32,
    stroke: egui::Stroke,
) {
    let along = (to - from).normalized();
    let across = egui::vec2(-along.y, along.x);
    painter.line_segment([from, to], stroke);
    for side in [-1.0f32, 1.0] {
        painter.line_segment([to, to - along * head + across * head * side * 0.6], stroke);
    }
}

/// Two discs and what a boolean keeps of them, drawn as the arcs that bound
/// the kept region: both outer arcs for a union, the two inner arcs for an
/// intersection, and one disc's outer arc with the other's inner arc for a
/// subtraction — the crescent.
fn boolean(
    painter: &egui::Painter,
    centre: egui::Pos2,
    unit: f32,
    icon: Icon,
    stroke: egui::Stroke,
) {
    use std::f32::consts::PI;
    // Past the set's shared optical size, to the edge of the box: the lens
    // between two discs is the narrowest mark in the set, and at the shared
    // size it was a smudge — and the difference between the three is the
    // whole point.
    let unit = unit / 0.72 * 0.95;
    let offset = unit * 0.3;
    let radius = unit * 0.66;
    // Where the two circles cross, as an angle from each centre.
    let alpha = (radius.powi(2) - offset.powi(2))
        .max(0.0)
        .sqrt()
        .atan2(offset);
    let arc = |middle: egui::Pos2, from: f32, to: f32| {
        let points: Vec<egui::Pos2> = (0..=20)
            .map(|step| {
                let a = from + (to - from) * step as f32 / 20.0;
                middle + egui::vec2(a.cos(), a.sin()) * radius
            })
            .collect();
        painter.add(egui::Shape::line(points, stroke));
    };
    let left = centre - egui::vec2(offset, 0.0);
    let right = centre + egui::vec2(offset, 0.0);
    // The left disc's far side, the right disc's far side, and each one's
    // near side — the part that lies inside the other.
    let left_outer = || arc(left, alpha, 2.0 * PI - alpha);
    let right_outer = || arc(right, alpha - PI, PI - alpha);
    let left_inner = || arc(left, -alpha, alpha);
    let right_inner = || arc(right, PI - alpha, PI + alpha);
    match icon {
        Icon::Union => {
            left_outer();
            right_outer();
        }
        Icon::Intersect => {
            left_inner();
            right_inner();
        }
        _ => {
            left_outer();
            right_inner();
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
