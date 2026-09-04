//! Turning a stamp about its own facing, from the brush panel to the mesh.
//!
//! The engine has carried a `stamp_azimuth` since ABI minor 74 and this
//! application sent a hard zero into it: every stamp landed at the same
//! world-fixed orientation whichever way the sculptor meant it. What that
//! costs is one number rather than one feature — a rake, a chisel, clay strips
//! and a turned alpha are the same brush at four angles — so the field is
//! wired here from `Shaping::azimuth` and measured on the surface it moved.
//!
//! **The angle is measured at a quarter turn and never at zero**, and that is
//! the whole method rather than an incidental choice. Upstream's own round
//! trip could not catch this field being dropped, because every preset in
//! their reference set carried an azimuth of zero — and zero survives a
//! schema that has never heard of the field exactly as well as one that has.
//! A test written at the default value tests the default, not the field.
//!
//! **And the stamp has to be one that can be turned.** A round footprint looks
//! the same at every angle by construction, and so does a radially symmetric
//! alpha — the rings the other alpha tests use would pass this file at every
//! azimuth and prove nothing. The stamp below is stripes: it varies along one
//! of its axes and is constant along the other, so a quarter turn is the
//! largest change it can express.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    Alpha, BrushSettings, Direction, GestureSample, SculptModel, Shaping, ToolKind,
};

/// A stamp that has a direction: stripes across `u`, constant along `v`.
fn stripes(side: u32) -> Alpha {
    let samples = (0..side)
        .flat_map(|_| {
            (0..side).map(move |x| {
                let u = x as f32 / (side - 1) as f32;
                // Four bands over the stamp's width, hard-edged, so a turn
                // moves material rather than blurring it.
                if ((u * 4.0) as u32) % 2 == 0 {
                    1.0
                } else {
                    0.0
                }
            })
        })
        .collect();
    Alpha {
        name: "faixas".into(),
        width: side,
        height: side,
        samples,
    }
}

/// One stroke over a converted mesh, at the grain the caller names.
///
/// Every run starts from its own document, so the two results differ by the
/// angle and by nothing that accumulated.
fn stroked_at(azimuth: f32) -> Vec<[f32; 3]> {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .expect("cross the starting form into a mesh");
    document.set_alpha(Some(stripes(64)));

    let brush = BrushSettings {
        size: 0.35,
        intensity: 1.0,
        alpha: true,
        shaping: Shaping {
            azimuth,
            ..Shaping::default()
        },
        ..BrushSettings::default()
    };
    let samples: Vec<GestureSample> = (0..6)
        .map(|i| {
            let t = i as f32 / 5.0;
            GestureSample {
                position: [(t - 0.5) * 0.3, 0.0, 1.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    document
        .apply_stroke(ToolKind::Padrao, brush, &samples, [false; 3])
        .expect("the stroke was refused");

    document.visible_mesh_geometry().0
}

/// The measurement: a quarter turn moves the same stroke's material somewhere
/// else. If the azimuth were dropped anywhere between the brush panel and
/// `clay_mesh_brush_desc` these two would be identical, which is exactly the
/// silence this exists to break.
#[test]
fn a_quarter_turn_lands_the_stamp_somewhere_else() {
    let upright = stroked_at(0.0);
    let turned = stroked_at(std::f32::consts::FRAC_PI_2);

    assert_eq!(
        upright.len(),
        turned.len(),
        "a stroke changed the topology, so the two runs are not comparable"
    );
    assert!(!upright.is_empty(), "the fixture carries no mesh");

    let moved = upright.iter().zip(&turned).filter(|(a, b)| a != b).count();
    assert!(
        moved > 0,
        "{} vertices, not one of them placed differently by a quarter turn: \
         the grain is being dropped somewhere between Shaping::azimuth and the \
         brush descriptor",
        upright.len()
    );
}

/// And the control: the same stroke twice at the same angle is the same mesh.
///
/// Without this the test above passes on any source of noise — a jittered
/// stroke, a non-deterministic mesher — and would go on passing with the
/// azimuth wired to nothing at all.
#[test]
fn the_same_grain_twice_is_the_same_mesh() {
    assert_eq!(
        stroked_at(std::f32::consts::FRAC_PI_2),
        stroked_at(std::f32::consts::FRAC_PI_2),
        "the same stroke at the same grain came out differently, so the \
         comparison beside this one is measuring noise"
    );
}
