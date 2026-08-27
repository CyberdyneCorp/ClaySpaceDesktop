//! What a mesh brush does to a mesh layer, drawn.
//!
//! These could not be taken until the viewport could draw a mesh layer at all.
//! A mesh layer is in neither the tape nor the brick cache, so the surface the
//! viewport reassembles from bricks cannot contain one — an imported mesh had
//! never been drawn, only carried and exported, and nothing had needed it to
//! be because nothing could change one. Mesh sculpting made that the
//! difference between implemented and usable.
//!
//! The fixture is the return trip in miniature: export the starting form and
//! import it back, which is the only route a mesh layer has into a document.

mod support;

use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ExchangeModel, ExportSettings, GestureSample, ImportSettings, Representation,
    SceneModel, SculptModel, ToolKind,
};
use clayspace_view::{Camera, Vertex};
use support::{save, Harness};

fn with_imported_mesh(who: &str) -> Option<(ClayDocument, std::path::PathBuf)> {
    let policy = BackendPolicy::discover(None).ok()?;
    let mut document = ClayDocument::new(policy)
        .and_then(ClayDocument::with_starting_form)
        .ok()?;
    let path = std::env::temp_dir().join(format!("clayspace-visual-mesh-{who}.obj"));
    let _ = std::fs::remove_file(&path);
    document
        .export_mesh(&path, ExportSettings::default())
        .ok()?;
    document
        .import_mesh(&path, ImportSettings::default())
        .ok()?;
    let key = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)?;
    document.set_active_layer(key).ok()?;
    Some((document, path))
}

/// The mesh layers, as the application uploads them.
fn upload(harness: &mut Harness, document: &mut ClayDocument) {
    let (positions, normals, colors, indices) = document.visible_mesh_geometry();
    let vertices: Vec<Vertex> = positions
        .into_iter()
        .zip(normals)
        .zip(colors)
        .map(|((position, normal), color)| Vertex {
            position,
            normal,
            color,
            mask: 0.0,
        })
        .collect();
    let gpu = harness.gpu.clone();
    harness.renderer.set_mesh_layers(&gpu, &vertices, &indices);
}

/// The one that says the viewport draws a mesh layer at all.
#[test]
fn an_imported_mesh_layer_is_drawn() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let Some((mut document, path)) = with_imported_mesh("drawn") else {
        return;
    };
    let camera = Camera::default();
    let nothing = clayspace_view::GpuMesh::new(&harness.gpu);

    // Nothing uploaded: the frame is the bare ground.
    let empty = harness
        .target
        .capture(&harness.gpu, &harness.renderer, &camera, &nothing, false);

    upload(&mut harness, &mut document);
    let drawn = harness
        .target
        .capture(&harness.gpu, &harness.renderer, &camera, &nothing, false);
    save(&drawn, "70-mesh-layer-drawn");

    let differing = empty
        .pixels
        .chunks_exact(4)
        .zip(drawn.pixels.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        differing > 5000,
        "the mesh layer drew nothing ({differing} pixels differ from an empty \
         frame), so a sculpted mesh would move and show nothing"
    );
    let _ = std::fs::remove_file(&path);
}

/// And that a brush's effect reaches the picture.
#[test]
fn a_mesh_brush_changes_what_is_drawn() {
    let Some(mut harness) = Harness::new() else {
        return;
    };
    let camera = Camera::default();
    let nothing = clayspace_view::GpuMesh::new(&harness.gpu);

    for tool in [ToolKind::Padrao, ToolKind::Inflar, ToolKind::Pincar] {
        let Some((mut document, path)) = with_imported_mesh(&format!("{tool:?}")) else {
            return;
        };
        upload(&mut harness, &mut document);
        let before =
            harness
                .target
                .capture(&harness.gpu, &harness.renderer, &camera, &nothing, false);

        let brush = BrushSettings {
            size: 0.5,
            intensity: 1.0,
            ..BrushSettings::default()
        };
        let samples: Vec<GestureSample> = (0..6)
            .map(|i| {
                let t = i as f32 / 5.0;
                GestureSample {
                    position: [-0.3 + t * 0.6, 0.0, 1.0],
                    pressure: 1.0,
                    time: t,
                }
            })
            .collect();
        document
            .apply_stroke(tool, brush, &samples, [false; 3])
            .unwrap_or_else(|e| panic!("{} was refused: {e}", tool.label()));

        upload(&mut harness, &mut document);
        let after =
            harness
                .target
                .capture(&harness.gpu, &harness.renderer, &camera, &nothing, false);
        save(&after, &format!("70-mesh-{}", tool.label().to_lowercase()));

        // Past `RENDER_NOISE` rather than byte-exact, for the reason
        // `visual_to_mesh` measured: a macOS runner leaves 1,294 pixels
        // byte-differing on an unchanged 3D frame, none past the threshold, so
        // a byte count with a floor of 100 was satisfied there by the
        // rasteriser alone whether or not a brush had run.
        //
        // The floor drops to 50 because the honest instrument tells the three
        // tools apart, and one of them is far subtler than the others:
        //
        //   Padrão   2,286 past the noise   (12,737 byte-differing)
        //   Inflar   1,701                  (12,861)
        //   Pinçar      97                  ( 8,541)
        //
        // Pinçar moves a lot of pixels a little — it draws the surface
        // sideways, which shifts shading far less than pushing it out does.
        // 50 sits above the zero an unchanged frame gives and below the least
        // that any of the three actually produces.
        let differing = support::differing_pixels(&before, &after);
        assert!(
            differing > 50,
            "{} left no mark on the drawn mesh ({differing} pixels past the \
             noise floor)",
            tool.label()
        );
        let _ = std::fs::remove_file(&path);
    }
}
