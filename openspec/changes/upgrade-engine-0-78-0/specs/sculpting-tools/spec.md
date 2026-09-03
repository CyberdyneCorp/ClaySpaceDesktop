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

#### Scenario: Buildup versus clamped differ observably
- **WHEN** the same stroke is applied twice over itself with accumulation enabled
  and again with it disabled
- **THEN** the accumulated pass deposits more than the clamped pass, matching the
  engine's buildup semantics

#### Scenario: Falloff selection reaches the engine
- **WHEN** the user selects an edge falloff
- **THEN** the corresponding falloff value is set in the brush parameters passed
  to the verb

#### Scenario: A turned stamp lands turned
- **WHEN** the same directional stamp is stroked along the same path twice, once
  upright and once at a quarter turn
- **THEN** the two strokes leave the material in different places

#### Scenario: A whole turn is none
- **WHEN** the stamp angle is set to a whole turn, or to a value that is not a
  representable angle
- **THEN** the setting reads as no rotation
