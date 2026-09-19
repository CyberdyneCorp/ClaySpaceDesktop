## MODIFIED Requirements

### Requirement: Sculpt layers are presented as a stack
The application SHALL present a voxel layer's sculpt layers as an ordered stack
that can be shown, hidden, reordered, merged down and removed, and SHALL report
what each costs in memory.

Showing, hiding and reordering are each one step of the history, as changing a
strength is. Merging a pass down and removing one **discard the recorded diff
they act on**, which nothing can put back, so the application SHALL NOT offer
them as something to take back and SHALL NOT leave a way back into that stack
that a later step could apply to a renumbered one.

Every operation on the stack SHALL state its refusal — a second recording
opened over an open one, a merge with nothing beneath it — on the line the
interface already shows scene refusals on.

#### Scenario: A sculpt layer is hidden
- **WHEN** the user hides a sculpt layer
- **THEN** the surface shows the sculpt without that layer's contribution

#### Scenario: Hiding a pass is one thing to take back
- **WHEN** the user hides a sculpt layer and undoes
- **THEN** the pass is shown again and nothing else moved

#### Scenario: Cost is reported
- **WHEN** the sculpt layer stack is shown
- **THEN** each layer's memory cost, and the total, are stated

#### Scenario: A refused operation on the stack is stated
- **WHEN** the user asks for something the stack cannot do
- **THEN** the interface says why, rather than the request appearing to have
  worked
