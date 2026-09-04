//! Every entry in the Máscaras menu, measured.
//!
//! `masking.rs` covered what a mask is *for* — a frozen region that resists
//! every verb — and covered the operations on the mask itself only in the
//! weakest possible way: Inverter, Suavizar and the bounded complement were
//! called and nothing at all was asserted about them. "It returned `Ok`" is
//! not "it works", and three of the six operations had no more than that
//! behind them.
//!
//! Two of them were also taking an amount nobody could set. `Expandir`,
//! `Contrair` and `Suavizar máscara` were dispatched from the menu with a
//! hard-coded 1, and an extrusion with every default it was born with, so its
//! thickness, rounding and edge smoothing were unreachable from the interface
//! entirely. The amounts live in the mask panel now; these hold what each one
//! actually does with them.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, ExtrudeSettings, ExtrudeSide, GestureSample, MaskModel, MaskOp,
    Representation, SceneModel, SculptModel, ToolKind,
};

/// A sphere with a patch of its near face frozen.
fn masked() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let at = SculptModel::pick(&document, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("the starting form is under the ray");
    let samples: Vec<GestureSample> = (0..4)
        .map(|i| GestureSample {
            position: at,
            pressure: 1.0,
            time: i as f32 * 0.1,
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Mascara,
            BrushSettings {
                size: 0.3,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("paint the mask");
    document
}

/// The middle of the frozen patch, a point on its shoulder, and one clear of
/// it — the three places an operation on a mask can be told apart.
const CENTRE: [f32; 3] = [0.0, 0.0, 1.0];
const SHOULDER: [f32; 3] = [0.28, 0.0, 0.96];
const OUTSIDE: [f32; 3] = [0.40, 0.0, 0.92];

fn weights(document: &ClayDocument) -> [f32; 3] {
    let read = document
        .mask_at(&[CENTRE, SHOULDER, OUTSIDE])
        .expect("the fixture is masked");
    [read[0], read[1], read[2]]
}

fn cells(document: &ClayDocument) -> usize {
    document.mask_state().painted_cells
}

// -- Inverter ----------------------------------------------------------------

#[test]
fn inverter_frees_what_was_frozen_and_freezes_what_was_free() {
    let mut document = masked();
    let [centre, _, outside] = weights(&document);
    assert!(centre > 0.9 && outside < 0.01, "the fixture is not a patch");

    document.apply_mask_op(MaskOp::Invert).expect("invert");
    let [centre, _, outside] = weights(&document);
    assert!(
        centre < 0.1,
        "the middle of the patch still reads {centre} after inverting"
    );
    assert!(
        outside > 0.9,
        "the clay beside the patch reads {outside} after inverting, so \
         inverting froze nothing new"
    );
}

#[test]
fn inverter_reaches_only_where_the_mask_has_been() {
    // Worth stating rather than discovering: a mask is a sparse field, and
    // inverting it fills the blocks it has *allocated* rather than the
    // universe. The far side of the model is untouched — which is what makes
    // the operation finite, and is why the bounded complement exists as a
    // separate entry for the "everything except this" a sculptor usually
    // means.
    let mut document = masked();
    document.apply_mask_op(MaskOp::Invert).expect("invert");
    let far = document.mask_at(&[[0.0, 0.0, -1.0]]).expect("still masked")[0];
    assert!(
        far < 0.01,
        "the far side of the model reads {far} after inverting a patch on the \
         near one, so inverting is not bounded after all"
    );
}

// -- Expandir and Contrair ---------------------------------------------------

#[test]
fn expandir_grows_the_patch_and_the_amount_reaches_it() {
    let base = masked();
    let before = cells(&base);

    let mut small = masked();
    small.apply_mask_op(MaskOp::Expand(1)).expect("expand");
    let mut large = masked();
    large.apply_mask_op(MaskOp::Expand(4)).expect("expand");

    assert!(
        cells(&small) > before,
        "expanding by one left {} cells against {before}",
        cells(&small)
    );
    assert!(
        cells(&large) > cells(&small),
        "expanding by four left {} cells and by one left {} — the amount the \
         panel sets does not reach the operation, which is what the menu's \
         hard-coded 1 meant for two years",
        cells(&large),
        cells(&small)
    );
    // And it grows *outward*: the shoulder of the patch freezes harder.
    assert!(
        weights(&large)[1] > weights(&small)[1],
        "the patch's shoulder did not freeze further as it grew"
    );
}

#[test]
fn contrair_shrinks_the_patch_and_the_amount_reaches_it() {
    let base = masked();
    let before = cells(&base);

    let mut small = masked();
    small.apply_mask_op(MaskOp::Contract(1)).expect("contract");
    let mut large = masked();
    large.apply_mask_op(MaskOp::Contract(4)).expect("contract");

    assert!(
        cells(&small) < before,
        "contracting by one left {} cells against {before}",
        cells(&small)
    );
    assert!(
        cells(&large) < cells(&small),
        "contracting by four left {} cells and by one left {}",
        cells(&large),
        cells(&small)
    );
}

#[test]
fn expanding_and_contracting_are_opposites() {
    let mut document = masked();
    let before = cells(&document);
    document.apply_mask_op(MaskOp::Expand(3)).expect("expand");
    document
        .apply_mask_op(MaskOp::Contract(3))
        .expect("contract");
    let after = cells(&document);
    // Not exactly: grey dilation followed by erosion closes small holes and
    // does not have to give the count back to the cell. Within a tenth is the
    // claim — the patch is the same patch, not a patch grown three cells.
    let drift = (after as f32 - before as f32).abs() / before as f32;
    assert!(
        drift < 0.1,
        "expanding by three and contracting by three left {after} cells \
         against {before}, a drift of {drift:.2}"
    );
}

// -- Suavizar máscara --------------------------------------------------------

#[test]
fn suavizar_softens_the_patch_and_the_amount_reaches_it() {
    // Called and never checked before. A smoothing pass averages a cell toward
    // its neighbours, so the middle of a hard patch comes down and the
    // boundary spreads — and running it again does more of both, which is what
    // says the panel's number arrived.
    let base = masked();
    let [centre_before, ..] = weights(&base);
    let before = cells(&base);

    let mut once = masked();
    once.apply_mask_op(MaskOp::Smooth(1)).expect("smooth");
    let mut lots = masked();
    lots.apply_mask_op(MaskOp::Smooth(8)).expect("smooth");

    let [centre_once, ..] = weights(&once);
    let [centre_lots, ..] = weights(&lots);
    assert!(
        centre_once < centre_before,
        "one pass left the middle at {centre_once} against {centre_before}"
    );
    assert!(
        centre_lots < centre_once,
        "eight passes left the middle at {centre_lots} and one pass left it at \
         {centre_once} — the amount does not reach the operation"
    );
    assert!(
        cells(&lots) > before,
        "smoothing did not spread the boundary: {} cells against {before}",
        cells(&lots)
    );
    // It softens rather than erases: a smoothed mask still freezes.
    assert!(
        centre_lots > 0.5,
        "eight passes took the middle to {centre_lots}, which is a mask being \
         rubbed out rather than softened"
    );
}

// -- Complemento delimitado --------------------------------------------------

#[test]
fn the_bounded_complement_inverts_inside_the_patch_and_leaves_the_rest() {
    // The difference from Inverter, which is the reason both entries exist:
    // this one is bounded by what the mask already covers, so it is
    // "everything except this, *here*" rather than "everything the mask has
    // ever allocated".
    let mut document = masked();
    document
        .apply_mask_op(MaskOp::InvertWithinBounds)
        .expect("bounded complement");

    let [centre, shoulder, _] = weights(&document);
    assert!(centre < 0.1, "the middle of the patch still reads {centre}");
    assert!(
        shoulder > 0.9,
        "the shoulder of the patch reads {shoulder}, so nothing was frozen in \
         the patch's place"
    );

    let far = document.mask_at(&[[0.0, 0.0, -1.0]]).expect("still masked")[0];
    assert!(
        far < 0.01,
        "the far side reads {far}: a *bounded* complement reached outside the \
         bounds it is named for"
    );
}

#[test]
fn the_bounded_complement_on_nothing_is_not_an_error() {
    // It has nothing to be the complement of, and refusing would be a dialogue
    // about an operation that would have done nothing anyway.
    let mut document = masked();
    document.apply_mask_op(MaskOp::Clear).expect("clear");
    // A mask that exists and is empty, rather than no mask at all.
    document.apply_mask_op(MaskOp::Invert).unwrap_err();
}

// -- Limpar ------------------------------------------------------------------

#[test]
fn limpar_leaves_nothing_frozen() {
    let mut document = masked();
    assert!(document.mask_state().is_active());
    document.apply_mask_op(MaskOp::Clear).expect("clear");
    assert!(!document.mask_state().is_active());
    assert_eq!(
        document.mask_at(&[CENTRE]),
        None,
        "a cleared mask still reads as painted"
    );
}

// -- Extrudar ----------------------------------------------------------------

/// How far the surface stands out along the axis the patch faces.
fn reach(document: &ClayDocument) -> f32 {
    SculptModel::pick(document, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .map(|hit| hit[2])
        .expect("the near face")
}

fn extruded(side: ExtrudeSide, thickness: f32) -> (ClayDocument, f32, f32) {
    let mut document = masked();
    let before = reach(&document);
    document
        .extrude_mask(ExtrudeSettings {
            thickness,
            side,
            border_round: 0.0,
            border_smooth: 0,
        })
        .expect("the extrusion was refused");
    let after = reach(&document);
    (document, before, after)
}

#[test]
fn extrudar_puts_the_patch_in_a_layer_of_its_own() {
    let mut document = masked();
    let before = document.scene().layers.len();
    document
        .extrude_mask(ExtrudeSettings::default())
        .expect("the extrusion was refused");
    assert_eq!(
        document.scene().layers.len(),
        before + 1,
        "an extrusion is a new piece of geometry rather than an edit to the \
         layer it came from, so it belongs in its own row where it can be \
         moved, hidden or thrown away"
    );
    // And the mask survives it: a sculptor who extrudes and does not like the
    // result should not have to paint the mask again.
    assert!(
        document.mask_state().is_active(),
        "extruding consumed the mask"
    );
}

#[test]
fn the_three_extrusion_sides_put_the_wall_in_three_places() {
    let (_, base, out) = extruded(ExtrudeSide::Outward, 0.2);
    let (_, _, inward) = extruded(ExtrudeSide::Inward, 0.2);
    let (_, _, centred) = extruded(ExtrudeSide::Centred, 0.2);

    assert!(
        out > base + 0.1,
        "Para fora reached {out} from {base}, which is not a wall standing off \
         the surface"
    );
    assert!(
        (inward - base).abs() < 0.01,
        "Para dentro moved the outside of the model to {inward} from {base}; a \
         wall built inward should leave the outer surface where it was"
    );
    // Half each way is the whole difference between Centrado and the other
    // two, and the outward half of it lands where arithmetic says: half the
    // thickness above the surface, 1.1015 measured against 1.1000.
    //
    // Not stated as half of *Para fora's* travel, which it is not — that one
    // reaches 1.16 rather than 1.20 for the same 0.2, so the wall it builds is
    // referred to a threshold surface a little inside the one a raycast finds.
    // Pinning Centrado to the thickness and Para fora to an ordering is what
    // can be justified from the measurements rather than assumed from the
    // names.
    assert!(
        (centred - (base + 0.1)).abs() < 0.01,
        "Centrado reached {centred}, where half of a 0.2 wall above a surface \
         at {base} is {}",
        base + 0.1
    );
    assert!(
        out > centred,
        "Para fora reached {out} and Centrado reached {centred}: a wall built \
         wholly outside must stand further out than one built half in"
    );
}

#[test]
fn the_extrusion_thickness_reaches_the_wall() {
    // The setting existed and had no control anywhere in the interface, so
    // every extrusion the application could make was 0.08 thick.
    let (_, base, thin) = extruded(ExtrudeSide::Outward, 0.05);
    let (_, _, thick) = extruded(ExtrudeSide::Outward, 0.2);
    assert!(
        thick > thin + 0.05,
        "a 0.2 wall reached {thick} and a 0.05 wall reached {thin} from {base}"
    );
}

#[test]
fn a_rounded_and_smoothed_rim_is_accepted_and_still_a_wall() {
    // The other two settings that had no control. Neither has a fingerprint a
    // raycast down the middle can see — they shape the *rim* — so this holds
    // the pair to being accepted and to not destroying the wall, which is what
    // a silently-ignored or silently-fatal parameter would look like.
    let mut document = masked();
    let base = reach(&document);
    document
        .extrude_mask(ExtrudeSettings {
            thickness: 0.2,
            side: ExtrudeSide::Outward,
            border_round: 0.05,
            border_smooth: 8,
        })
        .expect("a rounded, smoothed extrusion was refused");
    let after = reach(&document);
    assert!(
        after > base + 0.1,
        "the rounded wall reached {after} from {base}"
    );
}

#[test]
fn extruding_an_empty_mask_is_refused_rather_than_making_an_empty_layer() {
    let mut document = masked();
    document.apply_mask_op(MaskOp::Clear).expect("clear");
    let layers = document.scene().layers.len();
    document
        .extrude_mask(ExtrudeSettings::default())
        .expect_err("extruding nothing");
    assert_eq!(
        document.scene().layers.len(),
        layers,
        "a refused extrusion left a layer behind"
    );
}

// -- Extrudar, from each representation --------------------------------------
//
// Every test above runs on an SDF layer, and that is exactly how this shipped
// broken: `clay_document_mask_extrude` samples a *layer's field*, so it refuses
// a mesh and a grid alike — "this layer has no field to extrude from". On the
// mesh layer a sculptor is most likely to be on, Extrudar did nothing, and the
// refusal was written into a notice nobody read.

/// Freezes the patch facing the camera on whatever the active layer is.
fn freeze_near_face(document: &mut ClayDocument) {
    let at = SculptModel::pick(document, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0])
        .expect("the layer has a near face");
    let samples: Vec<GestureSample> = (0..4)
        .map(|i| GestureSample {
            position: at,
            pressure: 1.0,
            time: i as f32 * 0.1,
        })
        .collect();
    document
        .apply_stroke(
            ToolKind::Mascara,
            BrushSettings {
                size: 0.3,
                intensity: 1.0,
                ..BrushSettings::default()
            },
            &samples,
            [false; 3],
        )
        .expect("the mask stroke was refused");
}

#[test]
fn extrudar_pulls_a_wall_off_a_grid() {
    // `clay_voxel_mask_extrude` has been in the engine all along and was never
    // bound: a grid already knows which of its cells are on its surface, so it
    // needs no sampled field. The document's verb was the only one wired, and
    // it refused a grid.
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    document.add_voxel_layer("Voxels", 0.05).expect("a grid");
    document
        .apply_stroke(
            ToolKind::Padrao,
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
        .expect("the deposit was refused");
    freeze_near_face(&mut document);

    let before =
        SculptModel::pick(&document, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]).expect("surface")[2];
    let layers = document.scene().layers.len();
    document
        .extrude_mask(ExtrudeSettings {
            thickness: 0.2,
            side: ExtrudeSide::Outward,
            border_round: 0.0,
            border_smooth: 0,
        })
        .expect("extruding a grid was refused");

    assert_eq!(
        document.scene().layers.len(),
        layers + 1,
        "extruding a grid made no layer"
    );
    // Measured with the new row active, because a pick answers from the active
    // subtool when that subtool is a grid — `pick_active_grid` reads the cells
    // and knows nothing of a field beside them. Extrudar leaves the sculptor
    // on the grid they masked, so the wall has to be asked about where it is.
    let extrusion = document.scene().layers.last().expect("the new row").key;
    document
        .set_active_layer(extrusion)
        .expect("activate the extrusion");
    let after =
        SculptModel::pick(&document, [0.0, 0.0, 4.0], [0.0, 0.0, -1.0]).expect("surface")[2];
    assert!(
        after > before + 0.1,
        "the wall reached {after} from {before}, which is not a 0.2 plate \
         standing on the grid's surface"
    );
    // An SDF row, like the field path's: Extrudar means one thing whatever it
    // was run on, and the row it makes can be sculpted, hidden or thrown away
    // the same way either time.
    assert_eq!(
        document.active_representation(),
        Representation::Sdf,
        "the extrusion did not arrive as a field layer"
    );
}

#[test]
fn extrudar_on_a_mesh_says_what_to_do_instead() {
    // There is no engine verb for it: neither `clay_document_mask_extrude`,
    // which samples a field a mesh layer does not have, nor a mesh-sculptor
    // equivalent. The refusal has to name the way round rather than being a
    // click that does nothing.
    //
    // Re-checked at ClayCore v0.78.0: the only mask entry points that release
    // adds are the hierarchy's own per-pass ones, so there is still nothing to
    // extrude a mesh's mask with and the refusal still has to teach.
    let mut document = masked();
    document
        .convert_layer(Direction::SdfToMesh, 0.05, 0)
        .expect("into a mesh");
    freeze_near_face(&mut document);
    let layers = document.scene().layers.len();

    let refused = document
        .extrude_mask(ExtrudeSettings::default())
        .expect_err("extruding a mesh layer");
    let said = refused.to_string();
    assert!(
        said.contains("SDF"),
        "the refusal does not name the way round: {said}"
    );
    assert_eq!(
        document.scene().layers.len(),
        layers,
        "a refused extrusion left a layer behind"
    );
}

#[test]
fn every_representation_says_whether_it_can_be_extruded() {
    // The menu asks this before offering the entry, so the two cannot drift
    // apart into a menu that offers what the engine refuses.
    assert!(clayspace_model::can_extrude(Representation::Sdf));
    assert!(clayspace_model::can_extrude(Representation::Voxel));
    assert!(!clayspace_model::can_extrude(Representation::Mesh));
}
