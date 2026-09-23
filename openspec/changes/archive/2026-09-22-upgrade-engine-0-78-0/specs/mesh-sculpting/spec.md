## ADDED Requirements

### Requirement: A mesh segment recomputes its normals once, not once per dab
A segment of a mesh stroke is several engine calls — one for each enabled mirror,
and inside each of those a resolved stroke's own stamps — and a normal recompute
per stamp does the same vertices over and over wherever the dabs overlap. The
application SHALL defer the recompute across a segment and take it once,
coalesced.

**The final surface and the undo record SHALL be exact either way.** Deferral is
a rearrangement of work and never of the result: after the segment, the mesh
SHALL shade from the vertices it actually holds, and undoing the gesture SHALL
restore both the positions and the shading the form had before it.

Nothing in the engine flushes a deferred recompute on its own, and it cannot: the
sculptor does not know where a stroke ends, and guessing would flush mid-drag,
which is the cost deferring exists to avoid. So the flush SHALL be structural
rather than written at the end of each path that ends a stroke. The record a
segment's stamps are noted into and the sculptor that owes the recompute SHALL be
held as one value whose disposal recomputes, so that **every** way a gesture can
end settles: a normal commit, a cancel, a tool or subtool changed mid-drag, an
undo mid-drag, a refusal unwinding out of the middle of a segment, and the
document going away underneath it.

The flush SHALL be handed the same record the stamps were noted into and no
other. A record captures a vertex's normal the first time it sees that vertex, so
a flush into a fresh record would capture the already-moved normals as the
"before" and the undo would put the vertices back while leaving the shading where
the stroke wrote it.

#### Scenario: A committed gesture leaves no stale shading
- **WHEN** the sculptor makes a mesh stroke and releases the pointer
- **THEN** no vertex the stroke moved is left with the normal it had before the
  stroke

#### Scenario: A gesture abandoned mid-drag still settles
- **WHEN** a mesh gesture is ended by something other than a normal release —
  cancelled, interrupted by a change of tool or subtool, undone mid-drag, or
  ended by the document being replaced under it
- **THEN** the normals the segment deferred are recomputed anyway

#### Scenario: Undo restores the shading as well as the shape
- **WHEN** the sculptor undoes a mesh gesture whose normals were deferred
- **THEN** the vertices and their normals are both back to what they were before
  the gesture

### Requirement: A stamp is told which numbering its pick was made in
A mesh brush walks the surface outward from a weld class, and it can either be
told which class to start from or search for one, which the engine states is a
linear scan over the mesh and the wrong thing to do per stamp on a large one. The
application already picks — the pick that placed the cursor hit a triangle and
knows the answer — so it SHALL carry that class into the stamps that follow.

**A class SHALL never be carried without the token of the numbering it was picked
in.** A weld class is an index into a numbering a sculptor built, and this
application retires sculptors constantly: an eviction from its cache of four, a
removed subtool, an undo's reconciliation, a rebuild that replaces every triangle
deliberately. Each hands back a new numbering, in which an index from the old one
is comfortably in bounds — so nothing refuses it, the surface walk starts
somewhere else and returns empty, and the stamp does nothing at all, which looks
exactly like a stroke over a frozen region. With the token beside it the engine
refuses the seed and the stamp falls back to the scan it would otherwise have
done: one stamp slower, and correct.

A seed SHALL also be withheld wherever it cannot be shown to help, which is a
question the engine does not ask on the host's behalf. The surface walk abandons a
seed lying farther than the stamp's own radius from its centre, so a valid seed
handed to a stamp that has travelled past that radius loses the dab exactly as a
stale one does. The application SHALL therefore withhold the seed for a mirrored
copy, for a stamp whose centre has left the picked point's reach, and wherever the
stroke's own settings could shrink a stamp below the radius the reach was measured
against.

#### Scenario: A stroke after a rebuild is not silently lost
- **WHEN** a pick is made, the mesh under it is rebuilt so that the sculptor and
  its numbering are replaced, and a stamp is then made where the pick landed
- **THEN** the stamp moves the surface, and the rejection is counted rather than
  being invisible

#### Scenario: A stamp out of the pick's reach falls back rather than missing
- **WHEN** a stroke travels far enough from the picked point that the seed could
  no longer reach the stamp
- **THEN** the stamp is made without a seed and still lands

#### Scenario: A mirrored stamp is not seeded from the original's pick
- **WHEN** a symmetric stroke deposits a mirrored copy of a stamp
- **THEN** the mirrored stamp carries no seed, because the picked class is on the
  other side of the form
