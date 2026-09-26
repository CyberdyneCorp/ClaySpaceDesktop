//! Every tool on the field's shelf is a different tool (#179, #203).
//!
//! Four SDF tools once shared one relief verb and three more shared two
//! others, so the shelf offered distinctions it did not make: Camada was
//! Padrão to the byte whenever Acumular was off, Inflar left a *taller* mark
//! than Padrão where an inflate is broader and lower, Planar and Polir were
//! the same bake, and so were Suavizar and Relaxar. Polir and Relaxar are off
//! the field's shelf now, with a note each; the rest are measured here against
//! each other, in world units, at identical settings.
//!
//! The measurement is the mark a stroke leaves across the top of the starting
//! sphere: the height the surface rose (or fell) straight under the stroke,
//! and how far to the side of the stroke it was disturbed at all.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ExecutionFamily, GestureSample, Representation, SculptModel, ToolKind, ToolNote,
    Unavailable,
};

fn sphere() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Where the surface stands, looking straight down at `(x, y)`.
fn top_at(document: &ClayDocument, x: f32, y: f32) -> f32 {
    SculptModel::pick(document, [x, y, 4.0], [0.0, 0.0, -1.0])
        .map(|hit| hit[2])
        .unwrap_or(f32::NAN)
}

/// A stroke along x over the top of the sphere, riding its surface.
fn along_the_top() -> Vec<GestureSample> {
    (0..=12)
        .map(|step| {
            let t = step as f32 / 12.0;
            let x = (t - 0.5) * 0.6;
            GestureSample {
                position: [x, 0.0, (1.0 - x * x).sqrt()],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// The field's default brush, with Acumular set as asked.
fn brush(accumulate: bool) -> BrushSettings {
    let mut brush = BrushSettings::default_for(Representation::Sdf);
    brush.shaping.accumulate = accumulate;
    brush
}

/// Where the probes stand across the stroke: straight under it, and out to
/// twice the brush to its side.
const ASIDE: [f32; 21] = {
    let mut aside = [0.0; 21];
    let mut i = 0;
    while i < aside.len() {
        aside[i] = i as f32 * 0.02;
        i += 1;
    }
    aside
};

/// How far the surface moved at each probe across the stroke.
#[derive(Debug, Clone)]
struct Mark(Vec<f32>);

impl Mark {
    fn of(tool: ToolKind, brush: BrushSettings) -> Self {
        let rest = sphere();
        let mut document = sphere();
        document
            .apply_stroke(tool, brush, &along_the_top(), [false; 3])
            .expect("the stroke was refused");
        Self(
            ASIDE
                .iter()
                .map(|&y| top_at(&document, 0.0, y) - top_at(&rest, 0.0, y))
                .collect(),
        )
    }

    /// How far the surface moved straight under the stroke.
    fn peak(&self) -> f32 {
        self.0[0]
    }

    /// How far to the side of the stroke the surface is still disturbed by
    /// more than a tenth of the brush's cell size.
    fn reach(&self) -> f32 {
        ASIDE
            .iter()
            .zip(&self.0)
            .filter(|(_, moved)| moved.abs() > 0.002)
            .map(|(aside, _)| *aside)
            .fold(0.0, f32::max)
    }

    /// The largest difference between two marks at any probe.
    fn distance(&self, other: &Self) -> f32 {
        self.0
            .iter()
            .zip(&other.0)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max)
    }

    /// The steepest the surface's change is between two neighbouring probes,
    /// as a rise over run.
    fn steepest(&self) -> f32 {
        self.0
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs() / 0.02)
            .fold(0.0, f32::max)
    }

    fn describe(&self) -> String {
        self.0
            .iter()
            .map(|moved| format!("{moved:+.3}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// How different two tools' marks must be to count as two tools: a cell of
/// the brick cache the viewport draws, which is the least a sculptor can see.
const DISTINCT: f32 = 0.01;

/// The field's stamping tools — the relief family the four duplicates came
/// from — read off the capability table rather than listed, so a tool bound
/// into that family later is measured here from the day it is.
fn the_stamping_tools() -> Vec<ToolKind> {
    ToolKind::for_representation(Representation::Sdf)
        .into_iter()
        .filter(|tool| {
            tool.binding_on(Representation::Sdf)
                .is_some_and(|binding| binding.family == ExecutionFamily::FieldCombineOp)
        })
        .collect()
}

#[test]
fn every_pair_of_field_stamps_leaves_a_different_mark() {
    let tools = the_stamping_tools();
    assert!(
        tools.len() >= 5,
        "the field's stamping family shrank to {tools:?}; the measurement \
         below is then proving less than it says"
    );
    for accumulate in [true, false] {
        let marks: Vec<(ToolKind, Mark)> = tools
            .iter()
            .map(|&tool| (tool, Mark::of(tool, brush(accumulate))))
            .collect();
        for (tool, mark) in &marks {
            println!(
                "acumular {accumulate:<5} {:<8} {}",
                tool.key(),
                mark.describe()
            );
        }
        for (i, (a, first)) in marks.iter().enumerate() {
            for (b, second) in marks.iter().skip(i + 1) {
                let apart = first.distance(second);
                assert!(
                    apart > DISTINCT,
                    "{} and {} leave the same mark on a field with Acumular \
                     {}: at most {apart:.4} apart, where two tools must differ \
                     by {DISTINCT}",
                    a.label(),
                    b.label(),
                    if accumulate { "on" } else { "off" }
                );
            }
        }
    }
}

#[test]
fn inflate_is_broader_and_lower_than_standard() {
    for accumulate in [true, false] {
        let standard = Mark::of(ToolKind::Padrao, brush(accumulate));
        let inflate = Mark::of(ToolKind::Inflar, brush(accumulate));
        println!(
            "acumular {accumulate}: padrão peak {:.4} reach {:.2}, inflar peak {:.4} reach {:.2}",
            standard.peak(),
            standard.reach(),
            inflate.peak(),
            inflate.reach()
        );
        assert!(
            inflate.peak() < standard.peak() - 0.005,
            "Inflar rose {:.4} under the stroke and Padrão {:.4}: an inflate \
             is the lower of the two",
            inflate.peak(),
            standard.peak()
        );
        assert!(
            inflate.reach() > standard.reach(),
            "Inflar reached {:.2} aside and Padrão {:.2}: an inflate is the \
             broader of the two",
            inflate.reach(),
            standard.reach()
        );
    }
}

#[test]
fn layer_is_a_bounded_course_below_standard() {
    // Camada's claim is the ceiling, so it is measured where Padrão is closest
    // to it: Acumular off clamps Padrão too, which is where the two used to be
    // one call.
    let layer_on = Mark::of(ToolKind::Camada, brush(true));
    let layer_off = Mark::of(ToolKind::Camada, brush(false));
    let clamped_standard = Mark::of(ToolKind::Padrao, brush(false));
    println!(
        "camada {:.4} / {:.4}, padrão clamped {:.4}",
        layer_on.peak(),
        layer_off.peak(),
        clamped_standard.peak()
    );
    assert!(
        layer_on.distance(&layer_off) < 1e-4,
        "Acumular changed what Camada does, so its clamp is the panel's and \
         not the tool's"
    );
    assert!(
        layer_on.peak() > 0.005,
        "Camada laid down nothing a sculptor can see: {:.4}",
        layer_on.peak()
    );
    assert!(
        layer_on.peak() < clamped_standard.peak() * 0.75,
        "Camada rose {:.4} and a clamped Padrão {:.4}: the layer is the \
         shallower course",
        layer_on.peak(),
        clamped_standard.peak()
    );
}

#[test]
fn planar_invert_lowers_the_surface() {
    let skim = Mark::of(ToolKind::Planar, brush(true));
    let carve = Mark::of(
        ToolKind::Planar,
        BrushSettings {
            invert: true,
            ..brush(true)
        },
    );
    println!("planar          {}", skim.describe());
    println!("planar inverted {}", carve.describe());
    assert!(
        skim.peak() < -0.002,
        "Planar did not cut the crown of the sphere: {:.4}",
        skim.peak()
    );
    // The defect: inverted, it raised a slab rather than cutting.
    let highest = carve.0.iter().copied().fold(f32::MIN, f32::max);
    assert!(
        highest < 0.002,
        "an inverted Planar raised the surface by {highest:.4} somewhere \
         across the stroke: {}",
        carve.describe()
    );
    assert!(
        carve.peak() < skim.peak() - 0.005,
        "inverted, Planar cut {:.4} and upright {:.4}: inverting sinks the \
         plane, so it cuts deeper",
        carve.peak(),
        skim.peak()
    );
}

#[test]
fn planar_leaves_no_ledge_at_the_edge_of_its_region() {
    // A ledge is a wall: the surface's change jumping by a whole slab between
    // two probes a cell apart. The sphere's own slope across these probes is
    // below one in two, and a cut that tapers over its falloff stays within
    // one in one.
    for invert in [false, true] {
        let mark = Mark::of(
            ToolKind::Planar,
            BrushSettings {
                invert,
                ..brush(true)
            },
        );
        assert!(
            mark.steepest() < 1.0,
            "Planar{} changes the surface by a rise of {:.2} over a run of \
             one between neighbouring probes — a ledge: {}",
            if invert { " inverted" } else { "" },
            mark.steepest(),
            mark.describe()
        );
    }
}

/// The two tools the field no longer offers, and where each one sends a
/// sculptor who asks for it anyway.
#[test]
fn a_field_offers_one_flatten_and_one_smooth_and_says_so() {
    for (tool, note, instead) in [
        (
            ToolKind::Polir,
            ToolNote::SdfPolishIsPlanar,
            ToolKind::Planar,
        ),
        (
            ToolKind::Relaxar,
            ToolNote::SdfRelaxIsSmooth,
            ToolKind::Suavizar,
        ),
    ] {
        assert!(
            !ToolKind::for_representation(Representation::Sdf).contains(&tool),
            "{} is back on the field's shelf",
            tool.label()
        );
        let mut document = sphere();
        let refused = document.apply_stroke(tool, brush(true), &along_the_top(), [false; 3]);
        assert!(
            refused.is_err(),
            "{} still strokes a field: {refused:?}",
            tool.label()
        );
        let Err(Unavailable::NoVerbHere { note: said, .. }) =
            tool.availability(clayspace_model::LayerState::editable(Representation::Sdf))
        else {
            panic!("{} is not refused as absent on a field", tool.label());
        };
        assert_eq!(said, Some(note), "{}", tool.label());
        assert_eq!(tool.substitute_on(Representation::Sdf), instead);
    }
}

// -- Planar, while it is made --------------------------------------------------

/// The starting form's own symmetry, so opening a gesture does not point the
/// layer's mirror — an edit of its own, and not what these tests are about.
const STARTING_SYMMETRY: [bool; 3] = [true, false, false];

/// How high the *drawn* surface stands over the crown of the sphere: what the
/// viewport shows, which during a live gesture is not the document.
fn drawn_crown(document: &ClayDocument) -> f32 {
    let (cache, offset) = document.drawn_cache();
    let keys = cache.surface_bricks().expect("surface bricks");
    let live = document.live_gesture_is_open();
    let (mesh, _) = cache
        .mesh(
            (!live).then(|| document.document()),
            clayspace_engine::claycore::BrickMeshParams {
                gradient_normals: false,
                colors: false,
                gradient_eps: None,
            },
            &keys,
        )
        .expect("mesh the drawn surface");
    mesh.positions()
        .iter()
        .map(|p| [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]])
        .filter(|v| v[0].abs() < 0.04 && v[1].abs() < 0.04)
        .fold(f32::NEG_INFINITY, |top, v| top.max(v[2]))
}

/// How high the *document* stands over the crown, marched against the field
/// itself rather than read from the brick cache a pick reads.
fn document_crown(document: &ClayDocument) -> f32 {
    document
        .document()
        .raycast([0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("a raycast")
        .map(|hit| hit.position[2])
        .unwrap_or(f32::NAN)
}

/// The stroke sent the way the ViewModel sends a live one: in pieces, each
/// holding only what is new.
fn in_segments(document: &mut ClayDocument, brush: BrushSettings) {
    for piece in along_the_top().chunks(4) {
        let depth = document.history().depth;
        document
            .apply_stroke(ToolKind::Planar, brush, piece, STARTING_SYMMETRY)
            .expect("a live segment");
        assert_eq!(
            document.history().depth,
            depth,
            "a preview segment left an entry in the history, which the \
             ViewModel would count as the gesture's"
        );
    }
}

#[test]
fn planar_shows_itself_while_the_stroke_is_made() {
    // Deep enough to see on the drawn surface: the inverted cut.
    let carve = BrushSettings {
        invert: true,
        ..brush(true)
    };
    let untouched = sphere();
    let resting = drawn_crown(&untouched);
    let mut document = sphere();
    let depth = document.history().depth;
    assert!(
        document.open_live_gesture(ToolKind::Planar, STARTING_SYMMETRY),
        "an editable field is exactly where Planar can be previewed"
    );
    in_segments(&mut document, carve);

    let shown = drawn_crown(&document);
    println!("crown resting {resting:.4}, shown mid-stroke {shown:.4}");
    assert!(
        shown < resting - 0.02,
        "nothing was drawn while the stroke was made: the crown stood at \
         {shown:.4} against {resting:.4} untouched"
    );
    assert!(
        (document_crown(&document) - document_crown(&untouched)).abs() < 1e-4,
        "the preview was left in the document rather than taken back"
    );

    let recorded = document.close_live_gesture().expect("the release");
    assert!(recorded > 0, "the release recorded nothing");
    assert!(document.history().depth > depth);

    // What lands is what a held gesture lays down, and what was shown.
    let mut held = sphere();
    held.apply_stroke(ToolKind::Planar, carve, &along_the_top(), STARTING_SYMMETRY)
        .expect("the held stroke");
    let (landed, expected) = (top_at(&document, 0.0, 0.0), top_at(&held, 0.0, 0.0));
    println!(
        "landed {landed:.4}, held {expected:.4}, drawn after {:.4}",
        drawn_crown(&document)
    );
    assert!(
        (landed - expected).abs() < 1e-4,
        "the live gesture landed at {landed:.4} and the held one at {expected:.4}"
    );
    assert!(
        (drawn_crown(&document) - shown).abs() < 0.01,
        "the release moved the surface from what the last preview showed"
    );
}

#[test]
fn an_abandoned_planar_leaves_the_surface_as_it_was() {
    let resting = drawn_crown(&sphere());
    let mut document = sphere();
    let depth = document.history().depth;
    assert!(document.open_live_gesture(ToolKind::Planar, STARTING_SYMMETRY));
    in_segments(
        &mut document,
        BrushSettings {
            invert: true,
            ..brush(true)
        },
    );
    assert_eq!(document.discard_live_gesture(), 0);
    assert!(!document.live_gesture_is_open());
    assert_eq!(document.history().depth, depth);
    let after = drawn_crown(&document);
    assert!(
        (after - resting).abs() < 1e-3,
        "the abandoned cut is still drawn: the crown stands at {after:.4} \
         against {resting:.4}"
    );
}

#[test]
fn planar_is_held_where_it_cannot_be_previewed() {
    // A grid's Planar is a stamp and not a bake, and a protected field takes
    // no stroke at all: both keep the path they always took.
    let mut document = sphere();
    document
        .add_voxel_layer("Grade", 0.02)
        .expect("a grid layer");
    assert!(!document.open_live_gesture(ToolKind::Planar, STARTING_SYMMETRY));
}
