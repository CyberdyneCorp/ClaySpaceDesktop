//! What a mask is for: a frozen region that resists every verb.
//!
//! The contract is not "most tools respect the mask". A sculptor who freezes a
//! finger and then reaches for whichever brush is to hand expects the finger to
//! survive all of them, and a single tool that ignores the mask makes the
//! feature untrustworthy — worse than absent, because it invites reliance.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ExtrudeSettings, GestureSample, MaskModel, MaskOp, SceneModel, SculptModel,
    ToolKind,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Radius of the surface along a direction — the fingerprint used throughout.
fn radius_along(document: &ClayDocument, direction: [f32; 3]) -> Option<f32> {
    let n =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
            .sqrt();
    let unit = direction.map(|c| c / n);
    document
        .pick(unit.map(|c| c * 4.0), unit.map(|c| -c))
        .map(|hit| (hit[0] * hit[0] + hit[1] * hit[1] + hit[2] * hit[2]).sqrt())
}

/// Paints the mask over a spot, generously enough to cover a brush there.
fn freeze(document: &mut ClayDocument, at: [f32; 3]) {
    let brush = BrushSettings {
        size: 0.4,
        intensity: 1.0,
        ..BrushSettings::default()
    };
    let samples: Vec<GestureSample> = (0..4)
        .map(|i| GestureSample {
            position: at,
            pressure: 1.0,
            time: i as f32 * 0.01,
        })
        .collect();
    document
        .apply_stroke(ToolKind::Mascara, brush, &samples, [false; 3])
        .expect("paint the mask");
}

/// Every tool that can act on an SDF layer through a stroke.
fn sdf_tools() -> Vec<ToolKind> {
    ToolKind::ALL
        .into_iter()
        .filter(|tool| {
            *tool != ToolKind::Mascara
                && tool
                    .availability(clayspace_model::LayerState::editable(
                        clayspace_model::Representation::Sdf,
                    ))
                    .is_ok()
        })
        .collect()
}

#[test]
fn a_frozen_region_resists_every_tool_that_can_reach_it() {
    let mut reference = document();
    let at = [0.0f32, 0.0, 1.0];

    // What each tool does to an *unmasked* surface, so a tool that changes
    // nothing anyway cannot be mistaken for one the mask stopped.
    let mut moves = Vec::new();
    for tool in sdf_tools() {
        let mut document = document();
        let before = radius_along(&document, at).expect("surface");
        let samples: Vec<GestureSample> = (0..6)
            .map(|i| GestureSample {
                position: at,
                pressure: 1.0,
                time: i as f32 * 0.01,
            })
            .collect();
        let _ = document.apply_stroke(tool, BrushSettings::default(), &samples, [false; 3]);
        let after = radius_along(&document, at).expect("surface");
        moves.push((tool, (after - before).abs()));
    }
    let _ = &mut reference;

    // Now the same strokes with the region frozen.
    let mut ignored = Vec::new();
    for (tool, unmasked_move) in &moves {
        if *unmasked_move <= 0.002 {
            // It does nothing here masked or not; it proves nothing either way.
            continue;
        }
        let mut document = document();
        freeze(&mut document, at);
        assert!(
            document.mask_state().is_active(),
            "the mask did not take before testing {tool:?}"
        );

        let before = radius_along(&document, at).expect("surface");
        let samples: Vec<GestureSample> = (0..6)
            .map(|i| GestureSample {
                position: at,
                pressure: 1.0,
                time: i as f32 * 0.01,
            })
            .collect();
        let _ = document.apply_stroke(*tool, BrushSettings::default(), &samples, [false; 3]);
        let after = radius_along(&document, at).expect("surface");
        let masked_move = (after - before).abs();

        if masked_move > unmasked_move * 0.25 {
            ignored.push(format!(
                "{tool:?} moved {masked_move:.4} through the mask against \
                 {unmasked_move:.4} without it"
            ));
        }
    }

    assert!(
        ignored.is_empty(),
        "these tools edited a frozen region: {ignored:?}. A mask that most \
         tools respect is worse than none, because it invites reliance."
    );
}

#[test]
fn painting_the_mask_does_not_move_the_surface() {
    let mut document = document();
    let at = [0.0f32, 0.0, 1.0];
    let before = radius_along(&document, at).expect("surface");
    freeze(&mut document, at);
    let after = radius_along(&document, at).expect("surface");
    assert!(
        (before - after).abs() < 1e-4,
        "painting a mask moved the surface from {before} to {after}"
    );
    assert!(document.mask_state().is_active());
}

#[test]
fn the_mask_operations_do_what_they_are_called() {
    let mut document = document();
    freeze(&mut document, [0.0, 0.0, 1.0]);
    let painted = document.mask_state().painted_cells;
    assert!(painted > 0);

    document.apply_mask_op(MaskOp::Expand(2)).expect("expand");
    let expanded = document.mask_state().painted_cells;
    assert!(
        expanded > painted,
        "expand left {expanded} cells against {painted}"
    );

    document
        .apply_mask_op(MaskOp::Contract(2))
        .expect("contract");
    let contracted = document.mask_state().painted_cells;
    assert!(
        contracted < expanded,
        "contract left {contracted} cells against {expanded}"
    );

    document.apply_mask_op(MaskOp::Smooth(1)).expect("smooth");
    document.apply_mask_op(MaskOp::Invert).expect("invert");
    document
        .apply_mask_op(MaskOp::InvertWithinBounds)
        .expect("bounded complement");

    document.apply_mask_op(MaskOp::Clear).expect("clear");
    assert!(
        !document.mask_state().is_active(),
        "clearing left the mask active"
    );
}

#[test]
fn an_operation_on_a_mask_that_does_not_exist_says_so() {
    let mut document = document();
    // Clearing nothing is fine; the menu entry is always there.
    document.apply_mask_op(MaskOp::Clear).expect("clear");

    // The rest need something to act on, and refusing beats pretending.
    let refused = document.apply_mask_op(MaskOp::Invert);
    assert!(refused.is_err(), "inverting a mask that does not exist");
    let said = format!("{}", refused.unwrap_err()).to_lowercase();
    // On the word rather than on "is there a letter in it": the sculptor has
    // to be able to tell which of the several things that can be missing was
    // the one missing here.
    assert!(
        said.contains("máscara"),
        "the refusal does not name what was missing: {said}"
    );
}

#[test]
fn a_cleared_mask_stops_freezing_anything() {
    // The other half of the contract: unfreezing has to actually unfreeze.
    let mut document = document();
    let at = [0.0f32, 0.0, 1.0];
    freeze(&mut document, at);

    let before = radius_along(&document, at).expect("surface");
    document.apply_mask_op(MaskOp::Clear).expect("clear");
    let samples: Vec<GestureSample> = (0..6)
        .map(|i| GestureSample {
            position: at,
            pressure: 1.0,
            time: i as f32 * 0.01,
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &samples,
            [false; 3],
        )
        .expect("stroke");
    let after = radius_along(&document, at).expect("surface");
    assert!(
        (after - before).abs() > 0.002,
        "the surface did not move after the mask was cleared: {before} to {after}"
    );
}

#[test]
fn extruding_a_mask_makes_a_layer_and_keeps_the_mask() {
    let mut document = document();
    freeze(&mut document, [0.0, 0.0, 1.0]);
    let painted = document.mask_state().painted_cells;
    let layers = document.scene().layers.len();

    document
        .extrude_mask(ExtrudeSettings::default())
        .expect("extrude");

    assert_eq!(
        document.scene().layers.len(),
        layers + 1,
        "the extrusion did not arrive as its own layer"
    );
    // The engine smooths a *copy* of the mask for the rim, so the painted
    // region has to survive an operation that consumed it.
    assert_eq!(
        document.mask_state().painted_cells,
        painted,
        "extruding consumed the mask it was given"
    );
}

#[test]
fn extruding_without_a_mask_is_refused_readably() {
    let mut document = document();
    let error = document
        .extrude_mask(ExtrudeSettings::default())
        .expect_err("extruding nothing succeeded");
    let said = format!("{error}").to_lowercase();
    assert!(
        said.contains("máscara"),
        "the refusal does not name what was missing: {said}"
    );
}

/// The other way an extrusion has nothing to work from, and a different
/// answer: the mask is there, so "there is no mask" would send the sculptor
/// looking for a mask they can already see the panel counting.
#[test]
fn extruding_a_mask_with_nothing_painted_in_it_says_which_of_the_two_it_is() {
    let mut document = document();
    freeze(&mut document, [0.0, 0.0, 1.0]);
    // Eroded away rather than cleared: Clear takes the mask itself down, and
    // what is wanted here is a mask that exists and holds nothing.
    while document.mask_state().painted_cells > 0 {
        document
            .apply_mask_op(MaskOp::Contract(16))
            .expect("contract");
    }
    assert!(
        document.mask_state().present,
        "the mask went away instead of emptying"
    );

    let empty = format!(
        "{}",
        document
            .extrude_mask(ExtrudeSettings::default())
            .expect_err("extruding an empty mask succeeded")
    )
    .to_lowercase();
    assert!(
        empty.contains("vazia"),
        "an empty mask was not named as empty: {empty}"
    );

    // And the two refusals are distinguishable, which is the whole point of
    // there being two of them.
    document.apply_mask_op(MaskOp::Clear).expect("clear");
    let missing = format!(
        "{}",
        document
            .extrude_mask(ExtrudeSettings::default())
            .expect_err("extruding nothing succeeded")
    )
    .to_lowercase();
    assert_ne!(
        empty, missing,
        "an empty mask and no mask at all give the same answer"
    );
}

#[test]
fn a_mask_survives_a_resolution_change() {
    // The mask is a field of its own with its own cell size, deliberately not
    // the voxel grid's. A layer added at a different resolution must not
    // disturb what is frozen — otherwise a sculptor loses a mask by doing
    // something unrelated to it.
    let mut document = document();
    let at = [0.0f32, 0.0, 1.0];
    freeze(&mut document, at);
    let painted = document.mask_state().painted_cells;
    assert!(painted > 0);

    // A voxel layer at a coarser resolution than the SDF cache.
    document
        .add_voxel_layer("Grosso", 0.08)
        .expect("voxel layer");

    // Asked on the subtool it was painted on, because that is where it lives
    // now. A mask belongs to its subtool, and adding a layer makes the new one
    // active — so `mask_state` here answers for the grid that was just made,
    // which has no mask and never had one. What this scenario is about is
    // unchanged and is what the two assertions below hold: nothing that
    // happened at another resolution disturbed what was frozen.
    document
        .set_active_layer(document.scene().layers[0].key)
        .expect("back to the sculpted layer");
    assert_eq!(
        document.mask_state().painted_cells,
        painted,
        "adding a layer at another resolution changed the mask"
    );

    let before = radius_along(&document, at).expect("surface");
    let samples: Vec<GestureSample> = (0..6)
        .map(|i| GestureSample {
            position: at,
            pressure: 1.0,
            time: i as f32 * 0.01,
        })
        .collect();
    let _ = document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings::default(),
        &samples,
        [false; 3],
    );
    let after = radius_along(&document, at).expect("surface");
    assert!(
        (after - before).abs() < 0.002,
        "the mask stopped freezing after a resolution change: {before} to {after}"
    );
}

#[test]
fn a_mask_is_as_fine_as_the_surface_it_freezes() {
    // Painted with a large brush, the mask must still resolve detail the
    // surface can express — a coarse mask cannot be extruded at a sensible
    // thickness, and cannot follow a boundary the sculptor can see.
    let mut document = document();
    freeze(&mut document, [0.0, 0.0, 1.0]);
    document
        .extrude_mask(ExtrudeSettings {
            // Thinner than the old quarter-of-the-brush cell would have been.
            thickness: 0.03,
            ..ExtrudeSettings::default()
        })
        .expect("a thin wall should extrude");
}

// -- seeing the mask ---------------------------------------------------------
//
// A mask that cannot be seen is a trap: a sculptor who freezes a region and
// then finds a brush doing nothing has no way to tell a protected surface from
// a broken tool. These state the two things the viewport needs from the engine
// to draw one — a weight at any point, and a signal that it changed.

#[test]
fn a_mask_can_be_read_at_a_point() {
    let mut document = document();
    let frozen = [1.0, 0.0, 0.0];
    let free = [-1.0, 0.0, 0.0];

    assert_eq!(
        document.mask_at(&[frozen, free]),
        None,
        "an unpainted document reported mask weights; the viewport would sample \
         every vertex of every surface to draw nothing"
    );

    freeze(&mut document, frozen);
    let weights = document
        .mask_at(&[frozen, free])
        .expect("a painted mask reads as painted");
    assert!(
        weights[0] > 0.5,
        "the painted spot reads {}, so the frozen region would be drawn as free",
        weights[0]
    );
    assert!(
        weights[1] < 0.01,
        "the far side reads {}, so the whole model would be drawn frozen",
        weights[1]
    );
}

#[test]
fn the_mask_revision_moves_whenever_the_mask_does() {
    let mut document = document();
    let start = document.mask_revision();

    freeze(&mut document, [1.0, 0.0, 0.0]);
    let painted = document.mask_revision();
    assert_ne!(
        painted, start,
        "painting a mask left the revision where it was. A mask stroke moves no \
         clay and dirties no brick — deliberately, it is state the next stroke \
         reads — so this counter is the only thing that can tell the viewport \
         to look again, and what was just painted would stay invisible"
    );

    document.apply_mask_op(MaskOp::Invert).expect("invert");
    let inverted = document.mask_revision();
    assert_ne!(
        inverted, painted,
        "inverting the mask left the revision where it was"
    );

    document.apply_mask_op(MaskOp::Clear).expect("clear");
    assert_ne!(
        document.mask_revision(),
        inverted,
        "clearing the mask left the revision where it was, so the frozen region \
         would go on being drawn over clay that is free"
    );
    assert_eq!(
        document.mask_at(&[[1.0, 0.0, 0.0]]),
        None,
        "a cleared mask still reads as painted"
    );
}

#[test]
fn a_surface_stroke_leaves_the_mask_revision_alone() {
    // The other half: the counter drives a re-sample of every drawn vertex, so
    // a number that moved on every dab would pay that cost on every dab.
    let mut document = document();
    freeze(&mut document, [1.0, 0.0, 0.0]);
    let after_painting = document.mask_revision();

    document
        .apply_stroke(
            ToolKind::Padrao,
            BrushSettings::default(),
            &[GestureSample {
                position: [-1.0, 0.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the stroke was refused");

    assert_eq!(
        document.mask_revision(),
        after_painting,
        "an ordinary stroke moved the mask's revision, so every dab would \
         re-sample the mask across the whole surface"
    );
}

// -- the tool, on every representation ---------------------------------------

#[test]
fn the_mask_tool_paints_a_mask_on_a_grid_rather_than_depositing_clay() {
    // It deposited. `apply_stroke` asked the representation first and a voxel
    // layer sent every tool to `stroke_voxel`, where Máscara fell through to
    // the depositing arm — so the tool that exists to protect clay was adding
    // it, and the mask the sculptor thought they had painted did not exist.
    // The same shape of defect the SDF path already had and already fixed.
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document.add_voxel_layer("Voxels", 0.05).expect("a grid");

    let outcome = document
        .apply_stroke(
            ToolKind::Mascara,
            BrushSettings {
                size: 0.3,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &[GestureSample {
                position: [0.0, 0.0, 0.0],
                pressure: 1.0,
                time: 0.0,
            }],
            [false; 3],
        )
        .expect("the mask stroke was refused on a grid");

    assert!(outcome.changed, "the mask stroke reported no change");
    assert!(
        document.mask_state().is_active(),
        "painting a mask on a grid left no mask"
    );
    assert!(
        document.visible_mesh_geometry().3.is_empty(),
        "the mask stroke put {} indices of material into an empty grid — it \
         deposited clay where the sculptor asked to freeze a region",
        document.visible_mesh_geometry().3.len()
    );
}

#[test]
fn the_mask_tool_reaches_a_mesh_layer() {
    // It was refused there: the tool table gave Máscara no mesh verb, though
    // `stroke_mesh` has been handing the mask to the engine all along — so a
    // mesh could be *protected* by a mask and could not be used to paint one.
    // A mask belongs to no representation; it is a world-addressed field the
    // verbs consult.
    let mut document = document();
    document
        .convert_layer(clayspace_model::Direction::SdfToMesh, 0.05, 0)
        .expect("into a mesh");

    let at = SculptModel::pick(&document, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("the mesh has a near face");
    freeze(&mut document, at);
    assert!(
        document.mask_state().is_active(),
        "a mask painted on a mesh layer left no mask"
    );
}

#[test]
fn every_representation_offers_the_mask_tool() {
    for representation in [
        clayspace_model::Representation::Sdf,
        clayspace_model::Representation::Voxel,
        clayspace_model::Representation::Mesh,
    ] {
        assert!(
            ToolKind::Mascara.exists_on(representation),
            "Máscara is not offered on {representation:?}, so a sculptor \
             working there cannot freeze anything"
        );
    }
}
