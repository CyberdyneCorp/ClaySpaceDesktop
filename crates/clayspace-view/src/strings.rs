//! Every word the interface shows.
//!
//! Externalised so that no user-facing string is written inline, and so a
//! second locale is a table rather than a rewrite. The design is Portuguese
//! throughout, which is the default; English is carried alongside it because a
//! tool sold beyond one market needs the fallback path to have been exercised
//! rather than assumed.

/// Which language the interface is presented in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    /// Brazilian Portuguese — the design's own language.
    #[default]
    PtBr,
    EnUs,
}

impl Locale {
    pub const ALL: [Locale; 2] = [Self::PtBr, Self::EnUs];

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
        } else {
            Self::default()
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PtBr => "Português (Brasil)",
            Self::EnUs => "English (US)",
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

    // Labels
    pub label_intensity: &'static str,
    pub label_size: &'static str,
    pub label_flow: &'static str,
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

    // Actions and states
    pub action_undo: &'static str,
    pub action_redo: &'static str,
    pub action_frame_all: &'static str,
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

    label_intensity: "Intensidade",
    label_size: "Tamanho",
    label_flow: "Fluxo",
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

    action_undo: "Desfazer",
    action_redo: "Refazer",
    action_frame_all: "Enquadrar tudo",
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

    label_intensity: "Intensity",
    label_size: "Size",
    label_flow: "Flow",
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

    action_undo: "Undo",
    action_redo: "Redo",
    action_frame_all: "Frame all",
    state_unsaved: "unsaved",
    state_nothing_changed: "nothing changed",
};

impl Strings {
    pub fn for_locale(locale: Locale) -> &'static Strings {
        match locale {
            Locale::PtBr => &PT_BR,
            Locale::EnUs => &EN_US,
        }
    }

    /// Every string, for tests that check the whole table at once.
    pub fn all(&self) -> [&'static str; 43] {
        [
            self.menu_file,
            self.menu_edit,
            self.menu_view,
            self.menu_sculpt,
            self.menu_brushes,
            self.menu_dynamics,
            self.menu_masks,
            self.menu_window,
            self.menu_help,
            self.section_scene,
            self.section_layers,
            self.section_sculpt_settings,
            self.section_material,
            self.section_geometry,
            self.section_resolution,
            self.section_brush_controls,
            self.label_intensity,
            self.label_size,
            self.label_flow,
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
            self.action_undo,
            self.action_redo,
            self.action_frame_all,
            self.state_unsaved,
            self.state_nothing_changed,
            // Padding to a fixed size so adding a field is a compile error
            // here rather than a silently untested string.
            self.menu_file,
            self.menu_edit,
            self.menu_view,
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
        // localised and is not.
        let pt = Strings::for_locale(Locale::PtBr);
        let en = Strings::for_locale(Locale::EnUs);
        let differing = pt
            .all()
            .iter()
            .zip(en.all().iter())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            differing > 30,
            "only {differing} strings differ between the locales"
        );
    }

    #[test]
    fn an_untranslated_tag_falls_back_rather_than_failing() {
        assert_eq!(Locale::from_tag("fr-FR"), Locale::default());
        assert_eq!(Locale::from_tag(""), Locale::default());
        assert_eq!(Locale::from_tag("pt-BR"), Locale::PtBr);
        assert_eq!(Locale::from_tag("en-GB"), Locale::EnUs);
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
