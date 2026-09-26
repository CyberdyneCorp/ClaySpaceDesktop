# Grid to field off the interface thread

## Why

Issue #185 measured a grid-to-field crossing freezing the application: 23k
cells took 6.8 s, almost all of it inside the converter, and the whole of it on
the interface thread. The engine has since made the conversion one volume
instead of one per palette entry, and it now costs a few hundred milliseconds
at 5k, 23k and 100k cells alike — still a visible freeze, and still with no
stated budget. The conversion itself needs no document: it samples occupancy
over the grid's box and redistances the result.

## What changes

- A grid-to-field crossing runs in three parts: the active grid is read out as
  plain data on the interface thread, an owned copy is converted on a worker,
  and the field is placed back on the interface thread as one crossing and one
  undo step. The synchronous crossing runs the same three parts back to back,
  so both schedules convert the same way.
- The placement is refused, and nothing changes, when the grid was edited,
  undone or removed while the conversion ran. Replacing the document drops an
  in-flight crossing.
- While it runs the crossing is reported as outstanding work to an agent, and
  a second grid-to-field crossing is refused until it lands.
- The benchmark measures the crossing at 5k, 23k and 100k cells, split into
  the interface thread's share (budget 50 ms) and the worker's (budget 1 s).

Grid mask extrude, the other follow-up of #185, is pinned by an engine test at
the thicknesses the issue saw fail (0.3, 0.1, 0.05): it makes a wall row with
geometry, and undo and redo take it back and put it back whole. The mask
history banking in #225 and #234 is what made it one step in the interface.

## Capabilities

### Modified Capabilities

- `representation-conversion`: grid to field converts off the interface thread
  within a stated budget, and never lands on a grid that changed.

## Impact

`claycore` gains `VoxelField::snapshot`, `GridSnapshot` and `Send` for `Item`;
`clayspace-engine` gains the `grid_to_field` module with
`ClayDocument::begin_grid_to_field` and `finish_grid_to_field`; `clayspace-model`
gains `Refusal::SourceMoved`; the application runs the crossing as a background
job; the agent catalogue's `convert run` summary says so. No persisted format
changes.
