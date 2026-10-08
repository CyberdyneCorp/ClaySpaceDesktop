//! Getting an edit onto the screen, which is the half that was never the
//! problem and is now most of the cost.
//!
//! A figure that timed `apply_stroke` alone would measure the engine and call
//! it latency. What a sculptor waits for is the surface arriving, and the three
//! representations arrive by different routes: a field through the brick
//! cache's incremental re-mesh, a grid and a mesh through one buffer rebuilt
//! whole. This is those routes, as the application itself walks them.

use std::time::Instant;

use clayspace_app::SurfaceGeometry;
use clayspace_engine::ClayDocument;
use clayspace_model::{Representation, SculptModel};
use clayspace_view::{Gpu, OffscreenTarget, Renderer, Vertex};

use crate::skip::Skip;

/// Everything the application keeps between an edit and a frame.
pub struct Screen {
    geometry: SurfaceGeometry,
    renderer: Renderer,
    /// The build of the carried buffer the renderer holds, as the
    /// application keeps it, so an adaptive stroke is patched in place the
    /// way the application patches it.
    built: Option<u64>,
    /// What the last refresh sent for adaptive surfaces, in bytes.
    adaptive_bytes: u64,
    /// Where the last refresh spent its time, phase by phase, in
    /// milliseconds — what `CLAYSPACE_BENCH_PHASES` prints.
    laps: Vec<(&'static str, f64)>,
    /// When the phase being timed began.
    mark: Instant,
}

impl Screen {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            geometry: SurfaceGeometry::new(gpu),
            renderer: Renderer::new(gpu, OffscreenTarget::FORMAT),
            built: None,
            adaptive_bytes: 0,
            laps: Vec::new(),
            mark: Instant::now(),
        }
    }

    /// What the last refresh sent to the device for adaptive surfaces.
    pub fn adaptive_bytes(&self) -> u64 {
        self.adaptive_bytes
    }

    /// The last refresh's phases, as `resmooth=14.07 geometry=0.76 ...`.
    pub fn describe_laps(&self) -> String {
        self.laps
            .iter()
            .map(|(name, ms)| format!("{name}={ms:.2}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Brings the surface up before anything is timed, and leaves the device
    /// with nothing pending.
    pub fn prime(&mut self, gpu: &Gpu, document: &mut ClayDocument) -> Result<(), Skip> {
        self.geometry
            .rebuild(gpu, document)
            .map_err(|_| Skip::SurfaceWouldNotMesh)?;
        self.refresh(gpu, document)?;
        settle(gpu);
        Ok(())
    }

    /// What a frame pays after an edit, on whichever layer the edit went to.
    pub fn refresh(&mut self, gpu: &Gpu, document: &mut ClayDocument) -> Result<(), Skip> {
        self.laps.clear();
        self.mark = Instant::now();
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
                self.lap("resmooth");
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
            // An adaptive surface is drawn from its own triangles: the chunks
            // a stroke dirtied, written in place, and the same whole-buffer
            // rebuild when a patch cannot follow.
            Representation::Dynamic => {
                if !self.patch(gpu, document) {
                    self.upload(gpu, document);
                }
                Ok(())
            }
        }
    }

    /// Closes the phase being timed under `name` and opens the next.
    fn lap(&mut self, name: &'static str) {
        let now = Instant::now();
        self.laps
            .push((name, (now - self.mark).as_secs_f64() * 1000.0));
        self.mark = now;
    }

    /// The application's patch path: `false` where it has to rebuild.
    fn patch(&mut self, gpu: &Gpu, document: &mut ClayDocument) -> bool {
        let Some(built) = self.built else {
            return false;
        };
        let Some(patch) = clayspace_app::carried::patch_upload(document, built) else {
            return false;
        };
        if !patch.apply(gpu, &mut self.renderer) {
            return false;
        }
        self.adaptive_bytes = patch.bytes();
        true
    }

    /// The one buffer a grid and a mesh are both drawn from.
    fn upload(&mut self, gpu: &Gpu, document: &mut ClayDocument) {
        let _ = document.mesh_revision();
        let (positions, normals, colors, indices, spans) = document.visible_mesh_geometry();
        self.lap("geometry");
        self.built = Some(document.carried_build());
        self.adaptive_bytes = clayspace_app::carried::rebuilt_bytes(document.dynamic_upload());
        let frozen = document.mask_at(&positions);
        self.lap("mask");
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
            .map(|span| {
                clayspace_view::MeshSpan::new(span.layer, span.indices).chunked(span.chunked)
            })
            .collect();
        self.lap("vertices");
        self.renderer
            .set_mesh_layers(gpu, &vertices, &indices, &spans);
        self.lap("upload");
    }
}

/// Hands the device everything written so far and waits for it to finish,
/// so that a series starts from a quiet device.
///
/// The harness never presents a frame, so nothing submits for the field's
/// route: a field series writes its re-meshed bricks with `write_buffer` and
/// the staging sits in wgpu's pending writes until some submission carries
/// it. The first `set_mesh_layers` of the next series does — it flushes its
/// writes since #311 — carrying every earlier series' staging (341 MB after
/// the twelve field brushes, read off the device ledger) and with it the
/// deferred release of every buffer those series dropped. The device finishes
/// that a few samples later, and the sample whose flush finds it finished
/// pays the release: `brush.voxel.padrao` sample 6 of 13 read 80 ms against
/// 15 ms for its neighbours on this M3 Pro with the engine CPU-only, 216 to
/// 286 ms on the Linux CI runner and 0.8 to 2.0 s on the macOS one, every run
/// from 30 Sep to 7 Oct, and only when the field series ran first. The
/// application pays this per frame, spread over frames that submit; a series
/// timed here should not pay it at all.
fn settle(gpu: &Gpu) {
    gpu.flush_writes();
    gpu.device.poll(wgpu::Maintain::Wait);
    gpu.note_device_idle();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::groups::headless_gpu;
    use clayspace_app::Scene;
    use clayspace_engine::BackendPolicy;
    use clayspace_model::ToolKind;

    /// The regression behind `brush.voxel.padrao` on CI from 30 Sep to 7 Oct:
    /// a field series' uploads, never submitted, were carried and released
    /// inside the next series' timing. Priming the next series' screen has to
    /// leave the device with none of them pending — read off the ledger,
    /// which counts staging until a wait releases it.
    #[test]
    fn a_primed_screen_carries_nothing_the_series_before_left_pending() {
        let Some(gpu) = headless_gpu() else {
            return;
        };
        let policy = BackendPolicy::discover(None).expect("an engine backend");
        let scene = Scene::Reference;
        let mut document = scene.build(policy.clone()).expect("the reference scene");
        let mut screen = Screen::new(&gpu);
        screen.prime(&gpu, &mut document).expect("primed");
        for sample in scene.stroke(4) {
            document
                .apply_stroke(
                    ToolKind::Padrao,
                    scene.brush(),
                    &[sample],
                    [true, false, false],
                )
                .expect("a dab on the field");
            screen.refresh(&gpu, &mut document).expect("refreshed");
        }
        assert!(
            gpu.memory().staging > 0,
            "a field series writes its bricks and nothing here submits them"
        );
        drop(screen);

        let mut next = scene.build(policy).expect("the reference scene again");
        let mut screen = Screen::new(&gpu);
        screen.prime(&gpu, &mut next).expect("primed");
        assert_eq!(
            gpu.memory().staging,
            0,
            "the series before left staging pending for the next series' samples to pay"
        );
    }
}
