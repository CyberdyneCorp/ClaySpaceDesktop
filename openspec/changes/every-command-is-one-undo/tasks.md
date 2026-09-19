## 1. One way to bank, and one reason written down

- [x] 1.1 Add one accounting type to the ViewModel layer that records what an edit cost from the history depth either side of it, hands the counts over once, and carries the reason in one place.
- [x] 1.2 Move the mask ViewModel's hand-rolled version onto it, so there is one rule rather than a copy per ViewModel.
- [x] 1.3 Ask the model for the history depth through each ViewModel's own interface, so no ViewModel reaches past it.

## 2. The commands that were banking nothing

- [x] 2.1 The cage: bank applying it, and nothing for putting one up, dragging its points or taking it down.
- [x] 2.2 Placed forms: route every write in the object ViewModel through one function that banks, and bank a manipulator drag once for the whole gesture, measured from the press.
- [x] 2.3 The layer stack: route add, remove, consolidate, rename, reorder, protection and rebuild through one function that banks, and leave the three viewing commands and the hierarchy's stack out of it.
- [x] 2.4 The boolean: bank the resolved run.
- [x] 2.5 Collect every ViewModel's counts at the composition root, once per command, and beside the few operations the composition root runs directly.

## 3. The gesture's own commit

- [x] 3.1 Bank a committed gesture by the distance from the depth it opened at, as its cancel already reverts by, and retire the per-segment count.

## 4. Hold it

- [x] 4.1 `crates/clayspace-vm`: one test per command asserting a single banked action; a refused one banks none; a cage that bent nothing banks none; a drag banks once for the gesture and nothing per frame; looking at the scene banks nothing.
- [x] 4.2 `crates/clayspace-vm/tests/viewmodel.rs`: a committed gesture is one undo on a double that banks the whole gesture as one record, and still spends every entry on one that records per segment; a gesture that wrote nothing banks nothing.
- [x] 4.3 `crates/clayspace-app/tests/structural_undo.rs`: through the ViewModels and the banking seam the composition root uses — each command is one step of the reported history, one undo after a bend keeps every subtool, and a scripted session undoes one command at a time without removing a subtool the undone command did not create.

## 5. Say what changed

- [x] 5.1 `docs/features.md`: the history section states the rule and what the per-segment count cost; the mesh-gesture section is corrected where it described the old arithmetic.
