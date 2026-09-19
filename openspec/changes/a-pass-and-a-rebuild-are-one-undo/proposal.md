# A pass dialled and a rebuild are one undo

`edit-history` has required since the beginning that every operation changing
document state be undoable, and `voxel-sculpt-layers` says outright that
changing a recorded pass's strength and then undoing takes the strength change
back. Neither was true. A rebuild of a mesh layer's topology was not reachable
by Cmd+Z at all, and dialling a pass was not undoable in any sense — it moved
the surface and left nothing behind.

The history a sculptor presses is the sculpting ViewModel's: **a stack of how
many entries each action spent**, where one Cmd+Z pops one count and undoes
exactly that many. A stroke pushes its count, an armature gesture banks its
delta, a crossing and a grid repair were given theirs in `#149`, a mask edit in
`#152` — a rebuild and a pass operation pushed nothing.

## What went wrong

The two failed differently, and the difference is the whole of this change.

- **A rebuild is one engine entry that nobody banked.** `clay_document_voxel_-
  remesh_layer` is capture, rebuild, validate, replace, record: measured on the
  pinned engine, the undo depth rises by exactly one. So the entry sat there
  unclaimed, the next Cmd+Z popped the **previous** command's count and spent it
  here, and the one after that reached further still. Measured in the audit: the
  history depth was unchanged after `layer/remesh`, the undo after it removed
  subtools 48 and 49, and the redo restored none of them.
- **A pass operation is no engine entry at all.** Dialling, hiding or
  reordering a pass *recomposes* the grid — the passes above are reverted, the
  change applied, and they are replayed — and clay.h is explicit that a replay
  is not an edit. Measured: `set_strength` and `set_visible` each left the
  engine's undo depth exactly where they found it. There was nothing to bank,
  and the operation visibly moved the surface anyway.

And a rebuild was accepted **while a gesture was open**. An open gesture holds
an adjacency, a BVH and a `MeshDeltas` over the very triangles the rebuild
replaces: the rebuild landed, `clay_mesh_sculptor_flush_normals` and
`clay_mesh_deltas_revert` then failed against geometry that no longer existed,
and the band the gesture had drawn survived every undo afterwards.

## What this changes

- **The document keeps the way back from a pass operation itself**, the way it
  already keeps one for a mesh gesture, stamped on the monotonic sequence
  `#151` introduced and interleaved with the engine's entries by that stamp.
  Both directions are held, because a pass operation is not its own inverse:
  a strength is undone by the strength that was there before and redone by the
  one that was asked for.
- **A grid's pass operation goes through the one function on the scene
  ViewModel that runs an operation and banks what it cost.** That function and
  the `Unbanked` carrier behind it arrived with `every-command-is-one-undo`;
  what this change adds is the grid's stack of passes, so an operation added
  later cannot quietly arrive without an entry. What it cost is read from the
  history either side rather than assumed to be one, and an operation the model
  recorded nothing for banks nothing — which is what opening a recording is.
  A rebuild is measured the same way and banked once, beside its own refusal,
  because it answers with what it destroyed rather than with `Ok(())`.
- **A grid's passes are refused, stated, through the same channel every other
  scene refusal uses.** They used to go straight at the document from the
  composition root, and a second `begin_recording` was refused, printed to a
  terminal, and reported to the agent that asked as a pass that had been opened.
- **A rebuild while a gesture is open is refused with a sentence**, and leaves
  the layer byte-identical.

## What this deliberately does not do

**It does not make removing a pass or merging one down undoable.** Both destroy
the recorded diff they act on, and clay.h names them among the operations that
are not a step "because nothing records it" — there is no snapshot of a grid's
pass stack across the ABI, so nothing on this side can put a discarded diff
back. They are honest about it instead: they bank nothing, and they let go of
the ways back into that grid's stack rather than leave a record addressing a
pass by a position that has just been renumbered. Making them undoable needs
the engine.

**It does not stop a recompose replaying a stroke the history has taken back.**
A dab made while a pass is recording is recorded twice — once on the grid's
undo journal and once in the pass — and the engine states that as the cost of
having both. Undoing the dab reverts the cells and leaves the pass's copy, so
the next recompose brings it up again. That is the engine's, and it is the
reason dialling a pass is worth being able to take back rather than a thing
this change could fix on its own.
