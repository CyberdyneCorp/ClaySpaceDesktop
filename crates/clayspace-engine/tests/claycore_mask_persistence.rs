//! What the engine's per-layer mask can do, measured at the boundary.
//!
//! The question this answered was task 6.4's: does a mask attached to a layer
//! ride the document's save path? It does — `clay_document_add_mask` puts the
//! mask in the layer, and `clay_document_save` writes it.
//!
//! For a long while this file also recorded that the *application* could not
//! use that, and why. The reason was never the engine: `Document::mask` handed
//! back a `MaskRef` borrowing the document — it had to, since the handle may
//! not outlive it — while every masked verb in the wrapper wanted that handle
//! *and* the document together, which Rust cannot spell. So `ClayDocument`
//! kept each subtool's mask in a standalone `clay_mask_create` and lost it the
//! moment a file was closed.
//!
//! That is closed. `claycore::MaskSource` names the mask's **layer** instead of
//! lending the handle, `Document::layer_mask` lends one through a *shared*
//! borrow, and `Document::voxel_layer_masked` hands over a grid and its
//! layer's mask out of one borrow — so the resolution happens inside this
//! crate, where the document pointer and the mask pointer coexist for the
//! length of one C call and neither escapes.
//!
//! What is left here is the boundary measurement itself, which fails the day
//! the engine stops serializing masks. `clayspace-engine`'s
//! `mask_persistence.rs` is the property one layer up: paint, save, reopen,
//! and the same region is still frozen and still gating.

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
