//! What the camera can see, as six planes.
//!
//! The carried mesh layers arrive as one concatenated buffer with a span per
//! subtool, and the renderer draws one call per span. While a scene holds a
//! handful of subtools that is noise — the comment in the renderer saying so is
//! right, and it is why this did not exist. It stops being noise when a scene
//! holds fifty: every span is a draw, a bind and a full pass over geometry the
//! camera is not pointing at.
//!
//! Culling is done on the CPU against a box per span rather than on the GPU.
//! The test is a few multiplies against six planes, the spans number in the
//! tens, and the alternative — an indirect draw fed by a compute pass — is a
//! great deal of machinery for a scene that has not yet been shown to need it.
//!
//! Nothing here depends on which way depth runs. The planes come from the
//! clip-space inequalities `-w ≤ x,y ≤ w` and `0 ≤ z ≤ w`, which is what wgpu
//! guarantees whichever end of the range the near plane is mapped to.

use glam::{Mat4, Vec3, Vec4};

/// The six planes bounding what a view-projection matrix can see.
///
/// Each is `(a, b, c, d)` with `a·x + b·y + c·z + d ≥ 0` inside. They are not
/// normalised: nothing here needs a distance, only a sign, and normalising six
/// planes per frame to discard the result would be arithmetic for its own sake.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// The planes of a view-projection matrix.
    ///
    /// The Gribb–Hartmann extraction: each clip-space inequality is a linear
    /// combination of the matrix's rows, so the plane that bounds it is that
    /// combination. glam stores columns, so the rows are read across them.
    pub fn from_view_projection(m: Mat4) -> Self {
        let row = |i: usize| Vec4::new(m.x_axis[i], m.y_axis[i], m.z_axis[i], m.w_axis[i]);
        let (x, y, z, w) = (row(0), row(1), row(2), row(3));
        Self {
            planes: [
                w + x, // left:   x ≥ -w
                w - x, // right:  x ≤  w
                w + y, // bottom: y ≥ -w
                w - y, // top:    y ≤  w
                z,     // one depth plane: z ≥ 0
                w - z, // the other:       z ≤ w
            ],
        }
    }

    /// Whether an axis-aligned box is anywhere the camera can see.
    ///
    /// Conservative, and deliberately so: a box that is outside every plane
    /// individually may still be reported as visible when it straddles two of
    /// them diagonally. Drawing something that is not seen costs a draw call;
    /// culling something that is seen costs a hole in the picture, and only one
    /// of those is a bug.
    pub fn intersects(&self, min: Vec3, max: Vec3) -> bool {
        self.planes.iter().all(|plane| {
            // The corner furthest along the plane's normal. If even that is
            // behind the plane, every corner is.
            let corner = Vec3::new(
                if plane.x >= 0.0 { max.x } else { min.x },
                if plane.y >= 0.0 { max.y } else { min.y },
                if plane.z >= 0.0 { max.z } else { min.z },
            );
            plane.x * corner.x + plane.y * corner.y + plane.z * corner.z + plane.w >= 0.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Camera, ViewPreset};

    fn looking_at_the_origin() -> Frustum {
        let camera = Camera::default();
        Frustum::from_view_projection(camera.view_projection(16.0 / 9.0))
    }

    /// What the camera is pointed at is visible. The floor under every other
    /// claim here.
    #[test]
    fn what_is_framed_is_visible() {
        let frustum = looking_at_the_origin();
        assert!(frustum.intersects(Vec3::splat(-1.0), Vec3::splat(1.0)));
        assert!(frustum.intersects(Vec3::splat(-0.01), Vec3::splat(0.01)));
    }

    /// And what is behind the camera is not. The default camera stands at
    /// distance 4 in front of the origin, so a box far along the direction it
    /// came from is behind it.
    #[test]
    fn what_is_behind_the_camera_is_not_visible() {
        let camera = Camera::default();
        let frustum = Frustum::from_view_projection(camera.view_projection(16.0 / 9.0));
        let behind = camera.eye() + (camera.eye() - camera.target).normalize() * 50.0;
        assert!(!frustum.intersects(behind - Vec3::ONE, behind + Vec3::ONE));
    }

    /// And what is far off to one side. The camera's field of view is 45°
    /// vertically, so a box a hundred units sideways at the target's depth
    /// cannot be in it.
    #[test]
    fn what_is_off_to_the_side_is_not_visible() {
        let frustum = looking_at_the_origin();
        for offset in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y] {
            let at = offset * 100.0;
            assert!(
                !frustum.intersects(at - Vec3::ONE, at + Vec3::ONE),
                "a box at {at} was reported visible"
            );
        }
    }

    /// A box straddling the edge of the view is visible, because part of it is.
    #[test]
    fn a_box_crossing_the_edge_is_visible() {
        let frustum = looking_at_the_origin();
        // Wide enough that it reaches well outside the view and still covers
        // the middle of it.
        assert!(frustum.intersects(Vec3::new(-50.0, -0.5, -0.5), Vec3::new(50.0, 0.5, 0.5)));
    }

    /// Beyond the far plane is not visible, and this is the test that would
    /// catch a depth convention read the wrong way round: under a reversed
    /// range the near and far planes swap places in the matrix, and an
    /// extraction that assumed which was which would cull the scene and keep
    /// the void.
    #[test]
    fn the_depth_planes_are_the_right_way_round() {
        let mut camera = Camera::default();
        camera.frame_bounds(Vec3::splat(-1.0), Vec3::splat(1.0));
        let frustum = Frustum::from_view_projection(camera.view_projection(1.0));
        let (near, far) = camera.depth_range();
        let forward = (camera.target - camera.eye()).normalize();

        let just_inside = camera.eye() + forward * (near + far) * 0.5;
        assert!(
            frustum.intersects(
                just_inside - Vec3::splat(0.01),
                just_inside + Vec3::splat(0.01)
            ),
            "a point between the planes was culled"
        );
        let well_past = camera.eye() + forward * far * 4.0;
        assert!(
            !frustum.intersects(well_past - Vec3::splat(0.01), well_past + Vec3::splat(0.01)),
            "a point four times the far plane away was kept"
        );
    }

    /// The orthographic presets have their own projection, and it has to be
    /// culled against too — a preset that culled everything would show an
    /// empty viewport on the second click of Front.
    #[test]
    fn the_orthographic_presets_cull_correctly() {
        for preset in [ViewPreset::Front, ViewPreset::Side, ViewPreset::Top] {
            let mut camera = Camera::default();
            camera.apply_preset(preset);
            camera.frame_bounds(Vec3::splat(-1.0), Vec3::splat(1.0));
            let frustum = Frustum::from_view_projection(camera.view_projection(1.0));
            assert!(
                frustum.intersects(Vec3::splat(-1.0), Vec3::splat(1.0)),
                "{preset:?} culled the form it was framing"
            );
            assert!(
                !frustum.intersects(Vec3::splat(999.0), Vec3::splat(1000.0)),
                "{preset:?} kept a box a thousand units away"
            );
        }
    }
}
