## ADDED Requirements

### Requirement: Moving a carried subtool leaves the field alone
When the manipulator or the inspector places a whole mesh, grid, hierarchy or
adaptive-surface subtool, the application SHALL NOT dirty any field brick. The
next carried buffer rebuild SHALL draw the carried triangles at the new
placement. The first drag frame SHALL cost the same as every later frame.

#### Scenario: A mesh subtool dragged over the field it was crossed from
- **WHEN** a whole mesh subtool is dragged while the field it was crossed from
  is still drawn under it
- **THEN** no field brick is re-meshed on any drag frame, and the mesh is drawn
  where the manipulator put it

#### Scenario: The first drag frame fits in one frame
- **WHEN** the sculptor starts dragging a whole mesh subtool on the mesh
  reference scene
- **THEN** the document edit, the surface sync and the carried rebuild for that
  first frame take less than 16.7 ms in total

#### Scenario: A field subtool still refills
- **WHEN** a whole SDF subtool is moved
- **THEN** its bricks are refilled both where it stood and where it now stands
