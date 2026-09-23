## ADDED Requirements

### Requirement: A region-sampling verb is demonstrated against a surface it can act on
Four of the field verbs sample a region rather than stamp into it — the engine
adapter groups them as such — and each averages toward something the
neighbourhood already is.

A test that requires such a verb to move the surface SHALL measure it against a
surface that has something for it to do. A pristine sphere is the smoothest
thing there is, so requiring a smoothing verb to move one is requiring it to do
the job it exists *not* to do, and a fixture that passes on one is measuring
something other than the verb.

This SHALL NOT be met by lowering the threshold. The figure a verb has to clear
states what a sculptor would notice; a fixture that cannot produce it is the
part that is wrong.

#### Scenario: A smoothing verb is asked to smooth
- **WHEN** a region-sampling verb is tested for having any effect
- **THEN** it is applied to a surface carrying a feature it can flatten, and is
  required to flatten it by the same margin every other verb must move a
  surface by

#### Scenario: A stamping verb is unaffected
- **WHEN** a verb that displaces along a normal is tested for the same property
- **THEN** a resting surface is a sufficient fixture, and the threshold is
  unchanged
