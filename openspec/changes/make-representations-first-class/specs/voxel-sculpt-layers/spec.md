## Purpose

Recording a run of voxel strokes as an addressable layer whose strength stays
adjustable afterwards — so a pass of detail can be dialled back hours later
rather than only undone at the time.

## ADDED Requirements

### Requirement: A run of voxel strokes can be recorded as a sculpt layer
The application SHALL allow the user to begin and end recording on a voxel
layer, and SHALL record what the strokes between those points changed as a named
sculpt layer.

#### Scenario: Recording captures a run
- **WHEN** the user begins recording, makes several strokes, and ends recording
- **THEN** a sculpt layer holding those strokes' changes exists on the grid

#### Scenario: Recording state is visible
- **WHEN** recording is in progress
- **THEN** the application shows that it is, so a sculptor cannot record
  unknowingly

### Requirement: A sculpt layer's strength is adjustable after the fact
The application SHALL allow a recorded sculpt layer's strength to be changed
after it was recorded, and SHALL show the result on the surface.

#### Scenario: Dialling a finished pass back
- **WHEN** the user reduces the strength of a sculpt layer recorded earlier
- **THEN** the surface shows less of that pass, without the strokes being redone

#### Scenario: Strength is not undo
- **WHEN** the user changes a sculpt layer's strength and then undoes
- **THEN** the strength change is undone, and the sculpt layer still exists

### Requirement: Sculpt layers are presented as a stack
The application SHALL present a voxel layer's sculpt layers as an ordered stack
that can be shown, hidden, reordered, merged down and removed, and SHALL report
what each costs in memory.

#### Scenario: A sculpt layer is hidden
- **WHEN** the user hides a sculpt layer
- **THEN** the surface shows the sculpt without that layer's contribution

#### Scenario: Cost is reported
- **WHEN** the sculpt layer stack is shown
- **THEN** each layer's memory cost, and the total, are stated
