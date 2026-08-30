# An undo costs the edit, and a costly subtool says so

Two things ClayCore already offers and this application was not asking for.
Both were found by reading the integration guide's recommendations against the
code; both turned out to be nearer than the guide thought.

## An undo cost forty times the edit it reversed

Taking a dab back re-meshed the whole active layer, because that was the
narrowest region there was to name: the engine reverted whatever it reverted
and would not say where. `undo_step` said so in a comment — *"Undo can move
anything the layer holds, so the bound is the layer rather than a node set"*.

`clay_document_undo_bound` / `_redo_bound` report the world box of what the
step applied. They have been in the ABI since 0.40.0 and arrived here with the
0.60.0 pin; nothing bound them. The integration guide files this as a **P0
upstream ask**, which is out of date — it is ours to take, and taking it is one
call and a `match` on three states the wrapper already models as `Influence`.

Measured on the same fixture, before and after, moments apart — 1045 surface
bricks after 96 edits:

| | keys | engine | sync |
|---|---:|---:|---:|
| a dab | 18 | 0.76 ms | 7.49 ms |
| undoing it, before | 1045 | 24.36 ms | **273.69 ms** |
| undoing it, after | 18 | 0.94 ms | **8.63 ms** |

**31.7x less time in the sync, 58x fewer keys.** An undo now costs what the
edit cost, which is what it always should have.

**And it fixes a bug rather than only a cost.** The bound is a *world* region
where the old fallback was the *active layer's*, so undoing an edit made on a
different subtool re-meshed the wrong one and left the changed surface stale on
screen — the undo appeared to do nothing.
`undo_region.rs` is that case.

## The engine says when a subtool has become costly, and nothing listened

A chain of edits steepens the field it produces until a march takes many small
steps and every dab pays for it. The engine measures that and advises
collapsing the layer; collapsing it took a dab from 56 ms to 13 ms in this
repository's own measurements.

`LayerCost` carried that advice from the engine, through the adapter, into the
domain — and **no ViewModel or panel ever read it**. `consolidate_layer`, the
action behind it, was reachable only from a benchmark.

Part of the reason is measurable. `layer_cost` asks two questions at once, and
they are four orders of magnitude apart:

| | on a 97-item layer |
|---|---:|
| the advice (`clay_layer_field_report`) | **33 µs** |
| what collapsing would occupy | **287 ms** |

So the scene — assembled on every refresh — now carries the cheap half as
`FieldHealth`, and the estimate stays where it was, for the moment a sculptor
is deciding. `field_health.rs` holds the two apart, because putting them back
together is exactly the mistake that kept this feature off the screen.

The interface offers and never acts: consolidation costs seconds and changes
what the layer holds, so it is the sculptor's decision. The row appears only
while the engine is advising.

## What this deliberately does not do

The guide's other items — an off-thread mesh weld, per-chunk voxel GPU buffers,
a `GeometryChanges` contract, a frame-budget scheduler — are either already
recorded in the roadmap as known costs with escape routes, or refactors with no
measurement behind them. They are not taken here.
