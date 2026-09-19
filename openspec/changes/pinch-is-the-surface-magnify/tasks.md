## 1. The wrapper

- [x] 1.1 Bind `clay_layer_magnify_surface` and `clay_layer_magnify_surface_preview` in `crates/claycore/src/sculpt.rs`, beside `move_surface` and its preview.
- [x] 1.2 Give the descriptor a `MagnifyParams` that fills `struct_size` from the type compiled against, as every other descriptor does, and carry only the radius and the easing — a radial scale has no half-space to gate and no gesture id to fold on.

## 2. The field verb

- [x] 2.1 Route `Pinçar` on an SDF layer to the magnify, and point the layer's mirror rather than reflecting the gesture by hand.
- [x] 2.2 Take the magnitude from Intensidade and gather by default, and turn the sign over on the invert key, which spreads.
- [x] 2.3 Leave the dab standing on the surface where the gesture's raycast put it, which is what makes the scale a gather.
- [x] 2.4 Lay one dab per step of the brush's spacing along the path, so a pointer resting still does not pile frames at one centre.
- [x] 2.5 Drop the frozen samples before any dab is placed, since the engine's descriptor has no gate.
- [x] 2.6 Wrap the gesture's dabs in one undo group.
- [x] 2.7 Invalidate the dab's own ball with no dilation, once per image the layer's symmetry makes of it.

## 3. Leave Inflate where it is, and say what Standard costs

- [x] 3.1 `Inflar` keeps `clay_layer_apply_stroke (CLAY_OP_RELIEF)` and its wider region and shallower lift. ClayCore v0.120.0 measures relief to be the Inflate frame (#615, #618), so the relief binding is the faithful one and the radial scale is not an improvement on it.
- [x] 3.2 `ToolNote::SdfStandardIsAnInflate`: `Padrão` on a field is a Standard approximated by relief — a few percent of the amplitude where the form is smooth at the brush's scale, the whole amplitude on a feature narrower than the stamp.
- [x] 3.3 The note has a name in every locale, beside the other three.
- [x] 3.4 `crates/clayspace-engine/tests/table_truth.rs`: `a_field_standard_thickens_a_fin_and_not_a_sphere` measures the fin-against-sphere contrast the engine's notes draw, reading the field itself rather than a mesh, and `every_tool_note_is_proved_here` names it.

## 4. Hold it

- [x] 4.1 `crates/claycore/tests/sculpting.rs`: a magnify swells every item of a blended form; the sign is magnify against pinch; the frames of one gesture fold into one warp; a preview names the nodes and touches nothing; a strength or a radius of zero is refused.
- [x] 4.2 `crates/clayspace-engine/tests/sdf_magnify.rs`: the pinch gathers — the middle rises and the rim falls; inverting it spreads; a magnify across a blend moves both items; a gesture is one undo step; Pinçar is on the field's shelf.
- [x] 4.3 `crates/clayspace-engine/tests/sdf_brushes.rs`: Pinçar joins the field's surface brushes and its signed ones, and stays out of the depositing list — a gather is neither a deposit nor a cut.
- [x] 4.4 `crates/clayspace-model`: the field vocabulary is fifteen tools rather than fourteen.
- [x] 4.5 The two places that used Pinçar as their example of a tool a field has no verb for — `crates/clayspace-engine/tests/voxel_tools.rs` and `describe_offers_only_the_tools_the_layer_has_a_verb_for` — name Apagar instead, whose only verb is the grid's. Both assertions stand; only the example moved.

## 5. Say so

- [x] 5.1 `docs/features.md`: the Pinçar row, the count, the measurement behind the strength, what the invert key does, and the mask and warp-accumulation notes. Remove the "SDF Pinçar" gap, which is closed.
- [x] 5.2 `docs/features.md`: what relief is faithful to, the fixture table from ClayCore v0.120.0, and what Padrão's note tells a sculptor.
- [x] 5.3 `docs/roadmap.md`: ClayCore #391 is taken up rather than waiting upstream, the gesture-region note names two callers rather than one, and the faithful Standard is recorded as measured-and-not-shipped rather than as a gap.
- [x] 5.4 `docs/features.md`: the agent surface's example of what a field does not offer is `erase` rather than `pinch`.

## 6. Keep the file the same on both platforms

- [x] 6.1 Move the canonical fixture out of the binary into `clayspace_app::canonical`, and say there which verbs may stand in it: only the ones whose record is the ask itself, since a resolved warp and a baked volume are both floats the engine worked out and neither is the same across toolchains.
- [x] 6.2 `crates/clayspace-app/tests/canonical_document.rs`: every position the fixture stamps is in the saved bytes verbatim — the matrix's property, asked on one machine. The fixture keeps the verbs it had, Inflar included, since a relief stroke records the stamp it was asked for.

## 7. Everywhere else that knew the old shelf

A field offering a fifteenth brush is a fact several files had written down by
hand. Swept in one pass rather than one CI run at a time.

- [x] 7.1 `crates/clayspace-app/tests/visual_sdf_symmetry.rs`: Pinçar has a name in the file-name match, so the symmetry sweep measures it instead of panicking on it.
- [x] 7.2 `crates/clayspace-app/tests/visual_brushes.rs`: Pinçar joins `STAMPING`. The list groups by the shape of the cost, and a magnify dirties the ball under the brush — leaving it out would have filed it as bake-and-replace and given it the loose fence.
- [x] 7.3 `crates/clayspace-app/tests/visual_field_stroke_quality.rs`: the roughness table carries Pinçar at 1.07, and every other reading is unchanged.
- [x] 7.4 `crates/clayspace-app/src/main.rs`: the refusal test and the two substitution remarks name Raspar, which a field really has no verb for, rather than Pinçar.
- [x] 7.5 `crates/clayspace-mcp/src/catalogue/table.rs`: the narrowing's example is `erase`, matching the test that checks it.
- [x] 7.6 `crates/clayspace-app/tests/agent_end_to_end.rs`: the count of tools with no SDF verb is the current one, with what #127 said kept beside it.
- [x] 7.7 `README.md`: fifteen have an SDF verb, the SDF shelf's caption lists Pinch, and ClayCore #391 moves out of the things blocked upstream.
- [x] 7.8 `docs/features.md`: the mesh-accumulation table no longer says Pinçar is voxel and mesh.
