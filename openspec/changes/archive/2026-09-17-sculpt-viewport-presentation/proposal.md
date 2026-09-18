# The grid dissolves, and the quality tiers become reachable

## Why

Two things, both about what the viewport looks like when nobody is touching it.

**The grid drew a rectangle around the scene.** Twenty-one lines each way, all
one tone except the two through the origin, stopping dead at their extent. The
boundary was the strongest thing in the frame after the form, and a grid with
no landmarks in it is one nobody can count a distance on. It read as CAD
scaffolding rather than as a floor.

**The quality profiles were unreachable.** `quality.rs` has carried
`Performance`, `Sculpt` and `Presentation` — the guide's three tiers, with the
stroke-time degradation the same guide files under "consider later" — since it
was written. `QualityGovernor::set_profile` exists. Nothing in the application
had ever called it: the governor is built with `ViewportProfile::default()` and
the profile never changes for the life of the process. The specification
already says a stroke drops quality "whatever quality profile is selected",
which was describing a selection that could not be made.

## What Changes

- **The grid fades out before its own edge**, and is cut into segments so the
  fade varies *along* each line rather than uniformly per line.
- **Every fifth line is a major line**, so there is something to count cells
  against, with the two axes strongest.
- **The viewport profile is chosen from the View menu**, beside the shading
  terms it belongs with, and left in the interface's own memory for the
  composition root to read.

## Two decisions worth stating

**The fade is by distance from the origin, not from the camera.** The guide
suggests the camera, which would mean computing the fade in the shader: overlay
geometry is uploaded only when the overlays themselves change, not per frame,
so a camera-dependent colour would rebuild and re-upload the whole grid on
every orbit — the cost the same guide warns about in its next sentence. Origin
distance is also the better answer here: the form sits at the origin and the
grid is the floor under it, so a grid densest beneath the sculpt and dissolving
outward says the same thing from every camera instead of changing what it says
when the sculptor zooms.

**The fade is a mix toward the viewport's ground, not an alpha ramp.** The
overlay pipeline does blend, but a vertex carries three floats of colour and no
alpha, and widening it would cost every mesh in the application a quarter more
memory for one overlay's benefit. Mixing toward the ground is exact wherever
the grid is drawn over that ground, which is everywhere it can be seen: the
pipeline is depth-tested, so the form hides what is behind it. A camera below
the floor looking up would put an unfaded-looking line over the clay. That is
the one case this trades away, and the shader route is what to reach for if it
ever matters.

**The profile is not a command.** It changes what an idle frame is drawn
*with* and never what is drawn, so nothing about it reaches the document — and
it could not be a command in any case, since `ViewportProfile` is a view type
and commands live in the layer underneath the view. It goes the way the section
folds and the shelf's filter go, through the interface's own memory, read after
the frame by the composition root that owns the governor.

## Out of scope, and why

- **MatCap presets.** Already shipped: five built-ins, generated rather than
  bundled, covering the neutral, dark, cool, warm and polished readings the
  guide lists. Adding more before anyone has asked for one is guessing.
- **AO and shadow tuning.** The guide says not to build a second occlusion
  system, and this does not — but it also does not retune the first. The
  existing figures are held by measured requirements about sample counts,
  upsampling and pass order; changing them means moving those, with captures
  and a benchmark run behind it, and that is a change about occlusion rather
  than about the grid.
- **Not remembering the profile across launches.** Left for a later change
  rather than impossible. The reason first given here — that there is nowhere
  to remember it — was wrong: `SessionStore` has been keeping the recent
  documents and the chosen locale all along, and
  `the-regions-move-and-are-remembered` puts the arrangement of the regions
  beside them. The profile can follow.
