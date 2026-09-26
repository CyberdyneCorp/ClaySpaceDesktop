//! An adaptive surface the document holds beside one of its mesh layers, and
//! the file that keeps it.
//!
//! # Two objects, as with a hierarchy
//!
//! A `clay_dynamic_surface` is not a `clay_layer_id`: it is a free-standing
//! owning handle read from a mesh, and `clay_document_save` has never heard of
//! it. So an adaptive row is a real mesh layer in the `.clayspace` — its name,
//! its place in the stack, its transform, its mask — plus an [`Adaptive`] held
//! here. The mesh layer keeps the triangles the surface was read from; the
//! surface is what the brush reshapes, what the viewport draws and what the
//! side-car saves. See [`crate::multires`], which made the same arrangement
//! first and whose side-car format this one shares under its own header.
//!
//! # A missing side-car
//!
//! Opens, and the row comes back as the mesh layer it demonstrably is, with
//! the loss named in the diagnostics report — the hierarchy's rule, for the
//! hierarchy's reasons. What never happens is the other direction: a row is
//! promoted to Dynamic only by a record naming it.
//!
//! # History
//!
//! A gesture is recorded as the surface's bytes before it, the bounded
//! snapshot the design allows where no reversible delta crosses into this
//! application yet. That is exact — connectivity, positions and attributes
//! come back together — and it is ordered in the document's one history.

use claycore::{DynamicDesc, DynamicSurface, DynamicTopology};
use clayspace_model::{CageFault, ModelError, Refusal};

/// The triangles the viewport is drawing, and the surface state they were
/// copied at.
struct Drawn {
    watched: (claycore::SurfaceRevision, u64),
    /// Whether the surface carries a colour attribute at all, rather than the
    /// white this side fills in for one that does not.
    coloured: bool,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

/// An adaptive surface, and everything this side has to remember about it.
pub struct Adaptive {
    surface: DynamicSurface,
    drawn: Option<Drawn>,
    /// How many times this side has replaced the surface underneath itself.
    ///
    /// A surface put back from bytes is a new identity whose revisions start
    /// again, so the engine's counters alone cannot tell a redo from nothing
    /// having happened. This is monotone across every restore.
    generation: u64,
    open: Option<OpenGesture>,
}

/// A gesture that is open, and what it has done so far.
struct OpenGesture {
    /// The surface's bytes as they stood before it: what a dragging verb is
    /// laid down again from, and what the gesture enters the history as.
    bytes: Vec<u8>,
    /// Whether any segment changed the surface, so a stroke that reached
    /// nothing does not become an undo step.
    changed: bool,
}

impl Adaptive {
    /// Wraps a surface the caller has already built.
    pub fn holding(surface: DynamicSurface) -> Self {
        Self {
            surface,
            drawn: None,
            generation: 0,
            open: None,
        }
    }

    /// How every surface this application builds reads its mesh: the engine's
    /// own weld and seam tolerances.
    pub fn desc() -> DynamicDesc {
        DynamicDesc::default()
    }

    /// The topology policy a stroke runs with.
    ///
    /// The engine's own defaults — brush-relative detail, split and collapse
    /// on — and deliberately not a setting yet. When should the remesh run is
    /// answered per verb by the engine, which is the part a sculptor would
    /// otherwise get wrong; how fine is a control that arrives with its panel.
    pub fn topology() -> DynamicTopology {
        DynamicTopology::default()
    }

    /// Reads a mesh into a surface, or says in the domain's words why not.
    pub fn from_mesh(mesh: &claycore::Mesh) -> Result<Self, ModelError> {
        DynamicSurface::from_mesh(mesh, Self::desc())
            .map(Self::holding)
            .map_err(refused)
    }

    pub fn surface(&self) -> &DynamicSurface {
        &self.surface
    }

    pub fn surface_mut(&mut self) -> &mut DynamicSurface {
        &mut self.surface
    }

    /// The half-edge census: what the inspector and the agent are told.
    pub fn stats(&self) -> claycore::DynamicStats {
        self.surface.stats().unwrap_or_default()
    }

    /// What the viewport watches: the engine's three revisions and this
    /// side's own generation.
    pub fn watched(&self) -> (claycore::SurfaceRevision, u64) {
        (self.surface.revision().unwrap_or_default(), self.generation)
    }

    /// A number that moves whenever what is drawn moves, for a redraw hash.
    pub fn drawn_revision(&self) -> u64 {
        let (revision, generation) = self.watched();
        revision
            .topology
            .wrapping_add(revision.geometry)
            .wrapping_add(revision.attributes)
            .wrapping_add(generation.wrapping_mul(0x9E37_79B9))
    }

    /// What the viewport draws, copied whole and kept until the surface moves.
    ///
    /// Whole rather than per chunk for now: the dirty-chunk transport is a
    /// drawing change of its own, and this is correct at every size the
    /// crossing produces today — it is only not yet incremental.
    #[allow(clippy::type_complexity)]
    pub fn triangles(&mut self) -> Option<(&[[f32; 3]], &[[f32; 3]], &[[f32; 3]], &[u32])> {
        let watched = self.watched();
        let stale = !matches!(&self.drawn, Some(drawn) if drawn.watched == watched);
        if stale {
            let mesh = self.surface.to_mesh().ok()?;
            let positions = mesh.positions().to_vec();
            let count = positions.len();
            self.drawn = Some(Drawn {
                watched,
                coloured: mesh.colors().is_some(),
                normals: mesh.normals_or_derived(),
                colors: mesh
                    .colors()
                    .map(<[[f32; 3]]>::to_vec)
                    .unwrap_or_else(|| vec![[1.0; 3]; count]),
                indices: mesh.indices().to_vec(),
                positions,
            });
        }
        let drawn = self.drawn.as_ref()?;
        Some((
            &drawn.positions,
            &drawn.normals,
            &drawn.colors,
            &drawn.indices,
        ))
    }

    /// Whether the surface carries vertex colour for the colour brushes to
    /// write.
    ///
    /// A surface read from an uncoloured mesh carries none, and the engine's
    /// paint over one remeshes and colours nothing — a stroke that changes the
    /// topology and not what it was for. So the colour brushes are refused
    /// there, as they are on a mesh with no colour attribute.
    pub fn carries_colour(&mut self) -> bool {
        self.triangles().is_some() && self.drawn.as_ref().is_some_and(|drawn| drawn.coloured)
    }

    /// The triangles last drawn, for a caller that may not rebuild them — a
    /// pick is a question and takes `&self`.
    pub fn drawn_triangles(&self) -> Option<(&[[f32; 3]], &[u32])> {
        let drawn = self.drawn.as_ref()?;
        Some((&drawn.positions, &drawn.indices))
    }

    /// The box the surface occupies, in its own coordinates.
    pub fn bounds(&mut self) -> Option<([f32; 3], [f32; 3])> {
        let (positions, ..) = self.triangles()?;
        let first = *positions.first()?;
        Some(positions.iter().fold((first, first), |(min, max), point| {
            (
                std::array::from_fn(|i| min[i].min(point[i])),
                std::array::from_fn(|i| max[i].max(point[i])),
            )
        }))
    }

    /// The surface as an ordinary mesh, priced by the engine before it is
    /// paid for.
    pub fn to_mesh(&self) -> Result<claycore::Mesh, ModelError> {
        let priced = self
            .surface
            .preflight_to_mesh(0)
            .map_err(ModelError::engine)?;
        if !priced.allowed {
            return Err(ModelError::engine(format!(
                "a superfície adaptativa ocupa cerca de {} MB como malha, além do que cabe aqui",
                priced.peak_bytes / (1024 * 1024)
            )));
        }
        self.surface.to_mesh().map_err(ModelError::engine)
    }

    /// The surface as bytes, priced before it allocates.
    pub fn bytes(&self, budget: u64) -> Result<Vec<u8>, ModelError> {
        let priced = self
            .surface
            .preflight_encode(budget)
            .map_err(ModelError::engine)?;
        if !priced.allowed {
            return Err(ModelError::engine(format!(
                "a superfície adaptativa ocupa cerca de {} MB, além do que cabe aqui",
                priced.persistent_bytes / (1024 * 1024)
            )));
        }
        self.surface.serialize().map_err(ModelError::engine)
    }

    /// Puts the surface back to bytes taken from it earlier.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), ModelError> {
        self.surface = DynamicSurface::deserialize(bytes).map_err(ModelError::engine)?;
        self.drawn = None;
        self.generation = self.generation.wrapping_add(1);
        Ok(())
    }

    // -- the gesture ---------------------------------------------------------

    /// Records where the surface stood, if this is the first segment to reach
    /// it.
    pub fn open_gesture(&mut self) -> Result<(), ModelError> {
        if self.open.is_none() {
            self.open = Some(OpenGesture {
                bytes: self.bytes(0)?,
                changed: false,
            });
        }
        Ok(())
    }

    /// Whether the segment just stroked changed the surface.
    pub fn note_gesture_changed(&mut self, changed: bool) {
        if let Some(open) = self.open.as_mut() {
            open.changed |= changed;
        }
    }

    /// Takes the open gesture back to where it started, for a dragging verb
    /// that lays itself down again from its anchor.
    pub fn replay_from_the_anchor(&mut self) -> Result<(), ModelError> {
        let Some(open) = self.open.take() else {
            return Ok(());
        };
        self.restore(&open.bytes)?;
        self.open = Some(OpenGesture {
            bytes: open.bytes,
            changed: false,
        });
        Ok(())
    }

    /// The record the gesture leaves behind, and the gesture closed; `None`
    /// for one that changed nothing.
    pub fn close_gesture(&mut self) -> Option<Vec<u8>> {
        self.open
            .take()
            .filter(|open| open.changed)
            .map(|open| open.bytes)
    }

    pub fn gesture_is_open(&self) -> bool {
        self.open.is_some()
    }
}

impl std::fmt::Debug for Adaptive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Adaptive")
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

/// A refused read, in the domain's words where the engine named a model
/// problem a sculptor can go and mend.
pub fn refused(refused: claycore::DynamicRefusal) -> ModelError {
    match refused.reason {
        claycore::DynamicError::EmptyMesh => ModelError::Conversion(Refusal::SourceEmpty),
        claycore::DynamicError::NonManifoldEdge => ModelError::Conversion(Refusal::NotAdaptive {
            fault: CageFault::NonManifold,
        }),
        claycore::DynamicError::DegenerateTriangle => {
            ModelError::Conversion(Refusal::NotAdaptive {
                fault: CageFault::DegenerateFace,
            })
        }
        _ => ModelError::engine(refused.to_string()),
    }
}

// -- the side-car ------------------------------------------------------------

/// Where the adaptive surfaces live for a document at `path`.
pub fn sidecar_for(path: &std::path::Path) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".dynamic");
    path.with_file_name(name)
}

/// The first line of the file: the representation's stored key and a format
/// number, so a later format can be told from this one.
const HEADER: &[u8] = b"clayspace-dynamic 1\n";

/// Writes every surface the document holds, or removes the file when there
/// are none — see [`crate::multires::write_hierarchies`].
pub fn write_surfaces(
    path: &std::path::Path,
    surfaces: &[crate::multires::Saved],
) -> std::io::Result<()> {
    crate::multires::write_records(path, HEADER, surfaces)
}

/// Reads the surfaces back, reporting what could not be read.
pub fn read_surfaces(path: &std::path::Path) -> crate::multires::SideCar {
    crate::multires::read_records(path, HEADER)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(divisions: u32) -> claycore::Mesh {
        let stride = divisions + 1;
        let positions: Vec<[f32; 3]> = (0..stride)
            .flat_map(|z| (0..stride).map(move |x| [x as f32 * 0.5, 0.0, z as f32 * 0.5]))
            .collect();
        let indices: Vec<u32> = (0..divisions)
            .flat_map(|z| (0..divisions).map(move |x| z * stride + x))
            .flat_map(|a| [a, a + stride, a + 1, a + 1, a + stride, a + stride + 1])
            .collect();
        claycore::Mesh::from_triangles(&positions, &indices).expect("a sheet")
    }

    #[test]
    fn a_restore_moves_the_generation_and_drops_what_was_drawn() {
        let mut adaptive = Adaptive::from_mesh(&sheet(4)).expect("a sheet reads");
        let before = adaptive.drawn_revision();
        assert!(adaptive.triangles().is_some());
        let bytes = adaptive.bytes(0).expect("bytes");
        adaptive.restore(&bytes).expect("restore");
        assert!(adaptive.drawn_triangles().is_none());
        assert_ne!(adaptive.drawn_revision(), before);
    }

    #[test]
    fn an_empty_mesh_is_refused_in_the_domains_words() {
        let empty = claycore::Mesh::from_triangles(&[], &[]);
        let Ok(empty) = empty else {
            // The engine refuses an empty mesh at construction, which is the
            // same answer one step earlier.
            return;
        };
        assert!(matches!(
            Adaptive::from_mesh(&empty),
            Err(ModelError::Conversion(Refusal::SourceEmpty))
        ));
    }

    #[test]
    fn the_side_car_round_trips_under_its_own_header() {
        let path = std::env::temp_dir().join(format!(
            "clayspace-adaptive-sidecar-{}.clayspace",
            std::process::id()
        ));
        let sidecar = sidecar_for(&path);
        assert!(sidecar.to_string_lossy().ends_with(".clayspace.dynamic"));
        let adaptive = Adaptive::from_mesh(&sheet(2)).expect("a sheet reads");
        let bytes = adaptive.bytes(0).expect("bytes");
        write_surfaces(
            &sidecar,
            &[crate::multires::Saved {
                position: 3,
                bytes: bytes.clone(),
            }],
        )
        .expect("write");
        let read = read_surfaces(&sidecar);
        assert!(read.faults.is_empty());
        assert_eq!(read.records.len(), 1);
        assert_eq!(read.records[0].position, 3);
        assert_eq!(read.records[0].bytes, bytes);
        // A hierarchy side-car reader does not accept this file.
        assert!(!crate::multires::read_hierarchies(&sidecar)
            .faults
            .is_empty());
        write_surfaces(&sidecar, &[]).expect("remove");
        assert!(!sidecar.exists());
    }
}
