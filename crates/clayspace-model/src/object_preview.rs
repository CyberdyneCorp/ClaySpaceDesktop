//! A placed object's own surface, drawn where a drag has taken it.
//!
//! Moving a placed object in a field refills everything its old and new
//! bounds reach, and the viewport then re-meshes those bricks. On the
//! reference scene that cost one 30–60 ms frame before the manipulator's
//! adaptive deferral took over (#196, D14). The field cannot be moved as a
//! picture — the object is blended into it, and the rest of the layer has to
//! stay where it is — so the drag draws the object alone instead: its own
//! primitive, meshed once in its own frame when the drag begins, and posed on
//! every frame of the drag the way the engine would place it.
//!
//! The field is not touched until the pointer comes up, and then it is written
//! exactly once, through the same edit a drag without a preview would make.
//! What the preview shows is the bare shape, without the blend into its
//! neighbours and without its old image leaving the field: an approximation
//! of where the object is going, not of what the field will be.

use crate::Transform;

/// An object's surface in its own frame, and what places it in the world.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectPreview {
    /// The primitive's surface, with the node transform left out.
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Three vertex indices a triangle.
    pub indices: Vec<u32>,
    /// Where the subtool holding the object stands. The identity for a
    /// subtool that has not been moved.
    pub layer: Transform,
    /// The axes of the subtool's mirror the object is reflected through.
    ///
    /// None set when the layer has no mirror or the object opted out of it.
    /// One reflected image per set axis, as the engine emits them: the modes
    /// add, and no product of two reflections is drawn.
    pub mirror: [bool; 3],
}

/// Triangles in the world, ready to draw.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PosedPreview {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl ObjectPreview {
    /// The object's images with the object standing at `world`.
    ///
    /// `world` is what the manipulator speaks: the object as the world sees
    /// it, with its subtool's placement already composed in. It is taken back
    /// into the subtool's frame the way the document takes it when the drag
    /// is written, so each image stands where the engine would put it.
    pub fn posed(&self, world: Transform) -> PosedPreview {
        let node = self.layer.unplace(&world);
        let reflections =
            std::iter::once(None).chain((0..3).filter(|&axis| self.mirror[axis]).map(Some));
        let mut posed = PosedPreview::default();
        for reflection in reflections {
            self.append_image(&mut posed, &node, reflection);
        }
        posed
    }

    /// One image: node frame, then the reflection, then the subtool's frame.
    ///
    /// A reflection turns every triangle inside out, so its winding is
    /// reversed to keep its front faces facing out.
    fn append_image(&self, posed: &mut PosedPreview, node: &Transform, reflection: Option<usize>) {
        let reflect = |mut v: [f32; 3]| {
            if let Some(axis) = reflection {
                v[axis] = -v[axis];
            }
            v
        };
        let base = posed.positions.len() as u32;
        posed.positions.extend(
            self.positions
                .iter()
                .map(|&p| self.layer.into_world(reflect(node.into_world(p)))),
        );
        posed.normals.extend(self.normals.iter().map(|&n| {
            self.layer
                .normal_into_world(reflect(node.normal_into_world(n)))
        }));
        for triangle in self.indices.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|i| base + i);
            if reflection.is_some() {
                posed.indices.extend([a, c, b]);
            } else {
                posed.indices.extend([a, b, c]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> ObjectPreview {
        ObjectPreview {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
            layer: Transform::default(),
            mirror: [false; 3],
        }
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5)
    }

    #[test]
    fn an_unmirrored_object_is_drawn_once_where_it_is_taken() {
        let world = Transform {
            position: [0.5, -0.25, 2.0],
            rotation_axis: [0.0, 0.0, 1.0],
            rotation_angle: std::f32::consts::FRAC_PI_2,
            scale: [2.0, 1.0, 1.0],
        };
        let posed = triangle().posed(world);
        assert_eq!(posed.indices, vec![0, 1, 2]);
        for (at, &local) in triangle().positions.iter().enumerate() {
            assert!(close(posed.positions[at], world.into_world(local)));
        }
        assert!(close(posed.normals[0], [0.0, 0.0, 1.0]));
    }

    /// The images the engine emits under a mirror: the object and one copy
    /// per set axis, reflected through the subtool's own plane, with the copy
    /// wound the other way so it is not culled from the front.
    #[test]
    fn each_mirror_axis_adds_one_reflected_image_through_the_subtool_plane() {
        let layer = Transform {
            position: [1.0, 0.0, 0.0],
            ..Transform::default()
        };
        let preview = ObjectPreview {
            layer,
            mirror: [true, false, true],
            ..triangle()
        };
        let world = Transform {
            position: [1.5, 0.0, 0.0],
            ..Transform::default()
        };
        let posed = preview.posed(world);
        assert_eq!(posed.positions.len(), 9);
        assert_eq!(posed.indices, vec![0, 1, 2, 3, 5, 4, 6, 8, 7]);
        // The first vertex sits at the object's origin, half a unit along x
        // from the subtool's: its X image is half a unit the other side.
        assert!(close(posed.positions[0], [1.5, 0.0, 0.0]));
        assert!(close(posed.positions[3], [0.5, 0.0, 0.0]));
        assert!(close(posed.positions[4], [-0.5, 0.0, 0.0]));
        // The Z image of a triangle in the z = 0 plane lies on it, facing back.
        assert!(close(posed.positions[7], [2.5, 0.0, 0.0]));
        assert!(close(posed.normals[6], [0.0, 0.0, -1.0]));
    }

    /// A subtool that has been moved places its object's images too, and the
    /// world transform round-trips through it rather than being applied twice.
    #[test]
    fn a_moved_subtool_places_the_object_once() {
        let layer = Transform {
            position: [0.0, 2.0, 0.0],
            rotation_axis: [0.0, 1.0, 0.0],
            rotation_angle: 0.7,
            scale: [1.5; 3],
        };
        let world = layer.place(&Transform {
            position: [0.3, 0.0, 0.0],
            ..Transform::default()
        });
        let preview = ObjectPreview {
            layer,
            ..triangle()
        };
        let posed = preview.posed(world);
        for (at, &local) in triangle().positions.iter().enumerate() {
            assert!(close(posed.positions[at], world.into_world(local)));
        }
    }
}
