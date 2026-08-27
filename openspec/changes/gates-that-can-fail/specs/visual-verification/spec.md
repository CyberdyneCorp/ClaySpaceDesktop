## ADDED Requirements

### Requirement: A visual assertion is measured against the machine's own render floor
A test that decides something from two rendered frames SHALL compare against
the difference two renders of the *same* subject already produce on that
machine, rather than against a constant chosen once.

The floor is not the same everywhere: it is zero on Linux and it is not on
macOS, where a runner was measured leaving 1,294 pixels byte-differing on a
frame that was meant to be unchanged. Every threshold set below that number was
satisfied there by the rasteriser, whether or not the thing under test had
happened.

Where a floor is measured, it SHALL be measured through the same pipeline the
figures go through. A floor taken from two draws of one vertex buffer sits
under a comparison taken after a re-mesh, because a re-mesh can return the same
surface with its vertices in a different order and move a rasterized edge.

#### Scenario: An effect is claimed from a rendered difference
- **WHEN** a test asserts that an operation changed what is drawn
- **THEN** the pixels it counts are those past the measured noise, not those
  differing by any amount

#### Scenario: The floor is measured on the machine
- **WHEN** a test needs to know what "unchanged" looks like
- **THEN** it renders the unchanged subject through the same path and measures,
  rather than assuming a constant holds everywhere

### Requirement: An assertion that something is gentle first establishes it happened
A test asserting that an operation changed little SHALL first assert that the
operation did anything at all.

`the_smoothing_tools_smooth_rather_than_crumble` stopped applying its stroke
when a refactor replaced it, and stayed green for four releases: every figure
was read off two copies of one frame, and a surface compared with itself agrees
with every claim about being gentle. A second fault — a roughness truncated
from 5.83 to 5 by a cast, against an untruncated 5.40 — made the same frame
appear to have been *improved*.

A quantity that did not change is not evidence that a tool is careful. It is
what a deleted call looks like.

#### Scenario: A tool is asserted to be gentle
- **WHEN** a test bounds how much an operation changed
- **THEN** it asserts the operation moved something before it reads the bound

#### Scenario: A sample is averaged
- **WHEN** a mean or ratio is computed over the pixels that changed
- **THEN** the count is asserted too, so the mean is not taken over nothing

### Requirement: A sampled region is established to be on the subject
A test that reads a region of a frame SHALL establish that the region is on the
thing under test before concluding anything from it.

A cage test decided from the single pixel at the frame centre with nothing
saying the form covered it. Had the framing ever changed, both reads would have
become the background, the difference would have been zero, and the test would
have reported that a cage changed nothing about a surface that was not there.

#### Scenario: A region is read from a frame
- **WHEN** a test samples part of a rendered frame
- **THEN** it asserts the sample is on the subject rather than the background
