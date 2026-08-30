//! Shared machinery for the visual tests.
//!
//! Every test here renders a real frame on a real device and writes it to
//! `target/visual/`, so a change to shading, camera or geometry can be looked
//! at rather than only asserted about. The assertions are deliberately coarse
//! — "something was drawn", "these two differ", "this is brighter than that" —
//! because a pixel-exact golden would fail on every driver and tell us nothing
//! about whether the picture is right.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use clayspace_engine::claycore::{self, Document, Mesh, VertexLayout};
use clayspace_engine::ClayDocument;
use clayspace_model::SculptModel;
use clayspace_view::{Camera, Gpu, GpuMesh, Image, MeshSpan, OffscreenTarget, Renderer, Vertex};

/// Where captured frames are written.
pub fn output_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/visual")
        .canonicalize()
        .unwrap_or_else(|_| {
            let fallback = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/visual");
            std::fs::create_dir_all(&fallback).ok();
            fallback
        });
    std::fs::create_dir_all(&dir).expect("create the visual output directory");
    dir
}

/// Writes a captured frame as a PNG and returns where it went.
pub fn save(image: &Image, name: &str) -> PathBuf {
    let path = output_dir().join(format!("{name}.png"));
    write_png(&path, image);
    path
}

fn write_png(path: &Path, image: &Image) {
    let file = std::fs::File::create(path)
        .unwrap_or_else(|e| panic!("could not create {}: {e}", path.display()));
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("write the PNG header")
        .write_image_data(&image.pixels)
        .expect("write the PNG data");
}

/// A device, renderer and offscreen target ready to draw into.
pub struct Harness {
    pub gpu: Gpu,
    pub renderer: Renderer,
    pub target: OffscreenTarget,
}

impl Harness {
    pub const WIDTH: u32 = 480;
    pub const HEIGHT: u32 = 360;

    /// Builds a headless harness, or explains why it could not.
    ///
    /// Returns `None` where no adapter exists at all, so the suite skips
    /// rather than fails on a machine with no GPU of any kind — including a
    /// software one.
    pub fn new() -> Option<Self> {
        let gpu = match pollster::block_on(Gpu::headless()) {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("skipping visual tests: {e}");
                return None;
            }
        };
        let renderer = Renderer::new(&gpu, OffscreenTarget::FORMAT);
        let target = OffscreenTarget::new(&gpu, Self::WIDTH, Self::HEIGHT);
        Some(Self {
            gpu,
            renderer,
            target,
        })
    }

    /// Uploads an engine mesh and renders it, saving the frame.
    pub fn capture_mesh(&mut self, mesh: &Mesh, camera: &Camera, name: &str) -> Image {
        let gpu_mesh = self.upload(mesh);
        let image = self.target.capture(
            &self.gpu,
            &self.renderer,
            camera,
            &gpu_mesh,
            mesh.colors().is_some(),
        );
        save(&image, name);
        image
    }

    /// Renders an already-uploaded mesh.
    pub fn capture(&self, mesh: &GpuMesh, camera: &Camera, colored: bool, name: &str) -> Image {
        let image = self
            .target
            .capture(&self.gpu, &self.renderer, camera, mesh, colored);
        save(&image, name);
        image
    }

    /// Moves an engine mesh onto the GPU through the engine's own
    /// layout-directed copy — the path the viewport uses, not a test-only one.
    pub fn upload(&self, mesh: &Mesh) -> GpuMesh {
        let mut gpu_mesh = GpuMesh::new(&self.gpu);
        let (vertices, indices) = to_vertices(mesh);
        gpu_mesh.upload(&self.gpu, &vertices, &indices);
        gpu_mesh
    }

    /// The renderer's background, as the readback reports it.
    pub fn background(&self) -> [u8; 4] {
        // Rendered rather than computed: the sRGB target encodes the clear
        // colour, so the literal in the renderer is not what lands in the
        // buffer.
        let empty = GpuMesh::new(&self.gpu);
        let image =
            self.target
                .capture(&self.gpu, &self.renderer, &Camera::default(), &empty, false);
        image.pixel(0, 0)
    }
}

/// Converts an engine mesh into renderer vertices.
///
/// Uses `clay_mesh_copy_vertices` with the renderer's own layout, so this is
/// the same one-pass copy the viewport performs rather than a test-only
/// reimplementation that could diverge from it.
/// Where a difference stops being the driver and starts being the picture.
///
/// Two renders of the same geometry are not bit-identical on every device. A
/// tile-based GPU bins differently when the frame's contents change, so adding
/// a small overlay in one corner shifts a handful of silhouette pixels by a
/// level or two somewhere else — and an assertion written as "not one pixel
/// differs" then fails for a reason that has nothing to do with what it is
/// testing.
///
/// Measured on a macOS runner, over the six assertions that were written that
/// way, in pixels differing by more than this many levels:
///
/// | comparison                       | > 8 | > 32 |
/// |----------------------------------|-----|------|
/// | gizmo, over the sculpt (noise)   |   0 |    0 |
/// | mask cleared (noise)             |   0 |    0 |
/// | polyframe off again (noise)      |  36 |    0 |
/// | cursor cleared (noise)           |   8 |    0 |
/// | incremental settled (noise)      |   4 |    0 |
/// | mask painted (**the effect**)    |4611 | 2587 |
/// | polyframe on (**the effect**)    |31077|10249 |
/// | cursor drawn (**the effect**)    | 357 |  273 |
///
/// So 32 separates every one of them: nothing above it in any frame that was
/// meant to be unchanged, and hundreds to thousands in every frame that was
/// meant to change. A margin that wide is what makes this a threshold rather
/// than a fudge — halving it or doubling it changes no verdict here.
pub const RENDER_NOISE: u8 = 32;

/// How many pixels differ by more than [`RENDER_NOISE`].
///
/// The measure the module's own note asks for: "a pixel-exact golden would
/// fail on every driver and tell us nothing about whether the picture is
/// right". This is the coarse version — it answers "is this the same picture"
/// rather than "are these the same bytes".
pub fn differing_pixels(a: &Image, b: &Image) -> usize {
    differing_pixels_within(a, b, 0, 0, a.width.min(b.width), a.height.min(b.height))
}

/// The same, over one rectangle of the frame.
///
/// A small thing in a corner is drowned by a mean over the whole frame, so
/// several of these tests ask about a region instead.
pub fn differing_pixels_within(a: &Image, b: &Image, x0: u32, y0: u32, x1: u32, y1: u32) -> usize {
    let mut count = 0;
    for y in y0..y1.min(a.height).min(b.height) {
        for x in x0..x1.min(a.width).min(b.width) {
            let (pa, pb) = (a.pixel(x, y), b.pixel(x, y));
            if (0..3).any(|c| pa[c].abs_diff(pb[c]) > RENDER_NOISE) {
                count += 1;
            }
        }
    }
    count
}

pub fn to_vertices(mesh: &Mesh) -> (Vec<Vertex>, Vec<u32>) {
    let count = mesh.vertex_count();
    let mut bytes = vec![0u8; count * Vertex::STRIDE];

    // Colour is left at white where the mesh has none: the engine refuses a
    // layout naming an attribute it does not carry, so it is pre-filled here
    // and the copy writes around it.
    let has_colors = mesh.colors().is_some();
    if !has_colors {
        for vertex in bytes.chunks_exact_mut(Vertex::STRIDE) {
            for channel in 0..3 {
                let at = Vertex::COLOR_OFFSET + channel * 4;
                vertex[at..at + 4].copy_from_slice(&1.0f32.to_le_bytes());
            }
        }
    }

    let layout = VertexLayout {
        stride: Some(Vertex::STRIDE as u32),
        position_offset: Some(Vertex::POSITION_OFFSET as i32),
        normal_offset: Some(Vertex::NORMAL_OFFSET as i32),
        color_offset: has_colors.then_some(Vertex::COLOR_OFFSET as i32),
        uv_offset: None,
    };
    mesh.copy_vertices(layout, &mut bytes)
        .expect("copy vertices into the renderer's layout");

    let vertices: Vec<Vertex> = bytes
        .chunks_exact(Vertex::STRIDE)
        .map(|v| Vertex {
            position: read_vec3(v, Vertex::POSITION_OFFSET),
            normal: read_vec3(v, Vertex::NORMAL_OFFSET),
            color: read_vec3(v, Vertex::COLOR_OFFSET),
            mask: 0.0,
        })
        .collect();

    let mut indices = vec![0u32; mesh.index_count()];
    mesh.copy_indices(&mut indices).expect("copy indices");
    (vertices, indices)
}

fn read_vec3(bytes: &[u8], offset: usize) -> [f32; 3] {
    std::array::from_fn(|i| {
        let at = offset + i * 4;
        f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    })
}

/// A camera framing whatever the mesh covers.
pub fn framed_camera(mesh: &Mesh) -> Camera {
    let mut camera = Camera::default();
    match mesh.bounds() {
        Ok((min, max)) => camera.frame_bounds(min.into(), max.into()),
        Err(_) => camera.frame_default(),
    }
    camera
}

/// A camera framing whatever the document covers.
///
/// The sibling of [`framed_camera`] for the tests that render a whole
/// document rather than one engine mesh. Here rather than beside each of
/// them: it was written out byte-for-byte in seven test files, so how a scene
/// is framed had to be changed seven times or the captures stopped being
/// comparable with each other.
pub fn framed(document: &ClayDocument) -> Camera {
    let mut camera = Camera::default();
    match SculptModel::bounds(document) {
        Some((min, max)) => camera.frame_bounds(min.into(), max.into()),
        None => camera.frame_default(),
    }
    camera
}

/// The buffer the viewport would upload for a document.
///
/// The same call `App::sync_mesh_layers` makes, assembled the same way, so a
/// test measures the application's own path rather than a test-only one that
/// could drift from it. One name for it, because six names for one operation
/// meant a reader of any single file could not tell it was the shared path,
/// and a change to the vertex layout had to find all six.
pub fn viewport_geometry(document: &mut ClayDocument) -> (Vec<Vertex>, Vec<u32>) {
    let (vertices, indices, _) = viewport_layers(document);
    (vertices, indices)
}

/// The same buffer, with the spans that say whose triangles each run of it is.
///
/// Still one path: `viewport_geometry` is this with the spans dropped, so a
/// test that does not care which subtool a triangle came from does not have to
/// say so, and neither of them can drift from what the application uploads.
pub fn viewport_layers(document: &mut ClayDocument) -> (Vec<Vertex>, Vec<u32>, Vec<MeshSpan>) {
    let (positions, normals, colors, indices, spans) = document.visible_mesh_geometry();
    let vertices = positions
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
    let spans = spans
        .into_iter()
        .map(|span| MeshSpan::new(span.layer, span.indices))
        .collect();
    (vertices, indices, spans)
}

// -- fixtures ---------------------------------------------------------------

/// A sphere, the simplest thing with a silhouette.
pub fn sphere_document(radius: f32) -> Document {
    let mut doc = Document::new().expect("document");
    let layer = doc.add_sdf_layer("Base").expect("layer");
    let item = claycore::Item::sphere(radius).expect("sphere");
    doc.add_item(layer, &item).expect("place");
    doc
}

/// Meshes a document at a resolution fine enough to read but quick to build.
pub fn mesh_document(doc: &Document, resolution: i32) -> Mesh {
    doc.mesh(claycore::MeshParams {
        resolution,
        ..Default::default()
    })
    .expect("mesh the document")
}

/// Uploads an engine mesh straight to the GPU, with no per-key splitting.
///
/// The control for `visual_incremental`: whatever `SurfaceGeometry` does to
/// the same triangles has to end up looking like this.
pub fn upload_engine_mesh(gpu: &Gpu, mesh: &Mesh) -> GpuMesh {
    let mut gpu_mesh = GpuMesh::new(gpu);
    let (vertices, indices) = to_vertices(mesh);
    gpu_mesh.upload(gpu, &vertices, &indices);
    gpu_mesh
}
