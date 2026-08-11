//! The viewport camera.
//!
//! An orbit camera around a target, with the four view presets the design
//! calls for. Switching to an orthogonal preset switches the projection too,
//! and preserves framing rather than resetting the distance — a sculptor
//! comparing front and side wants the same subject size in both.

use glam::{Mat4, Vec3};

/// Which standard view is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewPreset {
    #[default]
    Perspective,
    Front,
    Side,
    Top,
}

impl ViewPreset {
    /// Every preset, in the order the interface presents them.
    pub const ALL: [ViewPreset; 4] = [Self::Perspective, Self::Front, Self::Side, Self::Top];

    /// The label shown in the viewport bar.
    pub fn label(self) -> &'static str {
        match self {
            Self::Perspective => "Perspectiva",
            Self::Front => "Frontal",
            Self::Side => "Lateral",
            Self::Top => "Superior",
        }
    }

    /// Orthogonal presets are drawn without perspective, so that a
    /// measurement taken on screen means the same thing anywhere on it.
    pub fn is_orthographic(self) -> bool {
        !matches!(self, Self::Perspective)
    }

    /// Where the camera sits, as a unit direction from the target.
    fn direction(self) -> Vec3 {
        match self {
            // A three-quarter view: the default a sculptor starts from.
            Self::Perspective => Vec3::new(0.55, 0.35, 1.0).normalize(),
            Self::Front => Vec3::Z,
            Self::Side => Vec3::X,
            Self::Top => Vec3::Y,
        }
    }

    fn up(self) -> Vec3 {
        match self {
            // Looking straight down, Y cannot also be up.
            Self::Top => -Vec3::Z,
            _ => Vec3::Y,
        }
    }
}

/// An orbit camera.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// What the camera orbits.
    pub target: Vec3,
    /// How far it sits from the target.
    pub distance: f32,
    /// Rotation about the world up axis, radians.
    pub yaw: f32,
    /// Rotation above the horizon, radians, clamped short of the poles.
    pub pitch: f32,
    pub preset: ViewPreset,
    /// Vertical field of view for the perspective projection, radians.
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        let mut camera = Self {
            target: Vec3::ZERO,
            distance: 4.0,
            yaw: 0.0,
            pitch: 0.0,
            preset: ViewPreset::Perspective,
            fov_y: 45f32.to_radians(),
            near: 0.01,
            far: 1000.0,
        };
        camera.apply_preset(ViewPreset::Perspective);
        camera
    }
}

impl Camera {
    /// Pitch is clamped just short of vertical: at exactly ±90° the up vector
    /// and the view direction are parallel and the view matrix degenerates.
    const MAX_PITCH: f32 = std::f32::consts::FRAC_PI_2 - 0.001;

    /// Where the camera is in world space.
    pub fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let direction = Vec3::new(cp * sy, sp, cp * cy);
        self.target + direction * self.distance
    }

    /// The up axis for the current preset.
    pub fn up(&self) -> Vec3 {
        self.preset.up()
    }

    pub fn view(&self) -> Mat4 {
        // Deprecated in glam 0.33 in favour of a free function, but the
        // semantics are identical and the constructor is clearer here.
        #[allow(deprecated)]
        Mat4::look_at_rh(self.eye(), self.target, self.up())
    }

    /// The rotation part of the view matrix, for taking normals into view
    /// space without also translating them.
    pub fn view_rotation(&self) -> Mat4 {
        let mut view = self.view();
        view.w_axis = glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
        view
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        if self.preset.is_orthographic() {
            // Half-height is derived from the distance so that switching
            // projection keeps the subject the same size on screen.
            let half_height = self.distance * (self.fov_y * 0.5).tan();
            let half_width = half_height * aspect;
            #[allow(deprecated)]
            Mat4::orthographic_rh(
                -half_width,
                half_width,
                -half_height,
                half_height,
                -self.far,
                self.far,
            )
        } else {
            {
                #[allow(deprecated)]
                Mat4::perspective_rh(self.fov_y, aspect, self.near, self.far)
            }
        }
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }

    /// Orbits by a pointer delta in radians.
    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw -= delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
        // Orbiting leaves any orthogonal preset: the view is no longer the one
        // the preset names, and pretending otherwise would mislabel it.
        if self.preset.is_orthographic() {
            self.preset = ViewPreset::Perspective;
        }
    }

    /// Slides the target across the view plane.
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let view = self.view();
        let right = Vec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
        let up = Vec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
        // Scaling by distance keeps a drag moving the same amount of *screen*
        // whatever the zoom.
        let scale = self.distance * 0.002;
        self.target += (-right * delta_x + up * delta_y) * scale;
    }

    /// Zooms multiplicatively, so each notch feels the same at any distance.
    pub fn zoom(&mut self, amount: f32) {
        self.distance = (self.distance * (1.0 - amount * 0.1)).clamp(0.01, 10_000.0);
    }

    /// Points the camera along a preset's axis, keeping the current framing.
    pub fn apply_preset(&mut self, preset: ViewPreset) {
        self.preset = preset;
        let direction = preset.direction();
        self.pitch = direction.y.asin().clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
        self.yaw = direction.x.atan2(direction.z);
    }

    /// Frames an axis-aligned box.
    ///
    /// An empty or degenerate box yields the defined default view rather than
    /// a division by zero or a camera inside the subject.
    pub fn frame_bounds(&mut self, min: Vec3, max: Vec3) {
        let size = max - min;
        if !size.is_finite() || size.max_element() <= 0.0 {
            self.target = Vec3::ZERO;
            self.distance = 4.0;
            return;
        }
        self.target = (min + max) * 0.5;
        // The bounding sphere's radius, then far enough back that it fits the
        // vertical field of view with a margin.
        let radius = size.length() * 0.5;
        self.distance = (radius / (self.fov_y * 0.5).sin()).max(radius * 1.5) * 1.1;
    }

    /// The world-space ray through a point on screen.
    ///
    /// `ndc` is in -1..=1 with y up, which is what the viewport hands over
    /// after converting from pixels. This is how a pointer becomes something
    /// the engine can pick with.
    pub fn ray_through(&self, ndc: [f32; 2], aspect: f32) -> ([f32; 3], [f32; 3]) {
        let eye = self.eye();
        let forward = (self.target - eye).normalize_or_zero();
        let right = forward.cross(self.up()).normalize_or_zero();
        let up = right.cross(forward);

        if self.preset.is_orthographic() {
            // Parallel rays offset across the view plane, rather than a fan
            // from a point: an orthographic pick must not converge.
            let half_height = self.distance * (self.fov_y * 0.5).tan();
            let origin =
                eye + right * (ndc[0] * half_height * aspect) + up * (ndc[1] * half_height);
            (origin.into(), forward.into())
        } else {
            let tan = (self.fov_y * 0.5).tan();
            let direction =
                (forward + right * (ndc[0] * tan * aspect) + up * (ndc[1] * tan)).normalize_or_zero();
            (eye.into(), direction.into())
        }
    }

    /// The framing an empty document gets.
    pub fn frame_default(&mut self) {
        self.target = Vec3::ZERO;
        self.distance = 4.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ray must come back through the pixel that produced it.
    ///
    /// This is the invariant that ties picking to what is on screen. It was
    /// broken in the shipped binary in a way no single-component test could
    /// see: the ray was built from the viewport rectangle the panels left,
    /// while the scene was drawn across the whole window with the window's
    /// aspect. Both were internally consistent; together they disagreed, and
    /// the brush landed off to one side of the pointer.
    fn round_trip(preset: ViewPreset, aspect: f32, point: Vec3) -> f32 {
        let mut camera = Camera::default();
        camera.apply_preset(preset);
        camera.target = Vec3::ZERO;
        camera.distance = 4.0;

        let clip = camera.view_projection(aspect) * point.extend(1.0);
        let ndc = [clip.x / clip.w, clip.y / clip.w];

        let (origin, direction) = camera.ray_through(ndc, aspect);
        let (origin, direction) = (Vec3::from(origin), Vec3::from(direction));

        // Distance from the point to the ray: the projection of the offset
        // that is perpendicular to the direction.
        let offset = point - origin;
        (offset - direction * offset.dot(direction)).length()
    }

    #[test]
    fn a_ray_returns_through_the_pixel_it_came_from() {
        for preset in [
            ViewPreset::Perspective,
            ViewPreset::Front,
            ViewPreset::Side,
            ViewPreset::Top,
        ] {
            // Several aspects, because the defect was an aspect mismatch and a
            // square viewport would have hidden it.
            for aspect in [1.0, 1.265, 1.6, 2.4] {
                for point in [
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(0.6, 0.4, 0.2),
                    Vec3::new(-0.9, 0.3, -0.5),
                ] {
                    let miss = round_trip(preset, aspect, point);
                    assert!(
                        miss < 1e-3,
                        "{preset:?} at aspect {aspect} missed {point} by {miss}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_wrong_aspect_misses() {
        // Proof the test above can fail: build the ray with the window's
        // aspect where the pixel was projected with the viewport's, which is
        // exactly what the binary did.
        let mut camera = Camera::default();
        camera.target = Vec3::ZERO;
        camera.distance = 4.0;
        let point = Vec3::new(0.6, 0.4, 0.2);

        let clip = camera.view_projection(1.265) * point.extend(1.0);
        let ndc = [clip.x / clip.w, clip.y / clip.w];
        let (origin, direction) = camera.ray_through(ndc, 1.6);
        let (origin, direction) = (Vec3::from(origin), Vec3::from(direction));

        let offset = point - origin;
        let miss = (offset - direction * offset.dot(direction)).length();
        assert!(
            miss > 0.05,
            "a mismatched aspect missed by only {miss}, so the round trip              above proves nothing"
        );
    }


    #[test]
    fn pitch_never_reaches_the_pole() {
        let mut camera = Camera::default();
        for _ in 0..100 {
            camera.orbit(0.0, 1.0);
        }
        assert!(camera.pitch < std::f32::consts::FRAC_PI_2);
        assert!(camera.view().is_finite(), "the view matrix degenerated at the pole");
    }

    #[test]
    fn framing_an_empty_box_gives_the_default_view() {
        let mut camera = Camera::default();
        camera.frame_bounds(Vec3::ZERO, Vec3::ZERO);
        assert_eq!(camera.target, Vec3::ZERO);
        assert!(camera.distance > 0.0, "an empty document must not put the camera at the origin");
    }

    #[test]
    fn a_preset_keeps_the_framing() {
        let mut camera = Camera::default();
        camera.frame_bounds(Vec3::splat(-2.0), Vec3::splat(2.0));
        let distance = camera.distance;
        let target = camera.target;

        camera.apply_preset(ViewPreset::Front);

        assert_eq!(camera.distance, distance, "switching preset must not rezoom");
        assert_eq!(camera.target, target, "switching preset must not recentre");
    }

    #[test]
    fn orthogonal_presets_use_an_orthographic_projection() {
        let mut camera = Camera::default();
        assert!(!camera.preset.is_orthographic());

        for preset in [ViewPreset::Front, ViewPreset::Side, ViewPreset::Top] {
            camera.apply_preset(preset);
            assert!(preset.is_orthographic(), "{preset:?} should be orthographic");
            // An orthographic projection has no perspective divide, so the
            // bottom-right element stays 1.
            let projection = camera.projection(1.5);
            assert_eq!(projection.w_axis.w, 1.0, "{preset:?} kept a perspective divide");
        }
    }

    #[test]
    fn orbiting_leaves_an_orthogonal_preset() {
        let mut camera = Camera::default();
        camera.apply_preset(ViewPreset::Front);
        camera.orbit(0.3, 0.1);
        assert_eq!(
            camera.preset,
            ViewPreset::Perspective,
            "orbiting away from Front must stop claiming to be Front"
        );
    }

    #[test]
    fn a_ray_through_the_centre_looks_at_the_target() {
        let camera = Camera::default();
        let (origin, direction) = camera.ray_through([0.0, 0.0], 1.5);
        let origin = Vec3::from(origin);
        let direction = Vec3::from(direction);

        // The centre ray must reach the target.
        let toward_target = (camera.target - origin).normalize();
        assert!(
            direction.dot(toward_target) > 0.999,
            "the centre ray does not point at what the camera is looking at"
        );
    }

    #[test]
    fn rays_fan_out_under_perspective_and_stay_parallel_under_orthographic() {
        let mut camera = Camera::default();
        let (_, left) = camera.ray_through([-0.8, 0.0], 1.5);
        let (_, right) = camera.ray_through([0.8, 0.0], 1.5);
        assert!(
            Vec3::from(left).dot(Vec3::from(right)) < 0.999,
            "perspective rays did not diverge"
        );

        camera.apply_preset(ViewPreset::Front);
        let (origin_a, dir_a) = camera.ray_through([-0.8, 0.0], 1.5);
        let (origin_b, dir_b) = camera.ray_through([0.8, 0.0], 1.5);
        assert!(
            Vec3::from(dir_a).dot(Vec3::from(dir_b)) > 0.999,
            "orthographic rays converged, so a pick would land in the wrong place"
        );
        assert_ne!(origin_a, origin_b, "orthographic rays share an origin");
    }

    #[test]
    fn zoom_is_multiplicative_and_bounded() {
        let mut camera = Camera::default();
        for _ in 0..500 {
            camera.zoom(1.0);
        }
        assert!(camera.distance > 0.0, "zooming in must not reach or pass the target");

        for _ in 0..500 {
            camera.zoom(-1.0);
        }
        assert!(camera.distance.is_finite(), "zooming out must stay finite");
    }
}
