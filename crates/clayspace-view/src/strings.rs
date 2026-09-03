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
    /// What each brush does, in one sentence, in the order of
    /// [`ToolKind::ALL`]. Shown under the name when a swatch is hovered.
    pub tool_hints: [&'static str; clayspace_model::ToolKind::ALL.len()],
    /// What differs about a tool on one representation, where anything does.
    ///
    /// One per `ToolNote`, in that enum's own order, so a note added to the
    /// domain stops this compiling until it has been worded.
    pub tool_notes: [&'static str; clayspace_model::ToolNote::ALL.len()],
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
    /// How the mask brush is used, in `MaskGesture::ALL` order.
    pub mask_gesture_names: [&'static str; clayspace_model::MaskGesture::ALL.len()],
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
    /// What each representation *is*, in one short phrase, for the card that
    /// stands for it in the representation bar.
    ///
    /// The name alone says which of three; this says what that means, which is
    /// what a sculptor meeting the vocabulary for the first time needs. Kept
    /// to a phrase — the bar is one row tall and a sentence would not fit.
    pub representation_sentences: [&'static str; clayspace_model::Representation::ALL.len()],
    /// The representation bar's own heading.
    pub section_representation: &'static str,
    /// What a card says on hover: this is what the active layer holds, or
    /// this is what crossing to it would cost a look at.
    pub hint_representation_active: &'static str,
    pub hint_representation_other: &'static str,
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
    /// The placed objects, as the section under the layers is headed.
    pub section_objects: &'static str,
    /// The four views, in `ViewPresetKind::ALL` order. They were drawn from
    /// the domain's `label()`, so an English screen read Perspectiva,
    /// Frontal, Lateral, Superior under its viewport.
    pub view_preset_names: [&'static str; clayspace_model::ViewPresetKind::ALL.len()],
    /// The brush edge profiles, in `Falloff::ALL` order.
    pub falloff_names: [&'static str; clayspace_model::Falloff::ALL.len()],
    /// The reference planes, in `RefPlane::ALL` order.
    pub ref_plane_names: [&'static str; clayspace_model::RefPlane::ALL.len()],
    /// How a curve's points join, in `CurveJoin::ALL` order.
    pub curve_join_names: [&'static str; clayspace_model::CurveJoin::ALL.len()],
    /// A curve's cross-section, in `CurveProfile::ALL` order.
    pub curve_profile_names: [&'static str; clayspace_model::CurveProfile::ALL.len()],
    /// The mask operations. Named fields rather than an array, because
    /// `MaskOp` carries an amount and has no `ALL` to index by.
    pub mask_op_invert: &'static str,
    pub mask_op_clear: &'static str,
    pub mask_op_expand: &'static str,
    pub mask_op_contract: &'static str,
    pub mask_op_smooth: &'static str,
    pub mask_op_complement: &'static str,
    /// What the geometry panel says when the counts are not the whole story.
    pub detail_reduced: &'static str,
    pub detail_pending: &'static str,
    pub label_no_placed_objects: &'static str,
    pub label_object_scale: &'static str,
    pub hint_shapes: &'static str,
    pub hint_uniform_scale: &'static str,
    pub label_shapes_sdf_only: &'static str,
    /// The two deformations, in `DeformVerb::ALL` order.
    pub deform_verb_names: [&'static str; clayspace_model::DeformVerb::ALL.len()],
    /// The manipulator, as the row of its modes is headed.
    pub label_manipulator: &'static str,
    /// What clicking the material preview does.
    pub hint_material: &'static str,
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
    /// The heading over the two ways the mask brush can be used.
    pub label_mask_gesture: &'static str,
    /// What a drawn gesture does, and what the modifier does to it.
    pub hint_mask_outline: &'static str,
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
    /// What acts on the whole form, at the head of the options bar.
    pub label_transform: &'static str,
    /// What the whole-layer manipulator does, and how to put it away.
    pub hint_layer_transform: &'static str,
    /// Why the whole-layer manipulator is refused while something smaller
    /// already has the widget.
    pub hint_transform_taken: &'static str,
    /// Why it is refused while no layer is active.
    pub hint_transform_needs_a_layer: &'static str,
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
    /// The docked shapes and boolean sections of the right region.
    pub section_shapes: &'static str,
    pub section_boolean: &'static str,
    pub section_geometry: &'static str,
    /// The contextual section's heading, one per representation.
    ///
    /// Distinct from `section_geometry` and from each other. The voxel display
    /// controls used to stand under `section_geometry` beside the polygon
    /// counts, which are also under `section_geometry` — two sections with one
    /// word between them, sharing the fold that word is keyed by, so putting
    /// one away put the other away too.
    pub section_field: &'static str,
    pub section_voxels: &'static str,
    pub section_mesh: &'static str,
    /// How many items the active field's edit list holds.
    pub label_field_items: &'static str,
    /// What a grid is made of: how coarse its cells are, and how many hold
    /// anything.
    pub label_voxel_cell: &'static str,
    pub label_voxel_occupied: &'static str,
    /// Whether that list has been collapsed into one.
    pub label_field_collapsed: &'static str,
    pub state_yes: &'static str,
    pub state_no: &'static str,
    /// What is true of every mesh layer, and is the reason its brushes differ.
    pub mesh_topology_fixed: &'static str,
    pub section_resolution: &'static str,
    pub section_brush_controls: &'static str,
    pub section_armature: &'static str,
    pub section_diagnostics: &'static str,
    /// The rendering section of the diagnostics report.
    pub section_rendering: &'static str,
    /// The mesh-sculpting section of the diagnostics report.
    pub section_mesh_sculpting: &'static str,

    // Labels
    /// How far a stamp is turned about its own facing — the grain.
    pub label_grain: &'static str,
    pub label_intensity: &'static str,
    pub label_size: &'static str,
    pub label_flow: &'static str,
    /// How an SDF edit meets what is under it.
    pub label_combine: &'static str,
    /// How sharply the join is made.
    pub label_blend: &'static str,
    /// The scalar stamp modulating a brush.
    pub label_alpha: &'static str,
    /// The colour a colour brush paints with.
    pub label_colour: &'static str,
    /// The row of colours chosen just before this one.
    pub label_recent_colours: &'static str,
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
    /// The row that says a field layer has steepened, and the button that
    /// collapses it.
    pub optimize_advice: &'static str,
    pub optimize_action: &'static str,
    pub optimize_busy: &'static str,
    /// Rebuilding a mesh layer's topology through a voxel field — DynaMesh.
    ///
    /// The action is named for what it does to the form rather than for the
    /// technique: a sculptor reaches for it because the surface has stopped
    /// taking detail, not because they want a voxel grid. The hint is where
    /// the technique and its price go.
    pub remesh_heading: &'static str,
    pub remesh_action: &'static str,
    pub remesh_busy: &'static str,
    pub remesh_hint: &'static str,
    pub remesh_resolution: &'static str,
    pub remesh_resolution_hint: &'static str,
    pub remesh_sharp: &'static str,
    pub remesh_sharp_hint: &'static str,
    pub remesh_remove_loose: &'static str,
    pub remesh_remove_loose_hint: &'static str,
    pub remesh_follow: &'static str,
    pub remesh_follow_hint: &'static str,
    /// What the last rebuild came to: triangles before and after, and the two
    /// things it may have done that a sculptor cannot see by looking.
    pub remesh_result: &'static str,
    pub remesh_pieces: &'static str,
    pub remesh_uvs_dropped: &'static str,
    pub remesh_not_watertight: &'static str,
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

    // Actions and states
    /// Shown where the shelf would be, on a layer whose representation this
    /// application has no verb bound for yet.
    pub shelf_no_tools: &'static str,
    /// The shelf's first filter: the brushes the active layer can actually
    /// use, which is what the shelf shows unless a sculptor asks otherwise.
    pub shelf_filter_all: &'static str,
    /// The brushes a sculptor starred, across every representation.
    pub shelf_filter_favourites: &'static str,
    /// The status area's autosave line: counting down to the next one, or
    /// saying there is nothing waiting to be written.
    pub label_autosave_in: &'static str,
    pub state_autosaved: &'static str,
    /// Starring a brush, and taking the star off it.
    pub action_favourite_add: &'static str,
    pub action_favourite_remove: &'static str,
    /// Shown where the star filter is chosen and nothing has been starred.
    pub shelf_no_favourites: &'static str,
    /// How much a frame is worth spending on, in `ViewportProfile::ALL` order.
    ///
    /// A display setting: it changes what an idle frame is drawn with and
    /// never what is drawn, so no sculpt is affected by choosing one.
    pub viewport_profile_names: [&'static str; crate::quality::ViewportProfile::ALL.len()],
    pub label_viewport_profile: &'static str,
    /// The three resizable regions, in `Panel::ALL` order, for the menu that
    /// puts them away and brings them back.
    pub panel_names: [&'static str; crate::layout::Panel::ALL.len()],
    pub action_reset_layout: &'static str,
    /// Clearing the chrome away, and bringing it back.
    pub action_focus: &'static str,
    /// The transform readout that stands beside the manipulator.
    ///
    /// Axis-and-angle rather than three Euler angles, and one scale factor
    /// rather than three, because that is what the engine's transforms take —
    /// see `SceneObject`. A readout of three rotations would be inventing two.
    pub hud_position: &'static str,
    pub hud_rotation: &'static str,
    pub hud_axis: &'static str,
    pub hud_scale: &'static str,
    /// Why a brush met while browsing another representation cannot be picked.
    pub shelf_tool_elsewhere: &'static str,
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
    /// What a crossing costs in history, and what it takes back.
    pub convert_undo_note: &'static str,
    /// The choice between adding a layer and replacing the one read.
    pub convert_in_place: &'static str,
    pub convert_in_place_hint: &'static str,
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
    /// Switching between MatCap and the studio light rig.
    pub action_shading: &'static str,
    /// The screen-space crease term.
    pub action_cavity: &'static str,
    /// Whether the studio rig's key light casts.
    pub action_shadows: &'static str,
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
    /// What a document that has never been saved is called.
    pub document_untitled: &'static str,
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
    mask_gesture_names: ["Pincel", "Laço", "Retângulo"],
    voxel_display_names: ["Voxels", "Suave"],
    gizmo_mode_names: ["Mover", "Girar", "Escalar"],
    tool_names: [
        "Padrão",
        "Inflar",
        "Suavizar",
        "Mover",
        "Mover Topológico",
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
    tool_hints: [
        "Empurra a superfície para fora ao longo da normal — o pincel do dia a dia",
        "Infla a superfície para fora; intensidade negativa a encolhe",
        "Alisa relevos e ruído numa superfície uniforme",
        "Arrasta a superfície com o traço, como puxar argila",
        "Arrasta pela superfície: o alcance é medido ao longo do material, não pelo espaço",
        "Junta a superfície em direção ao centro do traço",
        "Rebaixa os pontos altos até um plano e os alisa",
        "Aplana a superfície sem preencher os pontos baixos",
        "Preenche cavidades e vincos estreitos",
        "Levanta um degrau de altura fixa que não se acumula",
        "Pinta uma máscara; a área mascarada ignora todos os outros pincéis",
        "Puxa um tentáculo da superfície, afinando até a ponta",
        "Pole: aplana a superfície em facetas lisas",
        "Uniformiza o espaçamento dos vértices sem perder a forma",
        "Desliza a pele da superfície de lado, mantendo o interior",
        "Desenhe uma forma na tela para cortar a peça de lado a lado",
        "Acumula argila em placas achatadas, como se adiciona à mão",
        "Pinça um vinco marcado ao longo do traço",
        "Pinta cor na superfície sem movê-la",
        "Borra a cor existente ao longo do traço",
        "Remove material sob o pincel",
    ],
    tool_notes: ["Numa grelha, aplanar é dos dois lados: o material acima do plano sai e as concavidades abaixo dele enchem"],
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
    representation_sentences: [
        "Campo de distância com sinal",
        "Grade de voxels",
        "Malha de polígonos",
    ],
    section_representation: "REPRESENTAÇÃO",
    hint_representation_active: "o que a camada ativa contém",
    hint_representation_other: "esta camada não é isto — converter tem custo",
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
    section_objects: "OBJETOS",
    view_preset_names: ["Perspectiva", "Frontal", "Lateral", "Superior"],
    falloff_names: ["Dura", "Linear", "Suave", "Gaussiana"],
    ref_plane_names: ["Frontal", "Lateral", "Superior"],
    curve_join_names: ["Cantos", "Pelos pontos", "Arredondado"],
    curve_profile_names: ["Círculo", "Quadrado", "Hexágono", "Triângulo"],
    mask_op_invert: "Inverter",
    mask_op_clear: "Limpar",
    mask_op_expand: "Expandir",
    mask_op_contract: "Contrair",
    mask_op_smooth: "Suavizar máscara",
    mask_op_complement: "Complemento delimitado",
    detail_reduced: "detalhe reduzido",
    detail_pending: "ainda não gerado",
    label_no_placed_objects: "nenhum objeto colocado",
    label_object_scale: "Tamanho",
    hint_shapes: "Coloque uma forma e mire-a com o manipulador.",
    hint_uniform_scale: "A escala é uniforme. Use a gaiola para esticar em um eixo só.",
    label_shapes_sdf_only: "Um objeto vive na lista ordenada de uma camada SDF.",
    deform_verb_names: ["Afunilar", "Torcer"],
    label_manipulator: "Manipulador",
    hint_material: "Clique para trocar o material. Cada um é uma esfera iluminada; a forma é lida como ela.",
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
    label_mask_gesture: "Gesto",
    hint_mask_outline: "Desenhe em volta do que quer congelar — à mão livre ou \
arrastando um retângulo. Congela através da forma, dos dois lados. Com Ctrl, \
libera em vez de congelar.",
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
    label_transform: "Transformar",
    hint_layer_transform:
        "Transforma a camada inteira · clique de novo para guardar o manipulador",
    hint_transform_taken:
        "A gaiola, a curva ou o objeto selecionado já tem o manipulador · feche-o para transformar a camada",
    hint_transform_needs_a_layer: "Nenhuma camada ativa para transformar",
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
    section_shapes: "FORMAS",
    section_boolean: "BOOLEANA",
    section_geometry: "GEOMETRIA",
    section_field: "CAMPO",
    section_voxels: "VOXELS",
    section_mesh: "MALHA",
    label_field_items: "Itens no campo",
    label_voxel_cell: "Tamanho da célula",
    label_voxel_occupied: "Células ocupadas",
    label_field_collapsed: "Colapsado",
    state_yes: "sim",
    state_no: "não",
    mesh_topology_fixed: "Topologia fixa: os pincéis movem os vértices que existem e não criam nem removem nenhum.",
    section_resolution: "RESOLUÇÃO",
    section_brush_controls: "CONTROLES DE PINCEL",
    section_armature: "ARMADURA",
    section_diagnostics: "DIAGNÓSTICO",
    section_rendering: "RENDERIZAÇÃO",
    section_mesh_sculpting: "ESCULTURA EM MALHA",

    label_grain: "Grão",
    label_intensity: "Intensidade",
    label_size: "Tamanho",
    label_flow: "Fluxo",
    label_combine: "Operação",
    label_blend: "Junção",
    label_alpha: "Alfa",
    label_colour: "Cor",
    label_recent_colours: "Recentes",
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
    optimize_advice: "Esta camada ficou pesada de avaliar",
    optimize_action: "Otimizar",
    optimize_busy: "Otimizando…",
    remesh_heading: "Refazer a malha",
    remesh_action: "Refazer",
    remesh_busy: "Refazendo a malha…",
    remesh_hint: "reconstrói a topologia inteira: partes sobrepostas se fundem, \
                  triângulos esticados desaparecem e a densidade fica uniforme. \
                  A malha antiga não volta a não ser desfazendo",
    remesh_resolution: "Resolução",
    remesh_resolution_hint: "células ao longo da maior dimensão da forma; \
                            detalhe menor que uma célula não sobrevive",
    remesh_sharp: "Arestas vivas",
    remesh_sharp_hint: "segura quinas em vez de arredondá-las, ao custo da \
                        garantia de malha fechada — o motor marca este modo \
                        como experimental",
    remesh_remove_loose: "Remover pedaços soltos",
    remesh_remove_loose_hint: "descarta cacos pequenos demais para esta resolução",
    remesh_follow: "Seguir a forma atual",
    remesh_follow_hint: "puxa a malha nova de volta para a superfície que ela \
                         substitui, recuperando o detalhe que a amostragem arredondou",
    remesh_result: "triângulos",
    remesh_pieces: "peças",
    remesh_uvs_dropped: "as coordenadas de textura foram descartadas",
    remesh_not_watertight: "o resultado não ficou fechado",
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

    shelf_no_tools: "Nenhuma ferramenta para esta representação ainda",
    shelf_filter_all: "Disponíveis",
    shelf_filter_favourites: "★ Favoritos",
    label_autosave_in: "Salvamento automático em",
    state_autosaved: "Nada a salvar",
    action_favourite_add: "Adicionar aos favoritos",
    action_favourite_remove: "Remover dos favoritos",
    shelf_no_favourites: "Nenhum pincel favoritado ainda — use o menu de um pincel",
    viewport_profile_names: ["Desempenho", "Escultura", "Apresentação"],
    label_viewport_profile: "Qualidade da viewport",
    panel_names: ["Painel esquerdo", "Painel direito", "Prateleira"],
    action_reset_layout: "Restaurar disposição",
    action_focus: "Modo foco",
    hud_position: "Posição",
    hud_rotation: "Rotação",
    hud_axis: "Eixo",
    hud_scale: "Escala",
    shelf_tool_elsewhere: "sem verbo na camada ativa",
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
    convert_undo_note: "um desfazer tira a travessia inteira",
    convert_in_place: "Substituir a camada",
    convert_in_place_hint: "A camada lida sai e o resultado fica no lugar dela · um desfazer traz as duas de volta",
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
    action_shading: "Iluminação de estúdio",
        action_cavity: "Realce de cavidades",
        action_shadows: "Sombra do estúdio",
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
    document_untitled: "Sem título",
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
    mask_gesture_names: ["Brush", "Lasso", "Rectangle"],
    voxel_display_names: ["Voxels", "Smooth"],
    gizmo_mode_names: ["Move", "Turn", "Scale"],
    tool_names: [
        "Standard",
        "Inflate",
        "Smooth",
        "Move",
        "Topological Move",
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
    tool_hints: [
        "Pushes the surface out along its normal — the everyday brush",
        "Swells the surface outward; a negative intensity shrinks it",
        "Relaxes bumps and noise into an even surface",
        "Drags the surface with the stroke, like pulling clay",
        "Drags along the surface: the reach is measured through the material, not through space",
        "Gathers the surface toward the centre of the stroke",
        "Flattens high points down to a plane and smooths them",
        "Planes the surface flat without filling low spots",
        "Fills narrow pockets and creases",
        "Raises a step of fixed height that does not build up on itself",
        "Paints a mask; masked areas ignore every other brush",
        "Pulls a tendril out of the surface, tapering to the tip",
        "Polishes: planes the surface into smooth facets",
        "Evens out vertex spacing without losing the form",
        "Slides the surface skin sideways, leaving the interior",
        "Draw a shape on the screen to cut straight through the form",
        "Builds up clay in flat pats, the way it is added by hand",
        "Pinches a sharp crease along the stroke",
        "Paints colour onto the surface without moving it",
        "Smears existing colour along the stroke",
        "Removes material under the brush",
    ],
    tool_notes: ["On a grid, flatten is two-sided: material above the plane goes and hollows below it fill"],
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
    representation_sentences: ["Signed Distance Field", "Voxel Grid", "Polygon Mesh"],
    section_representation: "REPRESENTATION",
    hint_representation_active: "what the active layer holds",
    hint_representation_other: "this layer is not this — converting has a cost",
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
    section_objects: "OBJECTS",
    view_preset_names: ["Perspective", "Front", "Side", "Top"],
    falloff_names: ["Hard", "Linear", "Smooth", "Gaussian"],
    ref_plane_names: ["Front", "Side", "Top"],
    curve_join_names: ["Corners", "Through the points", "Rounded"],
    curve_profile_names: ["Circle", "Square", "Hexagon", "Triangle"],
    mask_op_invert: "Invert",
    mask_op_clear: "Clear",
    mask_op_expand: "Expand",
    mask_op_contract: "Contract",
    mask_op_smooth: "Smooth mask",
    mask_op_complement: "Bounded complement",
    detail_reduced: "reduced detail",
    detail_pending: "not generated yet",
    label_no_placed_objects: "nothing placed yet",
    label_object_scale: "Size",
    hint_shapes: "Place a shape, then aim it with the manipulator.",
    hint_uniform_scale: "Scale is uniform. Use the cage to stretch along one axis.",
    label_shapes_sdf_only: "An object lives in an SDF layer's ordered list.",
    deform_verb_names: ["Taper", "Twist"],
    label_manipulator: "Manipulator",
    hint_material: "Click to cycle the material. Each is a lit sphere; the form reads the way it does.",
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
    label_mask_gesture: "Gesture",
    hint_mask_outline: "Draw around what you want frozen — freehand, or drag a \
rectangle. It freezes through the form, both sides. Hold Ctrl to release \
instead.",
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
    label_transform: "Transform",
    hint_layer_transform:
        "Transforms the whole layer · click again to put the manipulator away",
    hint_transform_taken:
        "The cage, the curve or the selected object already has the manipulator · put it away to transform the layer",
    hint_transform_needs_a_layer: "No active layer to transform",
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
    section_shapes: "SHAPES",
    section_boolean: "BOOLEAN",
    section_geometry: "GEOMETRY",
    section_field: "FIELD",
    section_voxels: "VOXELS",
    section_mesh: "MESH",
    label_field_items: "Items in the field",
    label_voxel_cell: "Cell size",
    label_voxel_occupied: "Occupied cells",
    label_field_collapsed: "Collapsed",
    state_yes: "yes",
    state_no: "no",
    mesh_topology_fixed: "Fixed topology: the brushes move the vertices that are there and neither add nor remove any.",
    section_resolution: "RESOLUTION",
    section_brush_controls: "BRUSH CONTROLS",
    section_armature: "ARMATURE",
    section_diagnostics: "DIAGNOSTICS",
    section_rendering: "RENDERING",
    section_mesh_sculpting: "MESH SCULPTING",

    label_grain: "Grain",
    label_intensity: "Intensity",
    label_size: "Size",
    label_flow: "Flow",
    label_combine: "Operation",
    label_blend: "Join",
    label_alpha: "Alpha",
    label_colour: "Colour",
    label_recent_colours: "Recent",
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
    optimize_advice: "This subtool has become costly to evaluate",
    optimize_action: "Optimise",
    optimize_busy: "Optimising…",
    remesh_heading: "Rebuild the mesh",
    remesh_action: "Rebuild",
    remesh_busy: "Rebuilding the mesh…",
    remesh_hint: "rebuilds the whole topology: overlapping parts fuse, stretched \
                  triangles disappear and the density comes out even. The old \
                  mesh comes back only by undoing",
    remesh_resolution: "Resolution",
    remesh_resolution_hint: "cells across the form's longest dimension; detail \
                            finer than a cell does not survive",
    remesh_sharp: "Sharp edges",
    remesh_sharp_hint: "holds corners instead of rounding them, at the cost of \
                        the watertight guarantee — the engine marks this mode \
                        experimental",
    remesh_remove_loose: "Remove loose pieces",
    remesh_remove_loose_hint: "discards fragments too small for this resolution",
    remesh_follow: "Follow the current form",
    remesh_follow_hint: "pulls the new mesh back towards the surface it replaces, \
                         recovering detail the sampling rounded off",
    remesh_result: "triangles",
    remesh_pieces: "pieces",
    remesh_uvs_dropped: "texture coordinates were dropped",
    remesh_not_watertight: "the result did not come out closed",
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

    shelf_no_tools: "No tools for this representation yet",
    shelf_filter_all: "Available",
    shelf_filter_favourites: "★ Favourites",
    label_autosave_in: "Auto save in",
    state_autosaved: "Nothing to save",
    action_favourite_add: "Add to favourites",
    action_favourite_remove: "Remove from favourites",
    shelf_no_favourites: "No brushes starred yet — use a brush's own menu",
    viewport_profile_names: ["Performance", "Sculpt", "Presentation"],
    label_viewport_profile: "Viewport quality",
    panel_names: ["Left panel", "Right panel", "Shelf"],
    action_reset_layout: "Reset layout",
    action_focus: "Focus mode",
    hud_position: "Position",
    hud_rotation: "Rotation",
    hud_axis: "Axis",
    hud_scale: "Scale",
    shelf_tool_elsewhere: "no verb on the active layer",
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
    convert_undo_note: "one undo takes the whole crossing back",
    convert_in_place: "Replace the layer",
    convert_in_place_hint: "The layer read leaves and the result stands in its row · one undo brings both back",
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
    action_shading: "Studio lighting",
        action_cavity: "Cavity shading",
        action_shadows: "Studio shadow",
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
    document_untitled: "Untitled",
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
    mask_gesture_names: ["Pincel", "Lazo", "Rectángulo"],
    voxel_display_names: ["Vóxeles", "Suave"],
    gizmo_mode_names: ["Mover", "Girar", "Escalar"],
    tool_names: [
        "Estándar",
        "Inflar",
        "Suavizar",
        "Mover",
        "Mover Topológico",
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
    tool_hints: [
        "Empuja la superficie hacia fuera a lo largo de la normal — el pincel de cada día",
        "Infla la superficie hacia fuera; una intensidad negativa la encoge",
        "Alisa relieves y ruido en una superficie uniforme",
        "Arrastra la superficie con el trazo, como tirar de la arcilla",
        "Arrastra por la superficie: el alcance se mide a lo largo del material, no por el espacio",
        "Junta la superficie hacia el centro del trazo",
        "Rebaja los puntos altos hasta un plano y los alisa",
        "Aplana la superficie sin rellenar los puntos bajos",
        "Rellena cavidades y pliegues estrechos",
        "Levanta un escalón de altura fija que no se acumula",
        "Pinta una máscara; la zona enmascarada ignora todos los demás pinceles",
        "Tira de un tentáculo desde la superficie, afinándolo hasta la punta",
        "Pule: aplana la superficie en facetas lisas",
        "Uniformiza la separación de los vértices sin perder la forma",
        "Desliza la piel de la superficie de lado, dejando el interior",
        "Dibuja una forma en pantalla para cortar la pieza de lado a lado",
        "Acumula arcilla en placas planas, como se añade a mano",
        "Pellizca un pliegue marcado a lo largo del trazo",
        "Pinta color sobre la superficie sin moverla",
        "Difumina el color existente a lo largo del trazo",
        "Elimina material bajo el pincel",
    ],
    tool_notes: ["En una rejilla, aplanar es de dos lados: el material sobre el plano se va y los huecos bajo él se rellenan"],
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
    representation_sentences: [
        "Campo de distancia con signo",
        "Rejilla de vóxeles",
        "Malla de polígonos",
    ],
    section_representation: "REPRESENTACIÓN",
    hint_representation_active: "lo que contiene la capa activa",
    hint_representation_other: "esta capa no es esto — convertir tiene un costo",
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
    section_objects: "OBJETOS",
    view_preset_names: ["Perspectiva", "Frontal", "Lateral", "Superior"],
    falloff_names: ["Dura", "Lineal", "Suave", "Gaussiana"],
    ref_plane_names: ["Frontal", "Lateral", "Superior"],
    curve_join_names: ["Esquinas", "Por los puntos", "Redondeado"],
    curve_profile_names: ["Círculo", "Cuadrado", "Hexágono", "Triángulo"],
    mask_op_invert: "Invertir",
    mask_op_clear: "Limpiar",
    mask_op_expand: "Expandir",
    mask_op_contract: "Contraer",
    mask_op_smooth: "Suavizar máscara",
    mask_op_complement: "Complemento delimitado",
    detail_reduced: "detalle reducido",
    detail_pending: "aún no generado",
    label_no_placed_objects: "nada colocado aún",
    label_object_scale: "Tamaño",
    hint_shapes: "Coloca una forma y apúntala con el manipulador.",
    hint_uniform_scale: "La escala es uniforme. Usa la jaula para estirar en un solo eje.",
    label_shapes_sdf_only: "Un objeto vive en la lista ordenada de una capa SDF.",
    deform_verb_names: ["Estrechar", "Torcer"],
    label_manipulator: "Manipulador",
    hint_material: "Haz clic para cambiar el material. Cada uno es una esfera iluminada; la forma se lee como ella.",
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
    label_mask_gesture: "Gesto",
    hint_mask_outline: "Dibuja alrededor de lo que quieras congelar — a mano \
alzada o arrastrando un rectángulo. Congela a través de la forma, por ambos \
lados. Con Ctrl, libera en vez de congelar.",
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
    label_transform: "Transformar",
    hint_layer_transform:
        "Transforma la capa entera · pulse de nuevo para guardar el manipulador",
    hint_transform_taken:
        "La jaula, la curva o el objeto seleccionado ya tiene el manipulador · ciérrelo para transformar la capa",
    hint_transform_needs_a_layer: "Ninguna capa activa para transformar",
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
    section_shapes: "FORMAS",
    section_boolean: "BOOLEANA",
    section_geometry: "GEOMETRÍA",
    section_field: "CAMPO",
    section_voxels: "VÓXELES",
    section_mesh: "MALLA",
    label_field_items: "Elementos en el campo",
    label_voxel_cell: "Tamaño de celda",
    label_voxel_occupied: "Celdas ocupadas",
    label_field_collapsed: "Colapsado",
    state_yes: "sí",
    state_no: "no",
    mesh_topology_fixed: "Topología fija: los pinceles mueven los vértices que existen y no crean ni eliminan ninguno.",
    section_resolution: "RESOLUCIÓN",
    section_brush_controls: "CONTROLES DE PINCEL",
    // "Esqueleto", not "Armadura": armadura reads as armour outside of
    // structural engineering, and Spanish-speaking riggers learned the term
    // from Blender.
    section_armature: "ESQUELETO",
    section_diagnostics: "DIAGNÓSTICO",
    section_rendering: "RENDERIZAÇÃO",
    section_mesh_sculpting: "ESCULTURA EN MALLA",

    label_grain: "Grano",
    label_intensity: "Intensidad",
    label_size: "Tamaño",
    label_flow: "Flujo",
    label_combine: "Operación",
    label_blend: "Unión",
    label_alpha: "Alfa",
    label_colour: "Color",
    label_recent_colours: "Recientes",
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
    optimize_advice: "Esta capa se volvió costosa de evaluar",
    optimize_action: "Optimizar",
    optimize_busy: "Optimizando…",
    remesh_heading: "Rehacer la malla",
    remesh_action: "Rehacer",
    remesh_busy: "Rehaciendo la malla…",
    remesh_hint: "reconstruye toda la topología: las partes superpuestas se \
                  fusionan, los triángulos estirados desaparecen y la densidad \
                  queda uniforme. La malla anterior solo vuelve deshaciendo",
    remesh_resolution: "Resolución",
    remesh_resolution_hint: "celdas a lo largo de la dimensión mayor de la forma; \
                            el detalle menor que una celda no sobrevive",
    remesh_sharp: "Aristas vivas",
    remesh_sharp_hint: "conserva las esquinas en vez de redondearlas, a costa de \
                        la garantía de malla cerrada — el motor marca este modo \
                        como experimental",
    remesh_remove_loose: "Quitar piezas sueltas",
    remesh_remove_loose_hint: "descarta fragmentos demasiado pequeños para esta resolución",
    remesh_follow: "Seguir la forma actual",
    remesh_follow_hint: "tira de la malla nueva hacia la superficie que sustituye, \
                         recuperando el detalle que el muestreo redondeó",
    remesh_result: "triángulos",
    remesh_pieces: "piezas",
    remesh_uvs_dropped: "se descartaron las coordenadas de textura",
    remesh_not_watertight: "el resultado no quedó cerrado",
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

    shelf_no_tools: "Todavía no hay herramientas para esta representación",
    shelf_filter_all: "Disponibles",
    shelf_filter_favourites: "★ Favoritos",
    label_autosave_in: "Guardado automático en",
    state_autosaved: "Nada que guardar",
    action_favourite_add: "Añadir a favoritos",
    action_favourite_remove: "Quitar de favoritos",
    shelf_no_favourites: "Ningún pincel en favoritos — usa el menú de un pincel",
    viewport_profile_names: ["Rendimiento", "Escultura", "Presentación"],
    label_viewport_profile: "Calidad de la vista",
    panel_names: ["Panel izquierdo", "Panel derecho", "Estante"],
    action_reset_layout: "Restablecer disposición",
    action_focus: "Modo enfoque",
    hud_position: "Posición",
    hud_rotation: "Rotación",
    hud_axis: "Eje",
    hud_scale: "Escala",
    shelf_tool_elsewhere: "sin verbo en la capa activa",
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
    convert_undo_note: "un deshacer revierte la travesía entera",
    convert_in_place: "Sustituir la capa",
    convert_in_place_hint: "La capa leída sale y el resultado ocupa su fila · un deshacer devuelve ambas",
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
    action_shading: "Iluminação de estúdio",
        action_cavity: "Realce de cavidades",
        action_shadows: "Sombra do estúdio",
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
    document_untitled: "Sin título",
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

    /// What a brush does, in one sentence, in this language.
    pub fn tool_hint(&self, tool: clayspace_model::ToolKind) -> &'static str {
        Self::at(&self.tool_hints, clayspace_model::ToolKind::ALL, tool)
    }

    /// Every brush hint, for a test that checks the whole set at once.
    /// What differs about a tool on this representation, where anything does.
    pub fn tool_note(&self, note: clayspace_model::ToolNote) -> &'static str {
        Self::at(&self.tool_notes, clayspace_model::ToolNote::ALL, note)
    }

    /// The tool's sentence, with that caveat after it where there is one.
    pub fn tool_sentence(
        &self,
        tool: clayspace_model::ToolKind,
        representation: clayspace_model::Representation,
    ) -> String {
        let hint = self.tool_hint(tool);
        match tool.note_on(representation) {
            Some(note) => format!("{hint}\n{}", self.tool_note(note)),
            None => hint.to_string(),
        }
    }

    pub fn tool_hints(&self) -> &[&'static str] {
        &self.tool_hints
    }

    /// The name for one combine operation, in this locale.
    ///
    /// By position in `Combine::ALL`, which is what makes a new operation
    /// without a name a compile error rather than a Portuguese word on an
    /// English screen.
    pub fn combine_name(&self, op: clayspace_model::Combine) -> &'static str {
        Self::at(&self.combine_names, clayspace_model::Combine::ALL, op)
    }

    /// The name for one of the four views, in this locale.
    pub fn view_preset_name(&self, preset: clayspace_model::ViewPresetKind) -> &'static str {
        Self::at(
            &self.view_preset_names,
            clayspace_model::ViewPresetKind::ALL,
            preset,
        )
    }

    /// The name for a brush edge profile, in this locale.
    pub fn falloff_name(&self, falloff: clayspace_model::Falloff) -> &'static str {
        Self::at(&self.falloff_names, clayspace_model::Falloff::ALL, falloff)
    }

    /// The name for a reference plane, in this locale.
    pub fn ref_plane_name(&self, plane: clayspace_model::RefPlane) -> &'static str {
        Self::at(&self.ref_plane_names, clayspace_model::RefPlane::ALL, plane)
    }

    /// The name for a way of joining a curve's points, in this locale.
    pub fn curve_join_name(&self, join: clayspace_model::CurveJoin) -> &'static str {
        Self::at(
            &self.curve_join_names,
            clayspace_model::CurveJoin::ALL,
            join,
        )
    }

    /// The name for a curve's cross-section, in this locale.
    pub fn curve_profile_name(&self, profile: clayspace_model::CurveProfile) -> &'static str {
        Self::at(
            &self.curve_profile_names,
            clayspace_model::CurveProfile::ALL,
            profile,
        )
    }

    /// The name for a mask operation, in this locale, without its amount.
    pub fn mask_op_name(&self, op: clayspace_model::MaskOp) -> &'static str {
        use clayspace_model::MaskOp;
        match op {
            MaskOp::Invert => self.mask_op_invert,
            MaskOp::Clear => self.mask_op_clear,
            MaskOp::Expand(_) => self.mask_op_expand,
            MaskOp::Contract(_) => self.mask_op_contract,
            MaskOp::Smooth(_) => self.mask_op_smooth,
            MaskOp::InvertWithinBounds => self.mask_op_complement,
        }
    }

    /// What the geometry panel notes about the counts, in this locale.
    pub fn detail_note(&self, detail: clayspace_model::Detail) -> Option<&'static str> {
        use clayspace_model::Detail;
        match detail {
            Detail::Full => None,
            Detail::Reduced => Some(self.detail_reduced),
            Detail::Pending => Some(self.detail_pending),
        }
    }

    /// The name for one of the manipulator's modes, in this locale.
    pub fn gizmo_mode_name(&self, mode: clayspace_model::GizmoMode) -> &'static str {
        Self::at(
            &self.gizmo_mode_names,
            clayspace_model::GizmoMode::ALL,
            mode,
        )
    }

    /// The name for one deformation, in this locale.
    pub fn deform_verb_name(&self, verb: clayspace_model::DeformVerb) -> &'static str {
        Self::at(
            &self.deform_verb_names,
            clayspace_model::DeformVerb::ALL,
            verb,
        )
    }

    pub fn blend_name(&self, blend: clayspace_model::BlendProfile) -> &'static str {
        Self::at(&self.blend_names, clayspace_model::BlendProfile::ALL, blend)
    }

    /// What the mask brush's gesture is called, in this locale.
    pub fn mask_gesture_name(&self, gesture: clayspace_model::MaskGesture) -> &'static str {
        Self::at(
            &self.mask_gesture_names,
            clayspace_model::MaskGesture::ALL,
            gesture,
        )
    }

    pub fn extrude_side_name(&self, side: clayspace_model::ExtrudeSide) -> &'static str {
        Self::at(
            &self.extrude_side_names,
            clayspace_model::ExtrudeSide::ALL,
            side,
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

    pub fn panel_name(&self, panel: crate::layout::Panel) -> &'static str {
        Self::at(&self.panel_names, crate::layout::Panel::ALL, panel)
    }

    pub fn viewport_profile_name(&self, profile: crate::quality::ViewportProfile) -> &'static str {
        Self::at(
            &self.viewport_profile_names,
            crate::quality::ViewportProfile::ALL,
            profile,
        )
    }

    pub fn representation_sentence(&self, what: clayspace_model::Representation) -> &'static str {
        Self::at(
            &self.representation_sentences,
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
    pub fn all(&self) -> [&'static str; 226] {
        [
            self.label_autosave_in,
            self.state_autosaved,
            self.shelf_filter_favourites,
            self.action_favourite_add,
            self.action_favourite_remove,
            self.shelf_no_favourites,
            self.action_reset_layout,
            self.action_focus,
            self.label_voxel_cell,
            self.label_voxel_occupied,
            self.label_viewport_profile,
            self.hud_position,
            self.hud_rotation,
            self.hud_axis,
            self.hud_scale,
            self.shelf_filter_all,
            self.shelf_tool_elsewhere,
            self.section_field,
            self.section_voxels,
            self.section_mesh,
            self.label_field_items,
            self.label_field_collapsed,
            self.state_yes,
            self.state_no,
            self.mesh_topology_fixed,
            self.section_representation,
            self.hint_representation_active,
            self.hint_representation_other,
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
            self.section_objects,
            self.mask_op_invert,
            self.mask_op_clear,
            self.mask_op_expand,
            self.mask_op_contract,
            self.mask_op_smooth,
            self.mask_op_complement,
            self.detail_reduced,
            self.detail_pending,
            self.label_no_placed_objects,
            self.label_object_scale,
            self.label_shapes_sdf_only,
            self.label_manipulator,
            self.hint_material,
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
            self.label_transform,
            self.hint_layer_transform,
            self.hint_transform_taken,
            self.hint_transform_needs_a_layer,
            self.action_paint_mask,
            self.label_mask_gesture,
            self.hint_mask_outline,
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
            self.section_shapes,
            self.section_boolean,
            self.section_geometry,
            self.section_resolution,
            self.section_brush_controls,
            self.section_armature,
            self.section_diagnostics,
            self.section_rendering,
            self.section_mesh_sculpting,
            self.label_grain,
            self.label_intensity,
            self.label_size,
            self.label_flow,
            self.label_combine,
            self.label_blend,
            self.label_alpha,
            self.label_colour,
            self.label_recent_colours,
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
            self.action_shading,
            self.action_cavity,
            self.action_shadows,
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
            self.document_untitled,
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
    fn every_representation_is_named_and_explained_in_every_language() {
        // The bar states all three at once, so a card missing its phrase is
        // a hole beside two that have one — and two cards sharing a phrase
        // would say the representations are the same thing.
        for locale in Locale::ALL {
            let strings = Strings::for_locale(locale);
            let mut seen = std::collections::BTreeSet::new();
            for what in clayspace_model::Representation::ALL {
                let name = strings.representation_name(what);
                let sentence = strings.representation_sentence(what);
                assert!(
                    !name.is_empty(),
                    "{what:?} has no name in {}",
                    locale.label()
                );
                assert!(
                    !sentence.is_empty(),
                    "{what:?} has no phrase in {}",
                    locale.label()
                );
                assert!(
                    seen.insert(sentence),
                    "{} explains two representations as {sentence:?}",
                    locale.label()
                );
            }
        }
        let english = Strings::for_locale(Locale::EnUs);
        let portuguese = Strings::for_locale(Locale::PtBr);
        assert!(
            clayspace_model::Representation::ALL
                .iter()
                .all(|what| english.representation_sentence(*what)
                    != portuguese.representation_sentence(*what)),
            "a representation reads the same in both, so one table was copied"
        );
    }

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
    fn the_untitled_document_is_named_in_every_language() {
        // The ViewModel names a fresh document with one fixed marker and knows
        // no locale, so "Sem título" reached the menu bar on every language
        // until the View started translating it. The word has to differ from
        // the Portuguese in each other table, or the mapping is a no-op.
        let portuguese = Strings::for_locale(Locale::PtBr).document_untitled;
        assert_eq!(portuguese, clayspace_vm::UNTITLED);
        for locale in [Locale::EnUs, Locale::Es419] {
            assert_ne!(
                Strings::for_locale(locale).document_untitled,
                portuguese,
                "{} keeps the Portuguese untitled name",
                locale.label()
            );
        }
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
    fn every_brush_says_what_it_does_in_every_language() {
        // The swatch shows a name and a mark; the hint is the sentence that
        // says what the mark means, so it has to exist and be its own.
        use clayspace_model::ToolKind;
        for locale in Locale::ALL {
            let strings = Strings::for_locale(locale);
            let hints = strings.tool_hints();
            for (index, tool) in ToolKind::ALL.iter().enumerate() {
                let hint = strings.tool_hint(*tool);
                assert!(
                    !hint.is_empty(),
                    "{tool:?} says nothing in {}",
                    locale.label()
                );
                assert!(
                    hint.len() <= 90,
                    "{tool:?}'s hint in {} is a paragraph, not a sentence",
                    locale.label()
                );
                assert!(
                    !hints[..index].contains(&hint),
                    "{tool:?} shares its hint with another brush in {}",
                    locale.label()
                );
            }
        }
    }

    #[test]
    fn the_brush_hints_are_translated_rather_than_copied() {
        for (first, second) in [
            (Locale::EnUs, Locale::PtBr),
            (Locale::EnUs, Locale::Es419),
            (Locale::PtBr, Locale::Es419),
        ] {
            let a = Strings::for_locale(first).tool_hints();
            let b = Strings::for_locale(second).tool_hints();
            let same = a.iter().zip(b.iter()).filter(|(x, y)| x == y).count();
            assert_eq!(
                same,
                0,
                "{same} brush hints are the same in {} and {}",
                first.label(),
                second.label()
            );
        }
    }

    #[test]
    fn what_the_live_screen_showed_in_portuguese_is_translated() {
        // Seen on a running English build: Perspectiva under the viewport,
        // Dura and Suave on the edge chips, "ainda não gerado" in the geometry
        // panel. Every one of these tables must differ from the Portuguese.
        let en = Strings::for_locale(Locale::EnUs);
        let pt = Strings::for_locale(Locale::PtBr);
        let same = |a: &[&str], b: &[&str]| a.iter().zip(b).filter(|(x, y)| x == y).count();
        assert_eq!(same(&en.view_preset_names, &pt.view_preset_names), 0);
        assert!(
            same(&en.falloff_names, &pt.falloff_names) <= 1,
            "only Linear may coincide"
        );
        assert_eq!(same(&en.ref_plane_names, &pt.ref_plane_names), 0);
        assert_eq!(same(&en.curve_join_names, &pt.curve_join_names), 0);
        assert_eq!(same(&en.curve_profile_names, &pt.curve_profile_names), 0);
        assert_ne!(en.detail_pending, pt.detail_pending);
        assert_ne!(en.mask_op_invert, pt.mask_op_invert);
        assert_eq!(en.detail_note(clayspace_model::Detail::Full), None);
    }

    #[test]
    fn the_manipulator_and_the_deformations_are_named_in_english() {
        // Both were drawn from the domain's `label()`, so an English screen
        // read Mover, Girar, Escalar, Afunilar and Torcer.
        let english = Strings::for_locale(Locale::EnUs);
        let portuguese = Strings::for_locale(Locale::PtBr);
        for mode in clayspace_model::GizmoMode::ALL {
            assert_ne!(
                english.gizmo_mode_name(mode),
                portuguese.gizmo_mode_name(mode),
                "{mode:?} is not translated"
            );
        }
        for verb in clayspace_model::DeformVerb::ALL {
            assert_ne!(
                english.deform_verb_name(verb),
                portuguese.deform_verb_name(verb),
                "{verb:?} is not translated"
            );
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
                strings.section_objects,
                strings.section_material,
                strings.section_shapes,
                strings.section_boolean,
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
