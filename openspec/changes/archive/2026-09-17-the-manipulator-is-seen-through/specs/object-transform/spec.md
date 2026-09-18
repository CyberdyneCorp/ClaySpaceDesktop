## MODIFIED Requirements

### Requirement: The manipulator is seen wherever it stands
The manipulator, the deformation cage, a curve's control polygon and a
selected object's outline SHALL be drawn over the sculpted surface regardless
of depth: a handle that lies behind or inside the form SHALL still be drawn and
SHALL still be grabbable on the same terms as one in front of it. It MAY be
drawn fainter to say that it is behind — a cue about distance, never about
whether it can be used, and never to the point of being hidden.

The manipulator's handles SHALL be drawn heavier than a single device pixel,
its arrowheads, scale boxes and pivot SHALL be solid shaded bodies rather than
line hints, and its arms SHALL keep a constant size on screen as the camera
moves toward or away from what it acts on. The size drawn and the size
hit-tested SHALL come from one definition, and the strength drawn SHALL have no
bearing on either.

#### Scenario: A manipulator inside the form
- **WHEN** a manipulator's pivot and every handle lie inside a placed sphere
- **THEN** the manipulator is drawn over the sphere's surface, faint where the
  sphere stands in front of it

#### Scenario: A faint handle is still a handle
- **WHEN** a press lands on a handle that is drawn faint because the form is in
  front of it
- **THEN** it is grabbed exactly as a handle drawn at full strength would be

#### Scenario: Zooming keeps the widget the same size to the hand
- **WHEN** the camera moves to half its distance from the selection
- **THEN** the manipulator's arms cover the same fraction of the viewport as
  before, and a press at the drawn tip of an arm still finds that arm
