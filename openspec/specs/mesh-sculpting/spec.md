# mesh-sculpting Specification

## Purpose
Sculpting an imported mesh layer's own vertices — the return trip that lets a
retopologized model be refined in place — under the guarantee that its topology
is never changed.

## Requirements

### Requirement: A mesh layer's vertices are sculptable
The application SHALL allow a mesh layer to be sculpted with the engine's
fixed-topology brushes. Each offered brush SHALL map to a documented engine
verb, as every other tool in the application does.

#### Scenario: A stroke moves a mesh layer's vertices
- **WHEN** the user strokes across an active mesh layer with a mesh brush
- **THEN** the mesh's vertices move and the viewport shows the result

#### Scenario: A mesh layer is pickable
- **WHEN** the pointer is over a mesh layer's surface
- **THEN** the brush cursor sits on that surface, and a press begins a stroke
  rather than orbiting

### Requirement: Sculpting a mesh never changes its topology
The application SHALL NOT create, split or delete a polygon while sculpting a
mesh layer. A mesh exported after sculpting SHALL carry the same indices, and
the same quads where it had them, as before.
The face connectivity SHALL remain identical even if the vertex and triangle
counts happen to stay equal after an edit.

#### Scenario: Indices survive a stroke
- **WHEN** a mesh layer is sculpted and then exported
- **THEN** its face indices are unchanged from before the stroke

#### Scenario: Every offered brush preserves adjacency
- **WHEN** every offered mesh sculpting brush is applied to a fixed mesh
- **THEN** the vertex count, triangle count and index connectivity remain unchanged

#### Scenario: Quads survive a stroke
- **WHEN** a mesh layer imported with quads is sculpted and exported
- **THEN** its quads are unchanged from before the stroke

### Requirement: Stretching is shown rather than prevented
The application SHALL report when sculpting has stretched a mesh's triangles
beyond a stated quality, so that a sculptor learns the mesh wants retopology
rather than discovering it at export.

#### Scenario: A heavy pull is reported
- **WHEN** a stroke stretches the mesh past the stated quality
- **THEN** the application reports the quality and names retopology as the
  remedy

### Requirement: A mesh layer's colour is editable where it has colour
The application SHALL offer the colour brushes on a mesh layer that carries a
colour attribute, and SHALL refuse them with a stated reason on one that does
not, rather than creating the attribute silently.

#### Scenario: Painting a coloured mesh
- **WHEN** the user paints on a mesh layer carrying colour
- **THEN** the vertex colours change and no vertex moves

#### Scenario: A mesh with no colour refuses
- **WHEN** the user selects a colour brush on a mesh layer with no colour
  attribute
- **THEN** the brush is unavailable and states that the mesh carries no colour

### Requirement: Mesh deformers act on the whole form
The application SHALL offer taper, twist and a lattice cage on a mesh layer as
operations on the form rather than as brushes, with no centre, radius or
falloff.

#### Scenario: A taper reaches the whole layer
- **WHEN** the user tapers a mesh layer
- **THEN** every vertex is mapped, without a brush position being needed

#### Scenario: A lattice cage moves the form
- **WHEN** the user moves a lattice control point over a mesh layer
- **THEN** the form follows the cage

### Requirement: A mesh gesture is one undo step
The application SHALL record a mesh sculpting gesture as a single undoable
action that reverts the mesh exactly.

Cancelling a gesture in progress SHALL revert that gesture and nothing
underneath it, whatever the representation records it as. A mesh gesture is
previewed as it is made and banked as one record at the release, so the entries
its segments appear to have produced are not what a cancel owes; what it owes
is the document as it stood when the gesture opened.

#### Scenario: One gesture, one undo
- **WHEN** the user completes a mesh stroke and undoes
- **THEN** the mesh is exactly as it was before the stroke began

#### Scenario: A cancelled gesture leaves the committed ones standing
- **WHEN** the user commits two mesh gestures, begins a third and cancels it
- **THEN** the mesh is exactly as the second gesture left it, and both
  committed gestures are still there to undo

#### Scenario: A cancelled first gesture leaves the layer
- **WHEN** the user begins a gesture on a mesh layer that has just been created
  and cancels it
- **THEN** the layer is still in the document, holding what it held before the
  gesture began

### Requirement: A rebuild is refused while a gesture is open
A rebuild replaces every vertex and every index of the layer it is asked about.
An open gesture holds an adjacency, a spatial index and an exact record of what
it has moved over those same triangles, so the application SHALL refuse a
rebuild while a gesture is open, SHALL say so in a sentence that names the
stroke, and SHALL leave the layer byte-identical.

#### Scenario: A rebuild mid-stroke is refused
- **WHEN** a gesture is open on a mesh layer and a rebuild is asked for
- **THEN** it is refused with a reason, and the layer is exactly as the gesture
  left it

#### Scenario: The same rebuild goes through once the stroke is finished
- **WHEN** the gesture ends and the rebuild is asked for again
- **THEN** it happens

### Requirement: A mesh layer's topology can be rebuilt
The application SHALL let a sculptor rebuild a mesh layer's topology through a
voxel field, so that overlapping shells fuse, self-intersections resolve,
stretched triangles disappear and the density comes out uniform. This is the
repair for a form that has been pulled somewhere its triangles cannot follow,
and it is the mesh counterpart to collapsing a field layer that has steepened.

It SHALL be offered rather than taken: it destroys the topology it replaces, and
the decision is the sculptor's. Unlike the field layer's collapse — which is
offered only when the engine advises it — the rebuild SHALL be available
whenever a mesh layer is active. The engine measures a field's steepening and
can say when collapsing is worth it; there is no equivalent measurement for a
topology that has stopped taking detail, and the sculptor is the one who can see
that.

The sculptor SHALL choose the density, stated as cells across the form's longest
extent so that it means the same thing on any size of form, and the application
SHALL report back what that came to in world units.

The rebuild SHALL be one undoable step. A refusal SHALL leave the layer exactly
as it was, which is what makes it safe to offer a density the form may turn out
not to survive.

#### Scenario: A rebuilt layer holds new triangles
- **WHEN** the sculptor rebuilds an active mesh layer
- **THEN** the layer holds the rebuilt triangles and the viewport draws them

#### Scenario: The density is what was asked for
- **WHEN** the same form is rebuilt at two different densities
- **THEN** the coarser request produces fewer triangles and a larger cell

#### Scenario: A representation with no topology refuses by name
- **WHEN** a rebuild is asked of a field or a grid layer
- **THEN** it is refused with a reason naming what a rebuild applies to

#### Scenario: A rebuild is one step on the undo menu
- **WHEN** the sculptor undoes a rebuild
- **THEN** the triangles it replaced are back

### Requirement: A rebuild states what it destroyed
The application SHALL report, after a rebuild, what the rebuild cost: the
triangle counts before and after, how many separate pieces the form is now in,
and each thing the operation destroyed that a sculptor cannot see by looking at
the result.

Vertex and polygon identity are destroyed every time and texture coordinates are
dropped rather than reprojected — the engine will not pretend to carry a UV
layout across a seam, because a stretched layout looks like a preserved one. The
application SHALL say so rather than leaving it to be discovered later.

The report SHALL persist beside the control rather than appearing once. The
question a sculptor asks — "did those two actually join?" — is asked after
looking at the result, and the piece count is where it is answered.

#### Scenario: The counts are shown
- **WHEN** a rebuild completes
- **THEN** the triangle counts before and after are shown beside the control

#### Scenario: Dropped texture coordinates are stated
- **WHEN** a rebuild drops the source's texture coordinates
- **THEN** the application says so, as a fact about the rebuild rather than as a
  failure

#### Scenario: A form still in pieces says so
- **WHEN** a rebuild meant to fuse leaves more than one piece
- **THEN** the number of pieces is shown

### Requirement: Sculpting survives a rebuild and its undo
A rebuild replaces every vertex and every index in the layer. Any adjacency,
bounding volume or sculptor the application holds over that layer SHALL be
discarded when it happens, and a stroke made immediately afterwards SHALL land.

This SHALL hold in **both** directions of history. Undoing a rebuild replaces the
triangles again, and so does redoing it; the engine's geometry revision does not
move when history does, so the application SHALL keep its own account of where a
rebuild sits in the history rather than relying on that number alone.

#### Scenario: A stroke lands on the rebuilt mesh
- **WHEN** the sculptor rebuilds a mesh layer and immediately sculpts it
- **THEN** the stroke lands and moves the surface

#### Scenario: A stroke lands after the rebuild is undone
- **WHEN** the sculptor rebuilds a mesh layer, undoes the rebuild, and sculpts
- **THEN** the stroke lands and moves the surface

#### Scenario: A stroke lands after the rebuild is redone
- **WHEN** the sculptor undoes a rebuild, redoes it, and sculpts
- **THEN** the stroke lands and moves the surface

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
