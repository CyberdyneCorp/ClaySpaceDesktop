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

Create Hierarchy SHALL build as many levels as the sculptor asks for in one
action and one undo step, and SHALL state the cost of the whole hierarchy —
the cage and every level as it is held once drawn — against what the document
holds before anything is allocated. A figure the engine did not quote SHALL be
marked as projected. A level SHALL be charged at the higher of its build peak
and what it holds once drawn, whether it arrives through Create Hierarchy or a
single subdivision, because a level is drawn the moment it arrives.

#### Scenario: A level exceeds the remaining budget
- **WHEN** the requested level would put document usage over its memory limit
- **THEN** creation is refused with current usage, additional cost and limit

#### Scenario: A hierarchy is priced before anything is built
- **WHEN** a mesh layer is active and the sculptor looks at Create Hierarchy
- **THEN** what the document holds, what the hierarchy would add and the limit
  are shown, and asking for them adds no layer and no history entry

#### Scenario: A hierarchy that would not fit is refused before it is built
- **WHEN** Create Hierarchy is asked for a depth whose hierarchy, on top of
  what the document holds, passes the limit
- **THEN** it is refused naming the three figures, and the document holds the
  layers and the history it held before

#### Scenario: A hierarchy that fits is one step
- **WHEN** Create Hierarchy builds a hierarchy two levels deep
- **THEN** one layer holds three levels with sculpt and display on the top one,
  the mesh it was built from remains unless replacement was asked for, and one
  undo removes the hierarchy

#### Scenario: A plan that fits is not refused half way up
- **WHEN** the limit is exactly what the plan quoted for the hierarchy
- **THEN** every level is built, and the drawn hierarchy holds no more than the
  plan quoted

#### Scenario: A level is charged at what it holds once drawn
- **WHEN** a level whose build peak fits and whose drawn size does not is asked
  for
- **THEN** it is refused, naming the drawn size as the level's cost

#### Scenario: A coarse edit keeps fine detail
- **WHEN** a coarse sculpt level is edited while fine detail exists above it
- **THEN** the fine-detail checksum remains unchanged and the display level remains independently selectable

#### Scenario: A hierarchy is baked to a fixed mesh
- **WHEN** a sculptor chooses a level and accepts its bake preview
- **THEN** the resulting mesh carries the reported form and detail, and the reported hierarchy features are no longer editable on that mesh
