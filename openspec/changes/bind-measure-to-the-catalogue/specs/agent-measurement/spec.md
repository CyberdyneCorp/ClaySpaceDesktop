## ADDED Requirements

### Requirement: A measured answer names the work it left running
A measured command that leaves work running when the clock stops — a job it
started off the interface thread, such as a retopology — SHALL list that work
in its answer and SHALL say that the figure is the time to start it rather
than to finish it.

#### Scenario: Retopology started through measure
- **WHEN** an agent measures `retopo` `run`
- **THEN** the answer lists the retopology as outstanding and notes that the
  figure does not include it
