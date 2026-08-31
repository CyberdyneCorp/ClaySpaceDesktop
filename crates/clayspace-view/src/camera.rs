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
    /// Roughly how big what is being looked at is, in world units.
    ///
    /// Not a framing decision — [`Camera::distance`] is that — but a depth
    /// one: the far plane has to be past the back of the subject however close
    /// the camera has come to its front, and the distance alone cannot say
    /// where that is. Set by [`Camera::frame_bounds`], which is handed exactly
    /// this, and left alone by orbiting, panning and zooming, none of which
    /// change how big the subject is.
    pub scene_radius: f32,
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
            scene_radius: 1.0,
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

    /// The near and far planes this camera should be drawn with.
    ///
    /// Derived rather than fixed. The pair used to be 0.01 and 1000 whatever
    /// was on screen, which is two failures at once: a thumbnail-sized import
    /// zoomed into is clipped away by a near plane larger than the model, and
    /// a large one gets a depth buffer whose whole useful precision is spent
    /// on the first hundredth of the range.
    ///
    /// The near plane tracks the viewing distance, so how close the camera can
    /// come to a surface is the same at every scale. The far plane clears the
    /// back of the subject from wherever the camera has got to, so zooming in
    /// on the front of a form never clips its back away.
    ///
    /// Both change smoothly with the distance they are derived from, and
    /// nothing on screen is a function of the depth *value* — the buffer is
    /// compared, never displayed — so there is no popping to smooth away.
    pub fn depth_range(&self) -> (f32, f32) {
        // A thousandth of the way in. Under reversed-Z the cost of a small
        // near plane is nearly nothing, which is the whole reason that
        // convention is worth the trouble.
        let near = (self.distance * 1e-3).clamp(1e-6, 0.1);
        let far = (self.distance + self.scene_radius * 4.0).max(near * 1e3);
        (near, far)
    }

    /// The projection, under the reversed-Z convention the viewport draws in.
    ///
    /// Reversed means the near plane maps to depth 1 and the far plane to 0.
    /// Floating point has its precision concentrated near zero; a conventional
    /// mapping spends that precision on the far plane, where nothing needs it,
    /// and starves the near field, where a sculptor is working. Reversing the
    /// range puts the two together and makes precision very nearly uniform
    /// across the whole depth range.
    ///
    /// Obtained by handing glam the planes the other way round, which is
    /// exactly what reversing the range is: `perspective_rh(fov, aspect, f, n)`
    /// produces the matrix that sends `-n` to 1 and `-f` to 0. Writing the
    /// sixteen entries out by hand instead would be the same matrix with more
    /// ways to get a sign wrong; `the_depth_range_is_reversed` holds the claim.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let (near, far) = self.depth_range();
        if self.preset.is_orthographic() {
            // Half-height is derived from the distance so that switching
            // projection keeps the subject the same size on screen.
            let half_height = self.distance * (self.fov_y * 0.5).tan();
            let half_width = half_height * aspect;
            // Symmetric about the eye, so the subject is not clipped by a
            // plane behind the camera when the view is turned; reversed by the
            // same swap as the perspective case.
            #[allow(deprecated)]
            Mat4::orthographic_rh(
                -half_width,
                half_width,
                -half_height,
                half_height,
                far,
                -far,
            )
        } else {
            #[allow(deprecated)]
            Mat4::perspective_rh(self.fov_y, aspect, far, near)
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
    /// How much nearer one notch of the wheel brings the camera.
    ///
    /// A notch in divides the distance by this and a notch out multiplies by
    /// it, so about seven per cent a click. Fine enough to creep up on a detail
    /// without the wheel becoming a chore, and it compounds: ten notches still
    /// halve the distance.
    const ZOOM_PER_NOTCH: f32 = 1.08;

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

    /// The nearest the wheel alone can bring the camera.
    ///
    /// A floor rather than a stop on the clay: with nothing under the pointer
    /// there is nothing to stop at, and a multiplicative zoom would otherwise
    /// creep toward the pivot forever, taking the near plane with it.
    pub const MIN_DISTANCE: f32 = 0.01;

    /// The furthest, which is what keeps a hard flick of the wheel from
    /// compounding out to infinity and taking the view matrix with it.
    pub const MAX_DISTANCE: f32 = 10_000.0;

    /// Zooms multiplicatively, so each notch feels the same at any distance.
    ///
    /// The plain form, with nothing in front of the camera to stop at. It
    /// still bottoms out, but on an arbitrary floor rather than on the clay —
    /// which is what "zooming goes inside the model" is.
    pub fn zoom(&mut self, amount: f32) {
        self.zoom_toward(amount, None);
    }

    /// What one notch does to the distance.
    ///
    /// A *factor* per notch rather than a fraction subtracted from one, which
    /// is what this was. The subtracted form has two faults that only show up
    /// away from small numbers: it crosses zero — at more than ten notches in
    /// one frame it asks for a negative distance, which only the clamp caught —
    /// and it is not symmetric, so a notch in followed by a notch out lands
    /// somewhere slightly nearer than it started and a wheel jiggled back and
    /// forth walks the camera in. A factor has neither: it cannot reach zero
    /// from above, and in-then-out is exactly where it began.
    fn zoomed(&self, notches: f32) -> f32 {
        (self.distance * Self::ZOOM_PER_NOTCH.powf(-notches))
            .clamp(Self::MIN_DISTANCE, Self::MAX_DISTANCE)
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
    /// `amount` is in wheel notches: one whole one for a click of the wheel,
    /// a fraction of one for a trackpad.
    pub fn zoom_toward(&mut self, amount: f32, focus: Option<[f32; 3]>) {
        let wanted = self.zoomed(amount);
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
        let keep = (gap * Self::STANDOFF).max(self.depth_range().0 * 2.0);
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
            self.frame_default();
            return;
        }
        self.target = (min + max) * 0.5;
        // The bounding sphere's radius, then far enough back that it fits the
        // vertical field of view with a margin.
        let radius = size.length() * 0.5;
        self.distance = (radius / (self.fov_y * 0.5).sin()).max(radius * 1.5) * 1.1;
        // And remembered, because it is what the far plane is measured
        // against: zooming in on the front of this form must not clip its
        // back away, and the distance the camera has zoomed to cannot say
        // where that back is.
        self.scene_radius = radius;
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

    /// Where a world point lands in normalised device coordinates, if it is
    /// in front of the camera.
    ///
    /// Exactly the inverse of [`Camera::ray_through`], and written from the
    /// same basis rather than from the projection matrix: the matrix carries
    /// the reversed-Z convention and clip planes this does not need, and two
    /// derivations of one mapping are two things that can disagree. What it is
    /// for is asking where a control point *is on screen* — which is the
    /// question a rubber-band selection asks of every point at once.
    ///
    /// `None` where the point is behind a perspective camera, which has no
    /// screen position to have.
    pub fn screen_through(&self, world: [f32; 3], aspect: f32) -> Option<[f32; 2]> {
        let eye = self.eye();
        let forward = (self.target - eye).normalize_or_zero();
        let right = forward.cross(self.up()).normalize_or_zero();
        let up = right.cross(forward);
        let away = Vec3::from(world) - eye;
        let (across, high) = (away.dot(right), away.dot(up));

        if self.preset.is_orthographic() {
            // Parallel rays, so depth does not divide: everything the camera
            // can see has a position, in front of it or behind.
            let half_height = self.distance * (self.fov_y * 0.5).tan();
            if half_height.abs() < 1e-9 {
                return None;
            }
            Some([across / (half_height * aspect), high / half_height])
        } else {
            let depth = away.dot(forward);
            if depth <= 1e-6 {
                return None; // Behind the eye, or on the lens.
            }
            let tan = (self.fov_y * 0.5).tan();
            if tan.abs() < 1e-9 || aspect.abs() < 1e-9 {
                return None;
            }
            Some([across / (depth * tan * aspect), high / (depth * tan)])
        }
    }

    /// The framing an empty document gets.
    pub fn frame_default(&mut self) {
        self.target = Vec3::ZERO;
        self.distance = 4.0;
        self.scene_radius = 1.0;
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
    fn one_notch_moves_the_distance_by_the_stated_fraction() {
        // Reported as "the zoom jumps are too big". It was: egui reports
        // scrolling in *points* and one wheel notch is forty of them, and that
        // number went straight into a formula written for notches. A notch in
        // asked for a distance of −3× the current one, which only the clamp
        // caught; a notch out was five times further away. The unit is named
        // in the signature now, and this is what one of them is worth.
        let mut camera = Camera::default();
        let was = camera.distance;
        camera.zoom(1.0);
        let ratio = camera.distance / was;
        assert!(
            (ratio - 1.0 / Camera::ZOOM_PER_NOTCH).abs() < 1e-4,
            "one notch in moved the distance by {ratio}"
        );
        assert!(
            (0.9..0.96).contains(&ratio),
            "a notch of {ratio} is not the fine step this is meant to be"
        );
    }

    #[test]
    fn a_notch_in_and_a_notch_out_land_where_they_started() {
        // The subtracted form was not symmetric: 0.9 then 1.1 is 0.99, so a
        // wheel jiggled back and forth walked the camera in a little each time.
        let mut camera = Camera::default();
        let was = camera.distance;
        for _ in 0..20 {
            camera.zoom(1.0);
            camera.zoom(-1.0);
        }
        assert!(
            (camera.distance - was).abs() < 1e-3,
            "twenty in-and-out pairs left the camera at {} rather than {was}",
            camera.distance
        );
    }

    #[test]
    fn a_hard_flick_of_the_wheel_does_not_invert_the_camera() {
        // The subtracted form crosses zero past ten notches in one frame — a
        // trackpad fling, or the raw point delta this used to be handed. A
        // factor cannot reach zero from above.
        let mut camera = Camera::default();
        for amount in [10.0, 40.0, 400.0, 4000.0] {
            let mut camera = camera;
            camera.zoom(amount);
            assert!(
                camera.distance > 0.0 && camera.distance.is_finite(),
                "{amount} notches in one frame put the camera at {}",
                camera.distance
            );
        }
        camera.zoom(-4000.0);
        assert!(camera.distance.is_finite());
    }

    #[test]
    fn a_trackpad_moves_by_a_fraction_of_a_notch() {
        // A wheel steps; a trackpad glides. Both arrive as notches, so a tenth
        // of one has to be a tenth of the step rather than nothing.
        let mut camera = Camera::default();
        let was = camera.distance;
        for _ in 0..10 {
            camera.zoom(0.1);
        }
        let stepped = {
            let mut camera = Camera::default();
            camera.zoom(1.0);
            camera.distance
        };
        assert!(
            (camera.distance - stepped).abs() < 1e-3,
            "ten tenths of a notch reached {} where one notch reaches {stepped}",
            camera.distance
        );
        assert!(camera.distance < was);
    }

    /// Both bounds, on the numbers rather than on the sign.
    ///
    /// A multiplicative factor cannot reach zero from above and 1.08^500 is
    /// finite in f32, so `> 0.0` and `is_finite` are true whatever the clamp
    /// says — they were satisfied by a floor of a ten-millionth and a ceiling
    /// of 1e30. Five hundred notches is well past saturation in both
    /// directions, so the answer is the bound itself.
    #[test]
    fn zoom_is_multiplicative_and_bounded() {
        let mut camera = Camera::default();
        for _ in 0..500 {
            camera.zoom(1.0);
        }
        assert_eq!(
            camera.distance,
            Camera::MIN_DISTANCE,
            "zooming in did not stop at the floor"
        );

        for _ in 0..500 {
            camera.zoom(-1.0);
        }
        assert_eq!(
            camera.distance,
            Camera::MAX_DISTANCE,
            "zooming out did not stop at the ceiling"
        );
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
        // Most of the way, and the threshold is derived from the rate rather
        // than written as a number — the rate is a decision that can change,
        // and a literal here would have to be re-guessed each time it does.
        // Twenty notches leave `(1/rate)^20` of the distance; the allowance is
        // for the pivot following part of the way, which keeps the camera a
        // little further from the surface than the distance alone suggests.
        let predicted = start * Camera::ZOOM_PER_NOTCH.powi(-20);
        assert!(
            gap < predicted * 1.5,
            "twenty notches went from {start} to {gap} from the clay, where \
             the rate predicts about {predicted}"
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

    /// The near plane maps to depth 1 and the far plane to 0.
    ///
    /// This is the whole claim of a reversed range, and it is one sign flip
    /// away from a viewport that draws nothing: every pipeline compares
    /// `GreaterEqual` and the buffer clears to zero, so a projection that
    /// still ran the other way would fail the depth test everywhere.
    #[test]
    fn the_depth_range_is_reversed() {
        let camera = Camera::default();
        let (near, far) = camera.depth_range();
        let projection = camera.projection(1.5);

        // Points on the view axis at each plane. The camera looks down -z.
        let depth_at = |distance: f32| {
            let clip = projection * glam::Vec4::new(0.0, 0.0, -distance, 1.0);
            clip.z / clip.w
        };
        assert!(
            (depth_at(near) - 1.0).abs() < 1e-3,
            "the near plane came out at {}, not 1",
            depth_at(near)
        );
        assert!(
            depth_at(far).abs() < 1e-3,
            "the far plane came out at {}, not 0",
            depth_at(far)
        );
        // And monotonic between them, or the depth test orders nothing.
        let mid = depth_at((near + far) * 0.5);
        assert!(
            mid > 0.0 && mid < 1.0,
            "a point between the planes came out at {mid}"
        );
        assert!(
            depth_at(near * 4.0) > depth_at(far * 0.5),
            "nearer must compare greater under a reversed range"
        );
    }

    /// The orthographic presets reverse too, or switching to Front would draw
    /// the back of the form in front of it.
    #[test]
    fn the_orthographic_presets_reverse_with_it() {
        let mut camera = Camera::default();
        camera.apply_preset(ViewPreset::Front);
        let projection = camera.projection(1.5);
        let depth_at = |distance: f32| {
            let clip = projection * glam::Vec4::new(0.0, 0.0, -distance, 1.0);
            clip.z / clip.w
        };
        assert!(
            depth_at(1.0) > depth_at(10.0),
            "nearer must compare greater under an orthographic reversed range too"
        );
    }

    /// The occlusion passes reconstruct a view position from a depth and the
    /// inverse of this matrix. If the two disagree the whole pass shades a
    /// surface that is not where the sculpt is.
    #[test]
    fn the_projection_round_trips_through_its_inverse() {
        for preset in ViewPreset::ALL {
            let mut camera = Camera::default();
            camera.apply_preset(preset);
            let projection = camera.projection(16.0 / 9.0);
            let inverse = projection.inverse();

            for point in [
                glam::Vec3::new(0.0, 0.0, -1.0),
                glam::Vec3::new(0.7, -0.4, -2.5),
                glam::Vec3::new(-1.2, 0.9, -8.0),
            ] {
                let clip = projection * point.extend(1.0);
                let back = inverse * clip;
                let back = back.truncate() / back.w;
                assert!(
                    (back - point).length() < 1e-2,
                    "{preset:?}: {point} came back as {back}"
                );
            }
        }
    }

    /// A form far smaller than the old fixed near plane of 0.01 has to be
    /// drawable. It was not: the camera framed it at a distance under the near
    /// plane, so the whole model sat in front of the clip and nothing showed.
    #[test]
    fn a_tiny_form_is_not_clipped_away_by_the_near_plane() {
        let mut camera = Camera::default();
        camera.frame_bounds(Vec3::splat(-0.002), Vec3::splat(0.002));
        let (near, far) = camera.depth_range();
        assert!(
            near < camera.distance * 0.5,
            "the near plane at {near} is half the {} the camera stands off",
            camera.distance
        );
        assert!(
            far > camera.distance + camera.scene_radius,
            "the far plane at {far} is in front of the back of the form"
        );
    }

    /// And a form far larger. The far plane has to clear its back from
    /// wherever the camera has zoomed to, which the viewing distance alone
    /// cannot say — zooming into the front of a bust must not clip the back of
    /// its head away.
    #[test]
    fn the_far_plane_clears_the_back_of_a_form_zoomed_into() {
        let mut camera = Camera::default();
        camera.frame_bounds(Vec3::splat(-100.0), Vec3::splat(100.0));
        let radius = camera.scene_radius;
        // Right up against the front of it.
        camera.distance = radius * 0.01;
        let (near, far) = camera.depth_range();
        assert!(
            far > camera.distance + radius * 2.0,
            "zoomed in to {}, the far plane at {far} clips the back of a form \
             of radius {radius}",
            camera.distance
        );
        assert!(near > 0.0 && near < far);
    }

    /// The range moves smoothly with the distance it is derived from. A near
    /// plane that jumped would clip a surface in and out between two frames of
    /// one continuous zoom.
    #[test]
    fn the_depth_range_does_not_jump_as_the_camera_moves() {
        let mut camera = Camera::default();
        camera.frame_bounds(Vec3::splat(-1.0), Vec3::splat(1.0));
        let mut previous = camera.depth_range();
        for _ in 0..40 {
            camera.zoom(0.5);
            let range = camera.depth_range();
            let ratio = range.0 / previous.0;
            assert!(
                (0.5..2.0).contains(&ratio),
                "the near plane went from {} to {} in one notch",
                previous.0,
                range.0
            );
            previous = range;
        }
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

    #[test]
    fn a_screen_position_is_the_ray_that_goes_back_through_it() {
        // The pair has to be exact inverses or a rubber-band selection catches
        // points beside the ones the sculptor drew the box around. Checked
        // both ways round, on both projections, away from the centre where a
        // sign error would hide.
        for preset in [ViewPreset::Perspective, ViewPreset::Front] {
            let mut camera = Camera::default();
            camera.apply_preset(preset);
            let aspect = 4.0 / 3.0;
            for ndc in [[0.0f32, 0.0], [0.5, -0.25], [-0.9, 0.8], [0.31, 0.62]] {
                let (origin, direction) = camera.ray_through(ndc, aspect);
                let world: [f32; 3] = std::array::from_fn(|i| origin[i] + direction[i] * 3.0);
                let back = camera
                    .screen_through(world, aspect)
                    .expect("a point in front of the camera has a screen position");
                for axis in 0..2 {
                    assert!(
                        (back[axis] - ndc[axis]).abs() < 1e-3,
                        "{preset:?}: {ndc:?} came back as {back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn nothing_behind_a_perspective_camera_is_on_screen() {
        // A point behind the eye projects as neatly as one in front, with the
        // sign of the depth flipped — so without this a marquee drawn over the
        // form would also catch whatever stood behind the camera.
        let camera = Camera::default();
        let eye = camera.eye();
        let behind: [f32; 3] = (eye + (eye - camera.target).normalize() * 2.0).into();
        assert!(camera.screen_through(behind, 1.5).is_none());
    }
}
