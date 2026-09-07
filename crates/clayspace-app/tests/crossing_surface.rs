//! What the viewport still draws after a crossing empties the field.
//!
//! Reported from a session: converting a field subtool to a mesh in place drew
//! **both** — the field and the mesh together, the two surfaces interpenetrating
//! — and the field only went once mesh sculpting began.
//!
//! The cause is that `clay_document_mesh` refuses an empty document rather than
//! returning an empty mesh. `SurfaceGeometry::settle` meshes the whole field in
//! one call and clears its stored keys only *after* that call succeeds, so a
//! field that had just been crossed away left the refusal propagating and the
//! old surface standing in the GPU buffers. The application's conversion path
//! calls `settle`, and its error arm prints to stderr — where a sculptor never
//! sees it.
//!
//! `rebuild_at` never had this: it clears first and re-meshes per key, so an
//! empty document leaves it correctly empty. Only the whole-document path had
//! to be told that an empty field is a surface with nothing in it rather than
//! a failure.

mod support;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::Direction;
use support::Harness;

#[test]
fn a_crossing_that_empties_the_field_takes_its_surface_with_it() {
    let Some(harness) = Harness::new() else {
        return;
    };
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };
    let mut geometry = SurfaceGeometry::new(&harness.gpu);
    if geometry.rebuild(&harness.gpu, &mut document).is_err() {
        return;
    }
    let field_before = geometry.triangle_count();
    assert!(
        field_before > 0,
        "the fixture drew no field surface, so it cannot tell one that is \
         cleared from one that was never there"
    );

    if document
        .convert_layer_in_place(Direction::SdfToMesh, 0.02, 0)
        .is_err()
    {
        return;
    }
    assert!(
        !document.has_field_surface(),
        "the crossing left field content behind, so this fixture is no longer \
         about an emptied field"
    );

    // What the application does after a conversion, and all it does.
    geometry
        .settle(&harness.gpu, &mut document)
        .expect("an empty field is a surface with nothing in it, not a failure");
    assert_eq!(
        geometry.triangle_count(),
        0,
        "the field's {field_before} triangles are still being drawn after the \
         crossing that removed the field. A sculptor sees the old surface and \
         the new mesh together until something else happens to call sync"
    );
}

/// And the mesh the crossing made is there to be drawn in its place.
///
/// The other half of the same frame: it is not enough that the field goes, the
/// mesh has to arrive. This is what says the two halves are not trading places
/// with each other.
#[test]
fn the_mesh_the_crossing_made_is_ready_to_draw() {
    let Ok(policy) = BackendPolicy::discover(None) else {
        return;
    };
    let Ok(mut document) = ClayDocument::new(policy).and_then(ClayDocument::with_starting_form)
    else {
        return;
    };
    let before = document.mesh_revision();
    if document
        .convert_layer_in_place(Direction::SdfToMesh, 0.02, 0)
        .is_err()
    {
        return;
    }

    let (positions, _, _, indices, spans) = document.visible_mesh_geometry();
    assert!(
        !positions.is_empty() && !indices.is_empty() && !spans.is_empty(),
        "the crossing made a mesh the viewport cannot see: {} vertices, {} \
         indices, {} spans",
        positions.len(),
        indices.len(),
        spans.len()
    );
    assert_ne!(
        document.mesh_revision(),
        before,
        "the revision did not move, so the viewport — which uploads only when \
         it changes — would never upload this mesh"
    );
}
