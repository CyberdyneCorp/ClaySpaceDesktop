## ADDED Requirements

### Requirement: Grid to field converts off the interface thread
A grid-to-field crossing SHALL read the active grid on the interface thread,
convert it on a worker, and place the field back on the interface thread as
one crossing and one undo step, identical to the same crossing run in one
piece. Reading and placing together SHALL stay within 50 ms of interface
thread, and the conversion within 1 s of worker time, for grids of 5k, 23k and
100k cells, as the benchmark measures them.

While the conversion runs, the crossing SHALL be reported as outstanding work,
and a second grid-to-field crossing SHALL be refused. When the grid was edited,
undone or removed before the field is placed, the placement SHALL be refused
with a reason and the document SHALL NOT change; replacing the document SHALL
drop the crossing.

#### Scenario: The window stays live while a grid converts
- **WHEN** the user crosses a 23k-cell grid to a field
- **THEN** the conversion runs on a worker and the crossing is outstanding
  until the field layer lands as one undo step

#### Scenario: A grid edited during its conversion keeps the edit
- **WHEN** the user sculpts the grid while its conversion runs
- **THEN** the converted field is not placed, the refusal says the grid
  changed, and the scene and history are as the stroke left them

#### Scenario: The split crossing is the same crossing
- **WHEN** one grid is crossed to a field in one piece and an identical grid
  through a worker
- **THEN** the two scenes, the two field layers' extents and the undo steps
  agree
