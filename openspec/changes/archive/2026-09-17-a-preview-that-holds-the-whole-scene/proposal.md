# A live preview is drawn beside the rest of the scene

## Why

Suavizar and Relaxar open live only where the layer under the brush is the
**only visible field subtool**. That has been true since `live-field-brushes`
and it was written down there as a limitation with an upstream issue against
it, not as a design.

The reason was never the preview but what the preview is *of*. The brick cache
holds the hard union of every visible SDF layer and the engine attributes no
brick to the layer it came from, while a smooth transaction previews one layer
alone. The viewport meshes that preview instead of the document's own cache
while a gesture is open — so with a second field subtool visible, everything
but the layer being smoothed would have vanished for the length of the drag.
The gesture fell back to being held whole and applied on release: correct, and
the sculptor smooths blind.

Composing the two needed the document evaluated *without* one layer, and the
obvious route to that is not available here. Hiding the other subtools,
sampling, and showing them again is three edits to the document, and a smooth
commit correctly refuses a layer that moved since it began — so the route that
would work outside a transaction is exactly the route that cannot be taken
inside one. That is what
[ClayCore#378](https://github.com/CyberdyneCorp/ClayCore/issues/378) asked for.

ClayCore 0.78.0 ships it. `clay_brick_cache_eval_requests_excluding` evaluates
every visible SDF layer except one, **edits nothing and records no undo
entry**, and takes no seed and leaves none — so it is legal at any point inside
an open transaction, and it is priced like a stroke's first dab, which is the
right price for a call taken once at pointer-down. The layers it excludes do
not move while the artist drags.

## What changes

- The gate goes. A live smooth opens on any editable field subtool, however
  many others are visible.
- At pointer-down the preview's lattice is widened over the other visible field
  subtools' bounds and those bricks are filled from the rest of the document.
- Every batch the transaction produces is composed with the rest by an
  elementwise **minimum**. That is exact rather than an approximation: field
  subtools compose by a hard union, so the smaller of the two distances at a
  sample is the document's distance there. Nothing is blended and there is no
  seam to place.
- A document whose only visible field subtool is the one under the brush takes
  the path it took before: nothing is evaluated and nothing is composed. That
  is the case every brush figure in `benchmarks/` is measured on.

## What this does not do

- **It does not draw a per-subtool ghost.** The same entry point is what would
  make one possible — document-minus-L as one surface, L alone faintly on top —
  but that is a rendering feature with a control behind it, and it belongs to a
  change that can also measure it.
- **It does not touch solo, or the subtool bake.** Both hide layers around an
  operation, and both want *one layer alone* rather than everything-but-one.
  `clay_eval_points_excluding` names exactly one excluded layer, so a solo of a
  three-subtool document is not expressible through it. Those stay as they are.
- **It does not re-measure.** `brush.sdf.suavizar` and `brush.sdf.relaxar` are
  measured on a one-subtool scene and take the unchanged path, so no committed
  figure moves. What a two-subtool gesture costs is a new figure, and a new
  figure is a benchmark change of its own.

## The relabelled lattice, which is what made this fiddly

The preview keeps a cache of its own because its lattice cannot be made to land
on the document's — one padding cannot align three axes whose bounds have
different remainders. Preview brick `K` is stored at `K * span` and drawn at
`offset + K * span`, so the requests that cache hands out name the wrong world
position for anything that reads a document. `BrickRequest::translated` is the
copy that names the right one: only the origin moves, an evaluation derives a
request's world box from origin, spacing and dims and never re-derives it from
the key, and the original keeps the generation `submit` checks. Evaluate with
the copy, submit the original.
