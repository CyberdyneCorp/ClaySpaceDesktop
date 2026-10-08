//! An item that carries its own mirror axes (ClayCore #673, ABI 0.121.0).
//!
//! The layer's mirror reflects every item that takes part in it, whenever the
//! item was made, so turning it off or moving it to another axis changed
//! everything made under the old one. An item given its own axes is reflected
//! through those instead, and the layer's mirror no longer reaches it.
//!
//! Measured here as the release measured it: a radius-0.25 lump at
//! (0.6, 0.4, 0) under a layer mirror X. The field at the X twin's centre is
//! -0.25 when the twin is there and 0.95 when it is not; at the Y reflection's
//! centre it is 0.55 unless a Y twin stands there.

use claycore::{Document, ErrorKind, GroupCombine, Item, LayerId, MirrorAxes, NodeId};

const OFF: [bool; 3] = [false; 3];
const X: [bool; 3] = [true, false, false];
const Y: [bool; 3] = [false, true, false];

const LUMP: [f32; 3] = [0.6, 0.4, 0.0];
const X_TWIN: [f32; 3] = [-0.6, 0.4, 0.0];
const Y_TWIN: [f32; 3] = [0.6, -0.4, 0.0];

/// A field layer mirrored across X, with undo recording.
fn mirrored_layer() -> (Document, LayerId) {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    doc.set_layer_mirror(layer, X, 0.0)
        .expect("mirror across x");
    doc.enable_undo().expect("undo");
    (doc, layer)
}

/// The lump of the release's table, carrying `axes`.
fn lump(doc: &mut Document, layer: LayerId, axes: MirrorAxes) -> NodeId {
    let mut item = Item::sphere(0.25).expect("sphere");
    item.set_position(LUMP).expect("place");
    item.set_mirror_axes(axes).expect("own axes");
    doc.add_item(layer, &item).expect("add")
}

fn field(doc: &Document, at: [f32; 3]) -> f32 {
    doc.eval_points(None, &[at]).expect("evaluate")[0]
}

fn assert_field(doc: &Document, at: [f32; 3], expected: f32, what: &str) {
    let value = field(doc, at);
    assert!(
        (value - expected).abs() < 1e-3,
        "{what}: the field at {at:?} is {value}, expected {expected}"
    );
}

/// The release's table, row by row: an item holding X keeps its X twin when
/// the layer's mirror is turned off and when it is pointed at Y, where an
/// inheriting item loses it.
#[test]
fn own_axes_survive_the_layer_mirror_being_turned_off_or_switched() {
    let (mut doc, layer) = mirrored_layer();
    lump(&mut doc, layer, MirrorAxes::Axes(X));
    assert_field(&doc, X_TWIN, -0.25, "as made");
    assert_field(&doc, Y_TWIN, 0.55, "as made");

    doc.set_layer_mirror(layer, OFF, 0.0).expect("mirror off");
    assert_field(&doc, X_TWIN, -0.25, "layer mirror off, item holds X");
    assert_field(&doc, Y_TWIN, 0.55, "layer mirror off, item holds X");

    doc.set_layer_mirror(layer, Y, 0.0)
        .expect("mirror across y");
    assert_field(&doc, X_TWIN, -0.25, "layer mirror Y, item holds X");
    assert_field(&doc, Y_TWIN, 0.55, "layer mirror Y, item holds X");

    // The control: an inheriting item follows the layer, as it always did.
    let (mut doc, layer) = mirrored_layer();
    lump(&mut doc, layer, MirrorAxes::Inherit);
    assert_field(&doc, X_TWIN, -0.25, "inheriting, as made");
    doc.set_layer_mirror(layer, OFF, 0.0).expect("mirror off");
    assert_field(&doc, X_TWIN, 0.95, "inheriting, layer mirror off");
    doc.set_layer_mirror(layer, Y, 0.0)
        .expect("mirror across y");
    assert_field(&doc, X_TWIN, 0.95, "inheriting, layer mirror Y");
    assert_field(&doc, Y_TWIN, -0.25, "inheriting, layer mirror Y");
}

/// An item holding no axes has no twin on a mirrored layer, and the
/// participation flag decides nothing for an item with its own axes.
#[test]
fn an_item_holding_no_axes_has_no_twin_whatever_the_layer_carries() {
    let (mut doc, layer) = mirrored_layer();
    let mut item = Item::sphere(0.25).expect("sphere");
    item.set_position(LUMP).expect("place");
    item.set_mirror(true).expect("would follow the layer");
    item.set_mirror_axes(MirrorAxes::NONE)
        .expect("no axes of its own");
    doc.add_item(layer, &item).expect("add");
    assert_field(&doc, X_TWIN, 0.95, "no own axes on an X layer");
    assert_field(&doc, LUMP, -0.25, "the lump itself");
}

/// The readers report what was stored and what the item is actually
/// reflected through, which differ for an inheriting item.
#[test]
fn reading_back_reports_the_stored_choice_and_the_effective_axes() {
    let mut builder = Item::sphere(0.25).expect("sphere");
    assert_eq!(
        builder.mirror_axes().expect("read"),
        MirrorAxes::Inherit,
        "a builder that never set any inherits"
    );
    builder.set_mirror_axes(MirrorAxes::Axes(Y)).expect("set");
    assert_eq!(builder.mirror_axes().expect("read"), MirrorAxes::Axes(Y));

    let (mut doc, layer) = mirrored_layer();
    let inheriting = lump(&mut doc, layer, MirrorAxes::Inherit);
    let own = lump(&mut doc, layer, MirrorAxes::Axes(Y));
    let none = lump(&mut doc, layer, MirrorAxes::NONE);

    let read = doc.node_mirror(layer, inheriting).expect("read");
    assert_eq!(read.axes, MirrorAxes::Inherit);
    assert_eq!(
        read.effective, X,
        "an inheriting item is reflected through the layer's X"
    );
    assert!(read.reflected);

    let read = doc.node_mirror(layer, own).expect("read");
    assert_eq!(read.axes, MirrorAxes::Axes(Y));
    assert_eq!(read.effective, Y, "own axes replace the layer's");

    let read = doc.node_mirror(layer, none).expect("read");
    assert_eq!(read.axes, MirrorAxes::NONE);
    assert_eq!(read.effective, OFF);

    // Turning the layer's mirror off moves only the inheriting item's answer.
    doc.set_layer_mirror(layer, OFF, 0.0).expect("mirror off");
    assert_eq!(
        doc.node_mirror(layer, inheriting).expect("read").effective,
        OFF
    );
    assert_eq!(doc.node_mirror(layer, own).expect("read").effective, Y);
}

/// A placed item's mirror can be re-decided in one undo step, and undo puts
/// back both values it had.
#[test]
fn a_placed_items_mirror_is_one_undo_step() {
    let (mut doc, layer) = mirrored_layer();
    let node = lump(&mut doc, layer, MirrorAxes::Axes(X));
    doc.set_layer_mirror(layer, OFF, 0.0).expect("mirror off");
    assert_field(&doc, X_TWIN, -0.25, "holding X on a layer with no mirror");
    let depth = doc.undo_state().expect("undo state").undo_depth;

    doc.set_node_mirror(layer, node, false, MirrorAxes::Inherit)
        .expect("back on the layer's mirror, opted out");
    assert_eq!(
        doc.undo_state().expect("undo state").undo_depth,
        depth + 1,
        "one edit, one step"
    );
    let read = doc.node_mirror(layer, node).expect("read");
    assert_eq!((read.reflected, read.axes), (false, MirrorAxes::Inherit));
    assert_field(&doc, X_TWIN, 0.95, "inheriting a layer with no mirror");

    assert!(doc.undo().expect("undo"), "there was a step to undo");
    let read = doc.node_mirror(layer, node).expect("read");
    assert_eq!(
        (read.reflected, read.axes),
        (true, MirrorAxes::Axes(X)),
        "undo did not put back both values"
    );
    assert_field(&doc, X_TWIN, -0.25, "after the undo");
}

/// A group carries no mirror of its own: both the setter and the reader
/// refuse it rather than answer with a control that does not act.
#[test]
fn a_group_is_refused() {
    let (mut doc, layer) = mirrored_layer();
    let group = doc
        .add_group(layer, NodeId::ROOT, None, GroupCombine::default())
        .expect("a group");

    let refused = doc
        .set_node_mirror(layer, group, true, MirrorAxes::Axes(X))
        .expect_err("a group took a mirror");
    assert_eq!(refused.kind(), ErrorKind::InvalidArgument);
    let refused = doc
        .node_mirror(layer, group)
        .expect_err("a group answered what mirror it has");
    assert_eq!(refused.kind(), ErrorKind::InvalidArgument);
}

/// A document with an own-axes item saves at the current format and reopens
/// with the axes it was given, twin included.
#[test]
fn own_axes_survive_a_save_and_a_reopen() {
    let (mut doc, layer) = mirrored_layer();
    let node = lump(&mut doc, layer, MirrorAxes::Axes(X));
    doc.set_layer_mirror(layer, OFF, 0.0).expect("mirror off");

    let dir = std::env::temp_dir().join("claycore-item-mirror-axes");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let path = dir.join("own-axes.clayspace");
    doc.save(&path).expect("save");
    let reopened = Document::open(&path).expect("open");
    let _ = std::fs::remove_file(&path);

    let read = reopened.node_mirror(layer, node).expect("read");
    assert_eq!(
        read.axes,
        MirrorAxes::Axes(X),
        "the file lost the item's axes"
    );
    assert_eq!(read.effective, X);
    assert_field(&reopened, X_TWIN, -0.25, "reopened");
}
