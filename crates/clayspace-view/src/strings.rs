//! Every word the interface shows.
//!
//! Externalised so that no user-facing string is written inline, and so a
//! further locale is a table rather than a rewrite. The design is Portuguese
//! throughout, which is the default; English and Spanish are carried alongside
//! it because a tool sold beyond one market needs the fallback path to have
//! been exercised rather than assumed.

/// Which language the interface is presented in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    /// Brazilian Portuguese — the design's own language.
    #[default]
    PtBr,
    EnUs,
    /// Latin American Spanish rather than Castilian: the design is Brazilian,
    /// so the market next door is the one this reaches first, and its
    /// vocabulary is the one those artists already use.
    Es419,
}

impl Locale {
    pub const ALL: [Locale; 3] = [Self::PtBr, Self::EnUs, Self::Es419];

    /// Picks a locale from a system tag, falling back to the default.
    ///
    /// A tag with no translation gets the default rather than untranslated
    /// keys, which is the difference between an interface in the wrong
    /// language and one that is broken.
    pub fn from_tag(tag: &str) -> Self {
        let tag = tag.to_ascii_lowercase();
        if tag.starts_with("pt") {
            Self::PtBr
        } else if tag.starts_with("en") {
            Self::EnUs
        } else if tag.starts_with("es") {
            // Every Spanish tag, Castilian included: a Madrid interface in
            // Latin American Spanish is read; one in Portuguese is not.
            Self::Es419
        } else {
            Self::default()
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PtBr => "Português (Brasil)",
            Self::EnUs => "English (US)",
            Self::Es419 => "Español (Latinoamérica)",
        }
    }
}

/// Everything the interface says.
///
/// One struct rather than a lookup by key: a missing string is then a compile
/// error rather than a placeholder that ships.
#[derive(Debug, Clone, Copy)]
pub struct Strings {
    // Menus
    pub menu_file: &'static str,
    pub menu_edit: &'static str,
    pub menu_view: &'static str,
    pub menu_sculpt: &'static str,
    pub menu_brushes: &'static str,
    pub menu_dynamics: &'static str,
    pub menu_masks: &'static str,
    pub action_extrude: &'static str,
    pub menu_window: &'static str,
    pub menu_help: &'static str,

    // Sections
    pub section_scene: &'static str,
    pub section_layers: &'static str,
    pub section_sculpt_settings: &'static str,
    pub section_material: &'static str,
    pub section_geometry: &'static str,
    pub section_resolution: &'static str,
    pub section_brush_controls: &'static str,
    pub section_armature: &'static str,
    pub section_diagnostics: &'static str,

    // Labels
    pub label_intensity: &'static str,
    pub label_size: &'static str,
    pub label_flow: &'static str,
    /// How an SDF edit meets what is under it.
    pub label_combine: &'static str,
    /// How sharply the join is made.
    pub label_blend: &'static str,
    /// The scalar stamp modulating a brush.
    pub label_alpha: &'static str,
    /// Loading one.
    pub action_load_alpha: &'static str,
    /// Dropping it.
    pub action_clear_alpha: &'static str,
    /// Shown where none is loaded.
    pub alpha_none: &'static str,
    pub label_symmetry: &'static str,
    pub label_resolution: &'static str,
    pub label_smoothing: &'static str,
    pub label_voxel_size: &'static str,
    pub label_noise: &'static str,
    pub label_edge: &'static str,
    pub label_accumulate: &'static str,
    pub label_mirror: &'static str,
    pub label_polygons: &'static str,
    pub label_vertices: &'static str,
    pub label_triangles: &'static str,
    pub label_objects: &'static str,
    pub label_memory: &'static str,
    pub label_units: &'static str,
    pub label_backend: &'static str,
    pub label_new_layer: &'static str,
    pub label_spheres: &'static str,
    pub label_skin: &'static str,
    pub label_mirror_new: &'static str,

    // Actions and states
    /// Shown where the shelf would be, on a layer whose representation this
    /// application has no verb bound for yet.
    pub shelf_no_tools: &'static str,
    // Pre-bake repair.
    pub action_repair: &'static str,
    pub repair_airtight: &'static str,
    pub repair_voids: &'static str,
    pub repair_largest: &'static str,
    pub repair_close_holes: &'static str,
    pub repair_fill_voids: &'static str,
    pub repair_voxel_only: &'static str,
    // The conversion panel.
    pub action_convert: &'static str,
    pub label_convert_to: &'static str,
    pub label_cell_size: &'static str,
    pub label_convert_costs: &'static str,
    pub convert_surface_moves: &'static str,
    pub convert_features_vanish: &'static str,
    pub convert_sharp_edges_lost: &'static str,
    pub convert_history_lost: &'static str,
    pub convert_not_undoable: &'static str,
    pub convert_cells: &'static str,
    pub convert_run: &'static str,
    pub convert_none_here: &'static str,
    /// Prefix for the active layer's representation in the viewport bar.
    pub representation_label: &'static str,
    /// Said when a layer change forced a different tool.
    pub tool_substituted: &'static str,
    pub action_undo: &'static str,
    pub action_redo: &'static str,
    pub action_frame_all: &'static str,
    pub action_new: &'static str,
    pub action_open: &'static str,
    pub action_open_recent: &'static str,
    pub action_save: &'static str,
    pub action_save_as: &'static str,
    pub action_import: &'static str,
    pub action_export: &'static str,
    pub action_choose_file: &'static str,
    pub label_import_as: &'static str,
    pub label_scale: &'static str,
    pub label_mesher: &'static str,
    pub label_export_resolution: &'static str,
    pub label_decimate: &'static str,
    pub label_keep: &'static str,
    pub section_warnings: &'static str,
    pub action_quit: &'static str,
    pub state_no_recent: &'static str,
    pub action_diagnostics: &'static str,
    pub action_attribution: &'static str,
    pub action_copy: &'static str,
    pub state_copied: &'static str,
    pub action_armature_new: &'static str,
    pub action_armature_edit: &'static str,
    pub action_armature_remove: &'static str,
    pub action_skin_preview: &'static str,
    pub action_zsphere_negative: &'static str,
    /// What the pointer does while rigging, said once where it is needed.
    pub hint_armature: &'static str,
    pub hint_units: &'static str,
    pub state_unsaved: &'static str,
    pub state_nothing_changed: &'static str,
}

/// The Portuguese strings, which the design specifies.
const PT_BR: Strings = Strings {
    menu_file: "Arquivo",
    menu_edit: "Editar",
    menu_view: "Vista",
    menu_sculpt: "Escultura",
    menu_brushes: "Pincéis",
    menu_dynamics: "Dinâmica",
    menu_masks: "Máscaras",
    action_extrude: "Extrudar",
    menu_window: "Janela",
    menu_help: "Ajuda",

    section_scene: "CENA",
    section_layers: "CAMADAS",
    section_sculpt_settings: "CONFIGURAÇÕES DE ESCULTURA",
    section_material: "MATERIAL",
    section_geometry: "GEOMETRIA",
    section_resolution: "RESOLUÇÃO",
    section_brush_controls: "CONTROLES DE PINCEL",
    section_armature: "ARMADURA",
    section_diagnostics: "DIAGNÓSTICO",

    label_intensity: "Intensidade",
    label_size: "Tamanho",
    label_flow: "Fluxo",
    label_combine: "Operação",
    label_blend: "Junção",
    label_alpha: "Alfa",
    action_load_alpha: "Carregar alfa…",
    action_clear_alpha: "Remover alfa",
    alpha_none: "nenhum alfa carregado",
    label_symmetry: "Simetria",
    label_resolution: "Resolução",
    label_smoothing: "Suavização",
    label_voxel_size: "Tamanho do voxel",
    label_noise: "Ruído",
    label_edge: "Borda",
    label_accumulate: "Acumular",
    label_mirror: "Espelhamento",
    label_polygons: "Polígonos",
    label_vertices: "Vértices",
    label_triangles: "Triângulos",
    label_objects: "Objetos",
    label_memory: "MEMÓRIA",
    label_units: "Unidades",
    label_backend: "Aceleração",
    label_new_layer: "Nova camada",
    label_spheres: "Esferas",
    label_skin: "Pele",
    label_mirror_new: "Espelhar novas",

    shelf_no_tools: "Nenhuma ferramenta para esta representação ainda",
    representation_label: "Representação",
    tool_substituted: "ferramenta trocada: esta camada não tem essa",
    action_convert: "Converter",
    label_convert_to: "Converter para",
    label_cell_size: "Tamanho da célula",
    label_convert_costs: "O que esta travessia custa",
    convert_surface_moves: "a superfície move-se até",
    convert_features_vanish: "detalhes mais finos que isto desaparecem",
    convert_sharp_edges_lost: "arestas vivas viram degraus",
    convert_history_lost: "o histórico paramétrico não volta",
    convert_not_undoable: "não se desfaz: remova a camada criada para voltar atrás",
    convert_cells: "células",
    convert_run: "Converter",
    convert_none_here: "esta camada não tem para onde converter",
    action_repair: "Reparar",
    repair_airtight: "estanque: nenhum vazio fechado",
    repair_voids: "vazios fechados",
    repair_largest: "maior vazio",
    repair_close_holes: "Fechar furos",
    repair_fill_voids: "Preencher vazios",
    repair_voxel_only: "o reparo é para camadas voxel",
    action_undo: "Desfazer",
    action_redo: "Refazer",
    action_frame_all: "Enquadrar tudo",
    action_new: "Novo",
    action_open: "Abrir…",
    action_open_recent: "Abrir recente",
    action_save: "Salvar",
    action_save_as: "Salvar como…",
    action_import: "Importar malha…",
    action_export: "Exportar malha…",
    action_choose_file: "Escolher arquivo…",
    label_import_as: "Trazer como",
    label_scale: "Escala",
    label_mesher: "Malhador",
    label_export_resolution: "Célula",
    label_decimate: "Reduzir triângulos",
    label_keep: "Manter",
    section_warnings: "AVISOS",
    action_quit: "Sair",
    state_no_recent: "nenhum documento recente",
    action_diagnostics: "Diagnóstico",
    action_attribution: "Atribuições",
    action_copy: "Copiar relatório",
    state_copied: "copiado",
    action_armature_new: "Nova armadura",
    action_armature_edit: "Editar armadura",
    action_armature_remove: "Remover esfera",
    action_skin_preview: "Prévia da pele",
    action_zsphere_negative: "Esfera negativa",
    hint_armature: "Arraste de uma esfera para criar a seguinte · Alt arrasta · ⌘ redimensiona",
    hint_units: "Toque para trocar a unidade. Só muda a leitura; nada é redimensionado.",
    state_unsaved: "não salvo",
    state_nothing_changed: "nada mudou",
};

/// The English strings.
const EN_US: Strings = Strings {
    menu_file: "File",
    menu_edit: "Edit",
    menu_view: "View",
    menu_sculpt: "Sculpt",
    menu_brushes: "Brushes",
    menu_dynamics: "Dynamics",
    menu_masks: "Masks",
    action_extrude: "Extrude",
    menu_window: "Window",
    menu_help: "Help",

    section_scene: "SCENE",
    section_layers: "LAYERS",
    section_sculpt_settings: "SCULPT SETTINGS",
    section_material: "MATERIAL",
    section_geometry: "GEOMETRY",
    section_resolution: "RESOLUTION",
    section_brush_controls: "BRUSH CONTROLS",
    section_armature: "ARMATURE",
    section_diagnostics: "DIAGNOSTICS",

    label_intensity: "Intensity",
    label_size: "Size",
    label_flow: "Flow",
    label_combine: "Operation",
    label_blend: "Join",
    label_alpha: "Alpha",
    action_load_alpha: "Load alpha…",
    action_clear_alpha: "Clear alpha",
    alpha_none: "no alpha loaded",
    label_symmetry: "Symmetry",
    label_resolution: "Resolution",
    label_smoothing: "Smoothing",
    label_voxel_size: "Voxel size",
    label_noise: "Noise",
    label_edge: "Edge",
    label_accumulate: "Accumulate",
    label_mirror: "Mirror",
    label_polygons: "Polygons",
    label_vertices: "Vertices",
    label_triangles: "Triangles",
    label_objects: "Objects",
    label_memory: "MEMORY",
    label_units: "Units",
    label_backend: "Acceleration",
    label_new_layer: "New layer",
    label_spheres: "Spheres",
    label_skin: "Skin",
    label_mirror_new: "Mirror new",

    shelf_no_tools: "No tools for this representation yet",
    representation_label: "Representation",
    tool_substituted: "tool changed: this layer has no verb for that one",
    action_convert: "Convert",
    label_convert_to: "Convert to",
    label_cell_size: "Cell size",
    label_convert_costs: "What this crossing costs",
    convert_surface_moves: "the surface moves by up to",
    convert_features_vanish: "features thinner than this vanish",
    convert_sharp_edges_lost: "sharp edges become a staircase",
    convert_history_lost: "the parametric history does not come back",
    convert_not_undoable: "not undoable: remove the layer it adds to take it back",
    convert_cells: "cells",
    convert_run: "Convert",
    convert_none_here: "this layer has nowhere to convert to",
    action_repair: "Repair",
    repair_airtight: "airtight: no enclosed voids",
    repair_voids: "enclosed voids",
    repair_largest: "largest void",
    repair_close_holes: "Close holes",
    repair_fill_voids: "Fill voids",
    repair_voxel_only: "repair is for voxel layers",
    action_undo: "Undo",
    action_redo: "Redo",
    action_frame_all: "Frame all",
    action_new: "New",
    action_open: "Open…",
    action_open_recent: "Open recent",
    action_save: "Save",
    action_save_as: "Save as…",
    action_import: "Import mesh…",
    action_export: "Export mesh…",
    action_choose_file: "Choose file…",
    label_import_as: "Bring in as",
    label_scale: "Scale",
    label_mesher: "Mesher",
    label_export_resolution: "Cell",
    label_decimate: "Reduce triangles",
    label_keep: "Keep",
    section_warnings: "WARNINGS",
    action_quit: "Quit",
    state_no_recent: "no recent documents",
    action_diagnostics: "Diagnostics",
    action_attribution: "Attributions",
    action_copy: "Copy report",
    state_copied: "copied",
    action_armature_new: "New armature",
    action_armature_edit: "Edit armature",
    action_armature_remove: "Remove sphere",
    action_skin_preview: "Skin preview",
    action_zsphere_negative: "Negative sphere",
    hint_armature: "Drag out of a sphere to grow the next · Alt moves · ⌘ resizes",
    hint_units: "Click to change the unit. It changes the reading only; nothing is rescaled.",
    state_unsaved: "unsaved",
    state_nothing_changed: "nothing changed",
};

/// The Latin American Spanish strings.
const ES_419: Strings = Strings {
    menu_file: "Archivo",
    menu_edit: "Editar",
    menu_view: "Vista",
    menu_sculpt: "Escultura",
    menu_brushes: "Pinceles",
    menu_dynamics: "Dinámica",
    menu_masks: "Máscaras",
    action_extrude: "Extruir",
    menu_window: "Ventana",
    menu_help: "Ayuda",

    section_scene: "ESCENA",
    section_layers: "CAPAS",
    section_sculpt_settings: "AJUSTES DE ESCULTURA",
    section_material: "MATERIAL",
    section_geometry: "GEOMETRÍA",
    section_resolution: "RESOLUCIÓN",
    section_brush_controls: "CONTROLES DE PINCEL",
    // "Esqueleto", not "Armadura": armadura reads as armour outside of
    // structural engineering, and Spanish-speaking riggers learned the term
    // from Blender.
    section_armature: "ESQUELETO",
    section_diagnostics: "DIAGNÓSTICO",

    label_intensity: "Intensidad",
    label_size: "Tamaño",
    label_flow: "Flujo",
    label_combine: "Operación",
    label_blend: "Unión",
    label_alpha: "Alfa",
    action_load_alpha: "Cargar alfa…",
    action_clear_alpha: "Quitar alfa",
    alpha_none: "ningún alfa cargado",
    label_symmetry: "Simetría",
    label_resolution: "Resolución",
    label_smoothing: "Suavizado",
    label_voxel_size: "Tamaño del vóxel",
    label_noise: "Ruido",
    label_edge: "Borde",
    label_accumulate: "Acumular",
    // "Reflejo" rather than "Simetría", which the axis toggles already carry;
    // the two are separate controls and must not read as one.
    label_mirror: "Reflejo",
    label_polygons: "Polígonos",
    label_vertices: "Vértices",
    label_triangles: "Triángulos",
    label_objects: "Objetos",
    label_memory: "MEMORIA",
    label_units: "Unidades",
    label_backend: "Aceleración",
    label_new_layer: "Nueva capa",
    label_spheres: "Esferas",
    label_skin: "Piel",
    label_mirror_new: "Reflejar nuevas",

    shelf_no_tools: "Todavía no hay herramientas para esta representación",
    representation_label: "Representación",
    tool_substituted: "herramienta cambiada: esta capa no tiene ese verbo",
    action_convert: "Convertir",
    label_convert_to: "Convertir a",
    label_cell_size: "Tamaño de celda",
    label_convert_costs: "Lo que cuesta este cruce",
    convert_surface_moves: "la superficie se mueve hasta",
    convert_features_vanish: "los detalles más finos que esto desaparecen",
    convert_sharp_edges_lost: "los bordes vivos se vuelven escalones",
    convert_history_lost: "el historial paramétrico no vuelve",
    convert_not_undoable: "no se deshace: quite la capa creada para volver atrás",
    convert_cells: "celdas",
    convert_run: "Convertir",
    convert_none_here: "esta capa no tiene a dónde convertirse",
    action_repair: "Reparar",
    repair_airtight: "estanco: sin huecos cerrados",
    repair_voids: "huecos cerrados",
    repair_largest: "hueco mayor",
    repair_close_holes: "Cerrar agujeros",
    repair_fill_voids: "Rellenar huecos",
    repair_voxel_only: "la reparación es para capas vóxel",
    action_undo: "Deshacer",
    action_redo: "Rehacer",
    action_frame_all: "Encuadrar todo",
    action_new: "Nuevo",
    action_open: "Abrir…",
    action_open_recent: "Abrir reciente",
    action_save: "Guardar",
    action_save_as: "Guardar como…",
    action_import: "Importar malla…",
    action_export: "Exportar malla…",
    action_choose_file: "Elegir archivo…",
    label_import_as: "Traer como",
    label_scale: "Escala",
    label_mesher: "Mallador",
    label_export_resolution: "Celda",
    label_decimate: "Reducir triángulos",
    // The share of triangles that survives, so a different verb from the
    // checkbox above it; repeating "Reducir" would read as one control twice.
    label_keep: "Conservar",
    section_warnings: "ADVERTENCIAS",
    action_quit: "Salir",
    state_no_recent: "sin documentos recientes",
    action_diagnostics: "Diagnóstico",
    action_attribution: "Atribuciones",
    action_copy: "Copiar informe",
    state_copied: "copiado",
    action_armature_new: "Nuevo esqueleto",
    action_armature_edit: "Editar esqueleto",
    action_armature_remove: "Eliminar esfera",
    action_skin_preview: "Vista previa de la piel",
    action_zsphere_negative: "Esfera negativa",
    hint_armature: "Arrastra desde una esfera para crear la siguiente · Alt mueve · ⌘ redimensiona",
    hint_units: "Haz clic para cambiar la unidad. Solo cambia la lectura; no se reescala nada.",
    state_unsaved: "sin guardar",
    state_nothing_changed: "nada cambió",
};

impl Strings {
    pub fn for_locale(locale: Locale) -> &'static Strings {
        match locale {
            Locale::PtBr => &PT_BR,
            Locale::EnUs => &EN_US,
            Locale::Es419 => &ES_419,
        }
    }

    /// Every string, for tests that check the whole table at once.
    pub fn all(&self) -> [&'static str; 80] {
        [
            self.menu_file,
            self.menu_edit,
            self.menu_view,
            self.menu_sculpt,
            self.menu_brushes,
            self.menu_dynamics,
            self.menu_masks,
            self.action_extrude,
            self.menu_window,
            self.menu_help,
            self.section_scene,
            self.section_layers,
            self.section_sculpt_settings,
            self.section_material,
            self.section_geometry,
            self.section_resolution,
            self.section_brush_controls,
            self.section_armature,
            self.section_diagnostics,
            self.label_intensity,
            self.label_size,
            self.label_flow,
            self.label_combine,
            self.label_blend,
            self.label_alpha,
            self.action_load_alpha,
            self.action_clear_alpha,
            self.alpha_none,
            self.label_symmetry,
            self.label_resolution,
            self.label_smoothing,
            self.label_voxel_size,
            self.label_noise,
            self.label_edge,
            self.label_accumulate,
            self.label_mirror,
            self.label_polygons,
            self.label_vertices,
            self.label_triangles,
            self.label_objects,
            self.label_memory,
            self.label_units,
            self.label_backend,
            self.label_new_layer,
            self.label_spheres,
            self.label_skin,
            self.label_mirror_new,
            self.action_undo,
            self.action_redo,
            self.action_frame_all,
            self.action_new,
            self.action_open,
            self.action_open_recent,
            self.action_save,
            self.action_save_as,
            self.action_import,
            self.action_export,
            self.action_choose_file,
            self.label_import_as,
            self.label_scale,
            self.label_mesher,
            self.label_export_resolution,
            self.label_decimate,
            self.label_keep,
            self.section_warnings,
            self.action_quit,
            self.state_no_recent,
            self.action_diagnostics,
            self.action_attribution,
            self.action_copy,
            self.state_copied,
            self.action_armature_new,
            self.action_armature_edit,
            self.action_armature_remove,
            self.action_skin_preview,
            self.action_zsphere_negative,
            self.hint_armature,
            self.hint_units,
            self.state_unsaved,
            self.state_nothing_changed,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locale_translates_every_string() {
        for locale in Locale::ALL {
            let strings = Strings::for_locale(locale);
            for value in strings.all() {
                assert!(!value.is_empty(), "{} has an empty string", locale.label());
            }
        }
    }

    #[test]
    fn the_locales_actually_differ() {
        // A table copied rather than translated is worse than none: it looks
        // localised and is not. Every pair, because languages as close as
        // Portuguese and Spanish are where a copied table would hide.
        for (index, first) in Locale::ALL.iter().enumerate() {
            for second in &Locale::ALL[index + 1..] {
                let differing = Strings::for_locale(*first)
                    .all()
                    .iter()
                    .zip(Strings::for_locale(*second).all().iter())
                    .filter(|(a, b)| a != b)
                    .count();
                assert!(
                    differing > 30,
                    "only {differing} strings differ between {} and {}",
                    first.label(),
                    second.label()
                );
            }
        }
    }

    #[test]
    fn an_untranslated_tag_falls_back_rather_than_failing() {
        assert_eq!(Locale::from_tag("fr-FR"), Locale::default());
        assert_eq!(Locale::from_tag(""), Locale::default());
        assert_eq!(Locale::from_tag("pt-BR"), Locale::PtBr);
        assert_eq!(Locale::from_tag("en-GB"), Locale::EnUs);
        assert_eq!(Locale::from_tag("es-MX"), Locale::Es419);
        assert_eq!(
            Locale::from_tag("es-ES"),
            Locale::Es419,
            "Castilian reads the Latin American table rather than Portuguese"
        );
        assert_eq!(
            Locale::from_tag("EN-US"),
            Locale::EnUs,
            "tags are not case sensitive"
        );
    }

    #[test]
    fn section_headings_are_upper_case_in_every_locale() {
        // The design sets them as small spaced capitals; a lower-case heading
        // would read as body text.
        for locale in Locale::ALL {
            let strings = Strings::for_locale(locale);
            for heading in [
                strings.section_scene,
                strings.section_layers,
                strings.section_material,
                strings.section_geometry,
            ] {
                assert_eq!(
                    heading,
                    heading.to_uppercase(),
                    "{heading} is not set as a heading in {}",
                    locale.label()
                );
            }
        }
    }
}
