## 1. The way back from a pass operation

- [x] 1.1 Give the document a record of what it takes to undo and redo one operation on a grid's passes, stamped on the history sequence and interleaved with the engine's entries by it.
- [x] 1.2 Order the two records that cost the engine nothing — a mesh gesture and a pass operation — against each other as well as against the engine's top entry.
- [x] 1.3 Apply a pass operation through one function that takes no account of the history, so a step through the history reuses it rather than banking a second record.
- [x] 1.4 Let go of the records for a grid whose stack has been renumbered by a removal or a merge, and bound the stack so it does not grow for the life of a session.
- [x] 1.5 Count the records in the depth the interface reads and in whether Undo is offered.

## 2. One way in, and a count out of it

- [x] 2.1 Route the grid's pass operations through the scene ViewModel's one function that runs an operation and records what it cost, and measure the rebuild the same way beside its own refusal.
- [x] 2.2 Read the cost from the history either side rather than assuming one entry, and record nothing where the operation wrote nothing.
- [x] 2.3 Hand the counts to the ViewModel that owns Cmd+Z through the composition root's one drain, one count per operation, taken once.
- [x] 2.4 Ask the model for the history depth through the scene interface, so the ViewModel does not have to reach past it.
- [x] 2.5 Run a grid's pass operations through the scene ViewModel, so the refusal lands on the line the interface already shows.

## 3. A rebuild under an open gesture

- [x] 3.1 Refuse a rebuild while a gesture is open, in a sentence that names the stroke, before anything the rebuild would drop is dropped.

## 4. Hold it

- [x] 4.1 `crates/clayspace-vm/tests/scene.rs`: a rebuild and each pass operation bank exactly one action; opening a recording banks none; a refused operation banks none; the count is handed over once.
- [x] 4.2 `crates/clayspace-app/tests/pass_and_rebuild_undo.rs`: through the ViewModels and the banking seam the composition root uses — dialling a pass and rebuilding are each one step of the history the interface reads, one undo puts each back, and neither takes a subtool with it.
- [x] 4.3 `crates/clayspace-engine/tests/undo_ordering.rs`: the digest carries every grid's pass stack; each pass operation and a rebuild apply-and-undo back to the document they started from; a pass and an engine edit come apart in the order they were made; a rebuild during a gesture is refused and changes nothing.
