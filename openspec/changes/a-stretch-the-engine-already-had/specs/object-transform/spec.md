## MODIFIED Requirements

### Requirement: Scale is per axis where the engine can apply it
The manipulator SHALL offer a scale handle per axis on a target the engine can
scale per axis, and SHALL NOT offer one on a target it can only scale
uniformly.

A placed object is a node, and the engine's node transform takes a factor per
axis. A whole subtool is a layer, and the engine's layer transform takes one.
The handles offered SHALL follow that distinction rather than a single rule for
both.

Where a per-axis scale is applied, the application SHALL use the engine's
per-axis call for *every* transform of that target and not only for a stretched
one: each call writes the whole transform, so a uniform call applied to a
stretched node would collapse the stretch.

The interface SHALL present one factor where the three agree and three where
they differ, so that an evenly scaled target does not read as three numbers.

#### Scenario: Scaling a placed object
- **WHEN** an object is selected and a scale box on an axis is dragged
- **THEN** that axis is stretched and the other two are unchanged

#### Scenario: The centre handle stays uniform
- **WHEN** an object's centre handle is dragged in scale mode
- **THEN** all three axes are scaled by the same factor

#### Scenario: A whole subtool scales uniformly
- **WHEN** the manipulator is pointed at a whole subtool
- **THEN** no per-axis scale handle is offered, because the engine's layer transform takes one factor

#### Scenario: Per-axis scale on a cage is unaffected
- **WHEN** a lattice selection is scaled
- **THEN** the per-axis handles are still offered, because a cage scales its
  points and does not carry an engine transform

#### Scenario: A move does not unsquash what it moves
- **WHEN** a stretched object is moved or rotated
- **THEN** its per-axis scale is unchanged

#### Scenario: An evenly scaled object reads as one number
- **WHEN** an object's three scale factors are equal
- **THEN** the interface shows one factor rather than the same number three times

### Requirement: A transform is one undo step and survives a reopen
A transform gesture SHALL be one entry in the edit history however many frames
it took, and the resulting placement SHALL survive saving and reopening the
document.

A per-axis scale SHALL survive with it. Where the stored format cannot express
one, a document written by a version that could SHALL still open, with the
object read as evenly scaled rather than the row being dropped.

#### Scenario: A stretch survives a reopen
- **WHEN** a stretched object is saved and the document is reopened
- **THEN** the object is still stretched by the same factors

#### Scenario: A document written before per-axis scale still opens
- **WHEN** a document written by a version that stored one scale factor is opened
- **THEN** its objects are read as evenly scaled, and no row is dropped
