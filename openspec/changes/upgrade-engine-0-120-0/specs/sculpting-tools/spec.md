## MODIFIED Requirements

### Requirement: A grid dab writes solid material
A dab on a voxel layer SHALL write its whole footprint, and intensity SHALL
scale the footprint's radius rather than the fraction of cells written.

Occupancy is binary, so a weight between 0 and 1 can only be spent as scattered
coverage. Spending it that way left the middle of a stroke porous, and with a
seed fixed for every dab the skipped cells were the same ones each time, so no
amount of overlap closed them.

An alpha carve is the exception: the stamp's own greys have nowhere but
coverage to live, so it still dithers, with a seed that differs per dab.

**A drag is the other exception, and it is a different one.** A drag's falloff
is not a coverage control at all: the grab is an inverse map, so the weight
decides where each cell in the ball takes its material *from*. Flattened to a
constant it translates the whole neighbourhood rigidly, which is a block being
shoved rather than clay being drawn. So a drag SHALL be written solid like
every other dab and SHALL keep a falloff that falls to the rim, and the two
SHALL NOT be conflated.

#### Scenario: A stroke at the shelf's defaults
- **WHEN** a Padrão stroke is made on a grid at the default intensity and
  falloff
- **THEN** no cell in the core of its footprint is left empty

#### Scenario: A lighter brush
- **WHEN** the same stroke is made at a low intensity and at full intensity
- **THEN** the lighter stroke writes materially fewer cells than the full one,
  and both write their core solid

#### Scenario: A drag on a grid
- **WHEN** Mover drags material on a voxel layer
- **THEN** the cells it carries are the whole footprint rather than a scattered
  subset of it

#### Scenario: A drag draws a bulge rather than shoving a block
- **WHEN** Mover drags material on a voxel layer and the surface is measured
  over the drag's centre and near the rim of its footprint
- **THEN** the centre rises materially further than the rim

#### Scenario: A drag as long as its own brush arrives short of the ask
- **WHEN** Mover drags material on a voxel layer by the brush's own radius
- **THEN** the surface rises by materially less than the distance asked for,
  because the pull tapers rather than carrying the ball rigidly

### Requirement: Brush shaping controls are exposed
The interface SHALL expose the shaping parameters the engine's stroke engine and brush parameters accept: an alpha curve, noise amount, edge falloff, accumulation mode (buildup versus clamped), stroke smoothing, and mirroring. Each SHALL map to a stroke preset or brush parameter field, and SHALL NOT be presented if it has no engine counterpart.

The edge falloff SHALL be sent under the name the sculptor chose. Where the
engine's own reading of that name changes, the application SHALL follow the
engine rather than compensating for it behind the control, so that the name on
the dial and the curve on the surface stay the same thing.

#### Scenario: Buildup versus clamped differ observably
- **WHEN** the same stroke is applied twice over itself with accumulation enabled and again with it disabled
- **THEN** the accumulated pass deposits more than the clamped pass, matching the engine's buildup semantics

#### Scenario: Falloff selection reaches the engine
- **WHEN** the user selects an edge falloff
- **THEN** the corresponding falloff value is set in the brush parameters passed to the verb

#### Scenario: The name on the dial is the curve on the surface
- **WHEN** the engine's reading of a falloff name changes, and the user selects
  that falloff
- **THEN** the value sent is still the one that name stands for, rather than a
  neighbouring one chosen to reproduce the old curve
