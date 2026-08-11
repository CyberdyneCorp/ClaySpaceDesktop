//! The sculpting vocabulary the interface offers.
//!
//! Every tool here names the engine verb it invokes. A tool with no engine
//! counterpart is not offered, and a label never binds to a verb that does
//! something adjacent to what it says: the mapping follows the engine's own
//! ZBrush-equivalence table rather than an invention of ours.
//!
//! Where a verb exists on one representation only — carve-with-alpha is
//! voxel-side, flatten needs a region on the SDF side — the tool reports
//! itself unavailable *with a reason* rather than being offered and then
//! quietly doing nothing.

/// Which representation a layer holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Representation {
    /// An ordered edit list evaluated as a distance field.
    Sdf,
    /// A palette-indexed voxel grid.
    Voxel,
    /// An imported mesh the document carries but never sculpts.
    Mesh,
}

impl Representation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sdf => "SDF",
            Self::Voxel => "voxel",
            Self::Mesh => "mesh",
        }
    }
}

/// A tool the interface can offer.
///
/// Named as the interface names them, which is Portuguese, because the label
/// and the tool are the same thing to a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolKind {
    /// Displaces the accumulated surface along its normal. ZBrush's Standard.
    Padrao,
    /// Drags the assembled surface. Buds rather than stretches.
    Mover,
    /// Relief on the SDF side; dilation on the voxel side.
    Inflar,
    /// Relax on the SDF side; a majority filter on the voxel side.
    Suavizar,
    /// Freezes a region against every verb.
    Mascara,
    /// Snakehook: pulls a lobe out, adding material.
    Puxar,
    /// Flatten and smooth from one snapshot.
    Raspar,
    /// Flatten in cut-only mode, which is what keeps a facet crisp.
    Planar,
    /// Fills narrow pockets. Local, so it fills what is narrow, not enclosed.
    Preencher,
    /// Magnify with a negative strength.
    Pincar,
    /// A stroke with clamped accumulation.
    Camada,
    /// Smudge: drags the surface skin, leaving the interior.
    Nudge,
    /// hPolish: planes without filling.
    Polir,
    /// Relax, applied as a brush.
    Relaxar,
    /// The cut tool. The practitioners' ninety-percent tool.
    Trim,
}

/// Why a tool cannot be used right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// The verb exists on the other representation only.
    WrongRepresentation {
        needs: Representation,
        active: Representation,
    },
    /// The layer is ghosted or locked.
    LayerProtected,
    /// Mesh layers are carried, not sculpted.
    MeshLayer,
}

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongRepresentation { needs, active } => write!(
                f,
                "applies to {} layers; this one is {}",
                needs.label(),
                active.label()
            ),
            Self::LayerProtected => f.write_str("this layer is locked"),
            Self::MeshLayer => {
                f.write_str("mesh layers are carried, not sculpted")
            }
        }
    }
}

/// What a tool needs from the layer it is applied to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Requires {
    /// Works on either representation.
    Either,
    Sdf,
    Voxel,
}

impl ToolKind {
    /// Every tool, in the order the brush shelf presents them.
    pub const ALL: [ToolKind; 15] = [
        Self::Padrao,
        Self::Inflar,
        Self::Suavizar,
        Self::Mover,
        Self::Pincar,
        Self::Raspar,
        Self::Planar,
        Self::Preencher,
        Self::Camada,
        Self::Mascara,
        Self::Puxar,
        Self::Polir,
        Self::Relaxar,
        Self::Nudge,
        Self::Trim,
    ];

    /// The label the interface shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Padrao => "Padrão",
            Self::Mover => "Mover",
            Self::Inflar => "Inflar",
            Self::Suavizar => "Suavizar",
            Self::Mascara => "Máscara",
            Self::Puxar => "Puxar",
            Self::Raspar => "Raspar",
            Self::Planar => "Planar",
            Self::Preencher => "Preencher",
            Self::Pincar => "Pinçar",
            Self::Camada => "Camada",
            Self::Nudge => "Nudge",
            Self::Polir => "Polir",
            Self::Relaxar => "Relaxar",
            Self::Trim => "Trim",
        }
    }

    /// The engine entry point this tool invokes.
    ///
    /// Stated so that no tool can exist here without one, and so a reader can
    /// check a binding against the engine's documentation without reading the
    /// implementation.
    pub fn engine_verb(self) -> &'static str {
        match self {
            Self::Padrao => "clay_layer_apply_stroke (CLAY_OP_RELIEF)",
            Self::Mover => "clay_layer_move_surface",
            Self::Inflar => "clay_voxel_sculpt_inflate / CLAY_OP_RELIEF",
            Self::Suavizar => "clay_item_volume_relax / clay_voxel_sculpt_smooth",
            Self::Mascara => "clay_mask_apply_stroke",
            Self::Puxar => "clay_item_set_curve_points (snakehook)",
            Self::Raspar => "clay_voxel_sculpt_scrape",
            Self::Planar => "clay_item_volume_flatten (cut-only)",
            Self::Preencher => "clay_voxel_sculpt_fill_cavities",
            Self::Pincar => "clay_voxel_sculpt_pinch / magnify (negative)",
            Self::Camada => "clay_layer_apply_stroke (clamped accumulation)",
            Self::Nudge => "clay_voxel_sculpt_smudge",
            Self::Polir => "clay_item_volume_flatten (cut-only)",
            Self::Relaxar => "clay_item_volume_relax",
            Self::Trim => "clay_cut_create",
        }
    }

    fn requires(self) -> Requires {
        match self {
            // Both representations carry these.
            Self::Padrao
            | Self::Inflar
            | Self::Suavizar
            | Self::Mascara
            | Self::Camada => Requires::Either,
            // Field-side only: these act on the assembled surface or on a
            // sampled volume.
            Self::Mover | Self::Puxar | Self::Planar | Self::Polir | Self::Relaxar
            | Self::Trim => Requires::Sdf,
            // Voxel-side only: cell walks with no field equivalent yet.
            Self::Raspar | Self::Preencher | Self::Pincar | Self::Nudge => Requires::Voxel,
        }
    }

    /// Whether this tool can be applied to a layer, and why not if it cannot.
    pub fn availability(
        self,
        representation: Representation,
        editable: bool,
    ) -> Result<(), Unavailable> {
        if representation == Representation::Mesh {
            return Err(Unavailable::MeshLayer);
        }
        if !editable {
            return Err(Unavailable::LayerProtected);
        }
        match (self.requires(), representation) {
            (Requires::Either, _) => Ok(()),
            (Requires::Sdf, Representation::Sdf) => Ok(()),
            (Requires::Voxel, Representation::Voxel) => Ok(()),
            (Requires::Sdf, active) => Err(Unavailable::WrongRepresentation {
                needs: Representation::Sdf,
                active,
            }),
            (Requires::Voxel, active) => Err(Unavailable::WrongRepresentation {
                needs: Representation::Voxel,
                active,
            }),
        }
    }

    /// Whether the tool paints a mask rather than moving the surface.
    pub fn is_mask_tool(self) -> bool {
        self == Self::Mascara
    }
}

/// Which standard view is active. Mirrors the renderer's presets without the
/// ViewModel layer depending on the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewPresetKind {
    #[default]
    Perspective,
    Front,
    Side,
    Top,
}

impl ViewPresetKind {
    pub const ALL: [ViewPresetKind; 4] = [Self::Perspective, Self::Front, Self::Side, Self::Top];

    pub fn label(self) -> &'static str {
        match self {
            Self::Perspective => "Perspectiva",
            Self::Front => "Frontal",
            Self::Side => "Lateral",
            Self::Top => "Superior",
        }
    }
}

/// What a brush is set to.
///
/// Held per tool, so switching away and back returns the settings the user
/// left rather than a default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushSettings {
    /// Radius in **document units**, not pixels.
    ///
    /// The design's tool bar reads "Tamanho 38 px", which is a screen measure
    /// that maps through the zoom; the engine takes a world radius. Treating
    /// the number as world units directly gives a brush covering a third of a
    /// unit-sized model — and re-meshing what such a dab dirties costs several
    /// times the latency budget.
    pub size: f32,
    /// How hard each stamp bites, 0..=1.
    pub intensity: f32,
    /// How much of the stroke each stamp contributes, 0..=1.
    pub flow: f32,
}

impl Default for BrushSettings {
    fn default() -> Self {
        // Intensity and flow are the design's; the radius is a detail brush
        // on a unit-scale model, which is what "38 px" amounts to at a normal
        // framing.
        Self {
            size: 0.08,
            intensity: 0.65,
            flow: 0.80,
        }
    }
}

impl BrushSettings {
    /// Clamps to the ranges the engine accepts.
    ///
    /// A zero or negative radius is rejected by the engine, so it is clamped
    /// here rather than turned into an error the user cannot act on.
    pub fn sanitized(self) -> Self {
        Self {
            size: self.size.clamp(0.001, 100.0),
            intensity: self.intensity.clamp(0.0, 1.0),
            flow: self.flow.clamp(0.01, 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_names_an_engine_verb() {
        for tool in ToolKind::ALL {
            let verb = tool.engine_verb();
            assert!(
                verb.starts_with("clay_"),
                "{} does not name an engine entry point: {verb}",
                tool.label()
            );
        }
    }

    #[test]
    fn every_tool_has_a_distinct_label() {
        for (i, a) in ToolKind::ALL.iter().enumerate() {
            for b in ToolKind::ALL.iter().skip(i + 1) {
                assert_ne!(a.label(), b.label(), "two tools share a label");
            }
        }
    }

    #[test]
    fn a_voxel_only_tool_is_refused_on_an_sdf_layer_with_a_reason() {
        let error = ToolKind::Raspar
            .availability(Representation::Sdf, true)
            .expect_err("scrape is voxel-side");
        assert!(
            error.to_string().contains("voxel"),
            "the refusal must name what the tool needs: {error}"
        );
    }

    #[test]
    fn an_sdf_only_tool_is_refused_on_a_voxel_layer_with_a_reason() {
        let error = ToolKind::Mover
            .availability(Representation::Voxel, true)
            .expect_err("the move brush is field-side");
        assert!(error.to_string().contains("SDF"), "{error}");
    }

    #[test]
    fn switching_to_a_supporting_layer_re_enables_a_tool() {
        assert!(ToolKind::Raspar.availability(Representation::Sdf, true).is_err());
        assert!(
            ToolKind::Raspar.availability(Representation::Voxel, true).is_ok(),
            "the tool must become available without being reselected"
        );
    }

    #[test]
    fn no_tool_is_offered_on_a_mesh_layer() {
        for tool in ToolKind::ALL {
            let error = tool
                .availability(Representation::Mesh, true)
                .expect_err("mesh layers are carried, not sculpted");
            assert_eq!(error, Unavailable::MeshLayer, "{}", tool.label());
        }
    }

    #[test]
    fn no_tool_is_offered_on_a_protected_layer() {
        for tool in ToolKind::ALL {
            for representation in [Representation::Sdf, Representation::Voxel] {
                assert_eq!(
                    tool.availability(representation, false),
                    Err(Unavailable::LayerProtected),
                    "{} on a protected {} layer",
                    tool.label(),
                    representation.label()
                );
            }
        }
    }

    #[test]
    fn every_tool_works_on_at_least_one_representation() {
        for tool in ToolKind::ALL {
            let usable = [Representation::Sdf, Representation::Voxel]
                .iter()
                .any(|r| tool.availability(*r, true).is_ok());
            assert!(usable, "{} can never be used", tool.label());
        }
    }

    #[test]
    fn brush_settings_are_clamped_to_what_the_engine_accepts() {
        let settings = BrushSettings {
            size: -5.0,
            intensity: 4.0,
            flow: 0.0,
        }
        .sanitized();
        assert!(settings.size > 0.0, "a non-positive radius is rejected by the engine");
        assert!(settings.intensity <= 1.0);
        assert!(settings.flow > 0.0);
    }
}
