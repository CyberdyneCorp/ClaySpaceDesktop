## ADDED Requirements

### Requirement: View controls affect visible content
Toggling the grid SHALL change whether the floor grid is drawn. Frame All SHALL frame bounds of visible subtools only.

#### Scenario: Hidden active subtool
- **WHEN** the active subtool is hidden and another subtool is visible
- **THEN** Frame All SHALL frame the visible subtool

### Requirement: Reduced detail retains complete field coverage
The viewport SHALL draw a complete, shaded surface when reduced detail is requested. If some occupied coarse regions have no valid mip, it SHALL draw the complete full resolution surface instead.

#### Scenario: Incomplete mips after an edit
- **WHEN** an edited field has valid mips for only part of its surface
- **THEN** a reduced detail request SHALL draw the complete field without a blank or flat grey region
