## ADDED Requirements

### Requirement: An operation the composition root runs states its refusal where the interface reads it
A few operations are run by the composition root rather than dispatched to a
ViewModel, because each carries an answer back rather than a `Result<(), _>`.
Every one of them SHALL write its refusal to an observable channel the
interface draws and the agent door counts, on the same terms as a ViewModel's
own refusal: announced rather than set, so that the same impossible request
asked twice is two refusals and not one, while the words on screen do not
redraw for a repeat.

The refusal SHALL be cleared by the next operation that works, so the line
belongs to the last thing that was asked rather than to the last thing that
failed.

No operation's outcome SHALL be discarded — by `.is_ok()`, by an ignored
`Result`, or by a branch that neither acts nor states.

#### Scenario: A refused operation says why on screen
- **WHEN** an operation the composition root runs is refused
- **THEN** the reason appears on the interface's one "why that did not happen"
  line, in the words the operation was refused with

#### Scenario: The same refusal twice is two refusals
- **WHEN** the same impossible operation is asked for twice in a row
- **THEN** each attempt is counted as its own refusal, and the line on screen
  is not redrawn for the second

#### Scenario: A refusal does not outlive what it was about
- **WHEN** a refused operation is followed by one that works
- **THEN** the reason is taken down
