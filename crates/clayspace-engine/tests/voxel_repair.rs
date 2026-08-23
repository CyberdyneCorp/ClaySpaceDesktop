//! Pre-bake repair, and the report that has to come first.
//!
//! A sealed void is invisible until something needs the model to be solid —
//! a print, a boolean, a physical fabrication — which is why these are
//! *pre-bake* verbs rather than sculpting ones. A sculptor cannot see the
//! problem by looking, so the report is the only way they learn there is one.
//!
//! It is reported before anything is changed, always. A repair alters the
//! sculpt, and asking for consent to something unstated is not asking.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{Direction, LayerOperation, Representation, SceneModel, SculptModel};

/// A voxel layer, made by crossing the starting form over.
fn with_voxel_layer() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    document
        .convert_layer(Direction::SdfToVoxel, 0.05, 1)
        .ok()?;
    Some(document)
}

#[test]
fn a_grid_reports_what_is_wrong_with_it() {
    let Some(mut document) = with_voxel_layer() else {
        return;
    };
    assert_eq!(document.active_representation(), Representation::Voxel);
    let report = document
        .repair_report()
        .expect("a voxel layer has a report to give");
    // A rasterized sphere is solid, so this is the uninteresting answer — and
    // it is the one that says the report is being computed rather than
    // guessed. `airtight` and a void count have to agree.
    assert_eq!(
        report.airtight,
        report.enclosed_voids == 0,
        "the report contradicts itself: airtight={} with {} voids",
        report.airtight,
        report.enclosed_voids
    );
}

/// The report is not offered for something that cannot have one.
#[test]
fn a_field_has_no_repair_report() {
    let Some(mut document) = with_voxel_layer() else {
        return;
    };
    let sdf = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Sdf)
        .map(|layer| layer.key)
        .expect("the starting form");
    document.set_active_layer(sdf).expect("activate");
    assert!(
        document.repair_report().is_none(),
        "a field is continuous and has no holes to close"
    );
}

/// The repairs run, change the sculpt, and leave a report that still agrees
/// with itself.
#[test]
fn closing_holes_and_filling_voids_both_apply() {
    let Some(mut document) = with_voxel_layer() else {
        return;
    };
    let outcome = document
        .apply_operation(LayerOperation::CloseHoles { passes: 1 })
        .expect("close holes is a voxel operation");
    assert!(outcome.changed, "a repair reported changing nothing");

    document
        .apply_operation(LayerOperation::FillVoids)
        .expect("fill voids is a voxel operation");

    let after = document.repair_report().expect("a report");
    assert_eq!(
        after.airtight,
        after.enclosed_voids == 0,
        "the report after a repair contradicts itself"
    );
    // Filling every void the outside cannot reach is what airtight means, so
    // after it there must be none left.
    assert!(
        after.airtight,
        "{} voids survived a fill that exists to remove them",
        after.enclosed_voids
    );
}

/// The pre-bake verbs are refused where they mean nothing.
#[test]
fn a_repair_is_refused_on_a_field() {
    let Some(mut document) = with_voxel_layer() else {
        return;
    };
    let sdf = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Sdf)
        .map(|layer| layer.key)
        .expect("the starting form");
    document.set_active_layer(sdf).expect("activate");

    let error = document
        .apply_operation(LayerOperation::FillVoids)
        .expect_err("a field has no cells to fill");
    assert!(
        error.to_string().contains("voxel"),
        "the refusal must name where the verb applies: {error}"
    );
}

/// Refining a region is what the level stack is for: pay for detail where the
/// detail goes, rather than everywhere.
#[test]
fn a_region_can_be_refined_without_refining_everything() {
    let Some(mut document) = with_voxel_layer() else {
        return;
    };
    document
        .apply_operation(LayerOperation::RefineRegion {
            min: [-0.3, -0.3, -0.3],
            max: [0.3, 0.3, 0.3],
        })
        .expect("regional refinement is a voxel operation");
}
