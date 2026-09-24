## MODIFIED Requirements

### Requirement: A run of voxel strokes can be recorded as a sculpt layer
The application SHALL allow the user to begin and end recording on a voxel
layer, and SHALL record what the strokes between those points changed as a named
sculpt layer. Creating the sculpt layer SHALL be one undoable action.

#### Scenario: Recording captures a run
- **WHEN** the user begins recording, makes several strokes, and ends recording
- **THEN** a sculpt layer holding those strokes' changes exists on the grid

#### Scenario: Recording state is visible
- **WHEN** recording is in progress
- **THEN** the application shows that it is, so a sculptor cannot record
  unknowingly

#### Scenario: Undoing creation of a pass
- **WHEN** the user creates a sculpt layer and closes its recording without a stroke
- **AND** the user undoes once
- **THEN** that sculpt layer is removed without undoing an earlier edit

#### Scenario: Undoing an open pass
- **WHEN** the user starts recording a sculpt layer and undoes its creation
- **THEN** the layer is removed and the recording indicator is cleared
- **AND** redoing the creation restores the layer closed

#### Scenario: Undoing a stroke inside a pass
- **WHEN** the user records a stroke in a sculpt layer and undoes or redoes it
- **THEN** the grid and the recorded cell count reflect the same restored state

### Requirement: Sculpt layers are presented as a stack
The application SHALL present a voxel layer's sculpt layers as an ordered stack
that can be shown, hidden, reordered, merged down and removed, and SHALL report
what each costs in memory.

Showing, hiding, reordering, merging down, removing, and changing strength SHALL
each be one undoable action. Undo and redo SHALL restore both the stack controls
and the surface represented by the stack, even when another subtool is selected.

Every operation on the stack SHALL state its refusal — a second recording
opened over an open one, a merge with nothing beneath it — on the line the
interface already shows scene refusals on.

#### Scenario: A sculpt layer is hidden
- **WHEN** the user hides a sculpt layer
- **THEN** the surface shows the sculpt without that layer's contribution

#### Scenario: Hiding a pass is one thing to take back
- **WHEN** the user hides a sculpt layer and undoes
- **THEN** the pass is shown again and nothing else moved

#### Scenario: Undo and redo a pass edit from another subtool
- **WHEN** the user changes a pass on one voxel layer and selects another subtool
- **AND** the user undoes or redoes that change
- **THEN** the affected pass controls and surface agree with the restored state

#### Scenario: Undo removal or merge-down
- **WHEN** the user removes a sculpt layer or merges it down and then undoes
- **THEN** the earlier stack and its surface are restored

#### Scenario: Cost is reported
- **WHEN** the sculpt layer stack is shown
- **THEN** each layer's memory cost, and the total, are stated

#### Scenario: A refused operation on the stack is stated
- **WHEN** the user asks for something the stack cannot do
- **THEN** the interface says why, rather than the request appearing to have
  worked
