# Route layer reorder and layer protection

## Why

`scene-and-layers` has required both since `add-clayspace-desktop`:

> The user SHALL be able to create, rename, reorder and remove layers.

> The application SHALL expose the engine's three protection states: visible,
> ghosted (shown, not pickable, not editable) and locked (shown, pickable, not
> editable).

`SceneViewModel` implements both — `reorder` at `crates/clayspace-vm/src/scene_vm.rs:95`
and `set_protection` at `:80`, each going through `SceneModel` and `finish`, each
tested in `crates/clayspace-vm/tests/scene.rs`. **Nothing else calls either
one.** There is no `Command` variant for them, so there is no route from a
click and no route from an agent:

- `crates/clayspace-vm/src/command.rs` carries `SelectLayer`, `SetLayerVisible`,
  `SoloLayer`, `AddLayer`, `RemoveLayer`, `OptimizeLayer`, `RemeshLayer` and the
  four rename commands. No `MoveLayer`, no `SetLayerProtection`.
- `crates/clayspace-view/src/shell/left.rs:277` *draws* the ghost or lock badge
  on a row that has one, and the badge queues nothing when it is pressed. A
  sculptor can see that a layer is protected and cannot make it so.
- The layer stack has no drag-to-reorder. The hierarchy's pass rows do
  (`multires_pass_row`, `:465`), which is why the absence reads as an oversight
  rather than a decision.
- The agent door reaches whatever the command path reaches, by the rule in
  `agent-control`, so it reaches neither.

The audit behind #197 found this while comparing behaviour against the specs.
The choice the issue put was to route it or to drop the requirement. **Dropping
it would be wrong**: protection is not decoration, it is the only thing standing
between a stroke and a reference layer the sculptor meant to keep, and the
engine, the model and the ViewModel all already carry it. What is missing is the
last hop.

This change does not implement that hop. It records the decision — route it —
and states what the route has to be, so the work has a spec to be built against
instead of a requirement that has been true on paper for five months.

## What Changes

- **Two commands**, `MoveLayer(LayerKey, usize)` and
  `SetLayerProtection(LayerKey, Protection)`, dispatched by
  `SceneViewModel::reorder` and `::set_protection` — which is why this is a hop
  and not a feature.
- **The stack row carries both.** The ghost and lock badges become controls
  rather than indicators, and a layer row is dragged to a new position the way a
  hierarchy's pass row already is.
- **Both reach the agent door** by existing, because `agent-control` requires
  every tool to dispatch the command path the interface dispatches.
- **Both are one undo step**, as every other layer mutation is.

## What this change does not cover

Solo, visibility and the hierarchy's own per-pass lock are already routed and
are left alone. This is the layer stack's ghost, its lock and its order.
