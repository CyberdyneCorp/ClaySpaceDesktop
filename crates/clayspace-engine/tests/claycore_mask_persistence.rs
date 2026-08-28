//! What the engine's per-layer mask can do, measured at the boundary.
//!
//! The question this answers is task 6.4's: does a mask attached to a layer
//! ride the document's save path? It does — `clay_document_add_mask` puts the
//! mask in the layer, and `clay_document_save` writes it. That is worth having
//! and this crate does not have it yet: `ClayDocument` keeps each subtool's
//! mask as a standalone `clay_mask_create`, so a mask is lost when the
//! document is closed, exactly as it was before masks became per subtool.
//!
//! The reason is the borrow, not the engine. `Document::mask` hands back a
//! `MaskRef` that borrows the document — it must, since the handle may not
//! outlive it — while every masked verb in the wrapper takes the document
//! *and* `Option<&MaskField>` together: `apply_stroke(&mut self, …, mask)`,
//! `relax_region(&self, RelaxParams { mask, … })`, `flatten_region`,
//! `mask_extrude`, and a voxel grid borrowed out of the same document. The C
//! side is built for exactly that pairing and Rust cannot express it: a mask
//! lent out of a document cannot be handed back into it. Reaching the engine's
//! mask means giving those five entry points a form that takes the mask's
//! *layer* rather than the mask, which is a redesign of `claycore`'s masking
//! surface rather than the thin wrapper this change is allowed.
//!
//! So this test is the record of what is on the table, and it fails the day
//! the engine stops serializing masks — which is the other thing worth
//! knowing before that work is scheduled.

use claycore::{Document, Item};

#[test]
fn a_layers_mask_rides_the_documents_save_path() {
    let dir = std::env::temp_dir().join("clayspace-layer-mask");
    std::fs::create_dir_all(&dir).expect("a place to save");
    let path = dir.join("masked.clay");

    let mut document = Document::new().expect("a document");
    let layer = document.add_sdf_layer("Forma").expect("a layer");
    let body = Item::sphere(1.0).expect("a sphere");
    document.add_item(layer, &body).expect("place the sphere");

    let painted = {
        let mut mask = document.add_mask(layer, 0.05).expect("attach a mask");
        mask.fill([-0.5, -0.5, 0.5], [0.5, 0.5, 1.2], 1.0)
            .expect("freeze the near face");
        mask.painted_count().expect("what it covers")
    };
    assert!(painted > 0, "nothing was frozen");

    document.save(&path).expect("save");

    let mut reopened = Document::open(&path).expect("reopen");
    let ids = reopened.layer_ids().expect("its layers");
    let recovered = reopened
        .mask(ids[0])
        .expect("the layer came back without its mask")
        .painted_count()
        .expect("what it covers");
    assert_eq!(
        recovered, painted,
        "the mask came back covering something else"
    );
}

#[test]
fn a_mask_belongs_to_one_layer_and_not_to_its_neighbour() {
    let mut document = Document::new().expect("a document");
    let first = document.add_sdf_layer("Uma").expect("a layer");
    let second = document.add_sdf_layer("Outra").expect("a second layer");
    document
        .add_mask(first, 0.05)
        .expect("attach a mask")
        .fill([-0.5, -0.5, 0.5], [0.5, 0.5, 1.2], 1.0)
        .expect("freeze a region");

    assert!(
        document.mask(first).is_ok(),
        "the layer it was attached to has no mask"
    );
    assert!(
        document.mask(second).is_err(),
        "the mask reached a layer it was never painted on"
    );
}
