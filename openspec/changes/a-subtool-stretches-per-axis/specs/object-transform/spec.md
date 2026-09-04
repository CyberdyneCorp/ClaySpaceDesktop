## MODIFIED Requirements

### Requirement: Scale is per axis where the engine can apply it
The manipulator SHALL offer a scale handle per axis on a target the engine can
scale per axis, and SHALL NOT offer one on a target it can only scale
uniformly.

A placed object is a node and the engine's node transform takes a factor per
axis. A whole subtool is a layer, and the engine's layer transform has taken a
factor per axis since ClayCore ABI 0.74.0. Both therefore carry the three
boxes, and the manipulator SHALL be one widget with the same handles wherever
it stands rather than two widgets chosen by what it is pointed at.

Where a per-axis scale is applied, the application SHALL use the engine's
per-axis call for *every* transform of that target and not only for a stretched
one: each call writes the whole transform, so a uniform call applied to a
stretched target would collapse the stretch.

A world length carried into a target's own frame — a brush radius, a join width
— SHALL be divided by the largest of the three factors rather than by their
mean, so that a gesture never reaches outside the region it named.

The interface SHALL present one factor where the three agree and three where
they differ, so that an evenly scaled target does not read as three numbers.

#### Scenario: Scaling a placed object
- **WHEN** an object is selected and a scale box on an axis is dragged
- **THEN** that axis is stretched and the other two are unchanged

#### Scenario: The centre handle stays uniform
- **WHEN** an object's centre handle is dragged in scale mode
- **THEN** all three axes are scaled by the same factor

#### Scenario: A whole subtool stretches per axis
- **WHEN** the manipulator is pointed at a whole subtool and a scale box on an
  axis is dragged
- **THEN** that axis of the subtool is stretched, the other two are unchanged,
  and the stretch reaches the field the subtool evaluates to

#### Scenario: Per-axis scale on a cage is unaffected
- **WHEN** a lattice selection is scaled
- **THEN** the per-axis handles are still offered, because a cage scales its
  points and does not carry an engine transform

#### Scenario: A move does not unsquash what it moves
- **WHEN** a stretched object or subtool is moved or rotated
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

A whole subtool's placement — where it stands, how it is turned and how it is
stretched — SHALL be read back from the engine rather than reconstructed by the
application, both when a document is opened and after the history moves.

#### Scenario: A stretch survives a reopen
- **WHEN** a stretched object is saved and the document is reopened
- **THEN** the object is still stretched by the same factors

#### Scenario: A document written before per-axis scale still opens
- **WHEN** a document written by a version that stored one scale factor is opened
- **THEN** its objects are read as evenly scaled, and no row is dropped

#### Scenario: A moved subtool reopens where it stands
- **WHEN** a document whose subtool was moved, turned and stretched is reopened
- **THEN** the subtool's placement is what it was saved as, and the manipulator
  stands on the form rather than at the origin

#### Scenario: Undoing a stretch takes it back
- **WHEN** a whole subtool is stretched and the edit is undone
- **THEN** the subtool is unstretched, and redoing stretches it again

## ADDED Requirements

### Requirement: A stretched subtool is refused the deformation cage in words
The engine returns no warps at all for a layer carrying a per-axis scale,
because a cage records its item-to-cage placement as a rigid transform and a
squashed layer needs a general affine map. The application SHALL refuse the
cage on such a subtool with a message naming the stretch, rather than letting
the refusal arrive as a cage that reached nothing.

#### Scenario: Putting a cage on a stretched subtool
- **WHEN** a subtool carrying a per-axis stretch is caged and the cage applied
- **THEN** the application refuses it and names the stretch as the reason

#### Scenario: The same cage on an unstretched subtool
- **WHEN** the stretch is taken back and the same cage applied
- **THEN** it deforms the subtool as it always did

### Requirement: The document format version this build writes is named
The `.clayspace` container version this build writes SHALL be stated in the
code with its reasoning, checked against the pinned engine's own headers, and
readable from a written file rather than asserted. It SHALL appear in the
diagnostics report.

A build older than the one that introduced a minor refuses such a document
rather than misreading it, so the number is the answer to "it will not open
elsewhere".

#### Scenario: A document this build writes
- **WHEN** a document is saved and its header read back
- **THEN** the version in the file is the one the build says it writes

#### Scenario: The pinned engine moves its format
- **WHEN** the vendored engine declares a container or scene minor past what
  the build claims
- **THEN** the test that compares them fails
