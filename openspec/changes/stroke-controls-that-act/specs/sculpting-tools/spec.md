## MODIFIED Requirements

### Requirement: Brush shaping controls are exposed
The interface SHALL expose the shaping parameters the engine's stroke engine and
brush parameters accept: an alpha curve, noise amount, **the angle each stamp is
turned about its own facing**, edge falloff, accumulation mode (buildup versus
clamped), stroke smoothing, and mirroring. Each SHALL map to a stroke preset or
brush parameter field, and SHALL NOT be presented if it has no engine
counterpart.

The stamp angle SHALL be set in degrees over a whole turn and SHALL wrap rather
than clamp, because an angle has no ends: a whole turn is none, and a value the
control cannot represent — a quantity that is not a number, or an infinity —
SHALL become no rotation rather than reaching the engine, which builds a rotation
basis out of it. Zero SHALL mean no rotation at all rather than a rotation by
zero, which is the value every stroke made before this control existed was made
with.

The angle SHALL be observable only where the footprint has something to orient. A
round brush with no stamp loaded looks the same at every angle by construction,
so the control SHALL be offered without being gated on an alpha being present:
gating it would make a setting appear and disappear as the sculptor changes
stamps, and the setting is held per tool.

The edge falloff SHALL be sent under the name the sculptor chose. Where the
engine's own reading of that name changes, the application SHALL follow the
engine rather than compensating for it behind the control, so that the name on
the dial and the curve on the surface stay the same thing.

Every falloff SHALL lay down a measurably different profile on every
representation that offers the control. A field stroke stamps an item, which
has no falloff curve, so on a field the falloff SHALL be spent as the width of
the stamp's rim: Dura the narrowest, Gaussiana the widest, and Suave the rim
field strokes have always had.

#### Scenario: Buildup versus clamped differ observably
- **WHEN** the same stroke is applied twice over itself with accumulation enabled
  and again with it disabled
- **THEN** the accumulated pass deposits more than the clamped pass, matching the
  engine's buildup semantics

#### Scenario: Falloff selection reaches the engine
- **WHEN** the user selects an edge falloff
- **THEN** the corresponding falloff value is set in the brush parameters passed
  to the verb

#### Scenario: Each falloff lays down its own profile on a field
- **WHEN** the same dab is laid on a field with each of the four falloffs
- **THEN** no two profiles are the same, and a hard edge stops soonest while a
  Gaussian skirt reaches furthest

#### Scenario: A turned stamp lands turned
- **WHEN** the same directional stamp is stroked along the same path twice, once
  upright and once at a quarter turn
- **THEN** the two strokes leave the material in different places

#### Scenario: A whole turn is none
- **WHEN** the stamp angle is set to a whole turn, or to a value that is not a
  representable angle
- **THEN** the setting reads as no rotation

#### Scenario: The name on the dial is the curve on the surface
- **WHEN** the engine's reading of a falloff name changes, and the user selects
  that falloff
- **THEN** the value sent is still the one that name stands for, rather than a
  neighbouring one chosen to reproduce the old curve

### Requirement: Strokes are resolved by the engine's stroke engine
A drag across the surface SHALL be captured as stroke samples — position, pressure and timing — and resolved into edits by the engine's stroke engine, honoring arc-length spacing, pressure curves, jitter, taper and steady-stroke settings. The application SHALL NOT synthesize its own stamp spacing.

A stroke delivered in segments SHALL be stamped as the same gesture delivered
whole would be. The engine lays a stamp at the start of every call and runs its
steady stroke from there, so two properties of the gesture SHALL be carried
across segments by the application rather than restarted per call:

- **Where the next stamp is owed.** Each segment after the first SHALL begin at
  the point on the path, one stamp spacing past the last stamp laid down, and
  the spacing SHALL be the one the document lays that tool's stamps at. A stroke
  sampled sparsely — an agent's handful of far-apart samples — SHALL NOT come out
  as one stamp per sample.
- **The steady stroke.** The lazy-mouse lag SHALL be applied once, over the
  whole gesture, with the engine's own first-order lag, and the engine SHALL NOT
  be asked to apply it again per segment. A drag SHALL NOT be steadied: its
  displacement is measured to the pointer.

#### Scenario: Spacing follows arc length
- **WHEN** the user drags quickly across a region and slowly across another with the same settings
- **THEN** stamp spacing along the stroke is determined by distance travelled, not by the number of input samples received

#### Scenario: Pressure reaches the stroke
- **WHEN** a pressure-sensitive device reports varying pressure during a stroke
- **THEN** those pressure values are carried in the stroke samples handed to the engine

#### Scenario: A sparse stroke has no gaps
- **WHEN** the same arc is stroked at size 0.12 with nine samples and with a
  hundred and sixty-one
- **THEN** the sparse ridge nowhere sinks below 80% of the dense ridge's lowest
  point

#### Scenario: Stroke smoothing damps the path
- **WHEN** a zigzag is stroked with stroke smoothing at 0.5
- **THEN** the path the stamps follow swings less than half as far as the
  pointer did, however many segments delivered it
