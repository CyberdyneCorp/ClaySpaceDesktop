//! An adaptive surface under the brush uploads the chunks the stroke dirtied,
//! not the model (#209).
//!
//! Measured along the application's own path: the carried buffer built by
//! `visible_mesh_geometry`, then the patch `clayspace_app::carried` turns a
//! stroke into. Correctness is that the patched buffer draws exactly the
//! triangles a fresh build draws; cost is the bytes the patch weighs, against
//! the whole surface and across two model sizes.
mod support;

use clayspace_app::carried::{patch_upload, rebuilt_bytes, PatchUpload};
use clayspace_engine::{BackendPolicy, ClayDocument};
use clayspace_model::{
    BrushSettings, ConversionSettings, Direction, ExchangeModel, GestureSample, ImportSettings,
    Representation, SceneModel, SculptModel, ToolKind,
};
use clayspace_view::{MeshSpan, Vertex};

// -- fixtures ---------------------------------------------------------------

fn scratch(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "clayspace-dynamic-uploads-{name}-{}.obj",
        std::process::id()
    ))
}

/// A flat quad sheet facing +y, `divisions` quads a side, `step` apart.
fn sheet_obj(path: &std::path::Path, divisions: usize, step: f32) {
    use std::fmt::Write;
    let half = step * divisions as f32 / 2.0;
    let mut text = String::new();
    for z in 0..=divisions {
        for x in 0..=divisions {
            let _ = writeln!(
                text,
                "v {} 0 {}",
                -half + step * x as f32,
                -half + step * z as f32
            );
        }
    }
    let stride = divisions + 1;
    for z in 0..divisions {
        for x in 0..divisions {
            let a = z * stride + x + 1;
            let _ = writeln!(text, "f {} {} {} {}", a, a + stride, a + stride + 1, a + 1);
        }
    }
    std::fs::write(path, text).expect("write the sheet");
}

/// A document holding one sheet, crossed in place into an adaptive surface.
fn surface(who: &str, divisions: usize, step: f32) -> ClayDocument {
    let policy = BackendPolicy::discover(None).expect("discover backends");
    let mut document = ClayDocument::new(policy).expect("a document");
    let path = scratch(who);
    sheet_obj(&path, divisions, step);
    document
        .import_mesh(&path, ImportSettings::default())
        .expect("import the sheet");
    let _ = std::fs::remove_file(&path);
    let mesh = document
        .scene()
        .layers
        .iter()
        .find(|layer| layer.representation == Representation::Mesh)
        .map(|layer| layer.key)
        .expect("the sheet is a mesh layer");
    document.set_active_layer(mesh).expect("activate the sheet");
    let settings = ConversionSettings::default();
    document
        .convert_layer_in_place(Direction::MeshToDynamic, settings.cell_size, settings.blur)
        .expect("a welded sheet is an adaptive surface");
    document
}

/// A short Draw stroke across the middle of the sheet, a brush `size` wide.
fn stroke(document: &mut ClayDocument, size: f32) {
    document.begin_gesture();
    let samples: Vec<GestureSample> = (0..=6)
        .map(|step| {
            let t = step as f32 / 6.0;
            GestureSample {
                position: [-0.2 + 0.4 * t, 0.0, 0.0],
                pressure: 1.0,
                time: t,
            }
        })
        .collect();
    let outcome = document.apply_stroke(
        ToolKind::Padrao,
        BrushSettings {
            size,
            intensity: 0.8,
            ..BrushSettings::default()
        },
        &samples,
        [false; 3],
    );
    document.end_gesture();
    assert!(
        outcome.expect("the stroke is applied").changed,
        "the stroke landed"
    );
}

/// The carried buffer as the application builds it, with the build number
/// the renderer would be holding.
struct Built {
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
    build: u64,
    bytes: u64,
    chunks: usize,
}

fn build(document: &mut ClayDocument) -> Built {
    let (positions, _, _, indices, _) = document.visible_mesh_geometry();
    let upload = document.dynamic_upload();
    assert!(upload.rebuilt);
    Built {
        positions,
        indices,
        build: document.carried_build(),
        bytes: rebuilt_bytes(upload),
        chunks: upload.chunks,
    }
}

impl Built {
    /// Writes a patch into this copy, as the renderer writes it into its own.
    fn apply(&mut self, patch: &PatchUpload) {
        for (first, run) in &patch.vertices {
            let at = *first as usize;
            for (into, vertex) in self.positions[at..at + run.len()].iter_mut().zip(run) {
                *into = vertex.position;
            }
        }
        for (first, run) in &patch.indices {
            let at = *first as usize;
            self.indices[at..at + run.len()].copy_from_slice(run);
        }
    }

    /// The triangles drawn, degenerate ones left out, in a canonical order.
    fn triangles(&self) -> Vec<[[u32; 3]; 3]> {
        let mut triangles: Vec<[[u32; 3]; 3]> = self
            .indices
            .chunks_exact(3)
            .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
            .map(|t| {
                let corners =
                    [t[0], t[1], t[2]].map(|i| self.positions[i as usize].map(f32::to_bits));
                let first = (0..3).min_by_key(|&k| corners[k]).unwrap_or(0);
                std::array::from_fn(|k| corners[(first + k) % 3])
            })
            .collect();
        triangles.sort_unstable();
        triangles
    }
}

// -- what a stroke sends ------------------------------------------------------

/// A stroke is written into the chunks it dirtied, the patched buffer draws
/// exactly what a rebuild draws, and it weighs a fraction of the surface.
#[test]
fn a_dynamic_stroke_uploads_only_dirty_chunks() {
    let mut document = surface("dirty", 96, 4.0 / 96.0);
    let mut drawn = build(&mut document);
    let before = document.mesh_revision();

    stroke(&mut document, 0.3);
    assert_ne!(document.mesh_revision(), before, "the viewport is told");
    let patch = patch_upload(&mut document, drawn.build).expect("a stroke patches in place");
    assert!(patch.upload.chunks > 0, "the stroke dirtied chunks");
    assert!(
        patch.upload.chunks * 4 < drawn.chunks,
        "{} of {} chunks sent",
        patch.upload.chunks,
        drawn.chunks
    );
    assert!(
        patch.bytes() * 4 < drawn.bytes,
        "{} bytes patched against {} for the whole surface",
        patch.bytes(),
        drawn.bytes
    );

    drawn.apply(&patch);
    let fresh = build(&mut document);
    assert_eq!(
        drawn.triangles(),
        fresh.triangles(),
        "the patched buffer draws what a rebuild draws"
    );
}

/// Nothing dirty, nothing sent: asked again with no stroke between, the
/// patch is empty rather than a second copy of the last one.
#[test]
fn a_patch_is_not_sent_twice() {
    let mut document = surface("twice", 48, 4.0 / 48.0);
    let drawn = build(&mut document);
    stroke(&mut document, 0.3);
    let first = patch_upload(&mut document, drawn.build).expect("a patch");
    assert!(!first.is_empty());
    let second = patch_upload(&mut document, drawn.build).expect("still patchable");
    assert!(second.is_empty(), "{} bytes sent again", second.bytes());
}

/// An undo puts a surface back from bytes — a new chunk table — so it is
/// not patched: the buffer is built again, and draws the restored surface.
#[test]
fn an_undo_rebuilds_and_draws_the_restored_surface() {
    let mut document = surface("undo", 48, 4.0 / 48.0);
    let flat = build(&mut document);
    stroke(&mut document, 0.3);
    let sculpted = build(&mut document);
    assert_ne!(flat.triangles(), sculpted.triangles());

    assert!(document.undo().expect("undo"));
    assert!(
        patch_upload(&mut document, sculpted.build).is_none(),
        "a restored surface is laid out again, not patched"
    );
    assert_eq!(build(&mut document).triangles(), flat.triangles());
    assert!(document.redo().expect("redo"));
    assert_eq!(build(&mut document).triangles(), sculpted.triangles());
}

/// A patch against a build the renderer does not hold is refused: any other
/// caller of `visible_mesh_geometry` lays the regions out again.
#[test]
fn a_patch_is_only_offered_against_the_build_it_follows() {
    let mut document = surface("stale", 32, 4.0 / 32.0);
    let old = build(&mut document);
    let _newer = build(&mut document);
    stroke(&mut document, 0.3);
    assert!(patch_upload(&mut document, old.build).is_none());
}

/// Same density, ten times the area: the stroke touches the same number of
/// triangles on both, and sends about the same bytes, while the whole
/// surface it would once have re-sent is ten times larger.
#[test]
fn upload_volume_is_independent_of_model_size() {
    let step = 4.0 / 224.0;
    let mut measured = Vec::new();
    for divisions in [224usize, 708] {
        let mut document = surface(&format!("size-{divisions}"), divisions, step);
        let drawn = build(&mut document);
        stroke(&mut document, 0.3);
        let patch = patch_upload(&mut document, drawn.build).expect("a patch");
        eprintln!(
            "{} triangles: {} of {} chunks, {} bytes patched, {} bytes for the surface",
            divisions * divisions * 2,
            patch.upload.chunks,
            drawn.chunks,
            patch.bytes(),
            drawn.bytes
        );
        measured.push((patch.bytes(), drawn.bytes));
    }
    let [(small, small_whole), (large, large_whole)] = [measured[0], measured[1]];
    assert!(large_whole > 8 * small_whole, "the fixture grew tenfold");
    assert!(
        large <= 2 * small && small <= 2 * large,
        "the patch followed the stroke, not the model: {small} bytes at 100k \
         triangles, {large} at 1M"
    );
    assert!(
        large * 100 < large_whole,
        "a hundredth of the surface at most"
    );
}

// -- the renderer ------------------------------------------------------------

/// The renderer writes a patch in place, counting exactly its bytes, and
/// declines — writing nothing — where the polyframe would go stale.
#[test]
fn the_renderer_writes_a_patch_in_place_and_declines_under_the_polyframe() {
    let Some(mut harness) = support::Harness::new() else {
        return;
    };
    let mut document = surface("renderer", 48, 4.0 / 48.0);
    let (positions, normals, colors, indices, spans) = document.visible_mesh_geometry();
    let built = document.carried_build();
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
    let spans: Vec<MeshSpan> = spans
        .into_iter()
        .map(|span| MeshSpan::new(span.layer, span.indices))
        .collect();
    let gpu = harness.gpu.clone();
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);

    stroke(&mut document, 0.3);
    let patch = patch_upload(&mut document, built).expect("a patch");
    harness.gpu.take_uploaded_bytes();
    assert!(patch.apply(&gpu, &mut harness.renderer));
    assert_eq!(harness.gpu.take_uploaded_bytes(), patch.bytes());

    harness.renderer.set_polyframe(&gpu, true);
    // Switching the polyframe on uploads its own edges; only what the patch
    // does after that is measured.
    harness.gpu.take_uploaded_bytes();
    assert!(!patch.apply(&gpu, &mut harness.renderer));
    assert_eq!(harness.gpu.take_uploaded_bytes(), 0, "nothing was written");
}
