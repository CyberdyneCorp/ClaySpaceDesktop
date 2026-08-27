//! Symmetry on a mesh layer.
//!
//! It did nothing at all. `apply_stroke` takes the enabled axes and the mesh
//! arm of its dispatch did not pass them on — `stroke_mesh` was not even
//! *given* them, so every X, Y and Z button in the interface was inert on a
//! mesh while working on a field.
//!
//! There is no engine-side mesh symmetry to reach for: `clay_set_layer_mirror`
//! reflects a layer's *items*, and a mesh layer has vertices instead. Both
//! references do the same thing in that position — mirror the stroke and apply
//! it again — and this is measured against Blender 5.2 doing exactly that on a
//! 64×32 UV sphere:
//!
//! ```text
//!   symmetry   +x    -x    +y    -y     max displacement
//!   none       82     0    78     0     0.18306
//!   x          82    82   156     0     0.18306
//!   x, y      161   161   156   156     0.16893
//! ```
//!
//! One dab per reflection, at full strength, and two axes giving four dabs
//! rather than two.
//!
//! The counts are not the assertion here, and that is worth saying: our mesh
//! comes from marching cubes, whose vertex density is not the same on both
//! sides of a plane — measured, a lone dab moves 497 vertices at one place and
//! 272 at its mirror. Blender's UV sphere is symmetric by construction. What a
//! sculptor means by symmetry is that *the form* comes out symmetric, so that
//! is what these measure: the surface itself, at mirrored places.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{BrushSettings, Direction, GestureSample, SculptModel, ToolKind};

fn meshed() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    document
        .convert_layer(Direction::SdfToMesh, 0.04, 0)
        .expect("cross to a mesh");
    document
}

/// How far the surface stands from the centre along a direction.
///
/// Measured rather than counted: a marching-cubes mesh has different vertex
/// densities on either side of a plane, so a count of what moved says more
/// about the tessellation than about the stroke.
fn reach(document: &ClayDocument, direction: [f32; 3]) -> f32 {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    SculptModel::pick(document, unit.map(|c| c * 4.0), unit.map(|c| -c))
        .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
        .unwrap_or(f32::NAN)
}

/// The place the dab lands, and its reflections.
const AT: [f32; 3] = [0.575, 0.366, 0.732];

fn dab(document: &mut ClayDocument, tool: ToolKind, symmetry: [bool; 3]) {
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size: 0.25,
                intensity: 0.9,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: AT,
                pressure: 1.0,
                time: 0.0,
            }],
            symmetry,
        )
        .expect("the stroke was refused");
}

#[test]
fn symmetry_off_leaves_the_other_side_alone() {
    // The control. Without this the test below passes on a brush that reaches
    // the whole model.
    let mut document = meshed();
    let rest = reach(&document, [-AT[0], AT[1], AT[2]]);
    dab(&mut document, ToolKind::Padrao, [false; 3]);

    assert!(
        reach(&document, AT) > 1.02,
        "the dab itself did nothing, so there is nothing to mirror"
    );
    assert!(
        (reach(&document, [-AT[0], AT[1], AT[2]]) - rest).abs() < 1e-3,
        "a stroke with no symmetry reached the other side of the model"
    );
}

#[test]
fn x_symmetry_puts_the_same_form_on_the_other_side() {
    // The whole of what a sculptor means by symmetry: the other side comes out
    // the same. This was inert — the mesh arm of the dispatch dropped the axes
    // before `stroke_mesh` ever saw them.
    let mut document = meshed();
    dab(&mut document, ToolKind::Padrao, [true, false, false]);

    let here = reach(&document, AT);
    let there = reach(&document, [-AT[0], AT[1], AT[2]]);
    assert!(here > 1.02, "the dab did nothing");
    assert!(
        (here - there).abs() < 0.01,
        "the form stands {here} where the dab landed and {there} at its \
         mirror, so the X button does nothing on a mesh"
    );
    // And only there: y and z are off, so the quadrant below is untouched.
    let below = reach(&document, [AT[0], -AT[1], AT[2]]);
    assert!(
        below < here - 0.01,
        "x symmetry alone reached the -y side as well, at {below} against \
         {here}"
    );
}

#[test]
fn two_axes_give_four_dabs_and_three_give_eight() {
    // The subset lattice, which is what both references do and what a sculptor
    // means by "symmetric in x and y" — the four quadrants, not the two halves
    // twice. Measured in Blender: one dab moves 82 vertices, x symmetry moves
    // 82 on each side, and x with y moves 161 in each of four quadrants.
    let mut document = meshed();
    dab(&mut document, ToolKind::Padrao, [true, true, false]);

    let quadrants = [
        reach(&document, [AT[0], AT[1], AT[2]]),
        reach(&document, [-AT[0], AT[1], AT[2]]),
        reach(&document, [AT[0], -AT[1], AT[2]]),
        reach(&document, [-AT[0], -AT[1], AT[2]]),
    ];
    let worst = quadrants.iter().fold(f32::MIN, |worst, r| worst.max(*r));
    let least = quadrants.iter().fold(f32::MAX, |least, r| least.min(*r));
    assert!(
        worst > 1.02,
        "no quadrant was reached at all: {quadrants:?}"
    );
    assert!(
        worst - least < 0.01,
        "the four quadrants stand at {quadrants:?}, which is not four copies \
         of one dab"
    );

    // And with all three, the back of the model too.
    let mut all = meshed();
    dab(&mut all, ToolKind::Padrao, [true; 3]);
    let behind = reach(&all, [AT[0], AT[1], -AT[2]]);
    assert!(
        (behind - reach(&all, AT)).abs() < 0.01,
        "z symmetry left the back of the model at {behind}"
    );
}

#[test]
fn a_mirrored_drag_pulls_the_other_side_the_mirrored_way() {
    // A reflection turns a direction over as well as a position, and
    // forgetting that is the bug that makes a mirrored Grab pull the wrong
    // way: both sides would travel the same way in world space instead of
    // moving as a pair.
    let mut document = meshed();
    let before = document.visible_mesh_geometry().0;
    let anchor =
        SculptModel::pick(&document, [0.3, 0.0, 4.0], [0.0, 0.0, -1.0]).expect("the near face");
    let path: Vec<GestureSample> = (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                // Dragged outward along +x from a point on the near face.
                position: [anchor[0] + t * 0.5, anchor[1], anchor[2]],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Mover,
            BrushSettings {
                size: 0.3,
                intensity: 0.9,
                ..BrushSettings::default()
            },
            &path,
            [true, false, false],
        )
        .expect("the drag was refused");
    let after = document.visible_mesh_geometry().0;

    // Where each half went on average. Measured as a displacement rather than
    // as a silhouette: this drag carries material near the pole sideways, and
    // the model's widest point never moves.
    let mean = |side: fn(f32) -> bool| {
        let mut sum = [0.0f64; 3];
        let mut count = 0u32;
        for (was, now) in before.iter().zip(&after) {
            let moved: [f32; 3] = std::array::from_fn(|axis| now[axis] - was[axis]);
            if !side(was[0]) || moved.iter().all(|c| c.abs() < 1e-5) {
                continue;
            }
            for axis in 0..3 {
                sum[axis] += moved[axis] as f64;
            }
            count += 1;
        }
        (count, sum.map(|total| (total / count.max(1) as f64) as f32))
    };
    let (right_count, right) = mean(|x| x > 0.05);
    let (left_count, left) = mean(|x| x < -0.05);

    assert!(
        right_count > 50 && left_count > 50,
        "the drag moved {right_count} vertices on the right and {left_count} \
         on the left, so there is nothing to compare"
    );
    assert!(
        right[0] > 0.01,
        "the drag did not carry the right side along +x: {right:?}"
    );
    assert!(
        (right[0] + left[0]).abs() < right[0] * 0.25,
        "the right side travelled {:.4} along x and the left {:.4}. A \
         mirrored drag whose direction was not turned over sends both the same \
         way — one out and one into the model",
        right[0],
        left[0]
    );
    for axis in [1, 2] {
        assert!(
            (right[axis] - left[axis]).abs() < 0.01,
            "the two sides disagree off the mirrored axis: {right:?} against \
             {left:?}"
        );
    }
}

#[test]
fn a_symmetric_stroke_is_still_one_undo() {
    // Every reflection goes into the same set of deltas, because a sculptor
    // made one stroke — however many copies of it the axes called for.
    let mut document = meshed();
    let before = document.history().depth;
    let rest = reach(&document, AT);
    dab(&mut document, ToolKind::Padrao, [true, true, true]);

    assert_eq!(
        document.history().depth,
        before + 1,
        "a stroke mirrored eight ways left more than one entry on the stack"
    );
    document.undo().expect("undo");
    assert!(
        (reach(&document, AT) - rest).abs() < 1e-3,
        "one undo did not take the whole symmetric stroke back"
    );
}

// -- and a grid --------------------------------------------------------------

#[test]
fn symmetry_reaches_a_voxel_layer_too() {
    // The same defect one representation over: the voxel arm of the dispatch
    // dropped the axes as well, so the X, Y and Z buttons were inert on a grid
    // for the same reason they were on a mesh. A grid has no layer mirror
    // either — the mirror plane is the one its cell lattice already puts at
    // coordinate zero.
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document.add_voxel_layer("Voxels", 0.05).expect("a grid");

    let brush = BrushSettings {
        size: 0.25,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    let at = [0.4, 0.0, 0.0];
    document
        .apply_stroke(
            ToolKind::Padrao,
            brush,
            &[GestureSample {
                position: at,
                pressure: 1.0,
                time: 0.0,
            }],
            [true, false, false],
        )
        .expect("the stroke was refused");

    let (positions, ..) = document.visible_mesh_geometry();
    assert!(!positions.is_empty(), "nothing was deposited at all");
    let right = positions.iter().filter(|v| v[0] > 0.05).count();
    let left = positions.iter().filter(|v| v[0] < -0.05).count();
    assert!(
        right > 0 && left > 0,
        "the deposit reached {right} vertices on the right and {left} on the \
         left, so a grid still ignores its symmetry axes"
    );
    // Where the material actually reaches, rather than how many vertices the
    // mesher made of it. The counts are close but not equal — 164 against 152
    // measured — because the greedy mesher merges quads differently on either
    // side of the seam at x = 0. That is the mesher's business; the deposit
    // is what symmetry is about.
    let far = positions.iter().map(|v| v[0]).fold(f32::MIN, f32::max);
    let near = positions.iter().map(|v| v[0]).fold(f32::MAX, f32::min);
    assert!(
        (far + near).abs() < 0.06,
        "the deposit reaches {far} on the right and {near} on the left, which \
         is not one deposit and its reflection"
    );
}
