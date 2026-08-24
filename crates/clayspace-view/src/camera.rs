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

    /// How close the camera may come to what it is zooming toward.
    ///
    /// A fraction of what it can already see rather than a fixed length, so
    /// the standoff is the same *on screen* whatever scale the sculpt is at: a
    /// fixed one would be a mile on a thumbnail and nothing on a bust.
    const STANDOFF: f32 = 0.06;

    /// How much of the way toward the focus the pivot follows.
    ///
    /// Blender calls this zooming to the mouse position, and it is what makes
    /// a zoom feel aimed rather than merely closer: the point under the
    /// pointer drifts toward the middle as you come in, so the next orbit
    /// turns around what you were looking at. Partial rather than complete —
    /// snapping the pivot onto the surface would swing the view on every
    /// notch.
    const FOLLOW: f32 = 0.25;

    /// Zooms multiplicatively, so each notch feels the same at any distance.
    ///
    /// The plain form, with nothing in front of the camera to stop at. It
    /// still bottoms out, but on an arbitrary floor rather than on the clay —
    /// which is what "zooming goes inside the model" is.
    pub fn zoom(&mut self, amount: f32) {
        self.zoom_toward(amount, None);
    }

    /// Zooms toward a point in front of the camera, stopping short of it.
    ///
    /// `focus` is where the pointer's ray met the surface, in world space.
    /// With one, this is Blender's zoom: the camera comes in until it is a
    /// little way off the clay and then stops, and the pivot follows part of
    /// the way so the next orbit turns around what was under the pointer.
    ///
    /// Without one — the pointer over empty space — it is the plain
    /// multiplicative zoom, because there is nothing there to stop at and
    /// refusing to move would read as a broken wheel.
    pub fn zoom_toward(&mut self, amount: f32, focus: Option<[f32; 3]>) {
        let wanted = (self.distance * (1.0 - amount * 0.1)).clamp(0.01, 10_000.0);
        let Some(focus) = focus.map(Vec3::from) else {
            self.distance = wanted;
            return;
        };
        // Only ever a limit on coming *in*. Pulling back past the surface is
        // ordinary and must not be caught by any of this.
        if wanted >= self.distance {
            self.distance = wanted;
            return;
        }

        // What is left between the camera and the clay, and what must stay
        // left. A fraction of the gap so the standoff is the same on screen at
        // any scale, and never less than a little in front of the near plane —
        // a surface closer than that is clipped away, which looks exactly like
        // having gone through it.
        let gap = (focus - self.eye()).length();
        let keep = (gap * Self::STANDOFF).max(self.near * 2.0);
        let room = (gap - keep).max(0.0);
        self.distance = wanted.max(self.distance - room);

        // The pivot follows part of the way, so the next orbit turns around
        // what was under the pointer.
        self.target += (focus - self.target) * Self::FOLLOW * amount.abs().min(1.0);
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
            let direction = (forward + right * (ndc[0] * tan * aspect) + up * (ndc[1] * tan))
                .normalize_or_zero();
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
        let mut camera = Camera {
            target: Vec3::ZERO,
            distance: 4.0,
            ..Camera::default()
        };
        camera.apply_preset(preset);

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
        let camera = Camera {
            target: Vec3::ZERO,
            distance: 4.0,
            ..Camera::default()
        };
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
        assert!(
            camera.view().is_finite(),
            "the view matrix degenerated at the pole"
        );
    }

    #[test]
    fn framing_an_empty_box_gives_the_default_view() {
        let mut camera = Camera::default();
        camera.frame_bounds(Vec3::ZERO, Vec3::ZERO);
        assert_eq!(camera.target, Vec3::ZERO);
        assert!(
            camera.distance > 0.0,
            "an empty document must not put the camera at the origin"
        );
    }

    #[test]
    fn a_preset_keeps_the_framing() {
        let mut camera = Camera::default();
        camera.frame_bounds(Vec3::splat(-2.0), Vec3::splat(2.0));
        let distance = camera.distance;
        let target = camera.target;

        camera.apply_preset(ViewPreset::Front);

        assert_eq!(
            camera.distance, distance,
            "switching preset must not rezoom"
        );
        assert_eq!(camera.target, target, "switching preset must not recentre");
    }

    #[test]
    fn orthogonal_presets_use_an_orthographic_projection() {
        let mut camera = Camera::default();
        assert!(!camera.preset.is_orthographic());

        for preset in [ViewPreset::Front, ViewPreset::Side, ViewPreset::Top] {
            camera.apply_preset(preset);
            assert!(
                preset.is_orthographic(),
                "{preset:?} should be orthographic"
            );
            // An orthographic projection has no perspective divide, so the
            // bottom-right element stays 1.
            let projection = camera.projection(1.5);
            assert_eq!(
                projection.w_axis.w, 1.0,
                "{preset:?} kept a perspective divide"
            );
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
        assert!(
            camera.distance > 0.0,
            "zooming in must not reach or pass the target"
        );

        for _ in 0..500 {
            camera.zoom(-1.0);
        }
        assert!(camera.distance.is_finite(), "zooming out must stay finite");
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::*;

    /// A camera four units out, looking at the origin, with a surface one unit
    /// in front of the pivot — the ordinary case of zooming at a sculpt.
    fn looking_at_a_sphere() -> (Camera, Vec3) {
        let camera = Camera::default();
        let eye = camera.eye();
        let toward = (camera.target - eye).normalize();
        (camera, eye + toward * (camera.distance - 1.0))
    }

    #[test]
    fn zooming_in_stops_short_of_the_surface() {
        // The reported fault: the camera went inside the model. It can come as
        // close as it likes and never through.
        let (mut camera, surface) = looking_at_a_sphere();
        for _ in 0..200 {
            camera.zoom_toward(1.0, Some(surface.into()));
            let gap = (surface - camera.eye()).length();
            assert!(
                gap > 0.0,
                "the camera reached the surface it was zooming at"
            );
        }
        // And it is still in front of it, on the same side it started.
        let toward = (camera.target - camera.eye()).normalize();
        let ahead = (surface - camera.eye()).dot(toward);
        assert!(
            ahead > 0.0,
            "the surface ended up behind the camera, which is what going \
             through it means"
        );
    }

    #[test]
    fn it_gets_closer_every_notch() {
        // Stopping short must not mean stopping: a wheel that refuses to move
        // reads as broken, and detail work needs the last stretch.
        let (mut camera, surface) = looking_at_a_sphere();
        let start = (surface - camera.eye()).length();
        let mut gap = start;
        for _ in 0..20 {
            camera.zoom_toward(1.0, Some(surface.into()));
            let now = (surface - camera.eye()).length();
            assert!(now < gap, "a notch of zoom brought nothing closer");
            gap = now;
        }
        // Most of the way. The rate is the multiplicative one — a notch is a
        // tenth of what is left — so twenty of them close about six sevenths
        // of the gap, and the standoff is nowhere near binding yet.
        assert!(
            gap < start * 0.2,
            "twenty notches went from {start} to {gap} from the clay"
        );
    }

    #[test]
    fn zooming_out_is_never_held_back() {
        // The standoff limits coming in and nothing else. Pulling away past
        // the surface is ordinary.
        let (mut camera, surface) = looking_at_a_sphere();
        let before = camera.distance;
        camera.zoom_toward(-1.0, Some(surface.into()));
        assert!(
            camera.distance > before,
            "zooming out was caught by the standoff"
        );
    }

    #[test]
    fn the_pivot_follows_what_is_under_the_pointer() {
        // Blender calls it zooming to the mouse position, and it is what makes
        // a zoom feel aimed: the point under the pointer drifts toward the
        // middle, so the next orbit turns around what you were looking at.
        let mut camera = Camera::default();
        let eye = camera.eye();
        let toward = (camera.target - eye).normalize();
        // Off to one side, as a pointer away from the centre would find.
        let focus = eye + toward * 3.0 + Vec3::new(0.8, 0.0, 0.0);

        let before = (focus - camera.target).length();
        camera.zoom_toward(1.0, Some(focus.into()));
        let after = (focus - camera.target).length();
        assert!(
            after < before,
            "the pivot stayed where it was, so the next orbit turns around \
             somewhere the sculptor is not looking"
        );
        assert!(
            after > 0.0,
            "the pivot snapped onto the surface, which swings the view on \
             every notch"
        );
    }

    #[test]
    fn with_nothing_in_front_it_is_the_plain_zoom() {
        // The pointer over empty space. There is nothing to stop at, and
        // refusing to move would read as a broken wheel.
        let mut aimed = Camera::default();
        let mut plain = Camera::default();
        aimed.zoom_toward(1.0, None);
        plain.zoom(1.0);
        assert_eq!(aimed.distance, plain.distance);
        assert_eq!(aimed.target, plain.target);
    }
}
