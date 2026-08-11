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
use clayspace_view::{Camera, Gpu, GpuMesh, Image, OffscreenTarget, Renderer, Vertex};

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
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        image.width,
        image.height,
    );
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
        let image = self.target.capture(
            &self.gpu,
            &self.renderer,
            &Camera::default(),
            &empty,
            false,
        );
        image.pixel(0, 0)
    }
}

/// Converts an engine mesh into renderer vertices.
///
/// Uses `clay_mesh_copy_vertices` with the renderer's own layout, so this is
/// the same one-pass copy the viewport performs rather than a test-only
/// reimplementation that could diverge from it.
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
