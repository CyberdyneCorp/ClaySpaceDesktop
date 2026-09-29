## ADDED Requirements

### Requirement: A panel's rows fit the panel
Every row of a side panel SHALL fit the panel's width in every shipped
language. A row with a slider SHALL shorten the slider's track rather than push
its value past the panel's edge, since a row wider than its panel widens the
whole region and takes the overrun from the viewport.

#### Scenario: The central region does not depend on the active layer
- **WHEN** the active layer changes representation, and the left panel shows
  that representation's controls
- **THEN** the central region starts in the same place
