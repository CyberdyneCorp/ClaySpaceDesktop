//! The cases a cheaper occlusion path can break.
//!
//! `visual_occlusion` asks whether occlusion runs at all — a worked form
//! darkens, a sphere does not. That is the right question for a pass being
//! introduced and the wrong one for a pass being made cheaper: half-resolution
//! occlusion, a bilateral upsample and a scale-derived radius each preserve
//! "the folds darken" while breaking something else.
//!
//! So these fixtures name the *something else*. Each is a piece of geometry
//! chosen so that one property of the occlusion path is the only thing that
//! decides the picture:
//!
//! | fixture      | breaks when                                              |
//! |--------------|----------------------------------------------------------|
//! | `deep_crease`| the resolution drop loses the crease                      |
//! | `thin_gap`   | the reduction averages a foreground and a background      |
//! | `silhouette` | the upsample is not depth-aware and bleeds across an edge |
//! | `contact`    | the radius or bias loses the shadow where two forms meet  |
//! | `scale_*`    | the radius is absolute rather than a fraction of the form |
//!
//! The geometry is built here from plain triangles rather than sculpted
//! through the engine. That is deliberate: a fixture whose shape depends on a
//! mesher is a fixture that changes when the mesher does, and then a rendering
//! regression and a meshing change are indistinguishable.
//!
//! Numerical image equality is not used and would be wrong to use — occlusion
//! is a sampled integral and no two drivers agree on it to the byte. Every
//! assertion here is a *relation*: this region darkened, that region did not,
//! these two scales darkened alike.

mod support;

use clayspace_view::{Camera, GpuMesh, Image, Vertex};
use support::{save, Harness};

// ----------------------------------------------------------------------------
// Geometry
// ----------------------------------------------------------------------------

/// Triangles being accumulated into a fixture.
#[derive(Default)]
struct Shape {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Shape {
    /// One quad, wound anticlockwise seen from the side its normal faces.
    ///
    /// The normal is given rather than derived from the corners because these
    /// fixtures include faces whose winding a reader would have to work out,
    /// and a normal that disagrees with the winding is a face the opaque
    /// pipeline culls — which reads as "occlusion is broken" rather than as
    /// "the fixture is inside out".
    fn quad(&mut self, corners: [[f32; 3]; 4], normal: [f32; 3]) {
        let base = self.vertices.len() as u32;
        for position in corners {
            self.vertices.push(Vertex {
                position,
                normal,
                color: [1.0, 1.0, 1.0],
                mask: 0.0,
            });
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Every position multiplied, for the scale fixtures.
    fn scaled(&self, by: f32) -> Self {
        Self {
            vertices: self
                .vertices
                .iter()
                .map(|v| Vertex {
                    position: v.position.map(|c| c * by),
                    ..*v
                })
                .collect(),
            indices: self.indices.clone(),
        }
    }

    fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        Vertex::bounds(&self.vertices).expect("a fixture has vertices")
    }

    fn upload(&self, harness: &Harness) -> GpuMesh {
        let mut mesh = GpuMesh::new(&harness.gpu);
        mesh.upload(&harness.gpu, &self.vertices, &self.indices);
        mesh
    }
}

/// A flat slab with a trench cut across it, seen from above.
///
/// The trench is the fold. `width` and `depth` are what separate the fixtures:
/// a wide shallow one is a crease, a narrow deep one is a gap that a
/// half-resolution depth reduction can average away.
fn trenched_slab(width: f32, depth: f32) -> Shape {
    let mut shape = Shape::default();
    let (half, reach) = (width * 0.5, 2.0);
    let up = [0.0, 1.0, 0.0];

    // The two halves of the top surface.
    shape.quad(
        [
            [-reach, 0.0, -reach],
            [-reach, 0.0, reach],
            [-half, 0.0, reach],
            [-half, 0.0, -reach],
        ],
        up,
    );
    shape.quad(
        [
            [half, 0.0, -reach],
            [half, 0.0, reach],
            [reach, 0.0, reach],
            [reach, 0.0, -reach],
        ],
        up,
    );
    // The trench: two walls facing each other across it, and its floor.
    shape.quad(
        [
            [-half, 0.0, -reach],
            [-half, 0.0, reach],
            [-half, -depth, reach],
            [-half, -depth, -reach],
        ],
        [1.0, 0.0, 0.0],
    );
    shape.quad(
        [
            [half, -depth, -reach],
            [half, -depth, reach],
            [half, 0.0, reach],
            [half, 0.0, -reach],
        ],
        [-1.0, 0.0, 0.0],
    );
    shape.quad(
        [
            [-half, -depth, -reach],
            [-half, -depth, reach],
            [half, -depth, reach],
            [half, -depth, -reach],
        ],
        up,
    );
    shape
}

/// A box standing on a ground plane, for the shadow where the two meet.
fn box_on_ground() -> Shape {
    let mut shape = Shape::default();
    let reach = 2.5;
    shape.quad(
        [
            [-reach, 0.0, -reach],
            [-reach, 0.0, reach],
            [reach, 0.0, reach],
            [reach, 0.0, -reach],
        ],
        [0.0, 1.0, 0.0],
    );

    let (h, top) = (0.5, 1.0);
    // Four walls and a lid. The underside is never seen and is left off.
    for (normal, corners) in [
        (
            [0.0, 0.0, 1.0],
            [[-h, 0.0, h], [h, 0.0, h], [h, top, h], [-h, top, h]],
        ),
        (
            [0.0, 0.0, -1.0],
            [[h, 0.0, -h], [-h, 0.0, -h], [-h, top, -h], [h, top, -h]],
        ),
        (
            [1.0, 0.0, 0.0],
            [[h, 0.0, h], [h, 0.0, -h], [h, top, -h], [h, top, h]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[-h, 0.0, -h], [-h, 0.0, h], [-h, top, h], [-h, top, -h]],
        ),
        (
            [0.0, 1.0, 0.0],
            [[-h, top, -h], [-h, top, h], [h, top, h], [h, top, -h]],
        ),
    ] {
        shape.quad(corners, normal);
    }
    shape
}

/// A creased block floating well in front of a flat wall.
///
/// The block's own folds darken; the wall behind it has nothing near enough to
/// occlude it. Any darkening of the wall beside the block's outline is the
/// occlusion of the block leaking across the silhouette — which is exactly
/// what an upsample that averages without regard for depth does.
fn block_before_a_wall() -> (Shape, Camera) {
    let mut block = trenched_slab(0.35, 0.9);
    // Shrink the slab's footprint so it covers the middle of the frame and
    // leaves wall around it.
    for vertex in &mut block.vertices {
        vertex.position[0] *= 0.3;
        vertex.position[2] *= 0.3;
    }
    // Framed on the block alone, before the wall is added: framing on both
    // would put the camera far enough back to fit an eight-unit wall, and the
    // block — the thing whose outline is being examined — would be a speck.
    let (min, max) = block.bounds();
    let mut camera = Camera::default();
    camera.frame_bounds(min.into(), max.into());
    camera.orbit(0.0, 0.45);
    // Framing is not the same question as depth. `frame_bounds` sets the
    // scene radius from what it was asked to frame, and the far plane is
    // derived from that — so framing on the block alone would put the far
    // plane in front of the wall and clip away the background this fixture is
    // about. The application frames the whole scene and never meets this; a
    // fixture that deliberately frames one part of one has to say how big the
    // rest is.
    camera.scene_radius = WALL_DISTANCE * 1.5;

    // The wall, well behind the block and facing the camera. Wide and tall
    // enough to fill the frame behind it, so it has no edge of its own for
    // the block/wall classification below to trip over.
    let mut shape = block;
    shape.quad(
        [
            [-9.0, -9.0, -WALL_DISTANCE],
            [9.0, -9.0, -WALL_DISTANCE],
            [9.0, 9.0, -WALL_DISTANCE],
            [-9.0, 9.0, -WALL_DISTANCE],
        ],
        [0.0, 0.0, 1.0],
    );
    (shape, camera)
}

/// How far behind the origin the background wall stands.
///
/// Far enough that nothing on the block is within occlusion reach of it: the
/// radius is a fraction of the form's own size, and the form is about a unit
/// across.
const WALL_DISTANCE: f32 = 6.0;

// ----------------------------------------------------------------------------
// Measurement
// ----------------------------------------------------------------------------

/// How much darker a pixel has to come out before it counts as shaded.
///
/// Deliberately not `support::RENDER_NOISE`, which is 32 levels. That constant
/// answers "is this the same picture" for two whole frames, and is set wide
/// enough that a tile-based driver rebinning the frame cannot trip it.
/// Occlusion is not a picture, it is a multiplier: the worked reference form
/// darkens its deepest folds by a few tens of levels and everything else by
/// single digits, so a 32-level floor counts only the very bottom of a crease
/// and reports a working pass as a broken one.
///
/// Eight is comfortably above what two renders of the same geometry differ by
/// — the same-frame comparisons in `support` measure zero pixels over eight on
/// every fixture that was not meant to change — and low enough to see the
/// shading rather than only its extreme.
const DARKENED: i32 = 8;

/// The same frame with occlusion off and on.
struct Pair {
    without: Image,
    with: Image,
}

impl Pair {
    /// How much darker a pixel came out with occlusion on.
    ///
    /// Signed, and read off the red channel: the material is neutral, so one
    /// channel carries the whole value and reading three would only average
    /// the same number with itself.
    fn darkening(&self, x: u32, y: u32) -> i32 {
        self.without.pixel(x, y)[0] as i32 - self.with.pixel(x, y)[0] as i32
    }

    /// Pixels in a region that darkened past the noise floor, and the region's
    /// mean darkening.
    fn over(&self, region: impl Fn(u32, u32) -> bool) -> (usize, f64) {
        let (mut count, mut total, mut seen) = (0usize, 0.0f64, 0usize);
        for y in 0..self.with.height {
            for x in 0..self.with.width {
                if !region(x, y) {
                    continue;
                }
                seen += 1;
                let delta = self.darkening(x, y);
                total += delta as f64;
                if delta > DARKENED {
                    count += 1;
                }
            }
        }
        (count, total / seen.max(1) as f64)
    }

    /// The fraction of the drawn subject that darkened past the noise floor.
    ///
    /// A fraction rather than a count, because the scale fixtures frame two
    /// very differently sized forms to the same screen area and a count would
    /// compare their framing rather than their shading.
    fn darkened_fraction(&self, ground: [u8; 4]) -> f64 {
        let (mut dark, mut subject) = (0usize, 0usize);
        for y in 0..self.with.height {
            for x in 0..self.with.width {
                if is_ground(&self.without, x, y, ground) {
                    continue;
                }
                subject += 1;
                if self.darkening(x, y) > DARKENED {
                    dark += 1;
                }
            }
        }
        dark as f64 / subject.max(1) as f64
    }
}

/// Whether this pixel is the viewport's ground rather than the sculpt.
///
/// The ground is not written by the surface pipeline and so has no depth for
/// the occlusion pass to read; counting it would dilute every measure here
/// with pixels that could not have changed.
fn is_ground(image: &Image, x: u32, y: u32, ground: [u8; 4]) -> bool {
    let p = image.pixel(x, y);
    (0..3).all(|c| p[c].abs_diff(ground[c]) < 6)
}

/// Renders a fixture twice, with occlusion off and on, and saves both.
fn pair(harness: &mut Harness, shape: &Shape, camera: &Camera, name: &str) -> Pair {
    let mesh = shape.upload(harness);
    harness.renderer.set_occlusion(false);
    let without = harness
        .target
        .capture(&harness.gpu, &harness.renderer, camera, &mesh, false);
    save(&without, &format!("95-ao-{name}-off"));

    harness.renderer.set_occlusion(true);
    let with = harness
        .target
        .capture(&harness.gpu, &harness.renderer, camera, &mesh, false);
    save(&with, &format!("95-ao-{name}-on"));

    Pair { without, with }
}

/// A camera looking down into a fixture, close enough that the trench fills a
/// useful part of the frame.
///
/// Down, and the sign matters: these fixtures are open shells with no
/// underside, drawn by a pipeline that culls back faces, so a camera that ends
/// up below the slab sees nothing at all. `orbit` raises the pitch with a
/// positive delta and the default preset already sits a little above the
/// horizon, so this is a view from above the trench looking along it.
fn looking_in(shape: &Shape) -> Camera {
    let (min, max) = shape.bounds();
    let mut camera = Camera::default();
    camera.frame_bounds(min.into(), max.into());
    camera.orbit(0.35, 0.45);
    camera
}

// ----------------------------------------------------------------------------
// Fixtures
// ----------------------------------------------------------------------------

/// A wide fold, of the kind a sculptor pushes into a flank.
///
/// The least demanding fixture, and the one a resolution drop is most likely
/// to survive. It is here as the control: if this stops darkening, the pass is
/// not running rather than mis-tuned.
#[test]
fn a_deep_crease_darkens() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let shape = trenched_slab(0.8, 0.5);
    let camera = looking_in(&shape);
    let pair = pair(&mut harness, &shape, &camera, "deep-crease");

    let (dark, mean) = pair.over(|_, _| true);
    println!("deep crease: {dark} pixels darkened, mean {mean:.2}");
    assert!(
        dark > 200,
        "a fold half as deep as it is wide darkened only {dark} pixels — see \
         target/visual/95-ao-deep-crease-on.png"
    );
}

/// A narrow deep gap, which is where a half-resolution depth reduction earns
/// its keep or gives itself away.
///
/// Reducing four full-resolution depths to one by *averaging* them puts a
/// surface halfway between the lip of the gap and its floor — a surface that
/// is not there, and which occludes nothing. Taking the closest of the four
/// keeps the lip, and the gap stays a gap.
#[test]
fn a_thin_gap_darkens_without_filling_in() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let shape = trenched_slab(0.18, 0.9);
    let camera = looking_in(&shape);
    let ground = harness.background();
    let pair = pair(&mut harness, &shape, &camera, "thin-gap");

    let (dark, mean) = pair.over(|x, y| !is_ground(&pair.without, x, y, ground));
    println!("thin gap: {dark} pixels darkened, mean {mean:.2}");
    assert!(
        dark > 60,
        "a gap five times deeper than it is wide darkened only {dark} pixels, \
         which is a reduction that filled the gap in — see \
         target/visual/95-ao-thin-gap-on.png"
    );

    // And the slab either side of it is flat and open, so most of the frame
    // must be left alone. A pass that darkens everything is not shading a gap.
    let subject = (0..pair.with.height)
        .flat_map(|y| (0..pair.with.width).map(move |x| (x, y)))
        .filter(|(x, y)| !is_ground(&pair.without, *x, *y, ground))
        .count();
    assert!(
        dark * 3 < subject,
        "{dark} of {subject} surface pixels darkened, which is the whole slab \
         and not a gap in it"
    );
}

/// Occlusion must not cross a silhouette.
///
/// The creased block in front carries real occlusion. The wall six units
/// behind it carries none: nothing is within reach of it. Any darkening of the
/// wall beside the block's outline came across the silhouette, which is the
/// halo that gives a screen-space effect away — and which a box blur over the
/// occlusion buffer produces by construction, because it averages neighbours
/// it has no reason to believe are the same surface.
///
/// Measured away from the outline rather than over the whole wall. The pixels
/// the outline itself passes through are shared: at four samples a pixel, a
/// pixel on the edge is part block and part wall, and the reduction reports
/// the block for it because that is the nearer surface. That is correct, and
/// it is not what "bleeding" means. Bleeding is the *band* beyond the edge,
/// which is what a blur produces and a depth-weighted average does not — so
/// the assertion excludes a margin either side of the outline and holds the
/// rest of the wall to nothing at all.
#[test]
fn occlusion_does_not_bleed_across_a_silhouette() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let (shape, camera) = block_before_a_wall();
    let ground = harness.background();
    let pair = pair(&mut harness, &shape, &camera, "silhouette");

    // The wall is what is furthest from the camera, so it is what the frame
    // shows outside the block's outline. Its own shading is flat: with
    // occlusion off it is one value, which is what identifies it.
    let wall_value = wall_value(&pair.without);
    let (width, height) = (pair.with.width, pair.with.height);
    let at = |x: u32, y: u32| (y * width + x) as usize;

    let mut is_wall = vec![false; (width * height) as usize];
    let mut is_block = vec![false; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            if is_ground(&pair.without, x, y, ground) {
                continue;
            }
            if pair.without.pixel(x, y)[0].abs_diff(wall_value) <= 2 {
                is_wall[at(x, y)] = true;
            } else {
                is_block[at(x, y)] = true;
            }
        }
    }

    let wall_pixels = is_wall.iter().filter(|w| **w).count();
    let block_pixels = is_block.iter().filter(|b| **b).count();
    assert!(
        wall_pixels > 20_000 && block_pixels > 2_000,
        "the fixture is not a block against a wall: {block_pixels} block pixels \
         and {wall_pixels} wall pixels — see \
         target/visual/95-ao-silhouette-off.png"
    );

    // Wall pixels within this many of the outline are excluded, and the figure
    // is arithmetic rather than taste. An occlusion pixel covers two display
    // pixels; the upsample weighs a three-by-three neighbourhood of them, so
    // it can legitimately read an occlusion pixel two display pixels away; and
    // that occlusion pixel was itself reduced from a two-pixel block which may
    // straddle the outline. Two plus two, and one more for the multisampled
    // edge itself. Beyond five display pixels no part of the pass has any
    // business having seen the block, so anything darkening there crossed the
    // silhouette.
    const MARGIN: i32 = 5;
    let near_block = |x: u32, y: u32| {
        (-MARGIN..=MARGIN).any(|dy| {
            (-MARGIN..=MARGIN).any(|dx| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                nx >= 0
                    && ny >= 0
                    && (nx as u32) < width
                    && (ny as u32) < height
                    && is_block[at(nx as u32, ny as u32)]
            })
        })
    };

    let mut bled = 0usize;
    let mut open_wall = 0usize;
    let mut worst = 0i32;
    for y in 0..height {
        for x in 0..width {
            if !is_wall[at(x, y)] || near_block(x, y) {
                continue;
            }
            open_wall += 1;
            let delta = pair.darkening(x, y);
            worst = worst.max(delta);
            if delta > DARKENED {
                bled += 1;
            }
        }
    }
    println!("silhouette: {bled} of {open_wall} open wall pixels darkened, worst {worst} levels");

    assert!(
        open_wall > 10_000,
        "excluding the outline's margin left only {open_wall} wall pixels to \
         judge, which is not a background"
    );
    assert_eq!(
        bled, 0,
        "{bled} background pixels more than {MARGIN} from the block's outline \
         darkened, the worst by {worst} levels — that is occlusion crossing a \
         silhouette; see target/visual/95-ao-silhouette-on.png"
    );
}

/// The flat background's value, taken as the most common one in the frame.
///
/// The viewport's ground is the other flat expanse in a frame and it is dark,
/// while the wall is lit by the material — so the most common value at or
/// above mid grey is the wall rather than the ground.
fn wall_value(image: &Image) -> u8 {
    let mut histogram = [0usize; 256];
    for y in 0..image.height {
        for x in 0..image.width {
            histogram[image.pixel(x, y)[0] as usize] += 1;
        }
    }
    histogram
        .iter()
        .enumerate()
        .skip(96)
        .max_by_key(|(_, count)| **count)
        .map(|(value, _)| value as u8)
        .unwrap_or(128)
}

/// Where a form meets the ground it stands on.
///
/// The contact shadow is the shortest-range thing occlusion does, and the
/// first casualty of a radius or bias tuned for a different scale.
#[test]
fn a_contact_shadow_survives() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let shape = box_on_ground();
    let (min, max) = shape.bounds();
    let mut camera = Camera::default();
    camera.frame_bounds(min.into(), max.into());
    // Above the ground plane, or its single upward-facing quad is culled and
    // the fixture is a box standing on nothing.
    camera.orbit(0.4, 0.35);
    let pair = pair(&mut harness, &shape, &camera, "contact");

    let (dark, mean) = pair.over(|_, _| true);
    println!("contact: {dark} pixels darkened, mean {mean:.2}");
    assert!(
        dark > 100,
        "a box standing on a plane cast no contact shadow — {dark} pixels \
         darkened; see target/visual/95-ao-contact-on.png"
    );
}

/// The same form at a hundredth and a hundred times its size shades alike.
///
/// The occlusion radius was an absolute figure in view units, tuned against a
/// reference form of radius one. An imported model at another scale therefore
/// got occlusion that was either invisible or total, and neither is a property
/// of the shape. The radius is a fraction of what is being drawn, so framing
/// the same form at any size has to produce the same picture.
#[test]
fn occlusion_is_the_same_at_any_scale() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let ground = harness.background();
    let base = trenched_slab(0.6, 0.6);

    let mut fractions = Vec::new();
    for (name, factor) in [("small", 0.01f32), ("large", 100.0)] {
        let shape = base.scaled(factor);
        let camera = looking_in(&shape);
        let pair = pair(&mut harness, &shape, &camera, &format!("scale-{name}"));
        let fraction = pair.darkened_fraction(ground);
        println!(
            "scale {name} (×{factor}): {:.1}% of the form darkened",
            fraction * 100.0
        );
        fractions.push((name, factor, fraction));
    }

    let (_, _, small) = fractions[0];
    let (_, _, large) = fractions[1];
    assert!(
        small > 0.02 && large > 0.02,
        "the fold darkened {:.1}% of the form at ×0.01 and {:.1}% at ×100 — a \
         radius fixed in world units shades one of these and not the other",
        small * 100.0,
        large * 100.0
    );
    let (lo, hi) = if small < large {
        (small, large)
    } else {
        (large, small)
    };
    assert!(
        hi < lo * 2.0,
        "the same fold darkened {:.1}% of the form at one scale and {:.1}% at \
         the other, which is the scale showing through the shading",
        small * 100.0,
        large * 100.0
    );
}
