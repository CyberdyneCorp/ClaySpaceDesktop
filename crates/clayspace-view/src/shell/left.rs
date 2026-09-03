//! The left region: the scene, the layer stack, and what a stroke is made
//! under.
//!
//! The stack is the part a sculptor looks at most, so the rows carry more than
//! their names: what each layer holds, how strongly it acts, whether it is
//! visible, whether it is protected, and — on the active one — an accent rail,
//! because the tone step that used to carry that alone is three and a half per
//! cent of relative luminance.

use super::*;

/// Where the offer to collapse a costly layer was drawn, for a test that has
/// to establish it was drawn at all. Absent when the engine is not advising,
/// which is most of the time.
pub fn optimize_button_id() -> egui::Id {
    egui::Id::new("subtool-optimize")
}

/// Where the offer to rebuild a mesh layer's topology was drawn. Absent for
/// every representation but a mesh, and for a mesh row whose triangles have
/// not arrived.
pub fn remesh_button_id() -> egui::Id {
    egui::Id::new("subtool-remesh")
}

/// The scene tree and the layer stack.
pub fn left_panel(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    scene_section(ui, state);
    layers_section(ui, state, queue);
}

/// The scene tree: every node, indented by its depth, with whether it shows.
pub(super) fn scene_section(ui: &mut egui::Ui, state: &ShellState<'_>) {
    if !heading(ui, state.strings.section_scene) {
        return;
    }
    for node in &state.scene.nodes {
        ui.horizontal(|ui| {
            ui.add_space(space::SNUG + node.depth as f32 * space::ROOMY);
            // The active layer, because there is only one: the tree and the
            // stack read the same fact, so a click in the viewport lights the
            // same row a click in the stack does.
            let selected = state.scene.active == Some(node.key);
            let text = egui::RichText::new(&node.name)
                .size(type_scale::BODY)
                .color(if selected {
                    Tokens::text()
                } else {
                    Tokens::text_dim()
                });
            ui.label(text);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                icons::button(
                    ui,
                    if node.visible {
                        Icon::Visible
                    } else {
                        Icon::Hidden
                    },
                    node.visible,
                );
            });
        });
    }
}

/// The layer stack, and what stands under it: the placed objects on a field,
/// the recording control on a grid, and the control that adds a layer.
pub(super) fn layers_section(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    if !heading(ui, state.strings.section_layers) {
        return;
    }
    for layer in state.scene.layers.iter().rev() {
        layer_row(ui, state, layer, queue);
    }

    // The shapes standing in the active layer, under the layer they stand in.
    // Only on a field, because that is the only place an object can live, and
    // a heading that could only ever say "none" is worse than no heading.
    if state.representation == Representation::Sdf {
        ui.add_space(space::SNUG);
        object_rows(ui, state, queue);
    }
    // Recording a pass, and what the stack costs. Under the layer list because
    // that is where the passes themselves are, and only for a grid — a field
    // and a mesh have no stack to record into, and a button that can only
    // refuse is worse than one that is not there.
    if state.representation == Representation::Voxel {
        ui.add_space(space::SNUG);
        sculpt_recording_control(ui, state, queue);
    }
    // And what the field costs, where there is a field. Only when the engine
    // advises collapsing it: a row that is always there is a row nobody reads,
    // and the number it would carry means nothing to a sculptor until it means
    // something.
    if state.representation == Representation::Sdf {
        field_health_control(ui, state, queue);
    }
    // And the mesh counterpart, which is a rebuild rather than a collapse.
    // Always offered rather than only when something is wrong: a field
    // steepens measurably and the engine can say when, and there is no
    // equivalent number for "this topology has stopped taking detail" — the
    // sculptor is the one who can see that, so the control waits for them
    // instead of waiting for advice that does not exist.
    if state.representation == Representation::Mesh {
        remesh_control(ui, state, queue);
    }
    // And the hierarchy's: a new pass, what the stack costs, and why the
    // composition controls are refusing while the pointer is down. Under the
    // list because that is where the passes themselves are.
    if state.representation == Representation::Multires {
        ui.add_space(space::SNUG);
        multires_pass_control(ui, state, queue);
    }

    ui.add_space(space::SNUG);
    add_layer_control(ui, state, queue);
}

/// Adding a layer, and saying what it should hold.
///
/// The button alone makes the field layer it always made — "the default stays
/// what it was" — and the list beside it is how a sculptor asks for a grid
/// without crossing one afterwards, which is the cost the choice exists to
/// avoid.
///
/// `Representation::CREATABLE` and not `ALL`: a mesh layer comes from carrying
/// a mesh, and the entry that offered one here made a row labelled "Malha"
/// that could never hold a triangle. See the constant for the specification's
/// own qualification.
pub(super) fn add_layer_control(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    ui.horizontal(|ui| {
        let button = ui.button(format!("+  {}", s.label_new_layer));
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(new_layer_button_id(), button.rect));
        if button.clicked() {
            // Layer creation is a document change like any other.
            queue.push(Command::AddLayer(Representation::Sdf));
        }
        let list = egui::ComboBox::from_id_salt("new-layer-kind")
            .selected_text(s.label_new_layer_kind)
            .width(96.0)
            .show_ui(ui, |ui| {
                for representation in Representation::CREATABLE {
                    let response =
                        ui.selectable_label(false, s.representation_name(representation));
                    // Recorded where a test can find it, for the reason
                    // `slider_id` states: this whole control was unreachable
                    // from a test, which is how the entry that made a dead mesh
                    // row went unnoticed.
                    ui.ctx().memory_mut(|memory| {
                        memory
                            .data
                            .insert_temp(new_layer_kind_id(representation), response.rect)
                    });
                    if response.clicked() {
                        queue.push(Command::AddLayer(representation));
                    }
                }
            });
        ui.ctx().memory_mut(|memory| {
            memory
                .data
                .insert_temp(new_layer_kind_menu_id(), list.response.rect)
        });
    });
}

/// The id the "new layer" button carries, so a test can press it.
///
/// The same arrangement [`insert_as_chip_id`] has and for the same reason:
/// reaching a control by coordinate reaches whatever landed above it.
pub fn new_layer_button_id() -> egui::Id {
    egui::Id::new("new-layer-button")
}

/// The id the list beside it carries, which is what has to be pressed before
/// the entries inside it exist at all.
pub fn new_layer_kind_menu_id() -> egui::Id {
    egui::Id::new("new-layer-kind-menu")
}

/// The id one entry of that list carries.
pub fn new_layer_kind_id(representation: Representation) -> egui::Id {
    // By position rather than by name: `representation_name` is interface text
    // and an id built from one moves when a translation does.
    egui::Id::new(("new-layer-kind", representation as u8))
}

pub(super) fn layer_row(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    layer: &LayerSummary,
    queue: &mut CommandQueue,
) {
    let active = state.scene.active == Some(layer.key);
    let fill = if active {
        Tokens::raised()
    } else {
        Tokens::panel()
    };

    let row = egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::symmetric(
            space::SNUG as i8,
            space::TIGHT as i8,
        ))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let eye = if layer.visible {
                    Icon::Visible
                } else {
                    Icon::Hidden
                };
                if icons::button(ui, eye, layer.visible).clicked() {
                    queue.push(Command::SetLayerVisible(layer.key, !layer.visible));
                }

                // The name gets what is left after the row's right-hand side
                // is reserved, and truncates into it. A label sized to its own
                // text takes the whole row and the tag lands on top of it:
                // "Detalhes_secundario$DF" is what that looks like.
                //
                // Reserved rather than laid out by egui because a
                // right-to-left group inside a horizontal claims the remaining
                // width, which is the whole of it when the name has not been
                // bounded first.
                // The height of one widget, not of the rest of the panel:
                // this strip is *allocated* rather than bounded, so a height
                // of `available_height` would make the row as tall as the
                // layer stack has space for.
                let strip = egui::vec2(
                    (ui.available_width() - size::LAYER_ROW_TAIL).max(size::LAYER_NAME_MIN),
                    ui.spacing().interact_size.y,
                );
                match state.renaming {
                    Some((key, draft)) if key == layer.key => {
                        ui.allocate_ui_with_layout(
                            strip,
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| rename_field(ui, draft, queue),
                        );
                    }
                    _ => layer_name(ui, strip, state, layer, active, queue),
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    numeric(ui, format!("{:>3}", layer.intensity));
                    // What the layer holds. Told apart by the word rather than
                    // by a colour, so it survives the contrast theme and a
                    // reader who cannot separate the hues.
                    ui.label(
                        egui::RichText::new(representation_tag(layer.representation))
                            .size(type_scale::LABEL)
                            .color(Tokens::text_dim()),
                    )
                    .on_hover_text(layer.representation.label());
                    // The name grows into whatever is left, so without this
                    // the tag sits flush against it and the two read as one
                    // word: "Detalhes_secundariosSDF".
                    ui.add_space(space::SNUG);
                    if layer.protection.locked || layer.protection.ghost {
                        let icon = if layer.protection.ghost {
                            Icon::Ghost
                        } else {
                            Icon::Locked
                        };
                        let response = icons::button(ui, icon, false);
                        if let Some(refusal) = layer.protection.refusal() {
                            response.on_hover_text(refusal);
                        }
                    }
                });
            });
        });

    // The tone step from `panel` to `raised` is 3.5% of relative luminance,
    // and it was the only thing saying which of four layers a dab would land
    // on. The rail is what makes that answerable at a glance.
    if active {
        selection_rail(ui, row.response.rect);
    }

    // The passes recorded on this layer, nested under it.
    //
    // Under the layer rather than in a panel of its own: a pass has no meaning
    // apart from the grid it was recorded on, and a second stack elsewhere
    // would have to repeat which layer each entry belongs to. Shown only for
    // the active layer — a document with several grids would otherwise unroll
    // every stack at once, and only one of them can be recorded into.
    if active {
        let count = layer.sculpt_layers.len();
        // Top of the stack first, as the layer list itself is ordered: the
        // last pass recorded wins where two overlap, and the thing that wins
        // belongs at the top.
        for pass in layer.sculpt_layers.iter().rev() {
            sculpt_layer_row(ui, state, pass, count, queue);
        }
        // And the hierarchy's own stack, in the same place and for the same
        // reason. A different stack with a different addressing, drawn beside
        // the grid's rather than in a second idiom: a sculptor reading a layer
        // row already knows where a pass lives. Never both at once — a layer
        // is a grid or a hierarchy — so the two loops cannot interleave.
        if let Some(hierarchy) = layer.multires.as_ref() {
            multires_stack(ui, state, hierarchy, queue);
        }
    }
    ui.add_space(space::HAIR);
}

/// The id a hierarchy's pass row is registered under, so a test can ask where
/// a row went rather than measuring it off a capture.
pub fn multires_pass_row_id(id: clayspace_model::MultiresSculptLayerId) -> egui::Id {
    egui::Id::new(("multires-pass", id.raw()))
}

/// The same for the row that stands for the form under the passes.
pub fn multires_form_row_id() -> egui::Id {
    egui::Id::new("multires-form")
}

/// Where the offer of a new pass was drawn.
pub fn multires_add_pass_id() -> egui::Id {
    egui::Id::new("multires-add-pass")
}

/// And the offer to release what a stroke that undid itself left behind.
pub fn multires_compact_id() -> egui::Id {
    egui::Id::new("multires-compact")
}

/// A hierarchy's stack of passes, nested under the layer it stands on.
///
/// Top of the stack first, as the layer list itself is ordered — and then the
/// **form**, which is not a pass and is drawn as one anyway. That row is the
/// whole of the write domain as this application expresses it: `Automatic`
/// resolves to the active pass, or to the form where none is active, so which
/// row is selected *is* the answer to "where does the next stroke go". A
/// separate three-way control beside these rows would be a second way to say
/// the same thing, and the two would disagree the first time one of them
/// moved.
///
/// The form is at the bottom because that is where it is: everything above it
/// is stacked on it.
fn multires_stack(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    hierarchy: &clayspace_model::MultiresState,
    queue: &mut CommandQueue,
) {
    for pass in hierarchy.sculpt_layers.iter().rev() {
        multires_pass_row(
            ui,
            state,
            pass,
            hierarchy.active_sculpt_layer == pass.id,
            queue,
        );
    }
    multires_form_row(ui, state, hierarchy.active_sculpt_layer.is_base(), queue);
}

/// Adding a pass, what the stack costs, and why it is refusing.
///
/// The offer is enabled and refused by the model rather than greyed out here,
/// which is the arrangement every other control in this panel takes: the
/// engine holds the composition for the length of a gesture and answers with a
/// sentence, and a button greyed from this side would have to guess at the
/// same rule and could come to disagree with it. What is drawn here instead is
/// the *reason*, while it stands.
///
/// The bytes are shown and nothing is enforced against them, for the reason
/// the grid's stack enforces nothing against its own: a cap that silently
/// stopped recording would leave a pass on the surface and un-dialable. What a
/// sculptor has instead is four levers — compact, dial, merge, remove — in
/// increasing order of what they cost.
fn multires_pass_control(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    use clayspace_model::MultiresSculptLayerOp as Op;

    let s = state.strings;
    ui.horizontal(|ui| {
        let add = ui.button(format!("+  {}", s.multires_add_pass));
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(multires_add_pass_id(), add.rect));
        if add.clicked() {
            queue.push(Command::MultiresSculptLayer(Op::Add {
                // Unnamed: `MultiresSculptLayer::display_name` numbers it off
                // the row it is standing in, which is the number a sculptor is
                // counting. A name minted here would be minted from an id, and
                // ids are never reused — so a sculptor who made and deleted
                // nine passes would meet "Passe 10" as their second.
                name: String::new(),
            }));
        }
        let Some(cost) = state.multires_cost else {
            return;
        };
        // Offered only where there is something to release. Walking every
        // stored block of every pass is proportional to the stack rather than
        // to the dab, so this is a button and never something done for a
        // sculptor between strokes.
        if cost.layers > 0 {
            let compact = ui.button(s.multires_compact);
            ui.ctx()
                .memory_mut(|memory| memory.data.insert_temp(multires_compact_id(), compact.rect));
            if compact.clicked() {
                queue.push(Command::MultiresSculptLayer(Op::Compact));
            }
        }
    });
    // On its own line rather than beside the buttons: the panel is two hundred
    // pixels wide and the figure came off the end of it, reading "18.0 M".
    if let Some(cost) = state.multires_cost.filter(|cost| cost.layers > 0) {
        ui.label(
            egui::RichText::new(format!(
                "{} · {} {}",
                bytes_label(cost.bytes),
                thousands(cost.coverage_vertices as usize),
                s.multires_vertices
            ))
            .size(type_scale::LABEL)
            .color(if cost.worth_compacting() {
                Tokens::accent()
            } else {
                Tokens::text_dim()
            }),
        );
    }
    // The state that reads as a set of controls that have stopped working,
    // said where it is true. A stamp reads the evaluated surface, so a slider
    // moved between two stamps would author one gesture against two different
    // surfaces — the engine refuses rather than deferring, and this is that
    // refusal arriving before it is met.
    if state.multires_cost.is_some_and(|cost| cost.stroke_open) {
        ui.label(
            egui::RichText::new(s.multires_stroke_open)
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }
}

/// One pass on a hierarchy, indented under the layer it stands on.
///
/// Deliberately the same shape as [`sculpt_layer_row`] beside it, because a
/// sculptor should not have to learn a second layer idiom to work a second
/// representation. What differs is what the two stacks actually differ in:
/// this one is addressed by an id rather than by a position, it carries a lock
/// a grid's passes do not have, and its order is organisation rather than
/// result — so it is reordered by dragging the row rather than by two arrows,
/// which on a grid say something about which pass wins and here would say
/// something untrue.
///
/// **The name is the drag handle**, which is what a layer list does everywhere
/// else and what the width allows: the left panel gives a nested row about two
/// hundred pixels, and the first version of this put a grip, two icons, a
/// name, three glyph buttons and a dial in them. The picture is what caught
/// it — the name came out as a single letter between two boxes, and two of the
/// three glyphs were not in the font at all. So everything that is not reached
/// often lives in the row's own menu, where deleting a layer already lives.
fn multires_pass_row(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    pass: &clayspace_model::MultiresSculptLayer,
    // Whether the next stroke lands here.
    selected: bool,
    queue: &mut CommandQueue,
) {
    use clayspace_model::MultiresSculptLayerOp as Op;

    let row = egui::Frame::new()
        .fill(if selected {
            Tokens::raised()
        } else {
            Tokens::panel()
        })
        .inner_margin(egui::Margin::symmetric(
            space::SNUG as i8,
            space::HAIR as i8,
        ))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(space::SNUG);
                let eye = if pass.visible {
                    Icon::Visible
                } else {
                    Icon::Hidden
                };
                if icons::button(ui, eye, pass.visible).clicked() {
                    queue.push(Command::MultiresSculptLayer(Op::SetVisible {
                        id: pass.id,
                        visible: !pass.visible,
                    }));
                }
                multires_pass_name(ui, state, pass, selected, queue);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut strength = pass.strength;
                    let dial = ui.add(
                        egui::DragValue::new(&mut strength)
                            .speed(0.01)
                            .range(0.0..=1.0)
                            .fixed_decimals(2),
                    );
                    // The widget's own id, so a test can put the keyboard on
                    // it. A click on a `DragValue` sets it to wherever the
                    // pointer landed, so a test driven by clicking would pass
                    // with the emission below deleted.
                    ui.ctx().memory_mut(|memory| {
                        memory
                            .data
                            .insert_temp(multires_strength_id(pass.id), dial.id)
                    });
                    if dial.changed() {
                        queue.push(Command::MultiresSculptLayer(Op::SetStrength {
                            id: pass.id,
                            strength,
                        }));
                    }
                    multires_pass_badges(ui, state, pass);
                });
            });
        });

    ui.ctx().memory_mut(|memory| {
        memory
            .data
            .insert_temp(multires_pass_row_id(pass.id), row.response.rect)
    });
    if selected {
        selection_rail(ui, row.response.rect);
    }
    // Dropped on this row: the dragged pass takes this one's place. A position
    // and not an offset, because that is what the engine's move takes and what
    // a list drag means — released over the third row, it becomes the third.
    if let Some(dragged) = row
        .response
        .dnd_release_payload::<clayspace_model::MultiresSculptLayerId>()
    {
        if *dragged != pass.id {
            queue.push(Command::MultiresSculptLayer(Op::Move {
                id: *dragged,
                to: pass.index,
            }));
        }
    }
}

/// What a pass is, drawn as marks rather than as controls.
///
/// On the right and beside the dial, which is where a layer row already puts
/// its lock and its ghost, and shown only where they are true — so the name
/// begins in the same place on every row and a stack of ordinary passes is a
/// clean column rather than a grid of grey icons. The lock is set from the
/// row's menu for the same reason a layer's protection is.
///
/// The picture is what decided this. The first version put a lock button on
/// every row: `icons::button` separates its two states by one tone step, and
/// three rows of which one was locked were indistinguishable at sixteen
/// pixels.
fn multires_pass_badges(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    pass: &clayspace_model::MultiresSculptLayer,
) {
    let s = state.strings;
    let badge = |ui: &mut egui::Ui, icon: Icon, hint: &str| {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(size::ICON, size::ICON), egui::Sense::hover());
        icons::paint(ui.painter(), rect, icon, Tokens::accent());
        response.on_hover_text(hint.to_owned());
    };
    // A stored mask of the pass's own, which is a different thing from the
    // freeze a brush is gated by: this one is saved with the pass and says
    // where it contributes. See `Hierarchy::sculpt_layers` for why nothing in
    // this application can light it yet.
    if pass.masked {
        badge(ui, Icon::MaskPaint, s.multires_mask);
    }
    if pass.locked {
        badge(ui, Icon::Locked, s.multires_locked);
    }
}

/// The pass's name: what selects it, what drags it, and what its menu hangs
/// off.
///
/// One widget carrying all three because that is what a layer list is
/// everywhere a sculptor has met one. `dnd_drag_source` adds a drag sense over
/// the label and hands back both responses joined, so the click still selects
/// and a drag still reorders — and because the sense is on the label rather
/// than on the whole row, it does not swallow the strength control's own drag.
fn multires_pass_name(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    pass: &clayspace_model::MultiresSculptLayer,
    selected: bool,
    queue: &mut CommandQueue,
) {
    use clayspace_model::MultiresSculptLayerOp as Op;

    let s = state.strings;
    let name = egui::RichText::new(pass.display_name())
        .size(type_scale::LABEL)
        // A pass with nothing stored on it is shown rather than hidden and
        // reads as what it is: a sculptor who made one and has not used it yet
        // needs to see it to send a stroke into it.
        .color(if pass.is_empty() {
            Tokens::text_dim()
        } else {
            Tokens::text()
        });
    let response = ui
        .dnd_drag_source(multires_grip_id(pass.id), pass.id, |ui| {
            ui.selectable_label(selected, name)
        })
        .response
        .on_hover_text(format!(
            "{} · {} {} · {}",
            s.multires_pass_hint,
            thousands(pass.coverage_vertices as usize),
            s.multires_vertices,
            bytes_label(pass.bytes)
        ));
    if response.clicked() {
        queue.push(Command::MultiresSculptLayer(Op::SetActive { id: pass.id }));
    }
    response.context_menu(|ui| multires_pass_menu(ui, state, pass, queue));
}

/// What a pass's own menu offers: the three that take something away.
///
/// In a menu rather than on the row because the row has no width for them and
/// because none of the three is reached often — a sculptor dials a pass many
/// times for every time they merge one. Deleting a layer already lives in a
/// row's menu here, so this is where a sculptor looks.
fn multires_pass_menu(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    pass: &clayspace_model::MultiresSculptLayer,
    queue: &mut CommandQueue,
) {
    use clayspace_model::MultiresSculptLayerOp as Op;

    let s = state.strings;
    // A lock guards the coefficients and nothing else — the name, the slider,
    // the visibility and the mask all stay the sculptor's — so it is offered
    // in both directions from the same entry. A lock a sculptor could not take
    // off from where they put it on would be a trap.
    let lock = ui.button(if pass.locked {
        s.multires_unlock
    } else {
        s.multires_lock
    });
    ui.ctx().memory_mut(|memory| {
        memory
            .data
            .insert_temp(multires_lock_id(pass.id), lock.rect)
    });
    if lock.clicked() {
        queue.push(Command::MultiresSculptLayer(Op::SetLocked {
            id: pass.id,
            locked: !pass.locked,
        }));
        ui.close_menu();
    }
    ui.separator();
    // Merging needs a pass below to merge into, so the entry is absent on the
    // bottom row rather than present and refusing.
    if pass.index > 0 {
        let merge = ui.button(s.sculpt_merge_down);
        ui.ctx().memory_mut(|memory| {
            memory
                .data
                .insert_temp(multires_merge_id(pass.id), merge.rect)
        });
        if merge.clicked() {
            queue.push(Command::MultiresSculptLayer(Op::MergeDown { id: pass.id }));
            ui.close_menu();
        }
    }
    // The same statement with the form under the passes as the target, which
    // is the one every pass can make, including the bottom one.
    let bake = ui.button(s.multires_bake);
    ui.ctx().memory_mut(|memory| {
        memory
            .data
            .insert_temp(multires_bake_id(pass.id), bake.rect)
    });
    if bake.clicked() {
        queue.push(Command::MultiresSculptLayer(Op::BakeToBase { id: pass.id }));
        ui.close_menu();
    }
    let remove = ui.button(s.sculpt_remove);
    ui.ctx().memory_mut(|memory| {
        memory
            .data
            .insert_temp(multires_remove_id(pass.id), remove.rect)
    });
    if remove.clicked() {
        queue.push(Command::MultiresSculptLayer(Op::Remove { id: pass.id }));
        ui.close_menu();
    }
}

/// Where a pass's menu entries were drawn, so a test can ask rather than
/// measure an offset off a capture.
pub fn multires_lock_id(id: clayspace_model::MultiresSculptLayerId) -> egui::Id {
    egui::Id::new(("multires-lock", id.raw()))
}

pub fn multires_merge_id(id: clayspace_model::MultiresSculptLayerId) -> egui::Id {
    egui::Id::new(("multires-merge", id.raw()))
}

pub fn multires_bake_id(id: clayspace_model::MultiresSculptLayerId) -> egui::Id {
    egui::Id::new(("multires-bake", id.raw()))
}

pub fn multires_remove_id(id: clayspace_model::MultiresSculptLayerId) -> egui::Id {
    egui::Id::new(("multires-remove", id.raw()))
}

/// The id a pass's strength control registers itself under.
pub fn multires_strength_id(id: clayspace_model::MultiresSculptLayerId) -> egui::Id {
    egui::Id::new(("multires-strength", id.raw()))
}

/// The id a pass's drag handle is registered under.
fn multires_grip_id(id: clayspace_model::MultiresSculptLayerId) -> egui::Id {
    egui::Id::new(("multires-grip", id.raw()))
}

/// The row that stands for the form under the passes.
///
/// Drawn even where there are no passes, and that is deliberate: a hierarchy
/// with an empty stack is being sculpted in its form, and a row saying so is
/// how a sculptor learns there is anywhere else for a stroke to go before they
/// need it.
///
/// Built like a pass row — the same frame, the same fill when selected, the
/// same rail — and with the two icon slots left empty rather than filled, so
/// its name lines up with theirs. The first version drew it as a bare label
/// and it read as a stray caption under the stack rather than as the row a
/// stroke can be sent to; the picture is what said so.
fn multires_form_row(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    selected: bool,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    let row = egui::Frame::new()
        .fill(if selected {
            Tokens::raised()
        } else {
            Tokens::panel()
        })
        .inner_margin(egui::Margin::symmetric(
            space::SNUG as i8,
            space::HAIR as i8,
        ))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(space::SNUG);
                // Where a pass carries its eye. The form has none — it cannot
                // be hidden, since hiding the surface under the passes is not
                // a thing a hierarchy can do — so the slot is held rather than
                // closed up, or the one row a sculptor is choosing between
                // would be the one that does not line up with the others.
                ui.add_space(size::ICON + ui.spacing().item_spacing.x);
                let label = egui::RichText::new(s.multires_form)
                    .size(type_scale::LABEL)
                    .color(if selected {
                        Tokens::text()
                    } else {
                        Tokens::text_dim()
                    });
                ui.selectable_label(selected, label)
                    .on_hover_text(s.multires_form_hint)
            })
            .inner
        });

    ui.ctx().memory_mut(|memory| {
        memory
            .data
            .insert_temp(multires_form_row_id(), row.response.rect)
    });
    if selected {
        selection_rail(ui, row.response.rect);
    }
    if row.inner.clicked() {
        queue.push(Command::MultiresSculptLayer(
            clayspace_model::MultiresSculptLayerOp::SetActive {
                id: clayspace_model::MultiresSculptLayerId::BASE,
            },
        ));
    }
}

/// One recorded pass, indented under the layer it belongs to.
pub(super) fn sculpt_layer_row(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    pass: &SculptLayer,
    // How many passes the stack holds, so the top one offers no move up.
    count: usize,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    ui.horizontal(|ui| {
        ui.add_space(space::ROOMY);

        let eye = if pass.visible {
            Icon::Visible
        } else {
            Icon::Hidden
        };
        if icons::button(ui, eye, pass.visible).clicked() {
            queue.push(Command::SculptLayer(SculptLayerOp::SetVisible {
                index: pass.index,
                visible: !pass.visible,
            }));
        }

        ui.label(
            egui::RichText::new(pass.display_name())
                .size(type_scale::LABEL)
                // A pass that recorded nothing is shown rather than hidden — a
                // sculptor may have started recording and thought better of it,
                // and a row that vanishes is harder to explain than a dim one —
                // but dialling it does nothing, and it reads that way.
                .color(if pass.is_empty() {
                    Tokens::text_dim()
                } else {
                    Tokens::text()
                }),
        )
        .on_hover_text(format!(
            "{} · {}",
            format_args!("{} {}", thousands(pass.cells), s.sculpt_cells),
            bytes_label(pass.bytes)
        ));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("×")
                .on_hover_text(s.sculpt_remove)
                .clicked()
            {
                queue.push(Command::SculptLayer(SculptLayerOp::Remove {
                    index: pass.index,
                }));
            }
            // Merging needs something below to merge into, so the button is
            // absent on the bottom pass rather than present and refusing.
            if pass.index > 0
                && ui
                    .small_button("⤓")
                    .on_hover_text(s.sculpt_merge_down)
                    .clicked()
            {
                queue.push(Command::SculptLayer(SculptLayerOp::MergeDown {
                    index: pass.index,
                }));
            }
            // Order decides which pass wins where two touched the same cell,
            // so it is worth moving. Each arrow is absent at the end it cannot
            // move toward.
            if pass.index + 1 < count
                && ui
                    .small_button("▲")
                    .on_hover_text(s.sculpt_move_up)
                    .clicked()
            {
                queue.push(Command::SculptLayer(SculptLayerOp::Move {
                    from: pass.index,
                    to: pass.index + 1,
                }));
            }
            if pass.index > 0
                && ui
                    .small_button("▼")
                    .on_hover_text(s.sculpt_move_down)
                    .clicked()
            {
                queue.push(Command::SculptLayer(SculptLayerOp::Move {
                    from: pass.index,
                    to: pass.index - 1,
                }));
            }
            let mut strength = pass.strength;
            if ui
                .add(
                    egui::DragValue::new(&mut strength)
                        .speed(0.01)
                        .range(0.0..=1.0)
                        .fixed_decimals(2),
                )
                .changed()
            {
                queue.push(Command::SculptLayer(SculptLayerOp::SetStrength {
                    index: pass.index,
                    strength,
                }));
            }
        });
    });
}

/// The id the clickable part of a layer's row is registered under.
///
/// Public so a test can ask the interface where a row is rather than measuring
/// it off a capture — a coordinate read from a screenshot is a coordinate that
/// goes stale the next time anything above it changes height.
pub fn layer_row_id(key: LayerKey) -> egui::Id {
    egui::Id::new(("layer-row", key.0))
}

/// A layer's name, and the two gestures that act on it.
///
/// A single click makes the layer active, which is what the panel is mostly
/// for. A double click renames it in place, which is where every layer stack
/// puts that gesture — and the row's own menu says so as well, because a
/// gesture nothing announces is one only its authors know about.
pub(super) fn layer_name(
    ui: &mut egui::Ui,
    strip: egui::Vec2,
    state: &ShellState<'_>,
    layer: &LayerSummary,
    active: bool,
    queue: &mut CommandQueue,
) {
    let name = egui::RichText::new(&layer.name)
        .size(type_scale::BODY)
        // Primary text on the active row, secondary on the rest. One of the
        // three marks the active layer carries — the others are the raised
        // surface under it and the rail at its leading edge — so that none of
        // them is load-bearing on its own.
        .color(if active {
            Tokens::text()
        } else {
            Tokens::text_dim()
        });
    // The whole strip senses the click, not the glyphs. A label senses only
    // the width its own text takes, so a row named "Base" answered across
    // sixty pixels and was dead across the rest of itself: clicking the empty
    // part of the row selected nothing, and right-clicking it opened no menu.
    //
    // Interacted under the layer's own id rather than under the positional one
    // egui would generate. A context menu is keyed by the id of the widget it
    // hangs off, so a positional id moves the menu's identity whenever a row
    // moves in the stack — and it gives a test a name to ask the rectangle for
    // instead of a coordinate measured off a screenshot.
    let (rect, _) = ui.allocate_exact_size(strip, egui::Sense::hover());
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| ui.add(egui::Label::new(name).truncate()),
    );
    // After the label, not before. A truncated `Label` senses hover so it can
    // show the full text, and the widget registered last is the one on top —
    // so an interaction claimed first was shadowed for exactly the rows whose
    // names were long enough to truncate. Short names worked, long ones were
    // dead: the two halves of the stack behaved differently for a reason
    // nothing in the row said.
    let response = ui.interact(rect, layer_row_id(layer.key), egui::Sense::click());
    if response.double_clicked() {
        queue.push(Command::BeginRenameLayer(layer.key));
    } else if response.clicked() {
        queue.push(Command::SelectLayer(layer.key));
    }
    response.context_menu(|ui| layer_menu(ui, state, layer, queue));
}

/// What a layer row's own menu offers.
/// Where a layer row's crossing entry is recorded, so a test can find it by
/// the layer and the representation rather than by a pixel.
pub fn layer_convert_id(key: LayerKey, target: Representation) -> egui::Id {
    egui::Id::new(("layer-convert", key.0, target))
}

pub(super) fn layer_menu(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    layer: &LayerSummary,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    if ui.button(s.action_rename_layer).clicked() {
        queue.push(Command::BeginRenameLayer(layer.key));
        ui.close_menu();
    }
    // The state to push rather than a toggle read off the row: the scene says
    // which subtool is alone, so the entry cannot offer to solo one that
    // already is.
    let soloed = state.scene.is_soloed(layer.key);
    let solo = if soloed {
        s.action_release_solo
    } else {
        s.action_solo_layer
    };
    if ui.button(solo).clicked() {
        queue.push(Command::SoloLayer((!soloed).then_some(layer.key)));
        ui.close_menu();
    }
    // Crossing this layer into another representation, in place.
    //
    // `ConversionSettings::in_place` is what a sculptor means by converting
    // *this* layer — the source leaves as the result arrives and the result
    // stands where it stood, rather than a second layer appearing beside the
    // original. The setting has always been there; there was no way to ask for
    // it from the layer itself.
    //
    // Offered from the row rather than only from the representation bar because
    // the bar speaks for the *active* layer, and a sculptor looking at a stack
    // means the row they opened the menu on.
    let crossings = Direction::from_representation(layer.representation);
    if !crossings.is_empty() {
        ui.separator();
        for direction in crossings {
            let target = direction.to();
            // An ellipsis, because this opens the panel rather than converting:
            // a crossing costs something, a crossing into cells needs a size
            // chosen, and one that would not fit the budget is refused. The
            // panel is where all three are said.
            let label = format!("{} {}…", s.label_convert_to, s.representation_name(target));
            let entry = ui.button(label);
            ui.ctx().memory_mut(|memory| {
                memory
                    .data
                    .insert_temp(layer_convert_id(layer.key, target), entry.rect)
            });
            if entry.clicked() {
                // Made active first: the conversion acts on the active layer,
                // so a crossing asked of a row that is not the active one would
                // otherwise convert something else entirely.
                queue.push(Command::SelectLayer(layer.key));
                queue.push(Command::SetConversion(
                    clayspace_model::ConversionSettings {
                        direction,
                        in_place: true,
                        ..state.conversion
                    },
                ));
                if !state.show_convert {
                    queue.push(Command::ToggleConvert);
                }
                ui.close_menu();
            }
        }
        ui.separator();
    }

    // Disabled with the reason on it rather than offered and refused. The
    // document keeps one layer to sculpt on, and the model says so — but a
    // menu entry whose only outcome is an error message in the status area is
    // one the sculptor has to try before learning anything.
    let last = state.scene.layers.len() <= 1;
    let remove = ui.add_enabled(!last, egui::Button::new(s.action_remove_layer));
    if remove.clicked() {
        queue.push(Command::RemoveLayer(layer.key));
        ui.close_menu();
    }
    if last {
        remove.on_hover_text(s.layer_last_one);
    }
}

/// What the engine says about the active field layer, and the one thing to do
/// about it.
///
/// A chain of edits steepens the field it produces until a ray march takes
/// many small steps and every dab pays for it; the engine measures that and
/// says when collapsing the layer is worth it. Measured here, a layer the
/// engine advised on took a dab from 56 ms to 13 ms once collapsed — and
/// collapsing it took about six seconds.
///
/// Which is why this offers and never acts. Consolidation costs seconds and
/// changes what the layer holds, so it is the sculptor's decision; the
/// interface's job is to say that the moment has arrived, which until now it
/// never did — the engine's advice was computed and read by nothing.
pub(super) fn field_health_control(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    let Some(layer) = state.scene.active_layer() else {
        return;
    };
    let Some(health) = layer.health else {
        return;
    };
    if !health.advises_consolidation || health.consolidated {
        return;
    }

    ui.add_space(space::SNUG);
    ui.label(
        egui::RichText::new(s.optimize_advice)
            .size(type_scale::LABEL)
            .color(Tokens::accent()),
    );
    ui.horizontal(|ui| {
        let button = ui.button(s.optimize_action);
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(optimize_button_id(), button.rect));
        if button.clicked() {
            queue.push(Command::OptimizeLayer(layer.key));
        }
        // The count rather than the step scale: a sculptor can see how many
        // strokes they have made and cannot see a Lipschitz bound.
        ui.label(
            egui::RichText::new(health.items.to_string())
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
    });
}

/// Rebuilding a mesh layer's topology through a voxel field — DynaMesh.
///
/// The mesh counterpart to [`field_health_control`], and offered on the same
/// terms: it costs seconds, it destroys what it replaces, and it is never
/// taken quietly. What differs is when it is shown. A field's steepening is
/// something the engine measures and advises on, so that row appears only when
/// the advice arrives; a mesh's topology going wrong under a pull is something
/// only the sculptor can see, so this one is always there for them to reach.
///
/// The price is on the heading's hover rather than in a paragraph under the
/// button. It is the same sentence every time and a sculptor reaching for this
/// the tenth time is not reading it — but the first time, and the time they
/// wonder where their UVs went, it has to be somewhere.
pub(super) fn remesh_control(ui: &mut egui::Ui, state: &ShellState<'_>, queue: &mut CommandQueue) {
    let s = state.strings;
    let Some(layer) = state.scene.active_layer() else {
        return;
    };

    ui.add_space(space::SNUG);
    ui.label(
        egui::RichText::new(s.remesh_heading)
            .size(type_scale::LABEL)
            .color(Tokens::text_dim()),
    )
    .on_hover_text(s.remesh_hint);

    let mut settings = state.remesh;
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(s.remesh_resolution)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        // Logarithmic, because the numbers a sculptor moves between are 64,
        // 128, 256 rather than 128, 129, 130: on a linear track the whole
        // useful lower half of the range is the first centimetre.
        let slider = ui.add(
            egui::Slider::new(
                &mut settings.resolution,
                clayspace_model::RemeshSettings::RESOLUTION,
            )
            .logarithmic(true)
            .show_value(true),
        );
        if slider.changed() {
            changed = true;
        }
        slider.on_hover_text(s.remesh_resolution_hint);
    });

    for (label, hint, flag) in [
        (
            s.remesh_remove_loose,
            s.remesh_remove_loose_hint,
            &mut settings.remove_loose_pieces,
        ),
        (
            s.remesh_follow,
            s.remesh_follow_hint,
            &mut settings.follow_the_source,
        ),
        (s.remesh_sharp, s.remesh_sharp_hint, &mut settings.sharp),
    ] {
        let toggle = ui.checkbox(
            flag,
            egui::RichText::new(label)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        if toggle.changed() {
            changed = true;
        }
        toggle.on_hover_text(hint);
    }

    if changed {
        queue.push(Command::SetRemeshSettings(settings));
    }

    ui.horizontal(|ui| {
        // Enabled, and refused by the model where it has to be. The usual
        // arrangement in this file is to disable a control with its reason on
        // it, and the one case that would need it here — a mesh row whose
        // triangles have not arrived, since an import fills one later — is not
        // a fact the scene carries. Guessing at it from the layer's
        // representation would disable nothing and only look as though it
        // did, so the refusal is left where it is measurable: the model
        // states it and the shell shows it beside the layer stack.
        let button = ui.button(s.remesh_action);
        ui.ctx()
            .memory_mut(|memory| memory.data.insert_temp(remesh_button_id(), button.rect));
        if button.clicked() {
            // Made active first, as a conversion is: the rebuild acts on the
            // active layer, so asking it of a row that is not the active one
            // would rebuild something else entirely.
            queue.push(Command::SelectLayer(layer.key));
            queue.push(Command::RemeshLayer(layer.key));
        }

        // What the last one came to, beside the button that made it. The
        // triangle counts are the answer to "was that the resolution I meant";
        // the piece count is the answer to "why did those two not join".
        if let Some(outcome) = state.remesh_outcome {
            ui.label(
                egui::RichText::new(format!(
                    "{} → {} {}",
                    outcome.triangles_before, outcome.triangles_after, s.remesh_result
                ))
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
            );
        }
    });

    let Some(outcome) = state.remesh_outcome else {
        return;
    };
    if outcome.pieces > 1 {
        ui.label(
            egui::RichText::new(format!("{} {}", outcome.pieces, s.remesh_pieces))
                .size(type_scale::LABEL)
                .color(Tokens::accent()),
        );
    }
    // Both of these are things the sculptor cannot see by looking at the
    // result, which is the whole reason they are said. Dropped UVs are not a
    // failure — the engine will not pretend to reproject a layout across a
    // seam — and a rebuild that did not come out closed is the sharp mode
    // being what it is documented to be.
    for notice in [
        outcome.uvs_dropped.then_some(s.remesh_uvs_dropped),
        (!outcome.watertight).then_some(s.remesh_not_watertight),
    ]
    .into_iter()
    .flatten()
    {
        ui.label(
            egui::RichText::new(notice)
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
    }
}

/// Starting and stopping a recording, and what the stack occupies.
pub(super) fn sculpt_recording_control(
    ui: &mut egui::Ui,
    state: &ShellState<'_>,
    queue: &mut CommandQueue,
) {
    let s = state.strings;
    let cost = state.sculpt_cost;

    ui.horizontal(|ui| {
        if cost.recording {
            if ui.button(s.sculpt_end).clicked() {
                queue.push(Command::SculptLayer(SculptLayerOp::EndRecording));
            }
            ui.label(
                egui::RichText::new(s.sculpt_recording)
                    .size(type_scale::LABEL)
                    .color(Tokens::accent()),
            );
        } else if ui.button(s.sculpt_begin).clicked() {
            // Unnamed: the engine takes a name and the panel numbers what has
            // none, so a sculptor is not stopped by a dialog for something
            // they will rename or never look at.
            queue.push(Command::SculptLayer(SculptLayerOp::BeginRecording {
                name: String::new(),
            }));
        }
    });

    if cost.layers > 0 {
        ui.label(
            egui::RichText::new(bytes_label(cost.bytes))
                .size(type_scale::LABEL)
                .color(Tokens::text_dim()),
        );
        // Said rather than enforced. A cap that silently stopped recording
        // would leave a pass on the grid and un-dialable, which is a
        // correctness bug wearing a memory limit's clothes — so the number is
        // shown and the sculptor decides.
        if cost.worth_merging() {
            ui.label(
                egui::RichText::new(s.sculpt_worth_merging)
                    .size(type_scale::LABEL)
                    .color(Tokens::accent()),
            );
        }
    }
}
