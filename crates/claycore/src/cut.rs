//! A shape drawn over the model, resolved into an ordinary edit item.
//!
//! ZBrush calls it Trim, 3DCoat a cut. What makes it different from a boolean
//! is not the result but where the shape comes from: a boolean's operand is a
//! form standing in the scene, and a cut's is a line a sculptor drew on the
//! view.
//!
//! **The cut is a prism, not a frustum**, and the engine is emphatic about
//! why: a shape drawn under a perspective camera sweeps a converging wedge, so
//! cutting with one gives "a cut face that is not flat and a solid that depends
//! on where the camera was standing". A trim is a straight cut.
//!
//! **And the engine holds no viewport.** It takes the *frame* the shape was
//! drawn on — an origin and an orthonormal basis — with the outline in **world
//! units** on that frame, not pixels and not normalised device coordinates. A
//! frame that is not orthonormal is refused rather than squared up, "because
//! the shape the user saw was drawn in the frame they think they have". So the
//! crossing from what was drawn to what is cut belongs above this module, and
//! this one only refuses to paper over it.

use claycore_sys as sys;

use crate::error::check;
use crate::error::RawResult;
use crate::{Item, PointType, Result};

/// Which half of the frame an **open** stroke's outline covers.
///
/// It does not decide that half's fate — the op the resolved item is placed
/// with does, and the engine names a second flag as "a second way to say one
/// thing". See [`CutOutline::from_open_curve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrimSide {
    #[default]
    Below = 0,
    Above = 1,
    Left = 2,
    Right = 3,
}

/// The cross-section a cut sweeps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CutShape {
    /// A box on the frame, square to the axes it was drawn with.
    Rect {
        half_width: f32,
        half_height: f32,
    },
    Circle {
        radius: f32,
    },
    /// An outline in frame coordinates, closed implicitly.
    Polygon,
}

/// The frame a shape was drawn on, and how far the sweep reaches.
///
/// `right`, `up` and `forward` must be an orthonormal basis. They are passed
/// as given: an engine that refuses a degenerate frame is more useful than a
/// wrapper that quietly squares one up, since the shape the sculptor saw was
/// drawn in the frame they think they have.
#[derive(Debug, Clone, Copy)]
pub struct CutFrame {
    pub origin: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    /// The sweep direction.
    pub forward: [f32; 3],
    /// The region being cut. Used only to size the sweep, so that a cut goes
    /// all the way through rather than stopping inside and leaving a shelf.
    pub region: ([f32; 3], [f32; 3]),
    /// Bevels the cut walls. Zero is a hard edge.
    pub rounding: f32,
}

/// An outline in frame coordinates: `count * 2` floats, closed implicitly.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CutOutline(pub Vec<f32>);

impl CutOutline {
    pub fn points(&self) -> usize {
        self.0.len() / 2
    }

    /// ZBrush's Trim Curve: an **open** stroke drawn across the form,
    /// flattened and closed against the frame's own bounds on the side it
    /// covers.
    ///
    /// **Not [`Self::from_closed_curve`] with a flag.** That one tessellates
    /// closed and is a spline lasso; joining a trim stroke's endpoints "cuts a
    /// sliver between them instead of dividing the frame". Different shapes
    /// from the same points, so they are different calls.
    ///
    /// `extent_xy` is how far the closing edge reaches in the frame's own
    /// units — large enough to clear the region, or the cut stops short.
    pub fn from_open_curve(
        points_xyzr: &[f32],
        types: Option<&[i32]>,
        side: TrimSide,
        extent_xy: [f32; 2],
        tolerance: f32,
    ) -> Result<Self> {
        Self::resolved("clay_cut_polygon_from_open_curve", |out, count| {
            // SAFETY: `points_xyzr` is four floats a point and its count is
            // passed beside it; `types` is either one int a point or null with
            // the engine reading none; `out` is null on the sizing call and a
            // buffer of `*count` pairs on the second.
            unsafe {
                sys::clay_cut_polygon_from_open_curve(
                    points_xyzr.as_ptr(),
                    points_xyzr.len() / 4,
                    types.map_or(std::ptr::null(), |t| t.as_ptr()),
                    side as i32,
                    extent_xy.as_ptr(),
                    tolerance,
                    out,
                    count,
                )
            }
        })
    }

    /// A **closed** control-point curve — a spline lasso, flattened through the
    /// same tessellator a curve item uses, so the outline follows the curve a
    /// spline item would.
    pub fn from_closed_curve(
        points_xyzr: &[f32],
        types: Option<&[i32]>,
        tolerance: f32,
    ) -> Result<Self> {
        Self::resolved("clay_cut_polygon_from_curve", |out, count| {
            // SAFETY: as `from_open_curve`, without the side and the extent.
            unsafe {
                sys::clay_cut_polygon_from_curve(
                    points_xyzr.as_ptr(),
                    points_xyzr.len() / 4,
                    types.map_or(std::ptr::null(), |t| t.as_ptr()),
                    tolerance,
                    out,
                    count,
                )
            }
        })
    }

    /// The size query, held here so nothing above the bridge can make the
    /// second call with a buffer sized from a stale count.
    ///
    /// Both entry points take the pattern: a first call with a null output
    /// receives the vertex count, and the second fills a buffer of that size.
    /// A caller that kept the count across an edit would size a buffer for an
    /// outline that no longer exists.
    fn resolved(
        operation: &'static str,
        mut call: impl FnMut(*mut f32, *mut usize) -> RawResult,
    ) -> Result<Self> {
        let mut count: usize = 0;
        check(call(std::ptr::null_mut(), &mut count), operation)?;
        if count == 0 {
            return Ok(Self(Vec::new()));
        }
        let mut xy = vec![0.0f32; count * 2];
        let mut filled = count;
        check(call(xy.as_mut_ptr(), &mut filled), operation)?;
        xy.truncate(filled * 2);
        Ok(Self(xy))
    }
}

/// Resolves a drawn shape into an item, which the caller places like any
/// other and which frees itself.
///
/// `NULL` back from the engine is a refusal with the reason in its own error —
/// a frame that is not orthonormal is the documented one — so it is reported
/// rather than corrected here.
pub fn cut(frame: &CutFrame, shape: CutShape, outline: &CutOutline) -> Result<Item> {
    let (kind, half_width, half_height, radius) = match shape {
        CutShape::Rect {
            half_width,
            half_height,
        } => (
            sys::clay_cut_shape::CLAY_CUT_RECT,
            half_width,
            half_height,
            0.0,
        ),
        CutShape::Circle { radius } => (sys::clay_cut_shape::CLAY_CUT_CIRCLE, 0.0, 0.0, radius),
        CutShape::Polygon => (sys::clay_cut_shape::CLAY_CUT_POLYGON, 0.0, 0.0, 0.0),
    };
    let desc = sys::clay_cut_desc {
        struct_size: std::mem::size_of::<sys::clay_cut_desc>() as u32,
        origin: frame.origin,
        right: frame.right,
        up: frame.up,
        forward: frame.forward,
        shape: kind as i32,
        half_width,
        half_height,
        radius,
        rounding: frame.rounding.max(0.0),
        region_min: frame.region.0,
        region_max: frame.region.1,
        // Both zero, which the engine names as what a caller wants "unless it
        // is asking for a deliberate partial cut": the sweep is then derived
        // from the region and the cut passes all the way through. A partial
        // cut is not what this tool is.
        near_extent: 0.0,
        far_extent: 0.0,
    };
    let (points, count) = match shape {
        CutShape::Polygon => (outline.0.as_ptr(), outline.points()),
        _ => (std::ptr::null(), 0),
    };
    // SAFETY: `desc` is fully initialised and declares its own size; the
    // polygon is read only for CLAY_CUT_POLYGON and is null with a zero count
    // otherwise, which is what "not a polygon" spells.
    let raw = unsafe { sys::clay_cut_create(&desc, points, count) };
    // A refusal comes back as NULL with the reason in the engine's own error —
    // a frame that is not orthonormal is the documented one — and `from_raw`
    // turns that into a refusal here rather than a correction.
    Item::from_raw(raw, "clay_cut_create")
}

/// Kept so `PointType` is visible to readers of this module's imports: the
/// curve calls above take the same point types a curve item does.
const _: fn(PointType) -> PointType = |kind| kind;
