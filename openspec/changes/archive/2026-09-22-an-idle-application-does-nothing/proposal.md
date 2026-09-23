# An idle application does nothing

An application with a worked document open and nobody touching it sat at about
185–200% CPU. Profiling put **83% of main-thread samples** inside
`BrickCache::surface_bricks` — a walk of every stored brick, on a document that
nothing was changing.

Nothing asked for that walk. The status area's memory meter did, sixty times a
second, and did not know it was asking for one.

## What went wrong

`BrickCache::stats` reads like a counter and is not one. The engine's C binding
fills the `surface_bricks` field by building a vector of every stored key and
taking its length:

```cpp
filled.surface_bricks = cache->cache.surface_bricks().size();
```

So one call is one allocation and one walk of the whole cache, at a cost
proportional to the sculpture and paid whether or not anything changed. The
redraw path called it on every frame to fill in two numbers beside a progress
bar, and the agent's state report called it again for the budget — a constant
it reads back from the cache's configuration.

This is also why the figure was noise in every measurement taken of this
application: the meter that reports what the document costs was the largest
single thing on the idle main thread.

## What this changes

- **The meter is read on a clock rather than per frame.** At most one reading a
  second, from a small memo that holds the last figures and the moment they
  were taken. A meter a second behind reads the same to a person as an exact
  one, nothing in the application derives anything from the figure, and one
  walk a second is a cost no sculpture can make matter.
- **Opening another document takes a fresh reading at once**, rather than
  showing the closed document's bytes against the new document's name for the
  rest of the second.
- **The agent's state report reads the same meter.** The comment there already
  said it reads memory the way the status area does, so that an agent and a
  person cannot disagree; now it does, and an agent polling the report cannot
  put the per-call walk back.
- **A reading that fails keeps the last figures.** A cache that cannot answer
  has not thereby freed its memory, and a meter that fell to zero on an error
  would say it had.

## What this deliberately does not do

**It does not make `stats` cheap.** The walk is ClayCore's, behind a pinned
engine, and the field that costs it is one this application reads two numbers
past. Upstream is the right place for that; until then the fix here is to stop
asking sixty times a second for an answer that has not changed.

**It does not add a revision counter to the brick cache.** The cache exposes
nothing of the kind across the ABI, so the counter would have to be maintained
on this side and bumped at every mutation site — and a site missed is a meter
that silently stops moving. A clock cannot be wrong that way: the worst it can
be is a second late, which is what the requirement now says out loud.
