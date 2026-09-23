## MODIFIED Requirements

### Requirement: Interface responsiveness is independent of engine work
The application SHALL remain responsive to input while engine work is in progress: no engine operation SHALL block the interface thread for more than 16 ms.

Re-evaluating the surface cache SHALL be bounded by a budget the host sets and
SHALL be continued across frames rather than run to completion on the caller's
thread. The cache SHALL keep whatever a bounded drain did not take, so the work
is stopped and not dropped; the document SHALL report that a refill is
outstanding, and the host SHALL keep asking for frames until it is not. A
document with no budget set SHALL drain in full, because a caller with nothing
waiting on it needs the exact answer before it returns.

The bound SHALL apply to the batches as well as to the gaps between them: a
budget honoured only between batches is that budget plus a whole batch, which
on a slow backend is most of a frame.

Whether a region was re-evaluated in one drain or in many, the surface that
results SHALL be the same surface.

#### Scenario: A long operation does not freeze the window
- **WHEN** a consolidation, bake, import or export runs
- **THEN** the window continues to redraw and respond, and progress is displayed

#### Scenario: Interface-thread blocking is detectable
- **WHEN** the application runs with the debug instrumentation enabled
- **THEN** any interface-thread block exceeding 16 ms is recorded with the operation responsible

#### Scenario: A refill larger than one budget is continued
- **WHEN** a command dirties more of the field than one budget can re-evaluate
- **THEN** the command returns within the budget, the document reports the
  refill as outstanding, and the remaining bricks are re-evaluated over the
  following frames

#### Scenario: A pumped refill reaches the surface a whole drain reaches
- **WHEN** the same edit is drained in full by one caller and a budget at a
  time by another
- **THEN** both documents hold the same surface

#### Scenario: Showing or hiding a worked subtool returns within a frame
- **WHEN** the sculptor toggles the eye on a field subtool of any size
- **THEN** the write returns within the refill budget, and the surface the
  toggle changes is completed over the frames that follow

### Requirement: Edit cost is proportional to the region edited
The work performed for an edit SHALL be bounded by the region the edit's influence bound reaches, and SHALL NOT grow with the size of the rest of the document.

Taking an edit back SHALL be bounded the same way as making it. Retiring a
placed sweep SHALL re-evaluate the region the sweep occupied — the engine's own
bound for that item, asked for while the document still holds it — and SHALL
NOT re-evaluate the subtool the sweep was laid on, nor the box that subtool
occupied before the removal.

An operation that borrows a visibility pattern for its own length — sampling
one subtool alone, writing a file under a solo — SHALL re-evaluate nothing for
the flags it writes, because it restores the pattern before anything reads the
surface and the field is therefore the same field on both sides of it. Where a
restore does not complete, the layers left off the sculptor's pattern SHALL be
re-evaluated after all.

#### Scenario: A local edit in a large scene
- **WHEN** the same small edit is applied to the reference scene and to a scene ten times larger in surface area
- **THEN** the bricks re-evaluated and re-meshed are equivalent in both cases, and the measured cost does not scale with the larger scene

#### Scenario: Cancelling a curve costs the tube
- **WHEN** a curve is laid on a worked subtool and then cancelled
- **THEN** the bricks re-evaluated are the tube's own, the subtool's surface is
  left as it was, and no brick is left holding a sweep that is gone

#### Scenario: A boolean does not re-evaluate the subtools it never named
- **WHEN** a boolean runs over two subtools in a document holding others
- **THEN** no brick belonging to a subtool the boolean did not name is
  re-evaluated, and every subtool's visibility is what it was before
