## ADDED Requirements

### Requirement: The manipulator keeps its size on screen
The manipulator SHALL be sized so that it stays approximately the same size to
the hand whatever the camera's distance, for every target it can be placed on.
A manipulator MAY grow beyond that size to reach past a large target, and SHALL
NOT shrink below it.

The rule SHALL be the same for every target. Two widgets drawn with the same
shapes and worked with the same gestures SHALL NOT answer a zoom differently.

#### Scenario: Zooming out does not shrink the widget
- **WHEN** the camera is moved away from a selection with a manipulator on it
- **THEN** the manipulator's apparent size does not fall below its screen-constant floor

#### Scenario: Every target is sized by the same rule
- **WHEN** a manipulator is placed on a deformation cage and on a placed object at the same camera distance
- **THEN** neither is smaller than the screen-constant floor

### Requirement: The transform is reported beside the manipulator
While a manipulator is pointed at a placed object, the viewport SHALL show that
object's position, its rotation, the axis that rotation is about, and its
scale. The readout SHALL be translucent and SHALL NOT hide the form it
describes.

The readout SHALL report only values the domain holds: an axis and one angle
for rotation, and one factor for scale. It SHALL NOT present three rotation
values or three scale values.

The readout SHALL be shown only where it has an answer. A target that has no
single position, rotation and scale — a set of control points, or a whole layer
— SHALL be given none.

#### Scenario: The numbers are on screen while the widget is
- **WHEN** a manipulator is on a placed object
- **THEN** the object's position, rotation, rotation axis and scale are shown in the viewport

#### Scenario: Nothing is invented for rotation or scale
- **WHEN** the readout is shown
- **THEN** rotation is one angle about one axis, and scale is one factor

#### Scenario: A target with no single transform gets no readout
- **WHEN** the manipulator's target is a whole layer or a set of control points
- **THEN** no transform readout is drawn

#### Scenario: A value that rounds to nothing is not shown signed
- **WHEN** a coordinate is a small negative that rounds to zero at the display precision
- **THEN** it is shown as zero without a sign
