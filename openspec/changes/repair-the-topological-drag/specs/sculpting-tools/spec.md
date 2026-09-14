## ADDED Requirements

### Requirement: A bake-and-replace verb SHALL land feathered, over a band that carries it
A stroke that reaches the engine by sampling a region of the document into a
volume and placing it back with `Op::Replace` SHALL place that volume
**feathered**, and SHALL bake it with a **band at least as wide as the distance
the verb moves the surface**.

Both halves are the contract `clay_volume_params` states, and each one alone is
a defect a sculptor sees.

Without the feather, a hard `CLAY_OP_REPLACE` holds both the baked volume and
the field beneath it live at the boundary, and branch-switching between two
fields that touch ripples the normals at the cell wavelength: the zero set is
exact and the shading is not. Measured on the reported stroke of #128 against an
untouched sphere rendered in the same run, a hard placement reads **2.07x** the
sphere's own frame roughness where a feathered one reads **1.38x** — and a hard
placement of a region the verb never touched still reads **1.88x**, so the
defect is the placement and not the verb.

Without a band that covers the verb, the feather's own correction is clamped at
the band and the move is expressed only up to it. Measured on a horseshoe with a
brush of 1.0 and a drag of 0.3, a feathered placement over the default
three-cell band lifted the anchored tip **+0.0601** — the band, to four digits —
where the same placement over a band widened by the drag's own length lifted it
**+0.2997**.

Where the engine verb rebuilds the volume rather than editing it in place, and
so returns one the feather has been stripped from, the application SHALL restore
the feather by baking the placed region back out of the document through a
producer that takes `clay_volume_params`, and SHALL remove the unfeathered
placement it sampled through. Every engine edit such a repair costs SHALL be
bracketed into the one history entry the stroke is.

**Each half SHALL have its own guard, because neither measure sees both.** A
roughness measure cannot guard the band: a drag clamped to the band moves
almost nothing, and a surface that has barely moved is smooth, so the clamped
tool scores **better** than the repaired one (1.14x against 1.38x) and passes.
A displacement measure cannot guard the feather: a hard `CLAY_OP_REPLACE` is
not clamped at all, so it lifts the tip the full gesture (+0.2933) while
corrugating the shading. The application SHALL therefore hold both a roughness
guard and a displacement guard over this stroke, and reverting either half of
the repair SHALL fail exactly one of them.

**The band's cost SHALL be measured and stated rather than assumed small.** A
volume stores samples in the bricks its band reaches, so a band that carries
the drag thickens what the bake evaluates and holds: measured on the starting
sphere with brush 0.35, one stroke, peak RSS goes 190 → 503 MB and the stroke
51 → 111 ms for a 0.4 pull, and 405 → 2225 MB / 461 → 799 ms for a two-unit
one. The cost saturates once the band exceeds the sampled box, and it is the
band rather than the box: `padding` does not enter this path at all, because
the engine applies it only where no explicit region was passed.

#### Scenario: The topological drag leaves no hard edge
- **WHEN** Mover Topológico is stroked across an SDF layer
- **THEN** the rendered surface is no more than 1.5x rougher than the same
  sphere untouched, measured as the mean neighbour-to-neighbour pixel step over
  the lit pixels in the same run

#### Scenario: The drag still carries what it is named for
- **WHEN** Mover Topológico lifts one tip of a form whose two tips are close in
  space and far along the material
- **THEN** the anchored tip rises by the gesture and not by the bake's band, and
  the far tip does not follow

#### Scenario: The band is taken out from under the drag
- **WHEN** the bake's band is left at its three-cell default while the feathered
  placement stays
- **THEN** the displacement guard fails and names the tip's rise against the
  gesture — and the roughness guard does **not**, because the clamped drag is
  smoother than the repaired one

#### Scenario: One stroke is one undo
- **WHEN** a bake-and-replace stroke that repairs its own placement is undone
- **THEN** it costs the same number of history entries as a bake-and-replace
  stroke that makes a single edit, and one undo leaves neither the hard
  placement nor half the repair behind

### Requirement: A field brush SHALL be guarded in a frame, not only in a table
The application SHALL hold a guard that strokes **every** tool the shelf offers
on an SDF layer with the same gesture, renders the result, and compares its
roughness to an untouched sphere rendered in the same run. The guard SHALL be a
ratio rather than a level, because what a shading step is worth depends on the
device.

A tool whose stroke the guard cannot apply SHALL be reported as a failure. The
guard SHALL NOT skip a tool, and SHALL NOT pass by declining to look at one.

#### Scenario: A tool refuses its stroke
- **WHEN** one tool on the field shelf returns an error for the guard's gesture
- **THEN** the guard fails and names that tool, rather than passing with that
  tool — and every tool after it — unmeasured
