//! A finished retopology held as a preview, then accepted or discarded.
//!
//! The ViewModel holds the result between the job landing and the sculptor's
//! decision; what the document sees is the three model calls either side of
//! that wait. These drive those calls on a real document and hold the two
//! promises the preview makes: **accepting places exactly what publishing on
//! landing placed**, and **discarding leaves the document byte for byte as it
//! was**, history included.

use clayspace_engine::{BackendPolicy, ClayDocument, EngineRetopologiser};
use clayspace_model::{
    Direction, DocumentModel, LayerKey, RetopoModel, RetopoResult, RetopoSettings, RetopoUv,
    Retopologiser, SceneModel, SculptModel, UvSettings,
};

fn meshed() -> Option<ClayDocument> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    document.convert_layer(Direction::SdfToMesh, 0.05, 0).ok()?;
    Some(document)
}

fn with_uvs() -> RetopoSettings {
    RetopoSettings {
        target_quads: 600,
        uv: Some(UvSettings {
            max_chart_distortion: 0.0,
            ..UvSettings::default()
        }),
        ..RetopoSettings::default()
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("clayspace-retopo-preview");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let path = dir.join(name);
    let _ = std::fs::remove_file(&path);
    path
}

fn active_key(document: &ClayDocument) -> LayerKey {
    document.scene().active_layer().expect("active").key
}

/// Steps one and two of the job — read here, retopologise — without step
/// three, which is where the ViewModel now holds the result.
fn held(document: &mut ClayDocument, settings: RetopoSettings) -> RetopoResult {
    let source = document.retopo_source().expect("the source reads");
    EngineRetopologiser
        .run(&source, settings, &|_, _| {}, &|| false)
        .expect("the retopology runs")
}

/// What a document holds, as a file: the one account of it that includes
/// every layer, transform and UV at once.
fn saved_bytes(document: &mut ClayDocument, name: &str) -> Vec<u8> {
    let path = scratch(name);
    document.save(&path).expect("save");
    std::fs::read(&path).expect("the file reads back")
}

/// Everything a mesh layer holds that a placement could have got wrong.
fn layer_mesh(document: &mut ClayDocument, key: LayerKey) -> clayspace_model::UvPreview {
    document
        .uv_preview(key)
        .expect("readable")
        .expect("the layer carries a layout")
}

/// Accepting a held preview places what publishing on landing placed, bit for
/// bit: the same layer, the same name, the same history step — and the
/// preview drawn while it was held is exactly the accepted layer the viewport
/// draws afterwards.
#[test]
fn an_accepted_preview_is_what_publishing_placed() {
    let (Some(mut published), Some(mut accepted)) = (meshed(), meshed()) else {
        return;
    };
    // Moved, so the preview has a placement to get right.
    for document in [&mut published, &mut accepted] {
        let source = active_key(document);
        document
            .set_layer_transform(source, [0.5, -0.25, 0.0], 1.5)
            .expect("the source moves");
    }
    let depth = accepted.history_depth();
    let layers = accepted.scene().layers.len();

    // Today's path: run and place in one go.
    let outcome = published
        .retopologise(with_uvs())
        .expect("the retopology runs");
    assert!(matches!(outcome.uv, RetopoUv::Laid(_)), "{:?}", outcome.uv);

    // The preview path: run, hold, draw, then accept.
    let result = held(&mut accepted, with_uvs());
    let preview = accepted.retopo_preview(&result).expect("the held preview");
    assert_eq!(
        accepted.scene().layers.len(),
        layers,
        "holding placed something"
    );
    assert_eq!(accepted.history_depth(), depth, "holding banked a step");
    accepted
        .place_retopology(&result, with_uvs())
        .expect("the preview is accepted");

    assert_eq!(accepted.history_depth(), depth + 1, "not one undo step");
    assert_eq!(published.history_depth(), depth + 1);
    let (from_publish, from_accept) = (active_key(&published), active_key(&accepted));
    let published_layer = layer_mesh(&mut published, from_publish);
    let accepted_layer = layer_mesh(&mut accepted, from_accept);
    assert_eq!(
        published.scene().active_layer().map(|l| l.name.clone()),
        accepted.scene().active_layer().map(|l| l.name.clone())
    );
    assert_eq!(
        (&accepted_layer.positions, &accepted_layer.uvs),
        (&published_layer.positions, &published_layer.uvs),
        "the accepted layer differs from the one publishing placed"
    );
    assert_eq!(accepted_layer.normals, published_layer.normals);
    assert_eq!(accepted_layer.indices, published_layer.indices);

    // And what was drawn while it was held is what is drawn now.
    assert_eq!(preview.positions, accepted_layer.positions);
    assert_eq!(preview.normals, accepted_layer.normals);
    assert_eq!(preview.uvs, accepted_layer.uvs);
    assert_eq!(preview.indices, accepted_layer.indices);
    let islands = clayspace_model::uv_islands(&preview.positions, &preview.indices);
    let RetopoUv::Laid(report) = result.outcome.uv else {
        unreachable!("checked above on the published run");
    };
    println!(
        "held preview: {} vertices, {} triangles, {} islands for {} charts, \
         {} seams for {} seam edges",
        preview.positions.len(),
        preview.indices.len() / 3,
        islands.count,
        report.charts,
        islands.seams.len(),
        report.seam_edges
    );
    assert_eq!(islands.count, report.charts as usize);
    assert_eq!(islands.seams.len(), report.seam_edges);
}

/// Discarding leaves the document as it was: the same file byte for byte,
/// the same history depth, the same next undo — and nothing left over that a
/// later accept could place.
#[test]
fn a_discarded_preview_leaves_the_document_and_history_untouched() {
    let Some(mut document) = meshed() else {
        return;
    };
    let before = saved_bytes(&mut document, "before-discard.clayspace");
    let depth = document.history_depth();
    let layers = document.scene().layers.len();

    let result = held(&mut document, with_uvs());
    document.retopo_preview(&result).expect("the held preview");
    document.discard_retopology();

    let after = saved_bytes(&mut document, "after-discard.clayspace");
    println!(
        "saved document: {} bytes before, {} after",
        before.len(),
        after.len()
    );
    assert!(
        before == after,
        "a discarded preview changed the saved document"
    );
    assert_eq!(document.history_depth(), depth);
    assert!(
        document.place_retopology(&result, with_uvs()).is_err(),
        "a discarded preview could still be placed"
    );
    assert_eq!(document.scene().layers.len(), layers);

    // The next undo is the conversion before the preview, not the preview.
    assert!(document.undo().expect("undo"), "nothing to undo");
    assert_eq!(
        document.scene().layers.len(),
        layers - 1,
        "the undo after a discard took back something other than the conversion"
    );
}

/// A preview is not in the document, so a save writes none of it and leaves
/// it acceptable; the file reopens without it.
#[test]
fn a_save_writes_no_preview_and_keeps_it_acceptable() {
    let Some(mut document) = meshed() else {
        return;
    };
    let layers = document.scene().layers.len();
    let result = held(&mut document, with_uvs());
    let revision = document.retopo_source_revision().expect("recorded");
    let path = scratch("held.clayspace");
    document.save(&path).expect("save");
    assert_eq!(
        document.retopo_source_revision().ok(),
        Some(revision),
        "a save moved the source, so it would drop the preview"
    );

    let mut reopened = meshed().expect("a second document");
    reopened.open(&path).expect("open");
    assert_eq!(
        reopened.scene().layers.len(),
        layers,
        "the saved file carries the preview"
    );

    document
        .place_retopology(&result, with_uvs())
        .expect("the preview is still acceptable after a save");
    assert_eq!(document.scene().layers.len(), layers + 1);
}

/// An undo while a preview is held moves its source, and a source that moved
/// cannot receive it: the preview is stale and the document takes nothing.
#[test]
fn an_undo_leaves_a_held_preview_stale() {
    let Some(mut document) = meshed() else {
        return;
    };
    let source = active_key(&document);
    document
        .set_layer_transform(source, [0.25, 0.0, 0.0], 1.0)
        .expect("the source moves");
    let result = held(&mut document, RetopoSettings::default());
    let revision = document.retopo_source_revision().expect("recorded");

    assert!(document.undo().expect("undo"), "nothing to undo");
    assert_ne!(
        document.retopo_source_revision().ok(),
        Some(revision),
        "an undo left the preview's source where it was"
    );
    let layers = document.scene().layers.len();
    let depth = document.history_depth();
    assert!(document
        .place_retopology(&result, RetopoSettings::default())
        .is_err());
    assert_eq!(document.scene().layers.len(), layers);
    assert_eq!(document.history_depth(), depth);
}

/// A held result carrying no layout is drawn from its own triangles, with
/// normals derived from them, standing where the source stands.
#[test]
fn a_preview_without_uvs_stands_where_the_source_does() {
    let Some(mut document) = meshed() else {
        return;
    };
    let source = active_key(&document);
    document
        .set_layer_transform(source, [0.0, 1.0, 0.0], 2.0)
        .expect("the source moves");
    let result = held(
        &mut document,
        RetopoSettings {
            target_quads: 600,
            ..RetopoSettings::default()
        },
    );
    let preview = document.retopo_preview(&result).expect("the held preview");
    assert_eq!(preview.layer, source, "it does not stand in for the source");
    assert!(preview.uvs.is_empty());
    assert_eq!(preview.indices, result.indices);
    assert_eq!(preview.normals.len(), preview.positions.len());
    // Placed by the source's transform: scale two about the origin, then up
    // by one.
    for (drawn, own) in preview.positions.iter().zip(&result.positions) {
        for axis in 0..3 {
            let lift = if axis == 1 { 1.0 } else { 0.0 };
            assert!((drawn[axis] - (own[axis] * 2.0 + lift)).abs() < 1e-4);
        }
    }
}
