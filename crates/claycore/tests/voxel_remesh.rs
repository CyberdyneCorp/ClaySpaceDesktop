//! The voxel remesher, at the engine boundary.
//!
//! What sculpting applications call DynaMesh: throw a mesh into a voxel field
//! and march a new one out of it, so overlapping shells fuse, stretched
//! triangles disappear and the density comes out uniform. New in ClayCore
//! 0.63.0, and inside the document — one call, one undo step — in 0.64.0.
//!
//! Two things are worth measuring here rather than reading. The first is that
//! the operation is genuinely destructive: it is offered to a sculptor as
//! *repair*, and the report is the only place the price is stated. The second
//! is the estimate, because the interface asks for it on every drag of a
//! resolution slider and would be unusable if it cost anything like the
//! rebuild.

use claycore::{Document, Item, MeshLayerDesc, MeshParams, Op, RemeshParams, Resolution, Surface};

/// Two overlapping spheres, meshed — the shape the remesher exists for. Marched
/// out of a field they arrive as one surface with an interior seam, which is
/// what a rebuild is asked to resolve.
fn two_lobes() -> Option<claycore::Mesh> {
    let mut document = Document::new().ok()?;
    let layer = document.add_sdf_layer("corpo").ok()?;
    for x in [-0.35f32, 0.35] {
        let mut lobe = Item::sphere(0.5).ok()?;
        lobe.set_op(Op::Add).ok()?;
        lobe.set_position([x, 0.0, 0.0]).ok()?;
        document.add_item(layer, &lobe).ok()?;
    }
    document
        .mesh(MeshParams {
            voxel_size: Some(0.02),
            ..Default::default()
        })
        .ok()
}

fn mesh_document() -> Option<(Document, claycore::LayerId, claycore::Mesh)> {
    let source = two_lobes()?;
    let mut document = Document::new().ok()?;
    // A fresh document journals nothing until it is asked to, and the point of
    // this fixture is the step the rebuild puts on the stack.
    document.enable_undo().ok()?;
    let layer = document
        .attach_mesh_layer(
            &source,
            &MeshLayerDesc {
                name: "malha".into(),
                max_vertices: 0,
                max_triangles: 0,
                import_scale: 1.0,
            },
        )
        .ok()?;
    Some((document, layer, source))
}

/// The defaults come from the engine rather than from this crate.
///
/// `clay_mesh_voxel_remesh_defaults` exists so a caller does not transcribe
/// them, and the reason to hold it is that a transcribed default drifts
/// silently: the wrapper would go on producing whatever the header said on the
/// day it was written, and an upgrade that changed the engine's mind would
/// change nothing here and be impossible to see.
#[test]
fn the_defaults_are_the_engines_own() {
    let params = RemeshParams::default();
    assert!(
        matches!(params.resolution, Resolution::LongestAxis(n) if n > 0)
            || matches!(params.resolution, Resolution::VoxelSize(s) if s > 0.0),
        "the engine's default resolution came back unusable: {:?}",
        params.resolution
    );
    assert_eq!(
        params.surface,
        Surface::Smooth,
        "the default surface is the watertight one; sharp is experimental and \
         the engine flags it as such"
    );
}

/// A rebuild produces a mesh, and the report says what it cost.
#[test]
fn a_rebuild_resolves_two_lobes_into_one_surface() {
    let Some(source) = two_lobes() else {
        return;
    };
    let Ok((rebuilt, report)) = source.voxel_remesh(RemeshParams::at_longest_axis(96)) else {
        return;
    };

    assert!(
        !rebuilt.is_empty(),
        "the rebuild produced no triangles at all"
    );
    assert_eq!(
        report.result_triangles as usize,
        rebuilt.index_count() / 3,
        "the report and the mesh disagree about how many triangles came out"
    );
    assert_eq!(
        report.result_components, 1,
        "two overlapping lobes should fuse into one component, not {}",
        report.result_components
    );
    assert!(
        report.result_watertight,
        "the smooth mode promises a watertight result by construction"
    );
    // The resolution decides the density, which is the whole of what DynaMesh
    // is for. Held against *another rebuild* rather than against the source,
    // and that correction is worth keeping: the first version asserted the
    // result was coarser than the source and failed, because the source is
    // itself a marched isosurface at 0.02 over a 1.7-unit extent — 85 cells
    // across, which is coarser than the 96 being asked for. It measured which
    // of two fixtures happened to be finer, not whether the control works.
    //
    // Measured on 0.73.0: 96 gives 151,832 triangles and 48 gives 38,244,
    // against a source of 119,100.
    let Ok((_, coarse)) = source.voxel_remesh(RemeshParams::at_longest_axis(48)) else {
        return;
    };
    assert!(
        coarse.result_triangles < report.result_triangles,
        "48 across the longest axis produced {} triangles and 96 produced {}: \
         the resolution is not deciding the density",
        coarse.result_triangles,
        report.result_triangles
    );
    assert!(
        coarse.voxel_size > report.voxel_size,
        "the coarser request resolved to a smaller cell ({} against {}), so \
         the two numbers are not the same quantity",
        coarse.voxel_size,
        report.voxel_size
    );
}

/// And it says what it destroyed.
///
/// Vertex and polygon identity are gone and UVs are dropped rather than
/// reprojected. Both are in the report because a sculptor is entitled to be
/// told before an interface offers this as "repair" — and `uvs_dropped` is
/// specifically not a failure, so nothing else would surface it.
#[test]
fn the_report_states_what_the_rebuild_cost() {
    let Some(source) = two_lobes() else {
        return;
    };
    let Ok((_, report)) = source.voxel_remesh(RemeshParams::at_longest_axis(96)) else {
        return;
    };

    assert!(
        report.source_vertices > 0 && report.result_vertices > 0,
        "the report carries no vertex counts, so nothing can be shown about \
         what the rebuild replaced"
    );
    assert!(
        report.voxel_size > 0.0,
        "the resolved voxel size is what a longest-axis request means in world \
         units, and it is what an interface shows back"
    );
    assert!(
        !report.cancelled,
        "nothing cancelled this rebuild and it says it was cancelled"
    );
}

/// The estimate is cheap enough to put behind a slider.
///
/// The engine's claim is that it walks the source's triangles and marks a
/// brick lattice, allocating nothing proportional to the sample count it
/// predicts. This holds the claim rather than trusting it: an estimate that
/// cost anything like the rebuild would make a resolution control a slideshow,
/// which is exactly the trap `layer_cost` set for the consolidation row.
#[test]
fn asking_what_a_rebuild_costs_is_not_paying_for_it() {
    let Some(source) = two_lobes() else {
        return;
    };
    let params = RemeshParams::at_longest_axis(96);

    let started = std::time::Instant::now();
    let Ok(estimate) = source.remesh_estimate(params) else {
        return;
    };
    let estimating = started.elapsed();

    let started = std::time::Instant::now();
    let Ok(_) = source.voxel_remesh(params) else {
        return;
    };
    let rebuilding = started.elapsed();

    assert!(
        estimate.voxel_size > 0.0 && estimate.grid_dimensions.iter().all(|d| *d > 0),
        "the estimate came back with no grid: {estimate:?}"
    );
    assert!(
        estimating < rebuilding / 4,
        "estimating took {estimating:?} against {rebuilding:?} to rebuild. The \
         estimate is asked for on every drag of a resolution slider and the \
         rebuild is asked for once"
    );
}

/// Through the document it lands on the layer, as one undo step.
#[test]
fn a_layer_rebuild_replaces_the_layer_and_is_undoable() {
    let Some((mut document, layer, _source)) = mesh_document() else {
        return;
    };
    let before = document.mesh_layer_revision(layer).expect("a revision");

    let Ok(report) = document.remesh_layer(layer, RemeshParams::at_longest_axis(96)) else {
        return;
    };
    assert!(report.result_triangles > 0);

    let after = document.mesh_layer_revision(layer).expect("a revision");
    assert!(
        after > before,
        "the geometry revision did not move ({before} -> {after}). A rebuild \
         swaps every vertex and every index, and this is the only signal an \
         adjacency, a BVH or a live sculptor over the old ones has that it is \
         now wrong"
    );

    let triangles = |document: &mut Document| {
        document
            .read_mesh_layer("malha")
            .map(|(_, _, _, indices)| indices.len() / 3)
            .unwrap_or(0)
    };
    let rebuilt = triangles(&mut document);
    assert_eq!(
        rebuilt, report.result_triangles as usize,
        "the layer does not hold what the report says was built"
    );

    document.undo().expect("one step back");
    assert_ne!(
        triangles(&mut document),
        rebuilt,
        "undoing the rebuild left the rebuilt triangles in the layer"
    );

    // And the revision does NOT move when it does, which is what this holds.
    //
    // The number's own documentation says it is bumped "every time a layer's
    // triangles are replaced wholesale", and that what it exists for is the
    // change a cache does not survive — "a rebuild swaps every vertex and
    // every index, and an adjacency, a BVH or a live sculptor built over the
    // old ones is wrong in a way nothing else detects". Undoing a rebuild is
    // exactly that change, and the revision sits still through it: measured on
    // 0.73.0, attach 1 / rebuild 2 / undo 2 / redo 2, with the triangle count
    // going 119,100 -> 37,752 -> 119,100 -> 37,752.
    //
    // So the one moment the number was added for is the one moment it is
    // silent. Held as an equality rather than left unstated, because
    // `clayspace-engine` carries a second record — the engine depth a rebuild
    // sits at — purely to cover this, and that record is dead weight the day
    // this starts failing. Reported upstream; when it is fixed, this fails and
    // `ClayDocument::settle_geometry_revisions` loses its history half.
    assert_eq!(
        document.mesh_layer_revision(layer).expect("a revision"),
        after,
        "the geometry revision now moves when history replaces a layer's \
         triangles. That is good news and it is what the number is documented \
         to do: clayspace-engine's `Rebuild` record exists only because it did \
         not, and should go with this assertion"
    );
}

/// A refusal leaves the layer byte-identical, and carries the numbers that
/// explain it.
///
/// The resolution is the parameter a sculptor drives, so it is the one that
/// can be set to something the source will not survive. The engine's contract
/// is that nothing is written until the rebuild has succeeded *and* validated
/// — which is what lets an interface offer a resolution control at all,
/// instead of asking the sculptor to guess and then undo.
#[test]
fn a_refused_rebuild_changes_nothing() {
    let Some((mut document, layer, _source)) = mesh_document() else {
        return;
    };
    let before = document.mesh_layer_revision(layer).expect("a revision");

    // Zero cells across the longest axis is not a resolution. Refused for the
    // parameter rather than for the mesh, which is the refusal an interface
    // has to survive without leaving a half-rebuilt layer behind.
    let outcome = document.remesh_layer(layer, RemeshParams::at_longest_axis(0));
    let Err(refused) = outcome else {
        panic!("a resolution of zero was accepted");
    };
    assert!(
        !refused.error.to_string().is_empty(),
        "the refusal carries no message to show"
    );
    assert_eq!(
        document.mesh_layer_revision(layer).expect("a revision"),
        before,
        "a refused rebuild moved the layer's geometry revision, so it wrote \
         something"
    );
}
