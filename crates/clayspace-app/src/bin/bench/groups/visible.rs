//! Getting an edit onto the screen, which is the half that was never the
//! problem and is now most of the cost.
//!
//! A figure that timed `apply_stroke` alone would measure the engine and call
//! it latency. What a sculptor waits for is the surface arriving, and the three
//! representations arrive by different routes: a field through the brick
//! cache's incremental re-mesh, a grid and a mesh through one buffer rebuilt
//! whole. This is those routes, as the application itself walks them.

use clayspace_app::SurfaceGeometry;
use clayspace_engine::ClayDocument;
use clayspace_model::{Representation, SculptModel};
use clayspace_view::{Gpu, OffscreenTarget, Renderer, Vertex};

use crate::skip::Skip;

/// Everything the application keeps between an edit and a frame.
pub struct Screen {
    geometry: SurfaceGeometry,
    renderer: Renderer,
}

impl Screen {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            geometry: SurfaceGeometry::new(gpu),
            renderer: Renderer::new(gpu, OffscreenTarget::FORMAT),
        }
    }

    /// Brings the surface up before anything is timed.
    pub fn prime(&mut self, gpu: &Gpu, document: &mut ClayDocument) -> Result<(), Skip> {
        self.geometry
            .rebuild(gpu, document)
            .map_err(|_| Skip::SurfaceWouldNotMesh)?;
        self.refresh(gpu, document)
    }

    /// What a frame pays after an edit, on whichever layer the edit went to.
    pub fn refresh(&mut self, gpu: &Gpu, document: &mut ClayDocument) -> Result<(), Skip> {
        match document.active_representation() {
            Representation::Sdf => self
                .geometry
                .sync(gpu, document)
                .map(|_| ())
                .map_err(|_| Skip::SurfaceWouldNotMesh),
            Representation::Voxel => {
                // The smooth surface first, so the revision below reflects a
                // grid that moved. Cheap when nothing did.
                document
                    .resmooth_voxels()
                    .map_err(|_| Skip::SurfaceWouldNotMesh)?;
                self.upload(gpu, document);
                Ok(())
            }
            Representation::Mesh => {
                self.upload(gpu, document);
                Ok(())
            }
            // A hierarchy is drawn from its display level's triangles, which
            // arrive through the same whole-buffer rebuild a mesh takes — so
            // it is the mesh branch, and the `multires` group builds its own
            // subject rather than taking a `Scene` member (see that group for
            // why one is deliberately not added).
            Representation::Multires => {
                self.upload(gpu, document);
                Ok(())
            }
        }
    }

    /// The one buffer a grid and a mesh are both drawn from.
    fn upload(&mut self, gpu: &Gpu, document: &mut ClayDocument) {
        let _ = document.mesh_revision();
        let (positions, normals, colors, indices, spans) = document.visible_mesh_geometry();
        let frozen = document.mask_at(&positions);
        let vertices: Vec<Vertex> = positions
            .into_iter()
            .zip(normals)
            .zip(colors)
            .enumerate()
            .map(|(at, ((position, normal), color))| Vertex {
                position,
                normal,
                color,
                mask: frozen.as_ref().map_or(0.0, |weights| weights[at]),
            })
            .collect();
        let spans: Vec<clayspace_view::MeshSpan> = spans
            .into_iter()
            .map(|span| clayspace_view::MeshSpan::new(span.layer, span.indices))
            .collect();
        self.renderer
            .set_mesh_layers(gpu, &vertices, &indices, &spans);
    }
}
