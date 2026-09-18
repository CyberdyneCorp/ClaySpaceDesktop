# Refusals reach the caller

## Why

The specification already says a tool is refused wherever the interface would
refuse, "for the same reason and in the same words". About ten command families
did not obey it, and the way they failed was the same each time: the refusal
went to `eprintln!` and the function returned nothing.

stderr is not a surface. No sculptor is looking at it, and the agent door —
which decides whether a command was refused by reading the channels the
interface would have written the sentence on — saw nothing written and answered
a plain success. Measured, twice per family:

```
repair close_holes {}          # a voxel-only verb, on an SDF layer
-> {"touched_document": true, …}
   app.log: "a operação foi recusada: applies to voxel layers; this one is SDF"

convert set {direction:'field-to-grid', cell_size:0.0005}; convert run {}
-> success
   app.log: "needs 1658880000 cells, past the 512 MB budget"
```

The document was byte-identical in both cases, and the answer said the
opposite. An agent building on that answer goes on to its next step against a
document it believes it changed.

The families share one cause. A handful of operations are run by the
composition root rather than dispatched to a ViewModel — a repair, a crossing,
a pass of the active layer's stack, a rebuild — because each has an answer to
carry back rather than a `Result<(), _>` to dispatch. None of them had an
Observable at all, so there was nowhere for the refusal to go that either
reader could see. Two more dropped the outcome with `.is_ok()` and did not even
print it.

## What changes

The composition root gets one channel for the operations it runs itself, and
every one of them writes its refusal to it. The options bar draws that channel
like the five it already draws; the door counts it like the five it already
counts.

- **A refused operation is an error at the door**, carrying the refusal
  sentence, with no `Applied` and so no `touched_document: true`.
- **A refused operation says why in the options bar**, on the same one line a
  refused save, rebuild, extrude or transform uses.
- **`.is_ok()` is gone** from the two hierarchy paths. The outcome is named and
  stated, so the silent branch is safe for a reason a reader can see rather
  than by accident of which ViewModel happens to announce.
- **The rule the door relies on is now a function with a test.** Which channels
  the door compares was a list inside a method on the composition root, and a
  refusal written anywhere but that list is one the door answers success to —
  which is exactly what happened. It is pulled out, so the list can be asserted
  on rather than reviewed.

## What this deliberately does not do

**It does not remove the 36 `eprintln!` calls in the composition root.** Most
of them are diagnostics nobody can act on — a device lost and rebuilt, a
profile that could not be written — and those belong on stderr. What changes is
that no refusal's *only* output is one.

**It does not find readers for the notices the other ViewModels write.** The
lattice, the curve and the cut each write a refusal to an Observable nothing
reads; the armature's goes to `eprintln!`; the boolean's is drawn in its own
panel and is not one of the channels the door counts. Every one of those is the
same defect wearing a different coat, and each is a change of its own —
this one gives the composition root's operations the channel they never had,
which is the half that covers the most commands.
