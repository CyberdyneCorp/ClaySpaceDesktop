//! A UV layout, chosen how to be seen and drawn laid flat.
//!
//! Two surfaces offer it: the mesh inspector for an active layer carrying UVs,
//! and the retopology panel for a held preview carrying them. Both draw the
//! same chips and the same square, from the same state, so a display chosen in
//! one is the display the other shows.
//!
//! **The square is presentation only**, like the checker on the surface: it
//! reads the layout and pushes no command, so it can never enter the history
//! or mark the document modified.

use super::*;

use clayspace_model::{UvDisplay, UvLayout};

/// Where one UV display chip was drawn, for a test to press it.
pub fn uv_display_chip_id(display: UvDisplay) -> egui::Id {
    egui::Id::new(("uv-display", display as u8))
}

/// Where the UV square was drawn, for a test to find it.
pub fn uv_layout_square_id() -> egui::Id {
    egui::Id::new("uv-layout-square")
}

/// How the layout is drawn, where there is one, and the square it is laid
/// out in while a display is chosen.
///
/// Nothing at all where there is no layout: three chips that change nothing
/// would say there is something to see.
pub(super) fn uv_display_controls(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    if !state.carries_uvs {
        return;
    }
    let s = state.strings;
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(s.label_uv_display)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    ui.horizontal_wrapped(|ui| {
        for display in UvDisplay::ALL {
            let on = state.uv_display == display;
            let response = ui.add(chip(s.uv_display_name(display), on, Tokens::panel()));
            remember_rect(ui, uv_display_chip_id(display), response.rect);
            if response.clicked() {
                queue.push(Command::SetUvDisplay(display));
            }
        }
    });
    if !state.uv_display.is_on() {
        return;
    }
    ui.label(
        egui::RichText::new(s.hint_uv_display)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    );
    if let Some(layout) = state.uv_layout {
        ui.label(
            egui::RichText::new(s.label_uv_layout)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        uv_square(ui, layout, state.uv_display);
    }
}

fn remember_rect(ui: &egui::Ui, id: egui::Id, rect: egui::Rect) {
    ui.ctx()
        .memory_mut(|memory| memory.data.insert_temp(id, rect));
}

/// The widest the square is drawn. Wide enough to read a seam, narrow enough
/// to leave the panel's other controls on screen.
const SQUARE_SIDE: f32 = 220.0;

/// The UV square: every triangle at its UVs, filled by island under
/// [`UvDisplay::Islands`] and in one neutral tone under the checker, both
/// sides of every seam in the seam colour, and the unit square's outline.
fn uv_square(ui: &mut egui::Ui, layout: &UvLayout, display: UvDisplay) {
    let side = ui.available_width().clamp(80.0, SQUARE_SIDE);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
    remember_rect(ui, uv_layout_square_id(), rect);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Tokens::ground());
    painter.add(egui::Shape::mesh(square_mesh(layout, display, rect)));
    let seam = colour(crate::renderer::SEAM_COLOR);
    for side in &layout.islands.seam_sides {
        let ends = side.map(|v| at(rect, layout.uvs.get(v as usize).copied()));
        if let [Some(a), Some(b)] = ends {
            painter.line_segment([a, b], egui::Stroke::new(2.0, seam));
        }
    }
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, Tokens::text_dim()),
        egui::StrokeKind::Inside,
    );
}

/// The layout's triangles as one mesh in the square's coordinates.
///
/// A triangle naming a vertex the table does not have is left out rather than
/// drawn to a guessed corner.
pub fn square_mesh(layout: &UvLayout, display: UvDisplay, rect: egui::Rect) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    for (vertex, &uv) in layout.uvs.iter().enumerate() {
        let island = layout.islands.island.get(vertex).copied().unwrap_or(0);
        let fill = match display {
            UvDisplay::Islands => colour(crate::renderer::island_tint(island)),
            UvDisplay::Off | UvDisplay::Checker => Tokens::uv_chart(),
        };
        let point = at(rect, Some(uv)).unwrap_or(rect.min);
        mesh.colored_vertex(point, fill);
    }
    let count = layout.uvs.len() as u32;
    for triangle in layout.indices.chunks_exact(3) {
        if triangle.iter().all(|&v| v < count) {
            mesh.add_triangle(triangle[0], triangle[1], triangle[2]);
        }
    }
    mesh
}

/// A UV as a point in the square: `u` to the right, `v` up, as every UV
/// editor draws it.
fn at(rect: egui::Rect, uv: Option<[f32; 2]>) -> Option<egui::Pos2> {
    let [u, v] = uv?;
    Some(egui::pos2(
        rect.min.x + u * rect.width(),
        rect.max.y - v * rect.height(),
    ))
}

/// One of the viewport's own colours — an island tint or the seam colour —
/// as the panel draws it, so the square and the surface agree.
fn colour(rgb: [f32; 3]) -> egui::Color32 {
    let [r, g, b] = rgb.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    // derived from a token: the renderer's island tint or seam colour.
    egui::Color32::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clayspace_model::{uv_islands, LayerKey, UvPreview};

    /// Two quads split along their shared edge, laid side by side in the
    /// square's lower half.
    fn split() -> UvLayout {
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ];
        let indices = vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
        let uvs = vec![
            [0.0, 0.0],
            [0.4, 0.0],
            [0.4, 0.4],
            [0.0, 0.4],
            [0.5, 0.0],
            [0.9, 0.0],
            [0.9, 0.4],
            [0.5, 0.4],
        ];
        let layout = UvLayout::of(&UvPreview {
            layer: LayerKey(1),
            positions: positions.clone(),
            normals: vec![[0.0, 0.0, 1.0]; 8],
            uvs,
            indices: indices.clone(),
        });
        assert_eq!(layout.islands, uv_islands(&positions, &indices));
        layout
    }

    fn square() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(100.0, 100.0))
    }

    #[test]
    fn a_uv_lands_where_a_uv_editor_draws_it() {
        let rect = square();
        assert_eq!(at(rect, Some([0.0, 0.0])), Some(egui::pos2(10.0, 120.0)));
        assert_eq!(at(rect, Some([1.0, 1.0])), Some(egui::pos2(110.0, 20.0)));
        assert_eq!(at(rect, None), None);
    }

    #[test]
    fn every_triangle_is_drawn_at_its_uvs() {
        let layout = split();
        let mesh = square_mesh(&layout, UvDisplay::Checker, square());
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(mesh.indices, layout.indices);
        assert_eq!(mesh.vertices[5].pos, egui::pos2(100.0, 120.0));
        assert!(mesh.vertices.iter().all(|v| v.color == Tokens::uv_chart()));
    }

    #[test]
    fn islands_are_filled_apart() {
        let mesh = square_mesh(&split(), UvDisplay::Islands, square());
        let (left, right) = (mesh.vertices[0].color, mesh.vertices[4].color);
        assert_ne!(left, right);
        assert!(mesh.vertices[..4].iter().all(|v| v.color == left));
        assert!(mesh.vertices[4..].iter().all(|v| v.color == right));
    }

    #[test]
    fn a_triangle_past_the_table_is_left_out() {
        let mut layout = split();
        layout.indices.extend([0, 1, 42]);
        let mesh = square_mesh(&layout, UvDisplay::Checker, square());
        assert_eq!(mesh.indices.len(), 12);
    }
}
