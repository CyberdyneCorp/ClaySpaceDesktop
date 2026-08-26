//! Placing a shape, moving it, changing it, and taking it away.
//!
//! The bridge half of a boolean workflow: an item added to a layer keeps a
//! node id, and everything a sculptor does to a placed object afterwards
//! addresses it by that id. What is checked here is that each of those calls
//! reaches the field — that a moved subtraction moves its cavity, that an
//! exchanged primitive keeps the transform, and that the influence bound
//! answers with the three states the engine documents rather than two.

use claycore::{Blend, Document, Influence, Item, LayerId, Op, Primitive};

/// A unit sphere to cut into.
fn form() -> (Document, LayerId) {
    let mut doc = Document::new().expect("create document");
    let layer = doc.add_sdf_layer("Base").expect("add layer");
    let item = Item::sphere(1.0).expect("sphere");
    doc.add_item(layer, &item).expect("place");
    (doc, layer)
}

/// Whether a point is inside the surface.
fn inside(doc: &Document, at: [f32; 3]) -> bool {
    doc.eval_points(None, &[at]).expect("evaluate")[0] < 0.0
}

// -- the primitives ---------------------------------------------------------

#[test]
fn every_offered_primitive_builds() {
    // Not a list written twice: each is built with the parameters its own
    // variant names, and the engine rejects a wrong count, so this fails if a
    // parameter block is in the wrong order or the wrong length.
    let shapes = [
        Primitive::Sphere { radius: 0.5 },
        Primitive::Box { half: [0.4; 3] },
        Primitive::RoundBox {
            half: [0.4; 3],
            radius: 0.1,
        },
        Primitive::BoxFrame {
            half: [0.4; 3],
            thickness: 0.05,
        },
        Primitive::Torus {
            major: 0.5,
            minor: 0.15,
        },
        Primitive::Capsule {
            from: [0.0, -0.3, 0.0],
            to: [0.0, 0.3, 0.0],
            radius: 0.2,
        },
        Primitive::Cylinder {
            radius: 0.3,
            half_height: 0.5,
        },
        Primitive::RoundedCylinder {
            radius: 0.3,
            rim: 0.05,
            half_height: 0.5,
        },
        Primitive::Cone {
            half_height: 0.5,
            bottom: 0.4,
            top: 0.1,
        },
        Primitive::Ellipsoid {
            radii: [0.5, 0.3, 0.2],
        },
        Primitive::Octahedron { size: 0.5 },
        Primitive::HexPrism {
            radius: 0.4,
            half_depth: 0.2,
        },
        Primitive::TriPrism {
            radius: 0.4,
            half_depth: 0.2,
        },
        Primitive::Pyramid { height: 0.6 },
        Primitive::Tetrahedron { radius: 0.5 },
        Primitive::Dodecahedron { radius: 0.5 },
        Primitive::Icosahedron { radius: 0.5 },
        Primitive::CutSphere {
            radius: 0.5,
            at: 0.2,
        },
        Primitive::ExactCone {
            half_angle: 0.5,
            height: 0.6,
        },
        Primitive::SolidAngle {
            half_angle: 0.6,
            radius: 0.5,
        },
    ];

    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Shapes").expect("layer");
    for shape in shapes {
        let item = Item::of(shape).unwrap_or_else(|e| panic!("{shape:?} was refused: {e}"));
        doc.add_item(layer, &item)
            .unwrap_or_else(|e| panic!("{shape:?} could not be placed: {e}"));
    }
}

#[test]
fn a_primitive_carries_its_own_parameter_order() {
    // A torus is the one where transposing the two radii still builds, which
    // is exactly why the enum exists: major then minor, and a ring 0.5 across
    // is empty at its own centre.
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Ring").expect("layer");
    let item = Item::of(Primitive::Torus {
        major: 0.5,
        minor: 0.1,
    })
    .expect("torus");
    doc.add_item(layer, &item).expect("place");

    assert!(!inside(&doc, [0.0, 0.0, 0.0]), "a ring's centre is empty");
    assert!(inside(&doc, [0.5, 0.0, 0.0]), "the tube is at the radius");
}

// -- placed nodes -----------------------------------------------------------

#[test]
fn a_subtracted_object_moves_its_cavity() {
    let (mut doc, layer) = form();

    let mut cut = Item::of(Primitive::Cylinder {
        radius: 0.3,
        half_height: 2.0,
    })
    .expect("cylinder");
    cut.set_op(Op::Subtract).expect("op");
    cut.set_position([0.5, 0.0, 0.0]).expect("position");
    let node = doc.add_item(layer, &cut).expect("place");

    assert!(
        !inside(&doc, [0.5, 0.0, 0.0]),
        "the bore is where it was put"
    );
    assert!(inside(&doc, [-0.5, 0.0, 0.0]), "the far side is untouched");

    // The whole point of a live operand: the same node, somewhere else.
    doc.set_node_transform(layer, node, [-0.5, 0.0, 0.0], [0.0, 1.0, 0.0], 0.0, 1.0)
        .expect("retransform");

    assert!(inside(&doc, [0.5, 0.0, 0.0]), "the old bore closed");
    assert!(!inside(&doc, [-0.5, 0.0, 0.0]), "the bore moved with it");
}

#[test]
fn an_exchanged_primitive_keeps_the_transform() {
    let (mut doc, layer) = form();
    let mut cut = Item::of(Primitive::Box { half: [0.2; 3] }).expect("box");
    cut.set_op(Op::Subtract).expect("op");
    let node = doc.add_item(layer, &cut).expect("place");
    doc.set_node_transform(layer, node, [0.6, 0.0, 0.0], [0.0, 1.0, 0.0], 0.0, 1.0)
        .expect("place it off-centre");

    doc.set_node_prim(
        layer,
        node,
        Primitive::Cylinder {
            radius: 0.25,
            half_height: 2.0,
        },
    )
    .expect("exchange the shape");

    assert!(
        !inside(&doc, [0.6, 0.0, 0.0]),
        "the new shape cuts where the old one stood"
    );
    assert!(
        inside(&doc, [-0.6, 0.0, 0.0]),
        "and did not return to the origin"
    );
}

#[test]
fn an_operation_can_be_changed_after_placement() {
    let (mut doc, layer) = form();
    let mut lump = Item::of(Primitive::Sphere { radius: 0.4 }).expect("sphere");
    lump.set_op(Op::Add).expect("op");
    lump.set_position([1.0, 0.0, 0.0]).expect("position");
    let node = doc.add_item(layer, &lump).expect("place");

    assert!(inside(&doc, [1.2, 0.0, 0.0]), "added material is there");

    doc.set_node_op_blend(layer, node, Op::Subtract, Blend::Hard, 0.0, 0.0)
        .expect("re-op");

    assert!(
        !inside(&doc, [1.0, 0.0, 0.0]),
        "the same node now cuts instead"
    );
}

#[test]
fn removing_an_object_restores_the_surface() {
    let (mut doc, layer) = form();
    let mut cut = Item::of(Primitive::Sphere { radius: 0.5 }).expect("sphere");
    cut.set_op(Op::Subtract).expect("op");
    cut.set_position([0.8, 0.0, 0.0]).expect("position");
    let node = doc.add_item(layer, &cut).expect("place");
    assert!(!inside(&doc, [0.8, 0.0, 0.0]), "the cavity is cut");

    doc.remove_node(layer, node).expect("remove");
    assert!(inside(&doc, [0.8, 0.0, 0.0]), "and is gone with the object");
}

// -- what a move has to dirty -----------------------------------------------

#[test]
fn a_local_object_reports_the_box_it_reaches() {
    let (mut doc, layer) = form();
    let mut cut = Item::of(Primitive::Box { half: [0.2; 3] }).expect("box");
    cut.set_op(Op::Subtract).expect("op");
    cut.set_position([0.5, 0.0, 0.0]).expect("position");
    let node = doc.add_item(layer, &cut).expect("place");

    let Influence::Box { min, max } = doc.node_influence_bound(layer, node).expect("bound") else {
        panic!("a subtracted box is local and should report a finite box");
    };
    assert!(
        min[0] <= 0.3 && max[0] >= 0.7,
        "the box should cover the shape it was placed at, got {min:?} to {max:?}"
    );
}

/// The state that makes an `Option` the wrong return type: this is not
/// "nothing to dirty", it is "dirty everything", and an ordinary cube reaches
/// it through nothing more exotic than the operation a sculptor chose.
#[test]
fn an_intersecting_object_has_no_finite_bound() {
    let (mut doc, layer) = form();
    let mut cut = Item::of(Primitive::Box { half: [0.5; 3] }).expect("box");
    cut.set_op(Op::Intersect).expect("op");
    let node = doc.add_item(layer, &cut).expect("place");

    assert_eq!(
        doc.node_influence_bound(layer, node).expect("bound"),
        Influence::Everything,
        "a non-local op anywhere in the subtree removes the finite bound"
    );
}

#[test]
fn a_node_the_layer_does_not_hold_reports_nothing_rather_than_failing() {
    let (mut doc, layer) = form();
    let mut cut = Item::of(Primitive::Sphere { radius: 0.3 }).expect("sphere");
    cut.set_op(Op::Subtract).expect("op");
    let node = doc.add_item(layer, &cut).expect("place");
    doc.remove_node(layer, node).expect("remove");

    // "A selection outlives the nodes in it" — so this is an answer, and the
    // one that means dirty nothing rather than dirty everything.
    assert_eq!(
        doc.node_influence_bound(layer, node).expect("bound"),
        Influence::Nothing
    );
}

// -- refusals ---------------------------------------------------------------

#[test]
fn a_scale_of_zero_is_refused() {
    let (mut doc, layer) = form();
    let item = Item::sphere(0.3).expect("sphere");
    let node = doc.add_item(layer, &item).expect("place");

    assert!(
        doc.set_node_transform(layer, node, [0.0; 3], [0.0, 1.0, 0.0], 0.0, 0.0)
            .is_err(),
        "scale must be greater than zero"
    );
}

#[test]
fn transforming_a_node_the_layer_does_not_hold_is_refused() {
    let (mut doc, layer) = form();
    let item = Item::sphere(0.3).expect("sphere");
    let node = doc.add_item(layer, &item).expect("place");
    doc.remove_node(layer, node).expect("remove");

    assert!(
        doc.set_node_transform(layer, node, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.0, 1.0)
            .is_err(),
        "a node the document does not hold should be refused rather than ignored"
    );
}

// -- meshes -----------------------------------------------------------------

#[test]
fn a_transformed_mesh_is_the_same_mesh_moved() {
    let (doc, _) = form();
    let mesh = doc
        .mesh(claycore::MeshParams::default())
        .expect("mesh the form");
    let (before_min, _) = mesh.bounds().expect("bounds");

    let moved = mesh
        .transformed([2.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.0, 1.0)
        .expect("transform");
    let (after_min, _) = moved.bounds().expect("bounds");

    assert_eq!(
        moved.index_count(),
        mesh.index_count(),
        "a transform rewrites positions and touches no index"
    );
    assert!(
        (after_min[0] - before_min[0] - 2.0).abs() < 1e-3,
        "the mesh should have moved two units along x, got {before_min:?} to {after_min:?}"
    );
}

// -- what the side-car rests on ---------------------------------------------

/// The application has to keep an object's transform and parameters itself,
/// because the ABI sets them and never reads them back. That table is keyed by
/// node id, so it is worth nothing unless an id means the same node after a
/// save and a reopen. The header says the format records ids — "the one the
/// document format records, so a binding edit means exactly what a saved
/// document means" — and this is that claim, checked.
#[test]
fn a_node_id_survives_a_save_and_a_reopen() {
    let path = std::env::temp_dir().join("claycore-node-id-stability.clay");
    let _ = std::fs::remove_file(&path);

    let (mut doc, layer) = form();
    let mut first = Item::of(Primitive::Box { half: [0.2; 3] }).expect("box");
    first.set_op(Op::Subtract).expect("op");
    let a = doc.add_item(layer, &first).expect("place");
    let second = Item::of(Primitive::Sphere { radius: 0.2 }).expect("sphere");
    let b = doc.add_item(layer, &second).expect("place");
    // A gap in the id space, which is the case a naive scheme gets wrong.
    doc.remove_node(layer, a).expect("remove the first");
    doc.save(&path).expect("save");

    let reopened = Document::open(&path).expect("reopen");
    let layers = reopened.layer_ids().expect("layers");
    let layer = *layers.first().expect("a layer");
    let nodes = reopened.layer_nodes(layer).expect("nodes");

    assert!(
        nodes.iter().any(|node| node.get() == b.get()),
        "the surviving node's id changed across a save: {:?} does not hold {}",
        nodes.iter().map(|n| n.get()).collect::<Vec<_>>(),
        b.get()
    );
    assert!(
        !nodes.iter().any(|node| node.get() == a.get()),
        "a removed node's id came back"
    );
    let _ = std::fs::remove_file(&path);
}
