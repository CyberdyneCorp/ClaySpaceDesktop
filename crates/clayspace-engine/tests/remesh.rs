//! Rebuilding a mesh layer's topology — DynaMesh.
//!
//! `clay_document_voxel_remesh_layer` is new in ClayCore 0.64.0 and reached
//! this application with the 0.73.0 pin. What it does to the document is one
//! call and one undo step; what it does to *this side* is the interesting
//! part, and it is why these tests exist rather than only the ones at the
//! engine boundary in `claycore/tests/voxel_remesh.rs`.
//!
//! A mesh sculptor is a weld and an adjacency pass over every triangle a layer
//! carries — 160 ms on the reference form — so the document holds several and
//! keeps them across everything that does not invalidate one. A rebuild
//! invalidates one absolutely: every vertex and every index is replaced. The
//! engine refuses a stale sculptor rather than reading freed storage, so the
//! failure is a refused stroke rather than a crash, but a refused stroke in
//! the middle of a sculpting session is a session that has stopped working.
//!
//! **And the engine's own signal for this does not cover undo.** The
//! measurement is in `claycore/tests/voxel_remesh.rs`: the geometry revision
//! is bumped by the rebuild and sits still while history moves the triangles
//! back and forth. So the last two tests here are the ones that matter.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, Direction, GestureSample, LayerKey, RemeshSettings, Representation, SceneModel,
    SculptModel, ToolKind,
};

/// A document whose active layer is a mesh with triangles in it.
///
/// Reached by crossing the starting form rather than by importing a file, so
/// the fixture needs nothing on disk and the layer arrives exactly as a
/// sculptor's would.
fn meshed() -> Option<(ClayDocument, LayerKey)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let made = document.convert_layer(Direction::SdfToMesh, 0.05, 0).ok()?;
    Some((document, made))
}

fn triangles(document: &mut ClayDocument) -> usize {
    document.visible_mesh_geometry().3.len() / 3
}

fn brush() -> BrushSettings {
    BrushSettings {
        size: 0.25,
        intensity: 1.0,
        ..BrushSettings::default()
    }
}

/// A short drag across the front of the form.
fn arc() -> Vec<GestureSample> {
    (0..6)
        .map(|i| {
            let t = i as f32 / 5.0;
            GestureSample {
                position: [(t - 0.5) * 0.4, 0.0, 1.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect()
}

/// A rebuild replaces the layer's triangles and says what it cost.
#[test]
fn a_rebuild_replaces_the_layers_triangles() {
    let Some((mut document, key)) = meshed() else {
        return;
    };
    let before = triangles(&mut document);
    assert!(before > 0, "the crossing produced no triangles to rebuild");

    let outcome = document
        .remesh_layer(key, RemeshSettings::default())
        .expect("a mesh layer accepts a rebuild");

    assert_eq!(
        outcome.triangles_before, before as u64,
        "the outcome does not describe the mesh that was there"
    );
    assert_eq!(
        triangles(&mut document),
        outcome.triangles_after as usize,
        "the layer does not hold what the outcome says was built, so the \
         viewport is drawing something the panel is not describing"
    );
    assert!(
        outcome.voxel_size > 0.0,
        "a resolution given as a cell count has to come back as a length, \
         since that is the only form a sculptor can compare to their brush"
    );
}

/// It is refused on every representation that has no topology to rebuild.
///
/// Named rather than left to the engine's NOT_FOUND: a field steepens and is
/// consolidated, a grid has cells and is resampled, and offering the wrong one
/// of the three with a result code for a reason is how a sculptor learns not
/// to trust the panel.
#[test]
fn a_field_layer_refuses_a_rebuild_by_name() {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form");
    let key = document.scene().active.expect("something is active");
    assert_eq!(
        document.scene().layer(key).map(|l| l.representation),
        Some(Representation::Sdf)
    );

    let refusal = document
        .remesh_layer(key, RemeshSettings::default())
        .expect_err("a field layer has no topology to rebuild");
    assert!(
        refusal.to_string().contains("malha"),
        "the refusal has to name what a rebuild applies to: {refusal}"
    );
}

/// The resolution is what decides the density, through the whole path.
///
/// Held here as well as at the engine boundary because the two numbers a
/// sculptor moves between pass through a sanitizer and a parameter translation
/// on the way, and either could quietly drop the value.
#[test]
fn the_resolution_reaches_the_engine() {
    let Some((mut document, key)) = meshed() else {
        return;
    };
    let coarse = document
        .remesh_layer(
            key,
            RemeshSettings {
                resolution: 48,
                ..RemeshSettings::default()
            },
        )
        .expect("a rebuild at 48");

    let Some((mut document, key)) = meshed() else {
        return;
    };
    let fine = document
        .remesh_layer(
            key,
            RemeshSettings {
                resolution: 128,
                ..RemeshSettings::default()
            },
        )
        .expect("a rebuild at 128");

    assert!(
        coarse.triangles_after < fine.triangles_after,
        "48 across the longest axis produced {} triangles and 128 produced \
         {}: the slider is not reaching the engine",
        coarse.triangles_after,
        fine.triangles_after
    );
    assert!(
        coarse.voxel_size > fine.voxel_size,
        "the coarser request came back with the smaller cell ({} against {})",
        coarse.voxel_size,
        fine.voxel_size
    );
}

/// A stroke lands on the rebuilt mesh.
///
/// The point of the whole path: a sculptor rebuilds *because* the surface has
/// stopped taking detail, so the next thing they do is sculpt it. The sculptor
/// held over the old triangles has to be gone and a new one has to be ready by
/// the time this returns — a pick against a mesh layer is answered by the
/// sculptor's own raycast, so a layer with none is one where the press orbits
/// the camera instead of leaving a mark.
#[test]
fn a_stroke_lands_on_the_rebuilt_mesh() {
    let Some((mut document, key)) = meshed() else {
        return;
    };
    document
        .remesh_layer(key, RemeshSettings::default())
        .expect("a rebuild");

    let outcome = document
        .apply_stroke(ToolKind::Padrao, brush(), &arc(), [false; 3])
        .expect("a stroke on the rebuilt mesh must not be refused");
    assert!(
        outcome.changed,
        "the stroke reached the rebuilt mesh and moved nothing"
    );
}

/// And after undoing the rebuild.
///
/// This is the regression, and the engine cannot help with it. Undo puts back
/// every vertex and every index the rebuild replaced, and
/// `clay_document_mesh_layer_revision` — the number documented as bumped
/// "every time a layer's triangles are replaced wholesale", for exactly the
/// cache this invalidates — does not move when it does. Measured on 0.73.0 and
/// held in `claycore/tests/voxel_remesh.rs`.
///
/// So a sculptor who rebuilds, dislikes it, undoes and keeps working is
/// sculpting through an adjacency and a BVH over triangles that are no longer
/// in the document. `ClayDocument` records the engine depth each rebuild sits
/// at and drops the sculptor when history stands on either side of one; this
/// is what says that works, and it fails the day the record is removed.
#[test]
fn a_stroke_lands_after_the_rebuild_is_undone() {
    let Some((mut document, key)) = meshed() else {
        return;
    };
    let before = triangles(&mut document);
    document
        .remesh_layer(key, RemeshSettings::default())
        .expect("a rebuild");
    assert_ne!(
        triangles(&mut document),
        before,
        "the rebuild left the same triangle count, so this fixture cannot tell \
         the two meshes apart and would pass either way"
    );

    assert!(
        document.undo().expect("one step back"),
        "nothing was undone"
    );
    assert_eq!(
        triangles(&mut document),
        before,
        "undoing the rebuild did not put the original triangles back"
    );

    let outcome = document
        .apply_stroke(ToolKind::Padrao, brush(), &arc(), [false; 3])
        .expect(
            "a stroke after undoing a rebuild was refused. The mesh sculptor \
             is still the one built over the rebuilt triangles, which are no \
             longer in the document",
        );
    assert!(
        outcome.changed,
        "the stroke was accepted after the undo and moved nothing"
    );
}

/// And after redoing it, which replaces them again.
#[test]
fn a_stroke_lands_after_the_rebuild_is_redone() {
    let Some((mut document, key)) = meshed() else {
        return;
    };
    document
        .remesh_layer(key, RemeshSettings::default())
        .expect("a rebuild");
    let rebuilt = triangles(&mut document);

    assert!(document.undo().expect("one step back"));
    assert!(
        document.redo().expect("one step forward"),
        "nothing was redone"
    );
    assert_eq!(
        triangles(&mut document),
        rebuilt,
        "redoing the rebuild did not put the rebuilt triangles back"
    );

    let outcome = document
        .apply_stroke(ToolKind::Padrao, brush(), &arc(), [false; 3])
        .expect("a stroke after redoing a rebuild was refused");
    assert!(outcome.changed);
}
