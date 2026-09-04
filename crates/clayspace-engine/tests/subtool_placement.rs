//! Where a whole subtool stands, and who remembers it.
//!
//! The engine's own answer, since ClayCore ABI 0.74.0 gave the boundary
//! `clay_document_layer_transform` and its per-axis sibling (#373). Before
//! them the ABI set a layer transform and would not read one back, so this
//! application kept two mechanisms over the absence: a snapshot of every
//! layer's placement against every undo depth, and — because a file could not
//! be asked either — an assumption that a reopened subtool stood at the
//! origin. The first is gone and the second was a defect; these are the tests
//! that hold both closed.

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    DocumentModel, GizmoTarget, ObjectModel, SceneModel, SculptModel, Transform,
};

fn document() -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .expect("a document with a starting form")
}

/// Opens a document from a path, on the same terms `document` builds one.
fn document_at(path: &std::path::Path) -> ClayDocument {
    let mut opened = document();
    opened.open(path).expect("open");
    opened
}

/// Where a subtool stands survives a save and a reopen, squash included.
///
/// This is the round trip the format's minor 16 is about: a layer record
/// carries a per-axis `scale_axes` triple, and a `.clayspace` written by this
/// build carries it. Written and read by this build, which is the pairing that
/// has to hold — an older build refuses the file rather than misreading it,
/// and this workspace exchanges documents with no older build.
#[test]
fn a_squashed_subtool_reopens_squashed_and_where_it_stood() {
    let directory = std::env::temp_dir().join("clayspace-subtool-stretch-reopen");
    std::fs::create_dir_all(&directory).expect("a place to write");
    let path = directory.join("squashed.clayspace");

    let mut document = document();
    let key = document.scene().active.expect("an active layer");
    let target = GizmoTarget::Layer(key);
    let placed = Transform {
        position: [0.75, -0.25, 0.5],
        rotation_axis: [0.0, 1.0, 0.0],
        rotation_angle: 0.0,
        scale: [2.0, 1.0, 0.5],
    };
    document
        .set_target_transform(target, placed)
        .expect("place and squash");
    document.save(&path).expect("save");

    // What the file itself says it was written at, so the format decision is
    // measured rather than assumed.
    let format = claycore::Document::format_of(&path).expect("a readable header");
    assert_eq!(
        format,
        claycore::Document::FORMAT,
        "this build wrote a document at {format}, not at the minor it claims"
    );

    let mut reopened = document_at(&path);
    let key = reopened.scene().active.expect("an active layer");
    let read_back = reopened
        .target_transform(GizmoTarget::Layer(key))
        .expect("a transform");
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        read_back.scale, placed.scale,
        "the squash did not survive the round trip"
    );
    assert_eq!(
        read_back.position, placed.position,
        "a moved subtool came back at the origin: until ClayCore 0.74.0 there \
         was no call that answered where a layer stands, so every reopened \
         layer was assumed to be at one"
    );
}

/// And it survives the history, without the application keeping its own record
/// of where every layer stood at every undo depth.
///
/// That record existed because the ABI set a layer transform and would not
/// read one back. `clay_document_layer_transform_nonuniform` reads one back,
/// so the engine's own answer is what the cache is refreshed from — which also
/// means an undo it did not expect is followed rather than overwritten.
#[test]
fn undoing_a_stretch_puts_the_subtool_back_and_redoing_stretches_it_again() {
    let mut document = document();
    let key = document.scene().active.expect("an active layer");
    let target = GizmoTarget::Layer(key);
    let current = document.target_transform(target).expect("a transform");

    document
        .set_target_transform(
            target,
            Transform {
                scale: [3.0, 1.0, 1.0],
                ..current
            },
        )
        .expect("stretch");
    assert_eq!(
        document
            .target_transform(target)
            .expect("a transform")
            .scale,
        [3.0, 1.0, 1.0]
    );

    document.undo().expect("undo");
    assert_eq!(
        document
            .target_transform(target)
            .expect("a transform")
            .scale,
        [1.0; 3],
        "the cached placement stayed where the drag left it"
    );

    document.redo().expect("redo");
    assert_eq!(
        document
            .target_transform(target)
            .expect("a transform")
            .scale,
        [3.0, 1.0, 1.0],
        "the stretch could not be put back"
    );
}
