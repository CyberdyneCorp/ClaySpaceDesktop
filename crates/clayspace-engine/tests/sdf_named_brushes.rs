//! Argila and Vinco on a field, and Mover Topológico beside Mover.
//!
//! Three brushes the engine has had all along and the shelf did not offer on
//! an SDF layer. All three are measured against a *neighbouring* brush rather
//! than against zero, because "it changed something" is what every one of them
//! did before they were bound properly — the question is whether they changed
//! something the neighbour does not.
//!
//! - **Argila** is relief with buildup, so what separates it from Padrão is
//!   accumulation: crossing a stroke over itself has to add.
//! - **Vinco** is incise, so what separates it from a subtracting Padrão is
//!   that it displaces the accumulated field rather than combining a sphere
//!   with it — and that it cuts a *narrow* trough.
//! - **Mover Topológico** measures its reach along the material, so what
//!   separates it from Mover is a form whose parts are close in space and far
//!   along the surface.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, GestureSample, ObjectModel, Representation, SculptModel, ToolKind,
};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// How far the surface stands from the centre along a direction.
fn reach(document: &ClayDocument, direction: [f32; 3]) -> f32 {
    let length = direction.iter().map(|c| c * c).sum::<f32>().sqrt();
    let unit = direction.map(|c| c / length);
    SculptModel::pick(document, unit.map(|c| c * 4.0), unit.map(|c| -c))
        .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
        .unwrap_or(f32::NAN)
}

/// A short stroke across the top of the sphere, along x.
fn across(document: &mut ClayDocument, tool: ToolKind, invert: bool) -> bool {
    at_intensity(document, tool, invert, 1.0)
}

/// The same, at a stated intensity.
///
/// The buildup tests need one well below full: the engine saturates a relief
/// amplitude at roughly the brush radius, so at full strength a single pass
/// already reaches the ceiling and a second adds nothing — which measures the
/// saturation rather than the accumulation.
fn at_intensity(document: &mut ClayDocument, tool: ToolKind, invert: bool, intensity: f32) -> bool {
    let samples: Vec<GestureSample> = (0..=8)
        .map(|step| {
            let t = step as f32 / 8.0;
            GestureSample {
                position: [(t - 0.5) * 0.4, 0.0, 1.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            tool,
            BrushSettings {
                size: 0.2,
                intensity,
                invert,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("the stroke was refused")
        .changed
}

/// Well below full, so a stamp has room to accumulate before the engine's own
/// amplitude ceiling is reached.
const GENTLE: f32 = 0.3;

/// A probe to one side of the stroke, in the band between the two brushes'
/// footprints: the crease reaches 0.35 of the brush and Padrão the whole of it.
const BESIDE: [f32; 3] = [0.0, 0.1, 1.0];

/// The height above the sphere directly under the stroke, in units of the
/// starting radius.
fn under_the_stroke(document: &ClayDocument) -> f32 {
    reach(document, [0.0, 0.0, 1.0])
}

#[test]
fn the_shelf_offers_the_three_on_a_field() {
    let offered = ToolKind::for_representation(Representation::Sdf);
    for tool in [ToolKind::Argila, ToolKind::Vinco, ToolKind::MoverTopologico] {
        assert!(
            offered.contains(&tool),
            "{tool:?} is not offered on a field"
        );
    }
}

#[test]
fn every_tool_the_table_offers_on_a_field_lands() {
    // The table is the single source of truth for where a tool applies, so a
    // row added without a binding has to fail here rather than in front of a
    // sculptor. Trim and Máscara are excluded: one is drawn on the view frame
    // rather than stroked across the surface, and the other paints a freeze
    // instead of moving anything.
    for tool in ToolKind::for_representation(Representation::Sdf) {
        if !tool.is_stroke_tool() || tool.is_mask_tool() {
            continue;
        }
        let mut document = sphere();
        assert!(
            across(&mut document, tool, false),
            "{} is offered on a field and changed nothing",
            tool.label()
        );
    }
}

#[test]
fn clay_builds_up_where_a_stroke_crosses_itself() {
    // Buildup is what separates Argila from Padrão on a field: both are
    // relief, and only one accumulates. Two passes over the same ground.
    let mut clay = sphere();
    let mut layer = sphere();
    for _ in 0..2 {
        assert!(at_intensity(&mut clay, ToolKind::Argila, false, GENTLE));
        assert!(at_intensity(&mut layer, ToolKind::Camada, false, GENTLE));
    }
    let (built, clamped) = (under_the_stroke(&clay), under_the_stroke(&layer));
    println!("argila {built:.4}  camada {clamped:.4}");
    assert!(
        built > clamped + 0.01,
        "two passes of Argila reached {built} and two of Camada {clamped}, so \
         the clamped tool accumulated as much as the buildup one"
    );
}

#[test]
fn one_pass_of_clay_is_lower_than_two() {
    let mut once = sphere();
    let mut twice = sphere();
    at_intensity(&mut once, ToolKind::Argila, false, GENTLE);
    at_intensity(&mut twice, ToolKind::Argila, false, GENTLE);
    at_intensity(&mut twice, ToolKind::Argila, false, GENTLE);
    let (a, b) = (under_the_stroke(&once), under_the_stroke(&twice));
    println!("argila once {a:.4}  twice {b:.4}");
    assert!(
        b > a + 0.005,
        "a second pass of Argila added {:.4}, which is not a buildup",
        b - a
    );
}

#[test]
fn crease_cuts_a_trough() {
    let mut document = sphere();
    let before = under_the_stroke(&document);
    assert!(across(&mut document, ToolKind::Vinco, false));
    let after = under_the_stroke(&document);
    println!("vinco {before:.4} -> {after:.4}");
    assert!(
        after < before - 0.002,
        "Vinco raised the surface from {before} to {after} rather than cutting"
    );
}

#[test]
fn crease_is_the_incise_whatever_the_panel_is_set_to() {
    // A named brush *is* its operation. Vinco with the Combinar panel set to
    // Unir is still a crease — otherwise the tool would be a label over
    // whatever the panel happened to hold, which is what "no orphan tools"
    // exists to prevent one layer up.
    let cut = |op| {
        let mut document = sphere();
        document.set_combine(clayspace_model::CombineSettings {
            op,
            ..clayspace_model::CombineSettings::for_strokes()
        });
        across(&mut document, ToolKind::Vinco, false);
        under_the_stroke(&document)
    };
    let (union, subtract) = (
        cut(clayspace_model::Combine::Add),
        cut(clayspace_model::Combine::Subtract),
    );
    let rest = under_the_stroke(&sphere());
    println!("vinco under Unir {union:.4}  under Subtrair {subtract:.4}  rest {rest:.4}");
    assert!(
        (union - subtract).abs() < 1e-4,
        "the panel changed what Vinco does: {union} against {subtract}"
    );
    assert!(
        union < rest,
        "Vinco stopped cutting when the panel was set to Unir"
    );
}

#[test]
fn crease_is_narrower_than_the_same_stroke_with_the_standard_brush() {
    // "A thin region gives the line", in the engine's words. Measured as how
    // far to the side of the stroke the surface is still disturbed.
    // Just outside the crease's own narrow region and well inside Padrão's,
    // which is the band where "narrower" is a difference rather than a
    // rounding error.
    let mut crease = sphere();
    let mut standard = sphere();
    across(&mut crease, ToolKind::Vinco, false);
    across(&mut standard, ToolKind::Padrao, true);
    let rest = reach(&sphere(), BESIDE);
    let (aside_crease, aside_standard) = (reach(&crease, BESIDE), reach(&standard, BESIDE));
    println!(
        "beside the stroke: rest {rest:.4} vinco {aside_crease:.4} padrao {aside_standard:.4}"
    );
    assert!(
        (aside_crease - rest).abs() < (aside_standard - rest).abs(),
        "Vinco disturbed the surface beside the stroke by {:.4} and an \
         inverted Padrão by {:.4}, so the crease is not the narrower mark",
        (aside_crease - rest).abs(),
        (aside_standard - rest).abs()
    );
}

#[test]
fn crease_inverted_raises_the_ridge_it_would_have_cut() {
    // The engine names relief and incise as inverses, and `Combine::inverted`
    // already agrees; this is that pair reaching the tool.
    let mut cut = sphere();
    let mut raised = sphere();
    across(&mut cut, ToolKind::Vinco, false);
    across(&mut raised, ToolKind::Vinco, true);
    let rest = under_the_stroke(&sphere());
    let (down, up) = (under_the_stroke(&cut), under_the_stroke(&raised));
    println!("vinco {down:.4}  rest {rest:.4}  vinco inverted {up:.4}");
    assert!(
        down < rest && up > rest,
        "the pair did not straddle the untouched surface: {down} / {rest} / {up}"
    );
}

/// A horseshoe: two tips close in space, joined only through the bend.
///
/// The engine's own fixture for this verb, in its own words — "two fingers 0.32
/// apart joined only through a palm". Five overlapping balls in a U, so the
/// tips are 0.1 apart across the opening and about 1.3 apart along the
/// material.
fn horseshoe() -> ClayDocument {
    const CENTRES: [[f32; 3]; 5] = [
        // The two tips.
        [-0.25, 0.0, 0.6],
        [0.25, 0.0, 0.6],
        // The stems they stand on.
        [-0.25, 0.0, 0.2],
        [0.25, 0.0, 0.2],
        // The bend joining the stems.
        [0.0, 0.0, 0.0],
    ];
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    for centre in CENTRES {
        document
            .place_object(
                clayspace_model::Shape::Sphere,
                &[0.2],
                centre,
                clayspace_model::CombineSettings::default(),
            )
            .expect("place a ball");
    }
    document
}

/// Where the surface stands, looking straight down at `x`.
fn top_at(document: &ClayDocument, x: f32) -> f32 {
    SculptModel::pick(document, [x, 0.0, 4.0], [0.0, 0.0, -1.0])
        .map(|hit| hit[2])
        .unwrap_or(f32::NAN)
}

/// Lifts the left tip straight up, and reports how far each tip came.
fn lift_the_left_tip(tool: ToolKind) -> (f32, f32) {
    let mut document = horseshoe();
    let (near, far) = (top_at(&document, -0.25), top_at(&document, 0.25));
    let samples: Vec<GestureSample> = (0..=4)
        .map(|step| {
            let t = step as f32 / 4.0;
            GestureSample {
                position: [-0.25, 0.0, 0.8 + t * 0.3],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(
            tool,
            BrushSettings {
                // Wide enough to span the 0.5 between the tips in *space* and
                // nowhere near the 1.3 between them through the bend, which is
                // the whole of what makes the two verbs disagree.
                size: 1.0,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("the drag was refused");
    (
        top_at(&document, -0.25) - near,
        top_at(&document, 0.25) - far,
    )
}

#[test]
fn a_topological_drag_leaves_behind_what_a_euclidean_one_carries() {
    // Measured on the horseshoe with a brush of 1.0 and a drag of 0.3:
    //
    //   verb        near tip   far tip
    //   Mover        +0.167     +0.089
    //   Topológico   +0.295     +0.000
    //
    // The far tip is 0.5 away in space and about 1.3 away through the material,
    // which is why one verb carries it and the other does not.
    let (near_euclidean, far_euclidean) = lift_the_left_tip(ToolKind::Mover);
    let (near_topological, far_topological) = lift_the_left_tip(ToolKind::MoverTopologico);
    println!(
        "near: mover {near_euclidean:.4} topológico {near_topological:.4}\n\
         far:  mover {far_euclidean:.4} topológico {far_topological:.4}"
    );
    assert!(
        near_topological > 0.05,
        "the topological drag did not move the tip it was anchored on: \
         {near_topological:.4}"
    );
    assert!(
        far_euclidean > 0.03,
        "the Euclidean drag left the far tip alone too, so the fixture proves \
         nothing: it rose {far_euclidean:.4}"
    );
    assert!(
        far_topological < far_euclidean * 0.5,
        "the topological drag carried the far tip {far_topological:.4} against \
         the Euclidean {far_euclidean:.4}, so the reach is not being measured \
         along the material"
    );
}
