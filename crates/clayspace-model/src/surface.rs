//! How the sculpted surface itself is drawn.
//!
//! Not what it is — that is the document — but how much of it the sculptor
//! wants to see through. A form drawn solid hides whatever is behind it, and
//! sometimes what is behind it is the reference the form is being made from.

/// How opaque the sculpted surface is drawn, 0 to 1.
///
/// Blender spells this X-ray and ZBrush spells it Ghost; both are a switch.
/// A dial instead, because the useful amount depends on what is behind the
/// form: tracing a silhouette against a photograph wants a different number
/// from reaching a cage's control points.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SurfaceOpacity(f32);

impl Default for SurfaceOpacity {
    fn default() -> Self {
        Self(1.0)
    }
}

impl SurfaceOpacity {
    /// Solid, which is what sculpting wants.
    pub const SOLID: Self = Self(1.0);

    /// The least a surface can be faded to.
    ///
    /// Not zero. A surface faded to nothing is a surface turned off, and the
    /// sculptor loses the form, the brush cursor's footprint on it and any way
    /// of telling that a stroke landed. Turning the layer off is what turning
    /// the layer off is for.
    pub const FAINTEST: f32 = 0.1;

    /// What the automatic ghosting uses while a deformation cage is up.
    ///
    /// Half the control points are behind the form and a solid surface hides
    /// exactly the handles that need reaching, so the cage imposes this as a
    /// ceiling whatever the dial says.
    pub const CAGED: Self = Self(0.42);

    pub fn new(value: f32) -> Self {
        Self(value.clamp(Self::FAINTEST, 1.0))
    }

    pub fn get(self) -> f32 {
        self.0
    }

    /// Whether the surface is solid, and so drawn the ordinary way.
    pub fn is_solid(self) -> bool {
        self.0 >= 1.0
    }

    /// The stricter of two, which is how the cage overrides the dial.
    pub fn and(self, other: Self) -> Self {
        if other.0 < self.0 {
            other
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_opens_solid() {
        // Sculpting is the ordinary case and a solid surface is what it wants.
        assert!(SurfaceOpacity::default().is_solid());
    }

    #[test]
    fn it_cannot_be_faded_to_nothing() {
        // A surface faded to nothing loses the form, the cursor's footprint on
        // it, and any way of telling that a stroke landed.
        let gone = SurfaceOpacity::new(0.0);
        assert!(gone.get() >= SurfaceOpacity::FAINTEST);
        assert!(!gone.is_solid());
    }

    #[test]
    fn it_cannot_be_asked_for_more_than_solid() {
        assert_eq!(SurfaceOpacity::new(4.0).get(), 1.0);
    }

    #[test]
    fn a_cage_overrides_a_solid_surface_but_not_a_fainter_one() {
        // The cage needs to be seen through; the sculptor asking for fainter
        // still than that is not something to argue with.
        let solid = SurfaceOpacity::SOLID.and(SurfaceOpacity::CAGED);
        assert_eq!(solid, SurfaceOpacity::CAGED);

        let fainter = SurfaceOpacity::new(0.2).and(SurfaceOpacity::CAGED);
        assert!(
            fainter.get() < SurfaceOpacity::CAGED.get(),
            "the cage made a faint surface more solid: {}",
            fainter.get()
        );
    }
}
