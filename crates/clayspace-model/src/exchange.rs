//! Geometry in and out.
//!
//! Two ways in, because they answer different questions. A mesh *layer* is
//! carried verbatim — a scan, a scale reference, a kit part, geometry that has
//! to leave the pipeline as what it entered as. A mesh sampled into a *field*
//! becomes clay and can be sculpted, at the cost of being resampled. Choosing
//! between them is the import dialog's only real decision, so it is named
//! rather than inferred from a checkbox.

use std::path::Path;

use crate::sculpt::ModelError;

/// What an imported mesh becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportAs {
    /// Kept as triangles, on its own layer. Not sculptable.
    Reference,
    /// Sampled into a field, and sculptable from then on.
    Clay,
}

impl ImportAs {
    pub const ALL: [ImportAs; 2] = [Self::Reference, Self::Clay];

    pub fn label(self) -> &'static str {
        match self {
            Self::Reference => "Referência",
            Self::Clay => "Argila",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::Reference => "mantém os triângulos; não é esculpível",
            Self::Clay => "reamostra para um campo; passa a ser esculpível",
        }
    }
}

/// How an import is bounded and scaled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportSettings {
    pub becomes: ImportAs,
    /// A uniform scale baked into the stored geometry, so a unit conversion is
    /// resolved once at import rather than approximated by a layer transform.
    pub scale: f32,
    /// The ceiling this host allows, checked against the file's declared
    /// counts before anything is allocated.
    pub max_vertices: u64,
    pub max_triangles: u64,
}

impl Default for ImportSettings {
    fn default() -> Self {
        Self {
            becomes: ImportAs::Reference,
            scale: 1.0,
            // Well under the engine's 50M default. A desktop that tries to
            // carry fifty million vertices has already lost the frame budget,
            // and a ceiling that is never reached is not a ceiling.
            max_vertices: 8_000_000,
            max_triangles: 16_000_000,
        }
    }
}

/// Which mesher an export uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMesher {
    /// Watertight and 2-manifold by construction. The default, because an
    /// export usually leaves for something that will print or subdivide it.
    Watertight,
    /// Surface nets: faster, and not manifold.
    Fast,
    /// Dual contouring: keeps sharp features. Experimental upstream.
    Sharp,
}

impl ExportMesher {
    pub const ALL: [ExportMesher; 3] = [Self::Watertight, Self::Fast, Self::Sharp];

    pub fn label(self) -> &'static str {
        match self {
            Self::Watertight => "Estanque",
            Self::Fast => "Rápido",
            Self::Sharp => "Arestas vivas",
        }
    }

    /// What is given up by choosing it, or `None` for the safe one.
    pub fn caveat(self) -> Option<&'static str> {
        match self {
            Self::Watertight => None,
            Self::Fast => Some("não é uma malha manifold"),
            Self::Sharp => Some("experimental no motor"),
        }
    }
}

/// What an export writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExportSettings {
    pub mesher: ExportMesher,
    /// Cell size for the export grid. Smaller is finer and slower.
    pub resolution: f32,
    /// Target triangle ratio, or `None` to leave the mesh undecimated.
    pub decimate_to: Option<f32>,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            mesher: ExportMesher::Watertight,
            resolution: 0.02,
            decimate_to: None,
        }
    }
}

/// A file format this application reads or writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Obj,
    Ply,
    Fbx,
    Glb,
}

impl Format {
    pub const ALL: [Format; 4] = [Self::Obj, Self::Ply, Self::Fbx, Self::Glb];

    pub fn extension(self) -> &'static str {
        match self {
            Self::Obj => "obj",
            Self::Ply => "ply",
            Self::Fbx => "fbx",
            Self::Glb => "glb",
        }
    }

    pub fn of(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_string_lossy().to_lowercase();
        Self::ALL
            .into_iter()
            .find(|format| format.extension() == extension)
    }

    /// Whether the engine's importer reads it.
    ///
    /// GLB is written and not read: `clay_mesh_save` takes it and
    /// `clay_mesh_load` does not. Stated here so the file dialog can offer
    /// what actually works rather than what the specification hoped for.
    pub fn can_import(self) -> bool {
        !matches!(self, Self::Glb)
    }

    pub fn can_export(self) -> bool {
        true
    }

    /// What this format will not carry out of the door.
    ///
    /// Warned about before the write rather than discovered in the file, which
    /// is the specification's "attribute-support warnings".
    pub fn drops(self) -> &'static [&'static str] {
        match self {
            // PLY carries colour well and has no standard texture coordinates.
            Self::Ply => &["coordenadas de textura"],
            // FBX round trips through the engine's writer without vertex
            // colour, which is what a sculpt's polypaint would need.
            Self::Fbx => &["cores de vértice"],
            Self::Obj | Self::Glb => &[],
        }
    }
}

/// A warning worth showing before an export is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportWarning {
    pub message: String,
}

impl ExportWarning {
    /// Everything worth saying about writing this document in this format.
    ///
    /// Assembled from what the format cannot carry and what the mesher gives
    /// up. Both are knowable before the write, and both are the kind of thing
    /// that is otherwise found out by opening the file somewhere else.
    pub fn for_export(
        format: Format,
        settings: ExportSettings,
        has_mesh_layers: bool,
    ) -> Vec<Self> {
        let mut warnings = Vec::new();
        for dropped in format.drops() {
            warnings.push(Self {
                message: format!("{} não guarda {dropped}", format.extension().to_uppercase()),
            });
        }
        if let Some(caveat) = settings.mesher.caveat() {
            warnings.push(Self {
                message: format!("{}: {caveat}", settings.mesher.label()),
            });
        }
        if has_mesh_layers {
            // The engine's concat rule: an attribute present on some inputs
            // and absent on others is dropped from the result. The meshed
            // field always carries normals, so any mesh layer without them
            // costs the export its normals entirely.
            warnings.push(Self {
                message: "camadas de malha sem normais removem as normais do resultado".to_string(),
            });
        }
        warnings
    }
}

/// Reading and writing geometry.
pub trait ExchangeModel {
    /// Brings a file in, as a reference layer or as clay.
    fn import_mesh(&mut self, path: &Path, settings: ImportSettings) -> Result<(), ModelError>;

    /// Writes the document — field and every visible mesh layer — to a file.
    fn export_mesh(&mut self, path: &Path, settings: ExportSettings) -> Result<(), ModelError>;

    /// Whether the document carries mesh layers, which changes what an export
    /// can promise.
    fn has_mesh_layers(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_format_is_recognised_whatever_the_case() {
        assert_eq!(Format::of(Path::new("/a/b.OBJ")), Some(Format::Obj));
        assert_eq!(Format::of(Path::new("/a/b.ply")), Some(Format::Ply));
        assert_eq!(Format::of(Path::new("/a/b.Glb")), Some(Format::Glb));
        assert_eq!(Format::of(Path::new("/a/b.blend")), None);
        assert_eq!(Format::of(&PathBuf::from("/a/b")), None);
    }

    #[test]
    fn glb_can_be_written_and_not_read() {
        // The engine's asymmetry, stated so the file dialog offers what works.
        assert!(Format::Glb.can_export());
        assert!(!Format::Glb.can_import());
        for format in [Format::Obj, Format::Ply, Format::Fbx] {
            assert!(format.can_import(), "{format:?} should import");
        }
    }

    #[test]
    fn a_format_that_drops_an_attribute_says_so_before_the_write() {
        let warnings = ExportWarning::for_export(Format::Ply, ExportSettings::default(), false);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("PLY"));
        assert!(warnings[0].message.contains("textura"));
    }

    #[test]
    fn the_safe_mesher_warns_about_nothing() {
        assert_eq!(ExportSettings::default().mesher, ExportMesher::Watertight);
        assert!(
            ExportWarning::for_export(Format::Obj, ExportSettings::default(), false).is_empty()
        );
    }

    #[test]
    fn a_faster_mesher_says_what_it_gives_up() {
        let settings = ExportSettings {
            mesher: ExportMesher::Fast,
            ..Default::default()
        };
        let warnings = ExportWarning::for_export(Format::Obj, settings, false);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("manifold"));
    }

    #[test]
    fn mesh_layers_warn_about_the_attribute_that_the_whole_export_loses() {
        // The engine's concat rule bites the *result*, not the layer: one mesh
        // layer with no normals costs the entire export its normals.
        let warnings = ExportWarning::for_export(Format::Obj, ExportSettings::default(), true);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("normais"));
    }

    #[test]
    fn warnings_accumulate_rather_than_replacing_each_other() {
        let settings = ExportSettings {
            mesher: ExportMesher::Sharp,
            ..Default::default()
        };
        let warnings = ExportWarning::for_export(Format::Fbx, settings, true);
        assert_eq!(warnings.len(), 3, "{warnings:?}");
    }

    #[test]
    fn the_import_ceiling_is_lower_than_the_engines_own() {
        // A ceiling that is never reached is not a ceiling. The engine's
        // default is 50M vertices; a desktop that carries that has already
        // lost the frame budget.
        let settings = ImportSettings::default();
        assert!(settings.max_vertices < 50_000_000);
        assert!(settings.max_triangles > settings.max_vertices);
    }
}
