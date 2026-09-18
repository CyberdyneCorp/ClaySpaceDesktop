# Drain the engine's maintenance queue between two strokes

## Why

ClayCore v0.78.0 ships `clay_maintenance_queue_*`: a host's list of work that
makes the *next* interaction cheaper and *this* one slower — index rebuilds,
chunk compaction, detail promotion, slot-pool compaction, a deferred normal
flush. Nothing rebuilds on the engine's own behalf. What is queued is a
**request**, because upstream's own `Bvh::quality` measurements found a rebuild
helped one of five measured deformations and hurt two, so the decision is the
host's and the moment is the host's.

**This application has been throwing away the one measurement that decides it.**
`clay_mesh_sculptor_quality` has been wrapped since the mesh sculptor was, is
exposed as `ClayDocument::mesh_quality`, and is read by two tests and nothing
else. Meanwhile `MeshSculptor::refresh` — the rebuild — is wrapped and called
from nowhere in the workspace, while `refit` is called at four sites. So the
ray-query tree behind every mesh pick and every mesh dab is refitted forever and
its partition is never rebuilt, and the number that says it should be is
computed and dropped. That is `CLAY_MAINTENANCE_INDEX_REBUILD` in everything but
the entry point that asks for it.

**The two moments already exist.** `SculptModel::end_gesture` is reached by both
the pointer coming up and the gesture being cancelled; `apply_lattice` and
`cancel_lattice` are the same pair for the cage, which is a second gesture
lifecycle that never goes through `begin_gesture`. What did not exist is
anything holding those two moments together with the flag that says a gesture is
open — `previewing` was assigned from five separate places.

**A trim mid-drag is priced now rather than guessed at.** `bench_trim_recovery`
reports 0.62–2.04x at Warning and 13–182x at Critical, growing with the model,
which turns "prefer Warning mid-drag, or hold a pin until the stroke ends" into
advice a host can act on. `clay_memory_pin_*` is how a host acts on it.

## What changes

- `claycore` gains `MaintenanceQueue::into_stroke` and `StrokeScope`: the same
  gate as the borrowing `StrokeGuard`, for a host whose stroke is a *field*
  rather than a block. An interactive gesture opens on a press and closes on a
  release that arrives as its own event, so there is no scope for a borrowing
  guard to live in — and a guard held in a field beside the queue it borrows is
  a shape safe Rust does not have.
- `ClayDocument` holds a maintenance queue, and `previewing` is written from one
  place, because two things now follow it exactly: the queue's gate and a
  memory pin.
- Every path that writes through a mesh sculptor asks for that layer's tree to
  be rebuilt. Asking is a fold — a drag asks on every segment and the queue
  keeps one entry, counting the asking.
- Every way a gesture ends drains the queue against a budget of 8 ms. Whether a
  tree has actually decayed is read *there*, once, against the figure the same
  tree scored when it was built — never against an absolute number, which the
  engine says means nothing across two models.
- What a rebuild costs is learned by paying for one. The engine carries no
  machine model and says so, so the first request carries no estimate and is
  timed; every one after it is weighed against what this machine measured.
- `ClayDocument::mesh_quality`'s documentation is corrected. It claimed to
  report how stretched a mesh's triangles are; the engine's figure is the
  expected number of triangle tests a random ray must make.

## What does not change

The other four kinds have no producer here. Chunk compaction, detail promotion
and slot-pool compaction belong to an adaptive surface and a hierarchy, neither
of which this application holds. Deferred normals are not queued either: a mesh
gesture owes its flush to the handle that deferred it and `LiveMesh` settles on
`Drop`, which is a stronger guarantee than a queue entry, because the flush is
the one item that is not optional. An item of a kind nothing here produces is
still *completed* rather than left, so that a head item nobody will ever service
cannot block what is behind it.

Nothing trims yet, so the pin protects nothing today. It is held anyway, and
`ClayDocument::memory_pin` is the only way to obtain one, so the first trim
written cannot be written without one to hand it.
