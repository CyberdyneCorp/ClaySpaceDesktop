//! Every word the interface shows.
//!
//! Externalised so that no user-facing string is written inline, and so a
//! further locale is a table rather than a rewrite. The design is Portuguese
//! throughout, which is the default; English and Spanish are carried alongside
//! it because a tool sold beyond one market needs the fallback path to have
//! been exercised rather than assumed.

pub use clayspace_model::Locale;

/// Everything the interface says.
///
/// One struct rather than a lookup by key: a missing string is then a compile
/// error rather than a placeholder that ships.
#[derive(Debug, Clone, Copy)]
pub struct Strings {
    /// What each brush is called, in the order of [`ToolKind::ALL`].
    ///
    /// An array rather than twenty named fields because the names are one
    /// vocabulary rather than twenty unrelated strings — and a fixed length
    /// means a tool added without a name for it is a compile error, which is
    /// the same guarantee the named fields give.
    ///
    /// Here rather than on `ToolKind` because a name is a *word*, and the
    /// domain has no language. `ToolKind::label` keeps its Portuguese for the
    /// places that are not the interface: history entries, engine refusals and
    /// the diagnostics report.
    pub tool_names: [&'static str; clayspace_model::ToolKind::ALL.len()],
    /// Every combine operation, in `Combine::ALL` order.
    ///
    /// Here for the same reason `tool_names` is here, and it should have
    /// arrived at the same time: `Combine::label` returned one hardcoded
    /// Portuguese arm per variant and the options bar drew it directly, so a
    /// sculptor on English or Spanish got their menus in their language and
    /// every boolean control in Portuguese. `combine.rs` even documents the
    /// intent — "Not `Combine::label`, which is interface text and
    /// translated" — beside a `label` that was not.
    pub combine_names: [&'static str; clayspace_model::Combine::ALL.len()],
    /// Every blend profile, in `BlendProfile::ALL` order.
    pub blend_names: [&'static str; clayspace_model::BlendProfile::ALL.len()],
    /// Which way an extrusion goes, in `ExtrudeSide::ALL` order.
    pub extrude_side_names: [&'static str; clayspace_model::ExtrudeSide::ALL.len()],
    /// How a grid is drawn, in `VoxelDisplay::ALL` order.
    pub voxel_display_names: [&'static str; clayspace_model::VoxelDisplay::ALL.len()],
    /// The manipulator's three modes, in `GizmoMode::ALL` order.
    pub gizmo_mode_names: [&'static str; clayspace_model::GizmoMode::ALL.len()],

    /// What each shape the picker offers is called, in the order
    /// `Shape::ALL` presents them.
    pub shape_names: [&'static str; clayspace_model::Shape::ALL.len()],
    /// What each of a shape's measurements is called, in the order
    /// `PARAMETER_KEYS` lists them.
    ///
    /// A key is a stable identifier written into saved documents; showing one
    /// to a sculptor is showing them the inside of the file, which is what the
    /// first capture of this panel did.
    pub shape_parameter_names: [&'static str; clayspace_model::PARAMETER_KEYS.len()],
    /// Where an inserted form lands, in `InsertAs::ALL` order — the subtool
    /// first, because that is the default.
    pub insert_as_names: [&'static str; clayspace_model::InsertAs::ALL.len()],
    /// What a layer holds, in `Representation::ALL` order.
    ///
    /// `Representation::label` is the engine's own word ("SDF", "voxel") and
    /// reads the same in every language; these are the words a sculptor is
    /// offered when a new layer asks what it should be.
    pub representation_names: [&'static str; clayspace_model::Representation::ALL.len()],
    /// The shapes panel, and what it does.
    pub action_shapes: &'static str,
    pub label_shape: &'static str,
    /// The heading over the two destinations an insertion can take.
    pub label_insert_as: &'static str,
    /// Putting the picked form into the scene.
    pub action_insert: &'static str,
    /// Bringing a mesh file in as a subtool of its own.
    pub action_insert_mesh: &'static str,
    /// Copying a subtool already in the scene.
    pub action_copy_subtool: &'static str,
    /// Why the word is "copy" and not "instance": the engine has no instancing
    /// (ClayCore #364), so what arrives is independent and the sculptor should
    /// know that before they sculpt on it.
    pub hint_copy_subtool: &'static str,
    /// What representation a new layer should carry.
    pub label_new_layer_kind: &'static str,
    pub action_remove_object: &'static str,
    pub label_placed_objects: &'static str,
    pub label_no_placed_objects: &'static str,
    pub label_object_scale: &'static str,
    pub hint_shapes: &'static str,
    pub hint_uniform_scale: &'static str,
    pub label_shapes_sdf_only: &'static str,
    /// Which language this table is. Carried with the words rather than beside
    /// them, so the language menu's tick and the words on screen cannot
    /// disagree about what the interface is in.
    pub locale: Locale,
    // Menus
    pub menu_file: &'static str,
    pub menu_edit: &'static str,
    pub menu_view: &'static str,
    /// The language submenu.
    pub menu_language: &'static str,
    pub menu_sculpt: &'static str,
    pub menu_brushes: &'static str,
    pub menu_dynamics: &'static str,
    pub menu_masks: &'static str,
    /// Starting and stopping mask painting.
    pub action_paint_mask: &'static str,
    /// The curve section of the inspector.
    pub section_curve: &'static str,
    /// Placing a curve, and letting it go.
    pub action_curve: &'static str,
    pub action_curve_apply: &'static str,
    /// How thick the tube is.
    pub label_curve_radius: &'static str,
    /// How the control points join.
    pub label_curve_join: &'static str,
    /// The tube's cross-section.
    pub label_curve_profile: &'static str,
    /// How to place and shape one.
    pub hint_curve: &'static str,
    /// How a voxel layer is drawn.
    pub label_voxel_display: &'static str,
    /// How much the occupancy is filtered before the smooth surface is taken.
    pub label_voxel_blur: &'static str,
    /// What a blur above zero costs.
    pub hint_voxel_blur: &'static str,
    /// The lattice section of the inspector.
    pub section_lattice: &'static str,
    /// Putting a cage up and taking it down.
    pub action_cage: &'static str,
    /// Bending the layer through the cage.
    pub action_bend: &'static str,
    /// Control points per axis.
    pub label_cage_divisions: &'static str,
    /// Why a cage cannot be put around the active layer.
    pub status_cage_needs_a_field: &'static str,
    /// How to use the cage, where a person is when they need it.
    pub hint_cage: &'static str,
    /// The heading of the question asked when the active subtool changes under
    /// a cage that is still up.
    pub cage_switch_title: &'static str,
    /// Why the cage cannot come along, and what the sculptor has to decide.
    pub cage_switch_question: &'static str,
    /// Bend the form through the cage, then change subtool.
    pub cage_switch_apply: &'static str,
    /// Throw the cage away, then change subtool.
    pub cage_switch_drop: &'static str,
    /// Stay on this subtool and go on working the cage.
    pub cage_switch_stay: &'static str,
    /// Shown while the manipulator is set to turn.
    pub hint_gizmo_rotate: &'static str,
    /// Why turning and scaling are refused on a selection of one.
    pub hint_gizmo_needs_two: &'static str,
    /// The manipulator that acts on the whole active layer.
    pub section_layer_transform: &'static str,
    /// What that manipulator does, and how to put it away.
    pub hint_layer_transform: &'static str,
    /// The mask section of the inspector.
    pub section_mask: &'static str,
    /// How far Expandir, Contrair and Suavizar máscara reach.
    pub label_mask_steps: &'static str,
    /// How many cells are frozen.
    pub label_mask_cells: &'static str,
    /// Why an extrusion is unavailable on the active layer.
    pub status_extrude_needs_a_field: &'static str,
    /// An extrusion's wall thickness.
    pub label_extrude_thickness: &'static str,
    /// Rounding on the extruded rim.
    pub label_extrude_round: &'static str,
    /// Smoothing passes on a copy of the mask, for the rim.
    pub label_extrude_smooth: &'static str,
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
    /// The whole-form deformers.
    pub action_deform: &'static str,
    pub label_axis: &'static str,
    pub label_span: &'static str,
    pub label_scale_start: &'static str,
    pub label_scale_end: &'static str,
    pub label_angle: &'static str,
    /// Shown where a deformer does not apply.
    pub deform_mesh_only: &'static str,
    /// The reference images, behind the sculpt.
    pub action_references: &'static str,
    pub label_reference_opacity: &'static str,
    pub label_reference_size: &'static str,
    pub label_reference_across: &'static str,
    pub label_reference_up: &'static str,
    pub label_reference_depth: &'static str,
    pub action_load_reference: &'static str,
    pub action_clear_reference: &'static str,
    /// Shown on a plane with nothing on it.
    pub reference_none: &'static str,
    /// How much of the clay itself is drawn, so a reference shows through.
    pub label_surface_opacity: &'static str,
    pub hint_surface_opacity: &'static str,
    /// The recorded passes on a voxel layer.
    pub sculpt_begin: &'static str,
    pub sculpt_end: &'static str,
    pub sculpt_recording: &'static str,
    pub sculpt_cells: &'static str,
    pub sculpt_remove: &'static str,
    pub sculpt_merge_down: &'static str,
    pub sculpt_move_up: &'static str,
    pub sculpt_move_down: &'static str,
    /// Shown when the stack is large enough to be worth merging.
    pub sculpt_worth_merging: &'static str,
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
    /// Renaming and removing a layer, from the row's own menu.
    pub action_rename_layer: &'static str,
    pub action_remove_layer: &'static str,
    /// Showing one subtool on its own, and bringing the rest back.
    ///
    /// Two words rather than one that toggles: the entry says what the click
    /// will do, and "solo" on a row that is already alone says neither.
    pub action_solo_layer: &'static str,
    pub action_release_solo: &'static str,
    /// Why a layer cannot be removed, shown on the disabled entry.
    pub layer_last_one: &'static str,
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
    /// Shown for a crossing that ends in triangles.
    pub convert_fixed_topology: &'static str,
    pub convert_not_undoable: &'static str,
    pub convert_cells: &'static str,
    pub convert_run: &'static str,
    pub convert_none_here: &'static str,
    // The boolean panel.
    /// The three operations, in `BooleanOp::ALL` order.
    pub boolean_op_names: [&'static str; clayspace_model::BooleanOp::ALL.len()],
    /// Opening the panel, and the window's own title.
    pub action_boolean: &'static str,
    pub label_boolean_op: &'static str,
    /// Which subtool is being cut, and which is doing the cutting.
    ///
    /// Two labels rather than "first" and "second", because subtraction is not
    /// symmetric and naming the roles is the whole of what the sculptor is
    /// choosing between.
    pub label_boolean_base: &'static str,
    pub label_boolean_tool: &'static str,
    /// The word between the two names in the sentence a subtraction reads as.
    pub boolean_minus: &'static str,
    /// That the result is resolved rather than live, said before it runs.
    pub boolean_resolved: &'static str,
    /// What becomes of the operands, and why that is what makes it
    /// recoverable.
    pub boolean_keeps_operands: &'static str,
    pub action_boolean_consume: &'static str,
    pub hint_boolean_consume: &'static str,
    pub action_boolean_run: &'static str,
    /// Shown while the panel has no pair to run.
    pub boolean_pick_two: &'static str,
    /// What an operand picker shows before one is chosen.
    ///
    /// Its own string rather than [`Strings::boolean_pick_two`]: a picker
    /// reading "choose two subtools" appears twice, and each of the two says
    /// something about the pair rather than about itself.
    pub boolean_pick_one: &'static str,
    /// Prefix for the active layer's representation in the viewport bar.
    pub representation_label: &'static str,
    /// Said when a layer change forced a different tool.
    pub tool_substituted: &'static str,
    /// Said when a press in the viewport meets something that carries no
    /// manipulator — a stroke, an applied curve, a rig's skin.
    pub item_not_transformable: &'static str,
    pub action_undo: &'static str,
    pub action_redo: &'static str,
    pub action_frame_all: &'static str,
    /// A mesh layer's own edges, drawn over it.
    pub action_polyframe: &'static str,
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
    combine_names: [
        "Unir",
        "Subtrair",
        "Interseção",
        "Pintar",
        "Sulco",
        "Lingueta",
        "Tubo",
        "Gravar",
        "Relevar",
        "Embutir",
        "Casca",
        "Substituir",
        "Relevo",
        "Incisar",
    ],
    blend_names: ["Dura", "Quadrática", "Cúbica", "Circular", "Chanfro"],
    extrude_side_names: ["Para fora", "Para dentro", "Centrado"],
    voxel_display_names: ["Voxels", "Suave"],
    gizmo_mode_names: ["Mover", "Girar", "Escalar"],
    tool_names: [
        "Padrão",
        "Inflar",
        "Suavizar",
        "Mover",
        "Pinçar",
        "Raspar",
        "Planar",
        "Preencher",
        "Camada",
        "Máscara",
        "Puxar",
        "Polir",
        "Relaxar",
        "Nudge",
        "Trim",
        "Argila",
        "Vinco",
        "Pintar",
        "Borrar",
        "Apagar",
    ],
    shape_names: [
        "Caixa",
        "Esfera",
        "Cilindro",
        "Cone",
        "Toro",
        "Cápsula",
        "Elipsoide",
        "Pirâmide",
        "Caixa arredondada",
        "Moldura",
        "Cilindro arredondado",
        "Prisma hexagonal",
        "Prisma triangular",
        "Octaedro",
    ],
    shape_parameter_names: [
        "Largura",
        "Altura",
        "Profundidade",
        "Raio",
        "Altura",
        "Profundidade",
        "Raio da base",
        "Raio do topo",
        "Raio maior",
        "Raio menor",
        "Raio em X",
        "Raio em Y",
        "Raio em Z",
        "Altura",
        "Raio do canto",
        "Espessura",
        "Raio da borda",
        "Tamanho",
    ],
    insert_as_names: ["Novo subtool", "No subtool ativo"],
    representation_names: ["Campo (SDF)", "Voxels", "Malha"],
    action_shapes: "Formas",
    label_shape: "Forma",
    label_insert_as: "Inserir como",
    action_insert: "Inserir",
    action_insert_mesh: "Importar malha como subtool…",
    action_copy_subtool: "Copiar subtool",
    hint_copy_subtool: "A cópia é independente: esculpir a cópia não muda o original.",
    label_new_layer_kind: "Tipo",
    action_remove_object: "Remover objeto",
    label_placed_objects: "Objetos",
    label_no_placed_objects: "nenhum objeto colocado",
    label_object_scale: "Tamanho",
    hint_shapes: "Coloque uma forma e mire-a com o manipulador.",
    hint_uniform_scale: "A escala é uniforme. Use a gaiola para esticar em um eixo só.",
    label_shapes_sdf_only: "Um objeto vive na lista ordenada de uma camada SDF.",
    locale: Locale::PtBr,
    menu_file: "Arquivo",
    menu_edit: "Editar",
    menu_view: "Vista",
    menu_language: "Idioma",
    menu_sculpt: "Escultura",
    menu_brushes: "Pincéis",
    menu_dynamics: "Dinâmica",
    menu_masks: "Máscaras",
    action_paint_mask: "Pintar máscara",
    section_curve: "CURVA",
    action_curve: "Tubo por curva",
    action_curve_apply: "Aplicar",
    label_curve_radius: "Espessura",
    label_curve_join: "Junção",
    label_curve_profile: "Perfil",
    hint_curve: "Clique para pôr um ponto · arraste um ponto para movê-lo · Del remove",
    label_voxel_display: "Exibir voxels como",
    label_voxel_blur: "Suavização",
    hint_voxel_blur: "Acima de zero apaga voxels isolados e detalhes finos",
    section_lattice: "GAIOLA",
    action_cage: "Gaiola de deformação",
    action_bend: "Deformar",
    label_cage_divisions: "Pontos por eixo",
    status_cage_needs_a_field:
        "Uma camada de voxels não aceita uma gaiola. Converta-a para SDF ou malha primeiro.",
    hint_cage: "Arraste um ponto · Shift+clique soma à seleção · Deformar aplica",
    cage_switch_title: "Gaiola em aberto",
    cage_switch_question: "A gaiola foi ajustada a esta subferramenta e não \
                           acompanha a troca. Deformar antes de trocar, ou descartá-la?",
    cage_switch_apply: "Deformar e trocar",
    cage_switch_drop: "Descartar e trocar",
    cage_switch_stay: "Ficar aqui",
    hint_gizmo_rotate: "O anel externo gira no plano da tela · Ctrl trava em 15°",
    hint_gizmo_needs_two:
        "Girar e Escalar agem em torno do meio da seleção · escolha dois pontos ou mais",
    section_layer_transform: "TRANSFORMAR CAMADA",
    hint_layer_transform:
        "Move, gira e escala a camada inteira · clique de novo para guardar o manipulador",
    section_mask: "MÁSCARA",
    label_mask_steps: "Passos",
    label_mask_cells: "Células congeladas",
    status_extrude_needs_a_field:
        "Uma camada de malha não tem campo para extrudar. Converta-a para SDF primeiro.",
    label_extrude_thickness: "Espessura",
    label_extrude_round: "Arredondar",
    label_extrude_smooth: "Suavizar borda",
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
    action_deform: "Deformar…",
    label_axis: "Eixo",
    label_span: "Extensão",
    label_scale_start: "Escala inicial",
    label_scale_end: "Escala final",
    label_angle: "Ângulo",
    deform_mesh_only: "os deformadores de forma inteira agem sobre uma camada de malha",
    action_references: "Imagens de referência…",
    label_reference_opacity: "Opacidade",
    label_reference_size: "Altura",
    label_reference_across: "Deslocamento horizontal",
    label_reference_up: "Deslocamento vertical",
    label_reference_depth: "Profundidade",
    action_load_reference: "Carregar imagem…",
    action_clear_reference: "Remover imagem",
    reference_none: "nenhuma imagem neste plano",
    label_surface_opacity: "Opacidade do modelo",
    hint_surface_opacity: "o modelo fica translúcido para a referência aparecer através dele",
    sculpt_begin: "Gravar passe",
    sculpt_end: "Encerrar passe",
    sculpt_recording: "gravando",
    sculpt_cells: "células",
    sculpt_remove: "remover o passe",
    sculpt_merge_down: "fundir com o passe abaixo",
    sculpt_move_up: "subir na pilha",
    sculpt_move_down: "descer na pilha",
    sculpt_worth_merging: "a pilha de passes está grande; fundir passes reduz o custo pela metade",
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
    action_rename_layer: "Renomear",
    action_remove_layer: "Excluir",
    action_solo_layer: "Mostrar só esta",
    action_release_solo: "Mostrar todas",
    layer_last_one: "um documento guarda ao menos uma camada",
    label_spheres: "Esferas",
    label_skin: "Pele",
    label_mirror_new: "Espelhar novas",

    shelf_no_tools: "Nenhuma ferramenta para esta representação ainda",
    representation_label: "Representação",
    tool_substituted: "ferramenta trocada: esta camada não tem essa",
    item_not_transformable: "um traço, uma curva aplicada ou a pele de um esqueleto não se transforma: só uma forma colocada tem manipulador",
    action_convert: "Converter",
    label_convert_to: "Converter para",
    label_cell_size: "Tamanho da célula",
    label_convert_costs: "O que esta travessia custa",
    convert_surface_moves: "a superfície move-se até",
    convert_features_vanish: "detalhes mais finos que isto desaparecem",
    convert_sharp_edges_lost: "arestas vivas viram degraus",
    convert_history_lost: "o histórico paramétrico não volta",
    convert_fixed_topology: "a topologia é a da grade de amostragem; nada aqui a refaz",
    convert_not_undoable: "não se desfaz: remova a camada criada para voltar atrás",
    convert_cells: "células",
    convert_run: "Converter",
    convert_none_here: "esta camada não tem para onde converter",
    boolean_op_names: ["União", "Subtração", "Interseção"],
    action_boolean: "Booleana entre subtools",
    label_boolean_op: "Operação",
    label_boolean_base: "Base — o subtool que é cortado",
    label_boolean_tool: "Ferramenta — o subtool que corta",
    boolean_minus: "menos",
    boolean_resolved: "o resultado é resolvido, não ao vivo: mover um operando depois não o atualiza",
    boolean_keeps_operands: "os operandos ficam na cena, ocultos, e uma desfeita traz tudo de volta",
    action_boolean_consume: "Consumir os operandos",
    hint_boolean_consume: "Os operandos são removidos em vez de ocultados. Sem eles, não há como refazer a operação.",
    action_boolean_run: "Resolver booleana",
    boolean_pick_two: "Escolha dois subtools diferentes.",
    boolean_pick_one: "Escolher subtool",
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
    action_polyframe: "Malha aparente",
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
    combine_names: [
        "Union",
        "Subtract",
        "Intersect",
        "Paint",
        "Groove",
        "Tongue",
        "Pipe",
        "Engrave",
        "Emboss",
        "Inset",
        "Shell",
        "Replace",
        "Relief",
        "Incise",
    ],
    blend_names: ["Hard", "Quadratic", "Cubic", "Circular", "Chamfer"],
    extrude_side_names: ["Outward", "Inward", "Centred"],
    voxel_display_names: ["Voxels", "Smooth"],
    gizmo_mode_names: ["Move", "Turn", "Scale"],
    tool_names: [
        "Standard",
        "Inflate",
        "Smooth",
        "Move",
        "Pinch",
        "Scrape",
        "Planar",
        "Fill",
        "Layer",
        "Mask",
        "Snake Hook",
        "Polish",
        "Relax",
        "Nudge",
        "Trim",
        "Clay",
        "Crease",
        "Paint",
        "Smear",
        "Erase",
    ],
    shape_names: [
        "Box",
        "Sphere",
        "Cylinder",
        "Cone",
        "Torus",
        "Capsule",
        "Ellipsoid",
        "Pyramid",
        "Rounded box",
        "Box frame",
        "Rounded cylinder",
        "Hex prism",
        "Tri prism",
        "Octahedron",
    ],
    shape_parameter_names: [
        "Width",
        "Height",
        "Depth",
        "Radius",
        "Height",
        "Depth",
        "Base radius",
        "Top radius",
        "Major radius",
        "Minor radius",
        "Radius in X",
        "Radius in Y",
        "Radius in Z",
        "Height",
        "Corner radius",
        "Thickness",
        "Rim radius",
        "Size",
    ],
    insert_as_names: ["New subtool", "Into the active subtool"],
    representation_names: ["Field (SDF)", "Voxels", "Mesh"],
    action_shapes: "Shapes",
    label_shape: "Shape",
    label_insert_as: "Insert as",
    action_insert: "Insert",
    action_insert_mesh: "Import mesh as a subtool…",
    action_copy_subtool: "Copy subtool",
    hint_copy_subtool: "A copy is independent: sculpting the copy does not change the original.",
    label_new_layer_kind: "Kind",
    action_remove_object: "Remove object",
    label_placed_objects: "Objects",
    label_no_placed_objects: "nothing placed yet",
    label_object_scale: "Size",
    hint_shapes: "Place a shape, then aim it with the manipulator.",
    hint_uniform_scale: "Scale is uniform. Use the cage to stretch along one axis.",
    label_shapes_sdf_only: "An object lives in an SDF layer's ordered list.",
    locale: Locale::EnUs,
    menu_file: "File",
    menu_edit: "Edit",
    menu_view: "View",
    menu_language: "Language",
    menu_sculpt: "Sculpt",
    menu_brushes: "Brushes",
    menu_dynamics: "Dynamics",
    menu_masks: "Masks",
    action_paint_mask: "Paint mask",
    section_curve: "CURVE",
    action_curve: "Tube along a curve",
    action_curve_apply: "Apply",
    label_curve_radius: "Thickness",
    label_curve_join: "Join",
    label_curve_profile: "Profile",
    hint_curve: "Click to place a point · drag one to move it · Del removes",
    label_voxel_display: "Draw voxels as",
    label_voxel_blur: "Blur",
    hint_voxel_blur: "Above zero deletes isolated voxels and thin detail",
    section_lattice: "LATTICE",
    action_cage: "Deformation cage",
    action_bend: "Deform",
    label_cage_divisions: "Points per axis",
    status_cage_needs_a_field: "A voxel layer takes no cage. Cross it to SDF or mesh first.",
    hint_cage: "Drag a point · Shift-click adds to the selection · Deform applies",
    cage_switch_title: "A cage is still up",
    cage_switch_question: "The cage was fitted to this subtool and does not follow \
                           the switch. Deform first, or throw it away?",
    cage_switch_apply: "Deform and switch",
    cage_switch_drop: "Discard and switch",
    cage_switch_stay: "Stay here",
    hint_gizmo_rotate: "The outer ring turns in the screen plane · Ctrl snaps to 15°",
    hint_gizmo_needs_two:
        "Turn and Scale act about the middle of the selection · pick two points or more",
    section_layer_transform: "TRANSFORM LAYER",
    hint_layer_transform:
        "Moves, turns and scales the whole layer · click again to put the manipulator away",
    section_mask: "MASK",
    label_mask_steps: "Steps",
    label_mask_cells: "Frozen cells",
    status_extrude_needs_a_field:
        "A mesh layer has no field to extrude from. Cross it to SDF first.",
    label_extrude_thickness: "Thickness",
    label_extrude_round: "Round",
    label_extrude_smooth: "Smooth edge",
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
    action_deform: "Deform…",
    label_axis: "Axis",
    label_span: "Span",
    label_scale_start: "Start scale",
    label_scale_end: "End scale",
    label_angle: "Angle",
    deform_mesh_only: "whole-form deformers act on a mesh layer",
    action_references: "Reference images…",
    label_reference_opacity: "Opacity",
    label_reference_size: "Height",
    label_reference_across: "Horizontal offset",
    label_reference_up: "Vertical offset",
    label_reference_depth: "Depth",
    action_load_reference: "Load image…",
    action_clear_reference: "Remove image",
    reference_none: "no image on this plane",
    label_surface_opacity: "Model opacity",
    hint_surface_opacity: "the model turns translucent so the reference shows through it",
    sculpt_begin: "Record pass",
    sculpt_end: "End pass",
    sculpt_recording: "recording",
    sculpt_cells: "cells",
    sculpt_remove: "remove the pass",
    sculpt_merge_down: "merge into the pass below",
    sculpt_move_up: "move up the stack",
    sculpt_move_down: "move down the stack",
    sculpt_worth_merging: "the pass stack is large; merging passes halves the cost",
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
    action_rename_layer: "Rename",
    action_remove_layer: "Delete",
    action_solo_layer: "Show only this",
    action_release_solo: "Show all",
    layer_last_one: "a document keeps at least one layer",
    label_spheres: "Spheres",
    label_skin: "Skin",
    label_mirror_new: "Mirror new",

    shelf_no_tools: "No tools for this representation yet",
    representation_label: "Representation",
    tool_substituted: "tool changed: this layer has no verb for that one",
    item_not_transformable: "a stroke, an applied curve or a rig's skin cannot be transformed: only a placed shape carries a manipulator",
    action_convert: "Convert",
    label_convert_to: "Convert to",
    label_cell_size: "Cell size",
    label_convert_costs: "What this crossing costs",
    convert_surface_moves: "the surface moves by up to",
    convert_features_vanish: "features thinner than this vanish",
    convert_sharp_edges_lost: "sharp edges become a staircase",
    convert_history_lost: "the parametric history does not come back",
    convert_fixed_topology: "the topology is the sampling lattice's; nothing here re-flows it",
    convert_not_undoable: "not undoable: remove the layer it adds to take it back",
    convert_cells: "cells",
    convert_run: "Convert",
    convert_none_here: "this layer has nowhere to convert to",
    boolean_op_names: ["Union", "Subtraction", "Intersection"],
    action_boolean: "Boolean between subtools",
    label_boolean_op: "Operation",
    label_boolean_base: "Base — the subtool being cut",
    label_boolean_tool: "Tool — the subtool doing the cutting",
    boolean_minus: "minus",
    boolean_resolved: "the result is resolved rather than live: moving an operand afterwards will not update it",
    boolean_keeps_operands: "the operands stay in the scene, hidden, and one undo brings the whole thing back",
    action_boolean_consume: "Consume the operands",
    hint_boolean_consume: "The operands are removed rather than hidden. Without them there is no way to run it again.",
    action_boolean_run: "Resolve boolean",
    boolean_pick_two: "Choose two different subtools.",
    boolean_pick_one: "Choose a subtool",
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
    action_polyframe: "Polyframe",
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
    combine_names: [
        "Unir",
        "Restar",
        "Intersecar",
        "Pintar",
        "Ranura",
        "Lengüeta",
        "Tubo",
        "Grabar",
        "Realzar",
        "Embutir",
        "Cáscara",
        "Reemplazar",
        "Relieve",
        "Incidir",
    ],
    blend_names: ["Dura", "Cuadrática", "Cúbica", "Circular", "Chaflán"],
    extrude_side_names: ["Hacia fuera", "Hacia dentro", "Centrado"],
    voxel_display_names: ["Vóxeles", "Suave"],
    gizmo_mode_names: ["Mover", "Girar", "Escalar"],
    tool_names: [
        "Estándar",
        "Inflar",
        "Suavizar",
        "Mover",
        "Pellizcar",
        "Raspar",
        "Aplanar",
        "Rellenar",
        "Capa",
        "Máscara",
        "Gancho",
        "Pulir",
        "Relajar",
        "Empujar",
        "Recortar",
        "Arcilla",
        "Pliegue",
        "Pintar",
        "Difuminar",
        "Borrar",
    ],
    shape_names: [
        "Caja",
        "Esfera",
        "Cilindro",
        "Cono",
        "Toro",
        "Cápsula",
        "Elipsoide",
        "Pirámide",
        "Caja redondeada",
        "Marco",
        "Cilindro redondeado",
        "Prisma hexagonal",
        "Prisma triangular",
        "Octaedro",
    ],
    shape_parameter_names: [
        "Ancho",
        "Alto",
        "Profundidad",
        "Radio",
        "Alto",
        "Profundidad",
        "Radio de la base",
        "Radio superior",
        "Radio mayor",
        "Radio menor",
        "Radio en X",
        "Radio en Y",
        "Radio en Z",
        "Alto",
        "Radio de esquina",
        "Espesor",
        "Radio del borde",
        "Tamaño",
    ],
    insert_as_names: ["Nuevo subtool", "En el subtool activo"],
    representation_names: ["Campo (SDF)", "Vóxeles", "Malla"],
    action_shapes: "Formas",
    label_shape: "Forma",
    label_insert_as: "Insertar como",
    action_insert: "Insertar",
    action_insert_mesh: "Importar malla como subtool…",
    action_copy_subtool: "Copiar subtool",
    hint_copy_subtool: "La copia es independiente: esculpir la copia no cambia el original.",
    label_new_layer_kind: "Tipo",
    action_remove_object: "Quitar objeto",
    label_placed_objects: "Objetos",
    label_no_placed_objects: "nada colocado aún",
    label_object_scale: "Tamaño",
    hint_shapes: "Coloca una forma y apúntala con el manipulador.",
    hint_uniform_scale: "La escala es uniforme. Usa la jaula para estirar en un solo eje.",
    label_shapes_sdf_only: "Un objeto vive en la lista ordenada de una capa SDF.",
    locale: Locale::Es419,
    menu_file: "Archivo",
    menu_edit: "Editar",
    menu_view: "Vista",
    menu_language: "Idioma",
    menu_sculpt: "Escultura",
    menu_brushes: "Pinceles",
    menu_dynamics: "Dinámica",
    menu_masks: "Máscaras",
    action_paint_mask: "Pintar máscara",
    section_curve: "CURVA",
    action_curve: "Tubo por curva",
    action_curve_apply: "Aplicar",
    label_curve_radius: "Grosor",
    label_curve_join: "Unión",
    label_curve_profile: "Perfil",
    hint_curve: "Clic para poner un punto · arrastra uno para moverlo · Supr elimina",
    label_voxel_display: "Mostrar vóxeles como",
    label_voxel_blur: "Suavizado",
    hint_voxel_blur: "Por encima de cero borra vóxeles aislados y detalles finos",
    section_lattice: "GAIOLA",
    action_cage: "Gaiola de deformação",
    action_bend: "Deformar",
    label_cage_divisions: "Pontos por eixo",
    status_cage_needs_a_field:
        "Uma camada de voxels não aceita uma gaiola. Converta-a para SDF ou malha primeiro.",
    hint_cage: "Arraste um ponto · Shift+clique soma à seleção · Deformar aplica",
    cage_switch_title: "Hay una jaula abierta",
    cage_switch_question: "La jaula se ajustó a esta subherramienta y no acompaña \
                           el cambio. ¿Deformar antes de cambiar, o descartarla?",
    cage_switch_apply: "Deformar y cambiar",
    cage_switch_drop: "Descartar y cambiar",
    cage_switch_stay: "Quedarme aquí",
    hint_gizmo_rotate: "El anillo exterior gira en el plano de la pantalla · Ctrl fija a 15°",
    hint_gizmo_needs_two:
        "Girar y Escalar actúan en torno al medio de la selección · elige dos puntos o más",
    section_layer_transform: "TRANSFORMAR CAPA",
    hint_layer_transform:
        "Mueve, gira y escala la capa entera · pulse de nuevo para guardar el manipulador",
    section_mask: "MÁSCARA",
    label_mask_steps: "Passos",
    label_mask_cells: "Células congeladas",
    status_extrude_needs_a_field:
        "Uma camada de malha não tem campo para extrudir. Converta-a para SDF primeiro.",
    label_extrude_thickness: "Espessura",
    label_extrude_round: "Arredondar",
    label_extrude_smooth: "Suavizar borda",
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
    action_deform: "Deformar…",
    label_axis: "Eje",
    label_span: "Extensión",
    label_scale_start: "Escala inicial",
    label_scale_end: "Escala final",
    label_angle: "Ángulo",
    deform_mesh_only: "los deformadores de forma completa actúan sobre una capa de malla",
    action_references: "Imágenes de referencia…",
    label_reference_opacity: "Opacidad",
    label_reference_size: "Altura",
    label_reference_across: "Desplazamiento horizontal",
    label_reference_up: "Desplazamiento vertical",
    label_reference_depth: "Profundidad",
    action_load_reference: "Cargar imagen…",
    action_clear_reference: "Quitar imagen",
    reference_none: "ninguna imagen en este plano",
    label_surface_opacity: "Opacidad del modelo",
    hint_surface_opacity: "el modelo se vuelve translúcido para que la referencia se vea a través",
    sculpt_begin: "Grabar pase",
    sculpt_end: "Terminar pase",
    sculpt_recording: "grabando",
    sculpt_cells: "celdas",
    sculpt_remove: "quitar el pase",
    sculpt_merge_down: "fundir con el pase de abajo",
    sculpt_move_up: "subir en la pila",
    sculpt_move_down: "bajar en la pila",
    sculpt_worth_merging: "la pila de pases es grande; fundir pases reduce el coste a la mitad",
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
    action_rename_layer: "Renombrar",
    action_remove_layer: "Eliminar",
    action_solo_layer: "Mostrar solo esta",
    action_release_solo: "Mostrar todas",
    layer_last_one: "un documento guarda al menos una capa",
    label_spheres: "Esferas",
    label_skin: "Piel",
    label_mirror_new: "Reflejar nuevas",

    shelf_no_tools: "Todavía no hay herramientas para esta representación",
    representation_label: "Representación",
    tool_substituted: "herramienta cambiada: esta capa no tiene ese verbo",
    item_not_transformable: "un trazo, una curva aplicada o la piel de un esqueleto no se transforma: solo una forma colocada tiene manipulador",
    action_convert: "Convertir",
    label_convert_to: "Convertir a",
    label_cell_size: "Tamaño de celda",
    label_convert_costs: "Lo que cuesta este cruce",
    convert_surface_moves: "la superficie se mueve hasta",
    convert_features_vanish: "los detalles más finos que esto desaparecen",
    convert_sharp_edges_lost: "los bordes vivos se vuelven escalones",
    convert_history_lost: "el historial paramétrico no vuelve",
    convert_fixed_topology: "la topología es la de la retícula de muestreo; nada aquí la rehace",
    convert_not_undoable: "no se deshace: quite la capa creada para volver atrás",
    convert_cells: "celdas",
    convert_run: "Convertir",
    convert_none_here: "esta capa no tiene a dónde convertirse",
    boolean_op_names: ["Unión", "Sustracción", "Intersección"],
    action_boolean: "Booleana entre subtools",
    label_boolean_op: "Operación",
    label_boolean_base: "Base — el subtool que se corta",
    label_boolean_tool: "Herramienta — el subtool que corta",
    boolean_minus: "menos",
    boolean_resolved: "el resultado es resuelto, no en vivo: mover un operando después no lo actualiza",
    boolean_keeps_operands: "los operandos quedan en la escena, ocultos, y un deshacer lo devuelve todo",
    action_boolean_consume: "Consumir los operandos",
    hint_boolean_consume: "Los operandos se quitan en vez de ocultarse. Sin ellos no hay manera de rehacer la operación.",
    action_boolean_run: "Resolver booleana",
    boolean_pick_two: "Elija dos subtools diferentes.",
    boolean_pick_one: "Elegir subtool",
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
    action_polyframe: "Malla visible",
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
    /// What a brush is called, in this language.
    ///
    /// Falls back to the domain's own name for a tool missing from
    /// `ToolKind::ALL` — a shelf entry with no word on it is worse than one in
    /// the wrong language, and the fixed-length table makes that unreachable
    /// anyway.
    pub fn tool(&self, tool: clayspace_model::ToolKind) -> &'static str {
        clayspace_model::ToolKind::ALL
            .iter()
            .position(|known| *known == tool)
            .and_then(|at| self.tool_names.get(at).copied())
            .unwrap_or_else(|| tool.label())
    }

    /// What a shape is called, in this language.
    ///
    /// Falls back to the domain's own name, as `tool` does: a picker entry
    /// with no word on it is worse than one in the wrong language, and the
    /// fixed-length table makes that unreachable anyway.
    pub fn shape(&self, shape: clayspace_model::Shape) -> &'static str {
        clayspace_model::Shape::ALL
            .iter()
            .position(|known| *known == shape)
            .and_then(|at| self.shape_names.get(at).copied())
            .unwrap_or_else(|| shape.label())
    }

    /// What one of a shape's measurements is called, in this language.
    ///
    /// Falls back to the key, which is the identifier itself — ugly, and
    /// better than an empty label. The fixed-length table and
    /// `every_parameter_key_can_be_named` make that unreachable.
    pub fn shape_parameter(&self, key: &str) -> &'static str {
        clayspace_model::PARAMETER_KEYS
            .iter()
            .position(|known| *known == key)
            .and_then(|at| self.shape_parameter_names.get(at).copied())
            .unwrap_or("?")
    }

    /// Every shape name, for a test that checks the whole vocabulary at once.
    pub fn shape_names(&self) -> &[&'static str] {
        &self.shape_names
    }

    pub fn for_locale(locale: Locale) -> &'static Strings {
        match locale {
            Locale::PtBr => &PT_BR,
            Locale::EnUs => &EN_US,
            Locale::Es419 => &ES_419,
        }
    }

    /// Every brush name, for a test that checks the whole vocabulary at once.
    pub fn tool_names(&self) -> &[&'static str] {
        &self.tool_names
    }

    /// The name for one combine operation, in this locale.
    ///
    /// By position in `Combine::ALL`, which is what makes a new operation
    /// without a name a compile error rather than a Portuguese word on an
    /// English screen.
    pub fn combine_name(&self, op: clayspace_model::Combine) -> &'static str {
        Self::at(&self.combine_names, clayspace_model::Combine::ALL, op)
    }

    pub fn blend_name(&self, blend: clayspace_model::BlendProfile) -> &'static str {
        Self::at(&self.blend_names, clayspace_model::BlendProfile::ALL, blend)
    }

    pub fn extrude_side_name(&self, side: clayspace_model::ExtrudeSide) -> &'static str {
        Self::at(
            &self.extrude_side_names,
            clayspace_model::ExtrudeSide::ALL,
            side,
        )
    }

    /// What the manipulator's mode is called.
    ///
    /// `GizmoMode::label` is the domain's own word for it and reads in
    /// Portuguese on every locale; this is the one the interface draws.
    pub fn gizmo_mode_name(&self, mode: clayspace_model::GizmoMode) -> &'static str {
        Self::at(
            &self.gizmo_mode_names,
            clayspace_model::GizmoMode::ALL,
            mode,
        )
    }

    /// Where an insertion would land, in this locale.
    ///
    /// By position in `InsertAs::ALL`, which is what makes a destination added
    /// without a name a compile error rather than a Portuguese word on an
    /// English screen.
    pub fn insert_as_name(&self, destination: clayspace_model::InsertAs) -> &'static str {
        Self::at(
            &self.insert_as_names,
            clayspace_model::InsertAs::ALL,
            destination,
        )
    }

    /// What this boolean operation is called, in this locale.
    pub fn boolean_op(&self, op: clayspace_model::BooleanOp) -> &'static str {
        Self::at(&self.boolean_op_names, clayspace_model::BooleanOp::ALL, op)
    }

    /// What a layer of this representation is called, in this locale.
    pub fn representation_name(&self, what: clayspace_model::Representation) -> &'static str {
        Self::at(
            &self.representation_names,
            clayspace_model::Representation::ALL,
            what,
        )
    }

    pub fn voxel_display_name(&self, how: clayspace_model::VoxelDisplay) -> &'static str {
        Self::at(
            &self.voxel_display_names,
            clayspace_model::VoxelDisplay::ALL,
            how,
        )
    }

    /// The name sitting at `value`'s position in `all`.
    ///
    /// Falls back to the first entry rather than panicking: a missing name is
    /// a wrong word on screen, and taking the interface down over one would be
    /// the worse failure. The fixed-length arrays are what actually prevent it.
    fn at<T: PartialEq + Copy, const N: usize>(
        names: &[&'static str; N],
        all: [T; N],
        value: T,
    ) -> &'static str {
        all.iter()
            .position(|candidate| *candidate == value)
            .and_then(|at| names.get(at).copied())
            .unwrap_or(names[0])
    }

    /// Every string, for tests that check the whole table at once.
    pub fn all(&self) -> [&'static str; 173] {
        [
            self.action_shapes,
            self.label_shape,
            self.action_insert,
            self.label_insert_as,
            self.action_insert_mesh,
            self.action_copy_subtool,
            self.hint_copy_subtool,
            self.action_boolean,
            self.label_boolean_op,
            self.label_boolean_base,
            self.label_boolean_tool,
            self.boolean_minus,
            self.boolean_resolved,
            self.boolean_keeps_operands,
            self.action_boolean_consume,
            self.hint_boolean_consume,
            self.action_boolean_run,
            self.boolean_pick_two,
            self.boolean_pick_one,
            self.label_new_layer_kind,
            self.action_remove_object,
            self.label_placed_objects,
            self.label_no_placed_objects,
            self.label_object_scale,
            self.label_shapes_sdf_only,
            self.item_not_transformable,
            self.menu_file,
            self.menu_edit,
            self.menu_view,
            self.menu_language,
            self.menu_sculpt,
            self.menu_brushes,
            self.menu_dynamics,
            self.menu_masks,
            self.section_curve,
            self.action_curve,
            self.action_curve_apply,
            self.label_curve_radius,
            self.label_curve_join,
            self.label_curve_profile,
            self.hint_curve,
            self.label_voxel_display,
            self.label_voxel_blur,
            self.hint_voxel_blur,
            self.section_lattice,
            self.action_cage,
            self.action_bend,
            self.label_cage_divisions,
            self.status_cage_needs_a_field,
            self.hint_cage,
            self.cage_switch_title,
            self.cage_switch_question,
            self.cage_switch_apply,
            self.cage_switch_drop,
            self.cage_switch_stay,
            self.hint_gizmo_rotate,
            self.hint_gizmo_needs_two,
            self.section_layer_transform,
            self.hint_layer_transform,
            self.action_paint_mask,
            self.section_mask,
            self.label_mask_steps,
            self.label_mask_cells,
            self.status_extrude_needs_a_field,
            self.label_extrude_thickness,
            self.label_extrude_round,
            self.label_extrude_smooth,
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
            self.action_deform,
            self.label_axis,
            self.label_span,
            self.label_scale_start,
            self.label_scale_end,
            self.label_angle,
            self.deform_mesh_only,
            self.action_references,
            self.label_reference_opacity,
            self.label_reference_size,
            self.label_reference_across,
            self.label_reference_up,
            self.label_reference_depth,
            self.action_load_reference,
            self.action_clear_reference,
            self.reference_none,
            self.label_surface_opacity,
            self.hint_surface_opacity,
            self.sculpt_begin,
            self.sculpt_end,
            self.sculpt_recording,
            self.sculpt_cells,
            self.sculpt_remove,
            self.sculpt_merge_down,
            self.sculpt_move_up,
            self.sculpt_move_down,
            self.sculpt_worth_merging,
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
            self.action_rename_layer,
            self.action_remove_layer,
            self.action_solo_layer,
            self.action_release_solo,
            self.layer_last_one,
            self.label_spheres,
            self.label_skin,
            self.label_mirror_new,
            self.action_undo,
            self.action_redo,
            self.action_frame_all,
            self.action_polyframe,
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

    /// The destinations and the representations are two more vocabularies held
    /// by position, and the failure they can have is the shape shelf's: a
    /// control drawn from `InsertAs`'s or `Representation`'s own word would
    /// read in one language whatever the menu says. `Representation::label` is
    /// the engine's — "SDF", "voxel" — and was never a word for a sculptor.
    #[test]
    fn the_insert_control_speaks_every_language() {
        for locale in Locale::ALL {
            let strings = Strings::for_locale(locale);
            for destination in clayspace_model::InsertAs::ALL {
                assert!(
                    !strings.insert_as_name(destination).is_empty(),
                    "{destination:?} has no name in {}",
                    locale.label()
                );
            }
            for representation in clayspace_model::Representation::ALL {
                assert!(
                    !strings.representation_name(representation).is_empty(),
                    "{representation:?} has no name in {}",
                    locale.label()
                );
            }
        }

        let english = Strings::for_locale(Locale::EnUs);
        let portuguese = Strings::for_locale(Locale::PtBr);
        assert!(
            clayspace_model::InsertAs::ALL.iter().all(|destination| {
                english.insert_as_name(*destination) != portuguese.insert_as_name(*destination)
            }),
            "a destination reads the same in both, so one table was copied"
        );
    }

    /// A fourth vocabulary held by position, and the one whose failure would
    /// be quietest: three operations named from `BooleanOp::label` would read
    /// in Portuguese whatever the rest of the panel says.
    #[test]
    fn the_boolean_operations_speak_every_language() {
        for locale in Locale::ALL {
            let strings = Strings::for_locale(locale);
            let mut seen = std::collections::BTreeSet::new();
            for op in clayspace_model::BooleanOp::ALL {
                let name = strings.boolean_op(op);
                assert!(!name.is_empty(), "{op:?} has no name in {}", locale.label());
                assert!(
                    seen.insert(name),
                    "{} names two operations {name:?}",
                    locale.label()
                );
            }
        }
        let english = Strings::for_locale(Locale::EnUs);
        let portuguese = Strings::for_locale(Locale::PtBr);
        assert!(
            clayspace_model::BooleanOp::ALL
                .iter()
                .all(|op| english.boolean_op(*op) != portuguese.boolean_op(*op)),
            "an operation reads the same in both, so one table was copied"
        );
    }

    #[test]
    fn every_shape_has_a_name_in_every_language() {
        // The same failure the brush shelf had: a picker that showed
        // `Shape::label()` would be the domain's own Portuguese whatever the
        // interface language was.
        for locale in Locale::ALL {
            let strings = Strings::for_locale(locale);
            for shape in clayspace_model::Shape::ALL {
                let name = strings.shape(shape);
                assert!(
                    !name.is_empty(),
                    "{:?} has no name in {}",
                    shape,
                    locale.label()
                );
            }
        }
    }

    #[test]
    fn the_shape_names_are_translated_rather_than_copied() {
        let english = Strings::for_locale(Locale::EnUs).shape_names();
        let portuguese = Strings::for_locale(Locale::PtBr).shape_names();
        let differing = english
            .iter()
            .zip(portuguese.iter())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            differing >= 8,
            "only {differing} of {} shape names differ",
            english.len()
        );
    }

    #[test]
    fn every_brush_has_a_name_in_every_language() {
        // The brush shelf showed `ToolKind::label()` — the domain's own
        // Portuguese — on all three representations whatever the interface
        // language was, so choosing English translated the chrome and left
        // Padrão, Inflar and Relevo on the shelf.
        use clayspace_model::ToolKind;
        for locale in Locale::ALL {
            let strings = Strings::for_locale(locale);
            for tool in ToolKind::ALL {
                let name = strings.tool(tool);
                assert!(
                    !name.is_empty(),
                    "{:?} has no name in {}",
                    tool,
                    locale.label()
                );
            }
            // Distinct, because two brushes sharing a name on the shelf is a
            // shelf a sculptor cannot use.
            let mut seen = std::collections::BTreeSet::new();
            for tool in ToolKind::ALL {
                assert!(
                    seen.insert(strings.tool(tool)),
                    "{} names two brushes {:?}",
                    locale.label(),
                    strings.tool(tool)
                );
            }
        }
    }

    #[test]
    fn the_brush_names_are_translated_rather_than_copied() {
        // Portuguese and Spanish are close enough that a copied vocabulary
        // would pass unnoticed, and English is where a missed one shows.
        use clayspace_model::ToolKind;
        let translated = |from: Locale, to: Locale| {
            ToolKind::ALL
                .iter()
                .filter(|tool| {
                    Strings::for_locale(from).tool(**tool) != Strings::for_locale(to).tool(**tool)
                })
                .count()
        };
        assert!(
            translated(Locale::PtBr, Locale::EnUs) >= 15,
            "only {} of twenty brushes differ between Portuguese and English",
            translated(Locale::PtBr, Locale::EnUs)
        );
        assert!(
            translated(Locale::PtBr, Locale::Es419) >= 8,
            "only {} of twenty brushes differ between Portuguese and Spanish",
            translated(Locale::PtBr, Locale::Es419)
        );
        // The Portuguese table is the domain's own vocabulary, which is what
        // makes `ToolKind::label` safe to keep using off the interface.
        for tool in ToolKind::ALL {
            assert_eq!(
                Strings::for_locale(Locale::PtBr).tool(tool),
                tool.label(),
                "the Portuguese shelf disagrees with the domain about {tool:?}"
            );
        }
    }

    #[test]
    fn a_false_friend_is_not_carried_across() {
        // Portuguese `Borrar` is smear and Spanish `Borrar` is erase. Carried
        // straight across, the Spanish shelf would name the smudge brush
        // "erase" and leave the erase brush with the smudge's name — two
        // brushes, both wrong, and the mistake reads as correct to anyone
        // checking one language at a time.
        use clayspace_model::ToolKind;
        let es = Strings::for_locale(Locale::Es419);
        assert_eq!(es.tool(ToolKind::Apagar), "Borrar");
        assert_eq!(es.tool(ToolKind::Borrar), "Difuminar");
        assert_eq!(
            Strings::for_locale(Locale::PtBr).tool(ToolKind::Borrar),
            "Borrar"
        );
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
