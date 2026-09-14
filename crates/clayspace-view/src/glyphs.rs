//! A mark on every brush swatch saying what the brush does.
//!
//! The shelf held twenty identical grey spheres, told apart by the word under
//! each. ZBrush's shelf is read by shape first and word second — a sculptor
//! reaching for Inflate does not read "Inflate", they see the swollen ball —
//! and a row of look-alikes is a row the eye has to read one by one.
//!
//! The marks are drawn rather than shipped, for the reason the icon set is:
//! one pen, one weight, and a glyph that has to be explained is worse than the
//! word it sits over. Each is a picture of the *effect on a surface* — a hump
//! for a stroke that raises one, a plane for a stroke that cuts one, a hatch
//! for a mask — in the same visual language as the manipulator, where an
//! arrow slides, a ring turns and a box scales.
//!
//! Every glyph is a pure function of the rectangle it is drawn in, returning
//! shapes rather than painting them. That is what lets a test say each brush
//! has a distinct mark and that none of them leaves its swatch, without a GPU.

use clayspace_model::ToolKind;
use egui::{Color32, Pos2, Rect, Shape, Stroke};

/// The weight every glyph is drawn at.
///
/// Heavier than the icon set's 1.25: the icons sit in a 16-pixel box and the
/// glyphs on a 54-pixel shaded sphere, and a hairline over a shaded form is a
/// thing to squint for. One constant, so the set stays a set.
pub const STROKE: f32 = 1.6;

/// Draws a brush's mark centred in `rect`, in `ink`.
pub fn paint(painter: &egui::Painter, rect: Rect, tool: ToolKind, ink: Color32) {
    painter.extend(tool_glyph(tool, rect, ink));
}

/// The mark for a brush, as shapes in `rect`.
///
/// `ink` is passed rather than chosen so a caller can dim an inactive swatch
/// without this module knowing about interaction states.
pub fn tool_glyph(tool: ToolKind, rect: Rect, ink: Color32) -> Vec<Shape> {
    let pen = Pen::new(rect, ink);
    match tool {
        ToolKind::Padrao => standard(&pen),
        ToolKind::Inflar => inflate(&pen),
        ToolKind::Suavizar => smooth(&pen),
        ToolKind::Mover => move_(&pen),
        ToolKind::MoverTopologico => move_topological(&pen),
        ToolKind::Pincar => pinch(&pen),
        ToolKind::Raspar => scrape(&pen),
        ToolKind::Planar => planar(&pen),
        ToolKind::Preencher => fill(&pen),
        ToolKind::Camada => layer(&pen),
        ToolKind::Mascara => mask(&pen),
        ToolKind::Puxar => snake_hook(&pen),
        ToolKind::Polir => polish(&pen),
        ToolKind::Relaxar => relax(&pen),
        ToolKind::Nudge => nudge(&pen),
        ToolKind::Trim => trim(&pen),
        ToolKind::Argila => clay(&pen),
        ToolKind::Vinco => crease(&pen),
        ToolKind::Pintar => paint_brush(&pen),
        ToolKind::Borrar => smear(&pen),
        ToolKind::Apagar => erase(&pen),
    }
}

/// One pen for the whole set: a centre, a unit, and a stroke.
///
/// Glyph coordinates are in units of the swatch's sphere radius, so a glyph
/// is written once and lands the same on any swatch size.
struct Pen {
    centre: Pos2,
    unit: f32,
    stroke: Stroke,
    ink: Color32,
}

impl Pen {
    fn new(rect: Rect, ink: Color32) -> Self {
        Self {
            centre: rect.center(),
            // The sphere the shelf paints is 0.42 of the swatch; the glyph is
            // measured against it so it sits on the ball rather than around it.
            unit: rect.width().min(rect.height()) * 0.42,
            stroke: Stroke::new(STROKE, ink),
            ink,
        }
    }

    /// A point, in sphere radii from the centre; y grows downward as on
    /// screen.
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.centre + egui::vec2(x, y) * self.unit
    }

    fn line(&self, points: &[(f32, f32)]) -> Shape {
        Shape::line(
            points.iter().map(|&(x, y)| self.at(x, y)).collect(),
            self.stroke,
        )
    }

    fn segment(&self, from: (f32, f32), to: (f32, f32)) -> Shape {
        Shape::line_segment([self.at(from.0, from.1), self.at(to.0, to.1)], self.stroke)
    }

    /// A curve sampled from a function of `t` in `0..=1`.
    fn curve(&self, samples: usize, f: impl Fn(f32) -> (f32, f32)) -> Shape {
        Shape::line(
            (0..=samples)
                .map(|i| {
                    let (x, y) = f(i as f32 / samples as f32);
                    self.at(x, y)
                })
                .collect(),
            self.stroke,
        )
    }

    fn ring(&self, centre: (f32, f32), radius: f32) -> Shape {
        Shape::circle_stroke(self.at(centre.0, centre.1), radius * self.unit, self.stroke)
    }

    fn disc(&self, centre: (f32, f32), radius: f32) -> Shape {
        Shape::circle_filled(self.at(centre.0, centre.1), radius * self.unit, self.ink)
    }

    /// A shaft with an open head, which reads as a direction at any size.
    fn arrow(&self, from: (f32, f32), to: (f32, f32), out: &mut Vec<Shape>) {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (ux, uy) = (dx / length, dy / length);
        let head = 0.16;
        out.push(self.segment(from, to));
        for side in [-1.0, 1.0] {
            let (px, py) = (-uy * side, ux * side);
            out.push(self.segment(
                to,
                (
                    to.0 - ux * head + px * head * 0.6,
                    to.1 - uy * head + py * head * 0.6,
                ),
            ));
        }
    }
}

/// A raised hump on a baseline: what a stroke leaves.
///
/// `lean` shifts the top sideways in proportion to its height, which is the
/// difference between material added and material dragged. `cap` clips the
/// hump at a height, for the tools that plane it off.
fn hump(pen: &Pen, lean: f32, cap: Option<f32>) -> Shape {
    const BASE: f32 = 0.28;
    const HEIGHT: f32 = 0.62;
    pen.curve(28, |t| {
        let x = -0.62 + t * 1.24;
        let rise = HEIGHT * (-(x / 0.27).powi(2)).exp();
        let mut y = BASE - rise;
        if let Some(cap) = cap {
            y = y.max(cap);
        }
        (x + lean * (BASE - y), y)
    })
}

fn standard(pen: &Pen) -> Vec<Shape> {
    vec![hump(pen, 0.0, None)]
}

fn inflate(pen: &Pen) -> Vec<Shape> {
    let mut out = vec![pen.ring((0.0, 0.0), 0.28)];
    for (x, y) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
        pen.arrow((x * 0.36, y * 0.36), (x * 0.66, y * 0.66), &mut out);
    }
    out
}

fn smooth(pen: &Pen) -> Vec<Shape> {
    // Ripples on the left dying out to a flat line on the right.
    vec![pen.curve(40, |t| {
        let x = -0.62 + t * 1.24;
        let amplitude = 0.2 * ((0.2 - x) / 0.7).clamp(0.0, 1.0);
        (x, amplitude * (x * std::f32::consts::TAU / 0.42).sin())
    })]
}

fn move_(pen: &Pen) -> Vec<Shape> {
    // A hump pulled sideways, and the pull.
    let mut out = vec![hump(pen, 0.45, None)];
    pen.arrow((-0.05, -0.62), (0.5, -0.62), &mut out);
    out
}

/// The same hump and pull, over a *path* rather than a straight line.
///
/// What tells the two apart at swatch size is the second mark: an arc under
/// the hump standing for the surface the reach is measured along, where the
/// Euclidean drag has none. The arrow is the same, because the gesture is.
fn move_topological(pen: &Pen) -> Vec<Shape> {
    let mut out = vec![hump(pen, 0.45, None)];
    pen.arrow((-0.05, -0.62), (0.5, -0.62), &mut out);
    // The path the reach runs along: a shallow curve under the form, drawn as
    // three segments rather than a spline, which is all the swatch resolves.
    out.push(pen.segment((-0.72, 0.34), (-0.28, 0.5)));
    out.push(pen.segment((-0.28, 0.5), (0.28, 0.5)));
    out.push(pen.segment((0.28, 0.5), (0.72, 0.34)));
    out
}

fn pinch(pen: &Pen) -> Vec<Shape> {
    let mut out = vec![pen.segment((0.0, -0.4), (0.0, 0.4))];
    pen.arrow((-0.64, 0.0), (-0.14, 0.0), &mut out);
    pen.arrow((0.64, 0.0), (0.14, 0.0), &mut out);
    out
}

fn scrape(pen: &Pen) -> Vec<Shape> {
    // A hump planed off, and the blade that did it.
    vec![
        hump(pen, 0.0, Some(-0.12)),
        pen.segment((0.22, -0.6), (0.64, -0.34)),
        pen.segment((0.22, -0.6), (0.1, -0.72)),
    ]
}

fn planar(pen: &Pen) -> Vec<Shape> {
    // The plane, longer than the hump it flattens.
    vec![
        hump(pen, 0.0, Some(-0.1)),
        pen.segment((-0.64, -0.1), (0.64, -0.1)),
    ]
}

fn fill(pen: &Pen) -> Vec<Shape> {
    // A pocket, filled from the bottom up.
    let mut out = vec![pen.line(&[(-0.55, -0.35), (-0.28, 0.35), (0.28, 0.35), (0.55, -0.35)])];
    for (y, half) in [(0.2, 0.31), (0.05, 0.36), (-0.1, 0.42)] {
        out.push(pen.segment((-half, y), (half, y)));
    }
    out
}

fn layer(pen: &Pen) -> Vec<Shape> {
    // A step of even height: what does not build up on itself.
    vec![
        pen.line(&[
            (-0.64, 0.3),
            (-0.38, 0.3),
            (-0.32, -0.14),
            (0.32, -0.14),
            (0.38, 0.3),
            (0.64, 0.3),
        ]),
        pen.segment((-0.34, 0.08), (0.34, 0.08)),
    ]
}

fn mask(pen: &Pen) -> Vec<Shape> {
    // The frozen half, hatched.
    let mut out = vec![pen.segment((-0.6, 0.0), (0.6, 0.0))];
    for i in 0..5 {
        let x = -0.58 + i as f32 * 0.22;
        let run = (0.58 - x).min(0.34);
        out.push(pen.segment((x, 0.06), (x + run, 0.06 + run)));
    }
    out
}

fn snake_hook(pen: &Pen) -> Vec<Shape> {
    // A tendril pulled out of the surface, hooking over at the tip.
    let (p0, p1, p2, p3) = (
        (-0.35f32, 0.42f32),
        (-0.4f32, -0.55f32),
        (0.3f32, -0.78f32),
        (0.5f32, -0.1f32),
    );
    vec![
        pen.segment((-0.64, 0.42), (0.1, 0.42)),
        pen.curve(20, |t| {
            let s = 1.0 - t;
            let w = [s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t];
            (
                w[0] * p0.0 + w[1] * p1.0 + w[2] * p2.0 + w[3] * p3.0,
                w[0] * p0.1 + w[1] * p1.1 + w[2] * p2.1 + w[3] * p3.1,
            )
        }),
    ]
}

fn polish(pen: &Pen) -> Vec<Shape> {
    // A sparkle: the mark every culture reads as "shiny".
    vec![
        pen.segment((0.0, -0.58), (0.0, 0.58)),
        pen.segment((-0.58, 0.0), (0.58, 0.0)),
        pen.segment((-0.26, -0.26), (0.26, 0.26)),
        pen.segment((-0.26, 0.26), (0.26, -0.26)),
    ]
}

fn relax(pen: &Pen) -> Vec<Shape> {
    // One easy wave, the whole way across: settled, not flattened.
    vec![pen.curve(32, |t| {
        let x = -0.62 + t * 1.24;
        (x, 0.16 * (x * std::f32::consts::TAU / 1.24).sin())
    })]
}

fn nudge(pen: &Pen) -> Vec<Shape> {
    // The skin slides between two lines; the interior stays.
    let mut out = vec![
        pen.segment((-0.6, -0.26), (0.36, -0.26)),
        pen.segment((-0.6, 0.26), (0.36, 0.26)),
    ];
    pen.arrow((-0.3, 0.0), (0.6, 0.0), &mut out);
    out
}

fn trim(pen: &Pen) -> Vec<Shape> {
    use std::f32::consts::PI;
    // A form with a piece cut clean off and set aside.
    let left = pen.curve(20, |t| {
        let a = PI * 0.5 + t * PI;
        (-0.08 + 0.42 * a.cos(), 0.42 * a.sin())
    });
    let cap = pen.curve(12, |t| {
        let a = -PI * 0.5 + t * PI;
        (0.24 + 0.42 * a.cos(), 0.42 * a.sin())
    });
    let mut out = vec![
        left,
        pen.segment((-0.08, -0.42), (-0.08, 0.42)),
        cap,
        pen.segment((0.24, -0.42), (0.24, 0.42)),
    ];
    // The cutting line, dashed, running past the form.
    for i in 0..4 {
        let y = -0.66 + i as f32 * 0.4;
        out.push(pen.segment((0.08, y), (0.08, y + 0.14)));
    }
    out
}

fn clay(pen: &Pen) -> Vec<Shape> {
    // Lumps laid on one another, the way clay is added by hand.
    vec![
        pen.ring((-0.24, 0.16), 0.28),
        pen.ring((0.24, 0.16), 0.28),
        pen.ring((0.0, -0.22), 0.28),
    ]
}

fn crease(pen: &Pen) -> Vec<Shape> {
    // A sharp trough in an otherwise level surface.
    vec![pen.line(&[
        (-0.64, -0.12),
        (-0.16, -0.12),
        (0.0, 0.4),
        (0.16, -0.12),
        (0.64, -0.12),
    ])]
}

fn paint_brush(pen: &Pen) -> Vec<Shape> {
    // A drop of colour.
    vec![
        // The round of the drop, from one tangent point the long way round
        // to the other; the two straight sides meet at the apex above.
        pen.curve(20, |t| {
            let a = -std::f32::consts::FRAC_PI_4 + t * std::f32::consts::PI * 1.5;
            (0.3 * a.cos(), 0.14 + 0.3 * a.sin())
        }),
        pen.segment((-0.21, -0.07), (0.0, -0.56)),
        pen.segment((0.21, -0.07), (0.0, -0.56)),
    ]
}

fn smear(pen: &Pen) -> Vec<Shape> {
    // Colour dragged along, thinning as it goes.
    vec![
        pen.disc((-0.3, 0.0), 0.15),
        pen.segment((-0.16, -0.1), (0.5, -0.26)),
        pen.segment((-0.14, 0.0), (0.6, 0.0)),
        pen.segment((-0.16, 0.1), (0.5, 0.26)),
    ]
}

fn erase(pen: &Pen) -> Vec<Shape> {
    // An eraser on the slant, with the band that every eraser has.
    let angle: f32 = -0.6;
    let (c, s) = (angle.cos(), angle.sin());
    let turn = |x: f32, y: f32| (x * c - y * s, x * s + y * c);
    let corners = [
        turn(-0.5, -0.2),
        turn(0.5, -0.2),
        turn(0.5, 0.2),
        turn(-0.5, 0.2),
        turn(-0.5, -0.2),
    ];
    vec![
        pen.line(&corners),
        pen.segment(turn(-0.15, -0.2), turn(-0.15, 0.2)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swatch() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 100.0), egui::vec2(54.0, 54.0))
    }

    /// An ink to draw the tests with. A token rather than a literal, because
    /// `design::no_literal_colors` reads this file too.
    fn ink() -> Color32 {
        crate::design::Tokens::text()
    }

    #[test]
    fn every_brush_has_a_mark() {
        for tool in ToolKind::ALL {
            assert!(
                !tool_glyph(tool, swatch(), ink()).is_empty(),
                "{tool:?} draws nothing, so its swatch is a grey ball like the rest"
            );
        }
    }

    #[test]
    fn every_mark_is_its_own() {
        // A shelf where two brushes share a picture is a shelf that lies about
        // one of them.
        let pictures: Vec<String> = ToolKind::ALL
            .iter()
            .map(|tool| format!("{:?}", tool_glyph(*tool, swatch(), ink())))
            .collect();
        for (i, a) in ToolKind::ALL.iter().enumerate() {
            for (j, b) in ToolKind::ALL.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    pictures[i], pictures[j],
                    "{a:?} and {b:?} draw the same mark"
                );
            }
        }
    }

    #[test]
    fn no_mark_leaves_its_swatch() {
        let rect = swatch();
        for tool in ToolKind::ALL {
            for shape in tool_glyph(tool, rect, ink()) {
                let bounds = shape.visual_bounding_rect();
                assert!(
                    rect.expand(1.0).contains_rect(bounds),
                    "{tool:?} draws outside its swatch: {bounds:?} against {rect:?}"
                );
            }
        }
    }

    #[test]
    fn every_mark_is_drawn_in_the_ink_it_was_given() {
        // The ink is the caller's choice, so an inactive swatch can be dimmed
        // from outside; a glyph that picked its own colour would defeat that.
        let ink = crate::design::Tokens::ground();
        for tool in ToolKind::ALL {
            for shape in tool_glyph(tool, swatch(), ink) {
                let seen = match &shape {
                    Shape::LineSegment { stroke, .. } => stroke.color,
                    Shape::Path(path) => match &path.stroke.color {
                        egui::epaint::ColorMode::Solid(color) => *color,
                        other => panic!("{tool:?} draws a gradient: {other:?}"),
                    },
                    Shape::Circle(circle) => {
                        if circle.fill.a() == 0 {
                            circle.stroke.color
                        } else {
                            circle.fill
                        }
                    }
                    other => panic!("{tool:?} draws a shape the set does not use: {other:?}"),
                };
                assert_eq!(seen, ink, "{tool:?} chose its own colour");
            }
        }
    }

    #[allow(clippy::assertions_on_constants)] // see design.rs
    #[test]
    fn the_set_shares_one_stroke_weight() {
        assert!(STROKE >= crate::icons::STROKE);
        assert!(
            STROKE < 2.5,
            "heavier than this and the marks stop reading as line work"
        );
    }
}
