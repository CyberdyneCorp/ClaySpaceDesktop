//! Seeing a layer's UVs on the layer itself.
//!
//! A retopology asked for UVs leaves a layout on the layer it places, and the
//! report beside it says how many charts there are and how much of the square
//! they cover. Neither says whether the layout is any *good* on the form: a
//! chart stretched across a cheek, a seam running down the middle of a face.
//! That is read off the surface, which is what a checker is for — equal squares
//! in UV space that come out unequal on the mesh wherever the layout stretches.
//!
//! So this is a *display* setting, like the voxel display beside it. Nothing
//! here changes a vertex or a UV; it only says how the viewport draws them.

use std::collections::HashMap;

use crate::LayerKey;

/// How a layer carrying UVs is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UvDisplay {
    /// The layer's own material, as every other layer is drawn.
    ///
    /// The default: a sculptor judging form wants the clay, and a checker
    /// over it reads as a pattern on the surface rather than as the surface.
    #[default]
    Off,
    /// A checker in UV space over the material, with the seams drawn over it.
    ///
    /// Squares that stay square are a layout that does not stretch; squares
    /// that shear or grow are where it does.
    Checker,
    /// The same checker, each island tinted a colour of its own, with the
    /// seams drawn over it — which chart a region was laid out in.
    Islands,
}

impl UvDisplay {
    pub const ALL: [UvDisplay; 3] = [Self::Off, Self::Checker, Self::Islands];

    /// Whether the layout is drawn at all.
    pub fn is_on(self) -> bool {
        self != Self::Off
    }
}

/// One mesh layer with its UVs, placed in the world as the viewport draws it.
///
/// One UV per vertex, which is how a mesh layer stores them: a seam is a
/// position duplicated into one vertex per chart that meets there — see
/// [`crate::split_at_uv_seams`].
#[derive(Debug, Clone, PartialEq)]
pub struct UvPreview {
    pub layer: LayerKey,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// Three vertex indices a triangle.
    pub indices: Vec<u32>,
}

/// The islands and seams a per-vertex layout implies.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UvIslands {
    /// For each vertex, which island it belongs to, numbered from zero in
    /// order of first appearance.
    pub island: Vec<u32>,
    /// How many islands there are.
    pub count: usize,
    /// The seams, as vertex pairs — one per seam edge, on one of its sides.
    ///
    /// One side is what the surface needs: both sides stand at the same
    /// positions, so drawing the second would draw the same line again.
    pub seams: Vec<[u32; 2]>,
    /// Every side of every seam, as vertex pairs.
    ///
    /// What the UV square needs: there the two sides of a seam are two
    /// different edges, one on each island's outline, and highlighting only
    /// one would leave the other island's cut looking like its open border.
    pub seam_sides: Vec<[u32; 2]>,
}

/// A layout as the UV square draws it: the charts laid flat, not the form.
///
/// The same per-vertex table the checker reads, with the islands and seams
/// found once, so the square and the surface cannot disagree about which
/// chart a triangle is in or where a cut runs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UvLayout {
    pub uvs: Vec<[f32; 2]>,
    /// Three vertex indices a triangle, into `uvs`.
    pub indices: Vec<u32>,
    pub islands: UvIslands,
}

impl UvLayout {
    /// The layout a preview carries, with its islands and seams.
    pub fn of(preview: &UvPreview) -> Self {
        Self {
            uvs: preview.uvs.clone(),
            indices: preview.indices.clone(),
            islands: uv_islands(&preview.positions, &preview.indices),
        }
    }

    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Finds the islands and seams of a mesh whose UVs are one per vertex.
///
/// **An island** is a set of triangles joined through shared vertices. Since a
/// seam is where one position became two vertices, two charts share no vertex
/// and fall apart here without the UVs being read at all.
///
/// **A seam** is an edge only one triangle uses whose two positions are also
/// the two positions of another such edge — the same edge of the surface, on
/// two copies of its vertices. An edge used once with no twin is the mesh's own
/// open border, which is not a seam and is not drawn as one.
///
/// Positions are compared bit for bit, which is what the split writes: every
/// copy of a seam vertex carries the input position unchanged, and placing the
/// layer in the world moves every copy by the same arithmetic. A triangle
/// naming a vertex that does not exist is skipped rather than trusted.
pub fn uv_islands(positions: &[[f32; 3]], indices: &[u32]) -> UvIslands {
    let triangles: Vec<[u32; 3]> = indices
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .filter(|t| t.iter().all(|&v| (v as usize) < positions.len()))
        .collect();
    let (island, count) = islands(positions.len(), &triangles);
    let (seams, seam_sides) = seams(positions, &triangles);
    UvIslands {
        island,
        count,
        seams,
        seam_sides,
    }
}

/// Union-find over the triangles, then the roots renumbered densely.
fn islands(vertices: usize, triangles: &[[u32; 3]]) -> (Vec<u32>, usize) {
    let mut parent: Vec<u32> = (0..vertices as u32).collect();
    for triangle in triangles {
        union(&mut parent, triangle[0], triangle[1]);
        union(&mut parent, triangle[1], triangle[2]);
    }
    let mut numbered = HashMap::<u32, u32>::new();
    let island = (0..vertices as u32)
        .map(|vertex| {
            let root = find(&mut parent, vertex);
            let next = numbered.len() as u32;
            *numbered.entry(root).or_insert(next)
        })
        .collect();
    (island, numbered.len())
}

fn find(parent: &mut [u32], mut vertex: u32) -> u32 {
    while parent[vertex as usize] != vertex {
        let grand = parent[parent[vertex as usize] as usize];
        parent[vertex as usize] = grand;
        vertex = grand;
    }
    vertex
}

fn union(parent: &mut [u32], a: u32, b: u32) {
    let (a, b) = (find(parent, a), find(parent, b));
    if a != b {
        parent[a.max(b) as usize] = a.min(b);
    }
}

/// The border edges whose positions another border edge shares: one side of
/// each seam, then every side of every seam.
fn seams(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> (Vec<[u32; 2]>, Vec<[u32; 2]>) {
    let mut uses = HashMap::<(u32, u32), u32>::new();
    for triangle in triangles {
        for i in 0..3 {
            let (a, b) = (triangle[i], triangle[(i + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    let mut border: Vec<(u32, u32)> = uses
        .into_iter()
        .filter_map(|(edge, count)| (count == 1).then_some(edge))
        .collect();
    // Sorted so the seam list does not depend on the hash map's order.
    border.sort_unstable();

    let mut sides = HashMap::<[[u32; 3]; 2], Vec<[u32; 2]>>::new();
    for &(a, b) in &border {
        let key = position_pair(positions[a as usize], positions[b as usize]);
        sides.entry(key).or_default().push([a, b]);
    }
    let cut: Vec<Vec<[u32; 2]>> = sides.into_values().filter(|s| s.len() >= 2).collect();
    // Each side list is in `border` order, so its first entry is the lowest.
    let mut seams: Vec<[u32; 2]> = cut.iter().map(|s| s[0]).collect();
    seams.sort_unstable();
    let mut seam_sides: Vec<[u32; 2]> = cut.into_iter().flatten().collect();
    seam_sides.sort_unstable();
    (seams, seam_sides)
}

/// Two positions as an unordered key, bit for bit.
fn position_pair(a: [f32; 3], b: [f32; 3]) -> [[u32; 3]; 2] {
    let bits = |p: [f32; 3]| p.map(f32::to_bits);
    let (a, b) = (bits(a), bits(b));
    if a <= b {
        [a, b]
    } else {
        [b, a]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two quads side by side, split along the edge they share: the left one
    /// is vertices 0..4, the right one 4..8, and 1/2 stand where 4/7 do.
    ///
    /// ```text
    /// 3---2 7---6
    /// |   | |   |
    /// 0---1 4---5
    /// ```
    const SPLIT: [[f32; 3]; 8] = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [2.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ];
    const SPLIT_TRIANGLES: [u32; 12] = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];

    #[test]
    fn the_display_is_off_until_asked_for() {
        assert_eq!(UvDisplay::default(), UvDisplay::Off);
        assert!(!UvDisplay::Off.is_on());
        assert!(UvDisplay::Checker.is_on() && UvDisplay::Islands.is_on());
    }

    #[test]
    fn a_split_strip_is_two_islands_and_one_seam() {
        let found = uv_islands(&SPLIT, &SPLIT_TRIANGLES);
        assert_eq!(found.count, 2);
        assert_eq!(found.island, vec![0, 0, 0, 0, 1, 1, 1, 1]);
        // The shared edge, once, on one of its two sides.
        assert_eq!(found.seams.len(), 1);
        let [a, b] = found.seams[0];
        let ends = [SPLIT[a as usize], SPLIT[b as usize]];
        assert!(ends.contains(&[1.0, 0.0, 0.0]) && ends.contains(&[1.0, 1.0, 0.0]));
        // Both of its sides, one on each island's outline.
        assert_eq!(found.seam_sides, vec![[1, 2], [4, 7]]);
    }

    #[test]
    fn a_layout_carries_the_preview_table_and_its_islands() {
        let preview = UvPreview {
            layer: LayerKey(3),
            positions: SPLIT.to_vec(),
            normals: vec![[0.0, 0.0, 1.0]; 8],
            uvs: (0..8).map(|v| [v as f32 / 8.0, 0.5]).collect(),
            indices: SPLIT_TRIANGLES.to_vec(),
        };
        let layout = UvLayout::of(&preview);
        assert_eq!(layout.uvs, preview.uvs);
        assert_eq!(layout.indices, preview.indices);
        assert_eq!(layout.triangles(), 4);
        assert_eq!(layout.islands, uv_islands(&SPLIT, &SPLIT_TRIANGLES));
    }

    #[test]
    fn an_unsplit_strip_is_one_island_with_no_seam() {
        // The same two quads sharing 1 and 2: one chart, and its outline is
        // the mesh's open border rather than a seam.
        let positions = [SPLIT[0], SPLIT[1], SPLIT[2], SPLIT[3], SPLIT[5], SPLIT[6]];
        let welded = [0, 1, 2, 0, 2, 3, 1, 4, 5, 1, 5, 2];
        let found = uv_islands(&positions, &welded);
        assert_eq!(found.count, 1);
        assert!(found.seams.is_empty(), "{:?}", found.seams);
        assert!(found.seam_sides.is_empty());
    }

    #[test]
    fn a_quad_diagonal_is_never_a_seam() {
        // One quad, fanned: its diagonal is used by both triangles.
        let found = uv_islands(&SPLIT[..4], &SPLIT_TRIANGLES[..6]);
        assert_eq!(found.count, 1);
        assert!(found.seams.is_empty());
    }

    #[test]
    fn a_triangle_past_the_vertices_is_skipped() {
        let mut indices = SPLIT_TRIANGLES.to_vec();
        indices.extend([0, 1, 99]);
        assert_eq!(
            uv_islands(&SPLIT, &indices),
            uv_islands(&SPLIT, &SPLIT_TRIANGLES)
        );
    }

    #[test]
    fn no_triangles_is_no_seam() {
        let found = uv_islands(&SPLIT[..2], &[]);
        assert_eq!(found.island.len(), 2);
        assert!(found.seams.is_empty());
    }
}
