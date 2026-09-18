## 1. A sequence the document owns

- [x] 1.1 Add a monotonic `history_seq` to the document, and a stamp per entry the engine holds — one stack of stamps mirroring the engine's undo side, one mirroring its redo side.
- [x] 1.2 Reconcile the stamps against the engine's depth before every ordering decision and before every stamp handed out, so an entry noticed at that moment is stamped below the record stamped immediately after it.
- [x] 1.3 Move stamps between the two stacks on every path that steps the engine — the plain step, a crossing's several, and the visibility hop — so a shorter stack always means the engine dropped entries of its own accord.

## 2. Records that carry a stamp

- [x] 2.1 Replace the recorded depth on a gesture over carried geometry with a stamp of its own: it costs the engine no entry, so it is the newer thing exactly when its stamp is above the engine's top one.
- [x] 2.2 Replace the recorded depth on a crossing with the stamp of the entry it left on top, which is what names the crossing as *that* entry.
- [x] 2.3 Replace a visibility batch's recorded depths with the stamps of the entries it made. A batch *is* those entries and borrows their stamps rather than taking one of its own.
- [x] 2.4 File the object table under the stamp of the entry it was recorded on top of, rather than under the depth — a depth is reached again by any later edit.

## 3. One question, asked once

- [x] 3.1 Add one function that says which history holds the newest thing, and its mirror for the next thing forward.
- [x] 3.2 Rewrite `undo_step` and `redo_step` to ask it once. Drop the mesh question asked around the visibility hop, which existed only to break a tie the depth could not.
- [x] 3.3 Guard the hop with the same function, so a gesture newer than the entries a solo left is not hopped over.

## 4. What an entry landing means

- [x] 4.1 Drop every record on the document's redo side when a new engine entry is noticed, the way the engine drops its own.
- [x] 4.2 Drop the crossings and visibility batches whose engine entry the engine has evicted, and leave the carried gestures — which were never engine entries — alone.

## 5. Hold it

- [x] 5.1 `tests/undo_ordering.rs`: one undo after a stroke on carried geometry takes the stroke back and leaves the layer set alone.
- [x] 5.2 A run of commands undoes one at a time, each landing on the document as it stood before the command it was meant for.
- [x] 5.3 A gesture taken back before a new edit is not put back after it.
- [x] 5.4 Apply a run, take it all back, put it all forward: the document is where the run ended.
- [x] 5.5 The reported depth rises for every command that changes the document, and not for a solo.
- [x] 5.6 In-crate: a stamp is handed out once; an evicted entry drops its record; an eviction leaves the carried gestures standing. In-crate because the engine's history budget is not reachable from the bound ABI.
