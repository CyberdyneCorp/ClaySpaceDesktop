//! A layer's UV layout, drawn on the layer.
//!
//! The layout reaches the viewport as one UV per vertex of one mesh layer —
//! the layer a retopology asked for UVs placed. It is drawn *in place of* that
//! layer's span of the carried buffer, not over it: the same triangles at the
//! same positions, so depth, occlusion and every other layer are exactly as
//! they were, and only the material differs.
//!
//! **Its own vertex type and its own buffer**, because the UV is its own
//! attribute. The carried buffer's [`Vertex`] is the layout the engine writes
//! into directly and every surface in the scene pays for; widening it by a UV
//! would charge a voxel grid and an SDF surface eight bytes a vertex for a
//! preview only a mesh with a layout can use. This buffer exists only while
//! the preview is shown.
//!
//! **The seams are lines over it**, in a colour of their own, depth-biased the
//! way the polyframe is so they sit on the surface rather than fighting it.

use bytemuck::{Pod, Zeroable};
use clayspace_model::{uv_islands, LayerKey, UvDisplay, UvPreview};

use super::Vertex;

/// One vertex of the preview: where it is, which way it faces, the tint its
/// island is drawn with, and its UV.
///
/// `position` at 0, `normal` at 12, `tint` at 24, `uv` at 36, stride 44. The
/// first three sit where [`Vertex`] puts them and take the same shader
/// locations, so the vertex stage reads a preview exactly as it reads the
/// surface; the UV takes location 4, since 3 is the surface's mask.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct UvVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tint: [f32; 3],
    pub uv: [f32; 2],
}

impl UvVertex {
    pub const STRIDE: usize = std::mem::size_of::<Self>();
    pub const UV_OFFSET: usize = 36;
    /// The shader location the UV is read from.
    pub const UV_LOCATION: u32 = 4;

    pub(super) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: Self::STRIDE as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: Vertex::POSITION_OFFSET as u64,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: Vertex::NORMAL_OFFSET as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: Vertex::COLOR_OFFSET as u64,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: Self::UV_OFFSET as u64,
                    shader_location: Self::UV_LOCATION,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// The seams' colour, in linear space.
///
/// A saturated warm red rather than the polyframe's dark ink: a seam has to
/// read against both squares of the checker and against every island's tint,
/// and a dark line vanishes into the checker's dark squares.
pub const SEAM_COLOR: [f32; 3] = [0.95, 0.18, 0.12];

/// What the preview draws, ready to upload.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UvGeometry {
    pub vertices: Vec<UvVertex>,
    pub indices: Vec<u32>,
    /// The seams as a line list, in the overlay's vertex type.
    pub seam_vertices: Vec<Vertex>,
    pub seam_indices: Vec<u32>,
    /// How many islands the layout has.
    pub islands: usize,
}

/// The preview's geometry for one layer, as `display` asks for it.
///
/// Every island white under [`UvDisplay::Checker`] and a colour of its own
/// under [`UvDisplay::Islands`]; the seams in both.
pub fn uv_geometry(preview: &UvPreview, display: UvDisplay) -> UvGeometry {
    let found = uv_islands(&preview.positions, &preview.indices);
    let tint = |at: usize| match display {
        UvDisplay::Islands => island_tint(found.island[at]),
        UvDisplay::Off | UvDisplay::Checker => [1.0; 3],
    };
    let vertices = (0..preview.positions.len())
        .map(|at| UvVertex {
            position: preview.positions[at],
            normal: preview.normals.get(at).copied().unwrap_or([0.0, 0.0, 1.0]),
            tint: tint(at),
            uv: preview.uvs.get(at).copied().unwrap_or_default(),
        })
        .collect();
    let seam_vertices: Vec<Vertex> = found
        .seams
        .iter()
        .flatten()
        .map(|&at| Vertex {
            position: preview.positions[at as usize],
            normal: [0.0, 0.0, 1.0],
            color: SEAM_COLOR,
            mask: 0.0,
        })
        .collect();
    UvGeometry {
        vertices,
        indices: preview.indices.clone(),
        seam_indices: (0..seam_vertices.len() as u32).collect(),
        seam_vertices,
        islands: found.count,
    }
}

/// A light, distinct colour for one island.
///
/// Hues a golden-ratio turn apart, so neighbouring island numbers never land
/// on neighbouring hues however many there are. Light and only half saturated:
/// the tint multiplies the checker and the material under it, and a strong
/// colour would drown both.
pub fn island_tint(island: u32) -> [f32; 3] {
    const TURN: f32 = 0.618_034;
    let hue = (island as f32 * TURN).fract();
    hsv_to_rgb(hue, 0.5, 1.0)
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> [f32; 3] {
    let channel = |n: f32| {
        let k = (n + hue * 6.0) % 6.0;
        value - value * saturation * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    [channel(5.0), channel(3.0), channel(1.0)]
}

/// The preview on the device, and whose span it stands in for.
pub(super) struct UvPreviewMesh {
    pub(super) layer: LayerKey,
    pub(super) vertices: wgpu::Buffer,
    pub(super) indices: wgpu::Buffer,
    pub(super) index_count: u32,
    pub(super) seams: super::GpuMesh,
    /// The two buffers' bytes, counted against the device while they live.
    _resident: [crate::device_memory::Resident; 2],
}

impl UvPreviewMesh {
    pub(super) fn upload(gpu: &crate::gpu::Gpu, layer: LayerKey, geometry: &UvGeometry) -> Self {
        let (vertices, vertex_bytes) = upload_buffer(
            gpu,
            "uv preview vertices",
            bytemuck::cast_slice(&geometry.vertices),
            wgpu::BufferUsages::VERTEX,
        );
        let (indices, index_bytes) = upload_buffer(
            gpu,
            "uv preview indices",
            bytemuck::cast_slice(&geometry.indices),
            wgpu::BufferUsages::INDEX,
        );
        let mut seams = super::GpuMesh::new(gpu);
        seams.upload(gpu, &geometry.seam_vertices, &geometry.seam_indices);
        Self {
            layer,
            vertices,
            indices,
            index_count: geometry.indices.len() as u32,
            seams,
            _resident: [vertex_bytes, index_bytes],
        }
    }
}

fn upload_buffer(
    gpu: &crate::gpu::Gpu,
    label: &str,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
) -> (wgpu::Buffer, crate::device_memory::Resident) {
    let (buffer, resident) = super::mesh_buffer(gpu, label, bytes.len() as u64, usage);
    if !bytes.is_empty() {
        gpu.queue.write_buffer(&buffer, 0, bytes);
        gpu.note_upload(bytes.len() as u64);
    }
    (buffer, resident)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two quads split along the edge they share, each with its own UVs.
    fn split_strip() -> UvPreview {
        UvPreview {
            layer: LayerKey(7),
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [2.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 8],
            uvs: vec![
                [0.0, 0.0],
                [0.5, 0.0],
                [0.5, 0.5],
                [0.0, 0.5],
                [0.5, 0.5],
                [1.0, 0.5],
                [1.0, 1.0],
                [0.5, 1.0],
            ],
            indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
        }
    }

    #[test]
    fn the_uv_attribute_sits_after_the_surface_attributes() {
        assert_eq!(UvVertex::STRIDE, 44);
        let layout = UvVertex::layout();
        assert_eq!(layout.array_stride, UvVertex::STRIDE as u64);
        let uv = layout
            .attributes
            .iter()
            .find(|attribute| attribute.shader_location == UvVertex::UV_LOCATION)
            .expect("a uv attribute");
        assert_eq!(uv.offset, UvVertex::UV_OFFSET as u64);
        assert_eq!(uv.format, wgpu::VertexFormat::Float32x2);
    }

    #[test]
    fn the_checker_carries_every_uv_and_no_tint() {
        let preview = split_strip();
        let geometry = uv_geometry(&preview, UvDisplay::Checker);
        let uvs: Vec<[f32; 2]> = geometry.vertices.iter().map(|v| v.uv).collect();
        assert_eq!(uvs, preview.uvs);
        assert!(geometry.vertices.iter().all(|v| v.tint == [1.0; 3]));
        assert_eq!(geometry.indices, preview.indices);
        assert_eq!(geometry.islands, 2);
    }

    #[test]
    fn islands_are_tinted_apart() {
        let geometry = uv_geometry(&split_strip(), UvDisplay::Islands);
        let (left, right) = (geometry.vertices[0].tint, geometry.vertices[4].tint);
        assert_ne!(left, right, "two islands drawn the same colour");
        assert!(geometry.vertices[..4].iter().all(|v| v.tint == left));
        assert!(geometry.vertices[4..].iter().all(|v| v.tint == right));
    }

    #[test]
    fn the_seam_is_one_line_along_the_split() {
        let geometry = uv_geometry(&split_strip(), UvDisplay::Checker);
        assert_eq!(geometry.seam_vertices.len(), 2);
        assert_eq!(geometry.seam_indices, vec![0, 1]);
        assert!(geometry.seam_vertices.iter().all(|v| v.position[0] == 1.0));
        assert!(geometry.seam_vertices.iter().all(|v| v.color == SEAM_COLOR));
    }

    #[test]
    fn island_tints_are_light_and_distinct() {
        let tints: Vec<[f32; 3]> = (0..12).map(island_tint).collect();
        for (i, a) in tints.iter().enumerate() {
            assert!(a.iter().all(|c| (0.45..=1.0).contains(c)), "{a:?}");
            for b in &tints[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
