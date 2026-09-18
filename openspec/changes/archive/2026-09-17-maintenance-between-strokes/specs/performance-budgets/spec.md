## ADDED Requirements

### Requirement: Work that is not required for correctness happens between interactions
The application SHALL keep the work that makes the next interaction cheaper —
rebuilding a spatial index whose partition has decayed under editing being the
one it can currently produce — separate from the interaction that made it
necessary, and SHALL perform it only at a moment when no gesture is open.

That separation SHALL be a mechanism rather than a convention: the queue SHALL
be unreachable for draining while a gesture is open, and SHALL become reachable
again when the gesture ends *by any route* — committed, cancelled, abandoned, or
taken down with the document. A gesture that ends without saying so SHALL NOT
leave the application unable to do maintenance for the rest of the session.

Nothing queued this way is correctness. Work declined, deferred indefinitely or
never performed SHALL leave the document exactly as it was.

#### Scenario: Nothing is serviced with a pointer down
- **WHEN** work is queued during an open gesture and a drain is asked for
- **THEN** nothing is serviced and the work is still queued

#### Scenario: The moment a gesture ends is the moment it is serviced
- **WHEN** the gesture ends
- **THEN** the queued work is serviced without a further request

#### Scenario: A gesture that never ended cleanly still releases the gate
- **WHEN** a press arrives over a gesture that was never closed, and that
  gesture then ends
- **THEN** the queue is drainable rather than held shut

### Requirement: The between-strokes drain is budgeted, and states what its budget was chosen against
The drain SHALL run against a stated time budget rather than to completion. The
budget SHALL be named where it is defined, together with what it was chosen
against, and SHALL be no more than half of the interface-thread bound the
specification allows a single engine operation.

An item SHALL be started only where what remains of the budget covers the
estimate it carries. Work that does not fit SHALL be left queued rather than
performed or dropped, and SHALL remain visible with a count of how often it has
been asked for, so that work the application is starving can be seen rather than
inferred.

An estimate SHALL be measured on the machine that will pay it rather than
assumed: the first of a kind SHALL be filed with no estimate and timed, and
every request of that kind afterwards SHALL carry what was measured.

#### Scenario: A moment that cannot afford everything leaves the rest
- **WHEN** the queue holds more work than the budget covers
- **THEN** what fits is serviced, what does not is still queued, and the drain
  stops rather than overrunning

#### Scenario: Declining is not dropping
- **WHEN** a later moment can afford the work that was left
- **THEN** the same work is serviced and nothing was lost

#### Scenario: An estimate is what this machine measured
- **WHEN** work of a kind has been performed once
- **THEN** the next request of that kind carries the measured figure rather
  than a guess

### Requirement: A gesture holds a memory pin
While a gesture is open the application SHALL hold a memory pin, so that a trim
arriving mid-drag reports what it would have released and releases nothing. The
engine prices a trim's cost to the interaction after it — between 0.62 and 2.04
times at the gentlest pressure and between 13 and 182 times at the hardest,
growing with the model — and a drag is the one moment where that cost is certain
to be paid by the sculptor.

The pin SHALL be given back on every way a gesture ends, and SHALL be taken and
given back at exactly the moments the maintenance gate is, so that the two
cannot come apart.

#### Scenario: The pin follows the pointer
- **WHEN** a gesture opens
- **THEN** the pin is held, and it is given back when the gesture ends however
  it ends

#### Scenario: A cage is a gesture too
- **WHEN** a deformation cage is dragged and then applied, or abandoned
- **THEN** the pin was held for the drag and is given back either way
