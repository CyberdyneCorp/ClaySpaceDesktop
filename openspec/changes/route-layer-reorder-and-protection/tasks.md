# Tasks

## 1. The commands

- [ ] 1.1 `Command::MoveLayer(LayerKey, usize)` and
      `Command::SetLayerProtection(LayerKey, Protection)`, each dispatching the
      `SceneViewModel` method that already exists rather than reaching the model
      a second way
- [ ] 1.2 Both in the command label table, in the domain's own language, as
      every other layer command is
- [ ] 1.3 Both in the undo grouping the other layer mutations use, so one step
      back takes one reorder or one change of protection

## 2. The stack row

- [ ] 2.1 The ghost and lock badges become controls. Today they are drawn only
      where the state is already true — `left.rs:277` — which means the state
      cannot be reached from the row that displays it. Offer all three states
      from the row, showing which one holds
- [ ] 2.2 Drag a layer row to a new position, as `multires_pass_row` already
      allows for a hierarchy's passes. The layer stack's order *is* evaluation
      order, unlike a hierarchy's, so the drop target has to say what the new
      order evaluates to before the drop rather than after it
- [ ] 2.3 A refusal where one applies, in the words `Protection::refusal`
      already returns

## 3. The agent door

- [ ] 3.1 Nothing tool-side: `agent-control` requires every tool to dispatch the
      command path the interface dispatches, so both arrive by existing. Confirm
      that with a test through the door rather than by reading the rule

## 4. Tests

- [ ] 4.1 A reorder changes evaluation order and the viewport reflects it —
      the scenario `scene-and-layers` has carried unmet
- [ ] 4.2 A ghosted layer set from the row is not picked, and a locked layer set
      from the row refuses a brush with the stated reason
- [ ] 4.3 One undo takes back one reorder, and one takes back one change of
      protection
- [ ] 4.4 A guard that the two ViewModel methods have a caller outside the
      tests. The defect this change exists to fix was not that the code was
      wrong — it was that nothing reached it, and nothing said so

## 5. Write it down

- [ ] 5.1 `docs/features.md`: the layer stack section, which describes the
      protection states as something a sculptor sets
- [ ] 5.2 `openspec validate --all --strict`, and archive
