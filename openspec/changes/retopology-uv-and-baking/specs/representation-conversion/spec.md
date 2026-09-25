## ADDED Requirements

### Requirement: The retopology pipeline reaches a hierarchy and bakes out explicitly
The workflow SHALL offer Create Hierarchy from an accepted fixed mesh result.
Before creating a level, it SHALL report current document memory, the level's
estimated additional cost and the limit, and SHALL refuse a level that exceeds
the limit. Sculpt and display levels SHALL be separately named and controlled.
Cache trimming and compaction actions SHALL report the memory they freed.

Baking a hierarchy to a fixed mesh SHALL state which level and sculpt detail
the result carries and which hierarchy capabilities it loses. Removing the
highest level SHALL be undoable or require the existing destructive-operation
consent gate. State reports SHALL include level counts and a detail checksum.

#### Scenario: A level exceeds the remaining budget
- **WHEN** the requested level would put document usage over its memory limit
- **THEN** creation is refused with current usage, additional cost and limit

#### Scenario: A coarse edit keeps fine detail
- **WHEN** a coarse sculpt level is edited while fine detail exists above it
- **THEN** the fine-detail checksum remains unchanged and the display level remains independently selectable

#### Scenario: A hierarchy is baked to a fixed mesh
- **WHEN** a sculptor chooses a level and accepts its bake preview
- **THEN** the resulting mesh carries the reported form and detail, and the reported hierarchy features are no longer editable on that mesh
