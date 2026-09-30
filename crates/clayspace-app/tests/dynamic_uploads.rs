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
use clayspace_view::renderer::polyframe;
use clayspace_view::MeshSpan;

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
    spans: Vec<MeshSpan>,
    build: u64,
    bytes: u64,
    chunks: usize,
}

fn build(document: &mut ClayDocument) -> Built {
    let (positions, _, _, indices, spans) = document.visible_mesh_geometry();
    let upload = document.dynamic_upload();
    assert!(upload.rebuilt);
    let spans = spans
        .into_iter()
        .map(|span| MeshSpan::new(span.layer, span.indices).chunked(span.chunked))
        .collect();
    Built {
        positions,
        indices,
        spans,
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

    /// The polyframe's lines for this buffer, as the renderer derives them.
    fn lines(&self) -> polyframe::Lines {
        polyframe::lines(&self.indices, &self.spans)
    }

    /// Lines drawn, zero-length ones left out, as position pairs in a
    /// canonical order — duplicates kept, since an edge drawn twice reads
    /// darker than one drawn once.
    fn edges(&self, lines: &[u32]) -> Vec<[[u32; 3]; 2]> {
        let mut edges: Vec<[[u32; 3]; 2]> = lines
            .chunks_exact(2)
            .filter(|line| line[0] != line[1])
            .map(|line| {
                let mut ends =
                    [line[0], line[1]].map(|i| self.positions[i as usize].map(f32::to_bits));
                ends.sort_unstable();
                ends
            })
            .collect();
        edges.sort_unstable();
        edges
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

// -- the polyframe -----------------------------------------------------------

/// A stroke's patch carries the polyframe with it: every index run it sends
/// has its lines in the same slots, and the patched line list draws exactly
/// the edges a fresh derivation over a rebuild draws.
#[test]
fn a_patched_polyframe_draws_what_a_rebuild_draws() {
    let mut document = surface("polyframe", 96, 4.0 / 96.0);
    let mut drawn = build(&mut document);
    assert!(
        drawn.spans.iter().all(|span| span.chunked),
        "an uncoloured surface is laid out in slots"
    );
    let polyframe::Lines {
        indices: mut lines,
        layout,
        ..
    } = drawn.lines();
    assert_eq!(lines.len(), drawn.indices.len() * 2, "two per index");
    let packed = polyframe::lines(&drawn.indices, &[]).indices.len();
    eprintln!(
        "{} line indices in slots against {packed} packed",
        lines.len()
    );

    stroke(&mut document, 0.3);
    let patch = patch_upload(&mut document, drawn.build).expect("a patch");
    assert!(!patch.indices.is_empty(), "the stroke re-cut chunks");
    for (first, run) in &patch.indices {
        let (at, patched) = layout
            .patch(*first, run)
            .expect("every run of an adaptive patch rewrites whole slots");
        let at = at as usize;
        lines[at..at + patched.len()].copy_from_slice(&patched);
    }
    drawn.apply(&patch);

    let fresh = build(&mut document);
    assert_eq!(
        drawn.edges(&lines),
        fresh.edges(&fresh.lines().indices),
        "the patched polyframe draws what a rebuild draws"
    );
}

/// The renderer writes a patch in place, counting exactly its bytes — with
/// the polyframe on, the patched slots' lines as well — and still declines,
/// writing nothing, a run its lines cannot follow.
#[test]
fn the_renderer_patches_the_polyframe_with_the_triangles() {
    let Some(mut harness) = support::Harness::new() else {
        return;
    };
    let mut document = surface("renderer", 48, 4.0 / 48.0);
    let (vertices, indices, spans) = support::viewport_layers(&mut document);
    assert!(spans.iter().all(|span| span.chunked));
    let built = document.carried_build();
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
    assert!(
        patch.apply(&gpu, &mut harness.renderer),
        "the polyframe no longer forces a rebuild"
    );
    let rewritten: usize = patch.indices.iter().map(|(_, run)| run.len()).sum();
    let patched = harness.gpu.take_uploaded_bytes();
    assert_eq!(
        patched,
        patch.bytes() + (rewritten * 2 * 4) as u64,
        "the triangles, and two line indices for each index rewritten"
    );

    // What the same dab cost before: the whole buffer and its lines again.
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);
    let rebuilt = harness.gpu.take_uploaded_bytes();
    eprintln!("polyframe on: {patched} bytes patched against {rebuilt} rebuilt");
    assert!(patched * 4 < rebuilt, "a fraction of the rebuild");

    // The same buffer drawn as one ordinary span: its lines are packed, so
    // a patch cannot follow them and is declined while they are built.
    let packed = [MeshSpan::new(spans[0].layer, spans[0].indices.clone())];
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &packed);
    harness.gpu.take_uploaded_bytes();
    assert!(!patch.apply(&gpu, &mut harness.renderer));
    assert_eq!(harness.gpu.take_uploaded_bytes(), 0, "nothing was written");
}

/// The picture, not only the list: the polyframe drawn after a patch is the
/// polyframe drawn after a rebuild of the same surface.
#[test]
fn a_patched_polyframe_renders_like_a_rebuilt_one() {
    let Some(mut harness) = support::Harness::new() else {
        return;
    };
    let mut document = surface("polyframe-render", 48, 4.0 / 48.0);
    let mut camera = support::framed(&document);
    camera.pitch = 1.0;
    let (vertices, indices, spans) = support::viewport_layers(&mut document);
    let built = document.carried_build();
    let gpu = harness.gpu.clone();
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);
    harness.renderer.set_polyframe(&gpu, true);
    let surface = clayspace_view::GpuMesh::new(&gpu);
    let before = harness.capture(&surface, &camera, false, "dynamic-polyframe-before");

    stroke(&mut document, 0.3);
    let patch = patch_upload(&mut document, built).expect("a patch");
    assert!(patch.apply(&gpu, &mut harness.renderer));
    let patched = harness.capture(&surface, &camera, false, "dynamic-polyframe-patched");

    let (vertices, indices, spans) = support::viewport_layers(&mut document);
    harness
        .renderer
        .set_mesh_layers(&gpu, &vertices, &indices, &spans);
    let rebuilt = harness.capture(&surface, &camera, false, "dynamic-polyframe-rebuilt");

    let moved = support::differing_pixels(&before, &rebuilt);
    let stale = support::differing_pixels(&patched, &rebuilt);
    eprintln!("the stroke moved {moved} pixels; the patched frame differs by {stale}");
    assert!(moved > 500, "the stroke changed the wireframe");
    assert_eq!(stale, 0, "the patched polyframe is the rebuilt one");
}
