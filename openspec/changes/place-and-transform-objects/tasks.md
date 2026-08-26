## 1. The bridge reaches the items

Checked before starting: the bridge already wraps `clay_layer_set_transform`
(`set_node_transform`), `clay_remove_node`, `clay_layer_node_count` / `_at` /
`_children` (`layer_nodes`), `clay_layer_node_prim` (`node_prim`),
`clay_document_set_layer_transform` (`set_layer_transform`) and
`clay_raycast_attributed` (`raycast_attributed`, already reporting the layer
and node on `Hit`). Those tasks are struck rather than done again; what is
left is what is genuinely absent.

- [x] 1.1 Wrap the bounded primitives in `claycore`: a `Primitive` enum over the engine's finite shapes with the parameters each takes, and `Item::of(primitive)` beside the existing `Item::sphere`
- [x] 1.2 Wrap `clay_layer_set_transform` and `clay_remove_node` — already present as `set_node_transform` and `remove_node`
- [x] 1.2a Wrap `clay_layer_set_prim` and `clay_layer_set_op_blend`, which are not
- [x] 1.3 Wrap `clay_layer_node_count`, `clay_layer_node_at` and `clay_layer_node_prim` — already present as `layer_nodes` and `node_prim`
- [x] 1.4 Wrap `clay_raycast_attributed` — already present, and `Hit` already carries the layer and node
- [x] 1.4a Wrap `clay_layer_node_influence_bound`, including the unbounded answer it can give
- [x] 1.5 Wrap `clay_document_set_layer_transform` — already present as `set_layer_transform`
- [x] 1.5a Wrap `clay_mesh_transform`
- [x] 1.6 Test each new wrapper against a real document, including what the engine refuses: a group given a primitive, a scale of zero, a node the layer does not hold

## 2. An object in the domain

- [x] 2.1 Add `Primitive` to `clayspace-model` — the offered shapes, their parameters and their labels — with the unbounded three deliberately absent and a test that says so
- [x] 2.2 Add `SceneObject` and `ObjectSelection`: a layer, a node, the primitive it is, and the operation and blend it carries
- [x] 2.3 Add the objecthood rule — ~~an item is an object unless its primitive is stroke, swept or armature~~ — **reversed in implementation**: an SDF stamping stroke places `Item::sphere` per stamp, so the primitive cannot tell a stroke's spheres from a placed one and the rule would offer a row per stamp. Objecthood is recorded in the table the readback gap already forces, and `a_stroke_stamp_is_indistinguishable_from_a_placed_sphere` holds the reason
- [x] 2.4 Extend `SculptModel` (or a new `ObjectModel` trait beside it) with place, select, retransform, re-op, re-prim and remove, provided so a double refuses them rather than spelling out a refusal

## 3. Objects in the document

The ABI sets a node's transform, parameters and operation and reads none of
them back, so the application mirrors them — see design.md, *The application
mirrors the state, because the engine will not say*. Tasks 3.9 to 3.12 are
that mirror and would not exist if the engine had three getters.

- [x] 3.1 Implement placement on `ClayDocument`: add the item, mark its influence bound dirty, return the node, and refuse on a voxel or mesh layer with a reason naming what an object needs
- [x] 3.2 Implement the object list for the active layer, derived by walking the layer's nodes and testing each primitive
- [x] 3.3 Implement retransform: set the transform, dirty the union of the influence bound before and after, refill
- [x] 3.4 Implement re-op, re-prim and removal, each dirtying what it touched
- [x] 3.5 Implement picking through `clay_raycast_attributed`, returning the layer and node so a subtract item is selected by the surface it carved
- [x] 3.6 Bracket a drag in one undo group, as `convert_layer` does, so a gesture is one entry
- [x] 3.7 Handle a node with no finite influence bound by dirtying the layer rather than by assuming a box — reached by an `Intersect` object as much as by an unbounded primitive
- [x] 3.8 Test: a placed object survives save and reopen with its transform and operation (waits on 3.10); a moved subtraction moves its cavity, a removal restores the surface, and each is one undo step — done, 14 tests in `clayspace-engine/tests/objects.rs`
- [x] 3.9 Check the assumption the mirror rests on: a node id survives a save and a reopen, with a gap in the id space — `a_node_id_survives_a_save_and_a_reopen`
- [x] 3.10 Keep the object table — node id to shape, parameters, transform and combine — and write it beside the document, reading it back on open
- [x] 3.11 Pair the table with the engine's history: an object edit pushes its inverse, and undo and redo pop it in lockstep
- [x] 3.12 File `clay_layer_node_transform`, `clay_layer_node_params` and `clay_layer_node_op_blend` upstream, and hold the gap with a repro test as `claycore_repros.rs` holds the others

## 4. The manipulator outside the cage

- [x] 4.1 Give `GizmoHandle::all_for` the target as well as the mode, and return the centre alone in scale mode for anything the engine scales uniformly
- [x] 4.2 Add the target kinds — object, layer, mesh layer, curve points — and resolve a drag to a position, an axis-angle and a scale rather than to a point set
- [x] 4.3 Route an object's drag to `clay_layer_set_transform`
- [x] 4.4 Route a layer's drag to `clay_document_set_layer_transform`, and check the symmetry plane travels with it
- [x] 4.5 Route a mesh layer's drag to the layer transform, and use `clay_mesh_transform` only where a bake needs the moved vertices — a mesh layer *is* a layer, so `GizmoTarget::Layer` already carries it; the bake path is task 6
- [ ] 4.6 Extend the curve to turn and scale its selected points about their middle, reusing the drag maths the cage already has
- [ ] 4.7 Refuse a sculpting stroke as a target, with a message saying why rather than nothing happening
- [ ] 4.8 Test each target for the rules the cage already holds: the manipulator sits on the middle, an axis handle constrains, a wandering drag lands where it ends, a scale never passes zero

## 5. The interface

- [ ] 5.1 Add the insert command and a primitive picker, presenting the bounded shapes with their parameters
- [ ] 5.2 Place at a stated position — under the pointer on the surface where there is one, at the view's focus where there is not
- [ ] 5.3 Add the object list to the scene panel, showing placed objects and not showing a row per stroke
- [ ] 5.4 Make selection agree in both directions between the list and the viewport
- [ ] 5.5 Present the selected object's operation and blend in the options bar, editable, with the distance control refusing zero for the operations that need one
- [ ] 5.6 Draw the manipulator on the selection, with the mode on the keys the cage already uses
- [ ] 5.7 Draw a placed object legibly against the surface it combines with, so a subtracted object inside the form can still be seen and grabbed
- [ ] 5.8 Localise every new string in all three locales
- [ ] 5.9 Capture the manipulator on each target kind and the picker, and look at them

## 6. Custom objects as operands

- [ ] 6.1 Offer the conversion where a mesh layer is chosen as an operand, reusing the conversion panel's own cost computation
- [ ] 6.2 Leave the source mesh layer untouched and add the converted layer to the stack, as a conversion already does
- [ ] 6.3 Refuse nothing silently: declining leaves no layer, no boolean and no change
- [ ] 6.4 Test that the costs stated on use are the same figures the panel computes for that crossing at that resolution

## 7. Performance

- [ ] 7.1 Add an `object` group to the benchmark: place, retransform, re-op, remove and pick
- [ ] 7.2 Add `object.drag_frame` — one frame of a live boolean drag, which is the figure that decides whether this is usable — and `object.drag_frame_intersect` beside it, since a non-local op has no finite influence bound and dirties the whole layer
- [ ] 7.3 Implement the settle-on-release fallback where a drag frame exceeds the interactive budget, and check the drag stays responsive on the ten-times scene
- [ ] 7.4 Re-record the Linux baseline with the new figures, in its own commit

## 8. Documentation

- [ ] 8.1 Answer `docs/features.md`'s *Not built yet* entry for the manipulator outside the cage, and narrow its *Deliberately absent* entry for mesh-surface booleans to what remains true
- [ ] 8.2 Document the object workflow in `docs/features.md`: placing, the operations an object can carry, and what the manipulator does to each kind of target
- [ ] 8.3 State plainly that scale is uniform and why, and point at the cage for the other thing
- [ ] 8.4 Update `README.md`'s input table for the new gestures
- [ ] 8.5 Run `just check`
