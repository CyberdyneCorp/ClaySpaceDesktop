//! Where the two engines meet.
//!
//! ClayCore answers what the shape *is*; CyberRemesher rebuilds its topology,
//! lays out its UVs and bakes its maps. Neither knows the other's types — both
//! state that as a rule about themselves — so the correspondence lives here,
//! in the only place both are present.
//!
//! **The crossing is the handoff buffer profile**, not a file: positions,
//! normals and indices handed over in memory, version-gated by ClayCore's own
//! `HANDOFF_VERSION` rather than by a number restated here. Two libraries in
//! one address space have no reason to go through a temporary file, and the
//! engine's authors built the buffer profile for exactly this case.

use clayspace_model::{BakeMap, BakeModel, BakeResult, BakeSettings, BakedMap, Baker};
use clayspace_model::{
    ConformModel, ConformOutcome, ConformResult, ConformSettings, ConformSource, Conformer,
};
use clayspace_model::{
    ModelError, QuadMethod, Representation, RetopoModel, RetopoOutcome, RetopoResult,
    RetopoSettings, RetopoSource, Retopologiser, Unwrapper, UvModel, UvOutcome, UvResult,
    UvSettings, UvSource,
};

use crate::document::ClayDocument;

/// The domain's method, as the engine names it.
fn engine_method(method: QuadMethod) -> cyberremesh::QuadMethod {
    match method {
        QuadMethod::QuadCover => cyberremesh::QuadMethod::QuadCover,
        QuadMethod::ZRemesher => cyberremesh::QuadMethod::ZRemesher,
        QuadMethod::FieldAligned => cyberremesh::QuadMethod::FieldAligned,
        QuadMethod::InstantMeshes => cyberremesh::QuadMethod::InstantMeshes,
        QuadMethod::Integer => cyberremesh::QuadMethod::Integer,
    }
}

impl ClayDocument {
    /// Takes the active mesh subtool across to the retopology engine.
    ///
    /// **The handle it returns is valid for one operation.** The engine's
    /// element-id stability contract is that most retopology calls reassign
    /// vertex and face ids and subdivision reassigns all of them, with nothing
    /// to announce it — so nothing here keeps one, and any correspondence back
    /// into ClayCore is positional and rebuilt rather than an id that was
    /// kept.
    fn hand_off_active_mesh(&mut self) -> Result<cyberremesh::Mesh, ModelError> {
        let (positions, normals, _colours, indices, _spans) = self.visible_mesh_geometry();
        if indices.is_empty() {
            return Err(ModelError::engine(
                "esta camada de malha ainda não carrega triângulos",
            ));
        }
        let flat_positions: Vec<f32> = positions.iter().flat_map(|p| *p).collect();
        let flat_normals: Vec<f32> = normals.iter().flat_map(|n| *n).collect();
        cyberremesh::Mesh::from_handoff(
            &flat_positions,
            &flat_normals,
            &indices,
            claycore::HANDOFF_VERSION,
            "ClaySpaceDesktop",
        )
        .map_err(|e| ModelError::engine(format!("a entrega da malha foi recusada: {e}")))
    }
}

impl RetopoModel for ClayDocument {
    fn can_retopologise(&self) -> Result<(), String> {
        let (representation, carries) = self.active_layer_shape();
        if representation != Representation::Mesh {
            return Err(format!(
                "remalhar para quads reconstrói a topologia de uma malha; \
                 esta camada é {representation:?}"
            ));
        }
        if !carries {
            return Err("esta camada de malha ainda não carrega triângulos".to_string());
        }
        Ok(())
    }

    fn retopologise(&mut self, settings: RetopoSettings) -> Result<RetopoOutcome, ModelError> {
        self.can_retopologise().map_err(ModelError::engine)?;
        let settings = settings.sanitized();

        let source = self.hand_off_active_mesh()?;
        let triangles_before = source.triangle_count();

        let quads = cyberremesh::remesh(
            &source,
            cyberremesh::RemeshParams {
                target_quads: settings.target_quads,
                method: engine_method(settings.method),
                sharp_edge_degrees: settings.sharp_edge_degrees,
                pure_quads: settings.pure_quads,
                adaptivity: settings.adaptivity,
            },
            &mut cyberremesh::Unwatched,
        )
        .map_err(|e| ModelError::engine(format!("a retopologia foi recusada: {e}")))?;

        let outcome = RetopoOutcome {
            triangles_before,
            faces: quads.face_count(),
            triangles: quads.triangle_count(),
            vertices: quads.vertex_count(),
        };

        // Back into ClayCore as a **new** subtool beside the source. A
        // retopology a sculptor cannot compare against the sculpt is one they
        // cannot judge, and replacing the source is a decision that cannot be
        // undone by looking at it.
        //
        // The triangulation is what crosses back, not the quads: ClayCore's
        // mesh layers hold triangles, and the engine carries the quads beside
        // its own triangulation of them rather than instead of it — so this is
        // the same surface, drawn the way this application already draws one.
        let positions = quads.positions();
        let indices = quads.triangle_indices();
        drop(quads);

        self.attach_quads_beside_the_source(&positions, &indices)?;
        Ok(outcome)
    }

    fn retopo_source(&mut self) -> Result<RetopoSource, ModelError> {
        self.can_retopologise().map_err(ModelError::engine)?;
        let (positions, normals, _colours, indices, _spans) = self.visible_mesh_geometry();
        if indices.is_empty() {
            return Err(ModelError::engine(
                "esta camada de malha ainda não carrega triângulos",
            ));
        }
        Ok(RetopoSource {
            positions,
            normals,
            indices,
            name: self.scene_layers_name(),
        })
    }

    fn place_retopology(&mut self, result: &RetopoResult) -> Result<(), ModelError> {
        self.attach_quads_named(&result.positions, &result.indices, &result.name)?;
        Ok(())
    }
}

/// The heavy middle, off the interface thread.
///
/// Holds nothing: a retopology is a pure function of the geometry it is given
/// and the settings it is asked for, so there is no state to keep between runs
/// and nothing to invalidate. That is also what makes it `Send + Sync` without
/// a lock.
#[derive(Debug, Default, Clone, Copy)]
pub struct EngineRetopologiser;

impl Retopologiser for EngineRetopologiser {
    fn run(
        &self,
        source: &RetopoSource,
        settings: RetopoSettings,
        progress: &dyn Fn(f32, &str),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RetopoResult, String> {
        let settings = settings.sanitized();
        let flat_positions: Vec<f32> = source.positions.iter().flat_map(|p| *p).collect();
        let flat_normals: Vec<f32> = source.normals.iter().flat_map(|n| *n).collect();

        let mesh = cyberremesh::Mesh::from_handoff(
            &flat_positions,
            &flat_normals,
            &source.indices,
            claycore::HANDOFF_VERSION,
            "ClaySpaceDesktop",
        )
        .map_err(|e| format!("a entrega da malha foi recusada: {e}"))?;
        let triangles_before = mesh.triangle_count();

        let mut relay = Relay {
            progress,
            cancelled,
        };

        let quads = cyberremesh::remesh(
            &mesh,
            cyberremesh::RemeshParams {
                target_quads: settings.target_quads,
                method: engine_method(settings.method),
                sharp_edge_degrees: settings.sharp_edge_degrees,
                pure_quads: settings.pure_quads,
                adaptivity: settings.adaptivity,
            },
            &mut relay,
        )
        .map_err(|e| {
            if cyberremesh::was_cancelled(&e) {
                "a retopologia foi cancelada".to_string()
            } else {
                format!("a retopologia foi recusada: {e}")
            }
        })?;

        Ok(RetopoResult {
            outcome: RetopoOutcome {
                triangles_before,
                faces: quads.face_count(),
                triangles: quads.triangle_count(),
                vertices: quads.vertex_count(),
            },
            positions: quads.positions(),
            indices: quads.triangle_indices(),
            name: format!("{} · quads", source.name),
        })
        // `quads` drops here: a mesh handle is valid for the operation it was
        // made for, and everything worth keeping has been copied out.
    }
}

// -- UV ---------------------------------------------------------------------

impl UvModel for ClayDocument {
    fn can_unwrap(&self) -> Result<(), String> {
        // The same rule as retopology, and for the same reason: a UV layout is
        // a parameterisation of a mesh's faces, and a field has none.
        self.can_retopologise()
    }

    fn uv_source(&mut self) -> Result<UvSource, ModelError> {
        let source = self.retopo_source()?;
        Ok(UvSource {
            positions: source.positions,
            normals: source.normals,
            indices: source.indices,
            name: source.name,
        })
    }

    fn record_uv(&mut self, result: &UvResult) -> Result<(), ModelError> {
        // Nothing is written into the document, deliberately — see the trait's
        // own note. The report is held by the ViewModel that asked for it, and
        // the atlas stays in the engine that computed it.
        let _ = result;
        Ok(())
    }
}

/// The UV stage, off the interface thread.
///
/// Holds nothing, for the reason `EngineRetopologiser` holds nothing: a layout
/// is a function of the geometry and the settings, so there is no state to keep
/// and nothing to invalidate.
#[derive(Debug, Default, Clone, Copy)]
pub struct EngineUnwrapper;

impl Unwrapper for EngineUnwrapper {
    fn run(
        &self,
        source: &UvSource,
        settings: UvSettings,
        progress: &dyn Fn(f32, &str),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<UvResult, String> {
        let settings = settings.sanitized();
        let flat_positions: Vec<f32> = source.positions.iter().flat_map(|p| *p).collect();
        let flat_normals: Vec<f32> = source.normals.iter().flat_map(|n| *n).collect();

        let mut mesh = cyberremesh::Mesh::from_handoff(
            &flat_positions,
            &flat_normals,
            &source.indices,
            claycore::HANDOFF_VERSION,
            "ClaySpaceDesktop",
        )
        .map_err(|e| format!("a entrega da malha foi recusada: {e}"))?;

        let mut relay = Relay {
            progress,
            cancelled,
        };
        let atlas = cyberremesh::atlas(
            &mut mesh,
            cyberremesh::AtlasParams {
                max_chart_angle_degrees: settings.max_chart_angle_degrees,
                pack_margin: settings.pack_margin,
                texture_size: settings.texture_size,
                reorient_charts: settings.reorient_charts,
                merge_charts: settings.merge_charts,
                max_chart_distortion: settings.max_chart_distortion,
            },
            &mut relay,
        )
        .map_err(|e| {
            if cyberremesh::was_cancelled(&e) {
                "o desdobramento foi cancelado".to_string()
            } else {
                format!("o desdobramento foi recusado: {e}")
            }
        })?;

        Ok(UvResult {
            outcome: UvOutcome {
                charts: atlas.charts,
                seam_edges: atlas.seam_edges,
                max_angle_distortion: atlas.max_angle_distortion,
                rms_angle_distortion: atlas.rms_angle_distortion,
                flipped_charts: atlas.flipped_charts,
                dropped_charts: atlas.dropped_charts,
                packed_area: atlas.packed_area,
                packed_box_area: atlas.packed_box_area,
                texel_density: atlas.texel_density,
            },
            name: source.name.clone(),
        })
    }
}

/// Carries the two callbacks across into the engine's `Watcher`.
///
/// One type for both stages: a retopology and a UV layout report and cancel
/// identically, and two copies of this would be two places for the relaying to
/// drift.
struct Relay<'a> {
    progress: &'a dyn Fn(f32, &str),
    cancelled: &'a dyn Fn() -> bool,
}

impl cyberremesh::Watcher for Relay<'_> {
    fn progress(&mut self, fraction: f32, stage: &str) {
        (self.progress)(fraction, stage);
    }
    fn cancelled(&mut self) -> bool {
        (self.cancelled)()
    }
}

// -- baking -----------------------------------------------------------------

impl BakeModel for ClayDocument {
    fn can_bake(&self) -> Result<(), String> {
        // A bake writes into a UV layout on a mesh, so the same rule again.
        // What it does *not* require is a high-poly mesh: the four maps this
        // application offers are sampled from the field, which is the whole
        // reason it can offer them.
        self.can_retopologise()
    }
}

/// A field the retopology engine's baker can sample, backed by ClayCore.
///
/// **This type is the integration.** Everything else in this file moves
/// geometry between two libraries; this answers questions about a shape that
/// only one of them can answer, for a baker that has never had anything to ask.
///
/// A `claycore::Document` is neither `Send` nor `Sync`, and this crate is not
/// permitted `unsafe` — so the field is **opened on the worker thread that uses
/// it** from a snapshot on disk, and never crosses a thread boundary at all.
/// `cyberremesh::Field` carries no `Send` bound for exactly this reason: the
/// bake is a synchronous call, so a sendable field would be a promise nobody
/// needs and only `unsafe` could make.
struct ClayField {
    document: claycore::Document,
}

impl cyberremesh::Field for ClayField {
    fn distance(&self, at: [f32; 3]) -> f32 {
        self.document
            .eval_points(None, &[at])
            .ok()
            .and_then(|v| v.first().copied())
            .unwrap_or(0.0)
    }

    fn gradient(&self, at: [f32; 3]) -> [f32; 3] {
        self.document
            .eval_gradients(None, &[at])
            .ok()
            .and_then(|v| v.first().copied())
            .unwrap_or([0.0, 1.0, 0.0])
    }

    /// **The inversion, and it is the whole reason this method is named for
    /// openness rather than for occlusion.**
    ///
    /// The baker wants openness, where 1 is fully open. ClayCore answers
    /// occlusion, where 1 is fully enclosed. `1.0 - x` is the entire
    /// conversion, and skipping it bakes an ambient occlusion map that is dark
    /// where it should be light — everywhere — while looking entirely
    /// plausible. Both engines document their own direction; neither can
    /// document the join, because neither knows the other exists.
    fn openness(&self, at: [f32; 3], normal: [f32; 3], radius: f32) -> f32 {
        let occlusion = self
            .document
            .measure_points(
                claycore::SurfaceMeasure::Occlusion,
                &[at],
                claycore::MeasureParams {
                    direction: normal,
                    ..claycore::MeasureParams::occlusion(radius, 32)
                },
            )
            .ok()
            .and_then(|v| v.first().copied())
            .unwrap_or(0.0);
        1.0 - occlusion
    }
}

/// The baking stage, off the interface thread.
///
/// **Holds a snapshot of the document on disk, and there is no in-memory
/// alternative.** The field the baker samples is a `claycore::Document`, which
/// is not `Sync`, and the engine's C ABI offers `clay_document_save_memory` but
/// no matching open-from-memory — so a round trip through bytes is not
/// available and the snapshot is a temporary file.
///
/// That turns out to be the *right* semantics rather than a workaround. A bake
/// takes seconds and a sculptor may keep working while it runs; sampling the
/// live document would mean the maps describe a shape that was changing as they
/// were written. A snapshot is the shape they asked to bake.
///
/// The file is removed when this value drops, including on a refusal.
pub struct EngineBaker {
    snapshot: std::path::PathBuf,
}

impl EngineBaker {
    /// Writes the snapshot. Called on the interface thread, where the document
    /// lives.
    pub fn snapshot(document: &ClayDocument) -> Result<Self, ModelError> {
        let snapshot = std::env::temp_dir().join(format!(
            "clayspace-bake-{}-{}.clay",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        document
            .document()
            .save(&snapshot)
            .map_err(ModelError::engine)?;
        Ok(Self { snapshot })
    }
}

impl Drop for EngineBaker {
    fn drop(&mut self) {
        // A failed removal is not worth reporting: the file is in the system's
        // temporary directory, which is swept, and a bake that produced maps
        // should not report an error about its own scratch space.
        let _ = std::fs::remove_file(&self.snapshot);
    }
}

impl Baker for EngineBaker {
    fn run(
        &self,
        source: &UvSource,
        settings: &BakeSettings,
        into: &std::path::Path,
        progress: &dyn Fn(f32, &str),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<BakeResult, String> {
        if settings.maps.is_empty() {
            return Err("nenhum mapa foi pedido".to_string());
        }
        let settings = settings.clone().sanitized();

        let document = claycore::Document::open(&self.snapshot)
            .map_err(|e| format!("o campo não pôde ser reaberto para a cozedura: {e}"))?;
        let field = ClayField { document };

        let flat_positions: Vec<f32> = source.positions.iter().flat_map(|p| *p).collect();
        let flat_normals: Vec<f32> = source.normals.iter().flat_map(|n| *n).collect();
        let mut low = cyberremesh::Mesh::from_handoff(
            &flat_positions,
            &flat_normals,
            &source.indices,
            claycore::HANDOFF_VERSION,
            "ClaySpaceDesktop",
        )
        .map_err(|e| format!("a entrega da malha foi recusada: {e}"))?;

        // A bake writes into a layout, so one is made here if the caller has
        // not. Its own report is not carried up: this is the bake's business
        // and the UV panel is where a sculptor judges a layout.
        let mut relay = Relay {
            progress,
            cancelled,
        };
        cyberremesh::atlas(&mut low, Default::default(), &mut relay)
            .map_err(|e| format!("o desdobramento para a cozedura foi recusado: {e}"))?;

        let params = cyberremesh::BakeParams {
            width: settings.size,
            height: settings.size,
            cage_distance: settings.cage_distance,
            ao_samples: settings.ao_samples,
            ao_radius: settings.ao_radius,
            ..cyberremesh::BakeParams::default()
        };

        let stem = into
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "cozido".to_string());
        let directory = into.parent().unwrap_or(std::path::Path::new("."));

        let mut written = Vec::new();
        let mut refused = Vec::new();
        let total = settings.maps.len() as f32;
        for (at, map) in settings.maps.iter().enumerate() {
            if cancelled() {
                return Err("a cozedura foi cancelada".to_string());
            }
            progress(at as f32 / total, map.label());

            let engine_map = match map {
                BakeMap::Normal => cyberremesh::FieldMap::Normal,
                BakeMap::AmbientOcclusion => cyberremesh::FieldMap::AmbientOcclusion,
                BakeMap::Curvature => cyberremesh::FieldMap::Curvature,
                BakeMap::Cavity => cyberremesh::FieldMap::Cavity,
            };
            // Each map is attempted and its refusal recorded rather than
            // ending the run: three maps written and one refused is a useful
            // outcome, and reporting it as a failure would throw the three
            // away.
            match cyberremesh::bake_field(&low, engine_map, params, &field) {
                Ok(image) => {
                    let path = directory.join(format!("{stem}_{}.png", map.suffix()));
                    match image.save_png(&path) {
                        Ok(()) => written.push(BakedMap {
                            map: *map,
                            path,
                            width: image.width(),
                            height: image.height(),
                        }),
                        Err(e) => refused.push((*map, format!("não pôde ser escrito: {e}"))),
                    }
                }
                Err(e) => refused.push((*map, e.to_string())),
            }
        }

        if written.is_empty() {
            let why = refused
                .iter()
                .map(|(map, e)| format!("{}: {e}", map.label()))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!("nenhum mapa foi cozido — {why}"));
        }
        Ok(BakeResult { written, refused })
    }
}

// -- conform ----------------------------------------------------------------

impl ConformModel for ClayDocument {
    fn can_conform(&self) -> Result<(), String> {
        let (representation, carries) = self.active_layer_shape();
        if representation != Representation::Mesh {
            return Err(format!(
                "conformar move os vértices de uma malha para a superfície \
                 actual; esta camada é {representation:?}"
            ));
        }
        if !carries {
            return Err("esta camada de malha ainda não carrega triângulos".to_string());
        }
        // The second surface is the one this refusal is really about, and it
        // is why this check says more than the other three tools' do: a
        // conform needs a field to conform *to*, and a scene of nothing but
        // meshes has none.
        if !self.has_field_surface() {
            return Err(
                "conformar precisa de um campo para onde ir; este documento \
                 não tem nenhum"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn conform_source(&mut self) -> Result<ConformSource, ModelError> {
        self.can_conform().map_err(ModelError::engine)?;
        let edit = self.uv_source()?;
        // The sculpt as it is *now*, marched from the field rather than read
        // from a mesh layer: the whole premise of a conform is that the field
        // has moved since the retopology, so the target has to come from the
        // field and not from whatever mesh was made of it earlier.
        let target_mesh = self
            .document()
            .mesh(claycore::MeshParams {
                voxel_size: Some(Self::VOXEL_SIZE),
                mesher: claycore::Mesher::MarchingTetrahedra,
                ..claycore::MeshParams::default()
            })
            .map_err(ModelError::engine)?;
        let target = UvSource {
            positions: target_mesh.positions().to_vec(),
            normals: target_mesh
                .normals()
                .map(|n| n.to_vec())
                .unwrap_or_default(),
            indices: target_mesh.indices().to_vec(),
            name: "campo".to_string(),
        };
        Ok(ConformSource { edit, target })
    }

    fn apply_conform(&mut self, result: &ConformResult) -> Result<(), ModelError> {
        self.move_active_mesh_vertices(&result.positions)
    }
}

/// The conform stage, off the interface thread.
#[derive(Debug, Default, Clone, Copy)]
pub struct EngineConformer;

impl Conformer for EngineConformer {
    fn run(
        &self,
        source: &ConformSource,
        settings: ConformSettings,
        progress: &dyn Fn(f32, &str),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ConformResult, String> {
        let settings = settings.sanitized();
        if cancelled() {
            return Err("a conformação foi cancelada".to_string());
        }
        progress(0.0, "a entregar as malhas");

        let across = |surface: &UvSource| -> Result<cyberremesh::Mesh, String> {
            let positions: Vec<f32> = surface.positions.iter().flat_map(|p| *p).collect();
            let normals: Vec<f32> = surface.normals.iter().flat_map(|n| *n).collect();
            cyberremesh::Mesh::from_handoff(
                &positions,
                &normals,
                &surface.indices,
                claycore::HANDOFF_VERSION,
                "ClaySpaceDesktop",
            )
            .map_err(|e| format!("a entrega da malha foi recusada: {e}"))
        };
        let mut edit = across(&source.edit)?;
        let target = across(&source.target)?;

        progress(0.5, "a conformar");
        // A cap rather than everything: the flagged list is for a sculptor to
        // look at, and ten thousand indices is not something anyone looks at.
        // The count reports the true total either way, which is what the
        // report is judged by.
        const FLAGGED_CAP: usize = 512;
        let conformed = cyberremesh::conform(&mut edit, &target, settings.threshold, FLAGGED_CAP)
            .map_err(|e| format!("a conformação foi recusada: {e}"))?;

        Ok(ConformResult {
            positions: edit.positions(),
            outcome: ConformOutcome {
                moved_vertices: conformed.moved_vertices,
                max_deviation: conformed.max_deviation,
                rms_deviation: conformed.rms_deviation,
                flagged_count: conformed.flagged_count,
                flagged_returned: conformed.flagged.len(),
            },
        })
    }
}
