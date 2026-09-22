## 1. One way in, and a count out of it

- [x] 1.1 Add one function to the mask ViewModel that applies an edit and records what it cost, and route the menu's operations, the outline and the extrusion through it.
- [x] 1.2 Read the cost from the history either side of the edit rather than assuming one entry, and record nothing where the edit wrote nothing.
- [x] 1.3 Hand the counts to the ViewModel that owns Cmd+Z at the composition root, one count per edit, taken once.
- [x] 1.4 Ask the model for the history depth through the mask interface, so the ViewModel does not have to reach past it.

## 2. The amount a caller named

- [x] 2.1 Apply the amount the command carries rather than the panel's, since the menu already fills the panel's in before dispatching.
- [x] 2.2 Bring an amount outside what the engine takes inside it, by the same bounds the panel's own control uses, and say so.

## 3. Clearing nothing

- [x] 3.1 Return from the engine's clear before the snapshot and before the revision when nothing is frozen.
- [x] 3.2 Say that there was nothing to clear as a remark rather than a refusal, and carry the remark to the agent door beside the substituted-tool one.

## 4. What the viewport and an agent are told

- [x] 4.1 Bump the mask revision after a history step that moved, so a restored region is redrawn.
- [x] 4.2 Report on the wire whether anything is frozen rather than whether a mask field exists.

## 5. Hold it

- [x] 5.1 `crates/clayspace-vm`: every operation banks exactly one action; a refused one banks none; a clear over an empty mask banks none and says so; the amount the command carries is the one applied.
- [x] 5.2 `crates/clayspace-app/tests/mask_undo.rs`: through the ViewModels and the banking seam the composition root uses — each operation is one step of the history the interface reads, and one undo after a mask edit leaves the layer set and the clay alone.
- [x] 5.3 `crates/clayspace-engine/tests/undo_ordering.rs`: the digest carries the mask, and every operation, an outline and an extrusion each apply-and-undo back to the document they started from; a redo puts the operation back; clearing an empty mask costs neither an entry nor a revision.
