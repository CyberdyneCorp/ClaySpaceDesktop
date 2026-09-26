## MODIFIED Requirements

### Requirement: A live operand stays interactive while it is dragged
While an object that participates in a boolean is being dragged, the viewport
SHALL show the result of the boolean at the object's current position, and the
interface SHALL remain responsive throughout.

Where the re-evaluation cannot keep up, the application SHALL show the object
moving against the last completed surface and settle when the drag ends, rather
than blocking the drag.

Each frame of the drag SHALL refill the region that move changed, as the
engine reports it, and not the whole layer. For an intersecting object that
region is the swept union of where it was and where it went. The cached
surface SHALL then agree with the document, including after placing an
intersecting object and after undoing or redoing its drag.

#### Scenario: The cavity follows the drag
- **WHEN** a subtracted object is dragged across the form
- **THEN** the cavity moves with it

#### Scenario: A form too heavy to re-evaluate live
- **WHEN** the surface cannot be re-evaluated within the frame budget
- **THEN** the drag continues at interactive speed and the surface settles when
  it ends

#### Scenario: An intersecting object drags at the cost of a subtracting one
- **WHEN** an object set to intersect is dragged across the form
- **THEN** each frame refills about as many bricks as the same drag with the
  object subtracting, and the drawn surface matches the document where the
  object was, where it is and everywhere else

#### Scenario: Placing an intersecting object keeps only what it intersects
- **WHEN** an object set to intersect is placed on the form
- **THEN** the drawn surface outside the object is gone, as it is in the
  document
