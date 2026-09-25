## ADDED Requirements

### Requirement: Curve controls apply immediately and inactive edits refuse
A profile or radius change SHALL update the placed tube before reporting success. A profile change SHALL preserve the selected join. Non-circle profile replacement SHALL be one undoable edit and restore its control state on redo. A curve edit with no active curve SHALL return a refusal rather than success.

#### Scenario: A non-circle tube changes radius
- **WHEN** a square, hexagon or triangle tube's radius changes
- **THEN** its visible thickness changes before the edit returns

#### Scenario: A profile changes on a rounded guide
- **WHEN** a profile is changed on a curve with a rounded join
- **THEN** the placed tube follows that join immediately

#### Scenario: The curve is inactive
- **WHEN** a client tries to add a point or edit an inactive curve
- **THEN** the command is refused and changes nothing
